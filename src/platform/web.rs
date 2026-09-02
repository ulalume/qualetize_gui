//! Background work in a Web Worker running a second instance of this wasm
//! module, replies through `postMessage`.
//!
//! `worker.js` initializes the module and forwards every message to
//! [`worker_handle`]. Requests and replies travel as `postcard` bytes in a
//! transferred `ArrayBuffer`.

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
