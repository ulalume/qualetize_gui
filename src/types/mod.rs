pub mod app_state;
pub mod color_correction;
pub mod color_space;
pub mod dither;
pub mod export;
pub mod image;
pub mod preferences;
pub mod qualetize;

// Re-export all public types for convenience
pub use app_state::AppState;
pub use color_correction::ColorCorrection;
pub use color_space::ColorSpace;
pub use dither::DitherMode;
pub use export::ExportFormat;
pub use image::{ImageData, ImageDataIndexed};
pub use qualetize::{BGRA8, ClearColor, QualetizePreset, QualetizeSettings};
