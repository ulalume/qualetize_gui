use super::color_space::ColorSpace;
use super::dither::DitherMode;

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
    pub clear_color: String,
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
            clear_color: "none".to_string(),
        }
    }
}
