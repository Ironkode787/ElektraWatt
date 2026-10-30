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
//! archive name (`<name>.tmp-<digits>`, a crashed release's) is removed; an existing archive of that name is never
//! replaced ([`WriteError::Exists`]: archives are immutable, FR-REV-009; a complete unreferenced one is the next open's
//! "incomplete release"). The archive is written as `<name>.tmp-<g>`, synced with its directory, read back through the
//! seam, compared byte for byte with what was written and checked by the caller's verification (the sums and the
//! project's hashes against manifest g), then renamed into place without replacement and the directory synced. Any
//! failure removes the temp. The archive's name is therefore durable before the caller stamps the release.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

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

/// The temp name of the archive of `label` written for generation `generation`: `<file name>.tmp-<g>`.
pub fn temp_name(label: &str, generation: u64) -> String {
    format!("{}.tmp-{generation}", file_name(label))
}

/// The default archive root of the project directory `project` (an absolute path): `<parent>/<name>.ew-archive`, where
/// `<name>` is the directory's name without a trailing `.ew` (doc 05 §1, §14.2). `None` for a path without a parent
/// or a name.
///
/// ```
/// use std::path::Path;
/// use ew_store::archive::default_root;
///
/// assert_eq!(default_root(Path::new("/w/J-1.ew")).unwrap(), Path::new("/w/J-1.ew-archive"));
/// assert_eq!(default_root(Path::new("/w/plant")).unwrap(), Path::new("/w/plant.ew-archive"));
/// ```
pub fn default_root(project: &Path) -> Option<PathBuf> {
    let name = project.file_name()?.to_string_lossy();
    let stem = name.strip_suffix(".ew").unwrap_or(&name);
    Some(project.parent()?.join(format!("{stem}{ROOT_SUFFIX}")))
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
}

/// Whether `name` is a temp of the archive file `file`: `<file>.tmp-<digits>`.
fn is_temp_of(name: &str, file: &str) -> bool {
    name.strip_prefix(file)
        .and_then(|rest| rest.strip_prefix(".tmp-"))
        .is_some_and(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()))
}

/// The leftover temps of the archive of `label` in `root` (a crashed release's), by path.
///
/// # Errors
///
/// An I/O error listing `root` other than its absence.
pub fn leftover_temps(vfs: &dyn Vfs, root: &Path, label: &str) -> io::Result<Vec<PathBuf>> {
    let file = file_name(label);
    match vfs.list(root) {
        Ok(entries) => Ok(entries
            .into_iter()
            .filter(|(name, kind)| *kind == EntryKind::File && is_temp_of(name, &file))
            .map(|(name, _)| root.join(name))
            .collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Writes the archive `bytes` of the release `label` at generation `generation` into `root` atomically (see the module
/// documentation); `verify` checks the bytes read back (the sums and the project against manifest g).
///
/// # Errors
///
/// [`WriteError`]; on every error the temp this call created is removed (best effort) and nothing else is left.
pub fn write_atomically(
    vfs: &dyn Vfs,
    root: &Path,
    label: &str,
    generation: u64,
    bytes: &[u8],
    verify: &dyn Fn(&[u8]) -> Result<(), ArchiveError>,
) -> Result<Written, WriteError> {
    let io_err = |step: WriteStep, path: &Path| {
        let path = path.to_path_buf();
        move |error: io::Error| WriteError::Io { step, path, error }
    };
    let target = root.join(file_name(label));
    let temp = root.join(temp_name(label, generation));
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
    for leftover in leftover_temps(vfs, root, label).map_err(io_err(WriteStep::Root, root))? {
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
    let renamed = match vfs.rename_noreplace(&temp, &target) {
        Err(e) if e.kind() == io::ErrorKind::Unsupported => match vfs.stat(&target) {
            Ok(_) => Err(io::Error::from(io::ErrorKind::AlreadyExists)),
            Err(s) if s.kind() == io::ErrorKind::NotFound => vfs.rename(&temp, &target),
            Err(s) => Err(s),
        },
        other => other,
    };
    match renamed {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(cleanup(WriteError::Exists { path: target }));
        }
        Err(e) => return Err(cleanup(io_err(WriteStep::Rename, &target)(e))),
    }
    if let Err(e) = vfs.sync_dir(root) {
        // The archive is in place but its name may not be durable: take it back, so that the caller's failure leaves
        // nothing behind.
        let _ = vfs.remove_file(&target);
        let _ = vfs.sync_dir(root);
        return Err(io_err(WriteStep::Rename, root)(e));
    }
    Ok(Written {
        path: target,
        sha256: sha256(bytes),
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

    /// The atomic write: the archive lands under its name, durable; a leftover temp of the name is removed; an
    /// existing archive is never replaced.
    #[test]
    fn the_atomic_write() {
        let vfs = MemVfs::new(DirSync::Explicit);
        vfs.put_dir(Path::new("/w"));
        vfs.put(Path::new("/w/p.ew-archive/V1.0.ewarchive.tmp-7"), b"junk");
        vfs.put(Path::new("/w/p.ew-archive/V1.0.ewarchive.tmp-x"), b"not ours");
        let bytes = build(&entries()).unwrap();
        let root = Path::new("/w/p.ew-archive");
        let w = write_atomically(&vfs, root, "V1.0", 3, &bytes, &ok).unwrap();
        assert_eq!(w.path, root.join("V1.0.ewarchive"));
        assert_eq!(w.sha256, sha256(&bytes));
        let files = vfs.files(root);
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            ["V1.0.ewarchive", "V1.0.ewarchive.tmp-x"]
        );
        // Durable: a power loss keeps it.
        vfs.set_fault(Fault::PowerLoss { at: 1, keep: 0 });
        assert!(vfs.stat(root).is_err());
        vfs.restart();
        assert_eq!(vfs.files(root)["V1.0.ewarchive"], bytes);
        // Never replaced.
        assert!(matches!(
            write_atomically(&vfs, root, "V1.0", 4, &bytes, &ok),
            Err(WriteError::Exists { .. })
        ));
        assert!(leftover_temps(&vfs, root, "V1.0").unwrap().is_empty());
    }

    /// Every failing call of the write leaves no temp and no archive behind (the error is reported).
    #[test]
    fn a_failing_call_leaves_nothing() {
        let bytes = build(&entries()).unwrap();
        let root = Path::new("/w/p.ew-archive");
        let mut n = 1;
        loop {
            let vfs = MemVfs::new(DirSync::Explicit);
            vfs.put_dir(Path::new("/w"));
            vfs.reset_calls();
            vfs.set_fault(Fault::Error {
                at: n,
                kind: io::ErrorKind::Other,
            });
            let result = write_atomically(&vfs, root, "V1.0", 3, &bytes, &ok);
            let calls = vfs.calls();
            vfs.set_fault(Fault::None);
            match result {
                Ok(_) => {
                    assert!(n > calls, "call {n} failed silently");
                    break;
                }
                Err(_) => {
                    assert!(
                        vfs.files(root).is_empty(),
                        "call {n}: {:?}",
                        vfs.files(root).keys()
                    );
                }
            }
            n += 1;
        }
        // A verification failure removes the temp too.
        let vfs = MemVfs::new(DirSync::Explicit);
        let refuse = |_: &[u8]| -> Result<(), ArchiveError> { Err(ArchiveError::Project("no".to_owned())) };
        assert!(matches!(
            write_atomically(&vfs, root, "V1.0", 3, &bytes, &refuse),
            Err(WriteError::Verify(_))
        ));
        assert!(vfs.files(root).is_empty());
    }
}
