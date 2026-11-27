//! 独立于 USB 枚举查询电量。每个查询的回包只归入发起查询时的设备记录。
use super::compat::{builtin_receivers, BatteryProtocol, Package};
use super::{usb_battery_protocols, BatteryReading, BatterySource, Device, FRESH_FOR};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const MAX_READS: usize = 4;
const POLL_INTERVAL: Duration = Duration::from_secs(60);
const RETRY_INTERVAL: Duration = Duration::from_secs(20);

type Read = fn(&str, BatteryProtocol) -> Option<(u8, Option<bool>)>;

// 页面启停也不能让同一个物理实例同时收发两组请求。
static ACTIVE_READS: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

pub(super) fn is_reading(id: &str) -> bool {
    ACTIVE_READS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .contains(id)
}

struct ReadSlot(String);

impl ReadSlot {
    fn acquire(id: &str) -> Option<Self> {
        let mut active = ACTIVE_READS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active.len() >= MAX_READS || active.contains(id) {
            return None;
        }
        active.insert(id.to_owned());
        Some(Self(id.to_owned()))
    }
}

impl Drop for ReadSlot {
    fn drop(&mut self) {
        ACTIVE_READS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.0);
    }
}

struct Target {
    protocol: BatteryProtocol,
    next_read: Instant,
    task: Option<Receiver<Option<BatteryReading>>>,
    reading: Option<BatteryReading>,
}

impl Target {
    fn new(protocol: BatteryProtocol, now: Instant) -> Self {
        Self {
            protocol,
            next_read: now,
            task: None,
            reading: None,
        }
    }

    fn collect(&mut self, now: Instant) {
        let Some(task) = &self.task else { return };
        let result = match task.try_recv() {
            Ok(reading) => reading,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        self.task = None;
        self.next_read = now
            + if result.is_some() {
                POLL_INTERVAL
            } else {
                RETRY_INTERVAL
            };
        self.reading = result;
    }
}

pub(super) struct Poller {
    targets: BTreeMap<String, Target>,
    read: Read,
}

impl Poller {
    pub(super) fn new() -> Self {
        Self {
            targets: BTreeMap::new(),
            read: usb_battery_protocols::read,
        }
    }

    pub(super) fn sync(&mut self, devices: &[Device], pack: Option<&Package>, now: Instant) {
        let mut previous = std::mem::take(&mut self.targets);
        for device in devices {
            let Some(hardware) = device
                .id
                .strip_prefix("usb:USB\\")
                .and_then(|instance| instance.split_once('\\'))
                .map(|(hardware, _)| hardware)
            else {
                continue;
            };
            let protocol = builtin_receivers()
                .battery_protocol(hardware)
                .or_else(|| pack?.battery_protocol(hardware));
            let Some(protocol) = protocol else { continue };
            let target = previous
                .remove(&device.id)
                .filter(|old| old.protocol == protocol)
                .unwrap_or_else(|| Target::new(protocol, now));
            self.targets.insert(device.id.clone(), target);
        }
    }

    pub(super) fn target_ids(&self) -> impl Iterator<Item = &str> {
        self.targets.keys().map(String::as_str)
    }

    pub(super) fn invalidate(&mut self, id: &str) {
        if let Some(target) = self.targets.get_mut(id) {
            *target = Target::new(target.protocol, Instant::now());
        }
    }

    pub(super) fn poll(
        &mut self,
        devices: &mut [Device],
        now: Instant,
        blocked: impl Fn(&str) -> bool,
    ) {
        for device in devices {
            if let Some(target) = self.targets.get_mut(&device.id) {
                target.collect(now);
                device.reading = target
                    .reading
                    .as_ref()
                    .filter(|reading| now.saturating_duration_since(reading.observed) <= FRESH_FOR)
                    .cloned();
                device.battery = device.reading.as_ref().map(|reading| reading.percent);
                if target.task.is_none() && now >= target.next_read && !blocked(&device.id) {
                    let Some(slot) = ReadSlot::acquire(&device.id) else {
                        continue;
                    };
                    let (tx, rx) = mpsc::channel();
                    let (protocol, read) = (target.protocol, self.read);
                    let worker = thread::Builder::new()
                        .name("daozai-usb-battery".into())
                        .spawn(move || {
                            let reading = read(&slot.0, protocol)
                                .filter(|(percent, _)| *percent <= 100)
                                .map(|(percent, charging)| {
                                    BatteryReading::new(
                                        percent,
                                        BatterySource::UsbReceiver,
                                        Instant::now(),
                                        charging,
                                    )
                                });
                            let _ = tx.send(reading);
                            drop(slot);
                        });
                    if worker.is_ok() {
                        target.task = Some(rx);
                    } else {
                        target.next_read = now + RETRY_INTERVAL;
                    }
                }
            } else {
                device.battery = None;
                device.reading = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_mapping_enables_battery_and_unplug_discards_previous_read() {
        let mut devices = vec![Device {
            id: r"usb:USB\VID_3554&PID_F58A\RECEIVER".into(),
            ..Default::default()
        }];
        let package = Package::parse(
            br#"{
            "id":"daozai.wireless-receivers","api_version":2,"version":"1.1.0",
            "receivers":[{"vid":"3554","pid":"F58A","name":"VXE R1","battery_protocol":"compx"}]
        }"#,
        )
        .unwrap();
        let id = devices[0].id.clone();
        let now = Instant::now();
        let mut battery = Poller::new();
        battery.read = |_, _| None;
        battery.sync(&devices, None, now);
        assert!(battery.targets.is_empty());
        battery.sync(&devices, Some(&package), now);
        let (tx, rx) = mpsc::channel();
        battery.targets.get_mut(&id).unwrap().task = Some(rx);
        tx.send(Some(BatteryReading::new(
            0,
            BatterySource::UsbReceiver,
            now,
            Some(false),
        )))
        .unwrap();
        battery.poll(&mut devices, now, |_| false);
        assert_eq!(devices[0].battery, Some(0));
        let (late_tx, late_rx) = mpsc::channel();
        battery.targets.get_mut(&id).unwrap().task = Some(late_rx);
        battery.sync(&[], Some(&package), now);
        battery.sync(&devices, Some(&package), now);
        assert!(late_tx.send(None).is_err());
        battery.poll(&mut devices, now, |_| false);
        assert_eq!(devices[0].battery, None);
        battery.sync(&devices, None, now);
        assert!(battery.targets.is_empty());
    }

    #[test]
    fn usb_battery_slot_stays_reserved_until_the_old_query_exits() {
        let id = r"usb:USB\VID_3554&PID_F58A\SLOT";
        let previous = ReadSlot::acquire(id).unwrap();
        assert!(ReadSlot::acquire(id).is_none());
        assert!(ReadSlot::acquire(r"usb:USB\VID_3554&PID_F58A\OTHER").is_some());
        drop(previous);
        assert!(ReadSlot::acquire(id).is_some());
    }
}
