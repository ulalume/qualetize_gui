use crate::engine::QuantEngine;
use crate::platform::storage;
use crate::types::{
    QualetizeSettings, color_correction::ColorCorrection, image::PaletteSortSettings,
    tilepalquant::TpqSettings,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Key the settings in use are kept under so they survive a restart.
const SESSION_KEY: &str = "session";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingsBundle {
    pub qualetize_settings: QualetizeSettings,
    pub color_correction: ColorCorrection,
    #[serde(default)]
    pub sort_settings: PaletteSortSettings,
    #[serde(default)]
    pub engine: QuantEngine,
    #[serde(default)]
    pub tpq_settings: TpqSettings,
    #[serde(default)]
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
            engine: QuantEngine::default(),
            tpq_settings: TpqSettings::default(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Compare the settings only; `version` is metadata about the writer.
    pub fn matches(&self, other: &Self) -> bool {
        self.qualetize_settings == other.qualetize_settings
            && self.color_correction == other.color_correction
            && self.sort_settings == other.sort_settings
            && self.engine == other.engine
            && self.tpq_settings == other.tpq_settings
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize settings: {e}"))
    }

    /// Read a bundle from the JSON of a `.qset` file or of a stored session.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut settings = serde_json::from_str::<SettingsBundle>(json)
            .map_err(|e| format!("Failed to parse settings file: {e}"))?;

        // A hand-edited or older-version `.qset` may carry out-of-range values;
        // clamp them before they can reach the C library. See
        // `QualetizeSettings::sanitize`.
        settings.qualetize_settings.sanitize();
        settings.tpq_settings.sanitize();

        Ok(settings)
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let json_data = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read settings file: {e}"))?;
        let settings = Self::from_json(&json_data)?;

        log::info!("Settings loaded from: {}", path.as_ref().display());
        Ok(settings)
    }

    pub fn get_settings_file_extension() -> &'static str {
        "qset"
    }

    /// Restore the settings from the last run, falling back to the defaults.
    pub fn load_session() -> Self {
        let bundle = storage::load(SESSION_KEY).and_then(|json| match Self::from_json(&json) {
            Ok(bundle) => Some(bundle),
            Err(e) => {
                log::warn!("Failed to load session settings: {e}");
                None
            }
        });

        bundle.unwrap_or_else(|| {
            Self::new(
                QualetizeSettings::default(),
                ColorCorrection::default(),
                PaletteSortSettings::default(),
            )
        })
    }

    /// Mirror the settings in use so they survive a restart. Same format as a
    /// hand-saved `.qset`, so the stored value can be inspected or reused.
    pub fn save_session(&self) -> Result<(), String> {
        storage::save(SESSION_KEY, &self.to_json()?)
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
        // `enabled` is absent from this JSON, so it falls back to its serde default of `true`.
        assert!(bundle.color_correction.enabled);
        assert_eq!(bundle.sort_settings, PaletteSortSettings::default());
    }

    /// `version` is written but never read back, so a file saved before it existed
    /// (or hand-trimmed) must still load rather than fail to parse.
    #[test]
    fn a_bundle_missing_the_version_field_still_loads() {
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
            }
        }"#;

        let bundle: SettingsBundle = serde_json::from_str(json).expect("loads");
        assert_eq!(bundle.version, "");
    }

    /// `examples/genesis.qset` ships as a ready-to-use example: it must still load,
    /// and its `color_correction.enabled` must be explicit (`false`) rather than
    /// relying on the "missing means true" backwards-compatibility default, which
    /// would otherwise silently turn color correction on for this example.
    #[test]
    fn the_genesis_example_loads_disabled_and_matches_the_preset() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/genesis.qset");
        let bundle = SettingsBundle::load_from_file(&path).expect("example loads");

        assert!(!bundle.color_correction.enabled);

        let preset = QualetizeSettings::genesis();
        assert_eq!(
            bundle.qualetize_settings, preset,
            "loaded settings (left) do not match QualetizeSettings::genesis() (right):\n\
             loaded:  {:#?}\n\
             genesis: {:#?}",
            bundle.qualetize_settings, preset
        );
    }

    /// `from_json` is the one door settings come in through, so the clamping
    /// of out-of-range values has to happen there rather than in the callers.
    #[test]
    fn from_json_clamps_out_of_range_values() {
        let json = r#"{
            "qualetize_settings": {
                "tile_width": 0, "tile_height": 1000, "n_palettes": 65535, "n_colors": 0,
                "rgba_depth": "9a3!", "premul_alpha": false, "color_space": "RgbLinear",
                "dither_mode": "Floyd", "dither_level": 99.0, "tile_passes": 4000000,
                "color_passes": 4000000, "col0_is_clear": false, "clear_color": "None"
            },
            "color_correction": {
                "brightness": 0.0, "contrast": 1.0, "gamma": 1.0, "saturation": 1.0,
                "hue_shift": 0.0, "shadows": 0.0, "highlights": 0.0
            }
        }"#;

        let bundle = SettingsBundle::from_json(json).expect("loads");
        assert_eq!(bundle.qualetize_settings.tile_width, 1);
        assert_eq!(bundle.qualetize_settings.tile_height, 64);
        assert_eq!(bundle.qualetize_settings.n_colors, 1);
        assert_eq!(bundle.qualetize_settings.tile_passes, 1000);
        assert_eq!(bundle.qualetize_settings.color_passes, 100);
        assert_eq!(bundle.qualetize_settings.dither_level, 2.0);
    }

    #[test]
    fn from_json_rejects_text_that_is_not_a_bundle() {
        assert!(SettingsBundle::from_json("not json").is_err());
        assert!(SettingsBundle::from_json("{}").is_err());
    }

    /// The session and a `.qset` file carry the same JSON, so a bundle written
    /// by `to_json` has to come back through `from_json` unchanged.
    #[test]
    fn to_json_and_from_json_round_trip() {
        let mut bundle = SettingsBundle::new(
            QualetizeSettings::genesis(),
            ColorCorrection::default(),
            PaletteSortSettings::default(),
        );
        bundle.qualetize_settings.tile_width = 16;

        let restored = SettingsBundle::from_json(&bundle.to_json().unwrap()).expect("loads");
        assert!(bundle.matches(&restored));
        assert_eq!(restored.version, bundle.version);
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
