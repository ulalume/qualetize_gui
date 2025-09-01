use std::path::PathBuf;

use super::export::ExportFormat;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserPreferences {
    pub show_advanced: bool,
    pub show_original_image: bool,
    pub show_color_corrected_image: Option<bool>,
    pub show_palettes: bool,
    pub selected_export_format: ExportFormat,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            show_advanced: false,
            show_original_image: true,
            show_color_corrected_image: Some(false),
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
