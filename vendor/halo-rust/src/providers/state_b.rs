//! 复杂状态机协议批量移植（第二批）：inphic / rapoo / gwolves / steam /
//! plantronics / wraith / fractal / fantech / endgame_we / jabra。
//!
//! 所有字节格式、VID/PID、命令序列、校验与合法性判断均逐条对应
//! `vendor/halo-battery/upstream/providers/<name>.py`，不猜测报文。
//!
//! 注意：Rust `Collection` API 不暴露 HID path / product string，且 `hid.rs`
//! 只放行 `usage_page >= 0xFF00` 的集合。Python 侧读取 `000A:0000`（inphic）、
//! `0001:0005`（steam 兜底）的分支在这里不可达，只能退到厂商页集合；
//! 跨 USB 实例合并同一台设备（rapoo 接收器/线缆）也无法在一次 poll 内完成。
//! 这些差异及各协议的具体限制写在文末 `SKIPPED / 差异` 注释与交付说明中。
//!
//! 静态移植只能保证与 Python 源码逻辑一致；读数正确性必须实机确认。

use crate::hid::{Collection, Handle};
use crate::protocol::parse_instance_vid_pid;
use crate::providers::{sha256_hex, Provider, Status};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::{Duration, Instant};

fn dl(ms: u64) -> Instant {
    Instant::now() + Duration::from_millis(ms)
}

fn sum8(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |a, &b| a.wrapping_add(b))
}

/// `(0x55 - sum(data)) & 0xFF`，Python 侧常见的补码校验。
fn check55(data: &[u8]) -> u8 {
    0x55u8.wrapping_sub(sum8(data))
}

/// 单次短读；整体窗口到期时不再等待。
fn read_until(h: &mut Handle, end: Instant, chunk_ms: u64) -> Option<Vec<u8>> {
    if Instant::now() >= end {
        return None;
    }
    h.read((Instant::now() + Duration::from_millis(chunk_ms)).min(end))
}

fn status(
    key: impl Into<String>,
    name: impl Into<String>,
    kind: &str,
    source: &str,
    level: Option<u8>,
    charging: Option<bool>,
    online: bool,
    charging_known: bool,
) -> Status {
    let mut st = Status::new(key, name);
    st.kind = kind.into();
    st.source = source.into();
    st.level = level;
    st.charging = charging;
    st.online = online;
    st.charging_known = charging_known;
    st
}

// ===========================================================================
// Fractal Design Scape：VID 36BC，接收器 0001 / 线缆 2001，FF00:0001。
// 输出报告 02：<02><type><21><zeros>；输入报告 02 回显前三字节。
// 接收器 type 11 的 r[4]==1 表示耳机已连接；耳机 type F1 的 r[15] 是百分比、
// r[23]==1 表示 USB 充电。
// ===========================================================================
mod fractal {
    use super::*;

    const VID: u16 = 0x36BC;
    const DONGLE: u16 = 0x0001;
    const CABLE: u16 = 0x2001;
    const PAGE: u16 = 0xFF00;
    const USAGE: u16 = 0x0001;
    const REPORT: u8 = 0x02;
    const TYPE_DONGLE: u8 = 0x11;
    const TYPE_HEADSET: u8 = 0xF1;
    const GET_STATE: u8 = 0x21;
    const READ_MS: u64 = 250;
    const LISTEN_MS: u64 = 1000;

    fn request(device_type: u8, out_len: usize) -> Vec<u8> {
        let mut v = vec![REPORT, device_type, GET_STATE];
        if v.len() < out_len {
            v.resize(out_len, 0);
        }
        v
    }

    fn parse_state<'a>(r: &'a [u8], device_type: u8) -> Option<&'a [u8]> {
        if r.len() < 24 || r[0] != REPORT || r[1] != device_type || r[2] != GET_STATE {
            return None;
        }
        Some(r)
    }

    fn battery(r: &[u8]) -> Option<(u8, bool)> {
        if r.len() < 24 || r[15] > 100 {
            return None;
        }
        Some((r[15], r[23] == 1))
    }

    fn wait(h: &mut Handle, device_type: u8) -> Option<Vec<u8>> {
        let end = dl(LISTEN_MS);
        while Instant::now() < end {
            if let Some(r) = read_until(h, end, READ_MS) {
                if parse_state(&r, device_type).is_some() {
                    return Some(r);
                }
            }
        }
        None
    }

    pub struct Fractal;

    impl Provider for Fractal {
        fn name(&self) -> &'static str {
            "fractal"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID || (pid != DONGLE && pid != CABLE) {
                return Vec::new();
            }
            let cable = pid == CABLE;
            let mut eligible = false;
            for c in collections {
                if (c.usage_page, c.usage) != (PAGE, USAGE) || c.output_len < 4 || c.input_len < 24
                {
                    continue;
                }
                eligible = true;
                let Some(mut h) = c.open() else { continue };
                if !cable {
                    if !h.write(&request(TYPE_DONGLE, c.output_len), dl(READ_MS)) {
                        continue;
                    }
                    let Some(r) = wait(&mut h, TYPE_DONGLE) else {
                        continue;
                    };
                    // r[4] == 1：耳机已连接。
                    if r.len() <= 4 || r[4] != 1 {
                        continue;
                    }
                }
                if !h.write(&request(TYPE_HEADSET, c.output_len), dl(READ_MS)) {
                    continue;
                }
                let Some(r) = wait(&mut h, TYPE_HEADSET) else {
                    continue;
                };
                if let Some((level, charging)) = battery(&r) {
                    return vec![status(
                        "fractal:scape",
                        "Fractal Scape",
                        "headset",
                        "fractal",
                        Some(level),
                        Some(charging),
                        true,
                        true,
                    )];
                }
            }
            if eligible {
                // Python：集合合格但无应答时显示一个离线图标。
                return vec![status(
                    "fractal:scape",
                    "Fractal Scape",
                    "headset",
                    "fractal",
                    None,
                    Some(false),
                    false,
                    true,
                )];
            }
            Vec::new()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_and_reply() {
            let req = request(TYPE_DONGLE, 64);
            assert_eq!(req.len(), 64);
            assert_eq!(&req[..3], &[0x02, 0x11, 0x21]);
            assert_eq!(request(TYPE_HEADSET, 64)[1], 0xF1);

            // 按源码 docstring 的字段位置构造的状态帧（r[4]=已连接，r[15]=75%，r[23]=充电）。
            let mut r = vec![0u8; 24];
            r[0] = REPORT;
            r[1] = TYPE_HEADSET;
            r[2] = GET_STATE;
            r[4] = 1;
            r[15] = 75;
            r[23] = 1;
            assert!(parse_state(&r, TYPE_DONGLE).is_none());
            assert!(parse_state(&r, TYPE_HEADSET).is_some());
            assert_eq!(battery(&r), Some((75, true)));
            r[15] = 101;
            assert_eq!(battery(&r), None);
        }
    }
}

// ===========================================================================
// Fantech Helios Go XD5 / Redragon Invader Pro：VID 25A7，
// FA7B 有线 / FA7C 接收器 / FA7E Redragon。17 字节 feature report 08，
// cmd 04，末字节使整帧和为 0x55。回复可能在 input report 或 feature report 上，
// 支持三种字节布局；Redragon 的电量以十分之一为单位。
// ===========================================================================
mod fantech {
    use super::*;

    const VID: u16 = 0x25A7;
    const WIRED: u16 = 0xFA7B;
    const DONGLE: u16 = 0xFA7C;
    const REDRAGON: u16 = 0xFA7E;
    const PIDS: &[u16] = &[WIRED, DONGLE, REDRAGON];
    const REPORT: u8 = 0x08;
    const CMD: u8 = 0x04;
    const LEN: usize = 17;
    const LISTEN_MS: u64 = 500;
    const READ_MS: u64 = 250;

    fn request() -> [u8; LEN] {
        let mut p = [0u8; LEN];
        p[0] = REPORT;
        p[1] = CMD;
        p[16] = check55(&p[..16]);
        p
    }

    fn plausible(b: &[u8], power: usize, charge: usize, tenths: bool) -> Option<(u8, bool, bool)> {
        if b.len() <= charge || b[power] < 1 || b[power] > 100 || b[charge] > 1 {
            return None;
        }
        let level = if tenths {
            b[power] as u16 * 10
        } else {
            b[power] as u16
        };
        if level > 100 {
            return None;
        }
        let checksum_ok = b.len() >= LEN && sum8(&b[..LEN]) == 0x55;
        Some((level as u8, b[charge] == 1, checksum_ok))
    }

    fn parse_reply(r: &[u8], tenths: bool) -> Option<(u8, bool, bool)> {
        if r.is_empty() {
            return None;
        }
        if r.len() >= 7 && r[0] == 0x04 {
            if let Some(v) = plausible(r, 5, 6, tenths) {
                return Some(v);
            }
            if let Some(v) = plausible(r, 4, 5, tenths) {
                return Some(v);
            }
        }
        if r.len() >= 8 && r[1] == 0x04 {
            return plausible(r, 6, 7, tenths);
        }
        None
    }

    pub struct Fantech;

    impl Provider for Fantech {
        fn name(&self) -> &'static str {
            "fantech"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID || !PIDS.contains(&pid) {
                return Vec::new();
            }
            let Some(cmd) = collections.iter().find(|c| c.feature_len >= LEN) else {
                return Vec::new();
            };
            let Some(mut h) = cmd.open() else {
                return Vec::new();
            };
            let tenths = pid == REDRAGON;
            let req = request();
            if !h.set_feature(&req, dl(READ_MS)) {
                return Vec::new();
            }

            // Python 把命令集合（若有 input report）以及同 PID 的其它 input 集合都当 reader。
            let mut readers: Vec<Handle> = collections
                .iter()
                .filter(|c| !std::ptr::eq(*c, cmd) && c.input_len >= 6)
                .filter_map(|c| c.open())
                .collect();
            let end = dl(LISTEN_MS);
            let mut found: Option<(u8, bool, bool)> = None;
            while Instant::now() < end && !found.map_or(false, |f| f.2) {
                let mut saw = false;
                if cmd.input_len >= 6 {
                    if let Some(r) = read_until(&mut h, end, READ_MS) {
                        saw = true;
                        if let Some(p) = parse_reply(&r, tenths) {
                            if found.is_none() || p.2 {
                                found = Some(p);
                            }
                        }
                    }
                }
                for rh in &mut readers {
                    if let Some(r) = read_until(rh, end, READ_MS) {
                        saw = true;
                        if let Some(p) = parse_reply(&r, tenths) {
                            if found.is_none() || p.2 {
                                found = Some(p);
                            }
                        }
                    }
                }
                if !saw && Instant::now() >= end {
                    break;
                }
            }
            // input 没有拿到带校验的回复时，回退到 get_feature(08, 17)。
            if found.map_or(true, |f| !f.2) {
                if let Some(r) = h.get_feature(REPORT, dl(READ_MS)) {
                    if let Some(p) = parse_reply(&r, tenths) {
                        found = Some(p);
                    }
                }
            }
            let Some((level, charging, _)) = found else {
                return Vec::new();
            };
            let name = if pid == REDRAGON {
                "Redragon Invader Pro"
            } else {
                "Fantech Helios Go XD5"
            };
            vec![status(
                format!("fantech:{pid:04x}"),
                name,
                "mouse",
                "fantech",
                Some(level),
                Some(charging),
                true,
                true,
            )]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// 按源码布局构造一条 17 字节回复并补齐 0x55 校验。
        fn reply(level: u8, charging: u8) -> Vec<u8> {
            let mut r = vec![0u8; LEN];
            r[0] = REPORT;
            r[1] = CMD;
            // 有 report id 的布局：level 在 6，charging 在 7。
            r[6] = level;
            r[7] = charging;
            r[16] = check55(&r[..16]);
            r
        }

        #[test]
        fn request_and_reply() {
            let req = request();
            assert_eq!(req.len(), 17);
            assert_eq!(&req[..2], &[0x08, 0x04]);
            assert_eq!(req[16], 0x49); // 0x55 - (0x08 + 0x04)

            let r = reply(75, 0);
            assert_eq!(parse_reply(&r, false), Some((75, false, true)));
            // 去掉 report id 后，首字节 04 会按旧布局匹配（level/charge 在 5/6）。
            assert_eq!(parse_reply(&r[1..], false), Some((75, false, false)));

            // 无 report id 的旧布局：b[0]==04，level/charge 在 5/6。
            let mut old = vec![0u8; 7];
            old[0] = CMD;
            old[5] = 84;
            old[6] = 1;
            assert_eq!(parse_reply(&old, false), Some((84, true, false)));

            // Redragon 以十分之一上报：level 字节 2 -> 20%。
            let rr = reply(2, 1);
            assert_eq!(parse_reply(&rr, true), Some((20, true, true)));
        }
    }
}

// ===========================================================================
// Endgame Gear XM2we：VID 3367 / PID 1961。feature 08 发 17 字节查询，
// input 09 收 17 字节回复；整帧和 0x55，r[2:5]=0，r[5] 为 payload 长度。
// BATTERY=04 先查，LINK=03 后查（插线时链路会短暂掉）。
// FF02:0002 发命令，FF01:0000 收 input。
// ===========================================================================
mod endgame_we {
    use super::*;

    const VID: u16 = 0x3367;
    const PID: u16 = 0x1961;
    const BATTERY: u8 = 0x04;
    const LINK: u8 = 0x03;
    const CMD_PAGE: u16 = 0xFF02;
    const CMD_USAGE: u16 = 0x0002;
    const IN_PAGE: u16 = 0xFF01;
    const IN_USAGE: u16 = 0x0000;
    const TIMEOUT_MS: u64 = 400;
    const SEND_MS: u64 = 250;

    fn request(opcode: u8) -> [u8; 17] {
        let mut d = [0u8; 17];
        d[0] = 0x08;
        d[1] = opcode;
        d[16] = check55(&d[..16]);
        d
    }

    fn payload(data: &[u8], opcode: u8) -> Option<Vec<u8>> {
        if data.len() != 17 || data[0] != 0x09 || data[1] != opcode {
            return None;
        }
        if sum8(data) != 0x55 || data[2] != 0 || data[3] != 0 || data[4] != 0 {
            return None;
        }
        let size = data[5] as usize;
        if !(1..=10).contains(&size) || 6 + size > data.len() {
            return None;
        }
        Some(data[6..6 + size].to_vec())
    }

    fn query(cmd: &mut Handle, reader: &mut Handle, opcode: u8) -> Option<Vec<u8>> {
        if !cmd.set_feature(&request(opcode), dl(SEND_MS)) {
            return None;
        }
        let end = dl(TIMEOUT_MS);
        while Instant::now() < end {
            let Some(r) = read_until(reader, end, 50) else {
                continue;
            };
            if let Some(p) = payload(&r, opcode) {
                return Some(p);
            }
        }
        None
    }

    pub struct EndgameWe;

    impl Provider for EndgameWe {
        fn name(&self) -> &'static str {
            "endgame_we"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if (vid, pid) != (VID, PID) {
                return Vec::new();
            }
            let Some(cmd) = collections
                .iter()
                .find(|c| (c.usage_page, c.usage) == (CMD_PAGE, CMD_USAGE))
            else {
                return Vec::new();
            };
            let Some(reader) = collections
                .iter()
                .find(|c| (c.usage_page, c.usage) == (IN_PAGE, IN_USAGE))
            else {
                return Vec::new();
            };
            let Some(mut rh) = reader.open() else {
                return Vec::new();
            };
            let Some(mut ch) = cmd.open() else {
                return Vec::new();
            };

            let battery = query(&mut ch, &mut rh, BATTERY);
            let link = query(&mut ch, &mut rh, LINK);
            let Some(battery) = battery else {
                return Vec::new();
            };
            if battery.is_empty() || battery[0] > 100 {
                return Vec::new();
            }
            let charging_known = battery.len() > 1 && matches!(battery[1], 0 | 1);
            let charging = battery.len() > 1 && battery[1] == 1;
            let online = charging || link.as_ref().is_some_and(|l| l.first() == Some(&1));
            // Python 用 SHA-256(HID path)[:16] 作 key；Rust 无 path，改用 USB instance。
            let key = format!("endgame_we:{}", &sha256_hex(instance)[..16]);
            vec![status(
                key,
                "Endgame Gear XM2we",
                "mouse",
                "endgame_we",
                Some(battery[0]),
                Some(charging),
                online,
                charging_known,
            )]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_and_reply() {
            let req = request(BATTERY);
            assert_eq!(req.len(), 17);
            assert_eq!(&req[..2], &[0x08, 0x04]);
            assert_eq!(req[16], 0x49);
            assert_eq!(request(LINK)[16], 0x4A);

            // 09 04 00 00 00 02 <level> <charging> + padding，末字节补齐 0x55。
            let mut r = vec![0u8; 17];
            r[0] = 0x09;
            r[1] = BATTERY;
            r[5] = 2;
            r[6] = 87;
            r[7] = 1;
            r[16] = check55(&r[..16]);
            assert_eq!(sum8(&r), 0x55);
            assert_eq!(payload(&r, BATTERY), Some(vec![87, 1]));
            assert_eq!(payload(&r, LINK), None);
            r[2] = 1;
            assert_eq!(payload(&r, BATTERY), None);
        }
    }
}

// ===========================================================================
// Plantronics / Poly BT600：VID 047F，PID 02EE / 02F7，FFA2:0003 iface 3。
// 64 字节 report 07 隧道帧：07 01 01 10 <len> <addr> 00 00 <op> <grp> <item>。
// 每次读取前重放 attach 序列：2 字节 06 01 -> dongle HELLO -> headset HELLO
// -> GET 电量。电量 = level / (count - 1) * 100。
// ===========================================================================
mod plantronics {
    use super::*;

    const VID: u16 = 0x047F;
    const PIDS: &[u16] = &[0x02EE, 0x02F7];
    const PAGE: u16 = 0xFFA2;
    const USAGE: u16 = 0x0003;
    const IFACE: u8 = 3;
    const REPORT_ID: u8 = 0x07;
    const MSG_SIZE: usize = 64;
    const HEADER: [u8; 3] = [0x01, 0x01, 0x10];
    const OFF_ADDR: usize = 5;
    const OFF_OP: usize = 8;
    const OFF_GRP: usize = 9;
    const OFF_ITEM: usize = 10;
    const OFF_LEVEL: usize = 11;
    const OFF_COUNT: usize = 12;
    const OFF_CHARGING: usize = 13;
    const ADDR_DONGLE: u8 = 0x00;
    const ADDR_DONGLE_REPLY: u8 = 0x00;
    const ADDR_HEADSET: u8 = 0x20;
    const ADDR_HEADSET_REPLY: u8 = 0x02;
    const OP_HELLO: u8 = 0x01;
    const OP_GET: u8 = 0x02;
    const OP_HELLO_ACK: u8 = 0x08;
    const OP_GET_REPLY: u8 = 0x03;
    const GRP_HELLO: u8 = 0x01;
    const ITEM_HELLO: u8 = 0x02;
    const GRP_BATTERY: u8 = 0x0A;
    const ITEM_BATTERY: u8 = 0x1A;
    const SESSION_NUDGE: [u8; 2] = [0x06, 0x01];
    const MAX_READS: usize = 16;
    // Python 用 1000 ms；Rust 用 250 ms × 16 次以不超过 worker 的 20 s 预算。
    const READ_MS: u64 = 250;

    fn frame(op: u8, grp: u8, item: u8, addr: u8) -> [u8; MSG_SIZE] {
        let mut f = [0u8; MSG_SIZE];
        f[0] = REPORT_ID;
        f[1] = HEADER[0];
        f[2] = HEADER[1];
        f[3] = HEADER[2];
        f[4] = if op == OP_GET { 0x06 } else { 0x07 };
        f[OFF_ADDR] = addr;
        f[OFF_OP] = op;
        f[OFF_GRP] = grp;
        f[OFF_ITEM] = item;
        f
    }

    fn is_reply_for(r: &[u8], grp: u8, item: u8, addr: u8) -> bool {
        r.len() > OFF_ITEM
            && r[0] == REPORT_ID
            && r[1] == HEADER[0]
            && r[2] == HEADER[1]
            && r[3] == HEADER[2]
            && r[OFF_ADDR] == addr
            && r[OFF_GRP] == grp
            && r[OFF_ITEM] == item
    }

    fn parse_battery(r: &[u8]) -> Option<(u8, bool)> {
        if !is_reply_for(r, GRP_BATTERY, ITEM_BATTERY, ADDR_HEADSET_REPLY)
            || r[OFF_OP] != OP_GET_REPLY
        {
            return None;
        }
        let count = r[OFF_COUNT];
        if count < 2 {
            return None;
        }
        let level = r[OFF_LEVEL];
        if level >= count {
            return None;
        }
        Some((
            (level as u16 * 100 / (count as u16 - 1)).min(100) as u8,
            r[OFF_CHARGING] != 0,
        ))
    }

    fn await_reply(h: &mut Handle, op: Option<u8>, grp: u8, item: u8, addr: u8) -> Option<Vec<u8>> {
        for _ in 0..MAX_READS {
            let Some(r) = h.read(dl(READ_MS)) else {
                return None;
            };
            if is_reply_for(&r, grp, item, addr) && op.map_or(true, |o| r[OFF_OP] == o) {
                return Some(r);
            }
        }
        None
    }

    pub struct Plantronics;

    impl Provider for Plantronics {
        fn name(&self) -> &'static str {
            "plantronics"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID || !PIDS.contains(&pid) {
                return Vec::new();
            }
            // Python 在 FFA2:0003 的集合里按 interface 排序逐个尝试；
            // 这里优先 iface 3，找不到时退到任意 FFA2:0003。
            let Some(c) = collections
                .iter()
                .find(|c| (c.usage_page, c.usage) == (PAGE, USAGE) && c.interface == Some(IFACE))
                .or_else(|| {
                    collections
                        .iter()
                        .find(|c| (c.usage_page, c.usage) == (PAGE, USAGE))
                })
            else {
                return Vec::new();
            };
            let Some(mut h) = c.open() else {
                return Vec::new();
            };
            let _ = h.write(&SESSION_NUDGE, dl(READ_MS));
            for (addr, reply) in [
                (ADDR_DONGLE, ADDR_DONGLE_REPLY),
                (ADDR_HEADSET, ADDR_HEADSET_REPLY),
            ] {
                let _ = h.write(&frame(OP_HELLO, GRP_HELLO, ITEM_HELLO, addr), dl(READ_MS));
                let got = await_reply(&mut h, Some(OP_HELLO_ACK), GRP_HELLO, ITEM_HELLO, reply);
                if got.is_none() && addr == ADDR_HEADSET {
                    // 耳机未连接：关机 / 超出范围 / 未配对。
                    return Vec::new();
                }
            }
            let _ = h.write(
                &frame(OP_GET, GRP_BATTERY, ITEM_BATTERY, ADDR_HEADSET),
                dl(READ_MS),
            );
            let Some(r) = await_reply(
                &mut h,
                Some(OP_GET_REPLY),
                GRP_BATTERY,
                ITEM_BATTERY,
                ADDR_HEADSET_REPLY,
            ) else {
                return Vec::new();
            };
            let Some((level, charging)) = parse_battery(&r) else {
                return Vec::new();
            };
            vec![status(
                format!("plantronics:{pid:04x}"),
                "Plantronics headset",
                "headset",
                "plantronics",
                Some(level),
                Some(charging),
                true,
                true,
            )]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_and_reply() {
            let hello = frame(OP_HELLO, GRP_HELLO, ITEM_HELLO, ADDR_HEADSET);
            assert_eq!(hello.len(), 64);
            assert_eq!(
                &hello[..11],
                &[0x07, 0x01, 0x01, 0x10, 0x07, 0x20, 0, 0, 0x01, 0x01, 0x02]
            );
            let get = frame(OP_GET, GRP_BATTERY, ITEM_BATTERY, ADDR_HEADSET);
            assert_eq!(get[4], 0x06); // GET 的长度字节是 06，其它是 07
            assert_eq!(&get[8..11], &[0x02, 0x0A, 0x1A]);

            // 07 01 01 10 06 02 00 00 03 0A 1A <level> <count> <charging> ...
            let mut r = [0u8; MSG_SIZE];
            r[1..4].copy_from_slice(&HEADER);
            r[0] = REPORT_ID;
            r[4] = 0x06;
            r[OFF_ADDR] = ADDR_HEADSET_REPLY;
            r[OFF_OP] = OP_GET_REPLY;
            r[OFF_GRP] = GRP_BATTERY;
            r[OFF_ITEM] = ITEM_BATTERY;
            r[OFF_LEVEL] = 7; // 10 级中的第 7 级 -> 70%
            r[OFF_COUNT] = 11;
            r[OFF_CHARGING] = 1;
            r[14] = 0x01; // 剩余分钟高位
            r[15] = 0x2C;
            assert!(is_reply_for(
                &r,
                GRP_BATTERY,
                ITEM_BATTERY,
                ADDR_HEADSET_REPLY
            ));
            assert_eq!(parse_battery(&r), Some((70, true)));
            r[OFF_LEVEL] = 11;
            assert_eq!(parse_battery(&r), None);
        }
    }
}

// ===========================================================================
// Wraith W1 / W1 Freestyle：VID 093A，522C 接收器 / 622C 线缆，FF05。
// feature 09 8F 探型号（回复 r[3] 18/20）；feature 09 86 <button> 按键后
// input report 09 主动推送电量：bit7 充电，低 7 位百分比。
// ===========================================================================
mod wraith {
    use super::*;

    const VID: u16 = 0x093A;
    const RECEIVER: u16 = 0x522C;
    const CABLE: u16 = 0x622C;
    const PIDS: &[u16] = &[RECEIVER, CABLE];
    const PAGE: u16 = 0xFF05;
    const REPORT: u8 = 0x09;
    const MODEL_CMD: u8 = 0x8F;
    const STATUS_CMD: u8 = 0x86;
    const BUTTONS: &[u8] = &[0, 1, 2, 4, 3];
    const READ_MS: u64 = 50;
    const LISTEN_MS: u64 = 400;
    const SEND_MS: u64 = 250;

    fn model_name(model: u8) -> &'static str {
        if model == 20 {
            "Wraith W1 Freestyle"
        } else {
            "Wraith W1"
        }
    }

    fn parse_status(r: &[u8]) -> Option<(u8, bool)> {
        if r.len() < 2 || r[0] != REPORT {
            return None;
        }
        let raw = r[1];
        let level = raw & 0x7F;
        if level > 100 {
            return None;
        }
        Some((level, raw & 0x80 != 0))
    }

    fn probe(c: &Collection) -> Option<u8> {
        let mut h = c.open()?;
        if !h.set_feature(&[REPORT, MODEL_CMD], dl(SEND_MS)) {
            return None;
        }
        let end = dl(500);
        while Instant::now() < end {
            let Some(r) = h.get_feature(REPORT, read_deadline(end)) else {
                continue;
            };
            if r.len() >= 4 && r[0] == REPORT && r[1] == MODEL_CMD {
                return if r[3] == 18 || r[3] == 20 {
                    Some(r[3])
                } else {
                    None
                };
            }
        }
        None
    }

    fn read_deadline(end: Instant) -> Instant {
        (Instant::now() + Duration::from_millis(READ_MS)).min(end)
    }

    fn button(h: &mut Handle, button: u8) -> bool {
        if !h.set_feature(&[REPORT, STATUS_CMD, button], dl(SEND_MS)) {
            return false;
        }
        sleep(Duration::from_millis(150));
        let _ = h.get_feature(REPORT, dl(SEND_MS));
        sleep(Duration::from_millis(20));
        let _ = h.get_feature(REPORT, dl(SEND_MS));
        true
    }

    fn listen(readers: &mut [Handle], ms: u64) -> Option<(u8, bool)> {
        let end = dl(ms);
        while Instant::now() < end {
            for h in readers.iter_mut() {
                if let Some(r) = read_until(h, end, READ_MS) {
                    if let Some(v) = parse_status(&r) {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    pub struct Wraith;

    impl Provider for Wraith {
        fn name(&self) -> &'static str {
            "wraith"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID || !PIDS.contains(&pid) {
                return Vec::new();
            }
            let colls: Vec<&Collection> = collections
                .iter()
                .filter(|c| c.usage_page == PAGE && c.feature_len >= 9)
                .collect();
            if colls.is_empty() {
                return Vec::new();
            }
            let mut model = None;
            for c in colls.iter().rev() {
                if let Some(m) = probe(c) {
                    model = Some(m);
                    break;
                }
            }
            let Some(model) = model else {
                return Vec::new();
            };

            let mut readers: Vec<Handle> = collections
                .iter()
                .filter(|c| c.usage_page == PAGE && c.input_len >= 2)
                .filter_map(|c| c.open())
                .collect();
            if readers.is_empty() {
                return Vec::new();
            }
            let Some(cmd) = colls.last() else {
                return Vec::new();
            };
            let Some(mut ch) = cmd.open() else {
                return Vec::new();
            };
            sleep(Duration::from_millis(200));

            let mut found = None;
            for b in BUTTONS {
                if !button(&mut ch, *b) {
                    continue;
                }
                found = listen(&mut readers, LISTEN_MS);
                if found.is_some() {
                    break;
                }
            }
            for _ in 0..4 {
                if found.is_some() {
                    break;
                }
                if !button(&mut ch, 0) {
                    break;
                }
                found = listen(&mut readers, 550);
            }
            let (level, charging, online) = match found {
                Some((l, c)) => (Some(l), Some(c), true),
                None => (None, Some(false), false),
            };
            vec![status(
                format!("wraithw1:{pid:04x}"),
                model_name(model),
                "mouse",
                "wraith",
                level,
                charging,
                online,
                true,
            )]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_and_reply() {
            // Python 的 feature report 发送字节（不含 report id 由 hid 库补齐）。
            assert_eq!([REPORT, MODEL_CMD], [0x09, 0x8F]);
            assert_eq!([REPORT, STATUS_CMD, 0], [0x09, 0x86, 0x00]);

            // 型号探测：09 8F .. 18 -> W1；r[3]=20 -> Freestyle。
            let mut probe_reply = vec![0u8; 9];
            probe_reply[0] = REPORT;
            probe_reply[1] = MODEL_CMD;
            probe_reply[3] = 18;
            assert!(probe_reply.len() >= 4 && probe_reply[3] == 18);
            assert_eq!(model_name(18), "Wraith W1");
            assert_eq!(model_name(20), "Wraith W1 Freestyle");

            // input 09 4B -> 75% 未充电；09 CB -> 75% 充电。
            assert_eq!(parse_status(&[REPORT, 0x4B]), Some((75, false)));
            assert_eq!(parse_status(&[REPORT, 0xCB]), Some((75, true)));
            assert_eq!(parse_status(&[REPORT, 0xFF]), None); // 127% 越界
            assert_eq!(parse_status(&[0x08, 0x4B]), None);
        }
    }
}

// ===========================================================================
// Jabra Link 380/390 后的耳机（GNP 协议）：VID 0B0E，FF00:0001。
// 请求 05 <dst=04> 00 <seq> <40|len> <class> <op>；回复 05 00 <dst> <seq>
// <C0|len> <class> <op>。class FE 是 NAK。电量 class 12 / op 02：
// payload[0] bit0=充电，payload[1]=百分比；名称 class 02 / op 00。
// ===========================================================================
mod jabra {
    use super::*;

    const VID: u16 = 0x0B0E;
    const PAGE: u16 = 0xFF00;
    const USAGE: u16 = 0x0001;
    const REPORT_ID: u8 = 0x05;
    const FRAME_SIZE: usize = 64;
    const DST_HEADSET: u8 = 0x04;
    const FLAG_QUERY: u8 = 0x40;
    const FLAG_REPLY: u8 = 0xC0;
    const CLASS_NAK: u8 = 0xFE;
    const CLASS_DEVINFO: u8 = 0x02;
    const OP_NAME: u8 = 0x00;
    const CLASS_STATUS: u8 = 0x12;
    const OP_BATTERY: u8 = 0x02;
    const READ_MS: u64 = 300;
    const ATTEMPTS: usize = 6;
    const PIDS: &[(u16, &str)] = &[
        (0x24C7, "Jabra Link 380"),
        (0x24C8, "Jabra Link 380"),
        (0x24C9, "Jabra Link 380"),
        (0x24CA, "Jabra Link 380"),
        (0x24E9, "Jabra Link 380"),
        (0x2E57, "Jabra Link 390"),
    ];

    enum Reply {
        Nak,
        Data(Vec<u8>),
    }

    fn request(seq: u8, cls: u8, op: u8) -> [u8; FRAME_SIZE] {
        let mut f = [0u8; FRAME_SIZE];
        f[0] = REPORT_ID;
        f[1] = DST_HEADSET;
        f[2] = 0x00;
        f[3] = seq;
        f[4] = FLAG_QUERY | 6;
        f[5] = cls;
        f[6] = op;
        f
    }

    fn reply_payload(r: &[u8], seq: u8, cls: u8, op: u8) -> Option<Reply> {
        if r.len() < 6 {
            return None;
        }
        if r[0] != REPORT_ID || r[1] != 0x00 || r[2] != DST_HEADSET || r[3] != seq {
            return None;
        }
        if r[4] & 0xC0 != FLAG_REPLY {
            return None;
        }
        if r[5] == CLASS_NAK {
            return Some(Reply::Nak);
        }
        if r.len() < 7 || r[5] != cls || r[6] != op {
            return None;
        }
        let length = (r[4] & 0x3F) as usize;
        if length < 6 || length + 1 > r.len() {
            return None;
        }
        Some(Reply::Data(r[7..length + 1].to_vec()))
    }

    fn parse_name(payload: &[u8]) -> Option<String> {
        if payload.is_empty() {
            return None;
        }
        let n = payload[0] as usize;
        if n == 0 || n > payload.len() - 1 {
            return None;
        }
        let s = String::from_utf8_lossy(&payload[1..1 + n])
            .trim()
            .to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    fn parse_battery(payload: &[u8]) -> Option<(u8, bool)> {
        if payload.len() < 2 {
            return None;
        }
        let level = payload[1];
        if level > 100 {
            return None;
        }
        Some((level, payload[0] & 0x01 != 0))
    }

    #[derive(Default)]
    struct JState {
        seq: u8,
        names: HashMap<String, String>,
    }

    pub struct Jabra {
        state: Mutex<JState>,
    }

    impl Jabra {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(JState {
                    seq: 0x20,
                    ..JState::default()
                }),
            }
        }

        fn next_seq(&self) -> u8 {
            let mut s = self.state.lock().unwrap();
            let v = s.seq;
            s.seq = s.seq.wrapping_add(1);
            v
        }

        fn query(&self, h: &mut Handle, cls: u8, op: u8) -> Option<Vec<u8>> {
            let seq = self.next_seq();
            if !h.write(&request(seq, cls, op), dl(READ_MS)) {
                return None;
            }
            for _ in 0..ATTEMPTS {
                if let Some(r) = h.read(dl(READ_MS)) {
                    match reply_payload(&r, seq, cls, op) {
                        Some(Reply::Data(p)) => return Some(p),
                        Some(Reply::Nak) => return Some(Vec::new()),
                        None => continue,
                    }
                }
            }
            None
        }
    }

    impl Default for Jabra {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Jabra {
        fn name(&self) -> &'static str {
            "jabra"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID {
                return Vec::new();
            }
            let Some((_, dongle_name)) = PIDS.iter().find(|(p, _)| *p == pid) else {
                return Vec::new();
            };
            let Some(c) = collections
                .iter()
                .find(|c| (c.usage_page, c.usage) == (PAGE, USAGE))
            else {
                return Vec::new();
            };
            let Some(mut h) = c.open() else {
                return Vec::new();
            };

            let battery = self.query(&mut h, CLASS_STATUS, OP_BATTERY);
            let Some(battery) = battery else {
                self.state.lock().unwrap().names.remove(instance);
                return Vec::new();
            };
            let Some((level, charging)) = parse_battery(&battery) else {
                self.state.lock().unwrap().names.remove(instance);
                return Vec::new();
            };

            // 名称只在电量成功、且缓存未命中时查询；缓存 key 用 instance 代替 HID path。
            let mut name = self.state.lock().unwrap().names.get(instance).cloned();
            if name.is_none() {
                let payload = self.query(&mut h, CLASS_DEVINFO, OP_NAME);
                if let Some(n) = payload.as_deref().and_then(parse_name) {
                    self.state
                        .lock()
                        .unwrap()
                        .names
                        .insert(instance.to_string(), n.clone());
                    name = Some(n);
                }
            }
            let display = name.unwrap_or_else(|| format!("Jabra headset ({dongle_name})"));
            vec![status(
                format!("jabra:{pid:04x}"),
                display,
                "headset",
                "jabra",
                Some(level),
                Some(charging),
                true,
                true,
            )]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_and_reply() {
            let req = request(0x20, CLASS_STATUS, OP_BATTERY);
            assert_eq!(&req[..7], &[0x05, 0x04, 0x00, 0x20, 0x46, 0x12, 0x02]);
            assert_eq!(req.len(), 64);

            // jabra.py docstring 的 Evolve2 65：payload `04 15` = 21%，未充电。
            let mut r = [0u8; FRAME_SIZE];
            r[0] = REPORT_ID;
            r[1] = 0x00;
            r[2] = DST_HEADSET;
            r[3] = 0x20;
            r[4] = FLAG_REPLY | 8; // 6 字节头 + 2 字节 payload
            r[5] = CLASS_STATUS;
            r[6] = OP_BATTERY;
            r[7] = 0x04;
            r[8] = 0x15;
            match reply_payload(&r, 0x20, CLASS_STATUS, OP_BATTERY) {
                Some(Reply::Data(p)) => assert_eq!(parse_battery(&p), Some((21, false))),
                _ => panic!("expected battery payload"),
            }
            r[7] = 0x05; // bit0 -> charging
            match reply_payload(&r, 0x20, CLASS_STATUS, OP_BATTERY) {
                Some(Reply::Data(p)) => assert_eq!(parse_battery(&p), Some((21, true))),
                _ => panic!("expected battery payload"),
            }
            r[3] = 0x21;
            assert!(reply_payload(&r, 0x20, CLASS_STATUS, OP_BATTERY).is_none());
            r[3] = 0x20;
            r[5] = CLASS_NAK;
            assert!(matches!(
                reply_payload(&r, 0x20, CLASS_STATUS, OP_BATTERY),
                Some(Reply::Nak)
            ));

            // 名称 payload：<n> <n bytes UTF-8>
            assert_eq!(
                parse_name(&[6, b'J', b'a', b'b', b'r', b'a', b'?']).as_deref(),
                Some("Jabra?")
            );
        }
    }
}

// ===========================================================================
// G-Wolves 共用 8K 接收器 / 自带接收器 / 线缆：VID 33E4。
// 新协议 feature 00 64 字节 00 00 02 02 00 83 ...，回复 a1 00 02 02 00 83
// <charging> <level>；旧协议 00 02 8f <01 over receiver, 00 on cable> ...，
// 回复 a1 02 8f .. <charging> <level>。只接受 FeatureReportByteLength == 65
// 的集合（id 字节 + 64 字节）。
// ===========================================================================
mod gwolves {
    use super::*;

    const VID: u16 = 0x33E4;
    const RECEIVER: u16 = 0x3854;
    const FEATURE_LENGTH: usize = 65;
    const REPLY_TRIES: usize = 15;
    const ASLEEP_KEEP: u64 = 300;
    const fn new_request() -> [u8; 64] {
        let mut r = [0u8; 64];
        r[2] = 0x02;
        r[3] = 0x02;
        r[5] = 0x83;
        r
    }
    const NEW_REQ: [u8; 64] = new_request();

    // pid -> (name, icon pid, NEW/OLD, wired)
    const MODELS: &[(u16, &str, u16, bool, bool)] = &[
        (0x3808, "G-Wolves HTM Plus", 0x3817, false, true),
        (0x3817, "G-Wolves HTM Plus", 0x3817, false, false),
        (0x6808, "G-Wolves HSK Pro 2.0", 0x6817, false, true),
        (0x6817, "G-Wolves HSK Pro 2.0", 0x6817, false, false),
        (0x5608, "G-Wolves HTXU", 0x5617, false, true),
        (0x5617, "G-Wolves HTXU", 0x5617, false, false),
        (0x3608, "G-Wolves Fenrir Pro", 0x3617, false, true),
        (0x3617, "G-Wolves Fenrir Pro", 0x3617, false, false),
        (0x3908, "G-Wolves VUK", 0x3917, true, true),
        (0x3917, "G-Wolves VUK", 0x3917, true, false),
        (0x7904, "G-Wolves HT-S2", 0x7913, true, true),
        (0x7913, "G-Wolves HT-S2", 0x7913, true, false),
        (0x3708, "G-Wolves Fenrir Max", 0x3717, true, true),
        (0x3717, "G-Wolves Fenrir Max", 0x3717, true, false),
        (0x5308, "G-Wolves HTS Ultra", 0x5317, true, true),
        (0x5317, "G-Wolves HTS Ultra", 0x5317, true, false),
        (0x7704, "G-Wolves HTR", 0x7713, true, true),
        (0x7713, "G-Wolves HTR", 0x7713, true, false),
        (0x3508, "G-Wolves Fenrir", 0x3517, true, true),
        (0x3517, "G-Wolves Fenrir", 0x3517, true, false),
        (0x7908, "G-Wolves HT-S2 Pro", 0x7917, true, true),
        (0x7917, "G-Wolves HT-S2 Pro", 0x7917, true, false),
        (0x5808, "G-Wolves HSK Pro", 0x5817, true, true),
        (0x5817, "G-Wolves HSK Pro", 0x5817, true, false),
        (0x5807, "G-Wolves HSK Pro", 0x5817, true, false),
        (0x2708, "G-Wolves HTX Mini", 0x2717, true, true),
        (0x2717, "G-Wolves HTX Mini", 0x2717, true, false),
        (0x5408, "G-Wolves HTS Plus", 0x5417, true, true),
        (0x5417, "G-Wolves HTS Plus", 0x5417, true, false),
        (0x5407, "G-Wolves HTS Plus", 0x5417, true, false),
        (0x5708, "G-Wolves HTX", 0x5717, true, true),
        (0x5717, "G-Wolves HTX", 0x5717, true, false),
        (0x5707, "G-Wolves HTX", 0x5717, true, false),
        (0x5908, "G-Wolves HSK Plus", 0x5917, true, true),
        (0x5917, "G-Wolves HSK Plus", 0x5917, true, false),
        (0x5907, "G-Wolves HSK Plus", 0x5917, true, false),
        (0x7204, "G-Wolves HSK Lite", 0x7203, true, true),
        (0x7203, "G-Wolves HSK Lite", 0x7203, true, false),
        (0x7708, "G-Wolves HTR Pro", 0x7717, true, true),
        (0x7717, "G-Wolves HTR Pro", 0x7717, true, false),
        (0x5804, "G-Wolves HSK Pro ACE", 0x5803, true, true),
        (0x5803, "G-Wolves HSK Pro ACE", 0x5803, true, false),
        (0x5404, "G-Wolves HTS Plus ACE", 0x5403, true, true),
        (0x5403, "G-Wolves HTS Plus ACE", 0x5403, true, false),
        (0x5704, "G-Wolves HTX ACE", 0x5703, true, true),
        (0x5703, "G-Wolves HTX ACE", 0x5703, true, false),
        (0x5904, "G-Wolves HSK Plus ACE", 0x5903, true, true),
        (0x5903, "G-Wolves HSK Plus ACE", 0x5903, true, false),
    ];
    const WIRED: &[(u16, &str)] = &[
        (0x5418, "G-Wolves HTS Plus"),
        (0x5419, "G-Wolves HTS Plus"),
        (0x5219, "G-Wolves HTS Plus Pro"),
        (0x4718, "G-Wolves Lycan"),
        (0x4719, "G-Wolves Lycan"),
        (0x5618, "G-Wolves HTXU"),
        (0x5619, "G-Wolves HTXU"),
        (0x3619, "G-Wolves Fenrir Pro"),
        (0x3519, "G-Wolves Fenrir Asym"),
        (0x2719, "G-Wolves HTX Mini"),
        (0x4219, "G-Wolves WARG"),
    ];
    const DEFAULT_NAME: &str = "G-Wolves mouse";

    fn old_request(wired: bool) -> [u8; 64] {
        let mut r = [0u8; 64];
        r[0] = 0x00;
        r[1] = 0x02;
        r[2] = 0x8F;
        r[3] = if wired { 0x00 } else { 0x01 };
        r
    }

    fn parse_feature(r: &[u8]) -> Option<(u8, bool)> {
        if r.len() < 8 {
            return None;
        }
        for i in 5..r.len() - 2 {
            if r[i] == 0x83
                && r[i - 1] == 0x00
                && r[i - 2] == 0x02
                && (r[i - 5] == 0xA1 || r[i - 5] == 0xA2)
            {
                let charge = r[i + 1];
                let batt = r[i + 2];
                if batt <= 100 {
                    return Some((batt, charge != 0));
                }
            }
        }
        None
    }

    fn parse_old(r: &[u8]) -> Option<(u8, bool)> {
        for off in [1usize, 0] {
            if r.len() > off + 5 && r[off] == 0xA1 && r[off + 1] == 0x02 && r[off + 2] == 0x8F {
                let batt = r[off + 5];
                if batt <= 100 {
                    return Some((batt, r[off + 4] != 0));
                }
            }
        }
        None
    }

    fn read(c: &Collection, req: &[u8; 64], old: bool) -> Option<(u8, bool)> {
        let mut h = c.open()?;
        let mut frame = [0u8; FEATURE_LENGTH];
        frame[1..].copy_from_slice(req);
        if !h.set_feature(&frame, dl(250)) {
            return None;
        }
        for _ in 0..REPLY_TRIES {
            sleep(Duration::from_millis(50));
            let Some(r) = h.get_feature(0, dl(250)) else {
                continue;
            };
            let got = if old {
                parse_old(&r)
            } else {
                parse_feature(&r)
            };
            if let Some(v) = got {
                return Some(v);
            }
        }
        None
    }

    #[derive(Default)]
    struct GState {
        chosen: HashMap<u16, (u16, u16, Option<u8>)>,
        name: Option<String>,
        last: Option<(u8, bool, Instant)>,
        model_last: HashMap<u16, (u8, bool, Instant)>,
    }

    pub struct Gwolves {
        state: Mutex<GState>,
    }

    impl Gwolves {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(GState::default()),
            }
        }

        fn greyed(&self, key: String, name: String) -> Vec<Status> {
            let st = self.state.lock().unwrap();
            let last = if key == "gwolves" {
                st.last
            } else {
                let icon = key
                    .strip_prefix("gwolves:")
                    .and_then(|s| u16::from_str_radix(s, 16).ok());
                icon.and_then(|m| st.model_last.get(&m).copied())
            };
            match last {
                Some((level, charging, ts))
                    if Instant::now().saturating_duration_since(ts)
                        < Duration::from_secs(ASLEEP_KEEP) =>
                {
                    vec![status(
                        key,
                        name,
                        "mouse",
                        "gwolves",
                        Some(level),
                        Some(charging),
                        false,
                        true,
                    )]
                }
                _ => Vec::new(),
            }
        }
    }

    impl Default for Gwolves {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Gwolves {
        fn name(&self) -> &'static str {
            "gwolves"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID {
                return Vec::new();
            }
            let is_receiver = pid == RECEIVER;
            let wired = WIRED.iter().find(|(p, _)| *p == pid).map(|(_, n)| *n);
            let model = MODELS.iter().find(|(p, _, _, _, _)| *p == pid);
            if !is_receiver && wired.is_none() && model.is_none() {
                return Vec::new();
            }

            let (key, name, old, is_wired, icon) = match model {
                Some((_, n, icon, old, w)) => (
                    format!("gwolves:{:04x}", *icon),
                    (*n).to_string(),
                    *old,
                    *w,
                    Some(*icon),
                ),
                None if wired.is_some() => (
                    "gwolves".to_string(),
                    wired.unwrap().to_string(),
                    false,
                    true,
                    None,
                ),
                None => {
                    let n = self
                        .state
                        .lock()
                        .unwrap()
                        .name
                        .clone()
                        .unwrap_or_else(|| DEFAULT_NAME.to_string());
                    ("gwolves".to_string(), n, false, false, None)
                }
            };

            let cached = self.state.lock().unwrap().chosen.get(&pid).copied();
            let chosen = collections.iter().find(|c| {
                c.feature_len == FEATURE_LENGTH
                    && cached.map_or(true, |k| (c.usage_page, c.usage, c.interface) == k)
            });
            let Some(chosen) = chosen else {
                return self.greyed(key, name);
            };
            self.state
                .lock()
                .unwrap()
                .chosen
                .insert(pid, (chosen.usage_page, chosen.usage, chosen.interface));

            let req = if old { old_request(is_wired) } else { NEW_REQ };
            let Some((level, charging)) = read(chosen, &req, old) else {
                return self.greyed(key, name);
            };

            {
                let mut st = self.state.lock().unwrap();
                // 线缆给出型号名；接收器自己不知道型号。
                if wired.is_some() {
                    st.name = Some(name.clone());
                }
                if key == "gwolves" {
                    st.last = Some((level, charging, Instant::now()));
                } else if let Some(icon) = icon {
                    st.model_last
                        .insert(icon, (level, charging, Instant::now()));
                }
            }
            vec![status(
                key,
                name,
                "mouse",
                "gwolves",
                Some(level),
                Some(charging),
                true,
                true,
            )]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_and_reply() {
            assert_eq!(&NEW_REQ[..6], &[0x00, 0x00, 0x02, 0x02, 0x00, 0x83]);
            assert_eq!(old_request(true)[..4], [0x00, 0x02, 0x8F, 0x00]);
            assert_eq!(old_request(false)[..4], [0x00, 0x02, 0x8F, 0x01]);

            // 新协议：a1 00 02 02 00 83 <charging> <battery>
            let mut nr = [0u8; 65];
            nr[0] = 0xA1;
            nr[2] = 0x02;
            nr[3] = 0x02;
            nr[5] = 0x83;
            nr[6] = 0;
            nr[7] = 100;
            assert_eq!(parse_feature(&nr), Some((100, false)));
            nr[6] = 1;
            assert_eq!(parse_feature(&nr), Some((100, true)));

            // 旧协议：a1 02 8f .. <charging> <battery>
            let mut or = [0u8; 65];
            or[0] = 0xA1;
            or[1] = 0x02;
            or[2] = 0x8F;
            or[4] = 1;
            or[5] = 87;
            assert_eq!(parse_old(&or), Some((87, true)));
            assert_eq!(parse_old(&or[1..]), None);
        }
    }
}

// ===========================================================================
// Inphic In9 Pro / Attack Shark X11 / R1 / X3 Max：VID 1D57。
// 设备主动推送 report 03，字节 1 为型号码，字节 2 = 0x40 表示电量帧。
// 子命令：01 电量 / 02 充满 / 03 充电中；X6/R1 的 0x10 事件以 10% 为步长，
// 03 表示充电。只接受源码中对应接收器实测过的型号码。
// ===========================================================================
mod inphic {
    use super::*;

    const VID: u16 = 0x1D57;
    const PIDS: &[(u16, &str, &[u8])] = &[
        (0xFA65, "Inphic In9 Pro", &[0x95, 0x90, 0x93, 0x99, 0x3F]),
        (
            0xFA60,
            "Attack Shark X11 / R1 / X3 Max",
            &[0x55, 0x10, 0x4E],
        ),
    ];
    const MODEL_NAMES: &[(u16, u8, &str)] = &[(0xFA65, 0x3F, "Delux M900 Pro")];
    const REPORT_ID: u8 = 0x03;
    const CMD_BATTERY: u8 = 0x40;
    const SUB_FULL: u8 = 0x02;
    const SUB_CHARGING: u8 = 0x03;
    const FULL_IS_CHARGING_MODELS: &[u8] = &[0x4E];
    const X6_EVENT: u8 = 0x10;
    const X6_STEP: u8 = 10;
    const LEVEL_MIN: u8 = 1;
    const LEVEL_MAX: u8 = 100;
    const READ_MS: u64 = 250;
    const READ_ATTEMPTS: usize = 5;
    const PID_READ_ATTEMPTS: &[(u16, usize)] = &[(0xFA65, 12), (0xFA60, READ_ATTEMPTS)];
    const SWEEP_ATTEMPTS: usize = 2;
    const MAX_CANDIDATES: usize = 4;
    const ONLINE_FRESH: u64 = 90;
    const ASLEEP_KEEP: u64 = 300;
    const DETECT_RETRY: u64 = 60;
    const RESWEEP_AFTER: u64 = 600;
    const STATUS_USAGE: (u16, u16) = (0x000A, 0x0000);
    const VENDOR_USAGE: (u16, u16) = (0xFF00, 0x0001);
    const MOUSE_USAGE: (u16, u16) = (0x0001, 0x0002);
    const KEYBOARD_USAGE: (u16, u16) = (0x0001, 0x0006);

    /// `(level, charging)`：level=None 表示只有充电状态帧，保留上次电量。
    fn parse_frame(frame: &[u8], models: &[u8]) -> Option<(Option<u8>, bool)> {
        if frame.is_empty() {
            return None;
        }
        let f = if frame[0] == REPORT_ID {
            &frame[1..]
        } else {
            frame
        };
        if f.len() < 4 || !models.contains(&f[0]) || f[1] != CMD_BATTERY {
            return None;
        }
        if f[0] == X6_EVENT {
            let level = f[3] * X6_STEP;
            if (LEVEL_MIN..=LEVEL_MAX).contains(&level) {
                return Some((Some(level), f[2] == SUB_CHARGING));
            }
            return None;
        }
        if f[2] == SUB_CHARGING {
            return Some((None, true));
        }
        if f[2] == SUB_FULL {
            if FULL_IS_CHARGING_MODELS.contains(&f[0]) {
                // X3 Max 的插线状态，不是"充满"。
                return Some((None, true));
            }
            return Some((Some(100), true));
        }
        let level = f[3];
        if (LEVEL_MIN..=LEVEL_MAX).contains(&level) {
            return Some((Some(level), false));
        }
        None
    }

    fn rank(c: &Collection) -> u8 {
        let u = (c.usage_page, c.usage);
        if u == STATUS_USAGE {
            0
        } else if u == VENDOR_USAGE {
            1
        } else {
            2
        }
    }

    fn excluded(c: &Collection) -> bool {
        (c.usage_page, c.usage) == MOUSE_USAGE || (c.usage_page, c.usage) == KEYBOARD_USAGE
    }

    #[derive(Default)]
    struct IState {
        last: HashMap<u16, (Option<u8>, Instant)>,
        charging: HashMap<u16, bool>,
        chosen: HashMap<u16, (u16, u16, Option<u8>)>,
        swept: HashMap<u16, Instant>,
        model: HashMap<u16, u8>,
    }

    pub struct Inphic {
        state: Mutex<IState>,
    }

    impl Inphic {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(IState::default()),
            }
        }

        fn listen(
            c: &Collection,
            attempts: usize,
            models: &[u8],
            pid: u16,
            state: &Mutex<IState>,
        ) -> Option<(Option<u8>, bool)> {
            let mut h = c.open()?;
            let mut pending = None;
            for _ in 0..attempts {
                let Some(r) = h.read(dl(READ_MS)) else {
                    continue;
                };
                let got = parse_frame(&r, models);
                if got.is_none() {
                    continue;
                }
                let model = if r.first() == Some(&REPORT_ID) {
                    r.get(1).copied()
                } else {
                    r.first().copied()
                };
                if let Some(m) = model {
                    state.lock().unwrap().model.insert(pid, m);
                }
                if let Some((Some(level), charging)) = got {
                    // 充电帧之后又收到电量帧：电量新、线缆仍在。
                    return if pending.is_some() {
                        Some((Some(level), true))
                    } else {
                        Some((Some(level), charging))
                    };
                }
                pending = got;
            }
            pending
        }
    }

    impl Default for Inphic {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Inphic {
        fn name(&self) -> &'static str {
            "inphic"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID {
                return Vec::new();
            }
            let Some((_, default_name, models)) = PIDS.iter().find(|(p, _, _)| *p == pid) else {
                return Vec::new();
            };
            let models = *models;
            let now = Instant::now();

            let (chosen, last, swept) = {
                let st = self.state.lock().unwrap();
                (
                    st.chosen.get(&pid).copied(),
                    st.last.get(&pid).copied(),
                    st.swept.get(&pid).copied(),
                )
            };
            let known = chosen.is_some_and(|k| {
                collections
                    .iter()
                    .any(|c| (c.usage_page, c.usage, c.interface) == k)
            });
            let since_sweep =
                swept.map_or(u64::MAX, |t| now.saturating_duration_since(t).as_secs());
            let since_heard = last.map(|(_, t)| now.saturating_duration_since(t).as_secs());
            let sweep = if chosen.is_none() {
                since_sweep >= DETECT_RETRY
            } else if !known {
                since_sweep >= RESWEEP_AFTER
            } else {
                since_heard.is_some_and(|s| s >= RESWEEP_AFTER) && since_sweep >= RESWEEP_AFTER
            };
            if sweep {
                self.state.lock().unwrap().swept.insert(pid, now);
            }

            let mut order: Vec<&Collection> = collections.iter().filter(|c| !excluded(c)).collect();
            order.sort_by_key(|c| rank(c));
            order.dedup_by_key(|c| (c.usage_page, c.usage, c.interface));
            if let Some(k) = chosen {
                order.sort_by_key(|c| Some((c.usage_page, c.usage, c.interface)) != Some(k));
            }
            let budget: Vec<&Collection> = if sweep {
                order.into_iter().take(MAX_CANDIDATES).collect()
            } else if chosen.is_some() {
                order.into_iter().take(1).collect()
            } else {
                // 退避窗口内仍监听厂商 app 的状态集合（Rust 侧通常只有 FF00:0001 可达）。
                order
                    .into_iter()
                    .filter(|c| (c.usage_page, c.usage) == STATUS_USAGE)
                    .take(1)
                    .collect()
            };
            let pid_attempts = PID_READ_ATTEMPTS
                .iter()
                .find(|(p, _)| *p == pid)
                .map_or(READ_ATTEMPTS, |(_, n)| *n);

            let mut got = None;
            let mut got_identity = None;
            for c in budget {
                let identity = (c.usage_page, c.usage, c.interface);
                let is_chosen = chosen == Some(identity);
                let patience = (c.usage_page, c.usage) == STATUS_USAGE;
                let attempts = if is_chosen || patience {
                    pid_attempts
                } else {
                    SWEEP_ATTEMPTS
                };
                if let Some(v) = Self::listen(c, attempts, models, pid, &self.state) {
                    got = Some(v);
                    got_identity = Some(identity);
                    break;
                }
            }

            let name = {
                let st = self.state.lock().unwrap();
                let model = st.model.get(&pid).copied().unwrap_or(0);
                MODEL_NAMES
                    .iter()
                    .find(|(p, m, _)| *p == pid && *m == model)
                    .map_or(*default_name, |(_, _, n)| *n)
            };
            let key = format!("inphic:{pid:04x}");
            if let Some((level, charging)) = got {
                let mut st = self.state.lock().unwrap();
                if let Some(identity) = got_identity {
                    if chosen != Some(identity) {
                        st.chosen.insert(pid, identity);
                    }
                }
                let ts = Instant::now();
                let new_level = match (level, last) {
                    (Some(l), _) => Some(l),
                    (None, Some((old, _))) => old,
                    (None, None) => None,
                };
                st.last.insert(pid, (new_level, ts));
                st.charging.insert(pid, charging);
                return vec![status(
                    key,
                    name,
                    "mouse",
                    "inphic",
                    new_level,
                    Some(charging),
                    true,
                    true,
                )];
            }

            if let Some((level, ts)) = last {
                let age = now.saturating_duration_since(ts).as_secs();
                if age < ASLEEP_KEEP {
                    let charging = self
                        .state
                        .lock()
                        .unwrap()
                        .charging
                        .get(&pid)
                        .copied()
                        .unwrap_or(false);
                    return vec![status(
                        key,
                        name,
                        "mouse",
                        "inphic",
                        level,
                        Some(charging),
                        age < ONLINE_FRESH,
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
        fn frames_from_python_docstrings() {
            let inphic = PIDS[0].2;
            let shark = PIDS[1].2;
            // 03 95 40 01 4b -> 75%，未充电（hidapi 交回首字节 report id）。
            assert_eq!(
                parse_frame(&[0x03, 0x95, 0x40, 0x01, 0x4B], inphic),
                Some((Some(75), false))
            );
            // 03 55 40 01 1f -> X11 的 31%。
            assert_eq!(
                parse_frame(&[0x03, 0x55, 0x40, 0x01, 0x1F], shark),
                Some((Some(31), false))
            );
            // 03 4e 40 01 1e -> X3 Max 的 30%。
            assert_eq!(
                parse_frame(&[0x03, 0x4E, 0x40, 0x01, 0x1E], shark),
                Some((Some(30), false))
            );
            // X3 Max 的 02 是插线状态，不是充满：保留上次电量。
            assert_eq!(
                parse_frame(&[0x03, 0x4E, 0x40, 0x02, 0x1E], shark),
                Some((None, true))
            );
            // 03 10 40 01 0a -> X6/R1 的 100%；03 10 40 03 07 -> 70% 充电。
            assert_eq!(
                parse_frame(&[0x03, 0x10, 0x40, 0x01, 0x0A], shark),
                Some((Some(100), false))
            );
            assert_eq!(
                parse_frame(&[0x03, 0x10, 0x40, 0x03, 0x07], shark),
                Some((Some(70), true))
            );
            // 03 3F 40 01 4A -> Delux M900 Pro 的 74%。
            assert_eq!(
                parse_frame(&[0x03, 0x3F, 0x40, 0x01, 0x4A], inphic),
                Some((Some(74), false))
            );
            assert_eq!(parse_frame(&[0x03, 0x95, 0x40, 0x01, 0x00], inphic), None);
            // 被动协议：不构造任何请求。
            assert_eq!(STATUS_USAGE, (0x000A, 0x0000));
        }
    }
}

// ===========================================================================
// Rapoo VT7 / VT9 Pro / V700DIY-98：VID 24AE。全部被动监听，不写设备。
// 老 VT9 Pro（185A）：bb b0 42 20 03 <state> <level>（1 放电 / 2 充电）。
// Gen-2：input report 07，index 7 状态、index 8 电量；键盘 0xBD 帧在
// index 7 状态、index 8 电量。接收器与线缆是同一台设备的两个身份。
// ===========================================================================
mod rapoo {
    use super::*;

    const VID: u16 = 0x24AE;
    const VT7_RECEIVER: u16 = 0x1413;
    const VT9_PRO_RECEIVER: u16 = 0x185A;
    const VT7_CABLE: u16 = 0x4613;
    const V700DIY_KEYBOARD: u16 = 0x4824;
    const V700DIY_RECEIVER: u16 = 0x1924;
    // (pid, name, kind, logical key pid)
    const PIDS: &[(u16, &str, &str, u16)] = &[
        (VT7_RECEIVER, "Rapoo VT7 (Gen-2)", "mouse", VT7_RECEIVER),
        (VT7_CABLE, "Rapoo VT7 (Gen-2)", "mouse", VT7_RECEIVER),
        (
            V700DIY_RECEIVER,
            "Rapoo V700DIY-98",
            "keyboard",
            V700DIY_KEYBOARD,
        ),
        (
            V700DIY_KEYBOARD,
            "Rapoo V700DIY-98",
            "keyboard",
            V700DIY_KEYBOARD,
        ),
        (0x14A9, "Rapoo VT9 Pro V2", "mouse", 0x14A9),
        (VT9_PRO_RECEIVER, "Rapoo VT9 Pro", "mouse", VT9_PRO_RECEIVER),
    ];
    const VENDOR_PAGE: u16 = 0xFF00;
    const PREFERRED_USAGES: &[u16] = &[0x0002, 0x000E];
    const MOUSE_USAGE: (u16, u16) = (0x0001, 0x0002);
    const KEYBOARD_USAGE: (u16, u16) = (0x0001, 0x0006);
    const REPORT_ID: u8 = 0x07;
    const STATUS_INDEX: usize = 7;
    const LEVEL_INDEX: usize = 8;
    const ON_BATTERY: u8 = 0x01;
    const CHARGING: u8 = 0x02;
    const BD_MARKER: u8 = 0xBD;
    const BD_VARIANT: u8 = 0x01;
    const BD_STATE_INDEX: usize = 7;
    const BD_LEVEL_INDEX: usize = 8;
    const READ_MS: u64 = 250;
    const READ_ATTEMPTS: usize = 12;
    const SWEEP_ATTEMPTS: usize = 4;
    const VT9_PRO_ATTEMPTS: usize = 16;
    const VT9_PRO_BUFFER_LIMIT: usize = 64;
    const MAX_CANDIDATES: usize = 3;
    const QUIET_RESCAN_EVERY: u32 = 5;
    const ASLEEP_KEEP: u64 = 300;

    type Reading = (u8, bool);

    fn parse_vt9pro(r: &[u8]) -> Option<Reading> {
        if r.len() < 7 || r.len() > VT9_PRO_BUFFER_LIMIT {
            return None;
        }
        if r[7..].iter().any(|&b| b != 0) {
            return None;
        }
        if r[0] != 0xBB || r[1] != 0xB0 || r[4] != 0x03 {
            return None;
        }
        let (state, level) = (r[5], r[6]);
        if !matches!(state, 1 | 2) || level > 100 {
            return None;
        }
        Some((level, state == CHARGING))
    }

    fn parse_bd(r: &[u8]) -> Option<Reading> {
        if r.len() <= BD_LEVEL_INDEX {
            return None;
        }
        if r[0] != BD_MARKER || r[1] != BD_VARIANT {
            return None;
        }
        let state = r[BD_STATE_INDEX];
        if !matches!(state, ON_BATTERY | CHARGING) {
            return None;
        }
        let level = r[BD_LEVEL_INDEX];
        if level > 100 {
            return None;
        }
        Some((level, state == CHARGING))
    }

    fn parse_report(r: &[u8]) -> Option<Reading> {
        if r.first() == Some(&BD_MARKER) {
            return parse_bd(r);
        }
        if r.len() <= LEVEL_INDEX || r[0] != REPORT_ID {
            return None;
        }
        let status = r[STATUS_INDEX];
        if !matches!(status, ON_BATTERY | CHARGING) {
            return None;
        }
        let level = r[LEVEL_INDEX];
        if level > 100 {
            return None;
        }
        Some((level, status == CHARGING))
    }

    fn rank(c: &Collection) -> (u8, usize) {
        if c.usage_page == VENDOR_PAGE {
            if let Some(i) = PREFERRED_USAGES.iter().position(|&u| u == c.usage) {
                return (0, i);
            }
            return (1, 0);
        }
        (2, 0)
    }

    fn excluded(c: &Collection) -> bool {
        (c.usage_page, c.usage) == MOUSE_USAGE || (c.usage_page, c.usage) == KEYBOARD_USAGE
    }

    fn listen(
        c: &Collection,
        attempts: usize,
        parser: fn(&[u8]) -> Option<Reading>,
    ) -> Option<Reading> {
        let mut h = c.open()?;
        for _ in 0..attempts {
            let Some(r) = h.read(dl(READ_MS)) else {
                continue;
            };
            if let Some(v) = parser(&r) {
                return Some(v);
            }
            sleep(Duration::from_millis(20));
        }
        None
    }

    #[derive(Default)]
    struct RState {
        last: HashMap<String, (u8, bool, Instant)>,
        chosen: HashMap<u16, (u16, u16, Option<u8>)>,
        quiet: HashMap<u16, u32>,
    }

    pub struct Rapoo {
        state: Mutex<RState>,
    }

    impl Rapoo {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(RState::default()),
            }
        }

        fn info(logical: u16) -> (&'static str, &'static str) {
            PIDS.iter()
                .find(|(_, _, _, l)| *l == logical)
                .map_or(("Rapoo device", "mouse"), |(_, n, k, _)| (*n, *k))
        }
    }

    impl Default for Rapoo {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Rapoo {
        fn name(&self) -> &'static str {
            "rapoo"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID {
                return Vec::new();
            }
            let Some((_, name, kind, logical)) = PIDS.iter().find(|(p, _, _, _)| *p == pid) else {
                return Vec::new();
            };
            let (name, kind, logical) = (*name, *kind, *logical);
            let key = format!("rapoo:{logical:04x}");
            let is_vt9 = pid == VT9_PRO_RECEIVER;

            let mut order: Vec<&Collection> = collections.iter().filter(|c| !excluded(c)).collect();
            if is_vt9 {
                order.retain(|c| {
                    c.interface == Some(1)
                        && c.usage_page == VENDOR_PAGE
                        && PREFERRED_USAGES.contains(&c.usage)
                });
            }
            order.sort_by_key(|c| rank(c));
            order.dedup_by_key(|c| (c.usage_page, c.usage, c.interface));

            let (chosen, quiet) = {
                let st = self.state.lock().unwrap();
                (
                    st.chosen.get(&pid).copied(),
                    st.quiet.get(&pid).copied().unwrap_or(0),
                )
            };
            if let Some(k) = chosen {
                order.sort_by_key(|c| Some((c.usage_page, c.usage, c.interface)) != Some(k));
            }
            let deep = chosen.is_none() && (quiet == 0 || quiet % QUIET_RESCAN_EVERY == 0);
            let max_candidates = if is_vt9 { 2 } else { MAX_CANDIDATES };
            let budget: Vec<&Collection> = if deep {
                order.into_iter().take(max_candidates).collect()
            } else {
                order.into_iter().take(1).collect()
            };

            let mut got = None;
            for (n, c) in budget.iter().enumerate() {
                let attempts = if is_vt9 {
                    VT9_PRO_ATTEMPTS
                } else if n == 0 {
                    READ_ATTEMPTS
                } else {
                    SWEEP_ATTEMPTS
                };
                let parser: fn(&[u8]) -> Option<Reading> =
                    if is_vt9 { parse_vt9pro } else { parse_report };
                if let Some(v) = listen(c, attempts, parser) {
                    got = Some((v, (c.usage_page, c.usage, c.interface)));
                    break;
                }
            }

            let now = Instant::now();
            {
                let mut st = self.state.lock().unwrap();
                if let Some(((level, charging), identity)) = got {
                    st.quiet.insert(pid, 0);
                    st.chosen.insert(pid, identity);
                    st.last.insert(key.clone(), (level, charging, now));
                    return vec![status(
                        key,
                        name,
                        kind,
                        "rapoo",
                        Some(level),
                        Some(charging),
                        true,
                        true,
                    )];
                }
                st.quiet.insert(pid, quiet + 1);
            }

            // 未听到推送：保留上次电量，灰显一段时间。
            let st = self.state.lock().unwrap();
            if let Some(&(level, charging, ts)) = st.last.get(&key) {
                if now.saturating_duration_since(ts) < Duration::from_secs(ASLEEP_KEEP) {
                    let (n, k) = Self::info(logical);
                    return vec![status(
                        key,
                        n,
                        k,
                        "rapoo",
                        Some(level),
                        Some(charging),
                        false,
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
        fn frames_from_captures() {
            // #109 抓包：bb b0 42 20 03 01 64 -> 100%，未充电（state 1）。
            let bb = [0xBB, 0xB0, 0x42, 0x20, 0x03, 0x01, 0x64];
            assert_eq!(parse_vt9pro(&bb), Some((100, false)));
            let mut padded = bb.to_vec();
            padded.extend_from_slice(&[0u8; 57]);
            assert_eq!(parse_vt9pro(&padded), Some((100, false)));
            padded[7] = 1; // 非零填充不是这七字节布局
            assert_eq!(parse_vt9pro(&padded), None);
            let mut charging = bb;
            charging[5] = 2;
            assert_eq!(parse_vt9pro(&charging), Some((100, true)));

            // #193 抓包：bd 01 00 07 00 46 00 01 54 -> 84%，未充电。
            let bd = [0xBD, 0x01, 0x00, 0x07, 0x00, 0x46, 0x00, 0x01, 0x54];
            assert_eq!(parse_bd(&bd), Some((84, false)));

            // Gen-2 report 07：index 7 状态、index 8 电量。
            let mut r7 = vec![0u8; 12];
            r7[0] = REPORT_ID;
            r7[STATUS_INDEX] = CHARGING;
            r7[LEVEL_INDEX] = 75;
            assert_eq!(parse_report(&r7), Some((75, true)));
            r7[STATUS_INDEX] = 0;
            assert_eq!(parse_report(&r7), None);
        }
    }
}

// ===========================================================================
// Steam Controller：VID 28DE，puck 1304/1305（iface 2..5 每口一个槽），
// USB 直连 1302 只读 FF00:0001。全程被动监听。
// 0x43 电量帧：byte1 充电状态（1 放电 / 2 充电 / 4 充满），byte2 百分比；
// 0x79/0x46 无线状态，0x42/0x45/0x47 状态帧表示该槽控制器在线。
// ===========================================================================
mod steam {
    use super::*;

    const VID: u16 = 0x28DE;
    const PUCKS: &[u16] = &[0x1304, 0x1305];
    const USB_CONTROLLER: u16 = 0x1302;
    const NAME: &str = "Steam Controller";
    const USAGE_PAGE: u16 = 0xFF00;
    const USAGE: u16 = 0x0001;
    const GAMEPAD_USAGE: (u16, u16) = (0x0001, 0x0005);
    const SLOT_INTERFACES: &[u8] = &[2, 3, 4, 5];
    const REPORT_BATTERY: u8 = 0x43;
    const REPORTS_WIRELESS: &[u8] = &[0x79, 0x46];
    const REPORTS_STATE: &[u8] = &[0x42, 0x45, 0x47];
    const CHARGE_DISCHARGING: u8 = 1;
    const CHARGE_CHARGING: u8 = 2;
    const CHARGE_DONE: u8 = 4;
    const WINDOW_MS: u64 = 400;
    const BATTERY_WINDOW_MS: u64 = 5000;
    const MAX_REPORTS: usize = 4096;
    const MAX_SLOTS: usize = 16;
    const NOT_REPORTED: &str = "connected, battery level not reported yet";

    fn parse_charge(byte: u8) -> Option<bool> {
        if byte == CHARGE_DISCHARGING {
            Some(false)
        } else if byte == CHARGE_CHARGING || byte == CHARGE_DONE {
            Some(true)
        } else {
            None
        }
    }

    fn parse_battery(data: &[u8]) -> Option<(u8, bool)> {
        if data.len() < 15 || data[0] != REPORT_BATTERY {
            return None;
        }
        let charging = parse_charge(data[1])?;
        let level = data[2];
        if level > 100 {
            return None;
        }
        Some((level, charging))
    }

    #[derive(Default, Clone)]
    struct Res {
        battery: Option<(u8, bool)>,
        state: bool,
        wireless: Option<u8>,
        seen_battery: bool,
        reports: usize,
        tried: bool,
    }

    #[derive(Default)]
    struct SState {
        last: HashMap<String, (u8, bool)>,
    }

    pub struct Steam {
        state: Mutex<SState>,
    }

    impl Steam {
        pub fn new() -> Self {
            Self {
                state: Mutex::new(SState::default()),
            }
        }

        /// 所有槽共用一个监听窗口，轮询式非阻塞读（Rust Handle 无 set_nonblocking，
        /// 用 1 ms deadline 的现代替身）。
        fn listen(slots: &[(&Collection, String)]) -> Vec<Res> {
            let mut handles: Vec<Option<Handle>> = slots.iter().map(|(c, _)| c.open()).collect();
            let mut results = vec![Res::default(); slots.len()];
            let start = Instant::now();
            let whole = Duration::from_millis(BATTERY_WINDOW_MS);
            while Instant::now() < start + whole && handles.iter().any(|h| h.is_some()) {
                let mut received = false;
                let mut drop_idx = Vec::new();
                for i in 0..handles.len() {
                    let Some(h) = handles[i].as_mut() else {
                        continue;
                    };
                    let res = &mut results[i];
                    let limit = if res.state || res.wireless == Some(2) {
                        BATTERY_WINDOW_MS
                    } else {
                        WINDOW_MS
                    };
                    if res.reports >= MAX_REPORTS
                        || (res.tried
                            && Instant::now().saturating_duration_since(start).as_millis()
                                >= limit as u128)
                    {
                        drop_idx.push(i);
                        continue;
                    }
                    res.tried = true;
                    let Some(data) = h.read(Instant::now() + Duration::from_millis(1)) else {
                        continue;
                    };
                    received = true;
                    res.reports += 1;
                    match data[0] {
                        id if REPORTS_STATE.contains(&id) => res.state = true,
                        id if REPORTS_WIRELESS.contains(&id) => {
                            if data.len() >= 2 {
                                res.wireless = Some(data[1]);
                                if data[1] == 1 {
                                    drop_idx.push(i);
                                }
                            }
                        }
                        REPORT_BATTERY => {
                            res.seen_battery = true;
                            if let Some(v) = parse_battery(&data) {
                                res.battery = Some(v);
                                drop_idx.push(i);
                            }
                        }
                        _ => {}
                    }
                }
                for i in drop_idx {
                    handles[i] = None;
                }
                if !received {
                    sleep(Duration::from_millis(5));
                }
            }
            results
        }
    }

    impl Default for Steam {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Provider for Steam {
        fn name(&self) -> &'static str {
            "steam"
        }

        fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status> {
            let Some((vid, pid)) = parse_instance_vid_pid(instance) else {
                return Vec::new();
            };
            if vid != VID {
                return Vec::new();
            }
            if pid != USB_CONTROLLER && !PUCKS.contains(&pid) {
                return Vec::new();
            }

            // Rust 侧一个 poll 只有一个 USB instance：按 (pid, interface) 分组选集合。
            let mut grouped: HashMap<(u16, u8), Vec<&Collection>> = HashMap::new();
            for c in collections {
                let Some(iface) = c.interface else { continue };
                if pid == USB_CONTROLLER {
                    if (c.usage_page, c.usage) != (USAGE_PAGE, USAGE) {
                        continue;
                    }
                } else if !SLOT_INTERFACES.contains(&iface) {
                    continue;
                }
                grouped.entry((pid, iface)).or_default().push(c);
            }
            let mut ifaces: Vec<(u16, u8)> = grouped.keys().copied().collect();
            ifaces.sort_unstable();

            let endpoint = sha256_hex(&instance.to_ascii_lowercase());
            let mut slots: Vec<(&Collection, String)> = Vec::new();
            for (pid, iface) in ifaces {
                let mut cols = grouped[&(pid, iface)].clone();
                cols.sort_by_key(|c| (c.usage_page, c.usage));
                let pick = cols
                    .iter()
                    .find(|c| (c.usage_page, c.usage) == (USAGE_PAGE, USAGE))
                    .or_else(|| cols.iter().find(|c| c.usage_page >= 0xFF00))
                    .or_else(|| {
                        cols.iter()
                            .find(|c| (c.usage_page, c.usage) == GAMEPAD_USAGE)
                    });
                let Some(pick) = pick else { continue };
                let key = format!("steam:{pid:04x}:{iface}:path-{endpoint}");
                slots.push((*pick, key));
            }
            if slots.is_empty() {
                return Vec::new();
            }
            let mut slots = slots;
            if slots.len() > MAX_SLOTS {
                slots.truncate(MAX_SLOTS);
            }
            let results = Self::listen(&slots);
            let mut out = Vec::new();
            let mut current = HashSet::new();
            let mut st = self.state.lock().unwrap();
            for ((_, key), res) in slots.iter().zip(results) {
                current.insert(key.clone());
                if res.wireless == Some(1) {
                    st.last.remove(key);
                    continue;
                }
                let active = res.state || res.wireless == Some(2) || res.seen_battery;
                if active {
                    if let Some(v) = res.battery {
                        st.last.insert(key.clone(), v);
                    }
                    let Some(&(level, charging)) = st.last.get(key) else {
                        out.push(status(
                            key.clone(),
                            NAME,
                            "gamepad",
                            NOT_REPORTED,
                            None,
                            Some(false),
                            true,
                            true,
                        ));
                        continue;
                    };
                    out.push(status(
                        key.clone(),
                        NAME,
                        "gamepad",
                        "steam",
                        Some(level),
                        Some(charging),
                        true,
                        true,
                    ));
                    continue;
                }
                if let Some(&(level, _)) = st.last.get(key) {
                    out.push(status(
                        key.clone(),
                        NAME,
                        "gamepad",
                        "steam",
                        Some(level),
                        Some(false),
                        false,
                        true,
                    ));
                }
            }
            st.last.retain(|k, _| current.contains(k));
            out
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parse_battery_frames() {
            // input report 0x43：1+14 字节，byte1 状态，byte2 百分比。
            let mut r = vec![0u8; 15];
            r[0] = REPORT_BATTERY;
            r[1] = CHARGE_CHARGING;
            r[2] = 75;
            assert_eq!(parse_battery(&r), Some((75, true)));
            r[1] = CHARGE_DISCHARGING;
            assert_eq!(parse_battery(&r), Some((75, false)));
            r[1] = CHARGE_DONE;
            assert_eq!(parse_battery(&r), Some((75, true)));
            r[1] = 0; // reset：无意义
            assert_eq!(parse_battery(&r), None);
            r[1] = CHARGE_CHARGING;
            r[2] = 101;
            assert_eq!(parse_battery(&r), None);
            assert_eq!(parse_charge(3), None);
            // 被动协议：不构造任何请求。
            assert_eq!(
                (REPORT_BATTERY, WINDOW_MS, BATTERY_WINDOW_MS),
                (0x43, 400, 5000)
            );
        }
    }
}

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    vec![
        Box::new(fractal::Fractal),
        Box::new(fantech::Fantech),
        Box::new(endgame_we::EndgameWe),
        Box::new(plantronics::Plantronics),
        Box::new(wraith::Wraith),
        Box::new(jabra::Jabra::new()),
        Box::new(gwolves::Gwolves::new()),
        Box::new(inphic::Inphic::new()),
        Box::new(rapoo::Rapoo::new()),
        Box::new(steam::Steam::new()),
    ]
}

// ===========================================================================
// 与 Python 的差异 / 限制（静态移植固有风险，需实机确认）：
//
// - hid.rs 只放行 usage_page >= 0xFF00。inphic 的 000A:0000 状态集合、
//   steam 的 0001:0005 兜底集合在本模块不可达，只能监听厂商页集合。
// - Rust Collection 不暴露 HID path / product string：inphic、rapoo、gwolves、
//   jabra 的"记住应答集合 / 按 path 分组"改为 (usage_page, usage, interface)
//   或 USB instance 作为身份；同一接收器上多个同型集合时可能与 Python 不同。
// - rapoo 接收器(1413)与线缆(4613)共用一个图标、充电读数优先的跨实例合并在
//   Engine 的单次 poll 中没有对应机制；这里按实例分别产出并各自缓存。
// - inphic 的帧历史/诊断、steam 的 legacy key alias 与唤醒文案（ui_message）
//   在 Rust Status 中无对应字段，已省略。
// - plantronics 单次等待从 1000ms×16 缩到 250ms×16，命令序列与解析不变。
// - steam 用 1ms deadline 轮询代替 hidapi 的 set_nonblocking(True)。
// - endgame_we 的 key 由 SHA256(HID path) 改为基于 USB instance 的稳定 hash。
// - fantech / fractal / plantronics / wraith / endgame_we 的测试回复帧按源码
//   docstring 的字节布局构造（源码未附抓包 hex）；inphic / rapoo / jabra /
//   gwolves / steam / wraith 的样例帧来自源码注释里的实测报文。
// - 所有协议未做真机验证，读数正确性必须实机确认。
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_all_ten_providers() {
        let providers = all_providers();
        assert_eq!(providers.len(), 10);
        let mut names: Vec<&str> = providers.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        let mut expected = vec![
            "endgame_we",
            "fantech",
            "fractal",
            "gwolves",
            "inphic",
            "jabra",
            "plantronics",
            "rapoo",
            "steam",
            "wraith",
        ];
        expected.sort_unstable();
        assert_eq!(names, expected);
    }

    #[test]
    fn checksum_helper_matches_python() {
        // Python: (0x55 - sum([0x08, 0x04] + [0]*14)) & 0xFF == 0x49
        let mut data = [0u8; 16];
        data[0] = 0x08;
        data[1] = 0x04;
        assert_eq!(check55(&data), 0x49);
        assert_eq!(sum8(&[0x08, 0x04, 0x49]), 0x55);
    }
}
