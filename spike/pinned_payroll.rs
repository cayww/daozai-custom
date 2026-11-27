use crate::{
    paint::{Card, H},
    plugin_canvas::live::Counter,
    plugin_canvas_ui::CanvasTheme,
    skin_paint,
    theme::{self, W},
};
use chrono::{DateTime, Utc};
use eframe::egui::{self, Color32};
use std::time::Duration;

const FLIP_SECONDS: f64 = 0.2;
const FLIP_FRAME: Duration = Duration::from_millis(33);
const TOP_SIZE: f32 = 12.0;
const ICON_ROOM: f32 = 25.0;
const END_MARGIN: f32 = 4.0;
const GREEN: Color32 = Color32::from_rgb(61, 220, 132);
const COIN_INK: Color32 = Color32::from_rgb(17, 33, 24);

#[derive(Clone)]
struct Digits {
    current: String,
    previous: String,
    changed_at: f64,
    last_pass: u64,
}

pub(super) fn width(ui: &egui::Ui, counter: &Counter, now: DateTime<Utc>) -> f32 {
    let c = Card::new(ui, egui::Pos2::ZERO, ui.clip_rect());
    let measure = |second| {
        let value = counter.sample(second).0.clamp(0.0, 10_000_000.0);
        c.text_w(&format!("{value:.2}"), TOP_SIZE, W::Bold)
    };
    ICON_ROOM + measure(now.timestamp()).max(measure(now.timestamp() + 1)) + END_MARGIN
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    c: &Card<'_>,
    x: f32,
    y: f32,
    width: f32,
    vertical: bool,
    theme: CanvasTheme,
    counter: &Counter,
    now: DateTime<Utc>,
) -> (Option<Duration>, bool) {
    let (value, second) = counter.sample(now.timestamp());
    let amount = format!("{value:.2}");
    let height = if vertical { 42.0 } else { 22.0 };
    let left = x - width / 2.0;
    if !c
        .rect(left, y - height / 2.0, width, height)
        .intersect(c.painter.clip_rect())
        .is_positive()
    {
        return (None, false);
    }
    let ctx = c.ui.ctx();
    let time = c.ui.input(|input| input.time);
    let pass = ctx.cumulative_pass_nr();
    let id = c.ui.make_persistent_id(("live-pay-digits", vertical));
    let (previous, progress) = if c.live && c.ui.style().animation_time > 0.0 {
        ctx.data_mut(|data| {
            let digits = data.get_temp_mut_or_insert_with(id, || Digits {
                current: amount.clone(),
                previous: amount.clone(),
                changed_at: time,
                last_pass: pass,
            });
            if pass > digits.last_pass.saturating_add(1) {
                // 重新出现时从当前金额开始。
                digits.current.clone_from(&amount);
                digits.previous.clone_from(&amount);
            } else if digits.current != amount {
                digits.previous = std::mem::replace(&mut digits.current, amount.clone());
                digits.changed_at = time;
            }
            digits.last_pass = pass;
            let elapsed = time - digits.changed_at;
            if digits.previous != digits.current && (0.0..FLIP_SECONDS).contains(&elapsed) {
                let t = (elapsed / FLIP_SECONDS) as f32;
                (Some(digits.previous.clone()), t * t * (3.0 - 2.0 * t))
            } else {
                (None, 1.0)
            }
        })
    } else {
        (None, 1.0)
    };

    let c = c.clipped(left, y - height / 2.0, width, height);
    let ink = theme.palette.map_or(theme::WHITE, |palette| {
        skin_paint::map_color(theme::WHITE, &palette)
    });
    let row = |text: &str, offset: usize, anchor: f32, baseline: f32, size: f32, align: H| {
        let Some(before) = &previous else {
            c.text(anchor, baseline, text, size, W::Bold, ink, align);
            return;
        };
        let galley = c.galley(text, size, W::Bold, ink);
        let run_width = galley.size().x / c.s;
        let left = anchor
            - match align {
                H::Left => 0.0,
                H::Center => run_width / 2.0,
                H::Right => run_width,
            };
        for (index, glyph) in galley.rows.iter().flat_map(|row| &row.glyphs).enumerate() {
            let digit = glyph.chr;
            let x = left + glyph.pos.x / c.s;
            let from_right = amount.len() - offset - index;
            let old = before
                .len()
                .checked_sub(from_right)
                .map(|pos| char::from(before.as_bytes()[pos]))
                .filter(char::is_ascii_digit);
            let draw_digit = |card: &Card<'_>, digit: char, baseline: f32| {
                let mut buffer = [0; 4];
                card.text(
                    x,
                    baseline,
                    digit.encode_utf8(&mut buffer),
                    size,
                    W::Bold,
                    ink,
                    H::Left,
                );
            };
            if digit.is_ascii_digit() && old != Some(digit) {
                let ascent = glyph.font_ascent / c.s;
                let h = glyph.font_height / c.s;
                let cell = c.clipped(x, baseline - ascent, glyph.advance_width / c.s, h);
                if let Some(old) = old {
                    draw_digit(&cell, old, baseline - h * progress);
                }
                draw_digit(&cell, digit, baseline + h * (1.0 - progress));
            } else {
                draw_digit(&c, digit, baseline);
            }
        }
    };

    if vertical {
        let coin = || {
            c.circle(x, y - 13.0, 5.0, GREEN);
            c.text(x, y - 10.0, "¥", 7.5, W::Bold, COIN_INK, H::Center);
        };
        if let Some(size) = [14.0, 13.0, 12.0, 11.0]
            .into_iter()
            .find(|size| c.text_w(&amount, *size, W::Bold) <= width - 2.0)
        {
            coin();
            row(&amount, 0, x, y + 10.0, size, H::Center);
        } else {
            let whole_len = amount.len() - 3;
            let cut = whole_len.saturating_sub(4);
            if cut == 0 {
                coin();
            } else {
                row(&amount[..cut], 0, x, y - 9.0, 12.0, H::Center);
            }
            row(&amount[cut..whole_len], cut, x, y + 4.0, 14.0, H::Center);
            row(
                &amount[whole_len..],
                whole_len,
                x,
                y + 17.0,
                11.0,
                H::Center,
            );
        }
    } else {
        c.circle(left + 10.0, y, 7.5, GREEN);
        c.text(
            left + 10.0,
            y + 4.0,
            "¥",
            10.5,
            W::Bold,
            COIN_INK,
            H::Center,
        );
        row(
            &amount,
            0,
            left + width - END_MARGIN,
            y + 5.0,
            TOP_SIZE,
            H::Right,
        );
    }

    let animating = previous.is_some();
    let repaint = if !c.live {
        None
    } else if animating {
        Some(FLIP_FRAME)
    } else {
        second
            .map(|next| Duration::from_millis((next * 1000 - now.timestamp_millis()).max(1) as u64))
    };
    (repaint, animating)
}
