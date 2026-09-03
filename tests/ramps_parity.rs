//! Parity between `palette_ramps::ramp_order` and the reference
//! implementation in `.doc/palette-order/recommended.py`.
//!
//! `tests/fixtures/ramps/gen.py` regenerates `cases.json` by running the
//! reference over 16-color chunks of every sample palette and three
//! shuffled variants of sweetie-16.

use qualetize_gui::types::palette_ramps::ramp_order;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Case {
    name: String,
    input: Vec<[u8; 3]>,
    expected: Vec<[u8; 3]>,
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ramps")
}

fn cases() -> Vec<Case> {
    let path = fixture_dir().join("cases.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}. Regenerate with tests/fixtures/ramps/gen.py",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("cases.json is valid json")
}

#[test]
fn the_fixture_set_is_not_empty() {
    assert!(!cases().is_empty(), "no parity cases were generated");
}

#[test]
fn ramp_order_matches_the_reference_implementation() {
    for case in cases() {
        let order = ramp_order(&case.input);

        let mut seen = order.clone();
        seen.sort_unstable();
        assert_eq!(
            seen,
            (0..case.input.len()).collect::<Vec<_>>(),
            "{}: not a bijection",
            case.name
        );

        let got: Vec<[u8; 3]> = order.iter().map(|&i| case.input[i]).collect();
        assert_eq!(
            got, case.expected,
            "{}: order does not match the reference",
            case.name
        );
    }
}
