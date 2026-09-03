use super::color_space::ColorSpace;
use super::dither::DitherMode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::ptr;
use std::sync::LazyLock;

/// The C plan stores the per-channel level count in a `u8`, so a channel can hold
/// at most 255 entries.
pub const MAX_CUSTOM_LEVELS: usize = u8::MAX as usize;

// The C header aligns `Vec4f_t` to 16 only when `__SSE__` is defined, which build.rs
// enables (and mirrors as the `qualetize_sse` cfg) only for x86_64 targets. Key off
// that cfg rather than the target arch directly so the two stay in lockstep.
#[cfg(qualetize_sse)]
#[repr(C, align(16))]
pub struct Vec4f {
    pub f32: [f32; 4],
}

#[cfg(not(qualetize_sse))]
#[repr(C)]
pub struct Vec4f {
    pub f32: [f32; 4],
}

#[repr(C)]
pub struct QualetizePlan {
    pub tile_width: u16,
    pub tile_height: u16,
    pub n_palette_colors: u16,
    pub n_tile_palettes: u16,
    pub colorspace: u8,
    pub first_color_is_transparent: u8,
    pub premultiplied_alpha: u8,
    pub dither_type: u8,
    pub dither_level: f32,
    pub n_tile_cluster_passes: u32,
    pub n_color_cluster_passes: u32,
    pub color_depth: Vec4f,
    pub transparent_color: BGRA8,
    pub custom_levels: [*const f32; 4],
    pub custom_level_count: [u8; 4],
}

unsafe extern "C" {
    pub fn Qualetize(
        output_px_data: *mut u8,
        output_palette: *mut BGRA8,
        input_bitmap: *const BGRA8,
        input_palette: *const BGRA8,
        input_width: u32,
        input_height: u32,
        plan: *const QualetizePlan,
        rmse: *mut Vec4f,
    ) -> u8;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BGRA8 {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ClearColor {
    #[default]
    None,
    Rgb(u8, u8, u8),
}

impl ClearColor {
    pub fn to_bgra8(self) -> BGRA8 {
        match self {
            ClearColor::None => BGRA8 {
                b: 0,
                g: 0,
                r: 0,
                a: 0,
            },
            ClearColor::Rgb(r, g, b) => BGRA8 { b, g, r, a: 0xFF },
        }
    }
}

/// What goes into index 0 of every palette. Shared by both engines; Qualetize
/// has no shared color and runs [`FirstColor::Shared`] as [`FirstColor::Unique`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FirstColor {
    /// An ordinary quantized color, different per palette.
    #[default]
    Unique,
    /// `shared_color` in every palette, kept fixed during optimization.
    Shared,
    /// `transparent_color`, inserted at output; pixels with alpha below 255 map to it.
    TransparentFromAlpha,
    /// `transparent_color`, inserted at output; pixels whose RGB equals it map to it.
    TransparentFromColor,
}

impl FirstColor {
    pub fn description(&self) -> &'static str {
        match self {
            FirstColor::Unique => "Index 0 is a normal color, chosen per palette",
            FirstColor::Shared => {
                "Index 0 of every palette is the same color; Qualetize has no shared color and treats this as Unique"
            }
            FirstColor::TransparentFromAlpha => {
                "Index 0 is transparent; pixels with alpha below 255 map to it"
            }
            FirstColor::TransparentFromColor => {
                "Index 0 is transparent; pixels matching the color beside it map to it"
            }
        }
    }

    /// Whether index 0 is reserved and must stay in place (palette sort,
    /// optimization).
    pub fn pins_index_zero(self) -> bool {
        self != FirstColor::Unique
    }

    /// Whether index 0 is inserted at output rather than optimized.
    pub fn is_transparent(self) -> bool {
        matches!(
            self,
            FirstColor::TransparentFromAlpha | FirstColor::TransparentFromColor
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualetizeSettings {
    pub tile_width: u16,
    pub tile_height: u16,
    pub n_palettes: u16,
    pub n_colors: u16,
    pub rgba_depth: String,
    pub premul_alpha: bool,
    pub color_space: ColorSpace,
    pub dither_mode: DitherMode,
    pub dither_level: f32,
    pub tile_passes: u32,
    pub color_passes: u32,
    /// Both transparent modes of [`FirstColor`]. Serialized on its own since
    /// `.qset` files predate [`FirstColor`].
    pub col0_is_clear: bool,
    /// `Rgb` for [`FirstColor::TransparentFromColor`], `None` otherwise.
    pub clear_color: ClearColor,
    /// [`FirstColor::Shared`]. Only meaningful while `col0_is_clear` is false.
    #[serde(default)]
    pub first_color_shared: bool,
    /// The color index 0 takes in [`FirstColor::Shared`].
    #[serde(default)]
    pub shared_color: [u8; 3],
    /// The key color of [`FirstColor::TransparentFromColor`], kept while
    /// another mode is selected so switching back restores it.
    #[serde(default = "default_transparent_color")]
    pub transparent_color: [u8; 3],
    #[serde(default)]
    pub tile_reduce_post_enabled: bool,
    #[serde(default = "default_tile_reduce_post_threshold")]
    pub tile_reduce_post_threshold: f32,
    #[serde(default = "default_tile_reduce_allow_flip")]
    pub tile_reduce_allow_flip_x: bool,
    #[serde(default = "default_tile_reduce_allow_flip")]
    pub tile_reduce_allow_flip_y: bool,
    #[serde(default)]
    pub use_custom_levels: bool,
    #[serde(default = "default_custom_level_strings")]
    pub custom_levels: [String; 4],
}

#[derive(Default)]
pub enum QualetizePreset {
    #[default]
    Genesis,
    GenesisFullPals,
    GbaNds,
    GbaNdsFullPals,
}

impl QualetizePreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            QualetizePreset::Genesis => "Genesis",
            QualetizePreset::GenesisFullPals => "Genesis (full palettes)",
            QualetizePreset::GbaNds => "GBA/NDS",
            QualetizePreset::GbaNdsFullPals => "GBA/NDS (full palettes)",
        }
    }

    pub fn all() -> &'static [QualetizePreset] {
        &[
            QualetizePreset::Genesis,
            QualetizePreset::GenesisFullPals,
            QualetizePreset::GbaNds,
            QualetizePreset::GbaNdsFullPals,
        ]
    }

    pub fn qualetize_settings(&self) -> QualetizeSettings {
        match self {
            QualetizePreset::Genesis => QualetizeSettings::genesis(),
            QualetizePreset::GenesisFullPals => QualetizeSettings::genesis_full_palettes(),
            QualetizePreset::GbaNds => QualetizeSettings::gba_nds(),
            QualetizePreset::GbaNdsFullPals => QualetizeSettings::gba_nds_full_palettes(),
        }
    }
}

impl QualetizeSettings {
    /// Replace the quantization settings with `preset`, leaving the tile
    /// reduction post-pass alone.
    ///
    /// Tile reduction only lives in this struct because it is serialized
    /// alongside; it is a separate stage with its own reset, so a quantization
    /// preset should no more disturb it than it does color correction.
    pub fn apply_preset(&mut self, preset: QualetizeSettings) {
        *self = QualetizeSettings {
            tile_reduce_post_enabled: self.tile_reduce_post_enabled,
            tile_reduce_post_threshold: self.tile_reduce_post_threshold,
            tile_reduce_allow_flip_x: self.tile_reduce_allow_flip_x,
            tile_reduce_allow_flip_y: self.tile_reduce_allow_flip_y,
            ..preset
        };
    }

    /// Restore the tile reduction post-pass to its defaults, leaving the
    /// quantization settings alone. The enable flag is kept, the same way a
    /// color correction preset does not switch that section off.
    pub fn reset_tile_reduce(&mut self) {
        self.tile_reduce_post_threshold = default_tile_reduce_post_threshold();
        self.tile_reduce_allow_flip_x = default_tile_reduce_allow_flip();
        self.tile_reduce_allow_flip_y = default_tile_reduce_allow_flip();
    }

    /// Identical to [`Self::genesis`] apart from the pixel format and color space;
    /// expressed as a struct update over it so the two presets can't drift apart on
    /// the fields they share.
    pub fn gba_nds() -> Self {
        let rgba_depth = "5551".to_string();
        let custom_levels = default_level_strings_from_depth(&rgba_depth);
        Self {
            rgba_depth,
            color_space: ColorSpace::YcbcrPsy,
            use_custom_levels: false,
            custom_levels,
            ..Self::genesis()
        }
    }
    pub fn gba_nds_full_palettes() -> Self {
        Self {
            n_palettes: 16,
            col0_is_clear: true,
            ..Self::gba_nds()
        }
    }
    pub fn genesis() -> Self {
        let rgba_depth = "3331".to_string();
        Self {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 1,
            n_colors: 16,
            rgba_depth: rgba_depth.clone(),
            premul_alpha: false,
            color_space: ColorSpace::default(),
            dither_mode: DitherMode::None,
            dither_level: 0.5,
            tile_passes: 1000,
            color_passes: 100,
            col0_is_clear: false,
            clear_color: ClearColor::default(),
            first_color_shared: false,
            shared_color: [0, 0, 0],
            transparent_color: default_transparent_color(),
            tile_reduce_post_enabled: false,
            tile_reduce_post_threshold: default_tile_reduce_post_threshold(),
            tile_reduce_allow_flip_x: default_tile_reduce_allow_flip(),
            tile_reduce_allow_flip_y: default_tile_reduce_allow_flip(),
            use_custom_levels: true,
            custom_levels: genesis_custom_level_strings(),
        }
    }
    pub fn genesis_full_palettes() -> Self {
        Self {
            n_palettes: 4,
            col0_is_clear: true,
            ..Self::genesis()
        }
    }

    /// What index 0 of every palette holds, read back from the three fields
    /// that store it.
    pub fn first_color(&self) -> FirstColor {
        match (self.col0_is_clear, self.clear_color) {
            (true, ClearColor::Rgb(..)) => FirstColor::TransparentFromColor,
            (true, ClearColor::None) => FirstColor::TransparentFromAlpha,
            (false, _) if self.first_color_shared => FirstColor::Shared,
            (false, _) => FirstColor::Unique,
        }
    }

    /// Write `mode` into the three fields that store it. The key color is left
    /// alone, so switching away from [`FirstColor::TransparentFromColor`] and
    /// back restores it.
    pub fn set_first_color(&mut self, mode: FirstColor) {
        self.col0_is_clear = mode.is_transparent();
        self.first_color_shared = mode == FirstColor::Shared;
        self.clear_color = match mode {
            FirstColor::TransparentFromColor => {
                let [r, g, b] = self.transparent_color;
                ClearColor::Rgb(r, g, b)
            }
            _ => ClearColor::None,
        };
    }

    /// Set the key color, which [`FirstColor::TransparentFromColor`] also
    /// carries in `clear_color`.
    pub fn set_transparent_color(&mut self, rgb: [u8; 3]) {
        self.transparent_color = rgb;
        if self.first_color() == FirstColor::TransparentFromColor {
            self.clear_color = ClearColor::Rgb(rgb[0], rgb[1], rgb[2]);
        }
    }

    /// The allowed values of each channel (R, G, B, A): the custom level
    /// lists when enabled, otherwise the uniform levels of `rgba_depth`.
    /// A malformed custom list falls back to the depth for that channel.
    pub fn channel_levels(&self) -> [Vec<u8>; 4] {
        let from_depth = default_level_strings_from_depth(&self.rgba_depth);
        std::array::from_fn(|i| {
            let custom = self
                .use_custom_levels
                .then(|| parse_levels_u8(&self.custom_levels[i]))
                .flatten();
            custom.unwrap_or_else(|| {
                parse_levels_u8(&from_depth[i]).expect("generated level strings are valid")
            })
        })
    }

    /// Clamp settings loaded from disk into the ranges the UI enforces (see the
    /// `DragValue`/`Slider` ranges in `src/ui/settings_panel.rs`).
    ///
    /// A hand-edited or corrupted `.qset` can otherwise carry values the UI would
    /// never produce, and those reach the C `Qualetize` library unchecked: a
    /// `tile_width` of 0 is an integer divide by zero there, and an oversized
    /// `n_colors * n_palettes` product overruns the output palette buffer.
    pub fn sanitize(&mut self) {
        self.tile_width = self.tile_width.clamp(1, 64);
        self.tile_height = self.tile_height.clamp(1, 64);
        self.n_colors = self.n_colors.clamp(1, 256);
        let max_palettes = 256 / self.n_colors;
        self.n_palettes = self.n_palettes.clamp(1, max_palettes);
        self.tile_passes = self.tile_passes.clamp(0, 1000);
        self.color_passes = self.color_passes.clamp(0, 100);

        if !self.dither_level.is_finite() {
            self.dither_level = 0.5;
        }
        self.dither_level = self.dither_level.clamp(0.0, 2.0);

        if !self.tile_reduce_post_threshold.is_finite() || self.tile_reduce_post_threshold < 0.0 {
            self.tile_reduce_post_threshold = default_tile_reduce_post_threshold();
        }

        if !is_valid_rgba_depth(&self.rgba_depth) {
            self.rgba_depth = DEFAULT_RGBA_DEPTH.to_string();
        }

        // The three first color fields describe one setting, and only one
        // combination of them stands for each [`FirstColor`]: `clear_color`
        // holds a color only while index 0 is transparent, and the shared flag
        // is set only while it is not.
        if self.col0_is_clear {
            self.first_color_shared = false;
        } else if let ClearColor::Rgb(r, g, b) = self.clear_color {
            self.transparent_color = [r, g, b];
            self.clear_color = ClearColor::None;
        }
    }
}

/// A valid RGBA depth string is exactly 4 characters, each a digit `1'..='8'`
/// (bits per channel); anything else can't be turned into a sane [`Vec4f`] depth.
fn is_valid_rgba_depth(rgba_depth: &str) -> bool {
    let chars: Vec<char> = rgba_depth.chars().collect();
    chars.len() == 4 && chars.iter().all(|c| ('1'..='8').contains(c))
}

impl Default for QualetizeSettings {
    fn default() -> Self {
        Self::genesis()
    }
}

/// A digit `d` in `1..=8` means `d` bits per channel, i.e. `2^d - 1` levels; anything
/// else (including non-digit characters) falls back to a full 8-bit channel.
fn char_to_depth(c: char) -> f32 {
    match c.to_digit(10) {
        Some(d @ 1..=8) => ((1u32 << d) - 1) as f32,
        _ => 255.0,
    }
}

fn default_transparent_color() -> [u8; 3] {
    [255, 0, 255]
}

fn default_tile_reduce_post_threshold() -> f32 {
    25.0
}

fn default_tile_reduce_allow_flip() -> bool {
    true
}

/// Parses a 4-character RGBA depth string into per-channel level counts.
///
/// Counts *characters*, not bytes: a non-ASCII string can have `len() == 4` in bytes
/// while holding fewer than 4 `char`s (or vice versa), and indexing a `Vec<char>`
/// built from the wrong count would panic.
fn parse_rgba_depth(rgba_depth: &str) -> [f32; 4] {
    let chars: Vec<char> = rgba_depth.chars().collect();
    if let [a, b, c, d] = chars[..] {
        [
            char_to_depth(a),
            char_to_depth(b),
            char_to_depth(c),
            char_to_depth(d),
        ]
    } else {
        [255.0, 255.0, 255.0, 255.0] // Default to 8-bit
    }
}

const DEFAULT_RGBA_DEPTH: &str = "3331";

fn depth_to_levels(depth: f32) -> Vec<u8> {
    let clamped_depth = depth.clamp(1.0, 254.0);
    let steps = clamped_depth.round() as u32;
    (0..=steps)
        .map(|i| ((i as f32 / clamped_depth) * 255.0).round() as u8)
        .collect()
}

fn levels_to_string(levels: Vec<u8>) -> String {
    levels
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join(",")
}

pub(crate) fn default_level_strings_from_depth(rgba_depth: &str) -> [String; 4] {
    let depth = parse_rgba_depth(rgba_depth);
    [
        levels_to_string(depth_to_levels(depth[0])),
        levels_to_string(depth_to_levels(depth[1])),
        levels_to_string(depth_to_levels(depth[2])),
        levels_to_string(depth_to_levels(depth[3])),
    ]
}

fn genesis_custom_level_strings() -> [String; 4] {
    [
        "0,49,87,119,146,174,206,255".to_string(),
        "0,49,87,119,146,174,206,255".to_string(),
        "0,49,87,119,146,174,206,255".to_string(),
        "0,255".to_string(),
    ]
}

fn default_custom_level_strings() -> [String; 4] {
    default_level_strings_from_depth(DEFAULT_RGBA_DEPTH)
}

/// Comma separated list of 0-255 integers, without leading zeros or whitespace.
static LEVEL_LIST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9][0-9]|[0-9])(,(25[0-5]|2[0-4][0-9]|1[0-9]{2}|[0-9]|[1-9][0-9]))*$")
        .expect("level list regex is valid")
});

pub(crate) fn validate_0_255_array(array_str: &str) -> bool {
    if array_str.is_empty() {
        return false;
    }

    if !LEVEL_LIST_RE.is_match(array_str) {
        return false;
    }

    array_str.split(',').count() <= MAX_CUSTOM_LEVELS
}

/// Sorted 0-255 levels from a comma separated list, or `None` when the
/// list is malformed.
pub(crate) fn parse_levels_u8(array_str: &str) -> Option<Vec<u8>> {
    if !validate_0_255_array(array_str) {
        return None;
    }
    let mut values: Vec<u8> = array_str
        .split(',')
        .filter_map(|s| s.parse::<u8>().ok())
        .collect();
    values.sort_unstable();
    Some(values)
}

/// The levels normalized to 0.0..=1.0, as the C plan takes them.
fn parse_custom_levels(array_str: &str) -> Option<Vec<f32>> {
    let levels = parse_levels_u8(array_str)?;
    Some(levels.iter().map(|&v| v as f32 / 255.0).collect())
}

pub struct QualetizePlanOwned {
    // Private: `plan.custom_levels` points into `custom_level_storage` below, so
    // handing the plan out by value would let it outlive the storage that backs
    // its pointers. Only borrow it through `as_ptr`.
    plan: QualetizePlan,
    // Never read directly: it exists purely to keep the boxed slices `plan`
    // points into alive for as long as `self` is. Dropping them early would
    // dangle `plan.custom_levels`.
    #[allow(dead_code)]
    custom_level_storage: [Option<Box<[f32]>>; 4],
}

impl QualetizePlanOwned {
    /// Invariant: `plan.custom_levels` points into the boxed slices held in
    /// `custom_level_storage`, which live exactly as long as `self`. Callers must
    /// not use the returned pointer beyond `self`'s lifetime.
    pub fn as_ptr(&self) -> *const QualetizePlan {
        &self.plan
    }
}

impl From<QualetizeSettings> for QualetizePlanOwned {
    fn from(settings: QualetizeSettings) -> Self {
        let rgba_depth = parse_rgba_depth(&settings.rgba_depth);
        let mut plan = QualetizePlan {
            tile_width: settings.tile_width,
            tile_height: settings.tile_height,
            n_palette_colors: settings.n_colors,
            n_tile_palettes: settings.n_palettes,
            colorspace: settings.color_space.to_id(),
            first_color_is_transparent: if settings.col0_is_clear { 1 } else { 0 },
            premultiplied_alpha: if settings.premul_alpha { 1 } else { 0 },
            dither_type: settings.dither_mode.to_id(),
            dither_level: settings.dither_level,
            n_tile_cluster_passes: settings.tile_passes,
            n_color_cluster_passes: settings.color_passes,
            color_depth: Vec4f {
                f32: [rgba_depth[0], rgba_depth[1], rgba_depth[2], rgba_depth[3]],
            },
            transparent_color: settings.clear_color.to_bgra8(),
            custom_levels: [ptr::null(); 4],
            custom_level_count: [0; 4],
        };

        let mut custom_level_storage: [Option<Box<[f32]>>; 4] = [None, None, None, None];
        if settings.use_custom_levels {
            for (idx, level_str) in settings.custom_levels.iter().enumerate() {
                if let Some(levels) = parse_custom_levels(level_str)
                    && let Ok(len) = u8::try_from(levels.len())
                {
                    let boxed = levels.into_boxed_slice();
                    plan.custom_levels[idx] = boxed.as_ptr();
                    plan.custom_level_count[idx] = len;
                    custom_level_storage[idx] = Some(boxed);
                }
            }
        }

        Self {
            plan,
            custom_level_storage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checking `rgba_depth.len() == 4` would check bytes, not characters, so a
    /// 4-byte string that isn't 4 characters (e.g. containing multi-byte UTF-8)
    /// would pass that check and then panic indexing a shorter `Vec<char>`.
    #[test]
    fn parse_rgba_depth_does_not_panic_on_non_ascii_input() {
        // "é" is 2 bytes but 1 char: "é331" is 5 bytes / 4 chars, "éé31" is 6 bytes
        // / 4 chars, and "é31" is 4 bytes / 3 chars. None of these should panic, and
        // none is a valid 4-digit depth string other than the 4-char ones, which
        // fall back to the "not a digit" branch of `char_to_depth` for 'é'.
        assert_eq!(parse_rgba_depth("é31"), [255.0, 255.0, 255.0, 255.0]);
        assert_eq!(
            parse_rgba_depth("éé31"),
            [255.0, 255.0, char_to_depth('3'), char_to_depth('1')]
        );
        assert_eq!(parse_rgba_depth("é3331"), [255.0, 255.0, 255.0, 255.0]);
    }

    /// `char_to_depth`'s formula (`2^d - 1` for a digit `d` in `1..=8`) must agree
    /// with the explicit per-digit values below.
    #[test]
    fn char_to_depth_formula_matches_the_digit_table() {
        let table = [
            ('1', 1.0),
            ('2', 3.0),
            ('3', 7.0),
            ('4', 15.0),
            ('5', 31.0),
            ('6', 63.0),
            ('7', 127.0),
            ('8', 255.0),
        ];
        for (c, expected) in table {
            assert_eq!(char_to_depth(c), expected, "digit {c}");
        }
        // Out of range or non-digit characters still fall back to 8-bit.
        assert_eq!(char_to_depth('0'), 255.0);
        assert_eq!(char_to_depth('9'), 255.0);
        assert_eq!(char_to_depth('x'), 255.0);
    }

    /// The C library divides by `tile_width`/`tile_height` and writes
    /// `n_colors * n_palettes` palette entries, so out-of-range values loaded from a
    /// hand-edited `.qset` must be clamped rather than passed straight through.
    #[test]
    fn sanitize_clamps_out_of_range_values_loaded_from_disk() {
        let mut settings = QualetizeSettings::genesis();
        settings.tile_width = 0;
        settings.tile_height = 1000;
        settings.n_colors = 0;
        settings.n_palettes = u16::MAX;
        settings.tile_passes = u32::MAX;
        settings.color_passes = u32::MAX;
        settings.dither_level = f32::NAN;
        settings.tile_reduce_post_threshold = -5.0;
        settings.rgba_depth = "9a3!".to_string();

        settings.sanitize();

        assert!((1..=64).contains(&settings.tile_width));
        assert!((1..=64).contains(&settings.tile_height));
        assert!((1..=256).contains(&settings.n_colors));
        assert!(settings.n_colors as u32 * settings.n_palettes as u32 <= 256);
        assert!(settings.tile_passes <= 1000);
        assert!(settings.color_passes <= 100);
        assert_eq!(settings.dither_level, 0.5);
        assert!(settings.tile_reduce_post_threshold >= 0.0);
        assert_eq!(settings.rgba_depth, DEFAULT_RGBA_DEPTH);
    }

    /// A well-formed settings struct (e.g. any preset) must survive `sanitize`
    /// unchanged.
    #[test]
    fn sanitize_leaves_in_range_values_alone() {
        let mut settings = QualetizeSettings::genesis();
        let before = settings.clone();

        settings.sanitize();

        assert_eq!(settings, before);
    }

    #[test]
    fn validate_0_255_array_accepts_well_formed_lists() {
        assert!(validate_0_255_array("0"));
        assert!(validate_0_255_array("0,255"));
        assert!(validate_0_255_array("0,49,87,119,146,174,206,255"));
    }

    #[test]
    fn validate_0_255_array_rejects_malformed_lists() {
        assert!(!validate_0_255_array(""), "empty");
        assert!(!validate_0_255_array("256"), "out of range");
        assert!(!validate_0_255_array("0, 255"), "whitespace");
        assert!(!validate_0_255_array("0,"), "trailing comma");
        assert!(!validate_0_255_array("00"), "leading zero");
        assert!(!validate_0_255_array("0;255"), "wrong separator");
    }

    #[test]
    fn validate_0_255_array_rejects_more_entries_than_the_plan_can_hold() {
        let at_limit = vec!["1"; MAX_CUSTOM_LEVELS].join(",");
        let over_limit = vec!["1"; MAX_CUSTOM_LEVELS + 1].join(",");
        assert!(validate_0_255_array(&at_limit));
        assert!(!validate_0_255_array(&over_limit));
    }

    #[test]
    fn parse_custom_levels_normalizes_and_sorts() {
        let levels = parse_custom_levels("255,0,128").expect("valid list");
        assert_eq!(levels.len(), 3);
        assert!(levels[0] < levels[1] && levels[1] < levels[2]);
        assert_eq!(levels[0], 0.0);
        assert_eq!(levels[2], 1.0);
    }

    #[test]
    fn channel_levels_follow_the_depth_unless_custom_levels_are_on() {
        let mut settings = QualetizeSettings::gba_nds();
        assert!(!settings.use_custom_levels);
        let levels = settings.channel_levels();
        assert_eq!(levels[0].len(), 32, "5 bits");
        assert_eq!(levels[3], vec![0, 255], "1 bit alpha");

        settings.use_custom_levels = true;
        settings.custom_levels[0] = "0,128,255".to_string();
        settings.custom_levels[1] = "not a list".to_string();
        let levels = settings.channel_levels();
        assert_eq!(levels[0], vec![0, 128, 255]);
        assert_eq!(
            levels[1].len(),
            32,
            "malformed list falls back to the depth"
        );
    }

    #[test]
    fn parse_custom_levels_rejects_invalid_input() {
        assert!(parse_custom_levels("0,256").is_none());
        assert!(parse_custom_levels("").is_none());
    }

    /// A full 8-bit channel needs 256 steps but the plan stores the count in a `u8`,
    /// so `depth_to_levels` clamps to 254 steps (255 entries) and stays valid.
    #[test]
    fn generated_levels_always_fit_the_plan() {
        for depth in ["3331", "5551", "8888"] {
            for (channel, levels) in default_level_strings_from_depth(depth).iter().enumerate() {
                let count = levels.split(',').count();
                assert!(
                    count <= MAX_CUSTOM_LEVELS,
                    "{depth} channel {channel} produced {count} levels"
                );
                assert!(
                    validate_0_255_array(levels),
                    "{depth} channel {channel} is not accepted by its own validator"
                );
                assert!(u8::try_from(count).is_ok());
            }
        }
    }

    /// Applying a quantization preset would otherwise silently reset the tile
    /// reduction post-pass along with it.
    #[test]
    fn applying_a_preset_leaves_tile_reduction_alone() {
        let mut settings = QualetizeSettings::genesis();
        settings.tile_reduce_post_enabled = true;
        settings.tile_reduce_post_threshold = 123.0;
        settings.tile_reduce_allow_flip_x = false;
        settings.tile_reduce_allow_flip_y = false;

        settings.apply_preset(QualetizeSettings::gba_nds());

        assert_eq!(settings.color_space, ColorSpace::YcbcrPsy, "preset applied");
        assert_eq!(settings.rgba_depth, "5551", "preset applied");
        assert!(settings.tile_reduce_post_enabled, "tile reduction kept");
        assert_eq!(settings.tile_reduce_post_threshold, 123.0);
        assert!(!settings.tile_reduce_allow_flip_x);
        assert!(!settings.tile_reduce_allow_flip_y);
    }

    #[test]
    fn resetting_tile_reduction_leaves_quantization_alone() {
        let mut settings = QualetizeSettings::gba_nds();
        settings.n_palettes = 7;
        settings.tile_reduce_post_enabled = true;
        settings.tile_reduce_post_threshold = 123.0;
        settings.tile_reduce_allow_flip_x = false;

        settings.reset_tile_reduce();

        assert_eq!(settings.n_palettes, 7, "quantization untouched");
        assert_eq!(settings.color_space, ColorSpace::YcbcrPsy);
        assert_eq!(
            settings.tile_reduce_post_threshold,
            default_tile_reduce_post_threshold()
        );
        assert!(settings.tile_reduce_allow_flip_x);
        assert!(
            settings.tile_reduce_post_enabled,
            "reset restores values, not the enable flag"
        );
    }

    #[test]
    fn genesis_preset_uses_the_documented_levels() {
        let settings = QualetizeSettings::genesis();
        assert!(settings.use_custom_levels);
        assert_eq!(settings.custom_levels[0], "0,49,87,119,146,174,206,255");
        assert_eq!(settings.custom_levels[3], "0,255");
    }

    const ALL_FIRST_COLORS: [FirstColor; 4] = [
        FirstColor::Unique,
        FirstColor::Shared,
        FirstColor::TransparentFromAlpha,
        FirstColor::TransparentFromColor,
    ];

    /// Index 0 is reserved in every mode but `Unique`, and inserted at output
    /// only in the two transparent ones.
    #[test]
    fn only_unique_leaves_index_zero_free() {
        assert!(!FirstColor::Unique.pins_index_zero());
        assert!(FirstColor::Shared.pins_index_zero());
        assert!(!FirstColor::Unique.is_transparent());
        assert!(!FirstColor::Shared.is_transparent());
        assert!(FirstColor::TransparentFromAlpha.is_transparent());
        assert!(FirstColor::TransparentFromColor.is_transparent());
    }

    #[test]
    fn every_first_color_mode_survives_a_write_and_a_read_back() {
        let mut settings = QualetizeSettings::genesis();
        settings.transparent_color = [1, 2, 3];
        for mode in ALL_FIRST_COLORS {
            settings.set_first_color(mode);
            assert_eq!(settings.first_color(), mode);
            assert_eq!(settings.col0_is_clear, mode.is_transparent(), "{mode:?}");
            assert_eq!(
                settings.first_color_shared,
                mode == FirstColor::Shared,
                "{mode:?}"
            );
            assert_eq!(
                settings.transparent_color,
                [1, 2, 3],
                "{mode:?} leaves the key color alone"
            );
        }
    }

    /// The key color is what `TransparentFromColor` puts into `clear_color`,
    /// so a round trip through another mode has to come back to it.
    #[test]
    fn the_key_color_survives_a_trip_through_another_mode() {
        let mut settings = QualetizeSettings::genesis();
        settings.set_first_color(FirstColor::TransparentFromColor);
        settings.set_transparent_color([10, 20, 30]);
        assert_eq!(settings.clear_color, ClearColor::Rgb(10, 20, 30));

        settings.set_first_color(FirstColor::Unique);
        assert_eq!(settings.clear_color, ClearColor::None);
        assert_eq!(settings.transparent_color, [10, 20, 30]);

        settings.set_first_color(FirstColor::TransparentFromColor);
        assert_eq!(settings.clear_color, ClearColor::Rgb(10, 20, 30));
    }

    /// Setting the key color outside `TransparentFromColor` records it without
    /// turning transparency on.
    #[test]
    fn setting_the_key_color_in_another_mode_leaves_the_mode_alone() {
        let mut settings = QualetizeSettings::genesis();
        settings.set_first_color(FirstColor::TransparentFromAlpha);
        settings.set_transparent_color([9, 9, 9]);
        assert_eq!(settings.first_color(), FirstColor::TransparentFromAlpha);
        assert_eq!(settings.clear_color, ClearColor::None);
        assert_eq!(settings.transparent_color, [9, 9, 9]);
    }

    /// A `.qset` written before the three new fields existed carries only
    /// `col0_is_clear` and `clear_color`, and has to load to the mode those two
    /// described on their own.
    #[test]
    fn an_old_settings_file_loads_to_the_mode_its_two_fields_describe() {
        let load = |col0_is_clear: bool, clear_color: &str| {
            let json = format!(
                r#"{{"tile_width": 8, "tile_height": 8, "n_palettes": 1, "n_colors": 16,
                     "rgba_depth": "3331", "premul_alpha": false, "color_space": "RgbLinear",
                     "dither_mode": "None", "dither_level": 0.5, "tile_passes": 1000,
                     "color_passes": 100, "col0_is_clear": {col0_is_clear},
                     "clear_color": {clear_color}}}"#
            );
            serde_json::from_str::<QualetizeSettings>(&json).expect("loads")
        };

        let unique = load(false, r#""None""#);
        assert_eq!(unique.first_color(), FirstColor::Unique);
        assert!(!unique.first_color_shared);
        assert_eq!(
            unique.transparent_color,
            default_transparent_color(),
            "the key color falls back to its default"
        );

        assert_eq!(
            load(true, r#""None""#).first_color(),
            FirstColor::TransparentFromAlpha
        );
        assert_eq!(
            load(true, r#"{"Rgb": [255, 0, 255]}"#).first_color(),
            FirstColor::TransparentFromColor
        );
    }

    /// Only one combination of the three stored fields stands for each mode, so
    /// `sanitize` folds a hand-edited file back onto one of them.
    #[test]
    fn sanitize_normalizes_the_first_color_fields() {
        // A key color while index 0 is not transparent belongs in the field
        // that keeps it for later.
        let mut settings = QualetizeSettings::genesis();
        settings.col0_is_clear = false;
        settings.clear_color = ClearColor::Rgb(1, 2, 3);
        settings.sanitize();
        assert_eq!(settings.clear_color, ClearColor::None);
        assert_eq!(settings.transparent_color, [1, 2, 3]);
        assert_eq!(settings.first_color(), FirstColor::Unique);

        // A transparent index 0 leaves no room for a shared color.
        let mut settings = QualetizeSettings::genesis();
        settings.col0_is_clear = true;
        settings.first_color_shared = true;
        settings.sanitize();
        assert!(!settings.first_color_shared);
        assert_eq!(settings.first_color(), FirstColor::TransparentFromAlpha);
    }

    /// Every mode is already normalized, so writing one and sanitizing is a
    /// no-op.
    #[test]
    fn sanitize_leaves_every_first_color_mode_alone() {
        for mode in ALL_FIRST_COLORS {
            let mut settings = QualetizeSettings::genesis();
            settings.set_first_color(mode);
            let before = settings.clone();
            settings.sanitize();
            assert_eq!(settings, before, "{mode:?}");
        }
    }

    /// A preset replaces the first color settings along with the rest, so
    /// applying one drops a shared color picked before it.
    #[test]
    fn applying_a_preset_resets_the_first_color() {
        let mut settings = QualetizeSettings::genesis();
        settings.set_first_color(FirstColor::Shared);
        settings.shared_color = [1, 2, 3];
        settings.set_transparent_color([4, 5, 6]);

        settings.apply_preset(QualetizeSettings::genesis_full_palettes());

        assert_eq!(settings.first_color(), FirstColor::TransparentFromAlpha);
        assert!(!settings.first_color_shared);
        assert_eq!(settings.shared_color, [0, 0, 0]);
        assert_eq!(settings.transparent_color, default_transparent_color());
    }

    #[test]
    fn full_palette_presets_only_change_palette_layout() {
        let base = QualetizeSettings::genesis();
        let full = QualetizeSettings::genesis_full_palettes();
        assert_eq!(full.n_palettes, 4);
        assert!(full.col0_is_clear);
        assert_eq!(full.rgba_depth, base.rgba_depth);
        assert_eq!(full.custom_levels, base.custom_levels);
    }
}
