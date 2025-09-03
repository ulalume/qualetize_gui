use crate::types::{
    QualetizeSettings, color_correction::ColorCorrection, image::PaletteSortSettings,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingsBundle {
    pub qualetize_settings: QualetizeSettings,
    pub color_correction: ColorCorrection,
    #[serde(default)]
    pub sort_settings: PaletteSortSettings,
    pub version: String,
}

impl SettingsBundle {
    pub fn new(
        qualetize_settings: QualetizeSettings,
        color_correction: ColorCorrection,
        sort_settings: PaletteSortSettings,
    ) -> Self {
        Self {
            qualetize_settings,
            color_correction,
            sort_settings,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let json_data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        fs::write(&path, json_data).map_err(|e| format!("Failed to write settings file: {}", e))?;

        log::info!("Settings saved to: {}", path.as_ref().display());
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let json_data = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read settings file: {}", e))?;

        let settings = serde_json::from_str::<SettingsBundle>(&json_data)
            .map_err(|e| format!("Failed to parse settings file: {}", e))?;

        log::info!("Settings loaded from: {}", path.as_ref().display());
        Ok(settings)
    }

    pub fn get_default_settings_dir() -> Result<std::path::PathBuf, String> {
        if let Some(config_dir) = dirs::config_dir() {
            let app_config_dir = config_dir.join("QualetizeGUI");
            if !app_config_dir.exists() {
                fs::create_dir_all(&app_config_dir)
                    .map_err(|e| format!("Failed to create config directory: {}", e))?;
            }
            Ok(app_config_dir)
        } else {
            Err("Could not determine config directory".to_string())
        }
    }

    pub fn get_settings_file_extension() -> &'static str {
        "qset"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_serialization() {
        let settings = SettingsBundle::new(
            QualetizeSettings::default(),
            ColorCorrection::default(),
            PaletteSortSettings::default(),
        );

        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: SettingsBundle = serde_json::from_str(&json).unwrap();

        assert_eq!(settings.sort_settings.mode, deserialized.sort_settings.mode);
        assert_eq!(
            settings.qualetize_settings.tile_width,
            deserialized.qualetize_settings.tile_width
        );
        assert_eq!(
            settings.color_correction.brightness,
            deserialized.color_correction.brightness
        );
    }
}
