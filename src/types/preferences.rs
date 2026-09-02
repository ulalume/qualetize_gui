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
    #[serde(default)]
    pub show_advanced: bool,
    #[serde(default)]
    pub show_palettes: bool,

    #[serde(default)]
    pub show_appearance: bool,
    #[serde(default)]
    pub selected_export_format: ExportFormat,

    #[serde(default)]
    pub appearance_mode: AppearanceMode,

    #[serde(default, with = "color32_def")]
    pub background_color: Option<Color32>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            show_advanced: false,
            show_palettes: true,
            show_appearance: false,
            selected_export_format: ExportFormat::default(),
            appearance_mode: AppearanceMode::default(),
            background_color: None,
        }
    }
}

impl UserPreferences {
    pub fn config_path() -> PathBuf {
        if let Some(config_dir) = crate::settings_manager::config_dir() {
            config_dir.join("QualetizeGUI").join("preferences.json")
        } else {
            PathBuf::from("preferences.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        // A missing file just means first run: not worth a warning, so only the
        // parse failure of an existing file is logged.
        if let Ok(content) = std::fs::read_to_string(&path) {
            match serde_json::from_str(&content) {
                Ok(prefs) => return prefs,
                Err(e) => log::warn!("Failed to parse preferences at {}: {e}", path.display()),
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        let content = serde_json::to_string_pretty(self)?;
        crate::settings_manager::write_atomically(&path, content.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field carries `#[serde(default)]`, so a completely empty preferences
    /// file (e.g. `{}`, or one truncated by a crash) still deserializes instead of
    /// failing to load.
    #[test]
    fn an_empty_preferences_object_still_deserializes() {
        let prefs: UserPreferences = serde_json::from_str("{}").expect("loads");
        assert_eq!(prefs.selected_export_format, ExportFormat::PngIndexed);
        assert_eq!(prefs.background_color, None);
    }

    /// A preferences file written before a field existed is missing just that one
    /// key; it must still load, with the missing field taking its serde default.
    #[test]
    fn a_preferences_object_missing_one_field_still_deserializes() {
        let json = r#"{
            "show_advanced": true,
            "show_appearance": true,
            "selected_export_format": "Bmp",
            "appearance_mode": "Dark"
        }"#;

        let prefs: UserPreferences = serde_json::from_str(json).expect("loads");
        assert!(prefs.show_advanced);
        assert_eq!(prefs.selected_export_format, ExportFormat::Bmp);
        // "show_palettes" was omitted: falls back to bool's serde default.
        assert!(!prefs.show_palettes);
    }
}
