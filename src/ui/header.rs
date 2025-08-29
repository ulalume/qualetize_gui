use crate::types::AppState;

pub fn draw_header(ui: &mut egui::Ui, state: &mut AppState) -> bool {
    let mut changed = false;

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // Display options
        let palette_checkbox = ui.checkbox(&mut state.show_palettes, "Show Palettes");
        if palette_checkbox.changed() {
            changed = true;
        }

        ui.separator();

        let original_checkbox = ui.checkbox(&mut state.show_original_image, "Show Original Image");
        if original_checkbox.changed() {
            changed = true;
        }
    });
    changed
}
