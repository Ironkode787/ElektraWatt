// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The temporary directory of a test, shared by the tests of every crate: [`Tmp`], which is removed when the test
//! ends, [`copy_dir`], and the paths of the golden project and the starter content in the source tree. A dev-only
//! helper crate without dependencies, so any crate may take it as a dev-dependency.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Golden A in the source tree. A test never opens it in a writing or locking mode: it works on a copy
/// ([`Tmp::golden`]).
pub const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// The content tree of the source tree: the starter library and the templates.
pub const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

/// A fresh temporary directory ([`fresh_dir`]), removed on drop.
pub struct Tmp(pub PathBuf);

impl Tmp {
    /// A new, empty directory whose name starts with `stem`.
    pub fn new(stem: &str) -> Tmp {
        Tmp(fresh_dir(stem))
    }

    /// The directory.
    pub fn dir(&self) -> &Path {
        &self.0
    }

    /// The path of `name` inside the directory.
    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A copy of Golden A at `name` inside the directory, made now.
    pub fn golden(&self, name: &str) -> PathBuf {
        let target = self.0.join(name);
        copy_dir(Path::new(GOLDEN_A), &target);
        target
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Copies the directory tree `from` to `to`, creating `to` and its parents.
///
/// # Panics
///
/// When a directory cannot be read or created or a file cannot be copied: a test cannot go on without its files.
pub fn copy_dir(from: &Path, to: &Path) {
    let fail = |what: &str, path: &Path, e: std::io::Error| -> ! { panic!("cannot {what} {}: {e}", path.display()) };
    if let Err(e) = std::fs::create_dir_all(to) {
        fail("create", to, e);
    }
    let entries = std::fs::read_dir(from).unwrap_or_else(|e| fail("read", from, e));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| fail("read", from, e));
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target);
        } else if let Err(e) = std::fs::copy(&source, &target) {
            fail("copy", &source, e);
        }
    }
}

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

    use super::{Tmp, copy_dir, fresh_dir, fresh_dir_in};

    /// A tree is copied with its subdirectories, and the temporary directory is gone when its `Tmp` is dropped.
    #[test]
    fn a_tree_is_copied_and_the_directory_is_removed_on_drop() {
        let tmp = Tmp::new("ew-testdir-copy");
        let dir = tmp.dir().to_path_buf();
        std::fs::create_dir_all(tmp.path("a").join("sub")).unwrap();
        std::fs::write(tmp.path("a").join("sub").join("f.txt"), "x").unwrap();
        copy_dir(&tmp.path("a"), &tmp.path("b").join("c"));
        assert_eq!(
            std::fs::read_to_string(tmp.path("b").join("c").join("sub").join("f.txt")).unwrap(),
            "x"
        );
        drop(tmp);
        assert!(!dir.exists());
    }

    /// A leftover directory of the same PID and number (a killed run's settings and project lock) is
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
