//! Background work in a Web Worker running a second instance of this wasm
//! module, replies through `postMessage`.
//!
//! `worker.js` initializes the module and forwards every message to
//! [`worker_handle`]. Requests and replies travel as `postcard` bytes in a
//! transferred `ArrayBuffer`.
//!
//! Files come in through `rfd`'s asynchronous dialog and go out as browser
//! downloads; the settings that outlive a reload live in local storage.

use super::{DialogContext, FileDialogGuard};
use crate::settings_manager::SettingsBundle;
use crate::types::ExportFormat;
use crate::types::app_state::AppStateRequest;
use crate::worker::{self, WorkerReply, WorkerRequest};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, Worker, WorkerOptions, WorkerType};

/// Path of the worker script, relative to the page.
const WORKER_SCRIPT: &str = "./worker.js";

/// One background request at a time, polled from the UI thread.
///
/// Cancelling terminates the worker; the next request starts a fresh one.
#[derive(Default)]
pub struct Job {
    worker: Option<Worker>,
    inbox: Rc<RefCell<VecDeque<WorkerReply>>>,
    _on_message: Option<Closure<dyn FnMut(MessageEvent)>>,
}

impl Job {
    pub fn start(&mut self, request: WorkerRequest) {
        self.cancel();
        log::info!("starting a worker");
        let worker = match spawn_worker() {
            Ok(worker) => worker,
            Err(e) => {
                log::error!("could not start a worker: {e:?}");
                return;
            }
        };

        let inbox = self.inbox.clone();
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let bytes = js_sys::Uint8Array::new(&event.data()).to_vec();
            match postcard::from_bytes::<WorkerReply>(&bytes) {
                Ok(reply) => inbox.borrow_mut().push_back(reply),
                Err(e) => log::error!("unreadable worker reply: {e}"),
            }
        });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        match postcard::to_allocvec(&request) {
            Ok(bytes) => {
                let array = js_sys::Uint8Array::from(bytes.as_slice());
                let transfer = js_sys::Array::of1(&array.buffer());
                if let Err(e) = worker.post_message_with_transfer(&array, &transfer) {
                    log::error!("could not send the request to the worker: {e:?}");
                    worker.terminate();
                    return;
                }
            }
            Err(e) => {
                log::error!("could not encode the request: {e}");
                worker.terminate();
                return;
            }
        }

        self.worker = Some(worker);
        self._on_message = Some(on_message);
    }

    /// Stop the worker and forget about its replies.
    pub fn cancel(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.terminate();
        }
        self._on_message = None;
        self.inbox.borrow_mut().clear();
    }

    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }

    /// Every reply that arrived since the last call, in order. The job ends
    /// with the final reply.
    pub fn drain(&mut self) -> Vec<WorkerReply> {
        if self.worker.is_none() {
            return Vec::new();
        }
        let replies: Vec<WorkerReply> = self.inbox.borrow_mut().drain(..).collect();
        if replies.iter().any(WorkerReply::is_final) {
            self.cancel();
        }
        replies
    }
}

fn spawn_worker() -> Result<Worker, JsValue> {
    let options = WorkerOptions::new();
    options.set_type(WorkerType::Module);
    Worker::new_with_options(WORKER_SCRIPT, &options)
}

/// Entry point called by `worker.js` for every message: runs the request on
/// the worker thread and posts the replies back to the page.
#[wasm_bindgen]
pub fn worker_handle(request: &[u8]) {
    let _ = console_log::init_with_level(log::Level::Info);
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let post = |reply: WorkerReply| match postcard::to_allocvec(&reply) {
        Ok(bytes) => {
            let array = js_sys::Uint8Array::from(bytes.as_slice());
            let transfer = js_sys::Array::of1(&array.buffer());
            if let Err(e) = scope.post_message_with_transfer(&array, &transfer) {
                log::error!("could not post a worker reply: {e:?}");
            }
        }
        Err(e) => log::error!("could not encode a worker reply: {e}"),
    };

    let request = match postcard::from_bytes::<WorkerRequest>(request) {
        Ok(request) => request,
        Err(e) => {
            log::error!("unreadable worker request: {e}");
            return;
        }
    };
    // Cancellation terminates the worker outright, so the flag never rises.
    let cancel = AtomicBool::new(false);
    let done = worker::execute(request, &cancel, &post);
    post(done);
}

/// Ask for an image file and load it from the bytes the browser hands back;
/// a page has no paths to load it from.
pub fn pick_image(ctx: DialogContext) {
    wasm_bindgen_futures::spawn_local(async move {
        let _guard = FileDialogGuard::new(ctx.dialog_open.clone());
        let dialog = rfd::AsyncFileDialog::new()
            .add_filter("Image files", &["png", "jpg", "jpeg", "bmp", "tga", "tiff"]);
        let Some(file) = dialog.pick_file().await else {
            return;
        };
        let bytes = file.read().await;
        ctx.send(AppStateRequest::LoadImageBytes {
            name: file.file_name(),
            bytes,
        });
    });
}

/// Hand `bytes` to the browser as a download named after `default_path`.
pub fn export_image(
    bytes: Vec<u8>,
    default_path: String,
    format: ExportFormat,
    _ctx: DialogContext,
) {
    let name = file_name_of(&default_path);
    match download(&bytes, &name, format.mime()) {
        Ok(()) => log::info!("Export completed: {name}"),
        Err(e) => log::error!("Export failed: {e:?}"),
    }
}

/// Ask for a `.qset` file and load it from the bytes the browser hands back.
pub fn pick_settings_file(ctx: DialogContext) {
    wasm_bindgen_futures::spawn_local(async move {
        let _guard = FileDialogGuard::new(ctx.dialog_open.clone());
        let dialog = rfd::AsyncFileDialog::new().add_filter(
            "QualetizeGUI Settings",
            &[SettingsBundle::get_settings_file_extension()],
        );
        let Some(file) = dialog.pick_file().await else {
            return;
        };
        let bytes = file.read().await;
        ctx.send(AppStateRequest::LoadSettingsBytes {
            name: file.file_name(),
            bytes,
        });
    });
}

/// Hand the settings to the browser as a download; a page cannot choose
/// where they land.
pub fn save_settings(bundle_json: String, default_name: &str, _ctx: DialogContext) {
    match download(bundle_json.as_bytes(), default_name, "application/json") {
        Ok(()) => log::info!("Settings saved successfully to: {default_name}"),
        Err(e) => log::error!("Failed to save settings: {e:?}"),
    }
}

/// The part of `path` after the last separator, which is all a download name
/// can carry.
fn file_name_of(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("output")
        .to_string()
}

/// Save `bytes` through the browser: a blob behind an object URL, clicked
/// through a detached `<a download>`.
fn download(bytes: &[u8], name: &str, mime: &str) -> Result<(), JsValue> {
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes).into());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let result = click_download(&url, name);
    // The blob stays alive until its URL is released, whether or not the
    // click went through.
    web_sys::Url::revoke_object_url(&url)?;
    result
}

fn click_download(url: &str, name: &str) -> Result<(), JsValue> {
    let document = web_sys::window()
        .ok_or("no window")?
        .document()
        .ok_or("no document")?;
    let anchor: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
    anchor.set_href(url);
    anchor.set_download(name);

    // Firefox only follows the click of an anchor that is in the document.
    let body = document.body().ok_or("no body")?;
    body.append_child(&anchor)?;
    anchor.click();
    body.remove_child(&anchor)?;
    Ok(())
}

/// The settings that outlive a reload, kept in the origin's local storage.
pub mod storage {
    /// Local storage is shared by everything on the origin, so the keys carry
    /// the application name.
    fn storage_key(key: &str) -> String {
        format!("qualetize_gui.{key}")
    }

    fn local_storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn load(key: &str) -> Option<String> {
        local_storage()?.get_item(&storage_key(key)).ok()?
    }

    pub fn save(key: &str, value: &str) -> Result<(), String> {
        let storage = local_storage().ok_or("no local storage")?;
        storage
            .set_item(&storage_key(key), value)
            .map_err(|e| format!("could not write {key} to local storage: {e:?}"))
    }
}
