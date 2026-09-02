//! Background work on a thread, replies through a channel.

use crate::worker::{self, WorkerReply, WorkerRequest};
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
