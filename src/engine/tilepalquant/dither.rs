//! Ordered dithering: several candidate colors per pixel, one of them picked
//! by the pixel's place in a 2x2 pattern.

use super::Params;
use super::color::{Rgb, brightness};
use super::palette::{Candidate, closest_color};
use super::tile::Pixel;

/// The palette entry `pixel` is drawn with.
///
/// Each of the `p.dither_pixels` candidates is the closest entry to the pixel
/// plus the error the candidates before it left over; ordering them by
/// brightness and indexing with the pattern spreads that error over the 2x2
/// block instead of accumulating it.
pub fn closest_color_dither(palette: &[Rgb], pixel: &Pixel, p: &Params) -> Candidate {
    let mut error = Rgb::default();
    let linear_pixel = pixel.color.to_linear();

    let count = p.dither_pixels;
    let mut candidates = [Candidate::default(); 4];
    for slot in candidates.iter_mut().take(count) {
        let mut compared = linear_pixel;
        let mut weighted_error = error;
        weighted_error.scale(f64::from(p.dither_weight));
        compared.add(weighted_error);
        compared.clamp(0.0, 255.0 * 255.0);
        let compared = compared.to_srgb();

        let mut candidate = closest_color(palette, compared);
        let chosen = palette[candidate.color_index];
        candidate.compared_color = compared;
        candidate.brightness = brightness(chosen);
        *slot = candidate;

        // The error is measured against the color as it will be written, so
        // it has to go through the target format's levels first.
        let reduced = p.lut.snap(chosen).to_linear();
        error.add(linear_pixel);
        error.subtract(reduced);
    }

    for i in 0..count - 1 {
        for j in i + 1..count {
            if candidates[i].brightness > candidates[j].brightness {
                candidates.swap(i, j);
            }
        }
    }
    candidates[p.dither_index(pixel.x, pixel.y)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tilepalquant::tests::params_with;
    use crate::types::tilepalquant::DitherPattern;

    fn pixel(x: u32, y: u32, color: Rgb) -> Pixel {
        Pixel {
            tile_id: 0,
            color,
            x,
            y,
        }
    }

    #[test]
    fn a_single_entry_palette_leaves_no_choice() {
        let p = params_with(DitherPattern::Diagonal4, 0.5);
        let palette = [Rgb::new(10.0, 20.0, 30.0)];
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let found = closest_color_dither(&palette, &pixel(x, y, Rgb::default()), &p);
            assert_eq!(found.color_index, 0);
        }
    }

    #[test]
    fn the_pattern_spreads_a_midpoint_over_the_two_by_two_block() {
        // Halfway between black and white, so the candidates alternate and
        // the pattern decides which pixel takes which.
        let p = params_with(DitherPattern::Diagonal2, 1.0);
        let palette = [Rgb::new(0.0, 0.0, 0.0), Rgb::new(255.0, 255.0, 255.0)];
        let grey = Rgb::new(180.0, 180.0, 180.0);
        let taken: Vec<usize> = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .map(|&(x, y)| closest_color_dither(&palette, &pixel(x, y, grey), &p).color_index)
            .collect();
        assert_eq!(taken, [0, 1, 1, 0], "the diagonal of the 2x2 block matches");
    }

    #[test]
    fn candidates_come_out_ordered_by_brightness() {
        let p = params_with(DitherPattern::Horizontal4, 0.5);
        let palette = [
            Rgb::new(0.0, 0.0, 0.0),
            Rgb::new(90.0, 90.0, 90.0),
            Rgb::new(180.0, 180.0, 180.0),
            Rgb::new(255.0, 255.0, 255.0),
        ];
        let color = Rgb::new(140.0, 140.0, 140.0);
        // Horizontal4 is [[0, 3], [1, 2]], so (0,0) takes the darkest
        // candidate and (1,0) the brightest.
        let darkest = closest_color_dither(&palette, &pixel(0, 0, color), &p);
        let brightest = closest_color_dither(&palette, &pixel(1, 0, color), &p);
        assert!(
            darkest.brightness <= brightest.brightness,
            "{darkest:?} {brightest:?}"
        );
    }

    #[test]
    fn a_two_candidate_pattern_only_diffuses_once() {
        let p = params_with(DitherPattern::Vertical2, 0.5);
        assert_eq!(p.dither_pixels, 2);
        let palette = [Rgb::new(0.0, 0.0, 0.0), Rgb::new(255.0, 255.0, 255.0)];
        let found = closest_color_dither(&palette, &pixel(0, 0, Rgb::new(20.0, 20.0, 20.0)), &p);
        assert_eq!(found.color_index, 0);
    }
}
