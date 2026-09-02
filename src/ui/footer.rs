use super::styles;
use super::widgets;
use crate::types::{
    AppState, ExportFormat,
    app_state::{AppStateRequest, ExportSource},
};
use egui::{Color32, Vec2};

pub fn draw_footer(ui: &mut egui::Ui, state: &mut AppState) {
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
}

fn draw_view_controls(ui: &mut egui::Ui, state: &mut AppState) {
    let width = ui.available_width();
    if ui
        .button(if width > 360.0 {
            "🔄 Reset zoom"
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
                egui::Button::new("💾 Export image"),
            );
            if response.clicked() {
                // The button exports whatever the last enabled pass produced,
                // i.e. the image shown in the bottom right view.
                let source = if state.settings.tile_reduce_post_enabled {
                    ExportSource::TileReduced
                } else {
                    ExportSource::Qualetized
                };
                _ = state
                    .app_state_request_sender
                    .send(AppStateRequest::ExportImageDialog {
                        source,
                        format: state.preferences.selected_export_format,
                    });
            }
        });

        // Format selection ComboBox
        widgets::EnumCombo::new(
            "export_format_footer",
            ExportFormat::indexed_list(),
            ExportFormat::display_name,
        )
        .width(64.0)
        .show(ui, &mut state.preferences.selected_export_format);
        let count_label = match state.reduced_tile_count {
            Some(count) => format!("Tiles: {count}"),
            None => "Tiles: --".to_string(),
        };
        ui.menu_button(egui::RichText::new(count_label).strong(), |ui| {
            let mut options_changed = false;

            for (value, label) in [
                (
                    &mut state.tile_count.settings.allow_flip_x,
                    "Allowed X flips",
                ),
                (
                    &mut state.tile_count.settings.allow_flip_y,
                    "Allowed Y flips",
                ),
            ] {
                options_changed |= ui.checkbox(value, label).changed();
            }

            ui.separator();

            options_changed |= ui
                .checkbox(
                    &mut state.tile_count.settings.visible_only,
                    "Ignore fully transparent tiles",
                )
                .changed();

            if options_changed {
                state.tile_count.mark_dirty();
            }
        });
    });
}

fn apply_export_button_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.spacing.button_padding = egui::vec2(10.0, 4.0);

    // Inactive state
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.inactive.weak_bg_fill = styles::COLOR_TINT;

    // Hovered state
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, styles::COLOR_TINT_ACTIVE);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.hovered.weak_bg_fill = styles::COLOR_TINT;

    // Active state
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, styles::COLOR_TINT_ACTIVE);
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
    style.visuals.widgets.active.weak_bg_fill = styles::COLOR_TINT;
}
