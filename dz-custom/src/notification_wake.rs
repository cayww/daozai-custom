//! Windows 通知的唤醒信号，事件日志只用来判断何时查询。

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows::core::{w, Error, Result, PCWSTR};
use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_TIMEOUT, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, SetEvent, WaitForSingleObject};
use windows_sys::Win32::System::EventLog::{
    EvtClose, EvtNext, EvtSubscribe, EvtSubscribeToFutureEvents, EVT_HANDLE,
};

pub(super) struct NotificationEvent(OwnedHandle);

impl NotificationEvent {
    pub(super) fn new(manual_reset: bool) -> Result<Self> {
        let event = unsafe { CreateEventW(None, manual_reset, false, PCWSTR::null())? };
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(event.0) }))
    }

    pub(super) fn handle(&self) -> HANDLE {
        HANDLE(self.0.as_raw_handle())
    }

    pub(super) fn signal(&self) {
        unsafe {
            let _ = SetEvent(self.handle());
        }
    }

    pub(super) fn is_signaled(&self) -> bool {
        unsafe { WaitForSingleObject(self.handle(), 0) == WAIT_OBJECT_0 }
    }
}

pub(super) struct NotificationLog {
    subscription: EVT_HANDLE,
    ready: NotificationEvent,
}

impl NotificationLog {
    pub(super) fn subscribe() -> Result<Self> {
        let ready = NotificationEvent::new(true)?;
        // 仅关注 Toast 送达、取消和清除，排除读取过程可能伴随的端点清理事件。
        let subscription = unsafe {
            EvtSubscribe(
                0,
                ready.handle().0,
                w!("Microsoft-Windows-PushNotification-Platform/Operational").as_ptr(),
                w!("*[System[(EventID=3052 or EventID=3054 or EventID=3055 or EventID=3056 or EventID=3153)]]").as_ptr(),
                0,
                std::ptr::null(),
                None,
                EvtSubscribeToFutureEvents,
            )
        };
        if subscription == 0 {
            return Err(Error::from_win32());
        }
        let log = Self {
            subscription,
            ready,
        };
        // EvtNext 读到队列为空后，Windows 才会为后续事件置位通知信号。
        log.drain()?;
        Ok(log)
    }

    pub(super) fn event(&self) -> HANDLE {
        self.ready.handle()
    }

    pub(super) fn drain(&self) -> Result<bool> {
        // 先复位再读取，以免覆盖读取期间到达的新事件信号。
        unsafe {
            ResetEvent(self.event())?;
        }
        let mut changed = false;
        for _ in 0..4 {
            let mut events = [0; 16];
            let mut count = 0;
            if unsafe {
                EvtNext(
                    self.subscription,
                    events.len() as u32,
                    events.as_mut_ptr(),
                    0,
                    0,
                    &mut count,
                )
            } == 0
            {
                let error = Error::from_win32();
                return if [ERROR_NO_MORE_ITEMS.to_hresult(), ERROR_TIMEOUT.to_hresult()]
                    .contains(&error.code())
                {
                    Ok(changed)
                } else {
                    Err(error)
                };
            }
            for &event in &events[..count as usize] {
                unsafe {
                    let _ = EvtClose(event);
                }
            }
            changed |= count != 0;
            if count == 0 {
                return Ok(changed);
            }
        }
        // 限制单次处理量，让停止信号能及时被处理，剩余事件留给下一次唤醒。
        self.ready.signal();
        Ok(changed)
    }
}

impl Drop for NotificationLog {
    fn drop(&mut self) {
        unsafe {
            let _ = EvtClose(self.subscription);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_wake_events_keep_stop_and_coalesce_wakes() {
        let stop = NotificationEvent::new(true).unwrap();
        assert!(!stop.is_signaled());
        stop.signal();
        assert!(stop.is_signaled());
        assert!(stop.is_signaled());

        let wake = NotificationEvent::new(false).unwrap();
        wake.signal();
        wake.signal();
        assert!(wake.is_signaled());
        assert!(!wake.is_signaled());
    }
}
