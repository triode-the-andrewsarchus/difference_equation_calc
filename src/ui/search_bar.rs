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
                .desired_width(240.0)
                .font(egui::TextStyle::Monospace)
                .hint_text("e.g. a[n+1] > a[n], abs(a[n]) < 0.1"),
        );

        if !doc.state.search_query.is_empty()
            && ui
                .small_button("✕")
                .on_hover_text("Clear behavioral search query")
                .clicked()
        {
            doc.state.search_query.clear();
        }

        // Search match statistics badge & region navigation
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
                    "{} ({:.1}%) · {} regions",
                    search_result.total_matches,
                    pct,
                    search_result.regions.len()
                ))
                .monospace()
                .size(11.0)
                .color(Color32::from_rgb(0, 230, 200)),
            );

            // Region cycling buttons (Prev / Next matching region)
            if !search_result.regions.is_empty() {
                let current_sel_start = doc.state.selection.map(|(s0, _)| s0);
                let active_reg_idx = current_sel_start.and_then(|s0| {
                    search_result
                        .regions
                        .iter()
                        .position(|r| r.start == s0 && r.end.saturating_sub(1) >= r.start)
                });

                if ui
                    .small_button("◀ Prev")
                    .on_hover_text("Select previous matching behavioral region")
                    .clicked()
                {
                    let idx = match active_reg_idx {
                        Some(0) | None => search_result.regions.len() - 1,
                        Some(i) => i - 1,
                    };
                    let r = &search_result.regions[idx];
                    if r.end > r.start {
                        doc.state.selection = Some((r.start, r.end - 1));
                    }
                }

                let badge = match active_reg_idx {
                    Some(i) => format!("{}/{}", i + 1, search_result.regions.len()),
                    None => format!("–/{}", search_result.regions.len()),
                };
                ui.label(
                    egui::RichText::new(badge)
                        .monospace()
                        .size(10.0)
                        .color(Color32::from_rgb(160, 175, 195)),
                );

                if ui
                    .small_button("Next ▶")
                    .on_hover_text("Select next matching behavioral region")
                    .clicked()
                {
                    let idx = match active_reg_idx {
                        Some(i) if i + 1 < search_result.regions.len() => i + 1,
                        _ => 0,
                    };
                    let r = &search_result.regions[idx];
                    if r.end > r.start {
                        doc.state.selection = Some((r.start, r.end - 1));
                    }
                }
            }
        }

        // Context-aware preset query chips adapted to current document's variables
        ui.separator();
        ui.label(
            egui::RichText::new("Queries:")
                .size(11.0)
                .color(Color32::from_rgb(130, 140, 160)),
        );

        let (parsed_eqs, _) = doc.state.get_parsed_equations();
        let v0 = parsed_eqs
            .first()
            .map(|e| e.target_var.as_str())
            .unwrap_or("a");
        let v1 = parsed_eqs
            .get(1)
            .map(|e| e.target_var.as_str())
            .unwrap_or(v0);

        let dynamic_presets = [
            (
                "Rising",
                format!("{v0}[n+1] > {v0}[n]"),
                "Steps where primary variable is increasing",
            ),
            (
                "Peaks",
                format!("{v0}[n] > {v0}[n-1] and {v0}[n] >= {v0}[n+1]"),
                "Local maxima (crests) of primary variable",
            ),
            (
                "Zero-Cross",
                format!("{v0}[n] * {v0}[n-1] <= 0"),
                "Sign changes / zero crossings",
            ),
            (
                "Settled",
                format!("abs({v0}[n+1] - {v0}[n]) < 0.005"),
                "Near-stationary / settled regions",
            ),
            (
                if v0 != v1 { "Coupled +" } else { "Positive" },
                if v0 != v1 {
                    format!("{v0}[n] > 0 and {v1}[n] > 0")
                } else {
                    format!("{v0}[n] > 0")
                },
                "Regions where state variables are positive",
            ),
        ];

        for (label, query, tooltip) in dynamic_presets {
            let is_active = doc.state.search_query.trim() == query;
            let text = egui::RichText::new(format!("{label}: {query}"))
                .monospace()
                .size(10.0)
                .color(if is_active {
                    Color32::from_rgb(0, 230, 255)
                } else {
                    Color32::from_rgb(190, 200, 220)
                });
            if ui.small_button(text).on_hover_text(tooltip).clicked() {
                doc.state.search_query = query;
            }
        }
    });
}
