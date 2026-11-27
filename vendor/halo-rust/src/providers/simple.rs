//! 简单 HID 报表协议：一次写查询 + 一次读定长回复 + 字节解析。
//! 复杂协议（Logitech HID++、Corsair、Razer 等）按同一 trait 逐步迁入。
use crate::hid::Collection;
use crate::providers::{query, Provider, Status};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_millis(100);

pub struct Simple {
    name: &'static str,
    vids: &'static [u16],
    pids: &'static [u16],
    usage_page: Option<u16>,
    usage: Option<u16>,
    iface: Option<u8>,
    request: &'static [u8],
    parse: fn(&[u8]) -> Option<Status>,
}

impl Provider for Simple {
    fn name(&self) -> &'static str { self.name }

    fn poll(&self, _instance: &str, collections: &[Collection]) -> Vec<Status> {
        for collection in collections {
            if collection.usage_page < 0xff00 { continue; }
            if !self.vids.is_empty() { /* VID/PID 已由 receiver 匹配保证 */ }
            if self.usage_page.is_some_and(|u| collection.usage_page != u) { continue; }
            if self.usage.is_some_and(|u| collection.usage != u) { continue; }
            if self.iface.is_some_and(|i| collection.interface != Some(i)) { continue; }
            if let Some(frame) = query(collection, self.request, TIMEOUT) {
                if let Some(mut st) = (self.parse)(&frame) {
                    if st.key.is_empty() {
                        st.key = format!("{:04x}:{:04x}", collection.usage_page, collection.usage);
                    }
                    return vec![st];
                }
            }
        }
        Vec::new()
    }
}

fn level(byte: u8, charging: bool) -> Status {
    let mut st = Status::new("", "Device");
    st.level = Some(byte.min(100));
    st.charging = Some(charging);
    st
}

const SIMPLE: &[Simple] = &[
    Simple {
        name: "beitong", vids: &[0x20BC], pids: &[0x515C],
        usage_page: Some(0x00FF), usage: Some(0x0003), iface: Some(1),
        request: &[0x02, 0x15, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,
                   0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
                   0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
                   0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
        parse: |f| {
            if f.len() < 5 || f[..2] != [0x02, 0x15] || f[2] > 100 || ![0x50, 0x51].contains(&f[4]) {
                return None;
            }
            let mut st = level(f[2], f[4] == 0x51);
            st.name = "BEITONG KP40D".into();
            st.kind = "gamepad".into();
            st.source = "beitong".into();
            Some(st)
        },
    },
];

pub fn all_providers() -> Vec<Box<dyn Provider>> {
    SIMPLE.iter().map(|p| Box::new(Simple {
        name: p.name, vids: p.vids, pids: p.pids,
        usage_page: p.usage_page, usage: p.usage, iface: p.iface,
        request: p.request, parse: p.parse,
    }) as Box<dyn Provider>).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beitong_parser() {
        let p = &SIMPLE[0];
        let mut frame = vec![0u8; 64];
        frame[..2].copy_from_slice(&[0x02, 0x15]);
        frame[2] = 76; frame[4] = 0x51;
        let st = (p.parse)(&frame).unwrap();
        assert_eq!(st.level, Some(76));
        assert_eq!(st.charging, Some(true));
    }
}
