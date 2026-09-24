use crate::document::Document;
use crate::parser::ParseError;
use crate::query::SearchResult;
use crate::ui::icons::Icon;
use egui::{Color32, Ui};

pub fn render_search_bar(
    ui: &mut Ui,
    doc: &mut Document,
    search_result: &SearchResult,
    search_parse_error: Option<&ParseError>,
) {
    ui.horizontal(|ui| {
        Icon::Search.ui(ui, 13.0, Color32::from_rgb(0, 220, 255));
        ui.label(
            egui::RichText::new("Search:")
                .strong()
                .color(Color32::from_rgb(0, 220, 255)),
        );

        let _response = ui.add(
            egui::TextEdit::singleline(&mut doc.state.search_query)
                .desired_width(260.0)
                .font(egui::TextStyle::Monospace)
                .hint_text("e.g. a[n+1] > a[n], a[n] > 0"),
        );

        // Search match statistics badge
        if let Some(err) = search_parse_error {
            Icon::Alert.ui(ui, 11.0, Color32::from_rgb(255, 90, 90));
            ui.label(
                egui::RichText::new(&err.message)
                    .color(Color32::from_rgb(255, 90, 90))
                    .size(11.0),
            );
        } else if !doc.state.search_query.trim().is_empty() {
            let total_samples = search_result.mask.len();
            let pct = if total_samples > 0 {
                (search_result.total_matches as f64 * 100.0) / (total_samples as f64)
            } else {
                0.0
            };

            ui.label(
                egui::RichText::new(format!(
                    "{} samples ({:.1}%) across {} regions",
                    search_result.total_matches,
                    pct,
                    search_result.regions.len()
                ))
                .monospace()
                .size(11.0)
                .color(Color32::from_rgb(0, 230, 200)),
            );

            // Region selection buttons
            if !search_result.regions.is_empty()
                && ui
                    .small_button("Select 1st Region")
                    .on_hover_text("Set horizontal selection to the first matching search region")
                    .clicked()
            {
                let r = &search_result.regions[0];
                if r.end > r.start {
                    doc.state.selection = Some((r.start, r.end - 1));
                }
            }
        }

        // Preset query chips
        ui.separator();
        ui.label(
            egui::RichText::new("Presets:")
                .size(11.0)
                .color(Color32::from_rgb(130, 140, 160)),
        );

        let presets = [
            "a[n+1] > a[n]",
            "a[n] > 0",
            "abs(a[n]) < 0.1",
            "b[n] > 0 and a[n] < 0",
            "c[n] == 1",
        ];

        for &preset in &presets {
            if ui
                .small_button(egui::RichText::new(preset).monospace().size(10.0))
                .clicked()
            {
                doc.state.search_query = preset.to_string();
            }
        }
    });
}
