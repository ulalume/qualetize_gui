use super::BGRA8;
use super::ColorCorrection;
use super::palette_ramps::ramp_order;
use crate::color_processor::ColorProcessor;
use egui::{Color32, ColorImage, TextureHandle};
use image::{DynamicImage, ImageDecoder, ImageReader, metadata::Orientation};
use moxcms::{ColorProfile, DataColorSpace, Layout, TransformOptions, Xyzd};
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
    None,
    Luminance,
    Hue,
    Brightness,
    Saturation,
    /// Group colors into ramps by hue and chroma, neutrals first, each ramp
    /// dark to light.
    #[default]
    Ramps,
}

impl SortMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Luminance => "Luminance",
            Self::Hue => "Hue",
            Self::Brightness => "Brightness",
            Self::Saturation => "Saturation",
            Self::Ramps => "Ramps",
        }
    }
    pub fn all() -> &'static [Self] {
        &[
            Self::None,
            Self::Ramps,
            Self::Luminance,
            Self::Hue,
            Self::Brightness,
            Self::Saturation,
        ]
    }
}

impl ImageDataIndexed {
    /// `palettes` is the flat palette buffer as the library produces it; the
    /// per-palette rows for the UI are derived from it here so the two can
    /// never disagree.
    pub fn new(palettes: Vec<BGRA8>, colors_per_palette: usize, indexed_pixels: Vec<u8>) -> Self {
        let palettes_for_ui = palettes
            .chunks(colors_per_palette.max(1))
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|c| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a))
                    .collect()
            })
            .collect();
        Self {
            palettes_for_ui,
            palettes,
            indexed_pixels,
        }
    }

    pub fn colors_per_palette(&self) -> usize {
        self.palettes_for_ui.first().map_or(0, Vec::len)
    }

    /// Every pixel resolved through the palette, as RGBA bytes. Indices past
    /// the end of the palette come out opaque black.
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(self.indexed_pixels.len() * 4);
        for &index in &self.indexed_pixels {
            let px = self
                .palettes
                .get(index as usize)
                .map_or([0, 0, 0, 255], |c| [c.r, c.g, c.b, c.a]);
            pixels.extend_from_slice(&px);
        }
        pixels
    }

    /// Reorder the colors inside every palette, rewriting the pixel indices so
    /// each pixel still resolves to the same color.
    pub fn sorted(
        &self,
        mode: SortMode,
        order: SortOrder,
        first_color_is_transparent: bool,
    ) -> Self {
        let colors_per_palette = self.colors_per_palette();
        if colors_per_palette == 0 {
            return self.clone();
        }

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

            let order_of: Vec<usize> = if mode == SortMode::Ramps {
                Self::ramps_order_of(palette, order, first_color_is_transparent)
            } else {
                let mut order_of: Vec<usize> = (0..colors_per_palette).collect();
                order_of.sort_by(|&a, &b| {
                    Self::compare_colors(palette, a, b, mode, order, first_color_is_transparent)
                });
                order_of
            };

            for (new_idx, &old_idx) in order_of.iter().enumerate() {
                new_palettes[palette_start + new_idx] = self.palettes[palette_start + old_idx];
                if let Some(slot) = remap.get_mut(palette_start + old_idx) {
                    *slot = (palette_start + new_idx) as u8;
                }
            }
        }

        let indexed_pixels = self
            .indexed_pixels
            .iter()
            .map(|&pixel| remap[pixel as usize])
            .collect();
        Self::new(new_palettes, colors_per_palette, indexed_pixels)
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

    /// The palette order for [`SortMode::Ramps`]: a pinned first color stays
    /// at index 0 and is excluded from the ramp computation, the rest are
    /// permuted by [`ramp_order`]. `SortOrder::Descending` reverses the
    /// computed order; the pinned entry is unaffected either way.
    fn ramps_order_of(
        palette: &[Color32],
        order: SortOrder,
        first_color_is_transparent: bool,
    ) -> Vec<usize> {
        let n = palette.len();
        let rest_start = if first_color_is_transparent { 1 } else { 0 };
        let rest_indices: Vec<usize> = (rest_start..n).collect();

        let colors: Vec<[u8; 3]> = rest_indices
            .iter()
            .map(|&i| [palette[i].r(), palette[i].g(), palette[i].b()])
            .collect();

        let mut computed: Vec<usize> = ramp_order(&colors)
            .into_iter()
            .map(|i| rest_indices[i])
            .collect();
        if order == SortOrder::Descending {
            computed.reverse();
        }

        let mut result = Vec::with_capacity(n);
        if first_color_is_transparent {
            result.push(0);
        }
        result.extend(computed);
        result
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
            // Ramps does not use a per-color key; sorted() computes its
            // order for the whole palette with ramp_order instead.
            SortMode::Ramps => 0.0,
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

    /// An indexed image together with the RGBA texture it resolves to.
    pub fn from_indexed(
        indexed: ImageDataIndexed,
        width: u32,
        height: u32,
        ctx: &egui::Context,
    ) -> ImageData {
        let pixels = indexed.to_rgba();
        let size = [width as usize, height as usize];
        let color_image = ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ctx.load_texture("output", color_image, egui::TextureOptions::NEAREST);

        ImageData {
            texture,
            width,
            height,
            rgba_data: pixels,
            indexed: Some(indexed),
        }
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

                // Canonical form: the smallest of the tile and its allowed
                // flips, so mirrored tiles collapse into one entry.
                let mut best = tile.clone();
                for (flip_x, flip_y) in [(true, false), (false, true), (true, true)] {
                    if (flip_x && !options.allow_flip_x) || (flip_y && !options.allow_flip_y) {
                        continue;
                    }
                    let flipped = flip_tile(&tile, tile_w, flip_x, flip_y);
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
        Self::from_rgba(load_rgba(path)?, ctx)
    }

    /// An image decoded from `bytes`; `name` labels log messages.
    pub fn load_from_bytes(
        bytes: &[u8],
        name: &str,
        ctx: &egui::Context,
    ) -> Result<ImageData, String> {
        Self::from_rgba(load_rgba_from_bytes(bytes, name)?, ctx)
    }

    fn from_rgba(
        (rgba_data, width, height): (Vec<u8>, u32, u32),
        ctx: &egui::Context,
    ) -> Result<ImageData, String> {
        let size = [width as usize, height as usize];

        let color_image = ColorImage::from_rgba_unmultiplied(size, &rgba_data);
        let texture = ctx.load_texture("input", color_image, egui::TextureOptions::NEAREST);
        Ok(ImageData {
            texture,
            width,
            height,
            rgba_data,
            indexed: None,
        })
    }
}

/// `path` decoded to RGBA8 with the Exif orientation applied and the embedded
/// ICC profile converted to sRGB. Split out from [`ImageData::load`] so it can
/// be tested without an egui context.
fn load_rgba(path: &str) -> Result<(Vec<u8>, u32, u32), String> {
    let reader = ImageReader::open(path).map_err(|e| format!("Image loading error: {e}"))?;
    decode_rgba(reader, path)
}

/// [`load_rgba`] for an image already in memory; `name` labels log messages.
fn load_rgba_from_bytes(bytes: &[u8], name: &str) -> Result<(Vec<u8>, u32, u32), String> {
    decode_rgba(ImageReader::new(std::io::Cursor::new(bytes)), name)
}

fn decode_rgba<R: std::io::BufRead + std::io::Seek>(
    reader: ImageReader<R>,
    path: &str,
) -> Result<(Vec<u8>, u32, u32), String> {
    let mut decoder = reader
        .with_guessed_format()
        .map_err(|e| format!("Image loading error: {e}"))?
        .into_decoder()
        .map_err(|e| format!("Image loading error: {e}"))?;

    // Decoding consumes the decoder, so the metadata is read first.
    let icc = decoder.icc_profile().unwrap_or_else(|e| {
        log::warn!("{path}: unreadable ICC profile: {e}");
        None
    });
    let orientation = decoder.orientation().unwrap_or_else(|e| {
        log::warn!("{path}: unreadable orientation: {e}");
        Orientation::NoTransforms
    });

    let mut img =
        DynamicImage::from_decoder(decoder).map_err(|e| format!("Image loading error: {e}"))?;
    img.apply_orientation(orientation);

    let rgba_img = img.to_rgba8();
    let (width, height) = (rgba_img.width(), rgba_img.height());
    let mut rgba_data = rgba_img.into_raw();

    if let Some(icc) = icc {
        match to_srgb(&mut rgba_data, &icc) {
            Ok(true) => log::info!("{path}: embedded ICC profile converted to sRGB"),
            Ok(false) => {}
            Err(e) => log::warn!("{path}: embedded ICC profile ignored: {e}"),
        }
    }

    Ok((rgba_data, width, height))
}

/// The RGB channels of `rgba` converted from the ICC profile `icc` to sRGB,
/// in place, with alpha untouched.
///
/// `Ok(false)` means the buffer is left as it is, because the profile already
/// describes sRGB or its color space is one this does not handle (gray, CMYK
/// and the rest).
fn to_srgb(rgba: &mut [u8], icc: &[u8]) -> Result<bool, String> {
    if !rgba.len().is_multiple_of(4) {
        return Err(format!("{} bytes is not whole RGBA pixels", rgba.len()));
    }

    let source = ColorProfile::new_from_slice(icc).map_err(|e| format!("unreadable: {e}"))?;
    if source.color_space != DataColorSpace::Rgb {
        return Ok(false);
    }
    let srgb = ColorProfile::new_srgb();
    if is_srgb(&source, &srgb) {
        return Ok(false);
    }

    let transform = source
        .create_transform_8bit(
            Layout::Rgba,
            &srgb,
            Layout::Rgba,
            TransformOptions::default(),
        )
        .map_err(|e| format!("no transform to sRGB: {e}"))?;

    // The executor reads and writes separate slices, so the pixels pass
    // through a fixed scratch buffer rather than a copy of the whole image.
    const CHUNK_PIXELS: usize = 8192;
    let mut scratch = vec![0u8; CHUNK_PIXELS * 4];
    for chunk in rgba.chunks_mut(CHUNK_PIXELS * 4) {
        let scratch = &mut scratch[..chunk.len()];
        scratch.copy_from_slice(chunk);
        transform
            .transform(scratch, chunk)
            .map_err(|e| format!("transform failed: {e}"))?;
    }

    Ok(true)
}

/// Whether `profile` describes sRGB already: the same primaries and white
/// point, and the same transfer curve on each of the three channels.
fn is_srgb(profile: &ColorProfile, srgb: &ColorProfile) -> bool {
    let same_xyz = |a: Xyzd, b: Xyzd| {
        (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3 && (a.z - b.z).abs() < 1e-3
    };

    same_xyz(profile.red_colorant, srgb.red_colorant)
        && same_xyz(profile.green_colorant, srgb.green_colorant)
        && same_xyz(profile.blue_colorant, srgb.blue_colorant)
        && same_xyz(profile.white_point, srgb.white_point)
        && same_curve(
            profile.build_r_linearize_table::<u8, 256, 8>(false).ok(),
            srgb.build_r_linearize_table::<u8, 256, 8>(false).ok(),
        )
        && same_curve(
            profile.build_g_linearize_table::<u8, 256, 8>(false).ok(),
            srgb.build_g_linearize_table::<u8, 256, 8>(false).ok(),
        )
        && same_curve(
            profile.build_b_linearize_table::<u8, 256, 8>(false).ok(),
            srgb.build_b_linearize_table::<u8, 256, 8>(false).ok(),
        )
}

/// Two 8-bit linearization tables that agree everywhere. A table that could not
/// be built counts as a mismatch.
fn same_curve(a: Option<Box<[f32; 256]>>, b: Option<Box<[f32; 256]>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.iter().zip(b.iter()).all(|(a, b)| (a - b).abs() < 1e-3),
        _ => false,
    }
}

/// `tile` (row-major, `tile_w` wide) mirrored along the requested axes.
fn flip_tile(tile: &[u8], tile_w: usize, flip_x: bool, flip_y: bool) -> Vec<u8> {
    let mut rows: Vec<&[u8]> = tile.chunks_exact(tile_w).collect();
    if flip_y {
        rows.reverse();
    }
    let mut flipped = Vec::with_capacity(tile.len());
    for row in rows {
        if flip_x {
            flipped.extend(row.iter().rev());
        } else {
            flipped.extend_from_slice(row);
        }
    }
    flipped
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
            out[16..].as_chunks::<4>().0.iter().all(|px| *px == fill),
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
        ImageDataIndexed::new(palettes, colors_per_palette, pixels)
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
    fn sorted_ramps_sorts_a_grayscale_palette_dark_to_light() {
        let sorted = two_palettes().sorted(SortMode::Ramps, SortOrder::Ascending, false);

        for palette in &sorted.palettes_for_ui {
            let luminances: Vec<u8> = palette.iter().map(|c| c.r()).collect();
            assert!(
                luminances.windows(2).all(|w| w[0] <= w[1]),
                "palette not ascending: {luminances:?}"
            );
        }
        assert_eq!(
            resolved_colors(&sorted, 4),
            resolved_colors(&two_palettes(), 4)
        );
    }

    #[test]
    fn sorted_ramps_pins_the_transparent_first_color() {
        let sorted = two_palettes().sorted(SortMode::Ramps, SortOrder::Ascending, true);

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
        let empty = ImageDataIndexed::new(Vec::new(), 4, vec![0, 1, 2]);
        let sorted = empty.sorted(SortMode::Luminance, SortOrder::Ascending, false);
        assert_eq!(sorted.indexed_pixels, vec![0, 1, 2]);
    }

    /// Fixtures written by `tests/fixtures/icc/gen.py`.
    fn fixture(name: &str) -> String {
        format!("{}/tests/fixtures/icc/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    /// The ICC profile embedded in a fixture.
    fn fixture_icc(name: &str) -> Vec<u8> {
        ImageReader::open(fixture(name))
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .into_decoder()
            .unwrap()
            .icc_profile()
            .unwrap()
            .expect("the fixture carries an ICC profile")
    }

    #[test]
    fn p3_red_converts_to_clipped_srgb_red() {
        let mut rgba = vec![255, 0, 0, 255];
        assert_eq!(to_srgb(&mut rgba, &fixture_icc("red_p3.png")), Ok(true));

        println!("P3 (255, 0, 0, 255) -> sRGB {rgba:?}");
        // P3 red is outside the sRGB gamut, so it clips to the red corner.
        assert_eq!(rgba[0], 255, "red");
        assert!(rgba[1] <= 40, "green {}", rgba[1]);
        assert!(rgba[2] <= 40, "blue {}", rgba[2]);
        assert_eq!(rgba[3], 255, "alpha is untouched");
    }

    #[test]
    fn p3_gray_stays_gray() {
        let mut rgba = vec![128, 128, 128, 64];
        assert_eq!(to_srgb(&mut rgba, &fixture_icc("red_p3.png")), Ok(true));

        println!("P3 (128, 128, 128, 64) -> sRGB {rgba:?}");
        // The two spaces share a white point, so the gray axis is common.
        for (channel, value) in ["red", "green", "blue"].iter().zip(&rgba[..3]) {
            assert!((*value as i32 - 128).abs() <= 1, "{channel} {value}");
        }
        assert_eq!(rgba[3], 64, "alpha is untouched");
    }

    #[test]
    fn an_srgb_profile_is_left_alone() {
        let mut rgba = vec![255, 0, 0, 255, 12, 34, 56, 78];
        let before = rgba.clone();
        assert_eq!(to_srgb(&mut rgba, &fixture_icc("red_srgb.png")), Ok(false));
        assert_eq!(rgba, before);
    }

    #[test]
    fn an_unreadable_profile_is_an_error() {
        let mut rgba = vec![1, 2, 3, 4];
        assert!(to_srgb(&mut rgba, b"not a profile").is_err());
        assert_eq!(rgba, vec![1, 2, 3, 4], "the buffer is left as it is");
    }

    #[test]
    fn loading_a_p3_image_gives_srgb_pixels() {
        let (rgba, width, height) = load_rgba(&fixture("red_p3.png")).unwrap();
        assert_eq!((width, height), (8, 8));
        assert_eq!(rgba[0], 255);
        assert!(rgba[1] <= 40 && rgba[2] <= 40, "{:?}", &rgba[..4]);
    }

    /// Prints what the loader does to the Display P3 fixture:
    /// `cargo test -- --ignored --nocapture icc_conversion_report`
    #[test]
    #[ignore = "prints measurements rather than asserting"]
    fn icc_conversion_report() {
        let icc = fixture_icc("red_p3.png");
        let raw = image::open(fixture("red_p3.png")).unwrap().to_rgba8();
        let (converted, _, _) = load_rgba(&fixture("red_p3.png")).unwrap();
        println!(
            "red_p3.png: {:?} -> {:?}",
            &raw.as_raw()[..4],
            &converted[..4]
        );

        for probe in [
            [255u8, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [200, 50, 50, 255],
            [128, 128, 128, 255],
            [64, 192, 32, 128],
        ] {
            let mut rgba = probe.to_vec();
            let converted = to_srgb(&mut rgba, &icc);
            println!("P3 {probe:?} -> sRGB {rgba:?} ({converted:?})");
        }
    }

    #[test]
    fn loading_applies_the_exif_orientation() {
        // Stored 32x16 with the left half red; orientation 6 turns it 90 CW.
        let (rgba, width, height) = load_rgba(&fixture("rotated_90.jpg")).unwrap();
        assert_eq!((width, height), (16, 32), "width and height are swapped");

        let pixel = |x: u32, y: u32| {
            let i = ((y * width + x) * 4) as usize;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };
        let top = pixel(0, 0);
        let bottom = pixel(0, 31);
        assert!(top[0] > 200 && top[1] < 60, "top row is red: {top:?}");
        assert!(bottom[0] < 60, "bottom row is black: {bottom:?}");
    }
}
