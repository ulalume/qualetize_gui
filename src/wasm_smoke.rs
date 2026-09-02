//! Spike entry point: runs both engines on a synthetic image. Exported to JS on
//! wasm; callable from a test on any target so the two can be compared.

use crate::engine::{self, QuantEngine, QuantizeResult, RunContext};
use crate::types::QualetizeSettings;
use crate::types::tilepalquant::TpqSettings;
use std::sync::atomic::AtomicBool;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const SIZE: u32 = 16;

/// 16x16 RGBA gradient, red across and green down.
fn gradient() -> Vec<u8> {
    (0..SIZE * SIZE)
        .flat_map(|i| [(i % SIZE * SIZE) as u8, (i / SIZE * SIZE) as u8, 128, 255])
        .collect()
}

/// Root mean square error between the quantized image and its source, in
/// 0..=255 units per channel.
fn rmse(result: &QuantizeResult, rgba: &[u8]) -> f64 {
    let mut sum = 0.0;
    for (px, &index) in result.indexed_data.iter().enumerate() {
        let entry = result.palette_data[index as usize];
        let src = &rgba[px * 4..px * 4 + 3];
        for (a, b) in [entry.r, entry.g, entry.b].iter().zip(src) {
            let d = *a as f64 - *b as f64;
            sum += d * d;
        }
    }
    (sum / (result.indexed_data.len() * 3) as f64).sqrt()
}

fn run_one(which: QuantEngine) -> String {
    let rgba = gradient();
    let cancel = AtomicBool::new(false);
    let ctx = RunContext {
        cancel: &cancel,
        progress: None,
    };
    let mut settings = QualetizeSettings::genesis();
    settings.tile_passes = 4;
    settings.color_passes = 4;

    match engine::run(
        which,
        &rgba,
        SIZE,
        SIZE,
        &settings,
        &TpqSettings::default(),
        &ctx,
    ) {
        Some(Ok(result)) => {
            let first = result.palette_data[0];
            format!(
                "{}: {} px, {} palette entries, first entry rgba({},{},{},{}), RMSE {:.3}",
                which.display_name(),
                result.indexed_data.len(),
                result.palette_data.len(),
                first.r,
                first.g,
                first.b,
                first.a,
                rmse(&result, &rgba),
            )
        }
        Some(Err(e)) => format!("{}: failed: {e}", which.display_name()),
        None => format!("{}: cancelled", which.display_name()),
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn smoke() -> String {
    [QuantEngine::Qualetize, QuantEngine::TilePalQuant]
        .into_iter()
        .map(run_one)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke_runs() {
        println!("{}", super::smoke());
    }
}
