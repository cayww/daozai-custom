const PAYROLL_SAMPLE_AT: i64 = 1_795_500_300;

fn payroll_display(start_value: f64, per_second: f64) -> PinnedDisplay {
    let pinned: Pinned = serde_json::from_value(json!({
        "canvas": {
            "width": 110.0, "fit_width": true, "height": 22.0,
            "binds": {"cash": {"counter": {
                "start_value": start_value, "per_second": per_second,
                "start_at": PAYROLL_SAMPLE_AT, "decimals": 2,
            }}},
            "nodes": [
                {"circle": [10, 11, 7.5], "fill": "#3DDC84"},
                {"text": "¥", "at": [10, 15], "size": 10.5,
                    "fill": "#112118", "align": "center", "weight": 800},
                {"text": "{cash}", "at": [25, 16], "size": 12,
                    "fill": "$text", "max_width": 83, "roll": true, "weight": 800},
            ]
        }
    }))
    .unwrap();
    let mut display = pinned
        .display(chrono::DateTime::from_timestamp(PAYROLL_SAMPLE_AT, 0).unwrap())
        .0;
    display.plugin = "daozai.live-pay".into();
    display
}

fn payroll_frame(
    ctx: &egui::Context,
    display: &PinnedDisplay,
    slot: egui::Rect,
    vertical: bool,
    time: f64,
) -> (egui::FullOutput, Option<Duration>, bool) {
    let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(144.0, 54.0));
    let theme = CanvasTheme {
        palette: None,
        accent: None,
        background: egui::Color32::BLACK,
    };
    let now = chrono::DateTime::from_timestamp_millis(
        PAYROLL_SAMPLE_AT * 1000 + (time * 1000.0).round() as i64,
    )
    .unwrap();
    let mut repaint_after = None;
    let mut animating = false;
    let frame = ctx.run(
        egui::RawInput {
            screen_rect: Some(bounds),
            time: Some(time),
            predicted_dt: 0.0,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let card = crate::paint::Card::new(ui, egui::Pos2::ZERO, bounds);
                    let (clicked, next, rolling) = display.draw_at(
                        &card,
                        slot.center().x,
                        slot.center().y,
                        slot.width(),
                        vertical,
                        theme,
                        now,
                    );
                    assert!(!clicked);
                    repaint_after = next;
                    animating = rolling;
                });
        },
    );
    (frame, repaint_after, animating)
}

fn payroll_numbers(
    frame: &egui::FullOutput,
) -> impl Iterator<Item = (&egui::epaint::TextShape, egui::Rect)> {
    frame.shapes.iter().filter_map(|shape| match &shape.shape {
        egui::Shape::Text(text)
            if !text.galley.text().is_empty()
                && text
                    .galley
                    .text()
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '.') =>
        {
            Some((text, shape.clip_rect))
        }
        _ => None,
    })
}

fn capture_payroll_pinned(
    ctx: &egui::Context,
    name: &str,
    first: &egui::FullOutput,
    frame: &egui::FullOutput,
) {
    let Some(path) = std::env::var_os("DAOZAI_CANVAS_SCREENSHOTS") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    std::fs::create_dir_all(&path).unwrap();
    let mut raster = crate::preview_raster::Raster::new(144, 54);
    raster.update_textures(&first.textures_delta);
    raster.update_textures(&frame.textures_delta);
    raster.buf.fill([0.015, 0.015, 0.020, 1.0]);
    let primitives = ctx.tessellate(frame.shapes.clone(), frame.pixels_per_point);
    raster.draw(&primitives, frame.pixels_per_point);
    let mut rgba = Vec::new();
    raster.to_rgba8(&mut rgba);
    image::RgbaImage::from_raw(144, 54, rgba)
        .unwrap()
        .save(path.join(name))
        .unwrap();
}

fn payroll_glyphs(frame: &egui::FullOutput) -> Vec<(char, egui::Pos2)> {
    payroll_numbers(frame)
        .flat_map(|(text, _)| {
            text.galley.rows.iter().flat_map(move |row| {
                row.glyphs
                    .iter()
                    .map(move |glyph| (glyph.chr, text.pos + glyph.pos.to_vec2()))
            })
        })
        .collect()
}

fn payroll_stable_rows(
    frame: &egui::FullOutput,
    slot: egui::Rect,
    min_size: f32,
) -> Vec<&egui::epaint::TextShape> {
    assert!(
        !frame.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect)
            if rect.fill != egui::Color32::TRANSPARENT && slot.contains_rect(rect.rect))
        }),
        "金额文字无分格底"
    );
    let mut rows = Vec::new();
    for (text, clip) in payroll_numbers(frame) {
        let painted = text.galley.mesh_bounds.translate(text.pos.to_vec2());
        assert!(
            slot.expand(0.5).contains_rect(painted),
            "{painted:?} 超出 {slot:?}"
        );
        assert!(
            clip.expand(0.5).contains_rect(painted),
            "{painted:?} 超出裁剪 {clip:?}"
        );
        assert!(text.galley.job.sections[0].format.font_id.size >= min_size);
        assert_eq!(text.galley.rows.len(), 1);
        rows.push(text);
    }
    rows.sort_by(|a, b| a.pos.y.total_cmp(&b.pos.y));
    rows
}

fn payroll_coin(frame: &egui::FullOutput, slot: egui::Rect) -> egui::Pos2 {
    let green = egui::Color32::from_rgb(61, 220, 132);
    let coins: Vec<_> = frame
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) if circle.fill == green && slot.contains(circle.center) => {
                Some(circle.center)
            }
            _ => None,
        })
        .collect();
    assert_eq!(coins.len(), 1, "应保留绿色圆形币标");
    assert!(frame
        .shapes
        .iter()
        .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "¥")));
    coins[0]
}

fn payroll_amount_row<'a>(
    frame: &'a egui::FullOutput,
    slot: egui::Rect,
    amount: &str,
    min_size: f32,
) -> &'a egui::epaint::TextShape {
    let rows = payroll_stable_rows(frame, slot, min_size);
    assert_eq!(
        rows.iter()
            .map(|text| text.galley.text())
            .collect::<Vec<_>>(),
        [amount]
    );
    let glyphs = payroll_glyphs(frame);
    let y = glyphs[0].1.y;
    assert!(
        glyphs
            .iter()
            .all(|(_, position)| (position.y - y).abs() <= 0.5),
        "小数点与每位数字共用基线"
    );
    rows[0]
}

fn payroll_roll_column(
    frame: &egui::FullOutput,
    settled: &egui::FullOutput,
    index: usize,
    old: char,
    new: char,
) {
    let stable = payroll_glyphs(settled)[index].1;
    let column: Vec<_> = payroll_glyphs(frame)
        .into_iter()
        .filter(|(_, position)| (position.x - stable.x).abs() <= 0.75)
        .collect();
    assert!(
        column
            .iter()
            .any(|(ch, position)| *ch == old && position.y < stable.y - 0.5),
        "变动列的旧数字应向上推出：{column:?}"
    );
    assert!(
        column
            .iter()
            .any(|(ch, position)| *ch == new && position.y > stable.y + 0.5),
        "变动列的新数字应从下方进入：{column:?}"
    );
}

#[test]
fn live_pay_horizontal_pinned_keeps_coin_and_aligned_rolling_amount() {
    let display = payroll_display(98.98, 1.01);
    for (y, name) in [
        (12.0, "live-pay-pinned-top.png"),
        (42.0, "live-pay-pinned-bottom.png"),
    ] {
        let ctx = egui::Context::default();
        crate::theme::load_fonts(&ctx);
        let slot = egui::Rect::from_center_size(egui::pos2(72.0, y), egui::vec2(110.0, 22.0));
        let (warm, _, _) = payroll_frame(&ctx, &display, slot, false, 0.0);
        let (first, next, animating) = payroll_frame(&ctx, &display, slot, false, 0.02);
        assert!(!animating);
        assert!(next.is_some());
        let number = payroll_amount_row(&first, slot, "98.98", 12.0);
        let coin = payroll_coin(&first, slot);
        let painted = number.galley.mesh_bounds.translate(number.pos.to_vec2());
        assert!(coin.x < painted.left());
        assert!(
            (coin.y - painted.center().y).abs() <= 1.0,
            "币标和金额上下居中"
        );
        capture_payroll_pinned(&ctx, name, &warm, &first);

        let _ = payroll_frame(&ctx, &display, slot, false, 1.0);
        let (first_roll, _, animating) = payroll_frame(&ctx, &display, slot, false, 1.1);
        assert!(animating);
        let (first_settled, _, animating) = payroll_frame(&ctx, &display, slot, false, 1.3);
        assert!(!animating);
        payroll_amount_row(&first_settled, slot, "99.99", 12.0);
        payroll_roll_column(&first_roll, &first_settled, 1, '8', '9');
        let stable = payroll_glyphs(&first_settled);
        let rolling = payroll_glyphs(&first_roll);
        for index in [0, 2] {
            let (expected, fixed) = stable[index];
            assert!(
                rolling
                    .iter()
                    .any(|(ch, position)| *ch == expected && position.distance(fixed) <= 0.75),
                "未变的数字与小数点保持基线和横坐标"
            );
        }

        let _ = payroll_frame(&ctx, &display, slot, false, 2.0);
        let (second_roll, _, animating) = payroll_frame(&ctx, &display, slot, false, 2.1);
        assert!(animating);
        let (settled, _, animating) = payroll_frame(&ctx, &display, slot, false, 2.3);
        assert!(!animating);
        payroll_amount_row(&settled, slot, "101.00", 12.0);
        payroll_roll_column(&second_roll, &settled, 5, '9', '0');
        let before = payroll_glyphs(&first_settled);
        let after = payroll_glyphs(&settled);
        for index in 1..=3 {
            let old = before[before.len() - index].1;
            let new = after[after.len() - index].1;
            assert!(
                (old.x - new.x).abs() <= 0.75,
                "进位时尾数列应右对齐：{old:?} → {new:?}"
            );
        }

        for time in [2.5, 3.5, 4.5] {
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(time),
                    ..Default::default()
                },
                |_| {},
            );
        }
        let (reopened, _, animating) = payroll_frame(&ctx, &display, slot, false, 5.0);
        assert!(!animating, "重新显示时从当前金额起步");
        payroll_amount_row(&reopened, slot, "104.03", 12.0);
    }
}

#[test]
fn live_pay_side_pinned_keeps_short_amount_on_one_line_and_rolls() {
    let display = payroll_display(81.54, 0.46);
    for (x, name) in [
        (18.0, "live-pay-pinned-left.png"),
        (126.0, "live-pay-pinned-right.png"),
    ] {
        let ctx = egui::Context::default();
        crate::theme::load_fonts(&ctx);
        let slot = egui::Rect::from_center_size(egui::pos2(x, 27.0), egui::vec2(36.0, 42.0));
        let (warm, _, _) = payroll_frame(&ctx, &display, slot, true, 0.0);
        let (first, _, animating) = payroll_frame(&ctx, &display, slot, true, 0.02);
        assert!(!animating);
        let number = payroll_amount_row(&first, slot, "81.54", 11.0);
        let coin = payroll_coin(&first, slot);
        let painted = number.galley.mesh_bounds.translate(number.pos.to_vec2());
        assert!(coin.y < painted.top() && (coin.x - painted.center().x).abs() <= 1.0);
        capture_payroll_pinned(&ctx, name, &warm, &first);

        let _ = payroll_frame(&ctx, &display, slot, true, 1.0);
        let (rolling, _, animating) = payroll_frame(&ctx, &display, slot, true, 1.1);
        assert!(animating);
        let (settled, _, animating) = payroll_frame(&ctx, &display, slot, true, 1.3);
        assert!(!animating);
        payroll_amount_row(&settled, slot, "82.00", 11.0);
        payroll_roll_column(&rolling, &settled, 1, '1', '2');
    }
}

#[test]
fn live_pay_side_pinned_keeps_every_digit_when_large_amounts_need_multiple_lines() {
    for (amount, expected_rows) in [
        (1234.56, &["1234", ".56"][..]),
        (123456.78, &["12", "3456", ".78"][..]),
        (10000000.00, &["1000", "0000", ".00"][..]),
    ] {
        let display = payroll_display(amount, 0.0);
        for (x, side) in [(18.0, "left"), (126.0, "right")] {
            let ctx = egui::Context::default();
            crate::theme::load_fonts(&ctx);
            let slot = egui::Rect::from_center_size(egui::pos2(x, 27.0), egui::vec2(36.0, 42.0));
            let (warm, _, _) = payroll_frame(&ctx, &display, slot, true, 0.0);
            let (frame, _, animating) = payroll_frame(&ctx, &display, slot, true, 0.02);
            assert!(!animating);
            let rows = payroll_stable_rows(&frame, slot, 11.0);
            assert_eq!(
                rows.iter()
                    .map(|text| text.galley.text())
                    .collect::<Vec<_>>(),
                expected_rows
            );
            let painted: Vec<_> = rows
                .iter()
                .map(|text| text.galley.mesh_bounds.translate(text.pos.to_vec2()))
                .collect();
            assert!(
                painted
                    .windows(2)
                    .all(|pair| pair[0].bottom() <= pair[1].top()),
                "数字各行互不重叠：{painted:?}"
            );
            if amount >= 10000.0 {
                let name = format!("live-pay-pinned-{side}-{}.png", amount as u32);
                capture_payroll_pinned(&ctx, &name, &warm, &frame);
            }
        }
    }
}
