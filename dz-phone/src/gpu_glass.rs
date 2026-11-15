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
/// 抓到的桌面平均亮度（0..1）的 f32 位模式，供 App 决定深/浅材质。
static BG_LUMA: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f59_999a); // 0.85，未抓屏前默认浅色
/// DWM 捕获排除是否已经尝试过 / 成功设置了几个窗口。
static EXCLUSION_TRIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static EXCLUSION_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// 冻结诊断模式的预热帧数：前 24 帧保持排除以收集干净壁纸，之后取消排除、冻结。
static FREEZE_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static EXCLUDED_N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// 桌面平均亮度。非 Windows / 未抓屏时给一个偏浅的默认值。
pub fn background_luma() -> f32 {
    let forced = std::env::var("DAOZAI_LIQUID_FORCE_LUMA").ok()
        .and_then(|v| v.parse::<f32>().ok()).map(|v| v.clamp(0.0, 1.0));
    if let Some(v) = forced { return v; }
    f32::from_bits(BG_LUMA.load(std::sync::atomic::Ordering::Relaxed))
}


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

// 逐像素伪随机（0..1）。只用于抖动，不需要高质量分布。
fn hash21(p : vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q = q + vec2<f32>(dot(q, q.yx + vec2<f32>(33.33)));
    return fract((q.x + q.y) * q.x);
}

// 纹理固定 1280x512，只写左上角 (tw,th)；px 是 capture 内的物理像素坐标。
fn sample_px(px : vec2<f32>) -> vec3<f32> {
    // FLAT=2：用棋盘格当背景，折射位移、色散、越界都会直接显形。
    if (u.misc.w > 1.5) {
        // 20px 网格 + 1.6px 抗锯齿细线：硬棋盘本身的格子边缘也是一级级像素，
        // 很容易被当成「玻璃锯齿」，换成带 AA 的网格线就不会混淆。
        let q = px / 20.0;
        let dl = min(abs(fract(q.x) - 0.5), abs(fract(q.y) - 0.5)) * 20.0;
        let ln = 1.0 - smoothstep(0.6, 1.8, dl);
        return mix(vec3<f32>(0.80, 0.84, 0.90), vec3<f32>(0.18, 0.22, 0.30), ln);
    }
    let tex = vec2<f32>(u.extra.y, u.extra.z);
    let scale = vec2<f32>(u.extra.y / 1280.0, u.extra.z / 512.0);
    let uv = clamp(px / tex, vec2<f32>(0.001), vec2<f32>(0.999)) * scale;
    return textureSampleLevel(bg, samp, uv, 0.0).rgb;
}

// 分通道 5 点十字平均：R 采一侧、B 采另一侧，颜色差就是色散彩边。
fn tap5(px : vec2<f32>, ca : vec2<f32>, blur : f32) -> vec3<f32> {
    var off = array<vec2<f32>, 5>(
        vec2<f32>(0.0, 0.0), vec2<f32>(blur, 0.0), vec2<f32>(-blur, 0.0),
        vec2<f32>(0.0, blur), vec2<f32>(0.0, -blur)
    );
    var s = vec3<f32>(0.0);
    for (var i : i32 = 0; i < 5; i = i + 1) {
        let q = px + off[i];
        s = s + vec3<f32>(sample_px(q + ca).r, sample_px(q).g, sample_px(q - ca).b);
    }
    return s / 5.0;
}

@fragment
fn fs(i : VOut) -> @location(0) vec4<f32> {
    let tex = vec2<f32>(u.extra.y, u.extra.z);
    let p = i.uv * tex;
    // 形状整体内缩 1.2px：SetWindowRgn 的裁剪是每像素硬边、没有抗锯齿，
    // 玻璃边缘正好压在区域边界上时，半透明像素会被切掉 → 阶梯锯齿。
    // 内缩后抗锯齿完整落在窗口区域内，区域只裁到已经完全透明的部分。
    // inset 必须大于「白边半宽(1.6) + aa 半宽(0.8)」，否则白边外半部分被区域裁掉，
    // 边缘会变成 1px 硬跳变（实测 y:43 → 181，跨 138）—— 这就是看到的锯齿。
    let inset = 3.0;
    let half_size = max(u.bounds.zw * 0.5 - vec2<f32>(inset), vec2<f32>(2.0));
    let local = p - u.bounds.xy - u.bounds.zw * 0.5;
    let radius = max(min(u.extra.x - inset, min(half_size.x, half_size.y)), 0.0);
    let d = rr_sdf(local, half_size, radius);
    let mat = clamp(u.params.w, 0.0, 1.0);

    // 岛体外：SetWindowRgn 的窗口区域比岛体大一圈（展开时左右各约 38px），
    // 而兼容渲染的窗口底是不透明黑。这里必须把**背后的桌面**画回去，
    // 否则这圈余量会变成明显的黑边/黑色背景 —— 之前涂近黑就是这么来的。
    if (d > inset - 0.25) {
        let bg = sample_px(p);
        // 紧贴边缘留一点极淡的接触阴影，玻璃才不像凭空浮着。
        let sh = (1.0 - smoothstep(0.0, 7.0, d - 1.0)) * mix(0.06, 0.02, mat);
        let col = bg * (1.0 - sh) + vec3<f32>(0.05, 0.06, 0.07) * sh;
        return vec4<f32>(col, 1.0);
    }

    // 1.2px 的抗锯齿足够平滑；再宽会在实时模式下露出滞后 1~2 帧的桌面色，
    // 边缘反而显得毛躁。
    let aa = 1.0 - smoothstep(-1.2, 1.2, d);

    // 对齐诊断：DAOZAI_LIQUID_FLAT=1 时玻璃内原样输出抓到的背景。
    // 如果它与岛体外的壁纸无缝衔接，说明抓屏坐标与屏幕一致；错位就说明坐标有偏。
    if (u.misc.w > 0.5) {
        return vec4<f32>(sample_px(p), 1.0);
    }

    // SDF 梯度指向边缘外侧。
    let e = 1.0;
    let gx = rr_sdf(local + vec2<f32>(e, 0.0), half_size, radius) - rr_sdf(local - vec2<f32>(e, 0.0), half_size, radius);
    let gy = rr_sdf(local + vec2<f32>(0.0, e), half_size, radius) - rr_sdf(local - vec2<f32>(0.0, e), half_size, radius);
    let g = normalize(vec2<f32>(gx, gy) + vec2<f32>(1e-6));

    // 折射带：边缘弯得最狠，往里 t 增大、k 迅速衰减。
    // band 上限原来卡在 72 —— 实测展开态面板高 210px 时只有外侧 72px 参与折射，
    // 内侧 138px（66% 的高度）k≈0，等于 1:1 复制。这就是「小胶囊像玻璃、
    // 大面板像半透明贴纸」的根因。上限提到 130，strength 同步放宽到 78。
    // 展开/折叠过渡：形变过程中折射带临时加宽、色散随之增强，玻璃看起来像被
    // 「拉」开再稳住。morph 是「展开进度的变化速度」（0..1，带衰减），
    // 静止时恒为 0，所以折叠态和展开态完全不受影响。
    let morph = clamp(u.extra.w, 0.0, 1.0);
    let band = clamp(u.bounds.w * 0.80, 30.0, 130.0) * (1.0 + morph * 0.55);
    let t = clamp(-d / band, 0.0, 1.0);
    let k = pow(1.0 - pow(t, 0.30), 2.0);

    // 关键修复：折射方向朝玻璃**内部**（-g）。之前朝外，边缘会把岛体外的
    // 壁纸折进来，玻璃里就出现「不属于该区域的壁纸像素块」，贴屏顶时还会
    // 越界 clamp 成一条拉伸色带。
    let strength = clamp(u.bounds.w * 0.90, 26.0, 78.0) * max(u.misc.z, 0.2) * (1.0 + morph * 0.30);
    // 色散：参考实现的默认值是 chroma=12（R/B 采样点相距 24px），但那是给
    // 平滑照片准备的。我们是 1:1 的**锐利桌面抓屏**，24px 分离在树枝这种硬边上
    // 就是一条彩虹故障带 —— 黑白壁纸实测顶边饱和度 0.073，而背景只有 0.003。
    // 第一版收到 4.5px 后圆角仍能看到彩边（p95 还有 0.13），再收一半到 2.6px；
    // 直边的底噪从 0.40 压到 0.22，几乎只剩圆角处一点点色边。
    let cxn = clamp((abs(local.x) - (half_size.x - radius)) / max(radius, 1.0), 0.0, 1.0);
    let cyn = clamp((abs(local.y) - (half_size.y - radius)) / max(radius, 1.0), 0.0, 1.0);
    let corner = cxn * cyn;
    let chroma = clamp(strength * 0.045, 1.2, 2.6) * mix(0.22, 1.0, corner);
    let refr = -g * k * strength;
    let ca = -g * k * chroma;
    let bp = p + refr;

    var col = tap5(bp, ca, 1.5);
    // 玻璃比空气「多一层」，但这点提亮必须极轻。以前这里是四层白色叠加：
    // +0.004 冷色、10% 白雾、顶部冷光、外加一圈白边，浅色壁纸下整体被推成
    // 一片发白的雾 —— 玻璃感就是这么没的（实测岛内亮度只比壁纸高 8，却全糊了）。
    col = col * 1.004 + vec3<f32>(0.002, 0.003, 0.006);

    // 顶部一点冷色提亮，做出玻璃厚度（幅度从 0.18 砍到 0.10）。
    let topk = 1.0 - smoothstep(0.0, max(u.bounds.w, 1.0), p.y - u.bounds.y);
    col = col + vec3<f32>(0.05, 0.07, 0.10) * topk * 0.10;

    // 背后未折射的桌面色，后面算局部 scrim 和边缘混合都要用，先取一次。
    let edge_bg = sample_px(p);

    // 材质：不再无脑「加白」。亮背景下加白会把对比彻底压平 —— 实测背景亮度
    // 250 时白色高光只剩 +5 的余量，玻璃等于不存在（背景 60 时有 +124）。
    // 改成按背景亮度**反相补偿**：
    //   暗背景(<0.42) → 轻微提亮，材质才有存在感
    //   亮背景(>0.88) → 轻微压暗，把高光的对比空间腾出来
    let lum_m = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    let lift = mix(0.045, -0.035, smoothstep(0.42, 0.88, lum_m));
    col = col * (1.0 + lift);

    // 深色材质：保留背景明暗层次，但必须明显比背景沉。0.70 太轻（实测只暗 5.9，
    // 几乎看不出来），0.58 才能让深色玻璃成为「第二层材质」而不是一层蓝雾。
    col = mix(col, col * 0.58 + vec3<f32>(0.010, 0.014, 0.020), vec3<f32>((1.0 - mat) * 0.70));
    // 极淡冷白雾，只用来统一色相，不再承担材质感
    col = mix(col, vec3<f32>(0.96, 0.97, 1.0), vec3<f32>(mat * 0.02));

    // 局部 scrim：决定文字可读性的不是「整块背景的平均亮度」，而是**每个像素
    // 背后**的亮度。这里按像素取未折射的背景亮度，亮的地方多压、暗的地方几乎
    // 不动 —— 把文字背后的明暗落差压小，而不是把整块玻璃统一调暗（统一调暗会
    // 让深色区一起变灰，反而更闷）。只对浅色材质生效：深色材质本来就是白字。
    let bg_l = dot(edge_bg, vec3<f32>(0.299, 0.587, 0.114));
    let scrim = smoothstep(0.48, 0.96, bg_l) * 0.16 * mat;
    col = mix(col, vec3<f32>(0.30), scrim);

    // 白雾会洗掉饱和度，补回来一点，玻璃才有「透过一层介质」的颜色感。
    let lum0 = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    col = mix(vec3<f32>(lum0), col, 1.06);

    // 内侧厚度：紧挨高光下方压一道很薄的暗，玻璃才有厚度，而不是一张贴纸。
    // 位置在内侧 3px、σ≈2.4、只压 14%，再多就显脏。
    let tq = (d + 3.2) / 2.4;
    let thick = exp(-tq * tq);
    col = col * (1.0 - thick * 0.14);

    // 边缘光只给朝光源的一侧，背光侧很弱。
    let rim = 1.0 - smoothstep(0.0, 6.0, -d);
    // 光源方向跟着指针走。桌面没有陀螺仪，指针是唯一能表达「用户正在动」的输入：
    // Apple 的镜面高光随设备倾斜移动，这里把指针当成虚拟光源 —— 指针在哪一侧，
    // 哪一侧的边缘光就亮。指针离岛心很近时退回默认的左上光，避免方向在中心乱跳。
    // （misc.xy 就是指针在 capture 内的物理坐标，原来一直传进来但没人用。）
    let center = u.bounds.xy + u.bounds.zw * 0.5;
    let to_ptr = u.misc.xy - center;
    let plen = length(to_ptr);
    let fallback = normalize(vec2<f32>(-0.45, -0.65));
    let swung = to_ptr / max(plen, 1e-3);
    // 限制在上半圈：指针只控制**水平摆动**，垂直方向永远保持「光从上方来」。
    // 原因是岛仔贴在屏幕顶部，鼠标绝大多数时间在它下面 —— 真跟随的话静止状态
    // 会一直是「从下往上打光」，观感很别扭。人类对光源在上方有本能预期。
    let up_bias = vec2<f32>(swung.x, min(swung.y, -0.18));
    let Ld = normalize(mix(fallback, up_bias, smoothstep(3.0, 48.0, plen)) + vec2<f32>(1e-6));
    col = col + vec3<f32>(0.85, 0.90, 1.0) * rim * pow(max(dot(g, Ld), 0.0), 2.0) * 0.34;
    col = col + vec3<f32>(0.85, 0.90, 1.0) * rim * pow(max(dot(g, -Ld), 0.0), 3.0) * 0.06;
    col = col + u.tint.rgb * u.tint.a * 0.05 * rim;

    // 高光：玻璃质感的命门。必须是**一条锐利的亮线**，不是一圈雾。
    // 之前 exp(-d²/3.61)（σ=1.9，有效宽约 8px）把高光摊成了雾：实测 45px 高的
    // 胶囊上边缘有整整 10px 是白的（255,255,255,252,244,231,220,213…），
    // 上下各 10px 就吃掉 44% 的高度，观感自然从「玻璃」变成「磨砂塑料」。
    // 现在峰值落在边缘内侧 0.7px、σ≈1.15（有效宽约 3px）；整条线都在形状
    // 内部，所以不影响外缘覆盖率，外缘依旧交给 aa，不会退回 1px 硬跳。
    let lq = (d + 0.7) / 1.15;
    let line = exp(-lq * lq);
    let gq = (d + 1.2) / 4.5;
    let glow = exp(-gq * gq) * 0.05;
    // 高光**沿边缘方向调制**：迎光侧最亮，背光侧只留一条淡淡的边。
    // 这一步才是「液态感」的关键 —— 把 line 均匀铺满整圈，看起来只是一圈描边；
    // 让它的强度随光源方向变化，才像一块会反光的玻璃。
    // 实测（指针左/右/下三帧）：只加方向性的 rim 时侧边亮度差只有 3.6，
    // 调制 line 之后同一对比变成 19.6，肉眼才看得出光在动。
    let dirw = mix(0.28, 1.0, pow(max(dot(g, Ld), 0.0), 1.5));
    let line_v = line * dirw;
    // 高光权重也要跟背景走：暗背景下 0.62 就够亮（184 vs 背景 60，很抢眼），
    // 但亮背景下材质被压到 231、背景 250，权重不够高光反而比背景还暗，
    // 会看成一条「暗边」。亮背景拉到 0.92 并用纯白，才顶得到 253 附近。
    let line_w = mix(0.62, 0.92, smoothstep(0.60, 0.92, lum_m));
    col = mix(col, vec3<f32>(1.0), clamp(line_v * line_w + glow, 0.0, 1.0));

    // 关键：边缘必须和「背后的桌面」混合。兼容渲染的窗口不透明，
    // 输出预乘 col*alpha 在 aa→0 时会变成纯黑，岛体轮廓外就是一圈黑线
    // （圆角处最明显）。这里直接 mix 桌面色，边缘永远不会有黑。
    // 覆盖率取 max(aa, line*0.9)：高光自带覆盖率，不能被 aa 归零，
    // 否则白边的外半部分会被 mix(edge_bg, col, 0) 整个丢弃，边缘只剩 1px 硬跳。
    let cov = clamp(max(aa, line_v * 0.90), 0.0, 1.0);
    // 抖动：8-bit 输出在平滑渐变上必然出色带（深色壁纸尤其明显）。加 ±1 级噪声
    // 把量化台阶打散，幅度压在 1/255 量级 —— 只让渐变更顺，看不到噪点。
    let dth = (hash21(p) - 0.5) * (1.7 / 255.0);
    return vec4<f32>(mix(edge_bg, col, vec3<f32>(cov)) + vec3<f32>(dth), 1.0);
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

/// DAOZAI_LIQUID_FLAT=1：玻璃内原样输出背景，用于检查抓屏与屏幕是否对齐。
fn flat_flag() -> f32 {
    static FLAT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let v = FLAT.get_or_init(|| std::env::var("DAOZAI_LIQUID_FLAT").unwrap_or_default());
    return match v.as_str() { "2" => 2.0, "1" => 1.0, _ => 0.0 };
}

fn uniforms(f: &GlassFrame, tex: (u32, u32)) -> Uniforms {
    Uniforms {
        bounds: f.bounds,
        params: [1.52, f.optics[2].max(0.08), f.optics[0], f.optics[3]],
        extra: [f.shape[0], tex.0 as f32, tex.1 as f32, f.shape[1]],
        tint: f.tint,
        quad: f.clip,
        misc: [f.shape[2], f.shape[3], f.optics[1], flat_flag()],
    }
}

/// DWM 捕获排除：把自己从「所有屏幕捕获」里摘掉。
///
/// WGC / DWM 缩略图 / GDI BitBlt 抓的都是 DWM 合成后的画面，默认会把岛仔自己
/// 也抓进去。WDA_EXCLUDEFROMCAPTURE 让窗口照常显示在屏幕上，但在捕获路径里
/// 完全消失 —— 这样每帧抓到的就是干净的、实时的桌面，不再需要静态底片。
/// 副作用：第三方截图/录屏工具也会看不到岛仔，可用 DAOZAI_LIQUID_NO_EXCLUDE=1 关闭。
#[cfg(windows)]
fn enable_capture_exclusion() {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::*;
    if std::env::var("DAOZAI_LIQUID_NO_EXCLUDE").is_ok() {
        if !EXCLUSION_TRIED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            diag("exclude: 被 DAOZAI_LIQUID_NO_EXCLUDE 关闭，回退静态底片");
        }
        return;
    }
    // 实机证明：SetWindowRgn / 窗口样式变更会把 display affinity 重置，
    // 只设一次的话后面 WGC 帧又会抓到岛仔自己（dump 第 2 帧干净、第 7 帧有残影）。
    // 每帧重新声明一次，EnumWindows 几十个窗口的开销可以忽略。
    unsafe extern "system" fn visit(hwnd: HWND, _: LPARAM) -> BOOL {
        use windows::Win32::System::Threading::GetCurrentProcessId;
        let mut pid = 0u32;
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId()
            && windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool()
            && windows::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity(
                hwnd, windows::Win32::UI::WindowsAndMessaging::WDA_EXCLUDEFROMCAPTURE).is_ok()
        {
            EXCLUDED_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        BOOL(1)
    }
    let tick = EXCLUSION_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // 必须每帧刷新！SetWindowRgn / 样式变化会重置 affinity，降频到每 30 帧试过：
    // 中间那 29 帧 WGC 会抓回岛仔自己，而岛仔背景又来自上一帧纹理 → 直接反馈成
    // 满屏竖条纹，而且会写进底片固化下来。EnumWindows 的几十次调用开销远小于这个代价。
    let _ = EXCLUDED_N.swap(0, std::sync::atomic::Ordering::Relaxed);
    unsafe { let _ = EnumWindows(Some(visit), LPARAM(0)); }
    if tick == 0 || tick % 1800 == 0 {
        diag(&format!("exclude: 刷新 #{} 命中 {} 个窗口", tick,
                      EXCLUDED_N.load(std::sync::atomic::Ordering::Relaxed)));
    }
}

/// 诊断：把某一帧抓到的原始屏幕写成 PPM，直接看 WDA 排除到底给了什么。
/// 只有设置 DAOZAI_LIQUID_DUMP=1 时启用，最多写 8 帧。
#[cfg(windows)]
static DUMP_N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(windows)]
fn dump_capture(buf: &[u8], w: u32, h: u32, tag: &str) {
    if std::env::var("DAOZAI_LIQUID_DUMP").is_err() { return; }
    let n = DUMP_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if n >= 8 { return; }
    let dir = crate::model::config_dir().join("diagnostics");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("liquid-cap-{}-{}.ppm", n, tag));
    let mut out = Vec::with_capacity((w * h * 3) as usize + 32);
    out.extend_from_slice(format!("P6\n{} {}\n255\n", w, h).as_bytes());
    for i in (0..buf.len()).step_by(4) {
        out.push(buf[i]); out.push(buf[i + 1]); out.push(buf[i + 2]);
    }
    let _ = std::fs::write(path, out);
}

/// 取消捕获排除（冻结诊断模式预热结束后调用），让第三方截图重新能看到岛仔。
#[cfg(windows)]
fn disable_capture_exclusion() {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::*;
    unsafe extern "system" fn visit(hwnd: HWND, _: LPARAM) -> BOOL {
        use windows::Win32::System::Threading::GetCurrentProcessId;
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId() && IsWindowVisible(hwnd).as_bool() {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
        }
        BOOL(1)
    }
    unsafe { let _ = EnumWindows(Some(visit), LPARAM(0)); }
    diag("exclude: 冻结模式预热完成，已取消排除（截图可见）");
}

/// 统计抓屏里「岛体排除区以外」的平均亮度，作为深/浅材质的依据。
/// 每隔 16 像素采样一次，跳过岛体区域（那里可能是我们自己的玻璃）。
#[cfg(windows)]
fn sample_luma(buf: &[u8], w: u32, h: u32, x0: i32, y0: i32, x1: i32, y1: i32) -> f32 {
    let (mut sum, mut n) = (0.0f32, 0.0f32);
    for y in (0..h as i32).step_by(16) {
        for x in (0..w as i32).step_by(16) {
            if x >= x0 && x < x1 && y >= y0 && y < y1 { continue; }
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            // 用平均亮度：纹理是 BGRA，R/B 对调对平均值无影响。
            sum += (buf[i] as f32 + buf[i + 1] as f32 + buf[i + 2] as f32) / 3.0 / 255.0;
            n += 1.0;
        }
    }
    if n > 0.0 { (sum / n).clamp(0.0, 1.0) } else { 0.85 }
}

/// 排除未生效时的兜底：首帧抓的「干净桌面底片」。
/// 每帧只把岛体以外的像素写回底片，岛体内部一直用这张底片，避免自我鬼影。
#[cfg(windows)]
const PLATE_W: u32 = 1024;
#[cfg(windows)]
const PLATE_H: u32 = 512;
/// 锚点比首帧抓屏矩形左移 256px：展开时抓屏矩形会向左变宽。
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
            // DIB 是 BGRA，纹理格式也用 Bgra8Unorm，整块拷贝即可（省掉每帧 4 次/像素的转换）。
            let src = std::slice::from_raw_parts(bits as *const u8, rgba.len());
            rgba.copy_from_slice(src);
        }
        SelectObject(mem, old_bmp);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        if ok { Some((rgba, bw, bh)) } else { None }
    }
}

/// 区域平均亮度（0..255）。
#[cfg(windows)]
fn mean_luma(buf: &[u8], w: u32, h: u32, x0: i32, y0: i32, x1: i32, y1: i32) -> f32 {
    let (mut s, mut n) = (0.0f32, 0.0f32);
    for y in (y0.max(0)..y1.min(h as i32)).step_by(4) {
        for x in (x0.max(0)..x1.min(w as i32)).step_by(4) {
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            s += (buf[i] as f32 + buf[i + 1] as f32 + buf[i + 2] as f32) / 3.0;
            n += 1.0;
        }
    }
    if n > 0.0 { s / n } else { 0.0 }
}

/// 岛体以外（capture 外围环带）的平均亮度，作为「干净桌面」的参照。
#[cfg(windows)]
fn mean_luma_outside(buf: &[u8], w: u32, h: u32,
                     ex0: i32, ey0: i32, ex1: i32, ey1: i32) -> f32 {
    let (mut s, mut n) = (0.0f32, 0.0f32);
    for y in (0..h as i32).step_by(4) {
        for x in (0..w as i32).step_by(4) {
            if x >= ex0 && x < ex1 && y >= ey0 && y < ey1 { continue; }
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            s += (buf[i] as f32 + buf[i + 1] as f32 + buf[i + 2] as f32) / 3.0;
            n += 1.0;
        }
    }
    if n > 0.0 { s / n } else { 0.0 }
}

/// 滚动干净底片：干净帧把整块（含岛体）写进底片，污染帧只更新岛体以外。
/// 岛体内部因此最多滞后到最近一次干净帧，而不再是启动时的一张静态快照。
#[cfg(windows)]
fn compose_with_plate(raw: &[u8], bw: u32, bh: u32, x: i32, y: i32,
                      body: [f32; 4], clean_frame: bool, frozen: bool) -> Option<Vec<u8>> {
    let (cx0, cy0, cx1, cy1) = (
        (body[0] - 10.0).max(0.0) as i32,
        (body[1] - 10.0).max(0.0) as i32,
        ((body[0] + body[2] + 10.0).min(bw as f32)) as i32,
        ((body[1] + body[3] + 10.0).min(bh as f32)) as i32,
    );
    let mut guard = PLATE.lock().unwrap_or_else(|e| e.into_inner());
    let need_new = match guard.as_ref() {
        None => true,
        Some(p) => {
            let (ox, oy) = (x - p.x, y - p.y);
            ox < 0 || oy < 0 || ox + bw as i32 > p.w as i32 || oy + bh as i32 > p.h as i32
        }
    };
    if need_new {
        let ax = x - PLATE_ANCHOR;
        let big = grab_raw(ax, y, PLATE_W as i32, PLATE_H as i32);
        let (mut data, pw, ph, fpx, fpy) = if let Some((d, pw, ph)) = big {
            (d, pw, ph, ax, y)
        } else {
            (raw.to_vec(), bw, bh, x, y)
        };
        let (sx, sy) = (x - fpx, y - fpy);
        let (bx0, by0, bx1, by1) = (
            (body[0] + sx as f32 - 10.0).max(0.0) as i32,
            (body[1] + sy as f32 - 10.0).max(0.0) as i32,
            ((body[0] + sx as f32 + body[2] + 10.0).min(pw as f32)) as i32,
            ((body[1] + sy as f32 + body[3] + 10.0).min(ph as f32)) as i32,
        );
        // 岛体区域在底片里是黑的时候（WDA 排除的竞态残留）才抹成上下桌面色的
        // 渐变。之前是「只要不是冻结模式就无条件抹」，于是**干净帧也被抹掉了**：
        // 底片里岛体位置永远是一条竖直渐变，玻璃取样到它就永远是平的，
        // 冻结模式截图诊断看到的全是假背景 —— 玻璃感被判死刑主要就是这条。
        let top = row_mean(&data, pw, ph, bx0, bx1, by0 - 6);
        let bot = row_mean(&data, pw, ph, bx0, bx1, by1 + 6);
        let inner = mean_luma(&data, pw, ph, bx0, by0, bx1, by1);
        let ring = (top[0] + top[1] + top[2] + bot[0] + bot[1] + bot[2]) / 6.0;
        if inner < 45.0 && ring - inner > 80.0 {
            fill_gradient(&mut data, pw, bx0, by0, bx1, by1, top, bot);
        }
        diag(&format!("plate: new {}x{} at ({},{}) frozen={} inner={:.0}",
                      pw, ph, fpx, fpy, frozen, inner));
        *guard = Some(CleanPlate { x: fpx, y: fpy, w: pw, h: ph, data });
    }
    let p = guard.as_mut().unwrap();
    let (ox, oy) = (x - p.x, y - p.y);
    let mut out = vec![0u8; (bw * bh * 4) as usize];
    // 快速路径：干净帧且底片完全覆盖当前矩形时，输出就是 raw 本身，
    // 只要整块刷进底片。省掉每帧 90k+ 次逐像素判断（这是抓屏路径里最重的一段）。
    if clean_frame && ox >= 0 && oy >= 0 && ox + bw as i32 <= p.w as i32 && oy + bh as i32 <= p.h as i32 {
        out.copy_from_slice(raw);
        let row = (bw * 4) as usize;
        for yy in 0..bh as usize {
            let d = ((oy as usize + yy) * p.w as usize + ox as usize) * 4;
            p.data[d..d + row].copy_from_slice(&raw[yy * row..yy * row + row]);
        }
        return Some(out);
    }
    for yy in 0..bh as i32 {
        for xx in 0..bw as i32 {
            let d = ((yy as u32 * bw + xx as u32) * 4) as usize;
            let (qx, qy) = (ox + xx, oy + yy);
            let in_plate = qx >= 0 && qy >= 0 && qx < p.w as i32 && qy < p.h as i32;
            let excluded = xx >= cx0 && xx < cx1 && yy >= cy0 && yy < cy1;
            let use_raw = if frozen { !in_plate } else { clean_frame || !excluded || !in_plate };
            if use_raw {
                out[d..d + 4].copy_from_slice(&raw[d..d + 4]);
                if in_plate {
                    let s = ((qy as u32 * p.w + qx as u32) * 4) as usize;
                    p.data[s..s + 4].copy_from_slice(&raw[d..d + 4]);
                }
            } else {
                let s = ((qy as u32 * p.w + qx as u32) * 4) as usize;
                out[d..d + 4].copy_from_slice(&p.data[s..s + 4]);
            }
        }
    }
    Some(out)
}

/// 抓当前岛体外扩矩形。WGC 帧干净时整块刷新滚动底片，污染时只更新外围；
/// WGC 不可用则回退 GDI，同样走滚动底片（岛体内部保持最近一次干净画面）。
#[cfg(windows)]
fn capture(x: i32, y: i32, w: i32, h: i32, body: [f32; 4]) -> Option<(Vec<u8>, u32, u32)> {
    if w <= 0 || h <= 0 || w > 2048 || h > 1024 { return None; }
    // 截图诊断模式：不排除自身（截图能看到岛仔），背景冻结成启动时的纯壁纸，
    // 这样既能截到玻璃，又能看到真实的折射/色散细节。
    let frozen_env = std::env::var("DAOZAI_LIQUID_FREEZE_BG").is_ok();
    let warm_n = if frozen_env { FREEZE_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed) } else { u64::MAX };
    let warm = frozen_env && warm_n < 24;
    let frozen = frozen_env && warm_n >= 24;
    if !frozen_env || warm {
        enable_capture_exclusion();
    } else if warm_n == 24 {
        disable_capture_exclusion();
    }
    let (raw, bw, bh, from_wgc) = match crate::wgc::grab(x, y, w, h) {
        crate::wgc::Grab::Frame(p, tw, th) => (p, tw, th, true),
        // WGC 只是这一帧还没有新画面：复用上一帧纹理，别退回 GDI 造成闪烁。
        crate::wgc::Grab::NoFrame => return None,
        crate::wgc::Grab::Unavailable => {
            let (p, tw, th) = grab_raw(x, y, w, h)?;
            (p, tw, th, false)
        }
    };
    let (cx0, cy0, cx1, cy1) = (
        (body[0] - 10.0).max(0.0) as i32,
        (body[1] - 10.0).max(0.0) as i32,
        ((body[0] + body[2] + 10.0).min(bw as f32)) as i32,
        ((body[1] + body[3] + 10.0).min(bh as f32)) as i32,
    );
    BG_LUMA.store(sample_luma(&raw, bw, bh, cx0, cy0, cx1, cy1).to_bits(),
                  std::sync::atomic::Ordering::Relaxed);
    dump_capture(&raw, bw, bh, if from_wgc { "wgc" } else { "gdi" });
    let inner = mean_luma(&raw, bw, bh, cx0, cy0, cx1, cy1);
    let outer = mean_luma_outside(&raw, bw, bh, cx0, cy0, cx1, cy1);
    // 只丢弃「岛体几乎全黑、且明显比外围暗」的帧 —— 那是 WDA 排除的竞态残留。
    // 之前用「暗像素比例」判断，深色壁纸会把正常帧全判成污染（实测 inner=0.97），
    // 导致底片永远不更新、换壁纸也不生效。这里改用平均亮度对比，深色壁纸不受影响。
    let black_island = inner < 45.0 && outer - inner > 80.0;
    if black_island {
        if GRAB_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 60 == 0 {
            diag(&format!("capture: 丢弃黑岛形帧 inner={:.0} outer={:.0}", inner, outer));
        }
        return None;
    }
    if GRAB_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 120 == 0 {
        diag(&format!("capture: src={} inner={:.0} outer={:.0} cap={}x{}",
                      if from_wgc { "wgc" } else { "gdi" }, inner, outer, bw, bh));
    }
    // 默认信任 WGC 帧并整块刷新底片：换壁纸/窗口移动都能立即反映。
    let out = compose_with_plate(&raw, bw, bh, x, y, body,
                                 if frozen { false } else { true }, frozen)?;
    if frozen && warm_n == 24 { diag("freeze: 背景已冻结为启动时的干净壁纸"); }
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
        // 每帧都抓：节流会让「纹理内容」与「当前 bounds」错一帧，
        // 展开动画里几何被拉伸，表现为抖动/错位。
        {
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
                    format: wgpu::TextureFormat::Bgra8Unorm,
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
    // 形变强度 0..1（展开进度的变化速度，带衰减），驱动过渡期的光学增强。
    morph: f32,
    tint: [f32; 3],
    time: f32,
    pointer: (f32, f32),
    origin: (f32, f32),
    lightness: f32,
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
        shape: [radius * ppp, morph, (pointer.0 - capture.min.x) * ppp, (pointer.1 - capture.min.y) * ppp],
        tint: [tint[0], tint[1], tint[2], 1.0],
        optics: [time, 1.0, 0.20, lightness],
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
        let f = frame_for(body, capture, (1920, 1080), 1.0, 15.0, 0.0, [0.6, 0.7, 0.8], 1.0, (150.0, 30.0), (0.0, 0.0), 1.0);
        assert_eq!(f.bounds, [40.0, 20.0, 200.0, 30.0]);
        assert_eq!(f.clip, [-1.0, 1.0, 1.0, -1.0], "viewport 已由 egui 设为 callback 矩形");
        assert_eq!(uniforms(&f, (280, 90)).params[0], 1.52);
    }

    #[test]
    fn uniforms_are_packed_without_padding() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 96);
    }
}
