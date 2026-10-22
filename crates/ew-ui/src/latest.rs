// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One long-lived worker thread with a latest-request-wins slot (WP-S2.01 part 1 review ruling): a request that
//! arrives while another is being worked on replaces any request still waiting, so a burst of requests (the
//! Page Down key held) costs one render in flight and at most one more, never a thread and an image per request.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

struct State<T> {
    pending: Option<T>,
    closed: bool,
}

type Shared<T> = Arc<(Mutex<State<T>>, Condvar)>;

fn lock<T>(shared: &Shared<T>) -> MutexGuard<'_, State<T>> {
    shared.0.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A worker thread that runs a handler on the latest request.
pub(crate) struct LatestWorker<T: Send + 'static> {
    shared: Shared<T>,
    /// Kept so the thread is not detached by accident; it is not joined (a render in progress ends by itself).
    _thread: JoinHandle<()>,
}

impl<T: Send + 'static> LatestWorker<T> {
    /// Starts the thread; `handler` runs on it, one request at a time.
    pub(crate) fn new(mut handler: impl FnMut(T) + Send + 'static) -> LatestWorker<T> {
        let shared: Shared<T> = Arc::new((
            Mutex::new(State {
                pending: None,
                closed: false,
            }),
            Condvar::new(),
        ));
        let theirs = Arc::clone(&shared);
        let thread = thread::spawn(move || {
            loop {
                let request = {
                    let mut state = lock(&theirs);
                    loop {
                        if state.closed {
                            return;
                        }
                        if let Some(request) = state.pending.take() {
                            break request;
                        }
                        state = theirs.1.wait(state).unwrap_or_else(PoisonError::into_inner);
                    }
                };
                handler(request);
            }
        });
        LatestWorker {
            shared,
            _thread: thread,
        }
    }

    /// Makes `request` the one to work on next, dropping a request that was still waiting.
    pub(crate) fn submit(&self, request: T) {
        lock(&self.shared).pending = Some(request);
        self.shared.1.notify_one();
    }
}

impl<T: Send + 'static> Drop for LatestWorker<T> {
    fn drop(&mut self) {
        lock(&self.shared).closed = true;
        self.shared.1.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_burst_of_requests_renders_the_first_and_the_last_only() {
        let (tx, rx) = channel();
        let worker = LatestWorker::new(move |n: u32| {
            // The first request keeps the worker busy while the rest arrive.
            if n == 0 {
                thread::sleep(Duration::from_millis(150));
            }
            tx.send(n).unwrap();
        });
        worker.submit(0);
        // Let the worker take request 0 before the burst.
        thread::sleep(Duration::from_millis(30));
        for n in 1..=50 {
            worker.submit(n);
        }
        let mut done = Vec::new();
        while let Ok(n) = rx.recv_timeout(Duration::from_millis(600)) {
            done.push(n);
            if n == 50 {
                break;
            }
        }
        assert_eq!(done, [0, 50], "the 49 requests in between were replaced");
    }

    #[test]
    fn a_dropped_worker_stops_its_thread() {
        let (tx, rx) = channel::<u32>();
        let worker = LatestWorker::new(move |n: u32| tx.send(n).unwrap());
        worker.submit(1);
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)), Ok(1));
        drop(worker);
        // The thread has ended: its sender is dropped, so the channel reports the disconnect.
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
