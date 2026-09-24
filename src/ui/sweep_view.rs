use crate::document::Document;
use crate::sweep::{AttractorSweepResult, SweepMode, SweepResult};
use egui::{pos2, vec2, Color32, FontId, Rect, Sense, Stroke, Ui};

pub struct SweepPlotResponse {
    pub param_changed: Option<(String, f64)>,
}

pub fn render_sweep_panel(
    ui: &mut Ui,
    doc: &mut Document,
    sweep_result: &SweepResult,
    attractor_sweep: Option<&AttractorSweepResult>,
    sweep_mode: &mut SweepMode,
    bifurcation_var: &mut String,
    available_vars: &[String],
) -> SweepPlotResponse {
    let mut response_action = SweepPlotResponse {
        param_changed: None,
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
            ui.selectable_value(sweep_mode, SweepMode::Measurement, "Measurement");
            ui.selectable_value(
                sweep_mode,
                SweepMode::AttractorBifurcation,
                "Bifurcation Orbit",
            );

            ui.separator();

            // Parameter selector
            ui.label("Parameter:");
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

            match *sweep_mode {
                SweepMode::Measurement => {
                    // Measurement display
                    ui.label(
                        egui::RichText::new(format!(
                            "Measurement: {}",
                            doc.state.sweep_config.measurement.display_name()
                        ))
                        .monospace()
                        .color(Color32::from_rgb(0, 220, 255)),
                    );
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

            // Min and max sweep range
            ui.label("Range:");
            ui.add(
                egui::DragValue::new(&mut doc.state.sweep_config.min_val)
                    .speed(0.01)
                    .prefix("min: "),
            );
            ui.add(
                egui::DragValue::new(&mut doc.state.sweep_config.max_val)
                    .speed(0.01)
                    .prefix("max: "),
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
                let m_min = sweep_result.min_measure;
                let m_max = sweep_result.max_measure;

                let to_screen_y = |mval: f64| -> f32 {
                    let norm = (mval - m_min) / (m_max - m_min).max(1e-6);
                    plot_rect.bottom() - (norm as f32) * plot_rect.height()
                };

                // Grid labels
                painter.text(
                    pos2(rect.left() + 6.0, plot_rect.top()),
                    egui::Align2::LEFT_TOP,
                    format!("{:.3}", m_max),
                    FontId::monospace(9.5),
                    Color32::from_rgb(120, 130, 150),
                );
                painter.text(
                    pos2(rect.left() + 6.0, plot_rect.bottom()),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:.3}", m_min),
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
                        "{} = {:.4}{}",
                        doc.state.sweep_config.parameter_name,
                        current_param_val,
                        interp_m.map_or("".to_string(), |m| format!(", M = {:.4}", m))
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
                    let s_min = attractor.state_min;
                    let s_max = attractor.state_max;

                    let to_screen_y = |sval: f64| -> f32 {
                        let norm = (sval - s_min) / (s_max - s_min).max(1e-6);
                        plot_rect.bottom() - (norm as f32) * plot_rect.height()
                    };

                    // Grid labels
                    painter.text(
                        pos2(rect.left() + 6.0, plot_rect.top()),
                        egui::Align2::LEFT_TOP,
                        format!("{:.3}", s_max),
                        FontId::monospace(9.5),
                        Color32::from_rgb(120, 130, 150),
                    );
                    painter.text(
                        pos2(rect.left() + 6.0, plot_rect.bottom()),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{:.3}", s_min),
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
                                "{} = {:.4}",
                                doc.state.sweep_config.parameter_name, current_param_val
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
        }

        // Horizontal axis labels
        painter.text(
            pos2(plot_rect.left(), plot_rect.bottom() + 4.0),
            egui::Align2::LEFT_TOP,
            format!("{:.3}", p_min),
            FontId::monospace(9.5),
            Color32::from_rgb(120, 130, 150),
        );
        painter.text(
            pos2(plot_rect.right(), plot_rect.bottom() + 4.0),
            egui::Align2::RIGHT_TOP,
            format!("{:.3}", p_max),
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

        // Direct manipulation: Dragging horizontally updates parameter live!
        if response.dragged() || response.clicked() {
            if let Some(ptr) = response.interact_pointer_pos() {
                let new_param_val = screen_to_param(ptr.x);
                response_action.param_changed =
                    Some((doc.state.sweep_config.parameter_name.clone(), new_param_val));
            }
        }
    });

    response_action
}
