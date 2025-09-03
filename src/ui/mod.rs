mod footer;
mod header;
mod image_viewer;
mod settings_panel;
pub mod styles;

use crate::types::AppState;

pub struct UI;

impl UI {
    pub fn draw_settings_panel(ui: &mut egui::Ui, state: &mut AppState) -> bool {
        settings_panel::draw_settings_panel(ui, state)
    }

    pub fn draw_image_view(ui: &mut egui::Ui, state: &mut AppState, image_processing: bool) {
        image_viewer::draw_image_view(ui, state, image_processing)
    }

    pub fn draw_main_content(ui: &mut egui::Ui) {
        image_viewer::draw_main_content(ui)
    }

    pub fn draw_header(ui: &mut egui::Ui, state: &mut AppState) -> bool {
        header::draw_header(ui, state)
    }

    pub fn draw_footer(ui: &mut egui::Ui, state: &mut AppState) -> bool {
        footer::draw_footer(ui, state)
    }
}
