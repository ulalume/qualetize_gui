mod footer;
mod header;
mod image_viewer;
mod settings_panel;
pub mod styles;

pub use footer::draw_footer;
pub use header::draw_header;
pub use image_viewer::{draw_image_view, draw_main_content};
pub use settings_panel::draw_settings_panel;
