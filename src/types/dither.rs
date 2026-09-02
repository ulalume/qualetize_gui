#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum DitherMode {
    None,
    #[default]
    Floyd,
    Atkinson,
    Checker,
    Ord2,
    Ord4,
    Ord8,
    Ord16,
    Ord32,
    Ord64,
}

impl DitherMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            DitherMode::None => "None",
            DitherMode::Floyd => "Floyd-Steinberg",
            DitherMode::Atkinson => "Atkinson",
            DitherMode::Checker => "Checkerboard",
            DitherMode::Ord2 => "2x2 Ordered",
            DitherMode::Ord4 => "4x4 Ordered",
            DitherMode::Ord8 => "8x8 Ordered",
            DitherMode::Ord16 => "16x16 Ordered",
            DitherMode::Ord32 => "32x32 Ordered",
            DitherMode::Ord64 => "64x64 Ordered",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            DitherMode::None => "No dithering",
            DitherMode::Floyd => "Floyd-Steinberg error diffusion (default level: 0.5)",
            DitherMode::Atkinson => "Atkinson error diffusion (default level: 0.5)",
            DitherMode::Checker => "Checkerboard dithering (default level: 1.0)",
            DitherMode::Ord2 => "2x2 ordered dithering (default level: 1.0)",
            DitherMode::Ord4 => "4x4 ordered dithering (default level: 1.0)",
            DitherMode::Ord8 => "8x8 ordered dithering (default level: 1.0)",
            DitherMode::Ord16 => "16x16 ordered dithering (default level: 1.0)",
            DitherMode::Ord32 => "32x32 ordered dithering (default level: 1.0)",
            DitherMode::Ord64 => "64x64 ordered dithering (default level: 1.0)",
        }
    }

    /// Matches the C header's `DITHER_*` constants (`external/qualetize/include/Qualetize.h`):
    /// `DITHER_ORDERED(n)` is `n` itself, with a kernel of size `(2^n) x (2^n)`, so `OrdN`
    /// maps to `log2(N)` rather than to `N`. The CLI's mapping table
    /// (`external/qualetize/source/qualetize-cli.c`) confirms `ord2..ord64` -> `1..=6`.
    pub fn to_id(self) -> u8 {
        match self {
            DitherMode::None => 0,
            DitherMode::Floyd => 0xFE,
            DitherMode::Atkinson => 0xFD,
            DitherMode::Checker => 0xFF,
            DitherMode::Ord2 => 1,
            DitherMode::Ord4 => 2,
            DitherMode::Ord8 => 3,
            DitherMode::Ord16 => 4,
            DitherMode::Ord32 => 5,
            DitherMode::Ord64 => 6,
        }
    }

    pub fn all() -> &'static [DitherMode] {
        &[
            DitherMode::None,
            DitherMode::Floyd,
            DitherMode::Atkinson,
            DitherMode::Checker,
            DitherMode::Ord2,
            DitherMode::Ord4,
            DitherMode::Ord8,
            DitherMode::Ord16,
            DitherMode::Ord32,
            DitherMode::Ord64,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DITHER_ORDERED(n)` in Qualetize.h is `n` itself (kernel `(2^n) x (2^n)`), so
    /// each `OrdN` variant must map to `log2(N)`, not to `N`.
    #[test]
    fn ordered_dither_ids_are_log2_of_the_kernel_size() {
        assert_eq!(DitherMode::Ord2.to_id(), 1);
        assert_eq!(DitherMode::Ord4.to_id(), 2);
        assert_eq!(DitherMode::Ord8.to_id(), 3);
        assert_eq!(DitherMode::Ord16.to_id(), 4);
        assert_eq!(DitherMode::Ord32.to_id(), 5);
        assert_eq!(DitherMode::Ord64.to_id(), 6);
    }

    /// Non-ordered modes are fixed sentinel values defined by the C header.
    #[test]
    fn non_ordered_dither_ids_match_the_header_constants() {
        assert_eq!(DitherMode::None.to_id(), 0);
        assert_eq!(DitherMode::Floyd.to_id(), 0xFE);
        assert_eq!(DitherMode::Atkinson.to_id(), 0xFD);
        assert_eq!(DitherMode::Checker.to_id(), 0xFF);
    }
}
