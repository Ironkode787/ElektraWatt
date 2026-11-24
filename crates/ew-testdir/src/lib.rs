// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The temporary directory of a test, shared by the tests of every crate: [`Tmp`], which is removed when the test
//! ends, [`copy_dir`], and the paths of the test projects (the sample and the release fixture) and
//! the starter content in the source tree, the comparison of an expected file with its re-bless ([`compare_or_bless`],
//! [`bless`]), and a font file of any size for a project's typeface ([`font_of_size`]).
//! A dev-only helper crate without dependencies, so any crate may take it as
//! a dev-dependency. How each test binary opens the sample (in place, read-only, or a copy) is in its own support.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// The sample project in the source tree, written by `cargo xtask gen` from its UI script
/// (`fixtures/sample/sample.uiscript`; the test `sample` of `ew-viewmodel` checks it). A test that only reads opens it
/// in place, read-only; a test that writes works on a copy ([`Tmp::sample`]).
pub const SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/sample/sample.ew");

/// The release fixture in the source tree: one page, two devices, an author and a checker, the open revision V1.0
/// with its author (built and checked by the release tests of `ew-api`). Tests work on a copy.
pub const RELEASE_MIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/release-min");

/// The content tree of the source tree: the starter library and the templates.
pub const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

/// The test tables of the source tree: the parts table and the project tables the importers read.
pub const TABLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/tables");

/// The data rows of the test table `name` in [`TABLES`]: its lines but the header.
///
/// # Panics
///
/// When the table cannot be read.
pub fn table_rows(name: &str) -> usize {
    let path = Path::new(TABLES).join(name);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines().skip(1).filter(|l| !l.trim().is_empty()).count()
}

/// What the sample project holds, counted from its files: the totals a test compares a printed one with, so that a
/// change of the sample is not written again in the tests.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SampleFacts {
    /// Its terminals: the devices of kind `terminal`.
    pub terminals: usize,
    /// Its PLC channels.
    pub channels: usize,
}

/// The [`SampleFacts`] of [`SAMPLE`], read from its record files (one record a line, its kind first in `"$k"`).
///
/// # Panics
///
/// When a file of the sample cannot be read.
pub fn sample_facts() -> SampleFacts {
    fn lines(dir: &Path, out: &mut Vec<String>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                lines(&path, out);
            } else if path.extension().is_some_and(|x| x == "jsonl") {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                out.extend(text.lines().map(str::to_owned));
            }
        }
    }
    let mut records = Vec::new();
    lines(Path::new(SAMPLE), &mut records);
    let of = |kind: &str| format!("{{\"$k\":\"{kind}\"");
    SampleFacts {
        terminals: records
            .iter()
            .filter(|r| r.starts_with(&of("device")) && r.contains("\"kind\":\"terminal\""))
            .count(),
        channels: records.iter().filter(|r| r.starts_with(&of("channel"))).count(),
    }
}

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

    /// A copy of the sample project at `name` inside the directory, made now.
    pub fn sample(&self, name: &str) -> PathBuf {
        let target = self.0.join(name);
        copy_dir(Path::new(SAMPLE), &target);
        target
    }
}

/// The directory's path, so that a `Tmp` stands where its path is read (`dir.join(..)`, `&dir`).
impl std::ops::Deref for Tmp {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Tmp {
    fn as_ref(&self) -> &Path {
        &self.0
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
    let fail =
        |what: &str, path: &Path, e: std::io::Error| -> ! { panic!("cannot {what} {}: {e}", path.display()) };
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

/// Whether the expected files are rewritten instead of compared: `EW_BLESS=1`, the one switch of the tests that hold
/// an expected file.
// Allowed: the program boundary of a test, read where it is asked; the value never reaches a result.
#[allow(clippy::disallowed_methods)]
#[must_use]
pub fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// Compares `actual` with the expected file `path`, byte for byte; under [`bless`] writes it instead, its folder made
/// first.
///
/// # Panics
///
/// When the file cannot be read or written, or differs: the message names the file and `EW_BLESS=1`, and a text shows
/// both sides.
pub fn compare_or_bless(path: &Path, actual: impl AsRef<[u8]>) {
    let actual = actual.as_ref();
    if bless() {
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                panic!("cannot create {}: {e}", parent.display());
            }
        }
        if let Err(e) = std::fs::write(path, actual) {
            panic!("cannot write {}: {e}", path.display());
        }
        return;
    }
    let expected =
        std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 writes it)", path.display()));
    if expected != actual {
        match (std::str::from_utf8(&expected), std::str::from_utf8(actual)) {
            (Ok(expected), Ok(actual)) => {
                assert_eq!(
                    actual,
                    expected,
                    "{} differs (EW_BLESS=1 rewrites it)",
                    path.display()
                );
            }
            _ => panic!("{} differs (EW_BLESS=1 rewrites it)", path.display()),
        }
    }
}

/// A font file of the repository a test may take as a project's typeface (Routed Gothic, OFL-1.1, a bundled file).
pub const FONT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/RoutedGothic-Regular.ttf"
);

/// [`FONT`] grown to `len` bytes or a little more, still a valid font of the same face: one more table (`zpad`, zeros,
/// which no reader asks for) is appended to the table directory and its data at the end, every other table's offset
/// moved by the directory's new record. A large typeface for a test, of the size of the vendors' text faces, without a
/// vendor's file in the repository.
///
/// # Panics
///
/// When the file cannot be read or its table directory is cut short.
pub fn font_of_size(len: usize) -> Vec<u8> {
    let font = std::fs::read(FONT).expect("the repository's font file");
    let tables = usize::from(u16::from_be_bytes([font[4], font[5]]));
    let directory = 12 + 16 * tables;
    let mut out = font[..directory].to_vec();
    out[4..6].copy_from_slice(&u16::try_from(tables + 1).expect("few tables").to_be_bytes());
    // Every table's offset moves by the one new record of 16 bytes.
    for t in 0..tables {
        let at = 12 + 16 * t + 8;
        let offset = u32::from_be_bytes(out[at..at + 4].try_into().expect("four bytes")) + 16;
        out[at..at + 4].copy_from_slice(&offset.to_be_bytes());
    }
    let data_end = font.len() + 16;
    let pad_at = data_end.next_multiple_of(4);
    let pad_len = len.saturating_sub(pad_at).max(4);
    out.extend_from_slice(b"zpad");
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&u32::try_from(pad_at).expect("a small file").to_be_bytes());
    out.extend_from_slice(&u32::try_from(pad_len).expect("a small file").to_be_bytes());
    out.extend_from_slice(&font[directory..]);
    out.resize(pad_at + pad_len, 0);
    out
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
