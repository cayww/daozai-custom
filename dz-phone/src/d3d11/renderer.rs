#![warn(missing_docs)]

//! `egui-directx11`: a Direct3D11 renderer for [`egui`](https://crates.io/crates/egui).
//!
//! This crate aims to provide a *minimal* set of features and APIs to render
//! outputs from `egui` using Direct3D11. We assume you to be familiar with
//! developing graphics applications using Direct3D11, and if not, this crate is
//! not likely useful for you. Besides, this crate cares only about rendering
//! outputs from `egui`, so it is all *your* responsibility to handle things
//! like setting up the window and event loop, creating the device and swap
//! chain, etc.
//!
//! This crate is built upon the *official* Rust bindings of Direct3D11 and DXGI
//! APIs from the [`windows`](https://crates.io/crates/windows) crate [maintained by
//! Microsoft](https://github.com/microsoft/windows-rs). Using this crate with
//! other Direct3D11 bindings is not recommended and may result in unexpected
//! behavior.
//!
//! To get started, you can check the [`Renderer`] struct provided by this
//! crate. You can also take a look at the [example](https://github.com/Nekomaru-PKU/egui-directx11/blob/main/examples/main.rs), which demonstrates all you need to do to set up a minimal application
//! with Direct3D11 and `egui`. This example uses `winit` for window management
//! and event handling, while native Win32 APIs should also work well.

use super::texture::TexturePool;

use std::mem;

const fn zeroed<T>() -> T {
    unsafe { mem::zeroed() }
}

use egui::{
    ClippedPrimitive, Pos2,
    epaint::{ClippedShape, Primitive, Vertex, textures::TexturesDelta},
};

use windows::Win32::Foundation::{BOOL, RECT};
use windows::Win32::Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*};
use windows::core::{Interface, Result};

/// The core of this crate. You can set up a renderer via [`Renderer::new`]
/// and render the output from `egui` with [`Renderer::render`].
pub struct Renderer {
    device: ID3D11Device,
    input_layout: ID3D11InputLayout,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    rasterizer_state: ID3D11RasterizerState,
    sampler_state: ID3D11SamplerState,
    blend_state: ID3D11BlendState,
    /// egui 0.37 `Shape::Clear` 需要无混合写入透明黑。
    clear_blend_state: ID3D11BlendState,

    texture_pool: TexturePool,
}

/// Part of [`egui::FullOutput`] that is consumed by [`Renderer::render`].
///
/// Call to [`egui::Context::run`] or [`egui::Context::end_frame`] yields a
/// [`egui::FullOutput`]. The platform integration (for example `egui_winit`)
/// consumes [`egui::FullOutput::platform_output`] and
/// [`egui::FullOutput::viewport_output`], and the renderer consumes the rest.
///
/// To conveniently split a [`egui::FullOutput`] into a [`RendererOutput`] and
/// outputs for the platform integration, use [`split_output`].
#[allow(missing_docs)]
pub struct RendererOutput {
    pub textures_delta: TexturesDelta,
    pub shapes: Vec<ClippedShape>,
    pub pixels_per_point: f32,
}

/// Convenience method to split a [`egui::FullOutput`] into the
/// [`RendererOutput`] part and other parts for platform integration.
///
/// The returned tuple should be destructured as:
/// ```ignore
/// let (renderer_output, platform_output, viewport_output) =
///     egui_directx11::split_output(full_output);
/// ```
/// 把 egui 的逻辑裁剪矩形换算成物理像素，四舍五入并夹到渲染目标内。
///
/// 上游 `egui-wgpu` 的 `ScissorRect::new` 用 `.round()`；直接 `as i32` 是向零截断，
/// 在 1.25 / 1.5 这类非整数 DPI 下会让 scissor 边缘整体差 1px，相邻图形之间于是
/// 露出一条 1px 浅色缝（悬停抬升改变坐标小数部分时又会消失）。
pub(crate) fn scissor_pixels(
    clip_rect: egui::Rect,
    zoom_factor: f32,
    frame_size: (u32, u32),
) -> egui::Rect {
    scissor_pixels_with(clip_rect, zoom_factor, frame_size, ScissorTuning::from_env())
}

/// 调试用取整方式。默认 `Round`（与上游 wgpu 一致）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScissorRound {
    Round,
    Floor,
    Ceil,
    /// 直接 `as i32` 的向零截断（修复前的旧行为）。
    Trunc,
}

impl ScissorRound {
    fn apply(self, v: f32) -> f32 {
        match self {
            Self::Round => v.round(),
            Self::Floor => v.floor(),
            Self::Ceil => v.ceil(),
            Self::Trunc => v.trunc(),
        }
    }
}

/// 可在运行时用环境变量微调，便于实机定位 1px 接缝，不用重编译：
///
/// - `DAOZAI_SCISSOR_ROUND=round|floor|ceil|trunc`（默认 round）
/// - `DAOZAI_SCISSOR_EXPAND=<f32 像素>`：正数向外扩、负数向内缩（默认 0）
/// - `DAOZAI_SCISSOR_OFF=1`：完全关闭裁剪（设为整个渲染目标），用于判断接缝是否来自裁剪
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ScissorTuning {
    pub round: ScissorRound,
    pub expand: f32,
    pub off: bool,
}

impl Default for ScissorTuning {
    fn default() -> Self {
        Self { round: ScissorRound::Round, expand: 0.0, off: false }
    }
}

impl ScissorTuning {
    fn from_env() -> Self {
        // 每帧每个 mesh 都会问一次，缓存起来；环境变量改了需重启程序才生效。
        static CACHED: std::sync::OnceLock<ScissorTuning> = std::sync::OnceLock::new();
        *CACHED.get_or_init(Self::read_env)
    }

    fn read_env() -> Self {
        let mut t = Self::default();
        if let Ok(v) = std::env::var("DAOZAI_SCISSOR_ROUND") {
            t.round = match v.trim().to_ascii_lowercase().as_str() {
                "floor" => ScissorRound::Floor,
                "ceil" => ScissorRound::Ceil,
                "trunc" => ScissorRound::Trunc,
                _ => ScissorRound::Round,
            };
        }
        if let Ok(v) = std::env::var("DAOZAI_SCISSOR_EXPAND") {
            if let Ok(f) = v.trim().parse::<f32>() { t.expand = f; }
        }
        t.off = std::env::var("DAOZAI_SCISSOR_OFF").is_ok_and(|v| v.trim() == "1");
        t
    }
}

pub(crate) fn scissor_pixels_with(
    clip_rect: egui::Rect,
    zoom_factor: f32,
    frame_size: (u32, u32),
    tune: ScissorTuning,
) -> egui::Rect {
    let w = frame_size.0 as f32;
    let h = frame_size.1 as f32;
    if tune.off {
        // 完全裁到整块渲染目标，纯粹用于判断“接缝是否来自 scissor”。
        return egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(w, h));
    }
    let r = tune.round;
    let e = tune.expand;
    let x0 = (r.apply(clip_rect.min.x * zoom_factor) - e).clamp(0.0, w);
    let y0 = (r.apply(clip_rect.min.y * zoom_factor) - e).clamp(0.0, h);
    let x1 = (r.apply(clip_rect.max.x * zoom_factor) + e).clamp(x0, w);
    let y1 = (r.apply(clip_rect.max.y * zoom_factor) + e).clamp(y0, h);
    egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
}

/// 诊断：把与指定区域相交的图元（类型/矩形/颜色/线宽）转储到文件。
///
/// 用环境变量开启，跑一次即可定位某个位置上的细线是由哪个图元画出来的：
///
/// ```text
/// DAOZAI_DUMP_SHAPES="694,575,716,600"     感兴趣的区域（逻辑点 x0,y0,x1,y1）
/// DAOZAI_DUMP_FILE="D:/tmp/shapes.txt"     输出文件（默认 daozai-shapes.txt）
/// DAOZAI_DUMP_FRAME=1                      只转储第 1 帧（默认 1）
/// ```
pub(crate) fn maybe_dump_shapes(shapes: &[egui::epaint::ClippedShape]) {
    let Ok(region) = std::env::var("DAOZAI_DUMP_SHAPES") else {
        return;
    };
    let nums: Vec<f32> = region
        .split(',')
        .filter_map(|v| v.trim().parse::<f32>().ok())
        .collect();
    if nums.len() != 4 {
        return;
    }
    let want = egui::Rect::from_min_max(
        egui::pos2(nums[0], nums[1]),
        egui::pos2(nums[2], nums[3]),
    );
    let path = std::env::var("DAOZAI_DUMP_FILE")
        .unwrap_or_else(|_| "daozai-shapes.txt".to_owned());

    let mut out = String::new();
    out.push_str(&format!(
        "== 区域 {:?} 内的图元（按绘制顺序）==\n",
        want
    ));
    let mut index = 0usize;
    dump_shapes_into(shapes, want, &mut out, &mut index);
    out.push_str(&format!("== 共 {} 条 ==\n", index));
    let _ = std::fs::write(&path, out);
}

fn dump_shapes_into(
    shapes: &[egui::epaint::ClippedShape],
    want: egui::Rect,
    out: &mut String,
    index: &mut usize,
) {
    for clipped in shapes {
        dump_shape(&clipped.shape, clipped.clip_rect, want, out, index);
    }
}

fn dump_shape(
    shape: &egui::Shape,
    clip: egui::Rect,
    want: egui::Rect,
    out: &mut String,
    index: &mut usize,
) {
    use egui::Shape;
    if let Shape::Vec(inner) = shape {
        for s in inner {
            dump_shape(s, clip, want, out, index);
        }
        return;
    }
    let bounds = shape.visual_bounding_rect();
    if !bounds.intersects(want) {
        return;
    }
    *index += 1;
    let head = format!(
        "#{:03} clip=({:.1},{:.1})-({:.1},{:.1}) bounds=({:.1},{:.1})-({:.1},{:.1})",
        index,
        clip.min.x, clip.min.y, clip.max.x, clip.max.y,
        bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y,
    );
    let body = match shape {
        Shape::Rect(r) => format!(
            "Rect fill={:?} stroke={:?}({:.2}) radius={:?}",
            r.fill, r.stroke.color, r.stroke.width, r.corner_radius
        ),
        Shape::Circle(c) => format!(
            "Circle center=({:.2},{:.2}) r={:.2} fill={:?} stroke={:?}({:.2})",
            c.center.x, c.center.y, c.radius, c.fill, c.stroke.color, c.stroke.width
        ),
        Shape::LineSegment { stroke, .. } => {
            format!("LineSeg stroke={:?}({:.2})", stroke.color, stroke.width)
        }
        Shape::Path(p) => format!(
            "Path points={} closed={:?} fill={:?} stroke={:?}({:.2})",
            p.points.len(), p.closed, p.fill, p.stroke.color, p.stroke.width
        ),
        Shape::Text(t) => format!("Text \"{}\"", t.galley.text()),
        Shape::Mesh(m) => format!(
            "Mesh verts={} idx={}",
            m.vertices.len(),
            m.indices.len()
        ),
        other => format!("{:?}", std::mem::discriminant(other)),
    };
    out.push_str(&format!("{head} {body}\n"));
}

pub fn split_output(
    full_output: egui::FullOutput,
) -> (
    RendererOutput,
    egui::PlatformOutput,
    egui::OrderedViewportIdMap<egui::ViewportOutput>,
) {
    (
        RendererOutput {
            textures_delta: full_output.textures_delta,
            shapes: full_output.shapes,
            pixels_per_point: full_output.pixels_per_point,
        },
        full_output.platform_output,
        full_output.viewport_output,
    )
}

#[repr(C)]
struct VertexData {
    pos: Pos2,
    uv: Pos2,
    color: [f32; 4],
}

struct MeshData {
    vtx: Vec<VertexData>,
    idx: Vec<u32>,
    tex: egui::TextureId,
    clip_rect: egui::Rect,
}

enum DrawOp {
    Clear(MeshData),
    Mesh(MeshData),
}

impl Renderer {
    /// Create a [`Renderer`] using the provided Direct3D11 device. The
    /// [`Renderer`] holds various Direct3D11 resources and states derived
    /// from the device.
    ///
    /// If any Direct3D resource creation fails, this function will return an
    /// error. You can create the Direct3D11 device with debug layer enabled
    /// to find out details on the error.
    pub fn new(device: &ID3D11Device) -> Result<Self> {
        let mut input_layout = None;
        let mut vertex_shader = None;
        let mut pixel_shader = None;
        let mut rasterizer_state = None;
        let mut sampler_state = None;
        let mut blend_state = None;
        let mut clear_blend_state = None;
        unsafe {
            device.CreateInputLayout(
                &Self::INPUT_ELEMENTS_DESC,
                Self::VS_BLOB,
                Some(&mut input_layout),
            )?;
            device.CreateVertexShader(
                Self::VS_BLOB,
                None,
                Some(&mut vertex_shader),
            )?;
            device.CreatePixelShader(
                Self::PS_BLOB,
                None,
                Some(&mut pixel_shader),
            )?;
            device.CreateRasterizerState(
                &Self::RASTERIZER_DESC,
                Some(&mut rasterizer_state),
            )?;
            device.CreateSamplerState(
                &Self::SAMPLER_DESC,
                Some(&mut sampler_state),
            )?;
            device
                .CreateBlendState(&Self::BLEND_DESC, Some(&mut blend_state))?;
            device.CreateBlendState(
                &Self::CLEAR_BLEND_DESC,
                Some(&mut clear_blend_state),
            )?;
        };
        Ok(Self {
            device: device.clone(),
            input_layout: input_layout.unwrap(),
            vertex_shader: vertex_shader.unwrap(),
            pixel_shader: pixel_shader.unwrap(),
            rasterizer_state: rasterizer_state.unwrap(),
            sampler_state: sampler_state.unwrap(),
            blend_state: blend_state.unwrap(),
            clear_blend_state: clear_blend_state.unwrap(),
            texture_pool: TexturePool::new(device),
        })
    }

    /// Register a user-provided `ID3D11ShaderResourceView` and get a [`egui::TextureId`] for it.
    ///
    /// This allows you to use your own DirectX11 textures within egui. The returned
    /// [`egui::TextureId`] can be used with [`egui::Image`], [`egui::ImageButton`], or
    /// any other egui widget that accepts a texture ID.
    ///
    /// The texture will remain registered until you call [`Renderer::unregister_user_texture`]
    /// or the [`Renderer`] is dropped.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Assuming you have a ID3D11ShaderResourceView
    /// let texture_id = renderer.register_user_texture(my_srv);
    ///
    /// // Use it in egui
    /// ui.image(egui::ImageSource::Texture(egui::load::SizedTexture::new(
    ///     texture_id,
    ///     egui::vec2(256.0, 256.0),
    /// )));
    /// ```
    pub fn register_user_texture(
        &mut self,
        srv: ID3D11ShaderResourceView,
    ) -> egui::TextureId {
        self.texture_pool.register_user_texture(srv)
    }

    /// Unregister a user texture by its [`egui::TextureId`].
    ///
    /// Returns `true` if the texture was found and removed, `false` otherwise.
    /// Note that this only works for user-registered textures, not textures
    /// managed by egui itself.
    pub fn unregister_user_texture(&mut self, tid: egui::TextureId) -> bool {
        self.texture_pool.unregister_user_texture(tid)
    }

    /// Render the output of `egui` to the provided `render_target`.
    ///
    /// As `egui` requires color blending in gamma space, **the provided
    /// `render_target` MUST be in the gamma color space and viewed as
    /// non-sRGB-aware** (i.e. do NOT use `_SRGB` format in the texture and
    /// the view).
    ///
    /// If you have to render to a render target in linear color space or
    /// one that is sRGB-aware, you must create an intermediate render target
    /// in gamma color space and perform a blit operation afterwards.
    ///
    /// The `scale_factor` should be the scale factor of your window and not
    /// confused with [`egui::Context::zoom_factor`]. If you are using `winit`,
    /// the `scale_factor` can be aquired using `Window::scale_factor`.
    ///
    /// ## Error Handling
    ///
    /// If any Direct3D resource creation fails, this function will return an
    /// error. In this case you may have a incomplete or incorrect rendering
    /// result. You can create the Direct3D11 device with debug layer
    /// enabled to find out details on the error.
    /// If the device has been lost, you should drop the [`Renderer`] and create
    /// a new one.
    ///
    /// ## Pipeline State Management
    ///
    /// This function sets up its own Direct3D11 pipeline state for rendering on
    /// the provided device context. It assumes that the hull shader, domain
    /// shader and geometry shader stages are not active on the provided device
    /// context without any further checks. It is all *your* responsibility to
    /// backup the current pipeline state and restore it afterwards if your
    /// rendering pipeline depends on it.
    ///
    /// Particularly, it overrides:
    /// + The input layout, vertex buffer, index buffer and primitive topology
    ///   in the input assembly stage;
    /// + The current shader in the vertex shader stage;
    /// + The viewport and rasterizer state in the rasterizer stage;
    /// + The current shader, shader resource slot 0 and sampler slot 0 in the
    ///   pixel shader stage;
    /// + The render target(s) and blend state in the output merger stage;
    pub fn render(
        &mut self,
        device_context: &ID3D11DeviceContext,
        render_target: &ID3D11RenderTargetView,
        egui_ctx: &egui::Context,
        egui_output: RendererOutput,
    ) -> Result<()> {
        self.texture_pool
            .update(device_context, egui_output.textures_delta)?;

        if egui_output.shapes.is_empty() {
            return Ok(());
        }

        // 诊断：DAOZAI_DUMP_SHAPES 开启时把目标区域的图元写文件（只写一次）。
        {
            static ONCE: std::sync::Once = std::sync::Once::new();
            if std::env::var("DAOZAI_DUMP_SHAPES").is_ok() {
                ONCE.call_once(|| maybe_dump_shapes(&egui_output.shapes));
            }
        }

        let frame_size = Self::get_render_target_size(render_target)?;
        let frame_size_scaled = (
            frame_size.0 as f32 / egui_output.pixels_per_point,
            frame_size.1 as f32 / egui_output.pixels_per_point,
        );
        let zoom_factor = egui_ctx.zoom_factor();

        let clip_px = |r: egui::Rect| scissor_pixels(r, zoom_factor, frame_size);

        self.setup(device_context, render_target, frame_size);
        let ndc = |p: Pos2| {
            (
                p.x * zoom_factor / frame_size_scaled.0 * 2.0 - 1.0,
                1.0 - p.y * zoom_factor / frame_size_scaled.1 * 2.0,
            )
        };
        // 诊断开关（运行时环境变量，改完重启即生效，无需重编译）：
        //   DAOZAI_TESS_FEATHER=0      关闭羽化抗锯齿
        //   DAOZAI_TESS_FEATHER_PX=0.5 调整羽化宽度（默认 1.0）
        {
            let off = std::env::var("DAOZAI_TESS_FEATHER").is_ok_and(|v| v.trim() == "0");
            let px = std::env::var("DAOZAI_TESS_FEATHER_PX").ok()
                .and_then(|v| v.trim().parse::<f32>().ok());
            if off || px.is_some() {
                egui_ctx.tessellation_options_mut(|o| {
                    o.feathering = !off;
                    if let Some(px) = px { o.feathering_size_in_pixels = px; }
                });
            }
        }
        let draw_ops: Vec<DrawOp> = egui_ctx
            .tessellate(egui_output.shapes, egui_output.pixels_per_point)
            .into_iter()
            .filter_map(
                |ClippedPrimitive {
                     primitive,
                     clip_rect,
                 }| match primitive {
                    Primitive::Mesh(mesh) => {
                        if mesh.indices.is_empty() {
                            return None;
                        }
                        if mesh.indices.len() % 3 != 0 {
                            log::warn!(concat!(
                                "egui wants to draw a incomplete triangle. ",
                                "this request will be ignored."
                            ));
                            return None;
                        }
                        Some(DrawOp::Mesh(MeshData {
                            vtx: mesh
                                .vertices
                                .into_iter()
                                .map(|Vertex { pos, uv, color }| {
                                    let (x, y) = ndc(pos);
                                    VertexData {
                                        pos: Pos2::new(x, y),
                                        uv,
                                        color: [
                                            color[0] as f32 / 255.0,
                                            color[1] as f32 / 255.0,
                                            color[2] as f32 / 255.0,
                                            color[3] as f32 / 255.0,
                                        ],
                                    }
                                })
                                .collect(),
                            idx: mesh.indices,
                            tex: mesh.texture_id,
                            clip_rect: clip_px(
                                clip_rect * egui_output.pixels_per_point,
                            ),
                        }))
                    },
                    Primitive::Callback(..) => {
                        log::warn!("paint callbacks are not yet supported.");
                        None
                    },
                    Primitive::Clear(rect) => {
                        let rect = rect.intersect(clip_rect);
                        if rect.min.x >= rect.max.x || rect.min.y >= rect.max.y {
                            return None;
                        }
                        let corners = [
                            rect.min,
                            Pos2::new(rect.max.x, rect.min.y),
                            rect.max,
                            Pos2::new(rect.min.x, rect.max.y),
                        ];
                        let vtx = corners
                            .into_iter()
                            .map(|p| {
                                let (x, y) = ndc(p);
                                VertexData {
                                    pos: Pos2::new(x, y),
                                    uv: Pos2::new(0.0, 0.0),
                                    color: [0.0; 4],
                                }
                            })
                            .collect();
                        Some(DrawOp::Clear(MeshData {
                            vtx,
                            idx: vec![0, 1, 2, 0, 2, 3],
                            tex: egui::TextureId::default(),
                            clip_rect: clip_px(
                                rect * egui_output.pixels_per_point,
                            ),
                        }))
                    },
                },
            )
            .collect();

        let mut clear_blend = false;
        for op in draw_ops {
            match &op {
                DrawOp::Clear(_) if !clear_blend => {
                    unsafe {
                        device_context.OMSetBlendState(
                            &self.clear_blend_state,
                            Some(&[0.; 4]),
                            u32::MAX,
                        );
                    }
                    clear_blend = true;
                },
                DrawOp::Mesh(_) if clear_blend => {
                    unsafe {
                        device_context.OMSetBlendState(
                            &self.blend_state,
                            Some(&[0.; 4]),
                            u32::MAX,
                        );
                    }
                    clear_blend = false;
                },
                _ => {}
            }
            let mesh = match op {
                DrawOp::Clear(mesh) | DrawOp::Mesh(mesh) => mesh,
            };
            Self::draw_mesh(
                &self.device,
                device_context,
                &self.texture_pool,
                mesh,
            )?;
        }
        if clear_blend {
            unsafe {
                device_context.OMSetBlendState(
                    &self.blend_state,
                    Some(&[0.; 4]),
                    u32::MAX,
                );
            }
        }
        Ok(())
    }

    fn setup(
        &mut self,
        ctx: &ID3D11DeviceContext,
        render_target: &ID3D11RenderTargetView,
        frame_size: (u32, u32),
    ) {
        unsafe {
            ctx.IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            ctx.IASetInputLayout(&self.input_layout);
            ctx.VSSetShader(&self.vertex_shader, None);
            ctx.PSSetShader(&self.pixel_shader, None);
            ctx.RSSetState(&self.rasterizer_state);
            ctx.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.,
                TopLeftY: 0.,
                Width: frame_size.0 as _,
                Height: frame_size.1 as _,
                MinDepth: 0.,
                MaxDepth: 1.,
            }]));
            ctx.PSSetSamplers(0, Some(&[Some(self.sampler_state.clone())]));
            ctx.OMSetRenderTargets(Some(&[Some(render_target.clone())]), None);
            ctx.OMSetBlendState(&self.blend_state, Some(&[0.; 4]), u32::MAX);
        }
    }

    fn draw_mesh(
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        texture_pool: &TexturePool,
        mesh: MeshData,
    ) -> Result<()> {
        let vb = Self::create_index_buffer(device, &mesh.idx)?;
        let ib = Self::create_vertex_buffer(device, &mesh.vtx)?;
        unsafe {
            device_context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(ib)),
                Some(&(mem::size_of::<VertexData>() as _)),
                Some(&0),
            );
            device_context.IASetIndexBuffer(&vb, DXGI_FORMAT_R32_UINT, 0);
            device_context.RSSetScissorRects(Some(&[RECT {
                left: mesh.clip_rect.left() as _,
                top: mesh.clip_rect.top() as _,
                right: mesh.clip_rect.right() as _,
                bottom: mesh.clip_rect.bottom() as _,
            }]));
        }
        if let Some(srv) = texture_pool.get_srv(mesh.tex) {
            unsafe {
                device_context.PSSetShaderResources(0, Some(&[Some(srv)]))
            };
        } else {
            log::warn!(
                concat!(
                    "egui wants to sample a non-existing texture {:?}.",
                    "this request will be ignored."
                ),
                mesh.tex
            );
        };
        unsafe { device_context.DrawIndexed(mesh.idx.len() as _, 0, 0) };
        Ok(())
    }
}

impl Renderer {
    const VS_BLOB: &'static [u8] = include_bytes!("../../shaders/vs_egui.bin");
    const PS_BLOB: &'static [u8] = include_bytes!("../../shaders/ps_egui.bin");

    const INPUT_ELEMENTS_DESC: [D3D11_INPUT_ELEMENT_DESC; 3] = [
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: windows::core::s!("POSITION"),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: windows::core::s!("TEXCOORD"),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: windows::core::s!("COLOR"),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: D3D11_APPEND_ALIGNED_ELEMENT,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
    ];

    const RASTERIZER_DESC: D3D11_RASTERIZER_DESC = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        FrontCounterClockwise: BOOL(0),
        DepthBias: 0,
        DepthBiasClamp: 0.,
        SlopeScaledDepthBias: 0.,
        DepthClipEnable: BOOL(0),
        ScissorEnable: BOOL(1),
        MultisampleEnable: BOOL(0),
        AntialiasedLineEnable: BOOL(0),
    };

    const SAMPLER_DESC: D3D11_SAMPLER_DESC = D3D11_SAMPLER_DESC {
        // egui font atlas / meshes have no useful mip chain; bilinear is enough,
        // and point mip filtering avoids sampling a blurrier level if one exists.
        Filter: D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT,
        AddressU: D3D11_TEXTURE_ADDRESS_BORDER,
        AddressV: D3D11_TEXTURE_ADDRESS_BORDER,
        AddressW: D3D11_TEXTURE_ADDRESS_BORDER,
        ComparisonFunc: D3D11_COMPARISON_ALWAYS,
        BorderColor: [1., 1., 1., 1.],
        ..zeroed()
    };

    const BLEND_DESC: D3D11_BLEND_DESC = D3D11_BLEND_DESC {
        RenderTarget: [
            D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: BOOL(1),
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_INV_DEST_ALPHA,
                DestBlendAlpha: D3D11_BLEND_ONE,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as _,
            },
            zeroed(),
            zeroed(),
            zeroed(),
            zeroed(),
            zeroed(),
            zeroed(),
            zeroed(),
        ],
        ..zeroed()
    };

    const CLEAR_BLEND_DESC: D3D11_BLEND_DESC = D3D11_BLEND_DESC {
        RenderTarget: [
            D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: BOOL(0),
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_ZERO,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_ZERO,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as _,
            },
            zeroed(),
            zeroed(),
            zeroed(),
            zeroed(),
            zeroed(),
            zeroed(),
            zeroed(),
        ],
        ..zeroed()
    };
}

impl Renderer {
    fn create_vertex_buffer(
        device: &ID3D11Device,
        data: &[VertexData],
    ) -> Result<ID3D11Buffer> {
        let mut vertex_buffer = None;
        unsafe {
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: mem::size_of_val(data) as _,
                    Usage: D3D11_USAGE_IMMUTABLE,
                    BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as _,
                    ..D3D11_BUFFER_DESC::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: data.as_ptr() as _,
                    ..D3D11_SUBRESOURCE_DATA::default()
                }),
                Some(&mut vertex_buffer),
            )
        }?;
        Ok(vertex_buffer.unwrap())
    }

    fn create_index_buffer(
        device: &ID3D11Device,
        data: &[u32],
    ) -> Result<ID3D11Buffer> {
        let mut index_buffer = None;
        unsafe {
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: mem::size_of_val(data) as _,
                    Usage: D3D11_USAGE_IMMUTABLE,
                    BindFlags: D3D11_BIND_INDEX_BUFFER.0 as _,
                    ..D3D11_BUFFER_DESC::default()
                },
                Some(&D3D11_SUBRESOURCE_DATA {
                    pSysMem: data.as_ptr() as _,
                    ..D3D11_SUBRESOURCE_DATA::default()
                }),
                Some(&mut index_buffer),
            )
        }?;
        Ok(index_buffer.unwrap())
    }

    fn get_render_target_size(
        rtv: &ID3D11RenderTargetView,
    ) -> Result<(u32, u32)> {
        let tex = unsafe { rtv.GetResource() }?.cast::<ID3D11Texture2D>()?;
        let mut desc = zeroed();
        unsafe { tex.GetDesc(&mut desc) };
        Ok((desc.Width, desc.Height))
    }
}

#[cfg(test)]
mod scissor_tests {
    use super::{scissor_pixels_with, ScissorRound, ScissorTuning};
    use egui::{pos2, Rect};

    /// 测试一律用显式 tuning，避免受本机调试环境变量影响。
    fn scissor_pixels(r: Rect, zoom: f32, size: (u32, u32)) -> Rect {
        scissor_pixels_with(r, zoom, size, ScissorTuning::default())
    }

    /// 非整数 DPI 下必须四舍五入，不能向零截断——否则 1px 缝。
    #[test]
    fn scissor_rounds_to_nearest_physical_pixel_at_fractional_dpi() {
        let r = Rect::from_min_max(pos2(100.8, 200.9), pos2(300.4, 400.6));
        let out = scissor_pixels(r, 1.0, (1000, 1000));
        assert_eq!(out.min, pos2(101.0, 201.0), "0.8/0.9 应进位，而不是截断成 100/200");
        assert_eq!(out.max, pos2(300.0, 401.0));
    }

    #[test]
    fn scissor_clamps_into_render_target() {
        let r = Rect::from_min_max(pos2(-5.0, -5.0), pos2(2000.0, 2000.0));
        let out = scissor_pixels(r, 1.0, (800, 600));
        assert_eq!(out.min, pos2(0.0, 0.0));
        assert_eq!(out.max, pos2(800.0, 600.0));
    }

    #[test]
    fn scissor_applies_zoom_factor() {
        let r = Rect::from_min_max(pos2(10.0, 20.0), pos2(30.0, 40.0));
        let out = scissor_pixels(r, 2.0, (1000, 1000));
        assert_eq!(out.min, pos2(20.0, 40.0));
        assert_eq!(out.max, pos2(60.0, 80.0));
    }

    /// 上下界在 clamp 时不能交叉（退化矩形塌成零高/零宽，而不是反向）。
    #[test]
    fn scissor_never_inverts_bounds() {
        let r = Rect::from_min_max(pos2(500.0, 500.0), pos2(400.0, 400.0));
        let out = scissor_pixels(r, 1.0, (450, 450));
        assert!(out.min.x <= out.max.x && out.min.y <= out.max.y);
    }

    #[test]
    fn scissor_tuning_floor_and_ceil_differ_from_round() {
        let r = Rect::from_min_max(pos2(10.4, 10.6), pos2(20.6, 20.4));
        let floor = scissor_pixels_with(r, 1.0, (100, 100), ScissorTuning {
            round: ScissorRound::Floor, expand: 0.0, off: false,
        });
        let ceil = scissor_pixels_with(r, 1.0, (100, 100), ScissorTuning {
            round: ScissorRound::Ceil, expand: 0.0, off: false,
        });
        assert_eq!(floor, Rect::from_min_max(pos2(10.0, 10.0), pos2(20.0, 20.0)));
        assert_eq!(ceil, Rect::from_min_max(pos2(11.0, 11.0), pos2(21.0, 21.0)));
    }

    #[test]
    fn scissor_expand_grows_and_shrinks_the_rect() {
        let r = Rect::from_min_max(pos2(10.0, 10.0), pos2(20.0, 20.0));
        let grow = scissor_pixels_with(r, 1.0, (100, 100), ScissorTuning {
            round: ScissorRound::Round, expand: 1.0, off: false,
        });
        assert_eq!(grow, Rect::from_min_max(pos2(9.0, 9.0), pos2(21.0, 21.0)));
        let shrink = scissor_pixels_with(r, 1.0, (100, 100), ScissorTuning {
            round: ScissorRound::Round, expand: -1.0, off: false,
        });
        assert_eq!(shrink, Rect::from_min_max(pos2(11.0, 11.0), pos2(19.0, 19.0)));
    }

    /// DAOZAI_SCISSOR_OFF=1：整块渲染目标，用于判断接缝是否来自裁剪。
    #[test]
    fn scissor_off_covers_the_whole_target() {
        let r = Rect::from_min_max(pos2(10.0, 10.0), pos2(20.0, 20.0));
        let out = scissor_pixels_with(r, 1.0, (300, 200), ScissorTuning {
            round: ScissorRound::Round, expand: 0.0, off: true,
        });
        assert_eq!(out, Rect::from_min_max(pos2(0.0, 0.0), pos2(300.0, 200.0)));
    }
}
