use super::export::ExportFormat;
use crate::types::app_state::AppearanceMode;
use egui::Color32;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod color32_def {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(color: &Option<Color32>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match color {
            Some(c) => {
                let rgba = (c.r(), c.g(), c.b(), c.a());
                rgba.serialize(serializer)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Color32>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let rgba: Option<(u8, u8, u8, u8)> = Option::deserialize(deserializer)?;
        Ok(rgba.map(|(r, g, b, a)| Color32::from_rgba_premultiplied(r, g, b, a)))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserPreferences {
    pub show_advanced: bool,
    pub show_original_image: bool,

    #[serde(default)]
    pub show_color_corrected_image: bool,
    pub show_palettes: bool,

    #[serde(default)]
    pub show_debug_info: bool,
    #[serde(default)]
    pub show_appearance: bool,
    pub selected_export_format: ExportFormat,

    #[serde(default)]
    pub appearance_mode: AppearanceMode,

    #[serde(with = "color32_def")]
    pub background_color: Option<Color32>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            show_advanced: false,
            show_original_image: true,
            show_color_corrected_image: false,
            show_palettes: true,
            show_debug_info: false,
            show_appearance: false,
            selected_export_format: ExportFormat::default(),
            appearance_mode: AppearanceMode::default(),
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
