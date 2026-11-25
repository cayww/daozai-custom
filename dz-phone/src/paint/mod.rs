//! 面板内绘制：局部坐标画布 `Card`。
//!
//! `Card` 把「面板局部逻辑点」映射到屏幕，并自带裁剪、整体透明度与可交互开关：
//! 切页时旧页用 `live = false` 的画布绘制，只画不响应点击，也不会和新页抢 id。
//!
//! 子模块：[`icons`] 全部矢量图标、[`chase`] 进度条追光 mesh。两者都用
//! `pub use` 重新导出，调用方仍写 `paint::battery(...)`。

use crate::theme::{font, W};
use eframe::egui::emath::TSTransform;
use eframe::egui::epaint::Shape;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Vec2};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// 文字缩放死区：卡片弹簧缩放倍率在 1±6% 以内时，文字按基准字号画，
/// 不继承尾振。按下时 14% 的放大仍然保留（超过死区）。见 `Card::paint_galley`。
const TEXT_SCALE_DEADZONE: f32 = 0.06;

/// 文字缩放策略。由 [`Card::scaled`] 自动选择 `Auto`，需要时可用
/// [`Card::text_scale`] 显式指定。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextScale {
    /// 默认：没有发生 `scaled()` 时跟随卡片缩放，发生了就按 [`Self::Damped`]。
    #[default]
    Auto,
    /// 完全跟随卡片 `s`（包括弹簧尾振）。截图/整体 zoom 时用。
    Follow,
    /// 隔离弹簧尾振：以最近一次 `scaled()` 之前的字号为基准，
    /// 尾振落在死区内时把文字锁回基准字号；死区外连续跟随卡片缩放。
    Damped,
    /// 冻结在基准字号：入场动画里只让容器动，文字不参与缩放。
    Frozen,
    /// 静态倍率：按最终字号重新排版并缓存，不做 mesh transform。
    /// 只用于逐帧不变的 `k`；弹簧动画请用 `scaled()` 走 `Damped`，否则每帧重排会抖。
    Reflow,
}

/// `Damped` 模式下由卡片缩放比 `k` 求文字缩放比。
///
/// - `|k-1| < 死区`：返回 1（冻结，隔离弹簧尾振）；
/// - `[死区, 2×死区]`：smoothstep 过渡，避免在死区边界把字号硬跳 6%；
/// - 更远：连续跟随 `k`（按压缩放 14% 完整保留）。
///
/// 早期版本在死区外把缩放比量化到 1/64，想让同一档内字形完全静止；但按压松开时
/// 弹簧会在 1.16 → 0.84 之间来回摆，量化让它一档一档跳（60fps 下几乎每帧跨档），
/// 文字看起来上下抽动。量化换来的「静止」只在死区附近有意义，死区本身已经覆盖；
/// 死区外连续跟随是正常缩放，不会再被当成抖动。
fn damped_text_scale(k: f32) -> f32 {
    let d = k - 1.0;
    let ad = d.abs();
    let dead = TEXT_SCALE_DEADZONE;
    if ad <= dead {
        return 1.0;
    }
    let t = ((ad - dead) / dead).min(1.0);
    let t = t * t * (3.0 - 2.0 * t);
    1.0 + d * t
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum H {
    Left,
    Center,
    Right,
}

#[derive(Clone)]
pub struct Card<'a> {
    pub ui: &'a egui::Ui,
    pub painter: egui::Painter,
    pub origin: Pos2,
    pub s: f32,
    pub live: bool,
    /// 构造时的设备像素比。`fill/stroke/rect` 用它把屏幕坐标对齐到物理像素。
    ppp: f32,
    /// 文字缩放策略，默认 `Auto`。
    text_scale: TextScale,
    /// `scaled()` 写入：(不动点局部坐标, 缩放前基准 s)。后写的缩放覆盖先写的，
    /// 即死区只补偿**最内层**那次弹簧（外层翻页缩放仍跟随）。`paint_galley` 用。
    text_anchor: Option<(Pos2, f32)>,
    /// `scaled_text()` 写入：文字**显式**缩放比。按压果冻这类会来回振的弹簧只放大
    /// 玻璃、不写进字形时用它；`None` 表示文字跟随卡片 `s`（`Auto` 再叠加死区）。
    text_k: Option<f32>,
}

impl<'a> Card<'a> {
    pub fn new(ui: &'a egui::Ui, origin: Pos2, clip: Rect) -> Self {
        let painter = ui.painter().with_clip_rect(clip);
        let ppp = ui.ctx().pixels_per_point().max(0.1);
        Self {
            ui,
            painter,
            origin,
            s: 1.0,
            live: true,
            ppp,
            text_scale: TextScale::Auto,
            text_anchor: None,
            text_k: None,
        }
    }

    pub fn pos(&self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x, y) * self.s
    }
    /// 逻辑矩形 → 屏幕矩形，并把边缘对齐到物理像素边界。
    ///
    /// 填充块/分隔线在任意 `s` 下都落半像素，是控件「边缘闪烁」的主因：
    /// 同一条边在相邻两帧分别占 1 个像素和 2×50%，渲染出来就忽粗忽细。
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        let r = Rect::from_min_size(self.pos(x, y), Vec2::new(w, h) * self.s);
        self.snap_rect(r, w > 0.0, h > 0.0)
    }
    /// 屏幕坐标 → 最近的像素边界（整像素对齐，用于填充/裁剪）。
    pub fn snap(&self, v: f32) -> f32 {
        (v * self.ppp).round() / self.ppp
    }
    /// 屏幕坐标 → 最近的像素中心（1px 描边落在这里才不会被两个像素各吃一半）。
    pub fn snap_center(&self, v: f32) -> f32 {
        ((v * self.ppp - 0.5).round() + 0.5) / self.ppp
    }
    fn snap_rect(&self, r: Rect, keep_w: bool, keep_h: bool) -> Rect {
        let px = 1.0 / self.ppp;
        let (x0, y0) = (self.snap(r.min.x), self.snap(r.min.y));
        let (mut x1, mut y1) = (self.snap(r.max.x), self.snap(r.max.y));
        // 细到不足一个像素的填充（入场动画里的小控件）不能被对齐吞成 0 宽。
        if keep_w && x1 <= x0 + px * 0.5 {
            x1 = x0 + px;
        }
        if keep_h && y1 <= y0 + px * 0.5 {
            y1 = y0 + px;
        }
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    }
    /// 局部坐标 → 屏幕坐标，再对齐到像素边界。
    pub fn snapped_pos(&self, x: f32, y: f32) -> Pos2 {
        let p = self.pos(x, y);
        Pos2::new(self.snap(p.x), self.snap(p.y))
    }
    pub fn to_local(&self, p: Pos2) -> Pos2 {
        let v = (p - self.origin) / self.s;
        Pos2::new(v.x, v.y)
    }

    // ---------- 派生画布 ----------
    pub fn faded(&self, alpha: f32) -> Self {
        let mut c = self.clone();
        c.painter.multiply_opacity(alpha.clamp(0.0, 1.0));
        c
    }
    pub fn shifted(&self, dx: f32, dy: f32) -> Self {
        let mut c = self.clone();
        c.origin += Vec2::new(dx, dy) * self.s;
        c
    }
    /// 以局部点 (cx, cy) 为中心整体缩放（按钮按下的回弹）。
    /// 记录不动点与基准 s：文字绘制时按 [`TextScale::Auto`] 对小幅弹簧尾振加死区，
    /// 避免 damping ζ 通过 s 把振荡写进字形（用户实测 ζ 调大文字就不抖）。
    pub fn scaled(&self, cx: f32, cy: f32, k: f32) -> Self {
        let mut c = self.clone();
        let center = self.pos(cx, cy);
        c.origin = center + (self.origin - center) * k;
        c.s = self.s * k;
        c.text_anchor = Some((Pos2::new(cx, cy), self.s));
        c.text_k = None;
        // Reflow 只适用于逐帧不变的 k；若再套一层弹簧，必须退回 Damped，
        // 否则逐帧变化的 s 会触发 layout_job 每帧重排（抖动 + 图集开销）。
        if c.text_scale == TextScale::Reflow {
            c.text_scale = TextScale::Auto;
        }
        c
    }
    /// 静态缩放：容器按 `k` 放大，文字按最终字号重新排版后直接画在屏幕上。
    ///
    /// 与 [`Self::scaled`] 的区别：`k` 必须逐帧不变，`galley_shape()` 会以
    /// `font(size * s)` 重排一次并命中 egui 的 galley 缓存，因此不会产生
    /// 「放大位图字形 → 采到图集相邻黑像素」的黑线，也不需要任何 UV 补丁。
    /// 弹簧/逐帧变化的 `k` 请继续用 `scaled()`，否则每帧重排会抖。
    pub fn scaled_static(&self, cx: f32, cy: f32, k: f32) -> Self {
        let mut c = self.clone();
        let center = self.pos(cx, cy);
        c.origin = center + (self.origin - center) * k;
        c.s = self.s * k;
        c.text_scale = TextScale::Reflow;
        c.text_anchor = None;
        c.text_k = None;
        c
    }
    /// 卡片按 `k_shape` 缩放，但文字只按 `k_text` 缩放。
    ///
    /// 按压果冻（14%、ζ≈0.5）会让文字 mesh 每帧被不同的 `k` 重采样一遍，
    /// 位图字形在连续缩放下的边缘覆盖率一直在变，肉眼就是「上下抖」。
    /// 所以让玻璃吃满弹簧、文字只跟入场这类单次、无回振的动画：
    /// `k_text` 通常直接传入场缩放，按压时恒为 1，字形 mesh 逐帧完全一致。
    pub fn scaled_text(&self, cx: f32, cy: f32, k_shape: f32, k_text: f32) -> Self {
        let mut c = self.clone();
        let center = self.pos(cx, cy);
        c.origin = center + (self.origin - center) * k_shape;
        c.s = self.s * k_shape;
        c.text_anchor = Some((Pos2::new(cx, cy), self.s));
        c.text_k = Some(k_text);
        c
    }
    /// 显式指定文字缩放策略（默认 `Auto`，见 [`TextScale`]）。
    pub fn text_scale(&self, mode: TextScale) -> Self {
        let mut c = self.clone();
        c.text_scale = mode;
        c
    }
    pub fn ghost(&self) -> Self {
        let mut c = self.clone();
        c.live = false;
        c
    }
    pub fn clipped(&self, x: f32, y: f32, w: f32, h: f32) -> Self {
        let mut c = self.clone();
        let r = self.rect(x, y, w, h).intersect(self.painter.clip_rect());
        c.painter.set_clip_rect(r);
        c
    }

    /// 圆环轨道（插件状态进度用，上游 v0.6.70）。
    pub fn track(&self, cx: f32, cy: f32, r: f32, width: f32, color: egui::Color32) {
        let pts: Vec<egui::Pos2> = (0..96)
            .map(|i| {
                let a = i as f32 / 96.0 * std::f32::consts::TAU;
                self.pos(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        self.painter.add(egui::Shape::line(pts, egui::Stroke::new(width, color)));
    }
    // ---------- 文字（y 一律是基线，跨字体对齐稳定） ----------
    //
    // 字体缩放策略（2026-11-22 重构，彻底解决液态皮肤弹簧动画文字抖动）：
    // galley 一律按固定字号 `size` 排版（不含 s），字形只栅格化一次、缓存稳定；
    // 卡片缩放 s 由 Shape::transform 在绘制时缩放 mesh 顶点承担，不重新排版。
    // 因此 galley.size()/mesh_bounds 天然是「局部单位」，调用方无需再 /s。
    // 参考：egui#3197、Bevy B0005「If you need to smoothly scale font size,
    // use Transform::scale」。
    pub fn font(&self, size: f32, w: W) -> FontId {
        font(size, w)
    }
    pub fn galley(&self, t: &str, size: f32, w: W, color: Color32) -> std::sync::Arc<egui::Galley> {
        self.painter.layout_no_wrap(
            crate::rtl_text::display(t).into_owned(),
            font(size, w),
            color,
        )
    }
    /// 在局部坐标 p（未缩放）绘制已排版的 galley：先缩放 mesh 到屏幕，再平移 origin。
    ///
    /// 缩放策略见 [`TextScale`]：默认 `Auto` 在 `scaled()` 之后等价于 `Damped` ——
    /// 弹簧尾振（|k-1| < 死区）时文字锁回基准缩放、但保持不动点的屏幕位置不变，
    /// 于是玻璃/卡片仍随阻尼比弹，文字不再跟着胀缩。死区外连续跟随卡片缩放
    /// （不做量化——量化会让松手回弹变成一格一格跳，看起来像文字上下抽动）。
    pub fn paint_galley(&self, p: Pos2, g: std::sync::Arc<egui::Galley>, color: Color32) {
        let shape = self.galley_shape(p, g, color);
        self.painter.add(shape);
    }
    /// [`Self::paint_galley`] 的纯函数版本：返回已按当前 `s` 变形的文字图元。
    ///
    /// 拆出来是为了让测试能逐位比较「不同弹簧相位下字形 mesh 是否稳定」——
    /// 抖动的本质就是同一段文字在相邻帧生成了不同顶点。
    pub fn galley_shape(&self, p: Pos2, g: std::sync::Arc<egui::Galley>, color: Color32) -> Shape {
        // 静态倍率：按最终字号重排（LayoutJob 完整参与缓存键，固定 k 会命中缓存），
        // 直接画在 self.pos(p) 上；不做 mesh transform。
        if matches!(self.text_scale, TextScale::Reflow) {
            let s = self.s;
            if (s - 1.0).abs() < 1e-4 {
                return Shape::galley(self.pos(p.x, p.y), g, color);
            }
            let mut job = (*g.job).clone();
            for section in &mut job.sections {
                section.format.font_id.size *= s;
            }
            job.wrap.max_width *= s;
            let reflowed = self.ui.ctx().fonts_mut(|f| f.layout_job(job));
            return Shape::galley(self.pos(p.x, p.y), reflowed, color);
        }
        let (mut origin, mut s) = (self.origin.to_vec2(), self.s);
        let mode = match self.text_scale {
            TextScale::Auto if self.text_anchor.is_some() => TextScale::Damped,
            TextScale::Auto => TextScale::Follow,
            explicit => explicit,
        };
        if let (Some((pivot, base_s)), true) = (
            self.text_anchor,
            matches!(mode, TextScale::Damped | TextScale::Frozen),
        ) {
            let base_s = base_s.max(1e-6);
            let k = s / base_s;
            let k_text = match (mode, self.text_k) {
                (TextScale::Frozen, _) => 1.0,
                (_, Some(explicit)) => explicit,
                (TextScale::Damped, None) => damped_text_scale(k),
                _ => k,
            };
            if (k_text - k).abs() > 1e-6 {
                let pv = origin + pivot.to_vec2() * s;
                s = base_s * k_text;
                origin = pv - pivot.to_vec2() * s;
            }
        }
        let mut shape = Shape::galley(p, g, color);
        shape.transform(TSTransform::new(origin, s));
        shape
    }
    pub fn text_w(&self, t: &str, size: f32, w: W) -> f32 {
        self.galley(t, size, w, Color32::WHITE).size().x
    }
    /// 返回局部坐标下的文字宽度。
    pub fn text(&self, x: f32, base: f32, t: &str, size: f32, w: W, color: Color32, h: H) -> f32 {
        let g = self.galley(t, size, w, color);
        let gw = g.size().x;
        let asc = g
            .rows
            .first()
            .and_then(|r| r.glyphs.first())
            .map(|g| g.pos.y)
            .unwrap_or(size);
        let mut p = Pos2::new(x, base) - Vec2::new(0.0, asc);
        match h {
            H::Left => {}
            H::Center => p.x -= gw / 2.0,
            H::Right => p.x -= gw,
        }
        self.paint_galley(p, g, color);
        gw
    }
    /// 以字形的实际可见范围对齐垂直中心，保持中英文状态文字居中。
    pub fn centered_text(&self, x: f32, cy: f32, t: &str, size: f32, w: W, color: Color32, h: H) {
        if t.is_empty() {
            return;
        }
        let g = self.galley(t, size, w, color);
        let dx = match h {
            H::Left => 0.0,
            H::Center => g.size().x / 2.0,
            H::Right => g.size().x,
        };
        let pos = Pos2::new(x, cy) - Vec2::new(dx, g.mesh_bounds.center().y);
        self.paint_galley(pos, g, color);
    }
    /// 超宽时末尾加省略号。
    pub fn fit(&self, t: &str, size: f32, w: W, max_w: f32) -> String {
        if self.text_w(t, size, w) <= max_w {
            return t.to_owned();
        }
        let chars: Vec<char> = t.chars().collect();
        let (mut low, mut high) = (0, chars.len().saturating_sub(1));
        while low < high {
            let n = low + (high - low + 1) / 2;
            let s: String = chars[..n].iter().collect::<String>() + "…";
            if self.text_w(&s, size, w) <= max_w {
                low = n;
            } else {
                high = n - 1;
            }
        }
        chars[..low].iter().collect::<String>() + "…"
    }

    /// 按宽度折行，最多 `max_lines` 行，还有剩余内容时最后一行加省略号。
    ///
    /// 每一行用**二分查找**定位折行点（每行 O(log n) 次排版），而不是逐字符
    /// 把整行反复重排（旧实现每加一个字符就测一次整行宽度，长错误信息是 O(n²)）。
    /// 贪心语义不变：行内尽量放满、单字符超宽时也至少保留一个字符，
    /// `\n` 强制换行且保留空行，被 `max_lines` 截断时最后一行走 `fit`。
    pub fn wrap(&self, t: &str, size: f32, w: W, max_w: f32, max_lines: usize) -> Vec<String> {
        if max_lines == 0 {
            return Vec::new();
        }
        let chars: Vec<char> = t.chars().collect();
        let total = chars.len();
        let mut out: Vec<String> = Vec::new();
        let mut start = 0usize;
        while start < total && out.len() < max_lines {
            if chars[start] == '\n' {
                out.push(String::new());
                start += 1;
                continue;
            }
            // 本行最远到下一个换行为止（换行符本身不占行宽）。
            let limit = chars[start..]
                .iter()
                .position(|&c| c == '\n')
                .map_or(total, |i| start + i);
            // 二分最大的「宽度仍 ≤ max_w」前缀；至少保留 1 个字符。
            let (mut lo, mut hi) = (start + 2, limit);
            let mut n = start + 1;
            while lo <= hi {
                let mid = lo + (hi - lo) / 2;
                let line: String = chars[start..mid].iter().collect();
                if self.text_w(&line, size, w) <= max_w {
                    n = mid;
                    lo = mid + 1;
                } else {
                    hi = mid - 1;
                }
            }
            out.push(chars[start..n].iter().collect());
            start = if n == limit && limit < total {
                limit + 1
            } else {
                n
            };
        }
        // 行数用尽仍有剩余内容（含刚被挤出的换行）：最后一行加省略号。
        if out.len() == max_lines && start < total {
            if let Some(last) = out.last_mut() {
                *last = self.fit(last, size, w, max_w);
            }
        }
        out
    }

    // ---------- 图形 ----------
    pub fn fill(&self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color32) {
        self.painter
            .rect_filled(self.rect(x, y, w, h), r * self.s, color);
    }
    pub fn stroke(&self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, color: Color32) {
        let rc = self.stroke_rect(x, y, w, h, width);
        self.painter.rect_stroke(
            rc,
            r * self.s,
            Stroke::new(width * self.s, color),
            egui::StrokeKind::Middle,
        );
    }
    /// 描边路径矩形：1~2 物理像素的细线落到像素中心（线才正好盖满一整行/列像素），
    /// 更粗的线保持边界对齐，避免整条边凭空偏半像素。
    fn stroke_rect(&self, x: f32, y: f32, w: f32, h: f32, width: f32) -> Rect {
        let wd = width * self.s;
        let rc = self.rect(x, y, w, h);
        if wd * self.ppp <= 2.0 {
            Rect::from_min_max(
                Pos2::new(self.snap_center(rc.min.x), self.snap_center(rc.min.y)),
                Pos2::new(self.snap_center(rc.max.x), self.snap_center(rc.max.y)),
            )
        } else {
            rc
        }
    }
    pub fn circle(&self, x: f32, y: f32, r: f32, color: Color32) {
        self.painter
            .circle_filled(self.snapped_pos(x, y), r * self.s, color);
    }
    pub fn ring(&self, x: f32, y: f32, r: f32, width: f32, color: Color32) {
        // 圆心对齐到像素中心：静态圆点/圆环在不同缩放下的覆盖比例才稳定。
        // 半径保持连续，动画中的进度圆环不会被量化成 1px 台阶。
        self.painter.circle_stroke(
            self.snapped_pos(x, y),
            r * self.s,
            Stroke::new(width * self.s, color),
        );
    }
    /// 线段端点：轴对齐且 ≤2 物理像素宽时，横向坐标落到像素中心，
    /// 1px 分隔线/图标竖笔正好盖满一整列像素；斜线原样保留（硬对齐会破坏斜率）。
    fn seg_points(&self, a: (f32, f32), b: (f32, f32), width: f32) -> [Pos2; 2] {
        let mut p0 = self.pos(a.0, a.1);
        let mut p1 = self.pos(b.0, b.1);
        if width * self.s * self.ppp <= 2.0 {
            if (p0.x - p1.x).abs() < 0.01 {
                let x = self.snap_center((p0.x + p1.x) * 0.5);
                p0.x = x;
                p1.x = x;
            }
            if (p0.y - p1.y).abs() < 0.01 {
                let y = self.snap_center((p0.y + p1.y) * 0.5);
                p0.y = y;
                p1.y = y;
            }
        }
        [p0, p1]
    }
    pub fn seg(&self, a: (f32, f32), b: (f32, f32), width: f32, color: Color32) {
        let [p0, p1] = self.seg_points(a, b, width);
        self.painter
            .line_segment([p0, p1], Stroke::new(width * self.s, color));
    }
    pub fn path(&self, pts: &[(f32, f32)], width: f32, color: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pos(x, y)).collect();
        self.painter
            .add(egui::Shape::line(v, Stroke::new(width * self.s, color)));
    }
    pub fn poly(&self, pts: &[(f32, f32)], color: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.pos(x, y)).collect();
        self.painter
            .add(egui::Shape::convex_polygon(v, color, Stroke::NONE));
    }
    /// 圆弧描边（角度单位弧度，0 = 正右，顺时针为正）。
    pub fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, width: f32, color: Color32) {
        let n = (((a1 - a0).abs() / TAU) * 96.0).ceil().max(2.0) as usize;
        let pts: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        self.path(&pts, width, color);
    }
    /// 径向柔光（中心 color → 边缘透明）。
    pub fn glow(&self, cx: f32, cy: f32, r: f32, color: Color32) {
        let mut mesh = egui::Mesh::default();
        let c = self.pos(cx, cy);
        mesh.colored_vertex(c, color);
        let n = 48;
        for i in 0..=n {
            let a = i as f32 / n as f32 * TAU;
            // 中间加一圈 45% 处的半透明，近似高斯衰减
            mesh.colored_vertex(
                c + Vec2::angled(a) * r * 0.45 * self.s,
                color.gamma_multiply(0.45),
            );
            mesh.colored_vertex(c + Vec2::angled(a) * r * self.s, Color32::TRANSPARENT);
        }
        for i in 0..n as u32 {
            let (m0, o0, m1, o1) = (1 + i * 2, 2 + i * 2, 3 + i * 2, 4 + i * 2);
            mesh.add_triangle(0, m0, m1);
            mesh.add_triangle(m0, o0, o1);
            mesh.add_triangle(m0, o1, m1);
        }
        self.painter.add(egui::Shape::mesh(mesh));
    }
    /// 圆角竖直渐变，沿用 egui 的边缘抗锯齿。
    pub fn vgrad(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        top: Color32,
        bottom: Color32,
    ) {
        let r = self.rect(x, y, w, h);
        let shapes = vec![egui::epaint::ClippedShape {
            clip_rect: self.painter.clip_rect(),
            shape: egui::Shape::rect_filled(r, radius * self.s, Color32::WHITE),
            transform_after_tessellation: egui::epaint::emath::TSTransform::IDENTITY,
        }];
        for primitive in self
            .ui
            .ctx()
            .tessellate(shapes, self.ui.ctx().pixels_per_point())
        {
            if let egui::epaint::Primitive::Mesh(mut mesh) = primitive.primitive {
                mesh.texture_id = egui::TextureId::default();
                for vertex in &mut mesh.vertices {
                    let t = ((vertex.pos.y - r.top()) / r.height()).clamp(0.0, 1.0);
                    vertex.color = alpha(mix(top, bottom, t), vertex.color.a() as f32 / 255.0);
                }
                self.painter.add(egui::Shape::mesh(mesh));
            }
        }
    }
    /// 虚线圆角框；`phase` 让虚线沿边「流动」。
    pub fn dashed(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        width: f32,
        color: Color32,
        phase: f32,
    ) {
        let mut pts = Vec::new();
        let corner = |cx: f32, cy: f32, a0: f32, pts: &mut Vec<Pos2>| {
            for i in 0..=8 {
                let a = a0 + FRAC_PI_2 * i as f32 / 8.0;
                pts.push(self.pos(cx + r * a.cos(), cy + r * a.sin()));
            }
        };
        corner(x + w - r, y + r, -FRAC_PI_2, &mut pts);
        corner(x + w - r, y + h - r, 0.0, &mut pts);
        corner(x + r, y + h - r, FRAC_PI_2, &mut pts);
        corner(x + r, y + r, PI, &mut pts);
        pts.push(pts[0]);
        let dash = 4.0 * self.s;
        let gap = 3.5 * self.s;
        let off = (phase * self.s).rem_euclid(dash + gap);
        let shapes = egui::Shape::dashed_line_with_offset(
            &pts,
            Stroke::new(width * self.s, color),
            &[dash],
            &[gap],
            off,
        );
        self.painter.extend(shapes);
    }

    // ---------- 交互 ----------
    pub fn hit(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> Response {
        let rect = self.rect(x, y, w, h).intersect(self.painter.clip_rect());
        let sense = if self.live {
            Sense::click()
        } else {
            Sense::hover()
        };
        let id = egui::Id::unique(("card", self.live, id));
        self.ui.interact(rect, id, sense)
    }

    pub fn hit_drag(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        id: impl std::hash::Hash + std::fmt::Debug,
    ) -> Response {
        let rect = self.rect(x, y, w, h).intersect(self.painter.clip_rect());
        self.ui.interact(
            rect,
            egui::Id::unique(("card-drag", self.live, id)),
            if self.live {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        )
    }
    /// 悬停 / 按下的平滑过渡量（0..1）。
    pub fn hover_t(&self, r: &Response) -> (f32, f32) {
        let ctx = self.ui.ctx();
        let h = ctx.animate_bool_with_time(r.id.with("h"), self.live && r.hovered(), 0.14);
        let p = ctx.animate_bool_with_time(
            r.id.with("p"),
            self.live && r.is_pointer_button_down_on(),
            0.07,
        );
        (h, p)
    }
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a.clamp(0.0, 1.0))
}

#[cfg(test)]
mod gradient_tests {
    use super::*;
    #[test]
    fn regression_play_icon_has_no_internal_seam() {
        for ppp in [1.0, 1.25, 1.5, 2.0] {
            for scale in [0.9, 1.0, 1.4] {
                for offset in [0.0, 0.25, 0.5, 0.75] {
                    for (size, color) in [11.0, 15.0].into_iter().flat_map(|size| {
                        [
                            Color32::WHITE,
                            Color32::from_gray(22),
                            Color32::from_white_alpha(128),
                        ]
                        .map(|color| (size, color))
                    }) {
                        let ctx = egui::Context::default();
                        ctx.set_zoom_factor(ppp);
                        let center = Pos2::new(30.0 + offset, 26.0 + offset);
                        let raw = egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(96.0))),
                            ..Default::default()
                        };
                        let output = ctx.run_ui(raw, |ui| {
                            let card = Card::new(ui, Pos2::ZERO, ui.max_rect())
                                .scaled(center.x, center.y, scale);
                            play_pause(&card, center.x, center.y, size, 0.0, color);
                        });
                        let mut raster = crate::preview_raster::Raster::new(
                            (96.0 * ppp) as usize,
                            (96.0 * ppp) as usize,
                        );
                        raster.update_textures(&output.textures_delta);
                        raster.draw(&ctx.tessellate(output.shapes, ppp), ppp);
                        let y = (center.y * ppp).floor() as usize;
                        // egui 0.37 applies clip_rect strictly; keep the canvas wide enough
                        // that all sampled triangle interior points stay inside it.
                        for dx in [-0.5, 0.0, 0.5, 1.0, 1.5] {
                            let x = ((center.x + dx * scale) * ppp).floor() as usize;
                            let expected = color.a() as f32 / 255.0;
                            assert!((raster.buf[y*raster.w+x][3]-expected).abs()<0.02,"播放三角形内部应连续：ppp={ppp}, scale={scale}, offset={offset}, dx={dx}, alpha={}",raster.buf[y*raster.w+x][3]);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn long_email_links_do_not_create_hundreds_of_layouts() {
        let ctx = egui::Context::default();
        let text = format!("https://example.com/{}", "newsletter-link-".repeat(40));
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
                let before = ui.ctx().fonts(|f| f.num_galleys_in_cache());
                let fitted = c.fit(&text, 10.5, W::Regular, 180.0);
                assert!(fitted.ends_with('…'));
                assert!(c.text_w(&fitted, 10.5, W::Regular) <= 180.0);
                let layouts = ui.ctx().fonts(|f| f.num_galleys_in_cache()) - before;
                assert!(
                    layouts <= 20,
                    "长邮件链接只需少量排版，不应逐字重排：{layouts}"
                );
                println!("长链接排版次数：{layouts}");
            });
        });
    }
    #[test]
    fn text_fitting_preserves_short_text_and_unicode_results() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
                for text in [
                    "",
                    "a",
                    "收到新的邮件与验证码",
                    "会议提醒 🌴 完整标题",
                    "https://example.com/short",
                ] {
                    for width in [0.0, 10.0, 40.0, 80.0, 300.0] {
                        let expected = if c.text_w(text, 10.5, W::Regular) <= width {
                            text.into()
                        } else {
                            let chars: Vec<_> = text.chars().collect();
                            (1..chars.len())
                                .rev()
                                .map(|n| chars[..n].iter().collect::<String>() + "…")
                                .find(|s| c.text_w(s, 10.5, W::Regular) <= width)
                                .unwrap_or_else(|| "…".into())
                        };
                        assert_eq!(
                            c.fit(text, 10.5, W::Regular, width),
                            expected,
                            "宽度 {width} 的文字裁剪应保持原来的显示结果"
                        );
                    }
                }
            });
        });
    }
    #[test]
    fn cover_gradient_keeps_all_four_corners_clear() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0))),
            ..Default::default()
        };
        let output = ctx.run_ui(raw, |ui| {
            let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
            c.vgrad(20.0, 20.0, 62.0, 62.0, 13.0, Color32::RED, Color32::BLUE);
        });
        let meshes: Vec<_> = output
            .shapes
            .into_iter()
            .filter_map(|s| {
                if let egui::Shape::Mesh(m) = s.shape {
                    Some(m)
                } else {
                    None
                }
            })
            .collect();
        let covers = |point: Pos2| {
            meshes.iter().any(|m| {
                m.indices.chunks_exact(3).any(|tri| {
                    let [a, b, c] = [
                        m.vertices[tri[0] as usize].pos,
                        m.vertices[tri[1] as usize].pos,
                        m.vertices[tri[2] as usize].pos,
                    ];
                    let cross = |a: Pos2, b: Pos2| {
                        (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x)
                    };
                    let signs = [cross(a, b), cross(b, c), cross(c, a)];
                    signs.iter().all(|s| *s >= 0.0) || signs.iter().all(|s| *s <= 0.0)
                })
            })
        };
        assert!(covers(Pos2::new(51.0, 51.0)), "封面中心需要被绘制");
        for (x, y) in [(20.5, 20.5), (81.5, 20.5), (20.5, 81.5), (81.5, 81.5)] {
            assert!(!covers(Pos2::new(x, y)), "渐变不能覆盖圆角之外的区域");
        }
    }
    #[test]
    fn text_scale_damps_tail_without_boundary_jump() {
        // 死区内冻结在基准字号。
        assert_eq!(damped_text_scale(1.0), 1.0);
        assert_eq!(damped_text_scale(1.0 + TEXT_SCALE_DEADZONE * 0.9), 1.0);
        assert_eq!(damped_text_scale(1.0 - TEXT_SCALE_DEADZONE * 0.9), 1.0);
        // 死区边界连续：不能从 1.0 硬跳到 1.06。
        let lo = damped_text_scale(1.0 + TEXT_SCALE_DEADZONE - 1e-4);
        let hi = damped_text_scale(1.0 + TEXT_SCALE_DEADZONE + 1e-4);
        assert!((lo - hi).abs() <= 1e-3, "死区边界跳变：{lo} → {hi}");
        // 按压缩放（14%）必须完整保留，不被死区吞掉。
        assert!((damped_text_scale(1.14) - 1.14).abs() < 1e-3);
        assert!(damped_text_scale(0.82) < 0.9);
        // 死区外必须连续单调：松手回弹时一档一档跳会被看成文字上下抽动。
        let steps: Vec<f32> = (0..60)
            .map(|i| 1.06 + 0.14 * i as f32 / 59.0)
            .map(damped_text_scale)
            .collect();
        for pair in steps.windows(2) {
            assert!((pair[1] - pair[0]).abs() <= 0.01, "缩放比应连续：{pair:?}");
        }
    }

    /// 死区内，字形 mesh 必须逐位不变——相邻帧顶点不同=每帧重采样=抖动。
    #[test]
    fn scaled_text_mesh_holds_still_inside_deadzone() {
        fn mesh_coords(shape: &Shape) -> Vec<[i64; 2]> {
            // 行内顶点坐标（已含缩放）转定点，避开浮点相等噪声。
            let Shape::Text(t) = shape else {
                panic!("期望文字图元")
            };
            let q = |v: f32| (v as f64 * 4096.0).round() as i64;
            let mut out = Vec::new();
            for row in &t.galley.rows {
                out.push([q(row.pos.x), q(row.pos.y)]);
                out.push([
                    q(row.visuals.mesh_bounds.min.x),
                    q(row.visuals.mesh_bounds.min.y),
                ]);
                for v in &row.visuals.mesh.vertices {
                    out.push([q(v.pos.x), q(v.pos.y)]);
                }
            }
            out
        }
        for ppp in [1.0f32, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(ppp);
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))),
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
                let g = c.galley("岛仔 daozai 12:34", 18.0, W::Regular, Color32::WHITE);
                let at = |k: f32| {
                    let cc = c.scaled(40.0, 24.0, k);
                    mesh_coords(&cc.galley_shape(Pos2::new(4.0, 14.0), g.clone(), Color32::WHITE))
                };
                assert_eq!(
                    at(1.0),
                    at(1.0 + TEXT_SCALE_DEADZONE * 0.5),
                    "ppp={ppp} 死区内字形不应变化"
                );
                // 死区外连续跟随：更小的弹簧相位差对应更小的顶点位移，
                // 而不是「同格完全相等、跨格整档跳」。
                assert_ne!(at(1.2), at(1.2 + 0.002), "ppp={ppp} 死区外应连续跟随缩放");
                assert_ne!(at(1.2), at(1.24), "ppp={ppp} 更大缩放应有更大差异");
            });
        }
    }

    /// `scaled_text()`：玻璃吃满弹簧、文字只跟 `k_text` —— 按压缩放变化时字形必须逐位不变。
    #[test]
    fn scaled_text_ignores_press_spring_when_text_k_fixed() {
        fn coords(shape: &Shape) -> Vec<[i64; 2]> {
            let Shape::Text(t) = shape else {
                panic!("期望文字图元")
            };
            let q = |v: f32| (v as f64 * 4096.0).round() as i64;
            let mut out = Vec::new();
            for row in &t.galley.rows {
                out.push([q(row.pos.x), q(row.pos.y)]);
                for v in &row.visuals.mesh.vertices {
                    out.push([q(v.pos.x), q(v.pos.y)]);
                }
            }
            out
        }
        for ppp in [1.0f32, 1.5] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(ppp);
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))),
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
                let g = c.galley("＋ 新增", 9.0, W::Regular, Color32::WHITE);
                // 按压弹簧从 1.0 涨到 1.14、再回 0.9：文字 k 固定 1.0，字形必须一格不动。
                let at = |k_shape: f32, k_text: f32| {
                    let cc = c.scaled_text(124.0, 18.0, k_shape, k_text);
                    coords(&cc.galley_shape(Pos2::new(4.0, 14.0), g.clone(), Color32::WHITE))
                };
                assert_eq!(
                    at(1.0, 1.0),
                    at(1.14, 1.0),
                    "ppp={ppp} 按压放大不应写进字形"
                );
                assert_eq!(
                    at(1.14, 1.0),
                    at(0.9, 1.0),
                    "ppp={ppp} 回弹缩小同样不写字形"
                );
                assert_ne!(
                    at(1.0, 1.0),
                    at(1.0, 1.08),
                    "ppp={ppp} 显式 k_text 仍要放大文字"
                );
            });
        }
    }

    #[test]
    fn static_scaled_text_reflows_at_final_size_and_caches() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
            let base = c.galley("静态缩放", 12.0, W::Regular, Color32::WHITE);
            let shape = || {
                c.scaled_static(40.0, 24.0, 1.2)
                    .galley_shape(Pos2::new(4.0, 14.0), base.clone(), Color32::WHITE)
            };
            let a = shape();
            let b = shape();
            let (Shape::Text(ta), Shape::Text(tb)) = (&a, &b) else {
                panic!("期望文字图元")
            };
            assert!(
                std::sync::Arc::ptr_eq(&ta.galley, &tb.galley),
                "固定倍率必须命中 galley 缓存"
            );
            let size = ta.galley.job.sections[0].format.font_id.size;
            assert!((size - 14.4).abs() < 1e-3, "应按最终字号重排：{size}");
        });
    }

    #[test]
    fn scaled_after_static_reflow_falls_back_to_damped() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
            let g = c.galley("静态后弹簧", 12.0, W::Regular, Color32::WHITE);
            // Reflow 卡片若再套一层弹簧，必须退回 Damped，不能按逐帧变化的 s 重排。
            let cc = c.scaled_static(40.0, 24.0, 1.2).scaled(40.0, 24.0, 1.02);
            let shape = cc.galley_shape(Pos2::new(4.0, 14.0), g, Color32::WHITE);
            let Shape::Text(t) = shape else {
                panic!("期望文字图元")
            };
            assert_eq!(
                t.galley.job.sections[0].format.font_id.size,
                12.0,
                "Reflow 后再 scaled() 必须退回 Damped，字体不能按变化后的 s 重排"
            );
        });
    }

    #[test]
    fn wrap_matches_char_by_char_reference() {
        // 旧的逐字符算法：只作为新二分折行的行为基准。
        fn reference(
            c: &Card,
            t: &str,
            size: f32,
            w: W,
            max_w: f32,
            max_lines: usize,
        ) -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            let mut cur = String::new();
            let mut used = 0usize;
            let mut total = 0usize;
            for ch in t.chars() {
                total += 1;
                if ch == '\n' {
                    used = total;
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.push(ch);
                    if c.text_w(&cur, size, w) > max_w && cur.chars().count() > 1 {
                        cur.pop();
                        used = total - 1;
                        out.push(std::mem::take(&mut cur));
                        cur.push(ch);
                    }
                }
                if out.len() == max_lines {
                    break;
                }
            }
            if out.len() < max_lines && !cur.is_empty() {
                used = total;
                out.push(cur);
            }
            if out.len() == max_lines && used < t.chars().count() {
                if let Some(last) = out.last_mut() {
                    *last = c.fit(last, size, w, max_w);
                }
            }
            out
        }
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
                let texts = [
                    "收到新的邮件与验证码",
                    "https://example.com/very-long/path?query=1&x=2",
                    "a\n\nb\nc",
                    "\nabc",
                    "换行测试：第一行\n第二行很长很长很长很长的内容",
                    "🎵🌊 表情与中文混排测试",
                    "",
                    "x",
                    "长链接 https://example.com/a/b/c/d/e/f/g 混排中文与英文和数字 1234567890",
                ];
                for t in texts {
                    for max_w in [1.0f32, 20.0, 40.0, 80.0, 160.0, 400.0] {
                        for max_lines in [1usize, 2, 3, 5] {
                            assert_eq!(
                                c.wrap(t, 10.5, W::Regular, max_w, max_lines),
                                reference(&c, t, 10.5, W::Regular, max_w, max_lines),
                                "文本 {t:?} 宽 {max_w} 行 {max_lines} 的折行结果应与逐字算法一致"
                            );
                        }
                    }
                }
            });
        });
    }

    #[test]
    fn pixel_snapping_lands_on_device_pixels() {
        for ppp in [1.0f32, 1.25, 1.5, 2.0] {
            let ctx = egui::Context::default();
            ctx.set_zoom_factor(ppp);
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))),
                ..Default::default()
            };
            let _ = ctx.run_ui(raw, |ui| {
                let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
                let on_grid = |v: f32| {
                    let f = (v * c.ppp).fract().abs();
                    f < 1e-3 || 1.0 - f < 1e-3
                };
                for (x, y, w, h) in [
                    (0.3, 0.7, 10.1, 6.2),
                    (12.34, 5.67, 33.3, 4.4),
                    (-2.2, 8.8, 7.7, 7.7),
                ] {
                    let r = c.rect(x, y, w, h);
                    for e in [r.min.x, r.max.x, r.min.y, r.max.y] {
                        assert!(on_grid(e), "ppp={ppp} 矩形边 {e} 应落在设备像素边界");
                    }
                    assert!(r.width() >= 1.0 / c.ppp - 1e-6 && r.height() >= 1.0 / c.ppp - 1e-6);
                }
                // 入场动画里薄到不足 1px 的填充不能被对齐吞成 0 宽。
                let thin = c.rect(5.0, 5.0, 0.01, 0.01);
                assert!((thin.width() * c.ppp - 1.0).abs() < 1e-4);
                // 1~2 物理像素的描边路径落在像素中心。
                let sr = c.stroke_rect(1.0, 2.0, 20.0, 9.0, 1.0);
                for e in [sr.min.x, sr.max.x, sr.min.y, sr.max.y] {
                    let v = e * c.ppp - 0.5;
                    assert!(
                        (v - v.round()).abs() < 1e-3,
                        "ppp={ppp} 细描边 {e} 应在像素中心"
                    );
                }
                // 轴对齐细线对齐到像素中心，斜线端点保持原位。
                let [a, b] = c.seg_points((1.3, 1.0), (1.3, 8.0), 1.0);
                assert_eq!(a.x, b.x);
                let v = a.x * c.ppp - 0.5;
                assert!((v - v.round()).abs() < 1e-3);
                let [p, q] = c.seg_points((1.3, 1.0), (9.7, 8.2), 1.0);
                assert_eq!(p, c.pos(1.3, 1.0));
                assert_eq!(q, c.pos(9.7, 8.2));
            });
        }
    }
}

// ======================= 玻璃控件（液态玻璃皮肤） =======================
//
// 小控件（30~40 点的按钮、胶囊、进度槽）不走 GPU 透镜：这个尺寸上的折射位移
// 只有 1~2px，肉眼分辨不出来，白烧一遍纹理采样。这里用「同一套视觉语言」的
// 矢量近似做出玻璃片：极淡的填充 + 顶部内高光（厚度）+ 细白边 + 底部内暗边。
//
// 颜色一律按「深色玻璃上的样子」给（白/黑半透明），浅色皮肤由 skin_paint
// 统一换色（白→墨、黑→纸），所以这里不需要判断深/浅。

/// 一小片玻璃（胶囊、圆角按钮的底）。
///
/// `level`：0 = 静止，1 = 悬停/按下。只影响填充与白边的强度。
/// 玻璃控件的「材质」标记：从这里开始画的图形自带配色，换肤时不跟着换。
///
/// GPU 玻璃的描边在着色器里恒为白线，所以矢量回退版也必须保持白边 ——
/// 否则翻页过渡（控件走不了 GPU 的那半秒）会从黑边跳回白边。
pub(crate) fn chrome_mark(c: &Card) -> usize {
    crate::skin_paint::shape_count(c.ui.ctx(), c.painter.layer_id())
}

/// 把 `from` 起画的图形登记为「玻璃材质」：浅材质下不再被映射成暗雾
/// （white_alpha 的 premultiplied 亮度很低，通用换色会把它当成深色压成墨雾）。
pub(crate) fn chrome_since(c: &Card, from: usize) {
    crate::skin_paint::tone_since(
        c.ui.ctx(),
        c.painter.layer_id(),
        from,
        crate::skin_paint::keep_glass_chrome,
    );
}

pub fn glass_chip(c: &Card, x: f32, y: f32, w: f32, h: f32, r: f32, level: f32) {
    if w < 2.0 || h < 2.0 {
        return;
    }
    let from = chrome_mark(c);
    let lv = level.clamp(0.0, 1.0);
    let r = r.min(w.min(h) * 0.5);
    // 主体：极淡的白。静止时几乎只有一圈边，悬停才「亮起来」。
    c.fill(
        x,
        y,
        w,
        h,
        r,
        Color32::from_white_alpha((10.0 + 30.0 * lv) as u8),
    );
    // 顶部内高光：玻璃厚度的来源。整块高度做渐变，越往下越透明，
    // 这样不会切出一条和圆角对不上的横边。
    c.vgrad(
        x,
        y,
        w,
        h,
        r,
        Color32::from_white_alpha((22.0 + 24.0 * lv) as u8),
        Color32::TRANSPARENT,
    );
    // 底部内暗边：下缘「实」一点，不然整片会飘。
    if h >= 8.0 {
        let inset = r * 0.8;
        c.fill(
            x + inset,
            y + h - 1.0,
            (w - inset * 2.0).max(1.0),
            1.0,
            0.5,
            Color32::from_black_alpha(28),
        );
    }
    // 白边 1.5px：液态玻璃要细、要亮，粗了就成描边贴纸。
    c.stroke(
        x + 0.75,
        y + 0.75,
        w - 1.5,
        h - 1.5,
        (r - 0.75).max(0.0),
        1.5,
        Color32::from_white_alpha((120.0 + 60.0 * lv) as u8),
    );
    chrome_since(c, from);
}

/// 圆形玻璃片：播放控制、齿轮、翻页箭头。
pub fn glass_disc(c: &Card, cx: f32, cy: f32, r: f32, level: f32) {
    if r < 2.0 {
        return;
    }
    let from = chrome_mark(c);
    let lv = level.clamp(0.0, 1.0);
    c.circle(
        cx,
        cy,
        r,
        Color32::from_white_alpha((10.0 + 28.0 * lv) as u8),
    );
    // 上半圈内高光（光源在上方，和 GPU 那边的 line/rim 同一个约定）。
    c.arc(
        cx,
        cy,
        r - 0.8,
        PI * 1.12,
        PI * 1.88,
        1.1,
        Color32::from_white_alpha((26.0 + 30.0 * lv) as u8),
    );
    c.ring(
        cx,
        cy,
        (r - 0.75).max(0.5),
        1.5,
        Color32::from_white_alpha((110.0 + 60.0 * lv) as u8),
    );
    chrome_since(c, from);
}

/// 玻璃凹槽：进度条底槽用（竖向滑槽也能用）。
/// 黑 46% 的底 + 上缘阴影 + 下缘 1px 反光，读出「凹进去」的玻璃感。
/// 曾经用在进度条上失败过（浅色皮肤换色把压暗翻成提亮），液态皮肤改成恒 GLASS_DARK
/// 调色板之后这个问题不存在了，所以现在正式启用。
pub fn glass_groove(c: &Card, x: f32, y: f32, w: f32, h: f32) {
    if w < 2.0 || h < 1.5 {
        return;
    }
    let from = chrome_mark(c);
    let r = h * 0.5;
    c.fill(x, y, w, h, r, Color32::from_black_alpha(46));
    if h >= 6.0 {
        // 上缘阴影（光被槽壁挡掉）
        let inset = (r * 0.9).min(w * 0.4);
        c.fill(
            x + inset,
            y,
            (w - inset * 2.0).max(1.0),
            1.0,
            0.5,
            Color32::from_black_alpha(56),
        );
    }
    // 下缘反光：只有 1px，多一道就脏
    let inset = (r * 0.9).min(w * 0.4);
    c.fill(
        x + inset,
        y + h - 1.0,
        (w - inset * 2.0).max(1.0),
        1.0,
        0.5,
        Color32::from_white_alpha(26),
    );
    chrome_since(c, from);
}

/// 胶囊按钮：悬停提亮、按下轻微缩小，返回 (响应, 内容画布)。
pub fn capsule<'a>(
    c: &Card<'a>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    id: &str,
    base: Color32,
    hover: Color32,
) -> (Response, Card<'a>) {
    let r = c.hit(x, y, w, h, id);
    let (ht, pt) = c.hover_t(&r);
    let k = 1.0 - 0.05 * pt;
    let inner = c.scaled(x + w / 2.0, y + h / 2.0, k);
    inner.fill(x, y, w, h, h / 2.0, mix(base, hover, ht));
    (r, inner)
}

/// 液态玻璃皮肤下的胶囊按钮：hit / 悬停 / 按压缩放与 [`capsule`] 完全一致，
/// 填充换成玻璃层。`level` 是静止亮度（0=只剩一圈边），悬停时按 `hover_boost` 提亮。
/// 选中态由调用方自己表达（更高 level 或文字色）。
pub fn glass_capsule<'a>(
    c: &Card<'a>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    id: &str,
    level: f32,
    hover_boost: f32,
) -> (Response, Card<'a>) {
    let r = c.hit(x, y, w, h, id);
    let (ht, pt) = c.hover_t(&r);
    let k = 1.0 - 0.05 * pt;
    let inner = c.scaled(x + w / 2.0, y + h / 2.0, k);
    glass_chip(
        &inner,
        x,
        y,
        w,
        h,
        h / 2.0,
        (level + ht * hover_boost).clamp(0.0, 1.0),
    );
    (r, inner)
}

// ---------- 子模块 ----------
// 图标与追光 mesh 拆到同级文件；`pub use` 保持 `paint::battery(...)` 等调用路径不变。
mod chase;
mod icons;
pub use chase::chase;
pub use icons::*;

/// 插件状态用的咖啡杯图标（上游 v0.6.70）。
#[cfg(feature = "sdk-js")]
pub fn coffee(c: &Card, x: f32, y: f32, color: egui::Color32) {
    c.stroke(x - 4.5, y - 2.0, 7.6, 6.0, 1.8, 1.1, color);
    c.arc(x + 3.4, y + 1.0, 1.8, -std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2, 1.1, color);
    c.seg((x - 2.0, y - 5.8), (x - 2.0, y - 3.8), 1.0, color);
    c.seg((x + 0.6, y - 6.4), (x + 0.6, y - 3.8), 1.0, color);
}

/// 页面快捷键图标（插件跳转用，定制版用文本占位）。
#[cfg(feature="sdk-js")]
pub fn page_shortcut_icon(_c: &Card, _page: usize, _x: f32, _y: f32, _color: egui::Color32) {}
