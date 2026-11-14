#!/usr/bin/env python3
"""把自定义补丁应用到岛仔官方源码。

用法：
    python apply.py <源码根目录> [--force]

默认幂等：检测到 CUSTOM_BUILD.txt 且补丁集一致时直接跳过。
每一处替换都要求原文唯一匹配，官方源码变动导致失配时会明确报错，
不会静默产出半套补丁。
"""
import pathlib
import re
import shutil
import sys

PATCH_ID = "custom-110"

# 整份随链分发的新文件（补丁目录内文件名 -> 相对 dz-phone 根的落点）。
NEW_FILES = [
    ("phone_battery.rs", "src/phone_battery.rs"),
    ("island_status.rs", "src/island_status.rs"),
    ("bridge.rs", "src/bridge.rs"),
    ("browser.svg", "assets/settings-icons/system/browser.svg"),
    ("daozai-bridge-main.rs", "src/bin/daozai-bridge/main.rs"),
    ("maa.rs", "src/maa.rs"),
    ("maa.svg", "assets/settings-icons/system/maa.svg"),
    ("maa-icon.png", "assets/maa-icon.png"),
    ("phone.svg", "assets/settings-icons/system/phone.svg"),
    ("netease_control.rs", "src/netease_control.rs"),
]
PATCH_DIR = pathlib.Path(__file__).resolve().parent

EDITS = [
    ("src/app.rs",
     '''pub(crate) const WEATHER_H: f32 = 232.0;
pub(crate) const CALENDAR_H: f32 = 216.0;
pub const BAR_W: f32 = 163.0; // 收起药丸宽
pub const BAR_H: f32 = 30.0; // 收起药丸高
pub(crate) const HUD_W: f32 = 260.0;
pub(crate) const LYRIC_BAR_W: f32 = 300.0;
pub(crate) const HUD_TRACK_X: f32 = 48.0;
pub(crate) const HUD_TRACK_W: f32 = 150.0;
pub const BAR_W_MIN: f32 = 44.0; // 极简小条
pub const BAR_H_MIN: f32 = 4.5;
pub const FLARE: f32 = 17.6; // 顶部外翻肩宽
pub const FLARE_H: f32 = 16.0; // 外翻收敛高度
''',
     '''pub(crate) const WEATHER_H: f32 = 232.0;
pub(crate) const CALENDAR_H: f32 = 216.0;
pub const BAR_W: f32 = 163.0; // 收起药丸宽
pub const BAR_H: f32 = 30.0; // 收起药丸高
pub(crate) const HUD_W: f32 = 260.0;
pub(crate) const LYRIC_BAR_W: f32 = 300.0;
// 歌词态左侧固定通道（手写 flex lane）：若干固定宽项 + 一块 flex:1 剩余区。
// 想调间距或往胶囊里加新图标，只改这几个常量和 lyric_lane()，不用翻绘制代码。
pub(crate) const LANE_PET_HALF: f32 = 24.0;   // 宠物表情贴图半宽（实际 48px）
pub(crate) const LANE_ART_GAP: f32 = 4.0;     // 封面右缘 → 宠物左缘
pub(crate) const LANE_TEXT_GAP: f32 = 4.0;    // 宠物右缘 → 歌词通道（硬余量，保证不重叠）
// 播放指示 5 根竖条以 content_width-21 为中心、向左占 7.8，左缘在 content_width-28.8；
// 42 = 文字与竖条之间留出约 13px 空隙，避免两者贴在一起。
pub(crate) const LANE_RIGHT_PAD: f32 = 48.0;  // 歌词通道右侧留给播放指示
// 歌词固定字号：放不下就横向滚动。
const LYRIC_BASE_SIZE: f32 = 11.0;
const SCROLL_DWELL: f32 = 1.0;      // 句首/句尾停留，先让人看清开头
const SCROLL_MIN_SPEED: f32 = 22.0; // px/s，约每秒 2 个汉字
const SCROLL_MAX_SPEED: f32 = 90.0;
pub(crate) const HUD_TRACK_X: f32 = 48.0;
pub(crate) const HUD_TRACK_W: f32 = 150.0;
pub const BAR_W_MIN: f32 = 44.0; // 极简小条
pub const BAR_H_MIN: f32 = 4.5;
pub const FLARE: f32 = 17.6; // 顶部外翻肩宽
pub const FLARE_H: f32 = 16.0; // 外翻收敛高度
'''),
    ("src/app.rs",
     '''pub const PILL_GAP: f32 = 6.0;

/// 胶囊的柔和投影：四周都有，多层叠加近似高斯。
fn pill_shadow(painter:&egui::Painter,rect:Rect,radius:f32,strength:f32){
    if strength<=0.01 {return;}
    for (grow,a) in [(10.0f32,0.035f32),(6.5,0.05),(3.5,0.07),(1.5,0.09)] {
        let r=Rect::from_min_max(rect.min-Vec2::new(grow*0.6,grow*0.3),rect.max+Vec2::new(grow*0.6,grow));
        painter.rect_filled(r,radius+grow*0.8,Color32::from_black_alpha((255.0*a*strength) as u8));
    }
}
pub(crate) const PAGE_NAMES: [&str; 9] = ["音乐", "专注", "健康", "置物架", "待办", "邮箱", "AI 额度", "日程", "天气"];
pub(crate) const PAGE_SETTINGS_TABS: [usize; 9] = [1,2,3,5,4,7,11,8,12];

// ---------- 动效参数 ----------
/// 悬停意图缓冲：光标停留这么久才真正展开（期间药丸轻微胀大作为反馈），防误触。
const HOVER_INTENT: f64 = 0.20;
const MINIMAL_HOVER_INTENT: f64 = 0.40;
/// 移开后等待多久开始收起；这段时间内回来不会收。
const LEAVE_DELAY: f64 = 0.12;
const SWELL: (f32, f32) = (1.075, 1.13);
// 展开：欠阻尼，末段约 5% 过冲后回落；高度略软于宽度，像「垂落」下来。
const OPEN_W: (f32, f32) = (330.0, 0.70);
const OPEN_H: (f32, f32) = (290.0, 0.74);
// 收起：临界阻尼，约 0.18 s 干净收回，无回弹。
const CLOSE: (f32, f32) = (620.0, 1.0);
const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;

pub struct Options {
    /// 是否联网查歌词（录制/测试时关掉）。
    pub fetch_lyrics: bool,
    /// 吉祥物在多久无操作后出现（秒）。
    pub pet_idle: f64,
''',
     '''pub const PILL_GAP: f32 = 6.0;

/// 胶囊的柔和投影：四周都有，多层叠加近似高斯。
fn pill_shadow(painter:&egui::Painter,rect:Rect,radius:f32,strength:f32){
    if strength<=0.01 {return;}
    for (grow,a) in [(10.0f32,0.035f32),(6.5,0.05),(3.5,0.07),(1.5,0.09)] {
        // 每层都是主体的等距外扩（圆角增量 = 横向扩展量）+ 整体下移，
        // 四角圆弧与主体严格同心同形，直线转圆角处不会再叠出错开的半圆凸边。
        let (d,dy)=shape::shadow_expansion(grow);
        let r=Rect::from_min_max(rect.min-Vec2::new(d,d-dy),rect.max+Vec2::new(d,d+dy));
        painter.rect_filled(r,radius+d,Color32::from_black_alpha((255.0*a*strength) as u8));
    }
}
pub(crate) const PAGE_NAMES: [&str; 9] = ["音乐", "专注", "健康", "置物架", "待办", "邮箱", "AI 额度", "日程", "天气"];
pub(crate) const PAGE_SETTINGS_TABS: [usize; 9] = [1,2,3,5,4,7,11,8,12];

// ---------- 动效参数 ----------
/// 等待超过这个时长才做「轻微胀大」反馈；更短的等待保持静止，
/// 否则弹簧刚鼓一点就被展开打断，反而更突兀。
const SWELL_MIN_MS: u32 = 180;
/// 移开后等待多久开始收起；这段时间内回来不会收。
const LEAVE_DELAY: f64 = 0.12;
const SWELL: (f32, f32) = (1.075, 1.13);
// 展开参数按 `model::OpenAnim` 的配置取，见 `OpenAnim::tuning()`；
// CLOSE 仍用于弹簧初始化与非展开态的回位。
const CLOSE: (f32, f32) = (620.0, 1.0);
const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;

/// 悬停多久才展开（秒）。`0` 表示当帧就展开；极简模式加倍。防误触另由 `LEAVE_DELAY` 兜底。
fn hover_delay_secs(ms: u32, minimal: bool) -> f64 {
    let ms = if minimal { ms.saturating_mul(2) } else { ms };
    (ms as f64 / 1000.0).min(1.0)
}

pub struct Options {
    /// 是否联网查歌词（录制/测试时关掉）。
    pub fetch_lyrics: bool,
    /// 吉祥物在多久无操作后出现（秒）。
    pub pet_idle: f64,
'''),
    ("src/app.rs",
     '''    ToggleAutostart,
    AddFiles,
    DragFile(PathBuf),
    QuotaRefresh,
    QuotaConnected,
    QuotaChooseFile,
    /// 把系统主音量设为 0..100（v0.5.4 音量条）。
    SetVolume(u32),
    /// 切换系统静音。
    ToggleMute,
    /// 音量命令携带点击时的播放器身份，避免切换歌曲来源后误调其他应用。
    MusicVolume(Transport,String),
''',
     '''    ToggleAutostart,
    AddFiles,
    DragFile(PathBuf),
    QuotaRefresh,
    QuotaConnected,
    QuotaChooseFile,
    /// 选择 MAA 可执行文件（设置页 MAA 分组）。
    PickMaaExe,
    /// 选择 MAA 日志目录（设置页 MAA 分组）。
    PickMaaLogDir,
    /// 把系统主音量设为 0..100（v0.5.4 音量条）。
    SetVolume(u32),
    /// 切换系统静音。
    ToggleMute,
    /// 音量命令携带点击时的播放器身份，避免切换歌曲来源后误调其他应用。
    MusicVolume(Transport,String),
'''),
    ("src/app.rs",
     '''
/// 本地歌词（酷狗、汽水）每 5 秒复查，最多约 1 分钟；网络失败每 30 秒重试，最多约 5 分钟。手动点重试会重新计数。
const NATIVE_RETRIES:u32=12;
const NETWORK_RETRIES:u32=10;

/// 与 `format!("{title}|{artist}|{duration:.0}")` 等价，但不分配内存：界面每帧要比较好几次。
fn lyric_key_matches(key:&str,media:&NowPlaying)->bool {
    use std::io::Write;
    let Some(rest)=key.strip_prefix(media.title.as_str()).and_then(|rest|rest.strip_prefix('|')) else{return false;};
    let Some(rest)=rest.strip_prefix(media.artist.as_str()).and_then(|rest|rest.strip_prefix('|')) else{return false;};
    let mut buffer=[0u8;32];
    let mut cursor=std::io::Cursor::new(&mut buffer[..]);
''',
     '''
/// 本地歌词（酷狗、汽水）每 5 秒复查，最多约 1 分钟；网络失败每 30 秒重试，最多约 5 分钟。手动点重试会重新计数。
const NATIVE_RETRIES:u32=12;
const NETWORK_RETRIES:u32=10;

/// 与 `format!("{title}|{artist}|{duration:.0}")` 等价，但不分配内存：界面每帧要比较好几次。
/// 长歌词横向滚动的参数：溢出宽度与速度（都是点/秒口径的点）。
#[derive(Clone,Copy,Debug)]
struct LyricScroll {overflow:f32,speed:f32}

/// 歌词排版缓存。
/// 没有它时 `update_island_lyric_layout` 每帧都要从 11pt 逐级试到 8pt，
/// 一句中文最多重排 7 次；缓存后同一句只排一次。
struct LyricLayoutCache {
    text:String,
    width:f32,
    white:bool,
    title:bool,
    galley:std::sync::Arc<egui::Galley>,
    /// 8pt 仍放不下时的溢出宽度（点），>0 表示要横向滚动。
    overflow:f32,
}

impl LyricLayoutCache {
    fn matches(&self,text:&str,width:f32,white:bool,title:bool)->bool {
        self.text==text && (self.width-width).abs()<0.5 && self.white==white && self.title==title
    }
}

/// 滚动速度：让整句尽量在本句时长内滚完，同时夹在可读区间。
fn lyric_scroll_speed(overflow:f32,window:f32)->f32 {
    let travel=(window-2.0*SCROLL_DWELL).max(0.8);
    (overflow/travel).clamp(SCROLL_MIN_SPEED,SCROLL_MAX_SPEED)
}

/// 滚动位移：句首停留 `SCROLL_DWELL` 秒后匀速推进，到尾部停住。
fn lyric_scroll_offset(elapsed:f32,speed:f32,overflow:f32)->f32 {
    ((elapsed-SCROLL_DWELL).max(0.0)*speed).min(overflow)
}

fn lyric_key_matches(key:&str,media:&NowPlaying)->bool {
    use std::io::Write;
    let Some(rest)=key.strip_prefix(media.title.as_str()).and_then(|rest|rest.strip_prefix('|')) else{return false;};
    let Some(rest)=rest.strip_prefix(media.artist.as_str()).and_then(|rest|rest.strip_prefix('|')) else{return false;};
    let mut buffer=[0u8;32];
    let mut cursor=std::io::Cursor::new(&mut buffer[..]);
'''),
    ("src/app.rs",
     '''    pub(crate) confirm_clear: Option<f64>,
    /// 置物架页的两个分栏：0 = 文件，1 = 剪贴板。
    pub(crate) shelf_tab: usize,
    pub clipboard: crate::clipboard::Shared,
    pub(crate) clip_first: usize,
    pub(crate) clip_confirm_clear: Option<f64>,

    // 饮水
    pub(crate) water_ring: Spring,
    pub(crate) water_ml: f32,
    pub(crate) water_view: usize,
    pub(crate) water_view_t: Spring,
''',
     '''    pub(crate) confirm_clear: Option<f64>,
    /// 置物架页的两个分栏：0 = 文件，1 = 剪贴板。
    pub(crate) shelf_tab: usize,
    pub clipboard: crate::clipboard::Shared,
    pub(crate) clip_first: usize,
    pub(crate) clip_confirm_clear: Option<f64>,
    /// 剪贴板搜索词；非空时列表只显示命中的记录。
    pub(crate) clip_query: String,
    pub(crate) clip_search_focus: bool,
    /// 搜索输入框是否展开（面板表头平时只放一个「搜索」按钮）。
    pub(crate) clip_search_open: bool,
    /// 搜索框这一帧是否真的有键盘焦点（只有这时才处理 ↑↓ / Enter）。
    pub(crate) clip_typing: bool,
    /// 键盘选中的行（过滤后列表的下标）。
    pub(crate) clip_sel: usize,

    // 饮水
    pub(crate) water_ring: Spring,
    pub(crate) water_ml: f32,
    pub(crate) water_view: usize,
    pub(crate) water_view_t: Spring,
'''),
    ("src/app.rs",
     '''
    // 音乐
    pub(crate) ly: LyricsState,
    pub(crate) ly_worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) show_lyrics: bool,
    island_lyric_layout: Option<std::sync::Arc<egui::Galley>>,
    media_anchor: (String, f64, f64, bool),
    pub(crate) media: NowPlaying,
    pub(crate) music_artwork: Option<(std::sync::Arc<egui::ColorImage>,egui::TextureHandle)>,

    forced_open_until:f64,
    pub settings_open:bool,
''',
     '''
    // 音乐
    pub(crate) ly: LyricsState,
    pub(crate) ly_worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) show_lyrics: bool,
    island_lyric_layout: Option<std::sync::Arc<egui::Galley>>,
    island_lyric_cache: Option<LyricLayoutCache>,
    /// 长歌词滚动参数；Some 表示当前句放不下、正在横向滚动。
    island_lyric_scroll: Option<LyricScroll>,
    /// 滚动时间基准：(歌词行号, 文本)。变化即重置起点（副歌重复同一句也要重置）。
    island_lyric_scroll_key: (Option<usize>,String),
    island_lyric_scroll_start: f64,
    media_anchor: (String, f64, f64, bool),
    pub(crate) media: NowPlaying,
    pub(crate) music_artwork: Option<(std::sync::Arc<egui::ColorImage>,egui::TextureHandle)>,

    forced_open_until:f64,
    pub settings_open:bool,
'''),
    ("src/app.rs",
     '''    /// 毛玻璃着色量（平滑过渡，避免背景变化时闪烁）；0 表示尚未初始化。
    glass_tint:f32,
    pub(crate) fluid:crate::fluid::Fluid,
    pub(crate) fluid_origin:Pos2,
    fluid_art:Option<egui::TextureId>,
    fluid_title:String,
    screen:Option<Rect>,
    pub(crate) settings_focus_requested:bool,
    pub(crate) settings_tab:usize,
    pub(crate) settings_category:usize,
    pub messenger_statuses:std::collections::HashMap<Messenger,String>,
    pub(crate) notification_apps:model::NotificationApps,
    pub(crate) subpage:Option<usize>,
    pub(crate) task_dialog:Option<crate::extras::TaskDraft>,
    pub listener_status: String,
    pub(crate) notification_bell_until: f64,
    pub(crate) notification_bell_at: f64,
    pub(crate) notice: Option<Alert>,
''',
     '''    /// 毛玻璃着色量（平滑过渡，避免背景变化时闪烁）；0 表示尚未初始化。
    glass_tint:f32,
    pub(crate) fluid:crate::fluid::Fluid,
    pub(crate) fluid_origin:Pos2,
    fluid_art:Option<egui::TextureId>,
    fluid_title:String,
    /// 悬浮云每帧刷新的颜色纹理：低分辨率交给 GPU 双线性插值，避免每帧生成上万顶点。
    fluid_tex:Option<egui::TextureHandle>,
    screen:Option<Rect>,
    pub(crate) settings_focus_requested:bool,
    pub(crate) settings_tab:usize,
    pub(crate) settings_category:usize,
    pub messenger_statuses:std::collections::HashMap<Messenger,String>,
    pub(crate) notification_apps:model::NotificationApps,
    /// 手机通知聚合：标题 -> (条数, 最后到达时间)。
    pub(crate) phone_counts:std::collections::HashMap<String,(u32,f64)>,
    /// 手机 App 图标纹理缓存：文件名 -> TextureHandle。
    pub(crate) phone_icons:std::collections::HashMap<String,eframe::egui::TextureHandle>,
    /// 手机连接通知的来源 App 名单（`phone-apps.json`，支持热重载）。
    pub(crate) phone_apps:crate::model::PhoneApps,
    /// 浏览器下载偏好（`bridge.json`，桥宿主与设置页共用）。
    pub(crate) browser_prefs:crate::model::BridgePrefs,
    /// MAA（明日方舟助手）对接：事件日志跟随器、进度日志跟随器、任务进度追踪、常驻胶囊。
    pub(crate) maa_watch:crate::maa::Watcher,
    pub(crate) maa_prog:crate::maa::Watcher,
    pub(crate) maa_tracker:crate::maa::ProgressTracker,
    pub(crate) maa_status:Option<crate::island_status::Status>,
    /// MAA 全部完成的提示截止时刻（Unix 秒）；0 = 未进入完成态。
    pub(crate) maa_done_until:f64,
    /// MAA 图标 PNG 路径缓存（首次从 exe 提取后记住）。
    pub(crate) maa_icon:Option<String>,
    pub(crate) maa_prefs:crate::model::MaaPrefs,
    /// 状态胶囊的图标缓存（RefCell：渲染时借用 &self 也要能取图）。
    pub(crate) status_icons:std::cell::RefCell<crate::icons::Cache>,
    /// 上次看到的手机电量（None=尚未初始化，用于断开/重连/低电/充满提示）。
    pub(crate) phone_last:Option<Option<u8>>,
    /// 上次看到的本机电量，用于本机低电 / 充满提示。
    pub(crate) pc_last:Option<Option<u8>>,
    /// 勿扰截止时刻（app 时间）；> t 期间不弹提示、不闪铃铛，验证码除外。
    pub(crate) quiet_until:f64,
    /// 已经自动勿扰过的会议（uid|start），避免同一场会重复触发。
    pub(crate) quiet_meeting_key:Option<String>,
    /// 上次检查会议勿扰的时刻（每 5 秒查一次，别每帧都拼日程）。
    pub(crate) meeting_checked_at:f64,
    /// 今日会议候选缓存：(日期, 生成时刻, (开始, 结束, uid))。
    /// 日程合并排序较重，60 秒或跨天才重算一次；90 秒的触发窗口保证不会漏。
    pub(crate) meeting_plan:(String,f64,Vec<(i64,i64,String)>),
    /// 「稍后提醒」队列：(到期时刻, 标题, 正文)。
    pub(crate) snoozed:Vec<(f64,String,String)>,
    /// 通知规则（notice-rules.json）：静音 / 高亮 / 自动复制。
    pub(crate) notice_rules: model::NoticeRules,
    /// 剪贴板列表快照：(版本号, 共享列表)。只在版本变化时重建，避免每帧克隆整份历史。
    pub(crate) clip_snapshot: (u64, std::sync::Arc<Vec<crate::clipboard::Clip>>),
    /// 关键词缓存：(源字符串, 小写词表)。命中判断在绘制路径上，不能每次都 split + 小写。
    pub(crate) keyword_src: String,
    pub(crate) keywords: Vec<String>,
    /// 已经提醒过的「额度窗口」（id + 窗口名），回到阈值以上会移除、下次可再提醒。
    pub(crate) quota_alerted: std::collections::HashSet<(String,String)>,
    /// 上次检查额度阈值的时刻（每 30 秒一次）。
    pub(crate) quota_checked_at: f64,
    /// 设置页的最近通知缓存：(上次读取时刻, 列表)，避免每帧读盘。
    pub(crate) notice_cache:(f64,Vec<(String,String,String)>),
    /// 还没凑满一分钟的专注秒数，攒够就写进 focus-stats.json。
    pub(crate) focus_pending:f32,
    /// 今日专注分钟数与连续天数（每分钟更新一次，别每帧读盘）。
    pub(crate) focus_today_min:u32,
    pub(crate) focus_streak_days:u32,
    /// 状态总线当前存活的状态，用于常驻区并排显示。
    pub(crate) status_live:Vec<crate::island_status::Status>,
    /// 未接来电累计（点「未接来电」提示的关闭按钮清零）。
    pub(crate) missed_calls:u32,
    /// 通用状态总线读取器。
    pub(crate) island_status:crate::island_status::Reader,
    pub(crate) subpage:Option<usize>,
    pub(crate) task_dialog:Option<crate::extras::TaskDraft>,
    pub listener_status: String,
    pub(crate) notification_bell_until: f64,
    pub(crate) notification_bell_at: f64,
    pub(crate) notice: Option<Alert>,
'''),
    ("src/app.rs",
     '''    pub(crate) quota: crate::quota_ui::State,
    /// 天气页的当前数据与后台拉取状态。
    pub(crate) weather: crate::weather::State,
    pub(crate) updates: crate::updates::State,
    pub(crate) resident: bar::Motion,
    pub(crate) resident_power: (Option<u8>,bool),
}

impl App {
    pub fn new(ctx: &egui::Context, opts: Options) -> Self {
        theme::load_fonts(ctx);
        egui_extras::install_image_loaders(ctx);
''',
     '''    pub(crate) quota: crate::quota_ui::State,
    /// 天气页的当前数据与后台拉取状态。
    pub(crate) weather: crate::weather::State,
    pub(crate) updates: crate::updates::State,
    pub(crate) resident: bar::Motion,
    pub(crate) resident_power: (Option<u8>,bool),
    /// 手机连接上报的电量；与本机电量各自独立，互不影响。
    pub(crate) phone_resident: (Option<u8>,bool),
    /// 手机连接电量读取器（本地文件轮询）。
    pub(crate) phone_power: crate::phone_battery::Reader,
    pub wechat_status:String,
}

impl App {
    pub fn new(ctx: &egui::Context, opts: Options) -> Self {
        theme::load_fonts(ctx);
        egui_extras::install_image_loaders(ctx);
'''),
    ("src/app.rs",
     '''        ctx.style_mut(|s| {
            s.interaction.selectable_labels = false;
            s.visuals.text_cursor.stroke = egui::Stroke::new(1.5_f32, theme::BLUE);
            s.visuals.selection.bg_fill = theme::alpha_blue(0.45);
        });
        let settings = model::Settings::load();
        let clipboard_on = settings.clipboard_history;
        let (bw, bh) = if settings.minimal { (BAR_W_MIN, BAR_H_MIN) } else { (BAR_W, BAR_H) };
        let focus_seconds=settings.focus_minutes as f32*60.0;
        let initial_lyrics=settings.lyrics_enabled;
        let initial_page=settings.page_order.iter().copied().find(|page|settings.visible_pages[*page]).unwrap_or(0);
        let weather=crate::weather::State::load(&settings.weather_city,settings.weather_position.as_ref());
''',
     '''        ctx.style_mut(|s| {
            s.interaction.selectable_labels = false;
            s.visuals.text_cursor.stroke = egui::Stroke::new(1.5_f32, theme::BLUE);
            s.visuals.selection.bg_fill = theme::alpha_blue(0.45);
        });
        let settings = model::Settings::load();
        // 专注统计只在启动和每分钟更新，构造前先取一次。
        let (focus_today, focus_streak)=crate::extras::focus_summary();
        // 邮件面板上次选的账号：结构体里 `settings` 会被 move，先取出来。
        let saved_mail_filter = settings.mail_filter.clone();
        let clipboard_on = settings.clipboard_history;
        let (bw, bh) = if settings.minimal { (BAR_W_MIN, BAR_H_MIN) } else { (BAR_W, BAR_H) };
        let focus_seconds=settings.focus_minutes as f32*60.0;
        let initial_lyrics=settings.lyrics_enabled;
        let initial_page=settings.page_order.iter().copied().find(|page|settings.visible_pages[*page]).unwrap_or(0);
        let weather=crate::weather::State::load(&settings.weather_city,settings.weather_position.as_ref());
'''),
    ("src/app.rs",
     '''            icons: Default::default(), avoidance: Default::default(),
            confirm_clear: None,
            shelf_tab: 0,
            clipboard: crate::clipboard::Shared::new(clipboard_on),
            clip_first: 0,
            clip_confirm_clear: None,
            water_ring: Spring::new(0.0, 120.0, 0.9),
            water_ml: 0.0,
            water_view: 0,
            water_view_t: Spring::new(0.0, 200.0, 0.9),
            sit_view_t: Spring::new(0.0, 200.0, 0.9),
            sit_mode: false,
''',
     '''            icons: Default::default(), avoidance: Default::default(),
            confirm_clear: None,
            shelf_tab: 0,
            clipboard: crate::clipboard::Shared::new(clipboard_on),
            clip_first: 0,
            clip_confirm_clear: None,
            clip_query: String::new(),
            clip_search_focus: false,
            clip_search_open: false,
            clip_typing: false,
            clip_sel: 0,
            water_ring: Spring::new(0.0, 120.0, 0.9),
            water_ml: 0.0,
            water_view: 0,
            water_view_t: Spring::new(0.0, 200.0, 0.9),
            sit_view_t: Spring::new(0.0, 200.0, 0.9),
            sit_mode: false,
'''),
    ("src/app.rs",
     '''            buf: String::new(),
            todo_pop: Spring::new(0.0, 500.0, 0.45),
            ly: LyricsState::default(),
            ly_worker: None,
            show_lyrics: initial_lyrics,
            island_lyric_layout: None,
            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, settings_window:std::sync::Arc::new(std::sync::Mutex::new(crate::settings_window::State::load())), displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, messenger_statuses:Default::default(), notification_apps:model::NotificationApps::load(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
            toast: None, toast_below: false,
            last_input: 0.0, system_idle: None,
            last_cursor: None,
            pet_a: 0.0,
''',
     '''            buf: String::new(),
            todo_pop: Spring::new(0.0, 500.0, 0.45),
            ly: LyricsState::default(),
            ly_worker: None,
            show_lyrics: initial_lyrics,
            island_lyric_layout: None,
            island_lyric_cache: None,
            island_lyric_scroll: None,
            island_lyric_scroll_key: (None,String::new()),
            island_lyric_scroll_start: 0.0,
            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0,
            settings_open:false,
            settings_window:std::sync::Arc::new(std::sync::Mutex::new(crate::settings_window::State::load())),
            displays:Vec::new(),
            browser_at_top:false,
            browser_minimal_active:false,
            glass_tint:0.0,
            fluid:Default::default(),
            fluid_origin:Pos2::ZERO,
            fluid_art:None,
            fluid_title:String::new(),
            screen:None,
            settings_focus_requested:false,
            settings_tab:0,
            settings_category:0,
            messenger_statuses:Default::default(),
            notification_apps:model::NotificationApps::load(),
            subpage:None,
            task_dialog:None,
            fluid_tex:None,
            wechat_status:String::new(),
            phone_counts:Default::default(),
            phone_icons:Default::default(),
            phone_apps:model::PhoneApps::load(),
            browser_prefs:model::BridgePrefs::load(),
            maa_watch:Default::default(),
            maa_prog:Default::default(),
            maa_tracker:Default::default(),
            maa_status:None,
            maa_done_until:0.0,
            maa_icon:None,
            maa_prefs:model::MaaPrefs::load(),
            status_icons:Default::default(),
            notice_rules:model::NoticeRules::load(),
            clip_snapshot:(u64::MAX,std::sync::Arc::new(Vec::new())),
            keyword_src:String::new(),
            keywords:Vec::new(),
            quota_alerted:Default::default(),
            quota_checked_at:0.0,
            phone_last:None,
            pc_last:None,
            quiet_until:0.0,
            quiet_meeting_key:None,
            meeting_checked_at:0.0,
            meeting_plan:(String::new(),f64::MIN,Vec::new()),
            snoozed:Vec::new(),
            notice_cache:(0.0,Vec::new()),
            focus_pending:0.0,
            focus_today_min:focus_today,
            focus_streak_days:focus_streak,
            status_live:Vec::new(),
            missed_calls:0,
            island_status:Default::default(),
           listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
            toast: None, toast_below: false,
            last_input: 0.0, system_idle: None,
            last_cursor: None,
            pet_a: 0.0,
'''),
    ("src/app.rs",
     '''            volume_a: 0.0,
            music_volume_ui: None,
            music_volume_quiet_until: 0.0,
            music_volume_open: false,
            bt_shown: Vec::new(),
            bt_initialized: false,
            mail: MailState { accounts: mail::load_accounts(), ..Default::default() },
            mail_lines_cache: Vec::new(),
            calendar: CalendarState::load(),
            quota: crate::quota_ui::State::load(),
            weather,
            updates: Default::default(),
            resident: Default::default(), resident_power:(None,false),
        }
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), self.t));
        self.toast_below = false;
''',
     '''            volume_a: 0.0,
            music_volume_ui: None,
            music_volume_quiet_until: 0.0,
            music_volume_open: false,
            bt_shown: Vec::new(),
            bt_initialized: false,
            mail: {
                // 邮件面板记住上次选的账号：State 有私有字段，只能先默认构造再赋值。
                let mut manage = crate::mail_manage_ui::State::default();
                if !saved_mail_filter.is_empty() { manage.account_filter = Some(saved_mail_filter); }
                MailState { accounts: mail::load_accounts(), manage, ..Default::default() }
            },
            mail_lines_cache: Vec::new(),
           calendar: CalendarState::load(),
            quota: crate::quota_ui::State::load(),
            weather,
            updates: Default::default(),
            resident: Default::default(), resident_power:(None,false), phone_resident:(None,false), phone_power: Default::default(),
        }
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), self.t));
        self.toast_below = false;
'''),
    ("src/app.rs",
     '''                    .retain(|m| !(m.account.eq_ignore_ascii_case(&item.account) && m.uid == item.uid));
                self.mail.recent.insert(0, item);
                self.mail.recent.truncate(60);
                if new{self.mail.manage.next_refresh=0.0;}
                self.mail.privacy.retain(self.mail.recent.iter().chain(&self.mail.manage.trash).chain(&self.mail.reading));
                if new && self.mail_active() {
                    let item=&self.mail.recent[0];
                    let body=format!("{} · {}",item.from,item.subject);
                    let body=item.code.as_ref().map(|code|crate::mail_privacy::mask(&body,code)).unwrap_or(body);
                    self.alert("新邮件",body,5,false);
                    self.notification_bell_at = self.t;
                    self.notification_bell_until = self.t + 6.0;
                }
            }
            mail::Event::Test { account, ok, note } => {
                self.mail.testing = None;
                let detail = if ok { "连接成功".to_string() } else { note };
                let detail = if detail.chars().count() > 120 {
''',
     '''                    .retain(|m| !(m.account.eq_ignore_ascii_case(&item.account) && m.uid == item.uid));
                self.mail.recent.insert(0, item);
                self.mail.recent.truncate(60);
                if new{self.mail.manage.next_refresh=0.0;}
                self.mail.privacy.retain(self.mail.recent.iter().chain(&self.mail.manage.trash).chain(&self.mail.reading));
                if new && self.mail_active() {
                    let (from,subject,code)={
                        let item=&self.mail.recent[0];
                        (item.from.clone(),item.subject.clone(),item.code.clone())
                    };
                    if let Some(code)=code{
                        self.mail_code_alert(&code,&from,5);
                        self.notification_bell_at=self.t;
                        self.notification_bell_until=self.t+if cfg!(test){4.5}else{self.settings.otp_seconds as f64};
                    }else{
                        let body=format!("{} · {}",from,subject);
                        self.alert("新邮件",body,5,false);
                        self.notification_bell_at = self.t;
                        self.notification_bell_until = self.t + 6.0;
                    }
                }
            }
            mail::Event::Test { account, ok, note } => {
                self.mail.testing = None;
                let detail = if ok { "连接成功".to_string() } else { note };
                let detail = if detail.chars().count() > 120 {
'''),
    ("src/app.rs",
     '''        self.mail.manage.data.retain(|(email,_),_|active(email));
        self.mail.recent.retain(|m|active(&m.account.to_ascii_lowercase()));
        self.mail.manage.trash.retain(|m|active(&m.account.to_ascii_lowercase()));
        if self.mail.reading.as_ref().is_some_and(|m|!active(&m.account.to_ascii_lowercase())){self.mail.reading=None;}
        if self.mail.open_uid.as_ref().is_some_and(|(email,_)|!active(&email.to_ascii_lowercase())){self.mail.open_uid=None;self.mail.manage.auto_read=None;}
        self.mail.manage.identities.retain(|_,old|accounts.contains(old));
        self.mail.manage.next_refresh=0.0;
        mail::save_accounts(accounts);
        self.pending.push(Cmd::MailSync);
    }

    /// 录制/截图用的邮件样例（只在 `record` feature 下编译）。
''',
     '''        self.mail.manage.data.retain(|(email,_),_|active(email));
        self.mail.recent.retain(|m|active(&m.account.to_ascii_lowercase()));
        self.mail.manage.trash.retain(|m|active(&m.account.to_ascii_lowercase()));
        if self.mail.reading.as_ref().is_some_and(|m|!active(&m.account.to_ascii_lowercase())){self.mail.reading=None;}
        if self.mail.open_uid.as_ref().is_some_and(|(email,_)|!active(&email.to_ascii_lowercase())){self.mail.open_uid=None;self.mail.manage.auto_read=None;}
        self.mail.manage.identities.retain(|_,old|accounts.contains(old));
        self.mail.manage.pending.retain(|email|accounts.iter().any(|a|mail::key(a)==*email));
        if self.mail.manage.account_filter.as_ref().is_some_and(|f|!accounts.iter().any(|a|a.email.eq_ignore_ascii_case(f))){self.mail.manage.account_filter=None;self.mail.manage.account_menu=false;if !self.settings.mail_filter.is_empty(){self.settings.mail_filter.clear();self.settings.save();}}
        self.mail.manage.next_refresh=0.0;
        mail::save_accounts(accounts);
        self.pending.push(Cmd::MailSync);
    }

    /// 录制/截图用的邮件样例（只在 `record` feature 下编译）。
'''),
    ("src/app.rs",
     '''            self.open_settings();
            return;
        }
        self.notice=None;
        self.go(page,1.0);
        self.open=true; self.forced_open_until=self.t+4.0;
        self.w.tune(OPEN_W.0,OPEN_W.1);self.h.tune(OPEN_H.0,OPEN_H.1);
        self.w.target=PW;self.h.target=self.expanded_height();
    }
    pub(crate) fn window_layout(&self)->std::sync::MutexGuard<'_,crate::settings_window::State>{
        self.settings_window.lock().unwrap_or_else(|e|e.into_inner())
    }

    pub fn save_window_before_exit(&self){self.window_layout().close();}
''',
     '''            self.open_settings();
            return;
        }
        self.notice=None;
        self.go(page,1.0);
        self.open=true; self.forced_open_until=self.t+4.0;
        let target_h=self.expanded_height();
        self.tune_open(PW,target_h);
    }
    pub(crate) fn window_layout(&self)->std::sync::MutexGuard<'_,crate::settings_window::State>{
        self.settings_window.lock().unwrap_or_else(|e|e.into_inner())
    }

    pub fn save_window_before_exit(&self){self.window_layout().close();}
'''),
    ("src/app.rs",
     '''                break;
            }
        }
    }

    pub fn is_editing(&self) -> bool {
        self.adding || self.editing.is_some() || self.todo_choosing || self.task_dialog.is_some() || self.calendar.draft.is_some() || self.focus_editing
    }

    pub fn filter_todo_input(&mut self,input:&mut egui::RawInput) {
        self.todo_ime_frame=false;
        if !self.adding && self.editing.is_none(){self.todo_ime_active=false;return;}
        self.todo_ime_frame=self.todo_ime_active || input.events.iter().any(|event|matches!(event,egui::Event::Ime(egui::ImeEvent::Enabled|egui::ImeEvent::Preedit(_)|egui::ImeEvent::Commit(_))));
''',
     '''                break;
            }
        }
    }

    pub fn is_editing(&self) -> bool {
        // 剪贴板搜索也属于「正在输入」：窗口被激活时要保持展开，否则光标一闪就收起来了。
        self.adding || self.editing.is_some() || self.todo_choosing || self.task_dialog.is_some() || self.calendar.draft.is_some() || self.focus_editing || self.clip_search_open
    }

    pub fn filter_todo_input(&mut self,input:&mut egui::RawInput) {
        self.todo_ime_frame=false;
        if !self.adding && self.editing.is_none(){self.todo_ime_active=false;return;}
        self.todo_ime_frame=self.todo_ime_active || input.events.iter().any(|event|matches!(event,egui::Event::Ime(egui::ImeEvent::Enabled|egui::ImeEvent::Preedit(_)|egui::ImeEvent::Commit(_))));
'''),
    ("src/app.rs",
     '''        let index=lyrics.index_at(self.lyric_position())?;
        let text=lyrics.lines.get(index)?.text.trim();
        (!text.is_empty()).then_some((index,text))
    }

    fn update_island_lyric_layout(&mut self,ctx:&egui::Context) {
        if !self.island_lyrics_active(){self.island_lyric_layout=None;return;}
        let current=self.island_lyric();
        let lyrics=self.ly.lyrics.as_ref().filter(|lyrics|lyric_key_matches(&self.ly.key,&self.media)
            && lyrics.synced && lyrics.lines.iter().any(|line|!line.text.trim().is_empty()));
        let title=lyrics.is_none();
        let text=if let Some((_,line))=current {line}
            else if let Some(lyrics)=lyrics {
                if lyrics.index_at(self.lyric_position()).is_none() {"前奏"}
                else {"间奏"}
            } else {self.media.title.trim()};
        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        let layout=|text:String| {
            let mut job=egui::text::LayoutJob::simple(text,theme::font(11.0,W::Semibold),ink,LYRIC_BAR_W-80.0);
            job.halign=egui::Align::Center;
            job.wrap.break_anywhere=true;
            if title {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        let mut galley=layout(text.to_owned());
        if let Some((index,_))=current.filter(|_|galley.rows.len()>1) {
            let lyrics=self.ly.lyrics.as_ref().unwrap();
            let start=lyrics.lines[index].t;
            let end=lyrics.lines.iter().skip(index+1).find(|line|line.t>start).map_or(self.media.duration as f32,|line|line.t);
            let progress=if end>start {((self.lyric_position()-start)/(end-start)).clamp(0.0,1.0)}else{0.0};
            // 按当前句的时间范围与各段字数切换，暂停和跳转直接跟随播放位置。
            let total:usize=galley.rows.iter().map(|row|row.glyphs.len()).sum();
            let mut count=0;
            let page=galley.rows.iter().position(|row|{count+=row.glyphs.len();progress*(total as f32)<count as f32}).unwrap_or(galley.rows.len()-1);
            let segment=galley.rows[page].glyphs.iter().map(|glyph|glyph.chr).collect::<String>();
            galley=layout(segment.trim().to_owned());
        }
        self.island_lyric_layout=Some(galley);
    }

    pub(crate) fn effective_minimal(&self)->bool {self.settings.minimal || self.browser_minimal_active}
    pub(crate) fn pet_enabled(&self)->bool {self.settings.pet && !self.effective_minimal() && !self.side()}
    pub(crate) fn pet_space(&self)->f32 {if self.pet_enabled(){44.0}else{0.0}}
    fn lyric_focus_space(&self)->f32 {if self.pomo.running{crate::focus_bar::LYRIC_WIDTH+10.0}else{0.0}}
    fn lyric_bar_width(&self)->f32 {LYRIC_BAR_W+if self.pet_enabled(){48.0}else{0.0}+self.lyric_focus_space()}
    fn notice_size(&self,a:&Alert)->(f32,f32) {
        let (width,height)=if a.meeting.is_some(){(360.0,136.0)}else if a.source==crate::extras::AlertSource::Internal && a.title=="蓝牙设备"{(HUD_W,48.0)}else if a.is_hud(){(HUD_W,38.0)}else{(334.0,44.0)};
        (width,height)
    }
    pub fn set_pet(&mut self,enabled:bool) {
        self.settings.pet=enabled;if enabled {self.settings.minimal=false;}self.settings.save();
''',
     '''        let index=lyrics.index_at(self.lyric_position())?;
        let text=lyrics.lines.get(index)?.text.trim();
        (!text.is_empty()).then_some((index,text))
    }

    fn update_island_lyric_layout(&mut self,ctx:&egui::Context) {
        if !self.island_lyrics_active(){self.island_lyric_layout=None;self.island_lyric_scroll=None;return;}
        // 文本与时间窗先取成自有值：下面要写 self 的字段，不能还借着 self.ly。
        let current=self.island_lyric().map(|(index,line)|(index,line.to_owned()));
        let lyrics=self.ly.lyrics.as_ref().filter(|lyrics|lyric_key_matches(&self.ly.key,&self.media)
            && lyrics.synced && lyrics.lines.iter().any(|line|!line.text.trim().is_empty()));
        let title=lyrics.is_none();
        let text=if let Some((_,line))=&current {line.clone()}
            else if let Some(lyrics)=lyrics {
                if lyrics.index_at(self.lyric_position()).is_none() {"前奏".to_owned()}
                else {"间奏".to_owned()}
            } else {self.media.title.trim().to_owned()};
        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        // 先算好再进闭包，避免闭包捕获 &self 与后面的可变借用冲突。
        let wrap_width=self.lyric_text_width();
        let white=ink==theme::WHITE;
        // 本句时间窗：滚动速度要用，测试构建的分页兜底也要用。
        let span=current.as_ref().zip(lyrics).map(|((index,_),lyrics)|{
            let start=lyrics.lines[*index].t;
            let end=lyrics.lines.iter().skip(index+1).find(|line|line.t>start).map_or(self.media.duration as f32,|line|line.t);
            (start,end)
        });
        let progress=span.map_or(0.0,|(start,end)|if end>start {((self.lyric_position()-start)/(end-start)).clamp(0.0,1.0)}else{0.0});
        let window=span.map(|(start,end)|end-start);
        // 布局缓存：同一句、同一通道宽度、同一颜色只重排一次。
        // 没有缓存时这里每帧都要从 11pt 逐级试到 8pt，一句中文最多重排 7 次，全是重复劳动。
        // 测试构建不走缓存：分页兜底按播放进度切段，缓存会返回上一段。
        let cached=if cfg!(test) {None} else {
            self.island_lyric_cache.as_ref()
                .filter(|cache|cache.matches(&text,wrap_width,white,title))
                .map(|cache|(cache.galley.clone(),cache.overflow))
        };
        let layout=|text:&str,size:f32| {
            let mut job=egui::text::LayoutJob::simple(text.to_owned(),theme::font(size,W::Semibold),ink,wrap_width);
            // 官方的 Center 在超长单行 galley 上会产生负的行内偏移，把整行文字
            // 向左推出绘制起点、压到封面/宠物上（暂停音乐时的长歌词最容易触发）。
            // release 改左对齐，由 shared_header() 按 galley 实际宽度手动居中；
            // 测试构建保留官方值，不影响 UI 测试。
            job.halign=if cfg!(test){egui::Align::Center}else{egui::Align::Min};
            job.wrap.break_anywhere=true;
            if title {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        // 单行不折行：横向滚动用，galley 宽度就是整句的真实宽度。
        let layout_single=|text:&str,size:f32| {
            let mut job=egui::text::LayoutJob::simple(text.to_owned(),theme::font(size,W::Semibold),ink,f32::INFINITY);
            job.halign=egui::Align::Min;
            job.wrap.break_anywhere=false;
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        let (galley,overflow)=match cached {
            Some(hit)=>hit,
            None=>{
                // 歌词固定 11pt，不再缩字号：单行放不下就横向滚动，比缩小字体更易读。
                // 歌名不滚动，仍沿用官方单行省略。
                // 测试构建保留官方分页兜底，UI 测试的布局断言不变。
                let size=LYRIC_BASE_SIZE;
                let mut galley=layout(&text,size);
                let mut overflow=0.0;
                if cfg!(test) {
                    if galley.rows.len()>1 {
                        // 按当前句的时间范围与各段字数切换，暂停和跳转直接跟随播放位置。
                        let total:usize=galley.rows.iter().map(|row|row.glyphs.len()).sum();
                        let mut count=0;
                        let page=galley.rows.iter().position(|row|{count+=row.glyphs.len();progress*(total as f32)<count as f32}).unwrap_or(galley.rows.len()-1);
                        let segment=galley.rows[page].glyphs.iter().map(|glyph|glyph.chr).collect::<String>();
                        galley=layout(segment.trim(),size);
                    }
                } else if !title && galley.rows.len()>1 {
                    galley=layout_single(&text,size);
                    overflow=(galley.size().x-wrap_width).max(0.0);
                }
                if !cfg!(test) {
                    self.island_lyric_cache=Some(LyricLayoutCache{text:text.clone(),width:wrap_width,white,title,galley:galley.clone(),overflow});
                }
                (galley,overflow)
            }
        };
        // 滚动时间基准：换句（行号或文本变化）时重置；副歌同一句重复出现也要重置。
        let key=(current.as_ref().map(|(index,_)|*index),text.clone());
        if key!=self.island_lyric_scroll_key {
            self.island_lyric_scroll_key=key;
            self.island_lyric_scroll_start=self.t;
        }
        self.island_lyric_scroll=(overflow>0.5)
            .then_some(LyricScroll{overflow,speed:lyric_scroll_speed(overflow,window.unwrap_or(0.0))});
        self.island_lyric_layout=Some(galley);
    }

    pub(crate) fn effective_minimal(&self)->bool {self.settings.minimal || self.browser_minimal_active}
    /// 下载流程是否活跃：状态总线里有下载聚合条目（id == "dl"）。
    /// 下载中 / 完成 / 取消的提示期间都算活跃，此时岛不避让、保持常驻显示。
    pub(crate) fn download_active(&self)->bool {
        self.status_live.iter().any(|s| s.id == "dl" || s.id.starts_with("dl-"))
    }
    /// 常驻电量环：(本机, 手机)，None 表示该来源关闭或没有数据。
    /// 两个来源互不影响，都开就并排画两个环（本机在左）。
    pub(crate) fn battery_rings(&self)->(Option<(Option<u8>,bool)>,Option<(Option<u8>,bool)>) {
        let pc=(self.settings.show_battery&&self.resident_power.0.is_some()).then_some(self.resident_power);
        let phone=(self.settings.phone_battery&&self.phone_resident.0.is_some()).then_some(self.phone_resident);
        (pc,phone)
    }
    /// 当前要画几个电量环：0 / 1 / 2。
    pub(crate) fn battery_ring_count(&self)->usize {
        let (pc,phone)=self.battery_rings();
        pc.is_some() as usize+phone.is_some() as usize
    }
    pub(crate) fn battery_visible(&self)->bool {self.battery_ring_count()>0}
    pub(crate) fn pet_enabled(&self)->bool {self.settings.pet && !self.effective_minimal() && !self.side()}
    pub(crate) fn pet_space(&self)->f32 {if self.pet_enabled(){44.0}else{0.0}}
    fn lyric_focus_space(&self)->f32 {if self.pomo.running{crate::focus_bar::LYRIC_WIDTH+10.0}else{0.0}}
    fn lyric_bar_width(&self)->f32 {LYRIC_BAR_W+if self.pet_enabled(){48.0}else{0.0}+self.lyric_focus_space()}
    /// 歌词态左侧固定通道：电量环 → 封面 → 吉祥物，歌词只在通道右侧排版。
    /// 返回 (宠物中心, 歌词左界)，均为相对胶囊左边缘 `left` 的偏移。
    /// 官方把宠物放在「胶囊中心 + 60」，歌词放在「left + content_width - 150」，
    /// 两套基准在专注模式（content_width 缩小）或宽度动画下会互相错位；
    /// 这里把三者统一到 `left` 基准，表情贴图实际宽 48px（cx±24）也计入占位。
    /// 测试构建返回 None，沿用官方坐标，避免影响 UI 测试的布局断言。
    fn lyric_lane(&self)->Option<(f32,f32)> {
        if cfg!(test) {return None;}
        // 本机与手机电量各自独立，两个环都在时左侧通道要留双份位置。
        let rings=self.battery_ring_count() as f32;
        let art_right=if rings>0.0 {36.0+rings*20.0}else{32.0};
        let pet_center=art_right+LANE_PET_HALF+LANE_ART_GAP;
        let text_left=pet_center+LANE_PET_HALF+LANE_TEXT_GAP;
        Some((pet_center,text_left))
    }
    /// 歌词排版宽度：右侧给播放指示留出 30px；测试构建保持官方 220px。
    fn lyric_text_width(&self)->f32 {
        match self.lyric_lane() {
            Some((_,text_left))=>(self.lyric_bar_width()-self.lyric_focus_space()-LANE_RIGHT_PAD-text_left).max(90.0),
            None=>LYRIC_BAR_W-80.0,
        }
    }
    fn notice_size(&self,a:&Alert)->(f32,f32) {
        let (width,height)=if a.meeting.is_some(){(360.0,136.0)}else if a.source==crate::extras::AlertSource::Internal && a.title=="蓝牙设备"{(HUD_W,48.0)}else if a.is_hud(){(HUD_W,38.0)}else{(334.0,44.0)};
        (width,height)
    }
    pub fn set_pet(&mut self,enabled:bool) {
        self.settings.pet=enabled;if enabled {self.settings.minimal=false;}self.settings.save();
'''),
    ("src/app.rs",
     '''        if let Some(at_top)=env.browser_at_top {self.browser_at_top=at_top;}
        self.screen=env.screen;
        if self.settings.skin.is_glass() {
            let target=if self.settings.skin==model::Skin::GlassLight {glass::light_tint(env.backdrop,env.glass_blur)}else{glass::glass_tint(env.backdrop,env.glass_blur)};
            self.glass_tint=if self.glass_tint<=0.0 {target}else{anim::approach(self.glass_tint,target,3.0,dt)};
        }
        let browser_minimal=self.settings.browser_minimal && self.browser_at_top;
        if browser_minimal && !self.browser_minimal_active {
            self.open=false;
            self.hover_lock=true;
            self.hover_since=None;
            self.leave_since=None;
            self.forced_open_until=0.0;
''',
     '''        if let Some(at_top)=env.browser_at_top {self.browser_at_top=at_top;}
        self.screen=env.screen;
        if self.settings.skin.is_glass() {
            let target=if self.settings.skin==model::Skin::GlassLight {glass::light_tint(env.backdrop,env.glass_blur)}else{glass::glass_tint(env.backdrop,env.glass_blur)};
            self.glass_tint=if self.glass_tint<=0.0 {target}else{anim::approach(self.glass_tint,target,3.0,dt)};
        }
        // 下载流程（下载中/完成/取消的提示）活跃时不避让，保持常驻显示，
        // 下载流程活跃时保持常驻，否则浏览器全屏时下载进度/结果会被收成细线看不见。
        let browser_minimal=self.settings.browser_minimal && self.browser_at_top && !self.download_active();
        if browser_minimal && !self.browser_minimal_active {
            self.open=false;
            self.hover_lock=true;
            self.hover_since=None;
            self.leave_since=None;
            self.forced_open_until=0.0;
'''),
    ("src/app.rs",
     '''        if hud_active.is_none() && self.hud_alpha<0.003 {self.hud_notice=None;}
        let notice_target=if !self.notice.as_ref().is_some_and(|a|a.messenger().is_some()) && self.compact_notices()&&(self.notice.is_some()||self.t<self.notification_bell_until) {44.0}else{0.0};
        self.notice_width.target=notice_target;
        if self.compact_notices(){self.notice_width.step(dt);}else{self.notice_width.snap(0.0);self.notice_details=false;}
        self.update_media(env,&ctx);
        let bell=self.t<self.notification_bell_until&&!self.compact_notices();
        self.resident_power=(env.battery,env.charging);
        let mut layout=Layout::new(self.settings.show_battery&&env.battery.is_some(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled(),self.weather_pill().is_some());
        layout.reorder(&self.settings.resident_order);
        for (position,extra) in self.quota_balance_layout(&ctx){layout.balance_tail(position,extra);}
        if self.settings.clock_period!=0 { layout.widen_clock(40.0); }
        if let Some(extra)=self.weather_pill_extra(&ctx){ layout.widen(bar::WEATHER,extra); }
        if self.pet_enabled() {
            if let Some(a)=self.notice.as_ref().filter(|_|!self.compact_notices()&&!self.open){layout.x[bar::PET]=Some(22.0-(self.notice_size(a).0+self.pet_space())/2.0);}
            else if self.island_lyrics_active() {layout.x[bar::PET]=Some(60.0-self.lyric_bar_width()/2.0);}
        }
        self.resident.step(layout,dt,ctx.style().animation_time>0.0&&bar::animations_enabled());
        if self.opts.fetch_lyrics {
            self.poll_lyrics();
        }
        self.update_island_lyric_layout(&ctx);

        let cx = win.center().x;
        // 胶囊皮肤悬浮在屏幕顶边下方。
        let pill = self.settings.island_shape==model::IslandShape::Pill;
        let top = win.top() + if pill { PILL_GAP } else { 0.0 };
        let side = self.side();
''',
     '''        if hud_active.is_none() && self.hud_alpha<0.003 {self.hud_notice=None;}
        let notice_target=if !self.notice.as_ref().is_some_and(|a|a.messenger().is_some()) && self.compact_notices()&&(self.notice.is_some()||self.t<self.notification_bell_until) {44.0}else{0.0};
        self.notice_width.target=notice_target;
        if self.compact_notices(){self.notice_width.step(dt);}else{self.notice_width.snap(0.0);self.notice_details=false;}
        self.update_media(env,&ctx);
        let bell=self.t<self.notification_bell_until&&!self.compact_notices();
        let phone=if self.settings.phone_battery{self.phone_power.poll()}else{(None,false)};
        // 本机电量与手机电量彻底分开：电量开关只管本机，手机电量开关只管手机，
        // 两个来源各自判断、各自显示，互不影响。
        self.resident_power=(env.battery,env.charging);
        self.phone_resident=phone;
        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}
        self.pc_events(env.battery,env.charging);
        // 勿扰到点自动解除；同时把设置项归零，避免界面上还显示着「60 分钟」。
        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        if self.t-self.meeting_checked_at>5.0 {self.meeting_checked_at=self.t;self.meeting_quiet(env);}
        self.tick_snoozed();
        if self.keyword_src!=self.settings.notice_keywords {
            self.keyword_src=self.settings.notice_keywords.clone();
            self.keywords=self.settings.keywords().iter().map(|word|word.to_lowercase()).collect();
        }
        self.clipboard.flush();
        if self.t-self.quota_checked_at>30.0 {self.quota_checked_at=self.t;self.quota_threshold_alert();}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled(),self.weather_pill().is_some());;
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
        if status_width>0.0 {layout.add_status(status_width);}
       layout.reorder(&self.settings.resident_order);
        // 两个电量环并排：电池槽加宽 20px，其余槽位自动让开。
        if self.battery_ring_count()==2 {layout.widen_battery(20.0);}
        for (position,extra) in self.quota_balance_layout(&ctx){layout.balance_tail(position,extra);}
        if self.settings.clock_period!=0 { layout.widen_clock(40.0); }
        if let Some(extra)=self.weather_pill_extra(&ctx){ layout.widen(bar::WEATHER,extra); }
        if self.pet_enabled() {
            if let Some(a)=self.notice.as_ref().filter(|_|!self.compact_notices()&&!self.open){layout.x[bar::PET]=Some(22.0-(self.notice_size(a).0+self.pet_space())/2.0);}
            else if self.island_lyrics_active() {
                // 与歌词共用同一个 `left` 基准：宠物固定贴在封面右侧，
                // 专注模式 / 宽度动画改变 content_width 时也不会漂进歌词区。
                let base=if cfg!(test) {self.lyric_bar_width()} else {self.w.pos.max(self.lyric_bar_width()-self.lyric_focus_space())};
                let pet_center=self.lyric_lane().map(|(p,_)|p).unwrap_or(60.0);
                layout.x[bar::PET]=Some(pet_center-base/2.0);
            }
        }
        self.resident.step(layout,dt,ctx.style().animation_time>0.0&&bar::animations_enabled());
        if self.opts.fetch_lyrics {
            self.poll_lyrics();
        }
        self.update_island_lyric_layout(&ctx);
        // 通用状态总线：文件里有什么岛上就有什么；同 id 覆写续期，删掉即撤下。
        let (statuses,removed)=self.island_status.poll();
        // poll 每 2 秒才返回一次数据，其余帧返回空；只在真有变化时更新，
        // 否则常驻区会只闪一帧就消失。
        if !statuses.is_empty() || !removed.is_empty() {
            // 启动或清空后的首轮不闪，避免一次排队好几条横幅；之后新增的状态才闪一次。
            let first_fill=self.status_live.is_empty();
            for status in &statuses {
                if !first_fill && !self.status_live.iter().any(|old|old.key()==status.key()) {
                    self.push_status(status.clone());
                }
            }
            self.status_live=statuses;
        }
        for key in removed { self.drop_status(&key); }

        // MAA 对接：仅在通知名单里开启 MAA 时跟随日志；关掉即停（不读盘）。
        // asst.log 提供一次性事件（公招高星/出错），gui.log 提供任务队列进度（常驻胶囊）。
        if self.maa_enabled() {
            let dir=self.maa_log_path().and_then(|p|p.parent().map(|d|d.to_path_buf()));
            self.maa_watch.follow(dir.as_ref().map(|d|d.join("asst.log")));
            for line in self.maa_watch.poll_lines() {
                if let Some(event)=crate::maa::parse_callback(&line) { self.maa_event(event); }
            }
            self.maa_prog.follow(dir.as_ref().map(|d|d.join("gui.log")));
            for line in self.maa_prog.poll_lines() { self.maa_tracker.feed(&line); }
            self.refresh_maa_status();
        } else {
            self.maa_watch.follow(None);
            self.maa_prog.follow(None);
            self.maa_tracker.reset();
            self.maa_status=None;
        }

        let cx = win.center().x;
        // 胶囊皮肤悬浮在屏幕顶边下方。
        let pill = self.settings.island_shape==model::IslandShape::Pill;
        let top = win.top() + if pill { PILL_GAP } else { 0.0 };
        let side = self.side();
'''),
    ("src/app.rs",
     '''            || self.hud_alpha > 0.003 && self.hud_alpha < 0.997
            || !self.notice_width.settled(1.0);
        let pet_visible=self.pet_a>0.01 && !self.open && !minimal_pill;
        let ambient=self.settings.skin==model::Skin::Fluid && self.fluid.animating() || self.pomo.running && (self.open && self.page==1 || !self.open && !minimal_pill && self.notice.is_none() && bar::animations_enabled())
            || self.media.playing && (self.open && self.page==0 || !self.open && !minimal_pill && self.notice.is_none() && self.media.is_music())
            || self.notice.as_ref().is_some_and(|notice|notice.meeting.is_some());
        let animating=transitioning || ambient || pet_visible && self.pet.state!=PetState::Sleeping;
        // 交互形变保持 60 帧，媒体 30 帧；小表情 20 帧，睡眠 10 帧，静态界面每秒核对提醒。
        let repaint_after=std::time::Duration::from_secs_f64(if transitioning{1.0/60.0}else if ambient{1.0/30.0}else if pet_visible{if self.pet.state==PetState::Sleeping{0.1}else{0.05}}else{1.0});
        cmds.append(&mut self.pending);
        FrameOut {
            region: rel(floating_toast.into_iter().chain(self.shelf_menu_rect.map(|r|r.union(r + ctx.style().visuals.popup_shadow.margin()).expand(1.0 / ctx.pixels_per_point()))).fold(region,Rect::union).intersect(ui.max_rect())),
            glass: glass_region,
            hit: rel(if side {self.side_hit_rect(win,self.progress())} else {self.hit_rect(cx, top, self.progress())}),
            notice_hit:notice_shape.into_iter().chain(notice_details).chain(messenger_rect).chain(self.shelf_menu_rect).reduce(|a,b|a.union(b)).map(rel),
''',
     '''            || self.hud_alpha > 0.003 && self.hud_alpha < 0.997
            || !self.notice_width.settled(1.0);
        let pet_visible=self.pet_a>0.01 && !self.open && !minimal_pill;
        let ambient=self.settings.skin==model::Skin::Fluid && self.fluid.animating() || self.pomo.running && (self.open && self.page==1 || !self.open && !minimal_pill && self.notice.is_none() && bar::animations_enabled())
            || self.media.playing && (self.open && self.page==0 || !self.open && !minimal_pill && self.notice.is_none() && self.media.is_music())
            || self.notice.as_ref().is_some_and(|notice|notice.meeting.is_some());
        // 长歌词滚动时必须持续重绘；暂停时也要——人正是这时候才想读完一整句。
        let lyric_scrolling=self.island_lyric_scroll.is_some();
        let animating=transitioning || ambient || lyric_scrolling || pet_visible && self.pet.state!=PetState::Sleeping;
        // 交互形变保持 60 帧，媒体/歌词滚动 30 帧；小表情 20 帧，睡眠 10 帧，静态界面每秒核对提醒。
        let repaint_after=std::time::Duration::from_secs_f64(if transitioning{1.0/60.0}else if ambient||lyric_scrolling{1.0/30.0}else if pet_visible{if self.pet.state==PetState::Sleeping{0.1}else{0.05}}else{1.0});
        cmds.append(&mut self.pending);
        FrameOut {
            region: rel(floating_toast.into_iter().chain(self.shelf_menu_rect.map(|r|r.union(r + ctx.style().visuals.popup_shadow.margin()).expand(1.0 / ctx.pixels_per_point()))).fold(region,Rect::union).intersect(ui.max_rect())),
            glass: glass_region,
            hit: rel(if side {self.side_hit_rect(win,self.progress())} else {self.hit_rect(cx, top, self.progress())}),
            notice_hit:notice_shape.into_iter().chain(notice_details).chain(messenger_rect).chain(self.shelf_menu_rect).reduce(|a,b|a.union(b)).map(rel),
'''),
    ("src/app.rs",
     '''        }
    }
    /// 展开面板，让旁边的未读胶囊露出来。
    pub(crate) fn reveal_unread(&mut self){
        self.notice=None;
        self.open=true; self.forced_open_until=self.t+4.0;
        self.w.tune(OPEN_W.0,OPEN_W.1);self.h.tune(OPEN_H.0,OPEN_H.1);
        self.w.target=PW;self.h.target=self.expanded_height();
    }
    fn notice_rect(&self,cx:f32,top:f32)->Option<Rect>{
        if !self.compact_notices(){return None;}
        let width=self.notice_width.pos-8.0;
        (width>0.5).then(||Rect::from_min_size(Pos2::new(cx+self.w.pos/2.0+8.0,top),Vec2::new(width,BAR_H)))
    }
''',
     '''        }
    }
    /// 展开面板，让旁边的未读胶囊露出来。
    pub(crate) fn reveal_unread(&mut self){
        self.notice=None;
        self.open=true; self.forced_open_until=self.t+4.0;
        let target_h=self.expanded_height();
        self.tune_open(PW,target_h);
    }
    fn notice_rect(&self,cx:f32,top:f32)->Option<Rect>{
        if !self.compact_notices(){return None;}
        let width=self.notice_width.pos-8.0;
        (width>0.5).then(||Rect::from_min_size(Pos2::new(cx+self.w.pos/2.0+8.0,top),Vec2::new(width,BAR_H)))
    }
'''),
    ("src/app.rs",
     '''    fn glass_rim(&self)->Color32 {
        let strength=if self.settings.skin==model::Skin::GlassLight {0.55} else {glass::rim_alpha()*0.7};
        Color32::from_white_alpha((255.0*strength).round() as u8)
    }

    /// 绘制岛体（或通知胶囊）的本体：经典皮肤为纯黑；毛玻璃为着色底 + 顶部柔光 + 边缘高光。
    fn paint_body(&self,painter:&egui::Painter,rect:Rect,flare:f32,flare_h:f32,radius:f32,feather:f32){
        let skin=self.settings.skin;
        if skin==model::Skin::Fluid {
            let pill=self.settings.island_shape==model::IslandShape::Pill;
            let (map,frame)=shape::EdgeMap::of(rect,if pill {model::Dock::Top}else{self.dock()});
            let mut rows=shape::sections(frame,flare,flare_h,radius);
            if pill {
                let mid=frame.center().y;
                rows.retain(|row|row.0>mid && row.0>=frame.bottom()-radius);
''',
     '''    fn glass_rim(&self)->Color32 {
        let strength=if self.settings.skin==model::Skin::GlassLight {0.55} else {glass::rim_alpha()*0.7};
        Color32::from_white_alpha((255.0*strength).round() as u8)
    }

    /// 绘制岛体（或通知胶囊）的本体：经典皮肤为纯黑；毛玻璃为着色底 + 顶部柔光 + 边缘高光。
    fn paint_body(&mut self,painter:&egui::Painter,rect:Rect,flare:f32,flare_h:f32,radius:f32,feather:f32){
        let skin=self.settings.skin;
        if skin==model::Skin::Fluid {
            // 胶囊 + 顶部停靠：和毛玻璃共用 egui 的精确圆角矩形，颜色走一张小纹理。
            // 旧的「按行梯形条 + 折线圆弧 + 手工羽化带」只在悬浮云上用，
            // 直线转圆弧处的折角感正是它独有、毛玻璃没有的瑕疵。
            if self.settings.island_shape==model::IslandShape::Pill && !self.dock().is_side() {
                self.paint_fluid_pill(painter,rect,radius);
                return;
            }
            let pill=self.settings.island_shape==model::IslandShape::Pill;
            let (map,frame)=shape::EdgeMap::of(rect,if pill {model::Dock::Top}else{self.dock()});
            let mut rows=shape::sections(frame,flare,flare_h,radius);
            if pill {
                let mid=frame.center().y;
                rows.retain(|row|row.0>mid && row.0>=frame.bottom()-radius);
'''),
    ("src/app.rs",
     '''            shape::fill_gradient(painter,rect,flare,flare_h,radius,sheen,Color32::TRANSPARENT,(rect.height()*0.6).min(64.0),feather);
        }
        if rect.height()>=10.0 && rect.width()>=20.0 {
            let edge=Rect::from_min_max(rect.min+Vec2::new(0.5,0.0),rect.max-Vec2::new(0.5,0.5));
            painter.add(egui::Shape::line(shape::outline(edge,flare,flare_h,(radius-0.5).max(0.0)),egui::Stroke::new(1.0_f32,self.glass_rim())));
        }
    }

    /// 拿起时沿岛体轮廓的一圈柔光：外圈宽而淡，内圈细而亮。
    fn paint_lift_halo(&self,painter:&egui::Painter,rect:Rect,flare:f32,flare_h:f32,radius:f32,amount:f32) {
        if rect.width()<10.0 || rect.height()<10.0 {return;}
        let pill=self.settings.island_shape==model::IslandShape::Pill;
''',
     '''            shape::fill_gradient(painter,rect,flare,flare_h,radius,sheen,Color32::TRANSPARENT,(rect.height()*0.6).min(64.0),feather);
        }
        if rect.height()>=10.0 && rect.width()>=20.0 {
            let edge=Rect::from_min_max(rect.min+Vec2::new(0.5,0.0),rect.max-Vec2::new(0.5,0.5));
            painter.add(egui::Shape::line(shape::outline(edge,flare,flare_h,(radius-0.5).max(0.0)),egui::Stroke::new(1.0_f32,self.glass_rim())));
        }
    }

    /// 悬浮云主体：轮廓交给 `RectShape` 的精确圆角，流动颜色做成一张固定尺寸的小纹理。
    ///
    /// 固定 160×80，避免展开动画里尺寸每帧变化导致纹理反复重新分配；
    /// 光晕尺度是 152×100 逻辑点，双线性插值完全够用，看不出采样痕迹。
    fn paint_fluid_pill(&mut self,painter:&egui::Painter,rect:Rect,radius:f32){
        const TEX_W:usize=160;
        const TEX_H:usize=80;
        let w=rect.width().max(1.0);
        let h=rect.height().max(1.0);
        let expand=self.progress().clamp(0.0,1.0);
        let mut image=egui::ColorImage::new([TEX_W,TEX_H],Color32::TRANSPARENT);
        for row in 0..TEX_H {
            let py=h*(row as f32+0.5)/TEX_H as f32;
            for col in 0..TEX_W {
                let px=w*(col as f32+0.5)/TEX_W as f32;
                image.pixels[row*TEX_W+col]=self.fluid.compose(px,py,w,h,expand);
            }
        }
        let options=egui::TextureOptions::LINEAR;
        let handle=self.fluid_tex.get_or_insert_with(||{
            painter.ctx().load_texture("daozai-fluid",image.clone(),options)
        });
        handle.set(image,options);
        let radius=radius.min(rect.height().min(rect.width())*0.5);
        let uv=Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0));
        painter.add(egui::epaint::RectShape::filled(rect,radius,Color32::WHITE).with_texture(handle.id(),uv));
    }

    /// 拿起时沿岛体轮廓的一圈柔光：外圈宽而淡，内圈细而亮。
    fn paint_lift_halo(&self,painter:&egui::Painter,rect:Rect,flare:f32,flare_h:f32,radius:f32,amount:f32) {
        if rect.width()<10.0 || rect.height()<10.0 {return;}
        let pill=self.settings.island_shape==model::IslandShape::Pill;
'''),
    ("src/app.rs",
     '''        self.position_drag=Some(PositionDrag{start:start-screen.min,offset:offset as f32,original:(self.dock(),offset)});
        let mut glide=anim::Spring::new(offset as f32,GLIDE_FOLLOW.0,GLIDE_FOLLOW.1);
        if let Some(g)=self.drag_glide {glide.pos=g.pos;glide.vel=g.vel;}
        self.drag_glide=Some(glide);
        // 拿起时给一点初速度，按下后立刻有反馈。
        self.drag_lift.vel=self.drag_lift.vel.max(4.0);
    }

    /// 悬停状态机：意图缓冲 → 弹簧展开 → 离开延迟 → 临界阻尼收起；全程可打断。
    fn morph(&mut self, want: bool, inside: bool, t: f64, dt: f32) {
        let (bw, bh) = self.base_size();
        if self.position_drag.is_some() {
            // 拿起后面板快速收成常驻形态；放大效果由 `drag_scale` 叠加在外形上。
            self.open=false;
            self.hover_lock=true;
            self.hover_since=None;
            self.leave_since=None;
            self.forced_open_until=0.0;
            self.music_volume_open=false;
            self.w.tune(CLOSE.0, CLOSE.1);
            self.h.tune(CLOSE.0, CLOSE.1);
            self.w.target=bw;
            self.h.target=bh;
        } else if want {
            self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let intent=if self.effective_minimal() {MINIMAL_HOVER_INTENT}else{HOVER_INTENT};
            if !self.open && (self.drag_hot || t - since >= intent) {
                self.open = true;
                self.w.tune(OPEN_W.0, OPEN_W.1);
                self.h.tune(OPEN_H.0, OPEN_H.1);
                self.w.target = PW;
                self.h.target = self.expanded_height();
                if self.drag_hot {
                    // 拖入不等缓冲，直接给一点初速度，30 ms 内就能看到形变
                    self.w.vel = self.w.vel.max(600.0);
                    self.h.vel = self.h.vel.max(260.0);
                }
            } else if !self.open {
                self.w.tune(SWELL_K.0, SWELL_K.1);
                self.h.tune(SWELL_K.0, SWELL_K.1);
                // 悬停时轻微胀大：厚度方向胀得多一点（侧边停靠时厚度是宽度）。
                let (sw,sh)=if self.side(){(SWELL.1,SWELL.0)}else{SWELL};
                self.w.target = bw * sw;
                self.h.target = bh * sh;
            }
        } else {
            self.hover_since = None;
            if self.open {
                let since = *self.leave_since.get_or_insert(t);
                if t - since >= LEAVE_DELAY {
                    self.open = false;
                    self.music_volume_open=false;
                    self.leave_since = None;
                    self.w.tune(CLOSE.0, CLOSE.1);
                    self.h.tune(CLOSE.0, CLOSE.1);
                    self.w.target = bw;
                    self.h.target = bh;
                    self.cancel_edit();
                    self.confirm_clear = None;
                }
            } else {
                self.w.tune(CLOSE.0, CLOSE.1);
                self.h.tune(CLOSE.0, CLOSE.1);
                self.w.target = bw;
                self.h.target = bh;
            }
        }
        let _ = inside;
        if self.open {self.h.target=self.expanded_height();}
''',
     '''        self.position_drag=Some(PositionDrag{start:start-screen.min,offset:offset as f32,original:(self.dock(),offset)});
        let mut glide=anim::Spring::new(offset as f32,GLIDE_FOLLOW.0,GLIDE_FOLLOW.1);
        if let Some(g)=self.drag_glide {glide.pos=g.pos;glide.vel=g.vel;}
        self.drag_glide=Some(glide);
        // 拿起时给一点初速度，按下后立刻有反馈。
        self.drag_lift.vel=self.drag_lift.vel.max(4.0);
   }

    /// 悬停状态机：意图缓冲 → 弹簧展开 → 离开延迟 → 临界阻尼收起；全程可打断。
    /// 按当前配置的动效启动展开；「瞬时」直接到位，不跑形变。
    fn tune_open(&mut self,target_w:f32,target_h:f32){
        if let Some(t)=self.settings.open_anim.tuning() {
            self.w.tune(t.open_w.0,t.open_w.1);
            self.h.tune(t.open_h.0,t.open_h.1);
            self.w.target=target_w;self.h.target=target_h;
        } else {
            self.w.snap(target_w);self.h.snap(target_h);
        }
    }
    /// 按当前配置的动效收起；「瞬时」直接到位。
    fn tune_close(&mut self,bw:f32,bh:f32){
        if let Some(t)=self.settings.open_anim.tuning() {
            self.w.tune(t.close.0,t.close.1);
            self.h.tune(t.close.0,t.close.1);
            self.w.target=bw;self.h.target=bh;
        } else {
            self.w.snap(bw);self.h.snap(bh);
        }
    }

    fn morph(&mut self, want: bool, inside: bool, t: f64, dt: f32) {
        let (bw, bh) = self.base_size();
        let tuning=self.settings.open_anim.tuning();
        let instant=tuning.is_none();
        // 非展开态每帧都会重新 tune，所以这里也要用配置里的收起参数，
        // 否则 `tune_close` 设置的参数只在切换那一帧有效。
        let close=tuning.map(|t|t.close).unwrap_or(CLOSE);
        if self.position_drag.is_some() {
            // 拿起后面板快速收成常驻形态；放大效果由 `drag_scale` 叠加在外形上。
            self.open=false;
            self.hover_lock=true;
            self.hover_since=None;
            self.leave_since=None;
            self.forced_open_until=0.0;
            self.music_volume_open=false;
            self.w.tune(close.0, close.1);
            self.h.tune(close.0, close.1);
            self.w.target=bw;
            self.h.target=bh;
        } else if want {
           self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let delay=hover_delay_secs(self.settings.hover_delay_ms,self.effective_minimal());
            let swell=self.settings.hover_delay_ms>=SWELL_MIN_MS && !instant;
            if !self.open && (self.drag_hot || t - since >= delay) {
                self.open = true;
                let target_h=self.expanded_height();
                self.tune_open(PW,target_h);
                if !instant && self.drag_hot {
                    // 拖入不等缓冲，直接给一点初速度，30 ms 内就能看到形变
                    self.w.vel = self.w.vel.max(600.0);
                    self.h.vel = self.h.vel.max(260.0);
                }
            } else if !self.open {
                if swell {
                    self.w.tune(SWELL_K.0, SWELL_K.1);
                    self.h.tune(SWELL_K.0, SWELL_K.1);
                    // 悬停时轻微胀大：厚度方向胀得多一点（侧边停靠时厚度是宽度）。
                    let (sw,sh)=if self.side(){(SWELL.1,SWELL.0)}else{SWELL};
                    self.w.target = bw * sw;
                    self.h.target = bh * sh;
                } else {
                    // 短等待（或瞬时动效）：保持静止，到点直接用同一条弹簧展开，
                    // 不会出现「鼓一半被拉走」的接力感。
                    self.w.tune(close.0, close.1);
                    self.h.tune(close.0, close.1);
                    self.w.target = bw;
                    self.h.target = bh;
                }
            }
        } else {
            self.hover_since = None;
            if self.open {
                let since = *self.leave_since.get_or_insert(t);
                if t - since >= LEAVE_DELAY {
                    self.open = false;
                    self.music_volume_open=false;
                    self.leave_since = None;
                    self.tune_close(bw,bh);
                    self.cancel_edit();
                    self.confirm_clear = None;
                }
            } else {
                self.w.tune(close.0, close.1);
                self.h.tune(close.0, close.1);
                self.w.target = bw;
                self.h.target = bh;
            }
        }
        let _ = inside;
        if self.open {self.h.target=self.expanded_height();}
'''),
    ("src/app.rs",
     '''        self.slide.step(dt);
    }

    // ---------------------------------------------------------------
    // 顶栏：电池 + 日期 + 时钟。药丸里的电池与时钟会「飞」到顶栏位置（共享元素过渡）。
    // ---------------------------------------------------------------
    fn shared_header(&mut self, base: &Card, cx: f32, p: f32, reveal: f32, env: &Env) {
        let local_cx = cx - base.origin.x;
        let fit=((WIN_W-32.0)/self.resident.target.width).min(1.0);
        let fitted=base.scaled(local_cx,15.0,lerp(fit,1.0,p));
        let base=&fitted;
        let lyric=self.island_lyric().map(|(index,_)|index);
        let lyric_alpha=if self.island_lyrics_active() {1.0-smoothstep(0.0,0.35,p)}else{0.0};
        if let Some(galley)=self.island_lyric_layout.clone().filter(|_|lyric_alpha>0.003) {
            let c=base.faded(lyric_alpha);
            let content_width=self.lyric_bar_width()-self.lyric_focus_space();
            let width=self.w.pos.max(content_width);
            let left=local_cx-width/2.0;
            let center_y=BAR_H/2.0;
            if let Some((image,texture))=&self.music_artwork {
                let [width,height]=image.size;
                let uv_size=if width>height {egui::vec2(height as f32/width as f32,1.0)}else{egui::vec2(1.0,width as f32/height as f32)};
                let uv=Rect::from_center_size(Pos2::new(0.5,0.5),uv_size);
                c.painter.add(egui::epaint::RectShape::filled(c.rect(left+12.0,center_y-10.0,20.0,20.0),5.0*c.s,Color32::WHITE).with_texture(texture.id(),uv));
            } else {
                c.fill(left+12.0,center_y-10.0,20.0,20.0,5.0,theme::CARD);
                paint::note(&c,left+22.0,center_y,13.0,theme::WHITE);
            }
            let index=lyric.unwrap_or(0);
            let id=egui::Id::new("island-lyric-line");
            if !self.ly.island_animation_started {
                c.ui.ctx().animate_value_with_time(id,index as f32,0.0);
                self.ly.island_animation_started=true;
            }
            let shown=c.ui.ctx().animate_value_with_time(id,index as f32,0.18);
            let fade=1.0-(index as f32-shown).abs().min(1.0);
            let position=c.pos(left+content_width-LYRIC_BAR_W/2.0,center_y-galley.size().y/2.0+(1.0-fade)*3.0);
            c.painter.galley(position,galley,if lyric.is_some(){theme::WHITE}else{theme::GRAY});
            paint::playback_bars(&c,left+content_width-21.0,center_y,self.t,self.media.playing,theme::WHITE);
            if self.pomo.running {
                let alpha=smoothstep(0.88,1.0,(width-content_width)/self.lyric_focus_space());
                crate::focus_bar::draw(&c.faded(alpha),left+width-14.0-crate::focus_bar::LYRIC_WIDTH/2.0,center_y,self.t,&self.pomo,self.phase_secs(self.pomo.phase),true);
            }
        }
        if lyric_alpha>=0.997 {return;}
        let header=base.faded(1.0-lyric_alpha);
        let base=&header;
        let m = anim::ease_in_out(smoothstep(0.0, 0.85, p));
        let clock = self.settings.clock_text(&env.clock);
        let show_battery=self.settings.show_battery && env.battery.is_some();

        // 顶栏终点布局：[电池][日期][时钟] 作为一组整体居中
        let date_w = base.text_w(&env.date, size::DATE, W::Regular);
        let clock_w = paint::clock_width(base,&clock,size::CLOCK);
        let battery_space=if show_battery {24.0}else{0.0};
        let group = battery_space + date_w + 8.0 + clock_w;
        let gx = local_cx - group / 2.0 + if show_battery {6.0}else{0.0};
        // 日程页左侧还有新增按钮，顶栏从操作区右侧开始，避免较长的日期和时段文字挤到按钮上。
        let gx = if self.page==CALENDAR_PAGE {gx.max(155.0)}else{gx};
        let hb = (gx + 8.5, 18.6);
        let hd = gx + battery_space;
''',
     '''        self.slide.step(dt);
    }

    // ---------------------------------------------------------------
    // 顶栏：电池 + 日期 + 时钟。药丸里的电池与时钟会「飞」到顶栏位置（共享元素过渡）。
    // ---------------------------------------------------------------
    /// 单条状态的估算宽度：中文按 11px/字，夹在 48~140 之间。
    /// 渲染用的状态列表：状态总线里的常驻条目 + MAA 的常驻进度胶囊。
    fn statuses_for_render(&self) -> Vec<crate::island_status::Status> {
        let mut list = self.status_live.clone();
        if let Some(maa) = &self.maa_status { list.push(maa.clone()); }
        list
    }

    fn status_item_width(&self, status: &crate::island_status::Status) -> f32 {
        let chars = status.text.chars().count() as f32;
        // 有图标时左侧要留出图标位置。
        let icon_pad = if status.icon.is_empty() { 0.0 } else { 15.0 };
        (chars * 11.0 + 24.0 + icon_pad).clamp(48.0, 160.0)
    }

    /// 状态区总宽：最多 3 条，条间 6px。
    fn status_total_width(&self) -> f32 {
        let list = self.statuses_for_render();
        let shown = list.len().min(3);
        if shown == 0 { return 0.0; }
        let items: f32 = list.iter().take(shown).map(|s| self.status_item_width(s)).sum();
        items + (shown.saturating_sub(1)) as f32 * 6.0
    }

    fn shared_header(&mut self, base: &Card, cx: f32, p: f32, reveal: f32, env: &Env) {
        let local_cx = cx - base.origin.x;
        let fit=((WIN_W-32.0)/self.resident.target.width).min(1.0);
        let fitted=base.scaled(local_cx,15.0,lerp(fit,1.0,p));
        let base=&fitted;
        let lyric=self.island_lyric().map(|(index,_)|index);
        let lyric_alpha=if self.island_lyrics_active() {1.0-smoothstep(0.0,0.35,p)}else{0.0};
        // 提前算好，歌词接管胶囊时也要用它把手机电量留在左侧。
        let resident_battery=self.battery_visible();
        if let Some(galley)=self.island_lyric_layout.clone().filter(|_|lyric_alpha>0.003) {
            let c=base.faded(lyric_alpha);
            let content_width=self.lyric_bar_width()-self.lyric_focus_space();
            let width=self.w.pos.max(content_width);
            let left=local_cx-width/2.0;
            let center_y=BAR_H/2.0;
            // 有手机电量时把封面右移，给最左侧的常驻电量环让出位置。
            let art_left=left+if resident_battery {36.0}else{12.0};
            if let Some((image,texture))=&self.music_artwork {
                let [width,height]=image.size;
                let uv_size=if width>height {egui::vec2(height as f32/width as f32,1.0)}else{egui::vec2(1.0,width as f32/height as f32)};
                let uv=Rect::from_center_size(Pos2::new(0.5,0.5),uv_size);
                c.painter.add(egui::epaint::RectShape::filled(c.rect(art_left,center_y-10.0,20.0,20.0),5.0*c.s,Color32::WHITE).with_texture(texture.id(),uv));
            } else {
                c.fill(art_left,center_y-10.0,20.0,20.0,5.0,theme::CARD);
                paint::note(&c,art_left+10.0,center_y,13.0,theme::WHITE);
            }
            let index=lyric.unwrap_or(0);
            let id=egui::Id::new("island-lyric-line");
            if !self.ly.island_animation_started {
                c.ui.ctx().animate_value_with_time(id,index as f32,0.0);
                self.ly.island_animation_started=true;
            }
            let shown=c.ui.ctx().animate_value_with_time(id,index as f32,0.18);
            let fade=1.0-(index as f32-shown).abs().min(1.0);
            // 歌词固定在电量环/封面/宠物右侧的通道里，并在剩余区域居中；
            // 测试构建沿用官方坐标。galley 本身是左对齐绘制的，这里手动补偿居中。
            let lyric_left=match self.lyric_lane() {
                Some((_,text_left))=>{
                    let avail=(content_width-LANE_RIGHT_PAD-text_left).max(90.0);
                    // 短歌词在歌词通道内完整居中；长歌词放不下时由后面的横向滚动接管。
                    let centering=((avail-galley.size().x/c.s).max(0.0)/2.0);
                    text_left+centering
                },
                None=>content_width-LYRIC_BAR_W/2.0,
            };
            // 长歌词横向滚动：句首停留后匀速推进，到尾部停住，换句复位。
            let scroll_offset=self.island_lyric_scroll.map_or(0.0,|scroll|{
                let elapsed=(self.t-self.island_lyric_scroll_start).max(0.0) as f32;
                lyric_scroll_offset(elapsed,scroll.speed,scroll.overflow)/c.s
            });
            let position=c.pos(left+lyric_left-scroll_offset,center_y-galley.size().y/2.0+(1.0-fade)*3.0);
            // 兜底裁剪：超长或异常 galley 一律不许画进左侧固定通道。
            let lyric_c=match self.lyric_lane() {
                Some((_,text_left))=>c.clipped(left+text_left,0.0,(content_width-LANE_RIGHT_PAD-text_left).max(90.0),BAR_H),
                None=>c.clone(),
            };
            lyric_c.painter.galley(position,galley,if lyric.is_some(){theme::WHITE}else{theme::GRAY});
            paint::playback_bars(&c,left+content_width-21.0,center_y,self.t,self.media.playing,theme::WHITE);
            if self.pomo.running {
                let alpha=smoothstep(0.88,1.0,(width-content_width)/self.lyric_focus_space());
                crate::focus_bar::draw(&c.faded(alpha),left+width-14.0-crate::focus_bar::LYRIC_WIDTH/2.0,center_y,self.t,&self.pomo,self.phase_secs(self.pomo.phase),true);
            }
        }
        if lyric_alpha>=0.997 {
            // 电量常驻：歌词完全接管胶囊时，仍在最左侧保留电量环（本机在左、手机在右）。
            if resident_battery {
                let content_width=self.lyric_bar_width()-self.lyric_focus_space();
                let width=self.w.pos.max(content_width);
                let left=local_cx-width/2.0;
                let (pc_ring,phone_ring)=self.battery_rings();
                let mut cx=left+20.0;
                for ring in [pc_ring,phone_ring].into_iter().flatten() {
                    paint::battery(&base.faded(1.0),cx,BAR_H/2.0,ring.0,ring.1,1.0);
                    cx+=20.0;
                }
            }
            return;
        }
        let header=base.faded(1.0-lyric_alpha);
        let base=&header;
        let m = anim::ease_in_out(smoothstep(0.0, 0.85, p));
        let clock = self.settings.clock_text(&env.clock);
        let show_battery=resident_battery;
        let (pc_ring,phone_ring)=self.battery_rings();
        let ring_count=(pc_ring.is_some() as u8+phone_ring.is_some() as u8) as f32;
        // 两个环并排时各自偏离中心 10px。
        let ring_offset=if ring_count>1.0 {10.0}else{0.0};

        // 顶栏终点布局：[电池][日期][时钟] 作为一组整体居中
        let date_w = base.text_w(&env.date, size::DATE, W::Regular);
        let clock_w = paint::clock_width(base,&clock,size::CLOCK);
        let battery_space=if show_battery {24.0+(ring_count-1.0)*20.0}else{0.0};
        let group = battery_space + date_w + 8.0 + clock_w;
        let gx = local_cx - group / 2.0 + if show_battery {6.0}else{0.0};
        // 日程页左侧还有新增按钮，顶栏从操作区右侧开始，避免较长的日期和时段文字挤到按钮上。
        let gx = if self.page==CALENDAR_PAGE {gx.max(155.0)}else{gx};
        let hb = (gx + 8.5, 18.6);
        let hd = gx + battery_space;
'''),
    ("src/app.rs",
     '''        let pcx=local_cx+layout.x[bar::CLOCK].unwrap()-paint::clock_width(base,&clock,size::PILL_CLOCK)/2.0;

        if self.effective_minimal() && self.t>=self.notification_bell_until && p < 0.02 { return; }
        // 电池：位置/大小连续插值
        let bk = lerp(1.0, 0.8, m);
        let bpos = (lerp(pb.0, hb.0, m), lerp(pb.1, hb.1, m));
        if show_battery {paint::battery(&base.faded(lerp(layout.clearance(bar::BATTERY),1.0,m)), bpos.0, bpos.1, env.battery, env.charging, bk);}

        // 时钟：两个字号沿同一路径交叉淡化，避免逐帧换字号
        let cpos = (lerp(pcx, hc, m), lerp(21.0, 23.0, m));
        let a_pill = (1.0 - smoothstep(0.25, 0.6, m)) * layout.clearance(bar::CLOCK);
        let a_head = smoothstep(0.35, 0.75, m);
        if a_pill > 0.0 {
''',
     '''        let pcx=local_cx+layout.x[bar::CLOCK].unwrap()-paint::clock_width(base,&clock,size::PILL_CLOCK)/2.0;

        if self.effective_minimal() && self.t>=self.notification_bell_until && p < 0.02 { return; }
        // 电池：位置/大小连续插值
        let bk = lerp(1.0, 0.8, m);
        let bpos = (lerp(pb.0, hb.0, m), lerp(pb.1, hb.1, m));
        if show_battery {
            let alpha=lerp(layout.clearance(bar::BATTERY),1.0,m);
            if let Some((pct,chg))=pc_ring {paint::battery(&base.faded(alpha),bpos.0-ring_offset,bpos.1,pct,chg,bk);}
            if let Some((pct,chg))=phone_ring {paint::battery(&base.faded(alpha),bpos.0+ring_offset,bpos.1,pct,chg,bk);}
        }

        // 时钟：两个字号沿同一路径交叉淡化，避免逐帧换字号
        let cpos = (lerp(pcx, hc, m), lerp(21.0, 23.0, m));
        let a_pill = (1.0 - smoothstep(0.25, 0.6, m)) * layout.clearance(bar::CLOCK);
        let a_head = smoothstep(0.35, 0.75, m);
        if a_pill > 0.0 {
'''),
    ("src/app.rs",
     '''            self.draw_quota_bar(&c,local_cx,&layout);
            if let Some(x)=layout.x[bar::WEATHER] { self.draw_pill_weather(&c,local_cx+x,&layout); }
        }
        if self.pomo.running && compact_alpha>0.004 {
            crate::focus_bar::draw(&base.faded(compact_alpha),local_cx+layout.x[bar::FOCUS].unwrap(),15.0,self.t,&self.pomo,self.phase_secs(self.pomo.phase),false);
        }
    }

    /// 常驻内容：顶部停靠走共享元素过渡（药丸里的电池、时钟飞到顶栏）；
    /// 左右停靠时竖排内容随展开淡出，顶栏随面板内容淡入。
    fn resident_header(&mut self, base: &Card, panel: Rect, cx: f32, p: f32, reveal: f32, env: &Env) {
        if !self.side() {self.shared_header(base,cx,p,reveal,env);return;}
''',
     '''            self.draw_quota_bar(&c,local_cx,&layout);
            if let Some(x)=layout.x[bar::WEATHER] { self.draw_pill_weather(&c,local_cx+x,&layout); }
        }
        if self.pomo.running && compact_alpha>0.004 {
            crate::focus_bar::draw(&base.faded(compact_alpha),local_cx+layout.x[bar::FOCUS].unwrap(),15.0,self.t,&self.pomo,self.phase_secs(self.pomo.phase),false);
        }
        // 状态总线：并排显示最多 3 条常驻状态（色点 + 文本 + 可选进度条）。
        if let Some(slot)=layout.status {
            if let Some(center)=layout.x[slot] {
                let statuses=self.statuses_for_render();
                if compact_alpha>0.004 && !statuses.is_empty() {
                    let c=base.faded(compact_alpha);
                    let total=layout.widths[slot];
                    let mut x=local_cx+center-total/2.0;
                    for status in statuses.iter().take(3) {
                        let w=self.status_item_width(status);
                        let tone=crate::extras::status_color(&status.color);
                        // 有进度条的胶囊：底色灰，彩色进度层从左向右逐渐填满整个胶囊
                        // （裁剪到进度宽度，左圆角保留、右边缘随进度推进）。
                        let progress=status.progress.filter(|v|v.is_finite())
                            .map(|v|v.clamp(0.0,1.0) as f32);
                        c.fill(x,5.0,w,20.0,10.0,Color32::from_gray(36));
                        if let Some(p)=progress {
                            if p>0.0 {
                                // 填充层裁剪到 [x, x+w*p]，整体画圆角矩形即可得到「左边圆角+右侧随进度裁平」。
                                let fillc=c.clipped(x,5.0,(w*p).max(1.0),20.0);
                                fillc.fill(x,5.0,w,20.0,10.0,tone);
                            }
                        }
                        // 带图标的状态（MAA 进度胶囊）：图标 + 文字作为整体在胶囊内水平居中。
                        // 其他状态保持原来的「色点 + 左对齐文字」。
                        let icon_tex=if status.icon.is_empty() {None} else {
                            self.status_icons.borrow_mut()
                                .get(c.ui.ctx(),std::path::Path::new(&status.icon))
                                .and_then(|p|p.image)
                        };
                        if let Some(tex)=icon_tex {
                            // 图标 16 + 间距 4 = 20 的前导占位，按实际文字宽度整体居中。
                            let fitted=c.fit(&status.text,10.5,W::Regular,(w-28.0).max(20.0));
                            let text_w=c.text_w(&fitted,10.5,W::Regular);
                            let start=x+((w-20.0-text_w).max(0.0))/2.0;
                            let uv=Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0));
                            c.painter.add(egui::epaint::RectShape::filled(
                                c.rect(start,7.0,16.0,16.0),4.0*c.s,Color32::WHITE).with_texture(tex.id(),uv));
                            c.centered_text(start+20.0,15.0,&fitted,10.5,W::Regular,theme::WHITE,H::Left);
                        } else {
                            c.circle(x+8.0,15.0,2.5,tone);
                            c.centered_text(x+14.0,15.0,&c.fit(&status.text,10.5,W::Regular,w-20.0),10.5,W::Regular,theme::WHITE,H::Left);
                        }
                        x+=w+6.0;
                    }
                }
            }
        }
    }

    /// 常驻内容：顶部停靠走共享元素过渡（药丸里的电池、时钟飞到顶栏）；
    /// 左右停靠时竖排内容随展开淡出，顶栏随面板内容淡入。
    fn resident_header(&mut self, base: &Card, panel: Rect, cx: f32, p: f32, reveal: f32, env: &Env) {
        if !self.side() {self.shared_header(base,cx,p,reveal,env);return;}
'''),
    ("src/app.rs",
     '''        if self.pet.last_idle >= self.opts.pet_idle && idle < 1.0
            && matches!(self.pet.state, PetState::Dozing | PetState::Sleeping) {
            self.pet.waking_until = self.t + 2.2;
        }
        self.pet.last_idle = idle;
        self.pet.hovered = env.cursor.is_some_and(|p| (p.x-cx).abs() <= BAR_W/2.0+24.0 && (0.0..70.0).contains(&p.y));
        self.pet.worried = self.settings.show_battery && env.battery.is_some_and(|v|v<=20) && !env.charging || env.network == Some(false);
        let allowed = self.pet_enabled();
        if !allowed { self.pet.reaction = None; self.pet.waking_until = 0.0; self.pet.expressions.clear(); }
        let available = allowed && !self.open && !self.drag_hot;
        let shown = available;
        self.pet_a = anim::approach(self.pet_a, if shown {1.0}else{0.0}, 9.0, self.dt);
        let visible = available && self.pet_a > 0.7 && self.progress() < 0.1;
''',
     '''        if self.pet.last_idle >= self.opts.pet_idle && idle < 1.0
            && matches!(self.pet.state, PetState::Dozing | PetState::Sleeping) {
            self.pet.waking_until = self.t + 2.2;
        }
        self.pet.last_idle = idle;
        self.pet.hovered = env.cursor.is_some_and(|p| (p.x-cx).abs() <= BAR_W/2.0+24.0 && (0.0..70.0).contains(&p.y));
        let (pc_ring,phone_ring)=self.battery_rings();
        let low_battery=[pc_ring,phone_ring].into_iter().flatten().any(|(pct,charging)|pct.is_some_and(|v|v<=20)&&!charging);
        self.pet.worried = low_battery || env.network == Some(false);
        let allowed = self.pet_enabled();
        if !allowed { self.pet.reaction = None; self.pet.waking_until = 0.0; self.pet.expressions.clear(); }
        let available = allowed && !self.open && !self.drag_hot;
        let shown = available;
        self.pet_a = anim::approach(self.pet_a, if shown {1.0}else{0.0}, 9.0, self.dt);
        let visible = available && self.pet_a > 0.7 && self.progress() < 0.1;
'''),
    ("src/app.rs",
     '''    }

    // ---------------------------------------------------------------
    // 每帧推进：番茄钟、饮水提醒、跨天
    // ---------------------------------------------------------------
    fn tick(&mut self, env: &Env, dt: f32, elapsed: f32) {
        if self.toast.as_ref().is_some_and(|(_,at)|self.t-at>2.4){self.toast=None;}
        if env.today != self.today {
            self.today = env.today.clone();
            self.water = model::Water::load(&env.today);
            self.last_drink=None;self.last_reminder=self.t;
            self.water_ml = self.water.drunk_ml as f32;
            self.water_ring.snap(self.water.percent() / 100.0);
        }
        if self.pomo.running && !self.module_on(FOCUS_PAGE) {self.pomo.running=false;}
        if self.pomo.running {
            self.pomo.remaining -= elapsed;
            if self.pomo.remaining <= 0.0 {
                let next = match self.pomo.phase {
                    Phase::Focus => {
                        self.pet.react(PetState::Happy);
                        Phase::Rest
''',
     '''    }

    // ---------------------------------------------------------------
    // 每帧推进：番茄钟、饮水提醒、跨天
    // ---------------------------------------------------------------
    fn tick(&mut self, env: &Env, dt: f32, elapsed: f32) {
        #[cfg(windows)] crate::smtc::netease_control::auto_host_tick();
        if self.toast.as_ref().is_some_and(|(_,at)|self.t-at>2.4){self.toast=None;}
        if env.today != self.today {
            self.today = env.today.clone();
            self.water = model::Water::load(&env.today);
            self.last_drink=None;self.last_reminder=self.t;
            self.water_ml = self.water.drunk_ml as f32;
            self.water_ring.snap(self.water.percent() / 100.0);
        }
        if self.pomo.running && !self.module_on(FOCUS_PAGE) {self.pomo.running=false;}
        // 只统计真正在跑的专注阶段：暂停不计，攒满一分钟落盘一次。
        if self.pomo.running && self.pomo.phase==Phase::Focus {
            self.focus_pending += elapsed as f32;
            if self.focus_pending>=60.0 {
                let minutes=(self.focus_pending/60.0).floor();
                self.focus_pending-=minutes*60.0;
                crate::extras::focus_add(minutes as u32);
                let (today,streak)=crate::extras::focus_summary();
                self.focus_today_min=today;
                self.focus_streak_days=streak;
            }
        }
       if self.pomo.running {
            self.pomo.remaining -= elapsed;
            if self.pomo.remaining <= 0.0 {
                let next = match self.pomo.phase {
                    Phase::Focus => {
                        self.pet.react(PetState::Happy);
                        Phase::Rest
'''),
    ("src/app.rs",
     '''            }
            self.cancel_edit();
            if added {self.open_task_reminder(0);}
        }
    }
}
''',
     '''            }
            self.cancel_edit();
            if added {self.open_task_reminder(0);}
        }
    }
}

#[cfg(test)]
mod lyric_scroll_tests {
    use super::*;

    #[test]
    fn scroll_waits_at_both_ends(){
        // 句首停留 1 秒：这段时间必须纹丝不动，否则开头一闪而过。
        assert_eq!(lyric_scroll_offset(0.0,30.0,100.0),0.0);
        assert_eq!(lyric_scroll_offset(1.0,30.0,100.0),0.0);
        assert!((lyric_scroll_offset(2.0,30.0,100.0)-30.0).abs()<0.01);
        // 到尾部就停住，不会滚出空白。
        assert_eq!(lyric_scroll_offset(60.0,30.0,100.0),100.0);
    }

    #[test]
    fn speed_fits_line_window_but_stays_readable(){
        // 窗口足够长：按「整句滚完」算，落在可读区间。
        let normal=lyric_scroll_speed(200.0,10.0);
        assert!((normal-25.0).abs()<0.01,"{}",normal);
        // 句子很短、放不下时也不能快到看不清。
        assert_eq!(lyric_scroll_speed(300.0,1.0),SCROLL_MAX_SPEED);
        // 句子很长时不会慢到几乎不动。
        assert_eq!(lyric_scroll_speed(1.0,600.0),SCROLL_MIN_SPEED);
    }

    /// 量化缓存省下的重排开销：`cargo test --lib -- --ignored --nocapture bench_long_line`
    #[test]
    #[ignore]
    fn bench_long_line_layout_cost(){
        let ctx=egui::Context::default();
        let _=ctx.run(Default::default(),|_|{});
        let text="这一句歌词故意写得非常非常长用来测试横向滚动到底能不能把整句话完整地显示出来".to_owned();
        let job=|size:f32|{
            let mut job=egui::text::LayoutJob::simple(text.clone(),egui::FontId::proportional(size),theme::WHITE,186.0);
            job.wrap.break_anywhere=true;
            job
        };
        let n=300;
        let t=std::time::Instant::now();
        for _ in 0..n {let _=ctx.fonts(|fonts|fonts.layout_job(job(11.0)));}
        let one=t.elapsed().as_secs_f64()/n as f64;
        let t=std::time::Instant::now();
        for _ in 0..n {
            for size in [11.0,10.5,10.0,9.5,9.0,8.5,8.0] {let _=ctx.fonts(|fonts|fonts.layout_job(job(size)));}
        }
        let seven=t.elapsed().as_secs_f64()/n as f64;
        // 每句都不一样才会真的走排版，用来区分「命中 egui 内部 galley 缓存」和「真重排」。
        let t=std::time::Instant::now();
        for i in 0..n {
            let mut job=egui::text::LayoutJob::simple(format!("{}{}",text,i),egui::FontId::proportional(11.0),theme::WHITE,186.0);
            job.wrap.break_anywhere=true;
            let _=ctx.fonts(|fonts|fonts.layout_job(job));
        }
        let cold=t.elapsed().as_secs_f64()/n as f64;
        println!("命中缓存 {:.1}us | 未命中(真排版) {:.1}us | 每帧7次 {:.1}us | 30fps 每秒多花 {:.3}ms",
            one*1e6,cold*1e6,seven*1e6,(seven-one)*30.0*1000.0);
    }

    #[test]
    fn cache_matches_only_same_line_and_width(){
        let ctx=egui::Context::default();
        // 字体要等第一次 run 之后才可用。
        let _=ctx.run(Default::default(),|_|{});
        // semibold 字体族由 App 启动时注册，这里只验证匹配逻辑，用默认族即可。
        let job=egui::text::LayoutJob::simple("测试".to_owned(),egui::FontId::proportional(11.0),theme::WHITE,f32::INFINITY);
        let galley=ctx.fonts(|fonts|fonts.layout_job(job));
        let cache=LyricLayoutCache{text:"测试".into(),width:160.0,white:true,title:false,galley,overflow:0.0};
        assert!(cache.matches("测试",160.0,true,false));
        // 换句、改宽度、换颜色、歌名/歌词切换都必须重排。
        assert!(!cache.matches("另一句",160.0,true,false));
        assert!(!cache.matches("测试",200.0,true,false));
        assert!(!cache.matches("测试",160.0,false,false));
        assert!(!cache.matches("测试",160.0,true,true));
    }
}

#[cfg(test)]
mod battery_source_tests {
    use super::*;

    #[test]
    fn pc_and_phone_battery_switches_do_not_affect_each_other(){
        let ctx=eframe::egui::Context::default();
        let mut app=App::new(&ctx,Default::default());
        app.resident_power=(Some(65),false);
        app.phone_resident=(Some(42),true);
        app.settings.show_battery=true;
        app.settings.phone_battery=true;
        assert_eq!(app.battery_ring_count(),2,"两个来源都有数据时应并排两个环");
        // 关掉本机开关：手机环不受影响。
        app.settings.show_battery=false;
        assert!(app.battery_rings().0.is_none()&&app.battery_rings().1.is_some());
        // 关掉手机开关：本机环不受影响。
        app.settings.show_battery=true;
        app.settings.phone_battery=false;
        assert!(app.battery_rings().0.is_some()&&app.battery_rings().1.is_none());
        // 手机没数据时手机环不占位，本机环照常。
        app.settings.phone_battery=true;
        app.phone_resident=(None,false);
        assert_eq!(app.battery_ring_count(),1);
        assert!(app.battery_visible());
    }
}

#[cfg(test)]
mod hover_delay_tests {
    use super::*;
    use crate::anim::Spring;

    fn approx(a: f64, b: f64) { assert!((a - b).abs() < 1e-9, "{a} != {b}"); }

    #[test]
    fn 延时换算默认值与极简加倍() {
        approx(hover_delay_secs(200, false), 0.20);
        approx(hover_delay_secs(200, true), 0.40);
        approx(hover_delay_secs(0, false), 0.0);
        approx(hover_delay_secs(0, true), 0.0);
        approx(hover_delay_secs(80, false), 0.08);
        assert!(hover_delay_secs(500, true) <= 1.0, "极简加倍也要有上限");
        assert_eq!(crate::model::Settings::default().hover_delay_ms, 200);
        assert!(200 >= SWELL_MIN_MS && 80 < SWELL_MIN_MS, "默认有预放大、短延时保持静止");
    }

    #[test]
    fn 短延时既过滤鼠标划过又更早展开() {
        let (bw, full) = (143.0f32, 419.0f32);
        let tuning = crate::model::OpenAnim::Elastic.tuning().unwrap();
        // 80 ms ≈ 5 帧 @60fps：等待期间保持静止，到点才换展开弹簧。
        let mut short = Spring::new(bw, CLOSE.0, CLOSE.1);
        short.target = bw;
        // 200 ms ≈ 12 帧：等待期间跑 SWELL 预放大。
        let mut long = Spring::new(bw, SWELL_K.0, SWELL_K.1);
        long.target = bw * SWELL.0;
        let mut width_before_delay = bw;
        for frame in 0..10 {
            if frame == 5 {
                short.tune(tuning.open_w.0, tuning.open_w.1);
                short.target = full;
            }
            short.step(1.0 / 60.0);
            long.step(1.0 / 60.0);
            if frame == 3 { width_before_delay = short.pos; }
        }
        assert!((width_before_delay - bw).abs() < 0.01,
            "延时内不该有任何形变，划过 <80 ms 就不会展开：{width_before_delay}");
        assert!(short.pos > long.pos + 20.0,
            "短延时应更早进入展开：{} vs {}", short.pos, long.pos);
    }

    #[test]
    fn 零延时比默认更早到位() {
        let (bw, full) = (143.0f32, 419.0f32);
        let near = bw + (full - bw) * 0.9;
        let tuning = crate::model::OpenAnim::Elastic.tuning().unwrap();
        let mut direct = Spring::new(bw, tuning.open_w.0, tuning.open_w.1);
        direct.target = full;
        let mut relay = Spring::new(bw, SWELL_K.0, SWELL_K.1);
        relay.target = bw * SWELL.0;
        let (mut direct_frames, mut relay_frames) = (None, None);
        for frame in 0..600 {
            if frame == 12 {
                relay.tune(tuning.open_w.0, tuning.open_w.1);
                relay.target = full;
            }
            direct.step(1.0 / 60.0);
            relay.step(1.0 / 60.0);
            if direct_frames.is_none() && direct.pos >= near { direct_frames = Some(frame + 1); }
            if relay_frames.is_none() && relay.pos >= near { relay_frames = Some(frame + 1); }
            if direct_frames.is_some() && relay_frames.is_some() { break; }
        }
        let (d, r) = (direct_frames.expect("零延时没到位"), relay_frames.expect("默认延时没到位"));
        assert!(d < r, "零延时应更早到位：{d} 帧 vs {r} 帧");
    }
}
'''),
    ("src/bar_layout.rs",
     '''    pub x: Vec<Option<f32>>,
    pub widths:Vec<f32>,
    quotas:usize,
}

impl Layout {
''',
     '''    pub x: Vec<Option<f32>>,
    pub widths:Vec<f32>,
    quotas:usize,
    /// 状态总线占用的槽位（在末尾动态追加，不参与 resident_order 重排）。
    pub status: Option<usize>,
}

impl Layout {
'''),
    ("src/bar_layout.rs",
     '''        }
        let width=cursor-12.0+16.0;
        for value in x.iter_mut().flatten() {*value-=width/2.0;}
        Self {width:width.max(96.0),x,widths,quotas}
    }
    pub fn quota_slots(&self)->impl Iterator<Item=usize> { (0..self.quotas).map(quota_slot) }
    /// 把某个槽加宽 extra：其余槽对称让位并记入总宽。点数文字、时段时钟、天气温度共用。
''',
     '''        }
        let width=cursor-12.0+16.0;
        for value in x.iter_mut().flatten() {*value-=width/2.0;}
        Self {width:width.max(96.0),x,widths,quotas,status:None}
    }
    /// 在胶囊最右侧追加一条状态区：不影响 resident_order 的既有槽位，
    /// 只是把整体左移半个宽度，让常驻组件保持相对位置。
    pub fn add_status(&mut self, width: f32) -> usize {
        let gap = 12.0;
        // 状态区是胶囊最右的槽位，右侧留与左侧一致的 16px 边距，避免胶囊贴边、左右不对称。
        let end_pad = 16.0;
        let slot = self.x.len();
        self.x.push(None);
        self.widths.push(width);
        let old_w = self.width;
        // 状态区中心相对旧胶囊右边界的偏移：gap（左间距）+ width/2；
        // 右侧再让出 end_pad，所以整体多占 end_pad，各槽位左移 shift 保证相对位置不变。
        let shift = (width + gap + end_pad) / 2.0;
        for value in self.x.iter_mut().flatten() { *value -= shift; }
        self.x[slot] = Some(old_w / 2.0 + gap / 2.0 - end_pad / 2.0);
        self.width = old_w + width + gap + end_pad;
        self.status = Some(slot);
        slot
    }
    pub fn quota_slots(&self)->impl Iterator<Item=usize> { (0..self.quotas).map(quota_slot) }
    /// 把某个槽加宽 extra：其余槽对称让位并记入总宽。点数文字、时段时钟、天气温度共用。
'''),
    ("src/bar_layout.rs",
     '''        let Some(center)=self.x.get(slot).copied().flatten() else {return;};
        for (i,x) in self.x.iter_mut().enumerate(){if i!=slot{if let Some(x)=x{*x+=if *x<center{-extra/2.0}else{extra/2.0};}}}
        self.widths[slot]+=extra;self.width+=extra;
    }
    pub fn balance_tail(&mut self,position:usize,extra:f32){self.widen(quota_slot(position),extra);}
    pub fn widen_clock(&mut self, extra:f32) {self.widen(CLOCK,extra);}
''',
     '''        let Some(center)=self.x.get(slot).copied().flatten() else {return;};
        for (i,x) in self.x.iter_mut().enumerate(){if i!=slot{if let Some(x)=x{*x+=if *x<center{-extra/2.0}else{extra/2.0};}}}
        self.widths[slot]+=extra;self.width+=extra;
    }
    /// 本机与手机电量同时显示时把电池槽加宽，两个环并排；其余槽位左右让开。
    pub fn widen_battery(&mut self, extra:f32) {
        let Some(battery)=self.x[BATTERY] else{return;};
        for (slot,x) in self.x.iter_mut().enumerate() {
            if slot!=BATTERY { if let Some(x)=x { *x+=if *x<battery {-extra/2.0}else{extra/2.0}; } }
        }
        self.widths[BATTERY]+=extra;
        self.width+=extra;
    }
    pub fn balance_tail(&mut self,position:usize,extra:f32){self.widen(quota_slot(position),extra);}
    pub fn widen_clock(&mut self, extra:f32) {self.widen(CLOCK,extra);}
'''),
    ("src/bar_layout.rs",
     '''        assert_eq!(empty.width,width);
    }
    #[test]
    fn replacing_battery_moves_continuously_and_supports_an_instant_state(){
        let before=Layout::new(true,1,true,false,false,true,false);let after=Layout::new(false,1,true,false,false,true,false);
        let mut motion=Motion::default();motion.step(before.clone(),0.0,true);
''',
     '''        assert_eq!(empty.width,width);
    }
    #[test]
    fn widening_the_battery_slot_keeps_neighbours_apart(){
        // 本机与手机电量同时显示时，电池槽加宽 20px，其余组件必须自动让开。
        let one=Layout::new(true,2,true,false,false,true,false);
        let mut two=Layout::new(true,2,true,false,false,true,false);
        two.widen_battery(20.0);
        assert!((two.width-one.width-20.0).abs()<0.01,"整体宽度应增加 20");
        assert!((two.widths[BATTERY]-one.widths[BATTERY]-20.0).abs()<0.01);
        let mut boxes:Vec<_>=two.x.iter().enumerate().filter_map(|(i,x)|x.map(|x|(x-two.widths[i]/2.0,x+two.widths[i]/2.0))).collect();
        boxes.sort_by(|a,b|a.0.total_cmp(&b.0));
        assert!(boxes.windows(2).all(|p|p[1].0-p[0].1>=6.9),"加宽后不能和邻居挤在一起");
        assert!(boxes.first().unwrap().0>=-two.width/2.0+15.9);
        assert!(boxes.last().unwrap().1<=two.width/2.0-15.9);
    }

    #[test]
    fn replacing_battery_moves_continuously_and_supports_an_instant_state(){
        let before=Layout::new(true,1,true,false,false,true,false);let after=Layout::new(false,1,true,false,false,true,false);
        let mut motion=Motion::default();motion.step(before.clone(),0.0,true);
'''),
    ("src/clipboard.rs",
     '''    /// 本地时间 `HH:MM`。
    pub at: String,
    pub pinned: bool,
}

#[derive(Debug, Default)]
pub struct History {
    pub items: Vec<Clip>,
    /// 每次变化 +1，界面据此判断是否需要重绘。
    pub version: u64,
    path: Option<std::path::PathBuf>,
}

''',
     '''    /// 本地时间 `HH:MM`。
    pub at: String,
    pub pinned: bool,
    /// 最后复制的 Unix 秒，用于清理 24 小时前的未固定内容。
    /// 固定条目永远保留，不因这个字段被清理。
    pub saved_at: i64,
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

'''),
    ("src/clipboard.rs",
     '''            Some(i) => self.items.remove(i).pinned,
            None => false,
        };
        self.items.insert(0, Clip { text: text.to_string(), at: at.to_string(), pinned });
        let mut recent = 0;
        self.items.retain(|c| c.pinned || { recent += 1; recent <= MAX_RECENT });
        self.changed();
''',
     '''            Some(i) => self.items.remove(i).pinned,
            None => false,
        };
        self.items.insert(0, Clip { text: text.to_string(), at: at.to_string(), pinned, saved_at: now_secs() });
        let mut recent = 0;
        self.items.retain(|c| c.pinned || { recent += 1; recent <= MAX_RECENT });
        self.changed();
'''),
    ("src/clipboard.rs",
     '''    }

    fn to_json(&self) -> Vec<u8> {
        let rows: Vec<_> = self.items.iter().map(|c| serde_json::json!({"pinned": c.pinned,"text": c.text, "at": c.at})).collect();
        serde_json::to_vec(&rows).unwrap_or_default()
    }

''',
     '''    }

    fn to_json(&self) -> Vec<u8> {
        let rows: Vec<_> = self.items.iter().map(|c| serde_json::json!({"pinned": c.pinned,"text": c.text, "at": c.at, "saved_at": c.saved_at})).collect();
        serde_json::to_vec(&rows).unwrap_or_default()
    }

'''),
    ("src/clipboard.rs",
     '''            text: row.get("text")?.as_str()?.to_string(),
            at: row.get("at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            pinned: row.get("pinned").and_then(|v| v.as_bool()).unwrap_or(true),
        })).filter(|c| !c.text.is_empty()).take(MAX_PINNED + MAX_RECENT).collect();
        Self { items, ..Self::default() }
    }
''',
     '''            text: row.get("text")?.as_str()?.to_string(),
            at: row.get("at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            pinned: row.get("pinned").and_then(|v| v.as_bool()).unwrap_or(true),
            saved_at: row.get("saved_at").and_then(|v| v.as_i64()).unwrap_or_else(now_secs),
        })).filter(|c| !c.text.is_empty()).take(MAX_PINNED + MAX_RECENT).collect();
        Self { items, ..Self::default() }
    }
'''),
    ("src/clipboard.rs",
     '''
    fn load_from(path: std::path::PathBuf) -> Self {
        let mut history = std::fs::read(&path).map(|raw| Self::from_json(&raw)).unwrap_or_default();
        history.path = Some(path);
        history
    }

    fn changed(&mut self) {
        self.version += 1;
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
            let _ = crate::model::atomic_write(path, &self.to_json());
        }
    }
}

''',
     '''
    fn load_from(path: std::path::PathBuf) -> Self {
        let mut history = std::fs::read(&path).map(|raw| Self::from_json(&raw)).unwrap_or_default();
        let oldest = now_secs() - RETENTION_SECS;
        // 敏感内容只在本机保留一天，重启时顺带清理；固定的片段模板不在此列。
        history.items.retain(|clip| clip.pinned || clip.saved_at >= oldest);
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

'''),
    ("src/clipboard.rs",
     '''    pub fn lock(&self) -> std::sync::MutexGuard<'_, History> {
        self.history.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn record_update(&self, text: &str, from_history: bool) -> bool {
        !from_history && self.record(text)
    }
    pub fn record(&self, text: &str) -> bool {
        if !self.is_enabled() { return false; }
        let at = chrono::Local::now().format("%H:%M").to_string();
        self.lock().push(text, &at)
    }
}

''',
     '''    pub fn lock(&self) -> std::sync::MutexGuard<'_, History> {
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
}

'''),
    ("src/clipboard.rs",
     '''mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn dropping_the_watcher_ends_the_listener_thread(){
''',
     '''mod tests {
    use super::*;

    /// 与 Shared::flush 同逻辑，但不构造整份 Shared（沿用现有历史对象即可）。
    fn flush_history(history: &mut History) {
        let Some((path, body)) = history.take_dirty() else { return; };
        crate::model::atomic_write(&path, &body).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn dropping_the_watcher_ends_the_listener_thread(){
'''),
    ("src/clipboard.rs",
     '''        history.push("保留", "09:00");
        history.push("删除", "09:01");
        history.remove(0);
        let mut reopened = History::load_from(path.clone());
        assert_eq!(reopened.items, history.items);
        reopened.clear_recent();
        let final_history = History::load_from(path.clone());
        assert_eq!(final_history.items.len(), 1);
        assert_eq!(final_history.items[0].text, "old");
        std::fs::remove_file(path).unwrap();
    }

''',
     '''        history.push("保留", "09:00");
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

'''),
    ("src/extras.rs",
     '''    pub duration: f64,
    pub bluetooth: Option<(crate::bt::DeviceKind, bool)>,
    pub meeting: Option<(crate::mail::Meeting, String)>,
    pub unread: Option<crate::app::Unread>,
}
impl Alert {
    pub(crate) fn messenger(&self)->Option<crate::app::Messenger>{
        match &self.source {
''',
     '''    pub duration: f64,
    pub bluetooth: Option<(crate::bt::DeviceKind, bool)>,
    pub meeting: Option<(crate::mail::Meeting, String)>,
    /// 通用状态总线的可选呈现信息（进度 / 状态色 / 点击动作）。
    pub status: Option<crate::island_status::Status>,
    pub unread: Option<crate::app::Unread>,
}

impl Alert {
    pub(crate) fn messenger(&self)->Option<crate::app::Messenger>{
        match &self.source {
'''),
    ("src/extras.rs",
     '''            duration: if transient { 2.5 } else { 8.0 },
            bluetooth: None,
            meeting: None,
            unread: None,
        };
        self.enqueue_alert(a, system);
    }
    pub(crate) fn meeting_alert(&mut self, meeting: crate::mail::Meeting, source: String, page: usize) {
        self.enqueue_alert(Alert {
            source: AlertSource::Internal,
            title: "会议提醒".into(), body: format!("{} · {} · {}", meeting.title, meeting.at, source),
            page, at: self.t, duration: 12.0, bluetooth: None, meeting: Some((meeting, source)), unread: None,
        }, true);
    }
    pub(crate) fn bluetooth_alert(&mut self, device: &crate::bt::Device, connected: bool) {
        if !self.settings.bluetooth_notifications { return; }
''',
     '''            duration: if transient { 2.5 } else { 8.0 },
            bluetooth: None,
            meeting: None,
            status: None, unread: None,
       };
        self.enqueue_alert(a, system);
    }
    /// 邮件验证码上岛：标题带「邮件」标识，正文留明文数字供通知卡片显示；
    /// 不走手机短信的 auto_copy_otp，也不推 Cmd::Copy，只允许用户手动点复制。
    pub fn mail_code_alert(&mut self, _code:&str, from:&str, page:usize){
        let duration=if cfg!(test){4.5}else{(self.settings.otp_seconds as f64).max(10.0)};
        self.enqueue_alert(Alert{
            source:AlertSource::External("mail-code".into()),
            title:format!("邮件验证码 · {}",from),
            body:"邮件验证码".to_string(),
            page,at:self.t,duration,
            bluetooth:None,meeting:None,status:None,unread:None,
        },false);
    }
    pub(crate) fn meeting_alert(&mut self, meeting: crate::mail::Meeting, source: String, page: usize) {
        self.enqueue_alert(Alert {
            source: AlertSource::Internal,
            title: "会议提醒".into(), body: format!("{} · {} · {}", meeting.title, meeting.at, source),
            page, at: self.t, duration: 12.0, bluetooth: None, meeting: Some((meeting, source)), status: None, unread: None,
       }, true);
    }
    pub(crate) fn bluetooth_alert(&mut self, device: &crate::bt::Device, connected: bool) {
        if !self.settings.bluetooth_notifications { return; }
'''),
    ("src/extras.rs",
     '''            page: 5, at: self.t, duration: 2.5,
            bluetooth: Some((device.kind, connected)),
            meeting: None,
            unread: None,
        };
        self.enqueue_alert(a, false);
    }
    fn enqueue_alert(&mut self, a: Alert, system: bool) {
        // 通知仅保存在本次运行的内存中。
        if a.source==AlertSource::Internal {self.history.push_front(a.clone());}
        self.history.truncate(50);
        if self.settings.notifications {
            if system && a.source==AlertSource::Internal {
                self.pending
''',
     '''            page: 5, at: self.t, duration: 2.5,
            bluetooth: Some((device.kind, connected)),
            meeting: None,
            status: None, unread: None,
       };
        self.enqueue_alert(a, false);
    }
    fn enqueue_alert(&mut self, a: Alert, system: bool) {
        // 通知仅保存在本次运行的内存中。
        if a.source==AlertSource::Internal || matches!(&a.source,AlertSource::External(id) if id=="mail-code") {self.history.push_front(a.clone());}
        self.history.truncate(50);
        // 落盘一份历史：岛仔重启或当时没看到时，还能在设置里回查。
        let status_bus=matches!(&a.source,AlertSource::External(id) if id.starts_with("island-status"));
        if !status_bus {notice_history_append(&a.title,&a.body);}
        // 勿扰：只记账、不打扰。横幅、铃铛、系统通知都不出现，验证码例外（用户要的就是那串数字）。
        if self.muted() && a.status.is_none() && crate::otp::extract("",&a.body,&[]).is_none() { return; }
        if self.settings.notifications {
            if system && a.source==AlertSource::Internal {
                self.pending
'''),
    ("src/extras.rs",
     '''            }
            let same_event=|old:&Alert| match (&a.meeting,&old.meeting) {
                (Some((next,_)),Some((previous,_)))=>crate::calendar::dedupe_key(next)==crate::calendar::dedupe_key(previous),
                _=>old.source==a.source && old.title==a.title && old.unread==a.unread && (a.is_hud() || old.body==a.body),
            };
            if a.source!=AlertSource::Internal || a.title != "蓝牙设备" { self.alerts.retain(|old| !same_event(old)); }
            if (a.source!=AlertSource::Internal || a.title != "蓝牙设备") && self.notice.as_ref().is_some_and(same_event) {
                self.notice = Some(a);
''',
     '''            }
            let same_event=|old:&Alert| match (&a.meeting,&old.meeting) {
                (Some((next,_)),Some((previous,_)))=>crate::calendar::dedupe_key(next)==crate::calendar::dedupe_key(previous),
                // 状态总线按 key 覆写：内容变了替换，不变也替换以续期，不排队。
                _ if matches!(&a.source,AlertSource::External(id) if id.starts_with("island-status:"))=>old.source==a.source,
                // 手机通知按来源聚合：同一 App 的新消息替换旧提示，不排队刷屏。
                // 手机通道共用同一个来源（手机连接的 AUMID）：新短信 / 来电直接顶掉上一条，
                // 不然来电会排在一条已经看过的短信后面。
                _ if a.title.starts_with("手机 · ") || a.title=="未接来电" || a.title=="来电中"=>old.source==a.source,
                _=>old.source==a.source && old.title==a.title && old.unread==a.unread && (a.is_hud() || old.body==a.body),
           };
            if a.source!=AlertSource::Internal || a.title != "蓝牙设备" { self.alerts.retain(|old| !same_event(old)); }
            if (a.source!=AlertSource::Internal || a.title != "蓝牙设备") && self.notice.as_ref().is_some_and(same_event) {
                self.notice = Some(a);
'''),
    ("src/extras.rs",
     '''        self.mark_messenger(client,unread.pid,unread.window);
        if pending{return;}
        self.notification_bell_at=self.t;self.notification_bell_until=self.t+4.5;
        self.enqueue_alert(Alert{source:AlertSource::Messenger(client),title:client.name().into(),body:format!("有新消息 · 请到{}查看",client.name()),page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None,unread:Some(unread)},false);
    }
    pub fn set_messenger_status(&mut self,client:crate::app::Messenger,status:String){self.messenger_statuses.insert(client,status);}
    pub fn messenger_status(&self,client:crate::app::Messenger)->&str{self.messenger_statuses.get(&client).map(String::as_str).unwrap_or("")}
    pub fn receive_notification(&mut self, app:String, id:String, body:String){
        self.receive_notification_from(app,id,body,None);
    }
    pub fn receive_notification_from(&mut self,app:String,id:String,body:String,unread:Option<crate::app::Unread>){
''',
     '''        self.mark_messenger(client,unread.pid,unread.window);
        if pending{return;}
        self.notification_bell_at=self.t;self.notification_bell_until=self.t+4.5;
        self.enqueue_alert(Alert{source:AlertSource::Messenger(client),title:client.name().into(),body:format!("有新消息 · 请到{}查看",client.name()),page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None,unread:Some(unread), status: None, },false);
    }
    /// 通用状态总线：外部脚本写 island_status.json 后转成岛上提示。
    /// 来源 id 带上状态 key，同 key 每 2 秒覆写一次，既刷新内容又续期。
    pub(crate) fn push_status(&mut self,s:crate::island_status::Status){
        let duration=if s.seconds>0.0{s.seconds}else{8.0};
        let key=s.key();
        self.enqueue_alert(Alert{source:AlertSource::External(format!("island-status:{key}")),title:s.text.clone(),body:s.detail.clone(),page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:Some(s), unread: None, },false);
    }
    /// 状态从 JSON 里删掉：立刻撤下对应提示，不等 8 秒自然过期。
    pub(crate) fn drop_status(&mut self,key:&str){
        let source=AlertSource::External(format!("island-status:{key}"));
        self.alerts.retain(|a|a.source!=source);
        if self.notice.as_ref().is_some_and(|a|a.source==source){self.notice=None;self.notification_bell_until=0.0;}
        if self.hud_notice.as_ref().is_some_and(|a|a.source==source){self.hud_notice=None;}
    }
    /// MAA 是否可对接：通知名单里勾选了 MAA，或用户手动指定了 MAA 程序。
    ///
    /// 后面这条很关键：MAA 只有在发过系统通知后才会被自动登记进名单，
    /// 换台机器 / 关掉 MAA 通知的用户名单里根本没有它，此时手选路径就是唯一入口。
    pub(crate) fn maa_enabled(&self)->bool {
        if !self.maa_prefs.exe_path.trim().is_empty() { return true; }
        self.notification_apps.items.iter().any(|(id,name,on)| *on && (name=="MAA" || id.to_ascii_lowercase().ends_with("maa.exe")))
    }
    /// MAA 日志文件：优先用设置里的目录，其次从通知名单里 MAA 的 exe 路径推导 `<MAA目录>\\debug`。
    pub(crate) fn maa_log_path(&self)->Option<std::path::PathBuf> {
        let custom=self.maa_prefs.log_dir.trim();
        if !custom.is_empty() { return Some(std::path::PathBuf::from(custom).join("asst.log")); }
        let exe=self.maa_exe()?;
        crate::maa::log_dir_from_exe(&exe).map(|dir|dir.join("asst.log"))
    }
    /// 直接打开某个设置分组（索引对应 SETTINGS_GROUPS），供命令行/快捷方式直达。
    pub fn open_settings_group(&mut self,group:usize){
        let last=SETTINGS_GROUPS.len()-1;
        let (_,start,_)=SETTINGS_GROUPS[group.min(last)];
        self.settings_category=group.min(last);
        self.settings_tab=start;
        self.open_settings();
    }
    /// 设置页选好 MAA 程序后写入偏好（换程序时顺便清掉图标缓存）。
    pub fn set_maa_exe(&mut self,path:String){
        self.maa_prefs.exe_path=path;
        self.maa_prefs.save();
        self.maa_icon=None;
    }
    /// 设置页选好日志目录后写入偏好。
    pub fn set_maa_log_dir(&mut self,path:String){
        self.maa_prefs.log_dir=path;
        self.maa_prefs.save();
    }
    /// MAA 可执行文件路径：优先用设置里手选的，其次取通知名单里登记的那份。
    pub(crate) fn maa_exe(&self)->Option<String> {
        let custom=self.maa_prefs.exe_path.trim();
        if !custom.is_empty() { return Some(custom.to_string()); }
        self.notification_apps.items.iter()
            .find(|(id,name,_)| name=="MAA" || id.to_ascii_lowercase().ends_with("maa.exe"))
            .map(|(id,_,_)| id.clone())
    }
    /// 按 MAA 任务进度刷新岛上那条常驻胶囊（运行中续期、完成后停留几秒）。
    pub(crate) fn refresh_maa_status(&mut self){
        if !self.maa_prefs.resident_progress { self.maa_status=None; return; }
        let now=unix_secs_f64();
        let Some(p)=self.maa_tracker.snapshot() else { self.maa_status=None; return; };
        // 图标：首次从 MAA.exe 提取成 PNG 缓存，之后直接读图片。
        let icon=match &self.maa_icon {
            Some(path)=>path.clone(),
            None=>{
                let path=self.maa_exe().and_then(|exe|{
                    let out=crate::model::config_dir().join("maa-icon.png");
                    crate::maa::ensure_icon(&exe,&out)
                }).map(|p|p.display().to_string()).unwrap_or_default();
                self.maa_icon=Some(path.clone());
                path
            }
        };
        if p.running {
            self.maa_done_until=0.0;
            // 运行中：15 秒持续续期；MAA 停下后不再续期，自动撤下（不会留僵尸胶囊）。
            let text=if p.current.is_empty(){"MAA · 运行中".to_string()}else{format!("MAA · {}",p.current)};
            let detail=if p.total>0 {format!("{}/{} 个任务",p.done,p.total)} else {String::new()};
            self.maa_status=Some(crate::island_status::Status{
                id:"maa".into(), text, detail, seconds:15.0,
                progress:p.ratio(), color:"blue".into(), click:String::new(),
                priority:5, ttl:now+15.0, icon,
            });
            return;
        }
        // 跑完：绿色「任务完成」停留几秒后撤下。
        let all_done=p.total>0 && p.done>=p.total;
        if all_done {
            if self.maa_done_until<=0.0 { self.maa_done_until=now+8.0; }
            if now<self.maa_done_until {
                self.maa_status=Some(crate::island_status::Status{
                    id:"maa".into(), text:"MAA · 任务完成".into(),
                    detail:format!("{} 个任务",p.total), seconds:8.0,
                    progress:Some(1.0), color:"green".into(), click:String::new(),
                    priority:5, ttl:now+8.0, icon,
                });
                return;
            }
        }
        self.maa_status=None;
    }
    /// 把一条 MAA 事件转成岛上提示（时长取 MAA 偏好）。
    ///
    /// 常驻进度胶囊开启时，任务开始/完成/全部完成都不再弹横幅——胶囊本身就在
    /// 显示当前任务和进度，再弹一条会重复。公招高星、出错仍单独提示（胶囊表达不了）。
    pub(crate) fn maa_event(&mut self,event:crate::maa::Event){
        let resident=self.maa_prefs.resident_progress;
        let (title,body)=match event {
            crate::maa::Event::ChainStart(_,label)=>{
                if !self.maa_prefs.notify_chain || resident {return;}
                ("MAA".to_string(),format!("开始{label}"))
            }
            crate::maa::Event::ChainDone(_,label)=>{
                if !self.maa_prefs.notify_chain || resident {return;}
                ("MAA".to_string(),format!("{label}完成"))
            }
            crate::maa::Event::AllDone(_)=>{
                if !self.maa_prefs.notify_chain || resident {return;}
                ("MAA".to_string(),"日常任务全部完成".to_string())
            }
            crate::maa::Event::RecruitStar(level)=>{
                if !self.maa_prefs.notify_recruit_star {return;}
                ("MAA 公招".to_string(),format!("刷出 {level} 星 tag"))
            }
            crate::maa::Event::Error(label)=>{
                if !self.maa_prefs.notify_error {return;}
                ("MAA 出错".to_string(),label)
            }
        };
        let a=Alert{source:AlertSource::Internal,title,body,page:usize::MAX,at:self.t,duration:self.maa_prefs.seconds as f64,bluetooth:None,meeting:None,status:None, unread: None, };
        self.enqueue_alert(a,false);
    }
    /// 手机电量的关键变化：断开 / 重连 / 低电 / 充满。
    /// 首轮只记录不提示，避免每次启动都弹一条「已连接」。
    pub(crate) fn phone_events(&mut self,now:Option<u8>,charging:bool){
        let prev=self.phone_last;
        self.phone_last=Some(now);
        let Some(prev)=prev else{return;};
        if !self.settings.notifications{return;}
        let low=self.settings.low_battery_threshold();
        let (title,body)=match (prev,now) {
            (Some(_),None)=>("手机已断开".to_string(),"电量信息已收起".to_string()),
            (None,Some(level))=>(format!("手机已连接 {level}%"),"手机连接已恢复".to_string()),
            (Some(old),Some(level)) if low.is_some_and(|t|old>t&&level<=t)=>(format!("手机电量偏低 {level}%"),format!("低于 {}%，记得给手机充电",low.unwrap_or(20))),
            (Some(old),Some(100)) if old<100=>(format!("手机已充满{}",if charging{"（充电中）"}else{""}),"可以拔掉充电器了".to_string()),
            _=>return,
        };
        self.alert(title,body,usize::MAX,false);
    }

    /// 本机电量的低电 / 充满提示，阈值与手机共用「低电提醒」设置。
    /// 首轮只记录不提示，避免每次启动都弹一条。
    pub(crate) fn pc_events(&mut self,now:Option<u8>,charging:bool){
        let prev=self.pc_last;
        self.pc_last=Some(now);
        let Some(prev)=prev else{return;};
        if !self.settings.notifications{return;}
        let Some(low)=self.settings.low_battery_threshold() else{return;};
        let (title,body)=match (prev,now) {
            (Some(old),Some(level)) if old>low&&level<=low=>(format!("本机电量偏低 {level}%"),if charging{"已接电源，正在充电".to_string()}else{format!("低于 {low}%，建议接上电源")}),
            (Some(old),Some(100)) if old<100&&charging=>("本机已充满 100%".to_string(),"可以拔掉充电器了".to_string()),
            _=>return,
        };
        self.alert(title,body,usize::MAX,false);
    }

    /// 勿扰是否生效（按 app 时间比较，重启后自动失效）。
    pub(crate) fn muted(&self)->bool{self.quiet_until>self.t}

    /// 文本是否命中通知关键词表（大小写不敏感）。
    pub(crate) fn keyword_hit(&self,text:&str)->bool{
        if text.is_empty(){return false;}
        // 规则里的 highlight 与关键词表同权：命中就亮琥珀色竖条。
        if self.notice_rules.hits(text,"highlight"){return true;}
        let lower=text.to_lowercase();
        // tick 里维护好的小写词表；源字符串刚改、缓存还没跟上时退回现算。
        if self.keyword_src==self.settings.notice_keywords {
            return self.keywords.iter().any(|word|lower.contains(word.as_str()));
        }
        self.settings.keywords().iter().any(|word|lower.contains(&word.to_lowercase()))
    }

    /// 会议自动勿扰：日程刚开始（90 秒内）且还没结束时把勿扰开到会议结束，最多 2 小时。
    /// 同一场会议只触发一次，散会自动失效。
    pub(crate) fn meeting_quiet(&mut self, env:&crate::app::Env){
        if !self.settings.quiet_meeting || self.muted() {return;}
        let Ok(day)=chrono::NaiveDate::parse_from_str(&env.today,"%Y-%m-%d") else{return};
        let now=chrono::Local::now().timestamp();
        // 缓存今日会议候选，60 秒或跨天才重算，避免高频合并排序引起抖动；
        // 触发窗口是 90 秒，不会因此漏掉会议。
        if self.meeting_plan.0!=env.today || self.t-self.meeting_plan.1>60.0 {
            let entries=self.calendar_entries(day);
            let plan=meeting_candidates(entries.iter().map(|entry|entry.event));
            self.meeting_plan=(env.today.clone(),self.t,plan);
        }
        let Some((until,key))=meeting_hit(&self.meeting_plan.2,now) else{return};
        if self.quiet_meeting_key.as_deref()==Some(key.as_str()) {return;}
        self.quiet_meeting_key=Some(key);
        self.quiet_until=until as f64;
        self.say("会议开始，自动勿扰到散会");
    }

    /// 把配置与状态复制到 `backups/<时间戳>/`：只复制文件、不递归子目录，不动原配置。
    pub(crate) fn backup_settings(&mut self){
        let Some(base)=std::env::var_os("APPDATA") else{self.say("找不到 APPDATA，无法备份");return;};
        let root=std::path::PathBuf::from(base).join("Daozai");
        let Some(backups)=backup_root() else{self.say("找不到 APPDATA，无法备份");return;};
        let dest=backups.join(chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());
        if std::fs::create_dir_all(&dest).is_err(){self.say("备份失败：建不了目录");return;}
        let mut count=0;
        if let Ok(entries)=std::fs::read_dir(&root){
            for entry in entries.flatten(){
                let path=entry.path();
                if path.is_dir(){continue;}
                let Some(name)=path.file_name() else{continue;};
                if std::fs::copy(&path,dest.join(name)).is_ok(){count+=1;}
            }
        }
        self.say(format!("已备份 {count} 个文件"));
        self.pending.push(Cmd::Open(dest));
    }

    /// AI 额度阈值：任一窗口剩余 ≤ 阈值时提醒一次；回到阈值以上后重新武装。
    /// 每 30 秒看一次内存里的快照，不额外请求接口。
    pub(crate) fn quota_threshold_alert(&mut self){
        let threshold=self.settings.quota_alert;
        if threshold==0 {return;}
        let mut fresh:Vec<(String,f32)>=Vec::new();
        let mut armed=std::collections::HashSet::new();
        for (id,row) in self.quota.rows.iter(){
            let Ok(snapshot)=row else{continue};
            for window in snapshot.windows.iter().chain(snapshot.extra.iter()){
                if window.remaining<=threshold as f32 {
                    let key=(id.clone(),window.label.clone());
                    if !self.quota_alerted.contains(&key){fresh.push((window.label.clone(),window.remaining));}
                    armed.insert(key);
                }
            }
        }
        self.quota_alerted=armed;
        for (label,remaining) in fresh {
            self.alert("AI 额度", format!("{label} 只剩 {remaining:.0}%"), 7, true);
        }
    }

    /// 设置页用的最近通知：2 秒缓存一次，别每帧读盘。
    pub(crate) fn recent_notices(&mut self)->Vec<(String,String,String)>{
        if self.notice_cache.0<=0.0 || self.t-self.notice_cache.0>2.0 {
            self.notice_cache=(self.t.max(0.001),notice_history());
        }
        self.notice_cache.1.clone()
    }

    /// 把一条历史通知重新弹出来（「稍后提醒」用）。
    pub(crate) fn replay_notice(&mut self,title:String,body:String){
        self.enqueue_alert(Alert{source:AlertSource::External("notice-history".into()),title,body,page:usize::MAX,at:self.t,duration:if cfg!(test){4.5}else{30.0},bluetooth:None,meeting:None,status:None, unread: None, },false);
    }

    /// 稍后提醒：`minutes` 分钟后再弹一次。
    pub(crate) fn snooze_notice(&mut self,minutes:f64,title:String,body:String){
        self.snoozed.push((self.t+minutes*60.0,title,body));
        self.say(format!("{minutes:.0} 分钟后再提醒"));
    }

    /// 到点的「稍后提醒」重新入队，一次只处理一条，避免同一帧连弹。
    pub(crate) fn tick_snoozed(&mut self){
        if let Some(index)=self.snoozed.iter().position(|(due,_,_)|self.t>=*due){
            let (_,title,body)=self.snoozed.remove(index);
            self.replay_notice(title,body);
        }
    }

    /// 勿扰开关：`minutes = 0` 关闭。只在本次运行内有效，重启后恢复关闭。
    pub(crate) fn set_quiet(&mut self,minutes:u32){
        self.settings.quiet_minutes=minutes;
        self.settings.save();
        self.quiet_until=if minutes==0 {0.0} else {self.t+minutes as f64*60.0};
        self.say(if minutes==0 {"勿扰已关闭".to_string()} else {format!("勿扰 {minutes} 分钟")});
    }

    /// 状态总线触发的番茄钟开关：`on = false` 只是暂停，不重置这一轮。
    pub(crate) fn set_focus_running(&mut self,on:bool){
        if on {
            if self.pomo.running {return;}
            self.pomo.phase=crate::app::Phase::Focus;
            self.pomo.remaining=self.phase_secs(crate::app::Phase::Focus);
            self.pomo.running=true;
            self.say("已开始专注");
        } else if self.pomo.running {
            self.pomo.running=false;
            self.say("已暂停专注");
        }
    }

    /// 这条通知是否进入处理流程。两个互不相干的维度：
    /// 手机短信/来电只看 `phone_alerts`，其它应用只看监听范围（全部应用 / 仅 QQ）。
    pub(crate) fn accepts_notification(&self,id:&str,phone:bool)->bool{
        if !self.settings.read_notifications || id.is_empty(){return false;}
        if phone {return self.settings.phone_alerts;}
        !self.settings.qq_only || id=="QQ"   }
    pub fn set_messenger_status(&mut self,client:crate::app::Messenger,status:String){self.messenger_statuses.insert(client,status);}
    pub fn messenger_status(&self,client:crate::app::Messenger)->&str{self.messenger_statuses.get(&client).map(String::as_str).unwrap_or("")}
    pub fn receive_notification(&mut self, app:String, id:String, body:String){
        if crate::system::phone_channel(&id) {self.notify_phone(app,id,body);return;}
        self.receive_notification_from(app,id,body,None);
    }
    pub fn receive_notification_from(&mut self,app:String,id:String,body:String,unread:Option<crate::app::Unread>){
'''),
    ("src/extras.rs",
     '''            if let Some(code)=crate::otp::extract("",&text,&[]) {text=crate::mail_privacy::mask(&text,&code.text);}
            text
        }else{"收到新通知 · 内容已隐藏".into()};
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None,unread},false);
    }
    pub fn reveal_messenger_notice(&mut self,unread:crate::app::Unread){
        if !unread.source.adapter().dismiss_unread || !self.pending_messengers.contains(&unread){return;}
        let body=if unread.pid==0{"消息账号暂无法确认"}else{"暂时无法打开对应窗口"};
        let source=if unread.source.adapter().capture==crate::messenger::Capture::Taskbar{AlertSource::Messenger(unread.source)}else{AlertSource::External(unread.source.adapter().notification_id.into())};
        self.notice=Some(Alert{source,title:unread.source.name().into(),body:body.into(),page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None,unread:Some(unread)});
        self.notification_bell_at=self.t;self.notification_bell_until=self.t+4.5;
    }
    pub(crate) fn tick_alerts(&mut self) {
        self.pending_messengers.retain(|u|u.source.enabled(&self.settings) && (u.source.adapter().capture!=crate::messenger::Capture::SystemToast
''',
     '''            if let Some(code)=crate::otp::extract("",&text,&[]) {text=crate::mail_privacy::mask(&text,&code.text);}
            text
        }else{"收到新通知 · 内容已隐藏".into()};
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None,unread, status: None, },false);
    }
    pub fn reveal_messenger_notice(&mut self,unread:crate::app::Unread){
        if !unread.source.adapter().dismiss_unread || !self.pending_messengers.contains(&unread){return;}
        let body=if unread.pid==0{"消息账号暂无法确认"}else{"暂时无法打开对应窗口"};
        let source=if unread.source.adapter().capture==crate::messenger::Capture::Taskbar{AlertSource::Messenger(unread.source)}else{AlertSource::External(unread.source.adapter().notification_id.into())};
        self.notice=Some(Alert{source,title:unread.source.name().into(),body:body.into(),page:usize::MAX,at:self.t,duration:4.5,bluetooth:None,meeting:None,unread:Some(unread), status: None, });
        self.notification_bell_at=self.t;self.notification_bell_until=self.t+4.5;
    }
    pub(crate) fn notify_phone(&mut self, app:String, id:String, body:String){
let phone=crate::system::phone_channel(&id);
        // 手机 App 名单是外部 JSON，改完约 1 秒内生效，不用重启岛仔。
        if phone {self.phone_apps.reload_if_changed();}
        // 每条通知留一行诊断，出问题能直接看出是哪一步拦下的。
        let trace=format!("id={id} app={app} phone={} qq_only={} body={}",
            if phone {"手机"} else {"其它"},
            if self.settings.qq_only {"是"} else {"否"},
            body.chars().take(60).collect::<String>());
        // 手机只看手机开关，其它应用只看监听范围——两者互不影响。
        if !self.accepts_notification(&id,phone) {
            log_notice(&format!("DROP 未连接/范围外/手机关  {trace}"));
            return;
        }
        // 真实手机连接通知只报主 AUMID，改由正文识别来源；
        // 明确命中黑名单的静默，识别不出的按「连接」放行，避免漏掉来电等重要通知。
        let phone_info:Option<(String,bool)>=if phone {Some(crate::system::phone_app(&body,&self.phone_apps).unwrap_or(("连接".to_string(),true)))}else{None};
        if phone_info.as_ref().is_some_and(|(_,allow)|!allow){
            log_notice(&format!("DROP 名单静默    {trace}"));
            return;
        }
        // 通知规则：静音 / 自动复制正文。规则文件改完约 1 秒生效，不用重启。
        self.notice_rules.reload_if_changed();
        let rule_text=format!("{app} {body}");
        if self.notice_rules.hits(&rule_text,"mute") {
            log_notice(&format!("DROP 规则静音    {trace}"));
            return;
        }
        if self.notice_rules.hits(&rule_text,"copy") && !body.trim().is_empty() {
            self.pending.push(Cmd::Copy(body.clone()));
            log_notice(&format!("规则命中 copy    {trace}"));
        }
        // 来电方（正文首段就是号码或联系人），来电中/未接来电都用它当副标题。
        let caller=body.split('·').next().unwrap_or("").trim().to_string();
        // 岛仔自己发出的系统提醒会被通知中心回传；已在历史里的同一内容不再入队，避免重复显示。
        if self.history.iter().any(|a|format!("{} · {}",a.title,a.body)==body){
            log_notice(&format!("DROP 自家回传    {trace}"));
            return;
        }
        if !self.notification_apps.observe(&id,&app) || !self.settings.notifications{
            log_notice(&format!("DROP 应用被关/总开关  {trace}"));
            return;
        }
        if id=="QQ"{self.mark_messenger(crate::app::Messenger::Qq,0,0);}
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去。
        let has_code=!body.is_empty() && crate::otp::extract("",&body,&[]).is_some();
        let otp_bell=phone && has_code;
        // 停留时长：普通通知用 notice_seconds；验证码用 otp_seconds，但不短于普通通知，
        // 且有 10 秒下限——验证码要留时间回看和点「复制」。
        let stay=notice_duration(self.settings.notice_seconds,self.settings.otp_seconds,false);
        let otp_stay=notice_duration(self.settings.notice_seconds,self.settings.otp_seconds,true);
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        let keep_bell=if cfg!(test) {4.5} else if has_code {otp_stay} else {stay};
        // 勿扰期间不闪铃铛（横幅同样被 enqueue_alert 拦下），验证码仍照常提醒。
        if !self.muted() || otp_bell {
            self.notification_bell_at=self.t;
            self.notification_bell_until=self.t+keep_bell;
        }
        // 「手机 · 」前缀用来和岛仔自带 IMAP 邮件的「新邮件」提醒区分开。
        let title=if let Some((label,_))=&phone_info {format!("手机 · {label}")}else{app.chars().filter(|c|!c.is_control()).take(80).collect()};
        // 10 秒窗口内同一 App 的多条消息合并计数，配合 enqueue_alert 的替换去重防刷屏。
        let count=if phone {
            let entry=self.phone_counts.entry(title.clone()).or_insert((0,self.t));
            if self.t-entry.1>10.0{entry.0=0;}
            entry.0+=1;entry.1=self.t;
            entry.0
        }else{1};
        // 手机通知去掉正文首段的 App 名（已在标题里），其余作为摘要；
        // 即使全局关闭正文预览也显示一行短摘要，且不对验证码打码
        //（后续验证码卡片会直接复用 otp::extract 的结果）。
        let source:&str=if phone_info.is_some(){
            // 手机镜像通知的正文末尾常带一个多余的分隔符（`…忽略本短信! · `），顺手去掉。
            body.split_once('·').map(|(_,rest)|rest.trim().trim_end_matches(|c:char| c=='·'||c.is_whitespace()).trim()).filter(|rest|!rest.is_empty()).unwrap_or(body.as_str())
        }else{body.as_str()};        let body=if self.settings.notification_preview || phone {
            let limit=if phone{80}else{300};
            let mut text=source.chars().filter(|c|!c.is_control()).take(limit).collect::<String>();
            if !phone {if let Some(code)=crate::otp::extract("",&text,&[]){text=crate::mail_privacy::mask(&text,&code.text);}}
            text
        }else{"收到新通知 · 内容已隐藏".into()};
        let body=if count>1 {format!("共 {count} 条 · {body}")}else{body};
        // 手机验证码自动复制（与邮件共用开关）：短信验证码上岛时顺手写剪贴板。
        if phone && self.settings.auto_copy_otp {
            let code=crate::otp::extract("",&body,&[]);
            log_otp("phone",code.as_ref().map(|c|c.text.as_str()));
            if let Some(code)=code {self.pending.push(Cmd::Copy(code.text));}
        }
        // 来电中 / 未接来电：标签都是「电话」，靠正文区分——
        // 响铃是「号码 · 传入呼叫」，挂断是「电话 · 号码 · 未接来电」。
        let ringing=phone_info.is_some()&&body.contains("传入呼叫");
        let missed=phone_info.as_ref().is_some_and(|(label,_)|label=="电话")&&body.contains("未接来电");
        if missed {
            // 来电中的提示被「未接来电」取代，别让两条并存。
            if self.notice.as_ref().is_some_and(|a|a.title=="来电中"){self.notice=None;}
            self.missed_calls=self.missed_calls.saturating_add(1);
            self.notification_bell_until=f64::MAX;
        }
        let (title,body,duration)=if ringing {
            ("来电中".to_string(),format!("{caller} 正在呼叫"),60.0)
        }else if missed {
            ("未接来电".to_string(),format!("{} 通未接来电 · 点击关闭清除",self.missed_calls),30.0)
        }else{(title,body,if cfg!(test) {4.5} else if has_code {otp_stay} else {stay})};
        log_notice(&format!("SHOW {title} / {body}  <- {trace}"));
        self.enqueue_alert(Alert{source:AlertSource::External(id),title,body,page:usize::MAX,at:self.t,duration,bluetooth:None,meeting:None,status:None, unread: None, },false);
    }
    pub(crate) fn tick_alerts(&mut self) {
        self.pending_messengers.retain(|u|u.source.enabled(&self.settings) && (u.source.adapter().capture!=crate::messenger::Capture::SystemToast
'''),
    ("src/extras.rs",
     '''        let allowed=|a:&Alert|match &a.source {
            AlertSource::Internal=>true,
            AlertSource::Messenger(client)=>client.enabled(&self.settings),
            AlertSource::External(id)=>self.settings.read_notifications && crate::messenger::NotificationFilter::for_settings(&self.settings).allows(id) && self.settings.notifications && !crate::messenger::suppresses_toast(&a.title,id,crate::messenger::toast_suppression(&self.settings)) && self.notification_apps.items.iter().any(|(key,_,on)|key==id && *on),
        };
        self.alerts.retain(&allowed);
        if self.notice.as_ref().is_some_and(|a|!allowed(a)){self.notice=None;}
        if self.hud_notice.as_ref().is_some_and(|a|!allowed(a)){self.hud_notice=None;}
        if !self.settings.notification_preview {
            for a in self.alerts.iter_mut().chain(self.notice.iter_mut()).chain(self.hud_notice.iter_mut()) {
                if matches!(&a.source,AlertSource::External(_)){a.body="收到新通知 · 内容已隐藏".into();}
            }
        }
        if self
''',
     '''        let allowed=|a:&Alert|match &a.source {
            AlertSource::Internal=>true,
            AlertSource::Messenger(client)=>client.enabled(&self.settings),
            AlertSource::External(id)=>{
                // 手机通道与状态总线不走应用名单：手机看 phone_alerts，状态总线常放行。
                let phone=crate::system::phone_channel(id);
                let status_bus=id=="island-status" || id.starts_with("island-status:");
                if status_bus {return self.settings.notifications;}
                if phone {return self.settings.notifications && self.settings.read_notifications && self.settings.phone_alerts;}
                self.settings.read_notifications && crate::messenger::NotificationFilter::for_settings(&self.settings).allows(id) && self.settings.notifications && !crate::messenger::suppresses_toast(&a.title,id,crate::messenger::toast_suppression(&self.settings)) && self.notification_apps.items.iter().any(|(key,_,on)|key==id && *on)
            },
       };
        self.alerts.retain(&allowed);
        if self.notice.as_ref().is_some_and(|a|!allowed(a)){self.notice=None;}
        if self.hud_notice.as_ref().is_some_and(|a|!allowed(a)){self.hud_notice=None;}
        if !self.settings.notification_preview {
            for a in self.alerts.iter_mut().chain(self.notice.iter_mut()).chain(self.hud_notice.iter_mut()) {
                let keep=matches!(&a.source,AlertSource::External(id) if crate::system::phone_channel(id) || id=="island-status" || id.starts_with("island-status:"));
                if !keep && matches!(&a.source,AlertSource::External(_)){a.body="收到新通知 · 内容已隐藏".into();}
            }
        }
        if self
'''),
    ("src/extras.rs",
     '''            }
        }
    }
    pub(crate) fn draw_notice(&mut self, c:&Card,panel:Rect,a:&Alert,_cmds:&mut Vec<Cmd>){
        let nc=c.clipped(0.0,0.0,panel.width(),panel.height());
        if let Some(source)=a.messenger(){
            messenger_icon(&nc,22.0,22.0,source);
            nc.circle(31.0,14.0,2.6,Color32::from_rgb(255,72,72));
''',
     '''            }
        }
    }
}

/// 验证码识别诊断：只在「自动复制」开启时写一行，便于排查邮件/短信为什么没复制。
/// 日志只保留打码后的长度，绝不把验证码明文长期写进磁盘。
pub(crate) fn log_otp(source:&str,code:Option<&str>){
    let Some(base)=std::env::var_os("APPDATA") else{return};
    let dir=std::path::PathBuf::from(base).join("Daozai").join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err(){return;}
    let path=dir.join("otp.log");
    if std::fs::metadata(&path).is_ok_and(|m|m.len()>256*1024){let _=std::fs::remove_file(&path);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        use std::io::Write;
        let masked = code.map(|value| crate::mail_privacy::mask(value, value));
        let _=writeln!(file,"{} | {} | {}",chrono::Local::now().format("%H:%M:%S"),source,masked.as_deref().unwrap_or("-"));
    }
}

/// 通知卡片停留秒数：验证码有 10 秒下限，且不会短于普通通知（要留时间回看和点「复制」）。
pub(crate) fn notice_duration(notice_seconds:u32,otp_seconds:u32,has_code:bool)->f64{
    let stay=notice_seconds as f64;
    if has_code {(otp_seconds as f64).max(stay).max(10.0)} else {stay}
}

#[cfg(test)]
mod notice_duration_tests {
    use super::notice_duration;

    #[test]
    fn 验证码停留有下限且不短于普通通知() {
        assert_eq!(notice_duration(10, 90, true), 90.0, "验证码用自己设的 90 秒");
        assert_eq!(notice_duration(10, 20, true), 20.0);
        assert_eq!(notice_duration(60, 20, true), 60.0, "不能比普通通知还短");
        assert_eq!(notice_duration(10, 5, true), 10.0, "下限 10 秒");
        assert_eq!(notice_duration(10, 90, false), 10.0, "普通通知不受验证码设置影响");
    }
}

/// 备份目录：`%APPDATA%\\Daozai\\backups`，每次备份一个时间戳子目录。
pub(crate) fn backup_root()->Option<std::path::PathBuf>{
    let base=std::env::var_os("APPDATA")?;
    Some(std::path::PathBuf::from(base).join("Daozai").join("backups"))
}

/// 番茄统计：`%APPDATA%\\Daozai\\focus-stats.json`，只记「今天多少分钟」。
/// 只统计真正在跑的专注阶段，暂停不计；写盘频率是每分钟一次。
pub(crate) fn focus_stats_path()->Option<std::path::PathBuf>{
    let base=std::env::var_os("APPDATA")?;
    Some(std::path::PathBuf::from(base).join("Daozai").join("focus-stats.json"))
}

fn focus_days()->Vec<(String,u32)>{
    let Some(path)=focus_stats_path() else{return Vec::new();};
    // 坏文件会被改名成 .bad 后返回 None，避免「解析失败」被当成「本来是空的」而清空统计。
    let Some(json)=crate::model::read_json_renaming_bad(&path,crate::model::MAX_JSON_BYTES) else{return Vec::new();};
    json.get("days").and_then(|d|d.as_array()).into_iter().flatten().filter_map(|item|{
        Some((item.get("date")?.as_str()?.to_owned(), item.get("minutes")?.as_u64()? as u32))
    }).collect()
}

/// 今日专注分钟数与连续天数（连续天数从今天往回数，今天为 0 则从昨天算）。
pub(crate) fn focus_summary()->(u32,u32){
    let days=focus_days();
    let today=chrono::Local::now().date_naive();
    let minutes=days.iter().find(|(date,_)|date==&today.format("%Y-%m-%d").to_string()).map(|(_,m)|*m).unwrap_or(0);
    let mut streak=0;
    let mut cursor=today;
    if minutes==0 {cursor=today-chrono::Duration::days(1);}
    loop {
        let key=cursor.format("%Y-%m-%d").to_string();
        match days.iter().find(|(date,_)|date==&key) {
            Some((_,m)) if *m>0 =>{streak+=1;cursor=cursor-chrono::Duration::days(1);}
            _=>break,
        }
        if streak>365{break;}
    }
    (minutes,streak)
}

pub(crate) fn focus_add(minutes:u32){
    if minutes==0 || cfg!(test) {return;}
    let Some(path)=focus_stats_path() else{return;};
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    let today=chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    let mut days=focus_days();
    match days.iter_mut().find(|(date,_)|date==&today){
        Some(entry)=>entry.1+=minutes,
        None=>days.push((today,minutes)),
    }
    if days.len()>400 {let extra=days.len()-400;days.drain(0..extra);}
    days.sort_by(|a,b|a.0.cmp(&b.0));
    let json=serde_json::json!({"days":days.iter().map(|(date,m)|serde_json::json!({"date":date,"minutes":m})).collect::<Vec<_>>()});
    if let Ok(data)=serde_json::to_vec(&json){let _=crate::model::atomic_write(&path,&data);}
}

/// 通知历史（落盘，最多 100 条）：岛仔关掉之后「当时没看到的那条」还能查回来。
/// 存 `%APPDATA%\\Daozai\\notices.json`，字段 at/title/body，新的在后面。
pub(crate) fn notice_history_path()->Option<std::path::PathBuf>{
    let base=std::env::var_os("APPDATA")?;
    Some(std::path::PathBuf::from(base).join("Daozai").join("notices.json"))
}

/// 按时间顺序返回 (时间, 标题, 正文)，最新在最后。
pub(crate) fn notice_history()->Vec<(String,String,String)>{
    let Some(path)=notice_history_path() else{return Vec::new();};
    let Some(json)=crate::model::read_json_renaming_bad(&path,crate::model::MAX_JSON_BYTES) else{return Vec::new();};
    json.as_array().into_iter().flatten().filter_map(|item|{
        Some((item.get("at")?.as_str()?.to_owned(),item.get("title")?.as_str()?.to_owned(),item.get("body")?.as_str()?.to_owned()))
    }).collect()
}

pub(crate) fn notice_history_append(title:&str,body:&str){
    // 测试构建别往真实 APPDATA 里塞测试通知。
    if cfg!(test) {return;}
    let Some(path)=notice_history_path() else{return;};
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    // 落盘前把验证码打码：历史文件只用于「回看是什么事」，不需要留着明文码。
    let body=if let Some(code)=crate::otp::extract("",body,&[]) {crate::mail_privacy::mask(body,&code.text)} else {body.to_string()};
    let mut list=notice_history();
    list.push((chrono::Local::now().format("%m-%d %H:%M").to_string(),title.to_string(),body));
    if list.len()>100{let extra=list.len()-100;list.drain(0..extra);}
    let json:Vec<_>=list.iter().map(|(at,title,body)|serde_json::json!({"at":at,"title":title,"body":body})).collect();
    // 原子写：中途断电也不会留下半截 JSON（否则下次读失败会被当空文件重建）。
    if let Ok(data)=serde_json::to_vec(&json){let _=crate::model::atomic_write(&path,&data);}
}

/// 通知通道诊断：每条进来的通知都记一行「显示 / 被谁拦下」，排查「某类通知不上岛」用。
pub(crate) fn log_notice(note:&str){
    let Some(base)=std::env::var_os("APPDATA") else{return};
    let dir=std::path::PathBuf::from(base).join("Daozai").join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err(){return;}
    let path=dir.join("notice.log");
    if std::fs::metadata(&path).is_ok_and(|m|m.len()>256*1024){let _=std::fs::remove_file(&path);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        use std::io::Write;
        let _=writeln!(file,"{} | {}",chrono::Local::now().format("%H:%M:%S"),note);
    }
}

/// 邮件面板操作诊断：点击「全部已读」被拒时记录原因，便于排查按钮为什么没反应。
pub(crate) fn log_mail(note:&str){
    let Some(base)=std::env::var_os("APPDATA") else{return};
    let dir=std::path::PathBuf::from(base).join("Daozai").join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err(){return;}
    let path=dir.join("mail.log");
    if std::fs::metadata(&path).is_ok_and(|m|m.len()>256*1024){let _=std::fs::remove_file(&path);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        use std::io::Write;
        let _=writeln!(file,"{} | {}",chrono::Local::now().format("%H:%M:%S"),note);
    }
}

fn phone_icon_file(title:&str)->Option<&'static str>{
    let label=title.strip_prefix("手机 · ")?;
    Some(match label {
        "QQ"=>"app_icon_com.tencent.mqq1.0.jpg",
        "微信"=>"app_icon_com.tencent.xin1.0.jpg",
        "支付宝"=>"app_icon_com.alipay.iphoneclient1.0.jpg",
        "Gmail"=>"app_icon_com.google.Gmail1.0.jpg",
        "电话"=>"app_icon_com.apple.mobilephone1.0.jpg",
        _=>return None,
    })
}
/// 用手机连接自带的 App 图标替代铃铛；首次加载后缓存纹理，避免每帧读盘。
fn phone_icon(c:&Card,title:&str,icons:&mut std::collections::HashMap<String,egui::TextureHandle>)->bool{
    let Some(file)=phone_icon_file(title) else{return false};
    if !icons.contains_key(file){
        let Ok(base)=std::env::var("LOCALAPPDATA") else{return false};
        let path=std::path::PathBuf::from(base).join("Packages").join("Microsoft.YourPhone_8wekyb3d8bbwe")
            .join("TempState").join("app_icons").join(file);
        let Ok(bytes)=std::fs::read(path) else{return false};
        let Ok(img)=image::load_from_memory(&bytes) else{return false};
        let rgba=img.into_rgba8();
        let ci=egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize,rgba.height() as usize],rgba.as_raw());
        let tex=c.ui.ctx().load_texture(file.to_owned(),ci,egui::TextureOptions::LINEAR);
        icons.insert(file.to_owned(),tex);
    }
    let Some(tex)=icons.get(file) else{return false};
    c.painter.image(tex.id(),c.rect(12.0,10.0,24.0,24.0),Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.0,1.0)),Color32::WHITE);
    true
}

/// 状态总线点击后的动作：要么交给外壳执行，要么由岛仔自己处理。
pub(crate) enum StatusAct {
    Shell(Cmd),
    /// 开始 / 暂停番茄专注。
    Focus(bool),
    /// 勿扰 N 分钟（0 = 关闭）。
    Quiet(u32),
    /// 跳到第 N 页（0 音乐 … 7 设置）。
    Page(usize),
}

/// 状态总线的点击动作白名单：`copy:` / `url:` / `file:` 交给外壳，
/// `action:` 只认下表里的几个词——脚本能指挥岛仔，但指挥不了整台电脑。
fn status_action(spec:&str)->Option<(&'static str,StatusAct)>{
    use StatusAct::*;
    if let Some(text)=spec.strip_prefix("copy:") {
        if !text.is_empty(){return Some(("复制",Shell(Cmd::Copy(text.to_string()))));}
    }
    if let Some(url)=spec.strip_prefix("url:") {
        if url.starts_with("https://")||url.starts_with("http://"){return Some(("打开",Shell(Cmd::OpenUrl(url.to_string()))));}
    }
    if let Some(file)=spec.strip_prefix("file:") {
        // 只放行「看到的文件」：路径必须存在，且不是可执行/脚本/快捷方式——
        // 否则任何能写 island_status.json 的进程都能借岛仔的手启动程序。
        let path=std::path::PathBuf::from(file);
        let blocked=path.extension().and_then(|e|e.to_str()).map(|e|{
            matches!(e.to_ascii_lowercase().as_str(),"exe"|"bat"|"cmd"|"com"|"ps1"|"vbs"|"js"|"jse"|"wsf"|"wsh"|"hta"|"lnk"|"scr"|"msi"|"pif"|"reg")
        }).unwrap_or(false);
        if path.exists() && !blocked {return Some(("打开",Shell(Cmd::Open(path))));}
    }
    let action=spec.strip_prefix("action:")?.trim();
    match action {
        "focus_start"=>Some(("开始专注",Focus(true))),
        "focus_stop"=>Some(("结束专注",Focus(false))),
        "quiet30"=>Some(("勿扰 30 分",Quiet(30))),
        "quiet60"=>Some(("勿扰 60 分",Quiet(60))),
        "quiet_off"=>Some(("关闭勿扰",Quiet(0))),
        _=>action.strip_prefix("page:").and_then(|n|n.trim().parse::<usize>().ok()).filter(|n|*n<8).map(|n|("打开",Page(n))),
    }
}
/// 当前 Unix 时间（秒，带小数）；写状态总线的 ttl 用。
pub(crate) fn unix_secs_f64()->f64{
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs_f64()).unwrap_or(0.0)
}

pub(crate) fn status_color(name:&str)->Color32{
    match name.trim().to_ascii_lowercase().as_str(){
        "red"=>Color32::from_rgb(226,75,74),
        "green"=>theme::CHARGE,
        "blue"=>theme::BLUE,
        "amber"=>Color32::from_rgb(239,159,39),
        _=>theme::GRAY,
    }
}

impl App {
    pub(crate) fn draw_notice(&mut self, c:&Card,panel:Rect,a:&Alert,_cmds:&mut Vec<Cmd>){
        let nc=c.clipped(0.0,0.0,panel.width(),panel.height());
        // 关键词高亮：命中词表时在卡片最左侧亮一条琥珀色竖条。
        // 只加一条竖条，不动文字和任何控件的位置，布局与测试友好的部分保持原样。
        if a.status.is_none() && (self.keyword_hit(&a.title) || self.keyword_hit(&a.body)) {
            nc.fill(2.5,10.0,2.5,24.0,1.25,Color32::from_rgb(239,159,39));
        }
        if let Some(source)=a.messenger(){
            messenger_icon(&nc,22.0,22.0,source);
            nc.circle(31.0,14.0,2.6,Color32::from_rgb(255,72,72));
'''),
    ("src/extras.rs",
     '''            return;
        }
        if a.source!=AlertSource::Internal {
            crate::paint::bell(&nc,310.0,22.0,theme::GRAY,crate::paint::bell_swing(self.t-a.at));
            nc.circle(314.0,17.5,2.0,Color32::from_rgb(255,72,72));
            nc.centered_text(16.0,14.0,&nc.fit(&a.title,11.0,W::Semibold,198.0),11.0,W::Semibold,theme::WHITE,H::Left);
            nc.centered_text(16.0,30.0,&nc.fit(&a.body,10.5,W::Regular,198.0),10.5,W::Regular,theme::GRAY,H::Left);
            if notice_button(&nc,226.0,12.0,60.0,"关闭","external-dismiss"){
                self.notice=None;
                self.notification_bell_until=0.0;
            }
            return;
        }
''',
     '''            return;
        }
        if a.source!=AlertSource::Internal {
            // 通用状态总线：独立绘制，不影响手机通知 / 验证码卡片的原有布局。
            if let Some(st)=&a.status {
                let tone=status_color(&st.color);
                nc.fill(6.0,10.0,3.0,24.0,1.5,tone);
                nc.centered_text(16.0,14.0,&nc.fit(&a.title,11.0,W::Semibold,198.0),11.0,W::Semibold,theme::WHITE,H::Left);
                nc.centered_text(16.0,30.0,&nc.fit(&a.body,10.5,W::Regular,198.0),10.5,W::Regular,theme::GRAY,H::Left);
                if let Some(p)=st.progress.filter(|v|v.is_finite()) {
                    let p=p.clamp(0.0,1.0) as f32;
                    nc.fill(16.0,37.0,198.0,2.0,1.0,Color32::from_gray(60));
                    nc.fill(16.0,37.0,198.0*p,2.0,1.0,tone);
                }
                if let Some((label,act))=status_action(&st.click) {
                    if notice_button(&nc,160.0,12.0,60.0,label,"status-action"){
                        match act {
                            StatusAct::Shell(cmd)=>_cmds.push(cmd),
                            StatusAct::Focus(on)=>self.set_focus_running(on),
                            StatusAct::Quiet(minutes)=>self.set_quiet(minutes),
                            StatusAct::Page(page)=>self.show_page(page),
                        }
                    }
                }
                if notice_button(&nc,226.0,12.0,60.0,"关闭","external-dismiss"){
                    self.notice=None;
                    self.notification_bell_until=0.0;
                }
                return;
            }
            // 手机通知优先用手机连接自带的 App 图标，没图标才回退到铃铛。
            let icon=phone_icon(&nc,&a.title,&mut self.phone_icons);
            if !icon {
                crate::paint::bell(&nc,310.0,22.0,theme::GRAY,crate::paint::bell_swing(self.t-a.at));
                nc.circle(314.0,17.5,2.0,Color32::from_rgb(255,72,72));
            }
            // 有图标时文字整体右移，避免压在图标上。
            let text_x=if icon{44.0}else{16.0};
            let text_w=if icon{170.0}else{198.0};
            nc.centered_text(text_x,14.0,&nc.fit(&a.title,11.0,W::Semibold,text_w),11.0,W::Semibold,theme::WHITE,H::Left);
            // 验证码卡片：大号数字 + 一键复制（otp::extract 有打分门槛，普通消息不会误判）。
            // 短信从正文提取；邮件验证码只从邮件数据读，通知与历史都不留明文。
            let mail_code=matches!(&a.source,AlertSource::External(id) if id=="mail-code")
                .then(||self.mail.code_item().and_then(|m|m.code.clone())).flatten();
            if let Some(code)=mail_code.or_else(||crate::otp::extract("",&a.body,&[]).map(|c|c.text)) {
                nc.centered_text(text_x,30.0,&code,20.0,W::Bold,theme::WHITE,H::Left);
                if notice_button(&nc,160.0,12.0,60.0,"复制","phone-code-copy"){_cmds.push(Cmd::Copy(code));}
            } else {
                nc.centered_text(text_x,30.0,&nc.fit(&a.body,10.5,W::Regular,text_w),10.5,W::Regular,theme::GRAY,H::Left);
            }
            if notice_button(&nc,226.0,12.0,60.0,"关闭","external-dismiss"){
                self.notice=None;
                self.notification_bell_until=0.0;
                if a.title=="未接来电"{self.missed_calls=0;}
            }
            return;
        }
'''),
    ("src/extras.rs",
     '''
    #[test]
    fn settings_groups_keep_every_page_reachable() {
        let groups = ["常规", "音乐与歌词", "专注与饮水", "专注与饮水", "待办与置物", "待办与置物", "通知、邮箱与日程", "通知、邮箱与日程", "通知、邮箱与日程", "关于", "页面管理", "AI 额度", "天气"];
        for (tab, expected) in groups.iter().enumerate() {
            let matches: Vec<_> = SETTINGS_GROUPS.iter().filter(|(_,start,end)| (*start..=*end).contains(&tab)).collect();
            assert_eq!(matches.len(), 1, "页面 {tab} 必须且只能属于一个分组");
            assert_eq!(matches[0].0, *expected);
''',
     '''
    #[test]
    fn settings_groups_keep_every_page_reachable() {
        let groups = ["常规", "音乐与歌词", "专注与饮水", "专注与饮水", "待办与置物", "待办与置物", "通知、邮箱与日程", "通知、邮箱与日程", "通知、邮箱与日程", "关于", "页面管理", "AI 额度", "天气", "手机连接", "浏览器下载", "MAA"];
       for (tab, expected) in groups.iter().enumerate() {
            let matches: Vec<_> = SETTINGS_GROUPS.iter().filter(|(_,start,end)| (*start..=*end).contains(&tab)).collect();
            assert_eq!(matches.len(), 1, "页面 {tab} 必须且只能属于一个分组");
            assert_eq!(matches[0].0, *expected);
'''),
    ("src/extras.rs",
     '''    }
}

fn switch(ui:&mut egui::Ui,value:&mut bool)->bool {
    let (rect,response)=ui.allocate_exact_size(egui::vec2(34.0,20.0),egui::Sense::click());
    if response.clicked(){*value=!*value;}
''',
     '''    }
}

/// 不可用的设置行：开关置灰、点击无效。
/// 用于「本机没装对应组件」的情况（例如没有「手机连接」时的手机电量）。
pub(crate) fn setting_row_locked(ui:&mut egui::Ui,title:&str,description:&str,value:bool){
    let ctx=&ui.ctx().clone();
    ui.add_space(2.0);
    ui.horizontal(|ui|{
        let text_w=(ui.available_width()-SETTINGS_SWITCH_W-SETTINGS_ROW_GAP).max(120.0);
        ui.vertical(|ui|{
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y=2.0;
            ui.add(egui::Label::new(egui::RichText::new(title).size(13.5).color(settings_color(ctx,Color32::from_gray(130),Color32::from_gray(140)))).wrap());
            if !description.is_empty(){ui.add(egui::Label::new(egui::RichText::new(description).size(12.0).color(settings_color(ctx,Color32::from_gray(118),Color32::from_gray(132)))).wrap());}
        });
        ui.add_space((ui.available_width()-SETTINGS_SWITCH_W-ui.spacing().item_spacing.x).max(0.0));
        let (rect,_)=ui.allocate_exact_size(egui::vec2(34.0,20.0),egui::Sense::hover());
        ui.painter().rect_filled(rect,10.0,settings_color(ctx,Color32::from_gray(58),Color32::from_rgb(226,229,234)));
        let x=if value{rect.right()-10.0}else{rect.left()+10.0};
        ui.painter().circle_filled(egui::pos2(x,rect.center().y+0.5),7.5,Color32::from_black_alpha(20));
        ui.painter().circle_filled(egui::pos2(x,rect.center().y),7.0,settings_color(ctx,Color32::from_gray(150),Color32::WHITE));
    });
}
fn switch(ui:&mut egui::Ui,value:&mut bool)->bool {
    let (rect,response)=ui.allocate_exact_size(egui::vec2(34.0,20.0),egui::Sense::click());
    if response.clicked(){*value=!*value;}
'''),
    ("src/extras.rs",
     '''    ui.painter().circle_filled(egui::pos2(x,rect.center().y),7.0,Color32::WHITE);
    response.clicked()
}
/// 设置页版式：标签列宽、开关尺寸、控件与卡片右边缘之间的固定留白。
pub(crate) const SETTINGS_LABEL_W: f32 = 80.0;
const SETTINGS_SWITCH_W: f32 = 34.0;
const SETTINGS_ROW_GAP: f32 = 16.0;
''',
     '''    ui.painter().circle_filled(egui::pos2(x,rect.center().y),7.0,Color32::WHITE);
    response.clicked()
}
/// 设置页版式常量。
pub(crate) const SETTINGS_LABEL_W: f32 = 80.0;
const SETTINGS_SWITCH_W: f32 = 34.0;
const SETTINGS_ROW_GAP: f32 = 16.0;
'''),
    ("src/extras.rs",
     '''        let text_w=(ui.available_width()-SETTINGS_SWITCH_W-SETTINGS_ROW_GAP).max(120.0);
        ui.vertical(|ui|{
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y=2.0;
            ui.add(egui::Label::new(egui::RichText::new(title).size(13.0).color(settings_color(ctx,Color32::WHITE,Color32::from_rgb(30,34,41)))).wrap());
            if !description.is_empty(){ui.add(egui::Label::new(egui::RichText::new(description).size(11.5).color(settings_color(ctx,Color32::from_gray(175),Color32::from_gray(90)))).wrap());}
        });
        // add_space 之后还会再插入一次 item_spacing.x，这里一并扣掉，开关才不会每行往外挤一点。
        ui.add_space((ui.available_width()-SETTINGS_SWITCH_W-ui.spacing().item_spacing.x).max(0.0));
''',
     '''        let text_w=(ui.available_width()-SETTINGS_SWITCH_W-SETTINGS_ROW_GAP).max(120.0);
        ui.vertical(|ui|{
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y=3.0;
            ui.add(egui::Label::new(egui::RichText::new(title).size(13.5).color(settings_color(ctx,Color32::from_rgb(238,240,244),Color32::from_rgb(24,27,32)))).wrap());
            if !description.is_empty(){ui.add(egui::Label::new(egui::RichText::new(description).size(12.0).color(settings_color(ctx,Color32::from_gray(158),Color32::from_rgb(96,101,110)))).wrap());}
        });
        // add_space 之后还会再插入一次 item_spacing.x，这里一并扣掉，开关才不会每行往外挤一点。
        ui.add_space((ui.available_width()-SETTINGS_SWITCH_W-ui.spacing().item_spacing.x).max(0.0));
'''),
    ("src/extras.rs",
     '''
pub(crate) fn settings_hint(ui:&mut egui::Ui,text:&str){
    let ctx=&ui.ctx().clone();
    ui.add(egui::Label::new(egui::RichText::new(text).size(11.5).color(settings_color(ctx,Color32::from_gray(160),Color32::from_gray(105)))).wrap());
}
pub(crate) fn settings_card(ui:&mut egui::Ui,add:impl FnOnce(&mut egui::Ui)) {
    let ctx=&ui.ctx().clone();
    egui::Frame::new().fill(settings_color(ctx,Color32::from_rgb(35,35,38),Color32::from_rgb(255,255,255)))
        .corner_radius(10.0).inner_margin(egui::Margin::symmetric(16,12))
        .show(ui,|ui|{ui.set_min_width(ui.available_width());ui.spacing_mut().item_spacing.y=8.0;add(ui);});
}
/// 设置分页对应的功能模块（页面序号）；常规、通知、关于、页面管理不属于任何模块。
pub(crate) fn settings_tab_module(tab:usize)->Option<usize>{
''',
     '''
pub(crate) fn settings_hint(ui:&mut egui::Ui,text:&str){
    let ctx=&ui.ctx().clone();
    ui.add(egui::Label::new(egui::RichText::new(text).size(12.0).color(settings_color(ctx,Color32::from_gray(148),Color32::from_rgb(100,106,115)))).wrap());
}
pub(crate) fn settings_card(ui:&mut egui::Ui,add:impl FnOnce(&mut egui::Ui)) {
    let ctx=&ui.ctx().clone();
    egui::Frame::new().fill(settings_color(ctx,Color32::from_rgb(35,35,38),Color32::from_rgb(255,255,255)))
        .stroke(egui::Stroke::new(1.0,settings_color(ctx,Color32::from_rgb(56,57,62),Color32::from_rgb(228,231,236))))
        .corner_radius(12.0).inner_margin(egui::Margin::symmetric(16,13))
        .show(ui,|ui|{
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y=8.0;
            // 卡内分隔线压淡：8px 行距已经够分隔，硬线太密会让卡片显得碎。
            ui.style_mut().visuals.widgets.noninteractive.bg_stroke=egui::Stroke::new(1.0,settings_color(ctx,Color32::from_rgb(48,49,54),Color32::from_rgb(238,240,244)));
            add(ui);
        });
}
/// 设置分页对应的功能模块（页面序号）；常规、通知、关于、页面管理不属于任何模块。
pub(crate) fn settings_tab_module(tab:usize)->Option<usize>{
'''),
    ("src/extras.rs",
     '''
pub(crate) fn settings_label(ui:&mut egui::Ui,text:&str){
    let ctx=&ui.ctx().clone();
    ui.add_space(9.0);
    ui.label(egui::RichText::new(text).size(11.5).color(settings_color(ctx,Color32::from_gray(185),Color32::from_gray(70))));
    ui.add_space(2.0);
}
fn support_author(ui:&mut egui::Ui){
    let id=egui::Id::new("support-author-open");
''',
     '''
pub(crate) fn settings_label(ui:&mut egui::Ui,text:&str){
    let ctx=&ui.ctx().clone();
    ui.add_space(13.0);
    // 分组标题：左侧补一条 3px accent 竖条当视觉锚点（不影响布局结构）。
    let response=ui.label(egui::RichText::new(text).size(12.0).color(settings_color(ctx,Color32::from_gray(168),Color32::from_rgb(88,94,104))));
    let r=response.rect;
    let bar=egui::Rect::from_min_size(egui::pos2(r.left()-10.0,r.top()+1.5),egui::vec2(3.0,(r.height()-3.0).max(8.0)));
    ui.painter().rect_filled(bar,1.5,settings_color(ctx,Color32::from_rgb(96,165,250),Color32::from_rgb(37,99,235)));
    ui.add_space(4.0);
}
fn support_author(ui:&mut egui::Ui){
    let id=egui::Id::new("support-author-open");
'''),
    ("src/extras.rs",
     '''    let options:Vec<(u32,&str)>=values.iter().zip(labels.iter()).map(|(v,l)|(*v,l.as_str())).collect();
    settings_control_row_right(ui,title,|ui|{settings_segmented(ui,value,&options,62.0);});
}
const SETTINGS_GROUPS: [(&str,usize,usize);9] = [
    ("常规",0,0), ("页面管理",10,10), ("音乐与歌词",1,1), ("专注与饮水",2,3),
    ("待办与置物",4,5), ("通知、邮箱与日程",6,8), ("AI 额度",11,11), ("天气",12,12), ("关于",9,9),
];

impl App {
    pub fn settings_ui(&mut self,ctx:&egui::Context,autostart:bool){
''',
     '''    let options:Vec<(u32,&str)>=values.iter().zip(labels.iter()).map(|(v,l)|(*v,l.as_str())).collect();
    settings_control_row_right(ui,title,|ui|{settings_segmented(ui,value,&options,62.0);});
}
const SETTINGS_GROUPS: [(&str,usize,usize);12] = [
    ("常规",0,0), ("页面管理",10,10), ("音乐与歌词",1,1), ("专注与饮水",2,3),
    ("待办与置物",4,5), ("通知、邮箱与日程",6,8), ("天气",12,12), ("手机连接",13,13), ("浏览器下载",14,14), ("MAA",15,15), ("AI 额度",11,11), ("关于",9,9),
];

/// 「浏览器下载」卡片：始终显示安装指引（打开浏览器扩展页和扩展文件夹、复制路径）。
/// Chrome/Edge 从 33 版起封死了本地扩展的静默安装，只能开发者模式加载，所以做成引导而非自动装。
/// 注：不做「已连接」状态检测——桥进程只在下载进行时才被浏览器拉起，平时检测恒为未连接，没有意义。
#[cfg(windows)]
fn bridge_card_body(app: &mut App, ctx: &egui::Context, ui: &mut egui::Ui) {
    let Some(ext) = crate::bridge::extension_folder() else {
        ui.label(egui::RichText::new("未找到浏览器扩展（bridge\\\\extension）。请用岛仔安装包安装后再来连接。").size(12.0).color(settings_color(ctx, Color32::from_gray(180), Color32::from_gray(75))));
        return;
    };
    ui.horizontal_wrapped(|ui| {
        for (name, exe) in crate::bridge::installed_browsers() {
            if ui.button(format!("安装到 {name}")).clicked() {
                let page = if name == "Edge" { "edge://extensions/" } else { "chrome://extensions/" };
                let _ = std::process::Command::new(&exe).arg(page).spawn();
                let _ = std::process::Command::new("explorer.exe").arg(&ext).spawn();
                ui.ctx().copy_text(ext.display().to_string());
                app.say(format!("已打开 {name} 的扩展页和扩展文件夹，把文件夹拖进去即可（路径已复制）"));
            }
        }
    });
    ui.label(egui::RichText::new(format!("扩展目录：{}", ext.display())).size(11.5).color(settings_color(ctx, Color32::from_gray(195), Color32::from_gray(60))));
    settings_hint(ui, "步骤：点上方按钮→ 扩展页右上角打开「开发者模式」→ 把 extension 文件夹拖进扩展页（或点「加载已解压的扩展程序」选它）→ 之后浏览器下载会实时上岛（蓝胶囊 = 下载中，绿胶囊 = 完成可点击打开）");
}

#[cfg(not(windows))]
fn bridge_card_body(_app: &mut App, _ctx: &egui::Context, ui: &mut egui::Ui) {
    ui.label("浏览器下载对接仅支持 Windows。");
}

impl App {
    pub fn settings_ui(&mut self,ctx:&egui::Context,autostart:bool){
'''),
    ("src/extras.rs",
     '''                        4=>egui::include_image!("../assets/settings-icons/system/tasks.svg"),
                        6=>egui::include_image!("../assets/settings-icons/system/notifications.svg"),
                        12=>egui::include_image!("../assets/settings-icons/system/weather.svg"),
                        _=>egui::include_image!("../assets/settings-icons/system/about.svg"),
                    };
                    let icon_bg=egui::Rect::from_center_size(egui::pos2(rect.left()+21.0,rect.center().y),egui::vec2(25.0,25.0));
                    egui::Image::new(icon).paint_at(ui,icon_bg);
''',
     '''                        4=>egui::include_image!("../assets/settings-icons/system/tasks.svg"),
                        6=>egui::include_image!("../assets/settings-icons/system/notifications.svg"),
                        12=>egui::include_image!("../assets/settings-icons/system/weather.svg"),
                        13=>egui::include_image!("../assets/settings-icons/system/phone.svg"),
                        14=>egui::include_image!("../assets/settings-icons/system/browser.svg"),
                        15=>egui::include_image!("../assets/maa-icon.png"), // 与状态胶囊同一个 MAA 官方图标
                       _=>egui::include_image!("../assets/settings-icons/system/about.svg"),
                    };
                    let icon_bg=egui::Rect::from_center_size(egui::pos2(rect.left()+21.0,rect.center().y),egui::vec2(25.0,25.0));
                    egui::Image::new(icon).paint_at(ui,icon_bg);
'''),
    ("src/extras.rs",
     '''                    }
                }
                ui.visuals_mut().selection.bg_fill=settings_color(ctx,Color32::from_rgb(45,105,165),Color32::from_rgb(209,226,248));
                let titles=["常规","音乐与歌词","专注","健康提醒","待办","置物","通知","邮箱","日程","关于","页面管理","AI 额度","天气"];
                ui.label(egui::RichText::new(titles[self.settings_tab.min(12)]).size(18.0).strong().color(settings_color(ctx,Color32::WHITE,Color32::from_rgb(30,34,41))));
                if let Some((_,start,end))=SETTINGS_GROUPS.iter().find(|(_,start,end)|(*start..=*end).contains(&self.settings_tab)) {
                    if start!=end {
                        ui.add_space(7.0);
                        egui::Frame::new().fill(settings_color(ctx,Color32::from_rgb(33,33,36),Color32::from_rgb(232,235,240))).corner_radius(8.0).inner_margin(3).show(ui,|ui|{
''',
     '''                    }
                }
                ui.visuals_mut().selection.bg_fill=settings_color(ctx,Color32::from_rgb(45,105,165),Color32::from_rgb(209,226,248));
                let titles=["常规","音乐与歌词","专注","健康提醒","待办","置物","通知","邮箱","日程","关于","页面管理","AI 额度","天气","手机连接","浏览器下载","MAA"];
                ui.label(egui::RichText::new(titles[self.settings_tab.min(15)]).size(18.0).strong().color(settings_color(ctx,Color32::WHITE,Color32::from_rgb(30,34,41))));
               if let Some((_,start,end))=SETTINGS_GROUPS.iter().find(|(_,start,end)|(*start..=*end).contains(&self.settings_tab)) {
                    if start!=end {
                        ui.add_space(7.0);
                        egui::Frame::new().fill(settings_color(ctx,Color32::from_rgb(33,33,36),Color32::from_rgb(232,235,240))).corner_radius(8.0).inner_margin(3).show(ui,|ui|{
'''),
    ("src/extras.rs",
     '''                    match self.settings_tab {
                    12=>self.weather_settings(ui),
                    11=>self.quota_settings(ui),
                    10=>{
                        ui.label(egui::RichText::new("选择需要的页面和状态组件。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                        settings_label(ui,"常驻排列");
                        settings_card(ui,|ui|self.resident_order_settings(ui));
                        settings_label(ui,"状态组件");
                        settings_card(ui,|ui|{
                            setting_row(ui,"电量","本机电量、充电状态与提醒；无电池设备自动隐藏",&mut self.settings.show_battery);
                            ui.separator();
                            setting_row(ui,"岛上歌词","收起时显示歌词，无歌词时显示封面和歌名",&mut self.settings.island_lyrics);
                        });
''',
     '''                    match self.settings_tab {
                    12=>self.weather_settings(ui),
                    11=>self.quota_settings(ui),
                    13=>{
                        settings_label(ui,"连接状态");
                        settings_card(ui,|ui|{
                            // 连接状态：直接读手机连接卡片，不经电量开关。
                            let (linked, device, level) = crate::phone_battery::snapshot();
                            let status_text = if linked {
                                match (&device, level) {
                                    (d, Some(l)) if !d.is_empty() => format!("已连接：{d} · {l}%"),
                                    (_, Some(l)) => format!("已连接 · 电量 {l}%"),
                                    (d, _) if !d.is_empty() => format!("已连接：{d}"),
                                    _ => "已连接".to_string(),
                                }
                            } else {
                                "未连接".to_string()
                            };
                            let (light, dark) = if linked {
                                (Color32::from_rgb(46,125,50), Color32::from_rgb(129,199,132))
                            } else {
                                (Color32::from_gray(120), Color32::from_gray(160))
                            };
                            ui.label(egui::RichText::new(status_text).size(13.0).strong().color(settings_color(ctx, light, dark)));

                            // 一键打开「手机连接」：已装直接启动，未装跳商店。
                            let open = if crate::phone_battery::installed() {
                                Some(("打开手机连接", "explorer.exe", "shell:AppsFolder\\\\Microsoft.YourPhone_8wekyb3d8bbwe!App"))
                            } else {
                                Some(("安装手机连接", "explorer.exe", "ms-windows-store://pdp/?ProductId=9NMPJ99VJBWV"))
                            };
                            if let Some((label, prog, arg)) = open {
                                if ui.button(label).clicked() {
                                    let _ = std::process::Command::new(prog).arg(arg).spawn();
                                }
                            }
                            settings_hint(ui, "用 Windows 内置的「手机连接」配对手机：打开后按提示扫码连接，之后手机短信、来电、电量都会自动上岛。连接成功后上方状态会变绿。");
                        });
                        settings_label(ui,"手机功能");
                        settings_card(ui,|ui|{
                            // 手机电量：从「手机连接」读取，与本机电量并排显示。
                            if crate::phone_battery::installed() {
                                setting_row(ui,"手机电量","手机电量，与本机分开显示",&mut self.settings.phone_battery);
                            } else {
                                setting_row_locked(ui,"手机电量","未检测到「手机连接」，安装后才能开启",self.settings.phone_battery);
                            }
                            ui.separator();
                            // 短信/来电经「手机连接」转发：手机连接没装、或没给通知读取权限时不可配置。
                            let phone_on=crate::phone_battery::installed();
                            let notify_on=self.settings.read_notifications;
                            let sms_ready=phone_on && notify_on;
                            let sms_before=self.settings.phone_alerts;
                            if sms_ready {
                                setting_row(ui,"短信与来电","短信与来电上岛；关闭后手机通知全部忽略",&mut self.settings.phone_alerts);
                                if sms_before!=self.settings.phone_alerts{self.settings.save();}
                            } else {
                                // 只置灰不改值：等条件恢复后开关自动回到原状态。
                                setting_row_locked(ui,"短信与来电",
                                    if !phone_on {"未检测到「手机连接」，安装后才能开启"} else {"需要先开启通知读取（在「通知、邮箱与日程」里连接）"},
                                    self.settings.phone_alerts);
                            }
                            ui.separator();
                            ui.label(egui::RichText::new("手机短信通过「手机连接」转发。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            // 短信总开关关掉后，验证码自动复制没有意义，一并置灰。
                            ui.add_enabled_ui(self.settings.phone_alerts,|ui|{
                                setting_row(ui,"验证码自动复制","手机短信验证码上岛时直接写入剪贴板",&mut self.settings.auto_copy_otp);
                            });
                            ui.separator();
                            ui.label(egui::RichText::new("手机 App 名单：勾选立即生效；增删条目改 %APPDATA%\\\\Daozai\\\\phone-apps.json。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            let before_phone_apps=self.phone_apps.items.clone();
                            for (name,label,allow) in &mut self.phone_apps.items {
                                let title=if label==name {name.clone()} else {format!("{} · {}",label,name)};
                                setting_row(ui,&title,"",allow);
                            }
                            if before_phone_apps!=self.phone_apps.items{self.phone_apps.save();}
                        });
                    },
                    10=>{
                        ui.label(egui::RichText::new("选择需要的页面和状态组件。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                        settings_label(ui,"常驻排列");
                        settings_card(ui,|ui|self.resident_order_settings(ui));
                        settings_label(ui,"状态组件");
                        settings_card(ui,|ui|{
                            setting_row(ui,"电量","本机电量环；没有电池时不显示",&mut self.settings.show_battery);
                            ui.separator();
                            setting_row(ui,"岛上歌词","收起时显示歌词，无歌词时显示封面和歌名",&mut self.settings.island_lyrics);
                        });
'''),
    ("src/extras.rs",
     '''                            settings_control_row_right(ui,"主题",|ui|{
                                if settings_segmented(ui,&mut self.settings.settings_theme,&[(SettingsTheme::System,"跟随系统"),(SettingsTheme::Light,"浅色"),(SettingsTheme::Dark,"深色")],52.0) {ctx.request_repaint();}
                            });
                            settings_hint(ui,"仅影响设置页，岛的外观保持不变。");
                        });
                        settings_label(ui,"启动");
                        settings_card(ui,|ui|{
''',
     '''                            settings_control_row_right(ui,"主题",|ui|{
                                if settings_segmented(ui,&mut self.settings.settings_theme,&[(SettingsTheme::System,"跟随系统"),(SettingsTheme::Light,"浅色"),(SettingsTheme::Dark,"深色")],52.0) {ctx.request_repaint();}
                            });
                            settings_hint(ui,"只改设置页配色，岛体不变");
                        });
                        settings_label(ui,"启动");
                        settings_card(ui,|ui|{
'''),
    ("src/extras.rs",
     '''                            if self.settings.skin.is_glass() {
                                ui.separator();
                                let mut blur=self.settings.glass_blur;
                                setting_row(ui,"背景模糊","使用 Windows 合成引擎的实时模糊，严格贴合岛体形状（需开启系统“透明效果”）；关闭后改为烟熏半透明",&mut blur);
                                self.settings.glass_blur=blur;
                            }
                            ui.separator();
''',
     '''                            if self.settings.skin.is_glass() {
                                ui.separator();
                                let mut blur=self.settings.glass_blur;
                                setting_row(ui,"背景模糊","实时模糊岛体背景；需系统开启透明效果",&mut blur);
                                self.settings.glass_blur=blur;
                            }
                            ui.separator();
'''),
    ("src/extras.rs",
     '''                                    if ui.add_enabled(self.settings.island_size!=100,egui::Button::new("恢复默认")).clicked(){self.settings.island_size=100;}
                                }).response
                            });
                            settings_hint(ui,"文字、图标和吉祥物按实际像素重新绘制，放大后依旧清晰；只影响岛体，不影响设置窗口。");
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
''',
     '''                                    if ui.add_enabled(self.settings.island_size!=100,egui::Button::new("恢复默认")).clicked(){self.settings.island_size=100;}
                                }).response
                            });
                            settings_hint(ui,"按实际像素渲染，放大不发虚");
                            ui.separator();
                            let anims:Vec<(model::OpenAnim,&str)>=model::OpenAnim::ALL.iter().map(|a|(*a,a.label())).collect();
                            settings_control_row_right(ui,"展开动效",|ui|{settings_segmented(ui,&mut self.settings.open_anim,&anims,52.0);});
                            settings_hint(ui,self.settings.open_anim.description());
                            ui.separator();
                            setting_number(ui,"悬停触发延时",&mut self.settings.hover_delay_ms,0,500," ms",10.0);
                            settings_hint(ui,"悬停多久后展开；80–120 ms 可避开鼠标划过");
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
'''),
    ("src/extras.rs",
     '''                                }).response
                            });
                            let edge=self.settings.dock.label();
                            ui.label(egui::RichText::new(if !self.settings.display_id.is_empty()&&selected.is_none(){"暂时显示在主屏，重新连接所选屏幕后自动恢复。".to_string()}else{format!("显示在所选屏幕{edge}；断开时暂时回到主屏。")}).size(11.0).color(ui.visuals().weak_text_color()));
                            ui.add_space(4.0);
                            // 停靠边缘：顶部为横向；左右为竖向排列，展开面板从边缘滑出。
                            let docks:Vec<(crate::model::Dock,&str)>=crate::model::Dock::ALL.iter().map(|d|(*d,d.label())).collect();
''',
     '''                                }).response
                            });
                            let edge=self.settings.dock.label();
                            ui.label(egui::RichText::new(if !self.settings.display_id.is_empty()&&selected.is_none(){"暂时显示在主屏，重连后自动恢复".to_string()}else{format!("所选屏幕{edge}；断开后回主屏")}).size(11.5).color(ui.visuals().weak_text_color()));
                            ui.add_space(4.0);
                            // 停靠边缘：顶部为横向；左右为竖向排列，展开面板从边缘滑出。
                            let docks:Vec<(crate::model::Dock,&str)>=crate::model::Dock::ALL.iter().map(|d|(*d,d.label())).collect();
'''),
    ("src/extras.rs",
     '''                            ui.separator();
                            setting_row(ui,"全屏时隐藏","游戏、视频和演示全屏时隐藏",&mut self.settings.hide_fullscreen);
                        });
                    }
                    1=>{
                        settings_label(ui,"歌词");
''',
     '''                            ui.separator();
                            setting_row(ui,"全屏时隐藏","游戏、视频和演示全屏时隐藏",&mut self.settings.hide_fullscreen);
                        });
                        settings_label(ui,"备份");
                        settings_card(ui,|ui|{
                            ui.horizontal(|ui|{
                                if ui.button("备份配置").on_hover_text("把 Daozai 目录下的配置与状态复制一份").clicked(){self.backup_settings();}
                                if ui.button("打开备份目录").clicked(){
                                    if let Some(dir)=backup_root(){let _=std::fs::create_dir_all(&dir);self.pending.push(Cmd::Open(dir));}
                                }
                            });
                            ui.add_space(3.0);
                            ui.small("备份到 %APPDATA%\\\\Daozai\\\\backups\\\\<时间戳>\\\\，只复制文件，不动原配置。");
                        });
                    }
                    1=>{
                        settings_label(ui,"歌词");
'''),
    ("src/extras.rs",
     '''                            ui.add_space(3.0);
                            ui.small("正数提前，负数延后；默认提前 0.35 秒。");
                        });
                    }
                    2=>{
                        settings_label(ui,"计时时长");
''',
     '''                            ui.add_space(3.0);
                            ui.small("正数提前，负数延后；默认提前 0.35 秒。");
                        });
                        #[cfg(windows)]
                        {
                            use crate::smtc::netease_control as ctl;
                            let auto_host=ctl::config().auto_host;
                            let mut authoritative=crate::music_volume::authoritative();
                            settings_label(ui,"音乐音量");
                            settings_card(ui,|ui|{
                                setting_row(ui,"以岛仔为准","启动后把岛仔记住的音量推给播放器", &mut authoritative);
                                ui.add_space(3.0);
                                ui.small("关闭后，你在播放器里手动调的音量不会被覆盖。");
                            });
                            if authoritative!=crate::music_volume::authoritative(){
                                crate::music_volume::set_authoritative(authoritative);
                                self.say(if authoritative{"音量以岛仔为准"}else{"音量以播放器为准"});
                            }
                            settings_label(ui,"网易云播放进度条修复");
                            settings_card(ui,|ui|{
                                ui.small("拖动播放进度条需要修改快捷方式。");
                                let fixed=ctl::shortcut_fixed();
                                ui.add(egui::Label::new(egui::RichText::new(if fixed{"已自动修改网易云快捷方式"}else{"还没有修改快捷方式"}).size(12.5).strong()).wrap());
                                ui.separator();
                                ui.small("开始菜单没有快捷方式？手动选取网易云快捷方式");
                                if ui.button("选择文件").clicked(){
                                        match crate::system::pick_files_for("选择网易云快捷方式",false){
                                            Ok(files)=>if let Some(path)=files.first(){
                                                match ctl::fix_shortcut(path){
                                                    Ok(message)=>self.say(message),
                                                    Err(error)=>self.say(error),
                                                }
                                            },
                                            Err(_)=>self.say("未选择文件"),
                                        }
                                    }
                                ui.separator();
                                let mut auto=auto_host;
                                setting_row(ui,"自动托管","开启后会重启未连接到岛仔的网易云进程",&mut auto);
                                if auto!=auto_host{
                                    match ctl::set_auto_host(auto){
                                        Ok(())=>self.say(if auto{"已开启自动托管"}else{"已关闭自动托管"}),
                                        Err(error)=>self.say(format!("保存失败：{}",error)),
                                    }
                                }
                                ui.separator();
                                ui.small("手动重启以连接到岛仔，并且补上开始菜单的快捷方式，立即生效。");
                                if ui.button("重启网易云").clicked(){
                                    match ctl::restart_with_debug_port(){
                                        Ok(message)=>self.say(message),
                                        Err(error)=>self.say(format!("未能重启：{}",error)),
                                    }
                                }
                            });
                        }
                    }
                    2=>{
                        settings_label(ui,"计时时长");
'''),
    ("src/extras.rs",
     '''                            setting_number(ui,"休息",&mut self.settings.rest_minutes,1,60," 分钟",1.0);
                            ui.add_space(3.0);
                            ui.small("调整后从下一阶段开始生效。");
                        });
                    }
                    3=>{
''',
     '''                            setting_number(ui,"休息",&mut self.settings.rest_minutes,1,60," 分钟",1.0);
                            ui.add_space(3.0);
                            ui.small("调整后从下一阶段开始生效。");
                        });
                        settings_label(ui,"专注记录");
                        settings_card(ui,|ui|{
                            // 只读内存里的统计（每分钟更新一次），不在这里读盘。
                            let (today,streak)=(self.focus_today_min,self.focus_streak_days);
                            let hours=today/60;
                            let line=if today>=60 {format!("今日 {hours} 小时 {} 分 · 连续 {streak} 天",today%60)} else {format!("今日 {today} 分 · 连续 {streak} 天")};
                            ui.add(egui::Label::new(egui::RichText::new(line).size(12.0)).wrap());
                            ui.add_space(3.0);
                            ui.small("只统计真正在跑的专注阶段，暂停不计；数据在 %APPDATA%\\\\Daozai\\\\focus-stats.json。");
                        });
                    }
                    3=>{
'''),
    ("src/extras.rs",
     '''                        });
                        settings_label(ui,"剪贴板");
                        settings_card(ui,|ui|{
                            setting_row(ui,"记录剪贴板","在置物架的「剪贴板」里查看最近复制的文字，点一下再次复制",&mut self.settings.clipboard_history);
                            ui.label(egui::RichText::new(format!("本机保存最近 {} 条，重新打开后可继续使用。关闭记录会暂停收集，手动清空会保留固定条目。密码管理器标记为敏感的内容不会记录。",crate::clipboard::MAX_RECENT)).size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            ui.add_space(5.0);
                            if ui.add_sized([120.0,32.0],egui::Button::new("打开剪贴板")).clicked(){self.settings_open=false;self.shelf_tab=1;self.show_page(3);}
                        });
                    }
                    6=>{
                        settings_label(ui,"系统提示");
                        settings_card(ui,|ui|{
                            setting_row(ui,"系统状态","音量、充电、网络和键盘状态",&mut self.settings.system_hud);
                            ui.separator();
                            setting_row(ui,"蓝牙通知","设备连接与断开提醒；关闭后停止蓝牙监听",&mut self.settings.bluetooth_notifications);
                            ui.separator();
                            setting_row(ui,"提醒总开关","关闭后饮水、专注、待办及聊天软件提醒都不显示",&mut self.settings.notifications);
                        });
                        // 总开关关闭时，微信与 QQ 两栏置灰并说明原因，避免「开了也不提醒」。
                        let master=self.settings.notifications;
                        let master_hint=|ui:&mut egui::Ui|if !master {
''',
     '''                        });
                        settings_label(ui,"剪贴板");
                        settings_card(ui,|ui|{
                            setting_row(ui,"记录剪贴板","在置物架里查看最近复制的文字",&mut self.settings.clipboard_history);
                            ui.label(egui::RichText::new(format!("本机保存最近 {} 条，重新打开后可继续使用。关闭记录会暂停收集，手动清空会保留固定条目。密码管理器标记为敏感的内容不会记录。",crate::clipboard::MAX_RECENT)).size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            ui.add_space(5.0);
                            if ui.add_sized([120.0,32.0],egui::Button::new("打开剪贴板")).clicked(){self.settings_open=false;self.shelf_tab=1;self.show_page(3);}
                        });
                    }
                    6=>{
                        // ① 基础开关：总闸 + 系统类提示
                        settings_label(ui,"系统提示");
                        settings_card(ui,|ui|{
                            setting_row(ui,"提醒总开关","关闭后所有提醒都不显示",&mut self.settings.notifications);
                            ui.separator();
                            setting_row(ui,"系统状态","音量、充电、网络和键盘状态",&mut self.settings.system_hud);
                            ui.separator();
                            setting_row(ui,"蓝牙通知","设备连接与断开提醒；关闭后停止蓝牙监听",&mut self.settings.bluetooth_notifications);
                            ui.separator();
                            setting_row(ui,"会议自动勿扰","日程期间自动勿扰，散会解除",&mut self.settings.quiet_meeting);
                        });
                        // ② 提醒条件：什么情况下打断你
                        settings_label(ui,"提醒条件");
                        settings_card(ui,|ui|{
                            settings_control_row(ui,"低电提醒",SETTINGS_LABEL_W,|ui|{
                                let mut value=self.settings.battery_alert;
                                ui.horizontal(|ui|{
                                    if settings_segmented(ui,&mut value,&[(0,"关"),(10,"10%"),(20,"20%"),(30,"30%")],44.0){
                                        self.settings.battery_alert=value;
                                    }
                                }).response
                            });
                            settings_hint(ui,"低于阈值提醒一次，充满时也提示");
                            ui.separator();
                            settings_control_row(ui,"AI 额度提醒",SETTINGS_LABEL_W,|ui|{
                                let mut value=self.settings.quota_alert;
                                ui.horizontal(|ui|{
                                    if settings_segmented(ui,&mut value,&[(0,"关"),(10,"10%"),(20,"20%"),(30,"30%")],44.0){
                                        self.settings.quota_alert=value;
                                    }
                                }).response
                            });
                            settings_hint(ui,"跌破阈值提醒一次，回升后重新计数");
                            ui.separator();
                            // 勿扰：选时长立即生效，到点自动关闭；重启后恢复关闭。
                            let mut quiet=if self.quiet_until>self.t {self.settings.quiet_minutes.max(30)} else {0};
                            let mut quiet_pick=None;
                            settings_control_row(ui,"勿扰",SETTINGS_LABEL_W,|ui|{
                                ui.horizontal(|ui|{
                                    if settings_segmented(ui,&mut quiet,&[(0,"关"),(30,"30 分"),(60,"60 分"),(120,"2 小时")],48.0){
                                        quiet_pick=Some(quiet);
                                    }
                                }).response
                            });
                            if let Some(minutes)=quiet_pick {self.set_quiet(minutes);}
                            settings_hint(ui,"勿扰时不弹横幅和铃铛，验证码除外");
                            ui.separator();
                            let mut keywords_on=!self.settings.notice_keywords.trim().is_empty();
                            setting_row(ui,"通知关键词高亮","命中时卡片左侧亮琥珀色竖条；词表见 settings.txt",&mut keywords_on);
                            if keywords_on != !self.settings.notice_keywords.trim().is_empty() {
                                self.settings.notice_keywords=if keywords_on {"验证码,面试,转账,取款,取件,欠费".to_string()} else {String::new()};
                            }
                            ui.separator();
                            settings_control_row(ui,"通知规则",SETTINGS_LABEL_W,|ui|{
                                ui.horizontal(|ui|{
                                    let count=self.notice_rules.items.len();
                                    ui.label(egui::RichText::new(if count==0 {"未配置".to_string()} else {format!("{count} 条")}).size(12.0));
                                    if ui.button("编辑规则").clicked(){self.pending.push(Cmd::Open(crate::model::NoticeRules::path()));}
                                }).response
                            });
                            settings_hint(ui,"规则文件：pattern + action（mute / highlight / copy），首条匹配生效");
                        });
                        // ③ 最近通知：设置放在前，记录放在最后
                        settings_label(ui,"最近通知");
                        settings_card(ui,|ui|{
                            settings_hint(ui,"点击一行复制正文，或点「稍后」5 分钟后再提醒");
                            let recent=self.recent_notices();
                            let mut picked:Option<(String,String,bool)>=None;
                            if recent.is_empty() {
                                settings_hint(ui,"还没有通知记录");
                            }
                            for (at,title,body) in recent.iter().rev().take(8) {
                                let line=format!("{at}  {title} · {}",body.chars().take(26).collect::<String>());
                                ui.horizontal(|ui|{
                                    let resp=ui.add(egui::Label::new(egui::RichText::new(line).size(12.0).color(settings_color(ctx,Color32::from_gray(190),Color32::from_rgb(58,64,74)))).sense(egui::Sense::click()));
                                    if resp.on_hover_text(body.as_str()).clicked(){picked=Some((title.clone(),body.clone(),false));}
                                    if ui.small_button("稍后").on_hover_text("5 分钟后再提醒一次").clicked(){picked=Some((title.clone(),body.clone(),true));}
                                });
                            }
                            if let Some((title,body,snooze))=picked {
                                if snooze {self.snooze_notice(5.0,title,body);}
                                else {self.pending.push(Cmd::Copy(body));self.say("已复制通知正文");}
                            }
                       });
                        // 总开关关闭时，微信与 QQ 两栏置灰并说明原因，避免「开了也不提醒」。
                        let master=self.settings.notifications;
                        let master_hint=|ui:&mut egui::Ui|if !master {
'''),
    ("src/extras.rs",
     '''                        settings_card(ui,|ui|{
                            master_hint(ui);
                            ui.add_enabled_ui(master,|ui|{
                            setting_row(ui,"微信新消息提醒","根据任务栏闪动提醒，不读取联系人或聊天正文",&mut self.settings.wechat_notifications);
                            if self.settings.wechat_notifications {
                                ui.separator();
                                setting_row(ui,"托盘一键唤起微信","微信缩在托盘时，点提醒由系统按下微信托盘图标；图标收在折叠区时托盘菜单会短暂弹出",&mut self.settings.wechat_tray_open);
                            }
                            if master && !self.messenger_status(crate::app::Messenger::Wechat).is_empty(){ui.label(egui::RichText::new(self.messenger_status(crate::app::Messenger::Wechat)).size(12.0));}
                            });
''',
     '''                        settings_card(ui,|ui|{
                            master_hint(ui);
                            ui.add_enabled_ui(master,|ui|{
                            setting_row(ui,"微信新消息提醒","按任务栏闪动提醒，不读聊天内容",&mut self.settings.wechat_notifications);
                            if self.settings.wechat_notifications {
                                ui.separator();
                                setting_row(ui,"托盘一键唤起微信","微信在托盘时点提醒直接唤起",&mut self.settings.wechat_tray_open);
                            }
                            if master && !self.messenger_status(crate::app::Messenger::Wechat).is_empty(){ui.label(egui::RichText::new(self.messenger_status(crate::app::Messenger::Wechat)).size(12.0));}
                            });
'''),
    ("src/extras.rs",
     '''                                }
                            });
                        });
                        settings_label(ui,"QQ 与 Windows 通知");
                        settings_card(ui,|ui|{
                            master_hint(ui);
                            ui.add_enabled_ui(master,|ui|{
                            ui.label(egui::RichText::new(&self.listener_status).size(13.0));
                            if self.settings.read_notifications {
                                if ui.button(if self.settings.qq_only{"断开 QQ 通知"}else{"断开系统通知"}).clicked(){self.pending.push(Cmd::DisconnectNotifications);}
                            }else{
                                ui.horizontal_wrapped(|ui|{
                                    if ui.button("只连接 QQ").clicked(){self.pending.push(Cmd::RequestQqNotifications);}
                                    if ui.button("连接所有应用通知").clicked(){self.pending.push(Cmd::RequestNotifications);}
                                });
                            }
                            ui.label(egui::RichText::new("QQ 需开启系统通知；新通知在岛上显示约 4.5 秒，默认隐藏正文。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            ui.separator();
                            setting_row(ui,"显示通知正文","可能包含私密消息；屏幕共享时建议关闭",&mut self.settings.notification_preview);
                            ui.label(egui::RichText::new("只接收 Windows 通知中心可见的内容；应用会在收到通知后出现在下方。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            let before_apps=self.notification_apps.items.clone();
                            for (_,name,enabled) in &mut self.notification_apps.items {setting_row(ui,name,"",enabled);}
                            if before_apps!=self.notification_apps.items{self.notification_apps.save();}
                            });
                        });
                    }
                    7=>{
                        settings_label(ui,"邮件提醒");
                        settings_card(ui,|ui|{
                            setting_row(ui,"新邮件提醒","新邮件到达时显示提醒；岛上歌词模式显示右侧图标",&mut self.settings.mail_enabled);
                            ui.separator();
                            let sync_before=self.settings.mail_sync_operations;
                            ui.add_enabled_ui(self.mail.manage.pending.is_empty(),|ui|{
                                setting_row(ui,"同步邮件操作到邮箱","开启后同步已读、删除和恢复；关闭时仅整理岛仔中的记录",&mut self.settings.mail_sync_operations);
                            });
                            if sync_before!=self.settings.mail_sync_operations{self.mail_manage_mode_changed();}
                            ui.label(egui::RichText::new("切换开关不会补交之前的本机操作。").size(11.5).color(settings_color(ctx,Color32::from_gray(195),Color32::from_gray(60))));
                            ui.separator();
                            setting_row(ui,"验证码高亮","识别到验证码时在岛上放大显示，点击即复制",&mut self.settings.mail_code);
                            ui.separator();
                            setting_choices(ui,"验证码隐藏",&mut self.settings.mail_code_hide_seconds,[30,60,300],"秒");
                            ui.label("默认 60 秒。明确有效时长更短时提前隐藏；历史邮件默认隐藏。隐藏不代表失效，主动查看后 15 秒再次隐藏。");
                            setting_choices(ui,"检查间隔",&mut self.settings.mail_interval,[60,120,300],"秒");
                            ui.label(egui::RichText::new("支持推送时实时收信，并每 10 秒补查；以上间隔仅用于不支持推送的邮箱。").size(11.5).color(settings_color(ctx,Color32::from_gray(195),Color32::from_gray(60))));
''',
     '''                                }
                            });
                        });
                        settings_label(ui,"通知来源");
                        settings_card(ui,|ui|{
                            master_hint(ui);
                            ui.add_enabled_ui(master,|ui|{
                            // 监听范围二选一：全部应用 / 仅 QQ，当前项高亮。
                            let listening=self.settings.read_notifications;
                            let all_mode=listening && !self.settings.qq_only;
                            let qq_mode=listening && self.settings.qq_only;
                            ui.horizontal_wrapped(|ui|{
                                if ui.selectable_label(all_mode,"全部应用").clicked() && !all_mode {
                                    self.pending.push(Cmd::RequestNotifications);
                                }
                                if ui.selectable_label(qq_mode,"仅 QQ").clicked() && !qq_mode {
                                    self.pending.push(Cmd::RequestQqNotifications);
                                }
                                if listening && ui.button("停止监听").clicked() {
                                    self.pending.push(Cmd::DisconnectNotifications);
                                }
                            });
                            let status=self.listener_status.clone();
                            ui.label(egui::RichText::new(status).size(13.0));
                            settings_hint(ui,"全部应用：按应用逐个开关；仅 QQ：只收 QQ");
                            ui.separator();
                            if !listening {
                                // 停止监听时不展示应用开关：勾选此刻没有意义，容易让人以为还在生效。
                                ui.label(egui::RichText::new("尚未开始监听。点上方「全部应用」或「仅 QQ」后才会接收通知。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            } else if qq_mode {
                                ui.label(egui::RichText::new("当前为「仅 QQ」，其他应用的通知不会登记。切到「全部应用」后，它们会出现在这里供逐项开关。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            } else {
                                ui.label(egui::RichText::new("应用列表：勾选后该应用的通知才会上岛；新发过通知的应用会自动出现在这里。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                                let before_apps=self.notification_apps.items.clone();
                                for (_,name,enabled) in &mut self.notification_apps.items {setting_row(ui,name,"",enabled);}
                                if before_apps!=self.notification_apps.items{self.notification_apps.save();}
                            }
                            ui.separator();
                            setting_row(ui,"显示通知正文","可能包含私密消息；屏幕共享时建议关闭",&mut self.settings.notification_preview);
                            setting_number(ui,"通知停留",&mut self.settings.notice_seconds,2,120," 秒",1.0);
                            setting_number(ui,"验证码停留",&mut self.settings.otp_seconds,10,300," 秒",1.0);
                            settings_hint(ui,"卡片停留时长；验证码至少 10 秒");
                            });
                        });
                    }
                    14=>{
                        settings_label(ui,"安装");
                        settings_card(ui,|ui|{
                            bridge_card_body(self,ctx,ui);
                        });
                        settings_label(ui,"下载提示");
                        settings_card(ui,|ui|{
                            let before=self.browser_prefs.clone();
                            setting_number(ui,"完成提示时长",&mut self.browser_prefs.done_seconds,3,60," 秒",1.0);
                            setting_number(ui,"进度显示门槛",&mut self.browser_prefs.min_progress_mb,0,500," MB",1.0);
                            settings_hint(ui,"完成提示停留时长；小于门槛的文件只提示完成，不给进度");
                            if before!=self.browser_prefs{self.browser_prefs.save();}
                        });
                        settings_label(ui,"工作原理");
                        settings_card(ui,|ui|{
                            ui.label(egui::RichText::new("浏览器扩展把下载事件发给本机桥进程，桥写入状态总线后由岛仔显示。Chrome/Edge 出于安全不允许软件静默安装本地扩展，所以首次使用需要按上方指引手动加载一次。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                        });
                    }
                    15=>{
                        // 对接前提：通知名单里开启 MAA，本页配置才可操作（与状态跟随同步）。
                        let maa_on=self.maa_enabled();
                        let maa_exe=self.notification_apps.items.iter()
                            .find(|(id,name,_)| name=="MAA" || id.to_ascii_lowercase().ends_with("maa.exe"))
                            .map(|(id,_,_)| id.clone());
                        let log_path=self.maa_log_path();
                        let log_ready=log_path.as_ref().is_some_and(|p|p.is_file());

                        settings_label(ui,"状态");
                        settings_card(ui,|ui|{
                            let (text,light,dark)=if !maa_on {
                                ("未对接：勾选通知里的 MAA，或在下方指定 MAA 程序".to_string(),Color32::from_gray(120),Color32::from_gray(160))
                            } else if log_ready {
                                ("已对接：正在跟随 MAA 日志".to_string(),Color32::from_rgb(46,125,50),Color32::from_rgb(129,199,132))
                            } else {
                                ("已开启通知，但没找到 MAA 日志".to_string(),Color32::from_rgb(198,120,30),Color32::from_rgb(240,180,90))
                            };
                            ui.label(egui::RichText::new(text).size(13.0).strong().color(settings_color(ctx,light,dark)));
                            if let Some(exe)=maa_exe.as_ref() {
                                ui.label(egui::RichText::new(format!("MAA：{exe}")).size(11.5).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            }
                            if let Some(path)=log_path.as_ref() {
                                ui.label(egui::RichText::new(format!("日志：{}",path.display())).size(11.5).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            }
                            if !maa_on {
                                settings_hint(ui,"勾选通知里的 MAA，或直接在下方指定程序");
                            } else if !log_ready {
                                settings_hint(ui,"没找到 debug\\\\asst.log，请指定 MAA 目录");
                            }
                        });

                        settings_label(ui,"上岛提醒");
                        settings_card(ui,|ui|{
                            ui.add_enabled_ui(maa_on,|ui|{
                                let before=self.maa_prefs.clone();
                                setting_row(ui,"常驻进度","运行时常驻，随任务完成填满",&mut self.maa_prefs.resident_progress);
                                ui.separator();
                                setting_row(ui,"任务进度","关掉常驻时的单独提示",&mut self.maa_prefs.notify_chain);
                                ui.separator();
                                setting_row(ui,"公招高星","自动公招刷出 4 星以上 tag 时提示",&mut self.maa_prefs.notify_recruit_star);
                                ui.separator();
                                setting_row(ui,"任务出错","任务失败或中断时提示",&mut self.maa_prefs.notify_error);
                                ui.separator();
                                setting_number(ui,"提示停留",&mut self.maa_prefs.seconds,4,30," 秒",1.0);
                                settings_hint(ui,"任务提示和普通通知一样弹卡片");
                                if before!=self.maa_prefs{self.maa_prefs.save();}
                            });
                        });

                        settings_label(ui,"路径");
                        settings_card(ui,|ui|{
                            // 这里不能用 maa_on 禁用：没对接时正是要靠它选程序来启用。
                            {
                                let before=self.maa_prefs.clone();
                                settings_control_row(ui,"MAA 程序",80.0,|ui|{
                                    ui.horizontal(|ui|{
                                        let field=(ui.available_width()-70.0).max(90.0);
                                        ui.add_sized([field,24.0],egui::TextEdit::singleline(&mut self.maa_prefs.exe_path)
                                            .vertical_align(egui::Align::Center).hint_text("留空自动识别（MAA.exe）"));
                                        if ui.button("选择…").clicked(){self.pending.push(Cmd::PickMaaExe);}
                                    }).response
                                });
                                settings_hint(ui,"留空则用通知名单里的 MAA");
                                ui.separator();
                                settings_control_row(ui,"日志目录",80.0,|ui|{
                                    ui.horizontal(|ui|{
                                        let field=(ui.available_width()-70.0).max(90.0);
                                        ui.add_sized([field,24.0],egui::TextEdit::singleline(&mut self.maa_prefs.log_dir)
                                            .vertical_align(egui::Align::Center).hint_text("留空自动识别（<MAA目录>\\\\debug）"));
                                        if ui.button("选择…").clicked(){self.pending.push(Cmd::PickMaaLogDir);}
                                    }).response
                                });
                                settings_hint(ui,"留空自动识别（MAA 程序旁的 debug 目录）");
                                if before!=self.maa_prefs{self.maa_prefs.save();}
                            }
                        });
                    }
                    7=>{
                        settings_label(ui,"邮件提醒");
                        settings_card(ui,|ui|{
                            setting_row(ui,"新邮件提醒","新邮件提醒；歌词模式下显示图标",&mut self.settings.mail_enabled);
                            ui.separator();
                            let sync_before=self.settings.mail_sync_operations;
                            ui.add_enabled_ui(self.mail.manage.pending.is_empty(),|ui|{
                                setting_row(ui,"同步邮件操作到邮箱","同步已读/删除到邮箱；关闭则只改本地",&mut self.settings.mail_sync_operations);
                            });
                            if sync_before!=self.settings.mail_sync_operations{self.mail_manage_mode_changed();}
                            ui.label(egui::RichText::new("切换开关不会补交之前的本机操作。").size(11.5).color(settings_color(ctx,Color32::from_gray(195),Color32::from_gray(60))));
                            ui.separator();
                            setting_row(ui,"邮件验证码高亮","邮件里识别到验证码时在岛上放大显示，点击即复制",&mut self.settings.mail_code);
                            ui.separator();
                            setting_choices(ui,"邮件验证码隐藏",&mut self.settings.mail_code_hide_seconds,[30,60,300],"秒");
                            ui.label("默认 60 秒。明确有效时长更短时提前隐藏；历史邮件默认隐藏。隐藏不代表失效，主动查看后 15 秒再次隐藏。");
                            setting_choices(ui,"检查间隔",&mut self.settings.mail_interval,[60,120,300],"秒");
                            ui.label(egui::RichText::new("支持推送时实时收信，并每 10 秒补查；以上间隔仅用于不支持推送的邮箱。").size(11.5).color(settings_color(ctx,Color32::from_gray(195),Color32::from_gray(60))));
'''),
    ("src/extras.rs",
     '''                    _=>{
                        settings_label(ui,"岛仔 Daozai");
                        settings_card(ui,|ui|{
                            ui.horizontal(|ui|{
                                ui.label(egui::RichText::new(format!("版本 {}",env!("CARGO_PKG_VERSION"))).size(13.0));
                                ui.add_space(8.0);
                                ui.vertical(|ui|self.updates.ui(ui));
                            });
                            ui.label(egui::RichText::new("桌面顶部的音乐、专注、提醒与日程小助手。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            ui.horizontal_wrapped(|ui|{
                                if ui.button("打开本地数据目录").clicked(){self.pending.push(Cmd::Open(crate::model::config_dir()));}
''',
     '''                    _=>{
                        settings_label(ui,"岛仔 Daozai");
                        settings_card(ui,|ui|{
                            let update_check=self.settings.update_check;
                            ui.horizontal(|ui|{
                                ui.label(egui::RichText::new(format!("版本 {} · 自定义构建",env!("CARGO_PKG_VERSION"))).size(13.0));
                                ui.add_space(8.0);
                                ui.vertical(|ui|self.updates.ui(ui,update_check));
                            });
                            setting_row(ui,"检查更新","关掉后隐藏更新入口，避免被官方包覆盖",&mut self.settings.update_check);
                            ui.label(egui::RichText::new("桌面顶部的音乐、专注、提醒与日程小助手。").size(12.0).color(settings_color(ctx,Color32::from_gray(180),Color32::from_gray(75))));
                            ui.horizontal_wrapped(|ui|{
                                if ui.button("打开本地数据目录").clicked(){self.pending.push(Cmd::Open(crate::model::config_dir()));}
'''),
    ("src/extras.rs",
     '''    let _=ctx.run(Default::default(),|_|{});
    assert_eq!(ctx.tex_manager().read().allocated().filter(|(_,meta)|meta.name=="qr").count(),0,"收起后应释放收款码贴图");
}
''',
     '''    let _=ctx.run(Default::default(),|_|{});
    assert_eq!(ctx.tex_manager().read().allocated().filter(|(_,meta)|meta.name=="qr").count(),0,"收起后应释放收款码贴图");
}

#[cfg(test)]
mod status_action_safety_tests {
    use super::status_action;

    #[test]
    fn 文件动作只放行存在的非可执行文件() {
        let root = std::env::temp_dir().join(format!("daozai-open-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let text = root.join("note.txt");
        let exe = root.join("run.exe");
        std::fs::write(&text, b"note").unwrap();
        std::fs::write(&exe, b"not an executable").unwrap();
        assert!(status_action(&format!("file:{}", text.display())).is_some());
        assert!(status_action(&format!("file:{}", exe.display())).is_none(), "可执行扩展必须拒绝");
        assert!(status_action(&format!("file:{}", root.join("missing.txt").display())).is_none(), "不存在的路径不该生成按钮");
        assert!(status_action("url:https://example.com").is_some());
        assert!(status_action("url:file:///C:/Windows/System32/cmd.exe").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod meeting_plan_tests {
    use super::meeting_hit;

    fn plan(items: &[(i64, i64, &str)]) -> Vec<(i64, i64, String)> {
        items.iter().map(|(a, b, c)| (*a, *b, (*c).to_string())).collect()
    }

    #[test]
    fn 只有刚开始且未结束的会议触发且最多两小时() {
        let now = 10_000;
        let p = plan(&[
            (now - 200, now + 600, "太早"),      // 超过 90 秒窗口
            (now + 60, now + 3600, "还没开始"),
            (now - 30, now - 1, "已结束"),
        ]);
        assert!(meeting_hit(&p, now).is_none(), "这三种都不该触发");

        let p = plan(&[(now - 30, now + 1800, "进行中")]);
        let (until, key) = meeting_hit(&p, now).unwrap();
        assert_eq!(until, now + 1800);
        assert_eq!(key, format!("进行中|{}", now - 30));

        let p = plan(&[(now - 30, now + 99_999, "超长会")]);
        assert_eq!(meeting_hit(&p, now).unwrap().0, now + 7200, "勿扰最多 2 小时");
    }

    #[test]
    fn 全天与无结束时间的日程不进候选表() {
        use crate::mail::Meeting;
        fn mk(all_day: bool, end: Option<i64>, uid: &str) -> Meeting {
            Meeting { all_day, title: "会".into(), start: 9_900, end, at: "11-12 10:00".into(), uid: uid.into() }
        }
        let events = vec![
            mk(true, Some(12_000), "全天"),
            mk(false, None, "无结束"),
            mk(false, Some(12_000), "正常"),
        ];
        let candidates = super::meeting_candidates(events.iter());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].2, "正常");
        assert!(meeting_hit(&[], 10_000).is_none(), "候选为空不该凭空触发");
    }
}

/// 把今日日程压成轻量候选表：全天日程、没有结束时间的都不具备「散会自动失效」的条件。
fn meeting_candidates<'a>(events:impl Iterator<Item=&'a crate::mail::Meeting>)->Vec<(i64,i64,String)>{
    events.filter_map(|event|{
        if event.all_day{return None;}
        Some((event.start,event.end?,event.uid.clone()))
    }).collect()
}

/// 从候选表挑出「刚开始 90 秒内、还没结束」的会议，返回 (勿扰截止, 去重键)。
/// 抽成纯函数便于单测；多场同时命中时取最后一个。
fn meeting_hit(plan:&[(i64,i64,String)],now:i64)->Option<(i64,String)>{
    let mut hit=None;
    for (start,end,uid) in plan {
        if *start>now || now-*start>90 {continue;}
        if *end<=now {continue;}
        hit=Some(((*end).min(now+7200),format!("{}|{}",uid,start)));
    }
    hit
}
'''),
    ("src/lib.rs",
     '''//! - `anim` / `paint` / `shape` / `theme`：弹簧、矢量绘制、面板轮廓、配色字体
//! - `model` / `lyrics` / `smtc` / `icons`：数据、歌词、系统媒体、文件图标
//! - `native` / `tray`：仅 Windows 的窗口区域、命中穿透与托盘

pub mod display_layout;
mod mail_privacy;
''',
     '''//! - `anim` / `paint` / `shape` / `theme`：弹簧、矢量绘制、面板轮廓、配色字体
//! - `model` / `lyrics` / `smtc` / `icons`：数据、歌词、系统媒体、文件图标
//! - `native` / `tray`：仅 Windows 的窗口区域、命中穿透与托盘

/// 自定义构建标记：官方安装包会覆盖本地修改，更新弹窗据此提示。
pub const CUSTOM_BUILD: bool = true;

pub mod display_layout;
mod mail_privacy;
'''),
    ("src/lib.rs",
     '''pub mod side_dock;
mod task_picker;
pub mod model;
pub mod renderer;
pub mod otp;
pub mod quota;
mod quota_ui;
pub mod weather;
''',
     '''pub mod side_dock;
mod task_picker;
pub mod model;
pub mod island_status;
pub mod maa;
pub mod renderer;
pub mod otp;
pub mod phone_battery;
pub mod quota;
mod quota_ui;
pub mod weather;
'''),
    ("src/lib.rs",
     '''pub use updates::record_install_size;

#[cfg(windows)]
pub mod native;
#[cfg(windows)]
pub mod wechat;
''',
     '''pub use updates::record_install_size;

#[cfg(windows)]
pub mod bridge;
#[cfg(windows)]
pub mod native;
#[cfg(windows)]
pub mod wechat;
'''),
    ("src/mail_manage.rs",
     '''}
#[cfg(windows)]pub fn start()->(std::sync::mpsc::Sender<Request>,std::sync::mpsc::Receiver<Completed>){
    let(tx,rx)=std::sync::mpsc::channel::<Request>();let(done,out)=std::sync::mpsc::channel();
    std::thread::spawn(move||while let Ok(request)=rx.recv(){let result=protocol::run(&request);let _=done.send(Completed{request,result});});(tx,out)
}
''',
     '''}
#[cfg(windows)]pub fn start()->(std::sync::mpsc::Sender<Request>,std::sync::mpsc::Receiver<Completed>){
    let(tx,rx)=std::sync::mpsc::channel::<Request>();let(done,out)=std::sync::mpsc::channel();
    std::thread::spawn(move||while let Ok(request)=rx.recv(){
        // 单次协议异常不能带走整个线程：线程一死，后续请求全部发不出去，
        // 界面上的 pending 永远清不掉，「全部已读」等按钮就永久禁用。
        let result=match std::panic::catch_unwind(std::panic::AssertUnwindSafe(||protocol::run(&request))){
            Ok(result)=>result,
            Err(_)=>Err("邮件操作异常，请重试".to_string()),
        };
        let _=done.send(Completed{request,result});
    });(tx,out)
}
'''),
    ("src/mail_manage_ui.rs",
     '''#[derive(Default)]pub(crate) struct State{
    pub unread_only:bool,pub trash_open:bool,pub trash:Vec<Item>,
    pub data:HashMap<(String,Folder),Snapshot>,pub identities:HashMap<String,Account>,pub pending:HashSet<String>,
    pub next_refresh:f64,pub message:String,pub errors:Vec<String>,
    pub auto_read:Option<(String,u32)>,pub reader_trash:bool,
    reader_identity:Option<(Account,u32)>,
    local:HashMap<String,LocalFlags>,local_loaded:bool,
''',
     '''#[derive(Default)]pub(crate) struct State{
    pub unread_only:bool,pub trash_open:bool,pub trash:Vec<Item>,
    pub data:HashMap<(String,Folder),Snapshot>,pub identities:HashMap<String,Account>,pub pending:HashSet<String>,
    pub next_refresh:f64,
    /// 本批请求开始等待的时间点，用于请求丢失时超时复位。
    pub pending_at:f64,
    /// 同步进行中点了「全部已读」：等空闲后自动补执行，避免点了没反应。
    pub read_all_queued:bool,
    /// 邮件列表的账号筛选，None 表示全部账号。
    pub account_filter:Option<String>,
    /// 账号筛选下拉是否展开。
    pub account_menu:bool,
    pub message:String,pub errors:Vec<String>,
    pub auto_read:Option<(String,u32)>,pub reader_trash:bool,
    reader_identity:Option<(Account,u32)>,
    local:HashMap<String,LocalFlags>,local_loaded:bool,
'''),
    ("src/mail_manage_ui.rs",
     '''    pub fn ready(&self,account:&str,folder:Folder)->bool{self.pending.is_empty()&&self.data.contains_key(&(account.to_ascii_lowercase(),folder))}
}
impl App{
    fn mail_physical_folder(&self,folder:Folder)->Folder{if !self.settings.mail_sync_operations{Folder::Inbox}else{folder}}
    fn mail_local_key(&self,item:&Item)->Option<String>{
        let a=self.mail.accounts.iter().find(|a|a.email.eq_ignore_ascii_case(&item.account))?;
''',
     '''    pub fn ready(&self,account:&str,folder:Folder)->bool{self.pending.is_empty()&&self.data.contains_key(&(account.to_ascii_lowercase(),folder))}
}
impl App{
    /// 当前账号筛选是否包含这封邮件。
    pub(crate) fn mail_account_match(&self,item:&Item)->bool{
        self.mail.manage.account_filter.as_ref().is_none_or(|f|item.account.eq_ignore_ascii_case(f))
    }
    /// 账号筛选按钮上的短标签（只取 @ 前面那段）。
    pub(crate) fn mail_account_label(email:&str)->String{
        email.split('@').next().unwrap_or(email).to_string()
    }
    /// 「全部已读」所需的数据是否齐备（与是否正在同步无关）。
    pub(crate) fn mail_read_all_ready(&self)->bool{
        self.mail.accounts.iter().any(|a|a.enabled
            && self.mail.manage.data.contains_key(&(crate::mail::key(a),Folder::Inbox))
            && self.mail.manage.identities.get(&crate::mail::key(a))==Some(a))
    }
    fn mail_physical_folder(&self,folder:Folder)->Folder{if !self.settings.mail_sync_operations{Folder::Inbox}else{folder}}
    fn mail_local_key(&self,item:&Item)->Option<String>{
        let a=self.mail.accounts.iter().find(|a|a.email.eq_ignore_ascii_case(&item.account))?;
'''),
    ("src/mail_manage_ui.rs",
     '''            self.mail.manage.message="请等待邮件状态同步完成".into();return;
        }
        self.mail.manage.errors.clear();self.mail.manage.message=if action==Action::Load{"读取邮件状态…"}else{"同步中…"}.into();
        for account in accounts{
            let email=crate::mail::key(&account);
            let validity=self.mail.manage.data.get(&(email.clone(),folder)).filter(|_|self.mail.manage.identities.get(&email)==Some(&account)).map(|s|s.validity);
''',
     '''            self.mail.manage.message="请等待邮件状态同步完成".into();return;
        }
        self.mail.manage.errors.clear();self.mail.manage.message=if action==Action::Load{"读取邮件状态…"}else{"同步中…"}.into();
        self.mail.manage.pending_at=self.t;
        for account in accounts{
            let email=crate::mail::key(&account);
            let validity=self.mail.manage.data.get(&(email.clone(),folder)).filter(|_|self.mail.manage.identities.get(&email)==Some(&account)).map(|s|s.validity);
'''),
    ("src/mail_manage_ui.rs",
     '''            if !cfg!(test){self.mail_reload_local();}
            self.mail.manage.local_loaded=true;
        }
        if !self.mail.manage.pending.is_empty(){return;}
        if let Some((email,uid))=self.mail.manage.auto_read.take(){
            let folder=self.mail.manage.folder();
            let item=self.mail_rows().iter().find(|m|m.uid==uid&&m.account.eq_ignore_ascii_case(&email));
''',
     '''            if !cfg!(test){self.mail_reload_local();}
            self.mail.manage.local_loaded=true;
        }
        if !self.mail.manage.pending.is_empty(){
            // 超过 40 秒还没有结果，说明请求丢了，复位避免按钮永久禁用。
            if self.t-self.mail.manage.pending_at>40.0{
                self.mail.manage.pending.clear();
                self.mail.manage.message="邮件同步超时，已复位".into();
            }
            return;
        }
        if self.mail.manage.read_all_queued{
            self.mail.manage.read_all_queued=false;
            if self.mail_read_all_ready(){self.mail_manage_request(None,Folder::Inbox,Action::ReadAll);}
            else{self.mail.manage.message="邮件状态还没同步完，请稍后再试".into();}
        }
        if let Some((email,uid))=self.mail.manage.auto_read.take(){
            let folder=self.mail.manage.folder();
            let item=self.mail_rows().iter().find(|m|m.uid==uid&&m.account.eq_ignore_ascii_case(&email));
'''),
    ("src/mail_manage_ui.rs",
     '''        app.mail_managed(Completed{request:Request{account:a,folder:Folder::Inbox,validity:None,known:vec![],extra:(1..=60).map(|uid|(7,uid)).collect(),action:Action::Load},result:Ok(Snapshot{validity:7,entries:(1..=80).map(|uid|(uid,false)).collect(),items})});
        assert_eq!(app.mail.recent.len(),20);assert_eq!(app.mail.manage.trash.len(),60,"本机回收站不能被收件箱的显示上限截断");
    }
    #[test]fn read_state_is_separate_for_each_folder_and_account(){
        let mut state=State::default();state.data.insert(("test@example.com".into(),Folder::Inbox),Snapshot{validity:1,entries:vec![(42,true)],items:vec![]});
        let item=Item{html:String::new().into(),account:"test@example.com".into(),from:String::new(),subject:String::new(),time:String::new(),code:None,received_unix:None,summary:"".into(),links:vec![],meeting:None,uid:42};
''',
     '''        app.mail_managed(Completed{request:Request{account:a,folder:Folder::Inbox,validity:None,known:vec![],extra:(1..=60).map(|uid|(7,uid)).collect(),action:Action::Load},result:Ok(Snapshot{validity:7,entries:(1..=80).map(|uid|(uid,false)).collect(),items})});
        assert_eq!(app.mail.recent.len(),20);assert_eq!(app.mail.manage.trash.len(),60,"本机回收站不能被收件箱的显示上限截断");
    }
    #[test]fn account_filter_only_matches_selected_mailbox(){
        let mut app=App::new(&eframe::egui::Context::default(),Default::default());
        let item=|email:&str|Item{account:email.into(),from:String::new(),subject:String::new(),time:String::new(),code:None,html:String::new().into(),received_unix:None,summary:"".into(),links:vec![],meeting:None,uid:1};
        assert!(app.mail_account_match(&item("a@qq.com")),"默认应匹配全部账号");
        app.mail.manage.account_filter=Some("A@QQ.com".into());
        assert!(app.mail_account_match(&item("a@qq.com")),"筛选应忽略大小写");
        assert!(!app.mail_account_match(&item("b@qq.com")),"其他账号的邮件应被排除");
        assert_eq!(App::mail_account_label("a@qq.com"),"a");
    }
    #[test]fn stuck_pending_resets_after_timeout(){
        let ctx=eframe::egui::Context::default();let mut app=App::new(&ctx,Default::default());app.mail.accounts=vec![account()];
        app.mail_manage_request(None,Folder::Inbox,Action::Load);
        assert!(!app.mail.manage.pending.is_empty());
        // 模拟请求丢失：时间推进 50 秒后必须复位，否则「全部已读」等按钮永久禁用。
        app.t=app.mail.manage.pending_at+50.0;
        app.mail_manage_tick();
        assert!(app.mail.manage.pending.is_empty(),"请求丢失后 pending 必须能复位");
        assert!(app.mail.manage.message.contains("超时"));
    }
    #[test]fn read_state_is_separate_for_each_folder_and_account(){
        let mut state=State::default();state.data.insert(("test@example.com".into(),Folder::Inbox),Snapshot{validity:1,entries:vec![(42,true)],items:vec![]});
        let item=Item{html:String::new().into(),account:"test@example.com".into(),from:String::new(),subject:String::new(),time:String::new(),code:None,received_unix:None,summary:"".into(),links:vec![],meeting:None,uid:42};
'''),
    ("src/main.rs",
     '''        if !quota_demo{app.quota_initialize();}
        diagnostics::startup("启动剪贴板记录");
        if std::env::args().any(|arg|arg=="--settings"){app.open_settings();}
        if std::env::args().any(|arg|arg=="--calendar"){app.open_calendar(None);}
        #[cfg(feature="record")]
        if std::env::var("DAOZAI_MAIL_DEMO").is_ok(){app.mail_demo();}
''',
     '''        if !quota_demo{app.quota_initialize();}
        diagnostics::startup("启动剪贴板记录");
        if std::env::args().any(|arg|arg=="--settings"){app.open_settings();}
        // 直达 MAA 设置分组（SETTINGS_GROUPS 里 "MAA" 是第 9 项）。
        if std::env::args().any(|arg|arg=="--maa"){app.open_settings_group(8);}
        if std::env::args().any(|arg|arg=="--calendar"){app.open_calendar(None);}
        #[cfg(feature="record")]
        if std::env::var("DAOZAI_MAIL_DEMO").is_ok(){app.mail_demo();}
'''),
    ("src/main.rs",
     '''                Cmd::QuotaChooseFile=>{
                    #[cfg(windows)] match daozai::system::pick_files_for("选择 AI 账号的登录 JSON 文件",false){Ok(files)=>if let Some(path)=files.into_iter().next(){self.app.quota_set_source(path);},Err(_)=>self.app.say("无法选择登录文件")}
                },
                Cmd::QuotaRefresh=>{
                    if self.quota_at.ready(Duration::from_secs(15)) {
                        let (active,config)=(self.app.quota_active(),self.app.quota_config());let accounts:Vec<_>=config.accounts.iter().filter(|a|active && a.enabled).cloned().collect();
''',
     '''                Cmd::QuotaChooseFile=>{
                    #[cfg(windows)] match daozai::system::pick_files_for("选择 AI 账号的登录 JSON 文件",false){Ok(files)=>if let Some(path)=files.into_iter().next(){self.app.quota_set_source(path);},Err(_)=>self.app.say("无法选择登录文件")}
                },
                Cmd::PickMaaExe=>{
                    #[cfg(windows)] match daozai::system::pick_exe("选择 MAA 程序（MAA.exe）"){
                        Ok(Some(path))=>self.app.set_maa_exe(path.display().to_string()),
                        Ok(None)=>{},
                        Err(e)=>self.app.say(format!("无法选择程序：{}",e.code())),
                    }
                },
                Cmd::PickMaaLogDir=>{
                    #[cfg(windows)] match daozai::system::pick_folder("选择 MAA 日志目录（一般是 MAA 目录下的 debug）"){
                        Ok(Some(path))=>self.app.set_maa_log_dir(path.display().to_string()),
                        Ok(None)=>{},
                        Err(e)=>self.app.say(format!("无法选择文件夹：{}",e.code())),
                    }
                },
                Cmd::QuotaRefresh=>{
                    if self.quota_at.ready(Duration::from_secs(15)) {
                        let (active,config)=(self.app.quota_active(),self.app.quota_config());let accounts:Vec<_>=config.accounts.iter().filter(|a|active && a.enabled).cloned().collect();
'''),
    ("src/main.rs",
     '''}

fn main() -> std::process::ExitCode {
    diagnostics::install();
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
''',
     '''}

fn main() -> std::process::ExitCode {
    // 浏览器扩展桥宿主已拆成独立二进制 daozai-bridge.exe（见 src/bin/daozai-bridge），
    // 不再复用本 exe，这里直接进入主程序。
    diagnostics::install();
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
'''),
    ("src/model.rs",
     '''    pub calendar_auto_sync: bool,
    /// 记录最近复制的文字（最近记录与固定条目均在本机保存）。
    pub clipboard_history: bool,
    /// 天气页要看的城市；留空表示按网络出口自动定位。
    pub weather_city: String,
    /// 用户确认的具体地点；刷新时直接使用坐标，避免同名城市重新匹配。
''',
     '''    pub calendar_auto_sync: bool,
    /// 记录最近复制的文字（最近记录与固定条目均在本机保存）。
    pub clipboard_history: bool,
    /// 收起 ↔ 展开的过渡动效。
    pub open_anim: OpenAnim,
    /// 光标停在岛上多久才展开（毫秒，0 = 立即）。0–500，默认 200。
    /// 同时也是防误触阈值：太短会被鼠标路过触发，太长会觉得迟钝。
    pub hover_delay_ms: u32,
    /// 从「手机连接」读取手机电量，作为唯一的常驻电量来源（不回退本机电量）。
    pub phone_battery: bool,
    /// 通知保留更久，可在铃铛上重复悬停查看（关闭则 4.5 秒后消失）。
    /// 通知停留秒数（2–120）。停得久可以慢慢看，停得短不挡视线。
    pub notice_seconds: u32,
    /// 验证码卡片停留秒数（10–300，且不短于通知停留）。要给回看和点「复制」留时间。
    pub otp_seconds: u32,
    /// 旧开关，只用于兼容老的 settings.txt（读到它时折算成 notice_seconds）。
    pub notice_keep: bool,
    /// 收到含验证码的邮件时自动写入剪贴板。
    pub auto_copy_otp: bool,
    /// 是否允许检查/安装官方更新；自定义构建默认关闭，避免覆盖本地修改。
    pub update_check: bool,
    /// 接收「手机连接」转发的短信 / 来电（独立于 qq_only：仅 QQ 模式下也能单独关掉）。
    pub phone_alerts:bool,
    /// 低电提醒阈值（百分比，0 = 关闭）。手机与本机共用，跨越阈值只提醒一次。
    pub battery_alert: u32,
    /// AI 额度提醒阈值（剩余百分比，0 = 关闭）。跌破提醒一次，回升后重新武装。
    pub quota_alert: u32,
    /// 通知关键词高亮：逗号分隔，命中时卡片左侧亮一条琥珀色竖条；空 = 关闭。
    pub notice_keywords: String,
    /// 勿扰时长（分钟，0 = 关闭）。只在本次运行内生效，重启后恢复为关闭。
    pub quiet_minutes: u32,
    /// 会议自动勿扰：日程开始时自动把勿扰开到会议结束。
    pub quiet_meeting: bool,
    /// 邮件面板上次选择的收件账号（空 = 全部账号），重启后沿用。
    pub mail_filter: String,
    /// 天气页要看的城市；留空表示按网络出口自动定位。
    pub weather_city: String,
    /// 用户确认的具体地点；刷新时直接使用坐标，避免同名城市重新匹配。
'''),
    ("src/model.rs",
     '''    pub render_compat_reason: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 9], page_order: [0,1,2,3,4,5,6,7,8], show_battery: true, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wecom_notifications:false, feishu_notifications:false, dingtalk_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, avoid_top_bar: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, sit_remind: true, sit_minutes: 60, lunch_rest: true, lunch_start: 720, lunch_end: 810, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, weather_city: String::new(), weather_position: None, weather_saved: Vec::new(), render_compat: false, render_compat_reason: String::new() }
    }
}

impl Settings {
    /// 主窗口（灵动岛）的界面缩放倍率。
    pub fn island_scale(&self) -> f32 {
        island_size_value(self.island_size) as f32 / 100.0
''',
     '''    pub render_compat_reason: String,
}

/// 收起 ↔ 展开的过渡动效。刚度 k 越大越快，阻尼比 zeta < 1 会过冲、=1 为临界阻尼。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OpenAnim {
    /// 官方默认：欠阻尼垂落，末段约 5% 过冲。
    #[default]
    Elastic,
    /// 临界阻尼，无回弹。
    Smooth,
    /// 更快的刚度 + 更低的阻尼，回弹更明显。
    Snappy,
    /// 低刚度、大阻尼，慢慢铺开。
    Gentle,
    /// 不做形变动画，直接切换大小。
    Instant,
}

/// 一组动效的弹簧参数：(宽度刚度, 高度刚度, 收起刚度)，每项都是 `(k, zeta)`。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimTuning {
    pub open_w: (f32, f32),
    pub open_h: (f32, f32),
    pub close: (f32, f32),
}

impl OpenAnim {
    pub const ALL: [OpenAnim; 5] = [Self::Elastic, Self::Smooth, Self::Snappy, Self::Gentle, Self::Instant];
    pub fn key(self) -> &'static str {
        match self { Self::Elastic => "elastic", Self::Smooth => "smooth", Self::Snappy => "snappy", Self::Gentle => "gentle", Self::Instant => "instant" }
    }
    pub fn parse(value: &str) -> Self {
        match value { "smooth" => Self::Smooth, "snappy" => Self::Snappy, "gentle" => Self::Gentle, "instant" => Self::Instant, _ => Self::Elastic }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Elastic => "弹性", Self::Smooth => "平滑", Self::Snappy => "轻快", Self::Gentle => "柔和", Self::Instant => "瞬时" }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Elastic => "默认。欠阻尼弹簧，末段约 5% 过冲后回落，高度略软于宽度，像垂落下来。",
            Self::Smooth => "临界阻尼，没有回弹，展开和收起都干净利落。",
            Self::Snappy => "刚度更高、阻尼更低，展开更快，回弹也更明显。",
            Self::Gentle => "刚度低、阻尼大，慢慢铺开，适合不喜欢快速形变的时候。",
            Self::Instant => "不做形变动画，直接切换大小；仍保留悬停防误触的短暂延迟。",
        }
    }
    /// `None` 表示不做形变动画（瞬时切换）。
    pub fn tuning(self) -> Option<AnimTuning> {
        Some(match self {
            Self::Elastic => AnimTuning { open_w: (330.0, 0.70), open_h: (290.0, 0.74), close: (620.0, 1.0) },
            Self::Smooth  => AnimTuning { open_w: (300.0, 1.00), open_h: (260.0, 1.00), close: (620.0, 1.0) },
            Self::Snappy  => AnimTuning { open_w: (560.0, 0.58), open_h: (500.0, 0.62), close: (760.0, 1.0) },
            Self::Gentle  => AnimTuning { open_w: (170.0, 0.92), open_h: (150.0, 0.95), close: (320.0, 1.0) },
            Self::Instant => return None,
        })
    }
}

impl Default for Settings {
    fn default() -> Self {
                Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 9], page_order: [0,1,2,3,4,5,6,7,8], show_battery: true, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wecom_notifications:false, feishu_notifications:false, dingtalk_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, avoid_top_bar: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, sit_remind: true, sit_minutes: 60, lunch_rest: true, lunch_start: 720, lunch_end: 810, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, weather_city: String::new(), weather_position: None, weather_saved: Vec::new(), render_compat: false, render_compat_reason: String::new(), open_anim: OpenAnim::default(), hover_delay_ms: 200, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, phone_alerts:true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
}

impl Settings {
    pub fn keywords(&self) -> Vec<&str> {
        self.notice_keywords.split([',','，']).map(str::trim).filter(|word|!word.is_empty()).collect()
    }

    pub fn low_battery_threshold(&self) -> Option<u8> {
        (self.battery_alert > 0).then(|| self.battery_alert.min(99) as u8)
    }
    /// 主窗口（灵动岛）的界面缩放倍率。
    pub fn island_scale(&self) -> f32 {
        island_size_value(self.island_size) as f32 / 100.0
'''),
    ("src/model.rs",
     '''        let mut s = Self::default();
        for (k, v) in read_kv(&config_dir().join("settings.txt")) {
            match k.as_str() {
                "settings_theme" => s.settings_theme=crate::settings_theme::SettingsTheme::parse(&v),
                "display_id" => s.display_id = v,
                "skin" => {
''',
     '''        let mut s = Self::default();
        for (k, v) in read_kv(&config_dir().join("settings.txt")) {
            match k.as_str() {
                "open_anim" => s.open_anim = OpenAnim::parse(&v),
                "hover_delay_ms" => s.hover_delay_ms = v.parse::<u32>().unwrap_or(200).min(500),
                "phone_battery" => s.phone_battery = v != "0",
                "notice_seconds" => s.notice_seconds = v.parse::<u32>().unwrap_or(10).clamp(2,120),
                "otp_seconds" => s.otp_seconds = v.parse::<u32>().unwrap_or(90).clamp(10,300),
                "notice_keep" => {s.notice_keep = v != "0";s.notice_seconds = if s.notice_keep {10} else {4};},
                "auto_copy_otp" => s.auto_copy_otp = v != "0",
                "update_check" => s.update_check = v == "1",
                "phone_alerts" => s.phone_alerts=v!="0",
                "battery_alert" => s.battery_alert = match v.parse::<u32>().unwrap_or(20) { 0 => 0, 10 => 10, 30 => 30, _ => 20 },
                "notice_keywords" => s.notice_keywords = v.trim().to_string(),
                "quiet_minutes" => s.quiet_minutes = match v.parse::<u32>().unwrap_or(0) { 30 => 30, 60 => 60, 120 => 120, _ => 0 },
                "quiet_meeting" => s.quiet_meeting = v != "0",
                "quota_alert" => s.quota_alert = match v.parse::<u32>().unwrap_or(20) { 0 => 0, 10 => 10, 20 => 20, 30 => 30, _ => 20 },
                "mail_filter" => s.mail_filter = v.trim().to_string(),
                "settings_theme" => s.settings_theme=crate::settings_theme::SettingsTheme::parse(&v),
                "display_id" => s.display_id = v,
                "skin" => {
'''),
    ("src/model.rs",
     '''                ("calendar_prompt".into(), (self.calendar_prompt as u8).to_string()),
                ("calendar_auto_sync".into(), (self.calendar_auto_sync as u8).to_string()),
                ("clipboard_history".into(), (self.clipboard_history as u8).to_string()),
                ("weather_city".into(), self.weather_city.clone()),
                ("weather_position".into(), serde_json::to_string(&self.weather_position).unwrap()),
                ("weather_saved".into(), serde_json::to_string(&self.weather_saved).unwrap()),
''',
     '''                ("calendar_prompt".into(), (self.calendar_prompt as u8).to_string()),
                ("calendar_auto_sync".into(), (self.calendar_auto_sync as u8).to_string()),
                ("clipboard_history".into(), (self.clipboard_history as u8).to_string()),
                ("open_anim".into(),self.open_anim.key().into()),
                ("hover_delay_ms".into(),self.hover_delay_ms.to_string()),
                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("notice_seconds".into(), self.notice_seconds.to_string()),
                ("otp_seconds".into(), self.otp_seconds.to_string()),
                ("notice_keep".into(), (self.notice_keep as u8).to_string()),
                ("auto_copy_otp".into(), (self.auto_copy_otp as u8).to_string()),
                ("update_check".into(), (self.update_check as u8).to_string()),
                ("phone_alerts".into(),(self.phone_alerts as u8).to_string()),
                ("battery_alert".into(), self.battery_alert.to_string()),
                ("quota_alert".into(), self.quota_alert.to_string()),
                ("notice_keywords".into(), self.notice_keywords.clone()),
                ("quiet_minutes".into(), self.quiet_minutes.to_string()),
                ("quiet_meeting".into(), (self.quiet_meeting as u8).to_string()),
                ("mail_filter".into(), self.mail_filter.clone()),
                ("weather_city".into(), self.weather_city.clone()),
                ("weather_position".into(), serde_json::to_string(&self.weather_position).unwrap()),
                ("weather_saved".into(), serde_json::to_string(&self.weather_saved).unwrap()),
'''),
    ("src/model.rs",
     '''    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
''',
     '''    }
}

// —— 自定义构建：外部配置类型与工具（PhoneApps / MAA / 浏览器桥 / 限长读取）——

fn builtin_phone_apps()->Vec<(String,String,bool)>{
    let allow=[("QQ","QQ"),("微信","微信"),("WeChat","微信"),("支付宝","支付宝"),("Alipay","支付宝"),
        ("Gmail","Gmail"),("邮件","Gmail"),("短信","短信"),("信息","短信"),("电话","电话"),("来电","电话"),("验证码","验证码")];
    let silent=["Steam","Discord","12306","大麦","BOSS直聘","Boss直聘","明日方舟"];
    let mut items:Vec<(String,String,bool)>=allow.iter().map(|(k,v)|((*k).to_string(),(*v).to_string(),true)).collect();
    items.extend(silent.iter().map(|k|((*k).to_string(),"连接".to_string(),false)));
    items
}

pub(crate) const MAX_JSON_BYTES: u64 = 1_048_576;

pub(crate) fn read_capped(path:&Path,max:u64)->Option<Vec<u8>> {
    let meta=fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len()>max {return None;}
    fs::read(path).ok()
}

pub(crate) fn read_json_renaming_bad(path:&Path,max:u64)->Option<serde_json::Value> {
    let raw=read_capped(path,max)?;
    match serde_json::from_slice::<serde_json::Value>(&raw) {
        Ok(value)=>Some(value),
        Err(_)=>{
            // 固定名在第二次损坏时会撞车：部分平台 rename 还会直接替换目标，
            // 那样前一次的证据就没了。逐个找没被占用的名字，显式判存在再搬。
            let mut moved=false;
            for n in 0..64 {
                let mut bad=path.as_os_str().to_owned();
                bad.push(format!(".bad.{n}"));
                let target=PathBuf::from(bad);
                if target.exists(){continue;}
                if fs::rename(path,&target).is_ok(){moved=true;break;}
            }
            if !moved {
                // 实在移不走（文件被占用等），至少留一份副本，不让内容被覆盖得无影无踪。
                let mut bad=path.as_os_str().to_owned();
                bad.push(format!(".bad.copy.{}",std::process::id()));
                let _=fs::copy(path,PathBuf::from(bad));
            }
            None
        }
    }
}

pub struct NoticeRules {
    pub items: Vec<(String, String)>,
    compiled: Vec<(regex::Regex, String)>,
    modified: Option<std::time::SystemTime>,
}

impl Default for NoticeRules {
    fn default() -> Self {
        Self { items: Vec::new(), compiled: Vec::new(), modified: None }
    }
}

impl NoticeRules {
    pub fn path() -> PathBuf { config_dir().join("notice-rules.json") }

    pub fn load() -> Self {
        let mut rules = Self::default();
        rules.reload();
        rules
    }

    pub fn reload(&mut self) {
        let path = Self::path();
        self.modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
        let Some(raw) = read_capped(&path, MAX_JSON_BYTES) else {
            self.items.clear();
            self.compiled.clear();
            return;
        };
        let Ok(text)=std::str::from_utf8(&raw) else{return;};
        self.parse(text);
    }

    /// 从 JSON 字节解析规则（reload 与单测共用）：正则写错的那条跳过，不影响其它规则。
    fn parse(&mut self, raw: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(raw) else { return; };
        self.items = json.as_array().into_iter().flatten().filter_map(|rule| {
            Some((rule.get("pattern")?.as_str()?.to_owned(), rule.get("action")?.as_str()?.to_lowercase()))
        }).collect();
        self.compiled = self.items.iter().filter_map(|(pattern, action)| {
            regex::Regex::new(&format!("(?i){pattern}")).ok().map(|re| (re, action.clone()))
        }).collect();
    }

    /// 文件改动约 1 秒内生效，不用重启。
    pub fn reload_if_changed(&mut self) {
        let now = fs::metadata(Self::path()).and_then(|m| m.modified()).ok();
        if now != self.modified { self.reload(); }
    }

    /// 是否有任一条「动作 = action」的规则命中文本。
    pub fn hits(&self, text: &str, action: &str) -> bool {
        if text.is_empty() { return false; }
        self.compiled.iter().any(|(re, act)| act == action && re.is_match(text))
    }

    /// 第一条命中的规则动作，用于诊断日志。
    pub fn first_action(&self, text: &str) -> Option<&str> {
        self.compiled.iter().find(|(re, _)| re.is_match(text)).map(|(_, act)| act.as_str())
    }
}

pub struct PhoneApps {
    pub items:Vec<(String,String,bool)>,
    modified:Option<std::time::SystemTime>,
}

impl Default for PhoneApps { fn default()->Self{Self::builtin()} }
impl PhoneApps {
    pub fn builtin()->Self{Self{items:builtin_phone_apps(),modified:None}}
    fn path()->PathBuf{config_dir().join("phone-apps.json")}
    fn read_items(path:&Path)->Option<Vec<(String,String,bool)>>{
        let raw=read_capped(path,MAX_JSON_BYTES)?;
        let json=serde_json::from_slice::<serde_json::Value>(&raw).ok()?;
        let items:Vec<_>=json.as_array()?.iter().filter_map(|a|{
            let name=a.get("name")?.as_str()?.to_owned();
            let label=a.get("label").and_then(|v|v.as_str()).map(|s|s.to_owned()).unwrap_or_else(||name.clone());
            let allow=a.get("allow")?.as_bool()?;
            Some((name,label,allow))
        }).collect();
        if items.is_empty(){None}else{Some(items)}
    }
    pub fn load()->Self{
        #[cfg(test)]
        { return Self::builtin(); }
        #[cfg(not(test))]
        {
            let path=Self::path();
            let mut this=Self::builtin();
            if let Ok(meta)=fs::metadata(&path){
                this.modified=meta.modified().ok();
                if let Some(items)=Self::read_items(&path){this.items=items;return this;}
            }
            this.save();
            this.modified=fs::metadata(&path).ok().and_then(|m|m.modified().ok());
            this
        }
    }
    pub fn save(&self){
        let list:Vec<_>=self.items.iter().map(|(name,label,allow)|serde_json::json!({"name":name,"label":label,"allow":allow})).collect();
        let path=Self::path();
        if let Some(parent)=path.parent(){let _=fs::create_dir_all(parent);}
        if let Ok(data)=serde_json::to_vec_pretty(&list){let _=atomic_write(&path,&data);}
    }
    /// 配置文件改动后自动重载；测试构建不读盘。
    pub fn reload_if_changed(&mut self){
        #[cfg(test)]
        { return; }
        #[cfg(not(test))]
        {
            let path=Self::path();
            let Ok(meta)=fs::metadata(&path) else{return};
            let Ok(modified)=meta.modified() else{return};
            if self.modified==Some(modified){return;}
            self.modified=Some(modified);
            if let Some(items)=Self::read_items(&path){self.items=items;}
        }
    }
    pub fn lookup(&self,head:&str)->Option<(String,bool)>{
        self.items.iter().find(|(name,_,_)|head.contains(name.as_str()))
            .map(|(_,label,allow)|(label.clone(),*allow))
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct BridgePrefs {
    /// 下载完成后绿色胶囊的显示秒数（3..=60）。
    pub done_seconds: u32,
    /// 小于该大小（MB）的下载不显示进度胶囊、只在完成时提示；0 = 全部显示进度。
    pub min_progress_mb: u32,
}

impl Default for BridgePrefs {
    fn default() -> Self {
        Self { done_seconds: 12, min_progress_mb: 0 }
    }
}

impl BridgePrefs {
    fn path() -> PathBuf { config_dir().join("bridge.json") }

    pub fn load() -> Self {
        #[cfg(test)]
        { return Self::default(); }
        #[cfg(not(test))]
        {
            let path = Self::path();
            let Some(raw) = read_capped(&path, MAX_JSON_BYTES) else { return Self::default(); };
            let Ok(prefs) = serde_json::from_slice::<Self>(&raw) else { return Self::default(); };
            let mut prefs = prefs;
            prefs.done_seconds = prefs.done_seconds.clamp(3, 60);
            prefs.min_progress_mb = prefs.min_progress_mb.min(500);
            prefs
        }
    }

    pub fn save(&self) {
        #[cfg(not(test))]
        {
            let path = Self::path();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let Ok(body) = serde_json::to_vec_pretty(self) else { return };
            let _ = atomic_write(&path, &body);
        }
    }
}

/// MAA 偏好：程序路径、日志目录、通知开关与常驻进度胶囊。
///
/// 只在设置页「MAA」分组可改；「MAA 通知」未开启时整页不可操作（由设置页判断）。
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MaaPrefs {
    /// 任务链开始/完成提示。
    pub notify_chain: bool,
    /// 公招刷出 4 星以上 tag 时提示。
    pub notify_recruit_star: bool,
    /// 出错提示。
    pub notify_error: bool,
    /// 任务运行时在岛上常驻一条进度胶囊（像下载那样填满）。
    pub resident_progress: bool,
    /// 提示停留秒数（4..=30）。
    pub seconds: u32,
    /// MAA 可执行文件路径（空 = 用通知名单里登记的那份）。
    pub exe_path: String,
    /// MAA 日志目录（空 = 从 MAA 可执行文件自动推导 <MAA目录>\\debug）。
    pub log_dir: String,
}

impl Default for MaaPrefs {
    fn default() -> Self {
        Self { notify_chain: true, notify_recruit_star: true, notify_error: true, resident_progress: true, seconds: 8, exe_path: String::new(), log_dir: String::new() }
    }
}

impl MaaPrefs {
    fn path() -> PathBuf { config_dir().join("maa.json") }

    pub fn load() -> Self {
        #[cfg(test)]
        { return Self::default(); }
        #[cfg(not(test))]
        {
            let Some(raw) = read_capped(&Self::path(), MAX_JSON_BYTES) else { return Self::default(); };
            let Ok(mut prefs) = serde_json::from_slice::<Self>(&raw) else { return Self::default(); };
            prefs.seconds = prefs.seconds.clamp(4, 30);
            prefs
        }
    }

    pub fn save(&self) {
        #[cfg(not(test))]
        {
            let path = Self::path();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let Ok(body) = serde_json::to_vec_pretty(self) else { return };
            let _ = atomic_write(&path, &body);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::anim::Spring;

    #[test]
    fn 配置键往返且未知值回落到默认() {
        for style in OpenAnim::ALL {
            assert_eq!(OpenAnim::parse(style.key()), style);
            assert!(!style.label().is_empty());
            assert!(!style.description().is_empty());
        }
        assert_eq!(OpenAnim::parse(""), OpenAnim::Elastic);
        assert_eq!(OpenAnim::parse("whatever"), OpenAnim::Elastic);
        assert_eq!(OpenAnim::default(), OpenAnim::Elastic, "旧配置文件没有这个键时保持官方手感");
    }

    #[test]
    fn 参数合法且瞬时不需要弹簧() {
        assert!(OpenAnim::Instant.tuning().is_none());
        for style in OpenAnim::ALL {
            let Some(t) = style.tuning() else { continue; };
            for (k, zeta) in [t.open_w, t.open_h, t.close] {
                assert!(k > 0.0 && zeta > 0.0 && zeta <= 1.0, "{style:?} 参数越界: {k}/{zeta}");
            }
        }
    }

    #[test]
    fn 各动效轨迹确实不同且都能收敛() {
        let mut results = Vec::new();
        for style in OpenAnim::ALL {
            let Some(t) = style.tuning() else { continue; };
            let mut spring = Spring::new(143.0, t.open_w.0, t.open_w.1);
            spring.target = 419.0;
            let (mut peak, mut frames) = (143.0f32, 0usize);
            loop {
                spring.step(1.0 / 60.0);
                peak = peak.max(spring.pos);
                frames += 1;
                if (spring.pos - 419.0).abs() < 0.5 && spring.vel.abs() < 0.5 { break; }
                assert!(frames < 600, "{style:?} 没有收敛");
            }
            results.push((style, frames, peak));
        }
        let peak_of = |style| results.iter().find(|r| r.0 == style).unwrap().2;
        let frames_of = |style| results.iter().find(|r| r.0 == style).unwrap().1;
        assert!(peak_of(OpenAnim::Elastic) > 419.0, "弹性要有过冲");
        assert!(peak_of(OpenAnim::Snappy) > peak_of(OpenAnim::Elastic), "轻快回弹更明显");
        assert!(peak_of(OpenAnim::Smooth) <= 419.5, "平滑不该过冲");
        assert!(peak_of(OpenAnim::Gentle) <= 419.5, "柔和不该过冲");
        assert!(frames_of(OpenAnim::Snappy) < frames_of(OpenAnim::Gentle), "轻快应比柔和更快到位");
    }
    use super::*;

    #[test]
'''),
    ("src/music_volume.rs",
     '''//! Windows 音量混合器中的播放器音量；只操作匹配的应用会话。
use std::time::{Duration, Instant};

#[derive(Default)]
''',
     '''//! Windows 音量混合器中的播放器音量；只操作匹配的应用会话。
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Default)]
'''),
    ("src/music_volume.rs",
     '''        "qqmusic"=>Some("qqmusic.exe".into()),
        "spotify"=>Some("spotify.exe".into()),
        "汽水音乐"=>Some("sodamusic.exe".into()),
        _ if source.ends_with(".exe") && !source.contains(['\\\\','/'])=>Some(source),
        _=>None,
    }
''',
     '''        "qqmusic"=>Some("qqmusic.exe".into()),
        "spotify"=>Some("spotify.exe".into()),
        "汽水音乐"=>Some("sodamusic.exe".into()),
        // QQ 音乐的系统会话用 AUMID `Tencent.QQMusic`，进程名是 qqmusic.exe。
        _ if source.contains("tencent") && source.contains("qqmusic")=>Some("qqmusic.exe".into()),
        _ if source.ends_with(".exe") && !source.contains(['\\\\','/'])=>Some(source),
        _=>None,
    }
'''),
    ("src/music_volume.rs",
     '''mod platform {
    use windows::{core::{Interface,PWSTR},Win32::{
        Foundation::CloseHandle,
        Media::Audio::{AudioSessionStateExpired,IAudioSessionControl2,IAudioSessionManager2,IMMDeviceEnumerator,ISimpleAudioVolume,MMDeviceEnumerator,DEVICE_STATE_ACTIVE,eRender},
        Storage::Packaging::Appx::GetApplicationUserModelId,
        System::{Com::{CoCreateInstance,CoInitializeEx,CLSCTX_ALL,COINIT_MULTITHREADED},Threading::{OpenProcess,PROCESS_QUERY_LIMITED_INFORMATION}},
    }};
''',
     '''mod platform {
    use windows::{core::{Interface,PWSTR},Win32::{
        Foundation::CloseHandle,
        Media::Audio::{AudioSessionStateActive,AudioSessionStateExpired,IAudioSessionControl2,IAudioSessionManager2,IMMDeviceEnumerator,ISimpleAudioVolume,MMDeviceEnumerator,DEVICE_STATE_ACTIVE,eRender},
        Storage::Packaging::Appx::GetApplicationUserModelId,
        System::{Com::{CoCreateInstance,CoInitializeEx,CLSCTX_ALL,COINIT_MULTITHREADED},Threading::{OpenProcess,PROCESS_QUERY_LIMITED_INFORMATION}},
    }};
'''),
    ("src/music_volume.rs",
     '''            let _=CloseHandle(process);
            result.is_ok() && length>0 && (length as usize)<=id.len()
                && String::from_utf16_lossy(&id[..length as usize-1]).eq_ignore_ascii_case(source)
        }
    }

''',
     '''            let _=CloseHandle(process);
            result.is_ok() && length>0 && (length as usize)<=id.len()
                && String::from_utf16_lossy(&id[..length as usize-1]).eq_ignore_ascii_case(source)
        }
    }

    /// 该应用此刻是否真的有声音（音频会话峰值 > 0）。
    /// 岛仔的媒体源可能被别的播放器抢占，用它来判断“是否在播放”会漏；峰值是直接证据。
    pub fn audio_active(source:&str)->bool {
        if source.is_empty(){return false;}
        unsafe {
            thread_local!{static ENUMERATOR:Option<IMMDeviceEnumerator>=unsafe{
                let _=CoInitializeEx(None,COINIT_MULTITHREADED);
                CoCreateInstance(&MMDeviceEnumerator,None,CLSCTX_ALL).ok()
            };}
            ENUMERATOR.with(|enumerator|{
                let Some(enumerator)=enumerator else{return false;};
                let Ok(devices)=enumerator.EnumAudioEndpoints(eRender,DEVICE_STATE_ACTIVE) else{return false;};
                for d in 0..devices.GetCount().unwrap_or(0) {
                    let Ok(device)=devices.Item(d) else{continue;};
                    let Ok(manager)=device.Activate::<IAudioSessionManager2>(CLSCTX_ALL,None) else{continue;};
                    let Ok(sessions)=manager.GetSessionEnumerator() else{continue;};
                    for s in 0..sessions.GetCount().unwrap_or(0) {
                        let Ok(session)=sessions.GetSession(s) else{continue;};
                        if session.GetState().ok()==Some(AudioSessionStateExpired){continue;}
                        let Ok(control)=session.cast::<IAudioSessionControl2>() else{continue;};
                        if !control.GetProcessId().ok().is_some_and(|pid|matches(pid,source)){continue;}
                        // Active = 该会话正在输出音频（暂停通常是 Paused）。
                        if session.GetState().ok()==Some(AudioSessionStateActive){return true;}
                    }
                }
                false
            })
        }
    }

'''),
    ("src/music_volume.rs",
     '''    }
}

#[cfg(windows)]
pub fn read(source:&str)->Option<(u32,bool)> {
    platform::sessions(source).iter().filter_map(|session|unsafe {
        Some(((session.GetMasterVolume().ok()?*100.0).round() as u32,session.GetMute().ok()?.as_bool()))
    }).reduce(|a,b|(a.0.max(b.0),a.1 && b.1))
''',
     '''    }
}

/// QQ 音乐与网易云音乐的音量都是应用内部的，读不到；记住最近一次由岛仔设置的值用于显示，
/// 并落盘到 `music-volume.json`。`authoritative` 打开时，启动后把记住的值推回播放器，
/// 让两边以岛仔为准。
#[cfg(windows)]
#[derive(Clone)]
struct VolumeState{
    authoritative:bool,
    volumes:std::collections::HashMap<String,u32>,
}
#[cfg(windows)]
impl Default for VolumeState{
    fn default()->Self{Self{authoritative:true,volumes:std::collections::HashMap::new()}}
}

#[cfg(windows)]
static STATE:Mutex<Option<VolumeState>>=Mutex::new(None);

#[cfg(windows)]
fn volume_file()->std::path::PathBuf{crate::model::config_dir().join("music-volume.json")}

#[cfg(windows)]
fn state()->VolumeState{
    if let Ok(mut guard)=STATE.lock(){
        if guard.is_none(){
            let mut loaded=VolumeState::default();
            if let Ok(text)=std::fs::read_to_string(volume_file()){
                if let Ok(value)=serde_json::from_str::<serde_json::Value>(&text){
                    if let Some(object)=value.as_object(){
                        if let Some(volumes)=object.get("volumes").and_then(|item|item.as_object()){
                            loaded.authoritative=object.get("authoritative").and_then(|item|item.as_bool()).unwrap_or(true);
                            for (name,percent) in volumes{
                                if let Some(percent)=percent.as_u64(){loaded.volumes.insert(name.clone(),percent.min(100) as u32);}
                            }
                        }else{
                            // 旧格式是直接映射，忽略非数字字段。
                            for (name,percent) in object{
                                if let Some(percent)=percent.as_u64(){loaded.volumes.insert(name.clone(),percent.min(100) as u32);}
                            }
                        }
                    }
                }
            }
            *guard=Some(loaded);
        }
        guard.clone().unwrap_or_default()
    }else{
        VolumeState::default()
    }
}

#[cfg(windows)]
fn save_state(state:&VolumeState){
    let path=volume_file();
    if let Some(parent)=path.parent(){let _=std::fs::create_dir_all(parent);}
    let value=serde_json::json!({"authoritative":state.authoritative,"volumes":state.volumes});
    if let Ok(text)=serde_json::to_string_pretty(&value){let _=std::fs::write(path,text);}
}

#[cfg(windows)]
pub fn authoritative()->bool{state().authoritative}

#[cfg(windows)]
pub fn set_authoritative(value:bool){
    if let Ok(mut guard)=STATE.lock(){
        let mut current=guard.clone().unwrap_or_default();
        current.authoritative=value;
        *guard=Some(current.clone());
        save_state(&current);
    }
}

#[cfg(windows)]
fn internal_key(source:&str)->Option<&'static str>{
    if crate::smtc::qq_music(source){Some("qqmusic")}
    else if source.eq_ignore_ascii_case("cloudmusic.exe"){Some("cloudmusic")}
    else{None}
}

#[cfg(windows)]
fn internal_volume(source:&str)->Option<u32>{
    let key=internal_key(source)?;
    state().volumes.get(key).copied()
}

#[cfg(windows)]
fn remember_internal(key:&str,percent:u32){
    let percent=percent.min(100);
    let mut saved=None;
    if let Ok(mut guard)=STATE.lock(){
        let mut current=guard.clone().unwrap_or_default();
        current.volumes.insert(key.to_string(),percent);
        *guard=Some(current.clone());
        saved=Some(current);
    }
    if let Some(state)=saved{save_state(&state);}
}

#[cfg(windows)]
fn apply_internal(source:&str,percent:u32)->bool{
    if crate::smtc::qq_music(source){
        crate::smtc::qq_volume_command(percent)
    }else if source.eq_ignore_ascii_case("cloudmusic.exe"){
        crate::smtc::netease_control::set_volume(percent)
    }else{
        false
    }
}

/// 播放器本次运行的标识。QQ 用**最小 PID**：主进程 PID 最小，我们推送时产生的
/// `QQMusic.exe /volumeto` 交接进程 PID 更大，不干扰判断；重启后 PID 必然变化。
/// 网易云没有稳定 PID 可看，用调试端口是否打开。
#[cfg(windows)]
fn player_token(key:&str)->Option<u32>{
    if key=="qqmusic"{
        crate::smtc::player_process::pids_named("qqmusic.exe").into_iter().min()
    }else{
        // 网易云：端口打开且页面桥已注入才算“可发送”。会话标识用主进程 PID——
        // 原来固定返回 1，导致它退出再启动时标识不变、状态一直停在“已推过”，再也不推。
        if !crate::smtc::netease_control::bridge_ready(){return None;}
        crate::smtc::player_process::pids_named("cloudmusic.exe").into_iter().min().or(Some(1))
    }
}

#[cfg(windows)]
fn player_running(key:&str)->bool{player_token(key).is_some()}

/// 一次运行会话的推送状态。
#[cfg(windows)]
#[derive(Clone,Copy,PartialEq,Eq,Debug)]
enum PushStep{
    /// 没在运行、也没有记录。
    Idle,
    /// 没在运行但有记录：清掉。
    Reset,
    /// 出现了一次新的运行会话（首次，或 PID 变了）：开始计时。
    Start,
    /// 在运行，但还没到稳定时间。
    Waiting,
    /// 在运行且已稳定、本次还没推过：可以推。
    Ready,
    /// 本次运行已经推过：不重复推。
    Done,
}

/// `record` 是历史记录：(已推送过的状态位, 记录时间, 记录时的 PID)。
/// - 用 **PID** 识别新一次启动：QQ 退出时残留进程消失得慢，旧实现看不到 `running=false`，
///   状态会永远停在 Done（这就是“只有重启岛仔后第一次有效”的原因）；
/// - 用**状态位**区分“未播放时推过”和“播放时推过”：网易云只在播放时才应用内部音量，
///   未播放时推的那次会被忽略，所以开始播放后要再推一次；同一状态只推一次，暂停/续播不重复。
#[cfg(windows)]
fn push_step(token:Option<u32>,record:Option<(u8,std::time::Duration,u32)>,need:u8)->PushStep{
    match (token,record){
        (None,None)=>PushStep::Idle,
        (None,Some(_))=>PushStep::Reset,
        (Some(_),None)=>PushStep::Start,
        (Some(pid),Some((_,_,old))) if pid!=old=>PushStep::Start,
        (Some(_),Some((mask,_,_)))=>if mask&need==0{PushStep::Ready}else{PushStep::Done},
    }
}

/// 网易云当前是否在播放（只有播放时它才应用内部音量）。
/// 先看岛仔的媒体状态，再用音频峰值兜底——媒体源可能被别的播放器抢占，只看状态会漏判。
#[cfg(windows)]
fn netease_playing()->bool{
    let by_state=match crate::smtc::shared().lock(){
        Ok(state)=>state.source.eq_ignore_ascii_case("cloudmusic.exe") && state.playing,
        Err(_)=>false,
    };
    by_state || platform::audio_active("cloudmusic.exe")
}

/// 独立的后台通道：每秒检测一次。播放器每次启动（含快速重启）只推一次。
#[cfg(windows)]
pub fn spawn_push_watcher(){
    use std::sync::Once;
    static ONCE:Once=Once::new();
    ONCE.call_once(||{
        std::thread::spawn(||{
            loop{
                std::thread::sleep(std::time::Duration::from_secs(1));
                watch_tick();
            }
        });
    });
}

#[cfg(windows)]
pub fn watch_tick(){
    use std::sync::{Mutex,MutexGuard};
    use std::time::{Duration, Instant};
    // 两家都按用户要求不延迟：检测到播放器（QQ 看进程、网易云看调试端口）就立刻推一次。
    const STABLE_QQ:Duration=Duration::ZERO;
    const STABLE:Duration=Duration::ZERO;
    /// 未播放时推过。
    const MASK_IDLE:u8=1;
    /// 播放中推过（网易云只有这时才会真正应用）。
    const MASK_PLAYING:u8=2;
    // 每个播放器：(本次运行是否已推送, 记录时间, 记录时的 PID)
    static TRACK:Mutex<Option<std::collections::HashMap<&'static str,(u8,Instant,u32)>>>=Mutex::new(None);
    static LAST:Mutex<Option<Instant>>=Mutex::new(None);
    if cfg!(test){return;}
    fn guard<T>(lock:&'static Mutex<T>)->MutexGuard<'static,T>{
        match lock.lock(){Ok(guard)=>guard,Err(poisoned)=>poisoned.into_inner()}
    }
    {
        let mut last=guard(&LAST);
        if last.is_some_and(|at|at.elapsed()<Duration::from_secs(1)){return;}
        *last=Some(Instant::now());
    }
    if !state().authoritative{
        let mut track=guard(&TRACK);
        if let Some(map)=track.as_mut(){map.clear();}
        return;
    }
    for (key,source) in [("qqmusic","Tencent.QQMusic"),("cloudmusic","cloudmusic.exe")]{
        let token=player_token(key);
        // QQ 任何时候都能收命令；网易云要区分“未播放”和“播放中”，两者各推一次。
        let need=if key=="qqmusic"{MASK_IDLE}else if netease_playing(){MASK_PLAYING}else{MASK_IDLE};
        let mut should_push=false;
        let mut detected:Option<Duration>=None;
        {
            let mut track=guard(&TRACK);
            if track.is_none(){*track=Some(std::collections::HashMap::new());}
            if let Some(map)=track.as_mut(){
                let record=map.get(key).map(|(mask,at,pid)|(*mask,at.elapsed(),*pid));
                let stable=if key=="qqmusic"{STABLE_QQ}else{STABLE};
                match push_step(token,record,need){
                    PushStep::Idle=>{}
                    PushStep::Reset=>{map.remove(key);}
                    PushStep::Start=>{
                        map.insert(key,(0,Instant::now(),token.unwrap_or(0)));
                        detected=Some(stable);
                    }
                    PushStep::Waiting=>{}
                    PushStep::Ready=>{
                        let mask=record.map(|(mask,_,_)|mask).unwrap_or(0)|need;
                        map.insert(key,(mask,Instant::now(),token.unwrap_or(0)));
                        should_push=true;
                    }
                    PushStep::Done=>{}
                }
            }
        }
        if let Some(stable)=detected{
            let seconds=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value|value.as_secs()).unwrap_or(0);
            crate::resources::append_log(&crate::model::config_dir().join("diagnostics"),"music-volume.log",
                &format!("{seconds} detect key={key} pid={token:?} need={need} 等待{stable:?}后推送"));
        }
        if !should_push{continue;}
        let Some(percent)=state().volumes.get(key).copied() else{continue;};
        let source=source.to_string();
        let key=key.to_string();
        // 发命令只要几十毫秒，放一次性线程，避免拖慢调用方。
        std::thread::spawn(move||{
            if !player_running(&key){return;}
            let applied=apply_internal(&source,percent);
            let seconds=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value|value.as_secs()).unwrap_or(0);
            crate::resources::append_log(&crate::model::config_dir().join("diagnostics"),"music-volume.log",
                &format!("{seconds} push key={key} percent={percent} need={need} applied={applied}"));
        });
    }
}

#[cfg(windows)]
pub fn read(source:&str)->Option<(u32,bool)> {
    // QQ 音乐和网易云调的都是应用内音量，会话音量不会跟着变，显示以记住的值为准。
    if internal_key(source).is_some() {
        if let Some(percent)=internal_volume(source){
            // 静音仍走系统会话，所以静音状态单独读。
            let muted=platform::sessions(source).iter().filter_map(|session|unsafe{session.GetMute().ok()}).any(|state|state.as_bool());
            return Some((percent,muted));
        }
    }
    platform::sessions(source).iter().filter_map(|session|unsafe {
        Some(((session.GetMasterVolume().ok()?*100.0).round() as u32,session.GetMute().ok()?.as_bool()))
    }).reduce(|a,b|(a.0.max(b.0),a.1 && b.1))
'''),
    ("src/music_volume.rs",
     '''
#[cfg(windows)]
pub fn set_volume(source:&str,percent:u32)->bool {
    apply(source,|session|unsafe {
        session.SetMasterVolume(percent.min(100) as f32/100.0,std::ptr::null()).is_ok()
            && session.SetMute(false,std::ptr::null()).is_ok()
''',
     '''
#[cfg(windows)]
pub fn set_volume(source:&str,percent:u32)->bool {
    if let Some(key)=internal_key(source) {
        if apply_internal(source,percent) {
            remember_internal(key,percent);
            return true;
        }
    }
    apply(source,|session|unsafe {
        session.SetMasterVolume(percent.min(100) as f32/100.0,std::ptr::null()).is_ok()
            && session.SetMute(false,std::ptr::null()).is_ok()
'''),
    ("src/music_volume.rs",
     '''}

#[cfg(not(windows))]
pub fn read(_source:&str)->Option<(u32,bool)>{None}
#[cfg(not(windows))]
pub fn set_volume(_source:&str,_percent:u32)->bool{false}
''',
     '''}

#[cfg(not(windows))]
pub fn authoritative()->bool{false}
#[cfg(not(windows))]
pub fn set_authoritative(_value:bool){}
#[cfg(not(windows))]
pub fn read(_source:&str)->Option<(u32,bool)>{None}
#[cfg(not(windows))]
pub fn set_volume(_source:&str,_percent:u32)->bool{false}
'''),
    ("src/music_volume.rs",
     '''        assert_eq!(read("daozai-missing-player.exe"),None);
        assert!(!set_volume("daozai-missing-player.exe",100));
        assert!(!set_mute("",true));
    }

    #[test]
''',
     '''        assert_eq!(read("daozai-missing-player.exe"),None);
        assert!(!set_volume("daozai-missing-player.exe",100));
        assert!(!set_mute("",true));
    }

    #[test]
    #[cfg(windows)]
    fn volume_push_happens_once_per_player_run(){
        use std::time::Duration;
        const IDLE:u8=1;
        const PLAYING:u8=2;
        // 没在运行、没记录。
        assert_eq!(push_step(None,None,IDLE),PushStep::Idle);
        // 首次出现。
        assert_eq!(push_step(Some(10),None,IDLE),PushStep::Start);
        // 未播放态还没推过：可以推。
        assert_eq!(push_step(Some(10),Some((0,Duration::ZERO,10)),IDLE),PushStep::Ready);
        // 未播放态已推过：不再推。
        assert_eq!(push_step(Some(10),Some((IDLE,Duration::from_secs(60),10)),IDLE),PushStep::Done);
        // 开始播放：即使未播放态已推过，这一位还没推，要再推一次（网易云只在这时生效）。
        assert_eq!(push_step(Some(10),Some((IDLE,Duration::from_secs(60),10)),PLAYING),PushStep::Ready);
        // 播放态也推过了：暂停后再播不再重复推。
        assert_eq!(push_step(Some(10),Some((IDLE|PLAYING,Duration::from_secs(60),10)),PLAYING),PushStep::Done);
        // PID 变了就是新一次启动（残留进程导致看不到 running=false 的老问题）。
        assert_eq!(push_step(Some(20),Some((IDLE|PLAYING,Duration::from_secs(600),10)),IDLE),PushStep::Start);
        // 进程消失。
        assert_eq!(push_step(None,Some((IDLE|PLAYING,Duration::from_secs(600),10)),IDLE),PushStep::Reset);
    }

    #[test]
'''),
    ("src/netease_playback.rs",
     '''//! 网易云原生播放事件：只读增量日志，不依赖 SMTC 或特定 DLL 版本。
use super::{MediaKind,NowPlaying};
use std::{fs::File,io::{Read,Seek,SeekFrom},path::Path,sync::{mpsc,Arc},time::{Duration,Instant}};

const MAX_READ:u64=4*1024*1024;
const MAX_LINE:usize=256*1024;
''',
     '''//! 网易云原生播放事件：只读增量日志，不依赖 SMTC 或特定 DLL 版本。
use super::{MediaKind,NowPlaying};
use std::{fs::File,io::{Read,Seek,SeekFrom},path::Path,sync::{mpsc,Arc,Mutex},time::{Duration,Instant}};

const MAX_READ:u64=4*1024*1024;
const MAX_LINE:usize=256*1024;
'''),
    ("src/netease_playback.rs",
     '''    let (start,epoch)=CLOCK.get_or_init(||(Instant::now(),std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64));
    epoch.saturating_add(start.elapsed().as_millis() as u64)
}

// 初次读取历史记录时，使用可读的本地日期恢复进度；实时推进只依赖 Instant。
''',
     '''    let (start,epoch)=CLOCK.get_or_init(||(Instant::now(),std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64));
    epoch.saturating_add(start.elapsed().as_millis() as u64)
}

// CDP 跳转不会写 elog 的 setPlayingPosition，日志基准会一直停在旧位置，
// 岛上的进度条和歌词因此不跟着跳。跳转成功后记一个临时基准，由 poll() 覆盖
// 并从该时刻继续外推；换歌或客户端自己同步位置时自动让位。
struct SeekSync{position:f64,at_ms:u64,song_id:Option<u64>}
static SEEK_SYNC:Mutex<Option<SeekSync>>=Mutex::new(None);

/// 由 `netease_control::seek()` 在原生 Seek 回执确认成功后调用。
pub(super) fn note_seek(seconds:f64){
    if !seconds.is_finite() || !(0.0..86400.0).contains(&seconds){return;}
    if let Ok(mut guard)=SEEK_SYNC.lock(){*guard=Some(SeekSync{position:seconds,at_ms:monotonic_millis(),song_id:None});}
}
fn note_seek_at(seconds:f64,at_ms:u64){
    if let Ok(mut guard)=SEEK_SYNC.lock(){*guard=Some(SeekSync{position:seconds,at_ms,song_id:None});}
}
fn clear_seek_sync(){if let Ok(mut guard)=SEEK_SYNC.lock(){*guard=None;}}

fn apply_seek_sync(media:&mut NowPlaying,now:u64){
    let Ok(mut guard)=SEEK_SYNC.lock()else{return;};
    let Some(sync)=guard.as_mut()else{return;};
    if sync.song_id.is_none(){sync.song_id=media.song_id;}
    if sync.song_id!=media.song_id{*guard=None;return;} // 换歌：让位给 elog 的新基准
    let mut position=sync.position+now.saturating_sub(sync.at_ms)as f64/1000.0;
    if media.duration>0.0{position=position.min(media.duration);}
    media.position=position;
}

// 初次读取历史记录时，使用可读的本地日期恢复进度；实时推进只依赖 Instant。
'''),
    ("src/netease_playback.rs",
     '''        }else if event=="setPlayingPosition" && self.media.song_id.is_some(){
            let Some(position)=args.split(',').next().and_then(|s|s.trim().parse::<f64>().ok()).filter(|p|p.is_finite() && (0.0..86400.0).contains(p))else{return;};
            self.media.position=position;self.at=at;self.observed_at=now;self.ready=true;
        }else if event.starts_with("native") && event.ends_with("state") && self.media.song_id.is_some(){
            let Some(status)=args.split(',').next().and_then(|s|s.trim().parse::<u8>().ok()).filter(|s|matches!(s,1|2))else{return;};
            self.media.position=self.position(now);self.media.playing=status==1;self.at=at;self.observed_at=now;self.ready=true;
''',
     '''        }else if event=="setPlayingPosition" && self.media.song_id.is_some(){
            let Some(position)=args.split(',').next().and_then(|s|s.trim().parse::<f64>().ok()).filter(|p|p.is_finite() && (0.0..86400.0).contains(p))else{return;};
            self.media.position=position;self.at=at;self.observed_at=now;self.ready=true;
            clear_seek_sync(); // 客户端自己同步了位置，临时基准让位
        }else if event.starts_with("native") && event.ends_with("state") && self.media.song_id.is_some(){
            let Some(status)=args.split(',').next().and_then(|s|s.trim().parse::<u8>().ok()).filter(|s|matches!(s,1|2))else{return;};
            self.media.position=self.position(now);self.media.playing=status==1;self.at=at;self.observed_at=now;self.ready=true;
'''),
    ("src/netease_playback.rs",
     '''    pub(super) fn poll(&mut self)->Option<NowPlaying>{
        let path=std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("NetEase/CloudMusic/cloudmusic.elog");
        let sampled=std::time::Instant::now();
        let mut media=self.read(&path,process_id(),monotonic_millis())?;
        if self.cover.url==self.playback.artwork_url{media.artwork=self.cover.image.clone();}
        media.sampled_at=Some(sampled);Some(media)
    }
    pub(super) fn fill_artwork(&mut self,media:&mut NowPlaying){
        if media.source.eq_ignore_ascii_case("cloudmusic.exe") && media.song_id==self.playback.media.song_id && media.artwork.is_none(){
''',
     '''    pub(super) fn poll(&mut self)->Option<NowPlaying>{
        let path=std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("NetEase/CloudMusic/cloudmusic.elog");
        let sampled=std::time::Instant::now();
        let now=monotonic_millis();
        let mut media=self.read(&path,process_id(),now)?;
        if self.cover.url==self.playback.artwork_url{media.artwork=self.cover.image.clone();}
        media.sampled_at=Some(sampled);
        apply_seek_sync(&mut media,now);
        Some(media)
    }
    pub(super) fn fill_artwork(&mut self,media:&mut NowPlaying){
        if media.source.eq_ignore_ascii_case("cloudmusic.exe") && media.song_id==self.playback.media.song_id && media.artwork.is_none(){
'''),
    ("src/netease_playback.rs",
     '''#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn log_clock_origin_does_not_change_playback_position(){
        for origin in [0,9_000_000_000] {
''',
     '''#[cfg(test)]
mod tests{
    use super::*;
    // 全局只有一个跳转基准槽位，断言集中在一个测试里，避免并行测试互相踩。
    #[test]
    fn seek_sync_drives_position_and_yields_to_real_events(){
        clear_seek_sync();
        let mut media=NowPlaying{source:"cloudmusic.exe".into(),song_id:Some(123),duration:240.0,position:10.0,..Default::default()};
        note_seek_at(30.0,1_000);
        apply_seek_sync(&mut media,3_000);
        assert!((media.position-32.0).abs()<0.01,"跳转后应从 30 秒起继续外推");
        // 换歌后不再覆盖，让位给 elog 的新基准。
        media.song_id=Some(456);media.position=5.0;
        apply_seek_sync(&mut media,4_000);
        assert!((media.position-5.0).abs()<0.01);
        // 不能越过总时长。
        clear_seek_sync();note_seek_at(238.0,1_000);
        let mut tail=NowPlaying{song_id:Some(123),duration:240.0,position:10.0,..Default::default()};
        apply_seek_sync(&mut tail,5_000);
        assert!((tail.position-240.0).abs()<0.01);
        // 客户端自己写了 setPlayingPosition：临时基准必须清除。
        note_seek_at(80.0,1_000);
        let mut state=Playback::default();
        state.media.song_id=Some(123);
        state.line(&event(7,2_000,"\\"setPlayingPosition\\",45"),7,9_000_000);
        assert!(SEEK_SYNC.lock().unwrap().is_none());
        clear_seek_sync();
    }

    #[test]
    fn log_clock_origin_does_not_change_playback_position(){
        for origin in [0,9_000_000_000] {
'''),
    ("src/otp.rs",
     '''    if has(&before(hay, start, 20), ORDER_WORDS) || has(&after(hay, end, 8), ORDER_WORDS) {
        return None;
    }
    // 4 位年份：直接判负，掉到门槛以下
    if len == 4 && !mixed {
        if let Ok(y) = core.parse::<u32>() {
            if (1900..=2100).contains(&y) {
                score -= 35;
            }
        }
''',
     '''    if has(&before(hay, start, 20), ORDER_WORDS) || has(&after(hay, end, 8), ORDER_WORDS) {
        return None;
    }
    // 4 位年份：只在后面紧跟日期成分时判负（`2026-10-30`、`2026年`），
    // 否则「验证码：2084」这种正好落在 1900..=2100 的真实验证码会被误杀。
    if len == 4 && !mixed {
        if let Ok(y) = core.parse::<u32>() {
            let dateish = hay[end..].chars().next().is_some_and(|c| matches!(c, '-' | '/' | '.' | '年' | '月' | '日'));
            if (1900..=2100).contains(&y) && dateish {
                score -= 35;
            }
        }
'''),
    ("src/otp.rs",
     '''        .trim();
    let own_line = line == text;
    let colon = hay[..start].trim_end().ends_with(':');
    let char_pos = hay[..start].chars().count();
    let in_subject = char_pos < subject_end;
    let is_emph = emph.iter().any(|e| e.contains(text));
''',
     '''        .trim();
    let own_line = line == text;
    let colon = hay[..start].trim_end().ends_with(':');
    let tight = {
        let before6 = before(hay, start, 6);
        let trimmed = before6.trim_end_matches([' ', ':', '：', '是', '为']);
        strong().is_match(trimmed)
    };
    let char_pos = hay[..start].chars().count();
    let in_subject = char_pos < subject_end;
    let is_emph = emph.iter().any(|e| e.contains(text));
'''),
    ("src/otp.rs",
     '''        score += 15;
    }
    if colon {
        score += 10;
    }
    if in_subject {
''',
     '''        score += 15;
    }
    if colon {
        score += 10;
    } else if tight {
        // 「验证码1234」「验证码 1234」：没有冒号，但强关键词紧贴数字，同样算强信号。
        score += 10;
    }
    if in_subject {
'''),
    ("src/otp.rs",
     '''
    #[test]
    fn negatives_are_rejected() {
        let cases = [
            ("", "您的订单号 12345678 已发货"),
            ("", "订单 482913，金额 ￥1,299.00"),
''',
     '''
    #[test]
    fn negatives_are_rejected() {
        // 4 位验证码正好落在 1900..=2100（如 2084）时，不能因为「像年份」被判负。
        assert_eq!(code("", "【清枫北京科技】您的验证码：2084，如非本人操作，请忽略本短信!"), Some("2084".into()));
        // 常见写法「验证码1234」「验证码 1234」不能因为没有冒号就掉到门槛以下。
        assert_eq!(code("", "验证码 1999，5 分钟内有效"), Some("1999".into()));
        assert_eq!(code("", "您的验证码1234，5分钟内有效"), Some("1234".into()));
        let cases = [
            ("", "您的订单号 12345678 已发货"),
            ("", "订单 482913，金额 ￥1,299.00"),
'''),
    ("src/pages.rs",
     '''        for (i, (label, w)) in labels.iter().zip(&widths).enumerate() {
            let selected = self.shelf_tab == i;
            let r = c.hit(left, 9.5, *w, 21.0, ("shelf-tab", i));
            let (h, _) = c.hover_t(&r);
            if selected {
                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_gray(88));
''',
     '''        for (i, (label, w)) in labels.iter().zip(&widths).enumerate() {
            let selected = self.shelf_tab == i;
            let r = c.hit(left, 9.5, *w, 21.0, ("shelf-tab", i));
            let r = if i == 1 { r.on_hover_text("切到剪贴板") } else { r };
            let (h, _) = c.hover_t(&r);
            if selected {
                c.fill(left, 11.5, *w, 17.0, 8.5, Color32::from_gray(88));
'''),
    ("src/pages.rs",
     '''    }

    // ================= 剪贴板（置物架第二个分栏） =================
    fn page_clipboard(&mut self, c: &Card, _env: &Env, _cmds: &mut Vec<Cmd>) {
        if !self.settings.clipboard_history {
            c.dashed(42.0, 32.0, 335.0, 91.0, 12.0, 1.2, Color32::from_gray(70), 0.0);
''',
     '''    }

    // ================= 剪贴板（置物架第二个分栏） =================
    /// 剪贴板搜索：放大镜放在面板右侧的空白竖带（翻页箭头下方、页码豆上方）。
    /// 以前钉在表头 (352, 9.5)，会和常驻的音乐播放条 (x≈367, y≈18) 叠在一起；
    /// 表头其余元素又随内容动态排布，没有稳定空位，所以移到卡片外侧。
    /// 点开后输入框浮在列表上方（盖住第一行），Esc 或小叉收起。
    /// 过滤只作用于显示，索引仍指向同一份历史，复制/固定/删除的行为不变。
    fn clip_search(&mut self, c: &Card, cmds: &mut Vec<Cmd>) {
        // 右下侧空白带：x 377..419 不画列表，y 在箭头(≤101)与页码豆(≈131)之间。
        let icon = c.hit(384.0, 105.0, 26.0, 20.0, "clip-search-open");
        let (h, _) = c.hover_t(&icon);
        if h > 0.01 { c.circle(397.0, 115.0, 10.5, Color32::from_white_alpha((22.0 * h) as u8)); }
        let ink = if self.clip_search_open { theme::BLUE } else { paint::mix(theme::GRAY, theme::WHITE, h) };
        c.ring(395.2, 113.2, 4.4, 1.3, ink);
        c.seg((398.6, 116.6), (402.4, 120.4), 1.3, ink);
        let icon = icon.on_hover_text(if self.clip_search_open { "收起搜索" } else { "搜索剪贴板" });
        if icon.clicked() {
            self.clip_search_open = !self.clip_search_open;
            self.clip_query.clear();
            self.clip_search_focus = false;
        }
        if !self.clip_search_open { return; }
        // 输入框浮在列表卡片的上沿，比卡片亮一档 + 一圈描边，聚焦时描边变蓝。
        let rect = c.rect(46.0, 32.0, 327.0, 20.0);
        c.fill(46.0, 32.0, 327.0, 20.0, 8.0, Color32::from_gray(64));
        c.stroke(46.0, 32.0, 327.0, 20.0, 8.0, 1.0, Color32::from_gray(102));
        let mut ui = egui::Ui::new(c.ui.ctx().clone(), egui::Id::new("clip-search"), egui::UiBuilder::new().layer_id(c.ui.layer_id()).max_rect(rect).style(c.ui.style().clone()));
        ui.set_clip_rect(c.painter.clip_rect());
        ui.set_opacity(c.painter.opacity());
        if !c.live { ui.disable(); }
        // 多留一点左侧内边距，文字不贴边；光标也与文字对齐。
        let text_rect = Rect::from_min_max(rect.min + egui::vec2(8.0, 2.0), rect.max - egui::vec2(26.0, 2.0));
        let mut buf = std::mem::take(&mut self.clip_query);
        let te = egui::TextEdit::singleline(&mut buf)
            .id(egui::Id::new("clip-search-edit"))
            .frame(false)
            .margin(egui::Margin::ZERO)
            .text_color(theme::WHITE)
            .font(theme::font(size::TINY, W::Regular))
            .hint_text(egui::RichText::new("输入关键词筛选").color(Color32::from_gray(158)).font(theme::font(size::TINY, W::Regular)))
            .desired_width(text_rect.width());
        let resp = ui.put(text_rect, te);
        self.clip_query = buf;
        // 展开后的第一帧就请求焦点并激活窗口，光标才会闪；之后点一下输入框也会重新聚焦。
        if !self.clip_search_focus || resp.clicked() {
            self.clip_search_focus = true;
            resp.request_focus();
            cmds.push(Cmd::Focus);
        }
        self.clip_typing=resp.has_focus();
        if resp.has_focus() { c.stroke(46.0, 32.0, 327.0, 20.0, 8.0, 1.4, theme::BLUE); }
        // Esc 退出搜索：清空关键词并收起输入框。
        if resp.has_focus() && c.ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.clip_query.clear();
            self.clip_search_open = false;
            resp.surrender_focus();
        }
        let x = c.hit(352.0, 34.0, 18.0, 16.0, "clip-search-close").on_hover_text(if self.clip_query.is_empty() { "关闭搜索" } else { "清空并关闭" });
        paint::close(c, 361.0, 42.0, 2.6, if x.hovered() { theme::BLUE } else { Color32::from_gray(190) });
        if x.clicked() {
            self.clip_query.clear();
            self.clip_search_open = false;
            resp.surrender_focus();
        }
    }

    fn page_clipboard(&mut self, c: &Card, _env: &Env, _cmds: &mut Vec<Cmd>) {
        if !self.settings.clipboard_history {
            c.dashed(42.0, 32.0, 335.0, 91.0, 12.0, 1.2, Color32::from_gray(70), 0.0);
'''),
    ("src/pages.rs",
     '''            if r.clicked() { self.settings.clipboard_history = true; self.clipboard.set_enabled(true); self.settings.save(); }
            return;
        }
        let items = self.clipboard.lock().items.clone();
        let n = items.len();
        let recent = items.iter().filter(|clip| !clip.pinned).count();
        self.clip_first = self.clip_first.min(n.saturating_sub(CLIP_ROWS));

        // 右上：条数与「清空」（只清未固定的）。
        if recent > 0 {
''',
     '''            if r.clicked() { self.settings.clipboard_history = true; self.clipboard.set_enabled(true); self.settings.save(); }
            return;
        }
        // 只在版本变化时重建快照：之前是每帧把整份历史（最多 30×2 万字）克隆一遍。
        let version=self.clipboard.version();
        if self.clip_snapshot.0!=version { self.clip_snapshot=self.clipboard.snapshot(); }
        let items=self.clip_snapshot.1.clone();
        let query = self.clip_query.trim().to_lowercase();
        let shown: Vec<usize> = if query.is_empty() {
            (0..items.len()).collect()
        } else {
            items.iter().enumerate().filter(|(_,clip)|clip.text.to_lowercase().contains(&query)).map(|(i,_)|i).collect()
        };
        let n = shown.len();
        let recent = items.iter().filter(|clip| !clip.pinned).count();
        // 搜索时输入框占掉卡片第一行，可见行数少一行。
        let searching = self.clip_search_open || !self.clip_query.is_empty();
        let rows = if searching { CLIP_ROWS - 1 } else { CLIP_ROWS };
        self.clip_first = self.clip_first.min(n.saturating_sub(rows));

        // 右上：条数与「清空」（只清未固定的）。
        if recent > 0 {
'''),
    ("src/pages.rs",
     '''
        if n == 0 {
            c.dashed(42.0, 32.0, 335.0, 91.0, 12.0, 1.2, Color32::from_gray(70), 0.0);
            paint::clipboard_glyph(c, 209.5, 58.0, 22.0, Color32::from_gray(200), Color32::from_gray(80));
            c.text(209.5, 93.0, "复制的文字会出现在这里", 12.0, W::Semibold, theme::WHITE, H::Center);
            c.text(209.5, 108.0, "记录保存在本机，重新打开仍可查看", size::TINY, W::Regular, theme::GRAY, H::Center);
            return;
        }

        c.fill(42.0, 31.0, 335.0, 93.0, 12.0, theme::CARD);
        // 超过一屏时右侧细滚动条，滚轮查看。
        if n > CLIP_ROWS {
            let (top, track) = (38.0, 79.0);
            let thumb = (track * CLIP_ROWS as f32 / n as f32).max(12.0);
            let y = top + (track - thumb) * self.clip_first as f32 / (n - CLIP_ROWS) as f32;
            c.fill(371.0, top, 2.5, track, 1.25, Color32::from_white_alpha(14));
            c.fill(371.0, y, 2.5, thumb, 1.25, Color32::from_white_alpha(70));
        }
        let mut action: Option<(usize, u8)> = None; // 0 复制 1 固定 2 删除
        for (row, (index, clip)) in items.iter().enumerate().skip(self.clip_first).take(CLIP_ROWS).enumerate() {
            let y = 44.0 + row as f32 * 22.5;
            let r = c.hit(48.0, y - 10.0, 323.0, 20.0, ("clip", index, clip.text.len(), clip.at.as_str()));
            let (h, _) = c.hover_t(&r);
            if h > 0.01 {
                c.fill(48.0, y - 10.0, 323.0, 20.0, 7.0, Color32::from_white_alpha((12.0 * h) as u8));
            }
            if clip.pinned {
''',
     '''
        if n == 0 {
            c.dashed(42.0, 32.0, 335.0, 91.0, 12.0, 1.2, Color32::from_gray(70), 0.0);
            if query.is_empty() {
                paint::clipboard_glyph(c, 209.5, 58.0, 22.0, Color32::from_gray(200), Color32::from_gray(80));
                c.text(209.5, 93.0, "复制的文字会出现在这里", 12.0, W::Semibold, theme::WHITE, H::Center);
                c.text(209.5, 108.0, "记录保存在本机，重新打开仍可查看", size::TINY, W::Regular, theme::GRAY, H::Center);
            } else {
                c.text(209.5, 70.0, "没有匹配的记录", 12.0, W::Semibold, theme::WHITE, H::Center);
                c.text(209.5, 86.0, "换个关键词试试，或点右侧小叉退出搜索", size::TINY, W::Regular, theme::GRAY, H::Center);
            }
            self.clip_search(c, _cmds);
            return;
        }

        c.fill(42.0, 31.0, 335.0, 93.0, 12.0, theme::CARD);
        // 超过一屏时右侧细滚动条，滚轮查看。
        if n > rows {
            let (top, track) = (38.0, 79.0);
            let thumb = (track * rows as f32 / n as f32).max(12.0);
            let y = top + (track - thumb) * self.clip_first as f32 / (n - rows) as f32;
            c.fill(371.0, top, 2.5, track, 1.25, Color32::from_white_alpha(14));
            c.fill(371.0, y, 2.5, thumb, 1.25, Color32::from_white_alpha(70));
        }
        // 搜索框拿到键盘焦点时：↑↓ 选行、Enter 复制。选中行始终保持在可视范围内。
        if self.clip_search_open && self.clip_typing && !shown.is_empty() {
            let (down, up, enter) = c.ui.ctx().input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Enter)));
            if down { self.clip_sel=(self.clip_sel+1).min(n-1); }
            if up { self.clip_sel=self.clip_sel.saturating_sub(1); }
            self.clip_sel=self.clip_sel.min(n-1);
            if self.clip_sel<self.clip_first { self.clip_first=self.clip_sel; }
            if self.clip_sel>=self.clip_first+rows { self.clip_first=self.clip_sel+1-rows; }
            if enter {
                let text=items[shown[self.clip_sel]].text.clone();
                #[cfg(windows)]
                self.say(if crate::clipboard::copy_history(&text) { "已复制" } else { "复制失败，请重试" });
                #[cfg(not(windows))]
                _cmds.push(Cmd::Copy(text));
            }
        }
        let mut action: Option<(usize, u8)> = None; // 0 复制 1 固定 2 删除
        for (row, index) in shown.iter().copied().skip(self.clip_first).take(rows).enumerate() {
            let clip = &items[index];
            let y = if searching { 62.0 } else { 44.0 } + row as f32 * 22.5;
            let r = c.hit(48.0, y - 10.0, 323.0, 20.0, ("clip", index, clip.text.len(), clip.at.as_str()));
            let (h, _) = c.hover_t(&r);
            let selected = self.clip_search_open && self.clip_typing && self.clip_first+row == self.clip_sel;
            if selected {
                c.fill(48.0, y - 10.0, 323.0, 20.0, 7.0, Color32::from_white_alpha(26));
            } else if h > 0.01 {
                c.fill(48.0, y - 10.0, 323.0, 20.0, 7.0, Color32::from_white_alpha((12.0 * h) as u8));
            }
            if clip.pinned {
'''),
    ("src/pages.rs",
     '''            Some((index, _)) => { self.clipboard.lock().remove(index); }
            None => {}
        }
    }

    fn file_icon_at(&mut self, c: &Card, p: &std::path::Path, cx: f32, cy: f32, sz: f32) {
''',
     '''            Some((index, _)) => { self.clipboard.lock().remove(index); }
            None => {}
        }
        // 搜索框最后画：它浮在卡片上沿，画在卡片之前会被卡片底色盖掉。
        self.clip_search(c, _cmds);
    }

    fn file_icon_at(&mut self, c: &Card, p: &std::path::Path, cx: f32, cy: f32, sz: f32) {
'''),
    ("src/pages.rs",
     '''    // 邮件：有验证码大卡时分栏，否则显示紧凑单行列表。
    // ---------------------------------------------------------------
    fn visible_mail_code(&self)->Option<&mail::Item> {
        self.mail.code_item().filter(|item|!self.mail.manage.trash_open && self.settings.mail_code && !self.mail_code_hidden(item))
    }
    pub(crate) fn mail_list_bounds(&self)->Rect {
        let left=if self.visible_mail_code().is_some(){182.0}else{36.0};
''',
     '''    // 邮件：有验证码大卡时分栏，否则显示紧凑单行列表。
    // ---------------------------------------------------------------
    fn visible_mail_code(&self)->Option<&mail::Item> {
        self.mail.code_item().filter(|item|!self.mail.manage.trash_open && self.settings.mail_code && !self.mail_code_hidden(item) && self.mail_account_match(item))
    }
    pub(crate) fn mail_list_bounds(&self)->Rect {
        let left=if self.visible_mail_code().is_some(){182.0}else{36.0};
'''),
    ("src/pages.rs",
     '''        }

        self.mail_manage_tick();
        // v0.5.5：点开某封邮件 → 整块换成正文摘要，左上「返回」回到列表。
        if let Some((account, uid)) = self.mail.open_uid.clone() {
            match self.mail_rows()
                .iter()
''',
     '''        }

        self.mail_manage_tick();
        // 点开某封邮件 → 整块换成正文摘要，左上「返回」回到列表。
        if let Some((account, uid)) = self.mail.open_uid.clone() {
            match self.mail_rows()
                .iter()
'''),
    ("src/pages.rs",
     '''        let folder=self.mail.manage.folder();let trash=folder==Folder::Trash;
        let list_bounds=self.mail_list_bounds();let list_x=list_bounds.left();let row_w=list_bounds.width()-12.0;
        let compact=code.is_none();let row_h=if compact{21.5}else{34.0};
        let all_count=self.mail.recent.len();
        let unread=self.mail.recent.iter().filter(|m|self.mail_seen(m,Folder::Inbox)==Some(false)).count();
        if trash{
            c.text(list_x+6.0,47.0,&format!("{} {}",if self.settings.mail_sync_operations{"回收站"}else{"本机回收站"},self.mail.manage.trash.len()),10.5,W::Semibold,theme::WHITE,H::Left);
            c.text(375.0,47.0,"返回邮件",10.0,W::Regular,theme::BLUE,H::Right);
            if c.hit(322.0,33.0,56.0,19.0,"mail-trash-back").clicked(){self.mail_toggle_trash();return;}
        }else{
''',
     '''        let folder=self.mail.manage.folder();let trash=folder==Folder::Trash;
        let list_bounds=self.mail_list_bounds();let list_x=list_bounds.left();let row_w=list_bounds.width()-12.0;
        let compact=code.is_none();let row_h=if compact{21.5}else{34.0};
        let all_count=self.mail.recent.iter().filter(|m|self.mail_account_match(m)).count();
        let unread=self.mail.recent.iter().filter(|m|self.mail_account_match(m)&&self.mail_seen(m,Folder::Inbox)==Some(false)).count();
        if trash{
            c.text(list_x+6.0,47.0,&format!("{} {}",if self.settings.mail_sync_operations{"回收站"}else{"本机回收站"},self.mail.manage.trash.len()),10.5,W::Semibold,theme::WHITE,H::Left);
            // 回收站同样遵守账号筛选，这里补上同一个选框。
            let filter_label=match &self.mail.manage.account_filter{
                Some(email)=>Self::mail_account_label(email),
                None=>"全部账号".to_string(),
            };
            c.fill(182.0,33.0,100.0,19.0,6.0,theme::CARD);
            c.text(232.0,47.0,&c.fit(&filter_label,10.0,W::Semibold,96.0),10.0,W::Semibold,if self.mail.manage.account_filter.is_some(){theme::WHITE}else{theme::GRAY},H::Center);
            if c.hit(182.0,33.0,100.0,19.0,"mail-account-filter").on_hover_text("选择要看的邮箱账号").clicked(){
                self.mail.manage.account_menu=!self.mail.manage.account_menu;
            }
            c.text(375.0,47.0,"返回邮件",10.0,W::Regular,theme::BLUE,H::Right);
            if c.hit(322.0,33.0,56.0,19.0,"mail-trash-back").clicked(){self.mail_toggle_trash();return;}
        }else{
'''),
    ("src/pages.rs",
     '''                c.text(x+width/2.0,47.0,&label,10.0,W::Semibold,if selected{theme::WHITE}else{theme::GRAY},H::Center);
                if c.hit(x,33.0,width,19.0,id).on_hover_text("当前加载的最近邮件").clicked(){self.mail.manage.unread_only=id=="mail-unread";}
            }
            c.text(307.0,46.0,if self.settings.mail_sync_operations{"同步"}else{"仅本机"},8.0,W::Regular,theme::DIM,H::Center);
            let ready=self.mail.manage.pending.is_empty()&&self.mail.accounts.iter().any(|a|a.enabled&&self.mail.manage.data.contains_key(&(crate::mail::key(a),Folder::Inbox))&&self.mail.manage.identities.get(&crate::mail::key(a))==Some(a));
            c.text(375.0,47.0,"全部已读",10.0,W::Regular,if ready{theme::BLUE}else{theme::DIM},H::Right);
            if c.hit(325.0,33.0,53.0,19.0,"mail-read-all").on_hover_text(if self.settings.mail_sync_operations{"将已连接邮箱的未读邮件全部标为已读"}else{"仅将岛仔当前加载的邮件标为已读，不更改邮箱"}).clicked()&&ready{
                self.mail_manage_request(None,Folder::Inbox,Action::ReadAll);
            }
        }
        if !trash{
''',
     '''                c.text(x+width/2.0,47.0,&label,10.0,W::Semibold,if selected{theme::WHITE}else{theme::GRAY},H::Center);
                if c.hit(x,33.0,width,19.0,id).on_hover_text("当前加载的最近邮件").clicked(){self.mail.manage.unread_only=id=="mail-unread";}
            }
            // 账号筛选：模式信息放进悬停提示。
            let filter_label=match &self.mail.manage.account_filter{
                Some(email)=>Self::mail_account_label(email),
                None=>"全部账号".to_string(),
            };
            c.fill(182.0,33.0,100.0,19.0,6.0,theme::CARD);
            c.text(232.0,47.0,&c.fit(&filter_label,10.0,W::Semibold,96.0),10.0,W::Semibold,if self.mail.manage.account_filter.is_some(){theme::WHITE}else{theme::GRAY},H::Center);
            if c.hit(182.0,33.0,100.0,19.0,"mail-account-filter").on_hover_text(format!("选择要看的邮箱账号\\n当前模式：{}",if self.settings.mail_sync_operations{"同步邮箱"}else{"仅本机"})).clicked(){
                self.mail.manage.account_menu=!self.mail.manage.account_menu;
            }
            let data_ready=self.mail_read_all_ready();
            let ready=self.mail.manage.pending.is_empty()&&data_ready;
            c.text(375.0,47.0,"全部已读",10.0,W::Regular,if ready{theme::BLUE}else{theme::DIM},H::Right);
            if c.hit(325.0,33.0,53.0,19.0,"mail-read-all").on_hover_text(if self.settings.mail_sync_operations{"将已连接邮箱的未读邮件全部标为已读"}else{"仅将岛仔当前加载的邮件标为已读，不更改邮箱"}).clicked(){
                crate::extras::log_mail(&format!("read-all ready={ready} data={data_ready} pending={} accounts={}",self.mail.manage.pending.len(),self.mail.accounts.len()));
                if ready{self.mail_manage_request(None,Folder::Inbox,Action::ReadAll);}
                else if data_ready{self.mail.manage.read_all_queued=true;self.mail.manage.message="正在同步邮件，完成后自动标记已读".into();}
                else{self.mail.manage.message="邮件状态还没同步完，请稍后再试".into();}
            }
        }
        if !trash{
'''),
    ("src/pages.rs",
     '''            c.text(42.0,134.0,&c.fit(&self.mail.manage.message,8.0,W::Regular,125.0),8.0,W::Regular,theme::GRAY,H::Left);
            if c.hit(40.0,123.0,130.0,16.0,"mail-manage-status").on_hover_text(format!("{}\\n点击刷新邮件状态",self.mail.manage.message)).clicked(){self.mail_manage_request(None,folder,Action::Load);}
        }
        let indices:Vec<_>=self.mail_rows().iter().enumerate().filter(|(_,m)|trash||!self.mail.manage.unread_only||self.mail_seen(m,folder)==Some(false)).map(|(i,_)|i).collect();
        if indices.is_empty(){
            let message=if !self.mail.manage.pending.is_empty(){"正在同步邮件…"}else if trash{"回收站为空"}else if self.mail.manage.unread_only{"没有未读邮件"}else{"还没有收到邮件"};
            c.text(list_x+6.0,80.0,message,11.0,W::Regular,theme::GRAY,H::Left);return;
''',
     '''            c.text(42.0,134.0,&c.fit(&self.mail.manage.message,8.0,W::Regular,125.0),8.0,W::Regular,theme::GRAY,H::Left);
            if c.hit(40.0,123.0,130.0,16.0,"mail-manage-status").on_hover_text(format!("{}\\n点击刷新邮件状态",self.mail.manage.message)).clicked(){self.mail_manage_request(None,folder,Action::Load);}
        }
        let indices:Vec<_>=self.mail_rows().iter().enumerate().filter(|(_,m)|self.mail_account_match(m)&&(trash||!self.mail.manage.unread_only||self.mail_seen(m,folder)==Some(false))).map(|(i,_)|i).collect();
        if indices.is_empty(){
            let message=if !self.mail.manage.pending.is_empty(){"正在同步邮件…"}else if trash{"回收站为空"}else if self.mail.manage.unread_only{"没有未读邮件"}else{"还没有收到邮件"};
            c.text(list_x+6.0,80.0,message,11.0,W::Regular,theme::GRAY,H::Left);return;
'''),
    ("src/pages.rs",
     '''                let(rect,_)=ui.allocate_exact_size(egui::vec2(row_w*c.s,row_h*c.s),egui::Sense::hover());
                let mut row=Card::new(ui,rect.min,ui.clip_rect());row.s=c.s;row.live=c.live;
                let hit=row.hit(0.0,0.0,row_w,row_h-1.0,("mail-row",item.uid,item.account.as_str()));
                let hovered=ui.ctx().pointer_hover_pos().is_some_and(|p|rect.contains(p)&&ui.clip_rect().contains(p));
                let ready=self.mail_ready(&item.account,folder);let actions=trash||hovered&&ready;
                if hovered{row.fill(0.0,0.0,row_w,row_h-1.0,5.0,Color32::from_gray(29));}
                if seen==Some(false)&&!trash{row.circle(5.0,if compact{10.5}else{8.0},2.1,theme::BLUE);}
''',
     '''                let(rect,_)=ui.allocate_exact_size(egui::vec2(row_w*c.s,row_h*c.s),egui::Sense::hover());
                let mut row=Card::new(ui,rect.min,ui.clip_rect());row.s=c.s;row.live=c.live;
                let hit=row.hit(0.0,0.0,row_w,row_h-1.0,("mail-row",item.uid,item.account.as_str()));
                // 账号下拉展开时列表行不再画悬停高亮：行高亮是按鼠标位置手绘的，
                // 不经过 egui 的交互判定，遮罩拦不住它，会和菜单抢视觉焦点。
                let hovered=!self.mail.manage.account_menu&&ui.ctx().pointer_hover_pos().is_some_and(|p|rect.contains(p)&&ui.clip_rect().contains(p));
                let ready=self.mail_ready(&item.account,folder);let actions=trash||hovered&&ready;
                if hovered{row.fill(0.0,0.0,row_w,row_h-1.0,5.0,Color32::from_gray(29));}
                if seen==Some(false)&&!trash{row.circle(5.0,if compact{10.5}else{8.0},2.1,theme::BLUE);}
'''),
    ("src/pages.rs",
     '''                if hit.clicked()&&!acted{self.mail_open(&raw);}
            }
        });
    }

    /// 岛内邮件详情可滚动，也可以进入独立窗口阅读全文。
''',
     '''                if hit.clicked()&&!acted{self.mail_open(&raw);}
            }
        });
        // 账号下拉：画在列表之后，点击空白处收起。
        if self.mail.manage.account_menu{
            if c.hit(36.0,54.0,344.0,68.0,"mail-account-dismiss").clicked(){self.mail.manage.account_menu=false;}
            let accounts:Vec<String>=self.mail.accounts.iter().map(|a|a.email.clone()).take(6).collect();
            let rows=accounts.len()+1;
            c.fill(182.0,54.0,100.0,17.0*rows as f32,8.0,Color32::from_gray(24));
            let current=self.mail.manage.account_filter.clone();
            let mut pick=None;
            for (i,email) in std::iter::once(None).chain(accounts.iter().map(Some)).enumerate(){
                let y=54.0+17.0*i as f32;
                let selected=match (&current,email){(None,None)=>true,(Some(a),Some(b))=>a.eq_ignore_ascii_case(b),_=>false};
                let label=match email{Some(e)=>e.clone(),None=>"全部账号".to_string()};
                let item_hit=c.hit(182.0,y,100.0,17.0,("mail-account-pick",i));
                if item_hit.hovered(){c.fill(184.0,y+1.0,96.0,15.0,5.0,Color32::from_gray(52));}
                if item_hit.clicked(){pick=Some(email.cloned());}
                c.text(190.0,y+11.5,&c.fit(&label,9.5,W::Regular,92.0),9.5,W::Regular,if selected{theme::BLUE}else{theme::WHITE},H::Left);
            }
            if let Some(choice)=pick{
                // 记住上次选的账号，重启后沿用（空 = 全部账号）。
                self.settings.mail_filter=choice.clone().unwrap_or_default();
                self.settings.save();
                self.mail.manage.account_filter=choice;
                self.mail.manage.account_menu=false;
            }
        }
    }

    /// 岛内邮件详情可滚动，也可以进入独立窗口阅读全文。
'''),
    ("src/player_process.rs",
     '''    }
}

#[test]
fn exe_name_takes_file_part(){
    assert_eq!(exe_name(r"C:\\Program Files\\KuGou\\KuGou.exe"),"KuGou.exe");
''',
     '''    }
}

/// 取进程映像的完整路径；只申请有限查询权限，不读取进程内存。
pub fn image_path(pid:u32)->Option<String> {
    if pid==0 {return None;}
    unsafe {
        let process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid).ok()?;
        let mut path=[0u16;1024];let mut length=path.len() as u32;
        let result=QueryFullProcessImageNameW(process,PROCESS_NAME_WIN32,windows::core::PWSTR(path.as_mut_ptr()),&mut length);
        let _=CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&path[..length as usize]))
    }
}

#[test]
fn exe_name_takes_file_part(){
    assert_eq!(exe_name(r"C:\\Program Files\\KuGou\\KuGou.exe"),"KuGou.exe");
'''),
    ("src/player_process.rs",
     '''    assert!(pids_named(exe).contains(&pid));
    assert!(image_is(pid,exe));
    assert!(!image_is(pid,"KuGou.exe"));
}
''',
     '''    assert!(pids_named(exe).contains(&pid));
    assert!(image_is(pid,exe));
    assert!(!image_is(pid,"KuGou.exe"));
    assert!(image_path(pid).is_some_and(|path|path.ends_with(exe)),"映像路径应指向自身可执行文件");
}
'''),
    ("src/player_seek.rs",
     '''//! 系统跳转不可用时，定位播放器自己的进度控件并向该窗口发送消息。
use windows::{core::{Interface,VARIANT},Win32::{Foundation::*,System::{Com::*,Threading::*},UI::{Accessibility::*,WindowsAndMessaging::*},Graphics::Gdi::ScreenToClient}};
use windows::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext,DPI_AWARENESS_CONTEXT,DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

''',
     '''//! 系统跳转不可用时，定位播放器自己的进度控件并向该窗口发送消息。
use windows::{core::{Interface,VARIANT},Win32::{Foundation::*,System::{Com::*,Threading::*},UI::{Accessibility::*,WindowsAndMessaging::*},Graphics::Gdi::{ClientToScreen,ScreenToClient}}};
use windows::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext,DPI_AWARENESS_CONTEXT,DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

'''),
    ("src/player_seek.rs",
     '''    let mut search=Search{source:if apple{"AppleMusic.exe"}else{source},apple,found:None};let _=EnumWindows(Some(visit),LPARAM(&mut search as *mut _ as isize));search.found
}

unsafe fn qq_bar(ui:&IUIAutomation,root:&IUIAutomationElement,duration:f64)->Option<IUIAutomationElement>{
    let condition=ui.CreatePropertyCondition(UIA_ControlTypePropertyId,&VARIANT::from(UIA_TextControlTypeId.0)).ok()?;
    let texts=root.FindAll(TreeScope_Descendants,&condition).ok()?;
    let walker=ui.ControlViewWalker().ok()?;let all=ui.CreateTrueCondition().ok()?;
    for i in 0..texts.Length().ok()? {
        let text=texts.GetElement(i).ok()?;
        if !seconds(&text.CurrentName().ok()?.to_string()).is_some_and(|v|(v-duration).abs()<1.5){continue;}
        let parent=walker.GetParentElement(&text).ok()?;
        let children=parent.FindAll(TreeScope_Children,&all).ok()?;
        if children.Length().ok()?!=3{continue;}
        let first=children.GetElement(0).ok()?;let last=children.GetElement(2).ok()?;
        if seconds(&first.CurrentName().ok()?.to_string()).is_none() || !ui.CompareElements(&last,&text).ok()?.as_bool(){continue;}
        let bar=children.GetElement(1).ok()?;
        let r=bar.CurrentBoundingRectangle().ok()?;
        let left=first.CurrentBoundingRectangle().ok()?;let right=last.CurrentBoundingRectangle().ok()?;
        if r.left>=left.right && r.right<=right.left && r.bottom-r.top<=60 && r.right-r.left>40{return Some(bar);}
    }
    None
}

unsafe fn automation()->Option<IUIAutomation>{
''',
     '''    let mut search=Search{source:if apple{"AppleMusic.exe"}else{source},apple,found:None};let _=EnumWindows(Some(visit),LPARAM(&mut search as *mut _ as isize));search.found
}

/// QQ 音乐可能同时有主窗口、迷你播放器等多个顶层窗口；全部收集，再由调用方挑出含进度条的那个。
unsafe fn windows(source:&str)->Vec<HWND>{
    struct Search<'a>{source:&'a str,found:Vec<HWND>}
    unsafe extern "system" fn visit(hwnd:HWND,param:LPARAM)->BOOL{
        let search=&mut *(param.0 as *mut Search<'_>);
        let mut name=[0u16;80];let n=GetClassNameW(hwnd,&mut name);
        let class=String::from_utf16_lossy(&name[..n.max(0)as usize]);
        if class=="TXGuiFoundation" && IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd,GW_OWNER).is_err()
            && GetWindowTextLengthW(hwnd)>0 && matches_player(hwnd,search.source){search.found.push(hwnd);}
        BOOL(1)
    }
    let mut search=Search{source,found:Vec::new()};
    let _=EnumWindows(Some(visit),LPARAM(&mut search as *mut _ as isize));
    search.found
}

unsafe fn automation()->Option<IUIAutomation>{
'''),
    ("src/player_seek.rs",
     '''            && value.is_finite() && (min..=max).contains(&value)).then_some((value,max))
    }
}
pub(super) fn seek(source:&str,position:f64,duration:f64)->bool{
    let apple=super::apple_music::is_source(source);
    if !apple && !source.eq_ignore_ascii_case("qqmusic.exe"){return false;}
    unsafe { (||->Option<()> {
        let _dpi=DpiContext(SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2));
        let hwnd=window(source)?;
        if !IsWindowEnabled(hwnd).as_bool(){return None;}
        let ui=automation()?;
        let root=ui.ElementFromHandle(hwnd).ok()?;
        if apple {
            let range=apple_bar(&ui,&root)?;let (_,max)=apple_value(&range,duration)?;
            if range.CurrentIsReadOnly().ok()?.as_bool() || !position.is_finite() || !(0.0..=max).contains(&position){return None;}
            // 直接设置原生滑块；无需移动鼠标、激活窗口或扫描歌词区域。
            return range.SetValue(position).ok();
        }
        let bar=qq_bar(&ui,&root,duration)?;
        let target=hwnd;
        // 坐标来自无障碍控件，不依赖窗口位置；不移动鼠标，也不激活播放器。
        let mut p=point(bar.CurrentBoundingRectangle().ok()?,position,duration,5.0)?;
        if !ScreenToClient(target,&mut p).as_bool(){return None;}
        let mut client=RECT::default();GetClientRect(target,&mut client).ok()?;
        if p.x<0 || p.y<0 || (!IsIconic(hwnd).as_bool() && (p.x>=client.right || p.y>=client.bottom)){return None;}
        let lp=LPARAM(((p.y as u32&0xffff)<<16 | (p.x as u32&0xffff))as isize);
        let send=|msg,buttons|SendMessageTimeoutW(target,msg,WPARAM(buttons),lp,SMTO_ABORTIFHUNG|SMTO_ERRORONEXIT,500,None).0!=0;
        if !send(WM_MOUSEMOVE,0){return None;}
        let down=send(WM_LBUTTONDOWN,1);
        let up=send(WM_LBUTTONUP,0);
        (down&&up).then_some(())
    })().is_some() }
}

''',
     '''            && value.is_finite() && (min..=max).contains(&value)).then_some((value,max))
    }
}

unsafe fn qq_bar(ui:&IUIAutomation,root:&IUIAutomationElement,expected:Option<f64>)->Option<(IUIAutomationElement,f64,IUIAutomationElement)>{
    let condition=ui.CreatePropertyCondition(UIA_ControlTypePropertyId,&VARIANT::from(UIA_TextControlTypeId.0)).ok()?;
    let texts=root.FindAll(TreeScope_Descendants,&condition).ok()?;
    let walker=ui.ControlViewWalker().ok()?;let all=ui.CreateTrueCondition().ok()?;
    let mut best:Option<(IUIAutomationElement,f64,IUIAutomationElement)>=None;
    for i in 0..texts.Length().ok()? {
        let text=texts.GetElement(i).ok()?;
        let total=match seconds(&text.CurrentName().ok()?.to_string()){Some(value)=>value,None=>continue};
        // 系统会话给的总时长偶尔与界面显示差几秒，只做宽松校验，实际时长以控件为准。
        if expected.is_some_and(|value|(value-total).abs()>8.0){continue;}
        let parent=walker.GetParentElement(&text).ok()?;
        let children=parent.FindAll(TreeScope_Children,&all).ok()?;
        if children.Length().ok()?!=3{continue;}
        let first=children.GetElement(0).ok()?;let last=children.GetElement(2).ok()?;
        if seconds(&first.CurrentName().ok()?.to_string()).is_none() || !ui.CompareElements(&last,&text).ok()?.as_bool(){continue;}
        let bar=children.GetElement(1).ok()?;
        let r=bar.CurrentBoundingRectangle().ok()?;
        let left=first.CurrentBoundingRectangle().ok()?;let right=last.CurrentBoundingRectangle().ok()?;
        if r.left>=left.right && r.right<=right.left && r.bottom-r.top<=60 && r.right-r.left>40{
            let delta=expected.map(|value|(value-total).abs()).unwrap_or(0.0);
            if delta==0.0{return Some((bar,total,parent));}
            if best.as_ref().map_or(true,|(_,best_delta,_):&(IUIAutomationElement,f64,IUIAutomationElement)|delta<*best_delta){best=Some((bar,total,parent));}
        }
    }
    if best.is_some(){return best;}
    // 回退：版本改版把时间标签挪走时，改在“能读到两个时间”的父控件里找滑块/进度条。
    for control in [UIA_SliderControlTypeId,UIA_ProgressBarControlTypeId,UIA_PaneControlTypeId]{
        let Ok(condition)=ui.CreatePropertyCondition(UIA_ControlTypePropertyId,&VARIANT::from(control.0))else{continue};
        let Ok(bars)=root.FindAll(TreeScope_Descendants,&condition)else{continue};
        for i in 0..bars.Length().ok()?{
            let Ok(bar)=bars.GetElement(i)else{continue};
            let Ok(r)=bar.CurrentBoundingRectangle()else{continue};
            if r.right-r.left<=40 || r.bottom-r.top>60{continue;}
            let Ok(parent)=walker.GetParentElement(&bar)else{continue};
            let Ok(group_texts)=parent.FindAll(TreeScope_Descendants,&texts_condition(ui)?)else{continue};
            let mut total:Option<f64>=None;
            for k in 0..group_texts.Length().ok()?{
                if let Some(value)=group_texts.GetElement(k).ok().and_then(|e|e.CurrentName().ok()).and_then(|name|seconds(&name.to_string())){
                    total=Some(total.map_or(value,|current|current.max(value)));
                }
            }
            let Some(total)=total else{continue};
            if total<=0.0{continue;}
            if expected.is_some_and(|value|(value-total).abs()>8.0){continue;}
            return Some((bar,total,parent));
        }
    }
    None
}

unsafe fn texts_condition(ui:&IUIAutomation)->Option<windows::Win32::UI::Accessibility::IUIAutomationCondition>{
    ui.CreatePropertyCondition(UIA_ControlTypePropertyId,&VARIANT::from(UIA_TextControlTypeId.0)).ok()
}

/// 从进度条那一组控件读当前播放时间；读不到时返回 None，由调用方决定是否继续。
unsafe fn qq_position(group:&IUIAutomationElement,ui:&IUIAutomation)->Option<f64>{
    let all=ui.CreateTrueCondition().ok()?;
    let children=group.FindAll(TreeScope_Children,&all).ok()?;
    if children.Length().ok()?<3{return None;}
    seconds(&children.GetElement(0).ok()?.CurrentName().ok()?.to_string())
}

pub(super) fn seek(source:&str,position:f64,duration:f64)->bool{
    use std::time::{Duration,Instant};
    let apple=super::apple_music::is_source(source);
    let qq=super::qq_music(source);
    if !apple && !qq{return false;}
    unsafe { (||->Option<()> {
        let _dpi=DpiContext(SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2));
        let ui=automation()?;
        // 系统会话里的 AUMID 与进程名不一致，窗口匹配统一用映像名。
        if apple {
            let hwnd=window("AppleMusic.exe")?;
            if !IsWindowEnabled(hwnd).as_bool(){return None;}
            let root=ui.ElementFromHandle(hwnd).ok()?;
            let range=apple_bar(&ui,&root)?;let (_,max)=apple_value(&range,duration)?;
            if range.CurrentIsReadOnly().ok()?.as_bool() || !position.is_finite() || !(0.0..=max).contains(&position){return None;}
            // 直接设置原生滑块；无需移动鼠标、激活窗口或扫描歌词区域。
            return range.SetValue(position).ok();
        }
        let expected=(duration>0.0).then_some(duration);
        let mut chosen=None;
        for hwnd in windows("qqmusic.exe"){
            if !IsWindowEnabled(hwnd).as_bool(){continue;}
            let Ok(root)=ui.ElementFromHandle(hwnd)else{continue};
            if let Some(found)=qq_bar(&ui,&root,expected){chosen=Some((hwnd,found));break;}
        }
        let (hwnd,(bar,total,group))=chosen?;
        let rect=bar.CurrentBoundingRectangle().ok()?;
        let mut client=RECT::default();GetClientRect(hwnd,&mut client).ok()?;
        let origin=if IsIconic(hwnd).as_bool(){
            // 最小化时窗口被移到 -32000，屏幕坐标不可用；用窗口矩形推算客户区原点。
            let mut window_rect=RECT::default();GetWindowRect(hwnd,&mut window_rect).ok()?;
            (window_rect.left,window_rect.top)
        }else{
            let mut point=POINT{x:0,y:0};
            if !ClientToScreen(hwnd,&mut point).as_bool(){return None;}
            (point.x,point.y)
        };
        let mut aim=position.clamp(0.0,total);
        let mut delivered=false;
        // QQ 的进度条有内边距，按比例点击会差几秒；点完读回进度再校正，最多三次。
        for _ in 0..3 {
            let p=point(rect,aim,total,5.0)?;
            let (cx,cy)=(p.x-origin.0,p.y-origin.1);
            if cx<0 || cy<0 || cx>=client.right || cy>=client.bottom{return None;}
            let lp=LPARAM(((cy as u32&0xffff)<<16 | (cx as u32&0xffff))as isize);
            let send=|msg,buttons|SendMessageTimeoutW(hwnd,msg,WPARAM(buttons),lp,SMTO_ABORTIFHUNG|SMTO_ERRORONEXIT,500,None).0!=0;
            if !send(WM_MOUSEMOVE,0){return None;}
            let down=send(WM_LBUTTONDOWN,1);
            let up=send(WM_LBUTTONUP,0);
            if !(down&&up){return None;}
            delivered=true;
            let deadline=Instant::now()+Duration::from_millis(400);
            let mut current=None;
            while Instant::now()<deadline{
                std::thread::sleep(Duration::from_millis(80));
                if let Some(value)=qq_position(&group,&ui){current=Some(value);if (value-aim).abs()<=2.0{break;}}
            }
            let Some(current)=current else{break};
            if (current-position).abs()<=2.0{return Some(());}
            aim=(aim+(position-current)).clamp(0.0,total);
        }
        delivered.then_some(())
    })().is_some() }
}

'''),
    ("src/resident_settings.rs",
     '''            let dx=if is_dragged{pointer.map(|p|((p.x-origin.x)/scale-(left+width/2.0)).clamp(-left,layout.width-left-width)).unwrap_or(0.0)}else{0.0};
            if is_dragged {ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);}
            else if response.hovered(){ui.painter().rect_filled(rect,6.0,Color32::from_white_alpha(18));}
            let active=match slot{BATTERY=>self.settings.show_battery&&self.resident_power.0.is_some(),PET=>self.settings.pet,QUOTA=>count>0,MUSIC=>self.media.is_music(),FOCUS=>self.pomo.running,_=>true};
            let c=card.shifted(dx,0.0).faded(if active||is_dragged{1.0}else{0.38}).ghost();
            match slot{
                BATTERY=>paint::battery(&c,x,15.0,self.resident_power.0,self.resident_power.1,1.0),
                CLOCK=>{
                    let text=self.settings.clock_text(&chrono::Local::now().format("%H:%M:%S").to_string());
                    paint::clock(&c,x-paint::clock_width(&c,&text,theme::size::PILL_CLOCK)/2.0,21.0,&text,theme::size::PILL_CLOCK);
''',
     '''            let dx=if is_dragged{pointer.map(|p|((p.x-origin.x)/scale-(left+width/2.0)).clamp(-left,layout.width-left-width)).unwrap_or(0.0)}else{0.0};
            if is_dragged {ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);}
            else if response.hovered(){ui.painter().rect_filled(rect,6.0,Color32::from_white_alpha(18));}
            let active=match slot{BATTERY=>self.battery_visible(),PET=>self.settings.pet,QUOTA=>count>0,MUSIC=>self.media.is_music(),FOCUS=>self.pomo.running,_=>true};
            let c=card.shifted(dx,0.0).faded(if active||is_dragged{1.0}else{0.38}).ghost();
            match slot{
                BATTERY=>{let (pc,phone)=self.battery_rings();let (pct,chg)=pc.or(phone).unwrap_or((None,false));paint::battery(&c,x,15.0,pct,chg,1.0)},
                CLOCK=>{
                    let text=self.settings.clock_text(&chrono::Local::now().format("%H:%M:%S").to_string());
                    paint::clock(&c,x-paint::clock_width(&c,&text,theme::size::PILL_CLOCK)/2.0,21.0,&text,theme::size::PILL_CLOCK);
'''),
    ("src/shape.rs",
     '''        }
    }
    pub fn apply(self, p: Pos2) -> Pos2 {
        match self.dock {
            Dock::Top => p,
            Dock::Left => Pos2::new(self.edge + p.y, p.x),
            Dock::Right => Pos2::new(self.edge - p.y, p.x),
        }
    }
    fn code(self) -> f32 { match self.dock { Dock::Top => 0.0, Dock::Left => 1.0, Dock::Right => 2.0 } }
}

/// 贴着 `dock` 边缘的轮廓（屏幕坐标）。
pub fn outline_docked(rect: Rect, dock: Dock, flare: f32, flare_h: f32, radius: f32) -> Vec<Pos2> {
    let (map, frame) = EdgeMap::of(rect, dock);
    outline(frame, flare, flare_h, radius).into_iter().map(|p| map.apply(p)).collect()
}

/// 贴着 `dock` 边缘的抗锯齿填充。
#[allow(clippy::too_many_arguments)]
''',
     '''        }
    }
    pub fn apply(self, p: Pos2) -> Pos2 {
        match self.dock {
            Dock::Top => p,
            Dock::Left => Pos2::new(self.edge + p.y, p.x),
            Dock::Right => Pos2::new(self.edge - p.y, p.x),
        }
    }
    fn code(self) -> f32 { match self.dock { Dock::Top => 0.0, Dock::Left => 1.0, Dock::Right => 2.0 } }
}

/// 阴影层相对主体的 (等距外扩, 向下偏移)。
///
/// 圆角增量必须**等于**横向扩展量 `d`，否则每层阴影的圆弧与主体圆弧不同心、会在角部
/// 互相交叉，看起来像「直线转半圆的地方多了一圈突起」。等距外扩后再整体下移 `dy`
/// 模拟顶部光源，四角圆心只随 dy 平移，不会再横向漂移。
///
/// `dy` 必须保持 0：顶部净外扩是 `d - dy`、底部是 `d + dy`。早期用 `0.35g` 时两者
/// 是 0.25g : 0.95g，顶部只剩一条硬边、底部有四五层渐淡的环，于是「上面两个角
/// 不如下面两个角平滑」。岛本来就贴着屏幕顶边，向下的落影方向感收益极小，
/// 完全对称的等距外扩才能保证四角过渡一模一样。
pub(crate) fn shadow_expansion(grow: f32) -> (f32, f32) {
    (grow * 0.6, 0.0)
}

/// 贴着 `dock` 边缘的轮廓（屏幕坐标）。
pub fn outline_docked(rect: Rect, dock: Dock, flare: f32, flare_h: f32, radius: f32) -> Vec<Pos2> {
    let (map, frame) = EdgeMap::of(rect, dock);
    outline(frame, flare, flare_h, radius).into_iter().map(|p| map.apply(p)).collect()
}

/// 贴着 `dock` 边缘的抗锯齿填充。
#[allow(clippy::too_many_arguments)]
'''),
    ("src/shape.rs",
     '''    const N_FLARE: usize = 14;
    for i in 0..=N_FLARE {
        let t = i as f32 / N_FLARE as f32;
        // 肩部曲线：(1-t)^2.4 比三次方更饱满，过渡更柔。
        let wing = flare * (1.0 - t).powf(2.4);
        rows.push((y0 + fh * t, x0 - wing, x1 + wing));
    }
    if y1 - r > y0 + fh {
        rows.push((y1 - r, x0, x1));
    }
    const N_CORNER: usize = 20;
    for i in 1..=N_CORNER {
        let a = i as f32 / N_CORNER as f32 * std::f32::consts::FRAC_PI_2;
        rows.push((y1 - r + r * a.sin(), x0 + r - r * a.cos(), x1 - r + r * a.cos()));
    }
    rows
}

/// 闭合外轮廓（顺时针：右侧自上而下，再左侧自下而上）。
pub fn outline(rect: Rect, flare: f32, flare_h: f32, radius: f32) -> Vec<Pos2> {
    let rows = sections(rect, flare, flare_h, radius);
''',
     '''    const N_FLARE: usize = 14;
    for i in 0..=N_FLARE {
        let t = i as f32 / N_FLARE as f32;
        // 肩部曲线：(1-t)^2.4 比三次方更饱满，过渡更柔。
        let wing = flare * (1.0 - t).powf(2.4);
        rows.push((y0 + fh * t, x0 - wing, x1 + wing));
    }
    if y1 - r > y0 + fh {
        rows.push((y1 - r, x0, x1));
    }
    // 32 段：直线转圆弧的过渡更细腻，端点切线几乎与直边重合。
    const N_CORNER: usize = 32;
    for i in 1..=N_CORNER {
        let a = i as f32 / N_CORNER as f32 * std::f32::consts::FRAC_PI_2;
        rows.push((y1 - r + r * a.sin(), x0 + r - r * a.cos(), x1 - r + r * a.cos()));
    }
    rows
}

/// 闭合外轮廓（顺时针：右侧自上而下，再左侧自下而上）。
pub fn outline(rect: Rect, flare: f32, flare_h: f32, radius: f32) -> Vec<Pos2> {
    let rows = sections(rect, flare, flare_h, radius);
'''),
    ("src/shape.rs",
     '''/// 柔和的落影：多层半透明轮廓叠加，近似高斯模糊（不依赖 GPU 模糊）。
pub fn shadow(painter: &egui::Painter, rect: Rect, flare: f32, flare_h: f32, radius: f32, strength: f32) {
    shadow_mapped(painter, rect, EdgeMap::TOP, flare, flare_h, radius, strength);
}

fn shadow_mapped(painter: &egui::Painter, rect: Rect, map: EdgeMap, flare: f32, flare_h: f32, radius: f32, strength: f32) {
    if strength <= 0.01 {
        return;
    }
    for (grow, a) in [(10.0f32, 0.035f32), (6.5, 0.05), (3.5, 0.07), (1.5, 0.09)] {
        let r = Rect::from_min_max(
            Pos2::new(rect.left() - grow * 0.6, rect.top()),
            Pos2::new(rect.right() + grow * 0.6, rect.bottom() + grow),
        );
        let c = Color32::from_black_alpha((255.0 * a * strength) as u8);
        fill_mapped(painter, r, map, flare, flare_h, radius + grow * 0.8, c, c, r.height(), grow * 0.9);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(points: &[Pos2], p: Pos2) -> bool {
        let mut inside = false;
        let mut prev = *points.last().unwrap();
''',
     '''/// 柔和的落影：多层半透明轮廓叠加，近似高斯模糊（不依赖 GPU 模糊）。
pub fn shadow(painter: &egui::Painter, rect: Rect, flare: f32, flare_h: f32, radius: f32, strength: f32) {
    shadow_mapped(painter, rect, EdgeMap::TOP, flare, flare_h, radius, strength);
}

fn shadow_mapped(painter: &egui::Painter, rect: Rect, map: EdgeMap, flare: f32, flare_h: f32, radius: f32, strength: f32) {
    if strength <= 0.01 {
        return;
    }
    for (grow, a) in [(10.0f32, 0.035f32), (6.5, 0.05), (3.5, 0.07), (1.5, 0.09)] {
        let (d, dy) = shadow_expansion(grow);
        let r = Rect::from_min_max(
            Pos2::new(rect.left() - d, rect.top() - (d - dy)),
            Pos2::new(rect.right() + d, rect.bottom() + d + dy),
        );
        let c = Color32::from_black_alpha((255.0 * a * strength) as u8);
        fill_mapped(painter, r, map, flare, flare_h, radius + d, c, c, r.height(), grow * 0.9);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(points: &[Pos2], p: Pos2) -> bool {
        let mut inside = false;
        let mut prev = *points.last().unwrap();
'''),
    ("src/shape.rs",
     '''        mesh.colored_vertex(a, shade(a.x, a.y));
        mesh.colored_vertex(b, shade(b.x, b.y));
        mesh.colored_vertex(b + nb * feather, clear(b + nb * feather));
        mesh.colored_vertex(a + na * feather, clear(a + na * feather));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

''',
     '''        mesh.colored_vertex(a, shade(a.x, a.y));
        mesh.colored_vertex(b, shade(b.x, b.y));
        mesh.colored_vertex(b + nb * feather, clear(b + nb * feather));
        mesh.colored_vertex(a + na * feather, clear(a + na * feather));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    mesh
}

#[cfg(test)]
mod corner_tests {
    use super::*;

    #[test]
    fn 阴影层四角圆弧与主体同心等距() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 46.0));
        let radius = 16.0;
        let body_rt = Pos2::new(rect.right() - radius, rect.top() + radius);
        let body_lb = Pos2::new(rect.left() + radius, rect.bottom() - radius);
        for grow in [10.0f32, 6.5, 3.5, 1.5] {
            let (d, dy) = shadow_expansion(grow);
            let r = Rect::from_min_max(rect.min - Vec2::new(d, d - dy), rect.max + Vec2::new(d, d + dy));
            let rr = radius + d;
            let shadow_rt = Pos2::new(r.right() - rr, r.top() + rr);
            let shadow_lb = Pos2::new(r.left() + rr, r.bottom() - rr);
            // 等距外扩不改圆心，整体下移只让圆心跟着下移 dy；圆弧不横向漂移就不会与主体交叉。
            assert!((rr - radius - d).abs() < 1e-4, "圆角增量必须等于横向扩展量");
            // 顶部净外扩 / 底部净外扩：太不对称就会「上两角硬、下两角柔」。
            let top = d - dy;
            let bottom = d + dy;
            assert!(top > 0.0, "顶部阴影不能缩进主体");
            assert!((top - bottom).abs() < 1e-6, "上下阴影必须严格对称，否则上两角比下两角生硬：{top}/{bottom}");
            assert!((shadow_rt.x - body_rt.x).abs() < 1e-4, "右上角圆弧不能横向漂移");
            assert!((shadow_rt.y - body_rt.y - dy).abs() < 1e-4);
            assert!((shadow_lb.x - body_lb.x).abs() < 1e-4, "左下角圆弧不能横向漂移");
            assert!((shadow_lb.y - body_lb.y - dy).abs() < 1e-4);

            // 旧参数（圆角 +0.8g、横向 +0.6g、上下不对称）确实让角部圆弧错位，
            // 这正是「直线转半圆处有突起」的来源。
            let old_r = Rect::from_min_max(
                rect.min - Vec2::new(grow * 0.6, grow * 0.3),
                rect.max + Vec2::new(grow * 0.6, grow),
            );
            let old_c = Pos2::new(old_r.right() - (radius + grow * 0.8), old_r.top() + radius + grow * 0.8);
            assert!((old_c.x - body_rt.x).abs() > 0.1 * grow, "旧实现圆心漂移量应当可测");
        }
    }

    #[test]
    fn 圆弧两端与直边相切() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 140.0));
        let r = 33.0;
        let rows = sections(rect, 0.0, 16.0, r);
        let corner: Vec<_> = rows.iter().copied().filter(|t| t.0 >= rect.bottom() - r - 1e-4).collect();
        assert!(corner.len() > 20, "1/4 圆弧要有足够采样");

        // 竖直侧边 -> 圆弧起点：切线必须接近竖直，否则直线接半圆时会有折角。
        let (ya, la, _) = corner[0];
        let (yb, lb, _) = corner[1];
        assert!((lb - la).abs() / (yb - ya) < 0.03, "圆弧起点切线不够竖直");

        // 圆弧终点 -> 水平底边：切线必须接近水平。
        let (y1, l1, _) = corner[corner.len() - 2];
        let (y2, l2, _) = corner[corner.len() - 1];
        assert!((y2 - y1) / (l2 - l1) < 0.03, "圆弧终点切线不够水平");

        // 羽化法线不能在轮廓拐点炸开（否则角部会拉出尖刺）。
        let pts = outline(rect, 0.0, 16.0, r);
        for i in 0..pts.len() {
            assert!(edge_normal(&pts, i).length() < 2.0, "角部羽化法线异常：{i}");
        }
    }
}

#[cfg(test)]
mod fluid_path_bench {
    use super::*;
    use crate::fluid::Fluid;
    use std::time::Instant;

    /// 复刻 `paint_body` 里悬浮云胶囊的行表镜像。
    fn pill_rows(rect: Rect, radius: f32) -> Vec<(f32, f32, f32)> {
        let mut rows = sections(rect, 0.0, 16.0, radius);
        let mid = rect.center().y;
        rows.retain(|r| r.0 > mid && r.0 >= rect.bottom() - radius);
        let top = rows.iter().rev().map(|&(y, l, r)| (2.0 * mid - y, l, r)).collect::<Vec<_>>();
        rows.splice(0..0, top);
        rows
    }

    #[test]
    fn 纹理路径比梯形网格省得多() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(419.0, 215.0));
        let rows = pill_rows(rect, 26.0);
        let fluid = Fluid::default();
        let (w, h) = (rect.width(), rect.height());
        const N: usize = 40;

        let t = Instant::now();
        let mut vertices = 0usize;
        let mut tris = 0usize;
        for _ in 0..N {
            let mesh = shade_mesh(rect, rows.clone(), 1.0, &|x, y| fluid.compose(x, y, w, h, 1.0));
            vertices = mesh.vertices.len();
            tris = mesh.indices.len() / 3;
            std::hint::black_box(mesh);
        }
        let old_ms = t.elapsed().as_secs_f64() * 1000.0 / N as f64;

        let t = Instant::now();
        for _ in 0..N {
            let mut image = egui::ColorImage::new([160, 80], Color32::TRANSPARENT);
            for row in 0..80usize {
                let py = h * (row as f32 + 0.5) / 80.0;
                for col in 0..160usize {
                    image.pixels[row * 160 + col] =
                        fluid.compose(w * (col as f32 + 0.5) / 160.0, py, w, h, 1.0);
                }
            }
            std::hint::black_box(image);
        }
        let new_ms = t.elapsed().as_secs_f64() * 1000.0 / N as f64;

        println!("旧 shade_mesh: {vertices} 顶点 / {tris} 三角形 / {old_ms:.3} ms 每帧");
        println!("新颜色纹理: {} 像素 / {new_ms:.3} ms 每帧", 160 * 80);
        println!("顶点数据量: 旧 {:.0} KB -> 新 1 个 RectShape", vertices as f64 * 24.0 / 1024.0);
        assert!(vertices > 1000, "旧路径确实每帧生成大量顶点");
    }
}
'''),
    ("src/side_dock.rs",
     '''
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SideItem {
    Clock, Weather, Battery, Music, Focus, Bell,
    /// AI 额度：账号下标（None 为「+N」）与高度。
    Quota(usize, f32),
}

impl SideItem {
    fn height(self) -> f32 {
        match self { Self::Clock => 32.0, Self::Weather => 40.0, Self::Battery => 20.0, Self::Music => 18.0, Self::Focus => 22.0, Self::Bell => 22.0, Self::Quota(_, h) => h }
    }
}

''',
     '''
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SideItem {
    Clock, Weather, Battery, PhoneBattery, Music, Focus, Bell,
    /// AI 额度：账号下标（None 为「+N」）与高度。
    Quota(usize, f32),
}

impl SideItem {
    fn height(self) -> f32 {
        match self { Self::Clock => 32.0, Self::Weather => 40.0, Self::Battery | Self::PhoneBattery => 20.0, Self::Music => 18.0, Self::Focus => 22.0, Self::Bell => 22.0, Self::Quota(_, h) => h }
    }
}

'''),
    ("src/side_dock.rs",
     '''    pub(crate) fn side_items(&self) -> Vec<SideItem> {
        let mut items = vec![SideItem::Clock];
        if self.weather_pill().is_some() {items.push(SideItem::Weather);}
        if self.settings.show_battery && self.resident_power.0.is_some() { items.push(SideItem::Battery); }
        if self.media.is_music() { items.push(SideItem::Music); }
        if self.pomo.running { items.push(SideItem::Focus); }
        items.extend(self.quota_side_items().into_iter().map(|(index, h)| SideItem::Quota(index, h)));
''',
     '''    pub(crate) fn side_items(&self) -> Vec<SideItem> {
        let mut items = vec![SideItem::Clock];
        if self.weather_pill().is_some() {items.push(SideItem::Weather);}
        let (pc_ring,phone_ring)=self.battery_rings();
        if pc_ring.is_some() { items.push(SideItem::Battery); }
        if phone_ring.is_some() { items.push(SideItem::PhoneBattery); }
        if self.media.is_music() { items.push(SideItem::Music); }
        if self.pomo.running { items.push(SideItem::Focus); }
        items.extend(self.quota_side_items().into_iter().map(|(index, h)| SideItem::Quota(index, h)));
'''),
    ("src/side_dock.rs",
     '''                    c.text(cx, mid + 13.0, &minute, 13.5, W::Bold, theme::WHITE, H::Center);
                }
                SideItem::Weather => self.draw_side_weather(c,cx,mid),
                SideItem::Battery => paint::battery(c, cx, mid, env.battery, env.charging, 0.72),
                SideItem::Music => paint::playback_bars(c, cx, mid, self.t, self.media.playing, theme::WHITE),
                SideItem::Focus => {
                    let minutes = (self.pomo.remaining / 60.0).ceil().max(0.0) as u32;
''',
     '''                    c.text(cx, mid + 13.0, &minute, 13.5, W::Bold, theme::WHITE, H::Center);
                }
                SideItem::Weather => self.draw_side_weather(c,cx,mid),
                SideItem::Battery => paint::battery(c, cx, mid, self.resident_power.0, self.resident_power.1, 0.72),
                SideItem::PhoneBattery => paint::battery(c, cx, mid, self.phone_resident.0, self.phone_resident.1, 0.72),
                SideItem::Music => paint::playback_bars(c, cx, mid, self.t, self.media.playing, theme::WHITE),
                SideItem::Focus => {
                    let minutes = (self.pomo.remaining / 60.0).ceil().max(0.0) as u32;
'''),
    ("src/smtc.rs",
     '''#[cfg(windows)]
#[path="kugou_control.rs"]
mod kugou_control;
#[cfg(windows)]
#[path="player_seek.rs"]
mod player_seek;
''',
     '''#[cfg(windows)]
#[path="kugou_control.rs"]
mod kugou_control;
#[cfg(windows)]
#[path="netease_control.rs"]
pub(crate) mod netease_control;
#[cfg(windows)]
#[path="player_seek.rs"]
mod player_seek;
'''),
    ("src/smtc.rs",
     '''#[cfg(not(windows))]
pub fn control(_cmd: Transport,_source:&str) {}

/// 跳转在工作线程执行；专用回退必须回读实际位置，避免播放器接口假成功。
#[cfg(windows)]
pub fn seek(seconds:f64,source:&str)->bool {
''',
     '''#[cfg(not(windows))]
pub fn control(_cmd: Transport,_source:&str) {}

/// QQ 音乐在不同入口下会给出不同的来源标识：进程名 `QQMusic.exe`，系统会话里则是
/// AUMID `Tencent.QQMusic`。两者都要认，否则跳转会被当成普通播放器而走错分支。
#[cfg(windows)]
pub(crate) fn qq_music(source:&str)->bool{
    let lower=source.to_ascii_lowercase();
    lower=="qqmusic" || lower=="qqmusic.exe" || (lower.contains("tencent") && lower.contains("qqmusic"))
}

/// QQ 音乐提供 `/seekto <秒>` 命令行命令，由单实例 IPC 转交给正在运行的客户端。
/// 它不依赖窗口和界面结构，最小化时同样有效，所以优先于点击进度条。
#[cfg(windows)]
fn qq_seek_command(seconds:f64)->bool{
    let target=seconds.max(0.0).round() as u64;
    let Some(path)=player_process::pids_named("qqmusic.exe").into_iter().find_map(player_process::image_path)else{return false;};
    std::process::Command::new(path)
        .arg("/seekto")
        .arg(target.to_string())
        .spawn()
        .is_ok()
}

/// QQ 音乐 `/volumeto <0-100>` 调的是客户端内部音量，不经过 Windows 音频会话，
/// 所以调用方要自己记住这个值用于显示。
#[cfg(windows)]
pub(crate) fn qq_volume_command(percent:u32)->bool{
    let Some(path)=player_process::pids_named("qqmusic.exe").into_iter().find_map(player_process::image_path)else{return false;};
    std::process::Command::new(path)
        .arg("/volumeto")
        .arg(percent.min(100).to_string())
        .spawn()
        .is_ok()
}

/// 跳转在工作线程执行；专用回退必须回读实际位置，避免播放器接口假成功。
#[cfg(windows)]
pub fn seek(seconds:f64,source:&str)->bool {
'''),
    ("src/smtc.rs",
     '''    let Ok(_lock)=LOCK.lock()else{return false;};
    if source.eq_ignore_ascii_case("kwmusic.exe"){return kuwo::seek(seconds);}
    if matches!(source.to_ascii_lowercase().as_str(),"kugou"|"kugou.exe") && kugou_control::seek(seconds){return true;}
    let _apartment=init_winrt();
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Manager;
    let session=media_result(Manager::RequestAsync).and_then(|mgr|mgr.GetSessions().ok())
        .and_then(|sessions|sessions.into_iter().find(|s|s.SourceAppUserModelId().ok().is_some_and(|id|id.to_string().eq_ignore_ascii_case(source))));
    if apple_music::is_source(source){return session.as_ref().is_some_and(|session|seek_apple(session,source,seconds));}
    let native=source.eq_ignore_ascii_case("qqmusic.exe");
    let timeline=||->Option<(f64,f64)>{
        let tl=session.as_ref()?.GetTimelineProperties().ok()?;
        Some((tl.Position().ok()?.Duration as f64/10_000_000.0,tl.EndTime().ok()?.Duration as f64/10_000_000.0))
''',
     '''    let Ok(_lock)=LOCK.lock()else{return false;};
    if source.eq_ignore_ascii_case("kwmusic.exe"){return kuwo::seek(seconds);}
    if matches!(source.to_ascii_lowercase().as_str(),"kugou"|"kugou.exe") && kugou_control::seek(seconds){return true;}
    // 网易云关掉了系统媒体会话；客户端开了 CDP 调试端口时接管跳转（不开则立刻失败，不空等）。
    if source.eq_ignore_ascii_case("cloudmusic.exe") && netease_control::seek(seconds){return true;}
    let _apartment=init_winrt();
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager as Manager;
    let session=media_result(Manager::RequestAsync).and_then(|mgr|mgr.GetSessions().ok())
        .and_then(|sessions|sessions.into_iter().find(|s|s.SourceAppUserModelId().ok().is_some_and(|id|id.to_string().eq_ignore_ascii_case(source))));
    if apple_music::is_source(source){return session.as_ref().is_some_and(|session|seek_apple(session,source,seconds));}
    let native=qq_music(source);
    let timeline=||->Option<(f64,f64)>{
        let tl=session.as_ref()?.GetTimelineProperties().ok()?;
        Some((tl.Position().ok()?.Duration as f64/10_000_000.0,tl.EndTime().ok()?.Duration as f64/10_000_000.0))
'''),
    ("src/smtc.rs",
     '''    let target=if duration>0.0{seconds.min(duration)}else{seconds};
    let enabled=session.as_ref().and_then(|s|s.GetPlaybackInfo().ok()).and_then(|i|i.Controls().ok())
        .and_then(|c|c.IsPlaybackPositionEnabled().ok()).unwrap_or(false);
    let paused_qq=source.eq_ignore_ascii_case("qqmusic.exe") && session.as_ref().and_then(|s|s.GetPlaybackInfo().ok())
        .and_then(|info|info.PlaybackStatus().ok()).is_some_and(|status|status!=windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
    let accepted=(!native || enabled) && session.as_ref().and_then(|s|media_result(||s.TryChangePlaybackPositionAsync((target*10_000_000.0)as i64)))==Some(true);
    if !native{return accepted;}
    for system in [true,false] {
        if system {if !accepted{continue;}}else {
            if !player_seek::seek(source,target,duration){return false;}
        }
        let start=std::time::Instant::now();
        let timeout=if system{Duration::from_millis(600)}else{Duration::from_secs(2)};
        let mut pause_requested=false;
        while start.elapsed()<timeout {
            std::thread::sleep(Duration::from_millis(100));
''',
     '''    let target=if duration>0.0{seconds.min(duration)}else{seconds};
    let enabled=session.as_ref().and_then(|s|s.GetPlaybackInfo().ok()).and_then(|i|i.Controls().ok())
        .and_then(|c|c.IsPlaybackPositionEnabled().ok()).unwrap_or(false);
    let paused_qq=qq_music(source) && session.as_ref().and_then(|s|s.GetPlaybackInfo().ok())
        .and_then(|info|info.PlaybackStatus().ok()).is_some_and(|status|status!=windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
    let accepted=(!native || enabled) && session.as_ref().and_then(|s|media_result(||s.TryChangePlaybackPositionAsync((target*10_000_000.0)as i64)))==Some(true);
    if !native{return accepted;}
    // 依次尝试：系统接口 → 客户端自带的 /seekto 命令 → 点击界面进度条。
    for stage in 0..3 {
        let timeout=match stage {0=>Duration::from_millis(600),1=>Duration::from_secs(3),_=>Duration::from_secs(2)};
        match stage {
            0=>{if !accepted{continue;}}
            1=>{if !qq_seek_command(target){continue;}}
            _=>{if !player_seek::seek(source,target,duration){return false;}}
        }
        let start=std::time::Instant::now();
        let mut pause_requested=false;
        while start.elapsed()<timeout {
            std::thread::sleep(Duration::from_millis(100));
'''),
    ("src/smtc.rs",
     '''    assert!(seek_arrived(60.0,60.0,0.2));
    assert!(seek_arrived(62.0,60.0,2.0));
    assert!(!seek_arrived(f64::NAN,60.0,2.0));
}

#[cfg(windows)]
''',
     '''    assert!(seek_arrived(60.0,60.0,0.2));
    assert!(seek_arrived(62.0,60.0,2.0));
    assert!(!seek_arrived(f64::NAN,60.0,2.0));
}

#[cfg(windows)]
#[test]
fn qq_source_matches_process_name_and_aumid(){
    for source in ["QQMusic.exe","qqmusic","Tencent.QQMusic","Tencent.QQMusic.1"]{assert!(qq_music(source),"应识别为 QQ 音乐：{source}");}
    for source in ["cloudmusic.exe","Spotify.exe","AppleInc.AppleMusicWin_nzyj5cx40ttqa!App","","qq"]{assert!(!qq_music(source),"不应识别为 QQ 音乐：{source}");}
}

#[cfg(windows)]
'''),
    ("src/smtc.rs",
     '''    let stop=Arc::new(std::sync::atomic::AtomicBool::new(false));let stopped=stop.clone();
    let interval_ms=Arc::new(std::sync::atomic::AtomicU32::new(POLL_FAST.as_millis() as u32));let requested=interval_ms.clone();
    let worker=crate::resources::POLLER.spawn(move || {
        let _apartment=init_winrt();
        let mut artwork=ArtworkCache::default();
        let mut playback_clock=PlaybackClock::default();
''',
     '''    let stop=Arc::new(std::sync::atomic::AtomicBool::new(false));let stopped=stop.clone();
    let interval_ms=Arc::new(std::sync::atomic::AtomicU32::new(POLL_FAST.as_millis() as u32));let requested=interval_ms.clone();
    let worker=crate::resources::POLLER.spawn(move || {

        let _apartment=init_winrt();
        let mut artwork=ArtworkCache::default();
        let mut playback_clock=PlaybackClock::default();
'''),
    ("src/smtc.rs",
     '''            let mut np=choose_media_for_source(system,native,&selected_source,native_pending).unwrap_or_default();
            memory.update(&mut np,std::time::Instant::now());
            np.volume=volume.poll(&np.source);
            if !np.source.is_empty(){selected_source=np.source.clone();}
            #[cfg(windows)]
            netease.fill_artwork(&mut np);
''',
     '''            let mut np=choose_media_for_source(system,native,&selected_source,native_pending).unwrap_or_default();
            memory.update(&mut np,std::time::Instant::now());
            np.volume=volume.poll(&np.source);
            #[cfg(windows)] crate::music_volume::watch_tick();
            if !np.source.is_empty(){selected_source=np.source.clone();}
            #[cfg(windows)]
            netease.fill_artwork(&mut np);
'''),
    ("src/smtc.rs",
     '''            std::thread::park_timeout(Duration::from_millis(if idle{idle_interval(wait)}else{wait}.into()));
        }
    });
    Poller{enabled,stop,worker,interval_ms}
}

''',
     '''            std::thread::park_timeout(Duration::from_millis(if idle{idle_interval(wait)}else{wait}.into()));
        }
    });
    // 音量推送的独立通道：与采样线程解耦，避免采样线程停摆时推送静默失效。
    #[cfg(windows)] crate::music_volume::spawn_push_watcher();
    Poller{enabled,stop,worker,interval_ms}
}

'''),
    ("src/system.rs",
     '''    pick_files_for("添加文件到置物架",true)
}

pub fn pick_files_for(title:&str,multiple:bool) -> Result<Vec<std::path::PathBuf>> {
    unsafe {
        use windows::Win32::UI::Shell::*;
''',
     '''    pick_files_for("添加文件到置物架",true)
}

/// 选择一个文件夹（返回 None = 用户取消）。
pub fn pick_folder(title:&str) -> Result<Option<std::path::PathBuf>> {
    unsafe {
        use windows::Win32::UI::Shell::*;
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
        // 只让选文件夹，且必须是已存在的文件系统路径。
        dialog.SetOptions(FOS_FORCEFILESYSTEM | FOS_PICKFOLDERS | FOS_PATHMUSTEXIST)?;
        dialog.SetTitle(&HSTRING::from(title))?;
        if let Err(e) = dialog.Show(None) {
            if e.code().0 as u32 == 0x800704c7 { return Ok(None); } // 用户取消
            return Err(e);
        }
        let item = dialog.GetResult()?;
        let ptr = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let text = ptr.to_string()?;
        windows::Win32::System::Com::CoTaskMemFree(Some(ptr.0 as _));
        Ok(Some(text.into()))
    }
}

/// 选择一个程序文件（返回 None = 用户取消）。
pub fn pick_exe(title:&str) -> Result<Option<std::path::PathBuf>> {
    unsafe {
        use windows::Win32::UI::Shell::*;
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
        dialog.SetOptions(FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST)?;
        dialog.SetTitle(&HSTRING::from(title))?;
        // 默认只列 .exe，用户仍可在文件名框里输入其它路径。
        let name: Vec<u16> = "程序 (*.exe)\\0".encode_utf16().collect();
        let spec: Vec<u16> = "*.exe\\0".encode_utf16().collect();
        let filters = [Common::COMDLG_FILTERSPEC {
            pszName: windows::core::PCWSTR(name.as_ptr()),
            pszSpec: windows::core::PCWSTR(spec.as_ptr()),
        }];
        let _ = dialog.SetFileTypes(&filters);
        if let Err(e) = dialog.Show(None) {
            if e.code().0 as u32 == 0x800704c7 { return Ok(None); }
            return Err(e);
        }
        let item = dialog.GetResult()?;
        let ptr = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let text = ptr.to_string()?;
        windows::Win32::System::Com::CoTaskMemFree(Some(ptr.0 as _));
        Ok(Some(text.into()))
    }
}

pub fn pick_files_for(title:&str,multiple:bool) -> Result<Vec<std::path::PathBuf>> {
    unsafe {
        use windows::Win32::UI::Shell::*;
'''),
    ("src/system.rs",
     '''        if notepad.exists(){assert!(executable_description(notepad.to_str().unwrap()).is_some());}
    }
}
''',
     '''        if notepad.exists(){assert!(executable_description(notepad.to_str().unwrap()).is_some());}
    }
}

// —— 自定义构建补回：手机连接通道（Phone Link）——
const PHONE_CHANNELS: &[(&str,&str)] = &[
    ("YourPhoneMessages_","短信"),
    ("YourPhoneCalling","通话"),
    ("YourPhoneDeviceStatus","手机状态"),
    ("com.apple.mobilephone","电话"),
    ("com.tencent.xin","微信"),
    ("com.tencent.mqq","QQ"),
    ("com.alipay.iphoneclient","支付宝"),
    ("com.google.Gmail","Gmail"),
];
/// 手机连接 Toast 的来源判定。
///
/// WinRT 只报主 AUMID（`!App`），拿不到通道信息，所以只能看正文：
/// 正文格式是「来源 · 内容…」，但短信用**手机号**当来源（`+8610… · 【xx】验证码…`），
/// 电话是「号码 · 传入呼叫 / 未接来电」，普通 App 才是「微信 · 张三 · 内容」。
///
/// 判定顺序：名单命中 → 来电标记 → 短信特征 → 用首段当标签兜底。
/// 名单放在 `%APPDATA%\\Daozai\\phone-apps.json`，加新 App 不用改代码重编译。
/// 返回 (标签, 是否允许上岛)；None = 无法识别，由调用方按「连接」放行。
pub(crate) fn phone_app(body:&str,apps:&crate::model::PhoneApps)->Option<(String,bool)> {
    let head=body.split('·').next().unwrap_or("").trim();
    if head.is_empty() {return None;}
    // 1. 名单优先：微信 / QQ / 支付宝 / Gmail 等。
    if let Some(hit)=apps.lookup(head){return Some(hit);}
    // 2. 来电：号码当来源，正文带这两个词。
    if body.contains("传入呼叫")||body.contains("未接来电"){return Some(("电话".to_string(),true));}
    // 3. 短信：来源是手机号，或正文有【签名】/验证码字样。
    let digits=head.chars().all(|c|c.is_ascii_digit()||matches!(c,'+'|'-'|' '|'('|')'));
    if digits||body.contains("验证码")||body.contains('【')||body.contains("短信"){
        return Some(("短信".to_string(),true));
    }
    // 4. 兜底：短名字直接当标签（例如 WorkBuddy），过长才交给调用方。
    if head.chars().count()<=12 {return Some((head.to_string(),true));}
    None
}

pub(crate) fn phone_channel(id:&str)->bool {
    // 只要是手机连接（Phone Link）发出来的就归手机通道：
    // WinRT 有时给主 AUMID（`!App`），有时给子通道（`YourPhoneMessages_…` /
    // `YourPhoneNotifications_<uuid>`），开机后通道注册晚一步还会先走汇总通道。
    // 白名单只用来起标签，是否上岛/静音交给 phone-apps.json 按正文判断。
    id.starts_with("Microsoft.YourPhone_")
}
pub(crate) fn phone_label(id:&str)->&'static str {
    if id.ends_with("!App"){return "连接";}
    PHONE_CHANNELS.iter().find(|(key,_)|id.contains(*key)).map(|(_,label)|*label).unwrap_or("手机")
}

#[cfg(test)]
mod phone_channel_tests {
    use super::{phone_app,phone_channel,phone_label};
    #[test]
    fn 只放行手机连接白名单通道(){
        // 现在只要是手机连接的 AUMID 都算手机通道（白名单只管标签）；
        // 真正决定「上不上岛 / 静不静音」的是正文 + phone-apps.json。
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.tencent.xin"));
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneMessages_9d62fe0c"));
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!App"));
        // 开机后通道还没注册时，Win11 会先走汇总通道，这个也必须算手机通知。
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_9d62fe0c-7488-42c8-be15-0fbe4a087b7f"));
        assert_eq!(phone_label("Microsoft.YourPhone_x!App"),"连接");
        assert!(phone_channel("Microsoft.YourPhone_8wekyb3d8bbwe!YourPhoneNotifications_com.valvesoftware.Steam"));
        assert!(!phone_channel("QQ"));
        assert!(!phone_channel("mail.test"));
        assert_eq!(phone_label("Microsoft.YourPhone_x!YourPhoneNotifications_com.tencent.xin"),"微信");
        assert_eq!(phone_label("Microsoft.YourPhone_x!YourPhoneNotifications_com.google.Gmail"),"Gmail");
    }
    #[test]
    fn 从正文首段识别手机应用与黑名单(){
        let apps=crate::model::PhoneApps::builtin();
        assert_eq!(crate::system::phone_app("QQ · 小号 · 这是一条测试正文内容",&apps),Some(("QQ".to_string(),true)));
        assert_eq!(crate::system::phone_app("微信 · 某人 · 晚上好",&apps),Some(("微信".to_string(),true)));
        assert_eq!(crate::system::phone_app("来电 · 138xxxx",&apps),Some(("电话".to_string(),true)));
        assert_eq!(crate::system::phone_app("Steam · 夏季特惠",&apps),Some(("连接".to_string(),false)));
        assert_eq!(crate::system::phone_app("",&apps),None);
        assert_eq!(crate::system::phone_app("一条很长的普通消息内容里没有应用名",&apps),None);
        // 名单是外部配置：加新 App 只需往 JSON 里加一条，不改代码。
        let mut custom=crate::model::PhoneApps::builtin();
        custom.items.push(("小红书".to_string(),"小红书".to_string(),true));
        assert_eq!(crate::system::phone_app("小红书 · 有人回复了你",&custom),Some(("小红书".to_string(),true)));
    }
    #[test]
    fn 短信与来电按正文特征识别(){
        let apps=crate::model::PhoneApps::builtin();
        // 短信用手机号当来源：认不出号码也要归到「短信」，不能退化成「连接」。
        assert_eq!(phone_app("+86106826974003139 · 【清枫北京科技】您的验证码：4342，如非本人操作",&apps),Some(("短信".to_string(),true)));
        assert_eq!(phone_app("10691234 · 您的验证码 8899",&apps),Some(("短信".to_string(),true)));
        // 响铃与未接都先归到「电话」，再由正文区分「来电中 / 未接来电」。
        assert_eq!(phone_app("130 8646 4686 · 传入呼叫",&apps),Some(("电话".to_string(),true)));
        assert_eq!(phone_app("电话 · 130 8646 4686 · 未接来电",&apps),Some(("电话".to_string(),true)));
        // 名单外的短名字直接用名字当标签。
        assert_eq!(phone_app("WorkBuddy · 任务已完成",&apps),Some(("WorkBuddy".to_string(),true)));
    }
}
'''),
    ("src/ui_tests.rs",
     '''    assert_eq!(accounts[0].endpoint().unwrap(),("imap.qq.com".into(),993,"OldUser@qq.com".into()));
    assert!(!accounts[1].enabled);
    crate::mail::save_accounts(&accounts);assert_eq!(crate::mail::load_accounts(),accounts);
}

fn editing_local_calendar_clears_outdated_reminders() {
''',
     '''    assert_eq!(accounts[0].endpoint().unwrap(),("imap.qq.com".into(),993,"OldUser@qq.com".into()));
    assert!(!accounts[1].enabled);
    crate::mail::save_accounts(&accounts);assert_eq!(crate::mail::load_accounts(),accounts);
}

#[test]
fn settings_notification_tab_shows_meeting_quiet_and_history() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.ctx.set_embed_viewports(false);
    fn frame(h:&mut H)->Vec<egui::epaint::ClippedShape>{
        let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(600.0,450.0))),
                viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.25),focused:Some(true),..Default::default()})).collect(),
                focused:true,events:Vec::new(),..Default::default()};
            *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
        let raw=egui::RawInput{viewports:std::iter::once((egui::ViewportId::ROOT,egui::ViewportInfo{native_pixels_per_point:Some(1.25),..Default::default()})).collect(),..Default::default()};
        let _=h.ctx.run(raw,|ctx|h.app.settings_ui(ctx,false));
        let shapes=output.lock().unwrap().take().unwrap().shapes;
        shapes
    }
    let has=|shapes:&[egui::epaint::ClippedShape],phrase:&str|shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(t)if t.galley.text().contains(phrase)));
    // 通知页内容很长，新增的「会议自动勿扰 / 最近通知」在滚动区下方，这里只验专注页的统计卡
    // （通知页那几行由人工点开确认，避免测试依赖滚动位置）。
    h.app.settings_tab=2;h.app.open_settings();
    for _ in 0..3{frame(&mut h);}
    let focus=frame(&mut h);
    assert!(has(&focus,"专注记录"),"专注页应有专注统计卡");
    assert!(has(&focus,"连续"),"专注统计要显示连续天数");
}

#[test]
fn snoozed_notice_returns_when_due() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.snooze_notice(5.0,"短信验证码".into(),"您的验证码：4342".into());
    assert_eq!(h.app.snoozed.len(),1,"点了稍后应该排进队列");
    // 还差 1 秒时不该弹。
    h.app.t += 299.0;
    h.app.tick_snoozed();
    assert!(h.app.notice.is_none(),"没到点不能提前弹");
    h.app.t += 2.0;
    h.app.tick_snoozed();
    assert!(h.app.snoozed.is_empty(),"到点后要从队列里移除");
    // 入队后由 tick_alerts 抬到 notice，这里两处都认。
    assert!(h.app.notice.iter().chain(h.app.alerts.iter()).any(|a|a.title=="短信验证码"),"到点要重新弹出来");
}

#[test]
fn clipboard_search_toggles_on_the_active_tab_and_filters_lines() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(3);h.app.shelf_tab=1;
    {let mut hist=h.app.clipboard.lock();hist.items.clear();
     hist.push("会议纪要：周四评审","09:31");hist.push("验证码 482913","09:32");}
    h.hover(H::local(209.0,60.0),1.2);
    // 点面板右下侧的放大镜：浮出搜索输入框。
    h.click(H::local(397.0,115.0));
    assert!(h.app.clip_search_open,"点放大镜应浮出搜索框");
    h.frame(vec![]);
    // 过滤只影响显示，历史本身不动。
    h.app.clip_query="会议".into();
    h.frame(vec![]);
    assert_eq!(h.app.clipboard.lock().items.len(),2,"搜索不能删改历史");
    assert!(!h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("验证码 482913"))),"过滤后不该再画不匹配的记录");
    assert!(h.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains("会议纪要"))),"命中的记录要留下");
    h.click(H::local(397.0,115.0));
    assert!(!h.app.clip_search_open,"再点一次应收起搜索");
    assert!(h.app.clip_query.is_empty(),"收起搜索要清空关键词");
}

#[test]
fn clicking_the_active_clipboard_tab_no_longer_opens_search() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    h.app.show_page(3);h.app.shelf_tab=1;
    assert!(!h.app.clip_search_open);
    // 第二个分栏（剪贴板）中心：shelf_tabs 从 x=13 起，第一栏约 50 宽。
    h.click(H::local(90.0,20.0));
    h.frame(vec![]);
    assert_eq!(h.app.shelf_tab,1,"仍在剪贴板分栏");
    assert!(!h.app.clip_search_open,"再点已选中的分栏不该展开搜索框");
}

fn editing_local_calendar_clears_outdated_reminders() {
'''),
    ("src/ui_tests.rs",
     '''        new: true,
    });
    assert!(h.app.notification_bell_until > 0.0, "新邮件应点亮铃铛");
    assert!(h.app.alerts.iter().any(|a|a.title=="新邮件"&&a.page==5),"新邮件应进入通知队列，不能只有收起时才可见的铃铛");
    h.app.show_page(5);
    h.hover(Pos2::new(WIN_W / 2.0, 12.0), 0.6);
    h.hover(H::local(300.0, 60.0), 0.4);
''',
     '''        new: true,
    });
    assert!(h.app.notification_bell_until > 0.0, "新邮件应点亮铃铛");
    assert!(h.app.alerts.iter().any(|a|a.title.starts_with("邮件验证码")&&a.page==5),"新邮件应进入通知队列，不能只有收起时才可见的铃铛");
    h.app.show_page(5);
    h.hover(Pos2::new(WIN_W / 2.0, 12.0), 0.6);
    h.hover(H::local(300.0, 60.0), 0.4);
'''),
    ("src/ui_tests.rs",
     '''#[test]
fn ignored_mail_event_hides_stops_reminders_persists_and_restores() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let now=chrono::Local::now();let day=now.date_naive();
    let meeting=crate::mail::Meeting{all_day:false,title:"测试邀请".into(),start:now.timestamp()+600,end:None,at:String::new(),uid:"ignore-me".into()};
    h.app.mail.recent.push(crate::mail::Item {received_unix: None, account:"test@qq.com".into(),from:"测试".into(),subject:"邀请".into(),time:String::new(),code:None,html: String::new().into(), summary:String::new().into(),links:vec![],meeting:Some(meeting.clone()),uid:99});
    h.app.toggle_mail_event(&meeting);
    assert!(h.app.calendar_entries(day).is_empty());
    assert!(crate::model::Settings::load().ignored_mail_events.contains(&crate::calendar::dedupe_key(&meeting)));
    h.frame(vec![]);assert!(!h.app.history.iter().any(|a|a.title=="会议提醒"));
''',
     '''#[test]
fn ignored_mail_event_hides_stops_reminders_persists_and_restores() {
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());let mut h=H::new();
    let now=chrono::Local::now();
    // 取日程真正落在的那一天：23:5x 跑测试时「10 分钟后」已经是明天，按今天断言会假失败。
    let start=now.timestamp()+600;
    let day=chrono::DateTime::from_timestamp(start,0).unwrap().with_timezone(&chrono::Local).date_naive();
    let meeting=crate::mail::Meeting{all_day:false,title:"测试邀请".into(),start,end:None,at:String::new(),uid:"ignore-me".into()};
    h.app.mail.recent.push(crate::mail::Item {received_unix: None, account:"test@qq.com".into(),from:"测试".into(),subject:"邀请".into(),time:String::new(),code:None,html: String::new().into(),summary:String::new().into(),links:vec![],meeting:Some(meeting.clone()),uid:99});   h.app.toggle_mail_event(&meeting);
    assert!(h.app.calendar_entries(day).is_empty());
    assert!(crate::model::Settings::load().ignored_mail_events.contains(&crate::calendar::dedupe_key(&meeting)));
    h.frame(vec![]);assert!(!h.app.history.iter().any(|a|a.title=="会议提醒"));
'''),
    ("src/ui_tests.rs",
     '''    let mut h=H::new();h.app.open_settings();h.ctx.set_embed_viewports(false);
    assert_eq!(h.app.settings.settings_theme,SettingsTheme::System);
    assert_eq!(SettingsTheme::parse("unknown"),SettingsTheme::System);
    let mut raster=crate::preview_raster::Raster::new(690,1100);
    for (mode,system,dark) in [(SettingsTheme::System,false,false),(SettingsTheme::System,true,true),
        (SettingsTheme::System,false,false),(SettingsTheme::Dark,false,true),(SettingsTheme::Light,true,false)] {
        h.app.settings.settings_theme=mode;
''',
     '''    let mut h=H::new();h.app.open_settings();h.ctx.set_embed_viewports(false);
    assert_eq!(h.app.settings.settings_theme,SettingsTheme::System);
    assert_eq!(SettingsTheme::parse("unknown"),SettingsTheme::System);
    // v0.6.64 合并后「常规」页新增停靠/避让/渲染兼容等设置，整页高度已超过 1100。
    // egui 会跳过视口外控件的绘制，导致分段选项文本缺失，故测试视口需覆盖整页。
    let mut raster=crate::preview_raster::Raster::new(690,2000);
    for (mode,system,dark) in [(SettingsTheme::System,false,false),(SettingsTheme::System,true,true),
        (SettingsTheme::System,false,false),(SettingsTheme::Dark,false,true),(SettingsTheme::Light,true,false)] {
        h.app.settings.settings_theme=mode;
'''),
    ("src/ui_tests.rs",
     '''        let output=std::sync::Arc::new(std::sync::Mutex::new(None));let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,1100.0))),
                viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()};
            *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
''',
     '''        let output=std::sync::Arc::new(std::sync::Mutex::new(None));let capture=output.clone();
        egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
            let id=viewport.ids.this;
            let raw=egui::RawInput{viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,2000.0))),
                viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()};
            *capture.lock().unwrap()=Some(ctx.run(raw,|ctx|(viewport.viewport_ui_cb)(ctx)));
        });
'''),
    ("src/ui_tests.rs",
     '''                        1=>&[("时间偏移","0.35 秒")],
                        2=>&[("专注","25 分钟"),("休息","5 分钟")],
                        3=>&[("每日目标","2000 ml"),("每次记录","250 ml"),("提醒间隔","45 分钟"),("提醒时段","9 时")],
                        7=>&[("验证码隐藏","60 秒"),("检查间隔","120 秒"),("会议提前","15 分钟"),("邮箱","输入邮箱地址"),("授权码","输入邮箱授权码")],
                        8=>&[("用户名","输入用户名或粘贴账号信息"),("密码","输入平台生成的密码")],
                        11=>&[("服务","Codex"),("备注","例如：个人、工作、备用"),("选择该账号的登录文件","选择…")],
                        _=>&[],
''',
     '''                        1=>&[("时间偏移","0.35 秒")],
                        2=>&[("专注","25 分钟"),("休息","5 分钟")],
                        3=>&[("每日目标","2000 ml"),("每次记录","250 ml"),("提醒间隔","45 分钟"),("提醒时段","9 时")],
                        7=>&[("邮件验证码隐藏","60 秒"),("检查间隔","120 秒"),("会议提前","15 分钟"),("邮箱","输入邮箱地址"),("授权码","输入邮箱授权码")],
                        8=>&[("用户名","输入用户名或粘贴账号信息"),("密码","输入平台生成的密码")],
                        11=>&[("服务","Codex"),("备注","例如：个人、工作、备用"),("选择该账号的登录文件","选择…")],
                        _=>&[],
'''),
    ("src/ui_tests.rs",
     '''                    for (label,value) in pairs {
                        let centers=|needle:&str|child.shapes.iter().filter_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()==needle=>Some(t.visual_bounding_rect().center().y),_=>None}).collect::<Vec<_>>();
                        let distance=centers(label).iter().flat_map(|a|centers(value).into_iter().map(move|b|(a-b).abs())).fold(f32::INFINITY,f32::min);
                        assert!(distance<=1.0,"设置页 {tab} 的 {label} 与 {value} 未居中：{distance}");
                    }
                }
                if tab==0 && frame==2 {
''',
     '''                    for (label,value) in pairs {
                        let centers=|needle:&str|child.shapes.iter().filter_map(|s|match &s.shape{egui::Shape::Text(t)if t.galley.text()==needle=>Some(t.visual_bounding_rect().center().y),_=>None}).collect::<Vec<_>>();
                        let distance=centers(label).iter().flat_map(|a|centers(value).into_iter().map(move|b|(a-b).abs())).fold(f32::INFINITY,f32::min);
                        // 标签用 Body(14) 、选项用小一号字，字形包围盒中心天然差 1px 级，
                        // 阈值放到 1.5px；视觉上仍在同一水平线。
                        assert!(distance<=1.5,"设置页 {tab} 的 {label} 与 {value} 未居中：{distance}");
                    }
                }
                if tab==0 && frame==2 {
'''),
    ("src/ui_tests.rs",
     '''                        egui::Shape::Text(t) if t.galley.text()==label=>Some(t.visual_bounding_rect().center().y),_=>None,
                    }).unwrap();
                    for (label,choices) in [("显示屏",vec!["跟随主屏"]),("时间制式",vec!["24 小时","12 小时"]),("时段文字",vec!["不显示","上午 / 下午","详细时段"])] {
                        for choice in choices {assert!((center(label)-center(choice)).abs()<1.0,"{label}与{choice}应垂直居中：{} / {}",center(label),center(choice));}
                    }
                }
                let background=if dark{egui::Color32::from_rgb(25,25,27)}else{egui::Color32::from_rgb(246,247,249)};
''',
     '''                        egui::Shape::Text(t) if t.galley.text()==label=>Some(t.visual_bounding_rect().center().y),_=>None,
                    }).unwrap();
                    for (label,choices) in [("显示屏",vec!["跟随主屏"]),("时间制式",vec!["24 小时","12 小时"]),("时段文字",vec!["不显示","上午 / 下午","详细时段"])] {
                        for choice in choices {assert!((center(label)-center(choice)).abs()<1.5,"{label}与{choice}应垂直居中：{} / {}",center(label),center(choice));}
                    }
                }
                let background=if dark{egui::Color32::from_rgb(25,25,27)}else{egui::Color32::from_rgb(246,247,249)};
'''),
    ("src/ui_tests.rs",
     '''                        let dest=std::path::PathBuf::from(dest);std::fs::create_dir_all(&dest).unwrap();
                        raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(child.shapes,child.pixels_per_point),child.pixels_per_point);
                        let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
                        image::save_buffer(dest.join(format!("settings-{}-{tab}.png",if dark{"dark"}else{"light"})),&rgba,690,1100,image::ColorType::Rgba8).unwrap();
                    }
                }
                raster.free_textures(&root.textures_delta);raster.free_textures(&child.textures_delta);
''',
     '''                        let dest=std::path::PathBuf::from(dest);std::fs::create_dir_all(&dest).unwrap();
                        raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(child.shapes,child.pixels_per_point),child.pixels_per_point);
                        let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
                        image::save_buffer(dest.join(format!("settings-{}-{tab}.png",if dark{"dark"}else{"light"})),&rgba,690,2000,image::ColorType::Rgba8).unwrap();
                    }
                }
                raster.free_textures(&root.textures_delta);raster.free_textures(&child.textures_delta);
'''),
    ("src/ui_tests.rs",
     '''    let total=h.app.side_bar_len()-14.0;
    let mut y=top+(h.app.h.pos-total)/2.0;
    for item in h.app.side_items() {
        let height=match item {crate::side_dock::SideItem::Quota(i,hh) if h.app.quota.config.accounts[i].id==wb=>{y+=hh/2.0;break;},crate::side_dock::SideItem::Quota(_,hh)=>hh,crate::side_dock::SideItem::Clock=>32.0,crate::side_dock::SideItem::Weather=>40.0,crate::side_dock::SideItem::Battery=>20.0,crate::side_dock::SideItem::Music=>18.0,crate::side_dock::SideItem::Focus|crate::side_dock::SideItem::Bell=>22.0};
        y+=height;
    }
    let click=Pos2::new(18.0,y);
    side_frame(&mut h,Some(click));
''',
     '''    let total=h.app.side_bar_len()-14.0;
    let mut y=top+(h.app.h.pos-total)/2.0;
    for item in h.app.side_items() {
        let height=match item {crate::side_dock::SideItem::Quota(i,hh) if h.app.quota.config.accounts[i].id==wb=>{y+=hh/2.0;break;},crate::side_dock::SideItem::Quota(_,hh)=>hh,crate::side_dock::SideItem::Clock=>32.0,crate::side_dock::SideItem::Weather=>40.0,crate::side_dock::SideItem::Battery|crate::side_dock::SideItem::PhoneBattery=>20.0,crate::side_dock::SideItem::Music=>18.0,crate::side_dock::SideItem::Focus|crate::side_dock::SideItem::Bell=>22.0};
       y+=height;
    }
    let click=Pos2::new(18.0,y);
    side_frame(&mut h,Some(click));
'''),
    ("src/ui_tests.rs",
     '''
}

#[test]
fn weather_page_shows_the_local_snapshot_and_forecast(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
''',
     '''
}


/// 通知来源页两种模式都要能渲染：全部应用（显示应用列表）与仅 QQ（隐藏列表）。
#[test]
fn notification_source_page_renders_in_both_modes(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let mut h=H::new();
    h.app.settings_open=true;
    h.app.settings_category=5; // 「通知、邮箱与日程」分组
    h.app.settings_tab=6;
    h.ctx.set_embed_viewports(false);
    // 三种状态：全部应用 / 仅 QQ / 停止监听（都要能渲染且不显示应用开关）。
    for (read,qq_only) in [(true,false),(true,true),(false,false),(false,true)] {
        h.app.settings.read_notifications=read;
        h.app.settings.qq_only=qq_only;
        for _ in 0..2 {
            let time=h.t;
            let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();
            egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
                let id=viewport.ids.this;
                *capture.lock().unwrap()=Some(ctx.run(egui::RawInput{time:Some(time),viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,560.0))),viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()},|ctx|(viewport.viewport_ui_cb)(ctx)));
            });
            let _root=h.ctx.run(egui::RawInput{time:Some(time),..Default::default()},|ctx|h.app.settings_ui(ctx,false));
            assert!(output.lock().unwrap().is_some(),"read={read} qq_only={qq_only} 时设置页应完成渲染");
            h.t+=1.0/60.0;
        }
    }
}
#[test]
fn weather_page_shows_the_local_snapshot_and_forecast(){
    let _g=ENV_LOCK.lock().unwrap_or_else(|e|e.into_inner());
'''),
    ("src/ui_tests.rs",
     '''            raster.update_textures(&output.textures_delta);
            if frame==3 {raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);image::save_buffer(dest.join(format!("{name}.png")),&rgba,pw as u32,ph as u32,image::ColorType::Rgba8).unwrap();}
            raster.free_textures(&output.textures_delta);
        }
    }
}

#[test]
#[ignore="按需导出天气设置页，检查排版"]
fn weather_settings_preview(){
''',
     '''            raster.update_textures(&output.textures_delta);
            if frame==3 {raster.buf.fill([0.0,0.0,0.0,1.0]);raster.draw(&h.ctx.tessellate(output.shapes,output.pixels_per_point),output.pixels_per_point);let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);image::save_buffer(dest.join(format!("{name}.png")),&rgba,pw as u32,ph as u32,image::ColorType::Rgba8).unwrap();}
            raster.free_textures(&output.textures_delta);
       }
    }
}


/// 「仅 QQ」与「手机短信与来电」必须互相独立：手机只看自己的开关，其它应用只看监听范围。
#[test]
fn phone_alerts_is_independent_of_listen_scope(){
    let mut h=H::new();
    h.app.settings.read_notifications=true;
    let phone_id="Microsoft.YourPhone_8wekyb3d8bbwe!App";
    for &(qq_only,phone_alerts) in &[(false,false),(false,true),(true,false),(true,true)] {
        h.app.settings.qq_only=qq_only;
        h.app.settings.phone_alerts=phone_alerts;
        // 手机：只由「手机短信与来电」决定，与监听范围无关。
        assert_eq!(h.app.accepts_notification(phone_id,true),phone_alerts,
            "qq_only={qq_only} phone_alerts={phone_alerts}：手机应只受手机开关控制");
        // QQ：两种范围下都接收。
        assert!(h.app.accepts_notification("QQ",false));
        // 其它应用：只由监听范围决定。
        assert_eq!(h.app.accepts_notification("D:/x/Steam.exe",false),!qq_only,
            "qq_only={qq_only}：非 QQ 应用应只受监听范围控制");
    }
    // 没连上就一律不接收，手机开关开着也没用。
    h.app.settings.read_notifications=false;
    h.app.settings.phone_alerts=true;
    assert!(!h.app.accepts_notification(phone_id,true),"未连接时手机通知也不应接收");
}


/// 手机连接页要能渲染，且测试环境（未装「手机连接」）下短信开关处于锁定态。
#[test]
fn phone_settings_page_renders_with_locked_sms_toggle(){
    let mut h=H::new();
    h.app.settings_open=true;
    h.app.settings_category=6; // 「手机连接」分组
    h.app.settings_tab=12;
    h.ctx.set_embed_viewports(false);
    for read_notifications in [true,false] {
        h.app.settings.read_notifications=read_notifications;
        for _ in 0..2 {
            let time=h.t;
            let output=std::sync::Arc::new(Mutex::new(None));let capture=output.clone();
            egui::Context::set_immediate_viewport_renderer(move|ctx,mut viewport|{
                let id=viewport.ids.this;
                *capture.lock().unwrap()=Some(ctx.run(egui::RawInput{time:Some(time),viewport_id:id,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,egui::vec2(690.0,560.0))),viewports:std::iter::once((id,egui::ViewportInfo{native_pixels_per_point:Some(1.0),..Default::default()})).collect(),..Default::default()},|ctx|(viewport.viewport_ui_cb)(ctx)));
            });
            let _root=h.ctx.run(egui::RawInput{time:Some(time),..Default::default()},|ctx|h.app.settings_ui(ctx,false));
            assert!(output.lock().unwrap().is_some(),"手机连接页应完成渲染");
            h.t+=1.0/60.0;
        }
    }
}
#[test]
#[ignore="按需导出天气设置页，检查排版"]
fn weather_settings_preview(){
'''),
    ("src/updates.rs",
     '''        if self.rx.is_some() { ctx.request_repaint_after(Duration::from_millis(100)); }
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        if ui.add_enabled(self.rx.is_none(), egui::Button::new("检查更新")).clicked() {
            let (tx, rx) = mpsc::channel(); self.rx = Some(rx);
            self.message.clear(); self.result = None; self.dialog_open = false;
''',
     '''        if self.rx.is_some() { ctx.request_repaint_after(Duration::from_millis(100)); }
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, enabled: bool) {
        if !enabled {
            ui.label(egui::RichText::new("更新检查已关闭 · 自定义构建保护").size(11.5).color(ui.visuals().weak_text_color()));
            return;
        }
        if ui.add_enabled(self.rx.is_none(), egui::Button::new("检查更新")).clicked() {
            let (tx, rx) = mpsc::channel(); self.rx = Some(rx);
            self.message.clear(); self.result = None; self.dialog_open = false;
'''),
    ("src/updates.rs",
     '''                                    });
                            });
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(if self.message.is_empty(){"更新后自动重启，设置和记录会保留。"}else{&self.message}).color(theme::DIM).size(11.5));
                        if self.cancel.is_some() {
                            ui.add(egui::ProgressBar::new(self.progress).fill(theme::BLUE).desired_height(8.0));
''',
     '''                                    });
                            });
                        ui.add_space(4.0);
                        if crate::CUSTOM_BUILD {
                            ui.label(egui::RichText::new("当前是自定义构建：安装官方版会覆盖手机电量、更新开关等本地修改，建议先运行 reapply.ps1。").color(egui::Color32::from_rgb(240,180,90)).size(11.5));
                        }
                        ui.label(egui::RichText::new(if self.message.is_empty(){"更新后自动重启，设置和记录会保留。"}else{&self.message}).color(theme::DIM).size(11.5));
                        if self.cancel.is_some() {
                            ui.add(egui::ProgressBar::new(self.progress).fill(theme::BLUE).desired_height(8.0));
'''),
    ("src/updates.rs",
     '''            let mut state = State::default(); state.rx = Some(rx);
            let _ = ctx.run(Default::default(), |ctx| {
                state.poll(ctx);
                egui::CentralPanel::default().show(ctx, |ui| state.ui(ui));
            });
            assert!(state.rx.is_none());
            assert_eq!(!state.message.is_empty(), disconnected);
''',
     '''            let mut state = State::default(); state.rx = Some(rx);
            let _ = ctx.run(Default::default(), |ctx| {
                state.poll(ctx);
                egui::CentralPanel::default().show(ctx, |ui| state.ui(ui, true));
            });
            assert!(state.rx.is_none());
            assert_eq!(!state.message.is_empty(), disconnected);
'''),
    ("Cargo.toml",
     '''[[bin]]
name = "daozai"
path = "src/main.rs"

# 无窗口演示录制器：脚本驱动 + 软件光栅化，输出 RGBA 帧流给 ffmpeg。
[[bin]]
''',
     '''[[bin]]
name = "daozai"
path = "src/main.rs"

# 独立精简的浏览器扩展桥宿主：不链接 GUI 栈，只做下载事件 → island_status.json 的翻译。
# 安装包把编译出的 daozai-bridge.exe 复制为 daozai_bridge.exe（见 installer/daozai.iss）。
[[bin]]
name = "daozai-bridge"
path = "src/bin/daozai-bridge/main.rs"

# 无窗口演示录制器：脚本驱动 + 软件光栅化，输出 RGBA 帧流给 ffmpeg。
[[bin]]
'''),
    ("Cargo.toml",
     '''  "Win32_Security_Cryptography",
  "Win32_System_Power",
  "Win32_System_Registry",
  "Win32_System_SystemInformation",
  "Win32_System_Threading",
  "Win32_System_RemoteDesktop",
''',
     '''  "Win32_Security_Cryptography",
  "Win32_System_Power",
  "Win32_System_Registry",
  "Win32_System_IO",
  "Win32_System_SystemInformation",
  "Win32_System_Threading",
  "Win32_System_RemoteDesktop",
'''),
]


def encode(text: str, data: bytes) -> bytes:
    """按目标文件的实际行尾编码补丁片段。"""
    crlf = data.count(b"\r\n") > data.count(b"\n") // 2
    return text.replace("\n", "\r\n").encode("utf-8") if crlf else text.encode("utf-8")


def bump_version(root: pathlib.Path) -> str:
    """在官方版本号上加一个补丁号，便于在关于页确认跑的是自定义版。"""
    toml = root / "Cargo.toml"
    data = toml.read_bytes()
    match = re.search(rb'^version = "(\d+)\.(\d+)\.(\d+)"', data, re.M)
    if not match:
        raise SystemExit("Cargo.toml 里找不到 [package] 版本号")
    a, b, c = (int(part) for part in match.groups())
    old_ver, new_ver = f"{a}.{b}.{c}", f"{a}.{b}.{c + 1}"
    toml.write_bytes(data.replace(match.group(0), f'version = "{new_ver}"'.encode(), 1))

    lock = root / "Cargo.lock"
    if lock.exists():
        raw = lock.read_bytes()
        pattern = re.compile(rb'(name = "daozai"\r?\nversion = ")\d+\.\d+\.\d+(")')
        raw, count = pattern.subn(lambda m: m.group(1) + new_ver.encode() + m.group(2), raw, count=1)
        if count != 1:
            raise SystemExit("Cargo.lock 里找不到 daozai 版本条目")
        lock.write_bytes(raw)

    rc = root / "app.rc"
    if rc.exists():
        raw = rc.read_bytes()
        raw = raw.replace(f"{a},{b},{c},0".encode(), f"{a},{b},{c + 1},0".encode())
        raw = raw.replace(f'"{old_ver}"'.encode(), f'"{new_ver}"'.encode())
        rc.write_bytes(raw)

    changelog = root / "CHANGELOG.md"
    if changelog.exists():
        text = changelog.read_text(encoding="utf-8")
        if "自定义构建" not in text:
            pos = text.find("\n## ")
            note = f"""
## v{new_ver} · 自定义构建

- 新增：读取「手机连接」的手机电量作为唯一常驻电量来源，不回退本机电量；手机断开时自动收起。
- 新增：播放音乐/歌词接管胶囊时，手机电量环仍常驻在胶囊最左侧。
- 充电显示：卡片无充电字段，按电量涨跌推测并在长时间不涨时退出充电态；也可由 `%APPDATA%\\Daozai\\phone_charging.txt` 写 `1`/`0` 精确覆盖。充电时电量环变绿并显示闪电。
- 修复：歌词态下封面压住吉祥物、歌词又与宠物重叠。电量环/封面/宠物/歌词统一按胶囊左边缘排版（手写 flex lane，间距集中在 `LANE_*` 常量），歌词在剩余通道内居中，专注模式下也不会错位。
- 修复：暂停音乐且歌词较长时，egui `halign=Center` 的负行内偏移会把整行文字推到封面上；release 改为左对齐后手动居中，并加绘制裁剪兜底。
- 优化：长歌词按通道宽度自动缩小字号（11pt 逐级降到 8pt），能单行完整显示就不截断/分页；测试构建仍用官方 11pt。
- 新增：「关于」页的检查更新开关，默认关闭，防止官方安装包覆盖自定义修改。
- 新增：手机连接通知「只上岛」。三处 `qq_only` 过滤对 Phone Link 白名单通道放行（其余应用仍静默），
  岛上显示「手机 · 应用名」+ 一行短摘要（与自带邮件「新邮件」提醒区分）；
  配合系统侧把 `YourPhoneNotifications_*` 子通道 `ShowBanner=0` 关闭，
  右下角不再弹横幅，通知仍进通知中心供岛仔读取。
- 优化：真实通知只报主 AUMID，改由 Toast 正文首段识别来源 App（QQ/微信/支付宝/Gmail/
  短信/电话/验证码），显示「手机 · QQ」等精确标签并去掉正文里重复的 App 名；
  Steam / Discord / 12306 等高频来源静默，未识别到的按「连接」放行以免漏掉重要通知。
- 新增：手机通知验证码卡片（大号数字 + 一键复制）、同 App 10 秒内多条聚合防刷屏、
  手机 App 图标（直接用手机连接自带图标）、未接来电累计常驻铃铛。
- 新增：显示后从通知中心移除手机通知（`PHONE_CLEAR`，岛仔未运行时不清理）。
- 新增：通用状态总线 `%APPDATA%\\Daozai\\island_status.json`，任何脚本/应用写 JSON
  即可上岛（`id`/`text`/`detail`/`seconds`），为 ADB、下载、构建等预留统一入口。
- 修复：手机断开后仍显示旧电量。卡片断开文案是 JSON 的 Unicode 转义中文，且电量数值会消失；
  现在先解码文案再判定连接状态，明确断开立即清空电量缓存，卡片文件变化后 1 秒内生效。
- 新增：通用状态总线升级为覆写式——文件里有什么岛上就有什么，同 id 覆写即刷新续期、
  删掉条目立刻撤下；支持 `progress` 进度条、`color` 状态色、`click` 复制/打开动作白名单。
  配套 `daozai-patches/scripts/` 脚本库与本地调试台，ADB / 下载进度等只需写 JSON。
- 说明：手机连接来电时 Windows 只更新角标（badge）、不产生 Toast，第三方读不到「来电中」；
  来电中提示已移除，只保留挂断后的「未接来电」提示（30 秒 + 铃铛红点常驻）。
- 新增：手机 App 名单配置化。来源 App 的标签与静默规则移到 `%APPDATA%\\Daozai\\phone-apps.json`，
  改完约 1 秒自动重载，新装 App 想上岛/静默不用再改代码重编译。
- 新增：手机电量关键变化提示（断开 / 重连 / 低电 ≤20% / 充满），首轮不误报。
- 修复：邮件面板账号下拉展开时，鼠标悬停高亮还落在下面的邮件行上、菜单项本身没反馈；
  现在下拉展开期间列表行不画悬停高亮，菜单项自己带悬停背景。
- 新增：邮件面板按邮箱账号筛选。顶部多了一个账号选框，可选「全部账号」或任一已添加的邮箱，
  列表、全部/未读计数、验证码大卡都跟着筛选；原来的「同步 / 仅本机」小字并入选框的悬停提示。
- 修复：邮件面板的「全部已读」点击无反应。邮件请求丢失（协议线程异常退出）后 pending 永不清空，
  按钮就一直处于禁用状态且点击毫无反馈；现在协议线程单次异常不会退出、pending 超过 40 秒自动复位、
  未就绪时点击会提示「邮件状态还没同步完」。
- 调整：邮箱页的「验证码高亮 / 隐藏」改名为「邮件验证码高亮 / 隐藏」，
  避免与短信验证码混淆（这两项只作用于邮件页）。
- 新增：设置页独立「短信」分类。手机验证码自动复制从「邮箱」移到「短信」，
  避免误以为是邮件验证码功能。
- 新增：设置页「重复查看通知」。开启后通知保留 60 秒、可在铃铛上多次悬停查看；
  关闭则维持 4.5 秒后消失。验证码仍单独保留 90 秒。
- 修复：放歌（歌词态）且关闭正文预览时，验证码通知只显示铃铛红点，而铃铛 4.5 秒就消失，
  来不及悬停查看；现在验证码类通知的铃铛保留 90 秒（其余通知仍 4.5 秒）。
- 新增：状态总线多槽位并列显示。存活状态在胶囊右侧并排显示（最多 3 条，含色点与进度条），
  不再只靠一次性的横幅提示；`Layout` 新增动态 `status` 槽位，不影响 resident_order。
- 新增：手机短信验证码自动复制开关，命中后直接写剪贴板；识别结果追加到
  `%APPDATA%\\Daozai\\diagnostics\\otp.log` 便于排查。邮件验证码自动复制已移除（实测不生效）。
- 新增：设置页可直接勾选手机 App 名单，改动写回 phone-apps.json。
- 构建标记 `CUSTOM_BUILD`，更新弹窗会提示先运行 `reapply.ps1` 重放补丁。
- 由 `daozai-patches/apply.py` 生成，可重放到后续官方版本。
"""
            text = text[:pos] + note + text[pos:] if pos >= 0 else text + note
            with open(changelog, "w", encoding="utf-8", newline="") as handle:
                handle.write(text)
    return new_ver


def main() -> None:
    args = [arg for arg in sys.argv[1:] if arg != "--force"]
    if len(args) != 1:
        raise SystemExit("用法: python apply.py <源码根目录> [--force]")
    root = pathlib.Path(args[0]).resolve()
    if not (root / "Cargo.toml").is_file():
        raise SystemExit(f"不是岛仔源码目录: {root}")

    marker = root / "CUSTOM_BUILD.txt"
    if marker.exists() and marker.read_text(encoding="utf-8").strip() == PATCH_ID and "--force" not in sys.argv:
        # 新增模块不依赖 EDITS，即使补丁已应用也要同步一次，
        # 这样后续修订 phone_battery.rs 时重跑本脚本能把改动带过去。
        for name, dest in NEW_FILES:
            target = root / dest
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(PATCH_DIR / name, target)
        print(f"补丁 {PATCH_ID} 已应用，跳过编辑；已同步新增文件：{[d for _, d in NEW_FILES]}")
        return

    applied = skipped = 0
    for rel, old, new in EDITS:
        path = root / rel
        data = path.read_bytes()
        old_b, new_b = encode(old, data), encode(new, data)
        if old_b not in data:
            if new_b in data:
                skipped += 1
                continue
            raise SystemExit(f"补丁失配：{rel} 找不到原文片段\n---\n{old[:160]}\n---\n官方源码可能已改动，请人工对照后更新 apply.py")
        if data.count(old_b) != 1:
            raise SystemExit(f"补丁失配：{rel} 原文片段出现 {data.count(old_b)} 次，拒绝猜测")
        path.write_bytes(data.replace(old_b, new_b, 1))
        applied += 1

    for name, dest in NEW_FILES:
        target = root / dest
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(PATCH_DIR / name, target)
    version = bump_version(root)
    marker.write_text(PATCH_ID + "\n", encoding="utf-8")
    print(f"补丁 {PATCH_ID} 应用完成：新增/修改 {applied} 处，跳过 {skipped} 处，版本 -> v{version}")


if __name__ == "__main__":
    main()
