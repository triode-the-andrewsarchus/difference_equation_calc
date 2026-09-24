pub mod equation_view;
pub mod measure_view;
pub mod params_view;
pub mod search_bar;
pub mod sweep_view;
pub mod waveforms;

use crate::deps::analyze_dependencies;
use crate::document::{Document, DocumentState};
use crate::parser::parse_expression_str;
use crate::query::{execute_search, SearchResult};
use crate::sim::{run_simulation, SimulationResult};
use crate::sweep::{execute_parameter_sweep, SweepResult};
use crate::ui::equation_view::render_equation_list;
use crate::ui::measure_view::render_measurements_panel;
use crate::ui::params_view::render_parameters_panel;
use crate::ui::search_bar::render_search_bar;
use crate::ui::sweep_view::render_sweep_panel;
use crate::ui::waveforms::{render_waveform_grid, WaveformRowInfo, WaveformViewport, TRACE_COLORS};
use egui::{vec2, Color32};
use std::collections::HashMap;

pub struct DifferenceEquationApp {
    pub doc: Document,
    pub viewport: WaveformViewport,
    pub sim_result: SimulationResult,
    pub search_result: SearchResult,
    pub sweep_result: SweepResult,
    pub status_message: Option<(String, f32)>, // message, display timer
    pub cached_state: DocumentState,
}

impl DifferenceEquationApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let doc = Document::new(DocumentState::victor_oscillator_demo());
        let mut app = Self {
            cached_state: doc.state.clone(),
            doc,
            viewport: WaveformViewport {
                x_min: 0.0,
                x_max: 600.0,
                is_dragging_selection: false,
                drag_start_n: None,
            },
            sim_result: SimulationResult {
                series: HashMap::new(),
                sample_count: 600,
                failure: None,
            },
            search_result: SearchResult::default(),
            sweep_result: SweepResult {
                config: Default::default(),
                points: Vec::new(),
                min_measure: 0.0,
                max_measure: 1.0,
            },
            status_message: None,
        };

        app.recompute_all();
        app
    }

    pub fn recompute_all(&mut self) {
        let (eqs, _) = self.doc.state.get_parsed_equations();
        let params = self.doc.state.get_parameters_map();

        // 1. Run dynamical simulation
        if let Ok(sim) = run_simulation(
            &eqs,
            &self.doc.state.initial_conditions,
            &params,
            self.doc.state.sample_count,
        ) {
            self.sim_result = sim;
        }

        // 2. Run behavioral search
        let trimmed_query = self.doc.state.search_query.trim();
        if !trimmed_query.is_empty() {
            if let Ok(query_expr) = parse_expression_str(trimmed_query) {
                self.search_result = execute_search(&query_expr, &self.sim_result, &params);
            } else {
                self.search_result = SearchResult::empty(self.doc.state.sample_count);
            }
        } else {
            self.search_result = SearchResult::empty(self.doc.state.sample_count);
        }

        // 3. Run parameter sweep
        let (s0, s1) = self
            .doc
            .state
            .selection
            .unwrap_or((0, self.doc.state.sample_count.saturating_sub(1)));

        self.sweep_result = execute_parameter_sweep(
            &self.doc.state.sweep_config,
            &eqs,
            &self.doc.state.initial_conditions,
            &params,
            self.doc.state.sample_count,
            s0,
            s1,
        );

        self.cached_state = self.doc.state.clone();
    }
}

impl eframe::App for DifferenceEquationApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check keyboard shortcuts
        ctx.input(|i| {
            if i.modifiers.command || i.modifiers.ctrl {
                if i.key_pressed(egui::Key::Z) {
                    if i.modifiers.shift {
                        self.doc.redo();
                    } else {
                        self.doc.undo();
                    }
                } else if i.key_pressed(egui::Key::Y) {
                    self.doc.redo();
                }
            } else if i.key_pressed(egui::Key::Escape) {
                self.doc.state.selection = None;
            }
        });

        // Dark theme background styling
        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = Color32::from_rgb(14, 16, 22);
        style.visuals.window_fill = Color32::from_rgb(18, 20, 26);
        ctx.set_style(style);

        // Top Header
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("INTERACTING WITH BEHAVIOR")
                        .strong()
                        .color(Color32::from_rgb(0, 220, 255))
                        .size(14.0),
                );
                ui.label(
                    egui::RichText::new("· Discrete Dynamical Systems")
                        .size(11.5)
                        .color(Color32::from_rgb(140, 150, 170)),
                );

                ui.separator();

                // Undo / Redo
                if ui
                    .add_enabled(self.doc.can_undo(), egui::Button::new("↶ Undo"))
                    .on_hover_text("Undo (Ctrl+Z)")
                    .clicked()
                {
                    self.doc.undo();
                }
                if ui
                    .add_enabled(self.doc.can_redo(), egui::Button::new("↷ Redo"))
                    .on_hover_text("Redo (Ctrl+Y)")
                    .clicked()
                {
                    self.doc.redo();
                }

                ui.separator();

                // Reset to Victor Demo
                if ui
                    .button("Reset to Victor Demo")
                    .on_hover_text(
                        "Reset equations and parameters to Bret Victor's oscillator demo",
                    )
                    .clicked()
                {
                    self.doc.checkpoint();
                    self.doc.state = DocumentState::victor_oscillator_demo();
                    self.viewport.x_min = 0.0;
                    self.viewport.x_max = 600.0;
                    self.recompute_all();
                }

                ui.separator();

                // Save JSON
                if ui.button("Save JSON").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .set_file_name("system_behavior.json")
                        .save_file()
                    {
                        if let Ok(json) = self.doc.to_json() {
                            let _ = std::fs::write(path, json);
                            self.status_message =
                                Some(("Project saved successfully.".to_string(), 3.0));
                        }
                    }
                }

                // Load JSON
                if ui.button("Load JSON").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                    {
                        if let Ok(content) = std::fs::read_to_string(path) {
                            if let Ok(new_doc) = Document::from_json(&content) {
                                self.doc = new_doc;
                                self.recompute_all();
                                self.status_message =
                                    Some(("Project loaded successfully.".to_string(), 3.0));
                            }
                        }
                    }
                }

                ui.separator();

                // Timeline samples slider
                ui.label("Timeline:");
                let mut samples = self.doc.state.sample_count;
                if ui
                    .add(
                        egui::Slider::new(&mut samples, 100..=3000)
                            .step_by(50.0)
                            .suffix(" steps"),
                    )
                    .changed()
                {
                    self.doc.state.sample_count = samples;
                    self.viewport.x_max = self.viewport.x_max.min(samples as f64);
                }

                // Status message
                if let Some((ref msg, _)) = self.status_message {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(msg)
                                .color(Color32::from_rgb(100, 240, 150))
                                .size(11.0),
                        );
                    });
                }
            });
            ui.add_space(3.0);
        });

        // Behavioral Search Bar
        egui::TopBottomPanel::top("search_panel").show(ctx, |ui| {
            ui.add_space(2.0);
            let search_parse_err = if self.doc.state.search_query.trim().is_empty() {
                None
            } else {
                parse_expression_str(&self.doc.state.search_query).err()
            };
            render_search_bar(
                ui,
                &mut self.doc,
                &self.search_result,
                search_parse_err.as_ref(),
            );
            ui.add_space(2.0);
        });

        // Bottom panels: Parameters, Measurements, and Parameter Sweep
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .default_height(280.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);

                    // Row 1: Parameters + Measurements
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width() * 0.38, 120.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                render_parameters_panel(ui, &mut self.doc);
                            },
                        );

                        ui.allocate_ui_with_layout(
                            vec2(ui.available_width(), 120.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                render_measurements_panel(ui, &mut self.doc, &self.sim_result);
                            },
                        );
                    });

                    ui.add_space(6.0);

                    // Row 2: Parameter Sweep vs Measurement 2D Curve
                    let sweep_resp = render_sweep_panel(ui, &mut self.doc, &self.sweep_result);
                    if let Some((pname, pval)) = sweep_resp.param_changed {
                        self.doc.state.set_parameter_value(&pname, pval);
                    }

                    ui.add_space(4.0);
                });
            });

        // Center Panel: Aligned Equations (Left) and Waveforms (Right)
        egui::CentralPanel::default().show(ctx, |ui| {
            let (parsed_eqs, parse_errors) = self.doc.state.get_parsed_equations();
            let dep_err = analyze_dependencies(&parsed_eqs).err();

            // Prepare row info for waveform rendering
            let mut waveform_rows = Vec::new();
            for eq_doc in &self.doc.state.equations {
                let color = TRACE_COLORS[eq_doc.color_idx % TRACE_COLORS.len()];
                // Extract target variable name
                let target_name = parsed_eqs
                    .iter()
                    .find(|e| {
                        e.target_var.starts_with(&eq_doc.source[..1])
                            || eq_doc.source.starts_with(&e.target_var)
                    })
                    .map(|e| e.target_var.clone())
                    .unwrap_or_else(|| {
                        let name_part = eq_doc.source.split('[').next().unwrap_or("?").trim();
                        name_part.to_string()
                    });

                let values = if let Some(series) = self.sim_result.series.get(&target_name) {
                    &series.values[..]
                } else {
                    &[]
                };

                waveform_rows.push(WaveformRowInfo {
                    name: Box::leak(target_name.into_boxed_str()),
                    color,
                    values,
                });
            }

            // Split into Left (Equations) and Right (Waveforms)
            let eq_col_width = 380.0;
            let available_width = ui.available_width();
            let wf_width = (available_width - eq_col_width - 8.0).max(200.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Left Column: Equations
                    ui.allocate_ui_with_layout(
                        vec2(eq_col_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            render_equation_list(
                                ui,
                                &mut self.doc,
                                &parse_errors,
                                dep_err.as_ref(),
                            );
                        },
                    );

                    ui.separator();

                    // Right Column: Aligned Waveforms
                    ui.allocate_ui_with_layout(
                        vec2(wf_width, ui.available_height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            let wf_resp = render_waveform_grid(
                                ui,
                                &mut self.viewport,
                                &waveform_rows,
                                &self.sim_result,
                                &self.search_result,
                                self.doc.state.selection,
                            );

                            if let Some(new_sel) = wf_resp.new_selection {
                                self.doc.state.selection = new_sel;
                            }
                        },
                    );
                });
            });
        });

        // Check if document state changed to trigger live recomputation
        if self.doc.state != self.cached_state {
            self.recompute_all();
        }
    }
}
