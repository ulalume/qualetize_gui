//! The engine's random number generator and the pixel order it drives.
//!
//! The optimization draws pixels through [`RandomShuffle`], which is the only
//! consumer of randomness: with the same seed and the same input the engine
//! produces the same output. The reference implementation reseeds libc's
//! `rand()`, whose sequence is not reproducible across platforms, so the
//! generator here is a private one.

/// Where the shuffle order comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShuffleMode {
    /// Shuffle from a seeded generator.
    Seeded(u32),
    /// Leave the pixels in their natural order, so a run has no random input
    /// at all. Parity tests against the reference implementation use it.
    #[cfg_attr(not(test), allow(dead_code))]
    Fixed,
}

/// xoshiro128\*\*, seeded through splitmix64.
pub struct Rng {
    state: [u32; 4],
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        let mut source = u64::from(seed);
        let low = split_mix_64(&mut source);
        let high = split_mix_64(&mut source);
        let mut state = [
            low as u32,
            (low >> 32) as u32,
            high as u32,
            (high >> 32) as u32,
        ];
        if state == [0; 4] {
            state[0] = 1;
        }
        Self { state }
    }

    pub fn next_u32(&mut self) -> u32 {
        let state = &mut self.state;
        let result = state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let shifted = state[1] << 9;
        state[2] ^= state[0];
        state[3] ^= state[1];
        state[1] ^= state[2];
        state[0] ^= state[3];
        state[2] ^= shifted;
        state[3] = state[3].rotate_left(11);
        result
    }

    /// A value in 0.0..1.0, as the reference implementation's
    /// `rand() / (RAND_MAX + 1.0)`.
    pub fn next_f64(&mut self) -> f64 {
        f64::from(self.next_u32()) / 4294967296.0
    }
}

fn split_mix_64(source: &mut u64) -> u64 {
    *source = source.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *source;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Draws pixel indices in a shuffled order, reshuffling once every index has
/// been handed out.
///
/// The shuffle is the reference implementation's: each position swaps with one
/// drawn from the positions at or after it, which biases the order but is kept
/// so the search behaves the same way.
pub struct RandomShuffle {
    values: Vec<u32>,
    current_index: usize,
    rng: Option<Rng>,
}

impl RandomShuffle {
    pub fn new(count: usize, mode: ShuffleMode) -> Self {
        Self {
            values: (0..count as u32).collect(),
            // The first `next` runs past the end and so always shuffles first.
            current_index: count.saturating_sub(1),
            rng: match mode {
                ShuffleMode::Seeded(seed) => Some(Rng::new(seed)),
                ShuffleMode::Fixed => None,
            },
        }
    }

    /// The next index. Consumes exactly `len` random numbers on the draws that
    /// reshuffle, and none on the others.
    pub fn next(&mut self) -> usize {
        if self.values.is_empty() {
            // No pixel to draw. `run` rejects an image with no opaque pixels
            // before the optimization starts, so this is unreachable there.
            return 0;
        }
        self.current_index += 1;
        if self.current_index >= self.values.len() {
            self.shuffle();
            self.current_index = 0;
        }
        self.values[self.current_index] as usize
    }

    fn shuffle(&mut self) {
        let Some(rng) = self.rng.as_mut() else {
            return;
        };
        let len = self.values.len();
        for i in 0..len {
            let index = i + (rng.next_f64() * (len - i) as f64).floor() as usize;
            self.values.swap(i, index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(count: usize, mode: ShuffleMode, draws: usize) -> Vec<usize> {
        let mut shuffle = RandomShuffle::new(count, mode);
        (0..draws).map(|_| shuffle.next()).collect()
    }

    #[test]
    fn the_fixed_mode_hands_out_the_natural_order() {
        assert_eq!(draw(4, ShuffleMode::Fixed, 9), [0, 1, 2, 3, 0, 1, 2, 3, 0]);
    }

    #[test]
    fn a_seeded_shuffle_is_a_permutation_of_every_index() {
        let mut drawn = draw(64, ShuffleMode::Seeded(7), 64);
        drawn.sort_unstable();
        assert_eq!(drawn, (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn the_same_seed_draws_the_same_order_and_other_seeds_do_not() {
        let first = draw(64, ShuffleMode::Seeded(1), 128);
        assert_eq!(first, draw(64, ShuffleMode::Seeded(1), 128));
        assert_ne!(first, draw(64, ShuffleMode::Seeded(2), 128));
        assert_ne!(first, draw(64, ShuffleMode::Fixed, 128));
    }

    #[test]
    fn a_seeded_shuffle_consumes_one_random_number_per_value() {
        // Eight draws over four values reshuffle twice, four numbers each.
        let mut shuffle = RandomShuffle::new(4, ShuffleMode::Seeded(3));
        for _ in 0..8 {
            shuffle.next();
        }
        let mut expected = Rng::new(3);
        for _ in 0..8 {
            expected.next_u32();
        }
        assert_eq!(
            shuffle.rng.as_mut().unwrap().next_u32(),
            expected.next_u32()
        );
    }

    #[test]
    fn the_swap_partner_never_comes_from_before_the_position() {
        // The biased swap of the original: position `i` can only take a value
        // from `i..len`, so index 0 is as likely to stay put as to move.
        let mut shuffle = RandomShuffle::new(2, ShuffleMode::Seeded(11));
        shuffle.shuffle();
        assert!(shuffle.values == [0, 1] || shuffle.values == [1, 0]);
    }

    #[test]
    fn an_empty_shuffle_does_not_panic() {
        assert_eq!(draw(0, ShuffleMode::Seeded(5), 3), [0, 0, 0]);
    }

    #[test]
    fn the_generator_covers_the_unit_interval_without_reaching_one() {
        let mut rng = Rng::new(0);
        let values: Vec<f64> = (0..1000).map(|_| rng.next_f64()).collect();
        assert!(values.iter().all(|&v| (0.0..1.0).contains(&v)));
        assert!(values.iter().any(|&v| v < 0.1));
        assert!(values.iter().any(|&v| v > 0.9));
    }
}
