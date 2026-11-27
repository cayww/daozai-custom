//! §4.3「产品字符串放行」与 §4.4「特殊集合」协议：完整静态移植。
//!
//! 本文件严格只新增，不改动 `hid.rs` / `providers.rs`。所有请求字节、索引、
//! VID/PID、product string 匹配与校验均逐字来自
//! `vendor/halo-battery/upstream/providers/` 同名 Python 源码，未做任何猜测。
//!
//! 调用方已在 `hid.rs` 为 `Collection` 增加 `product` 字段，并放行 JBL Quantum
//! 350 / MOONDROP 的非 vendor 集合（同时保留其 output/feature 长度），因此下列
//! 协议全部可注册：
//!
//! - `gs3075t`、`attackshark_x66`：共用 0C45:FEFE / FF60:0061 / iface 3，
//!   以 product string 分流。这里额外用回复签名交叉校验，避免串读。
//! - `urx`：36B0:3002 / iface 1 / FF60:0061，product string 规范化后必须等于
//!   `"urx core75 wireless"`（EVO75 共享 VID/PID，不可放行）。
//! - `redsquare`：342D:E411 / FF60:0061，product string strip 后必须等于
//!   `"RSQ_20055_2.4G"`（同 VID/PID 的其它 Red Square 型号不可放行）。
//! - `atk_keyboard`：320F:5088 / 320F:5055 / FF1C:0092，product string 决定
//!   ATK V75X 或 VGN N75 PRO 的 label/key。
//! - `jbl_quantum350`：0ECB:206B / iface 3 / 0001:0000，两个 feature report。
//! - `moondrop`：35D8:0133 / iface 3 / 000C，0x4B 写读命令通道。
//!
//! Python 侧「无回复时保留上次读数」的跨 poll 状态机（asleep keep / REFRESH /
//! RETRY）不在本文件复刻：Rust `Provider::poll(&self, ..)` 无可变状态，且
//! `Engine` 不提供「无回复时沿用上次观测」的语义。无回复时只给出 offline
//! 状态，属已知行为偏差，不影响请求与回复的字节正确性。

use crate::hid::{Collection, Handle};
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

/// 单次 poll 的 HID I/O 窗口。Python 侧分别用 0.75s / 2.4s 的多次读取；
/// Rust 引擎整轮有 20s 上限，这里压缩为 750ms 并在窗口内多次短读。
const IO_TIMEOUT: Duration = Duration::from_millis(750);
const READ_SLICE: Duration = Duration::from_millis(150);

type ParseFn = fn(&[u8], u16) -> Option<Status>;

/// 共享 Sonix dongle（0C45:FEFE, FF60:0061, iface 3）的只读电量查询。
/// 请求固定，按回复签名区分型号。
struct SonixProvider {
    name: &'static str,
    request: &'static [u8],
    parse: ParseFn,
}

impl Provider for SonixProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != 0x0c45 || pid != 0xfefe {
            return Vec::new();
        }

        for col in collections.iter().filter(|c| {
            c.usage_page == 0xff60
                && c.usage == 0x0061
                && c.interface.is_none_or(|i| i == 3)
                && c.input_len > 0
        }) {
            let Some(mut handle) = col.open() else { continue };
            let overall = Instant::now() + IO_TIMEOUT;
            if !handle.write(self.request, overall) {
                continue;
            }
            // Python 每轮重试多次：首包可能只是唤醒应答或短 ACK。
            while Instant::now() < overall {
                let Some(frame) = handle.read(Instant::now() + READ_SLICE) else {
                    continue;
                };
                if let Some(status) = (self.parse)(&frame, pid) {
                    return vec![status];
                }
            }
        }
        Vec::new()
    }
}

// ---- 请求常量（严格来自 Python 源码）----

// gs3075t.py: write_frame() = [0x00 报告 id] + request() = 00 AA 10 18 + 29 个零。
// 直接对应 hidapi 的 33 字节写入（32 字节线上帧 + 前置报告 id）。
const fn gs3075t_request() -> [u8; 33] {
    let mut f = [0u8; 33];
    f[1] = 0xaa; // 帧头
    f[2] = 0x10; // CMD_BATTERY
    f[3] = 0x18; // FRAME_TYPE
    f
}

// attackshark_x66.py: make_request() = 00 | 20 01 | 补零 | 前 32 字节的字节和。
const fn x66_request() -> [u8; 33] {
    let mut f = [0u8; 33];
    f[1] = 0x20;
    f[2] = 0x01;
    let mut sum = 0u16;
    let mut i = 0;
    while i < 32 {
        sum += f[i] as u16;
        i += 1;
    }
    f[32] = (sum & 0xff) as u8; // = 0x21
    f
}

const GS3075T_REQUEST: [u8; 33] = gs3075t_request();
const X66_REQUEST: [u8; 33] = x66_request();

// ---- 解析函数（严格照搬 Python 的索引与校验）----

/// gs3075t.py `parse_reply`：hidapi 可能保留或剥掉报告 id，所以尝试
/// lead=0（32 字节帧）和 lead=1（33 字节帧）两种布局。
fn gs3075t_parse(f: &[u8], _pid: u16) -> Option<Status> {
    for (lead, level_at, charge_at) in [(0usize, 25usize, 26usize), (1, 26, 27)] {
        if f.len() <= charge_at || f[lead] != 0x55 || f[lead + 1] != 0x10 {
            continue;
        }
        let level = f[level_at];
        if !(1..=100).contains(&level) {
            continue;
        }
        // 0x02 充电中；0x01 插着线且已满。两者都算「接外部电源」。
        let charging = matches!(f[charge_at], 0x01 | 0x02);
        let mut st = status(
            "gs3075t:fefe",
            "GANSS GS3075T",
            "keyboard",
            "gs3075t",
        );
        st.level = Some(level);
        st.charging = Some(charging);
        return Some(st);
    }
    None
}

/// attackshark_x66.py `parse_reply`：先剥掉 Windows 保留的 0 报告 id，
/// 然后要求 `20 01 00` 前缀，电量在 byte 3；不校验尾部字节和。
fn x66_reply_data(f: &[u8]) -> &[u8] {
    if f.len() == 33 && f[0] == 0x00 {
        &f[1..]
    } else {
        f
    }
}

fn x66_parse(f: &[u8], _pid: u16) -> Option<Status> {
    let data = x66_reply_data(f);
    if data.len() < 5 || data[..3] != [0x20, 0x01, 0x00] {
        return None;
    }
    let level = data[3];
    if level > 100 {
        return None;
    }
    let mut st = status(
        "attackshark:x66",
        "ATTACKSHARK X66",
        "keyboard",
        "attackshark_x66",
    );
    st.level = Some(level);
    st.charging = None;
    st.charging_known = false;
    Some(st)
}

fn status(key: &'static str, name: &'static str, kind: &'static str, source: &'static str) -> Status {
    let mut st = Status::new(key, name);
    st.kind = kind.into();
    st.source = source.into();
    st
}

/// 打开集合、写一次请求、在窗口内短读直到 `parse` 接受某帧。
/// 泛型返回便于各协议复用，同时让每个 parse 保持纯函数（便于单测）。
fn write_read<T>(
    collection: &Collection,
    request: &[u8],
    timeout: Duration,
    read_slice: Duration,
    mut parse: impl FnMut(&[u8]) -> Option<T>,
) -> Option<T> {
    let mut handle = collection.open()?;
    let overall = Instant::now() + timeout;
    if !handle.write(request, overall) {
        return None;
    }
    while Instant::now() < overall {
        let Some(frame) = handle.read(Instant::now() + read_slice) else {
            continue;
        };
        if let Some(value) = parse(&frame) {
            return Some(value);
        }
    }
    None
}

// ==================== urx（§4.3）====================

/// urx.py `_core75_model`：`" ".join(product.split()).casefold()` 全等匹配。
/// 产品名为 ASCII，`to_lowercase()` 与 `casefold()` 等价。
fn urx_product_matches(product: &str) -> bool {
    product
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        == "urx core75 wireless"
}

// urx.py `make_request()`：00 | 08 00 07 | 补零，共 33 字节。
const URX_REQUEST: [u8; 33] = urx_request();
const fn urx_request() -> [u8; 33] {
    let mut f = [0u8; 33];
    f[1] = 0x08; // VIA_CUSTOM_GET
    f[2] = 0x00; // CHANNEL
    f[3] = 0x07; // VALUE_BATTERY
    f
}

/// urx.py `parse_reply`：hidapi 可能保留或剥掉报告 id，返回 level。
/// 充电状态在该板上不可用，调用方置 `charging_known = false`。
fn urx_parse(f: &[u8]) -> Option<u8> {
    if f.len() < 32 {
        return None;
    }
    let start = if f.len() > 3 && f[0] == 0x08 && f[1] == 0x00 && f[2] == 0x07 {
        0
    } else if f.len() >= 33 && f[0] == 0x00 && f[1] == 0x08 && f[2] == 0x00 && f[3] == 0x07 {
        1
    } else {
        return None;
    };
    let level = f[start + 3];
    (level <= 100).then_some(level)
}

struct UrxProvider;

impl Provider for UrxProvider {
    fn name(&self) -> &'static str {
        "urx"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != 0x36b0 || pid != 0x3002 {
            return Vec::new();
        }
        let mut found = false;
        for col in collections.iter().filter(|c| {
            c.usage_page == 0xff60
                && c.usage == 0x0061
                && c.interface == Some(1)
                && urx_product_matches(&c.product)
                && c.output_len >= 33
                && c.input_len >= 32
        }) {
            found = true;
            // Python 6 次 × 250ms 读取，这里给足 1500ms 窗口。
            let Some(level) = write_read(
                col,
                &URX_REQUEST,
                Duration::from_millis(1500),
                READ_SLICE,
                urx_parse,
            ) else {
                continue;
            };
            let mut st = status("urx:3002", "URX Core75", "keyboard", "urx");
            st.level = Some(level);
            st.charging_known = false;
            return vec![st];
        }
        if found {
            // 键盘休眠/关机：Python 会在 300s 内保留上次读数，Rust 无跨 poll 状态。
            let mut st = status("urx:3002", "URX Core75", "keyboard", "urx");
            st.level = None;
            st.charging_known = false;
            st.online = false;
            return vec![st];
        }
        Vec::new()
    }
}

// ==================== redsquare（§4.3）====================

/// redsquare.py `pick()`：product string 只做 `.strip()` 全等匹配。
fn redsquare_product_matches(product: &str) -> bool {
    product.trim() == "RSQ_20055_2.4G"
}

// redsquare.py `make_request()`：07 | 05 01 01 0A 07 8B | 补零，共 33 字节。
const REDSQUARE_REQUEST: [u8; 33] = redsquare_request();
const fn redsquare_request() -> [u8; 33] {
    let mut f = [0u8; 33];
    f[0] = 0x07; // REPORT_ID
    f[1] = 0x05; // payload[0]
    f[2] = 0x01;
    f[3] = 0x01;
    f[4] = 0x0a;
    f[5] = 0x07; // REPORT_ID（payload[4]）
    f[6] = 0x8b; // CMD_SETTINGS
    f
}

/// redsquare.py `parse_reply`：先按 `_without_report_id` 剥掉 `07 05`，
/// 再校验 `05 A1 01 0A 07 8B`，level 在 data[8]，范围 1..=100。
fn redsquare_parse(f: &[u8]) -> Option<u8> {
    let data = if f.len() >= 2 && f[0] == 0x07 && f[1] == 0x05 {
        &f[1..]
    } else {
        f
    };
    if data.len() < 9 || data[..6] != [0x05, 0xa1, 0x01, 0x0a, 0x07, 0x8b] {
        return None;
    }
    let level = data[8];
    (1..=100).contains(&level).then_some(level)
}

struct RedSquareProvider;

impl Provider for RedSquareProvider {
    fn name(&self) -> &'static str {
        "redsquare"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != 0x342d || pid != 0xe411 {
            return Vec::new();
        }
        let mut found = false;
        for col in collections.iter().filter(|c| {
            c.usage_page == 0xff60
                && c.usage == 0x0061
                && redsquare_product_matches(&c.product)
                && c.output_len >= 33
                && c.input_len >= 9
        }) {
            found = true;
            let Some(level) = write_read(
                col,
                &REDSQUARE_REQUEST,
                IO_TIMEOUT,
                READ_SLICE,
                redsquare_parse,
            ) else {
                continue;
            };
            let mut st = status("redsquare:20055", "Red Square Alumix TKL", "keyboard", "redsquare");
            st.level = Some(level);
            st.charging_known = false;
            return vec![st];
        }
        if found {
            let mut st = status("redsquare:20055", "Red Square Alumix TKL", "keyboard", "redsquare");
            st.level = None;
            st.charging_known = false;
            st.online = false;
            return vec![st];
        }
        Vec::new()
    }
}

// ==================== atk_keyboard（§4.3）====================

/// atk_keyboard.py `PRODUCT_NAMES` / `PRODUCT_KEYS` / `WIRED_PRODUCT`：
/// product string（strip + casefold）→ (label, key)。
fn atk_identity(pid: u16, product: &str) -> Option<(&'static str, &'static str)> {
    let name = product.trim().to_lowercase();
    match (pid, name.as_str()) {
        (0x5055, "atk v75x") | (0x5088, "atk v75x 2.4g dongle") => {
            Some(("ATK V75X", "atk_keyboard:320f:5088"))
        }
        (0x5088, "vgn n75 pro 2.4g dongle") => {
            Some(("VGN N75 PRO", "atk_keyboard:320f:5088:vgn-n75-pro"))
        }
        _ => None,
    }
}

/// atk_keyboard.py `_read()`：04 | seq_lo seq_hi | 1A | 28 个零，共 32 字节。
fn atk_request(seq: u16) -> [u8; 32] {
    let mut f = [0u8; 32];
    f[0] = 0x04; // REPORT_ID
    f[1] = (seq & 0xff) as u8;
    f[2] = (seq >> 8) as u8;
    f[3] = 0x1a; // CMD_BATTERY
    f
}

/// atk_keyboard.py `battery_reply()`：校验报告 id、序号、命令与 5..7 零位；
/// level/state 在 8/9，state 1/2 表示充电中。
fn atk_parse(f: &[u8], seq: u16) -> Option<(u8, bool)> {
    if f.len() < 10
        || f[0] != 0x04
        || f[1] != (seq & 0xff) as u8
        || f[2] != (seq >> 8) as u8
        || f[3] != 0x1a
        || f[5] != 0
        || f[6] != 0
    {
        return None;
    }
    let level = f[8];
    let state = f[9];
    if level > 100 || state > 2 {
        return None;
    }
    Some((level, state == 1 || state == 2))
}

struct AtkKeyboardProvider {
    seq: AtomicU16,
}

impl Provider for AtkKeyboardProvider {
    fn name(&self) -> &'static str {
        "atk_keyboard"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != 0x320f || (pid != 0x5088 && pid != 0x5055) {
            return Vec::new();
        }
        for col in collections.iter().filter(|c| {
            c.usage_page == 0xff1c && c.usage == 0x0092 && c.output_len >= 32 && c.input_len >= 10
        }) {
            let Some((label, key)) = atk_identity(pid, &col.product) else {
                continue;
            };
            // Python: seq = seq % 0xFFFF + 1（从 1 开始，循环到 0xFFFF）。
            let seq = self.seq.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
            let seq = if seq == 0 { 1 } else { seq };
            let request = atk_request(seq);
            let Some((level, charging)) = write_read(
                col,
                &request,
                Duration::from_millis(1000),
                READ_SLICE,
                |f| atk_parse(f, seq),
            ) else {
                let mut st = status(key, label, "keyboard", "atk_keyboard");
                st.level = None;
                st.online = false;
                st.approximate = true; // 无有效回复，沿用末次读数不再刷新
                return vec![st];
            };
            let mut st = status(key, label, "keyboard", "atk_keyboard");
            st.level = Some(level);
            st.charging = Some(charging);
            return vec![st];
        }
        Vec::new()
    }
}

// ==================== jbl_quantum350（§4.4）====================

const JBL_REPORT_LINK: u8 = 0x68;
const JBL_REPORT_LEVEL: u8 = 0x49;

/// jbl_quantum350.py `feature_byte()`：Windows hidapi 把报告 id 放在 byte 0，
/// 只接受恰好 2 字节、且首字节等于请求 id 的回复。
fn jbl_feature_byte(report: &[u8], report_id: u8) -> Option<u8> {
    if report.len() != 2 || report[0] != report_id {
        return None;
    }
    Some(report[1])
}

/// jbl_quantum350.py `_read()` 的判定：link 必须为 1，level 在 0..=100。
fn jbl_decide(link: Option<u8>, level: Option<u8>) -> Option<u8> {
    if link? != 1 {
        return None;
    }
    let level = level?;
    (level <= 100).then_some(level)
}

struct JblQuantum350Provider;

impl Provider for JblQuantum350Provider {
    fn name(&self) -> &'static str {
        "jbl_quantum350"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != 0x0ecb || pid != 0x206b {
            return Vec::new();
        }
        for col in collections.iter().filter(|c| {
            c.usage_page == 0x0001
                && c.usage == 0x0000
                && c.interface == Some(3)
                && c.feature_len >= 2
        }) {
            let Some(mut handle) = col.open() else { continue };
            let mut level = None;
            for _ in 0..2 {
                let Some(link_reply) =
                    handle.get_feature(JBL_REPORT_LINK, Instant::now() + READ_SLICE)
                else {
                    continue;
                };
                let Some(link) = jbl_feature_byte(&link_reply, JBL_REPORT_LINK) else {
                    continue;
                };
                if link != 1 {
                    break; // 头显离线，Python 立即返回 None
                }
                let Some(level_reply) =
                    handle.get_feature(JBL_REPORT_LEVEL, Instant::now() + READ_SLICE)
                else {
                    continue;
                };
                let Some(value) = jbl_feature_byte(&level_reply, JBL_REPORT_LEVEL) else {
                    continue;
                };
                if let Some(v) = jbl_decide(Some(link), Some(value)) {
                    level = Some(v);
                    break;
                }
            }
            let mut st = status("jbl:206b", "JBL Quantum 350 Wireless", "headset", "jbl");
            st.level = level;
            st.charging_known = false;
            st.online = level.is_some();
            return vec![st];
        }
        Vec::new()
    }
}

// ==================== moondrop（§4.4）====================

const MOONDROP_REPORT: u8 = 0x4b;
const MOONDROP_READ: u8 = 0x80;
const MOONDROP_CMD_BATTERY: u8 = 0x27;
const MOONDROP_CMD_LINK: u8 = 0x31;

/// moondrop.py `_query()`：4B 80 <cmd> 00 + 补零到输出报告长度。
fn moondrop_request(cmd: u8, out_len: usize) -> Vec<u8> {
    let mut req = vec![MOONDROP_REPORT, MOONDROP_READ, cmd, 0x00];
    req.resize(out_len.max(4), 0);
    req
}

/// moondrop.py `parse_battery()`：len>=10、4B 80 27、len>=6、level<=100，
/// 返回 (link, level)。
fn moondrop_parse_battery(f: &[u8]) -> Option<(bool, u8)> {
    if f.len() < 10
        || f[0] != MOONDROP_REPORT
        || f[1] != MOONDROP_READ
        || f[2] != MOONDROP_CMD_BATTERY
        || f[3] < 6
        || f[9] > 100
    {
        return None;
    }
    Some((f[8] != 0, f[9]))
}

/// moondrop.py `parse_link()`：len>=5、4B 80 31、len>=1，返回 link。
fn moondrop_parse_link(f: &[u8]) -> Option<bool> {
    if f.len() < 5
        || f[0] != MOONDROP_REPORT
        || f[1] != MOONDROP_READ
        || f[2] != MOONDROP_CMD_LINK
        || f[3] < 1
    {
        return None;
    }
    Some(f[4] != 0)
}

/// moondrop.py `_query()` 的写读循环：写命令后在监听窗口内等到同命令回复。
fn moondrop_query(
    handle: &mut Handle,
    cmd: u8,
    out_len: usize,
    deadline: Instant,
) -> Option<Vec<u8>> {
    let request = moondrop_request(cmd, out_len);
    if !handle.write(&request, deadline) {
        return None;
    }
    while Instant::now() < deadline {
        let Some(frame) = handle.read(Instant::now() + READ_SLICE) else {
            continue;
        };
        if frame.len() >= 3 && frame[..3] == [MOONDROP_REPORT, MOONDROP_READ, cmd] {
            return Some(frame);
        }
    }
    None
}

fn moondrop_status(link: bool, level: Option<u8>) -> Status {
    let mut st = status("moondrop:gm01", "MOONDROP GM01 Pro", "headset", "moondrop");
    st.level = if link { level } else { None };
    st.charging = Some(false); // 该固件没有充电状态字节
    st.online = link;
    st
}

struct MoondropProvider;

impl Provider for MoondropProvider {
    fn name(&self) -> &'static str {
        "moondrop"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != 0x35d8 || pid != 0x0133 {
            return Vec::new();
        }
        for col in collections.iter().filter(|c| {
            c.usage_page == 0x000c && c.interface == Some(3) && c.output_len >= 4 && c.input_len >= 10
        }) {
            let Some(mut handle) = col.open() else { continue };
            // Python 的每次 _query 各有 400ms 监听窗口（LISTEN_MS）。
            let deadline = Instant::now() + Duration::from_millis(400);
            if let Some(reply) = moondrop_query(
                &mut handle,
                MOONDROP_CMD_BATTERY,
                col.output_len,
                deadline,
            ) {
                if let Some((link, level)) = moondrop_parse_battery(&reply) {
                    return vec![moondrop_status(link, Some(level))];
                }
            }
            let deadline = Instant::now() + Duration::from_millis(400);
            if let Some(reply) = moondrop_query(
                &mut handle,
                MOONDROP_CMD_LINK,
                col.output_len,
                deadline,
            ) {
                if let Some(link) = moondrop_parse_link(&reply) {
                    return vec![moondrop_status(link, None)];
                }
            }
            // 无控制回复：Python 靠路径无 &colNN 判断线缆并报充电；Rust 的
            // Collection 不暴露路径，无法等价判断，保守给出离线状态。
            return vec![moondrop_status(false, None)];
        }
        Vec::new()
    }
}

/// 统一注册入口，供调用方汇总到 `providers::all_providers()`。
pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(SonixProvider {
            name: "gs3075t",
            request: &GS3075T_REQUEST,
            parse: gs3075t_parse,
        }),
        Box::new(SonixProvider {
            name: "attackshark_x66",
            request: &X66_REQUEST,
            parse: x66_parse,
        }),
        Box::new(UrxProvider),
        Box::new(RedSquareProvider),
        Box::new(AtkKeyboardProvider { seq: AtomicU16::new(0) }),
        Box::new(JblQuantum350Provider),
        Box::new(MoondropProvider),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gs3075t_request_and_reply() {
        // 请求：00 报告 id + AA 10 18 + 补零，共 33 字节。
        assert_eq!(GS3075T_REQUEST[..4], [0x00, 0xaa, 0x10, 0x18]);
        assert_eq!(GS3075T_REQUEST.len(), 33);
        assert!(GS3075T_REQUEST[4..].iter().all(|&b| b == 0));

        // Windows 保留报告 id 的 33 字节回复：55 在 byte 1，level 在 26。
        let mut f = [0u8; 33];
        f[1] = 0x55;
        f[2] = 0x10;
        f[26] = 76;
        f[27] = 0x02;
        let st = gs3075t_parse(&f, 0xfefe).expect("33-byte reply must parse");
        assert_eq!(st.level, Some(76));
        assert_eq!(st.charging, Some(true));
        assert_eq!(st.key, "gs3075t:fefe");
        assert_eq!(st.kind, "keyboard");

        // hidapi 剥掉报告 id 的 32 字节回复：55 在 byte 0，level 在 25。
        let mut g = [0u8; 32];
        g[0] = 0x55;
        g[1] = 0x10;
        g[25] = 42;
        g[26] = 0x00;
        let st = gs3075t_parse(&g, 0xfefe).expect("32-byte reply must parse");
        assert_eq!(st.level, Some(42));
        assert_eq!(st.charging, Some(false));

        // 插线且满电（0x01）也算 charging。
        g[26] = 0x01;
        assert_eq!(gs3075t_parse(&g, 0xfefe).unwrap().charging, Some(true));
        // 电量越界必须拒绝。
        g[25] = 101;
        assert!(gs3075t_parse(&g, 0xfefe).is_none());
    }

    #[test]
    fn attackshark_x66_request_and_reply() {
        // 请求：00 | 20 01 | 补零 | 前 32 字节之和（0x21）。
        assert_eq!(X66_REQUEST[..3], [0x00, 0x20, 0x01]);
        assert_eq!(X66_REQUEST[32], 0x21);
        assert!(X66_REQUEST[3..32].iter().all(|&b| b == 0));

        // 33 字节回复（前置报告 id 0）：剥掉 id 后 20 01 00 <level>。
        let mut f = [0u8; 33];
        f[1..4].copy_from_slice(&[0x20, 0x01, 0x00]);
        f[4] = 55;
        let st = x66_parse(&f, 0xfefe).expect("33-byte reply must parse");
        assert_eq!(st.level, Some(55));
        assert_eq!(st.charging, None);
        assert!(!st.charging_known);
        assert_eq!(st.key, "attackshark:x66");

        // 32 字节回复（无报告 id）：level 在 byte 3。
        let mut g = [0u8; 32];
        g[..3].copy_from_slice(&[0x20, 0x01, 0x00]);
        g[3] = 100;
        assert_eq!(x66_parse(&g, 0xfefe).unwrap().level, Some(100));

        // 命令前缀不符 / 电量越界：拒绝，避免与 gs3075t 串读。
        g[0] = 0x55;
        assert!(x66_parse(&g, 0xfefe).is_none());
        g[0] = 0x20;
        g[3] = 101;
        assert!(x66_parse(&g, 0xfefe).is_none());
    }

    #[test]
    fn urx_request_reply_and_product() {
        // 请求：00 | 08 00 07 | 补零，共 33 字节。
        assert_eq!(URX_REQUEST[..4], [0x00, 0x08, 0x00, 0x07]);
        assert_eq!(URX_REQUEST.len(), 33);
        assert!(URX_REQUEST[4..].iter().all(|&b| b == 0));

        // 33 字节回复（保留报告 id）：08 00 07 4c ...
        let mut f = [0u8; 33];
        f[1..4].copy_from_slice(&[0x08, 0x00, 0x07]);
        f[4] = 0x4c;
        assert_eq!(urx_parse(&f), Some(76));

        // 32 字节回复（剥掉报告 id）：08 00 07 2a ...
        let mut g = [0u8; 32];
        g[..3].copy_from_slice(&[0x08, 0x00, 0x07]);
        g[3] = 42;
        assert_eq!(urx_parse(&g), Some(42));

        // 错误前缀 / 越界 / 过短：拒绝。
        g[0] = 0x09;
        assert!(urx_parse(&g).is_none());
        g[0] = 0x08;
        g[3] = 101;
        assert!(urx_parse(&g).is_none());
        assert!(urx_parse(&[0x08, 0x00, 0x07]).is_none());

        // product string 匹配：EVO75 共享 VID/PID，必须拒绝。
        assert!(urx_product_matches("URX Core75 Wireless"));
        assert!(urx_product_matches("  urx   core75  wireless  "));
        assert!(!urx_product_matches("URX EVO75 Wireless"));
        assert!(!urx_product_matches(""));
    }

    #[test]
    fn redsquare_request_reply_and_product() {
        // 请求：07 | 05 01 01 0A 07 8B | 补零，共 33 字节。
        assert_eq!(REDSQUARE_REQUEST[..7], [0x07, 0x05, 0x01, 0x01, 0x0a, 0x07, 0x8b]);
        assert_eq!(REDSQUARE_REQUEST.len(), 33);
        assert!(REDSQUARE_REQUEST[7..].iter().all(|&b| b == 0));

        // 33 字节回复（保留报告 id 07）：05 A1 01 0A 07 8B .. .. 37 ...
        let mut f = [0u8; 33];
        f[0] = 0x07;
        f[1..7].copy_from_slice(&[0x05, 0xa1, 0x01, 0x0a, 0x07, 0x8b]);
        f[9] = 55;
        assert_eq!(redsquare_parse(&f), Some(55));

        // 32 字节回复（剥掉报告 id）：level 在 byte 8。
        let mut g = [0u8; 32];
        g[..6].copy_from_slice(&[0x05, 0xa1, 0x01, 0x0a, 0x07, 0x8b]);
        g[8] = 80;
        assert_eq!(redsquare_parse(&g), Some(80));

        // 该固件 level 从 1 开始；0 / 101 / 错误前缀都拒绝。
        g[8] = 0;
        assert!(redsquare_parse(&g).is_none());
        g[8] = 101;
        assert!(redsquare_parse(&g).is_none());
        g[1] = 0xa0;
        assert!(redsquare_parse(&g).is_none());

        assert!(redsquare_product_matches("RSQ_20055_2.4G"));
        assert!(redsquare_product_matches("  RSQ_20055_2.4G "));
        assert!(!redsquare_product_matches("RSQ_20055_2.4G DFU"));
    }

    #[test]
    fn atk_keyboard_request_reply_and_product() {
        // 请求：04 | seq_lo seq_hi | 1A | 28 个零，共 32 字节。
        let req = atk_request(1);
        assert_eq!(req[..4], [0x04, 0x01, 0x00, 0x1a]);
        assert_eq!(req.len(), 32);
        assert!(req[4..].iter().all(|&b| b == 0));
        let req = atk_request(0x1234);
        assert_eq!(req[..4], [0x04, 0x34, 0x12, 0x1a]);

        // 回复：04 | seq | 1A | .. | .. .. | .. | level | state。
        let mut f = [0u8; 10];
        f[..4].copy_from_slice(&[0x04, 0x01, 0x00, 0x1a]);
        f[8] = 77;
        f[9] = 1;
        assert_eq!(atk_parse(&f, 1), Some((77, true)));
        f[9] = 0;
        assert_eq!(atk_parse(&f, 1), Some((77, false)));
        f[9] = 2;
        assert_eq!(atk_parse(&f, 1), Some((77, true)));

        // 序号不符、5..7 非零、越界 level/state 都拒绝。
        assert!(atk_parse(&f, 2).is_none());
        f[5] = 1;
        assert!(atk_parse(&f, 1).is_none());
        f[5] = 0;
        f[8] = 101;
        assert!(atk_parse(&f, 1).is_none());
        f[8] = 50;
        f[9] = 3;
        assert!(atk_parse(&f, 1).is_none());

        // product string 决定 label/key，大小写/空白不敏感。
        assert_eq!(
            atk_identity(0x5088, "ATK V75X 2.4G Dongle"),
            Some(("ATK V75X", "atk_keyboard:320f:5088"))
        );
        assert_eq!(
            atk_identity(0x5088, "VGN N75 PRO 2.4G Dongle"),
            Some(("VGN N75 PRO", "atk_keyboard:320f:5088:vgn-n75-pro"))
        );
        assert_eq!(
            atk_identity(0x5055, " atk v75x "),
            Some(("ATK V75X", "atk_keyboard:320f:5088"))
        );
        assert!(atk_identity(0x5055, "VGN N75 PRO 2.4G Dongle").is_none());
        assert!(atk_identity(0x5088, "Other Keyboard").is_none());
    }

    #[test]
    fn jbl_quantum350_request_reply() {
        // 请求就是两个 feature report id。
        assert_eq!(JBL_REPORT_LINK, 0x68);
        assert_eq!(JBL_REPORT_LEVEL, 0x49);

        assert_eq!(jbl_feature_byte(&[0x68, 1], 0x68), Some(1));
        assert_eq!(jbl_feature_byte(&[0x49, 80], 0x49), Some(80));
        assert_eq!(jbl_feature_byte(&[0x68, 1, 0], 0x68), None); // 长度必须为 2
        assert_eq!(jbl_feature_byte(&[0x49, 80], 0x68), None); // id 不符
        assert_eq!(jbl_feature_byte(&[], 0x68), None);

        // link=1 + 合法 level 才给出读数。
        assert_eq!(jbl_decide(Some(1), Some(80)), Some(80));
        assert_eq!(jbl_decide(Some(0), Some(80)), None); // 头显离线
        assert_eq!(jbl_decide(Some(1), Some(101)), None);
        assert_eq!(jbl_decide(None, Some(50)), None);
        assert_eq!(jbl_decide(Some(1), None), None);
    }

    #[test]
    fn moondrop_request_reply() {
        // 请求：4B 80 <cmd> 00 + 补零到输出报告长度。
        let req = moondrop_request(MOONDROP_CMD_BATTERY, 64);
        assert_eq!(req[..4], [0x4b, 0x80, 0x27, 0x00]);
        assert_eq!(req.len(), 64);
        assert!(req[4..].iter().all(|&b| b == 0));
        assert_eq!(moondrop_request(MOONDROP_CMD_LINK, 4), vec![0x4b, 0x80, 0x31, 0x00]);

        // 电量回复：4B 80 27 06 .. .. .. .. link level。
        let mut f = [0u8; 10];
        f[..4].copy_from_slice(&[0x4b, 0x80, 0x27, 0x06]);
        f[8] = 1;
        f[9] = 55;
        assert_eq!(moondrop_parse_battery(&f), Some((true, 55)));
        f[3] = 5;
        assert!(moondrop_parse_battery(&f).is_none());
        f[3] = 6;
        f[9] = 101;
        assert!(moondrop_parse_battery(&f).is_none());
        f[9] = 55;
        f[2] = 0x31;
        assert!(moondrop_parse_battery(&f).is_none());

        // link 回复：4B 80 31 01 link。
        let g = [0x4b, 0x80, 0x31, 0x01, 0x01];
        assert_eq!(moondrop_parse_link(&g), Some(true));
        assert_eq!(moondrop_parse_link(&[0x4b, 0x80, 0x31, 0x00, 0x01]), None);
        assert_eq!(moondrop_parse_link(&[0x4b, 0x80, 0x27, 0x01, 0x01]), None);
    }

    #[test]
    fn registration_and_instance_filter() {
        let providers = all_providers();
        assert_eq!(providers.len(), 7);
        for name in [
            "gs3075t",
            "attackshark_x66",
            "urx",
            "redsquare",
            "atk_keyboard",
            "jbl_quantum350",
            "moondrop",
        ] {
            assert!(providers.iter().any(|p| p.name() == name), "缺少 {name}");
        }
        // VID/PID 不匹配时不触碰 HID（空集合下同样不得产出）。
        for p in &providers {
            assert!(p.poll(r"USB\VID_36B0&PID_3002\7&ABC&0&0", &[]).is_empty());
            assert!(p.poll(r"USB\VID_0C45&PID_1234\7&ABC&0&0", &[]).is_empty());
        }
        // 正确 VID/PID 但集合为空：不得产出占位读数。
        for (index, instance) in [
            (2usize, r"USB\VID_36B0&PID_3002\7&ABC&0&0"),
            (3usize, r"USB\VID_342D&PID_E411\7&ABC&0&0"),
            (4usize, r"USB\VID_320F&PID_5088\7&ABC&0&0"),
            (5usize, r"USB\VID_0ECB&PID_206B\7&ABC&0&0"),
            (6usize, r"USB\VID_35D8&PID_0133\7&ABC&0&0"),
        ] {
            assert!(providers[index].poll(instance, &[]).is_empty(), "provider {index}");
        }
        // 两个 provider 只接受各自独有的回复签名。
        let mut f = [0u8; 33];
        f[1] = 0x55;
        f[2] = 0x10;
        f[26] = 60;
        assert!(gs3075t_parse(&f, 0xfefe).is_some());
        assert!(x66_parse(&f, 0xfefe).is_none());
        let mut g = [0u8; 33];
        g[1..4].copy_from_slice(&[0x20, 0x01, 0x00]);
        g[4] = 60;
        assert!(x66_parse(&g, 0xfefe).is_some());
        assert!(gs3075t_parse(&g, 0xfefe).is_none());
    }
}
