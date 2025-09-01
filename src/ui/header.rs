use crate::types::{
    AppState, ColorCorrectionPreset, ExportFormat, QualetizePreset,
    app_state::{AppearanceMode, ExportRequest},
};
use rfd::FileDialog;
use std::path::Path;

pub fn draw_header(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut settings_changed = false;

    egui::MenuBar::new().ui(ui, |ui| {
        // --- File menu ---
        ui.menu_button("File", |ui| {
            if ui.button("Open Image...").clicked() {
                ui.close();

                // Ensure proper resource cleanup by scoping the dialog
                let selected_path = {
                    let dialog = FileDialog::new()
                        .add_filter("Image files", &["png", "jpg", "jpeg", "bmp", "tga", "tiff"]);
                    dialog.pick_file()
                }; // dialog is dropped here

                if let Some(path) = selected_path {
                    let path_str = path.display().to_string();
                    state.input_path = Some(path_str.clone());
                    state.preview_ready = false;
                    state.preview_processing = false;
                    state.color_corrected_image = Default::default();
                    state.output_image = Default::default();
                    state.zoom = 1.0;
                    state.pan_offset = egui::Vec2::ZERO;
                    state.result_message = "File selected, loading...".to_string();

                    settings_changed = true;
                }
            }
            ui.separator();

            ui.menu_button("Export Image", |ui| {
                ui.add_enabled_ui(state.color_corrected_image.texture.is_some(), |ui| {
                    if ui.button("Color Correction PNG").clicked() {
                        request_export(state, ExportFormat::Png, Some("color_corrected"));
                        ui.close();
                    }
                });
                ui.add_enabled_ui(state.preview_ready, |ui| {
                    if ui.button("Qualetized Indexed PNG").clicked() {
                        request_export(state, ExportFormat::PngIndexed, Some("qualetized"));
                        ui.close();
                    }
                    if ui.button("Qualetized Indexed BMP").clicked() {
                        request_export(state, ExportFormat::Bmp, Some("qualetized"));
                        ui.close();
                    }
                });
            });

            ui.separator();

            ui.menu_button("Settings", |ui| {
                if ui.button("Save Settings...").clicked() {
                    //
                }
                if ui.button("Load Settings...").clicked() {
                    //
                }
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
            ui.menu_button("Export Format", |ui| {
                for format in ExportFormat::indexed_list() {
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
        egui::containers::menu::MenuButton::new("View")
            .config(
                egui::containers::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
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

                if ui
                    .checkbox(&mut state.show_appearance_dialog, "Appearance")
                    .clicked()
                {
                    ui.close();
                }
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

pub fn request_export(state: &mut AppState, format: ExportFormat, suffix: Option<&str>) {
    if let Some(input_path) = state.input_path.clone() {
        let default_path = get_export_path(input_path, &format, suffix);

        // Ensure proper resource cleanup by scoping the dialog and result
        let export_result = {
            let mut dialog = FileDialog::new().add_filter(
                &format!("{} files", format.display_name()),
                &[format.extension()],
            );

            if let Some(filename) = default_path.file_name() {
                dialog = dialog.set_file_name(filename.to_string_lossy().to_string());
            }
            if let Some(parent) = default_path.parent() {
                dialog = dialog.set_directory(parent);
            }

            dialog.save_file()
        }; // dialog is dropped here

        // Process result after dialog is cleaned up
        if let Some(output_path) = export_result {
            let export_request = match format {
                ExportFormat::Png => ExportRequest::ColorCorrectedPng {
                    output_path: output_path.display().to_string(),
                },
                _ => ExportRequest::QualetizedIndexed {
                    output_path: output_path.display().to_string(),
                    format,
                },
            };
            state.pending_export_request = Some(export_request);
        }
    }
}

fn get_export_path(
    input_path: String,
    format: &ExportFormat,
    suffix: Option<&str>,
) -> std::path::PathBuf {
    let path = Path::new(&input_path);

    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().unwrap_or(std::ffi::OsStr::new("output"));
    let new_name = if let Some(suffix) = suffix {
        format!("{}_{}", stem.to_string_lossy(), suffix)
    } else {
        stem.to_string_lossy().to_string()
    };
    parent.join(new_name).with_extension(format.extension())
}
