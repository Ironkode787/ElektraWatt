// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The temporary directory of a test (FT-524, FT-538), shared by `common` and the test files with their own `Tmp`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A new, empty directory `<temp>/<stem>-<pid>-<n>` that this call created. A test process that is killed (a cancelled
/// or timed-out run) leaves its directory behind with the project's lock, the journal and the settings in it; a later
/// test process with the same PID (Windows and Linux reuse them) used to adopt it through `create_dir_all`, and opened
/// the project read-only or with a recovery offer, or (with `if !dir.exists()`) never copied the project at all.
/// `create_dir` fails on such a leftover, and the next number is taken instead; the leftover is not touched.
pub fn fresh_dir(stem: &str) -> PathBuf {
    fresh_dir_in(&std::env::temp_dir(), stem, &COUNTER)
}

/// [`fresh_dir`] under `parent` with the numbers of `counter`.
pub fn fresh_dir_in(parent: &Path, stem: &str, counter: &AtomicU32) -> PathBuf {
    loop {
        let n = counter.fetch_add(1, Ordering::Relaxed);
        let dir = parent.join(format!("{stem}-{}-{n}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot create the test directory {}: {e}", dir.display()),
        }
    }
}
