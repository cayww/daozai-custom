//! LAMZU Maya X、WLmouse Beast X Max 和 VAXEE 4K 接收器的只读 Feature 查询。
//! 帧格式及 Windows 实测见 HaloBattery v1.17.0 的协议文档（MIT）。
//! https://github.com/HeyOkay/HaloBattery/blob/v1.17.0/docs/protocols.md#lamzu-maya-x
//! https://github.com/HeyOkay/HaloBattery/issues/387
//! https://github.com/incconutwo/mouse-battery-tray/blob/c1c6059472dab366c700a7f7516e0dbffde68fd7/devices.py
//! https://github.com/HeyOkay/HaloBattery/blob/v1.17.0/docs/protocols.md#vaxee-wireless-mice-3057

use super::super::usb_hid::{self, Handle};
use std::{
    thread,
    time::{Duration, Instant},
};

const REPORT_83_QUERY: [u8; 7] = [0, 0, 0, 2, 2, 0, 0x83];
const VAXEE_REPORT: u8 = 0x0e;
const VAXEE_LEVEL: u8 = 0x0b;
const VAXEE_CHARGING: u8 = 0x10;

pub(super) fn read_report_83(instance: &str) -> Option<(u8, Option<bool>)> {
    let mut matches = usb_hid::collections(instance).into_iter().filter(|c| {
        c.interface == Some(2)
            && c.usage_page == 0xffff
            && c.usage == 0
            && c.feature_len == 65
            && c.supports_feature(0)
    });
    let control = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let mut handle = control.open()?;
    let deadline = Instant::now() + Duration::from_millis(600);
    if !handle.set_feature(&REPORT_83_QUERY, deadline) {
        return None;
    }
    for _ in 0..10 {
        if Instant::now() + Duration::from_millis(50) >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(50));
        let Some(reply) = handle.get_feature(0, deadline) else {
            return None;
        };
        if let Some(reading) = parse_report_83(&reply) {
            return Some(reading);
        }
    }
    None
}

fn parse_report_83(reply: &[u8]) -> Option<(u8, Option<bool>)> {
    // Windows 无编号报告在首字节补 0。
    let payload = reply.strip_prefix(&[0]).unwrap_or(reply);
    match payload {
        [0xa1 | 0xa2, 0, 2, 2, 0, 0x83, charge, level @ 0..=100, ..] => {
            Some((*level, Some(*charge != 0)))
        }
        _ => None,
    }
}

pub(super) fn read_vaxee(instance: &str) -> Option<(u8, Option<bool>)> {
    let mut matches = usb_hid::collections(instance).into_iter().filter(|c| {
        c.usage_page == 0xff05 && c.feature_len == 64 && c.supports_feature(VAXEE_REPORT)
    });
    let control = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let mut handle = control.open()?;
    let deadline = Instant::now() + Duration::from_millis(900);
    let ticks = match query_vaxee(&mut handle, VAXEE_LEVEL, deadline) {
        Some(ticks @ 0..=20) => ticks,
        _ => {
            let retry_at = Instant::now() + Duration::from_millis(250);
            if retry_at >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(250));
            query_vaxee(&mut handle, VAXEE_LEVEL, deadline).filter(|ticks| *ticks <= 20)?
        }
    };
    let charging = query_vaxee(&mut handle, VAXEE_CHARGING, deadline).map(|charge| charge != 0);
    Some((ticks * 5, charging))
}

fn query_vaxee(handle: &mut Handle, command: u8, deadline: Instant) -> Option<u8> {
    let wait = Duration::from_millis(100);
    if Instant::now() + wait >= deadline
        || !handle.set_feature(&[VAXEE_REPORT, 0xa5, command, 1, 1], deadline)
    {
        return None;
    }
    thread::sleep(wait);
    vaxee_value(&handle.get_feature(VAXEE_REPORT, deadline)?, command)
}

fn vaxee_value(reply: &[u8], command: u8) -> Option<u8> {
    if reply.starts_with(&[VAXEE_REPORT, 0xa5, command]) {
        reply.get(5).copied()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_83_accepts_charge_and_real_zero_but_ignores_unrelated_data() {
        // WLmouse 8K 接收器的 Windows 实机回包，HaloBattery #387。
        let beast_x_max = [0, 0xa1, 0, 2, 2, 0, 0x83, 0, 0x1a, 0, 0, 0];
        assert_eq!(parse_report_83(&beast_x_max), Some((26, Some(false))));
        let mut reply = [0, 0xa1, 0, 2, 2, 0, 0x83, 1, 71];
        assert_eq!(parse_report_83(&reply), Some((71, Some(true))));
        assert_eq!(parse_report_83(&reply[1..]), Some((71, Some(true))));
        reply[8] = 0;
        assert_eq!(parse_report_83(&reply), Some((0, Some(true))));
        reply[1] = 0xa4;
        assert_eq!(parse_report_83(&reply), None);
        reply[1] = 0xa2;
        reply[8] = 101;
        assert_eq!(parse_report_83(&reply), None);
        assert_eq!(parse_report_83(&REPORT_83_QUERY), None);
    }

    #[test]
    fn vaxee_rejects_reply_for_the_other_property_and_truncated_frames() {
        let reply = [VAXEE_REPORT, 0xa5, VAXEE_LEVEL, 1, 1, 17];
        assert_eq!(vaxee_value(&reply, VAXEE_LEVEL), Some(17));
        assert_eq!(vaxee_value(&reply, VAXEE_CHARGING), None);
        assert_eq!(vaxee_value(&reply[..5], VAXEE_LEVEL), None);
    }
}
