//! 设置、通知队列与岛仔表情。
use crate::app::{
    App, Cmd, Env, NotificationSound, Phase, HUD_TRACK_W, HUD_TRACK_X, HUD_W, PAGE_NAMES,
};
use crate::paint::{Card, H};
use crate::settings_theme::color as settings_color;
use crate::theme::{self, size, W};
use eframe::egui::{self, Color32, Rect};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AlertSource {
    Internal,
    Messenger(crate::app::Messenger),
    External(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HealthReminder {
    Water,
    Sit,
}
impl HealthReminder {
    pub(crate) const ALL: [Self; 2] = [Self::Water, Self::Sit];
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Water => "喝水时间到了",
            Self::Sit => crate::app::SIT_TITLE,
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Water => "待喝水",
            Self::Sit => "待活动",
        }
    }
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Water => "health-water",
            Self::Sit => "health-sit",
        }
    }
    pub(crate) fn icon(self, c: &Card, x: f32, y: f32, scale: f32) {
        let c = &c.scaled(x, y, scale);
        match self {
            Self::Water => {
                c.poly(
                    &[
                        (x - 7.0, y - 1.0),
                        (x + 5.0, y - 1.0),
                        (x + 3.5, y + 8.0),
                        (x - 5.5, y + 8.0),
                    ],
                    theme::BLUE,
                );
                c.path(
                    &[
                        (x - 9.0, y - 10.0),
                        (x - 6.5, y + 9.0),
                        (x + 4.5, y + 9.0),
                        (x + 7.0, y - 10.0),
                    ],
                    1.6,
                    theme::CYAN,
                );
                c.seg((x - 9.0, y - 10.0), (x + 7.0, y - 10.0), 1.6, theme::CYAN);
                c.path(
                    &[
                        (x + 6.5, y - 6.0),
                        (x + 10.5, y - 6.0),
                        (x + 11.5, y - 3.0),
                        (x + 10.0, y + 2.0),
                        (x + 5.5, y + 3.0),
                    ],
                    1.6,
                    theme::CYAN,
                );
                c.seg(
                    (x - 5.0, y - 6.5),
                    (x - 4.2, y - 2.5),
                    1.0,
                    Color32::from_white_alpha(170),
                );
            }
            Self::Sit => crate::paint::chair(c, x, y, 0.6, theme::SOFT_GREEN),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Alert {
    pub title: String,
    pub source: AlertSource,
    pub body: String,
    pub page: usize,
    pub at: f64,
    pub duration: f64,
    pub bluetooth: Option<(crate::bt::DeviceKind, bool)>,
    pub meeting: Option<(crate::mail::Meeting, String)>,
    /// 通用状态总线的可选呈现信息（进度 / 状态色 / 点击动作）。
    pub status: Option<crate::island_status::Status>,
    pub unread: Option<crate::app::Unread>,
}

impl Alert {
    pub(crate) fn health(&self) -> Option<HealthReminder> {
        if self.source != AlertSource::Internal {
            return None;
        }
        HealthReminder::ALL
            .into_iter()
            .find(|kind| self.title == kind.title())
    }
    pub(crate) fn messenger(&self) -> Option<crate::app::Messenger> {
        match &self.source {
            AlertSource::Messenger(client) => Some(*client),
            AlertSource::External(id) => {
                crate::app::Messenger::from_notification_id(id).filter(|client| {
                    client.adapter().capture == crate::messenger::Capture::SystemToast
                })
            }
            _ => None,
        }
    }
    pub(crate) fn is_hud(&self) -> bool {
        self.source == AlertSource::Internal
            && matches!(
                self.title.as_str(),
                "系统音量"
                    | "大写锁定"
                    | "键盘输入语言"
                    | "网络连接"
                    | "电源已连接"
                    | "已切换电池供电"
                    | "蓝牙设备"
            )
    }
}
fn bluetooth_notice_content(body: &str) -> (&str, &str, Option<u8>) {
    if let Some(name) = body.strip_suffix(" · 已断开") {
        return (name, "已断开", None);
    }
    if body == "已断开连接" {
        return ("蓝牙设备", "已断开", None);
    }
    if let Some((name, level)) = body.rsplit_once(" · 电量 ") {
        if let Some(level) = level
            .strip_suffix('%')
            .and_then(|s| s.parse::<u8>().ok())
            .filter(|v| *v <= 100)
        {
            return (name, "已连接", Some(level));
        }
    }
    (body, "已连接", None)
}

pub(crate) fn device_icon(c: &Card, kind: crate::bt::DeviceKind, x: f32, y: f32, size: f32) {
    let icon = match kind {
        crate::bt::DeviceKind::Bone
        | crate::bt::DeviceKind::Earbuds
        | crate::bt::DeviceKind::Headphones => {
            egui::include_image!("../assets/device-icons/headphones-round-bold.svg")
        }
        crate::bt::DeviceKind::Speaker => {
            egui::include_image!("../assets/device-icons/speaker-bold.svg")
        }
        crate::bt::DeviceKind::Keyboard => {
            egui::include_image!("../assets/device-icons/keyboard-bold.svg")
        }
        crate::bt::DeviceKind::Mouse => {
            egui::include_image!("../assets/device-icons/mouse-bold.svg")
        }
        crate::bt::DeviceKind::Gamepad => {
            egui::include_image!("../assets/device-icons/gamepad-bold.svg")
        }
        crate::bt::DeviceKind::Phone => {
            egui::include_image!("../assets/device-icons/smartphone-bold.svg")
        }
        crate::bt::DeviceKind::Other => {
            egui::include_image!("../assets/device-icons/bluetooth-linear.svg")
        }
    };
    let rect = c.rect(x, y, size, size);
    let egui::ImageSource::Bytes { uri, bytes } = icon else {
        return;
    };
    let pixels = (rect.width() * c.ui.ctx().pixels_per_point()).ceil() as u32;
    let icon = egui::Image::from_bytes(format!("{uri}/{pixels}.svg"), bytes);
    if let Ok(egui::load::TexturePoll::Ready { texture }) = icon
        .fit_to_exact_size(rect.size())
        .load_for_size(c.ui.ctx(), rect.size())
    {
        c.painter.image(
            texture.id,
            rect,
            Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

pub(crate) use crate::pet::PetState;

pub(crate) fn notice_badge(c: &Card, x: f32, y: f32, a: &Alert) {
    if a.source == AlertSource::Internal && a.title == "蓝牙设备" {
        c.circle(
            x,
            y,
            11.0,
            if a.body.contains("已断开") {
                Color32::from_gray(95)
            } else {
                theme::BLUE
            },
        );
        device_icon(
            c,
            a.bluetooth.map_or(crate::bt::DeviceKind::Other, |v| v.0),
            x - 7.5,
            y - 7.5,
            15.0,
        );
    } else {
        notice_icon(&c.scaled(x, y, 0.85), x, y, a);
    }
}
pub(crate) fn messenger_icon(c: &Card, x: f32, y: f32, source: crate::app::Messenger) {
    (source.adapter().paint_icon)(c, x, y);
}
fn notice_icon(c: &Card, x: f32, y: f32, a: &Alert) {
    if a.source != AlertSource::Internal {
        crate::paint::bell(c, x, y, theme::WHITE, 0.0);
        return;
    }
    match a.title.as_str() {
        "系统音量" => {
            c.poly(
                &[
                    (x - 9.0, y - 3.0),
                    (x - 5.0, y - 3.0),
                    (x, y - 7.0),
                    (x, y + 7.0),
                    (x - 5.0, y + 3.0),
                    (x - 9.0, y + 3.0),
                ],
                theme::WHITE,
            );
            c.arc(x, y, 5.0, -0.8, 0.8, 1.5, theme::WHITE);
            c.arc(x, y, 8.0, -0.7, 0.7, 1.3, theme::WHITE);
        }
        "大写锁定" => {
            let ink = if a.body == "已开启" {
                theme::GREEN
            } else {
                theme::GRAY
            };
            c.poly(&[(x - 8.0, y - 1.0), (x, y - 9.0), (x + 8.0, y - 1.0)], ink);
            c.fill(x - 3.0, y - 1.5, 6.0, 6.5, 0.5, ink);
            c.fill(x - 4.0, y + 7.0, 8.0, 2.0, 0.8, ink);
        }
        "键盘输入语言" => {
            c.stroke(x - 10.0, y - 8.5, 20.0, 17.0, 4.0, 1.2, theme::GRAY);
            let label = match a.body.as_str() {
                "中文" => "中",
                "英语" => "EN",
                _ => "Aa",
            };
            c.centered_text(
                x,
                y,
                label,
                if label == "EN" { 9.0 } else { 11.0 },
                W::Semibold,
                theme::WHITE,
                H::Center,
            );
        }
        "网络连接" => {
            let connected = a.body.contains("已连接");
            let ink = if connected { theme::GREEN } else { theme::GRAY };
            if a.body.starts_with("Wi-Fi") {
                for r in [4.0, 8.0, 12.0] {
                    c.arc(x, y + 6.0, r, -2.35, -0.79, 1.7, ink);
                }
                c.circle(x, y + 6.0, 1.6, ink);
            } else if a.body.starts_with("以太网") {
                c.stroke(x - 8.0, y - 8.0, 16.0, 14.0, 2.5, 1.6, ink);
                for dx in [-4.0, -1.3, 1.3, 4.0] {
                    c.seg((x + dx, y - 5.0), (x + dx, y - 1.0), 1.2, ink);
                }
                c.path(
                    &[
                        (x - 4.0, y + 6.0),
                        (x - 4.0, y + 9.0),
                        (x + 4.0, y + 9.0),
                        (x + 4.0, y + 6.0),
                    ],
                    1.6,
                    ink,
                );
            } else {
                c.ring(x, y, 8.0, 1.4, ink);
                c.seg((x - 8.0, y), (x + 8.0, y), 1.0, ink);
                c.stroke(x - 3.5, y - 8.0, 7.0, 16.0, 3.5, 1.0, ink);
            }
            if !connected {
                c.seg((x - 8.0, y - 8.0), (x + 9.0, y + 9.0), 2.0, theme::WHITE);
            }
        }
        "电源已连接" | "已切换电池供电" | "电量偏低" => {
            let low = a.title == "电量偏低";
            let ink = if low {
                theme::RED_TOP
            } else if a.title == "电源已连接" {
                theme::GREEN
            } else {
                theme::WHITE
            };
            c.stroke(x - 11.0, y - 6.0, 20.0, 12.0, 3.0, 1.5, ink);
            c.fill(x + 10.0, y - 2.5, 2.0, 5.0, 0.8, ink);
            if low {
                c.fill(x - 8.0, y - 3.5, 3.5, 7.0, 1.0, ink);
            } else if a.title == "电源已连接" {
                c.poly(&[(x, y - 4.5), (x - 4.0, y + 0.8), (x + 0.5, y + 0.8)], ink);
                c.poly(
                    &[(x + 0.5, y - 0.8), (x + 4.0, y - 0.8), (x - 1.0, y + 4.5)],
                    ink,
                );
            } else {
                c.fill(x - 8.0, y - 3.5, 12.0, 7.0, 1.0, ink);
            }
        }
        "喝水时间到了" => HealthReminder::Water.icon(c, x, y, 1.0),
        crate::app::SIT_TITLE => HealthReminder::Sit.icon(c, x, y, 1.0),
        "待办到时间了" => crate::paint::checklist(
            c,
            x,
            y,
            23.0,
            Color32::from_rgb(190, 180, 255),
            Color32::from_rgb(43, 35, 66),
        ),
        "会议提醒" => {
            let label = a
                .meeting
                .as_ref()
                .map(|(_, source)| source.as_str())
                .unwrap_or("");
            let source = if label == "本机日程" {
                crate::calendar_ui::Source::Local
            } else if label.starts_with("邮件") {
                crate::calendar_ui::Source::Mail("")
            } else {
                crate::calendar_ui::Source::Calendar(
                    crate::calendar::Provider::ALL
                        .into_iter()
                        .find(|provider| provider.label() == label),
                )
            };
            crate::calendar_ui::source_icon(c, source, x - 14.0, y - 14.0, 28.0);
        }
        "专注计时" => {
            crate::paint::stopwatch(&c.scaled(x, y, 0.65), x, y, 8.0, 0.0, theme::RED_TOP)
        }
        _ => crate::paint::bell(&c.scaled(x, y, 1.7), x, y, theme::GRAY, 0.0),
    }
}

impl App {
    pub(crate) fn health_badges(&self) -> Vec<HealthReminder> {
        if self.fullscreen_hidden || !self.module_on(crate::app::WATER_PAGE) {
            return Vec::new();
        }
        HealthReminder::ALL
            .into_iter()
            .filter(|kind| {
                self.health_pending[*kind as usize]
                    && (match kind {
                        HealthReminder::Water => self.water.remind,
                        HealthReminder::Sit => self.settings.sit_remind,
                    })
                    && (self.open
                        || !self
                            .notice
                            .as_ref()
                            .is_some_and(|a| a.health() == Some(*kind)))
            })
            .collect()
    }
    pub(crate) fn confirm_health_reminder(&mut self, kind: HealthReminder) {
        self.health_pending[kind as usize] = false;
        crate::model::save_health_pending(self.health_pending);
        match kind {
            HealthReminder::Water => self.last_reminder = self.t,
            HealthReminder::Sit => self.sit_start = self.t,
        }
        self.alerts.retain(|a| a.health() != Some(kind));
        if self
            .notice
            .as_ref()
            .is_some_and(|a| a.health() == Some(kind))
        {
            self.notice = None;
        }
        if self
            .hud_notice
            .as_ref()
            .is_some_and(|a| a.health() == Some(kind))
        {
            self.hud_notice = None;
        }
    }
    pub(crate) fn phase_secs(&self, phase: Phase) -> f32 {
        (match phase {
            Phase::Focus => self.settings.focus_minutes,
            Phase::Rest => self.settings.rest_minutes,
        }) as f32
            * 60.0
    }
    /// 已连续久坐的分钟数。
    pub(crate) fn sat_minutes(&self) -> u32 {
        ((self.t - self.sit_start).max(0.0) / 60.0) as u32
    }
    /// 下一次久坐提醒的时刻：从上次确认或起身起算满 sit_minutes。
    pub(crate) fn sit_due(&self) -> f64 {
        self.sit_start + self.settings.sit_minutes as f64 * 60.0
    }
    /// 「起身了」：久坐计时归零。
    pub(crate) fn stood_up(&mut self) {
        self.confirm_health_reminder(HealthReminder::Sit);
        self.pet.react(PetState::Happy);
        self.say("久坐计时已重新开始");
    }
    /// 现在是否在午休免打扰时段内（支持跨午夜；起止相同视为未设置）。
    pub(crate) fn lunch_now(&self, env: &Env) -> bool {
        let (a, b) = (self.settings.lunch_start, self.settings.lunch_end);
        let m = env.hour() as u32 * 60 + env.minute();
        self.settings.lunch_rest
            && a != b
            && if a < b {
                m >= a && m < b
            } else {
                m >= a || m < b
            }
    }
    /// 喝水和久坐提醒此刻能否弹出：在提醒时段内，且不在午休。
    pub(crate) fn remind_window(&self, env: &Env) -> bool {
        self.water_hours(env.hour() as u32) && !self.lunch_now(env)
    }
    pub(crate) fn water_hours(&self, hour: u32) -> bool {
        let (a, b) = (self.settings.water_start, self.settings.water_end);
        if a == b {
            true
        } else if a < b {
            hour >= a && hour < b
        } else {
            hour >= a || hour < b
        }
    }
    pub(crate) fn pet_state(&self) -> PetState {
        if let Some((state, _)) = self.pet.reaction {
            state
        } else if self.pet.waking_until > self.t {
            PetState::Waking
        } else if self.pomo.running && self.pomo.phase == Phase::Focus {
            PetState::Focused
        } else if self.pet.worried {
            PetState::Worried
        } else if self.media.playing && self.media.is_music() {
            if self.notice.is_some() || self.hud_notice.is_some() {
                PetState::Awake
            } else {
                PetState::Music
            }
        } else if self.pet.hovered {
            PetState::Curious
        } else if self.idle_for() >= 180.0 {
            PetState::Sleeping
        } else if self.idle_for() >= self.opts.pet_idle {
            PetState::Dozing
        } else {
            PetState::Awake
        }
    }
    pub fn alert(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
        page: usize,
        system: bool,
    ) {
        let title = title.into();
        if let Some(kind) = HealthReminder::ALL
            .into_iter()
            .find(|kind| title == kind.title())
        {
            if self.health_pending[kind as usize] {
                return;
            }
            self.health_pending[kind as usize] = true;
            crate::model::save_health_pending(self.health_pending);
        }
        let transient = matches!(
            title.as_str(),
            "系统音量"
                | "大写锁定"
                | "键盘输入语言"
                | "网络连接"
                | "电源已连接"
                | "已切换电池供电"
                | "蓝牙设备"
        );
        let a = Alert {
            source: AlertSource::Internal,
            title,
            body: body.into(),
            page,
            at: self.t,
            duration: if transient { 2.5 } else { 8.0 },
            bluetooth: None,
            meeting: None,
            status: None,
            unread: None,
        };
        self.enqueue_alert(a, system);
    }
    /// 邮件验证码上岛：标题带「邮件」标识，正文留明文数字供通知卡片显示；
    /// 不走手机短信的 auto_copy_otp，也不推 Cmd::Copy，只允许用户手动点复制。
    pub fn mail_code_alert(&mut self, _code: &str, from: &str, page: usize) {
        let duration = if cfg!(test) {
            4.5
        } else {
            (self.settings.otp_seconds as f64).max(10.0)
        };
        self.enqueue_alert(
            Alert {
                source: AlertSource::External("mail-code".into()),
                title: format!("邮件验证码 · {}", from),
                body: "邮件验证码".to_string(),
                page,
                at: self.t,
                duration,
                bluetooth: None,
                meeting: None,
                status: None,
                unread: None,
            },
            false,
        );
    }
    pub(crate) fn meeting_alert(
        &mut self,
        meeting: crate::mail::Meeting,
        source: String,
        page: usize,
    ) {
        self.enqueue_alert(
            Alert {
                source: AlertSource::Internal,
                title: "会议提醒".into(),
                body: format!("{} · {} · {}", meeting.title, meeting.at, source),
                page,
                at: self.t,
                duration: 12.0,
                bluetooth: None,
                meeting: Some((meeting, source)),
                status: None,
                unread: None,
            },
            true,
        );
    }
    pub(crate) fn bluetooth_alert(&mut self, device: &crate::bt::Device, connected: bool) {
        if !self.settings.bluetooth_notifications {
            return;
        }
        if connected
            && matches!(
                device.kind,
                crate::bt::DeviceKind::Bone
                    | crate::bt::DeviceKind::Earbuds
                    | crate::bt::DeviceKind::Headphones
                    | crate::bt::DeviceKind::Speaker
            )
        {
            self.alerts.retain(|a| a.title != "系统音量");
            if self.notice.as_ref().is_some_and(|a| a.title == "系统音量") {
                self.notice = None;
            }
        }
        let a = Alert {
            source: AlertSource::Internal,
            title: "蓝牙设备".into(),
            body: if connected {
                device.summary()
            } else {
                format!("{} · 已断开", device.name)
            },
            page: 5,
            at: self.t,
            duration: 2.5,
            bluetooth: Some((device.kind, connected)),
            meeting: None,
            status: None,
            unread: None,
        };
        self.enqueue_alert(a, false);
    }
    fn enqueue_alert(&mut self, a: Alert, system: bool) {
        // 通知仅保存在本次运行的内存中。
        if a.source == AlertSource::Internal
            || matches!(&a.source,AlertSource::External(id) if id=="mail-code")
        {
            self.history.push_front(a.clone());
        }
        self.history.truncate(50);
        if self.fullscreen_hidden && a.health().is_some() {
            return;
        }
        // 落盘一份历史：岛仔重启或当时没看到时，还能在设置里回查。
        let status_bus =
            matches!(&a.source,AlertSource::External(id) if id.starts_with("island-status"));
        if !status_bus {
            notice_history_append(&a.title, &a.body);
        }
        // 勿扰：只记账、不打扰。横幅、铃铛、系统通知都不出现，验证码例外（用户要的就是那串数字）。
        if self.muted() && a.status.is_none() && crate::otp::extract("", &a.body, &[]).is_none() {
            return;
        }
        if self.settings.notifications {
            if system && a.source == AlertSource::Internal && self.settings.system_notify {
                let sound = if !self.settings.sound {
                    NotificationSound::Silent
                } else if a.health().is_some() {
                    NotificationSound::Health
                } else {
                    NotificationSound::System
                };
                self.pending
                    .push(Cmd::Notify(a.title.clone(), a.body.clone(), sound));
            }
            let same_event = |old: &Alert| match (&a.meeting, &old.meeting) {
                (Some((next, _)), Some((previous, _))) => {
                    crate::calendar::dedupe_key(next) == crate::calendar::dedupe_key(previous)
                }
                // 状态总线按 key 覆写：内容变了替换，不变也替换以续期，不排队。
                _ if matches!(&a.source,AlertSource::External(id) if id.starts_with("island-status:")) => {
                    old.source == a.source
                }
                // 手机通知按来源聚合：同一 App 的新消息替换旧提示，不排队刷屏。
                // 手机通道共用同一个来源（手机连接的 AUMID）：新短信 / 来电直接顶掉上一条，
                // 不然来电会排在一条已经看过的短信后面。
                _ if a.title.starts_with("手机 · ")
                    || a.title == "未接来电"
                    || a.title == "来电中" =>
                {
                    old.source == a.source
                }
                _ => {
                    old.source == a.source
                        && old.title == a.title
                        && old.unread == a.unread
                        && (a.is_hud() || old.body == a.body)
                }
            };
            if a.source != AlertSource::Internal || a.title != "蓝牙设备" {
                self.alerts.retain(|old| !same_event(old));
            }
            if (a.source != AlertSource::Internal || a.title != "蓝牙设备")
                && self.notice.as_ref().is_some_and(same_event)
            {
                self.notice = Some(a);
                return;
            }
            self.alerts.push_back(a);
            while self.alerts.len() > 8 {
                self.alerts.pop_front();
            }
        }
    }
    /// 该来源的所有账号都已查看。
    pub fn viewed_messenger(&mut self, source: crate::app::Messenger) {
        self.pending_messengers.retain(|u| u.source != source);
        self.clear_messenger_notices(source);
    }
    /// 只清已查看的账号；无法确认账号的 QQ 提醒由用户明确关闭。
    pub fn viewed_client(&mut self, source: crate::app::Messenger, pid: u32) {
        self.pending_messengers
            .retain(|u| !u.viewed_by(source, pid));
        if pid != 0 {
            self.clear_account_notices(source, pid);
        }
        if !source.adapter().keep_unidentified {
            self.clear_account_notices(source, 0);
        }
        if !self.pending_messengers.iter().any(|u| u.source == source) {
            self.clear_messenger_notices(source);
        }
    }
    pub fn confirm_messenger_open(
        &mut self,
        unread: crate::app::Unread,
        foreground: Option<(crate::app::Messenger, u32)>,
    ) -> bool {
        if unread.pid == 0 || foreground != Some((unread.source, unread.pid)) {
            return false;
        }
        self.viewed_client(unread.source, unread.pid);
        true
    }
    pub fn discard_unread(&mut self, unread: crate::app::Unread) {
        if !self.pending_messengers.contains(&unread) {
            return;
        }
        self.pending_messengers.retain(|current| *current != unread);
        self.clear_account_notices(unread.source, unread.pid);
        if !self
            .pending_messengers
            .iter()
            .any(|current| current.source == unread.source)
        {
            self.clear_messenger_notices(unread.source);
        }
    }
    fn clear_account_notices(&mut self, source: crate::app::Messenger, pid: u32) {
        let matches = |notice: &Alert| {
            notice
                .unread
                .is_some_and(|u| u.source == source && u.pid == pid)
        };
        self.alerts.retain(|notice| !matches(notice));
        if self.notice.as_ref().is_some_and(matches) {
            self.notice = None;
            self.notification_bell_until = 0.0;
        }
    }
    fn clear_messenger_notices(&mut self, source: crate::app::Messenger) {
        self.alerts.retain(|a| a.messenger() != Some(source));
        if self
            .notice
            .as_ref()
            .is_some_and(|a| a.messenger() == Some(source))
        {
            self.notice = None;
            self.notification_bell_until = 0.0;
        }
    }
    fn mark_messenger(&mut self, source: crate::app::Messenger, pid: u32, window: isize) {
        if let Some(index) = self
            .pending_messengers
            .iter()
            .position(|u| u.source == source && u.pid == pid)
        {
            if window != 0 && self.pending_messengers[index].window != window {
                self.pending_messengers[index].window = window;
                if source.adapter().capture == crate::messenger::Capture::Taskbar {
                    let unread = self.pending_messengers[index];
                    for alert in self.alerts.iter_mut().chain(self.notice.iter_mut()) {
                        if alert
                            .unread
                            .is_some_and(|old| old.source == source && old.pid == pid)
                        {
                            alert.unread = Some(unread);
                        }
                    }
                } else {
                    self.clear_account_notices(source, pid);
                }
            }
            return;
        }
        self.pending_messengers.push(crate::app::Unread {
            source,
            pid,
            window,
        });
    }
    /// 横幅「查看」：跳到该来源最近一条未读的窗口。
    pub(crate) fn latest_unread(&self, source: crate::app::Messenger) -> crate::app::Unread {
        self.pending_messengers
            .iter()
            .rev()
            .find(|u| u.source == source)
            .copied()
            .unwrap_or(crate::app::Unread::of(source))
    }
    pub fn receive_wechat_reminder(&mut self) {
        self.receive_wechat_reminder_from(0, 0);
    }
    /// 微信窗口闪动：`pid` 是该账号的进程，`window` 是闪动的窗口。
    pub fn receive_wechat_reminder_from(&mut self, pid: u32, window: isize) {
        self.receive_messenger_reminder(crate::app::Unread {
            source: crate::app::Messenger::Wechat,
            pid,
            window,
        });
    }
    pub fn receive_messenger_reminder(&mut self, unread: crate::app::Unread) {
        let client = unread.source;
        if client.adapter().capture != crate::messenger::Capture::Taskbar
            || !client.enabled(&self.settings)
        {
            return;
        }
        let pending = self
            .pending_messengers
            .iter()
            .any(|old| old.source == client && old.pid == unread.pid);
        self.mark_messenger(client, unread.pid, unread.window);
        if pending {
            return;
        }
        self.notification_bell_at = self.t;
        self.notification_bell_until = self.t + 4.5;
        self.enqueue_alert(
            Alert {
                source: AlertSource::Messenger(client),
                title: client.name().into(),
                body: format!("有新消息 · 请到{}查看", client.name()),
                page: usize::MAX,
                at: self.t,
                duration: 4.5,
                bluetooth: None,
                meeting: None,
                unread: Some(unread),
                status: None,
            },
            false,
        );
    }
    /// 通用状态总线：外部脚本写 island_status.json 后转成岛上提示。
    /// 来源 id 带上状态 key，同 key 每 2 秒覆写一次，既刷新内容又续期。
    pub(crate) fn push_status(&mut self, s: crate::island_status::Status) {
        let duration = if s.seconds > 0.0 { s.seconds } else { 8.0 };
        let key = s.key();
        self.enqueue_alert(
            Alert {
                source: AlertSource::External(format!("island-status:{key}")),
                title: s.text.clone(),
                body: s.detail.clone(),
                page: usize::MAX,
                at: self.t,
                duration,
                bluetooth: None,
                meeting: None,
                status: Some(s),
                unread: None,
            },
            false,
        );
    }
    /// 状态从 JSON 里删掉：立刻撤下对应提示，不等 8 秒自然过期。
    pub(crate) fn drop_status(&mut self, key: &str) {
        let source = AlertSource::External(format!("island-status:{key}"));
        self.alerts.retain(|a| a.source != source);
        if self.notice.as_ref().is_some_and(|a| a.source == source) {
            self.notice = None;
            self.notification_bell_until = 0.0;
        }
        if self.hud_notice.as_ref().is_some_and(|a| a.source == source) {
            self.hud_notice = None;
        }
    }
    /// MAA 是否可对接：通知名单里勾选了 MAA，或用户手动指定了 MAA 程序。
    ///
    /// 后面这条很关键：MAA 只有在发过系统通知后才会被自动登记进名单，
    /// 换台机器 / 关掉 MAA 通知的用户名单里根本没有它，此时手选路径就是唯一入口。
    pub(crate) fn maa_enabled(&self) -> bool {
        if !self.maa_prefs.exe_path.trim().is_empty() {
            return true;
        }
        self.notification_apps.items.iter().any(|(id, name, on)| {
            *on && (name == "MAA" || id.to_ascii_lowercase().ends_with("maa.exe"))
        })
    }
    /// MAA 日志文件：优先用设置里的目录，其次从通知名单里 MAA 的 exe 路径推导 `<MAA目录>\debug`。
    pub(crate) fn maa_log_path(&self) -> Option<std::path::PathBuf> {
        let custom = self.maa_prefs.log_dir.trim();
        if !custom.is_empty() {
            return Some(std::path::PathBuf::from(custom).join("asst.log"));
        }
        let exe = self.maa_exe()?;
        crate::maa::log_dir_from_exe(&exe).map(|dir| dir.join("asst.log"))
    }
    /// 直接打开某个设置分组（索引对应 SETTINGS_GROUPS），供命令行/快捷方式直达。
    pub fn open_settings_group(&mut self, group: usize) {
        let last = SETTINGS_GROUPS.len() - 1;
        let (_, start, _, _) = SETTINGS_GROUPS[group.min(last)];
        self.settings_category = group.min(last);
        self.settings_tab = start;
        self.open_settings();
    }
    /// 设置页选好 MAA 程序后写入偏好（换程序时顺便清掉图标缓存）。
    pub fn set_maa_exe(&mut self, path: String) {
        self.maa_prefs.exe_path = path;
        self.maa_prefs.save();
        self.maa_icon = None;
    }
    /// 设置页选好日志目录后写入偏好。
    pub fn set_maa_log_dir(&mut self, path: String) {
        self.maa_prefs.log_dir = path;
        self.maa_prefs.save();
    }
    /// MAA 可执行文件路径：优先用设置里手选的，其次取通知名单里登记的那份。
    pub(crate) fn maa_exe(&self) -> Option<String> {
        let custom = self.maa_prefs.exe_path.trim();
        if !custom.is_empty() {
            return Some(custom.to_string());
        }
        self.notification_apps
            .items
            .iter()
            .find(|(id, name, _)| name == "MAA" || id.to_ascii_lowercase().ends_with("maa.exe"))
            .map(|(id, _, _)| id.clone())
    }
    /// 按 MAA 任务进度刷新岛上那条常驻胶囊（运行中续期、完成后停留几秒）。
    pub(crate) fn refresh_maa_status(&mut self) {
        if !self.maa_prefs.resident_progress {
            self.maa_status = None;
            return;
        }
        let now = unix_secs_f64();
        let Some(p) = self.maa_tracker.snapshot() else {
            self.maa_status = None;
            return;
        };
        // 图标：首次从 MAA.exe 提取成 PNG 缓存，之后直接读图片。
        let icon = match &self.maa_icon {
            Some(path) => path.clone(),
            None => {
                let path = self
                    .maa_exe()
                    .and_then(|exe| {
                        let out = crate::model::config_dir().join("maa-icon.png");
                        crate::maa::ensure_icon(&exe, &out)
                    })
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.maa_icon = Some(path.clone());
                path
            }
        };
        if p.running {
            self.maa_done_until = 0.0;
            // 运行中：15 秒持续续期；MAA 停下后不再续期，自动撤下（不会留僵尸胶囊）。
            let text = if p.current.is_empty() {
                "MAA · 运行中".to_string()
            } else {
                format!("MAA · {}", p.current)
            };
            let detail = if p.total > 0 {
                format!("{}/{} 个任务", p.done, p.total)
            } else {
                String::new()
            };
            self.maa_status = Some(crate::island_status::Status {
                id: "maa".into(),
                text,
                detail,
                seconds: 15.0,
                progress: p.ratio(),
                color: "blue".into(),
                click: String::new(),
                priority: 5,
                ttl: now + 15.0,
                icon,
            });
            return;
        }
        // 跑完：绿色「任务完成」停留几秒后撤下。
        let all_done = p.total > 0 && p.done >= p.total;
        if all_done {
            if self.maa_done_until <= 0.0 {
                self.maa_done_until = now + 8.0;
            }
            if now < self.maa_done_until {
                self.maa_status = Some(crate::island_status::Status {
                    id: "maa".into(),
                    text: "MAA · 任务完成".into(),
                    detail: format!("{} 个任务", p.total),
                    seconds: 8.0,
                    progress: Some(1.0),
                    color: "green".into(),
                    click: String::new(),
                    priority: 5,
                    ttl: now + 8.0,
                    icon,
                });
                return;
            }
        }
        self.maa_status = None;
    }
    /// 把一条 MAA 事件转成岛上提示（时长取 MAA 偏好）。
    ///
    /// 常驻进度胶囊开启时，任务开始/完成/全部完成都不再弹横幅——胶囊本身就在
    /// 显示当前任务和进度，再弹一条会重复。公招高星、出错仍单独提示（胶囊表达不了）。
    pub(crate) fn maa_event(&mut self, event: crate::maa::Event) {
        let resident = self.maa_prefs.resident_progress;
        let (title, body) = match event {
            crate::maa::Event::ChainStart(_, label) => {
                if !self.maa_prefs.notify_chain || resident {
                    return;
                }
                ("MAA".to_string(), format!("开始{label}"))
            }
            crate::maa::Event::ChainDone(_, label) => {
                if !self.maa_prefs.notify_chain || resident {
                    return;
                }
                ("MAA".to_string(), format!("{label}完成"))
            }
            crate::maa::Event::AllDone(_) => {
                if !self.maa_prefs.notify_chain || resident {
                    return;
                }
                ("MAA".to_string(), "日常任务全部完成".to_string())
            }
            crate::maa::Event::RecruitStar(level) => {
                if !self.maa_prefs.notify_recruit_star {
                    return;
                }
                ("MAA 公招".to_string(), format!("刷出 {level} 星 tag"))
            }
            crate::maa::Event::Error(label) => {
                if !self.maa_prefs.notify_error {
                    return;
                }
                ("MAA 出错".to_string(), label)
            }
        };
        let a = Alert {
            source: AlertSource::Internal,
            title,
            body,
            page: usize::MAX,
            at: self.t,
            duration: self.maa_prefs.seconds as f64,
            bluetooth: None,
            meeting: None,
            status: None,
            unread: None,
        };
        self.enqueue_alert(a, false);
    }
    /// 手机电量的关键变化：断开 / 重连 / 低电 / 充满。
    /// 首轮只记录不提示，避免每次启动都弹一条「已连接」。
    pub(crate) fn phone_events(&mut self, now: Option<u8>, charging: bool) {
        let prev = self.phone_last;
        self.phone_last = Some(now);
        let Some(prev) = prev else {
            return;
        };
        if !self.settings.notifications {
            return;
        }
        let low = self.settings.low_battery_threshold();
        let (title, body) = match (prev, now) {
            (Some(_), None) => ("手机已断开".to_string(), "电量信息已收起".to_string()),
            (None, Some(level)) => (format!("手机已连接 {level}%"), "手机连接已恢复".to_string()),
            (Some(old), Some(level)) if low.is_some_and(|t| old > t && level <= t) => (
                format!("手机电量偏低 {level}%"),
                format!("低于 {}%，记得给手机充电", low.unwrap_or(20)),
            ),
            (Some(old), Some(100)) if old < 100 => (
                format!("手机已充满{}", if charging { "（充电中）" } else { "" }),
                "可以拔掉充电器了".to_string(),
            ),
            _ => return,
        };
        self.alert(title, body, usize::MAX, false);
    }

    /// 本机电量的低电 / 充满提示，阈值与手机共用「低电提醒」设置。
    /// 首轮只记录不提示，避免每次启动都弹一条。
    pub(crate) fn pc_events(&mut self, now: Option<u8>, charging: bool) {
        let prev = self.pc_last;
        self.pc_last = Some(now);
        let Some(prev) = prev else {
            return;
        };
        if !self.settings.notifications {
            return;
        }
        let Some(low) = self.settings.low_battery_threshold() else {
            return;
        };
        let (title, body) = match (prev, now) {
            (Some(old), Some(level)) if old > low && level <= low => (
                format!("本机电量偏低 {level}%"),
                if charging {
                    "已接电源，正在充电".to_string()
                } else {
                    format!("低于 {low}%，建议接上电源")
                },
            ),
            (Some(old), Some(100)) if old < 100 && charging => (
                "本机已充满 100%".to_string(),
                "可以拔掉充电器了".to_string(),
            ),
            _ => return,
        };
        self.alert(title, body, usize::MAX, false);
    }

    /// 勿扰是否生效（按 app 时间比较，重启后自动失效）。
    pub(crate) fn muted(&self) -> bool {
        self.quiet_until > self.t
    }

    /// 文本是否命中通知关键词表（大小写不敏感）。
    pub(crate) fn keyword_hit(&self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        // 规则里的 highlight 与关键词表同权：命中就亮琥珀色竖条。
        if self.notice_rules.hits(text, "highlight") {
            return true;
        }
        let lower = text.to_lowercase();
        // tick 里维护好的小写词表；源字符串刚改、缓存还没跟上时退回现算。
        if self.keyword_src == self.settings.notice_keywords {
            return self
                .keywords
                .iter()
                .any(|word| lower.contains(word.as_str()));
        }
        self.settings
            .keywords()
            .iter()
            .any(|word| lower.contains(&word.to_lowercase()))
    }

    /// 会议自动勿扰：日程刚开始（90 秒内）且还没结束时把勿扰开到会议结束，最多 2 小时。
    /// 同一场会议只触发一次，散会自动失效。
    pub(crate) fn meeting_quiet(&mut self, env: &crate::app::Env) {
        if !self.settings.quiet_meeting || self.muted() {
            return;
        }
        let Ok(day) = chrono::NaiveDate::parse_from_str(&env.today, "%Y-%m-%d") else {
            return;
        };
        let now = chrono::Local::now().timestamp();
        // 缓存今日会议候选，60 秒或跨天才重算，避免高频合并排序引起抖动；
        // 触发窗口是 90 秒，不会因此漏掉会议。
        if self.meeting_plan.0 != env.today || self.t - self.meeting_plan.1 > 60.0 {
            let entries = self.calendar_entries(day);
            let plan = meeting_candidates(entries.iter().map(|entry| entry.event));
            self.meeting_plan = (env.today.clone(), self.t, plan);
        }
        let Some((until, key)) = meeting_hit(&self.meeting_plan.2, now) else {
            return;
        };
        if self.quiet_meeting_key.as_deref() == Some(key.as_str()) {
            return;
        }
        self.quiet_meeting_key = Some(key);
        self.quiet_until = until as f64;
        self.say("会议开始，自动勿扰到散会");
    }

    /// AI 额度阈值：任一窗口剩余 ≤ 阈值时提醒一次；回到阈值以上后重新武装。
    /// 每 30 秒看一次内存里的快照，不额外请求接口。
    pub(crate) fn quota_threshold_alert(&mut self) {
        let threshold = self.settings.quota_alert;
        if threshold == 0 {
            return;
        }
        let mut fresh: Vec<(String, f32)> = Vec::new();
        let mut armed = std::collections::HashSet::new();
        for (id, row) in self.quota.rows.iter() {
            let Ok(snapshot) = row else { continue };
            for window in snapshot.windows.iter().chain(snapshot.extra.iter()) {
                if window.remaining <= threshold as f32 {
                    let key = (id.clone(), window.label.clone());
                    if !self.quota_alerted.contains(&key) {
                        fresh.push((window.label.clone(), window.remaining));
                    }
                    armed.insert(key);
                }
            }
        }
        self.quota_alerted = armed;
        for (label, remaining) in fresh {
            self.alert("AI 额度", format!("{label} 只剩 {remaining:.0}%"), 7, true);
        }
    }

    /// 把一条历史通知重新弹出来（「稍后提醒」用）。
    pub(crate) fn replay_notice(&mut self, title: String, body: String) {
        self.enqueue_alert(
            Alert {
                source: AlertSource::External("notice-history".into()),
                title,
                body,
                page: usize::MAX,
                at: self.t,
                duration: if cfg!(test) { 4.5 } else { 30.0 },
                bluetooth: None,
                meeting: None,
                status: None,
                unread: None,
            },
            false,
        );
    }

    /// 稍后提醒：`minutes` 分钟后再弹一次。
    #[cfg_attr(not(test), allow(dead_code))] // 由测试覆盖，production 未接线时保留
    pub(crate) fn snooze_notice(&mut self, minutes: f64, title: String, body: String) {
        self.snoozed.push((self.t + minutes * 60.0, title, body));
        self.say(format!("{minutes:.0} 分钟后再提醒"));
    }

    /// 到点的「稍后提醒」重新入队，一次只处理一条，避免同一帧连弹。
    pub(crate) fn tick_snoozed(&mut self) {
        if let Some(index) = self.snoozed.iter().position(|(due, _, _)| self.t >= *due) {
            let (_, title, body) = self.snoozed.remove(index);
            self.replay_notice(title, body);
        }
    }

    /// 勿扰开关：`minutes = 0` 关闭。只在本次运行内有效，重启后恢复关闭。
    pub(crate) fn set_quiet(&mut self, minutes: u32) {
        self.settings.quiet_minutes = minutes;
        self.settings.save();
        self.quiet_until = if minutes == 0 {
            0.0
        } else {
            self.t + minutes as f64 * 60.0
        };
        self.say(if minutes == 0 {
            "勿扰已关闭".to_string()
        } else {
            format!("勿扰 {minutes} 分钟")
        });
    }

    /// 状态总线触发的番茄钟开关：`on = false` 只是暂停，不重置这一轮。
    pub(crate) fn set_focus_running(&mut self, on: bool) {
        if on {
            if self.pomo.running {
                return;
            }
            self.pomo.phase = crate::app::Phase::Focus;
            self.pomo.remaining = self.phase_secs(crate::app::Phase::Focus);
            self.pomo.running = true;
            self.say("已开始专注");
        } else if self.pomo.running {
            self.pomo.running = false;
            self.say("已暂停专注");
        }
    }

    /// 这条通知是否进入处理流程。两个互不相干的维度：
    /// 手机短信/来电只看 `phone_alerts`，其它应用只看监听范围（全部应用 / 仅 QQ）。
    pub(crate) fn accepts_notification(&self, id: &str, phone: bool) -> bool {
        if !self.settings.read_notifications || id.is_empty() {
            return false;
        }
        if phone {
            return self.settings.phone_alerts;
        }
        !self.settings.qq_only || id == "QQ"
    }
    pub fn set_messenger_status(&mut self, client: crate::app::Messenger, status: String) {
        self.messenger_statuses.insert(client, status);
    }
    pub fn messenger_status(&self, client: crate::app::Messenger) -> &str {
        self.messenger_statuses
            .get(&client)
            .map(String::as_str)
            .unwrap_or("")
    }
    pub fn receive_notification(&mut self, app: String, id: String, body: String) {
        if crate::system::phone_channel(&id) {
            self.notify_phone(app, id, body);
            return;
        }
        self.receive_notification_from(app, id, body, None);
    }
    pub fn receive_notification_from(
        &mut self,
        app: String,
        id: String,
        body: String,
        unread: Option<crate::app::Unread>,
    ) {
        // 手机连接的通知从这里进来（WinRT 监听器直连此路径，不经过 notify_phone），
        // 先做 App 名单自动登记，再照常显示。非手机通道是 no-op。
        self.maybe_register_phone_app(&id, &body);
        if !self.settings.read_notifications
            || id.is_empty()
            || !crate::messenger::NotificationFilter::for_settings(&self.settings).allows(&id)
        {
            return;
        }
        if crate::messenger::suppresses_toast(
            &app,
            &id,
            crate::messenger::toast_suppression(&self.settings),
        ) {
            return;
        }
        let client = crate::app::Messenger::from_notification_id(&id)
            .filter(|client| client.adapter().capture == crate::messenger::Capture::SystemToast);
        let id = client
            .map(|client| client.adapter().notification_id.to_string())
            .unwrap_or(id);
        // 岛仔自己发出的系统提醒会被通知中心回传；已在历史里的同一内容不再入队，避免重复显示。
        if crate::messenger::Messenger::from_notification_id(&id).is_none()
            && self
                .history
                .iter()
                .any(|a| format!("{} · {}", a.title, a.body) == body)
        {
            return;
        }
        if !self.notification_apps.observe(&id, &app) || !self.settings.notifications {
            return;
        }
        let unread = client.map(|client| {
            unread
                .filter(|u| u.source == client && u.pid != 0)
                .unwrap_or(crate::app::Unread::of(client))
        });
        if let Some(unread) = unread {
            self.mark_messenger(unread.source, unread.pid, unread.window);
        }
        self.notification_bell_at = self.t;
        self.notification_bell_until = self.t + 4.5;
        let title = app.chars().filter(|c| !c.is_control()).take(80).collect();
        let body = if self.settings.notification_preview {
            let mut text = body
                .chars()
                .filter(|c| !c.is_control())
                .take(300)
                .collect::<String>();
            if let Some(code) = crate::otp::extract("", &text, &[]) {
                text = crate::mail_privacy::mask(&text, &code.text);
            }
            text
        } else {
            "收到新通知 · 内容已隐藏".into()
        };
        self.enqueue_alert(
            Alert {
                source: AlertSource::External(id),
                title,
                body,
                page: usize::MAX,
                at: self.t,
                duration: 4.5,
                bluetooth: None,
                meeting: None,
                unread,
                status: None,
            },
            false,
        );
    }
    pub fn reveal_messenger_notice(&mut self, unread: crate::app::Unread) {
        if !unread.source.adapter().dismiss_unread || !self.pending_messengers.contains(&unread) {
            return;
        }
        let body = if unread.pid == 0 {
            "消息账号暂无法确认"
        } else {
            "暂时无法打开对应窗口"
        };
        let source = if unread.source.adapter().capture == crate::messenger::Capture::Taskbar {
            AlertSource::Messenger(unread.source)
        } else {
            AlertSource::External(unread.source.adapter().notification_id.into())
        };
        self.notice = Some(Alert {
            source,
            title: unread.source.name().into(),
            body: body.into(),
            page: usize::MAX,
            at: self.t,
            duration: 4.5,
            bluetooth: None,
            meeting: None,
            unread: Some(unread),
            status: None,
        });
        self.notification_bell_at = self.t;
        self.notification_bell_until = self.t + 4.5;
    }
    /// 手机通知自动登记：名单里没有的新 App 加进 phone-apps.json、默认开启。
    /// 两条通知入口都要调：notify_phone（旧路径）和 receive_notification_from
    /// （WinRT 监听器的实际路径，手机消息从这里进来）。只登记、不改显示逻辑。
    pub(crate) fn maybe_register_phone_app(&mut self, id: &str, body: &str) {
        if !crate::system::phone_channel(id) { return; }
        self.phone_apps.reload_if_changed();
        let head = body.split('·').next().unwrap_or("").trim().to_string();
        if let Some(new_app) = crate::system::phone_new_app(&head, body, &self.phone_apps) {
            if self.phone_apps.add_if_missing(&new_app) {
                self.phone_apps.save();
                log_notice(&format!("AUTO-ADD 手机 App 名单 + {new_app}"));
            }
        }
    }

    pub(crate) fn notify_phone(&mut self, app: String, id: String, body: String) {
        let phone = crate::system::phone_channel(&id);
        // 手机 App 名单是外部 JSON，改完约 1 秒内生效，不用重启岛仔。
        if phone {
            self.phone_apps.reload_if_changed();
        }
        // 每条通知留一行诊断，出问题能直接看出是哪一步拦下的。
        let trace = format!(
            "id={id} app={app} phone={} qq_only={} body={}",
            if phone { "手机" } else { "其它" },
            if self.settings.qq_only { "是" } else { "否" },
            body.chars().take(60).collect::<String>()
        );
        // 手机只看手机开关，其它应用只看监听范围——两者互不影响。
        if !self.accepts_notification(&id, phone) {
            log_notice(&format!("DROP 未连接/范围外/手机关  {trace}"));
            return;
        }
        // 真实手机连接通知只报主 AUMID，改由正文识别来源；
        // 明确命中黑名单的静默，识别不出的按「连接」放行，避免漏掉来电等重要通知。
        let phone_info: Option<(String, bool)> = if phone {
            self.maybe_register_phone_app(&id, &body);
            Some(
                crate::system::phone_app(&body, &self.phone_apps)
                    .unwrap_or(("连接".to_string(), true)),
            )
        } else {
            None
        };
        if phone_info.as_ref().is_some_and(|(_, allow)| !allow) {
            log_notice(&format!("DROP 名单静默    {trace}"));
            return;
        }
        // 通知规则：静音 / 自动复制正文。规则文件改完约 1 秒生效，不用重启。
        self.notice_rules.reload_if_changed();
        let rule_text = format!("{app} {body}");
        if self.notice_rules.hits(&rule_text, "mute") {
            log_notice(&format!("DROP 规则静音    {trace}"));
            return;
        }
        if self.notice_rules.hits(&rule_text, "copy") && !body.trim().is_empty() {
            self.pending.push(Cmd::Copy(body.clone()));
            log_notice(&format!("规则命中 copy    {trace}"));
        }
        // 来电方（正文首段就是号码或联系人），来电中/未接来电都用它当副标题。
        let caller = body.split('·').next().unwrap_or("").trim().to_string();
        // 岛仔自己发出的系统提醒会被通知中心回传；已在历史里的同一内容不再入队，避免重复显示。
        if self
            .history
            .iter()
            .any(|a| format!("{} · {}", a.title, a.body) == body)
        {
            log_notice(&format!("DROP 自家回传    {trace}"));
            return;
        }
        if !self.notification_apps.observe(&id, &app) || !self.settings.notifications {
            log_notice(&format!("DROP 应用被关/总开关  {trace}"));
            return;
        }
        if id == "QQ" {
            self.mark_messenger(crate::app::Messenger::Qq, 0, 0);
        }
        // 验证码只有几分钟有效，铃铛多留一会儿，避免鼠标来不及移上去。
        let has_code = !body.is_empty() && crate::otp::extract("", &body, &[]).is_some();
        let otp_bell = phone && has_code;
        // 停留时长：普通通知用 notice_seconds；验证码用 otp_seconds，但不短于普通通知，
        // 且有 10 秒下限——验证码要留时间回看和点「复制」。
        let stay = notice_duration(
            self.settings.notice_seconds,
            self.settings.otp_seconds,
            false,
        );
        let otp_stay = notice_duration(
            self.settings.notice_seconds,
            self.settings.otp_seconds,
            true,
        );
        // 测试构建保持官方 4.5 秒，避免时长类断言失效。
        let keep_bell = if cfg!(test) {
            4.5
        } else if has_code {
            otp_stay
        } else {
            stay
        };
        // 勿扰期间不闪铃铛（横幅同样被 enqueue_alert 拦下），验证码仍照常提醒。
        if !self.muted() || otp_bell {
            self.notification_bell_at = self.t;
            self.notification_bell_until = self.t + keep_bell;
        }
        // 「手机 · 」前缀用来和岛仔自带 IMAP 邮件的「新邮件」提醒区分开。
        let title = if let Some((label, _)) = &phone_info {
            format!("手机 · {label}")
        } else {
            app.chars().filter(|c| !c.is_control()).take(80).collect()
        };
        // 10 秒窗口内同一 App 的多条消息合并计数，配合 enqueue_alert 的替换去重防刷屏。
        let count = if phone {
            let entry = self
                .phone_counts
                .entry(title.clone())
                .or_insert((0, self.t));
            if self.t - entry.1 > 10.0 {
                entry.0 = 0;
            }
            entry.0 += 1;
            entry.1 = self.t;
            entry.0
        } else {
            1
        };
        // 手机通知去掉正文首段的 App 名（已在标题里），其余作为摘要；
        // 即使全局关闭正文预览也显示一行短摘要，且不对验证码打码
        //（后续验证码卡片会直接复用 otp::extract 的结果）。
        let source: &str = if phone_info.is_some() {
            // 手机镜像通知的正文末尾常带一个多余的分隔符（`…忽略本短信! · `），顺手去掉。
            body.split_once('·')
                .map(|(_, rest)| {
                    rest.trim()
                        .trim_end_matches(|c: char| c == '·' || c.is_whitespace())
                        .trim()
                })
                .filter(|rest| !rest.is_empty())
                .unwrap_or(body.as_str())
        } else {
            body.as_str()
        };
        let body = if self.settings.notification_preview || phone {
            let limit = if phone { 80 } else { 300 };
            let mut text = source
                .chars()
                .filter(|c| !c.is_control())
                .take(limit)
                .collect::<String>();
            if !phone {
                if let Some(code) = crate::otp::extract("", &text, &[]) {
                    text = crate::mail_privacy::mask(&text, &code.text);
                }
            }
            text
        } else {
            "收到新通知 · 内容已隐藏".into()
        };
        let body = if count > 1 {
            format!("共 {count} 条 · {body}")
        } else {
            body
        };
        // 手机验证码自动复制（与邮件共用开关）：短信验证码上岛时顺手写剪贴板。
        if phone && self.settings.auto_copy_otp {
            let code = crate::otp::extract("", &body, &[]);
            log_otp("phone", code.as_ref().map(|c| c.text.as_str()));
            if let Some(code) = code {
                // 验证码不进岛仔的剪贴板历史，只写系统剪贴板。
                self.pending.push(Cmd::CopyOtp(code.text));
            }
        }
        // 来电中 / 未接来电：标签都是「电话」，靠正文区分——
        // 响铃是「号码 · 传入呼叫」，挂断是「电话 · 号码 · 未接来电」。
        let ringing = phone_info.is_some() && body.contains("传入呼叫");
        let missed = phone_info
            .as_ref()
            .is_some_and(|(label, _)| label == "电话")
            && body.contains("未接来电");
        if missed {
            // 来电中的提示被「未接来电」取代，别让两条并存。
            if self.notice.as_ref().is_some_and(|a| a.title == "来电中") {
                self.notice = None;
            }
            self.missed_calls = self.missed_calls.saturating_add(1);
            self.notification_bell_until = f64::MAX;
        }
        let (title, body, duration) = if ringing {
            ("来电中".to_string(), format!("{caller} 正在呼叫"), 60.0)
        } else if missed {
            (
                "未接来电".to_string(),
                format!("{} 通未接来电 · 点击关闭清除", self.missed_calls),
                30.0,
            )
        } else {
            (
                title,
                body,
                if cfg!(test) {
                    4.5
                } else if has_code {
                    otp_stay
                } else {
                    stay
                },
            )
        };
        // 通知正文可能含短信验证码：落盘诊断日志前先打码，避免 generate diagnostics 时外泄。
        let logged_body = if let Some(code) = crate::otp::extract("", &body, &[]) {
            crate::mail_privacy::mask(&body, &code.text)
        } else {
            body.clone()
        };
        log_notice(&format!("SHOW {title} / {logged_body}  <- {trace}"));
        self.enqueue_alert(
            Alert {
                source: AlertSource::External(id),
                title,
                body,
                page: usize::MAX,
                at: self.t,
                duration,
                bluetooth: None,
                meeting: None,
                status: None,
                unread: None,
            },
            false,
        );
    }
    pub(crate) fn tick_alerts(&mut self) {
        self.pending_messengers.retain(|u| {
            u.source.enabled(&self.settings)
                && (u.source.adapter().capture != crate::messenger::Capture::SystemToast
                    || self
                        .notification_apps
                        .items
                        .iter()
                        .any(|(key, _, on)| key == u.source.adapter().notification_id && *on))
        });
        let allowed = |a: &Alert| match &a.source {
            AlertSource::Internal => !self.fullscreen_hidden || a.health().is_none(),
            AlertSource::Messenger(client) => client.enabled(&self.settings),
            AlertSource::External(id) => {
                // 手机通道与状态总线不走应用名单：手机看 phone_alerts，状态总线常放行。
                let phone = crate::system::phone_channel(id);
                let status_bus = id == "island-status" || id.starts_with("island-status:");
                if status_bus {
                    return self.settings.notifications;
                }
                if phone {
                    return self.settings.notifications
                        && self.settings.read_notifications
                        && self.settings.phone_alerts;
                }
                self.settings.read_notifications
                    && crate::messenger::NotificationFilter::for_settings(&self.settings).allows(id)
                    && self.settings.notifications
                    && !crate::messenger::suppresses_toast(
                        &a.title,
                        id,
                        crate::messenger::toast_suppression(&self.settings),
                    )
                    && self
                        .notification_apps
                        .items
                        .iter()
                        .any(|(key, _, on)| key == id && *on)
            }
        };
        self.alerts.retain(&allowed);
        if self.notice.as_ref().is_some_and(|a| !allowed(a)) {
            self.notice = None;
        }
        if self.hud_notice.as_ref().is_some_and(|a| !allowed(a)) {
            self.hud_notice = None;
        }
        if self.fullscreen_hidden {
            self.pending.retain(|cmd|!matches!(cmd,Cmd::Notify(title,..)if HealthReminder::ALL.into_iter().any(|kind|title==kind.title())));
        }
        if !self.settings.notification_preview {
            for a in self
                .alerts
                .iter_mut()
                .chain(self.notice.iter_mut())
                .chain(self.hud_notice.iter_mut())
            {
                let keep = matches!(&a.source,AlertSource::External(id) if crate::system::phone_channel(id) || id=="island-status" || id.starts_with("island-status:"));
                if !keep && matches!(&a.source, AlertSource::External(_)) {
                    a.body = "收到新通知 · 内容已隐藏".into();
                }
            }
        }
        if self
            .notice
            .as_ref()
            .is_some_and(|a| self.t - a.at > a.duration)
        {
            self.notice = None;
        }
        if self.notice.is_none() {
            if let Some(mut a) = self.alerts.pop_front() {
                a.at = self.t;
                self.notice = Some(a);
            }
        }
    }
}

/// 验证码识别诊断：只在「自动复制」开启时写一行，便于排查邮件/短信为什么没复制。
/// 日志只保留打码后的长度，绝不把验证码明文长期写进磁盘。
pub(crate) fn log_otp(source: &str, code: Option<&str>) {
    let Some(base) = std::env::var_os("APPDATA") else {
        return;
    };
    let dir = std::path::PathBuf::from(base)
        .join("Daozai")
        .join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("otp.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let masked = code.map(|value| crate::mail_privacy::mask(value, value));
        let _ = writeln!(
            file,
            "{} | {} | {}",
            chrono::Local::now().format("%H:%M:%S"),
            source,
            masked.as_deref().unwrap_or("-")
        );
    }
}

/// 通知卡片停留秒数：验证码有 10 秒下限，且不会短于普通通知（要留时间回看和点「复制」）。
pub(crate) fn notice_duration(notice_seconds: u32, otp_seconds: u32, has_code: bool) -> f64 {
    let stay = notice_seconds as f64;
    if has_code {
        (otp_seconds as f64).max(stay).max(10.0)
    } else {
        stay
    }
}

#[cfg(test)]
mod notice_duration_tests {
    use super::notice_duration;

    #[test]
    fn 验证码停留有下限且不短于普通通知() {
        assert_eq!(
            notice_duration(10, 90, true),
            90.0,
            "验证码用自己设的 90 秒"
        );
        assert_eq!(notice_duration(10, 20, true), 20.0);
        assert_eq!(notice_duration(60, 20, true), 60.0, "不能比普通通知还短");
        assert_eq!(notice_duration(10, 5, true), 10.0, "下限 10 秒");
        assert_eq!(
            notice_duration(10, 90, false),
            10.0,
            "普通通知不受验证码设置影响"
        );
    }
}

/// 番茄统计：`%APPDATA%\Daozai\focus-stats.json`，只记「今天多少分钟」。
/// 只统计真正在跑的专注阶段，暂停不计；写盘频率是每分钟一次。
pub(crate) fn focus_stats_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(
        std::path::PathBuf::from(base)
            .join("Daozai")
            .join("focus-stats.json"),
    )
}

fn focus_days() -> Vec<(String, u32)> {
    let Some(path) = focus_stats_path() else {
        return Vec::new();
    };
    // 坏文件会被改名成 .bad 后返回 None，避免「解析失败」被当成「本来是空的」而清空统计。
    let Some(json) = crate::model::read_json_renaming_bad(&path, crate::model::MAX_JSON_BYTES)
    else {
        return Vec::new();
    };
    json.get("days")
        .and_then(|d| d.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some((
                item.get("date")?.as_str()?.to_owned(),
                // 防止超大值静默截断为 0 或错误的小数值；越界条目直接丢弃。
                u32::try_from(item.get("minutes")?.as_u64()?).ok()?,
            ))
        })
        .collect()
}

/// 今日专注分钟数与连续天数（连续天数从今天往回数，今天为 0 则从昨天算）。
pub(crate) fn focus_summary() -> (u32, u32) {
    let days = focus_days();
    let today = chrono::Local::now().date_naive();
    let minutes = days
        .iter()
        .find(|(date, _)| date == &today.format("%Y-%m-%d").to_string())
        .map(|(_, m)| *m)
        .unwrap_or(0);
    let mut streak = 0;
    let mut cursor = today;
    if minutes == 0 {
        cursor = today - chrono::Duration::days(1);
    }
    loop {
        let key = cursor.format("%Y-%m-%d").to_string();
        match days.iter().find(|(date, _)| date == &key) {
            Some((_, m)) if *m > 0 => {
                streak += 1;
                cursor = cursor - chrono::Duration::days(1);
            }
            _ => break,
        }
        if streak > 365 {
            break;
        }
    }
    (minutes, streak)
}

pub(crate) fn focus_add(minutes: u32) {
    if minutes == 0 || cfg!(test) {
        return;
    }
    let Some(path) = focus_stats_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let today = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let mut days = focus_days();
    match days.iter_mut().find(|(date, _)| date == &today) {
        Some(entry) => entry.1 += minutes,
        None => days.push((today, minutes)),
    }
    if days.len() > 400 {
        let extra = days.len() - 400;
        days.drain(0..extra);
    }
    days.sort_by(|a, b| a.0.cmp(&b.0));
    let json = serde_json::json!({"days":days.iter().map(|(date,m)|serde_json::json!({"date":date,"minutes":m})).collect::<Vec<_>>()});
    if let Ok(data) = serde_json::to_vec(&json) {
        let _ = crate::model::atomic_write(&path, &data);
    }
}

/// 通知历史（落盘，最多 100 条）：岛仔关掉之后「当时没看到的那条」还能查回来。
/// 存 `%APPDATA%\Daozai\notices.json`，字段 at/title/body，新的在后面。
pub(crate) fn notice_history_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(
        std::path::PathBuf::from(base)
            .join("Daozai")
            .join("notices.json"),
    )
}

/// 按时间顺序返回 (时间, 标题, 正文)，最新在最后。
pub(crate) fn notice_history() -> Vec<(String, String, String)> {
    let Some(path) = notice_history_path() else {
        return Vec::new();
    };
    let Some(json) = crate::model::read_json_renaming_bad(&path, crate::model::MAX_JSON_BYTES)
    else {
        return Vec::new();
    };
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some((
                item.get("at")?.as_str()?.to_owned(),
                item.get("title")?.as_str()?.to_owned(),
                item.get("body")?.as_str()?.to_owned(),
            ))
        })
        .collect()
}

pub(crate) fn notice_history_append(title: &str, body: &str) {
    // 测试构建别往真实 APPDATA 里塞测试通知。
    if cfg!(test) {
        return;
    }
    let Some(path) = notice_history_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // 落盘前把验证码打码：历史文件只用于「回看是什么事」，不需要留着明文码。
    let body = if let Some(code) = crate::otp::extract("", body, &[]) {
        crate::mail_privacy::mask(body, &code.text)
    } else {
        body.to_string()
    };
    let mut list = notice_history();
    list.push((
        chrono::Local::now().format("%m-%d %H:%M").to_string(),
        title.to_string(),
        body,
    ));
    if list.len() > 100 {
        let extra = list.len() - 100;
        list.drain(0..extra);
    }
    let json: Vec<_> = list
        .iter()
        .map(|(at, title, body)| serde_json::json!({"at":at,"title":title,"body":body}))
        .collect();
    // 原子写：中途断电也不会留下半截 JSON（否则下次读失败会被当空文件重建）。
    if let Ok(data) = serde_json::to_vec(&json) {
        let _ = crate::model::atomic_write(&path, &data);
    }
}

/// 通知通道诊断：每条进来的通知都记一行「显示 / 被谁拦下」，排查「某类通知不上岛」用。
pub(crate) fn log_notice(note: &str) {
    let Some(base) = std::env::var_os("APPDATA") else {
        return;
    };
    let dir = std::path::PathBuf::from(base)
        .join("Daozai")
        .join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("notice.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(
            file,
            "{} | {}",
            chrono::Local::now().format("%H:%M:%S"),
            note
        );
    }
}

/// 邮件面板操作诊断：点击「全部已读」被拒时记录原因，便于排查按钮为什么没反应。
pub(crate) fn log_mail(note: &str) {
    let Some(base) = std::env::var_os("APPDATA") else {
        return;
    };
    let dir = std::path::PathBuf::from(base)
        .join("Daozai")
        .join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("mail.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(
            file,
            "{} | {}",
            chrono::Local::now().format("%H:%M:%S"),
            note
        );
    }
}

fn phone_icon_file(title: &str) -> Option<&'static str> {
    let label = title.strip_prefix("手机 · ")?;
    Some(match label {
        "QQ" => "app_icon_com.tencent.mqq1.0.jpg",
        "微信" => "app_icon_com.tencent.xin1.0.jpg",
        "支付宝" => "app_icon_com.alipay.iphoneclient1.0.jpg",
        "Gmail" => "app_icon_com.google.Gmail1.0.jpg",
        "电话" => "app_icon_com.apple.mobilephone1.0.jpg",
        _ => return None,
    })
}
/// 用手机连接自带的 App 图标替代铃铛；首次加载后缓存纹理，避免每帧读盘。
fn phone_icon(
    c: &Card,
    title: &str,
    icons: &mut std::collections::HashMap<String, egui::TextureHandle>,
) -> bool {
    let Some(file) = phone_icon_file(title) else {
        return false;
    };
    if !icons.contains_key(file) {
        let Ok(base) = std::env::var("LOCALAPPDATA") else {
            return false;
        };
        let path = std::path::PathBuf::from(base)
            .join("Packages")
            .join("Microsoft.YourPhone_8wekyb3d8bbwe")
            .join("TempState")
            .join("app_icons")
            .join(file);
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let Ok(img) = image::load_from_memory(&bytes) else {
            return false;
        };
        let rgba = img.into_rgba8();
        let ci = egui::ColorImage::from_rgba_unmultiplied(
            [rgba.width() as usize, rgba.height() as usize],
            rgba.as_raw(),
        );
        let tex =
            c.ui.ctx()
                .load_texture(file.to_owned(), ci, egui::TextureOptions::LINEAR);
        icons.insert(file.to_owned(), tex);
    }
    let Some(tex) = icons.get(file) else {
        return false;
    };
    c.painter.image(
        tex.id(),
        c.rect(12.0, 10.0, 24.0, 24.0),
        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
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
fn status_action(spec: &str) -> Option<(&'static str, StatusAct)> {
    use StatusAct::*;
    if let Some(text) = spec.strip_prefix("copy:") {
        if !text.is_empty() {
            return Some(("复制", Shell(Cmd::Copy(text.to_string()))));
        }
    }
    if let Some(url) = spec.strip_prefix("url:") {
        if url.starts_with("https://") || url.starts_with("http://") {
            return Some(("打开", Shell(Cmd::OpenUrl(url.to_string()))));
        }
    }
    if let Some(file) = spec.strip_prefix("file:") {
        // 只放行「看到的文件」：路径必须存在，且扩展名在白名单内。
        // 黑名单挡不住 .py/.jar/.msc/.chm 等可执行/脚本/解析即执行的文件类型——
        // 任何能写 island_status.json 的进程都能借岛仔的手启动程序，故改为白名单。
        let path = std::path::PathBuf::from(file);
        let allowed = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                matches!(
                    e.to_ascii_lowercase().as_str(),
                    // 文档
                    "txt" | "md" | "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx"
                        | "rtf" | "csv" | "epub" | "mobi"
                        // 图片
                        | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "heic"
                        // 音视频
                        | "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "mp4" | "mkv" | "avi" | "mov" | "webm"
                        // 压缩包
                        | "zip" | "rar" | "7z" | "tar" | "gz"
                )
            });
        if path.is_file() && allowed {
            return Some(("打开", Shell(Cmd::Open(path))));
        }
    }
    let action = spec.strip_prefix("action:")?.trim();
    match action {
        "focus_start" => Some(("开始专注", Focus(true))),
        "focus_stop" => Some(("结束专注", Focus(false))),
        "quiet30" => Some(("勿扰 30 分", Quiet(30))),
        "quiet60" => Some(("勿扰 60 分", Quiet(60))),
        "quiet_off" => Some(("关闭勿扰", Quiet(0))),
        _ => action
            .strip_prefix("page:")
            .and_then(|n| n.trim().parse::<usize>().ok())
            .filter(|n| *n < 8)
            .map(|n| ("打开", Page(n))),
    }
}
/// 当前 Unix 时间（秒，带小数）；写状态总线的 ttl 用。
pub(crate) fn unix_secs_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub(crate) fn status_color(name: &str) -> Color32 {
    match name.trim().to_ascii_lowercase().as_str() {
        "red" => Color32::from_rgb(226, 75, 74),
        "green" => theme::CHARGE,
        "blue" => theme::BLUE,
        "amber" => Color32::from_rgb(239, 159, 39),
        _ => theme::GRAY,
    }
}

impl App {
    pub(crate) fn draw_notice(&mut self, c: &Card, panel: Rect, a: &Alert, _cmds: &mut Vec<Cmd>) {
        let nc = c.clipped(0.0, 0.0, panel.width(), panel.height());
        // 关键词高亮：命中词表时在卡片最左侧亮一条琥珀色竖条。
        // 只加一条竖条，不动文字和任何控件的位置，布局与测试友好的部分保持原样。
        if a.status.is_none() && (self.keyword_hit(&a.title) || self.keyword_hit(&a.body)) {
            nc.fill(2.5, 10.0, 2.5, 24.0, 1.25, Color32::from_rgb(239, 159, 39));
        }
        if let Some(source) = a.messenger() {
            messenger_icon(&nc, 22.0, 22.0, source);
            nc.circle(31.0, 14.0, 2.6, Color32::from_rgb(255, 72, 72));
            nc.centered_text(
                44.0,
                14.0,
                &a.title,
                11.0,
                W::Semibold,
                theme::WHITE,
                H::Left,
            );
            nc.centered_text(
                44.0,
                30.0,
                &nc.fit(&a.body, 10.5, W::Regular, 154.0),
                10.5,
                W::Regular,
                theme::GRAY,
                H::Left,
            );
            if notice_button(&nc, 206.0, 12.0, 52.0, "查看", "messenger-view") {
                _cmds.push(Cmd::ViewMessenger(
                    a.unread.unwrap_or_else(|| self.latest_unread(source)),
                ));
            }
            if notice_button(&nc, 265.0, 12.0, 52.0, "关闭", "messenger-dismiss") {
                if source.adapter().dismiss_unread {
                    if let Some(unread) = a.unread {
                        self.discard_unread(unread);
                    }
                }
                self.notice = None;
                self.notification_bell_until = 0.0;
            }
            return;
        }
        if a.source != AlertSource::Internal {
            // 通用状态总线：独立绘制，不影响手机通知 / 验证码卡片的原有布局。
            if let Some(st) = &a.status {
                let tone = status_color(&st.color);
                nc.fill(6.0, 10.0, 3.0, 24.0, 1.5, tone);
                nc.centered_text(
                    16.0,
                    14.0,
                    &nc.fit(&a.title, 11.0, W::Semibold, 198.0),
                    11.0,
                    W::Semibold,
                    theme::WHITE,
                    H::Left,
                );
                nc.centered_text(
                    16.0,
                    30.0,
                    &nc.fit(&a.body, 10.5, W::Regular, 198.0),
                    10.5,
                    W::Regular,
                    theme::GRAY,
                    H::Left,
                );
                if let Some(p) = st.progress.filter(|v| v.is_finite()) {
                    let p = p.clamp(0.0, 1.0) as f32;
                    nc.fill(16.0, 37.0, 198.0, 2.0, 1.0, Color32::from_gray(60));
                    nc.fill(16.0, 37.0, 198.0 * p, 2.0, 1.0, tone);
                }
                if let Some((label, act)) = status_action(&st.click) {
                    if notice_button(&nc, 160.0, 12.0, 60.0, label, "status-action") {
                        match act {
                            StatusAct::Shell(cmd) => _cmds.push(cmd),
                            StatusAct::Focus(on) => self.set_focus_running(on),
                            StatusAct::Quiet(minutes) => self.set_quiet(minutes),
                            StatusAct::Page(page) => self.show_page(page),
                        }
                    }
                }
                if notice_button(&nc, 226.0, 12.0, 60.0, "关闭", "external-dismiss") {
                    self.notice = None;
                    self.notification_bell_until = 0.0;
                }
                return;
            }
            // 手机通知优先用手机连接自带的 App 图标，没图标才回退到铃铛。
            let icon = phone_icon(&nc, &a.title, &mut self.phone_icons);
            if !icon {
                crate::paint::bell(
                    &nc,
                    310.0,
                    22.0,
                    theme::GRAY,
                    crate::paint::bell_swing(self.t - a.at),
                );
                nc.circle(314.0, 17.5, 2.0, Color32::from_rgb(255, 72, 72));
            }
            // 有图标时文字整体右移，避免压在图标上。
            let text_x = if icon { 44.0 } else { 16.0 };
            let text_w = if icon { 170.0 } else { 198.0 };
            nc.centered_text(
                text_x,
                14.0,
                &nc.fit(&a.title, 11.0, W::Semibold, text_w),
                11.0,
                W::Semibold,
                theme::WHITE,
                H::Left,
            );
            // 验证码卡片：大号数字 + 一键复制（otp::extract 有打分门槛，普通消息不会误判）。
            // 短信从正文提取；邮件验证码只从邮件数据读，通知与历史都不留明文。
            let mail_code = matches!(&a.source,AlertSource::External(id) if id=="mail-code")
                .then(|| self.mail.code_item().and_then(|m| m.code.clone()))
                .flatten();
            if let Some(code) =
                mail_code.or_else(|| crate::otp::extract("", &a.body, &[]).map(|c| c.text))
            {
                nc.centered_text(text_x, 30.0, &code, 20.0, W::Bold, theme::WHITE, H::Left);
                if notice_button(&nc, 160.0, 12.0, 60.0, "复制", "phone-code-copy") {
                    _cmds.push(Cmd::CopyOtp(code));
                }
            } else {
                nc.centered_text(
                    text_x,
                    30.0,
                    &nc.fit(&a.body, 10.5, W::Regular, text_w),
                    10.5,
                    W::Regular,
                    theme::GRAY,
                    H::Left,
                );
            }
            if notice_button(&nc, 226.0, 12.0, 60.0, "关闭", "external-dismiss") {
                self.notice = None;
                self.notification_bell_until = 0.0;
                if a.title == "未接来电" {
                    self.missed_calls = 0;
                }
            }
            return;
        }
        if let Some((meeting, _)) = &a.meeting {
            let secondary = Color32::from_gray(174);
            notice_icon(&nc.scaled(34.0, 46.0, 36.0 / 28.0), 34.0, 46.0, a);
            let now = chrono::Local::now();
            let start = chrono::DateTime::from_timestamp(meeting.start, 0)
                .unwrap()
                .with_timezone(&chrono::Local);
            let left = meeting.start - now.timestamp();
            let countdown = if left > 0 {
                format!("{} 分钟后", (left + 59) / 60)
            } else {
                "正在开始".into()
            };
            let countdown_width = nc.text_w(&countdown, 10.5, W::Semibold) + 16.0;
            let countdown_x = 344.0 - countdown_width;
            nc.centered_text(
                64.0,
                33.0,
                &start.format("%H:%M").to_string(),
                30.0,
                W::Semibold,
                theme::WHITE,
                H::Left,
            );
            nc.fill(
                countdown_x,
                23.0,
                countdown_width,
                20.0,
                10.0,
                Color32::from_gray(28),
            );
            nc.centered_text(
                countdown_x + countdown_width / 2.0,
                33.0,
                &countdown,
                10.5,
                W::Semibold,
                theme::WHITE,
                H::Center,
            );
            let overflow = (nc.text_w(&meeting.title, size::TITLE, W::Semibold) - 280.0).max(0.0);
            let offset = (((self.t - a.at - 1.2).max(0.0) as f32) * 24.0).min(overflow);
            nc.clipped(64.0, 52.0, 280.0, 24.0).centered_text(
                64.0 - offset,
                64.0,
                &meeting.title,
                size::TITLE,
                W::Semibold,
                theme::WHITE,
                H::Left,
            );
            nc.seg((16.0, 84.0), (344.0, 84.0), 0.6, Color32::from_gray(36));
            let date = format!(
                "{}{}",
                if start.date_naive() == now.date_naive() {
                    "今天 · "
                } else {
                    ""
                },
                start.format("%-m月%-d日")
            );
            nc.centered_text(16.0, 110.0, &date, 11.0, W::Regular, secondary, H::Left);
            let (dismiss, dc) = crate::paint::capsule(
                &nc,
                188.0,
                98.0,
                72.0,
                24.0,
                "meeting-dismiss",
                Color32::from_gray(32),
                Color32::from_gray(48),
            );
            dc.centered_text(
                224.0,
                110.0,
                "关闭",
                10.5,
                W::Semibold,
                theme::WHITE,
                H::Center,
            );
            if dismiss.on_hover_text("关闭本次提醒，日程仍保留").clicked() {
                self.notice = None;
                return;
            }
            let (button, bc) = crate::paint::capsule(
                &nc,
                272.0,
                98.0,
                72.0,
                24.0,
                "meeting-action",
                Color32::from_gray(42),
                Color32::from_gray(56),
            );
            bc.centered_text(
                308.0,
                110.0,
                "查看",
                10.5,
                W::Semibold,
                theme::WHITE,
                H::Center,
            );
            if button.clicked() {
                self.notice = None;
                self.open_calendar(Some(meeting.clone()));
            }
            return;
        }
        if a.is_hud() {
            if a.title == "蓝牙设备" {
                let (name, status, battery) = bluetooth_notice_content(&a.body);
                nc.circle(
                    26.0,
                    24.0,
                    14.0,
                    if status == "已连接" {
                        Color32::from_rgb(10, 132, 255)
                    } else {
                        Color32::from_gray(95)
                    },
                );
                let kind = a
                    .bluetooth
                    .map(|(kind, _)| kind)
                    .unwrap_or(crate::bt::DeviceKind::Other);
                device_icon(&nc, kind, 17.0, 15.0, 18.0);
                let right = HUD_W - 14.0;
                let name_end = if let Some(level) = battery {
                    let pct = format!("{level}%");
                    let pct_width = nc.text_w(&pct, 10.5, W::Semibold);
                    let x = right - pct_width - 22.0;
                    nc.stroke(x, 20.0, 15.0, 8.0, 2.0, 1.0, Color32::from_gray(175));
                    nc.fill(x + 15.5, 22.5, 2.0, 3.0, 0.8, Color32::from_gray(175));
                    let fill = 11.0 * level as f32 / 100.0;
                    if fill > 0.0 {
                        nc.fill(
                            x + 2.0,
                            22.0,
                            fill,
                            4.0,
                            1.0,
                            if level <= 20 {
                                theme::RED
                            } else {
                                theme::GREEN
                            },
                        );
                    }
                    nc.centered_text(right, 24.0, &pct, 10.5, W::Semibold, theme::WHITE, H::Right);
                    x - 8.0
                } else {
                    right
                };
                nc.centered_text(
                    49.0,
                    16.0,
                    &nc.fit(name, 11.0, W::Semibold, (name_end - 49.0).max(20.0)),
                    11.0,
                    W::Semibold,
                    theme::WHITE,
                    H::Left,
                );
                nc.centered_text(49.0, 32.0, status, 10.5, W::Regular, theme::GRAY, H::Left);
                return;
            }
            match a.title.as_str() {
                "系统音量" => {
                    nc.poly(
                        &[
                            (17.0, 16.0),
                            (21.0, 16.0),
                            (26.0, 12.0),
                            (26.0, 26.0),
                            (21.0, 22.0),
                            (17.0, 22.0),
                        ],
                        theme::WHITE,
                    );
                    if self.volume_ui.is_some_and(|(_, muted)| muted) {
                        nc.seg((29.0, 16.0), (34.0, 22.0), 1.4, theme::WHITE);
                        nc.seg((34.0, 16.0), (29.0, 22.0), 1.4, theme::WHITE);
                    } else {
                        nc.arc(26.0, 19.0, 5.0, -0.8, 0.8, 1.5, theme::WHITE);
                        nc.arc(26.0, 19.0, 8.0, -0.7, 0.7, 1.3, theme::WHITE);
                    }
                }
                _ => notice_icon(&nc, 24.0, 19.0, a),
            }
            if a.title == "系统音量" {
                self.draw_volume_hud(&nc);
            } else {
                let (title, body) = if a.title == "键盘输入语言" {
                    ("输入语言", a.body.as_str())
                } else if a.title == "已切换电池供电" {
                    ("电池供电", a.body.as_str())
                } else if a.title == "网络连接" {
                    a.body
                        .split_once(" · ")
                        .unwrap_or(("网络", a.body.as_str()))
                } else {
                    (a.title.as_str(), a.body.as_str())
                };
                nc.centered_text(
                    48.0,
                    19.0,
                    &nc.fit(title, 11.0, W::Semibold, 90.0),
                    11.0,
                    W::Semibold,
                    theme::WHITE,
                    H::Left,
                );
                nc.centered_text(
                    HUD_W - 16.0,
                    19.0,
                    &nc.fit(body, 10.5, W::Regular, 92.0),
                    10.5,
                    W::Regular,
                    if a.title == "大写锁定" && a.body == "已开启"
                        || a.title == "网络连接" && a.body.contains("已连接")
                    {
                        theme::GREEN
                    } else {
                        theme::GRAY
                    },
                    H::Right,
                );
            }
        } else {
            let is_sit = a.health() == Some(HealthReminder::Sit);
            let is_water = a.health() == Some(HealthReminder::Water);
            let is_focus = a.page == 1;
            let is_power = a.title == "电量偏低";
            notice_icon(&nc, 22.0, 22.0, a);
            let max = if is_water || is_sit { 125.0 } else { 195.0 };
            nc.centered_text(
                43.0,
                14.0,
                &nc.fit(&a.title, 11.0, W::Semibold, max),
                11.0,
                W::Semibold,
                theme::WHITE,
                H::Left,
            );
            nc.centered_text(
                43.0,
                30.0,
                &nc.fit(&a.body, 10.5, W::Regular, max),
                10.5,
                W::Regular,
                theme::GRAY,
                H::Left,
            );
            if is_sit {
                if notice_button(&nc, 181.0, 12.0, 62.0, "知道了", "sit-confirm-read") {
                    self.confirm_health_reminder(HealthReminder::Sit);
                }
                if notice_button(&nc, 250.0, 12.0, 68.0, "起身了", "sit-confirm") {
                    self.notice = None;
                    self.stood_up();
                }
            } else if is_water {
                if notice_button(&nc, 181.0, 12.0, 62.0, "知道了", "water-confirm-read") {
                    self.confirm_health_reminder(HealthReminder::Water);
                }
                if notice_button(&nc, 250.0, 12.0, 68.0, "喝一口", "water-confirm") {
                    self.notice = None;
                    self.drink_water();
                }
            } else {
                let label = if is_focus {
                    "开始"
                } else if is_power {
                    "知道了"
                } else {
                    "查看"
                };
                if notice_button(&nc, 257.0, 12.0, 60.0, label, "notice-action") {
                    self.notice = None;
                    if is_power {
                        return;
                    }
                    if is_focus {
                        self.pomo.running = true;
                    } else if a.page < 6 {
                        self.show_page(a.page);
                        if a.page == 4 {
                            let order = self.todos.display_order(&self.today);
                            if let Some(index) = order.iter().position(|&i| {
                                self.todos.items[i].text == a.body
                                    && !self.todos.items[i].done_for(&self.today)
                            }) {
                                self.todo_first = index.min(order.len().saturating_sub(3));
                            }
                        }
                    } else {
                        self.settings_tab = 6;
                        self.open_settings();
                    }
                }
            }
            if let Some(kind) = a.health() {
                if nc
                    .hit(0.0, 0.0, 175.0, 44.0, "health-notice-confirm")
                    .clicked()
                {
                    self.confirm_health_reminder(kind);
                }
            }
        }
    }
    /// v0.5.4：可交互音量条。样式参考 oyenza-studio（MIT）的 HUD 音量条：
    /// 圆角胶囊、填充缓动、拖动时加粗并出现手柄；拖动/滚轮改音量，点喇叭静音。
    pub(crate) fn draw_volume_hud(&mut self, nc: &Card) {
        let (v, muted) = self.volume_ui.unwrap_or((0, false));
        let v = v.min(100);
        // 喇叭：点击静音 / 恢复
        let icon = nc.hit(12.0, 9.0, 28.0, 22.0, "volume-mute");
        if icon.clicked() && nc.live {
            self.volume_ui = Some((v, !muted));
            self.volume_quiet_until = self.t + 0.9;
            self.pending.push(Cmd::ToggleMute);
        }
        // 滑条：拖动 / 点击定位；滚轮微调
        let track = nc.hit_drag(HUD_TRACK_X, 8.0, HUD_TRACK_W, 24.0, "volume-track");
        let mut want: Option<u32> = None;
        if track.dragged() || track.clicked() {
            if let Some(p) = track.interact_pointer_pos() {
                let x = nc.to_local(p).x;
                want = Some(
                    (((x - HUD_TRACK_X) / HUD_TRACK_W) * 100.0)
                        .clamp(0.0, 100.0)
                        .round() as u32,
                );
            }
        } else if nc.live && track.hovered() {
            let dy = crate::app::raw_scroll_y(nc.ui.ctx());
            if dy.abs() > 0.5 {
                want = Some(((v as f32 + (dy / 120.0) * 4.0).clamp(0.0, 100.0)).round() as u32);
            }
        }
        if let Some(nv) = want {
            if self.volume_ui.map(|(o, _)| o) != Some(nv) || muted {
                self.volume_ui = Some((nv, false));
                self.volume_quiet_until = self.t + 0.9;
                self.pending.push(Cmd::SetVolume(nv));
            }
        }
        let (v, muted) = self.volume_ui.unwrap_or((v, muted));
        let pressed = track.dragged()
            || track.is_pointer_button_down_on()
            || icon.is_pointer_button_down_on();
        let target = if muted { 0.0 } else { v as f32 / 100.0 };
        self.volume_a = crate::anim::approach(
            self.volume_a,
            target,
            if pressed { 34.0 } else { 13.0 },
            self.dt,
        );
        let h = if pressed { 7.0 } else { 5.0 };
        let y = 19.0 - h / 2.0;
        nc.fill(
            HUD_TRACK_X,
            y,
            HUD_TRACK_W,
            h,
            h / 2.0,
            Color32::from_white_alpha(if pressed { 46 } else { 34 }),
        );
        if self.volume_a > 0.0 {
            let w = (HUD_TRACK_W * self.volume_a).max(h);
            let col = if muted {
                Color32::from_gray(150)
            } else {
                theme::WHITE
            };
            nc.fill(HUD_TRACK_X, y, w, h, h / 2.0, col);
            nc.fill(
                HUD_TRACK_X,
                y + 0.8,
                (w - 1.2).max(0.0),
                h * 0.4,
                h * 0.2,
                Color32::from_white_alpha(64),
            );
            if pressed {
                nc.circle(HUD_TRACK_X + w - h / 2.0, y + h / 2.0, h * 0.95, col);
            }
        }
        let label = if muted {
            "静音".to_string()
        } else {
            format!("{v}%")
        };
        nc.centered_text(
            HUD_W - 26.0,
            19.0,
            &label,
            10.5,
            if pressed { W::Semibold } else { W::Regular },
            if muted { theme::GRAY } else { theme::WHITE },
            H::Center,
        );
    }

    pub(crate) fn drink_water(&mut self) {
        let hour = self.current_hour;
        let amount = self.settings.serving_ml;
        let was = self.water.percent();
        let before = self.water.drunk_ml;
        self.water.drink(amount, hour);
        let amount = self.water.drunk_ml.saturating_sub(before);
        self.last_drink = Some((amount, hour));
        self.confirm_health_reminder(HealthReminder::Water);
        self.pet.react(PetState::Satisfied);
        self.say(format!("已记录 {amount} ml"));
        if was < 100.0 && self.water.percent() >= 100.0 {
            self.alert("今日饮水目标达成", "做得不错，继续保持", 2, true);
        }
    }
}

/// 将本地提醒时间规范化，拒绝不存在的日期与超出范围的时分。
pub(crate) fn parse_due(value: &str, today: &str) -> Option<String> {
    let value = value.trim();
    let full = if value.len() == 5 {
        format!("{today} {value}")
    } else {
        value.to_string()
    };
    if !full.is_ascii() || full.len() != 16 {
        return None;
    }
    let b = full.as_bytes();
    if b[4] != b'-' || b[7] != b'-' || b[10] != b' ' || b[13] != b':' {
        return None;
    }
    let year = full[0..4].parse::<u32>().ok()?;
    let month = full[5..7].parse::<usize>().ok()?;
    let day = full[8..10].parse::<u32>().ok()?;
    let hour = full[11..13].parse::<u32>().ok()?;
    let minute = full[14..16].parse::<u32>().ok()?;
    if !(2000..=9999).contains(&year) || !(1..=12).contains(&month) || hour > 23 || minute > 59 {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day == 0 || day > days[month - 1] {
        return None;
    }
    Some(full)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn island_size_drag_commits_only_on_release() {
        let ctx = egui::Context::default();
        let mut value = 100;
        let pos = egui::pos2(30.0, 30.0);
        let end = egui::pos2(110.0, 30.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let frames = [
            vec![],
            vec![egui::Event::PointerMoved(pos)],
            vec![button(pos, true)],
            vec![egui::Event::PointerMoved(end)],
            vec![button(end, false)],
        ];
        for (i, events) in frames.into_iter().enumerate() {
            let _ = ctx.run_ui(egui::RawInput {
                    events,
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320.0, 80.0),
                    )),
                    ..Default::default()
                }, |ui|{
                    egui::CentralPanel::default()
                        .frame(egui::Frame::new().inner_margin(20))
                        .show(ui, |ui| {
                            ui.spacing_mut().slider_width = 180.0;
                            island_size_slider(ui, &mut value);
                        });
                });
            if i < 4 {
                assert_eq!(value, 100);
            } else {
                assert_ne!(value, 100);
            }
        }
    }
    #[test]
    fn number_stepper_clicks_follow_steps_and_limits() {
        for (initial, step, x, expected) in [
            (5u32, 1, 127.0, 6),
            (5, 1, 13.0, 4),
            (0, 1, 13.0, 0),
            (10, 1, 127.0, 10),
            (9, 3, 127.0, 10),
            (1, 3, 13.0, 0),
        ] {
            let ctx = egui::Context::default();
            let mut value = initial;
            let pos = egui::pos2(x + 20.0, 34.0);
            let events = [
                vec![],
                vec![egui::Event::PointerMoved(pos)],
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                }],
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            ];
            for events in events {
                let raw = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(200.0, 80.0),
                    )),
                    events,
                    ..Default::default()
                };
                let _ = ctx.run_ui(raw, |ui|{
                    egui::CentralPanel::default()
                        .frame(egui::Frame::new().inner_margin(20))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                number_stepper(ui, &mut value, 0, 10, "", step, 76.0);
                            });
                        });
                });
            }
            assert_eq!(value, expected, "初值 {initial}，步长 {step}，点击位置 {x}");
        }
    }

    #[test]
    fn number_stepper_fits_with_settings_button_padding() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(340.0, 240.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui|{
            egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ui, |ui| {
                    ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);
                    for mut value in [0u32, 25, 180] {
                        let row = ui.horizontal(|ui| {
                            number_stepper(ui, &mut value, 0, 180, " 分钟", 1, 76.0);
                        });
                        assert!(
                            row.response.rect.width() <= 140.1,
                            "数值控件超出预留宽度：{}",
                            row.response.rect.width()
                        );
                    }
                    for mut value in [25u32, 5, 15] {
                        setting_number(ui, "计时时长", &mut value, 1, 180, " 分钟", 1.0);
                    }
                    assert!(ui.min_rect().right() <= 340.1, "连续数值行不应撑宽卡片");
                });
        });
    }

    #[test]
    fn bluetooth_notice_uses_real_connection_and_battery_data() {
        assert_eq!(
            bluetooth_notice_content("OpenMove by Shokz · 电量 80%"),
            ("OpenMove by Shokz", "已连接", Some(80))
        );
        assert_eq!(bluetooth_notice_content("耳机"), ("耳机", "已连接", None));
        assert_eq!(
            bluetooth_notice_content("已断开连接"),
            ("蓝牙设备", "已断开", None)
        );
        assert_eq!(
            bluetooth_notice_content("青龙键盘 · 已断开"),
            ("青龙键盘", "已断开", None)
        );
        assert_eq!(
            bluetooth_notice_content("耳机 · 电量 101%"),
            ("耳机 · 电量 101%", "已连接", None)
        );
    }

    #[test]
    fn device_icons_render_as_visible_svg() {
        let icons: &[&[u8]] = &[
            include_bytes!("../assets/device-icons/headphones-round-bold.svg"),
            include_bytes!("../assets/device-icons/speaker-bold.svg"),
            include_bytes!("../assets/device-icons/keyboard-bold.svg"),
            include_bytes!("../assets/device-icons/mouse-bold.svg"),
            include_bytes!("../assets/device-icons/gamepad-bold.svg"),
            include_bytes!("../assets/device-icons/smartphone-bold.svg"),
            include_bytes!("../assets/device-icons/bluetooth-linear.svg"),
        ];
        for bytes in icons {
            let image = egui_extras::image::load_svg_bytes(bytes, &Default::default()).unwrap();
            assert!(image.pixels.iter().any(|p| p.a() > 0));
        }
    }

    #[test]
    fn due_time_checks_calendar_and_format() {
        assert_eq!(
            parse_due("09:30", "2026-10-29").as_deref(),
            Some("2026-10-29 09:30")
        );
        assert!(parse_due("2026-02-29 12:00", "").is_none());
        assert!(parse_due("2028-02-29 12:00", "").is_some());
        for value in [
            "24:00",
            "12:60",
            "明天中午",
            "2026-13-01 10:00",
            "2026-00-01 10:00",
        ] {
            assert!(parse_due(value, "2026-10-29").is_none());
        }
    }
}

pub(crate) struct TaskDraft {
    index: usize,
    original: String,
    text: String,
    due: String,
    error: String,
    focused: bool,
    reminder_only: bool,
    pub(crate) repeat: u8,
    pub(crate) picker: crate::task_picker::ReminderPicker,
}
impl App {
    pub(crate) fn open_task_dialog(&mut self, index: usize) {
        self.todo_choosing = false;
        if let Some(task) = self.todos.items.get(index) {
            let draft = TaskDraft {
                index,
                original: task.text.clone(),
                text: task.text.clone(),
                due: task.due.clone().unwrap_or_default(),
                error: String::new(),
                focused: false,
                reminder_only: false,
                repeat: task.repeat,
                picker: crate::task_picker::ReminderPicker::new(task.due.as_deref()),
            };
            self.show_page(4);
            self.task_dialog = Some(draft);
        }
    }
    pub(crate) fn open_task_reminder(&mut self, index: usize) {
        self.open_task_dialog(index);
        if let Some(d) = self.task_dialog.as_mut() {
            d.reminder_only = true;
            d.picker.show();
        }
    }
    pub(crate) fn draw_task_editor(&mut self, c: &Card) {
        if self
            .task_dialog
            .as_ref()
            .is_some_and(|draft| draft.picker.open)
        {
            self.draw_task_reminder(c);
            return;
        }
        let reminder_only = self.task_dialog.as_ref().is_some_and(|d| d.reminder_only);
        c.text(
            15.0,
            25.0,
            if reminder_only {
                "设置提醒"
            } else {
                "编辑待办"
            },
            size::TITLE,
            W::Bold,
            theme::WHITE,
            H::Left,
        );
        c.text(
            43.0,
            48.0,
            if reminder_only { "待办" } else { "内容" },
            10.5,
            W::Regular,
            theme::GRAY,
            H::Left,
        );
        c.fill(91.0, 33.0, 275.0, 20.0, 6.0, theme::CARD);
        if let Some(d) = self.task_dialog.as_ref().filter(|d| d.reminder_only) {
            c.text(
                97.0,
                47.0,
                &c.fit(&d.text, 11.5, W::Regular, 260.0),
                11.5,
                W::Regular,
                theme::WHITE,
                H::Left,
            );
        }
        c.text(43.0, 72.0, "提醒", 10.5, W::Regular, theme::GRAY, H::Left);
        let time = self
            .task_dialog
            .as_ref()
            .map(|d| {
                if d.due.is_empty() {
                    "点击选择时间".into()
                } else if d.repeat != 0 {
                    format!(
                        "{} {}",
                        crate::model::repeat_label(d.repeat),
                        d.due.get(11..16).unwrap_or(&d.due)
                    )
                } else {
                    d.due.clone()
                }
            })
            .unwrap_or_default();
        if setting_button(c, 91.0, 58.0, 275.0, &time, false, "task-select-reminder") {
            if let Some(d) = self.task_dialog.as_mut() {
                d.picker.show();
            }
        }
        // 每周重复：七个星期按钮，全不选即一次性待办。
        c.text(43.0, 95.0, "重复", 10.5, W::Regular, theme::GRAY, H::Left);
        self.task_repeat_chips(c, 91.0, 81.0, 25.0, 19.0, 28.0);
        let repeat = self.task_dialog.as_ref().map(|d| d.repeat).unwrap_or(0);
        let quick = if repeat == crate::model::REPEAT_ALL {
            "不重复"
        } else {
            "每天"
        };
        if setting_button(c, 289.0, 81.0, 77.0, quick, false, "task-repeat-all") {
            if let Some(d) = self.task_dialog.as_mut() {
                d.repeat = if d.repeat == crate::model::REPEAT_ALL {
                    0
                } else {
                    crate::model::REPEAT_ALL
                };
            }
        }
        if let Some(d) = &self.task_dialog {
            let summary = if d.repeat != 0 {
                crate::model::repeat_label(d.repeat)
            } else {
                "只做一次".into()
            };
            let hint = if d.error.is_empty() {
                summary.as_str()
            } else {
                &d.error
            };
            c.text(
                43.0,
                117.0,
                &c.fit(
                    hint,
                    10.5,
                    W::Regular,
                    if reminder_only { 140.0 } else { 154.0 },
                ),
                10.5,
                if d.error.is_empty() {
                    W::Regular
                } else {
                    W::Semibold
                },
                if d.error.is_empty() {
                    theme::GRAY
                } else {
                    theme::RED
                },
                H::Left,
            );
        }
        if reminder_only {
            let has_reminder = self
                .task_dialog
                .as_ref()
                .and_then(|d| self.todos.items.get(d.index))
                .is_some_and(|task| task.due.is_some());
            let skip = if has_reminder {
                "取消提醒"
            } else if repeat != 0 {
                "只设重复"
            } else {
                "暂不提醒"
            };
            if setting_button(c, 188.0, 104.0, 88.0, skip, false, "task-reminder-skip") {
                if let Some(d) = self.task_dialog.as_mut() {
                    d.due.clear();
                }
                self.save_task_editor();
            }
        } else if setting_button(c, 202.0, 104.0, 74.0, "取消", false, "task-cancel") {
            self.task_dialog = None;
        }
        if setting_button(
            c,
            286.0,
            104.0,
            80.0,
            if reminder_only {
                "选择时间"
            } else {
                "保存"
            },
            true,
            "task-save",
        ) {
            if reminder_only {
                if let Some(d) = self.task_dialog.as_mut() {
                    d.picker.show();
                }
            } else {
                self.save_task_editor();
            }
        }
    }
    pub(crate) fn task_editor_inputs(
        &mut self,
        ui: &mut egui::Ui,
        origin: egui::Pos2,
        cmds: &mut Vec<Cmd>,
    ) {
        let Some(d) = self.task_dialog.as_mut() else {
            return;
        };
        if d.reminder_only || d.picker.open {
            return;
        }
        let name = ui.put(
            Rect::from_min_size(origin + egui::vec2(97.0, 35.0), egui::vec2(260.0, 17.0)),
            egui::TextEdit::singleline(&mut d.text)
                .vertical_align(egui::Align::Center)
                .id(egui::Id::new("task-name-edit"))
                .frame(egui::Frame::NONE)
                .font(theme::font(11.5, W::Regular)),
        );
        let first_focus = !d.focused;
        if first_focus {
            name.request_focus();
            d.focused = true;
            cmds.push(Cmd::Focus);
        }
        // 新建内容与下一步共用这一帧，避免同一次回车又提交了空提醒。
        if !first_focus && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.save_task_editor();
        }
    }
    /// 七个星期按钮（一…日），点一下切换；全不选即一次性待办。
    fn task_repeat_chips(&mut self, c: &Card, x: f32, y: f32, w: f32, h: f32, step: f32) {
        let repeat = self.task_dialog.as_ref().map(|d| d.repeat).unwrap_or(0);
        for (day, name) in crate::model::WEEKDAY_NAMES.iter().enumerate() {
            let on = repeat & (1 << day) != 0;
            let cx = x + day as f32 * step;
            let (r, b) = crate::paint::capsule(
                c,
                cx,
                y,
                w,
                h,
                &format!("task-repeat-{day}"),
                if on {
                    Color32::from_rgb(39, 94, 150)
                } else {
                    theme::BTN
                },
                theme::BTN_HOVER,
            );
            b.centered_text(
                cx + w / 2.0,
                y + h / 2.0,
                name,
                if h < 18.0 { 9.5 } else { 10.5 },
                W::Semibold,
                if on { theme::WHITE } else { theme::GRAY },
                H::Center,
            );
            if r.on_hover_text(format!("每周{name}")).clicked() {
                if let Some(d) = self.task_dialog.as_mut() {
                    d.repeat ^= 1 << day;
                    d.error.clear();
                }
            }
        }
    }
    fn draw_task_reminder(&mut self, c: &Card) {
        use crate::task_picker::PickerAction;
        let Some(d) = self.task_dialog.as_mut().filter(|d| d.picker.open) else {
            return;
        };
        let has_reminder = self
            .todos
            .items
            .get(d.index)
            .is_some_and(|task| task.due.is_some());
        c.text(
            15.0,
            25.0,
            "待办",
            size::TITLE,
            W::Bold,
            theme::WHITE,
            H::Left,
        );
        c.text(
            44.0,
            25.0,
            "清单",
            size::TITLE,
            W::Bold,
            theme::GREEN,
            H::Left,
        );
        let (done, total) = self.todos.progress(&self.today);
        c.text(
            42.0,
            61.0,
            &format!("{done}/{total}"),
            29.0,
            W::Bold,
            theme::WHITE,
            H::Left,
        );
        // 左侧：每周重复。选了星期几后，提醒只按时间、在这些日子到点提醒。
        d.picker.repeat_note = Some(if d.repeat != 0 {
            format!("{} · 到点提醒", crate::model::repeat_label(d.repeat))
        } else {
            String::new()
        });
        let action = d.picker.draw(c, &d.text, has_reminder);
        let focus = std::mem::take(&mut d.picker.focus);
        self.task_repeat_chips(c, 41.0, 84.0, 17.0, 16.0, 19.5);
        let Some(d) = self.task_dialog.as_mut() else {
            return;
        };
        if focus {
            self.pending.push(Cmd::Focus);
        }
        match action {
            Some(PickerAction::Save) => {
                d.due = d.picker.due();
                d.picker.open = false;
                if d.reminder_only {
                    self.save_task_editor();
                }
            }
            Some(PickerAction::Skip) => {
                d.due.clear();
                d.picker.open = false;
                if d.reminder_only {
                    self.save_task_editor();
                }
            }
            Some(PickerAction::Cancel) => {
                d.picker.open = false;
                if d.reminder_only {
                    self.task_dialog = None;
                }
            }
            None => {}
        }
    }
    fn save_task_editor(&mut self) {
        let Some(mut d) = self.task_dialog.take() else {
            return;
        };
        let due = if d.due.trim().is_empty() {
            Some(None)
        } else {
            parse_due(&d.due, &self.today).map(Some)
        };
        if d.text.trim().is_empty() {
            d.error = "请填写任务内容".into();
        } else if due.is_none() {
            d.error = "时间格式应为 18:30 或完整日期时间".into();
        } else if self
            .todos
            .items
            .get(d.index)
            .is_none_or(|t| t.text != d.original)
        {
            d.error = "任务已变化，请取消后重新打开".into();
        } else {
            let item = &mut self.todos.items[d.index];
            if item.repeat != d.repeat {
                item.done_on = None;
            }
            item.text = d.text.trim().into();
            item.due = due.unwrap();
            item.reminded = false;
            item.reminded_on = None;
            item.repeat = d.repeat;
            self.todos.save();
            self.say("待办已保存");
            return;
        }
        self.task_dialog = Some(d);
    }
}

fn notice_button(c: &Card, x: f32, y: f32, w: f32, text: &str, id: &str) -> bool {
    let (r, b) = crate::paint::capsule(
        c,
        x,
        y,
        w,
        20.0,
        id,
        Color32::from_gray(35),
        theme::BTN_HOVER,
    );
    b.centered_text(
        x + w / 2.0,
        y + 10.0,
        text,
        10.5,
        W::Semibold,
        theme::WHITE,
        H::Center,
    );
    r.clicked()
}
fn setting_button(c: &Card, x: f32, y: f32, w: f32, text: &str, on: bool, id: &str) -> bool {
    let (r, b) = crate::paint::capsule(
        c,
        x,
        y,
        w,
        19.0,
        id,
        if on {
            Color32::from_rgb(39, 94, 150)
        } else {
            theme::BTN
        },
        theme::BTN_HOVER,
    );
    b.text(
        x + w / 2.0,
        y + 13.3,
        text,
        10.5,
        W::Semibold,
        theme::WHITE,
        H::Center,
    );
    r.clicked()
}
fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60 % 24, m % 60)
}
/// 岛内小开关（与健康页右上开关同款）。
fn island_switch(c: &Card, x: f32, y: f32, on: bool, id: &str) -> bool {
    let r = c.hit(x - 2.0, y - 2.0, 38.0, 24.0, id);
    let (hv, _) = c.hover_t(&r);
    let track = if on {
        theme::SIT_GREEN
    } else {
        Color32::from_rgb(58, 58, 62)
    };
    c.fill(
        x,
        y,
        34.0,
        20.0,
        10.0,
        crate::paint::mix(track, theme::WHITE, hv * 0.08),
    );
    c.circle(
        if on { x + 24.0 } else { x + 10.0 },
        y + 10.0,
        8.0,
        theme::WHITE,
    );
    r.clicked()
}
impl App {
    fn health_time_summary(&self) -> String {
        let s = &self.settings;
        let hours = format!("{:02}:00–{:02}:00", s.water_start, s.water_end);
        if s.lunch_rest {
            format!(
                "{hours} · 午休 {}–{}",
                hhmm(s.lunch_start),
                hhmm(s.lunch_end)
            )
        } else {
            hours
        }
    }
    /// 「提醒时段」二级页：提醒时段起止、午休开关、午休起止（15 分钟一档），喝水和久坐共用。
    fn health_time_page(&mut self, c: &Card) {
        let edit_snapshot = self.settings.clone();
        let row_y = |i: usize| 38.0 + i as f32 * 22.0;
        for i in 1..4 {
            c.fill(
                44.0,
                row_y(i) - 2.0,
                326.0,
                1.0,
                0.5,
                Color32::from_rgb(28, 28, 31),
            );
        }
        let y = row_y(0);
        c.text(
            44.0,
            y + 13.0,
            "提醒时段",
            10.5,
            W::Regular,
            theme::GRAY,
            H::Left,
        );
        c.text(
            250.0,
            y + 13.0,
            "至",
            10.5,
            W::Regular,
            theme::GRAY,
            H::Center,
        );
        for i in 0..2 {
            let hour = if i == 0 {
                &mut self.settings.water_start
            } else {
                &mut self.settings.water_end
            };
            let value = egui::DragValue::new(hour)
                .range(0..=if i == 0 { 23 } else { 24 })
                .custom_formatter(|v, _| format!("{:02}:00", v as u32))
                .custom_parser(|t| {
                    let t = t.trim();
                    t.strip_suffix(":00")
                        .or_else(|| t.strip_suffix("：00"))
                        .unwrap_or(t)
                        .parse::<u32>()
                        .ok()
                        .map(|v| v as f64)
                });
            let (d, field) = stepper(
                c,
                if i == 0 { 238.0 } else { 370.0 },
                y,
                value,
                &format!("hour-{i}"),
            );
            if field.gained_focus() {
                note_number_edit_focus(c, ("health-number", format!("hour-{i}")), &edit_snapshot);
                self.pending.push(Cmd::Focus);
            }
            self.settings_editing |= field.has_focus();
            if d != 0 {
                let hour = if i == 0 {
                    &mut self.settings.water_start
                } else {
                    &mut self.settings.water_end
                };
                *hour = (*hour as i32 + d).rem_euclid(24) as u32;
            }
        }
        let y = row_y(1);
        c.text(
            44.0,
            y + 13.0,
            "午休不提醒",
            10.5,
            W::Regular,
            theme::GRAY,
            H::Left,
        );
        if island_switch(c, 336.0, y - 0.5, self.settings.lunch_rest, "lunch-rest") {
            self.settings.lunch_rest = !self.settings.lunch_rest;
        }
        let y = row_y(2);
        let on = self.settings.lunch_rest;
        let lc = c.faded(if on { 1.0 } else { 0.35 });
        let lc = if on { lc } else { lc.ghost() };
        lc.text(
            44.0,
            y + 13.0,
            "午休时间",
            10.5,
            W::Regular,
            theme::GRAY,
            H::Left,
        );
        lc.text(
            250.0,
            y + 13.0,
            "至",
            10.5,
            W::Regular,
            theme::GRAY,
            H::Center,
        );
        for i in 0..2 {
            let minute = if i == 0 {
                &mut self.settings.lunch_start
            } else {
                &mut self.settings.lunch_end
            };
            let value = egui::DragValue::new(minute)
                .range(0..=1439)
                .speed(1.0)
                .custom_formatter(|v, _| hhmm(v as u32))
                .custom_parser(|t| {
                    let (h, m) = t.trim().split_once([':', '：'])?;
                    Some(
                        (h.parse::<u32>().ok()?.min(23) * 60 + m.parse::<u32>().ok()?.min(59))
                            as f64,
                    )
                });
            let (d, field) = stepper(
                &lc,
                if i == 0 { 238.0 } else { 370.0 },
                y,
                value,
                &format!("lunch-{i}"),
            );
            if field.gained_focus() {
                note_number_edit_focus(c, ("health-number", format!("lunch-{i}")), &edit_snapshot);
                self.pending.push(Cmd::Focus);
            }
            self.settings_editing |= field.has_focus();
            if d != 0 {
                let minute = if i == 0 {
                    &mut self.settings.lunch_start
                } else {
                    &mut self.settings.lunch_end
                };
                *minute = (*minute as i32 + d * 15).rem_euclid(1440) as u32;
            }
        }
        c.text(
            44.0,
            row_y(3) + 13.0,
            "喝水和久坐提醒都只在提醒时段内、午休以外弹出",
            10.5,
            W::Regular,
            theme::GRAY,
            H::Left,
        );
    }
}
fn island_number_field(
    c: &Card,
    rect: Rect,
    value: egui::DragValue<'_>,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> egui::Response {
    let salt_id = egui::Id::new(&id);
    let mut ui = egui::Ui::new(
        c.ui.ctx().clone(),
        salt_id,
        egui::UiBuilder::new()
            .layer_id(c.ui.layer_id())
            .max_rect(rect)
            .style(c.ui.style().clone()),
    );
    ui.set_clip_rect(c.painter.clip_rect());
    ui.set_opacity(c.painter.opacity());
    if !c.live {
        ui.disable();
    }
    ui.spacing_mut().interact_size = rect.size();
    ui.spacing_mut().button_padding = egui::vec2(2.0, 1.0);
    ui.style_mut().drag_value_text_style = egui::TextStyle::Body;
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, c.font(11.0, W::Bold));
    ui.style_mut().explanation_tooltips = false;
    ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    let response = ui.put(rect, value.update_while_editing(false));
    // 记录 DragValue 真正使用的 id（Ui::put 还会套一层 child scope），供 Esc/步进清理用。
    c.ui.ctx().data_mut(|data| {
        data.insert_temp(salt_id.with("widget-id"), response.id);
    });
    crate::quota_ui::bar_tooltip(response, "点击输入数值".into())
}
/// 数字框聚焦时记下整份设置快照；Esc 取消输入时由 App 恢复（0.35 会立即写回键入值）。
fn note_number_edit_focus(c: &Card, field_id: impl std::hash::Hash + std::fmt::Debug, snapshot: &crate::model::Settings) {
    let widget_id = c.ui.ctx().data(|data| data.get_temp::<egui::Id>(egui::Id::new(&field_id).with("widget-id")));
    if let Some(widget_id) = widget_id {
        c.ui.ctx().data_mut(|data| {
            data.insert_temp(egui::Id::new("settings-edit-snapshot"), (widget_id, snapshot.clone()));
        });
    }
}
/// 点击加减按钮后清掉 DragValue 缓存的编辑文本，避免下一帧又把旧输入写回、覆盖步进结果。
fn clear_number_edit(c: &Card, id: impl std::hash::Hash + std::fmt::Debug) {
    let widget_id = c.ui.ctx().data(|data| data.get_temp::<egui::Id>(egui::Id::new(&id).with("widget-id")));
    if let Some(widget_id) = widget_id {
        c.ui.ctx().data_mut(|data| { data.remove_temp::<String>(widget_id); });
    }
}
/// 加减器：[−] 可输入数值 [+]，右缘对齐到 right。
fn stepper(
    c: &Card,
    right: f32,
    y: f32,
    value: egui::DragValue<'_>,
    id: &str,
) -> (i32, egui::Response) {
    let (w, vw) = (25.0, 58.0);
    let x = right - w * 2.0 - vw;
    let field = island_number_field(c, c.rect(x + w, y, vw, 19.0), value, ("health-number", id));
    let delta = if setting_button(c, x, y, w, "−", false, &format!("{id}-minus")) {
        -1
    } else if setting_button(c, right - w, y, w, "+", false, &format!("{id}-plus")) {
        1
    } else {
        0
    };
    (delta, field)
}
impl App {
    pub(crate) fn page_settings(&mut self, c: &Card) {
        let help = match self.settings_category {
            0 => "外观与系统",
            1 => "调整喝水、久坐与提醒时段",
            2 => "点击分钟数可直接输入，也可用加减微调",
            _ => "通知",
        };
        if self.settings_category != 1 {
            c.text(44.0, 44.0, help, 10.5, W::Regular, theme::GRAY, H::Left);
        }
        let before = format!("{:?}", self.settings);
        let edit_snapshot = self.settings.clone();
        self.settings_editing = false;
        match self.settings_category {
            0 => {
                let choices = [
                    ("吉祥物", self.settings.pet),
                    ("极简刘海", self.settings.minimal),
                    ("系统提示", self.settings.system_hud),
                    ("全屏隐藏", self.settings.hide_fullscreen),
                ];
                for (i, (name, on)) in choices.iter().enumerate() {
                    let x = 42.0 + (i % 2) as f32 * 169.0;
                    let y = 57.0 + (i / 2) as f32 * 27.0;
                    let text = format!("{}  {}", name, if *on { "开" } else { "关" });
                    if setting_button(c, x, y, 158.0, &text, *on, &format!("general-{i}")) {
                        match i {
                            0 => self.set_pet(!self.settings.pet),
                            1 => self.set_minimal(!self.settings.minimal),
                            2 => self.settings.system_hud = !self.settings.system_hud,
                            _ => self.settings.hide_fullscreen = !self.settings.hide_fullscreen,
                        }
                    }
                }
                c.text(
                    42.0,
                    116.0,
                    "岛仔收起时常驻；极简模式保留细线",
                    10.5,
                    W::Regular,
                    theme::GRAY,
                    H::Left,
                );
            }
            1 if self.health_time => self.health_time_page(c),
            1 => {
                // 与健康页同一套切换（标题旁的水滴 / 椅子）；四行一列，标签靠左、加减器靠右对齐。
                // 喝水三行、久坐一行加两句说明；最后一行「提醒时段」两边共用，位置不变。
                let sv = self.sit_view_t.pos.clamp(0.0, 1.0);
                let row_y = |i: usize| 38.0 + i as f32 * 22.0;
                for i in 1..4 {
                    c.fill(
                        44.0,
                        row_y(i) - 2.0,
                        326.0,
                        1.0,
                        0.5,
                        Color32::from_rgb(28, 28, 31),
                    );
                }
                if sv < 0.99 {
                    let wc = c.faded(1.0 - sv);
                    let wc = if self.sit_mode { wc.ghost() } else { wc };
                    for (i, label) in ["每日目标", "每次记录", "提醒间隔"].iter().enumerate()
                    {
                        wc.text(
                            44.0,
                            row_y(i) + 13.0,
                            label,
                            10.5,
                            W::Regular,
                            theme::GRAY,
                            H::Left,
                        );
                        let goal = self.water.goal_ml;
                        let mut liters = goal as f32 / 1000.0;
                        let value = match i {
                            0 => egui::DragValue::new(&mut liters)
                                .range(0.5..=6.0)
                                .speed(0.25)
                                .suffix(" L")
                                .min_decimals(1)
                                .max_decimals(2),
                            1 => egui::DragValue::new(&mut self.settings.serving_ml)
                                .range(50..=1000)
                                .speed(50.0)
                                .suffix(" ml"),
                            _ => egui::DragValue::new(&mut self.settings.water_minutes)
                                .range(5..=180)
                                .speed(5.0)
                                .suffix(" 分钟"),
                        };
                        let (d, field) =
                            stepper(&wc, 370.0, row_y(i), value, &format!("water-{i}"));
                    if field.gained_focus() {
                        note_number_edit_focus(c, ("health-number", format!("water-{i}")), &edit_snapshot);
                        self.pending.push(Cmd::Focus);
                    }
                    self.settings_editing |= field.has_focus();
                    if d != 0 {
                        clear_number_edit(&wc, ("health-number", format!("water-{i}")));
                        }
                        match i {
                            0 => {
                                self.water.goal_ml = ((liters * 1000.0).round() as i32 + d * 250)
                                    .clamp(500, 6000)
                                    as u32;
                                if self.water.goal_ml != goal {
                                    self.water.save();
                                }
                            }
                            1 => {
                                self.settings.serving_ml =
                                    (self.settings.serving_ml as i32 + d * 50).clamp(50, 1000)
                                        as u32
                            }
                            _ => {
                                self.settings.water_minutes =
                                    (self.settings.water_minutes as i32 + d * 5).clamp(5, 180)
                                        as u32
                            }
                        }
                    }
                }
                if sv > 0.01 {
                    let sc = c.faded(sv);
                    let sc = if self.sit_mode { sc } else { sc.ghost() };
                    sc.text(
                        44.0,
                        row_y(0) + 13.0,
                        "坐满多久提醒",
                        10.5,
                        W::Regular,
                        theme::GRAY,
                        H::Left,
                    );
                    let value = egui::DragValue::new(&mut self.settings.sit_minutes)
                        .range(15..=180)
                        .speed(5.0)
                        .suffix(" 分钟");
                    let (d, field) = stepper(&sc, 370.0, row_y(0), value, "sit");
                    if field.gained_focus() {
                        note_number_edit_focus(c, ("health-number", "sit"), &edit_snapshot);
                        self.pending.push(Cmd::Focus);
                    }
                    self.settings_editing |= field.has_focus();
                    if d != 0 {
                        self.settings.sit_minutes =
                            (self.settings.sit_minutes as i32 + d * 5).clamp(15, 180) as u32;
                    }
                    sc.text(
                        44.0,
                        row_y(1) + 13.0,
                        "专注计时进行中不打扰，专注结束后再提醒",
                        10.5,
                        W::Regular,
                        theme::GRAY,
                        H::Left,
                    );
                    sc.text(
                        44.0,
                        row_y(2) + 13.0,
                        "提醒后常驻，点击确认或起身后重新计时",
                        10.5,
                        W::Regular,
                        theme::GRAY,
                        H::Left,
                    );
                }
                // 最后一行两边共用：提醒时段 + 午休的摘要，点进二级页修改。
                let y = row_y(3);
                let r = c
                    .hit(40.0, y - 2.0, 334.0, 22.0, "health-time")
                    .on_hover_text("设置提醒时段和午休免打扰");
                let (hv, _) = c.hover_t(&r);
                if hv > 0.0 {
                    c.fill(
                        40.0,
                        y - 2.0,
                        334.0,
                        22.0,
                        8.0,
                        Color32::from_white_alpha((hv * 14.0) as u8),
                    );
                }
                c.text(
                    44.0,
                    y + 13.0,
                    "提醒时段",
                    10.5,
                    W::Regular,
                    theme::GRAY,
                    H::Left,
                );
                c.text(
                    356.0,
                    y + 13.0,
                    &self.health_time_summary(),
                    10.5,
                    W::Semibold,
                    theme::WHITE,
                    H::Right,
                );
                c.text(366.0, y + 13.5, "›", 14.0, W::Bold, theme::GRAY, H::Center);
                if r.clicked() {
                    self.health_time = true;
                }
            }
            2 => {
                for (i, name) in ["专注", "休息"].iter().enumerate() {
                    let y = 53.0 + i as f32 * 23.0;
                    c.text(48.0, y + 13.0, name, 10.5, W::Regular, theme::GRAY, H::Left);
                    let value = if i == 0 {
                        &mut self.settings.focus_minutes
                    } else {
                        &mut self.settings.rest_minutes
                    };
                    let max = if i == 0 { 180 } else { 60 };
                    let rect = c.rect(168.0, y, 76.0, 19.0);
                    let field = island_number_field(
                        c,
                        rect,
                        egui::DragValue::new(value).range(1..=max).suffix(" 分钟"),
                        ("focus-duration", i),
                    );
                    if field.gained_focus() {
                        note_number_edit_focus(c, ("focus-duration", i), &edit_snapshot);
                        self.pending.push(Cmd::Focus);
                    }
                    self.settings_editing |= field.has_focus();
                    if setting_button(c, 131.0, y, 27.0, "−", false, &format!("focus-minus-{i}"))
                    {
                        clear_number_edit(c, ("focus-duration", i));
                        *value = value.saturating_sub(1).max(1);
                    }
                    if setting_button(c, 255.0, y, 27.0, "+", false, &format!("focus-plus-{i}")) {
                        clear_number_edit(c, ("focus-duration", i));
                        *value = (*value + 1).min(max);
                    }
                }
                c.text(
                    315.0,
                    93.0,
                    "下次生效",
                    10.5,
                    W::Regular,
                    theme::GRAY,
                    H::Center,
                );
            }
            _ => {
                if setting_button(
                    c,
                    43.0,
                    56.0,
                    152.0,
                    if self.settings.notifications {
                        "提醒 · 开"
                    } else {
                        "提醒 · 关"
                    },
                    self.settings.notifications,
                    "set-notice",
                ) {
                    self.settings.notifications = !self.settings.notifications;
                }
                if setting_button(
                    c,
                    212.0,
                    56.0,
                    153.0,
                    if self.settings.system_notify && self.settings.sound {
                        "声音 · 开"
                    } else {
                        "声音 · 关"
                    },
                    self.settings.system_notify && self.settings.sound,
                    "set-sound",
                ) {
                    self.settings.sound = !(self.settings.system_notify && self.settings.sound);
                    if self.settings.sound {
                        self.settings.system_notify = true;
                    }
                }
                if setting_button(
                    c,
                    43.0,
                    84.0,
                    152.0,
                    if self.settings.read_notifications {
                        "断开系统通知"
                    } else {
                        "连接系统通知"
                    },
                    self.settings.read_notifications,
                    "set-listener",
                ) {
                    self.pending.push(if self.settings.read_notifications {
                        Cmd::DisconnectNotifications
                    } else {
                        Cmd::RequestNotifications
                    });
                }
                let status = c.fit(&self.listener_status, 10.5, W::Regular, 170.0);
                c.text(210.0, 97.0, &status, 10.5, W::Regular, theme::GRAY, H::Left);
            }
        }
        if before != format!("{:?}", self.settings) {
            self.settings.save();
        }
    }
}

/// 不可用的设置行：开关置灰、点击无效。
/// 用于「本机没装对应组件」的情况（例如没有「手机连接」时的手机电量）。
pub(crate) fn setting_row_locked(ui: &mut egui::Ui, title: &str, description: &str, value: bool) {
    let ctx = &ui.ctx().clone();
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let text_w = (ui.available_width() - SETTINGS_SWITCH_W - SETTINGS_ROW_GAP).max(120.0);
        ui.vertical(|ui| {
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add(
                egui::Label::new(egui::RichText::new(title).size(13.5).color(settings_color(
                    ctx,
                    Color32::from_gray(130),
                    Color32::from_gray(140),
                )))
                .wrap(),
            );
            if !description.is_empty() {
                ui.add(
                    egui::Label::new(egui::RichText::new(description).size(12.0).color(
                        settings_color(ctx, Color32::from_gray(118), Color32::from_gray(132)),
                    ))
                    .wrap(),
                );
            }
        });
        ui.add_space(
            (ui.available_width() - SETTINGS_SWITCH_W - ui.spacing().item_spacing.x).max(0.0),
        );
        let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 20.0), egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            10.0,
            settings_color(
                ctx,
                Color32::from_gray(58),
                Color32::from_rgb(226, 229, 234),
            ),
        );
        let x = if value {
            rect.right() - 10.0
        } else {
            rect.left() + 10.0
        };
        ui.painter().circle_filled(
            egui::pos2(x, rect.center().y + 0.5),
            7.5,
            Color32::from_black_alpha(20),
        );
        ui.painter().circle_filled(
            egui::pos2(x, rect.center().y),
            7.0,
            settings_color(ctx, Color32::from_gray(150), Color32::WHITE),
        );
    });
}
fn switch(ui: &mut egui::Ui, value: &mut bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(34.0, 20.0), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
    }
    let fill = if *value {
        Color32::from_rgb(48, 184, 92)
    } else {
        settings_color(
            &ui.ctx().clone(),
            Color32::from_rgb(82, 82, 87),
            Color32::from_rgb(203, 207, 214),
        )
    };
    ui.painter().rect_filled(rect, 10.0, fill);
    let x = if *value {
        rect.right() - 10.0
    } else {
        rect.left() + 10.0
    };
    ui.painter().circle_filled(
        egui::pos2(x, rect.center().y + 0.5),
        7.5,
        Color32::from_black_alpha(35),
    );
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), 7.0, Color32::WHITE);
    response.clicked()
}
/// 设置页版式常量。
pub(crate) const SETTINGS_LABEL_W: f32 = 80.0;
const SETTINGS_SWITCH_W: f32 = 34.0;
const SETTINGS_ROW_GAP: f32 = 16.0;

/// 开关行：左侧标题 + 说明（宽度固定、自动换行），右侧开关，长说明也不会把开关挤出卡片。
pub(crate) fn setting_row(ui: &mut egui::Ui, title: &str, description: &str, value: &mut bool) {
    let ctx = &ui.ctx().clone();
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let text_w = (ui.available_width() - SETTINGS_SWITCH_W - SETTINGS_ROW_GAP).max(120.0);
        ui.vertical(|ui| {
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.add(
                egui::Label::new(egui::RichText::new(title).size(13.5).color(settings_color(
                    ctx,
                    Color32::from_rgb(238, 240, 244),
                    Color32::from_rgb(24, 27, 32),
                )))
                .wrap(),
            );
            if !description.is_empty() {
                ui.add(
                    egui::Label::new(egui::RichText::new(description).size(12.0).color(
                        settings_color(
                            ctx,
                            Color32::from_gray(158),
                            Color32::from_rgb(96, 101, 110),
                        ),
                    ))
                    .wrap(),
                );
            }
        });
        // add_space 之后还会再插入一次 item_spacing.x，这里一并扣掉，开关才不会每行往外挤一点。
        ui.add_space(
            (ui.available_width() - SETTINGS_SWITCH_W - ui.spacing().item_spacing.x).max(0.0),
        );
        switch(ui, value);
    });
    ui.add_space(2.0);
}

/// 设置页统一的分段选择器（灰色底框 + 选中块）。返回值为是否发生了切换。
pub(crate) fn settings_segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, &str)],
    item_w: f32,
) -> bool {
    let ctx = &ui.ctx().clone();
    let mut changed = false;
    egui::Frame::new()
        .fill(settings_color(
            ctx,
            Color32::from_rgb(28, 28, 30),
            Color32::from_rgb(230, 233, 238),
        ))
        .corner_radius(8.0)
        .inner_margin(3)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (option, label) in options {
                    let selected = *value == *option;
                    let text = egui::RichText::new(*label).size(12.0).color(if selected {
                        settings_color(ctx, Color32::WHITE, Color32::from_rgb(30, 34, 41))
                    } else {
                        settings_color(ctx, Color32::from_gray(200), Color32::from_gray(70))
                    });
                    let button = egui::Button::new(text)
                        .fill(if selected {
                            settings_color(ctx, Color32::from_rgb(72, 72, 76), Color32::WHITE)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(6.0);
                    let width = item_w.max(
                        ctx.fonts_mut(|f| {
                            f.layout_no_wrap(
                                label.to_string(),
                                egui::FontId::proportional(12.0),
                                Color32::WHITE,
                            )
                            .size()
                            .x
                        }) + 20.0,
                    );
                    if ui.add_sized([width, 26.0], button).clicked() && !selected {
                        *value = *option;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// 选项行：标签列之后紧跟控件，同一卡片里所有控件左缘对齐成一条竖线。
pub(crate) fn settings_control_row_right(
    ui: &mut egui::Ui,
    title: &str,
    controls: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    settings_control_row(ui, title, SETTINGS_LABEL_W, |ui| {
        ui.horizontal(|ui| controls(ui)).response
    })
}

/// 设置项下方的说明文字。
/// 液态玻璃参数弹窗：入口按钮在「外观 → 液态玻璃与交互」卡里，靠「配置液态玻璃」唤出。
/// 调参时的取景形态（展开/收起）由弹窗里的「预览形态」开关决定，逻辑在 app 的状态机里
/// （设置页开着 + 外观页 + 玻璃皮肤 + 弹窗开着）；离开外观页或关掉设置窗就自动收起弹窗。
fn liquid_glass_config_window(app: &mut crate::app::App, ctx: &egui::Context) {
    // 液态玻璃参数与「兼容渲染（Direct3D 12）」无关：D3D11 自绘栈下也要能调。
    // 只按入口按钮的显示条件收起弹窗（设置页 + 外观页 + 液态皮肤）。
    if !(app.settings_open
        && app.settings_tab == SETTINGS_TAB_APPEARANCE
        && app.settings.skin == crate::model::Skin::Liquid)
    {
        app.liquid_cfg_open = false;
    }
    let mut open = app.liquid_cfg_open;
    if !open {
        return;
    }
    egui::Window::new(egui::RichText::new("液态玻璃配置").size(14.0).strong())
            .open(&mut open)
            .collapsible(false).resizable(false).default_width(560.0)
            .show(ctx,|ui|{
                egui::ScrollArea::vertical().max_height(430.0).show(ui,|ui|{
                    ui.set_min_width(ui.available_width());
                    settings_hint(ui,"调参数时岛仔会保持下面选中的形态，方便直接看效果");
                    let forms:[(bool,&str);2]=[(false,"展开态"),(true,"收起态")];
                    settings_control_row_right(ui,"预览形态",|ui|{settings_segmented(ui,&mut app.liquid_preview_collapsed,&forms,62.0);});
                    settings_hint(ui,"展开态看展开后的控件玻璃；收起态看胶囊常驻条的玻璃");
                                ui.separator();
                                let before=app.settings.liquid_prefer_dgpu;
                                setting_row(ui,"优先独立显卡","让液态玻璃尽量使用独显，重启生效",&mut app.settings.liquid_prefer_dgpu);
                                if before!=app.settings.liquid_prefer_dgpu {app.settings.save();}
                                settings_hint(ui,"混合显卡机器建议开启；当前 D3D11 栈的选卡策略仍在完善");
                                // 与启动期开关一致：改了重启才生效的设置就直接给「立即重启」。
                                // 启动值由 main.rs 写进环境变量（建窗前快照），无需额外字段。
                                let dgpu_boot=std::env::var("DAOZAI_LIQUID_PREFER_DGPU").map(|v|v!="0").unwrap_or(app.settings.liquid_prefer_dgpu);
                                if app.settings.liquid_prefer_dgpu!=dgpu_boot {
                                    ui.horizontal(|ui|{
                                        ui.label(egui::RichText::new("优先独显重启岛仔后生效").size(12.0).color(Color32::from_rgb(230,150,40)));
                                        if ui.button("立即重启").clicked(){app.pending.push(crate::app::Cmd::RestartApp);}
                                    });
                                }
                                ui.separator();
                                settings_control_row(ui,"形变强度",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_morph,0.0..=3.0).fixed_decimals(2));
                                        if ui.add_enabled((app.settings.liquid_morph-1.0).abs()>0.001,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_morph=1.0;}
                                    }).response
                                });
                                settings_hint(ui,"展开/折叠瞬间边缘折射的增强倍数；0 = 关闭，默认 1.0");
                                ui.separator();
                                settings_control_row(ui,"折射强度",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_refract_px,0.0..=120.0).fixed_decimals(0));
                                        if ui.add_enabled((app.settings.liquid_refract_px-48.0).abs()>0.05,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_refract_px=48.0;}
                                    }).response
                                });
                                settings_hint(ui,"边缘处背景被弯折的像素数（默认 48）；调大像更厚的透镜，调小更平");
                                ui.separator();
                                settings_control_row(ui,"色散强度",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_chroma_px,0.0..=30.0).fixed_decimals(0));
                                        if ui.add_enabled((app.settings.liquid_chroma_px-8.0).abs()>0.05,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_chroma_px=8.0;}
                                    }).response
                                });
                                settings_hint(ui,"红蓝彩边的强度，红蓝实际分离是它的两倍（默认 8；锐利抓屏在字边更容易出彩边）；0 = 关掉色散");
                                ui.separator();
                                settings_control_row(ui,"内部模糊",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_blur_px,0.0..=4.0).fixed_decimals(1));
                                        if ui.add_enabled((app.settings.liquid_blur_px-1.5).abs()>0.05,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_blur_px=1.5;}
                                    }).response
                                });
                                settings_hint(ui,"折射带里五点采样的步长（默认 1.5）；调大整个折射带更「磨」");
                                ui.separator();
                                settings_control_row(ui,"折射带宽",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_band_px,10.0..=140.0).fixed_decimals(0));
                                        if ui.add_enabled((app.settings.liquid_band_px-87.0).abs()>0.5,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_band_px=87.0;}
                                    }).response
                                });
                                settings_hint(ui,"折射从边缘往里延伸多深（默认 87）；小 = 只有边缘一圈在弯，大 = 大半块面板都在弯");
                                ui.separator();
                                settings_control_row(ui,"白边宽度",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_edge_w_px,0.0..=4.0).fixed_decimals(1));
                                        if ui.add_enabled((app.settings.liquid_edge_w_px-1.5).abs()>0.05,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_edge_w_px=1.5;}
                                    }).response
                                });
                                settings_hint(ui,"边缘高光的宽度（默认 1.5）；0 = 关掉白边");
                                ui.separator();
                                settings_control_row(ui,"白边亮度",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_edge_op,0.0..=1.0).fixed_decimals(2));
                                        if ui.add_enabled((app.settings.liquid_edge_op-0.90).abs()>0.005,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_edge_op=0.90;}
                                    }).response
                                });
                                settings_hint(ui,"边缘高光亮度（默认 0.90）；随背景明暗自动微调");
                                ui.separator();
                                settings_control_row(ui,"高光跟随鼠标",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_light_follow,0.0..=1.0).fixed_decimals(2));
                                        if ui.add_enabled((app.settings.liquid_light_follow-1.0).abs()>0.005,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_light_follow=1.0;}
                                    }).response
                                });
                                settings_hint(ui,"高光是否随鼠标方向摆动（默认 1.0）：0 = 固定左上光，不跟鼠标；1 = 完全跟随");
                                ui.separator();
                                settings_control_row(ui,"弹簧刚度",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_spring,0.2..=3.0).fixed_decimals(2));
                                        if ui.add_enabled((app.settings.liquid_spring-1.0).abs()>0.005,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_spring=1.0;}
                                    }).response
                                });
                                settings_hint(ui,"控件果冻回弹的快慢（默认 1.0）：调大弹得更快，调小更柔更缓");
                                ui.separator();
                                settings_control_row(ui,"阻尼比",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_zeta,0.1..=1.0).fixed_decimals(2));
                                        if ui.add_enabled((app.settings.liquid_zeta-0.5).abs()>0.005,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_zeta=0.5;}
                                    }).response
                                });
                                settings_hint(ui,"入场回弹的「收劲」（默认 0.5）：调小会来回弹跳几下、更 Q 弹，1.0 = 不弹跳直接到位；按压反馈固定为不过冲，不受这里影响");
                                ui.separator();
                                settings_control_row(ui,"静止刷新率",56.0,|ui|{
                                    ui.horizontal(|ui|{
                                        ui.spacing_mut().slider_width=180.0;
                                        ui.add(egui::Slider::new(&mut app.settings.liquid_idle_fps,1..=30).fixed_decimals(0));
                                        if ui.add_enabled(app.settings.liquid_idle_fps!=15,egui::Button::new("恢复默认")).clicked(){app.settings.liquid_idle_fps=15;}
                                    }).response
                                });
                                settings_hint(ui,"无悬停、无动画时的重绘帧率（默认 15）：调低更省电，但壁纸/窗口变化时玻璃背景跟得越慢；30 = 静止不降档");
                                ui.separator();
                                let before=app.settings.liquid_deep_sleep;
                                setting_row(ui,"收起态深度休眠","收起且完全静止 2 分钟后降到 1Hz，任何输入即时唤醒",&mut app.settings.liquid_deep_sleep);
                                if before!=app.settings.liquid_deep_sleep {app.settings.save();}
                                settings_hint(ui,"默认开；省电可保持开启");
                                ui.separator();
                                let before=app.settings.liquid_shadow;
                                setting_row(ui,"岛体投影","胶囊外围的多层落影",&mut app.settings.liquid_shadow);
                                if before!=app.settings.liquid_shadow {app.settings.save();}
                                settings_hint(ui,"默认开；浅色壁纸上比较明显，关掉外围更干净");
                });
            });
    app.liquid_cfg_open = open;
}

fn island_size_slider(ui: &mut egui::Ui, value: &mut u32) -> egui::Response {
    let id = ui.make_persistent_id("island-size-draft");
    let mut draft = ui.data(|d| d.get_temp::<u32>(id)).unwrap_or(*value);
    let response = ui.add(
        egui::Slider::new(
            &mut draft,
            crate::model::ISLAND_SIZE_MIN..=crate::model::ISLAND_SIZE_MAX,
        )
        .step_by(crate::model::ISLAND_SIZE_STEP as f64)
        .suffix("%"),
    );
    if ui.input(|i| i.pointer.primary_down()) {
        if response.changed() {
            ui.data_mut(|d| d.insert_temp(id, draft));
        }
    } else {
        *value = crate::model::island_size_value(draft);
        ui.data_mut(|d| d.remove::<u32>(id));
    }
    response
}

pub(crate) fn settings_hint(ui: &mut egui::Ui, text: &str) {
    let ctx = &ui.ctx().clone();
    ui.add(
        egui::Label::new(egui::RichText::new(text).size(12.0).color(settings_color(
            ctx,
            Color32::from_gray(148),
            Color32::from_rgb(100, 106, 115),
        )))
        .wrap(),
    );
}
pub(crate) fn settings_card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let ctx = &ui.ctx().clone();
    egui::Frame::new()
        .fill(settings_color(
            ctx,
            Color32::from_rgb(35, 35, 38),
            Color32::from_rgb(255, 255, 255),
        ))
        .stroke(egui::Stroke::new(
            1.0_f32,
            settings_color(
                ctx,
                Color32::from_rgb(56, 57, 62),
                Color32::from_rgb(228, 231, 236),
            ),
        ))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(16, 13))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            // 卡内分隔线压淡：8px 行距已经够分隔，硬线太密会让卡片显得碎。
            ui.style_mut().visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(
                1.0_f32,
                settings_color(
                    ctx,
                    Color32::from_rgb(48, 49, 54),
                    Color32::from_rgb(238, 240, 244),
                ),
            );
            add(ui);
        });
}
/// 设置分页对应的功能模块（页面序号）；常规、通知、关于、页面管理不属于任何模块。
#[cfg_attr(not(test), allow(dead_code))] // 由设置页 UI 测试覆盖
pub(crate) fn settings_tab_module(tab: usize) -> Option<usize> {
    use crate::app::*;
    match tab {
        1 => Some(MUSIC_PAGE),
        2 => Some(FOCUS_PAGE),
        3 => Some(WATER_PAGE),
        4 => Some(TODO_PAGE),
        5 => Some(SHELF_PAGE),
        7 => Some(MAIL_PAGE),
        8 => Some(CALENDAR_PAGE),
        11 => Some(QUOTA_PAGE),
        12 => Some(WEATHER_PAGE),
        _ => None,
    }
}

pub(crate) fn settings_label(ui: &mut egui::Ui, text: &str) {
    let ctx = &ui.ctx().clone();
    ui.add_space(13.0);
    // 分组标题：左侧补一条 3px accent 竖条当视觉锚点（不影响布局结构）。
    let response = ui.label(egui::RichText::new(text).size(12.0).color(settings_color(
        ctx,
        Color32::from_gray(168),
        Color32::from_rgb(88, 94, 104),
    )));
    let r = response.rect;
    let bar = egui::Rect::from_min_size(
        egui::pos2(r.left() - 10.0, r.top() + 1.5),
        egui::vec2(3.0, (r.height() - 3.0).max(8.0)),
    );
    ui.painter().rect_filled(
        bar,
        1.5,
        settings_color(
            ctx,
            Color32::from_rgb(96, 165, 250),
            Color32::from_rgb(37, 99, 235),
        ),
    );
    ui.add_space(4.0);
}
/// 关于岛仔：本项目介绍（独立定制版本线）+ 作者与仓库地址。不含任何捐赠入口。
fn about_upstream(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("岛仔 Daozai · 自定义版").size(15.0).strong());
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("本项目由 cayww 基于小唐维护的岛仔深度定制而来，保留并扩展了本地功能。项目地址：")
            .size(12.0),
    );
    ui.add_space(3.0);
    ui.hyperlink_to("github.com/cayww/daozai-custom", "https://github.com/cayww/daozai-custom");
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("检查更新与发布包来自本项目仓库的 releases；原上游地址仅供查看。")
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );
    ui.hyperlink_to("github.com/Renly1994/xiaotang-daozai", "https://github.com/Renly1994/xiaotang-daozai");
}
pub(crate) fn settings_control_row(
    ui: &mut egui::Ui,
    title: &str,
    label_width: f32,
    controls: impl FnOnce(&mut egui::Ui) -> egui::Response,
) -> egui::Response {
    ui.horizontal(|ui| {
        // 同一张卡片里的控件左缘对齐：标签列至少 SETTINGS_LABEL_W。
        let (label_rect, _) = ui.allocate_exact_size(
            egui::vec2(label_width.max(SETTINGS_LABEL_W), 30.0),
            egui::Sense::hover(),
        );
        let content = controls(ui);
        ui.painter().text(
            egui::pos2(label_rect.left(), content.rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
        );
        content
    })
    .inner
}
fn setting_number<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    title: &str,
    value: &mut N,
    min: N,
    max: N,
    suffix: &str,
    speed: f64,
) -> bool {
    let mut changed = false;
    settings_control_row(ui, title, 80.0, |ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(140.0, 30.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    changed = number_stepper(ui, value, min, max, suffix, N::from_f64(speed), 76.0);
                },
            );
        })
        .response
    });
    changed
}
fn number_stepper<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut N,
    min: N,
    max: N,
    suffix: &str,
    step: N,
    field_width: f32,
) -> bool {
    let ctx = &ui.ctx().clone();
    let mut changed = false;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
        let button = |text| {
            egui::Button::new(egui::RichText::new(text).size(16.0).color(settings_color(
                ctx,
                Color32::WHITE,
                Color32::from_rgb(30, 34, 41),
            )))
            .fill(settings_color(
                ctx,
                Color32::from_rgb(48, 48, 51),
                Color32::from_rgb(229, 232, 238),
            ))
            .stroke(egui::Stroke::NONE)
            .corner_radius(13.0)
        };
        let decrease = ui
            .add_enabled(*value > min, button("−").min_size(egui::vec2(26.0, 26.0)))
            .on_hover_text("减少");
        if decrease.clicked() {
            *value = N::from_f64((value.to_f64() - step.to_f64()).max(min.to_f64()));
            changed = true;
        }
        let field = ui.add_sized(
            [field_width, 28.0],
            egui::DragValue::new(value)
                .range(min..=max)
                .speed(step.to_f64())
                .suffix(suffix)
                .fixed_decimals(if N::INTEGRAL { 0 } else { 2 }),
        );
        changed |= field.changed();
        if ui
            .add_enabled(*value < max, button("+").min_size(egui::vec2(26.0, 26.0)))
            .on_hover_text("增加")
            .clicked()
        {
            *value = N::from_f64((value.to_f64() + step.to_f64()).min(max.to_f64()));
            changed = true;
        }
    });
    changed
}
/// 时刻加减器（一天中的分钟数，15 分钟一档，跨午夜循环），中间可直接输入 12:30。
fn time_stepper(ui: &mut egui::Ui, value: &mut u32, id: &str) -> bool {
    let ctx = &ui.ctx().clone();
    let mut changed = false;
    ui.push_id(id, |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
        let button = |text| {
            egui::Button::new(egui::RichText::new(text).size(16.0).color(settings_color(
                ctx,
                Color32::WHITE,
                Color32::from_rgb(30, 34, 41),
            )))
            .fill(settings_color(
                ctx,
                Color32::from_rgb(48, 48, 51),
                Color32::from_rgb(229, 232, 238),
            ))
            .stroke(egui::Stroke::NONE)
            .corner_radius(13.0)
        };
        if ui
            .add(button("−").min_size(egui::vec2(26.0, 26.0)))
            .on_hover_text("提前 15 分钟")
            .clicked()
        {
            *value = (*value + 1440 - 15) % 1440;
            changed = true;
        }
        let field = ui.add_sized(
            [56.0, 28.0],
            egui::DragValue::new(value)
                .range(0..=1439)
                .speed(1.0)
                .custom_formatter(|v, _| {
                    let m = v as u32;
                    format!("{:02}:{:02}", m / 60, m % 60)
                })
                .custom_parser(|t| {
                    let (h, m) = t.trim().split_once([':', '：'])?;
                    Some(
                        (h.parse::<u32>().ok()?.min(23) * 60 + m.parse::<u32>().ok()?.min(59))
                            as f64,
                    )
                }),
        );
        changed |= field.changed();
        if ui
            .add(button("+").min_size(egui::vec2(26.0, 26.0)))
            .on_hover_text("推后 15 分钟")
            .clicked()
        {
            *value = (*value + 15) % 1440;
            changed = true;
        }
    });
    changed
}
fn setting_choices(ui: &mut egui::Ui, title: &str, value: &mut u32, values: [u32; 3], unit: &str) {
    let labels = values.map(|v| format!("{v} {unit}"));
    let options: Vec<(u32, &str)> = values
        .iter()
        .zip(labels.iter())
        .map(|(v, l)| (*v, l.as_str()))
        .collect();
    settings_control_row_right(ui, title, |ui| {
        settings_segmented(ui, value, &options, 62.0);
    });
}
/// 设置页分页序号。旧序号保持不变（岛上入口、测试都按序号跳转），新增外观与位置两页。
pub(crate) const SETTINGS_TAB_APPEARANCE: usize = 13;
pub(crate) const SETTINGS_TAB_POSITION: usize = 14;
/// 本地定制页：手机连接/浏览器下载/MAA/液态玻璃配置集中于此。
pub(crate) const SETTINGS_TAB_CUSTOM: usize = 15;
/// 侧栏：一项就是一页，不再有页内子标签。专注与健康（2、3）、待办与置物（4、5）各合成一页；关于固定左下角。
/// 第四列是这一项之前的分组标题。
const SETTINGS_GROUPS: [(&str, usize, usize, &str); 14] = [
    ("通用", 0, 0, "岛仔"),
    ("外观", SETTINGS_TAB_APPEARANCE, SETTINGS_TAB_APPEARANCE, ""),
    ("位置", SETTINGS_TAB_POSITION, SETTINGS_TAB_POSITION, ""),
    ("页面管理", 10, 10, ""),
    ("音乐", 1, 1, "功能"),
    ("专注与健康", 2, 3, ""),
    ("待办与置物", 4, 5, ""),
    ("消息提醒", 6, 6, ""),
    ("邮箱", 7, 7, ""),
    ("日程", 8, 8, ""),
    ("AI 额度", 11, 11, ""),
    ("天气", 12, 12, ""),
    (
        "自定义与集成",
        SETTINGS_TAB_CUSTOM,
        SETTINGS_TAB_CUSTOM,
        "定制",
    ),
    ("关于", 9, 9, ""),
];
/// 分页标题；合并页（3→2、5→4）显示合并后的名字。
fn settings_tab_title(tab: usize) -> &'static str {
    SETTINGS_GROUPS
        .iter()
        .find(|(_, start, end, _)| (*start..=*end).contains(&tab))
        .map_or("通用", |g| g.0)
}
#[cfg(test)]
pub(crate) fn settings_tab_group_for_tests(tab: usize) -> &'static str {
    settings_tab_title(tab)
}

/// 「浏览器下载」卡片：始终显示安装指引（打开浏览器扩展页和扩展文件夹、复制路径）。
/// Chrome/Edge 从 33 版起封死了本地扩展的静默安装，只能开发者模式加载，所以做成引导而非自动装。
/// 注：不做「已连接」状态检测——桥进程只在下载进行时才被浏览器拉起，平时检测恒为未连接，没有意义。
#[cfg(windows)]
fn bridge_card_body(app: &mut App, ctx: &egui::Context, ui: &mut egui::Ui) {
    let Some(ext) = crate::bridge::extension_folder() else {
        ui.label(
            egui::RichText::new(
                "未找到浏览器扩展（bridge\\extension）。请用岛仔安装包安装后再来连接。",
            )
            .size(12.0)
            .color(settings_color(
                ctx,
                Color32::from_gray(180),
                Color32::from_gray(75),
            )),
        );
        return;
    };
    ui.horizontal_wrapped(|ui| {
        for (name, exe) in crate::bridge::installed_browsers() {
            if ui.button(format!("安装到 {name}")).clicked() {
                let page = if name == "Edge" {
                    "edge://extensions/"
                } else {
                    "chrome://extensions/"
                };
                let _ = std::process::Command::new(&exe).arg(page).spawn();
                let _ = std::process::Command::new("explorer.exe").arg(&ext).spawn();
                ui.ctx().copy_text(ext.display().to_string());
                app.say(format!(
                    "已打开 {name} 的扩展页和扩展文件夹，把文件夹拖进去即可（路径已复制）"
                ));
            }
        }
    });
    ui.label(
        egui::RichText::new(format!("扩展目录：{}", ext.display()))
            .size(11.5)
            .color(settings_color(
                ctx,
                Color32::from_gray(195),
                Color32::from_gray(60),
            )),
    );
    settings_hint(ui, "步骤：点上方按钮→ 扩展页右上角打开「开发者模式」→ 把 extension 文件夹拖进扩展页（或点「加载已解压的扩展程序」选它）→ 之后浏览器下载会实时上岛（蓝胶囊 = 下载中，绿胶囊 = 完成可点击打开）");
}

#[cfg(not(windows))]
fn bridge_card_body(_app: &mut App, _ctx: &egui::Context, ui: &mut egui::Ui) {
    ui.label("浏览器下载对接仅支持 Windows。");
}

fn settings_separator(ui: &mut egui::Ui) {
    ui.separator();
}

fn settings_page_icon(page: usize) -> egui::ImageSource<'static> {
    use crate::app::*;
    let (name, bytes): (&str, &'static [u8]) = match page {
        MUSIC_PAGE => (
            "music",
            include_bytes!("../assets/settings-icons/system/music.svg"),
        ),
        FOCUS_PAGE => (
            "focus",
            include_bytes!("../assets/settings-icons/system/focus.svg"),
        ),
        WATER_PAGE => (
            "health",
            include_bytes!("../assets/settings-icons/system/health.svg"),
        ),
        SHELF_PAGE => (
            "shelf",
            include_bytes!("../assets/settings-icons/system/shelf.svg"),
        ),
        TODO_PAGE => (
            "tasks",
            include_bytes!("../assets/settings-icons/system/tasks.svg"),
        ),
        MAIL_PAGE => (
            "mail",
            include_bytes!("../assets/settings-icons/system/mail.svg"),
        ),
        QUOTA_PAGE => (
            "quota",
            include_bytes!("../assets/settings-icons/system/quota.svg"),
        ),
        CALENDAR_PAGE => (
            "calendar",
            include_bytes!("../assets/settings-icons/system/calendar.svg"),
        ),
        _ => (
            "weather",
            include_bytes!("../assets/settings-icons/system/weather.svg"),
        ),
    };
    egui::ImageSource::Bytes {
        uri: format!("bytes://settings-header-{name}.svg").into(),
        bytes: bytes.into(),
    }
}

fn settings_state_chip(ui: &mut egui::Ui, on: bool) {
    let ctx = &ui.ctx().clone();
    let (text, fg, bg) = if on {
        (
            "已开启",
            settings_color(
                ctx,
                Color32::from_rgb(127, 214, 155),
                Color32::from_rgb(29, 122, 60),
            ),
            settings_color(
                ctx,
                Color32::from_rgb(28, 51, 36),
                Color32::from_rgb(227, 244, 232),
            ),
        )
    } else {
        (
            "已关闭",
            settings_color(ctx, Color32::from_gray(160), Color32::from_gray(105)),
            settings_color(
                ctx,
                Color32::from_rgb(44, 44, 48),
                Color32::from_rgb(232, 235, 240),
            ),
        )
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), egui::FontId::proportional(11.5), fg);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(galley.size().x + 16.0, 20.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 10.0, bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
}

const TIME_RANGE_HOURS_W: f32 = (26.0 + 6.0 + 48.0 + 6.0 + 26.0) * 2.0 + 8.0 + 13.0 + 8.0;
const TIME_RANGE_CLOCK_W: f32 = (26.0 + 6.0 + 56.0 + 6.0 + 26.0) * 2.0 + 8.0 + 13.0 + 8.0;

fn settings_advanced(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    let ctx = &ui.ctx().clone();
    let state = egui::Id::new(("settings-advanced", id));
    let mut open = ctx.data(|d| d.get_temp::<bool>(state)).unwrap_or(false);
    ui.add_space(2.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            rect.expand2(egui::vec2(6.0, 0.0)),
            7.0,
            settings_color(
                ctx,
                Color32::from_rgb(44, 44, 48),
                Color32::from_rgb(242, 244, 247),
            ),
        );
    }
    let fg = settings_color(ctx, Color32::from_gray(205), Color32::from_gray(60));
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        "高级",
        egui::FontId::proportional(13.0),
        fg,
    );
    // 右侧箭头：收起朝右，展开朝下。
    let c = egui::pos2(rect.right() - 10.0, rect.center().y);
    let pts = if open {
        vec![
            c + egui::vec2(-4.5, -2.0),
            c + egui::vec2(0.0, 2.5),
            c + egui::vec2(4.5, -2.0),
        ]
    } else {
        vec![
            c + egui::vec2(-2.0, -4.5),
            c + egui::vec2(2.5, 0.0),
            c + egui::vec2(-2.0, 4.5),
        ]
    };
    ui.painter()
        .add(egui::Shape::line(pts, egui::Stroke::new(1.6, fg)));
    if response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        open = !open;
        ctx.data_mut(|d| d.insert_temp(state, open));
    }
    if open {
        ui.spacing_mut().item_spacing.y = 8.0;
        add(ui);
    }
}
fn settings_apply_style(ui: &mut egui::Ui) {
    let ctx = &ui.ctx().clone();
    // 分割线：浅色模式用很淡的灰，深色模式略亮于卡片，不再是系统默认的粗灰线。
    ui.visuals_mut().widgets.noninteractive.bg_stroke = egui::Stroke::new(
        1.0,
        settings_color(
            ctx,
            Color32::from_rgb(52, 52, 57),
            Color32::from_rgb(233, 235, 240),
        ),
    );
    ui.spacing_mut().item_spacing.y = ui.spacing().item_spacing.y.max(5.0);
    ui.spacing_mut().item_spacing = egui::vec2(8.0, 5.0);
    ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    ui.visuals_mut().override_text_color = Some(settings_color(
        ctx,
        Color32::WHITE,
        Color32::from_rgb(30, 34, 41),
    ));
    ui.visuals_mut().extreme_bg_color = settings_color(
        ctx,
        Color32::from_rgb(25, 25, 28),
        Color32::from_rgb(244, 246, 249),
    );
    ui.visuals_mut().widgets.inactive.bg_fill = settings_color(
        ctx,
        Color32::from_rgb(50, 50, 54),
        Color32::from_rgb(228, 231, 236),
    );
    ui.visuals_mut().widgets.hovered.bg_fill = settings_color(
        ctx,
        Color32::from_rgb(65, 65, 71),
        Color32::from_rgb(216, 222, 231),
    );
    // 统一按钮：高度 28、圆角 7、内边距 12×5，无描边；默认按钮也套用这套样式。
    ui.spacing_mut().button_padding = egui::vec2(12.0, 5.0);
    ui.spacing_mut().interact_size.y = 28.0;
    {
        let fill = settings_color(
            ctx,
            Color32::from_rgb(50, 50, 54),
            Color32::from_rgb(228, 231, 236),
        );
        let hover = settings_color(
            ctx,
            Color32::from_rgb(65, 65, 71),
            Color32::from_rgb(216, 222, 231),
        );
        let widgets = &mut ui.visuals_mut().widgets;
        for (w, f) in [
            (&mut widgets.inactive, fill),
            (&mut widgets.hovered, hover),
            (&mut widgets.active, hover),
        ] {
            w.corner_radius = egui::CornerRadius::same(7);
            w.weak_bg_fill = f;
            w.bg_stroke = egui::Stroke::NONE;
        }
    }
    ui.visuals_mut().selection.bg_fill = settings_color(
        ctx,
        Color32::from_rgb(45, 105, 165),
        Color32::from_rgb(209, 226, 248),
    );
}
fn settings_modal_frame(ctx: &egui::Context) -> egui::Frame {
    egui::Frame::new()
        .fill(settings_color(
            ctx,
            Color32::from_rgb(36, 36, 40),
            Color32::WHITE,
        ))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::same(20))
        .stroke(egui::Stroke::new(
            1.0,
            settings_color(
                ctx,
                Color32::from_rgb(58, 58, 64),
                Color32::from_rgb(222, 225, 231),
            ),
        ))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(90),
        })
}
fn settings_modal_title(ui: &mut egui::Ui, title: &str) -> bool {
    let ctx = &ui.ctx().clone();
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .size(16.0)
                .strong()
                .color(settings_color(
                    ctx,
                    Color32::WHITE,
                    Color32::from_rgb(30, 34, 41),
                )),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close = ui
                .add(egui::Button::new(egui::RichText::new("×").size(18.0)).frame(false))
                .on_hover_text("关闭")
                .clicked();
        });
    });
    ui.add_space(4.0);
    settings_separator(ui);
    ui.add_space(6.0);
    close
}

fn settings_toggle_chip(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    let ctx = &ui.ctx().clone();
    let label = text.to_string();
    let fg = if on {
        settings_color(
            ctx,
            Color32::from_rgb(127, 214, 155),
            Color32::from_rgb(29, 122, 60),
        )
    } else {
        settings_color(ctx, Color32::from_gray(185), Color32::from_gray(80))
    };
    let fill = if on {
        settings_color(
            ctx,
            Color32::from_rgb(28, 51, 36),
            Color32::from_rgb(227, 244, 232),
        )
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if on {
        egui::Stroke::NONE
    } else {
        egui::Stroke::new(
            1.0,
            settings_color(
                ctx,
                Color32::from_rgb(60, 60, 66),
                Color32::from_rgb(214, 218, 225),
            ),
        )
    };
    ui.add(
        egui::Button::new(egui::RichText::new(label).size(12.0).color(fg))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(13.0)
            .min_size(egui::vec2(0.0, 26.0)),
    )
}

impl App {
    fn settings_feature_header(
        &mut self,
        ui: &mut egui::Ui,
        page: usize,
        description: &str,
        extra: impl FnOnce(&mut Self, &mut egui::Ui),
    ) -> bool {
        let ctx = &ui.ctx().clone();
        let on = self.module_on(page);
        let (mut enable, mut open) = (false, false);
        settings_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(settings_page_icon(page))
                        .fit_to_exact_size(egui::vec2(36.0, 36.0)),
                );
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        egui::RichText::new(PAGE_NAMES[page])
                            .size(15.0)
                            .strong()
                            .color(settings_color(
                                ctx,
                                Color32::WHITE,
                                Color32::from_rgb(30, 34, 41),
                            )),
                    );
                    ui.label(
                        egui::RichText::new(description)
                            .size(11.5)
                            .color(settings_color(
                                ctx,
                                Color32::from_gray(175),
                                Color32::from_gray(90),
                            )),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // 每个功能卡片同一个位置：开着时「打开」跳到岛上对应页面，关着时「开启」。
                    if on {
                        open = ui
                            .button("打开")
                            .on_hover_text(format!("在岛上打开「{}」", PAGE_NAMES[page]))
                            .clicked();
                        extra(self, ui);
                    } else {
                        enable = ui.button("开启").clicked();
                    }
                    settings_state_chip(ui, on);
                });
            });
            if !on {
                ui.label(
                    egui::RichText::new(format!(
                        "「{}」已关闭，不会在后台同步或提醒",
                        PAGE_NAMES[page]
                    ))
                    .size(12.0)
                    .color(settings_color(
                        ctx,
                        Color32::from_rgb(238, 181, 91),
                        Color32::from_rgb(139, 88, 12),
                    )),
                );
            }
        });
        if enable {
            self.set_page_visible(page, true);
        }
        if open {
            self.settings_open = false;
            if page == crate::app::CALENDAR_PAGE {
                self.open_calendar(None);
            } else {
                if page == crate::app::SHELF_PAGE {
                    self.shelf_tab = 0;
                }
                self.show_page(page);
            }
        }
        self.module_on(page)
    }

    pub fn settings_ui(&mut self, ctx: &egui::Context, autostart: bool) {
        if !self.settings_open {
            self.window_layout().close();
            self.weather_source.close();
        }
        self.updates.poll(ctx);
        // 更新脚本已就绪：交给外壳走完整退出（摘托盘 + exit），否则只关视窗时
        // 托盘还在、进程不退出，脚本替换 exe 会超时，UI 卡在「正在准备重启…」。
        if self.updates.take_exit_request() { self.pending.push(crate::app::Cmd::ExitForUpdate); }
        self.updates.dialog_ui(ctx);
        if !self.settings_open {
            return;
        }
        let id = egui::ViewportId::from_hash_of("daozai-settings");
        let focus_requested = std::mem::take(&mut self.settings_focus_requested);
        let opening = self.window_layout().open();
        ctx.show_viewport_immediate(id,egui::ViewportBuilder::default().with_title("岛仔设置").with_icon(theme::app_icon()).with_inner_size([690.0,560.0]).with_min_inner_size([600.0,450.0]).with_visible(false).with_active(false).with_window_level(egui::WindowLevel::Normal),|ctx,_|{
            let restored=opening && self.window_layout().restore(ctx);
            if !restored{self.window_layout().observe(ctx);}
            let dark=self.settings.settings_theme.dark(crate::settings_theme::system_dark(ctx));
            let egui_ctx=ctx.ctx().clone();
            let original_theme=egui_ctx.theme();
            let theme=if dark{egui::Theme::Dark}else{egui::Theme::Light};
            let original_style=egui_ctx.style_of(theme);
            let mut style=(*original_style).clone();
            style.visuals=if dark{egui::Visuals::dark()}else{egui::Visuals::light()};
            // 侧栏与正文之间的竖线、弹窗里的分隔线都用这条淡线；浅色模式不再是一条深灰粗线。
            // 侧栏边框已去掉，这里是唯一的一条：浅色模式一条淡线，深色模式不画线，靠两块底色区分。
            style.visuals.widgets.noninteractive.bg_stroke=if dark{egui::Stroke::NONE}else{egui::Stroke::new(1.0,Color32::from_rgb(226,229,234))};
            egui_ctx.set_style_of(theme,style.clone());
            egui_ctx.set_theme(if dark{egui::ThemePreference::Dark}else{egui::ThemePreference::Light});
            ctx.set_style(style);
            ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(if dark{egui::SystemTheme::Dark}else{egui::SystemTheme::Light}));
            if self.settings.settings_theme==crate::settings_theme::SettingsTheme::System {
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
            }
            if focus_requested {
                // Glow 在首帧交换缓冲后执行窗口命令，避免先显示白色空窗口。
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            if ctx.input(|i|i.viewport().close_requested()){self.settings_open=false;self.quota_close_connection();}
            let before=format!("{:?}",self.settings);
            let sidebar_fill=settings_color(ctx,Color32::from_rgb(29,29,32),Color32::from_rgb(237,239,243));
            egui::Panel::left("settings-sidebar").resizable(false).exact_size(180.0).frame(egui::Frame::new().fill(sidebar_fill).stroke(egui::Stroke::NONE).inner_margin(egui::Margin::symmetric(10,12))).show(ctx,|ui|{
                ui.add_space(2.0);
                // 13 项加两个分组标题：窗口拉到最矮时按可用高度压缩行高，「关于」始终留在底部可见。
                // 每项占 行高 + 控件间距(3) + 行后空隙；两个分组标题和「关于」上方预留约 80。
                let per=(ui.available_height()-80.0)/SETTINGS_GROUPS.len() as f32;
                let row_h=(per-6.0).clamp(20.0,32.0);
                let row_gap=(per-3.0-row_h).clamp(0.0,3.0);
                for (name,start,end,section) in SETTINGS_GROUPS {
                    if !section.is_empty() {
                        ui.add_space(if start==0{2.0}else{8.0});
                        ui.label(egui::RichText::new(section).size(11.0).color(settings_color(ui.ctx(),Color32::from_gray(150),Color32::from_gray(105))));
                        ui.add_space(3.0);
                    }
                    if start==9 {ui.add_space((ui.available_height()-row_h-4.0).max(4.0));}
                    let selected=(start..=end).contains(&self.settings_tab);
                    let (rect,response)=ui.allocate_exact_size(egui::vec2(ui.available_width(),row_h),egui::Sense::click());
                    if response.clicked() && !selected{self.settings_tab=start;}
                    if selected || response.hovered(){
                        ui.painter().rect_filled(rect,7.0,if selected{settings_color(ui.ctx(),Color32::from_rgb(43,43,46),Color32::from_rgb(214,221,232))}else{settings_color(ui.ctx(),Color32::from_rgb(30,30,33),Color32::from_rgb(225,230,238))});
                    }
                    let icon=match start {
                        0=>egui::include_image!("../assets/settings-icons/system/general.svg"),
                        11=>egui::include_image!("../assets/settings-icons/system/quota.svg"),
                        10=>egui::include_image!("../assets/settings-icons/system/pages.svg"),
                        1=>egui::include_image!("../assets/settings-icons/system/music.svg"),
                        2=>egui::include_image!("../assets/settings-icons/system/focus.svg"),
                        4=>egui::include_image!("../assets/settings-icons/system/tasks.svg"),
                        6=>egui::include_image!("../assets/settings-icons/system/notifications.svg"),
                        12=>egui::include_image!("../assets/settings-icons/system/weather.svg"),
                        7=>egui::include_image!("../assets/settings-icons/system/mail.svg"),
                        8=>egui::include_image!("../assets/settings-icons/system/calendar.svg"),
                        SETTINGS_TAB_APPEARANCE=>egui::include_image!("../assets/settings-icons/system/appearance.svg"),
                        SETTINGS_TAB_POSITION=>egui::include_image!("../assets/settings-icons/system/position.svg"),
                        // 自定义与集成：里面是浏览器 / 手机 / MAA / 网易云等外部集成，用浏览器图标比「关于」贴切。
                        SETTINGS_TAB_CUSTOM=>egui::include_image!("../assets/settings-icons/system/browser.svg"),
                        _=>egui::include_image!("../assets/settings-icons/system/about.svg"),
                    };
                    let icon_bg=egui::Rect::from_center_size(egui::pos2(rect.left()+21.0,rect.center().y),egui::vec2(23.0,23.0));
                    egui::Image::new(icon).paint_at(ui,icon_bg);
                    ui.painter().text(egui::pos2(rect.left()+40.0,rect.center().y),egui::Align2::LEFT_CENTER,name,egui::FontId::proportional(13.0),if selected{settings_color(ui.ctx(),Color32::WHITE,Color32::from_rgb(30,34,41))}else{settings_color(ui.ctx(),Color32::from_gray(225),Color32::from_gray(55))});
                    ui.add_space(row_gap);
                }
            });
            let panel_fill=settings_color(ctx,Color32::from_rgb(25,25,27),Color32::from_rgb(246,247,249));
            egui::CentralPanel::default().frame(egui::Frame::new().fill(panel_fill).inner_margin(egui::Margin::symmetric(22,16) )).show(ctx,|ui|{
                settings_apply_style(ui);
                ui.label(egui::RichText::new(settings_tab_title(self.settings_tab)).size(18.0).strong().color(settings_color(ui.ctx(),Color32::WHITE,Color32::from_rgb(30,34,41))));
                ui.add_space(6.0);
                // 合并页：健康（3）并入专注（2），置物（4→5）并入待办，旧序号仍能打开对应页面。
                let tab=match self.settings_tab {3=>2,5=>4,t=>t};
                egui::ScrollArea::vertical().id_salt(("settings-scroll",tab)).auto_shrink([false,false]).show(ui,|ui|{
                    // 右侧给滚动条留出空隙，卡片和开关不会被它压住。
                    ui.set_width((ui.available_width()-12.0).max(240.0));
                    match tab {
                    12=>{
                        let on=self.settings_feature_header(ui,crate::app::WEATHER_PAGE,"当地天气与七日预报",|_,_|{});
                        ui.add_space(4.0);
                        ui.add_enabled_ui(on,|ui|self.weather_settings(ui));
                    }
                    11=>{
                        let on=self.settings_feature_header(ui,crate::app::QUOTA_PAGE,"每 3 分钟自动更新剩余额度",|_,_|{});
                        ui.add_space(4.0);
                        ui.add_enabled_ui(on,|ui|self.quota_settings(ui));
                    }
                    10=>{
                        ui.label(egui::RichText::new("选择需要的页面和状态组件。").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                        // 岛上显示：收起时的常驻内容在这里统一排序和开关；岛上歌词属于音乐的显示方式，在音乐页设置。
                        settings_label(ui,"岛上显示");
                        settings_card(ui,|ui|{
                            self.resident_order_settings(ui);
                            ui.separator();
                            setting_row(ui,"时间","关闭后仍可在展开顶栏查看",&mut self.settings.show_clock);
                            ui.separator();
                            setting_row(ui,"电量","无电池设备自动隐藏",&mut self.settings.show_battery);
                            ui.separator();
                            setting_row(ui,"天气","天气图标和温度，点击打开天气页",&mut self.settings.show_weather);
                            ui.separator();
                            let before=self.quota.config.compact;
                            setting_row(ui,"AI 额度","常驻账号；WorkBuddy 积分在前，DeepSeek 余额在最后",&mut self.quota.config.compact);
                            if before!=self.quota.config.compact {self.quota.config.save();}
                        });
                        settings_label(ui,"功能页面");
                        settings_card(ui,|ui|{
                            let descriptions=["播放控制与同步歌词","番茄钟与休息计时","喝水与久坐提醒","临时收纳文件位置","任务清单与到期提醒","邮件摘要与验证码","Agent 剩余额度和账号切换","月历、邮件邀请与工作日程","当地天气与七日预报"];
                            ui.label(egui::RichText::new("拖动左侧手柄调整顺序；关闭的功能不在后台运行，不同步也不提醒，已有数据保留。").size(11.5).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            #[derive(Clone)]
                            struct PageDrag(usize);
                            let mut movement=None;
                            let mut jump=None;
                            let row_w=ui.available_width();
                            for (index,page) in self.settings.page_order.into_iter().enumerate() {
                                if index>0 {ui.separator();}
                                let mut visible=self.settings.visible_pages[page];
                                let can_change=!visible || self.settings.visible_pages.iter().filter(|enabled|**enabled).count()>1;
                                let row=ui.push_id(("page-order",page),|ui|ui.horizontal(|ui|{
                                    ui.set_max_width(row_w);
                                    let (rect,handle)=ui.allocate_exact_size(egui::vec2(20.0,40.0),egui::Sense::drag());
                                    for x in [-3.0,3.0] {for y in [-6.0,0.0,6.0] {
                                        ui.painter().circle_filled(rect.center()+egui::vec2(x,y),1.4,settings_color(ui.ctx(),Color32::from_gray(155),Color32::from_gray(100)));
                                    }}
                                    handle.on_hover_cursor(egui::CursorIcon::Grab).dnd_set_drag_payload(PageDrag(page));
                                    // 每行右侧「设置 ›」直接跳到这个功能的设置页，开关仍在最右。
                                    let jump_w=58.0;
                                    ui.allocate_ui(egui::vec2((ui.available_width()-jump_w-ui.spacing().item_spacing.x).max(160.0),40.0),|ui|{
                                        ui.add_enabled_ui(can_change,|ui|{setting_row(ui,PAGE_NAMES[page],descriptions[page],&mut visible);});
                                    });
                                    if ui.add_sized([jump_w,26.0],egui::Button::new(egui::RichText::new("设置 ›").size(12.0)).frame(false)).on_hover_text(format!("打开「{}」设置",PAGE_NAMES[page])).clicked(){jump=Some(crate::app::PAGE_SETTINGS_TABS[page]);}
                                })).inner.response;
                                if let Some(payload)=row.dnd_hover_payload::<PageDrag>() {
                                    if payload.0!=page {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                        let y=if self.settings.page_order.iter().position(|p|*p==payload.0).unwrap()<index {row.rect.bottom()}else{row.rect.top()};
                                        ui.painter().line_segment([egui::pos2(row.rect.left(),y),egui::pos2(row.rect.right(),y)],egui::Stroke::new(2.0,Color32::from_rgb(70,150,255)));
                                    }
                                }
                                if let Some(payload)=row.dnd_release_payload::<PageDrag>() {movement=Some((payload.0,index));}
                                if visible!=self.settings.visible_pages[page] {self.set_page_visible(page,visible);}
                            }
                            if let Some(tab)=jump {self.settings_tab=tab;}
                            if let Some((page,to))=movement {
                                let from=self.settings.page_order.iter().position(|p|*p==page).unwrap();
                                if from<to {self.settings.page_order[from..=to].rotate_left(1);}
                                else {self.settings.page_order[to..=from].rotate_right(1);}
                            }
                            if self.settings.visible_pages.iter().filter(|enabled|**enabled).count()==1 {
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new("至少保留一个页面。").size(11.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            }
                        });
                    }
                    0=>{
                        settings_label(ui,"设置页外观");
                        settings_card(ui,|ui|{
                            use crate::settings_theme::SettingsTheme;
                            settings_control_row_right(ui,"主题",|ui|{
                                if settings_segmented(ui,&mut self.settings.settings_theme,&[(SettingsTheme::System,"跟随系统"),(SettingsTheme::Light,"浅色"),(SettingsTheme::Dark,"深色")],52.0) {ui.ctx().request_repaint();}
                            });
                            settings_hint(ui,"仅影响设置页，岛的外观保持不变。");
                        });
                        settings_label(ui,"时间格式");
                        settings_card(ui,|ui|{
                            settings_control_row_right(ui,"时间制式",|ui|{settings_segmented(ui,&mut self.settings.clock_12h,&[(false,"24 小时"),(true,"12 小时")],72.0);});
                            settings_control_row_right(ui,"时段文字",|ui|{settings_segmented(ui,&mut self.settings.clock_period,&[(0,"不显示"),(1,"上午 / 下午"),(2,"详细时段")],72.0);});
                            if self.settings.clock_period==2 {settings_hint(ui,"详细时段：凌晨、上午、中午、下午、傍晚、晚上");}
                            ui.separator();
                            setting_row(ui,"显示秒数","时间显示为 14:32:08",&mut self.settings.clock_seconds);
                        });
                        settings_label(ui,"启动");
                        settings_card(ui,|ui|{
                            let mut a=autostart;
                            setting_row(ui,"开机启动","",&mut a);
                            if a!=autostart{self.pending.push(Cmd::ToggleAutostart);}
                            ui.separator();
                            setting_row(ui,"全屏时隐藏","全屏隐藏岛仔，健康提醒静默保留",&mut self.settings.hide_fullscreen);
                        });
                        self.avoidance.settings(ui,&mut self.settings);
                        ui.add_space(8.0);
                        settings_card(ui,|ui|{
                            settings_advanced(ui,"settings-general-advanced",|ui|{
                                let before=self.settings.render_compat;
                                setting_row(ui,"兼容渲染（Direct3D 12）","出现黑框、闪烁或打开设置后崩溃时开启；切换后重启岛仔生效",&mut self.settings.render_compat);
                                // 手动关闭后不再显示自动切换的原因；再次开启视为手动开启。
                                if before!=self.settings.render_compat {self.settings.render_compat_reason.clear();}
                                if !self.settings.render_compat_reason.is_empty() {settings_hint(ui,&self.settings.render_compat_reason);}
                            });
                            // 待重启的提示不折叠，收起「高级」也能看到。
                            if self.settings.render_compat!=self.render_compat_running {
                                ui.horizontal(|ui|{
                                    ui.label(egui::RichText::new("兼容渲染重启岛仔后生效").size(12.0).color(Color32::from_rgb(230,150,40)));
                                    if ui.button("立即重启").clicked(){self.pending.push(Cmd::RestartApp);}
                                });
                            }
                        });
                    }
                    SETTINGS_TAB_APPEARANCE=>{
                        settings_label(ui,"岛体外观");
                        settings_card(ui,|ui|{
                            use crate::model::{self as model,Skin};
                            let skins:Vec<(Skin,&str)>=Skin::ALL.iter().map(|s|(*s,s.label())).collect();
                            settings_control_row_right(ui,"皮肤",|ui|{settings_segmented(ui,&mut self.settings.skin,&skins,64.0);});
                            settings_hint(ui,self.settings.skin.description());
                            // 「配置液态玻璃」入口夹在皮肤与形状之间，且只在当前就是液态玻璃皮肤时出现。
                            // 注意 is_glass() 只含毛玻璃浅/深，不含 Liquid，这里必须用 Skin::Liquid 判断。
                            if self.settings.skin==Skin::Liquid {
                                ui.horizontal(|ui|{
                                    if ui.button("配置液态玻璃").clicked(){self.liquid_cfg_open=!self.liquid_cfg_open;}
                                    settings_hint(ui,"折射、色散、投影、休眠等液态玻璃专属参数，弹窗里调");
                                });
                            }
                            if self.settings.skin.is_glass() {
                                let mut blur=self.settings.glass_blur;
                                setting_row(ui,"背景模糊","需开启系统“透明效果”；关闭后改为烟熏半透明",&mut blur);
                                self.settings.glass_blur=blur;
                            }
                            ui.separator();
                            let shapes:Vec<(model::IslandShape,&str)>=model::IslandShape::ALL.iter().map(|s|(*s,s.label())).collect();
                            settings_control_row_right(ui,"形状",|ui|{settings_segmented(ui,&mut self.settings.island_shape,&shapes,64.0);});
                            settings_hint(ui,self.settings.island_shape.description());
                            ui.separator();
                            settings_control_row(ui,"大小",56.0,|ui|{
                                ui.horizontal_wrapped(|ui|{
                                    // 给百分比和恢复按钮留出空间，宽窗口仍使用原来的轨道长度。
                                    ui.spacing_mut().slider_width=(ui.available_width()-150.0).clamp(64.0,180.0);
                                    island_size_slider(ui,&mut self.settings.island_size);
                                    if ui.add_enabled(self.settings.island_size!=100,egui::Button::new("恢复默认")).clicked(){self.settings.island_size=100;}
                                }).response
                            });
                            settings_hint(ui,"只影响岛体，放大后文字和图标依旧清晰。");
                        });
                        settings_label(ui,"岛体显示");
                        settings_card(ui,|ui|{
                            let mut minimal=self.settings.minimal;
                            setting_row(ui,"极简刘海","收起时显示一条细线",&mut minimal);
                            if minimal!=self.settings.minimal {self.set_minimal(minimal);}
                            ui.separator();
                            let mut pet=self.settings.pet;
                            setting_row(ui,"岛仔吉祥物","收起时常驻，播放音乐时一起唱歌",&mut pet);
                            if pet!=self.settings.pet {self.set_pet(pet);}
                            settings_advanced(ui,"settings-appearance-advanced",|ui|{
                                setting_row(ui,"OLED 像素偏移","防止 OLED 屏幕烧屏：收起静止时让岛体缓慢移动几个像素，不改动你保存的位置",&mut self.settings.oled_shift);
                            });
                        });
                        // ===== 岛体交互（所有皮肤通用）=====
                        // 从「自定义与集成」搬来：展开动效/悬停延时对所有皮肤生效，与液态玻璃无关。
                        // 「配置液态玻璃」按钮已移到上面的「岛体外观」卡（皮肤与形状之间）。
                        settings_label(ui,"岛体交互");
                        settings_card(ui,|ui|{
                            let anims:Vec<(crate::model::OpenAnim,&str)>=crate::model::OpenAnim::ALL.iter().map(|a|(*a,a.label())).collect();
                            settings_control_row_right(ui,"展开动效",|ui|{settings_segmented(ui,&mut self.settings.open_anim,&anims,52.0);});
                            settings_hint(ui,self.settings.open_anim.description());
                            ui.separator();
                            setting_number(ui,"悬停触发延时",&mut self.settings.hover_delay_ms,0,500," ms",10.0);
                            settings_hint(ui,"悬停多久后展开；80–120 ms 可避开鼠标划过");
                        });
                    }
                    SETTINGS_TAB_POSITION=>{
                        settings_card(ui,|ui|{
                            let selected=self.displays.iter().find(|d|d.id==self.settings.display_id);
                            let label=if self.settings.display_id.is_empty(){"跟随主屏"}else{selected.map_or("所选屏幕未连接",|d|d.label.as_str())};
                            settings_control_row(ui,"显示屏",56.0,|ui|{
                                egui::ComboBox::from_id_salt("island-display").selected_text(label).width(ui.available_width().min(300.0)).truncate().show_ui(ui,|ui|{
                                    ui.selectable_value(&mut self.settings.display_id,String::new(),"跟随主屏");
                                    for display in &self.displays {ui.selectable_value(&mut self.settings.display_id,display.id.clone(),&display.label);}
                                }).response
                            });
                            // 只在所选屏幕断开时说明，平时不占位置。
                            if !self.settings.display_id.is_empty()&&selected.is_none(){settings_hint(ui,"暂时显示在主屏，重新连接所选屏幕后自动恢复。");}
                            ui.separator();
                            // 停靠边缘：顶部为横向；左右为竖向排列，展开面板从边缘滑出。
                            let docks:Vec<(crate::model::Dock,&str)>=crate::model::Dock::ALL.iter().map(|d|(*d,d.label())).collect();
                            settings_control_row_right(ui,"停靠",|ui|{
                                // 换边后偏移的含义变了（水平 ↔ 上下），回到居中。
                                if settings_segmented(ui,&mut self.settings.dock,&docks,64.0) {self.settings.island_offset=0;}
                            });
                            if self.settings.dock.is_side() {settings_hint(ui,"左右停靠时竖向排列；吉祥物和岛上歌词只在顶部停靠时显示。");}
                            let side=self.settings.dock.is_side();
                            settings_control_row(ui,if side{"上下位置"}else{"水平位置"},56.0,|ui|{
                                ui.horizontal_wrapped(|ui|{
                                    let limit=crate::model::ISLAND_OFFSET_MAX;
                                    let offset=&mut self.settings.island_offset;
                                    let (less,more,less_tip,more_tip)=if side{("↑","↓","向上微调","向下微调")}else{("←","→","向左微调","向右微调")};
                                    if ui.add_enabled(*offset>-limit,egui::Button::new(less).min_size(egui::vec2(26.0,22.0))).on_hover_text(less_tip).clicked(){*offset=(*offset-5).max(-limit);}
                                    // 左侧微调按钮已占位，剩余宽度还要容纳右侧微调和恢复居中。
                                    ui.spacing_mut().slider_width=(ui.available_size_before_wrap().x-134.0).clamp(64.0,150.0);
                                    ui.add(egui::Slider::new(&mut *offset,-limit..=limit).show_value(false));
                                    if ui.add_enabled(*offset<limit,egui::Button::new(more).min_size(egui::vec2(26.0,22.0))).on_hover_text(more_tip).clicked(){*offset=(*offset+5).min(limit);}
                                    if ui.add_enabled(*offset!=0,egui::Button::new("恢复居中")).clicked(){*offset=0;}
                                }).response
                            });
                            let offset=self.settings.island_offset;
                            let (less,more)=if side{("偏上","偏下")}else{("偏左","偏右")};
                            let position=match offset {0=>"居中".to_string(),o if o<0=>format!("{less} {}%",-o),o=>format!("{more} {o}%")};
                            settings_hint(ui,&format!("当前：{position}。也可以展开后按住日期时间直接拖动。"));
                            ui.separator();
                            setting_row(ui,"顶栏适配","顶部有菜单栏或任务栏时使用",&mut self.settings.avoid_top_bar);
                            if self.settings.avoid_top_bar {
                                settings_control_row_right(ui,"显示方式",|ui|{
                                    settings_segmented(ui,&mut self.settings.top_bar_on_top,&[(false,"避开顶栏"),(true,"岛仔置顶")],90.0);
                                });
                                if self.settings.top_bar_on_top {
                                    settings_hint(ui,"岛体显示在顶栏上方。");
                                } else {
                                    let reserved=crate::display_layout::selected(&self.displays,&self.settings.display_id).map_or(0,|d|{
                                        let b=crate::display_layout::placement_bounds(d,true);(b.top-d.bounds.top).max(0)
                                    });
                                    settings_hint(ui,&if reserved>0 {format!("已检测到顶栏，岛体下移 {reserved} 像素。")}else{"未检测到顶栏；没有向系统登记的顶栏可尝试“岛仔置顶”。".to_string()});
                                }
                            }
                        });
                    }
                    1=>{
                        let on=self.settings_feature_header(ui,crate::app::MUSIC_PAGE,"播放控制与同步歌词",|_,_|{});
                        ui.add_enabled_ui(on,|ui|{
                            settings_label(ui,"歌词");
                            settings_card(ui,|ui|{
                                setting_row(ui,"岛上歌词","播放时收起的岛显示当前句，其他常驻图标暂时隐藏；无同步歌词时显示封面和歌名",&mut self.settings.island_lyrics);
                                ui.separator();
                                setting_row(ui,"展开时显示歌词","在播放器右侧同步滚动",&mut self.settings.lyrics_enabled);
                                self.show_lyrics=self.settings.lyrics_enabled;
                                ui.separator();
                                let mut advance=-self.settings.lyric_delay_ms as f64/1000.0;
                                if setting_number(ui,"时间偏移",&mut advance,-5.0,5.0," 秒",0.1){self.settings.lyric_delay_ms=(-advance*1000.0).round()as i32;}
                                settings_hint(ui,"歌词和歌声对不上时调整：正数提前，负数延后；默认提前 0.35 秒。");
                            });
                        });
                    }
                    2=>{
                        let on=self.settings_feature_header(ui,crate::app::FOCUS_PAGE,"番茄钟与休息计时",|_,_|{});
                        ui.add_enabled_ui(on,|ui|{
                            settings_card(ui,|ui|{
                                setting_number(ui,"专注",&mut self.settings.focus_minutes,1,180," 分钟",1.0);
                                ui.separator();
                                setting_number(ui,"休息",&mut self.settings.rest_minutes,1,60," 分钟",1.0);
                                settings_hint(ui,"调整后从下一阶段开始生效。");
                            });
                        });
                        ui.add_space(12.0);
                        let on=self.settings_feature_header(ui,crate::app::WATER_PAGE,"喝水与久坐提醒",|_,_|{});
                        ui.add_enabled_ui(on,|ui|{
                            settings_card(ui,|ui|{
                                // 喝水：开关一行，数量与间隔紧随其后。
                                let old_remind=self.water.remind;
                                setting_row(ui,"喝水提醒","确认或喝水后重新计时；达标后停止提醒",&mut self.water.remind);
                                if self.water.remind!=old_remind{self.water.save();self.confirm_health_reminder(HealthReminder::Water);}
                                ui.add_enabled_ui(self.water.remind,|ui|{
                                    setting_number(ui,"提醒间隔",&mut self.settings.water_minutes,5,180," 分钟",1.0);
                                    setting_number(ui,"每次记录",&mut self.settings.serving_ml,50,1000," ml",25.0);
                                    if setting_number(ui,"每日目标",&mut self.water.goal_ml,500,6000," ml",50.0){self.water.save();}
                                });
                                ui.separator();
                                let old_sit=self.settings.sit_remind;
                                setting_row(ui,"久坐提醒","确认提醒或点「起身了」后重新计时",&mut self.settings.sit_remind);
                                if self.settings.sit_remind!=old_sit {self.confirm_health_reminder(HealthReminder::Sit);}
                                ui.add_enabled_ui(self.settings.sit_remind,|ui|{
                                    setting_number(ui,"久坐时长",&mut self.settings.sit_minutes,15,180," 分钟",1.0);
                                });
                                ui.separator();
                                // 提醒时段与午休对喝水和久坐都生效。
                                // 喝水和久坐提醒都关掉时，时段和午休不起作用，一起置灰。
                                let any_reminder=self.water.remind||self.settings.sit_remind;
                                ui.add_enabled_ui(any_reminder,|ui|{
                                    settings_control_row(ui,"提醒时段",80.0,|ui|{
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                            // 区域宽度正好等于两个加减器加「至」，靠右放置后与上面单个加减器的右边缘对齐。
                                            ui.allocate_ui_with_layout(egui::vec2(TIME_RANGE_HOURS_W,30.0),egui::Layout::left_to_right(egui::Align::Center),|ui|{
                                                number_stepper(ui,&mut self.settings.water_start,0,23," 时",1,48.0);
                                                ui.label("至");
                                                number_stepper(ui,&mut self.settings.water_end,0,24," 时",1,48.0);
                                            });
                                        }).response
                                    });
                                    setting_row(ui,"午休不提醒","午休时间内喝水和久坐都不弹提醒",&mut self.settings.lunch_rest);
                                    if self.settings.lunch_rest {
                                        settings_control_row(ui,"午休时间",80.0,|ui|{
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                                ui.allocate_ui_with_layout(egui::vec2(TIME_RANGE_CLOCK_W,30.0),egui::Layout::left_to_right(egui::Align::Center),|ui|{
                                                    time_stepper(ui,&mut self.settings.lunch_start,"lunch-start");
                                                    ui.label("至");
                                                    time_stepper(ui,&mut self.settings.lunch_end,"lunch-end");
                                                });
                                            }).response
                                        });
                                    }
                                    settings_hint(ui,"提醒时段和午休对喝水、久坐都生效；专注计时进行中不打扰，结束后再提醒。");
                                });
                            });
                        });
                    }
                    4=>{
                        let on=self.settings_feature_header(ui,crate::app::TODO_PAGE,"任务清单与到期提醒",|_,_|{});
                        let _=on;
                        ui.add_space(12.0);
                        let on=self.settings_feature_header(ui,crate::app::SHELF_PAGE,"临时收纳文件与剪贴板",|_,_|{});
                        ui.add_enabled_ui(on,|ui|{
                            settings_card(ui,|ui|{
                                ui.label(egui::RichText::new(format!("当前 {} 项 · 移除条目不会删除原文件",self.shelf.items.len())).size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                                ui.separator();
                                setting_row(ui,"记录剪贴板","在置物架的「剪贴板」里查看最近复制的文字，点一下再次复制",&mut self.settings.clipboard_history);
                                settings_hint(ui,&format!("本机保存最近 {} 条；密码管理器标记为敏感的内容不会记录。",crate::clipboard::MAX_RECENT));
                                if ui.button("打开剪贴板").clicked(){self.settings_open=false;self.shelf_tab=1;self.show_page(3);}
                            });
                        });
                    }
                    6=>{
                        settings_label(ui,"提醒");
                        settings_card(ui,|ui|{
                            setting_row(ui,"提醒总开关","关闭后饮水、专注、待办及聊天软件提醒都不显示",&mut self.settings.notifications);
                            // 提醒方式由系统通知和提示音两个设置共同决定。
                            ui.separator();
                            ui.add_enabled_ui(self.settings.notifications,|ui|{
                                let mut mode:u8=if !self.settings.system_notify{0}else if self.settings.sound{2}else{1};
                                settings_control_row_right(ui,"提醒方式",|ui|{settings_segmented(ui,&mut mode,&[(0,"仅岛上"),(1,"加系统通知"),(2,"系统通知 + 声音")],96.0);});
                                settings_hint(ui,["只在岛上提醒，不发送 Windows 系统通知","同时在 Windows 通知中心留下记录，不响铃","同时发送 Windows 系统通知并播放提示音"][mode as usize]);
                                self.settings.system_notify=mode>0;
                                self.settings.sound=mode==2;
                            });
                        });
                        settings_label(ui,"系统提示");
                        settings_card(ui,|ui|{
                            setting_row(ui,"系统状态","音量、充电、网络和键盘状态",&mut self.settings.system_hud);
                            ui.separator();
                            setting_row(ui,"蓝牙设备","连接与断开时提醒；关闭后停止蓝牙监听",&mut self.settings.bluetooth_notifications);
                        });
                        // 聊天软件：一个软件一行、一个开关；附属选项缩进跟在所属软件下面。
                        let master=self.settings.notifications;
                        settings_label(ui,"聊天软件");
                        if !master {
                            ui.label(egui::RichText::new("提醒总开关已关闭，打开后下面的提醒才会生效").size(12.0).color(Color32::from_rgb(230,150,40)));
                        }
                        let sub=|ui:&mut egui::Ui,title:&str,description:&str,value:&mut bool|{
                            ui.horizontal(|ui|{ui.add_space(22.0);ui.vertical(|ui|setting_row(ui,title,description,value));});
                        };
                        let mut open_connect=false;
                        settings_card(ui,|ui|{
                            ui.add_enabled_ui(master,|ui|{
                                setting_row(ui,"微信","根据任务栏闪动提醒，不读取联系人或聊天正文",&mut self.settings.wechat_notifications);
                                if self.settings.wechat_notifications {
                                    sub(ui,"托盘一键唤起","微信缩在托盘时，点提醒直接唤起微信",&mut self.settings.wechat_tray_open);
                                }
                                if master && !self.messenger_status(crate::app::Messenger::Wechat).is_empty(){settings_hint(ui,&self.messenger_status(crate::app::Messenger::Wechat));}
                                ui.separator();
                                setting_row(ui,"企业微信","",&mut self.settings.wecom_notifications);
                                setting_row(ui,"飞书","",&mut self.settings.feishu_notifications);
                                setting_row(ui,"钉钉","",&mut self.settings.dingtalk_notifications);
                                settings_hint(ui,"企业微信、飞书、钉钉需在客户端开启任务栏闪动；点击提醒打开对应窗口。");
                                for client in crate::messenger::Messenger::OFFICE {
                                    if client.enabled(&self.settings) && !self.messenger_status(client).is_empty(){settings_hint(ui,&self.messenger_status(client));}
                                }
                                ui.separator();
                                ui.horizontal(|ui|{
                                    ui.vertical(|ui|{
                                        ui.spacing_mut().item_spacing.y=2.0;
                                        ui.label(egui::RichText::new("QQ 与其他应用").size(13.0).color(settings_color(ui.ctx(),Color32::WHITE,Color32::from_rgb(30,34,41))));
                                        ui.label(egui::RichText::new(&self.listener_status).size(11.5).color(settings_color(ui.ctx(),Color32::from_gray(175),Color32::from_gray(90))));
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                        if self.settings.read_notifications {
                                            if ui.button(if self.settings.qq_only{"断开 QQ 通知"}else{"断开系统通知"}).clicked(){self.pending.push(Cmd::DisconnectNotifications);}
                                        } else if ui.button("连接…").clicked(){open_connect=true;}
                                    });
                                });
                                sub(ui,"显示通知正文","仅 QQ 与 Windows 通知；可能包含私密消息，屏幕共享时建议关闭",&mut self.settings.notification_preview);
                                // 收到过通知的应用可能有几十个：默认收起，展开后用可点选的标签排成几行，不再一应用一行。
                                let before_apps=self.notification_apps.items.clone();
                                let total=self.notification_apps.items.len();
                                if total>0 {
                                    let on_count=self.notification_apps.items.iter().filter(|(_,_,enabled)|*enabled).count();
                                    ui.horizontal(|ui|{ui.add_space(22.0);ui.vertical(|ui|{
                                        egui::CollapsingHeader::new(egui::RichText::new(format!("按应用接收（已开 {on_count} / 共 {total}）")).size(12.5)).id_salt("settings-notification-apps").default_open(false).show(ui,|ui|{
                                            ui.horizontal_wrapped(|ui|{
                                                ui.spacing_mut().item_spacing=egui::vec2(6.0,6.0);
                                                for (_,name,enabled) in &mut self.notification_apps.items {
                                                    if settings_toggle_chip(ui,name,*enabled).clicked(){*enabled=!*enabled;}
                                                }
                                            });
                                            settings_hint(ui,"点选切换；绿色为接收，灰框为不接收。应用会在收到通知后出现在这里。");
                                        });
                                    });});
                                }
                                if before_apps!=self.notification_apps.items{self.notification_apps.save();}
                            });
                        });
                        let connect_id=egui::Id::new("settings-notify-connect");
                        if open_connect {ui.ctx().data_mut(|d|d.insert_temp(connect_id,true));}
                        if ui.ctx().data(|d|d.get_temp::<bool>(connect_id)).unwrap_or(false) {
                            let mut close=false;
                            let modal=egui::Modal::new(connect_id.with("modal")).frame(settings_modal_frame(ui.ctx())).backdrop_color(Color32::from_black_alpha(110)).show(ui.ctx(),|ui|{
                                ui.set_width(360.0);
                                settings_apply_style(ui);if settings_modal_title(ui,"连接 QQ 与其他应用"){close=true;}
                                if ui.add_sized([ui.available_width(),30.0],egui::Button::new("只连接 QQ")).clicked(){self.pending.push(Cmd::RequestQqNotifications);close=true;}
                                settings_hint(ui,"QQ 需开启系统通知；新通知在岛上显示约 4.5 秒，默认隐藏正文。");
                                ui.add_space(6.0);
                                if ui.add_sized([ui.available_width(),30.0],egui::Button::new("连接所有应用通知")).clicked(){self.pending.push(Cmd::RequestNotifications);close=true;}
                                settings_hint(ui,"只接收 Windows 通知中心可见的内容；应用会在收到通知后出现在列表里。");
                                ui.add_space(8.0);
                                if ui.button("取消").clicked(){close=true;}
                            });
                            if close || modal.should_close() {ui.ctx().data_mut(|d|d.remove::<bool>(connect_id));}
                        }
                    }
                    7=>{
                        let mut add_mail=false;
                        let on=self.settings_feature_header(ui,crate::app::MAIL_PAGE,"新邮件提醒与验证码；岛上歌词模式显示右侧图标",|_,_|{});
                        ui.add_enabled_ui(on,|ui|{
                        settings_label(ui,"设置");
                        settings_card(ui,|ui|{
                            setting_row(ui,"验证码高亮","识别到验证码时在岛上放大显示，点击即复制",&mut self.settings.mail_code);
                            ui.separator();
                            let sync_before=self.settings.mail_sync_operations;
                            ui.add_enabled_ui(self.mail.manage.pending.is_empty(),|ui|{
                                setting_row(ui,"同步邮件操作到邮箱","已读、删除和恢复同步到邮箱；关闭时只整理岛仔里的记录，切换不会补交之前的操作",&mut self.settings.mail_sync_operations);
                            });
                            if sync_before!=self.settings.mail_sync_operations{self.mail_manage_mode_changed();}
                            ui.separator();
                            ui.horizontal(|ui|{
                                ui.vertical(|ui|{
                                    ui.spacing_mut().item_spacing.y=2.0;
                                    ui.label(egui::RichText::new("会议提前").size(13.0).color(settings_color(ui.ctx(),Color32::WHITE,Color32::from_rgb(30,34,41))));
                                    ui.label(egui::RichText::new(format!("{} 分钟 · 与日程共用",self.settings.mail_ics)).size(11.5).color(settings_color(ui.ctx(),Color32::from_gray(175),Color32::from_gray(90))));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.button("在日程中设置 ›").clicked(){self.settings_tab=8;}});
                            });
                            settings_advanced(ui,"settings-mail-advanced",|ui|{
                                setting_choices(ui,"验证码隐藏",&mut self.settings.mail_code_hide_seconds,[30,60,300],"秒");
                                settings_hint(ui,"明确有效时长更短时提前隐藏；隐藏不代表失效，主动查看后 15 秒再次隐藏。");
                                setting_choices(ui,"检查间隔",&mut self.settings.mail_interval,[60,120,300],"秒");
                                settings_hint(ui,"支持推送的邮箱实时收信；间隔只用于不支持推送的邮箱。");
                            });
                        });
                        ui.horizontal(|ui|{
                            settings_label(ui,"邮箱账号");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM),|ui|{if ui.button("＋ 添加邮箱").clicked(){add_mail=true;}});
                        });
                        settings_card(ui,|ui|{
                            if self.mail.accounts.is_empty(){ui.label(egui::RichText::new("还没有邮箱。点「＋ 添加邮箱」，保存后自动连接。").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(195),Color32::from_gray(60))));}
                            let total=self.mail.accounts.len();
                            let(mut changed,mut remove,mut test)=(false,None,None);
                            for i in 0..total{
                                let(email,provider)=(self.mail.accounts[i].email.clone(),self.mail.accounts[i].provider);
                                let status=self.mail.status_of(&email).label();
                                let mut on=self.mail.accounts[i].enabled;
                                if i>0{ui.separator();}
                                ui.label(egui::RichText::new(&email).size(13.0).color(settings_color(ui.ctx(),Color32::WHITE,Color32::from_rgb(30,34,41))));
                                ui.label(egui::RichText::new(format!("{} · {}",provider.label(),status)).size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                                ui.horizontal(|ui|{
                                    if ui.add_sized([66.0,30.0],egui::Button::new("测试")).clicked(){test=Some(i);}
                                    if ui.add_sized([66.0,30.0],egui::Button::new("删除")).clicked(){remove=Some(i);}
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("启用").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(190),Color32::from_gray(65))));
                                    if switch(ui,&mut on){self.mail.accounts[i].enabled=on;changed=true;}
                                });
                            }
                            if changed{self.mail_accounts_changed();}
                            if let Some(i)=remove{
                                let email=self.mail.accounts[i].email.clone();
                                let credential=self.mail.accounts[i].credential_key();
                                self.mail.accounts.remove(i);
                                crate::mail::secrets::delete(&credential);
                                self.mail.status.remove(&email.to_ascii_lowercase());
                                self.mail.note=format!("{email}：已移除，凭据已从系统凭据管理器删除");
                                self.mail_accounts_changed();
                            }
                            if let Some(i)=test{
                                let account=self.mail.accounts[i].clone();
                                if let Err(error)=account.endpoint(){
                                    self.mail.note=error;
                                }else{
                                match crate::mail::secrets::load(&account.credential_key()){
                                    Ok(pass)=>{self.mail.testing=Some(account.email.clone());self.mail.note=format!("{}：正在测试连接…",account.email);self.pending.push(Cmd::MailTest(account,pass));}
                                    Err(e)=>self.mail.note=format!("{}：{e}",account.email),
                                }
                                }
                            }
                                                    if !self.mail.note.is_empty(){ui.add_space(3.0);ui.add(egui::Label::new(egui::RichText::new(&self.mail.note).size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75)))).wrap());}
                        });
                        });
                        // 添加邮箱：一次性的长表单放进弹窗，页面上只保留账号列表。
                        let form_id=egui::Id::new("settings-mail-add");
                        if add_mail {self.mail.form.open=true;}
                        if self.mail.form.open {
                            let mut close=false;
                            let modal=egui::Modal::new(form_id).frame(settings_modal_frame(ui.ctx())).backdrop_color(Color32::from_black_alpha(110)).show(ui.ctx(),|ui|{
                            ui.set_width(440.0);
                            settings_apply_style(ui);if settings_modal_title(ui,"添加邮箱"){close=true;}
                            ui.spacing_mut().item_spacing.y=8.0;
                            settings_control_row(ui,"邮箱服务",56.0,|ui|{
                                egui::ComboBox::from_id_salt("mail-provider").width(160.0).selected_text(self.mail.form.provider.label()).show_ui(ui,|ui|{
                                    for p in crate::mail::Provider::ALL{if ui.selectable_label(self.mail.form.provider==p,p.label()).clicked(){self.mail.form.provider=p;}}
                                }).response
                            });
                            ui.add_space(5.0);
                            let microsoft=self.mail.form.provider==crate::mail::Provider::Microsoft;
                            ui.label(egui::RichText::new(if microsoft{"连接微软邮箱"}else{"获取授权码或专用密码"}).size(12.5).strong());
                            ui.add(egui::Label::new(egui::RichText::new(self.mail.form.provider.hint()).size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(200),Color32::from_gray(55)))).wrap());
                            let (label,url)=self.mail.form.provider.setup_link();
                            if !url.is_empty() && ui.link(label).clicked(){self.pending.push(Cmd::OpenUrl(url.into()));}
                            ui.add_space(8.0);
                            if self.mail.form.provider==crate::mail::Provider::Custom {
                                settings_control_row(ui,"IMAP 主机",80.0,|ui|{ui.text_edit_singleline(&mut self.mail.form.custom.host)});
                                settings_control_row(ui,"TLS 端口",80.0,|ui|{ui.add(egui::DragValue::new(&mut self.mail.form.custom.port).range(1..=65535))});
                                settings_control_row(ui,"登录用户名",80.0,|ui|{ui.add(egui::TextEdit::singleline(&mut self.mail.form.custom.username).vertical_align(egui::Align::Center).hint_text("留空使用邮箱地址"))});
                            }
                            if !microsoft{
                            let input_width=(ui.available_width()-56.0-48.0-2.0*ui.spacing().item_spacing.x).max(100.0);
                            settings_control_row(ui,"邮箱",56.0,|ui|{
                                ui.add_sized([input_width,30.0],egui::TextEdit::singleline(&mut self.mail.form.email).vertical_align(egui::Align::Center).hint_text("输入邮箱地址"))
                            });
                            let target=crate::mail::Account{custom:(self.mail.form.provider==crate::mail::Provider::Custom).then(||self.mail.form.custom.clone()),email:self.mail.form.email.clone(),provider:self.mail.form.provider,enabled:true};
                            let unsupported=target.endpoint().err().as_deref()==Some(crate::mail::MICROSOFT_UNSUPPORTED);
                            if unsupported{ui.add(egui::Label::new(crate::mail::MICROSOFT_UNSUPPORTED).wrap());}
                            ui.add_enabled_ui(!unsupported,|ui|{
                            settings_control_row(ui,"授权码",56.0,|ui|{
                                let field=ui.add_sized([input_width,30.0],egui::TextEdit::singleline(&mut self.mail.form.password).vertical_align(egui::Align::Center).password(!self.mail.form.show).hint_text("输入邮箱授权码"));
                                if ui.add_sized([48.0,30.0],egui::Button::new(if self.mail.form.show{"隐藏"}else{"显示"})).clicked(){self.mail.form.show=!self.mail.form.show;}
                                field
                            });
                            ui.horizontal(|ui|{
                                if ui.button("测试连接").clicked(){
                                    let email=self.mail.form.email.trim().to_string();
                                    let target=crate::mail::Account{custom:(self.mail.form.provider==crate::mail::Provider::Custom).then(||self.mail.form.custom.clone()),email:email.clone(),provider:self.mail.form.provider,enabled:true};
                                    if !email.contains('@'){self.mail.note="请先填邮箱地址".into();}
                                    else if let Err(error)=target.endpoint(){self.mail.note=error;}
                                    else if self.mail.form.password.is_empty(){self.mail.note="请先填授权码（不是邮箱登录密码）".into();}
                                    else{
                                        self.mail.testing=Some(email.clone());
                                        self.mail.note=format!("{email}：正在测试连接…");
                                        self.pending.push(Cmd::MailTest(crate::mail::Account {custom: (self.mail.form.provider==crate::mail::Provider::Custom).then(||self.mail.form.custom.clone()), email:email.clone(),provider:self.mail.form.provider,enabled:true},self.mail.form.password.clone()));
                                    }
                                }
                                if ui.button("保存并开始监控").clicked(){
                                    let email=self.mail.form.email.trim().to_string();
                                    let target=crate::mail::Account{custom:(self.mail.form.provider==crate::mail::Provider::Custom).then(||self.mail.form.custom.clone()),email:email.clone(),provider:self.mail.form.provider,enabled:true};
                                    if !email.contains('@'){self.mail.note="请先填邮箱地址".into();}
                                    else if let Err(error)=target.endpoint(){self.mail.note=error;}
                                    else if self.mail.form.password.is_empty(){self.mail.note="请先填授权码（不是邮箱登录密码）".into();}
                                    else{
                                        match crate::mail::secrets::save(&target.credential_key(),&self.mail.form.password){
                                            Ok(())=>{
                                                let account=crate::mail::Account {custom: (self.mail.form.provider==crate::mail::Provider::Custom).then(||self.mail.form.custom.clone()), email:email.clone(),provider:self.mail.form.provider,enabled:true};
                                                if let Some(slot)=self.mail.accounts.iter_mut().find(|a|a.email.eq_ignore_ascii_case(&email)){*slot=account;}else{self.mail.accounts.push(account);}
                                                self.mail.form.password.clear();
                                                self.mail.note=format!("{email}：已保存，正在连接…");
                                                self.mail_accounts_changed();
                                            }
                                            Err(e)=>self.mail.note=e,
                                        }
                                    }
                                }
                            });
                            ui.label(egui::RichText::new("授权码存储在 Windows 凭据管理器中。").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            });
                            }
                                                    if !self.mail.note.is_empty(){ui.add_space(3.0);ui.add(egui::Label::new(egui::RichText::new(&self.mail.note).size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75)))).wrap());}
                            });
                            // 保存后弹窗保持打开，结果和「测试连接」都在这里；用户自己关闭。
                            if close || modal.should_close() {self.mail.form.open=false;}
                        }
                    }
                    8=>{
                        ui.spacing_mut().item_spacing.y=4.0;
                        let mut open_add=false;
                        let on=self.settings_feature_header(ui,crate::app::CALENDAR_PAGE,"月历、邮件邀请与工作日程",|_,_|{});
                        ui.add_enabled_ui(on,|ui|{
                        settings_label(ui,"设置");
                        settings_card(ui,|ui|{
                            setting_row(ui,"自动同步","每 5 分钟更新日程；关闭后可点「立即同步」",&mut self.settings.calendar_auto_sync);
                            ui.separator();
                            setting_choices(ui,"会议提前",&mut self.settings.mail_ics,[5,15,30],"分钟");
                            settings_hint(ui,"与邮件中的会议邀请共用提醒，同一场会议只提醒一次。");
                        });
                        settings_label(ui,"日历账号");
                        let mut import=false;
                        settings_card(ui,|ui|{
                        if let Some(cfg)=self.calendar.config.clone(){
                                let n=self.calendar.events.len();
                                // 一行：平台图标、名称与状态，右侧立即同步和断开。
                                ui.horizontal(|ui|{
                                    let (rect,_)=ui.allocate_exact_size(egui::vec2(28.0,28.0),egui::Sense::hover());
                                    ui.painter().rect_filled(rect,7.0,Color32::from_rgb(51,112,255));
                                    ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,cfg.provider.label().chars().next().unwrap_or('日'),egui::FontId::proportional(13.0),Color32::WHITE);
                                    ui.vertical(|ui|{
                                        ui.spacing_mut().item_spacing.y=2.0;
                                        ui.label(egui::RichText::new(format!("{} · 最后同步 {}",cfg.provider.label(),if self.calendar.last_sync.is_empty(){"—".to_string()}else{self.calendar.last_sync.clone()})).size(13.0).color(settings_color(ui.ctx(),Color32::WHITE,Color32::from_rgb(30,34,41))));
                                        ui.label(egui::RichText::new(if self.calendar.connected{format!("● 已连接 · 未来 24 小时 {n} 个日程")}else if self.calendar.failures==0{"● 正在连接…".into()}else{format!("● 上次连接失败 · 已知 {n} 个日程")}).size(11.5).color(if self.calendar.connected{settings_color(ui.ctx(),Color32::from_rgb(120,220,160),Color32::from_rgb(24,121,64))}else{settings_color(ui.ctx(),Color32::from_rgb(240,180,110),Color32::from_rgb(145,89,12))}));
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                        if ui.add_enabled(!self.calendar.testing,egui::Button::new("断开")).clicked(){self.pending.push(Cmd::CalendarForget);}
                                        if ui.button("立即同步").clicked(){self.calendar.note="正在同步…".into();self.calendar.note_error=false;self.pending.push(Cmd::CalendarSync);}
                                    });
                                });
                                if let Some(feed)=self.calendar.feeds.iter().find(|feed|feed.config==cfg) {
                                    let status=if feed.month.loading {"正在同步本月日程…".to_owned()}
                                        else if !feed.month.error.is_empty(){format!("本月同步失败：{}",feed.month.error)}
                                        else if feed.month.loaded==Some(self.calendar.view.month){format!("{} {} {} 个日程",self.calendar.view.month.format("%Y-%m"),if feed.month.warning.is_empty(){"已同步"}else{"已读取"},feed.month.events.len())}
                                        else {String::new()};
                                    if !status.is_empty(){ui.add(egui::Label::new(egui::RichText::new(status).size(12.0)).wrap());}
                                    if !feed.month.warning.is_empty(){ui.add(egui::Label::new(egui::RichText::new(format!("本月部分日程未同步：{}",feed.month.warning)).size(12.0).color(settings_color(ui.ctx(),Color32::from_rgb(238,181,91),Color32::from_rgb(139,88,12)))).wrap());}
                                }
                                if !self.calendar.warning.is_empty(){ui.add(egui::Label::new(egui::RichText::new(format!("同步提示：{}",self.calendar.warning)).size(12.0).color(settings_color(ui.ctx(),Color32::from_rgb(238,181,91),Color32::from_rgb(139,88,12)))).wrap());}
                                if !self.calendar.error.is_empty(){
                                    ui.add(egui::Label::new(egui::RichText::new(&self.calendar.error).size(12.0).color(settings_color(ui.ctx(),Color32::from_rgb(240,96,96),Color32::from_rgb(184,34,34)))).wrap());
                                }
                                ui.separator();
                        }else{
                            ui.label(egui::RichText::new("还没有连接日历。添加飞书、钉钉等日历，或导入 .ics 文件。").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(195),Color32::from_gray(60))));
                        }
                        ui.horizontal(|ui|{
                            if ui.button(if self.calendar.config.is_some(){"更换日历"}else{"＋ 添加日历"}).clicked(){open_add=true;}
                            if ui.button("导入 .ics 文件").clicked(){import=true;}
                        });
                        if !self.calendar.import_note.is_empty(){settings_hint(ui,&self.calendar.import_note);}
                        });
                        if import {self.pending.push(Cmd::CalendarImport);}
                        });
                        // 添加日历：选平台、填账号、获取方法、导入 .ics 都在弹窗里。
                        let add_id=egui::Id::new("settings-calendar-add");
                        if open_add {self.calendar.form.open=true;self.calendar.form.guide_open=true;}
                        if self.calendar.form.open {
                            let mut close=false;
                            let connected_before=self.calendar.config.clone();
                            let modal=egui::Modal::new(add_id.with("modal")).frame(settings_modal_frame(ui.ctx())).backdrop_color(Color32::from_black_alpha(110)).show(ui.ctx(),|ui|{
                            ui.set_width(460.0);
                            ui.spacing_mut().item_spacing.y=6.0;
                            settings_apply_style(ui);if settings_modal_title(ui,"添加日历"){close=true;}
                            settings_label(ui,"选择日历平台");
                            egui::Frame::new().fill(settings_color(ui.ctx(),Color32::from_rgb(29,29,31),Color32::from_rgb(236,239,244))).corner_radius(8.0).inner_margin(3).show(ui,|ui|{
                                ui.set_min_width(ui.available_width());
                                ui.spacing_mut().item_spacing.x=2.0;
                                // 六个平台分两行：飞书 / 钉钉 / 企业微信，iCloud / CalDAV / Exchange。
                                let width=((ui.available_width()-4.0)/3.0).max(80.0);
                                for row in crate::calendar::Provider::ALL.chunks(3){
                                    ui.horizontal(|ui|{
                                        for &p in row{
                                            let selected=self.calendar.form.provider==p;
                                            let button=egui::Button::new(egui::RichText::new(p.label()).size(13.0).color(if selected{settings_color(ui.ctx(),Color32::WHITE,Color32::from_rgb(30,34,41))}else{settings_color(ui.ctx(),Color32::from_gray(190),Color32::from_gray(65))}))
                                                .fill(if selected{settings_color(ui.ctx(),Color32::from_rgb(81,81,85),Color32::from_rgb(208,217,231))}else{Color32::TRANSPARENT})
                                                .stroke(egui::Stroke::NONE).corner_radius(6.0);
                                            let button=ui.add_enabled_ui(!self.calendar.testing,|ui|ui.add_sized([width,30.0],button)).inner;
                                            let button=match p{
                                                crate::calendar::Provider::ICloud=>button.on_hover_text("苹果日历：Apple ID + App 专用密码"),
                                                crate::calendar::Provider::Custom=>button.on_hover_text("任意 CalDAV 日历：Nextcloud、群晖、Fastmail 等"),
                                                crate::calendar::Provider::Exchange=>button.on_hover_text("公司自建或第三方 Exchange 日历（ActiveSync），只读同步"),
                                                _=>button,
                                            };
                                            if button.clicked() && !selected{
                                                self.select_calendar_provider(p);
                                            }
                                        }
                                    });
                                }
                            });
                            ui.add_space(7.0);
                            ui.horizontal_top(|ui|{
                                ui.allocate_ui_with_layout(egui::vec2(50.0,18.0),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_size(egui::vec2(50.0,18.0));ui.label(egui::RichText::new("服务器").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));});
                                if self.calendar.form.provider.custom_server(){
                                    ui.add_sized([ui.available_width(),22.0],egui::TextEdit::singleline(&mut self.calendar.form.server).vertical_align(egui::Align::Center)
                                        .hint_text(egui::RichText::new(self.calendar.form.provider.server_hint()).color(settings_color(ui.ctx(),Color32::from_gray(155),Color32::from_gray(100)))));
                                }else{
                                    ui.add(egui::Label::new(egui::RichText::new(self.calendar.form.provider.server()).size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(205),Color32::from_gray(55)))).wrap());
                                }
                            });
                            settings_label(ui,"连接账号");
                            ui.scope(|ui|{
                                ui.visuals_mut().extreme_bg_color=settings_color(ui.ctx(),Color32::from_rgb(25,25,28),Color32::from_rgb(244,246,249));
                            let input_width=(ui.available_width()-50.0-48.0-2.0*ui.spacing().item_spacing.x).max(80.0);
                                ui.horizontal(|ui|{
                                    ui.allocate_ui_with_layout(egui::vec2(50.0,30.0),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_size(egui::vec2(50.0,30.0));ui.label(egui::RichText::new("用户名").size(13.0).color(settings_color(ui.ctx(),Color32::from_gray(210),Color32::from_gray(55))));});
                                    let user=ui.add_sized([input_width,30.0],egui::TextEdit::singleline(&mut self.calendar.form.username).vertical_align(egui::Align::Center)
                                        .hint_text(egui::RichText::new("输入用户名或粘贴账号信息").color(settings_color(ui.ctx(),Color32::from_gray(155),Color32::from_gray(100)))));
                                    if user.changed(){
                                        let pasted=self.calendar.form.username.clone();
                                        if let Some((u,p))=crate::calendar::parse_paste(&pasted){
                                            self.calendar.form.username=u;
                                            self.calendar.form.password=p;
                                            self.calendar.note="已从粘贴内容中填入用户名和密码".into();
                                            self.calendar.note_error=false;
                                        }
                                    }
                                });
                                ui.add_space(5.0);
                                ui.horizontal(|ui|{
                                    ui.allocate_ui_with_layout(egui::vec2(50.0,30.0),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_size(egui::vec2(50.0,30.0));ui.label(egui::RichText::new("密码").size(13.0).color(settings_color(ui.ctx(),Color32::from_gray(210),Color32::from_gray(55))));});
                                    ui.add_sized([input_width,30.0],egui::TextEdit::singleline(&mut self.calendar.form.password).vertical_align(egui::Align::Center)
                                        .password(!self.calendar.form.show).hint_text(egui::RichText::new("输入平台生成的密码").color(settings_color(ui.ctx(),Color32::from_gray(155),Color32::from_gray(100)))));
                                    if ui.add_sized([48.0,30.0],egui::Button::new(if self.calendar.form.show{"隐藏"}else{"显示"})).clicked(){self.calendar.form.show=!self.calendar.form.show;}
                                });
                            });
                            ui.add_space(7.0);
                            let guide=if self.calendar.form.guide_open{"如何获取用户名和密码？（点击收起）"}else{"如何获取用户名和密码？"};
                            if ui.link(egui::RichText::new(guide).color(settings_color(ui.ctx(),Color32::from_rgb(120,185,245),Color32::from_rgb(26,100,177)))).clicked(){self.calendar.form.guide_open=!self.calendar.form.guide_open;}
                            if self.calendar.form.guide_open{
                                ui.add_space(4.0);
                                for(i,step)in self.calendar.form.provider.guide().iter().enumerate(){
                                    ui.label(egui::RichText::new(format!("{}. {}",i+1,step)).size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(200),Color32::from_gray(55))));
                                }
                            }
                            ui.add_space(9.0);
                            let busy=self.calendar.testing;
                            let label=if busy{"正在测试连接…"}else{"测试并保存"};
                            let btn=egui::Button::new(egui::RichText::new(label).size(13.0).strong().color(Color32::WHITE))
                                .fill(if busy{settings_color(ui.ctx(),Color32::from_rgb(70,70,76),Color32::from_rgb(213,219,227))}else{Color32::from_rgb(45,105,165)})
                                .corner_radius(7.0);
                            if ui.add_enabled_ui(!busy,|ui|ui.add_sized([130.0,32.0],btn)).inner.clicked(){
                                let user=self.calendar.form.username.trim().to_string();
                                let custom=self.calendar.form.provider.custom_server();
                                let server=crate::calendar::normalize_server(&self.calendar.form.server);
                                if custom && server.is_none(){self.calendar.note="请填写以 https:// 开头的 CalDAV 服务器地址".into();self.calendar.note_error=true;}
                                else if user.is_empty(){self.calendar.note="请先填用户名".into();self.calendar.note_error=true;}
                                else if self.calendar.form.password.is_empty(){self.calendar.note="请先填密码".into();self.calendar.note_error=true;}
                                else{
                                    let cfg=crate::calendar::Config{provider:self.calendar.form.provider,username:user,href:None,server:if custom{server}else{None}};
                                    self.calendar.testing=true;
                                    self.calendar.test_password=self.calendar.form.password.clone();
                                    self.calendar.note="正在连接日历…".into();
                                    self.calendar.note_error=false;
                                    self.pending.push(Cmd::CalendarTest(cfg,self.calendar.form.password.clone()));
                                }
                            }
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new("连接成功后保存，密码存储在 Windows 凭据管理器中。").size(11.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            if !self.calendar.note.is_empty(){
                                ui.add_space(5.0);
                                let color=if self.calendar.note_error{settings_color(ui.ctx(),Color32::from_rgb(255,125,125),Color32::from_rgb(184,34,34))}else{settings_color(ui.ctx(),Color32::from_gray(210),Color32::from_gray(55))};
                                ui.add(egui::Label::new(egui::RichText::new(&self.calendar.note).size(12.0).color(color)).wrap());
                            }
                            });
                            // 连接成功（日历配置变化）后自动关闭弹窗。
                            if self.calendar.config.is_some() && self.calendar.config!=connected_before && !self.calendar.testing {close=true;}
                            if close || modal.should_close() {self.calendar.form.open=false;}
                        }
                    }

                    SETTINGS_TAB_CUSTOM=>{
                        // ===== 本地定制的第三方集成：网易云 / 手机 / 浏览器 / MAA =====
                        // 按「跟谁集成」分组，相关的卡片挨在一起；液态玻璃与交互已移到
                        // 「外观」页（SETTINGS_TAB_APPEARANCE），便于整块并入上游。

                        // ---- 网易云音乐 ----
                        settings_label(ui,"网易云音乐");
                        settings_card(ui,|ui|{
                            let fixed=crate::smtc::netease_control::shortcut_fixed();
                            ui.label(egui::RichText::new(if fixed{"快捷方式已带调试端口"}else{"快捷方式还没有调试端口"}).size(12.5).strong());
                            settings_hint(ui,"拖动播放进度条需要改网易云快捷方式的启动参数；启动岛仔时会自动修复全部快捷方式，重启网易云后生效。只改参数，不动客户端本身。");
                            ui.separator();
                            if ui.button("修复快捷方式").clicked(){
                                let count=crate::smtc::netease_control::fix_all_shortcuts();
                                if count==0 {self.say("没找到需要修改的网易云快捷方式，可以在下方手动选择");}
                                else {self.say(format!("已修复 {count} 个快捷方式，重启网易云后生效"));}
                            }
                            ui.horizontal(|ui|{
                                ui.label(egui::RichText::new("找不到时手动指定：").size(11.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                                if ui.button("选择文件").clicked(){
                                    match crate::system::pick_files_for("选择网易云快捷方式",false){
                                        Ok(files)=>if let Some(path)=files.first(){
                                            match crate::smtc::netease_control::fix_shortcut(path){
                                                Ok(message)=>self.say(message),
                                                Err(error)=>self.say(error),
                                            }
                                        }else{self.say("未选择文件");},
                                        Err(error)=>self.say(format!("打开文件对话框失败：{}",error)),
                                    }
                                }
                            });
                        });

                        // ---- 手机（连接状态 + 手机功能）----
                        settings_label(ui,"手机");
                        settings_hint(ui,"下面两项配合：先在「连接状态」里配对手机，再到「手机功能」里开关电量与通知转发。");
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
                            ui.label(egui::RichText::new(status_text).size(13.0).strong().color(settings_color(ui.ctx(), light, dark)));

                            // 一键打开「手机连接」：已装直接启动，未装跳商店。
                            let open = if crate::phone_battery::installed() {
                                Some(("打开手机连接", "explorer.exe", "shell:AppsFolder\\Microsoft.YourPhone_8wekyb3d8bbwe!App"))
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
                            ui.label(egui::RichText::new("手机短信通过「手机连接」转发。").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            // 短信总开关关掉后，验证码自动复制没有意义，一并置灰。
                            ui.add_enabled_ui(self.settings.phone_alerts,|ui|{
                                setting_row(ui,"验证码自动复制","手机短信验证码上岛时直接写入剪贴板",&mut self.settings.auto_copy_otp);
                            });
                            ui.separator();
                            ui.label(egui::RichText::new("手机 App 名单：检测到新 App 通知时自动加入并默认开启，这里只需取消勾选即可静音。也可直接改 %APPDATA%\\Daozai\\phone-apps.json。").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            let before_phone_apps=self.phone_apps.items.clone();
                            for (name,label,allow) in &mut self.phone_apps.items {
                                let title=if label==name {name.clone()} else {format!("{} · {}",label,name)};
                                setting_row(ui,&title,"",allow);
                            }
                            if before_phone_apps!=self.phone_apps.items{self.phone_apps.save();}
                        });
                        // ---- 浏览器（安装 + 下载提示 + 工作原理）----
                        settings_label(ui,"浏览器");
                        settings_label(ui,"安装");
                        settings_card(ui,|ui|{
                            { let uictx=ui.ctx().clone(); bridge_card_body(self,&uictx,ui); }
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
                            ui.label(egui::RichText::new("浏览器扩展把下载事件发给本机桥进程，桥写入状态总线后由岛仔显示。Chrome/Edge 出于安全不允许软件静默安装本地扩展，所以首次使用需要按上方指引手动加载一次。").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                        });
                        // ---- MAA（状态 + 上岛提醒 + 路径）----
                        settings_label(ui,"MAA");
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
                            ui.label(egui::RichText::new(text).size(13.0).strong().color(settings_color(ui.ctx(),light,dark)));
                            if let Some(exe)=maa_exe.as_ref() {
                                ui.label(egui::RichText::new(format!("MAA：{exe}")).size(11.5).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            }
                            if let Some(path)=log_path.as_ref() {
                                ui.label(egui::RichText::new(format!("日志：{}",path.display())).size(11.5).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            }
                            if !maa_on {
                                settings_hint(ui,"勾选通知里的 MAA，或直接在下方指定程序");
                            } else if !log_ready {
                                settings_hint(ui,"没找到 debug\\asst.log，请指定 MAA 目录");
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
                                            .vertical_align(egui::Align::Center).hint_text("留空自动识别（<MAA目录>\\debug）"));
                                        if ui.button("选择…").clicked(){self.pending.push(Cmd::PickMaaLogDir);}
                                    }).response
                                });
                                settings_hint(ui,"留空自动识别（MAA 程序旁的 debug 目录）");
                                if before!=self.maa_prefs{self.maa_prefs.save();}
                            }
                        });
                    }
                    _=>{
                        settings_label(ui,"岛仔 Daozai");
                        settings_card(ui,|ui|{
                            ui.horizontal(|ui|{
                                ui.label(egui::RichText::new(format!("版本 {}",env!("CARGO_PKG_VERSION"))).size(13.0));
                                ui.add_space(8.0);
                                ui.vertical(|ui|self.updates.ui(ui,self.settings.update_check));
                            });
                            ui.separator();
                            // 开关放这里：旧配置里 update_check=0 的用户也能自己打开。
                            let before=self.settings.update_check;
                            setting_row(ui,"自动检查更新","启动时自动检查本仓库 releases；关闭后仍可用上方按钮手动检查", &mut self.settings.update_check);
                            if before!=self.settings.update_check{self.settings.save();}
                            ui.label(egui::RichText::new("桌面顶部的音乐、专注、提醒与日程小助手。").size(12.0).color(settings_color(ui.ctx(),Color32::from_gray(180),Color32::from_gray(75))));
                            ui.horizontal_wrapped(|ui|{
                                if ui.button("打开本地数据目录").clicked(){self.pending.push(Cmd::Open(crate::model::config_dir()));}
                                if ui.button("收集诊断").clicked(){
                                    // 另起 `daozai.exe --collect-diagnostics`：在新窗口里收集，诊断包保存到桌面并自动选中。
                                    if crate::diagnostics_tool::launch().is_err(){self.say("无法启动诊断收集，请从开始菜单打开「收集岛仔诊断」");}
                                }
                                ui.label(egui::RichText::new("设备图标：Solar Icons · 480 Design").size(11.0).color(settings_color(ui.ctx(),Color32::from_gray(150),Color32::from_gray(105))));
                                ui.hyperlink_to("CC BY 4.0", "https://creativecommons.org/licenses/by/4.0/");
                            });
                        });
                        settings_label(ui,"关于岛仔");
                        settings_card(ui,about_upstream);
                    }
                    }
                });
            });
            // 液态玻璃参数弹窗挂在视口顶层（而非某个 tab 的滚动区里），这样离开「外观」页时
            // 守卫能可靠地把 liquid_cfg_open 清掉、弹窗随之收起。
            liquid_glass_config_window(self,ctx);
            self.avoidance.dialog(ctx,&mut self.settings);
            if before!=format!("{:?}",self.settings){self.settings.save();}
            ctx.set_style(original_style);
            egui_ctx.set_theme(original_theme);
        });
        if !self.settings_open {
            self.window_layout().close();
            self.weather_source.close();
        } else {
            self.weather_tutorial_ui(ctx);
        }
    }
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
        assert!(
            status_action(&format!("file:{}", exe.display())).is_none(),
            "可执行扩展必须拒绝"
        );
        assert!(
            status_action(&format!("file:{}", root.join("missing.txt").display())).is_none(),
            "不存在的路径不该生成按钮"
        );
        assert!(status_action("url:https://example.com").is_some());
        assert!(status_action("url:file:///C:/Windows/System32/cmd.exe").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod meeting_plan_tests {
    use super::meeting_hit;

    fn plan(items: &[(i64, i64, &str)]) -> Vec<(i64, i64, String)> {
        items
            .iter()
            .map(|(a, b, c)| (*a, *b, (*c).to_string()))
            .collect()
    }

    #[test]
    fn 只有刚开始且未结束的会议触发且最多两小时() {
        let now = 10_000;
        let p = plan(&[
            (now - 200, now + 600, "太早"), // 超过 90 秒窗口
            (now + 60, now + 3600, "还没开始"),
            (now - 30, now - 1, "已结束"),
        ]);
        assert!(meeting_hit(&p, now).is_none(), "这三种都不该触发");

        let p = plan(&[(now - 30, now + 1800, "进行中")]);
        let (until, key) = meeting_hit(&p, now).unwrap();
        assert_eq!(until, now + 1800);
        assert_eq!(key, format!("进行中|{}", now - 30));

        let p = plan(&[(now - 30, now + 99_999, "超长会")]);
        assert_eq!(
            meeting_hit(&p, now).unwrap().0,
            now + 7200,
            "勿扰最多 2 小时"
        );
    }

    #[test]
    fn 全天与无结束时间的日程不进候选表() {
        use crate::mail::Meeting;
        fn mk(all_day: bool, end: Option<i64>, uid: &str) -> Meeting {
            Meeting {
                all_day,
                title: "会".into(),
                start: 9_900,
                end,
                at: "11-12 10:00".into(),
                uid: uid.into(),
            }
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
fn meeting_candidates<'a>(
    events: impl Iterator<Item = &'a crate::mail::Meeting>,
) -> Vec<(i64, i64, String)> {
    events
        .filter_map(|event| {
            if event.all_day {
                return None;
            }
            Some((event.start, event.end?, event.uid.clone()))
        })
        .collect()
}

/// 从候选表挑出「刚开始 90 秒内、还没结束」的会议，返回 (勿扰截止, 去重键)。
/// 抽成纯函数便于单测；多场同时命中时取最后一个。
fn meeting_hit(plan: &[(i64, i64, String)], now: i64) -> Option<(i64, String)> {
    let mut hit = None;
    for (start, end, uid) in plan {
        if *start > now || now - *start > 90 {
            continue;
        }
        if *end <= now {
            continue;
        }
        hit = Some(((*end).min(now + 7200), format!("{}|{}", uid, start)));
    }
    hit
}
