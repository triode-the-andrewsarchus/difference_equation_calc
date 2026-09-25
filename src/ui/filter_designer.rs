use crate::document::Document;
use crate::filter::{
    pole_to_fc_q_ext, CascadeFilter, Complex, FilterEngine, FilterType, StimulusType,
};
use crate::ui::format::format_compact_value;
use crate::ui::icons::Icon;
use egui::{epaint::PathShape, pos2, vec2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    Pole,
    Zero,
}

#[derive(Debug, Clone)]
pub struct FilterDesignerState {
    pub is_open: bool,
    pub engine: FilterEngine,
    pub filter_type: FilterType,
    pub stimulus: StimulusType,
    pub pole_count: usize,
    pub allow_self_oscillation: bool,
    pub soft_saturation: bool,
    pub fc: f64,
    pub q: f64,
    pub sample_rate: f64,
    pub live_preview: bool,
    pub dragging: Option<DragTarget>,
}

impl Default for FilterDesignerState {
    fn default() -> Self {
        Self {
            is_open: false,
            engine: FilterEngine::BiquadDirectForm2,
            filter_type: FilterType::Lowpass,
            stimulus: StimulusType::Step,
            pole_count: 2,
            allow_self_oscillation: false,
            soft_saturation: false,
            fc: 0.10,
            q: 2.5,
            sample_rate: 48000.0,
            live_preview: true,
            dragging: None,
        }
    }
}

pub struct FilterDesignerResponse {
    pub applied: bool,
}

pub fn render_filter_designer_window(
    ctx: &egui::Context,
    state: &mut FilterDesignerState,
    doc: &mut Document,
) -> FilterDesignerResponse {
    let mut response = FilterDesignerResponse { applied: false };
    if !state.is_open {
        return response;
    }

    let mut is_open = state.is_open;
    let mut close_requested = false;
    let mut should_apply = false;

    egui::Window::new("Digital Filter Designer")
        .open(&mut is_open)
        .resizable(true)
        .default_size(vec2(740.0, 560.0))
        .min_size(vec2(620.0, 480.0))
        .max_size(vec2(1200.0, 850.0))
        .show(ctx, |ui| {
            let mut state_changed = false;

            // Row 1: Filter Type & Architecture
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Filter:").strong());
                for ft in FilterType::ALL {
                    let is_active = state.filter_type == ft;
                    if ui
                        .selectable_label(is_active, ft.short_name())
                        .on_hover_text(ft.name())
                        .clicked()
                        && !is_active
                    {
                        state.filter_type = ft;
                        state_changed = true;
                    }
                }

                ui.separator();

                ui.label(egui::RichText::new("Engine:").strong());
                let is_biquad = state.engine == FilterEngine::BiquadDirectForm2;
                if ui
                    .selectable_label(is_biquad, "Biquad DF2")
                    .on_hover_text("Direct Form II cascaded biquad difference equations")
                    .clicked()
                    && !is_biquad
                {
                    state.engine = FilterEngine::BiquadDirectForm2;
                    state_changed = true;
                }
                let is_svf = state.engine == FilterEngine::ChamberlinSvf;
                if ui
                    .selectable_label(is_svf, "Chamberlin SVF")
                    .on_hover_text("State-variable filter with simultaneous lowpass, bandpass, highpass outputs")
                    .clicked()
                    && !is_svf
                {
                    state.engine = FilterEngine::ChamberlinSvf;
                    state_changed = true;
                }
            });

            ui.separator();

            // Row 2: Pole Count & Self-Oscillation Controls
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Poles:").strong());
                for p in [2, 4, 6, 8] {
                    let is_active = state.pole_count == p;
                    let label = format!("{}-Pole ({}dB)", p, p * 6);
                    if ui
                        .selectable_label(is_active, label)
                        .on_hover_text(format!(
                            "Order {} ({} cascaded stages, {} dB/octave roll-off)",
                            p,
                            p / 2,
                            p * 6
                        ))
                        .clicked()
                        && !is_active
                    {
                        state.pole_count = p;
                        state_changed = true;
                    }
                }

                ui.separator();

                if ui
                    .checkbox(
                        &mut state.allow_self_oscillation,
                        "Self-Oscillation (|z| ≥ 1.0)",
                    )
                    .on_hover_text("Allow poles to move on/outside the unit circle for negative damping and autonomous limit cycles")
                    .changed()
                {
                    state_changed = true;
                }

                if ui
                    .checkbox(&mut state.soft_saturation, "Soft Saturation (tanh)")
                    .on_hover_text("Wrap feedback in tanh non-linearity to stabilize self-oscillation into a non-exploding limit cycle")
                    .changed()
                {
                    state_changed = true;
                }
            });

            ui.separator();

            // Row 3: Stimulus and Live Link
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Stimulus:").strong());
                for stim in StimulusType::ALL {
                    let is_active = state.stimulus == stim;
                    if ui
                        .selectable_label(is_active, stim.short_name())
                        .on_hover_text(stim.name())
                        .clicked()
                        && !is_active
                    {
                        state.stimulus = stim;
                        state_changed = true;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .checkbox(&mut state.live_preview, "Live Link")
                        .on_hover_text("Continuously stream filter parameters and equations into simulation in real time")
                        .changed()
                        && state.live_preview
                    {
                        state_changed = true;
                    }
                });
            });

            ui.add_space(4.0);

            // Compute cascade filter
            let cascade = CascadeFilter::new(
                state.filter_type,
                state.pole_count,
                state.fc,
                state.q,
                state.allow_self_oscillation,
            );
            let max_pole_mag = cascade.max_pole_mag();

            // Interactive Graphics Area: Z-Plane (Left) and Bode Plot (Right) using bounded columns
            let canvas_h = 240.0;
            ui.columns(2, |cols| {
                // 1. Z-Domain Complex Unit Circle
                let col0 = &mut cols[0];
                let col0_w = col0.available_width();
                col0.label(
                    egui::RichText::new(format!(
                        "Z-Domain Pole-Zero Plane ({}-Pole, max |z| = {})",
                        state.pole_count,
                        format_compact_value(max_pole_mag)
                    ))
                    .size(11.0)
                    .color(Color32::from_rgb(180, 200, 230)),
                );

                let z_changed = render_zplane_canvas(
                    col0,
                    vec2(col0_w, canvas_h - 18.0),
                    state,
                    &cascade,
                );
                if z_changed {
                    state_changed = true;
                }

                // 2. Bode Frequency Response Plot
                let col1 = &mut cols[1];
                let col1_w = col1.available_width();
                let hz = state.fc * state.sample_rate;
                col1.label(
                    egui::RichText::new(format!(
                        "Magnitude Response ({} dB/oct, Cutoff: {:.0} Hz)",
                        state.pole_count * 6,
                        hz
                    ))
                    .size(11.0)
                    .color(Color32::from_rgb(180, 200, 230)),
                );

                render_bode_plot_canvas(
                    col1,
                    vec2(col1_w, canvas_h - 18.0),
                    &cascade,
                    state.fc,
                    state.sample_rate,
                );
            });

            ui.add_space(6.0);
            ui.separator();

            // Continuous Parameter Sliders
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    // Cutoff frequency slider
                    let hz = state.fc * state.sample_rate;
                    ui.horizontal(|ui| {
                        ui.label("Cutoff Frequency fc:");
                        let mut fc = state.fc;
                        if ui
                            .add(
                                egui::Slider::new(&mut fc, 0.005..=0.490)
                                    .step_by(0.001)
                                    .text(format!("{:.1} Hz", hz)),
                            )
                            .on_hover_text("Normalized cutoff frequency fc / fs")
                            .changed()
                        {
                            state.fc = fc;
                            state_changed = true;
                        }
                    });

                    // Q / Resonance factor slider
                    ui.horizontal(|ui| {
                        let q_max = if state.allow_self_oscillation {
                            60.0
                        } else {
                            25.0
                        };
                        let q_desc = if max_pole_mag >= 1.0 {
                            "Self-Oscillation (|z| ≥ 1.0)"
                        } else if (state.q - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.05 {
                            "Butterworth (flat)"
                        } else if state.q > 5.0 {
                            "High Resonance"
                        } else if state.q < 0.5 {
                            "Overdamped"
                        } else {
                            "Resonant"
                        };

                        ui.label("Resonance Q:");
                        let mut q = state.q;
                        if ui
                            .add(
                                egui::Slider::new(&mut q, 0.2..=q_max)
                                    .logarithmic(true)
                                    .text(q_desc),
                            )
                            .on_hover_text("Filter quality factor Q (resonance factor)")
                            .changed()
                        {
                            state.q = q;
                            state_changed = true;
                        }
                    });
                });

                ui.separator();

                // Coefficients readout & Stability Status
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        let (status_text, status_color) = if max_pole_mag < 0.999 {
                            (
                                format!(
                                    "STABLE (max |z| = {} < 1.0)",
                                    format_compact_value(max_pole_mag)
                                ),
                                Color32::from_rgb(80, 240, 140),
                            )
                        } else if max_pole_mag <= 1.002 {
                            (
                                format!(
                                    "SELF-OSCILLATING / MARGINAL (|z| = {})",
                                    format_compact_value(max_pole_mag)
                                ),
                                Color32::from_rgb(255, 200, 50),
                            )
                        } else {
                            (
                                format!(
                                    "UNSTABLE / GROWING (max |z| = {} ≥ 1.0)",
                                    format_compact_value(max_pole_mag)
                                ),
                                Color32::from_rgb(255, 80, 80),
                            )
                        };

                        ui.label("Stability:");
                        ui.label(
                            egui::RichText::new(status_text)
                                .strong()
                                .color(status_color),
                        );
                    });

                    match state.engine {
                        FilterEngine::BiquadDirectForm2 => {
                            if cascade.stages.len() == 1 {
                                let c = &cascade.stages[0];
                                ui.label(
                                    egui::RichText::new(format!(
                                        "b0: {:+.4}   b1: {:+.4}   b2: {:+.4}\na1: {:+.4}   a2: {:+.4}",
                                        c.b0, c.b1, c.b2, c.a1, c.a2
                                    ))
                                    .monospace()
                                    .size(11.0)
                                    .color(Color32::from_rgb(190, 200, 220)),
                                );
                            } else {
                                let c1 = &cascade.stages[0];
                                let c_last = cascade.stages.last().unwrap();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Stage 1: a1: {:+.4}, a2: {:+.4}\nStage {}: a1: {:+.4}, a2: {:+.4}",
                                        c1.a1, c1.a2, cascade.stages.len(), c_last.a1, c_last.a2
                                    ))
                                    .monospace()
                                    .size(11.0)
                                    .color(Color32::from_rgb(190, 200, 220)),
                                );
                            }
                        }
                        FilterEngine::ChamberlinSvf => {
                            let f_val = 2.0 * (PI * state.fc.clamp(0.01, 0.45)).sin();
                            let q_damp = if state.allow_self_oscillation && state.q >= 25.0 {
                                -(state.q - 25.0) * 0.015
                            } else {
                                1.0 / state.q.clamp(0.2, 25.0)
                            };
                            ui.label(
                                egui::RichText::new(format!(
                                    "f: {:.4} (2 sin(π fc))\nq: {:.4} (damping; <= 0 oscillates)",
                                    f_val, q_damp
                                ))
                                .monospace()
                                .size(11.0)
                                .color(Color32::from_rgb(190, 200, 220)),
                            );
                        }
                    }
                });
            });

            ui.add_space(6.0);
            ui.separator();

            // Bottom Buttons
            ui.horizontal(|ui| {
                let apply_btn = egui::Button::image_and_text(
                    Icon::Waves.image_tinted(14.0, Color32::from_rgb(80, 240, 160)),
                    "Apply to Document",
                );
                if ui
                    .add(apply_btn)
                    .on_hover_text("Load this filter equations and stimulus into active document")
                    .clicked()
                {
                    should_apply = true;
                }

                if ui.button("Reset Defaults").clicked() {
                    state.fc = 0.10;
                    state.q = 2.5;
                    state.pole_count = 2;
                    state.allow_self_oscillation = false;
                    state.soft_saturation = false;
                    state_changed = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close_requested = true;
                    }
                });
            });

            if state_changed && state.live_preview {
                should_apply = true;
            }
        });

    if close_requested {
        is_open = false;
    }
    state.is_open = is_open;

    if should_apply {
        apply_filter_to_document(state, doc);
        response.applied = true;
    }

    response
}

pub fn apply_filter_to_document(state: &FilterDesignerState, doc: &mut Document) {
    let new_doc_state = match state.engine {
        FilterEngine::BiquadDirectForm2 => crate::filter::create_cascade_biquad_doc(
            state.filter_type,
            state.pole_count,
            state.fc,
            state.q,
            state.stimulus,
            state.allow_self_oscillation,
            state.soft_saturation,
        ),
        FilterEngine::ChamberlinSvf => crate::filter::create_cascade_svf_doc(
            state.pole_count,
            state.fc,
            state.q,
            state.stimulus,
            state.allow_self_oscillation,
            state.soft_saturation,
        ),
    };

    doc.state.equations = new_doc_state.equations;
    doc.state.initial_conditions = new_doc_state.initial_conditions;
    doc.state.parameters = new_doc_state.parameters;
    doc.state.selection = new_doc_state.selection;
    doc.state.search_query = new_doc_state.search_query;
    doc.state.sweep_config = new_doc_state.sweep_config;
}

fn render_zplane_canvas(
    ui: &mut Ui,
    size: Vec2,
    state: &mut FilterDesignerState,
    cascade: &CascadeFilter,
) -> bool {
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let rect = response.rect;

    // Background
    painter.rect_filled(rect, 4.0, Color32::from_rgb(16, 20, 28));

    // Coordinate mapping: center is at rect center, unit circle radius is r_pixels
    let center = rect.center();
    let max_radius_scale = if state.allow_self_oscillation {
        0.33
    } else {
        0.40
    };
    let r_pixels = (rect.width().min(rect.height()) * max_radius_scale).max(35.0);

    let to_screen = |z: Complex| -> Pos2 {
        pos2(
            center.x + (z.re as f32) * r_pixels,
            center.y - (z.im as f32) * r_pixels,
        )
    };

    let from_screen = |pos: Pos2| -> Complex {
        Complex::new(
            ((pos.x - center.x) / r_pixels) as f64,
            ((center.y - pos.y) / r_pixels) as f64,
        )
    };

    // If self-oscillation is allowed, render outer unstable boundary zone
    if state.allow_self_oscillation {
        painter.circle_filled(
            center,
            r_pixels * 1.25,
            Color32::from_rgba_unmultiplied(70, 20, 30, 40),
        );
        painter.circle_stroke(
            center,
            r_pixels * 1.25,
            Stroke::new(1.0_f32, Color32::from_rgb(120, 45, 55)),
        );
    }

    // Shaded stable unit disc (|z| < 1)
    painter.circle_filled(
        center,
        r_pixels,
        Color32::from_rgba_unmultiplied(20, 50, 75, 50),
    );

    // Axes
    painter.line_segment(
        [
            pos2(rect.left() + 8.0, center.y),
            pos2(rect.right() - 8.0, center.y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(50, 60, 80)),
    );
    painter.line_segment(
        [
            pos2(center.x, rect.top() + 8.0),
            pos2(center.x, rect.bottom() - 8.0),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(50, 60, 80)),
    );

    // Unit circle boundary (|z| = 1)
    let unit_circle_stroke = if state.allow_self_oscillation {
        Stroke::new(2.0_f32, Color32::from_rgb(255, 170, 70))
    } else {
        Stroke::new(1.5_f32, Color32::from_rgb(60, 140, 200))
    };
    painter.circle_stroke(center, r_pixels, unit_circle_stroke);

    // Half circle (|z| = 0.5)
    painter.circle_stroke(
        center,
        r_pixels * 0.5,
        Stroke::new(1.0_f32, Color32::from_rgb(35, 45, 60)),
    );

    // Axis coordinate labels
    let font_id = FontId::monospace(9.0);
    painter.text(
        pos2(center.x + r_pixels + 3.0, center.y + 2.0),
        egui::Align2::LEFT_TOP,
        "1",
        font_id.clone(),
        Color32::from_rgb(120, 140, 170),
    );
    painter.text(
        pos2(center.x - r_pixels - 3.0, center.y + 2.0),
        egui::Align2::RIGHT_TOP,
        "-1",
        font_id.clone(),
        Color32::from_rgb(120, 140, 170),
    );
    painter.text(
        pos2(center.x + 3.0, center.y - r_pixels - 2.0),
        egui::Align2::LEFT_BOTTOM,
        "j",
        font_id.clone(),
        Color32::from_rgb(120, 140, 170),
    );
    painter.text(
        pos2(center.x + 3.0, center.y + r_pixels + 2.0),
        egui::Align2::LEFT_TOP,
        "-j",
        font_id.clone(),
        Color32::from_rgb(120, 140, 170),
    );

    if state.allow_self_oscillation {
        painter.text(
            pos2(center.x, rect.top() + 6.0),
            egui::Align2::CENTER_TOP,
            "OSCILLATION ZONE (|z| >= 1.0)",
            FontId::monospace(8.0),
            Color32::from_rgb(255, 140, 90),
        );
    }

    // Draw Zeros as 'O' cyan circles
    let zero_color = Color32::from_rgb(70, 220, 240);
    for (z1, z2) in cascade.all_zeros() {
        for z in [z1, z2] {
            if z != Complex::ZERO {
                let p = to_screen(z);
                painter.circle_stroke(p, 5.0, Stroke::new(2.0_f32, zero_color));
            }
        }
    }

    // Helper to draw pole 'X' mark with optional glowing halo for unstable poles
    let draw_x = |painter: &egui::Painter, pos: Pos2, color: Color32, is_unstable: bool| {
        if is_unstable {
            painter.circle_stroke(
                pos,
                9.0,
                Stroke::new(1.5_f32, Color32::from_rgb(255, 70, 70)),
            );
        }
        let d = 5.0;
        painter.line_segment(
            [pos2(pos.x - d, pos.y - d), pos2(pos.x + d, pos.y + d)],
            Stroke::new(2.5_f32, color),
        );
        painter.line_segment(
            [pos2(pos.x - d, pos.y + d), pos2(pos.x + d, pos.y - d)],
            Stroke::new(2.5_f32, color),
        );
    };

    // Draw all poles from all stages of the cascade
    let all_poles = cascade.all_poles();
    let mut pole_screen_positions = Vec::new();

    for (stage_idx, (p1, p2)) in all_poles.iter().enumerate() {
        let is_resonant = stage_idx == all_poles.len() - 1;
        let is_unstable = p1.mag() >= 1.0;
        let color = if is_unstable {
            Color32::from_rgb(255, 60, 60)
        } else if is_resonant {
            Color32::from_rgb(255, 110, 110)
        } else {
            Color32::from_rgb(220, 140, 140)
        };

        let p1_s = to_screen(*p1);
        let p2_s = to_screen(*p2);
        draw_x(&painter, p1_s, color, is_unstable);
        draw_x(&painter, p2_s, color, is_unstable);
        pole_screen_positions.push((p1_s, *p1, stage_idx));
        pole_screen_positions.push((p2_s, *p2, stage_idx));
    }

    // Direct manipulation interaction
    let mut param_changed = false;
    let pointer_pos = response.interact_pointer_pos();

    if response.drag_started() {
        if let Some(pos) = pointer_pos {
            // Check if near any pole
            let near_pole = pole_screen_positions
                .iter()
                .any(|(sp, _, _)| pos.distance(*sp) < 14.0);
            if near_pole {
                state.dragging = Some(DragTarget::Pole);
            } else {
                let z = from_screen(pos);
                let (fc, q) = pole_to_fc_q_ext(z.re, z.im, state.allow_self_oscillation);
                state.fc = fc;
                state.q = q;
                state.dragging = Some(DragTarget::Pole);
                param_changed = true;
            }
        }
    }

    if response.dragged() {
        if let (Some(DragTarget::Pole), Some(pos)) = (state.dragging, pointer_pos) {
            let z = from_screen(pos);
            let (fc, q) = pole_to_fc_q_ext(z.re, z.im, state.allow_self_oscillation);
            state.fc = fc;
            state.q = q;
            param_changed = true;
        }
    }

    if response.drag_stopped() {
        state.dragging = None;
    }

    // Tooltip display on hover
    if let Some(pos) = pointer_pos {
        for (sp, p, stage_idx) in &pole_screen_positions {
            if pos.distance(*sp) < 16.0 {
                let mag = p.mag();
                let status_str = if mag < 0.999 {
                    "Stable"
                } else if mag <= 1.002 {
                    "Marginally Stable (Self-Oscillating)"
                } else {
                    "Unstable (Exponential Growth)"
                };
                response.on_hover_text(format!(
                    "Pole Stage {}/{}: z = {} ± {} j\n|z| = {} ({})\nDrag to adjust Cutoff & Resonance",
                    stage_idx + 1,
                    all_poles.len(),
                    format_compact_value(p.re),
                    format_compact_value(p.im.abs()),
                    format_compact_value(mag),
                    status_str
                ));
                break;
            }
        }
    }

    param_changed
}

fn render_bode_plot_canvas(
    ui: &mut Ui,
    size: Vec2,
    cascade: &CascadeFilter,
    fc: f64,
    sample_rate: f64,
) {
    let (response, painter) = ui.allocate_painter(size, Sense::hover());
    let rect = response.rect;

    // Background
    painter.rect_filled(rect, 4.0, Color32::from_rgb(16, 20, 28));

    let margin_left = 38.0;
    let margin_right = 16.0;
    let margin_top = 16.0;
    let margin_bottom = 22.0;

    let plot_rect = Rect::from_min_max(
        pos2(rect.left() + margin_left, rect.top() + margin_top),
        pos2(rect.right() - margin_right, rect.bottom() - margin_bottom),
    );

    let min_db = -80.0_f64;
    let max_db = 24.0_f64;

    let db_to_y = |db: f64| -> f32 {
        let t = ((db - min_db) / (max_db - min_db)).clamp(0.0, 1.0) as f32;
        plot_rect.bottom() - t * plot_rect.height()
    };

    let norm_f_to_x = |f: f64| -> f32 {
        let t = (f / 0.50).clamp(0.0, 1.0) as f32;
        plot_rect.left() + t * plot_rect.width()
    };

    // Horizontal dB grid lines
    let font_id = FontId::monospace(8.5);
    for db in [-80.0, -60.0, -40.0, -20.0, -3.0, 0.0, 12.0] {
        let y = db_to_y(db);
        let stroke = if db == 0.0 {
            Stroke::new(1.2_f32, Color32::from_rgb(90, 110, 140))
        } else if db == -3.0 {
            Stroke::new(1.0_f32, Color32::from_rgb(160, 120, 50))
        } else {
            Stroke::new(0.8_f32, Color32::from_rgb(32, 40, 55))
        };

        painter.line_segment(
            [pos2(plot_rect.left(), y), pos2(plot_rect.right(), y)],
            stroke,
        );

        painter.text(
            pos2(plot_rect.left() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{:>3.0} dB", db),
            font_id.clone(),
            Color32::from_rgb(110, 130, 160),
        );
    }

    // Vertical Frequency grid lines
    for f in [0.1, 0.2, 0.3, 0.4] {
        let x = norm_f_to_x(f);
        painter.line_segment(
            [pos2(x, plot_rect.top()), pos2(x, plot_rect.bottom())],
            Stroke::new(0.8_f32, Color32::from_rgb(32, 40, 55)),
        );

        let khz = (f * sample_rate) / 1000.0;
        painter.text(
            pos2(x, plot_rect.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            format!("{:.1}k", khz),
            font_id.clone(),
            Color32::from_rgb(110, 130, 160),
        );
    }

    // Evaluate magnitude response across frequency spectrum
    let num_points = 180;
    let mut curve_points: Vec<Pos2> = Vec::with_capacity(num_points);
    let mut fill_path = vec![pos2(plot_rect.left(), plot_rect.bottom())];

    for i in 0..num_points {
        let f_norm = (i as f64 / (num_points - 1) as f64) * 0.50;
        let omega = 2.0 * PI * f_norm;
        let (mag_db, _) = cascade.eval_frequency_response(omega);

        let x = norm_f_to_x(f_norm);
        let y = db_to_y(mag_db);
        let pt = pos2(x, y);

        curve_points.push(pt);
        fill_path.push(pt);
    }

    fill_path.push(pos2(plot_rect.right(), plot_rect.bottom()));

    // Shaded fill under curve
    painter.add(PathShape::convex_polygon(
        fill_path,
        Color32::from_rgba_unmultiplied(40, 120, 200, 35),
        Stroke::NONE,
    ));

    // Response Curve line
    for window in curve_points.windows(2) {
        painter.line_segment(
            [window[0], window[1]],
            Stroke::new(1.8_f32, Color32::from_rgb(90, 200, 255)),
        );
    }

    // Cutoff frequency indicator marker
    let fc_x = norm_f_to_x(fc);
    let (fc_db, _) = cascade.eval_frequency_response(2.0 * PI * fc);
    let fc_y = db_to_y(fc_db);

    painter.line_segment(
        [pos2(fc_x, plot_rect.top()), pos2(fc_x, plot_rect.bottom())],
        Stroke::new(1.0_f32, Color32::from_rgb(255, 180, 60)),
    );

    painter.circle_filled(pos2(fc_x, fc_y), 4.0, Color32::from_rgb(255, 180, 60));

    // Interactive Hover read-out
    if let Some(pos) = response.hover_pos() {
        if plot_rect.contains(pos) {
            let t = ((pos.x - plot_rect.left()) / plot_rect.width()).clamp(0.0, 1.0) as f64;
            let hover_f = t * 0.50;
            let (hover_db, _) = cascade.eval_frequency_response(2.0 * PI * hover_f);
            let hover_y = db_to_y(hover_db);

            // Crosshair
            painter.line_segment(
                [
                    pos2(pos.x, plot_rect.top()),
                    pos2(pos.x, plot_rect.bottom()),
                ],
                Stroke::new(0.8_f32, Color32::from_rgb(200, 210, 240)),
            );
            painter.circle_filled(pos2(pos.x, hover_y), 3.5, Color32::from_rgb(255, 255, 255));

            response.on_hover_text(format!(
                "Freq: {:.4} ({:.1} Hz)\nGain: {:+.2} dB ({} Poles)",
                hover_f,
                hover_f * sample_rate,
                hover_db,
                cascade.pole_count
            ));
        }
    }
}
