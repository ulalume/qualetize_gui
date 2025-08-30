use egui::Vec2;
use std::path::PathBuf;

use super::{
    export::ExportFormat,
    image::{ColorCorrection, ImageData},
    settings::QualetizeSettings,
};

pub struct AppState {
    // ファイル管理
    pub input_path: Option<String>,
    pub output_path: Option<String>,
    pub output_name: String,
    pub input_image: ImageData,
    pub output_image: ImageData,

    // UI状態
    pub show_advanced: bool,
    pub show_original_image: bool,
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
    preferences: UserPreferences,
}

impl Default for AppState {
    fn default() -> Self {
        let preferences = UserPreferences::load();
        Self {
            input_path: None,
            output_path: None,
            output_name: String::new(),
            input_image: ImageData::default(),
            output_image: ImageData::default(),

            show_advanced: preferences.show_advanced,
            show_original_image: preferences.show_original_image,
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
        }
    }
}

impl AppState {
    pub fn check_and_save_preferences(&mut self) {
        if self.show_advanced != self.preferences.show_advanced
            || self.show_original_image != self.preferences.show_original_image
            || self.show_palettes != self.preferences.show_palettes
            || self.selected_export_format != self.preferences.selected_export_format
        {
            self.preferences.show_advanced = self.show_advanced;
            self.preferences.show_original_image = self.show_original_image;
            self.preferences.show_palettes = self.show_palettes;
            self.preferences.selected_export_format = self.selected_export_format.clone();
            if let Err(e) = self.preferences.save() {
                eprintln!("Failed to save preferences: {}", e);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct UserPreferences {
    pub show_advanced: bool,
    pub show_original_image: bool,
    pub show_palettes: bool,
    pub selected_export_format: ExportFormat,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            show_advanced: false,
            show_original_image: true,
            show_palettes: true,
            selected_export_format: ExportFormat::default(),
        }
    }
}

impl UserPreferences {
    pub fn config_path() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("QualetizeGUI").join("preferences.json")
        } else {
            PathBuf::from("preferences.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(prefs) = serde_json::from_str(&content) {
                return prefs;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
