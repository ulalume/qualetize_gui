//! The desktop side of the platform layer: background work on a thread,
//! `rfd` file dialogs, settings in the per-user configuration directory.

use super::{DialogContext, FileDialogGuard};
use crate::settings_manager::SettingsBundle;
use crate::types::ExportFormat;
use crate::types::app_state::AppStateRequest;
use crate::worker::{self, WorkerReply, WorkerRequest};
use rfd::FileDialog;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

/// One background request at a time, polled from the UI thread.
///
/// Starting a request replaces the receiver, so a reply from an earlier
/// thread can never be observed: its send fails against the dropped receiver
/// and the thread exits on its own.
pub struct Job {
    replies: Option<mpsc::Receiver<WorkerReply>>,
    cancel: Arc<AtomicBool>,
}

impl Default for Job {
    fn default() -> Self {
        Self {
            replies: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Job {
    /// Cancel the running request, if any, and run `request` on a new thread.
    pub fn start(&mut self, request: WorkerRequest) {
        self.cancel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let flag = cancel.clone();
        std::thread::spawn(move || {
            let progress_sender = sender.clone();
            let report = |reply: WorkerReply| {
                let _ = progress_sender.send(reply);
            };
            let done = worker::execute(request, &flag, &report);
            let _ = sender.send(done);
        });
        self.replies = Some(receiver);
        self.cancel = cancel;
    }

    /// Ask the worker to stop and forget about its replies.
    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.replies = None;
    }

    pub fn is_running(&self) -> bool {
        self.replies.is_some()
    }

    /// Every reply that arrived since the last call, in order. The job ends
    /// with the final reply, or when the worker went away without one.
    pub fn drain(&mut self) -> Vec<WorkerReply> {
        let Some(receiver) = &self.replies else {
            return Vec::new();
        };
        let mut replies = Vec::new();
        let mut finished = false;
        loop {
            match receiver.try_recv() {
                Ok(reply) => {
                    finished |= reply.is_final();
                    replies.push(reply);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if finished {
            self.replies = None;
        }
        replies
    }
}

/// Run a file dialog on a worker thread so the UI keeps repainting, and feed
/// whatever the user picked back into the request channel.
///
/// The dialog flag is held for the lifetime of the dialog so drag & drop is
/// ignored while it is up.
fn spawn_file_dialog<F>(ctx: DialogContext, pick: F)
where
    F: FnOnce(FileDialog) -> Option<AppStateRequest> + Send + 'static,
{
    std::thread::spawn(move || {
        let _guard = FileDialogGuard::new(ctx.dialog_open.clone());
        if let Some(request) = pick(FileDialog::new()) {
            ctx.send(request);
        }
    });
}

/// Ask for an image file and load it by path.
pub fn pick_image(ctx: DialogContext) {
    spawn_file_dialog(ctx, |dialog| {
        let path = dialog
            .add_filter("Image files", &["png", "jpg", "jpeg", "bmp", "tga", "tiff"])
            .pick_file()?;
        Some(AppStateRequest::LoadImage {
            path: path.display().to_string(),
        })
    });
}

/// Ask where to put `bytes` and write them there on a worker thread.
///
/// `default_path` is the whole suggested path: its file name fills the dialog
/// in, its directory is where the dialog opens.
pub fn export_image(
    bytes: Vec<u8>,
    default_path: String,
    format: ExportFormat,
    ctx: DialogContext,
) {
    let default_path = PathBuf::from(default_path);
    let dialog_flag = ctx.dialog_open.clone();

    std::thread::spawn(move || {
        let output_path = {
            let _guard = FileDialogGuard::new(dialog_flag);
            let mut dialog = FileDialog::new().add_filter(
                format!("{} files", format.display_name()),
                &[format.extension()],
            );
            if let Some(file_name) = default_path.file_name() {
                dialog = dialog.set_file_name(file_name.to_string_lossy().to_string());
            }
            if let Some(parent) = default_path.parent() {
                dialog = dialog.set_directory(parent);
            }
            match dialog.save_file() {
                Some(path) => path,
                None => return,
            }
        };

        match std::fs::write(&output_path, &bytes) {
            Ok(()) => log::info!("Export completed: {}", output_path.display()),
            Err(e) => log::error!("Export failed: {e}"),
        }
    });
}

/// Ask for a `.qset` file and load it by path.
pub fn pick_settings_file(ctx: DialogContext) {
    spawn_file_dialog(ctx, |dialog| {
        let path = settings_file_dialog(dialog).pick_file()?;
        Some(AppStateRequest::LoadSettings {
            path: path.display().to_string(),
        })
    });
}

/// Ask where to put the settings and write `bundle_json` there.
pub fn save_settings(bundle_json: String, default_name: &str, ctx: DialogContext) {
    let default_name = default_name.to_string();
    let dialog_flag = ctx.dialog_open.clone();

    std::thread::spawn(move || {
        let path = {
            let _guard = FileDialogGuard::new(dialog_flag);
            match settings_file_dialog(FileDialog::new())
                .set_file_name(default_name)
                .save_file()
            {
                Some(path) => path,
                None => return,
            }
        };

        match write_atomically(&path, bundle_json.as_bytes()) {
            Ok(()) => log::info!("Settings saved successfully to: {}", path.display()),
            Err(e) => log::error!("Failed to save settings: {e}"),
        }
    });
}

/// Filter and starting directory shared by the settings load/save dialogs.
fn settings_file_dialog(dialog: FileDialog) -> FileDialog {
    let mut dialog = dialog.add_filter(
        "QualetizeGUI Settings",
        &[SettingsBundle::get_settings_file_extension()],
    );
    if let Ok(settings_dir) = settings_dir() {
        dialog = dialog.set_directory(&settings_dir);
    }
    dialog
}

/// The application's own directory inside the per-user configuration
/// directory, created if it is not there yet.
pub fn settings_dir() -> Result<PathBuf, String> {
    let Some(config_dir) = dirs::config_dir() else {
        return Err("Could not determine config directory".to_string());
    };
    let app_config_dir = config_dir.join("QualetizeGUI");
    if !app_config_dir.exists() {
        std::fs::create_dir_all(&app_config_dir)
            .map_err(|e| format!("Failed to create config directory: {e}"))?;
    }
    Ok(app_config_dir)
}

/// Write `bytes` to `path` without ever leaving a truncated file behind.
///
/// Writes go to a sibling `<path>.tmp` file first and are only made visible by an
/// atomic rename over the real target, so a crash or power loss mid-write can lose
/// the new content but never corrupts what was already on disk.
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp_path = Path::new(&tmp_name);

    std::fs::write(tmp_path, bytes)?;
    std::fs::rename(tmp_path, path)
}

/// The settings that outlive a run, kept as files under the per-user
/// configuration directory.
pub mod storage {
    use super::*;

    /// Where the value for `key` lives.
    ///
    /// The session is the same format as a hand-saved `.qset`, so it can be
    /// inspected or reused.
    fn path(key: &str) -> Option<PathBuf> {
        let file_name = match key {
            "session" => "session.qset",
            "preferences" => "preferences.json",
            other => {
                log::error!("no file for the storage key {other}");
                return None;
            }
        };
        match dirs::config_dir() {
            Some(dir) => Some(dir.join("QualetizeGUI").join(file_name)),
            // Nothing to anchor the configuration to: fall back to the
            // working directory rather than losing the value entirely.
            None => Some(PathBuf::from(file_name)),
        }
    }

    pub fn load(key: &str) -> Option<String> {
        let path = path(key)?;
        // A missing file is just a first run, not something to report.
        std::fs::read_to_string(&path).ok()
    }

    pub fn save(key: &str, value: &str) -> Result<(), String> {
        let path = path(key).ok_or_else(|| format!("no file for the storage key {key}"))?;
        write_atomically(&path, value.as_bytes())
            .map_err(|e| format!("Failed to write {}: {e}", path.display()))
    }
}
