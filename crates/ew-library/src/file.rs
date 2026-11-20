// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one all-or-nothing file write of the program: the bytes go to a temporary file beside the target, are synced
//! and renamed over it, so a failure, a cancel or a crash never leaves a partial file at the target's name.
//!
//! It lives here because `ew-library` is the lowest crate that every writer reaches under `ci/dep-rules.toml` (the
//! command-line program, the view model and `ew-api` through `ew_api::library`); the crates below it do no file I/O.
//! Not here: the project save and the release archive (`ew-store`, through its `Vfs`, which never replace a file of
//! another writer), and the backup and symbol exports of `ew-api` (written without replacing anything).

use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

/// Writes `bytes` to `path` all or nothing: [`temp_beside`] written and synced, then renamed over `path` (an existing
/// file is replaced) and the folder synced where the platform can; the temporary file is removed on a failure.
///
/// # Errors
///
/// The I/O error of the first step that failed.
pub fn write_replacing(path: &Path, bytes: &[u8]) -> io::Result<()> {
    Staged::write(temp_beside(path), path, bytes, &OpenOptions::new())?.commit()
}

/// The temporary name of `path` for this process: `<file name>.tmp-<process id>` in its folder, so that two
/// programs writing one file never write into each other's temporary file.
pub fn temp_beside(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".tmp-{}", std::process::id()));
    path.with_file_name(name)
}

/// Bytes written to a temporary file beside their target and synced, not yet in its place. Dropped before
/// [`Staged::commit`] (a failure, a cancel, a panic's unwinding), it removes the temporary file.
pub struct Staged {
    temp: PathBuf,
    target: PathBuf,
    armed: bool,
}

impl Staged {
    /// Writes `bytes` to the new file `temp` (in the folder of `target`; a leftover at that name is removed first),
    /// created by `options` (with write and create-new set here: a user-only mode stays the creator's), and syncs it.
    ///
    /// # Errors
    ///
    /// The I/O error of the first step that failed; the temporary file is removed then.
    pub fn write(temp: PathBuf, target: &Path, bytes: &[u8], options: &OpenOptions) -> io::Result<Staged> {
        let _ = std::fs::remove_file(&temp);
        let mut file = options.clone().write(true).create_new(true).open(&temp)?;
        let staged = Staged {
            temp,
            target: target.to_path_buf(),
            armed: true,
        };
        let written = file.write_all(bytes).and_then(|()| file.sync_all());
        // Closed before a failure drops `staged`, which removes the file.
        drop(file);
        written.map(|()| staged)
    }

    /// Renames the temporary file over the target (an existing file is replaced) and syncs the folder on Unix (best
    /// effort: Windows has no folder sync, and some file systems refuse one).
    ///
    /// # Errors
    ///
    /// The rename's I/O error; the temporary file is removed then.
    pub fn commit(mut self) -> io::Result<()> {
        std::fs::rename(&self.temp, &self.target)?;
        self.armed = false;
        let dir = match self.target.parent() {
            Some(d) if !d.as_os_str().is_empty() => d,
            _ => Path::new("."),
        };
        if cfg!(unix)
            && let Ok(f) = std::fs::File::open(dir)
        {
            let _ = f.sync_all();
        }
        Ok(())
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.temp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The write every writer of the program shares: it replaces an existing file and leaves no temporary file, and a
    /// staged write dropped before its commit leaves the target as it was.
    #[test]
    fn a_write_replaces_the_target_and_an_uncommitted_one_leaves_it() {
        let dir = ew_testdir::Tmp::new("ew-library-file");
        let target = dir.join("out.txt");
        std::fs::write(&target, b"old").unwrap();
        write_replacing(&target, b"new").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let staged = Staged::write(temp_beside(&target), &target, b"never", &OpenOptions::new()).unwrap();
        assert!(temp_beside(&target).is_file());
        drop(staged);
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["out.txt"]);
        assert!(write_replacing(&dir.join("missing/out.txt"), b"x").is_err());
    }
}
