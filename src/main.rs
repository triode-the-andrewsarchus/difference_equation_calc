use difference_equation_calc::ui::DifferenceEquationApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Difference Equation Calculator — Interacting with Behavior"),
        ..Default::default()
    };

    eframe::run_native(
        "Difference Equation Calculator",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(DifferenceEquationApp::new(cc)))
        }),
    )
}
