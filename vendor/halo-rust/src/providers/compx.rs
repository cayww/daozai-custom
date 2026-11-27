//! CompX report-8 电量协议（多厂商共享实现）。
//!
//! Python 参考：`vendor/halo-battery/upstream/providers/compx.py` 以及使用它的
//! `attackshark_r11.py` / `dareu.py` / `io_aurora.py` / `lamzu.py` / `pulsar.py`。
//! 帧格式（17 字节）：`[0]=0x08, [1]=cmd, [2..15]=参数, [16]=checksum`，
//! `checksum = (0x55 - sum(frame[0..16])) % 256`。严格照搬 Python 的字节校验，
//! 不简化合法性判断。
use crate::hid::Collection;
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{Provider, Status};
use std::time::{Duration, Instant};

// ---- compx.py 常量（原样对应，不改动）----
const PAYLOAD_HEADER: u8 = 0x08;
const PAYLOAD_LEN: usize = 17;
const CHECKSUM_BASE: u8 = 0x55;
const CMD_POWER: u8 = 0x04;
const LEVEL_INDEX: usize = 6;
const POWER_INDEX: usize = 7;
const VOLTAGE_SLICE: (usize, usize) = (8, 10);
const CONTROL_USAGE: (u16, u16) = (0xFF02, 0x0002);
const READ_ATTEMPTS: usize = 4;
const READ_TIMEOUT_MS: u64 = 250;
const DRAIN_ATTEMPTS: usize = 4;
const DRAIN_TIMEOUT_MS: u64 = 30;

// ---- dareu.py 的 feature 变体 ----
const REPLY_USAGE: (u16, u16) = (0xFF01, 0x0000);
const REPLY_HEADER: u8 = 0x09;
const STATUS_DATA: u8 = 0x00;

/// `(CHECKSUM_BASE - sum(payload)) % 256`，用 `rem_euclid` 复刻 Python 的非负取模。
fn checksum(payload: &[u8]) -> u8 {
    let sum: i32 = payload.iter().map(|&b| i32::from(b)).sum();
    (i32::from(CHECKSUM_BASE) - sum).rem_euclid(256) as u8
}

/// `08 04 00 * 14 + checksum`（共 17 字节）。
fn make_request() -> [u8; PAYLOAD_LEN] {
    let mut frame = [0u8; PAYLOAD_LEN];
    frame[0] = PAYLOAD_HEADER;
    frame[1] = CMD_POWER;
    frame[PAYLOAD_LEN - 1] = checksum(&frame[..PAYLOAD_LEN - 1]);
    frame
}

/// compx.py `parse_power`：0x08 头 + 0x04 命令 + 校验和 + 0..100 电量。
fn parse_power(r: &[u8]) -> Option<(u8, bool)> {
    if r.len() < PAYLOAD_LEN {
        return None;
    }
    let frame = &r[..PAYLOAD_LEN];
    if frame[0] != PAYLOAD_HEADER || frame[1] != CMD_POWER {
        return None;
    }
    if frame[PAYLOAD_LEN - 1] != checksum(&frame[..PAYLOAD_LEN - 1]) {
        return None;
    }
    let level = frame[LEVEL_INDEX];
    if level > 100 {
        return None;
    }
    Some((level, frame[POWER_INDEX] != 0))
}

/// io_aurora.py `power_reply`：power 位 >3 判非法，3 表示充电状态未知。
fn power_reply_aurora(r: &[u8]) -> Option<(u8, bool, bool)> {
    if r.len() < PAYLOAD_LEN {
        return None;
    }
    let (level, charging) = parse_power(r)?;
    if r[POWER_INDEX] > 3 {
        return None;
    }
    let known = r[POWER_INDEX] != 3;
    Some((level, if known { charging } else { false }, known))
}

/// dareu.py `parse_power`：0x09 回复头 + 0x04 命令 + status 0 + 同样的校验和。
fn parse_power_dareu(r: &[u8]) -> Option<(u8, bool)> {
    if r.len() < PAYLOAD_LEN {
        return None;
    }
    let frame = &r[..PAYLOAD_LEN];
    if frame[0] != REPLY_HEADER || frame[1] != CMD_POWER || frame[2] != STATUS_DATA {
        return None;
    }
    if frame[PAYLOAD_LEN - 1] != checksum(&frame[..PAYLOAD_LEN - 1]) {
        return None;
    }
    let level = frame[LEVEL_INDEX];
    if level > 100 {
        return None;
    }
    Some((level, frame[POWER_INDEX] != 0))
}

#[allow(dead_code)] // 诊断用，Python 侧同名字段
fn voltage_mv(r: &[u8]) -> Option<u16> {
    if r.len() < VOLTAGE_SLICE.1 {
        return None;
    }
    Some(u16::from_be_bytes([r[VOLTAGE_SLICE.0], r[VOLTAGE_SLICE.0 + 1]]))
}

#[derive(Clone, Copy)]
enum Variant {
    /// 大多数 CompX 设备：写 interrupt 请求，读 interrupt 回复。
    Interrupt {
        /// io_aurora：power 位 3 表示未知，并允许 feature 回复回退。
        aurora: bool,
    },
    /// dareu：请求走 feature 到 ff02:0002，回复走 input 于 ff01:0000。
    Feature,
}

/// 一个 CompX 设备组。VID/PID/名称/key 全部逐字抄自对应 Python provider。
pub struct CompX {
    provider: &'static str,
    vids: &'static [u16],
    /// (pid, 显示名, 状态 key, 设备类型)
    devices: &'static [(u16, &'static str, &'static str, &'static str)],
    /// 部分设备只认固定接口上的控制集合。
    interface: Option<u8>,
    variant: Variant,
}

fn deadline_ms(ms: u64) -> Instant {
    Instant::now() + Duration::from_millis(ms)
}

/// 优先选 `ff02:0002` 控制集合；输出报告必须能承载 17 字节帧（长度 0 表示未知，不否决）。
fn pick_control<'a>(collections: &'a [Collection], interface: Option<u8>) -> Option<&'a Collection> {
    let control: Vec<&Collection> = collections
        .iter()
        .filter(|c| (c.usage_page, c.usage) == CONTROL_USAGE)
        .filter(|c| interface.map_or(true, |i| c.interface == Some(i)))
        .collect();
    if let Some(c) = control
        .iter()
        .copied()
        .find(|c| c.output_len == 0 || c.output_len >= PAYLOAD_LEN)
    {
        return Some(c);
    }
    control.first().copied()
}

fn query_interrupt(
    collection: &Collection,
    request: &[u8],
    aurora: bool,
) -> Option<(u8, bool, bool)> {
    let mut handle = collection.open()?;
    // 丢弃接收器在本次请求之前推送的陈旧帧。
    for _ in 0..DRAIN_ATTEMPTS {
        if handle.read(deadline_ms(DRAIN_TIMEOUT_MS)).is_none() {
            break;
        }
    }
    if !handle.write(request, deadline_ms(READ_TIMEOUT_MS)) {
        return None;
    }
    for _ in 0..READ_ATTEMPTS {
        let Some(frame) = handle.read(deadline_ms(READ_TIMEOUT_MS)) else {
            break;
        };
        let parsed = if aurora {
            power_reply_aurora(&frame)
        } else {
            parse_power(&frame).map(|(level, charging)| (level, charging, true))
        };
        if let Some(parsed) = parsed {
            return Some(parsed);
        }
    }
    // io_aurora.py：部分 CompX 接收器把只读请求回成 feature 报告。
    if aurora && collection.feature_len >= PAYLOAD_LEN {
        if let Some(frame) = handle.get_feature(PAYLOAD_HEADER, deadline_ms(READ_TIMEOUT_MS)) {
            if let Some(parsed) = power_reply_aurora(&frame) {
                return Some(parsed);
            }
        }
    }
    None
}

fn query_feature(collections: &[Collection], request: &[u8]) -> Option<(u8, bool)> {
    let command = collections
        .iter()
        .find(|c| (c.usage_page, c.usage) == CONTROL_USAGE)?;
    let reply = collections
        .iter()
        .find(|c| (c.usage_page, c.usage) == REPLY_USAGE)?;
    let mut command = command.open()?;
    let mut reply = reply.open()?;
    for _ in 0..DRAIN_ATTEMPTS {
        if reply.read(deadline_ms(DRAIN_TIMEOUT_MS)).is_none() {
            break;
        }
    }
    if !command.set_feature(request, deadline_ms(READ_TIMEOUT_MS)) {
        return None;
    }
    for _ in 0..READ_ATTEMPTS {
        let Some(frame) = reply.read(deadline_ms(READ_TIMEOUT_MS)) else {
            break;
        };
        if let Some(parsed) = parse_power_dareu(&frame) {
            return Some(parsed);
        }
    }
    None
}

impl Provider for CompX {
    fn name(&self) -> &'static str {
        self.provider
    }

    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
        let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
            return Vec::new();
        };
        if !self.vids.contains(&vid) {
            return Vec::new();
        }
        let Some(&(_, name, key, kind)) = self.devices.iter().find(|(p, _, _, _)| *p == pid) else {
            return Vec::new();
        };
        let request = make_request();
        let parsed = match self.variant {
            Variant::Interrupt { aurora } => {
                let Some(collection) = pick_control(collections, self.interface) else {
                    return Vec::new();
                };
                query_interrupt(collection, &request, aurora)
            }
            Variant::Feature => query_feature(collections, &request).map(|(l, c)| (l, c, true)),
        };
        let Some((level, charging, charging_known)) = parsed else {
            return Vec::new();
        };
        let mut status = Status::new(key, name);
        status.kind = kind.into();
        status.level = Some(level);
        status.charging = Some(charging);
        status.charging_known = charging_known;
        status.source = self.provider.into();
        vec![status]
    }
}

// 厂商设备表：VID/PID 与名称逐字取自 Python 源码，未做合并或猜测。
const COMPX: &[CompX] = &[
    // attackshark_r11.py: VID=0x3554, PID=0xFB44, KEY="attackshark:fb44"
    CompX {
        provider: "attackshark_r11",
        vids: &[0x3554],
        devices: &[(0xFB44, "Attack Shark R11 Ultra", "attackshark:fb44", "mouse")],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
    // dareu.py: VID=0x260D, PIDS={0x1074, 0x1084}，feature 请求 / 0x09 回复
    CompX {
        provider: "dareu",
        vids: &[0x260D],
        devices: &[
            (0x1074, "DAREU A950 (2.4 GHz)", "dareu:260d1074", "mouse"),
            (0x1084, "DAREU mouse (2.4 GHz)", "dareu:260d1084", "mouse"),
        ],
        interface: None,
        variant: Variant::Feature,
    },
    // io_aurora.py: VID=0x3554, PID=0xFB08(receiver)/0xFB09(cable)，interface 1，
    // power 位 3 表示充电状态未知。Python 还按 product string 把关，Rust 侧
    // Collection 不暴露该字段，故仅按 PID + interface + ff02:0002 匹配。
    CompX {
        provider: "io_aurora",
        vids: &[0x3554],
        devices: &[
            (0xFB08, "IO Aurora", "io_aurora:fb08", "mouse"),
            (0xFB09, "IO Aurora", "io_aurora:fb08", "mouse"),
        ],
        interface: Some(1),
        variant: Variant::Interrupt { aurora: true },
    },
    // lamzu.py 的 CompX 家族（Thorn/Atlantis，COMPX_VID=0x3554）。
    // 0xF50F 在 Python 中依 product string 区分 Atlantis cable 与 Thorn wired；
    // Rust 侧无该字段，按 Thorn wired 处理（key 同为 lamzu:3554f510）。
    CompX {
        provider: "lamzu",
        vids: &[0x3554],
        devices: &[
            (0xF510, "LAMZU Thorn (4K receiver)", "lamzu:3554f510", "mouse"),
            (0xF50F, "LAMZU Thorn (wired)", "lamzu:3554f510", "mouse"),
        ],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
    // pulsar.py PIDS：3554 家族。F58C/F58A 与 373B:1273 按 RECEIVER_OF 归到
    // 同一个 receiver key，避免一根鼠标出现两个图标。
    CompX {
        provider: "pulsar",
        vids: &[0x3554],
        devices: &[
            (0xF5F7, "Scyrox 8K Dongle", "pulsar:3554f5f7", "mouse"),
            (0xF5F6, "Scyrox (wired)", "pulsar:3554f5f6", "mouse"),
            (0xF5DF, "Cougar Revenger Pro 4K", "pulsar:3554f5df", "mouse"),
            (0xF5DE, "Cougar Revenger Pro 4K (wired)", "pulsar:3554f5de", "mouse"),
            (0xF5DC, "Cougar Revenger Pro (2.4 GHz)", "pulsar:3554f5dc", "mouse"),
            (0xF508, "Pulsar X2 V2 Mini (wireless)", "pulsar:3554f508", "mouse"),
            (0xF50D, "Lamzu Atlantis V2 (2.4 GHz)", "pulsar:3554f50d", "mouse"),
            (0xF507, "Pulsar X2 V2 Mini (wired)", "pulsar:3554f507", "mouse"),
            (0xF58F, "ATK VXE R1 SE+ (wired)", "pulsar:3554f58f", "mouse"),
            (0xF58A, "VXE R1 Pro / R1 Pro Max (2.4 GHz)", "pulsar:3554f58a", "mouse"),
            (0xF58C, "VXE R1 Pro Max (wired)", "pulsar:3554f58a", "mouse"),
            (0xF505, "VGN Dragonfly 4K (receiver)", "pulsar:3554f505", "mouse"),
            (0xF506, "VGN Dragonfly 4K (wired)", "pulsar:3554f506", "mouse"),
            (0xF5D5, "Redragon M991 / K1NG Max (2.4 GHz)", "pulsar:3554f5d5", "mouse"),
            (0xF55D, "Redragon M916 Pro / M913 (2.4 GHz)", "pulsar:3554f55d", "mouse"),
            (0xF55E, "Redragon M991 (wired)", "pulsar:3554f55e", "mouse"),
            (0xFA09, "Compx 2.4 GHz keyboard", "pulsar:3554fa09", "keyboard"),
            (0xF520, "Teevolution Terra Pro (1K receiver)", "pulsar:3554f520", "mouse"),
            (0xF523, "Teevolution Terra Pro (2.4 GHz)", "pulsar:3554f523", "mouse"),
            (0xF526, "Zaopin Z2 4K (wired)", "pulsar:3554f526", "mouse"),
            (0xF527, "Zaopin Z2 4K (2.4 GHz)", "pulsar:3554f527", "mouse"),
        ],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
    CompX {
        provider: "pulsar",
        vids: &[0x35BB],
        devices: &[
            (0xD3FA, "HECATE mouse (wired)", "pulsar:35bbd3fa", "mouse"),
            (0xD3FC, "HECATE mouse (wired)", "pulsar:35bbd3fc", "mouse"),
            (0xD3FD, "HECATE mouse (wired)", "pulsar:35bbd3fd", "mouse"),
        ],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
    CompX {
        provider: "pulsar",
        vids: &[0x1B1C],
        devices: &[(0x2B2A, "Corsair Sabre v2 Pro Ultralight", "pulsar:1b1c2b2a", "mouse")],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
    CompX {
        provider: "pulsar",
        vids: &[0x373B],
        devices: &[
            (0x11B6, "ATK A9 Ultimate (wired)", "pulsar:373b11b6", "mouse"),
            (0x11D9, "ATK A9 Ultimate (2.4 GHz dongle)", "pulsar:373b11d9", "mouse"),
            (0x10C9, "ATK A9 SE (2.4 GHz)", "pulsar:373b10c9", "mouse"),
            (0x1085, "ATK VXE R1 SE+ (2.4 GHz)", "pulsar:373b1085", "mouse"),
            (0x101A, "ATK X1 Pro (2.4 GHz)", "pulsar:373b101a", "mouse"),
            (0x101B, "ATK Fire X 8K (2.4 GHz dongle)", "pulsar:373b101b", "mouse"),
            (0x1273, "VXE R1 Pro Max (wired)", "pulsar:3554f58a", "mouse"),
        ],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
    CompX {
        provider: "pulsar",
        vids: &[0x3770],
        devices: &[
            (0x0300, "Hitscan Hyperlight (8K receiver)", "pulsar:37700300", "mouse"),
            (0x0200, "Hitscan Hyperlight (2.4 GHz)", "pulsar:37700200", "mouse"),
            (0x0100, "Hitscan Hyperlight (wired)", "pulsar:37700100", "mouse"),
        ],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
    CompX {
        provider: "pulsar",
        vids: &[0x3710],
        devices: &[
            (0x5406, "Pulsar X2 CrazyLight Mini (2.4 GHz)", "pulsar:37105406", "mouse"),
            (0x3414, "Pulsar X2 CrazyLight Mini (wired)", "pulsar:37103414", "mouse"),
            (0x3510, "Pulsar X2N CrazyLight (wired)", "pulsar:37103510", "mouse"),
        ],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
    // G-Wolves 共享 8K 接收器（pulsar.py docstring / #297 实测）
    CompX {
        provider: "pulsar",
        vids: &[0x33E4],
        devices: &[(0x3854, "G-Wolves mouse (8K receiver)", "pulsar:33e43854", "mouse")],
        interface: None,
        variant: Variant::Interrupt { aurora: false },
    },
];

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    COMPX
        .iter()
        .map(|p| {
            Box::new(CompX {
                provider: p.provider,
                vids: p.vids,
                devices: p.devices,
                interface: p.interface,
                variant: p.variant,
            }) as Box<dyn Provider>
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一帧合法的标准 CompX 回复。
    fn reply(level: u8, power: u8) -> Vec<u8> {
        let mut frame = vec![0u8; PAYLOAD_LEN];
        frame[0] = PAYLOAD_HEADER;
        frame[1] = CMD_POWER;
        frame[LEVEL_INDEX] = level;
        frame[POWER_INDEX] = power;
        frame[PAYLOAD_LEN - 1] = checksum(&frame[..PAYLOAD_LEN - 1]);
        frame
    }

    #[test]
    fn checksum_matches_python() {
        // Python: (0x55 - (0x08 + 0x04)) % 256 = 0x49
        assert_eq!(checksum(&[0x08, 0x04]), 0x49);
        // 大量字节时 Python 的 % 256 非负回绕
        assert_eq!(checksum(&[0xFF; 16]), 0x65);
        let request = make_request();
        assert_eq!(request.len(), PAYLOAD_LEN);
        assert_eq!(request[0], PAYLOAD_HEADER);
        assert_eq!(request[1], CMD_POWER);
        assert_eq!(request[2..PAYLOAD_LEN - 1].iter().sum::<u8>(), 0);
        assert_eq!(
            request[PAYLOAD_LEN - 1],
            checksum(&request[..PAYLOAD_LEN - 1])
        );
        assert_eq!(request[PAYLOAD_LEN - 1], 0x49);
    }

    #[test]
    fn parse_power_valid() {
        let frame = reply(90, 0);
        assert_eq!(parse_power(&frame), Some((90, false)));
        let frame = reply(100, 1);
        assert_eq!(parse_power(&frame), Some((100, true)));
        // Python 只取前 17 字节，尾随数据不影响解析
        let mut long = frame.clone();
        long.extend_from_slice(&[0xAA; 9]);
        assert_eq!(parse_power(&long), Some((100, true)));
    }

    #[test]
    fn parse_power_invalid() {
        let frame = reply(50, 1);
        // 长度不足
        assert_eq!(parse_power(&frame[..PAYLOAD_LEN - 1]), None);
        // 头部不是 0x08
        let mut bad = frame.clone();
        bad[0] = 0x09;
        assert_eq!(parse_power(&bad), None);
        // 命令不是 0x04
        bad = frame.clone();
        bad[1] = 0x0A;
        assert_eq!(parse_power(&bad), None);
        // 校验和错误
        bad = frame;
        bad[PAYLOAD_LEN - 1] ^= 0xFF;
        assert_eq!(parse_power(&bad), None);
        // 电量越界
        assert_eq!(parse_power(&reply(101, 0)), None);
        assert_eq!(parse_power(&reply(255, 0)), None);
    }

    #[test]
    fn dareu_and_aurora_variants() {
        // dareu: 0x09 头 + status 0
        let mut dareu = reply(76, 1);
        dareu[0] = REPLY_HEADER;
        dareu[2] = STATUS_DATA;
        dareu[PAYLOAD_LEN - 1] = checksum(&dareu[..PAYLOAD_LEN - 1]);
        assert_eq!(parse_power_dareu(&dareu), Some((76, true)));
        // dareu 拒绝标准 0x08 头，也拒绝非零 status
        assert_eq!(parse_power_dareu(&reply(76, 1)), None);
        dareu[2] = 1;
        dareu[PAYLOAD_LEN - 1] = checksum(&dareu[..PAYLOAD_LEN - 1]);
        assert_eq!(parse_power_dareu(&dareu), None);
        // io_aurora: power 位 3 = 未知，>3 非法
        assert_eq!(power_reply_aurora(&reply(80, 3)), Some((80, false, false)));
        assert_eq!(power_reply_aurora(&reply(80, 2)), Some((80, true, true)));
        assert_eq!(power_reply_aurora(&reply(80, 4)), None);
    }

    #[test]
    fn voltage_and_registration() {
        let mut frame = reply(50, 0);
        frame[VOLTAGE_SLICE.0] = 0x10;
        frame[VOLTAGE_SLICE.0 + 1] = 0x73;
        assert_eq!(voltage_mv(&frame), Some(0x1073));
        assert_eq!(voltage_mv(&[0u8; 8]), None);
        let providers = all_providers();
        for name in ["attackshark_r11", "dareu", "io_aurora", "lamzu", "pulsar"] {
            assert!(providers.iter().any(|p| p.name() == name), "缺少 {name}");
        }
        // 无 HID 集合时安全返回空，VID/PID 不匹配时不误报
        let p = providers
            .iter()
            .find(|p| p.name() == "attackshark_r11")
            .unwrap();
        assert!(p.poll(r"USB\VID_3554&PID_FB44\7&1A2B3C&0&0", &[]).is_empty());
        assert!(p.poll(r"USB\VID_3554&PID_FB45\7&1A2B3C&0&0", &[]).is_empty());
    }
}
