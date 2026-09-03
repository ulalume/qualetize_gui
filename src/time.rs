//! `Instant` and `SystemTime` for every target the app builds for.
//!
//! `std::time::{Instant, SystemTime}::now` panic on `wasm32-unknown-unknown`;
//! `web_time` reads `performance.now()` and `Date.now()` there and re-exports
//! `std`'s types elsewhere.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_arch = "wasm32")]
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};
