//! The work the UI hands to a background worker and the replies it gets
//! back. Requests and replies are plain data so they can cross a thread
//! boundary directly (native) or a `postMessage` boundary serialized (web).

use crate::engine::{self, Progress, QuantEngine, QuantizeResult, RunContext};
use crate::image_processor::{TileReduceOptions, TileReduceResult, reduce_tiles_indexed};
use crate::types::tilepalquant::TpqSettings;
use crate::types::{BGRA8, QualetizeSettings};
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerRequest {
    Quantize {
        engine: QuantEngine,
        settings: QualetizeSettings,
        tpq: TpqSettings,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    TileReduce {
        indexed: Vec<u8>,
        palettes: Vec<BGRA8>,
        width: u32,
        height: u32,
        opts: TileReduceOptions,
    },
}

/// What a worker sends back. Exactly one of the `*Done` variants ends a
/// request; `Progress` may arrive any number of times before it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerReply {
    Progress(Progress),
    /// `None` when the run was cancelled.
    QuantizeDone(Option<Result<QuantizeResult, String>>),
    /// `None` when the run was cancelled.
    TileReduceDone(Option<TileReduceResult>),
}

impl WorkerReply {
    pub fn is_final(&self) -> bool {
        !matches!(self, WorkerReply::Progress(_))
    }
}

/// Run `request` to completion on the calling thread. Progress goes to
/// `report` as it happens; the final reply is returned.
pub fn execute(
    request: WorkerRequest,
    cancel: &AtomicBool,
    report: &dyn Fn(WorkerReply),
) -> WorkerReply {
    match request {
        WorkerRequest::Quantize {
            engine,
            settings,
            tpq,
            rgba,
            width,
            height,
        } => {
            let progress = |progress: Progress| report(WorkerReply::Progress(progress));
            let ctx = RunContext {
                cancel,
                progress: Some(&progress),
            };
            WorkerReply::QuantizeDone(engine::run(
                engine, &rgba, width, height, &settings, &tpq, &ctx,
            ))
        }
        WorkerRequest::TileReduce {
            indexed,
            palettes,
            width,
            height,
            opts,
        } => {
            let mut indexed_pixels = indexed;
            let merged =
                reduce_tiles_indexed(&mut indexed_pixels, &palettes, width, height, &opts, cancel);
            WorkerReply::TileReduceDone(merged.map(|merged| TileReduceResult {
                indexed_pixels,
                merged,
            }))
        }
    }
}
