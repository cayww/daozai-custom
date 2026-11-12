//! MAA（MaaAssistantArknights）对接：读它的 debug 日志，把运行状态显示到岛上。
//!
//! MAA 不自带「推送给第三方」的接口，但它的核心日志 `debug/asst.log` 会把每一条
//! 回调消息原样打印出来（`Assistant::append_callback | <类型> <JSON>`），
//! 这是最稳妥的对接点——只读日志，不改 MAA、不需要它在后台开服务。
//!
//! 关心的事件：
//! - `TaskChainStart` / `TaskChainCompleted`：某个任务链开始/结束（公招、理智作战…）
//! - `AllTasksCompleted`：全部日常跑完
//! - `SubTaskExtraInfo` 且 `class=asst::AutoRecruitTask` 且 `details.level>=4`：
//!   公招刷出 4 星以上 tag，值得提醒
//! - `SubTaskError` / `TaskChainError`：出错
//!
//! 日志可能很大（几十 MB），只从上次读到的偏移往后读，增量解析。

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// 一次最多读这么多新增字节，避免日志暴涨时一帧读太多把界面卡住。
const MAX_READ: u64 = 512 * 1024;

/// MAA 的事件种类（决定岛上怎么显示）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// 任务链开始：`(任务链英文名, 中文名)`。
    ChainStart(String, String),
    /// 任务链完成。
    ChainDone(String, String),
    /// 全部任务完成：总用时描述（可能为空）。
    AllDone(String),
    /// 公招刷出高星 tag：星级。
    RecruitStar(u8),
    /// 出错：任务名 + 错误摘要。
    Error(String),
}

/// 把 MAA 的任务链英文名翻译成中文（未知原样返回）。
pub fn chain_label(chain: &str) -> String {
    match chain {
        "StartUp" => "开始唤醒",
        "CloseDown" => "关闭游戏",
        "Fight" => "理智作战",
        "Recruit" => "自动公招",
        "Infrast" => "基建换班",
        "Mall" => "信用购物",
        "Award" => "领取奖励",
        "Roguelike" => "自动肉鸽",
        "Copilot" => "自动战斗",
        "Reclamation" => "生息演算",
        "Custom" => "自定义任务",
        "Depot" => "仓库识别",
        "OperBox" => "干员识别",
        other => other,
    }
    .to_string()
}

/// 解析一行日志里的回调消息。非回调行返回 None。
///
/// 形如：`... Assistant::append_callback | TaskChainStart {"taskchain":"Recruit",...}`
pub fn parse_callback(line: &str) -> Option<Event> {
    let marker = "append_callback | ";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let (kind, payload) = match rest.split_once(' ') {
        Some((k, p)) => (k.trim(), p.trim()),
        None => (rest.trim(), ""),
    };
    match kind {
        "TaskChainStart" => {
            let chain = json_str(payload, "taskchain")?;
            Some(Event::ChainStart(chain.clone(), chain_label(&chain)))
        }
        "TaskChainCompleted" => {
            let chain = json_str(payload, "taskchain")?;
            Some(Event::ChainDone(chain.clone(), chain_label(&chain)))
        }
        "AllTasksCompleted" => Some(Event::AllDone(String::new())),
        "SubTaskExtraInfo" => {
            // 只关心公招高星：class=asst::AutoRecruitTask 且 details.level >= 4。
            if !payload.contains("AutoRecruitTask") {
                return None;
            }
            let level = json_u8(payload, "\"level\":")?;
            if level >= 4 {
                Some(Event::RecruitStar(level))
            } else {
                None
            }
        }
        "SubTaskError" | "TaskChainError" => {
            let chain = json_str(payload, "taskchain").unwrap_or_else(|| "任务".into());
            Some(Event::Error(chain_label(&chain)))
        }
        _ => None,
    }
}

/// 极简取值：在 JSON 文本里找 `"key":"value"` 的 value（不引第三方 JSON 解析，
/// 回调消息结构固定、字段顺序无关，够用且稳）。
fn json_str(payload: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let start = payload.find(&needle)? + needle.len();
    let rest = &payload[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn json_u8(payload: &str, needle: &str) -> Option<u8> {
    let start = payload.find(needle)? + needle.len();
    let rest = payload[start..].trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// 从 MAA 可执行文件路径推出日志目录（`<MAA目录>/debug`）。
pub fn log_dir_from_exe(exe: &str) -> Option<PathBuf> {
    let dir = Path::new(exe).parent()?;
    let debug = dir.join("debug");
    debug.is_dir().then_some(debug)
}

/// 日志跟随器：记住读到的位置，每次 poll 只解析新增部分。
#[derive(Default)]
pub struct Watcher {
    path: Option<PathBuf>,
    offset: u64,
    /// 首轮只对齐到文件末尾（不把历史日志一次性全弹出来）。
    primed: bool,
    /// 上次检查时刻：轮询节流，避免每帧都去 stat/open 日志文件。
    last_check: Option<std::time::Instant>,
}

impl Watcher {
    /// 指定要跟随的日志文件；路径变化时重置位置。
    pub fn follow(&mut self, path: Option<PathBuf>) {
        if self.path != path {
            self.path = path;
            self.offset = 0;
            self.primed = false;
            self.last_check = None;
        }
    }

    /// 读新增内容并返回解析出的事件（最多读到文件末尾）。
    pub fn poll(&mut self) -> Vec<Event> {
        // 500ms 一次足够实时，又不会每帧读盘。
        if self.last_check.is_some_and(|at| at.elapsed().as_millis() < 500) {
            return Vec::new();
        }
        self.last_check = Some(std::time::Instant::now());
        let Some(path) = self.path.clone() else { return Vec::new(); };
        let Ok(mut file) = std::fs::File::open(&path) else { return Vec::new(); };
        let Ok(len) = file.metadata().map(|m| m.len()) else { return Vec::new(); };

        // 日志被轮转/截断（新长度比记录位置短）时从头开始。
        if len < self.offset {
            self.offset = 0;
            self.primed = false;
        }
        if !self.primed {
            // 首轮直接跳到末尾，忽略历史。
            self.offset = len;
            self.primed = true;
            return Vec::new();
        }
        if len == self.offset {
            return Vec::new();
        }

        let to_read = (len - self.offset).min(MAX_READ);
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::with_capacity(to_read as usize);
        let Ok(read) = file.take(to_read).read_to_end(&mut buf) else { return Vec::new(); };
        self.offset += read as u64;

        // 只处理完整的行：最后一段可能是半行，留到下次（把 offset 回退到最后一个换行）。
        let Some(last_nl) = buf.iter().rposition(|b| *b == b'\n') else {
            self.offset -= read as u64;
            return Vec::new();
        };
        let consumed = last_nl + 1;
        self.offset -= (read - consumed) as u64;

        let text = String::from_utf8_lossy(&buf[..consumed]);
        text.lines().filter_map(parse_callback).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解析任务链开始与完成() {
        let line = r#"[2026-11-12 20:24:27.156][INF][Px1][Tx2] Assistant::append_callback | TaskChainStart {"taskchain":"Recruit","taskid":6,"uuid":"x"}"#;
        assert_eq!(parse_callback(line), Some(Event::ChainStart("Recruit".into(), "自动公招".into())));
        let done = r#"[..] Assistant::append_callback | TaskChainCompleted {"taskchain":"Mall","taskid":7}"#;
        assert_eq!(parse_callback(done), Some(Event::ChainDone("Mall".into(), "信用购物".into())));
    }

    #[test]
    fn 解析全部完成与错误() {
        let all = r#"[..] Assistant::append_callback | AllTasksCompleted {"finished_tasks":[4,5,6],"taskchain":"Award","taskid":8}"#;
        assert_eq!(parse_callback(all), Some(Event::AllDone(String::new())));
        let err = r#"[..] Assistant::append_callback | SubTaskError {"taskchain":"Fight","taskid":3}"#;
        assert_eq!(parse_callback(err), Some(Event::Error("理智作战".into())));
    }

    #[test]
    fn 只在公招高星时提醒() {
        let high = r#"[..] Assistant::append_callback | SubTaskExtraInfo {"class":"asst::AutoRecruitTask","details":{"level":5,"result":[]},"subtask":"AutoRecruitTask"}"#;
        assert_eq!(parse_callback(high), Some(Event::RecruitStar(5)));
        let low = r#"[..] Assistant::append_callback | SubTaskExtraInfo {"class":"asst::AutoRecruitTask","details":{"level":3,"result":[]}}"#;
        assert_eq!(parse_callback(low), None);
        // 其他类的 ExtraInfo 不关心。
        let other = r#"[..] Assistant::append_callback | SubTaskExtraInfo {"class":"asst::FightTask","details":{"level":6}}"#;
        assert_eq!(parse_callback(other), None);
    }

    #[test]
    fn 非回调行忽略() {
        assert_eq!(parse_callback("[..][TRC] asst::Assistant::~Assistant | enter"), None);
        assert_eq!(parse_callback(""), None);
    }

    #[test]
    fn 新增内容增量读取且不重复() {
        let dir = std::env::temp_dir().join(format!("maa-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("asst.log");
        std::fs::write(&log, "").unwrap();

        let mut w = Watcher::default();
        w.follow(Some(log.clone()));
        // 首轮对齐末尾，不吐历史。
        assert!(w.poll().is_empty());

        let line = "[..] Assistant::append_callback | TaskChainStart {\"taskchain\":\"Fight\",\"taskid\":1}\n";
        std::fs::write(&log, line).unwrap();
        w.last_check = None; // 跳过 500ms 轮询节流，模拟下一次检查窗口。
        assert_eq!(w.poll(), vec![Event::ChainStart("Fight".into(), "理智作战".into())]);
        // 再 poll 不应重复同一条。
        w.last_check = None;
        assert!(w.poll().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
