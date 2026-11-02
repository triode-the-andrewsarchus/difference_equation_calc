use crate::ast::Equation;
use crate::document::{Document, ParameterDoc};
use crate::ui::format::format_compact_value;
use crate::ui::icons::Icon;
use egui::{pos2, Color32, Stroke, Ui};

pub fn render_parameters_panel(ui: &mut Ui, doc: &mut Document, parsed_equations: &[Equation]) {
    let mut delete_param_idx = None;
    let mut add_param = false;

    let clamp_th = doc.state.clamping_mode.threshold();
    let params_map = doc.state.get_parameters_map();

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Parameters").size(14.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::image_and_text(
                        Icon::Plus.image_tinted(11.0, Color32::from_rgb(200, 210, 230)),
                        "Param",
                    ))
                    .on_hover_text("Add new named parameter")
                    .clicked()
                {
                    add_param = true;
                }
            });
        });

        ui.add_space(4.0);

        if doc.state.parameters.is_empty() {
            ui.label(
                egui::RichText::new(
                    "No parameters defined. Promote a number in an equation or add one above.",
                )
                .size(11.0)
                .color(Color32::from_rgb(130, 140, 160))
                .italics(),
            );
        } else {
            let active_sweep_param = doc.state.sweep_config.parameter_name.clone();
            let mut new_sweep_param: Option<(String, f64, f64)> = None;
            let param_count = doc.state.parameters.len();
            for i in 0..param_count {
                let p = &mut doc.state.parameters[i];

                // Precompute clamping hazard zones for this parameter if clamping is enabled
                let clamping_zones = if let Some(th) = clamp_th {
                    crate::sim::analyze_parameter_clamping_zones(
                        parsed_equations,
                        &doc.state.initial_conditions,
                        &params_map,
                        &p.name,
                        p.min,
                        p.max,
                        th,
                        20,
                        60,
                    )
                } else {
                    Vec::new()
                };

                let norm_val = if p.max > p.min {
                    ((p.value - p.min) / (p.max - p.min)) as f32
                } else {
                    0.0_f32
                };

                let current_clipping = clamping_zones.iter().find(|zone| {
                    norm_val >= zone.start_norm - 0.015 && norm_val <= zone.end_norm + 0.015
                });

                let is_swept = p.name == active_sweep_param;

                ui.horizontal(|ui| {
                    let badge_text = if is_swept {
                        format!("● {}", p.name)
                    } else {
                        p.name.clone()
                    };
                    let badge_color = if is_swept {
                        Color32::from_rgb(0, 225, 255)
                    } else {
                        Color32::from_rgb(255, 200, 100)
                    };

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(badge_text)
                                    .monospace()
                                    .strong()
                                    .size(11.5)
                                    .color(badge_color),
                            )
                            .frame(false),
                        )
                        .on_hover_text("Click to select this parameter as the active Sweep axis")
                        .clicked()
                    {
                        new_sweep_param = Some((p.name.clone(), p.min, p.max));
                    }

                    let slider = egui::Slider::new(&mut p.value, p.min..=p.max)
                        .step_by(p.step)
                        .show_value(true)
                        .max_decimals(4);

                    let slider_resp = ui.add(slider);

                    // Render clamping danger zones along the bottom of the slider track
                    if !clamping_zones.is_empty() {
                        let track_left = slider_resp.rect.left() + 6.0;
                        let track_right = slider_resp.rect.right() - 6.0;
                        let track_w = (track_right - track_left).max(1.0);
                        let y_hazard = slider_resp.rect.bottom() - 1.5;

                        for zone in &clamping_zones {
                            let z_left = track_left + zone.start_norm * track_w;
                            let z_right = track_left + zone.end_norm * track_w;
                            if z_right > z_left {
                                ui.painter().line_segment(
                                    [pos2(z_left, y_hazard), pos2(z_right, y_hazard)],
                                    Stroke::new(
                                        2.5_f32,
                                        Color32::from_rgba_unmultiplied(255, 60, 60, 210),
                                    ),
                                );
                            }
                        }
                    }

                    // Indicator if current parameter value produces clamping/clipping
                    if let Some(zone) = current_clipping {
                        let peak_str = if zone.max_peak.is_finite() {
                            format_compact_value(zone.max_peak)
                        } else {
                            "DIVERGES".to_string()
                        };
                        ui.label(
                            egui::RichText::new("CLIPS")
                                .size(9.5)
                                .monospace()
                                .strong()
                                .color(Color32::from_rgb(255, 80, 80)),
                        )
                        .on_hover_text(format!(
                            "Current value causes simulation to exceed limit (Peak: {})",
                            peak_str
                        ));
                    }

                    // Interactive min..max range bounds
                    let mut min_v = p.min;
                    let mut max_v = p.max;
                    if ui
                        .add(
                            egui::DragValue::new(&mut min_v)
                                .speed(0.02)
                                .prefix("[")
                                .max_decimals(2),
                        )
                        .on_hover_text("Drag to adjust parameter minimum bound")
                        .changed()
                    {
                        p.min = min_v.min(p.max - 1e-4);
                        p.value = p.value.clamp(p.min, p.max);
                        p.step = ((p.max - p.min) / 250.0).max(1e-5);
                    }
                    if ui
                        .add(
                            egui::DragValue::new(&mut max_v)
                                .speed(0.02)
                                .prefix("..")
                                .suffix("]")
                                .max_decimals(2),
                        )
                        .on_hover_text("Drag to adjust parameter maximum bound")
                        .changed()
                    {
                        p.max = max_v.max(p.min + 1e-4);
                        p.value = p.value.clamp(p.min, p.max);
                        p.step = ((p.max - p.min) / 250.0).max(1e-5);
                    }

                    if ui
                        .add(egui::Button::image(
                            Icon::Close.image_tinted(9.0, Color32::from_rgb(180, 190, 210)),
                        ))
                        .on_hover_text("Delete parameter")
                        .clicked()
                    {
                        delete_param_idx = Some(i);
                    }
                });
            }

            if let Some((name, min_v, max_v)) = new_sweep_param {
                doc.state.sweep_config.parameter_name = name;
                doc.state.sweep_config.min_val = min_v;
                doc.state.sweep_config.max_val = max_v;
            }
        }

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // Direct manipulation of Initial Conditions at n = 0
        let mut zero_all_inits = false;
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("Initial Conditions (n = 0)")
                    .size(13.0)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Zero All")
                    .on_hover_text("Set all initial conditions at n = 0 to 0.0")
                    .clicked()
                {
                    zero_all_inits = true;
                }
            });
        });
        ui.add_space(2.0);

        // Ensure all parsed equation target variables have an entry in initial_conditions
        for eq in parsed_equations {
            doc.state
                .initial_conditions
                .entry(eq.target_var.clone())
                .or_insert(0.0);
        }
        if zero_all_inits {
            for val in doc.state.initial_conditions.values_mut() {
                *val = 0.0;
            }
        }

        let mut init_keys: Vec<String> = doc.state.initial_conditions.keys().cloned().collect();
        init_keys.sort();

        if init_keys.is_empty() {
            ui.label(
                egui::RichText::new("No initial conditions specified.")
                    .size(11.0)
                    .color(Color32::from_rgb(130, 140, 160))
                    .italics(),
            );
        } else {
            for var in init_keys {
                if let Some(val) = doc.state.initial_conditions.get_mut(&var) {
                    let is_init_clamped = if let Some(th) = clamp_th {
                        val.abs() >= th - 1e-4
                    } else {
                        false
                    };

                    let label_color = if is_init_clamped {
                        Color32::from_rgb(255, 90, 90)
                    } else {
                        Color32::from_rgb(70, 240, 150)
                    };

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}[0]", var))
                                .monospace()
                                .strong()
                                .color(label_color),
                        );

                        let mut drag = egui::DragValue::new(val)
                            .speed(0.01)
                            .custom_formatter(|n, _| format_compact_value(n));

                        if let Some(th) = clamp_th {
                            drag = drag.range(-th..=th);
                        }

                        if ui.add(drag).changed() {
                            if let Some(th) = clamp_th {
                                *val = val.clamp(-th, th);
                            }
                        }

                        if is_init_clamped {
                            ui.label(
                                egui::RichText::new("[RAIL]")
                                    .size(9.0)
                                    .monospace()
                                    .strong()
                                    .color(Color32::from_rgb(255, 90, 90)),
                            )
                            .on_hover_text("Initial condition is at or outside the clamping rail.");
                        }
                    });
                }
            }
        }
    });

    if let Some(idx) = delete_param_idx {
        doc.state.parameters.remove(idx);
    }

    if add_param {
        let name = format!("k{}", doc.state.parameters.len() + 1);
        doc.state.parameters.push(ParameterDoc {
            name,
            value: 0.1,
            min: 0.0,
            max: 1.0,
            step: 0.005,
        });
    }
}
