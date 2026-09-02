//! The tilepalquant parity fixtures: that the set is complete and holds what
//! the comparison expects of it.
//!
//! The bit-exact comparison itself lives in `src/engine/tilepalquant/parity.rs`
//! and runs with the rest of `cargo test`. It cannot live here: the package
//! builds only a binary, so an integration test has no library to link the
//! engine from.
//!
//! `tests/fixtures/tpq/gen.sh` regenerates everything below, running the
//! reference implementation with `TPQ_FIXED_SHUFFLE=1`.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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
    tile_w: u32,
    tile_h: u32,
    num_pals: u32,
    cols_per_pal: u32,
    bits_per_chan: u32,
    col_zero: String,
    dither: String,
    dither_pat: String,
    dither_wt: f32,
    frac_of_px: f32,
    rand_seed: u32,
    shared_col: [u8; 3],
    transp_col: [u8; 3],
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tpq")
}

fn manifest() -> Manifest {
    let path = fixture_dir().join("manifest.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}. Regenerate the fixtures with tests/fixtures/tpq/gen.sh",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("the manifest is valid json")
}

/// The color type, bit depth, size and palette length of a fixture png.
fn png_shape(path: &Path) -> (png::ColorType, png::BitDepth, u32, u32, usize) {
    let file = std::io::BufReader::new(
        std::fs::File::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())),
    );
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::IDENTITY);
    let reader = decoder.read_info().expect("read png header");
    let info = reader.info();
    (
        info.color_type,
        info.bit_depth,
        info.width,
        info.height,
        info.palette.as_ref().map_or(0, |plte| plte.len() / 3),
    )
}

#[test]
fn every_case_names_files_that_are_there_and_ran_to_completion() {
    let manifest = manifest();
    assert!(!manifest.cases.is_empty(), "the manifest lists no cases");
    for case in &manifest.cases {
        assert_eq!(case.status, "ok", "case {} did not run", case.case);
        for name in [&case.input, &case.output] {
            let path = fixture_dir().join(name);
            assert!(path.is_file(), "{} is missing", path.display());
        }
    }
}

#[test]
fn case_names_are_unique_per_input() {
    let manifest = manifest();
    let mut seen = BTreeSet::new();
    for case in &manifest.cases {
        assert!(
            seen.insert((case.input.clone(), case.case.clone())),
            "{} / {} appears twice",
            case.input,
            case.case
        );
        assert!(
            seen.insert((String::new(), case.output.clone())),
            "{} is produced by two cases",
            case.output
        );
    }
}

#[test]
fn outputs_are_eight_bit_indexed_pngs_the_size_of_their_input() {
    for case in &manifest().cases {
        let (_, _, input_width, input_height, _) = png_shape(&fixture_dir().join(&case.input));
        let (color_type, bit_depth, width, height, palette_len) =
            png_shape(&fixture_dir().join(&case.output));
        let name = &case.output;
        assert_eq!(color_type, png::ColorType::Indexed, "{name}");
        assert_eq!(bit_depth, png::BitDepth::Eight, "{name}");
        assert_eq!((width, height), (input_width, input_height), "{name}");
        assert_eq!(
            palette_len,
            (case.params.num_pals * case.params.cols_per_pal) as usize,
            "{name} holds one entry per palette slot"
        );
    }
}

#[test]
fn inputs_are_a_whole_number_of_tiles() {
    for case in &manifest().cases {
        let (_, _, width, height, _) = png_shape(&fixture_dir().join(&case.input));
        assert_eq!(width % case.params.tile_w, 0, "{}", case.input);
        assert_eq!(height % case.params.tile_h, 0, "{}", case.input);
    }
}

#[test]
fn every_case_is_within_the_ranges_the_engine_accepts() {
    for case in &manifest().cases {
        let params = &case.params;
        let name = &case.case;
        assert!((1..=512).contains(&params.tile_w), "{name}");
        assert!((1..=512).contains(&params.tile_h), "{name}");
        assert!((1..=64).contains(&params.num_pals), "{name}");
        assert!((2..=256).contains(&params.cols_per_pal), "{name}");
        assert!(
            params.num_pals * params.cols_per_pal <= 256,
            "{name} asks for more than 256 colors"
        );
        assert!((2..=8).contains(&params.bits_per_chan), "{name}");
        assert!((0.01..=10.0).contains(&params.frac_of_px), "{name}");
        assert!((0.01..=1.0).contains(&params.dither_wt), "{name}");
        assert!(
            ["unique", "shared", "transp", "transp_color"].contains(&params.col_zero.as_str()),
            "{name}: unknown col_zero {}",
            params.col_zero
        );
        assert!(
            ["off", "fast", "slow"].contains(&params.dither.as_str()),
            "{name}: unknown dither {}",
            params.dither
        );
        assert!(
            ["diag4", "horiz4", "vert4", "diag2", "horiz2", "vert2"]
                .contains(&params.dither_pat.as_str()),
            "{name}: unknown pattern {}",
            params.dither_pat
        );
        let _ = (params.rand_seed, params.shared_col, params.transp_col);
    }
}

#[test]
fn the_set_covers_every_index_zero_mode_and_every_dither_mode() {
    let manifest = manifest();
    for mode in ["unique", "shared", "transp", "transp_color"] {
        assert!(
            manifest
                .cases
                .iter()
                .any(|case| case.params.col_zero == mode),
            "no case covers col_zero {mode}"
        );
    }
    for dither in ["off", "fast", "slow"] {
        assert!(
            manifest
                .cases
                .iter()
                .any(|case| case.params.dither == dither),
            "no case covers dither {dither}"
        );
    }
}
