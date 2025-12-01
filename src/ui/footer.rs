use super::styles;
use crate::types::{AppState, ExportFormat, app_state::AppStateRequest};
use egui::{Color32, Vec2};

pub fn draw_footer(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let export_clicked = false;
    let width = ui.available_width();

    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        draw_view_controls(ui, state);

        if width > 660.0 {
            ui.separator();
            ui.label("🖱 Drag to pan, scroll to zoom");
        }

        ui.separator();
        draw_export_controls(ui, state);
    });

    export_clicked
}

fn draw_view_controls(ui: &mut egui::Ui, state: &mut AppState) {
    let width = ui.available_width();
    if ui
        .button(if width > 360.0 {
            "🔄 Reset Zoom"
        } else {
            "🔄"
        })
        .clicked()
    {
        state.zoom = 1.0;
        state.pan_offset = Vec2::ZERO;
    }
    if width > 460.0 {
        ui.label(format!("🔍 Zoom: {:.1}x", state.zoom));
    }
}

fn draw_export_controls(ui: &mut egui::Ui, state: &mut AppState) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.scope(|ui| {
            apply_export_button_style(ui);
            let response = ui.add_enabled(
                state.output_image.is_some(),
                egui::Button::new("💾 Export Image"),
            );
            if response.clicked() {
                _ = state
                    .app_state_request_sender
                    .send(AppStateRequest::ExportImageDialog {
                        format: state.preferences.selected_export_format.clone(),
                        suffix: Some("qualetized".to_string()),
                    });
            }
        });

        // Format selection ComboBox
        egui::ComboBox::from_id_salt("export_format_footer")
            .selected_text(state.preferences.selected_export_format.display_name())
            .width(64.0)
            .show_ui(ui, |ui| {
                for format in ExportFormat::indexed_list() {
                    ui.selectable_value(
                        &mut state.preferences.selected_export_format,
                        format.clone(),
                        format.display_name(),
                    );
                }
            });

        if let Some(count) = state.tile_count.last_count {
            ui.label(egui::RichText::new(format!("Tiles: {count}")).strong());
        }
    });
}

fn apply_export_button_style(ui: &mut egui::Ui) {
    ui.style_mut().spacing.button_padding = egui::vec2(10.0, 4.0);
    let style = &mut ui.style_mut();

    // style.spacing.button_padding = egui::vec2(10.0, 4.0);

    // Inactive state
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.inactive.weak_bg_fill = styles::COLOR_TINT;

    // Hovered state
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, styles::COLOR_TINT_ACTIVE);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.hovered.weak_bg_fill = styles::COLOR_TINT;

    // Active state
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, styles::COLOR_TINT_ACTIVE);
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.active.weak_bg_fill = styles::COLOR_TINT;
}
