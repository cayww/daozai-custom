//! 与 Python 组件完全一致的 stdin/stdout JSON 协议（VERSION=1）。
use serde::{Deserialize, Serialize};

pub const VERSION: u16 = 1;
pub const UPSTREAM: &str = "1.18.0";
pub const MAX_LINE: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverWire {
    pub vid: u16,
    pub pid: u16,
    pub usb_instance: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    pub version: u16,
    pub id: u64,
    pub command: String,
    pub receivers: Vec<ReceiverWire>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Ready {
    pub version: u16,
    pub event: &'static str,
    pub upstream: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub level: Option<u8>,
    pub charging: Option<bool>,
    pub online: bool,
    pub approximate: bool,
    /// Python DeviceStatus.ui_message 的文案键（如 state.charging_was）。
    #[serde(default)]
    pub ui_message: String,
    pub fresh: bool,
    pub age_ms: Option<u64>,
    pub vid: u16,
    pub pid: u16,
    pub usb_instance: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Reply {
    pub version: u16,
    pub id: u64,
    pub devices: Vec<Reading>,
}

impl Poll {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION || self.command != "poll" {
            return Err("无效的轮询请求".into());
        }
        if self.receivers.len() > 256 {
            return Err("设备列表超出限制".into());
        }
        for r in &self.receivers {
            if !valid_instance(&r.usb_instance) {
                return Err("USB 接收器实例无效".into());
            }
            let (vid, pid) = parse_instance_vid_pid(&r.usb_instance)
                .ok_or_else(|| "USB 接收器实例与 VID/PID 不一致".to_string())?;
            if vid != r.vid || pid != r.pid {
                return Err("USB 接收器实例与 VID/PID 不一致".into());
            }
        }
        Ok(())
    }
}

/// 与 Python `USB_INSTANCE` 正则一致：`USB\VID_xxxx&PID_xxxx\<instance>`。
pub fn valid_instance(s: &str) -> bool {
    s.len() <= 512
        && !s.chars().any(|c| (c as u32) < 32 || (c as u32) == 127)
        && parse_instance_vid_pid(s).is_some()
}

pub fn parse_instance_vid_pid(s: &str) -> Option<(u16, u16)> {
    let upper = s.to_ascii_uppercase();
    let rest = upper.strip_prefix("USB\\VID_")?;
    if rest.len() < 4 + 5 + 1 {
        return None;
    }
    let vid = u16::from_str_radix(rest.get(0..4)?, 16).ok()?;
    if !rest[4..].starts_with("&PID_") {
        return None;
    }
    let pid = u16::from_str_radix(rest.get(9..13)?, 16).ok()?;
    if !rest[13..].starts_with('\\') || rest.len() <= 14 {
        return None;
    }
    Some((vid, pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_parsing() {
        let s = r"USB\VID_054C&PID_0CE6\7&1A2B3C&0&0";
        assert_eq!(parse_instance_vid_pid(s), Some((0x054c, 0x0ce6)));
        assert!(valid_instance(s));
        assert!(!valid_instance(""));
        assert!(!valid_instance(r"USB\VID_054C&PID_0CE6"));
    }

    #[test]
    fn wire_roundtrip() {
        let r = Ready { version: 1, event: "ready", upstream: UPSTREAM };
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, r#"{"version":1,"event":"ready","upstream":"1.18.0"}"#);
    }
}
