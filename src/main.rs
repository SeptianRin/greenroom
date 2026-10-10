#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod harness;
mod runner;

fn main() -> eframe::Result {
    let (cfg, path, note) = config::load();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([960.0, 700.0])
            .with_min_inner_size([680.0, 460.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Greenroom",
        options,
        Box::new(move |cc| {
            app::install_theme(&cc.egui_ctx);
            Ok(Box::new(app::GreenroomApp::new(cfg, path, note)))
        }),
    )
}
