//! Rust port of tiledpalettequant (rilden; C++ port by bbbbbr), MIT.
//! Specification: `.doc/tilepalquant-port-spec.md`.
//!
//! The engine picks `n_palettes` palettes of `n_colors` colors and assigns one
//! palette to every tile, by moving palette colors towards randomly drawn
//! pixels and repairing the entries that earn the least.

pub mod color;
pub mod dither;
pub mod levels;
pub mod output;
pub mod palette;
pub mod rng;
pub mod tile;

#[cfg(test)]
mod parity;

use super::{Progress, QuantizeResult, RunContext, TargetFormat};
use crate::types::FirstColor;
use crate::types::tilepalquant::{TpqDitherMode, TpqSettings};
use color::Rgb;
use levels::ColorLut;
use output::quantize_tiles;
use palette::{
    expand_by_one_color, k_means, mean_square_error, move_palettes_closer, quantize_1_color,
    reduce_palettes, replace_weakest_colors,
};
use rng::{RandomShuffle, ShuffleMode};
use tile::{SourceImage, Transparency, extract_all_pixels, extract_tiles};

/// How many rounds of [`replace_weakest_colors`] the optimization runs.
const REPLACE_ITERATIONS: i64 = 10;
/// How much better an alternative has to be before an entry is replaced.
const MIN_COLOR_FACTOR: f32 = 0.5;
const MIN_PALETTE_FACTOR: f32 = 0.5;
/// How many k-means passes finish a run without dithering.
const K_MEANS_PASSES: i64 = 3;

/// Everything derived from the settings that the passes need, resolved once.
pub struct Params {
    pub n_palettes: usize,
    pub colors_per_palette: usize,
    pub tile_width: u32,
    pub tile_height: u32,
    pub dither: TpqDitherMode,
    pub dither_pattern: [[u8; 2]; 2],
    pub dither_pixels: usize,
    pub dither_weight: f32,
    pub first_color: FirstColor,
    /// The color index 0 stands for, unsnapped. `Unique` does not use it.
    pub first_color_value: Rgb,
    pub lut: ColorLut,
    /// Pixel draws per optimization step.
    pub iterations: i32,
    pub alpha: f32,
    pub final_alpha: f32,
    pub show_progress: bool,
}

impl Params {
    /// The index the shared color occupies, and which the optimization must
    /// therefore leave alone.
    pub fn shared_color_index(&self) -> Option<usize> {
        (self.first_color == FirstColor::Shared).then_some(0)
    }

    /// How many entries the output palette has ahead of the optimized colors:
    /// the transparent modes insert one, the others none.
    pub fn adjusted_index(&self) -> usize {
        usize::from(self.first_color.is_transparent())
    }

    pub fn transparency(&self) -> Transparency {
        match self.first_color {
            FirstColor::TransparentFromAlpha => Transparency::FromAlpha,
            FirstColor::TransparentFromColor => Transparency::FromColor(self.first_color_value),
            FirstColor::Unique | FirstColor::Shared => Transparency::None,
        }
    }

    /// Which dither candidate the pixel at `x`, `y` takes.
    pub fn dither_index(&self, x: u32, y: u32) -> usize {
        self.dither_pattern[(x & 1) as usize][(y & 1) as usize] as usize
    }
}

/// Quantize `rgba_data` into `target.n_palettes` palettes of
/// `target.n_colors` colors on the tile grid. Returns `None` when cancelled.
pub fn run(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    target: &TargetFormat,
    settings: &TpqSettings,
    ctx: &RunContext,
) -> Option<Result<QuantizeResult, String>> {
    run_with_shuffle(
        rgba_data,
        width,
        height,
        target,
        settings,
        ctx,
        ShuffleMode::Seeded(settings.rand_seed),
    )
}

/// [`run`], with the pixel order chosen by the caller.
///
/// [`ShuffleMode::Fixed`] takes all randomness out of a run, which is how the
/// parity tests compare against the reference implementation.
#[doc(hidden)]
pub fn run_with_shuffle(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    target: &TargetFormat,
    settings: &TpqSettings,
    ctx: &RunContext,
    shuffle_mode: ShuffleMode,
) -> Option<Result<QuantizeResult, String>> {
    if let Err(message) = validate(rgba_data, width, height, target) {
        return Some(Err(message));
    }

    let first_color_value = match target.first_color {
        FirstColor::Unique => Rgb::default(),
        FirstColor::Shared => Rgb::from_u8(target.shared_color),
        // Both transparent modes take the transparent color; the original
        // reads the shared color for neither of them.
        FirstColor::TransparentFromAlpha | FirstColor::TransparentFromColor => {
            Rgb::from_u8(target.transparent_color)
        }
    };
    let lut = ColorLut::new(&target.levels);

    // Without dithering the image is snapped to the target levels up front, so
    // the tiles hold only colors the output can reproduce. Alpha is left as it
    // is: this engine treats it as a mask, not as a color.
    let mut data = rgba_data.to_vec();
    if settings.dither_mode == TpqDitherMode::Off {
        for pixel in data.as_chunks_mut::<4>().0 {
            for (channel, value) in pixel[..3].iter_mut().enumerate() {
                *value = lut.channel(channel).snap_u8(*value);
            }
        }
    }
    let image = SourceImage::new(data, width, height);

    let mut params = Params {
        n_palettes: target.n_palettes as usize,
        colors_per_palette: target.n_colors as usize,
        tile_width: u32::from(target.tile_width),
        tile_height: u32::from(target.tile_height),
        dither: settings.dither_mode,
        dither_pattern: settings.dither_pattern.matrix(),
        dither_pixels: settings.dither_pattern.candidates(),
        dither_weight: settings.dither_weight,
        first_color: target.first_color,
        first_color_value,
        lut,
        iterations: 0,
        alpha: 0.3,
        final_alpha: 0.05,
        show_progress: settings.show_progress,
    };

    let tiles = extract_tiles(
        &image,
        params.tile_width,
        params.tile_height,
        params.transparency(),
    );
    let pixels = extract_all_pixels(&tiles);
    if pixels.is_empty() {
        return Some(Err(
            "every pixel of the image is transparent, so there is nothing to quantize".to_string(),
        ));
    }

    params.iterations = (settings.fraction_of_pixels * pixels.len() as f32) as i32;
    if params.dither == TpqDitherMode::Slow {
        // Dithered searches are far more expensive, so the slow mode takes
        // fewer and smaller steps.
        params.iterations /= 5;
        params.alpha = 0.1;
        params.final_alpha = 0.02;
    }

    let mut shuffle = RandomShuffle::new(pixels.len(), shuffle_mode);
    quantize(&image, &tiles, &pixels, &mut shuffle, &params, ctx).map(Ok)
}

fn validate(
    rgba_data: &[u8],
    width: u32,
    height: u32,
    target: &TargetFormat,
) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("the image is empty".to_string());
    }
    if rgba_data.len() != (width as usize) * (height as usize) * 4 {
        return Err(format!(
            "the image data holds {} bytes, not the {} bytes of a {width}x{height} RGBA image",
            rgba_data.len(),
            (width as usize) * (height as usize) * 4
        ));
    }
    let (tile_width, tile_height) = (u32::from(target.tile_width), u32::from(target.tile_height));
    if tile_width == 0 || tile_height == 0 {
        return Err("the tile size must be at least 1x1".to_string());
    }
    if !width.is_multiple_of(tile_width) || !height.is_multiple_of(tile_height) {
        return Err(format!(
            "the image is {width}x{height}, which is not a whole number of {tile_width}x{tile_height} tiles"
        ));
    }
    if target.n_colors < 2 {
        return Err("tilepalquant needs at least 2 colors per palette".to_string());
    }
    if target.n_palettes < 1 {
        return Err("at least one palette is needed".to_string());
    }
    let total = usize::from(target.n_palettes) * usize::from(target.n_colors);
    if total > 256 {
        return Err(format!(
            "{} palettes of {} colors is {total} colors, more than the 256 an index fits",
            target.n_palettes, target.n_colors
        ));
    }
    Ok(())
}

/// Sends progress to the host, with previews rate limited so they stay a
/// small share of the run: a preview costs a full `quantize_tiles`, so one
/// is only sent after at least [`PREVIEW_MIN_GAP`] and four times the cost
/// of the previous preview have passed.
struct Reporter<'a> {
    ctx: &'a RunContext<'a>,
    image: &'a SourceImage,
    p: &'a Params,
    use_dither: bool,
    last_preview: Option<crate::time::Instant>,
    last_cost: std::time::Duration,
}

const PREVIEW_MIN_GAP: std::time::Duration = std::time::Duration::from_millis(100);

impl Reporter<'_> {
    fn report(&mut self, percent: i64, palettes: Option<&[Vec<Rgb>]>) {
        let wants_preview = self.p.show_progress && self.ctx.progress.is_some();
        let due = match self.last_preview {
            None => true,
            Some(at) => at.elapsed() >= PREVIEW_MIN_GAP.max(self.last_cost * 4),
        };
        let preview = palettes.filter(|_| wants_preview && due).map(|palettes| {
            let started = crate::time::Instant::now();
            let preview = quantize_tiles(palettes, self.image, self.use_dither, self.p);
            self.last_cost = started.elapsed();
            self.last_preview = Some(crate::time::Instant::now());
            preview
        });
        self.ctx.report(Progress {
            percent: percent.clamp(0, 100) as u8,
            preview,
        });
    }
}

/// The optimization itself. Returns `None` when the run is cancelled.
fn quantize(
    image: &SourceImage,
    tiles: &[tile::Tile],
    pixels: &[tile::Pixel],
    shuffle: &mut RandomShuffle,
    p: &Params,
    ctx: &RunContext,
) -> Option<QuantizeResult> {
    let use_dither = p.dither != TpqDitherMode::Off;
    // The share of the run each stage accounts for. Dithering skips the
    // k-means passes at the end, which is what the last stop is for.
    let mut progress_stops = [25i64, 65, 90, 100];
    if use_dither {
        progress_stops[3] = 94;
    }
    let mut reporter = Reporter {
        ctx,
        image,
        p,
        use_dither,
        last_preview: None,
        last_cost: std::time::Duration::ZERO,
    };
    let mut report =
        |percent: i64, palettes: Option<&[Vec<Rgb>]>| reporter.report(percent, palettes);

    report(0, None);
    let mut palettes = quantize_1_color(tiles, pixels, shuffle, p);

    let mut start_index = 2;
    if p.first_color == FirstColor::Shared {
        start_index += 1;
    }
    let mut end_index = p.colors_per_palette;
    if p.first_color.is_transparent() {
        end_index -= 1;
    }

    report(progress_stops[0] / p.n_palettes as i64, Some(&palettes));
    if ctx.is_cancelled() {
        return None;
    }

    for num_colors in start_index..=end_index {
        expand_by_one_color(&mut palettes, tiles, pixels, shuffle, p);
        report(
            progress_stops[0] * num_colors as i64 / p.colors_per_palette as i64,
            Some(&palettes),
        );
        if ctx.is_cancelled() {
            return None;
        }
    }

    let mut min_mse = mean_square_error(&palettes, tiles);
    let mut min_palettes = palettes.clone();
    for iteration in 0..REPLACE_ITERATIONS {
        palettes = replace_weakest_colors(
            &palettes,
            tiles,
            MIN_COLOR_FACTOR,
            MIN_PALETTE_FACTOR,
            true,
            p,
        );
        for _ in 0..p.iterations {
            let pixel = pixels[shuffle.next_index()];
            move_palettes_closer(&mut palettes, tiles, &pixel, p.alpha, p);
        }
        let mse = mean_square_error(&palettes, tiles);
        if mse < min_mse {
            min_mse = mse;
            min_palettes = palettes.clone();
        }
        // The last round shows the palettes the run will actually keep.
        let shown = if iteration == REPLACE_ITERATIONS - 1 {
            &min_palettes
        } else {
            &palettes
        };
        report(
            progress_stops[0]
                + (progress_stops[1] - progress_stops[0]) * (iteration + 1) / REPLACE_ITERATIONS,
            Some(shown),
        );
        if ctx.is_cancelled() {
            return None;
        }
    }
    palettes = min_palettes;

    if !use_dither {
        palettes = reduce_palettes(&palettes, p);
    }

    let final_iterations = i64::from(p.iterations) * 10;
    let mut next_update = i64::from(p.iterations);
    for iteration in 0..final_iterations {
        let pixel = pixels[shuffle.next_index()];
        move_palettes_closer(&mut palettes, tiles, &pixel, p.final_alpha, p);
        if iteration >= next_update {
            next_update += i64::from(p.iterations);
            report(
                progress_stops[1]
                    + (progress_stops[2] - progress_stops[1]) * iteration / final_iterations,
                Some(&palettes),
            );
            if ctx.is_cancelled() {
                return None;
            }
        }
    }
    report(progress_stops[2], Some(&palettes));
    if ctx.is_cancelled() {
        return None;
    }

    if !use_dither {
        palettes = reduce_palettes(&palettes, p);
        for iteration in 0..K_MEANS_PASSES {
            palettes = k_means(&palettes, tiles, p);
            report(
                progress_stops[2]
                    + (progress_stops[3] - progress_stops[2]) * (iteration + 1) / K_MEANS_PASSES,
                Some(&palettes),
            );
            if ctx.is_cancelled() {
                return None;
            }
        }
    }

    palettes = reduce_palettes(&palettes, p);
    let result = quantize_tiles(&palettes, image, use_dither, p);
    report(100, None);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tilepalquant::DitherPattern;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    fn levels(bits: u32) -> Vec<u8> {
        let steps = (1u32 << bits) - 1;
        (0..=steps)
            .map(|i| (f64::from(i) * 255.0 / f64::from(steps)).round() as u8)
            .collect()
    }

    fn target() -> TargetFormat {
        TargetFormat {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 2,
            n_colors: 4,
            levels: [levels(3), levels(3), levels(3), vec![0, 255]],
            first_color: FirstColor::Unique,
            shared_color: [0, 0, 0],
            transparent_color: [255, 0, 255],
        }
    }

    /// [`target`] with what index 0 holds picked by the caller.
    fn target_with(first_color: FirstColor) -> TargetFormat {
        TargetFormat {
            first_color,
            ..target()
        }
    }

    /// Two 8x8 tiles: warm colors on the left, cool ones on the right, with a
    /// transparent block and a magenta block cut into them.
    fn two_tiles() -> Vec<u8> {
        let mut data = Vec::with_capacity(16 * 8 * 4);
        for y in 0..8u32 {
            for x in 0..16u32 {
                let pixel = if x < 2 && y < 2 {
                    [0, 0, 0, 0]
                } else if (8..10).contains(&x) && y < 2 {
                    [255, 0, 255, 255]
                } else if x < 8 {
                    [200, (y * 16) as u8, 40, 255]
                } else {
                    [30, (y * 16) as u8, 210, 255]
                };
                data.extend_from_slice(&pixel);
            }
        }
        data
    }

    /// A `Params` matching [`target`], with dithering off.
    pub fn params() -> Params {
        Params {
            n_palettes: 2,
            colors_per_palette: 4,
            tile_width: 8,
            tile_height: 8,
            dither: TpqDitherMode::Off,
            dither_pattern: DitherPattern::Diagonal4.matrix(),
            dither_pixels: DitherPattern::Diagonal4.candidates(),
            dither_weight: 0.5,
            first_color: FirstColor::Unique,
            first_color_value: Rgb::default(),
            lut: ColorLut::new(&target().levels),
            iterations: 1,
            alpha: 0.3,
            final_alpha: 0.05,
            show_progress: false,
        }
    }

    /// [`params`] set up for a dithered search.
    pub fn params_with(pattern: DitherPattern, dither_weight: f32) -> Params {
        Params {
            dither: TpqDitherMode::Fast,
            dither_pattern: pattern.matrix(),
            dither_pixels: pattern.candidates(),
            dither_weight,
            ..params()
        }
    }

    fn settings() -> TpqSettings {
        TpqSettings {
            show_progress: false,
            ..TpqSettings::default()
        }
    }

    fn run_engine(target: &TargetFormat, settings: &TpqSettings) -> Result<QuantizeResult, String> {
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        run(&two_tiles(), 16, 8, target, settings, &ctx).expect("not cancelled")
    }

    /// Which palette entry a pixel resolved to, as (palette, color).
    fn entry(result: &QuantizeResult, x: u32, y: u32) -> (usize, usize) {
        let index = result.indexed_data[(x + 16 * y) as usize] as usize;
        (
            index / result.colors_per_palette,
            index % result.colors_per_palette,
        )
    }

    #[test]
    fn a_run_fills_every_palette_and_indexes_every_pixel() {
        let result = run_engine(&target(), &settings()).expect("succeeds");
        assert_eq!(result.indexed_data.len(), 16 * 8);
        assert_eq!(result.colors_per_palette, 4);
        assert_eq!(result.palette_data.len(), 8);
        assert!(result.indexed_data.iter().all(|&index| index < 8));
        assert!(result.palette_data.iter().all(|color| color.a == 255));
    }

    #[test]
    fn unique_leaves_index_zero_to_the_optimization() {
        let result = run_engine(&target(), &settings()).expect("succeeds");
        let zeros: Vec<_> = result
            .palette_data
            .chunks(4)
            .map(|palette| (palette[0].r, palette[0].g, palette[0].b))
            .collect();
        assert_ne!(
            zeros[0], zeros[1],
            "the two palettes cover different colors"
        );
    }

    #[test]
    fn shared_puts_the_same_snapped_color_in_every_index_zero() {
        let target = TargetFormat {
            shared_color: [10, 200, 250],
            ..target_with(FirstColor::Shared)
        };
        let result = run_engine(&target, &settings()).expect("succeeds");
        // The shared color is snapped to the target levels like any other.
        for palette in result.palette_data.chunks(4) {
            assert_eq!(
                (palette[0].r, palette[0].g, palette[0].b, palette[0].a),
                (0, 182, 255, 255)
            );
        }
    }

    #[test]
    fn transparent_from_alpha_reserves_index_zero_for_the_clear_pixels() {
        let result = run_engine(&target_with(FirstColor::TransparentFromAlpha), &settings())
            .expect("succeeds");
        for palette in result.palette_data.chunks(4) {
            assert_eq!(
                (palette[0].r, palette[0].g, palette[0].b, palette[0].a),
                (255, 0, 255, 0),
                "index 0 is the transparent color, marked clear"
            );
            assert!(palette[1..].iter().all(|color| color.a == 255));
        }
        assert_eq!(entry(&result, 0, 0).1, 0, "a clear pixel takes index 0");
        for y in 0..8 {
            for x in 0..16 {
                let clear = x < 2 && y < 2;
                assert_eq!(entry(&result, x, y).1 == 0, clear, "at {x},{y}");
            }
        }
    }

    #[test]
    fn transparent_from_color_reserves_index_zero_for_the_key_color() {
        let result = run_engine(&target_with(FirstColor::TransparentFromColor), &settings())
            .expect("succeeds");
        for palette in result.palette_data.chunks(4) {
            assert_eq!(
                (palette[0].r, palette[0].g, palette[0].b, palette[0].a),
                (255, 0, 255, 0)
            );
        }
        for y in 0..8 {
            for x in 0..16 {
                let key = (8..10).contains(&x) && y < 2;
                assert_eq!(entry(&result, x, y).1 == 0, key, "at {x},{y}");
            }
        }
    }

    #[test]
    fn a_seed_decides_the_run_and_a_different_one_changes_it() {
        let mut settings = settings();
        settings.rand_seed = 1;
        let first = run_engine(&target(), &settings).expect("succeeds");
        let again = run_engine(&target(), &settings).expect("succeeds");
        assert_eq!(first.indexed_data, again.indexed_data);
        assert_eq!(palette_bytes(&first), palette_bytes(&again));

        settings.rand_seed = 12345;
        let other = run_engine(&target(), &settings).expect("succeeds");
        assert_ne!(
            (palette_bytes(&other), &other.indexed_data),
            (palette_bytes(&first), &first.indexed_data)
        );
    }

    fn palette_bytes(result: &QuantizeResult) -> Vec<[u8; 4]> {
        result
            .palette_data
            .iter()
            .map(|color| [color.b, color.g, color.r, color.a])
            .collect()
    }

    #[test]
    fn a_cancelled_run_returns_nothing() {
        let cancel = AtomicBool::new(true);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let result = run(&two_tiles(), 16, 8, &target(), &settings(), &ctx);
        assert!(result.is_none());
    }

    #[test]
    fn progress_climbs_to_a_hundred_and_carries_previews() {
        let cancel = AtomicBool::new(false);
        let (sender, receiver) = mpsc::channel();
        let report = |progress: Progress| {
            let _ = sender.send(progress);
        };
        let ctx = RunContext {
            cancel: &cancel,
            progress: Some(&report),
        };
        let settings = TpqSettings {
            show_progress: true,
            ..settings()
        };
        let result = run(&two_tiles(), 16, 8, &target(), &settings, &ctx)
            .expect("not cancelled")
            .expect("succeeds");
        drop(sender);

        let reports: Vec<Progress> = receiver.iter().collect();
        assert!(reports.len() > 4, "got {} reports", reports.len());
        assert_eq!(reports[0].percent, 0);
        assert_eq!(reports.last().expect("reported").percent, 100);
        assert!(
            reports
                .windows(2)
                .all(|pair| pair[0].percent <= pair[1].percent),
            "percentages never go backwards"
        );
        // Previews are rate limited, so a run this short sends few of them.
        // Early previews show palettes that are still growing, so they can
        // hold fewer colors than the final result, never more.
        let previews: Vec<&QuantizeResult> = reports
            .iter()
            .filter_map(|report| report.preview.as_ref())
            .collect();
        assert!(!previews.is_empty());
        for preview in previews {
            assert_eq!(preview.indexed_data.len(), result.indexed_data.len());
            assert!(preview.colors_per_palette <= result.colors_per_palette);
            assert_eq!(
                preview.palette_data.len(),
                preview.colors_per_palette * target().n_palettes as usize
            );
        }
    }

    #[test]
    fn previews_stay_away_when_they_are_switched_off() {
        let cancel = AtomicBool::new(false);
        let (sender, receiver) = mpsc::channel();
        let report = |progress: Progress| {
            let _ = sender.send(progress);
        };
        let ctx = RunContext {
            cancel: &cancel,
            progress: Some(&report),
        };
        run(&two_tiles(), 16, 8, &target(), &settings(), &ctx);
        drop(sender);
        assert!(receiver.iter().all(|report| report.preview.is_none()));
    }

    #[test]
    fn an_image_that_is_not_a_whole_number_of_tiles_is_refused() {
        let mut target = target();
        target.tile_width = 5;
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let error = run(&two_tiles(), 16, 8, &target, &settings(), &ctx)
            .expect("not cancelled")
            .expect_err("refused");
        assert!(error.contains("5x8 tiles"), "{error}");
    }

    #[test]
    fn out_of_range_palette_layouts_are_refused() {
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let refuse = |n_palettes, n_colors| {
            let target = TargetFormat {
                n_palettes,
                n_colors,
                ..target()
            };
            run(&two_tiles(), 16, 8, &target, &settings(), &ctx)
                .expect("not cancelled")
                .expect_err("refused")
        };
        assert!(refuse(2, 1).contains("at least 2 colors"));
        assert!(refuse(64, 16).contains("1024 colors"));
    }

    #[test]
    fn an_image_with_nothing_opaque_is_refused() {
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let error = run(
            &vec![0u8; 16 * 8 * 4],
            16,
            8,
            &target_with(FirstColor::TransparentFromAlpha),
            &settings(),
            &ctx,
        )
        .expect("not cancelled")
        .expect_err("refused");
        assert!(error.contains("transparent"), "{error}");
    }

    #[test]
    fn a_short_buffer_is_refused() {
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let error = run(&[0u8; 16], 16, 8, &target(), &settings(), &ctx)
            .expect("not cancelled")
            .expect_err("refused");
        assert!(error.contains("512 bytes"), "{error}");
    }

    #[test]
    fn two_colors_per_palette_in_a_transparent_mode_leaves_one_real_color() {
        let target = TargetFormat {
            n_colors: 2,
            n_palettes: 1,
            ..target_with(FirstColor::TransparentFromAlpha)
        };
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let result = run(&two_tiles(), 16, 8, &target, &settings(), &ctx)
            .expect("not cancelled")
            .expect("succeeds");
        assert_eq!(result.palette_data.len(), 2);
        assert_eq!(result.palette_data[0].a, 0);
        assert!(result.indexed_data.iter().all(|&index| index < 2));
    }

    #[test]
    fn every_dither_mode_and_pattern_produces_a_full_result() {
        for mode in TpqDitherMode::all() {
            for pattern in DitherPattern::all() {
                let settings = TpqSettings {
                    dither_mode: *mode,
                    dither_pattern: *pattern,
                    ..settings()
                };
                let result = run_engine(&target(), &settings).expect("succeeds");
                assert_eq!(result.indexed_data.len(), 16 * 8, "{mode:?} {pattern:?}");
                assert!(
                    result.indexed_data.iter().all(|&index| index < 8),
                    "{mode:?} {pattern:?}"
                );
            }
        }
    }

    #[test]
    fn a_single_pixel_image_completes_without_any_iterations() {
        let target = TargetFormat {
            tile_width: 1,
            tile_height: 1,
            n_palettes: 2,
            n_colors: 2,
            ..target()
        };
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let result = run(&[10, 20, 30, 255], 1, 1, &target, &settings(), &ctx)
            .expect("not cancelled")
            .expect("succeeds");
        assert_eq!(result.indexed_data.len(), 1);
        assert_eq!(result.palette_data.len(), 4);
    }

    #[test]
    fn a_one_by_one_tile_grid_works() {
        let target = TargetFormat {
            tile_width: 1,
            tile_height: 1,
            ..target()
        };
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let result = run(&two_tiles(), 16, 8, &target, &settings(), &ctx)
            .expect("not cancelled")
            .expect("succeeds");
        assert_eq!(result.indexed_data.len(), 16 * 8);
    }
}

/// Timing of a large run with and without progress previews. Ignored by
/// default; run with `cargo test --release -- --ignored preview_cost --nocapture`.
#[cfg(test)]
mod timing {
    use super::*;
    use crate::types::tilepalquant::TpqSettings;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;

    fn upscaled_fixture(factor: u32) -> (Vec<u8>, u32, u32) {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/tpq/photo_64x32.png"
        );
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        let (w, h) = (info.width, info.height);
        let bpp = info.color_type.samples();
        let (out_w, out_h) = (w * factor, h * factor);
        let mut out = Vec::with_capacity((out_w * out_h * 4) as usize);
        for y in 0..out_h {
            for x in 0..out_w {
                let i = (((y / factor) * w + x / factor) as usize) * bpp;
                out.extend_from_slice(&[buf[i], buf[i + 1], buf[i + 2], 255]);
            }
        }
        (out, out_w, out_h)
    }

    #[test]
    #[ignore]
    fn preview_cost() {
        let (data, w, h) = upscaled_fixture(16);
        let levels: Vec<u8> = (0..8).map(|i| (i * 255 / 7) as u8).collect();
        let target = TargetFormat {
            tile_width: 8,
            tile_height: 8,
            n_palettes: 4,
            n_colors: 16,
            levels: [levels.clone(), levels.clone(), levels, vec![0, 255]],
            first_color: FirstColor::Unique,
            shared_color: [0, 0, 0],
            transparent_color: [255, 0, 255],
        };
        for show_progress in [false, true] {
            let settings = TpqSettings {
                show_progress,
                ..TpqSettings::default()
            };
            let cancel = AtomicBool::new(false);
            let (sender, receiver) = mpsc::channel();
            let report = |progress: Progress| {
                let _ = sender.send(progress);
            };
            let ctx = RunContext {
                cancel: &cancel,
                progress: Some(&report),
            };
            let started = crate::time::Instant::now();
            let result = run(&data, w, h, &target, &settings, &ctx);
            let elapsed = started.elapsed();
            assert!(matches!(result, Some(Ok(_))));
            let previews = receiver.try_iter().filter(|p| p.preview.is_some()).count();
            println!(
                "{w}x{h} show_progress={show_progress}: {:.0} ms, {previews} previews",
                elapsed.as_secs_f64() * 1000.0
            );
        }
    }
}
