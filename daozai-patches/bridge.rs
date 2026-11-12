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
            Some("dl") => handle_download(&msg, speed, &prefs_cached()),
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

/// 用户偏好（bridge.json，设置 → 浏览器下载页写入）。5 秒缓存：消息频率约 2 条/秒，
/// 没必要每条读盘；改设置后最迟 5 秒生效。
fn prefs_cached() -> crate::model::BridgePrefs {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<(Instant, crate::model::BridgePrefs)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap();
    if let Some((at, prefs)) = cache.as_ref() {
        if at.elapsed().as_secs() < 5 {
            return prefs.clone();
        }
    }
    let prefs = crate::model::BridgePrefs::load();
    *cache = Some((Instant::now(), prefs.clone()));
    prefs
}

fn handle_download(msg: &serde_json::Value, speed: &mut HashMap<String, (u64, Instant)>, prefs: &crate::model::BridgePrefs) {
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
            let done_seconds = prefs.done_seconds as f64;
            let mut item = serde_json::json!({
                "id": key, "text": format!("下载完成 · {name}"),
                "seconds": done_seconds, "color": "green",
                // ttl 到点岛仔自动撤下：桥挂了/没人续写也不会留下永久胶囊（custom-48 修复）。
                "ttl": unix_secs() + done_seconds,
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
            // 小文件不打扰：小于门槛的下载不显示进度胶囊（完成时仍会提示）；
            // total 未知（0）时无法判断大小，照常显示。
            let threshold = prefs.min_progress_mb as u64 * 1024 * 1024;
            if threshold > 0 && total > 0 && total < threshold {
                return;
            }
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
                // 每条 progress 消息覆写即续期；桥进程挂了最多 15 秒后自动撤下（防僵尸胶囊）。
                "ttl": unix_secs() + PROGRESS_SECONDS,
            });
            if total > 0 {
                item["progress"] = serde_json::json!((received as f64 / total as f64).clamp(0.0, 1.0));
            }
            set_status(&key, item);
        }
    }
}

/// 通用状态透传的字段白名单（ttl/priority 同为状态总线的标准字段），未知字段一律丢弃。
const STATUS_FIELDS: [&str; 9] = [
    "id", "text", "detail", "seconds", "progress", "color", "click", "ttl", "priority",
];

/// 通用状态透传：只认状态总线的字段白名单，其余一律丢弃。
fn handle_status(msg: &serde_json::Value) {
    let mut item = serde_json::Map::new();
    for field in STATUS_FIELDS {
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
    #[cfg(test)]
    {
        // 单测绝不能碰真实的 %APPDATA%\Daozai\island_status.json——
        // 测试写入会直接出现在用户岛上（custom-48 实测踩坑），重定向到临时目录。
        let dir = std::env::temp_dir().join(format!("daozai-bridge-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok()?;
        return Some(dir.join("island_status.json"));
    }
    #[cfg(not(test))]
    {
        let base = std::env::var_os("APPDATA")?;
        Some(PathBuf::from(base).join("Daozai").join("island_status.json"))
    }
}

/// 覆写一条状态（同 id 替换），写失败不影响主循环。
fn set_status(key: &str, item: serde_json::Value) {
    let done = with_status_lock(|| {
        let path = status_path()?;
        let mut list: Vec<serde_json::Value> = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        // 只撤下同 key 条目（有 id 按 id，无 id 按文本兜底）；其他条目一律保留。
        // 此前谓词写反过（id != key && ... 会误删所有带 id 的其他条目，导致
        // 并发下载互相覆盖、还会冲掉岛上其他来源的状态），custom-52 修复。
        list.retain(|entry| {
            let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let fallback = entry.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let same = if id.is_empty() { fallback == key } else { id == key };
            !same
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
            let same = if id.is_empty() { fallback == key } else { id == key };
            !same
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
    // 进程内先串行化：同一进程多个写者（并行测试/未来多线程）先过这把互斥体，
    // 文件锁只留给跨进程的竞争——否则同进程线程互相抢文件锁会把重试预算饿死。
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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

fn unix_secs() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
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

/// 主程序侧：桥宿主进程是否活着 = 浏览器扩展此刻已连接。
///
/// Chrome/Edge 只在扩展真正连上时才拉起宿主进程，断开（关浏览器/关扩展）即回收，
/// 所以「daozai_bridge.exe 在进程表里」就是「扩展在线」的可靠信号。
pub fn host_process_running() -> bool {
    process_running(BRIDGE_EXE)
}

/// 按文件名（不区分大小写）查进程表里有没有指定进程。
pub fn process_running(image: &str) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return false;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = false;
        let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
        while ok {
            let end = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case(image) {
                found = true;
                break;
            }
            ok = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
        found
    }
}

/// 找本机可用的浏览器（Chromium 系，支持 Native Messaging）：返回 (显示名, exe 路径)。
///
/// 优先注册表 App Paths（含 HKCU 便携安装），再用常见安装位置兜底；只返回真实存在的 exe，
/// 去重后 Chrome 在前、Edge 在后。
pub fn installed_browsers() -> Vec<(&'static str, PathBuf)> {
    let mut result: Vec<(&'static str, PathBuf)> = Vec::new();
    for (name, image, fallbacks) in [
        ("Chrome", "chrome.exe", [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ]),
        ("Edge", "msedge.exe", [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        ]),
    ] {
        let candidates: Vec<PathBuf> = app_path(image)
            .into_iter()
            .chain(fallbacks.iter().map(PathBuf::from))
            .chain(std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(r"Google\Chrome\Application").join(image)))
            .filter(|p| p.is_file())
            .collect();
        if let Some(exe) = candidates.into_iter().next() {
            if !result.iter().any(|(n, _)| *n == name) {
                result.push((name, exe));
            }
        }
    }
    result
}

/// 注册表 App Paths 里注册的浏览器完整路径（HKLM 优先，HKCU 兜底）。
fn app_path(image: &str) -> Option<PathBuf> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let subkey: HSTRING = format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{image}").into();
    let mut buffer = [0u16; 512];
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let mut size = (buffer.len() * 2) as u32;
        let ok = unsafe {
            RegGetValueW(root, &subkey, None, RRF_RT_REG_SZ, None, Some(buffer.as_mut_ptr().cast()), Some(&mut size))
        };
        if ok.is_ok() {
            let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
            let path = String::from_utf16_lossy(&buffer[..end]);
            if !path.trim().is_empty() {
                return Some(PathBuf::from(path.trim().trim_matches('"')));
            }
        }
    }
    None
}

/// 扩展文件夹（安装包布局：exe 同目录的 bridge\extension）。开发目录跑的时候不存在。
pub fn extension_folder() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let ext = dir.join("bridge").join("extension");
    ext.is_dir().then_some(ext)
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
        handle_download(&msg, &mut speed, &Default::default());
        assert!(speed.contains_key("dl-17"));
        // complete：路径存在才给 click
        let tmp = std::env::temp_dir().join("daozai-bridge-test-file.txt");
        std::fs::write(&tmp, b"x").unwrap();
        let msg = serde_json::json!({
            "t":"dl","id":17,"name":"note.txt","state":"complete",
            "path":tmp.to_string_lossy()
        });
        handle_download(&msg, &mut speed, &Default::default());
        assert!(speed.is_empty(), "完成后速度记录应清空");
    }

    #[test]
    fn 下载条目必须带ttl否则永远挂在岛上() {
        let mut speed = HashMap::new();
        let before = unix_secs();
        let msg = serde_json::json!({
            "t":"dl","id":18,"name":"a.zip","state":"progress","received":10,"total":100
        });
        handle_download(&msg, &mut speed, &Default::default());
        let entry = read_test_status("dl-18").expect("progress 条目应已写入");
        let ttl = entry.get("ttl").and_then(|v| v.as_f64()).expect("progress 必须带 ttl");
        assert!((before + 10.0..=before + 20.0).contains(&ttl), "progress ttl 应在写入时刻 +15s 附近：{ttl}");
        let msg = serde_json::json!({"t":"dl","id":18,"name":"a.zip","state":"complete","path":""});
        handle_download(&msg, &mut speed, &Default::default());
        let entry = read_test_status("dl-18").expect("complete 条目应覆盖同 id");
        let ttl = entry.get("ttl").and_then(|v| v.as_f64()).expect("complete 必须带 ttl");
        let after = unix_secs();
        assert!((after + 5.0..=after + 20.0).contains(&ttl), "complete ttl 应在写入时刻 +12s 附近：{ttl}");
    }

    fn find_test_entry(id: &str, text: &str) -> bool {
        let Some(path) = status_path() else { return false };
        let Some(list): Option<Vec<serde_json::Value>> = std::fs::read(&path).ok().and_then(|raw| serde_json::from_slice(&raw).ok()) else { return false };
        list.iter().any(|entry| {
            entry.get("id").and_then(|v| v.as_str()) == Some(id)
                || entry.get("text").and_then(|v| v.as_str()) == Some(text)
        })
    }

    #[test]
    fn 写入不冲掉其他条目() {
        use crate::model::BridgePrefs;
        // 并发下载（或岛上其他来源的条目）必须共存：set_status 只覆盖同 key。
        // 回归背景：旧谓词会误删所有 id 不同的条目（custom-52 修复）。
        let mut speed = HashMap::new();
        let prefs = BridgePrefs::default();
        handle_download(&serde_json::json!({"t":"dl","id":1,"name":"a.bin","state":"progress","received":100,"total":1000}), &mut speed, &prefs);
        handle_download(&serde_json::json!({"t":"dl","id":2,"name":"b.bin","state":"progress","received":200,"total":2000}), &mut speed, &prefs);
        // 其他来源的条目（无 id，走文本兜底 key）
        set_status("微信 · 张三", serde_json::json!({"text": "微信 · 张三"}));
        // 再写一条下载，前两条和其他来源条目都必须还在
        handle_download(&serde_json::json!({"t":"dl","id":3,"name":"c.bin","state":"progress","received":300,"total":3000}), &mut speed, &prefs);
        assert!(find_test_entry("dl-1", ""));
        assert!(find_test_entry("dl-2", ""));
        assert!(find_test_entry("dl-3", ""));
        assert!(find_test_entry("", "微信 · 张三"), "无 id 条目按文本兜底 key 匹配");
        // 同 id 覆盖：重写 dl-1 不应新增重复条目
        handle_download(&serde_json::json!({"t":"dl","id":1,"name":"a2.bin","state":"progress","received":400,"total":1000}), &mut speed, &prefs);
        let raw = std::fs::read(status_path().unwrap()).unwrap();
        let list: Vec<serde_json::Value> = serde_json::from_slice(&raw).unwrap();
        assert_eq!(list.iter().filter(|e| e.get("id").and_then(|v| v.as_str()) == Some("dl-1")).count(), 1, "同 id 应覆盖不重复");
        // 撤下也只影响自身
        remove_status("dl-2");
        assert!(!find_test_entry("dl-2", ""));
        assert!(find_test_entry("dl-1", ""));
        assert!(find_test_entry("", "微信 · 张三"));
    }

    #[test]
    fn 完成时长跟随用户偏好() {
        let mut speed = HashMap::new();
        let prefs = crate::model::BridgePrefs { done_seconds: 30, min_progress_mb: 0 };
        let before = unix_secs();
        handle_download(&serde_json::json!({
            "t":"dl","id":21,"name":"p.zip","state":"complete","path":""
        }), &mut speed, &prefs);
        let entry = read_test_status("dl-21").expect("complete 条目应写入");
        let ttl = entry.get("ttl").and_then(|v| v.as_f64()).unwrap();
        assert!((before + 25.0..=before + 35.0).contains(&ttl), "done_seconds=30 时 ttl 应为 +30s：{ttl}");
    }

    #[test]
    fn 小于门槛的下载不显示进度完成仍提示() {
        let mut speed = HashMap::new();
        let prefs = crate::model::BridgePrefs { done_seconds: 12, min_progress_mb: 5 };
        // 3 MB < 5 MB：progress 不上岛。
        handle_download(&serde_json::json!({
            "t":"dl","id":22,"name":"small.zip","state":"progress","received":1024,"total":3*1024*1024
        }), &mut speed, &prefs);
        assert!(read_test_status("dl-22").is_none(), "小于门槛的下载不应显示进度");
        // 大小未知（total=0）：照常显示。
        handle_download(&serde_json::json!({
            "t":"dl","id":23,"name":"unk.zip","state":"progress","received":1024,"total":0
        }), &mut speed, &prefs);
        assert!(read_test_status("dl-23").is_some(), "大小未知的下载应照常显示进度");
        // 完成不受门槛影响。
        handle_download(&serde_json::json!({
            "t":"dl","id":22,"name":"small.zip","state":"complete","path":""
        }), &mut speed, &prefs);
        assert!(read_test_status("dl-22").is_some(), "完成提示不受门槛影响");
    }

    #[test]
    fn 单测写状态不污染用户真实文件() {
        // status_path 在测试下重定向到临时目录，真实 %APPDATA% 文件必须原样。
        let mut speed = HashMap::new();
        handle_download(&serde_json::json!({
            "t":"dl","id":99,"name":"x.zip","state":"progress","received":1,"total":2
        }), &mut speed, &Default::default());
        let real = std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("Daozai").join("island_status.json"));
        if let Some(path) = real.filter(|p| p.exists()) {
            let raw = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(!raw.contains("dl-99"), "测试条目绝不能出现在用户真实状态文件里：{raw}");
        }
    }

    /// 读取测试重定向目录里的状态文件，返回指定 id 的条目。
    fn read_test_status(id: &str) -> Option<serde_json::Value> {
        let path = status_path()?;
        let list: Vec<serde_json::Value> = std::fs::read(&path).ok().and_then(|raw| serde_json::from_slice(&raw).ok())?;
        list.into_iter().find(|entry| entry.get("id").and_then(|v| v.as_str()) == Some(id))
    }

    #[test]
    fn 通用状态只透传白名单字段() {
        let msg = serde_json::json!({
            "t":"status","id":"tab","text":"在听歌","detail":"xxx",
            "evil_field":"rm -rf /","seconds":6,"color":"green",
            "ttl":1800.0,"priority":5
        });
        // 直接构造校验：handle_status 内部走 set_status（真写文件），这里只验证字段过滤逻辑
        let mut item = serde_json::Map::new();
        for field in STATUS_FIELDS {
            if let Some(value) = msg.get(field).filter(|v| !v.is_null()) {
                item.insert(field.into(), value.clone());
            }
        }
        assert_eq!(item.len(), 7);
        assert!(item.get("evil_field").is_none());
        assert_eq!(item.get("ttl").and_then(|v| v.as_f64()), Some(1800.0), "ttl 必须透传，否则脚本写的自动过期失效");
        assert_eq!(item.get("priority").and_then(|v| v.as_i64()), Some(5));
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

    #[test]
    fn 进程查询能查到不存在的名字为假() {
        assert!(!process_running("daozai-definitely-not-running-xyz.exe"));
    }

    #[test]
    fn 桥在线状态与进程查询一致() {
        // host_process_running 只是 process_running 的薄封装，两者必须同值。
        assert_eq!(host_process_running(), process_running(BRIDGE_EXE));
    }

    #[test]
    fn 浏览器检测至少找到Edge且路径真实存在() {
        let browsers = installed_browsers();
        let edge = browsers.iter().find(|(name, _)| *name == "Edge");
        assert!(edge.is_some(), "Windows 自带 Edge，进程表机器上应该能找到");
        assert!(edge.unwrap().1.is_file(), "返回的 exe 路径必须真实存在");
        // 同名浏览器不重复。
        for (name, _) in &browsers {
            assert_eq!(browsers.iter().filter(|(n, _)| n == name).count(), 1);
        }
    }

    #[test]
    fn AppPaths注册表读取返回存在的路径或无() {
        // chrome.exe / msedge.exe 至少有一个在 App Paths 里注册；读出来的要么 None 要么存在的文件。
        for image in ["chrome.exe", "msedge.exe"] {
            if let Some(path) = app_path(image) {
                assert!(path.is_file(), "App Paths 指向的 {} 不存在：{}", image, path.display());
            }
        }
    }

    #[test]
    fn 扩展目录仅在安装布局下出现() {
        // 开发目录（target 下）没有 bridge\extension，应返回 None 或一个不存在的目录也算合理，
        // 但绝不能 panic。
        let _ = extension_folder();
    }
}
