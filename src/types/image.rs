use egui::{TextureHandle, Vec2};

#[derive(Clone)]
pub struct ImageData {
    pub texture: Option<TextureHandle>,
    pub size: Vec2,
    pub palettes: Vec<Vec<egui::Color32>>, // パレット情報を追加
}

impl Default for ImageData {
    fn default() -> Self {
        Self {
            texture: None,
            size: Vec2::ZERO,
            palettes: Vec::new(),
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
