use crate::ast::Equation;
use crate::sim::{compute_phase_field, SimulationResult};
use crate::ui::format::format_compact_value;
use egui::{pos2, vec2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PhaseViewState {
    pub var_x: String,
    pub var_y: String,
    pub show_ghost: bool,
    pub show_vector_field: bool,
    pub show_nullclines: bool,
    pub is_dragging_initial_pos: bool,
}

impl Default for PhaseViewState {
    fn default() -> Self {
        Self {
            var_x: "a".to_string(),
            var_y: "b".to_string(),
            show_ghost: true,
            show_vector_field: true,
            show_nullclines: true,
            is_dragging_initial_pos: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct PhasePanelResponse {
    pub clicked_step: Option<usize>,
    pub new_initial_conditions: Vec<(String, f64)>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_phase_panel(
    ui: &mut Ui,
    sim_result: &SimulationResult,
    ghost_sim: Option<&SimulationResult>,
    cursor_step: Option<usize>,
    selection: Option<(usize, usize)>,
    equations: &[Equation],
    initial_conditions: &HashMap<String, f64>,
    parameters: &HashMap<String, f64>,
    clamp_threshold: Option<f64>,
    state: &mut PhaseViewState,
) -> PhasePanelResponse {
    let mut response_action = PhasePanelResponse::default();

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("Phase Portrait (State Space)")
                    .size(14.0)
                    .strong(),
            );

            ui.separator();

            let mut vars: Vec<_> = sim_result.series.keys().cloned().collect();
            vars.sort();

            if vars.is_empty() {
                ui.label("No active state variables.");
                return;
            }

            // Ensure valid selection
            if !vars.contains(&state.var_x) {
                state.var_x = vars[0].clone();
            }
            if !vars.contains(&state.var_y) {
                state.var_y = vars.get(1).unwrap_or(&vars[0]).clone();
            }

            // X Variable selector
            ui.label("X:");
            egui::ComboBox::from_id_salt("phase_var_x")
                .selected_text(&state.var_x)
                .show_ui(ui, |ui| {
                    for v in &vars {
                        ui.selectable_value(&mut state.var_x, v.clone(), v);
                    }
                });

            // Y Variable selector
            ui.label("Y:");
            egui::ComboBox::from_id_salt("phase_var_y")
                .selected_text(&state.var_y)
                .show_ui(ui, |ui| {
                    for v in &vars {
                        ui.selectable_value(&mut state.var_y, v.clone(), v);
                    }
                });

            ui.checkbox(&mut state.show_vector_field, "Field");
            ui.checkbox(&mut state.show_nullclines, "Nullclines");

            if ghost_sim.is_some() {
                ui.checkbox(&mut state.show_ghost, "Ghost orbit");
            }
        });

        ui.add_space(4.0);

        let series_x = sim_result.series.get(&state.var_x);
        let series_y = sim_result.series.get(&state.var_y);

        if series_x.is_none() || series_y.is_none() {
            ui.label("Selected state variables not found in simulation result.");
            return;
        }

        let xs = &series_x.unwrap().values;
        let ys = &series_y.unwrap().values;
        let n_pts = xs.len().min(ys.len());

        if n_pts < 2 {
            ui.label("Insufficient simulation points for phase trajectory.");
            return;
        }

        // Bounding box calculation
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut is_clamped = false;

        for i in 0..n_pts {
            let mut x = xs[i];
            let mut y = ys[i];
            if let Some(th) = clamp_threshold {
                if x.abs() >= th - 1e-4 || y.abs() >= th - 1e-4 {
                    is_clamped = true;
                }
                x = x.clamp(-th, th);
                y = y.clamp(-th, th);
            }
            if x.is_finite() && y.is_finite() {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }

        if !min_x.is_finite() || !max_x.is_finite() {
            min_x = -1.0;
            max_x = 1.0;
        }
        if !min_y.is_finite() || !max_y.is_finite() {
            min_y = -1.0;
            max_y = 1.0;
        }

        let (plot_min_x, plot_max_x, plot_min_y, plot_max_y) = if let Some(th) = clamp_threshold {
            let bound = th * 1.08;
            (-bound, bound, -bound, bound)
        } else {
            let span_x = (max_x - min_x).max(1e-4);
            let span_y = (max_y - min_y).max(1e-4);
            let max_span = span_x.max(span_y) * 1.12;

            let center_x = (min_x + max_x) * 0.5;
            let center_y = (min_y + max_y) * 0.5;
            (
                center_x - max_span * 0.5,
                center_x + max_span * 0.5,
                center_y - max_span * 0.5,
                center_y + max_span * 0.5,
            )
        };

        let canvas_size = vec2(ui.available_width().max(200.0), 220.0);
        let (response, painter) = ui.allocate_painter(canvas_size, Sense::click_and_drag());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 4.0_f32, Color32::from_rgb(18, 20, 26));

        // Coordinate transforms respecting clamping threshold
        let to_screen = |x: f64, y: f64| -> egui::Pos2 {
            let cx = if let Some(th) = clamp_threshold {
                x.clamp(-th, th)
            } else {
                x
            };
            let cy = if let Some(th) = clamp_threshold {
                y.clamp(-th, th)
            } else {
                y
            };
            let norm_x = (cx - plot_min_x) / (plot_max_x - plot_min_x).max(1e-6);
            let norm_y = (cy - plot_min_y) / (plot_max_y - plot_min_y).max(1e-6);
            pos2(
                rect.left() + (norm_x as f32) * rect.width(),
                rect.bottom() - (norm_y as f32) * rect.height(),
            )
        };

        let screen_to_phase_coords = |pos: egui::Pos2| -> (f64, f64) {
            let norm_x = ((pos.x - rect.left()) / rect.width().max(1.0)) as f64;
            let norm_y = ((rect.bottom() - pos.y) / rect.height().max(1.0)) as f64;
            let raw_x = plot_min_x + norm_x * (plot_max_x - plot_min_x);
            let raw_y = plot_min_y + norm_y * (plot_max_y - plot_min_y);
            if let Some(th) = clamp_threshold {
                (raw_x.clamp(-th, th), raw_y.clamp(-th, th))
            } else {
                (raw_x, raw_y)
            }
        };

        // Draw origin axes crosshair if in range
        let origin = to_screen(0.0, 0.0);
        if origin.x >= rect.left() && origin.x <= rect.right() {
            painter.line_segment(
                [pos2(origin.x, rect.top()), pos2(origin.x, rect.bottom())],
                Stroke::new(1.0_f32, Color32::from_rgb(32, 36, 48)),
            );
        }
        if origin.y >= rect.top() && origin.y <= rect.bottom() {
            painter.line_segment(
                [pos2(rect.left(), origin.y), pos2(rect.right(), origin.y)],
                Stroke::new(1.0_f32, Color32::from_rgb(32, 36, 48)),
            );
        }

        // Draw Vector Field & Nullclines (Direction Field)
        if (state.show_vector_field || state.show_nullclines) && !equations.is_empty() {
            let field = compute_phase_field(
                equations,
                &state.var_x,
                &state.var_y,
                initial_conditions,
                parameters,
                plot_min_x,
                plot_max_x,
                plot_min_y,
                plot_max_y,
                14,
                14,
            );

            if state.show_vector_field {
                for pt in &field.vectors {
                    let tail = to_screen(pt.x, pt.y);
                    let len = 10.0_f32 * (pt.magnitude as f32).clamp(0.2, 1.0);
                    let head = pos2(
                        tail.x + (pt.norm_dx as f32) * len,
                        tail.y - (pt.norm_dy as f32) * len,
                    );
                    painter.line_segment(
                        [tail, head],
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(80, 160, 220, 55)),
                    );
                    let theta = (-(pt.dy as f32)).atan2(pt.dx as f32);
                    let tip1 = pos2(
                        head.x - 3.0 * (theta - 0.45).cos(),
                        head.y - 3.0 * (theta - 0.45).sin(),
                    );
                    let tip2 = pos2(
                        head.x - 3.0 * (theta + 0.45).cos(),
                        head.y - 3.0 * (theta + 0.45).sin(),
                    );
                    painter.line_segment(
                        [head, tip1],
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(80, 160, 220, 70)),
                    );
                    painter.line_segment(
                        [head, tip2],
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(80, 160, 220, 70)),
                    );
                }
            }

            if state.show_nullclines {
                let x_null_stroke =
                    Stroke::new(1.4_f32, Color32::from_rgba_unmultiplied(0, 230, 220, 120));
                for seg in &field.x_nullclines {
                    let p0 = to_screen(seg.p0.0, seg.p0.1);
                    let p1 = to_screen(seg.p1.0, seg.p1.1);
                    painter.line_segment([p0, p1], x_null_stroke);
                }

                let y_null_stroke =
                    Stroke::new(1.4_f32, Color32::from_rgba_unmultiplied(255, 90, 180, 120));
                for seg in &field.y_nullclines {
                    let p0 = to_screen(seg.p0.0, seg.p0.1);
                    let p1 = to_screen(seg.p1.0, seg.p1.1);
                    painter.line_segment([p0, p1], y_null_stroke);
                }
            }
        }

        // Draw Ghost Orbit if available and enabled
        if state.show_ghost {
            if let Some(ghost) = ghost_sim {
                if let (Some(g_series_x), Some(g_series_y)) = (
                    ghost.series.get(&state.var_x),
                    ghost.series.get(&state.var_y),
                ) {
                    let g_xs = &g_series_x.values;
                    let g_ys = &g_series_y.values;
                    let g_pts = g_xs.len().min(g_ys.len());
                    let mut ghost_line = Vec::new();
                    let step_stride = if g_pts > 1500 { 2 } else { 1 };
                    for i in (0..g_pts).step_by(step_stride) {
                        let gx = g_xs[i];
                        let gy = g_ys[i];
                        if gx.is_finite() && gy.is_finite() {
                            ghost_line.push(to_screen(gx, gy));
                        }
                    }
                    if ghost_line.len() >= 2 {
                        painter.add(egui::Shape::line(
                            ghost_line,
                            Stroke::new(
                                1.2_f32,
                                Color32::from_rgba_unmultiplied(140, 150, 180, 60),
                            ),
                        ));
                    }
                }
            }
        }

        // Draw Clamping Rail Boundary Box if clamping is active
        if let Some(th) = clamp_threshold {
            let p_tl = to_screen(-th, th);
            let p_br = to_screen(th, -th);
            let rail_rect = Rect::from_two_pos(p_tl, p_br);
            let rail_stroke = if is_clamped {
                Stroke::new(1.2_f32, Color32::from_rgba_unmultiplied(255, 70, 70, 200))
            } else {
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(70, 80, 100, 100))
            };
            painter.rect_stroke(rail_rect, 0.0, rail_stroke, StrokeKind::Inside);
            if is_clamped {
                painter.text(
                    pos2(rail_rect.right() - 4.0, rail_rect.top() + 4.0),
                    egui::Align2::RIGHT_TOP,
                    format!("RAILS (±{}) [CLAMPED]", format_compact_value(th)),
                    FontId::monospace(8.5),
                    Color32::from_rgb(255, 90, 90),
                );
            }
        }

        // Draw Active Trajectory
        let (sel_start, sel_end) = selection.unwrap_or((0, n_pts.saturating_sub(1)));
        let is_selection_active = selection.is_some();

        let stride = if n_pts > 2000 { 2 } else { 1 };
        let mut normal_segments = Vec::new();
        let mut selected_segments = Vec::new();
        let mut clamped_pts = Vec::new();

        for i in (0..n_pts - 1).step_by(stride) {
            let next_i = (i + stride).min(n_pts - 1);
            let x0 = xs[i];
            let y0 = ys[i];
            let x1 = xs[next_i];
            let y1 = ys[next_i];

            if let Some(th) = clamp_threshold {
                if x0.abs() >= th - 1e-4 || y0.abs() >= th - 1e-4 {
                    clamped_pts.push(to_screen(x0, y0));
                }
            }

            let p0 = to_screen(x0, y0);
            let p1 = to_screen(x1, y1);

            if xs[i].is_finite()
                && ys[i].is_finite()
                && xs[next_i].is_finite()
                && ys[next_i].is_finite()
            {
                if is_selection_active && i >= sel_start && next_i <= sel_end {
                    selected_segments.push([p0, p1]);
                } else {
                    normal_segments.push([p0, p1]);
                }
            }
        }

        // Render normal segments
        let base_stroke = if is_selection_active {
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(0, 180, 220, 60))
        } else {
            Stroke::new(1.6_f32, Color32::from_rgb(0, 210, 255))
        };

        for seg in normal_segments {
            painter.line_segment(seg, base_stroke);
        }

        // Render selected highlight segments
        if is_selection_active {
            let sel_stroke = Stroke::new(2.4_f32, Color32::from_rgb(255, 230, 80));
            for seg in selected_segments {
                painter.line_segment(seg, sel_stroke);
            }
        }

        // Highlight clipped points on rails
        for pt in clamped_pts {
            painter.circle_filled(pt, 2.5_f32, Color32::from_rgb(255, 60, 60));
        }

        // Start point indicator & draggable handle (n = 0)
        let p_start = to_screen(xs[0], ys[0]);
        painter.circle_stroke(
            p_start,
            6.5,
            Stroke::new(1.4_f32, Color32::from_rgb(255, 230, 80)),
        );
        painter.circle_filled(p_start, 4.0, Color32::from_rgb(70, 240, 150));
        painter.text(
            pos2(p_start.x + 6.0, p_start.y - 6.0),
            egui::Align2::LEFT_BOTTOM,
            "init",
            FontId::monospace(9.0),
            Color32::from_rgb(180, 220, 200),
        );

        // End point indicator (n = N - 1)
        let p_end = to_screen(xs[n_pts - 1], ys[n_pts - 1]);
        painter.circle_filled(p_end, 3.5, Color32::from_rgb(255, 100, 100));

        // Active timeline cursor point
        if let Some(step) = cursor_step {
            if step < n_pts {
                let cur_x = xs[step];
                let cur_y = ys[step];
                if cur_x.is_finite() && cur_y.is_finite() {
                    let p_cur = to_screen(cur_x, cur_y);
                    painter.circle_stroke(
                        p_cur,
                        7.5,
                        Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                    );
                    painter.circle_filled(p_cur, 4.5, Color32::from_rgb(255, 220, 50));

                    // HUD readout with clamping feedback
                    let is_pt_clamped = if let Some(th) = clamp_threshold {
                        cur_x.abs() >= th - 1e-4 || cur_y.abs() >= th - 1e-4
                    } else {
                        false
                    };

                    let readout = if is_pt_clamped {
                        format!(
                            "n={step}: ({}, {}) [CLAMPED]",
                            format_compact_value(cur_x),
                            format_compact_value(cur_y)
                        )
                    } else {
                        format!(
                            "n={step}: ({}, {})",
                            format_compact_value(cur_x),
                            format_compact_value(cur_y)
                        )
                    };

                    painter.text(
                        pos2(rect.right() - 8.0, rect.top() + 8.0),
                        egui::Align2::RIGHT_TOP,
                        readout,
                        FontId::monospace(10.5),
                        if is_pt_clamped {
                            Color32::from_rgb(255, 90, 90)
                        } else {
                            Color32::from_rgb(255, 230, 80)
                        },
                    );
                }
            }
        }

        // Direct interaction: Drag start point to adjust initial conditions, or scrub time cursor
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(ptr) = response.interact_pointer_pos() {
                if (ptr - p_start).length() <= 16.0 {
                    state.is_dragging_initial_pos = true;
                }
            }
        }

        if state.is_dragging_initial_pos {
            if let Some(ptr) = response.interact_pointer_pos() {
                let (new_x, new_y) = screen_to_phase_coords(ptr);
                response_action
                    .new_initial_conditions
                    .push((state.var_x.clone(), new_x));
                response_action
                    .new_initial_conditions
                    .push((state.var_y.clone(), new_y));

                painter.circle_filled(ptr, 5.0, Color32::from_rgb(70, 240, 150));
                painter.circle_stroke(
                    ptr,
                    9.0,
                    Stroke::new(1.5_f32, Color32::from_rgb(255, 230, 80)),
                );
                painter.text(
                    pos2(ptr.x + 10.0, ptr.y - 10.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!(
                        "({}, {}) = ({}, {})",
                        state.var_x,
                        state.var_y,
                        format_compact_value(new_x),
                        format_compact_value(new_y)
                    ),
                    FontId::monospace(10.0),
                    Color32::from_rgb(255, 230, 80),
                );
            }
            if response.drag_stopped() {
                state.is_dragging_initial_pos = false;
            }
        } else if response.clicked() || response.dragged() {
            if let Some(ptr) = response.interact_pointer_pos() {
                let mut best_dist_sq = f32::MAX;
                let mut best_idx = 0;

                for i in (0..n_pts).step_by(stride) {
                    let pt = to_screen(xs[i], ys[i]);
                    let dx = pt.x - ptr.x;
                    let dy = pt.y - ptr.y;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < best_dist_sq {
                        best_dist_sq = dist_sq;
                        best_idx = i;
                    }
                }

                if best_dist_sq < 900.0 {
                    response_action.clicked_step = Some(best_idx);
                }
            }
        }

        // Axes labels at bottom corners
        painter.text(
            pos2(rect.left() + 8.0, rect.bottom() - 6.0),
            egui::Align2::LEFT_BOTTOM,
            format!("X: {}", state.var_x),
            FontId::proportional(11.0),
            Color32::from_rgb(130, 140, 160),
        );
        painter.text(
            pos2(rect.left() + 8.0, rect.top() + 6.0),
            egui::Align2::LEFT_TOP,
            format!("Y: {}", state.var_y),
            FontId::proportional(11.0),
            Color32::from_rgb(130, 140, 160),
        );
    });

    response_action
}
