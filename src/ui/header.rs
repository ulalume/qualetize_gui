use crate::types::app_state::{AppStateRequest, ExportSource};
use crate::types::{
    AppState, ExportFormat, QualetizePreset, app_state::AppearanceMode,
    color_correction::ColorCorrectionPreset,
};
use crate::ui::styles::UiMarginExt;

/// Small app icon shown at the start of the menu bar, in points.
const MENU_ICON_SIZE: f32 = 16.0;

/// Returns `(settings_changed, tile_reduce_changed)`, matching the settings
/// panel: resetting tile reduction only needs the post-pass to rerun.
pub fn draw_header(ui: &mut egui::Ui, state: &mut AppState) -> (bool, bool) {
    let mut settings_changed = false;
    let mut tile_reduce_changed = false;

    egui::MenuBar::new().ui(ui, |ui| {
        // --- App icon ---
        let (small_icon, _) = state.app_icons(ui.ctx()).clone();
        if ui
            .add(
                egui::Button::image(
                    egui::Image::new(&small_icon)
                        .fit_to_exact_size(egui::vec2(MENU_ICON_SIZE, MENU_ICON_SIZE)),
                )
                .frame(false),
            )
            .on_hover_text("About QualetizeGUI")
            .clicked()
        {
            state.show_about = true;
        }
        ui.add_space(4.0);

        // --- File menu ---
        ui.menu_button("File", |ui| {
            if ui.button("Open image...").clicked() {
                _ = state
                    .app_state_request_sender
                    .send(AppStateRequest::OpenImageDialog);
                ui.close();
            }
            if ui
                .add_enabled(
                    state.input_image.is_some(),
                    egui::Button::new("Remove image"),
                )
                .clicked()
            {
                _ = state
                    .app_state_request_sender
                    .send(AppStateRequest::RemoveImage);
                ui.close();
            }
            ui.separator();

            ui.menu_button("Export image", |ui| {
                // Passes that are switched off have nothing to export, so their
                // entries stay visible but disabled.
                const ENTRIES: [(ExportSource, ExportFormat, &str); 5] = [
                    (
                        ExportSource::ColorCorrected,
                        ExportFormat::Png,
                        "Color corrected PNG",
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
                        "Tile reduced PNG",
                    ),
                    (
                        ExportSource::TileReduced,
                        ExportFormat::Bmp,
                        "Tile reduced BMP",
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
                if ui.button("Load settings...").clicked() {
                    ui.close();
                    _ = state
                        .app_state_request_sender
                        .send(AppStateRequest::LoadSettingsDialog);
                }
                if ui.button("Save settings...").clicked() {
                    ui.close();
                    _ = state
                        .app_state_request_sender
                        .send(AppStateRequest::SaveSettingsDialog);
                }
            });
        });

        // --- Edit menu ---
        ui.menu_button("Edit", |ui| {
            let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
            let redo_shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            );
            if ui
                .add_enabled(
                    state.history.can_undo(),
                    egui::Button::new("Undo")
                        .shortcut_text(ui.ctx().format_shortcut(&undo_shortcut)),
                )
                .clicked()
            {
                _ = state.app_state_request_sender.send(AppStateRequest::Undo);
                ui.close();
            }
            if ui
                .add_enabled(
                    state.history.can_redo(),
                    egui::Button::new("Redo")
                        .shortcut_text(ui.ctx().format_shortcut(&redo_shortcut)),
                )
                .clicked()
            {
                _ = state.app_state_request_sender.send(AppStateRequest::Redo);
                ui.close();
            }
            ui.separator();

            ui.menu_button("Reset qualetization", |ui| {
                for preset in QualetizePreset::all() {
                    if ui.button(preset.display_name()).clicked() {
                        state.apply_qualetize_preset(preset.qualetize_settings());
                        settings_changed = true;
                        ui.close();
                    }
                }
            });
            ui.menu_button("Reset color correction", |ui| {
                for preset in ColorCorrectionPreset::all() {
                    let enabled = match preset {
                        ColorCorrectionPreset::None => !state.color_correction.is_default(),
                        _ => true,
                    };
                    if ui
                        .add_enabled(enabled, egui::Button::new(preset.display_name()))
                        .clicked()
                    {
                        // Edits `state.color_correction`, not `state.settings`;
                        // app.rs detects that change itself, so this does not
                        // need to request a re-quantization.
                        state
                            .color_correction
                            .apply_preset(preset.color_correction());
                        ui.close();
                    }
                }
            });
            if ui
                .add_enabled(
                    !state.settings.tile_reduce_is_default(),
                    egui::Button::new("Reset tile reduction"),
                )
                .clicked()
            {
                state.settings.reset_tile_reduce();
                tile_reduce_changed = true;
                ui.close();
            }
            if ui
                .add_enabled(
                    !state.palette_sort_settings.is_default(),
                    egui::Button::new("Reset palette order"),
                )
                .clicked()
            {
                state.palette_sort_settings.reset();
                ui.close();
            }
            ui.separator();
            ui.menu_button("Export format", |ui| {
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
                ui.label(egui::widget_text::RichText::new("Canvas").small());
                ui.checkbox(&mut state.preferences.show_palettes, "Palettes");
                ui.checkbox(&mut state.preferences.show_results, "Results");

                ui.separator();

                ui.menu_button("Zoom", |ui| {
                    for z in [1.0, 2.0, 4.0, 8.0] {
                        if ui.button(format!("Zoom {}x", z as i32)).clicked() {
                            state.zoom = z;
                            state.pan_offset = egui::Vec2::ZERO;
                            ui.close();
                        }
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
    egui::Window::new("Appearance")
        .open(&mut show_dialog)
        .resizable(false)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            ui.subheading_with_margin("Theme");
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut state.preferences.appearance_mode,
                    AppearanceMode::System,
                    "System default",
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

            ui.subheading_with_margin("Canvas background color");
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
                if ui.button("Reset view settings").clicked() {
                    state.reset_view_settings();
                }
            });
        });
    state.preferences.show_appearance = show_dialog;

    (settings_changed, tile_reduce_changed)
}
