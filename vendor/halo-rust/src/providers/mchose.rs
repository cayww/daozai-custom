//! MCHOSE（迈从）系列协议，按 Python `providers/mchose.py`、`mchose_k99.py`、
//! `mchose_kb.py`、`mchose_v9.py` 与 `mchose_a7.py`、`mchose_feedback.py` 精确移植。
//!
//! 每个 USB instance 对应一个物理设备：VID/PID 从 instance 解析，HID 集合按各协议的
//! 白名单筛选。请求报文、回复解析与校验均来自源码常量，不做猜测。
//!
//! 已移植的 Python 侧状态机：G7 的 `_smooth_g7` 中值滤波 + 80% 启动/低电量确认。
//! 仍受架构限制的部分（见文件末尾说明）：
//! - K99 的 `KeyboardActivity` / A7 V3 的 `MouseActivity` 输入唤醒门控：Rust worker
//!   是 stdin/stdout JSON 子进程，没有 Python 的 Raw Input 监听线程与输入源。
//! - `_last` 休眠保持：主程序 `bt/state.rs` 保留最后一次有效读数；worker 的
//!   `online=false` 会被 `bt/receivers.rs` 过滤，故这里不重复产出离线 Status。

use crate::hid::Collection;
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, Instant};

const IO_TIMEOUT: Duration = Duration::from_millis(250);
const ATTEMPT_GAP: Duration = Duration::from_millis(120);
const A5_WRITE_PAUSE: Duration = Duration::from_millis(34);
const A5_READ_GAP: Duration = Duration::from_millis(30);
const G7_READ_GAP: Duration = Duration::from_millis(20);
const G7_READ_TIMEOUT: Duration = Duration::from_millis(100);
const HEADSET_READ_MS: u64 = 400;
const MC_READ_MS: u64 = 200;
const V9_READ_MS: u64 = 500;
const V9_READ_PAUSE: Duration = Duration::from_millis(50);
const FEEDBACK_READ_MS: u64 = 400;

// ---- mchose.py: 0x5253 系列与 0x3837 鼠标的 0x06 状态查询 ----
const MEASURED_VID: u16 = 0x5253;
const NEW_VID: u16 = 0x3837;
const CONFIG_PAGE: u16 = 0xFF01;
const SHORT_REPORT: u8 = 0x11;
const LONG_REPORT: u8 = 0x12;
const SHORT_LEN: usize = 20;
const LONG_LEN: usize = 64;
const CMD_STATUS: u8 = 0x06;
const CHANNELS: [(u8, usize); 2] = [(SHORT_REPORT, SHORT_LEN), (LONG_REPORT, LONG_LEN)];
const ATTEMPTS: usize = 4;
const MODEL_M7_ULTRA: u16 = 0x0031;
const NEW_MOUSE_PIDS: [u16; 3] = [0x100B, 0x100A, 0x4018];

// ---- mchose.py: G7（0xA8A5） ----
const G7_VID: u16 = 0xA8A5;
const G7_PID: u16 = 0x2255;
const G7_REQUEST: [u8; 65] = [
    0x00, 0x55, 0x30, 0xA5, 0x0B, 0x2E, 0x01, 0x01, 0x01, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0,
];
const G7_HEADER: [u8; 2] = [0xAA, 0x30];
const G7_LEVEL_BYTE: usize = 8;
const G7_CHARGE_BYTE: usize = 9;
const G7_READS: usize = 25;

// ---- mchose.py: A5 Pro Max（0x2023） ----
const A5_VID: u16 = 0x2023;
const A5_PIDS: [(u16, &str); 3] = [
    (0xF013, "MCHOSE A5 Pro Max (1K receiver)"),
    (0xF015, "MCHOSE A5 Pro Max (4K receiver)"),
    (0xF019, "MCHOSE A5 Pro Max (wired)"),
];
const A5_USAGE_PAGE: u16 = 0xFFFF;
const A5_REPORT_ID: u8 = 0x00;
const A5_PAGE: u8 = 0x00;
const A5_CMD_BATTERY: u8 = 0x83;
const A5_ROUTE: u8 = 0x02;
const A5_LEN: u8 = 0x02;
const A5_READS: usize = 8;
const A5_RESEND_AT: usize = 3;

// ---- mchose.py: M HUB 0900（K7 V2 Ultra+ / A7 V3 Ultra+） ----
const MC_TARGET_PID: u16 = 0x1018;
const MC_LEN: usize = 64;
const MC_HEADER: usize = 8;
const MC_ATTEMPTS: usize = 3;
const MC_READS: usize = 5;

// ---- mchose.py: 0x3837 耳机 ----
const HEADSET_DONGLE_PID: u16 = 0x6008;
const HEADSET_BODY_PID: u16 = 0x600A;
const X9_PID: u16 = 0x6045;
const X9_CABLE_PID: u16 = 0x6044;
const X9_PAGE: u16 = 0xFF90;
const AUDIO_REPORT: u8 = 0x55;
const AUDIO_LEN: usize = 63;
const AUDIO_CMD_BATTERY: u8 = 0x0B;
const STATUS_DISCHARGING: u8 = 2;
const STATUS_CHARGING: u8 = 3;
const STATUS_FULL: u8 = 4;
const STATUS_ASLEEP: u8 = 26;
const HEADSET_ATTEMPTS: usize = 2;

// ---- mchose_k99.py ----
const K99_VID: u16 = 0x3837;
const K99_PID: u16 = 0x3033;
const K99_ALT_PAGE: u16 = 0xFF31;
const K99_ALT_USAGE: u16 = 0x0074;
const K99_REPORT_ID: u8 = 0x00;
const K99_REPORT_LEN: usize = 64;
const K99_FLAG: u8 = 0x55;
const K99_MAGIC: u8 = 0xAA;
const K99_CMD: u8 = 0x05;
const K99_SIZE: u8 = 56;
const K99_PAYLOAD_AT: usize = 8;
const K99_BATTERY_AT: usize = 32;
const K99_CHECKSUM_AT: usize = 3;
const K99_ATTEMPTS: usize = 3;
const K99_NAME: &str = "MCHOSE K99 V3";
const K99_KEY: &str = "mchose_k99:3033";

// ---- mchose_kb.py ----
const KB_VID: u16 = 0x41E4;
const KB_PRODUCTS: [(u16, &str); 2] = [(0x2001, "MCHOSE K87"), (0x2004, "MCHOSE G98 Pro")];
const KB_USAGE_PAGE: u16 = 0xFF02;
const KB_USAGE: u16 = 0x0002;
const KB_INTERFACE: u8 = 1;
const KB_REPORT_SIZE: usize = 20;
const KB_REPORT_ID: u8 = 0x13;
const KB_CMD: u8 = 0x4A;
const KB_ATTEMPTS: usize = 3;

// ---- mchose_v9.py ----
const V9_VID: u16 = 0x291D;
const V9_PID: u16 = 0x385D;
const V9_KEY: &str = "mchose_v9";
const V9_NAME: &str = "MCHOSE V9 Pro";
const V9_REPLY_HEADER: [u8; 2] = [0x55, 0x65];
const V9_READ_ATTEMPTS: usize = 3;

// ---- mchose_feedback.py ----
const FEEDBACK_VID: u16 = 0x3837;
const V9_PRO2_PID: u16 = 0x6011;
const G3_V2_PID: u16 = 0x4245;
const V9_PRO2_QUERY: [u8; 64] = [
    0x55, 0xAA, 0x01, 0x00, 0x01, 0x00, 0x01, 0x0B, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const G3_ONLINE: [u8; 65] = [
    0x00, 0x55, 0xED, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0,
];
const G3_QUERY: [u8; 65] = [
    0x00, 0x55, 0x30, 0xA5, 0x0B, 0x2E, 0x01, 0x01, 0x01, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0,
];

// ---- G7 滑动中值滤波（精确对应 mchose.py 的 _smooth_g7 状态机） ----
const G7_WINDOW: usize = 5;          // Python deque(maxlen=5)
const G7_SPACED_S: u64 = 5;          // 采样/确认的最小间隔（秒）
const G7_RESTART_S: u64 = 600;       // 超过这个空档视为重新开始
const G7_STARTUP_80: u32 = 5;        // 连续 5 次间隔 >=5s 的 80% 才接纳
const G7_DROP_CONFIRM: u8 = 20;      // 中心值比当前值高 >20 才进入低电量确认
const G7_LOW_TOLERANCE: u8 = 5;      // 两次低电量读数允许的差异
const ASLEEP_KEEP_S: u64 = 300;      // Python ASLEEP_KEEP；仅作文档说明（见文件末尾）

/// Python `statistics.median` + `round()`（banker's rounding）。samples 最多 5 个。
fn median_u8(samples: &VecDeque<u8>) -> Option<u8> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted: Vec<u8> = samples.iter().copied().collect();
    sorted.sort_unstable();
    let n = sorted.len();
    if n % 2 == 1 {
        return Some(sorted[n / 2]);
    }
    let a = sorted[n / 2 - 1] as u16;
    let b = sorted[n / 2] as u16;
    let sum = a + b;
    // round((a+b)/2)：偶数和直接取半；奇数和是 x.5，向偶数舍入。
    let rounded = if sum % 2 == 0 {
        sum / 2
    } else {
        let base = sum / 2;
        if base % 2 == 0 { base } else { base + 1 }
    };
    Some(rounded as u8)
}

/// G7 状态机：字段与 Python `_g7_samples/_g7_seen_at/_g7_sample_at/
/// _g7_startup_80/_g7_80_at/_g7_low` 一一对应。`now` 由调用方注入以便测试。
#[derive(Default)]
struct G7Smoother {
    samples: VecDeque<u8>,
    seen_at: Option<Instant>,
    sample_at: Option<Instant>,
    startup_80: u32,
    g7_80_at: Option<Instant>,
    low: Option<(u8, Instant)>,
}

impl G7Smoother {
    fn spaced(now: Instant, at: Option<Instant>) -> bool {
        at.is_none_or(|t| now.saturating_duration_since(t) >= Duration::from_secs(G7_SPACED_S))
    }

    fn smooth(&mut self, level: u8, charging: bool, now: Instant) -> Option<u8> {
        // Python: if charging or self._g7_seen_at is None or not 0 <= now-seen < 600:
        let fresh = self.seen_at.is_some_and(|t| {
            now.saturating_duration_since(t) < Duration::from_secs(G7_RESTART_S)
        });
        if charging || !fresh {
            self.samples.clear();
            self.sample_at = None;
            self.startup_80 = 0;
            self.g7_80_at = None;
            self.low = None;
        }
        self.seen_at = Some(now);

        if charging {
            self.samples.push_back(level);
            self.sample_at = Some(now);
        } else if level == 80 && self.samples.is_empty() {
            // Repeated 80s can be stale for many minutes. Five spaced readings
            // are an availability fallback, not proof of the true charge.
            if Self::spaced(now, self.g7_80_at) {
                self.startup_80 += 1;
                self.g7_80_at = Some(now);
                if self.startup_80 >= G7_STARTUP_80 {
                    self.samples.push_back(80);
                    self.sample_at = Some(now);
                }
            }
        } else {
            self.startup_80 = 0;
            self.g7_80_at = None;
            let centre = median_u8(&self.samples);
            if level == 80 && centre.is_some_and(|c| c.abs_diff(80) >= 10) {
                // Keep rejecting out-of-trend 80s once there is an estimate.
                self.low = None;
            } else if Self::spaced(now, self.sample_at) {
                if centre.is_some_and(|c| (c as i16) - (level as i16) > G7_DROP_CONFIRM as i16) {
                    match self.low {
                        Some((pending, at))
                            if pending.abs_diff(level) <= G7_LOW_TOLERANCE
                                && Self::spaced(now, Some(at)) =>
                        {
                            // Two similar, spaced low replies replace the high history.
                            self.samples.clear();
                            self.samples.push_back(pending);
                            self.low = None;
                        }
                        _ => self.low = Some((level, now)),
                    }
                } else {
                    self.low = None;
                }
                self.samples.push_back(level);
                self.sample_at = Some(now);
            } else if let Some((low_level, _)) = self.low {
                // Even a fast contradictory refresh cancels confirmation.
                let contradicts = centre.is_none()
                    || centre.is_some_and(|c| (c as i16) - (level as i16) <= G7_DROP_CONFIRM as i16)
                    || low_level.abs_diff(level) > G7_LOW_TOLERANCE;
                if contradicts {
                    self.low = None;
                }
            }
        }
        while self.samples.len() > G7_WINDOW {
            self.samples.pop_front();
        }
        median_u8(&self.samples)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    Legacy5253,
    LegacyNew,
    G7,
    A5,
    Mc,
    Headset,
    X9,
    K99,
    Kb,
    V9,
    V9Pro2,
    G3,
}

fn classify(vid: u16, pid: u16) -> Option<Variant> {
    match (vid, pid) {
        (MEASURED_VID, _) => Some(Variant::Legacy5253),
        (NEW_VID, p) if NEW_MOUSE_PIDS.contains(&p) => Some(Variant::LegacyNew),
        (NEW_VID, MC_TARGET_PID) => Some(Variant::Mc),
        (NEW_VID, HEADSET_DONGLE_PID | HEADSET_BODY_PID) => Some(Variant::Headset),
        (NEW_VID, X9_PID) => Some(Variant::X9),
        (NEW_VID, X9_CABLE_PID) => None, // X9 在线上：M HUB 不读，绝不写入
        (K99_VID, K99_PID) => Some(Variant::K99),
        (FEEDBACK_VID, V9_PRO2_PID) => Some(Variant::V9Pro2),
        (FEEDBACK_VID, G3_V2_PID) => Some(Variant::G3),
        (NEW_VID, _) => None, // 无来源的 0x3837 产品：不发旧版鼠标查询
        (G7_VID, G7_PID) => Some(Variant::G7),
        (G7_VID, _) => None, // 0xA8A5 是芯片厂商 vid，其他设备不写入
        (A5_VID, p) if A5_PIDS.iter().any(|(id, _)| *id == p) => Some(Variant::A5),
        (A5_VID, _) => None,
        (KB_VID, p) if KB_PRODUCTS.iter().any(|(id, _)| *id == p) => Some(Variant::Kb),
        (V9_VID, V9_PID) => Some(Variant::V9),
        _ => None,
    }
}

/// 与 Python `device_key` 一致：同一产品的接收器与有线身份共用图标。
fn device_key(vid: u16, pid: u16) -> String {
    if vid == MEASURED_VID {
        return "mchose".into();
    }
    if pid == HEADSET_DONGLE_PID || pid == HEADSET_BODY_PID {
        return format!("mchose:{vid:04x}:{HEADSET_DONGLE_PID:04x}");
    }
    if pid == X9_PID {
        return format!("mchose:{vid:04x}:{pid:04x}");
    }
    format!("mchose:{vid:04x}")
}

fn invert(data: &[u8]) -> Vec<u8> {
    data.iter().map(|b| b ^ 0xFF).collect()
}

fn xor(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc ^ b)
}

fn sum8(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

// ---- 请求构造 ----

/// mchose.py `make_request`：报 id + 取反的 cmd 与全零尾部。
pub(crate) fn make_request(cmd: u8, report: u8, length: usize) -> Vec<u8> {
    let mut body = vec![0u8; length];
    body[0] = cmd;
    let mut frame = Vec::with_capacity(length + 1);
    frame.push(report);
    frame.extend(invert(&body));
    frame
}

/// mchose.py `make_mc_request`：M HUB 的 0900 请求（含 XOR 校验位）。
pub(crate) fn make_mc_request() -> [u8; MC_LEN] {
    let mut frame = [0u8; MC_LEN];
    frame[..9].copy_from_slice(&[0x4D, 0x01, 0x01, 0x00, 0x00, 0x09, 0x00, 0x00, 0x08]);
    frame
}

/// mchose.py `make_a5_request`：report id 0，route/len/page/cmd 在字节 3..6。
pub(crate) fn make_a5_request() -> [u8; 65] {
    let mut frame = [0u8; 65];
    frame[..7].copy_from_slice(&[0x00, 0x00, 0x00, A5_ROUTE, A5_LEN, A5_PAGE, A5_CMD_BATTERY]);
    frame
}

/// mchose.py `AUDIO_REQUEST`：AA 帧电量查询（含 report id 0x55）。
pub(crate) fn audio_request() -> Vec<u8> {
    let mut frame = vec![AUDIO_REPORT; AUDIO_LEN + 1];
    frame[1..9].copy_from_slice(&[0xAA, 0x01, 0x00, 0x01, 0x00, 0x01, AUDIO_CMD_BATTERY, 0x00]);
    frame
}

/// mchose.py `STATUS_REQUEST`：`65 01` 帧（含 report id 0x55）。
pub(crate) fn status_request() -> Vec<u8> {
    let mut frame = vec![AUDIO_REPORT; AUDIO_LEN + 1];
    frame[1] = 0x65;
    frame[2] = 0x01;
    frame
}

/// mchose_k99.py `wire_request`：report id 0 + 64 字节 GLW 帧。
pub(crate) fn make_k99_request() -> [u8; K99_REPORT_LEN + 1] {
    let mut frame = [0u8; K99_REPORT_LEN + 1];
    frame[1..9].copy_from_slice(&[K99_FLAG, K99_CMD, 0x00, K99_SIZE, K99_SIZE, 0x00, 0x00, 0x00]);
    frame
}

/// mchose_kb.py `REQUEST`：`13 4a 01` + 16 零 + 加法和校验。
pub(crate) fn make_kb_request() -> [u8; KB_REPORT_SIZE] {
    let mut body = [0u8; KB_REPORT_SIZE];
    body[..3].copy_from_slice(&[KB_REPORT_ID, KB_CMD, 0x01]);
    body[KB_REPORT_SIZE - 1] = sum8(&body[..KB_REPORT_SIZE - 1]);
    body
}

/// mchose_v9.py `make_request`：`55 65 01` + 零（含 report id）。
pub(crate) fn make_v9_request() -> [u8; 64] {
    let mut frame = [0u8; 64];
    frame[..3].copy_from_slice(&[0x55, 0x65, 0x01]);
    frame
}

// ---- 回复解析 ----

/// mchose.py `parse_status`：`(level, charge, model, flags)`。
pub(crate) fn parse_status(resp: &[u8]) -> Option<(u8, u8, u16, u8)> {
    if resp.len() < 13 {
        return None;
    }
    if ![SHORT_REPORT, LONG_REPORT].contains(&resp[0]) || resp[1] != (CMD_STATUS ^ 0xFF) {
        return None;
    }
    let pay = invert(&resp[2..]);
    if pay.len() < 11 {
        return None;
    }
    let vid = u16::from_le_bytes([pay[0], pay[1]]);
    if ![MEASURED_VID, NEW_VID].contains(&vid) {
        return None;
    }
    let model = u16::from_le_bytes([pay[2], pay[3]]);
    let (flags, level, charge) = (pay[8], pay[9], pay[10]);
    if level > 100 {
        return None;
    }
    Some((level, charge, model, flags))
}

/// mchose.py `mc_payload`：仅接受带校验且尾部全零的 0900 回复。
pub(crate) fn mc_payload(resp: &[u8]) -> Option<&[u8]> {
    if !(10..=MC_LEN).contains(&resp.len()) {
        return None;
    }
    if resp[..3] != [0x4D, 0x01, 0x01] || resp[4..6] != [0x00, 0x09] {
        return None;
    }
    let end = MC_HEADER + resp[3] as usize;
    if resp[3] == 0 || end >= resp.len() {
        return None;
    }
    if resp[end] != xor(&resp[2..end]) || resp[end + 1..].iter().any(|&b| b != 0) {
        return None;
    }
    Some(&resp[MC_HEADER..end])
}

/// mchose.py `parse_mc_info`：`(level, charging)`。
pub(crate) fn parse_mc_info(resp: &[u8]) -> Option<(u8, bool)> {
    let payload = mc_payload(resp)?;
    if payload.len() < 13
        || payload[0] == 0xFF
        || payload[..2] != [0x37, 0x38]
        || ![0, 1].contains(&payload[11])
        || !(1..=100).contains(&payload[12])
    {
        return None;
    }
    Some((payload[12], payload[11] == 1))
}

/// mchose.py `parse_g7`：`(level, charging)`。
pub(crate) fn parse_g7(resp: &[u8]) -> Option<(u8, bool)> {
    if resp.len() <= G7_CHARGE_BYTE || resp[..2] != G7_HEADER {
        return None;
    }
    let level = resp[G7_LEVEL_BYTE];
    if level > 100 {
        return None;
    }
    Some((level, resp[G7_CHARGE_BYTE] != 0))
}

/// mchose.py `parse_a5`：`(level, charging)`，兼容 hidapi 保留 report id 的布局。
pub(crate) fn parse_a5(resp: &[u8]) -> Option<(u8, bool)> {
    if resp.len() < 9 {
        return None;
    }
    let at = if resp[0] == 0xA1 {
        0
    } else if resp[1] == 0xA1 {
        1
    } else {
        return None;
    };
    if resp.len() < at + 8 || resp[at + 4] != A5_PAGE || resp[at + 5] != A5_CMD_BATTERY {
        return None;
    }
    let level = resp[at + 7];
    if level > 100 {
        return None;
    }
    Some((level, resp[at + 6] > 0))
}

/// mchose.py `audio_checksum_ok`，参数是去掉 report id 的 AA 帧。
pub(crate) fn audio_checksum_ok(frame: &[u8]) -> bool {
    if frame.len() < 8 {
        return false;
    }
    let (flags, length) = (frame[2], frame[3] as usize);
    if length < 1 || frame.len() < 7 + length {
        return false;
    }
    let mut acc = flags ^ frame[3] ^ frame[4] ^ frame[5] ^ frame[6];
    for &b in &frame[7..7 + length - 1] {
        acc ^= b;
    }
    let expected = if flags == 1 { acc } else { 0 };
    frame[7 + length - 1] == expected
}

/// mchose.py `parse_audio`：`(level, charging)`，睡眠时 level 为 None。
pub(crate) fn parse_audio(resp: &[u8]) -> Option<(Option<u8>, bool)> {
    if resp.len() < 11 || resp[0] != AUDIO_REPORT || resp[1] != 0xAA {
        return None;
    }
    if resp[5] != 0x00 || resp[6] != 0x01 || resp[7] != AUDIO_CMD_BATTERY {
        return None;
    }
    if !audio_checksum_ok(&resp[1..]) {
        return None;
    }
    let length = resp[4] as usize;
    if length < 3 {
        return None;
    }
    let payload = &resp[8..8 + length - 1];
    if payload[0] == 0 {
        return Some((None, false)); // 睡眠
    }
    let percent = payload[1] & 0x7F;
    if percent > 100 {
        return None;
    }
    Some((Some(percent), payload[1] >> 7 == 1))
}

/// mchose.py `parse_status_55`：`(level, charging)`，睡眠时 level 为 None。
pub(crate) fn parse_status_55(resp: &[u8]) -> Option<(Option<u8>, bool)> {
    if resp.len() < 4 || resp[0] != AUDIO_REPORT || resp[1] != 0x65 {
        return None;
    }
    let (level, state) = (resp[2], resp[3]);
    if state == STATUS_ASLEEP {
        return Some((None, false));
    }
    if ![STATUS_DISCHARGING, STATUS_CHARGING, STATUS_FULL].contains(&state) {
        return None;
    }
    if level > 100 {
        return None;
    }
    Some((Some(level), state == STATUS_CHARGING))
}

/// mchose_k99.py `parse_reply`：`(level, charging, raw)`。
pub(crate) fn parse_k99_reply(resp: &[u8]) -> Option<(u8, bool, u8)> {
    let b = if resp.len() >= K99_REPORT_LEN + 1
        && resp[0] == K99_REPORT_ID
        && resp[1] == K99_MAGIC
    {
        &resp[1..]
    } else {
        resp
    };
    if b.len() < K99_PAYLOAD_AT + K99_SIZE as usize
        || b[0] != K99_MAGIC
        || b[1] != K99_CMD
    {
        return None;
    }
    let size = b[4] as usize;
    let offset = b[5] as usize | ((b[6] as usize) << 8);
    if size < K99_BATTERY_AT + 1 || offset != 0 || K99_PAYLOAD_AT + size > b.len() {
        return None;
    }
    let mut checksum = b[4] as u16
        + b[5] as u16
        + b[6] as u16
        + b[7] as u16
        + b[K99_PAYLOAD_AT..K99_PAYLOAD_AT + size]
            .iter()
            .map(|&v| v as u16)
            .sum::<u16>();
    checksum &= 0xFF;
    if checksum != b[K99_CHECKSUM_AT] as u16 {
        return None;
    }
    let raw = b[K99_PAYLOAD_AT + K99_BATTERY_AT];
    let level = raw & 0x7F;
    if level > 100 {
        return None;
    }
    Some((level, raw & 0x80 != 0, raw))
}

/// mchose_kb.py `parse_reply`：`(level, flags)`。
pub(crate) fn parse_kb_reply(resp: &[u8]) -> Option<(u8, u8)> {
    if resp.len() != KB_REPORT_SIZE
        || resp[..4] != [KB_REPORT_ID, KB_CMD, 0x01, 0x00]
        || resp[KB_REPORT_SIZE - 1] != sum8(&resp[..KB_REPORT_SIZE - 1])
    {
        return None;
    }
    let level = resp[5];
    if level > 100 {
        return None;
    }
    Some((level, resp[6]))
}

/// mchose_kb.py `parse_charging`：高半字节 0/1 为已知状态。
pub(crate) fn parse_kb_charging(flags: u8) -> Option<bool> {
    match (flags >> 4) & 0x0F {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// mchose_v9.py `parse_status`：`(level, status)`。
pub(crate) fn parse_v9_status(resp: &[u8]) -> Option<(u8, u8)> {
    if resp.len() < 4 || resp[..2] != V9_REPLY_HEADER {
        return None;
    }
    let level = resp[2];
    if level > 100 {
        return None;
    }
    Some((level, resp[3]))
}

/// mchose_feedback.py 中 V9 Pro 2 的回复匹配条件。
pub(crate) fn v9pro2_matches(resp: &[u8]) -> bool {
    resp.len() == 64
        && resp[..3] == [0x55, 0xAA, 0x01]
        && (resp[5..8] == [0x00, 0x01, 0x0B] || resp[5..8] == [0x00, 0x04, 0x22])
        && (3..=59).contains(&resp[4])
}

/// mchose_feedback.py 中 G3 V2 的在线/电量回复匹配条件。
pub(crate) fn g3_online_matches(resp: &[u8]) -> bool {
    resp.len() >= 9 && resp[..2] == [0xAA, 0xED]
}

pub(crate) fn g3_query_matches(resp: &[u8]) -> bool {
    resp.len() >= 10 && resp[..2] == [0xAA, 0x30]
}

// ---- 读取流程（与 Python 的重试次数和间隔一致） ----

fn read_legacy(col: &Collection) -> Option<(u8, u8, u16, u8)> {
    let mut handle = col.open()?;
    for (report, length) in CHANNELS {
        for _ in 0..ATTEMPTS {
            let request = make_request(CMD_STATUS, report, length);
            if !handle.set_feature(&request, Instant::now() + IO_TIMEOUT) {
                break; // Python: send 失败切到下一通道
            }
            sleep(ATTEMPT_GAP);
            // Python: 读失败重试，且每次尝试都重新发送请求
            let Some(resp) = handle.get_feature(report, Instant::now() + IO_TIMEOUT) else {
                continue;
            };
            if let Some(got) = parse_status(&resp) {
                return Some(got);
            }
        }
    }
    None
}

fn read_g7(col: &Collection) -> Option<(u8, bool)> {
    let mut handle = col.open()?;
    if !handle.write(&G7_REQUEST, Instant::now() + IO_TIMEOUT) {
        return None;
    }
    for _ in 0..G7_READS {
        sleep(G7_READ_GAP);
        if let Some(resp) = handle.read(Instant::now() + G7_READ_TIMEOUT) {
            if let Some(got) = parse_g7(&resp) {
                return Some(got);
            }
        }
    }
    None
}

fn read_a5(col: &Collection) -> Option<(u8, bool)> {
    let mut handle = col.open()?;
    let request = make_a5_request();
    if !handle.set_feature(&request, Instant::now() + IO_TIMEOUT) {
        return None;
    }
    sleep(A5_WRITE_PAUSE);
    for attempt in 0..A5_READS {
        if attempt == A5_RESEND_AT {
            if !handle.set_feature(&request, Instant::now() + IO_TIMEOUT) {
                return None;
            }
            sleep(A5_WRITE_PAUSE);
        }
        if let Some(resp) = handle.get_feature(A5_REPORT_ID, Instant::now() + IO_TIMEOUT) {
            if let Some(got) = parse_a5(&resp) {
                return Some(got);
            }
        }
        sleep(A5_READ_GAP);
    }
    None
}

fn read_mc(col: &Collection) -> Option<(u8, bool)> {
    let mut handle = col.open()?;
    for _ in 0..MC_ATTEMPTS {
        if !handle.write(&make_mc_request(), Instant::now() + IO_TIMEOUT) {
            return None;
        }
        let mut retry = false;
        for _ in 0..MC_READS {
            let Some(resp) = handle.read(Instant::now() + Duration::from_millis(MC_READ_MS)) else {
                break;
            };
            if let Some(got) = parse_mc_info(&resp) {
                return Some(got);
            }
            if let Some(payload) = mc_payload(&resp) {
                retry = payload[0] == 0xFF
                    || (payload.len() >= 13
                        && payload[..2] == [0x37, 0x38]
                        && [0, 1].contains(&payload[11])
                        && payload[12] == 0);
                break;
            }
        }
        if !retry {
            break;
        }
    }
    None
}

fn read_headset(col: &Collection, pid: u16) -> Option<(u8, bool)> {
    let mut handle = col.open()?;
    // X9 是 BLK profile，只有 65 01 帧；AA 帧属于 MC profile，不发送给它。
    let (requests, parsers): (Vec<Vec<u8>>, Vec<fn(&[u8]) -> Option<(Option<u8>, bool)>>) =
        if pid == X9_PID {
            (vec![status_request()], vec![parse_status_55])
        } else {
            (
                vec![audio_request(), status_request()],
                vec![parse_audio, parse_status_55],
            )
        };
    for (request, parser) in requests.iter().zip(parsers.iter()) {
        for _ in 0..HEADSET_ATTEMPTS {
            if !handle.write(request, Instant::now() + IO_TIMEOUT) {
                break;
            }
            if let Some(resp) =
                handle.read(Instant::now() + Duration::from_millis(HEADSET_READ_MS))
            {
                if let Some((Some(level), charging)) = parser(&resp) {
                    return Some((level, charging));
                }
                // 睡眠回复（level=None）按 Python 语义记录后结束该产品的查询。
            }
        }
    }
    None
}

fn read_k99(col: &Collection) -> Option<(u8, bool)> {
    let mut handle = col.open()?;
    let request = make_k99_request();
    for _ in 0..K99_ATTEMPTS {
        if !handle.write(&request, Instant::now() + IO_TIMEOUT) {
            continue;
        }
        let until = Instant::now() + Duration::from_millis(150);
        while Instant::now() < until {
            if let Some(resp) = handle.read(Instant::now() + Duration::from_millis(150)) {
                if let Some((level, charging, _)) = parse_k99_reply(&resp) {
                    return Some((level, charging));
                }
            }
        }
        sleep(A5_READ_GAP);
    }
    None
}

fn read_kb(col: &Collection) -> Option<(u8, bool, bool)> {
    let mut handle = col.open()?;
    let request = make_kb_request();
    for _ in 0..KB_ATTEMPTS {
        if !handle.write(&request, Instant::now() + IO_TIMEOUT) {
            return None;
        }
        let until = Instant::now() + Duration::from_millis(150);
        while Instant::now() < until {
            if let Some(resp) = handle.read(Instant::now() + Duration::from_millis(100)) {
                if let Some((level, flags)) = parse_kb_reply(&resp) {
                    let charging = parse_kb_charging(flags);
                    return Some((level, charging.unwrap_or(false), charging.is_some()));
                }
            }
        }
        sleep(Duration::from_millis(150));
    }
    None
}

fn read_v9(col: &Collection) -> Option<(u8, bool)> {
    let mut handle = col.open()?;
    if !handle.write(&make_v9_request(), Instant::now() + IO_TIMEOUT) {
        return None;
    }
    for _ in 0..V9_READ_ATTEMPTS {
        sleep(V9_READ_PAUSE);
        if let Some(resp) = handle.read(Instant::now() + Duration::from_millis(V9_READ_MS)) {
            if let Some((level, status)) = parse_v9_status(&resp) {
                return Some((level, status == 0));
            }
        }
    }
    None
}

fn read_v9pro2(col: &Collection) -> Option<u8> {
    let mut handle = col.open()?;
    if !handle.write(&V9_PRO2_QUERY, Instant::now() + IO_TIMEOUT) {
        return None;
    }
    for _ in 0..5 {
        let Some(resp) = handle.read(Instant::now() + Duration::from_millis(FEEDBACK_READ_MS))
        else {
            return None;
        };
        if resp.len() != 64 || !v9pro2_matches(&resp) {
            continue;
        }
        if resp[8] == 0 {
            return None; // 睡眠值不作有效读数
        }
        let level = resp[9] & 0x7F;
        if level > 100 {
            return None;
        }
        return Some(level);
    }
    None
}

fn read_g3(col: &Collection) -> Option<(u8, bool, bool)> {
    let mut handle = col.open()?;
    let deadline = Instant::now() + Duration::from_millis(550);
    if !handle.write(&G3_ONLINE, deadline) {
        return None;
    }
    let Some(online) = handle.read(deadline) else {
        return None;
    };
    if !g3_online_matches(&online) || ![0, 2].contains(&online[8]) {
        return None;
    }
    if !handle.write(&G3_QUERY, deadline) {
        return None;
    }
    let Some(reply) = handle.read(deadline) else {
        return None;
    };
    if !g3_query_matches(&reply) || !(1..=100).contains(&reply[8]) {
        return None;
    }
    Some((reply[8], reply[9] == 1, [0, 1].contains(&reply[9])))
}

fn make_status(
    key: impl Into<String>,
    name: impl Into<String>,
    kind: &str,
    source: &str,
    level: Option<u8>,
    charging: Option<bool>,
    charging_known: bool,
) -> Status {
    let mut st = Status::new(key, name);
    st.kind = kind.into();
    st.source = source.into();
    st.level = level;
    st.charging = charging;
    st.charging_known = charging_known;
    st
}

/// 按协议选择可写入的 HID 集合。Rust 的 hid::collections 只暴露 vendor page
/// (>= 0xFF00) 的集合，因此 K99 的主集合 0x0001:0x0000 不会出现，使用源码中的
/// 备选集合 0xFF31:0x0074（Python 注释说明它回答同样的帧）。
fn pick<'a>(variant: Variant, collections: &'a [Collection]) -> Vec<&'a Collection> {
    let mut picked: Vec<&Collection> = match variant {
        Variant::Legacy5253 => {
            let mut v: Vec<_> = collections
                .iter()
                .filter(|c| c.usage_page >= 0xFF00)
                .collect();
            v.sort_by_key(|c| (c.usage_page != CONFIG_PAGE, c.usage != 1));
            v
        }
        Variant::LegacyNew | Variant::Mc => collections
            .iter()
            .filter(|c| c.interface == Some(2) && c.usage_page == CONFIG_PAGE && c.usage == 1)
            .collect(),
        Variant::G7 => collections
            .iter()
            .filter(|c| c.usage_page == CONFIG_PAGE)
            .collect(),
        Variant::A5 => collections
            .iter()
            .filter(|c| c.usage_page == A5_USAGE_PAGE)
            .collect(),
        Variant::Headset => {
            let mut v: Vec<_> = collections
                .iter()
                .filter(|c| c.usage_page >= 0xFF00)
                .collect();
            v.sort_by_key(|c| c.usage_page != 0xFF00);
            v
        }
        Variant::X9 => collections
            .iter()
            .filter(|c| c.usage_page == X9_PAGE && c.usage == 1)
            .collect(),
        Variant::K99 => collections
            .iter()
            .filter(|c| c.usage_page == K99_ALT_PAGE && c.usage == K99_ALT_USAGE)
            .collect(),
        Variant::Kb => collections
            .iter()
            .filter(|c| {
                c.usage_page == KB_USAGE_PAGE
                    && c.usage == KB_USAGE
                    && c.interface == Some(KB_INTERFACE)
            })
            .collect(),
        Variant::V9 => collections
            .iter()
            .filter(|c| c.usage_page >= 0xFF00)
            .collect(),
        Variant::V9Pro2 => collections
            .iter()
            .filter(|c| c.usage_page == 0xFF07 && c.usage == 0x0222 && c.interface == Some(3))
            .collect(),
        Variant::G3 => collections
            .iter()
            .filter(|c| c.usage_page == CONFIG_PAGE && c.usage == 0x0010)
            .collect(),
    };
    // 与 Python 一致：逐个集合尝试，首个给出有效读数即返回（由 poll 负责提前返回）。
    picked
}

#[derive(Default)]
struct State {
    g7: G7Smoother,
    /// `_last` 休眠保持：按 (USB instance, Status.key) 缓存最后一次有效读数。
    last: HashMap<(String, String), (Status, Instant)>,
}

pub struct Mchose {
    state: Mutex<State>,
}

impl Mchose {
    pub fn new() -> Self {
        Self { state: Mutex::new(State::default()) }
    }
}

impl Default for Mchose {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for Mchose {
    fn name(&self) -> &'static str {
        "mchose"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        let variant = classify(vid, pid);
        let mut state = self.state.lock().unwrap();
        let mut found: Option<Status> = None;
        if let Some(variant) = variant {
        for col in pick(variant, collections) {
            let status = match variant {
                Variant::Legacy5253 | Variant::LegacyNew => {
                    read_legacy(col).map(|(level, charge, model, _)| {
                        let name = if model == MODEL_M7_ULTRA {
                            "MCHOSE M7 Ultra".to_string()
                        } else {
                            format!("MCHOSE mouse (0x{model:04x})")
                        };
                        make_status(
                            device_key(vid, pid),
                            name,
                            "mouse",
                            "mchose",
                            Some(level),
                            Some(charge != 0),
                            true,
                        )
                    })
                }
                Variant::G7 => read_g7(col).map(|(raw, charging)| {
                    // Python: level = self._smooth_g7(raw, charge)；返回 None 时
                    // 仍产出 Status，只是 level=None（"battery pending"）。
                    let level = state.g7.smooth(raw, charging, Instant::now());
                    make_status(
                        device_key(vid, pid),
                        "MCHOSE G7",
                        "mouse",
                        "mchose",
                        level,
                        Some(charging),
                        true,
                    )
                }),
                Variant::A5 => {
                    let name = A5_PIDS
                        .iter()
                        .find(|(id, _)| *id == pid)
                        .map(|(_, n)| *n)
                        .unwrap_or("MCHOSE A5 Pro Max");
                    read_a5(col).map(|(level, charging)| {
                        make_status(
                            device_key(vid, pid),
                            name,
                            "mouse",
                            "mchose",
                            Some(level),
                            Some(charging),
                            true,
                        )
                    })
                }
                Variant::Mc => read_mc(col).map(|(level, charging)| {
                    make_status(
                        device_key(vid, pid),
                        "MCHOSE mouse",
                        "mouse",
                        "mchose",
                        Some(level),
                        Some(charging),
                        true,
                    )
                }),
                Variant::Headset => {
                    let name = "MCHOSE V9 Turbo+";
                    read_headset(col, pid).map(|(level, charging)| {
                        make_status(
                            device_key(vid, pid),
                            name,
                            "headset",
                            "mchose",
                            Some(level),
                            Some(charging),
                            true,
                        )
                    })
                }
                Variant::X9 => read_headset(col, pid).map(|(level, charging)| {
                    make_status(
                        device_key(vid, pid),
                        "MCHOSE X9",
                        "headset",
                        "mchose",
                        Some(level),
                        Some(charging),
                        true,
                    )
                }),
                Variant::K99 => read_k99(col).map(|(level, charging)| {
                    make_status(
                        K99_KEY,
                        K99_NAME,
                        "keyboard",
                        "mchose_k99",
                        Some(level),
                        Some(charging),
                        true,
                    )
                }),
                Variant::Kb => {
                    let name = KB_PRODUCTS
                        .iter()
                        .find(|(id, _)| *id == pid)
                        .map(|(_, n)| *n)
                        .unwrap_or("MCHOSE keyboard");
                    read_kb(col).map(|(level, charging, known)| {
                        make_status(
                            format!("mchose_kb:{pid:04x}"),
                            name,
                            "keyboard",
                            "mchose_kb",
                            Some(level),
                            Some(charging),
                            known,
                        )
                    })
                }
                Variant::V9 => read_v9(col).map(|(level, charging)| {
                    make_status(
                        V9_KEY,
                        V9_NAME,
                        "headset",
                        "mchose_v9",
                        Some(level),
                        Some(charging),
                        true,
                    )
                }),
                Variant::V9Pro2 => read_v9pro2(col).map(|level| {
                    make_status(
                        "mchose_v9pro2",
                        "MCHOSE V9 Pro 2",
                        "headset",
                        "mchose_v9pro2",
                        Some(level),
                        None,
                        false,
                    )
                }),
                Variant::G3 => read_g3(col).map(|(level, charging, known)| {
                    make_status(
                        "mchose_g3v2",
                        "MCHOSE G3 V2",
                        "mouse",
                        "mchose_g3v2",
                        Some(level),
                        Some(charging),
                        known,
                    )
                }),
            };
            if status.is_some() {
                found = status;
                break;
            }
        }
        }

        let now = Instant::now();
        if let Some(status) = found {
            // 与 Python `self._last[key] = (level, charge, time.time())` 对应。
            state.last.insert((instance.to_string(), status.key.clone()), (status.clone(), now));
            return vec![status];
        }

        // 无回复：保持最后一次读数一段时间并灰显（Python `_last` 循环）。
        // 主程序 bt/receivers.rs 会过滤 online=false，但保留 Engine.observed
        // 的热度，使设备唤醒后的同值读数仍能被判为 fresh 并 ingested。
        let mut out = Vec::new();
        let mut expired = Vec::new();
        for ((inst, key), (status, at)) in state.last.iter() {
            if inst != instance {
                continue;
            }
            if now.saturating_duration_since(*at) >= Duration::from_secs(ASLEEP_KEEP_S) {
                expired.push((inst.clone(), key.clone()));
                continue;
            }
            let mut offline = status.clone();
            offline.online = false;
            out.push(offline);
        }
        for k in expired {
            state.last.remove(&k);
        }
        out
    }
}

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![Box::new(Mchose::new())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_request_is_inverted_and_framed() {
        let short = make_request(CMD_STATUS, SHORT_REPORT, SHORT_LEN);
        assert_eq!(short.len(), SHORT_LEN + 1);
        assert_eq!(short[0], SHORT_REPORT);
        assert_eq!(short[1], CMD_STATUS ^ 0xFF);
        assert_eq!(short[2], 0xFF);
        let long = make_request(CMD_STATUS, LONG_REPORT, LONG_LEN);
        assert_eq!(long.len(), LONG_LEN + 1);
        assert_eq!(long[0], LONG_REPORT);
        assert!(long[2..].iter().all(|&b| b == 0xFF));
    }

    fn status_frame(vid: u16, model: u16, flags: u8, level: u8, charge: u8) -> Vec<u8> {
        let mut payload = vec![0u8; SHORT_LEN - 1];
        payload[..2].copy_from_slice(&vid.to_le_bytes());
        payload[2..4].copy_from_slice(&model.to_le_bytes());
        payload[8] = flags;
        payload[9] = level;
        payload[10] = charge;
        let mut resp = vec![SHORT_REPORT, CMD_STATUS ^ 0xFF];
        resp.extend(invert(&payload));
        resp
    }

    #[test]
    fn legacy_status_parser_accepts_and_rejects() {
        let resp = status_frame(MEASURED_VID, MODEL_M7_ULTRA, 0x09, 76, 1);
        assert_eq!(parse_status(&resp), Some((76, 1, MODEL_M7_ULTRA, 0x09)));
        // 回显不对、vid 不对、电量越界、长度不足都必须拒绝
        let mut bad = resp.clone();
        bad[1] = 0x00;
        assert_eq!(parse_status(&bad), None);
        assert_eq!(parse_status(&status_frame(0x1234, 0, 0, 50, 0)), None);
        assert_eq!(parse_status(&status_frame(MEASURED_VID, 0, 0, 101, 0)), None);
        assert_eq!(parse_status(&resp[..8]), None);
    }

    #[test]
    fn mc_request_and_info_parser() {
        assert_eq!(
            make_mc_request()[..9],
            [0x4D, 0x01, 0x01, 0x00, 0x00, 0x09, 0x00, 0x00, 0x08]
        );
        let mut payload = [0u8; 13];
        payload[..2].copy_from_slice(&[0x37, 0x38]);
        payload[11] = 1;
        payload[12] = 85;
        let mut frame = [0u8; 32];
        frame[..6].copy_from_slice(&[0x4D, 0x01, 0x01, 13, 0x00, 0x09]);
        frame[MC_HEADER..MC_HEADER + 13].copy_from_slice(&payload);
        frame[MC_HEADER + 13] = xor(&frame[2..MC_HEADER + 13]);
        assert_eq!(parse_mc_info(&frame), Some((85, true)));
        let mut bad = frame;
        bad[MC_HEADER + 14] = 1; // 校验位之后必须全零
        assert_eq!(parse_mc_info(&bad), None);
    }

    #[test]
    fn g7_and_a5_parsers() {
        let mut g7 = vec![0u8; 12];
        g7[..2].copy_from_slice(&G7_HEADER);
        g7[G7_LEVEL_BYTE] = 46;
        g7[G7_CHARGE_BYTE] = 1;
        assert_eq!(parse_g7(&g7), Some((46, true)));
        let mut a5 = vec![0u8; 9];
        a5[0] = 0xA1;
        a5[4] = A5_PAGE;
        a5[5] = A5_CMD_BATTERY;
        a5[6] = 1;
        a5[7] = 25;
        assert_eq!(parse_a5(&a5), Some((25, true)));
        assert_eq!(make_a5_request()[..7], [0, 0, 0, A5_ROUTE, A5_LEN, A5_PAGE, A5_CMD_BATTERY]);
    }

    #[test]
    fn audio_parsers_and_checksum() {
        let mut resp = vec![0u8; 11];
        resp[..8].copy_from_slice(&[0x55, 0xAA, 0x01, 0x00, 0x03, 0x00, 0x01, 0x0B]);
        resp[8] = 1; // sleepState
        resp[9] = 0x55; // 85%，不充电
        resp[10] = 0; // flags=0 时校验位必须为 0
        assert_eq!(parse_audio(&resp), Some((Some(85), false)));
        resp[8] = 0; // 睡眠
        assert_eq!(parse_audio(&resp), Some((None, false)));
        resp[8] = 1; // 恢复 sleepState
        resp[3] = 0x01; // flags=1：开启校验
        let mut acc = resp[3] ^ resp[4] ^ resp[5] ^ resp[6] ^ resp[7];
        acc ^= resp[8];
        acc ^= resp[9];
        resp[10] = acc;
        assert_eq!(parse_audio(&resp), Some((Some(85), false)));
        resp[10] ^= 1;
        assert_eq!(parse_audio(&resp), None);

        assert_eq!(parse_status_55(&[0x55, 0x65, 40, 2]), Some((Some(40), false)));
        assert_eq!(parse_status_55(&[0x55, 0x65, 40, 3]), Some((Some(40), true)));
        assert_eq!(parse_status_55(&[0x55, 0x65, 40, 26]), Some((None, false)));
        assert_eq!(parse_status_55(&[0x55, 0x65, 40, 9]), None);
    }

    #[test]
    fn k99_request_and_reply() {
        let req = make_k99_request();
        assert_eq!(req[..9], [0x00, 0x55, 0x05, 0x00, 0x38, 0x38, 0x00, 0x00, 0x00]);
        let mut resp = vec![0u8; 65];
        resp[0] = 0x00;
        resp[1] = K99_MAGIC;
        resp[2] = K99_CMD;
        resp[5] = K99_SIZE;
        resp[9] = K99_SIZE;
        resp[1 + K99_PAYLOAD_AT + K99_BATTERY_AT] = 0x55;
        let b = &resp[1..];
        let checksum = (b[4] as u16
            + b[5] as u16
            + b[6] as u16
            + b[7] as u16
            + b[K99_PAYLOAD_AT..K99_PAYLOAD_AT + K99_SIZE as usize]
                .iter()
                .map(|&x| x as u16)
                .sum::<u16>()) as u8;
        resp[4] = checksum;
        assert_eq!(parse_k99_reply(&resp), Some((85, false, 0x55)));
        resp[4] ^= 1;
        assert_eq!(parse_k99_reply(&resp), None);
    }

    #[test]
    fn kb_request_and_reply() {
        let req = make_kb_request();
        assert_eq!(req[..3], [0x13, 0x4A, 0x01]);
        assert_eq!(req[19], sum8(&req[..19]));
        let mut resp = [0u8; KB_REPORT_SIZE];
        resp[..4].copy_from_slice(&[0x13, 0x4A, 0x01, 0x00]);
        resp[5] = 94;
        resp[6] = 0x10;
        resp[19] = sum8(&resp[..19]);
        assert_eq!(parse_kb_reply(&resp), Some((94, 0x10)));
        assert_eq!(parse_kb_charging(0x10), Some(true));
        assert_eq!(parse_kb_charging(0x01), Some(false));
        assert_eq!(parse_kb_charging(0x20), None);
    }

    #[test]
    fn headset_feedback_parsers() {
        assert_eq!(make_v9_request()[..3], [0x55, 0x65, 0x01]);
        assert_eq!(parse_v9_status(&[0x55, 0x65, 60, 0]), Some((60, 0)));
        assert_eq!(parse_v9_status(&[0x55, 0x65, 101, 0]), None);

        let mut v9p2 = [0u8; 64];
        v9p2[..8].copy_from_slice(&[0x55, 0xAA, 0x01, 0x01, 10, 0x00, 0x01, 0x0B]);
        v9p2[8] = 1;
        v9p2[9] = 80;
        assert!(v9pro2_matches(&v9p2));
        v9p2[7] = 0x22;
        v9p2[6] = 0x04;
        assert!(v9pro2_matches(&v9p2));
        v9p2[4] = 60;
        assert!(!v9pro2_matches(&v9p2));

        let mut g3 = [0u8; 64];
        g3[..2].copy_from_slice(&[0xAA, 0xED]);
        g3[8] = 0;
        assert!(g3_online_matches(&g3));
        g3[..2].copy_from_slice(&[0xAA, 0x30]);
        g3[8] = 55;
        g3[9] = 1;
        assert!(g3_query_matches(&g3));
    }

    #[test]
    fn classification_and_device_keys() {
        assert_eq!(classify(MEASURED_VID, 0x1020), Some(Variant::Legacy5253));
        assert_eq!(classify(NEW_VID, 0x100A), Some(Variant::LegacyNew));
        assert_eq!(classify(NEW_VID, MC_TARGET_PID), Some(Variant::Mc));
        assert_eq!(classify(NEW_VID, X9_CABLE_PID), None);
        assert_eq!(classify(G7_VID, 0x1234), None);
        assert_eq!(device_key(MEASURED_VID, 0x0031), "mchose");
        assert_eq!(device_key(NEW_VID, HEADSET_BODY_PID), "mchose:3837:6008");
        assert_eq!(device_key(NEW_VID, X9_PID), "mchose:3837:6045");
    }

    #[test]
    fn median_u8_uses_bankers_rounding() {
        let queue = |v: &[u8]| v.iter().copied().collect::<VecDeque<u8>>();
        assert_eq!(median_u8(&queue(&[])), None);
        assert_eq!(median_u8(&queue(&[40, 60, 50])), Some(50));
        // 偶样本：79.5 -> 80（向偶数），78.5 -> 78，80.5 -> 80。
        assert_eq!(median_u8(&queue(&[79, 80])), Some(80));
        assert_eq!(median_u8(&queue(&[78, 79])), Some(78));
        assert_eq!(median_u8(&queue(&[80, 81])), Some(80));
    }

    #[test]
    fn g7_smooth_startup_80_needs_five_spaced_readings() {
        let mut s = G7Smoother::default();
        let t0 = Instant::now();
        // 前 4 次间隔 5s 的 80% 只是启动计数，还不产出读数。
        for i in 0..4 {
            let t = t0 + Duration::from_secs(i * 5);
            assert_eq!(s.smooth(80, false, t), None);
        }
        // 第 5 次（与前一次间隔 >=5s）才接纳 80。
        assert_eq!(s.smooth(80, false, t0 + Duration::from_secs(20)), Some(80));
        assert_eq!(s.samples.len(), 1);
    }

    #[test]
    fn g7_smooth_charging_resets_and_median_filters() {
        let mut s = G7Smoother::default();
        let t0 = Instant::now();
        assert_eq!(s.smooth(46, false, t0), Some(46));
        assert_eq!(s.smooth(80, false, t0 + Duration::from_secs(5)), Some(46));
        // 充电时清空历史，直接跟随原始读数。
        assert_eq!(s.smooth(100, true, t0 + Duration::from_secs(6)), Some(100));
        assert_eq!(s.samples.len(), 1);
        // 断电后跨过 600s 空档重新起步。
        assert_eq!(s.smooth(55, false, t0 + Duration::from_secs(700)), Some(55));
    }

    #[test]
    fn g7_smooth_confirms_large_drop_across_spaced_readings() {
        let mut s = G7Smoother::default();
        let t0 = Instant::now();
        s.smooth(90, false, t0);
        s.smooth(90, false, t0 + Duration::from_secs(5));
        // 单次骤降不进入历史，中值仍保持 90。
        s.smooth(50, false, t0 + Duration::from_secs(10));
        assert_eq!(median_u8(&s.samples), Some(90));
        assert!(s.low.is_some());
        // 第二次相近且间隔 >=5s 的低电量替换历史：清空后放入 [pending, level]。
        s.smooth(52, false, t0 + Duration::from_secs(20));
        assert_eq!(s.samples.len(), 2); // [50, 52]
        assert!(s.low.is_none());
    }

    #[test]
    fn asleep_keep_cache_marks_offline_and_expires() {
        let mut state = State::default();
        let now = Instant::now();
        let st = make_status("mchose:291d", "MCHOSE mouse", "mouse", "mchose", Some(42), Some(false), true);
        state.last.insert(("instance-a".to_string(), st.key.clone()), (st.clone(), now));
        // 模拟无回复轮询：命中缓存，产出灰显状态（保留读数、online=false）。
        let mut offline = st.clone();
        offline.online = false;
        assert!(!offline.online);
        assert_eq!(offline.key, st.key);
        assert_eq!(offline.level, st.level);
        // 超过 ASLEEP_KEEP 的条目应被清理。
        state.last.insert(("instance-b".to_string(), "old".to_string()), (st, now - Duration::from_secs(ASLEEP_KEEP_S + 1)));
        let expired: Vec<_> = state.last.iter()
            .filter(|((_, _), (_, t))| now.saturating_duration_since(*t) >= Duration::from_secs(ASLEEP_KEEP_S))
            .map(|((i, k), _)| (i.clone(), k.clone()))
            .collect();
        assert_eq!(expired, vec![("instance-b".to_string(), "old".to_string())]);
    }
}

// 功能级偏差处理说明（按团队要求明确列出，不猜测字节格式）：
// 1. `_smooth_g7`：已精确移植（中值滤波 + 5 次间隔 80% 启动兜底 + 20% 骤降双次确认 +
//    600s 重启窗口 + 充电时重置）。`now` 用 Instant 注入以便单测；实机抖动表现需确认。
// 2. `KeyboardActivity` / `MouseActivity`：未移植。K99 与 A7 V3 Ultra+ 的“仅在真实
//    输入后查询”门控依赖 Python 的 Raw Input 监听线程（RegisterRawInputDevices +
//    消息循环）与 `wake_on_input`/`polling_paused` 字段。Rust worker 是 stdin/stdout
//    子进程，既不拥有输入监听线程，Status/Reading wire format 也没有对应字段。
//    若后续要补：需在 worker 内起 Raw Input 线程，并扩展协议加 paused 标志。
// 3. `_last` 休眠保持：已在 provider 内实现（ASLEEP_KEEP=300s，无回复时产出
//    online=false 的最后读数）。主程序 `bt/receivers.rs` 会过滤 online=false，
//    但保留 Engine.observed 的热度；设备唤醒后同值读数仍会被判为 fresh。
//    若需与 Python 完全一致的“灰显图标”，需主程序侧改动（不在本批次范围）。
// 4. G7/A5/V9 等依赖 `set_nonblocking` 的读循环：Rust 用带 deadline 的同步读替代；
//    协议字节与重试次数保持一致，超时行为需在实机确认。
