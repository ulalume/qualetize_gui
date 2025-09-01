use super::color_space::ColorSpace;
use super::dither::DitherMode;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BGRA8 {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClearColor {
    None,
    RGB(u8, u8, u8),
}

impl Default for ClearColor {
    fn default() -> Self {
        ClearColor::None
    }
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
            ClearColor::RGB(r, g, b) => BGRA8 {
                b: *b,
                g: *g,
                r: *r,
                a: 0xFF,
            },
        }
    }

    // pub fn to_string(&self) -> String {
    //     match self {
    //         ClearColor::None => "none".to_string(),
    //         ClearColor::RGB(r, g, b) => format!("#{:02X}{:02X}{:02X}", r, g, b),
    //     }
    // }

    // pub fn from_string(s: &str) -> Self {
    //     if s.trim().to_lowercase() == "none" {
    //         return ClearColor::None;
    //     }

    //     if let Some(hex) = s.strip_prefix('#') {
    //         if hex.len() == 6 {
    //             if let (Ok(r), Ok(g), Ok(b)) = (
    //                 u8::from_str_radix(&hex[0..2], 16),
    //                 u8::from_str_radix(&hex[2..4], 16),
    //                 u8::from_str_radix(&hex[4..6], 16),
    //             ) {
    //                 return ClearColor::RGB(r, g, b);
    //             }
    //         }
    //     }

    //     ClearColor::None
    // }
}

#[derive(Clone, Debug)]
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

pub enum QualetizePreset {
    Genesis,
    GenesisFullPals,
    GbaNds,
    GbaNdsFullPals,
}
impl Default for QualetizePreset {
    fn default() -> Self {
        QualetizePreset::Genesis
    }
}

impl QualetizeSettings {
    pub fn preset(mode: QualetizePreset) -> Self {
        match mode {
            QualetizePreset::Genesis => Self::genesis(),
            QualetizePreset::GenesisFullPals => Self::genesis_full_palettes(),
            QualetizePreset::GbaNds => Self::gba_nds(),
            QualetizePreset::GbaNdsFullPals => Self::gba_nds_full_palettes(),
        }
    }
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
