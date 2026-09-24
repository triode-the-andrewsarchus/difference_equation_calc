use crate::sim::SimulationResult;
use egui::{pos2, vec2, Color32, FontId, Sense, Stroke, Ui};

#[derive(Debug, Clone)]
pub struct PhaseViewState {
    pub var_x: String,
    pub var_y: String,
    pub show_ghost: bool,
}

impl Default for PhaseViewState {
    fn default() -> Self {
        Self {
            var_x: "a".to_string(),
            var_y: "b".to_string(),
            show_ghost: true,
        }
    }
}

pub fn render_phase_panel(
    ui: &mut Ui,
    sim_result: &SimulationResult,
    ghost_sim: Option<&SimulationResult>,
    cursor_step: Option<usize>,
    selection: Option<(usize, usize)>,
    state: &mut PhaseViewState,
) -> Option<usize> {
    let mut clicked_step = None;

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

        for i in 0..n_pts {
            let x = xs[i].clamp(-1.0, 1.0);
            let y = ys[i].clamp(-1.0, 1.0);
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

        // Add padding while keeping within [-1.0, 1.0]
        let span_x = (max_x - min_x).max(1e-4);
        let span_y = (max_y - min_y).max(1e-4);
        let max_span = span_x.max(span_y) * 1.08;

        let center_x = (min_x + max_x) * 0.5;
        let center_y = (min_y + max_y) * 0.5;

        let plot_min_x = (center_x - max_span * 0.5).max(-1.0);
        let plot_max_x = (center_x + max_span * 0.5).min(1.0);
        let plot_min_y = (center_y - max_span * 0.5).max(-1.0);
        let plot_max_y = (center_y + max_span * 0.5).min(1.0);

        let canvas_size = vec2(ui.available_width().max(200.0), 220.0);
        let (response, painter) = ui.allocate_painter(canvas_size, Sense::click_and_drag());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 4.0_f32, Color32::from_rgb(18, 20, 26));

        // Coordinate transforms clamped to [-1.0, 1.0]
        let to_screen = |x: f64, y: f64| -> egui::Pos2 {
            let cx = x.clamp(-1.0, 1.0);
            let cy = y.clamp(-1.0, 1.0);
            let norm_x = (cx - plot_min_x) / (plot_max_x - plot_min_x).max(1e-6);
            let norm_y = (cy - plot_min_y) / (plot_max_y - plot_min_y).max(1e-6);
            pos2(
                rect.left() + (norm_x as f32) * rect.width(),
                rect.bottom() - (norm_y as f32) * rect.height(),
            )
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

        // Draw Active Trajectory
        let (sel_start, sel_end) = selection.unwrap_or((0, n_pts.saturating_sub(1)));
        let is_selection_active = selection.is_some();

        let stride = if n_pts > 2000 { 2 } else { 1 };
        let mut normal_segments = Vec::new();
        let mut selected_segments = Vec::new();

        for i in (0..n_pts - 1).step_by(stride) {
            let next_i = (i + stride).min(n_pts - 1);
            let p0 = to_screen(xs[i], ys[i]);
            let p1 = to_screen(xs[next_i], ys[next_i]);

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

        // Start point indicator (n = 0)
        let p_start = to_screen(xs[0], ys[0]);
        painter.circle_filled(p_start, 4.0, Color32::from_rgb(70, 240, 150));
        painter.circle_stroke(
            p_start,
            4.0,
            Stroke::new(1.0_f32, Color32::from_rgb(20, 30, 20)),
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
                    // Glowing outer ring
                    painter.circle_stroke(
                        p_cur,
                        7.5,
                        Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                    );
                    painter.circle_filled(p_cur, 4.5, Color32::from_rgb(255, 220, 50));

                    // HUD readout clamped to [-1.0, 1.0]
                    painter.text(
                        pos2(rect.right() - 8.0, rect.top() + 8.0),
                        egui::Align2::RIGHT_TOP,
                        format!(
                            "n={step}: ({:.3}, {:.3})",
                            cur_x.clamp(-1.0, 1.0),
                            cur_y.clamp(-1.0, 1.0)
                        ),
                        FontId::monospace(10.5),
                        Color32::from_rgb(255, 230, 80),
                    );
                }
            }
        }

        // Direct interaction: Click or drag on phase trajectory to scrub time cursor
        if response.clicked() || response.dragged() {
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
                    // within 30px
                    clicked_step = Some(best_idx);
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

    clicked_step
}
