//! 液态玻璃 GPU 通道（试验）：在 egui 的同一个 wgpu render pass 里画真光学折射。
//!
//! 光学部分借鉴 [ziuus/liquid-glass](https://github.com/ziuus/liquid-glass)（MIT）的
//! `glass-core/src/renderer/glass.wgsl`：Snell 折射、Cauchy 七波长色散、Schlick Fresnel、
//! C¹ 零斜率透镜剖面、只朝左上的镜面高光与只朝右下的接触阴影。
//! 这里按岛仔的 wgpu 24 / egui-wgpu 0.31 重写了管线与 WGSL，**没有引入任何新依赖**。
//!
//! 前提：岛仔默认跑 glow（OpenGL），只有设置里的「兼容渲染（Direct3D 12）」开启时
//! `RenderState` 才存在。没有 wgpu 设备时 `App` 会回退到 CPU 顶点色皮肤。
//!
//! 背景纹理由 GDI 抓「岛体外扩矩形」得到；岛体矩形内用上边缘像素填充，
//! 避免折射到已经画好的自己（反馈循环）。

use eframe::egui_wgpu::{self, wgpu};
use std::sync::Mutex;

/// 没有封面时的中性淡彩。
pub const NEUTRAL_TINT: [f32; 3] = [0.62, 0.66, 0.74];
/// 背景纹理固定尺寸：展开动画里 capture 每帧都变，重建纹理会闪烁并拖慢合成。
/// shader 里用同样的常量把 uv 映射到纹理左上角的有效子区。
const TEX_W: u32 = 1280;
const TEX_H: u32 = 512;

/// 光学参数与几何，全部按物理像素给。
#[derive(Clone, Copy)]
pub struct GlassFrame {
    /// 抓取矩形在屏幕上的物理坐标 x,y,w,h。
    pub capture: [f32; 4],
    /// 岛体在抓取矩形内的 x,y,w,h。
    pub bounds: [f32; 4],
    /// 圆角半径（像素）、展开程度 0..1、光标局部坐标（像素）。
    pub shape: [f32; 4],
    /// 封面主色淡彩与强度。
    pub tint: [f32; 4],
    /// 时间（秒）、折射强度、色散、亮/暗模式。
    pub optics: [f32; 4],
    /// 抓取矩形的 clip-space 坐标 x0,y0,x1,y1。
    pub clip: [f32; 4],
}

/// 每帧挂到 egui painter 上的回调。
pub struct GlassCallback {
    pub frame: GlassFrame,
    pub format: wgpu::TextureFormat,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    bounds: [f32; 4],
    params: [f32; 4],   // ior, dispersion, time, light
    extra: [f32; 4],    // radius, tex_w, tex_h, expand
    tint: [f32; 4],
    quad: [f32; 4],
    misc: [f32; 4],     // pointer_x, pointer_y, refraction, _
}

impl Uniforms {
    fn bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const Self as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }
}

struct GlassGpu {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    texture: Option<wgpu::Texture>,
    bind_group: Option<wgpu::BindGroup>,
    size: (u32, u32),
}

static GPU: Mutex<Option<GlassGpu>> = Mutex::new(None);
static FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static GRAB_N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// GPU 通道已经成功跑起来：此时兜底底色可以几乎透明，让折射层自己说话。
pub fn gpu_ready() -> bool {
    !FAILED.load(std::sync::atomic::Ordering::Relaxed)
        && TICK.load(std::sync::atomic::Ordering::Relaxed) > 0
}

fn diag(message: &str) {
    use std::io::Write;
    let dir = crate::model::config_dir().join("diagnostics");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("liquid-glass.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) { let _ = std::fs::write(&path, b""); }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{}", message);
    }
}

const SHADER: &str = r#"
struct U {
    bounds : vec4<f32>,
    params : vec4<f32>,
    extra  : vec4<f32>,
    tint   : vec4<f32>,
    quad   : vec4<f32>,
    misc   : vec4<f32>,
}
@group(0) @binding(0) var<uniform> u : U;
@group(0) @binding(1) var bg : texture_2d<f32>;
@group(0) @binding(2) var samp : sampler;

struct VOut {
    @builtin(position) pos : vec4<f32>,
    @location(0) uv : vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) i : u32) -> VOut {
    var xy = array<vec2<f32>, 6>(
        vec2<f32>(u.quad.x, u.quad.y), vec2<f32>(u.quad.z, u.quad.y), vec2<f32>(u.quad.x, u.quad.w),
        vec2<f32>(u.quad.x, u.quad.w), vec2<f32>(u.quad.z, u.quad.y), vec2<f32>(u.quad.z, u.quad.w)
    );
    var uv = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0)
    );
    var o = VOut(vec4<f32>(0.0), vec2<f32>(0.0));
    o.pos = vec4<f32>(xy[i], 0.0, 1.0);
    o.uv = uv[i];
    return o;
}

fn rr_sdf(p : vec2<f32>, b : vec2<f32>, r : f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

fn lens(t : f32) -> f32 {
    // C¹ 零斜率剖面：两端导数为 0，边缘不会出现折角。
    let x = clamp(t, 0.0, 1.0);
    return 1.0 - x * x * (3.0 - 2.0 * x);
}

fn bg_at(uv : vec2<f32>) -> vec3<f32> {
    // uv 是相对 capture 的 0..1；纹理是固定的 1280x512，只用了左上角 (tw,th)。
    let scale = vec2<f32>(u.extra.y / 1280.0, u.extra.z / 512.0);
    let tuv = clamp(uv, vec2<f32>(0.001), vec2<f32>(0.999)) * scale;
    // 不再人为压暗：兼容渲染下窗口底是黑的，玻璃颜色必须等于桌面才会「透」。
    return textureSampleLevel(bg, samp, tuv, 0.0).rgb;
}

fn bg_blur(uv : vec2<f32>) -> vec3<f32> {
    // 9 点大半径模糊：浅色材质要把桌面上的文字/窗口边界抹成柔和的磨砂，
    // 否则玻璃太透，背景文字会和岛上的深色文字叠在一起。
    let px = vec2<f32>(11.0 / u.extra.y, 11.0 / u.extra.z);
    return (bg_at(uv) * 2.0
        + bg_at(uv + vec2<f32>(px.x, 0.0)) + bg_at(uv - vec2<f32>(px.x, 0.0))
        + bg_at(uv + vec2<f32>(0.0, px.y)) + bg_at(uv - vec2<f32>(0.0, px.y))
        + bg_at(uv + px) + bg_at(uv - px)
        + bg_at(uv + vec2<f32>(px.x, -px.y)) + bg_at(uv + vec2<f32>(-px.x, px.y))) / 10.0;
}

@fragment
fn fs(i : VOut) -> @location(0) vec4<f32> {
    let tex = vec2<f32>(u.extra.y, u.extra.z);
    let px = i.uv * tex;
    let half_size = u.bounds.zw * 0.5;
    let local = px - u.bounds.xy - half_size;
    let radius = min(u.extra.x, min(half_size.x, half_size.y));
    let d = rr_sdf(local, half_size, radius);

    // 玻璃外面只留一圈 7px 定向接触阴影（朝右下，光源在左上）。
    let outward = max(d, 0.0);
    let shadow_dir = dot(normalize(local + vec2<f32>(1e-4)), vec2<f32>(0.58, 0.81));
    let contact = (1.0 - smoothstep(0.0, 7.0, outward)) * max(shadow_dir, 0.0)
                  * select(0.05, 0.09, u.params.w > 0.5) * step(0.0, d);
    let aa = 1.0 - smoothstep(-0.7, 0.7, d);

    // SDF 梯度就是曲面法线。
    let e = 1.0;
    let gx = rr_sdf(local + vec2<f32>(e, 0.0), half_size, radius) - rr_sdf(local - vec2<f32>(e, 0.0), half_size, radius);
    let gy = rr_sdf(local + vec2<f32>(0.0, e), half_size, radius) - rr_sdf(local - vec2<f32>(0.0, e), half_size, radius);
    let n = normalize(vec2<f32>(gx, gy) + vec2<f32>(1e-5));

    let bevel = max(radius * 0.55, 9.0);
    let profile = lens((-d) / bevel);
    let rim = 1.0 - smoothstep(0.0, bevel, -d);

    // Snell 折射：按 IOR 与透镜厚度把背景采样 UV 推开。
    let eta = (u.params.x - 1.0) * u.misc.z * profile;
    let refr = n * eta * 16.0;
    let base_uv = i.uv + refr / tex;

    // Cauchy 色散：7 个波长各自错开一点，模拟 RGB 边缘分离。
    let disp = clamp(u.params.y, 0.02, 0.30) * rim;
    var w = array<f32, 7>(0.06, 0.09, 0.14, 0.22, 0.20, 0.17, 0.12);
    var col = vec3<f32>(0.0);
    for (var k = 0; k < 7; k = k + 1) {
        // 各通道权重相同，色差只来自采样点错开；否则整块玻璃会被染成偏绿。
        let shift = (f32(k) - 3.0) / 3.0 * disp * 26.0 * n;
        col = col + bg_at(base_uv + shift / tex) * w[k];
    }

    // 浅色液态玻璃 = 磨砂底图（强模糊 + 一点白雾）+ 少量清晰折射。
    // 桌面颜色/明暗仍然透出来，但文字与窗口边界被抹掉，深色字才不会和背景字打架。
    let frost = mix(bg_blur(base_uv), vec3<f32>(0.96, 0.96, 0.975), vec3<f32>(0.34));
    col = mix(col, frost, vec3<f32>(0.78));

    // 玻璃本体：压一点亮度保证白字可读，亮背景压得更深。
    let light = u.params.w;
    let body = mix(1.0, 0.94, light);
    col = mix(col * body, col, vec3<f32>(0.35));
    col = col + u.tint.rgb * u.tint.a * 0.10 * rim;

    // Schlick Fresnel：掠射角更亮。
    let grazing = clamp(1.0 - (1.0 - abs(n.x)), 0.0, 1.0);
    let fresnel = 0.04 + 0.96 * pow(grazing, 5.0);
    col = col + vec3<f32>(fresnel * 0.28 * rim);

    // 只朝左上的 1px 级镜面高光，背光侧不给白边。
    let L = normalize(vec2<f32>(-0.45, -0.65));
    let ndl = max(dot(n, L), 0.0);
    let glint = pow(ndl, 5.0) * pow(rim, 1.3);
    col = col + vec3<f32>(glint * 1.00);

    // 光标追光：沿边缘把高光带过去。
    let gd = length(px - u.bounds.xy - u.misc.xy) / 90.0;
    let glow = exp(-gd * gd * 1.6);
    col = col + vec3<f32>(0.16, 0.20, 0.26) * glow * rim;

    // 底部内缘压暗，做出厚度。
    let vy = (px.y - u.bounds.y) / max(u.bounds.w, 1.0);
    col = col * (1.0 - 0.10 * pow(vy, 2.0) * rim);

    // 岛体外面只留朝右下的接触阴影：颜色必须压暗，
    // 否则 contact 这段 alpha 会带着浅色背景画成一圈亮描边。
    let outside = smoothstep(0.0, 3.0, d);
    col = mix(col, vec3<f32>(0.055, 0.065, 0.075), outside * 0.80);

    // 兼容渲染（DX12）用 SetWindowRgn 把窗口裁成岛体轮廓，窗口底清成不透明黑，
    // 逐像素透明不可用。预乘混合下最终颜色就是这里输出的颜色，所以岛体内部
    // 必须 alpha=1 且颜色等于抓到的桌面，看起来才是「透明」而不是一块灰板。
    // 折射/色散/高光叠在这张真实底图上，只有抗锯齿的 1px 边缘留半透明。
    let alpha = clamp(aa + contact, 0.0, 1.0);
    return vec4<f32>(col * alpha, alpha);
}
"#;

fn make_gpu(device: &wgpu::Device, format: wgpu::TextureFormat) -> Option<GlassGpu> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| make_gpu_inner(device, format))).ok()
}

fn make_gpu_inner(device: &wgpu::Device, format: wgpu::TextureFormat) -> GlassGpu {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("daozai-liquid-glass"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("daozai-glass-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("daozai-glass-pipeline-layout"),
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    // 与 egui-wgpu 的预乘 alpha 混合保持一致。
    let blend = wgpu::BlendState {
        color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
        alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
    };
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("daozai-glass-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("daozai-glass-uniform"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    GlassGpu { pipeline, layout, uniform, sampler, texture: None, bind_group: None, size: (0, 0) }
}

fn uniforms(f: &GlassFrame, tex: (u32, u32)) -> Uniforms {
    Uniforms {
        bounds: f.bounds,
        params: [1.52, f.optics[2].max(0.08), f.optics[0], f.optics[3]],
        extra: [f.shape[0], tex.0 as f32, tex.1 as f32, f.shape[1]],
        tint: f.tint,
        quad: f.clip,
        misc: [f.shape[2], f.shape[3], f.optics[1], 0.0],
    }
}

/// 首帧岛仔还没画出来时抓的「干净桌面底片」。
/// 之后每帧只把岛体以外的像素写回底片，岛体内部一直用这张底片：
/// 折射能采到真实桌面细节，又不会把上一帧的自己（文字、控件）抓进来产生鬼影。
#[cfg(windows)]
const PLATE_W: u32 = 1024;
#[cfg(windows)]
const PLATE_H: u32 = 512;
/// 底片锚点比首帧抓屏矩形左移 256px：展开时抓屏矩形会向左变宽，
/// 锚点固定才不会在动画里重建底片（重建时岛仔已可见，会被抹成灰渐变）。
#[cfg(windows)]
const PLATE_ANCHOR: i32 = 256;

#[cfg(windows)]
struct CleanPlate {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    data: Vec<u8>,
}

#[cfg(windows)]
static PLATE: Mutex<Option<CleanPlate>> = Mutex::new(None);

#[cfg(windows)]
fn row_mean(buf: &[u8], w: u32, h: u32, x0: i32, x1: i32, y: i32) -> [f32; 3] {
    let yy = y.clamp(0, h as i32 - 1) as u32;
    let a = x0.max(0) as u32;
    let b = (x1.min(w as i32)).max(a as i32) as u32;
    let (mut r, mut g, mut bl, mut n) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for x in a..b {
        let i = ((yy * w + x) * 4) as usize;
        r += buf[i] as f32; g += buf[i + 1] as f32; bl += buf[i + 2] as f32; n += 1.0;
    }
    if n > 0.0 { [r / n, g / n, bl / n] } else { [238.0, 238.0, 238.0] }
}

/// 只在「首帧就已经能看到岛仔」时兜底：把岛体区域抹成上下边缘平均色的渐变。
#[cfg(windows)]
fn fill_gradient(buf: &mut [u8], w: u32, x0: i32, y0: i32, x1: i32, y1: i32, top: [f32; 3], bot: [f32; 3]) {
    if x1 <= x0 || y1 <= y0 { return; }
    let hh = (y1 - y0) as f32;
    for yy in y0..y1 {
        let k = (yy - y0) as f32 / hh;
        for xx in x0..x1 {
            let i = ((yy as u32 * w + xx as u32) * 4) as usize;
            buf[i] = (top[0] + (bot[0] - top[0]) * k) as u8;
            buf[i + 1] = (top[1] + (bot[1] - top[1]) * k) as u8;
            buf[i + 2] = (top[2] + (bot[2] - top[2]) * k) as u8;
            buf[i + 3] = 255;
        }
    }
}

/// 纯 GDI 抓屏，返回自上而下的 RGBA；不做任何岛体处理。
#[cfg(windows)]
fn grab_raw(x: i32, y: i32, w: i32, h: i32) -> Option<(Vec<u8>, u32, u32)> {
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::Graphics::Gdi::HGDIOBJ;
    if w <= 0 || h <= 0 || w > 2048 || h > 1024 { return None; }
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() { return None; }
        let mem = CreateCompatibleDC(screen);
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // 负数 = 自上而下
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB.0;
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(mem, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let old_bmp = SelectObject(mem, HGDIOBJ(bmp.0));
        let ok = BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY).is_ok();
        let (bw, bh) = (w as u32, h as u32);
        let mut rgba = vec![0u8; (bw * bh * 4) as usize];
        if ok && !bits.is_null() {
            let src = std::slice::from_raw_parts(bits as *const u8, rgba.len());
            for i in (0..rgba.len()).step_by(4) {
                rgba[i] = src[i + 2];
                rgba[i + 1] = src[i + 1];
                rgba[i + 2] = src[i];
                rgba[i + 3] = 255;
            }
        }
        SelectObject(mem, old_bmp);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        if ok { Some((rgba, bw, bh)) } else { None }
    }
}

/// 抓当前岛体外扩矩形，但与「干净底片」合成：岛体内部用底片（真实桌面），
/// 岛体外部用本帧抓屏并顺手写回底片，让底片跟着桌面变化更新。
#[cfg(windows)]
fn capture(x: i32, y: i32, w: i32, h: i32, body: [f32; 4]) -> Option<(Vec<u8>, u32, u32)> {
    if w <= 0 || h <= 0 || w > 2048 || h > 1024 { return None; }
    let (raw, bw, bh) = grab_raw(x, y, w, h)?;
    let mut guard = PLATE.lock().unwrap_or_else(|e| e.into_inner());
    // 只有抓屏矩形跑出底片范围才重建；展开动画里 x 左移 ~100px 属于正常范围。
    let need_new = match guard.as_ref() {
        None => true,
        Some(p) => {
            let (ox, oy) = (x - p.x, y - p.y);
            ox < 0 || oy < 0 || ox + w > p.w as i32 || oy + h > p.h as i32
        }
    };
    if need_new {
        let ax = x - PLATE_ANCHOR;
        let big = grab_raw(ax, y, PLATE_W as i32, PLATE_H as i32);
        let (mut data, pw, ph, fpx, fpy) = if let Some((d, pw, ph)) = big {
            (d, pw, ph, ax, y)
        } else {
            (raw.clone(), bw, bh, x, y)
        };
        let (sx, sy) = (x - fpx, y - fpy);
        let (bx0, by0, bx1, by1) = (
            (body[0] + sx as f32 - 10.0).max(0.0) as i32,
            (body[1] + sy as f32 - 10.0).max(0.0) as i32,
            ((body[0] + sx as f32 + body[2] + 10.0).min(pw as f32)) as i32,
            ((body[1] + sy as f32 + body[3] + 10.0).min(ph as f32)) as i32,
        );
        let top = row_mean(&data, pw, ph, bx0, bx1, by0 - 6);
        let bot = row_mean(&data, pw, ph, bx0, bx1, by1 + 6);
        // 首帧通常岛仔还没画出来；万一已经画出来了，中心会明显比左侧桌面暗，
        // 这时把岛体区域抹成渐变，避免底片里存进自己的深色玻璃。
        let cx = ((bx0 + bx1) / 2).clamp(0, pw as i32 - 1) as u32;
        let cy = ((by0 + by1) / 2).clamp(0, ph as i32 - 1) as u32;
        let ci = ((cy * pw + cx) * 4) as usize;
        let li = ((cy * pw + bx0.clamp(0, pw as i32 - 1) as u32) * 4) as usize;
        let bright = |i: usize| (data[i] as i32 + data[i + 1] as i32 + data[i + 2] as i32) / 3;
        let (cb, lb) = (bright(ci), bright(li));
        let feedback = lb - cb > 40;
        if feedback { fill_gradient(&mut data, pw, bx0, by0, bx1, by1, top, bot); }
        diag(&format!("plate: new {}x{} at ({},{}) feedback={} center={} desktop={}",
                      pw, ph, fpx, fpy, feedback, cb, lb));
        *guard = Some(CleanPlate { x: fpx, y: fpy, w: pw, h: ph, data });
    }
    let p = guard.as_mut().unwrap();
    let (ox, oy) = (x - p.x, y - p.y);
    // 排除区（capture 坐标）：岛体 + 10px，连窗口阴影/描边一起排掉。
    let (cx0, cy0, cx1, cy1) = (
        (body[0] - 10.0).max(0.0) as i32,
        (body[1] - 10.0).max(0.0) as i32,
        ((body[0] + body[2] + 10.0).min(bw as f32)) as i32,
        ((body[1] + body[3] + 10.0).min(bh as f32)) as i32,
    );
    if GRAB_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 120 == 0 {
        diag(&format!("plate: cap={}x{} off=({},{}) body=({},{},{},{})",
                      bw, bh, ox, oy, cx0, cy0, cx1, cy1));
    }
    let mut out = vec![0u8; (bw * bh * 4) as usize];
    for yy in 0..bh as i32 {
        for xx in 0..bw as i32 {
            let d = ((yy as u32 * bw + xx as u32) * 4) as usize;
            let (qx, qy) = (ox + xx, oy + yy);
            let in_plate = qx >= 0 && qy >= 0 && qx < p.w as i32 && qy < p.h as i32;
            let excluded = xx >= cx0 && xx < cx1 && yy >= cy0 && yy < cy1;
            if excluded && in_plate {
                let s = ((qy as u32 * p.w + qx as u32) * 4) as usize;
                out[d..d + 4].copy_from_slice(&p.data[s..s + 4]);
            } else {
                out[d..d + 4].copy_from_slice(&raw[d..d + 4]);
                if in_plate {
                    let s = ((qy as u32 * p.w + qx as u32) * 4) as usize;
                    p.data[s..s + 4].copy_from_slice(&raw[d..d + 4]);
                }
            }
        }
    }
    Some((out, bw, bh))
}

#[cfg(not(windows))]
fn capture(_x: i32, _y: i32, w: i32, h: i32, _body: [f32; 4]) -> Option<(Vec<u8>, u32, u32)> {
    // 非 Windows 只给一张渐变，保证模块能编译；真机效果以 Windows 为准。
    let (w, h) = (w.max(1) as u32, h.max(1) as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            rgba[i] = (40.0 + x as f32 / w as f32 * 90.0) as u8;
            rgba[i + 1] = (70.0 + y as f32 / h as f32 * 60.0) as u8;
            rgba[i + 2] = 150;
            rgba[i + 3] = 255;
        }
    }
    Some((rgba, w, h))
}

impl egui_wgpu::CallbackTrait for GlassCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let mut guard = GPU.lock().unwrap_or_else(|e| e.into_inner());
        let tick = TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if FAILED.load(std::sync::atomic::Ordering::Relaxed) { return Vec::new(); }
        if guard.is_none() {
            match make_gpu(device, self.format) {
                Some(gpu) => { *guard = Some(gpu); diag("make_gpu: OK"); }
                None => {
                    FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
                    diag("make_gpu: FAILED (shader/pipeline 创建抛错)，回退纯色");
                    return Vec::new();
                }
            }
        }
        let gpu = guard.as_mut().unwrap();
        let f = &self.frame;
        let (cw, ch) = (f.capture[2].max(1.0) as i32, f.capture[3].max(1.0) as i32);
        let mut tex_size = gpu.size;
        // 每 2 帧抓一次屏：GDI BitBlt 与 CPU 填充是这段最重的部分。
        if tick % 2 == 0 || gpu.texture.is_none() {
            let Some((rgba, tw, th)) = capture(f.capture[0] as i32, f.capture[1] as i32, cw, ch, f.bounds) else {
                if tick == 0 { diag("capture: GDI 抓屏失败"); }
                return Vec::new();
            };
            if tick % 120 == 0 {
                let mut sum = 0u64;
                let mut n = 0u64;
                let mut i = 0usize;
                while i + 4 <= rgba.len() { sum += rgba[i] as u64; n += 1; i += 4004; }
                let mean = if n > 0 { sum / n } else { 0 };
                diag(&format!("tick={tick} cap={tw}x{th} mean_r={mean} tex={:?} clip={:?} bounds={:?}", gpu.size, f.clip, f.bounds));
            }
            if gpu.texture.is_none() {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("daozai-glass-backdrop"),
                    size: wgpu::Extent3d { width: TEX_W, height: TEX_H, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("daozai-glass-bind"),
                    layout: &gpu.layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: gpu.uniform.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&gpu.sampler) },
                    ],
                }));
                gpu.texture = Some(texture);
            }
            gpu.size = (tw, th);
            // wgpu 要求 bytes_per_row 是 256 的倍数；不对齐会触发 validation error
            // （纹理保持未初始化 → 整块黑），所以每行补 padding 再上传。
            if tw > TEX_W || th > TEX_H { return Vec::new(); }
            let bpr = (tw * 4 + 255) / 256 * 256;
            let mut padded = vec![0u8; bpr as usize * th as usize];
            let row = (tw * 4) as usize;
            for y in 0..th as usize {
                let (s, d) = (y * row, y * bpr as usize);
                padded[d..d + row].copy_from_slice(&rgba[s..s + row]);
            }
            if tick == 0 { diag(&format!("upload: bpr={bpr} (raw={}) rows={th}", tw * 4)); }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: gpu.texture.as_ref().unwrap(), mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &padded,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(th) },
                wgpu::Extent3d { width: tw, height: th, depth_or_array_layers: 1 },
            );
            tex_size = (tw, th);
        } else if gpu.size == (0, 0) {
            return Vec::new();
        } else {
            tex_size = gpu.size;
        }
        let u = uniforms(f, tex_size);
        queue.write_buffer(&gpu.uniform, 0, u.bytes());
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::epaint::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _resources: &egui_wgpu::CallbackResources,
    ) {
        let guard = GPU.lock().unwrap_or_else(|e| e.into_inner());
        let Some(gpu) = guard.as_ref() else { return; };
        let Some(bind) = gpu.bind_group.as_ref() else { return; };
        let tick = TICK.load(std::sync::atomic::Ordering::Relaxed);
        if tick == 1 { diag("paint: pipeline/bind_group 已就绪"); }
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// 把逻辑点矩形换成屏幕物理像素与 clip-space 坐标。
pub fn frame_for(
    body: eframe::egui::Rect,
    capture: eframe::egui::Rect,
    screen_px: (u32, u32),
    ppp: f32,
    radius: f32,
    expand: f32,
    tint: [f32; 3],
    time: f32,
    pointer: (f32, f32),
    origin: (f32, f32),
) -> GlassFrame {
    // 抓屏用屏幕绝对物理坐标，clip-space / bounds 用窗口内坐标。
    let c = ((capture.min.x + origin.0) * ppp, (capture.min.y + origin.1) * ppp);
    let cw = capture.width() * ppp;
    let ch = capture.height() * ppp;
    let bounds = [
        (body.min.x - capture.min.x) * ppp,
        (body.min.y - capture.min.y) * ppp,
        body.width() * ppp,
        body.height() * ppp,
    ];
    // egui-wgpu 在调用回调前已把 render pass 的 viewport 设成 callback.rect，
    // 所以 clip-space 只需铺满 [-1,1]；uv 0..1 正好对应抓到的那块纹理。
    let _ = screen_px;
    GlassFrame {
        capture: [c.0, c.1, cw, ch],
        bounds,
        shape: [radius * ppp, expand, (pointer.0 - capture.min.x) * ppp, (pointer.1 - capture.min.y) * ppp],
        tint: [tint[0], tint[1], tint[2], 1.0],
        optics: [time, 1.0, 0.20, 0.0],
        clip: [-1.0, 1.0, 1.0, -1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_maps_rects_to_pixels_and_clip_space() {
        use eframe::egui::{Pos2, Rect};
        let body = Rect::from_min_size(Pos2::new(100.0, 20.0), eframe::egui::vec2(200.0, 30.0));
        let capture = Rect::from_min_size(Pos2::new(60.0, 0.0), eframe::egui::vec2(280.0, 90.0));
        let f = frame_for(body, capture, (1920, 1080), 1.0, 15.0, 0.0, [0.6, 0.7, 0.8], 1.0, (150.0, 30.0), (0.0, 0.0));
        assert_eq!(f.bounds, [40.0, 20.0, 200.0, 30.0]);
        assert_eq!(f.clip, [-1.0, 1.0, 1.0, -1.0], "viewport 已由 egui 设为 callback 矩形");
        assert_eq!(uniforms(&f, (280, 90)).params[0], 1.52);
    }

    #[test]
    fn uniforms_are_packed_without_padding() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 96);
    }
}
