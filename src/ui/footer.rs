use crate::types::{AppState, ExportFormat};
use crate::ui::colors;
use egui::{Color32, Vec2};
use rfd::FileDialog;
use std::path::Path;

pub fn draw_footer(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let export_clicked = false;
    let width = ui.available_width();

    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        draw_view_controls(ui, state);

        if width > 560.0 {
            ui.separator();
            ui.label("🖱 Drag to pan, scroll to zoom");
        }

        ui.separator();
        draw_export_controls(ui, state);
    });

    export_clicked
}

fn draw_view_controls(ui: &mut egui::Ui, state: &mut AppState) {
    if ui.button("🔄 Reset View").clicked() {
        state.zoom = 1.0;
        state.pan_offset = Vec2::ZERO;
    }
    ui.label(format!("🔍 Zoom: {:.1}x", state.zoom));
}

fn draw_export_controls(ui: &mut egui::Ui, state: &mut AppState) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.scope(|ui| {
            apply_export_button_style(ui);
            let response =
                ui.add_enabled(state.preview_ready, egui::Button::new("💾 Export Image"));
            if response.clicked() {
                show_file_dialog(state);
            }
        });
        // Format selection ComboBox
        egui::ComboBox::from_id_salt("export_format_footer")
            .selected_text(state.selected_export_format.display_name())
            .width(64.0)
            .show_ui(ui, |ui| {
                for format in ExportFormat::all() {
                    ui.selectable_value(
                        &mut state.selected_export_format,
                        format.clone(),
                        format.display_name(),
                    );
                }
            });
    });
}

fn apply_export_button_style(ui: &mut egui::Ui) {
    ui.style_mut().spacing.button_padding = egui::vec2(10.0, 4.0);
    let style = &mut ui.style_mut();

    // style.spacing.button_padding = egui::vec2(10.0, 4.0);

    // Inactive state
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.inactive.weak_bg_fill = colors::COLOR_TINT;

    // Hovered state
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, colors::COLOR_TINT_ACTIVE);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.hovered.weak_bg_fill = colors::COLOR_TINT;

    // Active state
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, colors::COLOR_TINT_ACTIVE);
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.active.weak_bg_fill = colors::COLOR_TINT;
}

fn show_file_dialog(state: &AppState) {
    if let Some(input_path) = &state.input_path {
        let default_path = get_default_export_path(state);
        let format = &state.selected_export_format;

        let mut dialog = FileDialog::new().add_filter(
            &format!("{} files", format.display_name()),
            &[format.extension()],
        );

        // Set default filename and directory if available
        if let Some(default_path) = default_path {
            // Change extension based on selected format
            let mut new_path = default_path.clone();
            new_path.set_extension(format.extension());

            if let Some(filename) = new_path.file_name() {
                dialog = dialog.set_file_name(filename.to_string_lossy().to_string());
            }
            if let Some(parent) = new_path.parent() {
                dialog = dialog.set_directory(parent);
            }
        }

        if let Some(output_path) = dialog.save_file() {
            export_image_async(state, input_path.clone(), output_path.display().to_string());
        }
    }
}

fn get_default_export_path(state: &AppState) -> Option<std::path::PathBuf> {
    match (&state.output_path, &state.output_name) {
        (Some(output_path), output_name) if !output_name.is_empty() => {
            Some(Path::new(output_path).join(output_name))
        }
        _ => None,
    }
}

fn export_image_async(state: &AppState, input_path: String, output_path: String) {
    let settings = state.settings.clone();
    let color_correction = state.color_correction.clone();
    let export_format = state.selected_export_format.clone();

    std::thread::spawn(move || {
        let _ = crate::image_processing::ImageProcessor::export_image(
            input_path,
            output_path,
            settings,
            color_correction,
            export_format,
        );
    });
}
