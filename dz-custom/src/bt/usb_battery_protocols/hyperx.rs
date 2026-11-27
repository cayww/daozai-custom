//! HyperX 接收器的只读电量查询。
//!
//! 协议和样本参考 HaloBattery 1.17.0 以及 HyperHeadset 57dfa2b，两者均为 MIT。
//! Cloud II Core：https://github.com/HeyOkay/HaloBattery/issues/155
//! Cloud III S：https://github.com/HeyOkay/HaloBattery/issues/106
//! Cloud III S 的 NGENUITY 抓包：https://github.com/LennardKittner/HyperHeadset/issues/36
//! Cloud Alpha 2：https://github.com/HeyOkay/HaloBattery/issues/26
//!
//! 完整许可声明见 THIRD_PARTY_NOTICES.txt。

use super::super::usb_hid::{self, Handle};
use std::time::{Duration, Instant};

const CLOUD_II_REPORT_LEN: usize = 62;
const REPORT_LEN: usize = 64;
const CLOUD_II_REPORT_ID: u8 = 0x66;
const CLOUD_III_S_REPORT_ID: u8 = 0x0c;
const ALPHA_2_QUERY: [u8; 2] = [0x50, 0x02];
const ALPHA_2_REPLY: [u8; 2] = [0x51, 0x02];
const CLOUD_TIMEOUT: Duration = Duration::from_secs(1);
const ALPHA_2_TIMEOUT: Duration = Duration::from_millis(1_500);

pub(super) fn read_cloud_ii_core(instance: &str) -> Option<(u8, Option<bool>)> {
    let mut handle = open_vendor_collection(
        instance,
        3,
        (0xff13, 0x0001),
        (CLOUD_II_REPORT_ID, CLOUD_II_REPORT_LEN),
        (CLOUD_II_REPORT_ID, 5),
    )?;
    let percent = query(
        &mut handle,
        &report_with_prefix::<CLOUD_II_REPORT_LEN>(&[CLOUD_II_REPORT_ID, 0x89]),
        CLOUD_TIMEOUT,
        parse_cloud_ii_percent,
    )?;
    let charging = query(
        &mut handle,
        &report_with_prefix::<CLOUD_II_REPORT_LEN>(&[CLOUD_II_REPORT_ID, 0x8a]),
        CLOUD_TIMEOUT,
        parse_cloud_ii_charge_state,
    )
    .and_then(charging_state);
    Some((percent, charging))
}

pub(super) fn read_cloud_iii_s(instance: &str) -> Option<(u8, Option<bool>)> {
    let mut handles = Vec::new();
    let mut readers = Vec::new();
    let mut writer = None;
    for collection in usb_hid::collections(instance) {
        let usage = (collection.usage_page, collection.usage);
        if collection.interface != Some(3)
            || !matches!(
                usage,
                (0xff13, 0x0001) | (0xffc0, 0x0202) | (0x01c0, 0x0001)
            )
        {
            continue;
        }
        let reads =
            collection.input_len >= 7 && collection.supports_report(CLOUD_III_S_REPORT_ID, false);
        let writes = usage != (0x01c0, 0x0001)
            && collection.output_len >= REPORT_LEN
            && collection.supports_report(CLOUD_III_S_REPORT_ID, true);
        if !reads && !writes {
            continue;
        }
        if writes && writer.replace(handles.len()).is_some() {
            return None;
        }
        let Some(handle) = collection.open() else {
            if writes {
                return None;
            }
            continue;
        };
        if reads {
            readers.push(handles.len());
        }
        handles.push(handle);
    }
    let writer = writer?;
    if readers.is_empty() {
        return None;
    }
    let percent = query_cloud_iii_s(&mut handles, writer, &readers, 0x06)
        .filter(|&percent| percent <= 100)?;
    let charging = query_cloud_iii_s(&mut handles, writer, &readers, 0x48).and_then(charging_state);
    Some((percent, charging))
}

pub(super) fn read_cloud_alpha_2(instance: &str) -> Option<(u8, Option<bool>)> {
    let mut handle = open_vendor_collection(
        instance,
        2,
        (0xff13, 0xff00),
        (ALPHA_2_QUERY[0], REPORT_LEN),
        (ALPHA_2_REPLY[0], REPORT_LEN),
    )?;
    query(
        &mut handle,
        &report_with_prefix::<REPORT_LEN>(&ALPHA_2_QUERY),
        ALPHA_2_TIMEOUT,
        parse_cloud_alpha_2,
    )
}

fn open_vendor_collection(
    instance: &str,
    interface: u8,
    usage: (u16, u16),
    (output_id, output_len): (u8, usize),
    (input_id, input_len): (u8, usize),
) -> Option<Handle> {
    let mut candidates = usb_hid::collections(instance)
        .into_iter()
        .filter(|collection| {
            collection.interface == Some(interface)
                && (collection.usage_page, collection.usage) == usage
                && collection.input_len >= input_len
                && collection.output_len >= output_len
                && collection.supports_report(input_id, false)
                && collection.supports_report(output_id, true)
        });
    let collection = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    collection.open()
}

fn report_with_prefix<const LEN: usize>(prefix: &[u8]) -> [u8; LEN] {
    let mut report = [0; LEN];
    report[..prefix.len()].copy_from_slice(prefix);
    report
}

fn query<T>(
    handle: &mut Handle,
    request: &[u8],
    timeout: Duration,
    parse: impl Fn(&[u8]) -> Option<T>,
) -> Option<T> {
    let deadline = Instant::now() + timeout;
    if !handle.write(request, deadline) {
        return None;
    }
    while Instant::now() < deadline {
        if let Some(value) = parse(&handle.read(deadline)?) {
            return Some(value);
        }
    }
    None
}

fn charging_state(state: u8) -> Option<bool> {
    match state {
        0 | 2 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn parse_cloud_ii_percent(report: &[u8]) -> Option<u8> {
    match report {
        [CLOUD_II_REPORT_ID, 0x89 | 0x0d, state_high, state_low, level, ..]
            if (*state_high != 0 || *state_low != 0) && *level <= 100 =>
        {
            Some(*level)
        }
        _ => None,
    }
}

fn parse_cloud_ii_charge_state(report: &[u8]) -> Option<u8> {
    match report {
        [CLOUD_II_REPORT_ID, 0x8a | 0x0c, state, ..] => Some(*state),
        _ => None,
    }
}

fn query_cloud_iii_s(
    handles: &mut [Handle],
    writer: usize,
    readers: &[usize],
    command: u8,
) -> Option<u8> {
    let request = report_with_prefix::<REPORT_LEN>(&[CLOUD_III_S_REPORT_ID, 2, 3, 1, 0, command]);
    let deadline = Instant::now() + CLOUD_TIMEOUT;
    if !handles[writer].write(&request, deadline) {
        return None;
    }
    // Windows 可能把同一接口的输入报告投递到另一集合。轮流读取每个已确认的集合。
    while Instant::now() < deadline {
        for &reader in readers {
            let read_deadline = if readers.len() == 1 {
                deadline
            } else {
                deadline.min(Instant::now() + Duration::from_millis(30))
            };
            match handles[reader].read(read_deadline) {
                Some(report) => {
                    if let Some(value) = parse_cloud_iii_s(&report, command) {
                        return Some(value);
                    }
                }
                None if readers.len() == 1 => return None,
                None => {}
            }
        }
        if readers.len() > 1 {
            // 拔插时多个句柄可能立即返回失败，等一小段时间再轮询。
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    None
}

fn parse_cloud_iii_s(report: &[u8], command: u8) -> Option<u8> {
    match report {
        [CLOUD_III_S_REPORT_ID, 2, 3, 1, 0, actual, state, ..] if *actual == command => {
            Some(*state)
        }
        _ => None,
    }
}

fn parse_cloud_alpha_2(report: &[u8]) -> Option<(u8, Option<bool>)> {
    let report = report.get(..REPORT_LEN)?;
    if !report.starts_with(&ALPHA_2_REPLY) || report[2] > 100 {
        return None;
    }
    Some((report[2], Some(report[6] & 0x80 != 0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_ii_core_checks_reply_kind_and_valid_state() {
        assert_eq!(
            parse_cloud_ii_percent(&[0x66, 0x89, 0x0e, 0xd7, 48]),
            Some(48)
        );
        assert_eq!(parse_cloud_ii_percent(&[0x66, 0x0d, 1, 0, 0]), Some(0));
        assert_eq!(parse_cloud_ii_percent(&[0x66, 0x89, 0, 0, 0]), None);
        assert_eq!(parse_cloud_ii_percent(&[0x66, 0x89, 1, 0, 101]), None);
        assert_eq!(parse_cloud_ii_percent(&[0x66, 0x8a, 0x0e, 0xd7, 48]), None);
        assert_eq!(
            parse_cloud_ii_charge_state(&[0x66, 0x8a, 0, 0]).and_then(charging_state),
            Some(false)
        );
        assert_eq!(
            parse_cloud_ii_charge_state(&[0x66, 0x0c, 1, 0]).and_then(charging_state),
            Some(true)
        );
        assert_eq!(
            parse_cloud_ii_charge_state(&[0x66, 0x8a, 0xff, 0]).and_then(charging_state),
            None
        );
    }

    #[test]
    fn cloud_iii_s_matches_captured_responses_from_the_input_collection() {
        // Windows 真机回包来自 USB 接口 3 的 01C0:0001 集合。
        // https://github.com/HeyOkay/HaloBattery/issues/106#issuecomment-5869901995
        let percent = [0x0c, 2, 3, 1, 0, 0x06, 0x59, 0x0f];
        let charge = [0x0c, 2, 3, 1, 0, 0x48, 0x00, 0x00];
        assert_eq!(parse_cloud_iii_s(&percent, 0x06), Some(89));
        assert_eq!(parse_cloud_iii_s(&percent, 0x48), None);
        assert_eq!(
            parse_cloud_iii_s(&charge, 0x48).and_then(charging_state),
            Some(false)
        );
        assert_eq!(parse_cloud_iii_s(&percent[..6], 0x06), None);
        let mut unrelated = percent;
        unrelated[0] = 0x0d;
        assert_eq!(parse_cloud_iii_s(&unrelated, 0x06), None);
        assert_eq!(charging_state(2), Some(false));
        assert_eq!(charging_state(0xff), None);
    }

    #[test]
    fn cloud_alpha_2_reads_only_two_byte_battery_response() {
        // 两次 NGENUITY 抓包分别与其显示的 51%、67% 一致。
        let idle = report_with_prefix::<REPORT_LEN>(&[0x51, 2, 0x33, 0, 0x1a, 0, 0, 0x0f]);
        let charged = report_with_prefix::<REPORT_LEN>(&[0x51, 2, 0x43, 0, 0x1a, 0, 0x86, 0x0f]);
        assert_eq!(parse_cloud_alpha_2(&idle), Some((51, Some(false))));
        assert_eq!(parse_cloud_alpha_2(&charged), Some((67, Some(true))));
        assert_eq!(parse_cloud_alpha_2(&idle[..63]), None);
        let wrong = report_with_prefix::<REPORT_LEN>(&[0x61, 2, 0x5a]);
        let unavailable = report_with_prefix::<REPORT_LEN>(&[0x51, 2, 0xff]);
        assert_eq!(parse_cloud_alpha_2(&wrong), None);
        assert_eq!(parse_cloud_alpha_2(&unavailable), None);
    }
}
