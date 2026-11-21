//! Minimal Windows Graphics Capture wrapper for the dao zai D3D11 spike.
//!
//! Uses polling (TryGetNextFrame) — no async event handler needed.
//! One D3D11 device drives both the swapchain and the capture frame pool, so
//! CopyResource stays on the GPU: no cross-API sharing, no CPU readback.

use windows::Win32::{
    Foundation::HWND,
    Graphics::{
        Direct3D11::{ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D},
        Dxgi::IDXGIDevice,
    },
};
use windows::core::Interface;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::factory;

pub struct WgcCapture {
    pool: Direct3D11CaptureFramePool,
    _session: GraphicsCaptureSession, // keep alive; dropping stops capture
    width: u32,
    height: u32,
    diag_done: std::cell::Cell<bool>,
}

impl WgcCapture {
    /// Capture the primary monitor. `device` should be the same device that
    /// drives the swapchain so CopyResource is a pure GPU-side operation.
    pub fn new(device: &ID3D11Device) -> windows::core::Result<Self> {
        // WGC/WinRT requires a COM apartment. The app main thread may not have
        // one; initialize MTA (idempotent — RPC_E_CHANGED_MODE is harmless).
        let _ = unsafe { windows::Win32::System::WinRT::RoInitialize(
            windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
        ) };

        // WinRT needs an IDirect3DDevice wrapping our DXGI device (same GPU).
        let dxgi: IDXGIDevice = device.cast()?;
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }?;
        let d3d_device: IDirect3DDevice = inspectable.cast()?;

        // Create a capture item for the primary monitor via the interop factory.
        let interop: IGraphicsCaptureItemInterop =
            factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let primary = unsafe { windows::Win32::Graphics::Gdi::MonitorFromWindow(
            HWND(std::ptr::null_mut()),
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTOPRIMARY,
        ) };
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(primary)? };

        // Use item.Size() (not monitor rect) per Microsoft guidance.
        let size = item.Size()?;
        let width = size.Width.max(1) as u32;
        let height = size.Height.max(1) as u32;

        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &d3d_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            SizeInt32 { Width: width as i32, Height: height as i32 },
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        session.StartCapture()?;
        // Best-effort: hide the capture border (Windows 11 yellow frame).
        let _ = session.SetIsBorderRequired(false);

        Ok(Self { pool, _session: session, width, height,
                  diag_done: std::cell::Cell::new(false) })
    }

    pub fn size(&self) -> (u32, u32) { (self.width, self.height) }

    /// Poll the newest frame and copy it into `dst` (GPU-side, format must be
    /// B8G8R8A8_UNORM). Returns true if a fresh frame was copied.
    pub fn grab_into(
        &self,
        ctx: &ID3D11DeviceContext,
        dst: &ID3D11Texture2D,
    ) -> bool {
        loop {
            let frame = match self.pool.TryGetNextFrame() {
                Ok(f) => f,
                Err(e) => {
                    if !self.diag_done.get() {
                        self.diag_done.set(true);
                        eprintln!("[wgc] TryGetNextFrame error first time: {:?}", e);
                    }
                    return false; // no new frame
                }
            };
            if !self.diag_done.get() {
                self.diag_done.set(true);
                eprintln!("[wgc] got first frame OK");
            }
            let surface = match frame.Surface() {
                Ok(s) => s,
                Err(e) => { if !self.diag_done.get() { self.diag_done.set(true); eprintln!("[wgc] Surface() failed: {:?}", e); } return false; }
            };
            // IDirect3DSurface is NOT directly castable to ID3D11Texture2D;
            // must go through IDirect3DDxgiInterfaceAccess::GetInterface().
            let access: IDirect3DDxgiInterfaceAccess = match surface.cast() {
                Ok(a) => a,
                Err(e) => { if !self.diag_done.get() { self.diag_done.set(true); eprintln!("[wgc] cast access failed: {:?}", e); } return false; }
            };
            let src: ID3D11Texture2D = match unsafe { access.GetInterface() } {
                Ok(t) => t,
                Err(e) => { if !self.diag_done.get() { self.diag_done.set(true); eprintln!("[wgc] GetInterface failed: {:?}", e); } return false; }
            };
            // Frame texture confirmed as 2560x1440 B8G8R8A8 (matches dst).
            #[cfg(debug_assertions)]
            {
                let mut sd = unsafe { std::mem::zeroed::<windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC>() };
                unsafe { src.GetDesc(&mut sd) };
                eprintln!("[wgc] src desc: {}x{} fmt={:?}", sd.Width, sd.Height, sd.Format);
            }
            unsafe { ctx.CopyResource(dst, &src) };
            return true;
        }
    }
}
