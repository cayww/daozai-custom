//! 复杂状态机协议批次 A：Turtle Beach / AJAZZ / Lofree / Audeze / DJI。
//!
//! 严格按 `vendor/halo-battery/upstream/providers/<name>.py` 移植：命令帧、PID 表、
//! 校验和、状态机与合法性判断逐条对应，不猜测字节格式。每个协议的单元测试使用
//! Python 源码注释中的实测报文/布局构造样例。
//!
//! 注意：这是静态移植，只能保证与 Python 的逻辑一致；读数正确性仍需实机确认。
//! 另外，Rust 的 `Collection` 不暴露 product string / serial / HID path，且
//! `Provider::poll` 以单个 USB instance 为单位调用，Python 侧按 product string、
//! serial 或跨实例分组的行为在这里只能用 VID/PID + collection 组合近似，
//! 并在各模块注释与最终交接说明中逐条标注。
use crate::hid::{Collection, Handle};
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, Instant};

fn dl(ms: u64) -> Instant {
    Instant::now() + Duration::from_millis(ms)
}

fn status(
    key: impl Into<String>,
    name: impl Into<String>,
    kind: &str,
    source: &str,
    level: Option<u8>,
    charging: Option<bool>,
    charging_known: bool,
    online: bool,
) -> Status {
    let mut st = Status::new(key, name);
    st.kind = kind.into();
    st.source = source.into();
    st.level = level;
    st.charging = charging;
    st.charging_known = charging_known;
    st.online = online;
    st
}

/// 一次 poll 观测周期内同一图标只保留一个来源（Python 按 serial/path 分组）。
/// 主程序的轮询间隔为 60 s，5 s 的窗口足以只合并同一轮里的多个实例。
fn claim(recent: &mut HashMap<String, Instant>, key: &str, now: Instant, window: Duration) -> bool {
    if recent
        .get(key)
        .is_some_and(|at| now.saturating_duration_since(*at) < window)
    {
        return false;
    }
    recent.insert(key.to_string(), now);
    true
}

const DEDUP_WINDOW: Duration = Duration::from_secs(5);

// ===========================================================================
// Turtle Beach：VID 0x10F5，vendor collection FF13:0001。
// 两条交换：Pro II / Atlas Air 用 Swarm II 的 SInf/GSI 62 字节问答；
// Stealth 700 Gen 3 用 RACE 隧道里的 CoAP GET /GSI。两者都返回同一份 GSI JSON
// 记录（240 = 电量，220 = 昵称，250 = 充电），共用下面的解析器。
// ===========================================================================
mod turtlebeach {
    use super::*;
    use serde_json::Value;

    const VID: u16 = 0x10F5;
    const KNOWN: &[(u16, &str)] = &[
        (0x229B, "Turtle Beach Stealth Pro II"),
        (0x229E, "Turtle Beach Stealth Pro II"),
        (0x2285, "Turtle Beach Stealth Pro II Xbox"),
        (0x2250, "Turtle Beach Stealth 700 Gen 3"),
        (0x2251, "Turtle Beach Stealth 700 Gen 3"),
        (0x2253, "Turtle Beach Stealth 700 Gen 3 headset"),
        (0x225E, "Turtle Beach Atlas Air"),
        (0x2232, "Turtle Beach Stealth 600X Gen 3"),
    ];
    const ENABLED_PIDS: &[u16] = &[0x229B, 0x229E, 0x2285, 0x2250, 0x2251, 0x2253, 0x225E];
    const COAP_PIDS: &[u16] = &[0x2250, 0x2251, 0x2253];
    const COAP_FALLBACK_PIDS: &[u16] = &[0x2232]; // release hold：只保留解析，不放行
    const DUAL_KEYS: &[(u16, &str)] = &[
        (0x225E, "turtlebeach:atlasair"),
        (0x2232, "turtlebeach:stealth600x"),
    ];
    const FAMILY_KEY: &str = "turtlebeach:stealthpro2";
    const STEALTH700_KEY: &str = "turtlebeach:stealth700";

    const VENDOR_USAGE_PAGE: u16 = 0xFF13;
    const VENDOR_USAGE: u16 = 0x0001;
    const MSG_SIZE: usize = 62;
    const REPORT_ID_IN: u8 = 0x07;
    const OUT_REPORT_ID: u8 = 0x06;
    const RACE_COAP_CLIENT_ENABLE: u16 = 0x9942;
    const RACE_COAP_USB: u16 = 0x9902;
    const COAP_ENABLE_PAUSE_MS: u64 = 300;
    const COAP_POLL_DELAY_MS: u64 = 30;
    const COAP_READS: usize = 30;
    const PACKET_DELAY_MS: u64 = 60;
    const MAX_READS: usize = 16;
    const SINF_READS: usize = 10;
    const FRESH_SHORT_READS: usize = 5;
    const TOTAL_READS: usize = 32;
    const ASLEEP_KEEP_S: u64 = 300;

    // Swarm II 抓包 packet 616 / packet 150；62 字节含 report id 0x06。
    const ASK_GSI: [u8; 62] = [
        0x06, 0x18, 0x00, 0x05, 0x5A, 0x14, 0x00, 0x02, 0x99, 0x48, 0x01, 0x44, 0x14, 0x04,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x61, 0x00, 0x53, 0x47, 0x53, 0x49, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const ASK_SINF: [u8; 62] = [
        0x06, 0x18, 0x00, 0x05, 0x5A, 0x14, 0x00, 0x02, 0x99, 0x48, 0x01, 0xC0, 0x4A, 0x01,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x61, 0x00, 0x53, 0x49, 0x6E, 0x66, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    fn ask_gsi_fresh() -> [u8; 62] {
        let mut ask = ASK_GSI;
        ask[12] = 0x11;
        ask[13] = 0x01;
        ask
    }

    fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.is_empty() || hay.len() < needle.len() {
            return None;
        }
        (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
    }

    /// `07 <len> 00 <len bytes>` 的帧载荷；格式不对的帧整帧丢弃。
    fn strip_frames(frames: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        for f in frames {
            if f.len() < 3 || f[0] != REPORT_ID_IN {
                continue;
            }
            let n = (f[1] as usize).min(f.len() - 3);
            out.extend_from_slice(&f[3..3 + n]);
        }
        out
    }

    /// 带大括号计数的 JSON 扫描：记录之间的封装字节被跳过，控制字节放弃当前候选。
    fn json_documents(blob: &[u8]) -> Vec<Value> {
        let mut out = Vec::new();
        let mut i = 0usize;
        while let Some(rel) = find_sub(&blob[i..], b"{\"") {
            let s = i + rel;
            let (mut depth, mut in_str, mut esc, mut end) = (0i32, false, false, None);
            let mut j = s;
            while j < blob.len() {
                let c = blob[j];
                if in_str {
                    if esc {
                        esc = false;
                    } else if c == b'\\' {
                        esc = true;
                    } else if c == b'"' {
                        in_str = false;
                    }
                } else if c == b'"' {
                    in_str = true;
                } else if c == b'{' {
                    depth += 1;
                } else if c == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(j + 1);
                        break;
                    }
                } else if c < 0x20 {
                    break;
                }
                j += 1;
            }
            match end {
                None => i = s + 1,
                Some(e) => {
                    if let Ok(v) = serde_json::from_slice::<Value>(&blob[s..e]) {
                        out.push(v);
                    }
                    i = e;
                }
            }
        }
        out
    }

    fn origin(d: &Value) -> Option<&str> {
        d.get("UP")
            .and_then(Value::as_str)
            .or_else(|| d.get("OR").and_then(Value::as_str))
    }

    fn py_str(v: &Value) -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => {
                if *b {
                    "True".into()
                } else {
                    "False".into()
                }
            }
            Value::Null => "None".into(),
            _ => v.to_string(),
        }
    }

    /// (level, name)：240/220 分别独立跟踪，部分更新不会清掉另一个。
    fn status_from_documents(docs: &[Value]) -> (Option<u8>, Option<String>) {
        let (mut level, mut name) = (None, None);
        for d in docs {
            if origin(d) != Some("GSI") {
                continue;
            }
            let Some(kvp) = d.get("KVP").and_then(Value::as_object) else {
                continue;
            };
            if let Some(v) = kvp.get("220").and_then(Value::as_str) {
                if !v.is_empty() {
                    name = Some(v.to_string());
                }
            }
            if let Some(v) = kvp.get("240") {
                let parsed = v
                    .as_str()
                    .and_then(|s| s.parse::<i64>().ok())
                    .or_else(|| v.as_i64());
                if let Some(n) = parsed {
                    if (0..=100).contains(&n) {
                        level = Some(n as u8);
                    }
                }
            }
        }
        (level, name)
    }

    /// 充电取最新的 250；不含 250 的部分更新保持「未回答」。
    fn charging_from_documents(docs: &[Value]) -> Option<bool> {
        for d in docs.iter().rev() {
            if origin(d) != Some("GSI") {
                continue;
            }
            let Some(kvp) = d.get("KVP").and_then(Value::as_object) else {
                continue;
            };
            if let Some(v) = kvp.get("250") {
                return Some(py_str(v) == "1");
            }
        }
        None
    }

    /// Swarm II 的 SInf → GSI →（冷却时的）fresh-token GSI 三次询问。
    fn read_sinf(h: &mut Handle) -> (Option<u8>, Option<String>, Option<bool>) {
        let fresh = ask_gsi_fresh();
        let asks: [(&[u8], usize); 3] = [
            (&ASK_SINF, SINF_READS),
            (&ASK_GSI, MAX_READS),
            (&fresh, MAX_READS),
        ];
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let (mut level, mut name) = (None, None);
        let mut payload_seen = false;
        let mut budget = TOTAL_READS;
        for (idx, (req, requested)) in asks.iter().enumerate() {
            if level.is_some() {
                break;
            }
            let mut reads = *requested;
            if idx == 2 && !payload_seen {
                reads = reads.min(FRESH_SHORT_READS);
            }
            if !h.write(req, dl(500)) {
                continue;
            }
            for _ in 0..reads {
                if budget == 0 {
                    break;
                }
                budget -= 1;
                let Some(f) = h.read(dl(PACKET_DELAY_MS)) else {
                    continue;
                };
                if f.is_empty() {
                    continue;
                }
                if f.len() > 3 && f[1] > 0 {
                    payload_seen = true;
                }
                frames.push(f);
                let (l, n) = status_from_documents(&json_documents(&strip_frames(&frames)));
                level = l;
                if n.is_some() {
                    name = n;
                }
                if level.is_some() {
                    break;
                }
            }
        }
        let docs = json_documents(&strip_frames(&frames));
        let charging = charging_from_documents(&docs);
        if name.is_none() {
            name = status_from_documents(&docs).1;
        }
        (level, name, charging)
    }

    fn race_frame(cmd: u16, payload: &[u8]) -> Vec<u8> {
        let n = payload.len() + 2;
        let mut v = vec![
            0x05,
            0x5A,
            (n & 0xFF) as u8,
            ((n >> 8) & 0xFF) as u8,
            (cmd & 0xFF) as u8,
            ((cmd >> 8) & 0xFF) as u8,
        ];
        v.extend_from_slice(payload);
        v
    }

    fn coap_get(path: &str) -> Vec<u8> {
        let mut v = vec![0x40, 0x01, 0x00, 0x01, 0xB0 | path.len() as u8];
        v.extend_from_slice(path.as_bytes());
        v
    }

    /// 输出报告 0x06：`<len16 LE><RACE 帧>`，补齐到 62 字节。
    fn coap_output(frame: &[u8]) -> [u8; MSG_SIZE] {
        let mut buf = [0u8; MSG_SIZE];
        buf[0] = OUT_REPORT_ID;
        buf[1] = (frame.len() & 0xFF) as u8;
        buf[2] = ((frame.len() >> 8) & 0xFF) as u8;
        let n = frame.len().min(MSG_SIZE - 3);
        buf[3..3 + n].copy_from_slice(&frame[..n]);
        buf
    }

    /// Stealth 700 Gen 3 的 CoAP/RACE 隧道；该型号的 250 不随充电变化，充电恒为未知。
    fn read_coap(h: &mut Handle) -> (Option<u8>, Option<String>, Option<bool>) {
        for _ in 0..5 {
            let _ = h.read(dl(30));
        }
        let out = coap_output(&race_frame(RACE_COAP_CLIENT_ENABLE, &[]));
        if !h.write(&out, dl(500)) {
            return (None, None, None);
        }
        sleep(Duration::from_millis(COAP_ENABLE_PAUSE_MS));
        for _ in 0..5 {
            let _ = h.read(dl(30));
        }
        let out = coap_output(&race_frame(RACE_COAP_USB, &coap_get("GSI")));
        if !h.write(&out, dl(500)) {
            return (None, None, None);
        }
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let (mut level, mut name) = (None, None);
        for _ in 0..COAP_READS {
            let Some(f) = h.read(dl(COAP_POLL_DELAY_MS)) else {
                continue;
            };
            if f.is_empty() {
                continue;
            }
            frames.push(f);
            let (l, n) = status_from_documents(&json_documents(&strip_frames(&frames)));
            level = l;
            if n.is_some() {
                name = n;
            }
            if level.is_some() {
                break;
            }
        }
        if level.is_none() {
            let (l, n) = status_from_documents(&json_documents(&strip_frames(&frames)));
            level = l;
            if n.is_some() {
                name = n;
            }
        }
        (level, name, None)
    }

    fn read_status(h: &mut Handle, pid: u16) -> (Option<u8>, Option<String>, Option<bool>) {
        if COAP_PIDS.contains(&pid) {
            return read_coap(h);
        }
        let r = read_sinf(h);
        if r.0.is_none() && COAP_FALLBACK_PIDS.contains(&pid) {
            let coap = read_coap(h);
            if coap.0.is_some() {
                return coap;
            }
        }
        r
    }

    #[derive(Default)]
    struct State {
        last: HashMap<String, (u8, Option<bool>, Instant)>,
        recent: HashMap<String, Instant>,
    }

    pub struct TurtleBeach {
        state: Mutex<State>,
    }

    impl TurtleBeach {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(State::default()),
            }
        }
    }

    impl Default for TurtleBeach {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for TurtleBeach {
        fn name(&self) -> &'static str {
            "turtlebeach"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID
                || !KNOWN.iter().any(|(p, _)| *p == pid)
                || !ENABLED_PIDS.contains(&pid)
            {
                return Vec::new();
            }
            let known_name = KNOWN
                .iter()
                .find(|(p, _)| *p == pid)
                .map(|(_, n)| *n)
                .unwrap_or("Turtle Beach");
            // Python 的 700 Gen 3 名字来自 product string；Rust Collection 无此字段，
            // 用 PID 表里的名字代替（图标 key 与分组不受影响）。
            let key = if COAP_PIDS.contains(&pid) {
                STEALTH700_KEY
            } else if let Some((_, k)) = DUAL_KEYS.iter().find(|(p, _)| *p == pid) {
                *k
            } else {
                FAMILY_KEY
            };

            let mut cands: Vec<&Collection> = collections
                .iter()
                .filter(|c| (c.usage_page, c.usage) == (VENDOR_USAGE_PAGE, VENDOR_USAGE))
                .collect();
            if cands.is_empty() {
                cands = collections
                    .iter()
                    .filter(|c| c.usage_page == VENDOR_USAGE_PAGE)
                    .collect();
            }
            if cands.is_empty() {
                cands = collections
                    .iter()
                    .filter(|c| c.usage_page >= 0xFF00)
                    .collect();
            }

            let mut level = None;
            let mut charging = None;
            for c in cands {
                let Some(mut h) = c.open() else {
                    continue;
                };
                let (l, _, ch) = read_status(&mut h, pid);
                if l.is_some() {
                    level = l;
                    charging = ch;
                    break;
                }
            }

            let now = Instant::now();
            let mut state = self.state.lock().unwrap();
            let (shown, chg, online) = match level {
                Some(lv) => {
                    state
                        .last
                        .insert(key.to_string(), (lv, charging, now));
                    (Some(lv), charging, true)
                }
                None => match state.last.get(key) {
                    Some((lv, ch, when))
                        if now.saturating_duration_since(*when) < Duration::from_secs(ASLEEP_KEEP_S) =>
                    {
                        (Some(*lv), *ch, false)
                    }
                    _ => return Vec::new(),
                },
            };
            if !claim(&mut state.recent, key, now, DEDUP_WINDOW) {
                return Vec::new();
            }
            vec![status(
                key,
                known_name,
                "headset",
                "turtlebeach",
                shown,
                chg,
                chg.is_some(),
                online,
            )]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn swarm_ii_asks_are_byte_exact() {
            assert_eq!(ASK_GSI.len(), 62);
            assert_eq!(ASK_SINF.len(), 62);
            assert_eq!(ASK_GSI[0], 0x06);
            assert_eq!(&ASK_GSI[12..14], &[0x14, 0x04]);
            assert_eq!(&ASK_GSI[21..27], b"a\0SGSI");
            assert_eq!(&ASK_SINF[21..27], b"a\0SInf");
            let fresh = ask_gsi_fresh();
            assert_eq!(&fresh[12..14], &[0x11, 0x01]);
            assert_eq!(fresh[14..], ASK_GSI[14..]);
        }

        #[test]
        fn coap_race_frames_and_output_report() {
            assert_eq!(race_frame(0x9942, &[]), vec![0x05, 0x5A, 0x02, 0x00, 0x42, 0x99]);
            assert_eq!(coap_get("GSI"), vec![0x40, 0x01, 0x00, 0x01, 0xB3, b'G', b'S', b'I']);
            let out = coap_output(&race_frame(0x9902, &coap_get("GSI")));
            assert_eq!(out.len(), 62);
            assert_eq!(out[0], 0x06);
            // 报告长度字段是整帧长度 14；RACE 内部的 len 字段是 payload+2 = 10。
            assert_eq!(&out[1..3], &[0x0E, 0x00]);
            assert_eq!(&out[3..9], &[0x05, 0x5A, 0x0A, 0x00, 0x02, 0x99]);
            assert_eq!(&out[9..13], &[0x40, 0x01, 0x00, 0x01]);
        }

        #[test]
        fn gsi_record_and_partial_push() {
            let doc = br#"{"OR":"GSI","KVP":{"200":"1","220":"My Headset","240":"86","250":"0"}}"#;
            let mut blob = vec![0x05, 0x5B, 0xAA]; // envelope：不在 JSON 扫描范围内
            blob.extend_from_slice(doc);
            let docs = json_documents(&blob);
            assert_eq!(status_from_documents(&docs), (Some(86), Some("My Headset".into())));
            assert_eq!(charging_from_documents(&docs), Some(false));

            let mut push = Vec::new();
            push.extend_from_slice(br#"{"UP":"GSI","KVP":{"250":"1"}}"#);
            let d2 = json_documents(&push);
            assert_eq!(charging_from_documents(&d2), Some(true));
            assert_eq!(status_from_documents(&d2), (None, None));
        }

        #[test]
        fn framed_replies_are_stripped_before_json_scan() {
            let payload = br#"{"OR":"GSI","KVP":{"240":"77"}}"#;
            let mut frame = vec![0x07, payload.len() as u8, 0x00];
            frame.extend_from_slice(payload);
            assert_eq!(strip_frames(&[frame]), payload);
            assert_eq!(
                status_from_documents(&json_documents(&strip_frames(&[vec![0x07, 0x01, 0x00, 0xAA]]))),
                (None, None)
            );
        }
    }
}

// ===========================================================================
// AJAZZ：VID 0x249A PID 0x5C2F（2.4G 接收器）+ 248A:5D2E（鼠标自身 USB）；
// 3151:402D（AJ159 PRO，已放行）/3151:5031（Kysona，保留未放行）。
// AJ179 用 0x10 info 帧与 C0 广播；3151 8K 用 F7/0x05 feature 交换。
// ===========================================================================
mod ajazz {
    use super::*;

    const VID: u16 = 0x249A;
    const PID: u16 = 0x5C2F;
    const WIRED_VID: u16 = 0x248A;
    const WIRED_PID: u16 = 0x5D2E;
    const NAME: &str = "AJAZZ AJ179 V2 MAX";
    const KEY: &str = "ajazz:5c2f";
    const ALT_TARGETS: &[(u16, u16, &str)] = &[
        (0x3151, 0x402D, "AJAZZ AJ159 PRO"),
        (0x3151, 0x5031, "Kysona Uranus"),
    ];
    const REPORT_SIZE: usize = 32;
    const OUTPUT_LEN: usize = 33;
    const READ_SIZE: usize = 33;
    const READ_TIMEOUT_MS: u64 = 250;
    const READ_ATTEMPTS: usize = 5;
    const INFO_CMD: u8 = 0x10;
    const INFO_PAD: u8 = 0x00;
    const INFO_MARK: u8 = 0x01;
    const INFO_LEN: u8 = 0x0B;
    const LEVEL_INDEX: usize = 13;
    const FLAG_INDEX: usize = 14;
    const CHECKSUM_INDEX: usize = 31;
    const CHECKSUM_FROM: usize = 4;
    const ANNOUNCE_ID: u8 = 0xC0;
    const ANNOUNCE_LIVE: u8 = 0x01;
    const ANNOUNCE_LEVEL_INDEX: usize = 2;
    const SERIES_POLL_OPCODE: u8 = 0xF7;
    const SERIES_POLL_LENGTHS: [usize; 3] = [33, 65, 67];
    const SERIES_SETTLE_MS: u64 = 50;
    const SERIES_REPORT_ID: u8 = 0x05;
    const SERIES_LEVEL_INDEX: usize = 3;
    const SERIES_CONTROL_USAGE: (u16, u16) = (0xFFFF, 0x0002);
    const MOUSE_USAGE: (u16, u16) = (0x0001, 0x0002);
    const VENDOR_PAGE: u16 = 0xFF00;
    const MAX_CANDIDATES: usize = 6;
    const ASLEEP_KEEP_S: u64 = 300;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Kind {
        Info,
        Announcement,
        Status,
    }

    /// hidapi 要求 report id (0) 在前，后面 32 字节 payload，0x10 是命令。
    fn make_request() -> [u8; OUTPUT_LEN] {
        let mut req = [0u8; OUTPUT_LEN];
        req[1] = INFO_CMD;
        req
    }

    fn parse_reply(frame: &[u8]) -> Option<(u8, Kind, Option<bool>)> {
        if frame.is_empty() {
            return None;
        }
        let mut f = frame.to_vec();
        if f.len() == READ_SIZE && f[0] == 0x00 {
            f.remove(0);
        }
        if f.len() < REPORT_SIZE {
            return None;
        }
        if f[0] == ANNOUNCE_ID {
            if f[1] != ANNOUNCE_LIVE {
                return None;
            }
            let level = f[ANNOUNCE_LEVEL_INDEX];
            return (level > 0 && level <= 100).then_some((level, Kind::Announcement, None));
        }
        if f[0] != INFO_CMD || f[1] != INFO_PAD || f[2] != INFO_MARK || f[3] != INFO_LEN {
            return None;
        }
        let sum = f[CHECKSUM_FROM..CHECKSUM_INDEX]
            .iter()
            .fold(0u8, |a, &b| a.wrapping_add(b));
        if sum != f[CHECKSUM_INDEX] {
            return None;
        }
        let level = f[LEVEL_INDEX];
        (level > 0 && level <= 100)
            .then(|| (level, Kind::Info, Some(f[FLAG_INDEX] == 0x00)))
    }

    /// `05 00 00 <level>`；0 表示 2.4G 尚未连上，不能当成空电量。
    fn parse_series_reply(frame: &[u8], report_id: u8) -> Option<u8> {
        if report_id != SERIES_REPORT_ID || frame.len() <= SERIES_LEVEL_INDEX {
            return None;
        }
        if frame[0] != SERIES_REPORT_ID || frame[1..SERIES_LEVEL_INDEX].iter().any(|&b| b != 0) {
            return None;
        }
        let level = frame[SERIES_LEVEL_INDEX];
        (1..=100).contains(&level).then_some(level)
    }

    /// AJ159 诊断用：同样只认编号报告 05 00 00 <level>。
    #[allow(dead_code)]
    fn parse_aj159_diagnostic(frame: &[u8]) -> Option<u8> {
        if frame.len() < 4 {
            return None;
        }
        parse_series_reply(frame, SERIES_REPORT_ID)
    }

    /// 按 OutputReportByteLength 排序：能接受 33 字节写的是 vendor 通道。
    fn candidates<'a>(collections: &'a [Collection]) -> Vec<&'a Collection> {
        let mut seen = HashSet::new();
        let mut out: Vec<&Collection> = collections
            .iter()
            .filter(|c| {
                if (c.usage_page, c.usage) == MOUSE_USAGE {
                    return false;
                }
                seen.insert((
                    c.usage_page,
                    c.usage,
                    c.interface,
                    c.output_len,
                    c.feature_len,
                ))
            })
            .collect();
        out.sort_by_key(|c| {
            if c.output_len == OUTPUT_LEN {
                (0, 0)
            } else if c.output_len == 0 {
                // Rust 对 usage_page < 0xFF00 的集合把 output_len 记为 0；视为未知能力。
                (1, if c.usage_page >= VENDOR_PAGE { 0 } else { 1 })
            } else {
                (2, 0)
            }
        });
        out
    }

    /// 写 0x10 并听完 READ_ATTEMPTS 帧：广播留作后备，info 块立即返回（带充电位）。
    fn ask(c: &Collection) -> (Option<(u8, Kind, Option<bool>)>, bool) {
        let Some(mut h) = c.open() else {
            return (None, false);
        };
        let req = make_request();
        if !h.write(&req, dl(READ_TIMEOUT_MS)) {
            return (None, false);
        }
        let mut announce = None;
        for _ in 0..READ_ATTEMPTS {
            let Some(r) = h.read(dl(READ_TIMEOUT_MS)) else {
                continue;
            };
            let Some(got) = parse_reply(&r) else {
                continue;
            };
            if got.1 == Kind::Announcement {
                announce = Some(got);
                continue;
            }
            return (Some(got), true);
        }
        (announce, true)
    }

    /// 3151 8K 接收器：F7 feature 唤醒 + GET_FEATURE 0x05 读状态。
    fn ask_series(c: &Collection) -> (Option<(u8, Kind, Option<bool>)>, bool) {
        let Some(mut h) = c.open() else {
            return (None, false);
        };
        let mut accepted = false;
        for len in SERIES_POLL_LENGTHS {
            if c.feature_len != 0 && c.feature_len < len {
                continue;
            }
            let mut poll = vec![0u8; len];
            poll[0] = 0x00;
            poll[1] = SERIES_POLL_OPCODE;
            if !h.set_feature(&poll, dl(500)) {
                continue;
            }
            accepted = true;
            sleep(Duration::from_millis(SERIES_SETTLE_MS));
            // Rust 的 get_feature 按集合的 feature 长度取一帧；Python 还会按
            // (length, 65, 33) 重试，但同一帧的前缀一致，不改变解析结果。
            if let Some(r) = h.get_feature(SERIES_REPORT_ID, dl(READ_TIMEOUT_MS)) {
                if let Some(level) = parse_series_reply(&r, SERIES_REPORT_ID) {
                    return (Some((level, Kind::Status, None)), true);
                }
            }
        }
        (None, accepted)
    }

    #[derive(Default)]
    struct AltState {
        last: Option<(u8, Instant)>,
        charging: bool,
    }

    #[derive(Default)]
    struct State {
        last: Option<(u8, Instant)>,
        charging: bool,
        chosen: Option<(u16, u16, Option<u8>)>,
        alt: HashMap<u16, AltState>,
        recent: HashMap<String, Instant>,
    }

    pub struct Ajazz {
        state: Mutex<State>,
    }

    impl Ajazz {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(State::default()),
            }
        }
    }

    impl Default for Ajazz {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Ajazz {
        fn name(&self) -> &'static str {
            "ajazz"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            let now = Instant::now();
            let mut state = self.state.lock().unwrap();

            if (vid, pid) == (VID, PID) {
                let mut order = candidates(collections);
                if let Some(ch) = state.chosen {
                    order.sort_by_key(|c| ((c.usage_page, c.usage, c.interface) != ch) as u8);
                }
                let mut got = None;
                for c in order.iter().take(MAX_CANDIDATES) {
                    let (g, accepted) = ask(c);
                    if accepted {
                        state.chosen = Some((c.usage_page, c.usage, c.interface));
                        got = g;
                        break; // vendor 通道静默即鼠标睡了，不再试下一个
                    }
                    if g.is_some() {
                        got = g;
                        break;
                    }
                }
                let (level, charging, online) = match got {
                    Some((lv, _, ch)) => {
                        // 无线链路上的回答不可能是充电中的状态。
                        let charging = ch.unwrap_or(false);
                        if ch.is_some() {
                            state.charging = charging;
                        }
                        state.last = Some((lv, now));
                        (Some(lv), Some(charging), true)
                    }
                    None => match state.last {
                        Some((lv, when))
                            if now.saturating_duration_since(when)
                                < Duration::from_secs(ASLEEP_KEEP_S) =>
                        {
                            (Some(lv), Some(state.charging), false)
                        }
                        _ => (None, Some(false), false),
                    },
                };
                if !claim(&mut state.recent, KEY, now, DEDUP_WINDOW) {
                    return Vec::new();
                }
                return vec![status(KEY, NAME, "mouse", "ajazz", level, charging, true, online)];
            }

            if (vid, pid) == (WIRED_VID, WIRED_PID) {
                // 无线链路断开时鼠标在自身 USB 线上回答同一个 KEY；轮询以
                // USB instance 为单位，这里作为接收器分支的补充路径。
                let order = candidates(collections);
                let mut got = None;
                for c in order.iter().take(MAX_CANDIDATES) {
                    let (g, accepted) = ask(c);
                    if g.is_some() {
                        got = g;
                    }
                    if accepted {
                        break;
                    }
                }
                let (level, charging, online) = match got {
                    Some((lv, _, ch)) => {
                        let charging = ch.unwrap_or(state.charging);
                        if ch.is_some() {
                            state.charging = charging;
                        }
                        state.last = Some((lv, now));
                        (Some(lv), Some(charging), true)
                    }
                    None => match state.last {
                        Some((lv, when))
                            if now.saturating_duration_since(when)
                                < Duration::from_secs(ASLEEP_KEEP_S) =>
                        {
                            (Some(lv), Some(state.charging), false)
                        }
                        _ => (None, Some(false), false),
                    },
                };
                if !claim(&mut state.recent, KEY, now, DEDUP_WINDOW) {
                    return Vec::new();
                }
                return vec![status(KEY, NAME, "mouse", "ajazz", level, charging, true, online)];
            }

            let Some((_, _, name)) = ALT_TARGETS.iter().find(|(v, p, _)| *v == vid && *p == pid)
            else {
                return Vec::new();
            };
            // 只有 3151:402D 被放行；5031 保留协议但不打开。
            if vid != 0x3151 || pid != 0x402D {
                return Vec::new();
            }
            let control: Vec<&Collection> = collections
                .iter()
                .filter(|c| (c.usage_page, c.usage) == SERIES_CONTROL_USAGE)
                .collect();
            let mut got = None;
            for c in control {
                let (g, accepted) = ask_series(c);
                if g.is_some() {
                    got = g;
                }
                let _ = accepted;
            }
            let st = state.alt.entry(pid).or_default();
            let key = format!("ajazz:{pid:04x}");
            let (level, charging, online) = match got {
                Some((lv, _, ch)) => {
                    let charging = match ch {
                        Some(v) => {
                            st.charging = v;
                            v
                        }
                        None => st.charging,
                    };
                    st.last = Some((lv, now));
                    (Some(lv), charging, true)
                }
                None => match st.last {
                    Some((lv, when))
                        if now.saturating_duration_since(when)
                            < Duration::from_secs(ASLEEP_KEEP_S) =>
                    {
                        (Some(lv), st.charging, false)
                    }
                    _ => (None, st.charging, false),
                },
            };
            if !claim(&mut state.recent, &key, now, DEDUP_WINDOW) {
                return Vec::new();
            }
            // Python 对该目标显式 charging_known=False。
            vec![status(&key, *name, "mouse", "ajazz", level, Some(charging), false, online)]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_and_info_reply_checksum() {
            let req = make_request();
            assert_eq!(req.len(), 33);
            assert_eq!(req[0], 0x00);
            assert_eq!(req[1], INFO_CMD);

            // 源码注释中的实测块：10 00 01 0b "N625" .. 4b(75%) 01(off cable) .. 49
            let mut f = [0u8; 32];
            let hdr = [
                0x10, 0x00, 0x01, 0x0B, 0x4E, 0x36, 0x32, 0x35, 0x00, 0x00, 0x11, 0x01, 0x00,
                0x4B, 0x01, 0x00,
            ];
            f[..16].copy_from_slice(&hdr);
            f[31] = f[4..31].iter().fold(0u8, |a, &b| a.wrapping_add(b));
            assert_eq!(f[31], 0x49);
            // 抓包里的 0x01 = 不在充电线上；0x00 才是充电中。
            assert_eq!(parse_reply(&f), Some((75, Kind::Info, Some(false))));

            let mut with_id = [0u8; 33];
            with_id[1..].copy_from_slice(&f);
            assert_eq!(parse_reply(&with_id), Some((75, Kind::Info, Some(false))));

            f[14] = 0x00;
            f[31] = f[4..31].iter().fold(0u8, |a, &b| a.wrapping_add(b));
            assert_eq!(parse_reply(&f), Some((75, Kind::Info, Some(true))));
            f[31] ^= 0xFF;
            assert_eq!(parse_reply(&f), None);
        }

        #[test]
        fn announcement_and_series_reply() {
            let mut a = [0u8; 32];
            a[0] = ANNOUNCE_ID;
            a[1] = ANNOUNCE_LIVE;
            a[2] = 74;
            assert_eq!(parse_reply(&a), Some((74, Kind::Announcement, None)));
            a[1] = 0x00; // c0 00 是命令应答，不是广播
            assert_eq!(parse_reply(&a), None);

            // #289 实测：05 00 00 46 -> 70%
            let r = [0x05, 0x00, 0x00, 0x46, 0x01, 0x01, 0x01, 0x02];
            assert_eq!(parse_series_reply(&r, SERIES_REPORT_ID), Some(70));
            assert_eq!(parse_series_reply(&r, 0x06), None);
            let reconnect = [0x05, 0x01, 0x00, 0x46];
            assert_eq!(parse_series_reply(&reconnect, SERIES_REPORT_ID), None);
            assert_eq!(parse_series_reply(&[0x05, 0x00, 0x00, 0x00], SERIES_REPORT_ID), None);
        }
    }
}

// ===========================================================================
// Lofree：Hyzen (388D:0025, FF1C:0092) 的 0x04 START/cmd/END 事务；
// Flow Lite84 (05AC:024F / 3554:FA0A, FF02:0002) 的 17 字节 Compx 帧；
// AULA V9 (320F:22A0/22B7) 复用同一事务并读取 charge 字节与链路心跳。
// ===========================================================================
mod lofree {
    use super::*;

    const LOFREE_VID: u16 = 0x388D;
    const USAGE_PAGE: u16 = 0xFF1C;
    const USAGE: u16 = 0x0092;
    const DONGLES: &[(u16, &str)] = &[(0x0025, "Lofree Hyzen")];
    const WIRED_OF: &[(u16, u16)] = &[(0x0025, 0x0024)];
    const AULA_VID: u16 = 0x320F;
    const AULA_PIDS: &[(u16, &str)] = &[
        (0x22A0, "AULA V9 Max+ (2.4 GHz receiver)"),
        (0x22B7, "AULA V9 Max+ (USB cable)"),
    ];
    const REPORT_ID: u8 = 0x04;
    const FRAME: usize = 31; // report id 之后的长度
    const START: u8 = 0x01;
    const END: u8 = 0x02;
    const CMD_ONLINE: u8 = 0xAA;
    const CMD_BATTERY: u8 = 0x1A;
    const DATA_AT: usize = 7;
    const TIMEOUT_MS: u64 = 3000;
    const CHARGE_AT: usize = 8;
    const HEARTBEAT_AT: usize = 7;
    const LINK_ON: u8 = 0xFF;
    const READ_MS: u64 = 100;

    const FLOW_RECEIVERS: &[(u16, u16)] = &[(0x05AC, 0x024F), (0x3554, 0xFA0A)];
    const FLOW_NAME: &str = "Lofree Flow Lite84";
    const FLOW_CONTROL_USAGE: (u16, u16) = (0xFF02, 0x0002);
    const FLOW_HEADER: u8 = 0x08;
    const FLOW_LEN: usize = 17;
    const FLOW_CHECKSUM_BASE: u16 = 0x55;
    const FLOW_CMD_ONLINE: u8 = 0x03;
    const FLOW_CMD_BATTERY: u8 = 0x04;
    const FLOW_FLAG_KEYBOARD: u8 = 0x80;
    const FLOW_ONLINE_INDEX: usize = 6;
    const FLOW_LEVEL_INDEX: usize = 6;
    const FLOW_CHARGING_INDEX: usize = 7;
    const FLOW_READ_ATTEMPTS: usize = 4;
    const FLOW_READ_MS: u64 = 250;
    const FLOW_FLUSH_MS: u64 = 30;

    #[derive(Default)]
    struct State {
        link_down: bool,
        wired_seen: Option<Instant>,
        recent: HashMap<String, Instant>,
    }

    fn frame(b2: u8) -> [u8; FRAME + 1] {
        let mut f = [0u8; FRAME + 1];
        f[0] = REPORT_ID;
        f[3] = b2;
        f
    }

    fn payload(r: &[u8]) -> Option<Vec<u8>> {
        if r.len() < 2 || r[0] != REPORT_ID {
            return None;
        }
        Some(r[1..].to_vec())
    }

    fn wait_payload(
        h: &mut Handle,
        want: impl Fn(&[u8]) -> bool,
        watch_link: bool,
        end: Instant,
        state: &mut State,
    ) -> Option<Vec<u8>> {
        while Instant::now() < end {
            let d = end.min(dl(READ_MS));
            let Some(r) = h.read(d) else {
                continue;
            };
            let Some(p) = payload(&r) else {
                continue;
            };
            if watch_link && p.len() > HEARTBEAT_AT && p[0] == 0xAA && p[2] == 0xAA {
                if p[HEARTBEAT_AT] != LINK_ON {
                    state.link_down = true;
                }
                continue;
            }
            if want(&p) {
                return Some(p);
            }
        }
        None
    }

    /// 一次 START / 命令 / END 事务；END 无论命令是否应答都要发。
    fn command(
        h: &mut Handle,
        cmd: u8,
        watch_link: bool,
        state: &mut State,
    ) -> Option<Vec<u8>> {
        let end = Instant::now() + Duration::from_millis(TIMEOUT_MS);
        if !h.write(&frame(START), dl(500)) {
            return None;
        }
        if wait_payload(h, |p| p.len() >= 3 && p[2] == START, watch_link, end, state).is_none() {
            return None;
        }
        let mut req = [0u8; FRAME + 1];
        req[0] = REPORT_ID;
        req[3] = cmd;
        if !h.write(&req, dl(500)) {
            return None;
        }
        let got = wait_payload(
            h,
            |p| {
                p.len() > DATA_AT
                    && (p[2] == cmd || p[2] == 0x00)
                    && p[4] == 0x00
                    && p[5] == 0x00
            },
            watch_link,
            end,
            state,
        );
        let _ = h.write(&frame(END), dl(500));
        let close_end = end.min(Instant::now() + Duration::from_millis(500));
        let _ = wait_payload(h, |p| p.len() >= 3 && p[2] == END, false, close_end, state);
        got
    }

    fn read_hyzen(h: &mut Handle, state: &mut State) -> Option<u8> {
        let online = command(h, CMD_ONLINE, false, state)?;
        if online.get(DATA_AT).copied().unwrap_or(0) == 0 {
            return None;
        }
        let answer = command(h, CMD_BATTERY, false, state)?;
        let level = *answer.get(DATA_AT)?;
        (level <= 100).then_some(level)
    }

    /// AULA 的 charge：0 未充 / 1 充电中 / 2 已充满；非法值按 0。
    fn mouse_charge(answer: &[u8]) -> bool {
        let charge = answer.get(CHARGE_AT).copied().unwrap_or(0);
        matches!(charge, 1 | 2)
    }

    fn read_mouse(h: &mut Handle, state: &mut State) -> Option<(u8, bool)> {
        state.link_down = false;
        let answer = command(h, CMD_BATTERY, true, state)?;
        let level = *answer.get(DATA_AT)?;
        // 刚重插时接收器会先回一串 0，0 不是电量。
        if !(1..=100).contains(&level) {
            return None;
        }
        Some((level, mouse_charge(&answer)))
    }

    fn flow_checksum(f: &[u8]) -> u8 {
        let sum: u16 = f.iter().map(|&b| b as u16).sum();
        (FLOW_CHECKSUM_BASE.wrapping_sub(sum & 0xFF) & 0xFF) as u8
    }

    fn flow_request(cmd: u8) -> [u8; FLOW_LEN] {
        let mut f = [0u8; FLOW_LEN];
        f[0] = FLOW_HEADER;
        f[1] = cmd;
        f[5] = FLOW_FLAG_KEYBOARD;
        f[FLOW_LEN - 1] = flow_checksum(&f[..FLOW_LEN - 1]);
        f
    }

    fn flow_parse(r: &[u8], cmd: u8) -> Option<Vec<u8>> {
        if r.len() < FLOW_LEN {
            return None;
        }
        let f = &r[..FLOW_LEN];
        if f[0] != FLOW_HEADER || f[1] != cmd || f[2] != 0x00 {
            return None;
        }
        if f[FLOW_LEN - 1] != flow_checksum(&f[..FLOW_LEN - 1]) {
            return None;
        }
        Some(f.to_vec())
    }

    fn flow_command(h: &mut Handle, cmd: u8) -> Option<Vec<u8>> {
        if !h.write(&flow_request(cmd), dl(500)) {
            return None;
        }
        for _ in 0..FLOW_READ_ATTEMPTS {
            let Some(r) = h.read(dl(FLOW_READ_MS)) else {
                break;
            };
            if let Some(f) = flow_parse(&r, cmd) {
                return Some(f);
            }
        }
        None
    }

    fn flow_read(h: &mut Handle) -> Option<(u8, bool)> {
        for _ in 0..4 {
            if h.read(dl(FLOW_FLUSH_MS)).is_none() {
                break;
            }
        }
        let online = flow_command(h, FLOW_CMD_ONLINE)?;
        if online.get(FLOW_ONLINE_INDEX).copied() != Some(1) {
            return None;
        }
        let reply = flow_command(h, FLOW_CMD_BATTERY)?;
        let level = *reply.get(FLOW_LEVEL_INDEX)?;
        if level > 100 {
            return None;
        }
        Some((level, reply.get(FLOW_CHARGING_INDEX).copied() == Some(1)))
    }

    pub struct Lofree {
        state: Mutex<State>,
    }

    impl Lofree {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(State::default()),
            }
        }
    }

    impl Default for Lofree {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Lofree {
        fn name(&self) -> &'static str {
            "lofree"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            let now = Instant::now();
            let mut state = self.state.lock().unwrap();

            if vid == AULA_VID && AULA_PIDS.iter().any(|(p, _)| *p == pid) {
                // 线缆优先；这里每次调用只有一个 pid，顺序只影响同一实例的多集合。
                let mut cols: Vec<&Collection> = collections
                    .iter()
                    .filter(|c| (c.usage_page, c.usage) == (USAGE_PAGE, USAGE))
                    .collect();
                cols.sort_by_key(|c| (c.interface != Some(2)) as u8);
                let mut got = None;
                for c in cols {
                    let Some(mut h) = c.open() else {
                        continue;
                    };
                    if let Some(v) = read_mouse(&mut h, &mut state) {
                        got = Some(v);
                        break;
                    }
                }
                let Some((level, charging)) = got else {
                    return Vec::new();
                };
                if !claim(&mut state.recent, "aula:mouse", now, DEDUP_WINDOW) {
                    return Vec::new();
                }
                let name = AULA_PIDS
                    .iter()
                    .find(|(p, _)| *p == pid)
                    .map(|(_, n)| *n)
                    .unwrap_or("AULA V9 Max+");
                return vec![status(
                    "aula:mouse",
                    name,
                    "mouse",
                    "lofree",
                    Some(level),
                    Some(charging),
                    true,
                    !state.link_down,
                )];
            }

            if vid == LOFREE_VID && WIRED_OF.iter().any(|(_, wired)| *wired == pid) {
                // 有线型号本身不读：只让 0x0025 的接收器跳过读取。
                state.wired_seen = Some(now);
                return Vec::new();
            }
            if vid == LOFREE_VID {
                let Some((_, name)) = DONGLES.iter().find(|(p, _)| *p == pid) else {
                    return Vec::new();
                };
                if state
                    .wired_seen
                    .is_some_and(|at| now.saturating_duration_since(at) < DEDUP_WINDOW)
                {
                    return Vec::new();
                }
                let key = format!("lofree:{pid:04x}");
                for c in collections {
                    if (c.usage_page, c.usage) != (USAGE_PAGE, USAGE) {
                        continue;
                    }
                    let Some(mut h) = c.open() else {
                        continue;
                    };
                    if let Some(level) = read_hyzen(&mut h, &mut state) {
                        if !claim(&mut state.recent, &key, now, DEDUP_WINDOW) {
                            return Vec::new();
                        }
                        return vec![status(
                            &key,
                            *name,
                            "keyboard",
                            "lofree",
                            Some(level),
                            Some(false),
                            true,
                            true,
                        )];
                    }
                }
                return Vec::new();
            }

            if !FLOW_RECEIVERS.iter().any(|(v, p)| *v == vid && *p == pid) {
                return Vec::new();
            }
            // Python 还要求 product string == "2.4G Wireless Receiver"；
            // Rust Collection 无该字段，仅以 VID/PID + FF02:0002 匹配。
            let key = format!("lofree:{pid:04x}");
            for c in collections {
                if (c.usage_page, c.usage) != FLOW_CONTROL_USAGE {
                    continue;
                }
                let Some(mut h) = c.open() else {
                    continue;
                };
                if let Some((level, charging)) = flow_read(&mut h) {
                    if !claim(&mut state.recent, &key, now, DEDUP_WINDOW) {
                        return Vec::new();
                    }
                    return vec![status(
                        &key,
                        FLOW_NAME,
                        "keyboard",
                        "lofree",
                        Some(level),
                        Some(charging),
                        true,
                        true,
                    )];
                }
            }
            Vec::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn hyzen_transaction_frames() {
            let start = frame(START);
            assert_eq!(start.len(), 32);
            assert_eq!(&start[..4], &[0x04, 0x00, 0x00, 0x01]);
            let mut cmd = [0u8; 32];
            cmd[0] = REPORT_ID;
            cmd[3] = CMD_BATTERY;
            assert_eq!(cmd[3], 0x1A);
            let p = payload(&cmd).unwrap();
            assert_eq!(p.len(), FRAME);
            assert_eq!(p[2], CMD_BATTERY);
            assert!(payload(&[0x03, 0x00]).is_none());
        }

        #[test]
        fn flow_request_and_reply_checksum() {
            let req = flow_request(FLOW_CMD_ONLINE);
            assert_eq!(req.len(), 17);
            assert_eq!(&req[..6], &[0x08, 0x03, 0x00, 0x00, 0x00, 0x80]);
            assert_eq!(req[16], 202);
            assert_eq!(req.iter().map(|&b| b as u16).sum::<u16>() % 256, 0x55);
            assert_eq!(flow_request(FLOW_CMD_BATTERY)[16], 201);

            // 实测布局：08 04 00 00 00 80 64 01 0E 74 ... <checksum>
            let mut reply = flow_request(FLOW_CMD_BATTERY);
            reply[FLOW_LEVEL_INDEX] = 100;
            reply[FLOW_CHARGING_INDEX] = 1;
            reply[8] = 0x0E;
            reply[9] = 0x74;
            reply[16] = flow_checksum(&reply[..16]);
            let parsed = flow_parse(&reply, FLOW_CMD_BATTERY).unwrap();
            assert_eq!(parsed[FLOW_LEVEL_INDEX], 100);
            assert_eq!(parsed[FLOW_CHARGING_INDEX], 1);
            assert!(flow_parse(&reply, FLOW_CMD_ONLINE).is_none());
            reply[16] ^= 0xFF;
            assert!(flow_parse(&reply, FLOW_CMD_BATTERY).is_none());
        }

        #[test]
        fn aula_charge_byte_mapping() {
            let mut answer = [0u8; 9];
            answer[DATA_AT] = 0x59; // 89%
            assert!(!mouse_charge(&answer));
            answer[CHARGE_AT] = 0x01;
            assert!(mouse_charge(&answer));
            answer[CHARGE_AT] = 0x02;
            assert!(mouse_charge(&answer));
            answer[CHARGE_AT] = 0x07;
            assert!(!mouse_charge(&answer));
        }
    }
}

// ===========================================================================
// Audeze：VID 0x3329，FF13:0001，62 字节 report 0x06/0x07。
// 单包 BATTERY_ONLY 快路径 + 14+6 包完整序列回退（HeadsetControl 抓包表），
// 在滚动缓冲里扫描 d6 0c 00 00 <percent>，取最新一帧的合法值。
// ===========================================================================
mod audeze {
    use super::*;

    const VID: u16 = 0x3329;
    const KNOWN: &[(u16, &str)] = &[
        (0x4B19, "Audeze Maxwell"),
        (0x4B18, "Audeze Maxwell (Xbox)"),
        (0x4B1A, "Audeze Maxwell (USB cable)"),
        (0x4B29, "Audeze Maxwell 2"),
        (0x4B28, "Audeze Maxwell 2 (Xbox)"),
    ];
    const CABLE_PIDS: &[u16] = &[0x4B1A];
    // Python 用两端相同的 serial 作 key；Rust Collection 无 serial，固定 key 保证一个图标。
    const HEADSET_KEY: &str = "audeze:maxwell";
    const HEADSET_NAME: &str = "Audeze Maxwell";
    const MSG_SIZE: usize = 62;
    const REPORT_ID_OUT: u8 = 0x06;
    const REPORT_ID_IN: u8 = 0x07;
    const PACKET_DELAY_MS: u64 = 60;
    const EXTRA_READS: usize = 2;
    const VENDOR_USAGE_PAGE: u16 = 0xFF13;
    const VENDOR_USAGE: u16 = 0x0001;
    const BATTERY_MARKER: [u8; 4] = [0xD6, 0x0C, 0x00, 0x00];
    const ECHO_MIN_FRAMES: usize = 5;
    const STUCK_POLLS: usize = 2;
    const STUCK_TEXT: &str = "no answer from the headset - unplug the dongle and plug it back in";
    const ZERO_GRACE_S: u64 = 90;
    const WAKING_TEXT: &str = "connected, battery level not reported yet";

    // HeadsetControl 的 14 包初始化序列（逐字节）。
    const INIT: &[&[u8]] = &[
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x20],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x25],
        &[0x06, 0x07, 0x80, 0x05, 0x5A, 0x03, 0x00, 0x07, 0x1C],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x28],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x83, 0x2C, 0x01],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x83, 0x2C, 0x07],
        &[0x06, 0x07, 0x00, 0x05, 0x5A, 0x03, 0x00, 0x07, 0x1C],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x2D],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x2C],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x83, 0x2C, 0x0B],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x24],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x2F],
        &[0x06, 0x07, 0x80, 0x05, 0x5A, 0x03, 0x00, 0xD6, 0x0C],
    ];
    const STATUS: &[&[u8]] = &[
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x22],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x83, 0x2C, 0x0B],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x24],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x01, 0x09, 0x2C],
        &[0x06, 0x08, 0x80, 0x05, 0x5A, 0x04, 0x00, 0x83, 0x2C, 0x07],
    ];
    const BATTERY_QUERY: &[u8] = &[0x06, 0x07, 0x80, 0x05, 0x5A, 0x03, 0x00, 0xD6, 0x0C];

    fn packet(values: &[u8]) -> [u8; MSG_SIZE] {
        let mut data = [0u8; MSG_SIZE];
        let n = values.len().min(MSG_SIZE);
        data[..n].copy_from_slice(&values[..n]);
        data
    }

    /// 从第一个合法 d6 0c 00 00 <0..100> 标记取电量；非法值继续往后找。
    fn find_level(data: &[u8]) -> Option<u8> {
        let mut start = 0usize;
        while start + BATTERY_MARKER.len() < data.len() {
            let rel = data[start..]
                .windows(BATTERY_MARKER.len())
                .position(|w| w == BATTERY_MARKER)?;
            let i = start + rel;
            if i + 4 >= data.len() {
                return None;
            }
            let level = data[i + 4];
            if level <= 100 {
                return Some(level);
            }
            start = i + 1;
        }
        None
    }

    fn newest_level(frames: &[Vec<u8>]) -> Option<u8> {
        frames.iter().rev().find_map(|f| find_level(f))
    }

    /// 卡死的 dongle：每帧都是请求的空回声，后面什么都没有。
    fn echo_only(frames: &[Vec<u8>]) -> bool {
        frames.len() >= ECHO_MIN_FRAMES
            && frames
                .iter()
                .all(|f| f.len() > 3 && f[0] == REPORT_ID_IN && f[3..].iter().all(|&b| b == 0))
    }

    /// 发一组包并收帧；前三包都无人应答就认为这个集合不说话。
    fn sequence(h: &mut Handle, requests: &[&[u8]]) -> (Vec<Vec<u8>>, bool) {
        let mut frames = Vec::new();
        let mut unanswered = 0;
        for req in requests {
            let frame = packet(req);
            if !h.write(&frame, dl(500)) {
                break;
            }
            sleep(Duration::from_millis(PACKET_DELAY_MS));
            match h.read(dl(PACKET_DELAY_MS)) {
                Some(f) if !f.is_empty() => frames.push(f),
                _ => {
                    if frames.is_empty() {
                        unanswered += 1;
                        if unanswered >= 3 {
                            return (frames, false);
                        }
                    }
                }
            }
        }
        for _ in 0..EXTRA_READS {
            sleep(Duration::from_millis(PACKET_DELAY_MS));
            if let Some(f) = h.read(dl(PACKET_DELAY_MS)) {
                if !f.is_empty() {
                    frames.push(f);
                }
            }
        }
        (frames, true)
    }

    #[derive(Default)]
    struct State {
        short_useless: HashMap<String, bool>,
        echo_polls: HashMap<String, usize>,
        up_since: HashMap<String, Instant>,
        recent: HashMap<String, Instant>,
    }

    /// -> (level, charging, echo_only)。协议本身没有充电位，调用方按端点推断。
    fn read_battery(h: &mut Handle, key: &str, state: &mut State) -> (Option<u8>, Option<bool>, bool) {
        let short = !state.short_useless.get(key).copied().unwrap_or(false);
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let mut answered = true;
        let mut level = None;
        if short {
            let (f, a) = sequence(h, &[BATTERY_QUERY]);
            frames = f;
            answered = a;
            level = newest_level(&frames);
        }
        if level.is_none() && answered {
            let (more, _) = sequence(h, REQUESTS);
            frames.extend(more.iter().cloned());
            level = newest_level(&more);
        }
        match level {
            Some(lv) => {
                state.short_useless.insert(key.to_string(), false);
                state.up_since.entry(key.to_string()).or_insert_with(Instant::now);
                (Some(lv), Some(false), false)
            }
            None => {
                if short {
                    state.short_useless.insert(key.to_string(), true);
                }
                let echo = echo_only(&frames);
                (None, None, echo)
            }
        }
    }

    // INIT + STATUS，共 20 包，顺序与 Python `REQUESTS = INIT + STATUS` 一致。
    const REQUESTS: &[&[u8]] = &[
        INIT[0], INIT[1], INIT[2], INIT[3], INIT[4], INIT[5], INIT[6], INIT[7], INIT[8], INIT[9],
        INIT[10], INIT[11], INIT[12], INIT[13], STATUS[0], STATUS[1], STATUS[2], STATUS[3],
        STATUS[4], STATUS[5],
    ];

    pub struct Audeze {
        state: Mutex<State>,
    }

    impl Audeze {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(State::default()),
            }
        }
    }

    impl Default for Audeze {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Audeze {
        fn name(&self) -> &'static str {
            "audeze"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID || !KNOWN.iter().any(|(p, _)| *p == pid) {
                return Vec::new();
            }
            let now = Instant::now();
            let mut state = self.state.lock().unwrap();
            let key = format!("{pid:04x}");
            let is_cable = CABLE_PIDS.contains(&pid);

            let mut level = None;
            let mut echoed = false;
            for c in collections {
                if (c.usage_page, c.usage) != (VENDOR_USAGE_PAGE, VENDOR_USAGE) {
                    continue;
                }
                let Some(mut h) = c.open() else {
                    continue;
                };
                let (l, _, e) = read_battery(&mut h, &key, &mut state);
                echoed |= e;
                if l.is_some() {
                    level = l;
                    break;
                }
            }

            let plugs = state.echo_polls.entry(key.clone()).or_insert(0);
            if level.is_none() && echoed {
                *plugs += 1;
            } else {
                *plugs = 0;
            }
            let stuck = *plugs >= STUCK_POLLS;
            if level.is_none() && !stuck {
                // Python 在 product string 以 Dongle 结尾时跳过读取（关机）；
                // Rust Collection 无 product string，无法等价判断，这里如实不实现。
                return Vec::new();
            }
            if !claim(&mut state.recent, HEADSET_KEY, now, DEDUP_WINDOW) {
                return Vec::new();
            }

            if let Some(lv) = level {
                let since = *state.up_since.get(&key).unwrap_or(&now);
                if lv == 0 && now.saturating_duration_since(since) < Duration::from_secs(ZERO_GRACE_S) {
                    // Rust Status 没有 ui_message/pending 字段；用 level=None 表示“稍后重查”。
                    let mut st = status(HEADSET_KEY, HEADSET_NAME, "headset", "audeze", None, None, false, true);
                    st.source = WAKING_TEXT.into();
                    return vec![st];
                }
                return vec![status(
                    HEADSET_KEY,
                    HEADSET_NAME,
                    "headset",
                    "audeze",
                    Some(lv),
                    Some(is_cable),
                    true,
                    true,
                )];
            }
            if stuck {
                let mut st = status(HEADSET_KEY, HEADSET_NAME, "headset", "audeze", None, None, false, true);
                st.source = STUCK_TEXT.into();
                return vec![st];
            }
            unreachable!("level.is_none() && !stuck 已在上面返回")
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn packet_and_battery_marker_scan() {
            let p = packet(BATTERY_QUERY);
            assert_eq!(p.len(), 62);
            assert_eq!(&p[..9], BATTERY_QUERY);
            assert_eq!(INIT[13], BATTERY_QUERY);
            assert_eq!(INIT.len(), 14);
            assert_eq!(STATUS.len(), 6);

            let mut frame = [0u8; 62];
            frame[10..15].copy_from_slice(&[0xD6, 0x0C, 0x00, 0x00, 75]);
            assert_eq!(find_level(&frame), Some(75));
            frame[14] = 200; // 非法值：继续找下一个标记
            frame[20..25].copy_from_slice(&[0xD6, 0x0C, 0x00, 0x00, 88]);
            assert_eq!(find_level(&frame), Some(88));
            assert_eq!(newest_level(&[vec![0u8; 62], frame.to_vec()]), Some(88));
        }

        #[test]
        fn echo_only_needs_five_blank_replies() {
            let echoes: Vec<Vec<u8>> = (0..5)
                .map(|_| {
                    let mut f = vec![0u8; 62];
                    f[0] = REPORT_ID_IN;
                    f
                })
                .collect();
            assert!(echo_only(&echoes));
            let mut with_marker = echoes.clone();
            with_marker[4][10..15].copy_from_slice(&[0xD6, 0x0C, 0x00, 0x00, 80]);
            assert!(!echo_only(&with_marker));
            assert!(!echo_only(&echoes[..4]));
        }
    }
}

// ===========================================================================
// DJI：VID 0x2CA3 PID 0x4011，状态在 interface 6 的 WinUSB 批量端点 0x86 上，
// 不在 HID 集合上。下面完整移植 DUML 分帧 / CRC-16 X-25 / dialect / 记录解析 /
// 位掩码 + GRACE_PUSHES + STALE 状态机，并给出可测试的 absorb/snapshot 逻辑。
// 阻塞点：Rust Provider 只能拿到 hid::Collection（HID 报告），无法打开 WinUSB
// interface 6 做 bulk IN；在不改 hid.rs / providers.rs 的前提下 poll() 不产出读数。
// ===========================================================================
mod dji {
    #![allow(dead_code)] // VID/PID/端点常量供接入 WinUSB 读取器时使用
    use super::*;

    const VID: u16 = 0x2CA3;
    const PID: u16 = 0x4011;
    const IFACE: u8 = 6;
    const EP_IN: u8 = 0x86;
    const STALE_S: u64 = 8;
    const GRACE_PUSHES: usize = 3;
    // gauge 1..7：1 满电，7 为关机前最后一档；0 未定义。
    const GAUGE_TO_PERCENT: [Option<u8>; 8] = [
        None,
        Some(100),
        Some(90),
        Some(75),
        Some(60),
        Some(35),
        Some(15),
        Some(5),
    ];

    fn crc16(data: &[u8], init: u16) -> u16 {
        let mut crc = init;
        for &b in data {
            crc ^= b as u16;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0x8408
                } else {
                    crc >> 1
                };
            }
        }
        crc
    }

    /// v2 seed 0x3692 -> 整帧 residue 0；v1 seed 0 -> residue 0xBB01。
    fn crc_ok(frame: &[u8]) -> bool {
        crc16(frame, 0x3692) == 0 || crc16(frame, 0x0000) == 0xBB01
    }

    fn split_frames(buf: &[u8]) -> (Vec<Vec<u8>>, Vec<u8>) {
        let mut out = Vec::new();
        let (mut i, n) = (0usize, buf.len());
        while i + 3 < n {
            if buf[i] != 0x55 || buf[i + 2] != 0x04 {
                i += 1;
                continue;
            }
            let ln = buf[i + 1] as usize;
            if ln < 14 {
                i += 1;
                continue;
            }
            if i + ln > n {
                break; // 半帧留到下次
            }
            out.push(buf[i..i + ln].to_vec());
            i += ln;
        }
        (out, buf[i..].to_vec())
    }

    fn dialect(frame: &[u8]) -> &'static str {
        if frame.len() > 11 && frame[9] == 0x5B && frame[10] == 0x03 {
            if frame[11] == 0x03 {
                "v2"
            } else {
                "v1"
            }
        } else {
            "?"
        }
    }

    #[derive(Default, Clone)]
    struct Identity {
        firmware: String,
        serial: String,
        product: String,
    }

    impl Identity {
        fn merge(&mut self, other: Identity) {
            if !other.firmware.is_empty() {
                self.firmware = other.firmware;
            }
            if !other.serial.is_empty() {
                self.serial = other.serial;
            }
            if !other.product.is_empty() {
                self.product = other.product;
            }
        }
    }

    fn trim(s: &[u8]) -> String {
        String::from_utf8_lossy(s)
            .trim_matches(|c| c == '\0' || c == ' ')
            .to_string()
    }

    /// `[tag][unit][reserved:3][len][data]`，unit 0=rx / 1=tx1 / 2=tx2。
    fn identity_records(frame: &[u8]) -> HashMap<String, Identity> {
        let mut units: HashMap<String, Identity> = HashMap::new();
        let mut i = 14usize;
        let end = frame.len().max(14).saturating_sub(2);
        while i + 6 <= end && i + 6 <= frame.len() {
            let (tag, unit, ln) = (frame[i], frame[i + 1], frame[i + 5] as usize);
            if i + 6 + ln > frame.len() {
                break;
            }
            let data = &frame[i + 6..i + 6 + ln];
            let name = match unit {
                0 => "rx",
                1 => "tx1",
                2 => "tx2",
                _ => {
                    i += 6 + ln;
                    continue;
                }
            };
            let info = units.entry(name.to_string()).or_default();
            if tag == 0x01 && data.len() >= 18 {
                info.firmware = format!("{}.{}.{}.{}", data[3], data[2], data[1], data[0]);
                info.serial = trim(&data[4..18]);
            } else if tag == 0x06 {
                info.product = trim(data);
            }
            i += 6 + ln;
        }
        units
    }

    fn record_tags(frame: &[u8]) -> HashSet<u8> {
        let mut tags = HashSet::new();
        let mut i = 14usize;
        let end = frame.len().max(14).saturating_sub(2);
        while i + 6 <= end && i + 6 <= frame.len() {
            let (tag, ln) = (frame[i], frame[i + 5] as usize);
            if i + 6 + ln > frame.len() {
                break;
            }
            tags.insert(tag);
            i += 6 + ln;
        }
        tags
    }

    #[derive(Clone)]
    struct TxSlot {
        unit: u8,
        gauge: u8,
        charging: bool,
        raw: String,
    }

    /// v2 status push：54/86/118 字节，52 起每 32 字节一个发射器 slot。
    fn decode_status(frame: &[u8]) -> Option<Vec<TxSlot>> {
        if dialect(frame) != "v2" || !matches!(frame.len(), 54 | 86 | 118) {
            return None;
        }
        let mut out = Vec::new();
        for i in 0..(frame.len() - 54) / 32 {
            let slot = &frame[52 + 32 * i..52 + 32 * i + 32];
            let unit = slot[1];
            if unit != 1 && unit != 2 {
                continue;
            }
            out.push(TxSlot {
                unit,
                gauge: (slot[7] >> 2) & 0x07,
                charging: slot[7] & 0x02 != 0,
                raw: slot
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            });
        }
        Some(out)
    }

    #[derive(Clone)]
    struct TxState {
        gauge: u8,
        charging: bool,
        raw: String,
        misses: usize,
        gone: bool,
        seen: Option<Instant>,
    }

    impl Default for TxState {
        fn default() -> Self {
            Self {
                gauge: 0,
                charging: false,
                raw: String::new(),
                misses: 0,
                gone: false,
                seen: None,
            }
        }
    }

    #[derive(Default)]
    struct RxState {
        entries: HashMap<String, TxState>,
        info: HashMap<String, Identity>,
        dialect: &'static str,
        bitmask: u8,
        counts: HashMap<String, usize>,
    }

    /// CRC 不通过的帧绝不改变电量；位掩码在 offset 44，连续 GRACE_PUSHES
    /// 次没有自己的位才判为发射器关机。
    fn absorb(state: &mut RxState, frame: &[u8], now: Instant) {
        if !crc_ok(frame) {
            *state.counts.entry("bad-crc".into()).or_insert(0) += 1;
            return;
        }
        let seen_dialect = dialect(frame);
        if seen_dialect != "?" {
            state.dialect = seen_dialect;
        }
        let status = decode_status(frame);
        let kind = if status.is_some() {
            "status"
        } else {
            let tags = record_tags(frame);
            if tags.iter().any(|t| *t == 0x01 || *t == 0x06) {
                "identity"
            } else if tags.contains(&0x05) {
                "level"
            } else if frame.len() == 14 {
                "ack"
            } else {
                "other"
            }
        };
        *state.counts.entry(kind.into()).or_insert(0) += 1;
        if kind == "status" {
            let connected = frame.get(44).copied().unwrap_or(0);
            state.bitmask = connected;
            for slot in status.expect("checked above") {
                let name = format!("tx{}", slot.unit);
                let entry = state.entries.entry(name).or_default();
                entry.gauge = slot.gauge;
                entry.charging = slot.charging;
                entry.raw = slot.raw;
                entry.misses = 0;
                entry.gone = false;
                entry.seen = Some(now);
            }
            for (name, entry) in state.entries.iter_mut() {
                let bit = if name == "tx1" {
                    0x01
                } else if name == "tx2" {
                    0x02
                } else {
                    0
                };
                if bit != 0 && connected & bit != 0 {
                    continue;
                }
                entry.misses += 1;
                if entry.misses >= GRACE_PUSHES {
                    entry.gone = true;
                }
            }
        } else if kind == "identity" {
            for (name, info) in identity_records(frame) {
                state.info.entry(name).or_default().merge(info);
            }
        }
    }

    fn readings(state: &RxState, now: Instant) -> Vec<Status> {
        let mut out = Vec::new();
        for name in ["tx1", "tx2"] {
            let Some(entry) = state.entries.get(name) else {
                continue;
            };
            if entry.gone {
                continue;
            }
            let Some(seen) = entry.seen else {
                continue;
            };
            if now.saturating_duration_since(seen) > Duration::from_secs(STALE_S) {
                continue;
            }
            let info = state.info.get(name);
            let serial = info.map(|i| i.serial.as_str()).unwrap_or("");
            let product = info
                .map(|i| i.product.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("DJI Mic");
            let key = if serial.is_empty() {
                name.to_string()
            } else {
                serial.to_string()
            };
            let mut st = status(
                format!("dji:{key}"),
                format!("{} {}", product, name.to_uppercase()),
                "microphone",
                "dji",
                GAUGE_TO_PERCENT.get(entry.gauge as usize).copied().flatten(),
                Some(entry.charging),
                true,
                true,
            );
            st.approximate = true;
            out.push(st);
        }
        out
    }

    pub struct Dji {
        state: Mutex<RxState>,
    }

    impl Dji {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(RxState::default()),
            }
        }

        /// 供未来接入 WinUSB 批量读取器（或测试注入抓包帧）使用。
        #[allow(dead_code)]
        pub fn inject_frame(&self, frame: &[u8]) {
            absorb(&mut self.state.lock().unwrap(), frame, Instant::now());
        }

        #[allow(dead_code)]
        pub fn snapshot(&self) -> Vec<Status> {
            let state = self.state.lock().unwrap();
            readings(&state, Instant::now())
        }
    }

    impl Default for Dji {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Dji {
        fn name(&self) -> &'static str {
            "dji"
        }

        fn poll(&self, _instance: &str, _collections: &[Collection]) -> Vec<Status> {
            // Python 的接收器在 USB interface 6（class ff/subclass f0）上用 bulk
            // IN 0x86 / OUT 0x06 推送 DUML 帧；HID 上的 ff90/ff82 集合是静默的。
            // Provider 只能拿到 hid::Collection，无法按 USB instance 打开 interface 6
            // 并绑定 WinUSB，因此在两个接口签名不变的前提下这里无法产出读数。
            // 解析与状态机已完整实现并由下面的单测覆盖，注入帧后 snapshot() 可用。
            Vec::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// 按源码布局构造一帧 v2 status push，并用 v2 seed 写出 CRC-16。
        fn status_frame(total: usize, unit: u8, gauge: u8, charging: bool, connected: u8) -> Vec<u8> {
            let mut f = vec![0u8; total];
            f[0] = 0x55;
            f[1] = total as u8;
            f[2] = 0x04;
            f[9] = 0x5B;
            f[10] = 0x03;
            f[11] = 0x03;
            f[44] = connected;
            let slot = &mut f[52..52 + 32];
            slot[1] = unit;
            slot[7] = (gauge << 2) | if charging { 0x02 } else { 0x00 };
            let crc = crc16(&f[..total - 2], 0x3692);
            f[total - 2..].copy_from_slice(&crc.to_le_bytes());
            f
        }

        #[test]
        fn crc_and_duml_framing() {
            let frame = status_frame(86, 1, 1, true, 0x01);
            assert_eq!(frame.len(), 86);
            assert!(crc_ok(&frame));
            assert_eq!(dialect(&frame), "v2");
            let (whole, tail) = split_frames(&[b"junk".to_vec(), frame.clone(), vec![0x55, 86]].concat());
            assert_eq!(whole.len(), 1);
            assert_eq!(whole[0], frame);
            assert_eq!(tail, vec![0x55, 86]);
            let bad = [0x55u8, 14, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            assert!(!crc_ok(&bad));
        }

        #[test]
        fn status_slot_and_gauge() {
            let frame = status_frame(86, 2, 3, false, 0x02);
            let slots = decode_status(&frame).unwrap();
            assert_eq!(slots.len(), 1);
            assert_eq!(slots[0].unit, 2);
            assert_eq!(slots[0].gauge, 3);
            assert!(!slots[0].charging);
            assert_eq!(GAUGE_TO_PERCENT[3], Some(75));

            let now = Instant::now();
            let mut rx = RxState::default();
            absorb(&mut rx, &frame, now);
            assert_eq!(rx.bitmask, 0x02);
            let out = readings(&rx, now);
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].level, Some(75));
            assert!(!out[0].charging.unwrap());
            assert!(out[0].approximate);
        }

        #[test]
        fn grace_pushes_hide_a_dropped_transmitter() {
            let now = Instant::now();
            // 只有 TX1 在位的 118 字节帧（两个 slot）。
            let mut both = status_frame(118, 2, 2, true, 0x03);
            let first = status_frame(118, 1, 1, false, 0x03);
            both[52 + 32..52 + 64].copy_from_slice(&first[52..84]);
            let crc = crc16(&both[..116], 0x3692);
            both[116..].copy_from_slice(&crc.to_le_bytes());
            assert!(crc_ok(&both));
            let one = status_frame(86, 1, 1, false, 0x01);
            let mut rx = RxState::default();
            absorb(&mut rx, &both, now);
            assert_eq!(readings(&rx, now).len(), 2);
            absorb(&mut rx, &one, now);
            assert_eq!(readings(&rx, now).len(), 2);
            absorb(&mut rx, &one, now);
            assert_eq!(readings(&rx, now).len(), 2);
            absorb(&mut rx, &one, now); // 第 3 次才判 TX2 关机
            assert_eq!(readings(&rx, now).len(), 1);
        }
    }
}

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(turtlebeach::TurtleBeach::new()),
        Box::new(ajazz::Ajazz::new()),
        Box::new(lofree::Lofree::new()),
        Box::new(audeze::Audeze::new()),
        // poll() 受限于 HID-only 的 Provider 接口，不产出读数；
        // 协议解析与状态机已实现并有单测，接入 WinUSB 批量端点后即可启用。
        Box::new(dji::Dji::new()),
    ]
}
