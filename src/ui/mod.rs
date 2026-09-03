mod footer;
mod header;
mod image_viewer;
mod results_panel;
mod settings_panel;
pub mod styles;
mod widgets;

pub use footer::draw_footer;
pub use header::draw_header;
pub use image_viewer::{draw_image_view, draw_main_content};
pub use results_panel::draw_results_panel;
pub use settings_panel::draw_settings_panel;
