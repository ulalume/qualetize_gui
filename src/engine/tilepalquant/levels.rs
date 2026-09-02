//! Snapping color channels to the levels the target format allows.
//!
//! Every color the engine writes passes through here: the source pre-pass,
//! `reduce_palettes`, the color the dither error is measured against, the
//! output palettes, and the color-zero value. Alpha is never snapped.

use super::color::Rgb;

/// The values one channel may take, and the rule for snapping to them.
///
/// Evenly spaced levels are a rounding of a grid, `round(k * alpha)`, and are
/// quantized on that grid: the level index is `round(value / alpha)`. Ties
/// then fall where the unrounded grid puts them, which is not always on the
/// nearer of the two rounded levels. Any other list snaps to the nearest
/// level, ties going to the lower one.
pub struct ChannelLut {
    levels: Vec<u8>,
    /// `Some(alpha)` when the levels are a grid of that spacing.
    grid_alpha: Option<f64>,
    /// Midpoints between neighboring levels, for unevenly spaced levels.
    thresholds: Vec<f64>,
    table: [u8; 256],
}

impl ChannelLut {
    /// `levels` is sorted and deduplicated here; an empty list means every
    /// byte value is allowed.
    pub fn new(levels: &[u8]) -> Self {
        let mut levels = levels.to_vec();
        levels.sort_unstable();
        levels.dedup();
        if levels.is_empty() {
            levels = (0..=255).collect();
        }
        let grid_alpha = grid_alpha(&levels);
        let thresholds = levels
            .windows(2)
            .map(|pair| (f64::from(pair[0]) + f64::from(pair[1])) / 2.0)
            .collect();
        let mut lut = Self {
            levels,
            grid_alpha,
            thresholds,
            table: [0; 256],
        };
        for value in 0..=255usize {
            lut.table[value] = lut.snap(value as f64);
        }
        lut
    }

    #[cfg(test)]
    pub fn levels(&self) -> &[u8] {
        &self.levels
    }

    pub fn snap_u8(&self, value: u8) -> u8 {
        self.table[value as usize]
    }

    pub fn snap_f64(&self, value: f64) -> f64 {
        f64::from(self.snap(value))
    }

    fn snap(&self, value: f64) -> u8 {
        let value = value.clamp(0.0, 255.0);
        let index = match self.grid_alpha {
            Some(alpha) => ((value / alpha).round() as usize).min(self.levels.len() - 1),
            None => self
                .thresholds
                .partition_point(|&threshold| threshold < value),
        };
        self.levels[index]
    }
}

/// The spacing `levels` is a rounding of, if it is one.
///
/// A bit depth's levels are the decimal constants below rounded to bytes.
/// Those constants sit a few digits from the exact `255 / (2^n - 1)`, so
/// snapping through the ratio instead would part company with the original
/// algorithm on the values that land exactly between two of its levels --
/// 127.5 with 5 bits, for one, which the k-means passes reach by averaging
/// the two levels around it. Quantizing on the grid the levels came from
/// keeps every byte of the output identical instead.
fn grid_alpha(levels: &[u8]) -> Option<f64> {
    /// `255 / (2^n - 1)` as the original algorithm writes it, indexed by n.
    const BIT_DEPTH_ALPHA: [f32; 9] = [
        0.0, 255.0, 85.0, 36.42857, 17.0, 8.22581, 4.04762, 2.00787, 1.0,
    ];

    if levels.len() < 2 {
        return None;
    }
    let alpha = if levels.len().is_power_of_two() {
        f64::from(BIT_DEPTH_ALPHA[levels.len().trailing_zeros() as usize])
    } else {
        255.0 / (levels.len() - 1) as f64
    };
    for (index, &level) in levels.iter().enumerate() {
        if f64::from(level) != (index as f64 * alpha).round() {
            return None;
        }
    }
    Some(alpha)
}

/// The three channel tables a color goes through. The target format's alpha
/// levels are not used: this engine treats alpha as a mask, not as a color.
pub struct ColorLut {
    channels: [ChannelLut; 3],
}

impl ColorLut {
    pub fn new(levels: &[Vec<u8>; 4]) -> Self {
        Self {
            channels: [
                ChannelLut::new(&levels[0]),
                ChannelLut::new(&levels[1]),
                ChannelLut::new(&levels[2]),
            ],
        }
    }

    pub fn channel(&self, index: usize) -> &ChannelLut {
        &self.channels[index]
    }

    pub fn snap(&self, color: Rgb) -> Rgb {
        Rgb::new(
            self.channels[0].snap_f64(color.r),
            self.channels[1].snap_f64(color.g),
            self.channels[2].snap_f64(color.b),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The levels a bit depth produces, as the reference implementation's
    /// `-bits_per_chan` does.
    pub fn uniform_levels(bits: u32) -> Vec<u8> {
        let steps = (1u32 << bits) - 1;
        (0..=steps)
            .map(|i| (f64::from(i) * 255.0 / f64::from(steps)).round() as u8)
            .collect()
    }

    /// `round(round(v / alpha) * alpha)` with the reference implementation's
    /// `alpha` table, which is what its `toNbit` computes.
    fn reference_snap(value: f64, bits: usize) -> f64 {
        const ALPHA: [f32; 9] = [
            0.0, 255.0, 85.0, 36.42857, 17.0, 8.22581, 4.04762, 2.00787, 1.0,
        ];
        let alpha = f64::from(ALPHA[bits]);
        ((value / alpha).round() * alpha).round()
    }

    #[test]
    fn a_bit_depth_table_matches_the_reference_rounding_for_every_byte() {
        for bits in 2..=8 {
            let lut = ChannelLut::new(&uniform_levels(bits as u32));
            for value in 0..=255u8 {
                assert_eq!(
                    f64::from(lut.snap_u8(value)),
                    reference_snap(f64::from(value), bits),
                    "{bits} bits, value {value}"
                );
            }
        }
    }

    #[test]
    fn a_bit_depth_table_snaps_values_between_levels_like_the_reference() {
        let lut = ChannelLut::new(&uniform_levels(3));
        // 164 sits exactly between the levels 146 and 182, and the grid puts
        // it on the upper one; nearest-level with ties to the lower would not.
        assert_eq!(lut.snap_f64(164.0), 182.0);
        assert_eq!(lut.snap_f64(163.0), 146.0);
        for value in [163.0, 163.9, 164.0, 236.5, 237.0] {
            assert_eq!(lut.snap_f64(value), reference_snap(value, 3), "{value}");
        }
        // 127.5 is the one value a 5 bit grid places differently from the
        // exact 255/31 ratio, and k-means reaches it by averaging its
        // neighbouring levels 123 and 132.
        let five_bit = ChannelLut::new(&uniform_levels(5));
        assert_eq!(five_bit.snap_f64(127.5), 123.0);
        assert_eq!(five_bit.snap_f64(127.5), reference_snap(127.5, 5));
    }

    #[test]
    fn every_bit_depth_grid_agrees_with_the_reference_on_fractional_values() {
        for bits in 2..=8 {
            let lut = ChannelLut::new(&uniform_levels(bits as u32));
            for step in 0..=2550 {
                let value = f64::from(step) / 10.0;
                assert_eq!(
                    lut.snap_f64(value),
                    reference_snap(value, bits),
                    "{bits} bits, value {value}"
                );
            }
        }
    }

    #[test]
    fn evenly_spaced_levels_that_are_not_a_bit_depth_use_the_exact_spacing() {
        // Six levels, 51 apart.
        let lut = ChannelLut::new(&[0, 51, 102, 153, 204, 255]);
        assert_eq!(lut.snap_u8(25), 0, "a tie takes the lower level");
        assert_eq!(lut.snap_u8(26), 51);
        assert_eq!(lut.snap_f64(25.5), 51.0, "on the grid, a tie rounds up");
    }

    #[test]
    fn uneven_levels_snap_to_the_nearest_with_ties_going_lower() {
        let lut = ChannelLut::new(&[0, 10, 40, 255]);
        assert_eq!(lut.snap_u8(0), 0);
        assert_eq!(lut.snap_u8(4), 0);
        assert_eq!(lut.snap_u8(5), 0, "a tie takes the lower level");
        assert_eq!(lut.snap_u8(6), 10);
        assert_eq!(lut.snap_u8(25), 10, "a tie takes the lower level");
        assert_eq!(lut.snap_u8(26), 40);
        assert_eq!(lut.snap_u8(255), 255);
    }

    #[test]
    fn snapping_is_idempotent_and_stays_inside_the_level_list() {
        for levels in [vec![0, 10, 40, 255], uniform_levels(5), vec![0, 255]] {
            let lut = ChannelLut::new(&levels);
            for value in 0..=255u8 {
                let snapped = lut.snap_u8(value);
                assert!(levels.contains(&snapped));
                assert_eq!(lut.snap_u8(snapped), snapped);
            }
        }
    }

    #[test]
    fn values_outside_the_byte_range_clamp_to_the_end_levels() {
        let lut = ChannelLut::new(&uniform_levels(3));
        assert_eq!(lut.snap_f64(-12.0), 0.0);
        assert_eq!(lut.snap_f64(400.0), 255.0);
    }

    #[test]
    fn an_unsorted_list_with_repeats_is_normalized() {
        let lut = ChannelLut::new(&[255, 0, 128, 0, 255]);
        assert_eq!(lut.levels(), [0, 128, 255]);
    }

    #[test]
    fn an_empty_list_allows_every_byte() {
        let lut = ChannelLut::new(&[]);
        assert_eq!(lut.levels().len(), 256);
        assert_eq!(lut.snap_u8(137), 137);
    }

    #[test]
    fn a_single_level_absorbs_everything() {
        let lut = ChannelLut::new(&[42]);
        assert_eq!(lut.snap_u8(0), 42);
        assert_eq!(lut.snap_f64(255.0), 42.0);
    }

    #[test]
    fn the_color_lut_ignores_the_alpha_levels() {
        let levels = [
            uniform_levels(2),
            uniform_levels(2),
            uniform_levels(2),
            vec![0, 255],
        ];
        let mut other = levels.clone();
        other[3] = vec![0, 128, 255];
        let color = Rgb::new(100.0, 100.0, 100.0);
        assert_eq!(
            ColorLut::new(&levels).snap(color),
            ColorLut::new(&other).snap(color)
        );
    }
}
