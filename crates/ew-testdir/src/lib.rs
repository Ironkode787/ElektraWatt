// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The temporary directory of a test (FT-524, FT-538), shared by the tests of every crate (FT-569, FT-586). A dev-only
//! helper crate without dependencies, so any crate may take it as a dev-dependency.

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

/// [`fresh_dir`] under `parent` with the numbers of `counter`. `parent` is created first when it does not exist yet
/// (`CARGO_TARGET_TMPDIR` of a test run from a nextest archive on a fresh machine has no `target/tmp`); only the leaf
/// must be new.
pub fn fresh_dir_in(parent: &Path, stem: &str, counter: &AtomicU32) -> PathBuf {
    if let Err(e) = std::fs::create_dir_all(parent) {
        panic!(
            "cannot create the parent of the test directories {}: {e}",
            parent.display()
        );
    }
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;

    use super::{fresh_dir, fresh_dir_in};

    /// FT-524, FT-538: a leftover directory of the same PID and number (a killed run's settings and project lock) is
    /// skipped, not adopted, and left as it was.
    #[test]
    fn a_leftover_directory_of_the_same_pid_is_not_adopted() {
        let parent = fresh_dir("ew-fresh-dir");
        let pid = std::process::id();
        let leftover = parent.join(format!("t-{pid}-0"));
        std::fs::create_dir_all(leftover.join("g.ew")).unwrap();
        std::fs::write(leftover.join("g.ew").join(".~ew.lock"), "{}\n").unwrap();
        std::fs::write(leftover.join("settings.toml"), "canvas_backend = \"cpu\"\n").unwrap();
        let counter = AtomicU32::new(0);
        let dir = fresh_dir_in(&parent, "t", &counter);
        assert_eq!(dir, parent.join(format!("t-{pid}-1")));
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "a new, empty directory"
        );
        assert!(
            leftover.join("g.ew").join(".~ew.lock").is_file(),
            "the leftover is not touched"
        );
        assert_eq!(
            fresh_dir_in(&parent, "t", &counter),
            parent.join(format!("t-{pid}-2"))
        );
        std::fs::remove_dir_all(&parent).unwrap();
    }

    /// A parent that does not exist yet (`target/tmp` of a test run from a nextest archive on a fresh CI runner) is
    /// created, and the result is a new, empty directory in it.
    #[test]
    fn a_missing_parent_is_created() {
        let root = fresh_dir("ew-fresh-dir-parent");
        let parent = root.join("target").join("tmp");
        assert!(!parent.exists());
        let counter = AtomicU32::new(0);
        let dir = fresh_dir_in(&parent, "t", &counter);
        assert_eq!(dir, parent.join(format!("t-{}-0", std::process::id())));
        assert!(dir.is_dir());
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "a new, empty directory"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
