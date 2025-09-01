use super::export::ExportFormat;
use crate::types::app_state::AppearanceMode;
use egui::Color32;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SerdeColor32(u8, u8, u8, u8);

impl From<Color32> for SerdeColor32 {
    fn from(c: Color32) -> Self {
        Self(c.r(), c.g(), c.b(), c.a())
    }
}
impl From<SerdeColor32> for Color32 {
    fn from(c: SerdeColor32) -> Self {
        Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3)
    }
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserPreferences {
    pub show_advanced: bool,
    pub show_original_image: bool,
    pub show_color_corrected_image: Option<bool>,
    pub show_palettes: bool,
    pub show_debug_info: Option<bool>,
    pub selected_export_format: ExportFormat,

    pub appearance_mode: Option<AppearanceMode>,
    pub background_color: Option<SerdeColor32>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            show_advanced: false,
            show_original_image: true,
            show_color_corrected_image: Some(false),
            show_palettes: true,
            show_debug_info: Some(false),
            selected_export_format: ExportFormat::default(),
            appearance_mode: Some(AppearanceMode::default()),
            background_color: None,
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
