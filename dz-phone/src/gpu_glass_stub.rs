//! Migration stub for the egui 0.35 + winit + egui-directx11 (D3D11) stack.
//!
//! The original `gpu_glass.rs` is a WGSL/eframe-wgpu 0.31 paint callback. In the
//! new stack the liquid-glass optics are drawn by the HLSL layer in the D3D11
//! render loop (see egui-directx11 shaders/glass.hlsl), so this module keeps only
//! the data-shaped API that the rest of the app references, with inert bodies.
//! This lets the UI code compile against egui 0.35 before the render layer is
//! fully wired up.

use eframe::egui::Rect;

pub const MAX_CTL: usize = 13;

#[derive(Clone, Copy)]
pub struct Ctl {
    pub rect: Rect,
    pub radius: f32,
    pub mag: f32,
    pub refract: f32,
}
impl Default for Ctl {
    fn default() -> Self { Self { rect: Rect::ZERO, radius: 0.0, mag: 1.0, refract: 1.0 } }
}

/// Opaque frame descriptor; the real HLSL path builds its own uniform buffer.
#[derive(Clone, Copy)]
pub struct GlassFrame {
    pub body: Rect,
    pub capture: Rect,
}
impl Default for GlassFrame {
    fn default() -> Self { Self { body: Rect::ZERO, capture: Rect::ZERO } }
}

/// Kept for API compatibility; no egui-wgpu callback in the new stack.
pub struct GlassCallback {
    pub frame: GlassFrame,
    pub format: u32, // placeholder for the old wgpu::TextureFormat
}

pub const NEUTRAL_TINT: [f32; 3] = [0.62, 0.66, 0.74];

pub fn background_luma() -> f32 { 0.5 }
pub fn seed_theme_luma(_v: f32) {}
pub fn gpu_ready() -> bool { true }
pub fn set_morph_scale(_v: f32) {}
pub fn set_liquid_optics(_refract_px: f32, _chroma_px: f32) {}
pub fn set_liquid_look(_blur: f32, _band: f32, _edge: f32, _op: f32) {}
pub fn set_direct_composition(_active: bool) {}

pub fn frame_for(
    body: Rect,
    capture: Rect,
    _screen_px: (u32, u32),
    _ppp: f32,
    _radius: f32,
    _morph: f32,
    _tint: [f32; 3],
    _time: f32,
    _pointer: (f32, f32),
    _origin: (f32, f32),
    _lightness: f32,
    _light_follow: f32,
    _controls: &[Ctl],
    _notch: Option<(f32, f32)>,
) -> GlassFrame {
    GlassFrame { body, capture }
}
