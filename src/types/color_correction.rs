use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorCorrection {
    /// When off the input image is passed through untouched, the settings are
    /// hidden and no "Color corrected" view is shown.
    ///
    /// A settings file missing this field carries no on/off intent, so it
    /// loads as enabled. A fresh [`ColorCorrection::default()`] on the other
    /// hand starts disabled.
    #[serde(default = "enabled_when_missing")]
    pub enabled: bool,
    pub brightness: f32, // -1.0 to 1.0
    pub contrast: f32,   // 0.0 to 2.0
    pub gamma: f32,      // 0.1 to 3.0
    pub saturation: f32, // 0.0 to 2.0
    pub hue_shift: f32,  // -180.0 to 180.0 degrees
    pub shadows: f32,    // -1.0 to 1.0
    pub highlights: f32, // -1.0 to 1.0
}

fn enabled_when_missing() -> bool {
    true
}

pub enum ColorCorrectionPreset {
    None,
    Vibrant,
    Warm,
    Cool,
    Dark,
}

impl ColorCorrectionPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            ColorCorrectionPreset::None => "None",
            ColorCorrectionPreset::Vibrant => "Vibrant",
            ColorCorrectionPreset::Warm => "Warm",
            ColorCorrectionPreset::Cool => "Cool",
            ColorCorrectionPreset::Dark => "Dark",
        }
    }

    pub fn all() -> &'static [ColorCorrectionPreset] {
        &[
            ColorCorrectionPreset::None,
            ColorCorrectionPreset::Vibrant,
            ColorCorrectionPreset::Warm,
            ColorCorrectionPreset::Cool,
            ColorCorrectionPreset::Dark,
        ]
    }

    pub fn color_correction(&self) -> ColorCorrection {
        match self {
            ColorCorrectionPreset::None => ColorCorrection::default(),
            ColorCorrectionPreset::Vibrant => ColorCorrection::preset_vibrant(),
            ColorCorrectionPreset::Warm => ColorCorrection::preset_retro_warm(),
            ColorCorrectionPreset::Cool => ColorCorrection::preset_retro_cool(),
            ColorCorrectionPreset::Dark => ColorCorrection::preset_dark(),
        }
    }
}

impl Default for ColorCorrection {
    fn default() -> Self {
        Self {
            enabled: false,
            brightness: 0.0,
            contrast: 1.0,
            gamma: 1.0,
            saturation: 1.0,
            hue_shift: 0.0,
            shadows: 0.0,
            highlights: 0.0,
        }
    }
}

impl ColorCorrection {
    /// Replace the correction values with `preset`, leaving [`Self::enabled`]
    /// alone: picking a preset should not switch the whole section off.
    pub fn apply_preset(&mut self, preset: ColorCorrection) {
        *self = ColorCorrection {
            enabled: self.enabled,
            ..preset
        };
    }

    pub fn preset_dark() -> ColorCorrection {
        ColorCorrection {
            contrast: 1.75,
            gamma: 0.28,
            saturation: 0.30,
            hue_shift: 100.0,
            ..ColorCorrection::default()
        }
    }

    pub fn preset_vibrant() -> ColorCorrection {
        ColorCorrection {
            saturation: 1.3,
            contrast: 1.1,
            ..ColorCorrection::default()
        }
    }

    pub fn preset_retro_warm() -> ColorCorrection {
        ColorCorrection {
            hue_shift: 10.0,
            saturation: 1.2,
            brightness: 0.05,
            highlights: -0.1,
            ..ColorCorrection::default()
        }
    }

    pub fn preset_retro_cool() -> ColorCorrection {
        ColorCorrection {
            hue_shift: -15.0,
            saturation: 0.9,
            shadows: 0.1,
            highlights: -0.05,
            ..ColorCorrection::default()
        }
    }
}
