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

    /// Compare the settings only; `version` is metadata about the writer.
    pub fn matches(&self, other: &Self) -> bool {
        self.qualetize_settings == other.qualetize_settings
            && self.color_correction == other.color_correction
            && self.sort_settings == other.sort_settings
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let json_data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;

        fs::write(&path, json_data).map_err(|e| format!("Failed to write settings file: {e}"))?;

        log::info!("Settings saved to: {}", path.as_ref().display());
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let json_data =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read settings file: {e}"))?;

        let settings = serde_json::from_str::<SettingsBundle>(&json_data)
            .map_err(|e| format!("Failed to parse settings file: {e}"))?;

        log::info!("Settings loaded from: {}", path.as_ref().display());
        Ok(settings)
    }

    pub fn get_default_settings_dir() -> Result<std::path::PathBuf, String> {
        if let Some(config_dir) = dirs::config_dir() {
            let app_config_dir = config_dir.join("QualetizeGUI");
            if !app_config_dir.exists() {
                fs::create_dir_all(&app_config_dir)
                    .map_err(|e| format!("Failed to create config directory: {e}"))?;
            }
            Ok(app_config_dir)
        } else {
            Err("Could not determine config directory".to_string())
        }
    }

    pub fn get_settings_file_extension() -> &'static str {
        "qset"
    }

    /// Where the settings in use are mirrored so they survive a restart.
    /// Same format as a hand-saved `.qset`, so it can be inspected or reused.
    pub fn session_path() -> Option<std::path::PathBuf> {
        Some(
            dirs::config_dir()?
                .join("QualetizeGUI")
                .join("session.qset"),
        )
    }

    /// Restore the settings from the last run, falling back to the defaults.
    pub fn load_session() -> Self {
        let bundle = Self::session_path().and_then(|path| Self::load_from_file(path).ok());

        bundle.unwrap_or_else(|| {
            Self::new(
                QualetizeSettings::default(),
                ColorCorrection::default(),
                PaletteSortSettings::default(),
            )
        })
    }

    pub fn save_session(&self) -> Result<(), String> {
        let path = Self::session_path().ok_or("Could not determine config directory")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {e}"))?;
        }
        self.save_to_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ignores_the_writer_version() {
        let a = SettingsBundle::new(
            QualetizeSettings::default(),
            ColorCorrection::default(),
            PaletteSortSettings::default(),
        );
        let mut b = a.clone();
        b.version = "0.0.0-other".to_string();
        assert!(a.matches(&b));

        b.qualetize_settings.tile_width = 16;
        assert!(!a.matches(&b));
    }

    #[test]
    fn matches_notices_a_color_correction_change() {
        let a = SettingsBundle::new(
            QualetizeSettings::default(),
            ColorCorrection::default(),
            PaletteSortSettings::default(),
        );
        let mut b = a.clone();
        b.color_correction.enabled = !b.color_correction.enabled;
        assert!(!a.matches(&b));
    }

    /// A session file written before a field existed still has to load.
    #[test]
    fn a_session_missing_newer_fields_still_loads() {
        let json = r#"{
            "qualetize_settings": {
                "tile_width": 8, "tile_height": 8, "n_palettes": 1, "n_colors": 16,
                "rgba_depth": "3331", "premul_alpha": false, "color_space": "RgbLinear",
                "dither_mode": "Floyd", "dither_level": 0.5, "tile_passes": 1000,
                "color_passes": 100, "col0_is_clear": false, "clear_color": "None"
            },
            "color_correction": {
                "brightness": 0.0, "contrast": 1.0, "gamma": 1.0, "saturation": 1.0,
                "hue_shift": 0.0, "shadows": 0.0, "highlights": 0.0
            },
            "version": "0.4.0"
        }"#;

        let bundle: SettingsBundle = serde_json::from_str(json).expect("loads");
        assert_eq!(bundle.qualetize_settings.tile_width, 8);
        // Predates the flag, and back then correction was always applied.
        assert!(bundle.color_correction.enabled);
        assert_eq!(bundle.sort_settings, PaletteSortSettings::default());
    }

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
