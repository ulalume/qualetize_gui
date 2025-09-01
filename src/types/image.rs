use egui::{Color32, TextureHandle, Vec2};

#[derive(Clone)]
pub struct ImageData {
    pub texture: Option<TextureHandle>,
    pub size: Vec2,
    pub palettes: Vec<Vec<egui::Color32>>,
    pub pixels: Vec<u8>,
}

impl Default for ImageData {
    fn default() -> Self {
        Self {
            texture: None,
            size: Vec2::ZERO,
            palettes: Vec::new(),
            pixels: Vec::new(),
        }
    }
}

impl ImageData {
    /// Get the color of the top-left pixel (0, 0)
    pub fn get_top_left_pixel_color(&self) -> Option<Color32> {
        if self.pixels.len() >= 4 && self.size.x > 0.0 && self.size.y > 0.0 {
            let r = self.pixels[0];
            let g = self.pixels[1];
            let b = self.pixels[2];
            let _a = self.pixels[3]; // Alpha not used for RGB color
            Some(Color32::from_rgb(r, g, b))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorCorrection {
    pub brightness: f32, // -1.0 to 1.0
    pub contrast: f32,   // 0.0 to 2.0
    pub gamma: f32,      // 0.1 to 3.0
    pub saturation: f32, // 0.0 to 2.0
    pub hue_shift: f32,  // -180.0 to 180.0 degrees
    pub shadows: f32,    // -1.0 to 1.0
    pub highlights: f32, // -1.0 to 1.0
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
