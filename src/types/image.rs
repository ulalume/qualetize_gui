use std::fs::File;
use std::io::BufReader;

use super::BGRA8;
use super::ColorCorrection;
use crate::color_processor::ColorProcessor;
use crate::image_processor::QualetizeResult;
use crate::types::color_management::convert_rgba_with_color_profile;
use egui::{Color32, ColorImage, TextureHandle};
use image::DynamicImage;
use image::ImageDecoder;
use image::ImageReader;
use moxcms::ColorProfile;
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct PaletteSortSettings {
    pub mode: SortMode,
    pub order: SortOrder,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Ascending => "Ascending",
            Self::Descending => "Descending",
        }
    }
    pub fn all() -> &'static [Self] {
        &[Self::Ascending, Self::Descending]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum SortMode {
    #[default]
    None,
    Luminance,
    Hue,
    Brightness,
    Saturation,
}

impl SortMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "Default",
            Self::Luminance => "Luminance",
            Self::Hue => "Hue",
            Self::Brightness => "Brightness",
            Self::Saturation => "Saturation",
        }
    }
    pub fn all() -> &'static [Self] {
        &[
            Self::None,
            Self::Luminance,
            Self::Hue,
            Self::Brightness,
            Self::Saturation,
        ]
    }
}

impl ImageDataIndexed {
    pub fn sorted(
        &self,
        mode: SortMode,
        order: SortOrder,
        first_color_is_transparent: bool,
    ) -> Self {
        // Get the number of colors per palette from palettes_for_ui
        if self.palettes_for_ui.is_empty() {
            return self.clone();
        }

        let colors_per_palette = self.palettes_for_ui[0].len();
        let num_palettes = self.palettes_for_ui.len();

        // Create a new copy to work with
        let mut new_palettes_for_ui = self.palettes_for_ui.clone();
        let mut new_palettes = self.palettes.clone();
        let mut new_indexed_pixels = self.indexed_pixels.clone();

        // Process each palette
        for palette_idx in 0..num_palettes.min(new_palettes_for_ui.len()) {
            // Get colors for this palette
            let palette_start = palette_idx * colors_per_palette;
            let palette_end = palette_start + colors_per_palette;

            if palette_end > self.palettes.len() {
                continue;
            }

            // Create index mapping for sorting
            let mut indices: Vec<usize> = (0..colors_per_palette).collect();

            // Sort indices based on color values
            indices.sort_by(|&a, &b| {
                if first_color_is_transparent {
                    if a == 0 {
                        return std::cmp::Ordering::Less;
                    } else if b == 0 {
                        return std::cmp::Ordering::Greater;
                    }
                }
                let color_a = &self.palettes_for_ui[palette_idx][a];
                let color_b = &self.palettes_for_ui[palette_idx][b];

                let sort_key_a = Self::get_sort_key(color_a, &mode);
                let sort_key_b = Self::get_sort_key(color_b, &mode);

                match order {
                    SortOrder::Ascending => sort_key_a
                        .partial_cmp(&sort_key_b)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    SortOrder::Descending => sort_key_b
                        .partial_cmp(&sort_key_a)
                        .unwrap_or(std::cmp::Ordering::Equal),
                }
            });

            // Create reverse mapping (old index -> new index)
            let mut index_mapping = vec![0; colors_per_palette];
            for (new_idx, &old_idx) in indices.iter().enumerate() {
                index_mapping[old_idx] = new_idx;
            }

            // Update palettes_for_ui for this palette
            let mut sorted_ui_palette = vec![egui::Color32::BLACK; colors_per_palette];
            for (new_idx, &old_idx) in indices.iter().enumerate() {
                sorted_ui_palette[new_idx] = self.palettes_for_ui[palette_idx][old_idx];
            }
            new_palettes_for_ui[palette_idx] = sorted_ui_palette;

            // Update palettes for this palette
            let mut sorted_palette = vec![
                BGRA8 {
                    b: 0,
                    g: 0,
                    r: 0,
                    a: 255
                };
                colors_per_palette
            ];
            for (new_idx, &old_idx) in indices.iter().enumerate() {
                sorted_palette[new_idx] = self.palettes[palette_start + old_idx];
            }
            for (i, color) in sorted_palette.iter().enumerate() {
                new_palettes[palette_start + i] = *color;
            }

            // Update indexed_pixels that reference this palette
            for pixel in new_indexed_pixels.iter_mut() {
                let pixel_palette_idx = (*pixel as usize) / colors_per_palette;
                let pixel_color_idx = (*pixel as usize) % colors_per_palette;

                if pixel_palette_idx == palette_idx {
                    let new_color_idx = index_mapping[pixel_color_idx];
                    *pixel = (palette_idx * colors_per_palette + new_color_idx) as u8;
                }
            }
        }

        ImageDataIndexed {
            palettes_for_ui: new_palettes_for_ui,
            palettes: new_palettes,
            indexed_pixels: new_indexed_pixels,
        }
    }

    fn get_sort_key(color: &egui::Color32, mode: &SortMode) -> f32 {
        if mode == &SortMode::None {
            return 0.0;
        }
        let r = color.r() as f32 / 255.0;
        let g = color.g() as f32 / 255.0;
        let b = color.b() as f32 / 255.0;
        let a = color.a() as f32 / 255.0;

        let (h, s, v) = ColorProcessor::rgb_to_hsv(r, g, b);
        let l = ColorProcessor::rgb_f32_to_luminance(r, g, b);

        match mode {
            SortMode::None => 0.0,
            SortMode::Luminance => l * 10000.0 + a + v,
            SortMode::Hue => h * 10000.0 + a + l,
            SortMode::Saturation => s * 10000.0 + a + l,
            SortMode::Brightness => v * 10000.0 + a + l,
        }
    }
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
        display_icc_profile: &Option<Vec<u8>>,
    ) -> ImageData {
        let rgba_img = ColorProcessor::apply_pixels_correction(
            &self.rgba_data,
            self.width,
            self.height,
            color_correction,
        );
        let size = [self.width as usize, self.height as usize];
        let rgba_data = rgba_img.into_raw();

        let color_image = if let Some(display_icc_data) = display_icc_profile
            && let Some(display_profile) = ColorProfile::new_from_slice(display_icc_data).ok()
        {
            let display_data = convert_rgba_with_color_profile(
                &rgba_data,
                self.width as usize,
                &ColorProfile::new_srgb(),
                &display_profile,
            );
            ColorImage::from_rgba_unmultiplied(size, &display_data)
        } else {
            // No display profile, use sRGB data directly for both display and storage
            ColorImage::from_rgba_unmultiplied(size, &rgba_data)
        };
        let texture = ctx.load_texture(
            "color_corrected",
            color_image,
            egui::TextureOptions::NEAREST,
        );

        ImageData {
            texture,
            width: size[0] as u32,
            height: size[1] as u32,
            rgba_data,
            indexed: None,
        }
    }

    pub fn create_from_qualetize_result(
        result: QualetizeResult,
        ctx: &egui::Context,
        display_icc_profile: &Option<Vec<u8>>,
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

        // Calculate display data and create texture
        let color_image = if let Some(display_icc_data) = display_icc_profile
            && let Some(display_profile) = ColorProfile::new_from_slice(display_icc_data).ok()
        {
            let display_data = convert_rgba_with_color_profile(
                &pixels,
                width as usize,
                &ColorProfile::new_srgb(),
                &display_profile,
            );
            ColorImage::from_rgba_unmultiplied(size, &display_data)
        } else {
            // No display profile, use sRGB data directly for both display and storage
            ColorImage::from_rgba_unmultiplied(size, &pixels)
        };
        let texture = ctx.load_texture("output", color_image, egui::TextureOptions::NEAREST);

        // パレット情報を直接変換
        let palettes_for_ui = Self::convert_palette_data(
            &palette_data,
            settings.n_palettes as usize,
            settings.n_colors as usize,
        );

        Ok(ImageData {
            texture,
            width,
            height,
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

    pub fn load(
        path: &str,
        ctx: &egui::Context,
        display_icc_profile: &Option<Vec<u8>>,
    ) -> Result<ImageData, String> {
        let file = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
        let buf_reader = BufReader::new(file);

        let reader = ImageReader::new(buf_reader)
            .with_guessed_format()
            .map_err(|e| format!("Image format guess error: {e}"))?;
        let mut decoder = reader
            .into_decoder()
            .map_err(|e| format!("Decoder creation error: {e}"))?;

        let icc_profile: Option<Vec<u8>> = decoder
            .icc_profile()
            .map_err(|e| format!("Error reading ICC profile: {e}"))?;

        let img = DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("Error decoding image: {e}"))?;

        let rgba_img = img.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let width = img.width() as usize;

        // Determine if we need to apply color profile transformation
        let needs_profile_transform = icc_profile.is_some();

        let rgba_data = rgba_img.into_raw();
        // Calculate rgba_data (sRGB space)
        let rgba_data = if needs_profile_transform {
            convert_rgba_with_color_profile(
                &rgba_data,
                width,
                &ColorProfile::new_from_slice(icc_profile.as_ref().unwrap()).unwrap(),
                &ColorProfile::new_srgb(),
            )
        } else {
            rgba_data
        };

        // Calculate display data and create texture
        let color_image = if let Some(display_icc_data) = display_icc_profile
            && let Some(display_profile) = ColorProfile::new_from_slice(display_icc_data).ok()
        {
            let rgba_data = convert_rgba_with_color_profile(
                &rgba_data,
                width,
                &ColorProfile::new_srgb(),
                &display_profile,
            );
            ColorImage::from_rgba_unmultiplied(size, &rgba_data)
        } else {
            // No display profile, use sRGB data directly for both display and storage
            ColorImage::from_rgba_unmultiplied(size, &rgba_data)
        };
        let texture = ctx.load_texture("input", color_image, egui::TextureOptions::NEAREST);

        Ok(ImageData {
            texture,
            width: size[0] as u32,
            height: size[1] as u32,
            rgba_data,
            indexed: None,
        })
    }
}
