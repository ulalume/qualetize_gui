use super::BGRA8;
use crate::color_processor::ColorProcessor;
use crate::image_processor::QualetizeResult;
use egui::{Color32, ColorImage, TextureHandle};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct ImageData {
    pub texture: TextureHandle,
    pub width: u32,
    pub height: u32,
    pub rgba_data: Vec<u8>,
    // indexed data
    pub indexed: Option<ImageDataIndexed>,
}

#[derive(Clone)]
pub struct ImageDataIndexed {
    pub palettes_for_ui: Vec<Vec<egui::Color32>>,
    pub palettes: Vec<BGRA8>,
    pub indexed_pixels: Vec<u8>,
}

impl ImageData {
    /// Get the color of the top-left pixel (0, 0)
    pub fn get_top_left_pixel_color(&self) -> Option<Color32> {
        if self.rgba_data.len() >= 4 && self.width > 0 && self.height > 0 {
            let r = self.rgba_data[0];
            let g = self.rgba_data[1];
            let b = self.rgba_data[2];
            Some(Color32::from_rgb(r, g, b))
        } else {
            None
        }
    }

    pub fn color_corrected(
        &self,
        color_correction: &ColorCorrection,
        ctx: &egui::Context,
    ) -> ImageData {
        let rgba_img = ColorProcessor::apply_pixels_correction(
            &self.rgba_data,
            self.width,
            self.height,
            color_correction,
        );
        let size = [self.width as usize, self.height as usize];
        let rgba_data = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba_data);
        let texture = ctx.load_texture(
            "color_corrected",
            color_image,
            egui::TextureOptions::NEAREST,
        );

        ImageData {
            texture: texture,
            width: size[0] as u32,
            height: size[1] as u32,
            rgba_data,
            indexed: None,
        }
    }

    pub fn create_from_qualetize_result(
        result: QualetizeResult,
        ctx: &egui::Context,
    ) -> Result<ImageData, String> {
        let QualetizeResult {
            indexed_data,
            palette_data,
            settings,
            width,
            height,
            generation_id: _,
        } = result;

        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for &pixel_index in &indexed_data {
            let palette_index = pixel_index as usize;
            if palette_index < palette_data.len() {
                let color = &palette_data[palette_index];
                pixels.extend_from_slice(&[color.r, color.g, color.b, color.a]);
            } else {
                pixels.extend_from_slice(&[0, 0, 0, 255]);
            }
        }

        let size = [width as usize, height as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ctx.load_texture("output", color_image, egui::TextureOptions::NEAREST);

        // パレット情報を直接変換
        let palettes_for_ui = Self::convert_palette_data(
            &palette_data,
            settings.n_palettes as usize,
            settings.n_colors as usize,
        );

        Ok(ImageData {
            texture: texture,
            width: width,
            height: height,
            rgba_data: pixels,
            indexed: Some(ImageDataIndexed {
                palettes_for_ui,
                palettes: palette_data,
                indexed_pixels: indexed_data,
            }),
        })
    }
    fn convert_palette_data(
        palette_data: &[BGRA8],
        n_palettes: usize,
        n_colors: usize,
    ) -> Vec<Vec<egui::Color32>> {
        let colors_per_palette = n_colors;
        let mut palettes = Vec::new();

        let egui_colors: Vec<egui::Color32> = palette_data
            .iter()
            .map(|bgra| egui::Color32::from_rgba_unmultiplied(bgra.r, bgra.g, bgra.b, bgra.a))
            .collect();

        for chunk in egui_colors.chunks(colors_per_palette) {
            palettes.push(chunk.to_vec());
        }

        while palettes.len() < n_palettes {
            palettes.push(vec![egui::Color32::BLACK; colors_per_palette]);
        }
        palettes.truncate(n_palettes);

        palettes
    }

    pub fn load(path: &str, ctx: &egui::Context) -> Result<ImageData, String> {
        let img = image::open(path).map_err(|e| format!("Image loading error: {}", e))?;
        let rgba_img = img.to_rgba8();
        let size = [rgba_img.width() as usize, rgba_img.height() as usize];
        let rgba_data = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba_data);
        let texture = ctx.load_texture("input", color_image, egui::TextureOptions::NEAREST);

        Ok(ImageData {
            texture: texture,
            width: size[0] as u32,
            height: size[1] as u32,
            rgba_data,
            indexed: None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
