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

/// 背景亮度：由 app 启动时从壁纸文件播种（seed_theme_luma），供 liquid_k 决定深/浅材质。
/// 迁移期不再从 wgpu 抓屏帧读；新 D3D11 栈后续可在此接入 WGC 帧亮度。
static BG_LUMA: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f000000); // 0.5
pub fn background_luma() -> f32 {
    if let Some(v) = std::env::var("DAOZAI_LIQUID_FORCE_LUMA").ok().and_then(|s| s.parse::<f32>().ok()) {
        return v.clamp(0.0, 1.0);
    }
    f32::from_bits(BG_LUMA.load(std::sync::atomic::Ordering::Relaxed))
}
pub fn seed_theme_luma(v: f32) {
    BG_LUMA.store(v.clamp(0.0, 1.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
}
pub fn gpu_ready() -> bool { true }
pub fn set_morph_scale(v: f32) {
    if let Ok(mut u) = UNIFORMS.lock() { u.morph = v; }
}
pub fn set_liquid_optics(refract_px: f32, chroma_px: f32) {
    if let Ok(mut u) = UNIFORMS.lock() { u.refract = refract_px; u.chroma = chroma_px; }
}
pub fn set_liquid_look(blur: f32, band: f32, edge: f32, op: f32) {
    if let Ok(mut u) = UNIFORMS.lock() { u.blur = blur; u.band = band; u.edge_w = edge; u.edge_op = op; }
}
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

// ---------------------------------------------------------------------------
// D3D11 HLSL 桥接：app 每帧把岛体几何/材质参数写到这里，渲染循环读取生成 uniform。
// ---------------------------------------------------------------------------

/// 从 egui 点坐标转到 HLSL 需要的物理像素 uniform。
#[derive(Clone, Copy, Default)]
pub struct GlassUniforms {
    /// 岛体 rect（egui 点，相对窗口左上）
    pub body_rect: [f32; 4],
    pub radius: f32,
    pub chroma: f32,
    pub refract: f32,
    pub morph: f32,
    pub material: f32,
    pub blur: f32,
    pub band: f32,
    pub edge_w: f32,
    pub edge_op: f32,
    pub pointer: [f32; 2],
    pub light_follow: f32,
    pub active: bool,
}

static UNIFORMS: std::sync::Mutex<GlassUniforms> = std::sync::Mutex::new(GlassUniforms {
    body_rect: [0.0; 4], radius: 0.0, chroma: 0.0, refract: 0.0, morph: 0.0, material: 0.0,
    blur: 0.0, band: 0.0, edge_w: 0.0, edge_op: 0.0, pointer: [0.0; 2],
    light_follow: 0.0, active: false,
});

pub fn uniforms() -> GlassUniforms {
    UNIFORMS.lock().map(|u| *u).unwrap_or_default()
}

/// app 的 paint_gpu_glass 每帧写入岛体真实几何/指针/材质。
pub fn set_body(rect: Rect, radius: f32, pointer: (f32, f32), material: f32) {
    if let Ok(mut u) = UNIFORMS.lock() {
        u.body_rect = [rect.left(), rect.top(), rect.width(), rect.height()];
        u.radius = radius;
        u.pointer = [pointer.0, pointer.1];
        u.material = material;
        u.active = true;
    }
}

/// 顶栏/刘海形状用的肩部参数（-1 表示胶囊/圆角矩形）。
static SHAPE_PARAMS: std::sync::Mutex<[f32; 2]> = std::sync::Mutex::new([0.0; 2]);
pub fn set_shape(flare: f32, flare_h: f32) {
    if let Ok(mut s) = SHAPE_PARAMS.lock() { *s = [flare, flare_h]; }
}
pub fn shape_params() -> [f32; 2] {
    SHAPE_PARAMS.lock().map(|s| *s).unwrap_or([0.0; 2])
}
