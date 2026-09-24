use crate::document::{Document, ParameterDoc};
use crate::ui::icons::Icon;
use egui::{Color32, Ui};

pub fn render_parameters_panel(ui: &mut Ui, doc: &mut Document) {
    let mut delete_param_idx = None;
    let mut add_param = false;

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
            let param_count = doc.state.parameters.len();
            for i in 0..param_count {
                let p = &mut doc.state.parameters[i];
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&p.name)
                            .monospace()
                            .strong()
                            .color(Color32::from_rgb(255, 200, 100)),
                    );

                    let slider = egui::Slider::new(&mut p.value, p.min..=p.max)
                        .step_by(p.step)
                        .show_value(true)
                        .max_decimals(4);

                    ui.add(slider);

                    // Min/max config in tooltip or collapsible
                    ui.label(
                        egui::RichText::new(format!("[{:.2} .. {:.2}]", p.min, p.max))
                            .size(10.0)
                            .color(Color32::from_rgb(110, 120, 140)),
                    );

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
