use egui::{Color32, Vec2};

use super::{
    export::ExportFormat,
    image::{ColorCorrection, ImageData},
    preferences::{SerdeColor32, UserPreferences},
    settings::QualetizeSettings,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AppearanceMode {
    System,
    Light,
    Dark,
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
impl Default for AppearanceMode {
    fn default() -> Self {
        AppearanceMode::System
    }
}

pub struct AppState {
    // Appearance
    pub appearance_mode: AppearanceMode,
    pub background_color: Option<Color32>,

    // ファイル管理
    pub input_path: Option<String>,
    pub input_image: ImageData,
    pub color_corrected_image: ImageData,
    pub output_image: ImageData,

    // UI状態
    pub show_advanced: bool,
    pub show_original_image: bool,
    pub show_color_corrected_image: bool,
    pub show_palettes: bool,
    pub show_debug_info: bool,
    pub selected_export_format: ExportFormat,
    pub show_appearance_dialog: bool,

    // 画像表示制御
    pub zoom: f32,
    pub pan_offset: Vec2,

    // 設定
    pub settings: QualetizeSettings,
    pub color_correction: ColorCorrection,

    // 処理状態
    pub preview_ready: bool,
    pub preview_processing: bool,
    pub result_message: String,
    pub settings_changed: bool,

    // 警告状態
    pub tile_size_warning: bool,

    // デバウンス機能
    pub last_settings_change_time: Option<std::time::Instant>,
    pub debounce_delay: std::time::Duration,

    // ユーザー設定
    pub preferences: UserPreferences,

    // Color correction tracking
    pub last_color_correction: ColorCorrection,

    // Export requests
    pub pending_export_request: Option<ExportRequest>,

    // Settings save/load requests
    pub pending_settings_request: Option<SettingsRequest>,
}

impl Default for AppState {
    fn default() -> Self {
        let preferences = UserPreferences::load();
        Self {
            appearance_mode: preferences
                .appearance_mode
                .unwrap_or(AppearanceMode::System),
            background_color: if let Some(color) = preferences.background_color.clone() {
                Some(Color32::from(color))
            } else {
                None
            },

            input_path: None,
            input_image: ImageData::default(),
            color_corrected_image: ImageData::default(),
            output_image: ImageData::default(),

            show_advanced: preferences.show_advanced,
            show_original_image: preferences.show_original_image,
            show_color_corrected_image: preferences.show_color_corrected_image.unwrap_or(false),
            show_palettes: preferences.show_palettes,
            show_debug_info: preferences.show_debug_info.unwrap_or(false),
            selected_export_format: preferences.selected_export_format.clone(),
            show_appearance_dialog: false,

            zoom: 1.0,
            pan_offset: Vec2::ZERO,

            settings: QualetizeSettings::default(),
            color_correction: ColorCorrection::default(),

            preview_ready: false,
            preview_processing: false,
            result_message: String::new(),
            settings_changed: false,

            // 警告状態
            tile_size_warning: false,

            // デバウンス機能 - 100msの遅延（応答性向上）
            last_settings_change_time: None,
            debounce_delay: std::time::Duration::from_millis(100),

            preferences,
            last_color_correction: ColorCorrection::default(),

            pending_export_request: None,

            pending_settings_request: None,
        }
    }
}

impl AppState {
    pub fn tile_size_warning_message(&self) -> String {
        format!(
            "Image size ({}×{}) is not divisible by tile size ({}×{}). Qualetize processing cannot proceed.",
            self.input_image.width,
            self.input_image.height,
            self.settings.tile_width,
            self.settings.tile_height,
        )
    }
    fn same_color(&self) -> bool {
        match (
            self.preferences.background_color.clone(),
            self.background_color,
        ) {
            (Some(pref_color), Some(back_color)) => pref_color == SerdeColor32::from(back_color),
            (None, None) => true,
            _ => false,
        }
    }
    pub fn check_and_save_preferences(&mut self) {
        if self.show_advanced != self.preferences.show_advanced
            || self.show_original_image != self.preferences.show_original_image
            || self.show_palettes != self.preferences.show_palettes
            || self.selected_export_format != self.preferences.selected_export_format
            || self.show_color_corrected_image
                != self.preferences.show_color_corrected_image.unwrap_or(false)
            || self.show_debug_info != self.preferences.show_debug_info.unwrap_or(false)
            || self.appearance_mode
                != self
                    .preferences
                    .appearance_mode
                    .unwrap_or(AppearanceMode::System)
            || !self.same_color()
        {
            self.preferences.show_advanced = self.show_advanced;
            self.preferences.show_original_image = self.show_original_image;
            self.preferences.show_palettes = self.show_palettes;
            self.preferences.selected_export_format = self.selected_export_format.clone();
            self.preferences.show_color_corrected_image = Some(self.show_color_corrected_image);
            self.preferences.show_debug_info = Some(self.show_debug_info);

            self.preferences.appearance_mode = Some(self.appearance_mode);
            self.preferences.background_color = if let Some(col) = self.background_color {
                Some(SerdeColor32::from(col))
            } else {
                None
            };

            if let Err(e) = self.preferences.save() {
                eprintln!("Failed to save preferences: {}", e);
            }
        }
    }

    /// Check if color corrected image needs to be regenerated
    pub fn needs_color_correction_update(&self) -> bool {
        // If no color corrected image exists, it needs to be generated
        if self.color_corrected_image.texture.is_none() {
            return true;
        }

        // If input image changed, color corrected image needs update
        if self.input_image.width != self.color_corrected_image.width
            || self.input_image.height != self.color_corrected_image.height
        {
            return true;
        }

        // Could add more sophisticated checking here (e.g., timestamp comparison)
        false
    }

    /// Clear color corrected image when input changes
    pub fn invalidate_color_corrected_image(&mut self) {
        self.color_corrected_image = ImageData::default();
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
        let def = UserPreferences::default();
        self.show_advanced = def.show_advanced;
        self.show_original_image = def.show_original_image;
        self.show_palettes = def.show_palettes;
        self.selected_export_format = def.selected_export_format.clone();
        self.show_color_corrected_image = def.show_color_corrected_image.unwrap_or(false);
        self.show_debug_info = def.show_debug_info.unwrap_or(false);

        self.appearance_mode = def.appearance_mode.unwrap_or(AppearanceMode::System);
        self.background_color = if let Some(col) = def.background_color {
            Some(Color32::from(col))
        } else {
            None
        }
    }
}
