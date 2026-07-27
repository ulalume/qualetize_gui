use super::color_space::ColorSpace;
use super::dither::DitherMode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::ptr;
use std::sync::LazyLock;

/// The C plan stores the per-channel level count in a `u8`, so a channel can hold
/// at most 255 entries.
pub const MAX_CUSTOM_LEVELS: usize = u8::MAX as usize;

#[cfg(target_arch = "x86_64")]
#[repr(C, align(16))]
pub struct Vec4f {
    pub f32: [f32; 4],
}

#[cfg(not(target_arch = "x86_64"))]
#[repr(C)]
pub struct Vec4f {
    pub f32: [f32; 4],
}

#[repr(C)]
pub struct QualetizePlan {
    pub tile_width: u16,
    pub tile_height: u16,
    pub n_palette_colors: u16,
    pub n_tile_palettes: u16,
    pub colorspace: u8,
    pub first_color_is_transparent: u8,
    pub premultiplied_alpha: u8,
    pub dither_type: u8,
    pub dither_level: f32,
    pub n_tile_cluster_passes: u32,
    pub n_color_cluster_passes: u32,
    pub color_depth: Vec4f,
    pub transparent_color: BGRA8,
    pub custom_levels: [*const f32; 4],
    pub custom_level_count: [u8; 4],
}

unsafe extern "C" {
    pub fn Qualetize(
        output_px_data: *mut u8,
        output_palette: *mut BGRA8,
        input_bitmap: *const BGRA8,
        input_palette: *const BGRA8,
        input_width: u32,
        input_height: u32,
        plan: *const QualetizePlan,
        rmse: *mut Vec4f,
    ) -> u8;
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BGRA8 {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClearColor {
    #[default]
    None,
    Rgb(u8, u8, u8),
}

impl ClearColor {
    pub fn to_bgra8(self) -> BGRA8 {
        match self {
            ClearColor::None => BGRA8 {
                b: 0,
                g: 0,
                r: 0,
                a: 0,
            },
            ClearColor::Rgb(r, g, b) => BGRA8 { b, g, r, a: 0xFF },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualetizeSettings {
    pub tile_width: u16,
    pub tile_height: u16,
    pub n_palettes: u16,
    pub n_colors: u16,
    pub rgba_depth: String,
    pub premul_alpha: bool,
    pub color_space: ColorSpace,
    pub dither_mode: DitherMode,
    pub dither_level: f32,
    pub tile_passes: u32,
    pub color_passes: u32,
    pub col0_is_clear: bool,
    pub clear_color: ClearColor,
    #[serde(default)]
    pub tile_reduce_post_enabled: bool,
    #[serde(default = "default_tile_reduce_post_threshold")]
    pub tile_reduce_post_threshold: f32,
    #[serde(default = "default_tile_reduce_allow_flip")]
    pub tile_reduce_allow_flip_x: bool,
    #[serde(default = "default_tile_reduce_allow_flip")]
    pub tile_reduce_allow_flip_y: bool,
    #[serde(default)]
    pub use_custom_levels: bool,
    #[serde(default = "default_custom_level_strings")]
    pub custom_levels: [String; 4],
}

#[derive(Default)]
pub enum QualetizePreset {
    #[default]
    Genesis,
    GenesisFullPals,
    GbaNds,
    GbaNdsFullPals,
}

impl QualetizePreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            QualetizePreset::Genesis => "Genesis",
            QualetizePreset::GenesisFullPals => "Genesis (Full Palettes)",
            QualetizePreset::GbaNds => "GBA/ NDS",
            QualetizePreset::GbaNdsFullPals => "GBA/ NDS (Full palettes)",
        }
    }

    pub fn all() -> &'static [QualetizePreset] {
        &[
            QualetizePreset::Genesis,
            QualetizePreset::GenesisFullPals,
            QualetizePreset::GbaNds,
            QualetizePreset::GbaNdsFullPals,
        ]
    }

    pub fn qualetize_settings(&self) -> QualetizeSettings {
        match self {
            QualetizePreset::Genesis => QualetizeSettings::genesis(),
            QualetizePreset::GenesisFullPals => QualetizeSettings::genesis_full_palettes(),
            QualetizePreset::GbaNds => QualetizeSettings::gba_nds(),
            QualetizePreset::GbaNdsFullPals => QualetizeSettings::gba_nds_full_palettes(),
        }
    }
}

impl QualetizeSettings {
    pub fn gba_nds() -> Self {
        let rgba_depth = "5551".to_string();
        Self {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 1,
            n_colors: 16,
            rgba_depth: rgba_depth.clone(),
            premul_alpha: false,
            color_space: ColorSpace::YcbcrPsy,
            dither_mode: DitherMode::Floyd,
            dither_level: 0.5,
            tile_passes: 1000,
            color_passes: 100,
            col0_is_clear: false,
            clear_color: ClearColor::default(),
            tile_reduce_post_enabled: false,
            tile_reduce_post_threshold: default_tile_reduce_post_threshold(),
            tile_reduce_allow_flip_x: default_tile_reduce_allow_flip(),
            tile_reduce_allow_flip_y: default_tile_reduce_allow_flip(),
            use_custom_levels: false,
            custom_levels: default_level_strings_from_depth(&rgba_depth),
        }
    }
    pub fn gba_nds_full_palettes() -> Self {
        Self {
            n_palettes: 16,
            col0_is_clear: true,
            ..Self::gba_nds()
        }
    }
    pub fn genesis() -> Self {
        let rgba_depth = "3331".to_string();
        Self {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 1,
            n_colors: 16,
            rgba_depth: rgba_depth.clone(),
            premul_alpha: false,
            color_space: ColorSpace::default(),
            dither_mode: DitherMode::default(),
            dither_level: 0.5,
            tile_passes: 1000,
            color_passes: 100,
            col0_is_clear: false,
            clear_color: ClearColor::default(),
            tile_reduce_post_enabled: false,
            tile_reduce_post_threshold: default_tile_reduce_post_threshold(),
            tile_reduce_allow_flip_x: default_tile_reduce_allow_flip(),
            tile_reduce_allow_flip_y: default_tile_reduce_allow_flip(),
            use_custom_levels: true,
            custom_levels: genesis_custom_level_strings(),
        }
    }
    pub fn genesis_full_palettes() -> Self {
        Self {
            n_palettes: 4,
            col0_is_clear: true,
            ..Self::genesis()
        }
    }
}

impl Default for QualetizeSettings {
    fn default() -> Self {
        Self::genesis()
    }
}

fn char_to_depth(c: char) -> f32 {
    match c {
        '1' => 1.0,
        '2' => 3.0,
        '3' => 7.0,
        '4' => 15.0,
        '5' => 31.0,
        '6' => 63.0,
        '7' => 127.0,
        '8' => 255.0,
        _ => 255.0,
    }
}

fn default_tile_reduce_post_threshold() -> f32 {
    25.0
}

fn default_tile_reduce_allow_flip() -> bool {
    true
}

fn parse_rgba_depth(rgba_depth: &str) -> [f32; 4] {
    if rgba_depth.len() == 4 {
        let chars: Vec<char> = rgba_depth.chars().collect();
        [
            char_to_depth(chars[0]),
            char_to_depth(chars[1]),
            char_to_depth(chars[2]),
            char_to_depth(chars[3]),
        ]
    } else {
        [255.0, 255.0, 255.0, 255.0] // Default to 8-bit
    }
}

const DEFAULT_RGBA_DEPTH: &str = "3331";

fn depth_to_levels(depth: f32) -> Vec<u8> {
    let clamped_depth = depth.clamp(1.0, 254.0);
    let steps = clamped_depth.round() as u32;
    (0..=steps)
        .map(|i| ((i as f32 / clamped_depth) * 255.0).round() as u8)
        .collect()
}

fn levels_to_string(levels: Vec<u8>) -> String {
    levels
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join(",")
}

pub fn default_level_strings_from_depth(rgba_depth: &str) -> [String; 4] {
    let depth = parse_rgba_depth(rgba_depth);
    [
        levels_to_string(depth_to_levels(depth[0])),
        levels_to_string(depth_to_levels(depth[1])),
        levels_to_string(depth_to_levels(depth[2])),
        levels_to_string(depth_to_levels(depth[3])),
    ]
}

fn genesis_custom_level_strings() -> [String; 4] {
    [
        "0,49,87,119,146,174,206,255".to_string(),
        "0,49,87,119,146,174,206,255".to_string(),
        "0,49,87,119,146,174,206,255".to_string(),
        "0,255".to_string(),
    ]
}

fn default_custom_level_strings() -> [String; 4] {
    default_level_strings_from_depth(DEFAULT_RGBA_DEPTH)
}

/// Comma separated list of 0-255 integers, without leading zeros or whitespace.
static LEVEL_LIST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9][0-9]|[0-9])(,(25[0-5]|2[0-4][0-9]|1[0-9]{2}|[0-9]|[1-9][0-9]))*$")
        .expect("level list regex is valid")
});

pub fn validate_0_255_array(array_str: &str) -> bool {
    if array_str.is_empty() {
        return false;
    }

    if !LEVEL_LIST_RE.is_match(array_str) {
        return false;
    }

    array_str.split(',').count() <= MAX_CUSTOM_LEVELS
}

fn parse_custom_levels(array_str: &str) -> Option<Vec<f32>> {
    if !validate_0_255_array(array_str) {
        return None;
    }
    let mut values: Vec<f32> = array_str
        .split(',')
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .map(|v| (v as f32) / 255.0)
        .collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(values)
}

pub struct QualetizePlanOwned {
    pub plan: QualetizePlan,
    custom_level_storage: [Option<Box<[f32]>>; 4],
}

impl QualetizePlanOwned {
    pub fn as_ptr(&self) -> *const QualetizePlan {
        let _ = &self.custom_level_storage;
        &self.plan
    }
}

impl From<QualetizeSettings> for QualetizePlanOwned {
    fn from(settings: QualetizeSettings) -> Self {
        let rgba_depth = parse_rgba_depth(&settings.rgba_depth);
        let mut plan = QualetizePlan {
            tile_width: settings.tile_width,
            tile_height: settings.tile_height,
            n_palette_colors: settings.n_colors,
            n_tile_palettes: settings.n_palettes,
            colorspace: settings.color_space.to_id(),
            first_color_is_transparent: if settings.col0_is_clear { 1 } else { 0 },
            premultiplied_alpha: if settings.premul_alpha { 1 } else { 0 },
            dither_type: settings.dither_mode.to_id(),
            dither_level: settings.dither_level,
            n_tile_cluster_passes: settings.tile_passes,
            n_color_cluster_passes: settings.color_passes,
            color_depth: Vec4f {
                f32: [rgba_depth[0], rgba_depth[1], rgba_depth[2], rgba_depth[3]],
            },
            transparent_color: settings.clear_color.to_bgra8(),
            custom_levels: [ptr::null(); 4],
            custom_level_count: [0; 4],
        };

        let mut custom_level_storage: [Option<Box<[f32]>>; 4] = [None, None, None, None];
        if settings.use_custom_levels {
            for (idx, level_str) in settings.custom_levels.iter().enumerate() {
                if let Some(levels) = parse_custom_levels(level_str)
                    && let Ok(len) = u8::try_from(levels.len())
                {
                    let boxed = levels.into_boxed_slice();
                    plan.custom_levels[idx] = boxed.as_ptr();
                    plan.custom_level_count[idx] = len;
                    custom_level_storage[idx] = Some(boxed);
                }
            }
        }

        Self {
            plan,
            custom_level_storage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_0_255_array_accepts_well_formed_lists() {
        assert!(validate_0_255_array("0"));
        assert!(validate_0_255_array("0,255"));
        assert!(validate_0_255_array("0,49,87,119,146,174,206,255"));
    }

    #[test]
    fn validate_0_255_array_rejects_malformed_lists() {
        assert!(!validate_0_255_array(""), "empty");
        assert!(!validate_0_255_array("256"), "out of range");
        assert!(!validate_0_255_array("0, 255"), "whitespace");
        assert!(!validate_0_255_array("0,"), "trailing comma");
        assert!(!validate_0_255_array("00"), "leading zero");
        assert!(!validate_0_255_array("0;255"), "wrong separator");
    }

    #[test]
    fn validate_0_255_array_rejects_more_entries_than_the_plan_can_hold() {
        let at_limit = vec!["1"; MAX_CUSTOM_LEVELS].join(",");
        let over_limit = vec!["1"; MAX_CUSTOM_LEVELS + 1].join(",");
        assert!(validate_0_255_array(&at_limit));
        assert!(!validate_0_255_array(&over_limit));
    }

    #[test]
    fn parse_custom_levels_normalizes_and_sorts() {
        let levels = parse_custom_levels("255,0,128").expect("valid list");
        assert_eq!(levels.len(), 3);
        assert!(levels[0] < levels[1] && levels[1] < levels[2]);
        assert_eq!(levels[0], 0.0);
        assert_eq!(levels[2], 1.0);
    }

    #[test]
    fn parse_custom_levels_rejects_invalid_input() {
        assert!(parse_custom_levels("0,256").is_none());
        assert!(parse_custom_levels("").is_none());
    }

    /// A full 8-bit channel needs 256 steps but the plan stores the count in a `u8`,
    /// so `depth_to_levels` clamps to 254 steps (255 entries) and stays valid.
    #[test]
    fn generated_levels_always_fit_the_plan() {
        for depth in ["3331", "5551", "8888"] {
            for (channel, levels) in default_level_strings_from_depth(depth).iter().enumerate() {
                let count = levels.split(',').count();
                assert!(
                    count <= MAX_CUSTOM_LEVELS,
                    "{depth} channel {channel} produced {count} levels"
                );
                assert!(
                    validate_0_255_array(levels),
                    "{depth} channel {channel} is not accepted by its own validator"
                );
                assert!(u8::try_from(count).is_ok());
            }
        }
    }

    #[test]
    fn genesis_preset_uses_the_documented_levels() {
        let settings = QualetizeSettings::genesis();
        assert!(settings.use_custom_levels);
        assert_eq!(settings.custom_levels[0], "0,49,87,119,146,174,206,255");
        assert_eq!(settings.custom_levels[3], "0,255");
    }

    #[test]
    fn full_palette_presets_only_change_palette_layout() {
        let base = QualetizeSettings::genesis();
        let full = QualetizeSettings::genesis_full_palettes();
        assert_eq!(full.n_palettes, 4);
        assert!(full.col0_is_clear);
        assert_eq!(full.rgba_depth, base.rgba_depth);
        assert_eq!(full.custom_levels, base.custom_levels);
    }
}
