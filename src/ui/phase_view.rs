use crate::ast::Equation;
use crate::measure::compute_spectrum;
use crate::sim::{compute_phase_field, compute_return_map_curve, SimulationResult};
use crate::ui::format::format_compact_value;
use egui::{pos2, vec2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PhaseViewMode {
    #[default]
    PhasePortrait,
    CobwebReturn,
    SpectrumFft,
}

#[derive(Debug, Clone)]
pub struct PhaseViewState {
    pub mode: PhaseViewMode,
    pub var_x: String,
    pub var_y: String,
    pub show_ghost: bool,
    pub show_vector_field: bool,
    pub show_nullclines: bool,
    pub spectrum_db: bool,
    pub is_dragging_initial_pos: bool,
}

impl Default for PhaseViewState {
    fn default() -> Self {
        Self {
            mode: PhaseViewMode::PhasePortrait,
            var_x: "a".to_string(),
            var_y: "b".to_string(),
            show_ghost: true,
            show_vector_field: true,
            show_nullclines: true,
            spectrum_db: true,
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
            ui.selectable_value(&mut state.mode, PhaseViewMode::PhasePortrait, "2D Phase")
                .on_hover_text(
                    "2D State Space Portrait (X[n] vs Y[n]) with vector field & nullclines",
                );
            ui.selectable_value(
                &mut state.mode,
                PhaseViewMode::CobwebReturn,
                "Cobweb / Return",
            )
            .on_hover_text("1D Return Map (X[n] vs X[n+1]) with y=x diagonal and cobweb orbit");
            ui.selectable_value(
                &mut state.mode,
                PhaseViewMode::SpectrumFft,
                "Spectrum (FFT)",
            )
            .on_hover_text("Frequency domain magnitude spectrum (Hann-windowed FFT)");

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
            ui.label(if state.mode == PhaseViewMode::PhasePortrait {
                "X:"
            } else {
                "Var:"
            });
            egui::ComboBox::from_id_salt("phase_var_x")
                .selected_text(&state.var_x)
                .show_ui(ui, |ui| {
                    for v in &vars {
                        ui.selectable_value(&mut state.var_x, v.clone(), v);
                    }
                });

            match state.mode {
                PhaseViewMode::PhasePortrait => {
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
                        ui.checkbox(&mut state.show_ghost, "Ghost");
                    }
                }
                PhaseViewMode::CobwebReturn => {
                    if ghost_sim.is_some() {
                        ui.checkbox(&mut state.show_ghost, "Ghost");
                    }
                }
                PhaseViewMode::SpectrumFft => {
                    ui.checkbox(&mut state.spectrum_db, "dB Scale");
                    if ghost_sim.is_some() {
                        ui.checkbox(&mut state.show_ghost, "Ghost");
                    }
                }
            }
        });

        ui.add_space(4.0);

        if state.mode == PhaseViewMode::SpectrumFft {
            render_spectrum_canvas(ui, sim_result, ghost_sim, selection, state);
            return;
        }

        if state.mode == PhaseViewMode::CobwebReturn {
            render_cobweb_canvas(
                ui,
                sim_result,
                ghost_sim,
                cursor_step,
                selection,
                equations,
                initial_conditions,
                parameters,
                clamp_threshold,
                state,
                &mut response_action,
            );
            return;
        }

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

#[allow(clippy::too_many_arguments)]
fn render_cobweb_canvas(
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
    response_action: &mut PhasePanelResponse,
) {
    let Some(series_x) = sim_result.series.get(&state.var_x) else {
        ui.label("Selected state variable not found.");
        return;
    };
    let xs = &series_x.values;
    if xs.len() < 2 {
        ui.label("Insufficient points for return map.");
        return;
    }

    let mut min_v = f64::INFINITY;
    let mut max_v = f64::NEG_INFINITY;
    for &x in xs {
        let cx = if let Some(th) = clamp_threshold {
            x.clamp(-th, th)
        } else {
            x
        };
        if cx.is_finite() {
            min_v = min_v.min(cx);
            max_v = max_v.max(cx);
        }
    }
    if !min_v.is_finite() || !max_v.is_finite() {
        min_v = -1.0;
        max_v = 1.0;
    }
    let span = (max_v - min_v).max(0.1) * 1.15;
    let center = (min_v + max_v) * 0.5;
    let (plot_min, plot_max) = if let Some(th) = clamp_threshold {
        (-th * 1.08, th * 1.08)
    } else {
        (center - span * 0.5, center + span * 0.5)
    };

    let canvas_size = vec2(ui.available_width().max(200.0), 220.0);
    let (response, painter) = ui.allocate_painter(canvas_size, Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 4.0_f32, Color32::from_rgb(18, 20, 26));

    let to_screen = |x: f64, y: f64| -> egui::Pos2 {
        let nx = ((x - plot_min) / (plot_max - plot_min).max(1e-6)).clamp(0.0, 1.0);
        let ny = ((y - plot_min) / (plot_max - plot_min).max(1e-6)).clamp(0.0, 1.0);
        pos2(
            rect.left() + (nx as f32) * rect.width(),
            rect.bottom() - (ny as f32) * rect.height(),
        )
    };

    let screen_to_val = |sx: f32| -> f64 {
        let nx = ((sx - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0) as f64;
        plot_min + nx * (plot_max - plot_min)
    };

    // Diagonal identity line y = x
    let diag_p0 = to_screen(plot_min, plot_min);
    let diag_p1 = to_screen(plot_max, plot_max);
    painter.line_segment(
        [diag_p0, diag_p1],
        Stroke::new(1.0_f32, Color32::from_rgb(60, 70, 90)),
    );

    // 1-step map curve f(x)
    let curve_pts = compute_return_map_curve(
        equations,
        &state.var_x,
        initial_conditions,
        parameters,
        plot_min,
        plot_max,
        80,
    );
    if curve_pts.len() >= 2 {
        let screen_curve: Vec<egui::Pos2> = curve_pts
            .into_iter()
            .map(|(x, y)| to_screen(x, y))
            .collect();
        painter.add(egui::Shape::line(
            screen_curve,
            Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 130, 200, 150)),
        ));
    }

    // Ghost cobweb if enabled
    if state.show_ghost {
        if let Some(ghost) = ghost_sim {
            if let Some(g_series) = ghost.series.get(&state.var_x) {
                let gxs = &g_series.values;
                let mut g_pts = Vec::new();
                for i in 0..gxs.len().saturating_sub(1).min(400) {
                    if gxs[i].is_finite() && gxs[i + 1].is_finite() {
                        g_pts.push(to_screen(gxs[i], gxs[i]));
                        g_pts.push(to_screen(gxs[i], gxs[i + 1]));
                    }
                }
                if g_pts.len() >= 2 {
                    painter.add(egui::Shape::line(
                        g_pts,
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(140, 150, 180, 50)),
                    ));
                }
            }
        }
    }

    // Cobweb staircase (x[n], x[n]) -> (x[n], x[n+1]) -> (x[n+1], x[n+1])
    let (s0, s1) = selection.unwrap_or((0, xs.len().saturating_sub(2)));
    let end_i = s1.min(xs.len().saturating_sub(2));
    let start_i = s0.min(end_i);
    let max_draw = (end_i.saturating_sub(start_i) + 1).min(300);

    let mut cobweb_line = Vec::with_capacity(max_draw * 2 + 1);
    for i in start_i..(start_i + max_draw) {
        let x0 = xs[i];
        let x1 = xs[i + 1];
        if x0.is_finite() && x1.is_finite() {
            if cobweb_line.is_empty() {
                cobweb_line.push(to_screen(x0, x0));
            }
            let pt_map = to_screen(x0, x1);
            let pt_diag = to_screen(x1, x1);
            cobweb_line.push(pt_map);
            cobweb_line.push(pt_diag);
            painter.circle_filled(pt_map, 2.0, Color32::from_rgb(0, 220, 255));
        }
    }
    if cobweb_line.len() >= 2 {
        painter.add(egui::Shape::line(
            cobweb_line,
            Stroke::new(1.3_f32, Color32::from_rgba_unmultiplied(0, 220, 255, 190)),
        ));
    }

    // Start handle at x[0]
    let p_start = to_screen(xs[0], xs[0]);
    painter.circle_stroke(
        p_start,
        6.0,
        Stroke::new(1.4_f32, Color32::from_rgb(255, 230, 80)),
    );
    painter.circle_filled(p_start, 3.5, Color32::from_rgb(70, 240, 150));

    // Active cursor step highlight
    if let Some(step) = cursor_step {
        if step + 1 < xs.len() && xs[step].is_finite() && xs[step + 1].is_finite() {
            let p_cur = to_screen(xs[step], xs[step + 1]);
            painter.circle_stroke(p_cur, 7.0, Stroke::new(1.5_f32, Color32::WHITE));
            painter.circle_filled(p_cur, 4.0, Color32::from_rgb(255, 220, 50));
            painter.text(
                pos2(rect.right() - 8.0, rect.top() + 8.0),
                egui::Align2::RIGHT_TOP,
                format!(
                    "n={step}: {} ➔ {}",
                    format_compact_value(xs[step]),
                    format_compact_value(xs[step + 1])
                ),
                FontId::monospace(10.5),
                Color32::from_rgb(255, 230, 80),
            );
        }
    }

    // Drag start point to change initial condition x[0]
    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(ptr) = response.interact_pointer_pos() {
            if (ptr - p_start).length() <= 18.0 {
                state.is_dragging_initial_pos = true;
            }
        }
    }
    if state.is_dragging_initial_pos {
        if let Some(ptr) = response.interact_pointer_pos() {
            let new_x0 = screen_to_val(ptr.x);
            response_action
                .new_initial_conditions
                .push((state.var_x.clone(), new_x0));
        }
        if response.drag_stopped() {
            state.is_dragging_initial_pos = false;
        }
    }

    painter.text(
        pos2(rect.left() + 8.0, rect.bottom() - 6.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{}[n]", state.var_x),
        FontId::monospace(10.5),
        Color32::from_rgb(130, 140, 160),
    );
    painter.text(
        pos2(rect.left() + 8.0, rect.top() + 6.0),
        egui::Align2::LEFT_TOP,
        format!("{}[n+1]  (Cobweb & Return Map)", state.var_x),
        FontId::monospace(10.5),
        Color32::from_rgb(255, 160, 210),
    );
}

fn render_spectrum_canvas(
    ui: &mut Ui,
    sim_result: &SimulationResult,
    ghost_sim: Option<&SimulationResult>,
    selection: Option<(usize, usize)>,
    state: &PhaseViewState,
) {
    let Some(series_x) = sim_result.series.get(&state.var_x) else {
        ui.label("Selected state variable not found.");
        return;
    };
    let vals = &series_x.values;
    if vals.len() < 4 {
        ui.label("Insufficient samples for FFT spectrum.");
        return;
    }

    let (s0, s1) = selection.unwrap_or((0, vals.len().saturating_sub(1)));
    let actual_end = s1.min(vals.len().saturating_sub(1));
    let slice = if s0 <= actual_end {
        &vals[s0..=actual_end]
    } else {
        vals
    };

    let spec = compute_spectrum(slice);

    let canvas_size = vec2(ui.available_width().max(200.0), 220.0);
    let (response, painter) = ui.allocate_painter(canvas_size, Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 4.0_f32, Color32::from_rgb(18, 20, 26));

    let pad_left = 42.0;
    let pad_right = 12.0;
    let pad_top = 22.0;
    let pad_bottom = 22.0;
    let plot_rect = Rect::from_min_max(
        pos2(rect.left() + pad_left, rect.top() + pad_top),
        pos2(rect.right() - pad_right, rect.bottom() - pad_bottom),
    );

    let (y_min, y_max) = if state.spectrum_db {
        (-80.0, (spec.peak_db + 6.0).max(6.0))
    } else {
        (0.0, (spec.peak_mag * 1.15).max(0.1))
    };

    let freq_to_x = |f: f64| -> f32 {
        let norm = (f / 0.5).clamp(0.0, 1.0);
        plot_rect.left() + (norm as f32) * plot_rect.width()
    };

    let val_to_y = |v: f64| -> f32 {
        let norm = ((v - y_min) / (y_max - y_min).max(1e-6)).clamp(0.0, 1.0);
        plot_rect.bottom() - (norm as f32) * plot_rect.height()
    };

    // Grid lines
    for &f_tick in &[0.0, 0.1, 0.2, 0.25, 0.3, 0.4, 0.5] {
        let sx = freq_to_x(f_tick);
        painter.line_segment(
            [pos2(sx, plot_rect.top()), pos2(sx, plot_rect.bottom())],
            Stroke::new(1.0_f32, Color32::from_rgb(28, 32, 42)),
        );
        painter.text(
            pos2(sx, plot_rect.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            format!("{f_tick:.2}"),
            FontId::monospace(8.5),
            Color32::from_rgb(110, 120, 140),
        );
    }

    // Y axis labels
    painter.text(
        pos2(plot_rect.left() - 4.0, plot_rect.top()),
        egui::Align2::RIGHT_TOP,
        if state.spectrum_db {
            format!("{:.0}dB", y_max)
        } else {
            format_compact_value(y_max)
        },
        FontId::monospace(8.5),
        Color32::from_rgb(110, 120, 140),
    );
    painter.text(
        pos2(plot_rect.left() - 4.0, plot_rect.bottom()),
        egui::Align2::RIGHT_BOTTOM,
        if state.spectrum_db {
            format!("{:.0}dB", y_min)
        } else {
            "0".to_string()
        },
        FontId::monospace(8.5),
        Color32::from_rgb(110, 120, 140),
    );

    // Ghost spectrum if available
    if state.show_ghost {
        if let Some(ghost) = ghost_sim {
            if let Some(g_series) = ghost.series.get(&state.var_x) {
                let g_end = s1.min(g_series.values.len().saturating_sub(1));
                if s0 <= g_end {
                    let g_spec = compute_spectrum(&g_series.values[s0..=g_end]);
                    let g_pts: Vec<egui::Pos2> = g_spec
                        .bins
                        .iter()
                        .map(|b| {
                            let yv = if state.spectrum_db {
                                b.mag_db
                            } else {
                                b.magnitude
                            };
                            pos2(freq_to_x(b.freq), val_to_y(yv))
                        })
                        .collect();
                    if g_pts.len() >= 2 {
                        painter.add(egui::Shape::line(
                            g_pts,
                            Stroke::new(
                                1.2_f32,
                                Color32::from_rgba_unmultiplied(150, 165, 200, 80),
                            ),
                        ));
                    }
                }
            }
        }
    }

    // Active spectrum polyline
    let pts: Vec<egui::Pos2> = spec
        .bins
        .iter()
        .map(|b| {
            let yv = if state.spectrum_db {
                b.mag_db
            } else {
                b.magnitude
            };
            pos2(freq_to_x(b.freq), val_to_y(yv))
        })
        .collect();
    if pts.len() >= 2 {
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(1.8_f32, Color32::from_rgb(0, 225, 255)),
        ));
    }

    // Highlight Peak Frequency
    if spec.peak_mag > 1e-5 {
        let px = freq_to_x(spec.peak_freq);
        let py = val_to_y(if state.spectrum_db {
            spec.peak_db
        } else {
            spec.peak_mag
        });
        painter.circle_filled(pos2(px, py), 4.0, Color32::from_rgb(255, 215, 70));
        let period_str = if spec.peak_freq > 1e-4 {
            format!("T={:.1}", 1.0 / spec.peak_freq)
        } else {
            "DC".to_string()
        };
        painter.text(
            pos2(rect.right() - 8.0, rect.top() + 5.0),
            egui::Align2::RIGHT_TOP,
            format!(
                "Peak f={:.4} cyc/step ({period_str}) · {:.1} dB",
                spec.peak_freq, spec.peak_db
            ),
            FontId::monospace(10.0),
            Color32::from_rgb(255, 225, 90),
        );
    }

    // Hover readout
    if let Some(ptr) = response.hover_pos() {
        if plot_rect.contains(ptr) {
            let norm_f = ((ptr.x - plot_rect.left()) / plot_rect.width()).clamp(0.0, 1.0) as f64;
            let f_hover = norm_f * 0.5;
            painter.line_segment(
                [
                    pos2(ptr.x, plot_rect.top()),
                    pos2(ptr.x, plot_rect.bottom()),
                ],
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 130)),
            );
            let period = if f_hover > 1e-4 {
                format!("T={:.1} steps", 1.0 / f_hover)
            } else {
                "DC".to_string()
            };
            painter.text(
                pos2(rect.left() + pad_left + 4.0, rect.top() + 5.0),
                egui::Align2::LEFT_TOP,
                format!("Cursor: f={f_hover:.4} ({period})"),
                FontId::monospace(10.0),
                Color32::from_rgb(180, 230, 255),
            );
        }
    }
}
