use super::BGRA8;
use super::ColorCorrection;
use crate::color_processor::ColorProcessor;
use crate::image_processor::QualetizeResult;
use egui::{Color32, ColorImage, TextureHandle};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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

#[derive(Clone, Copy)]
pub struct TileCountOptions {
    pub visible_only: bool,
    pub allow_flip_x: bool,
    pub allow_flip_y: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PaletteSortSettings {
    pub mode: SortMode,
    pub order: SortOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    /// Reorder the colors inside every palette, rewriting the pixel indices so
    /// each pixel still resolves to the same color.
    pub fn sorted(
        &self,
        mode: SortMode,
        order: SortOrder,
        first_color_is_transparent: bool,
    ) -> Self {
        let Some(first_palette) = self.palettes_for_ui.first() else {
            return self.clone();
        };
        let colors_per_palette = first_palette.len();
        if colors_per_palette == 0 {
            return self.clone();
        }

        let mut new_palettes_for_ui = self.palettes_for_ui.clone();
        let mut new_palettes = self.palettes.clone();

        // One global old-index -> new-index table for every palette, so the
        // pixel buffer is rewritten in a single pass instead of once per
        // palette. Entries not covered by a complete palette stay identity.
        let mut remap: [u8; 256] = std::array::from_fn(|i| i as u8);

        for (palette_idx, palette) in self.palettes_for_ui.iter().enumerate() {
            let palette_start = palette_idx * colors_per_palette;
            if palette.len() != colors_per_palette
                || palette_start + colors_per_palette > self.palettes.len()
            {
                continue;
            }

            let mut order_of: Vec<usize> = (0..colors_per_palette).collect();
            order_of.sort_by(|&a, &b| {
                Self::compare_colors(palette, a, b, mode, order, first_color_is_transparent)
            });

            for (new_idx, &old_idx) in order_of.iter().enumerate() {
                new_palettes_for_ui[palette_idx][new_idx] = palette[old_idx];
                new_palettes[palette_start + new_idx] = self.palettes[palette_start + old_idx];
                if let Some(slot) = remap.get_mut(palette_start + old_idx) {
                    *slot = (palette_start + new_idx) as u8;
                }
            }
        }

        ImageDataIndexed {
            palettes_for_ui: new_palettes_for_ui,
            palettes: new_palettes,
            indexed_pixels: self
                .indexed_pixels
                .iter()
                .map(|&pixel| remap[pixel as usize])
                .collect(),
        }
    }

    /// Ordering of two entries of the same palette. When the first color is the
    /// transparent one it is pinned to index 0 regardless of the sort key.
    fn compare_colors(
        palette: &[Color32],
        a: usize,
        b: usize,
        mode: SortMode,
        order: SortOrder,
        first_color_is_transparent: bool,
    ) -> std::cmp::Ordering {
        if first_color_is_transparent {
            if a == 0 {
                return std::cmp::Ordering::Less;
            }
            if b == 0 {
                return std::cmp::Ordering::Greater;
            }
        }

        let key_a = Self::get_sort_key(&palette[a], mode);
        let key_b = Self::get_sort_key(&palette[b], mode);
        let ordering = match order {
            SortOrder::Ascending => key_a.partial_cmp(&key_b),
            SortOrder::Descending => key_b.partial_cmp(&key_a),
        };
        ordering.unwrap_or(std::cmp::Ordering::Equal)
    }

    fn get_sort_key(color: &Color32, mode: SortMode) -> f32 {
        if mode == SortMode::None {
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
    /// RGBA of the top-left pixel, used as the fill when extending the image.
    pub fn top_left_pixel(&self) -> [u8; 4] {
        self.rgba_data
            .get(0..4)
            .map_or([0, 0, 0, 0], |px| [px[0], px[1], px[2], px[3]])
    }

    /// Copy this image into a larger `width` x `height` canvas anchored at the
    /// top left, filling the added area with `fill`.
    pub fn extended_to(
        &self,
        width: u32,
        height: u32,
        fill: [u8; 4],
        ctx: &egui::Context,
    ) -> ImageData {
        let rgba_data = extend_pixels(
            &self.rgba_data,
            self.width,
            self.height,
            width,
            height,
            fill,
        );

        let size = [width as usize, height as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba_data);
        let texture =
            ctx.load_texture("input_extended", color_image, egui::TextureOptions::NEAREST);

        ImageData {
            texture,
            width,
            height,
            rgba_data,
            indexed: None,
        }
    }

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

        // Split the flat palette into per-palette rows for the overlay.
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

    pub fn count_unique_tiles(
        indexed: &ImageDataIndexed,
        width: u32,
        height: u32,
        tile_width: u16,
        tile_height: u16,
        options: TileCountOptions,
    ) -> Option<usize> {
        if tile_width == 0 || tile_height == 0 {
            return None;
        }
        if !width.is_multiple_of(tile_width as u32) || !height.is_multiple_of(tile_height as u32) {
            return None;
        }

        let tiles_x = width / tile_width as u32;
        let tiles_y = height / tile_height as u32;
        let stride = width as usize;
        let tile_w = tile_width as usize;
        let tile_h = tile_height as usize;
        let tile_area = tile_w * tile_h;

        let mut unique_tiles: HashSet<Vec<u8>> = HashSet::new();

        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let mut tile = Vec::with_capacity(tile_area);
                let mut fully_transparent = true;

                for y in 0..tile_h {
                    let offset = ((ty as usize * tile_h + y) * stride) + (tx as usize * tile_w);
                    let row = &indexed.indexed_pixels[offset..offset + tile_w];
                    for &idx in row {
                        if let Some(color) = indexed.palettes.get(idx as usize) {
                            if color.a != 0 {
                                fully_transparent = false;
                            }
                        } else {
                            fully_transparent = false;
                        }
                    }
                    tile.extend_from_slice(row);
                }

                if options.visible_only && fully_transparent {
                    continue;
                }

                let base = tile;
                let mut best = base.clone();

                if options.allow_flip_x {
                    let mut flipped = Vec::with_capacity(tile_area);
                    for y in 0..tile_h {
                        let row_start = y * tile_w;
                        let row = &base[row_start..row_start + tile_w];
                        flipped.extend(row.iter().rev());
                    }
                    if flipped < best {
                        best = flipped;
                    }
                }

                if options.allow_flip_y {
                    let mut flipped = Vec::with_capacity(tile_area);
                    for y in (0..tile_h).rev() {
                        let row_start = y * tile_w;
                        flipped.extend_from_slice(&base[row_start..row_start + tile_w]);
                    }
                    if flipped < best {
                        best = flipped;
                    }
                }

                if options.allow_flip_x && options.allow_flip_y {
                    let mut flipped = Vec::with_capacity(tile_area);
                    for y in (0..tile_h).rev() {
                        let row_start = y * tile_w;
                        let row = &base[row_start..row_start + tile_w];
                        flipped.extend(row.iter().rev());
                    }
                    if flipped < best {
                        best = flipped;
                    }
                }

                unique_tiles.insert(best);
            }
        }

        Some(unique_tiles.len())
    }

    pub fn load(path: &str, ctx: &egui::Context) -> Result<ImageData, String> {
        let img = image::open(path).map_err(|e| format!("Image loading error: {e}"))?;
        let rgba_img = img.to_rgba8();
        let size = [rgba_img.width() as usize, rgba_img.height() as usize];
        let rgba_data = rgba_img.into_raw();

        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba_data);
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

/// Place `src` at the top left of a `width` x `height` RGBA buffer, filling the
/// rest with `fill`. Split out from [`ImageData::extended_to`] so it can be
/// tested without an egui context.
fn extend_pixels(
    src: &[u8],
    src_width: u32,
    src_height: u32,
    width: u32,
    height: u32,
    fill: [u8; 4],
) -> Vec<u8> {
    let copied_width = src_width.min(width);
    let mut out = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        if y < src_height {
            let row_start = (y * src_width * 4) as usize;
            let row_end = row_start + (copied_width * 4) as usize;
            out.extend_from_slice(&src[row_start..row_end]);
        }

        let already_written = if y < src_height { copied_width } else { 0 };
        for _ in already_written..width {
            out.extend_from_slice(&fill);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `extended_to` needs a Context for the texture, so the pixel layout is
    /// covered by a standalone helper that both share.
    #[test]
    fn extending_keeps_the_original_at_the_top_left() {
        // 2x1 image: red, green
        let src = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let fill = [1, 2, 3, 4];
        let out = extend_pixels(&src, 2, 1, 4, 3, fill);

        assert_eq!(out.len(), 4 * 3 * 4);
        assert_eq!(&out[0..4], &[255, 0, 0, 255], "original pixel (0,0)");
        assert_eq!(&out[4..8], &[0, 255, 0, 255], "original pixel (1,0)");
        assert_eq!(&out[8..12], &fill, "padding to the right");
        assert_eq!(&out[12..16], &fill, "padding to the right");
        assert!(
            out[16..].chunks_exact(4).all(|px| px == fill),
            "every added row is filled"
        );
    }

    #[test]
    fn extending_to_the_same_size_is_a_copy() {
        let src = vec![9, 8, 7, 6, 5, 4, 3, 2];
        assert_eq!(extend_pixels(&src, 2, 1, 2, 1, [0; 4]), src);
    }

    fn bgra(r: u8, g: u8, b: u8, a: u8) -> BGRA8 {
        BGRA8 { b, g, r, a }
    }

    fn ui_color(c: &BGRA8) -> Color32 {
        Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
    }

    fn indexed(
        palettes: Vec<BGRA8>,
        colors_per_palette: usize,
        pixels: Vec<u8>,
    ) -> ImageDataIndexed {
        let palettes_for_ui = palettes
            .chunks(colors_per_palette)
            .map(|chunk| chunk.iter().map(ui_color).collect())
            .collect();
        ImageDataIndexed {
            palettes_for_ui,
            palettes,
            indexed_pixels: pixels,
        }
    }

    fn opts(visible_only: bool, flip_x: bool, flip_y: bool) -> TileCountOptions {
        TileCountOptions {
            visible_only,
            allow_flip_x: flip_x,
            allow_flip_y: flip_y,
        }
    }

    /// 4x2 image, 2x2 tiles: the right tile is the horizontal mirror of the left one.
    fn mirrored_pair() -> ImageDataIndexed {
        let palettes = (0..5).map(|i| bgra(i * 10, i * 10, i * 10, 255)).collect();
        #[rustfmt::skip]
        let pixels = vec![
            1, 2,   2, 1,
            3, 4,   4, 3,
        ];
        indexed(palettes, 5, pixels)
    }

    #[test]
    fn count_unique_tiles_collapses_mirrored_tiles() {
        let image = mirrored_pair();
        assert_eq!(
            ImageData::count_unique_tiles(&image, 4, 2, 2, 2, opts(false, true, false)),
            Some(1)
        );
        assert_eq!(
            ImageData::count_unique_tiles(&image, 4, 2, 2, 2, opts(false, false, false)),
            Some(2)
        );
    }

    #[test]
    fn count_unique_tiles_can_skip_fully_transparent_tiles() {
        let mut palettes: Vec<BGRA8> = (0..5).map(|i| bgra(i * 10, i * 10, i * 10, 255)).collect();
        palettes[0] = bgra(0, 0, 0, 0);
        #[rustfmt::skip]
        let pixels = vec![
            1, 2,   0, 0,
            3, 4,   0, 0,
        ];
        let image = indexed(palettes, 5, pixels);

        assert_eq!(
            ImageData::count_unique_tiles(&image, 4, 2, 2, 2, opts(true, false, false)),
            Some(1)
        );
        assert_eq!(
            ImageData::count_unique_tiles(&image, 4, 2, 2, 2, opts(false, false, false)),
            Some(2)
        );
    }

    #[test]
    fn count_unique_tiles_rejects_indivisible_dimensions() {
        let image = mirrored_pair();
        assert_eq!(
            ImageData::count_unique_tiles(&image, 4, 2, 3, 2, opts(false, false, false)),
            None
        );
        assert_eq!(
            ImageData::count_unique_tiles(&image, 4, 2, 0, 2, opts(false, false, false)),
            None
        );
    }

    /// Two palettes of four colors, deliberately unsorted by luminance.
    fn two_palettes() -> ImageDataIndexed {
        let palettes = vec![
            bgra(200, 200, 200, 255),
            bgra(0, 0, 0, 255),
            bgra(120, 120, 120, 255),
            bgra(60, 60, 60, 255),
            bgra(10, 10, 10, 255),
            bgra(250, 250, 250, 255),
            bgra(40, 40, 40, 255),
            bgra(180, 180, 180, 255),
        ];
        indexed(palettes, 4, vec![0, 1, 2, 3, 4, 5, 6, 7])
    }

    /// The color each pixel resolves to must be unchanged by sorting; only the
    /// indices and the palette layout move.
    fn resolved_colors(image: &ImageDataIndexed, colors_per_palette: usize) -> Vec<Color32> {
        image
            .indexed_pixels
            .iter()
            .map(|&px| {
                let palette = px as usize / colors_per_palette;
                let color = px as usize % colors_per_palette;
                image.palettes_for_ui[palette][color]
            })
            .collect()
    }

    #[test]
    fn sorted_preserves_resolved_pixel_colors() {
        let image = two_palettes();
        let before = resolved_colors(&image, 4);

        let sorted = image.sorted(SortMode::Luminance, SortOrder::Ascending, false);

        assert_eq!(resolved_colors(&sorted, 4), before);
        assert_eq!(sorted.palettes.len(), image.palettes.len());
    }

    #[test]
    fn sorted_orders_each_palette_independently() {
        let sorted = two_palettes().sorted(SortMode::Luminance, SortOrder::Ascending, false);

        for palette in &sorted.palettes_for_ui {
            let luminances: Vec<u8> = palette.iter().map(|c| c.r()).collect();
            assert!(
                luminances.windows(2).all(|w| w[0] <= w[1]),
                "palette not ascending: {luminances:?}"
            );
        }
    }

    #[test]
    fn sorted_descending_reverses_the_order() {
        let sorted = two_palettes().sorted(SortMode::Luminance, SortOrder::Descending, false);

        for palette in &sorted.palettes_for_ui {
            let luminances: Vec<u8> = palette.iter().map(|c| c.r()).collect();
            assert!(
                luminances.windows(2).all(|w| w[0] >= w[1]),
                "palette not descending: {luminances:?}"
            );
        }
    }

    #[test]
    fn sorted_pins_the_transparent_first_color() {
        let sorted = two_palettes().sorted(SortMode::Luminance, SortOrder::Ascending, true);

        // Index 0 of each palette must keep its original color.
        assert_eq!(
            sorted.palettes_for_ui[0][0],
            ui_color(&bgra(200, 200, 200, 255))
        );
        assert_eq!(
            sorted.palettes_for_ui[1][0],
            ui_color(&bgra(10, 10, 10, 255))
        );
        assert_eq!(
            resolved_colors(&sorted, 4),
            resolved_colors(&two_palettes(), 4)
        );
    }

    #[test]
    fn sorted_is_a_noop_without_palettes() {
        let empty = ImageDataIndexed {
            palettes_for_ui: Vec::new(),
            palettes: Vec::new(),
            indexed_pixels: vec![0, 1, 2],
        };
        let sorted = empty.sorted(SortMode::Luminance, SortOrder::Ascending, false);
        assert_eq!(sorted.indexed_pixels, vec![0, 1, 2]);
    }
}
