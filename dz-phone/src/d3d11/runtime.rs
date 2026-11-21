//! D3D11 自绘渲染运行时：单 device 上 WGC 抓屏 → HLSL 玻璃 → egui widgets →
//! Present → DComp Commit。从 egui-directx11 的 glass_spike.rs 移植到 windows 0.58。
//!
//! 透明路线：CreateSwapChainForComposition + IDCompositionVisual（逐像素预乘 alpha），
//! 窗口用 WS_EX_NOREDIRECTIONBITMAP，玻璃外区域完全透明显示桌面。

use super::renderer::{Renderer, RendererOutput};
use super::wgc_capture::WgcCapture;
use std::ptr;
use windows::core::Interface;
use windows::Win32::{
    Foundation::{BOOL, HMODULE, HWND, RECT},
    Graphics::{
        Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0, D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST},
        Direct3D11::*,
        DirectComposition::{DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual},
        Dxgi::{Common::*, *},
    },
};

pub const MAX_CTL: usize = 13;

/// 玻璃层 uniform，布局必须与 shaders/glass.hlsl 的 `cbuffer U` 完全一致（11 x float4）。
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GlassParams {
    /// [x, y, w, h] 岛体玻璃在窗口内的矩形（物理像素）
    pub bounds: [f32; 4],
    /// [chroma, -, morph, material]
    pub params: [f32; 4],
    /// [radius, texW, texH, -]
    pub extra: [f32; 4],
    /// [r, g, b, a] 玻璃染色
    pub tint: [f32; 4],
    /// [flipY, origin_x, origin_y, -] WGC 全屏帧采样用窗口屏幕原点
    pub quad: [f32; 4],
    /// [pointer_x, pointer_y, refract, flat]
    pub misc: [f32; 4],
    /// 控件玻璃矩形（最多 MAX_CTL 个）
    pub crect: [[f32; 4]; MAX_CTL],
    /// 控件玻璃参数 [radius, enabled, magnify, refract]
    pub cprm: [[f32; 4]; MAX_CTL],
    /// [blur, band, edge_width, edge_opacity]
    pub look: [f32; 4],
    /// [-, -, -, isDComp=1] 逐像素 alpha
    pub notch: [f32; 4],
    /// 光源配置
    pub lightcfg: [f32; 4],
}

struct GlassPipeline {
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    cbuf: ID3D11Buffer,
    sampler: ID3D11SamplerState,
    blend: ID3D11BlendState,
    bg_srv: ID3D11ShaderResourceView,
    bg_tex: ID3D11Texture2D,
    bg_w: u32,
    bg_h: u32,
}

struct DcompState {
    _device: IDCompositionDevice,
    _visual: IDCompositionVisual,
    _target: IDCompositionTarget,
}

/// 一帧的渲染统计（供 UI 状态行显示）。
#[derive(Clone, Copy, Default)]
pub struct FrameStats {
    pub fps: f32,
    pub wgc_live: bool,
}

/// 渲染运行时。持有窗口生命周期内不能重建的 D3D/DComp 状态。
pub struct D3d11Runtime {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    swap: IDXGISwapChain1,
    rt: Option<ID3D11RenderTargetView>,
    dcomp: DcompState,
    glass: GlassPipeline,
    wgc: Option<WgcCapture>,
    wgc_ever_frame: bool,
    pub egui_ctx: egui::Context,
    egui_renderer: Renderer,
    egui_winit: Option<egui_winit::State>,
    frames: u64,
    last_report: std::time::Instant,
    fps: f32,
    /// 上一帧 egui 请求的重绘延迟（about_to_wait 用）
    pub repaint_delay: std::time::Duration,
    /// 主窗口 viewport 信息（窗口命令处理用）
    viewport_info: egui::ViewportInfo,
    actions: Vec<egui_winit::ActionRequested>,
}

impl D3d11Runtime {
    pub fn new(window: &winit::window::Window) -> windows::core::Result<Self> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::Win32(h) = window.window_handle().unwrap().as_raw() else {
            return Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL));
        };
        let size = window.inner_size();
        eprintln!("[d3d11] create_device_swap {}x{} hwnd={:?}", size.width, size.height, h.hwnd.get());
        let (device, ctx, swap, dcomp) = match Self::create_device_swap(HWND(h.hwnd.get() as _), size.width, size.height) {
            Ok(v) => { eprintln!("[d3d11] device/swap/dcomp OK"); v }
            Err(e) => { eprintln!("[d3d11] device/swap FAILED: {:?}", e); return Err(e); }
        };
        let rt = Self::create_rt(&device, &swap)?;

        let egui_ctx = egui::Context::default();
        let egui_renderer = Renderer::new(&device)?;
        let egui_winit = egui_winit::State::new(
            egui_ctx.clone(), egui_ctx.viewport_id(), window, None, None, None);

        // WGC 与主 device 同一份，CopyResource 纯 GPU、零回读。失败时背景保持黑色，
        // 玻璃仍可显示（HLSL 采样黑底），不影响 UI 交互。
        let wgc = match WgcCapture::new(&device) {
            Ok(c) => { eprintln!("[d3d11] WGC OK {}x{}", c.size().0, c.size().1); Some(c) }
            Err(e) => { eprintln!("[d3d11] WGC unavailable: {:?}", e); None }
        };
        let (bg_w, bg_h) = wgc.as_ref().map(|c| c.size()).unwrap_or((1920, 1080));
        eprintln!("[d3d11] creating glass pipeline bg={}x{}", bg_w, bg_h);
        let glass = match Self::create_glass_pipeline(&device, bg_w, bg_h) {
            Ok(g) => { eprintln!("[d3d11] glass pipeline OK"); g }
            Err(e) => { eprintln!("[d3d11] glass pipeline FAILED: {:?}", e); return Err(e); }
        };

        Ok(Self {
            device, ctx, swap, rt: Some(rt), dcomp, glass, wgc,
            wgc_ever_frame: false, egui_ctx, egui_renderer, egui_winit: Some(egui_winit),
            frames: 0, last_report: std::time::Instant::now(), fps: 0.0,
            repaint_delay: std::time::Duration::ZERO,
            viewport_info: egui::ViewportInfo::default(), actions: Vec::new(),
        })
    }

    fn create_device_swap(window: HWND, w: u32, h: u32)
        -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext, IDXGISwapChain1, DcompState)> {
        let factory: IDXGIFactory2 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS::default()) }?;
        let adapter: IDXGIAdapter = unsafe { factory.EnumAdapters(0) }?;
        let mut device = None;
        let mut ctx = None;
        unsafe {
            // windows 0.58: 第 3 参 software 是 Param<HMODULE>，与 0.62 直接 HMODULE 不同。
            D3D11CreateDevice(
                &adapter, D3D_DRIVER_TYPE_UNKNOWN, HMODULE(ptr::null_mut()),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut ctx),
            )?;
        }
        let device = device.unwrap();
        let ctx = ctx.unwrap();

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: w, Height: h,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            Stereo: BOOL(0),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: 0,
        };
        let swap = unsafe { factory.CreateSwapChainForComposition(&device, &desc, None)? };

        let dxgi_device: IDXGIDevice = device.cast()?;
        let dcomp: IDCompositionDevice = unsafe { DCompositionCreateDevice2(&dxgi_device)? };
        let visual: IDCompositionVisual = unsafe { dcomp.CreateVisual()? };
        let target: IDCompositionTarget = unsafe { dcomp.CreateTargetForHwnd(window, true)? };
        unsafe {
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
        }
        let dcomp_state = DcompState { _device: dcomp, _visual: visual, _target: target };
        Ok((device, ctx, swap, dcomp_state))
    }

    fn create_rt(device: &ID3D11Device, swap: &IDXGISwapChain1) -> windows::core::Result<ID3D11RenderTargetView> {
        let tex = unsafe { swap.GetBuffer::<ID3D11Texture2D>(0)? };
        let mut rt = None;
        unsafe { device.CreateRenderTargetView(&tex, None, Some(&mut rt))? };
        Ok(rt.unwrap())
    }

    fn create_glass_pipeline(device: &ID3D11Device, bg_w: u32, bg_h: u32) -> windows::core::Result<GlassPipeline> {
        let vs_blob = include_bytes!("../../shaders/glass_vs.bin");
        let ps_blob = include_bytes!("../../shaders/glass_ps.bin");
        unsafe {
            let mut vs = None;
            device.CreateVertexShader(vs_blob, None, Some(&mut vs))?;
            let mut ps = None;
            device.CreatePixelShader(ps_blob, None, Some(&mut ps))?;
            let mut cbuf = None;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: std::mem::size_of::<GlassParams>() as u32,
                    Usage: D3D11_USAGE_DYNAMIC,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
                    MiscFlags: 0, StructureByteStride: 0,
                }, None, Some(&mut cbuf))?;
            let mut sampler = None;
            device.CreateSamplerState(
                &D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    ComparisonFunc: D3D11_COMPARISON_ALWAYS,
                    BorderColor: [0.0; 4], MaxAnisotropy: 1,
                    MinLOD: 0.0, MaxLOD: f32::MAX, MipLODBias: 0.0,
                }, Some(&mut sampler))?;
            let mut blend = None;
            device.CreateBlendState(
                &D3D11_BLEND_DESC {
                    AlphaToCoverageEnable: BOOL(0), IndependentBlendEnable: BOOL(0),
                    RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: BOOL(1),
                        SrcBlend: D3D11_BLEND_ONE, DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                        BlendOp: D3D11_BLEND_OP_ADD,
                        SrcBlendAlpha: D3D11_BLEND_ONE, DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                        BlendOpAlpha: D3D11_BLEND_OP_ADD,
                        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    }; 8],
                }, Some(&mut blend))?;

            // 背景纹理：BGRA，尺寸匹配 WGC 帧。初始全黑（WGC 首帧后由 CopyResource 覆盖）。
            let desc = D3D11_TEXTURE2D_DESC {
                Width: bg_w, Height: bg_h, MipLevels: 1, ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                CPUAccessFlags: 0, MiscFlags: 0,
            };
            let mut bg_tex = None;
            device.CreateTexture2D(&desc, None, Some(&mut bg_tex))?;
            let bg_tex = bg_tex.unwrap();
            let mut bg_srv = None;
            device.CreateShaderResourceView(&bg_tex, None, Some(&mut bg_srv))?;

            Ok(GlassPipeline {
                vs: vs.unwrap(), ps: ps.unwrap(), cbuf: cbuf.unwrap(),
                sampler: sampler.unwrap(), blend: blend.unwrap(),
                bg_srv: bg_srv.unwrap(), bg_tex, bg_w, bg_h,
            })
        }
    }

    /// 窗口尺寸变化：重建 RT（swap 由 DComp 持有，ResizeBuffers 即可）。
    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 { return; }
        self.rt.take();
        unsafe { let _ = self.swap.ResizeBuffers(2, w, h, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SWAP_CHAIN_FLAG(0)); }
        self.rt = Self::create_rt(&self.device, &self.swap).ok();
    }

    /// 取 egui-winit 输入（由事件循环在 Redraw 时调用）。
    pub fn take_input(&mut self, window: &winit::window::Window) -> egui::RawInput {
        self.egui_winit.as_mut().unwrap().take_egui_input(window)
    }

    pub fn on_window_event(&mut self, window: &winit::window::Window, event: &winit::event::WindowEvent) -> bool {
        let resp = self.egui_winit.as_mut().unwrap().on_window_event(window, event);
        if !resp.consumed {
            if let winit::event::WindowEvent::Resized(s) = event {
                self.resize(s.width, s.height);
            }
        }
        resp.repaint
    }

    /// 渲染一帧：clear 透明 → 抓屏 → HLSL 玻璃 → egui → Present → Commit。
    /// `run_ui` 回调里调用岛仔的 Shell 绘制逻辑。
    pub fn render(&mut self, window: &winit::window::Window, origin: (f32, f32),
                  params: &GlassParams, input: egui::RawInput,
                  mut run_ui: impl FnMut(&egui::Context, &mut egui::Ui)) -> FrameStats {
        // WGC 抓屏（GPU 侧 CopyResource）
        if let Some(wgc) = self.wgc.as_ref() {
            if wgc.grab_into(&self.ctx, &self.glass.bg_tex) { self.wgc_ever_frame = true; }
        }
        // HLSL uniform：补上窗口原点与 flipY（WGC top-down）
        let mut u = *params;
        u.quad[0] = if self.wgc.is_some() { 1.0 } else { 0.0 };
        u.quad[1] = origin.0;
        u.quad[2] = origin.1;
        u.notch[3] = 1.0; // DComp 逐像素 alpha

        // 更新窗口信息（位置/尺寸/DPI），供 egui 生成正确的 ViewportCommand。
        egui_winit::update_viewport_info(&mut self.viewport_info, &self.egui_ctx, window, false);
        // egui pass（ctx 先克隆，避免闭包内同时借用 self 与移动 run_ui）
        let ctx = self.egui_ctx.clone();
        let egui_output = ctx.run_ui(input, |ui| run_ui(&ctx, ui));
        let platform = {
            let (ro, po, viewports) = super::renderer::split_output(egui_output);
            // 记录 egui 请求的重绘间隔（岛仔隐藏态 1fps、常态按设置 fps）
            let mut d = std::time::Duration::ZERO;
            for (_, vo) in viewports.iter() { d = d.max(vo.repaint_delay); }
            self.repaint_delay = d;
            // 处理主窗口命令（OuterPosition/Visible/SetTheme 等）——egui_winit 的
            // handle_platform_output 只处理光标/剪贴板，窗口命令必须走这里。
            if let Some(vo) = viewports.get(&egui::ViewportId::ROOT) {
                egui_winit::process_viewport_commands(&ctx, &mut self.viewport_info,
                    vo.commands.clone(), window, &mut self.actions);
            }
            self.egui_winit.as_mut().unwrap().handle_platform_output(window, po);
            ro
        };
        // 帧率统计
        self.frames += 1;
        let elapsed = self.last_report.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            self.fps = self.frames as f32 / elapsed;
            self.frames = 0;
            self.last_report = std::time::Instant::now();
        }
        let size = window.inner_size();

        if let Some(rt) = self.rt.clone() {
            unsafe {
                self.ctx.ClearRenderTargetView(&rt, &[0.0, 0.0, 0.0, 0.0]);
                self.ctx.OMSetRenderTargets(Some(&[Some(rt.clone())]), None);
            }
            self.draw_glass(size.width, size.height, &u);
            let _ = self.egui_renderer.render(&self.ctx, &rt, &self.egui_ctx, platform);
            unsafe { let _ = self.swap.Present(1, DXGI_PRESENT(0)); }
            unsafe { let _ = self.dcomp._device.Commit(); }
        }
        FrameStats { fps: self.fps, wgc_live: self.wgc_ever_frame }
    }

    fn draw_glass(&mut self, win_w: u32, win_h: u32, u: &GlassParams) {
        unsafe {
            let mut mapped = std::mem::zeroed::<D3D11_MAPPED_SUBRESOURCE>();
            if self.ctx.Map(&self.glass.cbuf, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped)).is_ok() {
                ptr::copy_nonoverlapping(u as *const GlassParams as *const u8,
                    mapped.pData as *mut u8, std::mem::size_of::<GlassParams>());
                self.ctx.Unmap(&self.glass.cbuf, 0);
            }
            let ctx = &self.ctx;
            ctx.VSSetShader(&self.glass.vs, None);
            ctx.PSSetShader(&self.glass.ps, None);
            ctx.VSSetConstantBuffers(0, Some(&[Some(self.glass.cbuf.clone())]));
            ctx.PSSetConstantBuffers(0, Some(&[Some(self.glass.cbuf.clone())]));
            ctx.PSSetShaderResources(0, Some(&[Some(self.glass.bg_srv.clone())]));
            ctx.PSSetSamplers(0, Some(&[Some(self.glass.sampler.clone())]));
            ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0, TopLeftY: 0.0, Width: win_w as f32, Height: win_h as f32,
                MinDepth: 0.0, MaxDepth: 1.0,
            }]));
            ctx.OMSetBlendState(&self.glass.blend, Some(&[0.0; 4]), u32::MAX);
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.Draw(3, 0);
        }
    }
}

/// 轻量子窗口渲染器：普通 egui 窗口（设置页/邮件阅读器等独立 viewport），
/// 无 WGC 玻璃，用不透明 clear（或按 builder.transparent 决定）。
pub struct PlainWindow {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    swap: IDXGISwapChain1,
    rt: Option<ID3D11RenderTargetView>,
    pub egui_winit: egui_winit::State,
    renderer: Renderer,
    pub transparent: bool,
    viewport_info: egui::ViewportInfo,
    actions: Vec<egui_winit::ActionRequested>,
}

impl PlainWindow {
    pub fn new(window: &winit::window::Window, egui_ctx: &egui::Context, transparent: bool) -> windows::core::Result<Self> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::Win32(h) = window.window_handle().unwrap().as_raw() else {
            return Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL));
        };
        let size = window.inner_size();
        let factory: IDXGIFactory2 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS::default()) }?;
        let adapter: IDXGIAdapter = unsafe { factory.EnumAdapters(0) }?;
        let mut device = None; let mut ctx = None;
        unsafe {
            D3D11CreateDevice(&adapter, D3D_DRIVER_TYPE_UNKNOWN, HMODULE(ptr::null_mut()),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION, Some(&mut device), None, Some(&mut ctx))?;
        }
        let device = device.unwrap(); let ctx = ctx.unwrap();
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: size.width, Height: size.height,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM, Stereo: BOOL(0),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT, BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH, SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: if transparent { DXGI_ALPHA_MODE_PREMULTIPLIED } else { DXGI_ALPHA_MODE_IGNORE },
            Flags: 0,
        };
        // 普通子窗口用 Hwnd swapchain（不需 DComp）；透明用 no_redirection + premultiplied。
        let swap: IDXGISwapChain1 = unsafe { factory.CreateSwapChainForHwnd(
            &device, HWND(h.hwnd.get() as _), &desc, None, None)? };
        let tex = unsafe { swap.GetBuffer::<ID3D11Texture2D>(0)? };
        let mut rt = None;
        unsafe { device.CreateRenderTargetView(&tex, None, Some(&mut rt))? };
        let renderer = Renderer::new(&device)?;
        let egui_winit = egui_winit::State::new(egui_ctx.clone(), egui_ctx.viewport_id(), window, None, None, None);
        Ok(Self { device, ctx, swap, rt, egui_winit, renderer, transparent, viewport_info: egui::ViewportInfo::default(), actions: Vec::new() })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 { return; }
        self.rt.take();
        unsafe { let _ = self.swap.ResizeBuffers(2, w, h, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SWAP_CHAIN_FLAG(0)); }
        let tex = unsafe { self.swap.GetBuffer::<ID3D11Texture2D>(0) };
        if let Ok(tex) = tex {
            let mut rt = None;
            unsafe { let _ = self.device.CreateRenderTargetView(&tex, None, Some(&mut rt)); }
            self.rt = rt;
        }
    }

    pub fn on_event(&mut self, window: &winit::window::Window, event: &winit::event::WindowEvent) -> bool {
        let resp = self.egui_winit.on_window_event(window, event);
        if !resp.consumed {
            if let winit::event::WindowEvent::Resized(s) = event { self.resize(s.width, s.height); }
        }
        resp.repaint
    }

    /// 渲染子 viewport 的 UI。
    pub fn render(&mut self, window: &winit::window::Window, egui_ctx: &egui::Context,
                  mut input: egui::RawInput, viewport_id: egui::ViewportId,
                  mut run_ui: impl FnMut(&mut egui::Ui)) {
        // 子 viewport 的输入必须带自己的 viewport_id，否则 egui 当成 ROOT。
        input.viewport_id = viewport_id;
        egui_winit::update_viewport_info(&mut self.viewport_info, egui_ctx, window, false);
        let output = egui_ctx.run_ui(input, |ui| run_ui(ui));
        let (ro, po, viewports) = super::renderer::split_output(output);
        if let Some(vo) = viewports.get(&viewport_id) {
            egui_winit::process_viewport_commands(egui_ctx, &mut self.viewport_info,
                vo.commands.clone(), window, &mut self.actions);
        }
        self.egui_winit.handle_platform_output(window, po);
        let size = window.inner_size();
        if let Some(rt) = self.rt.clone() {
            unsafe {
                self.ctx.OMSetRenderTargets(Some(&[Some(rt.clone())]), None);
                let clear = if self.transparent { [0.0, 0.0, 0.0, 0.0] } else { [0.1, 0.1, 0.12, 1.0] };
                self.ctx.ClearRenderTargetView(&rt, &clear);
            }
            let _ = self.renderer.render(&self.ctx, &rt, egui_ctx, ro);
            unsafe { let _ = self.swap.Present(1, DXGI_PRESENT(0)); }
        }
        let _ = size;
    }
}
