//! winit 事件循环 + D3D11 自绘运行时，替换 eframe::run_native。
//! 主窗口（岛体）+ 独立子 viewport（设置页/邮件阅读器）多窗口管理。

use crate::Shell;
use daozai::d3d11::runtime::{D3d11Runtime, GlassParams, PlainWindow, FrameStats};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    platform::windows::WindowAttributesExtWindows,
    window::{Window, WindowAttributes, WindowId},
};

/// 子 viewport 的 UI closure（egui 回调借用主 app，主线程内当帧有效）。
type UiCb = Rc<RefCell<Box<dyn FnMut(&mut egui::Ui)>>>;

/// egui repaint 唤醒事件循环的代理（user event = ()）。
static REPAINT_PROXY: std::sync::OnceLock<winit::event_loop::EventLoopProxy<()>> = std::sync::OnceLock::new();

thread_local! {
    /// set_immediate_viewport_renderer 回调把新/更新的子 viewport 放进这个队列，
    /// 事件循环在下一处 about_to_wait 里消费它（创建/更新 winit 窗口）。
    static VIEWPORT_REQ: RefCell<Vec<(egui::ViewportId, Window, UiCb)>> = RefCell::new(Vec::new());
}

struct SubWindow {
    window: Window,
    renderer: PlainWindow,
    cb: UiCb,
    id: egui::ViewportId,
}

struct DaozaiApp {
    attrs: WindowAttributes,
    window: Option<Window>,
    runtime: Option<D3d11Runtime>,
    shell: Option<Shell>,
    subs: HashMap<WindowId, SubWindow>,
    sub_ids: HashMap<egui::ViewportId, WindowId>,
    pending_close: Vec<WindowId>,
}

impl DaozaiApp {
    fn new(attrs: WindowAttributes) -> Self {
        Self { attrs, window: None, runtime: None, shell: None,
               subs: HashMap::new(), sub_ids: HashMap::new(), pending_close: Vec::new() }
    }

    /// 消费 egui 的 viewport 请求：新建或更新子窗口。
    fn drain_viewports(&mut self) {
        let reqs = VIEWPORT_REQ.with(|q| std::mem::take(&mut *q.borrow_mut()));
        for (id, window, cb) in reqs {
            if self.sub_ids.contains_key(&id) {
                if let Some(&wid) = self.sub_ids.get(&id) {
                    if let Some(sub) = self.subs.get_mut(&wid) { sub.cb = cb; }
                }
                continue;
            }
            let Some(rt) = self.runtime.as_ref() else { continue };
            // 子窗口是否透明：从 builder 推断（设置页 with_transparent 未设，按不透明）。
            let transparent = false;
            let renderer = match PlainWindow::new(&window, &rt.egui_ctx, transparent) {
                Ok(r) => r, Err(e) => { eprintln!("[d3d11] PlainWindow failed: {e}"); continue; }
            };
            let wid = window.id();
            eprintln!("[winit] sub viewport {:?} -> window {:?}", id, wid);
            self.sub_ids.insert(id, wid);
            self.subs.insert(wid, SubWindow { window, renderer, cb, id });
        }
    }

    /// 主窗口绘制一帧。在 about_to_wait 的事件循环上下文中调用，
    /// 避免在 RedrawRequested handler 内创建子窗口造成重入死锁。
    fn draw_main(&mut self) {
        let Some(w) = self.window.as_ref() else { return };
        let origin = w.inner_position().map(|p| (p.x as f32, p.y as f32)).unwrap_or((0.0, 0.0));
        let ppp = self.runtime.as_ref().unwrap().egui_ctx.pixels_per_point();
        let params = build_glass_params(ppp);
        let input = { let w = self.window.as_ref().unwrap(); self.runtime.as_mut().unwrap().take_input(w) };
        let window = self.window.as_ref().unwrap();
        let rt = self.runtime.as_mut().unwrap();
        let sh = self.shell.as_mut().unwrap();
        let _: FrameStats = rt.render(window, origin, &params, input, |_ctx, ui| sh.draw(ui));
        self.drain_viewports();
        // 不可见子窗口直接绘制（#5229）
        let invisible: Vec<WindowId> = self.subs.iter()
            .filter(|(_, s)| s.window.is_visible() != Some(true))
            .map(|(id, _)| *id).collect();
        for wid in invisible { self.render_sub_inner(wid); }
    }

    /// 绘制子窗口一帧（RedrawRequested 与"不可见窗口直接绘制"共用）。
    fn render_sub(&mut self, wid: WindowId) {
        self.render_sub_inner(wid);
    }
    fn render_sub_inner(&mut self, wid: WindowId) {
        let Some(rt) = self.runtime.as_ref() else { return };
        let ctx = rt.egui_ctx.clone();
        let Some(sub) = self.subs.get_mut(&wid) else { return };
        let input = sub.renderer.egui_winit.take_egui_input(&sub.window);
        let cb = sub.cb.clone();
        sub.renderer.render(&sub.window, &ctx, input, sub.id, |ui| {
            if let Ok(mut f) = cb.try_borrow_mut() { f(ui); }
        });
    }
}

impl ApplicationHandler<()> for DaozaiApp {
    fn user_event(&mut self, _el: &ActiveEventLoop, _event: ()) {
        // egui 后台线程请求重绘：消费 viewport 队列并请求重画。
        self.drain_viewports();
        if let Some(w) = self.window.as_ref() { w.request_redraw(); }
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let window = match el.create_window(self.attrs.clone()) {
            Ok(w) => w,
            Err(e) => { eprintln!("[winit] create_window failed: {e}"); el.exit(); return; }
        };
        let runtime = match D3d11Runtime::new(&window) {
            Ok(r) => r,
            Err(e) => { eprintln!("[d3d11] runtime init failed: {e}"); el.exit(); return; }
        };
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => windows::Win32::Foundation::HWND(h.hwnd.get() as _),
            _ => { eprintln!("[winit] not a win32 window"); el.exit(); return; }
        };
        let ctx = runtime.egui_ctx.clone();

        // 多窗口限制：set_embed_viewports(false) + create_window 在 draw 上下文会
        // 让 winit 事件循环死锁（eframe 需一整套 check_redraw_requests 调度才能安全）。
        // 暂保持 egui 默认内嵌：设置页画在本体，功能完整、循环稳定。
        // TODO: 接 eframe 的完整 new_events/check_redraw_requests 模式后再开多窗口。
        if let Some(proxy) = REPAINT_PROXY.get() {
            let px = proxy.clone();
            ctx.set_request_repaint_callback(move |_info| { let _ = px.send_event(()); });
        }
        // 暂时不注册 immediate viewport renderer（内嵌模式下不需要）。

        let shell = Shell::new(ctx, hwnd);
        self.shell = Some(shell);
        self.runtime = Some(runtime);
        self.window = Some(window);
    }

    fn suspended(&mut self, _el: &ActiveEventLoop) {
        self.window.take();
        self.runtime.take();
        self.shell.take();
        self.subs.clear();
        self.sub_ids.clear();
    }

    fn new_events(&mut self, el: &ActiveEventLoop, cause: winit::event::StartCause) {
        // WaitUntil 到期时 winit 调 new_events；绘制统一放在 about_to_wait，
        // 避免同一帧画两次（这里只维持 16ms 的唤醒节拍）。
        if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. })
            || matches!(cause, winit::event::StartCause::Init) {
            self.drain_viewports();
            el.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(16)));
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.drain_viewports();
        self.draw_main();
        for sub in self.subs.values() {
            sub.window.request_redraw();
        }
        // 固定 60fps 的 WaitUntil：确保 about_to_wait 每帧都被调用（Poll 模式下 winit
        // 不调 about_to_wait，导致 viewport 队列无人消费）。
        el.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(16)));
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.window.as_ref().is_some_and(|w| w.id() == id) {
            let repaint = self.runtime.as_mut().map(|r| {
                let w = self.window.as_ref().unwrap();
                r.on_window_event(w, &event)
            }).unwrap_or(false);
            match event {
                // 绘制在 about_to_wait 的事件循环上下文中做（避免 create_window 重入死锁）。
                WindowEvent::RedrawRequested => {}
                _ => {}
            }
            let _ = repaint;
            return;
        }

        // 子 viewport 窗口
        if let Some(sub) = self.subs.get_mut(&id) {
            if event == WindowEvent::CloseRequested {
                self.pending_close.push(id);
                return;
            }
            let repaint = sub.renderer.on_event(&sub.window, &event);
            if repaint { sub.window.request_redraw(); }
        }
        if let WindowEvent::RedrawRequested = event { self.render_sub(id); }
    }
}

/// 从 app 当前状态构造 HLSL 玻璃 uniform（岛体胶囊布局）。
///
/// 坐标系（移植自旧 wgpu 栈 gpu_glass::build_uniform）：app/egui 传进来的几何和
/// 光学强度全是「逻辑点 / CSS px」，而 shader 的全屏三角形铺的是「物理像素」渲染
/// 目标（= 逻辑尺寸 × ppp，如 960×270 × 1.25 = 1200×338）。所有像素单位必须先乘
/// ppp 转成设备像素——漏掉这步会让 shader 认为画布只有实际的 1/1.25，采到的桌面
/// 区域随之缩小 1.25 倍，表现为「玻璃里的背景被放大一圈」（高 DPI 屏尤甚）。
fn build_glass_params(ppp: f32) -> GlassParams {
    let u = daozai::gpu_glass::uniforms();
    let mut p = GlassParams::default();
    if !u.active {
        // 液态皮肤未激活：不画玻璃（shader 的 bounds 设到窗口外）。
        p.bounds = [-10000.0, -10000.0, 1.0, 1.0];
        return p;
    }
    let ppp = ppp.max(0.1);
    // 画布物理尺寸 = egui 逻辑画布 × ppp（与 D3D11 RT / swap 的真实尺寸一致）。
    let cw = crate::app::CANVAS_W * ppp;
    let h = crate::app::WIN_H * ppp;
    // 岛体矩形、指针：逻辑点 → 设备像素。y 从 egui(顶原点) 翻到 D3D11(底原点)。
    let b = u.body_rect;
    let bx = b[0] * ppp;
    let by = b[1] * ppp;
    let bw = b[2] * ppp;
    let bh = b[3] * ppp;
    p.bounds = [bx, h - by - bh, bw, bh];
    p.extra = [u.radius * ppp, cw, h, 0.0];
    // 光学强度（折射/色散/模糊，单位 CSS px）同样转设备像素；morph/material 无量纲不缩放。
    p.params = [u.chroma * ppp, 0.0, u.morph, u.material];
    p.misc = [u.pointer[0] * ppp, h - u.pointer[1] * ppp, u.refract * ppp, 0.0];
    p.look = [u.blur * ppp, u.band * ppp, u.edge_w * ppp, u.edge_op];
    p.lightcfg = [u.light_follow, 0.0, 0.0, 0.0];
    // 控件玻璃透镜在 D3d11Runtime::render 里、run_ui 之后填充——
    // app 的控件快照由 run_ui 当帧写入，在这里填会读到上一帧，展开/收起动画时差一帧
    // 会在宠物与时间之间留下一条残留透镜白边。见 refresh_control_uniforms。
    // 深浅材质的玻璃染色：暗色玻璃偏冷灰，浅色偏白。
    p.tint = if u.material > 0.5 { [0.96,0.97,1.0,0.10] } else { [0.55,0.72,0.90,0.10] };
    p.notch = [0.0, 0.0, 0.0, 1.0]; // DComp 逐像素 alpha
    p
}


pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<()>::new()?;
    let _ = REPAINT_PROXY.set(event_loop.create_proxy());
    let attrs = WindowAttributes::default()
        .with_title("岛仔")
        .with_inner_size(PhysicalSize::new(crate::app::CANVAS_W, crate::app::WIN_H))
        .with_decorations(false)
        .with_transparent(true)
        .with_active(false)
        .with_resizable(false)
        .with_no_redirection_bitmap(true);
    let mut app = DaozaiApp::new(attrs);
    event_loop.run_app(&mut app)?;
    Ok(())
}
