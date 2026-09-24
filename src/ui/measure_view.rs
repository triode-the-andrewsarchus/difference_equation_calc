use crate::document::Document;
use crate::measure::{calculate_measurement, MeasurementSpec, MeasurementType};
use crate::sim::SimulationResult;
use crate::ui::waveforms::TRACE_COLORS;
use egui::{vec2, Color32, Ui};

pub fn render_measurements_panel(ui: &mut Ui, doc: &mut Document, sim_result: &SimulationResult) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Measurements").size(14.0).strong());
            if let Some((s0, s1)) = doc.state.selection {
                ui.label(
                    egui::RichText::new(format!(
                        "Selection: n = {} .. {} (Δn = {})",
                        s0,
                        s1,
                        s1.saturating_sub(s0) + 1
                    ))
                    .monospace()
                    .color(Color32::from_rgb(255, 230, 80)),
                );

                if ui.small_button("Clear Selection").clicked() {
                    doc.state.selection = None;
                }
            } else {
                ui.label(
                    egui::RichText::new(format!("Whole timeline (0 .. {})", sim_result.sample_count.saturating_sub(1)))
                        .italics()
                        .color(Color32::from_rgb(140, 150, 170)),
                );
            }
        });

        ui.add_space(4.0);

        let (s0, s1) = doc
            .state
            .selection
            .unwrap_or((0, sim_result.sample_count.saturating_sub(1)));

        // Iterate variables that exist in simulation result
        let mut var_names: Vec<_> = sim_result.series.keys().cloned().collect();
        var_names.sort();

        if var_names.is_empty() {
            ui.label("No simulation data available.");
            return;
        }

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (v_idx, var) in var_names.iter().enumerate() {
                    let color = TRACE_COLORS[v_idx % TRACE_COLORS.len()];

                    ui.vertical(|ui| {
                        // Header card for variable
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(vec2(8.0, 8.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 4.0, color);
                            ui.label(
                                egui::RichText::new(var)
                                    .strong()
                                    .color(color)
                                    .size(13.0),
                            );
                        });

                        egui::Frame::NONE
                            .fill(Color32::from_rgb(22, 24, 32))
                            .corner_radius(4.0)
                            .inner_margin(6.0)
                            .show(ui, |ui| {
                                egui::Grid::new(format!("grid_{var}"))
                                    .num_columns(3)
                                    .spacing([6.0, 2.0])
                                    .show(ui, |ui| {
                                        let types_to_show = [
                                            MeasurementType::Mean,
                                            MeasurementType::Min,
                                            MeasurementType::Max,
                                            MeasurementType::Range,
                                            MeasurementType::Rms,
                                            MeasurementType::Delta,
                                        ];

                                        for m_type in types_to_show {
                                            let spec = MeasurementSpec::new(var, m_type);
                                            let val_str = match calculate_measurement(&spec, sim_result, s0, s1) {
                                                Ok(v) => format!("{:.4}", v),
                                                Err(_) => "---".to_string(),
                                            };

                                            ui.label(
                                                egui::RichText::new(m_type.name())
                                                    .size(10.5)
                                                    .color(Color32::from_rgb(160, 170, 190)),
                                            );
                                            ui.label(
                                                egui::RichText::new(val_str)
                                                    .monospace()
                                                    .size(10.5)
                                                    .color(Color32::WHITE),
                                            );

                                            let is_current_sweep = doc.state.sweep_config.measurement == spec;
                                            let btn_text = if is_current_sweep { "● Plotted" } else { "Plot" };
                                            let btn_color = if is_current_sweep {
                                                Color32::from_rgb(0, 220, 255)
                                            } else {
                                                Color32::from_rgb(100, 110, 130)
                                            };

                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new(btn_text)
                                                            .size(9.0)
                                                            .color(btn_color),
                                                    )
                                                    .fill(Color32::from_rgb(30, 33, 44)),
                                                )
                                                .on_hover_text(format!("Sweep this measurement against parameter vs {}", doc.state.sweep_config.parameter_name))
                                                .clicked()
                                            {
                                                doc.state.sweep_config.measurement = spec;
                                            }

                                            ui.end_row();
                                        }
                                    });
                            });
                    });

                    ui.add_space(8.0);
                }
            });
        });
    });
}
