//! Razer 设备电量协议。
//!
//! 通用 90 字节 feature report（0x07:0x80 / 0x07:0x84）精确移植自
//! `upstream/providers/razer.py`；本文件还包含其专用子协议（全部只从 Python
//! 源码逆向出的字节格式移植，未做实机验证）：
//!   - blackshark2020：0528 的 64 字节 FF 特征报告
//!   - blackshark：0555/0556/0583 的 64 字节 "PA" output/input 协议
//!   - blackshark_v3：057A/0577 的 0x02/ff14 会话帧
//!   - blackshark_v3x：057C/057D 的 0x07/ff82 特征报告
//!   - blackshark_mtk / kraken_v4：0565/056E / 056C/056B 的 MediaTek 0x02 帧
//!   - barracuda：053A 的 0x01 "PA" 变体帧
//!   - barracuda_x：0552 的 Airoha RACE 协议
//!
//! 仍明确不处理：052E（2020 充电线，电池从 0528 读）、0536（Barracuda X
//! 充电线，无输出端点）、0550（Python 侧只有文档、无权威自动交换协议）。
use crate::hid::{Collection, Handle};
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const VID: u16 = 0x1532;

// 状态字节（Python 中 STATUS_FOREIGN=0x100 不是单字节，故统一用 u16）。
const STATUS_OK: u16 = 0x02;
const STATUS_BUSY: u16 = 0x01;
const STATUS_TIMEOUT: u16 = 0x04;
const STATUS_FOREIGN: u16 = 0x100;
#[allow(dead_code)]
const STATUS_NOT_SUPPORTED: u16 = 0x05;

const SENDS: u8 = 3;
const ASLEEP_KEEP: Duration = Duration::from_secs(300);
const DEAD_KEEP: Duration = Duration::from_secs(300);
const QUERY_TIMEOUT: Duration = Duration::from_secs(1);
const IO_TIMEOUT: Duration = Duration::from_millis(200);

const TRANSACTION_IDS: &[u8] = &[0x1F, 0x3F, 0xFF, 0x9F, 0x08];
const PA_PIDS: &[u16] = &[0x0555, 0x0556, 0x0583];

// PID -> (名称, 首选 transaction id)，逐项对应 Python 的 KNOWN。
const KNOWN: &[(u16, &str, u8)] = &[
    (0x0528, "Razer BlackShark V2 Pro (2020)", 0x3F),
    (0x0555, "Razer BlackShark V2 Pro (2023)", 0x3F),
    (0x0556, "Razer BlackShark V2 Pro (2023)", 0x3F),
    (0x0557, "Razer BlackShark V2 Pro (2023)", 0x3F),
    (0x057A, "Razer BlackShark V3", 0x3F),
    (0x0577, "Razer BlackShark V3 Pro", 0x3F),
    (0x00A4, "Razer Mouse Dock Pro", 0x1F),
    (0x0290, "Razer DeathStalker V2 Pro", 0x9F),
    (0x0292, "Razer DeathStalker V2 Pro", 0x1F),
    (0x0296, "Razer DeathStalker V2 Pro TKL", 0x9F),
    (0x0298, "Razer DeathStalker V2 Pro TKL", 0x1F),
    (0x0271, "Razer BlackWidow V3 Mini HyperSpeed", 0x9F),
    (0x0258, "Razer BlackWidow V3 Mini HyperSpeed", 0x1F),
    (0x02BA, "Razer BlackWidow V4 Mini HyperSpeed", 0x9F),
    (0x02B9, "Razer BlackWidow V4 Mini HyperSpeed", 0x1F),
    (0x02D5, "Razer BlackWidow V4 Tenkeyless HyperSpeed", 0x9F),
    (0x02D7, "Razer BlackWidow V4 Tenkeyless HyperSpeed", 0x1F),
    (0x001F, "Razer Naga Epic", 0xFF),
    (0x0024, "Razer Mamba (2012)", 0xFF),
    (0x0025, "Razer Mamba (2012)", 0xFF),
    (0x0032, "Razer Ouroboros", 0xFF),
    (0x003E, "Razer Naga Epic Chroma", 0xFF),
    (0x003F, "Razer Naga Epic Chroma", 0xFF),
    (0x0044, "Razer Mamba", 0xFF),
    (0x0045, "Razer Mamba", 0xFF),
    (0x0059, "Razer Lancehead", 0x3F),
    (0x005A, "Razer Lancehead", 0x3F),
    (0x0062, "Razer Atheris", 0x1F),
    (0x006F, "Razer Lancehead Wireless", 0x1F),
    (0x0070, "Razer Lancehead Wireless", 0x1F),
    (0x0072, "Razer Mamba Wireless", 0x3F),
    (0x0073, "Razer Mamba Wireless", 0x3F),
    (0x0077, "Razer Pro Click", 0x1F),
    (0x0080, "Razer Pro Click", 0x1F),
    (0x007A, "Razer Viper Ultimate", 0xFF),
    (0x007B, "Razer Viper Ultimate", 0xFF),
    (0x007C, "Razer DeathAdder V2 Pro", 0x3F),
    (0x007D, "Razer DeathAdder V2 Pro", 0x3F),
    (0x0083, "Razer Basilisk X HyperSpeed", 0xFF),
    (0x0086, "Razer Basilisk Ultimate", 0x1F),
    (0x0088, "Razer Basilisk Ultimate", 0x1F),
    (0x008F, "Razer Naga Pro", 0x1F),
    (0x0090, "Razer Naga Pro", 0x1F),
    (0x0094, "Razer Orochi V2", 0x1F),
    (0x009A, "Razer Pro Click Mini", 0x1F),
    (0x009C, "Razer DeathAdder V2 X HyperSpeed", 0x1F),
    (0x009E, "Razer Viper Mini SE", 0x1F),
    (0x009F, "Razer Viper Mini SE", 0x1F),
    (0x00A5, "Razer Viper V2 Pro", 0x1F),
    (0x00A6, "Razer Viper V2 Pro", 0x1F),
    (0x00A7, "Razer Naga V2 Pro", 0x1F),
    (0x00A8, "Razer Naga V2 Pro", 0x1F),
    (0x00AA, "Razer Basilisk V3 Pro", 0x1F),
    (0x00AB, "Razer Basilisk V3 Pro", 0x1F),
    (0x00AF, "Razer Cobra Pro", 0x1F),
    (0x00B0, "Razer Cobra Pro", 0x1F),
    (0x00B3, "Razer HyperPolling Dongle", 0x1F),
    (0x00B4, "Razer Naga V2 HyperSpeed", 0x1F),
    (0x00B6, "Razer DeathAdder V3 Pro", 0x1F),
    (0x00B7, "Razer DeathAdder V3 Pro", 0x1F),
    (0x00B8, "Razer Viper V3 HyperSpeed", 0x1F),
    (0x00B9, "Razer Basilisk V3 X HyperSpeed", 0x1F),
    (0x00BE, "Razer DeathAdder V4 Pro", 0x1F),
    (0x00BF, "Razer DeathAdder V4 Pro", 0x1F),
    (0x00C0, "Razer Viper V3 Pro", 0x1F),
    (0x00C1, "Razer Viper V3 Pro", 0x1F),
    (0x00DE, "Razer Viper V3 Pro SE", 0x1F),
    (0x00DF, "Razer Viper V3 Pro SE", 0x1F),
    (0x00E5, "Razer Viper V4 Pro", 0x1F),
    (0x00E6, "Razer Viper V4 Pro", 0x1F),
    (0x00E2, "Razer Basilisk V4 Pro", 0x1F),
    (0x00E3, "Razer Basilisk V4 Pro", 0x1F),
    (0x00C2, "Razer DeathAdder V3 Pro", 0x1F),
    (0x00C3, "Razer DeathAdder V3 Pro", 0x1F),
    (0x00C4, "Razer DeathAdder V3 HyperSpeed", 0x1F),
    (0x00C5, "Razer DeathAdder V3 HyperSpeed", 0x1F),
    (0x00C7, "Razer Pro Click V2 Vertical Edition", 0x1F),
    (0x00C8, "Razer Pro Click V2 Vertical Edition", 0x1F),
    (0x00CC, "Razer Basilisk V3 Pro 35K", 0x1F),
    (0x00CD, "Razer Basilisk V3 Pro 35K", 0x1F),
    (0x00D0, "Razer Pro Click V2", 0x1F),
    (0x00D1, "Razer Pro Click V2", 0x1F),
    (0x00D3, "Razer Basilisk Mobile", 0x1F),
    (0x00D4, "Razer Basilisk Mobile", 0x1F),
    (0x00D6, "Razer Basilisk V3 Pro 35K Phantom Green", 0x1F),
    (0x00D7, "Razer Basilisk V3 Pro 35K Phantom Green", 0x1F),
    (0x025A, "Razer BlackWidow V3 Pro", 0x3F),
    (0x025C, "Razer BlackWidow V3 Pro", 0x9F),
    (0x027B, "Razer Pro Type Ultra", 0x9F),
];

const KEYBOARD_INTERFACE: &[(u16, u8)] = &[
    (0x0290, 2), (0x0296, 2), (0x02D5, 2),
    (0x0292, 3), (0x0298, 3), (0x02D7, 3),
    (0x0271, 3), (0x0258, 3), (0x02BA, 3), (0x02B9, 3),
];

// 这些 PID 由本文件内的专用 provider 实现，通用 90 字节帧不再尝试，
// 以免把 64 字节专用帧误当 90 字节报告、白等多个 transaction id 的超时。
const DEDICATED_PIDS: &[u16] = &[
    0x0528,                     // blackshark2020
    0x0555, 0x0556, 0x0583,     // blackshark（PA）
    0x057A, 0x0577,             // blackshark_v3
    0x057C, 0x057D,             // blackshark_v3x
    0x0565, 0x056E,             // blackshark_mtk
    0x053A,                     // barracuda
    0x0552,                     // barracuda_x
    0x056B, 0x056C,             // kraken_v4
];

// 已全部放行到专用 provider；保留常量仅供通用帧的显式跳过检查。
// 052E / 0536 / 0550 不在 KNOWN 中，通用帧本就因缺少 product string 而跳过。
const SKIP_PIDS: &[u16] = &[];

fn known(pid: u16) -> Option<(&'static str, u8)> {
    KNOWN.iter().find(|(p, _, _)| *p == pid).map(|(_, n, t)| (*n, *t))
}

fn keyboard_interface(pid: u16) -> Option<u8> {
    KEYBOARD_INTERFACE.iter().find(|(p, _)| *p == pid).map(|(_, i)| *i)
}

/// 构造 90 字节请求；CRC 为 msg[2..88] 的 XOR（不含 transaction id）。
fn build_request(transaction_id: u8, cmd_class: u8, cmd_id: u8, size: u8) -> [u8; 90] {
    let mut msg = [0u8; 90];
    msg[1] = transaction_id;
    msg[5] = size;
    msg[6] = cmd_class;
    msg[7] = cmd_id;
    let mut crc = 0u8;
    for &b in &msg[2..88] {
        crc ^= b;
    }
    msg[88] = crc;
    msg
}

/// Windows 的 hidapi 会带回 report id 作为首字节；去掉后固定为 90 字节。
fn normalize_reply(resp: &[u8]) -> Option<&[u8]> {
    if resp.is_empty() {
        return None;
    }
    let data = if resp.len() >= 91 { &resp[1..91] } else { resp };
    (data.len() >= 90).then_some(data)
}

/// -> (status, arg1)；命令不匹配时 arg1 为 None（可能是别的软件占用集合）。
fn parse_reply(data: &[u8], cmd_class: u8, cmd_id: u8) -> (u16, Option<u8>) {
    let status = u16::from(data[0]);
    if data[6] != cmd_class || data[7] != cmd_id {
        return (status, None);
    }
    (status, Some(data[9]))
}

/// round(raw / 255 * 100)。该比值不可能恰好落在 .5 上，f64 round 与 Python round 一致。
fn to_percent(raw: u8) -> u8 {
    ((f64::from(raw) / 255.0) * 100.0).round() as u8
}

fn send(handle: &mut Handle, req: &[u8; 90]) -> bool {
    let mut frame = [0u8; 91];
    frame[1..].copy_from_slice(req);
    handle.set_feature(&frame, Instant::now() + IO_TIMEOUT)
}

/// Python `_query`：发送后轮询 reply，忽略 busy 与其它命令的应答，必要时重发。
fn query(handle: &mut Handle, tid: u8, cmd_class: u8, cmd_id: u8) -> (Option<u16>, Option<u8>) {
    let req = build_request(tid, cmd_class, cmd_id, 0x02);
    if !send(handle, &req) {
        return (None, None);
    }
    let overall = Instant::now() + QUERY_TIMEOUT;
    let (mut sends, mut foreign) = (1u8, 0u8);
    std::thread::sleep(Duration::from_millis(60));
    loop {
        let resp = match handle.get_feature(0, Instant::now() + IO_TIMEOUT) {
            Some(v) => v,
            None => return (None, None),
        };
        let data = match normalize_reply(&resp) {
            Some(d) => d,
            None => return (None, None),
        };
        let (status, value) = parse_reply(data, cmd_class, cmd_id);
        let is_foreign = status == STATUS_OK && value.is_none();
        if Instant::now() < overall && (status == STATUS_BUSY || is_foreign) {
            if is_foreign {
                foreign += 1;
                if foreign % 4 == 0 && sends < SENDS {
                    if !send(handle, &req) {
                        return (None, None);
                    }
                    sends += 1;
                }
            }
            std::thread::sleep(Duration::from_millis(80));
            continue;
        }
        if is_foreign {
            return (Some(STATUS_FOREIGN), None);
        }
        return (Some(status), value);
    }
}

/// Python `_read` -> (status, level%, charging)。
fn read(collection: &Collection, tid: u8) -> (Option<u16>, Option<u8>, Option<bool>) {
    let Some(mut handle) = collection.open() else {
        return (None, None, None);
    };
    let (status, raw) = query(&mut handle, tid, 0x07, 0x80);
    if status != Some(STATUS_OK) || raw.is_none() {
        return (status, None, None);
    }
    let level = to_percent(raw.expect("checked above"));
    let (charging_status, charging_raw) = query(&mut handle, tid, 0x07, 0x84);
    let charging = charging_status == Some(STATUS_OK)
        && charging_raw.is_some_and(|v| v != 0);
    (status, Some(level), Some(charging))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct CollectionKey {
    usage_page: u16,
    usage: u16,
    interface: Option<u8>,
}

fn collection_key(c: &Collection) -> CollectionKey {
    CollectionKey { usage_page: c.usage_page, usage: c.usage, interface: c.interface }
}

#[derive(Clone, Copy)]
struct Candidate {
    key: CollectionKey,
    tid: u8,
}

#[derive(Default)]
struct State {
    cache: HashMap<String, Candidate>,
    good: HashMap<String, Candidate>,
    dead: HashMap<CollectionKey, Instant>,
    failing: HashMap<String, String>,
    last: HashMap<String, (u8, bool, Instant)>,
}

fn rank(c: &Collection, want: Option<u8>) -> (u8, u8, u8) {
    let iface = c.interface.unwrap_or(99);
    let preferred = if Some(iface) == want { 0 } else { 1 };
    let vendor = if matches!(c.usage_page, 0x0001 | 0xFF00) { 0 } else { 1 };
    (preferred, vendor, iface)
}

/// Python `_poll_group`：缓存 -> 排序后的集合/transaction id 探测 -> 超时兜底。
fn poll_group(
    state: &mut State,
    gkey: &str,
    collections: &[Collection],
    pref_tid: u8,
    want: Option<u8>,
) -> (Option<u16>, Option<u8>, Option<bool>) {
    let now = Instant::now();
    if let Some(cached) = state.cache.get(gkey).copied() {
        match collections.iter().find(|c| collection_key(c) == cached.key) {
            Some(collection) => {
                let result = read(collection, cached.tid);
                if result.0 == Some(STATUS_OK) {
                    state.good.insert(gkey.to_string(), cached);
                }
                if matches!(result.0, Some(STATUS_OK | STATUS_TIMEOUT | STATUS_FOREIGN)) {
                    return result;
                }
                state.cache.remove(gkey);
            }
            None => {
                state.cache.remove(gkey);
            }
        }
    }

    let mut tids = vec![pref_tid];
    for tid in TRANSACTION_IDS {
        if !tids.contains(tid) {
            tids.push(*tid);
        }
    }
    let mut sorted: Vec<&Collection> = collections.iter().collect();
    sorted.sort_by_key(|c| rank(c, want));

    let proven_good = state.good.get(gkey).copied();
    let mut timeout_hit = None;
    for collection in sorted {
        let key = collection_key(collection);
        let proven = proven_good.is_some_and(|g| g.key == key);
        if !proven && state.dead.get(&key).is_some_and(|at| *at > now) {
            continue;
        }
        let mut answered = false;
        for tid in &tids {
            let result = read(collection, *tid);
            let Some(status) = result.0 else {
                break; // 该集合拒绝 feature report，换下一个集合
            };
            answered = true;
            if status == STATUS_OK {
                let cand = Candidate { key, tid: *tid };
                state.good.insert(gkey.to_string(), cand);
                state.cache.insert(gkey.to_string(), cand);
                return result;
            }
            if status == STATUS_TIMEOUT {
                timeout_hit = Some(result);
                if proven {
                    if let Some(good) = proven_good {
                        state.cache.insert(gkey.to_string(), good);
                    }
                    return timeout_hit.expect("set above");
                }
                continue;
            }
            if status == STATUS_FOREIGN {
                state.cache.insert(gkey.to_string(), Candidate { key, tid: *tid });
                return result;
            }
        }
        if !answered && !proven {
            state.dead.insert(key, now + DEAD_KEEP);
        }
    }
    timeout_hit.unwrap_or((None, None, None))
}

pub struct Razer {
    state: Mutex<State>,
}

impl Razer {
    pub fn new() -> Self {
        Self { state: Mutex::new(State::default()) }
    }
}

impl Default for Razer {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for Razer {
    fn name(&self) -> &'static str {
        "razer"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID {
            return Vec::new();
        }
        let Some((name, pref_tid)) = known(pid) else {
            // Python: 不在 KNOWN 中的设备仅在产品名含无线关键字时探测；
            // Collection 未暴露 product string，因此保守跳过未知 PID。
            return Vec::new();
        };
        if SKIP_PIDS.contains(&pid) || DEDICATED_PIDS.contains(&pid) {
            return Vec::new();
        }

        let kind = if keyboard_interface(pid).is_some() { "keyboard" } else { "" };
        let serial = instance.rsplit('\\').next().unwrap_or(instance);
        let key = format!("razer:{pid:04x}:{serial}");
        let gkey = instance.to_string();
        let lower = name.to_lowercase();
        let is_headset = PA_PIDS.contains(&pid)
            || lower.contains("blackshark")
            || lower.contains("barracuda");

        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let result = poll_group(&mut state, &gkey, collections, pref_tid, keyboard_interface(pid));
        let now = Instant::now();

        if result.0 != Some(STATUS_OK) {
            let reason = match result.0 {
                None => "no reply".to_string(),
                Some(STATUS_FOREIGN) => "only replies for another app".to_string(),
                Some(status) => format!("status {status:02x}"),
            };
            state.failing.insert(gkey.clone(), reason);
            return keep_asleep(&state, &key, name, kind, is_headset, now);
        }

        state.failing.remove(&gkey);
        let level = result.1.unwrap_or(0);
        let charging = result.2.unwrap_or(false);
        state.last.insert(key.clone(), (level, charging, now));
        let mut status = Status::new(key, name);
        status.kind = kind.to_string();
        status.level = Some(level);
        status.charging = Some(charging);
        status.online = true;
        status.source = "razer".to_string();
        vec![status]
    }
}

fn keep_asleep(
    state: &State,
    key: &str,
    name: &str,
    kind: &str,
    is_headset: bool,
    now: Instant,
) -> Vec<Status> {
    if is_headset {
        return Vec::new();
    }
    let Some(&(level, charging, at)) = state.last.get(key) else {
        return Vec::new();
    };
    if now.saturating_duration_since(at) >= ASLEEP_KEEP {
        return Vec::new();
    }
    let mut status = Status::new(key, name);
    status.kind = kind.to_string();
    status.level = Some(level);
    status.charging = Some(charging);
    status.online = false;
    status.source = "razer".to_string();
    vec![status]
}

// ===========================================================================
// 专用子协议：字节格式全部逐项照搬 Python 源码，未做实机验证。
// ===========================================================================

fn xor_bytes(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc ^ b)
}

// --------------------------------------------------------------- blackshark2020
const BS2020_REPLY_PREFIX: &[u8] = &[0xff, 0x0f, 0x05, 0xfe, 0x12, 0x04, 0x1f, 0x08, 0x05];
#[allow(dead_code)] // 只在测试里校验旧帧不会被当成有效回复
const BS2020_NO_REPLY_PREFIX: &[u8] = &[0xff, 0x01, 0x00, 0xfe, 0x12, 0x04, 0x1f, 0x08, 0x05];

/// Python `REQUEST`：前 9 字节抓包固定值，其余补零至 64（report id 0xff 在 [0]）。
fn bs2020_request() -> [u8; 64] {
    let mut b = [0u8; 64];
    b[..9].copy_from_slice(&[0xff, 0x0a, 0x00, 0xfd, 0x04, 0x12, 0xf1, 0x02, 0x05]);
    b
}

/// Python `parse_reply` -> (state, mV, byte14)；保留 report id，只认完整 64 字节。
fn bs2020_parse_reply(data: &[u8]) -> Option<(u8, u16, u8)> {
    if data.len() != 64 || !data.starts_with(BS2020_REPLY_PREFIX) {
        return None;
    }
    Some((data[11], u16::from_be_bytes([data[12], data[13]]), data[14]))
}

pub struct Blackshark2020;

impl Provider for Blackshark2020 {
    fn name(&self) -> &'static str {
        "blackshark2020"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID || pid != 0x0528 {
            return Vec::new();
        }
        // Python `is_candidate`: usage_page ff00:0001。052E 充电线不在此读取。
        let Some(c) = collections.iter().find(|c| c.usage_page == 0xFF00 && c.usage == 0x01) else {
            return Vec::new();
        };
        let Some(mut handle) = c.open() else {
            return Vec::new();
        };
        if !handle.set_feature(&bs2020_request(), Instant::now() + IO_TIMEOUT) {
            return Vec::new();
        }
        let end = Instant::now() + Duration::from_millis(500);
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let resp = handle.get_feature(0xFF, Instant::now() + Duration::from_millis(100));
            if let Some(data) = resp.as_deref() {
                if let Some((state, voltage, raw14)) = bs2020_parse_reply(data) {
                    if voltage == 0 {
                        return Vec::new();
                    }
                    let charging = match state {
                        0x09 => Some(true),
                        0x01 | 0x02 | 0x03 | 0x06 => Some(false),
                        _ => None,
                    };
                    let level = if charging != Some(true) && raw14 <= 100 { Some(raw14) } else { None };
                    let serial = instance.rsplit('\\').next().unwrap_or(instance);
                    let mut status = Status::new(
                        format!("razer:{pid:04x}:{serial}"),
                        "Razer BlackShark V2 Pro (2020)",
                    );
                    status.level = level;
                    status.charging = Some(charging.unwrap_or(false));
                    status.charging_known = charging.is_some();
                    status.online = true;
                    status.source = "razer".to_string();
                    return vec![status];
                }
                // NO_REPLY_PREFIX / 其它回复都意味着 "offline" / "fail"，均无读数。
            }
            if Instant::now() >= end {
                return Vec::new();
            }
        }
    }
}

// -------------------------------------------------------------- blackshark (PA)
const PA_REPORT_ID: u8 = 0x02;
const PA_TYPE_REMOTE: u8 = 0x02;
const PA_TYPE_QUERY: u8 = 0x03;
const PA_CMD_REMOTE: u8 = 0xE1;
const PA_CMD_BATTERY: u8 = 0x21;
const PA_CMD_CHARGING: u8 = 0x2A;
const PA_REPLY_CMD_OFF: usize = 12;

fn pa_frame_query(cmd_id: u8, cmd_type: u8) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[0] = PA_REPORT_ID;
    b[1] = 0x80;
    b[2] = 8;
    b[5] = 0x50; // 'P'
    b[6] = 0x41; // 'A'
    b[7] = 0x08;
    b[9] = cmd_type;
    b[10] = cmd_id;
    b
}

fn pa_frame_remote(on: bool) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[0] = PA_REPORT_ID;
    b[1] = 0x80;
    b[2] = 0x07;
    b[5] = 0x50;
    b[6] = 0x41;
    b[7] = 0x0E;
    b[9] = PA_TYPE_REMOTE;
    b[10] = PA_CMD_REMOTE;
    b[11] = u8::from(on);
    b
}

/// ACK 的 payload。off=0 要求首字节为 report id；off=-1 兼容 hidapi 不带 report id。
fn pa_parse_reply(data: &[u8], cmd_id: u8) -> Option<Vec<u8>> {
    for off in [0i32, -1] {
        let c = (PA_REPLY_CMD_OFF as i32 + off) as usize;
        if data.len() > c + 3 && (off == -1 || data[0] == PA_REPORT_ID) {
            if data[c] == cmd_id && data[c + 1] == 0x01 {
                let n = data[c + 2] as usize;
                let end = (c + 3 + n.max(1)).min(data.len());
                return Some(data[c + 3..end].to_vec());
            }
        }
    }
    None
}

fn pa_drain(handle: &mut Handle) {
    for _ in 0..32 {
        if handle.read(Instant::now() + Duration::from_millis(5)).is_none() {
            break;
        }
    }
}

/// Python `PASession.query`：drain -> 牺牲帧唤醒 -> 3 次 remote+query -> 收 remote off。
fn pa_query(handle: &mut Handle, cmd_id: u8) -> Option<Vec<u8>> {
    pa_drain(handle);
    let remote = pa_frame_remote(true);
    handle.write(&remote, Instant::now() + IO_TIMEOUT);
    std::thread::sleep(Duration::from_millis(35));
    let mut result = None;
    for _ in 0..3 {
        handle.write(&remote, Instant::now() + IO_TIMEOUT);
        std::thread::sleep(Duration::from_millis(35));
        let query = pa_frame_query(cmd_id, PA_TYPE_QUERY);
        if !handle.write(&query, Instant::now() + IO_TIMEOUT) {
            break;
        }
        let end = Instant::now() + Duration::from_millis(150);
        while Instant::now() < end {
            let Some(data) = handle.read(Instant::now() + Duration::from_millis(50)) else {
                continue;
            };
            if let Some(payload) = pa_parse_reply(&data, cmd_id) {
                result = Some(payload);
                break;
            }
        }
        if result.is_some() {
            break;
        }
    }
    handle.write(&pa_frame_remote(false), Instant::now() + IO_TIMEOUT);
    result
}

pub struct BlacksharkPa {
    cache: Mutex<Option<CollectionKey>>,
}

impl BlacksharkPa {
    pub fn new() -> Self {
        Self { cache: Mutex::new(None) }
    }
}

impl Default for BlacksharkPa {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for BlacksharkPa {
    fn name(&self) -> &'static str {
        "blackshark"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID || !matches!(pid, 0x0555 | 0x0556 | 0x0583) {
            return Vec::new();
        }
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let mut ordered: Vec<&Collection> = collections
            .iter()
            .filter(|c| matches!(c.usage_page, 0xFF00 | 0xFF14))
            .collect();
        if let Some(cached) = *cache {
            ordered.sort_by_key(|c| usize::from(collection_key(c) != cached));
        }
        for c in ordered {
            let key = collection_key(c);
            let mut handle = c.open();
            let mut woke = false;
            'wake: for reopen in 0..2 {
                for attempt in 0..4 {
                    if let Some(h) = handle.as_mut() {
                        if h.write(&pa_frame_remote(true), Instant::now() + IO_TIMEOUT) {
                            woke = true;
                            break 'wake;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(150 * (attempt + 1)));
                }
                if reopen == 1 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
                handle = c.open();
            }
            if !woke {
                continue; // nowake：错误集合不可缓存
            }
            std::thread::sleep(Duration::from_millis(35));
            let Some(handle) = handle.as_mut() else {
                continue;
            };
            let Some(battery) = pa_query(handle, PA_CMD_BATTERY) else {
                *cache = Some(key); // offline：确认是该协议的集合，记住它
                break;
            };
            let Some(&level) = battery.first() else {
                continue;
            };
            if level > 100 {
                *cache = Some(key);
                break;
            }
            let charging = pa_query(handle, PA_CMD_CHARGING)
                .and_then(|v| v.first().copied())
                .unwrap_or(0)
                != 0;
            *cache = Some(key);
            let serial = instance.rsplit('\\').next().unwrap_or(instance);
            let mut status = Status::new(
                format!("razer:{pid:04x}:{serial}"),
                "Razer BlackShark V2 Pro (2023)",
            );
            status.level = Some(level);
            status.charging = Some(charging);
            status.online = true;
            status.source = "razer".to_string();
            return vec![status];
        }
        Vec::new()
    }
}

// ---------------------------------------------------------------- blackshark_v3
const V3_REPORT_ID: u8 = 0x02;
const V3_TRANSACTION: u8 = 0x60;
const V3_CHECKSUM_INDEX: usize = 62;
const V3_CLASS_INIT: u8 = 0x02;
const V3_CLASS_LINK: u8 = 0x20;
const V3_CLASS_BATTERY: u8 = 0x21;
const V3_CLASS_CHARGING: u8 = 0x2A;
const V3_KIND_REPLY: u8 = 0x01;
const V3_KIND_PUSH: u8 = 0x02;
const V3_REFRESH: Duration = Duration::from_secs(300);

fn v3_checksum(frame: &[u8]) -> u8 {
    xor_bytes(&frame[..V3_CHECKSUM_INDEX])
}

fn v3_make_frame(class: u8, direction: u8) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[0] = V3_REPORT_ID;
    b[2] = V3_TRANSACTION;
    b[6] = 4;
    b[9] = direction;
    b[10] = class;
    b[V3_CHECKSUM_INDEX] = v3_checksum(&b);
    b
}

fn v3_parse_frame(data: &[u8]) -> Option<(u8, u8)> {
    if data.len() <= V3_CHECKSUM_INDEX || data[0] != V3_REPORT_ID {
        return None;
    }
    if data[V3_CHECKSUM_INDEX] != v3_checksum(data) {
        return None;
    }
    if !matches!(data[11], V3_KIND_REPLY | V3_KIND_PUSH) || data[12] < 1 {
        return None;
    }
    Some((data[10], data[13]))
}

#[derive(Clone, Copy)]
struct V3State {
    level: Option<u8>,
    charging: bool,
    woken: bool,
    link_up: bool,
    next_query: Instant,
}

impl Default for V3State {
    fn default() -> Self {
        Self {
            level: None,
            charging: false,
            woken: false,
            link_up: false,
            next_query: Instant::now(),
        }
    }
}

fn v3_handle(state: &mut V3State, data: &[u8]) {
    let Some((class, value)) = v3_parse_frame(data) else {
        return;
    };
    if class == V3_CLASS_BATTERY && value <= 100 {
        state.level = Some(value);
        state.next_query = Instant::now() + V3_REFRESH;
    } else if class == V3_CLASS_CHARGING && value <= 1 {
        state.charging = value == 1;
    } else if class == V3_CLASS_LINK && value == 1 {
        state.level = None;
        state.charging = false;
        state.woken = false;
        state.link_up = true;
        state.next_query = Instant::now();
    }
}

fn v3_listen(handle: &mut Handle, state: &mut V3State, secs: Duration) {
    let end = Instant::now() + secs;
    while Instant::now() < end {
        if let Some(data) = handle.read(Instant::now() + Duration::from_millis(50)) {
            v3_handle(state, &data);
            if state.level.is_some() {
                return;
            }
        }
    }
}

fn v3_ask(handle: &mut Handle, state: &mut V3State) {
    if state.link_up {
        state.link_up = false;
        std::thread::sleep(Duration::from_millis(600));
    }
    state.level = None;
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(700));
        }
        if !state.woken {
            handle.write(&v3_make_frame(V3_CLASS_INIT, 0x00), Instant::now() + IO_TIMEOUT);
            v3_listen(handle, state, Duration::from_millis(50));
            handle.write(&v3_make_frame(V3_CLASS_CHARGING, 0x00), Instant::now() + IO_TIMEOUT);
            v3_listen(handle, state, Duration::from_millis(50));
        }
        handle.write(&v3_make_frame(V3_CLASS_CHARGING, 0x80), Instant::now() + IO_TIMEOUT);
        v3_listen(handle, state, Duration::from_millis(50));
        handle.write(&v3_make_frame(V3_CLASS_BATTERY, 0x80), Instant::now() + IO_TIMEOUT);
        v3_listen(handle, state, Duration::from_millis(300));
        if state.level.is_some() {
            state.woken = true;
            return;
        }
    }
    state.woken = false;
    state.next_query = Instant::now() + V3_REFRESH;
}

pub struct BlacksharkV3 {
    // 只保存纯状态；Python 把 collection 长期打开以缓存 push，Rust 的 Handle
    // 非 Send，因此每次 poll 重新打开，保留查询节流与缓存语义。
    state: Mutex<HashMap<CollectionKey, V3State>>,
}

impl BlacksharkV3 {
    pub fn new() -> Self {
        Self { state: Mutex::new(HashMap::new()) }
    }
}

impl Default for BlacksharkV3 {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for BlacksharkV3 {
    fn name(&self) -> &'static str {
        "blackshark_v3"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID || !matches!(pid, 0x057A | 0x0577) {
            return Vec::new();
        }
        let Some(c) = collections.iter().find(|c| c.usage_page == 0xFF14 && c.usage == 0x0001) else {
            return Vec::new();
        };
        let key = collection_key(c);
        let mut states = self.state.lock().unwrap_or_else(|e| e.into_inner());
        states.entry(key).or_insert_with(V3State::default);
        let Some(mut handle) = c.open() else {
            return Vec::new();
        };
        let state = states.get_mut(&key).expect("inserted above");
        for _ in 0..128 {
            let Some(data) = handle.read(Instant::now() + Duration::from_millis(1)) else {
                break;
            };
            v3_handle(state, &data);
        }
        if Instant::now() >= state.next_query {
            v3_ask(&mut handle, state);
        }
        let Some(level) = state.level else {
            return Vec::new();
        };
        let name = if pid == 0x0577 {
            "Razer BlackShark V3 Pro"
        } else {
            "Razer BlackShark V3"
        };
        let serial = instance.rsplit('\\').next().unwrap_or(instance);
        let mut status = Status::new(format!("razer:{pid:04x}:{serial}"), name);
        status.level = Some(level);
        status.charging = Some(state.charging);
        status.online = true;
        status.source = "razer".to_string();
        vec![status]
    }
}

// -------------------------------------------------------------- blackshark_v3x
const V3X_REPORT_ID: u8 = 0x07;
const V3X_TRANSACTION: u8 = 0x03;
const V3X_CLASS: u8 = 0x07;
const V3X_CMD_BATTERY: u8 = 0x80;
const V3X_CMD_CHARGING: u8 = 0x84;
const V3X_CRC_FIRST: usize = 3;
const V3X_CRC_LAST: usize = 60;
const V3X_CRC_INDEX: usize = 62;

fn v3x_checksum(frame: &[u8]) -> u8 {
    xor_bytes(&frame[V3X_CRC_FIRST..=V3X_CRC_LAST])
}

fn v3x_make_query(command: u8) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[0] = V3X_REPORT_ID;
    b[2] = V3X_TRANSACTION;
    b[6] = 0x02;
    b[7] = V3X_CLASS;
    b[8] = command;
    b[9] = 0x00;
    b[10] = 0x00;
    b[V3X_CRC_INDEX] = v3x_checksum(&b);
    b
}

fn v3x_parse_reply(data: &[u8], command: u8) -> Option<u8> {
    if data.len() < 64 || data[0] != V3X_REPORT_ID {
        return None;
    }
    if data[2] != V3X_TRANSACTION {
        return None;
    }
    let minimum = if command == V3X_CMD_BATTERY { 2 } else { 1 };
    if data[6] < minimum || data[6] > 52 {
        return None;
    }
    if data[7] != V3X_CLASS || data[8] != command {
        return None;
    }
    if data[V3X_CRC_INDEX] != v3x_checksum(data) {
        return None;
    }
    if data[1] != 0x02 {
        return None;
    }
    let value = data[10];
    if command == V3X_CMD_CHARGING {
        return if value <= 1 { Some(value) } else { None };
    }
    if value <= 100 {
        Some(value)
    } else {
        None
    }
}

fn v3x_query(handle: &mut Handle, command: u8) -> Option<u8> {
    let request = v3x_make_query(command);
    if !handle.set_feature(&request, Instant::now() + IO_TIMEOUT) {
        return None;
    }
    for _ in 0..4 {
        std::thread::sleep(Duration::from_millis(50));
        if let Some(reply) = handle.get_feature(V3X_REPORT_ID, Instant::now() + IO_TIMEOUT) {
            if let Some(value) = v3x_parse_reply(&reply, command) {
                return Some(value);
            }
        }
    }
    None
}

pub struct BlacksharkV3X {
    last: Mutex<Option<(u8, Instant)>>,
}

impl BlacksharkV3X {
    pub fn new() -> Self {
        Self { last: Mutex::new(None) }
    }
}

impl Default for BlacksharkV3X {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for BlacksharkV3X {
    fn name(&self) -> &'static str {
        "blackshark_v3x"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID || !matches!(pid, 0x057C | 0x057D) {
            return Vec::new();
        }
        let Some(c) = collections.iter().find(|c| {
            c.interface == Some(0) && c.usage_page == 0xFF82 && c.usage == 0x0001
        }) else {
            return Vec::new();
        };
        let key = format!("blackshark_v3x:{pid:04x}");
        let name = if pid == 0x057C {
            "Razer BlackShark V3 X HyperSpeed (USB cable)"
        } else {
            "Razer BlackShark V3 X HyperSpeed"
        };
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut handle) = c.open() else {
            return v3x_fallback(&mut last, &key, name);
        };
        let Some(level) = v3x_query(&mut handle, V3X_CMD_BATTERY) else {
            return v3x_fallback(&mut last, &key, name);
        };
        let charging = v3x_query(&mut handle, V3X_CMD_CHARGING);
        *last = Some((level, Instant::now()));
        let mut status = Status::new(key, name);
        status.kind = "headset".to_string();
        status.level = Some(level);
        status.charging = Some(charging.unwrap_or(0) != 0);
        status.charging_known = charging.is_some();
        status.online = true;
        status.source = "blackshark_v3x".to_string();
        vec![status]
    }
}

fn v3x_fallback(last: &mut Option<(u8, Instant)>, key: &str, name: &str) -> Vec<Status> {
    let mut status = Status::new(key, name);
    status.kind = "headset".to_string();
    status.source = "blackshark_v3x".to_string();
    status.charging_known = false;
    if let Some((level, at)) = *last {
        if Instant::now().saturating_duration_since(at) < Duration::from_secs(300) {
            status.level = Some(level);
            status.charging = Some(false);
            return vec![status];
        }
    }
    status.online = false;
    vec![status]
}

// --------------------------------------- blackshark_mtk / kraken_v4 (MediaTek)
const MTK_REPORT_ID: u8 = 0x02;
const MTK_SEQ_BASE: u8 = 0x60;
const MTK_CMD_BATTERY: u8 = 0x21;
const MTK_CMD_CHARGING: u8 = 0x2A;

fn mtk_make_frame(seq: u8, domain: u8, cmd: u8) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[0] = MTK_REPORT_ID;
    b[2] = MTK_SEQ_BASE | (seq & 0x1F);
    b[6] = 0x04;
    b[9] = domain;
    b[10] = cmd;
    b[62] = xor_bytes(&b[..62]);
    b
}

/// 驱动接受含或不含 report id 两种校验和；payload 已去掉 report id。
fn mtk_reply_checksum_ok(payload: &[u8]) -> bool {
    if payload.len() < 62 {
        return false;
    }
    let crc = xor_bytes(&payload[..61]);
    payload[61] == crc || payload[61] == (crc ^ MTK_REPORT_ID)
}

fn mtk_parse_reply(frame: &[u8], seq: u8, domain: u8, cmd: u8) -> Option<u8> {
    let payload = if frame.first() == Some(&MTK_REPORT_ID) {
        &frame[1..]
    } else {
        frame
    };
    if payload.len() < 13 {
        return None;
    }
    if payload[1] & 0x1F != seq & 0x1F {
        return None;
    }
    if payload[7] != 0x80 {
        return None;
    }
    if payload[8] != domain && !(domain == 0 && payload[8] == 0x80) {
        return None;
    }
    if payload[9] != cmd || payload[10] != 0x01 {
        return None;
    }
    if !(1..=48).contains(&payload[11]) {
        return None;
    }
    if !mtk_reply_checksum_ok(payload) {
        return None;
    }
    Some(payload[12])
}

fn mtk_query(handle: &mut Handle, seq: &mut u8, domain: u8, cmd: u8) -> Option<u8> {
    let s = *seq & 0x1F;
    *seq = (*seq + 1) & 0x1F;
    if !handle.write(&mtk_make_frame(s, domain, cmd), Instant::now() + IO_TIMEOUT) {
        return None;
    }
    for _ in 0..6 {
        if let Some(reply) = handle.read(Instant::now() + Duration::from_millis(200)) {
            if let Some(value) = mtk_parse_reply(&reply, s, domain, cmd) {
                return Some(value);
            }
        }
    }
    None
}

fn mtk_collection<'a>(collections: &'a [Collection]) -> Option<&'a Collection> {
    collections
        .iter()
        .find(|c| c.usage_page == 0xFF14 && c.usage == 0x0001)
}

pub struct BlacksharkMtk {
    state: Mutex<(u8, Option<(u8, bool, Instant)>)>,
}

impl BlacksharkMtk {
    pub fn new() -> Self {
        Self { state: Mutex::new((0, None)) }
    }
}

impl Default for BlacksharkMtk {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for BlacksharkMtk {
    fn name(&self) -> &'static str {
        "blackshark_mtk"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID || !matches!(pid, 0x0565 | 0x056E) {
            return Vec::new();
        }
        let Some(c) = mtk_collection(collections) else {
            return Vec::new();
        };
        let domain = if pid == 0x0565 { 0x80 } else { 0x00 };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut handle) = c.open() else {
            return mtk_fallback(&mut state.1, "blackshark_hs", "Razer BlackShark V2 HyperSpeed", "blackshark_mtk");
        };
        let level = mtk_query(&mut handle, &mut state.0, domain, MTK_CMD_BATTERY);
        let got = match level {
            Some(level) if level <= 100 => {
                let charging = mtk_query(&mut handle, &mut state.0, domain, MTK_CMD_CHARGING);
                match charging {
                    Some(value) => Some((level, value > 0, true)),
                    None => Some((level, false, false)),
                }
            }
            _ => None,
        };
        if let Some((level, charging, known)) = got {
            state.1 = Some((level, charging, Instant::now()));
            let mut status = Status::new("blackshark_hs", "Razer BlackShark V2 HyperSpeed");
            status.kind = "headset".to_string();
            status.level = Some(level);
            status.charging = Some(charging);
            status.charging_known = known;
            status.online = true;
            status.source = "blackshark_mtk".to_string();
            return vec![status];
        }
        mtk_fallback(&mut state.1, "blackshark_hs", "Razer BlackShark V2 HyperSpeed", "blackshark_mtk")
    }
}

fn mtk_fallback(
    last: &mut Option<(u8, bool, Instant)>,
    key: &str,
    name: &str,
    source: &str,
) -> Vec<Status> {
    if let Some((level, _, at)) = *last {
        if Instant::now().saturating_duration_since(at) < Duration::from_secs(180) {
            let mut status = Status::new(key, name);
            status.kind = "headset".to_string();
            status.level = Some(level);
            status.charging = Some(false);
            status.charging_known = false;
            status.online = false;
            status.source = source.to_string();
            return vec![status];
        }
    }
    Vec::new()
}

pub struct KrakenV4 {
    state: Mutex<(u8, Option<(u8, Option<bool>, Instant)>)>,
}

impl KrakenV4 {
    pub fn new() -> Self {
        Self { state: Mutex::new((0, None)) }
    }
}

impl Default for KrakenV4 {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for KrakenV4 {
    fn name(&self) -> &'static str {
        "kraken_v4"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID || !matches!(pid, 0x056B | 0x056C) {
            return Vec::new();
        }
        let Some(c) = mtk_collection(collections) else {
            return Vec::new();
        };
        let domain = if pid == 0x056C { 0x80 } else { 0x00 };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut handle) = c.open() else {
            return kraken_fallback(&mut state.1);
        };
        let level = mtk_query(&mut handle, &mut state.0, domain, MTK_CMD_BATTERY);
        let got = match level {
            Some(level) if level <= 100 => {
                std::thread::sleep(Duration::from_millis(20));
                let charging = mtk_query(&mut handle, &mut state.0, domain, MTK_CMD_CHARGING);
                match charging {
                    Some(v @ 0) | Some(v @ 1) => Some((level, Some(v == 1))),
                    _ => Some((level, None)),
                }
            }
            _ => None,
        };
        if let Some((level, charging)) = got {
            state.1 = Some((level, charging, Instant::now()));
            let mut status = Status::new("kraken_v4", "Razer Kraken V4");
            status.kind = "headset".to_string();
            status.level = Some(level);
            status.charging = Some(charging.unwrap_or(false));
            status.charging_known = charging.is_some();
            status.online = true;
            status.source = "kraken_v4".to_string();
            return vec![status];
        }
        kraken_fallback(&mut state.1)
    }
}

fn kraken_fallback(last: &mut Option<(u8, Option<bool>, Instant)>) -> Vec<Status> {
    if let Some((level, _, at)) = *last {
        if Instant::now().saturating_duration_since(at) < Duration::from_secs(180) {
            let mut status = Status::new("kraken_v4", "Razer Kraken V4");
            status.kind = "headset".to_string();
            status.level = Some(level);
            status.charging = Some(false);
            status.charging_known = false;
            status.online = false;
            status.source = "kraken_v4".to_string();
            return vec![status];
        }
    }
    Vec::new()
}

// ------------------------------------------------------------------- barracuda
const BCD_REPORT_ID: u8 = 0x01;
const BCD_TYPE_REMOTE: u8 = 0x02;
const BCD_TYPE_QUERY: u8 = 0x03;
const BCD_CMD_REMOTE: u8 = 0xE1;
const BCD_CMD_BATTERY: u8 = 0x21;
const BCD_CMD_CHARGING: u8 = 0x2A;
const BCD_REPLY_CMD_OFF: usize = 13;

fn bcd_frame_query(cmd_id: u8, cmd_type: u8) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[0] = BCD_REPORT_ID;
    b[1] = 0x80;
    b[2] = 8;
    b[3] = 0x50; // 'P'
    b[4] = 0x41; // 'A'
    b[5] = 0x08;
    b[6] = 0x08;
    b[7] = cmd_type;
    b[8] = cmd_id;
    b
}

fn bcd_frame_remote(on: bool) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[0] = BCD_REPORT_ID;
    b[1] = 0x80;
    b[2] = 0x07;
    b[3] = 0x50;
    b[4] = 0x41;
    b[5] = 0x0E;
    b[6] = 0x08;
    b[7] = BCD_TYPE_REMOTE;
    b[8] = BCD_CMD_REMOTE;
    b[9] = u8::from(on);
    b
}

#[allow(dead_code)] // Python 用它区分「别的报告」与「无法识别」，Rust 只在测试中校验
fn bcd_is_reply(data: &[u8]) -> bool {
    for off in [0i32, -1] {
        if off == -1 && data.first() == Some(&BCD_REPORT_ID) {
            continue;
        }
        let m = (3 + off) as usize;
        if data.len() > m + 1 && data[m] == 0x50 && data[m + 1] == 0x49 {
            return true;
        }
    }
    false
}

fn bcd_parse_reply(data: &[u8], cmd_id: u8) -> Option<Vec<u8>> {
    for off in [0i32, -1] {
        if off == -1 && data.first() == Some(&BCD_REPORT_ID) {
            continue;
        }
        let c = (BCD_REPLY_CMD_OFF as i32 + off) as usize;
        if data.len() > c + 3 && data[c] == cmd_id && matches!(data[c + 1], 0x01 | 0x02) {
            let n = data[c + 2] as usize;
            if n > 0 && n <= 32 {
                let end = (c + 3 + n).min(data.len());
                return Some(data[c + 3..end].to_vec());
            }
        }
    }
    None
}

fn bcd_parse_level(payload: &[u8]) -> Option<u8> {
    payload.first().copied().filter(|v| *v <= 100)
}

fn bcd_drain(handle: &mut Handle) {
    for _ in 0..8 {
        if handle.read(Instant::now() + Duration::from_millis(20)).is_none() {
            return;
        }
    }
}

fn bcd_query(handle: &mut Handle, cmd_id: u8) -> Option<Vec<u8>> {
    let mut result = None;
    for _ in 0..4 {
        bcd_drain(handle);
        let query = bcd_frame_query(cmd_id, BCD_TYPE_QUERY);
        if !handle.write(&query, Instant::now() + IO_TIMEOUT) {
            return None;
        }
        for _ in 0..6 {
            if let Some(data) = handle.read(Instant::now() + Duration::from_millis(150)) {
                if let Some(payload) = bcd_parse_reply(&data, cmd_id) {
                    result = Some(payload);
                    break;
                }
            }
        }
        if result.is_some() {
            break;
        }
    }
    result
}

pub struct Barracuda;

impl Provider for Barracuda {
    fn name(&self) -> &'static str {
        "barracuda"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID || pid != 0x053A {
            return Vec::new();
        }
        let Some(c) = collections.iter().find(|c| c.usage_page == 0xFF00) else {
            return Vec::new();
        };
        let serial = instance.rsplit('\\').next().unwrap_or(instance);
        let mut status = Status::new(
            format!("barracuda:{pid:04x}:{serial}"),
            "Razer Barracuda Pro (2.4 GHz)",
        );
        status.kind = "headset".to_string();
        status.source = "barracuda".to_string();
        let Some(mut handle) = c.open() else {
            return vec![status];
        };
        let mut woke = false;
        for attempt in 0..3 {
            bcd_drain(&mut handle);
            if handle.write(&bcd_frame_remote(true), Instant::now() + IO_TIMEOUT) {
                woke = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(150 * (attempt + 1)));
        }
        if !woke {
            return vec![status];
        }
        std::thread::sleep(Duration::from_millis(50));
        let Some(battery) = bcd_query(&mut handle, BCD_CMD_BATTERY) else {
            handle.write(&bcd_frame_remote(false), Instant::now() + IO_TIMEOUT);
            return vec![status];
        };
        let charging = bcd_query(&mut handle, BCD_CMD_CHARGING);
        handle.write(&bcd_frame_remote(false), Instant::now() + IO_TIMEOUT);
        let Some(level) = bcd_parse_level(&battery) else {
            return vec![status];
        };
        status.level = Some(level);
        status.charging = Some(charging.and_then(|v| v.first().copied()).unwrap_or(0) != 0);
        status.online = true;
        vec![status]
    }
}

// ---------------------------------------------------------------- barracuda_x
const BX_REPORT_ID: u8 = 0x01;
const BX_HEADER_LEN: usize = 10;
const BX_GROUP_REMOTE: u8 = 0x0E;
const BX_GROUP_BATTERY: u8 = 0x06;
const BX_CURVE: [u16; 11] = [3300, 3680, 3750, 3790, 3830, 3870, 3910, 3960, 4020, 4080, 4150];

fn bx_race_request(group: u8, seq: u8, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0x50, 0x41, group, seq];
    v.extend_from_slice(body);
    v
}

fn bx_wrap(report_id: u8, race: &[u8]) -> Vec<u8> {
    let mut v = vec![report_id, 0x80, race.len() as u8];
    v.extend_from_slice(race);
    v
}

fn bx_find_frame(report: &[u8], offset: usize, length: usize, group: u8, seq: u8) -> Option<Vec<u8>> {
    let end = (offset + length).min(report.len());
    let mut pos = offset;
    while pos + BX_HEADER_LEN <= end {
        if report[pos] != 0x50 || report[pos + 1] != 0x49 {
            return None;
        }
        let payload_len = report[pos + 8] as usize | ((report[pos + 9] as usize) << 8);
        let frame_end = pos + BX_HEADER_LEN + payload_len;
        if frame_end > end {
            return None;
        }
        let payload = report[pos + BX_HEADER_LEN..frame_end].to_vec();
        if payload.len() >= 2 && payload[0] == group && payload[1] == (0x80 | seq) {
            return Some(payload);
        }
        pos = frame_end;
    }
    None
}

fn bx_battery_mv(payload: &[u8]) -> Option<u16> {
    if payload.len() < 5 || payload[0] != BX_GROUP_BATTERY {
        return None;
    }
    if payload[2] != 0x00 {
        return None;
    }
    let mv = payload[3] as u16 | ((payload[4] as u16) << 8);
    if (2500..=4500).contains(&mv) {
        Some(mv)
    } else {
        None
    }
}

fn bx_to_percent(mv: u16) -> u8 {
    if mv <= BX_CURVE[0] {
        return 0;
    }
    for i in 1..BX_CURVE.len() {
        if mv < BX_CURVE[i] {
            let span = u32::from(BX_CURVE[i] - BX_CURVE[i - 1]);
            let rise = u32::from(mv - BX_CURVE[i - 1]) * 10 / span;
            return (i - 1) as u8 * 10 + rise as u8;
        }
    }
    100
}

fn bx_is_charging(samples: &[u16]) -> bool {
    if samples.is_empty() {
        return false;
    }
    let min = samples.iter().min().expect("non-empty");
    let max = samples.iter().max().expect("non-empty");
    (*max - *min) >= 12 || *min >= 4180
}

fn bx_enter_app_mode(handle: &mut Handle) -> bool {
    if !handle.write(&[BX_REPORT_ID, 0x40], Instant::now() + IO_TIMEOUT) {
        return false;
    }
    let end = Instant::now() + Duration::from_millis(800);
    while Instant::now() < end {
        let Some(r) = handle.read(Instant::now() + Duration::from_millis(200)) else {
            continue;
        };
        if r.first() == Some(&BX_REPORT_ID) && r.get(1) == Some(&0x40) {
            return r.len() >= 4 && r[2] == 0x01 && r[3] == 0x01;
        }
    }
    false
}

fn bx_race(handle: &mut Handle, seq: u8, group: u8, body: &[u8]) -> Option<Vec<u8>> {
    let frame = bx_wrap(BX_REPORT_ID, &bx_race_request(group, seq, body));
    if !handle.write(&frame, Instant::now() + IO_TIMEOUT) {
        return None;
    }
    let end = Instant::now() + Duration::from_millis(800);
    while Instant::now() < end {
        let Some(r) = handle.read(Instant::now() + Duration::from_millis(200)) else {
            continue;
        };
        let payload = if r.first() == Some(&BX_REPORT_ID) {
            if r.len() < 3 || r[1] != 0x80 {
                continue;
            }
            bx_find_frame(&r, 3, r[2] as usize, group, seq)
        } else {
            bx_find_frame(&r, 0, r.len(), group, seq)
        };
        if payload.is_some() {
            return payload;
        }
    }
    None
}

struct BxState {
    seq: u8,
    last: Option<(u8, Instant)>,
}

fn bx_next_seq(state: &mut BxState) -> u8 {
    state.seq = state.seq % 127 + 1;
    state.seq
}

fn bx_ask(handle: &mut Handle, state: &mut BxState) -> Option<(u8, bool)> {
    if !bx_enter_app_mode(handle) {
        return None;
    }
    let remote = bx_race(handle, bx_next_seq(state), BX_GROUP_REMOTE, &[0x02, 0xE1, 0x01]);
    if !remote.as_ref().map_or(false, |v| v.len() >= 3 && v[2] == 0x00) {
        let _ = bx_race(handle, bx_next_seq(state), BX_GROUP_REMOTE, &[0x02, 0xE1, 0x00]);
        return None;
    }
    let mut samples = Vec::new();
    for i in 0..7 {
        if i > 0 {
            std::thread::sleep(Duration::from_millis(350));
        }
        let reply = bx_race(handle, bx_next_seq(state), BX_GROUP_BATTERY, &[0x01, 0x00, 0x31]);
        if let Some(mv) = reply.as_deref().and_then(bx_battery_mv) {
            samples.push(mv);
        }
    }
    let _ = bx_race(handle, bx_next_seq(state), BX_GROUP_REMOTE, &[0x02, 0xE1, 0x00]);
    if samples.is_empty() {
        return None;
    }
    let mv = *samples.iter().min().expect("non-empty");
    Some((bx_to_percent(mv), bx_is_charging(&samples)))
}

pub struct BarracudaX {
    state: Mutex<BxState>,
}

impl BarracudaX {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(BxState { seq: 0, last: None }),
        }
    }
}

impl Default for BarracudaX {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for BarracudaX {
    fn name(&self) -> &'static str {
        "barracuda_x"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != VID || pid != 0x0552 {
            return Vec::new();
        }
        let Some(c) = collections.iter().find(|c| {
            c.usage_page == 0xFF00 && c.input_len > 0 && c.output_len > 0
        }) else {
            return Vec::new();
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut status = Status::new("barracuda-x:0552", "Razer Barracuda X (2022)");
        status.kind = "headset".to_string();
        status.source = "barracuda_x".to_string();
        let Some(mut handle) = c.open() else {
            return bx_fallback(&mut state.last, status);
        };
        let Some((level, charging)) = bx_ask(&mut handle, &mut state) else {
            return bx_fallback(&mut state.last, status);
        };
        state.last = Some((level, Instant::now()));
        status.level = Some(level);
        status.charging = Some(charging);
        status.online = true;
        vec![status]
    }
}

fn bx_fallback(last: &mut Option<(u8, Instant)>, mut status: Status) -> Vec<Status> {
    if let Some((level, at)) = *last {
        if Instant::now().saturating_duration_since(at) < Duration::from_secs(300) {
            status.level = Some(level);
            status.charging = Some(false);
            return vec![status];
        }
    }
    status.level = None;
    status.charging = Some(false);
    status.online = false;
    vec![status]
}

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(Razer::new()),
        Box::new(Blackshark2020),
        Box::new(BlacksharkPa::new()),
        Box::new(BlacksharkV3::new()),
        Box::new(BlacksharkV3X::new()),
        Box::new(BlacksharkMtk::new()),
        Box::new(KrakenV4::new()),
        Box::new(Barracuda),
        Box::new(BarracudaX::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_layout_and_crc() {
        let req = build_request(0x1F, 0x07, 0x80, 0x02);
        assert_eq!(req.len(), 90);
        assert_eq!(req[0], 0x00);
        assert_eq!(req[1], 0x1F);
        assert_eq!(req[5], 0x02);
        assert_eq!(req[6], 0x07);
        assert_eq!(req[7], 0x80);
        // Python 的 msg[2:88] 不含 msg[1]（transaction id）
        assert_eq!(req[88], 0x02 ^ 0x07 ^ 0x80);
        assert_eq!(req[89], 0x00);
    }

    #[test]
    fn normalize_reply_strips_report_id() {
        assert_eq!(normalize_reply(&[]), None);
        let short = vec![0u8; 89];
        assert_eq!(normalize_reply(&short), None);
        let mut with_id = vec![0u8; 91];
        with_id[1] = 0x02;
        assert_eq!(normalize_reply(&with_id), Some(&with_id[1..91]));
        let plain = vec![5u8; 90];
        assert_eq!(normalize_reply(&plain), Some(plain.as_slice()));
    }

    #[test]
    fn parse_reply_matches_command() {
        let mut data = vec![0u8; 90];
        data[0] = STATUS_OK as u8;
        data[6] = 0x07;
        data[7] = 0x80;
        data[9] = 200;
        assert_eq!(parse_reply(&data, 0x07, 0x80), (STATUS_OK, Some(200)));
        // 命令不符：即使 status=02 也算别的软件的包
        assert_eq!(parse_reply(&data, 0x07, 0x84), (STATUS_OK, None));
        data[0] = STATUS_BUSY as u8;
        assert_eq!(parse_reply(&data, 0x07, 0x80), (STATUS_BUSY, Some(200)));
        data[0] = STATUS_NOT_SUPPORTED as u8;
        assert_eq!(parse_reply(&data, 0x07, 0x80), (STATUS_NOT_SUPPORTED, Some(200)));
    }

    #[test]
    fn level_rounding() {
        assert_eq!(to_percent(0), 0);
        assert_eq!(to_percent(255), 100);
        assert_eq!(to_percent(128), 50);
        assert_eq!(to_percent(51), 20);
        assert_eq!(to_percent(13), 5);
        assert_eq!(to_percent(14), 5);
    }

    #[test]
    fn known_table_and_preferred_tids() {
        assert_eq!(known(0x0528), Some(("Razer BlackShark V2 Pro (2020)", 0x3F)));
        assert_eq!(known(0x00A5), Some(("Razer Viper V2 Pro", 0x1F)));
        assert_eq!(known(0x007A), Some(("Razer Viper Ultimate", 0xFF)));
        assert_eq!(known(0x027B), Some(("Razer Pro Type Ultra", 0x9F)));
        assert_eq!(known(0x9999), None);
        assert_eq!(keyboard_interface(0x0290), Some(2));
        assert_eq!(keyboard_interface(0x0292), Some(3));
        assert_eq!(keyboard_interface(0x00A5), None);
    }

    #[test]
    fn dedicated_pids_released_from_skip() {
        // 8 个专用协议已放行，不再走通用 90 字节帧。
        for pid in [0x0528, 0x057A, 0x0577, 0x0555, 0x0556, 0x0583,
                    0x0565, 0x056E, 0x057C, 0x057D, 0x053A, 0x0552, 0x056B, 0x056C] {
            assert!(!SKIP_PIDS.contains(&pid), "pid {pid:04x} must be released");
            assert!(DEDICATED_PIDS.contains(&pid), "pid {pid:04x} must be dedicated");
        }
        // 052E / 0536 / 0550 也从 SKIP_PIDS 放行，但不在 KNOWN 中：
        // 通用帧因无 product string 仍不会碰它们，也没有专用 provider 认领。
        for pid in [0x052E, 0x0536, 0x0550] {
            assert!(!SKIP_PIDS.contains(&pid));
            assert!(known(pid).is_none());
        }
        // 0557 有通用帧兜底，既不跳过也不专用化。
        assert!(!SKIP_PIDS.contains(&0x0557));
        assert!(!DEDICATED_PIDS.contains(&0x0557));
        assert!(KNOWN.iter().any(|(p, _, _)| *p == 0x0557));
    }

    #[test]
    fn transaction_ids_are_unique_with_preference_first() {
        let mut tids = vec![0xFF];
        for tid in TRANSACTION_IDS {
            if !tids.contains(tid) {
                tids.push(*tid);
            }
        }
        assert_eq!(tids, vec![0xFF, 0x1F, 0x3F, 0x9F, 0x08]);
    }

    #[test]
    fn vendor_filter_rejects_other_vids() {
        let razer = Razer::new();
        assert_eq!(razer.name(), "razer");
        assert!(razer.poll(r"USB\VID_054C&PID_0CE6\7&1A2B3C&0&0", &[]).is_empty());
    }

    // ------------------------------------------------------------ blackshark2020
    #[test]
    fn bs2020_request_and_captured_replies() {
        let req = bs2020_request();
        assert_eq!(req.len(), 64);
        assert_eq!(&req[..9], &[0xff, 0x0a, 0x00, 0xfd, 0x04, 0x12, 0xf1, 0x02, 0x05]);
        assert!(req[9..].iter().all(|&b| b == 0));

        // Python 源码头部记录的实际读取（on battery）。
        let mut reply = [0u8; 64];
        reply[..15].copy_from_slice(&[
            0xff, 0x0f, 0x05, 0xfe, 0x12, 0x04, 0x1f, 0x08, 0x05, 0x03, 0x05, 0x01, 0x0e, 0xc0, 0x50,
        ]);
        assert_eq!(bs2020_parse_reply(&reply), Some((0x01, 0x0ec0, 0x50)));

        // charging: state 0x09，raw14=0x50；上层会把 level 隐藏。
        let mut charging = [0u8; 64];
        charging[..15].copy_from_slice(&[
            0xff, 0x0f, 0x05, 0xfe, 0x12, 0x04, 0x1f, 0x08, 0x05, 0x05, 0x03, 0x09, 0x10, 0x80, 0x50,
        ]);
        assert_eq!(bs2020_parse_reply(&charging), Some((0x09, 0x1080, 0x50)));

        // 无新回复时的旧帧不能当成有效电量。
        let mut stale = [0u8; 64];
        stale[..9].copy_from_slice(BS2020_NO_REPLY_PREFIX);
        assert_eq!(bs2020_parse_reply(&stale), None);
        assert_eq!(bs2020_parse_reply(&reply[..63]), None);
    }

    // --------------------------------------------------------------- blackshark PA
    #[test]
    fn pa_frames_and_reply() {
        let q = pa_frame_query(0x21, 0x03);
        assert_eq!(q.len(), 64);
        assert_eq!(q[0], 0x02);
        assert_eq!(q[1], 0x80);
        assert_eq!(q[2], 8);
        assert_eq!(&q[5..7], &[0x50, 0x41]);
        assert_eq!(q[7], 0x08);
        assert_eq!(q[9], 0x03);
        assert_eq!(q[10], 0x21);

        let r = pa_frame_remote(true);
        assert_eq!(r[2], 0x07);
        assert_eq!(r[5], 0x50);
        assert_eq!(r[6], 0x41);
        assert_eq!(r[7], 0x0E);
        assert_eq!(r[9], 0x02);
        assert_eq!(r[10], 0xE1);
        assert_eq!(r[11], 1);
        assert_eq!(pa_frame_remote(false)[11], 0);

        // Python 文档规定的 ACK 布局（带 report id）。
        let mut ack = [0u8; 64];
        ack[0] = 0x02;
        ack[12] = 0x21;
        ack[13] = 0x01;
        ack[14] = 1;
        ack[15] = 90;
        assert_eq!(pa_parse_reply(&ack, 0x21), Some(vec![90]));
        assert_eq!(pa_parse_reply(&ack, 0x2A), None);

        // hidapi 不带 report id 的变体：整体左移一字节。
        let mut ack2 = [0u8; 64];
        ack2[11] = 0x21;
        ack2[12] = 0x01;
        ack2[13] = 1;
        ack2[14] = 55;
        assert_eq!(pa_parse_reply(&ack2, 0x21), Some(vec![55]));
    }

    // ---------------------------------------------------------------- blackshark_v3
    #[test]
    fn v3_frames_and_synapse_capture() {
        // Python 源码抓包：OUT 02 00 60 . . . 04 . . 00 02 00 00 00 [62]=64
        let wake = v3_make_frame(0x02, 0x00);
        assert_eq!(wake[0], 0x02);
        assert_eq!(wake[2], 0x60);
        assert_eq!(wake[6], 0x04);
        assert_eq!(wake[9], 0x00);
        assert_eq!(wake[10], 0x02);
        assert_eq!(wake[62], 0x64); // Python 抓包标注 [62]=64（十六进制）

        // OUT 02 00 60 . . . 04 . . 80 21 00 00 00 [62]=c7
        let battery_query = v3_make_frame(0x21, 0x80);
        assert_eq!(battery_query[9], 0x80);
        assert_eq!(battery_query[10], 0x21);
        assert_eq!(battery_query[62], 0xc7);

        // IN 02 02 60 . . . 05 . . 80 21 01 01 61 [62]=a5 -> 97%
        let mut reply = [0u8; 64];
        reply[0] = 0x02;
        reply[1] = 0x02;
        reply[2] = 0x60;
        reply[6] = 0x05;
        reply[9] = 0x80;
        reply[10] = 0x21;
        reply[11] = 0x01;
        reply[12] = 0x01;
        reply[13] = 0x61;
        reply[62] = v3_checksum(&reply);
        assert_eq!(reply[62], 0xa5);
        assert_eq!(v3_parse_frame(&reply), Some((0x21, 0x61)));

        let mut state = V3State::default();
        v3_handle(&mut state, &reply);
        assert_eq!(state.level, Some(0x61));
        assert!(!state.charging);
        assert!(state.woken == false);
        assert!(state.next_query > Instant::now());

        // link-up push: 02 02 60 ... 05 ... 80 20 01 01 01
        let mut link = [0u8; 64];
        link[0] = 0x02;
        link[1] = 0x02;
        link[2] = 0x60;
        link[6] = 0x05;
        link[9] = 0x80;
        link[10] = 0x20;
        link[11] = 0x02;
        link[12] = 0x01;
        link[13] = 0x01;
        link[62] = v3_checksum(&link);
        v3_handle(&mut state, &link);
        assert_eq!(state.level, None);
        assert!(state.link_up);

        reply[62] ^= 1;
        assert_eq!(v3_parse_frame(&reply), None);
    }

    // -------------------------------------------------------------- blackshark_v3x
    #[test]
    fn v3x_query_and_reply() {
        let q = v3x_make_query(0x80);
        assert_eq!(q[0], 0x07);
        assert_eq!(q[2], 0x03);
        assert_eq!(q[6], 0x02);
        assert_eq!(q[7], 0x07);
        assert_eq!(q[8], 0x80);
        assert_eq!(q[9], 0);
        assert_eq!(q[10], 0);
        assert_eq!(q[62], v3x_checksum(&q));

        // 按 Python 文档布局构造的有效回复（level=100）。
        let mut r = v3x_make_query(0x80);
        r[1] = 0x02;
        r[10] = 100;
        r[62] = v3x_checksum(&r);
        assert_eq!(v3x_parse_reply(&r, 0x80), Some(100));
        assert_eq!(v3x_parse_reply(&r, 0x84), None);

        r[1] = 0x01; // BUSY
        assert_eq!(v3x_parse_reply(&r, 0x80), None);
        r[1] = 0x02;
        r[10] = 101; // 越界
        r[62] = v3x_checksum(&r);
        assert_eq!(v3x_parse_reply(&r, 0x80), None);

        let mut chg = v3x_make_query(0x84);
        chg[1] = 0x02;
        chg[10] = 1;
        chg[62] = v3x_checksum(&chg);
        assert_eq!(v3x_parse_reply(&chg, 0x84), Some(1));
    }

    // ---------------------------------------- blackshark_mtk / kraken_v4 (MediaTek)
    #[test]
    fn mtk_frame_and_reply() {
        let f = mtk_make_frame(0, 0x80, 0x21);
        assert_eq!(f.len(), 64);
        assert_eq!(f[0], 0x02);
        assert_eq!(f[2], 0x60);
        assert_eq!(f[6], 0x04);
        assert_eq!(f[9], 0x80);
        assert_eq!(f[10], 0x21);
        assert_eq!(f[62], xor_bytes(&f[..62]));

        // 按 Python `parse_reply` 布局构造（hidapi 保留 report id）。
        let mut r = [0u8; 64];
        r[0] = 0x02;
        r[2] = 0x60;
        r[8] = 0x80; // 耳机地址
        r[9] = 0x80; // domain
        r[10] = 0x21;
        r[11] = 0x01; // ack
        r[12] = 1; // payload count
        r[13] = 87; // level
        r[62] = xor_bytes(&r[1..62]);
        assert_eq!(mtk_parse_reply(&r, 0, 0x80, 0x21), Some(87));
        assert_eq!(mtk_parse_reply(&r, 0, 0x80, 0x2A), None);
        let mut crc_bad = r;
        crc_bad[62] ^= 1;
        assert_eq!(mtk_parse_reply(&crc_bad, 0, 0x80, 0x21), None);
    }

    #[test]
    fn kraken_v4_cable_domain_reply() {
        let f = mtk_make_frame(3, 0x00, 0x21);
        assert_eq!(f[2], 0x63);
        assert_eq!(f[9], 0x00);

        // 线缆回复的 domain 槽位是耳机地址 0x80（Python 的 domain==0 特例）。
        let mut r = [0u8; 64];
        r[0] = 0x02;
        r[2] = 0x63;
        r[8] = 0x80;
        r[9] = 0x80;
        r[10] = 0x21;
        r[11] = 0x01;
        r[12] = 2;
        r[13] = 72;
        r[62] = xor_bytes(&r[1..62]);
        assert_eq!(mtk_parse_reply(&r, 3, 0x00, 0x21), Some(72));
        assert_eq!(mtk_parse_reply(&r, 3, 0x80, 0x21), Some(72));

        // 真实的本地 domain=00 回复不能回答接收器（domain=0x80）的查询。
        r[9] = 0x00;
        r[62] = xor_bytes(&r[1..62]);
        assert_eq!(mtk_parse_reply(&r, 3, 0x80, 0x21), None);
        assert_eq!(mtk_parse_reply(&r, 3, 0x00, 0x21), Some(72));

        // charging 值 1。
        r[10] = 0x2A;
        r[13] = 1;
        r[62] = xor_bytes(&r[1..62]);
        assert_eq!(mtk_parse_reply(&r, 3, 0x00, 0x2A), Some(1));
    }

    // ------------------------------------------------------------------- barracuda
    #[test]
    fn barracuda_frames_and_reply() {
        let q = bcd_frame_query(0x21, 0x03);
        assert_eq!(q[0], 0x01);
        assert_eq!(q[1], 0x80);
        assert_eq!(q[2], 8);
        assert_eq!(&q[3..5], &[0x50, 0x41]);
        assert_eq!(q[5], 0x08);
        assert_eq!(q[6], 0x08);
        assert_eq!(q[7], 0x03);
        assert_eq!(q[8], 0x21);

        let r = bcd_frame_remote(true);
        assert_eq!(r[0], 0x01);
        assert_eq!(r[2], 0x07);
        assert_eq!(r[5], 0x0E);
        assert_eq!(r[7], 0x02);
        assert_eq!(r[8], 0xE1);
        assert_eq!(r[9], 1);
        assert_eq!(bcd_frame_remote(false)[9], 0);

        // Python 文档：reply [13]=cmd [14]=ACK [15]=len [16..]=data。
        let mut ack = [0u8; 64];
        ack[0] = 0x01;
        ack[3] = 0x50;
        ack[4] = 0x49;
        ack[13] = 0x21;
        ack[14] = 0x01;
        ack[15] = 1;
        ack[16] = 76;
        assert!(bcd_is_reply(&ack));
        assert_eq!(bcd_parse_reply(&ack, 0x21), Some(vec![76]));
        assert_eq!(bcd_parse_reply(&ack, 0x2A), None);
        assert_eq!(bcd_parse_level(&[76]), Some(76));
        assert_eq!(bcd_parse_level(&[200]), None);
        assert_eq!(bcd_parse_level(&[]), None);
    }

    // ---------------------------------------------------------------- barracuda_x
    #[test]
    fn barracuda_x_race_and_curve() {
        let race = bx_race_request(0x06, 1, &[0x01, 0x00, 0x31]);
        assert_eq!(race, vec![0x50, 0x41, 0x06, 0x01, 0x01, 0x00, 0x31]);
        let frame = bx_wrap(0x01, &race);
        assert_eq!(frame[0], 0x01);
        assert_eq!(frame[1], 0x80);
        assert_eq!(frame[2], 7);
        assert_eq!(&frame[3..], &race[..]);

        // Python 文档的 RACE 回复：01 80 <len> 50 49 <10 字节头> <payload>。
        // payload = [group, 0x80|seq, status, mV_LE]，mV=3900=0x0f3c。
        let mut report = [0u8; 64];
        report[0] = 0x01;
        report[1] = 0x80;
        report[2] = 15; // len(race)
        report[3] = 0x50;
        report[4] = 0x49;
        report[11] = 0x05; // payload len LO
        report[12] = 0x00; // payload len HI
        report[13] = 0x06; // group
        report[14] = 0x81; // 0x80|seq
        report[15] = 0x00; // status
        report[16] = 0x3c; // mV LO
        report[17] = 0x0f; // mV HI
        let payload = bx_find_frame(&report, 3, 15, 0x06, 1).expect("matching RACE frame");
        assert_eq!(payload, vec![0x06, 0x81, 0x00, 0x3c, 0x0f]);
        assert_eq!(bx_battery_mv(&payload), Some(3900));
        assert_eq!(bx_battery_mv(&[0x06, 0x81, 0x01, 0x3c, 0x0f]), None);
        assert_eq!(bx_find_frame(&report, 3, 15, 0x0E, 1), None);

        assert_eq!(bx_to_percent(3300), 0);
        assert_eq!(bx_to_percent(3900), 57);
        assert_eq!(bx_to_percent(3975), 72);
        assert_eq!(bx_to_percent(5000), 100);
        assert!(bx_is_charging(&[3900, 3920, 3940]));
        assert!(!bx_is_charging(&[3900, 3901, 3902]));
        assert!(bx_is_charging(&[4200, 4200, 4200]));
    }

    #[test]
    fn all_specialized_providers_registered() {
        let providers = super::all_providers();
        let names: Vec<&str> = providers.iter().map(|p| p.name()).collect();
        for name in [
            "razer", "blackshark2020", "blackshark", "blackshark_v3", "blackshark_v3x",
            "blackshark_mtk", "kraken_v4", "barracuda", "barracuda_x",
        ] {
            assert!(names.contains(&name), "provider {name} must be registered");
        }
    }
}
