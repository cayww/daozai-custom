//! Windows USB 接收器的 HID 报告传输。
use std::{ffi::c_void, time::Instant};
use windows::{
    core::{Owned, HRESULT, PCWSTR},
    Win32::{
        Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*, Properties::*},
        Foundation::{ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE},
        Storage::FileSystem::{
            CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            Threading::{CreateEventW, ResetEvent},
            IO::{
                CancelIoEx, DeviceIoControl, GetOverlappedResult, GetOverlappedResultEx, OVERLAPPED,
            },
        },
    },
};

// Windows SDK hidclass.h：FILE_DEVICE_KEYBOARD，功能号 100，分别为 IN/OUT_DIRECT。
const IOCTL_HID_SET_FEATURE: u32 = 0x000b_0191;
const IOCTL_HID_GET_FEATURE: u32 = 0x000b_0192;

pub struct Collection {
    pub usage_page: u16,
    pub usage: u16,
    pub interface: Option<u8>,
    pub input_len: usize,
    pub output_len: usize,
    pub feature_len: usize,
    /// HidD_GetAttributes，用于只放行特定厂商的标准 HID 集合。
    pub vid: u16,
    pub pid: u16,
    /// HidD_GetProductString，供只靠 product string 分流的协议使用。
    pub product: String,
    path: Vec<u16>,
    preparsed: Preparsed,
}

struct Preparsed(PHIDP_PREPARSED_DATA);

impl Drop for Preparsed {
    fn drop(&mut self) {
        unsafe {
            let _ = HidD_FreePreparsedData(self.0);
        }
    }
}

pub fn collections(instance: &str) -> Vec<Collection> {
    let instance = instance.strip_prefix("usb:").unwrap_or(instance);
    let instance = instance.to_ascii_uppercase();
    if !instance.starts_with("USB\\VID_") {
        return Vec::new();
    }
    let instance: Vec<_> = instance.encode_utf16().chain([0]).collect();
    let mut root = 0;
    if unsafe {
        CM_Locate_DevNodeW(
            &mut root,
            PCWSTR(instance.as_ptr()),
            CM_LOCATE_DEVNODE_NORMAL,
        )
    } != CR_SUCCESS
    {
        return Vec::new();
    }
    let mut collections = Vec::new();
    let mut pending = vec![(root, None)];
    while let Some((node, parent_interface)) = pending.pop() {
        let mut id = [0u16; MAX_DEVICE_ID_LEN as usize];
        if unsafe { CM_Get_Device_IDW(node, &mut id, 0) } != CR_SUCCESS {
            continue;
        }
        let Some(end) = id.iter().position(|&unit| unit == 0) else {
            continue;
        };
        let name = String::from_utf16_lossy(&id[..end]).to_ascii_uppercase();
        let mut identity = name.split('\\');
        let (Some(bus), Some(hardware)) = (identity.next(), identity.next()) else {
            continue;
        };
        let interface = hardware.split('&').find_map(|part| {
            let number = part.strip_prefix("MI_")?;
            if number.len() != 2 {
                return None;
            }
            u8::from_str_radix(number, 16).ok()
        });
        // 设备树中的另一枚完整 USB 设备属于另一个物理实例。
        if node != root && bus == "USB" && interface.is_none() {
            continue;
        }
        let interface = interface.or(parent_interface);
        if bus == "HID" && supported_collection(node, interface) {
            if let Some(paths) = present_paths(PCWSTR(id.as_ptr())) {
                collections.extend(
                    paths
                        .split_inclusive(|&unit| unit == 0)
                        .filter(|path| path.len() > 1 && path.last() == Some(&0))
                        .filter_map(|path| collection_for(path, interface)),
                );
            }
        }
        let mut child = 0;
        if unsafe { CM_Get_Child(&mut child, node, 0) } == CR_SUCCESS {
            loop {
                pending.push((child, interface));
                let mut sibling = 0;
                if unsafe { CM_Get_Sibling(&mut sibling, child, 0) } != CR_SUCCESS {
                    break;
                }
                child = sibling;
            }
        }
    }
    collections
}

/// 诊断用：列出该 USB 实例下所有 HID 集合的真实 usage / interface / 报告长度，
/// 不做白名单过滤，用来排查「设备插着但引擎看不到」。
pub fn debug_raw_collections(instance: &str) -> Vec<String> {
    let instance: Vec<u16> = instance.encode_utf16().chain(std::iter::once(0)).collect();
    let mut root = 0u32;
    if unsafe { CM_Locate_DevNodeW(&mut root, PCWSTR(instance.as_ptr()), CM_LOCATE_DEVNODE_NORMAL) }
        != CR_SUCCESS
    {
        return vec![format!("CM_Locate_DevNodeW failed for {instance:?}")];
    }
    let mut out = Vec::new();
    let mut pending = vec![(root, None)];
    while let Some((node, parent_interface)) = pending.pop() {
        let mut id = [0u16; MAX_DEVICE_ID_LEN as usize];
        if unsafe { CM_Get_Device_IDW(node, &mut id, 0) } != CR_SUCCESS {
            continue;
        }
        let end = id.iter().position(|&u| u == 0).unwrap_or(id.len());
        let name = String::from_utf16_lossy(&id[..end]);
        let upper = name.to_ascii_uppercase();
        let mut parts = upper.split('\\');
        let bus = parts.next().unwrap_or("");
        let hardware = parts.next().unwrap_or("");
        let interface = hardware.split('&').find_map(|part| {
            let number = part.strip_prefix("MI_")?;
            (number.len() == 2).then(|| u8::from_str_radix(number, 16).ok()).flatten()
        }).or(parent_interface);
        if node != root && bus == "USB" && interface.is_none() {
            continue;
        }
        if bus == "HID" {
            if let Some(paths) = present_paths(PCWSTR(id.as_ptr())) {
                for path in paths.split_inclusive(|&u| u == 0).filter(|p| p.len() > 1 && p.last() == Some(&0)) {
                    let Some(metadata) = open_path(path, 0) else { continue };
                    let mut prepared = PHIDP_PREPARSED_DATA::default();
                    if !unsafe { HidD_GetPreparsedData(*metadata, &mut prepared) }.as_bool() { continue; }
                    let preparsed = Preparsed(prepared);
                    let mut caps = HIDP_CAPS::default();
                    let status = unsafe { HidP_GetCaps(preparsed.0, &mut caps) };
                    let mut attributes = HIDD_ATTRIBUTES {
                        Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32,
                        ..Default::default()
                    };
                    let vid_pid = if unsafe { HidD_GetAttributes(*metadata, &mut attributes) }.as_bool() {
                        format!("{:04x}:{:04x}", attributes.VendorID, attributes.ProductID)
                    } else { "????:????".to_string() };
                    out.push(format!(
                        "iface={interface:?} usage={:04x}:{:04x} in/out/feat={}/{}/{} vidpid={vid_pid} admitted={} razer_mouse={} node={name}",
                        caps.UsagePage, caps.Usage, caps.InputReportByteLength,
                        caps.OutputReportByteLength, caps.FeatureReportByteLength,
                        status == HIDP_STATUS_SUCCESS && supported_collection(node, interface),
                        razer_mouse_feature_collection(caps.UsagePage, caps.Usage, interface,
                            attributes.VendorID, attributes.ProductID),
                    ));
                }
            }
        }
        let mut child = 0;
        if unsafe { CM_Get_Child(&mut child, node, 0) } == CR_SUCCESS {
            loop {
                pending.push((child, interface));
                let mut sibling = 0;
                if unsafe { CM_Get_Sibling(&mut sibling, child, 0) } != CR_SUCCESS { break; }
                child = sibling;
            }
        }
    }
    out
}

fn present_paths(instance: PCWSTR) -> Option<Vec<u16>> {
    for _ in 0..3 {
        let mut size = 0;
        if unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut size,
                &GUID_DEVINTERFACE_HID,
                instance,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        } != CR_SUCCESS
        {
            return None;
        }
        let mut paths = vec![0; size as usize];
        match unsafe {
            CM_Get_Device_Interface_ListW(
                &GUID_DEVINTERFACE_HID,
                instance,
                &mut paths,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        } {
            CR_SUCCESS => return Some(paths),
            CR_BUFFER_SMALL => continue,
            _ => return None,
        }
    }
    None
}

fn collection_for(path: &[u16], interface: Option<u8>) -> Option<Collection> {
    // 零访问权限仅取报告描述长度。发送请求前由适配器选定集合并打开读写句柄。
    let metadata = open_path(path, 0)?;
    let mut prepared = PHIDP_PREPARSED_DATA::default();
    if !unsafe { HidD_GetPreparsedData(*metadata, &mut prepared) }.as_bool() {
        return None;
    }
    let preparsed = Preparsed(prepared);
    let mut attributes = HIDD_ATTRIBUTES {
        Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32,
        ..Default::default()
    };
    let (vid, pid) = if unsafe { HidD_GetAttributes(*metadata, &mut attributes) }.as_bool() {
        (attributes.VendorID, attributes.ProductID)
    } else {
        (0, 0)
    };
    let mut caps = HIDP_CAPS::default();
    let status = unsafe { HidP_GetCaps(preparsed.0, &mut caps) };
    let feature_mouse = razer_mouse_feature_collection(
        caps.UsagePage, caps.Usage, interface, vid, pid,
    );
    if status != HIDP_STATUS_SUCCESS
        || (!supported_usage(caps.UsagePage, caps.Usage, interface) && !feature_mouse)
    {
        return None;
    }
    let special = special_consumer_collection(caps.UsagePage, caps.Usage, interface) || feature_mouse;
    let writable = caps.UsagePage >= 0xff00 || special;
    let mut product_buf = [0u16; 256];
    let product = if unsafe {
        HidD_GetProductString(
            *metadata,
            product_buf.as_mut_ptr().cast(),
            (product_buf.len() * 2) as u32,
        )
    }
    .as_bool()
    {
        let end = product_buf.iter().position(|&u| u == 0).unwrap_or(product_buf.len());
        String::from_utf16_lossy(&product_buf[..end])
    } else {
        String::new()
    };
    Some(Collection {
        usage_page: caps.UsagePage,
        usage: caps.Usage,
        interface,
        input_len: usize::from(caps.InputReportByteLength),
        output_len: if writable {
            usize::from(caps.OutputReportByteLength)
        } else {
            0
        },
        feature_len: if writable {
            usize::from(caps.FeatureReportByteLength)
        } else {
            0
        },
        product,
        vid,
        pid,
        path: path.to_vec(),
        preparsed,
    })
}

fn special_consumer_collection(page: u16, usage: u16, interface: Option<u8>) -> bool {
    // JBL Quantum 350：接口 3 的 Generic Desktop 集合，用 feature report 读电量。
    // MOONDROP GM01 Pro：接口 3 的 Consumer 集合，用 vendor 风格的 0x4B 命令读。
    // 其余低 usage page 来自 Python 侧实测集合：akko_s9 / eightbitdo / eweadn_ydx /
    // machenike / msi PRO / zowie EC-CW。这里只负责让集合进入引擎，协议侧仍按
    // VID/PID + interface + usage 精确匹配，不会误读其它设备。
    (page == 0x0001 && matches!(usage, 0x0000 | 0x0004 | 0x0005))
        || (page == 0x000c && interface == Some(3))
        || matches!(page, 0x000a | 0x008c | 0x00ff)
}

/// 雷蛇鼠标的标准 HID 鼠标集合（0001:0002），电量 90 字节 feature report
/// 走的就是它（OpenRazer / 社区逆向一致，Viper V3 Pro 等）。Windows 会独占
/// 该集合，普通读写句柄打不开，须用零访问权限只做 Feature IOCTL。
/// 不放行 interface 3：那里是键盘/消费集合，之前已验证不应进入引擎。
fn razer_mouse_feature_collection(
    page: u16,
    usage: u16,
    interface: Option<u8>,
    vid: u16,
    _pid: u16,
) -> bool {
    vid == 0x1532 && page == 0x0001 && usage == 0x0002 && interface != Some(3)
}

fn supported_usage(page: u16, usage: u16, interface: Option<u8>) -> bool {
    // Cloud III S 的实测输入报告位于接口 3 的这一个集合。它只允许读取。
    page >= 0xff00
        || (page, usage, interface) == (0x01c0, 0x0001, Some(3))
        || special_consumer_collection(page, usage, interface)
}

fn supported_collection(node: u32, interface: Option<u8>) -> bool {
    let mut kind = DEVPROPTYPE::default();
    let mut ids = [0u16; 512];
    let mut size = std::mem::size_of_val(&ids) as u32;
    if unsafe {
        CM_Get_DevNode_PropertyW(
            node,
            &DEVPKEY_Device_HardwareIds,
            &mut kind,
            Some(ids.as_mut_ptr().cast()),
            &mut size,
            0,
        )
    } != CR_SUCCESS
        || kind != DEVPROP_TYPE_STRING_LIST
    {
        return false;
    }
    let text = String::from_utf16_lossy(&ids[..size as usize / 2]).to_ascii_uppercase();
    // 节点级过滤必须和 collection_for 使用同一套 Razer 例外：否则 0001:0002
    // 鼠标集合在这里就被丢掉，后面的 feature report 特判根本没有机会执行。
    let (vid, pid) = hardware_vid_pid(&text);
    text.split('\0')
        .filter_map(|id| id.strip_prefix("HID_DEVICE_UP:")?.split_once("_U:"))
        .any(|(page, usage)| {
            let (Ok(page), Ok(usage)) = (
                u16::from_str_radix(page, 16),
                u16::from_str_radix(usage, 16),
            ) else {
                return false;
            };
            supported_usage(page, usage, interface)
                || razer_mouse_feature_collection(page, usage, interface, vid, pid)
        })
}

fn hardware_vid_pid(text: &str) -> (u16, u16) {
    for part in text.split(['&', '\\', '\0']) {
        if let Some(hex) = part.strip_prefix("VID_") {
            if let Ok(vid) = u16::from_str_radix(&hex[..hex.len().min(4)], 16) {
                let pid = text
                    .split(['&', '\\', '\0'])
                    .find_map(|p| p.strip_prefix("PID_"))
                    .and_then(|hex| u16::from_str_radix(&hex[..hex.len().min(4)], 16).ok())
                    .unwrap_or(0);
                return (vid, pid);
            }
        }
    }
    (0, 0)
}

fn open_path(path: &[u16], access: u32) -> Option<Owned<HANDLE>> {
    let flags = if access != 0 {
        FILE_FLAG_OVERLAPPED
    } else {
        Default::default()
    };
    unsafe {
        let file = CreateFileW(
            PCWSTR(path.as_ptr()),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            flags,
            HANDLE::default(),
        )
        .ok()?;
        Some(Owned::new(file))
    }
}

pub struct Handle {
    file: Owned<HANDLE>,
    event: Owned<HANDLE>,
    input_len: usize,
    output_len: usize,
    feature_len: usize,
}

impl Collection {
    pub fn supports_report(&self, id: u8, output: bool) -> bool {
        if output {
            self.has_report(HidP_Output, id, self.output_len)
        } else {
            self.has_report(HidP_Input, id, self.input_len)
        }
    }

    /// Raw Input 设备名与 HID 接口路径同源，供输入唤醒门控比对。
    pub fn path(&self) -> &[u16] {
        &self.path
    }

    pub fn supports_feature(&self, id: u8) -> bool {
        self.has_report(HidP_Feature, id, self.feature_len)
    }

    fn has_report(&self, kind: HIDP_REPORT_TYPE, id: u8, size: usize) -> bool {
        if size == 0 {
            return false;
        }
        let mut report = vec![0; size];
        let status = unsafe { HidP_InitializeReportForID(kind, id, self.preparsed.0, &mut report) };
        status == HIDP_STATUS_SUCCESS
    }

    pub fn open(&self) -> Option<Handle> {
        let access = if self.usage_page >= 0xff00
            || special_consumer_collection(self.usage_page, self.usage, self.interface)
        {
            (GENERIC_READ | GENERIC_WRITE).0
        } else {
            GENERIC_READ.0
        };
        // 系统独占的雷蛇鼠标集合用 dwDesiredAccess=0 打开即可收发 Feature
        // 报告（社区实测，Windows 不接受 GENERIC_READ/WRITE 时会 err=5）。
        let file = if razer_mouse_feature_collection(
            self.usage_page,
            self.usage,
            self.interface,
            self.vid,
            self.pid,
        ) {
            open_path(&self.path, 0).or_else(|| open_path(&self.path, access))?
        } else {
            open_path(&self.path, access)?
        };
        let event = unsafe { Owned::new(CreateEventW(None, true, false, PCWSTR::null()).ok()?) };
        Some(Handle {
            file,
            event,
            input_len: self.input_len,
            output_len: self.output_len,
            feature_len: self.feature_len,
        })
    }
}

enum Operation {
    Read,
    Write,
    SetFeature,
    GetFeature,
}

impl Handle {
    pub fn write(&mut self, bytes: &[u8], deadline: Instant) -> bool {
        if bytes.is_empty() || bytes.len() > self.output_len {
            return false;
        }
        let mut report = vec![0; self.output_len];
        report[..bytes.len()].copy_from_slice(bytes);
        self.transfer(&mut report, deadline, Operation::Write) == Some(self.output_len)
    }

    pub fn read(&mut self, deadline: Instant) -> Option<Vec<u8>> {
        if self.input_len == 0 {
            return None;
        }
        let mut report = vec![0; self.input_len];
        let received = self.transfer(&mut report, deadline, Operation::Read)?;
        if received == 0 || received > report.len() {
            return None;
        }
        report.truncate(received);
        Some(report)
    }

    /// HidD_GetInputReport：按 report id 取一次输入报告（控制传输）。
    /// HeadsetControl 的 prepareDevice / Python hid.get_input_report 在 Kingston
    /// 修订的每次写前都做这一步且忽略结果，用来清掉陈旧输入帧。
    pub fn prepare_input_report(&mut self, report_id: u8) {
        if self.input_len == 0 {
            return;
        }
        let mut report = vec![0u8; self.input_len];
        report[0] = report_id;
        unsafe {
            let _ = HidD_GetInputReport(*self.file, report.as_mut_ptr().cast(), report.len() as u32);
        }
    }

    pub fn set_feature(&mut self, bytes: &[u8], deadline: Instant) -> bool {
        if bytes.is_empty() || bytes.len() > self.feature_len {
            return false;
        }
        let mut report = vec![0; self.feature_len];
        report[..bytes.len()].copy_from_slice(bytes);
        self.transfer(&mut report, deadline, Operation::SetFeature)
            .is_some()
    }

    pub fn get_feature(&mut self, report_id: u8, deadline: Instant) -> Option<Vec<u8>> {
        if self.feature_len == 0 {
            return None;
        }
        let mut report = vec![0; self.feature_len];
        report[0] = report_id;
        let received = self.transfer(&mut report, deadline, Operation::GetFeature)?;
        if received == 0 || received > report.len() || report[0] != report_id {
            return None;
        }
        // 无编号的报告仍有首字节 0，Windows 返回的长度可能不含此字节。
        report.truncate(received + usize::from(report_id == 0 && received < report.len()));
        Some(report)
    }

    fn transfer(&mut self, report: &mut [u8], deadline: Instant, kind: Operation) -> Option<usize> {
        if Instant::now() >= deadline {
            return None;
        }
        unsafe { ResetEvent(*self.event) }.ok()?;
        let mut operation = OVERLAPPED {
            hEvent: *self.event,
            ..Default::default()
        };
        let file = *self.file;
        let buffer: *mut c_void = report.as_mut_ptr().cast();
        let len = report.len() as u32;
        let submitted = unsafe {
            match kind {
                Operation::Read => ReadFile(file, Some(report), None, Some(&mut operation)),
                Operation::Write => WriteFile(file, Some(report), None, Some(&mut operation)),
                Operation::SetFeature => DeviceIoControl(
                    file,
                    IOCTL_HID_SET_FEATURE,
                    Some(buffer.cast_const()),
                    len,
                    None,
                    0,
                    None,
                    Some(&mut operation),
                ),
                Operation::GetFeature => DeviceIoControl(
                    file,
                    IOCTL_HID_GET_FEATURE,
                    Some(buffer.cast_const()),
                    len,
                    Some(buffer),
                    len,
                    None,
                    Some(&mut operation),
                ),
            }
        };
        if submitted.is_err_and(|err| err.code() != HRESULT::from_win32(ERROR_IO_PENDING.0)) {
            return None;
        }
        let timeout = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(u128::from(u32::MAX - 1)) as u32;
        let mut bytes = 0;
        if unsafe { GetOverlappedResultEx(file, &operation, &mut bytes, timeout, false) }.is_ok() {
            return (Instant::now() <= deadline).then_some(bytes as usize);
        }
        // 取消只提交请求。确认 I/O 已退出后，才能释放栈上的操作结构和报告缓冲区。
        unsafe {
            let _ = CancelIoEx(file, Some(&operation));
            let _ = GetOverlappedResult(file, &operation, &mut bytes, true);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hid_collection_keeps_the_observed_nonvendor_input_exception_narrow() {
        assert!(supported_usage(0xff01, 0, Some(1)));
        assert!(supported_usage(0x01c0, 1, Some(3)));
        assert!(!supported_usage(0x01c0, 1, Some(2)));
        assert!(!supported_usage(0x01c0, 1, None));
        assert!(!supported_usage(0x01c0, 2, Some(3)));
        assert!(!supported_usage(1, 2, Some(3)));
        assert!(!supported_usage(1, 6, Some(3)));
    }

    #[test]
    fn razer_mouse_collection_is_admitted_only_for_razer() {
        assert!(razer_mouse_feature_collection(1, 2, Some(0), 0x1532, 0x00C1));
        assert!(razer_mouse_feature_collection(1, 2, None, 0x1532, 0x00C0));
        assert!(!razer_mouse_feature_collection(1, 2, Some(3), 0x1532, 0x00C1));
        assert!(!razer_mouse_feature_collection(1, 2, Some(0), 0x046D, 0x00C1));
        assert!(!razer_mouse_feature_collection(1, 6, Some(0), 0x1532, 0x00C1));
    }

    #[test]
    fn hardware_ids_yield_vid_pid_for_node_level_razer_gate() {
        assert_eq!(
            hardware_vid_pid(r"HID\VID_1532&PID_00B7&REV_0200\HID_DEVICE_UP:0001_U:0002"),
            (0x1532, 0x00B7)
        );
        assert_eq!(hardware_vid_pid("HID_DEVICE_UP:000C_U:0001"), (0, 0));
    }
}
