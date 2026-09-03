//! Settings specific to the tilepalquant engine. The target format (tile
//! size, palettes, colors, channel levels, what index 0 of every palette
//! holds) is shared with Qualetize and lives in `QualetizeSettings`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TpqDitherMode {
    Off,
    /// Dither only when assigning final pixel colors.
    #[default]
    Fast,
    /// Evaluate palettes with dithering during optimization as well.
    Slow,
}

impl TpqDitherMode {
    pub fn description(&self) -> &'static str {
        match self {
            TpqDitherMode::Off => "No dithering",
            TpqDitherMode::Fast => "Dither only when choosing the final color of each pixel",
            TpqDitherMode::Slow => {
                "Evaluate palettes with dithering while optimizing them; much slower"
            }
        }
    }

    #[cfg(test)]
    pub fn all() -> &'static [TpqDitherMode] {
        &[TpqDitherMode::Off, TpqDitherMode::Fast, TpqDitherMode::Slow]
    }
}

/// 2x2 ordered dither patterns. The matrix value at `[x & 1][y & 1]` picks
/// which of the error-diffused candidates a pixel takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DitherPattern {
    Diagonal4,
    Horizontal4,
    Vertical4,
    #[default]
    Diagonal2,
    Horizontal2,
    Vertical2,
}

impl DitherPattern {
    pub fn display_name(&self) -> &'static str {
        match self {
            DitherPattern::Diagonal4 => "Diagonal 4",
            DitherPattern::Horizontal4 => "Horizontal 4",
            DitherPattern::Vertical4 => "Vertical 4",
            DitherPattern::Diagonal2 => "Diagonal 2",
            DitherPattern::Horizontal2 => "Horizontal 2",
            DitherPattern::Vertical2 => "Vertical 2",
        }
    }

    #[cfg(test)]
    pub fn all() -> &'static [DitherPattern] {
        &[
            DitherPattern::Diagonal4,
            DitherPattern::Horizontal4,
            DitherPattern::Vertical4,
            DitherPattern::Diagonal2,
            DitherPattern::Horizontal2,
            DitherPattern::Vertical2,
        ]
    }

    /// Candidate index per pixel position, indexed `[x & 1][y & 1]`.
    pub fn matrix(self) -> [[u8; 2]; 2] {
        match self {
            DitherPattern::Diagonal4 => [[0, 2], [3, 1]],
            DitherPattern::Horizontal4 => [[0, 3], [1, 2]],
            DitherPattern::Vertical4 => [[0, 1], [3, 2]],
            DitherPattern::Diagonal2 => [[0, 1], [1, 0]],
            DitherPattern::Horizontal2 => [[0, 1], [0, 1]],
            DitherPattern::Vertical2 => [[0, 0], [1, 1]],
        }
    }

    /// Number of error-diffused candidates the pattern selects from.
    pub fn candidates(self) -> usize {
        match self {
            DitherPattern::Diagonal4 | DitherPattern::Horizontal4 | DitherPattern::Vertical4 => 4,
            DitherPattern::Diagonal2 | DitherPattern::Horizontal2 | DitherPattern::Vertical2 => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TpqSettings {
    /// Iteration budget as a fraction of the pixel count.
    #[serde(default = "default_fraction_of_pixels")]
    pub fraction_of_pixels: f32,
    #[serde(default)]
    pub dither_mode: TpqDitherMode,
    #[serde(default)]
    pub dither_pattern: DitherPattern,
    #[serde(default = "default_dither_weight")]
    pub dither_weight: f32,
    #[serde(default)]
    pub rand_seed: u32,
    /// Pick a fresh seed for every run and write it back to `rand_seed`.
    #[serde(default)]
    pub randomize_seed: bool,
    /// Report intermediate quantizations while running.
    #[serde(default = "default_true")]
    pub show_progress: bool,
}

fn default_fraction_of_pixels() -> f32 {
    0.1
}

fn default_dither_weight() -> f32 {
    0.5
}

fn default_true() -> bool {
    true
}

impl Default for TpqSettings {
    fn default() -> Self {
        Self {
            fraction_of_pixels: default_fraction_of_pixels(),
            dither_mode: TpqDitherMode::default(),
            dither_pattern: DitherPattern::default(),
            dither_weight: default_dither_weight(),
            rand_seed: 0,
            randomize_seed: false,
            show_progress: true,
        }
    }
}

pub const FRACTION_OF_PIXELS_RANGE: std::ops::RangeInclusive<f32> = 0.01..=10.0;
pub const DITHER_WEIGHT_RANGE: std::ops::RangeInclusive<f32> = 0.01..=1.0;

impl TpqSettings {
    /// Restore the preset dithering: fast, Diagonal 2, weight 0.5.
    pub fn reset_dithering(&mut self) {
        self.dither_mode = TpqDitherMode::default();
        self.dither_pattern = DitherPattern::default();
        self.dither_weight = default_dither_weight();
    }

    /// Clamp values loaded from disk into the ranges the UI enforces.
    pub fn sanitize(&mut self) {
        if !self.fraction_of_pixels.is_finite() {
            self.fraction_of_pixels = default_fraction_of_pixels();
        }
        self.fraction_of_pixels = self.fraction_of_pixels.clamp(
            *FRACTION_OF_PIXELS_RANGE.start(),
            *FRACTION_OF_PIXELS_RANGE.end(),
        );
        if !self.dither_weight.is_finite() {
            self.dither_weight = default_dither_weight();
        }
        self.dither_weight = self
            .dither_weight
            .clamp(*DITHER_WEIGHT_RANGE.start(), *DITHER_WEIGHT_RANGE.end());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_object_deserializes_to_the_defaults() {
        let settings: TpqSettings = serde_json::from_str("{}").expect("loads");
        assert_eq!(settings, TpqSettings::default());
    }

    #[test]
    fn resetting_dithering_switches_it_off() {
        let mut settings = TpqSettings {
            dither_mode: TpqDitherMode::Slow,
            ..TpqSettings::default()
        };
        settings.reset_dithering();
        assert_eq!(settings.dither_mode, TpqDitherMode::Fast);
        assert_eq!(settings.dither_pattern, DitherPattern::Diagonal2);
        assert_eq!(settings.dither_weight, 0.5);
    }

    #[test]
    fn sanitize_clamps_out_of_range_values() {
        let mut settings = TpqSettings {
            fraction_of_pixels: f32::NAN,
            dither_weight: 5.0,
            ..TpqSettings::default()
        };
        settings.sanitize();
        assert_eq!(settings.fraction_of_pixels, 0.1);
        assert_eq!(settings.dither_weight, 1.0);
    }

    #[test]
    fn patterns_pick_from_as_many_candidates_as_their_matrix_names() {
        for pattern in DitherPattern::all() {
            let max = pattern.matrix().iter().flatten().copied().max().unwrap() as usize;
            assert_eq!(max + 1, pattern.candidates(), "{pattern:?}");
        }
    }

    /// A `.qset` written before the first color settings moved into
    /// `QualetizeSettings` still carries them here; they have to be ignored
    /// rather than refuse the file.
    #[test]
    fn an_old_settings_file_still_loads_with_the_moved_fields_present() {
        let settings: TpqSettings = serde_json::from_str(
            r#"{"color_zero": "Shared", "shared_color": [1, 2, 3],
                 "transparent_color": [4, 5, 6], "rand_seed": 7}"#,
        )
        .expect("loads");
        assert_eq!(settings.rand_seed, 7);
        assert_eq!(
            TpqSettings {
                rand_seed: 0,
                ..settings
            },
            TpqSettings::default()
        );
    }
}
