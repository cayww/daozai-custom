//! Razer 设备电量协议：90 字节 feature report（与 Synapse / OpenRazer 相同）。
//! 精确移植自 `upstream/providers/razer.py` 的通用 0x07:0x80 / 0x07:0x84 帧部分。
//!
//! 以下子协议依赖各自的会话/状态机或专用帧格式，不在本模块内移植（见 README 报告）：
//!   - BlackShark V2 Pro 2020 (0528/052E)
//!   - BlackShark V3 / V3 Pro (057A/0577) 的 ff14 会话
//!   - BlackShark V2 Pro 2023 的 "PA" output/input 协议 (0555/0556/0583)
//!   - MediaTek 接收器 (0565/056E)
//!   - BlackShark V3 X (057C/057D)、Barracuda X / Pro (0552/053A/0536/0550)
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

// 这些 PID 由 Python 的其它 provider / 专用状态机读取，或明确不发包；本模块跳过。
const SKIP_PIDS: &[u16] = &[
    0x0528, 0x052E,             // BlackShark V2 Pro 2020 + 充电线
    0x057A, 0x0577,             // BlackShark V3 / V3 Pro（ff14 会话）
    0x0555, 0x0556, 0x0583,     // BlackShark V2 Pro 2023 PA 协议
    0x0565, 0x056E,             // MediaTek 接收器 / 有线耳机
    0x057D, 0x057C,             // BlackShark V3 X
    0x0536, 0x0550, 0x0552, 0x053A, // Barracuda 系列
];

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
        if SKIP_PIDS.contains(&pid) {
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

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![Box::new(Razer::new())]
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
    fn skipped_sub_protocols() {
        for pid in [0x0528, 0x052E, 0x057A, 0x0577, 0x0555, 0x0556, 0x0583,
                    0x0565, 0x056E, 0x057D, 0x057C, 0x0536, 0x0550, 0x0552, 0x053A] {
            assert!(SKIP_PIDS.contains(&pid), "pid {pid:04x} must be skipped");
        }
        // 0557 有通用帧兜底，不跳过
        assert!(!SKIP_PIDS.contains(&0x0557));
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
}
