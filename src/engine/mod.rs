//! Quantization engines. Each engine turns an RGBA image into indexed
//! pixels plus a flat palette, in the shape `ImageDataIndexed` consumes.
// Consumed by the tilepalquant engine and its settings panel.
#![allow(dead_code)]

pub mod qualetize;
pub mod tilepalquant;

use crate::types::BGRA8;
use crate::types::QualetizeSettings;
use crate::types::tilepalquant::TpqSettings;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum QuantEngine {
    #[default]
    Qualetize,
    TilePalQuant,
}

impl QuantEngine {
    pub fn display_name(&self) -> &'static str {
        match self {
            QuantEngine::Qualetize => "Qualetize",
            QuantEngine::TilePalQuant => "tilepalquant",
        }
    }

    pub fn all() -> &'static [QuantEngine] {
        &[QuantEngine::Qualetize, QuantEngine::TilePalQuant]
    }
}

/// Indexed output of either engine. `palette_data` holds
/// `n_palettes * colors_per_palette` entries and each pixel index is
/// `palette_idx * colors_per_palette + color_idx`.
#[derive(Debug, Clone)]
pub struct QuantizeResult {
    pub indexed_data: Vec<u8>,
    pub palette_data: Vec<BGRA8>,
    pub colors_per_palette: usize,
    pub width: u32,
    pub height: u32,
}

/// Intermediate state reported by an engine while it runs.
#[derive(Debug, Clone)]
pub struct Progress {
    pub percent: u8,
    /// A quantization of the current palettes, in the shape of the final
    /// result, for showing the palettes converge.
    pub preview: Option<QuantizeResult>,
}

/// The hardware format both engines target: tile grid, palette layout and
/// the per-channel levels colors are snapped to.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetFormat {
    pub tile_width: u16,
    pub tile_height: u16,
    pub n_palettes: u16,
    pub n_colors: u16,
    /// Allowed values per channel (R, G, B, A), ascending, each within 0..=255.
    pub levels: [Vec<u8>; 4],
}

impl TargetFormat {
    pub fn from_settings(settings: &QualetizeSettings) -> Self {
        Self {
            tile_width: settings.tile_width,
            tile_height: settings.tile_height,
            n_palettes: settings.n_palettes,
            n_colors: settings.n_colors,
            levels: settings.channel_levels(),
        }
    }
}

/// What a running engine can observe about the outside world.
pub struct RunContext<'a> {
    pub cancel: &'a AtomicBool,
    pub progress: Option<&'a mpsc::Sender<Progress>>,
}

impl RunContext<'_> {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn report(&self, progress: Progress) {
        if let Some(sender) = self.progress {
            let _ = sender.send(progress);
        }
    }
}

/// Run `engine` on `rgba_data`. Returns `None` when cancelled.
pub fn run(
    engine: QuantEngine,
    rgba_data: &[u8],
    width: u32,
    height: u32,
    settings: &QualetizeSettings,
    tpq: &TpqSettings,
    ctx: &RunContext,
) -> Option<Result<QuantizeResult, String>> {
    let started = std::time::Instant::now();
    let result = match engine {
        QuantEngine::Qualetize => {
            // The C call cannot be interrupted, so this only saves work when
            // the job was superseded before its thread got scheduled.
            if ctx.is_cancelled() {
                return None;
            }
            Some(qualetize::run(rgba_data, width, height, settings))
        }
        QuantEngine::TilePalQuant => {
            let target = TargetFormat::from_settings(settings);
            tilepalquant::run(rgba_data, width, height, &target, tpq, ctx)
        }
    };
    match &result {
        Some(Ok(_)) => log::info!(
            "{} finished {width}x{height} in {:.0} ms",
            engine.display_name(),
            started.elapsed().as_secs_f64() * 1000.0
        ),
        Some(Err(e)) => log::warn!("{} failed: {e}", engine.display_name()),
        None => log::debug!("{} cancelled", engine.display_name()),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 16x16 RGBA gradient.
    fn gradient() -> Vec<u8> {
        (0..16 * 16)
            .flat_map(|i| [(i % 16 * 16) as u8, (i / 16 * 16) as u8, 128, 255])
            .collect()
    }

    fn run_engine(engine: QuantEngine) -> Option<Result<QuantizeResult, String>> {
        let cancel = AtomicBool::new(false);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let mut settings = QualetizeSettings::genesis();
        settings.tile_passes = 4;
        settings.color_passes = 4;
        run(
            engine,
            &gradient(),
            16,
            16,
            &settings,
            &TpqSettings::default(),
            &ctx,
        )
    }

    #[test]
    fn the_qualetize_engine_produces_a_result_of_the_documented_shape() {
        let result = run_engine(QuantEngine::Qualetize)
            .expect("not cancelled")
            .expect("succeeds");
        assert_eq!(result.indexed_data.len(), 16 * 16);
        assert_eq!(result.colors_per_palette, 16);
        assert_eq!(result.palette_data.len(), 16);
        assert!(result.indexed_data.iter().all(|&i| (i as usize) < 16));
    }

    #[test]
    fn a_cancelled_qualetize_run_returns_nothing() {
        let cancel = AtomicBool::new(true);
        let ctx = RunContext {
            cancel: &cancel,
            progress: None,
        };
        let result = run(
            QuantEngine::Qualetize,
            &gradient(),
            16,
            16,
            &QualetizeSettings::genesis(),
            &TpqSettings::default(),
            &ctx,
        );
        assert!(result.is_none());
    }

    #[test]
    fn target_format_carries_the_channel_levels() {
        let settings = QualetizeSettings::genesis();
        let target = TargetFormat::from_settings(&settings);
        assert_eq!(target.levels[0], vec![0, 49, 87, 119, 146, 174, 206, 255]);
        assert_eq!(
            (target.tile_width, target.n_palettes, target.n_colors),
            (8, 1, 16)
        );
    }
}
