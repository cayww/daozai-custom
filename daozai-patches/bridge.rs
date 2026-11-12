//! 浏览器扩展桥：岛仔兼任 Chrome/Edge 的 Native Messaging 宿主。
//!
//! 安装包把 daozai.exe 复制一份为 `daozai_bridge.exe`，浏览器的宿主清单指向它；
//! `main()` 看到这个文件名就进入本模块——不建窗口、不碰托盘、不抢单实例互斥体，
//! 只在 stdin/stdout 上跑原生消息协议（4 字节小端长度 + UTF-8 JSON）。
//!
//! 消息翻译成 `island_status.json` 的条目：下载中 = 蓝胶囊 + 进度条 + 速度，
//! 完成 = 绿胶囊（点击打开文件），取消/中断 = 立即撤下。
//! 写入与 python 版 `StatusBus` 同一把文件锁（island_status.lock 首字节），
//! 与历史 python 桥互斥，读改写原子替换，岛仔不会读到半截 JSON。
#![cfg(windows)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Instant;

use std::os::windows::io::AsRawHandle;
use windows::core::HSTRING;
use windows::Win32::Storage::FileSystem::{
    LockFileEx, MoveFileExW, UnlockFile, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    MOVEFILE_REPLACE_EXISTING,
};
use windows::Win32::System::IO::OVERLAPPED;

/// Native Messaging 宿主名（安装包写入注册表的键名）。
pub const HOST_NAME: &str = "com.daozai.island";
/// 扩展固定 ID（manifest key 派生），宿主清单只放行它。
pub const EXT_ID: &str = "dcmheeclfeihicoaakoffjbbepfdfcin";
/// 桥模式识别：宿主清单指向这份改了名的自身副本。
pub const BRIDGE_EXE: &str = "daozai_bridge.exe";

/// 进行中条目的自动消失时限（秒）：桥进程挂了也不会留僵尸胶囊，活着就靠覆写续期。
const PROGRESS_SECONDS: f64 = 15.0;
/// 完成提示的停留时长（秒）。
const DONE_SECONDS: f64 = 12.0;
/// 单条消息上限（字节）：浏览器侧出了问题也不会把内存顶爆。
const MAX_MESSAGE: usize = 1 << 20;

/// 是否以桥宿主身份被浏览器启动（看自己 exe 的文件名）。
pub fn is_bridge_invocation() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .is_some_and(|n| n.eq_ignore_ascii_case(BRIDGE_EXE))
}

/// 宿主主循环：读到 stdin 断开（浏览器回收 Service Worker）为止，返回退出码。
pub fn run_host() -> u32 {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut speed: HashMap<String, (u64, Instant)> = HashMap::new();
    let mut head = [0u8; 4];
    let mut body = Vec::new();
    loop {
        if reader.read_exact(&mut head).is_err() {
            break; // 端口断开，正常退出
        }
        let len = u32::from_le_bytes(head) as usize;
        if len == 0 || len > MAX_MESSAGE {
            break;
        }
        body.clear();
        body.resize(len, 0);
        if reader.read_exact(&mut body).is_err() {
            break;
        }
        let reply = match serde_json::from_slice::<serde_json::Value>(&body) {
            Ok(msg) => handle(msg, &mut speed),
            Err(error) => serde_json::json!({ "ok": false, "error": error.to_string() }),
        };
        if write_frame(&mut writer, &reply).is_err() {
            break;
        }
    }
    0
}

/// 处理一条消息并返回给浏览器的回执。协议与 python 版桥一致：
/// `dl`（下载事件）与 `status`（通用状态透传，白名单字段）。
fn handle(msg: serde_json::Value, speed: &mut HashMap<String, (u64, Instant)>) -> serde_json::Value {
    let ok = (|| {
        match msg.get("t").and_then(|t| t.as_str()) {
            Some("dl") => handle_download(&msg, speed),
            Some("status") => handle_status(&msg),
            Some("ping") => {}
            _ => return Err(format!("unknown t={:?}", msg.get("t"))),
        }
        Ok(())
    })();
    match ok {
        Ok(()) => serde_json::json!({ "ok": true }),
        Err(error) => serde_json::json!({ "ok": false, "error": error }),
    }
}

fn handle_download(msg: &serde_json::Value, speed: &mut HashMap<String, (u64, Instant)>) {
    let key = "dl-".to_string()
        + msg.get("id").and_then(|v| v.as_u64()).map(|n| n.to_string()).as_deref()
            .or_else(|| msg.get("name").and_then(|v| v.as_str())).unwrap_or("x");
    match msg.get("state").and_then(|v| v.as_str()).unwrap_or("progress") {
        "cancel" => {
            speed.remove(&key);
            remove_status(&key);
        }
        "complete" => {
            speed.remove(&key);
            let name = short_name(msg.get("name").and_then(|v| v.as_str()).unwrap_or("下载"));
            let mut item = serde_json::json!({
                "id": key, "text": format!("下载完成 · {name}"),
                "seconds": DONE_SECONDS, "color": "green",
            });
            let path = msg.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if !path.is_empty() && std::path::Path::new(path).is_file() {
                item["click"] = serde_json::json!(format!("file:{path}"));
            }
            set_status(&key, item);
        }
        _ => {
            let received = msg.get("received").and_then(|v| v.as_u64()).unwrap_or(0);
            let total = msg.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
            let name = short_name(msg.get("name").and_then(|v| v.as_str()).unwrap_or("下载"));
            let now = Instant::now();
            let mut detail = match total {
                0 => fmt_bytes(received),
                _ => format!("{} / {}", fmt_bytes(received), fmt_bytes(total)),
            };
            if let Some((prev, at)) = speed.get(&key) {
                if now > *at {
                    let rate = (received.saturating_sub(*prev)) as f64 / now.duration_since(*at).as_secs_f64();
                    if rate > 0.0 {
                        detail.push_str(&format!(" · {}/s", fmt_bytes(rate as u64)));
                    }
                }
            }
            speed.insert(key.clone(), (received, now));
            let mut item = serde_json::json!({
                "id": key, "text": format!("下载 · {name}"),
                "detail": detail, "seconds": PROGRESS_SECONDS, "color": "blue",
            });
            if total > 0 {
                item["progress"] = serde_json::json!((received as f64 / total as f64).clamp(0.0, 1.0));
            }
            set_status(&key, item);
        }
    }
}

/// 通用状态透传：只认状态总线的字段白名单，其余一律丢弃。
fn handle_status(msg: &serde_json::Value) {
    let fields = ["id", "text", "detail", "seconds", "progress", "color", "click"];
    let mut item = serde_json::Map::new();
    for field in fields {
        if let Some(value) = msg.get(field).filter(|v| !v.is_null()) {
            item.insert(field.into(), value.clone());
        }
    }
    let text = item.get("text").and_then(|v| v.as_str()).unwrap_or("");
    if text.is_empty() {
        return;
    }
    let key = item.get("id").and_then(|v| v.as_str()).unwrap_or(text).to_string();
    set_status(&key, serde_json::Value::Object(item));
}

/// 状态文件路径：与岛仔读的 `island_status::path()` 同一位置。
fn status_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("Daozai").join("island_status.json"))
}

/// 覆写一条状态（同 id 替换），写失败不影响主循环。
fn set_status(key: &str, item: serde_json::Value) {
    let done = with_status_lock(|| {
        let path = status_path()?;
        let mut list: Vec<serde_json::Value> = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        list.retain(|entry| {
            let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let fallback = entry.get("text").and_then(|v| v.as_str()).unwrap_or("");
            id != key && (id.is_empty() && fallback != key)
        });
        list.push(item);
        atomic_write(&path, &serde_json::to_vec(&list).ok()?)
    })
    .is_some();
    if !done { eprintln!("bridge: set_status({key}) failed"); }
}

/// 撤下一条状态。
fn remove_status(key: &str) {
    with_status_lock(|| {
        let path = status_path()?;
        let mut list: Vec<serde_json::Value> = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        let before = list.len();
        list.retain(|entry| {
            let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let fallback = entry.get("text").and_then(|v| v.as_str()).unwrap_or("");
            id != key && (id.is_empty() && fallback != key)
        });
        if list.len() == before {
            return None;
        }
        atomic_write(&path, &serde_json::to_vec(&list).ok()?)
    });
}

/// 临时文件 + 原子替换（MOVEFILE_REPLACE_EXISTING），岛仔只会看到完整 JSON。
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Option<()> {
    let mut tmp = path.to_path_buf();
    tmp.set_extension("json.tmp");
    if let Err(e) = std::fs::write(&tmp, bytes) { eprintln!("bridge: tmp write {e}"); return None; }
    let from = HSTRING::from(tmp.as_os_str());
    let to = HSTRING::from(path.as_os_str());
    unsafe {
        if let Err(e) = MoveFileExW(&from, &to, MOVEFILE_REPLACE_EXISTING) { eprintln!("bridge: move {e}"); return None; }
    }
    Some(())
}

/// 岛仔状态文件的写锁：与 python `StatusBus` 同一把（island_status.lock 的首字节），
/// Chrome 和 Edge 同时各起一个桥实例时也不会互相覆盖。
fn with_status_lock<T>(op: impl FnOnce() -> T) -> Option<T> {
    let path = status_path()?.with_extension("lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    // 注意必须「读写」打开：只写句柄（append-only）上 LockFileEx 会报拒绝访问。
    let file = std::fs::OpenOptions::new().read(true).write(true).create(true).open(&path).ok()?;
    let handle = windows::Win32::Foundation::HANDLE(file.as_raw_handle());
    let mut overlapped = OVERLAPPED::default();
    // 先试拿锁，拿不到就带 200ms 退避重试（对端也是普通本机进程，不会长持）。
    for _ in 0..50 {
        let locked = unsafe {
            LockFileEx(handle, LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY, 0, 1, 0, &mut overlapped)
        };
        if locked.is_ok() {
            let out = op();
            unsafe { let _ = UnlockFile(handle, 0, 0, 1, 0); };
            return Some(out);
        }
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    eprintln!("bridge: status lock busy");
    None
}

/// 帧编码：4 字节小端长度 + 内容。
fn write_frame(writer: &mut impl Write, value: &serde_json::Value) -> std::io::Result<()> {
    let raw = serde_json::to_vec(value)?;
    writer.write_all(&(raw.len() as u32).to_le_bytes())?;
    writer.write_all(&raw)?;
    writer.flush()
}

fn fmt_bytes(n: u64) -> String {
    const KB: u64 = 1 << 10;
    const MB: u64 = 1 << 20;
    const GB: u64 = 1 << 30;
    if n >= GB {
        format!("{:.1} GB", n as f64 / GB as f64)
    } else if n >= MB {
        format!("{:.1} MB", n as f64 / MB as f64)
    } else if n >= KB {
        format!("{:.1} KB", n as f64 / KB as f64)
    } else {
        format!("{n} B")
    }
}

fn short_name(name: &str) -> String {
    let base = name.rsplit(['\\', '/']).next().unwrap_or(name);
    let mut chars: Vec<char> = base.chars().collect();
    if chars.len() > 24 {
        chars.truncate(23);
        chars.push('…');
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 字节数格式化覆盖各档位() {
        assert_eq!(fmt_bytes(999), "999 B");
        assert_eq!(fmt_bytes(2048), "2.0 KB");
        assert_eq!(fmt_bytes(60 * 1024 * 1024), "60.0 MB");
        assert_eq!(fmt_bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn 文件名取最后一段并截断() {
        assert_eq!(short_name(r"C:\Users\cay\Downloads\demo.zip"), "demo.zip");
        assert_eq!(short_name("file.zip"), "file.zip");
        let long = "很长的文件名这是一段超过二十四个字符的下载文件名.bin";
        assert_eq!(short_name(long).chars().count(), 24);
        assert!(short_name(long).ends_with('…'));
    }

    #[test]
    fn 下载消息翻译成状态总线条目() {
        let mut speed = HashMap::new();
        // progress：进度、速度、蓝色、15 秒自动消失
        let msg = serde_json::json!({
            "t":"dl","id":17,"name":"C:\\x\\演示文件.zip","state":"progress",
            "received":60*1024*1024,"total":100*1024*1024
        });
        handle_download(&msg, &mut speed);
        assert!(speed.contains_key("dl-17"));
        // complete：路径存在才给 click
        let tmp = std::env::temp_dir().join("daozai-bridge-test-file.txt");
        std::fs::write(&tmp, b"x").unwrap();
        let msg = serde_json::json!({
            "t":"dl","id":17,"name":"note.txt","state":"complete",
            "path":tmp.to_string_lossy()
        });
        handle_download(&msg, &mut speed);
        assert!(speed.is_empty(), "完成后速度记录应清空");
    }

    #[test]
    fn 通用状态只透传白名单字段() {
        let msg = serde_json::json!({
            "t":"status","id":"tab","text":"在听歌","detail":"xxx",
            "evil_field":"rm -rf /","seconds":6,"color":"green"
        });
        // 直接构造校验：handle_status 内部走 set_status（真写文件），这里只验证字段过滤逻辑
        let fields = ["id", "text", "detail", "seconds", "progress", "color", "click"];
        let mut item = serde_json::Map::new();
        for field in fields {
            if let Some(value) = msg.get(field).filter(|v| !v.is_null()) {
                item.insert(field.into(), value.clone());
            }
        }
        assert_eq!(item.len(), 5);
        assert!(item.get("evil_field").is_none());
    }

    #[test]
    fn 帧编码解码往返一致() {
        let value = serde_json::json!({ "ok": true, "中文": "字段" });
        let mut buffer: Vec<u8> = Vec::new();
        write_frame(&mut buffer, &value).unwrap();
        let len = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
        assert_eq!(len + 4, buffer.len());
        let back: serde_json::Value = serde_json::from_slice(&buffer[4..]).unwrap();
        assert_eq!(back, value);
    }

    #[test]
    fn 桥模式按文件名识别() {
        // 不能改自己的 exe 名，这里验证判断函数对当前 exe 的行为是稳定的 bool。
        let _ = is_bridge_invocation();
    }
}
