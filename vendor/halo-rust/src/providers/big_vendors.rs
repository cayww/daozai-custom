//! 三大厂商的大型状态机电量协议：Corsair / Asus / Logitech。
//!
//! 严格按 `vendor/halo-battery/upstream/providers/` 下的 Python 参考实现移植：
//! 命令序列、应答字段位置、状态机与合法性判断均逐条对应，不猜测字节格式。
//! 无法在不暴露 HID path / product string 的 Rust Collection API 下安全移植的
//! 子协议，在文件末尾 `SKIPPED` 注释中逐条列出。
use crate::hid::{Collection, Handle};
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, Instant};

fn deadline(ms: u64) -> Instant {
    Instant::now() + Duration::from_millis(ms)
}

/// 丢弃接收器在本次请求前推送的陈旧帧。
fn drain(h: &mut Handle, attempts: usize, timeout_ms: u64) {
    for _ in 0..attempts {
        if h.read(deadline(timeout_ms)).is_none() {
            break;
        }
    }
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
    st.level = level;
    st.charging = charging;
    st.charging_known = charging_known;
    st.source = source.into();
    st
}

// ===========================================================================
// Corsair：VID 0x1B1C
// 参考 corsair.py（1440 行）。四个家族：
//   1) Void Elite（FFC5:0001）— C9 64 → 64 00 <level> B1 <status>
//   2) HS80 USB/接收器（接口 3 FF42）— 02 <target> 02 0f
//   3) Void v2（接口 4 FF42:0001）— 固件查询 / 心跳 / 电量 的唤醒握手
//   4) Bragi/Slipstream（FF42:0001）— 路由读属性，双次确认 + 充电属性
// ===========================================================================
mod corsair {
    use super::*;

    const VID: u16 = 0x1B1C;
    const USAGE_PAGE: u16 = 0xFF42;
    const USAGE: u16 = 0x0001;
    const CONTROL_INTERFACE: u8 = 4;
    const RECEIVER_ENDPOINT: u8 = 0x08;
    const HEADSET_ENDPOINT: u8 = 0x09;
    const MSG_SIZE_WRITE: usize = 65;
    const CMD_FIRMWARE: u8 = 0x13;
    const CMD_HEARTBEAT: u8 = 0x12;
    const CMD_BATTERY: u8 = 0x0F;
    const FW_SUB: u8 = 0x02;
    const HB_SUB: u8 = 0x02;
    const BATTERY_SUB: u8 = 0x02;
    const LEVEL_INDEX: usize = 4;
    const LEVEL_MAX: u16 = 1000;
    const ATTEMPTS: usize = 3;
    const READ_TIMEOUT_MS: u64 = 500;
    const FLUSH_TIMEOUT_MS: u64 = 30;

    // HS80 USB
    const HS80_USB_PID: u16 = 0x0A69;
    const HS80_CHARGING_CABLE_PID: u16 = 0x0A6A;
    const HS80_RECEIVER_PID: u16 = 0x0A6B;
    const HS80_USB_INTERFACE: u8 = 3;
    const HS80_USB_TARGET: u8 = 0x08;
    const HS80_USB_REPORT_SIZE: usize = 64;
    const HS80_USB_READ_ATTEMPTS: usize = 3;

    // Void v2（headsetcontrol 三件套）
    const PIDS: &[(u16, &str)] = &[
        (0x2A08, "Corsair Void v2 Wireless"),
        (0x2A02, "Corsair Virtuoso Max Wireless"),
        (0x0A97, "Corsair HS80 Max Wireless"),
    ];

    // Bragi / Slipstream: (pid, name, kind, key)
    const BRAGI_PIDS: &[(u16, &str, &str, &str)] = &[
        (0x1B7F, "Corsair Dark Core RGB Pro SE", "mouse", "corsair:1b7f"),
        (0x1BB5, "Corsair M65 RGB Ultra Wireless", "mouse", "corsair:m65"),
        (0x1BA6, "Corsair M65 RGB Ultra Wireless", "mouse", "corsair:m65"),
        (0x2B07, "Corsair K65 PLUS Wireless", "keyboard", "corsair:2b07"),
        (0x1BDC, "Corsair M75 Wireless", "mouse", "corsair:1bdc"),
        (0x0A40, "Corsair Virtuoso RGB Wireless SE", "headset", "corsair:virtuoso-se"),
        (0x0A3E, "Corsair Virtuoso RGB Wireless SE", "headset", "corsair:virtuoso-se"),
        (0x0A3D, "Corsair Virtuoso RGB Wireless SE", "headset", "corsair:virtuoso-se"),
        (0x0A64, "Corsair Virtuoso RGB Wireless XT", "headset", "corsair:virtuoso-xt"),
        (0x0A62, "Corsair Virtuoso RGB Wireless XT", "headset", "corsair:virtuoso-xt"),
    ];
    const BRAGI_VENDOR_PAGE: u16 = 0xFF42;
    const BRAGI_USAGE: u16 = 0x0001;
    const BRAGI_MSG_SIZE: usize = 64;
    const BRAGI_ROUTE_DONGLE: u8 = 0x08;
    const BRAGI_ROUTE_MOUSE: u8 = 0x09;
    const BRAGI_ROUTE_CHILD: u8 = 0x01;
    const BRAGI_CMD_GET: u8 = 0x02;
    const BRAGI_PROP_BATTERY: u8 = 0x0F;
    const BRAGI_PROP_CHARGE: u8 = 0x10;
    const BRAGI_PROP_HEARTBEAT: u8 = 0x12;
    const BRAGI_PROP_FIRMWARE: u8 = 0x13;
    const BRAGI_LEVEL_MAX: u16 = 1000;
    const BRAGI_WINDOW_S: u64 = 7_000; // BRAGI_POLL_BUDGET_S；实际问询上限由 min(6 s) 控制
    const BRAGI_CONFIRM_WINDOW_S: u64 = 3_000;
    const BRAGI_READ_ATTEMPTS: usize = 96;
    const M65_CABLE_PID: u16 = 0x1BB5;
    const BRAGI_XT_CABLE_PID: u16 = 0x0A62;
    const BRAGI_BARE_PIDS: &[u16] = &[0x0A40, 0x0A3E, 0x0A3D, 0x0A64, 0x0A62];
    const BRAGI_SESSION_PIDS: &[u16] = &[0x0A40];
    const BRAGI_WIRED_PIDS: &[u16] = &[0x0A3D, 0x0A62];
    const BRAGI_CHARGE_PIDS: &[u16] = &[
        0x0A40, 0x0A3E, 0x0A3D, 0x0A64, 0x0A62, 0x1BB5, 0x1BA6,
    ];

    // Void Elite（老 Void）
    const VOID_VENDOR_PAGE: u16 = 0xFFC5;
    const VOID_USAGE: u16 = 0x0001;
    const VOID_REPORT_ID: u8 = 0xC9;
    const VOID_ASK: u8 = 0x64;
    const VOID_REPLY_ID: u8 = 0x64;
    const VOID_MARKER: u8 = 0xB1;
    const VOID_PIDS: &[(u16, &str)] = &[(0x0A55, "Corsair Void Elite Wireless")];

    // SABRE v3 PRO（Slipstream V2）：独立实现见下
    const SABRE_V3_PIDS: &[(u16, &str)] = &[
        (0x2B43, "Corsair SABRE v3 PRO Ultralight"),
        (0x2B42, "Corsair SABRE v3 PRO Ultralight"),
    ];
    const SABRE_V3_CABLE_PID: u16 = 0x2B43;
    const SABRE_V3_KEY: &str = "corsair:sabre-v3";
    const SABRE_V3_MSG_SIZE: usize = 512;
    const SABRE_V3_ENDPOINT: u8 = 0x01;
    const SABRE_V3_CABLE_ENDPOINT: u8 = 0x00;
    const SABRE_V3_CMD_GET: u8 = 0x02;
    const SABRE_V3_PROP_BATTERY: u8 = 0x0F;
    const SABRE_V3_PROP_CHARGE: u8 = 0x10;
    #[allow(dead_code)] // Python _sabre_v3_read 的默认超时；询问循环用剩余窗口时间
    const SABRE_V3_READ_MS: u64 = 500;
    const SABRE_V3_REPLY_MS: u64 = 600;

    pub struct Corsair {
        state: Mutex<State>,
    }

    #[derive(Default)]
    struct State {
        /// 同一型号接收器/线缆共用一个图标：线缆的充电状态优先。
        bragi: HashMap<String, (u8, bool)>,
        /// HS80 线缆身份是否出现过（Python 用整轮 enumerate 判断）。
        hs80_cable_seen: bool,
    }

    impl Corsair {
        pub fn new() -> Self {
            Self { state: Mutex::new(State::default()) }
        }
    }

    impl Default for Corsair {
        fn default() -> Self {
            Self::new()
        }
    }

    // ---- Void v2 握手族 ----
    fn make_request(endpoint: u8, sub: u8, command: u8) -> [u8; MSG_SIZE_WRITE] {
        let mut frame = [0u8; MSG_SIZE_WRITE];
        frame[0] = 0x00;
        frame[1] = 0x02;
        frame[2] = endpoint;
        frame[3] = sub;
        frame[4] = command;
        frame
    }

    /// `_write_frame`：先发 64 字节无 report id，失败再发 65 字节带 id 的旧形式。
    fn write_frame(h: &mut Handle, frame: &[u8]) -> bool {
        if frame.len() > 1 && h.write(&frame[1..], deadline(READ_TIMEOUT_MS)) {
            return true;
        }
        h.write(frame, deadline(READ_TIMEOUT_MS))
    }

    fn write_cmd(h: &mut Handle, endpoint: u8, sub: u8, command: u8) -> bool {
        write_frame(h, &make_request(endpoint, sub, command))
    }

    /// `parse_level`：LEVEL_INDEX 处小端 16 位，0 或 >1000 都不是读数。
    fn parse_level(r: &[u8]) -> Option<u8> {
        if r.len() <= LEVEL_INDEX + 1 {
            return None;
        }
        let vendor = u16::from_le_bytes([r[LEVEL_INDEX], r[LEVEL_INDEX + 1]]);
        if vendor == 0 || vendor > LEVEL_MAX {
            return None;
        }
        Some((vendor / 10) as u8)
    }

    /// `_query`：接收器固件查询 → 接收器心跳 → 清空 → 头戴心跳 → 3 次电量请求。
    fn query_void_v2(h: &mut Handle) -> Option<u8> {
        if !write_cmd(h, RECEIVER_ENDPOINT, FW_SUB, CMD_FIRMWARE) {
            return None;
        }
        if !write_cmd(h, RECEIVER_ENDPOINT, HB_SUB, CMD_HEARTBEAT) {
            return None;
        }
        drain(h, 4, FLUSH_TIMEOUT_MS);
        if !write_cmd(h, HEADSET_ENDPOINT, HB_SUB, CMD_HEARTBEAT) {
            return None;
        }
        // 头戴无应答意味着关机/休眠，不再继续。
        if h.read(deadline(READ_TIMEOUT_MS)).is_none() {
            return None;
        }
        drain(h, 4, FLUSH_TIMEOUT_MS);
        for _ in 0..ATTEMPTS {
            if !write_cmd(h, HEADSET_ENDPOINT, BATTERY_SUB, CMD_BATTERY) {
                continue;
            }
            if let Some(r) = h.read(deadline(READ_TIMEOUT_MS)) {
                if let Some(level) = parse_level(&r) {
                    return Some(level);
                }
            }
        }
        None
    }

    // ---- HS80 USB / 接收器 ----
    fn hs80_request(target: u8) -> [u8; HS80_USB_REPORT_SIZE] {
        let mut frame = [0u8; HS80_USB_REPORT_SIZE];
        frame[0] = 0x02;
        frame[1] = target;
        frame[2] = 0x02;
        frame[3] = 0x0F;
        frame
    }

    /// `parse_hs80_usb_battery`：01 <target-08> 02 00 <tenths LE>。
    fn parse_hs80_usb(report: &[u8], target: u8) -> Option<u8> {
        if report.len() < 6 {
            return None;
        }
        if report[0] != 0x01
            || report[1] != target - 0x08
            || report[2] != 0x02
            || report[3] != 0x00
        {
            return None;
        }
        let tenths = u16::from_le_bytes([report[4], report[5]]);
        if tenths > LEVEL_MAX {
            return None;
        }
        Some((tenths / 10) as u8)
    }

    fn query_hs80(c: &Collection, target: u8) -> Option<u8> {
        let mut h = c.open()?;
        drain(&mut h, 4, FLUSH_TIMEOUT_MS);
        let request = hs80_request(target);
        if !h.write(&request, deadline(READ_TIMEOUT_MS)) {
            return None;
        }
        for _ in 0..HS80_USB_READ_ATTEMPTS {
            let Some(r) = h.read(deadline(READ_TIMEOUT_MS)) else {
                continue;
            };
            if let Some(level) = parse_hs80_usb(&r, target) {
                return Some(level);
            }
        }
        None
    }

    // ---- Bragi / Slipstream ----
    fn bragi_request(prop: u8, route: u8) -> [u8; BRAGI_MSG_SIZE + 1] {
        let mut buf = [0u8; BRAGI_MSG_SIZE + 1];
        buf[0] = route;
        buf[1] = BRAGI_CMD_GET;
        buf[2] = prop;
        buf
    }

    fn bragi_se_request(prop: u8, route: u8) -> [u8; BRAGI_MSG_SIZE] {
        let mut buf = [0u8; BRAGI_MSG_SIZE];
        buf[0] = 0x02;
        buf[1] = route;
        buf[2] = BRAGI_CMD_GET;
        buf[3] = prop;
        buf
    }

    /// `parse_bragi_raw`：Dark Core `01 02 00 <v LE>`，Virtuoso `01 01 02 00 <v LE>`。
    fn parse_bragi_raw(reply: &[u8]) -> Option<u16> {
        let data = if reply.len() >= BRAGI_MSG_SIZE + 1 {
            &reply[1..]
        } else {
            reply
        };
        if data.len() < 5 || data[0] != BRAGI_ROUTE_CHILD {
            return None;
        }
        if data[1] == BRAGI_CMD_GET && data[2] == 0x00 {
            return Some(u16::from_le_bytes([data[3], data[4]]));
        }
        if data.len() >= 6
            && data[1] == BRAGI_ROUTE_CHILD
            && data[2] == BRAGI_CMD_GET
            && data[3] == 0x00
        {
            return Some(u16::from_le_bytes([data[4], data[5]]));
        }
        None
    }

    fn parse_bragi(reply: &[u8]) -> Option<u8> {
        let v = parse_bragi_raw(reply)?;
        if v == 0 || v > BRAGI_LEVEL_MAX {
            return None;
        }
        Some((v / 10) as u8)
    }

    fn parse_bragi_charge(reply: &[u8]) -> Option<bool> {
        match parse_bragi_raw(reply)? {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    }

    /// 有线 M65：`00 02 00 <v LE>`，或 Windows 把 report id 单独交回的形式。
    fn parse_bragi_cable_raw(reply: &[u8]) -> Option<u16> {
        if reply.len() >= 5 && reply[0] == 0x00 && reply[1] == BRAGI_CMD_GET && reply[2] == 0x00 {
            return Some(u16::from_le_bytes([reply[3], reply[4]]));
        }
        if reply.len() >= 4 && reply[0] == BRAGI_CMD_GET && reply[1] == 0x00 {
            return Some(u16::from_le_bytes([reply[2], reply[3]]));
        }
        None
    }

    fn parse_bragi_cable(reply: &[u8]) -> Option<u8> {
        let v = parse_bragi_cable_raw(reply)?;
        if v == 0 || v > BRAGI_LEVEL_MAX {
            return None;
        }
        Some((v / 10) as u8)
    }

    fn parse_bragi_charge_cable_state(reply: &[u8]) -> Option<u8> {
        let v = parse_bragi_cable_raw(reply)?;
        if matches!(v, 0..=3) {
            Some(v as u8)
        } else {
            None
        }
    }

    /// 有线 XT：`01 00 02 00 <u32 LE>`，位置严格按 HeadsetControl 的 0A62。
    fn parse_bragi_xt_cable(reply: &[u8], charging: bool) -> Option<BragiValue> {
        let data = if reply.len() == BRAGI_MSG_SIZE + 1 && reply[0] == 0 {
            &reply[1..]
        } else {
            reply
        };
        if data.len() != BRAGI_MSG_SIZE || data[..4] != [0x01, 0x00, 0x02, 0x00] {
            return None;
        }
        let value = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if charging {
            match value {
                1 => Some(BragiValue::Charge(true)),
                2 => Some(BragiValue::Charge(false)),
                _ => None,
            }
        } else if value <= BRAGI_LEVEL_MAX as u32 {
            Some(BragiValue::Level((value / 10) as u8))
        } else {
            None
        }
    }

    enum BragiValue {
        Level(u8),
        Charge(bool),
    }

    impl BragiValue {
        fn level(self) -> Option<u8> {
            match self {
                BragiValue::Level(v) => Some(v),
                BragiValue::Charge(_) => None,
            }
        }
        fn charge(self) -> Option<bool> {
            match self {
                BragiValue::Charge(v) => Some(v),
                BragiValue::Level(_) => None,
            }
        }
    }

    fn write_bragi(h: &mut Handle, prop: u8, route: u8, pid: u16) -> bool {
        if BRAGI_BARE_PIDS.contains(&pid) {
            let req = bragi_se_request(prop, route);
            return h.write(&req, deadline(READ_TIMEOUT_MS));
        }
        let req = bragi_request(prop, route);
        if h.write(&req, deadline(READ_TIMEOUT_MS)) {
            return true;
        }
        h.write(&req[1..], deadline(READ_TIMEOUT_MS))
    }

    fn ask_bragi(h: &mut Handle, window_ms: u64, pid: u16, route: u8, charge: bool) -> Option<u8> {
        let end = Instant::now() + Duration::from_millis(window_ms);
        for _ in 0..BRAGI_READ_ATTEMPTS {
            if Instant::now() >= end {
                return None;
            }
            let Some(r) = h.read(deadline(READ_TIMEOUT_MS)) else {
                continue;
            };
            let value = if pid == M65_CABLE_PID {
                if charge {
                    parse_bragi_charge_cable_state(&r)
                } else {
                    parse_bragi_cable(&r)
                }
            } else if pid == BRAGI_XT_CABLE_PID && route == BRAGI_ROUTE_DONGLE {
                let Some(parsed) = parse_bragi_xt_cable(&r, charge) else {
                    continue;
                };
                if charge {
                    parsed.charge().map(|b| b as u8)
                } else {
                    parsed.level()
                }
            } else if charge {
                parse_bragi_charge(&r).map(|b| b as u8)
            } else {
                parse_bragi(&r)
            };
            if let Some(v) = value {
                return Some(v);
            }
        }
        None
    }

    fn read_window(h: &mut Handle, window_ms: u64) {
        let end = Instant::now() + Duration::from_millis(window_ms);
        for _ in 0..BRAGI_READ_ATTEMPTS {
            if Instant::now() >= end {
                return;
            }
            let _ = h.read(deadline(READ_TIMEOUT_MS));
        }
    }

    /// 0A40 实测会话：固件查询 → 接收器心跳 → 头戴心跳，每步后读回一个短窗口。
    fn bragi_session(h: &mut Handle, end: Instant, pid: u16) {
        for (prop, route) in [
            (BRAGI_PROP_FIRMWARE, BRAGI_ROUTE_DONGLE),
            (BRAGI_PROP_HEARTBEAT, BRAGI_ROUTE_DONGLE),
            (BRAGI_PROP_HEARTBEAT, BRAGI_ROUTE_MOUSE),
        ] {
            if Instant::now() >= end {
                return;
            }
            if !write_bragi(h, prop, route, pid) {
                return;
            }
            let left = end.saturating_duration_since(Instant::now()).as_millis().min(800) as u64;
            read_window(h, left);
        }
    }

    fn routes(pid: u16) -> &'static [u8] {
        if pid == M65_CABLE_PID {
            return &[BRAGI_ROUTE_DONGLE];
        }
        if BRAGI_WIRED_PIDS.contains(&pid) {
            return &[BRAGI_ROUTE_DONGLE, BRAGI_ROUTE_MOUSE];
        }
        if pid == 0x0A3E || pid == 0x0A64 {
            return &[BRAGI_ROUTE_MOUSE, BRAGI_ROUTE_DONGLE];
        }
        &[BRAGI_ROUTE_MOUSE]
    }

    fn query_bragi(c: &Collection, pid: u16) -> Option<(u8, Option<bool>)> {
        let mut h = c.open()?;
        drain(&mut h, 4, FLUSH_TIMEOUT_MS);
        let end = Instant::now() + Duration::from_millis(BRAGI_WINDOW_S);
        if BRAGI_SESSION_PIDS.contains(&pid) {
            bragi_session(&mut h, end, pid);
        }
        let mut first = None;
        let mut route = BRAGI_ROUTE_MOUSE;
        for (i, r) in routes(pid).iter().enumerate() {
            route = *r;
            if !write_bragi(&mut h, BRAGI_PROP_BATTERY, route, pid) {
                return None;
            }
            let left = end.saturating_duration_since(Instant::now()).as_millis().min(BRAGI_WINDOW_S as u128) as u64;
            first = ask_bragi(&mut h, left, pid, route, false);
            if first.is_some() {
                break;
            }
            if i + 1 < routes(pid).len() {
                continue;
            }
        }
        let first = first?;
        if !write_bragi(&mut h, BRAGI_PROP_BATTERY, route, pid) {
            return None;
        }
        let left = end
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(BRAGI_CONFIRM_WINDOW_S as u128) as u64;
        if let Some(second) = ask_bragi(&mut h, left, pid, route, false) {
            if second != first {
                // 两次回答不一致：拒绝显示，避免其他应用的应答被误判为电量。
                return None;
            }
        }
        let charging = if BRAGI_CHARGE_PIDS.contains(&pid)
            && Instant::now() < end
            && write_bragi(&mut h, BRAGI_PROP_CHARGE, route, pid)
        {
            let left = end
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(BRAGI_CONFIRM_WINDOW_S as u128) as u64;
            ask_bragi(&mut h, left, pid, route, true).map(|b| b != 0)
        } else {
            None
        };
        Some((first, charging))
    }

    // ---- Void Elite（FFC5:0001）----
    fn parse_void(reply: &[u8]) -> Option<(u8, bool)> {
        if reply.len() < 5 || reply[0] != VOID_REPLY_ID || reply[3] != VOID_MARKER {
            return None;
        }
        let status = reply[4];
        if status == 0 {
            return None;
        }
        let level = reply[2] & 0x7F;
        if level > 100 {
            return None;
        }
        Some((level, status == 4 || status == 5))
    }

    fn query_void(c: &Collection) -> Option<(u8, bool)> {
        let mut h = c.open()?;
        let mut forms: Vec<Vec<u8>> = vec![vec![VOID_REPORT_ID, VOID_ASK]];
        let mut padded = vec![VOID_REPORT_ID, VOID_ASK];
        padded.resize(MSG_SIZE_WRITE, 0);
        forms.push(padded);
        for form in forms {
            if !h.write(&form, deadline(READ_TIMEOUT_MS)) {
                continue;
            }
            for _ in 0..ATTEMPTS {
                let Some(r) = h.read(deadline(READ_TIMEOUT_MS)) else {
                    break;
                };
                if let Some(got) = parse_void(&r) {
                    return Some(got);
                }
            }
        }
        None
    }

    // ---- SABRE v3（Slipstream V2）----
    fn sabre_frame(prop: u8, endpoint: u8, size: usize) -> Vec<u8> {
        let mut frame = vec![0u8; size];
        frame[1] = endpoint;
        frame[2] = 0x01;
        frame[3] = 0x01;
        frame[4] = SABRE_V3_CMD_GET;
        frame[5] = prop;
        frame
    }

    fn sabre_reply(r: &[u8], endpoint: u8, cmd: u8) -> Option<(u8, &[u8])> {
        for start in 0..=1 {
            if r.len() < start + 5 {
                break;
            }
            if start == 1 && r[0] != 0x00 {
                break;
            }
            if r[start] == endpoint && r[start + 3] == cmd {
                return Some((r[start + 4], &r[start + 5..]));
            }
        }
        None
    }

    fn sabre_level(data: &[u8]) -> Option<u8> {
        if data.len() < 2 {
            return None;
        }
        let raw = u16::from_le_bytes([data[0], data[1]]);
        if !(10..=1000).contains(&raw) {
            return None;
        }
        Some((raw / 10) as u8)
    }

    fn sabre_charge(data: &[u8]) -> Option<bool> {
        match data.first().copied()? {
            1 => Some(true),
            2 | 3 => Some(false),
            _ => None,
        }
    }

    fn sabre_ask(h: &mut Handle, frame: &[u8], endpoint: u8) -> Option<(u8, Vec<u8>)> {
        drain(h, 8, FLUSH_TIMEOUT_MS);
        if !h.write(frame, deadline(READ_TIMEOUT_MS)) && frame.len() > 1 {
            let _ = h.write(&frame[1..], deadline(READ_TIMEOUT_MS));
        }
        let end = deadline(SABRE_V3_REPLY_MS);
        while Instant::now() < end {
            let left = end.saturating_duration_since(Instant::now()).as_millis().min(u128::from(u32::MAX)) as u64;
            let Some(r) = h.read(deadline(left)) else {
                break;
            };
            if let Some((status, data)) = sabre_reply(&r, endpoint, SABRE_V3_CMD_GET) {
                return Some((status, data.to_vec()));
            }
        }
        None
    }

    fn query_sabre(c: &Collection, pid: u16) -> Option<(u8, Option<bool>)> {
        let (endpoint, size) = if pid == SABRE_V3_CABLE_PID {
            (SABRE_V3_CABLE_ENDPOINT, 64 + 1)
        } else {
            (SABRE_V3_ENDPOINT, SABRE_V3_MSG_SIZE + 1)
        };
        let mut h = c.open()?;
        let (status, data) = sabre_ask(
            &mut h,
            &sabre_frame(SABRE_V3_PROP_BATTERY, endpoint, size),
            endpoint,
        )?;
        if status != 0 {
            return None;
        }
        let level = sabre_level(&data)?;
        let charging = sabre_ask(
            &mut h,
            &sabre_frame(SABRE_V3_PROP_CHARGE, endpoint, size),
            endpoint,
        )
        .and_then(|(status, data)| if status == 0 { sabre_charge(&data) } else { None });
        Some((level, charging))
    }

    impl Provider for Corsair {
        fn name(&self) -> &'static str {
            "corsair"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID {
                return Vec::new();
            }
            let mut state = self.state.lock().unwrap();

            // HS80：线缆身份先记下，供接收器读数判断 charging。
            if pid == HS80_USB_PID || pid == HS80_CHARGING_CABLE_PID {
                state.hs80_cable_seen = true;
            }

            // 1) HS80 直接 USB / 接收器（接口 3 FF42）
            if pid == HS80_USB_PID || pid == HS80_RECEIVER_PID {
                let target = if pid == HS80_USB_PID {
                    HS80_USB_TARGET
                } else {
                    HEADSET_ENDPOINT
                };
                for c in collections {
                    if c.interface != Some(HS80_USB_INTERFACE) || c.usage_page != 0xFF42 {
                        continue;
                    }
                    if let Some(level) = query_hs80(c, target) {
                        let charging = target == HS80_USB_TARGET || state.hs80_cable_seen;
                        return vec![make_status(
                            "corsair:hs80",
                            "Corsair HS80 RGB Wireless",
                            "headset",
                            "corsair",
                            Some(level),
                            Some(charging),
                            true,
                        )];
                    }
                }
                return Vec::new();
            }

            // 2) Void v2 三件套（接口 4 FF42:0001）
            if let Some((_, name)) = PIDS.iter().find(|(p, _)| *p == pid) {
                for c in collections {
                    if c.interface != Some(CONTROL_INTERFACE)
                        || (c.usage_page, c.usage) != (USAGE_PAGE, USAGE)
                    {
                        continue;
                    }
                    let Some(mut h) = c.open() else {
                        continue;
                    };
                    if let Some(level) = query_void_v2(&mut h) {
                        return vec![make_status(
                            format!("corsair:{pid:04x}"),
                            *name,
                            "headset",
                            "corsair",
                            Some(level),
                            Some(false),
                            true,
                        )];
                    }
                }
                return Vec::new();
            }

            // 3) SABRE v3（FF42:0001）
            if let Some((_, name)) = SABRE_V3_PIDS.iter().find(|(p, _)| *p == pid) {
                let key = if pid == SABRE_V3_CABLE_PID {
                    (SABRE_V3_CABLE_ENDPOINT, 64 + 1)
                } else {
                    (SABRE_V3_ENDPOINT, SABRE_V3_MSG_SIZE + 1)
                };
                for c in collections {
                    if (c.usage_page, c.usage) != (BRAGI_VENDOR_PAGE, BRAGI_USAGE) {
                        continue;
                    }
                    if c.output_len != 0 && c.output_len < key.1 {
                        continue;
                    }
                    if let Some((level, charging)) = query_sabre(c, pid) {
                        let via = if pid == SABRE_V3_CABLE_PID {
                            "USB cable"
                        } else {
                            "2.4 GHz receiver"
                        };
                        let mut st = make_status(
                            SABRE_V3_KEY,
                            *name,
                            "mouse",
                            "corsair",
                            Some(level),
                            Some(charging.unwrap_or(true)),
                            charging.is_some(),
                        );
                        st.source = format!("corsair ({via})");
                        return vec![st];
                    }
                }
                return Vec::new();
            }

            // 4) Bragi / Slipstream
            if let Some((_, name, kind, key)) = BRAGI_PIDS.iter().find(|(p, _, _, _)| *p == pid) {
                // M65 接收器：线缆已读到电量时不再询问（Python 同样跳过）。
                if pid == 0x1BA6 && state.bragi.contains_key("corsair:m65") {
                    return Vec::new();
                }
                for c in collections {
                    if (c.usage_page, c.usage) != (BRAGI_VENDOR_PAGE, BRAGI_USAGE) {
                        continue;
                    }
                    let Some((level, charging)) = query_bragi(c, pid) else {
                        continue;
                    };
                    let charging = charging.unwrap_or(false);
                    let kept = state.bragi.get(*key).copied();
                    if kept.is_none_or(|(_, old)| charging && !old) {
                        state.bragi.insert((*key).to_string(), (level, charging));
                    }
                    let (level, charging) = state.bragi[*key];
                    return vec![make_status(
                        *key,
                        *name,
                        kind,
                        "corsair",
                        Some(level),
                        Some(charging),
                        true,
                    )];
                }
                return Vec::new();
            }

            // 5) Void Elite（FFC5:0001）
            if let Some((_, name)) = VOID_PIDS.iter().find(|(p, _)| *p == pid) {
                for c in collections {
                    if (c.usage_page, c.usage) != (VOID_VENDOR_PAGE, VOID_USAGE) {
                        continue;
                    }
                    if let Some((level, charging)) = query_void(c) {
                        return vec![make_status(
                            format!("corsair:void:{pid:04x}"),
                            *name,
                            "headset",
                            "corsair",
                            Some(level),
                            Some(charging),
                            true,
                        )];
                    }
                }
            }
            Vec::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn void_v2_request_and_parse() {
            let frame = make_request(HEADSET_ENDPOINT, BATTERY_SUB, CMD_BATTERY);
            assert_eq!(frame.len(), 65);
            assert_eq!(&frame[..5], &[0x00, 0x02, 0x09, 0x02, 0x0F]);
            let mut reply = [0u8; 64];
            reply[LEVEL_INDEX] = 0xE8;
            reply[LEVEL_INDEX + 1] = 0x03;
            assert_eq!(parse_level(&reply), Some(100));
            reply[LEVEL_INDEX] = 0x00;
            reply[LEVEL_INDEX + 1] = 0x00;
            assert_eq!(parse_level(&reply), None);
        }

        #[test]
        fn hs80_request_and_parse() {
            let req = hs80_request(HS80_USB_TARGET);
            assert_eq!(&req[..4], &[0x02, 0x08, 0x02, 0x0F]);
            let mut reply = [0u8; 64];
            reply[..6].copy_from_slice(&[0x01, 0x00, 0x02, 0x00, 0xE8, 0x03]);
            assert_eq!(parse_hs80_usb(&reply, HS80_USB_TARGET), Some(100));
            reply[1] = 0x01;
            assert_eq!(parse_hs80_usb(&reply, HS80_USB_TARGET), None);
        }

        #[test]
        fn bragi_request_shapes_and_parse() {
            let dark = bragi_request(BRAGI_PROP_BATTERY, BRAGI_ROUTE_MOUSE);
            assert_eq!(dark.len(), 65);
            assert_eq!(&dark[..4], &[0x09, 0x02, 0x0F, 0x00]);
            let se = bragi_se_request(BRAGI_PROP_BATTERY, BRAGI_ROUTE_MOUSE);
            assert_eq!(se.len(), 64);
            assert_eq!(&se[..4], &[0x02, 0x09, 0x02, 0x0F]);
            let mut reply = [0u8; 65];
            reply[0] = 0x00;
            reply[1..6].copy_from_slice(&[0x01, 0x02, 0x00, 0xE8, 0x03]);
            assert_eq!(parse_bragi(&reply), Some(100));
            assert_eq!(parse_bragi_charge(&reply), None);
            reply[4] = 0x01;
            reply[5] = 0x00;
            assert_eq!(parse_bragi_charge(&reply), Some(true));
            // Virtuoso：01 01 02 00 <v LE>
            let mut virt = [0u8; 64];
            virt[..6].copy_from_slice(&[0x01, 0x01, 0x02, 0x00, 0xD4, 0x03]);
            assert_eq!(parse_bragi(&virt), Some(98));
        }

        #[test]
        fn void_elite_parse() {
            assert_eq!(parse_void(&[0x64, 0x00, 0x4B, 0xB1, 0x04]), Some((75, true)));
            assert_eq!(parse_void(&[0x64, 0x00, 0x4B, 0xB1, 0x00]), None);
            // 位 7 是麦克风抬起标志，不属于电量；状态 2 仍是在电池。
            assert_eq!(parse_void(&[0x64, 0x00, 0xCB, 0xB1, 0x02]), Some((75, false)));
            assert_eq!(parse_void(&[0x64, 0x00, 0x4B, 0x00, 0x02]), None);
        }

        #[test]
        fn sabre_v3_frame_and_parse() {
            let frame = sabre_frame(SABRE_V3_PROP_BATTERY, SABRE_V3_ENDPOINT, 513);
            assert_eq!(frame.len(), 513);
            assert_eq!(&frame[..6], &[0x00, 0x01, 0x01, 0x01, 0x02, 0x0F]);
            let reply = [0x01, 0x00, 0x01, 0x02, 0x00, 0x84, 0x03];
            let (status, data) = sabre_reply(&reply, SABRE_V3_ENDPOINT, SABRE_V3_CMD_GET).unwrap();
            assert_eq!(status, 0);
            assert_eq!(sabre_level(data), Some(90));
        }
    }
}

// ===========================================================================
// Asus：VID 0x0B05
// 参考 asus.py（770 行）。核心：G-Helper 的 00 12 07 电量命令，
// 加 ROG OMNI 接收器的配对列表 / 鼠标 / 键盘集合，以及两个耳机的 feature 报告。
// ===========================================================================
mod asus {
    use super::*;

    const VID: u16 = 0x0B05;
    const INTERFACE: u8 = 0;
    const REQUEST: &[u8] = &[0x00, 0x12, 0x07];
    const ECHO: (u8, u8) = (0x12, 0x07);
    const ERROR: (u8, u8) = (0xFF, 0xAA);
    const PACKET_LENGTH: usize = 65;
    const LEVEL_BYTE: usize = 4;
    const CHARGING_BYTE: usize = 9;
    const READ_TIMEOUT_MS: u64 = 300;
    const READ_ATTEMPTS: usize = 8;
    const WRITE_ATTEMPTS: usize = 3;
    const PERCENT: u8 = 1;
    const STEPS: u8 = 25;

    // pid -> (name, scale)
    const KNOWN: &[(u16, &str, u8)] = &[
        (0x1A72, "ROG Gladius III Aimpoint", PERCENT),
        (0x1A70, "ROG Gladius III Aimpoint", PERCENT),
        (0x1B0C, "ROG Gladius III Eva 2", PERCENT),
        (0x1B0A, "ROG Gladius III Eva 2", PERCENT),
        (0x197F, "ROG Gladius III Wireless", PERCENT),
        (0x197D, "ROG Gladius III Wireless", PERCENT),
        (0x1A1A, "ROG Chakram X", PERCENT),
        (0x1A18, "ROG Chakram X", PERCENT),
        (0x1A94, "ROG Harpe Ace Aim Lab Edition", PERCENT),
        (0x1A92, "ROG Harpe Ace Aim Lab Edition", PERCENT),
        (0x1A68, "ROG Keris Wireless Aimpoint", PERCENT),
        (0x1A66, "ROG Keris Wireless Aimpoint", PERCENT),
        (0x1979, "ROG Spatha X", PERCENT),
        (0x1977, "ROG Spatha X", PERCENT),
        (0x19F4, "TUF Gaming M4 Wireless", PERCENT),
        (0x1A8D, "TX Gaming Mouse", PERCENT),
        (0x1AF5, "TX Gaming Mouse Mini", PERCENT),
        (0x1AF3, "TX Gaming Mouse Mini", PERCENT),
        (0x1C5A, "TX Gaming Mouse Mini Miku", PERCENT),
        (0x1C57, "TUF Gaming Mini Miku Edition", PERCENT),
        (0x1C56, "TUF Gaming Mini Miku Edition", PERCENT),
        (0x18E5, "ROG Chakram", STEPS),
        (0x18E3, "ROG Chakram", STEPS),
        (0x1960, "ROG Keris Wireless", STEPS),
        (0x195E, "ROG Keris Wireless", STEPS),
        (0x1A59, "ROG Keris EVA Edition", STEPS),
        (0x1A57, "ROG Keris EVA Edition", STEPS),
        (0x1908, "ROG Pugio II", STEPS),
        (0x1906, "ROG Pugio II", STEPS),
        (0x1949, "ROG Strix Impact II Wireless", STEPS),
        (0x1947, "ROG Strix Impact II Wireless", STEPS),
    ];

    // ROG OMNI 接收器
    const OMNI_PID: u16 = 0x1ACE;
    const OMNI_INTERFACE: u8 = 2;
    const OMNI_CONTROL: u16 = 0xFF02;
    const OMNI_KEYBOARD: u16 = 0xFF00;
    const OMNI_MOUSE: u16 = 0xFF01;
    const OMNI_PACKET_LENGTH: usize = 64;
    const OMNI_PAIRS: &[u8] = &[0x01, 0xA0, 0x00, 0x00];
    const OMNI_SLOT: usize = 4;
    const OMNI_FIRST_SLOT: usize = 5;
    const OMNI_MOUSE_REQUEST: &[u8] = &[0x03, 0x12, 0x07];
    const OMNI_KEYBOARD_REQUEST: &[u8] = &[0x02, 0x12, 0x01];
    const KEYBOARD_ECHO: (u8, u8) = (0x12, 0x01);
    const KEYBOARD_CHARGING_BYTE: usize = 9;
    const OMNI_MICE: &[(u16, &str)] = &[
        (0x1B65, "ROG Harpe Ace Mini"),
        (0x1C0E, "ROG Keris II Origin"),
        (0x1D4E, "ROG Keris II Origin KJP"),
        (0x1A94, "ROG Harpe Ace Aim Lab Edition"),
        (0x1AD7, "ROG Strix Impact III Wireless"),
        (0x1A72, "ROG Gladius III Aimpoint"),
        (0x1A68, "ROG Keris Wireless Aimpoint"),
        (0x1A6A, "ROG Keris Wireless Aimpoint"),
        (0x1B1A, "ROG Keris II Ace"),
        (0x1B18, "ROG Keris II Ace"),
        (0x1B68, "ROG Harpe Ace Extreme"),
        (0x1B69, "ROG Harpe Ace Extreme"),
    ];
    const OMNI_KEYBOARDS: &[(u16, &str, usize)] = &[
        (0x1A85, "ROG Azoth", 6),
        (0x1B42, "ROG Azoth Extreme", 6),
        (0x1CF1, "ROG Azoth Extreme SE", 6),
        (0x1C25, "ROG Azoth X", 6),
        (0x1AB0, "ROG Strix Scope II 96 Wireless", 6),
        (0x1B7A, "ROG Strix Scope II 96 RX Wireless", 6),
        (0x1B06, "ROG Falchion RX Low Profile", 11),
        (0x1CEA, "ROG Strix Morph 96 Wireless", 6),
    ];

    // SpeedNova / Harpe II
    const HARPE_II_RECEIVER: u16 = 0x1AD0;
    const HARPE_II_CABLE: u16 = 0x1C69;
    const SPEEDNOVA_KEYBOARD_PID: u16 = 0x1C12;
    const SPEEDNOVA_KEYBOARD_NAME: &str = "ROG Azoth 96 HE";

    // 耳机 feature 报告
    const HEADSET_PID: u16 = 0x18D6;
    const HEADSET_WIRED_PID: u16 = 0x18D7;
    const HEADSET_NAME: &str = "ROG Strix Go 2.4";
    const HEADSET_REPORT_ID: u8 = 0xFF;
    const HEADSET_REQUEST: &[u8] = &[
        0xFF, 0x08, 0x00, 0xFD, 0x04, 0x12, 0xF1, 0x03, 0x52, 0x01,
    ];
    const HEADSET_PACKET_LENGTH: usize = 64;
    const HEADSET_VOLTAGE_LSB: usize = 11;
    const HEADSET_VOLTAGE_MSB: usize = 12;
    const HEADSET_CHARGING_BYTE: usize = 9;
    const HEADSET_CHARGING_VALUE: u8 = 0x0A;
    const HEADSET_CHARGING_LIFT_MV: u16 = 160;
    const HEADSET_CHARGING_FULL_MV: u16 = 4100;
    const HEADSET_HEADER: u8 = 0x1B;
    const HEADSET_SLEEP_MS: u64 = 35;
    const HEADSET_ATTEMPTS: usize = 3;
    const HEADSET_ERROR: (u8, u8) = (0xFF, 0xAA);

    const TUF_H3_PID: u16 = 0x1963;
    const TUF_H3_NAME: &str = "TUF Gaming H3 Wireless";
    const TUF_H3_REQUEST: &[u8] = &[0xFF, 0x07, 0x00, 0xFD, 0x04, 0x0C, 0xF1, 0x02, 0x01];
    const TUF_H3_HEADER: u8 = 0x12;
    const TUF_H3_MV_RANGE: (u16, u16) = (3000, 4500);
    const TUF_H3_STEP: u8 = 25;
    const TUF_H3_RECHECK_MS: u64 = 3_000;
    const TUF_H3_RECHECKS: usize = 2;

    pub struct Asus {
        state: Mutex<State>,
    }

    #[derive(Default)]
    struct State {
        /// TUF H3 上次确认的充电状态（跨轮询保持）。
        settled: HashMap<String, bool>,
    }

    impl Asus {
        pub fn new() -> Self {
            Self { state: Mutex::new(State::default()) }
        }
    }

    impl Default for Asus {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Clone, Copy)]
    struct Reading {
        level: u8,
        charging: bool,
        approximate: bool,
    }

    fn offset(r: &[u8], pair: (u8, u8)) -> Option<usize> {
        if r.len() > 1 && (r[0], r[1]) == pair {
            return Some(0);
        }
        if r.len() > 2 && r[0] == 0x00 && (r[1], r[2]) == pair {
            return Some(1);
        }
        None
    }

    fn is_error(r: &[u8]) -> bool {
        offset(r, ERROR).is_some()
    }

    fn body<'a>(r: &'a [u8], rid: u8) -> &'a [u8] {
        if rid != 0 && !r.is_empty() && r[0] == rid {
            &r[1..]
        } else {
            r
        }
    }

    /// `_ask`：清空 → 最多 3 次写 + 每次 8 次读，返回第一个 wanted 接受的帧。
    fn ask(
        c: &Collection,
        request: &[u8],
        wanted: fn(&[u8]) -> bool,
        length: usize,
    ) -> Option<Vec<u8>> {
        let mut h = c.open()?;
        drain(&mut h, 16, 5);
        let rid = request[0];
        let mut frame = vec![0u8; length];
        frame[..request.len()].copy_from_slice(request);
        for _ in 0..WRITE_ATTEMPTS {
            if !h.write(&frame, deadline(READ_TIMEOUT_MS)) {
                return None;
            }
            for _ in 0..READ_ATTEMPTS {
                let Some(r) = h.read(deadline(READ_TIMEOUT_MS)) else {
                    break;
                };
                let b = body(&r, rid);
                if is_error(b) || b.iter().all(|&x| x == 0) {
                    return None;
                }
                if wanted(&r) {
                    return Some(r);
                }
            }
        }
        None
    }

    fn has_echo(r: &[u8]) -> bool {
        offset(r, ECHO).is_some()
    }

    fn parse_reply(r: &[u8], scale: u8) -> Option<Reading> {
        let m = offset(r, ECHO)?;
        if r.len() < m + CHARGING_BYTE + 1 {
            return None;
        }
        let raw = r[m + LEVEL_BYTE];
        let charging = r[m + CHARGING_BYTE] != 0;
        if raw == 0 && !charging {
            return None; // 待机，不是空电
        }
        if scale == STEPS {
            if raw > 4 {
                return None;
            }
            return Some(Reading { level: raw * STEPS, charging, approximate: true });
        }
        if raw > 100 {
            return None;
        }
        Some(Reading { level: raw, charging, approximate: false })
    }

    fn pair_pids(r: &[u8]) -> Vec<u16> {
        if r.len() < 2 || (r[0], r[1]) != (OMNI_PAIRS[0], OMNI_PAIRS[1]) {
            return Vec::new();
        }
        let mut pids = Vec::new();
        let mut i = OMNI_FIRST_SLOT;
        while i + 1 < r.len() {
            let pid = u16::from_le_bytes([r[i], r[i + 1]]);
            if pid == 0 {
                break;
            }
            pids.push(pid);
            i += OMNI_SLOT;
        }
        pids
    }

    fn is_omni_mouse(r: &[u8]) -> bool {
        r.len() > 2 && r[0] == OMNI_MOUSE_REQUEST[0] && (r[1], r[2]) == ECHO
    }

    fn is_omni_keyboard(r: &[u8]) -> bool {
        r.len() > 2 && r[0] == OMNI_KEYBOARD_REQUEST[0] && (r[1], r[2]) == KEYBOARD_ECHO
    }

    fn parse_omni_mouse(r: &[u8]) -> Option<Reading> {
        if !is_omni_mouse(r) {
            return None;
        }
        parse_reply(&r[1..], PERCENT)
    }

    fn parse_omni_keyboard(r: &[u8], battery_byte: usize) -> Option<Reading> {
        if !is_omni_keyboard(r) || r.len() <= battery_byte.max(KEYBOARD_CHARGING_BYTE) {
            return None;
        }
        let level = r[battery_byte];
        let charging = r[KEYBOARD_CHARGING_BYTE] != 0;
        if level > 100 || (level == 0 && !charging) {
            return None;
        }
        Some(Reading { level, charging, approximate: false })
    }

    /// Solaar 的锂电放电曲线：mV -> 百分比（asus.py 从 logitech.py 复用）。
    fn voltage_to_percent(mv: u16) -> u16 {
        const CURVE: &[(u16, u16)] = &[
            (4186, 100), (4067, 90), (3989, 80), (3922, 70), (3859, 60), (3811, 50),
            (3778, 40), (3751, 30), (3717, 20), (3671, 10), (3646, 5), (3579, 2), (3500, 0),
        ];
        if mv >= CURVE[0].0 {
            return 100;
        }
        for pair in CURVE.windows(2) {
            let (hi_mv, hi_p) = pair[0];
            let (lo_mv, lo_p) = pair[1];
            if mv >= lo_mv {
                let v = f32::from(lo_p)
                    + (f32::from(mv - lo_mv) * f32::from(hi_p - lo_p))
                        / f32::from(hi_mv - lo_mv);
                return v.round() as u16;
            }
        }
        0
    }

    /// `_headset_frame`：按 report id 位置读取电压（11/12）与充电字节（9）。
    fn headset_frame(r: &[u8], header: u8) -> Option<(u16, Option<u8>)> {
        if r.len() < 4 {
            return None;
        }
        let skip = if r[0] == HEADSET_REPORT_ID { 0 } else { 1 };
        let at = |n: usize| -> Option<u8> {
            let i = n.checked_sub(skip)?;
            r.get(i).copied()
        };
        if at(1) == Some(HEADSET_ERROR.0) && at(2) == Some(HEADSET_ERROR.1) {
            return None;
        }
        if at(1) == Some(0) && at(2) == Some(0) && at(3) == Some(0) {
            return None;
        }
        if at(1) != Some(header) {
            return None;
        }
        let lsb = at(HEADSET_VOLTAGE_LSB)?;
        let msb = at(HEADSET_VOLTAGE_MSB)?;
        Some((u16::from_le_bytes([lsb, msb]), at(HEADSET_CHARGING_BYTE)))
    }

    fn parse_headset_reply(r: &[u8]) -> Option<(u8, bool)> {
        let (mut mv, state) = headset_frame(r, HEADSET_HEADER)?;
        let mut msb = (mv >> 8) as u8;
        let charging = state == Some(HEADSET_CHARGING_VALUE);
        if charging && mv < HEADSET_CHARGING_FULL_MV {
            mv = mv.saturating_sub(HEADSET_CHARGING_LIFT_MV);
            msb = (mv >> 8) as u8;
        }
        for (nibble, ceil_mv, level) in [
            (Some(0x10u8), 4100u16, 100u8),
            (Some(0x0F), 3800, 75),
            (None, 3700, 50),
            (Some(0x0E), 3500, 25),
        ] {
            if (nibble.is_some_and(|n| msb >= n)) || mv >= ceil_mv {
                return Some((level, charging));
            }
        }
        Some((10, charging))
    }

    fn parse_strix_go_reply(r: &[u8]) -> Option<(u8, Option<bool>)> {
        parse_headset_reply(r).map(|(level, charging)| (level, Some(charging)))
    }

    fn parse_tuf_h3_reply(r: &[u8]) -> Option<(u8, Option<bool>)> {
        let (mv, state) = headset_frame(r, TUF_H3_HEADER)?;
        if !(TUF_H3_MV_RANGE.0..=TUF_H3_MV_RANGE.1).contains(&mv) {
            return None;
        }
        let pct = voltage_to_percent(mv);
        let level = ((pct + u16::from(TUF_H3_STEP) - 1) / u16::from(TUF_H3_STEP)) * u16::from(TUF_H3_STEP);
        let charging = match state {
            Some(0x03) => Some(false),
            Some(0x05) => Some(true),
            _ => None,
        };
        Some((level as u8, charging))
    }

    fn read_headset(
        c: &Collection,
        request: &[u8],
        parse: fn(&[u8]) -> Option<(u8, Option<bool>)>,
    ) -> Option<(u8, Option<bool>)> {
        let mut h = c.open()?;
        let mut frame = vec![0u8; HEADSET_PACKET_LENGTH];
        frame[..request.len()].copy_from_slice(request);
        for _ in 0..HEADSET_ATTEMPTS {
            if !h.set_feature(&frame, deadline(READ_TIMEOUT_MS)) {
                return None;
            }
            sleep(Duration::from_millis(HEADSET_SLEEP_MS));
            let Some(r) = h.get_feature(HEADSET_REPORT_ID, deadline(READ_TIMEOUT_MS)) else {
                continue;
            };
            if r.is_empty() {
                continue;
            }
            return parse(&r);
        }
        None
    }

    /// TUF H3 的 0x03 帧需隔 3 秒再读：两次 0x03 才算在电池，0x05 立刻算充电。
    fn settle_charging(
        state: &mut State,
        key: &str,
        c: &Collection,
        res: (u8, Option<bool>),
    ) -> (u8, Option<bool>) {
        if res.1 == Some(false) && state.settled.get(key) == Some(&false) {
            return res;
        }
        let mut readings = vec![res];
        for _ in 0..TUF_H3_RECHECKS {
            let states: Vec<Option<bool>> = readings.iter().map(|r| r.1).collect();
            if states.contains(&Some(true)) || states.iter().filter(|&&s| s == Some(false)).count() >= 2 {
                break;
            }
            sleep(Duration::from_millis(TUF_H3_RECHECK_MS));
            if let Some(more) = read_headset(c, TUF_H3_REQUEST, parse_tuf_h3_reply) {
                readings.push(more);
            }
        }
        let mut saw_true = false;
        let mut false_count = 0;
        for r in &readings {
            match r.1 {
                Some(true) => saw_true = true,
                Some(false) => false_count += 1,
                None => {}
            }
        }
        let charging = if saw_true {
            Some(true)
        } else if false_count >= 2 {
            Some(false)
        } else {
            None
        };
        if let Some(c) = charging {
            state.settled.insert(key.to_string(), c);
        }
        let last = readings.last().copied().unwrap_or(res);
        (last.0, charging)
    }

    fn collect_omni(collections: &[Collection]) -> HashMap<u16, &Collection> {
        let mut cols = HashMap::new();
        for c in collections {
            if c.interface == Some(OMNI_INTERFACE) {
                cols.insert(c.usage_page, c);
            }
        }
        cols
    }

    /// OMNI 接收器：先读配对列表，再按配对 pid 选择鼠标/键盘集合。
    fn poll_omni(
        cols: &HashMap<u16, &Collection>,
        pid: u16,
    ) -> Vec<(String, &'static str, Reading)> {
        let Some(control) = cols.get(&OMNI_CONTROL) else {
            return Vec::new();
        };
        let Some(r) = ask(control, OMNI_PAIRS, |x| !pair_pids(x).is_empty(), OMNI_PACKET_LENGTH) else {
            return Vec::new();
        };
        let pids = pair_pids(&r);
        let mut out = Vec::new();
        if let Some((_, name)) = OMNI_MICE.iter().find(|(p, _)| pids.contains(p)) {
            if let Some(mouse) = cols.get(&OMNI_MOUSE) {
                if let Some(r) = ask(mouse, OMNI_MOUSE_REQUEST, is_omni_mouse, OMNI_PACKET_LENGTH) {
                    if let Some(reading) = parse_omni_mouse(&r) {
                        out.push(((*name).into(), "mouse", reading));
                    }
                }
            }
        }
        if let Some((_, name, battery_byte)) = OMNI_KEYBOARDS.iter().find(|(p, _, _)| pids.contains(p)) {
            if let Some(kb) = cols.get(&OMNI_KEYBOARD) {
                if let Some(r) = ask(kb, OMNI_KEYBOARD_REQUEST, is_omni_keyboard, OMNI_PACKET_LENGTH) {
                    if let Some(reading) = parse_omni_keyboard(&r, *battery_byte) {
                        out.push(((*name).into(), "keyboard", reading));
                    }
                }
            }
        }
        let _ = pid;
        out
    }

    impl Provider for Asus {
        fn name(&self) -> &'static str {
            "asus"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID {
                return Vec::new();
            }
            let mut state = self.state.lock().unwrap();
            let mut out = Vec::new();

            // 1) G-Helper 普通鼠标 / 线缆（接口 0 的 vendor 集合）
            if let Some((_, name, scale)) = KNOWN.iter().find(|(p, _, _)| *p == pid) {
                for c in collections {
                    if c.interface != Some(INTERFACE) || c.usage_page < 0xFF00 {
                        continue;
                    }
                    if let Some(r) = ask(c, REQUEST, has_echo, PACKET_LENGTH) {
                        if let Some(reading) = parse_reply(&r, *scale) {
                            let mut st = make_status(
                                format!("asus:{}", name.to_lowercase().replace(' ', "-")),
                                *name,
                                "mouse",
                                "asus",
                                Some(reading.level),
                                Some(reading.charging),
                                true,
                            );
                            st.approximate = reading.approximate;
                            out.push(st);
                            break;
                        }
                    }
                }
                return out;
            }

            // 2) Harpe II 线缆：64 字节包，和普通鼠标同样的 12 07 命令
            if pid == HARPE_II_CABLE {
                for c in collections {
                    if c.interface != Some(INTERFACE) || c.usage_page < 0xFF00 {
                        continue;
                    }
                    if let Some(r) = ask(c, REQUEST, has_echo, HEADSET_PACKET_LENGTH) {
                        if let Some(reading) = parse_reply(&r, PERCENT) {
                            out.push(make_status(
                                "asus:rog-harpe-ii-ace",
                                "ROG Harpe II Ace",
                                "mouse",
                                "asus",
                                Some(reading.level),
                                Some(reading.charging),
                                true,
                            ));
                            break;
                        }
                    }
                }
                return out;
            }

            // 3) OMNI / SpeedNova 接收器
            if pid == OMNI_PID || pid == HARPE_II_RECEIVER {
                let cols = collect_omni(collections);
                for (name, kind, reading) in poll_omni(&cols, pid) {
                    out.push(make_status(
                        format!("asus:{}", name.to_lowercase().replace(' ', "-")),
                        name,
                        kind,
                        "asus",
                        Some(reading.level),
                        Some(reading.charging),
                        true,
                    ));
                }
                // SpeedNova：配对列表里 1C12 才算 Azoth 96 HE，键盘字节 6。
                if pid == HARPE_II_RECEIVER {
                    if let Some(control) = cols.get(&OMNI_CONTROL) {
                        if let Some(r) = ask(control, OMNI_PAIRS, |x| !pair_pids(x).is_empty(), OMNI_PACKET_LENGTH) {
                            if r.len() == OMNI_PACKET_LENGTH
                                && pair_pids(&r).contains(&SPEEDNOVA_KEYBOARD_PID)
                            {
                                if let Some(kb) = cols.get(&OMNI_KEYBOARD) {
                                    if let Some(r) = ask(kb, OMNI_KEYBOARD_REQUEST, is_omni_keyboard, OMNI_PACKET_LENGTH) {
                                        if let Some(reading) = parse_omni_keyboard(&r, 6) {
                                            out.push(make_status(
                                                "asus:rog-azoth-96-he",
                                                SPEEDNOVA_KEYBOARD_NAME,
                                                "keyboard",
                                                "asus",
                                                Some(reading.level),
                                                Some(reading.charging),
                                                true,
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                return out;
            }

            // 4) 耳机 feature 报告（Strix Go 2.4 / TUF H3）
            let headset = if pid == HEADSET_PID || pid == HEADSET_WIRED_PID {
                Some((HEADSET_NAME, HEADSET_REQUEST, parse_strix_go_reply as fn(&[u8]) -> Option<(u8, Option<bool>)>, false))
            } else if pid == TUF_H3_PID {
                Some((TUF_H3_NAME, TUF_H3_REQUEST, parse_tuf_h3_reply as fn(&[u8]) -> Option<(u8, Option<bool>)>, true))
            } else {
                None
            };
            if let Some((name, request, parse, recheck)) = headset {
                let key = format!("asus:{}", name.to_lowercase().replace(' ', "-"));
                for c in collections {
                    if c.feature_len < HEADSET_PACKET_LENGTH {
                        continue;
                    }
                    let Some(mut res) = read_headset(c, request, parse) else {
                        continue;
                    };
                    if recheck {
                        res = settle_charging(&mut state, &key, c, res);
                    }
                    let (level, charging) = res;
                    let charging = if pid == HEADSET_WIRED_PID { Some(true) } else { charging };
                    out.push(make_status(
                        key,
                        name,
                        "headset",
                        "asus",
                        Some(level),
                        charging,
                        charging.is_some(),
                    ));
                    break;
                }
            }
            out
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn asus_mouse_reply() {
            let mut r = vec![0u8; PACKET_LENGTH];
            r[0] = 0x12;
            r[1] = 0x07;
            r[LEVEL_BYTE] = 76;
            r[CHARGING_BYTE] = 1;
            let got = parse_reply(&r, PERCENT).unwrap();
            assert_eq!((got.level, got.charging, got.approximate), (76, true, false));
            // 报告 id 0 在前的另一种形式
            let mut r2 = vec![0u8; PACKET_LENGTH];
            r2[1] = 0x12;
            r2[2] = 0x07;
            r2[1 + LEVEL_BYTE] = 3;
            let got = parse_reply(&r2, STEPS).unwrap();
            assert_eq!((got.level, got.approximate), (75, true));
        }

        #[test]
        fn asus_standby_and_error() {
            let r = [0x12u8, 0x07, 0, 0, 0, 0, 0, 0, 0, 0];
            assert!(parse_reply(&r, PERCENT).is_none());
            assert!(is_error(&[0x00, 0xFF, 0xAA]));
        }

        #[test]
        fn omni_pair_list() {
            let mut r = vec![0u8; OMNI_PACKET_LENGTH];
            r[0] = 0x01;
            r[1] = 0xA0;
            r[OMNI_FIRST_SLOT] = 0x65;
            r[OMNI_FIRST_SLOT + 1] = 0x1B;
            r[OMNI_FIRST_SLOT + OMNI_SLOT] = 0x12;
            r[OMNI_FIRST_SLOT + OMNI_SLOT + 1] = 0x1C;
            assert_eq!(pair_pids(&r), vec![0x1B65, 0x1C12]);
        }

        #[test]
        fn strix_go_voltage_parse() {
            let mut r = vec![0u8; HEADSET_PACKET_LENGTH];
            r[0] = HEADSET_REPORT_ID;
            r[1] = HEADSET_HEADER;
            r[11] = 0x9F;
            r[12] = 0x0F; // 3999 mV -> 75%
            r[9] = 0x0A;
            let (level, charging) = parse_headset_reply(&r).unwrap();
            assert_eq!((level, charging), (75, true));
        }

        #[test]
        fn tuf_h3_parse() {
            let mut r = vec![0u8; HEADSET_PACKET_LENGTH];
            r[0] = HEADSET_REPORT_ID;
            r[1] = TUF_H3_HEADER;
            r[11] = 0xA2;
            r[12] = 0x0F; // 4002 mV
            r[9] = 0x05;
            let (level, charging) = parse_tuf_h3_reply(&r).unwrap();
            assert_eq!((level, charging), (100, Some(true)));
        }
    }
}

// ===========================================================================
// Logitech：VID 0x046D（HID++ 2.0 / 1.0）
// 参考 logitech.py（721 行）+ logitech_g325.py。核心是 HID++ 请求/应答状态机：
// 短集合 ff00:0001（7 字节）、长集合 ff00:0002（20 字节），slot 1..6 或 0xFF。
// ===========================================================================
mod logitech {
    use super::*;

    const VID: u16 = 0x046D;
    const SWIDS: &[u8] = &[0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F];
    const TIMEOUT_MS: u64 = 600;
    const PING_TIMEOUT_MS: u64 = 2_000;
    const ASLEEP_KEEP_S: u64 = 300;
    const SHORT_PAGE: u16 = 0xFF00;
    const LONG_USAGE: u16 = 0x0002;
    const SHORT_USAGE: u16 = 0x0001;
    const HEADSET_PAGE: u16 = 0xFF43;
    const HEADSET_USAGE: u16 = 0x0202;

    const F_ROOT: u16 = 0x0000;
    const F_INFO: u16 = 0x0003;
    const F_NAME: u16 = 0x0005;
    const F_UNIFIED: u16 = 0x1004;
    const F_STATUS: u16 = 0x1000;
    const F_VOLTAGE: u16 = 0x1001;
    const F_ADC: u16 = 0x1F20;
    const BATTERY_FEATURES: &[u16] = &[F_UNIFIED, F_STATUS, F_VOLTAGE, F_ADC];
    const REG_CHARGE: u8 = 0x0D;
    const REG_STATUS: u8 = 0x07;
    const STATUS_LEVELS: &[(u8, u8)] = &[(7, 90), (5, 50), (3, 20), (1, 5)];
    const ERR_OLD_PROTOCOL: u8 = 0x01;
    #[allow(dead_code)] // Python _ping 只对这两种错误写诊断，不改变行为
    const ERR_CONNECT_FAIL: u8 = 0x04;
    const ERR_EMPTY_SLOT: u8 = 0x08;
    #[allow(dead_code)]
    const ERR_UNREACHABLE: u8 = 0x09;

    const RECEIVERS: &[u16] = &[
        0xC548, 0xC52B, 0xC532, 0xC52F, 0xC518, 0xC51A, 0xC51B, 0xC521, 0xC525, 0xC526,
        0xC52E, 0xC531, 0xC534, 0xC535, 0xC537, 0xC539, 0xC53A, 0xC53D, 0xC53F, 0xC541,
        0xC545, 0xC547, 0xC54D, 0xC54F,
    ];
    const HIDPP10_MODELS: &[(u16, &str, &str)] = &[(0xC525, "Logitech MX Revolution", "mouse")];
    const HEADSETS: &[(u16, &str)] = &[
        (0x0A66, "Logitech G533"),
        (0x0AC4, "Logitech G535"),
        (0x0A5C, "Logitech G633"),
        (0x0A89, "Logitech G635"),
        (0x0A5B, "Logitech G933"),
        (0x0A87, "Logitech G935"),
        (0x0AB5, "Logitech G733"),
        (0x0AFE, "Logitech G733"),
        (0x0B1F, "Logitech G733"),
        (0x0AA7, "Logitech G PRO"),
        (0x0AAA, "Logitech G PRO X"),
        (0x0ABA, "Logitech G PRO X"),
        (0x0AFB, "Logitech G PRO X 2"),
        (0x0AFC, "Logitech G PRO X 2"),
    ];
    const HEADSET_CHARGE_REG: u8 = 0x0D;
    const HEADSET_REG_SUBID: u8 = 0x05;
    const HEADSET_CHARGING: u8 = 0x03;
    const HEADSET_CURVE: &[(u16, u8)] = &[
        (4175, 100), (3817, 50), (3766, 30), (3730, 20), (3664, 5), (3310, 0),
    ];
    const KINDS: &[(u8, &str)] = &[
        (0, "keyboard"), (2, "keyboard"), (3, "mouse"), (4, "mouse"), (5, "mouse"),
    ];
    const VOLTAGE_CURVE: &[(u16, u8)] = &[
        (4186, 100), (4067, 90), (3989, 80), (3922, 70), (3859, 60), (3811, 50),
        (3778, 40), (3751, 30), (3717, 20), (3671, 10), (3646, 5), (3579, 2), (3500, 0),
    ];
    const MAX_BATTERY_MV: u16 = 5000;
    const ON_CHARGER: &[u8] = &[1, 2, 3, 4];
    const APPROX_LEVELS: &[(u8, u8)] = &[(8, 90), (4, 50), (2, 20), (1, 5)];

    // G325：独立简单协议（logitech_g325.py）
    const G325_PID: u16 = 0x0B21;
    const G325_REPORT_ID: u8 = 0x50;
    const G325_PAGE: u16 = 0xFFA0;
    const G325_USAGE: u16 = 0x0001;
    const G325_REQUEST: &[u8] = &[
        0x50, 0x23, 0x0B, 0x00, 0x03, 0x1A, 0x00, 0x03, 0x00, 0x07, 0x0A,
    ];
    const G325_PACKET_SIZE: usize = 64;
    const G325_READ_ATTEMPTS: usize = 4;
    const G325_READ_TIMEOUT_MS: u64 = 1500;

    pub struct Logitech {
        state: Mutex<State>,
    }

    #[derive(Default)]
    struct State {
        ids: HashMap<(u16, String, u8), (String, String, String)>,
        asleep: HashSet<(u16, String, u8)>,
        last: HashMap<String, (Status, Instant)>,
        slot_key: HashMap<(u16, String, u8), String>,
        discharge: HashMap<String, u8>,
        pid_instances: HashMap<u16, HashSet<String>>,
    }

    impl Logitech {
        pub fn new() -> Self {
            Self { state: Mutex::new(State::default()) }
        }
    }

    impl Default for Logitech {
        fn default() -> Self {
            Self::new()
        }
    }

    fn voltage_to_percent(mv: u16) -> u8 {
        if mv >= VOLTAGE_CURVE[0].0 {
            return 100;
        }
        for pair in VOLTAGE_CURVE.windows(2) {
            let (hi_mv, hi_p) = pair[0];
            let (lo_mv, lo_p) = pair[1];
            if mv >= lo_mv {
                let v = f32::from(lo_p)
                    + (f32::from(mv - lo_mv) * f32::from(hi_p - lo_p))
                        / f32::from(hi_mv - lo_mv);
                return v.round() as u8;
            }
        }
        0
    }

    fn headset_voltage_to_percent(mv: u16) -> u8 {
        if mv >= HEADSET_CURVE[0].0 {
            return 100;
        }
        for pair in HEADSET_CURVE.windows(2) {
            let (hi_mv, hi_p) = pair[0];
            let (lo_mv, lo_p) = pair[1];
            if mv >= lo_mv {
                let v = f32::from(lo_p)
                    + (f32::from(mv - lo_mv) * f32::from(hi_p - lo_p))
                        / f32::from(hi_mv - lo_mv);
                return v.round() as u8;
            }
        }
        0
    }

    fn parse_headset_voltage(r: &[u8]) -> Option<(u8, bool)> {
        if r.len() < 7 {
            return None;
        }
        let mv = u16::from_be_bytes([r[4], r[5]]);
        if !(2500..=MAX_BATTERY_MV).contains(&mv) {
            return None;
        }
        Some((headset_voltage_to_percent(mv), r[6] == HEADSET_CHARGING))
    }

    fn charge_complete(feature: u16, p: &[u8]) -> bool {
        feature == F_VOLTAGE && p.len() >= 3 && (p[2] & 0x80) != 0 && (p[2] & 0x03) == 1
    }

    struct Battery {
        level: Option<u8>,
        charging: bool,
        grade: Option<u8>,
    }

    fn parse_battery(feature: u16, p: &[u8]) -> Battery {
        let none = Battery { level: None, charging: false, grade: None };
        if p.len() < 3 {
            return none;
        }
        if feature == F_UNIFIED {
            let charging = ON_CHARGER.contains(&p[2]);
            if (1..=100).contains(&p[0]) {
                return Battery { level: Some(p[0]), charging, grade: None };
            }
            if p[0] == 0 {
                if let Some((_, level)) = APPROX_LEVELS.iter().find(|(bits, _)| *bits == p[1]) {
                    return Battery { level: Some(*level), charging, grade: Some(p[1]) };
                }
            }
            return none;
        }
        if feature == F_STATUS {
            return Battery {
                level: ((1..=100).contains(&p[0])).then_some(p[0]),
                charging: ON_CHARGER.contains(&p[2]),
                grade: None,
            };
        }
        if feature == F_VOLTAGE || feature == F_ADC {
            let mv = u16::from_be_bytes([p[0], p[1]]);
            if feature == F_ADC && (p[2] & 0x01) == 0 {
                return none; // 耳机未连接
            }
            if !(2500..=MAX_BATTERY_MV).contains(&mv) {
                return none;
            }
            let charging = if feature == F_VOLTAGE {
                (p[2] & 0x80) != 0
            } else {
                (p[2] & 0x02) != 0
            };
            return Battery { level: Some(voltage_to_percent(mv)), charging, grade: None };
        }
        none
    }

    fn parse_register_battery(reg: u8, p: &[u8]) -> Battery {
        let none = Battery { level: None, charging: false, grade: None };
        if reg == REG_CHARGE {
            if p.len() < 3 || !(1..=100).contains(&p[0]) {
                return none;
            }
            return Battery {
                level: Some(p[0]),
                charging: matches!(p[2] & 0xF0, 0x50 | 0x90),
                grade: None,
            };
        }
        if reg == REG_STATUS {
            if p.len() < 2 {
                return none;
            }
            let charging = (p[1] & 0x21) == 0x21 || (p[1] & 0x22) == 0x22;
            if let Some((_, level)) = STATUS_LEVELS.iter().find(|(s, _)| *s == p[0]) {
                return Battery { level: Some(*level), charging, grade: Some(p[0]) };
            }
            return Battery { level: None, charging, grade: None };
        }
        none
    }

    /// 一个物理接收器/设备上的 HID++ 集合。持有长/短/备用句柄。
    struct Channel {
        long: Handle,
        short: Option<Handle>,
        alt: Option<Handle>,
        swid: usize,
        error: bool,
        error_code: u8,
    }

    impl Channel {
        fn open(short: Option<&Collection>, long: &Collection, alt: Option<&Collection>) -> Option<Self> {
            let long = long.open()?;
            let short = short.and_then(|c| c.open());
            let alt = alt.and_then(|c| c.open());
            Some(Self { long, short, alt, swid: 0, error: false, error_code: 0 })
        }

        fn handles(&mut self) -> Vec<&mut Handle> {
            let mut v = vec![&mut self.long];
            if let Some(s) = &mut self.short {
                v.push(s);
            }
            v
        }

        /// `feat` 是 feature **index**（0..255），不是 16 位 feature id；
        /// 16 位 id 先经 `feature_index()` 换成 index 才传进来。
        fn request(
            &mut self,
            idx: u8,
            feat: u8,
            func: u8,
            params: &[u8],
            timeout_ms: u64,
        ) -> Option<Vec<u8>> {
            self.error = false;
            self.error_code = 0;
            self.swid = (self.swid + 1) % SWIDS.len();
            let fn_byte = (func << 4) | SWIDS[self.swid];
            let mut req = vec![0x11, idx, feat, fn_byte];
            req.extend_from_slice(params);
            req.resize(20, 0);
            if !self.long.write(&req, deadline(timeout_ms)) {
                return None;
            }
            let end = Instant::now() + Duration::from_millis(timeout_ms);
            while Instant::now() < end {
                for h in self.handles() {
                    let left = end.saturating_duration_since(Instant::now()).as_millis().min(20) as u64;
                    let Some(r) = h.read(deadline(left)) else {
                        continue;
                    };
                    if r.len() < 4 || r[1] != idx {
                        continue;
                    }
                    // 10 <slot> 8f <feat> <fn> <err> / 11 <slot> ff <feat> <fn> <err>
                    if (r[2] == 0x8F || r[2] == 0xFF)
                        && r.len() >= 6
                        && r[3] == feat
                        && r[4] == fn_byte
                    {
                        self.error = true;
                        self.error_code = r[5];
                        return None;
                    }
                    // 11 <slot> <feat> <fn> <p1> <p2> <p3>
                    if r[2] == feat && r[3] == fn_byte && r.len() >= 7 {
                        return Some(r[4..].to_vec());
                    }
                }
                sleep(Duration::from_millis(5));
            }
            None
        }

        fn read_register(&mut self, idx: u8, reg: u8, timeout_ms: u64) -> Option<Vec<u8>> {
            let short = self.short.as_mut()?;
            self.error = false;
            self.error_code = 0;
            if !short.write(&[0x10, idx, 0x81, reg, 0, 0, 0], deadline(timeout_ms)) {
                return None;
            }
            let end = Instant::now() + Duration::from_millis(timeout_ms);
            while Instant::now() < end {
                for h in self.handles() {
                    let left = end.saturating_duration_since(Instant::now()).as_millis().min(20) as u64;
                    let Some(r) = h.read(deadline(left)) else {
                        continue;
                    };
                    if r.len() < 4 || r[1] != idx {
                        continue;
                    }
                    if (r[2] == 0x8F || r[2] == 0xFF)
                        && r.len() >= 6
                        && r[3] == 0x81
                        && r[4] == reg
                    {
                        self.error = true;
                        self.error_code = r[5];
                        return None;
                    }
                    if r[2] == 0x81 && r[3] == reg && r.len() >= 7 {
                        return Some(r[4..].to_vec());
                    }
                }
                sleep(Duration::from_millis(5));
            }
            None
        }

        fn read_headset_register(&mut self, idx: u8, reg: u8, timeout_ms: u64) -> Option<Vec<u8>> {
            self.error = false;
            self.error_code = 0;
            let payload = [0x11, idx, HEADSET_REG_SUBID, reg];
            let mut frame = [0u8; 20];
            frame[..4].copy_from_slice(&payload);
            if !self.long.write(&frame, deadline(timeout_ms)) {
                return None;
            }
            if let Some(alt) = &mut self.alt {
                let _ = alt.write(&frame, deadline(timeout_ms));
            }
            let end = Instant::now() + Duration::from_millis(timeout_ms);
            while Instant::now() < end {
                let handles: Vec<&mut Handle> = match &mut self.alt {
                    Some(alt) => vec![&mut self.long, alt],
                    None => vec![&mut self.long],
                };
                for h in handles {
                    let left = end.saturating_duration_since(Instant::now()).as_millis().min(20) as u64;
                    let Some(r) = h.read(deadline(left)) else {
                        continue;
                    };
                    if r.len() < 4 || r[1] != idx {
                        continue;
                    }
                    if r[2] == 0x8F || r[2] == 0xFF {
                        self.error = true;
                        self.error_code = r.get(5).copied().unwrap_or(0);
                        continue;
                    }
                    if r[2] == HEADSET_REG_SUBID && r[3] == reg && r.len() >= 7 {
                        return Some(r);
                    }
                }
                sleep(Duration::from_millis(5));
            }
            None
        }

        fn feature_index(&mut self, idx: u8, feature: u16) -> u8 {
            let r = self.request(idx, F_ROOT as u8, 0, &[(feature >> 8) as u8, feature as u8], TIMEOUT_MS);
            match r {
                Some(p) if p.len() >= 3 => p[0],
                _ => 0,
            }
        }

        fn identity(&mut self, idx: u8) -> (String, String, String) {
            let mut name = String::new();
            let mut kind = String::new();
            let mut unit = String::new();
            let fi = self.feature_index(idx, F_NAME);
            if fi != 0 {
                if let Some(r) = self.request(idx, fi, 0, &[], TIMEOUT_MS) {
                    if r.len() >= 3 {
                        let length = r[0] as usize;
                        let mut raw = Vec::new();
                        while raw.len() < length {
                            let Some(r) = self.request(idx, fi, 1, &[raw.len() as u8], TIMEOUT_MS) else {
                                break;
                            };
                            if r.len() < 3 {
                                break;
                            }
                            let need = 16.min(length - raw.len());
                            raw.extend_from_slice(&r[..need.min(r.len())]);
                        }
                        if raw.len() == length {
                            name = String::from_utf8_lossy(&raw).trim().to_string();
                        }
                    }
                }
                if let Some(r) = self.request(idx, fi, 2, &[], TIMEOUT_MS) {
                    if r.len() >= 3 {
                        kind = KINDS.iter().find(|(v, _)| *v == r[0]).map(|(_, k)| *k).unwrap_or("").into();
                    }
                }
            }
            let fi = self.feature_index(idx, F_INFO);
            if fi != 0 {
                if let Some(r) = self.request(idx, fi, 0, &[], TIMEOUT_MS) {
                    if r.len() >= 5 && r[1..5].iter().any(|&b| b != 0) {
                        unit = r[1..5].iter().map(|b| format!("{b:02X}")).collect();
                    }
                }
            }
            (name, kind, unit)
        }
    }

    fn headset_name(pid: u16) -> Option<&'static str> {
        HEADSETS.iter().find(|(p, _)| *p == pid).map(|(_, n)| *n)
    }

    fn ping(state: &mut State, ch: &mut Channel, pid: u16, inst: &str, idx: u8) -> bool {
        let slot = (pid, inst.to_string(), idx);
        let timeout = if state.asleep.contains(&slot) { TIMEOUT_MS } else { PING_TIMEOUT_MS };
        let reply = ch.request(idx, F_ROOT as u8, 1, &[], timeout);
        if let Some(r) = reply {
            if r.len() >= 3 {
                state.asleep.remove(&slot);
                return true;
            }
        }
        if !ch.error {
            state.asleep.insert(slot);
            return false;
        }
        state.asleep.remove(&slot);
        if ch.error_code == ERR_EMPTY_SLOT {
            state.ids.remove(&slot);
        }
        false
    }

    fn register_battery(
        state: &mut State,
        ch: &mut Channel,
        pid: u16,
        inst: &str,
        idx: u8,
        multi: bool,
    ) -> Option<Status> {
        for reg in [REG_CHARGE, REG_STATUS] {
            let Some(r) = ch.read_register(idx, reg, TIMEOUT_MS) else {
                continue;
            };
            let b = parse_register_battery(reg, &r);
            let Some(level) = b.level else {
                continue;
            };
            let slot = (pid, inst.to_string(), idx);
            let name = state.ids.get(&slot).map(|(n, _, _)| n.clone()).unwrap_or_else(|| "Logitech device".into());
            let (name, kind) = HIDPP10_MODELS
                .iter()
                .find(|(p, _, _)| *p == pid)
                .map(|(_, n, k)| ((*n).to_string(), (*k).to_string()))
                .unwrap_or((name, String::new()));
            let prefix = if multi { format!("{inst}:") } else { String::new() };
            let key = format!("logitech:{prefix}{pid:04x}:{idx}");
            replace_slot_key(state, &slot, &key);
            let mut st = make_status(
                key, name, &kind, "logitech", Some(level), Some(b.charging), true,
            );
            st.approximate = b.grade.is_some();
            return Some(st);
        }
        None
    }

    fn headset_register(
        state: &mut State,
        ch: &mut Channel,
        pid: u16,
        inst: &str,
        idx: u8,
        multi: bool,
    ) -> Option<Status> {
        let r = ch.read_headset_register(idx, HEADSET_CHARGE_REG, TIMEOUT_MS)?;
        let (level, charging) = parse_headset_voltage(&r)?;
        let name = headset_name(pid).unwrap_or("Logitech headset");
        let prefix = if multi { format!("{inst}:") } else { String::new() };
        let key = format!("logitech:{prefix}{pid:04x}:{idx}");
        let slot = (pid, inst.to_string(), idx);
        replace_slot_key(state, &slot, &key);
        Some(make_status(key, name, "headset", "logitech", Some(level), Some(charging), true))
    }

    fn replace_slot_key(state: &mut State, slot: &(u16, String, u8), key: &str) {
        if let Some(old) = state.slot_key.get(slot) {
            if old != key {
                state.last.remove(old);
            }
        }
        state.slot_key.insert(slot.clone(), key.to_string());
    }

    fn read_device(
        state: &mut State,
        ch: &mut Channel,
        pid: u16,
        inst: &str,
        idx: u8,
        multi: bool,
    ) -> Option<Status> {
        let slot = (pid, inst.to_string(), idx);
        // G535：只有长集合的 HID++ 1.0 耳机，先读寄存器再 ping。
        if pid == 0x0AC4 && ch.short.is_none() {
            if let Some(st) = headset_register(state, ch, pid, inst, idx, multi) {
                return Some(st);
            }
        }
        if !ping(state, ch, pid, inst, idx) {
            if ch.error && ch.error_code == ERR_OLD_PROTOCOL {
                return register_battery(state, ch, pid, inst, idx, multi);
            }
            return None;
        }
        let (name, kind, unit) = match state.ids.get(&slot) {
            Some(v) => v.clone(),
            None => {
                let id = ch.identity(idx);
                if !id.0.is_empty() || !id.2.is_empty() {
                    state.ids.insert(slot.clone(), id.clone());
                }
                id
            }
        };
        let (mut name, mut kind) = (name, kind);
        if let Some(n) = headset_name(pid) {
            name = if name.is_empty() { n.to_string() } else { name };
            kind = "headset".into();
        }
        if let Some((_, n, k)) = HIDPP10_MODELS.iter().find(|(p, _, _)| *p == pid) {
            name = (*n).into();
            kind = (*k).into();
        }
        if name.is_empty() {
            name = "Logitech device".into();
        }
        for feature in BATTERY_FEATURES {
            let fi = ch.feature_index(idx, *feature);
            if fi == 0 {
                continue;
            }
            let func = if *feature == F_UNIFIED { 1 } else { 0 };
            let Some(p) = ch.request(idx, fi, func, &[], TIMEOUT_MS) else {
                continue;
            };
            let b = parse_battery(*feature, &p);
            let Some(level) = b.level else {
                continue;
            };
            let prefix = if multi { format!("{inst}:") } else { String::new() };
            let key = if unit.is_empty() {
                format!("logitech:{prefix}{pid:04x}:{idx}")
            } else {
                format!("logitech:{unit}")
            };
            replace_slot_key(state, &slot, &key);
            // 电压型电量：充电时的电压读数是满电附近，改用断电前最后一次读数。
            if *feature == F_VOLTAGE || *feature == F_ADC {
                if !b.charging {
                    state.discharge.insert(key.clone(), level);
                } else if !charge_complete(*feature, &p) {
                    if let Some(before) = state.discharge.get(&key).copied() {
                        let mut st = make_status(
                            key, name, &kind, "logitech", Some(before), Some(true), true,
                        );
                        st.approximate = true;
                        return Some(st);
                    }
                    return Some(make_status(
                        key, name, &kind, "logitech", None, Some(true), true,
                    ));
                }
            }
            let mut st = make_status(key, name, &kind, "logitech", Some(level), Some(b.charging), true);
            st.approximate = b.grade.is_some();
            return Some(st);
        }
        register_battery(state, ch, pid, inst, idx, multi)
    }

    impl Provider for Logitech {
        fn name(&self) -> &'static str {
            "logitech"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID {
                return Vec::new();
            }

            // G325 走自己的简单请求/应答协议。
            if pid == G325_PID {
                return poll_g325(collections);
            }

            let mut state = self.state.lock().unwrap();
            let instances = state.pid_instances.entry(pid).or_default();
            instances.insert(instance.to_string());
            let multi = instances.len() > 1;

            let short = collections.iter().find(|c| {
                c.usage_page == SHORT_PAGE && c.usage == SHORT_USAGE
            });
            let long = collections.iter().find(|c| {
                c.usage_page == SHORT_PAGE && c.usage == LONG_USAGE
            }).or_else(|| {
                if headset_name(pid).is_some() {
                    collections.iter().find(|c| {
                        (c.usage_page, c.usage) == (HEADSET_PAGE, HEADSET_USAGE)
                    })
                } else {
                    None
                }
            });
            let Some(long) = long else {
                return Vec::new();
            };
            let alt = if headset_name(pid).is_some() && long.usage_page != HEADSET_PAGE {
                collections.iter().find(|c| {
                    (c.usage_page, c.usage) == (HEADSET_PAGE, HEADSET_USAGE)
                })
            } else {
                None
            };
            let Some(mut ch) = Channel::open(short, long, alt) else {
                return Vec::new();
            };

            let receiver = RECEIVERS.contains(&pid);
            let indices: Vec<u8> = if receiver {
                (1..=6).collect()
            } else {
                vec![0xFF]
            };
            let mut found: Vec<Status> = Vec::new();
            for idx in indices {
                if let Some(st) = read_device(&mut state, &mut ch, pid, instance, idx, multi) {
                    if let Some(existing) = found.iter_mut().find(|x| x.key == st.key) {
                        if st.charging == Some(true) || existing.charging != Some(true) {
                            *existing = st;
                        }
                    } else {
                        found.push(st);
                    }
                }
            }

            let now = Instant::now();
            for st in &found {
                state.last.insert(st.key.clone(), (st.clone(), now));
            }
            // 休眠/关机：保留最后的读数一段时间（灰显由 online=false 表达）。
            let keys: HashSet<String> = found.iter().map(|s| s.key.clone()).collect();
            let stale: Vec<(String, (Status, Instant))> = state
                .last
                .iter()
                .filter(|(key, _)| !keys.contains(*key))
                .map(|(key, v)| (key.clone(), v.clone()))
                .collect();
            for (key, (st, t)) in stale {
                if now.duration_since(t).as_secs() < ASLEEP_KEEP_S {
                    let mut offline = st;
                    offline.online = false;
                    found.push(offline);
                } else {
                    state.last.remove(&key);
                }
            }
            found
        }
    }

    /// `(level, charging, charging_known)`；状态字节 0=放电、2=充电、其他=未知。
    fn parse_g325_reply(r: &[u8]) -> Option<(u8, bool, bool)> {
        if r.len() < 14
            || r[0] != G325_REPORT_ID
            || r[1] != 0x23
            || r[2] != 0x0B
            || r[5] != 0x10
        {
            return None;
        }
        let level = r[12];
        if !(1..=100).contains(&level) {
            return None;
        }
        let (charging, known) = match r[13] {
            0x00 => (false, true),
            0x02 => (true, true),
            _ => (false, false),
        };
        Some((level, charging, known))
    }

    fn poll_g325(collections: &[Collection]) -> Vec<Status> {
        // pick(): ffa0:0001，接口 3 优先。
        let mut candidates: Vec<&Collection> = collections
            .iter()
            .filter(|c| (c.usage_page, c.usage) == (G325_PAGE, G325_USAGE))
            .collect();
        candidates.sort_by_key(|c| if c.interface == Some(3) { 0 } else { 1 });
        for c in candidates {
            let Some(mut h) = c.open() else {
                continue;
            };
            let mut frame = [0u8; G325_PACKET_SIZE];
            frame[..G325_REQUEST.len()].copy_from_slice(G325_REQUEST);
            if !h.write(&frame, deadline(G325_READ_TIMEOUT_MS)) {
                continue;
            }
            for _ in 0..G325_READ_ATTEMPTS {
                let Some(r) = h.read(deadline(G325_READ_TIMEOUT_MS)) else {
                    continue;
                };
                if let Some((level, charging, known)) = parse_g325_reply(&r) {
                    let mut st = make_status(
                        "logitech:g325",
                        "Logitech G325 LIGHTSPEED",
                        "headset",
                        "logitech",
                        Some(level),
                        Some(charging),
                        known,
                    );
                    st.online = true;
                    return vec![st];
                }
            }
        }
        Vec::new()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn hidpp_request_shape() {
            // `feat` 参数是 feature index（不是 16 位 id）：11 <idx> <index> <fn|swid> <params>
            let swid = SWIDS[0];
            let func = 1u8;
            let mut req = vec![0x11, 0x09, 0x02, (func << 4) | swid];
            req.resize(20, 0);
            assert_eq!(req.len(), 20);
            assert_eq!(&req[..4], &[0x11, 0x09, 0x02, 0x1A]);
        }

        #[test]
        fn voltage_curve() {
            assert_eq!(voltage_to_percent(4186), 100);
            assert_eq!(voltage_to_percent(3811), 50);
            assert_eq!(voltage_to_percent(3000), 0);
            assert_eq!(headset_voltage_to_percent(4175), 100);
        }

        #[test]
        fn battery_feature_parsing() {
            let b = parse_battery(F_UNIFIED, &[80, 0, 1]);
            assert_eq!((b.level, b.charging), (Some(80), true));
            let b = parse_battery(F_UNIFIED, &[0, 8, 0]);
            assert_eq!((b.level, b.grade), (Some(90), Some(8)));
            // 4270 mV，flags=0x81：bit7 充电 + 低 2 位 1 = Solaar FULL
            let b = parse_battery(F_VOLTAGE, &[0x10, 0xAE, 0x81]);
            assert!(b.charging && charge_complete(F_VOLTAGE, &[0x10, 0xAE, 0x81]));
            let b = parse_battery(F_ADC, &[0x10, 0xAE, 0x00]); // 耳机未连接
            assert_eq!(b.level, None);
        }

        #[test]
        fn headset_voltage_reply() {
            let r = [0x10, 0xFF, 0x05, 0x0D, 0x10, 0xAE, 0x03];
            let (level, charging) = parse_headset_voltage(&r).unwrap();
            assert_eq!((level, charging), (100, true));
        }

        #[test]
        fn g325_reply() {
            let mut r = vec![0u8; G325_PACKET_SIZE];
            r[0] = G325_REPORT_ID;
            r[1] = 0x23;
            r[2] = 0x0B;
            r[5] = 0x10;
            r[12] = 87;
            r[13] = 0x02;
            assert_eq!(parse_g325_reply(&r), Some((87, true, true)));
            r[13] = 0x00;
            assert_eq!(parse_g325_reply(&r), Some((87, false, true)));
            r[13] = 0x7F;
            assert_eq!(parse_g325_reply(&r), Some((87, false, false)));
        }
    }
}

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(corsair::Corsair::new()),
        Box::new(asus::Asus::new()),
        Box::new(logitech::Logitech::new()),
    ]
}

// 明确跳过、未移植的子协议（原因：Rust Collection API 不暴露 HID path / product
// string，或需要跨 USB 实例枚举 / 复杂写入会话，无法在不猜字节格式的前提下安全移植）：
//
// - Corsair Bragi SABRE v3 的 `00 00 04 0d` 自发帧过滤：仅按 endpoint/cmd 匹配，
//   与 Python 一致；跨实例的“线缆身份出现即视为充电”由 hs80_cable_seen 近似。
// - Corsair `_probe_matrix` / `_listen` / 软件模式切换：仅 --probe 诊断路径， tray 不发送。
// - Asus Harpe II 接收器的 `mi_02&col03` 路径子串判断：Rust Collection 无 path 字段，
//   改用 (pid, interface=2, FF01:0001) 匹配；其余按源码实现。
// - Asus OMNI/SpeedNova 以“同一接收器实例”分组，与 Python 按 path instance 分组等价。
// - Logitech G535 的 000C:0001 消费集合：hid.rs 只放行 usage_page>=0xFF00 的集合，
//   该集合不会传入；仅保留 ff43:0202 备用集合路径。
// - Logitech Centurion（bridge 协议）、G435 RTK（切换 bootloader 模式）：独立的复杂
//   设备协议，不属于 HID++ 核心，未移植。
// - Logitech 的 unit id 跨接收器去重 / ui_message 文案：Rust Status 无对应字段，
//   近似等级只置 approximate=true；电量数值与充电状态按源码解析。
