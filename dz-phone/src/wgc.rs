//! Windows Graphics Capture 实时抓屏（液态玻璃试验）。
//!
//! GDI BitBlt 读的是「桌面 DC」，`WDA_EXCLUDEFROMCAPTURE` 在那里只会把窗口区域
//! 变成黑色（或异步失效后抓到窗口自己），拿不到干净桌面。WGC 走 DWM 的独立
//! 合成路径：被排除的窗口不进入合成树，帧里露出的是它后面的真实桌面。
//!
//! 这里抓整个显示器的合成帧，只把岛体外扩矩形 `CopySubresourceRegion` 到一张
//! staging 纹理再回读，避免每帧全屏 CPU 拷贝。任何一步失败都会永久回退到
//! GDI + 干净底片，不影响岛仔其它功能。

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use windows::core::{factory, Interface};
use windows::Graphics::SizeInt32;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{IDXGIDevice};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Foundation::POINT;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

struct Wgc {
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    _item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    staging: ID3D11Texture2D,
    /// 显示器左上角在屏幕坐标系里的位置（多显示器时可能非 0）。
    mon_x: i32,
    mon_y: i32,
    sw: i32,
    sh: i32,
    stage_w: u32,
    stage_h: u32,
}

static WGC: Mutex<Option<Wgc>> = Mutex::new(None);
static FAILED: AtomicBool = AtomicBool::new(false);
static TRIED: AtomicBool = AtomicBool::new(false);
static FRAMES: AtomicU64 = AtomicU64::new(0);
/// 抓屏源代次：每次因显示器变化重建会话时 +1。GPU 层的干净底片据此判断
/// 桌面源是否已换屏，换屏后必须重抓，不能复用旧显示器的底片内容。
static EPOCH: AtomicU64 = AtomicU64::new(0);
/// 当前 WGC 会话绑定的显示器身份：左上角原点 + 分辨率。切屏时与实时查询值比对，
/// 不一致就重建会话。用 i32 的 (mon_x,mon_y,sw,sh) 作为显示器身份（多屏下原点唯一）。
static BOUND_MONITOR: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

fn log(msg: &str) {
    use std::io::Write;
    let dir = crate::model::config_dir().join("diagnostics");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("liquid-glass.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
        let _ = std::fs::write(&path, b"");
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", msg);
    }
}

/// WGC 是否已经成功产出过帧；供诊断显示。
pub fn frames() -> u64 { FRAMES.load(Ordering::Relaxed) }

/// 抓屏源代次；换屏重建会话后递增。GPU 底片用它判断是否失效。
pub fn epoch() -> u64 { EPOCH.load(Ordering::Relaxed) }

/// 查询矩形中心点所在显示器的句柄与身份 (mon_x, mon_y, sw, sh)。用矩形中心而非
/// 左上角，避免岛体落在两屏交界时左上角被判给隔壁屏。
fn monitor_for_rect(x: i32, y: i32, w: i32, h: i32) -> Option<(HMONITOR, (i32, i32, i32, i32))> {
    unsafe {
        let monitor = MonitorFromPoint(
            POINT { x: x + w / 2, y: y + h / 2 }, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut mi).as_bool() { return None; }
        let (mx, my) = (mi.rcMonitor.left, mi.rcMonitor.top);
        Some((monitor, (mx, my, mi.rcMonitor.right - mx, mi.rcMonitor.bottom - my)))
    }
}

fn staging(device: &ID3D11Device, w: u32, h: u32) -> windows::core::Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut out: Option<ID3D11Texture2D> = None;
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut out)) }?;
    out.ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x8000_4005u32 as i32)))
}

fn init(_x: i32, _y: i32, w: u32, h: u32,
        monitor: HMONITOR, ident: (i32, i32, i32, i32)) -> windows::core::Result<Wgc> {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        D3D11CreateDevice(
            None, D3D_DRIVER_TYPE_HARDWARE, windows::Win32::Foundation::HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION,
            Some(&mut device), None, Some(&mut context),
        )?;
        let device = device.ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x8000_4005u32 as i32)))?;
        let context = context.ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x8000_4005u32 as i32)))?;

        // WinRT 侧要的是 IDirect3DDevice，用 CreateDirect3D11DeviceFromDXGIDevice 从
        // 同一个 D3D11 设备包装出来，保证帧池的纹理和我们的 staging 在同一个 GPU 上。
        let dxgi: IDXGIDevice = device.cast()?;
        let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)?;
        let d3d_device: IDirect3DDevice = inspectable.cast()?;

        let (mon_x, mon_y, sw, sh) = ident;

        let interop: IGraphicsCaptureItemInterop =
            factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = interop.CreateForMonitor(monitor)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2,
            SizeInt32 { Width: sw, Height: sh },
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        session.StartCapture()?;
        // Windows 11 在显示器被 WGC 捕获时会在屏幕四周画一圈黄色提示边框。
        // 尝试关掉它；这一步可能需要系统授权，失败就保持原样（不影响抓屏）。
        match session.SetIsBorderRequired(false) {
            Ok(()) => log("wgc: 已关闭系统捕获黄框"),
            Err(e) => log(&format!("wgc: 关闭捕获黄框失败 {:?}", e)),
        }

        let stage = staging(&device, w, h)?;
        Ok(Wgc {
            _device: device, context, _item: item, pool, session,
            staging: stage, mon_x, mon_y, sw, sh, stage_w: w, stage_h: h,
        })
    }
}

/// 抓屏结果。必须区分「没有新帧」和「WGC 不可用」：
/// 前者应当复用上一帧纹理，后者才回退 GDI —— 否则会 GDI/WGC 交替导致闪烁。
pub enum Grab {
    Frame(Vec<u8>, u32, u32),
    NoFrame,
    Unavailable,
}

/// 抓屏幕物理矩形 (x,y,w,h) 的**实时 DWM 合成帧**，返回自上而下的 RGBA。
pub fn grab(x: i32, y: i32, w: i32, h: i32) -> Grab {
    if w <= 0 || h <= 0 || w > 2048 || h > 1024 { return Grab::Unavailable; }
    let (bw, bh) = (w as u32, h as u32);
    // 多显示器：岛仔被移到另一块屏（或主屏切换 / 拔线改了屏幕布局）后，抓屏矩形
    // 所属显示器会变。WGC 会话是在第一次 grab 时按当时的 (x,y) 绑定的，mon_x/mon_y/
    // sw/sh 永久固定；若继续用旧会话，`x - mon_x` 会算出错位甚至越界的坐标，轻则抓到
    // 隔壁屏内容、重则直接 Unavailable 回退 GDI + 旧屏底片 —— 表现就是玻璃横向拉伸、
    // 半块拱形残留。这里每帧按矩形中心实时查显示器，身份变了就销毁旧会话重建。
    let current = monitor_for_rect(x, y, w, h);
    let mut guard = WGC.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((monitor, ident)) = current {
        let bound = *BOUND_MONITOR.lock().unwrap_or_else(|e| e.into_inner());
        let changed = guard.as_ref()
            .map(|s| (s.mon_x, s.mon_y, s.sw, s.sh) != ident)
            .unwrap_or_else(|| bound.is_some_and(|b| b != ident));
        if changed {
            // drop 旧 Wgc：session/pool/device 的 RAII 析构会自动停止旧屏捕获并释放 COM 资源。
            *guard = None;
            EPOCH.fetch_add(1, Ordering::Relaxed);
            log(&format!("wgc: 显示器变化 {:?} → {:?}，重建会话",
                         bound.map(|b| (b.0, b.1, b.2, b.3)), ident));
            // 换屏后给新显示器一次全新机会：清掉旧屏的失败/已尝试标志。
            FAILED.store(false, Ordering::Relaxed);
            TRIED.store(false, Ordering::Relaxed);
        }
    }
    if FAILED.load(Ordering::Relaxed) { return Grab::Unavailable; }
    if guard.is_none() {
        if !TRIED.swap(true, Ordering::Relaxed) {
            let Some((monitor, ident)) = current else { return Grab::Unavailable; };
            match init(x, y, bw, bh, monitor, ident) {
                Ok(s) => {
                    log(&format!("wgc: init OK monitor=({},{} {}x{}) cap={}x{}", s.mon_x, s.mon_y, s.sw, s.sh, bw, bh));
                    *BOUND_MONITOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(ident);
                    *guard = Some(s);
                }
                Err(e) => {
                    FAILED.store(true, Ordering::Relaxed);
                    log(&format!("wgc: init FAILED {:?}，回退 GDI + 底片", e));
                    return Grab::Unavailable;
                }
            }
        } else { return Grab::Unavailable; }
    }
    let Some(s) = guard.as_mut() else { return Grab::Unavailable };
    // 展开动画里抓屏矩形会变，staging 纹理跟着重建（只在尺寸变化时）。
    if s.stage_w != bw || s.stage_h != bh {
        match staging(&s._device, bw, bh) {
            Ok(t) => { s.staging = t; s.stage_w = bw; s.stage_h = bh; }
            Err(e) => { log(&format!("wgc: staging resize FAILED {:?}", e)); return Grab::Unavailable; }
        }
    }
    let (sx, sy) = (x - s.mon_x, y - s.mon_y);
    if sx < 0 || sy < 0 || sx + w > s.sw || sy + h > s.sh { return Grab::Unavailable; }
    let src_box = D3D11_BOX {
        left: sx as u32, top: sy as u32, front: 0,
        right: (sx + w) as u32, bottom: (sy + h) as u32, back: 1,
    };
    unsafe {
        // 帧池是异步的，一次只取一帧会拿到滞后画面；连取到没有新帧为止，
        // 用最新的一帧（最多丢 4 个旧帧）。
        let mut frame = s.pool.TryGetNextFrame().ok();
        for _ in 0..4 {
            match s.pool.TryGetNextFrame() { Ok(f) => frame = Some(f), Err(_) => break }
        }
        let Some(frame) = frame else { return Grab::NoFrame };
        let surface = match frame.Surface() { Ok(v) => v, Err(_) => return Grab::NoFrame };
        let access: IDirect3DDxgiInterfaceAccess = match surface.cast() { Ok(v) => v, Err(_) => return Grab::Unavailable };
        let src_tex: ID3D11Texture2D = match access.GetInterface() { Ok(v) => v, Err(_) => return Grab::Unavailable };
        s.context.CopySubresourceRegion(&s.staging, 0, 0, 0, 0, &src_tex, 0, Some(&src_box));
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        if s.context.Map(&s.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).is_err() { return Grab::Unavailable; }
        // 帧格式本来就是 B8G8R8A8，和 wgpu 纹理的 Bgra8Unorm 一致，
        // 按行整段拷贝即可 —— 之前每帧做 4 次/像素的逐字节转换，在展开态
        // （820x291≈24 万像素）是明显开销。
        let mut rgba = vec![0u8; (bw * bh * 4) as usize];
        if !mapped.pData.is_null() {
            let ptr = mapped.pData as *const u8;
            let pitch = mapped.RowPitch as usize;
            let row_bytes = (bw * 4) as usize;
            for row in 0..bh as usize {
                let src = std::slice::from_raw_parts(ptr.add(row * pitch), row_bytes);
                rgba[row * row_bytes..(row + 1) * row_bytes].copy_from_slice(src);
            }
        }
        s.context.Unmap(&s.staging, 0);
        drop(frame);
        let n = FRAMES.fetch_add(1, Ordering::Relaxed);
        if n == 0 || n % 600 == 0 { log(&format!("wgc: frame {} cap={}x{}", n, bw, bh)); }
        if mapped.pData.is_null() { return Grab::NoFrame; }
        Grab::Frame(rgba, bw, bh)
    }
}
