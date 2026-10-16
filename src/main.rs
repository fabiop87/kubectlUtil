#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
mod action;
mod app;
mod command;
mod theme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 820.0])
            .with_min_inner_size([860.0, 700.0])
            .with_title("kubectlUtil"),
        ..Default::default()
    };

    eframe::run_native(
        "kubectlUtil",
        options,
        Box::new(|cc| {
            // Fontes, paleta (claro/escuro), cantos arredondados e espaçamento.
            theme::install(&cc.egui_ctx);
            // O contexto é entregue ao app para que threads de trabalho possam
            // pedir um novo desenho assim que um resultado estiver pronto.
            Ok(Box::new(app::KubectlUtilApp::new(cc.egui_ctx.clone())))
        }),
    )
}
