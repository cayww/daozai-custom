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

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use windows::core::{factory, Interface};
use windows::Graphics::SizeInt32;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_UNKNOWN};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGIAdapter, IDXGIFactory1, CreateDXGIFactory1, IDXGIResource1, IDXGIKeyedMutex};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Foundation::{LUID, GENERIC_ALL};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Foundation::{BOOL, LPARAM, POINT, RECT};
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
    _session: GraphicsCaptureSession,
    staging: ID3D11Texture2D,
    /// spike: 可共享给 wgpu(D3D12) 的纹理 + 同步原语。只在
    /// `DAOZAI_LIQUID_GPU_SHARE=1` 时创建；默认 None 时完全走旧的 staging 读回。
    shared: Option<SharedFrame>,
    /// 显示器左上角在屏幕坐标系里的位置（多显示器时可能非 0）。
    mon_x: i32,
    mon_y: i32,
    sw: i32,
    sh: i32,
    stage_w: u32,
    stage_h: u32,
}

/// spike: producer 侧持有的共享纹理资源（D3D11 设备独占）。
/// NT handle 由纹理导出，wgpu(D3D12) 侧凭它 OpenSharedHandle 导入同一块显存。
pub struct SharedFrame {
    /// 这张纹理本身。WGC 帧 CopyResource 到它；wgpu 侧打开的是同一块底层显存。
    pub tex: ID3D11Texture2D,
    pub handle: windows::Win32::Foundation::HANDLE,
    pub mutex: IDXGIKeyedMutex,
    /// GPU 完成查询：producer 写完 CopyResource 后 spin 等它 signal，
    /// 再 ReleaseSync，保证 consumer 读到的是完整帧（社区 scrying 的实证做法）。
    pub done: ID3D11Query,
    pub w: u32,
    pub h: u32,
}

/// spike: 交给 consumer(wgpu) 的轻量句柄。只带 handle + 尺寸 + 抓屏源代次，
/// 不含 COM 对象——同步在 producer 侧已经做完，consumer 只管导入采样。
///
/// `epoch` 是换屏感知的关键：显示器拓扑变化会重建 WGC 会话与共享纹理（新的 NT
/// handle），但抓屏矩形尺寸由岛仔 UI 决定、往往不变。consumer 若只按尺寸判断
/// 是否重新导入，就会继续采样旧纹理 → 背景放大 / 静止壁纸 / 没铺满 / 发黑。
/// 带上 EPOCH 后，consumer 一发现代次变了就重新 OpenSharedHandle。
#[derive(Clone, Copy)]
pub struct SharedHandle {
    pub handle: windows::Win32::Foundation::HANDLE,
    pub w: u32,
    pub h: u32,
    pub epoch: u64,
}

// COM 接口与 HANDLE 在这个进程里只在持有 WGC 锁的单一线程上下文里使用，
// 但 windows crate 的裸 HANDLE(*mut c_void) 不自动 Send。这里显式声明：
// 共享纹理/同步对象本身是驱动管理的线程安全资源，跨线程传递句柄是合法的。
unsafe impl Send for SharedFrame {}
unsafe impl Sync for SharedFrame {}

/// spike: wgpu 设备的 D3D12 adapter LUID。WGC 的 D3D11 设备必须建在同一块 GPU 上，
/// 否则共享纹理会经系统内存中转，零拷贝收益归零。由 gpu_glass 在拿到 wgpu device 后写入。
static WGPU_LUID: Mutex<Option<LUID>> = Mutex::new(None);

// ===== spike: 零拷贝路径的主题色小范围取样 =====
// 零拷贝帧不落地 CPU，而主题明暗原本只在 capture() 的 staging 读回里算。
// 这里用一张**独立、固定尺寸**的 staging（与随抓屏变化的 s.staging 隔离），
// 只把主题采样框那一小块从干净 WGC 帧读回算平均亮度。按时间节流，失败静默。
const THEME_STAGE_W: u32 = 1024; // ≥ 520CSS × 常见 ppp 上限，固定不变
const THEME_STAGE_H: u32 = 256;
/// 主题亮度两次采样之间的最小间隔。按时间而非按帧节流：无论帧率高低
/// （静止深休眠可低至个位数 fps），主题色滞后都不超过 500ms，且静止态不空转。
const THEME_SAMPLE_INTERVAL_MS: u64 = 500;
static THEME_STAGE: Mutex<Option<ID3D11Texture2D>> = Mutex::new(None);
static THEME_RECT: Mutex<Option<(i32,i32,i32,i32)>> = Mutex::new(None);
static THEME_LUMA: AtomicU32 = AtomicU32::new(0x3f59_999a); // 0.85 默认浅色
/// 从浅到深的大幅突变待确认：连续两次采样都低才真正采纳，挡掉开机过渡暗帧。
static THEME_DARK_PENDING: AtomicBool = AtomicBool::new(false);
static THEME_LAST_SAMPLE: Mutex<Option<Instant>> = Mutex::new(None);
/// 毫秒时间戳的进程级原点（OnceLock 保证只初始化一次，跨平台安全）。
static THEME_EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
fn theme_epoch() -> Instant { *THEME_EPOCH.get_or_init(Instant::now) }

/// 纯判断：距上次采样是否已过 interval。用于把「按时间节流」逻辑单独单测，
/// 锁定「低帧率下延迟不放大、静止态不空转」。now/last 都是毫秒时间戳。
fn theme_sample_due(now_ms: u64, last_ms: Option<u64>, interval_ms: u64) -> bool {
    match last_ms {
        None => true,
        Some(last) => now_ms.saturating_sub(last) >= interval_ms,
    }
}

/// gpu_glass 每帧设置主题采样框（相对抓屏缓冲坐标）；None 表示不采样。
pub fn set_theme_rect(rect:Option<(i32,i32,i32,i32)>){if let Ok(mut g)=THEME_RECT.lock(){*g=rect;}}
/// 取最近一次采样到的背景亮度（f32 bits）。无采样时返回默认值，不影响调用方。
pub fn theme_luma()->f32{f32::from_bits(THEME_LUMA.load(Ordering::Relaxed))}
/// 用壁纸文件亮度播种主题锚值（启动时调用，开机第一帧就不依赖抓屏）。
pub fn seed_theme_luma(v: f32){ THEME_LUMA.store(v.clamp(0.0,1.0).to_bits(),Ordering::Relaxed); }

/// 纯边界计算：把主题采样框 rect（相对抓屏缓冲坐标）换算成源 box（相对显示器帧）
/// 与读回尺寸 (left,top,right,bottom,rw,rh)。任何退化/越界/溢出都返回 None，
/// 调用方据此静默跳过。抽成不依赖 COM 的纯函数，专门单测「尺寸剧烈变化时绝不越界」。
/// - rw/rh 先夹到固定 staging 上限（1024×256），杜绝 from_raw_parts 越界；
/// - 源 box 必须完整落在显示器帧 [0,sw)×[0,sh) 内，否则 CopySubresourceRegion 会失败；
/// - 全程用 i64 中间运算，极端 i32 坐标也不会溢出 panic（debug 构建尤其重要）。
fn theme_sample_box(
    rect: Option<(i32,i32,i32,i32)>, sx: i32, sy: i32, sw: i32, sh: i32,
) -> Option<(i32,i32,i32,i32,u32,u32)> {
    let (rx0,ry0,rx1,ry1)=rect?;
    if rx1<=rx0 || ry1<=ry0 {return None;}
    let rw=((rx1 as i64-rx0 as i64).min(THEME_STAGE_W as i64) as u32).max(0);
    let rh=((ry1 as i64-ry0 as i64).min(THEME_STAGE_H as i64) as u32).max(0);
    if rw==0 || rh==0 {return None;}
    if sw<=0 || sh<=0 {return None;}
    let left=sx as i64+rx0 as i64;
    let top=sy as i64+ry0 as i64;
    let right=left+rw as i64;
    let bottom=top+rh as i64;
    if left<0 || top<0 || right>sw as i64 || bottom>sh as i64 {return None;}
    // 能走到这里说明 right<=sw<=i32::MAX 且 left>=0，安全收窄回 i32。
    Some((left as i32,top as i32,right as i32,bottom as i32,rw,rh))
}

/// 主题亮度要不要落盘：大幅「浅→深」突变要求连续两次采样都低才采纳。
/// 开机/切屏早期抓到过渡暗帧，若立即采纳会把液态玻璃判成深色（黑底）且不易恢复；
/// 真·深色壁纸是持续的，第二次采样仍低就会采纳。平滑变化/变亮立即采纳。
/// `anchor` 是壁纸文件亮度（Some 时）：抓屏值与它差 >0.25 视为开机过渡帧/污染帧，
/// 直接拒绝——这是比"连续两次"更直接的判据，因为壁纸文件是稳定的真值来源。
/// 返回 (要写入的新亮度, 更新后的待确认标记)。
pub(crate) fn theme_luma_commit(prev: f32, luma: f32, pending: bool, anchor: Option<f32>) -> (Option<f32>, bool) {
    // 壁纸锚点校验：抓屏与壁纸亮度严重不符 → 采到的是过渡帧/抓到自身，不采纳。
    if anchor.is_some_and(|a| (luma - a).abs() > 0.25) {
        return (None, false);
    }
    if prev - luma > 0.30 {
        if pending { (Some(luma), false) } else { (None, true) }
    } else {
        (Some(luma), false)
    }
}

/// 从干净 WGC 帧 src_tex 里取 theme_rect 小块算平均亮度，写 THEME_LUMA。
/// 全部失败路径静默——主题色只是视觉微调，绝不能影响零拷贝主渲染或导致崩溃。
/// sx/sy = 抓屏矩形在显示器帧里的原点；sw/sh = 显示器帧尺寸（用于裁剪源 box）。
#[cfg(windows)]
fn sample_theme_luma(
    device:&ID3D11Device, context:&ID3D11DeviceContext,
    src_tex:&ID3D11Texture2D, sx:i32, sy:i32, sw:i32, sh:i32,
) {
    let rect=THEME_RECT.lock().ok().and_then(|g|*g);
    let Some((left,top,right,bottom,rw,rh))=theme_sample_box(rect,sx,sy,sw,sh) else{return;};
    let stage={
        let Ok(mut guard)=THEME_STAGE.lock() else{return;};
        if guard.is_none(){
            let desc=D3D11_TEXTURE2D_DESC{
                Width:THEME_STAGE_W, Height:THEME_STAGE_H, MipLevels:1, ArraySize:1,
                Format:DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc:DXGI_SAMPLE_DESC{Count:1,Quality:0},
                Usage:D3D11_USAGE_STAGING, BindFlags:0,
                CPUAccessFlags:D3D11_CPU_ACCESS_READ.0 as u32, MiscFlags:0,
            };
            let mut tex:Option<ID3D11Texture2D>=None;
            if unsafe{device.CreateTexture2D(&desc,None,Some(&mut tex))}.is_err(){return;}
            *guard=tex;
        }
        let Some(stage)=guard.as_ref() else{return;};
        stage.clone()
    };
    let box_src=D3D11_BOX{left:left as u32,top:top as u32,front:0,right:right as u32,bottom:bottom as u32,back:1};
    unsafe{
        // 复制采样框到固定 staging 左上角；目标尺寸 rw×rh 恒 ≤ staging 尺寸。
        context.CopySubresourceRegion(&stage,0,0,0,0,src_tex,0,Some(&box_src));
        let mut mapped=D3D11_MAPPED_SUBRESOURCE::default();
        if context.Map(&stage,0,D3D11_MAP_READ,0,Some(&mut mapped)).is_err(){return;}
        if !mapped.pData.is_null(){
            let ptr=mapped.pData as *const u8;
            let pitch=mapped.RowPitch as usize;
            // 每行只读 rw*4 字节；rw ≤ THEME_STAGE_W，pitch ≥ THEME_STAGE_W*4，绝不越界。
            if pitch>=THEME_STAGE_W as usize*4 {
                let mut sum=0.0f32; let mut n=0.0f32;
                for y in (0..rh as usize).step_by(4){
                    let row=std::slice::from_raw_parts(ptr.add(y*pitch),rw as usize*4);
                    for x in (0..rw as usize).step_by(4){
                        let i=x*4;
                        sum+=(row[i] as f32+row[i+1] as f32+row[i+2] as f32)/3.0;
                        n+=1.0;
                    }
                }
                if n>0.0{
                    let luma=(sum/n/255.0).clamp(0.0,1.0);
                    // 开机/切屏早期桌面还在合成，抓到的过渡帧偏黑。采纳它会把液态玻璃
                    // 判成深色材质（黑底），且之后采样被节流、一直不恢复 →
                    // 「开机后液态玻璃变黑、重启才好」。
                    // 判据：从浅到深的大幅突变不立即采纳，要求连续两次采样都低才认
                    // （真·深色壁纸是持续的，过渡帧是短暂的）。浅→深平滑变化不受影响。
                    let prev=f32::from_bits(THEME_LUMA.load(Ordering::Relaxed));
                    let pending=THEME_DARK_PENDING.load(Ordering::Relaxed);
                    let anchor=crate::wallpaper::luma();
                    let (commit,new_pending)=theme_luma_commit(prev,luma,pending,anchor);
                    if let Some(v)=commit { THEME_LUMA.store(v.to_bits(),Ordering::Relaxed); }
                    THEME_DARK_PENDING.store(new_pending,Ordering::Relaxed);
                }
            }
        }
        let _=context.Unmap(&stage,0);
    }
}
/// spike: 是否启用 GPU 共享纹理路径（默认关闭，完全不影响旧路径）。
pub fn gpu_share_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        // "1"/"true"/任意非空非0 视为开启；"0"/未设置视为关闭。
        // 启动期 main.rs 会把设置页的 liquid_gpu_share 写进来。
        match std::env::var("DAOZAI_LIQUID_GPU_SHARE") {
            Ok(v) => {
                let v = v.trim();
                !v.is_empty() && v != "0" && v.to_lowercase() != "false"
            }
            Err(_) => false,
        }
    })
}
pub fn set_wgpu_luid(luid: LUID) {
    *WGPU_LUID.lock().unwrap_or_else(|e| e.into_inner()) = Some(luid);
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
/// **整个桌面拓扑**的签名（所有显示器的 rect 集合）。只比较「抓屏矩形属于哪块屏」
/// 挡不住「仅主屏显示 → 扩展」这种切换：主屏 rect 没变，但 DWM 已经重建了合成器，
/// 旧会话出的是重建期的坏帧，玻璃会花掉（表现为岛内缩进一块壁纸 + 四周发黑）。
/// 所以每几帧算一次全桌面签名，变了就一律重置。
static TOPO_SIG: AtomicU64 = AtomicU64::new(0);
/// 拓扑变化后的稳定期：这段时间内既不抓屏也不重建底片。切显示模式时桌面有
/// 1 秒左右的「向内收拢、四周变黑、再复原」动画，抓到中间态会污染底片。
static SETTLE_UNTIL: Mutex<Option<std::time::Instant>> = Mutex::new(None);
/// 拓扑查询降频计数器：EnumDisplayMonitors 不便宜，约每 100ms 查一次足够。
static TOPO_TICK: AtomicU64 = AtomicU64::new(0);
/// 切换显示模式后等待桌面稳定的时长，固定 1000ms（原先的自学习逻辑已移除：
/// 学习值全局共享，快机器学出的 400ms 不够慢机器/复制模式的重构动画用）。
const SETTLE_MS: u64 = 1000;

/// 拓扑已变化、等待抓着 WGC 锁的那一方把旧会话 drop 掉。用原子标志而不是在
/// `poll_topology` 里直接拿 WGC 锁 —— 调用它的 `grab` 可能正持着那把锁，会死锁。
static NEED_RESET: AtomicBool = AtomicBool::new(false);

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

/// 判断是否 D3D11 设备丢失（0x887A0005 DEVICE_REMOVED / 0x887A0006 DEVICE_RESET）。
/// 换显示模式（尤其复制到不同分辨率）会让 DWM 重建合成器、驱动重置我们的 D3D11 设备，
/// 此后所有 CreateTexture2D/Copy 都失败、会话永久卡死 —— 表现为背景定格在换屏前那张、
/// 视口变了纹理没变 → 玻璃被拉伸放大。按微软 WGC 指引，此时应在原 frame pool 上调
/// `Recreate` 换新 device，保留 session/item，而不是销毁整个会话（后者反复
/// StartCapture 会触发 DWM 合成抖动 + 捕获黄框闪烁）。
fn is_device_lost(e: &windows::core::Error) -> bool {
    let code = e.code().0 as u32;
    code == 0x887A0005 || code == 0x887A0006 // DXGI_ERROR_DEVICE_REMOVED / DEVICE_RESET
}

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

/// 收集各显示器 rect 的回调（EnumDisplayMonitors 要求 C 调用约定）。
unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let out = &mut *(data.0 as *mut Vec<(i32, i32, i32, i32)>);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(monitor, &mut info).as_bool() {
        let (l, t, r, b) = (
            info.rcMonitor.left,
            info.rcMonitor.top,
            info.rcMonitor.right,
            info.rcMonitor.bottom,
        );
        out.push((l, t, r - l, b - t));
    }
    // 继续枚举（TRUE = 1）。
    BOOL(1)
}

/// 整个桌面的拓扑签名：所有显示器 rect 排序后做 FNV。显示器增减、位置或分辨率
/// 变化都会改变签名；单块屏内部的窗口变化不会。
fn desktop_signature() -> u64 {
    let mut rects: Vec<(i32, i32, i32, i32)> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut rects as *mut _ as isize),
        );
    }
    if rects.is_empty() {
        return 0;
    }
    rects.sort_unstable();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (l, t, w, hh) in rects {
        for v in [l, t, w, hh] {
            h ^= v as u32 as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// 检测桌面拓扑是否刚变过（每约 100ms 实查一次，其余帧返回 false）。
///
/// 公开给 gpu_glass 的抓屏入口调用：`capture` 会在 settling() 期间直接返回 None，
/// 若不在这里先检测，稳定期内的再次切换就永远发现不了（窗口无法续期）。
pub fn poll_topology() -> bool {
    if TOPO_TICK.fetch_add(1, Ordering::Relaxed) % 6 != 0 {
        return false;
    }
    let sig = desktop_signature();
    if sig == 0 {
        return false;
    }
    let previous = TOPO_SIG.swap(sig, Ordering::Relaxed);
    let changed = previous != 0 && previous != sig;
    // 首次建立基线、以及每次变化都打一行，便于实机确认检测是否真的在工作。
    if previous == 0 || changed {
        log(&format!("desktop: sig={:016x} rects={}", sig, monitor_rects_desc()));
    }
    if changed {
        // **必须在这里就把重置登记下来**，不能靠返回值：调用方可能是 `App::frame`
        // （它只关心节拍、不处理返回值），真正 drop 会话的 `grab` 那一帧又可能被
        // 降频跳过，结果稳定期从未设置 —— 切换瞬间的全黑帧就被存成底片，玻璃永久发黑。
        NEED_RESET.store(true, Ordering::Relaxed);
        EPOCH.fetch_add(1, Ordering::Relaxed);
        *BOUND_MONITOR.lock().unwrap_or_else(|e| e.into_inner()) = None;
        FAILED.store(false, Ordering::Relaxed);
        TRIED.store(false, Ordering::Relaxed);
        let settle = settle_ms();
        let now = std::time::Instant::now();
        *SETTLE_UNTIL.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(now + std::time::Duration::from_millis(settle));
        log(&format!("desktop: 显示器拓扑变化，等待 {}ms 稳定后重抓底片", settle));
    }
    changed
}

/// 当前显示器 rect 的文字描述（仅诊断日志用）。
fn monitor_rects_desc() -> String {
    let mut rects: Vec<(i32, i32, i32, i32)> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut rects as *mut _ as isize),
        );
    }
    rects.sort_unstable();
    rects
        .iter()
        .map(|(l, t, w, h)| format!("({},{} {}x{})", l, t, w, h))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 拓扑变化后的稳定期时长，固定 1000ms。可用 `DAOZAI_SETTLE_MS` 覆盖，方便实机调试。
/// 原先的自学习（按上次稳定耗时 ×0.85 逐次收敛）已移除：学习值全局共享，快机器学出的
/// 400ms 不够慢机器/复制模式的 DWM 重构动画用，反而导致抓到过渡帧铺不满。
fn settle_ms() -> u64 {
    if let Some(v) = std::env::var("DAOZAI_SETTLE_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
    {
        return v.clamp(0, 30_000);
    }
    SETTLE_MS
}

/// 桌面拓扑刚变过、还在等 DWM 稳定：这段时间内**既不抓屏也不重建底片**。
///
/// 必须由 `App::frame` 每帧调用 `poll_topology`，不能只靠抓屏路径 —— 用户切显示
/// 模式时岛仔通常是**收起态**（不抓屏），只在开玻璃时检测会整段错过，展开后用的
/// 还是旧拓扑的底片，直接花屏。
pub fn settling() -> bool {
    SETTLE_UNTIL
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|t| std::time::Instant::now() < t)
}

fn staging(device: &ID3D11Device, w: u32, h: u32) -> windows::core::Result<ID3D11Texture2D> {    let desc = D3D11_TEXTURE2D_DESC {
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

/// spike: 按 LUID 找到与 wgpu(D3D12) 同一块 GPU 的 DXGI adapter。
/// 找不到就返回 None，让 D3D11CreateDevice 退回默认卡（此时会走系统内存中转，
/// 共享纹理仍可用但零拷贝收益打折——诊断日志会标注）。
fn adapter_by_luid(luid: LUID) -> Option<IDXGIAdapter> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        for i in 0..16u32 {
            let Ok(adapter) = factory.EnumAdapters(i) else { break };
            if let Ok(desc) = adapter.GetDesc() {
                if desc.AdapterLuid == luid { return Some(adapter); }
            }
        }
    }
    None
}

/// spike: 创建一张可供 D3D12 OpenSharedHandle 的 D3D11 纹理。
/// 用 NT handle + keyed mutex：NT handle 是 DX12 唯一支持的模型；keyed mutex
/// 用于 producer(D3D11) 写之前的独占，consumer(D3D12) 侧无法 QI 它（见 gpu_glass）。
fn shared_texture(device: &ID3D11Device, w: u32, h: u32) -> windows::core::Result<SharedFrame> {
    unsafe {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w, Height: h, MipLevels: 1, ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0
                | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0) as u32,
        };
        let mut tex: Option<ID3D11Texture2D> = None;
        device.CreateTexture2D(&desc, None, Some(&mut tex))?;
        let tex = tex.ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x8000_4005u32 as i32)))?;
        let res: IDXGIResource1 = tex.cast()?;
        let handle = res.CreateSharedHandle(None, GENERIC_ALL.0, windows::core::PCWSTR::null())?;
        let mutex: IDXGIKeyedMutex = tex.cast()?;
        let mut done: Option<ID3D11Query> = None;
        device.CreateQuery(
            &D3D11_QUERY_DESC { Query: D3D11_QUERY_EVENT, MiscFlags: 0 },
            Some(&mut done),
        )?;
        let done = done.ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x8000_4005u32 as i32)))?;
        Ok(SharedFrame { tex, handle, mutex, done, w, h })
    }
}

fn init(_x: i32, _y: i32, w: u32, h: u32,
        monitor: HMONITOR, ident: (i32, i32, i32, i32)) -> windows::core::Result<Wgc> {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
        let luid = *WGPU_LUID.lock().unwrap_or_else(|e| e.into_inner());
        // spike: 同卡优先。D3D_DRIVER_TYPE_UNKNOWN 在显式指定 adapter 时是强制要求。
        let adapter = luid.and_then(adapter_by_luid);
        let (adapter, driver_type) = match adapter {
            Some(a) => {
                log(&format!("wgc: D3D11 设备绑定到 wgpu 同卡 adapter: {:?}",
                    a.GetDesc().map(|d| String::from_utf16_lossy(&d.Description)).unwrap_or_default()));
                (Some(a), D3D_DRIVER_TYPE_UNKNOWN)
            }
            None => {
                if gpu_share_enabled() {
                    log("wgc: 警告——未匹配到 wgpu 同卡 adapter，共享纹理可能经系统内存中转");
                }
                (None, D3D_DRIVER_TYPE_HARDWARE)
            }
        };
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        D3D11CreateDevice(
            adapter.as_ref(), driver_type, windows::Win32::Foundation::HMODULE::default(),
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
        // 用 item.Size() 当帧池尺寸，而不是显示器 rcMonitor —— 微软 WGC 官方指引。
        // 复制模式会把两块屏协商到同一分辨率，item.Size() 可能小于 (sw,sh)；
        // 用 rcMonitor 建池会拿到尺寸不符的帧，配合固定 src_box 就越界/半张图。
        let item_size = item.Size()?;
        let (fw, fh) = (item_size.Width.max(1), item_size.Height.max(1));
        log(&format!("wgc: item.Size()={}x{} rcMonitor={}x{}", fw, fh, sw, sh));
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2,
            SizeInt32 { Width: fw, Height: fh },
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
        // spike: 共享纹理按需创建（失败不致命，退回旧 staging 路径）。
        let shared = if gpu_share_enabled() {
            match shared_texture(&device, w, h) {
                Ok(s) => { log("wgc: GPU 共享纹理已创建"); Some(s) }
                Err(e) => { log(&format!("wgc: 共享纹理创建失败，退回 staging 读回: {:?}", e)); None }
            }
        } else { None };
        Ok(Wgc {
            _device: device, context, _item: item, pool, _session: session,
            staging: stage, shared, mon_x, mon_y, sw, sh, stage_w: w, stage_h: h,
        })
    }
}

/// device lost 后原地恢复抓屏（微软 WGC 官方指引）。
///
/// 关键：在**原 frame pool 对象**上调 `Recreate` 换新的 D3D11 device，保留
/// session / item / pool 本身，不重新 `StartCapture`。此前做法是 `*guard = None`
/// 销毁整个 Wgc 再 init —— 每轮重建都 StartCapture + DWM 重合成 + 捕获黄框重启，
/// 换分辨率期设备反复 lost 时形成重建风暴（实测 54 次），表现为鼠标/岛体持续闪。
///
/// `Recreate` 只换 device/pool 内部纹理，session 仍绑在原 pool 上继续工作。
/// device 换掉后，staging 与共享纹理都绑在旧 device 上、必须一并重建。
/// 返回 Err 表示恢复失败（调用方按 Unavailable 回退，下一帧再试）。
fn recreate_after_device_lost(s: &mut Wgc, w: u32, h: u32) -> windows::core::Result<()> {
    unsafe {
        // 1) 新建 D3D11 device/context，仍绑定 wgpu 同卡 adapter。
        let luid = *WGPU_LUID.lock().unwrap_or_else(|e| e.into_inner());
        let adapter = luid.and_then(adapter_by_luid);
        let (adapter, driver_type) = match adapter {
            Some(a) => (Some(a), D3D_DRIVER_TYPE_UNKNOWN),
            None => (None, D3D_DRIVER_TYPE_HARDWARE),
        };
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        D3D11CreateDevice(
            adapter.as_ref(), driver_type, windows::Win32::Foundation::HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION,
            Some(&mut device), None, Some(&mut context),
        )?;
        let device = device.ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x8000_4005u32 as i32)))?;
        let context = context.ok_or_else(|| windows::core::Error::from(windows::core::HRESULT(0x8000_4005u32 as i32)))?;

        // 2) 包装成 WinRT IDirect3DDevice，在原 pool 上 Recreate。
        let dxgi: IDXGIDevice = device.cast()?;
        let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)?;
        let d3d_device: IDirect3DDevice = inspectable.cast()?;
        // 用 item.Size() 的最新尺寸（换分辨率后可能已变），与 init 一致。
        let item_size = s._item.Size()?;
        let size = SizeInt32 {
            Width: item_size.Width.max(1), Height: item_size.Height.max(1),
        };
        s.pool.Recreate(&d3d_device, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size)?;
        // 社区实证（微软 Screen capture / sekin.in / macmyths）：Recreate 会丢弃池内
        // 所有旧帧，必须 "resume the session" 才会重新驱动帧到达。此前只 Recreate
        // 不重启 session → 帧永久不再到达 → 玻璃定格（实测 Recreate 后 0 次导入/出帧）。
        // session 对象保留（没新建），只在新 pool 上重新激活；不同于此前每次整体
        // 重建的 StartCapture 风暴，这里 device 稳定后只会发生一次。
        // StartCapture 若报"已在运行"也不算失败——幂等，忽略即可；真正失败由后续帧是否到达判断。
        match s._session.StartCapture() {
            Ok(()) => log(&format!("wgc: device-lost 恢复——pool.Recreate {}x{} + session 重新激活", size.Width, size.Height)),
            Err(e) => log(&format!("wgc: device-lost 恢复——pool.Recreate {}x{}，session 重启返回 {:?}（可能已在运行，继续观察帧）", size.Width, size.Height, e)),
        }

        // 3) 重建绑在旧 device 上的 staging 与共享纹理。
        let stage = staging(&device, w, h)?;
        let shared = if gpu_share_enabled() {
            match shared_texture(&device, w, h) {
                Ok(t) => Some(t),
                Err(e) => { log(&format!("wgc: 恢复时共享纹理创建失败，回退 staging: {:?}", e)); None }
            }
        } else { None };

        s._device = device;
        s.context = context;
        s.staging = stage;
        s.stage_w = w;
        s.stage_h = h;
        s.shared = shared;
        // 新共享纹理 = 新 NT handle，递增代次让 wgpu 侧重新 OpenSharedHandle。
        EPOCH.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

/// 抓屏结果。必须区分「没有新帧」和「WGC 不可用」：
/// 前者应当复用上一帧纹理，后者才回退 GDI —— 否则会 GDI/WGC 交替导致闪烁。
pub enum Grab {
    Frame(Vec<u8>, u32, u32),
    /// spike: WGC 帧已复制进共享 D3D11 纹理，wgpu(D3D12) 侧凭 handle 直接导入，
    /// 全程不落地 CPU。
    Shared(SharedHandle),
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
    // 先查整桌面拓扑：切显示模式（仅主屏 → 扩展 / 复制 / 主屏互换）时主屏 rect
    // 可能不变，只看「矩形属于哪块屏」会漏掉。拓扑一变，旧会话与旧底片全部作废。
    let topology_changed = poll_topology();
    let mut guard = WGC.lock().unwrap_or_else(|e| e.into_inner());
    let _ = topology_changed; // 重置登记已在 poll_topology 内完成，这里只负责 drop 旧会话
    if NEED_RESET.swap(false, Ordering::Relaxed) {
        *guard = None;
        log("desktop: 旧抓屏会话已销毁，等稳定期结束后重建");
    }
    // 稳定期内不出帧：切显示模式时桌面有「向内收拢、四周变黑、再复原」的动画，
    // 抓到中间态会写进底片，之后玻璃一直是花的（就是用户看到的那种）。
    if settling() {
        return Grab::NoFrame;
    }
    if let Some((_monitor, ident)) = current {
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
    // device lost 时不能只 resize 纹理（会永久失败），要销毁整个会话重建 device。
    let mut device_lost = false;
    if s.stage_w != bw || s.stage_h != bh {
        match staging(&s._device, bw, bh) {
            Ok(t) => { s.staging = t; s.stage_w = bw; s.stage_h = bh; }
            Err(e) => {
                log(&format!("wgc: staging resize FAILED {:?}", e));
                device_lost = is_device_lost(&e);
                if !device_lost { return Grab::Unavailable; }
            }
        }
    }
    if device_lost {
        // 微软 WGC 指引：在原 pool 上 Recreate 新 device，保留 session/item，
        // 不销毁整个会话（避免 StartCapture 风暴导致换分辨率后持续闪）。
        if recreate_after_device_lost(s, bw, bh).is_err() {
            // Recreate 失败兜底：整体重建，下一帧重新 init。
            *guard = None;
            FAILED.store(false, Ordering::Relaxed);
            TRIED.store(false, Ordering::Relaxed);
            log("wgc: device lost，Recreate 失败，回退整体重建会话");
        }
        return Grab::Unavailable;
    }
    let (sx, sy) = (x - s.mon_x, y - s.mon_y);
    if sx < 0 || sy < 0 || sx + w > s.sw || sy + h > s.sh { return Grab::Unavailable; }
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
        // src_box 按显示器几何 (sx..sx+w) 取框；帧池已按 item.Size() 建，正常
        // item.Size()==rcMonitor。复制模式协商降级时真实帧更小，超出的 box 会被
        // CopySubresourceRegion 静默裁剪 → staging 留黑。这里夹到真实帧内避免读到
        // 未定义像素；返回值仍是请求的 w×h，缺失部分保持黑，交给上层按视口几何处理。
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        src_tex.GetDesc(&mut desc);
        let (real_w, real_h) = (desc.Width as i32, desc.Height as i32);
        let src_box = D3D11_BOX {
            left: sx as u32, top: sy as u32, front: 0,
            right: (sx + w).min(real_w).max(sx + 1) as u32,
            bottom: (sy + h).min(real_h).max(sy + 1) as u32, back: 1,
        };
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

/// spike: 零拷贝抓屏。与 `grab` 共用会话/拓扑管理，区别只在最后一段：
/// 把 WGC 帧 CopyResource 到一张 NT-handle 共享纹理，用 keyed mutex + event query
/// 保证 D3D11 写完后才交给 D3D12 读，**没有 Map(READ)、没有 CPU 像素拷贝**。
///
/// 同步契约（社区 scrying 0.2.0 实证方案）：
///   producer: AcquireSync(0) → CopySubresourceRegion → query(END) → Flush
///             → spin GetData 等 GPU 完成 → ReleaseSync(0)
///   consumer: D3D12 从 keyed-mutex 句柄打开的资源无法 QI 到 IDXGIKeyedMutex，
///             故 wgpu 侧改用一次性 1x1 copy 触发状态屏障刷新（见 gpu_glass）。
pub fn grab_shared(x: i32, y: i32, w: i32, h: i32) -> Grab {
    if w <= 0 || h <= 0 || w > 2048 || h > 1024 { return Grab::Unavailable; }
    let (bw, bh) = (w as u32, h as u32);
    let current = monitor_for_rect(x, y, w, h);
    let topology_changed = poll_topology();
    let mut guard = WGC.lock().unwrap_or_else(|e| e.into_inner());
    let _ = topology_changed;
    if NEED_RESET.swap(false, Ordering::Relaxed) {
        *guard = None;
        log("desktop: 旧抓屏会话已销毁，等稳定期结束后重建");
    }
    if settling() { return Grab::NoFrame; }
    if let Some((_monitor, ident)) = current {
        let bound = *BOUND_MONITOR.lock().unwrap_or_else(|e| e.into_inner());
        let changed = guard.as_ref()
            .map(|s| (s.mon_x, s.mon_y, s.sw, s.sh) != ident)
            .unwrap_or_else(|| bound.is_some_and(|b| b != ident));
        if changed {
            *guard = None;
            EPOCH.fetch_add(1, Ordering::Relaxed);
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
                    log("wgc: [gpu-share] init OK");
                    *BOUND_MONITOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(ident);
                    *guard = Some(s);
                }
                Err(e) => { FAILED.store(true, Ordering::Relaxed); log(&format!("wgc: [gpu-share] init FAILED {:?}", e)); return Grab::Unavailable; }
            }
        } else { return Grab::Unavailable; }
    }
    let s = guard.as_mut().unwrap();
    // 会话没有共享纹理（创建失败或关闭开关）→ 回退让调用方走旧路径。
    if s.shared.is_none() { return Grab::Unavailable; }
    // 展开动画里尺寸会变：共享纹理按新尺寸重建（wgpu 侧会据此重新导入）。
    let mut device_lost = false;
    if s.shared.as_ref().is_none_or(|t| t.w != bw || t.h != bh) {
        match shared_texture(&s._device, bw, bh) {
            Ok(t) => s.shared = Some(t),
            Err(e) => {
                log(&format!("wgc: [gpu-share] shared resize FAILED {:?}", e));
                device_lost = is_device_lost(&e);
                if !device_lost { return Grab::Unavailable; }
            }
        }
    }
    if device_lost {
        // 同 grab()：原 pool 上 Recreate 恢复，保留 session 避免 StartCapture 风暴。
        // 注意共享纹理尺寸用 bw/bh（有效帧），与当前缓存键一致。
        if recreate_after_device_lost(s, bw, bh).is_err() {
            *guard = None;
            FAILED.store(false, Ordering::Relaxed);
            TRIED.store(false, Ordering::Relaxed);
            log("wgc: [gpu-share] device lost，Recreate 失败，回退整体重建会话");
        }
        return Grab::Unavailable;
    }
    let (sx, sy) = (x - s.mon_x, y - s.mon_y);
    if sx < 0 || sy < 0 || sx + w > s.sw || sy + h > s.sh { return Grab::Unavailable; }
    unsafe {
        // 帧池异步：连取到没有新帧为止，用最新一帧（最多丢 4 帧），与 grab 一致。
        let mut frame = s.pool.TryGetNextFrame().ok();
        for _ in 0..4 {
            match s.pool.TryGetNextFrame() { Ok(f) => frame = Some(f), Err(_) => break }
        }
        let Some(frame) = frame else { return Grab::NoFrame };
        let surface = match frame.Surface() { Ok(v) => v, Err(_) => return Grab::NoFrame };
        let access: IDirect3DDxgiInterfaceAccess = match surface.cast() { Ok(v) => v, Err(_) => return Grab::Unavailable };
        let src_tex: ID3D11Texture2D = match access.GetInterface() { Ok(v) => v, Err(_) => return Grab::Unavailable };
        // 同 grab()：复制模式下真实帧可能比显示器几何小，box 夹到真实帧内，
        // 不丢帧（丢帧会让零拷贝纹理永久停在换屏前那张 → 背景定格/铺不满）。
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        src_tex.GetDesc(&mut desc);
        let (real_w, real_h) = (desc.Width as i32, desc.Height as i32);
        let box_clamped = D3D11_BOX {
            left: sx as u32, top: sy as u32, front: 0,
            right: (sx + w).min(real_w).max(sx + 1) as u32,
            bottom: (sy + h).min(real_h).max(sy + 1) as u32, back: 1,
        };

        let shared = s.shared.as_ref().unwrap();
        // producer 独占：keyed mutex key=0，超时 500ms 放弃（避免卡住 UI 线程）。
        if shared.mutex.AcquireSync(0, 500).is_err() { drop(frame); return Grab::NoFrame; }
        // D3D11_QUERY_EVENT 按微软文档 Begin 是 disabled 的，只能用 End 标记
        // GPU 时间线事件点 + GetData 轮询 BOOL（Chromium ANGLE/Dawn 均如此）。
        // 此前错误地 Begin(EVENT query) 让 query 状态机异常、GetData 永不 signal
        // → 误判 device lost → 触发 Recreate/整体重建 → 玻璃定格/闪。
        s.context.CopySubresourceRegion(&shared.tex, 0, 0, 0, 0, &src_tex, 0, Some(&box_clamped));
        s.context.End(&shared.done);
        s.context.Flush();
        // 等 GPU 写完：yield 让出 CPU（纯 spin 占死主线程 → 改分辨率后输入卡），
        // 每 256 次查一次 device lost 提前退出，上限兜底防死锁。
        let mut signaled: i32 = 0;
        let mut spins = 0u32;
        let mut lost_early = false;
        while signaled == 0 && spins < 2_000_000 {
            let _ = s.context.GetData(&shared.done, Some(&mut signaled as *mut _ as *mut core::ffi::c_void), 4, 0);
            spins += 1;
            if signaled != 0 { break; }
            if spins % 256 == 0 && unsafe { s._device.GetDeviceRemovedReason().is_err() } {
                lost_early = true;
                break;
            }
            std::thread::yield_now();
        }
        if lost_early { signaled = 0; }
        // 主题色小范围取样：按时间节流（≥500ms），从这张干净帧读采样框算亮度。
        // 用 Instant 而非帧计数：低帧率下主题滞后不放大，60fps 也不必每帧读回。
        // 失败静默、与共享纹理/keyed mutex 无关（副本在独立固定 staging 上读回）。
        let now = Instant::now();
        let due = match THEME_LAST_SAMPLE.lock() {
            Ok(mut guard) => {
                let epoch = theme_epoch();
                let now_ms = now.duration_since(epoch).as_millis() as u64;
                let last_ms = (*guard).map(|t| t.duration_since(epoch).as_millis() as u64);
                if theme_sample_due(now_ms, last_ms, THEME_SAMPLE_INTERVAL_MS) {
                    *guard = Some(now);
                    true
                } else { false }
            }
            Err(_) => false, // 锁中毒则本次跳过，不影响主渲染
        };
        if due {
            sample_theme_luma(&s._device, &s.context, &src_tex, sx, sy, s.sw, s.sh);
        }
        let _ = shared.mutex.ReleaseSync(0);
        drop(frame);
        if signaled == 0 {
            // 超时常见原因是 device lost：device 被移除后 query 永不 signal。
            // 检查一次，是则销毁会话重建；否则只是偶发丢帧。
            let lost = unsafe { s._device.GetDeviceRemovedReason().is_err() };
            log(&format!("wgc: [gpu-share] event query 超时，device_lost={}", lost));
            if lost {
                // 同前：原 pool Recreate 恢复，避免每次超时都整体重建 → StartCapture 风暴。
                if recreate_after_device_lost(s, bw, bh).is_err() {
                    *guard = None;
                    FAILED.store(false, Ordering::Relaxed);
                    TRIED.store(false, Ordering::Relaxed);
                }
            }
            return Grab::NoFrame;
        }
        let n = FRAMES.fetch_add(1, Ordering::Relaxed);
        if n == 0 || n % 600 == 0 { log(&format!("wgc: [gpu-share] frame {} cap={}x{}", n, bw, bh)); }
        Grab::Shared(SharedHandle { handle: shared.handle, w: bw, h: bh, epoch: EPOCH.load(Ordering::Relaxed) })
    }
}


// ===== 单测：锁定主题取样边界逻辑，确保尺寸变化时绝不越界 =====
#[cfg(test)]
mod theme_sample_tests {
    use super::{theme_sample_box, theme_sample_due, theme_luma_commit, THEME_STAGE_W, THEME_STAGE_H, THEME_SAMPLE_INTERVAL_MS};

    #[test]
    fn 开机过渡暗帧不立即采纳(){
        // 没有壁纸锚点时走连续两次确认
        assert_eq!(theme_luma_commit(0.85,0.2,false,None),(None,true));
        assert_eq!(theme_luma_commit(0.85,0.2,true,None),(Some(0.2),false));
        assert_eq!(theme_luma_commit(0.85,0.7,false,None),(Some(0.7),false));
        assert_eq!(theme_luma_commit(0.2,0.8,false,None),(Some(0.8),false));
        assert_eq!(theme_luma_commit(0.85,0.83,true,None),(Some(0.83),false));
    }
    #[test]
    fn 壁纸锚点挡掉过渡暗帧(){
        // 壁纸是浅色 0.9，抓屏采到 0.2（开机过渡帧）→ 直接拒绝，不进待确认
        assert_eq!(theme_luma_commit(0.85,0.2,false,Some(0.9)),(None,false));
        // 差 0.04（正常采样区间差异）→ 采纳
        assert_eq!(theme_luma_commit(0.85,0.86,false,Some(0.9)),(Some(0.86),false));
        // 真深色壁纸 0.15，抓屏采到 0.18 → 锚点一致；prev 已接近(0.17)，平滑采纳
        assert_eq!(theme_luma_commit(0.17,0.18,false,Some(0.15)),(Some(0.18),false));
        // prev 仍是旧浅色时，深色壁纸也要连续两次确认（防过渡帧）
        assert_eq!(theme_luma_commit(0.85,0.18,false,Some(0.15)),(None,true));
        assert_eq!(theme_luma_commit(0.85,0.18,true,Some(0.15)),(Some(0.18),false));
        // 恰好 0.25 边界 → 采纳（严格大于才拒绝）
        assert_eq!(theme_luma_commit(0.85,0.65,false,Some(0.9)),(Some(0.65),false));
    }

    // 按时间节流的纯判断：从未采过立即采；未到间隔不采；到间隔采。
    #[test]
    fn theme_sample_due_respects_interval() {
        let iv = THEME_SAMPLE_INTERVAL_MS;
        assert!(theme_sample_due(1000, None, iv), "首次应立即采样");
        assert!(!theme_sample_due(1499, Some(1000), iv), "间隔未满 500ms 不采");
        assert!(theme_sample_due(1500, Some(1000), iv), "正好 500ms 应采样");
        assert!(theme_sample_due(9999, Some(1000), iv), "远超间隔应采样");
    }

    // 低帧率场景：帧数少不该拖慢主题响应——只看时间是否到期。
    #[test]
    fn theme_sample_due_is_frame_rate_independent() {
        // 假设 5fps：200ms 一帧，累计 500ms 才 3 帧。关键是时间到就采，与帧数无关。
        let iv = THEME_SAMPLE_INTERVAL_MS;
        assert!(!theme_sample_due(400, Some(0), iv), "5fps 第 2 帧 400ms 未到期");
        assert!(theme_sample_due(600, Some(0), iv), "5fps 第 3 帧 600ms 已到期，立即采样");
        // 时间倒退（系统时钟回拨）时用 saturating_sub 兜底，不会误判成到期。
        assert!(!theme_sample_due(100, Some(500), iv), "时钟回拨不应触发采样");
    }

    // 正常情况：采样框完整落在显示器帧内，原样换算。
    #[test]
    fn theme_sample_box_normal_within_frame() {
        // rect=(10,20)-(110,80)，抓屏原点(0,0)，显示器 1920x1080
        assert_eq!(
            theme_sample_box(Some((10, 20, 110, 80)), 0, 0, 1920, 1080),
            Some((10, 20, 110, 80, 100, 60))
        );
    }

    // 抓屏原点非零（多显示器，主屏在右）：源 box 要叠加 sx/sy 偏移。
    #[test]
    fn theme_sample_box_respects_capture_origin() {
        // 抓屏原点 sx=100,sy=50；rect=(10,20)-(60,40) → box=(110,70)-(160,90)
        assert_eq!(
            theme_sample_box(Some((10, 20, 60, 40)), 100, 50, 1920, 1080),
            Some((110, 70, 160, 90, 50, 20))
        );
    }

    // 尺寸变化核心场景：采样框宽高超过固定 staging 上限，必须被夹住，
    // 这样读回时 from_raw_parts(ptr, rw*4) 的 rw 恒 ≤ 1024，绝不越界。
    #[test]
    fn theme_sample_box_clamps_to_staging_limit() {
        // 起点在帧内、宽高远超固定 staging 上限：必须夹到 1024×256 而非越界。
        let huge = (
            0,
            0,
            THEME_STAGE_W as i32 * 3,
            THEME_STAGE_H as i32 * 3,
        );
        let got = theme_sample_box(Some(huge), 0, 0, 100000, 100000);
        let (l, t, r, b, rw, rh) = got.expect("宽高夹取后仍应有效");
        assert_eq!(rw, THEME_STAGE_W, "rw 必须夹到 1024");
        assert_eq!(rh, THEME_STAGE_H, "rh 必须夹到 256");
        assert_eq!(r - l, THEME_STAGE_W as i32, "源 box 宽应与 rw 一致");
        assert_eq!(b - t, THEME_STAGE_H as i32, "源 box 高应与 rh 一致");
        // 读回字节数不得超过一行 pitch 下限（1024*4）——这就是防越界的本质。
        assert!((rw as usize) * 4 <= THEME_STAGE_W as usize * 4);
    }

    // 源 box 右下越出显示器帧 → 必须拒绝（CopySubresourceRegion 会失败）。
    #[test]
    fn theme_sample_box_rejects_out_of_frame() {
        // right=2000 超上限夹到 1024 后 right=1034 合法；这里用「夹取后仍越界」的坐标：
        // 起点 1500 + 夹取后的 1024 = 2524 > sw=1920
        assert_eq!(theme_sample_box(Some((1500, 10, 3000, 60)), 0, 0, 1920, 1080), None);
        // 起点 y=1000 + 夹取后的 256 = 1256 > sh=1080
        assert_eq!(theme_sample_box(Some((10, 1000, 60, 3000)), 0, 0, 1920, 1080), None);
        // 叠加非零原点后越界：sx=1900 + left=10 + 宽50 = 1960 > 1920
        assert_eq!(theme_sample_box(Some((10, 10, 60, 60)), 1900, 0, 1920, 1080), None);
    }

    // 负坐标：源 box 左上越出显示器帧 → 拒绝。
    #[test]
    fn theme_sample_box_rejects_negative_origin() {
        assert_eq!(theme_sample_box(Some((-5, 10, 40, 60)), 0, 0, 1920, 1080), None);
        assert_eq!(theme_sample_box(Some((10, -5, 40, 60)), 0, 0, 1920, 1080), None);
    }

    // 退化/非法矩形与非法显示器尺寸 → 拒绝。
    #[test]
    fn theme_sample_box_rejects_degenerate_and_invalid_frame() {
        assert_eq!(theme_sample_box(None, 0, 0, 1920, 1080), None);
        assert_eq!(theme_sample_box(Some((10, 10, 10, 60)), 0, 0, 1920, 1080), None); // 宽=0
        assert_eq!(theme_sample_box(Some((10, 10, 60, 10)), 0, 0, 1920, 1080), None); // 高=0
        assert_eq!(theme_sample_box(Some((60, 60, 10, 10)), 0, 0, 1920, 1080), None); // 翻转
        assert_eq!(theme_sample_box(Some((10, 10, 60, 60)), 0, 0, 0, 0), None);       // 空显示器
        assert_eq!(theme_sample_box(Some((10, 10, 60, 60)), 0, 0, -1920, 1080), None); // 负尺寸
    }

    // 极端 i32 坐标：全程 i64 中间运算，绝不溢出 panic（debug 构建也会过）。
    #[test]
    fn theme_sample_box_never_overflows() {
        // rx1-rx0 接近 i32 上限；斜杠坐标取 i32 极值。
        assert_eq!(
            theme_sample_box(Some((i32::MIN, i32::MIN, i32::MAX, i32::MAX)), 0, 0, 1920, 1080),
            None
        );
        // 抓屏原点为极值，与采样框相加不会 wrap 成"看似合法"的小坐标。
        assert_eq!(
            theme_sample_box(Some((1, 1, 50, 50)), i32::MAX, i32::MAX, 1920, 1080),
            None
        );
        assert_eq!(
            theme_sample_box(Some((1, 1, 50, 50)), i32::MIN, i32::MIN, 1920, 1080),
            None
        );
    }

    // 紧贴下/右边界：等于显示器帧尺寸是合法的（right==sw, bottom==sh）。
    #[test]
    fn theme_sample_box_allows_exact_edge() {
        assert_eq!(
            theme_sample_box(Some((1820, 1020, 1920, 1080)), 0, 0, 1920, 1080),
            Some((1820, 1020, 1920, 1080, 100, 60))
        );
    }

    // 属性式扫描：在多种显示器尺寸下，只要返回 Some，保证 rw/rh 不超上限、
    // box 落在帧内、且 w==rw/h==rh ——覆盖"尺寸频繁变化"的连续取值。
    #[test]
    fn theme_sample_box_invariant_across_sizes() {
        for sw in [640, 1280, 1920, 2560, 3840] {
            for sh in [480, 720, 1080, 1440, 2160] {
                for (rx0, ry0) in [(0, 0), (7, 13), (100, 200)] {
                    for (w, h) in [(50, 30), (1000, 400), (5000, 3000)] {
                        let rect = (rx0, ry0, rx0 + w, ry0 + h);
                        if let Some((l, t, r, b, rw, rh)) =
                            theme_sample_box(Some(rect), 0, 0, sw, sh)
                        {
                            assert!(rw <= THEME_STAGE_W && rh <= THEME_STAGE_H);
                            assert!(l >= 0 && t >= 0 && r <= sw && b <= sh);
                            assert_eq!(r - l, rw as i32);
                            assert_eq!(b - t, rh as i32);
                            assert!((rw as usize) * 4 <= THEME_STAGE_W as usize * 4);
                        }
                    }
                }
            }
        }
    }
}
