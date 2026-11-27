//! SteelSeries 电量协议族（按 upstream/providers/steelseries.py 与
//! upstream/providers/steelseries_elite.py 精确移植）。
//!
//! 包含三类交换：
//!   * Nova 7 / Nova 5 / Arctis 7+ / GameBuds：`00 b0` 输出报告 + `b0` 回复；
//!   * Rival 3 / Aerox 鼠标：`00 aa 01` 或 `00 d2` 请求 + 回显；
//!   * 老款 Arctis 与 Nova Pro 基站：各自的 `ask(request, accept)` 交换。
//! 另有 Nova Elite 基座（1038:2244）的 `01 b0` / `07 b7` / `07 b5` 协议。
//!
//! 所有电量字段先按各型号的范围校验，越界一律拒绝，不伪造读数。
//! 与 Python 的差异仅在传输层：Rust 的 Handle 无 set_nonblocking，读取用短 deadline 轮询代替。
use crate::hid::{Collection, Handle};
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const STEELSERIES_VID: u16 = 0x1038;
const INTERFACE: u8 = 3;
const VENDOR_USAGE_PAGE: u16 = 0xFFC0;
const VENDOR_USAGE: u16 = 0x0001;

const REQUEST: [u8; 2] = [0x00, 0xB0];
const TIMEOUT: Duration = Duration::from_secs(1);
const READ_TIMEOUT: Duration = Duration::from_millis(100);
const IO_TIMEOUT: Duration = Duration::from_millis(200);

/// Python 的 `Reading = (level, charging, online)`。
type Reading = (Option<u8>, bool, bool);

fn no_reading() -> Reading {
    (None, false, false)
}

/// Python `_valid_level`：先限定 0..=maximum（字节天然是整数）。
fn valid_level(level: u8, maximum: u8) -> bool {
    level <= maximum
}

fn until(d: Duration) -> Instant {
    Instant::now() + d
}

fn now_or(d: Duration, end: Instant) -> Instant {
    until(d).min(end)
}

// ---------------------------------------------------------------------------
// HEADSET MODELS：b0 回复解析
// ---------------------------------------------------------------------------

/// Nova 7：`b0 <link=03> <battery> <status>`，status 00 表示关机/超距。
fn parse_nova7(r: &[u8]) -> Reading {
    if r.len() < 4 || r[0] != 0xB0 || r[1] != 0x03 || r[3] == 0x00 || !valid_level(r[2], 100) {
        return no_reading();
    }
    (Some(r[2]), r[3] == 0x01 || r[3] == 0x02, true)
}

/// 原厂固件的 0..4 档电量（乘 25 转成百分数）。
fn parse_nova7_discrete(r: &[u8]) -> Reading {
    let (level, charging, online) = parse_nova7(r);
    let Some(level) = level else {
        return no_reading();
    };
    if !valid_level(level, 4) {
        return no_reading();
    }
    (Some(level * 25), charging, online)
}

/// Nova 5 / Nova 3P：`b0 <status> <?> <battery> <charging>`，status 02 = 关机。
fn parse_nova5(r: &[u8]) -> Reading {
    if r.len() < 5 || r[0] != 0xB0 || r[1] == 0x02 || !valid_level(r[3], 100) {
        return no_reading();
    }
    (Some(r[3]), r[4] == 0x01, true)
}

/// Arctis 7+：`b0 <status> <level 0..4> <charging>`，status 01 = 关机。
fn parse_arctis7_plus(r: &[u8]) -> Reading {
    if r.len() < 4 || r[0] != 0xB0 || r[1] == 0x01 || !valid_level(r[2], 4) {
        return no_reading();
    }
    (Some(r[2] * 25), r[3] == 0x01, true)
}

/// GameBuds：只取仍在盒外（status 03）的耳机中较低的电量。
fn parse_gamebuds(r: &[u8]) -> Reading {
    if r.len() < 7 || r[0] != 0xB0 {
        return no_reading();
    }
    let mut levels = Vec::new();
    for i in 0..2 {
        if r[3 + i] == 0x03 {
            levels.push(r[5 + i]);
        }
    }
    if levels.is_empty() || !levels.iter().all(|&l| valid_level(l, 100)) {
        return no_reading();
    }
    (Some(*levels.iter().min().unwrap()), false, true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeadsetParser {
    Nova7,
    Nova7Discrete,
    Nova5,
    Arctis7Plus,
    GameBuds,
}

impl HeadsetParser {
    fn parse(self, r: &[u8]) -> Reading {
        match self {
            Self::Nova7 => parse_nova7(r),
            Self::Nova7Discrete => parse_nova7_discrete(r),
            Self::Nova5 => parse_nova5(r),
            Self::Arctis7Plus => parse_arctis7_plus(r),
            Self::GameBuds => parse_gamebuds(r),
        }
    }
}

/// Python `MODELS`：product id -> (名称, 解析函数)，逐项照抄。
const MODELS: &[(u16, &str, HeadsetParser)] = &[
    (0x22A1, "Arctis Nova 7", HeadsetParser::Nova7),
    (0x2202, "Arctis Nova 7", HeadsetParser::Nova7Discrete),
    (0x227E, "Arctis Nova 7 Gen 2", HeadsetParser::Nova7),
    (0x2206, "Arctis Nova 7x", HeadsetParser::Nova7Discrete),
    (0x2258, "Arctis Nova 7x", HeadsetParser::Nova7),
    (0x229E, "Arctis Nova 7x", HeadsetParser::Nova7),
    (0x22AD, "Arctis Nova 7x", HeadsetParser::Nova7),
    (0x22A4, "Arctis Nova 7X", HeadsetParser::Nova7Discrete),
    (0x22A5, "Arctis Nova 7X", HeadsetParser::Nova7),
    (0x223A, "Arctis Nova 7 Diablo IV", HeadsetParser::Nova7Discrete),
    (0x22A9, "Arctis Nova 7 Diablo IV", HeadsetParser::Nova7),
    (0x227A, "Arctis Nova 7 WoW Edition", HeadsetParser::Nova7Discrete),
    (0x2232, "Arctis Nova 5", HeadsetParser::Nova5),
    (0x2253, "Arctis Nova 5X", HeadsetParser::Nova5),
    (0x220A, "Arctis Nova 7P", HeadsetParser::Nova7Discrete),
    (0x22A7, "Arctis Nova 7P", HeadsetParser::Nova7),
    (0x2298, "Arctis Nova 7P", HeadsetParser::Nova7),
    (0x2269, "Arctis Nova 3P Wireless", HeadsetParser::Nova5),
    (0x226D, "Arctis Nova 3X Wireless", HeadsetParser::Nova5),
    (0x220E, "Arctis 7+", HeadsetParser::Arctis7Plus),
    (0x2212, "Arctis 7+ PS5", HeadsetParser::Arctis7Plus),
    (0x2216, "Arctis 7+ Xbox", HeadsetParser::Arctis7Plus),
    (0x2236, "Arctis 7+ Destiny", HeadsetParser::Arctis7Plus),
    (0x230A, "Arctis GameBuds", HeadsetParser::GameBuds),
    (0x2317, "Arctis GameBuds X", HeadsetParser::GameBuds),
];

fn model(pid: u16) -> Option<(&'static str, HeadsetParser)> {
    MODELS
        .iter()
        .find(|(p, _, _)| *p == pid)
        .map(|(_, n, p)| (*n, *p))
}

// ---------------------------------------------------------------------------
// MOUSE MODELS：aa / d2 回显交换
// ---------------------------------------------------------------------------

const MOUSE_REQUEST: &[u8] = &[0x00, 0xAA, 0x01];
const MOUSE_ECHO: u8 = 0xAA;
const AEROX_REQUEST: &[u8] = &[0x00, 0xD2];
const AEROX_ECHO: u8 = 0xD2;
const AEROX_STEPS: u8 = 21;
const AEROX_CHARGING: u8 = 0x80;
const MOUSE_WRITE_ATTEMPTS: usize = 3;
const MOUSE_READ_ATTEMPTS: usize = 6;
const MOUSE_READ_TIMEOUT: Duration = Duration::from_millis(100);

/// Rival 3：同时接受带 aa 回显与 rivalcfg 无回显两种报文布局。
fn parse_rival3(r: &[u8]) -> Reading {
    if r.is_empty() {
        return no_reading();
    }
    let m = if r[0] == 0x00 && r.len() > 1 { 1 } else { 0 };
    if r.len() >= m + 4 && r[m] == MOUSE_ECHO {
        let level = r[m + 1];
        if !valid_level(level, 100) {
            return no_reading();
        }
        return (Some(level), r[m + 3] != 0, true);
    }
    if r.len() >= m + 3 {
        let level = r[m];
        let charging = r[m + 2];
        if valid_level(level, 100) && (charging == 0 || charging == 1) {
            return (Some(level), charging == 1, true);
        }
    }
    no_reading()
}

/// Aerox 3/5/9：`d2 <level>`，bit7 为充电标记，1..21 为档位，>21 直接是百分比。
fn parse_aerox3(r: &[u8]) -> Reading {
    if r.is_empty() {
        return no_reading();
    }
    let m = if r[0] == 0x00 && r.len() > 1 { 1 } else { 0 };
    if r.len() < m + 2 || r[m] != AEROX_ECHO {
        return no_reading();
    }
    let b = r[m + 1];
    if !valid_level(b, 255) {
        return no_reading();
    }
    let v = b & !AEROX_CHARGING;
    if v == 0 || v > 100 {
        return no_reading();
    }
    let level = if v > AEROX_STEPS { v } else { (v - 1) * 5 };
    (Some(level), b & AEROX_CHARGING != 0, true)
}

#[derive(Clone, Copy)]
enum MouseParser {
    Rival3,
    Aerox3,
}

impl MouseParser {
    fn parse(self, r: &[u8]) -> Reading {
        match self {
            Self::Rival3 => parse_rival3(r),
            Self::Aerox3 => parse_aerox3(r),
        }
    }
}

#[derive(Clone, Copy)]
struct MouseExchange {
    request: &'static [u8],
    echo: u8,
}

const RIVAL_EXCHANGE: MouseExchange = MouseExchange { request: MOUSE_REQUEST, echo: MOUSE_ECHO };
const AEROX_EXCHANGE: MouseExchange = MouseExchange { request: AEROX_REQUEST, echo: AEROX_ECHO };

/// Python `MOUSE_MODELS`。
const MOUSE_MODELS: &[(u16, &str, MouseParser)] = &[
    (0x1830, "SteelSeries Rival 3 Wireless", MouseParser::Rival3),
    (0x1872, "SteelSeries Rival 3 Wireless Gen 2", MouseParser::Rival3),
    (0x1838, "SteelSeries Aerox 3 Wireless", MouseParser::Aerox3),
    (0x1878, "SteelSeries Aerox 3 Wireless CS2 Dragon Lore", MouseParser::Aerox3),
    (0x1852, "SteelSeries Aerox 5 Wireless", MouseParser::Aerox3),
    (0x185C, "SteelSeries Aerox 5 Wireless Destiny 2 Edition", MouseParser::Aerox3),
    (0x1860, "SteelSeries Aerox 5 Wireless Diablo IV Edition", MouseParser::Aerox3),
    (0x1858, "SteelSeries Aerox 9 Wireless", MouseParser::Aerox3),
    (0x1874, "SteelSeries Aerox 9 Wireless WOW Edition", MouseParser::Aerox3),
];

fn mouse_model(pid: u16) -> Option<(&'static str, MouseParser, MouseExchange)> {
    MOUSE_MODELS.iter().find(|(p, _, _)| *p == pid).map(|(_, n, parser)| {
        let exchange = match pid {
            0x1838 | 0x1878 | 0x1852 | 0x185C | 0x1860 | 0x1858 | 0x1874 => AEROX_EXCHANGE,
            _ => RIVAL_EXCHANGE,
        };
        (*n, *parser, exchange)
    })
}

// ---------------------------------------------------------------------------
// CLASSIC MODELS：ask(request, accept) 交换
// ---------------------------------------------------------------------------

/// Python `ask`：写请求后反复读，返回首个被 accept 的回复；`answered` 记录是否有被接受的回复。
fn ask<F: Fn(&[u8]) -> bool>(
    handle: &mut Handle,
    answered: &mut bool,
    request: &[u8],
    accept: F,
    report_id_zero: bool,
) -> Option<Vec<u8>> {
    // Python：if report_id_zero: request = [0x00] + request
    let prefixed = if report_id_zero {
        let mut p = Vec::with_capacity(request.len() + 1);
        p.push(0x00);
        p.extend_from_slice(request);
        Some(p)
    } else {
        None
    };
    let request = prefixed.as_deref().unwrap_or(request);
    if !handle.write(request, until(IO_TIMEOUT)) {
        return None;
    }
    let end = until(TIMEOUT);
    while Instant::now() < end {
        let Some(r) = handle.read(now_or(READ_TIMEOUT, end)) else {
            continue;
        };
        if accept(&r) {
            *answered = true;
            return Some(r);
        }
    }
    None
}

/// Arctis 1 回复解析：status 01 = 关机，无充电标记。
fn parse_arctis1_reply(r: &[u8]) -> Reading {
    if r.len() < 4 || r[2] == 0x01 || !valid_level(r[3], 100) {
        return no_reading();
    }
    (Some(r[3]), false, true)
}

/// Arctis 1：`06 12 -> 06 12 <status> <level>`。
fn exchange_arctis1(handle: &mut Handle, answered: &mut bool, zero: bool) -> Reading {
    let Some(r) = ask(
        handle,
        answered,
        &[0x06, 0x12],
        |r| r.len() >= 4 && r[0] == 0x06 && r[1] == 0x12,
        zero,
    ) else {
        return no_reading();
    };
    parse_arctis1_reply(&r)
}

/// Arctis 7 (2018) 连接查询：link 03 = 已连接。
fn parse_arctis7_2018_link(r: &[u8]) -> bool {
    r.len() >= 3 && r[2] == 0x03
}

/// Arctis 7 (2018) 电量：Linux 驱动把超额读数封顶到 100。
fn parse_arctis7_2018_level(r: &[u8]) -> Reading {
    if r.len() < 3 || !valid_level(r[2], 255) {
        return no_reading();
    }
    (Some(r[2].min(100)), false, true)
}

/// Arctis 7 (2018)：先查连接 `06 14`（link 03），再读 `06 18` 电量并封顶 100。
fn exchange_arctis7_2018(handle: &mut Handle, answered: &mut bool, zero: bool) -> Reading {
    let Some(r) = ask(
        handle,
        answered,
        &[0x06, 0x14],
        |r| r.len() >= 4 && r[0] == 0x06 && r[1] == 0x14,
        zero,
    ) else {
        return no_reading();
    };
    if !parse_arctis7_2018_link(&r) {
        return no_reading();
    }
    let Some(r) = ask(
        handle,
        answered,
        &[0x06, 0x18],
        |r| r.len() >= 4 && r[0] == 0x06 && r[1] == 0x18,
        zero,
    ) else {
        return no_reading();
    };
    parse_arctis7_2018_level(&r)
}

/// Arctis 7 / Pro Wireless 2019：`06 18` 回复，0 表示没在发声。
fn parse_arctis7_reply(r: &[u8]) -> Reading {
    if r.len() < 3 || r[2] == 0 || !valid_level(r[2], 100) {
        return no_reading();
    }
    (Some(r[2]), false, true)
}

/// Arctis 7 / Pro Wireless 2019：`06 18 -> 06 18 <level>`。
fn exchange_arctis7(handle: &mut Handle, answered: &mut bool, zero: bool) -> Reading {
    let Some(r) = ask(
        handle,
        answered,
        &[0x06, 0x18],
        |r| r.len() >= 4 && r[0] == 0x06 && r[1] == 0x18,
        zero,
    ) else {
        return no_reading();
    };
    parse_arctis7_reply(&r)
}

/// Arctis 9 回复解析：raw 0x64..0x9A 端点饱和后线性映射到 0..100。
fn parse_arctis9_reply(r: &[u8]) -> Reading {
    if r.len() < 5 || r[0] != 0xAA || r[1] != 0x01 || !valid_level(r[3], 255) {
        return no_reading();
    }
    let raw = r[3].clamp(0x64, 0x9A);
    let level = ((u16::from(raw) - 0x64) * 100 / (0x9A - 0x64)) as u8;
    (Some(level), r[4] == 0x01, true)
}

/// Arctis 9：`00 20 -> aa 01 <?> <raw> <charging>`。
fn exchange_arctis9(handle: &mut Handle, answered: &mut bool, zero: bool) -> Reading {
    let Some(r) = ask(
        handle,
        answered,
        &[0x00, 0x20],
        |r| r.len() >= 5 && (r[0] == 0xAA || r[0] == 0x55),
        zero,
    ) else {
        return no_reading();
    };
    parse_arctis9_reply(&r)
}

const PRO_WIRELESS_REQUEST: [u8; 31] = [
    0x41, 0xAA, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0,
];
const PRO_WIRELESS_LEVEL_REQUEST: [u8; 31] = [
    0x40, 0xAA, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0,
];

/// Arctis Pro Wireless 状态：02 = 关机，04 = 开机（此时电量尚未读到）。
fn parse_pro_state(r: &[u8]) -> Reading {
    if r.is_empty() || r[0] == 0x02 {
        return no_reading();
    }
    (None, false, true)
}

/// Arctis Pro Wireless 电量：0..4 档乘 25。
fn parse_pro_level(r: &[u8]) -> Reading {
    if r.is_empty() || !valid_level(r[0], 4) {
        return no_reading();
    }
    (Some(r[0] * 25), false, true)
}

/// Arctis Pro Wireless：`41 aa -> <state>`（02 关机/04 开机），再 `40 aa -> <level 0..4>`。
fn exchange_pro_wireless(handle: &mut Handle, answered: &mut bool, zero: bool) -> Reading {
    let Some(r) = ask(
        handle,
        answered,
        &PRO_WIRELESS_REQUEST,
        |r| !r.is_empty() && (r[0] == 0x02 || r[0] == 0x04),
        zero,
    ) else {
        return no_reading();
    };
    if !parse_pro_state(&r).2 {
        return no_reading();
    }
    let Some(r) = ask(
        handle,
        answered,
        &PRO_WIRELESS_LEVEL_REQUEST,
        |r| !r.is_empty() && valid_level(r[0], 4),
        zero,
    ) else {
        return no_reading();
    };
    parse_pro_level(&r)
}

const NOVA_PRO_REQUEST: [u8; 2] = [0x06, 0xB0];
const NOVA_PRO_OFF: u8 = 0x01;
const NOVA_PRO_CHARGING: u8 = 0x02;
const NOVA_PRO_ONLINE: u8 = 0x08;

/// Nova Pro 基站回复解析：byte6 九档电量 / byte15 状态，不支持普通百分比。
fn parse_nova_pro_reply(r: &[u8]) -> Reading {
    if r.len() < 16 || r[15] == NOVA_PRO_OFF || !valid_level(r[6], 8) {
        return no_reading();
    }
    (Some((u16::from(r[6]) * 100 / 8) as u8), r[15] == NOVA_PRO_CHARGING, true)
}

/// Nova Pro 基站：`06 b0 -> byte6 九档电量 / byte15 状态`。
fn exchange_nova_pro(handle: &mut Handle, answered: &mut bool, zero: bool) -> Reading {
    let Some(r) = ask(
        handle,
        answered,
        &NOVA_PRO_REQUEST,
        |r| r.len() >= 16 && matches!(r[15], NOVA_PRO_OFF | NOVA_PRO_CHARGING | NOVA_PRO_ONLINE),
        zero,
    ) else {
        return no_reading();
    };
    parse_nova_pro_reply(&r)
}

#[derive(Clone, Copy)]
enum ClassicExchange {
    Arctis1,
    Arctis7_2018,
    Arctis7,
    Arctis9,
    ProWireless,
    NovaPro,
}

impl ClassicExchange {
    fn run(self, handle: &mut Handle, answered: &mut bool, zero: bool) -> Reading {
        match self {
            Self::Arctis1 => exchange_arctis1(handle, answered, zero),
            Self::Arctis7_2018 => exchange_arctis7_2018(handle, answered, zero),
            Self::Arctis7 => exchange_arctis7(handle, answered, zero),
            Self::Arctis9 => exchange_arctis9(handle, answered, zero),
            Self::ProWireless => exchange_pro_wireless(handle, answered, zero),
            Self::NovaPro => exchange_nova_pro(handle, answered, zero),
        }
    }
}

struct ClassicModel {
    pid: u16,
    name: &'static str,
    /// Python 的 `interface`，基站可在 3/4 两个接口作答。
    interfaces: &'static [u8],
    /// Python 的 `usage page`，None 表示任意 >= 0xFF00 的厂商集合。
    page: Option<u16>,
    exchange: ClassicExchange,
}

/// Python `CLASSIC_MODELS`，逐项照抄（Arctis Pro GameDAC 1280 为有线设备，原表即排除）。
const CLASSIC_MODELS: &[ClassicModel] = &[
    ClassicModel { pid: 0x12B3, name: "Arctis 1 Wireless", interfaces: &[3], page: Some(0xFF43), exchange: ClassicExchange::Arctis1 },
    ClassicModel { pid: 0x12B6, name: "Arctis 1 Wireless Xbox", interfaces: &[3], page: Some(0xFF43), exchange: ClassicExchange::Arctis1 },
    ClassicModel { pid: 0x12D7, name: "Arctis 7X", interfaces: &[3], page: Some(0xFF43), exchange: ClassicExchange::Arctis1 },
    ClassicModel { pid: 0x12D5, name: "Arctis 7P", interfaces: &[3], page: Some(0xFF43), exchange: ClassicExchange::Arctis1 },
    ClassicModel { pid: 0x12AD, name: "Arctis 7", interfaces: &[5], page: None, exchange: ClassicExchange::Arctis7_2018 },
    ClassicModel { pid: 0x1260, name: "Arctis 7", interfaces: &[5], page: None, exchange: ClassicExchange::Arctis7 },
    ClassicModel { pid: 0x1252, name: "Arctis Pro Wireless 2019", interfaces: &[5], page: None, exchange: ClassicExchange::Arctis7 },
    ClassicModel { pid: 0x12C2, name: "Arctis 9", interfaces: &[0], page: None, exchange: ClassicExchange::Arctis9 },
    ClassicModel { pid: 0x1290, name: "Arctis Pro Wireless", interfaces: &[0], page: None, exchange: ClassicExchange::ProWireless },
    ClassicModel { pid: 0x12E0, name: "Arctis Nova Pro Wireless", interfaces: &[3, 4], page: None, exchange: ClassicExchange::NovaPro },
    ClassicModel { pid: 0x12E5, name: "Arctis Nova Pro Wireless X", interfaces: &[3, 4], page: None, exchange: ClassicExchange::NovaPro },
];

/// Python `COARSE_MODELS`：九档/五档读数标记 "about NN%"，Rust 用 approximate 表示。
const COARSE_MODELS: &[u16] = &[0x12E0, 0x12E5, 0x1290];

// ---------------------------------------------------------------------------
// 集合选择与传输
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
struct CollectionKey {
    page: u16,
    usage: u16,
    interface: Option<u8>,
}

fn key_of(c: &Collection) -> CollectionKey {
    CollectionKey { page: c.usage_page, usage: c.usage, interface: c.interface }
}

/// Python `_pick`：只取 0xFFC0:0001 配置集合，接口 3 优先，否则取第一个。
fn pick_vendor<'a>(collections: &'a [Collection]) -> Option<&'a Collection> {
    let mine: Vec<&Collection> = collections
        .iter()
        .filter(|c| (c.usage_page, c.usage) == (VENDOR_USAGE_PAGE, VENDOR_USAGE))
        .collect();
    mine.iter()
        .copied()
        .find(|c| c.interface == Some(INTERFACE))
        .or_else(|| mine.first().copied())
}

/// Python `_read`：写 `00 b0`，等待 b0 回复（无 b0 则退回首帧，解析器会拒绝）。
fn read_headset(collection: &Collection) -> Option<Vec<u8>> {
    let mut handle = collection.open()?;
    if !handle.write(&REQUEST, until(IO_TIMEOUT)) {
        return None;
    }
    let end = until(TIMEOUT);
    let mut first: Option<Vec<u8>> = None;
    while Instant::now() < end {
        let Some(r) = handle.read(now_or(READ_TIMEOUT, end)) else {
            continue;
        };
        if r.first() == Some(&0xB0) {
            return Some(r);
        }
        if first.is_none() {
            first = Some(r);
        }
    }
    first
}

/// Python `_read_mouse`：请求补齐 64 字节后发送；echo 或解析接受才收，最多三轮。
fn read_mouse(
    collection: &Collection,
    parser: MouseParser,
    exchange: MouseExchange,
) -> Option<Vec<u8>> {
    let mut handle = collection.open()?;
    // hid.rs 的 write() 会把请求按集合的输出报告长度零填充，等价于 Python 的
    // `request + [0x00] * (64 - len(request))`，且不会写出超过报告长度的字节。
    for _ in 0..MOUSE_WRITE_ATTEMPTS {
        if !handle.write(exchange.request, until(IO_TIMEOUT)) {
            continue;
        }
        for _ in 0..MOUSE_READ_ATTEMPTS {
            let Some(r) = handle.read(until(MOUSE_READ_TIMEOUT)) else {
                continue;
            };
            if r.first() == Some(&exchange.echo)
                || (r.first() == Some(&0x00) && r.len() > 1 && r[1] == exchange.echo)
            {
                return Some(r);
            }
            if parser.parse(&r).2 {
                return Some(r);
            }
            break;
        }
    }
    None
}

fn status(pid: u16, name: &str, kind: &str, source: &str) -> Status {
    let mut st = Status::new(format!("steelseries:{pid:04x}"), name);
    st.kind = kind.into();
    st.source = source.into();
    st
}

// ---------------------------------------------------------------------------
// SteelSeriesProvider（steelseries.py）
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct SteelSeries {
    /// Python `_classic_path`：pid -> 已应答的集合。
    classic_path: Mutex<HashMap<u16, CollectionKey>>,
}

impl SteelSeries {
    pub fn new() -> Self {
        Self::default()
    }

    fn poll_classic(&self, pid: u16, collections: &[Collection], out: &mut Vec<Status>) {
        for model in CLASSIC_MODELS {
            // Python 的 paths 过滤条件包含 d["product_id"] == pid。
            if model.pid != pid {
                continue;
            }
            let candidates: Vec<&Collection> = collections
                .iter()
                .filter(|c| c.interface.is_some_and(|i| model.interfaces.contains(&i)))
                .filter(|c| match model.page {
                    Some(page) => c.usage_page == page,
                    None => c.usage_page >= 0xFF00,
                })
                .collect();
            if candidates.is_empty() {
                continue;
            }

            let known = self
                .classic_path
                .lock()
                .ok()
                .and_then(|m| m.get(&model.pid).copied());
            let mut ordered: Vec<&Collection> = candidates.clone();
            if let Some(key) = known {
                if let Some(pos) = ordered.iter().position(|c| key_of(c) == key) {
                    let first = ordered.remove(pos);
                    ordered.insert(0, first);
                }
            }

            for collection in ordered {
                let Some(mut handle) = collection.open() else {
                    continue;
                };
                let mut answered = false;
                let mut reading = model.exchange.run(&mut handle, &mut answered, false);
                // 0x1290：首发报文无应答时，按 Python 用 report id 0 再试一次。
                if model.pid == 0x1290 && !answered {
                    let mut retried = false;
                    reading = model.exchange.run(&mut handle, &mut retried, true);
                    answered |= retried;
                }
                if !answered {
                    continue;
                }
                if let Ok(mut paths) = self.classic_path.lock() {
                    paths.insert(model.pid, key_of(collection));
                }
                if reading.2 && reading.0.is_some() {
                    let mut st = status(model.pid, model.name, "headset", "steelseries");
                    st.level = reading.0;
                    st.charging = Some(reading.1);
                    st.approximate = COARSE_MODELS.contains(&model.pid);
                    st.charging_known = model.pid != 0x1290;
                    out.push(st);
                }
                break;
            }
        }
    }
}

impl Provider for SteelSeries {
    fn name(&self) -> &'static str {
        "steelseries"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != STEELSERIES_VID {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Some((name, parser, exchange)) = mouse_model(pid) {
            if let Some(collection) = pick_vendor(collections) {
                if let Some(reply) = read_mouse(collection, parser, exchange) {
                    let (level, charging, online) = parser.parse(&reply);
                    if online && level.is_some() {
                        let mut st = status(pid, name, "mouse", "steelseries");
                        st.level = level;
                        st.charging = Some(charging);
                        out.push(st);
                    }
                }
            }
            return out;
        }
        if let Some((name, parser)) = model(pid) {
            if let Some(collection) = pick_vendor(collections) {
                if let Some(reply) = read_headset(collection) {
                    let (level, charging, online) = parser.parse(&reply);
                    if online && level.is_some() {
                        let mut st = status(pid, name, "headset", "steelseries");
                        st.level = level;
                        st.charging = Some(charging);
                        out.push(st);
                    }
                }
            }
        }
        self.poll_classic(pid, collections, &mut out);
        out
    }
}

// ---------------------------------------------------------------------------
// SteelSeriesEliteProvider（steelseries_elite.py，1038:2244）
// ---------------------------------------------------------------------------

const ELITE_PID: u16 = 0x2244;
const fn elite_request() -> [u8; 64] {
    let mut request = [0u8; 64];
    request[0] = 0x01;
    request[1] = 0xB0;
    request
}
const ELITE_REQUEST: [u8; 64] = elite_request();
const ELITE_FRAME: u8 = 0x07;
const ELITE_REPLY: u8 = 0x01;
const ELITE_STATUS: u8 = 0xB0;
const ELITE_REPLY_LEVEL: usize = 6;
const ELITE_REPLY_POWER: usize = 14;
const ELITE_REPLY_CHARGING: usize = 15;
const ELITE_BATTERY: u8 = 0xB7;
const ELITE_POWER: u8 = 0xB5;
const ELITE_CHARGING: u8 = 0x02;
const ELITE_POWER_OFFLINE: u8 = 0x01;
const ELITE_POWER_CABLE: u8 = 0x02;
const ELITE_READ_WINDOW: Duration = Duration::from_secs(1);
const ELITE_READ_SLOT: Duration = Duration::from_millis(10);
const ELITE_READ_PAUSE: Duration = Duration::from_millis(10);

/// 直接回复 `01 b0 ...` -> (电量或 None, 充电, 电源状态)。
fn parse_elite_reply(r: &[u8]) -> Option<(Option<u8>, bool, u8)> {
    if r.is_empty()
        || r.len() <= ELITE_REPLY_CHARGING
        || r[0] != ELITE_REPLY
        || r[1] != ELITE_STATUS
    {
        return None;
    }
    let level = if valid_level(r[ELITE_REPLY_LEVEL], 100) {
        Some(r[ELITE_REPLY_LEVEL])
    } else {
        None
    };
    Some((level, r[ELITE_REPLY_CHARGING] == ELITE_CHARGING, r[ELITE_REPLY_POWER]))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EliteFrameKind {
    Battery,
    Power,
}

/// 推送帧 `07 b7` / `07 b5`。
fn parse_elite_frame(r: &[u8]) -> Option<(EliteFrameKind, u8, bool)> {
    if r.len() < 5 || r[0] != ELITE_FRAME {
        return None;
    }
    if r[1] == ELITE_BATTERY {
        if !valid_level(r[2], 100) {
            return None;
        }
        Some((EliteFrameKind::Battery, r[2], r[4] == ELITE_CHARGING))
    } else if r[1] == ELITE_POWER {
        Some((EliteFrameKind::Power, r[4], false))
    } else {
        None
    }
}

/// Python `_order`：0xFFC0 优先，然后其它厂商页，最后其余。
fn elite_order(c: &Collection) -> (u8, u16) {
    let rank = if c.usage_page == VENDOR_USAGE_PAGE {
        0
    } else if c.usage_page >= 0xFF00 {
        1
    } else {
        2
    };
    (rank, c.usage_page)
}

#[derive(Default)]
pub struct SteelSeriesElite {
    /// Python `_path`：pid -> 接受 `01 b0` 的集合。
    path: Mutex<HashMap<u16, CollectionKey>>,
}

impl SteelSeriesElite {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Python `_send`：先试记忆集合，再找第一个接受 01 b0 的厂商集合。
fn elite_send(
    handles: &mut [(&Collection, Handle)],
    pid: u16,
    cache: &Mutex<HashMap<u16, CollectionKey>>,
) {
    let vendor: Vec<usize> = (0..handles.len())
        .filter(|&i| handles[i].0.usage_page >= 0xFF00)
        .collect();
    let cached = cache.lock().ok().and_then(|m| m.get(&pid).copied());

    if let Some(key) = cached {
        if let Some(&i) = vendor.iter().find(|&&i| key_of(handles[i].0) == key) {
            if handles[i].1.write(&ELITE_REQUEST, until(IO_TIMEOUT)) {
                return;
            }
            if let Ok(mut m) = cache.lock() {
                m.remove(&pid);
            }
        }
    }
    for &i in &vendor {
        if Some(key_of(handles[i].0)) == cached {
            continue;
        }
        if handles[i].1.write(&ELITE_REQUEST, until(IO_TIMEOUT)) {
            if let Ok(mut m) = cache.lock() {
                m.insert(pid, key_of(handles[i].0));
            }
            return;
        }
    }
}

/// Python `_listen`：轮读所有集合直到拿到电量与电源状态，离线立即返回。
fn elite_listen(handles: &mut [(&Collection, Handle)]) -> (Option<(u8, bool)>, Option<u8>) {
    let mut level: Option<(u8, bool)> = None;
    let mut power: Option<u8> = None;
    let end = until(ELITE_READ_WINDOW);
    while Instant::now() < end && (level.is_none() || power.is_none()) {
        let mut got = false;
        for (_, handle) in handles.iter_mut() {
            let Some(r) = handle.read(now_or(ELITE_READ_SLOT, end)) else {
                continue;
            };
            if r.is_empty() {
                continue;
            }
            got = true;
            if let Some((l, charging, p)) = parse_elite_reply(&r) {
                if let Some(l) = l {
                    level = Some((l, charging));
                }
                power = Some(p);
                continue;
            }
            if let Some((kind, value, flag)) = parse_elite_frame(&r) {
                if kind == EliteFrameKind::Battery {
                    level = Some((value, flag));
                } else {
                    power = Some(value);
                    if value == ELITE_POWER_OFFLINE {
                        return (level, power);
                    }
                }
            }
        }
        if !got {
            std::thread::sleep(ELITE_READ_PAUSE);
        }
    }
    (level, power)
}

impl Provider for SteelSeriesElite {
    fn name(&self) -> &'static str {
        "steelseries_elite"
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if vid != STEELSERIES_VID || pid != ELITE_PID {
            return Vec::new();
        }
        let mut mine: Vec<&Collection> = collections
            .iter()
            .filter(|c| c.interface == Some(INTERFACE))
            .collect();
        if mine.is_empty() {
            return Vec::new();
        }
        mine.sort_by_key(|c| elite_order(c));
        let mut handles: Vec<(&Collection, Handle)> = mine
            .into_iter()
            .filter_map(|c| c.open().map(|h| (c, h)))
            .collect();

        elite_send(&mut handles, pid, &self.path);
        let (level, power) = elite_listen(&mut handles);

        if power == Some(ELITE_POWER_OFFLINE) {
            return Vec::new();
        }
        let Some((percent, charging)) = level else {
            return Vec::new();
        };
        let mut st = status(pid, "Arctis Nova Elite", "headset", "steelseries");
        st.level = Some(percent);
        st.charging = Some(charging || power == Some(ELITE_POWER_CABLE));
        vec![st]
    }
}

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![Box::new(SteelSeries::new()), Box::new(SteelSeriesElite::new())]
}

#[cfg(test)]
mod tests {
    use super::*;

    // 源码里的实测报文：steelseries.py:30 指出 `01 00 63 02` 若无 b0 校验会被误读成 99%。
    #[test]
    fn nova7_rejects_stray_report() {
        assert_eq!(parse_nova7(&[0x01, 0x00, 0x63, 0x02]), no_reading());
        assert_eq!(parse_nova7(&[0xB0, 0x03, 99, 0x03]), (Some(99), false, true));
        assert_eq!(parse_nova7(&[0xB0, 0x03, 99, 0x01]), (Some(99), true, true));
        assert_eq!(parse_nova7(&[0xB0, 0x03, 99, 0x02]), (Some(99), true, true));
        // link 02 = 未连接，电量字节虽保留旧值也不读数。
        assert_eq!(parse_nova7(&[0xB0, 0x02, 99, 0x03]), no_reading());
        // status 00 = 关机/超距。
        assert_eq!(parse_nova7(&[0xB0, 0x03, 99, 0x00]), no_reading());
        assert_eq!(parse_nova7(&[0xB0, 0x03, 101, 0x03]), no_reading());
        assert_eq!(parse_nova7(&[0xB0, 0x03, 50]), no_reading());
    }

    #[test]
    fn nova7_discrete_and_nova5_parsers() {
        // 原厂固件 0..4 档：3 -> 75%，4 -> 100%。
        assert_eq!(parse_nova7_discrete(&[0xB0, 0x03, 3, 0x02]), (Some(75), true, true));
        assert_eq!(parse_nova7_discrete(&[0xB0, 0x03, 4, 0x03]), (Some(100), false, true));
        // b0 固件报 0..100 时不能走离散解析器（level > 4 被拒绝）。
        assert_eq!(parse_nova7_discrete(&[0xB0, 0x03, 99, 0x03]), no_reading());

        assert_eq!(parse_nova5(&[0xB0, 0x01, 0x00, 88, 0x01]), (Some(88), true, true));
        assert_eq!(parse_nova5(&[0xB0, 0x05, 0x00, 88, 0x00]), (Some(88), false, true));
        assert_eq!(parse_nova5(&[0xB0, 0x02, 0x00, 88, 0x01]), no_reading());
        assert_eq!(parse_nova5(&[0xB0, 0x01, 0x00, 101, 0x01]), no_reading());
    }

    #[test]
    fn arctis7_plus_and_gamebuds_parsers() {
        assert_eq!(parse_arctis7_plus(&[0xB0, 0x02, 4, 0x01]), (Some(100), true, true));
        assert_eq!(parse_arctis7_plus(&[0xB0, 0x03, 2, 0x00]), (Some(50), false, true));
        assert_eq!(parse_arctis7_plus(&[0xB0, 0x01, 4, 0x01]), no_reading());
        assert_eq!(parse_arctis7_plus(&[0xB0, 0x02, 5, 0x01]), no_reading());

        // 两只都在盒外：取较低者。
        assert_eq!(
            parse_gamebuds(&[0xB0, 0, 0, 0x03, 0x03, 70, 80]),
            (Some(70), false, true)
        );
        // 一只在盒内只算另一只，盒内报 0% 被忽略。
        assert_eq!(
            parse_gamebuds(&[0xB0, 0, 0, 0x01, 0x03, 0, 80]),
            (Some(80), false, true)
        );
        assert_eq!(parse_gamebuds(&[0xB0, 0, 0, 0x01, 0x02, 0, 80]), no_reading());
        assert_eq!(
            parse_gamebuds(&[0xB0, 0, 0, 0x03, 0x03, 101, 80]),
            no_reading()
        );
    }

    #[test]
    fn rival3_accepts_both_layouts() {
        // 带 aa 回显：level=85，charging=01。
        assert_eq!(parse_rival3(&[0xAA, 85, 0, 0x01]), (Some(85), true, true));
        // Windows 报告 id 0 在前。
        assert_eq!(parse_rival3(&[0x00, 0xAA, 85, 0, 0x00]), (Some(85), false, true));
        // rivalcfg 无回显布局：level, <?>, charging。
        assert_eq!(parse_rival3(&[85, 0, 1]), (Some(85), true, true));
        assert_eq!(parse_rival3(&[85, 0, 0]), (Some(85), false, true));
        // 杂帧不能靠任意 charging 字节蒙混过关。
        assert_eq!(parse_rival3(&[85, 0, 2]), no_reading());
        assert_eq!(parse_rival3(&[0xAA, 101, 0, 1]), no_reading());
        assert_eq!(parse_rival3(&[]), no_reading());
    }

    #[test]
    fn aerox3_step_and_percent_layouts() {
        // 21 档：21 -> 100%，1 -> 0%，充电位 bit7。
        assert_eq!(parse_aerox3(&[0xD2, 21]), (Some(100), false, true));
        assert_eq!(parse_aerox3(&[0xD2, 1]), (Some(0), false, true));
        assert_eq!(parse_aerox3(&[0xD2, 0x80 | 21]), (Some(100), true, true));
        // 大于 21 直接是百分比。
        assert_eq!(parse_aerox3(&[0xD2, 80]), (Some(80), false, true));
        assert_eq!(parse_aerox3(&[0x00, 0xD2, 80]), (Some(80), false, true));
        // 0 = 鼠标关机/休眠；无 d2 回显的杂帧不读。
        assert_eq!(parse_aerox3(&[0xD2, 0]), no_reading());
        assert_eq!(parse_aerox3(&[21]), no_reading());
    }

    #[test]
    fn classic_exchange_parsers() {
        // Arctis 1: 06 12 -> 06 12 <status> <level>
        assert_eq!(parse_arctis1_reply(&[0x06, 0x12, 0x02, 76]), (Some(76), false, true));
        assert_eq!(parse_arctis1_reply(&[0x06, 0x12, 0x01, 76]), no_reading());
        assert_eq!(parse_arctis1_reply(&[0x06, 0x12, 0x02, 101]), no_reading());

        // Arctis 7 (2018): 连接字节 + 电量封顶。
        assert!(parse_arctis7_2018_link(&[0x06, 0x14, 0x03]));
        assert!(!parse_arctis7_2018_link(&[0x06, 0x14, 0x02]));
        assert_eq!(parse_arctis7_2018_level(&[0x06, 0x18, 120]), (Some(100), false, true));
        assert_eq!(parse_arctis7_2018_level(&[0x06, 0x18, 55]), (Some(55), false, true));

        // Arctis 7: 0 表示没在发声。
        assert_eq!(parse_arctis7_reply(&[0x06, 0x18, 72]), (Some(72), false, true));
        assert_eq!(parse_arctis7_reply(&[0x06, 0x18, 0]), no_reading());

        // Arctis 9: raw 0x64..0x9A -> 0..100（端点饱和）。
        assert_eq!(parse_arctis9_reply(&[0xAA, 0x01, 0, 0x64, 0x00]), (Some(0), false, true));
        assert_eq!(parse_arctis9_reply(&[0xAA, 0x01, 0, 0x9A, 0x01]), (Some(100), true, true));
        assert_eq!(parse_arctis9_reply(&[0xAA, 0x01, 0, 0x7F, 0x01]), (Some(50), true, true));
        assert_eq!(parse_arctis9_reply(&[0x55, 0x01, 0, 0x7F, 0x01]), no_reading());

        // Arctis Pro Wireless: 状态 02 关机，04 开机；电量 0..4 档。
        assert_eq!(parse_pro_state(&[0x02]), no_reading());
        assert_eq!(parse_pro_state(&[0x04]), (None, false, true));
        assert_eq!(parse_pro_level(&[0]), (Some(0), false, true));
        assert_eq!(parse_pro_level(&[4]), (Some(100), false, true));
        assert_eq!(parse_pro_level(&[5]), no_reading());

        // Nova Pro 九档：byte15 08 在线 / 02 充电 / 01 关机。
        let mut n = vec![0u8; 16];
        n[6] = 4;
        n[15] = 0x08;
        assert_eq!(parse_nova_pro_reply(&n), (Some(50), false, true));
        n[15] = 0x02;
        assert_eq!(parse_nova_pro_reply(&n), (Some(50), true, true));
        n[15] = 0x01;
        assert_eq!(parse_nova_pro_reply(&n), no_reading());
        n[15] = 0x08;
        n[6] = 9;
        assert_eq!(parse_nova_pro_reply(&n), no_reading());
    }

    #[test]
    fn model_tables_match_python() {
        assert_eq!(model(0x22A1), Some(("Arctis Nova 7", HeadsetParser::Nova7)));
        assert_eq!(model(0x2202), Some(("Arctis Nova 7", HeadsetParser::Nova7Discrete)));
        assert_eq!(model(0x2232), Some(("Arctis Nova 5", HeadsetParser::Nova5)));
        assert_eq!(model(0x220E), Some(("Arctis 7+", HeadsetParser::Arctis7Plus)));
        assert_eq!(model(0x230A), Some(("Arctis GameBuds", HeadsetParser::GameBuds)));
        assert_eq!(model(0x9999), None);
        assert_eq!(MODELS.len(), 25);

        let (name, _, exchange) = mouse_model(0x1838).unwrap();
        assert_eq!(name, "SteelSeries Aerox 3 Wireless");
        assert_eq!(exchange.echo, AEROX_ECHO);
        assert_eq!(mouse_model(0x1830).unwrap().2.echo, MOUSE_ECHO);
        assert_eq!(MOUSE_MODELS.len(), 9);
        assert_eq!(CLASSIC_MODELS.len(), 11);
        assert!(COARSE_MODELS.contains(&0x1290) && COARSE_MODELS.contains(&0x12E0));
    }

    // steelseries_elite.py:19 的实测回复 `01 b0 00 00 01 00 1f 64 ...`，GG 显示 31%。
    #[test]
    fn elite_direct_reply_matches_hardware_sample() {
        let mut r = vec![0u8; 16];
        r[..8].copy_from_slice(&[0x01, 0xB0, 0x00, 0x00, 0x01, 0x00, 0x1F, 0x64]);
        r[14] = 0x08; // on battery
        r[15] = 0x08;
        assert_eq!(parse_elite_reply(&r), Some((Some(31), false, 0x08)));
        r[15] = 0x02; // charging
        assert_eq!(parse_elite_reply(&r), Some((Some(31), true, 0x08)));
        r[6] = 101;
        assert_eq!(parse_elite_reply(&r), Some((None, true, 0x08)));
        r[1] = 0xB1;
        assert_eq!(parse_elite_reply(&r), None);
        assert_eq!(parse_elite_reply(&r[..15]), None);
    }

    #[test]
    fn elite_push_frames() {
        // 07 b7 <headset level> <spare level> <charging> ...
        let frame = [0x07, 0xB7, 98, 0x00, 0x02];
        assert_eq!(
            parse_elite_frame(&frame),
            Some((EliteFrameKind::Battery, 98, true))
        );
        let frame = [0x07, 0xB7, 101, 0x00, 0x08];
        assert_eq!(parse_elite_frame(&frame), None);
        // 07 b5 <?> <bluetooth> <power>
        let frame = [0x07, 0xB5, 0x00, 0x02, 0x01];
        assert_eq!(parse_elite_frame(&frame), Some((EliteFrameKind::Power, 0x01, false)));
        let frame = [0x07, 0xB6, 0x00, 0x02, 0x02];
        assert_eq!(parse_elite_frame(&frame), None);
    }

    #[test]
    fn registration_and_vid_filter() {
        let providers = all_providers();
        assert_eq!(providers.len(), 2);
        assert!(providers.iter().any(|p| p.name() == "steelseries"));
        assert!(providers.iter().any(|p| p.name() == "steelseries_elite"));
        assert!(providers
            .iter()
            .all(|p| p.poll(r"USB\VID_1532&PID_0CE6\7&1A2B3C&0&0", &[]).is_empty()));
    }

    #[test]
    fn request_bytes() {
        assert_eq!(REQUEST, [0x00, 0xB0]);
        assert_eq!(MOUSE_REQUEST, [0x00, 0xAA, 0x01]);
        assert_eq!(AEROX_REQUEST, [0x00, 0xD2]);
        assert_eq!(ELITE_REQUEST[..2], [0x01, 0xB0]);
        assert_eq!(ELITE_REQUEST.len(), 64);
        assert!(ELITE_REQUEST[2..].iter().all(|&b| b == 0));
        assert_eq!(PRO_WIRELESS_REQUEST.len(), 31);
        assert_eq!(&PRO_WIRELESS_REQUEST[..2], &[0x41, 0xAA]);
        assert_eq!(&PRO_WIRELESS_LEVEL_REQUEST[..2], &[0x40, 0xAA]);
    }
}
