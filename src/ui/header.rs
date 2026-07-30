use crate::types::app_state::{AppStateRequest, ExportSource};
use crate::types::{
    AppState, ExportFormat, QualetizePreset, app_state::AppearanceMode,
    color_correction::ColorCorrectionPreset,
};
use crate::ui::styles::UiMarginExt;

/// Returns `(settings_changed, tile_reduce_changed)`, matching the settings
/// panel: resetting tile reduction only needs the post-pass to rerun.
pub fn draw_header(ui: &mut egui::Ui, state: &mut AppState) -> (bool, bool) {
    let mut settings_changed = false;
    let mut tile_reduce_changed = false;

    egui::MenuBar::new().ui(ui, |ui| {
        // --- File menu ---
        ui.menu_button("File", |ui| {
            if ui.button("Open Image...").clicked() {
                _ = state
                    .app_state_request_sender
                    .send(AppStateRequest::OpenImageDialog);
                ui.close();
            }
            ui.separator();

            ui.menu_button("Export Image", |ui| {
                // Passes that are switched off have nothing to export, so their
                // entries stay visible but disabled.
                const ENTRIES: [(ExportSource, ExportFormat, &str); 5] = [
                    (
                        ExportSource::ColorCorrected,
                        ExportFormat::Png,
                        "Color Corrected PNG",
                    ),
                    (
                        ExportSource::Qualetized,
                        ExportFormat::PngIndexed,
                        "Qualetized PNG",
                    ),
                    (
                        ExportSource::Qualetized,
                        ExportFormat::Bmp,
                        "Qualetized BMP",
                    ),
                    (
                        ExportSource::TileReduced,
                        ExportFormat::PngIndexed,
                        "Tile Reduced PNG",
                    ),
                    (
                        ExportSource::TileReduced,
                        ExportFormat::Bmp,
                        "Tile Reduced BMP",
                    ),
                ];

                for (source, format, label) in ENTRIES {
                    let enabled = state.can_export(source);
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        _ = state
                            .app_state_request_sender
                            .send(AppStateRequest::ExportImageDialog { source, format });
                        ui.close();
                    }
                }
            });

            ui.separator();

            ui.menu_button("Settings", |ui| {
                if ui.button("Load Settings...").clicked() {
                    ui.close();
                    _ = state
                        .app_state_request_sender
                        .send(AppStateRequest::LoadSettingsDialog);
                }
                if ui.button("Save Settings...").clicked() {
                    ui.close();
                    _ = state
                        .app_state_request_sender
                        .send(AppStateRequest::SaveSettingsDialog);
                }
            });
        });

        // --- Edit menu ---
        ui.menu_button("Edit", |ui| {
            ui.menu_button("Reset Qualetize", |ui| {
                for preset in QualetizePreset::all() {
                    if ui.button(preset.display_name()).clicked() {
                        state.settings.apply_preset(preset.qualetize_settings());
                        settings_changed = true;
                        ui.close();
                    }
                }
            });
            if ui.button("Reset Tile Reduction").clicked() {
                state.settings.reset_tile_reduce();
                tile_reduce_changed = true;
                ui.close();
            }
            ui.menu_button("Reset Color Correction", |ui| {
                for preset in ColorCorrectionPreset::all() {
                    if ui.button(preset.display_name()).clicked() {
                        state
                            .color_correction
                            .apply_preset(preset.color_correction());
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
                            &mut state.preferences.selected_export_format,
                            *format,
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
                ui.label(egui::widget_text::RichText::new("Settings").small());
                ui.checkbox(&mut state.preferences.show_advanced, "Advanced Settings");
                ui.checkbox(&mut state.preferences.show_debug_info, "Debug Info");

                ui.separator();
                ui.label(egui::widget_text::RichText::new("Canvas").small());
                ui.checkbox(&mut state.preferences.show_palettes, "Palettes");

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
                    .checkbox(&mut state.preferences.show_appearance, "Appearance")
                    .clicked()
                {
                    ui.close();
                }
            });
    });

    let mut show_dialog = state.preferences.show_appearance;
    if egui::Window::new("Appearance")
        .open(&mut show_dialog)
        .resizable(false)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            ui.subheading_with_margin("Theme");
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut state.preferences.appearance_mode,
                    AppearanceMode::System,
                    "System Default",
                );
                ui.selectable_value(
                    &mut state.preferences.appearance_mode,
                    AppearanceMode::Light,
                    "Light",
                );
                ui.selectable_value(
                    &mut state.preferences.appearance_mode,
                    AppearanceMode::Dark,
                    "Dark",
                );
            });
            ui.separator();

            ui.subheading_with_margin("Canvas Background Color");
            ui.horizontal(|ui| {
                // Use selectable_value for Default/Custom selection
                let mut use_default = state.preferences.background_color.is_none();

                if ui
                    .selectable_value(&mut use_default, true, "Default")
                    .changed()
                    && use_default
                {
                    state.preferences.background_color = None;
                }

                if ui
                    .selectable_value(&mut use_default, false, "Custom")
                    .changed()
                    && !use_default
                {
                    // Set to a default color when switching to custom
                    state.preferences.background_color = Some(egui::Color32::from_gray(64));
                }

                // Show color picker only when using custom
                if !use_default && let Some(ref mut color) = state.preferences.background_color {
                    let mut color_array = [color.r(), color.g(), color.b()];
                    if ui.color_edit_button_srgb(&mut color_array).changed() {
                        *color =
                            egui::Color32::from_rgb(color_array[0], color_array[1], color_array[2]);
                    }

                    // Show current color as text
                    ui.label(format!(
                        "#{:02X}{:02X}{:02X}",
                        color.r(),
                        color.g(),
                        color.b()
                    ));
                }
            });

            ui.separator();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if ui.button("Reset View Settings").clicked() {
                    state.reset_view_settings();
                }
            });
        })
        .is_some()
    {
        state.preferences.show_appearance = show_dialog;
    }

    (settings_changed, tile_reduce_changed)
}
