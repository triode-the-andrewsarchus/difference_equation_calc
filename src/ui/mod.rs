pub mod equation_view;
pub mod icons;
pub mod measure_view;
pub mod params_view;
pub mod phase_view;
pub mod search_bar;
pub mod sweep_view;
pub mod waveforms;

use crate::deps::analyze_dependencies;
use crate::document::{Document, DocumentState};
use crate::parser::parse_expression_str;
use crate::presets::ALL_PRESETS;
use crate::query::{execute_search, SearchResult};
use crate::sim::{run_simulation, SimulationResult};
use crate::sweep::{
    execute_attractor_sweep, execute_parameter_sweep, AttractorSweepConfig, AttractorSweepResult,
    SweepMode, SweepResult,
};
use crate::ui::equation_view::render_equation_list;
use crate::ui::icons::Icon;
use crate::ui::measure_view::render_measurements_panel;
use crate::ui::params_view::render_parameters_panel;
use crate::ui::phase_view::{render_phase_panel, PhaseViewState};
use crate::ui::search_bar::render_search_bar;
use crate::ui::sweep_view::render_sweep_panel;
use crate::ui::waveforms::{render_waveform_grid, WaveformRowInfo, WaveformViewport, TRACE_COLORS};
use egui::{vec2, Color32};
use std::collections::HashMap;

pub struct DifferenceEquationApp {
    pub doc: Document,
    pub viewport: WaveformViewport,
    pub sim_result: SimulationResult,
    pub ghost_sim: Option<SimulationResult>,
    pub is_ghost_pinned: bool,
    pub cursor_step: Option<usize>,
    pub phase_state: PhaseViewState,
    pub sweep_mode: SweepMode,
    pub bifurcation_var: String,
    pub attractor_sweep_result: Option<AttractorSweepResult>,
    pub search_result: SearchResult,
    pub sweep_result: SweepResult,
    pub status_message: Option<(String, f32)>, // message, display timer
    pub cached_state: DocumentState,
    cached_sweep_mode: SweepMode,
    cached_bifurcation_var: String,
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
            ghost_sim: None,
            is_ghost_pinned: false,
            cursor_step: None,
            phase_state: PhaseViewState {
                var_x: "a".to_string(),
                var_y: "b".to_string(),
                show_ghost: true,
            },
            sweep_mode: SweepMode::Measurement,
            bifurcation_var: "a".to_string(),
            attractor_sweep_result: None,
            search_result: SearchResult::default(),
            sweep_result: SweepResult {
                config: Default::default(),
                points: Vec::new(),
                min_measure: 0.0,
                max_measure: 1.0,
            },
            status_message: None,
            cached_sweep_mode: SweepMode::Measurement,
            cached_bifurcation_var: "a".to_string(),
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

        // 3. Run parameter sweep or attractor bifurcation sweep
        match self.sweep_mode {
            SweepMode::Measurement => {
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
            }
            SweepMode::AttractorBifurcation => {
                let config = AttractorSweepConfig {
                    parameter_name: self.doc.state.sweep_config.parameter_name.clone(),
                    variable_name: self.bifurcation_var.clone(),
                    min_val: self.doc.state.sweep_config.min_val,
                    max_val: self.doc.state.sweep_config.max_val,
                    slices: 140,
                    total_steps: 300,
                    tail_steps: 70,
                };
                self.attractor_sweep_result = Some(execute_attractor_sweep(
                    &config,
                    &eqs,
                    &self.doc.state.initial_conditions,
                    &params,
                ));
            }
        }

        self.cached_state = self.doc.state.clone();
        self.cached_sweep_mode = self.sweep_mode;
        self.cached_bifurcation_var = self.bifurcation_var.clone();
    }
}

impl eframe::App for DifferenceEquationApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Decrement status message timer
        if let Some((_, ref mut timer)) = self.status_message {
            *timer -= ctx.input(|i| i.stable_dt);
            if *timer <= 0.0 {
                self.status_message = None;
            }
        }

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

        // Top Header Panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("Interacting with Behavior")
                        .strong()
                        .size(17.0)
                        .color(Color32::from_rgb(220, 230, 255)),
                );

                ui.label(
                    egui::RichText::new("· Discrete Dynamical Systems")
                        .italics()
                        .size(12.0)
                        .color(Color32::from_rgb(130, 140, 160)),
                );

                ui.separator();

                // Undo / Redo
                let undo_btn = egui::Button::image_and_text(
                    Icon::Undo.image_tinted(13.0, Color32::from_rgb(200, 210, 230)),
                    "Undo",
                );
                if ui
                    .add_enabled(self.doc.can_undo(), undo_btn)
                    .on_hover_text("Undo (Ctrl+Z)")
                    .clicked()
                {
                    self.doc.undo();
                }

                let redo_btn = egui::Button::image_and_text(
                    Icon::Redo.image_tinted(13.0, Color32::from_rgb(200, 210, 230)),
                    "Redo",
                );
                if ui
                    .add_enabled(self.doc.can_redo(), redo_btn)
                    .on_hover_text("Redo (Ctrl+Y)")
                    .clicked()
                {
                    self.doc.redo();
                }

                ui.separator();

                // Presets Dropdown
                egui::ComboBox::from_id_salt("presets_picker")
                    .selected_text("Presets ▾")
                    .show_ui(ui, |ui| {
                        for preset in ALL_PRESETS {
                            if ui
                                .button(preset.name)
                                .on_hover_text(preset.description)
                                .clicked()
                            {
                                self.doc = Document::new((preset.state_builder)());
                                self.viewport.x_min = 0.0;
                                self.viewport.x_max = self.doc.state.sample_count as f64;
                                self.phase_state.var_x = preset.default_phase_pair.0.to_string();
                                self.phase_state.var_y = preset.default_phase_pair.1.to_string();
                                self.bifurcation_var = preset.default_phase_pair.0.to_string();
                                self.recompute_all();
                                self.status_message =
                                    Some((format!("Loaded preset: {}", preset.name), 3.0));
                            }
                        }
                    });

                // Ghost Pin Toggle Button
                let pin_label = if self.is_ghost_pinned {
                    "Ghost Pinned"
                } else {
                    "Pin Ghost"
                };
                let pin_color = if self.is_ghost_pinned {
                    Color32::from_rgb(0, 220, 255)
                } else {
                    Color32::from_rgb(200, 210, 230)
                };
                let pin_btn = egui::Button::image_and_text(
                    Icon::Pin.image_tinted(13.0, pin_color),
                    pin_label,
                );
                if ui
                    .add(pin_btn)
                    .on_hover_text("Pin current simulation as ghost reference for comparison")
                    .clicked()
                {
                    if self.is_ghost_pinned {
                        self.is_ghost_pinned = false;
                        self.ghost_sim = None;
                        self.status_message = Some(("Ghost unpinned.".to_string(), 2.0));
                    } else {
                        self.is_ghost_pinned = true;
                        self.ghost_sim = Some(self.sim_result.clone());
                        self.status_message =
                            Some(("Ghost simulation pinned for comparison.".to_string(), 2.5));
                    }
                }

                ui.separator();

                // Save JSON
                let save_btn = egui::Button::image_and_text(
                    Icon::Save.image_tinted(13.0, Color32::from_rgb(200, 210, 230)),
                    "Save",
                );
                if ui
                    .add(save_btn)
                    .on_hover_text("Save session to JSON file")
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .set_file_name("system_behavior.json")
                        .save_file()
                    {
                        if let Ok(json) = self.doc.to_json() {
                            let _ = std::fs::write(path, json);
                            self.status_message =
                                Some(("Session saved successfully.".to_string(), 3.0));
                        }
                    }
                }

                // Load JSON
                let load_btn = egui::Button::image_and_text(
                    Icon::Folder.image_tinted(13.0, Color32::from_rgb(200, 210, 230)),
                    "Load",
                );
                if ui
                    .add(load_btn)
                    .on_hover_text("Load session from JSON file")
                    .clicked()
                {
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

        // Bottom panels: Parameters, Measurements, Phase Portrait, and Parameter Sweep
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(true)
            .default_height(320.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);

                    // Row 1: Parameters + Measurements + Phase Space
                    ui.horizontal_top(|ui| {
                        // Parameters column
                        ui.allocate_ui_with_layout(
                            vec2(260.0, 130.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                render_parameters_panel(ui, &mut self.doc);
                            },
                        );

                        // Calculate available width for Measurements and Phase Portrait
                        let rem_w = ui.available_width();
                        let phase_w = 300.0_f32.min(rem_w * 0.42);
                        let measure_w = (rem_w - phase_w - 8.0).max(200.0);

                        // Measurements column
                        ui.allocate_ui_with_layout(
                            vec2(measure_w, 130.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                render_measurements_panel(
                                    ui,
                                    &mut self.doc,
                                    &self.sim_result,
                                    self.ghost_sim.as_ref(),
                                );
                            },
                        );

                        // Phase Portrait column
                        ui.allocate_ui_with_layout(
                            vec2(phase_w, 130.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                let phase_click = render_phase_panel(
                                    ui,
                                    &self.sim_result,
                                    self.ghost_sim.as_ref(),
                                    self.cursor_step,
                                    self.doc.state.selection,
                                    &mut self.phase_state,
                                );
                                if let Some(step) = phase_click {
                                    self.cursor_step = Some(step);
                                }
                            },
                        );
                    });

                    ui.add_space(6.0);

                    // Row 2: Parameter Sweep vs Measurement / Attractor 2D Curve
                    let mut available_vars: Vec<String> =
                        self.sim_result.series.keys().cloned().collect();
                    available_vars.sort();

                    let sweep_resp = render_sweep_panel(
                        ui,
                        &mut self.doc,
                        &self.sweep_result,
                        self.attractor_sweep_result.as_ref(),
                        &mut self.sweep_mode,
                        &mut self.bifurcation_var,
                        &available_vars,
                    );
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

                let ghost_values = self
                    .ghost_sim
                    .as_ref()
                    .and_then(|g| g.series.get(&target_name))
                    .map(|s| &s.values[..]);

                waveform_rows.push(WaveformRowInfo {
                    name: Box::leak(target_name.into_boxed_str()),
                    color,
                    values,
                    ghost_values,
                });
            }

            // Split into Left (Equations) and Right (Waveforms)
            let eq_col_width = 390.0;
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
                                Some(&self.sim_result),
                                self.cursor_step,
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

                            if let Some(hovered) = wf_resp.hovered_step {
                                self.cursor_step = Some(hovered);
                            }

                            if let Some(new_sel) = wf_resp.new_selection {
                                self.doc.state.selection = new_sel;
                            }
                        },
                    );
                });
            });
        });

        // Check if document state or sweep mode changed to trigger live recomputation
        if self.doc.state != self.cached_state
            || self.sweep_mode != self.cached_sweep_mode
            || self.bifurcation_var != self.cached_bifurcation_var
        {
            self.recompute_all();
        }
    }
}
