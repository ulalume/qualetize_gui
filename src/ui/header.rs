use crate::types::AppState;
use rfd::FileDialog;
use std::path::Path;

pub fn draw_header(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            settings_changed |= draw_file_selection(ui, state);
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Display options
            ui.checkbox(&mut state.show_palettes, "Palettes");
            ui.separator();
            ui.checkbox(
                &mut state.show_color_corrected_image,
                "Color Corrected Image",
            );
            ui.separator();
            ui.checkbox(&mut state.show_original_image, "Original Image");
            ui.separator();
            ui.checkbox(&mut state.show_advanced, "Advanced Settings");
            ui.label("View:");
        });
    });
    settings_changed
}

fn draw_file_selection(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    ui.horizontal(|ui| {
        if ui.button("📁 Select Input File").clicked() {
            if let Some(path) = FileDialog::new()
                .add_filter("Image files", &["png", "jpg", "jpeg", "bmp", "tga", "tiff"])
                .pick_file()
            {
                let path_str = path.display().to_string();
                state.input_path = Some(path_str.clone());
                state.preview_ready = false;
                state.preview_processing = false;
                state.output_image = Default::default();
                state.zoom = 1.0;
                state.pan_offset = egui::Vec2::ZERO;
                state.result_message = "File selected, loading...".to_string();

                // Set default output settings
                if let Some(parent) = path.parent() {
                    state.output_path = Some(parent.to_string_lossy().to_string());
                } else {
                    state.output_path = Some(".".to_string());
                }

                if let Some(stem) = path.file_stem() {
                    state.output_name = format!("{}_qualetized.bmp", stem.to_string_lossy());
                } else {
                    state.output_name = "output_qualetized.bmp".to_string();
                }

                settings_changed = true;
            }
        }

        if let Some(path) = &state.input_path {
            ui.label(format!(
                "📄 {}",
                Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
        }
    });

    settings_changed
}
