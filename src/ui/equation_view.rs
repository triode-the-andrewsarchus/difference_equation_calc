use crate::ast::Span;
use crate::deps::DependencyError;
use crate::document::{Document, ParameterDoc};
use crate::lexer::{Lexer, TokenKind};
use crate::parser::ParseError;
use crate::sim::SimulationResult;
use crate::ui::format::format_compact_value;
use crate::ui::icons::Icon;
use crate::ui::waveforms::TRACE_COLORS;
use egui::{vec2, Color32, Ui};
use std::collections::HashMap;

pub struct EquationScrubAction {
    pub eq_id: u64,
    pub span: Span,
    pub new_value_str: String,
}

pub struct EquationPromoteAction {
    pub eq_id: u64,
    pub span: Span,
    pub param_name: String,
    pub param_value: f64,
}

pub fn render_equation_list(
    ui: &mut Ui,
    doc: &mut Document,
    parse_errors: &HashMap<u64, ParseError>,
    dep_error: Option<&DependencyError>,
    sim_result: Option<&SimulationResult>,
    cursor_step: Option<usize>,
) {
    let mut scrub_action = None;
    let mut promote_action = None;
    let mut delete_eq_id = None;
    let mut add_equation = false;
    let mut source_changes: Vec<(usize, String)> = Vec::new();
    let mut init_cond_changes: Vec<(String, f64)> = Vec::new();

    let existing_params: Vec<String> = doc
        .state
        .parameters
        .iter()
        .map(|p| p.name.clone())
        .collect();

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.heading(egui::RichText::new("Equations").size(15.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::image_and_text(
                        Icon::Plus.image_tinted(11.0, Color32::from_rgb(200, 210, 230)),
                        "Add",
                    ))
                    .on_hover_text("Add new recurrence equation")
                    .clicked()
                {
                    add_equation = true;
                }
            });
        });

        ui.add_space(4.0);

        let equations_count = doc.state.equations.len();

        for i in 0..equations_count {
            let eq_id = doc.state.equations[i].id;
            let eq_color_idx = doc.state.equations[i].color_idx;
            let eq_source = doc.state.equations[i].source.clone();

            let color = TRACE_COLORS[eq_color_idx % TRACE_COLORS.len()];
            let parse_err = parse_errors.get(&eq_id);

            ui.vertical(|ui| {
                egui::Frame::NONE
                    .fill(Color32::from_rgb(22, 24, 32))
                    .corner_radius(4.0)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Color dot indicator
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 4.5, color);

                            // Equation text edit
                            let mut text = eq_source.clone();
                            let response = ui.add(
                                egui::TextEdit::singleline(&mut text)
                                    .desired_width(ui.available_width() - 40.0)
                                    .font(egui::TextStyle::Monospace),
                            );

                            if response.changed() {
                                source_changes.push((i, text));
                            }

                            // Delete equation button
                            if ui
                                .add(egui::Button::image(
                                    Icon::Close.image_tinted(9.0, Color32::from_rgb(180, 190, 210)),
                                ))
                                .on_hover_text("Remove equation")
                                .clicked()
                            {
                                delete_eq_id = Some(eq_id);
                            }
                        });

                        // Display inline parse error if any
                        if let Some(err) = parse_err {
                            ui.horizontal(|ui| {
                                Icon::Alert.ui(ui, 11.0, Color32::from_rgb(255, 90, 90));
                                ui.label(
                                    egui::RichText::new(&err.message)
                                        .color(Color32::from_rgb(255, 90, 90))
                                        .size(10.5),
                                );
                            });
                        }

                        // Extract numeric literals for scrubbing
                        let literals = extract_numeric_literals(&eq_source);
                        if !literals.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new("scrub:")
                                        .size(10.0)
                                        .color(Color32::from_rgb(130, 140, 160)),
                                );

                                for lit in literals {
                                    let btn_label = format!("{:.3}", lit.value);
                                    let chip_response = ui.add(
                                        egui::Button::new(
                                            egui::RichText::new(btn_label)
                                                .monospace()
                                                .size(10.5)
                                                .color(Color32::from_rgb(220, 230, 255)),
                                        )
                                        .fill(Color32::from_rgb(32, 36, 48))
                                        .corner_radius(3.0),
                                    );

                                    // Drag scrub interaction
                                    if chip_response.dragged() {
                                        let drag_delta = chip_response.drag_delta().x;
                                        let step = if ui.input(|i| i.modifiers.shift) {
                                            0.01 // coarse
                                        } else if ui
                                            .input(|i| i.modifiers.ctrl || i.modifiers.command)
                                        {
                                            0.0001 // fine
                                        } else {
                                            0.001 // default
                                        };

                                        let new_val = lit.value + (drag_delta as f64) * step;
                                        let formatted = if new_val.abs() < 0.001 && new_val != 0.0 {
                                            format!("{:.5}", new_val)
                                        } else {
                                            format!("{:.3}", new_val)
                                        };

                                        scrub_action = Some(EquationScrubAction {
                                            eq_id,
                                            span: lit.span,
                                            new_value_str: formatted,
                                        });
                                    }

                                    // Promote to parameter button
                                    if ui
                                        .add(egui::Button::image_and_text(
                                            Icon::ArrowRight.image_tinted(
                                                10.0,
                                                Color32::from_rgb(180, 200, 240),
                                            ),
                                            "param",
                                        ))
                                        .on_hover_text("Promote this number to a named parameter")
                                        .clicked()
                                    {
                                        let candidate_name =
                                            find_available_param_name(&existing_params);
                                        promote_action = Some(EquationPromoteAction {
                                            eq_id,
                                            span: lit.span,
                                            param_name: candidate_name,
                                            param_value: lit.value,
                                        });
                                    }
                                }
                            });
                        }

                        // Initial condition editor for this target variable if parsed
                        let target_var = extract_target_var(&eq_source);
                        if let Some(ref var_name) = target_var {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{var_name}[0] ="))
                                        .monospace()
                                        .size(10.5)
                                        .color(Color32::from_rgb(140, 150, 170)),
                                );
                                let mut init_val = doc
                                    .state
                                    .initial_conditions
                                    .get(var_name)
                                    .copied()
                                    .unwrap_or(0.0);
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut init_val)
                                            .speed(0.05)
                                            .max_decimals(3),
                                    )
                                    .changed()
                                {
                                    init_cond_changes.push((var_name.clone(), init_val));
                                }
                            });
                        }

                        // Live term evaluation readout at cursor step n
                        if let (Some(sim), Some(step)) = (sim_result, cursor_step) {
                            if let Ok(parsed_eq) = crate::parser::parse_equation_str(&eq_source) {
                                let params_map = doc.state.get_parameters_map();
                                let ctx = crate::sim::SimResultContext {
                                    sim_result: sim,
                                    current_step: step,
                                    parameters: &params_map,
                                };
                                let (overall_rhs, terms) =
                                    crate::eval::collect_equation_term_evaluations(
                                        &parsed_eq.rhs,
                                        &ctx,
                                    );

                                if !terms.is_empty() || overall_rhs.is_some() {
                                    ui.add_space(2.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!("n={step}:"))
                                                .size(10.0)
                                                .monospace()
                                                .color(Color32::from_rgb(0, 220, 255)),
                                        );

                                        for t in terms {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}={}",
                                                    t.label,
                                                    format_compact_value(t.value)
                                                ))
                                                .size(10.0)
                                                .monospace()
                                                .color(Color32::from_rgb(180, 200, 230)),
                                            );
                                        }

                                        if let Some(rhs_v) = overall_rhs {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "➔ next {}={}",
                                                    parsed_eq.target_var,
                                                    format_compact_value(rhs_v)
                                                ))
                                                .size(10.0)
                                                .monospace()
                                                .strong()
                                                .color(Color32::from_rgb(255, 220, 80)),
                                            );
                                        }
                                    });
                                }
                            }
                        }
                    });
            });

            ui.add_space(3.0);
        }

        // Global dependency error notification
        if let Some(dep_err) = dep_error {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                Icon::Alert.ui(ui, 12.0, Color32::from_rgb(255, 100, 100));
                ui.label(
                    egui::RichText::new(format!("Recurrence Error: {dep_err}"))
                        .color(Color32::from_rgb(255, 100, 100))
                        .size(11.0),
                );
            });
        }
    });

    // Apply source changes
    for (idx, text) in source_changes {
        if idx < doc.state.equations.len() {
            doc.state.equations[idx].source = text;
        }
    }

    // Apply initial condition changes
    for (var_name, val) in init_cond_changes {
        doc.state.initial_conditions.insert(var_name, val);
    }

    // Apply scrub action
    if let Some(action) = scrub_action {
        if let Some(eq) = doc
            .state
            .equations
            .iter_mut()
            .find(|e| e.id == action.eq_id)
        {
            if action.span.start <= action.span.end && action.span.end <= eq.source.len() {
                let mut new_src = String::new();
                new_src.push_str(&eq.source[..action.span.start]);
                new_src.push_str(&action.new_value_str);
                new_src.push_str(&eq.source[action.span.end..]);
                eq.source = new_src;
            }
        }
    }

    // Apply promote to parameter action
    if let Some(action) = promote_action {
        if let Some(eq) = doc
            .state
            .equations
            .iter_mut()
            .find(|e| e.id == action.eq_id)
        {
            if action.span.start <= action.span.end && action.span.end <= eq.source.len() {
                let mut new_src = String::new();
                new_src.push_str(&eq.source[..action.span.start]);
                new_src.push_str(&action.param_name);
                new_src.push_str(&eq.source[action.span.end..]);
                eq.source = new_src;

                // Add parameter to document
                doc.state.parameters.push(ParameterDoc {
                    name: action.param_name,
                    value: action.param_value,
                    min: (action.param_value * 0.1).min(0.0),
                    max: (action.param_value * 2.5).max(1.0),
                    step: 0.001,
                });
            }
        }
    }

    // Apply delete equation
    if let Some(del_id) = delete_eq_id {
        doc.state.equations.retain(|e| e.id != del_id);
    }

    // Apply add equation
    if add_equation {
        let new_id = doc.next_equation_id();
        let next_char = (b'a' + (doc.state.equations.len() as u8 % 26)) as char;
        let new_src = format!("{next_char}[n+1] = {next_char}[n] * 0.99");
        let color_idx = doc.state.equations.len();
        doc.state.equations.push(crate::document::EquationDoc {
            id: new_id,
            source: new_src,
            color_idx,
        });
    }
}

struct NumericLiteralSpan {
    value: f64,
    span: Span,
}

fn extract_numeric_literals(source: &str) -> Vec<NumericLiteralSpan> {
    let mut list = Vec::new();
    let mut lexer = Lexer::new(source);
    let mut inside_bracket = false;

    while let Ok(tok) = lexer.next_token() {
        match tok.kind {
            TokenKind::LBracket => inside_bracket = true,
            TokenKind::RBracket => inside_bracket = false,
            TokenKind::Number { value, .. } => {
                // Don't scrub inside index brackets [n+1]
                if !inside_bracket {
                    list.push(NumericLiteralSpan {
                        value,
                        span: tok.span,
                    });
                }
            }
            TokenKind::Eof => break,
            _ => {}
        }
    }
    list
}

fn extract_target_var(source: &str) -> Option<String> {
    let mut lexer = Lexer::new(source);
    if let Ok(tok) = lexer.next_token() {
        if let TokenKind::Ident(name) = tok.kind {
            return Some(name);
        }
    }
    None
}

fn find_available_param_name(existing_params: &[String]) -> String {
    for candidate in &["k", "p", "alpha", "beta", "gamma", "m", "c", "gain"] {
        if !existing_params.iter().any(|p| p == candidate) {
            return candidate.to_string();
        }
    }
    format!("p{}", existing_params.len() + 1)
}
