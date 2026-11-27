//! SteelSeries Arctis Nova 7 原配接收器与 Nova Elite 基站的电量查询。
//!
//! 字段和 Windows 真机回包参考 HaloBattery 1.17.0（MIT）。完整许可见 THIRD_PARTY_NOTICES.txt。
//! https://github.com/HeyOkay/HaloBattery/blob/v1.17.0/tests/test_steelseries.py
//! https://github.com/HeyOkay/HaloBattery/blob/v1.17.0/providers/steelseries_elite.py
//! https://github.com/HeyOkay/HaloBattery/issues/138

use super::super::usb_hid;
use std::time::{Duration, Instant};

const COMMAND: u8 = 0xb0;
const TIMEOUT: Duration = Duration::from_millis(600);

pub(super) fn read_nova_7(instance: &str) -> Option<(u8, Option<bool>)> {
    let report = query(instance, 0, 4)?;
    parse_nova_7(&report)
}

pub(super) fn read_nova_elite(instance: &str) -> Option<(u8, Option<bool>)> {
    let report = query(instance, 1, 16)?;
    parse_nova_elite(&report)
}

fn query(instance: &str, report_id: u8, min_reply_len: usize) -> Option<Vec<u8>> {
    let mut candidates = usb_hid::collections(instance).into_iter().filter(|c| {
        c.interface == Some(3)
            && (c.usage_page, c.usage) == (0xffc0, 0x0001)
            && c.input_len >= min_reply_len
            && c.supports_report(report_id, true)
            && (c.supports_report(report_id, false)
                || report_id == 0 && c.supports_report(COMMAND, false))
    });
    let collection = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let mut handle = collection.open()?;
    let deadline = Instant::now() + TIMEOUT;
    let request = [report_id, COMMAND];
    if !handle.write(&request, deadline) {
        return None;
    }
    while let Some(report) = handle.read(deadline) {
        let has_reply = if report_id == 0 {
            unnumbered_report(&report).first() == Some(&COMMAND)
        } else {
            report.starts_with(&request)
        };
        if has_reply {
            return Some(report);
        }
    }
    None
}

fn unnumbered_report(report: &[u8]) -> &[u8] {
    report.strip_prefix(&[0]).unwrap_or(report)
}

fn parse_nova_7(report: &[u8]) -> Option<(u8, Option<bool>)> {
    let &[COMMAND, 0x03, level, state, ..] = unnumbered_report(report) else {
        return None;
    };
    let charging = match state {
        0x01 | 0x02 => true,
        0x03 => false,
        _ => return None,
    };
    (level <= 100).then_some((level, Some(charging)))
}

fn parse_nova_elite(report: &[u8]) -> Option<(u8, Option<bool>)> {
    if report.len() < 16 || !report.starts_with(&[1, COMMAND]) {
        return None;
    }
    let level = report[6];
    // 真机关机时仍回状态 0x04 和电量 0。只有通电连接状态下的电量才有效。
    if level > 100 || !matches!(report[14], 0x02 | 0x08) {
        return None;
    }
    let charging = match report[15] {
        0x02 => Some(true),
        0x08 => Some(false),
        _ => None,
    };
    Some((level, charging))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nova_7_reads_captured_percent_and_charging_only_while_linked() {
        let on_battery = [0xb0, 0x03, 0x49, 0x03, 0x1e, 0x64, 0, 0];
        let charging_with_report_id = [0, 0xb0, 0x03, 0x45, 0x01, 0x22, 0x64, 0, 0];
        assert_eq!(parse_nova_7(&on_battery), Some((73, Some(false))));
        assert_eq!(
            parse_nova_7(&charging_with_report_id),
            Some((69, Some(true)))
        );
        assert_eq!(parse_nova_7(&[0xb0, 0x02, 0x49, 0x00]), None);
        assert_eq!(parse_nova_7(&[0xb0, 0x02, 0x49, 0x03]), None);
        assert_eq!(parse_nova_7(&[0xb0, 0x03, 0, 0x03]), Some((0, Some(false))));
        assert_eq!(parse_nova_7(&[0xb0, 0x03, 101, 0x03]), None);
        assert_eq!(parse_nova_7(&[0xb0, 0x03, 73, 0xff]), None);
        assert_eq!(parse_nova_7(&[0x01, 0x00, 0x63, 0x02]), None);
        assert_eq!(parse_nova_7(&on_battery[..3]), None);
    }

    #[test]
    fn nova_elite_reads_captured_headset_report_and_suppresses_offline_zero() {
        let charging = [
            0x01, 0xb0, 0, 0, 1, 0, 0x1e, 0x64, 8, 0, 2, 3, 5, 0, 8, 2, 3,
        ];
        let switched_off = [0x01, 0xb0, 0, 0, 1, 0, 0, 0x64, 8, 0, 2, 3, 5, 0, 4, 1, 3];
        assert_eq!(parse_nova_elite(&charging), Some((30, Some(true))));
        assert_eq!(parse_nova_elite(&switched_off), None);
        assert_eq!(parse_nova_elite(&charging[..15]), None);

        let mut report = charging;
        report[15] = 8;
        assert_eq!(parse_nova_elite(&report), Some((30, Some(false))));
        report[6] = 0;
        assert_eq!(parse_nova_elite(&report), Some((0, Some(false))));
        report[14] = 1;
        assert_eq!(parse_nova_elite(&report), None);
        report[14] = 8;
        report[6] = 101;
        assert_eq!(parse_nova_elite(&report), None);
        report[6] = 31;
        report[15] = 0xff;
        assert_eq!(parse_nova_elite(&report), Some((31, None)));
        report[1] = 0xb7;
        assert_eq!(parse_nova_elite(&report), None);
    }
}
