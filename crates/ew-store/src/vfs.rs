// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The I/O seam of doc 05 §5.6: every file operation of `ew-store` goes through [`Vfs`], so that the
//! fault-injection harness can number, fail and "power off" each call ([`crate::mem::MemVfs`]).

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub use ew_format::EntryKind;

/// What [`Vfs::stat`] reports about a path, without following links.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stat {
    /// The entry kind.
    pub kind: EntryKind,
    /// The length in bytes (files).
    pub len: u64,
    /// The modification time, where the file system has one.
    pub modified: Option<SystemTime>,
}

/// The file operations the store uses (doc 05 §5.6). Each call is one numbered step of the fault-injection
/// harness, so the save protocol's durability argument is made over these calls only:
///
/// - data reach the disk only through [`Vfs::sync`] of the file;
/// - a name (created, renamed or removed) reaches the disk only through [`Vfs::sync_dir`] of its directory
///   (on Windows NTFS's metadata log makes renames durable without it, doc 05 §5.2 step 5);
/// - [`Vfs::rename`] replaces its target atomically.
pub trait Vfs: Send + Sync + fmt::Debug {
    /// The whole content of a regular file.
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// Up to `len` bytes of a regular file from byte `offset` (fewer at its end, none past it): the chunked reads of
    /// a scan that must not hold a whole file in memory (the asset purge, FT-380). The default reads the whole file.
    fn read_at(&self, path: &Path, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let all = self.read(path)?;
        let start = usize::try_from(offset).unwrap_or(usize::MAX).min(all.len());
        let end = start.saturating_add(len).min(all.len());
        Ok(all[start..end].to_vec())
    }
    /// The kind, size and modification time of a path, without following a link; `NotFound` when absent.
    fn stat(&self, path: &Path) -> io::Result<Stat>;
    /// The entries of a directory with their kinds (links not followed), sorted by name; `NotFound` when
    /// the directory is absent.
    fn list(&self, dir: &Path) -> io::Result<Vec<(String, EntryKind)>>;
    /// Creates a directory and its missing parents.
    fn create_dir_all(&self, dir: &Path) -> io::Result<()>;
    /// Creates a new file with `data` (`O_EXCL`, SMB2 `FILE_CREATE`); `AlreadyExists` when the path exists.
    /// Nothing is durable until [`Vfs::sync`] and [`Vfs::sync_dir`].
    fn create_new(&self, path: &Path, data: &[u8]) -> io::Result<()>;
    /// Appends `data` to a file, creating it when absent. Not durable until [`Vfs::sync`].
    fn append(&self, path: &Path, data: &[u8]) -> io::Result<()>;
    /// Makes a file's data durable (`sync_all`; `F_FULLFSYNC` on Apple platforms through std).
    fn sync(&self, path: &Path) -> io::Result<()>;
    /// Makes a directory's entries durable; a no-op where the platform has no directory sync (Windows).
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;
    /// Renames `from` over `to`, replacing it atomically.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    /// Renames `from` to `to` without replacing: `AlreadyExists` when `to` exists, `Unsupported` where the
    /// file system cannot do it atomically (doc 05 §7.4 step 3: the caller falls back to [`Vfs::rename`]).
    fn rename_noreplace(&self, from: &Path, to: &Path) -> io::Result<()>;
    /// Removes a file.
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    /// Removes an empty directory.
    fn remove_dir(&self, dir: &Path) -> io::Result<()>;
    /// Sets a file's modification time (the lock heartbeat, doc 05 §7.2).
    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()>;
    /// The canonical absolute form of an existing path, links and `.` / `..` resolved: the key of the
    /// project's journal directory, so that one project reached through two paths (a mapped drive and its
    /// UNC path, a symbolic link) has one journal (doc 05 §8.1).
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
}

/// The project-relative path `rel` (with `/` separators) below `root`.
pub fn join(root: &Path, rel: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in rel.split('/') {
        path.push(segment);
    }
    path
}

/// The production [`Vfs`]: `std::fs`.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsVfs;

fn kind_of(ft: fs::FileType) -> EntryKind {
    if ft.is_symlink() {
        EntryKind::Link
    } else if ft.is_file() {
        EntryKind::File
    } else if ft.is_dir() {
        EntryKind::Dir
    } else {
        EntryKind::Other
    }
}

impl Vfs for OsVfs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        // Read through a handle opened on the path itself, after checking that the path is a regular file:
        // links are never followed (NFR-SEC-16).
        let meta = fs::symlink_metadata(path)?;
        if !meta.file_type().is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"));
        }
        fs::read(path)
    }

    fn read_at(&self, path: &Path, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let meta = fs::symlink_metadata(path)?;
        if !meta.file_type().is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"));
        }
        let mut file = fs::File::open(path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut out = Vec::with_capacity(len.min(1 << 20));
        file.take(u64::try_from(len).unwrap_or(u64::MAX))
            .read_to_end(&mut out)?;
        Ok(out)
    }

    fn stat(&self, path: &Path) -> io::Result<Stat> {
        let meta = fs::symlink_metadata(path)?;
        Ok(Stat {
            kind: kind_of(meta.file_type()),
            len: meta.len(),
            modified: meta.modified().ok(),
        })
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<(String, EntryKind)>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let Ok(name) = entry.file_name().into_string() else {
                // A name that is not Unicode is never an ElektraWatt name.
                continue;
            };
            out.push((name, kind_of(entry.file_type()?)));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)
    }

    fn create_new(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(data)
    }

    fn append(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().append(true).create(true).open(path)?;
        file.write_all(data)
    }

    fn sync(&self, path: &Path) -> io::Result<()> {
        // A write handle: FlushFileBuffers on Windows needs one.
        OpenOptions::new().write(true).open(path)?.sync_all()
    }

    #[cfg(unix)]
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        fs::File::open(dir)?.sync_all()
    }

    #[cfg(not(unix))]
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        // Doc 05 §5.2 step 5: directory sync is unavailable on Windows; the NTFS metadata log is relied on.
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn rename_noreplace(&self, from: &Path, to: &Path) -> io::Result<()> {
        // A hard link is created atomically and fails when `to` exists (POSIX `link`, Windows
        // `CreateHardLinkW`), so linking then unlinking is a rename without replacement. File systems
        // without hard links (FAT, some SMB and NFS mounts) report an error other than `AlreadyExists`,
        // which is mapped to `Unsupported` for the caller's fallback (doc 05 §7.4 step 3).
        match fs::hard_link(from, to) {
            Ok(()) => fs::remove_file(from),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(e),
            Err(e) => Err(io::Error::new(io::ErrorKind::Unsupported, e)),
        }
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir(&self, dir: &Path) -> io::Result<()> {
        fs::remove_dir(dir)
    }

    fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()> {
        OpenOptions::new().write(true).open(path)?.set_modified(time)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        Ok(without_verbatim_prefix(fs::canonicalize(path)?))
    }
}

/// A Windows verbatim path in its plain form, which the recovery dialog shows: `\\?\C:\x` as `C:\x`,
/// `\\?\UNC\server\share\x` as `\\server\share\x`; other paths unchanged.
fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path;
    };
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

/// Whether an I/O error is one that Windows antivirus, indexers and sync clients cause while they hold a
/// file (doc 05 §5.2 step 6: `ERROR_SHARING_VIOLATION`, `ERROR_ACCESS_DENIED`), worth a retry.
pub fn is_transient(e: &io::Error) -> bool {
    cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 5))
        || e.kind() == io::ErrorKind::Interrupted
        || e.kind() == io::ErrorKind::ResourceBusy
}
