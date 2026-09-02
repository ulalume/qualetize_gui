//! Rust port of tiledpalettequant (rilden; C++ port by bbbbbr), MIT.
//! Specification: `.doc/tilepalquant-port-spec.md`.

use super::{QuantizeResult, RunContext, TargetFormat};
use crate::types::tilepalquant::TpqSettings;

/// Quantize `rgba_data` into `target.n_palettes` palettes of
/// `target.n_colors` colors on the tile grid. Returns `None` when cancelled.
pub fn run(
    _rgba_data: &[u8],
    _width: u32,
    _height: u32,
    _target: &TargetFormat,
    _settings: &TpqSettings,
    _ctx: &RunContext,
) -> Option<Result<QuantizeResult, String>> {
    Some(Err("tilepalquant engine is not implemented yet".to_string()))
}
