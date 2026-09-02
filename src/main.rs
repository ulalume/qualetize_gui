#![windows_subsystem = "windows"]
use eframe::egui;
use qualetize_gui::app::QualetizeApp;

#[cfg(not(target_arch = "wasm32"))]
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

    // An image path on the command line is opened at startup.
    let initial = std::env::args()
        .nth(1)
        .map(|path| qualetize_gui::types::app_state::AppStateRequest::LoadImage { path });

    eframe::run_native(
        "Qualetize GUI - Image Quantization Tool",
        options,
        Box::new(move |cc| Ok(Box::new(QualetizeApp::new(cc, initial)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
