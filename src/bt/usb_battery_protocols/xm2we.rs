//! XM2we 原配接收器的电量与无线状态查询。
//!
//! Windows 上的集合和验证记录来自 HaloBattery 1.17.0（MIT）。
//! https://github.com/HeyOkay/HaloBattery/blob/a20391e2a630eddf4ce8b1dc00a9a40e342aa2fc/docs/protocols.md#endgame-gear-xm2we
//! 17 字节报文参考 xm2we-linux 的协议说明（MIT）。
//! https://github.com/okms/xm2we-linux/blob/4f3fddcf3dc938e7176cac7e3f28394f11241159/PROTOCOL.md#framing

use super::super::usb_hid;
use std::time::{Duration, Instant};

const REPORT_LEN: usize = 17;
const FEATURE_REPORT: u8 = 0x08;
const INPUT_REPORT: u8 = 0x09;
const QUERY_TIMEOUT: Duration = Duration::from_millis(400);

#[derive(Clone, Copy)]
enum Status {
    Link = 0x03,
    Battery = 0x04,
}

pub(super) fn read(instance: &str) -> Option<(u8, Option<bool>)> {
    let collections = usb_hid::collections(instance);
    let mut requests = collections.iter().filter(|c| {
        c.usage_page == 0xff02
            && c.usage == 0x0002
            && c.interface.is_some()
            && c.feature_len == REPORT_LEN
            && c.supports_feature(FEATURE_REPORT)
    });
    let request = requests.next()?;
    if requests.next().is_some() {
        return None;
    }
    let mut replies = collections.iter().filter(|c| {
        c.usage_page == 0xff01
            && c.usage == 0x0000
            && c.interface == request.interface
            && c.input_len >= REPORT_LEN
            && c.supports_report(INPUT_REPORT, false)
    });
    let reply = replies.next()?;
    if replies.next().is_some() {
        return None;
    }
    // 先打开同一个 USB 实例的应答集合，确保两端可用后才发请求。
    let mut receiver = reply.open()?;
    let mut sender = request.open()?;
    battery_status(|status| {
        query(
            status,
            |frame, deadline| sender.set_feature(frame, deadline),
            |deadline| receiver.read(deadline),
        )
    })
}

fn checksum(data: &[u8]) -> u8 {
    data.iter()
        .fold(0x55u8, |sum, &byte| sum.wrapping_sub(byte))
}

fn request(status: Status) -> [u8; REPORT_LEN] {
    let mut frame = [0; REPORT_LEN];
    frame[0] = FEATURE_REPORT;
    frame[1] = status as u8;
    frame[16] = checksum(&frame[..16]);
    frame
}

fn reply(data: &[u8], status: Status) -> Option<(u8, Option<u8>)> {
    let frame = data.get(..REPORT_LEN)?;
    let payload_len = frame[5];
    if !frame.starts_with(&[INPUT_REPORT, status as u8, 0, 0, 0])
        || frame[16] != checksum(&frame[..16])
        || !(1..=10).contains(&payload_len)
        || data[REPORT_LEN..].iter().any(|&byte| byte != 0)
    {
        return None;
    }
    Some((frame[6], (payload_len >= 2).then_some(frame[7])))
}

fn query(
    status: Status,
    send: impl FnOnce(&[u8], Instant) -> bool,
    mut receive: impl FnMut(Instant) -> Option<Vec<u8>>,
) -> Option<(u8, Option<u8>)> {
    let deadline = Instant::now() + QUERY_TIMEOUT;
    if !send(&request(status), deadline) {
        return None;
    }
    while Instant::now() < deadline {
        if let Some(payload) = reply(&receive(deadline)?, status) {
            return Some(payload);
        }
    }
    None
}

fn battery_status(
    mut query: impl FnMut(Status) -> Option<(u8, Option<u8>)>,
) -> Option<(u8, Option<bool>)> {
    let (level, charging) = query(Status::Battery)?;
    if level > 100 {
        return None;
    }
    let charging = match charging {
        Some(0) => Some(false),
        Some(1) => Some(true),
        _ => None,
    };
    // 充电时链路会短暂断开；未充电时确认在线，防止把接收器缓存当成新读数。
    let online = charging == Some(true) || query(Status::Link).is_some_and(|(link, _)| link == 1);
    online.then_some((level, charging))
}

#[cfg(test)]
mod tests {
    use super::*;

    // HaloBattery 的 Windows 实测记录给出了 90% 和充电变化。按公开报文格式还原。
    const ON_BATTERY: [u8; REPORT_LEN] =
        [0x09, 0x04, 0, 0, 0, 2, 90, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xec];
    const CHARGING: [u8; REPORT_LEN] =
        [0x09, 0x04, 0, 0, 0, 2, 90, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0xeb];
    const ONLINE: [u8; REPORT_LEN] = [0x09, 0x03, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x47];
    const OFFLINE: [u8; REPORT_LEN] = [0x09, 0x03, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x48];

    #[test]
    fn xm2we_queries_only_battery_and_link() {
        assert_eq!(
            request(Status::Battery),
            [8, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x49]
        );
        assert_eq!(
            request(Status::Link),
            [8, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x4a]
        );
        assert_eq!(
            battery_status(|status| match status {
                Status::Battery => reply(&ON_BATTERY, status),
                Status::Link => reply(&ONLINE, status),
            }),
            Some((90, Some(false)))
        );
    }

    #[test]
    fn xm2we_rejects_offline_cache_and_keeps_valid_charging() {
        assert_eq!(
            battery_status(|status| match status {
                Status::Battery => reply(&ON_BATTERY, status),
                Status::Link => reply(&OFFLINE, status),
            }),
            None
        );
        assert_eq!(
            battery_status(|status| match status {
                Status::Battery => reply(&CHARGING, status),
                Status::Link => panic!("已确认充电时无需再查链路"),
            }),
            Some((90, Some(true)))
        );
    }

    #[test]
    fn xm2we_checks_header_checksum_length_and_unknown_status() {
        let mut invalid = ON_BATTERY;
        invalid[16] ^= 1;
        assert_eq!(reply(&invalid, Status::Battery), None);
        assert_eq!(reply(&ON_BATTERY[..16], Status::Battery), None);
        assert_eq!(reply(&ON_BATTERY, Status::Link), None);
        for (level, flags, expected) in [(0, Some(2), Some((0, None))), (101, Some(1), None)] {
            assert_eq!(
                battery_status(|status| match status {
                    Status::Battery => Some((level, flags)),
                    Status::Link => Some((1, None)),
                }),
                expected
            );
        }
    }

    #[test]
    fn xm2we_query_ignores_other_reports_and_handles_failures() {
        assert_eq!(
            query(
                Status::Battery,
                |_, _| false,
                |_| panic!("发送失败时无需读取")
            ),
            None
        );
        assert_eq!(query(Status::Battery, |_, _| true, |_| None), None);
        let mut responses = [ONLINE, ON_BATTERY].into_iter();
        assert_eq!(
            query(
                Status::Battery,
                |bytes, _| bytes == request(Status::Battery),
                |_| responses.next().map(|data| data.to_vec())
            ),
            Some((90, Some(0)))
        );
    }
}
