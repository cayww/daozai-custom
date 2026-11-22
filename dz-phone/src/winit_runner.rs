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

/// egui repaint 唤醒事件循环的代理（user event = ()）。
static REPAINT_PROXY: std::sync::OnceLock<winit::event_loop::EventLoopProxy<()>> = std::sync::OnceLock::new();

thread_local! {
    /// immediate viewport renderer 回调（在 draw 上下文同步执行）把该帧子窗口的
    /// FullOutput 放进此队列；drain_viewports 在事件循环里消费、建/更新 winit 窗口。
    /// 存 output 而非 UI 闭包：immediate 回调借用 app，不能跨帧存；output 可以。
    static VIEWPORT_REQ: RefCell<Vec<(egui::ViewportId, Rc<Window>, egui::FullOutput)>> = RefCell::new(Vec::new());
    /// immediate 回调每帧都触发（show_viewport_immediate 每帧调用），窗口必须复用，
    /// 否则每帧 create 一个 invisible 新窗口（builder with_visible(false)）。
    static VIEWPORT_WINDOWS: RefCell<HashMap<egui::ViewportId, (Rc<Window>, egui::ViewportBuilder)>> = RefCell::new(HashMap::new());
    /// 子窗口的输入（含鼠标/键盘事件）：window_event 累积，draw_main 前取出交给
    /// immediate 回调（回调在主窗口 draw 上下文执行，拿不到 sub.renderer）。
    static VIEWPORT_INPUTS: RefCell<HashMap<egui::ViewportId, egui::RawInput>> = RefCell::new(HashMap::new());
    /// 子窗口右上角 X 的关闭请求（ViewportId）：window_event 记录，immediate 回调
    /// 转成 ViewportInfo.events[Close]，让设置页 UI 的 close_requested() 读到并自行关闭。
    static CLOSE_REQ: RefCell<Vec<egui::ViewportId>> = RefCell::new(Vec::new());
    /// 照搬 eframe 0.35：回调在 draw 上下文执行但 create_window 需要 &ActiveEventLoop，
    /// 用 thread_local 存当前 event loop（guard 保证生命周期）。
    static CURRENT_EVENT_LOOP: std::cell::Cell<Option<*const ActiveEventLoop>> = const { std::cell::Cell::new(None) };
}

/// 在事件处理方法入口建立 event loop 上下文，guard drop 时清除。
fn with_event_loop_context(el: &ActiveEventLoop, f: impl FnOnce()) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) { CURRENT_EVENT_LOOP.with(|c| c.set(None)); }
    }
    CURRENT_EVENT_LOOP.with(|c| c.set(Some(std::ptr::from_ref::<ActiveEventLoop>(el))));
    let _guard = Guard;
    f();
}
fn with_current_event_loop<F, R>(f: F) -> Option<R>
where F: FnOnce(&ActiveEventLoop) -> R {
    CURRENT_EVENT_LOOP.with(|c| c.get().map(|ptr| {
        #[allow(unsafe_code)] // 指针由 guard 持有，存活期间有效（eframe 同款）
        unsafe { f(&*ptr) }
    }))
}

struct SubWindow {
    window: Rc<Window>,
    renderer: PlainWindow,
    output: Option<egui::FullOutput>,
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

    /// 消费 immediate viewport 请求：新建/更新子窗口，并保存该帧 output 供绘制。
    fn drain_viewports(&mut self) {
        let reqs = VIEWPORT_REQ.with(|q| std::mem::take(&mut *q.borrow_mut()));
        for (id, window, output) in reqs {
            let wid = window.id();
            if let Some(&existing) = self.sub_ids.get(&id) {
                if let Some(sub) = self.subs.get_mut(&existing) {
                    sub.window = window;
                    sub.output = Some(output);
                }
                continue;
            }
            let Some(rt) = self.runtime.as_ref() else { continue };
            let renderer = match PlainWindow::new(&window, &rt.egui_ctx, false) {
                Ok(r) => r, Err(e) => { eprintln!("[d3d11] PlainWindow failed: {e}"); continue; }
            };
            eprintln!("[winit] sub viewport {id:?} -> window {wid:?}");
            // builder 里 with_visible(false) 防白屏，但 Visible(true) 命令在首帧发出时
            // 窗口尚未创建（drain 在 render 之后）而丢失，这里窗口建好后主动显示。
            window.set_visible(true);
            self.sub_ids.insert(id, wid);
            self.subs.insert(wid, SubWindow { window, renderer, output: Some(output), id });
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
        // 收集子窗口输入（含鼠标/键盘事件）供 immediate 回调使用。
        for sub in self.subs.values_mut() {
            let sin = sub.renderer.egui_winit.take_egui_input(&sub.window);
            VIEWPORT_INPUTS.with(|m| { m.borrow_mut().insert(sub.id, sin); });
        }
        let window = self.window.as_ref().unwrap();
        let rt = self.runtime.as_mut().unwrap();
        let sh = self.shell.as_mut().unwrap();
        let stats: FrameStats = rt.render(window, origin, &params, input, |_ctx, ui| sh.draw(ui));
        // 把主窗口 output 里子 viewport 的窗口命令（Visible(true)/位置/大小）应用到子窗口。
        for (vid, cmds) in stats.viewport_commands {
            if let Some(&wid) = self.sub_ids.get(&vid) {
                if let Some(sub) = self.subs.get_mut(&wid) {
                    egui_winit::process_viewport_commands(&rt.egui_ctx, &mut sub.renderer.viewport_info,
                        cmds, &sub.window, &mut sub.renderer.actions);
                }
            }
        }
        // app 不再 show_viewport 的子窗口（如设置页 settings_open=false）必须销毁，
        // 否则窗口对象残留、右上角 X 关不掉（eframe remove_viewports_not_in 同款）。
        let stale: Vec<egui::ViewportId> = self.sub_ids.keys()
            .filter(|vid| !stats.active_viewports.contains(vid))
            .copied().collect();
        for vid in stale {
            if let Some(wid) = self.sub_ids.remove(&vid) {
                self.subs.remove(&wid);
                self.pending_close.retain(|w| *w != wid);
                VIEWPORT_WINDOWS.with(|m| { m.borrow_mut().remove(&vid); });
                VIEWPORT_INPUTS.with(|m| { m.borrow_mut().remove(&vid); });
            }
        }
        self.drain_viewports();
        // immediate viewport 由父窗口驱动重绘：父每帧 draw 时通过回调拿到子窗口 output，
        // 这里统一画所有子窗口（含可见的）。原来的"只画 invisible"是内嵌 workaround，
        // 独立窗口后必须每帧画，否则内容区空白。
        let subs: Vec<WindowId> = self.subs.keys().copied().collect();
        for wid in subs { self.render_sub_inner(wid); }
    }

    /// 绘制子窗口一帧（RedrawRequested 与"不可见窗口直接绘制"共用）。
    fn render_sub(&mut self, wid: WindowId) {
        self.render_sub_inner(wid);
    }
    fn render_sub_inner(&mut self, wid: WindowId) {
        let Some(rt) = self.runtime.as_ref() else { return };
        let ctx = rt.egui_ctx.clone();
        let Some(sub) = self.subs.get_mut(&wid) else { return };
        // immediate viewport 的 UI 在主窗口 draw 时已同步跑过，这里只画存起来的 output。
        if let Some(output) = sub.output.take() {
            sub.renderer.paint_output(&sub.window, &ctx, sub.id, output);
        }
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

        if let Some(proxy) = REPAINT_PROXY.get() {
            let px = proxy.clone();
            ctx.set_request_repaint_callback(move |_info| { let _ = px.send_event(()); });
        }
        // 让 show_viewport_immediate 走原生独立窗口（egui 默认内嵌会把设置页画到岛仔本体）。
        // 照搬 eframe 0.35：注册 immediate renderer，回调在 draw 上下文同步执行——
        // 用 event_loop_context 取当前 event loop 建窗口，跑 viewport_ui_cb 拿 output 存队列。
        ctx.set_embed_viewports(false);
        egui::Context::set_immediate_viewport_renderer(|egui_ctx, viewport| {
            let egui::ImmediateViewport { ids, builder, mut viewport_ui_cb } = viewport;
            let id = ids.this;
            // 复用已创建的窗口；只有首次或 builder 变化时才 create。
            let need_create = VIEWPORT_WINDOWS.with(|m| {
                m.borrow().get(&id).map(|(_, old)| *old != builder).unwrap_or(true)
            });
            if need_create {
                let attrs = egui_winit::create_winit_window_attributes(egui_ctx, builder.clone());
                let Some(Ok(window)) = with_current_event_loop(|el| el.create_window(attrs)) else {
                    eprintln!("[winit] viewport {id:?} create_window failed");
                    return;
                };
                let window = Rc::new(window);
                egui_winit::apply_viewport_builder_to_window(egui_ctx, &window, &builder);
                VIEWPORT_WINDOWS.with(|m| { m.borrow_mut().insert(id, (window, builder.clone())); });
            }
            let Some(window) = VIEWPORT_WINDOWS.with(|m| m.borrow().get(&id).map(|(w, _)| w.clone())) else { return };
            // 子窗口 input：优先用 window_event 累积的真实 input（含鼠标/键盘），
            // 否则（首帧）构造最小 input。viewport_id/viewports 必须配套，
            // 否则设置页 ctx.input(|i| i.viewport()) 会 panic（egui RawInput::viewport）。
            let mut input = VIEWPORT_INPUTS.with(|m| m.borrow_mut().remove(&id)).unwrap_or_default();
            input.viewport_id = id;
            let ppp = egui_ctx.pixels_per_point();
            if input.screen_rect.is_none() {
                let size = window.inner_size();
                input.screen_rect = Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0),
                    egui::vec2(size.width as f32 / ppp, size.height as f32 / ppp)));
            }
            if !input.viewports.contains_key(&id) {
                input.viewports.insert(id, egui::ViewportInfo {
                    native_pixels_per_point: Some(egui_ctx.native_pixels_per_point().unwrap_or(ppp)),
                    inner_rect: input.screen_rect,
                    ..Default::default()
                });
            }
            if input.time.is_none() { input.time = Some(egui_ctx.input(|i| i.time)); }
            // 右上角 X：把 CloseRequested 转成 egui ViewportEvent::Close，
            // 设置页 UI 的 viewport().close_requested() 读到后置 settings_open=false。
            let close_now = CLOSE_REQ.with(|q| {
                let mut q = q.borrow_mut();
                let hit = q.iter().position(|&v| v == id);
                hit.map(|i| { q.remove(i); })
            });
            if close_now.is_some() {
                input.viewports.entry(id).or_default()
                    .events.push(egui::ViewportEvent::Close);
            }
            // 同步跑子窗口 UI（viewport_ui_cb 借用 app，只能在此调用，不能跨帧存）。
            let output = egui_ctx.run_ui(input, |ui| viewport_ui_cb(ui));
            VIEWPORT_REQ.with(|q| q.borrow_mut().push((id, window, output)));
        });

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
        with_event_loop_context(el, || {
            self.drain_viewports();
            self.draw_main();
        });
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
                // 转成 egui 的 Close 事件喂给设置页 UI（它读 viewport().close_requested()
                // 自行把 settings_open 置 false，下一帧不再 show_viewport 窗口即关闭）。
                let vid = sub.id;
                CLOSE_REQ.with(|q| q.borrow_mut().push(vid));
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
