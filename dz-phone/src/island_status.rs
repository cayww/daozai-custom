//! 通用状态总线：任何脚本 / 应用写 `%APPDATA%\Daozai\island_status.json` 即可上岛。
//!
//! 覆写式语义：文件里的每一条就是「当前状态」。脚本持续覆写同一 `id` 即可常驻并刷新内容，
//! 条目从数组里删掉就立刻撤下，不需要重启岛仔，也不会在岛上堆积成一串旧提示。
//!
//! ```json
//! [{"id":"obs","text":"直播中 01:23","detail":"码率 6000kbps","seconds":0,
//!   "progress":0.55,"color":"green","click":"url:https://example.com"}]
//! ```
//!
//! 字段：`id`（缺省用 text 当 key）、`text` 标题、`detail` 副标题、`seconds`（0 = 默认 8 秒）、
//! `progress`（0~1，卡片底部进度条）、`color`（red/green/blue/amber/gray 白名单）、
//! `click`（copy:文本 / url:链接 / file:路径，动作白名单，不执行任意命令）。
//! 解析失败时保持上一次的状态，避免脚本写文件写一半把岛上的状态全撤了。

use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf, time::{Duration, Instant}};

const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Deserialize)]
pub struct Status {
    #[serde(default)] pub id: String,
    pub text: String,
    #[serde(default)] pub detail: String,
    #[serde(default)] pub seconds: f64,
    /// 0~1，画在提示卡片底部；None = 不画进度条。
    #[serde(default)] pub progress: Option<f64>,
    /// 状态色白名单，未知值回退灰色。
    #[serde(default)] pub color: String,
    /// 点击动作白名单，未知值不生成按钮。
    #[serde(default)] pub click: String,
}

impl Status {
    /// 同一 id 的覆写/续期/撤下都靠这个 key。
    pub fn key(&self) -> String {
        if self.id.is_empty() { self.text.clone() } else { self.id.clone() }
    }
}

#[derive(Default)]
pub struct Reader {
    checked: Option<Instant>,
    seen: HashSet<String>,
}

impl Reader {
    /// 返回 (当前存活的全部状态, 本次消失的 key)。
    /// 每轮都把存活状态重新推一遍：由 `enqueue_alert` 按来源替换，
    /// 既刷新内容又续期，于是「文件里有 = 岛上有」。
    pub fn poll(&mut self) -> (Vec<Status>, Vec<String>) {
        if self.checked.is_some_and(|at| at.elapsed() < INTERVAL) { return (Vec::new(), Vec::new()); }
        self.checked = Some(Instant::now());
        let Some(path) = path() else { return (Vec::new(), clear_seen(&mut self.seen)); };
        let Ok(raw) = std::fs::read_to_string(&path) else { return (Vec::new(), clear_seen(&mut self.seen)); };
        parse(&raw, &mut self.seen).unwrap_or_default()
    }
}

/// 解析成功才更新 seen；半写的 JSON 不会误撤下状态。
fn parse(raw: &str, seen: &mut HashSet<String>) -> Option<(Vec<Status>, Vec<String>)> {
    let list: Vec<Status> = serde_json::from_str(raw).ok()?;
    let live: HashSet<String> = list.iter().map(Status::key).collect();
    let gone: Vec<String> = seen.iter().filter(|k| !live.contains(*k)).cloned().collect();
    seen.clear();
    seen.extend(live);
    Some((list, gone))
}

fn clear_seen(seen: &mut HashSet<String>) -> Vec<String> {
    seen.drain().collect()
}

fn path() -> Option<PathBuf> {
    #[cfg(test)]
    return None;
    #[cfg(not(test))]
    {
        let base = std::env::var_os("APPDATA")?;
        Some(PathBuf::from(base).join("Daozai").join("island_status.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 状态格式可解析且字段可缺省() {
        let full: Status = serde_json::from_str(r#"{"id":"obs","text":"直播中","detail":"6000kbps","seconds":0,"progress":0.5,"color":"green","click":"url:https://x"}"#).unwrap();
        assert_eq!(full.id, "obs");
        assert_eq!(full.progress, Some(0.5));
        assert_eq!(full.color, "green");
        assert_eq!(full.click, "url:https://x");
        assert_eq!(full.key(), "obs");
        let short: Status = serde_json::from_str(r#"{"text":"构建完成"}"#).unwrap();
        assert!(short.id.is_empty() && short.detail.is_empty() && short.seconds == 0.0);
        assert_eq!(short.progress, None);
        assert_eq!(short.key(), "构建完成");
    }

    #[test]
    fn 覆写式状态存活即重推删掉即撤回() {
        let mut seen = HashSet::new();
        let raw = r#"[{"id":"obs","text":"直播中","progress":0.5,"color":"green"}]"#;
        let (list, gone) = parse(raw, &mut seen).unwrap();
        assert_eq!(list.len(), 1);
        assert!(gone.is_empty());
        assert_eq!(list[0].progress, Some(0.5));
        // 内容不变也要重推，用来给岛上的提示续期。
        let (again, gone) = parse(raw, &mut seen).unwrap();
        assert_eq!(again.len(), 1);
        assert!(gone.is_empty());
        // 内容变化同样是覆写，不是新增一条。
        let (changed, gone) = parse(r#"[{"id":"obs","text":"直播中 01:23"}]"#, &mut seen).unwrap();
        assert_eq!(changed[0].text, "直播中 01:23");
        assert!(gone.is_empty());
        // 条目删掉 → 返回撤回 key。
        let (empty, gone) = parse("[]", &mut seen).unwrap();
        assert!(empty.is_empty());
        assert_eq!(gone, vec!["obs"]);
    }

    #[test]
    fn 半写文件不会撤下状态() {
        let mut seen = HashSet::new();
        seen.insert("obs".to_string());
        assert!(parse("{bad json", &mut seen).is_none());
        assert!(seen.contains("obs"));
    }

    #[test]
    fn 无id时用标题当key() {
        let mut seen = HashSet::new();
        parse(r#"[{"text":"下载中"}]"#, &mut seen).unwrap();
        assert!(seen.contains("下载中"));
        let (_, gone) = parse(r#"[{"text":"下载完成"}]"#, &mut seen).unwrap();
        assert_eq!(gone, vec!["下载中"]);
    }
}
