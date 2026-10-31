// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The as-built archive's file (doc 05 §14.1 step 4, §14.2; WP-S1.18 D11, D13): its name, its layout as a
//! deterministic ZIP ([`crate::zip`]), its check, and the atomic write through the [`Vfs`] seam.
//!
//! **Layout.** `mimetype` first and stored (`application/vnd.elektrawatt.archive+zip`), then every other entry
//! deflated in byte order of its path: `ARCHIVE.json`, `SHA256SUMS`, `check/report.json`, `pdf/…`, `project/…`
//! (the canonical project directory of the released generation: `manifest.json` and every file it lists, `lib/` and
//! `assets/` included) and `reports/…`. `SHA256SUMS` holds one `sha256sum` line (`<64 hex digits><2 spaces><path>`)
//! per other entry, `mimetype` included, in byte order of the paths, so `sha256sum -c SHA256SUMS` verifies an
//! extracted archive. The caller composes the entries ([`build`] adds `mimetype` and `SHA256SUMS`).
//!
//! **Name and place.** `<label>.ewarchive` with the label sanitised ([`sanitise`]: characters outside
//! `[A-Za-z0-9._-]` become `_`, a Windows reserved device name gets a `_` prefix), in the archive root, by default
//! `<parent>/<name>.ew-archive/` beside the project directory `<parent>/<name>.ew` ([`default_root`]); never inside
//! the project (ADR-0013).
//!
//! **The atomic write** ([`write_atomically`], doc 05 §14.1 step 4): the root is created; a leftover temp of the same
//! archive name and the same owner (`<name>.tmp-<owner>-<digits>`, a crashed release of the same project's) is
//! removed, and a temp of any other owner is never touched (a root may be shared, and another project's release may be
//! writing in it); an existing archive of that name is never replaced ([`WriteError::Exists`]: archives are
//! immutable, FR-REV-009; a complete unreferenced one is the next open's "incomplete release"). The archive is written
//! as `<name>.tmp-<owner>-<g>`, synced with its directory, read back through the seam, compared byte for byte with what
//! was written and checked by the caller's verification (the sums and the project's hashes against manifest g), then
//! (after the run record, below) put into place without replacement and the directory synced: a hard link, which keeps
//! the temp's modification time, else, on a file system without hard links, the target created anew (`create_new`,
//! never over an existing file: another copy's archive is never replaced) with the temp's modification time set. Any
//! failure removes the temp, the record and an archive this call created, and never a file it did not create. The
//! archive's name is therefore durable before the caller stamps the release.
//!
//! The owner is the caller's marker of the project directory (the release uses the directory's key, [`path_key`]):
//! only one run of a project directory writes at a time (its write lock), so a temp that carries the owner is a crashed
//! run's, never a live one's. A copy of the directory elsewhere (an operating-system copy, a restore, a Save as)
//! carries the same project ID but has another key, so neither removes the other's temp (FT-814).
//!
//! **The run record** ([`RunRecord`], FT-814). Before its archive has its name (the temp written and verified), a
//! release run records durably in the per-user state directory, outside the project, that this project directory's run
//! writes it: the archive's path, its SHA-256 and its modification time (the temp's, which the archive keeps), under
//! `<state>/releases/<key of the directory>-<key of the archive>.json` ([`run_record_path`]), so a crash right after
//! the rename leaves an archive the directory can prove its own. The run removes the record when it is settled; a
//! record whose archive is gone or released is stale, and the release removes it at the next open
//! ([`run_records_of`]). A discard of an incomplete release requires a
//! matching record: a copy of the directory names the same project and, at the same generation, even writes the same
//! archive bytes, but it does not carry the record, so it never discards the other copy's archive.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use ew_format::{FileHash, Manifest, hash_text, sha256};

use crate::vfs::{EntryKind, Vfs};
use crate::zip::{self, Method, ZipError, ZipLimits};

/// The media type in the `mimetype` entry.
pub const MIMETYPE: &str = "application/vnd.elektrawatt.archive+zip";
/// The first entry, stored.
pub const MIMETYPE_ENTRY: &str = "mimetype";
/// The checksum entry.
pub const SUMS_ENTRY: &str = "SHA256SUMS";
/// The archive's own description (project, revision, generation, release context, the release record's facts).
pub const DESCRIPTION_ENTRY: &str = "ARCHIVE.json";
/// The prefix of the project directory's entries.
pub const PROJECT_PREFIX: &str = "project/";
/// The extension of an archive file.
pub const EXTENSION: &str = "ewarchive";
/// The suffix of the default archive root beside the project directory.
pub const ROOT_SUFFIX: &str = ".ew-archive";

/// `text` as a file-name part (doc 05 §14.2): every character outside `[A-Za-z0-9._-]` becomes `_`, and a Windows
/// reserved device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1` … `COM9`, `LPT1` … `LPT9`, any case, before the first
/// `.`) gets a `_` prefix.
///
/// ```
/// use ew_store::archive::sanitise;
///
/// assert_eq!(sanitise("V2.0"), "V2.0");
/// assert_eq!(sanitise("Rev A/1 ü"), "Rev_A_1__");
/// assert_eq!(sanitise("con"), "_con");
/// assert_eq!(sanitise("LPT1.x"), "_LPT1.x");
/// ```
pub fn sanitise(text: &str) -> String {
    let out: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = out.split('.').next().unwrap_or_default().to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    if reserved { format!("_{out}") } else { out }
}

/// The archive's file name for a release labelled `label`: `<sanitised label>.ewarchive`.
pub fn file_name(label: &str) -> String {
    format!("{}.{EXTENSION}", sanitise(label))
}

/// The temp name of the archive of `label` written by `owner` for generation `generation`:
/// `<file name>.tmp-<owner>-<g>`, with the owner sanitised ([`sanitise`]). The release's owner is the key of the
/// project directory ([`path_key`]).
pub fn temp_name(label: &str, owner: &str, generation: u64) -> String {
    format!("{}.tmp-{}-{generation}", file_name(label), sanitise(owner))
}

/// The default archive root of the project directory `project`: `<parent>/<name>.ew-archive`, where `<name>` is the
/// directory's name without a trailing `.ew` (doc 05 §1, §14.2). A path without a name (`.`, `a/..`) is
/// [`resolved`] first. `None` for a path without a parent or a name even then (a file-system root).
///
/// ```
/// use std::path::Path;
/// use ew_store::archive::default_root;
///
/// assert_eq!(default_root(Path::new("/w/J-1.ew")).unwrap(), Path::new("/w/J-1.ew-archive"));
/// assert_eq!(default_root(Path::new("/w/plant")).unwrap(), Path::new("/w/plant.ew-archive"));
/// ```
pub fn default_root(project: &Path) -> Option<PathBuf> {
    let resolved_project;
    let project = if project.file_name().is_none() {
        resolved_project = resolved(project);
        resolved_project.as_path()
    } else {
        project
    };
    let name = project.file_name()?.to_string_lossy();
    let stem = name.strip_suffix(".ew").unwrap_or(&name);
    Some(project.parent()?.join(format!("{stem}{ROOT_SUFFIX}")))
}

/// `path` made absolute (against the current directory when it has no root) and cleaned lexically: `.` dropped, `..`
/// taking back the component before it. No link is resolved and nothing is read.
pub fn resolved(path: &Path) -> PathBuf {
    let absolute = if path.has_root() {
        path.to_path_buf()
    } else {
        std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
    };
    let mut out = PathBuf::new();
    for c in absolute.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                // `..` at the root stays at the root.
                if out.file_name().is_some() {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `path` [`resolved`], with its longest existing ancestor canonicalised through `vfs` (links, `..` through a link,
/// the case the file system stores); with `fold`, folded to lower case on Windows (the form [`lies_inside`] compares).
fn comparable(vfs: &dyn Vfs, path: &Path, fold: bool) -> PathBuf {
    let path = resolved(path);
    let mut base = path.as_path();
    let mut rest = Vec::new();
    let canonical = loop {
        if let Ok(c) = vfs.canonicalize(base) {
            break Some(c);
        }
        match (base.parent(), base.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                base = parent;
            }
            _ => break None,
        }
    };
    let mut out = canonical.unwrap_or_else(|| base.to_path_buf());
    for name in rest.iter().rev() {
        out.push(name);
    }
    if fold && cfg!(windows) {
        PathBuf::from(out.to_string_lossy().to_lowercase())
    } else {
        out
    }
}

/// Whether `path` lies in the directory `dir` (or is it), judged on both made absolute, cleaned and canonicalised where
/// they exist, and case-insensitively on Windows (ADR-0013: an archive root is never inside the project; a relative
/// path, a `..` path or a different case reaches the same directory).
pub fn lies_inside(vfs: &dyn Vfs, path: &Path, dir: &Path) -> bool {
    comparable(vfs, path, true).starts_with(comparable(vfs, dir, true))
}

/// `path` as the release compares and records it: made absolute, cleaned and canonicalised where it exists (links
/// resolved, in the case the file system stores: Windows returns the stored case), with `/` separators. A relative
/// path, a `..` path or, where the file system ignores case, another case reaches the same text. Nothing is folded:
/// two directories whose names differ only in case (in a case-sensitive folder) have two texts.
pub fn path_text(vfs: &dyn Vfs, path: &Path) -> String {
    comparable(vfs, path, false).to_string_lossy().replace('\\', "/")
}

/// The key of `path`: the first 16 hex digits of the SHA-256 of [`path_text`]. The release names its archive temps
/// ([`temp_name`]) and its run records ([`run_record_path`]) by the key of the project directory, which a copy of the
/// directory elsewhere does not share (FT-814).
///
/// ```
/// use std::path::Path;
/// use ew_store::archive::path_key;
/// use ew_store::mem::{DirSync, MemVfs};
///
/// let vfs = MemVfs::new(DirSync::Explicit);
/// let a = path_key(&vfs, Path::new("/w/a/plant.ew"));
/// assert_eq!(a.len(), 16);
/// assert_eq!(a, path_key(&vfs, Path::new("/w/b/../a/plant.ew")));
/// assert_ne!(a, path_key(&vfs, Path::new("/w/b/plant.ew")));
/// ```
pub fn path_key(vfs: &dyn Vfs, path: &Path) -> String {
    key_of(&path_text(vfs, path))
}

/// The first 16 hex digits of the SHA-256 of `text`.
fn key_of(text: &str) -> String {
    let hex = hash_text(&sha256(text.as_bytes()));
    hex.strip_prefix("sha256:").unwrap_or(&hex)[..16].to_owned()
}

/// Why an archive's bytes are not an archive of this layout, or not the release they claim.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ArchiveError {
    /// The container: see [`ZipError`].
    Zip(ZipError),
    /// The entries are not the layout (it says how).
    Layout(String),
    /// `SHA256SUMS` does not verify (it says how).
    Sums(String),
    /// `project/` is not the generation the manifest describes (it says how).
    Project(String),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::Zip(e) => e.fmt(f),
            ArchiveError::Layout(why) => write!(f, "the archive's layout is wrong: {why}"),
            ArchiveError::Sums(why) => write!(f, "SHA256SUMS does not verify: {why}"),
            ArchiveError::Project(why) => {
                write!(f, "the archived project is not the released generation: {why}")
            }
        }
    }
}

impl std::error::Error for ArchiveError {}

impl From<ZipError> for ArchiveError {
    fn from(e: ZipError) -> ArchiveError {
        ArchiveError::Zip(e)
    }
}

/// The text of `SHA256SUMS` over `entries`: one `sha256sum` line per entry, in byte order of the paths.
pub fn sha256sums<'a>(entries: impl IntoIterator<Item = (&'a str, &'a [u8])>) -> String {
    let mut lines: Vec<(&str, String)> = entries
        .into_iter()
        .map(|(path, data)| {
            let hex = hash_text(&sha256(data));
            (path, hex.strip_prefix("sha256:").unwrap_or(&hex).to_owned())
        })
        .collect();
    lines.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = String::new();
    for (path, hex) in lines {
        out.push_str(&hex);
        out.push_str("  ");
        out.push_str(path);
        out.push('\n');
    }
    out
}

/// The bytes of the archive of `entries` (the layout's paths without `mimetype` and `SHA256SUMS`, which this adds):
/// `mimetype` stored first, then every entry and `SHA256SUMS` deflated in byte order of their paths.
///
/// # Errors
///
/// [`ArchiveError::Layout`] when `entries` names `mimetype` or `SHA256SUMS`; [`ArchiveError::Zip`] for a name the
/// container refuses or sizes beyond it.
pub fn build(entries: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, ArchiveError> {
    for reserved in [MIMETYPE_ENTRY, SUMS_ENTRY] {
        if entries.contains_key(reserved) {
            return Err(ArchiveError::Layout(format!(
                "`{reserved}` is written by the archive itself"
            )));
        }
    }
    let sums = sha256sums(
        entries
            .iter()
            .map(|(p, d)| (p.as_str(), d.as_slice()))
            .chain(std::iter::once((MIMETYPE_ENTRY, MIMETYPE.as_bytes()))),
    );
    let mut all: BTreeMap<&str, &[u8]> = entries.iter().map(|(p, d)| (p.as_str(), d.as_slice())).collect();
    all.insert(SUMS_ENTRY, sums.as_bytes());
    let mut list: Vec<(&str, Method, &[u8])> = vec![(MIMETYPE_ENTRY, Method::Stored, MIMETYPE.as_bytes())];
    list.extend(all.into_iter().map(|(p, d)| (p, Method::Deflated, d)));
    Ok(zip::write(&list)?)
}

/// Reads an archive and checks its layout: `mimetype` first, stored, holding [`MIMETYPE`]; every other entry deflated
/// and in strictly increasing byte order of its path; `SHA256SUMS` listing exactly every other entry, in byte order,
/// each with its SHA-256. Returns every entry by path (`mimetype` and `SHA256SUMS` included).
///
/// # Errors
///
/// [`ArchiveError`]: the container, the layout or the sums.
pub fn read(bytes: &[u8], limits: &ZipLimits) -> Result<BTreeMap<String, Vec<u8>>, ArchiveError> {
    let entries = zip::read(bytes, limits)?;
    let Some(first) = entries.first() else {
        return Err(ArchiveError::Layout("no entries".to_owned()));
    };
    if first.name != MIMETYPE_ENTRY || first.method != Method::Stored || first.data != MIMETYPE.as_bytes() {
        return Err(ArchiveError::Layout(format!(
            "the first entry is not `{MIMETYPE_ENTRY}`, stored, holding `{MIMETYPE}`"
        )));
    }
    for pair in entries[1..].windows(2) {
        if pair[0].name.as_bytes() >= pair[1].name.as_bytes() {
            return Err(ArchiveError::Layout(format!(
                "`{}` is not in byte order after `{}`",
                pair[1].name, pair[0].name
            )));
        }
    }
    if let Some(stored) = entries[1..].iter().find(|e| e.method != Method::Deflated) {
        return Err(ArchiveError::Layout(format!("`{}` is not deflated", stored.name)));
    }
    let mut map: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for e in entries {
        map.insert(e.name, e.data);
    }
    let sums = map
        .get(SUMS_ENTRY)
        .ok_or_else(|| ArchiveError::Layout(format!("no `{SUMS_ENTRY}`")))?;
    let text = std::str::from_utf8(sums).map_err(|_| ArchiveError::Sums("not UTF-8".to_owned()))?;
    let expected = sha256sums(
        map.iter()
            .filter(|(p, _)| p.as_str() != SUMS_ENTRY)
            .map(|(p, d)| (p.as_str(), d.as_slice())),
    );
    if text != expected {
        // Name the first line that differs.
        let line = text
            .lines()
            .zip(expected.lines())
            .find(|(a, b)| a != b)
            .map_or_else(
                || "the entries listed are not every other entry".to_owned(),
                |(a, _)| format!("`{a}`"),
            );
        return Err(ArchiveError::Sums(line));
    }
    Ok(map)
}

/// Checks that the archive's `project/` is exactly the generation `manifest` describes (doc 05 §14.1 step 4):
/// `project/manifest.json` is `manifest_bytes`, every file the manifest lists is there with its SHA-256, and no other
/// `project/` entry exists.
///
/// # Errors
///
/// [`ArchiveError::Project`] naming the first difference.
pub fn check_project(
    entries: &BTreeMap<String, Vec<u8>>,
    manifest: &Manifest,
    manifest_bytes: &[u8],
) -> Result<(), ArchiveError> {
    let project: BTreeMap<&str, &Vec<u8>> = entries
        .iter()
        .filter_map(|(p, d)| p.strip_prefix(PROJECT_PREFIX).map(|rel| (rel, d)))
        .collect();
    if project.get(ew_format::MANIFEST).map(|d| d.as_slice()) != Some(manifest_bytes) {
        return Err(ArchiveError::Project(format!(
            "project/{} is not the manifest of generation {}",
            ew_format::MANIFEST,
            manifest.generation
        )));
    }
    for (path, hash) in &manifest.files {
        let Some(data) = project.get(path.as_str()) else {
            return Err(ArchiveError::Project(format!("project/{path} is missing")));
        };
        match hash {
            FileHash::Sha256(h) if *h == sha256(data) => {}
            FileHash::Sha256(_) => {
                return Err(ArchiveError::Project(format!(
                    "project/{path} does not have the manifest's hash"
                )));
            }
            FileHash::Pending => {
                return Err(ArchiveError::Project(format!(
                    "the manifest's hash of {path} is pending (a merged project)"
                )));
            }
        }
    }
    if let Some(extra) = project
        .keys()
        .find(|rel| **rel != ew_format::MANIFEST && !manifest.files.keys().any(|p| p.as_str() == **rel))
    {
        return Err(ArchiveError::Project(format!(
            "project/{extra} is not listed in the manifest"
        )));
    }
    Ok(())
}

/// Where the atomic write failed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WriteStep {
    /// Creating the archive root or removing a leftover temp.
    Root,
    /// Writing the temp.
    Temp,
    /// Syncing the temp and its directory.
    Sync,
    /// Reading the temp back.
    ReadBack,
    /// Renaming the temp into place and syncing the directory.
    Rename,
}

/// Why the archive was not written.
#[derive(Debug)]
pub enum WriteError {
    /// An archive of that name exists: it is never replaced (an incomplete release's archive is discarded or completed
    /// first).
    Exists {
        /// The archive.
        path: PathBuf,
    },
    /// An I/O failure; the temp is removed.
    Io {
        /// The step.
        step: WriteStep,
        /// The path.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
    /// The temp read back differs from what was written, or does not verify; it is removed.
    Verify(ArchiveError),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Exists { path } => write!(
                f,
                "the archive {} exists already; an archive is never replaced (discard or complete the incomplete \
                 release first)",
                path.display()
            ),
            WriteError::Io { step, path, error } => {
                write!(
                    f,
                    "writing the archive failed at {step:?} ({}): {error}",
                    path.display()
                )
            }
            WriteError::Verify(e) => write!(f, "the archive written does not verify: {e}"),
        }
    }
}

impl std::error::Error for WriteError {}

/// The archive in place: its path and the SHA-256 of its bytes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Written {
    /// The archive.
    pub path: PathBuf,
    /// The SHA-256 of its bytes (`ReleaseRecord.archive.sha256`).
    pub sha256: [u8; 32],
    /// The run's record, when one was asked for and written ([`write_atomically`]'s `record`).
    pub run_record: Option<PathBuf>,
}

/// The run record [`write_atomically`] writes before the archive has its name (FT-814): the state directory and the
/// record, whose `archive`, `sha256` and `modified` the write fills in (the archive's [`path_text`], the SHA-256 of
/// its bytes and the temp's modification time, which the archive keeps).
#[derive(Clone, Debug)]
pub struct RecordAt<'a> {
    /// The per-user state directory.
    pub state: &'a Path,
    /// The record.
    pub record: RunRecord,
}

/// Whether `name` is a temp of the archive file `file` written by `owner` (sanitised): `<file>.tmp-<owner>-<digits>`.
fn is_temp_of(name: &str, file: &str, owner: &str) -> bool {
    name.strip_prefix(file)
        .and_then(|rest| rest.strip_prefix(".tmp-"))
        .and_then(|rest| rest.strip_prefix(owner))
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()))
}

/// The leftover temps of the archive of `label` that `owner` wrote in `root` (a crashed release of the same project's),
/// by path. A temp of another owner is never listed.
///
/// # Errors
///
/// An I/O error listing `root` other than its absence.
pub fn leftover_temps(vfs: &dyn Vfs, root: &Path, label: &str, owner: &str) -> io::Result<Vec<PathBuf>> {
    let file = file_name(label);
    let owner = sanitise(owner);
    match vfs.list(root) {
        Ok(entries) => Ok(entries
            .into_iter()
            .filter(|(name, kind)| *kind == EntryKind::File && is_temp_of(name, &file, &owner))
            .map(|(name, _)| root.join(name))
            .collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Writes the archive `bytes` of the release `label` by `owner` at generation `generation` into `root` atomically (see
/// the module documentation); `verify` checks the bytes read back (the sums and the project against manifest g).
/// With `record`, the run's record is written once the temp is verified and before the archive has its name
/// ([`Written::run_record`]; best effort: a record that cannot be written is left out).
///
/// # Errors
///
/// [`WriteError`]; on every error the temp, the archive and the record this call created are removed (best effort)
/// and nothing else is left. An existing archive (another copy's) is never replaced or removed.
#[allow(clippy::too_many_arguments)] // The seam, the name's parts, the bytes, their check and the record: one each.
pub fn write_atomically(
    vfs: &dyn Vfs,
    root: &Path,
    label: &str,
    owner: &str,
    generation: u64,
    bytes: &[u8],
    verify: &dyn Fn(&[u8]) -> Result<(), ArchiveError>,
    record: Option<RecordAt<'_>>,
) -> Result<Written, WriteError> {
    let io_err = |step: WriteStep, path: &Path| {
        let path = path.to_path_buf();
        move |error: io::Error| WriteError::Io { step, path, error }
    };
    let target = root.join(file_name(label));
    let temp = root.join(temp_name(label, owner, generation));
    // The root, durable before anything is named in it.
    let created = match vfs.stat(root) {
        Ok(_) => false,
        Err(e) if e.kind() == io::ErrorKind::NotFound => true,
        Err(e) => return Err(io_err(WriteStep::Root, root)(e)),
    };
    vfs.create_dir_all(root).map_err(io_err(WriteStep::Root, root))?;
    if created {
        if let Some(parent) = root.parent() {
            vfs.sync_dir(parent).map_err(io_err(WriteStep::Root, parent))?;
        }
    }
    for leftover in leftover_temps(vfs, root, label, owner).map_err(io_err(WriteStep::Root, root))? {
        vfs.remove_file(&leftover)
            .map_err(io_err(WriteStep::Root, &leftover))?;
    }
    match vfs.stat(&target) {
        Ok(_) => return Err(WriteError::Exists { path: target }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_err(WriteStep::Root, &target)(e)),
    }
    let cleanup = |e: WriteError| {
        let _ = vfs.remove_file(&temp);
        let _ = vfs.sync_dir(root);
        e
    };
    vfs.create_new(&temp, bytes)
        .map_err(io_err(WriteStep::Temp, &temp))
        .map_err(cleanup)?;
    vfs.sync(&temp)
        .and_then(|()| vfs.sync_dir(root))
        .map_err(io_err(WriteStep::Sync, &temp))
        .map_err(cleanup)?;
    let back = vfs
        .read(&temp)
        .map_err(io_err(WriteStep::ReadBack, &temp))
        .map_err(cleanup)?;
    if back != bytes {
        return Err(cleanup(WriteError::Verify(ArchiveError::Layout(
            "the bytes read back differ from the bytes written".to_owned(),
        ))));
    }
    verify(&back).map_err(WriteError::Verify).map_err(cleanup)?;
    // The temp's modification time: the archive keeps it (a hard link is the same file; the fallback sets it).
    let modified = vfs
        .stat(&temp)
        .map_err(io_err(WriteStep::Sync, &temp))
        .map_err(cleanup)?
        .modified;
    // The run's record (FT-814), durable before the archive has its name, so that a crash after the rename leaves an
    // archive this directory can prove its own. Best effort: without it a discard after a crash keeps the archive.
    let run_record = record.and_then(|at| {
        let record = RunRecord {
            archive: path_text(vfs, &target),
            sha256: hash_text(&sha256(bytes)),
            modified: modified.and_then(record_time),
            ..at.record
        };
        write_run_record(vfs, at.state, &record).ok()
    });
    let fail = |e: WriteError| {
        if let Some(path) = &run_record {
            let _ = remove_run_record(vfs, path);
        }
        cleanup(e)
    };
    // Into place without replacing anything: a hard link (the same file), else, on a file system without hard links,
    // the target created anew (`create_new`: an existing file, another copy's archive, is never replaced; a torn one
    // reads as damaged and is kept). Only a target this call created is ever removed.
    let mut placed = false;
    let renamed = match vfs.rename_noreplace(&temp, &target) {
        Ok(()) => {
            placed = true;
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::Unsupported => match vfs.create_new(&target, bytes) {
            Ok(()) => {
                placed = true;
                modified
                    .map_or(Ok(()), |t| vfs.set_modified(&target, t))
                    .and_then(|()| vfs.sync(&target))
                    .and_then(|()| vfs.remove_file(&temp))
            }
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    };
    match renamed {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && !placed => {
            return Err(fail(WriteError::Exists { path: target }));
        }
        Err(e) => {
            if placed {
                let _ = vfs.remove_file(&target);
            }
            return Err(fail(io_err(WriteStep::Rename, &target)(e)));
        }
    }
    if let Err(e) = vfs.sync_dir(root) {
        // The archive is in place but its name may not be durable: take it back (this call created it), so that the
        // caller's failure leaves nothing behind.
        let _ = vfs.remove_file(&target);
        return Err(fail(io_err(WriteStep::Rename, root)(e)));
    }
    Ok(Written {
        path: target,
        sha256: sha256(bytes),
        run_record,
    })
}

/// Removes an archive this run created (a failed release, doc 05 §14.1 "On failure") or one the user discards, and
/// syncs its directory.
///
/// # Errors
///
/// The I/O error of the removal (an absent file is not an error).
pub fn remove(vfs: &dyn Vfs, path: &Path) -> io::Result<()> {
    match vfs.remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    match path.parent() {
        Some(dir) => vfs.sync_dir(dir),
        None => Ok(()),
    }
}

/// The folder of the release runs' records in the per-user state directory: `<state>/releases`.
pub const RUNS_DIR: &str = "releases";

/// What a release run records once its archive is in place (see the module documentation, FT-814): the positive
/// evidence that this project directory's own run wrote the archive, which a discard requires.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunRecord {
    /// The project directory ([`path_text`]).
    pub project_dir: String,
    /// The archive ([`path_text`]).
    pub archive: String,
    /// The project's ID.
    pub project: String,
    /// The revision's ID.
    pub revision: String,
    /// The revision's label.
    pub label: String,
    /// The released generation g.
    pub generation: u64,
    /// The SHA-256 of the archive's bytes, as `sha256:<hex>`.
    pub sha256: String,
    /// The archive's modification time when the run had written it (seconds and nanoseconds since the Unix epoch),
    /// where the file system reports one: another file of the same bytes, written later under the same name, is not
    /// the one this run wrote.
    pub modified: Option<(u64, u32)>,
    /// The run's mark: a tag the run creates carries it in its message (`Run: <mark>`), so a discard deletes only a
    /// tag this directory's run created.
    pub run: String,
}

/// A modification time as a [`RunRecord`] keeps it; `None` before the Unix epoch.
pub fn record_time(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| (d.as_secs(), d.subsec_nanos()))
}

/// The record file of the run of the project directory `project_dir` that writes `archive` (both as [`path_text`]):
/// `<state>/releases/<key of the directory>-<key of the archive>.json`.
pub fn run_record_path(state: &Path, project_dir: &str, archive: &str) -> PathBuf {
    state
        .join(RUNS_DIR)
        .join(format!("{}-{}.json", key_of(project_dir), key_of(archive)))
}

impl RunRecord {
    /// The record's file in the state directory `state` ([`run_record_path`]).
    pub fn path(&self, state: &Path) -> PathBuf {
        run_record_path(state, &self.project_dir, &self.archive)
    }

    fn to_text(&self) -> String {
        let value = serde_json::json!({
            "v": 1,
            "project_dir": self.project_dir,
            "archive": self.archive,
            "project": self.project,
            "revision": self.revision,
            "label": self.label,
            "generation": self.generation,
            "sha256": self.sha256,
            "modified": self.modified.map(|(s, n)| serde_json::json!([s, n])),
            "run": self.run,
        });
        let mut text = value.to_string();
        text.push('\n');
        text
    }

    fn parse(bytes: &[u8]) -> Option<RunRecord> {
        let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        if v.get("v")?.as_u64()? != 1 {
            return None;
        }
        let text = |name: &str| v.get(name)?.as_str().map(str::to_owned);
        let modified = match v.get("modified")? {
            serde_json::Value::Null => None,
            m => {
                let pair = m.as_array().filter(|a| a.len() == 2)?;
                Some((pair[0].as_u64()?, u32::try_from(pair[1].as_u64()?).ok()?))
            }
        };
        Some(RunRecord {
            project_dir: text("project_dir")?,
            archive: text("archive")?,
            project: text("project")?,
            revision: text("revision")?,
            label: text("label")?,
            generation: v.get("generation")?.as_u64()?,
            sha256: text("sha256")?,
            modified,
            run: text("run")?,
        })
    }
}

/// Writes `record` under the state directory `state`, durably (the file and its directory synced; the folder created
/// and its parent synced when it was absent, and the state directory's parent too when that was absent), replacing an earlier record of the same directory and archive (one a
/// settled run could not remove). Returns the record's file.
///
/// # Errors
///
/// The I/O error of a step; the file is then removed (best effort), so a half-written record never stays.
pub fn write_run_record(vfs: &dyn Vfs, state: &Path, record: &RunRecord) -> io::Result<PathBuf> {
    let path = record.path(state);
    let dir = state.join(RUNS_DIR);
    let absent = |p: &Path| match vfs.stat(p) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e),
    };
    let created = absent(&dir)?;
    // The state directory itself may be new too: then its name reaches the disk through its parent's sync.
    let state_created = created && absent(state)?;
    vfs.create_dir_all(&dir)?;
    if state_created {
        if let Some(parent) = state.parent() {
            vfs.sync_dir(parent)?;
        }
    }
    if created {
        vfs.sync_dir(state)?;
    }
    match vfs.remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let written = vfs
        .create_new(&path, record.to_text().as_bytes())
        .and_then(|()| vfs.sync(&path))
        .and_then(|()| vfs.sync_dir(&dir));
    match written {
        Ok(()) => Ok(path),
        Err(e) => {
            let _ = vfs.remove_file(&path);
            Err(e)
        }
    }
}

/// The run records of the project directory `project_dir` ([`path_text`]) under the state directory `state`, by file,
/// in the order of their names: every `<key of the directory>-*.json` in `<state>/releases` that reads as a record of
/// that directory (a file that does not read, or names another directory, is left out). None when the folder is absent.
///
/// # Errors
///
/// An I/O error listing the folder other than its absence.
pub fn run_records_of(
    vfs: &dyn Vfs,
    state: &Path,
    project_dir: &str,
) -> io::Result<Vec<(PathBuf, RunRecord)>> {
    let dir = state.join(RUNS_DIR);
    let prefix = format!("{}-", key_of(project_dir));
    let entries = match vfs.list(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(entries
        .into_iter()
        .filter(|(name, kind)| {
            *kind == EntryKind::File && name.starts_with(&prefix) && name.ends_with(".json")
        })
        .filter_map(|(name, _)| {
            let path = dir.join(name);
            read_run_record(vfs, &path)
                .filter(|r| r.project_dir == project_dir)
                .map(|r| (path, r))
        })
        .collect())
}

/// The record at `path`, when it exists and reads: a missing, unreadable or malformed record is `None` (no evidence).
pub fn read_run_record(vfs: &dyn Vfs, path: &Path) -> Option<RunRecord> {
    vfs.read(path).ok().and_then(|b| RunRecord::parse(&b))
}

/// Removes the record at `path` and syncs its directory (an absent record is not an error).
///
/// # Errors
///
/// The I/O error of the removal.
pub fn remove_run_record(vfs: &dyn Vfs, path: &Path) -> io::Result<()> {
    remove(vfs, path)
}

/// Whether `a` and `b` are the same modification time as a [`RunRecord`] keeps it, `None` (a file system without one)
/// matching only `None`.
pub fn same_time(a: Option<(u64, u32)>, b: Option<SystemTime>) -> bool {
    a == b.and_then(record_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::{DirSync, Fault, MemVfs};
    use ew_format::FilePath;

    fn entries() -> BTreeMap<String, Vec<u8>> {
        let mut m = BTreeMap::new();
        m.insert("ARCHIVE.json".to_owned(), b"{}\n".to_vec());
        m.insert("project/manifest.json".to_owned(), b"{\"m\":1}\n".to_vec());
        m.insert("project/project.jsonl".to_owned(), b"p\n".to_vec());
        m.insert("check/report.json".to_owned(), b"{}\n".to_vec());
        m
    }

    fn manifest() -> Manifest {
        let mut m = Manifest::new(
            "0.1",
            "ElektraWatt 0.0.0",
            "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37".parse().unwrap(),
            3,
        );
        m.files.insert(
            FilePath::new("project.jsonl").unwrap(),
            FileHash::Sha256(sha256(b"p\n")),
        );
        m
    }

    /// The layout: `mimetype` first and stored, then the entries and `SHA256SUMS` in byte order; the sums list every
    /// other entry, `mimetype` included, in byte order.
    #[test]
    fn the_layout_and_the_sums() {
        let bytes = build(&entries()).unwrap();
        let read_back = zip::read(&bytes, &ZipLimits::default()).unwrap();
        let names: Vec<&str> = read_back.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "mimetype",
                "ARCHIVE.json",
                "SHA256SUMS",
                "check/report.json",
                "project/manifest.json",
                "project/project.jsonl"
            ]
        );
        let map = read(&bytes, &ZipLimits::default()).unwrap();
        let sums = String::from_utf8(map[SUMS_ENTRY].clone()).unwrap();
        let listed: Vec<&str> = sums.lines().map(|l| &l[66..]).collect();
        assert_eq!(
            listed,
            [
                "ARCHIVE.json",
                "check/report.json",
                "mimetype",
                "project/manifest.json",
                "project/project.jsonl"
            ]
        );
        assert!(sums.lines().all(|l| l.len() > 66 && &l[64..66] == "  "));
        assert_eq!(build(&entries()).unwrap(), bytes, "deterministic");
        check_project(&map, &manifest(), b"{\"m\":1}\n").unwrap();
    }

    /// A changed entry fails the sums; a project file that is not the manifest's fails the project check; reserved
    /// names are refused.
    #[test]
    fn damage_and_mismatch_are_refused() {
        let mut e = entries();
        e.insert(MIMETYPE_ENTRY.to_owned(), Vec::new());
        assert!(matches!(build(&e), Err(ArchiveError::Layout(_))));
        // A wrong sums entry: rebuild the ZIP by hand with one sum changed.
        let good = read(&build(&entries()).unwrap(), &ZipLimits::default()).unwrap();
        let mut sums = String::from_utf8(good[SUMS_ENTRY].clone()).unwrap();
        sums.replace_range(0..1, if sums.starts_with('0') { "1" } else { "0" });
        let list: Vec<(&str, Method, &[u8])> =
            std::iter::once((MIMETYPE_ENTRY, Method::Stored, MIMETYPE.as_bytes()))
                .chain(
                    good.iter()
                        .filter(|(p, _)| p.as_str() != MIMETYPE_ENTRY)
                        .map(|(p, d)| {
                            let d: &[u8] = if p == SUMS_ENTRY { sums.as_bytes() } else { d };
                            (p.as_str(), Method::Deflated, d)
                        }),
                )
                .collect();
        let bad = zip::write(&list).unwrap();
        assert!(matches!(
            read(&bad, &ZipLimits::default()),
            Err(ArchiveError::Sums(_))
        ));
        // The project check.
        let mut other = manifest();
        other.files.insert(
            FilePath::new("pages/x.jsonl").unwrap(),
            FileHash::Sha256(sha256(b"x\n")),
        );
        assert!(matches!(
            check_project(&good, &other, b"{\"m\":1}\n"),
            Err(ArchiveError::Project(_))
        ));
        assert!(matches!(
            check_project(&good, &manifest(), b"{\"m\":2}\n"),
            Err(ArchiveError::Project(_))
        ));
    }

    fn ok(_: &[u8]) -> Result<(), ArchiveError> {
        Ok(())
    }

    /// FIX39a (finding 2): a `..` path, a `.` path and (on Windows) a case difference reach the project directory, and
    /// are inside it; a sibling with the project's name as a prefix is not.
    #[test]
    fn inside_is_judged_on_normalised_paths() {
        let vfs = MemVfs::new(DirSync::Explicit);
        vfs.put_dir(Path::new("/w/p.ew"));
        let dir = Path::new("/w/p.ew");
        for inside in [
            "/w/p.ew",
            "/w/p.ew/arch",
            "/w/p.ew/../p.ew/arch",
            "/w/x/../p.ew/./a/b",
            "/w/./p.ew/",
        ] {
            assert!(lies_inside(&vfs, Path::new(inside), dir), "{inside}");
        }
        for outside in ["/w/p.ew-archive", "/w/p.ew/../q", "/w", "/p.ew"] {
            assert!(!lies_inside(&vfs, Path::new(outside), dir), "{outside}");
        }
        assert!(lies_inside(&vfs, Path::new("/w/arch"), Path::new("/w/p.ew/..")));
        if cfg!(windows) {
            assert!(lies_inside(&vfs, Path::new("/W/P.EW/arch"), dir));
        }
    }

    /// FIX39a (finding 2): a relative path is made absolute against the current directory before the comparison, and a
    /// project given as `.` has a default archive root beside it.
    #[test]
    fn relative_paths_are_made_absolute() {
        let cwd = std::env::current_dir().unwrap();
        let vfs = crate::vfs::OsVfs;
        assert!(lies_inside(&vfs, Path::new("arch"), &cwd));
        assert!(lies_inside(&vfs, Path::new("./src/../arch"), Path::new(".")));
        assert!(lies_inside(&vfs, &cwd.join("arch"), Path::new(".")));
        assert!(!lies_inside(&vfs, Path::new("../elsewhere"), &cwd));
        let name = cwd.file_name().unwrap().to_string_lossy();
        let expected = cwd.parent().unwrap().join(format!(
            "{}.ew-archive",
            name.strip_suffix(".ew").unwrap_or(&name)
        ));
        assert_eq!(default_root(Path::new(".")).unwrap(), expected);
        assert_eq!(default_root(&Path::new("src").join("..")).unwrap(), expected);
        assert_eq!(resolved(Path::new("a/./b/../c")), cwd.join("a").join("c"));
    }

    /// The atomic write: the archive lands under its name, durable; a leftover temp of the name and the owner is
    /// removed, and another owner's temp (another project's release, maybe still writing) and any other file are kept;
    /// an existing archive is never replaced.
    #[test]
    fn the_atomic_write() {
        let vfs = MemVfs::new(DirSync::Explicit);
        vfs.put_dir(Path::new("/w"));
        vfs.put(Path::new("/w/p.ew-archive/V1.0.ewarchive.tmp-a1-7"), b"junk");
        vfs.put(Path::new("/w/p.ew-archive/V1.0.ewarchive.tmp-a1-3"), b"junk");
        vfs.put(
            Path::new("/w/p.ew-archive/V1.0.ewarchive.tmp-b2-3"),
            b"another run",
        );
        vfs.put(Path::new("/w/p.ew-archive/V1.0.ewarchive.tmp-7"), b"another run");
        vfs.put(
            Path::new("/w/p.ew-archive/V1.0.ewarchive.tmp-a1-x"),
            b"not a temp",
        );
        let bytes = build(&entries()).unwrap();
        let root = Path::new("/w/p.ew-archive");
        let w = write_atomically(&vfs, root, "V1.0", "a1", 3, &bytes, &ok, None).unwrap();
        assert_eq!(w.path, root.join("V1.0.ewarchive"));
        assert_eq!(w.sha256, sha256(&bytes));
        let files = vfs.files(root);
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "V1.0.ewarchive",
                "V1.0.ewarchive.tmp-7",
                "V1.0.ewarchive.tmp-a1-x",
                "V1.0.ewarchive.tmp-b2-3"
            ]
        );
        assert_eq!(files["V1.0.ewarchive.tmp-b2-3"], b"another run");
        // Durable: a power loss keeps it.
        vfs.set_fault(Fault::PowerLoss { at: 1, keep: 0 });
        assert!(vfs.stat(root).is_err());
        vfs.restart();
        assert_eq!(vfs.files(root)["V1.0.ewarchive"], bytes);
        // Never replaced.
        assert!(matches!(
            write_atomically(&vfs, root, "V1.0", "a1", 4, &bytes, &ok, None),
            Err(WriteError::Exists { .. })
        ));
        assert!(leftover_temps(&vfs, root, "V1.0", "a1").unwrap().is_empty());
        assert_eq!(
            leftover_temps(&vfs, root, "V1.0", "b2").unwrap(),
            [root.join("V1.0.ewarchive.tmp-b2-3")]
        );
    }

    fn record(run: &str) -> RunRecord {
        RunRecord {
            project_dir: "/w/a/p.ew".to_owned(),
            archive: "/w/p.ew-archive/V1.0.ewarchive".to_owned(),
            project: "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37".to_owned(),
            revision: "0b3c2a91-5d3e-7b48-9a61-0e2f4c8d1b38".to_owned(),
            label: "V1.0".to_owned(),
            generation: 3,
            sha256: hash_text(&sha256(b"the archive")),
            modified: Some((1_792_241_530, 1_700)),
            run: run.to_owned(),
        }
    }

    /// FT-814: the key of a path follows the directory, not its spelling (a `..` path reaches the same key); another
    /// directory of the same name has another key, and so has one whose name differs only in case on a case-sensitive
    /// file system (the memory one): nothing is folded (FIX40a, finding 5).
    #[test]
    fn the_key_follows_the_directory() {
        let vfs = MemVfs::new(DirSync::Explicit);
        vfs.put_dir(Path::new("/w/a/p.ew"));
        vfs.put_dir(Path::new("/W/A/P.EW"));
        let key = path_key(&vfs, Path::new("/w/a/p.ew"));
        assert_eq!(key.len(), 16);
        assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(key, path_key(&vfs, Path::new("/w/b/../a/./p.ew")));
        assert_ne!(key, path_key(&vfs, Path::new("/w/b/p.ew")));
        assert_ne!(key, path_key(&vfs, Path::new("/W/A/P.EW")));
        assert_eq!(
            path_text(&vfs, Path::new("/w/b/../a/p.ew")),
            path_text(&vfs, Path::new("/w/a/p.ew"))
        );
        assert_eq!(path_text(&vfs, Path::new("/W/A/P.EW")), "/W/A/P.EW");
    }

    /// FIX40a (finding 5): on the real file system the key is the directory's canonical path in the case the file
    /// system stores. Where names ignore case (Windows), another spelling of one directory reaches its key; where they
    /// do not, two directories whose names differ only in case have two keys.
    #[test]
    fn the_key_keeps_the_stored_case() {
        let base = ew_testdir::fresh_dir("ew-archive-key-case");
        let vfs = crate::vfs::OsVfs;
        let dir = base.join("Plant.ew");
        std::fs::create_dir_all(&dir).unwrap();
        let key = path_key(&vfs, &dir);
        let text = path_text(&vfs, &dir);
        assert!(text.ends_with("/Plant.ew"), "the stored case: {text}");
        assert_ne!(key, path_key(&vfs, &base.join("Other.ew")));
        let lower = base.join("plant.ew");
        if lower.exists() {
            // Names ignore case: the other spelling is the same directory.
            assert_eq!(key, path_key(&vfs, &lower));
            assert_eq!(text, path_text(&vfs, &lower));
        } else {
            std::fs::create_dir_all(&lower).unwrap();
            assert_ne!(key, path_key(&vfs, &lower), "two directories");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// FT-814: a run record is written durably under `<state>/releases/<key>-<key>.json`, read back equal, replaced by
    /// a later record of the same directory and archive, and removed; a missing or malformed record reads as `None`.
    #[test]
    fn the_run_record() {
        let vfs = MemVfs::new(DirSync::Explicit);
        vfs.put_dir(Path::new("/state"));
        let state = Path::new("/state");
        let first = record("run-1");
        let path = write_run_record(&vfs, state, &first).unwrap();
        assert_eq!(path, first.path(state));
        assert_eq!(path.parent(), Some(state.join(RUNS_DIR).as_path()));
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(
            name,
            format!("{}-{}.json", key_of(&first.project_dir), key_of(&first.archive))
        );
        assert_eq!(read_run_record(&vfs, &path), Some(first.clone()));
        // Durable: a power loss right after the write keeps it.
        vfs.set_fault(Fault::PowerLoss { at: 1, keep: 0 });
        assert!(vfs.stat(&path).is_err());
        vfs.restart();
        assert_eq!(read_run_record(&vfs, &path), Some(first.clone()));
        // Replaced by a later run of the same directory and archive; another archive has another file.
        let second = RunRecord {
            modified: None,
            ..record("run-2")
        };
        assert_eq!(write_run_record(&vfs, state, &second).unwrap(), path);
        assert_eq!(read_run_record(&vfs, &path), Some(second));
        let other = RunRecord {
            archive: "/w/p.ew-archive/V2.0.ewarchive".to_owned(),
            ..record("run-3")
        };
        assert_ne!(write_run_record(&vfs, state, &other).unwrap(), path);
        // Malformed or of another version: no evidence.
        vfs.put(&path, b"{\"v\":1,\"project_dir\":\"/w/a/p.ew\"}\n");
        assert_eq!(read_run_record(&vfs, &path), None);
        vfs.put(&path, first.to_text().replace("\"v\":1", "\"v\":2").as_bytes());
        assert_eq!(read_run_record(&vfs, &path), None);
        remove_run_record(&vfs, &path).unwrap();
        assert_eq!(read_run_record(&vfs, &path), None);
        remove_run_record(&vfs, &path).unwrap();
        // The modification time as kept.
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::new(1_792_241_530, 1_700);
        assert_eq!(record_time(t), Some((1_792_241_530, 1_700)));
        assert!(same_time(Some((1_792_241_530, 1_700)), Some(t)));
        assert!(!same_time(Some((1_792_241_530, 1_800)), Some(t)));
        assert!(same_time(None, None));
        assert!(!same_time(None, Some(t)));
    }

    /// FT-814: a failing call of the record's write leaves no record behind.
    #[test]
    fn a_failing_record_write_leaves_nothing() {
        let state = Path::new("/state");
        let mut n = 1;
        loop {
            let vfs = MemVfs::new(DirSync::Explicit);
            vfs.put_dir(state);
            vfs.reset_calls();
            vfs.set_fault(Fault::Error {
                at: n,
                kind: io::ErrorKind::Other,
            });
            let result = write_run_record(&vfs, state, &record("run-1"));
            let calls = vfs.calls();
            vfs.set_fault(Fault::None);
            match result {
                Ok(_) => {
                    assert!(n > calls, "call {n} failed silently");
                    break;
                }
                Err(_) => assert!(vfs.files(&state.join(RUNS_DIR)).is_empty(), "call {n}"),
            }
            n += 1;
        }
    }

    /// What [`write_atomically`] records for the run of `/w/a/p.ew` in `/state`.
    fn record_at() -> RecordAt<'static> {
        RecordAt {
            state: Path::new("/state"),
            record: record("run-1"),
        }
    }

    /// Every failing call of the write leaves no temp, no archive and no run record behind (the error is reported), on
    /// a file system with hard links and on one without (the fallback). A failing call the write may pass over (the
    /// record's, which is best effort) leaves the archive complete.
    #[test]
    fn a_failing_call_leaves_nothing() {
        let bytes = build(&entries()).unwrap();
        let root = Path::new("/w/p.ew-archive");
        let runs = Path::new("/state").join(RUNS_DIR);
        for without_links in [false, true] {
            let mut n = 1;
            loop {
                let vfs = MemVfs::new(DirSync::Explicit);
                vfs.put_dir(Path::new("/w"));
                vfs.put_dir(Path::new("/state"));
                vfs.set_noreplace_unsupported(without_links);
                vfs.reset_calls();
                vfs.set_fault(Fault::Error {
                    at: n,
                    kind: io::ErrorKind::Other,
                });
                let result = write_atomically(&vfs, root, "V1.0", "a1", 3, &bytes, &ok, Some(record_at()));
                let calls = vfs.calls();
                vfs.set_fault(Fault::None);
                match result {
                    Ok(w) if n > calls => {
                        assert_eq!(
                            read_run_record(&vfs, w.run_record.as_deref().unwrap())
                                .unwrap()
                                .run,
                            "run-1"
                        );
                        break;
                    }
                    Ok(w) => {
                        assert_eq!(
                            vfs.files(root).keys().collect::<Vec<_>>(),
                            ["V1.0.ewarchive"],
                            "call {n}"
                        );
                        assert_eq!(vfs.read(&w.path).unwrap(), bytes, "call {n}");
                    }
                    Err(_) => {
                        assert!(
                            vfs.files(root).is_empty(),
                            "call {n} ({without_links}): {:?}",
                            vfs.files(root).keys()
                        );
                        assert!(
                            vfs.files(&runs).is_empty(),
                            "call {n} ({without_links}): the record"
                        );
                    }
                }
                n += 1;
            }
        }
        // A verification failure removes the temp too.
        let vfs = MemVfs::new(DirSync::Explicit);
        let refuse = |_: &[u8]| -> Result<(), ArchiveError> { Err(ArchiveError::Project("no".to_owned())) };
        assert!(matches!(
            write_atomically(&vfs, root, "V1.0", "a1", 3, &bytes, &refuse, None),
            Err(WriteError::Verify(_))
        ));
        assert!(vfs.files(root).is_empty());
    }

    /// FIX40a (finding 3): the run record is durable before the archive has its name. A kill or a power loss at every
    /// call of the write, with hard links and without: whenever the archive is there after the restart, its record is
    /// there too and matches it (the archive, its SHA-256 and its modification time), so the directory's own leftover
    /// is discardable.
    #[test]
    fn an_archive_in_place_always_has_its_record() {
        let bytes = build(&entries()).unwrap();
        let root = Path::new("/w/p.ew-archive");
        let target = root.join("V1.0.ewarchive");
        for without_links in [false, true] {
            for power in [false, true] {
                let mut placed = 0;
                for n in 1.. {
                    let vfs = MemVfs::new(DirSync::Explicit);
                    vfs.put_dir(Path::new("/w"));
                    vfs.put_dir(Path::new("/state"));
                    vfs.set_noreplace_unsupported(without_links);
                    vfs.reset_calls();
                    vfs.set_fault(if power {
                        Fault::PowerLoss { at: n, keep: 0 }
                    } else {
                        Fault::Kill { at: n }
                    });
                    let result =
                        write_atomically(&vfs, root, "V1.0", "a1", 3, &bytes, &ok, Some(record_at()));
                    let calls = vfs.calls();
                    vfs.restart();
                    if result.is_ok() && n > calls {
                        break;
                    }
                    let Ok(stat) = vfs.stat(&target) else { continue };
                    placed += 1;
                    let path = run_record_path(Path::new("/state"), "/w/a/p.ew", &path_text(&vfs, &target));
                    let kept = read_run_record(&vfs, &path)
                        .unwrap_or_else(|| panic!("call {n} ({without_links}, {power}): no record"));
                    assert_eq!(kept.archive, path_text(&vfs, &target));
                    assert_eq!(
                        kept.sha256,
                        hash_text(&sha256(&vfs.read(&target).unwrap())),
                        "call {n}"
                    );
                    assert!(same_time(kept.modified, stat.modified), "call {n}");
                }
                assert!(
                    placed > 0 || power,
                    "({without_links}, {power}): a kill after the rename was tried"
                );
            }
        }
    }

    /// A memory file system on which another copy's release creates the archive right before this write places its own
    /// (`race`: when the write looks at the target, or creates or renames over it, once the temp is there), and whose
    /// clock moves on to `later` once the temp is written.
    #[derive(Debug)]
    struct Racing {
        inner: MemVfs,
        target: PathBuf,
        race: Option<Vec<u8>>,
        later: SystemTime,
    }

    impl Racing {
        fn temp_written(&self) -> bool {
            self.inner
                .files(self.target.parent().unwrap())
                .keys()
                .any(|name| name.contains(".tmp-"))
        }

        fn race(&self, path: &Path) {
            if path == self.target && self.temp_written() && self.inner.stat(path).is_err() {
                if let Some(other) = &self.race {
                    self.inner.put(path, other);
                }
            }
        }
    }

    impl Vfs for Racing {
        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            self.inner.read(path)
        }
        fn stat(&self, path: &Path) -> io::Result<crate::vfs::Stat> {
            let stat = self.inner.stat(path);
            self.race(path);
            stat
        }
        fn list(&self, dir: &Path) -> io::Result<Vec<(String, EntryKind)>> {
            self.inner.list(dir)
        }
        fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
            self.inner.create_dir_all(dir)
        }
        fn create_new(&self, path: &Path, data: &[u8]) -> io::Result<()> {
            self.race(path);
            let created = self.inner.create_new(path, data);
            if path.to_string_lossy().contains(".tmp-") {
                self.inner.set_now(self.later);
            }
            created
        }
        fn append(&self, path: &Path, data: &[u8]) -> io::Result<()> {
            self.inner.append(path, data)
        }
        fn sync(&self, path: &Path) -> io::Result<()> {
            self.inner.sync(path)
        }
        fn sync_dir(&self, dir: &Path) -> io::Result<()> {
            self.inner.sync_dir(dir)
        }
        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            self.race(to);
            self.inner.rename(from, to)
        }
        fn rename_noreplace(&self, from: &Path, to: &Path) -> io::Result<()> {
            self.inner.rename_noreplace(from, to)
        }
        fn remove_file(&self, path: &Path) -> io::Result<()> {
            self.inner.remove_file(path)
        }
        fn remove_dir(&self, dir: &Path) -> io::Result<()> {
            self.inner.remove_dir(dir)
        }
        fn set_modified(&self, path: &Path, time: SystemTime) -> io::Result<()> {
            self.inner.set_modified(path, time)
        }
        fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
            self.inner.canonicalize(path)
        }
    }

    fn racing(race: Option<&[u8]>) -> Racing {
        let inner = MemVfs::new(DirSync::Explicit);
        inner.put_dir(Path::new("/w"));
        inner.put_dir(Path::new("/state"));
        inner.set_noreplace_unsupported(true);
        Racing {
            inner,
            target: PathBuf::from("/w/p.ew-archive/V1.0.ewarchive"),
            race: race.map(<[u8]>::to_vec),
            later: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_900_000_000),
        }
    }

    /// FIX40a (finding 4): on a file system without hard links the archive is created anew (never renamed over an
    /// existing file) with the temp's modification time, which the record names; another copy's archive that appears
    /// between the write's checks and its placement is never replaced nor removed (the write reports it as existing and
    /// takes back its temp and its record).
    #[test]
    fn the_fallback_never_replaces_another_archive() {
        let bytes = build(&entries()).unwrap();
        let root = Path::new("/w/p.ew-archive");
        let vfs = racing(None);
        let w = write_atomically(&vfs, root, "V1.0", "a1", 3, &bytes, &ok, Some(record_at())).unwrap();
        assert_eq!(
            vfs.inner.files(root).keys().collect::<Vec<_>>(),
            ["V1.0.ewarchive"]
        );
        assert_eq!(vfs.read(&w.path).unwrap(), bytes);
        let kept = read_run_record(&vfs, w.run_record.as_deref().unwrap()).unwrap();
        let modified = vfs.stat(&w.path).unwrap().modified;
        assert!(same_time(kept.modified, modified), "the temp's time: {kept:?}");
        assert_ne!(
            modified,
            Some(vfs.later),
            "created after the clock moved on, with the temp's time set"
        );

        let other = b"another copy's archive";
        let vfs = racing(Some(other));
        assert!(matches!(
            write_atomically(&vfs, root, "V1.0", "a1", 3, &bytes, &ok, Some(record_at())),
            Err(WriteError::Exists { .. })
        ));
        assert_eq!(
            vfs.inner.files(root).keys().collect::<Vec<_>>(),
            ["V1.0.ewarchive"]
        );
        assert_eq!(
            vfs.read(&root.join("V1.0.ewarchive")).unwrap(),
            other,
            "kept byte for byte"
        );
        assert!(
            vfs.inner.files(&Path::new("/state").join(RUNS_DIR)).is_empty(),
            "the record is taken back"
        );
    }

    /// FIX40a (finding 3): on the real file system the archive keeps the temp's modification time through the
    /// hard-link rename (or the fallback), so the record written before the rename names the archive's time.
    #[test]
    fn the_record_names_the_archives_time_on_disk() {
        let base = ew_testdir::fresh_dir("ew-archive-record-time");
        let vfs = crate::vfs::OsVfs;
        let root = base.join("p.ew-archive");
        let state = base.join("state");
        let project = base.join("p.ew");
        std::fs::create_dir_all(&project).unwrap();
        let bytes = build(&entries()).unwrap();
        let at = RecordAt {
            state: &state,
            record: RunRecord {
                project_dir: path_text(&vfs, &project),
                ..record("run-1")
            },
        };
        let w = write_atomically(&vfs, &root, "V1.0", "k", 3, &bytes, &ok, Some(at)).unwrap();
        let kept = read_run_record(&vfs, w.run_record.as_deref().unwrap()).unwrap();
        assert_eq!(kept.archive, path_text(&vfs, &w.path));
        assert_eq!(kept.sha256, hash_text(&sha256(&bytes)));
        let modified = std::fs::metadata(&w.path).unwrap().modified().ok();
        assert!(
            same_time(kept.modified, modified),
            "{:?} against {modified:?}",
            kept.modified
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// FIX40a (finding 9): a record written into a state directory that did not exist survives a power loss right after
    /// the write (the state directory's name is made durable through its parent).
    #[test]
    fn a_record_in_a_new_state_directory_is_durable() {
        let vfs = MemVfs::new(DirSync::Explicit);
        vfs.put_dir(Path::new("/home"));
        let state = Path::new("/home/state");
        let path = write_run_record(&vfs, state, &record("run-1")).unwrap();
        vfs.set_fault(Fault::PowerLoss { at: 1, keep: 0 });
        assert!(vfs.stat(&path).is_err());
        vfs.restart();
        assert_eq!(read_run_record(&vfs, &path), Some(record("run-1")));
    }

    /// FIX40a (finding 2): the run records of a directory are those named by its key that read as its records.
    #[test]
    fn the_run_records_of_a_directory() {
        let vfs = MemVfs::new(DirSync::Explicit);
        vfs.put_dir(Path::new("/state"));
        let state = Path::new("/state");
        assert!(run_records_of(&vfs, state, "/w/a/p.ew").unwrap().is_empty());
        let mine = write_run_record(&vfs, state, &record("run-1")).unwrap();
        let other = RunRecord {
            archive: "/w/p.ew-archive/V2.0.ewarchive".to_owned(),
            ..record("run-2")
        };
        let mine2 = write_run_record(&vfs, state, &other).unwrap();
        let theirs = RunRecord {
            project_dir: "/w/b/p.ew".to_owned(),
            ..record("run-3")
        };
        write_run_record(&vfs, state, &theirs).unwrap();
        vfs.put(
            &state
                .join(RUNS_DIR)
                .join(format!("{}-junk.json", key_of("/w/a/p.ew"))),
            b"junk",
        );
        let mut expected = vec![(mine, record("run-1")), (mine2, other)];
        expected.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(run_records_of(&vfs, state, "/w/a/p.ew").unwrap(), expected);
    }
}
