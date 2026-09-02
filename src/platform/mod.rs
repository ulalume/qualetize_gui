//! Everything that differs between the native and the web build: where
//! background work runs, where files come from and go to, where settings
//! are kept. The rest of the app only uses what this module re-exports.

use crate::types::ExportFormat;
use crate::types::app_state::AppStateRequest;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{Job, export_image, pick_image, pick_settings_file, save_settings, storage};

#[cfg(target_arch = "wasm32")]
pub mod web;
#[cfg(target_arch = "wasm32")]
pub use web::{Job, export_image, pick_image, pick_settings_file, save_settings, storage};

/// What a file dialog needs to report back: the channel its result goes to,
/// the flag that keeps drag & drop out while it is up, and the context to
/// wake so the result is picked up on the next frame.
#[derive(Clone)]
pub struct DialogContext {
    pub sender: mpsc::Sender<AppStateRequest>,
    pub dialog_open: Arc<AtomicBool>,
    pub egui_ctx: egui::Context,
}

impl DialogContext {
    /// Hand `request` to the app and ask for a frame to handle it in.
    pub fn send(&self, request: AppStateRequest) {
        _ = self.sender.send(request);
        self.egui_ctx.request_repaint();
    }
}

/// Raises the "a dialog is up" flag for as long as it lives.
pub struct FileDialogGuard {
    flag: Arc<AtomicBool>,
}

impl FileDialogGuard {
    pub fn new(flag: Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::Relaxed);
        Self { flag }
    }
}

impl Drop for FileDialogGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Relaxed);
        log::debug!("FileDialogGuard dropped - dialog closed");
    }
}

/// Default name a settings file is saved under.
pub const DEFAULT_SETTINGS_FILE_NAME: &str = "qualetize_settings.qset";

/// Build the default export path for `input_path`.
///
/// The extension is appended rather than set via [`std::path::Path::with_extension`],
/// which would treat everything after the last dot of the *new* name as an extension
/// and silently truncate it (`hero.idle.png` -> `hero.png`).
pub fn export_path(input_path: &str, format: ExportFormat, suffix: Option<&str>) -> PathBuf {
    let path = Path::new(input_path);

    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_stem()
        .unwrap_or(std::ffi::OsStr::new("output"))
        .to_string_lossy();
    let extension = format.extension();
    let file_name = match suffix {
        Some(suffix) => format!("{stem}_{suffix}.{extension}"),
        None => format!("{stem}.{extension}"),
    };
    parent.join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_of(input: &str, format: ExportFormat, suffix: Option<&str>) -> String {
        export_path(input, format, suffix)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn export_path_appends_suffix_and_extension() {
        assert_eq!(
            path_of(
                "/img/hero.png",
                ExportFormat::PngIndexed,
                Some("qualetized")
            ),
            "/img/hero_qualetized.png"
        );
        assert_eq!(
            path_of("/img/hero.png", ExportFormat::Bmp, Some("qualetized")),
            "/img/hero_qualetized.bmp"
        );
    }

    #[test]
    fn export_path_without_suffix_keeps_stem() {
        assert_eq!(
            path_of("/img/hero.png", ExportFormat::Bmp, None),
            "/img/hero.bmp"
        );
    }

    /// `with_extension` would truncate `hero.idle.png` to `hero.png`,
    /// dropping both the suffix and part of the original file name.
    #[test]
    fn export_path_preserves_dots_in_file_name() {
        assert_eq!(
            path_of(
                "/img/hero.idle.png",
                ExportFormat::PngIndexed,
                Some("qualetized")
            ),
            "/img/hero.idle_qualetized.png"
        );
        assert_eq!(
            path_of("/img/tile.v2.bmp", ExportFormat::Bmp, None),
            "/img/tile.v2.bmp"
        );
    }

    #[test]
    fn export_path_handles_missing_extension_and_parent() {
        assert_eq!(
            path_of("hero", ExportFormat::Bmp, Some("qualetized")),
            "hero_qualetized.bmp"
        );
    }
}
