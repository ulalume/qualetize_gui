use egui::Vec2;

use super::{
    export::ExportFormat,
    image::{ColorCorrection, ImageData},
    preferences::UserPreferences,
    settings::QualetizeSettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AppearanceMode {
    System,
    Light,
    Dark,
}

impl Default for AppearanceMode {
    fn default() -> Self {
        AppearanceMode::System
    }
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
}

pub struct AppState {
    // Image management
    pub input_path: Option<String>,
    pub input_image: Option<ImageData>,
    pub color_corrected_image: Option<ImageData>,
    pub output_image: Option<ImageData>,

    // View Settings
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub preferences: UserPreferences,
    pub last_preferences: UserPreferences,

    // Qualetize Settings
    pub settings: QualetizeSettings,
    pub settings_changed: bool,

    // Color Correction Settings
    pub color_correction: ColorCorrection,
    pub last_color_correction: ColorCorrection,

    // warning
    pub tile_size_warning: bool,

    // debounce
    pub last_settings_change_time: Option<std::time::Instant>,
    pub debounce_delay: std::time::Duration,

    // Export requests
    pub pending_app_state_request: Option<AppStateRequest>,
}

impl Default for AppState {
    fn default() -> Self {
        let preferences = UserPreferences::load();
        Self {
            input_path: None,
            input_image: None,
            color_corrected_image: None,
            output_image: None,

            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            preferences: preferences.clone(),
            last_preferences: preferences.clone(),

            settings: QualetizeSettings::default(),
            settings_changed: false,
            last_settings_change_time: None,
            debounce_delay: std::time::Duration::from_millis(100),

            last_color_correction: ColorCorrection::default(),
            color_correction: ColorCorrection::default(),

            tile_size_warning: false,

            pending_app_state_request: None,
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
                eprintln!("Failed to save preferences: {}", e);
            }
        }
    }

    /// Check if color corrected image needs to be regenerated
    pub fn needs_color_correction_update(&self) -> bool {
        self.color_corrected_image.is_none()
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
