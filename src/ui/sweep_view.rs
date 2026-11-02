use crate::document::Document;
use crate::measure::MeasurementType;
use crate::sweep::{AttractorSweepResult, Sweep2DResult, SweepMode, SweepResult};
use crate::ui::format::format_compact_value;
use egui::{pos2, vec2, Color32, FontId, Rect, Sense, Stroke, Ui};

pub struct SweepPlotResponse {
    pub param_changed: Option<(String, f64)>,
    pub param_y_changed: Option<(String, f64)>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_sweep_panel(
    ui: &mut Ui,
    doc: &mut Document,
    sweep_result: &SweepResult,
    attractor_sweep: Option<&AttractorSweepResult>,
    sweep_2d_result: Option<&Sweep2DResult>,
    sweep_mode: &mut SweepMode,
    bifurcation_var: &mut String,
    sweep_param_y: &mut String,
    available_vars: &[String],
) -> SweepPlotResponse {
    let mut response_action = SweepPlotResponse {
        param_changed: None,
        param_y_changed: None,
    };

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("Parameter Exploration")
                    .size(14.0)
                    .strong(),
            );

            ui.separator();

            // Mode switch
            ui.selectable_value(sweep_mode, SweepMode::Measurement, "1D Measurement")
                .on_hover_text("Sweep 1 parameter against any selection measurement");
            ui.selectable_value(
                sweep_mode,
                SweepMode::AttractorBifurcation,
                "Bifurcation Orbit",
            )
            .on_hover_text("Plot steady-state attractor / orbit values across parameter range");
            ui.selectable_value(sweep_mode, SweepMode::Heatmap2D, "2D Parameter Map")
                .on_hover_text(
                    "2D parameter space heatmap: drag in 2D to scrub two parameters simultaneously",
                );

            ui.separator();

            // Parameter X selector
            ui.label(if *sweep_mode == SweepMode::Heatmap2D {
                "Param X:"
            } else {
                "Parameter:"
            });
            let current_param = doc.state.sweep_config.parameter_name.clone();
            egui::ComboBox::from_id_salt("sweep_param_select")
                .selected_text(&current_param)
                .show_ui(ui, |ui| {
                    for p in &doc.state.parameters {
                        if ui
                            .selectable_value(
                                &mut doc.state.sweep_config.parameter_name,
                                p.name.clone(),
                                &p.name,
                            )
                            .clicked()
                        {
                            doc.state.sweep_config.min_val = p.min;
                            doc.state.sweep_config.max_val = p.max;
                        }
                    }
                });

            if *sweep_mode == SweepMode::Heatmap2D {
                ui.label("Param Y:");
                if doc
                    .state
                    .parameters
                    .iter()
                    .all(|p| &p.name != sweep_param_y)
                {
                    if let Some(second) = doc
                        .state
                        .parameters
                        .iter()
                        .find(|p| p.name != doc.state.sweep_config.parameter_name)
                        .or_else(|| doc.state.parameters.first())
                    {
                        *sweep_param_y = second.name.clone();
                    }
                }
                egui::ComboBox::from_id_salt("sweep_param_y_select")
                    .selected_text(sweep_param_y.as_str())
                    .show_ui(ui, |ui| {
                        for p in &doc.state.parameters {
                            ui.selectable_value(sweep_param_y, p.name.clone(), &p.name);
                        }
                    });
            }

            match *sweep_mode {
                SweepMode::Measurement | SweepMode::Heatmap2D => {
                    // Interactive Measurement Type & Variable selectors right in the header
                    ui.label("Metric:");
                    egui::ComboBox::from_id_salt("sweep_metric_kind")
                        .selected_text(doc.state.sweep_config.measurement.kind.name())
                        .show_ui(ui, |ui| {
                            for &kind in MeasurementType::ALL {
                                ui.selectable_value(
                                    &mut doc.state.sweep_config.measurement.kind,
                                    kind,
                                    kind.name(),
                                );
                            }
                        });

                    if doc.state.sweep_config.measurement.kind != MeasurementType::Duration {
                        ui.label("of");
                        if !available_vars.is_empty()
                            && !available_vars
                                .contains(&doc.state.sweep_config.measurement.variable)
                        {
                            doc.state.sweep_config.measurement.variable = available_vars[0].clone();
                        }
                        egui::ComboBox::from_id_salt("sweep_metric_var")
                            .selected_text(&doc.state.sweep_config.measurement.variable)
                            .show_ui(ui, |ui| {
                                for v in available_vars {
                                    ui.selectable_value(
                                        &mut doc.state.sweep_config.measurement.variable,
                                        v.clone(),
                                        v,
                                    );
                                }
                            });
                    }
                }
                SweepMode::AttractorBifurcation => {
                    ui.label("Variable:");
                    if !available_vars.is_empty() && !available_vars.contains(bifurcation_var) {
                        *bifurcation_var = available_vars[0].clone();
                    }
                    egui::ComboBox::from_id_salt("bifurcation_var_select")
                        .selected_text(bifurcation_var.as_str())
                        .show_ui(ui, |ui| {
                            for v in available_vars {
                                ui.selectable_value(bifurcation_var, v.clone(), v);
                            }
                        });
                }
            }

            // Min and max sweep range for X
            ui.label("Range X:");
            ui.add(
                egui::DragValue::new(&mut doc.state.sweep_config.min_val)
                    .speed(0.01)
                    .prefix("["),
            );
            ui.add(
                egui::DragValue::new(&mut doc.state.sweep_config.max_val)
                    .speed(0.01)
                    .prefix("..")
                    .suffix("]"),
            );
        });

        ui.add_space(4.0);

        // Current parameter value in document
        let current_param_val = doc
            .state
            .parameters
            .iter()
            .find(|p| p.name == doc.state.sweep_config.parameter_name)
            .map(|p| p.value)
            .unwrap_or(doc.state.sweep_config.min_val);

        // Render sweep 2D curve / points
        let plot_height = 140.0;
        let plot_width = ui.available_width().max(200.0);

        let (response, painter) =
            ui.allocate_painter(vec2(plot_width, plot_height), Sense::click_and_drag());

        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 4.0_f32, Color32::from_rgb(18, 20, 26));
        painter.rect_stroke(
            rect,
            4.0_f32,
            Stroke::new(1.0_f32, Color32::from_rgb(38, 42, 54)),
            egui::StrokeKind::Outside,
        );

        let p_min = doc.state.sweep_config.min_val;
        let p_max = doc.state.sweep_config.max_val;

        let margin_left = 60.0;
        let margin_bottom = 24.0;
        let margin_top = 16.0;
        let margin_right = 20.0;

        let plot_rect = Rect::from_min_max(
            pos2(rect.left() + margin_left, rect.top() + margin_top),
            pos2(rect.right() - margin_right, rect.bottom() - margin_bottom),
        );

        let to_screen_x = |pval: f64| -> f32 {
            let norm = (pval - p_min) / (p_max - p_min).max(1e-6);
            plot_rect.left() + (norm as f32) * plot_rect.width()
        };

        let screen_to_param = |sx: f32| -> f64 {
            let norm = ((sx - plot_rect.left()) / plot_rect.width()).clamp(0.0, 1.0);
            p_min + (norm as f64) * (p_max - p_min)
        };
        match *sweep_mode {
            SweepMode::Measurement => {
                let raw_m_min = if sweep_result.min_measure.is_finite() {
                    sweep_result.min_measure
                } else {
                    -1.0
                };
                let raw_m_max = if sweep_result.max_measure.is_finite() {
                    sweep_result.max_measure
                } else {
                    1.0
                };
                let span = (raw_m_max - raw_m_min).max(1e-4);
                let m_min = raw_m_min - span * 0.05;
                let m_max = raw_m_max + span * 0.05;

                let to_screen_y = |mval: f64| -> f32 {
                    let clamped = mval.clamp(m_min, m_max);
                    let norm = (clamped - m_min) / (m_max - m_min).max(1e-6);
                    plot_rect.bottom() - (norm as f32) * plot_rect.height()
                };

                // Grid labels
                painter.text(
                    pos2(rect.left() + 6.0, plot_rect.top()),
                    egui::Align2::LEFT_TOP,
                    format_compact_value(m_max),
                    FontId::monospace(9.5),
                    Color32::from_rgb(120, 130, 150),
                );
                painter.text(
                    pos2(rect.left() + 6.0, plot_rect.bottom()),
                    egui::Align2::LEFT_BOTTOM,
                    format_compact_value(m_min),
                    FontId::monospace(9.5),
                    Color32::from_rgb(120, 130, 150),
                );

                // Zero line if in range
                if m_min <= 0.0 && m_max >= 0.0 {
                    let zero_y = to_screen_y(0.0);
                    painter.line_segment(
                        [
                            pos2(plot_rect.left(), zero_y),
                            pos2(plot_rect.right(), zero_y),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgb(45, 50, 65)),
                    );
                }

                // Measurement line curve
                let mut path_points = Vec::new();
                for pt in &sweep_result.points {
                    if let Some(mval) = pt.measurement_value {
                        let sx = to_screen_x(pt.param_value);
                        let sy = to_screen_y(mval);
                        path_points.push(pos2(sx, sy));
                    } else if path_points.len() >= 2 {
                        painter.add(egui::Shape::line(
                            path_points.clone(),
                            Stroke::new(2.0_f32, Color32::from_rgb(0, 220, 255)),
                        ));
                        path_points.clear();
                    }
                }
                if path_points.len() >= 2 {
                    painter.add(egui::Shape::line(
                        path_points,
                        Stroke::new(2.0_f32, Color32::from_rgb(0, 220, 255)),
                    ));
                }

                // Handle dot and vertical line
                let cur_x = to_screen_x(current_param_val);
                if cur_x >= plot_rect.left() && cur_x <= plot_rect.right() {
                    painter.line_segment(
                        [
                            pos2(cur_x, plot_rect.top()),
                            pos2(cur_x, plot_rect.bottom()),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 200, 80, 160)),
                    );

                    // Interpolate measurement
                    let mut interp_m = None;
                    for i in 0..sweep_result.points.len().saturating_sub(1) {
                        let p0 = &sweep_result.points[i];
                        let p1 = &sweep_result.points[i + 1];
                        if current_param_val >= p0.param_value
                            && current_param_val <= p1.param_value
                        {
                            if let (Some(m0), Some(m1)) =
                                (p0.measurement_value, p1.measurement_value)
                            {
                                let span = (p1.param_value - p0.param_value).max(1e-9);
                                let frac = (current_param_val - p0.param_value) / span;
                                interp_m = Some(m0 + frac * (m1 - m0));
                                break;
                            }
                        }
                    }

                    let cur_y = interp_m.map_or(plot_rect.center().y, to_screen_y);
                    painter.circle_filled(
                        pos2(cur_x, cur_y),
                        7.0,
                        Color32::from_rgba_premultiplied(255, 200, 80, 80),
                    );
                    painter.circle_filled(pos2(cur_x, cur_y), 4.5, Color32::from_rgb(255, 215, 90));

                    let readout_label = format!(
                        "{} = {}{}",
                        doc.state.sweep_config.parameter_name,
                        format_compact_value(current_param_val),
                        interp_m.map_or("".to_string(), |m| format!(
                            ", M = {}",
                            format_compact_value(m)
                        ))
                    );
                    painter.text(
                        pos2(cur_x + 8.0, cur_y - 12.0),
                        egui::Align2::LEFT_BOTTOM,
                        readout_label,
                        FontId::monospace(10.5),
                        Color32::from_rgb(255, 235, 140),
                    );
                }
            }
            SweepMode::AttractorBifurcation => {
                if let Some(attractor) = attractor_sweep {
                    let raw_s_min = if attractor.state_min.is_finite() {
                        attractor.state_min
                    } else {
                        -1.0
                    };
                    let raw_s_max = if attractor.state_max.is_finite() {
                        attractor.state_max
                    } else {
                        1.0
                    };
                    let span = (raw_s_max - raw_s_min).max(1e-4);
                    let s_min = raw_s_min - span * 0.05;
                    let s_max = raw_s_max + span * 0.05;

                    let to_screen_y = |sval: f64| -> f32 {
                        let clamped = sval.clamp(s_min, s_max);
                        let norm = (clamped - s_min) / (s_max - s_min).max(1e-6);
                        plot_rect.bottom() - (norm as f32) * plot_rect.height()
                    };

                    // Grid labels
                    painter.text(
                        pos2(rect.left() + 6.0, plot_rect.top()),
                        egui::Align2::LEFT_TOP,
                        format_compact_value(s_max),
                        FontId::monospace(9.5),
                        Color32::from_rgb(120, 130, 150),
                    );
                    painter.text(
                        pos2(rect.left() + 6.0, plot_rect.bottom()),
                        egui::Align2::LEFT_BOTTOM,
                        format_compact_value(s_min),
                        FontId::monospace(9.5),
                        Color32::from_rgb(120, 130, 150),
                    );

                    // Zero line if in range
                    if s_min <= 0.0 && s_max >= 0.0 {
                        let zero_y = to_screen_y(0.0);
                        painter.line_segment(
                            [
                                pos2(plot_rect.left(), zero_y),
                                pos2(plot_rect.right(), zero_y),
                            ],
                            Stroke::new(1.0_f32, Color32::from_rgb(45, 50, 65)),
                        );
                    }

                    // Render attractor point cloud
                    let pt_color = Color32::from_rgba_unmultiplied(0, 225, 255, 160);
                    for pt in &attractor.points {
                        let sx = to_screen_x(pt.param_val);
                        let sy = to_screen_y(pt.state_val);
                        if sx >= plot_rect.left()
                            && sx <= plot_rect.right()
                            && sy >= plot_rect.top()
                            && sy <= plot_rect.bottom()
                        {
                            painter.circle_filled(pos2(sx, sy), 1.2, pt_color);
                        }
                    }

                    // Vertical parameter scrub line
                    let cur_x = to_screen_x(current_param_val);
                    if cur_x >= plot_rect.left() && cur_x <= plot_rect.right() {
                        painter.line_segment(
                            [
                                pos2(cur_x, plot_rect.top()),
                                pos2(cur_x, plot_rect.bottom()),
                            ],
                            Stroke::new(1.2_f32, Color32::from_rgb(255, 215, 90)),
                        );
                        painter.text(
                            pos2(cur_x + 6.0, plot_rect.top() + 4.0),
                            egui::Align2::LEFT_TOP,
                            format!(
                                "{} = {}",
                                doc.state.sweep_config.parameter_name,
                                format_compact_value(current_param_val)
                            ),
                            FontId::monospace(10.5),
                            Color32::from_rgb(255, 235, 140),
                        );
                    }
                } else {
                    painter.text(
                        plot_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Calculating attractor bifurcation map...",
                        FontId::proportional(12.0),
                        Color32::from_rgb(140, 150, 170),
                    );
                }
            }
            SweepMode::Heatmap2D => {
                if let Some(hm) = sweep_2d_result {
                    let rows = hm.grid.len().max(1);
                    let cols = hm.grid.first().map_or(1, |r| r.len()).max(1);
                    let cell_w = plot_rect.width() / (cols as f32);
                    let cell_h = plot_rect.height() / (rows as f32);
                    let m_span = (hm.max_measure - hm.min_measure).max(1e-9);

                    for (r, row_vals) in hm.grid.iter().enumerate() {
                        for (c, &cell_opt) in row_vals.iter().enumerate() {
                            let x0 = plot_rect.left() + (c as f32) * cell_w;
                            // Row 0 is y_min at bottom
                            let y1 = plot_rect.bottom() - (r as f32) * cell_h;
                            let y0 = y1 - cell_h;
                            let cell_rect =
                                Rect::from_min_max(pos2(x0, y0), pos2(x0 + cell_w + 0.6, y1 + 0.6));

                            let color = if let Some(val) = cell_opt {
                                let t = ((val - hm.min_measure) / m_span).clamp(0.0, 1.0) as f32;
                                heatmap_colormap(t)
                            } else {
                                Color32::from_rgb(45, 22, 28)
                            };
                            painter.rect_filled(cell_rect, 0.0, color);
                        }
                    }

                    // Y-axis parameter labels
                    painter.text(
                        pos2(rect.left() + 6.0, plot_rect.top()),
                        egui::Align2::LEFT_TOP,
                        format!(
                            "{}={}",
                            hm.config.param_y_name,
                            format_compact_value(hm.config.y_max)
                        ),
                        FontId::monospace(9.0),
                        Color32::from_rgb(150, 165, 190),
                    );
                    painter.text(
                        pos2(rect.left() + 6.0, plot_rect.bottom()),
                        egui::Align2::LEFT_BOTTOM,
                        format!(
                            "{}={}",
                            hm.config.param_y_name,
                            format_compact_value(hm.config.y_min)
                        ),
                        FontId::monospace(9.0),
                        Color32::from_rgb(150, 165, 190),
                    );
                    painter.text(
                        pos2(rect.left() + 6.0, plot_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!(
                            "M: [{}..{}]",
                            format_compact_value(hm.min_measure),
                            format_compact_value(hm.max_measure)
                        ),
                        FontId::monospace(8.5),
                        Color32::from_rgb(0, 225, 255),
                    );

                    // Current (Param X, Param Y) crosshair handle
                    let cur_y_val = doc
                        .state
                        .parameters
                        .iter()
                        .find(|p| p.name == hm.config.param_y_name)
                        .map(|p| p.value)
                        .unwrap_or(hm.config.y_min);

                    let cur_x = to_screen_x(current_param_val);
                    let norm_y = ((cur_y_val - hm.config.y_min)
                        / (hm.config.y_max - hm.config.y_min).max(1e-6))
                    .clamp(0.0, 1.0) as f32;
                    let cur_y = plot_rect.bottom() - norm_y * plot_rect.height();

                    painter.line_segment(
                        [
                            pos2(cur_x, plot_rect.top()),
                            pos2(cur_x, plot_rect.bottom()),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 160)),
                    );
                    painter.line_segment(
                        [
                            pos2(plot_rect.left(), cur_y),
                            pos2(plot_rect.right(), cur_y),
                        ],
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 160)),
                    );
                    painter.circle_stroke(
                        pos2(cur_x, cur_y),
                        6.5,
                        Stroke::new(1.8_f32, Color32::WHITE),
                    );
                    painter.circle_filled(pos2(cur_x, cur_y), 3.5, Color32::from_rgb(255, 220, 60));

                    // 2D direct drag manipulation
                    if response.dragged() || response.clicked() {
                        if let Some(ptr) = response.interact_pointer_pos() {
                            let new_px = screen_to_param(ptr.x);
                            let ny = ((plot_rect.bottom() - ptr.y) / plot_rect.height().max(1.0))
                                .clamp(0.0, 1.0) as f64;
                            let new_py = hm.config.y_min + ny * (hm.config.y_max - hm.config.y_min);
                            response_action.param_changed =
                                Some((hm.config.param_x_name.clone(), new_px));
                            response_action.param_y_changed =
                                Some((hm.config.param_y_name.clone(), new_py));
                        }
                    }
                } else {
                    painter.text(
                        plot_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Calculating 2D parameter map...",
                        FontId::proportional(12.0),
                        Color32::from_rgb(140, 150, 170),
                    );
                }
            }
        }

        // Horizontal axis labels
        painter.text(
            pos2(plot_rect.left(), plot_rect.bottom() + 4.0),
            egui::Align2::LEFT_TOP,
            format_compact_value(p_min),
            FontId::monospace(9.5),
            Color32::from_rgb(120, 130, 150),
        );
        painter.text(
            pos2(plot_rect.right(), plot_rect.bottom() + 4.0),
            egui::Align2::RIGHT_TOP,
            format_compact_value(p_max),
            FontId::monospace(9.5),
            Color32::from_rgb(120, 130, 150),
        );

        // Parameter name label at center bottom
        painter.text(
            pos2(plot_rect.center().x, plot_rect.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            &doc.state.sweep_config.parameter_name,
            FontId::monospace(11.0),
            Color32::from_rgb(200, 210, 230),
        );

        // Direct manipulation: Dragging horizontally updates parameter live in 1D modes
        if *sweep_mode != SweepMode::Heatmap2D && (response.dragged() || response.clicked()) {
            if let Some(ptr) = response.interact_pointer_pos() {
                let new_param_val = screen_to_param(ptr.x);
                response_action.param_changed =
                    Some((doc.state.sweep_config.parameter_name.clone(), new_param_val));
            }
        }
    });

    response_action
}

fn heatmap_colormap(t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    // Smooth perceptual dark-indigo -> teal -> emerald -> gold colormap
    if t < 0.33 {
        let u = t / 0.33;
        Color32::from_rgb(
            (18.0 + u * 10.0) as u8,
            (24.0 + u * 110.0) as u8,
            (58.0 + u * 115.0) as u8,
        )
    } else if t < 0.66 {
        let u = (t - 0.33) / 0.33;
        Color32::from_rgb(
            (28.0 + u * 50.0) as u8,
            (134.0 + u * 85.0) as u8,
            (173.0 - u * 55.0) as u8,
        )
    } else {
        let u = (t - 0.66) / 0.34;
        Color32::from_rgb(
            (78.0 + u * 177.0) as u8,
            (219.0 + u * 15.0) as u8,
            (118.0 - u * 48.0) as u8,
        )
    }
}
