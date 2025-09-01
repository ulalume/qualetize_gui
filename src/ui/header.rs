use crate::types::{
    AppState, ColorCorrectionPreset, ExportFormat, QualetizePreset, app_state::AppearanceMode,
};
use rfd::FileDialog;

pub fn draw_header(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    egui::MenuBar::new().ui(ui, |ui| {
        // --- File menu ---
        ui.menu_button("File", |ui| {
            if ui.button("Open Image...").clicked() {
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
                ui.close();
            }
            ui.separator();

            ui.menu_button("Export Image", |ui| {
                ui.add_enabled_ui(state.preview_ready, |ui| {
                    if ui.button("Color Correction PNG").clicked() {
                        // export
                        ui.close();
                    }
                    if ui.button("Color Correction BMP").clicked() {
                        ui.close();
                    }
                    if ui.button("Qualetized PNG").clicked() {
                        ui.close();
                    }
                    if ui.button("Qualetized BMP").clicked() {
                        ui.close();
                    }
                });
            });

            ui.separator();

            ui.menu_button("Settings", |ui| {
                ui.button("Save Settings...");
                ui.button("Load Settings...");
            });
        });

        // --- Edit menu ---
        ui.menu_button("Edit", |ui| {
            ui.menu_button("Reset Qualetize", |ui| {
                for preset in QualetizePreset::all() {
                    if ui.button(preset.display_name()).clicked() {
                        state.settings = preset.qualetize_settings();
                        settings_changed = true;
                        ui.close();
                    }
                }
            });
            ui.menu_button("Reset Color Correction", |ui| {
                for preset in ColorCorrectionPreset::all() {
                    if ui.button(preset.display_name()).clicked() {
                        state.color_correction = preset.color_correction();
                        settings_changed = true;
                        ui.close();
                    }
                }
            });
            ui.separator();
            ui.menu_button("Zoom", |ui| {
                if ui.button("Zoom 1x").clicked() {
                    state.zoom = 1.0;
                    state.pan_offset = egui::Vec2::ZERO;
                    ui.close();
                }
                if ui.button("Zoom 2x").clicked() {
                    state.zoom = 2.0;
                    state.pan_offset = egui::Vec2::ZERO;
                    ui.close();
                }
                if ui.button("Zoom 4x").clicked() {
                    state.zoom = 4.0;
                    state.pan_offset = egui::Vec2::ZERO;
                    ui.close();
                }
                if ui.button("Zoom 8x").clicked() {
                    state.zoom = 8.0;
                    state.pan_offset = egui::Vec2::ZERO;
                    ui.close();
                }
            });
            ui.separator();
            ui.menu_button("Export Format", |ui| {
                for format in ExportFormat::all() {
                    if ui
                        .selectable_value(
                            &mut state.selected_export_format,
                            format.clone(),
                            format.display_name(),
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                }
            });
        });

        // --- View menu ---
        ui.menu_button("View", |ui| {
            ui.checkbox(&mut state.show_advanced, "Advanced Settings");
            ui.checkbox(&mut state.show_debug_info, "Debug Info");

            ui.separator();

            ui.checkbox(&mut state.show_original_image, "Original Image");
            ui.checkbox(
                &mut state.show_color_corrected_image,
                "Color Corrected Image",
            );

            ui.separator();

            ui.checkbox(&mut state.show_palettes, "Palettes");

            ui.separator();

            ui.checkbox(&mut state.show_appearance_dialog, "Appearance");
        });
    });

    let mut show_dialog = state.show_appearance_dialog;
    if let Some(_) = egui::Window::new("Appearance")
        .open(&mut show_dialog)
        .resizable(false)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            ui.strong("Theme");
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut state.appearance_mode,
                    AppearanceMode::System,
                    "System Default",
                );
                ui.selectable_value(&mut state.appearance_mode, AppearanceMode::Light, "Light");
                ui.selectable_value(&mut state.appearance_mode, AppearanceMode::Dark, "Dark");
            });
            ui.separator();

            ui.strong("Image Background Color");
            ui.horizontal(|ui| {
                // Use selectable_value for Default/Custom selection
                let mut use_default = state.background_color.is_none();

                if ui
                    .selectable_value(&mut use_default, true, "Default")
                    .changed()
                {
                    if use_default {
                        state.background_color = None;
                    }
                }

                if ui
                    .selectable_value(&mut use_default, false, "Custom")
                    .changed()
                {
                    if !use_default {
                        // Set to a default color when switching to custom
                        state.background_color = Some(egui::Color32::from_gray(64));
                    }
                }

                // Show color picker only when using custom
                if !use_default {
                    if let Some(ref mut color) = state.background_color {
                        let mut color_array = [color.r(), color.g(), color.b()];
                        if ui.color_edit_button_srgb(&mut color_array).changed() {
                            *color = egui::Color32::from_rgb(
                                color_array[0],
                                color_array[1],
                                color_array[2],
                            );
                        }

                        // Show current color as text
                        ui.label(format!(
                            "#{:02X}{:02X}{:02X}",
                            color.r(),
                            color.g(),
                            color.b()
                        ));
                    }
                }
            });

            ui.separator();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui.button("Reset View Settings").clicked() {
                    state.reset_view_settings();
                }
            });
        })
    {
        state.show_appearance_dialog = show_dialog;
    }

    settings_changed
}
