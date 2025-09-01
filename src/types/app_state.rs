use egui::Vec2;

use super::{
    export::ExportFormat,
    image::{ColorCorrection, ImageData},
    preferences::UserPreferences,
    settings::QualetizeSettings,
};

pub struct AppState {
    // ファイル管理
    pub input_path: Option<String>,
    pub output_path: Option<String>,
    pub output_name: String,
    pub input_image: ImageData,
    pub color_corrected_image: ImageData,
    pub output_image: ImageData,

    // UI状態
    pub show_advanced: bool,
    pub show_original_image: bool,
    pub show_color_corrected_image: bool,
    pub show_palettes: bool,
    pub selected_export_format: ExportFormat,

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
    pub tile_size_warning_message: String,

    // デバウンス機能
    pub last_settings_change_time: Option<std::time::Instant>,
    pub debounce_delay: std::time::Duration,

    // ユーザー設定
    pub preferences: UserPreferences,

    // Color correction tracking
    pub last_color_correction: ColorCorrection,
}

impl Default for AppState {
    fn default() -> Self {
        let preferences = UserPreferences::load();
        Self {
            input_path: None,
            output_path: None,
            output_name: String::new(),
            input_image: ImageData::default(),
            color_corrected_image: ImageData::default(),
            output_image: ImageData::default(),

            show_advanced: preferences.show_advanced,
            show_original_image: preferences.show_original_image,
            show_color_corrected_image: preferences.show_color_corrected_image.unwrap_or(false),
            show_palettes: preferences.show_palettes,
            selected_export_format: preferences.selected_export_format.clone(),

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
            tile_size_warning_message: String::new(),

            // デバウンス機能 - 100msの遅延（応答性向上）
            last_settings_change_time: None,
            debounce_delay: std::time::Duration::from_millis(100),

            preferences,
            last_color_correction: ColorCorrection::default(),
        }
    }
}

impl AppState {
    pub fn check_and_save_preferences(&mut self) {
        if self.show_advanced != self.preferences.show_advanced
            || self.show_original_image != self.preferences.show_original_image
            || self.show_palettes != self.preferences.show_palettes
            || self.selected_export_format != self.preferences.selected_export_format
            || self.show_color_corrected_image
                != self.preferences.show_color_corrected_image.unwrap_or(false)
        {
            self.preferences.show_advanced = self.show_advanced;
            self.preferences.show_original_image = self.show_original_image;
            self.preferences.show_palettes = self.show_palettes;
            self.preferences.selected_export_format = self.selected_export_format.clone();
            self.preferences.show_color_corrected_image = Some(self.show_color_corrected_image);
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
        if self.input_image.size != self.color_corrected_image.size {
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
}
