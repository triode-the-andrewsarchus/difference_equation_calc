use crate::query::SearchResult;
use crate::sim::SimulationResult;
use egui::{pos2, vec2, Color32, FontId, Painter, Rect, Response, Sense, Stroke, Ui};

pub const TRACE_COLORS: [Color32; 6] = [
    Color32::from_rgb(0, 220, 255),   // Cyan (a)
    Color32::from_rgb(255, 95, 175),  // Magenta / Pink (b)
    Color32::from_rgb(255, 200, 60),  // Amber / Gold (c)
    Color32::from_rgb(70, 240, 150),  // Emerald / Mint (d)
    Color32::from_rgb(180, 130, 255), // Purple
    Color32::from_rgb(255, 120, 80),  // Coral
];

#[derive(Debug, Clone)]
pub struct WaveformViewport {
    pub x_min: f64,
    pub x_max: f64,
    pub is_dragging_selection: bool,
    pub drag_start_n: Option<usize>,
}

impl Default for WaveformViewport {
    fn default() -> Self {
        Self {
            x_min: 0.0,
            x_max: 600.0,
            is_dragging_selection: false,
            drag_start_n: None,
        }
    }
}

pub struct WaveformRowInfo<'a> {
    pub name: &'a str,
    pub color: Color32,
    pub values: &'a [f64],
    pub ghost_values: Option<&'a [f64]>,
}

pub struct WaveformGridResponse {
    pub new_selection: Option<Option<(usize, usize)>>,
    pub hovered_step: Option<usize>,
}

pub fn render_waveform_grid(
    ui: &mut Ui,
    viewport: &mut WaveformViewport,
    rows: &[WaveformRowInfo],
    sim_result: &SimulationResult,
    search_result: &SearchResult,
    selection: Option<(usize, usize)>,
) -> WaveformGridResponse {
    let mut response_action = WaveformGridResponse {
        new_selection: None,
        hovered_step: None,
    };

    if rows.is_empty() {
        ui.label(egui::RichText::new("No active equations to display.").italics());
        return response_action;
    }

    let sample_count = sim_result.sample_count.max(2);
    let total_width = ui.available_width().max(200.0);
    let row_height = 100.0;
    let total_height = (rows.len() as f32) * row_height;

    let (response, painter) =
        ui.allocate_painter(vec2(total_width, total_height), Sense::click_and_drag());

    let canvas_rect = response.rect;

    // Viewport pan & zoom interaction
    handle_viewport_input(ui, &response, viewport, sample_count, canvas_rect);

    // Coordinate conversion closures
    let x_to_screen = |n: f64| -> f32 {
        let span = (viewport.x_max - viewport.x_min).max(1.0);
        let norm = (n - viewport.x_min) / span;
        canvas_rect.left() + (norm as f32) * canvas_rect.width()
    };

    let screen_to_x = |sx: f32| -> f64 {
        let norm = (sx - canvas_rect.left()) / canvas_rect.width();
        let span = viewport.x_max - viewport.x_min;
        viewport.x_min + (norm as f64) * span
    };

    // Background
    painter.rect_filled(canvas_rect, 4.0, Color32::from_rgb(18, 20, 26));

    // Draw search highlight regions (spans all rows vertically)
    for region in &search_result.regions {
        let x0 = x_to_screen(region.start as f64);
        let x1 = x_to_screen(region.end as f64);
        if x1 > canvas_rect.left() && x0 < canvas_rect.right() {
            let clamped_x0 = x0.max(canvas_rect.left());
            let clamped_x1 = x1.min(canvas_rect.right());
            let highlight_rect = Rect::from_min_max(
                pos2(clamped_x0, canvas_rect.top()),
                pos2(clamped_x1, canvas_rect.bottom()),
            );
            // Glowing amber-teal highlight for search matches
            painter.rect_filled(
                highlight_rect,
                0.0,
                Color32::from_rgba_premultiplied(0, 150, 180, 45),
            );
            // Top/bottom indicator ticks
            painter.line_segment(
                [
                    pos2(clamped_x0, canvas_rect.top()),
                    pos2(clamped_x1, canvas_rect.top()),
                ],
                Stroke::new(2.0_f32, Color32::from_rgb(0, 220, 255)),
            );
        }
    }

    // Draw selection highlight (spans all rows vertically)
    if let Some((sel_start, sel_end)) = selection {
        let x0 = x_to_screen(sel_start as f64);
        let x1 = x_to_screen((sel_end + 1) as f64);
        let clamped_x0 = x0.max(canvas_rect.left());
        let clamped_x1 = x1.min(canvas_rect.right());
        if clamped_x1 > clamped_x0 {
            let sel_rect = Rect::from_min_max(
                pos2(clamped_x0, canvas_rect.top()),
                pos2(clamped_x1, canvas_rect.bottom()),
            );
            // Selection band
            painter.rect_filled(
                sel_rect,
                0.0,
                Color32::from_rgba_premultiplied(255, 220, 50, 40),
            );
            // Selection boundary lines
            painter.line_segment(
                [
                    pos2(clamped_x0, canvas_rect.top()),
                    pos2(clamped_x0, canvas_rect.bottom()),
                ],
                Stroke::new(1.5_f32, Color32::from_rgb(255, 230, 80)),
            );
            painter.line_segment(
                [
                    pos2(clamped_x1, canvas_rect.top()),
                    pos2(clamped_x1, canvas_rect.bottom()),
                ],
                Stroke::new(1.5_f32, Color32::from_rgb(255, 230, 80)),
            );

            // Selection header badge
            let badge_text = format!(
                "n: {} .. {} (Δn = {})",
                sel_start,
                sel_end,
                sel_end.saturating_sub(sel_start) + 1
            );
            painter.text(
                pos2(clamped_x0 + 4.0, canvas_rect.top() + 4.0),
                egui::Align2::LEFT_TOP,
                badge_text,
                FontId::proportional(11.0),
                Color32::from_rgb(255, 235, 120),
            );
        }
    }

    // Grid lines for time/n
    draw_time_grid(&painter, canvas_rect, viewport, &x_to_screen);

    // Render individual waveform rows
    for (idx, row) in rows.iter().enumerate() {
        let row_top = canvas_rect.top() + (idx as f32) * row_height;
        let row_bottom = row_top + row_height;
        let row_rect = Rect::from_min_max(
            pos2(canvas_rect.left(), row_top),
            pos2(canvas_rect.right(), row_bottom),
        );

        // Separator line
        if idx > 0 {
            painter.line_segment(
                [
                    pos2(canvas_rect.left(), row_top),
                    pos2(canvas_rect.right(), row_top),
                ],
                Stroke::new(1.0_f32, Color32::from_rgb(35, 38, 48)),
            );
        }

        render_single_row(&painter, row_rect, row, viewport, &x_to_screen);
    }

    // Draw failure marker if simulation failed at some step
    if let Some(ref failure) = sim_result.failure {
        let fail_x = x_to_screen(failure.step as f64);
        if fail_x >= canvas_rect.left() && fail_x <= canvas_rect.right() {
            painter.line_segment(
                [
                    pos2(fail_x, canvas_rect.top()),
                    pos2(fail_x, canvas_rect.bottom()),
                ],
                Stroke::new(2.0_f32, Color32::from_rgb(255, 60, 60)),
            );
            painter.text(
                pos2(fail_x + 4.0, canvas_rect.bottom() - 16.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Failed at n={}: {}", failure.step, failure.message),
                FontId::proportional(11.0),
                Color32::from_rgb(255, 80, 80),
            );
        }
    }

    // Handle mouse selection drag
    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(ptr) = response.interact_pointer_pos() {
            let n_raw = screen_to_x(ptr.x).round() as i64;
            let n_clamped = n_raw.clamp(0, (sample_count - 1) as i64) as usize;
            viewport.is_dragging_selection = true;
            viewport.drag_start_n = Some(n_clamped);
        }
    }

    if viewport.is_dragging_selection {
        if let (Some(start_n), Some(ptr)) = (viewport.drag_start_n, response.interact_pointer_pos())
        {
            let n_raw = screen_to_x(ptr.x).round() as i64;
            let current_n = n_raw.clamp(0, (sample_count - 1) as i64) as usize;
            let s0 = start_n.min(current_n);
            let s1 = start_n.max(current_n);
            response_action.new_selection = Some(Some((s0, s1)));
        }

        if response.drag_stopped() {
            viewport.is_dragging_selection = false;
        }
    }

    // Click without drag clears selection
    if response.clicked_by(egui::PointerButton::Primary) && !viewport.is_dragging_selection {
        if let Some(ptr) = response.interact_pointer_pos() {
            let n_raw = screen_to_x(ptr.x).round() as i64;
            let clicked_n = n_raw.clamp(0, (sample_count - 1) as i64) as usize;
            // Point selection
            response_action.new_selection = Some(Some((clicked_n, clicked_n)));
        }
    }

    // Synchronized Cursor across ALL rows
    if let Some(ptr) = response.hover_pos() {
        if canvas_rect.contains(ptr) {
            let n_raw = screen_to_x(ptr.x).round() as i64;
            if n_raw >= 0 && (n_raw as usize) < sample_count {
                let step = n_raw as usize;
                response_action.hovered_step = Some(step);

                let cursor_x = x_to_screen(step as f64);
                // Vertical cursor line
                painter.line_segment(
                    [
                        pos2(cursor_x, canvas_rect.top()),
                        pos2(cursor_x, canvas_rect.bottom()),
                    ],
                    Stroke::new(
                        1.0_f32,
                        Color32::from_rgba_premultiplied(255, 255, 255, 180),
                    ),
                );

                // Cursor readout tooltip/overlay
                draw_cursor_readout(&painter, canvas_rect, cursor_x, ptr.y, step, rows);
            }
        }
    }

    response_action
}

fn handle_viewport_input(
    ui: &Ui,
    response: &Response,
    viewport: &mut WaveformViewport,
    sample_count: usize,
    canvas_rect: Rect,
) {
    // Zoom with scroll wheel
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scroll_delta.abs() > 0.01 {
        let zoom_factor = if scroll_delta > 0.0 { 0.85 } else { 1.15 };
        let pointer_x = response.hover_pos().map_or(canvas_rect.center().x, |p| p.x);
        let norm = (pointer_x - canvas_rect.left()) / canvas_rect.width();
        let current_span = viewport.x_max - viewport.x_min;
        let new_span = (current_span * zoom_factor).clamp(10.0, (sample_count as f64) * 2.0);

        let center_n = viewport.x_min + (norm as f64) * current_span;
        viewport.x_min = center_n - (norm as f64) * new_span;
        viewport.x_max = viewport.x_min + new_span;
    }

    // Pan with secondary mouse button or middle mouse button
    if response.dragged_by(egui::PointerButton::Secondary)
        || response.dragged_by(egui::PointerButton::Middle)
    {
        let drag_delta = response.drag_delta().x;
        let span = viewport.x_max - viewport.x_min;
        let delta_n = -(drag_delta as f64) * (span / (canvas_rect.width() as f64));
        viewport.x_min += delta_n;
        viewport.x_max += delta_n;
    }
}

fn draw_time_grid(
    painter: &Painter,
    rect: Rect,
    viewport: &WaveformViewport,
    x_to_screen: &impl Fn(f64) -> f32,
) {
    let span = viewport.x_max - viewport.x_min;
    let step_n = if span < 50.0 {
        5.0
    } else if span < 150.0 {
        10.0
    } else if span < 400.0 {
        25.0
    } else if span < 1000.0 {
        50.0
    } else {
        100.0
    };

    let start_tick = (viewport.x_min / step_n).floor() * step_n;
    let mut t = start_tick;

    while t <= viewport.x_max {
        if t >= 0.0 {
            let sx = x_to_screen(t);
            if sx >= rect.left() && sx <= rect.right() {
                painter.line_segment(
                    [pos2(sx, rect.top()), pos2(sx, rect.bottom())],
                    Stroke::new(1.0_f32, Color32::from_rgb(26, 29, 38)),
                );
                // Grid label at bottom
                painter.text(
                    pos2(sx + 2.0, rect.bottom() - 12.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:.0}", t),
                    FontId::monospace(9.0),
                    Color32::from_rgb(80, 85, 100),
                );
            }
        }
        t += step_n;
    }
}

fn render_single_row(
    painter: &Painter,
    row_rect: Rect,
    row: &WaveformRowInfo,
    viewport: &WaveformViewport,
    x_to_screen: &impl Fn(f64) -> f32,
) {
    let values = row.values;
    if values.is_empty() {
        return;
    }

    // Determine value bounds in visible range
    let start_idx = (viewport.x_min.floor().max(0.0) as usize).min(values.len());
    let end_idx = (viewport.x_max.ceil().max(0.0) as usize).min(values.len());

    let mut min_val = f64::INFINITY;
    let mut max_val = f64::NEG_INFINITY;

    for v in &values[start_idx..end_idx] {
        if v.is_finite() {
            let clamped = v.clamp(-1.0, 1.0);
            min_val = min_val.min(clamped);
            max_val = max_val.max(clamped);
        }
    }

    if !min_val.is_finite() || !max_val.is_finite() {
        min_val = -1.0;
        max_val = 1.0;
    }

    // Always clamp plot range within -1.0 .. +1.0
    let abs_peak = min_val.abs().max(max_val.abs()).clamp(0.1, 1.0);
    let plot_min = if min_val >= 0.0 && max_val <= 1.0 {
        -0.05
    } else {
        (-abs_peak * 1.05).max(-1.0)
    };
    let plot_max = (abs_peak * 1.05).min(1.0);

    let y_to_screen = |val: f64| -> f32 {
        let clamped = val.clamp(-1.0, 1.0);
        let norm = (clamped - plot_min) / (plot_max - plot_min).max(1e-6);
        row_rect.bottom() - (norm as f32) * row_rect.height()
    };

    // Zero-axis baseline
    let zero_y = y_to_screen(0.0);
    if zero_y >= row_rect.top() && zero_y <= row_rect.bottom() {
        painter.line_segment(
            [
                pos2(row_rect.left(), zero_y),
                pos2(row_rect.right(), zero_y),
            ],
            Stroke::new(1.0_f32, Color32::from_rgb(38, 42, 54)),
        );
    }

    // Variable badge / name
    painter.text(
        pos2(row_rect.left() + 8.0, row_rect.top() + 6.0),
        egui::Align2::LEFT_TOP,
        row.name,
        FontId::proportional(14.0),
        row.color,
    );

    // Min / max labels
    painter.text(
        pos2(row_rect.right() - 6.0, row_rect.top() + 4.0),
        egui::Align2::RIGHT_TOP,
        format!("+{:.2}", plot_max),
        FontId::monospace(9.0),
        Color32::from_rgb(90, 95, 110),
    );
    painter.text(
        pos2(row_rect.right() - 6.0, row_rect.bottom() - 4.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.2}", plot_min),
        FontId::monospace(9.0),
        Color32::from_rgb(90, 95, 110),
    );

    // If ghost trace exists, render it first as a faint comparison curve
    if let Some(ghost) = row.ghost_values {
        let ghost_end = end_idx.min(ghost.len());
        let ghost_start = start_idx.min(ghost_end);
        let sample_step = if (ghost_end - ghost_start) > 2000 {
            2
        } else {
            1
        };
        let mut ghost_points = Vec::new();
        let ghost_stroke = Stroke::new(
            1.2_f32,
            Color32::from_rgba_unmultiplied(row.color.r(), row.color.g(), row.color.b(), 75),
        );

        for i in (ghost_start..ghost_end).step_by(sample_step) {
            let val = ghost[i];
            if val.is_finite() {
                let sx = x_to_screen(i as f64);
                let sy = y_to_screen(val);
                ghost_points.push(pos2(sx, sy));
            } else {
                if ghost_points.len() >= 2 {
                    painter.add(egui::Shape::line(ghost_points.clone(), ghost_stroke));
                }
                ghost_points.clear();
            }
        }
        if ghost_points.len() >= 2 {
            painter.add(egui::Shape::line(ghost_points, ghost_stroke));
        }
    }

    // Build waveform polyline path
    let mut points = Vec::new();
    let sample_step = if (end_idx - start_idx) > 2000 { 2 } else { 1 };

    for i in (start_idx..end_idx).step_by(sample_step) {
        let val = values[i];
        if val.is_finite() {
            let sx = x_to_screen(i as f64);
            let sy = y_to_screen(val);
            points.push(pos2(sx, sy));
        } else {
            // Broken connection on NaN/Inf
            if points.len() >= 2 {
                painter.add(egui::Shape::line(
                    points.clone(),
                    Stroke::new(1.8_f32, row.color),
                ));
            }
            points.clear();
        }
    }

    if points.len() >= 2 {
        painter.add(egui::Shape::line(points, Stroke::new(1.8_f32, row.color)));
    }
}

fn draw_cursor_readout(
    painter: &Painter,
    canvas_rect: Rect,
    cursor_x: f32,
    cursor_y: f32,
    step: usize,
    rows: &[WaveformRowInfo],
) {
    let mut lines = Vec::new();
    lines.push(format!("n = {step}"));

    for row in rows {
        if let Some(&val) = row.values.get(step) {
            lines.push(format!("{}: {:.4}", row.name, val));
        }
    }

    let has_any_ghost = rows.iter().any(|r| r.ghost_values.is_some());
    let box_width = if has_any_ghost { 170.0 } else { 110.0 };
    let box_height = 20.0 + (rows.len() as f32) * 16.0;

    let mut box_x = cursor_x + 12.0;
    if box_x + box_width > canvas_rect.right() {
        box_x = cursor_x - box_width - 12.0;
    }
    let box_y = cursor_y.clamp(
        canvas_rect.top() + 10.0,
        canvas_rect.bottom() - box_height - 10.0,
    );

    let readout_rect = Rect::from_min_size(pos2(box_x, box_y), vec2(box_width, box_height));

    painter.rect_filled(
        readout_rect,
        6.0_f32,
        Color32::from_rgba_premultiplied(22, 25, 34, 235),
    );
    painter.rect_stroke(
        readout_rect,
        6.0_f32,
        Stroke::new(1.0_f32, Color32::from_rgb(60, 68, 85)),
        egui::StrokeKind::Outside,
    );

    // Title: n
    painter.text(
        pos2(box_x + 8.0, box_y + 6.0),
        egui::Align2::LEFT_TOP,
        format!("n = {step}"),
        FontId::monospace(11.0),
        Color32::WHITE,
    );

    for (i, row) in rows.iter().enumerate() {
        if let Some(&val) = row.values.get(step) {
            let clamped_val = val.clamp(-1.0, 1.0);
            let label = if let Some(ghost) = row.ghost_values.and_then(|g| g.get(step)) {
                format!(
                    "{}: {:.3} (ghost {:.3})",
                    row.name,
                    clamped_val,
                    ghost.clamp(-1.0, 1.0)
                )
            } else {
                format!("{}: {:.4}", row.name, clamped_val)
            };

            painter.text(
                pos2(box_x + 8.0, box_y + 22.0 + (i as f32) * 15.0),
                egui::Align2::LEFT_TOP,
                label,
                FontId::monospace(10.5),
                row.color,
            );
        }
    }
}
