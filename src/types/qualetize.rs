use super::color_space::ColorSpace;
use super::dither::DitherMode;
use serde::{Deserialize, Serialize};

#[repr(C, align(16))]
pub struct Vec4f {
    pub f32: [f32; 4],
}

#[repr(C, align(8))]
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
    pub split_ratio: f32,
    pub n_tile_cluster_passes: u32,
    pub n_color_cluster_passes: u32,
    pub color_depth: Vec4f,
    pub transparent_color: BGRA8,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum ClearColor {
    #[default]
    None,
    Rgb(u8, u8, u8),
}

impl ClearColor {
    pub fn to_bgra8(&self) -> BGRA8 {
        match self {
            ClearColor::None => BGRA8 {
                b: 0,
                g: 0,
                r: 0,
                a: 0,
            },
            ClearColor::Rgb(r, g, b) => BGRA8 {
                b: *b,
                g: *g,
                r: *r,
                a: 0xFF,
            },
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
    pub split_ratio: f32,
    pub col0_is_clear: bool,
    pub clear_color: ClearColor,
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
        Self {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 1,
            n_colors: 16,
            rgba_depth: "5551".to_string(),
            premul_alpha: false,
            color_space: ColorSpace::YcbcrPsy,
            dither_mode: DitherMode::Floyd,
            dither_level: 0.5,
            tile_passes: 1000,
            color_passes: 100,
            split_ratio: -1.0,
            col0_is_clear: false,
            clear_color: ClearColor::default(),
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
        Self {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 1,
            n_colors: 16,
            rgba_depth: "3331".to_string(),
            premul_alpha: false,
            color_space: ColorSpace::default(),
            dither_mode: DitherMode::default(),
            dither_level: 0.5,
            tile_passes: 1000,
            color_passes: 100,
            split_ratio: -1.0,
            col0_is_clear: false,
            clear_color: ClearColor::default(),
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
        Self {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 1,
            n_colors: 16,
            rgba_depth: "3331".to_string(),
            premul_alpha: false,
            color_space: ColorSpace::default(),
            dither_mode: DitherMode::default(),
            dither_level: 0.5,
            tile_passes: 1000,
            color_passes: 100,
            split_ratio: -1.0,
            col0_is_clear: false,
            clear_color: ClearColor::default(),
        }
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

impl From<QualetizeSettings> for QualetizePlan {
    fn from(settings: QualetizeSettings) -> Self {
        let rgba_depth = parse_rgba_depth(&settings.rgba_depth);

        QualetizePlan {
            tile_width: settings.tile_width,
            tile_height: settings.tile_height,
            n_palette_colors: settings.n_colors,
            n_tile_palettes: settings.n_palettes,
            colorspace: settings.color_space.to_id(),
            first_color_is_transparent: if settings.col0_is_clear { 1 } else { 0 },
            premultiplied_alpha: if settings.premul_alpha { 1 } else { 0 },
            dither_type: settings.dither_mode.to_id(),
            dither_level: settings.dither_level,
            split_ratio: settings.split_ratio,
            n_tile_cluster_passes: settings.tile_passes,
            n_color_cluster_passes: settings.color_passes,
            color_depth: Vec4f {
                f32: [rgba_depth[0], rgba_depth[1], rgba_depth[2], rgba_depth[3]],
            },
            transparent_color: settings.clear_color.to_bgra8(),
        }
    }
}
