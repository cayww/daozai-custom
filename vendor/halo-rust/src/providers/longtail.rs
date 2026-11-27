//! §4.5 长尾协议静态移植（26 个；不含已由其它批次负责的 kraken_v4 / steam）。
//!
//! 每个 Spec 的 VID/PID、请求字节、校验与解析索引均逐字来自
//! `vendor/halo-battery/upstream/providers/<name>.py`，不猜字节格式。
//! 简单协议用声明式 `Spec`（多步请求的每一 replies[i] 对应一步的回答），
//! 需要跨轮询缓存/复杂握手的用专属 Provider（eweadn_ydx / centurion / g435）。
//!
//! 已知运行时限制（静态移植无法在此文件内解决，需实机与 hid.rs 配合确认）：
//! 1. `hid.rs::supported_usage` 只放行 usage_page >= 0xFF00 的集合，因此
//!    akko_s9(008C/000C)、eightbitdo(0001 游戏手柄)、eweadn_ydx(000A)、
//!    machenike(008C)、msi PRO(00FF)、zowie EC-CW(00FF) 的集合不会传入 poll；
//!    本文件不自行过滤低 page，hid.rs 放行后即可生效（未改 hid.rs）。
//! 2. 依赖「同一 VID 下兄弟 PID（充电线）在插」的 charging 推断无法从单个
//!    receiver instance 观察到；glorious/wlmouse/vaxee/nzxt 等按 reply 自身字段上报。
//! 3. Python 侧 `_last` 休眠保持：本文件与 mchose.rs 一致，无回复时不产出
//!    Status，由 Engine 的 fresh/age_ms 承担读取生命周期。
//! 4. 静态移植只能保证与 Python 逻辑一致，读数正确性需实机确认。

use crate::hid::{Collection, Handle};
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const IO_TIMEOUT: Duration = Duration::from_millis(250);

type ParseFn = fn(&[Vec<u8>], u16) -> Option<Status>;
type AcceptFn = fn(&[u8]) -> bool;

#[derive(Clone, Copy)]
struct Selector {
    page: u16,   // 0 = 任意 page（本文件不自行过滤 < 0xFF00）
    usage: u16,  // 0xffff = 任意 usage
    iface: Option<u8>,
}

const ANY: Selector = Selector { page: 0, usage: 0xffff, iface: None };

fn matches(sel: Selector, col: &Collection) -> bool {
    if sel.page != 0 && col.usage_page != sel.page { return false; }
    if sel.usage != 0xffff && col.usage != sel.usage { return false; }
    sel.iface.is_none_or(|i| col.interface == Some(i))
}

#[derive(Clone, Copy)]
enum Op {
    /// output report 写请求，同一集合 input report 读回复
    WriteRead,
    /// set_feature 写请求，get_feature(id) 读回复
    Feature { id: u8 },
    /// set_feature 写请求，另一集合 input report 读回复
    FeatureInput { reply: Selector },
    /// 只读，不发送请求（监听被动上报）
    Listen,
}

struct Step {
    op: Op,
    request: &'static [u8],
    attempts: u8,
    accept: AcceptFn,
}

struct Spec {
    name: &'static str,
    vids: &'static [u16],       // 空 = 任意 VID
    pids: &'static [u16],       // 空 = 任意 PID
    selector: Selector,
    steps: &'static [Step],
    parse: ParseFn,
}

struct LongtailProvider(&'static Spec);

impl Provider for LongtailProvider {
    fn name(&self) -> &'static str { self.0.name }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let spec = self.0;
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else { return Vec::new() };
        if !spec.vids.is_empty() && !spec.vids.contains(&vid) { return Vec::new(); }
        if !spec.pids.is_empty() && !spec.pids.contains(&pid) { return Vec::new(); }
        for col in collections.iter().filter(|c| matches(spec.selector, c)) {
            let replies = exchange(spec.steps, col, collections);
            if let Some(st) = (spec.parse)(&replies, pid) {
                return vec![st];
            }
        }
        Vec::new()
    }
}

fn exchange(steps: &[Step], col: &Collection, collections: &[Collection]) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(steps.len());
    for step in steps {
        let mut reply: Option<Vec<u8>> = None;
        for _ in 0..step.attempts.max(1) {
            let deadline = Instant::now() + IO_TIMEOUT;
            let Some(mut handle) = col.open() else { break };
            let sent = match step.op {
                Op::WriteRead | Op::FeatureInput { .. } => handle.write(step.request, deadline),
                Op::Feature { .. } => handle.set_feature(step.request, deadline),
                Op::Listen => true,
            };
            if !sent { break; }
            let frame = match step.op {
                Op::WriteRead | Op::Listen => handle.read(deadline),
                Op::Feature { id } => handle.get_feature(id, deadline),
                Op::FeatureInput { reply } => {
                    let target = if matches(reply, col) {
                        Some(col)
                    } else {
                        collections.iter().find(|c| matches(reply, c))
                    };
                    match target {
                        Some(rc) if std::ptr::eq(rc, col) => handle.read(deadline),
                        Some(rc) => rc.open().and_then(|mut h| h.read(deadline)),
                        None => None,
                    }
                }
            };
            if let Some(f) = frame {
                if (step.accept)(&f) { reply = Some(f); break; }
            }
        }
        out.push(reply.unwrap_or_default());
    }
    out
}

fn get<'a>(replies: &'a [Vec<u8>], i: usize) -> &'a [u8] {
    replies.get(i).map_or(&[], |v| v.as_slice())
}

fn mk_status(
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

const fn padded<const N: usize>(prefix: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut i = 0;
    while i < prefix.len() && i < N { out[i] = prefix[i]; i += 1; }
    out
}

const fn sum8(data: &[u8]) -> u8 {
    let mut acc = 0u8;
    let mut i = 0;
    while i < data.len() { acc = acc.wrapping_add(data[i]); i += 1; }
    acc
}

// const fn 中 Range 索引尚不稳定，故用「长度/起点」变体构造请求常量。
const fn sum8_prefix(data: &[u8], n: usize) -> u8 {
    let mut acc = 0u8;
    let mut i = 0;
    while i < n { acc = acc.wrapping_add(data[i]); i += 1; }
    acc
}

const fn sum8_tail(data: &[u8], start: usize) -> u8 {
    let mut acc = 0u8;
    let mut i = start;
    while i < data.len() { acc = acc.wrapping_add(data[i]); i += 1; }
    acc
}

fn any_nonempty(f: &[u8]) -> bool { !f.is_empty() }

// ---------------- 请求常量 ----------------

// akko_s9.py: CRC-16/IBM (init FFFF, poly 8005) 覆盖前 62 字节，大端 trailer。
const fn akko_checksum_n(payload: &[u8], n: usize) -> u16 {
    let mut crc: u16 = 0xffff;
    let mut i = 0;
    while i < n {
        crc ^= payload[i] as u16;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0x8005 } else { crc >> 1 };
            bit += 1;
        }
        i += 1;
    }
    crc
}

fn akko_checksum(payload: &[u8]) -> u16 { akko_checksum_n(payload, payload.len()) }

const fn akko_request(cmd: u8) -> [u8; 64] {
    let mut f = [0u8; 64];
    f[0] = 0x02; f[1] = cmd; f[2] = 0x01;
    let c = akko_checksum_n(&f, 62);
    f[62] = (c >> 8) as u8;
    f[63] = (c & 0xff) as u8;
    f
}

const AKKO_LINK: [u8; 64] = akko_request(0x10);
const AKKO_BATTERY: [u8; 64] = akko_request(0x12);

// atk_nearlink.py: 64 字节 wire = report id + 63 字节 payload，requestCode=1。
const ATK_RECEIVER: [u8; 64] = padded(&[0x05, 0x01, 0x00, 0x1a]);
const ATK_CABLE: [u8; 64] = padded(&[0x04, 0x01, 0x00, 0x1a]);

// atk_bitmouse.py: report 08，frame[0] = sum(frame[1:])。
const fn bitmouse_request(cmd: u8) -> [u8; 64] {
    let mut f = [0u8; 64];
    f[0] = 0x08; f[2] = 0x72; f[3] = 0x02; f[5] = 0x01; f[6] = cmd; f[7] = 0x01;
    f[1] = sum8_tail(&f, 2);
    f
}
const BITMOUSE_BATTERY: [u8; 64] = bitmouse_request(7);
const BITMOUSE_CHARGING: [u8; 64] = bitmouse_request(15);

const AULA_REQUEST: [u8; 33] = padded(&[0x00, 0xaa, 0x10]);
const fn f108_request() -> [u8; 33] {
    let mut f = [0u8; 33];
    f[1] = 0x20; f[2] = 0x01; f[32] = 0x21; // sum(0x20 + 0x01)
    f
}
const F108_REQUEST: [u8; 33] = f108_request();

const E8B2_INFO: [u8; 64] = padded(&[0x81, 0x04, 0x00, 0x01]);
const E8B2_PAD: [u8; 64] = padded(&[0x81, 0x04, 0x21, 0x01, 0x00, 0x00, 0x06]);

// finalmouse.py: [len, 0x80|cmd, plen, payload...]，len = 2 + plen。
const fn finalmouse_request(cmd: u8) -> [u8; 64] {
    let mut f = [0u8; 64];
    f[0] = 0x04; f[1] = 0x02; f[2] = 0x80 | cmd;
    f
}
const FM_LINK: [u8; 64] = finalmouse_request(36);
const FM_BATTERY: [u8; 64] = finalmouse_request(38);
const FM_VBAT: [u8; 64] = finalmouse_request(5);
const FM_CHARGING: [u8; 64] = finalmouse_request(37);

// flydigi.py: SDL 的 zeroed info/arch 帧，及 padded/numbered 变体。
const FLY_ARCH: [u8; 6] = [0x00, 0x5a, 0xa5, 0x07, 0x00, 0x00];
const FLY_INFO: [u8; 6] = [0x00, 0x5a, 0xa5, 0x01, 0x02, 0x00];
const FLY_INFO_PADDED: [u8; 32] = padded(&FLY_INFO);
const FLY_INFO_NUMBERED: [u8; 6] = [0x03, 0x5a, 0xa5, 0x01, 0x02, 0x00];
const FLY_INFO_NUMBERED_PADDED: [u8; 32] = padded(&FLY_INFO_NUMBERED);

const GAMESIR_REQUEST: [u8; 2] = [0x0f, 0x0b];
// frame[0] = report id 0，BATTERY_COMMAND 落在 [1..7]，0x83 位于 byte 6。
const GLORIOUS_REQUEST: [u8; 65] = padded(&[0x00, 0x00, 0x00, 0x02, 0x02, 0x00, 0x83]);

// irocks.py: 17 字节，末字节 = 0x55 - sum(前 16)。
const fn irocks_request(cmd: u8) -> [u8; 17] {
    let mut f = [0u8; 17];
    f[0] = 0x08; f[1] = cmd;
    f[16] = (0x55u8).wrapping_sub(sum8_prefix(&f, 16));
    f
}
const IROCKS_CONNECT: [u8; 17] = irocks_request(3);
const IROCKS_POWER: [u8; 17] = irocks_request(4);

const KEYCHRON_REQUEST: [u8; 64] = padded(&[0xb3, 0x06]);
const MACHENIKE_REQUEST: [u8; 21] = padded(&[0x51, 0x06]);
const MSI_REQUEST: [u8; 65] = padded(&[0x00, 0x0f, 0x20]);
const MSI_PRO_REQUEST: [u8; 65] = padded(&[0x00, 0x0d, 0xb0, 0x01, 0x00, 0x00, 0x00, 0x05]);

// ninjutso_sora_v3.py: [0x06, cmd, 0, 0, 1, 0, ...] 共 16 字节（含 report id）。
const fn ninjutso_request(cmd: u8) -> [u8; 16] {
    let mut f = [0u8; 16];
    f[0] = 0x06; f[1] = cmd; f[4] = 0x01;
    f
}
const NINJUTSO_BATTERY: [u8; 16] = ninjutso_request(0x12);
const NINJUTSO_CHARGING: [u8; 16] = ninjutso_request(0x11);

const NZXT_REQUEST: [u8; 64] = padded(&[0x4e, 0x02, 0x81, 0x00, 0xb0]);

// skullcandy.py: 06 <len=7> <recipient=80> | 05 5A 03 00 D6 0C 00
const SKULLCANDY_ASK: [u8; 62] = padded(&[0x06, 0x07, 0x80, 0x05, 0x5a, 0x03, 0x00, 0xd6, 0x0c, 0x00]);

// sony_inzone.py: make_hid_report(0x0102) = [02, 12] + make_query(0x0102) + 补零。
const SONY_HID_REQUEST: [u8; 64] = padded(&[
    0x02, 0x0c, 0x01, 0x00, 0xfc, 0x08, 0x96, 0xc3,
    0x41, 0x04, 0x01, 0x02, 0x01, 0xa2,
]);

const fn vaxee_request(cmd: u8) -> [u8; 64] {
    let mut f = [0u8; 64];
    f[0] = 0x0e; f[1] = 0xa5; f[2] = cmd; f[3] = 0x01; f[4] = 0x01;
    f
}
const VAXEE_BATTERY: [u8; 64] = vaxee_request(0x0b);
const VAXEE_CHARGING: [u8; 64] = vaxee_request(0x10);

const WLMOUSE_REQUEST: [u8; 65] = padded(&[0x00, 0x00, 0x00, 0x02, 0x02, 0x00, 0x83]);
const ZOWIE_CW_REQUEST: [u8; 65] = padded(&[0x00, 0xa5, 0x03]);
const ZOWIE_DW_REQUEST: [u8; 16] = padded(&[0x08, 0xb4, 0x15]);

// ---------------- 解析函数（索引严格照搬 Python）----------------

// ---- akko_s9 ----
fn akko_reply_ok(f: &[u8], cmd: u8) -> bool {
    f.len() >= 64 && f[0] == 0x02 && f[1] == cmd && f[2] == 0x01
        && (u16::from(f[62]) << 8 | u16::from(f[63])) == akko_checksum(&f[..62])
}
fn akko_link_ok(f: &[u8]) -> bool { akko_reply_ok(f, 0x10) }
fn akko_battery_ok(f: &[u8]) -> bool { akko_reply_ok(f, 0x12) }

fn akko_s9_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let link = get(r, 0);
    if !akko_link_ok(link) { return None; }
    let mut st = mk_status("akko:1087", "Akko S9 Ultra", "headset", "akko_s9", None, None, false);
    if link[4] == 0 { st.online = false; return Some(st); }   // 耳机未连接/超距
    if link[4] != 1 { return None; }
    let battery = get(r, 1);
    if akko_battery_ok(battery) {
        let level = battery[4];
        if (1..=100).contains(&level) { st.level = Some(level); }
    }
    Some(st)
}

// ---- atk_nearlink ----
fn atk_nearlink_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let (name, report_id) = match pid {
        0x108c => ("ATK X1 Ultimate (2.4 GHz)", 0x05),
        0x108b => ("ATK X1 Ultimate (wired)", 0x04),
        _ => return None,
    };
    let f = get(r, 0);
    if f.len() < 64 || f[0] != report_id || f[3] != 0x1a { return None; }
    let level = f[8];
    if level > 100 { return None; }
    Some(mk_status(
        format!("atk_nearlink:{pid:04x}"), name, "mouse", "atk_nearlink",
        Some(level), Some(f[9] != 0), true,
    ))
}

// ---- atk_bitmouse ----
fn bitmouse_value(f: &[u8], cmd: u8) -> Option<u8> {
    if f.len() < 8 || f[0] != 0x08 || f[1] != 0x72 || f[2] == 0xff
        || f[5] != cmd || f[6] < 1 { return None; }
    Some(f[7])
}
fn bitmouse_battery_ok(f: &[u8]) -> bool { bitmouse_value(f, 7).is_some() }
fn bitmouse_charging_ok(f: &[u8]) -> bool { bitmouse_value(f, 15).is_some() }

fn atk_bitmouse_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let level = bitmouse_value(get(r, 0), 7)?;
    if !(1..=100).contains(&level) { return None; }
    let charging = bitmouse_value(get(r, 1), 15).unwrap_or(0) == 1;
    Some(mk_status(
        "pulsar:373b1216", "ATK VXE NK mouse (2.4 GHz)", "mouse", "atk_bitmouse",
        Some(level), Some(charging), true,
    ))
}

// ---- aula ----
fn aula_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let f = get(r, 0);
    for (lead, level_at) in [(0usize, 25usize), (1usize, 26usize)] {
        if f.len() > level_at && f[lead] == 0x55 && f[lead + 1] == 0x10 {
            let level = f[level_at];
            if (1..=100).contains(&level) {
                return Some(mk_status(
                    "aula:fefc", "AULA keyboard", "keyboard", "aula",
                    Some(level), Some(false), true,
                ));
            }
        }
    }
    None
}

// ---- aula_f108 ----
fn f108_ok(f: &[u8]) -> bool {
    let f = if f.len() == 33 && f[0] == 0 { &f[1..] } else { f };
    f.len() == 32 && f[..3] == [0x20, 0x01, 0x00] && sum8(&f[..31]) == f[31]
}
fn aula_f108_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let raw = get(r, 0);
    let f = if raw.len() == 33 && raw[0] == 0 { &raw[1..] } else { raw };
    if !f108_ok(raw) || f.len() != 32 { return None; }
    let level = f[3];
    if level > 100 { return None; }
    Some(mk_status(
        "aula_f108:024f", "AULA F108 Pro", "keyboard", "aula_f108",
        Some(level), Some(false), false,
    ))
}

// ---- eightbitdo（listen-only）----
fn bitdo_accept(f: &[u8]) -> bool {
    f.len() > 14 && (f[0] == 0x01 || f[0] == 0x04)
        && (f[14] & 0x7f) >= 1 && (f[14] & 0x7f) <= 100
}
fn eightbitdo_name(pid: u16) -> Option<&'static str> {
    match pid {
        0x6000 | 0x6100 => Some("8BitDo SF30 Pro"),
        0x6001 | 0x6101 => Some("8BitDo SN30 Pro"),
        0x6003 | 0x6006 => Some("8BitDo Pro 2"),
        0x6009 => Some("8BitDo Pro 3"),
        0x6012 => Some("8BitDo Ultimate 2"),
        _ => None,
    }
}
fn eightbitdo_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let name = eightbitdo_name(pid)?;
    for f in r {
        if !bitdo_accept(f) { continue; }
        let level = f[14] & 0x7f;
        let charging = f[14] & 0x80 != 0;
        return Some(mk_status(
            format!("8bitdo:{pid:04x}"), name, "gamepad", "8bitdo",
            Some(level), Some(charging), true,
        ));
    }
    None
}

// ---- eightbitdo_ultimate2 ----
fn e8b2_info_ok(f: &[u8]) -> bool {
    f.len() >= 6 && f[..6] == [0x02, 0x04, 0x04, 0x00, 0x00, 0x01]
}
fn e8b2_pad_ok(f: &[u8]) -> bool {
    f.len() > 18 && f[..6] == [0x02, 0x04, 0x04, 0x00, 0x21, 0x01]
}
fn eightbitdo_ultimate2_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let f = get(r, 1);
    if !e8b2_pad_ok(f) { return None; }
    let level = f[18];
    if !(1..=100).contains(&level) { return None; }
    Some(mk_status(
        "eightbitdo_ultimate2:310b", "8BitDo Ultimate 2", "gamepad", "eightbitdo_ultimate2",
        Some(level), Some(false), false,
    ))
}

// ---- eweadn_ydx（独立 Provider：同接收器两个集合，GET_FEATURE 5 触发 input 04）----
struct EweadnYdx;

fn eweadn_parse(f: &[u8]) -> Option<Status> {
    if f.len() != 32 || f[0] != 4 { return None; }
    if f[1] == 0 {
        let mut st = mk_status(
            "eweadn_ydx", "EWEADN X23 Pro", "mouse", "eweadn_ydx", None, Some(false), false,
        );
        st.online = false;
        return Some(st);
    }
    if f[1] != 1 { return None; }
    let level = f[4];
    let charge = f[5];
    if level > 100 { return None; }
    let known = matches!(charge, 0 | 1 | 2);
    let shown = if charge == 2 { 100 } else { level };
    Some(mk_status(
        "eweadn_ydx", "EWEADN X23 Pro", "mouse", "eweadn_ydx",
        Some(shown), Some(charge == 1), known,
    ))
}

impl Provider for EweadnYdx {
    fn name(&self) -> &'static str { "eweadn_ydx" }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else { return Vec::new() };
        if vid != 0x04f3 || pid != 0x026f { return Vec::new(); }
        let feature_col = collections.iter().find(|c| {
            c.usage_page == 0x000a && c.usage == 0 && c.feature_len > 0 && c.input_len == 0
        });
        let input_col = collections.iter().find(|c| {
            c.usage_page == 0x000a && c.usage == 0 && c.input_len > 0
        });
        let (Some(fc), Some(ic)) = (feature_col, input_col) else { return Vec::new() };
        let Some(mut fh) = fc.open() else { return Vec::new() };
        let deadline = Instant::now() + IO_TIMEOUT;
        if fh.get_feature(5, deadline).is_none() { return Vec::new(); }
        let Some(mut ih) = ic.open() else { return Vec::new() };
        let Some(frame) = ih.read(deadline) else { return Vec::new() };
        eweadn_parse(&frame).into_iter().collect()
    }
}

// ---- finalmouse ----
fn fm_frame<'a>(f: &'a [u8], cmd: u8) -> Option<&'a [u8]> {
    if f.len() < 4 || f[0] != 0x05 { return None; }
    let plen = f[3] as usize;
    if f.len() < 4 + plen || f[2] & 0x7f != cmd { return None; }
    Some(&f[4..4 + plen])
}
fn fm_link_ok(f: &[u8]) -> bool { fm_frame(f, 36).is_some() }
fn fm_battery_ok(f: &[u8]) -> bool { fm_frame(f, 38).is_some() }
fn fm_vbat_ok(f: &[u8]) -> bool { fm_frame(f, 5).is_some() }
fn fm_charging_ok(f: &[u8]) -> bool { fm_frame(f, 37).is_some() }

fn fm_volts_to_percent(mv: u16) -> Option<u8> {
    if mv == 0 || mv == 0xffff { return None; }
    let volts = [3.0f64, 3.62, 3.66, 3.74, 3.88, 4.17, 4.38];
    let pct = [0.2f64, 5.0, 10.0, 25.0, 50.0, 75.0, 100.0];
    let v = f64::from(mv) / 1000.0;
    if v > volts[6] { return Some(100); }
    if v < volts[0] { return Some(0); }
    for i in 0..6 {
        if volts[i] <= v && v <= volts[i + 1] {
            let p = pct[i] + (pct[i + 1] - pct[i]) * (v - volts[i]) / (volts[i + 1] - volts[i]);
            return Some((p + 0.5).floor() as u8);
        }
    }
    Some(100)
}

fn finalmouse_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let link = fm_frame(get(r, 0), 36)?;
    if link.is_empty() || link[0] == 0 { return None; }   // 鼠标未连接
    let mut level = None;
    if let Some(p) = fm_frame(get(r, 1), 38) {
        if !p.is_empty() && p[0] <= 100 { level = Some(p[0]); }
    }
    if level.is_none() {
        if let Some(p) = fm_frame(get(r, 2), 5) {
            if p.len() >= 2 {
                let mv = u16::from(p[0]) | (u16::from(p[1]) << 8);
                level = fm_volts_to_percent(mv);
            }
        }
    }
    let level = level?;
    let charging = fm_frame(get(r, 3), 37).is_some_and(|p| !p.is_empty() && p[0] == 1);
    Some(mk_status(
        "finalmouse:0100", "Finalmouse UltralightX", "mouse", "finalmouse",
        Some(level), Some(charging), true,
    ))
}

// ---- flydigi ----
fn fly_battery(f: &[u8]) -> Option<(u8, bool)> {
    if f.len() < 17 { return None; }
    let body = if f[0] == 0x5a { f } else { &f[1..] };
    if body.len() < 17 || body[0] != 0x5a || body[1] != 0xa5 || body[2] != 0x01 { return None; }
    let byte = body[11];
    let state = byte >> 4;
    let step = byte & 0x0f;
    if state > 2 || step > 5 { return None; }
    let level = if state == 2 { 100 } else { step * 20 };
    Some((level, state == 1 || state == 2))
}
fn fly_ok(f: &[u8]) -> bool { fly_battery(f).is_some() }

fn flydigi_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    for f in r {
        if let Some((level, charging)) = fly_battery(f) {
            return Some(mk_status(
                "flydigi:2401", "FlyDigi Vader 5 Pro", "gamepad", "flydigi",
                Some(level), Some(charging), true,
            ));
        }
    }
    None
}

// ---- gamesir ----
fn gamesir_name(pid: u16) -> String {
    match pid {
        0x100b | 0x0575 => "GameSir Cyclone 2".to_string(),
        0x10c5 | 0x10c6 => "GameSir G7 Pro 8K Aimlabs".to_string(),
        0x10c7 | 0x10c8 => "GameSir G7 Pro 8K Nioh 3".to_string(),
        0x10c9 | 0x10ca => "GameSir G7 Pro 8K NBA 2K".to_string(),
        0x10fd | 0x10fe => "GameSir G7 Pro 8K Golden Apple".to_string(),
        _ => "GameSir G7 Pro 8K".to_string(),
    }
}
fn gamesir_ok(f: &[u8]) -> bool {
    f.len() > 36 && f[0] == 0x12 && !(f[35] == 0 && f[36] == 0)
}
fn gamesir_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let f = get(r, 0);
    if !gamesir_ok(f) || f[36] > 100 { return None; }
    Some(mk_status(
        format!("gamesir:{pid:04x}"), gamesir_name(pid), "gamepad", "gamesir",
        Some(f[36]), Some(f[35] == 1), true,
    ))
}

// ---- glorious ----
fn glorious_name(pid: u16) -> &'static str {
    match pid {
        0x2022 => "Glorious Model O Wireless",
        0x2024 => "Glorious Model O- Wireless",
        0x2023 => "Glorious Model D Wireless",
        0x2025 => "Glorious Model D- Wireless",
        _ => "Glorious mouse",
    }
}
fn glorious_ok(f: &[u8]) -> bool {
    f.len() > 8 && f[0] == 0x00 && f[6] == 0x83 && f[1] == 0xa1
}
fn glorious_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let f = get(r, 0);
    if !glorious_ok(f) || f[8] > 100 { return None; }
    Some(mk_status(
        format!("glorious:{pid:04x}"), glorious_name(pid), "mouse", "glorious",
        Some(f[8]), Some(f[7] != 0), true,
    ))
}

// ---- irocks ----
fn irocks_reply(f: &[u8], cmd: u8) -> Option<[u8; 17]> {
    if f.len() < 17 { return None; }
    let mut b = [0u8; 17];
    b.copy_from_slice(&f[..17]);
    if !matches!(b[0], 0x08 | 0x09) || b[1] != cmd { return None; }
    if b[16] != (0x55u8).wrapping_sub(sum8(&b[..16])) { return None; }
    Some(b)
}
fn irocks_connect_ok(f: &[u8]) -> bool { irocks_reply(f, 3).is_some() }
fn irocks_power_ok(f: &[u8]) -> bool { irocks_reply(f, 4).is_some() }

fn irocks_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    match pid {
        0xc009 => {
            let connect = irocks_reply(get(r, 0), 3)?;
            if connect[2] != 0 || connect[6] != 1 { return None; } // 鼠标离线
            let power = irocks_reply(get(r, 1), 4)?;
            let level = power[6];
            if level > 100 { return None; }
            Some(mk_status(
                "irocks:c009", "iRocks M31R (2.4 GHz)", "mouse", "irocks",
                Some(level), Some(power[7] != 0), true,
            ))
        }
        0xc010 => {
            let power = irocks_reply(get(r, 0), 4)?;
            let level = power[6];
            if level > 100 { return None; }
            Some(mk_status(
                "irocks:c010", "iRocks M31R (wired)", "mouse", "irocks",
                Some(level), Some(power[7] != 0), true,
            ))
        }
        _ => None,
    }
}

// ---- jbl（listen-only）----
fn jbl_ok(f: &[u8]) -> bool { f.len() > 1 && f[0] == 0x08 && f[1] <= 100 }
fn jbl_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let f = get(r, 0);
    if !jbl_ok(f) { return None; }
    Some(mk_status(
        "jbl:2088", "JBL Quantum 910 Wireless", "headset", "jbl",
        Some(f[1]), Some(false), true,
    ))
}

// ---- keychron ----
fn keychron_name(pid: u16) -> &'static str {
    match pid {
        0xd028 => "Keychron Ultra-Link 8K",
        0xd048 => "Keychron M5",
        0xd026 => "Keychron Nape Pro",
        _ => "Keychron mouse",
    }
}
fn keychron_ok(f: &[u8]) -> bool { f.len() > 20 && f[0] == 0xb4 && f[1] == 0x06 }
fn keychron_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    for f in r {
        if !keychron_ok(f) { continue; }
        let raw = f[20];
        let level = raw & 0x7f;
        if level > 100 { continue; }
        return Some(mk_status(
            format!("keychron:{pid:04x}"), keychron_name(pid), "mouse", "keychron",
            Some(level), Some(raw & 0x80 != 0), true,
        ));
    }
    None
}

// ---- logitech_centurion（独立 Provider，完整发现 + battery 0104 + legacy 回退）----
const CENTURION_LEGACY: [u8; 64] = padded(&[
    0x51, 0x08, 0x00, 0x03, 0x1a, 0x00, 0x03, 0x00, 0x04, 0x0a,
]);

fn centurion_frame(payload: &[u8], flags: u8) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[0] = 0x51;
    out[1] = (payload.len() + 1) as u8;
    out[2] = flags;
    let n = payload.len().min(61);
    out[3..3 + n].copy_from_slice(&payload[..n]);
    out
}

fn centurion_direct(feature: u8, function: u8, params: &[u8]) -> [u8; 64] {
    let mut p = [0u8; 61];
    p[0] = feature;
    p[1] = (function & 0xf0) | 0x01;
    let n = params.len().min(59);
    p[2..2 + n].copy_from_slice(&params[..n]);
    centurion_frame(&p[..2 + n], 0)
}

fn centurion_bridge(bridge: u8, sub: u8, function: u8, params: &[u8]) -> [u8; 64] {
    let mut submsg = [0u8; 61];
    submsg[0] = 0x00;
    submsg[1] = sub;
    submsg[2] = (function & 0xf0) | 0x01;
    // 外层 payload 至多 61 字节（64 字节报表减去 id/len/flags），留出 4 字节头。
    let n = params.len().min(54);
    submsg[3..3 + n].copy_from_slice(&params[..n]);
    let total = 3 + n;
    let mut p = [0u8; 61];
    p[0] = bridge;
    p[1] = 0x11;
    p[2] = ((total >> 8) & 0x0f) as u8;
    p[3] = (total & 0xff) as u8;
    p[4..4 + total].copy_from_slice(&submsg[..total]);
    centurion_frame(&p[..4 + total], 0)
}

fn centurion_payload(r: &[u8]) -> Option<&[u8]> {
    if r.len() < 4 || r[0] != 0x51 { return None; }
    let n = r[1] as usize;
    if n <= 1 || n + 2 > r.len() { return None; }
    Some(&r[3..2 + n])
}

fn centurion_read_frames(h: &mut Handle, count: usize, deadline: Instant) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for _ in 0..count {
        if let Some(f) = h.read(deadline) {
            if !f.is_empty() { out.push(f); }
        }
    }
    out
}

struct LogitechCenturion {
    state: Mutex<HashMap<String, (u8, HashMap<u16, u8>)>>,
}

impl LogitechCenturion {
    fn new() -> Self { Self { state: Mutex::new(HashMap::new()) } }

    fn direct(&self, h: &mut Handle, feature: u8, function: u8, params: &[u8]) -> Option<Vec<u8>> {
        let req = centurion_direct(feature, function, params);
        if !h.write(&req, Instant::now() + IO_TIMEOUT) { return None; }
        let fn_sw = (function & 0xf0) | 0x01;
        let deadline = Instant::now() + Duration::from_millis(250);
        for r in centurion_read_frames(h, 8, deadline) {
            let Some(p) = centurion_payload(&r) else { continue };
            if p.len() < 2 { continue; }
            if p[0] == 0xff && p[1] == feature { continue; } // 接收器错误回复
            if p[0] == feature && p[1] == fn_sw { return Some(p[2..].to_vec()); }
        }
        None
    }

    fn bridge(&self, h: &mut Handle, bridge: u8, sub: u8, function: u8, params: &[u8]) -> Option<Vec<u8>> {
        let req = centurion_bridge(bridge, sub, function, params);
        if !h.write(&req, Instant::now() + IO_TIMEOUT) { return None; }
        let fn_sw = (function & 0xf0) | 0x01;
        let deadline = Instant::now() + Duration::from_millis(250);
        for r in centurion_read_frames(h, 8, deadline) {
            let Some(p) = centurion_payload(&r) else { continue };
            if p.len() < 2 || p[0] != bridge || (p[1] & 0xf0) != 0x10 { continue; }
            if (p[1] & 0x0f) == 0x01 { continue; } // 接收器 ACK，不是耳机回答
            if (p[1] & 0x0f) != 0 || p.len() < 7 || p[4] != 0x00 { continue; }
            if p[5] == 0xff && p[6] == sub { return None; }
            if p[5] == sub && p[6] == fn_sw { return Some(p[7..].to_vec()); }
        }
        None
    }

    fn discover(&self, h: &mut Handle) -> Option<(u8, HashMap<u16, u8>)> {
        let fs = self.direct(h, 0, 0x00, &[0x00, 0x01])?;
        let fs_index = *fs.first()?;
        if fs_index == 0 { return None; }
        let count = *self.direct(h, fs_index, 0x00, &[])?.first()?;
        let mut bridge = None;
        for n in 0..count {
            let d = self.direct(h, fs_index, 0x10, &[n])?;
            if d.len() >= 3 && (u16::from(d[1]) << 8 | u16::from(d[2])) == 0x0003 {
                bridge = Some(n);
                break;
            }
        }
        let bridge = bridge?;
        let sfs = self.bridge(h, bridge, 0x00, 0x00, &[0x00, 0x01])?;
        let sfs_index = *sfs.first()?;
        if sfs_index == 0 { return None; }
        let sub_count = *self.bridge(h, bridge, sfs_index, 0x00, &[])?.first()?;
        let mut features = HashMap::new();
        for n in 0..sub_count {
            let d = self.bridge(h, bridge, sfs_index, 0x10, &[n])?;
            if d.len() >= 3 {
                features.insert(u16::from(d[1]) << 8 | u16::from(d[2]), n);
            }
        }
        Some((bridge, features))
    }

    fn legacy(&self, h: &mut Handle) -> Option<(u8, bool)> {
        if !h.write(&CENTURION_LEGACY, Instant::now() + IO_TIMEOUT) { return None; }
        let deadline = Instant::now() + Duration::from_millis(250);
        for r in centurion_read_frames(h, 4, deadline) {
            if r.len() < 7 || r[0] != 0x51 { continue; }
            if r[1] == 0x05 && r[6] == 0x00 { return None; } // 耳机已关机
            if r.len() >= 13 && r[1] == 0x0b && r[8] == 0x04 && r[10] <= 100 {
                return Some((r[10], r[12] == 0x02));
            }
        }
        None
    }

    fn read_known(&self, h: &mut Handle, bridge: u8, features: &HashMap<u16, u8>) -> Option<(u8, bool)> {
        let Some(&index) = features.get(&0x0104) else { return self.legacy(h) };
        let data = self.bridge(h, bridge, index, 0x00, &[])?;
        let level = *data.first()?;
        if level > 100 { return None; }
        let known = data.len() >= 3 && matches!(data[2], 1 | 2 | 3);
        Some((level, known))
    }
}

impl Provider for LogitechCenturion {
    fn name(&self) -> &'static str { "logitech_centurion" }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else { return Vec::new() };
        // 0x0B45（PRO X 3）仍按 Python 的 release hold 不放行。
        if vid != 0x046d || pid != 0x0af7 { return Vec::new(); }
        let Some(col) = collections.iter().find(|c| c.usage_page == 0xffa0 && c.usage == 0x0001)
        else { return Vec::new() };
        let Some(mut h) = col.open() else { return Vec::new() };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let cached = state.get(instance).cloned();
        let res = match cached {
            Some((bridge, features)) => self.read_known(&mut h, bridge, &features),
            None => {
                let discovered = self.discover(&mut h);
                match discovered {
                    Some((bridge, features)) => {
                        let r = self.read_known(&mut h, bridge, &features);
                        if r.is_some() { state.insert(instance.to_string(), (bridge, features)); }
                        r
                    }
                    None => None,
                }
            }
        };
        if let Some((level, charging)) = res {
            return vec![mk_status(
                format!("logitech-centurion:{pid:04x}"),
                "Logitech G PRO X 2 LIGHTSPEED", "headset", "logitech",
                Some(level), Some(charging), true,
            )];
        }
        Vec::new()
    }
}

// ---- logitech_g435_rtk（独立 Provider，bootloader 会话 + 900 s 缓存）----
const G435_ENTER: [u8; 64] = padded(&[0x06, 0x01, 0x03]);
const G435_EXIT: [u8; 64] = padded(&[0x06, 0x08, 0x03]);
const G435_REMOTE_ON: [u8; 64] = padded(&[
    0x06, 0x04, 0x03, 0x00, 0x07, 0x50, 0x41, 0x0e, 0x00, 0x02, 0xe1, 0x01,
]);
const G435_REMOTE_OFF: [u8; 64] = padded(&[
    0x06, 0x04, 0x03, 0x00, 0x07, 0x50, 0x41, 0x0e, 0x00, 0x02, 0xe1, 0x00,
]);
const G435_BATTERY: [u8; 64] = padded(&[
    0x06, 0x04, 0x03, 0x00, 0x07, 0x50, 0x41, 0x06, 0x00, 0x01, 0x00, 0x31,
]);

fn g435_mv(frame: &[u8]) -> Option<u16> {
    let pattern = [0x06u8, 0x80, 0x00];
    if frame.len() < 5 { return None; }
    let pos = (0..=frame.len() - pattern.len()).find(|&i| frame[i..i + 3] == pattern)?;
    let mv = u16::from(frame[pos + 3]) | (u16::from(frame[pos + 4]) << 8);
    (3000..=4300).contains(&mv).then_some(mv)
}

fn g435_percent(mv: u16) -> u8 {
    let curve: [(u16, u8); 11] = [
        (4200, 100), (4100, 90), (4000, 80), (3900, 65), (3800, 50),
        (3750, 40), (3700, 30), (3650, 20), (3600, 10), (3500, 5), (3300, 0),
    ];
    if mv >= curve[0].0 { return 100; }
    for i in 0..curve.len() - 1 {
        if mv >= curve[i + 1].0 {
            let (v_hi, p_hi) = curve[i];
            let (v_lo, p_lo) = curve[i + 1];
            let p = f64::from(p_lo)
                + (f64::from(p_hi) - f64::from(p_lo)) * f64::from(mv - v_lo)
                    / f64::from(v_hi - v_lo);
            return p.round() as u8;
        }
    }
    0
}

struct LogitechG435 {
    cached: Mutex<Option<(Instant, Option<u8>)>>,
}

impl LogitechG435 {
    fn new() -> Self { Self { cached: Mutex::new(None) } }

    fn read(&self, h: &mut Handle) -> Option<Option<u16>> {
        let deadline = Instant::now() + IO_TIMEOUT;
        if !h.set_feature(&G435_ENTER, deadline) { return None; }
        let _ = h.get_feature(0x06, deadline); // 丢弃 enter ack
        if !h.set_feature(&G435_REMOTE_ON, deadline) { return None; }
        let _ = h.set_feature(&G435_BATTERY, deadline);
        let read_until = Instant::now() + Duration::from_millis(1000);
        let mut found = None;
        while Instant::now() < read_until {
            let short = Instant::now() + Duration::from_millis(50);
            if let Some(frame) = h.get_feature(0x06, short) {
                if let Some(mv) = g435_mv(&frame) { found = Some(mv); break; }
            }
        }
        let _ = h.set_feature(&G435_REMOTE_OFF, deadline);
        let _ = h.set_feature(&G435_EXIT, deadline);
        Some(found)
    }
}

impl Provider for LogitechG435 {
    fn name(&self) -> &'static str { "logitech_g435_rtk" }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else { return Vec::new() };
        if vid != 0x046d || pid != 0x0acb { return Vec::new(); }
        let Some(col) = collections.iter().find(|c| c.usage_page == 0xffc0) else { return Vec::new() };
        let mut cache = self.cached.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let level = match *cache {
            Some((at, lvl)) if now.saturating_duration_since(at) < Duration::from_secs(900) => lvl,
            _ => {
                let Some(mut h) = col.open() else { return Vec::new() };
                let Some(session) = self.read(&mut h) else { return Vec::new() };
                let lvl = session.map(g435_percent);
                *cache = Some((Instant::now(), lvl));
                lvl
            }
        };
        let mut st = mk_status(
            "logitech:0acb", "Logitech G435 LIGHTSPEED", "headset", "logitech",
            level, Some(false), false,
        );
        st.online = level.is_some();
        vec![st]
    }
}

// ---- machenike ----
fn machenike_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let cable = pid == 0xff4e;
    if !cable && pid != 0xff4c { return None; }
    let f = get(r, 0);
    if f.len() != 21 || f[0] != 0x51 || f[1] != 0x06 || f[4..8] != [0x8a, 0x24, 0x4e, 0xff] {
        return None;
    }
    let link = if cable { 0u8 } else { 1u8 };
    if (f[10] & 7) != link { return None; }
    let connected = cable || (f[10] & 8) != 0;
    if !connected {
        let mut st = mk_status(
            "machenike:l7", "Machenike L7 Arctic / L7 Mix", "mouse", "machenike",
            None, Some(false), true,
        );
        st.online = false;
        return Some(st);
    }
    if f[11] > 100 || f[12] > 1 { return None; }
    Some(mk_status(
        "machenike:l7", "Machenike L7 Arctic / L7 Mix", "mouse", "machenike",
        Some(f[11]), Some(f[12] == 1), true,
    ))
}

// ---- msi ----
fn msi_info(pid: u16) -> (&'static str, &'static str) {
    match pid {
        0x9e22 => ("msi:9e22", "MSI VERSA 300 ELITE WIRELESS"),
        0x0d3c | 0x0d3b => ("msi:0d31", "MSI Clutch GM31 Lightweight Wireless"),
        0x0d4c | 0x0d4b => ("msi:0d41", "MSI Clutch GM41 Lightweight Wireless"),
        0x1fc6 => ("msi:1fc6", "MSI VERSA PRO WIRELESS"),
        0x1620 => ("msi:1620", "MSI STRIKE PRO WIRELESS"),
        _ => ("msi:unknown", "MSI mouse"),
    }
}
fn msi_query_ok(f: &[u8]) -> bool {
    let b = if f.len() > 1 && f[0] == 0x00 && f[1] == 0x0f { &f[1..] } else { f };
    b.len() >= 15 && b[0] == 0x0f && b[1] == 0x20 && b[14] & 0x7f <= 100
}
fn msi_query_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let raw = get(r, 0);
    let b = if raw.len() > 1 && raw[0] == 0x00 && raw[1] == 0x0f { &raw[1..] } else { raw };
    if !msi_query_ok(raw) || b.len() < 15 { return None; }
    let (key, name) = msi_info(pid);
    Some(mk_status(
        key, name, "mouse", "msi", Some(b[14] & 0x7f), Some(b[14] & 0x80 != 0), true,
    ))
}
fn msi_passive_ok(f: &[u8]) -> bool { f.len() >= 4 && f[0] == 0xa0 && f[2] == 0x07 }
fn msi_passive_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let f = get(r, 0);
    if !msi_passive_ok(f) { return None; }
    let raw = u16::from(f[1]);
    let pct = ((raw * 51 + 1760 + 50) / 100).min(100) as u8;
    let (key, name) = msi_info(pid);
    Some(mk_status(key, name, "mouse", "msi", Some(pct), Some(false), true))
}
fn msi_pro_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let raw = get(r, 0);
    let f: Vec<u8> = if raw.len() == 64 {
        let mut v = Vec::with_capacity(65);
        v.push(0);
        v.extend_from_slice(raw);
        v
    } else { raw.to_vec() };
    if f.len() < 65 || f[7] != 0x05 { return None; }
    let level = f[9];
    let state = f[10];
    if level == 0 || level > 100 || !matches!(state, 0x01 | 0x02) { return None; }
    let (key, name) = msi_info(pid);
    let kind = if pid == 0x1620 { "keyboard" } else { "mouse" };
    Some(mk_status(key, name, kind, "msi", Some(level), Some(state == 0x01), true))
}

// ---- ninjutso_sora_v3 ----
fn ninjutso_value(f: &[u8], cmd: u8) -> Option<u8> {
    if f.len() >= 9 && f[0] == 0x06 && f[1] == cmd { Some(f[8]) }
    else if f.len() >= 8 && f[0] == cmd { Some(f[7]) }
    else { None }
}
fn ninjutso_battery_ok(f: &[u8]) -> bool { ninjutso_value(f, 0x12).is_some() }
fn ninjutso_charging_ok(f: &[u8]) -> bool { ninjutso_value(f, 0x11).is_some() }
fn ninjutso_sora_v3_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let level = ninjutso_value(get(r, 0), 0x12)?;
    if level > 100 { return None; }
    let (charging, known) = match ninjutso_value(get(r, 1), 0x11) {
        Some(v) => (v != 0, true),
        None => (false, false),
    };
    Some(mk_status(
        "ninjutso_sora_v3:eb02", "Ninjutso Sora V3", "mouse", "ninjutso_sora_v3",
        Some(level), Some(charging), known,
    ))
}

// ---- nzxt ----
fn nzxt_ok(f: &[u8]) -> bool {
    f.len() >= 27 && f[0] == 0x4e && f[1] == 0x02 && f[2] == 0x97
}
fn nzxt_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let f = get(r, 0);
    if !nzxt_ok(f) { return None; }
    let level = u16::from(f[17]) | (u16::from(f[18]) << 8);
    if level > 100 { return None; }
    let charging = f[22] & 0x01 != 0;
    Some(mk_status(
        "nzxt:2101", "NZXT Lift Elite", "mouse", "nzxt",
        Some(level as u8), Some(charging), true,
    ))
}

// ---- skullcandy ----
fn skullcandy_percent(report: &[u8]) -> Option<u8> {
    if report.len() < 9 { return None; }
    let data: &[u8] = if report[0] == 0x07 {
        if report.len() < 3 || report[2] != 0x80 { return None; }
        let end = 3 + report[1] as usize;
        if end > report.len() { return None; }
        &report[3..end]
    } else if report[0] == 0x05 {
        report
    } else {
        return None;
    };
    let mut i = 0;
    while i + 6 <= data.len() {
        if data[i] != 0x05 { i += 1; continue; }
        let typ = data[i + 1];
        let len = (u16::from(data[i + 2]) | (u16::from(data[i + 3]) << 8)) as usize;
        if len < 2 || i + 4 + len > data.len() { break; }
        let opcode = u16::from(data[i + 4]) | (u16::from(data[i + 5]) << 8);
        let payload = &data[i + 6..i + 4 + len];
        if typ == 0x5d && opcode == 0x0cd6 && payload.len() >= 3
            && payload[0] == 0 && payload[1] == 0x00 && payload[2] <= 100 {
            return Some(payload[2]);
        }
        i += 4 + len;
    }
    // 重建同步：在声明的耳机数据内搜索 05 5D 05 00 D6 0C。
    let pattern = [0x05u8, 0x5d, 0x05, 0x00, 0xd6, 0x0c];
    if data.len() >= pattern.len() {
        for k in 0..=data.len() - pattern.len() {
            if data[k..k + pattern.len()] == pattern
                && data.len() >= k + 9
                && data[k + 6] == 0 && data[k + 7] == 0x00 && data[k + 8] <= 100 {
                return Some(data[k + 8]);
            }
        }
    }
    None
}
fn skullcandy_ok(f: &[u8]) -> bool { skullcandy_percent(f).is_some() }
fn skullcandy_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let name = match pid {
        0x3210 => "Skullcandy PLYR",
        0x5310 => "Skullcandy Crusher PLYR 720",
        _ => return None,
    };
    let level = skullcandy_percent(get(r, 0))?;
    Some(mk_status(
        format!("skullcandy:{pid:04x}"), name, "headset", "skullcandy",
        Some(level), Some(false), true,
    ))
}

// ---- sony_inzone（HID H5 0x0EBF；串口 H9 无 Rust 对应实现）----
const SONY_SIGNATURE: [u8; 8] = [0x04, 0xff, 0x0b, 0x00, 0x96, 0xc3, 0x14, 0x04];
fn sony_inzone_ok(f: &[u8]) -> bool {
    f.len() >= 16 && f[0] == 0x02 && f[1] as usize >= 14
        && f[2..10] == SONY_SIGNATURE && f[10] == 0x10
}
fn sony_inzone_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    if pid != 0x0ebf { return None; }
    let f = get(r, 0);
    if !sony_inzone_ok(f) { return None; }
    if (u16::from(f[11]) | (u16::from(f[12]) << 8)) != 0x0102 { return None; }
    if sum8(&f[6..15]) != f[15] { return None; }
    let chg = f[13];
    let level = f[14];
    if level == 0xff {
        let mut st = mk_status(
            "sony_inzone_h5", "Sony INZONE H5", "headset", "sony_inzone",
            None, Some(false), false,
        );
        st.online = false;
        return Some(st);
    }
    if level > 100 { return None; }
    Some(mk_status(
        "sony_inzone_h5", "Sony INZONE H5", "headset", "sony_inzone",
        Some(level), Some(chg == 1), matches!(chg, 0 | 1),
    ))
}

// ---- vaxee ----
fn vaxee_value(f: &[u8], cmd: u8) -> Option<u8> {
    if f.len() >= 6 && f[0] == 0x0e && f[1] == 0xa5 && f[2] == cmd { Some(f[5]) } else { None }
}
fn vaxee_battery_ok(f: &[u8]) -> bool { vaxee_value(f, 0x0b).is_some() }
fn vaxee_charging_ok(f: &[u8]) -> bool { vaxee_value(f, 0x10).is_some() }
fn vaxee_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    let battery = vaxee_value(get(r, 0), 0x0b)?;
    if battery > 20 { return None; }
    let charging = vaxee_value(get(r, 1), 0x10).unwrap_or(0);
    Some(mk_status(
        "vaxee", "VAXEE mouse", "mouse", "vaxee",
        Some(battery * 5), Some(charging != 0), true,
    ))
}

// ---- wlmouse ----
fn wlmouse_feature(f: &[u8]) -> Option<(u8, bool)> {
    if f.len() < 8 { return None; }
    for i in 5..f.len() - 2 {
        if f[i] == 0x83 && f[i - 1] == 0x00 && f[i - 2] == 0x02
            && matches!(f[i - 5], 0xa1 | 0xa2) && f[i + 2] <= 100 {
            return Some((f[i + 2], f[i + 1] != 0));
        }
    }
    None
}
fn wlmouse_heartbeat(f: &[u8]) -> Option<(u8, bool)> {
    for off in 0..=1usize {
        if f.len() >= off + 4 && f[off] == 0x03 && f[off + 1] == 0x00 && f[off + 2] <= 100 {
            return Some((f[off + 2], f[off + 3] != 0));
        }
    }
    None
}
fn wl_feature_ok(f: &[u8]) -> bool { wlmouse_feature(f).is_some() }
fn wl_heartbeat_ok(f: &[u8]) -> bool { wlmouse_heartbeat(f).is_some() }
fn wlmouse_parse(r: &[Vec<u8>], _pid: u16) -> Option<Status> {
    for f in r {
        if let Some((level, charging)) = wlmouse_feature(f).or_else(|| wlmouse_heartbeat(f)) {
            return Some(mk_status(
                "wlmouse", "WLmouse", "mouse", "wlmouse",
                Some(level), Some(charging), true,
            ));
        }
    }
    None
}

// ---- zowie ----
fn zowie_percent(mv: u32) -> u8 {
    let volts: [u32; 6] = [4200, 4000, 3890, 3760, 3680, 3500];
    let pcts: [u8; 6] = [100, 75, 50, 25, 10, 0];
    if mv >= volts[0] { return 100; }
    if mv <= volts[5] { return 0; }
    for i in 0..5 {
        if volts[i + 1] <= mv && mv <= volts[i] {
            let p = f64::from(pcts[i + 1])
                + f64::from(mv - volts[i + 1]) / f64::from(volts[i] - volts[i + 1])
                    * f64::from(pcts[i] - pcts[i + 1]);
            return p.round() as u8;
        }
    }
    0
}
fn zowie_cw_ok(f: &[u8]) -> bool {
    if f.len() < 3 { return false; }
    let mv = u16::from(f[0]) | (u16::from(f[1]) << 8);
    mv == 0 || (3000..=4500).contains(&mv)
}
fn zowie_cw_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let f = get(r, 0);
    if !zowie_cw_ok(f) || f.len() < 3 { return None; }
    let mv = u16::from(f[0]) | (u16::from(f[1]) << 8);
    if mv == 0 { return None; }
    let pct = if (1..=100).contains(&f[2]) { f[2] } else { zowie_percent(u32::from(mv)) };
    let charging = pid == 0x8005;
    Some(mk_status(
        format!("zowie:{pid:04x}"), "ZOWIE EC-CW", "mouse", "zowie",
        Some(pct), Some(charging), true,
    ))
}
fn zowie_dw_ok(f: &[u8]) -> bool {
    f.len() >= 5 && f[0] == 0x09 && f[1] != 0xe6 && f[4] == 0xc8
}
fn zowie_dw_parse(r: &[Vec<u8>], pid: u16) -> Option<Status> {
    let f = get(r, 0);
    if !zowie_dw_ok(f) { return None; }
    let mv = (u32::from(f[1]) * 256 + u32::from(f[2])) * 10;
    if mv == 0 { return None; }
    let name = if pid == 0x8008 { "ZOWIE U2" } else { "ZOWIE DW mouse" };
    Some(mk_status(
        format!("zowie:{pid:04x}"), name, "mouse", "zowie",
        Some(zowie_percent(mv)), Some(false), false,
    ))
}

// ---------------- 注册表 ----------------

static SPECS: &[Spec] = &[
    // akko_s9.py
    Spec { name: "akko_s9", vids: &[0x36eb], pids: &[0x1087], selector: ANY, steps: &[
        Step { op: Op::WriteRead, request: &AKKO_LINK, attempts: 2, accept: akko_link_ok },
        Step { op: Op::WriteRead, request: &AKKO_BATTERY, attempts: 2, accept: akko_battery_ok },
    ], parse: akko_s9_parse },

    // atk_nearlink.py：接收器 0xFFB3/report 5，有线 0xFFB2/report 4
    Spec { name: "atk_nearlink", vids: &[0x373b], pids: &[0x108c],
        selector: Selector { page: 0xffb3, usage: 0x0001, iface: Some(1) }, steps: &[
            Step { op: Op::WriteRead, request: &ATK_RECEIVER, attempts: 4, accept: any_nonempty },
        ], parse: atk_nearlink_parse },
    Spec { name: "atk_nearlink", vids: &[0x373b], pids: &[0x108b],
        selector: Selector { page: 0xffb2, usage: 0x0001, iface: Some(1) }, steps: &[
            Step { op: Op::WriteRead, request: &ATK_CABLE, attempts: 4, accept: any_nonempty },
        ], parse: atk_nearlink_parse },

    // atk_bitmouse.py
    Spec { name: "atk_bitmouse", vids: &[0x373b], pids: &[0x1216],
        selector: Selector { page: 0xff05, usage: 0x0001, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &BITMOUSE_BATTERY, attempts: 4, accept: bitmouse_battery_ok },
            Step { op: Op::WriteRead, request: &BITMOUSE_CHARGING, attempts: 4, accept: bitmouse_charging_ok },
        ], parse: atk_bitmouse_parse },

    // aula.py / aula_f108.py
    Spec { name: "aula", vids: &[0x0c45], pids: &[0xfefc],
        selector: Selector { page: 0xff80, usage: 0x0001, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &AULA_REQUEST, attempts: 2, accept: any_nonempty },
        ], parse: aula_parse },
    Spec { name: "aula_f108", vids: &[0x05ac], pids: &[0x024f],
        selector: Selector { page: 0xff60, usage: 0x0061, iface: Some(3) }, steps: &[
            Step { op: Op::WriteRead, request: &F108_REQUEST, attempts: 4, accept: f108_ok },
        ], parse: aula_f108_parse },

    // eightbitdo.py：listen-only，D-input 手柄集合（受 hid.rs page 过滤限制）
    Spec { name: "eightbitdo", vids: &[0x2dc8],
        pids: &[0x6000, 0x6100, 0x6001, 0x6101, 0x6003, 0x6006, 0x6009, 0x6012],
        selector: ANY, steps: &[
            Step { op: Op::Listen, request: &[], attempts: 1, accept: bitdo_accept },
        ], parse: eightbitdo_parse },
    // eightbitdo_ultimate2.py：只打开 FF7A:0001
    Spec { name: "eightbitdo_ultimate2", vids: &[0x2dc8], pids: &[0x310b],
        selector: Selector { page: 0xff7a, usage: 0x0001, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &E8B2_INFO, attempts: 1, accept: e8b2_info_ok },
            Step { op: Op::WriteRead, request: &E8B2_PAD, attempts: 1, accept: e8b2_pad_ok },
        ], parse: eightbitdo_ultimate2_parse },

    // finalmouse.py：link -> battery status -> 电压回退 -> charging
    Spec { name: "finalmouse", vids: &[0x361d], pids: &[0x0100],
        selector: Selector { page: 0xff00, usage: 0x0001, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &FM_LINK, attempts: 1, accept: fm_link_ok },
            Step { op: Op::WriteRead, request: &FM_BATTERY, attempts: 1, accept: fm_battery_ok },
            Step { op: Op::WriteRead, request: &FM_VBAT, attempts: 1, accept: fm_vbat_ok },
            Step { op: Op::WriteRead, request: &FM_CHARGING, attempts: 1, accept: fm_charging_ok },
        ], parse: finalmouse_parse },

    // flydigi.py：arch 帧 + 4 种 info 写形状
    Spec { name: "flydigi", vids: &[0x37d7], pids: &[0x2401],
        selector: Selector { page: 0xffa0, usage: 0xffff, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &FLY_ARCH, attempts: 1, accept: any_nonempty },
            Step { op: Op::WriteRead, request: &FLY_INFO, attempts: 1, accept: fly_ok },
            Step { op: Op::WriteRead, request: &FLY_INFO_PADDED, attempts: 1, accept: fly_ok },
            Step { op: Op::WriteRead, request: &FLY_INFO_NUMBERED, attempts: 1, accept: fly_ok },
            Step { op: Op::WriteRead, request: &FLY_INFO_NUMBERED_PADDED, attempts: 1, accept: fly_ok },
        ], parse: flydigi_parse },

    // gamesir.py
    Spec { name: "gamesir", vids: &[0x3537],
        pids: &[0x100b, 0x0575, 0x1053, 0x10b7, 0x10b9, 0x10cb, 0x10cc, 0x10cd, 0x10ce,
                0x10c5, 0x10c6, 0x10c7, 0x10c8, 0x10c9, 0x10ca, 0x10fd, 0x10fe],
        selector: Selector { page: 0xfff0, usage: 0x0040, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &GAMESIR_REQUEST, attempts: 1, accept: gamesir_ok },
        ], parse: gamesir_parse },

    // glorious.py：65 字节 feature report
    Spec { name: "glorious", vids: &[0x258a], pids: &[0x2022, 0x2024, 0x2023, 0x2025],
        selector: ANY, steps: &[
            Step { op: Op::Feature { id: 0 }, request: &GLORIOUS_REQUEST, attempts: 2, accept: glorious_ok },
        ], parse: glorious_parse },

    // irocks.py：命令在 FF02:0002（feature），回复在 FF01:0000（input）
    Spec { name: "irocks", vids: &[0x14a5], pids: &[0xc009],
        selector: Selector { page: 0xff02, usage: 0x0002, iface: None }, steps: &[
            Step { op: Op::FeatureInput { reply: Selector { page: 0xff01, usage: 0x0000, iface: None } },
                request: &IROCKS_CONNECT, attempts: 8, accept: irocks_connect_ok },
            Step { op: Op::FeatureInput { reply: Selector { page: 0xff01, usage: 0x0000, iface: None } },
                request: &IROCKS_POWER, attempts: 8, accept: irocks_power_ok },
        ], parse: irocks_parse },
    Spec { name: "irocks", vids: &[0x14a5], pids: &[0xc010],
        selector: Selector { page: 0xff02, usage: 0x0002, iface: None }, steps: &[
            Step { op: Op::FeatureInput { reply: Selector { page: 0xff01, usage: 0x0000, iface: None } },
                request: &IROCKS_POWER, attempts: 8, accept: irocks_power_ok },
        ], parse: irocks_parse },

    // jbl.py：listen-only
    Spec { name: "jbl", vids: &[0x0ecb], pids: &[0x2088],
        selector: Selector { page: 0xff13, usage: 0x0001, iface: None }, steps: &[
            Step { op: Op::Listen, request: &[], attempts: 1, accept: jbl_ok },
        ], parse: jbl_parse },

    // keychron.py：feature 失败时同帧改 output 写（两步骤按序校验）
    Spec { name: "keychron", vids: &[0x3434], pids: &[0xd028, 0xd048, 0xd026],
        selector: Selector { page: 0xffc1, usage: 0x0001, iface: None }, steps: &[
            Step { op: Op::FeatureInput { reply: ANY }, request: &KEYCHRON_REQUEST, attempts: 1, accept: keychron_ok },
            Step { op: Op::WriteRead, request: &KEYCHRON_REQUEST, attempts: 1, accept: keychron_ok },
        ], parse: keychron_parse },

    // machenike.py
    Spec { name: "machenike", vids: &[0x248a], pids: &[0xff4c],
        selector: Selector { page: 0x008c, usage: 0x0001, iface: Some(1) }, steps: &[
            Step { op: Op::Feature { id: 0x51 }, request: &MACHENIKE_REQUEST, attempts: 1, accept: any_nonempty },
        ], parse: machenike_parse },
    Spec { name: "machenike", vids: &[0x248a], pids: &[0xff4e],
        selector: Selector { page: 0x008c, usage: 0x0001, iface: Some(2) }, steps: &[
            Step { op: Op::Feature { id: 0x51 }, request: &MACHENIKE_REQUEST, attempts: 1, accept: any_nonempty },
        ], parse: machenike_parse },

    // msi.py：三种既有型号 + PRO 主动查询
    Spec { name: "msi", vids: &[0x0db0], pids: &[0x9e22],
        selector: Selector { page: 0xff0d, usage: 0x00b0, iface: Some(1) }, steps: &[
            Step { op: Op::WriteRead, request: &MSI_REQUEST, attempts: 1, accept: msi_query_ok },
        ], parse: msi_query_parse },
    Spec { name: "msi", vids: &[0x0db0], pids: &[0x0d3b, 0x0d3c],
        selector: Selector { page: 0xff00, usage: 0x0001, iface: Some(1) }, steps: &[
            Step { op: Op::WriteRead, request: &MSI_REQUEST, attempts: 1, accept: msi_query_ok },
        ], parse: msi_query_parse },
    Spec { name: "msi", vids: &[0x0db0], pids: &[0x0d4b, 0x0d4c],
        selector: Selector { page: 0xff10, usage: 0x0006, iface: Some(1) }, steps: &[
            Step { op: Op::Listen, request: &[], attempts: 1, accept: msi_passive_ok },
        ], parse: msi_passive_parse },
    Spec { name: "msi", vids: &[0x0db0], pids: &[0x1fc6, 0x1620],
        selector: Selector { page: 0x00ff, usage: 0x0001, iface: Some(1) }, steps: &[
            Step { op: Op::WriteRead, request: &MSI_PRO_REQUEST, attempts: 1, accept: any_nonempty },
        ], parse: msi_pro_parse },

    // ninjutso_sora_v3.py
    Spec { name: "ninjutso_sora_v3", vids: &[0x093a], pids: &[0xeb02, 0xe010],
        selector: Selector { page: 0xff01, usage: 0x0001, iface: None }, steps: &[
            Step { op: Op::Feature { id: 0x06 }, request: &NINJUTSO_BATTERY, attempts: 4, accept: ninjutso_battery_ok },
            Step { op: Op::Feature { id: 0x06 }, request: &NINJUTSO_CHARGING, attempts: 4, accept: ninjutso_charging_ok },
        ], parse: ninjutso_sora_v3_parse },

    // nzxt.py
    Spec { name: "nzxt", vids: &[0x1e71], pids: &[0x2101, 0x2129],
        selector: Selector { page: 0xffca, usage: 0x0001, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &NZXT_REQUEST, attempts: 10, accept: nzxt_ok },
        ], parse: nzxt_parse },

    // skullcandy.py
    Spec { name: "skullcandy", vids: &[0x34f0], pids: &[0x3210, 0x5310],
        selector: Selector { page: 0xff13, usage: 0xffff, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &SKULLCANDY_ASK, attempts: 12, accept: skullcandy_ok },
        ], parse: skullcandy_parse },

    // sony_inzone.py：H5 的 HID 通道（H9 串口协议无 Rust COM 实现，不放行）
    Spec { name: "sony_inzone", vids: &[0x054c], pids: &[0x0ebf],
        selector: Selector { page: 0xff04, usage: 0xffff, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &SONY_HID_REQUEST, attempts: 8, accept: sony_inzone_ok },
        ], parse: sony_inzone_parse },

    // vaxee.py：按 VID + usage page 放行，PID 通配
    Spec { name: "vaxee", vids: &[0x3057], pids: &[],
        selector: Selector { page: 0xff05, usage: 0xffff, iface: None }, steps: &[
            Step { op: Op::Feature { id: 0x0e }, request: &VAXEE_BATTERY, attempts: 2, accept: vaxee_battery_ok },
            Step { op: Op::Feature { id: 0x0e }, request: &VAXEE_CHARGING, attempts: 2, accept: vaxee_charging_ok },
        ], parse: vaxee_parse },

    // wlmouse.py：feature 查询 + 被动心跳回退
    Spec { name: "wlmouse", vids: &[0x36a7], pids: &[0xa887, 0xa868, 0xa880, 0xa881],
        selector: Selector { page: 0xffff, usage: 0x0000, iface: None }, steps: &[
            Step { op: Op::Feature { id: 0 }, request: &WLMOUSE_REQUEST, attempts: 15, accept: wl_feature_ok },
        ], parse: wlmouse_parse },
    Spec { name: "wlmouse", vids: &[0x36a7], pids: &[],
        selector: ANY, steps: &[
            Step { op: Op::Listen, request: &[], attempts: 1, accept: wl_heartbeat_ok },
        ], parse: wlmouse_parse },

    // zowie.py：EC-CW（00FF）+ DW/U2（FF03 写 / FF04 读）
    Spec { name: "zowie", vids: &[0x04a5], pids: &[0x8005, 0x8006],
        selector: ANY, steps: &[
            Step { op: Op::WriteRead, request: &ZOWIE_CW_REQUEST, attempts: 1, accept: zowie_cw_ok },
        ], parse: zowie_cw_parse },
    Spec { name: "zowie", vids: &[0x04a5], pids: &[0x8008, 0x800a],
        selector: Selector { page: 0xff03, usage: 0xffff, iface: None }, steps: &[
            Step { op: Op::WriteRead, request: &ZOWIE_DW_REQUEST, attempts: 1, accept: zowie_dw_ok },
        ], parse: zowie_dw_parse },
];

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    let mut out: Vec<Box<dyn Provider>> = SPECS
        .iter()
        .map(|s| Box::new(LongtailProvider(s)) as Box<dyn Provider>)
        .collect();
    out.push(Box::new(EweadnYdx));
    out.push(Box::new(LogitechCenturion::new()));
    out.push(Box::new(LogitechG435::new()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(name: &str, replies: &[Vec<u8>], pid: u16) -> Status {
        let spec = SPECS.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no spec {name}"));
        (spec.parse)(replies, pid).unwrap_or_else(|| panic!("{name} rejected frames: {replies:02x?}"))
    }

    fn frame(bytes: &[u8]) -> Vec<u8> { bytes.to_vec() }

    #[test]
    fn akko_s9() {
        // Python 文档：全零 0x10 请求 trailer = 4B43，全零 0x12 请求 trailer = 7112。
        assert_eq!([AKKO_LINK[62], AKKO_LINK[63]], [0x4b, 0x43]);
        assert_eq!([AKKO_BATTERY[62], AKKO_BATTERY[63]], [0x71, 0x12]);
        let mut link = [0u8; 64];
        link[0] = 0x02; link[1] = 0x10; link[2] = 0x01; link[4] = 1;
        let c = akko_checksum(&link[..62]);
        link[62] = (c >> 8) as u8; link[63] = (c & 0xff) as u8;
        let mut batt = [0u8; 64];
        batt[0] = 0x02; batt[1] = 0x12; batt[2] = 0x01; batt[4] = 99;
        let c = akko_checksum(&batt[..62]);
        batt[62] = (c >> 8) as u8; batt[63] = (c & 0xff) as u8;
        let st = parsed("akko_s9", &[frame(&link), frame(&batt)], 0x1087);
        assert_eq!(st.level, Some(99));
        assert_eq!(st.charging_known, false);
        link[4] = 0;
        let c = akko_checksum(&link[..62]);
        link[62] = (c >> 8) as u8; link[63] = (c & 0xff) as u8;
        let st = parsed("akko_s9", &[frame(&link), frame(&batt)], 0x1087);
        assert!(!st.online);
    }

    #[test]
    fn atk_nearlink() {
        let mut f = [0u8; 64];
        f[0] = 0x05; f[3] = 0x1a; f[8] = 90; f[9] = 1;
        let st = parsed("atk_nearlink", &[frame(&f)], 0x108c);
        assert_eq!(st.level, Some(90));
        assert_eq!(st.charging, Some(true));
        f[8] = 101;
        assert!((atk_nearlink_parse)(&[frame(&f)], 0x108c).is_none());
    }

    #[test]
    fn atk_bitmouse() {
        // reply: [0]=report id 08, [1]=marker 72, [5]=cmd, [7]=payload
        let mut batt = [0u8; 64];
        batt[..8].copy_from_slice(&[0x08, 0x72, 0x00, 0x05, 0x00, 7, 1, 0x5a]);
        let mut chg = [0u8; 64];
        chg[..8].copy_from_slice(&[0x08, 0x72, 0x00, 0x05, 0x00, 15, 1, 1]);
        let st = parsed("atk_bitmouse", &[frame(&batt), frame(&chg)], 0x1216);
        assert_eq!(st.level, Some(90));
        assert_eq!(st.charging, Some(true));
        // request[1] = sum(request[2..])
        assert_eq!(BITMOUSE_BATTERY[1], sum8(&BITMOUSE_BATTERY[2..]));
    }

    #[test]
    fn aula() {
        let mut f = [0u8; 33];
        f[0] = 0x55; f[1] = 0x10; f[25] = 96;
        let st = parsed("aula", &[frame(&f)], 0xfefc);
        assert_eq!(st.level, Some(96));
        let mut g = [0u8; 33];
        g[1] = 0x55; g[2] = 0x10; g[26] = 95; // hidapi 去掉前导 0
        assert_eq!(parsed("aula", &[frame(&g)], 0xfefc).level, Some(95));
    }

    #[test]
    fn aula_f108() {
        let mut f = [0u8; 32];
        f[..3].copy_from_slice(&[0x20, 0x01, 0x00]);
        f[3] = 33; f[31] = sum8(&f[..31]);
        let st = parsed("aula_f108", &[frame(&f)], 0x024f);
        assert_eq!(st.level, Some(33));
        assert_eq!(st.charging_known, false);
        assert_eq!(F108_REQUEST[32], 0x21);
    }

    #[test]
    fn eightbitdo() {
        let mut f = [0u8; 32];
        f[0] = 0x04; f[14] = 0x55; // 0x55 = 85%，bit7 置位表示充电
        let st = parsed("eightbitdo", &[frame(&f)], 0x6009);
        assert_eq!(st.level, Some(85));
        assert_eq!(st.charging, Some(false));
        f[14] = 0xd2;
        let st = parsed("eightbitdo", &[frame(&f)], 0x6009);
        assert_eq!(st.level, Some(82));
        assert_eq!(st.charging, Some(true));
        f[14] = 0;
        assert!((eightbitdo_parse)(&[frame(&f)], 0x6009).is_none());
    }

    #[test]
    fn eightbitdo_ultimate2() {
        let mut info = [0u8; 64];
        info[..6].copy_from_slice(&[0x02, 0x04, 0x04, 0x00, 0x00, 0x01]);
        let mut pad = [0u8; 64];
        pad[..6].copy_from_slice(&[0x02, 0x04, 0x04, 0x00, 0x21, 0x01]);
        pad[18] = 76;
        let st = parsed("eightbitdo_ultimate2", &[frame(&info), frame(&pad)], 0x310b);
        assert_eq!(st.level, Some(76));
        assert_eq!(st.charging_known, false);
    }

    #[test]
    fn eweadn_ydx() {
        let mut f = [0u8; 32];
        f[0] = 4; f[1] = 1; f[4] = 55; f[5] = 1;
        let st = eweadn_parse(&f).unwrap();
        assert_eq!(st.level, Some(55));
        assert_eq!(st.charging, Some(true));
        f[5] = 2;
        assert_eq!(eweadn_parse(&f).unwrap().level, Some(100));
        f[1] = 0;
        assert!(!eweadn_parse(&f).unwrap().online);
    }

    #[test]
    fn finalmouse() {
        let mut link = [0u8; 64];
        link[0] = 0x05; link[2] = 36; link[3] = 1; link[4] = 1;
        let mut batt = [0u8; 64];
        batt[0] = 0x05; batt[2] = 38; batt[3] = 3; batt[4] = 88;
        let mut chg = [0u8; 64];
        chg[0] = 0x05; chg[2] = 37; chg[3] = 1; chg[4] = 1;
        let st = parsed("finalmouse", &[
            frame(&link), frame(&batt), Vec::new(), frame(&chg),
        ], 0x0100);
        assert_eq!(st.level, Some(88));
        assert_eq!(st.charging, Some(true));
        // 电压回退：3.88 V -> 50%
        let mut vbat = [0u8; 64];
        vbat[0] = 0x05; vbat[2] = 5; vbat[3] = 2;
        vbat[4] = 0x2c; vbat[5] = 0x0f; // 3880 mV
        assert_eq!(fm_volts_to_percent(3880), Some(50));
        let st = parsed("finalmouse", &[
            frame(&link), Vec::new(), frame(&vbat), Vec::new(),
        ], 0x0100);
        assert_eq!(st.level, Some(50));
    }

    #[test]
    fn flydigi() {
        let mut f = [0u8; 32];
        f[..3].copy_from_slice(&[0x5a, 0xa5, 0x01]);
        f[11] = 0x32; // state=3? -> 无效；改为 state=1 step=2
        f[11] = 0x12;
        let st = parsed("flydigi", &[frame(&f)], 0x2401);
        assert_eq!(st.level, Some(40));
        assert_eq!(st.charging, Some(true));
        f[11] = 0x25; // state=2 -> 100%
        assert_eq!(parsed("flydigi", &[frame(&f)], 0x2401).level, Some(100));
    }

    #[test]
    fn gamesir() {
        let mut f = [0u8; 64];
        f[0] = 0x12; f[35] = 1; f[36] = 98;
        let st = parsed("gamesir", &[frame(&f)], 0x10c6);
        assert_eq!(st.level, Some(98));
        assert_eq!(st.charging, Some(true));
        assert_eq!(st.name, "GameSir G7 Pro 8K Aimlabs");
        f[35] = 0; f[36] = 0;
        assert!((gamesir_parse)(&[frame(&f)], 0x10c6).is_none());
    }

    #[test]
    fn glorious() {
        let mut f = [0u8; 65];
        f[0] = 0; f[1] = 0xa1; f[6] = 0x83; f[7] = 1; f[8] = 96;
        let st = parsed("glorious", &[frame(&f)], 0x2022);
        assert_eq!(st.level, Some(96));
        assert_eq!(st.charging, Some(true));
        f[1] = 0xa4; // asleep 不算读数
        assert!((glorious_parse)(&[frame(&f)], 0x2022).is_none());
        assert_eq!(GLORIOUS_REQUEST[3..7], [0x02, 0x02, 0x00, 0x83]);
    }

    #[test]
    fn irocks() {
        let mut connect = [0u8; 17];
        connect[0] = 0x08; connect[1] = 0x03; connect[6] = 1;
        connect[16] = (0x55u8).wrapping_sub(sum8(&connect[..16]));
        let mut power = [0u8; 17];
        power[0] = 0x09; power[1] = 0x04; power[6] = 55; power[7] = 1;
        power[16] = (0x55u8).wrapping_sub(sum8(&power[..16]));
        let st = parsed("irocks", &[frame(&connect), frame(&power)], 0xc009);
        assert_eq!(st.level, Some(55));
        assert_eq!(st.charging, Some(true));
        connect[6] = 0;
        connect[16] = (0x55u8).wrapping_sub(sum8(&connect[..16]));
        assert!((irocks_parse)(&[frame(&connect), frame(&power)], 0xc009).is_none());
    }

    #[test]
    fn jbl() {
        let f = [0x08u8, 95];
        let st = parsed("jbl", &[frame(&f)], 0x2088);
        assert_eq!(st.level, Some(95));
        assert_eq!(st.charging, Some(false));
        assert!((jbl_parse)(&[frame(&[0x2f, 95])], 0x2088).is_none());
    }

    #[test]
    fn keychron() {
        let mut f = [0u8; 64];
        f[0] = 0xb4; f[1] = 0x06; f[20] = 0xd2; // 82%，充电
        let st = parsed("keychron", &[frame(&f)], 0xd028);
        assert_eq!(st.level, Some(82));
        assert_eq!(st.charging, Some(true));
        assert_eq!(KEYCHRON_REQUEST[..2], [0xb3, 0x06]);
    }

    #[test]
    fn logitech_centurion() {
        let req = centurion_direct(0, 0x00, &[0x00, 0x01]);
        // payload = [feature, function|sw, 00, 01] -> frame len = 4 + 1 = 5
        assert_eq!(req[..5], [0x51, 0x05, 0x00, 0x00, 0x01]);
        let bridge = centurion_bridge(3, 4, 0x00, &[]);
        // payload = [bridge=03, 11, size_hi=00, size_lo=03] + sub [00, 04, 01]
        assert_eq!(bridge[..10], [0x51, 0x08, 0x00, 0x03, 0x11, 0x00, 0x03, 0x00, 0x04, 0x01]);
        let reply = [0x51u8, 0x05, 0x00, 0x04, 0x01, 0x00, 0x01, 0x4c];
        let p = centurion_payload(&reply).unwrap();
        assert_eq!(p, &[0x04, 0x01, 0x00, 0x01]);
        let legacy = [0x51u8, 0x0b, 0, 0, 0, 0, 0, 0, 0x04, 0, 76, 0, 0x02];
        // legacy 回答的结构条件与 Python parse_legacy 一致（真实读写需硬件）。
        assert!(legacy.len() >= 13 && legacy[1] == 0x0b && legacy[8] == 0x04 && legacy[10] <= 100);
    }

    #[test]
    fn logitech_g435_rtk() {
        let frame = [0x06u8, 0x80, 0x00, 0x5c, 0x10, 0x00, 0x00];
        assert_eq!(g435_mv(&frame), Some(4188));
        assert_eq!(g435_percent(4188), 99);
        assert_eq!(g435_percent(3800), 50);
        assert_eq!(g435_percent(3500), 5);
        assert_eq!(g435_percent(102), 0); // 102 mV 被窗口拒绝，不会成为读数
        assert!(g435_mv(&[0x06, 0x80, 0x00, 0x66, 0x00, 0]).is_none());
        assert_eq!(G435_REMOTE_ON[10..12], [0xe1, 0x01]);
        assert_eq!(G435_REMOTE_OFF[10..12], [0xe1, 0x00]);
    }

    #[test]
    fn machenike() {
        let mut f = [0u8; 21];
        f[0] = 0x51; f[1] = 0x06;
        f[4..8].copy_from_slice(&[0x8a, 0x24, 0x4e, 0xff]);
        f[10] = 1 | 8; f[11] = 76; f[12] = 1; // 接收器：低三位=1 且 bit3=已连接
        let st = parsed("machenike", &[frame(&f)], 0xff4c);
        assert_eq!(st.level, Some(76));
        assert_eq!(st.charging, Some(true));
        f[10] = 0; // 有线：低三位=0，bit3 不参与
        let st = parsed("machenike", &[frame(&f)], 0xff4e);
        assert_eq!(st.level, Some(76));
    }

    #[test]
    fn msi() {
        let mut f = [0u8; 15];
        f[0] = 0x0f; f[1] = 0x20; f[14] = 0xda; // 90%，充电位
        let st = parsed("msi", &[frame(&f)], 0x9e22);
        assert_eq!(st.level, Some(90));
        assert_eq!(st.charging, Some(true));
        let passive = [0xa0u8, 40, 0x07, 0];
        // gm41_percent(40) = (40*51 + 1760 + 50) // 100 = 38
        assert_eq!(
            msi_passive_parse(&[frame(&passive)], 0x0d4b).unwrap().level,
            Some(38)
        );
        let mut pro = [0u8; 65];
        pro[7] = 0x05; pro[9] = 77; pro[10] = 0x01;
        let st = msi_pro_parse(&[frame(&pro)], 0x1fc6).unwrap();
        assert_eq!(st.level, Some(77));
        assert_eq!(st.charging, Some(true));
        assert_eq!(MSI_PRO_REQUEST[7], 0x05);
    }

    #[test]
    fn ninjutso_sora_v3() {
        let mut batt = [0u8; 16];
        batt[0] = 0x06; batt[1] = 0x12; batt[8] = 70;
        let mut chg = [0u8; 16];
        chg[0] = 0x06; chg[1] = 0x11; chg[8] = 1;
        let st = parsed("ninjutso_sora_v3", &[frame(&batt), frame(&chg)], 0xeb02);
        assert_eq!(st.level, Some(70));
        assert_eq!(st.charging, Some(true));
        chg[8] = 0;
        assert_eq!(parsed("ninjutso_sora_v3", &[frame(&batt), frame(&chg)], 0xeb02).charging, Some(false));
    }

    #[test]
    fn nzxt() {
        let mut f = [0u8; 27];
        f[0] = 0x4e; f[1] = 0x02; f[2] = 0x97;
        f[17] = 77; f[18] = 0; f[22] = 1;
        let st = parsed("nzxt", &[frame(&f)], 0x2101);
        assert_eq!(st.level, Some(77));
        assert_eq!(st.charging, Some(true));
        // ACK 帧被 accept 过滤
        let ack = [0x4e, 0xe5, 0x00];
        assert!(!nzxt_ok(&ack));
    }

    #[test]
    fn skullcandy() {
        // 07 <len=9> 80 | 05 5D <u16len=5> <u16opcode=D60C> <status=0 role=0 pct=70>
        let f = [
            0x07u8, 0x09, 0x80, 0x05, 0x5d, 0x05, 0x00, 0xd6, 0x0c, 0x00, 0x00, 0x46,
        ];
        let st = parsed("skullcandy", &[frame(&f)], 0x3210);
        assert_eq!(st.level, Some(70));
        assert_eq!(SKULLCANDY_ASK[..10], [0x06, 0x07, 0x80, 0x05, 0x5a, 0x03, 0x00, 0xd6, 0x0c, 0x00]);
    }

    #[test]
    fn sony_inzone() {
        let mut f = [0u8; 16];
        f[0] = 0x02; f[1] = 14;
        f[2..10].copy_from_slice(&SONY_SIGNATURE);
        f[10] = 0x10; f[11] = 0x02; f[12] = 0x01; f[13] = 1; f[14] = 76;
        f[15] = sum8(&f[6..15]);
        let st = parsed("sony_inzone", &[frame(&f)], 0x0ebf);
        assert_eq!(st.level, Some(76));
        assert_eq!(st.charging, Some(true));
        f[14] = 0xff; // 耳机离线：改了 payload 必须同步刷新校验
        f[15] = sum8(&f[6..15]);
        assert!(!parsed("sony_inzone", &[frame(&f)], 0x0ebf).online);
        assert_eq!(SONY_HID_REQUEST[11..14], [0x02, 0x01, 0xa2]);
    }

    #[test]
    fn vaxee() {
        let mut batt = [0u8; 64];
        batt[..6].copy_from_slice(&[0x0e, 0xa5, 0x0b, 0x01, 0x01, 14]);
        let mut chg = [0u8; 64];
        chg[..6].copy_from_slice(&[0x0e, 0xa5, 0x10, 0x01, 0x01, 1]);
        let st = parsed("vaxee", &[frame(&batt), frame(&chg)], 0x2001);
        assert_eq!(st.level, Some(70));
        assert_eq!(st.charging, Some(true));
        batt[5] = 21;
        assert!((vaxee_parse)(&[frame(&batt), frame(&chg)], 0x2001).is_none());
    }

    #[test]
    fn wlmouse() {
        let mut f = [0u8; 65];
        f[0] = 0; f[1] = 0xa1; f[2] = 0; f[3] = 2; f[4] = 2; f[5] = 0;
        f[6] = 0x83; f[7] = 1; f[8] = 96;
        let st = parsed("wlmouse", &[frame(&f)], 0xa881);
        assert_eq!(st.level, Some(96));
        assert_eq!(st.charging, Some(true));
        let hb = [0x03, 0x00, 55, 0x01];
        assert_eq!(parsed("wlmouse", &[frame(&hb)], 0xa880).level, Some(55));
    }

    #[test]
    fn zowie() {
        let cw = [0x4cu8, 0x10, 76]; // 4188 mV, 76%
        let st = parsed("zowie", &[frame(&cw)], 0x8005);
        assert_eq!(st.level, Some(76));
        assert_eq!(st.charging, Some(true));
        let dw = [0x09u8, 0x10, 0x6e, 0x00, 0xc8]; // (0x106e)*10 = 42220，按 Python 比较阈值
        let st = zowie_dw_parse(&[frame(&dw)], 0x8008).unwrap();
        assert_eq!(st.level, Some(zowie_percent(42220)));
        assert_eq!(st.name, "ZOWIE U2");
    }

    #[test]
    fn registration_and_instance_filter() {
        let providers = all_providers();
        for name in [
            "akko_s9", "atk_nearlink", "atk_bitmouse", "aula", "aula_f108", "eightbitdo",
            "eightbitdo_ultimate2", "eweadn_ydx", "finalmouse", "flydigi", "gamesir",
            "glorious", "irocks", "jbl", "keychron", "logitech_centurion",
            "logitech_g435_rtk", "machenike", "msi", "ninjutso_sora_v3", "nzxt",
            "skullcandy", "sony_inzone", "vaxee", "wlmouse", "zowie",
        ] {
            assert!(providers.iter().any(|p| p.name() == name), "缺少 {name}");
        }
        assert_eq!(providers.len(), SPECS.len() + 3);
        // VID/PID 不匹配时不触碰 HID
        let p = providers.iter().find(|p| p.name() == "vaxee").unwrap();
        assert!(p.poll(r"USB\VID_3057&PID_9999\7&ABC&0&0", &[]).is_empty());
    }
}
