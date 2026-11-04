// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The counting allocator of the hostile corpus test (WP-S6.04 D7, D10; ID-1334): the system allocator, with the bytes
//! live and their peak counted, so that the test bounds each refusal's extra peak allocation (NFR-SEC-02). It is this
//! test binary's global allocator only; `ew-testkit`'s library keeps `#![forbid(unsafe_code)]`, and this is the one file
//! of the crate that `ci/unsafe-allowlist.toml` lists. No time is measured (owner's rule).

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The system allocator, counting.
pub struct Counting;

/// The bytes allocated and not yet freed.
static LIVE: AtomicUsize = AtomicUsize::new(0);
/// The most bytes live at once since the last [`extra_peak`] started.
static PEAK: AtomicUsize = AtomicUsize::new(0);
/// One measurement at a time: the counters are the process's.
static MEASURING: Mutex<()> = Mutex::new(());

fn grow(n: usize) {
    let live = LIVE.fetch_add(n, Ordering::SeqCst) + n;
    PEAK.fetch_max(live, Ordering::SeqCst);
}

fn shrink(n: usize) {
    LIVE.fetch_sub(n, Ordering::SeqCst);
}

// SAFETY: every method passes its arguments to `System` unchanged and returns what `System` returns, so `Counting`
// keeps `GlobalAlloc`'s contract as `System` does; the counting itself is atomic arithmetic and never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller keeps `alloc`'s contract (a layout of non-zero size), which `System.alloc` asks for.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as `alloc`: the caller's layout is of non-zero size.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller passes a block this allocator returned, so one of `System`, with the layout it was
        // allocated with.
        unsafe { System.dealloc(ptr, layout) };
        shrink(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller passes a block of `System` (through this allocator) with its layout and a non-zero new
        // size that does not overflow when rounded to the alignment, as `System.realloc` asks.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            // Both blocks may be live at once while the bytes move: count the new one first.
            grow(new_size);
            shrink(layout.size());
        }
        new
    }
}

/// Runs `f` and returns its result with the most bytes it held at once beyond those live when it started (all threads
/// of the process count; measurements run one at a time).
pub fn extra_peak<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let _one = MEASURING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let start = LIVE.load(Ordering::SeqCst);
    PEAK.store(start, Ordering::SeqCst);
    let out = f();
    let peak = PEAK.load(Ordering::SeqCst);
    (out, peak.saturating_sub(start))
}
