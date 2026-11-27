//! 批量移植的简单 HID 电量协议（一请求 + 一回复 + 按字节解析）。
//!
//! 每个 Spec 的 VID/PID、请求字节、集合筛选与解析条件均逐字来自
//! `vendor/halo-battery/upstream/providers/` 下同名 Python 源码；不做猜测。
//! 需要多步握手（plantronics/wraith/fractal/...）、按产品字符串放行
//! （gs3075t/urx/attackshark_x66/redsquare/...）、非 vendor 集合
//! （jbl_quantum350/moondrop）或已被其他 Rust provider 覆盖
//! （beitong/dareu/io_aurora/blackshark2020）的协议不在本文件内。
//!
//! 移植清单见文件末尾 `tests::ported_protocol_names`。

use crate::hid::Collection;
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::time::{Duration, Instant};

const IO_TIMEOUT: Duration = Duration::from_millis(250);

/// 请求/回复的传输方式，与 Python 侧使用的报告类型一一对应。
enum Transport {
    /// output report 写请求，input report 读回复。
    WriteRead,
    /// feature report 写请求，再用同一（或指定）集合的 get_feature 读回复。
    FeatureRead { id: u8 },
    /// feature report 写请求，input report 读回复（命令与回复在不同集合）。
    FeatureInput,
}

type ParseFn = fn(&[u8], u16) -> Option<Status>;

struct Spec {
    /// Provider 身份，传给 Engine 参与 key 计算。
    name: &'static str,
    vids: &'static [u16],
    pids: &'static [u16],
    /// 写请求所在集合；0 表示任意 vendor page，0xFFFF 表示任意 usage。
    page: u16,
    usage: u16,
    iface: Option<u8>,
    /// 回复集合；0 表示与请求集合相同。
    reply_page: u16,
    reply_usage: u16,
    reply_iface: Option<u8>,
    transport: Transport,
    request: &'static [u8],
    parse: ParseFn,
}

struct MiscProvider(&'static Spec);

impl Provider for MiscProvider {
    fn name(&self) -> &'static str {
        self.0.name
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let spec = self.0;
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if !spec.vids.contains(&vid) || !spec.pids.contains(&pid) {
            return Vec::new();
        }

        for col in collections.iter().filter(|c| matches_write(spec, c)) {
            let Some(mut handle) = col.open() else { continue };
            let write_ok = match spec.transport {
                Transport::WriteRead | Transport::FeatureInput => {
                    handle.write(spec.request, Instant::now() + IO_TIMEOUT)
                }
                Transport::FeatureRead { .. } => {
                    handle.set_feature(spec.request, Instant::now() + IO_TIMEOUT)
                }
            };
            if !write_ok {
                continue;
            }

            let frame = match spec.transport {
                Transport::WriteRead => handle.read(Instant::now() + IO_TIMEOUT),
                Transport::FeatureRead { id } => {
                    handle.get_feature(id, Instant::now() + IO_TIMEOUT)
                }
                Transport::FeatureInput => {
                    let reply = if spec.reply_page == 0 {
                        Some(col)
                    } else {
                        collections.iter().find(|c| matches_reply(spec, c))
                    };
                    let Some(reply) = reply else { continue };
                    if std::ptr::eq(reply, col) {
                        handle.read(Instant::now() + IO_TIMEOUT)
                    } else {
                        let Some(mut reader) = reply.open() else { continue };
                        reader.read(Instant::now() + IO_TIMEOUT)
                    }
                }
            };
            let Some(frame) = frame else { continue };
            if let Some(status) = (spec.parse)(&frame, pid) {
                return vec![status];
            }
        }
        Vec::new()
    }
}

fn matches_write(spec: &Spec, col: &Collection) -> bool {
    if col.usage_page < 0xff00 {
        return false;
    }
    if spec.page != 0 && col.usage_page != spec.page {
        return false;
    }
    if spec.usage != 0xffff && col.usage != spec.usage {
        return false;
    }
    spec.iface.is_none_or(|i| col.interface == Some(i))
}

fn matches_reply(spec: &Spec, col: &Collection) -> bool {
    if col.usage_page != spec.reply_page || col.usage != spec.reply_usage {
        return false;
    }
    spec.reply_iface.is_none_or(|i| col.interface == Some(i))
}

const fn frame<const N: usize>(prefix: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut i = 0;
    while i < prefix.len() && i < N {
        out[i] = prefix[i];
        i += 1;
    }
    out
}

fn status(
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

fn sum8(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

// ---- 请求常量（字节严格来自 Python 源码）----

// arbiter.py: 65 字节 feature report，req[3]=02 req[4]=02 req[6]=83
const ARBITER_REQUEST: [u8; 65] = frame(&[0, 0, 0, 0x02, 0x02, 0, 0x83]);
// areson.py: 17 字节，checksum = 0x55 - sum(bytes 0..15) = 0x49
const ARESON_REQUEST: [u8; 17] =
    frame(&[0x08, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x49]);
// cherry.py: report 04, type 1A, payload 03 00 00 55, LE16 checksum of bytes 3.. = 0x0072
const CHERRY_REQUEST: [u8; 64] = frame(&[0x04, 0x72, 0x00, 0x1a, 0x03, 0, 0, 0x55]);
// coolermaster.py: 00 46 81 00（hid 写时按 output_len 补零）
const COOLERMASTER_REQUEST: [u8; 4] = [0x00, 0x46, 0x81, 0x00];
// astro.py: 02 0c 03 00 06 0c
const ASTRO_REQUEST: [u8; 64] = frame(&[0x02, 0x0c, 0x03, 0x00, 0x06, 0x0c]);
// jbl_hidv3.py: 88 00 dd 01 00 01 00 02 00 0d 00
const JBL_REQUEST: [u8; 11] = frame(&[0x88, 0x00, 0xdd, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x0d, 0x00]);
// royal_kludge.py: 00 01 01 02 01（65 字节报表）
const ROYAL_REQUEST: [u8; 65] = frame(&[0x00, 0x01, 0x01, 0x02, 0x01]);
// alienware.py: 00 | 0e a1 00 ...
const ALIENWARE_REQUEST: [u8; 65] = frame(&[0x00, 0x0e, 0xa1]);
// am_infinity.py: 00 f7 00 ...（65 / 67 两种长度都在源码里尝试）
const AM_INFINITY_REQUEST_65: [u8; 65] = frame(&[0x00, 0xf7]);
const AM_INFINITY_REQUEST_67: [u8; 67] = frame(&[0x00, 0xf7]);
// attackshark_r5.py: 00 | 02 02 00 83
const R5_REQUEST: [u8; 65] = frame(&[0x00, 0x02, 0x02, 0x00, 0x83]);
// eweadn.py: 00 30 00 ... 00（末字节 sum(f[5:32])，全零）
const EWEADN_REQUEST: [u8; 33] = frame(&[0x00, 0x30]);
// monsgeek.py: 00 f7 00 ...；X820（5006）在 payload[7] 带 bit7 校验 0x08
const MONGEEK_REQUEST: [u8; 65] = frame(&[0x00, 0xf7]);
const MONGEEK_REQUEST_CKSUM: [u8; 65] = frame(&[0x00, 0xf7, 0, 0, 0, 0, 0, 0, 0x08]);
// ninjutso_sprime.py: 05 15 00 00 01（32 字节 feature）
const NINJUTSO_REQUEST: [u8; 32] = frame(&[0x05, 0x15, 0x00, 0x00, 0x01]);
// raikiri.py: b0 12 04（64 字节 output report）
const RAIKIRI_REQUEST: [u8; 64] = frame(&[0xb0, 0x12, 0x04]);
// royuan.py: 00 | f7 ... payload[7] = 0xff - 0xf7 = 0x08
const ROYUAN_REQUEST: [u8; 65] = frame(&[0x00, 0xf7, 0, 0, 0, 0, 0, 0, 0x08]);
// sony_h9ii.py: zoneout 的 64 字节 audio status query
const SONY_REQUEST: [u8; 64] = frame(&[
    0x02, 0x0c, 0x01, 0x00, 0xfc, 0x08, 0x96, 0xc3, 0x41, 0x06, 0x01, 0x01, 0x00, 0xa2,
]);

/// darmoshark.py `make_query`：末字节使整帧和为 0xA1。
const fn darmoshark_request(wireless: bool) -> [u8; 64] {
    let mut f = [0u8; 64];
    f[0] = if wireless { 0x01 | 0x40 } else { 0x01 };
    f[2] = 129;
    f[3] = 1;
    let mut sum = 0u16;
    let mut i = 0;
    while i < 63 {
        sum += f[i] as u16;
        i += 1;
    }
    f[63] = (0xa1u16.wrapping_sub(sum) & 0xff) as u8;
    f
}

const DARMO_WIRELESS: [u8; 64] = darmoshark_request(true);
const DARMO_WIRED: [u8; 64] = darmoshark_request(false);

// ---- 解析函数（严格照搬 Python 的索引与校验）----

fn arbiter_parse(f: &[u8], pid: u16) -> Option<Status> {
    let (key, name, receiver) = match pid {
        0x0009 => ("arbiter:000a", "Arbiter Studio Akitsu", false),
        0x000a => ("arbiter:000a", "Arbiter Studio Akitsu", true),
        0x0038 => ("arbiter:0039", "Arbiter Studio Akitsu 3950", false),
        0x0039 => ("arbiter:0039", "Arbiter Studio Akitsu 3950", true),
        0x0066 => ("arbiter:0067", "Arbiter Studio Akitsu Medium", false),
        0x0067 => ("arbiter:0067", "Arbiter Studio Akitsu Medium", true),
        _ => return None,
    };
    let (state, chg, level) = if f.len() >= 9 && f[4] == 0x02 && f[6] == 0x83 {
        (f[1], f[7], f[8])
    } else if f.len() >= 8 && f[3] == 0x02 && f[5] == 0x83 {
        (f[0], f[6], f[7])
    } else {
        return None;
    };
    let mut st = status(key, name, "mouse", "arbiter", None, Some(false), true);
    if state == 0xa0 {
        st.online = false;
        return Some(st);
    }
    if !matches!(state, 0xa1 | 0xa2) || level > 100 {
        return None;
    }
    let connected = level > 0 || !receiver;
    st.level = connected.then_some(level);
    st.charging = Some(chg == 1);
    st.online = connected;
    Some(st)
}

fn areson_parse(f: &[u8], pid: u16) -> Option<Status> {
    if f.len() < 17 || sum8(&f[..17]) != 0x55 {
        return None;
    }
    if f[0] != 0x09 || f[1] != 0x04 || f[2] != 0x00 || f[6] > 10 {
        return None;
    }
    Some(status(
        format!("areson:{pid:04x}"),
        "Redragon wireless mouse",
        "mouse",
        "areson",
        Some(f[6] * 10),
        Some(f[7] != 0),
        true,
    ))
}

fn cherry_parse(f: &[u8], _pid: u16) -> Option<Status> {
    if f.len() < 10 || f[0] != 0x04 || f[3] != 0x1a || f[8] > 100 {
        return None;
    }
    Some(status(
        "cherry:mx30s",
        "CHERRY MX 3.0S Wireless",
        "keyboard",
        "cherry",
        Some(f[8]),
        Some(f[9] != 0),
        true,
    ))
}

fn coolermaster_parse(f: &[u8], pid: u16) -> Option<Status> {
    if f.len() < 13 || f[0] != 0x47 || f[1] != 0x81 || !(1..=100).contains(&f[10]) {
        return None;
    }
    let model = f[2];
    let (key, name) = if pid == 0x0169 || (pid == 0x01b7 && model == 0x50) {
        ("coolermaster:mm712", "Cooler Master MM712")
    } else if pid == 0x0167 || (pid == 0x01b7 && model == 0x70) {
        ("coolermaster:mm731", "Cooler Master MM731")
    } else {
        return Some(status(
            format!("coolermaster:{pid:04x}"),
            "Cooler Master wireless mouse",
            "mouse",
            "coolermaster",
            Some(f[10]),
            Some(f[11] != 0),
            true,
        ));
    };
    Some(status(
        key,
        name,
        "mouse",
        "coolermaster",
        Some(f[10]),
        Some(f[11] != 0),
        true,
    ))
}

fn darmoshark_parse(f: &[u8], pid: u16) -> Option<Status> {
    if f.len() < 12 || f[0] & 0x3f != 0x01 || f[3] != 0x01 {
        return None;
    }
    let level = if f[6] == 0x15 && f[7] == 0x19 { f[11] } else { f[6] };
    if level > 100 {
        return None;
    }
    let name = if pid == 0x0723 {
        "Darmoshark N5"
    } else {
        "Darmoshark N5 (USB cable)"
    };
    Some(status("darmoshark:n5", name, "mouse", "darmoshark", Some(level), None, false))
}

fn astro_parse(f: &[u8], _pid: u16) -> Option<Status> {
    if f.len() <= 6 || f[0] != 0x02 || f[1] != 0x0c || f[4] != 0x06 || f[6] > 100 {
        return None;
    }
    // Python: parse_dock 在帧过短时返回 None，调用处 `or False`。
    let charging = f.len() > 8 && f[8] != 0;
    Some(status(
        "astro:0b1c",
        "Astro A50 Gen 5",
        "headset",
        "astro",
        Some(f[6]),
        Some(charging),
        true,
    ))
}

fn jbl_hidv3_parse(f: &[u8], pid: u16) -> Option<Status> {
    let (key, name) = match pid {
        0x2125 | 0x2126 => ("jbl:650", "JBL Quantum 650 Wireless"),
        0x213b | 0x213d => ("jbl:650x", "JBL Quantum 650X Wireless"),
        0x2127 | 0x2128 => ("jbl:950", "JBL Quantum 950 Wireless"),
        0x213e | 0x2140 => ("jbl:950x", "JBL Quantum 950X Wireless"),
        0x218d | 0x218e => ("jbl:520", "JBL Quantum 520 Wireless"),
        0x21a7 | 0x21a9 => ("jbl:520x", "JBL Quantum 520X Wireless"),
        0x219a | 0x219b => ("jbl:850", "JBL Quantum 850 Wireless"),
        _ => return None,
    };
    if f.len() < 16 || f[0] != 0x86 || f[1..6] != [0x00, 0xdd, 0x01, 0x00, 0x01] {
        return None;
    }
    if f[10] & 0x80 != 0 || f[11] != 0x0d || f[12] != 0x00 || (f[13] == 0 && f[14] == 0) {
        return None;
    }
    let level = f[15] & 0x7f;
    if level > 100 {
        return None;
    }
    Some(status(key, name, "headset", "jbl", Some(level), Some(f[15] & 0x80 != 0), true))
}

fn royal_kludge_parse(f: &[u8], pid: u16) -> Option<Status> {
    if f.len() < 11 || f[0] & 0x1f != 0x01 || f[1] & 0x0f != 0x01 || f[2] != 0x02 {
        return None;
    }
    if (f[3] >> 4) & 1 == 0 {
        return None;
    }
    let pct = u16::from(f[6]) | (u16::from(f[7]) << 8);
    if pct > 100 {
        return None;
    }
    let cable = matches!(pid, 0x00a5 | 0x00e5);
    let charging = f[10] != 0 || cable;
    Some(status(
        "rk:mg6",
        "RK MG6 Pro",
        "mouse",
        "royal_kludge",
        Some(pct as u8),
        Some(charging),
        true,
    ))
}

fn alienware_parse(f: &[u8], _pid: u16) -> Option<Status> {
    let start = [0usize, 1].into_iter().find(|&i| {
        f.len() > i + 2 && f[i] == 0x0e && f[i + 1] == 0xa1
    })?;
    let level = f[start + 2];
    if !(1..=100).contains(&level) {
        return None;
    }
    let state = f.get(start + 3).copied().unwrap_or(0);
    Some(status(
        "alienware:211d",
        "Alienware Pro Wireless Gaming Keyboard",
        "keyboard",
        "alienware",
        Some(level),
        Some(state == 0x04),
        true,
    ))
}

fn am_infinity_parse(f: &[u8], _pid: u16) -> Option<Status> {
    if f.len() < 4 {
        return None;
    }
    let charge_index = usize::from(f[0] == 0x05) + 2;
    if f[1..charge_index].iter().any(|&b| b != 0) {
        return None;
    }
    let charge = f[charge_index];
    if !(1..=100).contains(&charge) {
        return None;
    }
    Some(status(
        "am_infinity",
        "AM Infinity 8K Mouse",
        "mouse",
        "am_infinity",
        Some(charge),
        None,
        false,
    ))
}

fn attackshark_r5_parse(f: &[u8], _pid: u16) -> Option<Status> {
    if f.len() < 9 {
        return None;
    }
    let pair = if f[0] == 0xa1 && f[3] == 0x02 && f[5] == 0x83 {
        (f[6], f[7])
    } else if f[1] == 0xa1 && f[4] == 0x02 && f[6] == 0x83 {
        (f[7], f[8])
    } else {
        return None;
    };
    let (level, charging) = if matches!(pair.0, 0 | 1) && pair.1 <= 100 {
        (pair.1, pair.0 == 1)
    } else if matches!(pair.1, 0 | 1) && pair.0 <= 100 {
        (pair.0, pair.1 == 1)
    } else {
        return None;
    };
    Some(status(
        "attackshark:0047",
        "Attack Shark R5 Ultra",
        "mouse",
        "attackshark_r5",
        Some(level),
        Some(charging),
        true,
    ))
}

fn eweadn_parse(f: &[u8], _pid: u16) -> Option<Status> {
    if f.len() < 15 || f[0] != 0x30 || f[1] != 0x00 || f[13] > 100 {
        return None;
    }
    let dev_id: String = f[4..8]
        .iter()
        .copied()
        .filter(|b| (0x20..0x7f).contains(b))
        .map(char::from)
        .collect::<String>()
        .trim()
        .to_string();
    let name = match dev_id.as_str() {
        "M156" => "EWEADN S9",
        "M302" => "EWEADN H2",
        _ => "EWEADN mouse",
    };
    Some(status(
        "eweadn",
        name,
        "mouse",
        "eweadn",
        Some(f[13]),
        Some(f[12] != 0),
        true,
    ))
}

fn monsgeek_parse(f: &[u8], pid: u16) -> Option<Status> {
    if f.len() < 9 {
        return None;
    }
    let kind = f[7];
    let level = f[2];
    if !matches!(kind, 1 | 3) || f[4] != 0 || level > 100 {
        return None;
    }
    let (key, name) = match pid {
        0x5026 => ("monsgeek:5026", "MonsGeek FUN60 Pro"),
        0x5006 => ("monsgeek:5006", "Attack Shark X820 Ultra"),
        _ => return None,
    };
    Some(status(
        key,
        name,
        "keyboard",
        "monsgeek",
        Some(level),
        None,
        false,
    ))
}

fn ninjutso_sprime_parse(f: &[u8], pid: u16) -> Option<Status> {
    let (key, name, cable, allow_full) = match pid {
        0xae1c => ("ninjutso:sora_v2", "Ninjutso Sora V2", false, false),
        0xae11 => ("ninjutso:sora_v2", "Ninjutso Sora V2", true, false),
        0xac11 | 0xac12 | 0xac13 | 0xac1c | 0xad8c => ("sprime:pm1", "Sprime PM1", matches!(pid, 0xac11 | 0xac12 | 0xac13), true),
        _ => return None,
    };
    if f.len() < 13 || f[0] != 0x05 || f[1] != 0x15 || f[9] > 100 {
        return None;
    }
    let connected = cable || f[12] != 0;
    let mut level = if f[9] == 0 { 100 } else { f[9] };
    let mut charging = connected && f[10] != 0;
    if allow_full && f[11] != 0 {
        level = 100;
        charging = false;
    }
    Some(status(
        key,
        name,
        "mouse",
        "ninjutso_sprime",
        connected.then_some(level),
        Some(charging),
        true,
    ))
}

fn raikiri_parse(f: &[u8], _pid: u16) -> Option<Status> {
    if f.len() <= 10 || f[0] != 0xb0 || f[1] != 0x12 || f[2] != 0x04 || f[5] > 100 {
        return None;
    }
    Some(status(
        "raikiri:1c92",
        "ROG Raikiri II",
        "gamepad",
        "raikiri",
        Some(f[5]),
        Some(f[10] != 0x00),
        true,
    ))
}

fn royuan_parse(f: &[u8], _pid: u16) -> Option<Status> {
    let frame = if f.len() >= 65 && f[0] == 0x00 {
        &f[1..65]
    } else if f.len() >= 64 {
        &f[..64]
    } else {
        return None;
    };
    if !matches!(frame[6], 1 | 3) || !(1..=100).contains(&frame[1]) {
        return None;
    }
    Some(status(
        "royuan:4011",
        "ROYUAN 2.4G keyboard",
        "keyboard",
        "royuan",
        Some(frame[1]),
        None,
        false,
    ))
}

fn sony_h9ii_parse(f: &[u8], _pid: u16) -> Option<Status> {
    if f.len() != 64 || f[0] != 0x02 || f[9] != 0x06 || f == SONY_REQUEST.as_slice() {
        return None;
    }
    let level = f[15];
    if level > 100 {
        return None;
    }
    let charge = f[14];
    Some(status(
        "sony_h9ii:0fa8",
        "Sony INZONE H9 II",
        "headset",
        "sony_h9ii",
        Some(level),
        Some(charge == 1),
        matches!(charge, 0 | 1),
    ))
}

// ---- 注册表 ----

macro_rules! spec {
    ($name:expr, $vids:expr, $pids:expr, $page:expr, $usage:expr, $iface:expr,
     $transport:expr, $request:expr, $parse:expr) => {
        Spec {
            name: $name, vids: $vids, pids: $pids, page: $page, usage: $usage, iface: $iface,
            reply_page: 0, reply_usage: 0xffff, reply_iface: None,
            transport: $transport, request: $request, parse: $parse,
        }
    };
    ($name:expr, $vids:expr, $pids:expr, $page:expr, $usage:expr, $iface:expr,
     $reply_page:expr, $reply_usage:expr, $reply_iface:expr,
     $transport:expr, $request:expr, $parse:expr) => {
        Spec {
            name: $name, vids: $vids, pids: $pids, page: $page, usage: $usage, iface: $iface,
            reply_page: $reply_page, reply_usage: $reply_usage, reply_iface: $reply_iface,
            transport: $transport, request: $request, parse: $parse,
        }
    };
}

static SPECS: &[Spec] = &[
    // arbiter.py：MODELS 中 6 个 PID，接收器/有线共用同一个 key。
    spec! { "arbiter", &[0x373e], &[0x0009], 0xffff, 0x0000, None,
            Transport::FeatureRead { id: 0 }, &ARBITER_REQUEST, arbiter_parse },
    spec! { "arbiter", &[0x373e], &[0x000a], 0xffff, 0x0000, None,
            Transport::FeatureRead { id: 0 }, &ARBITER_REQUEST, arbiter_parse },
    spec! { "arbiter", &[0x373e], &[0x0038], 0xffff, 0x0000, None,
            Transport::FeatureRead { id: 0 }, &ARBITER_REQUEST, arbiter_parse },
    spec! { "arbiter", &[0x373e], &[0x0039], 0xffff, 0x0000, None,
            Transport::FeatureRead { id: 0 }, &ARBITER_REQUEST, arbiter_parse },
    spec! { "arbiter", &[0x373e], &[0x0066], 0xffff, 0x0000, None,
            Transport::FeatureRead { id: 0 }, &ARBITER_REQUEST, arbiter_parse },
    spec! { "arbiter", &[0x373e], &[0x0067], 0xffff, 0x0000, None,
            Transport::FeatureRead { id: 0 }, &ARBITER_REQUEST, arbiter_parse },

    // areson.py：feature 写在 ff02:0002，input 回复在 ff01:0000。
    spec! { "areson", &[0x25a7], &[0xfa07, 0xfa08], 0xff02, 0x0002, Some(1),
            0xff01, 0x0000, Some(1), Transport::FeatureInput, &ARESON_REQUEST, areson_parse },

    // cherry.py
    spec! { "cherry", &[0x046a], &[0x00e3, 0x00ea], 0xff1c, 0x0092, None,
            Transport::WriteRead, &CHERRY_REQUEST, cherry_parse },

    // coolermaster.py：dongle 按 reply 的 model 字节区分，两根线按 PID 区分。
    spec! { "coolermaster", &[0x2516], &[0x01b7], 0xff0a, 0xffff, Some(3),
            Transport::WriteRead, &COOLERMASTER_REQUEST, coolermaster_parse },
    spec! { "coolermaster", &[0x2516], &[0x0169, 0x0167], 0xff0a, 0xffff, Some(3),
            Transport::WriteRead, &COOLERMASTER_REQUEST, coolermaster_parse },

    // darmoshark.py：无线接收器请求带 0x40 flag。
    spec! { "darmoshark", &[0x1915], &[0x0723], 0xff0a, 0x0001, None,
            Transport::WriteRead, &DARMO_WIRELESS, darmoshark_parse },
    spec! { "darmoshark", &[0x1915], &[0x073c], 0xff0a, 0x0001, None,
            Transport::WriteRead, &DARMO_WIRED, darmoshark_parse },

    // astro.py
    spec! { "astro", &[0x046d], &[0x0b1c], 0xff32, 0x0074, None,
            Transport::WriteRead, &ASTRO_REQUEST, astro_parse },

    // jbl_hidv3.py
    spec! { "jbl", &[0x0ecb], &[0x2125, 0x2126], 0xff12, 0xffff, None,
            Transport::FeatureInput, &JBL_REQUEST, jbl_hidv3_parse },
    spec! { "jbl", &[0x0ecb], &[0x213b, 0x213d], 0xff12, 0xffff, None,
            Transport::FeatureInput, &JBL_REQUEST, jbl_hidv3_parse },
    spec! { "jbl", &[0x0ecb], &[0x2127, 0x2128], 0xff12, 0xffff, None,
            Transport::FeatureInput, &JBL_REQUEST, jbl_hidv3_parse },
    spec! { "jbl", &[0x0ecb], &[0x213e, 0x2140], 0xff12, 0xffff, None,
            Transport::FeatureInput, &JBL_REQUEST, jbl_hidv3_parse },
    spec! { "jbl", &[0x0ecb], &[0x218d, 0x218e], 0xff12, 0xffff, None,
            Transport::FeatureInput, &JBL_REQUEST, jbl_hidv3_parse },
    spec! { "jbl", &[0x0ecb], &[0x21a7, 0x21a9], 0xff12, 0xffff, None,
            Transport::FeatureInput, &JBL_REQUEST, jbl_hidv3_parse },
    spec! { "jbl", &[0x0ecb], &[0x219a, 0x219b], 0xff12, 0xffff, None,
            Transport::FeatureInput, &JBL_REQUEST, jbl_hidv3_parse },

    // royal_kludge.py：接收器 ff00:0001，有线 fff2:0001。
    spec! { "royal_kludge", &[0x2e3c], &[0x00a6, 0x00e6], 0xff00, 0x0001, None,
            Transport::WriteRead, &ROYAL_REQUEST, royal_kludge_parse },
    spec! { "royal_kludge", &[0x2e3c], &[0x00a5, 0x00e5], 0xfff2, 0x0001, None,
            Transport::WriteRead, &ROYAL_REQUEST, royal_kludge_parse },

    // alienware.py
    spec! { "alienware", &[0x413c], &[0x211d], 0xff00, 0x0001, Some(1),
            Transport::FeatureInput, &ALIENWARE_REQUEST, alienware_parse },

    // am_infinity.py：源码同时尝试 65 和 67 字节的 poll。
    spec! { "am_infinity", &[0x3151], &[0x5007], 0xffff, 0x0002, None,
            Transport::FeatureRead { id: 0x05 }, &AM_INFINITY_REQUEST_65, am_infinity_parse },
    spec! { "am_infinity", &[0x3151], &[0x5007], 0xffff, 0x0002, None,
            Transport::FeatureRead { id: 0x05 }, &AM_INFINITY_REQUEST_67, am_infinity_parse },

    // attackshark_r5.py：源码遍历该 PID 的全部 vendor 集合。
    spec! { "attackshark_r5", &[0x373e], &[0x0047], 0x0000, 0xffff, None,
            Transport::FeatureRead { id: 0 }, &R5_REQUEST, attackshark_r5_parse },

    // eweadn.py
    spec! { "eweadn", &[0x089d], &[0x062f], 0xff01, 0x0010, None,
            Transport::WriteRead, &EWEADN_REQUEST, eweadn_parse },

    // monsgeek.py（含 ROYUAN X820 的 bit7 校验变体）
    spec! { "monsgeek", &[0x3151], &[0x5026], 0xffff, 0x0002, None,
            Transport::FeatureRead { id: 0 }, &MONGEEK_REQUEST, monsgeek_parse },
    spec! { "monsgeek", &[0x3151], &[0x5006], 0xffff, 0x0002, None,
            Transport::FeatureRead { id: 0 }, &MONGEEK_REQUEST_CKSUM, monsgeek_parse },

    // ninjutso_sprime.py
    spec! { "ninjutso_sprime", &[0x1915], &[0xae1c, 0xae11], 0xffa0, 0x0001, None,
            Transport::FeatureRead { id: 0x05 }, &NINJUTSO_REQUEST, ninjutso_sprime_parse },
    spec! { "ninjutso_sprime", &[0x1915], &[0xac11, 0xac12, 0xac13, 0xac1c, 0xad8c],
            0xffa0, 0x0001, None,
            Transport::FeatureRead { id: 0x05 }, &NINJUTSO_REQUEST, ninjutso_sprime_parse },

    // raikiri.py
    spec! { "raikiri", &[0x0b05], &[0x1c92], 0xff03, 0x0001, Some(4),
            Transport::WriteRead, &RAIKIRI_REQUEST, raikiri_parse },

    // royuan.py
    spec! { "royuan", &[0x3151], &[0x4011], 0xffff, 0x0002, None,
            Transport::FeatureRead { id: 0 }, &ROYUAN_REQUEST, royuan_parse },

    // sony_h9ii.py
    spec! { "sony_h9ii", &[0x054c], &[0x0fa8], 0xff04, 0x0001, Some(5),
            Transport::WriteRead, &SONY_REQUEST, sony_h9ii_parse },
];

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    SPECS.iter().map(|s| Box::new(MiscProvider(s)) as Box<dyn Provider>).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(parse: ParseFn, frame: &[u8], pid: u16) -> Status {
        parse(frame, pid).unwrap_or_else(|| panic!("frame rejected: {frame:02x?}"))
    }

    #[test]
    fn arbiter_parser() {
        // 布局 1：b[4]=02 b[6]=83，status=b[1]
        let mut f = [0u8; 9];
        f[1] = 0xa1; f[4] = 0x02; f[6] = 0x83; f[7] = 1; f[8] = 76;
        let s = st(arbiter_parse, &f, 0x000a);
        assert_eq!(s.level, Some(76));
        assert_eq!(s.charging, Some(true));
        assert_eq!(s.key, "arbiter:000a");
        // 布局 2：b[3]=02 b[5]=83，status=b[0]
        let mut g = [0u8; 8];
        g[0] = 0xa2; g[3] = 0x02; g[5] = 0x83; g[6] = 0; g[7] = 55;
        let s = st(arbiter_parse, &g, 0x0039);
        assert_eq!(s.level, Some(55));
        assert_eq!(s.charging, Some(false));
        // A0 鼠标睡眠：仍产出离线状态
        f[1] = 0xa0;
        let s = st(arbiter_parse, &f, 0x000a);
        assert_eq!(s.level, None);
        assert!(!s.online);
        assert_eq!(ARBITER_REQUEST[3..7], [0x02, 0x02, 0, 0x83]);
    }

    #[test]
    fn areson_parser() {
        let mut f = [0u8; 17];
        f[..3].copy_from_slice(&[0x09, 0x04, 0x00]);
        f[6] = 7; f[7] = 1;
        f[16] = (0x55u8).wrapping_sub(sum8(&f[..16]));
        let s = st(areson_parse, &f, 0xfa07);
        assert_eq!(s.level, Some(70));
        assert_eq!(s.charging, Some(true));
        assert_eq!(ARESON_REQUEST[0], 0x08);
        f[16] ^= 1;
        assert!(areson_parse(&f, 0xfa07).is_none());
    }

    #[test]
    fn cherry_parser() {
        let mut f = [0u8; 10];
        f[0] = 0x04; f[3] = 0x1a; f[8] = 76; f[9] = 1;
        let s = st(cherry_parse, &f, 0x00ea);
        assert_eq!(s.level, Some(76));
        assert_eq!(s.charging, Some(true));
        f[8] = 101;
        assert!(cherry_parse(&f, 0x00ea).is_none());
        assert_eq!(CHERRY_REQUEST[..8], [0x04, 0x72, 0x00, 0x1a, 0x03, 0, 0, 0x55]);
    }

    #[test]
    fn coolermaster_parser() {
        let mut f = [0u8; 13];
        f[..2].copy_from_slice(&[0x47, 0x81]);
        f[2] = 0x50; f[10] = 76; f[11] = 1; f[12] = 0;
        let s = st(coolermaster_parse, &f, 0x01b7);
        assert_eq!(s.level, Some(76));
        assert_eq!(s.charging, Some(true));
        assert_eq!(s.key, "coolermaster:mm712");
        f[2] = 0x70;
        assert_eq!(st(coolermaster_parse, &f, 0x01b7).key, "coolermaster:mm731");
        assert_eq!(COOLERMASTER_REQUEST, [0x00, 0x46, 0x81, 0x00]);
    }

    #[test]
    fn darmoshark_parser() {
        let mut f = [0u8; 12];
        f[0] = 0x01; f[3] = 0x01; f[6] = 63;
        let s = st(darmoshark_parse, &f, 0x0723);
        assert_eq!(s.level, Some(63));
        assert_eq!(s.charging_known, false);
        // v2 布局：6..8 = 15 19，电量在 11
        f[6] = 0x15; f[7] = 0x19; f[11] = 88;
        assert_eq!(st(darmoshark_parse, &f, 0x073c).level, Some(88));
        assert_eq!(sum8(&DARMO_WIRELESS), 0xa1);
        assert_eq!(sum8(&DARMO_WIRED), 0xa1);
        assert_eq!(DARMO_WIRELESS[0], 0x41);
    }

    #[test]
    fn astro_parser() {
        let mut f = [0u8; 9];
        f[..6].copy_from_slice(&[0x02, 0x0c, 0, 0, 0x06, 0]);
        f[6] = 76; f[8] = 1;
        let s = st(astro_parse, &f, 0x0b1c);
        assert_eq!(s.level, Some(76));
        assert_eq!(s.charging, Some(true));
        f[8] = 0;
        assert_eq!(st(astro_parse, &f, 0x0b1c).charging, Some(false));
    }

    #[test]
    fn jbl_hidv3_parser() {
        let mut f = [0u8; 16];
        f[..6].copy_from_slice(&[0x86, 0, 0xdd, 1, 0, 1]);
        f[11] = 0x0d; f[13] = 1;
        f[15] = 0x55;
        let s = st(jbl_hidv3_parse, &f, 0x2125);
        assert_eq!(s.level, Some(85));
        assert_eq!(s.charging, Some(false));
        assert_eq!(s.key, "jbl:650");
        f[15] = 0xd2; // bit7 charging, 0x52 = 82
        let s = st(jbl_hidv3_parse, &f, 0x2126);
        assert_eq!(s.level, Some(82));
        assert_eq!(s.charging, Some(true));
        f[10] = 0x80; // error bit
        assert!(jbl_hidv3_parse(&f, 0x2125).is_none());
        assert_eq!(JBL_REQUEST, [0x88, 0, 0xdd, 1, 0, 1, 0, 2, 0, 0x0d, 0]);
    }

    #[test]
    fn royal_kludge_parser() {
        let mut f = [0u8; 11];
        f[..3].copy_from_slice(&[0x01, 0x01, 0x02]);
        f[3] = 0x10; f[6] = 76; f[10] = 0;
        let s = st(royal_kludge_parse, &f, 0x00a6);
        assert_eq!(s.level, Some(76));
        assert_eq!(s.charging, Some(false));
        f[10] = 1;
        assert_eq!(st(royal_kludge_parse, &f, 0x00a6).charging, Some(true));
        // 有线 PID 即使 plugged=0 也算充电
        f[10] = 0;
        assert_eq!(st(royal_kludge_parse, &f, 0x00a5).charging, Some(true));
        f[3] = 0x00; // access_res 为 0：接收器替鼠标回答，不可信
        assert!(royal_kludge_parse(&f, 0x00a6).is_none());
    }

    #[test]
    fn alienware_parser() {
        let f = [0x0e, 0xa1, 25, 0x04];
        let s = st(alienware_parse, &f, 0x211d);
        assert_eq!(s.level, Some(25));
        assert_eq!(s.charging, Some(true));
        let g = [0x00, 0x0e, 0xa1, 37, 0x00];
        let s = st(alienware_parse, &g, 0x211d);
        assert_eq!(s.level, Some(37));
        assert_eq!(s.charging, Some(false));
    }

    #[test]
    fn am_infinity_parser() {
        let f = [0x05, 0x00, 0x00, 100];
        let s = st(am_infinity_parse, &f, 0x5007);
        assert_eq!(s.level, Some(100));
        assert_eq!(s.charging_known, false);
        let g = [0x00, 0x00, 64, 0];
        assert_eq!(st(am_infinity_parse, &g, 0x5007).level, Some(64));
        let bad = [0x05, 0x01, 0x00, 64]; // 前缀非 0：重连垃圾帧
        assert!(am_infinity_parse(&bad, 0x5007).is_none());
        assert_eq!(AM_INFINITY_REQUEST_65[..2], [0x00, 0xf7]);
    }

    #[test]
    fn attackshark_r5_parser() {
        let mut f = [0u8; 9];
        f[0] = 0xa1; f[3] = 0x02; f[5] = 0x83; f[6] = 1; f[7] = 76;
        let s = st(attackshark_r5_parse, &f, 0x0047);
        assert_eq!(s.level, Some(76));
        assert_eq!(s.charging, Some(true));
        // 充电标志与百分比对调时仍按「两者之一在 0/1」的规则接受
        f[6] = 76; f[7] = 0;
        let s = st(attackshark_r5_parse, &f, 0x0047);
        assert_eq!(s.level, Some(76));
        assert_eq!(s.charging, Some(false));
        let mut g = [0u8; 9];
        g[1] = 0xa1; g[4] = 0x02; g[6] = 0x83; g[7] = 0; g[8] = 90;
        assert_eq!(st(attackshark_r5_parse, &g, 0x0047).level, Some(90));
    }

    #[test]
    fn eweadn_parser() {
        let mut f = [0u8; 15];
        f[0] = 0x30;
        f[4..8].copy_from_slice(b"M156");
        f[12] = 1; f[13] = 70;
        let s = st(eweadn_parse, &f, 0x062f);
        assert_eq!(s.level, Some(70));
        assert_eq!(s.charging, Some(true));
        assert_eq!(s.name, "EWEADN S9");
        f[0] = 0x02; // 鼠标睡眠
        assert!(eweadn_parse(&f, 0x062f).is_none());
        assert_eq!(EWEADN_REQUEST[1], 0x30);
    }

    #[test]
    fn monsgeek_parser() {
        let mut f = [0u8; 9];
        f[2] = 54; f[7] = 1;
        let s = st(monsgeek_parse, &f, 0x5026);
        assert_eq!(s.level, Some(54));
        assert_eq!(s.charging_known, false);
        f[4] = 1; // 键盘离线
        assert!(monsgeek_parse(&f, 0x5026).is_none());
        f[4] = 0; f[7] = 2; // 只有鼠标
        assert!(monsgeek_parse(&f, 0x5026).is_none());
        assert_eq!(MONGEEK_REQUEST_CKSUM[8], 0x08);
    }

    #[test]
    fn ninjutso_sprime_parser() {
        let mut f = [0u8; 13];
        f[..2].copy_from_slice(&[0x05, 0x15]);
        f[9] = 70; f[10] = 1; f[12] = 1;
        let s = st(ninjutso_sprime_parse, &f, 0xae1c);
        assert_eq!(s.level, Some(70));
        assert_eq!(s.charging, Some(true));
        assert_eq!(s.key, "ninjutso:sora_v2");
        f[9] = 0; // Sora 的 0 表示 100%
        assert_eq!(st(ninjutso_sprime_parse, &f, 0xae1c).level, Some(100));
        f[9] = 70; f[11] = 1; // Sprime 的 full 标志
        let s = st(ninjutso_sprime_parse, &f, 0xac1c);
        assert_eq!(s.level, Some(100));
        assert_eq!(s.charging, Some(false));
        f[12] = 0; // 接收器模式下未连接
        assert_eq!(st(ninjutso_sprime_parse, &f, 0xae1c).level, None);
    }

    #[test]
    fn raikiri_parser() {
        let mut f = [0u8; 11];
        f[..3].copy_from_slice(&[0xb0, 0x12, 0x04]);
        f[5] = 70; f[10] = 1;
        let s = st(raikiri_parse, &f, 0x1c92);
        assert_eq!(s.level, Some(70));
        assert_eq!(s.charging, Some(true));
        f[2] = 0x05;
        assert!(raikiri_parse(&f, 0x1c92).is_none());
    }

    #[test]
    fn royuan_parser() {
        let mut f = [0u8; 65];
        f[2] = 54; f[7] = 1; // payload[1]=54, payload[6]=keyboard
        let s = st(royuan_parse, &f, 0x4011);
        assert_eq!(s.level, Some(54));
        assert_eq!(s.charging_known, false);
        f[7] = 2; // mouse 不算键盘
        assert!(royuan_parse(&f, 0x4011).is_none());
        assert_eq!(ROYUAN_REQUEST[8], 0x08);
    }

    #[test]
    fn sony_h9ii_parser() {
        let mut f = [0u8; 64];
        f[0] = 0x02; f[9] = 0x06; f[14] = 1; f[15] = 76;
        let s = st(sony_h9ii_parse, &f, 0x0fa8);
        assert_eq!(s.level, Some(76));
        assert_eq!(s.charging, Some(true));
        f[14] = 9; // 未知充电状态：保留电量但标记未知
        let s = st(sony_h9ii_parse, &f, 0x0fa8);
        assert_eq!(s.level, Some(76));
        assert!(!s.charging_known);
        assert!(sony_h9ii_parse(&SONY_REQUEST, 0x0fa8).is_none());
    }

    #[test]
    fn registration_and_instance_filter() {
        let providers = all_providers();
        for name in [
            "arbiter", "areson", "cherry", "coolermaster", "darmoshark", "astro", "jbl",
            "royal_kludge", "alienware", "am_infinity", "attackshark_r5", "eweadn",
            "monsgeek", "ninjutso_sprime", "raikiri", "royuan", "sony_h9ii",
        ] {
            assert!(providers.iter().any(|p| p.name() == name), "缺少 {name}");
        }
        // 无集合 / VID/PID 不匹配时不产出，也不触碰 HID
        let p = providers.iter().find(|p| p.name() == "royuan").unwrap();
        assert!(p.poll(r"USB\VID_3151&PID_4011\7&ABC&0&0", &[]).is_empty());
        assert!(p.poll(r"USB\VID_3151&PID_5007\7&ABC&0&0", &[]).is_empty());
        assert_eq!(providers.len(), SPECS.len());
    }
}

// 已移植协议：arbiter, areson, cherry, coolermaster, darmoshark, astro, jbl_hidv3,
// royal_kludge, alienware, am_infinity, attackshark_r5, eweadn, monsgeek,
// ninjutso_sprime, raikiri, royuan, sony_h9ii（共 17 个）。
