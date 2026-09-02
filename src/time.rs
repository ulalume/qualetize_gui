//! `Instant` for every target the app builds for.
//!
//! `std::time::Instant::now` panics on `wasm32-unknown-unknown`; `web_time`
//! reads `performance.now()` there and re-exports `std`'s type elsewhere.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;
