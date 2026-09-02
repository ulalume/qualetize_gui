//! The palette searches and the passes that build, grow, repair and refine
//! the palettes.

use super::Params;
use super::color::{Rgb, color_distance, move_color_closer};
use super::dither::closest_color_dither;
use super::rng::RandomShuffle;
use super::tile::{Pixel, Tile};
use crate::types::tilepalquant::{ColorZero, TpqDitherMode};

/// An entry picked by one of the searches, with the distance that won it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Candidate {
    pub color_index: usize,
    pub distance: f64,
    /// The color the distance was measured against. A dithered search
    /// compares the pixel plus its accumulated error, not the pixel itself.
    pub compared_color: Rgb,
    pub brightness: f64,
}

/// The index of the largest value, the first one when several are equal.
pub fn max_index(values: &[f64]) -> usize {
    let mut best = 0;
    for index in 1..values.len() {
        if values[index] > values[best] {
            best = index;
        }
    }
    best
}

/// The index of the smallest value, the first one when several are equal.
pub fn min_index(values: &[f64]) -> usize {
    let mut best = 0;
    for index in 1..values.len() {
        if values[index] < values[best] {
            best = index;
        }
    }
    best
}

/// The palette entry closest to `color`.
pub fn closest_color(palette: &[Rgb], color: Rgb) -> Candidate {
    closest_color_with_second(palette, color).0
}

/// The closest entry, plus the distance to the closest of the remaining ones,
/// which says how much that entry is worth keeping.
pub fn closest_color_with_second(palette: &[Rgb], color: Rgb) -> (Candidate, f64) {
    let last = palette.len() - 1;
    let mut min_index = last;
    let mut min_distance = color_distance(palette[last], color);
    let mut second = 0.0;
    let mut have_second = false;
    for index in (0..last).rev() {
        let distance = color_distance(palette[index], color);
        if distance < min_distance {
            second = min_distance;
            have_second = true;
            min_index = index;
            min_distance = distance;
        } else if !have_second || distance < second {
            second = distance;
            have_second = true;
        }
    }
    let candidate = Candidate {
        color_index: min_index,
        distance: min_distance,
        ..Candidate::default()
    };
    (candidate, if have_second { second } else { 0.0 })
}

/// How badly `palette` fits `tile`: the distance from every color to its
/// closest entry, weighted by how many pixels carry it.
///
/// With a `bound`, the sum is abandoned as soon as it passes it; the caller
/// only wanted to know whether this palette could still win.
pub fn palette_distance(palette: &[Rgb], tile: &Tile, bound: Option<f64>) -> f64 {
    let mut sum = 0.0;
    for (index, &color) in tile.colors.iter().enumerate() {
        sum += f64::from(tile.counts[index]) * closest_color(palette, color).distance;
        if bound.is_some_and(|bound| sum > bound) {
            return sum;
        }
    }
    sum
}

/// [`palette_distance`] measured through the dither, over every pixel rather
/// than every distinct color.
pub fn palette_distance_dither(palette: &[Rgb], tile: &Tile, p: &Params) -> f64 {
    let mut sum = 0.0;
    for pixel in &tile.pixels {
        sum += closest_color_dither(palette, pixel, p).distance;
    }
    sum
}

pub fn closest_palette_index(palettes: &[Vec<Rgb>], tile: &Tile) -> usize {
    if palettes.len() == 1 {
        return 0;
    }
    let mut best = 0;
    let mut min_distance = palette_distance(&palettes[0], tile, None);
    for (index, palette) in palettes.iter().enumerate().skip(1) {
        let distance = palette_distance(palette, tile, Some(min_distance));
        if distance < min_distance {
            min_distance = distance;
            best = index;
        }
    }
    best
}

pub fn closest_palette_index_dither(palettes: &[Vec<Rgb>], tile: &Tile, p: &Params) -> usize {
    if palettes.len() == 1 {
        return 0;
    }
    let mut best = 0;
    let mut min_distance = palette_distance_dither(&palettes[0], tile, p);
    for (index, palette) in palettes.iter().enumerate().skip(1) {
        let distance = palette_distance_dither(palette, tile, p);
        if distance < min_distance {
            min_distance = distance;
            best = index;
        }
    }
    best
}

/// The closest palette, plus the distance to the closest of the others.
pub fn closest_palette_distance_with_second(
    palettes: &[Vec<Rgb>],
    tile: &Tile,
) -> (Candidate, f64) {
    let mut best = 0;
    let mut min_distance = palette_distance(&palettes[0], tile, None);
    let mut second = 0.0;
    let mut have_second = false;
    for (index, palette) in palettes.iter().enumerate().skip(1) {
        let distance = palette_distance(palette, tile, None);
        if distance < min_distance {
            second = min_distance;
            have_second = true;
            min_distance = distance;
            best = index;
        } else if !have_second || distance < second {
            second = distance;
            have_second = true;
        }
    }
    let candidate = Candidate {
        color_index: best,
        distance: min_distance,
        ..Candidate::default()
    };
    (candidate, if have_second { second } else { 0.0 })
}

/// [`closest_palette_distance_with_second`] measured through the dither.
pub fn closest_palette_distance_dither_with_second(
    palettes: &[Vec<Rgb>],
    tile: &Tile,
    p: &Params,
) -> (Candidate, f64) {
    let distances: Vec<f64> = palettes
        .iter()
        .map(|palette| palette_distance_dither(palette, tile, p))
        .collect();
    let best = min_index(&distances);
    let mut second = 0.0;
    let mut have_second = false;
    for (index, &distance) in distances.iter().enumerate() {
        if index != best && (!have_second || distance < second) {
            second = distance;
            have_second = true;
        }
    }
    let candidate = Candidate {
        color_index: best,
        distance: distances[best],
        ..Candidate::default()
    };
    (candidate, if have_second { second } else { 0.0 })
}

/// Move the one palette entry that `pixel` would be drawn with a step towards
/// it. This is the whole of the optimization's inner loop.
pub fn move_palettes_closer(
    palettes: &mut [Vec<Rgb>],
    tiles: &[Tile],
    pixel: &Pixel,
    alpha: f32,
    p: &Params,
) {
    let tile = &tiles[pixel.tile_id];
    let (palette_index, color_index, target) = if p.dither == TpqDitherMode::Slow {
        let palette_index = closest_palette_index_dither(palettes, tile, p);
        let candidate = closest_color_dither(&palettes[palette_index], pixel, p);
        (
            palette_index,
            candidate.color_index,
            candidate.compared_color,
        )
    } else {
        let palette_index = closest_palette_index(palettes, tile);
        let candidate = closest_color(&palettes[palette_index], pixel.color);
        (palette_index, candidate.color_index, pixel.color)
    };
    if Some(color_index) != p.shared_color_index() {
        move_color_closer(&mut palettes[palette_index][color_index], target, alpha);
    }
}

/// Replace the entries that carry the least: in every palette the color whose
/// pixels have the closest alternative is overwritten with a copy of the one
/// under the most strain, and the same for whole palettes.
pub fn replace_weakest_colors(
    palettes: &[Vec<Rgb>],
    tiles: &[Tile],
    min_color_factor: f32,
    min_palette_factor: f32,
    replace_palettes: bool,
    p: &Params,
) -> Vec<Vec<Rgb>> {
    let slow_dither = p.dither == TpqDitherMode::Slow;
    let mut palette_of_tile = vec![0usize; tiles.len()];
    let mut max_palette_index = 0;
    let mut min_palette_index = 0;
    let mut total_palette_mse = vec![0.0; palettes.len()];
    let mut removed_palette_mse = vec![0.0; palettes.len()];

    if palettes.len() > 1 {
        for (tile_index, tile) in tiles.iter().enumerate() {
            let (candidate, second) = if slow_dither {
                closest_palette_distance_dither_with_second(palettes, tile, p)
            } else {
                closest_palette_distance_with_second(palettes, tile)
            };
            total_palette_mse[candidate.color_index] += candidate.distance;
            palette_of_tile[tile_index] = candidate.color_index;
            removed_palette_mse[candidate.color_index] += second;
        }
        max_palette_index = max_index(&total_palette_mse);
        min_palette_index = min_index(&removed_palette_mse);
    }

    let mut result: Vec<Vec<Rgb>> = Vec::with_capacity(palettes.len());
    if palettes[0].len() > 1 {
        let mut total_color_mse: Vec<Vec<f64>> =
            palettes.iter().map(|pal| vec![0.0; pal.len()]).collect();
        let mut second_color_mse = total_color_mse.clone();

        for (tile_index, tile) in tiles.iter().enumerate() {
            let palette_index = palette_of_tile[tile_index];
            let palette = &palettes[palette_index];
            if slow_dither {
                for pixel in &tile.pixels {
                    let candidate = closest_color_dither(palette, pixel, p);
                    total_color_mse[palette_index][candidate.color_index] += candidate.distance;
                    let remaining: Vec<Rgb> = palette
                        .iter()
                        .enumerate()
                        .filter(|&(index, _)| index != candidate.color_index)
                        .map(|(_, &color)| color)
                        .collect();
                    let second = closest_color_dither(&remaining, pixel, p);
                    second_color_mse[palette_index][candidate.color_index] += second.distance;
                }
            } else {
                for (index, &color) in tile.colors.iter().enumerate() {
                    let (candidate, second) = closest_color_with_second(palette, color);
                    let count = f64::from(tile.counts[index]);
                    total_color_mse[palette_index][candidate.color_index] +=
                        candidate.distance * count;
                    second_color_mse[palette_index][candidate.color_index] += second * count;
                }
            }
        }

        for (palette_index, palette) in palettes.iter().enumerate() {
            let max_color_index = max_index(&total_color_mse[palette_index]);
            let min_color_index = min_index(&second_color_mse[palette_index]);
            let replace_min_color = min_color_index != max_color_index
                && Some(min_color_index) != p.shared_color_index()
                && second_color_mse[palette_index][min_color_index]
                    < f64::from(min_color_factor) * total_color_mse[palette_index][max_color_index];
            let colors = (0..palette.len())
                .map(|index| {
                    if index == min_color_index && replace_min_color {
                        palette[max_color_index]
                    } else {
                        palette[index]
                    }
                })
                .collect();
            result.push(colors);
        }
    } else {
        result.extend(palettes.iter().cloned());
    }

    if replace_palettes
        && min_palette_index != max_palette_index
        && removed_palette_mse[min_palette_index]
            < f64::from(min_palette_factor) * total_palette_mse[max_palette_index]
    {
        result[min_palette_index] = result[max_palette_index].clone();
    }
    result
}

/// One k-means step: every color moves to the weighted average of the pixels
/// that resolve to it. Entries nothing resolves to keep their value, as does
/// the shared index 0.
pub fn k_means(palettes: &[Vec<Rgb>], tiles: &[Tile], p: &Params) -> Vec<Vec<Rgb>> {
    let mut counts: Vec<Vec<i32>> = palettes.iter().map(|pal| vec![0; pal.len()]).collect();
    let mut sums: Vec<Vec<Rgb>> = palettes
        .iter()
        .map(|pal| vec![Rgb::default(); pal.len()])
        .collect();

    for tile in tiles {
        if p.dither == TpqDitherMode::Slow {
            let palette_index = closest_palette_index_dither(palettes, tile, p);
            for pixel in &tile.pixels {
                let candidate = closest_color_dither(&palettes[palette_index], pixel, p);
                counts[palette_index][candidate.color_index] += 1;
                sums[palette_index][candidate.color_index].add(pixel.color);
            }
        } else {
            let palette_index = closest_palette_index(palettes, tile);
            for (index, &color) in tile.colors.iter().enumerate() {
                let candidate = closest_color(&palettes[palette_index], color);
                counts[palette_index][candidate.color_index] += tile.counts[index];
                let mut weighted = color;
                weighted.scale(f64::from(tile.counts[index]));
                sums[palette_index][candidate.color_index].add(weighted);
            }
        }
    }

    for (palette_index, palette) in palettes.iter().enumerate() {
        for color_index in 0..palette.len() {
            let count = counts[palette_index][color_index];
            if count == 0 || Some(color_index) == p.shared_color_index() {
                sums[palette_index][color_index] = palette[color_index];
            } else {
                sums[palette_index][color_index].scale(1.0 / f64::from(count));
            }
        }
    }
    sums
}

/// The average error of the whole image under these palettes, in `f32` as the
/// reference implementation keeps it: the value decides which iteration's
/// palettes are kept.
pub fn mean_square_error(palettes: &[Vec<Rgb>], tiles: &[Tile]) -> f32 {
    let mut total_distance = 0.0;
    let mut count: i64 = 0;
    for tile in tiles {
        let palette_index = closest_palette_index(palettes, tile);
        for (index, &color) in tile.colors.iter().enumerate() {
            let distance = closest_color(&palettes[palette_index], color).distance;
            total_distance += distance * f64::from(tile.counts[index]);
            count += i64::from(tile.counts[index]);
        }
    }
    (total_distance / count as f64) as f32
}

/// Every color snapped to the levels the target format allows.
pub fn reduce_palettes(palettes: &[Vec<Rgb>], p: &Params) -> Vec<Vec<Rgb>> {
    palettes
        .iter()
        .map(|palette| palette.iter().map(|&color| p.lut.snap(color)).collect())
        .collect()
}

/// Build one color per palette: start from the average of the image and split
/// the palette that fits its tiles worst, until there are enough palettes.
pub fn quantize_1_color(
    tiles: &[Tile],
    pixels: &[Pixel],
    shuffle: &mut RandomShuffle,
    p: &Params,
) -> Vec<Vec<Rgb>> {
    let mut average = Rgb::default();
    for pixel in pixels {
        average.add(pixel.color);
    }
    average.scale(1.0 / f64::from(pixels.len() as f32));

    let mut palettes = vec![vec![average]];
    if p.color_zero == ColorZero::Shared {
        palettes[0].push(average);
        palettes[0][0] = p.color_zero_value;
    }

    let mut split_index = 0;
    for palette_count in 2..=p.n_palettes {
        palettes.push(palettes[split_index].clone());
        for _ in 0..p.iterations {
            let pixel = pixels[shuffle.next()];
            move_palettes_closer(&mut palettes, tiles, &pixel, p.alpha, p);
        }
        let mut distances = vec![0.0; palette_count];
        for tile in tiles {
            let (candidate, _) = closest_palette_distance_with_second(&palettes, tile);
            distances[candidate.color_index] += candidate.distance;
        }
        split_index = max_index(&distances);
    }
    palettes
}

/// Add one color to every palette, splitting the entry that carries the most
/// error, and settle the palettes again.
pub fn expand_by_one_color(
    palettes: &mut [Vec<Rgb>],
    tiles: &[Tile],
    pixels: &[Pixel],
    shuffle: &mut RandomShuffle,
    p: &Params,
) {
    let num_colors = palettes[0].len() + 1;
    let mut split_indexes = vec![0usize; palettes.len()];
    if num_colors > 2 {
        let mut total_color_distances = vec![vec![0.0; num_colors]; palettes.len()];
        for tile in tiles {
            let palette_index = closest_palette_index(palettes, tile);
            let palette = &palettes[palette_index];
            for (index, &color) in tile.colors.iter().enumerate() {
                let candidate = closest_color(palette, color);
                total_color_distances[palette_index][candidate.color_index] +=
                    f64::from(tile.counts[index]) * candidate.distance;
            }
        }
        for (index, distances) in total_color_distances.iter().enumerate() {
            split_indexes[index] = max_index(distances);
        }
    }

    for (index, palette) in palettes.iter_mut().enumerate() {
        let split = palette[split_indexes[index]];
        palette.push(split);
    }

    for _ in 0..p.iterations {
        let pixel = pixels[shuffle.next()];
        move_palettes_closer(palettes, tiles, &pixel, p.alpha, p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tilepalquant::tests::params;

    fn palette() -> Vec<Rgb> {
        vec![
            Rgb::new(0.0, 0.0, 0.0),
            Rgb::new(255.0, 0.0, 0.0),
            Rgb::new(0.0, 255.0, 0.0),
            Rgb::new(255.0, 255.0, 255.0),
        ]
    }

    fn tile_of(colors: &[(Rgb, i32)]) -> Tile {
        let mut tile = Tile::default();
        for (index, &(color, count)) in colors.iter().enumerate() {
            tile.colors.push(color);
            tile.counts.push(count);
            for _ in 0..count {
                tile.pixels.push(Pixel {
                    tile_id: 0,
                    color,
                    x: index as u32,
                    y: 0,
                });
            }
        }
        tile
    }

    #[test]
    fn the_extremes_are_the_first_of_equal_values() {
        assert_eq!(max_index(&[1.0, 3.0, 3.0, 2.0]), 1);
        assert_eq!(min_index(&[2.0, 1.0, 1.0, 3.0]), 1);
        assert_eq!(max_index(&[0.0]), 0);
    }

    #[test]
    fn the_closest_color_is_the_one_it_is_nearest() {
        let palette = palette();
        let found = closest_color(&palette, Rgb::new(240.0, 10.0, 10.0));
        assert_eq!(found.color_index, 1);
        assert!(found.distance > 0.0);
        assert_eq!(closest_color(&palette, palette[2]).distance, 0.0);
    }

    #[test]
    fn equal_distances_keep_the_later_entry() {
        // The search walks the palette backwards and only moves on a strictly
        // smaller distance, so the last of several equals wins.
        let palette = vec![Rgb::new(10.0, 0.0, 0.0), Rgb::new(10.0, 0.0, 0.0)];
        assert_eq!(closest_color(&palette, Rgb::default()).color_index, 1);
    }

    #[test]
    fn the_second_distance_is_the_best_of_the_remaining_entries() {
        let palette = palette();
        let (found, second) = closest_color_with_second(&palette, Rgb::new(250.0, 5.0, 5.0));
        assert_eq!(found.color_index, 1);
        let without_winner: Vec<Rgb> = palette
            .iter()
            .enumerate()
            .filter(|&(index, _)| index != found.color_index)
            .map(|(_, &color)| color)
            .collect();
        assert_eq!(
            second,
            closest_color(&without_winner, Rgb::new(250.0, 5.0, 5.0)).distance
        );
    }

    #[test]
    fn a_single_entry_palette_has_no_second_distance() {
        let (found, second) = closest_color_with_second(&[Rgb::default()], Rgb::new(1.0, 2.0, 3.0));
        assert_eq!(found.color_index, 0);
        assert_eq!(second, 0.0);
    }

    #[test]
    fn a_bound_does_not_change_which_palette_wins() {
        let tile = tile_of(&[
            (Rgb::new(250.0, 0.0, 0.0), 3),
            (Rgb::new(10.0, 10.0, 10.0), 1),
        ]);
        let palettes = vec![palette(), vec![Rgb::new(128.0, 128.0, 128.0); 4]];
        let unbounded: Vec<f64> = palettes
            .iter()
            .map(|palette| palette_distance(palette, &tile, None))
            .collect();
        assert_eq!(
            closest_palette_index(&palettes, &tile),
            min_index(&unbounded)
        );
        // A bound only abandons sums that have already lost.
        assert!(palette_distance(&palettes[1], &tile, Some(0.0)) > 0.0);
    }

    #[test]
    fn counts_weight_the_palette_distance() {
        let once = tile_of(&[(Rgb::new(100.0, 0.0, 0.0), 1)]);
        let thrice = tile_of(&[(Rgb::new(100.0, 0.0, 0.0), 3)]);
        let palette = palette();
        assert_eq!(
            palette_distance(&palette, &thrice, None),
            3.0 * palette_distance(&palette, &once, None)
        );
    }

    #[test]
    fn the_second_palette_distance_is_the_best_of_the_others() {
        let tile = tile_of(&[(Rgb::new(250.0, 0.0, 0.0), 1)]);
        let palettes = vec![
            palette(),
            vec![Rgb::new(128.0, 128.0, 128.0); 4],
            vec![Rgb::new(200.0, 0.0, 0.0); 4],
        ];
        let (found, second) = closest_palette_distance_with_second(&palettes, &tile);
        assert_eq!(found.color_index, 0);
        assert_eq!(second, palette_distance(&palettes[2], &tile, None));
    }

    #[test]
    fn the_error_is_the_average_over_every_pixel() {
        let tile = tile_of(&[(Rgb::new(0.0, 0.0, 0.0), 1), (Rgb::new(0.0, 0.0, 10.0), 3)]);
        let palettes = vec![vec![Rgb::default(); 2]];
        // Three pixels are 10 blue away from black, one is exactly on it.
        assert_eq!(mean_square_error(&palettes, &[tile]), 75.0);
    }

    #[test]
    fn k_means_moves_colors_onto_the_pixels_that_chose_them() {
        let tile = tile_of(&[(Rgb::new(10.0, 0.0, 0.0), 1), (Rgb::new(20.0, 0.0, 0.0), 3)]);
        let palettes = vec![vec![Rgb::new(0.0, 0.0, 0.0), Rgb::new(255.0, 255.0, 255.0)]];
        let moved = k_means(&palettes, &[tile], &params());
        assert_eq!(moved[0][0], Rgb::new(17.5, 0.0, 0.0));
        assert_eq!(moved[0][1], palettes[0][1], "nothing chose it, so it stays");
    }

    #[test]
    fn replacing_the_weakest_color_copies_the_one_under_the_most_strain() {
        // Two entries sit on the same color, so one of them earns nothing.
        let tile = tile_of(&[(Rgb::new(0.0, 0.0, 0.0), 1), (Rgb::new(200.0, 0.0, 0.0), 8)]);
        let palettes = vec![vec![
            Rgb::new(0.0, 0.0, 0.0),
            Rgb::new(0.0, 0.0, 0.0),
            Rgb::new(100.0, 0.0, 0.0),
        ]];
        let repaired = replace_weakest_colors(&palettes, &[tile], 0.5, 0.5, true, &params());
        assert_eq!(repaired[0].len(), 3);
        assert!(
            repaired[0]
                .iter()
                .filter(|&&c| c == Rgb::new(100.0, 0.0, 0.0))
                .count()
                == 2,
            "the entry carrying the error was duplicated: {:?}",
            repaired[0]
        );
    }

    #[test]
    fn reducing_snaps_every_color_to_the_target_levels() {
        let palettes = vec![vec![Rgb::new(1.0, 130.0, 254.0)]];
        let reduced = reduce_palettes(&palettes, &params());
        assert_eq!(reduced[0][0], Rgb::new(0.0, 146.0, 255.0));
    }
}
