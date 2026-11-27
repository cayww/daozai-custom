//! DJI Mic Mini 接收器状态通道：VID_2CA3/PID_4011 的 MI_06 WinUSB bulk IN。
//!
//! 协议事实来自 `ShadowBitBasher/DJI-Mic-Control` 的 PROTOCOL.md（Unlicense）与
//! Python `dji_winusb.py`：状态帧不是 HID 报告，而是 interface 6 / EP 0x86 上的
//! 主动推送 DUML 帧；Windows 需要 WinUSB 驱动绑定，路径通过 cfgmgr32 找 MI_06
//! 子节点并读 DeviceInterfaceGUID(s) 得到。这里只实现只读通道，不做任何控制写。
#![cfg(windows)]

use windows::core::{w, GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Usb::*;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WIN32_ERROR, GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegQueryValueExW, HKEY, KEY_QUERY_VALUE, REG_MULTI_SZ, REG_SZ,
};

pub const RECEIVER_PREFIX: &str = r"USB\VID_2CA3&PID_4011\";
pub const MI06_PREFIX: &str = r"USB\VID_2CA3&PID_4011&MI_06\";
pub const EP_IN: u8 = 0x86;
pub const READ_TIMEOUT_MS: u32 = 1000;

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn text(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

fn parse_guid(units: &[u16]) -> Option<GUID> {
    let raw = text(units);
    let trimmed = raw.trim().trim_start_matches('{').trim_end_matches('}');
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if digits.len() != 32 {
        return None;
    }
    u128::from_str_radix(&digits, 16).ok().map(GUID::from_u128)
}

fn device_id(node: u32) -> Option<Vec<u16>> {
    let mut buffer = vec![0u16; 512];
    if unsafe { CM_Get_Device_IDW(node, &mut buffer, 0) } == CR_SUCCESS {
        Some(buffer)
    } else {
        None
    }
}

fn is_winusb(node: u32) -> bool {
    let mut buffer = [0u16; 32];
    let mut len = (buffer.len() * 2) as u32;
    let ok = unsafe {
        CM_Get_DevNode_Registry_PropertyW(
            node,
            CM_DRP_SERVICE,
            None,
            Some(buffer.as_mut_ptr().cast()),
            &mut len,
            0,
        )
    } == CR_SUCCESS;
    ok && text(&buffer).eq_ignore_ascii_case("winusb")
}

fn interface_guids(node: u32) -> Vec<GUID> {
    let mut key = HKEY::default();
    if unsafe {
        CM_Open_DevNode_Key(
            node,
            KEY_QUERY_VALUE.0,
            0,
            RegDisposition_OpenExisting,
            &mut key,
            CM_REGISTRY_HARDWARE,
        )
    } != CR_SUCCESS
    {
        return Vec::new();
    }
    let mut guids = Vec::new();
    for name in [w!("DeviceInterfaceGUIDs"), w!("DeviceInterfaceGUID")] {
        let mut kind = windows::Win32::System::Registry::REG_VALUE_TYPE::default();
        let mut size = 0u32;
        let ok = unsafe {
            RegQueryValueExW(key, name, None, Some(&mut kind), None, Some(&mut size))
        };
        if ok != WIN32_ERROR(0) || size == 0 || size > 8192 {
            continue;
        }
        let mut bytes = vec![0u8; size as usize];
        let ok = unsafe {
            RegQueryValueExW(
                key,
                name,
                None,
                Some(&mut kind),
                Some(bytes.as_mut_ptr()),
                Some(&mut size),
            )
        };
        if ok != WIN32_ERROR(0) || (kind != REG_SZ && kind != REG_MULTI_SZ) {
            continue;
        }
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        for part in units.split(|&u| u == 0) {
            if let Some(guid) = parse_guid(part) {
                if !guids.contains(&guid) {
                    guids.push(guid);
                }
            }
        }
    }
    unsafe { RegCloseKey(key) };
    guids
}

/// 接收器当前是否还插着。reader 线程据此在拔插后退出并清理状态。
pub fn device_present(instance: &str) -> bool {
    if !is_receiver_instance(instance) {
        return false;
    }
    let instance = wide(instance);
    let mut node = 0u32;
    unsafe {
        CM_Locate_DevNodeW(&mut node, PCWSTR(instance.as_ptr()), CM_LOCATE_DEVNODE_NORMAL)
            == CR_SUCCESS
    }
}

pub fn is_receiver_instance(instance: &str) -> bool {
    let upper = instance.to_ascii_uppercase();
    upper.starts_with(RECEIVER_PREFIX) && !upper.contains("&MI_")
}

fn is_mi06(id: &[u16]) -> bool {
    text(id).to_ascii_uppercase().starts_with(MI06_PREFIX)
}

/// 按 Python `_CfgMgr.find` 的顺序找 MI_06 的 WinUSB 设备接口路径；
/// 必须唯一，避免把 HID 或音频接口误当状态通道。
pub fn find_path(instance: &str) -> Option<Vec<u16>> {
    if !is_receiver_instance(instance) {
        return None;
    }
    let instance = wide(instance);
    let mut root = 0u32;
    if unsafe { CM_Locate_DevNodeW(&mut root, PCWSTR(instance.as_ptr()), CM_LOCATE_DEVNODE_NORMAL) }
        != CR_SUCCESS
    {
        return None;
    }
    let mut child = 0u32;
    if unsafe { CM_Get_Child(&mut child, root, 0) } != CR_SUCCESS {
        return None;
    }
    loop {
        let id = device_id(child)?;
        if is_mi06(&id) {
            if !is_winusb(child) {
                return None;
            }
            let mut paths: Vec<Vec<u16>> = Vec::new();
            let id_ptr = PCWSTR(id.as_ptr());
            for guid in interface_guids(child) {
                let mut size = 0u32;
                if unsafe {
                    CM_Get_Device_Interface_List_SizeW(
                        &mut size,
                        &guid,
                        id_ptr,
                        CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
                    )
                } != CR_SUCCESS
                    || !(1..=32768).contains(&size)
                {
                    continue;
                }
                let mut buffer = vec![0u16; size as usize];
                if unsafe {
                    CM_Get_Device_Interface_ListW(
                        &guid,
                        id_ptr,
                        &mut buffer,
                        CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
                    )
                } != CR_SUCCESS
                {
                    continue;
                }
                for path in buffer.split(|&u| u == 0).filter(|p| !p.is_empty()) {
                    let owned = path.to_vec();
                    if !paths.contains(&owned) {
                        paths.push(owned);
                    }
                }
            }
            return if paths.len() == 1 { Some(paths.remove(0)) } else { None };
        }
        if unsafe { CM_Get_Sibling(&mut child, child, 0) } != CR_SUCCESS {
            return None;
        }
    }
}

pub struct BulkIn {
    interface: WINUSB_INTERFACE_HANDLE,
    file: HANDLE,
}

impl Drop for BulkIn {
    fn drop(&mut self) {
        unsafe {
            if !self.interface.is_invalid() {
                let _ = WinUsb_Free(self.interface);
            }
            let _ = CloseHandle(self.file);
        }
    }
}

impl BulkIn {
    pub fn open(path: &[u16]) -> Option<Self> {
        let file = unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                HANDLE::default(),
            )
        }
        .ok()?;
        let mut interface = WINUSB_INTERFACE_HANDLE::default();
        if unsafe { WinUsb_Initialize(file, &mut interface) }.is_err() {
            unsafe { CloseHandle(file) };
            return None;
        }
        let reader = Self { interface, file };
        let mut alternate = 0u8;
        let mut descriptor = USB_INTERFACE_DESCRIPTOR::default();
        let valid = unsafe {
            WinUsb_GetCurrentAlternateSetting(reader.interface, &mut alternate).is_ok()
                && WinUsb_QueryInterfaceSettings(reader.interface, alternate, &mut descriptor)
                    .is_ok()
                && descriptor.bInterfaceNumber == 6
                && descriptor.bInterfaceClass == 0xFF
                && descriptor.bInterfaceSubClass == 0xF0
        };
        if !valid {
            return None;
        }
        let mut found = None;
        for index in 0..descriptor.bNumEndpoints {
            let mut info = WINUSB_PIPE_INFORMATION::default();
            if unsafe { WinUsb_QueryPipe(reader.interface, alternate, index, &mut info) }.is_ok()
                && info.PipeId == EP_IN
                && info.PipeType.0 == 2
            {
                found = Some(info);
                break;
            }
        }
        if found.is_none() {
            return None;
        }
        let timeout = READ_TIMEOUT_MS;
        let ok = unsafe {
            WinUsb_SetPipePolicy(
                reader.interface,
                EP_IN,
                PIPE_TRANSFER_TIMEOUT,
                std::mem::size_of::<u32>() as u32,
                std::ptr::addr_of!(timeout).cast(),
            )
        };
        ok.is_ok().then_some(reader)
    }

    /// 超时返回空 Vec；真正 I/O 错误返回 None（调用方重开管道）。
    pub fn read(&mut self) -> Option<Vec<u8>> {
        let mut buffer = [0u8; 4096];
        let mut transferred = 0u32;
        match unsafe {
            WinUsb_ReadPipe(
                self.interface,
                EP_IN,
                Some(&mut buffer),
                Some(&mut transferred),
                None,
            )
        } {
            Ok(()) => Some(buffer[..transferred.min(buffer.len() as u32) as usize].to_vec()),
            Err(err) if matches!(err.code().0, 121 | 1460) => Some(Vec::new()),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_parsing_covers_registry_format() {
        let raw = wide("{12345678-1234-5678-9abc-def012345678}");
        let guid = parse_guid(&raw).expect("valid GUID");
        assert_eq!(guid.to_u128(), 0x12345678_1234_5678_9abc_def012345678);
        assert!(parse_guid(&wide("not-a-guid")).is_none());
    }

    #[test]
    fn receiver_instance_gate_keeps_children_out() {
        assert!(is_receiver_instance(r"USB\VID_2CA3&PID_4011\012345"));
        assert!(!is_receiver_instance(r"USB\VID_2CA3&PID_4011&MI_06\012345"));
        assert!(!is_receiver_instance(r"USB\VID_1532&PID_00C1\012345"));
        assert!(is_mi06(&wide(r"USB\VID_2CA3&PID_4011&MI_06\012345")));
    }
}
