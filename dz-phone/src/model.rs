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
    pub visible_pages: [bool; 9],
    pub page_order: [usize; 9],
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
    pub wecom_notifications:bool,
    pub feishu_notifications:bool,
    pub dingtalk_notifications:bool,
    /// 微信缩在托盘时，点提醒通过系统托盘按钮唤起微信（默认关闭）。
    pub wechat_tray_open:bool,
    pub qq_only:bool,
    /// 接收「手机连接」转发的短信 / 来电（独立于 qq_only：仅 QQ 模式下也能单独关掉）。
    pub phone_alerts:bool,
    pub notification_preview:bool,
    pub notifications: bool,
    pub sound: bool,
    pub lyrics_enabled:bool,
    pub island_lyrics:bool,
    pub lyric_delay_ms:i32,
    pub hide_fullscreen: bool,
    /// 顶栏类软件（含置顶的任务栏）登记占用屏幕顶部时，岛体从它的下沿开始摆放。
    pub avoid_top_bar: bool,
    pub focus_minutes: u32,
    pub rest_minutes: u32,
    pub water_minutes: u32,
    pub serving_ml: u32,
    pub water_start: u32,
    pub water_end: u32,
    /// 久坐提醒：连续使用电脑达到 sit_minutes 分钟后提醒起身；与喝水共用提醒时段。
    pub sit_remind: bool,
    pub sit_minutes: u32,
    /// 午休免打扰：lunch_start..lunch_end（一天中的分钟数）内喝水和久坐都不提醒。
    pub lunch_rest: bool,
    pub lunch_start: u32,
    pub lunch_end: u32,
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
    /// 天气页要看的城市；留空表示按网络出口自动定位。
    pub weather_city: String,
    /// 用户确认的具体地点；刷新时直接使用坐标，避免同名城市重新匹配。
    pub weather_position: Option<crate::weather::Position>,
    /// 已保存、可在岛上下拉切换的城市（不含自动定位），最多 `weather::MAX_CITIES` 个。
    pub weather_saved: Vec<crate::weather::Position>,
    /// 兼容渲染：用 Direct3D 12 代替 OpenGL（部分显卡的 OpenGL 驱动会崩溃或画面错乱）。重启后生效。
    pub render_compat: bool,
    /// 自动切换到兼容渲染的原因；手动开启时为空。
    pub render_compat_reason: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { display_id: String::new(), skin: Skin::Classic, island_shape: IslandShape::Notch, glass_blur: true, island_size: 100, island_offset: 0, dock: Dock::Top, settings_theme: Default::default(), pet: true, minimal: false, browser_minimal: false, avoid_browsers: true, avoid_apps: Vec::new(), visible_pages: [true; 9], page_order: [0,1,2,3,4,5,6,7,8], show_battery: true, clock_12h: false, clock_period: 0, resident_order: Vec::new(), system_hud: true, bluetooth_notifications: true, read_notifications:false, wechat_notifications:false, wecom_notifications:false, feishu_notifications:false, dingtalk_notifications:false, wechat_tray_open:false, qq_only:false, notification_preview:false, notifications: true, sound: false, lyrics_enabled:true, island_lyrics:false, lyric_delay_ms:-350, hide_fullscreen: true, avoid_top_bar: true, focus_minutes: 25, rest_minutes: 5, water_minutes: 45, serving_ml: 250, water_start: 9, water_end: 22, sit_remind: true, sit_minutes: 60, lunch_rest: true, lunch_start: 720, lunch_end: 810, mail_enabled: true, mail_sync_operations:false, mail_code: true, mail_code_hide_seconds:60, mail_interval: 120, mail_ics: 15, ignored_mail_events: Vec::new(), calendar_prompt: true, calendar_auto_sync: true, clipboard_history: true, weather_city: String::new(), weather_position: None, weather_saved: Vec::new(), render_compat: false, render_compat_reason: String::new(), open_anim: OpenAnim::default(), hover_delay_ms: 200, phone_battery: true, notice_seconds: 10, otp_seconds: 90, notice_keep: true, auto_copy_otp: false, update_check: false, phone_alerts:true, battery_alert: 20, quota_alert: 20, notice_keywords: String::new(), quiet_minutes: 0, quiet_meeting: true, mail_filter: String::new() }
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
                        if order.len()==9 && (0..9).all(|page|order.contains(&page)) {
                            s.page_order.copy_from_slice(&order);
                        } else if order.len()==8 && (0..8).all(|page|order.contains(&page)) {
                            // 旧配置里没有天气页：接在最后，其余顺序保持不变。
                            s.page_order[..8].copy_from_slice(&order);
                            s.page_order[8]=8;
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
                "visible_pages" if (6..=9).contains(&v.len()) && v.bytes().all(|b| b==b'0' || b==b'1') => {
                    for (enabled,value) in s.visible_pages.iter_mut().zip(v.bytes()) { *enabled=value==b'1'; }
                    for enabled in &mut s.visible_pages[v.len()..]{*enabled=v.bytes().any(|b|b==b'1');}
                }
                "read_notifications" => s.read_notifications=v=="1",
                "wechat_notifications" => s.wechat_notifications=v=="1",
                "wecom_notifications" => s.wecom_notifications=v=="1",
                "feishu_notifications" => s.feishu_notifications=v=="1",
                "dingtalk_notifications" => s.dingtalk_notifications=v=="1",
                "wechat_tray_open" => s.wechat_tray_open=v=="1",
                "qq_only" => s.qq_only=v=="1",
                "phone_alerts" => s.phone_alerts=v!="0",
                "notification_preview" => s.notification_preview=v=="1",
                "system_hud" => s.system_hud = v != "0",
                "bluetooth_notifications" => s.bluetooth_notifications = v != "0",
                "notifications" => s.notifications = v != "0",
                "sound" => s.sound = v == "1",
                "lyrics_enabled" => s.lyrics_enabled=v!="0",
                "island_lyrics" => s.island_lyrics=v=="1",
                "lyric_delay_ms" => s.lyric_delay_ms=v.parse::<i32>().unwrap_or(-350).clamp(-5000,5000),
                "hide_fullscreen" => s.hide_fullscreen = v != "0",
                "avoid_top_bar" => s.avoid_top_bar = v != "0",
                "focus_minutes" => s.focus_minutes = v.parse::<u32>().unwrap_or(25).clamp(1, 180),
                "rest_minutes" | "short_minutes" => s.rest_minutes = v.parse::<u32>().unwrap_or(5).clamp(1, 60),
                "water_minutes" => s.water_minutes = v.parse::<u32>().unwrap_or(45).clamp(5, 180),
                "serving_ml" => s.serving_ml = v.parse::<u32>().unwrap_or(250).clamp(50, 1000),
                "water_start" => s.water_start = v.parse::<u32>().unwrap_or(9).min(23),
                "water_end" => s.water_end = v.parse::<u32>().unwrap_or(22).min(24),
                "sit_remind" => s.sit_remind = v != "0",
                "sit_minutes" => s.sit_minutes = v.parse::<u32>().unwrap_or(60).clamp(15, 180),
                "lunch_rest" => s.lunch_rest = v != "0",
                "lunch_start" => s.lunch_start = v.parse::<u32>().unwrap_or(720).min(1439),
                "lunch_end" => s.lunch_end = v.parse::<u32>().unwrap_or(810).min(1439),
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
                "weather_city" => s.weather_city = v.trim().to_string(),
                "weather_position" => s.weather_position=serde_json::from_str::<Option<crate::weather::Position>>(&v).ok().flatten().filter(|p|p.valid()),
                "weather_saved" => s.weather_saved=serde_json::from_str::<Vec<crate::weather::Position>>(&v).unwrap_or_default().into_iter().filter(|p|p.valid()).collect(),
                "render_compat" => s.render_compat = v == "1",
                "render_compat_reason" => s.render_compat_reason = v.chars().filter(|c| !c.is_control()).take(200).collect(),
                _ => {}
            }
        }
        if !has_delay && legacy_relay_off { s.hover_delay_ms = 0; }
        if !s.visible_pages.contains(&true) { s.visible_pages[0]=true; }
        // 旧版只有一个指定城市：迁移进已保存列表，升级后下拉框里能直接看到它。
        if let Some(position)=s.weather_position.clone().filter(|p|p.label==s.weather_city) {
            if !s.weather_saved.contains(&position) {s.weather_saved.insert(0,position);}
        }
        s.weather_saved.dedup();s.weather_saved.truncate(crate::weather::MAX_CITIES);
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
                ("wecom_notifications".into(),(self.wecom_notifications as u8).to_string()),
                ("feishu_notifications".into(),(self.feishu_notifications as u8).to_string()),
                ("dingtalk_notifications".into(),(self.dingtalk_notifications as u8).to_string()),
                ("wechat_tray_open".into(),(self.wechat_tray_open as u8).to_string()),
                ("qq_only".into(),(self.qq_only as u8).to_string()),
                ("phone_alerts".into(),(self.phone_alerts as u8).to_string()),
                ("notification_preview".into(),(self.notification_preview as u8).to_string()),
                ("system_hud".into(), (self.system_hud as u8).to_string()),
                ("bluetooth_notifications".into(), (self.bluetooth_notifications as u8).to_string()),
                ("notifications".into(), (self.notifications as u8).to_string()),
                ("sound".into(), (self.sound as u8).to_string()),
                ("lyrics_enabled".into(),(self.lyrics_enabled as u8).to_string()),
                ("island_lyrics".into(),(self.island_lyrics as u8).to_string()),
                ("lyric_delay_ms".into(),self.lyric_delay_ms.to_string()),
                ("hide_fullscreen".into(), (self.hide_fullscreen as u8).to_string()),
                ("avoid_top_bar".into(), (self.avoid_top_bar as u8).to_string()),
                ("focus_minutes".into(), self.focus_minutes.to_string()),
                ("rest_minutes".into(), self.rest_minutes.to_string()),
                ("water_minutes".into(), self.water_minutes.to_string()),
                ("serving_ml".into(), self.serving_ml.to_string()),
                ("water_start".into(), self.water_start.to_string()),
                ("water_end".into(), self.water_end.to_string()),
                ("sit_remind".into(), (self.sit_remind as u8).to_string()),
                ("sit_minutes".into(), self.sit_minutes.to_string()),
                ("lunch_rest".into(), (self.lunch_rest as u8).to_string()),
                ("lunch_start".into(), self.lunch_start.to_string()),
                ("lunch_end".into(), self.lunch_end.to_string()),
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
                ("weather_city".into(), self.weather_city.clone()),
                ("weather_position".into(), serde_json::to_string(&self.weather_position).unwrap()),
                ("weather_saved".into(), serde_json::to_string(&self.weather_saved).unwrap()),
                ("render_compat".into(), (self.render_compat as u8).to_string()),
                ("render_compat_reason".into(), self.render_compat_reason.clone()),
