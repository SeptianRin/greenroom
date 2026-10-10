#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod harness;
mod runner;

fn main() -> eframe::Result {
    let (cfg, path, note) = config::load();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([920.0, 660.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Greenroom",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
            Ok(Box::new(app::GreenroomApp::new(cfg, path, note)))
        }),
    )
}
