pub mod app_state;
pub mod color_space;
pub mod dither;
pub mod export;
pub mod image;
pub mod preferences;
pub mod settings;

// Re-export all public types for convenience
pub use app_state::AppState;
pub use color_space::ColorSpace;
pub use dither::DitherMode;
pub use export::ExportFormat;
pub use image::{ColorCorrection, ImageData};
pub use settings::{BGRA8, ClearColor, QualetizeSettings};
