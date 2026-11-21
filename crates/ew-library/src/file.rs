// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one all-or-nothing file write of the program: the bytes go to a temporary file beside the target, are synced
//! and renamed over it, so a failure, a cancel or a crash never leaves a partial file at the target's name. A set of
//! files (the SVG, PNG and DXF exports' pages) replaces its directory as a whole the same way ([`write_file_set`]).
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

/// Why a set of files does not replace a directory ([`write_file_set`]).
#[derive(Debug)]
pub enum FileSetError {
    /// The directory holds this entry (its name), which is no file of the set's extensions: nothing is written or
    /// removed.
    Foreign(String),
    /// Reading or writing `path` failed.
    Io {
        /// The file or directory.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
}

/// The first entry of `dir` that is no file with one of the extensions `exts` (its name); `None` for a directory that
/// holds only such files, or none.
///
/// # Errors
///
/// [`FileSetError::Io`] when the directory cannot be read.
pub fn foreign_entry(dir: &Path, exts: &[&str]) -> Result<Option<String>, FileSetError> {
    let at = |path: &Path| {
        let path = path.to_path_buf();
        move |error| FileSetError::Io { path, error }
    };
    if !dir.exists() {
        return Ok(None);
    }
    for e in std::fs::read_dir(dir).map_err(at(dir))? {
        let path = e.map_err(at(dir))?.path();
        if !path.is_file() || path.extension().is_none_or(|x| !exts.iter().any(|ext| x == *ext)) {
            return Ok(Some(
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            ));
        }
    }
    Ok(None)
}

/// `path` with `suffix` appended to its last component.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Writes the files `files` (name and bytes) into the directory `dir` as a whole: they go to a fresh sibling directory
/// first, each synced, which then replaces `dir`, so `dir` holds exactly this set (no stale page of an earlier one) and
/// a failure leaves the earlier set as it was. A directory that holds anything but files with the extensions `exts` is
/// refused, so no other file is ever removed (`ew export svg|png|dxf`, the window's DXF export).
///
/// # Errors
///
/// [`FileSetError::Foreign`] for such a directory; [`FileSetError::Io`] for the first step that failed, after which the
/// sibling directory is removed and `dir` is as it was.
pub fn write_file_set(dir: &Path, exts: &[&str], files: &[(&str, &[u8])]) -> Result<(), FileSetError> {
    if let Some(name) = foreign_entry(dir, exts)? {
        return Err(FileSetError::Foreign(name));
    }
    let at = |path: &Path, error: io::Error| FileSetError::Io {
        path: path.to_path_buf(),
        error,
    };
    let tmp = with_suffix(dir, &format!(".tmp-{}", std::process::id()));
    let old = with_suffix(dir, &format!(".old-{}", std::process::id()));
    let written = std::fs::create_dir_all(&tmp)
        .map_err(|e| at(&tmp, e))
        .and_then(|()| {
            files.iter().try_for_each(|(name, bytes)| {
                let path = tmp.join(name);
                std::fs::File::create(&path)
                    .and_then(|mut f| f.write_all(bytes).and_then(|()| f.sync_all()))
                    .map_err(|e| at(&path, e))
            })
        });
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let had = dir.exists();
    if had && let Err(e) = std::fs::rename(dir, &old) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(at(dir, e));
    }
    if let Err(e) = std::fs::rename(&tmp, dir) {
        if had {
            let _ = std::fs::rename(&old, dir);
        }
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(at(dir, e));
    }
    if had {
        let _ = std::fs::remove_dir_all(&old);
    }
    Ok(())
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

    /// A set replaces its directory as a whole (no stale file of the earlier set stays), and a directory that holds
    /// another file is refused and left as it was.
    #[test]
    fn a_file_set_replaces_its_directory_and_a_foreign_file_refuses_it() {
        let root = ew_testdir::Tmp::new("ew-library-file-set");
        let dir = root.join("set");
        let names = |d: &Path| {
            let mut n: Vec<String> = std::fs::read_dir(d)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            n.sort();
            n
        };
        write_file_set(&dir, &["dxf", "png"], &[("1.dxf", b"a"), ("2.dxf", b"b"), ("p.png", b"c")]).unwrap();
        write_file_set(&dir, &["dxf", "png"], &[("1.dxf", b"A")]).unwrap();
        assert_eq!(names(&dir), ["1.dxf"]);
        assert_eq!(std::fs::read(dir.join("1.dxf")).unwrap(), b"A");
        std::fs::write(dir.join("notes.txt"), b"mine").unwrap();
        assert!(matches!(
            write_file_set(&dir, &["dxf", "png"], &[("1.dxf", b"x")]),
            Err(FileSetError::Foreign(n)) if n == "notes.txt"
        ));
        assert_eq!(names(&dir), ["1.dxf", "notes.txt"]);
        assert_eq!(names(&root), ["set"], "no sibling directory is left");
    }
}
