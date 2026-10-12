#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod command;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1050.0, 760.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("kubectlUtil"),
        ..Default::default()
    };

    eframe::run_native(
        "kubectlUtil",
        options,
        Box::new(|cc| {
            let mut visuals = egui::Visuals::light();
            visuals.panel_fill = egui::Color32::from_rgb(247, 249, 252);
            visuals.window_fill = egui::Color32::WHITE;
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(app::KubectlUtilApp::default()))
        }),
    )
}
