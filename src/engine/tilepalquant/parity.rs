//! Bit-exact comparison with the reference C++ implementation.
//!
//! `tests/fixtures/tpq/manifest.json` lists the cases and
//! `tests/fixtures/tpq/gen.sh` regenerates them, running `tilepalquant` with
//! `TPQ_FIXED_SHUFFLE=1` so its pixel order is the identity. The engine runs
//! the same cases with [`ShuffleMode::Fixed`], which takes every random draw
//! out of both sides and leaves the arithmetic to be compared.
//!
//! These live in the source tree rather than in `tests/` because the package
//! builds only a binary: an integration test has no library to link against.
//! `tests/tpq_parity.rs` checks the fixture set itself.

use super::*;
use crate::types::FirstColor;
use crate::types::tilepalquant::{DitherPattern, TpqDitherMode, TpqSettings};
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

#[derive(Deserialize)]
struct Manifest {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    case: String,
    input: String,
    output: String,
    params: CaseParams,
    status: String,
}

#[derive(Deserialize)]
struct CaseParams {
    tile_w: u16,
    tile_h: u16,
    num_pals: u16,
    cols_per_pal: u16,
    bits_per_chan: u32,
    col_zero: String,
    shared_col: [u8; 3],
    transp_col: [u8; 3],
    dither: String,
    dither_pat: String,
    dither_wt: f32,
    frac_of_px: f32,
    rand_seed: u32,
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tpq")
}

/// The levels a `-bits_per_chan n` run quantizes to.
fn uniform_levels(bits: u32) -> Vec<u8> {
    let steps = (1u32 << bits) - 1;
    (0..=steps)
        .map(|i| (f64::from(i) * 255.0 / f64::from(steps)).round() as u8)
        .collect()
}

/// An RGBA8 image, whatever color type the file holds.
fn read_rgba(path: &Path) -> (Vec<u8>, u32, u32) {
    let file = std::io::BufReader::new(
        std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display())),
    );
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("read png header");
    let mut buffer = vec![0; reader.output_buffer_size().expect("png fits in memory")];
    let info = reader.next_frame(&mut buffer).expect("read png pixels");
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "{}", path.display());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buffer[..info.buffer_size()]
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|px| [px[0], px[1], px[2], 255])
            .collect(),
        other => panic!("{}: unexpected color type {other:?}", path.display()),
    };
    (rgba, info.width, info.height)
}

/// The palette indices and the PLTE entries of an 8 bit indexed PNG, read
/// without any expansion so the indices survive.
fn read_indexed(path: &Path) -> (Vec<u8>, Vec<[u8; 3]>, u32, u32) {
    let file = std::io::BufReader::new(
        std::fs::File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display())),
    );
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().expect("read png header");
    let palette = reader
        .info()
        .palette
        .as_ref()
        .expect("the reference output is an indexed png")
        .as_chunks::<3>()
        .0
        .to_vec();
    let mut buffer = vec![0; reader.output_buffer_size().expect("png fits in memory")];
    let info = reader.next_frame(&mut buffer).expect("read png pixels");
    assert_eq!(
        info.color_type,
        png::ColorType::Indexed,
        "{}",
        path.display()
    );
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "{}", path.display());
    (
        buffer[..info.buffer_size()].to_vec(),
        palette,
        info.width,
        info.height,
    )
}

fn first_color(name: &str) -> FirstColor {
    match name {
        "unique" => FirstColor::Unique,
        "shared" => FirstColor::Shared,
        "transp" => FirstColor::TransparentFromAlpha,
        "transp_color" => FirstColor::TransparentFromColor,
        other => panic!("unknown col_zero {other:?}"),
    }
}

fn dither_mode(name: &str) -> TpqDitherMode {
    match name {
        "off" => TpqDitherMode::Off,
        "fast" => TpqDitherMode::Fast,
        "slow" => TpqDitherMode::Slow,
        other => panic!("unknown dither mode {other:?}"),
    }
}

fn dither_pattern(name: &str) -> DitherPattern {
    match name {
        "diag4" => DitherPattern::Diagonal4,
        "horiz4" => DitherPattern::Horizontal4,
        "vert4" => DitherPattern::Vertical4,
        "diag2" => DitherPattern::Diagonal2,
        "horiz2" => DitherPattern::Horizontal2,
        "vert2" => DitherPattern::Vertical2,
        other => panic!("unknown dither pattern {other:?}"),
    }
}

fn describe_palette(palette: &[[u8; 3]], colors_per_palette: usize) -> String {
    let mut text = String::new();
    for (index, entry) in palette.iter().enumerate() {
        if index % colors_per_palette == 0 {
            let _ = write!(text, "\n      palette {}:", index / colors_per_palette);
        }
        let _ = write!(text, " {:3},{:3},{:3} |", entry[0], entry[1], entry[2]);
    }
    text
}

/// Run one case and compare it with the reference output. `Err` describes
/// every way the two differ.
fn check_case(case: &Case) -> Result<(), String> {
    assert_eq!(case.status, "ok", "fixture {} was not generated", case.case);
    let params = &case.params;
    let (rgba, width, height) = read_rgba(&fixture_dir().join(&case.input));
    let levels = uniform_levels(params.bits_per_chan);
    let target = TargetFormat {
        tile_width: params.tile_w,
        tile_height: params.tile_h,
        n_palettes: params.num_pals,
        n_colors: params.cols_per_pal,
        levels: [levels.clone(), levels.clone(), levels, vec![0, 255]],
        first_color: first_color(&params.col_zero),
        shared_color: params.shared_col,
        transparent_color: params.transp_col,
    };
    let settings = TpqSettings {
        fraction_of_pixels: params.frac_of_px,
        dither_mode: dither_mode(&params.dither),
        dither_pattern: dither_pattern(&params.dither_pat),
        dither_weight: params.dither_wt,
        rand_seed: params.rand_seed,
        show_progress: false,
    };
    let cancel = AtomicBool::new(false);
    let ctx = RunContext {
        cancel: &cancel,
        progress: None,
    };
    let result = run_with_shuffle(
        &rgba,
        width,
        height,
        &target,
        &settings,
        &ctx,
        ShuffleMode::Fixed,
    )
    .expect("not cancelled")
    .expect("the case is a valid input");

    let (expected_indices, expected_palette, expected_width, expected_height) =
        read_indexed(&fixture_dir().join(&case.output));
    let colors_per_palette = usize::from(params.cols_per_pal);
    let ours: Vec<[u8; 3]> = result
        .palette_data
        .iter()
        .map(|color| [color.r, color.g, color.b])
        .collect();

    let mut problems = Vec::new();
    if (result.width, result.height) != (expected_width, expected_height) {
        problems.push(format!(
            "size {}x{} against {expected_width}x{expected_height}",
            result.width, result.height
        ));
    }
    if result.colors_per_palette != colors_per_palette {
        problems.push(format!(
            "{} colors per palette against {colors_per_palette}",
            result.colors_per_palette
        ));
    }
    if ours.len() != expected_palette.len() {
        problems.push(format!(
            "{} palette entries against {}",
            ours.len(),
            expected_palette.len()
        ));
    } else if ours != expected_palette {
        let first = ours
            .iter()
            .zip(&expected_palette)
            .position(|(a, b)| a != b)
            .expect("they differ");
        problems.push(format!(
            "palette entry {first} is {:?}, the reference has {:?}\n    ours:{}\n    reference:{}",
            ours[first],
            expected_palette[first],
            describe_palette(&ours, colors_per_palette),
            describe_palette(&expected_palette, colors_per_palette),
        ));
    }
    if result.indexed_data != expected_indices {
        match result
            .indexed_data
            .iter()
            .zip(&expected_indices)
            .position(|(a, b)| a != b)
        {
            Some(at) => problems.push(format!(
                "pixel {at} ({}, {}) is index {}, the reference has {} ({} of {} pixels differ)",
                at as u32 % width,
                at as u32 / width,
                result.indexed_data[at],
                expected_indices[at],
                result
                    .indexed_data
                    .iter()
                    .zip(&expected_indices)
                    .filter(|(a, b)| a != b)
                    .count(),
                expected_indices.len(),
            )),
            None => problems.push(format!(
                "{} pixels against {}",
                result.indexed_data.len(),
                expected_indices.len()
            )),
        }
    }

    // The reference writes every palette entry opaque, including the one it
    // reserves for transparency; this engine marks that one transparent so the
    // host can draw it as such.
    let transparent_mode = target.first_color.is_transparent();
    for (index, color) in result.palette_data.iter().enumerate() {
        let expected_alpha = if transparent_mode && index % colors_per_palette == 0 {
            0
        } else {
            255
        };
        if color.a != expected_alpha {
            problems.push(format!(
                "palette entry {index} has alpha {}, expected {expected_alpha}",
                color.a
            ));
            break;
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} / {} ({}):\n  {}",
            case.input,
            case.case,
            case.output,
            problems.join("\n  ")
        ))
    }
}

fn load_manifest() -> Manifest {
    let path = fixture_dir().join("manifest.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}. Regenerate the fixtures with tests/fixtures/tpq/gen.sh",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("the manifest is valid json")
}

#[test]
fn every_case_matches_the_reference_implementation() {
    let manifest = load_manifest();
    assert!(!manifest.cases.is_empty(), "the manifest lists no cases");
    let failures: Vec<String> = manifest
        .cases
        .iter()
        .filter_map(|case| check_case(case).err())
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} cases differ from the reference implementation:\n\n{}\n",
        failures.len(),
        manifest.cases.len(),
        failures.join("\n\n")
    );
}
