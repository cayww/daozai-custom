//! 剪贴板记录：最近复制的文字与图片，在置物架页的「剪贴板」里查看、再次复制、固定。
//!
//! - 文字直接存 `clipboard.json`；图片落盘为 PNG（`clip-images/`），JSON 只记元数据。
//! - 图片有尺寸与文件大小上限，避免大图占满磁盘。
//! - 最近记录与固定条目都保存在本机。
//! - 密码管理器等标记为「不进剪贴板历史」的内容一律跳过
//!   （`ExcludeClipboardContentFromMonitorProcessing` / `CanIncludeInClipboardHistory = 0`）。
//! - Windows 上用 `AddClipboardFormatListener` 等系统通知，没有轮询；
//!   监听线程只在剪贴板真正变化时醒来。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 未固定条目的上限；固定的不占这个名额。
pub const MAX_RECENT: usize = 30;
/// 单条最长字符数，超过的不记录（大段日志、整份文件内容）。
pub const MAX_CHARS: usize = 20_000;
pub const MAX_PINNED: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub text: String,
    /// 本地时间 `HH:MM`。
    pub at: String,
    pub pinned: bool,
    /// 最后复制的 Unix 秒，用于清理 24 小时前的未固定内容。
    /// 固定条目永远保留，不因这个字段被清理。
    pub saved_at: i64,
    /// 图片条目：PNG 文件名（相对 config_dir()/clip-images/）+ 宽高。
    /// 文字条目为 None。两者互斥：有图片时 text 只作搜索/提示用的描述。
    pub image: Option<ImageMeta>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageMeta {
    /// 落盘文件名，仅 basename，不含路径分隔符。
    pub file: String,
    pub width: u32,
    pub height: u32,
}

/// 图片入历史的上限：超过就不记录，避免截屏/设计稿把磁盘撑爆。
pub const MAX_IMAGE_PIXELS: u32 = 16_000_000; // 约 4000×4000
pub const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024; // 解码后的 RGBA 上限

/// 图片落盘目录。测试时由 DAOZAI_DATA_DIR 隔离（见 model::config_dir）。
fn images_dir() -> std::path::PathBuf { crate::model::config_dir().join("clip-images") }

/// 生成不猜路径的文件名：时间戳(毫秒)+进程+序号，纯 basename。
fn image_file_name() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis()).unwrap_or_default();
    format!("clip-{ms}-{}-{}.png", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed))
}

/// 把解码后的 RGBA 图片写盘。只在 Windows 监听线程里调用。
#[cfg(windows)]
fn save_image_png(rgba: &[u8], width: u32, height: u32) -> Option<ImageMeta> {
    use image::ImageEncoder;
    if width == 0 || height == 0
        || width.saturating_mul(height) > MAX_IMAGE_PIXELS
        || rgba.len() > MAX_IMAGE_BYTES { return None; }
    let file = image_file_name();
    let dir = images_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(&file);
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8).ok()?;
    // 原子写：和 clipboard.json 一样避免中途崩溃留下半张图。
    crate::model::atomic_write(&path, &buf).ok()?;
    Some(ImageMeta { file, width, height })
}

/// 删除一张落盘图片。失效条目/过期条目清理时调用。
fn remove_image_file(meta: &ImageMeta) {
    if meta.file.is_empty() || meta.file.contains('/') || meta.file.contains('\\') { return; }
    let _ = std::fs::remove_file(images_dir().join(&meta.file));
}

/// 未固定剪贴板只保留最近 24 小时；固定条目不受影响。
/// 旧的 JSON 没有时间戳时按「今天刚读到」迁移，不会在升级时突然清空历史。
const RETENTION_SECS: i64 = 24 * 60 * 60;

fn now_secs() -> i64 { chrono::Local::now().timestamp() }

#[derive(Debug, Default)]
pub struct History {
    pub items: Vec<Clip>,
    /// 每次变化 +1，界面据此判断是否需要重绘。
    pub version: u64,
    /// 有未落盘的改动；由 `flush()` 在锁外统一写盘。
    dirty: bool,
    path: Option<std::path::PathBuf>,
}

impl History {
    /// 新内容放最前；同样的文字只留一条（移到最前、保留固定状态）。
    pub fn push(&mut self, text: &str, at: &str) -> bool {
        if text.trim().is_empty() || text.chars().count() > MAX_CHARS { return false; }
        let pinned = match self.items.iter().position(|c| c.text == text) {
            Some(0) => return false,
            Some(i) => self.items.remove(i).pinned,
            None => false,
        };
        self.items.insert(0, Clip { text: text.to_string(), at: at.to_string(), pinned, saved_at: now_secs(), image: None });
        let mut recent = 0;
        self.items.retain(|c| c.pinned || { recent += 1; recent <= MAX_RECENT });
        self.changed();
        true
    }

    /// 图片入历史。按文件名去重（每次复制生成新文件名，所以图片不会互相合并），
    /// 与文字共用 24h/固定/上限规则。返回是否真的插入。
    pub fn push_image(&mut self, meta: ImageMeta, at: &str) -> bool {
        let text = format!("图片 {}×{}", meta.width, meta.height);
        self.items.insert(0, Clip { text, at: at.to_string(), pinned: false, saved_at: now_secs(), image: Some(meta) });
        let mut recent = 0;
        self.items.retain(|c| c.pinned || { recent += 1; recent <= MAX_RECENT });
        self.changed();
        true
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.items.len() {
            if let Some(meta) = self.items[index].image.take() { remove_image_file(&meta); }
            self.items.remove(index);
            self.changed();
        }
    }

    /// 清空未固定的条目，固定的保留。图片文件随未固定条目一起删。
    pub fn clear_recent(&mut self) {
        let mut removed = Vec::new();
        self.items.retain(|c| {
            if c.pinned { true } else {
                if let Some(meta) = &c.image { removed.push(meta.clone()); }
                false
            }
        });
        for meta in removed { remove_image_file(&meta); }
        self.changed();
    }

    pub fn toggle_pin(&mut self, index: usize) -> bool {
        let pinned_count = self.items.iter().filter(|c| c.pinned).count();
        let Some(item) = self.items.get_mut(index) else { return false; };
        if !item.pinned && pinned_count >= MAX_PINNED { return false; }
        item.pinned = !item.pinned;
        self.changed();
        true
    }

    fn to_json(&self) -> Vec<u8> {
        let rows: Vec<_> = self.items.iter().map(|c| {
            let mut row = serde_json::json!({"pinned": c.pinned,"text": c.text, "at": c.at, "saved_at": c.saved_at});
            if let Some(im) = &c.image {
                row["image"] = serde_json::json!({"file": im.file, "width": im.width, "height": im.height});
            }
            row
        }).collect();
        serde_json::to_vec(&rows).unwrap_or_default()
    }

    pub fn from_json(raw: &[u8]) -> Self {
        let rows: Vec<serde_json::Value> = serde_json::from_slice(raw).unwrap_or_default();
        let items = rows.iter().filter_map(|row| {
            let image = row.get("image").and_then(|v| {
                let file = v.get("file")?.as_str()?.to_string();
                // 只接受纯文件名，防止被篡改的 JSON 指向任意路径。
                if file.is_empty() || file.contains('/') || file.contains('\\') || file.contains("..") { return None; }
                Some(ImageMeta {
                    file,
                    width: v.get("width").and_then(|n| n.as_u64()).unwrap_or(0) as u32,
                    height: v.get("height").and_then(|n| n.as_u64()).unwrap_or(0) as u32,
                })
            });
            Some(Clip {
                text: row.get("text")?.as_str()?.to_string(),
                at: row.get("at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                pinned: row.get("pinned").and_then(|v| v.as_bool()).unwrap_or(true),
                saved_at: row.get("saved_at").and_then(|v| v.as_i64()).unwrap_or_else(now_secs),
                image,
            })
        }).filter(|c| !c.text.is_empty()).take(MAX_PINNED + MAX_RECENT).collect();
        Self { items, ..Self::default() }
    }

    fn path() -> std::path::PathBuf { crate::model::config_dir().join("clipboard.json") }

    pub fn load() -> Self {
        Self::load_from(Self::path())
    }

    fn load_from(path: std::path::PathBuf) -> Self {
        let mut history = std::fs::read(&path).map(|raw| Self::from_json(&raw)).unwrap_or_default();
        let oldest = now_secs() - RETENTION_SECS;
        // 敏感内容只在本机保留一天，重启时顺带清理；固定的片段模板不在此列。
        let mut expired = Vec::new();
        history.items.retain(|clip| {
            let keep = clip.pinned || clip.saved_at >= oldest;
            if !keep { if let Some(meta) = &clip.image { expired.push(meta.clone()); } }
            keep
        });
        for meta in expired { remove_image_file(&meta); }
        history.path = Some(path);
        history
    }

    /// 只标记「有改动」。写盘交给 `flush()` 在锁外做——
    /// 之前是在 Mutex 里直接 atomic_write，界面每帧取快照时会被磁盘 IO 卡住。
    fn changed(&mut self) {
        self.version += 1;
        self.dirty = true;
    }

    /// 取出待写盘的内容（同时清掉 dirty）。没有改动返回 None。
    fn take_dirty(&mut self) -> Option<(std::path::PathBuf, Vec<u8>)> {
        if !self.dirty { return None; }
        let path = self.path.clone()?;
        self.dirty = false;
        Some((path, self.to_json()))
    }
}

/// 界面与监听线程共用。
#[derive(Clone)]
pub struct Shared {
    pub history: Arc<Mutex<History>>,
    pub enabled: Arc<AtomicBool>,
}

impl Shared {
    pub fn new(enabled: bool) -> Self {
        Self { history: Arc::new(Mutex::new(History::load())), enabled: Arc::new(AtomicBool::new(enabled)) }
    }
    pub fn set_enabled(&self, on: bool) { self.enabled.store(on, Ordering::Relaxed); }
    pub fn is_enabled(&self) -> bool { self.enabled.load(Ordering::Relaxed) }
    pub fn lock(&self) -> std::sync::MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// 有改动才写盘，且整段在锁外完成；一次复制只序列化一次。
    pub fn flush(&self) {
        let payload = { let mut history = self.lock(); history.take_dirty() };
        let Some((path, body)) = payload else { return; };
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        let _ = crate::model::atomic_write(&path, &body);
    }

    /// 只读版本号（加锁极短），界面先用它判断是否需要重建快照。
    pub fn version(&self) -> u64 { self.lock().version }

    /// 当前列表 + 版本号。界面用版本号判断要不要重建快照，避免每帧克隆整份历史。
    pub fn snapshot(&self) -> (u64, std::sync::Arc<Vec<Clip>>) {
        let history = self.lock();
        (history.version, std::sync::Arc::new(history.items.clone()))
    }

    fn record_update(&self, text: &str, from_history: bool) -> bool {
        !from_history && self.record(text)
    }
    pub fn record(&self, text: &str) -> bool {
        if !self.is_enabled() { return false; }
        let at = chrono::Local::now().format("%H:%M").to_string();
        let pushed = self.lock().push(text, &at);
        self.flush();
        pushed
    }

    /// 图片条目入历史（监听线程在 Win32 侧落盘后调用）。
    pub fn record_image(&self, meta: ImageMeta) -> bool {
        if !self.is_enabled() { return false; }
        let at = chrono::Local::now().format("%H:%M").to_string();
        let pushed = self.lock().push_image(meta, &at);
        self.flush();
        pushed
    }
}

impl Default for Shared {
    fn default() -> Self { Self::new(true) }
}

/// 列表里的一行预览：换行与连续空白压成一个空格。
pub fn preview(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- Windows 监听

/// 剪贴板监听的持有者；销毁时让监听线程退出并注销系统监听，不等待线程结束。
pub struct Watcher {
    #[cfg(windows)]
    stop: Arc<win::Stop>,
}

#[cfg(windows)]
impl Drop for Watcher {
    fn drop(&mut self) { self.stop.request(); }
}

#[cfg(windows)]
pub fn watch(shared: Shared, repaint: impl Fn() + Send + 'static) -> Option<Watcher> {
    let stop = Arc::new(win::Stop::default());
    let thread_stop = stop.clone();
    std::thread::Builder::new().name("daozai-clipboard".into()).spawn(move || {
        // 启动失败（极少见）时剪贴板页只是没有新记录，不影响其他功能。
        let _ = win::run(shared, Box::new(repaint), &thread_stop);
    }).ok()?;
    Some(Watcher { stop })
}

#[cfg(not(windows))]
pub fn watch(_shared: Shared, _repaint: impl Fn() + Send + 'static) -> Option<Watcher> { Some(Watcher {}) }

#[cfg(not(windows))]
pub fn copy_history(_text: &str) -> bool { false }

#[cfg(not(windows))]
pub fn copy_history_image(_meta: &ImageMeta) -> bool { false }

#[cfg(windows)]
const HISTORY_COPY_FORMAT: &str = "Daozai.ClipboardHistoryCopy";

#[cfg(windows)]
pub fn copy_history(text: &str) -> bool {
    use clipboard_win::{Clipboard, raw, register_format};
    let Some(format) = register_format(HISTORY_COPY_FORMAT) else { return false };
    let Ok(_clipboard) = Clipboard::new_attempts(10) else { return false };
    raw::set_string(text).is_ok() && raw::set_without_clear(format.get(), &[1]).is_ok()
}

/// 把历史里的图片条目重新写回系统剪贴板（带自我记录标记）。
/// 从 PNG 还原为 CF_DIB（BITMAPINFOHEADER + 24/32bpp 像素）。
#[cfg(windows)]
pub fn copy_history_image(meta: &ImageMeta) -> bool {
    use clipboard_win::{Clipboard, raw, register_format};
    if meta.file.is_empty() || meta.file.contains('/') || meta.file.contains('\\') || meta.file.contains("..") { return false; }
    let Ok(png) = std::fs::read(images_dir().join(&meta.file)) else { return false; };
    let Ok(img) = image::load_from_memory(&png) else { return false; };
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    if w == 0 || h == 0 || w > 16384 || h > 16384 { return false; }
    // BITMAPINFOHEADER（40B）+ 32bpp BGRA，biHeight 为正表示自下而上。
    let row_bytes = (w as usize) * 4;
    let mut dib = vec![0u8; 40 + row_bytes * h as usize];
    dib[0..4].copy_from_slice(&40u32.to_le_bytes());
    dib[4..8].copy_from_slice(&(w as i32).to_le_bytes());
    dib[8..12].copy_from_slice(&(h as i32).to_le_bytes());
    dib[12..14].copy_from_slice(&1u16.to_le_bytes());
    dib[14..16].copy_from_slice(&32u16.to_le_bytes());
    let src = rgba.as_raw();
    for y in 0..h as usize {
        let dst = 40 + (h as usize - 1 - y) * row_bytes;
        let srow = &src[y * row_bytes..(y + 1) * row_bytes];
        let drow = &mut dib[dst..dst + row_bytes];
        for x in 0..w as usize {
            drow[x * 4] = srow[x * 4 + 2];
            drow[x * 4 + 1] = srow[x * 4 + 1];
            drow[x * 4 + 2] = srow[x * 4];
            drow[x * 4 + 3] = srow[x * 4 + 3];
        }
    }
    let Some(format) = register_format(HISTORY_COPY_FORMAT) else { return false };
    let Ok(_clipboard) = Clipboard::new_attempts(10) else { return false };
    raw::set(clipboard_win::formats::CF_DIB, &dib).is_ok()
        && raw::set_without_clear(format.get(), &[1]).is_ok()
}

#[cfg(windows)]
mod win {
    use super::Shared;
    use std::cell::RefCell;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::UI::WindowsAndMessaging::*;

    const WM_CLIPBOARDUPDATE: u32 = 0x031D;

    struct State { shared: Shared, repaint: Box<dyn Fn() + Send> }

    /// 监听线程建好消息窗口后登记线程号；持有者销毁时先置位再读线程号，两边总有一方看到对方。
    #[derive(Default)]
    pub struct Stop { requested: std::sync::atomic::AtomicBool, thread: std::sync::atomic::AtomicU32 }
    impl Stop {
        pub fn request(&self) {
            use std::sync::atomic::Ordering::SeqCst;
            self.requested.store(true, SeqCst);
            let thread = self.thread.load(SeqCst);
            if thread != 0 { unsafe { let _ = PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0)); } }
        }
        fn register(&self) -> bool {
            use std::sync::atomic::Ordering::SeqCst;
            self.thread.store(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() }, SeqCst);
            !self.requested.load(SeqCst)
        }
    }
    thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_CLIPBOARDUPDATE {
            STATE.with(|state| {
                if let Some(state) = state.borrow().as_ref() {
                    if state.shared.is_enabled() {
                        match read_clipboard(hwnd) {
                            Some(Content::Text { text, from_history }) => {
                                if state.shared.record_update(&text, from_history) { (state.repaint)(); }
                            }
                            Some(Content::Image { meta, from_history }) => {
                                // 历史条目写回时也带标记，避免截图→复制→又记一条。
                                if !from_history && state.shared.record_image(meta) { (state.repaint)(); }
                            }
                            None => {}
                        }
                    }
                }
            });
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }

    /// 密码管理器、远程桌面等会给敏感内容加上这些格式，表示不要进入剪贴板历史。
    unsafe fn excluded() -> bool {
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        let ignore = RegisterClipboardFormatW(w!("Clipboard Viewer Ignore"));
        if (exclude != 0 && IsClipboardFormatAvailable(exclude).is_ok()) || (ignore != 0 && IsClipboardFormatAvailable(ignore).is_ok()) {
            return true;
        }
        // 值为 DWORD 0 表示不允许进入历史。
        let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        if history == 0 || IsClipboardFormatAvailable(history).is_err() { return false; }
        let Ok(handle) = GetClipboardData(history) else { return true; };
        let memory = HGLOBAL(handle.0);
        let ptr = GlobalLock(memory) as *const u32;
        if ptr.is_null() { return true; }
        let allowed = GlobalSize(memory) >= 4 && *ptr != 0;
        let _ = GlobalUnlock(memory);
        !allowed
    }

    enum Content {
        Text { text: String, from_history: bool },
        Image { meta: super::ImageMeta, from_history: bool },
    }

    /// 优先读文字；没有文字（或文字超长/被排除）时才尝试图片。
    /// 两者共用一次 OpenClipboard，避免同一帧反复抢剪贴板。
    unsafe fn read_clipboard(hwnd: HWND) -> Option<Content> {
        let has_text = IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_ok();
        let has_dib = IsClipboardFormatAvailable(clipboard_win::formats::CF_DIB).is_ok()
            || IsClipboardFormatAvailable(clipboard_win::formats::CF_DIBV5).is_ok();
        if !has_text && !has_dib { return None; }
        // 其他程序可能正占用剪贴板，稍等重试几次。
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(hwnd).is_ok() { opened = true; break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened { return None; }
        let content = (|| {
            if excluded() { return None; }
            let from_history = clipboard_win::register_format(super::HISTORY_COPY_FORMAT)
                .is_some_and(|format| IsClipboardFormatAvailable(format.get()).is_ok());
            if has_text {
                if let Some(text) = read_text_locked() {
                    return Some(Content::Text { text, from_history });
                }
            }
            // 没文字才认图片：复制带文字说明的图时优先记文字，避免一条变两条。
            let rgba = read_dib_locked()?;
            let meta = super::save_image_png(&rgba.0, rgba.1, rgba.2)?;
            Some(Content::Image { meta, from_history })
        })();
        let _ = CloseClipboard();
        content
    }

    /// 调用者已 OpenClipboard 且确认有 CF_UNICODETEXT。
    unsafe fn read_text_locked() -> Option<String> {
        let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
        let memory = HGLOBAL(handle.0);
        let ptr = GlobalLock(memory) as *const u16;
        if ptr.is_null() { return None; }
        let max = (GlobalSize(memory) / 2).min(super::MAX_CHARS * 2 + 1);
        let mut len = 0;
        while len < max && *ptr.add(len) != 0 { len += 1; }
        let text = (len <= super::MAX_CHARS * 2).then(|| String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)));
        let _ = GlobalUnlock(memory);
        text
    }

    /// 调用者已 OpenClipboard。读 CF_DIB / CF_DIBV5，返回 (RGBA, 宽, 高)。
    /// 只处理无压缩的 32bpp(BGRA) / 24bpp(BGR) —— 截图工具与浏览器复制图片都走这两种。
    unsafe fn read_dib_locked() -> Option<(Vec<u8>, u32, u32)> {
        const CF_DIB: u32 = clipboard_win::formats::CF_DIB;
        let mut format = CF_DIB;
        let mut handle = GetClipboardData(format).ok();
        if handle.is_none() {
            format = clipboard_win::formats::CF_DIBV5;
            handle = GetClipboardData(format).ok();
        }
        let handle = handle?;
        let memory = HGLOBAL(handle.0);
        let ptr = GlobalLock(memory) as *const u8;
        if ptr.is_null() { return None; }
        let total = GlobalSize(memory);
        let out = (|| {
            if total < 40 { return None; }
            let data = std::slice::from_raw_parts(ptr, total);
            let header = 40usize; // BITMAPINFOHEADER；DIBV5 前 40 字节布局相同
            let width = i32::from_le_bytes(data[4..8].try_into().ok()?);
            let height = i32::from_le_bytes(data[8..12].try_into().ok()?);
            let bpp = u16::from_le_bytes(data[14..16].try_into().ok()?);
            let compression = u32::from_le_bytes(data[16..20].try_into().ok()?);
            if width <= 0 || height == 0 || compression != 0 || (bpp != 24 && bpp != 32) { return None; }
            let (w, h) = (width as u32, height.unsigned_abs());
            let stride = ((w as usize * bpp as usize + 31) / 32) * 4;
            let top_down = height < 0;
            let extra = if format == clipboard_win::formats::CF_DIBV5 { 124 - 40 } else { 0 };
            let offset = header + extra;
            let need = offset.checked_add(stride.checked_mul(h as usize)?)?;
            if need > total { return None; }
            let mut rgba = vec![0u8; w as usize * h as usize * 4];
            for row in 0..h as usize {
                let src_row = if top_down { row } else { h as usize - 1 - row };
                let s = &data[offset + src_row * stride..offset + (src_row + 1) * stride];
                let d = &mut rgba[row * w as usize * 4..(row + 1) * w as usize * 4];
                for x in 0..w as usize {
                    let (b, g, r) = if bpp == 32 {
                        (s[x * 4], s[x * 4 + 1], s[x * 4 + 2])
                    } else {
                        (s[x * 3], s[x * 3 + 1], s[x * 3 + 2])
                    };
                    d[x * 4] = r; d[x * 4 + 1] = g; d[x * 4 + 2] = b;
                    d[x * 4 + 3] = if bpp == 32 { s[x * 4 + 3] } else { 255 };
                }
            }
            Some((rgba, w, h))
        })();
        let _ = GlobalUnlock(memory);
        out
    }

    pub fn run(shared: Shared, repaint: Box<dyn Fn() + Send>, stop: &Stop) -> windows::core::Result<()> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("DaozaiClipboardWatcher");
            let wc = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: instance.into(), lpszClassName: class, ..Default::default() };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), class, PCWSTR::null(), WINDOW_STYLE(0), 0, 0, 0, 0, HWND_MESSAGE, None, instance, None)?;
            // 消息窗口建好后线程已有消息队列，此后投递的 WM_QUIT 不会丢。
            if !stop.register() { let _ = DestroyWindow(hwnd); return Ok(()); }
            STATE.with(|state| *state.borrow_mut() = Some(State { shared, repaint }));
            AddClipboardFormatListener(hwnd)?;
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = RemoveClipboardFormatListener(hwnd);
            let _ = DestroyWindow(hwnd);
            STATE.with(|state| *state.borrow_mut() = None);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 与 Shared::flush 同逻辑，但不构造整份 Shared（沿用现有历史对象即可）。
    fn flush_history(history: &mut History) {
        let Some((path, body)) = history.take_dirty() else { return; };
        crate::model::atomic_write(&path, &body).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn dropping_the_watcher_ends_the_listener_thread(){
        // 先停后启和启动后再停两种顺序都必须让线程退出，不能留下常驻的监听窗口。
        for stop_first in [true,false] {
            let stop=Arc::new(win::Stop::default());let thread_stop=stop.clone();
            let shared=Shared{history:Default::default(),enabled:Arc::new(AtomicBool::new(false))};
            if stop_first {stop.request();}
            let (done,finished)=std::sync::mpsc::channel();
            std::thread::spawn(move||{let _=win::run(shared,Box::new(||{}),&thread_stop);let _=done.send(());});
            if !stop_first {std::thread::sleep(std::time::Duration::from_millis(200));stop.request();}
            finished.recv_timeout(std::time::Duration::from_secs(3)).expect("监听线程应在停止后退出");
        }
    }

    #[test]
    fn history_copy_preserves_order_but_external_copy_promotes() {
        let shared = Shared { history: Default::default(), enabled: Arc::new(AtomicBool::new(true)) };
        shared.lock().items.clear();
        shared.record("较早的一条");
        shared.record("最近的一条");
        let before = shared.lock().items.clone();
        assert!(!shared.record_update("较早的一条", true));
        assert_eq!(shared.lock().items, before);
        assert!(shared.record_update("较早的一条", false));
        assert_eq!(shared.lock().items[0].text, "较早的一条");
    }
    #[test]
    fn history_dedupes_caps_and_keeps_pins() {
        let mut h = History::default();
        assert!(!h.push("   \n", "09:00"), "空白不记录");
        assert!(h.push("a", "09:00") && h.push("b", "09:01"));
        assert!(!h.push("b", "09:02"), "与最新一条相同不重复记录");
        assert!(h.push("a", "09:03"));
        assert_eq!(h.items.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(h.items[0].at, "09:03");
        h.items[1].pinned = true; // b
        for i in 0..(MAX_RECENT + 5) { h.push(&format!("n{i}"), "10:00"); }
        assert_eq!(h.items.iter().filter(|c| !c.pinned).count(), MAX_RECENT);
        assert!(h.items.iter().any(|c| c.text == "b" && c.pinned), "固定的不会被挤掉");
        h.clear_recent();
        assert_eq!(h.items.len(), 1);
        assert!(!h.push(&"x".repeat(MAX_CHARS + 1), "10:00"), "超长内容不记录");
    }

    #[test]
    fn reopening_restores_history_and_respects_manual_cleanup() {
        let path = std::env::temp_dir().join(format!("daozai-clip-{}.json", std::process::id()));
        std::fs::write(&path, br#"[{"text":"old","at":"08:00"}]"#).unwrap();
        let mut history = History::load_from(path.clone());
        assert!(history.items[0].pinned);
        history.push("保留", "09:00");
        history.push("删除", "09:01");
        history.remove(0);
        flush_history(&mut history);
        let mut reopened = History::load_from(path.clone());
        assert_eq!(reopened.items, history.items);
        reopened.clear_recent();
        flush_history(&mut reopened);
        let final_history = History::load_from(path.clone());
        assert_eq!(final_history.items.len(), 1);
        assert_eq!(final_history.items[0].text, "old");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn snapshot_version_changes_only_when_content_changes() {
        let shared = Shared { history: Default::default(), enabled: Arc::new(AtomicBool::new(true)) };
        let first = shared.snapshot();
        assert_eq!(shared.snapshot().0, first.0, "重复读取不该让界面重建快照");
        assert!(shared.record("new"));
        let after = shared.snapshot();
        assert!(after.0 > first.0, "内容变化才推进快照版本");
        assert!(!shared.record("new"), "重复内容不产生新记录");
        assert_eq!(shared.snapshot().0, after.0);
    }

    #[test]
    fn old_unpinned_clips_expire_but_pins_survive() {
        let path = std::env::temp_dir().join(format!("daozai-clip-retention-{}.json", std::process::id()));
        let old = now_secs() - RETENTION_SECS - 60;
        std::fs::write(&path, serde_json::to_vec(&serde_json::json!([
            {"text":"过期秘密","at":"08:00","pinned":false,"saved_at":old},
            {"text":"长期模板","at":"08:01","pinned":true,"saved_at":old}
        ])).unwrap()).unwrap();
        let history = History::load_from(path.clone());
        assert_eq!(history.items.len(), 1);
        assert_eq!(history.items[0].text, "长期模板");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pinned_round_trip_and_preview() {
        let mut h = History::default();
        h.push("第一行\n第二行  末尾", "08:00");
        h.push("临时", "08:01");
        h.items[1].pinned = true;
        let back = History::from_json(&h.to_json());
        assert_eq!(back.items, h.items);
        assert_eq!(preview(&back.items[1].text), "第一行 第二行 末尾");
        assert!(History::from_json(b"not json").items.is_empty());
    }

    #[test]
    fn image_clip_round_trips_and_old_json_still_loads() {
        let mut h = History::default();
        h.push("文字", "08:00");
        assert!(h.push_image(ImageMeta { file: "clip-1.png".into(), width: 1920, height: 1080 }, "08:01"));
        assert_eq!(h.items[0].text, "图片 1920×1080");
        assert!(h.items[0].image.is_some());
        assert!(h.items[1].image.is_none(), "文字条目不该被标成图片");
        let back = History::from_json(&h.to_json());
        assert_eq!(back.items, h.items, "图片元数据必须随 JSON 往返");
        // 旧版本没有 image 字段的 JSON 依旧可读，条目退化为纯文字。
        let legacy = r#"[{"text":"旧记录","at":"07:00","pinned":true,"saved_at":1}]"#;
        let old = History::from_json(legacy.as_bytes());
        assert_eq!(old.items.len(), 1);
        assert!(old.items[0].image.is_none());
    }

    #[test]
    fn image_clip_rejects_path_traversal_in_json() {
        let raw = r#"[{"text":"图片 10×10","at":"08:00","image":{"file":"..\\evil.png","width":10,"height":10}}]"#;
        let h = History::from_json(raw.as_bytes());
        assert_eq!(h.items.len(), 1, "条目本身可以载入，但路径必须被丢弃");
        assert!(h.items[0].image.is_none(), "不能信任 JSON 里的文件路径");
    }

    #[test]
    fn image_clip_shares_pin_and_recent_limits() {
        let mut h = History::default();
        h.push_image(ImageMeta { file: "a.png".into(), width: 4, height: 3 }, "09:00");
        h.items[0].pinned = true;
        for i in 0..(MAX_RECENT + 5) {
            h.push_image(ImageMeta { file: format!("n{i}.png"), width: 4, height: 3 }, "09:01");
        }
        assert!(h.items.iter().any(|c| c.pinned && c.image.as_ref().is_some_and(|im| im.file == "a.png")), "固定的图片不能被挤掉");
        assert_eq!(h.items.iter().filter(|c| !c.pinned).count(), MAX_RECENT);
    }
}
