#![windows_subsystem = "windows"]
mod app;
mod color_processor;
mod image_processing;
mod types;
mod ui;

use app::QualetizeApp;
use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_drag_and_drop(true)
            .with_icon(egui::IconData::default())
            .with_title("Qualetize GUI - Image Quantization Tool"),
        ..Default::default()
    };

    eframe::run_native(
        "Qualetize GUI - Image Quantization Tool",
        options,
        Box::new(|cc| Ok(Box::new(QualetizeApp::new(cc)))),
    )
}
