//! Everything that differs between the native and the web build: where
//! background work runs, where files come from and go to, where settings
//! are kept. The rest of the app only uses what this module re-exports.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Job;

#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Job;
