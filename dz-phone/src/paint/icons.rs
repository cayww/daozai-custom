//! 全部矢量图标（`paint` 子模块）。
//!
//! 图标只依赖 [`super::Card`] 的 `pos/fill/stroke/poly/...` 原语。
//! 从 `paint.rs` 拆出来，让画布/文字与图标库各自独立演进。

use super::{alpha, glass_disc, mix, Card, H};
use crate::theme::{self, W};
use eframe::egui::{self, Color32, Stroke};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
// ======================= 图标 =======================

pub fn close(c:&Card,cx:f32,cy:f32,r:f32,color:Color32) {
    c.seg((cx-r,cy-r),(cx+r,cy+r),1.2,color);
    c.seg((cx-r,cy+r),(cx+r,cy-r),1.2,color);
}

/// 翻页箭头。`scale` 由控件弹簧给（入场张开 + 按压回弹），`gpu` 表示这块玻璃
/// 已经由 GPU 着色器画了 —— 那样这里只画箭头本体，不再叠一片矢量玻璃。
pub fn chevron(c: &Card, x: f32, y: f32, dir: f32, hover: f32, scale: f32, glass: bool, gpu: bool) {
    let c = c.scaled(x, y, scale);
    if glass {
        // 液态玻璃：整颗换成玻璃片，箭头保持白色（浅色皮肤由换色改成墨色）。
        if !gpu { glass_disc(&c, x, y, 12.0, 0.18 + hover * 0.82); }
    } else {
        c.circle(x, y, 9.0, mix(theme::ARROW_BG, Color32::from_rgb(78, 78, 82), hover));
    }
    let nudge = dir * 0.8 * hover;
    let (cx, s) = (x + nudge + dir * 0.6, 2.7);
    c.path(&[(cx - dir * s * 0.75, y - s), (cx + dir * s * 0.45, y), (cx - dir * s * 0.75, y + s)], 1.6, Color32::WHITE);
}

/// 电池环：充电绿色 + 闪电；低电量红色；否则灰色并显示百分比弧。
pub fn battery(c: &Card, cx: f32, cy: f32, pct: Option<u8>, charging: bool, k: f32) {
    let r = 8.2 * k;
    let w = 2.6 * k;
    let level = pct.unwrap_or(100) as f32 / 100.0;
    let col = if charging {
        theme::CHARGE
    } else if level < 0.2 {
        theme::RED
    } else {
        Color32::from_rgb(220, 220, 224)
    };
    c.ring(cx, cy, r, w, alpha(col, 0.22));
    let a0 = -FRAC_PI_2;
    if level >= 0.999 {
        c.ring(cx, cy, r, w, col);
    } else {
        c.arc(cx, cy, r, a0, a0 + TAU * level.max(0.02), w, col);
    }
    if charging || pct.is_none() {
        let s = k;
        c.poly(&[(cx + 1.6 * s, cy - 5.0 * s), (cx - 3.2 * s, cy + 0.9 * s), (cx + 0.3 * s, cy + 0.9 * s)], col);
        c.poly(&[(cx - 0.3 * s, cy - 0.9 * s), (cx + 3.2 * s, cy - 0.9 * s), (cx - 1.6 * s, cy + 5.0 * s)], col);
    } else {
        c.text(cx, cy + 2.4 * k, &format!("{}", pct.unwrap_or(0)), 6.6 * k, W::Bold, col, H::Center);
    }
}

/// 播放 ↔ 暂停形变：`t` = 0 播放三角，1 暂停双竖条。
pub fn play_pause(c: &Card, cx: f32, cy: f32, size: f32, t: f32, color: Color32) {
    let (w, h) = (size * 0.86, size);
    let bw = w * 0.34;
    let l = |a: (f32, f32), b: (f32, f32)| (cx + a.0 + (b.0 - a.0) * t, cy + a.1 + (b.1 - a.1) * t);
    // 三角拆成左梯形 + 右三角，与两根竖条一一对应插值
    let dx = w * 0.08 * (1.0 - t); // 三角视觉重心右移
    if t==0.0 {
        // 静止播放态画完整三角形，公共边不能被两块图形分别羽化。
        c.poly(&[(cx-w/2.0+dx,cy-h/2.0),(cx+w/2.0+dx,cy),(cx-w/2.0+dx,cy+h/2.0)],color);
        return;
    }
    let left = [
        l((-w / 2.0 + dx, -h / 2.0), (-w / 2.0, -h / 2.0)),
        l((dx, -h / 4.0), (-w / 2.0 + bw, -h / 2.0)),
        l((dx, h / 4.0), (-w / 2.0 + bw, h / 2.0)),
        l((-w / 2.0 + dx, h / 2.0), (-w / 2.0, h / 2.0)),
    ];
    let right = [
        l((dx, -h / 4.0), (w / 2.0 - bw, -h / 2.0)),
        l((w / 2.0 + dx, -0.01), (w / 2.0, -h / 2.0)),
        l((w / 2.0 + dx, 0.01), (w / 2.0, h / 2.0)),
        l((dx, h / 4.0), (w / 2.0 - bw, h / 2.0)),
    ];
    c.poly(&left, color);
    c.poly(&right, color);
}

/// 上一首/下一首的双三角。`size` = 图标总高（旧版硬编码 12.0）；玻璃圆盘外圈
/// 已有描边，内部图标整体调小一档避免拥挤，总宽按 size 等比（size=12 时总宽 18.2）。
pub fn skip(c: &Card, cx: f32, cy: f32, dir: f32, size: f32, color: Color32) {
    let k = size / 12.0;
    let half_gap = 4.3 * k;   // 两个三角的中心间距一半
    let half_w = 4.6 * k;     // 单个三角半宽
    let half_h = 6.0 * k;     // 单个三角半高
    for s in [-1.0f32, 1.0] {
        let x = cx + s * half_gap;
        c.poly(&[(x - dir * half_w, cy - half_h), (x + dir * half_w, cy), (x - dir * half_w, cy + half_h)], color);
    }
}

pub fn playback_bars(c:&Card,cx:f32,cy:f32,time:f64,playing:bool,color:Color32) {
    for i in 0..5 {
        let phase=if playing {time as f32*(5.0+i as f32*0.7)}else{0.0}+i as f32*1.6;
        let height=if playing {3.5+(phase.sin()*0.5+0.5)*8.0}else{3.5};
        c.fill(cx-7.8+i as f32*3.9,cy-height/2.0,1.9,height,0.95,color);
    }
}

/// `scale` = 相对旧版（喇叭体宽约 20）的缩放；玻璃圆盘外圈已有描边，内部图标
/// 缩小一档避免拥挤。
pub fn speaker(c:&Card,cx:f32,cy:f32,muted:bool,scale:f32,color:Color32) {
    let s=scale;
    c.fill(cx-7.0*s,cy-2.8*s,4.0*s,5.6*s,0.8*s,color);
    c.poly(&[(cx-4.0*s,cy-2.8*s),(cx+0.5*s,cy-6.0*s),(cx+0.5*s,cy+6.0*s),(cx-4.0*s,cy+2.8*s)],color);
    if muted {
        c.seg((cx+4.0*s,cy-2.7*s),(cx+9.0*s,cy+2.7*s),1.4*s,color);
        c.seg((cx+4.0*s,cy+2.7*s),(cx+9.0*s,cy-2.7*s),1.4*s,color);
    } else {
        c.arc(cx-0.8*s,cy,5.4*s,-0.7,0.7,1.4*s,color);
        c.arc(cx-0.8*s,cy,8.5*s,-0.7,0.7,1.4*s,alpha(color,0.6));
    }
}

pub fn note(c: &Card, cx: f32, cy: f32, size: f32, color: Color32) {
    let s = size;
    c.circle(cx - s * 0.24, cy + s * 0.28, s * 0.14, color);
    c.circle(cx + s * 0.26, cy + s * 0.18, s * 0.14, color);
    c.seg((cx - s * 0.11, cy + s * 0.28), (cx - s * 0.11, cy - s * 0.30), 1.6, color);
    c.seg((cx + s * 0.39, cy + s * 0.18), (cx + s * 0.39, cy - s * 0.42), 1.6, color);
    c.poly(&[(cx - s * 0.11 - 0.8, cy - s * 0.30), (cx + s * 0.39 + 0.8, cy - s * 0.42), (cx + s * 0.39 + 0.8, cy - s * 0.30), (cx - s * 0.11 - 0.8, cy - s * 0.18)], color);
}

pub fn stopwatch(c: &Card, cx: f32, cy: f32, r: f32, hand: f32, color: Color32) {
    c.ring(cx, cy + 2.0, r, 3.0, color);
    c.fill(cx - 6.8, cy - r - 6.0, 13.6, 3.6, 1.6, color);
    c.fill(cx - 1.8, cy - r - 3.0, 3.6, 4.0, 1.0, color);
    let a = -FRAC_PI_2 + hand;
    let k = (r + 2.5, r + 6.0);
    let b = (-FRAC_PI_2 + 0.78f32).sin_cos();
    c.seg((cx + b.1 * k.0, cy + 2.0 + b.0 * k.0), (cx + b.1 * k.1, cy + 2.0 + b.0 * k.1), 3.4, color);
    c.seg((cx, cy + 2.0), (cx + a.cos() * r * 0.62, cy + 2.0 + a.sin() * r * 0.62), 3.0, color);
    c.circle(cx, cy + 2.0, 2.0, color);
}

/// 重置图标：开口圆弧 + 箭头，`spin` 为整体旋转（弧度）。
pub fn reset(c: &Card, cx: f32, cy: f32, spin: f32, color: Color32) {
    let r = 5.6;
    let a0 = -1.1 + spin;
    let a1 = a0 + 5.0;
    c.arc(cx, cy, r, a0, a1, 1.8, color);
    let (s, co) = a0.sin_cos();
    let tip = (cx + co * r, cy + s * r);
    // 箭头沿切线方向（逆时针）
    let t = (s, -co);
    let n = (co, s);
    c.poly(
        &[
            (tip.0 + t.0 * 3.4, tip.1 + t.1 * 3.4),
            (tip.0 + n.0 * 2.8 - t.0 * 0.6, tip.1 + n.1 * 2.8 - t.1 * 0.6),
            (tip.0 - n.0 * 2.8 - t.0 * 0.6, tip.1 - n.1 * 2.8 - t.1 * 0.6),
        ],
        color,
    );
}

pub fn target(c: &Card, x: f32, y: f32, color: Color32) {
    c.ring(x, y, 5.0, 1.1, color);
    c.ring(x, y, 2.4, 1.1, color);
    c.seg((x, y), (x + 5.2, y - 5.2), 1.2, color);
    c.poly(&[(x + 5.6, y - 7.4), (x + 7.4, y - 5.6), (x + 4.4, y - 4.4)], color);
}

/// 办公椅（久坐提醒的图标）：侧面的实心转椅——微微后仰的靠背、扶手、坐垫、气压杆、带滚轮的底座。
/// 和水滴一样是物体图标，宽约 24k、高约 30k，(x,y) 为中心。
pub fn chair(c: &Card, x: f32, y: f32, k: f32, color: Color32) {
    let p = |dx: f32, dy: f32| (x + dx * k, y + dy * k);
    let bar = |a: (f32, f32), b: (f32, f32), w: f32| {
        c.seg(p(a.0, a.1), p(b.0, b.1), w * k, color);
        c.circle(p(a.0, a.1).0, p(a.0, a.1).1, w * k * 0.5, color);
        c.circle(p(b.0, b.1).0, p(b.0, b.1).1, w * k * 0.5, color);
    };
    bar((-9.0, -13.0), (-7.0, 0.5), 5.0);        // 靠背
    bar((-7.0, 2.0), (8.5, 2.0), 4.6);           // 坐垫
    bar((-4.0, -4.5), (5.0, -4.5), 2.2);         // 扶手
    bar((3.5, -4.5), (3.5, 0.0), 2.2);           // 扶手支撑
    bar((0.5, 4.0), (0.5, 10.0), 2.6);           // 气压杆
    bar((-9.0, 11.0), (10.0, 11.0), 2.4);        // 底座
    for dx in [-9.0f32, 10.0] {                  // 滚轮
        c.circle(x + dx * k, y + 14.0 * k, 2.0 * k, color);
    }
}

pub fn droplet(c: &Card, cx: f32, cy: f32, size: f32, color: Color32) {
    // 水滴 = 尖顶 + 与两条切线相接的圆底
    let r = size * 0.34;
    let center = (cx, cy + size * 0.5 - r);
    let tip = (cx, cy - size * 0.5);
    let d = center.1 - tip.1;
    let phi = (r / d).clamp(-1.0, 1.0).acos();
    let mut pts = vec![tip];
    let n = 20;
    for i in 0..=n {
        let b = phi + (TAU - 2.0 * phi) * i as f32 / n as f32;
        pts.push((center.0 + r * b.sin(), center.1 - r * b.cos()));
    }
    c.poly(&pts, color);
}

pub(crate) fn bell_swing(elapsed:f64)->f32 {
    if !(0.0..0.95).contains(&elapsed) {return 0.0;}
    let t=elapsed as f32/0.95;
    (t*TAU*2.0).sin()*0.48*(1.0-t).sqrt()
}

pub fn bell(c: &Card, x: f32, y: f32, color: Color32, angle:f32) {
    let mut pts = Vec::new();
    for i in 0..=12 {
        let a = PI + PI * i as f32 / 12.0;
        pts.push((x + 3.4 * a.cos(), y - 0.6 + 3.4 * a.sin()));
    }
    pts.push((x + 3.6, y + 2.4));
    pts.push((x + 4.6, y + 3.2));
    pts.push((x - 4.6, y + 3.2));
    pts.push((x - 3.6, y + 2.4));
    let rotate=|(px,py):(f32,f32)| {
        let (sin,cos)=angle.sin_cos();let dx=px-x;let dy=py-(y-5.0);
        (x+dx*cos-dy*sin,y-5.0+dx*sin+dy*cos)
    };
    c.poly(&pts.into_iter().map(rotate).collect::<Vec<_>>(), color);
    let clapper=rotate((x,y+4.6));
    c.circle(clapper.0,clapper.1,1.2,color);
}

pub fn gear(c: &Card, x: f32, y: f32, color: Color32) {
    let points=(0..64).map(|i|{
        let angle=i as f32*TAU/64.0;
        let radius=if i%8<2 || i%8>=6 {7.0}else{5.6};
        c.pos(x+radius*angle.cos(),y+radius*angle.sin())
    }).collect();
    c.painter.add(egui::Shape::closed_line(points,Stroke::new(1.2*c.s,color)));
    c.painter.circle_stroke(c.pos(x,y),2.3*c.s,Stroke::new(1.2*c.s,color));
}

pub fn sliders(c: &Card, x: f32, y: f32, color: Color32) {
    for (dy, dx) in [(-2.4f32, -2.0f32), (2.4, 2.2)] {
        c.seg((x - 5.0, y + dy), (x + 5.0, y + dy), 1.1, color);
        c.circle(x + dx, y + dy, 1.9, color);
    }
}

pub fn plus(c: &Card, cx: f32, cy: f32, r: f32, color: Color32, ink: Color32) {
    c.circle(cx, cy, r, color);
    c.seg((cx - r * 0.5, cy), (cx + r * 0.5, cy), 1.3, ink);
    c.seg((cx, cy - r * 0.5), (cx, cy + r * 0.5), 1.3, ink);
}

pub fn pencil(c: &Card, cx: f32, cy: f32, size: f32, color: Color32) {
    let s = size;
    c.poly(&[(cx - s * 0.30, cy + s * 0.14), (cx + s * 0.22, cy - s * 0.38), (cx + s * 0.40, cy - s * 0.20), (cx - s * 0.12, cy + s * 0.32)], color);
    c.poly(&[(cx - s * 0.40, cy + s * 0.42), (cx - s * 0.30, cy + s * 0.14), (cx - s * 0.12, cy + s * 0.32)], color);
}

/// 勾：`t` 0..1 逐笔画出。
pub fn check(c: &Card, cx: f32, cy: f32, s: f32, t: f32, width: f32, color: Color32) {
    let a = (cx - s * 0.45, cy + s * 0.02);
    let b = (cx - s * 0.12, cy + s * 0.34);
    let d = (cx + s * 0.48, cy - s * 0.32);
    let t1 = (t / 0.4).clamp(0.0, 1.0);
    let t2 = ((t - 0.4) / 0.6).clamp(0.0, 1.0);
    if t1 <= 0.0 {
        return;
    }
    let p1 = (a.0 + (b.0 - a.0) * t1, a.1 + (b.1 - a.1) * t1);
    if t2 <= 0.0 {
        c.path(&[a, p1], width, color);
    } else {
        let p2 = (b.0 + (d.0 - b.0) * t2, b.1 + (d.1 - b.1) * t2);
        c.path(&[a, b, p2], width, color);
    }
}

pub fn checklist(c: &Card, cx: f32, cy: f32, size: f32, ink: Color32, plate: Color32) {
    c.fill(cx - size / 2.0, cy - size / 2.0, size, size, size * 0.26, plate);
    for dy in [-0.24f32, 0.0, 0.24] {
        let y = cy + dy * size;
        check(c, cx - size * 0.17, y, size * 0.2, 1.0, 1.6, ink);
        c.seg((cx + size * 0.04, y), (cx + size * 0.28, y), 1.6, ink);
    }
}

/// 下载托盘：向下箭头落进收纳盒。`drop` 为箭头下坠位移（拖入时轻微弹跳）。
pub fn tray(c: &Card, cx: f32, cy: f32, size: f32, drop: f32, color: Color32, shade: Color32) {
    let u = size / 30.0;
    let ay = cy - 13.0 * u + drop * u;
    c.fill(cx - 1.8 * u, ay, 3.6 * u, 8.0 * u, 1.0 * u, color);
    c.poly(&[(cx - 6.0 * u, ay + 6.5 * u), (cx + 6.0 * u, ay + 6.5 * u), (cx, ay + 12.5 * u)], color);
    c.poly(
        &[
            (cx - 10.0 * u, cy + 1.0 * u),
            (cx + 10.0 * u, cy + 1.0 * u),
            (cx + 13.5 * u, cy + 8.5 * u),
            (cx - 13.5 * u, cy + 8.5 * u),
        ],
        shade,
    );
    c.fill(cx - 14.0 * u, cy + 8.0 * u, 28.0 * u, 8.5 * u, 1.8 * u, color);
    let mut notch = Vec::new();
    for i in 0..=16 {
        let a = PI * i as f32 / 16.0;
        notch.push((cx + 4.6 * u * a.cos(), cy + 8.0 * u + 3.6 * u * a.sin()));
    }
    c.poly(&notch, shade);
}

// ---------- 文件图标（取不到系统图标时的矢量版） ----------
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
    Folder,
    Image,
    Pdf,
    Doc,
    Sheet,
    Archive,
    Audio,
    Other,
}

pub fn file_kind(p: &std::path::Path) -> FileKind {
    if p.is_dir() {
        return FileKind::Folder;
    }
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "heic" | "svg" => FileKind::Image,
        "pdf" => FileKind::Pdf,
        "doc" | "docx" | "txt" | "md" | "rtf" => FileKind::Doc,
        "xls" | "xlsx" | "csv" => FileKind::Sheet,
        "zip" | "7z" | "rar" | "tar" | "gz" => FileKind::Archive,
        "mp3" | "flac" | "wav" | "m4a" | "ogg" => FileKind::Audio,
        _ => FileKind::Other,
    }
}

pub fn file_glyph(c: &Card, kind: FileKind, cx: f32, cy: f32, size: f32) {
    let s = size / 36.0;
    if kind == FileKind::Folder {
        let back = Color32::from_rgb(230, 167, 38);
        let front = Color32::from_rgb(255, 202, 74);
        c.fill(cx - 17.0 * s, cy - 12.0 * s, 13.0 * s, 6.0 * s, 2.0 * s, back);
        c.fill(cx - 17.0 * s, cy - 9.5 * s, 34.0 * s, 22.0 * s, 3.0 * s, back);
        c.fill(cx - 17.0 * s, cy - 5.5 * s, 34.0 * s, 19.5 * s, 3.0 * s, front);
        c.fill(cx - 17.0 * s, cy - 5.5 * s, 34.0 * s, 1.2 * s, 0.6 * s, Color32::from_rgba_unmultiplied(255, 240, 200, 120));
        return;
    }
    // 纸张 + 折角
    let (w, h) = (26.0 * s, 33.0 * s);
    let (x0, y0) = (cx - w / 2.0, cy - h / 2.0);
    let fold = 7.5 * s;
    let paper = Color32::from_rgb(246, 246, 248);
    c.poly(&[(x0, y0 + 2.0 * s), (x0 + w - fold, y0), (x0 + w, y0 + fold), (x0 + w, y0 + h), (x0, y0 + h)], paper);
    c.fill(x0, y0, w - fold, h, 2.4 * s, paper);
    c.fill(x0, y0 + fold, w, h - fold, 2.4 * s, paper);
    c.poly(&[(x0 + w - fold, y0), (x0 + w, y0 + fold), (x0 + w - fold, y0 + fold)], Color32::from_rgb(205, 205, 212));
    let accent = match kind {
        FileKind::Image => Color32::from_rgb(64, 156, 255),
        FileKind::Pdf => Color32::from_rgb(235, 64, 52),
        FileKind::Doc => Color32::from_rgb(44, 110, 230),
        FileKind::Sheet => Color32::from_rgb(33, 160, 90),
        FileKind::Archive => Color32::from_rgb(140, 110, 230),
        FileKind::Audio => Color32::from_rgb(255, 69, 120),
        _ => Color32::from_rgb(150, 150, 160),
    };
    match kind {
        FileKind::Image => {
            c.fill(x0 + 4.0 * s, y0 + 11.0 * s, w - 8.0 * s, 15.0 * s, 2.0 * s, Color32::from_rgb(214, 234, 255));
            c.poly(&[(x0 + 5.0 * s, y0 + 25.0 * s), (x0 + 11.0 * s, y0 + 16.5 * s), (x0 + 16.0 * s, y0 + 25.0 * s)], accent);
            c.poly(&[(x0 + 11.0 * s, y0 + 25.0 * s), (x0 + 16.5 * s, y0 + 19.0 * s), (x0 + 21.0 * s, y0 + 25.0 * s)], Color32::from_rgb(40, 120, 220));
            c.circle(x0 + 18.0 * s, y0 + 14.5 * s, 1.8 * s, YELLOW_SOFT);
        }
        FileKind::Pdf | FileKind::Doc | FileKind::Sheet => {
            c.fill(x0 + 3.0 * s, y0 + h - 12.5 * s, 15.0 * s, 8.0 * s, 1.6 * s, accent);
            let tag = match kind {
                FileKind::Pdf => "PDF",
                FileKind::Doc => "DOC",
                _ => "XLS",
            };
            c.text(x0 + 10.5 * s, y0 + h - 6.3 * s, tag, 5.4 * s, W::Bold, Color32::WHITE, H::Center);
            for i in 0..3 {
                let y = y0 + 9.0 * s + i as f32 * 3.6 * s;
                c.fill(x0 + 4.0 * s, y, if i == 2 { 10.0 * s } else { 16.0 * s }, 1.4 * s, 0.7 * s, Color32::from_rgb(200, 200, 208));
            }
        }
        FileKind::Archive => {
            for i in 0..5 {
                let y = y0 + 3.0 * s + i as f32 * 3.2 * s;
                c.fill(cx - 2.2 * s + if i % 2 == 0 { 0.0 } else { 2.2 * s }, y, 2.2 * s, 2.0 * s, 0.4 * s, accent);
            }
            c.fill(cx - 3.0 * s, y0 + 19.0 * s, 6.0 * s, 7.0 * s, 1.6 * s, accent);
        }
        FileKind::Audio => note(c, cx, cy + 2.0 * s, 16.0 * s, accent),
        _ => {
            for i in 0..4 {
                let y = y0 + 10.0 * s + i as f32 * 4.0 * s;
                c.fill(x0 + 4.0 * s, y, 18.0 * s, 1.4 * s, 0.7 * s, Color32::from_rgb(200, 200, 208));
            }
        }
    }
}

const YELLOW_SOFT: Color32 = Color32::from_rgb(255, 214, 92);


// 时段文字弱化，数字保持原有字号与字重。
pub fn clock_width(c:&Card,text:&str,size:f32)->f32 {
    match text.split_once(' ') {
        Some((period,digits))=>c.text_w(period,size*0.72,W::Semibold)+4.0+c.text_w(digits,size,W::Bold),
        None=>c.text_w(text,size,W::Bold),
    }
}
pub fn clock(c:&Card,x:f32,y:f32,text:&str,size:f32) {
    let (digits,x)=if let Some((period,digits))=text.split_once(' ') {
        let numbers=c.galley(digits,size,W::Bold,theme::WHITE);
        let ascent=numbers.rows.first().and_then(|r|r.glyphs.first()).map(|g|g.pos.y).unwrap_or(size);
        // galley 固定字号排版，mesh_bounds 已是局部单位，无需再 /c.s
        let center=y+(numbers.mesh_bounds.center().y-ascent);
        c.centered_text(x,center,period,size*0.72,W::Semibold,theme::WHITE,H::Left);
        (digits,x+c.text_w(period,size*0.72,W::Semibold)+4.0)
    }else{(text,x)};
    c.text(x,y,digits,size,W::Bold,theme::WHITE,H::Left);
}

/// 图钉：固定的剪贴板条目。
pub fn pin(c: &Card, cx: f32, cy: f32, size: f32, color: Color32) {
    let s = size / 10.0;
    c.fill(cx - 2.4 * s, cy - 5.0 * s, 4.8 * s, 4.2 * s, 1.0 * s, color);
    c.poly(&[(cx - 4.2 * s, cy - 1.2 * s), (cx + 4.2 * s, cy - 1.2 * s), (cx + 2.6 * s, cy + 1.0 * s), (cx - 2.6 * s, cy + 1.0 * s)], color);
    c.seg((cx, cy + 1.0 * s), (cx, cy + 5.2 * s), 1.1, color);
}

/// 剪贴板：空状态插图。
pub fn clipboard_glyph(c: &Card, cx: f32, cy: f32, size: f32, color: Color32, shade: Color32) {
    let u = size / 22.0;
    c.fill(cx - 9.0 * u, cy - 9.0 * u, 18.0 * u, 22.0 * u, 3.0 * u, shade);
    c.stroke(cx - 9.0 * u, cy - 9.0 * u, 18.0 * u, 22.0 * u, 3.0 * u, 1.3, color);
    c.fill(cx - 4.5 * u, cy - 11.5 * u, 9.0 * u, 5.0 * u, 1.6 * u, color);
    for (i, w) in [11.0, 8.0, 10.0].into_iter().enumerate() {
        c.fill(cx - 5.5 * u, cy - 1.5 * u + i as f32 * 4.5 * u, w * u, 1.4 * u, 0.7 * u, color);
    }
}

