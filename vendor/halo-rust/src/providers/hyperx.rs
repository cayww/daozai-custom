//! HyperX / Kingston 电量协议族（按 upstream/providers/hyperx*.py 精确移植）。
//!
//! 每个协议用实例中的 VID/PID 与 HID 集合的 usage page/usage 选端点；
//! 回复一律校验报文前缀/命令回显，电量 >100 一律拒绝。
use crate::hid::{Collection, Handle};
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const HYPERX_VID: u16 = 0x03f0;
const KINGSTON_VID: u16 = 0x0951;

// ---------------------------------------------------------------------------
// 通用工具
// ---------------------------------------------------------------------------

fn pick<'a>(collections: &'a [Collection], page: u16, usage: u16) -> Option<&'a Collection> {
    collections.iter().find(|c| c.usage_page == page && c.usage == usage)
}

fn pick_vendor<'a>(collections: &'a [Collection], page: u16, usage: u16) -> Option<&'a Collection> {
    collections
        .iter()
        .find(|c| c.usage_page == page && c.usage == usage)
        .or_else(|| collections.iter().find(|c| c.usage_page == page))
}

fn until(ms: u64) -> Instant {
    Instant::now() + Duration::from_millis(ms)
}

fn headset(key_pid: u16, name: &str, source: &str) -> Status {
    let mut st = Status::new(format!("hyperx:{key_pid:04x}"), name);
    st.kind = "headset".into();
    st.source = source.into();
    st
}

// ---------------------------------------------------------------------------
// 1. hyperx.py: Cloud II Wireless (03F0) 与 Kingston 修订 (0951)
// ---------------------------------------------------------------------------

const C2_WRITE_TIMEOUT: Duration = Duration::from_millis(100);
const C2_READ_TIMEOUT: Duration = Duration::from_secs(1);
const C2_REPLY_LEN: usize = 20;
const C2_CMD_LEVEL: u8 = 0x02;
const C2_CMD_CHARGING: u8 = 0x03;

fn c2_request(cmd: u8) -> Vec<u8> {
    let mut request = vec![0x06, 0xff, 0xbb, cmd, 0x00];
    request.resize(52, 0);
    request
}

fn parse_c2_level(r: &[u8]) -> Option<u8> {
    if r.len() <= 0x07 || r[..4] != [0x06, 0xff, 0xbb, C2_CMD_LEVEL] {
        return None;
    }
    (r[0x07] <= 100).then_some(r[0x07])
}

fn parse_c2_charging(r: &[u8]) -> Option<bool> {
    if r.len() <= 0x04 || r[..4] != [0x06, 0xff, 0xbb, C2_CMD_CHARGING] {
        return None;
    }
    Some(r[0x04] == 1)
}

fn c2_query(collection: &Collection, cmd: u8) -> Option<Vec<u8>> {
    let mut handle = collection.open()?;
    if !handle.write(&c2_request(cmd), until(100)) {
        return None;
    }
    std::thread::sleep(C2_WRITE_TIMEOUT);
    let deadline = Instant::now() + C2_READ_TIMEOUT;
    let mut reply = handle.read(deadline)?;
    reply.truncate(C2_REPLY_LEN);
    Some(reply)
}

// Kingston (0951:1718) 使用更长的交换：报告 id 0x06 请求，0x0B 回显。
const K_PACKET_LEN: usize = 62;
const K_READ_LEN: usize = 64;
const K_CMD_LEVEL: u8 = 0x02;
const K_CMD_CHARGING: u8 = 0x03;

fn kingston_request(cmd: u8) -> Vec<u8> {
    let mut request = vec![
        0x06, 0x00, 0x02, 0x00, 0x9a, 0x00, 0x00, 0x68, 0x4a, 0x8e, 0x0a, 0x00, 0x00, 0x00,
        0xbb, cmd, 0x00,
    ];
    request.resize(K_PACKET_LEN, 0);
    request
}

fn parse_kingston_reply(r: &[u8], cmd: u8) -> Option<&[u8]> {
    if r.len() < 8 || r[0] != 0x0b || r[2] != 0xbb || r[3] != cmd {
        return None;
    }
    Some(r)
}

fn parse_kingston_level(reply: &[u8]) -> Option<u8> {
    if reply.len() <= 7 {
        return None;
    }
    (reply[7] <= 100).then_some(reply[7])
}

fn parse_kingston_charging(reply: &[u8]) -> Option<bool> {
    let state = *reply.get(4)?;
    match state {
        0 => Some(false),
        1 | 2 => Some(true),
        _ => None,
    }
}

fn kingston_query(collection: &Collection, cmd: u8) -> Option<Vec<u8>> {
    let mut handle = collection.open()?;
    // Python `_query_kingston` 在每次 write 前调用 hid.get_input_report(0x06, 62)
    // （HeadsetControl 的 prepareDevice），结果被忽略。现在用 HidD_GetInputReport
    // 精确复刻按 report id 取报告的控制传输，不再用输入管道 read() 近似。
    handle.prepare_input_report(0x06);
    if !handle.write(&kingston_request(cmd), until(100)) {
        return None;
    }
    std::thread::sleep(Duration::from_millis(100));
    let mut reply = handle.read(Instant::now() + Duration::from_secs(1))?;
    reply.truncate(K_READ_LEN);
    if parse_kingston_reply(&reply, cmd).is_none() {
        return None;
    }
    Some(reply)
}

struct Cloud2;

impl Provider for Cloud2 {
    fn name(&self) -> &'static str {
        "hyperx"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        match vid {
            HYPERX_VID if matches!(pid, 0x0696 | 0x018b) => {
                let Some(c) = pick(collections, 0xff90, 0x0303) else {
                    return Vec::new();
                };
                let Some(reply) = c2_query(c, C2_CMD_LEVEL) else {
                    return Vec::new();
                };
                let Some(level) = parse_c2_level(&reply) else {
                    return Vec::new();
                };
                let charging = c2_query(c, C2_CMD_CHARGING)
                    .as_deref()
                    .and_then(parse_c2_charging)
                    .unwrap_or(false);
                let mut st = headset(pid, "HyperX Cloud II Wireless", "hyperx");
                st.level = Some(level);
                st.charging = Some(charging);
                vec![st]
            }
            KINGSTON_VID if pid == 0x1718 => {
                let Some(c) = pick_vendor(collections, 0xff13, 0x0001) else {
                    return Vec::new();
                };
                let Some(reply) = kingston_query(c, K_CMD_LEVEL) else {
                    return Vec::new();
                };
                let Some(level) = parse_kingston_level(&reply) else {
                    return Vec::new();
                };
                let charging = kingston_query(c, K_CMD_CHARGING)
                    .as_deref()
                    .and_then(parse_kingston_charging)
                    .unwrap_or(false);
                let mut st = headset(pid, "HyperX Cloud II Wireless", "hyperx");
                st.level = Some(level);
                st.charging = Some(charging);
                vec![st]
            }
            _ => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// 2. hyperx_alpha2.py: Cloud Alpha 2 (03F0:08BE)
// ---------------------------------------------------------------------------

fn alpha2_request() -> Vec<u8> {
    let mut request = vec![0x50, 0x02];
    request.resize(64, 0);
    request
}

fn parse_alpha2(r: &[u8]) -> Option<(u8, bool)> {
    if r.len() <= 6 || r[0] != 0x51 || r[1] != 0x02 || r[2] > 100 {
        return None;
    }
    Some((r[2], r[6] & 0x80 != 0))
}

fn alpha2_query(collection: &Collection) -> Option<(u8, bool)> {
    let mut handle = collection.open()?;
    // NGENUITY 同时运行时队列里全是杂帧，先排空（最多 64 帧，每帧 5 ms）。
    let mut drained = 0;
    while drained < 64 {
        if handle.read(until(5)).is_none() {
            break;
        }
        drained += 1;
    }
    let _ = handle.write(&alpha2_request(), until(200));
    let end = until(1500);
    while Instant::now() < end {
        let deadline = (until(150)).min(end);
        if let Some(reply) = handle.read(deadline) {
            if let Some(parsed) = parse_alpha2(&reply) {
                return Some(parsed);
            }
        }
    }
    None
}

struct Alpha2;

impl Provider for Alpha2 {
    fn name(&self) -> &'static str {
        "hyperx_alpha2"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != HYPERX_VID || pid != 0x08be {
            return Vec::new();
        }
        let Some(c) = pick(collections, 0xff13, 0xff00) else {
            return Vec::new();
        };
        let Some((level, charging)) = alpha2_query(c) else {
            return Vec::new();
        };
        let mut st = headset(pid, "HyperX Cloud Alpha 2", "hyperx_alpha2");
        st.level = Some(level);
        st.charging = Some(charging);
        vec![st]
    }
}

// ---------------------------------------------------------------------------
// 3. hyperx_alpha_wireless.py: Cloud Alpha Wireless (03F0:098D)
// ---------------------------------------------------------------------------

const SEL_CONNECTION: u8 = 0x03;
const SEL_LEVEL: u8 = 0x0b;
const SEL_CHARGING: u8 = 0x0c;

fn aw_request(selector: u8) -> Vec<u8> {
    let mut request = vec![0x21, 0xbb, selector];
    request.resize(31, 0);
    request
}

fn aw_is_reply(r: &[u8], selector: u8) -> bool {
    r.len() > 3 && r[..3] == [0x21, 0xbb, selector]
}

fn parse_aw_connected(r: &[u8]) -> Option<bool> {
    if !aw_is_reply(r, SEL_CONNECTION) {
        return None;
    }
    Some(r[3] != 1)
}

fn parse_aw_level(r: &[u8]) -> Option<u8> {
    if !aw_is_reply(r, SEL_LEVEL) || r[3] > 100 {
        return None;
    }
    Some(r[3])
}

fn parse_aw_charging(r: &[u8]) -> Option<bool> {
    if !aw_is_reply(r, SEL_CHARGING) || r[3] > 1 {
        return None;
    }
    Some(r[3] == 1)
}

/// -> (请求已发出?, 与 selector 匹配的回复)
fn aw_ask(handle: &mut Handle, selector: u8) -> (bool, Option<Vec<u8>>) {
    if !handle.write(&aw_request(selector), until(200)) {
        return (false, None);
    }
    let end = until(2000);
    while Instant::now() < end {
        let deadline = (until(200)).min(end);
        if let Some(reply) = handle.read(deadline) {
            if aw_is_reply(&reply, selector) {
                return (true, Some(reply));
            }
        }
    }
    (true, None)
}

struct AlphaWireless;

impl Provider for AlphaWireless {
    fn name(&self) -> &'static str {
        "hyperx_alpha_wireless"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != HYPERX_VID || pid != 0x098d {
            return Vec::new();
        }
        let Some(c) = pick(collections, 0xff43, 0x0202) else {
            return Vec::new();
        };
        let Some(mut handle) = c.open() else {
            return Vec::new();
        };
        let (sent, connection) = aw_ask(&mut handle, SEL_CONNECTION);
        if !sent {
            return Vec::new();
        }
        let connected = connection
            .as_deref()
            .and_then(parse_aw_connected)
            .unwrap_or(false);
        let mut st = headset(pid, "HyperX Cloud Alpha Wireless", "hyperx_alpha_wireless");
        if !connected {
            st.online = false;
            st.charging = Some(false);
            return vec![st];
        }
        let (_, charging_reply) = aw_ask(&mut handle, SEL_CHARGING);
        let charging = charging_reply
            .as_deref()
            .and_then(parse_aw_charging)
            .unwrap_or(false);
        let (_, level_reply) = aw_ask(&mut handle, SEL_LEVEL);
        let level = level_reply.as_deref().and_then(parse_aw_level);
        if level.is_none() && !charging {
            st.online = false;
            st.charging = Some(false);
            return vec![st];
        }
        st.level = level;
        st.charging = Some(charging);
        vec![st]
    }
}

// ---------------------------------------------------------------------------
// 4. hyperx_cloud3.py: Cloud III Wireless / Cloud II Core Wireless (03F0)
// ---------------------------------------------------------------------------

const C3_CMD_BATTERY: u8 = 0x89;
const C3_RESP_BATTERY: u8 = 0x0d;
const C3_CMD_CHARGING: u8 = 0x8a;
const C3_RESP_CHARGING: u8 = 0x0c;

fn c3_request(cmd: u8) -> Vec<u8> {
    let mut request = vec![0x66, cmd];
    request.resize(62, 0);
    request
}

fn parse_c3_battery(r: &[u8]) -> Option<u8> {
    if r.len() <= 4 || r[0] != 0x66 || ![C3_CMD_BATTERY, C3_RESP_BATTERY].contains(&r[1]) {
        return None;
    }
    if r[2] == 0 && r[3] == 0 {
        return None;
    }
    (r[4] <= 100).then_some(r[4])
}

fn parse_c3_charging(r: &[u8]) -> Option<bool> {
    if r.len() <= 2 || r[0] != 0x66 || ![C3_CMD_CHARGING, C3_RESP_CHARGING].contains(&r[1]) {
        return None;
    }
    match r[2] {
        0 => Some(false),
        1 | 2 => Some(true),
        _ => None,
    }
}

fn c3_write(handle: &mut Handle, packet: &[u8]) -> bool {
    if handle.write(packet, until(200)) {
        return true;
    }
    // 某些 Windows 接收器只接受 feature report；Rust 的 write() 只给出布尔失败，
    // 因此失败时直接尝试 Python 侧的 fallback。
    handle.set_feature(packet, until(200))
}

fn c3_query(handle: &mut Handle, cmd: u8, expected: &[u8]) -> Option<Vec<u8>> {
    if !c3_write(handle, &c3_request(cmd)) {
        return None;
    }
    std::thread::sleep(Duration::from_millis(100));
    for _ in 0..6 {
        if let Some(reply) = handle.read(until(200)) {
            if reply.first() == Some(&0x66) && reply.get(1).is_some_and(|b| expected.contains(b)) {
                return Some(reply);
            }
        }
    }
    None
}

fn c3_name(pid: u16) -> &'static str {
    match pid {
        0x0795 | 0x0995 => "HyperX Cloud II Core Wireless",
        _ => "HyperX Cloud III Wireless",
    }
}

fn c3_canonical(pid: u16) -> u16 {
    match pid {
        0x0c9d => 0x05b7,
        0x0795 => 0x0995,
        other => other,
    }
}

struct Cloud3;

impl Provider for Cloud3 {
    fn name(&self) -> &'static str {
        "hyperx_cloud3"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != HYPERX_VID || !matches!(pid, 0x05b7 | 0x0fcc | 0x0c9d | 0x0795 | 0x0995) {
            return Vec::new();
        }
        let Some(c) = pick(collections, 0xff13, 0x0001) else {
            return Vec::new();
        };
        let Some(mut handle) = c.open() else {
            return Vec::new();
        };
        let Some(reply) = c3_query(
            &mut handle,
            C3_CMD_BATTERY,
            &[C3_CMD_BATTERY, C3_RESP_BATTERY],
        ) else {
            return Vec::new();
        };
        let Some(level) = parse_c3_battery(&reply) else {
            return Vec::new();
        };
        let charging = c3_query(
            &mut handle,
            C3_CMD_CHARGING,
            &[C3_CMD_CHARGING, C3_RESP_CHARGING],
        )
        .as_deref()
        .and_then(parse_c3_charging)
        .unwrap_or(false);
        let mut st = headset(c3_canonical(pid), c3_name(pid), "hyperx");
        st.level = Some(level);
        st.charging = Some(charging);
        vec![st]
    }
}

// ---------------------------------------------------------------------------
// 5. hyperx_cloud3s.py: Cloud III S Wireless (03F0:02CC / 03F0:06BE)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SKind {
    Battery,
    Charging,
}

fn s3_request(cmd: u8) -> Vec<u8> {
    let mut request = vec![0x0c, 0x02, 0x03, 0x01, 0x00, cmd];
    request.resize(64, 0);
    request
}

fn parse_s3(r: &[u8]) -> Option<(SKind, u8)> {
    if r.len() < 7 {
        return None;
    }
    if r[0] == 0x0c {
        if r[5] == 0x06 {
            return (r[6] <= 100).then_some((SKind::Battery, r[6]));
        }
        if r[5] == 0x48 {
            return Some((SKind::Charging, r[6]));
        }
    } else if r[0] == 0x0d {
        if r[4] == 1 {
            return (r[5] <= 100).then_some((SKind::Battery, r[5]));
        }
        if r[4] == 10 {
            return Some((SKind::Charging, r[5]));
        }
    }
    None
}

fn s3_charging_flag(state: u8) -> Option<bool> {
    match state {
        0 => Some(false),
        1 | 2 => Some(true),
        _ => None,
    }
}

/// 发送 0x0C 输出报告并从该接收器的所有集合上读取回复。
/// 0x01c0:0001@MI_03 只读集合由 hid.rs 特别保留，答案往往落在这里。
fn s3_ask(collections: &[Collection], cmd: u8, want: SKind) -> Option<u8> {
    let mut ordered: Vec<&Collection> = collections.iter().collect();
    ordered.sort_by_key(|c| (if c.usage_page >= 0xff00 { 0 } else { 1 }, c.usage_page));
    let mut handles: Vec<(&Collection, Handle)> = ordered
        .into_iter()
        .filter_map(|c| c.open().map(|h| (c, h)))
        .collect();
    let packet = s3_request(cmd);
    let mut sent = false;
    for (c, handle) in &mut handles {
        if c.usage_page >= 0xff00 && c.supports_report(0x0c, true) {
            if handle.write(&packet, until(200)) {
                sent = true;
                break;
            }
        }
    }
    if !sent {
        return None;
    }
    let end = until(1000);
    while Instant::now() < end {
        let mut got = false;
        for (_, handle) in &mut handles {
            let deadline = (until(10)).min(end);
            if let Some(reply) = handle.read(deadline) {
                got = true;
                if let Some((kind, value)) = parse_s3(&reply) {
                    if kind == want {
                        return Some(value);
                    }
                }
            }
        }
        if !got {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    None
}

struct Cloud3S;

impl Provider for Cloud3S {
    fn name(&self) -> &'static str {
        "hyperx_cloud3s"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != HYPERX_VID || !matches!(pid, 0x02cc | 0x06be) {
            return Vec::new();
        }
        let Some(level) = s3_ask(collections, 0x06, SKind::Battery) else {
            return Vec::new();
        };
        let charging = s3_ask(collections, 0x48, SKind::Charging)
            .and_then(s3_charging_flag)
            .unwrap_or(false);
        let mut st = headset(pid, "HyperX Cloud III S Wireless", "hyperx");
        st.level = Some(level);
        st.charging = Some(charging);
        vec![st]
    }
}

// ---------------------------------------------------------------------------
// 6. hyperx_cloud_core.py: Cloud Core Wireless (03F0 / 0951, 173F + 1740)
// ---------------------------------------------------------------------------

const CC_VOLTAGES: [u16; 20] = [
    3328, 3584, 3674, 3704, 3732, 3744, 3754, 3764, 3774, 3784, 3794, 3804, 3824, 3840, 3860,
    3890, 3910, 3940, 3960, 3970,
];
const CC_PERCENTS: [u8; 20] = [
    5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95, 100,
];
const CC_REQUEST: [u8; 64] = [
    0xff, 0x07, 0x00, 0xfd, 0x04, 0x0c, 0xf1, 0x02, 0x01, 0x04, 0xf0, 0x0c, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

fn voltage_to_percent(voltage: u16) -> u8 {
    let index = CC_VOLTAGES.partition_point(|&v| v <= voltage).saturating_sub(1);
    CC_PERCENTS[index]
}

fn parse_cc(reply: &[u8]) -> Option<(u16, Option<bool>)> {
    if reply.len() < 13 || reply[0] != 0xff || reply[1] != 0x12 {
        return None;
    }
    let voltage = u16::from_le_bytes([reply[11], reply[12]]);
    if !(2500..=4500).contains(&voltage) {
        return None;
    }
    let charging = match reply[9] {
        0x05 => Some(true),
        0x03 => Some(false),
        _ => None,
    };
    Some((voltage, charging))
}

fn cc_query(collection: &Collection) -> Option<(u16, Option<bool>)> {
    let mut handle = collection.open()?;
    if !handle.set_feature(&CC_REQUEST, until(200)) {
        return None;
    }
    let end = until(250);
    while Instant::now() < end {
        let deadline = (until(10)).min(end);
        if let Some(reply) = handle.get_feature(0xff, deadline) {
            if let Some(parsed) = parse_cc(&reply) {
                return Some(parsed);
            }
        }
    }
    None
}

struct CloudCore {
    history: Mutex<HashMap<String, VecDeque<u16>>>,
}

impl Provider for CloudCore {
    fn name(&self) -> &'static str {
        "hyperx_cloud_core"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if !matches!((vid, pid), (HYPERX_VID, 0x173f | 0x1740) | (KINGSTON_VID, 0x173f | 0x1740)) {
            return Vec::new();
        }
        let Some(c) = pick(collections, 0xff00, 0x0001) else {
            return Vec::new();
        };
        let Some((voltage, charging)) = cc_query(c) else {
            return Vec::new();
        };
        let key = "hyperx:173f";
        let Ok(mut history) = self.history.lock() else {
            return Vec::new();
        };
        let entry = history.entry(key.into()).or_insert_with(|| VecDeque::with_capacity(20));
        entry.push_back(voltage);
        if entry.len() > 20 {
            entry.pop_front();
        }
        let average = (entry.iter().map(|v| *v as u32).sum::<u32>() / entry.len() as u32) as u16;
        let level = voltage_to_percent(average as u16);
        let mut st = headset(0x173f, "HyperX Cloud Core Wireless", "hyperx_cloud_core");
        st.level = Some(level);
        st.charging = Some(charging.unwrap_or(false));
        st.charging_known = charging.is_some();
        st.approximate = true; // Python 的 approx="about N%" 是粗粒度读数
        vec![st]
    }
}

// ---------------------------------------------------------------------------
// 7. hyperx_cloud_flight.py: Cloud Flight Wireless (0951:16C4 / 0951:1723)
// ---------------------------------------------------------------------------

const FLIGHT_REQUEST: [u8; 20] = [0x21, 0xff, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
const FLIGHT_THRESHOLD: u16 = 0x100b;

fn flight_estimate(voltage: u16) -> u8 {
    let v = voltage as f64;
    let percent = if voltage <= 3648 {
        0.00125 * v
    } else if voltage > 3975 {
        100.0
    } else {
        0.00000002547505 * v.powi(4)
            - 0.0003900299 * v.powi(3)
            + 2.238321 * v.powi(2)
            - 5706.256 * v
            + 5452299.0
    };
    // Python 不对多项式结果封顶；>100 的读数由 Engine 过滤，不在这里伪造 100%。
    percent.round_ties_even().clamp(0.0, 255.0) as u8
}

fn parse_flight(reply: &[u8]) -> Option<(Option<u8>, bool)> {
    if reply.len() != 15 && reply.len() != 20 {
        return None;
    }
    let voltage = u16::from_be_bytes([reply[3], reply[4]]);
    if voltage > FLIGHT_THRESHOLD {
        Some((None, true))
    } else {
        Some((Some(flight_estimate(voltage)), false))
    }
}

struct CloudFlight;

impl Provider for CloudFlight {
    fn name(&self) -> &'static str {
        "hyperx_cloud_flight"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != KINGSTON_VID || !matches!(pid, 0x16c4 | 0x1723) {
            return Vec::new();
        }
        let Some(c) = collections
            .iter()
            .find(|c| c.usage == 0x0303 && matches!(c.usage_page, 0xff53 | 0xff90))
        else {
            return Vec::new();
        };
        let Some(mut handle) = c.open() else {
            return Vec::new();
        };
        if !handle.write(&FLIGHT_REQUEST, until(200)) {
            return Vec::new();
        }
        let Some(mut reply) = handle.read(Instant::now() + Duration::from_secs(2)) else {
            return Vec::new();
        };
        reply.truncate(20);
        let Some((level, charging)) = parse_flight(&reply) else {
            return Vec::new();
        };
        let mut st = headset(pid, "HyperX Cloud Flight Wireless", "hyperx_cloud_flight");
        st.level = level;
        st.charging = Some(charging);
        vec![st]
    }
}

// ---------------------------------------------------------------------------
// 8. hyperx_ngenuity.py: Pulsefire Haste 2 Wireless (03F0:0D97 / 03F0:0F98)
// ---------------------------------------------------------------------------

fn ng_request(cmd: u8, sub: u8) -> Vec<u8> {
    let mut request = vec![cmd, sub];
    request.resize(64, 0);
    request
}

fn parse_ng_battery(r: &[u8]) -> Option<(u8, bool, bool)> {
    if r.len() < 10 || r[0] != 0x51 || r[1] != 0x02 || r[2] > 100 {
        return None;
    }
    let (charging, known) = match r[3] {
        0x00 => (false, true),
        0x01 | 0x02 => (true, true),
        _ => (false, false),
    };
    Some((r[2], charging, known))
}

fn parse_ng_link(r: &[u8]) -> Option<bool> {
    if r.len() < 3 || r[0] != 0x53 || r[1] != 0x01 {
        return None;
    }
    Some(r[2] != 0)
}

fn ng_ask(handle: &mut Handle, cmd: u8, sub: u8, reply_id: u8) -> Option<Vec<u8>> {
    if !handle.write(&ng_request(cmd, sub), until(200)) {
        return None;
    }
    std::thread::sleep(Duration::from_millis(10));
    let end = until(400);
    while Instant::now() < end {
        let deadline = (until(50)).min(end);
        if let Some(reply) = handle.read(deadline) {
            if reply.first() == Some(&reply_id) && reply.get(1) == Some(&sub) {
                return Some(reply);
            }
        }
    }
    None
}

struct Ngenuity {
    // 鼠标睡眠时最多保留 300 秒的灰色读数（Python ASLEEP_KEEP）。
    last: Mutex<HashMap<String, (u8, bool, bool, Instant)>>,
}

impl Provider for Ngenuity {
    fn name(&self) -> &'static str {
        "hyperx_ngenuity"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != HYPERX_VID || !matches!(pid, 0x0d97 | 0x0f98) {
            return Vec::new();
        }
        let Some(c) = collections
            .iter()
            .filter(|c| c.usage_page == 0xff00)
            .min_by_key(|c| (c.interface != Some(2), c.usage != 0x0001))
        else {
            return Vec::new();
        };
        let key = format!("hyperx_ngenuity:{pid:04x}");
        let name = "HyperX Pulsefire Haste 2 Wireless";
        let Some(mut handle) = c.open() else {
            return Vec::new();
        };
        let link = ng_ask(&mut handle, 0x52, 0x01, 0x53).and_then(|r| parse_ng_link(&r));
        if link == Some(false) {
            return self.cached(&key, name);
        }
        let Some(reply) = ng_ask(&mut handle, 0x50, 0x02, 0x51) else {
            return self.cached(&key, name);
        };
        let Some((level, charging, known)) = parse_ng_battery(&reply) else {
            return self.cached(&key, name);
        };
        if let Ok(mut last) = self.last.lock() {
            last.insert(key.clone(), (level, charging, known, Instant::now()));
        }
        let mut st = Status::new(key, name);
        st.kind = "mouse".into();
        st.source = "hyperx_ngenuity".into();
        st.level = Some(level);
        st.charging = Some(charging);
        st.charging_known = known;
        vec![st]
    }
}

impl Ngenuity {
    fn cached(&self, key: &str, name: &str) -> Vec<Status> {
        let Ok(last) = self.last.lock() else {
            return Vec::new();
        };
        let Some((level, charging, known, at)) = last.get(key) else {
            return Vec::new();
        };
        if at.elapsed() > Duration::from_secs(300) {
            return Vec::new();
        }
        let mut st = Status::new(key, name);
        st.kind = "mouse".into();
        st.source = "hyperx_ngenuity".into();
        st.level = Some(*level);
        st.charging = Some(*charging);
        st.charging_known = *known;
        st.online = false;
        vec![st]
    }
}

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(Cloud2),
        Box::new(Alpha2),
        Box::new(AlphaWireless),
        Box::new(Cloud3),
        Box::new(Cloud3S),
        Box::new(CloudCore {
            history: Mutex::new(HashMap::new()),
        }),
        Box::new(CloudFlight),
        Box::new(Ngenuity {
            last: Mutex::new(HashMap::new()),
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud2_requests_and_parsers() {
        assert_eq!(c2_request(0x02).len(), 52);
        assert_eq!(&c2_request(0x03)[..4], &[0x06, 0xff, 0xbb, 0x03]);
        let mut reply = vec![0u8; 20];
        reply[..4].copy_from_slice(&[0x06, 0xff, 0xbb, 0x02]);
        reply[7] = 76;
        assert_eq!(parse_c2_level(&reply), Some(76));
        reply[7] = 101;
        assert_eq!(parse_c2_level(&reply), None);
        reply[..4].copy_from_slice(&[0x06, 0xff, 0xbb, 0x03]);
        reply[4] = 1;
        assert_eq!(parse_c2_charging(&reply), Some(true));
        reply[4] = 0;
        assert_eq!(parse_c2_charging(&reply), Some(false));
        reply[3] = 0x02;
        assert_eq!(parse_c2_charging(&reply), None);
    }

    #[test]
    fn kingston_requests_and_parsers() {
        let request = kingston_request(0x02);
        assert_eq!(request.len(), 62);
        assert_eq!(&request[..17], &[
            0x06, 0x00, 0x02, 0x00, 0x9a, 0x00, 0x00, 0x68, 0x4a, 0x8e, 0x0a, 0x00, 0x00, 0x00,
            0xbb, 0x02, 0x00,
        ]);
        let mut reply = vec![0u8; 64];
        reply[..4].copy_from_slice(&[0x0b, 0x00, 0xbb, 0x02]);
        reply[7] = 55;
        assert!(parse_kingston_reply(&reply, 0x02).is_some());
        assert_eq!(parse_kingston_level(&reply), Some(55));
        reply[3] = 0x03;
        reply[4] = 2;
        assert_eq!(parse_kingston_charging(&reply), Some(true));
        reply[4] = 0;
        assert_eq!(parse_kingston_charging(&reply), Some(false));
        reply[4] = 9;
        assert_eq!(parse_kingston_charging(&reply), None);
        reply[0] = 0x0c;
        assert!(parse_kingston_reply(&reply, 0x03).is_none());
    }

    #[test]
    fn kingston_prepare_report_id_differs_from_reply() {
        // Python 的 prepare read 用 report id 0x06（与请求首字节相同），
        // 真正的回复是 0x0B；所以短 deadline 的 read() 只能做 best-effort drain，
        // 无法按 id 取到 0x06 报告——严格等价性说明见 kingston_query 注释。
        let request = kingston_request(K_CMD_LEVEL);
        assert_eq!(request[0], 0x06);
        let mut reply = vec![0u8; K_READ_LEN];
        reply[..4].copy_from_slice(&[0x0B, 0x00, 0xBB, K_CMD_LEVEL]);
        assert!(parse_kingston_reply(&reply, K_CMD_LEVEL).is_some());
        reply[0] = 0x06;
        assert!(parse_kingston_reply(&reply, K_CMD_LEVEL).is_none());
    }

    #[test]
    fn alpha2_parser() {
        assert_eq!(alpha2_request(), {
            let mut r = vec![0x50, 0x02];
            r.resize(64, 0);
            r
        });
        let mut reply = vec![0u8; 64];
        reply[..2].copy_from_slice(&[0x51, 0x02]);
        reply[2] = 51;
        reply[6] = 0x80;
        assert_eq!(parse_alpha2(&reply), Some((51, true)));
        reply[6] = 0;
        assert_eq!(parse_alpha2(&reply), Some((51, false)));
        reply[2] = 101;
        assert_eq!(parse_alpha2(&reply), None);
        reply[0] = 0x50;
        assert_eq!(parse_alpha2(&reply), None);
    }

    #[test]
    fn alpha_wireless_parsers() {
        assert_eq!(aw_request(0x0b).len(), 31);
        let mut reply = vec![0u8; 8];
        reply[..3].copy_from_slice(&[0x21, 0xbb, 0x03]);
        reply[3] = 1;
        assert_eq!(parse_aw_connected(&reply), Some(false));
        reply[3] = 0;
        assert_eq!(parse_aw_connected(&reply), Some(true));
        reply[2] = 0x0b;
        reply[3] = 88;
        assert_eq!(parse_aw_level(&reply), Some(88));
        reply[3] = 101;
        assert_eq!(parse_aw_level(&reply), None);
        reply[2] = 0x0c;
        reply[3] = 1;
        assert_eq!(parse_aw_charging(&reply), Some(true));
        reply[3] = 2;
        assert_eq!(parse_aw_charging(&reply), None);
        reply[1] = 0x00;
        assert_eq!(parse_aw_level(&reply), None);
    }

    #[test]
    fn cloud3_parsers() {
        assert_eq!(c3_request(0x89).len(), 62);
        let mut reply = vec![0u8; 62];
        reply[..2].copy_from_slice(&[0x66, 0x89]);
        reply[2] = 0x0e;
        reply[4] = 48;
        assert_eq!(parse_c3_battery(&reply), Some(48));
        reply[..2].copy_from_slice(&[0x66, 0x0d]);
        assert_eq!(parse_c3_battery(&reply), Some(48));
        reply[2] = 0;
        reply[3] = 0;
        assert_eq!(parse_c3_battery(&reply), None);
        reply[2] = 1;
        reply[4] = 101;
        assert_eq!(parse_c3_battery(&reply), None);
        reply[..2].copy_from_slice(&[0x66, 0x8a]);
        reply[2] = 2;
        assert_eq!(parse_c3_charging(&reply), Some(true));
        reply[2] = 0;
        assert_eq!(parse_c3_charging(&reply), Some(false));
        reply[2] = 7;
        assert_eq!(parse_c3_charging(&reply), None);
        assert_eq!(c3_canonical(0x0c9d), 0x05b7);
        assert_eq!(c3_canonical(0x0795), 0x0995);
    }

    #[test]
    fn cloud3s_parsers() {
        assert_eq!(s3_request(0x06), {
            let mut r = vec![0x0c, 0x02, 0x03, 0x01, 0x00, 0x06];
            r.resize(64, 0);
            r
        });
        let mut reply = vec![0u8; 64];
        reply[0] = 0x0c;
        reply[5] = 0x06;
        reply[6] = 64;
        assert_eq!(parse_s3(&reply), Some((SKind::Battery, 64)));
        reply[6] = 0xff;
        assert_eq!(parse_s3(&reply), None);
        reply[5] = 0x48;
        reply[6] = 2;
        assert_eq!(parse_s3(&reply), Some((SKind::Charging, 2)));
        assert_eq!(s3_charging_flag(2), Some(true));
        assert_eq!(s3_charging_flag(0), Some(false));
        assert_eq!(s3_charging_flag(9), None);
        reply.fill(0);
        reply[0] = 0x0d;
        reply[4] = 1;
        reply[5] = 72;
        assert_eq!(parse_s3(&reply), Some((SKind::Battery, 72)));
        reply[4] = 10;
        reply[5] = 1;
        assert_eq!(parse_s3(&reply), Some((SKind::Charging, 1)));
    }

    #[test]
    fn cloud_core_curve_and_parser() {
        assert_eq!(voltage_to_percent(3000), 5);
        assert_eq!(voltage_to_percent(3700), 15); // 3700 落在 3674 与 3704 之间
        assert_eq!(voltage_to_percent(3970), 100);
        let mut reply = vec![0u8; 64];
        reply[..2].copy_from_slice(&[0xff, 0x12]);
        reply[9] = 0x05;
        reply[11] = 0xb0;
        reply[12] = 0x0e; // 3760 mV
        assert_eq!(parse_cc(&reply), Some((3760, Some(true))));
        reply[9] = 0x03;
        assert_eq!(parse_cc(&reply), Some((3760, Some(false))));
        reply[9] = 0x00;
        assert_eq!(parse_cc(&reply), Some((3760, None)));
        reply[12] = 0x00; // 176 mV: out of range
        assert_eq!(parse_cc(&reply), None);
        reply[1] = 0x11;
        assert_eq!(parse_cc(&reply), None);
    }

    #[test]
    fn cloud_flight_curve_and_parser() {
        assert_eq!(flight_estimate(3000), 4);
        assert_eq!(flight_estimate(3975), 100);
        let v = flight_estimate(3700);
        assert!((1..=100).contains(&v));
        let mut reply = vec![0u8; 20];
        reply[3] = 0x0e;
        reply[4] = 0x68; // 3688 mV
        assert_eq!(parse_flight(&reply), Some((Some(flight_estimate(0x0e68)), false)));
        reply[4] = 0x0c; // 0x100c > 0x100b
        reply[3] = 0x10;
        assert_eq!(parse_flight(&reply), Some((None, true)));
        reply.truncate(15); // 15 字节回复同样有效
        reply[3] = 0x0e;
        reply[4] = 0x68;
        assert_eq!(parse_flight(&reply), Some((Some(flight_estimate(0x0e68)), false)));
        reply.pop();
        assert_eq!(parse_flight(&reply), None);
    }

    #[test]
    fn ngenuity_parsers() {
        assert_eq!(ng_request(0x50, 0x02).len(), 64);
        let mut reply = vec![0u8; 64];
        reply[..2].copy_from_slice(&[0x51, 0x02]);
        reply[2] = 67;
        reply[3] = 0x02;
        reply[6] = 0x2c;
        reply[7] = 0x0f;
        assert_eq!(parse_ng_battery(&reply), Some((67, true, true)));
        reply[3] = 0x03;
        assert_eq!(parse_ng_battery(&reply), Some((67, false, false)));
        reply[3] = 0xff;
        assert_eq!(parse_ng_battery(&reply), Some((67, false, false)));
        reply[2] = 101;
        assert_eq!(parse_ng_battery(&reply), None);
        let mut link = vec![0u8; 4];
        link[..2].copy_from_slice(&[0x53, 0x01]);
        link[2] = 1;
        assert_eq!(parse_ng_link(&link), Some(true));
        link[2] = 0;
        assert_eq!(parse_ng_link(&link), Some(false));
        link[0] = 0x51;
        assert_eq!(parse_ng_link(&link), None);
    }
}
