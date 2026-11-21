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

/// 仿 eframe 官方 event_loop_context：让 show_viewport_immediate 的回调
/// 能拿到当前 ActiveEventLoop 直接创建原生窗口（否则 callback 不触发/embedded）。
mod event_loop_context {
    use std::cell::Cell;
    use winit::event_loop::ActiveEventLoop;
    thread_local! {
        static CURRENT: Cell<Option<*const ActiveEventLoop>> = const { Cell::new(None) };
    }
    struct Guard;
    pub fn with<F: FnOnce()>(el: &ActiveEventLoop, f: F) {
        CURRENT.with(|c| {
            debug_assert!(c.get().is_none());
            c.set(Some(el as *const _));
        });
        let _g = Guard;
        f();
        CURRENT.with(|c| c.set(None));
        let _ = _g;
    }
    pub fn with_current<F: FnOnce(&ActiveEventLoop) -> R, R>(f: F) -> Option<R> {
        CURRENT.with(|c| c.get().map(|p| unsafe { f(&*p) }))
    }
}

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
        static DC: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dc = DC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

    /// 主窗口绘制一帧。必须在 about_to_wait 的事件循环上下文中调用：
    /// show_viewport_immediate 的回调会 create_window，在 RedrawRequested handler 内会重入死锁。
    fn draw_main(&mut self, el: &ActiveEventLoop) {
        static FC: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let fc = FC.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some(w) = self.window.as_ref() else { return };
        let origin = w.inner_position().map(|p| (p.x as f32, p.y as f32)).unwrap_or((0.0, 0.0));
        let params = build_glass_params();
        let input = { let w = self.window.as_ref().unwrap(); self.runtime.as_mut().unwrap().take_input(w) };
        let window = self.window.as_ref().unwrap();
        event_loop_context::with(el, || {
            let rt = self.runtime.as_mut().unwrap();
            let sh = self.shell.as_mut().unwrap();
            let _: FrameStats = rt.render(window, origin, &params, input, |_ctx, ui| sh.draw(ui));
        });
        self.drain_viewports();
        // 不可见子窗口直接绘制（#5229）
        let invisible: Vec<WindowId> = self.subs.iter()
            .filter(|(_, s)| s.window.is_visible() != Some(true))
            .map(|(id, _)| *id).collect();
        for wid in invisible { self.render_sub_direct(el, wid); }
    }

    /// 绘制子窗口一帧（RedrawRequested 与"不可见窗口直接绘制"共用）。
    fn render_sub_direct(&mut self, el: &ActiveEventLoop, wid: WindowId) {
        event_loop_context::with(el, || self.render_sub_inner(wid));
    }
    fn render_sub(&mut self, _el: &ActiveEventLoop, wid: WindowId) {
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
    fn user_event(&mut self, el: &ActiveEventLoop, _event: ()) {
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
        // WaitUntil 到期时 winit 调 new_events（不是 about_to_wait）——驱动每帧绘制。
        if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. })
            || matches!(cause, winit::event::StartCause::Init) {
            self.drain_viewports();
            self.draw_main(el);
            el.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(16)));
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // viewport 请求两处都消费，确保不漏（about_to_wait / RedrawRequested 时机不同）。
        self.drain_viewports();
        self.draw_main(el);
        let mut earliest: Option<Duration> = None;
        if let (Some(w), Some(r)) = (self.window.as_ref(), self.runtime.as_ref()) {
            let d = r.repaint_delay;
            earliest = Some(d);
            if d.is_zero() || d < Duration::from_millis(1) { w.request_redraw(); }
        }
        for sub in self.subs.values() {
            sub.window.request_redraw();
        }
        // 固定 60fps 的 WaitUntil：确保 about_to_wait 每帧都被调用（Poll 模式下 winit
        // 不调 about_to_wait，导致 viewport 队列无人消费）。稳定后可改为按 earliest 睡眠。
        let _ = earliest;
        el.set_control_flow(ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(16)));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
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
        if let WindowEvent::RedrawRequested = event { self.render_sub(el, id); }
    }
}

/// 从 app 当前状态构造 HLSL 玻璃 uniform（岛体胶囊布局）。
fn build_glass_params() -> GlassParams {
    let (w, h) = (crate::app::CANVAS_W, crate::app::WIN_H);
    let mut p = GlassParams::default();
    let pad = 6.0f32;
    p.bounds = [pad, pad, w as f32 - pad * 2.0, h as f32 - pad * 2.0];
    p.params = [18.0, 0.0, 0.0, 0.82];
    p.extra = [18.0, w as f32, h as f32, 0.0];
    p.tint = [0.30, 0.55, 1.0, 0.55];
    p.misc = [w as f32 * 0.5, h as f32 * 0.5, 52.0, 0.0];
    p.look = [2.5, 110.0, 1.6, 0.92];
    p.notch = [0.0, 0.0, 0.0, 1.0];
    p.lightcfg = [1.0, 0.0, 0.0, 0.0];
    // 控件玻璃暂不填（cprm[].y=enabled=0），避免示例矩形出现在岛体外。
    // 后续可把 app 的实际控件矩形通过 set_liquid_optics 桥接进来。
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
