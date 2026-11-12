//! 持久化与状态模型。
//!
//! 文件布局与原工具同名同结构（原工具字符串中可见的路径与键名），但默认目录
//! 用独立的 `%APPDATA%\Daozai`，避免与原工具的 `%APPDATA%\Nusa` 互相覆盖：
//!   %APPDATA%\Daozai\settings.txt   -> pet=0/1, minimal=0/1
//!   %APPDATA%\Daozai\todo.txt
//!   %APPDATA%\Daozai\water.txt      -> bottle=<ml> / goal=<ml> / start=YYYY-MM-DD
//!   %APPDATA%\Daozai\shelf.txt      -> 置物清单（每行一个绝对路径）
//!
//! 设置环境变量 `DAOZAI_DATA_DIR` 可把以上目录整体改到别处（用于与已安装的
//! 原工具并存、或跑测试时不碰真实数据）。

use std::fs;
use std::path::{Path, PathBuf};

fn env_dir(key: &str) -> PathBuf {
    PathBuf::from(std::env::var(key).unwrap_or_else(|_| ".".into()))
}

pub fn config_dir() -> PathBuf {
    if let Ok(root) = std::env::var("DAOZAI_DATA_DIR") {
        return PathBuf::from(root);
    }
    env_dir("APPDATA").join("Daozai")
}

fn read_kv(path: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(text) = fs::read_to_string(path) {
        for line in text.trim_start_matches('\u{feff}').lines() {
            if let Some((k, v)) = line.split_once('=') {
                out.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    out
}

fn write_kv(path: &Path, pairs: &[(String, String)]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body: String = pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}\n"))
        .collect();
    atomic_write(path, body.as_bytes())
}

/// 外部 JSON 的体积上限（1 MB）。岛仔读的都是「脚本 / 配置」级别的小文件，
/// 真遇到超大文件或管道，宁可当作没写，也不要一次性读进内存。
pub(crate) const MAX_JSON_BYTES: u64 = 1_048_576;

/// 读取外部 JSON：只读常规文件，且不超过 `max` 字节；不满足返回 None。
pub(crate) fn read_capped(path:&Path,max:u64)->Option<Vec<u8>> {
    let meta=fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len()>max {return None;}
    fs::read(path).ok()
}

/// 读一个 JSON 数组文件。解析失败时把坏文件改名成 `.bad` 再返回 None，
/// 调用方据此重建——不会把「文件坏了」误当成「本来是空的」从而把历史清掉。
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

pub(crate) fn atomic_write(path:&Path,body:&[u8])->std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64,Ordering};
    static SERIAL:AtomicU64=AtomicU64::new(0);
    let (tmp,mut file)=loop {
        let mut name=path.file_name().ok_or_else(||std::io::Error::other("缺少文件名"))?.to_os_string();
        name.push(format!(".{}-{}.tmp",std::process::id(),SERIAL.fetch_add(1,Ordering::Relaxed)));
        let tmp=path.with_file_name(name);
        match fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file)=>break (tmp,file),
            Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>continue,
            Err(e)=>return Err(e),
        }
    };
    let result=file.write_all(body).and_then(|_|file.sync_all());
    drop(file);
    let result=result.and_then(|_|replace_file(&tmp,path));
    if result.is_err(){let _=fs::remove_file(&tmp);}
    result
}

fn replace_file(tmp:&Path,path:&Path)->std::io::Result<()> {
    let result=fs::rename(tmp,path);
    #[cfg(windows)]
    if result.as_ref().is_err_and(|e|e.kind()==std::io::ErrorKind::CrossesDevices) {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_COPY_ALLOWED,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH};
        // Windows 目录映射可能将同目录替换报告为跨卷，交给系统复制并替换。
        let source:Vec<u16>=tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let target:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
        return unsafe{MoveFileExW(PCWSTR(source.as_ptr()),PCWSTR(target.as_ptr()),MOVEFILE_REPLACE_EXISTING|MOVEFILE_COPY_ALLOWED|MOVEFILE_WRITE_THROUGH)}.map_err(std::io::Error::other);
    }
    result
}

/// 岛体配色皮肤（与形状独立，可任意组合）。
/// - 经典：纯黑；奶油：暖白底深棕字、橙色强调（参考 Oyen）；
/// - 毛玻璃·浅 / 毛玻璃·深：系统实时模糊 + 浅色或深色玻璃。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Skin {
    #[default]
    Classic,
    Cream,
    GlassLight,
    GlassDark,
    Fluid,
}

impl Skin {
    pub const ALL: [Skin; 5] = [Skin::Classic, Skin::Cream, Skin::GlassLight, Skin::GlassDark, Skin::Fluid];
    pub fn key(self) -> &'static str {
        match self { Self::Classic => "classic", Self::Fluid => "fluid", Self::Cream => "cream", Self::GlassLight => "glass_light", Self::GlassDark => "glass" }
    }
    pub fn parse(value: &str) -> Self {
        match value { "fluid" => Self::Fluid, "cream" => Self::Cream, "glass_light" => Self::GlassLight, "glass" | "glass_dark" => Self::GlassDark, _ => Self::Classic }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Classic => "经典", Self::Fluid => "悬浮云", Self::Cream => "奶油", Self::GlassLight => "毛玻璃·浅", Self::GlassDark => "毛玻璃·深" }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Classic => "纯黑岛体，与屏幕刘海融为一体，资源占用最低。",
            Self::Fluid => "半透明灰底随音乐封面换色，光晕跟随鼠标，播放进度带有渐隐光尾。皮肤作者：啵（@hanhb123）。",
            Self::Cream => "暖白底色配深棕文字和橙色强调，柔和明亮。",
            Self::GlassLight => "浅色磨砂玻璃，透出身后桌面的颜色，文字为深色；按背景明暗自动调整，保证清晰。",
            Self::GlassDark => "深色磨砂玻璃，透出身后桌面的颜色，文字为白色；按背景明暗自动调整，保证清晰。",
        }
    }
    pub fn is_glass(self) -> bool { matches!(self, Self::GlassLight | Self::GlassDark) }
}

/// 岛体形状：贴着屏幕顶边的刘海，或悬浮在顶边下方的胶囊。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IslandShape {
    #[default]
    Notch,
    Pill,
}

impl IslandShape {
    pub const ALL: [IslandShape; 2] = [IslandShape::Notch, IslandShape::Pill];
    pub fn key(self) -> &'static str {
        match self { Self::Notch => "notch", Self::Pill => "pill" }
    }
    pub fn parse(value: &str) -> Self {
        match value { "pill" => Self::Pill, _ => Self::Notch }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Notch => "刘海", Self::Pill => "胶囊" }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Notch => "贴着屏幕顶边，顶部两侧向外柔和过渡，像屏幕刘海。",
            Self::Pill => "悬浮在屏幕顶边下方，四角圆润，收起时是完整的胶囊。",
        }
    }
}

/// 岛体停靠的屏幕边缘：顶部为横向刘海 / 胶囊，左右两侧为竖向排列。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dock {
    #[default]
    Top,
    Left,
    Right,
}

impl Dock {
    pub const ALL: [Dock; 3] = [Dock::Top, Dock::Left, Dock::Right];
    pub fn key(self) -> &'static str {
        match self { Self::Top => "top", Self::Left => "left", Self::Right => "right" }
    }
    pub fn parse(value: &str) -> Self {
        match value { "left" => Self::Left, "right" => Self::Right, _ => Self::Top }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Top => "顶部", Self::Left => "左侧", Self::Right => "右侧" }
    }
    pub fn is_side(self) -> bool { self != Self::Top }
}

/// 岛体大小（百分比）的范围与步进。
pub const ISLAND_SIZE_MIN: u32 = 80;
pub const ISLAND_SIZE_MAX: u32 = 140;
pub const ISLAND_SIZE_STEP: u32 = 5;
/// 水平位置：-100 = 最左，0 = 居中，100 = 最右（按可移动范围的百分比）。
pub const ISLAND_OFFSET_MAX: i32 = 100;

pub fn island_size_value(value: u32) -> u32 {
    let value = value.clamp(ISLAND_SIZE_MIN, ISLAND_SIZE_MAX);
    (value + ISLAND_SIZE_STEP / 2) / ISLAND_SIZE_STEP * ISLAND_SIZE_STEP
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub display_id: String,
    /// 岛体皮肤、毛玻璃是否使用系统模糊、大小百分比、水平位置。
    pub skin: Skin,
    pub island_shape: IslandShape,
    /// 收起 ↔ 展开的过渡动效。
    pub open_anim: OpenAnim,
    /// 光标停在岛上多久才展开（毫秒，0 = 立即）。0–500，默认 200。
    /// 同时也是防误触阈值：太短会被鼠标路过触发，太长会觉得迟钝。
    pub hover_delay_ms: u32,
    pub glass_blur: bool,
    pub island_size: u32,
    pub island_offset: i32,
    /// 停靠边缘；左右停靠时 `island_offset` 表示上下位置。
    pub dock: Dock,
    pub settings_theme: crate::settings_theme::SettingsTheme,
    pub pet: bool,
    pub minimal: bool,
    pub browser_minimal: bool,
    pub avoid_browsers: bool,
    pub avoid_apps: Vec<(String,String)>,
    pub visible_pages: [bool; 8],
    pub page_order: [usize; 8],
    pub show_battery: bool,
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
    pub clock_12h: bool,
    pub clock_period: u8,
    pub resident_order: Vec<usize>,
    pub system_hud: bool,
    pub bluetooth_notifications: bool,
    pub read_notifications:bool,
    pub wechat_notifications:bool,
    /// 微信缩在托盘时，点提醒通过系统托盘按钮唤起微信（默认关闭）。
    pub wechat_tray_open:bool,
    pub qq_only:bool,
    pub notification_preview:bool,
    pub notifications: bool,
    pub sound: bool,
    pub lyrics_enabled:bool,
    pub island_lyrics:bool,
    pub lyric_delay_ms:i32,
    pub hide_fullscreen: bool,
    pub focus_minutes: u32,
    pub rest_minutes: u32,
    pub water_minutes: u32,
    pub serving_ml: u32,
    pub water_start: u32,
    pub water_end: u32,
    /// v0.5.5 邮件提醒：总开关、验证码高亮、轮询间隔（秒）、会议提前量（分钟）。
    pub mail_enabled: bool,
    pub mail_sync_operations:bool,
    pub mail_code: bool,
    pub mail_code_hide_seconds:u32,
    pub mail_interval: u32,
    pub mail_ics: u32,
    pub ignored_mail_events: Vec<String>,
    /// v0.5.6 日程：还没配置时在岛上给一次「接入飞书日历」的提示；
    /// 用户点开设置或关掉后置 false，不再弹。
    pub calendar_prompt: bool,
    pub calendar_auto_sync: bool,
    /// 记录最近复制的文字（最近记录与固定条目均在本机保存）。
    pub clipboard_history: bool,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, open_anim: OpenAnim::default(), hover_delay_ms: 200, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 8], page_order: [0,1,2,3,4,5,6,7], show_battery: true, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
    }
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

impl Settings {
    /// 主窗口（灵动岛）的界面缩放倍率。
    pub fn island_scale(&self) -> f32 {
        island_size_value(self.island_size) as f32 / 100.0
    }

    /// 水平位置，-1..=1。
    pub fn island_offset_ratio(&self) -> f32 {
        self.island_offset.clamp(-ISLAND_OFFSET_MAX, ISLAND_OFFSET_MAX) as f32 / ISLAND_OFFSET_MAX as f32
    }

    /// 低电提醒阈值；`None` 表示关闭提醒。
    pub fn low_battery_threshold(&self) -> Option<u8> {
        (self.battery_alert > 0).then(|| self.battery_alert.min(99) as u8)
    }

    /// 通知关键词表（已去掉空白项）。空表示不启用高亮。
    pub fn keywords(&self) -> Vec<&str> {
        self.notice_keywords.split([',','，']).map(str::trim).filter(|word|!word.is_empty()).collect()
    }

    pub fn clock_text(&self, clock: &str) -> String {
        use chrono::Timelike;
        let Ok(time) = chrono::NaiveTime::parse_from_str(clock.get(..5).unwrap_or(""), "%H:%M") else { return "--:--".into(); };
        let (pm, hour) = time.hour12();
        let digits=if self.clock_12h {format!("{}:{:02}",hour,time.minute())}else{time.format("%H:%M").to_string()};
        let period=match self.clock_period {
            1=>if pm {"下午"}else{"上午"},
            2=>match time.hour() {0..=5=>"凌晨",6..=10=>"上午",11..=12=>"中午",13..=16=>"下午",17..=18=>"傍晚",_=>"晚上"},
            _=>return digits,
        };
        format!("{period} {digits}")
    }

    pub fn load() -> Self {
        let mut s = Self::default();
        let kv = read_kv(&config_dir().join("settings.txt"));
        let has_delay = kv.iter().any(|(k, _)| k == "hover_delay_ms");
        // 上一版用布尔开关表达「无接力」；没有新键时把它折算成 0 ms。
        let legacy_relay_off = kv.iter().any(|(k, v)| k == "hover_relay" && v == "0");
        for (k, v) in kv {
            match k.as_str() {
                "settings_theme" => s.settings_theme=crate::settings_theme::SettingsTheme::parse(&v),
                "display_id" => s.display_id = v,
                "skin" => {
                    // 早期测试版把胶囊当作皮肤保存。
                    if v == "pill" { s.island_shape = IslandShape::Pill; }
                    s.skin = Skin::parse(&v);
                }
                "island_shape" => s.island_shape = IslandShape::parse(&v),
                "open_anim" => s.open_anim = OpenAnim::parse(&v),
                "hover_delay_ms" => s.hover_delay_ms = v.parse::<u32>().unwrap_or(200).min(500),
                "dock" => s.dock = Dock::parse(&v),
                "glass_blur" => s.glass_blur = v != "0",
                "island_size" => s.island_size = island_size_value(v.parse::<u32>().unwrap_or(100)),
                "island_offset" => s.island_offset = v.parse::<i32>().unwrap_or(0).clamp(-ISLAND_OFFSET_MAX, ISLAND_OFFSET_MAX),
                "pet" => s.pet = v == "1" || v.eq_ignore_ascii_case("true"),
                "minimal" => s.minimal = v == "1" || v.eq_ignore_ascii_case("true"),
                "browser_minimal" => s.browser_minimal = v == "1",
                "avoid_browsers" => s.avoid_browsers = v != "0",
                "avoid_apps" => s.avoid_apps = serde_json::from_str(&v).unwrap_or_default(),
                "resident_order" => {
                    let order:Vec<usize>=v.split(',').filter_map(|n|n.parse().ok()).collect();
                    if crate::bar_layout::valid_order(&order){s.resident_order=order;}
                }
                "page_order" => {
                    if let Ok(order)=v.split(',').map(str::parse::<usize>).collect::<Result<Vec<_>,_>>() {
                        if order.len()==8 && (0..8).all(|page|order.contains(&page)) {
                            s.page_order.copy_from_slice(&order);
                        }
                    }
                }
                "clock_period" => s.clock_period = v.parse::<u8>().ok().filter(|v|*v<=2).unwrap_or(0),
                "clock_12h" => s.clock_12h = v == "1",
                "show_battery" => s.show_battery = v != "0",
                "phone_battery" => s.phone_battery = v != "0",
                "notice_keep" => {s.notice_keep = v != "0";s.notice_seconds = if s.notice_keep {10} else {4};},
                "notice_seconds" => s.notice_seconds = v.parse::<u32>().unwrap_or(10).clamp(2,120),
                "otp_seconds" => s.otp_seconds = v.parse::<u32>().unwrap_or(90).clamp(10,300),
                "auto_copy_otp" => s.auto_copy_otp = v != "0",
                "update_check" => s.update_check = v == "1",
                "visible_pages" if (6..=8).contains(&v.len()) && v.bytes().all(|b| b==b'0' || b==b'1') => {
                    for (enabled,value) in s.visible_pages.iter_mut().zip(v.bytes()) { *enabled=value==b'1'; }
                    for enabled in &mut s.visible_pages[v.len()..]{*enabled=v.bytes().any(|b|b==b'1');}
                }
                "read_notifications" => s.read_notifications=v=="1",
                "wechat_notifications" => s.wechat_notifications=v=="1",
                "wechat_tray_open" => s.wechat_tray_open=v=="1",
                "qq_only" => s.qq_only=v=="1",
                "notification_preview" => s.notification_preview=v=="1",
                "system_hud" => s.system_hud = v != "0",
                "bluetooth_notifications" => s.bluetooth_notifications = v != "0",
                "notifications" => s.notifications = v != "0",
                "sound" => s.sound = v == "1",
                "lyrics_enabled" => s.lyrics_enabled=v!="0",
                "island_lyrics" => s.island_lyrics=v=="1",
                "lyric_delay_ms" => s.lyric_delay_ms=v.parse::<i32>().unwrap_or(-350).clamp(-5000,5000),
                "hide_fullscreen" => s.hide_fullscreen = v != "0",
                "focus_minutes" => s.focus_minutes = v.parse::<u32>().unwrap_or(25).clamp(1, 180),
                "rest_minutes" | "short_minutes" => s.rest_minutes = v.parse::<u32>().unwrap_or(5).clamp(1, 60),
                "water_minutes" => s.water_minutes = v.parse::<u32>().unwrap_or(45).clamp(5, 180),
                "serving_ml" => s.serving_ml = v.parse::<u32>().unwrap_or(250).clamp(50, 1000),
                "water_start" => s.water_start = v.parse::<u32>().unwrap_or(9).min(23),
                "water_end" => s.water_end = v.parse::<u32>().unwrap_or(22).min(24),
                "mail_enabled" => s.mail_enabled = v != "0",
                "mail_sync_operations" => s.mail_sync_operations=v=="1",
                "mail_code" => s.mail_code = v != "0",
                "mail_code_hide_seconds" => s.mail_code_hide_seconds=v.parse::<u32>().unwrap_or(60).clamp(15,300),
                "mail_interval" => s.mail_interval = match v.parse::<u32>().unwrap_or(120) { 60 => 60, 300 => 300, _ => 120 },
                "ignored_mail_events" => s.ignored_mail_events = serde_json::from_str(&v).unwrap_or_default(),
                "mail_ics" => s.mail_ics = match v.parse::<u32>().unwrap_or(15) { 5 => 5, 30 => 30, _ => 15 },
                "calendar_prompt" => s.calendar_prompt = v != "0",
                "calendar_auto_sync" => s.calendar_auto_sync = v != "0",
                "clipboard_history" => s.clipboard_history = v != "0",
                "battery_alert" => s.battery_alert = match v.parse::<u32>().unwrap_or(20) { 0 => 0, 10 => 10, 30 => 30, _ => 20 },
                "notice_keywords" => s.notice_keywords = v.trim().to_string(),
                "quiet_minutes" => s.quiet_minutes = match v.parse::<u32>().unwrap_or(0) { 30 => 30, 60 => 60, 120 => 120, _ => 0 },
                "quiet_meeting" => s.quiet_meeting = v != "0",
                "quota_alert" => s.quota_alert = match v.parse::<u32>().unwrap_or(20) { 0 => 0, 10 => 10, 20 => 20, 30 => 30, _ => 20 },
                "mail_filter" => s.mail_filter = v.trim().to_string(),
                _ => {}
            }
        }
        if !has_delay && legacy_relay_off { s.hover_delay_ms = 0; }
        if !s.visible_pages.contains(&true) { s.visible_pages[0]=true; }
        s
    }

    pub fn save(&self) {
        let _ = write_kv(
            &config_dir().join("settings.txt"),
            &[
                ("display_id".into(),self.display_id.clone()),
                ("skin".into(),self.skin.key().into()),
                ("island_shape".into(),self.island_shape.key().into()),
                ("open_anim".into(),self.open_anim.key().into()),
                ("hover_delay_ms".into(),self.hover_delay_ms.to_string()),
                ("hover_relay".into(),(u8::from(self.hover_delay_ms>0)).to_string()),
                ("dock".into(),self.dock.key().into()),
                ("glass_blur".into(),(self.glass_blur as u8).to_string()),
                ("island_size".into(),island_size_value(self.island_size).to_string()),
                ("island_offset".into(),self.island_offset.clamp(-ISLAND_OFFSET_MAX,ISLAND_OFFSET_MAX).to_string()),
                ("settings_theme".into(),self.settings_theme.key().into()),
                ("pet".into(), if self.pet { "1".to_string() } else { "0".to_string() }),
                ("minimal".into(), if self.minimal { "1".to_string() } else { "0".to_string() }),
                ("browser_minimal".into(), (self.browser_minimal as u8).to_string()),
                ("avoid_browsers".into(), (self.avoid_browsers as u8).to_string()),
                ("avoid_apps".into(), serde_json::to_string(&self.avoid_apps).unwrap_or_default()),
                ("visible_pages".into(), self.visible_pages.iter().map(|enabled|if *enabled {'1'}else{'0'}).collect()),
                ("page_order".into(),self.page_order.iter().map(usize::to_string).collect::<Vec<_>>().join(",")),
                ("resident_order".into(),self.resident_order.iter().map(usize::to_string).collect::<Vec<_>>().join(",")),
                ("clock_period".into(), self.clock_period.to_string()),
                ("clock_12h".into(), (self.clock_12h as u8).to_string()),
                ("show_battery".into(), (self.show_battery as u8).to_string()),
                ("phone_battery".into(), (self.phone_battery as u8).to_string()),
                ("notice_keep".into(), (self.notice_keep as u8).to_string()),
                ("notice_seconds".into(), self.notice_seconds.to_string()),
                ("otp_seconds".into(), self.otp_seconds.to_string()),
                ("auto_copy_otp".into(), (self.auto_copy_otp as u8).to_string()),
                ("update_check".into(), (self.update_check as u8).to_string()),
                ("read_notifications".into(),(self.read_notifications as u8).to_string()),
                ("wechat_notifications".into(),(self.wechat_notifications as u8).to_string()),
                ("wechat_tray_open".into(),(self.wechat_tray_open as u8).to_string()),
                ("qq_only".into(),(self.qq_only as u8).to_string()),
                ("notification_preview".into(),(self.notification_preview as u8).to_string()),
                ("system_hud".into(), (self.system_hud as u8).to_string()),
                ("bluetooth_notifications".into(), (self.bluetooth_notifications as u8).to_string()),
                ("notifications".into(), (self.notifications as u8).to_string()),
                ("sound".into(), (self.sound as u8).to_string()),
                ("lyrics_enabled".into(),(self.lyrics_enabled as u8).to_string()),
                ("island_lyrics".into(),(self.island_lyrics as u8).to_string()),
                ("lyric_delay_ms".into(),self.lyric_delay_ms.to_string()),
                ("hide_fullscreen".into(), (self.hide_fullscreen as u8).to_string()),
                ("focus_minutes".into(), self.focus_minutes.to_string()),
                ("rest_minutes".into(), self.rest_minutes.to_string()),
                ("water_minutes".into(), self.water_minutes.to_string()),
                ("serving_ml".into(), self.serving_ml.to_string()),
                ("water_start".into(), self.water_start.to_string()),
                ("water_end".into(), self.water_end.to_string()),
                ("mail_enabled".into(), (self.mail_enabled as u8).to_string()),
                ("mail_sync_operations".into(),(self.mail_sync_operations as u8).to_string()),
                ("mail_code".into(), (self.mail_code as u8).to_string()),
                ("mail_code_hide_seconds".into(), self.mail_code_hide_seconds.to_string()),
                ("mail_interval".into(), self.mail_interval.to_string()),
                ("mail_ics".into(), self.mail_ics.to_string()),
                ("ignored_mail_events".into(), serde_json::to_string(&self.ignored_mail_events).unwrap()),
                ("calendar_prompt".into(), (self.calendar_prompt as u8).to_string()),
                ("calendar_auto_sync".into(), (self.calendar_auto_sync as u8).to_string()),
                ("clipboard_history".into(), (self.clipboard_history as u8).to_string()),
                ("battery_alert".into(), self.battery_alert.to_string()),
                ("notice_keywords".into(), self.notice_keywords.clone()),
                ("quiet_minutes".into(), self.quiet_minutes.to_string()),
                ("quiet_meeting".into(), (self.quiet_meeting as u8).to_string()),
                ("quota_alert".into(), self.quota_alert.to_string()),
                ("mail_filter".into(), self.mail_filter.clone()),
            ],
        );
    }
}

#[derive(Clone, Debug, Default)]
pub struct TodoItem {
    pub text: String,
    /// 一次性待办：完成日期；每周重复：最近一次完成的日期（只算当天）。YYYY-MM-DD
    pub done_on: Option<String>,
    /// 本地日期与时间：YYYY-MM-DD HH:MM；每周重复只用其中的时间。
    pub due: Option<String>,
    pub reminded: bool,
    /// 每周重复：bit0 = 周一 … bit6 = 周日；0 = 不重复（一次性待办）。
    pub repeat: u8,
    /// 每周重复：当天已提醒的日期，第二天自动重新提醒。
    pub reminded_on: Option<String>,
}

pub const WEEKDAY_NAMES: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];
pub const REPEAT_ALL: u8 = 0b111_1111;
pub const REPEAT_WORKDAYS: u8 = 0b001_1111;
pub const REPEAT_WEEKEND: u8 = 0b110_0000;

fn weekday_bit(date: &str) -> Option<u8> {
    use chrono::Datelike;
    let day = chrono::NaiveDate::parse_from_str(date.get(..10)?, "%Y-%m-%d").ok()?;
    Some(1 << day.weekday().num_days_from_monday())
}

/// 「每天」「工作日」「周末」「每周一、三、五」。
pub fn repeat_label(mask: u8) -> String {
    match mask & REPEAT_ALL {
        0 => "不重复".into(),
        REPEAT_ALL => "每天".into(),
        REPEAT_WORKDAYS => "工作日".into(),
        REPEAT_WEEKEND => "周末".into(),
        mask => {
            let days: Vec<&str> = (0..7).filter(|i| mask & (1 << i) != 0).map(|i| WEEKDAY_NAMES[i]).collect();
            format!("每周{}", days.join("、"))
        }
    }
}

/// 列表里的短标签：「每天」「工作日」「周末」「周一三五」。
pub fn repeat_tag(mask: u8) -> String {
    match mask & REPEAT_ALL {
        0 => String::new(),
        REPEAT_ALL => "每天".into(),
        REPEAT_WORKDAYS => "工作日".into(),
        REPEAT_WEEKEND => "周末".into(),
        mask => format!("周{}", (0..7).filter(|i| mask & (1 << i) != 0).map(|i| WEEKDAY_NAMES[i]).collect::<String>()),
    }
}

impl TodoItem {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }

    pub fn is_repeating(&self) -> bool { self.repeat & REPEAT_ALL != 0 }

    /// 这一天是否要做：一次性待办每天都显示；重复待办只在选中的星期几显示。
    pub fn scheduled_on(&self, today: &str) -> bool {
        !self.is_repeating() || weekday_bit(today).is_some_and(|bit| self.repeat & bit != 0)
    }

    /// 今天是否已完成：重复待办第二天自动变回未完成。
    pub fn done_for(&self, today: &str) -> bool {
        if self.is_repeating() { self.done_on.as_deref() == Some(today) } else { self.done_on.is_some() }
    }

    /// 勾选 / 取消勾选。
    pub fn toggle(&mut self, today: &str) {
        self.done_on = if self.done_for(today) { None } else { Some(today.to_string()) };
    }

    /// `now` 为 `YYYY-MM-DD HH:MM`。一次性待办按完整时间提醒一次；
    /// 重复待办在每个选中的日子到点提醒一次。
    pub fn reminder_due(&self, now: &str) -> bool {
        let Some(due) = self.due.as_deref() else { return false; };
        let today = now.get(..10).unwrap_or("");
        if self.done_for(today) { return false; }
        if !self.is_repeating() { return !self.reminded && due <= now; }
        let (Some(at), Some(clock)) = (due.get(11..16), now.get(11..16)) else { return false; };
        self.scheduled_on(today) && self.reminded_on.as_deref() != Some(today) && at <= clock
    }

    pub fn mark_reminded(&mut self, now: &str) {
        if self.is_repeating() { self.reminded_on = now.get(..10).map(str::to_string); } else { self.reminded = true; }
    }

    /// 悬停提示里的提醒说明。
    pub fn reminder_text(&self) -> Option<String> {
        let due = self.due.as_deref()?;
        Some(if self.is_repeating() { format!("{} {}", repeat_label(self.repeat), due.get(11..16).unwrap_or(due)) } else { due.to_string() })
    }
}

#[derive(Clone, Debug, Default)]
pub struct TodoList {
    pub items: Vec<TodoItem>,
}

impl TodoList {
    pub fn load() -> Self {
        fs::read_to_string(config_dir().join("todo.txt")).map(|text| Self::parse(&text)).unwrap_or_default()
    }

    pub fn parse(text: &str) -> Self {
        let mut list = Self::default();
        {
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                // 格式：<x 或 o>|<完成日期或 ->|正文；也接受纯文本行（视为未完成）
                if !line.contains('|') {
                    list.items.push(TodoItem::new(line.trim()));
                    continue;
                }
                // 每周重复的标记写成 `r<星期位>`（旧版本读到会当作未完成的普通待办）。
                let mut parts = line.splitn(3, '|');
                let flag = parts.next().unwrap_or("o");
                let when = parts.next().unwrap_or("-");
                let raw = parts.next().unwrap_or("").trim();
                let repeat = flag.strip_prefix('r').and_then(|bits| bits.parse::<u8>().ok()).unwrap_or(0) & REPEAT_ALL;
                let (due, reminded_field, text) = if let Some(rest) = raw.strip_prefix("@due:") {
                    let mut fields = rest.splitn(3, '|');
                    let due = fields.next().unwrap_or("").to_string();
                    let reminded = fields.next().unwrap_or("0").to_string();
                    (Some(due), reminded, fields.next().unwrap_or("").to_string())
                } else { (None, "0".to_string(), raw.to_string()) };
                if text.is_empty() {
                    continue;
                }
                let done = (flag == "x" || repeat != 0) && when != "-";
                list.items.push(TodoItem {
                    text, due, repeat,
                    reminded: repeat == 0 && reminded_field == "1",
                    reminded_on: (repeat != 0 && reminded_field.len() == 10).then_some(reminded_field),
                    done_on: done.then(|| when.to_string()),
                });
            }
        }
        list
    }

    pub fn save(&self) {
        let body = self.serialize();
        let path = config_dir().join("todo.txt");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = atomic_write(&path, body.as_bytes());
    }

    pub fn serialize(&self) -> String {
        self
            .items
            .iter()
            .map(|i| {
                let state = if i.is_repeating() {
                    format!("r{}|{}", i.repeat & REPEAT_ALL, i.done_on.as_deref().unwrap_or("-"))
                } else {
                    i.done_on.as_ref().map(|d| format!("x|{d}")).unwrap_or_else(|| "o|-".into())
                };
                let text = i.text.replace(['\n', '\r'], " ");
                let reminded = if i.is_repeating() { i.reminded_on.clone().unwrap_or_else(|| "0".into()) } else { (i.reminded as u8).to_string() };
                match &i.due {
                    Some(due) => format!("{state}|@due:{due}|{reminded}|{text}\n"),
                    None => format!("{state}|{text}\n"),
                }
            })
            .collect()
    }

    pub fn done_today(&self, today: &str) -> usize {
        self.items
            .iter()
            .filter(|i| i.done_on.as_deref() == Some(today))
            .count()
    }

    /// 今天要做的待办（不在今天的重复待办除外）中，(已完成, 总数)。
    pub fn progress(&self, today: &str) -> (usize, usize) {
        let scheduled = self.items.iter().filter(|i| i.scheduled_on(today));
        let (mut done, mut total) = (0, 0);
        for item in scheduled { total += 1; if item.done_for(today) { done += 1; } }
        (done, total)
    }

    /// 今天未完成在前，其他日期的未完成其次，已完成沉底；原始存储顺序不变。
    pub fn display_order(&self, today: &str) -> Vec<usize> {
        let mut order:Vec<_>=(0..self.items.len()).collect();
        order.sort_by_key(|&i|{let item=&self.items[i];(item.done_for(today),!item.scheduled_on(today))});
        order
    }
}

#[derive(Clone, Debug)]
pub struct Water {
    pub bottle_ml: u32,
    pub goal_ml: u32,
    pub start_day: String,
    pub drunk_ml: u32,
    pub refills: u32,
    /// 每小时饮水量（ml），给饮水页的柱状图用。
    pub hours: [u32; 24],
    /// 定时提醒开关。
    pub remind: bool,
}

impl Default for Water {
    fn default() -> Self {
        Self { bottle_ml: 2000, goal_ml: 2000, start_day: String::new(), drunk_ml: 0, refills: 0, hours: [0; 24], remind: true }
    }
}

impl Water {
    /// `water.txt` 只写原工具已知的三个键；复刻版新增的进度键写到 `water-extra.txt`，
    /// 避免给原工具的文件引入未知键。
    pub fn load(today: &str) -> Self {
        let mut w = Self::default();
        for (k, v) in read_kv(&config_dir().join("water.txt")) {
            match k.as_str() {
                "bottle" => w.bottle_ml = v.parse::<f32>().map(|x| x as u32).unwrap_or(2000),
                "goal" => w.goal_ml = v.parse().unwrap_or(2000),
                "start" => w.start_day = v,
                _ => {}
            }
        }
        for (k, v) in read_kv(&config_dir().join("water-extra.txt")) {
            match k.as_str() {
                "drunk" => w.drunk_ml = v.parse().unwrap_or(0),
                "refills" => w.refills = v.parse().unwrap_or(0),
                "remind" => w.remind = v != "0",
                "hours" => {
                    for (i, x) in v.split(',').take(24).enumerate() {
                        w.hours[i] = x.trim().parse().unwrap_or(0);
                    }
                }
                _ => {}
            }
        }
        w.goal_ml=w.goal_ml.clamp(500,6000);
        w.bottle_ml=w.bottle_ml.clamp(50,6000);
        if w.start_day != today {
            // 跨天重置，与原工具 start= 字段语义一致
            w.start_day = today.to_string();
            w.drunk_ml = 0;
            w.refills = 0;
            w.hours = [0; 24];
            w.save();
        }
        w
    }

    pub fn save(&self) {
        let _ = write_kv(
            &config_dir().join("water.txt"),
            &[
                ("bottle".into(), self.bottle_ml.to_string()),
                ("goal".into(), self.goal_ml.to_string()),
                ("start".into(), self.start_day.clone()),
            ],
        );
        let _ = write_kv(
            &config_dir().join("water-extra.txt"),
            &[
                ("drunk".into(), self.drunk_ml.to_string()),
                ("refills".into(), self.refills.to_string()),
                ("remind".into(), if self.remind { "1".into() } else { "0".into() }),
                (
                    "hours".into(),
                    self.hours.iter().map(|h| h.to_string()).collect::<Vec<_>>().join(","),
                ),
            ],
        );
    }

    /// 喝一口：计入总量与当前小时。
    pub fn drink(&mut self, ml: u32, hour: usize) {
        let total=self.drunk_ml.saturating_add(ml).min(self.goal_ml.max(ml).saturating_mul(4));
        let added=total.saturating_sub(self.drunk_ml);
        self.drunk_ml=total;
        self.hours[hour.min(23)]=self.hours[hour.min(23)].saturating_add(added);
        self.save();
    }

    pub fn percent(&self) -> f32 {
        if self.goal_ml == 0 {
            0.0
        } else {
            (self.drunk_ml as f32 / self.goal_ml as f32 * 100.0).min(100.0)
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Shelf {
    pub items: Vec<PathBuf>,
}

impl Shelf {
    pub fn load() -> Self {
        let mut s = Self::default();
        if let Ok(text) = fs::read_to_string(config_dir().join("shelf.txt")) {
            for line in text.lines() {
                let p = PathBuf::from(line.trim());
                if !line.trim().is_empty() && !s.items.contains(&p) {
                    s.items.push(p);
                }
            }
        }
        s
    }

    pub fn save(&self) {
        let body: String = self
            .items
            .iter()
            .map(|p| format!("{}\n", p.display()))
            .collect();
        let path = config_dir().join("shelf.txt");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = atomic_write(&path, body.as_bytes());
    }

    /// 原工具的做法：记录文件所在位置，不复制文件本体（官网说明
    /// "the shelf remembers where your files live"）。
    pub fn add(&mut self, src: &Path) -> std::io::Result<()> {
        if !src.exists() { return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "文件不存在")); }
        if !self.items.iter().any(|p| p == src) {
            self.items.push(src.to_path_buf());
            self.save();
        }
        Ok(())
    }

    pub fn remove(&mut self, p: &Path) {
        self.items.retain(|item| item != p);
        self.save();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.save();
    }
}

pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KB * KB * KB {
        format!("{:.1} GB", b / (KB * KB * KB))
    } else if b >= KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn island_appearance_values_are_normalized() {
        assert_eq!(island_size_value(100), 100);
        assert_eq!(island_size_value(10), ISLAND_SIZE_MIN);
        assert_eq!(island_size_value(999), ISLAND_SIZE_MAX);
        assert_eq!(island_size_value(113), 115);
        assert_eq!(island_size_value(112), 110);
        for skin in Skin::ALL { assert_eq!(Skin::parse(skin.key()), skin); }
        assert_eq!(Skin::parse("unknown"), Skin::Classic);
        assert_eq!(Skin::parse("glass"), Skin::GlassDark, "旧版本保存的毛玻璃即深色毛玻璃");
        for shape in IslandShape::ALL { assert_eq!(IslandShape::parse(shape.key()), shape); }
        assert_eq!(Settings::default().island_shape, IslandShape::Notch);
        let mut s = Settings::default();
        assert_eq!((s.skin, s.glass_blur, s.island_scale(), s.island_offset_ratio()), (Skin::Classic, true, 1.0, 0.0));
        s.island_size = 125; s.island_offset = -250;
        assert_eq!((s.island_scale(), s.island_offset_ratio()), (1.25, -1.0));
    }

    #[test]
    fn quality_save_preserves_neighbor_and_cleans_failed_writes() {
        let dir=std::env::temp_dir().join(format!("daozai-quality-save-{}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path=dir.join("settings.txt");let neighbor=dir.join("settings.tmp");
        fs::write(&neighbor,b"other data").unwrap();
        let saved=atomic_write(&path,b"settings");
        let untouched=fs::read(&neighbor).ok()==Some(b"other data".to_vec());
        let blocked=dir.join("blocked.txt");fs::create_dir_all(&blocked).unwrap();
        assert!(atomic_write(&blocked,b"cannot replace directory").is_err());
        let leftovers=fs::read_dir(&dir).unwrap().flatten().filter(|e|e.path()!=path && e.path()!=neighbor && e.path()!=blocked).count();
        let failed_tmp=blocked.with_extension("tmp").exists();
        for entry in fs::read_dir(&dir).unwrap().flatten(){if entry.file_type().unwrap().is_file(){fs::remove_file(entry.path()).unwrap();}}
        fs::remove_dir(blocked).unwrap();fs::remove_dir(dir).unwrap();
        assert!(saved.is_ok() && untouched,"保存不能覆盖或移走同名 .tmp 文件");
        assert!(!failed_tmp && leftovers==0,"失败的保存应清除本次临时文件");
    }

    #[test]
    fn atomic_write_replaces_existing_settings() {
        let dir=std::env::temp_dir().join(format!("daozai-save-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&dir).unwrap();
        let path=dir.join("settings.txt");
        atomic_write(&path,b"focus_minutes=25\n").unwrap();
        atomic_write(&path,b"focus_minutes=26\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(),b"focus_minutes=26\n");
        assert!(!path.with_extension("tmp").exists(),"保存后不应遗留临时配置");
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn quality_concurrent_saves_publish_one_complete_value() {
        let dir=std::env::temp_dir().join(format!("daozai-quality-concurrent-{}",std::process::id()));fs::create_dir_all(&dir).unwrap();
        let path=dir.join("cache.lrc");let gate=std::sync::Arc::new(std::sync::Barrier::new(4));
        let workers:Vec<_>=(0..4u8).map(|byte|{let path=path.clone();let gate=gate.clone();std::thread::spawn(move||{gate.wait();atomic_write(&path,&vec![byte;128*1024])})}).collect();
        for worker in workers{worker.join().unwrap().unwrap();}
        let content=fs::read(&path).unwrap();assert_eq!(content.len(),128*1024);assert!(content.iter().all(|b|*b==content[0]));
        assert_eq!(fs::read_dir(&dir).unwrap().count(),1);fs::remove_file(path).unwrap();fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn weekly_todos_round_trip_and_reset_each_day() {
        // 2026-11-02 是周一。
        let mut weekly = TodoItem::new("周会 | 准备材料");
        weekly.repeat = 0b000_0101; // 周一、周三
        weekly.due = Some("2026-11-02 09:30".into());
        weekly.toggle("2026-11-02");
        weekly.reminded_on = Some("2026-11-02".into());
        let once = TodoItem { done_on: Some("2026-11-01".into()), reminded: true, due: Some("2026-11-01 08:00".into()), ..TodoItem::new("一次性") };
        let list = TodoList { items: vec![weekly.clone(), once.clone()] };
        let loaded = TodoList::parse(&list.serialize());
        assert_eq!(loaded.items.len(), 2);
        let w = &loaded.items[0];
        assert_eq!((w.text.as_str(), w.repeat, w.done_on.as_deref(), w.reminded_on.as_deref()), ("周会 | 准备材料", 0b101, Some("2026-11-02"), Some("2026-11-02")));
        assert!(w.done_for("2026-11-02") && !w.done_for("2026-11-04"), "重复待办第二天自动变回未完成");
        assert!(w.scheduled_on("2026-11-04") && !w.scheduled_on("2026-11-03"));
        assert!(!w.reminder_due("2026-11-02 10:00"), "当天已提醒");
        assert!(!w.reminder_due("2026-11-04 09:00") && w.reminder_due("2026-11-04 09:30"));
        assert!(!w.reminder_due("2026-11-03 10:00"), "不在选中的星期几不提醒");
        let o = &loaded.items[1];
        assert_eq!((o.repeat, o.reminded, o.done_on.as_deref()), (0, true, Some("2026-11-01")));
        assert!(o.done_for("2026-11-09") && o.scheduled_on("2026-11-03"));
        assert_eq!(loaded.progress("2026-11-03"), (1, 1));
        assert_eq!(loaded.progress("2026-11-04"), (1, 2));
        assert_eq!(loaded.display_order("2026-11-03"), vec![0, 1],"已完成事项应排在其他日期的未完成事项后面");
        assert_eq!(repeat_label(REPEAT_WORKDAYS), "工作日");
        assert_eq!(repeat_label(0b101_0101), "每周一、三、五、日");
        assert_eq!(repeat_tag(0b001_0101), "周一三五");
    }

    #[test]
    fn human_size_formats_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536 * 1024), "1.5 MB");
        assert_eq!(human_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn water_percent_clamps_at_100() {
        let w = Water { drunk_ml: 1000, ..Water::default() };
        assert!((w.percent() - 50.0).abs() < 0.01);
        let w = Water { drunk_ml: 5000, ..w };
        assert!((w.percent() - 100.0).abs() < 0.01);
    }
}

/// 仅保存应用标识与开关，不保存通知内容。
#[derive(Clone,Debug,Default)]
pub struct NotificationApps {pub items:Vec<(String,String,bool)>}
impl NotificationApps {
    pub fn load()->Self {
        let path=config_dir().join("notification-apps.json");
        let Some(raw)=read_capped(&path,MAX_JSON_BYTES) else{return Self::default()};
        let Ok(json)=serde_json::from_slice::<serde_json::Value>(&raw) else{return Self::default()};
        let items=json.as_array().into_iter().flatten().filter_map(|a|Some((a.get("id")?.as_str()?.to_owned(),a.get("name")?.as_str()?.to_owned(),a.get("enabled")?.as_bool()?))).collect();Self{items}
    }
    pub fn save(&self){
        let list:Vec<_>=self.items.iter().map(|(id,name,enabled)|serde_json::json!({"id":id,"name":name,"enabled":enabled})).collect();
        let path=config_dir().join("notification-apps.json");if let Some(parent)=path.parent(){let _=fs::create_dir_all(parent);}if let Ok(data)=serde_json::to_vec(&list){let _=atomic_write(&path,&data);}
    }
    pub fn observe(&mut self,id:&str,name:&str)->bool{
        if let Some(item)=self.items.iter().find(|a|a.0==id){return item.2;}
        let enabled=true;
        self.items.push((id.to_string(),name.to_string(),enabled));self.save();enabled
    }
}

/// 通知规则：`notice-rules.json` 里每条 = 正则 + 动作，按文件顺序、第一条命中即生效。
///
/// ```json
/// [{"pattern":"^(广告|促销)","action":"mute"},
///  {"pattern":"验证码","action":"copy"},
///  {"pattern":"面试|offer","action":"highlight"}]
/// ```
///
/// 动作：`mute` 静音（不上岛）、`highlight` 左侧亮琥珀色竖条、`copy` 自动复制正文。
/// 正则按 `(?i)` 编译，写错的那条会被跳过，不影响其它规则。
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

/// 手机连接通知的来源 App 名单：`name` 匹配 Toast 正文首段，`label` 是岛上显示的标签，
/// `allow=false` 表示静默。放在 `phone-apps.json`，改完约 1 秒自动重载，不用重编译。
pub struct PhoneApps {
    pub items:Vec<(String,String,bool)>,
    modified:Option<std::time::SystemTime>,
}
fn builtin_phone_apps()->Vec<(String,String,bool)>{
    let allow=[("QQ","QQ"),("微信","微信"),("WeChat","微信"),("支付宝","支付宝"),("Alipay","支付宝"),
        ("Gmail","Gmail"),("邮件","Gmail"),("短信","短信"),("信息","短信"),("电话","电话"),("来电","电话"),("验证码","验证码")];
    let silent=["Steam","Discord","12306","大麦","BOSS直聘","Boss直聘","明日方舟"];
    let mut items:Vec<(String,String,bool)>=allow.iter().map(|(k,v)|((*k).to_string(),(*v).to_string(),true)).collect();
    items.extend(silent.iter().map(|k|((*k).to_string(),"连接".to_string(),false)));
    items
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

/// 浏览器扩展桥的用户偏好（设置 → 浏览器下载页写入，桥宿主读取）。
/// 存 config_dir()/bridge.json，桥进程与主程序通过这份文件交接。
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

/// MAA（明日方舟助手）对接偏好：存 config_dir()/maa.json。
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
    /// MAA 日志目录（空 = 从 MAA 可执行文件自动推导 <MAA目录>\debug）。
    pub log_dir: String,
}

impl Default for MaaPrefs {
    fn default() -> Self {
        Self { notify_chain: true, notify_recruit_star: true, notify_error: true, resident_progress: true, seconds: 8, log_dir: String::new() }
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
mod notice_rules_tests {
    use super::NoticeRules;

    #[test]
    fn 规则按顺序第一条命中且坏正则不影响其它条() {
        let mut rules = NoticeRules::default();
        rules.parse(r#"[{"pattern":"广告","action":"mute"},{"pattern":"(","action":"copy"},{"pattern":"验证码|校验码","action":"copy"},{"pattern":"面试","action":"highlight"}]"#);
        assert!(rules.hits("这是一条广告推送", "mute"), "广告应被静音");
        assert!(rules.hits("您的验证码 1234", "copy"), "验证码应自动复制");
        assert!(rules.hits("明天下午面试", "highlight"), "面试应高亮");
        assert!(!rules.hits("普通消息", "mute"));
        assert_eq!(rules.first_action("广告里带验证码"), Some("mute"), "按文件顺序第一条生效");
        // 空文本不该命中。
        assert!(!rules.hits("", "mute"));
    }
}

#[cfg(test)]
mod external_json_safety_tests {
    use super::*;

    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("daozai-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn 外部_JSON_只读常规文件且有体积上限() {
        let file = path("capped");
        fs::write(&file, b"[]").unwrap();
        assert!(read_capped(&file, 2).is_some());
        assert!(read_capped(&file, 1).is_none(), "超过上限的输入不能读进内存");
        let dir = path("directory");
        fs::create_dir_all(&dir).unwrap();
        assert!(read_capped(&dir, MAX_JSON_BYTES).is_none(), "目录不能当 JSON 文件读");
        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn 坏_JSON_被改名而不是伪装成空历史() {
        let file = path("broken");
        fs::write(&file, b"{not json").unwrap();
        assert!(read_json_renaming_bad(&file, MAX_JSON_BYTES).is_none());
        let quarantined: Vec<_> = fs::read_dir(std::env::temp_dir()).unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().starts_with(&format!("{}.bad.", file.display())))
            .collect();
        assert_eq!(quarantined.len(), 1, "坏文件应被隔离保留，不能直接覆盖");
        assert!(!file.exists(), "原路径必须腾空，否则下次写入会盖掉坏文件");
        // 故意不删隔离文件，模拟「已经损坏过一次」。

        // 第二次损坏：.bad.0 已存在，仍必须成功隔离成 .bad.1。
        fs::write(&file, b"{still broken").unwrap();
        assert!(read_json_renaming_bad(&file, MAX_JSON_BYTES).is_none());
        assert!(!file.exists(), "隔离目标已存在时也不能留下坏文件被覆盖");
        let again: Vec<_> = fs::read_dir(std::env::temp_dir()).unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().starts_with(&format!("{}.bad.", file.display())))
            .collect();
        assert_eq!(again.len(), 2, "两次损坏应各留一份副本");
        for p in again { let _ = fs::remove_file(p); }
    }
}

#[cfg(test)]
mod open_anim_tests {
    use super::*;
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
}
