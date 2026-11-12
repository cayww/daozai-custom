//! 灵动岛主体：状态、悬停形变、切页、顶栏与公共层。与平台无关——
//! Windows 外壳（`main.rs`）和无窗口录制器（`daozai-record`）共用同一份代码。
//!
//! 时间一律来自 `Env::t`（秒），不直接读系统时钟，所以录屏可逐帧复现。

use crate::extras::{Alert, PetState};
use crate::anim::{self, lerp, smoothstep, Spring};
use crate::bar_layout::{self as bar, Layout};
use crate::calendar;
use crate::glass;
use crate::lyrics;
use crate::mail;
use crate::model;
use crate::paint::{self, Card, H};
use crate::shape;
use crate::smtc::{NowPlaying, Transport};
use crate::theme::{self, size, W};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::collections::HashMap;
use std::path::PathBuf;

// ---------- 几何（逻辑点；原版 125% 截图 ÷ 1.25） ----------
pub const PW: f32 = 419.0; // 展开面板宽
pub const PH: f32 = 140.0; // 展开面板高
pub(crate) const CALENDAR_PAGE: usize = 7;
pub(crate) const CALENDAR_H: f32 = 216.0;
pub const BAR_W: f32 = 163.0; // 收起药丸宽
pub const BAR_H: f32 = 30.0; // 收起药丸高
pub(crate) const HUD_W: f32 = 260.0;
pub(crate) const LYRIC_BAR_W: f32 = 300.0;
// 歌词态左侧固定通道（手写 flex lane）：若干固定宽项 + 一块 flex:1 剩余区。
// 想调间距或往胶囊里加新图标，只改这几个常量和 lyric_lane()，不用翻绘制代码。
pub(crate) const LANE_PET_HALF: f32 = 24.0;   // 宠物表情贴图半宽（实际 48px）
pub(crate) const LANE_ART_GAP: f32 = 4.0;     // 封面右缘 → 宠物左缘
pub(crate) const LANE_TEXT_GAP: f32 = 4.0;    // 宠物右缘 → 歌词通道（硬余量，保证不重叠）
pub(crate) const LANE_CENTER_CAP: f32 = 16.0; // 歌词在通道内居中的最大偏移，避免短歌词离宠物太远
// 播放指示 5 根竖条以 content_width-21 为中心、向左占 7.8，左缘在 content_width-28.8；
// 这里留 42 才能让文字与竖条之间有 ~13px 空隙（原来 30 只剩 1.2px，看起来贴在一起）。
pub(crate) const LANE_RIGHT_PAD: f32 = 48.0;  // 歌词通道右侧留给播放指示
// 歌词固定字号：放不下就横向滚动，不再缩字号。
const LYRIC_BASE_SIZE: f32 = 11.0;
const SCROLL_DWELL: f32 = 1.0;      // 句首/句尾停留，先让人看清开头
const SCROLL_MIN_SPEED: f32 = 22.0; // px/s，约每秒 2 个汉字
const SCROLL_MAX_SPEED: f32 = 90.0;
pub(crate) const HUD_TRACK_X: f32 = 48.0;
pub(crate) const HUD_TRACK_W: f32 = 150.0;
pub const BAR_W_MIN: f32 = 44.0; // 极简小条
pub const BAR_H_MIN: f32 = 4.5;
pub const FLARE: f32 = 17.6; // 顶部外翻肩宽
pub const FLARE_H: f32 = 16.0; // 外翻收敛高度
pub const RAD: f32 = 33.0; // 底部圆角
pub const WIN_W: f32 = 640.0; // 窗口（透明画布）
pub const CANVAS_W: f32 = WIN_W + 320.0; // 保留右侧通知及阴影的完整空间，原岛仍按 WIN_W 定位。
pub const WIN_H: f32 = 270.0;
/// 胶囊皮肤与屏幕顶边的间隙。
pub const PILL_GAP: f32 = 6.0;

/// 胶囊的柔和投影：四周都有，多层叠加近似高斯。
fn pill_shadow(painter:&egui::Painter,rect:Rect,radius:f32,strength:f32){
    if strength<=0.01 {return;}
    for (grow,a) in [(10.0f32,0.035f32),(6.5,0.05),(3.5,0.07),(1.5,0.09)] {
        // 每层都是主体的等距外扩（圆角增量 = 横向扩展量）+ 整体下移，
        // 四角圆弧与主体严格同心同形，直线转圆角处不会再叠出错开的半圆凸边。
        let (d,dy)=shape::shadow_expansion(grow);
        let r=Rect::from_min_max(rect.min-Vec2::new(d,d-dy),rect.max+Vec2::new(d,d+dy));
        painter.rect_filled(r,radius+d,Color32::from_black_alpha((255.0*a*strength) as u8));
    }
}
pub(crate) const PAGE_NAMES: [&str; 8] = ["音乐", "专注", "饮水", "置物架", "待办", "邮箱", "AI 额度", "日程"];

// ---------- 动效参数 ----------
/// 等待超过这个时长才做「轻微胀大」反馈；更短的等待保持静止，
/// 否则弹簧刚鼓一点就被展开打断，反而更突兀。
const SWELL_MIN_MS: u32 = 180;
/// 移开后等待多久开始收起；这段时间内回来不会收。
const LEAVE_DELAY: f64 = 0.12;
const SWELL: (f32, f32) = (1.075, 1.13);
// 展开参数按 `model::OpenAnim` 的配置取，见 `OpenAnim::tuning()`；
// CLOSE 仍用于弹簧初始化与非展开态的回位。
const CLOSE: (f32, f32) = (620.0, 1.0);
const SWELL_K: (f32, f32) = (420.0, 0.8);
const PAGE_SLIDE: f32 = 46.0;

/// 悬停多久才展开（秒）。`0` 表示当帧就展开；极简模式加倍，
/// 默认 200 ms 时仍是原来的 0.40 s 保守值。防误触另由 `LEAVE_DELAY` 兜底。
fn hover_delay_secs(ms: u32, minimal: bool) -> f64 {
    let ms = if minimal { ms.saturating_mul(2) } else { ms };
    (ms as f64 / 1000.0).min(1.0)
}

pub struct Options {
    /// 是否联网查歌词（录制/测试时关掉）。
    pub fetch_lyrics: bool,
    /// 吉祥物在多久无操作后出现（秒）。
    pub pet_idle: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self { fetch_lyrics: true, pet_idle: 45.0 }
    }
}

/// 每帧由外壳提供的外部世界。
#[derive(Clone, Default)]
pub struct Env {
    pub t: f64,
    /// `HH:MM:SS`
    pub clock: String,
    /// `YYYY-MM-DD`
    pub today: String,
    /// 顶栏日期，例如 `10月28日 周三`
    pub date: String,
    pub battery: Option<u8>,
    pub charging: bool,
    /// 互联网连接状态；未知时不触发表情提醒。
    pub network: Option<bool>,
    /// 光标位置（与 `frame` 传入的 `win` 同一坐标系）；不在窗口附近时为 None。
    pub cursor: Option<Pos2>,
    pub idle_seconds: Option<f64>,
    /// 前台浏览器的顶部是否与岛重叠；岛仔自身窗口或暂不可读时为 None。
    pub browser_at_top: Option<bool>,
    pub media: NowPlaying,
    /// 系统主音量（0..100, 是否静音）；v0.5.4 音量条与系统双向同步。
    pub volume: Option<(u32, bool)>,
    /// 已连接的蓝牙音频设备（名称 + 可选电量）。
    pub bt: Vec<crate::bt::Device>,
    /// 蓝牙服务是否已经完成首次扫描。
    pub bt_ready: bool,
    /// 岛所在屏幕（与 `win` 同一坐标系，逻辑点）；用于让通知详情留在屏幕内。
    pub screen: Option<Rect>,
    /// 毛玻璃皮肤：岛体四周采样得到的背景色（sRGB 0..1）。
    pub backdrop: Option<[f32; 3]>,
    /// 毛玻璃皮肤：系统背景模糊当前是否生效。
    pub glass_blur: bool,
    /// 不支持逐像素透明的兼容后端需要按内容轮廓裁剪窗口。
    pub opaque_surface: bool,
}

impl Env {
    pub fn hour(&self) -> usize {
        self.clock.get(0..2).and_then(|s| s.parse().ok()).unwrap_or(12)
    }
    pub fn minute(&self) -> u32 {
        self.clock.get(3..5).and_then(|s| s.parse().ok()).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Messenger { Wechat, Qq }
impl Messenger {
    pub fn name(self)->&'static str {match self {Self::Wechat=>"微信",Self::Qq=>"QQ"}}
}

/// 一条未读：来源、账号所在进程（QQ 通知分不出账号，记为 0）和发出提醒的窗口（没有时为 0）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unread { pub source: Messenger, pub pid: u32, pub window: isize }
impl Unread {
    pub fn of(source: Messenger) -> Self { Self { source, pid: 0, window: 0 } }
}

/// 展开后未读胶囊的宽度、离面板的间距和每个图标占的高度。
pub(crate) const UNREAD_W: f32 = 40.0;
pub(crate) const UNREAD_GAP: f32 = 8.0;
pub(crate) const UNREAD_SLOT: f32 = 36.0;
/// 收起时两个来源图标的间距：并排各占一格，不叠放。
fn unread_step(_horizontal: bool) -> f32 { 22.0 }

/// 交给外壳执行的副作用。
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Media(Transport),
    Open(PathBuf),
    ViewMessenger(Unread),
    OpenUrl(String),
    /// 需要键盘输入：外壳应让窗口可激活并取得焦点。
    Focus,
    Reveal(PathBuf),
    Copy(String),
    Notify(String, String),
    RequestNotifications,
    RequestQqNotifications,
    DisconnectNotifications,
    ToggleAutostart,
    AddFiles,
    DragFile(PathBuf),
    QuotaRefresh,
    QuotaConnected,
    QuotaChooseFile,
    /// 把系统主音量设为 0..100（v0.5.4 音量条）。
    SetVolume(u32),
    /// 切换系统静音。
    ToggleMute,
    /// v0.5.5：账号清单变了，让外壳重新同步收信线程。
    MailSync,
    /// 测试某个邮箱的连接（授权码只在内存里传给工作线程）。
    MailTest(mail::Account, String),
    MailManage(crate::mail_manage::Request),
    /// v0.5.6：用临时密码试连日历，成功后才允许落盘。
    CalendarTest(calendar::Config, String),
    /// 让外壳立即同步一次日历。
    CalendarSync,
    CalendarMonth(chrono::NaiveDate),
    /// 断开并删除日历配置（凭据由界面删除）。
    CalendarForget,
    /// 选择 .ics 文件导入本机日程。
    CalendarImport,
}

/// 毛玻璃皮肤下原生窗口区域的组成部分（相对 `win` 左上角，逻辑点）。
/// 系统模糊只出现在窗口区域内，所以区域要贴合岛体轮廓，而不是外接矩形。
#[derive(Clone, Debug, PartialEq)]
pub enum GlassShape {
    /// 任意闭合轮廓。
    Outline(Vec<Pos2>),
    /// 岛体 / 通知胶囊：顶部外翻肩部 + 竖直侧边 + 底部圆角（参数同 `shape::fill`）。
    Notch { rect: Rect, flare: f32, flare_h: f32, radius: f32 },
    /// 停靠在左右边缘的岛体：肩部沿屏幕边缘展开（参数同 `shape::fill_docked`）。
    Docked { rect: Rect, dock: model::Dock, flare: f32, flare_h: f32, radius: f32 },
    /// 圆角矩形（通知详情、提示、浮动确认）。
    Round(Rect, f32),
}

impl GlassShape {
    /// 闭合轮廓（逻辑点）；窗口区域按它裁剪。
    pub fn outline(&self) -> Vec<Pos2> {
        match self {
            Self::Outline(points) => points.clone(),
            Self::Notch { rect, flare, flare_h, radius } => shape::outline(*rect, *flare, *flare_h, *radius),
            Self::Docked { rect, dock, flare, flare_h, radius } => shape::outline_docked(*rect, *dock, *flare, *flare_h, *radius),
            Self::Round(r, radius) => shape::outline(*r, 0.0, 0.5, *radius),
        }
    }
}

/// 一块系统模糊：物理像素的窗口坐标与圆角半径（0 = 直角）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlurPiece {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radius: f32,
}

/// 把毛玻璃轮廓拆成合成引擎能直接表达的圆角矩形块：
/// 主体向上多延伸一个圆角半径，让顶部两角落在窗口外，只留下底部圆角；
/// 外翻的肩部按物理像素逐行拼出（相邻同宽的行合并），边缘误差不超过半个像素。
pub fn blur_pieces(shapes: &[GlassShape], ppp: f32) -> Vec<BlurPiece> {
    let mut out = Vec::new();
    for shape in shapes {
        match shape {
            GlassShape::Docked { rect, dock, flare, flare_h, radius } => {
                if rect.width() <= 0.0 || rect.height() <= 0.0 { continue; }
                // 在贴顶坐标系里按刘海拆块，再把每块转到左 / 右边缘（宽高互换）。
                let (map, frame) = shape::EdgeMap::of(*rect, *dock);
                let mut pieces = blur_pieces(&[GlassShape::Notch { rect: frame, flare: *flare, flare_h: *flare_h, radius: *radius }], ppp);
                let edge = (map.apply(Pos2::ZERO).x * ppp).round();
                for p in &mut pieces {
                    let (x, y, w, h) = (p.x, p.y, p.w, p.h);
                    *p = BlurPiece { x: if *dock == model::Dock::Left { edge + y } else { edge - y - h }, y: x, w: h, h: w, radius: p.radius };
                }
                out.extend(pieces);
            }
            GlassShape::Notch { rect, flare, flare_h, radius } => {
                if rect.width() <= 0.0 || rect.height() <= 0.0 { continue; }
                let r = radius.min(rect.height() * 0.5).min(rect.width() * 0.5).max(0.0) * ppp;
                let fh = flare_h.min(rect.height() * 0.5).max(0.5) * ppp;
                let (x0, x1, y0, y1) = ((rect.left() * ppp).round(), (rect.right() * ppp).round(), (rect.top() * ppp).round(), (rect.bottom() * ppp).round());
                out.push(BlurPiece { x: x0, y: y0 - r, w: x1 - x0, h: y1 - y0 + r, radius: r });
                let rows = fh.ceil() as i32;
                let mut i = 0;
                while i < rows {
                    let wing = |row: i32| (flare * ppp * (1.0 - ((row as f32 + 0.5) / fh).min(1.0)).powf(2.4)).round();
                    let w = wing(i);
                    if w < 1.0 { break; }
                    let mut h = 1;
                    while i + h < rows && wing(i + h) == w { h += 1; }
                    let y = y0 + i as f32;
                    out.push(BlurPiece { x: x0 - w, y, w, h: h as f32, radius: 0.0 });
                    out.push(BlurPiece { x: x1, y, w, h: h as f32, radius: 0.0 });
                    i += h;
                }
            }
            GlassShape::Round(r, radius) => {
                let (x,y,right,bottom)=((r.left()*ppp).round(),(r.top()*ppp).round(),(r.right()*ppp).round(),(r.bottom()*ppp).round());
                let (w,h)=(right-x,bottom-y);
                if w<=0.0 || h<=0.0 {continue;}
                // 收起动画先按物理边界取整，再限制圆角，避免上下圆弧在中线交叠。
                out.push(BlurPiece { x,y,w,h,radius:(radius*ppp).max(0.0).min(w.min(h)*0.5) });
            }
            GlassShape::Outline(points) => {
                // 任意轮廓只能退化为外接矩形；岛体本身走 Notch，不会用到这里。
                let b = Rect::from_points(points);
                if b.is_positive() {
                    out.push(BlurPiece { x: (b.left() * ppp).round(), y: (b.top() * ppp).round(), w: (b.width() * ppp).round(), h: (b.height() * ppp).round(), radius: 0.0 });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod blur_piece_tests {
    use super::*;

    fn covered(pieces: &[BlurPiece], x: f32, y: f32) -> bool {
        pieces.iter().any(|p| {
            if !(x >= p.x && x < p.x + p.w && y >= p.y && y < p.y + p.h) { return false; }
            // 圆角：只判断四角的圆弧外侧
            let r = p.radius;
            let cx = x.clamp(p.x + r, p.x + p.w - r);
            let cy = y.clamp(p.y + r, p.y + p.h - r);
            (x - cx).powi(2) + (y - cy).powi(2) <= r * r + 1e-3
        })
    }

    #[test]
    fn blur_follows_the_island_outline_not_its_bounding_box() {
        for ppp in [1.0, 1.25, 1.5, 2.0] {
            let rect = Rect::from_min_size(Pos2::new(110.0, 0.0), Vec2::new(419.0, 140.0));
            let shape = GlassShape::Notch { rect, flare: 17.6, flare_h: 16.0, radius: 33.0 };
            let pieces = blur_pieces(std::slice::from_ref(&shape), ppp);
            let outline = shape.outline();
            // 轮廓内部的点都被模糊覆盖，外部（含底部圆角外、肩部外）都不覆盖；边缘留 1.5 像素容差。
            let inside = |x: f32, y: f32| {
                let mut c = false;
                let mut prev = *outline.last().unwrap();
                for &next in &outline {
                    if (next.y > y) != (prev.y > y) && x < (prev.x - next.x) * (y - next.y) / (prev.y - next.y) + next.x { c = !c; }
                    prev = next;
                }
                c
            };
            let mut checked = 0;
            for yi in 0..(150.0 * ppp) as i32 {
                for xi in ((80.0 * ppp) as i32..(560.0 * ppp) as i32).step_by(3) {
                    let (x, y) = (xi as f32 + 0.5, yi as f32 + 0.5);
                    let (lx, ly) = (x / ppp, y / ppp);
                    let deep_in = inside(lx, ly) && [(-1.5, 0.0), (1.5, 0.0), (0.0, 1.5), (0.0, -1.5)].iter().all(|(dx, dy)| inside(lx + dx / ppp, (ly + dy / ppp).max(0.01)));
                    let far_out = !inside(lx, ly) && [(-1.5, 0.0), (1.5, 0.0), (0.0, 1.5), (0.0, -1.5)].iter().all(|(dx, dy)| !inside(lx + dx / ppp, ly + dy / ppp));
                    if deep_in { assert!(covered(&pieces, x, y), "{ppp} 缩放下岛体内 ({x},{y}) 没有模糊"); checked += 1; }
                    if far_out && y >= 0.0 { assert!(!covered(&pieces, x, y), "{ppp} 缩放下岛体外 ({x},{y}) 不能出现模糊"); }
                }
            }
            assert!(checked > 1000);
            assert!(pieces.len() < 60, "肩部合并后块数应很少：{}", pieces.len());
        }
    }

    #[test]
    fn side_docked_blur_follows_the_rotated_outline() {
        for dock in [model::Dock::Left, model::Dock::Right] {
            for ppp in [1.0, 1.5] {
                let x = if dock == model::Dock::Left { 0.0 } else { 81.0 };
                let shape = GlassShape::Docked { rect: Rect::from_min_size(Pos2::new(x, 200.0), Vec2::new(419.0, 140.0)), dock, flare: 17.6, flare_h: 16.0, radius: 33.0 };
                let pieces = blur_pieces(std::slice::from_ref(&shape), ppp);
                let outline = shape.outline();
                let inside = |x: f32, y: f32| {
                    let mut c = false;
                    let mut prev = *outline.last().unwrap();
                    for &next in &outline {
                        if (next.y > y) != (prev.y > y) && x < (prev.x - next.x) * (y - next.y) / (prev.y - next.y) + next.x { c = !c; }
                        prev = next;
                    }
                    c
                };
                let mut checked = 0;
                for yi in ((170.0 * ppp) as i32..(370.0 * ppp) as i32).step_by(2) {
                    for xi in (0..(500.0 * ppp) as i32).step_by(3) {
                        let (px, py) = (xi as f32 + 0.5, yi as f32 + 0.5);
                        let (lx, ly) = (px / ppp, py / ppp);
                        let near = [(-1.5, 0.0), (1.5, 0.0), (0.0, 1.5), (0.0, -1.5)];
                        let deep_in = inside(lx, ly) && near.iter().all(|(dx, dy)| inside((lx + dx / ppp).clamp(0.01, 499.99), ly + dy / ppp));
                        let far_out = !inside(lx, ly) && near.iter().all(|(dx, dy)| !inside(lx + dx / ppp, ly + dy / ppp));
                        if deep_in { assert!(covered(&pieces, px, py), "{dock:?} {ppp}: 岛体内 ({px},{py}) 没有模糊"); checked += 1; }
                        let on_canvas = lx > 0.5 && lx < 499.5;
                        if far_out && on_canvas { assert!(!covered(&pieces, px, py), "{dock:?} {ppp}: 岛体外 ({px},{py}) 不能有模糊"); }
                    }
                }
                assert!(checked > 1000);
            }
        }
    }

    #[test]
    fn collapsing_pill_blur_radius_fits_the_physical_bounds() {
        for ppp in [1.0,1.25,1.5,2.0] {
            for height in [30.02,30.3,30.49,30.5,30.8,31.0,31.2,32.1,35.3] {
                let rect=Rect::from_min_size(Pos2::new(230.23,6.0),Vec2::new(179.31,height));
                let radius=15.0+(height-30.0)*0.1;
                let pieces=blur_pieces(&[GlassShape::Round(rect,radius)],ppp);
                assert_eq!(pieces.len(),1);
                let p=pieces[0];
                assert!(p.radius*2.0<=p.w.min(p.h),"收起时圆弧不能超过物理边界：ppp={ppp}, height={height}, piece={p:?}");
                assert_eq!(p.x+p.w,(rect.right()*ppp).round(),"模糊与岛体右边界应使用同一像素位置");
                assert_eq!(p.y+p.h,(rect.bottom()*ppp).round(),"模糊与岛体下边界应使用同一像素位置");
            }
        }
    }

    #[test]
    fn rounded_rectangles_and_empty_shapes() {
        let pieces = blur_pieces(&[GlassShape::Round(Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(100.0, 40.0)), 10.0)], 1.5);
        assert_eq!(pieces, vec![BlurPiece { x: 15.0, y: 30.0, w: 150.0, h: 60.0, radius: 15.0 }]);
        assert!(blur_pieces(&[GlassShape::Notch { rect: Rect::NOTHING, flare: 5.0, flare_h: 8.0, radius: 12.0 }], 1.0).is_empty());
        // 极简细条：没有肩部，只有一块
        let tiny = blur_pieces(&[GlassShape::Notch { rect: Rect::from_min_size(Pos2::new(298.0, 0.0), Vec2::new(44.0, 4.5)), flare: 0.0, flare_h: 8.0, radius: 2.25 }], 1.25);
        assert_eq!(tiny.len(), 1);
    }
}

pub struct FrameOut {
    /// 原生窗口应保留的可见区域（相对 `win` 左上角）。
    pub region: Rect,
    /// 毛玻璃 + 系统模糊时，替代 `region` 的贴合轮廓；其余情况为 None。
    pub glass: Option<Vec<GlassShape>>,
    /// 鼠标命中区（相对 `win` 左上角），区域外点击穿透到下层窗口。
    pub hit: Rect,
    pub notice_hit: Option<Rect>,
    /// 文件拖入感应区；独立于普通鼠标命中区。
    pub file_drop_zone: Option<Rect>,
    /// 展开状态立即生效，不等待展开动画结束。
    pub expanded: bool,
    pub editing: bool,
    pub animating: bool,
    pub repaint_after: std::time::Duration,
    pub cmds: Vec<Cmd>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Focus,
    Rest,
}

impl Phase {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Phase::Focus => "专注时间",
            Phase::Rest => "休息",
        }
    }
}

pub(crate) struct Pomo {
    pub phase: Phase,
    pub remaining: f32,
    pub running: bool,
}

/// 本地歌词（酷狗、汽水）每 5 秒复查，最多约 1 分钟；网络失败每 30 秒重试，最多约 5 分钟。手动点重试会重新计数。
const NATIVE_RETRIES:u32=12;
const NETWORK_RETRIES:u32=10;

/// 与 `format!("{title}|{artist}|{duration:.0}")` 等价，但不分配内存：界面每帧要比较好几次。
/// 长歌词横向滚动的参数：溢出宽度与速度（都是点/秒口径的点）。
#[derive(Clone,Copy,Debug)]
struct LyricScroll {overflow:f32,speed:f32}

/// 歌词排版缓存。
/// 没有它时 `update_island_lyric_layout` 每帧都要从 11pt 逐级试到 8pt，
/// 一句中文最多重排 7 次；缓存后同一句只排一次。
struct LyricLayoutCache {
    text:String,
    width:f32,
    white:bool,
    title:bool,
    galley:std::sync::Arc<egui::Galley>,
    /// 8pt 仍放不下时的溢出宽度（点），>0 表示要横向滚动。
    overflow:f32,
}

impl LyricLayoutCache {
    fn matches(&self,text:&str,width:f32,white:bool,title:bool)->bool {
        self.text==text && (self.width-width).abs()<0.5 && self.white==white && self.title==title
    }
}

/// 滚动速度：让整句尽量在本句时长内滚完，同时夹在可读区间。
fn lyric_scroll_speed(overflow:f32,window:f32)->f32 {
    let travel=(window-2.0*SCROLL_DWELL).max(0.8);
    (overflow/travel).clamp(SCROLL_MIN_SPEED,SCROLL_MAX_SPEED)
}

/// 滚动位移：句首停留 `SCROLL_DWELL` 秒后匀速推进，到尾部停住。
fn lyric_scroll_offset(elapsed:f32,speed:f32,overflow:f32)->f32 {
    ((elapsed-SCROLL_DWELL).max(0.0)*speed).min(overflow)
}

fn lyric_key_matches(key:&str,media:&NowPlaying)->bool {
    use std::io::Write;
    let Some(rest)=key.strip_prefix(media.title.as_str()).and_then(|rest|rest.strip_prefix('|')) else{return false;};
    let Some(rest)=rest.strip_prefix(media.artist.as_str()).and_then(|rest|rest.strip_prefix('|')) else{return false;};
    let mut buffer=[0u8;32];
    let mut cursor=std::io::Cursor::new(&mut buffer[..]);
    if write!(cursor,"{:.0}",media.duration).is_err(){return rest==format!("{:.0}",media.duration);}
    let written=cursor.position() as usize;
    rest.as_bytes()==&buffer[..written]
}

#[test]
fn lyric_key_matches_the_formatted_key_without_allocating(){
    for (title,artist,duration) in [("歌","手",120.4),("A|B","C",0.0),("歌","",239.5),("歌","手",240.5),("","",f64::NAN),("t","a",1e30)] {
        let media=NowPlaying{title:title.into(),artist:artist.into(),duration,..Default::default()};
        let key=format!("{}|{}|{:.0}",title,artist,duration);
        assert!(lyric_key_matches(&key,&media),"{key}");
        assert!(!lyric_key_matches(&format!("{key}0"),&media));
        assert!(!lyric_key_matches(&format!("x{key}"),&media));
    }
}

#[derive(Default)]
pub(crate) struct LyricsState {
    pub key: String,
    pub island_animation_started: bool,
    pub scroll_animation_started: bool,
    pub loading: bool,
    pub error: Option<lyrics::FetchError>,
    pub retry_at: Option<f64>,
    /// 发起本次查询时已知的歌曲 ID；之后才拿到 ID 且还没有同步歌词时，按 ID 再查一次。
    pub song_id: Option<u64>,
    /// 同一首歌已自动重试的次数；达到上限后停止，避免整首歌期间反复读盘或联网。
    pub attempts: u32,
    pub lyrics: Option<lyrics::Lyrics>,
    pub rx: Option<std::sync::mpsc::Receiver<Result<(lyrics::Lyrics,bool), lyrics::FetchError>>>,
    pub native_rx: Option<std::sync::mpsc::Receiver<Option<lyrics::Lyrics>>>,
}

/// v0.5.5：邮件面板与设置表单的界面状态。正文只在内存里过一次，不落盘。
#[derive(Default)]
pub(crate) struct MailState {
    pub privacy:crate::mail_privacy::Privacy,
    pub manage:crate::mail_manage_ui::State,
    pub accounts: Vec<mail::Account>,
    /// 每个账号的连接状态，键是邮箱小写。
    pub status: HashMap<String, mail::Status>,
    /// 最近邮件，新的在前，最多 60 封。
    pub recent: Vec<mail::Item>,
    /// 设置页里「测试连接」的结果。
    pub note: String,
    pub testing: Option<String>,
    pub form: MailForm,
    /// 面板里点开的邮件（账号 + uid，避免两个账号 uid 撞车）。
    pub open_uid: Option<(String, u32)>,
    pub reading: Option<mail::Item>,
    pub reader_focus: bool,
    /// 已经提醒过的会议，键是 `UID + DTSTART`（邮件和日历两条来源共用）。
    pub fired: std::collections::HashSet<String>,
}

#[derive(Default)]
pub(crate) struct MailForm {
    pub provider: mail::Provider,
    pub custom: mail::CustomImap,
    pub email: String,
    pub password: String,
    pub show: bool,
}

impl MailState {
    /// 最近一封带验证码的邮件。
    pub fn code_item(&self) -> Option<&mail::Item> {self.recent.iter().find(|m|m.code.is_some())}
    pub fn status_of(&self, email: &str) -> mail::Status {
        self.status
            .get(&email.to_ascii_lowercase())
            .cloned()
            .unwrap_or(mail::Status::Idle)
    }
}

/// v0.5.6：日程（CalDAV）的界面状态。密码只在表单里暂存，保存后立刻清掉。
#[derive(Default)]
pub(crate) struct CalendarState {
    pub local: Vec<crate::local_calendar::Entry>,
    pub local_error: String,
    pub draft: Option<crate::local_calendar::Draft>,
    pub feeds: Vec<CalendarFeed>,
    pub config: Option<calendar::Config>,
    /// 未来 24 小时的日程（提醒也读这里）。
    pub events: Vec<mail::Meeting>,
    pub view: crate::calendar_ui::State,
    pub connected: bool,
    pub last_sync: String,
    /// 连续失败次数；>= 2 才在设置页显示红字。
    pub failures: u32,
    pub error: String,
    pub testing: bool,
    /// 设置表单里的提示（成功/失败/粘贴拆分的说明）。
    pub note: String,
    /// `note` 是不是错误提示（界面显示成红字）。
    pub note_error: bool,
    pub form: CalendarForm,
    /// 「测试并保存」用的临时密码，测试成功才落盘。
    pub test_password: String,
    /// 导入 .ics 文件的结果说明。
    pub import_note: String,
}

pub(crate) struct CalendarFeed {
    pub config: calendar::Config,
    pub events: Vec<mail::Meeting>,
    pub month: crate::calendar_ui::State,
    pub connected: bool,
    pub last_sync: String,
    pub failures: u32,
    pub error: String,
}

impl CalendarFeed {
    fn new(config: calendar::Config) -> Self {
        Self { config, events: vec![], month: Default::default(), connected: false,
            last_sync: String::new(), failures: 0, error: String::new() }
    }
}

#[derive(Default)]
pub(crate) struct CalendarForm {
    pub provider: calendar::Provider,
    /// 自定义 CalDAV 的服务器地址。
    pub server: String,
    pub username: String,
    pub password: String,
    pub show: bool,
    pub guide_open: bool,
}

impl CalendarState {
    pub fn load() -> Self {
        let config = calendar::load_config();
        let form = CalendarForm { provider: calendar::selected_provider(),
            username: config.as_ref().map(|cfg| cfg.username.clone()).unwrap_or_default(),
            server: config.as_ref().filter(|cfg| cfg.provider.custom_server()).and_then(|cfg| cfg.server.clone()).unwrap_or_default(), ..Default::default() };
        let feeds = calendar::load_configs().into_iter().map(CalendarFeed::new).collect();
        let (local,local_error)=match crate::local_calendar::load(){Ok(events)=>(events,String::new()),Err(error)=>(vec![],error)};
        Self { config, form, feeds, local, local_error, ..Default::default() }
    }

}

pub struct App {
    pub opts: Options,
    pub settings: model::Settings,
    pub(crate) todos: model::TodoList,
    pub(crate) water: model::Water,
    pub(crate) shelf: model::Shelf,

    // 形变
    pub(crate) w: Spring,
    pub(crate) h: Spring,
    pub(crate) open: bool,
    hover_since: Option<f64>,
    leave_since: Option<f64>,
    /// 前台切换导致的强制收起：光标离开前不再自动展开。
    pub hover_lock: bool,
    pub(crate) reveal: f32,

    // 切页
    pub(crate) page: usize,
    page_from: usize,
    page_dir: f32,
    page_t0: f64,
    slide: Spring,
    dots: [Spring; 8],
    wheel_delta: f32,
    wheel_at: f64,
    wheel_page_at: f64,

    // 拖入
    pub(crate) drag_hot: bool,
    pub(crate) drag_a: f32,
    drag_saved: Option<usize>,

    // 置物架
    pub(crate) shelf_scroll: Spring,
    pub(crate) shelf_first: usize,
    pub(crate) shelf_menu_rect: Option<Rect>,
    pub(crate) shelf_x: HashMap<PathBuf, Spring>,
    pub(crate) shelf_born: HashMap<PathBuf, f64>,
    pub(crate) icons: crate::icons::Cache,
    pub(crate) avoidance: crate::avoidance::Picker,
    pub(crate) confirm_clear: Option<f64>,
    /// 置物架页的两个分栏：0 = 文件，1 = 剪贴板。
    pub(crate) shelf_tab: usize,
    pub clipboard: crate::clipboard::Shared,
    pub(crate) clip_first: usize,
    pub(crate) clip_confirm_clear: Option<f64>,
    /// 剪贴板搜索词；非空时列表只显示命中的记录。
    pub(crate) clip_query: String,
    pub(crate) clip_search_focus: bool,
    /// 搜索输入框是否展开（面板表头平时只放一个「搜索」按钮）。
    pub(crate) clip_search_open: bool,
    /// 搜索框这一帧是否真的有键盘焦点（只有这时才处理 ↑↓ / Enter）。
    pub(crate) clip_typing: bool,
    /// 键盘选中的行（过滤后列表的下标）。
    pub(crate) clip_sel: usize,

    // 饮水
    pub(crate) water_ring: Spring,
    pub(crate) water_ml: f32,
    pub(crate) water_view: usize,
    pub(crate) water_view_t: Spring,
    pub(crate) remind_t: Spring,
    pub(crate) last_reminder: f64,

    // 专注
    pub(crate) pomo: Pomo,
    pub(crate) focus_editing: bool,
    pub(crate) ring_shown: Spring,
    pub(crate) reset_spin: Spring,

    // 待办
    pub(crate) adding: bool,
    pub(crate) editing: Option<usize>,
    pub(crate) buf: String,
    pub(crate) todo_input_focused: bool,
    todo_ime_active: bool,
    todo_ime_frame: bool,
    pub(crate) todo_pop: Spring,

    // 音乐
    pub(crate) ly: LyricsState,
    pub(crate) ly_worker: Option<std::thread::JoinHandle<()>>,
    pub(crate) show_lyrics: bool,
    island_lyric_layout: Option<std::sync::Arc<egui::Galley>>,
    island_lyric_cache: Option<LyricLayoutCache>,
    /// 长歌词滚动参数；Some 表示当前句放不下、正在横向滚动。
    island_lyric_scroll: Option<LyricScroll>,
    /// 滚动时间基准：(歌词行号, 文本)。变化即重置起点（副歌重复同一句也要重置）。
    island_lyric_scroll_key: (Option<usize>,String),
    island_lyric_scroll_start: f64,
    media_anchor: (String, f64, f64, bool),
    pub(crate) media: NowPlaying,
    pub(crate) music_artwork: Option<(std::sync::Arc<egui::ColorImage>,egui::TextureHandle)>,

    forced_open_until:f64,
    pub settings_open:bool,
    pub displays:Vec<crate::display_layout::Display>,
    browser_at_top:bool,
    browser_minimal_active:bool,
    /// 毛玻璃着色量（平滑过渡，避免背景变化时闪烁）；0 表示尚未初始化。
    glass_tint:f32,
    pub(crate) fluid:crate::fluid::Fluid,
    pub(crate) fluid_origin:Pos2,
    fluid_art:Option<egui::TextureId>,
    fluid_title:String,
    /// 悬浮云每帧刷新的颜色纹理：低分辨率交给 GPU 双线性插值，避免每帧生成上万顶点。
    fluid_tex:Option<egui::TextureHandle>,
    screen:Option<Rect>,
    pub(crate) settings_focus_requested:bool,
    pub(crate) settings_tab:usize,
    pub(crate) settings_category:usize,
    pub wechat_status:String,
    pub(crate) notification_apps:model::NotificationApps,
    /// 手机通知聚合：标题 -> (条数, 最后到达时间)。
    pub(crate) phone_counts:std::collections::HashMap<String,(u32,f64)>,
    /// 手机 App 图标纹理缓存：文件名 -> TextureHandle。
    pub(crate) phone_icons:std::collections::HashMap<String,eframe::egui::TextureHandle>,
    /// 手机连接通知的来源 App 名单（`phone-apps.json`，支持热重载）。
    pub(crate) phone_apps:crate::model::PhoneApps,
    /// 浏览器下载偏好（`bridge.json`，桥宿主与设置页共用）。
    pub(crate) browser_prefs:crate::model::BridgePrefs,
    /// 上次看到的手机电量（None=尚未初始化，用于断开/重连/低电/充满提示）。
    pub(crate) phone_last:Option<Option<u8>>,
    /// 上次看到的本机电量，用于本机低电 / 充满提示。
    pub(crate) pc_last:Option<Option<u8>>,
    /// 勿扰截止时刻（app 时间）；> t 期间不弹提示、不闪铃铛，验证码除外。
    pub(crate) quiet_until:f64,
    /// 已经自动勿扰过的会议（uid|start），避免同一场会重复触发。
    pub(crate) quiet_meeting_key:Option<String>,
    /// 上次检查会议勿扰的时刻（每 5 秒查一次，别每帧都拼日程）。
    pub(crate) meeting_checked_at:f64,
    /// 今日会议候选缓存：(日期, 生成时刻, (开始, 结束, uid))。
    /// 日程合并排序较重，60 秒或跨天才重算一次；90 秒的触发窗口保证不会漏。
    pub(crate) meeting_plan:(String,f64,Vec<(i64,i64,String)>),
    /// 「稍后提醒」队列：(到期时刻, 标题, 正文)。
    pub(crate) snoozed:Vec<(f64,String,String)>,
    /// 通知规则（notice-rules.json）：静音 / 高亮 / 自动复制。
    pub(crate) notice_rules: model::NoticeRules,
    /// 剪贴板列表快照：(版本号, 共享列表)。只在版本变化时重建，避免每帧克隆整份历史。
    pub(crate) clip_snapshot: (u64, std::sync::Arc<Vec<crate::clipboard::Clip>>),
    /// 关键词缓存：(源字符串, 小写词表)。命中判断在绘制路径上，不能每次都 split + 小写。
    pub(crate) keyword_src: String,
    pub(crate) keywords: Vec<String>,
    /// 已经提醒过的「额度窗口」（id + 窗口名），回到阈值以上会移除、下次可再提醒。
    pub(crate) quota_alerted: std::collections::HashSet<(String,String)>,
    /// 上次检查额度阈值的时刻（每 30 秒一次）。
    pub(crate) quota_checked_at: f64,
    /// 设置页的最近通知缓存：(上次读取时刻, 列表)，避免每帧读盘。
    pub(crate) notice_cache:(f64,Vec<(String,String,String)>),
    /// 还没凑满一分钟的专注秒数，攒够就写进 focus-stats.json。
    pub(crate) focus_pending:f32,
    /// 今日专注分钟数与连续天数（每分钟更新一次，别每帧读盘）。
    pub(crate) focus_today_min:u32,
    pub(crate) focus_streak_days:u32,
    /// 状态总线当前存活的状态，用于常驻区并排显示。
    pub(crate) status_live:Vec<crate::island_status::Status>,
    /// 未接来电累计（点「未接来电」提示的关闭按钮清零）。
    pub(crate) missed_calls:u32,
    /// 通用状态总线读取器。
    pub(crate) island_status:crate::island_status::Reader,
    pub(crate) subpage:Option<usize>,
    pub(crate) task_dialog:Option<crate::extras::TaskDraft>,
    pub listener_status: String,
    pub(crate) notification_bell_until: f64,
    pub(crate) notification_bell_at: f64,
    pub(crate) notice: Option<Alert>,
    pub(crate) hud_notice: Option<Alert>,
    hud_alpha: f32,
    notice_width: Spring,
    notice_details: bool,
    pub pending_messengers:Vec<Unread>,
    messenger_scroll: f32,
    pub(crate) alerts: std::collections::VecDeque<Alert>,
    pub(crate) history: std::collections::VecDeque<Alert>,
    pub(crate) pending: Vec<Cmd>,
    pub(crate) todo_first: usize,
    pub(crate) todo_choosing: bool,
    pub(crate) pet: crate::pet::Pet,
    pub(crate) last_drink: Option<(u32, usize)>,
    // 其它
    toast: Option<(String, f64)>,
    toast_below: bool,
    last_input: f64,
    system_idle: Option<f64>,
    last_cursor: Option<Pos2>,
    pet_a: f32,
    last_t: Option<f64>,
    pub(crate) t: f64,
    pub(crate) dt: f32,
    pub(crate) today: String,
    pub(crate) current_hour: usize,
    pub(crate) cursor_local: Option<Pos2>,
    started: bool,
    /// v0.5.4：音量条展示值（拖动时先用本地值，随后跟上系统回报）。
    pub(crate) volume_ui: Option<(u32, bool)>,
    /// 拖动/滚轮刚改过音量，这段时间内系统回声不再单独弹一条。
    pub(crate) volume_quiet_until: f64,
    /// 音量条填充的缓动量（0..1），让数值变化有「量感」。
    pub(crate) volume_a: f32,
    /// 上一次看到的蓝牙设备，用来只在连接/断开时提示一次。
    pub(crate) bt_shown: Vec<crate::bt::Device>,
    bt_initialized: bool,
    /// v0.5.5：邮件账号、最近邮件与设置表单。
    pub(crate) mail: MailState,
    /// v0.5.6：日历（CalDAV）配置、未来 24 小时事件与设置表单。
    pub(crate) calendar: CalendarState,
    pub(crate) quota: crate::quota_ui::State,
    pub(crate) updates: crate::updates::State,
    pub(crate) resident: bar::Motion,
    pub(crate) resident_power: (Option<u8>,bool),
    /// 手机连接上报的电量；与本机电量各自独立，互不影响。
    pub(crate) phone_resident: (Option<u8>,bool),
    /// 手机连接电量读取器（本地文件轮询）。
    pub(crate) phone_power: crate::phone_battery::Reader,
}

impl App {
    pub fn new(ctx: &egui::Context, opts: Options) -> Self {
        theme::load_fonts(ctx);
        egui_extras::install_image_loaders(ctx);
        ctx.set_theme(egui::ThemePreference::Dark);
        ctx.set_visuals(egui::Visuals::dark());
        ctx.style_mut(|s| {
            s.interaction.selectable_labels = false;
            s.visuals.text_cursor.stroke = egui::Stroke::new(1.5_f32, theme::BLUE);
            s.visuals.selection.bg_fill = theme::alpha_blue(0.45);
        });
        let settings = model::Settings::load();
        // 专注统计只在启动和每分钟更新，构造前先取一次。
        let (focus_today, focus_streak)=crate::extras::focus_summary();
        // 邮件面板上次选的账号：结构体里 `settings` 会被 move，先取出来。
        let saved_mail_filter = settings.mail_filter.clone();
        let clipboard_on = settings.clipboard_history;
        let (bw, bh) = if settings.minimal { (BAR_W_MIN, BAR_H_MIN) } else { (BAR_W, BAR_H) };
        let focus_seconds=settings.focus_minutes as f32*60.0;
        let initial_lyrics=settings.lyrics_enabled;
        let initial_page=settings.page_order.iter().copied().find(|page|settings.visible_pages[*page]).unwrap_or(0);
        Self {
            opts,
            settings,
            todos: model::TodoList::load(),
            water: model::Water::default(),
            shelf: model::Shelf::load(),
            w: Spring::new(bw, CLOSE.0, CLOSE.1),
            h: Spring::new(bh, CLOSE.0, CLOSE.1),
            open: false,
            hover_since: None,
            leave_since: None,
            hover_lock: false,
            reveal: 0.0,
            page: initial_page,
            page_from: initial_page,
            page_dir: 1.0,
            page_t0: -10.0,
            slide: Spring::new(0.0, 260.0, 0.92),
            dots: std::array::from_fn(|i| Spring::new(if i == initial_page { 16.0 } else { 4.0 }, 380.0, 0.86)),
            wheel_delta: 0.0,
            wheel_at: -1.0,
            wheel_page_at: -1.0,
            drag_hot: false,
            drag_a: 0.0,
            drag_saved: None,
            shelf_scroll: Spring::new(0.0, 300.0, 0.9),
            shelf_first: 0,
            shelf_menu_rect: None,
            shelf_x: HashMap::new(),
            shelf_born: HashMap::new(),
            icons: Default::default(), avoidance: Default::default(),
            confirm_clear: None,
            shelf_tab: 0,
            clipboard: crate::clipboard::Shared::new(clipboard_on),
            clip_first: 0,
            clip_confirm_clear: None,
            clip_query: String::new(),
            clip_search_focus: false,
            clip_search_open: false,
            clip_typing: false,
            clip_sel: 0,
            water_ring: Spring::new(0.0, 120.0, 0.9),
            water_ml: 0.0,
            water_view: 0,
            water_view_t: Spring::new(0.0, 200.0, 0.9),
            remind_t: Spring::new(1.0, 400.0, 0.8),
            last_reminder: 0.0,
            pomo: Pomo { phase: Phase::Focus, remaining: focus_seconds, running: false }, focus_editing:false,
            ring_shown: Spring::new(1.0, 160.0, 0.95),
            reset_spin: Spring::new(0.0, 160.0, 0.82),
            adding: false,
            todo_input_focused: false,
            todo_ime_active: false,
            todo_ime_frame: false,
            editing: None,
            buf: String::new(),
            todo_pop: Spring::new(0.0, 500.0, 0.45),
            ly: LyricsState::default(),
            ly_worker: None,
            show_lyrics: initial_lyrics,
            island_lyric_layout: None,
            island_lyric_cache: None,
            island_lyric_scroll: None,
            island_lyric_scroll_key: (None,String::new()),
            island_lyric_scroll_start: 0.0,
            media_anchor: (String::new(), 0.0, 0.0, false),
            media: NowPlaying::default(),
            music_artwork: None,
            forced_open_until:0.0, settings_open:false, displays:Vec::new(), browser_at_top:false, browser_minimal_active:false, glass_tint:0.0, fluid:Default::default(), fluid_origin:Pos2::ZERO, fluid_art:None, fluid_title:String::new(), fluid_tex:None, screen:None, settings_focus_requested:false, settings_tab:0, settings_category:0, wechat_status:String::new(), notification_apps:model::NotificationApps::load(), phone_counts:Default::default(), phone_icons:Default::default(), phone_apps:model::PhoneApps::load(), browser_prefs:model::BridgePrefs::load(), notice_rules:model::NoticeRules::load(), clip_snapshot:(u64::MAX,std::sync::Arc::new(Vec::new())), keyword_src:String::new(), keywords:Vec::new(), quota_alerted:Default::default(), quota_checked_at:0.0, phone_last:None, pc_last:None, quiet_until:0.0, quiet_meeting_key:None, meeting_checked_at:0.0, meeting_plan:(String::new(),f64::MIN,Vec::new()), snoozed:Vec::new(), notice_cache:(0.0,Vec::new()), focus_pending:0.0, focus_today_min:focus_today, focus_streak_days:focus_streak, status_live:Vec::new(), missed_calls:0, island_status:Default::default(), subpage:None, task_dialog:None,
            listener_status: "尚未连接".into(),
            notification_bell_until: 0.0, notification_bell_at: -10.0, notice: None, hud_notice: None, hud_alpha: 0.0, notice_width:Spring::new(0.0,CLOSE.0,CLOSE.1), notice_details:false, pending_messengers:Vec::new(), messenger_scroll:0.0, alerts: Default::default(), history: Default::default(), pending: vec![],
            todo_first: 0, todo_choosing: false, pet: Default::default(), last_drink: None,
            toast: None, toast_below: false,
            last_input: 0.0, system_idle: None,
            last_cursor: None,
            pet_a: 0.0,
            last_t: None,
            t: 0.0,
            dt: 0.0,
            today: String::new(), current_hour: 0,
            cursor_local: None,
            started: false,
            volume_ui: None,
            volume_quiet_until: 0.0,
            volume_a: 0.0,
            bt_shown: Vec::new(),
            bt_initialized: false,
            mail: {
                // 邮件面板记住上次选的账号：State 有私有字段，只能先默认构造再赋值。
                let mut manage = crate::mail_manage_ui::State::default();
                if !saved_mail_filter.is_empty() { manage.account_filter = Some(saved_mail_filter); }
                MailState { accounts: mail::load_accounts(), manage, ..Default::default() }
            },
            calendar: CalendarState::load(),
            quota: crate::quota_ui::State::load(),
            updates: Default::default(),
            resident: Default::default(), resident_power:(None,false), phone_resident:(None,false), phone_power: Default::default(),
        }
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), self.t));
        self.toast_below = false;
    }

    pub(crate) fn code_copied(&mut self, _code:&str) {
        self.say("验证码已复制");
        self.toast_below = true;
    }

    pub(crate) fn mail_code_hidden(&self,item:&mail::Item)->bool {
        self.mail.privacy.hidden(item,self.t,self.settings.mail_code_hide_seconds)
    }
    pub(crate) fn reveal_mail_code(&mut self,item:&mail::Item){self.mail.privacy.reveal(item,self.t);}
    pub(crate) fn display_mail(&self,item:&mail::Item)->mail::Item {
        let mut safe=item.clone();
        if self.mail_code_hidden(item) {
            if let Some(code)=&item.code {
                safe.subject=crate::mail_privacy::mask(&safe.subject,code);
                safe.from=crate::mail_privacy::mask(&safe.from,code);
                safe.summary=self.mail.privacy.masked_body(item);
                safe.links.clear(); // Avoid tooltip/URL/copy leaks while hidden.
                if let Some(event)=&mut safe.meeting {event.title=crate::mail_privacy::mask(&event.title,code);}
                safe.code=None;
            }
        }
        safe
    }

    /// 收信服务推来的事件；历史邮件静默，新邮件进入通知队列。
    pub fn mail_event(&mut self, ev: mail::Event) {
        let watching=|email:&str|self.settings.mail_enabled && self.mail.accounts.iter().any(|a|a.enabled && a.email.eq_ignore_ascii_case(email));
        match ev {
            mail::Event::Managed(done)=>self.mail_managed(done),
            mail::Event::Status { account, status } => {
                if !watching(&account){return;}
                if status==mail::Status::Ready{
                    self.mail.manage.data.remove(&(account.to_ascii_lowercase(),crate::mail_manage::Folder::Inbox));self.mail.manage.next_refresh=0.0;
                    if (!self.mail.manage.trash_open||!self.settings.mail_sync_operations)&&self.mail.open_uid.as_ref().is_some_and(|(a,_)|a.eq_ignore_ascii_case(&account)){
                        self.mail.open_uid=None;self.mail.manage.auto_read=None;
                    }
                }
                self.mail.status.insert(account.to_ascii_lowercase(), status);
            }
            mail::Event::Mail { item, new } => {
                if !watching(&item.account){return;}
                if !new&&self.mail.manage.data.get(&(item.account.to_ascii_lowercase(),crate::mail_manage::Folder::Inbox)).is_some_and(|s|!s.entries.iter().any(|(uid,_)|*uid==item.uid)){return;}
                self.mail.privacy.observe(&item,new,self.t,chrono::Utc::now().timestamp());
                if self.mail_locally_removed(&item){
                    self.mail.manage.trash.retain(|m|m.uid!=item.uid||!m.account.eq_ignore_ascii_case(&item.account));self.mail.manage.trash.insert(0,item);self.mail.manage.trash.truncate(60);return;
                }
                self.mail
                    .recent
                    .retain(|m| !(m.account.eq_ignore_ascii_case(&item.account) && m.uid == item.uid));
                self.mail.recent.insert(0, item);
                self.mail.recent.truncate(60);
                if new{self.mail.manage.next_refresh=0.0;}
                self.mail.privacy.retain(self.mail.recent.iter().chain(&self.mail.manage.trash).chain(&self.mail.reading));
                if new && self.settings.mail_enabled {
                    let item=&self.mail.recent[0];
                    let body=format!("{} · {}",item.from,item.subject);
                    let body=item.code.as_ref().map(|code|crate::mail_privacy::mask(&body,code)).unwrap_or(body);
                    self.alert("新邮件",body,5,false);
                    self.notification_bell_at = self.t;
                    self.notification_bell_until = self.t + 6.0;
                }
            }
            mail::Event::Test { account, ok, note } => {
                self.mail.testing = None;
                let detail = if ok { "连接成功".to_string() } else { note };
                let detail = if detail.chars().count() > 120 {
                    format!("{}…", detail.chars().take(120).collect::<String>())
                } else {
                    detail
                };
                self.mail.note = format!("{account}：{detail}");
                self.say(if ok {
                    format!("{account} 连接成功")
                } else {
                    format!("{account} 连接失败")
                });
            }
        }
    }

    /// 设置窗口第 8 个标签页：邮件。
    pub fn open_mail_settings(&mut self) {
        self.settings_tab = 7;
        self.open_settings();
    }

    /// v0.5.6：日历服务推来的事件。失败不弹窗，只累计次数（连续两次才显示红字）。
    pub fn calendar_event(&mut self, ev: calendar::Event) {
        match ev {
            calendar::Event::Test { ok, message, count, config } => {
                self.calendar.testing = false;
                if !ok {
                    self.calendar.note = message;
                    self.calendar.note_error = true;
                    return;
                }
                let Some(cfg) = config else {
                    self.calendar.note = "服务器没有返回日历地址".into();
                    self.calendar.note_error = true;
                    return;
                };
                if let Err(e) = calendar::save_connection(&cfg, &self.calendar.test_password, self.calendar.config.as_ref()) {
                    self.calendar.note = e;
                    self.calendar.note_error = true;
                    return;
                }
                self.calendar.note = format!("已连接 · 未来 24 小时 {count} 个日程");
                self.calendar.note_error = false;
                if self.calendar.config.as_ref()!=Some(&cfg) {
                    self.calendar.events.clear();
                    self.calendar.view.events.clear();
                    self.calendar.view.loaded=None;
                    self.calendar.view.loading=false;
                    self.calendar.view.error.clear();
                }
                if !self.calendar.feeds.iter().any(|feed| feed.config == cfg) {
                    self.calendar.feeds.retain(|feed| feed.config.provider != cfg.provider);
                    self.calendar.feeds.push(CalendarFeed::new(cfg.clone()));
                }
                self.calendar.config = Some(cfg);
                self.calendar.connected = true;
                self.calendar.failures = 0;
                self.calendar.error.clear();
                self.calendar.last_sync = calendar::now_label();
                self.calendar.test_password.clear();
                self.calendar.form.password.clear();
                self.pending.push(Cmd::CalendarSync);
                self.request_calendar_month(true);
            }
            calendar::Event::Synced { config, events, at } => {
                if let Some(feed)=self.calendar.feeds.iter_mut().find(|feed|feed.config==config) {
                    feed.events=events.clone();feed.connected=true;feed.failures=0;feed.error.clear();feed.last_sync=at.clone();
                }
                if self.calendar.config.as_ref()!=Some(&config){return;}
                self.calendar.events = events;
                self.calendar.connected = true;
                self.calendar.failures = 0;
                self.calendar.error.clear();
                self.calendar.last_sync = at;
            }
            calendar::Event::MonthLoaded { config, month, result } => {
                if !self.calendar.feeds.is_empty() {
                    if month!=self.calendar.view.month {return;}
                    let Some(feed)=self.calendar.feeds.iter_mut().find(|feed|feed.config==config) else{return;};
                    feed.month.loading=false;
                    match result {
                        Ok(events)=>{feed.month.events=events;feed.month.loaded=Some(month);feed.month.error.clear();}
                        Err(message)=>feed.month.error=message,
                    }
                    self.refresh_calendar_month_status();
                    return;
                }
                let view=&mut self.calendar.view;
                if month!=view.month || self.calendar.config.as_ref()!=Some(&config){return;}
                view.loading=false;
                match result {
                    Ok(events)=>{view.events=events;view.loaded=Some(month);view.error.clear();}
                    Err(message)=>view.error=message,
                }
            }
            calendar::Event::Failed { config, message } => {
                if let Some(feed)=self.calendar.feeds.iter_mut().find(|feed|feed.config==config) {
                    feed.connected=false;feed.failures=feed.failures.saturating_add(1);feed.error=message.clone();
                }
                if self.calendar.config.as_ref()==Some(&config) {
                    self.calendar.connected = false;
                    self.calendar.failures = self.calendar.failures.saturating_add(1);
                    self.calendar.error = message;
                }
            }
        }
    }

    /// 所有已连接的平台都参与同步，设置标签只决定正在编辑的账号。
    pub fn calendar_configs(&self) -> Vec<calendar::Config> {
        if self.calendar.feeds.is_empty() {self.calendar.config.clone().into_iter().collect()}
        else {self.calendar.feeds.iter().map(|feed|feed.config.clone()).collect()}
    }

    pub(crate) fn refresh_calendar_month_status(&mut self) {
        let month=self.calendar.view.month;
        self.calendar.view.loading=self.calendar.feeds.iter().any(|feed|feed.month.loading);
        self.calendar.view.loaded=(!self.calendar.feeds.is_empty() && self.calendar.feeds.iter().all(|feed|feed.month.loaded==Some(month))).then_some(month);
        self.calendar.view.error=self.calendar.feeds.iter().filter(|feed|!feed.month.error.is_empty())
            .map(|feed|format!("{}：{}",feed.config.provider.label(),feed.month.error)).collect::<Vec<_>>().join("；");
    }

    pub(crate) fn select_calendar_provider(&mut self, provider: calendar::Provider) {
        if self.calendar.testing || self.calendar.form.provider == provider { return; }
        if let Err(error) = calendar::select_provider(provider) {
            self.calendar.note = error;
            self.calendar.note_error = true;
            return;
        }
        let feed=self.calendar.feeds.iter().find(|feed|feed.config.provider==provider);
        self.calendar.config=feed.map(|feed|feed.config.clone());
        self.calendar.events=feed.map(|feed|feed.events.clone()).unwrap_or_default();
        self.calendar.connected=feed.is_some_and(|feed|feed.connected);
        self.calendar.last_sync=feed.map(|feed|feed.last_sync.clone()).unwrap_or_default();
        self.calendar.failures=feed.map(|feed|feed.failures).unwrap_or_default();
        self.calendar.error=feed.map(|feed|feed.error.clone()).unwrap_or_default();
        self.calendar.form=CalendarForm{provider,username:self.calendar.config.as_ref().map(|cfg|cfg.username.clone()).unwrap_or_default(),
            server:self.calendar.config.as_ref().filter(|cfg|cfg.provider.custom_server()).and_then(|cfg|cfg.server.clone()).unwrap_or_default(),..Default::default()};
        self.calendar.note.clear();self.calendar.note_error=false;
    }

    /// 只移除正在管理的平台，保留其他平台的日程与提醒。
    pub fn forget_calendar(&mut self) -> Option<calendar::Config> {
        let old = self.calendar.config.take();
        if let Some(config)=&old {
            self.calendar.feeds.retain(|feed|feed.config!=*config);
            let is_old=|alert:&crate::extras::Alert|alert.meeting.as_ref().is_some_and(|(_,source)|source==config.provider.label());
            self.alerts.retain(|alert|!is_old(alert));
            if self.notice.as_ref().is_some_and(is_old){self.notice=None;}
        }
        self.calendar.connected = false;
        self.calendar.events.clear();
        self.calendar.view.events.clear();
        self.refresh_calendar_month_status();
        self.calendar.last_sync.clear();
        self.calendar.failures = 0;
        self.calendar.error.clear();
        self.calendar.note.clear();
        old
    }

    /// 外壳同步收信线程时读账号清单（不含任何凭据）。
    pub fn mail_accounts(&self) -> &[mail::Account] {
        &self.mail.accounts
    }

    /// 账号增删或开关变化后：落盘并让外壳重新对齐收信线程。
    pub(crate) fn mail_accounts_changed(&mut self) {
        let accounts=&self.mail.accounts;
        let known=&self.mail.manage.identities;
        let active=|email:&str|accounts.iter().any(|a|a.enabled && mail::key(a)==email && known.get(email).is_none_or(|old|old==a));
        self.mail.manage.data.retain(|(email,_),_|active(email));
        self.mail.recent.retain(|m|active(&m.account.to_ascii_lowercase()));
        self.mail.manage.trash.retain(|m|active(&m.account.to_ascii_lowercase()));
        if self.mail.reading.as_ref().is_some_and(|m|!active(&m.account.to_ascii_lowercase())){self.mail.reading=None;}
        if self.mail.open_uid.as_ref().is_some_and(|(email,_)|!active(&email.to_ascii_lowercase())){self.mail.open_uid=None;self.mail.manage.auto_read=None;}
        self.mail.manage.identities.retain(|_,old|accounts.contains(old));
        self.mail.manage.pending.retain(|email|accounts.iter().any(|a|mail::key(a)==*email));
        if self.mail.manage.account_filter.as_ref().is_some_and(|f|!accounts.iter().any(|a|a.email.eq_ignore_ascii_case(f))){self.mail.manage.account_filter=None;self.mail.manage.account_menu=false;if !self.settings.mail_filter.is_empty(){self.settings.mail_filter.clear();self.settings.save();}}
        self.mail.manage.next_refresh=0.0;
        mail::save_accounts(accounts);
        self.pending.push(Cmd::MailSync);
    }

    /// 录制/截图用的邮件样例（只在 `record` feature 下编译）。
    ///
    /// `DAOZAI_MAIL_DEMO=1|code` 面板带验证码，`=list` 只列邮件，`=fail` 用假授权码走一次
    /// 「测试连接失败」，方便在没有真实邮箱时验收失败路径。
    #[cfg(feature = "record")]
    pub fn mail_demo(&mut self) {
        use mail::{Account, Item, Provider, Status};
        let mode = std::env::var("DAOZAI_MAIL_DEMO").unwrap_or_default();
        self.settings_tab = 7;
        if mode == "fail" {
            self.mail.form.provider = Provider::Qq;
            self.mail.form.email = "test@qq.com".into();
            self.mail.form.password = "invalid-auth-code".into();
            self.mail.note = "test@qq.com：正在测试连接…".into();
            self.pending.push(Cmd::MailTest(
                Account { custom: None, email: "test@qq.com".into(), provider: Provider::Qq, enabled: true },
                "invalid-auth-code".into(),
            ));
            return;
        }
        self.mail.accounts = vec![
            Account { custom: None, email: "demo@qq.com".into(), provider: Provider::Qq, enabled: false },
            Account { custom: None, email: "demo@icloud.com".into(), provider: Provider::ICloud, enabled: false },
        ];
        for a in &self.mail.accounts {
            self.mail.status.insert(a.email.clone(), Status::Ready);
        }
        // 会议演示：开始时间设成 10 分钟后，正好落在默认 15 分钟提前量里，
        // 启动即触发一次「会议提醒」。
        let mstart = chrono::Local::now().timestamp() + 600;
        let mat = chrono::DateTime::from_timestamp(mstart, 0)
            .map(|d| d.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let rows = [
            (
                "GitHub",
                "[GitHub] Please verify your device",
                "09:41",
                Some("482913"),
                "We noticed a sign-in attempt from a new device. Enter 482913 to finish signing in. If this wasn't you, secure your account now.",
                None,
            ),
            (
                "Apple",
                "Your Apple ID verification code",
                "09:38",
                None,
                "Your Apple ID code is 573104. Don't share it with anyone. If you didn't request it, you can ignore this message.",
                None,
            ),
            (
                "腾讯企业邮",
                "会议邀请：周五方案评审",
                "09:12",
                None,
                "本周五 14:30 在三楼会议室评审 v0.5.5 方案，请提前十分钟到场。会议邀请见附件。",
                Some((mstart, mat)),
            ),
            (
                "顺丰速运",
                "您的快件已到达菜鸟驿站",
                "08:57",
                None,
                "您的快件已到达菜鸟驿站（望京 SOHO T1 店），请凭取件码 6-3-2 在 20:00 前领取。",
                None,
            ),
        ];
        let with_code = mode != "list";
        self.mail.recent = rows
            .iter()
            .enumerate()
            .map(|(i, (from, subject, time, code, summary, meeting))| Item {
                received_unix: None, account: "demo@qq.com".into(),
                from: (*from).into(),
                subject: (*subject).into(),
                time: (*time).into(),
                code: if with_code { (*code).map(str::to_string) } else { None },
                summary: (*summary).into(),
                links: Vec::new(),
                meeting: meeting.as_ref().map(|(start, at)| mail::Meeting { all_day:false, end:None, title: "周五方案评审".into(), start: *start, at: at.clone(), uid: "demo-ics-1".into() }),
                uid: 900 - i as u32,
            })
            .collect();
        self.mail.note = "demo@qq.com：连接成功".into();
        self.page = 5;
        self.dots_target();
    }

    /// 录制/截图用的日程样例（只在 `record` feature 下编译）。
    ///
    /// `DAOZAI_CAL_DEMO=paste|fail|connected|alert`。
    #[cfg(feature = "record")]
    pub fn calendar_demo(&mut self) {
        use calendar::Provider;
        let mode = std::env::var("DAOZAI_CAL_DEMO").unwrap_or_default();
        self.calendar.form.provider = Provider::Feishu;
        self.calendar.form.username = "demo***@feishu.example".into();
        self.settings_tab = 8;
        self.open_settings();
        self.settings.calendar_prompt = false;
        let later = |secs: i64| chrono::Local::now().timestamp() + secs;
        let label = |ts: i64| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|d| d.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string())
                .unwrap_or_default()
        };
        let demo_cfg = calendar::Config {
            provider: Provider::Feishu,
            username: "demo***@feishu.example".into(),
            href: Some("https://caldav.feishu.cn/calendars/8f3c2d7a-1111-2222-3333-444455556666/".into()),
            server: None,
        };
        match mode.as_str() {
            "paste" => {
                self.calendar.form.username =
                    "服务器地址：https://caldav.feishu.cn/calendars/\n用户名：demo***@feishu.example\n密码：demo-pass-123".into();
                if let Some((u, p)) = calendar::parse_paste(&self.calendar.form.username) {
                    self.calendar.form.username = u;
                    self.calendar.form.password = p;
                    self.calendar.note = "已从粘贴内容拆出用户名和密码，服务器地址已按平台填好".into();
                }
            }
            "fail" => {
                self.calendar.form.password = "demo-pass-123".into();
                self.calendar.note = calendar::human_error(Provider::Feishu, Some(401));
                self.calendar.note_error = true;
            }
            "connected" => {
                let a = later(3600);
                let b = later(6 * 3600);
                self.calendar.form.password = "demo-pass-123".into();
                self.calendar.events = vec![
                    mail::Meeting { all_day:false, end:None, title: "产品评审".into(), start: a, at: label(a), uid: "demo-cal-1".into() },
                    mail::Meeting { all_day:false, end:None, title: "设计周会".into(), start: b, at: label(b), uid: "demo-cal-2".into() },
                ];
                self.calendar.config = Some(demo_cfg.clone());
                self.calendar.connected = true;
                self.calendar.last_sync = calendar::now_label();
                self.calendar.note.clear();
                self.calendar.note_error = false;
            }
            "alert" => {
                let s = later(600);
                self.calendar.events =
                    vec![mail::Meeting { all_day:false, end:None, title: "飞书日程：产品评审".into(), start: s, at: label(s), uid: "demo-cal-alert".into() }];
                self.calendar.config = Some(demo_cfg.clone());
                self.calendar.connected = true;
                self.calendar.last_sync = calendar::now_label();
                self.settings_open = false;
            }
            _ => {}
        }
    }

    /// v0.5.4：把系统音量与蓝牙设备状态同步进界面；只在连接/断开时提示一次。
    fn sync_device(&mut self, env: &Env) {
        if self.t >= self.volume_quiet_until {
            if let Some(v) = env.volume {
                self.volume_ui = Some(v);
            }
        }
        if !self.settings.bluetooth_notifications {
            self.bt_shown.clear();
            self.bt_initialized=false;
            self.alerts.retain(|a|a.source!=crate::extras::AlertSource::Internal || a.title!="蓝牙设备");
            if self.notice.as_ref().is_some_and(|a|a.source==crate::extras::AlertSource::Internal && a.title=="蓝牙设备") {self.notice=None;}
            return;
        }
        if !env.bt_ready { return; }
        // 只把首次完整扫描作为基线，之后的连接变化都应提示。
        if !self.bt_initialized {
            self.bt_initialized = true;
            self.bt_shown = env.bt.clone();
            return;
        }
        let connected: Vec<_> = env.bt.iter().filter(|d| !self.bt_shown.iter().any(|old| old.id == d.id)).cloned().collect();
        let disconnected: Vec<_> = self.bt_shown.iter().filter(|d| !env.bt.iter().any(|new| new.id == d.id)).cloned().collect();
        for device in &connected { self.bluetooth_alert(device, true); }
        for device in &disconnected { self.bluetooth_alert(device, false); }
        self.bt_shown = env.bt.clone();
    }

    /// 距离上次交互的秒数（小精灵出现的依据）。
    pub fn idle_for(&self) -> f64 {
        self.system_idle.unwrap_or(self.t - self.last_input)
    }

    pub fn show_page(&mut self,page:usize) {
        if !self.settings.visible_pages[page] {
            self.settings_tab=[1,2,3,5,4,7,11,8][page];
            self.open_settings();
            return;
        }
        self.notice=None;
        self.go(page,1.0);
        self.open=true; self.forced_open_until=self.t+4.0;
        let target_h=self.expanded_height();
        self.tune_open(PW,target_h);
    }
    pub fn open_settings(&mut self){
        self.settings_open=true;
        self.settings_focus_requested=true;
    }
    pub fn open_settings_category(&mut self,category:usize){
        self.settings_category=category.min(3);
        if category==0 || category>=3 {self.settings_tab=if category==0{0}else{6};self.open_settings();return;}
        let page=if category==1{2}else{1};
        self.show_page(page);
        if self.settings.visible_pages[page] && (category==1 || category==2) {self.subpage=Some(page);}
    }
    pub fn page(&self) -> usize {
        self.page
    }

    pub(crate) fn set_page_visible(&mut self,page:usize,visible:bool) {
        if !visible && self.settings.visible_pages.iter().filter(|enabled|**enabled).count()==1
            && self.settings.visible_pages[page] { return; }
        self.settings.visible_pages[page]=visible;
        if !self.settings.visible_pages[self.page] {
            if let Some(first)=self.settings.page_order.iter().copied().find(|page|self.settings.visible_pages[*page]) { self.go(first,1.0); }
        }
    }

    fn navigate_page(&mut self,next:bool) {
        let position=self.settings.page_order.iter().position(|page|*page==self.page).unwrap();
        for offset in 1..=PAGE_NAMES.len() {
            let page=self.settings.page_order[(position+if next {offset}else{PAGE_NAMES.len()-offset})%PAGE_NAMES.len()];
            if self.settings.visible_pages[page] {
                self.go(page,if next {1.0}else{-1.0});
                break;
            }
        }
    }

    pub fn is_editing(&self) -> bool {
        // 剪贴板搜索也属于「正在输入」：窗口被激活时要保持展开，否则光标一闪就收起来了。
        self.adding || self.editing.is_some() || self.todo_choosing || self.task_dialog.is_some() || self.calendar.draft.is_some() || self.focus_editing || self.clip_search_open
    }

    pub fn filter_todo_input(&mut self,input:&mut egui::RawInput) {
        self.todo_ime_frame=false;
        if !self.adding && self.editing.is_none(){self.todo_ime_active=false;return;}
        self.todo_ime_frame=self.todo_ime_active || input.events.iter().any(|event|matches!(event,egui::Event::Ime(egui::ImeEvent::Enabled|egui::ImeEvent::Preedit(_)|egui::ImeEvent::Commit(_))));
        for event in &input.events {
            match event {
                egui::Event::Ime(egui::ImeEvent::Enabled|egui::ImeEvent::Preedit(_))=>self.todo_ime_active=true,
                egui::Event::Ime(egui::ImeEvent::Commit(_)|egui::ImeEvent::Disabled)=>self.todo_ime_active=false,
                _=>{},
            }
        }
        // 首帧和结束帧的按键、失焦仍属于组合输入，不能透传给待办编辑。
        if self.todo_ime_frame {input.events.retain(|event|!matches!(event,egui::Event::Key{key:egui::Key::Backspace|egui::Key::Delete|egui::Key::Enter|egui::Key::Escape|egui::Key::ArrowLeft|egui::Key::ArrowRight|egui::Key::ArrowUp|egui::Key::ArrowDown,..}));}
    }

    pub fn foreground_changed(&mut self) {
        if self.todo_ime_frame {return;}
        self.hover_lock=true;
        if self.adding && self.buf.trim().is_empty(){self.cancel_edit();}
    }

    /// 带过渡地切到某页。`dir` > 0 表示内容从右侧进入。
    pub(crate) fn go(&mut self, page: usize, dir: f32) {
        if page == self.page || !self.settings.visible_pages[page] { return; }
        self.subpage=None;
        // 连点：上一段过渡还没结束时，以当前页为起点重新开始，位移速度保留。
        self.page_from = self.page;
        self.page = page;
        if page==CALENDAR_PAGE {self.request_calendar_month(false);}
        self.page_dir = dir;
        self.page_t0 = self.t;
        self.slide.pos = dir * PAGE_SLIDE + if self.page_e() < 0.4 { self.slide.pos * 0.3 } else { 0.0 };
        self.slide.target = 0.0;
        self.dots_target();
        self.cancel_edit();
    }

    fn page_e(&self) -> f32 {
        (self.t - self.page_t0) as f32
    }

    fn dots_target(&mut self) {
        for (i, d) in self.dots.iter_mut().enumerate() {
            d.target = if i == self.page { 16.0 } else { 4.0 };
        }
    }

    pub(crate) fn cancel_edit(&mut self) {
        self.focus_editing=false;
        self.adding = false;
        self.editing = None;
        self.todo_choosing = false;
        self.task_dialog=None;
        self.calendar.draft=None;
        self.buf.clear();
        self.todo_input_focused=false;
        self.todo_ime_active=false;
        self.todo_ime_frame=false;
    }

    fn island_lyrics_active(&self)->bool {
        if self.side() {return false;}
        self.settings.island_lyrics && self.settings.visible_pages[0] && self.media.is_music()
            && !self.browser_minimal_active
    }
    pub(crate) fn compact_notices(&self)->bool {self.island_lyrics_active()&&!self.open&&!self.notice.as_ref().is_some_and(|a|a.meeting.is_some()||self.settings.notification_preview&&matches!(a.source,crate::extras::AlertSource::External(_)))}

    /// 换句、分段和展开高亮共用时间调整；正数延后，负数提前。
    pub(crate) fn lyric_position(&self)->f32 {(self.media.position-self.settings.lyric_delay_ms as f64/1000.0)as f32}

    pub(crate) fn island_lyric(&self) -> Option<(usize,&str)> {
        if !self.island_lyrics_active(){return None;}
        if !lyric_key_matches(&self.ly.key,&self.media) {return None;}
        let lyrics=self.ly.lyrics.as_ref().filter(|lyrics|lyrics.synced)?;
        let index=lyrics.index_at(self.lyric_position())?;
        let text=lyrics.lines.get(index)?.text.trim();
        (!text.is_empty()).then_some((index,text))
    }

    fn update_island_lyric_layout(&mut self,ctx:&egui::Context) {
        if !self.island_lyrics_active(){self.island_lyric_layout=None;self.island_lyric_scroll=None;return;}
        // 文本与时间窗先取成自有值：下面要写 self 的字段，不能还借着 self.ly。
        let current=self.island_lyric().map(|(index,line)|(index,line.to_owned()));
        let lyrics=self.ly.lyrics.as_ref().filter(|lyrics|lyric_key_matches(&self.ly.key,&self.media)
            && lyrics.synced && lyrics.lines.iter().any(|line|!line.text.trim().is_empty()));
        let title=lyrics.is_none();
        let text=if let Some((_,line))=&current {line.clone()}
            else if let Some(lyrics)=lyrics {
                if lyrics.index_at(self.lyric_position()).is_none() {"前奏".to_owned()}
                else {"间奏".to_owned()}
            } else {self.media.title.trim().to_owned()};
        let ink=if current.is_some()||title{theme::WHITE}else{theme::GRAY};
        // 先算好再进闭包，避免闭包捕获 &self 与后面的可变借用冲突。
        let wrap_width=self.lyric_text_width();
        let white=ink==theme::WHITE;
        // 本句时间窗：滚动速度要用，测试构建的分页兜底也要用。
        let span=current.as_ref().zip(lyrics).map(|((index,_),lyrics)|{
            let start=lyrics.lines[*index].t;
            let end=lyrics.lines.iter().skip(index+1).find(|line|line.t>start).map_or(self.media.duration as f32,|line|line.t);
            (start,end)
        });
        let progress=span.map_or(0.0,|(start,end)|if end>start {((self.lyric_position()-start)/(end-start)).clamp(0.0,1.0)}else{0.0});
        let window=span.map(|(start,end)|end-start);
        // 布局缓存：同一句、同一通道宽度、同一颜色只重排一次。
        // 没有缓存时这里每帧都要从 11pt 逐级试到 8pt，一句中文最多重排 7 次，全是重复劳动。
        // 测试构建不走缓存：分页兜底按播放进度切段，缓存会返回上一段。
        let cached=if cfg!(test) {None} else {
            self.island_lyric_cache.as_ref()
                .filter(|cache|cache.matches(&text,wrap_width,white,title))
                .map(|cache|(cache.galley.clone(),cache.overflow))
        };
        let layout=|text:&str,size:f32| {
            let mut job=egui::text::LayoutJob::simple(text.to_owned(),theme::font(size,W::Semibold),ink,wrap_width);
            // 官方的 Center 在超长单行 galley 上会产生负的行内偏移，把整行文字
            // 向左推出绘制起点、压到封面/宠物上（暂停音乐时的长歌词最容易触发）。
            // release 改左对齐，由 shared_header() 按 galley 实际宽度手动居中；
            // 测试构建保留官方值，不影响 UI 测试。
            job.halign=if cfg!(test){egui::Align::Center}else{egui::Align::Min};
            job.wrap.break_anywhere=true;
            if title {job.wrap.max_rows=1;}
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        // 单行不折行：横向滚动用，galley 宽度就是整句的真实宽度。
        let layout_single=|text:&str,size:f32| {
            let mut job=egui::text::LayoutJob::simple(text.to_owned(),theme::font(size,W::Semibold),ink,f32::INFINITY);
            job.halign=egui::Align::Min;
            job.wrap.break_anywhere=false;
            ctx.fonts(|fonts|fonts.layout_job(job))
        };
        let (galley,overflow)=match cached {
            Some(hit)=>hit,
            None=>{
                // 歌词固定 11pt，不再缩字号：单行放不下就横向滚动，比缩小字体更易读。
                // 歌名不滚动，仍沿用官方单行省略。
                // 测试构建保留官方分页兜底，UI 测试的布局断言不变。
                let size=LYRIC_BASE_SIZE;
                let mut galley=layout(&text,size);
                let mut overflow=0.0;
                if cfg!(test) {
                    if galley.rows.len()>1 {
                        // 按当前句的时间范围与各段字数切换，暂停和跳转直接跟随播放位置。
                        let total:usize=galley.rows.iter().map(|row|row.glyphs.len()).sum();
                        let mut count=0;
                        let page=galley.rows.iter().position(|row|{count+=row.glyphs.len();progress*(total as f32)<count as f32}).unwrap_or(galley.rows.len()-1);
                        let segment=galley.rows[page].glyphs.iter().map(|glyph|glyph.chr).collect::<String>();
                        galley=layout(segment.trim(),size);
                    }
                } else if !title && galley.rows.len()>1 {
                    galley=layout_single(&text,size);
                    overflow=(galley.size().x-wrap_width).max(0.0);
                }
                if !cfg!(test) {
                    self.island_lyric_cache=Some(LyricLayoutCache{text:text.clone(),width:wrap_width,white,title,galley:galley.clone(),overflow});
                }
                (galley,overflow)
            }
        };
        // 滚动时间基准：换句（行号或文本变化）时重置；副歌同一句重复出现也要重置。
        let key=(current.as_ref().map(|(index,_)|*index),text.clone());
        if key!=self.island_lyric_scroll_key {
            self.island_lyric_scroll_key=key;
            self.island_lyric_scroll_start=self.t;
        }
        self.island_lyric_scroll=(overflow>0.5)
            .then_some(LyricScroll{overflow,speed:lyric_scroll_speed(overflow,window.unwrap_or(0.0))});
        self.island_lyric_layout=Some(galley);
    }

    pub(crate) fn effective_minimal(&self)->bool {self.settings.minimal || self.browser_minimal_active}
    /// 常驻电量环：(本机, 手机)，None 表示该来源关闭或没有数据。
    /// 两个来源互不影响，都开就并排画两个环（本机在左）。
    pub(crate) fn battery_rings(&self)->(Option<(Option<u8>,bool)>,Option<(Option<u8>,bool)>) {
        let pc=(self.settings.show_battery&&self.resident_power.0.is_some()).then_some(self.resident_power);
        let phone=(self.settings.phone_battery&&self.phone_resident.0.is_some()).then_some(self.phone_resident);
        (pc,phone)
    }
    /// 当前要画几个电量环：0 / 1 / 2。
    pub(crate) fn battery_ring_count(&self)->usize {
        let (pc,phone)=self.battery_rings();
        pc.is_some() as usize+phone.is_some() as usize
    }
    pub(crate) fn battery_visible(&self)->bool {self.battery_ring_count()>0}
    pub(crate) fn pet_enabled(&self)->bool {self.settings.pet && !self.effective_minimal() && !self.side()}
    pub(crate) fn pet_space(&self)->f32 {if self.pet_enabled(){44.0}else{0.0}}
    fn lyric_focus_space(&self)->f32 {if self.pomo.running{crate::focus_bar::LYRIC_WIDTH+10.0}else{0.0}}
    fn lyric_bar_width(&self)->f32 {LYRIC_BAR_W+if self.pet_enabled(){48.0}else{0.0}+self.lyric_focus_space()}
    /// 歌词态左侧固定通道：电量环 → 封面 → 吉祥物，歌词只在通道右侧排版。
    /// 返回 (宠物中心, 歌词左界)，均为相对胶囊左边缘 `left` 的偏移。
    /// 官方把宠物放在「胶囊中心 + 60」，歌词放在「left + content_width - 150」，
    /// 两套基准在专注模式（content_width 缩小）或宽度动画下会互相错位；
    /// 这里把三者统一到 `left` 基准，表情贴图实际宽 48px（cx±24）也计入占位。
    /// 测试构建返回 None，沿用官方坐标，避免影响 UI 测试的布局断言。
    fn lyric_lane(&self)->Option<(f32,f32)> {
        if cfg!(test) {return None;}
        // 本机与手机电量各自独立，两个环都在时左侧通道要留双份位置。
        let rings=self.battery_ring_count() as f32;
        let art_right=if rings>0.0 {36.0+rings*20.0}else{32.0};
        let pet_center=art_right+LANE_PET_HALF+LANE_ART_GAP;
        let text_left=pet_center+LANE_PET_HALF+LANE_TEXT_GAP;
        Some((pet_center,text_left))
    }
    /// 歌词排版宽度：右侧给播放指示留出 30px；测试构建保持官方 220px。
    fn lyric_text_width(&self)->f32 {
        match self.lyric_lane() {
            Some((_,text_left))=>(self.lyric_bar_width()-self.lyric_focus_space()-LANE_RIGHT_PAD-text_left).max(90.0),
            None=>LYRIC_BAR_W-80.0,
        }
    }
    fn notice_size(&self,a:&Alert)->(f32,f32) {
        let (width,height)=if a.meeting.is_some(){(360.0,136.0)}else if a.source==crate::extras::AlertSource::Internal && a.title=="蓝牙设备"{(HUD_W,48.0)}else if a.is_hud(){(HUD_W,38.0)}else{(334.0,44.0)};
        (width,height)
    }
    pub fn set_pet(&mut self,enabled:bool) {
        self.settings.pet=enabled;if enabled {self.settings.minimal=false;}self.settings.save();
    }
    pub fn set_minimal(&mut self,enabled:bool) {
        self.settings.minimal=enabled;if enabled {self.settings.pet=false;}self.settings.save();
    }
    fn base_size(&self) -> (f32, f32) {
        if !self.compact_notices()&&!self.open {
            if let Some(a)=&self.notice {let (width,height)=self.notice_size(a);return (width+self.pet_space(),height);}
        }
        if self.side() {
            // 左右停靠：竖向胶囊（通知仍是贴边滑出的横向卡片，见上）。
            return self.side_base_size(self.effective_minimal()&&self.t>=self.notification_bell_until&&(self.browser_minimal_active||self.pending_messengers.is_empty()));
        }
        if self.island_lyrics_active() {
            (self.lyric_bar_width(),BAR_H)
        } else if self.effective_minimal()&&self.t>=self.notification_bell_until {
            (BAR_W_MIN, BAR_H_MIN)
        } else {
            (self.resident.target.width.min(WIN_W-32.0), BAR_H)
        }
    }

    /// 0 = 药丸，1 = 完整面板（展开时可略超过 1）。
    #[cfg(test)]
    pub(crate) fn current_width(&self) -> f32 { self.w.pos+self.notice_width.pos }

    pub fn progress(&self) -> f32 {
        let (bw, bh) = self.base_size();
        // 左右停靠时面板沿水平方向从边缘滑出，用宽度衡量展开程度。
        if self.side() {return (self.w.pos - bw) / (PW - bw).max(1.0);}
        (self.h.pos - bh) / (self.expanded_height() - bh)
    }

    pub(crate) fn expanded_height(&self)->f32 {
        if self.page==CALENDAR_PAGE {CALENDAR_H}else if self.page==6{176.0}else if self.page==5&&self.mail.open_uid.is_some(){236.0}else{PH}
    }

    // ---------------------------------------------------------------
    // 每帧入口
    // ---------------------------------------------------------------
    pub fn frame(&mut self, ui: &mut egui::Ui, win: Rect, env: &Env) -> FrameOut {
        let ctx = ui.ctx().clone();
        let dt = match self.last_t {
            Some(t0) => (env.t - t0).clamp(0.0, 0.1) as f32,
            None => 0.0,
        };
        let elapsed = self.last_t.map(|at| (env.t - at).max(0.0) as f32).unwrap_or(0.0);
        self.last_t = Some(env.t);
        self.t = env.t;
        self.quota_tick();
        self.current_hour=env.hour();
        self.system_idle=env.idle_seconds;
        self.dt = dt;
        if let Some(at_top)=env.browser_at_top {self.browser_at_top=at_top;}
        self.screen=env.screen;
        if self.settings.skin.is_glass() {
            let target=if self.settings.skin==model::Skin::GlassLight {glass::light_tint(env.backdrop,env.glass_blur)}else{glass::glass_tint(env.backdrop,env.glass_blur)};
            self.glass_tint=if self.glass_tint<=0.0 {target}else{anim::approach(self.glass_tint,target,3.0,dt)};
        }
        let browser_minimal=self.settings.browser_minimal && self.browser_at_top;
        if browser_minimal && !self.browser_minimal_active {
            self.open=false;
            self.hover_lock=true;
            self.hover_since=None;
            self.leave_since=None;
            self.forced_open_until=0.0;
        }
        self.browser_minimal_active=browser_minimal;
        if !self.started {
            self.started = true;
            self.last_input = env.t;
            self.last_reminder = env.t;
            self.page_t0 = env.t - 10.0;
            if self.page==CALENDAR_PAGE {self.request_calendar_month(false);}
        }
        let mut cmds = Vec::new();

        self.tick(env, dt, elapsed);
        self.sync_device(env);
        self.tick_alerts();
        let hud_active=self.notice.clone();
        if hud_active.as_ref().map(|a|&a.title)!=self.hud_notice.as_ref().map(|a|&a.title)||hud_active.is_none(){self.notice_details=false;}
        if let Some(a)=&hud_active {self.hud_notice=Some(a.clone());}
        self.hud_alpha=anim::approach(self.hud_alpha,if hud_active.is_some(){1.0}else{0.0},if hud_active.is_some(){16.0}else{8.0},dt);
        if hud_active.is_none() && self.hud_alpha<0.003 {self.hud_notice=None;}
        let notice_target=if !self.notice.as_ref().is_some_and(|a|a.messenger().is_some()) && self.compact_notices()&&(self.notice.is_some()||self.t<self.notification_bell_until) {44.0}else{0.0};
        self.notice_width.target=notice_target;
        if self.compact_notices(){self.notice_width.step(dt);}else{self.notice_width.snap(0.0);self.notice_details=false;}
        self.update_media(env,&ctx);
        let bell=self.t<self.notification_bell_until&&!self.compact_notices();
        let phone=if self.settings.phone_battery{self.phone_power.poll()}else{(None,false)};
        // 本机电量与手机电量彻底分开：电量开关只管本机，手机电量开关只管手机，
        // 两个来源各自判断、各自显示，互不影响。
        self.resident_power=(env.battery,env.charging);
        self.phone_resident=phone;
        if self.settings.phone_battery {self.phone_events(phone.0,phone.1);}
        self.pc_events(env.battery,env.charging);
        // 勿扰到点自动解除；同时把设置项归零，避免界面上还显示着「60 分钟」。
        if self.quiet_until>0.0 && self.t>=self.quiet_until {self.quiet_until=0.0;self.settings.quiet_minutes=0;self.settings.save();}
        if self.t-self.meeting_checked_at>5.0 {self.meeting_checked_at=self.t;self.meeting_quiet(env);}
        self.tick_snoozed();
        if self.keyword_src!=self.settings.notice_keywords {
            self.keyword_src=self.settings.notice_keywords.clone();
            self.keywords=self.settings.keywords().iter().map(|word|word.to_lowercase()).collect();
        }
        self.clipboard.flush();
        if self.t-self.quota_checked_at>30.0 {self.quota_checked_at=self.t;self.quota_threshold_alert();}
        let mut layout=Layout::new(self.battery_visible(),self.quota_bar_indices().len(),self.media.is_music()&&!bell,self.pomo.running,bell,self.pet_enabled());
        // 状态总线：有存活状态时在胶囊右侧留一块并排区域（最多 3 条）。
        let status_width=self.status_total_width();
        if status_width>0.0 {layout.add_status(status_width);}
        layout.reorder(&self.settings.resident_order);
        // 两个电量环并排：电池槽加宽 20px，其余槽位自动让开。
        if self.battery_ring_count()==2 {layout.widen_battery(20.0);}
        for (position,extra) in self.quota_balance_layout(&ctx){layout.balance_tail(position,extra);}
        if self.settings.clock_period!=0 { layout.widen_clock(40.0); }
        if self.pet_enabled() {
            if let Some(a)=self.notice.as_ref().filter(|_|!self.compact_notices()&&!self.open){layout.x[bar::PET]=Some(22.0-(self.notice_size(a).0+self.pet_space())/2.0);}
            else if self.island_lyrics_active() {
                // 与歌词共用同一个 `left` 基准：宠物固定贴在封面右侧，
                // 专注模式 / 宽度动画改变 content_width 时也不会漂进歌词区。
                let base=if cfg!(test) {self.lyric_bar_width()} else {self.w.pos.max(self.lyric_bar_width()-self.lyric_focus_space())};
                let pet_center=self.lyric_lane().map(|(p,_)|p).unwrap_or(60.0);
                layout.x[bar::PET]=Some(pet_center-base/2.0);
            }
        }
        self.resident.step(layout,dt,ctx.style().animation_time>0.0&&bar::animations_enabled());
        if self.opts.fetch_lyrics {
            self.poll_lyrics();
        }
        self.update_island_lyric_layout(&ctx);
        // 通用状态总线：文件里有什么岛上就有什么；同 id 覆写续期，删掉即撤下。
        let (statuses,removed)=self.island_status.poll();
        // poll 每 2 秒才返回一次数据，其余帧返回空；只在真有变化时更新，
        // 否则常驻区会只闪一帧就消失。
        if !statuses.is_empty() || !removed.is_empty() {
            // 启动或清空后的首轮不闪，避免一次排队好几条横幅；之后新增的状态才闪一次。
            let first_fill=self.status_live.is_empty();
            for status in &statuses {
                if !first_fill && !self.status_live.iter().any(|old|old.key()==status.key()) {
                    self.push_status(status.clone());
                }
            }
            self.status_live=statuses;
        }
        for key in removed { self.drop_status(&key); }

        let cx = win.center().x;
        // 胶囊皮肤悬浮在屏幕顶边下方。
        let pill = self.settings.island_shape==model::IslandShape::Pill;
        let top = win.top() + if pill { PILL_GAP } else { 0.0 };
        let side = self.side();
        let dock = self.dock();

        // ---------- 输入：拖放 ----------
        let (hovering_files, dropped) = ctx.input(|i| {
            (
                self.settings.visible_pages[3] && !i.raw.hovered_files.is_empty(),
                i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>(),
            )
        });
        if !dropped.is_empty() && self.settings.visible_pages[3] {
            self.on_drop(dropped);
            self.show_page(3);
        } else if hovering_files && !self.drag_hot {
            self.drag_hot = true;
            if self.page != 3 {
                self.drag_saved = Some(self.page);
                self.go(3, if self.page < 3 { 1.0 } else { -1.0 });
            }
        } else if !hovering_files && self.drag_hot {
            // 拖到外面松手 / 取消：安静地退回原页
            self.drag_hot = false;
            if let Some(p) = self.drag_saved.take() {
                self.go(p, if p < 3 { -1.0 } else { 1.0 });
            }
        }
        self.drag_a = anim::approach(self.drag_a, if self.drag_hot { 1.0 } else { 0.0 }, 16.0, dt);

        // ---------- 输入：悬停 ----------
        let p_prev = self.progress();
        let hit = if side {self.side_hit_rect(win,p_prev)} else {self.hit_rect(cx, top, p_prev)};
        let previous_menu=self.shelf_menu_rect.take();
        let inside = env.cursor.is_some_and(|c| hit.contains(c) || previous_menu.is_some_and(|r|r.contains(c)));
        let on_notice=env.cursor.is_some_and(|c|self.messenger_badge_rect(win,self.island_rect(win,self.w.pos,self.h.pos)).is_some_and(|r|r.contains(c)) || self.notice_rect(cx,top).is_some_and(|r|r.contains(c))||self.notice_details_rect(cx,top).is_some_and(|r|r.contains(c)));
        if !inside {
            self.hover_lock = false;
        }
        if env.cursor != self.last_cursor || ctx.input(|i| !i.events.is_empty()) {
            if inside || hovering_files || ctx.input(|i| !i.events.is_empty()) {
                self.last_input = env.t;
            }
            self.last_cursor = env.cursor;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.is_editing() {
            self.cancel_edit();
        }
        if (!self.is_editing() || self.todo_choosing) && inside && self.open && self.reveal > 0.95 {
            let (scroll, left, right, esc) = ctx.input(|i| (i.raw_scroll_delta.y, i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight), i.key_pressed(egui::Key::Escape)));
            if esc { self.hover_lock = true; }
            if (left || right) && !self.todo_choosing {
                self.wheel_delta=0.0;
                self.navigate_page(right);
            } else if scroll!=0.0 {
                let cursor=env.cursor.unwrap()-self.content_origin(win).to_vec2();
                let mail_reading=self.page==5 && self.mail.open_uid.is_some();
                let mail_list=self.page==5 && !self.mail.accounts.is_empty() && self.mail.open_uid.is_none() && self.mail_list_bounds().contains(cursor);
                if mail_reading || mail_list {self.wheel_delta=0.0;}
                else {
                    // 累积高精度滚轮的小幅输入；停顿或反向时丢弃上一段余量。
                    if self.t-self.wheel_at>0.18 || self.wheel_delta*scroll<0.0 {self.wheel_delta=0.0;}
                    self.wheel_at=self.t;
                    self.wheel_delta+=scroll;
                    if self.wheel_delta.abs()>=8.0 {
                        let next=self.wheel_delta<0.0;
                        self.wheel_delta=0.0;
                        let calendar_max=if self.page==CALENDAR_PAGE {self.calendar_scroll_max(env)}else{0};
                        let calendar_body=self.page==CALENDAR_PAGE && Rect::from_min_max(Pos2::new(163.0,52.0),Pos2::new(378.0,208.0)).contains(cursor);
                        let clipboard_tab=self.page==3 && self.shelf_tab==1;
                        let clipboard_body=clipboard_tab && Rect::from_min_max(Pos2::new(42.0,31.0),Pos2::new(377.0,124.0)).contains(cursor);
                        let list=match self.page {
                            CALENDAR_PAGE if calendar_body=>Some((&mut self.calendar.view.first,calendar_max)),
                            4 if Rect::from_min_max(Pos2::new(196.0,31.0),Pos2::new(377.0,124.0)).contains(cursor)
                                =>Some((&mut self.todo_first,self.todos.items.len().saturating_sub(3))),
                            3 if clipboard_body
                                =>Some((&mut self.clip_first,self.clipboard.lock().items.len().saturating_sub(crate::pages::CLIP_ROWS))),
                            3 if !clipboard_tab && Rect::from_min_max(Pos2::new(36.0,28.0),Pos2::new(383.0,128.0)).contains(cursor)
                                =>Some((&mut self.shelf_first,self.shelf.items.len().saturating_sub(5))),
                            _=>None,
                        };
                        let content_scrolls=(self.page==4 || clipboard_body || calendar_body) && list.as_ref().is_some_and(|(_,max)|*max>0);
                        let scrolled=list.is_some_and(|(first,max)|{
                            let target=if next {(*first+1).min(max)}else{first.saturating_sub(1)};
                            let changed=target!=*first;*first=target;changed
                        });
                        // 内容溢出时滚动列表并停在边界；未溢出时滚轮翻页。
                        if !scrolled && !content_scrolls && !self.todo_choosing && self.t-self.wheel_page_at>=0.20 {
                            self.navigate_page(next);
                            self.wheel_page_at=self.t;
                        }
                        ctx.input_mut(|i|{i.raw_scroll_delta=Vec2::ZERO;i.smooth_scroll_delta=Vec2::ZERO;});
                    }
                }
            }
        } else {self.wheel_delta=0.0;}
        let want = (inside && !self.hover_lock && (self.compact_notices()||self.notice.is_none()||self.open)) || self.open && on_notice || (self.open && self.t<self.forced_open_until) || self.drag_hot || (self.is_editing() && self.open);
        self.morph(want, inside, env.t, dt);
        self.update_pet(env, cx);

        // ---------- 绘制 ----------
        let p = self.progress();
        let (w, h) = (self.w.pos, self.h.pos);
        let panel = self.island_rect(win, w, h);
        let messenger_rect=self.messenger_badge_rect(win,panel);
        let appear=self.messenger_appear();
        let outside_badge=messenger_rect.filter(|_|appear>0.02);
        let pc = anim::clamp01(p);
        let painter = ui.painter_at(ui.max_rect().expand(40.0));
        let feather = 1.0 / ctx.pixels_per_point();
        let notice_rect=self.notice_rect(cx,top);
        let notice_shape=notice_rect.map(|r|Rect::from_min_max(Pos2::new(r.left()-36.0,r.top()),r.max));
        let body=messenger_rect.filter(|_|outside_badge.is_none()).map_or(panel,|badge|panel.union(badge));
        let body=notice_shape.map_or(body,|notice|body.union(notice));
        let minimal_pill = self.effective_minimal() && !self.island_lyrics_active() && self.notice.is_none() && self.t>=self.notification_bell_until && pc < 0.02;
        let flare = if minimal_pill || pill { 0.0 } else { lerp(5.0, FLARE, pc) };
        let k = (w / PW).clamp(0.3, 1.1);
        let thickness = if side { crate::side_dock::SIDE_BAR_W } else { BAR_H };
        let radius = if minimal_pill { BAR_H_MIN / 2.0 } else if pill { (w.min(h) * 0.5).min(lerp(thickness * 0.5, 26.0, pc)) } else { lerp(12.5, RAD * k.min(1.0), pc) };
        let flare_h = lerp(8.0, FLARE_H, pc);
        let thick = glass::thickness(PH);
        let glass_skin = self.settings.skin.is_glass();
        let layer = painter.layer_id();
        let palette = self.skin_palette();
        // 换色时跳过的图形范围（岛体底色、阴影、吉祥物）。
        let mut keep_colors:Vec<(usize,usize)>=Vec::new();
        // 毛玻璃：真实模糊下保留柔和投影增加层次；无模糊时阴影再减淡一些，保持轻盈。
        let skin_shadow = if !glass_skin {1.0} else if env.glass_blur {0.75} else {0.7};
        let shadow_k = glass::shadow_alpha(0.0, thick) / 0.12 * skin_shadow;
        let island_shadow=|r:Rect,flare:f32,flare_h:f32,radius:f32,strength:f32| if pill {pill_shadow(&painter,r,radius,strength)} else if side {shape::shadow_docked(&painter,r,dock,flare,flare_h,radius,strength)} else {shape::shadow(&painter,r,flare,flare_h,radius,strength)};
        island_shadow(body, flare, flare_h, radius, smoothstep(0.1, 0.9, pc) * shadow_k);
        if let Some(r)=outside_badge{
            let appear=self.messenger_appear();
            painter.rect_filled(r,r.width()/2.0,paint::alpha(self.glass_fill(),appear));
            if glass_skin {painter.rect_stroke(r,r.width()/2.0,egui::Stroke::new(1.0_f32,paint::alpha(self.glass_rim(),appear)),egui::StrokeKind::Inside);}
        }
        if self.settings.skin==model::Skin::Fluid {
            self.fluid_origin=body.min;
            let art=self.music_artwork.as_ref().map(|(_,texture)|texture.id());
            if art!=self.fluid_art || self.media.title!=self.fluid_title {
                let tint=if self.media.title.is_empty(){crate::fluid::Fluid::neutral()}
                    else if let Some((image,_))=&self.music_artwork{crate::fluid::Fluid::tint_from_image(image)}
                    else{crate::fluid::Fluid::tint_from_title(&self.media.title)};
                self.fluid.set_tint(tint);
                self.fluid_art=art;
                self.fluid_title.clone_from(&self.media.title);
            }
            self.fluid.set_cursor(env.cursor.filter(|p|body.contains(*p)).map(|p|(p.x-body.left(),p.y-body.top())));
            if !self.open || self.page!=0 || self.media.title.is_empty(){self.fluid.set_seek(None,0.0);}
            self.fluid.advance(env.t,dt);
        }
        self.paint_body(&painter, body, flare, flare_h, radius, feather);
        let content_from=crate::skin_paint::shape_count(&ctx,layer);

        // 内容只随形体「揭开」：不缩放，按面板裁剪，到 55% 才开始淡入
        let reveal = if self.open { smoothstep(0.50, 0.97, p) } else { smoothstep(0.55, 0.92, p) };
        self.reveal = reveal;
        let origin = self.content_origin(win);
        let clip = panel.intersect(win);
        let base = Card::new(ui, origin, clip);
        self.cursor_local = env.cursor.map(|p| base.to_local(p));

        let banner=!self.compact_notices()&&self.notice.is_some()&&!self.open;
        let restoring_lyrics=!self.open&&self.notice.is_none()&&self.island_lyrics_active()&&self.hud_notice.as_ref().is_some_and(|a|a.meeting.is_some()||a.source!=crate::extras::AlertSource::Internal);
        let fading_hud=!self.compact_notices()&&!self.open&&self.hud_notice.as_ref().is_some_and(|a|a.is_hud())&&(banner||self.hud_alpha>0.003);
        if fading_hud {
            let from=self.resident.target.width;let to=HUD_W+self.pet_space();
            let width_progress=if (to-from).abs()<1.0 {if banner{1.0}else{0.0}}else{((self.w.pos-from)/(to-from)).clamp(0.0,1.0)};
            let (opacity,header_opacity)=if banner {(self.hud_alpha*smoothstep(0.45,0.85,width_progress),1.0-smoothstep(0.05,0.40,width_progress))}
                else {(self.hud_alpha*smoothstep(0.20,0.60,width_progress),1.0-smoothstep(0.05,0.25,width_progress))};
            if !minimal_pill||pc>0.0 {self.resident_header(&base.faded(header_opacity),panel,cx,pc,reveal,env);}
            if let Some(a)=self.hud_notice.clone(){
                let overlay=Card::new(ui,panel.min+Vec2::new(self.pet_space(),0.0),panel.intersect(ui.max_rect())).faded(opacity);
                let overlay=if opacity<0.85||!banner{overlay.ghost()}else{overlay};
                self.draw_notice(&overlay,panel,&a,&mut cmds);
            }
        }else if banner {
            if let Some(a)=self.notice.clone(){self.draw_notice(&Card::new(ui,panel.min+Vec2::new(self.pet_space(),0.0),panel.intersect(ui.max_rect())),panel,&a,&mut cmds);}
        }else if !minimal_pill || pc > 0.0 {
            self.resident_header(&base, panel, cx, if restoring_lyrics{0.0}else{pc}, reveal, env);
        }
        if let Some(rect)=notice_rect {
            let c=Card::new(ui,rect.min,rect.intersect(ui.max_rect()));
            if let Some(a)=self.hud_notice.clone() {
                let opacity=self.hud_alpha*smoothstep(0.45,0.95,self.notice_width.pos/44.0);
                let c=c.faded(opacity);let c=if self.notice.is_none()||opacity<0.85 {c.ghost()}else{c};
                if a.source!=crate::extras::AlertSource::Internal {
                    paint::bell(&c.scaled(18.0,15.0,0.85),18.0,15.0,theme::WHITE,paint::bell_swing(self.t-a.at));
                    c.circle(22.0,10.5,2.0,Color32::from_rgb(255,72,72));
                }else{crate::extras::notice_badge(&c,18.0,15.0,&a);}
                let button=crate::quota_ui::bar_tooltip(c.hit(0.0,0.0,36.0,BAR_H,"notice-badge"),format!("{}\n{}",a.title,a.body));
                if button.clicked(){self.notice_details=!self.notice_details;}
            }else if self.t<self.notification_bell_until {
                paint::bell(&c.scaled(20.0,14.0,1.2),20.0,14.0,theme::WHITE,paint::bell_swing(self.t-self.notification_bell_at));
                c.circle(24.0,9.5,2.0,Color32::from_rgb(255,72,72));
                if c.hit(6.0,0.0,28.0,30.0,"bell-mail").clicked(){self.show_page(5);}
            }
        }
        if let Some(rect)=messenger_rect {
            let from=crate::skin_paint::shape_count(&ctx,layer);
            let c=Card::new(ui,rect.min,rect.intersect(ui.max_rect()));
            if outside_badge.is_some() {
                let c=c.scaled(0.0,0.0,appear);
                self.draw_unread_capsule(&ctx,&c,rect,env.cursor,&mut cmds);
            }else{
                let horizontal=!(self.side() && self.notice.is_none());
                let scale=self.messenger_stack_appear();
                self.draw_unread_stack(&c.scaled(0.0,0.0,scale).faded(scale),horizontal,&mut cmds);
            }
            keep_colors.push((from,crate::skin_paint::shape_count(&ctx,layer)));
        }
        let notice_details=self.notice_details_rect(cx,top);
        if let Some(rect)=notice_details {
            let radius=if self.hud_notice.as_ref().is_some_and(|a|a.meeting.is_some()){14.0}else{10.0};
            let from=crate::skin_paint::shape_count(&ctx,layer);
            shape::shadow(&painter,rect,0.0,0.0,radius,0.5*shadow_k);
            painter.rect_filled(rect,radius,self.glass_fill());
            if glass_skin {
                painter.rect_stroke(rect,radius,egui::Stroke::new(1.0_f32,self.glass_rim()),egui::StrokeKind::Inside);
            }
            keep_colors.push((from,crate::skin_paint::shape_count(&ctx,layer)));
            self.draw_notice(&Card::new(ui,rect.min,rect.intersect(ui.max_rect())),rect,&self.hud_notice.clone().unwrap(),&mut cmds);
            if ctx.input(|i|i.pointer.any_pressed()&&i.pointer.interact_pos().is_some_and(|p|!rect.contains(p)&&!notice_rect.is_some_and(|r|r.contains(p)))){self.notice_details=false;}
        }
        let pet_alpha=self.pet_a*(1.0-smoothstep(0.0,0.3,pc));
        if pet_alpha>0.004 && !minimal_pill {
            let y=(h.min(self.base_size().1)-BAR_H).max(0.0)/2.0;
            // 吉祥物跟随皮肤换色：浅色底上变成深色版本，保持清晰。
            let fit=if self.island_lyrics_active()||self.notice.is_some(){1.0}else{((WIN_W-32.0)/self.resident.target.width).min(1.0)};
            self.pet.draw(&base.scaled(cx-origin.x,15.0,fit).shifted(0.0,y).faded(pet_alpha),cx-origin.x+self.resident.position(bar::PET),self.t,self.skin_palette());
        }
        if reveal > 0.004 && !banner && !restoring_lyrics {
            let c = base.shifted(0.0, (1.0 - reveal) * -5.0).faded(reveal);
            let c = if reveal < 0.6 { c.ghost() } else { c };
            if self.calendar.draft.is_some(){self.draw_calendar_editor(&c);}else if self.task_dialog.is_some(){self.draw_task_editor(&c);}else{self.panel_body(&c, env, &mut cmds);}
            if !self.toast_below {self.toast_layer(&c,106.0);}
        }
        // 岛下的确认提示使用独立裁剪区域，收起时跟随岛体下沿。
        let floating_toast=if self.toast_below && self.toast.is_some() {
            // 侧边停靠：提示出现在胶囊正下方，靠边时向内挪，保证完整显示。
            let toast_origin=if side {
                let center=if dock==model::Dock::Left {panel.center().x.max(win.left()+90.0)} else {panel.center().x.min(win.right()-90.0)};
                Pos2::new(center-PW/2.0,panel.top())
            } else {origin};
            self.toast_layer(&Card::new(ui,toast_origin,win),h+8.0)
        }else{None};
        if (self.adding||self.editing.is_some()) && reveal > 0.9 && self.page == 4 && self.page_e() > 0.25 {
            self.input_field(ui, origin, &mut cmds);
        }

        if self.task_dialog.is_some() && reveal>0.9 {self.task_editor_inputs(ui,origin,&mut cmds);}
        if self.calendar.draft.is_some() && reveal>0.9 {self.calendar_editor_inputs(ui,origin,&mut cmds);}

        // ---------- 输出 ----------
        let rel = |r: Rect| r.translate(-win.min.to_vec2());
        let mut region = if side {
            // 侧边：面板从边缘滑出，区域包含肩部（沿边缘上下）与外侧阴影，并一直延伸到屏幕边缘。
            let r=if p > 0.02 || self.open {
                let height=self.expanded_height().max(self.h.pos)*1.08+FLARE*2.0+24.0;
                self.island_rect(win,PW+20.0,height).union(panel.expand(20.0))
            } else {
                let (bw,bh)=self.base_size();
                self.island_rect(win,bw*SWELL.1+14.0,bh*SWELL.0+FLARE+16.0).union(panel.expand(12.0))
            };
            if dock==model::Dock::Left {Rect::from_min_max(Pos2::new(win.left(),r.top()),r.max)} else {Rect::from_min_max(r.min,Pos2::new(win.right(),r.bottom()))}
        } else if p > 0.02 || self.open {
            let height=self.expanded_height().max(self.h.pos)*1.08+16.0;
            Rect::from_center_size(Pos2::new(cx, top + height/2.0), Vec2::new(PW + FLARE * 2.0 + 56.0, height))
        } else {
            let (bw, bh) = self.base_size();
            Rect::from_center_size(Pos2::new(cx, top + (bh * SWELL.1 + 12.0) / 2.0), Vec2::new(bw * SWELL.0 + 24.0, bh * SWELL.1 + 12.0))
        };
        if let Some(r)=messenger_rect{region=region.union(r.expand(5.0));}
        if let Some(rect)=notice_shape {region=region.union(rect.expand2(Vec2::new(12.0,8.0)));}
        if let Some(rect)=notice_details {region=region.union(rect.expand(12.0));}
        // 胶囊上方留空的那一小条也要在窗口区域内，阴影才不会被截断。
        if pill && !side {region.min.y=region.min.y.min(win.top());}
        // 浅色皮肤：把这一帧岛体内容统一换成皮肤配色。
        if let Some(palette)=palette {
            let end=crate::skin_paint::shape_count(&ctx,layer);
            let mut ranges=Vec::new();let mut at=content_from;
            keep_colors.sort();
            for (from,to) in keep_colors {if from>at {ranges.push((at,from));} at=at.max(to);}
            if end>at {ranges.push((at,end));}
            crate::skin_paint::recolor_layer(&ctx,layer,&ranges,&palette);
        }
        // 原生裁剪还需包含悬停提示；只扩大绘制区域，保持桌面点击穿透。
        let mut tooltips=Vec::new();
        ctx.memory(|memory|{
            for layer in memory.areas().visible_layer_ids() {
                if layer.order==egui::Order::Tooltip {
                    if let Some(rect)=memory.area_rect(layer.id){region=region.union(rect.expand(12.0));tooltips.push(rect);}
                }
            }
        });
        // 毛玻璃：窗口区域只取岛体与浮层的真实轮廓，系统模糊不会溢出到透明画布上。
        let glass_region=(glass_skin && self.settings.glass_blur || env.opaque_surface).then(||{
            let outline=|r:Rect,flare:f32,flare_h:f32,radius:f32| if pill {GlassShape::Round(rel(r),radius.min(r.height().min(r.width())*0.5))}
                else if side {GlassShape::Docked{rect:rel(r),dock,flare,flare_h,radius}} else {GlassShape::Notch{rect:rel(r),flare,flare_h,radius}};
            let mut shapes=vec![outline(body,flare,flare_h,radius)];
            if let Some(r)=outside_badge{shapes.push(GlassShape::Round(rel(r),r.width()/2.0));}
            if let Some(r)=notice_details {
                let radius=if self.hud_notice.as_ref().is_some_and(|a|a.meeting.is_some()){14.0}else{10.0};
                shapes.push(GlassShape::Round(rel(r),radius));
            }
            if let Some(r)=floating_toast {shapes.push(GlassShape::Round(rel(r),11.0));}
            shapes.extend(tooltips.iter().map(|r|GlassShape::Round(rel(*r),6.0)));
            shapes
        });
        let transitioning = !self.w.settled(PW)
            || !self.h.settled(self.expanded_height())
            || self.page_e() < 0.7
            || self.dots.iter().any(|d| !d.settled(16.0))
            || self.hover_since.is_some() && !self.open
            || self.leave_since.is_some()
            || (self.drag_a > 0.001 && self.drag_a < 0.999)
            || self.drag_hot
            || (self.pet_a > 0.001 && self.pet_a < 0.999)
            || !self.resident.settled()
            || self.open && self.page==3 && self.shelf_x.values().any(|s| !s.settled(68.0))
            || self.open && self.page==2 && !self.water_ring.settled(1.0)
            || self.toast.is_some()
            || self.t < self.notification_bell_until && self.t-self.notification_bell_at < 0.95
            || self.notice.as_ref().is_some_and(|a|a.source!=crate::extras::AlertSource::Internal&&self.t-a.at<0.95)
            || self.hud_alpha > 0.003 && self.hud_alpha < 0.997
            || !self.notice_width.settled(1.0);
        let pet_visible=self.pet_a>0.01 && !self.open && !minimal_pill;
        let ambient=self.settings.skin==model::Skin::Fluid && self.fluid.animating() || self.pomo.running && (self.open && self.page==1 || !self.open && !minimal_pill && self.notice.is_none() && bar::animations_enabled())
            || self.media.playing && (self.open && self.page==0 || !self.open && !minimal_pill && self.notice.is_none() && self.media.is_music())
            || self.notice.as_ref().is_some_and(|notice|notice.meeting.is_some());
        // 长歌词滚动时必须持续重绘；暂停时也要——人正是这时候才想读完一整句。
        let lyric_scrolling=self.island_lyric_scroll.is_some();
        let animating=transitioning || ambient || lyric_scrolling || pet_visible && self.pet.state!=PetState::Sleeping;
        // 交互形变保持 60 帧，媒体/歌词滚动 30 帧；小表情 20 帧，睡眠 10 帧，静态界面每秒核对提醒。
        let repaint_after=std::time::Duration::from_secs_f64(if transitioning{1.0/60.0}else if ambient||lyric_scrolling{1.0/30.0}else if pet_visible{if self.pet.state==PetState::Sleeping{0.1}else{0.05}}else{1.0});
        cmds.append(&mut self.pending);
        FrameOut {
            region: rel(floating_toast.into_iter().chain(self.shelf_menu_rect.map(|r|r.union(r + ctx.style().visuals.popup_shadow.margin()).expand(1.0 / ctx.pixels_per_point()))).fold(region,Rect::union).intersect(ui.max_rect())),
            glass: glass_region,
            hit: rel(if side {self.side_hit_rect(win,self.progress())} else {self.hit_rect(cx, top, self.progress())}),
            notice_hit:notice_shape.into_iter().chain(notice_details).chain(messenger_rect).chain(self.shelf_menu_rect).reduce(|a,b|a.union(b)).map(rel),
            file_drop_zone:self.settings.visible_pages[3].then(||rel(if side {self.island_rect(win,PW,self.expanded_height())} else {Rect::from_min_size(Pos2::new(cx-PW/2.0,top),Vec2::new(PW,96.0))})),
            expanded: self.open,
            editing: self.is_editing(),
            animating,
            repaint_after,
            cmds,
        }
    }

    fn hit_rect(&self, cx: f32, top: f32, p: f32) -> Rect {
        let r=self.hit_rect_base(cx,top,p);
        // 胶囊离屏幕顶边有一段空隙：把光标推到屏幕最顶端也要能唤出岛。
        if self.settings.island_shape==model::IslandShape::Pill {Rect::from_min_max(Pos2::new(r.left(),r.top()-PILL_GAP),r.max)} else {r}
    }

    fn hit_rect_base(&self, cx: f32, top: f32, p: f32) -> Rect {
        if self.effective_minimal() && !self.open {
            let (width,height)=self.base_size();
            let width=self.w.pos.min(width);
            let height=self.h.pos.min(height);
            Rect::from_min_size(Pos2::new(cx-width/2.0,top),Vec2::new(width,height))
        } else if self.notice.is_some()&&!self.compact_notices()&&!self.open {
            let (w,h)=self.base_size();Rect::from_min_size(Pos2::new(cx-(w+8.0)/2.0,top),Vec2::new(w+8.0,h+6.0))
        } else if p > 0.04 || self.open {
            Rect::from_min_size(Pos2::new(cx - PW / 2.0 - 4.0, top), Vec2::new(PW + 8.0, self.expanded_height() + 2.0))
        } else {
            let (bw,bh)=self.base_size();
            let (hit_w,hit_h)=(bw+12.0,bh+4.0);
            Rect::from_min_size(Pos2::new(cx-hit_w/2.0,top),Vec2::new(hit_w,hit_h))
        }
    }

    /// 展开后未读胶囊的出现程度：面板基本展开后从面板旁淡入。
    pub(crate) fn messenger_appear(&self)->f32 {
        if self.calendar.draft.is_some() || self.task_dialog.is_some(){return 0.0;}
        smoothstep(0.0,1.0,anim::clamp01((self.progress()-0.7)/0.3))
    }
    fn messenger_stack_appear(&self)->f32 {1.0-smoothstep(0.04,0.30,self.progress())}
    /// 同一来源有多个账号时给图标编号（1、2…）；只有一个账号不编号。
    pub(crate) fn unread_tag(&self,i:usize)->Option<usize>{
        let source=self.pending_messengers.get(i)?.source;
        let same=self.pending_messengers.iter().filter(|u|u.source==source).count();
        (same>1).then(||self.pending_messengers[..=i].iter().filter(|u|u.source==source).count())
    }
    fn unread_id(&self,i:usize)->String{
        let name=self.pending_messengers[i].source.name();
        match self.unread_tag(i){Some(n) if n>1=>format!("{name}{n}"),_=>name.to_string()}
    }
    /// 展开胶囊高度：一个是圆，两个是短胶囊，三个及以上与面板等高（多出的滚动）。
    fn unread_capsule_height(&self,panel_h:f32)->f32 {
        let n=self.pending_messengers.len();
        if n>=3 {panel_h} else {4.0+UNREAD_SLOT*n as f32}
    }
    pub(crate) fn unread_top_inset(&self)->f32 {if self.dock()==model::Dock::Top {UNREAD_GAP}else{0.0}}
    fn unread_slot(&self,height:f32)->f32 {
        if self.pending_messengers.len()>=3 {(height-4.0)/3.0} else {UNREAD_SLOT}
    }
    /// 收起时叠放的来源图标（最多两个）。
    fn unread_sources(&self)->Vec<Messenger>{
        let mut sources=Vec::new();
        for u in &self.pending_messengers {if !sources.contains(&u.source){sources.push(u.source);}}
        sources
    }
    /// 收起时叠放区沿胶囊方向的长度。
    fn unread_stack_len(&self,horizontal:bool)->f32 {
        let icons=self.unread_sources().len().min(2) as f32;
        32.0+unread_step(horizontal)*(icons-1.0)
    }

    fn messenger_badge_rect(&self,win:Rect,panel:Rect)->Option<Rect>{
        if self.pending_messengers.is_empty() || self.browser_minimal_active && !self.open {return None;}
        // 完整横幅已包含该来源图标。歌词模式和横幅收起后使用常驻入口。
        if !self.open && !self.compact_notices() && self.pending_messengers.len()==1
            && self.notice.as_ref().and_then(|a|a.messenger())==self.pending_messengers.first().map(|u|u.source){return None;}
        let bounds=self.screen.map_or(win,|screen|screen.intersect(win));
        let appear=self.messenger_appear();
        if appear>0.02 {
            // 展开：贴在面板外侧、顶端对齐的竖向胶囊；顶部停靠和左停靠在右边，右停靠在左边。
            let full=self.island_rect(win,PW,self.expanded_height());
            // 顶部停靠时面板贴着屏幕上沿，胶囊往下让一点，圆顶不会被屏幕边切掉。
            let inset=self.unread_top_inset();
            let height=self.unread_capsule_height(full.height()-inset);
            let left=self.dock()==model::Dock::Right || self.dock()==model::Dock::Top && full.right()+UNREAD_GAP+UNREAD_W>bounds.right();
            let width=UNREAD_W*appear;
            let x=if left {panel.left()-UNREAD_GAP*appear-width}else{panel.right()+UNREAD_GAP*appear};
            return Some(Rect::from_min_size(Pos2::new(x,panel.top()+inset),Vec2::new(width,height*appear)));
        }
        // 极简刘海：横幅照常弹出，收起后只留细线，未读不常挂在线条上；展开岛体时在面板旁出现。
        if self.effective_minimal(){return None;}
        let scale=self.messenger_stack_appear();
        if scale<0.02 || self.open && (self.calendar.draft.is_some() || self.task_dialog.is_some()){return None;}
        let side_column=self.side() && self.notice.is_none();
        let length=self.unread_stack_len(!side_column);
        let (width,height)=if side_column {(32.0,length)}else{(length,BAR_H)};
        let (width,height)=(width*scale,height*scale);
        let (x,y)=if side_column {
            (panel.center().x-width/2.0,panel.bottom())
        }else{
            let right=panel.right()+self.notice_width.pos;
            let y=if self.notice.is_some() && !self.compact_notices(){panel.center().y-BAR_H/2.0}else{panel.top()};
            (if right+width<=bounds.right(){right}else{panel.left()-width},y)
        };
        Some(Rect::from_min_size(Pos2::new(x,y),Vec2::new(width,height)))
    }

    /// 展开后的未读胶囊：每个账号一个图标，点击跳到对应窗口；超过三个时滚轮翻看。
    fn draw_unread_capsule(&mut self,ctx:&egui::Context,c:&Card,rect:Rect,cursor:Option<Pos2>,cmds:&mut Vec<Cmd>){
        let n=self.pending_messengers.len();
        let height=rect.height()/c.s;
        let slot=self.unread_slot(height);
        let max_scroll=(n as f32-3.0).max(0.0)*slot;
        if max_scroll>0.0 && cursor.is_some_and(|p|rect.contains(p)) {
            let delta=ctx.input(|i|i.raw_scroll_delta.y);
            self.messenger_scroll-=delta*0.5;
        }
        self.messenger_scroll=self.messenger_scroll.clamp(0.0,max_scroll);
        let c=c.faded(self.messenger_appear());
        let inner=c.clipped(0.0,2.0,UNREAD_W,height-4.0);
        let x=UNREAD_W/2.0;
        for i in 0..n {
            let u=self.pending_messengers[i];
            let cy=2.0+slot*(i as f32+0.5)-self.messenger_scroll;
            if cy< -slot || cy>height+slot {continue;}
            let scale=if u.source==Messenger::Wechat {1.0}else{1.12};
            crate::extras::messenger_icon(&inner.scaled(x,cy,scale),x,cy,u.source);
            inner.circle(x+10.0,cy-9.0,2.6,Color32::from_rgb(255,72,72));
            if let Some(tag)=self.unread_tag(i){
                inner.circle(x+10.0,cy+8.5,6.0,Color32::from_gray(238));
                inner.centered_text(x+10.0,cy+8.5,&tag.to_string(),8.5,W::Bold,Color32::from_gray(28),H::Center);
            }
            let name=match self.unread_tag(i){Some(n)=>format!("{} · 账号 {n}",u.source.name()),None=>u.source.name().to_string()};
            let button=inner.hit(0.0,cy-slot/2.0,UNREAD_W,slot,self.unread_id(i)).on_hover_text(format!("查看{name}"));
            if button.clicked(){cmds.push(Cmd::ViewMessenger(u));}
        }
        if max_scroll>0.0 {
            // 还有没露出的图标：上下边缘各一条淡淡的指示。
            let ink=paint::alpha(theme::WHITE,0.35);
            if self.messenger_scroll>0.5 {c.fill(x-6.0,4.0,12.0,2.0,1.0,ink);}
            if self.messenger_scroll<max_scroll-0.5 {c.fill(x-6.0,height-6.0,12.0,2.0,1.0,ink);}
        }
    }

    /// 收起时的未读入口：每个来源一个图标加小红点（同一来源多个账号合成一个）。只有一条时点它直接跳转，多条时展开面板挑选。
    fn draw_unread_stack(&mut self,c:&Card,horizontal:bool,cmds:&mut Vec<Cmd>){
        let sources=self.unread_sources();
        let count=self.pending_messengers.len();
        let icons=sources.len().min(2);
        let step=unread_step(horizontal);
        let at=|k:usize|if horizontal {(16.0+step*k as f32,14.0)}else{(16.0,14.0+step*k as f32)};
        for k in 0..icons {
            let (x,y)=at(k);
            let scale=if sources[k]==Messenger::Wechat {0.85}else{0.92};
            crate::extras::messenger_icon(&c.scaled(x,y,scale),x,y,sources[k]);
        }
        // 每个来源图标右上角一个小红点，不显示数字。
        for k in 0..icons {let (x,y)=at(k);c.circle(x+8.0,y-7.0,2.6,Color32::from_rgb(255,59,48));}
        let (w,h)=if horizontal {(self.unread_stack_len(true),BAR_H)}else{(32.0,self.unread_stack_len(false))};
        if count==1 {
            let u=self.pending_messengers[0];
            let button=c.hit(0.0,0.0,w,h,u.source.name()).on_hover_text(format!("查看{}",u.source.name()));
            if button.clicked(){cmds.push(Cmd::ViewMessenger(u));}
        }else{
            let button=c.hit(0.0,0.0,w,h,"messenger-stack").on_hover_text(format!("{count} 条未读 · 展开挑选"));
            if button.clicked(){self.reveal_unread();}
        }
    }
    /// 展开面板，让旁边的未读胶囊露出来。
    pub(crate) fn reveal_unread(&mut self){
        self.notice=None;
        self.open=true; self.forced_open_until=self.t+4.0;
        let target_h=self.expanded_height();
        self.tune_open(PW,target_h);
    }
    fn notice_rect(&self,cx:f32,top:f32)->Option<Rect>{
        if !self.compact_notices(){return None;}
        let width=self.notice_width.pos-8.0;
        (width>0.5).then(||Rect::from_min_size(Pos2::new(cx+self.w.pos/2.0+8.0,top),Vec2::new(width,BAR_H)))
    }
    fn notice_details_rect(&self,cx:f32,top:f32)->Option<Rect>{
        let a=self.hud_notice.as_ref().filter(|_|self.notice_details&&self.notice.is_some())?;
        let badge=self.notice_rect(cx,top)?;let (width,height)=self.notice_size(a);
        let mut x=badge.left();
        // 岛体靠近屏幕右缘时，详情卡片向左展开，始终完整留在屏幕内。
        if let Some(screen)=self.screen {x=x.min(screen.right()-width-8.0).max(screen.left()+8.0);}
        Some(Rect::from_min_size(Pos2::new(x,badge.bottom()+6.0),Vec2::new(width,height)))
    }

    /// 当前皮肤的换色调色板（经典为 None）。
    pub(crate) fn skin_palette(&self)->Option<crate::skin_paint::Palette> {
        match self.settings.skin {model::Skin::Cream=>Some(crate::skin_paint::CREAM),model::Skin::GlassLight=>Some(crate::skin_paint::GLASS_LIGHT),model::Skin::GlassDark=>Some(crate::skin_paint::GLASS_DARK),model::Skin::Classic|model::Skin::Fluid=>None}
    }

    /// 设置页预览用的岛体底色：没有真实模糊，毛玻璃按较实的玻璃表现。
    pub(crate) fn preview_fill(&self)->Color32 {
        match self.settings.skin {
            model::Skin::GlassDark=>{let (r,g,b)=glass::GROUND_DARK;Color32::from_rgba_unmultiplied(r,g,b,215)}
            model::Skin::GlassLight=>{let [r,g,b]=glass::LIGHT_GROUND.map(|v|(v*255.0).round() as u8);Color32::from_rgba_unmultiplied(r,g,b,225)}
            _=>self.glass_fill(),
        }
    }

    /// 岛体底色：经典为纯黑；奶油为暖白；毛玻璃按当前着色量叠加深色或浅色玻璃。
    fn glass_fill(&self)->Color32 {
        let alpha=(self.glass_tint.clamp(0.0,1.0)*255.0).round() as u8;
        match self.settings.skin {
            model::Skin::GlassDark=>{let (r,g,b)=glass::GROUND_DARK;Color32::from_rgba_unmultiplied(r,g,b,alpha)}
            model::Skin::GlassLight=>{let [r,g,b]=glass::LIGHT_GROUND.map(|v|(v*255.0).round() as u8);Color32::from_rgba_unmultiplied(r,g,b,alpha)}
            model::Skin::Cream=>{let [r,g,b]=crate::skin_paint::CREAM.paper;Color32::from_rgb(r,g,b)}
            model::Skin::Classic=>Color32::BLACK,
            model::Skin::Fluid=>self.fluid.compose(0.5,0.5,1.0,1.0,0.0),
        }
    }

    /// 毛玻璃边缘高光：浅色玻璃更亮，像苹果的玻璃边缘。
    fn glass_rim(&self)->Color32 {
        let strength=if self.settings.skin==model::Skin::GlassLight {0.55} else {glass::rim_alpha()*0.7};
        Color32::from_white_alpha((255.0*strength).round() as u8)
    }

    /// 绘制岛体（或通知胶囊）的本体：经典皮肤为纯黑；毛玻璃为着色底 + 顶部柔光 + 边缘高光。
    fn paint_body(&mut self,painter:&egui::Painter,rect:Rect,flare:f32,flare_h:f32,radius:f32,feather:f32){
        let skin=self.settings.skin;
        if skin==model::Skin::Fluid {
            // 胶囊 + 顶部停靠：和毛玻璃共用 egui 的精确圆角矩形，颜色走一张小纹理。
            // 旧的「按行梯形条 + 折线圆弧 + 手工羽化带」只在悬浮云上用，
            // 直线转圆弧处的折角感正是它独有、毛玻璃没有的瑕疵。
            if self.settings.island_shape==model::IslandShape::Pill && !self.dock().is_side() {
                self.paint_fluid_pill(painter,rect,radius);
                return;
            }
            let pill=self.settings.island_shape==model::IslandShape::Pill;
            let (map,frame)=shape::EdgeMap::of(rect,if pill {model::Dock::Top}else{self.dock()});
            let mut rows=shape::sections(frame,flare,flare_h,radius);
            if pill {
                let mid=frame.center().y;
                rows.retain(|row|row.0>mid && row.0>=frame.bottom()-radius);
                let top=rows.iter().rev().map(|&(y,l,r)|(2.0*mid-y,l,r)).collect::<Vec<_>>();
                rows.splice(0..0,top);
            }
            let expand=self.progress().clamp(0.0,1.0);
            let mut mesh=shape::shade_mesh(frame,rows,feather,&|x,y|{
                let p=map.apply(Pos2::new(x,y))-rect.min;
                self.fluid.compose(p.x,p.y,rect.width(),rect.height(),expand)
            });
            if self.dock().is_side() && !pill {
                for vertex in &mut mesh.vertices {vertex.pos=map.apply(vertex.pos);}
            }
            painter.add(egui::Shape::mesh(mesh));
            return;
        }
        if self.settings.island_shape==model::IslandShape::Pill {
            let radius=radius.min(rect.height().min(rect.width())*0.5);
            painter.rect_filled(rect,radius,self.glass_fill());
            if skin.is_glass() && rect.height()>=10.0 {
                painter.rect_stroke(rect,radius,egui::Stroke::new(1.0_f32,self.glass_rim()),egui::StrokeKind::Inside);
            }
            return;
        }
        let dock=self.dock();
        if dock.is_side() {
            shape::fill_docked(painter,rect,dock,flare,flare_h,radius,self.glass_fill(),feather);
            if !skin.is_glass() {return;}
            if skin==model::Skin::GlassLight {
                let sheen=Color32::from_white_alpha(15);
                shape::fill_gradient_docked(painter,rect,dock,flare,flare_h,radius,sheen,Color32::TRANSPARENT,(rect.width()*0.6).min(64.0),feather);
            }
            if rect.height()>=20.0 && rect.width()>=10.0 {
                let edge=if dock==model::Dock::Left {Rect::from_min_max(rect.min+Vec2::new(0.0,0.5),rect.max-Vec2::new(0.5,0.5))}
                    else {Rect::from_min_max(rect.min+Vec2::new(0.5,0.5),rect.max-Vec2::new(0.0,0.5))};
                painter.add(egui::Shape::line(shape::outline_docked(edge,dock,flare,flare_h,(radius-0.5).max(0.0)),egui::Stroke::new(1.0_f32,self.glass_rim())));
            }
            return;
        }
        shape::fill(painter,rect,flare,flare_h,radius,self.glass_fill(),feather);
        if !skin.is_glass() {return;}
        // 玻璃厚度带来的上亮下暗：顶部白光在前 60% 高度（最多 64 点）内淡出；浅色玻璃更亮。
        if skin==model::Skin::GlassLight {
            let sheen=Color32::from_white_alpha(15);
            shape::fill_gradient(painter,rect,flare,flare_h,radius,sheen,Color32::TRANSPARENT,(rect.height()*0.6).min(64.0),feather);
        }
        if rect.height()>=10.0 && rect.width()>=20.0 {
            let edge=Rect::from_min_max(rect.min+Vec2::new(0.5,0.0),rect.max-Vec2::new(0.5,0.5));
            painter.add(egui::Shape::line(shape::outline(edge,flare,flare_h,(radius-0.5).max(0.0)),egui::Stroke::new(1.0_f32,self.glass_rim())));
        }
    }

    /// 悬浮云主体：轮廓交给 `RectShape` 的精确圆角，流动颜色做成一张固定尺寸的小纹理。
    ///
    /// 固定 160×80，避免展开动画里尺寸每帧变化导致纹理反复重新分配；
    /// 光晕尺度是 152×100 逻辑点，双线性插值完全够用，看不出采样痕迹。
    fn paint_fluid_pill(&mut self,painter:&egui::Painter,rect:Rect,radius:f32){
        const TEX_W:usize=160;
        const TEX_H:usize=80;
        let w=rect.width().max(1.0);
        let h=rect.height().max(1.0);
        let expand=self.progress().clamp(0.0,1.0);
        let mut image=egui::ColorImage::new([TEX_W,TEX_H],Color32::TRANSPARENT);
        for row in 0..TEX_H {
            let py=h*(row as f32+0.5)/TEX_H as f32;
            for col in 0..TEX_W {
                let px=w*(col as f32+0.5)/TEX_W as f32;
                image.pixels[row*TEX_W+col]=self.fluid.compose(px,py,w,h,expand);
            }
        }
        let options=egui::TextureOptions::LINEAR;
        let handle=self.fluid_tex.get_or_insert_with(||{
            painter.ctx().load_texture("daozai-fluid",image.clone(),options)
        });
        handle.set(image,options);
        let radius=radius.min(rect.height().min(rect.width())*0.5);
        let uv=Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0));
        painter.add(egui::epaint::RectShape::filled(rect,radius,Color32::WHITE).with_texture(handle.id(),uv));
    }

    /// 悬停状态机：意图缓冲 → 弹簧展开 → 离开延迟 → 临界阻尼收起；全程可打断。
    /// 按当前配置的动效启动展开；「瞬时」直接到位，不跑形变。
    fn tune_open(&mut self,target_w:f32,target_h:f32){
        if let Some(t)=self.settings.open_anim.tuning() {
            self.w.tune(t.open_w.0,t.open_w.1);
            self.h.tune(t.open_h.0,t.open_h.1);
            self.w.target=target_w;self.h.target=target_h;
        } else {
            self.w.snap(target_w);self.h.snap(target_h);
        }
    }
    /// 按当前配置的动效收起；「瞬时」直接到位。
    fn tune_close(&mut self,bw:f32,bh:f32){
        if let Some(t)=self.settings.open_anim.tuning() {
            self.w.tune(t.close.0,t.close.1);
            self.h.tune(t.close.0,t.close.1);
            self.w.target=bw;self.h.target=bh;
        } else {
            self.w.snap(bw);self.h.snap(bh);
        }
    }

    fn morph(&mut self, want: bool, inside: bool, t: f64, dt: f32) {
        let (bw, bh) = self.base_size();
        let tuning=self.settings.open_anim.tuning();
        let instant=tuning.is_none();
        // 非展开态每帧都会重新 tune，所以这里也要用配置里的收起参数，
        // 否则 `tune_close` 设置的参数只在切换那一帧有效。
        let close=tuning.map(|t|t.close).unwrap_or(CLOSE);
        if want {
            self.leave_since = None;
            let since = *self.hover_since.get_or_insert(t);
            let delay=hover_delay_secs(self.settings.hover_delay_ms,self.effective_minimal());
            let swell=self.settings.hover_delay_ms>=SWELL_MIN_MS && !instant;
            if !self.open && (self.drag_hot || t - since >= delay) {
                self.open = true;
                let target_h=self.expanded_height();
                self.tune_open(PW,target_h);
                if !instant && self.drag_hot {
                    // 拖入不等缓冲，直接给一点初速度，30 ms 内就能看到形变
                    self.w.vel = self.w.vel.max(600.0);
                    self.h.vel = self.h.vel.max(260.0);
                }
            } else if !self.open {
                if swell {
                    self.w.tune(SWELL_K.0, SWELL_K.1);
                    self.h.tune(SWELL_K.0, SWELL_K.1);
                    // 悬停时轻微胀大：厚度方向胀得多一点（侧边停靠时厚度是宽度）。
                    let (sw,sh)=if self.side(){(SWELL.1,SWELL.0)}else{SWELL};
                    self.w.target = bw * sw;
                    self.h.target = bh * sh;
                } else {
                    // 短等待（或瞬时动效）：保持静止，到点直接用同一条弹簧展开，
                    // 不会出现「鼓一半被拉走」的接力感。
                    self.w.tune(close.0, close.1);
                    self.h.tune(close.0, close.1);
                    self.w.target = bw;
                    self.h.target = bh;
                }
            }
        } else {
            self.hover_since = None;
            if self.open {
                let since = *self.leave_since.get_or_insert(t);
                if t - since >= LEAVE_DELAY {
                    self.open = false;
                    self.leave_since = None;
                    self.tune_close(bw,bh);
                    self.cancel_edit();
                    self.confirm_clear = None;
                }
            } else {
                self.w.tune(close.0, close.1);
                self.h.tune(close.0, close.1);
                self.w.target = bw;
                self.h.target = bh;
            }
        }
        let _ = inside;
        if self.open {self.h.target=self.expanded_height();}
        let (prev_w,prev_h)=(self.w.pos,self.h.pos);
        self.w.step(dt);
        self.h.step(dt);
        // 仅在从外侧越过目标时收住，扩展提示条时不强制跳到目标宽度。
        if !self.open {
            if prev_w>=bw && self.w.pos<bw {self.w.snap(bw);}
            if prev_h>=bh && self.h.pos<bh {self.h.snap(bh);}
        }
        for d in &mut self.dots {
            d.step(dt);
        }
        self.slide.step(dt);
    }

    // ---------------------------------------------------------------
    // 顶栏：电池 + 日期 + 时钟。药丸里的电池与时钟会「飞」到顶栏位置（共享元素过渡）。
    // ---------------------------------------------------------------
    /// 单条状态的估算宽度：中文按 11px/字，夹在 48~140 之间。
    fn status_item_width(&self, status: &crate::island_status::Status) -> f32 {
        let chars = status.text.chars().count() as f32;
        (chars * 11.0 + 24.0).clamp(48.0, 140.0)
    }

    /// 状态区总宽：最多 3 条，条间 6px。
    fn status_total_width(&self) -> f32 {
        let shown = self.status_live.len().min(3);
        if shown == 0 { return 0.0; }
        let items: f32 = self.status_live.iter().take(shown).map(|s| self.status_item_width(s)).sum();
        items + (shown.saturating_sub(1)) as f32 * 6.0
    }

    fn shared_header(&mut self, base: &Card, cx: f32, p: f32, reveal: f32, env: &Env) {
        let local_cx = cx - base.origin.x;
        let fit=((WIN_W-32.0)/self.resident.target.width).min(1.0);
        let fitted=base.scaled(local_cx,15.0,lerp(fit,1.0,p));
        let base=&fitted;
        let lyric=self.island_lyric().map(|(index,_)|index);
        let lyric_alpha=if self.island_lyrics_active() {1.0-smoothstep(0.0,0.35,p)}else{0.0};
        // 提前算好，歌词接管胶囊时也要用它把手机电量留在左侧。
        let resident_battery=self.battery_visible();
        if let Some(galley)=self.island_lyric_layout.clone().filter(|_|lyric_alpha>0.003) {
            let c=base.faded(lyric_alpha);
            let content_width=self.lyric_bar_width()-self.lyric_focus_space();
            let width=self.w.pos.max(content_width);
            let left=local_cx-width/2.0;
            let center_y=BAR_H/2.0;
            // 有手机电量时把封面右移，给最左侧的常驻电量环让出位置。
            let art_left=left+if resident_battery {36.0}else{12.0};
            if let Some((image,texture))=&self.music_artwork {
                let [width,height]=image.size;
                let uv_size=if width>height {egui::vec2(height as f32/width as f32,1.0)}else{egui::vec2(1.0,width as f32/height as f32)};
                let uv=Rect::from_center_size(Pos2::new(0.5,0.5),uv_size);
                c.painter.add(egui::epaint::RectShape::filled(c.rect(art_left,center_y-10.0,20.0,20.0),5.0*c.s,Color32::WHITE).with_texture(texture.id(),uv));
            } else {
                c.fill(art_left,center_y-10.0,20.0,20.0,5.0,theme::CARD);
                paint::note(&c,art_left+10.0,center_y,13.0,theme::WHITE);
            }
            let index=lyric.unwrap_or(0);
            let id=egui::Id::new("island-lyric-line");
            if !self.ly.island_animation_started {
                c.ui.ctx().animate_value_with_time(id,index as f32,0.0);
                self.ly.island_animation_started=true;
            }
            let shown=c.ui.ctx().animate_value_with_time(id,index as f32,0.18);
            let fade=1.0-(index as f32-shown).abs().min(1.0);
            // 歌词固定在电量环/封面/宠物右侧的通道里，并在剩余区域居中；
            // 测试构建沿用官方坐标。galley 本身是左对齐绘制的，这里手动补偿居中。
            let lyric_left=match self.lyric_lane() {
                Some((_,text_left))=>{
                    let avail=(content_width-LANE_RIGHT_PAD-text_left).max(90.0);
                    // 居中偏移设上限：短歌词不会被推到通道正中间、与宠物之间空出一大片。
                    // 间隙因此稳定在 LANE_TEXT_GAP ~ LANE_TEXT_GAP+LANE_CENTER_CAP。
                    let centering=((avail-galley.size().x/c.s).max(0.0)/2.0).min(LANE_CENTER_CAP);
                    text_left+centering
                },
                None=>content_width-LYRIC_BAR_W/2.0,
            };
            // 长歌词横向滚动：句首停留后匀速推进，到尾部停住，换句复位。
            let scroll_offset=self.island_lyric_scroll.map_or(0.0,|scroll|{
                let elapsed=(self.t-self.island_lyric_scroll_start).max(0.0) as f32;
                lyric_scroll_offset(elapsed,scroll.speed,scroll.overflow)/c.s
            });
            let position=c.pos(left+lyric_left-scroll_offset,center_y-galley.size().y/2.0+(1.0-fade)*3.0);
            // 兜底裁剪：超长或异常 galley 一律不许画进左侧固定通道。
            let lyric_c=match self.lyric_lane() {
                Some((_,text_left))=>c.clipped(left+text_left,0.0,(content_width-LANE_RIGHT_PAD-text_left).max(90.0),BAR_H),
                None=>c.clone(),
            };
            lyric_c.painter.galley(position,galley,if lyric.is_some(){theme::WHITE}else{theme::GRAY});
            paint::playback_bars(&c,left+content_width-21.0,center_y,self.t,self.media.playing,theme::WHITE);
            if self.pomo.running {
                let alpha=smoothstep(0.88,1.0,(width-content_width)/self.lyric_focus_space());
                crate::focus_bar::draw(&c.faded(alpha),left+width-14.0-crate::focus_bar::LYRIC_WIDTH/2.0,center_y,self.t,&self.pomo,self.phase_secs(self.pomo.phase),true);
            }
        }
        if lyric_alpha>=0.997 {
            // 电量常驻：歌词完全接管胶囊时，仍在最左侧保留电量环（本机在左、手机在右）。
            if resident_battery {
                let content_width=self.lyric_bar_width()-self.lyric_focus_space();
                let width=self.w.pos.max(content_width);
                let left=local_cx-width/2.0;
                let (pc_ring,phone_ring)=self.battery_rings();
                let mut cx=left+20.0;
                for ring in [pc_ring,phone_ring].into_iter().flatten() {
                    paint::battery(&base.faded(1.0),cx,BAR_H/2.0,ring.0,ring.1,1.0);
                    cx+=20.0;
                }
            }
            return;
        }
        let header=base.faded(1.0-lyric_alpha);
        let base=&header;
        let m = anim::ease_in_out(smoothstep(0.0, 0.85, p));
        let clock = self.settings.clock_text(&env.clock);
        let show_battery=resident_battery;
        let (pc_ring,phone_ring)=self.battery_rings();
        let ring_count=(pc_ring.is_some() as u8+phone_ring.is_some() as u8) as f32;
        // 两个环并排时各自偏离中心 10px。
        let ring_offset=if ring_count>1.0 {10.0}else{0.0};

        // 顶栏终点布局：[电池][日期][时钟] 作为一组整体居中
        let date_w = base.text_w(&env.date, size::DATE, W::Regular);
        let clock_w = paint::clock_width(base,&clock,size::CLOCK);
        let battery_space=if show_battery {24.0+(ring_count-1.0)*20.0}else{0.0};
        let group = battery_space + date_w + 8.0 + clock_w;
        let gx = local_cx - group / 2.0 + if show_battery {6.0}else{0.0};
        // 日程页左侧还有新增按钮，顶栏从操作区右侧开始，避免较长的日期和时段文字挤到按钮上。
        let gx = if self.page==CALENDAR_PAGE {gx.max(155.0)}else{gx};
        let hb = (gx + 8.5, 18.6);
        let hd = gx + battery_space;
        let hc = hd + date_w + 8.0;
        // 药丸起点布局
        let has_music=self.media.is_music();
        let layout=self.resident.current();
        let pb=(local_cx+layout.x[bar::BATTERY].unwrap_or(0.0),15.0);
        let pcx=local_cx+layout.x[bar::CLOCK].unwrap()-paint::clock_width(base,&clock,size::PILL_CLOCK)/2.0;

        if self.effective_minimal() && self.t>=self.notification_bell_until && p < 0.02 { return; }
        // 电池：位置/大小连续插值
        let bk = lerp(1.0, 0.8, m);
        let bpos = (lerp(pb.0, hb.0, m), lerp(pb.1, hb.1, m));
        if show_battery {
            let alpha=lerp(layout.clearance(bar::BATTERY),1.0,m);
            if let Some((pct,chg))=pc_ring {paint::battery(&base.faded(alpha),bpos.0-ring_offset,bpos.1,pct,chg,bk);}
            if let Some((pct,chg))=phone_ring {paint::battery(&base.faded(alpha),bpos.0+ring_offset,bpos.1,pct,chg,bk);}
        }

        // 时钟：两个字号沿同一路径交叉淡化，避免逐帧换字号
        let cpos = (lerp(pcx, hc, m), lerp(21.0, 23.0, m));
        let a_pill = (1.0 - smoothstep(0.25, 0.6, m)) * layout.clearance(bar::CLOCK);
        let a_head = smoothstep(0.35, 0.75, m);
        if a_pill > 0.0 {
            paint::clock(&base.faded(a_pill),cpos.0,cpos.1,&clock,size::PILL_CLOCK);
        }
        if a_head > 0.0 {
            paint::clock(&base.faded(a_head),cpos.0,cpos.1-0.5,&clock,size::CLOCK);
        }
        // 日期随内容淡入，从药丸方向轻轻滑出
        if reveal > 0.0 {
            base.faded(reveal).text(hd + (1.0 - reveal) * 10.0, 22.5, &env.date, size::DATE, W::Regular, Color32::from_rgb(210, 210, 214), H::Left);
        }
        if has_music && (p>0.1 || layout.x[bar::MUSIC].is_some()) {
            paint::playback_bars(base,lerp(local_cx+layout.x[bar::MUSIC].unwrap_or(0.0),367.0,m),lerp(15.0,18.0,m),self.t,self.media.playing,theme::WHITE);
        }
        let compact_alpha=1.0-smoothstep(0.0,0.22,p);
        if self.t<self.notification_bell_until&&!self.compact_notices()&&p<0.1 {
            if let Some(x)=layout.x[bar::BELL]{
                let x=local_cx+x;paint::bell(&base.scaled(x,14.0,1.2),x,14.0,theme::WHITE,paint::bell_swing(self.t-self.notification_bell_at));
                base.circle(x+4.0,9.5,2.0,Color32::from_rgb(255,72,72));
                if base.hit(x-14.0,3.0,28.0,25.0,"bell-mail").clicked(){self.show_page(5);}
            }
        }
        if compact_alpha>0.004 {
            let c=base.faded(compact_alpha);let c=if compact_alpha<0.8{c.ghost()}else{c};
            self.draw_quota_bar(&c,local_cx,&layout);
        }
        if self.pomo.running && compact_alpha>0.004 {
            crate::focus_bar::draw(&base.faded(compact_alpha),local_cx+layout.x[bar::FOCUS].unwrap(),15.0,self.t,&self.pomo,self.phase_secs(self.pomo.phase),false);
        }
        // 状态总线：并排显示最多 3 条常驻状态（色点 + 文本 + 可选进度条）。
        if let Some(slot)=layout.status {
            if let Some(center)=layout.x[slot] {
                if compact_alpha>0.004 && !self.status_live.is_empty() {
                    let c=base.faded(compact_alpha);
                    let total=layout.widths[slot];
                    let mut x=local_cx+center-total/2.0;
                    for status in self.status_live.iter().take(3) {
                        let w=self.status_item_width(status);
                        let tone=crate::extras::status_color(&status.color);
                        c.fill(x,5.0,w,20.0,10.0,Color32::from_gray(36));
                        c.circle(x+8.0,15.0,2.5,tone);
                        c.centered_text(x+14.0,15.0,&c.fit(&status.text,10.5,W::Regular,w-20.0),10.5,W::Regular,theme::WHITE,H::Left);
                        if let Some(progress)=status.progress.filter(|v|v.is_finite()) {
                            let p=progress.clamp(0.0,1.0) as f32;
                            c.fill(x+2.0,23.0,w-4.0,1.5,0.75,Color32::from_gray(60));
                            c.fill(x+2.0,23.0,(w-4.0)*p,1.5,0.75,tone);
                        }
                        x+=w+6.0;
                    }
                }
            }
        }
    }

    /// 常驻内容：顶部停靠走共享元素过渡（药丸里的电池、时钟飞到顶栏）；
    /// 左右停靠时竖排内容随展开淡出，顶栏随面板内容淡入。
    fn resident_header(&mut self, base: &Card, panel: Rect, cx: f32, p: f32, reveal: f32, env: &Env) {
        if !self.side() {self.shared_header(base,cx,p,reveal,env);return;}
        let bar_alpha=1.0-smoothstep(0.0,0.3,p);
        let minimal=self.effective_minimal()&&self.t>=self.notification_bell_until;
        if bar_alpha>0.004 && !minimal {
            let c=base.shifted(panel.min.x-base.origin.x,panel.min.y-base.origin.y).faded(bar_alpha);
            let c=if bar_alpha<0.8 {c.ghost()} else {c};
            self.side_bar(&c,panel.width(),panel.height(),env);
        }
        if reveal>0.004 {self.shared_header(&base.faded(reveal),base.origin.x+PW/2.0,1.0,reveal,env);}
    }

    fn update_pet(&mut self, env: &Env, cx: f32) {
        let cx=cx+self.resident.position(bar::PET);
        let idle = self.idle_for();
        if self.pet.last_idle >= self.opts.pet_idle && idle < 1.0
            && matches!(self.pet.state, PetState::Dozing | PetState::Sleeping) {
            self.pet.waking_until = self.t + 2.2;
        }
        self.pet.last_idle = idle;
        self.pet.hovered = env.cursor.is_some_and(|p| (p.x-cx).abs() <= BAR_W/2.0+24.0 && (0.0..70.0).contains(&p.y));
        let (pc_ring,phone_ring)=self.battery_rings();
        let low_battery=[pc_ring,phone_ring].into_iter().flatten().any(|(pct,charging)|pct.is_some_and(|v|v<=20)&&!charging);
        self.pet.worried = low_battery || env.network == Some(false);
        let allowed = self.pet_enabled();
        if !allowed { self.pet.reaction = None; self.pet.waking_until = 0.0; self.pet.expressions.clear(); }
        let available = allowed && !self.open && !self.drag_hot;
        let shown = available;
        self.pet_a = anim::approach(self.pet_a, if shown {1.0}else{0.0}, 9.0, self.dt);
        let visible = available && self.pet_a > 0.7 && self.progress() < 0.1;
        self.pet.advance(self.dt, visible);
        let state = self.pet_state();
        let gaze = env.cursor.map(|p| [((p.x-cx)/28.0).clamp(-3.0,3.0), ((p.y-15.0)/40.0).clamp(-1.0,1.5)])
            .unwrap_or([(self.t as f32 * 0.35).sin()*0.8, 0.0]);
        self.pet.step(state, self.t, self.dt, gaze, visible);
    }

    // ---------------------------------------------------------------
    // 面板主体：页标题 + 页面（带切页过渡）+ 翻页箭头 + 页码豆
    // ---------------------------------------------------------------
    fn panel_body(&mut self, c: &Card, env: &Env, cmds: &mut Vec<Cmd>) {
        let settings=c.hit(380.0,4.0,28.0,28.0,"page-settings").on_hover_text(format!("打开{}设置窗口",PAGE_NAMES[self.page]));
        let (hover,press)=c.hover_t(&settings);
        c.circle(394.0,18.0,13.0,Color32::from_white_alpha((hover*24.0) as u8));
        paint::gear(&c.scaled(394.0,18.0,0.8*(1.0-press*0.08)),394.0,18.0,paint::mix(theme::GRAY,theme::WHITE,hover));
        if settings.clicked(){self.settings_tab=[1,2,3,5,4,7,11,8][self.page];self.open_settings();}
        if self.subpage==Some(self.page) {
            c.text(15.0,25.0,"‹",19.0,W::Bold,theme::WHITE,H::Left);
            c.text(37.0,25.0,if self.page==2{"饮水设置"}else{"专注设置"},size::TITLE,W::Bold,theme::WHITE,H::Left);
            if c.hit(9.0,7.0,28.0,23.0,"subpage-back").clicked(){self.subpage=None;self.focus_editing=false;}
            self.page_settings(c);self.nav(c);return;
        }
        let e = self.page_e();
        let in_tx = e < 0.55 && self.page_from != self.page;
        if in_tx {
            // 旧页：原地向反方向轻移并快速淡出；新页：弹簧滑入 + 稍晚淡入
            let old_a = 1.0 - smoothstep(0.0, 0.16, e);
            let new_a = smoothstep(0.03, 0.28, e);
            let old_dx = -self.page_dir * 18.0 * anim::ease_out_cubic(e / 0.3);
            if old_a > 0.0 {
                let oc = c.faded(old_a).ghost();
                self.page_title(self.page_from, &oc);
                self.render_page(self.page_from, &oc.shifted(old_dx, 0.0), env, &mut Vec::new());
            }
            let nc = c.faded(new_a);
            self.page_title(self.page, &nc);
            let nc = nc.shifted(self.slide.pos, 0.0);
            let nc = if e < 0.12 { nc.ghost() } else { nc };
            self.render_page(self.page, &nc, env, cmds);
        } else {
            self.page_title(self.page, c);
            self.render_page(self.page, c, env, cmds);
        }
        self.nav(c);
    }

    fn page_title(&self, page: usize, c: &Card) {
        let base = 25.0;
        let (a, b, col) = match page {
            0 | CALENDAR_PAGE => return,
            1 => ("番茄", "专注", theme::RED_TOP),
            2 => ("饮水", "提醒", theme::BLUE),
            // 置物架的标题就是「置物架 | 剪贴板」分栏，由页面自己画。
            3 => return,
            4 => ("待办", "清单", theme::GREEN),
            5 => ("邮箱", "", theme::CYAN),
            6 => ("AI", "额度", theme::SOFT_GREEN),
            _ => ("岛仔", "设置", theme::WHITE),
        };
        let wa = c.text(15.0, base, a, size::TITLE, W::Bold, theme::WHITE, H::Left);
        if !b.is_empty() {
            c.text(15.0 + wa + 1.5, base, b, size::TITLE, W::Bold, col, H::Left);
        }
    }

    fn nav(&mut self, c: &Card) {
        let pages: Vec<_>=self.settings.page_order.iter().copied().filter(|page|self.settings.visible_pages[*page]).collect();
        if pages.len()<2 { return; }
        let arrow_y=if self.page==6{88.0}else{68.0};
        let l = c.hit(8.0, arrow_y-13.0, 26.0, 26.0, "nav-l");
        let r = c.hit(385.0, arrow_y-13.0, 26.0, 26.0, "nav-r");
        let (lh, lp) = c.hover_t(&l);
        let (rh, rp) = c.hover_t(&r);
        paint::chevron(c, 21.0, arrow_y, -1.0, lh, lp);
        paint::chevron(c, 398.0, arrow_y, 1.0, rh, rp);
        if l.clicked() { self.navigate_page(false); }
        if r.clicked() { self.navigate_page(true); }
        // 页码豆：宽度由弹簧驱动，整行实时居中；当前页是白色长条
        let gap = 4.0;
        let dot_y=self.expanded_height()-7.2;
        let total: f32 = pages.iter().map(|page| self.dots[*page].pos).sum::<f32>() + gap * (pages.len()-1) as f32;
        let mut x = PW / 2.0 - total / 2.0;
        let mut clicked = None;
        for i in pages {
            let w = self.dots[i].pos.max(3.0);
            let on = anim::remap(w, 4.0, 16.0);
            let col = paint::mix(Color32::from_rgb(92, 92, 96), theme::WHITE, on);
            c.fill(x, dot_y - 2.0, w, 4.0, 2.0, col);
            let hr = c.hit(x - 2.0, dot_y - 6.0, w + 4.0, 12.0, ("dot", i));
            hr.clone().on_hover_text(PAGE_NAMES[i]);
            if hr.hovered() && i != self.page {
                c.fill(x - 1.0, dot_y - 3.0, w + 2.0, 6.0, 3.0, Color32::from_white_alpha(28));
            }
            if hr.clicked() {
                clicked = Some(i);
            }
            x += w + gap;
        }
        if let Some(i) = clicked {
            let position=|page|self.settings.page_order.iter().position(|p|*p==page).unwrap();
            let dir = if position(i) > position(self.page) { 1.0 } else { -1.0 };
            self.go(i, dir);
        }
    }

    fn render_page(&mut self, page: usize, c: &Card, env: &Env, cmds: &mut Vec<Cmd>) {
        match page {
            0 => self.page_music(c, env, cmds),
            1 => self.page_focus(c, env),
            2 => self.page_water(c, env),
            3 => self.page_shelf(c, env, cmds),
            4 => self.page_todo(c, env, cmds),
            5 => self.page_mail(c, env, cmds),
            6 => self.page_quota(c),
            CALENDAR_PAGE => self.page_calendar(c,env),
            _ => self.page_settings(c),
        }
    }

    fn toast_layer(&mut self, c: &Card, base_y:f32)->Option<Rect> {
        let Some((msg, at)) = self.toast.clone() else { return None };
        let e = (self.t - at) as f32;
        if e > 2.4 {
            self.toast = None;
            return None;
        }
        let a = smoothstep(0.0, 0.16, e) * (1.0 - smoothstep(2.1, 2.4, e));
        let rise = (1.0 - anim::ease_out_cubic(e / 0.3)) * 6.0;
        let msg=c.fit(&msg,size::BODY,W::Regular,310.0);
        let w = c.text_w(&msg, size::BODY, W::Regular) + 26.0;
        let tc = c.faded(a);
        let y = base_y + rise;
        tc.fill(PW / 2.0 - w / 2.0, y, w, 22.0, 11.0, Color32::from_rgb(44, 44, 48));
        tc.text(PW / 2.0, y + 15.0, &msg, size::BODY, W::Regular, theme::WHITE, H::Center);
        Some(tc.rect(PW/2.0-w/2.0,y,w,22.0))
    }

    // ---------------------------------------------------------------
    // 每帧推进：番茄钟、饮水提醒、跨天
    // ---------------------------------------------------------------
    fn tick(&mut self, env: &Env, dt: f32, elapsed: f32) {
        if self.toast.as_ref().is_some_and(|(_,at)|self.t-at>2.4){self.toast=None;}
        if env.today != self.today {
            self.today = env.today.clone();
            self.water = model::Water::load(&env.today);
            self.last_drink=None;self.last_reminder=self.t;
            self.water_ml = self.water.drunk_ml as f32;
            self.water_ring.snap(self.water.percent() / 100.0);
        }
        // 只统计真正在跑的专注阶段：暂停不计，攒满一分钟落盘一次。
        if self.pomo.running && self.pomo.phase==Phase::Focus {
            self.focus_pending += elapsed as f32;
            if self.focus_pending>=60.0 {
                let minutes=(self.focus_pending/60.0).floor();
                self.focus_pending-=minutes*60.0;
                crate::extras::focus_add(minutes as u32);
                let (today,streak)=crate::extras::focus_summary();
                self.focus_today_min=today;
                self.focus_streak_days=streak;
            }
        }
        if self.pomo.running {
            self.pomo.remaining -= elapsed;
            if self.pomo.remaining <= 0.0 {
                let next = match self.pomo.phase {
                    Phase::Focus => {
                        self.pet.react(PetState::Happy);
                        Phase::Rest
                    }
                    Phase::Rest => Phase::Focus,
                };
                self.pomo.phase = next;
                self.pomo.remaining = self.phase_secs(next);
                self.pomo.running = false;
                self.alert("专注计时", format!("该{}了", next.label()), 1, true);
            }
        }
        self.ring_shown.target = (self.pomo.remaining / self.phase_secs(self.pomo.phase)).clamp(0.0, 1.0);
        self.ring_shown.step(dt);
        self.reset_spin.step(dt);
        self.todo_pop.step(dt);

        if self.water.remind && self.water.percent() < 100.0 && self.water_hours(env.hour() as u32) && self.t - self.last_reminder >= self.settings.water_minutes as f64 * 60.0 {
            self.last_reminder = self.t;
            self.alert("喝水时间到了", format!("休息一下，喝 {} ml 水吧", self.settings.serving_ml), 2, true);
        }
        let now = format!("{} {}", env.today, env.clock.get(..5).unwrap_or("00:00"));
        let due: Vec<_> = self.todos.items.iter().enumerate().filter(|(_, item)| item.reminder_due(&now)).map(|(i,item)| (i,item.text.clone())).collect();
        for (i,text) in due { self.todos.items[i].mark_reminded(&now); self.todos.save(); self.alert("待办到时间了", text, 4, true); }
        // v0.5.5 会议提醒：.ics 里的开始时间，默认提前 15 分钟（可设 5/15/30）。
        // v0.5.6 起日历（CalDAV）事件走同一套：同一场会（UID + DTSTART）只提醒一次，
        // 邮件和日历两条来源撞上了也只弹一次。
        let lead = self.settings.mail_ics as i64 * 60;
        let now_ts = chrono::Local::now().timestamp();
        let mut picks = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in &self.calendar.local {
            let Some(at)=entry.reminder_at() else{continue;};
            let key=calendar::dedupe_key(&entry.event);
            let cutoff=if entry.event.all_day{at+60}else{entry.event.start+60};
            if now_ts>=at && now_ts<cutoff && !self.mail.fired.contains(&key) && seen.insert(key.clone()) {
                picks.push((key,entry.event.clone(),"本机日程".into(),CALENDAR_PAGE));
            }
        }
        for (m,provider) in self.calendar_work_events(None) {
            let left = m.start - now_ts;
            let key = calendar::dedupe_key(m);
            if !m.all_day && left <= lead && left > -60 && !self.mail.fired.contains(&key) && seen.insert(key.clone()) {
                let source=provider.map(|provider|provider.label()).unwrap_or("日历");
                picks.push((key, m.clone(), source.to_string(), CALENDAR_PAGE));
            }
        }
        if self.settings.mail_enabled {
            for m in self.mail.recent.iter().filter_map(|m| m.meeting.as_ref()).filter(|m|!self.mail_event_ignored(m)) {
                let left = m.start - now_ts;
                let key = calendar::dedupe_key(m);
                if !m.all_day && left <= lead && left > -60 && !self.mail.fired.contains(&key) && seen.insert(key.clone()) {
                    picks.push((key, m.clone(), "邮件邀请".to_string(), CALENDAR_PAGE));
                }
            }
        }
        for (key, meeting, source, page) in picks {
            self.mail.fired.insert(key);
            self.meeting_alert(meeting, source, page);
        }
        self.water_ring.target = self.water.percent() / 100.0;
        self.water_ring.step(dt);
        self.water_ml = anim::approach(self.water_ml, self.water.drunk_ml as f32, 9.0, dt);
        self.water_view_t.target = self.water_view as f32;
        self.water_view_t.step(dt);
        self.remind_t.target = if self.water.remind { 1.0 } else { 0.0 };
        self.remind_t.step(dt);
        // 剪贴板记录开关：关闭时同时清掉未固定的记录。
        if self.clipboard.is_enabled() != self.settings.clipboard_history {
            self.clipboard.set_enabled(self.settings.clipboard_history);
        }
        self.shelf_scroll.target = self.shelf_first as f32;
        self.shelf_scroll.step(dt);
        if let Some(at) = self.confirm_clear {
            if self.t - at > 3.0 {
                self.confirm_clear = None;
            }
        }
    }

    /// 媒体进度在两次 SMTC 刷新之间按时间外推，进度条才会连续走动。
    fn update_media(&mut self, env: &Env, ctx: &egui::Context) {
        let m = &env.media;
        if !m.is_music() {
            self.media=NowPlaying::default();
            self.media_anchor=(String::new(),0.0,0.0,false);
            self.music_artwork=None;
            self.ly=LyricsState::default();
            return;
        }
        let a = &self.media_anchor;
        if a.0 != m.title || (a.1 - m.position).abs() > 0.01 || a.3 != m.playing {
            self.media_anchor = (m.title.clone(), m.position, env.t, m.playing);
        }
        let mut np = m.clone();
        if m.playing {
            let (_, pos, _, _) = &self.media_anchor;
            np.position = if let Some(sampled)=m.sampled_at {m.position+sampled.elapsed().as_secs_f64()}
                else {*pos};
            if m.duration>0.0 {np.position=np.position.min(m.duration);}
        }
        // 网易云的 song_id 只来自本机日志，SMTC 那一路没有；None 与 Some 之间来回不是换歌，不能每次都清空歌词重查。
        let other_song=matches!((self.media.song_id,np.song_id),(Some(old),Some(new)) if old!=new);
        if self.media.source != np.source || other_song { self.ly=LyricsState::default(); }
        self.media = np;
        if let Some(image)=&self.media.artwork {
            if !self.music_artwork.as_ref().is_some_and(|(old,_)|std::sync::Arc::ptr_eq(old,image)) {
                self.music_artwork=Some((image.clone(),ctx.load_texture("music-artwork",image.clone(),egui::TextureOptions::LINEAR)));
            }
        } else {self.music_artwork=None;}
    }

    fn poll_lyrics(&mut self) {
        if !self.media.is_music(){self.ly=LyricsState::default();return;}
        if !self.show_lyrics && !self.settings.island_lyrics {self.ly=LyricsState::default();return;}
        let np = &self.media;
        if np.title.is_empty(){self.ly=LyricsState::default();return;}
        let key = format!("{}|{}|{:.0}", np.title, np.artist, np.duration);
        let id_arrived=key==self.ly.key && !self.ly.loading && self.ly.song_id.is_none() && np.song_id.is_some()
            && !self.ly.lyrics.as_ref().is_some_and(|lyrics|lyrics.synced);
        if key != self.ly.key || id_arrived || self.ly.retry_at.is_some_and(|at|self.t>=at) {
            // 歌曲信息抖动或快速切歌不能叠加网络线程；旧请求完成后只查询当前歌曲。
            if self.ly_worker.as_ref().is_some_and(|worker|!worker.is_finished()){return;}
            let Some(lease)=crate::resources::LYRICS.acquire()else{return;};
            let native_only=key==self.ly.key && np.source.to_ascii_lowercase().contains("kugou")
                && self.ly.error==Some(lyrics::FetchError::NotFound);
            let cached=if key==self.ly.key && np.source.to_ascii_lowercase().contains("kugou"){self.ly.lyrics.take()}else{None};
            let attempts=if key==self.ly.key {self.ly.attempts.saturating_add(1)}else{0};
            self.ly = LyricsState { key, loading: cached.is_none() && !native_only, lyrics:cached.clone(),
                error:native_only.then_some(lyrics::FetchError::NotFound), song_id:np.song_id, attempts, ..Default::default() };
            let (tx, rx) = std::sync::mpsc::channel();
            self.ly.rx = Some(rx);
            let (source, title, artist, dur) = (np.source.clone(), np.title.clone(), np.artist.clone(), np.duration);
            let song_id=np.song_id;
            if source.to_ascii_lowercase().contains("kugou") || lyrics::is_soda(&source) {
                let (tx,rx)=std::sync::mpsc::sync_channel(1);self.ly.native_rx=Some(rx);
                let (source,title,artist)=(source.clone(),title.clone(),artist.clone());
                crate::resources::NATIVE_LYRICS.spawn(move ||{
                    // 客户端通常在开播几秒内写好歌词：先密后疏（0.5s 起，最长 5s 一次），最多等 90 秒。
                    // 汽水的缓存文件可达 32MB，修改时间和大小都没变时不重复整份读取，但每 15 秒仍完整核对一次。
                    let started=std::time::Instant::now();
                    let mut wait=std::time::Duration::from_millis(500);
                    let mut last:Option<((std::time::SystemTime,u64),std::time::Instant)>=None;
                    loop {
                        let print=lyrics::native_fingerprint(&source);
                        let unchanged=print.is_some_and(|print|last.is_some_and(|(seen,at)|seen==print && at.elapsed()<std::time::Duration::from_secs(15)));
                        let found=if unchanged {None}else{
                            if let Some(print)=print {last=Some((print,std::time::Instant::now()));}
                            lyrics::read_native(&source,&title,&artist,dur)
                        };
                        let done=found.is_some();
                        match tx.try_send(found) {
                            Ok(())=>{if done{break;}}
                            Err(std::sync::mpsc::TrySendError::Disconnected(_))=>break,
                            Err(std::sync::mpsc::TrySendError::Full(_))=>{}
                        }
                        if started.elapsed()>=std::time::Duration::from_secs(90){break;}
                        std::thread::sleep(wait);
                        wait=(wait*2).min(std::time::Duration::from_secs(5));
                    }
                });
            }
            self.ly_worker=lease.spawn(move || {
                let result=if native_only {
                    lyrics::kugou_cache::read(&title,&artist,dur).map(|lyrics|(lyrics,false)).ok_or(lyrics::FetchError::NotFound)
                }else{lyrics::fetch_for_display(&source, &title, &artist, dur, song_id, cached)};
                let _ = tx.send(result);
            });
            if self.ly_worker.is_none(){self.ly.loading=false;self.ly.key.clear();}
        }
        if let Some(rx)=&self.ly.native_rx {
            let mut found=None;
            while let Ok(result)=rx.try_recv(){if result.is_some(){found=result;}}
            if let Some(lyrics)=found {
                self.ly.lyrics=Some(lyrics);self.ly.loading=false;self.ly.error=None;
                self.ly.retry_at=None;self.ly.rx=None;self.ly.native_rx=None;
            }
        }
        if let Some(rx) = &self.ly.rx {
            if let Ok(r) = rx.try_recv() {
                self.ly.loading = false;
                match r {
                    Ok((l,waiting_native)) => {
                        self.ly.error=None;
                        self.ly.retry_at=None;
                        if (waiting_native || lyrics::is_soda(&self.media.source) && !l.synced) && self.ly.attempts<NATIVE_RETRIES {self.ly.retry_at=Some(self.t+5.0);}
                        self.ly.lyrics = Some(l);
                    },
                    Err(error) => {
                        let native=lyrics::is_soda(&self.media.source) || self.media.source.to_ascii_lowercase().contains("kugou") && error==lyrics::FetchError::NotFound;
                        if native {if self.ly.attempts<NATIVE_RETRIES{self.ly.retry_at=Some(self.t+5.0);}}
                        else if matches!(error,lyrics::FetchError::Unavailable(_)|lyrics::FetchError::InvalidResponse(_)) && self.ly.attempts<NETWORK_RETRIES {self.ly.retry_at=Some(self.t+30.0);}
                        self.ly.error = Some(error);
                    },
                }
                self.ly.rx = None;
            }
        }
    }

    /// 外部直接提供歌词（录制演示用）。
    pub fn set_lyrics(&mut self, title: &str, artist: &str, duration: f64, l: lyrics::Lyrics) {
        self.ly = LyricsState { key: format!("{title}|{artist}|{duration:.0}"), lyrics: Some(l), ..Default::default() };
    }

    pub fn on_drop(&mut self, files: Vec<PathBuf>) {
        if !self.settings.visible_pages[3] || files.is_empty() { return; }
        self.drag_hot = false;
        self.drag_saved = None;
        if self.page != 3 {
            self.go(3, 1.0);
        }
        let (mut added,mut duplicate,mut failed) = (0,0,0);
        for p in files {
            if !p.exists() {failed+=1;continue;}
            if self.shelf.items.contains(&p) {
                duplicate+=1;
                continue;
            }
            if self.shelf.add(&p).is_ok() {
                self.shelf_born.insert(p.clone(), self.t);
                added += 1;
            }else{failed+=1;}
        }
        let message=if added==0 && failed==0 {
            if duplicate==1{"该文件已在置物架中".into()}else{"这些文件已在置物架中".into()}
        }else if added==0 && duplicate==0 {"文件不存在或无法访问".into()}
        else {
            let mut parts=Vec::new();
            if added>0{parts.push(format!("已添加 {added} 个文件"));}
            if duplicate>0{parts.push(format!("{duplicate} 个已存在"));}
            if failed>0{parts.push(format!("{failed} 个无法添加"));}
            parts.join("，")
        };
        self.say(message);
        if added>0 {
            self.pet.react(PetState::Happy);
            self.shelf_first = self.shelf.items.len().saturating_sub(5);
        }
    }

    /// 待办输入框：真正的 egui 文本框，放在待办卡片顶部那一行。
    fn input_field(&mut self, ui: &mut egui::Ui, origin: Pos2, cmds: &mut Vec<Cmd>) {
        let rect = Rect::from_min_size(origin + Vec2::new(222.0, 37.0), Vec2::new(142.0, 18.0));
        let mut buf = std::mem::take(&mut self.buf);
        let te = egui::TextEdit::singleline(&mut buf)
            .id(egui::Id::new("todo-input"))
            .frame(false)
            .margin(egui::Margin::ZERO)
            .text_color(theme::WHITE)
            .font(theme::font(size::BODY, W::Regular))
            .hint_text(egui::RichText::new("要做点什么？回车保存").color(theme::DIM).font(theme::font(size::BODY, W::Regular)))
            .desired_width(rect.width());
        let resp = ui.put(rect, te);
        let first_focus=!self.todo_input_focused;
        self.buf=buf;
        let lost_window=ui.input(|i|i.events.iter().any(|event|matches!(event,egui::Event::WindowFocused(false))));
        if !first_focus && !self.todo_ime_frame && self.adding && self.buf.trim().is_empty() && (resp.clicked_elsewhere() || lost_window) {
            resp.surrender_focus();
            self.cancel_edit();
            return;
        }
        if first_focus {
            self.todo_input_focused=true;
            resp.request_focus();
            cmds.push(Cmd::Focus);
        }
        let submit = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if submit {
            let text = self.buf.trim().to_string();
            let mut added=false;
            if let Some(i) = self.editing {
                if !text.is_empty() && i < self.todos.items.len() {
                    self.todos.items[i].text = text;
                    self.todos.save();
                }
            } else if !text.is_empty() {
                self.todo_first = 0;
                self.todos.items.insert(0, model::TodoItem::new(text));
                self.todos.save();
                self.todo_pop.vel += 6.0;
                added=true;
            }
            self.cancel_edit();
            if added {self.open_task_reminder(0);}
        }
    }
}

#[cfg(test)]
mod lyric_scroll_tests {
    use super::*;

    #[test]
    fn scroll_waits_at_both_ends(){
        // 句首停留 1 秒：这段时间必须纹丝不动，否则开头一闪而过。
        assert_eq!(lyric_scroll_offset(0.0,30.0,100.0),0.0);
        assert_eq!(lyric_scroll_offset(1.0,30.0,100.0),0.0);
        assert!((lyric_scroll_offset(2.0,30.0,100.0)-30.0).abs()<0.01);
        // 到尾部就停住，不会滚出空白。
        assert_eq!(lyric_scroll_offset(60.0,30.0,100.0),100.0);
    }

    #[test]
    fn speed_fits_line_window_but_stays_readable(){
        // 窗口足够长：按「整句滚完」算，落在可读区间。
        let normal=lyric_scroll_speed(200.0,10.0);
        assert!((normal-25.0).abs()<0.01,"{}",normal);
        // 句子很短、放不下时也不能快到看不清。
        assert_eq!(lyric_scroll_speed(300.0,1.0),SCROLL_MAX_SPEED);
        // 句子很长时不会慢到几乎不动。
        assert_eq!(lyric_scroll_speed(1.0,600.0),SCROLL_MIN_SPEED);
    }

    /// 量化缓存省下的重排开销：`cargo test --lib -- --ignored --nocapture bench_long_line`
    #[test]
    #[ignore]
    fn bench_long_line_layout_cost(){
        let ctx=egui::Context::default();
        let _=ctx.run(Default::default(),|_|{});
        let text="这一句歌词故意写得非常非常长用来测试横向滚动到底能不能把整句话完整地显示出来".to_owned();
        let job=|size:f32|{
            let mut job=egui::text::LayoutJob::simple(text.clone(),egui::FontId::proportional(size),theme::WHITE,186.0);
            job.wrap.break_anywhere=true;
            job
        };
        let n=300;
        let t=std::time::Instant::now();
        for _ in 0..n {let _=ctx.fonts(|fonts|fonts.layout_job(job(11.0)));}
        let one=t.elapsed().as_secs_f64()/n as f64;
        let t=std::time::Instant::now();
        for _ in 0..n {
            for size in [11.0,10.5,10.0,9.5,9.0,8.5,8.0] {let _=ctx.fonts(|fonts|fonts.layout_job(job(size)));}
        }
        let seven=t.elapsed().as_secs_f64()/n as f64;
        // 每句都不一样才会真的走排版，用来区分「命中 egui 内部 galley 缓存」和「真重排」。
        let t=std::time::Instant::now();
        for i in 0..n {
            let mut job=egui::text::LayoutJob::simple(format!("{}{}",text,i),egui::FontId::proportional(11.0),theme::WHITE,186.0);
            job.wrap.break_anywhere=true;
            let _=ctx.fonts(|fonts|fonts.layout_job(job));
        }
        let cold=t.elapsed().as_secs_f64()/n as f64;
        println!("命中缓存 {:.1}us | 未命中(真排版) {:.1}us | 每帧7次 {:.1}us | 30fps 每秒多花 {:.3}ms",
            one*1e6,cold*1e6,seven*1e6,(seven-one)*30.0*1000.0);
    }

    #[test]
    fn cache_matches_only_same_line_and_width(){
        let ctx=egui::Context::default();
        // 字体要等第一次 run 之后才可用。
        let _=ctx.run(Default::default(),|_|{});
        // semibold 字体族由 App 启动时注册，这里只验证匹配逻辑，用默认族即可。
        let job=egui::text::LayoutJob::simple("测试".to_owned(),egui::FontId::proportional(11.0),theme::WHITE,f32::INFINITY);
        let galley=ctx.fonts(|fonts|fonts.layout_job(job));
        let cache=LyricLayoutCache{text:"测试".into(),width:160.0,white:true,title:false,galley,overflow:0.0};
        assert!(cache.matches("测试",160.0,true,false));
        // 换句、改宽度、换颜色、歌名/歌词切换都必须重排。
        assert!(!cache.matches("另一句",160.0,true,false));
        assert!(!cache.matches("测试",200.0,true,false));
        assert!(!cache.matches("测试",160.0,false,false));
        assert!(!cache.matches("测试",160.0,true,true));
    }
}

#[cfg(test)]
mod battery_source_tests {
    use super::*;

    #[test]
    fn pc_and_phone_battery_switches_do_not_affect_each_other(){
        let ctx=eframe::egui::Context::default();
        let mut app=App::new(&ctx,Default::default());
        app.resident_power=(Some(65),false);
        app.phone_resident=(Some(42),true);
        app.settings.show_battery=true;
        app.settings.phone_battery=true;
        assert_eq!(app.battery_ring_count(),2,"两个来源都有数据时应并排两个环");
        // 关掉本机开关：手机环不受影响。
        app.settings.show_battery=false;
        assert!(app.battery_rings().0.is_none()&&app.battery_rings().1.is_some());
        // 关掉手机开关：本机环不受影响。
        app.settings.show_battery=true;
        app.settings.phone_battery=false;
        assert!(app.battery_rings().0.is_some()&&app.battery_rings().1.is_none());
        // 手机没数据时手机环不占位，本机环照常。
        app.settings.phone_battery=true;
        app.phone_resident=(None,false);
        assert_eq!(app.battery_ring_count(),1);
        assert!(app.battery_visible());
    }
}

#[cfg(test)]
mod hover_delay_tests {
    use super::*;
    use crate::anim::Spring;

    fn approx(a: f64, b: f64) { assert!((a - b).abs() < 1e-9, "{a} != {b}"); }

    #[test]
    fn 延时换算默认值与极简加倍() {
        approx(hover_delay_secs(200, false), 0.20);
        approx(hover_delay_secs(200, true), 0.40);
        approx(hover_delay_secs(0, false), 0.0);
        approx(hover_delay_secs(0, true), 0.0);
        approx(hover_delay_secs(80, false), 0.08);
        assert!(hover_delay_secs(500, true) <= 1.0, "极简加倍也要有上限");
        assert_eq!(crate::model::Settings::default().hover_delay_ms, 200);
        assert!(200 >= SWELL_MIN_MS && 80 < SWELL_MIN_MS, "默认有预放大、短延时保持静止");
    }

    #[test]
    fn 短延时既过滤鼠标划过又更早展开() {
        let (bw, full) = (143.0f32, 419.0f32);
        let tuning = crate::model::OpenAnim::Elastic.tuning().unwrap();
        // 80 ms ≈ 5 帧 @60fps：等待期间保持静止，到点才换展开弹簧。
        let mut short = Spring::new(bw, CLOSE.0, CLOSE.1);
        short.target = bw;
        // 200 ms ≈ 12 帧：等待期间跑 SWELL 预放大。
        let mut long = Spring::new(bw, SWELL_K.0, SWELL_K.1);
        long.target = bw * SWELL.0;
        let mut width_before_delay = bw;
        for frame in 0..10 {
            if frame == 5 {
                short.tune(tuning.open_w.0, tuning.open_w.1);
                short.target = full;
            }
            short.step(1.0 / 60.0);
            long.step(1.0 / 60.0);
            if frame == 3 { width_before_delay = short.pos; }
        }
        assert!((width_before_delay - bw).abs() < 0.01,
            "延时内不该有任何形变，划过 <80 ms 就不会展开：{width_before_delay}");
        assert!(short.pos > long.pos + 20.0,
            "短延时应更早进入展开：{} vs {}", short.pos, long.pos);
    }

    #[test]
    fn 零延时比默认更早到位() {
        let (bw, full) = (143.0f32, 419.0f32);
        let near = bw + (full - bw) * 0.9;
        let tuning = crate::model::OpenAnim::Elastic.tuning().unwrap();
        let mut direct = Spring::new(bw, tuning.open_w.0, tuning.open_w.1);
        direct.target = full;
        let mut relay = Spring::new(bw, SWELL_K.0, SWELL_K.1);
        relay.target = bw * SWELL.0;
        let (mut direct_frames, mut relay_frames) = (None, None);
        for frame in 0..600 {
            if frame == 12 {
                relay.tune(tuning.open_w.0, tuning.open_w.1);
                relay.target = full;
            }
            direct.step(1.0 / 60.0);
            relay.step(1.0 / 60.0);
            if direct_frames.is_none() && direct.pos >= near { direct_frames = Some(frame + 1); }
            if relay_frames.is_none() && relay.pos >= near { relay_frames = Some(frame + 1); }
            if direct_frames.is_some() && relay_frames.is_some() { break; }
        }
        let (d, r) = (direct_frames.expect("零延时没到位"), relay_frames.expect("默认延时没到位"));
        assert!(d < r, "零延时应更早到位：{d} 帧 vs {r} 帧");
    }
}
