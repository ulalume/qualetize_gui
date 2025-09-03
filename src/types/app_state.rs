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
pub enum ExportRequest {
    ColorCorrectedPng {
        output_path: String,
    },
    QualetizedIndexed {
        output_path: String,
        format: ExportFormat,
    },
}

// Settings save/load request types
#[derive(Debug, Clone)]
pub enum SettingsRequest {
    Save { path: String },
    Load { path: String },
}

pub struct AppState {
    // ファイル管理
    pub input_path: Option<String>,
    pub input_image: Option<ImageData>,
    pub color_corrected_image: Option<ImageData>,
    pub output_image: Option<ImageData>,

    // ユーザー設定
    pub preferences: UserPreferences,
    pub last_preferences: UserPreferences,

    // 画像表示制御
    pub zoom: f32,
    pub pan_offset: Vec2,

    // 設定
    pub settings: QualetizeSettings,

    pub color_correction: ColorCorrection,
    pub last_color_correction: ColorCorrection,

    // 処理状態
    pub settings_changed: bool,

    // 警告状態
    pub tile_size_warning: bool,

    // デバウンス機能
    pub last_settings_change_time: Option<std::time::Instant>,
    pub debounce_delay: std::time::Duration,

    // Export requests
    pub pending_export_request: Option<ExportRequest>,

    // Settings save/load requests
    pub pending_settings_request: Option<SettingsRequest>,
}

impl Default for AppState {
    fn default() -> Self {
        let preferences = UserPreferences::load();
        Self {
            input_path: None,
            input_image: None,
            color_corrected_image: None,
            output_image: None,

            preferences: preferences.clone(),
            last_preferences: preferences.clone(),

            zoom: 1.0,
            pan_offset: Vec2::ZERO,

            settings: QualetizeSettings::default(),

            last_color_correction: ColorCorrection::default(),
            color_correction: ColorCorrection::default(),

            settings_changed: false,

            // 警告状態
            tile_size_warning: false,

            // デバウンス機能 - 100msの遅延（応答性向上）
            last_settings_change_time: None,
            debounce_delay: std::time::Duration::from_millis(100),

            pending_export_request: None,

            pending_settings_request: None,
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
