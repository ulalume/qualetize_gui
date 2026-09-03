//! Turning palettes into the engine's result: one palette per tile, one index
//! per pixel, and the flat palette buffer the host draws with.

use super::Params;
use super::color::Rgb;
use super::dither::closest_color_dither;
use super::palette::{closest_color, closest_palette_index, closest_palette_index_dither};
use super::tile::{Pixel, SourceImage, extract_tile};
use crate::engine::QuantizeResult;
use crate::types::BGRA8;
use crate::types::FirstColor;
use crate::types::tilepalquant::TpqDitherMode;

/// Quantize the whole image against `palettes`.
///
/// The same code produces the final result and every intermediate preview, so
/// a preview differs from the result only in how far the palettes have
/// converged. Palettes shorter than the target color count -- which is what
/// the previews taken while the palettes are still growing hold -- come out
/// with a matching shorter stride.
pub fn quantize_tiles(
    palettes: &[Vec<Rgb>],
    image: &SourceImage,
    use_dither: bool,
    p: &Params,
) -> QuantizeResult {
    let adjusted_index = p.adjusted_index();
    let reduced: Vec<Vec<Rgb>> = palettes
        .iter()
        .map(|palette| palette.iter().map(|&color| p.lut.snap(color)).collect())
        .collect();

    // The image was snapped to the target levels up front only when dithering
    // is off, so the color the transparent pixels are recognized by is snapped
    // in exactly the other case.
    let transparent_color = if p.dither == TpqDitherMode::Off {
        p.first_color_value
    } else {
        p.lut.snap(p.first_color_value)
    };
    let snapped_first_color = p.lut.snap(p.first_color_value);

    let palette_len = reduced[0].len();
    let colors_per_palette = palette_len + adjusted_index;
    let mut palette_data = Vec::with_capacity(reduced.len() * colors_per_palette);
    for palette in &reduced {
        if adjusted_index == 1 {
            let [r, g, b] = snapped_first_color.to_u8();
            palette_data.push(BGRA8 { b, g, r, a: 0 });
        }
        for &color in palette {
            let [r, g, b] = color.to_u8();
            palette_data.push(BGRA8 { b, g, r, a: 255 });
        }
    }

    let mut indexed_data = vec![0u8; (image.width as usize) * (image.height as usize)];
    for start_y in (0..image.height).step_by(p.tile_height as usize) {
        for start_x in (0..image.width).step_by(p.tile_width as usize) {
            let tile = extract_tile(
                image,
                start_x,
                start_y,
                p.tile_width,
                p.tile_height,
                p.transparency(),
                0,
            );
            let mut palette_index = 0;
            if !tile.colors.is_empty() {
                palette_index = if use_dither {
                    closest_palette_index_dither(&reduced, &tile, p)
                } else {
                    closest_palette_index(&reduced, &tile)
                };
            }
            let palette = &reduced[palette_index];
            let palette_base = palette_index * colors_per_palette;

            let end_x = (start_x + p.tile_width).min(image.width);
            let end_y = (start_y + p.tile_height).min(image.height);
            for y in start_y..end_y {
                for x in start_x..end_x {
                    let color = image.color(x, y);
                    let transparent = match p.first_color {
                        FirstColor::TransparentFromAlpha => image.alpha(x, y) < 255,
                        FirstColor::TransparentFromColor => color == transparent_color,
                        FirstColor::Unique | FirstColor::Shared => false,
                    };
                    let index = if transparent {
                        palette_base
                    } else {
                        let color_index = if use_dither {
                            let pixel = Pixel {
                                tile_id: 0,
                                color,
                                x,
                                y,
                            };
                            closest_color_dither(palette, &pixel, p).color_index
                        } else {
                            closest_color(palette, color).color_index
                        };
                        palette_base + color_index + adjusted_index
                    };
                    indexed_data[x as usize + image.width as usize * y as usize] = index as u8;
                }
            }
        }
    }

    QuantizeResult {
        indexed_data,
        palette_data,
        colors_per_palette,
        width: image.width,
        height: image.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tilepalquant::Params;
    use crate::engine::tilepalquant::tests::params;

    /// A 2x1 image, one red pixel and one blue one, each its own tile.
    fn image() -> SourceImage {
        SourceImage::new(vec![255, 0, 0, 255, 0, 0, 255, 255], 2, 1)
    }

    fn single_pixel_tiles() -> Params {
        Params {
            tile_width: 1,
            tile_height: 1,
            ..params()
        }
    }

    #[test]
    fn a_pixel_index_is_its_palette_times_the_stride_plus_its_color() {
        let palettes = vec![
            vec![Rgb::default(), Rgb::new(255.0, 0.0, 0.0)],
            vec![Rgb::default(), Rgb::new(0.0, 0.0, 255.0)],
        ];
        let result = quantize_tiles(&palettes, &image(), false, &single_pixel_tiles());
        assert_eq!(result.colors_per_palette, 2);
        assert_eq!(result.palette_data.len(), 4);
        assert_eq!(result.indexed_data, [1, 3]);
        assert!(result.palette_data.iter().all(|color| color.a == 255));
    }

    #[test]
    fn a_transparent_mode_inserts_index_zero_and_widens_the_stride() {
        let mut p = Params {
            first_color: FirstColor::TransparentFromColor,
            first_color_value: Rgb::new(255.0, 0.0, 0.0),
            ..single_pixel_tiles()
        };
        p.colors_per_palette = 3;
        let palettes = vec![
            vec![Rgb::new(0.0, 255.0, 0.0), Rgb::new(0.0, 0.0, 255.0)],
            vec![Rgb::new(255.0, 255.0, 0.0), Rgb::new(0.0, 255.0, 255.0)],
        ];
        let result = quantize_tiles(&palettes, &image(), false, &p);
        assert_eq!(
            result.colors_per_palette, 3,
            "two colors plus the inserted one"
        );
        assert_eq!(result.palette_data.len(), 6);
        assert_eq!(
            (result.palette_data[0].r, result.palette_data[0].a),
            (255, 0),
            "index 0 is the key color, marked clear"
        );
        // The red pixel is the key color, so it takes index 0 of its palette;
        // the blue one takes the entry it matches, one slot further along.
        assert_eq!(result.indexed_data[0] % 3, 0);
        assert_eq!(result.indexed_data[1] % 3, 2);
    }

    #[test]
    fn palettes_shorter_than_the_target_get_a_matching_stride() {
        // A preview taken while the palettes still hold one color each.
        let palettes = vec![
            vec![Rgb::new(255.0, 0.0, 0.0)],
            vec![Rgb::new(0.0, 0.0, 255.0)],
        ];
        let result = quantize_tiles(&palettes, &image(), false, &single_pixel_tiles());
        assert_eq!(result.colors_per_palette, 1);
        assert_eq!(result.indexed_data, [0, 1]);
        assert!(
            result
                .indexed_data
                .iter()
                .all(|&index| (index as usize) < result.palette_data.len())
        );
    }

    #[test]
    fn output_colors_are_snapped_to_the_target_levels() {
        let palettes = vec![vec![Rgb::new(200.0, 200.0, 200.0)], vec![Rgb::default()]];
        let result = quantize_tiles(&palettes, &image(), false, &single_pixel_tiles());
        assert_eq!(
            (result.palette_data[0].r, result.palette_data[0].g),
            (182, 182)
        );
    }

    #[test]
    fn a_tile_with_nothing_opaque_falls_back_to_the_first_palette() {
        let image = SourceImage::new(vec![0; 8], 2, 1);
        let p = Params {
            first_color: FirstColor::TransparentFromAlpha,
            first_color_value: Rgb::new(255.0, 0.0, 255.0),
            ..single_pixel_tiles()
        };
        let palettes = vec![vec![Rgb::new(1.0, 2.0, 3.0)], vec![Rgb::new(4.0, 5.0, 6.0)]];
        let result = quantize_tiles(&palettes, &image, false, &p);
        assert_eq!(result.indexed_data, [0, 0]);
    }
}
