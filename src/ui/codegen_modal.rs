use crate::codegen::c::{generate_c_code, generate_c_standalone_runner};
use crate::codegen::python::{generate_python_code, generate_python_standalone_runner};
use crate::codegen::rust::{generate_rust_code, generate_rust_standalone_runner};
use crate::codegen::SystemModel;
use crate::document::DocumentState;
use crate::ui::icons::Icon;
use egui::{Color32, RichText, ScrollArea, Ui, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenLanguage {
    C,
    Rust,
    Python,
}

#[derive(Debug, Clone)]
pub struct CodegenModalState {
    pub is_open: bool,
    pub language: CodegenLanguage,
    pub standalone_runner: bool,
    pub copy_status: Option<(String, f32)>,
}

impl Default for CodegenModalState {
    fn default() -> Self {
        Self {
            is_open: false,
            language: CodegenLanguage::C,
            standalone_runner: false,
            copy_status: None,
        }
    }
}

pub fn render_codegen_modal(
    ctx: &egui::Context,
    state: &mut CodegenModalState,
    doc_state: &DocumentState,
) {
    if !state.is_open {
        return;
    }

    if let Some((_, ref mut timer)) = state.copy_status {
        *timer -= ctx.input(|i| i.stable_dt);
        if *timer <= 0.0 {
            state.copy_status = None;
        }
    }

    let (equations, parse_errors) = doc_state.get_parsed_equations();
    let params = doc_state.get_parameters_map();

    let mut is_open = state.is_open;
    Window::new("Export Code")
        .open(&mut is_open)
        .resizable(true)
        .default_size([760.0, 520.0])
        .show(ctx, |ui| {
            render_codegen_content(
                ui,
                state,
                doc_state,
                &equations,
                parse_errors.is_empty(),
                &params,
            );
        });

    state.is_open = is_open;
}

fn render_codegen_content(
    ui: &mut Ui,
    state: &mut CodegenModalState,
    doc_state: &DocumentState,
    equations: &[crate::ast::Equation],
    no_parse_errors: bool,
    params: &std::collections::HashMap<String, f64>,
) {
    // Top Controls Bar
    ui.horizontal(|ui| {
        ui.label(RichText::new("Language:").strong());

        let languages = [
            (CodegenLanguage::C, "C (C99 / DSP Header)"),
            (CodegenLanguage::Rust, "Rust (#![no_std])"),
            (CodegenLanguage::Python, "Python (3.x)"),
        ];

        for (lang, label) in languages {
            if ui.selectable_label(state.language == lang, label).clicked() {
                state.language = lang;
            }
        }

        ui.separator();

        ui.checkbox(&mut state.standalone_runner, "Standalone Runner (main)");

        ui.separator();

        // Generate the code string
        let code_result = if !no_parse_errors {
            Err("Document equations contain syntax or parse errors.".to_string())
        } else if equations.is_empty() {
            Err("No equations defined in document.".to_string())
        } else {
            match SystemModel::from_equations(equations, &doc_state.initial_conditions, params) {
                Ok(model) => match (state.language, state.standalone_runner) {
                    (CodegenLanguage::C, false) => {
                        generate_c_code(&model).map_err(|e| e.to_string())
                    }
                    (CodegenLanguage::C, true) => {
                        generate_c_standalone_runner(&model, doc_state.sample_count)
                            .map_err(|e| e.to_string())
                    }
                    (CodegenLanguage::Rust, false) => {
                        generate_rust_code(&model).map_err(|e| e.to_string())
                    }
                    (CodegenLanguage::Rust, true) => {
                        generate_rust_standalone_runner(&model, doc_state.sample_count)
                            .map_err(|e| e.to_string())
                    }
                    (CodegenLanguage::Python, false) => {
                        generate_python_code(&model).map_err(|e| e.to_string())
                    }
                    (CodegenLanguage::Python, true) => {
                        generate_python_standalone_runner(&model, doc_state.sample_count)
                            .map_err(|e| e.to_string())
                    }
                },
                Err(e) => Err(e.to_string()),
            }
        };

        let copy_btn = egui::Button::image_and_text(
            Icon::Save.image_tinted(13.0, Color32::from_rgb(200, 230, 255)),
            "Copy Code",
        );

        if ui
            .add_enabled(code_result.is_ok(), copy_btn)
            .on_hover_text("Copy generated code to system clipboard")
            .clicked()
        {
            if let Ok(ref code) = code_result {
                ui.ctx().copy_text(code.clone());
                state.copy_status = Some(("Copied to clipboard!".to_string(), 2.0));
            }
        }

        if let Some((ref msg, _)) = state.copy_status {
            ui.label(
                RichText::new(msg)
                    .color(Color32::from_rgb(80, 240, 160))
                    .strong(),
            );
        }
    });

    ui.add_space(6.0);

    // Code Display Box
    match if !no_parse_errors {
        Err("Document equations contain syntax or parse errors.".to_string())
    } else if equations.is_empty() {
        Err("No equations defined in document.".to_string())
    } else {
        match SystemModel::from_equations(equations, &doc_state.initial_conditions, params) {
            Ok(model) => match (state.language, state.standalone_runner) {
                (CodegenLanguage::C, false) => generate_c_code(&model).map_err(|e| e.to_string()),
                (CodegenLanguage::C, true) => {
                    generate_c_standalone_runner(&model, doc_state.sample_count)
                        .map_err(|e| e.to_string())
                }
                (CodegenLanguage::Rust, false) => {
                    generate_rust_code(&model).map_err(|e| e.to_string())
                }
                (CodegenLanguage::Rust, true) => {
                    generate_rust_standalone_runner(&model, doc_state.sample_count)
                        .map_err(|e| e.to_string())
                }
                (CodegenLanguage::Python, false) => {
                    generate_python_code(&model).map_err(|e| e.to_string())
                }
                (CodegenLanguage::Python, true) => {
                    generate_python_standalone_runner(&model, doc_state.sample_count)
                        .map_err(|e| e.to_string())
                }
            },
            Err(e) => Err(e.to_string()),
        }
    } {
        Ok(code) => {
            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut code_str = code;
                    ui.add(
                        egui::TextEdit::multiline(&mut code_str)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .code_editor(),
                    );
                });
        }
        Err(err_msg) => {
            ui.colored_label(
                Color32::from_rgb(255, 100, 100),
                format!("Codegen Error: {err_msg}"),
            );
        }
    }
}
