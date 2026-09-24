use crate::document::Document;
use crate::filter::{pole_to_fc_q, BiquadCoeffs, Complex, FilterEngine, FilterType, StimulusType};
use crate::ui::icons::Icon;
use egui::{epaint::PathShape, pos2, vec2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    Pole1,
    Pole2,
    Zero1,
    Zero2,
}

#[derive(Debug, Clone)]
pub struct FilterDesignerState {
    pub is_open: bool,
    pub engine: FilterEngine,
    pub filter_type: FilterType,
    pub stimulus: StimulusType,
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
        .default_size(vec2(720.0, 520.0))
        .min_size(vec2(640.0, 460.0))
        .show(ctx, |ui| {
            let mut state_changed = false;

            // Top Bar: Filter Type & Architecture
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Filter:").strong());
                for ft in FilterType::ALL {
                    let is_active = state.filter_type == ft;
                    if ui.selectable_label(is_active, ft.name()).clicked() && !is_active {
                        state.filter_type = ft;
                        state_changed = true;
                    }
                }

                ui.separator();

                ui.label(egui::RichText::new("Engine:").strong());
                let is_biquad = state.engine == FilterEngine::BiquadDirectForm2;
                if ui
                    .selectable_label(is_biquad, "Biquad (DF2)")
                    .on_hover_text("Direct Form II 2nd-order difference equation with 5 parameters")
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

            // Row with Stimulus and Controls
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Stimulus:").strong());
                for stim in StimulusType::ALL {
                    let is_active = state.stimulus == stim;
                    if ui.selectable_label(is_active, stim.name()).clicked() && !is_active {
                        state.stimulus = stim;
                        state_changed = true;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .checkbox(&mut state.live_preview, "Live Link to Document")
                        .on_hover_text("Continuously stream filter parameters and equations into simulation in real time")
                        .changed()
                        && state.live_preview
                    {
                        state_changed = true;
                    }
                });
            });

            ui.add_space(4.0);

            // Compute coefficients, poles, and zeros
            let coeffs = BiquadCoeffs::from_filter_params(state.filter_type, state.fc, state.q);
            let (pole1, pole2) = coeffs.poles();
            let (zero1, zero2) = coeffs.zeros();
            let pole_mag = pole1.mag().max(pole2.mag());
            let is_stable = pole_mag < 1.0;

            // Interactive Graphics Area: Z-Plane (Left) and Bode Plot (Right)
            ui.horizontal(|ui| {
                let avail_w = ui.available_width();
                let zplane_w = (avail_w * 0.38).clamp(240.0, 320.0);
                let bode_w = avail_w - zplane_w - 12.0;
                let canvas_h = 240.0;

                // 1. Z-Domain Complex Unit Circle
                ui.allocate_ui_with_layout(
                    vec2(zplane_w, canvas_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Z-Domain Pole-Zero Complex Plane")
                                .size(11.0)
                                .color(Color32::from_rgb(180, 200, 230)),
                        );

                        let z_changed = render_zplane_canvas(
                            ui,
                            vec2(zplane_w, canvas_h - 18.0),
                            state,
                            pole1,
                            pole2,
                            zero1,
                            zero2,
                        );
                        if z_changed {
                            state_changed = true;
                        }
                    },
                );

                ui.separator();

                // 2. Bode Frequency Response Plot
                ui.allocate_ui_with_layout(
                    vec2(bode_w, canvas_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let hz = state.fc * state.sample_rate;
                        ui.label(
                            egui::RichText::new(format!(
                                "Magnitude Response |H(e^jω)| (Cutoff: {:.4} / {:.0} Hz)",
                                state.fc, hz
                            ))
                            .size(11.0)
                            .color(Color32::from_rgb(180, 200, 230)),
                        );

                        render_bode_plot_canvas(
                            ui,
                            vec2(bode_w, canvas_h - 18.0),
                            &coeffs,
                            state.fc,
                            state.sample_rate,
                        );
                    },
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
                        let q_desc = if (state.q - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.05 {
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
                                egui::Slider::new(&mut q, 0.2..=25.0)
                                    .logarithmic(true)
                                    .text(q_desc),
                            )
                            .on_hover_text("Filter quality factor Q (resonance)")
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
                        let status_text = if is_stable {
                            format!("STABLE (max |z| = {:.3} < 1.0)", pole_mag)
                        } else {
                            format!("UNSTABLE (max |z| = {:.3} >= 1.0)", pole_mag)
                        };
                        let status_color = if is_stable {
                            Color32::from_rgb(80, 240, 140)
                        } else {
                            Color32::from_rgb(255, 90, 90)
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
                            ui.label(
                                egui::RichText::new(format!(
                                    "b0: {:+.4}   b1: {:+.4}   b2: {:+.4}\na1: {:+.4}   a2: {:+.4}",
                                    coeffs.b0, coeffs.b1, coeffs.b2, coeffs.a1, coeffs.a2
                                ))
                                .monospace()
                                .size(11.0)
                                .color(Color32::from_rgb(190, 200, 220)),
                            );
                        }
                        FilterEngine::ChamberlinSvf => {
                            let f_val = 2.0 * (PI * state.fc.clamp(0.01, 0.45)).sin();
                            let q_damp = 1.0 / state.q.clamp(0.2, 20.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "f: {:.4} (2 sin(π fc))\nq: {:.4} (1 / Q damping)",
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
        FilterEngine::BiquadDirectForm2 => crate::filter::create_biquad_filter_doc(
            state.filter_type,
            state.fc,
            state.q,
            state.stimulus,
        ),
        FilterEngine::ChamberlinSvf => {
            crate::filter::create_chamberlin_svf_doc(state.fc, state.q, state.stimulus)
        }
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
    pole1: Complex,
    pole2: Complex,
    zero1: Complex,
    zero2: Complex,
) -> bool {
    let (response, painter) = ui.allocate_painter(size, Sense::click_and_drag());
    let rect = response.rect;

    // Background
    painter.rect_filled(rect, 4.0, Color32::from_rgb(16, 20, 28));

    // Coordinate mapping: center is at rect center, unit circle radius is r_pixels
    let center = rect.center();
    let r_pixels = (rect.width().min(rect.height()) * 0.40).max(40.0);

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

    // Shaded stable unit disc (|z| < 1)
    painter.circle_filled(
        center,
        r_pixels,
        Color32::from_rgba_unmultiplied(20, 50, 75, 50),
    );

    // Axes
    painter.line_segment(
        [
            pos2(rect.left() + 10.0, center.y),
            pos2(rect.right() - 10.0, center.y),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(50, 60, 80)),
    );
    painter.line_segment(
        [
            pos2(center.x, rect.top() + 10.0),
            pos2(center.x, rect.bottom() - 10.0),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(50, 60, 80)),
    );

    // Unit circle boundary (|z| = 1)
    painter.circle_stroke(
        center,
        r_pixels,
        Stroke::new(1.5_f32, Color32::from_rgb(60, 140, 200)),
    );

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

    // Draw Zeros as 'O' cyan circles
    let zero_color = Color32::from_rgb(70, 220, 240);
    for z in [zero1, zero2] {
        let p = to_screen(z);
        painter.circle_stroke(p, 5.0, Stroke::new(2.0_f32, zero_color));
    }

    // Draw Poles as 'X' coral marks
    let pole_color = Color32::from_rgb(255, 100, 100);
    let draw_x = |painter: &egui::Painter, pos: Pos2, color: Color32| {
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

    let p1_screen = to_screen(pole1);
    let p2_screen = to_screen(pole2);
    draw_x(&painter, p1_screen, pole_color);
    draw_x(&painter, p2_screen, pole_color);

    // Direct manipulation interaction
    let mut param_changed = false;
    let pointer_pos = response.interact_pointer_pos();

    if response.drag_started() {
        if let Some(pos) = pointer_pos {
            // Check if clicking near pole1 or pole2
            if pos.distance(p1_screen) < 14.0 {
                state.dragging = Some(DragTarget::Pole1);
            } else if pos.distance(p2_screen) < 14.0 {
                state.dragging = Some(DragTarget::Pole2);
            } else if pos.distance(to_screen(zero1)) < 14.0 {
                state.dragging = Some(DragTarget::Zero1);
            } else if pos.distance(to_screen(zero2)) < 14.0 {
                state.dragging = Some(DragTarget::Zero2);
            } else {
                // Clicking anywhere on unit circle moves poles there
                let z = from_screen(pos);
                let (fc, q) = pole_to_fc_q(z.re, z.im);
                state.fc = fc;
                state.q = q;
                state.dragging = Some(DragTarget::Pole1);
                param_changed = true;
            }
        }
    }

    if response.dragged() {
        if let (Some(target), Some(pos)) = (state.dragging, pointer_pos) {
            let z = from_screen(pos);
            match target {
                DragTarget::Pole1 | DragTarget::Pole2 => {
                    let (fc, q) = pole_to_fc_q(z.re, z.im);
                    state.fc = fc;
                    state.q = q;
                    param_changed = true;
                }
                DragTarget::Zero1 | DragTarget::Zero2 => {
                    let theta = z.im.abs().atan2(z.re);
                    let fc = (theta / (2.0 * PI)).clamp(0.005, 0.495);
                    state.fc = fc;
                    param_changed = true;
                }
            }
        }
    }

    if response.drag_stopped() {
        state.dragging = None;
    }

    // Tooltip display on hover
    if let Some(pos) = pointer_pos {
        if pos.distance(p1_screen) < 16.0 || pos.distance(p2_screen) < 16.0 {
            response.on_hover_text(format!(
                "Pole z = {:.3} ± {:.3} j\n|z| = {:.3} (Stability: {})\nDrag to adjust Cutoff & Resonance",
                pole1.re,
                pole1.im.abs(),
                pole1.mag(),
                if pole1.mag() < 1.0 { "Stable" } else { "Unstable" }
            ));
        } else if pos.distance(to_screen(zero1)) < 16.0 || pos.distance(to_screen(zero2)) < 16.0 {
            response.on_hover_text(format!(
                "Zero z = {:.3} ± {:.3} j\n|z| = {:.3}\nDrag to adjust Notch Center Frequency",
                zero1.re,
                zero1.im.abs(),
                zero1.mag()
            ));
        }
    }

    param_changed
}

fn render_bode_plot_canvas(
    ui: &mut Ui,
    size: Vec2,
    coeffs: &BiquadCoeffs,
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

    let min_db = -60.0_f64;
    let max_db = 18.0_f64;

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
    for db in [-60.0, -40.0, -20.0, -3.0, 0.0, 12.0] {
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
        let (mag_db, _) = coeffs.eval_frequency_response(omega);

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
    let (fc_db, _) = coeffs.eval_frequency_response(2.0 * PI * fc);
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
            let (hover_db, _) = coeffs.eval_frequency_response(2.0 * PI * hover_f);
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
                "Freq: {:.4} ({:.1} Hz)\nGain: {:+.2} dB",
                hover_f,
                hover_f * sample_rate,
                hover_db
            ));
        }
    }
}
