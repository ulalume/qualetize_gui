use egui::Vec2;
use std::sync::{Arc, atomic::AtomicBool, mpsc};

use super::{
    color_correction::ColorCorrection,
    export::ExportFormat,
    image::{ImageData, ImageDataIndexed, PaletteSortSettings},
    preferences::UserPreferences,
    qualetize::QualetizeSettings,
};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Default,
)]
pub enum AppearanceMode {
    #[default]
    System,
    Light,
    Dark,
}

// Export request types
#[derive(Debug, Clone)]
pub enum AppStateRequest {
    LoadImage {
        path: String,
    },
    ColorCorrectedPng {
        output_path: String,
    },
    QualetizedIndexed {
        output_path: String,
        format: ExportFormat,
    },
    SaveSettings {
        path: String,
    },
    LoadSettings {
        path: String,
    },

    OpenImageDialog,
    ExportImageDialog {
        format: ExportFormat,
        suffix: Option<String>,
    },
    SaveSettingsDialog,
    LoadSettingsDialog,
}

#[derive(Debug, Clone)]
pub struct QualetizeRequest {
    pub time: std::time::Instant,
}

pub struct AppState {
    // Image management
    pub input_path: Option<String>,
    pub input_image: Option<ImageData>,
    pub color_corrected_image: Option<ImageData>,
    pub output_image: Option<ImageData>,
    pub output_palette_sorted_indexed_image: Option<ImageDataIndexed>,

    // View Settings
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub preferences: UserPreferences,
    last_preferences: UserPreferences,

    // Qualetize Settings
    pub settings: QualetizeSettings,
    pub request_update_qualetized_image: Option<QualetizeRequest>,
    pub debounce_delay: std::time::Duration,

    // Color Correction Settings
    pub color_correction: ColorCorrection,
    last_color_correction: ColorCorrection,

    // Palette Sort Settings
    pub palette_sort_settings: PaletteSortSettings,
    last_palette_sort_settings: PaletteSortSettings,

    // warning
    pub tile_size_warning: bool,

    // Export requests
    pub app_state_request_receiver: mpsc::Receiver<AppStateRequest>,
    pub app_state_request_sender: mpsc::Sender<AppStateRequest>,

    pub file_dialog_open: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        let preferences = UserPreferences::load();
        let (sender, receiver) = mpsc::channel();

        Self {
            input_path: None,
            input_image: None,
            color_corrected_image: None,
            output_image: None,
            output_palette_sorted_indexed_image: None,

            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            preferences: preferences.clone(),
            last_preferences: preferences.clone(),

            settings: QualetizeSettings::default(),
            request_update_qualetized_image: None,
            debounce_delay: std::time::Duration::from_millis(100),

            last_color_correction: ColorCorrection::default(),
            color_correction: ColorCorrection::default(),

            palette_sort_settings: PaletteSortSettings::default(),
            last_palette_sort_settings: PaletteSortSettings::default(),

            tile_size_warning: false,

            app_state_request_receiver: receiver,
            app_state_request_sender: sender,

            file_dialog_open: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AppState {
    pub fn tile_size_warning_message(&self) -> String {
        let Some(input_image) = &self.input_image else {
            return String::new();
        };
        format!(
            "Image size ({}×{}) is not divisible by tile size ({}×{}). Qualetize processing cannot proceed.",
            input_image.width,
            input_image.height,
            self.settings.tile_width,
            self.settings.tile_height,
        )
    }

    pub fn check_and_save_preferences(&mut self) {
        if self.preferences != self.last_preferences {
            self.last_preferences = self.preferences.clone();
            if let Err(e) = self.preferences.save() {
                eprintln!("Failed to save preferences: {e}");
            }
        }
    }

    pub fn palette_sort_settings_changed(&self) -> bool {
        self.palette_sort_settings != self.last_palette_sort_settings
    }

    /// Update the tracked color correction settings
    pub fn update_palette_sort_settings_tracking(&mut self) {
        self.last_palette_sort_settings = self.palette_sort_settings.clone();
    }

    /// Check if color correction settings have changed
    pub fn color_correction_changed(&self) -> bool {
        self.color_correction != self.last_color_correction
    }

    /// Update the tracked color correction settings
    pub fn update_color_correction_tracking(&mut self) {
        self.last_color_correction = self.color_correction.clone();
    }

    pub fn reset_view_settings(&mut self) {
        self.preferences = UserPreferences::default();
    }
}
