use crate::query::SearchResult;
use crate::sim::SimulationResult;
use crate::ui::format::format_compact_value;
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
    pub is_dragging_init_cond: Option<(String, usize)>,
    pub is_goal_seeking: bool,
}

impl Default for WaveformViewport {
    fn default() -> Self {
        Self {
            x_min: 0.0,
            x_max: 600.0,
            is_dragging_selection: false,
            drag_start_n: None,
            is_dragging_init_cond: None,
            is_goal_seeking: false,
        }
    }
}

pub struct WaveformRowInfo<'a> {
    pub name: &'a str,
    pub color: Color32,
    pub values: &'a [f64],
    pub ghost_values: Option<&'a [f64]>,
    pub family_traces: Vec<&'a [f64]>,
    pub clamp_threshold: Option<f64>,
    pub is_clamped: bool,
    pub max_peak: f64,
}

#[derive(Debug, Clone)]
pub struct GoalSeekAction {
    pub target_var: String,
    pub target_step: usize,
    pub target_value: f64,
}

pub struct WaveformGridResponse {
    pub new_selection: Option<Option<(usize, usize)>>,
    pub hovered_step: Option<usize>,
    pub new_initial_condition: Option<(String, f64)>,
    pub goal_seek: Option<GoalSeekAction>,
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
        new_initial_condition: None,
        goal_seek: None,
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

    // Helper closures for row geometry
    let get_row_index = |py: f32| -> Option<usize> {
        if py < canvas_rect.top() || py > canvas_rect.bottom() {
            return None;
        }
        let idx = ((py - canvas_rect.top()) / row_height).floor() as usize;
        if idx < rows.len() {
            Some(idx)
        } else {
            None
        }
    };

    let get_row_rect = |idx: usize| -> Rect {
        let top = canvas_rect.top() + (idx as f32) * row_height;
        Rect::from_min_max(
            pos2(canvas_rect.left(), top),
            pos2(canvas_rect.right(), top + row_height),
        )
    };

    let (vx_min, vx_max) = (viewport.x_min, viewport.x_max);
    let screen_to_row_val = |r_idx: usize, sy: f32| -> f64 {
        let r_rect = get_row_rect(r_idx);
        let row_info = &rows[r_idx];
        let (p_min, p_max) =
            get_row_bounds(row_info.values, vx_min, vx_max, row_info.clamp_threshold);
        let norm = ((r_rect.bottom() - sy) / r_rect.height()).clamp(0.0, 1.0);
        let raw = p_min + (norm as f64) * (p_max - p_min);
        if let Some(th) = row_info.clamp_threshold {
            raw.clamp(-th, th)
        } else {
            raw
        }
    };

    // Handle mouse drag start: Alt+drag (Goal-Seek), Near n=0 (Initial Condition), or Range Selection
    let alt_down = ui.input(|i| i.modifiers.alt);
    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(ptr) = response.interact_pointer_pos() {
            if alt_down {
                viewport.is_goal_seeking = true;
            } else {
                let x0 = x_to_screen(0.0);
                if (ptr.x - x0).abs() <= 16.0 {
                    if let Some(r_idx) = get_row_index(ptr.y) {
                        viewport.is_dragging_init_cond =
                            Some((rows[r_idx].name.to_string(), r_idx));
                    }
                } else {
                    let n_raw = screen_to_x(ptr.x).round() as i64;
                    let n_clamped = n_raw.clamp(0, (sample_count - 1) as i64) as usize;
                    viewport.is_dragging_selection = true;
                    viewport.drag_start_n = Some(n_clamped);
                }
            }
        }
    }

    if let Some((ref var_name, r_idx)) = viewport.is_dragging_init_cond.clone() {
        if let Some(ptr) = response.interact_pointer_pos() {
            let new_val = screen_to_row_val(r_idx, ptr.y);
            response_action.new_initial_condition = Some((var_name.clone(), new_val));

            // HUD overlay at n=0
            let x0 = x_to_screen(0.0);
            painter.circle_filled(pos2(x0, ptr.y), 5.0, Color32::from_rgb(80, 240, 160));
            painter.circle_stroke(
                pos2(x0, ptr.y),
                9.0,
                Stroke::new(1.5_f32, Color32::from_rgb(255, 255, 255)),
            );
            painter.text(
                pos2(x0 + 12.0, ptr.y - 10.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}[0] = {}", var_name, format_compact_value(new_val)),
                FontId::monospace(11.0),
                Color32::from_rgb(80, 240, 160),
            );
        }
        if response.drag_stopped() {
            viewport.is_dragging_init_cond = None;
        }
    } else if viewport.is_goal_seeking {
        if let Some(ptr) = response.interact_pointer_pos() {
            if let Some(r_idx) = get_row_index(ptr.y) {
                let n_raw = screen_to_x(ptr.x).round() as i64;
                let target_step = n_raw.clamp(0, (sample_count - 1) as i64) as usize;
                let target_var = rows[r_idx].name.to_string();
                let target_value = screen_to_row_val(r_idx, ptr.y);
                response_action.goal_seek = Some(GoalSeekAction {
                    target_var: target_var.clone(),
                    target_step,
                    target_value,
                });

                // Draw crosshair indicator on canvas
                let sx = x_to_screen(target_step as f64);
                let sy = ptr.y;
                painter.line_segment(
                    [pos2(sx - 12.0, sy), pos2(sx + 12.0, sy)],
                    Stroke::new(1.5_f32, Color32::from_rgb(255, 200, 50)),
                );
                painter.line_segment(
                    [pos2(sx, sy - 12.0), pos2(sx, sy + 12.0)],
                    Stroke::new(1.5_f32, Color32::from_rgb(255, 200, 50)),
                );
                painter.circle_stroke(
                    pos2(sx, sy),
                    8.0,
                    Stroke::new(1.5_f32, Color32::from_rgb(255, 200, 50)),
                );
                painter.text(
                    pos2(sx + 14.0, sy - 12.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!(
                        "Goal: {}[{}] = {}",
                        target_var,
                        target_step,
                        format_compact_value(target_value)
                    ),
                    FontId::monospace(11.0),
                    Color32::from_rgb(255, 220, 80),
                );
            }
        }
        if response.drag_stopped() {
            viewport.is_goal_seeking = false;
        }
    } else if viewport.is_dragging_selection {
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
    if response.clicked_by(egui::PointerButton::Primary)
        && !viewport.is_dragging_selection
        && viewport.is_dragging_init_cond.is_none()
        && !viewport.is_goal_seeking
    {
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

pub fn get_row_bounds(
    values: &[f64],
    x_min: f64,
    x_max: f64,
    clamp_threshold: Option<f64>,
) -> (f64, f64) {
    let start_idx = (x_min.floor().max(0.0) as usize).min(values.len());
    let end_idx = (x_max.ceil().max(0.0) as usize).min(values.len());

    let mut min_val = f64::INFINITY;
    let mut max_val = f64::NEG_INFINITY;

    for v in &values[start_idx..end_idx] {
        if v.is_finite() {
            let val = if let Some(th) = clamp_threshold {
                v.clamp(-th, th)
            } else {
                *v
            };
            min_val = min_val.min(val);
            max_val = max_val.max(val);
        }
    }

    if !min_val.is_finite() || !max_val.is_finite() {
        min_val = -1.0;
        max_val = 1.0;
    }

    if let Some(th) = clamp_threshold {
        let abs_peak = min_val.abs().max(max_val.abs()).max(th * 0.2);
        let plot_min = if min_val >= 0.0 && max_val <= th {
            -th * 0.05
        } else {
            (-abs_peak * 1.08).max(-th * 1.08)
        };
        let plot_max = (abs_peak * 1.08).min(th * 1.08);
        (plot_min, plot_max)
    } else {
        // Natural dynamic range with breathing room
        let abs_peak = min_val.abs().max(max_val.abs()).max(0.1);
        let plot_min = if min_val >= 0.0 {
            -abs_peak * 0.05
        } else {
            -abs_peak * 1.12
        };
        let plot_max = abs_peak * 1.12;
        (plot_min, plot_max)
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

    let start_idx = (viewport.x_min.floor().max(0.0) as usize).min(values.len());
    let end_idx = (viewport.x_max.ceil().max(0.0) as usize).min(values.len());

    let (plot_min, plot_max) =
        get_row_bounds(values, viewport.x_min, viewport.x_max, row.clamp_threshold);

    let y_to_screen = |val: f64| -> f32 {
        let actual = if let Some(th) = row.clamp_threshold {
            val.clamp(-th, th)
        } else {
            val
        };
        let norm = (actual - plot_min) / (plot_max - plot_min).max(1e-6);
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

    // Rail Lines & Clamping Visual Feedback
    if let Some(th) = row.clamp_threshold {
        let top_rail_y = y_to_screen(th);
        let bottom_rail_y = y_to_screen(-th);

        let rail_stroke = if row.is_clamped {
            Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(255, 70, 70, 210))
        } else {
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(85, 95, 115, 80))
        };

        if top_rail_y >= row_rect.top() && top_rail_y <= row_rect.bottom() {
            painter.line_segment(
                [
                    pos2(row_rect.left(), top_rail_y),
                    pos2(row_rect.right(), top_rail_y),
                ],
                rail_stroke,
            );
            if row.is_clamped {
                painter.text(
                    pos2(row_rect.right() - 6.0, top_rail_y + 2.0),
                    egui::Align2::RIGHT_TOP,
                    format!("+{} [RAIL]", format_compact_value(th)),
                    FontId::monospace(8.5),
                    Color32::from_rgb(255, 90, 90),
                );
            }
        }

        if bottom_rail_y >= row_rect.top() && bottom_rail_y <= row_rect.bottom() {
            painter.line_segment(
                [
                    pos2(row_rect.left(), bottom_rail_y),
                    pos2(row_rect.right(), bottom_rail_y),
                ],
                rail_stroke,
            );
            if row.is_clamped {
                painter.text(
                    pos2(row_rect.right() - 6.0, bottom_rail_y - 2.0),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("-{} [RAIL]", format_compact_value(th)),
                    FontId::monospace(8.5),
                    Color32::from_rgb(255, 90, 90),
                );
            }
        }
    }

    // Variable badge / name
    painter.text(
        pos2(row_rect.left() + 8.0, row_rect.top() + 6.0),
        egui::Align2::LEFT_TOP,
        row.name,
        FontId::proportional(14.0),
        row.color,
    );

    // Visual badge showing clamping activity
    if row.is_clamped {
        let badge_rect = Rect::from_min_size(
            pos2(row_rect.left() + 32.0, row_rect.top() + 6.0),
            vec2(138.0, 16.0),
        );
        painter.rect_filled(
            badge_rect,
            3.0_f32,
            Color32::from_rgba_premultiplied(160, 30, 30, 220),
        );
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("CLAMPED (Peak: {})", format_compact_value(row.max_peak)),
            FontId::monospace(9.5),
            Color32::from_rgb(255, 230, 230),
        );
    }

    // Min / max labels
    painter.text(
        pos2(row_rect.right() - 6.0, row_rect.top() + 4.0),
        egui::Align2::RIGHT_TOP,
        format!("+{}", format_compact_value(plot_max)),
        FontId::monospace(9.0),
        Color32::from_rgb(90, 95, 110),
    );
    painter.text(
        pos2(row_rect.right() - 6.0, row_rect.bottom() - 4.0),
        egui::Align2::RIGHT_BOTTOM,
        format_compact_value(plot_min),
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

    // Parameter Family / Ensemble ribbon traces
    if !row.family_traces.is_empty() {
        let total_fam = row.family_traces.len().max(1);
        for (f_idx, f_trace) in row.family_traces.iter().enumerate() {
            let f_end = end_idx.min(f_trace.len());
            let f_start = start_idx.min(f_end);
            let sample_step = if (f_end - f_start) > 2000 { 2 } else { 1 };
            let alpha = 35 + ((f_idx * 45) / total_fam) as u8;
            let fam_stroke = Stroke::new(
                1.0_f32,
                Color32::from_rgba_unmultiplied(row.color.r(), row.color.g(), row.color.b(), alpha),
            );

            let mut f_points = Vec::new();
            for i in (f_start..f_end).step_by(sample_step) {
                let val = f_trace[i];
                if val.is_finite() {
                    let sx = x_to_screen(i as f64);
                    let sy = y_to_screen(val);
                    f_points.push(pos2(sx, sy));
                } else {
                    if f_points.len() >= 2 {
                        painter.add(egui::Shape::line(f_points.clone(), fam_stroke));
                    }
                    f_points.clear();
                }
            }
            if f_points.len() >= 2 {
                painter.add(egui::Shape::line(f_points, fam_stroke));
            }
        }
    }

    // Build waveform polyline path with clipped peaks highlighted
    let mut points = Vec::new();
    let mut clipped_points = Vec::new();
    let sample_step = if (end_idx - start_idx) > 2000 { 2 } else { 1 };

    for i in (start_idx..end_idx).step_by(sample_step) {
        let val = values[i];
        if val.is_finite() {
            let sx = x_to_screen(i as f64);
            let sy = y_to_screen(val);
            let pt = pos2(sx, sy);
            points.push(pt);

            if let Some(th) = row.clamp_threshold {
                if val.abs() >= th - 1e-4 {
                    clipped_points.push(pt);
                }
            }
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

    // Highlight clipped peaks with red indicator dots
    for pt in clipped_points {
        painter.circle_filled(pt, 2.5_f32, Color32::from_rgb(255, 60, 60));
    }

    // Draggable initial condition handle at n = 0
    let x0 = x_to_screen(0.0);
    if x0 >= row_rect.left() - 8.0 && x0 <= row_rect.right() + 8.0 && !values.is_empty() {
        let y0 = y_to_screen(values[0]);
        let handle_pos = pos2(x0, y0);
        painter.circle_stroke(
            handle_pos,
            5.5,
            Stroke::new(1.2_f32, Color32::from_rgb(255, 230, 80)),
        );
        painter.circle_filled(handle_pos, 3.5, Color32::from_rgb(70, 240, 150));
        painter.text(
            pos2(x0 + 7.0, y0 - 6.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{}[0]", row.name),
            FontId::monospace(9.0),
            Color32::from_rgb(180, 220, 200),
        );
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
            lines.push(format!("{}: {}", row.name, format_compact_value(val)));
        }
    }

    let has_any_ghost = rows.iter().any(|r| r.ghost_values.is_some());
    let box_width = if has_any_ghost { 170.0 } else { 120.0 };
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
            let is_pt_clamped = if let Some(th) = row.clamp_threshold {
                val.abs() >= th - 1e-4
            } else {
                false
            };

            let label = if is_pt_clamped {
                format!("{}: {} [CLAMPED]", row.name, format_compact_value(val))
            } else if let Some(ghost) = row.ghost_values.and_then(|g| g.get(step)) {
                format!(
                    "{}: {} (ghost {})",
                    row.name,
                    format_compact_value(val),
                    format_compact_value(*ghost)
                )
            } else {
                format!("{}: {}", row.name, format_compact_value(val))
            };

            let color = if is_pt_clamped {
                Color32::from_rgb(255, 90, 90)
            } else {
                row.color
            };

            painter.text(
                pos2(box_x + 8.0, box_y + 22.0 + (i as f32) * 15.0),
                egui::Align2::LEFT_TOP,
                label,
                FontId::monospace(10.5),
                color,
            );
        }
    }
}
