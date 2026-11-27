//! Provider 注册表与轮询引擎。
//! 使用主程序验证过的 HID Collection/Handle 接口（按 USB instance 定位）。
use crate::hid::{self, Collection, Handle};
use crate::protocol::{Reading, ReceiverWire};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub mod mchose;
pub mod simple;
pub mod compx;
pub mod razer;
pub mod hyperx;
pub mod big_vendors;
pub mod misc;
pub mod steelseries;

/// 统一注册入口：所有协议族在这里汇总。
pub fn all_providers() -> Vec<Box<dyn Provider>> {
    let mut all = simple::all_providers();
    all.extend(compx::all_providers());
    all.extend(razer::all_providers());
    all.extend(hyperx::all_providers());
    all.extend(mchose::all_providers());
    all.extend(steelseries::all_providers());
    all.extend(misc::all_providers());
    all.extend(big_vendors::all_providers());
    all
}

/// 一次观测的设备状态。
#[derive(Clone, Debug)]
pub struct Status {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub level: Option<u8>,
    pub charging: Option<bool>,
    pub online: bool,
    pub approximate: bool,
    pub charging_known: bool,
    pub source: String,
}

impl Status {
    pub fn new(key: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            key: key.into(), name: name.into(), kind: String::new(), level: None,
            charging: None, online: true, approximate: false, charging_known: true,
            source: String::new(),
        }
    }
}

/// 协议实现接口。Provider 按接收器 USB instance 与 HID 集合匹配。
pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;
    fn poll(&self, instance: &str, collections: &[Collection]) -> Vec<Status>;
}

pub struct Engine {
    observed: HashMap<String, (Instant, (Option<u8>, Option<bool>, bool))>,
}

impl Engine {
    pub fn new() -> Self {
        Self { observed: HashMap::new() }
    }

    pub fn poll(&mut self, receivers: &[ReceiverWire], providers: &[Box<dyn Provider>]) -> Vec<Reading> {
        let now = Instant::now();
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let deadline = Instant::now() + Duration::from_secs(20);

        for receiver in receivers {
            // 从协议要求的 USB instance 定位该物理接收器的 HID 集合。
            let collections = hid::collections(&receiver.usb_instance);
            if collections.is_empty() { continue; }
            for provider in providers {
                for status in provider.poll(&receiver.usb_instance, &collections) {
                    if out.len() >= 256 { return out; }
                    let key = sha256_hex(&format!("{}\0{}\0{}", provider.name(), receiver.usb_instance, status.key));
                    if !seen.insert(key.clone()) { continue; }
                    let coarse = status.approximate;
                    let level = status.level.filter(|v| *v <= 100 && (!status.approximate || coarse));
                    let charging = if status.charging_known { status.charging } else { None };
                    let current = (level, charging, coarse);
                    let previous = self.observed.get(&key).copied();
                    let (fresh, age_ms) = if previous.is_some_and(|p| p.1 == current) {
                        let at = previous.expect("checked above").0;
                        (true, Some(now.saturating_duration_since(at).as_millis() as u64))
                    } else { (false, None) };
                    out.push(Reading {
                        key, name: status.name.chars().take(256).collect(), kind: status.kind,
                        level, charging, online: status.online, approximate: coarse, fresh, age_ms,
                        vid: receiver.vid, pid: receiver.pid,
                        usb_instance: receiver.usb_instance.clone(),
                    });
                }
            }
            let _ = deadline;
        }

        let mut next = HashMap::new();
        for d in &out {
            let current = (d.level, d.charging, d.approximate);
            if d.fresh { next.insert(d.key.clone(), (now, current)); }
            else if let Some(v) = self.observed.get(&d.key) { next.insert(d.key.clone(), *v); }
        }
        self.observed = next;
        out
    }
}

/// 稳定设备 key：64 位十六进制（FNV-1a 双轮）。key 不用于安全用途。
pub fn sha256_hex(input: &str) -> String {
    fn fnv(data: &[u8], seed: u64) -> u64 {
        let mut h = seed;
        for b in data { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
        h
    }
    let a = fnv(input.as_bytes(), 0xcbf29ce484222325);
    let b = fnv(input.as_bytes(), 0x84222325cbf29ce4);
    format!("{:016x}{:016x}{:016x}{:016x}", a, b, a.wrapping_mul(b), b.wrapping_add(a))
}

/// 便捷：打开集合并写请求、读回复。
pub fn query(collection: &Collection, request: &[u8], timeout: Duration) -> Option<Vec<u8>> {
    let mut handle: Handle = collection.open()?;
    let deadline = Instant::now() + timeout;
    if !handle.write(request, deadline) { return None; }
    handle.read(deadline)
}
