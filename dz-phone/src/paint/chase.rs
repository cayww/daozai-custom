//! 进度条追光 mesh（`paint` 子模块）。

use super::{mix, Card};
use eframe::egui::{self, Color32, Pos2};
const CHASE_ROWS: [f32; 23] = [
    -16.0, -11.0, -8.2, -6.2, -4.6, -3.4, -2.4, -1.5, -1.0, -0.65, -0.32, 0.0, 0.32, 0.65, 1.0, 1.5, 2.4, 3.4, 4.6, 6.2,
    8.2, 11.0, 16.0,
];

/// 追光 mesh 的缓存键：只有这些参数变化才需要重新计算整片彩色顶点。
#[derive(Clone, PartialEq)]
struct ChaseKey {
    x0: f32,
    head: f32,
    y: f32,
    thick: f32,
    tint: Color32,
    hot: f32,
    s: f32,
    origin: Pos2,
    ppp: f32,
}

#[derive(Clone)]
struct ChaseCache {
    key: ChaseKey,
    mesh: egui::Mesh,
}

/// 播放头位置的量化步长：播放时每帧只前进零点几 DIP，量化到 0.25 DIP 后
/// 十几帧才变一次，整片 mesh 可以复用；0.25 DIP 的位移在亚像素抗锯齿下不可见。
const HEAD_QUANTUM: f32 = 0.25;

/// 进度条追光：**一片**解析式彗尾，不是「光带 + 亮珠 + 几层光晕」的叠罗汉。
///
/// 每个顶点直接采样五个连续函数并就地做 source-over 合成：
/// 1. `halo`：沿光带铺开的宽高斯外晕，把光"洇"到轨道上下；
/// 2. `under`：已播放段的底色光，横贯整段、很淡；
/// 3. `band`：贴着中轴的细光带，**在头部收窄**，尾巴那头再张开；
/// 4. `core`：播放头附近的中等亮核；
/// 5. `needle`：压在播放头正上方的一颗极小的近白高光——头部之所以"尖"全靠它。
/// 五者共用同一条横向拖尾曲线，所以从头到尾是一次连续过渡，
/// 没有矩形拼接、没有台阶色带、没有可见的圆环接缝。
///
/// 光**朝后**：亮核在 `head`，尾巴一路向左按 `exp(-(d/tail)^1.35)` 衰减，
/// 头前只有约 3 DIP 的收边（两头一阶导都为 0，接得上，不会顿一下）。
///
/// `hot`：暂停 0.45 / 播放 0.62 / 按住拖动 1.0，只影响亮度与尾巴长度。
///
/// 播放头每帧只挪零点几 DIP，所以结果按 `ChaseKey` 缓存：头部量化到
/// [`HEAD_QUANTUM`]、`hot` 量化到 1/256，参数没跨格就整片复用 mesh。
pub fn chase(c: &Card, x0: f32, head: f32, y: f32, thick: f32, tint: Color32, hot: f32) {
    let th = thick.max(1.0);
    let hot = (hot.clamp(0.0, 1.0) * 256.0).round() / 256.0;
    let head = (head / HEAD_QUANTUM).round() * HEAD_QUANTUM;
    let span = head - x0;
    if span < 0.5 {
        return;
    }
    let key = ChaseKey {
        x0,
        head,
        y,
        thick: th,
        tint,
        hot,
        s: c.s,
        origin: c.origin,
        ppp: c.ui.ctx().pixels_per_point(),
    };
    let cache_id = egui::Id::new(("paint::chase", c.ui.id()));
    let cached = c.ui.ctx().data_mut(|d| {
        d.get_temp::<ChaseCache>(cache_id)
            .and_then(|c| if c.key == key { Some(c.mesh) } else { None })
    });
    if let Some(mesh) = cached {
        c.painter.add(egui::Shape::mesh(mesh));
        return;
    }
    // 拖尾特征长度：播放约 46 DIP，拖动时拉长到 56 DIP（越用力尾巴越长）。
    let tail = 30.0 + 26.0 * hot;
    let sy = th * 0.46; // 细光带半高
    let by = th * 1.15 + 1.4 * hot; // 外晕半径（贴着轨道，不要糊成一片雾）
    let cx = th * 1.1; // 亮核横向尺度
    let cy = th * 0.9; // 亮核纵向尺度
    let nx = th * 0.85; // 尖端高光的横向尺度
    let ny = th * 0.32; // 竖向尺度（压扁 → 收成横向的楔形尖头）
    let ramp = 6.0; // 轨道左端收边（够宽才不会在起点切出一条垂直硬边）
    let lead = 6.0f32; // 头部前方收边（越宽过渡越软）

    // 按屏幕像素宽度过滤细光带，移动时亮度不会集中到单行像素。
    let pixel = 1.0 / (c.s * c.ui.ctx().pixels_per_point()).max(0.1);
    let filter_variance = pixel * pixel / 6.0;
    let filtered = |offset: f32, width: f32| {
        let variance = width * width + filter_variance;
        width / variance.sqrt() * (-offset * offset / variance).exp()
    };

    let a_halo = 0.045 + 0.045 * hot;
    let a_under = 0.26 + 0.10 * hot;
    let a_band = 0.16 + 0.16 * hot;
    let a_core = 0.24 + 0.22 * hot;
    let a_needle = 0.50 + 0.30 * hot;

    let c_halo = mix(tint, Color32::WHITE, 0.34);
    let c_under = mix(tint, Color32::WHITE, 0.46);
    let c_band = mix(tint, Color32::WHITE, 0.66);
    let c_core = mix(tint, Color32::WHITE, 0.90);
    let c_needle = mix(tint, Color32::WHITE, 0.97);

    // 列：尾巴 1 DIP 一列（平滑曲线够用），头部 6 DIP 单独加密到 0.6 DIP，
    // 并且**保证有一列正好落在 head 上**——尖点不被插值削平，头才立得住。
    let n_back = (span / 1.0).ceil().max(2.0) as usize;
    let n_fwd = (lead / 0.6).ceil().max(2.0) as usize;
    let mut xs = Vec::with_capacity(n_back + n_fwd + 1);
    for i in 0..=n_back {
        xs.push(x0 + span * i as f32 / n_back as f32);
    }
    for i in 1..=n_fwd {
        xs.push(head + lead * i as f32 / n_fwd as f32);
    }
    let cols = xs.len();

    let mut mesh = egui::Mesh::default();
    mesh.vertices.reserve(cols * CHASE_ROWS.len());
    for off in CHASE_ROWS {
        for &x in &xs {
            let d = head - x; // >0 在头后（左），<0 在头前
            // 横向：向后指数拖尾，向前 6 DIP 内平滑收边
            let back = (-(d.max(0.0) / tail).powf(1.35)).exp();
            // 前端收边随 |off| 提前：越靠上下越早收，前端自然成一个尖角，
            // 而不是一条垂直切平的钝口。
            let d_front = d - off.abs();  // 收角 45°：每往后 1 DIP，半宽张 1 DIP
            let fwd = if d_front >= 0.0 {
                1.0
            } else {
                let q = (-d_front / lead).min(1.0);
                // 两端导数为 0 的平滑收边（smoothstep 的补），斜坡均匀，不会出现台阶
                (1.0 - q).powi(2) * (1.0 + 2.0 * q)
            };
            let s = ((x - x0) / ramp).clamp(0.0, 1.0);
            let start = s * s * (3.0 - 2.0 * s);
            let prof = back * fwd * start;
            // 纵向：所有层都在头部收敛成一点（锥形收尖），往尾巴那头再张开
            let t_along = (d.max(0.0) / tail).min(1.0);
            let taper = 0.48 + 0.52 * t_along;
            let taper_halo = 0.40 + 0.60 * t_along;
            let halo = (-(off / (by * taper_halo)).powi(2)).exp();
            let band = filtered(off, sy * taper / 1.2_f32.sqrt());
            let core = (-((d / (cx * (0.6 + 0.4 * t_along))).powi(2) + (off / (cy * (0.5 + 0.5 * t_along))).powi(2)) * 1.4).exp();
            // 尖端：很扁很亮的一道，自己按高斯在头前几 DIP 内收掉，不被 6 DIP 的
            // 前收边切齐——否则会看见一条垂直的刀口
            let needle = (-(d / nx).powi(2) * 1.2).exp() * filtered(off, ny / 1.2_f32.sqrt());

            let mut acc = [0.0f32; 3];
            let mut inv = 1.0f32;
            let mut put = |col: Color32, sa: f32| {
                let sa = sa.clamp(0.0, 1.0);
                if sa <= 0.002 {
                    return;
                }
                acc[0] += col.r() as f32 / 255.0 * sa * inv;
                acc[1] += col.g() as f32 / 255.0 * sa * inv;
                acc[2] += col.b() as f32 / 255.0 * sa * inv;
                inv *= 1.0 - sa;
            };
            put(c_halo, a_halo * prof * halo);
            // 已播放段的底光不跟着尾巴衰减：整段都要看得见「已播放」，
            // 只是越靠近头部越亮——衰减交给上面那几层彗尾。
            put(c_under, a_under * fwd * start * band);
            put(c_band, a_band * prof * band);
            put(c_core, a_core * core * fwd);
            put(c_needle, a_needle * needle);
            let a = 1.0 - inv;
            let inv_a = if a > 1e-4 { 1.0 / a } else { 0.0 };
            let q = |v: f32| (v * inv_a * 255.0).clamp(0.0, 255.0) as u8;
            let col = Color32::from_rgba_unmultiplied(q(acc[0]), q(acc[1]), q(acc[2]), (a * 255.0) as u8);
            mesh.colored_vertex(c.pos(x, y + off), col);
        }
    }
    let stride = cols as u32;
    for j in 0..CHASE_ROWS.len() as u32 - 1 {
        for i in 0..stride - 1 {
            let a = j * stride + i;
            let b = a + 1;
            let d = a + stride;
            let e = d + 1;
            mesh.add_triangle(a, b, e);
            mesh.add_triangle(a, e, d);
        }
    }
    c.ui.ctx().data_mut(|d| d.insert_temp(cache_id, ChaseCache { key, mesh: mesh.clone() }));
    c.painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Color32, Pos2, Rect, Vec2};

    fn draw(ctx: &egui::Context, head: f32) -> Vec<egui::epaint::Vertex> {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 40.0))),
            ..Default::default()
        };
        let out = ctx.run_ui(raw, |ui| {
            let c = Card::new(ui, Pos2::ZERO, ui.max_rect());
            chase(&c, 10.0, head, 20.0, 4.0, Color32::from_rgb(200, 120, 60), 0.62);
        });
        out.shapes
            .into_iter()
            .find_map(|s| if let egui::Shape::Mesh(m) = s.shape { Some(m.vertices.clone()) } else { None })
            .expect("追光应生成 mesh")
    }

    #[test]
    fn chase_mesh_cache_reuses_subpixel_steps() {
        let ctx = egui::Context::default();
        let a = draw(&ctx, 100.0);
        let b = draw(&ctx, 100.1); // 与 100.0 同一量化格
        assert_eq!(a, b, "0.1 DIP 内应命中缓存，mesh 不变");
        let c = draw(&ctx, 100.6); // 跨过量化格 → 重建
        assert_ne!(a, c, "跨格后 mesh 应随之更新");
        let fresh = draw(&egui::Context::default(), 100.6);
        assert_eq!(c, fresh, "缓存命中的 mesh 必须与冷重建完全一致");
    }
}

