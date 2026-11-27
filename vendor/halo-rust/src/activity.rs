//! 键盘/鼠标活动监听（Raw Input，仅取设备路径与类型，不保留按键内容）。
//!
//! Python 侧 `atk_activity.py` / `mouse_activity.py` 用 RegisterRawInputDevices
//! + RIDEV_INPUTSINK + 只解析 RAWINPUTHEADER 实现「设备刚被真实输入过才查询电量」，
//! 避免请求本身把睡眠中的键盘/鼠标唤醒。这里按同样思路实现。
#![cfg(windows)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, RegisterRawInputDevices, HRAWINPUT, RIDEV_INPUTSINK,
    RID_HEADER, RIDI_DEVICENAME, RAWINPUTDEVICE, RAWINPUTHEADER,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassW, TranslateMessage, UnregisterClassW, HWND_MESSAGE, MSG, WNDCLASSW, WM_DESTROY,
    WM_INPUT,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Keyboard,
    Mouse,
}

#[derive(Default)]
struct Inner {
    keyboard: HashMap<String, Instant>,
    mouse: HashMap<String, Instant>,
    enabled: bool,
}

pub struct Activity {
    inner: Mutex<Inner>,
}

static ACTIVITY: OnceLock<Arc<Activity>> = OnceLock::new();

fn normalize(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    lower.split_once("#{").map_or(lower.clone(), |(head, _)| head.to_string())
}

impl Activity {
    fn observe(&self, raw_type: u32, device: &str) {
        let name = normalize(device);
        if name.is_empty() {
            return;
        }
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        match raw_type {
            0 => {
                inner.mouse.insert(name, now);
            }
            1 => {
                inner.keyboard.insert(name, now);
            }
            _ => {}
        }
    }

    fn run(self: Arc<Self>) {
        let Ok(module) = (unsafe { GetModuleHandleW(None) }) else { return };
        let instance: HINSTANCE = module.into();
        let class = w!("HaloBatteryRawInput");
        let window_class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&window_class) } == 0 {
            return;
        }
        let Ok(hwnd) = (unsafe {
            CreateWindowExW(
                Default::default(),
                class,
                class,
                Default::default(),
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                None,
                instance,
                None,
            )
        }) else {
            let _ = unsafe { UnregisterClassW(class, instance) };
            return;
        };
        let devices = [
            RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 6,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
            RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 2,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
        ];
        if unsafe { RegisterRawInputDevices(&devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32) }
            .is_err()
        {
            unsafe {
                let _ = DestroyWindow(hwnd);
                let _ = UnregisterClassW(class, instance);
            }
            return;
        }
        self.inner.lock().unwrap().enabled = true;
        let mut message = MSG::default();
        while unsafe { GetMessageW(&mut message, HWND::default(), 0, 0) }.as_bool() {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        unsafe {
            let _ = DestroyWindow(hwnd);
            let _ = UnregisterClassW(class, instance);
        }
    }
}

unsafe extern "system" fn proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_INPUT {
        let mut header = RAWINPUTHEADER::default();
        let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;
        let mut size = header_size;
        let got = unsafe {
            GetRawInputData(
                HRAWINPUT(lparam.0 as *mut c_void),
                RID_HEADER,
                Some((&mut header as *mut RAWINPUTHEADER).cast()),
                &mut size,
                header_size,
            )
        };
        if got == header_size {
            let mut length = 0u32;
            let probe = unsafe {
                GetRawInputDeviceInfoW(header.hDevice, RIDI_DEVICENAME, None, &mut length)
            };
            if probe != u32::MAX && length > 0 {
                let mut buffer = vec![0u16; length as usize + 1];
                let read = unsafe {
                    GetRawInputDeviceInfoW(
                        header.hDevice,
                        RIDI_DEVICENAME,
                        Some(buffer.as_mut_ptr().cast()),
                        &mut length,
                    )
                };
                if read != u32::MAX {
                    let name = String::from_utf16_lossy(&buffer[..length as usize]);
                    if let Some(activity) = ACTIVITY.get() {
                        activity.observe(header.dwType, &name);
                    }
                }
            }
        }
    } else if message == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// Raw Input 不可用时返回 true（不门控，退回本轮迁移前的保守行为）。
pub fn recently_active(kind: Kind, paths: &[String], window: Duration) -> bool {
    let activity = ACTIVITY.get_or_init(|| {
        let activity = Arc::new(Activity {
            inner: Mutex::new(Inner::default()),
        });
        let thread_activity = activity.clone();
        thread::spawn(move || thread_activity.run());
        // 给监听线程最多 2 秒完成窗口创建与注册地址。
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if activity.inner.lock().unwrap().enabled {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        activity
    });
    let inner = activity.inner.lock().unwrap();
    if !inner.enabled {
        return true;
    }
    let table = match kind {
        Kind::Keyboard => &inner.keyboard,
        Kind::Mouse => &inner.mouse,
    };
    let now = Instant::now();
    paths.iter().any(|path| {
        table
            .get(&normalize(path))
            .is_some_and(|at| now.saturating_duration_since(*at) <= window)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_input_paths_normalize_like_python() {
        assert_eq!(
            normalize(r"\\?\hid#vid_3837&pid_3033#{4d1e55b2-f16f-11cf-88cb-001111000030}"),
            r"\\?\hid#vid_3837&pid_3033"
        );
        assert_eq!(normalize("HID#ABC"), "hid#abc");
    }
}
