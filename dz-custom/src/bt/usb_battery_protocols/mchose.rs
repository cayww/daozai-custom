//! 两种迈从接收器的只读电量查询。
//!
//! M7 Ultra 的采样及 A5 Pro Max 1K 接收器的验证记录来自 HaloBattery 1.17.0（MIT）。
//! https://github.com/HeyOkay/HaloBattery/blob/a20391e2a630eddf4ce8b1dc00a9a40e342aa2fc/docs/protocols.md#mchose-m7-ultra
//! https://github.com/HeyOkay/HaloBattery/issues/149
//! M7 短报文见 mchose-linux（MIT），A5 报文参照公开协议说明独立实现。
//! https://github.com/alexfrih/mchose-linux/blob/main/PROTOCOL.md#frame
//! https://github.com/z750sasr/mchose-a5-pro-max-web-driver/blob/2cf54c0f0b6739ea1460b80cdccda960a60b2ca5/docs/PROTOCOL.md

use super::super::usb_hid::{self, Handle};
use std::{
    thread,
    time::{Duration, Instant},
};

const M7_STATUS: u8 = 0x06;
const M7_REPORTS: [(u8, usize); 2] = [(0x11, 21), (0x12, 65)];
const M7_SETTLE: Duration = Duration::from_millis(120);
const A5_REPORT_LEN: usize = 65;
const A5_COMMAND: u8 = 0x83;
const FEATURE_TIMEOUT: Duration = Duration::from_millis(120);

pub(super) fn read_m7_ultra(instance: &str) -> Option<(u8, Option<bool>)> {
    // 连接的物理接收器 VID 可校验回复，睡眠时全零或回显请求的报文会被拒绝。
    let hardware = instance.split('\\').nth(1)?;
    let vid = hardware.split('&').next()?.strip_prefix("VID_")?;
    let vid = u16::from_str_radix(vid, 16).ok()?;
    let collections = usb_hid::collections(instance);
    let mut candidates = collections.iter().filter(|collection| {
        (
            collection.interface,
            collection.usage_page,
            collection.usage,
        ) == (Some(2), 0xff01, 1)
            && M7_REPORTS.iter().any(|&(id, size)| {
                collection.feature_len >= size && collection.supports_feature(id)
            })
    });
    let collection = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let mut handle = collection.open()?;
    let deadline = Instant::now() + Duration::from_millis(1500);
    for (id, size) in M7_REPORTS {
        if collection.feature_len < size || !collection.supports_feature(id) {
            continue;
        }
        let request = m7_request(id);
        for _ in 0..4 {
            if !handle.set_feature(&request[..size], io_deadline(deadline)) {
                break;
            }
            // 实测只重复读会持续回显请求，每次重试都要重新发送查询。
            thread::sleep(M7_SETTLE);
            if let Some(reading) = handle
                .get_feature(id, io_deadline(deadline))
                .and_then(|data| parse_m7(&data, id, vid))
            {
                return Some(reading);
            }
        }
    }
    None
}

fn m7_request(report: u8) -> [u8; 65] {
    let mut request = [0xff; 65];
    request[0] = report;
    request[1] = !M7_STATUS;
    request
}

fn parse_m7(data: &[u8], report: u8, receiver_vid: u16) -> Option<(u8, Option<bool>)> {
    let status = data.get(..13)?;
    if !status.starts_with(&[report, !M7_STATUS])
        || u16::from_le_bytes([!status[2], !status[3]]) != receiver_vid
    {
        return None;
    }
    let level = !status[11];
    let charging = match !status[12] {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    (level <= 100).then_some((level, charging))
}

pub(super) fn read_a5_pro_max(instance: &str) -> Option<(u8, Option<bool>)> {
    let collections = usb_hid::collections(instance);
    // 1K 接收器实测描述符的两个 FFFF 集合。FFA0 集合不发送任何请求。
    for (interface, usage) in [(1, 1), (2, 0)] {
        let mut candidates = collections.iter().filter(|collection| {
            (
                collection.interface,
                collection.usage_page,
                collection.usage,
            ) == (Some(interface), 0xffff, usage)
                && collection.feature_len == A5_REPORT_LEN
                && collection.supports_feature(0)
        });
        let Some(collection) = candidates.next() else {
            continue;
        };
        if candidates.next().is_some() {
            return None;
        }
        if let Some(reading) = collection
            .open()
            .and_then(|mut handle| query_a5(&mut handle))
        {
            return Some(reading);
        }
    }
    None
}

fn query_a5(handle: &mut Handle) -> Option<(u8, Option<bool>)> {
    let deadline = Instant::now() + Duration::from_millis(500);
    let request = a5_request();
    for attempt in 0..8 {
        if Instant::now() >= deadline {
            break;
        }
        if matches!(attempt, 0 | 3) {
            if !handle.set_feature(&request, io_deadline(deadline)) {
                return None;
            }
            thread::sleep(Duration::from_millis(34));
        }
        if let Some(reading) = handle
            .get_feature(0, io_deadline(deadline))
            .and_then(|data| parse_a5(&data))
        {
            return Some(reading);
        }
        thread::sleep(Duration::from_millis(30));
    }
    None
}

fn a5_request() -> [u8; A5_REPORT_LEN] {
    let mut request = [0; A5_REPORT_LEN];
    request[3] = 2;
    request[4] = 2;
    request[6] = A5_COMMAND;
    request
}

fn parse_a5(data: &[u8]) -> Option<(u8, Option<bool>)> {
    // Windows 缓冲区第一个字节为报告编号，随后才是协议的响应正文。
    let [0, 0xa1, _, _, _, 0, A5_COMMAND, charge, level, ..] = data else {
        return None;
    };
    (*level <= 100).then_some((*level, Some(*charge != 0)))
}

fn io_deadline(deadline: Instant) -> Instant {
    deadline.min(Instant::now() + FEATURE_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mchose_m7_matches_inverted_receiver_capture_and_rejects_echo() {
        // HaloBattery 文档中的 M7 Ultra 解码记录（100%，使用接收器）换成线上的逐字节取反格式。
        let decoded = [0x53, 0x52, 0x31, 0, 2, 5, 7, 0, 9, 100, 0, 100];
        let mut report = m7_request(0x11);
        for (encoded, value) in report[2..].iter_mut().zip(decoded) {
            *encoded = !value;
        }
        assert_eq!(parse_m7(&report, 0x11, 0x5253), Some((100, Some(false))));
        assert_eq!(parse_m7(&report, 0x12, 0x5253), None);
        assert_eq!(parse_m7(&report, 0x11, 0x3837), None);
        assert_eq!(parse_m7(&[0; 65], 0x11, 0x5253), None);
        assert_eq!(parse_m7(&m7_request(0x11), 0x11, 0x5253), None);
        report[11] = 0xff;
        report[12] = !1;
        assert_eq!(parse_m7(&report, 0x11, 0x5253), Some((0, Some(true))));
        report[11] = !101;
        assert_eq!(parse_m7(&report, 0x11, 0x5253), None);
    }

    #[test]
    fn mchose_a5_matches_battery_reply_and_rejects_unrelated_reports() {
        // 按 #149 公布的实测电量重建返回码。原始捕获未公开。
        let battery = [0, 0xa1, 0, 0, 0, 0, 0x83, 0, 22];
        let charging = [0, 0xa1, 0, 0, 0, 0, 0x83, 1, 25];
        assert_eq!(&a5_request()[..7], &[0, 0, 0, 2, 2, 0, 0x83]);
        assert_eq!(parse_a5(&battery), Some((22, Some(false))));
        assert_eq!(parse_a5(&charging), Some((25, Some(true))));
        assert_eq!(parse_a5(&battery[..8]), None);
        assert_eq!(parse_a5(&a5_request()), None);
        let mut wrong = charging;
        wrong[5] = 2;
        assert_eq!(parse_a5(&wrong), None);
        wrong[5] = 0;
        wrong[8] = 101;
        assert_eq!(parse_a5(&wrong), None);
    }
}
