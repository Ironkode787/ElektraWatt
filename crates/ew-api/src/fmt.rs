// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew fmt` (FR-EXT-008): the project files in their canonical encoding and line order (doc 05 §3).
//!
//! - **The rewrite set** ([`plan`]) is what the loader found not in canonical form or holding a record of another
//!   file (`ew_store::Store::canonical_rewrite`): the save plan with no changed record, never a full rewrite, so a file
//!   already canonical is not written.
//! - **Kept lines** ([`kept_lines`]): quarantined lines, lines of unknown kinds and untouched malformed values stay in
//!   the files verbatim, as in every save (ADR-0011); the open's findings name them, so they are reported.
//! - **The content check** ([`verify_content`]) reads each file of the set back and compares the content of its lines
//!   with the content of the canonical bytes, over the whole set (a record may move to another file): only form may
//!   change, never content, else nothing is written.
//! - **The write** ([`format()`]) is a save of the session's store: the save protocol's temps, intent and renames, so a
//!   crash between files leaves either the old or the new generation, never a torn file.
//!
//! # `ew fmt --repair`
//!
//! [`repair`] discards exactly the broken lines, EW-CORE-005 at line level (a line
//! that is not one JSON value, or not UTF-8, doc 05 §6.3), from a session opened with
//! [`StoreOptions::repair`](crate::StoreOptions): the integrity demotion let those lines through, and only those. In
//! order, before anything is written: a store whose files cannot be packed as found refuses (no manifest, an
//! incomplete save, a listed file that cannot be read, a file changed on disk since the repair opened it, the entry
//! cap); without a broken line nothing is written; the content check runs over the repair's rewrite set with the
//! broken lines left out (only form may change elsewhere). Then the backup: the generation's files as found (decision
//! log ID-1186), broken lines and edits outside ElektraWatt since the last save included, packed as an `.ewz` whose
//! manifest is the loaded one hashed to those bytes ([`ewz::pack_as_found`]; [`Backup::edited`] counts the files that
//! differ from the generation as saved) and written into the project folder's parent as
//! `<project folder>.before-repair-g<generation>.ewz` ([`backup_path`]), to a temporary name beside it, read back,
//! synced, moved there without replacing anything ([`Vfs::rename_noreplace`]; on a file system without hard links,
//! [`Vfs::rename`] after the name is checked absent again) and the folder synced; a file already at that name is reused
//! when it holds this generation's pack byte
//! for byte and refused otherwise. Last the one save, [`ew_store::Store::discard_broken_lines`]: the files that hold
//! the broken lines rewritten without them, every other line as `ew fmt` writes it (kept lines byte for byte). A
//! failure before the save's commit point leaves the generation on disk as it was, the backup beside it; the caller
//! opens the project again afterwards.
//!
//! # Form and content
//!
//! Form, which `ew fmt` may change: the order of the lines and of an object's members, white space, string escapes,
//! a byte order mark, CR line ends, empty lines, a number's spelling (`1.0` and `1`, `1e2` and `100`) and the Unicode
//! normalisation of a string (NFC, doc 05 §3.1). Content, which it may never change: anything else, such as a member
//! dropped or added (`"x":null` and an absent `x` are different content here, so a file holding one is refused rather
//! than rewritten), a value, or the order of an array's items. A line that is not one JSON value compares by its bytes
//! (CR line end aside).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use ew_store::ew_format::{FilePath, Finding, FindingKind, PathClass, classify};
use ew_store::ewz::{self, PackError};
use ew_store::{EntryKind, GenerationError, Vfs};
use serde_json::Value;

use crate::{Access, OpenReport, ReadOnlyReason, SaveError, Session};

/// A file of the rewrite set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rewrite {
    /// The file, relative to the project directory (`devices/3f.jsonl`).
    pub file: String,
    /// `true` when nothing is left in it and the save removes it (its records belong in other files).
    pub removed: bool,
}

/// A line (or a file) the canonical form keeps verbatim, as the open reported it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeptLine {
    /// The file, relative to the project directory.
    pub file: String,
    /// The 1-based line; `None` for a whole file (a library document, a file above a limit).
    pub line: Option<u32>,
    /// The finding's rule (`EW-CORE-007`).
    pub code: String,
    /// Why it is kept, as the open's finding says it.
    pub reason: String,
}

/// What `ew fmt` writes: the rewrite set with the canonical bytes of each file.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Plan {
    files: BTreeMap<FilePath, Option<Vec<u8>>>,
}

impl Plan {
    /// The files of the set in path order.
    pub fn files(&self) -> Vec<Rewrite> {
        self.files
            .iter()
            .map(|(path, bytes)| Rewrite {
                file: path.as_str().to_owned(),
                removed: bytes.is_none(),
            })
            .collect()
    }

    /// Whether every file is already canonical.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// What [`format()`] wrote.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Formatted {
    /// The generation on disk afterwards (unchanged when nothing was written).
    pub generation: u64,
    /// The files rewritten or removed, in path order.
    pub files: Vec<Rewrite>,
}

/// Why `ew fmt` writes nothing.
#[derive(Debug)]
pub enum FmtError {
    /// The session has no project directory.
    NoProject,
    /// The session has unsaved changes: a format writes only the generation on disk.
    Dirty,
    /// A record has no file or no JSON form.
    Render(SaveError),
    /// A file of the set cannot be read back for the content check.
    Read {
        /// The file, relative to the project directory.
        file: String,
        /// The error.
        error: io::Error,
    },
    /// The canonical form changes content, not only form.
    Content {
        /// The file, relative to the project directory.
        file: String,
        /// The 1-based line: of the file on disk, or of its canonical bytes when `added`.
        line: u32,
        /// `true`: the canonical bytes hold this line and the files on disk do not; `false`: the line on disk is not
        /// in the canonical bytes.
        added: bool,
    },
    /// The save failed.
    Save(SaveError),
}

impl fmt::Display for FmtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FmtError::NoProject => f.write_str("the session has no project directory"),
            FmtError::Dirty => {
                f.write_str("the session has unsaved changes; a format writes only the saved project")
            }
            FmtError::Render(e) => write!(f, "the canonical form cannot be written: {e}"),
            FmtError::Read { file, error } => write!(f, "{file} cannot be read back: {error}"),
            FmtError::Content {
                file,
                line,
                added: false,
            } => write!(
                f,
                "{file} line {line}: the canonical form would change the content of this line, not only its form"
            ),
            FmtError::Content {
                file,
                line,
                added: true,
            } => write!(
                f,
                "{file}: the canonical form would hold content that no file on disk holds (its line {line})"
            ),
            FmtError::Save(e) => write!(f, "the save failed: {e}"),
        }
    }
}

impl std::error::Error for FmtError {}

/// The rewrite set of the session's project with the canonical bytes of each file; nothing is written.
///
/// # Errors
///
/// [`FmtError::NoProject`] without a project directory; [`FmtError::Render`] for a record without a file or a JSON
/// form.
pub fn plan(session: &Session) -> Result<Plan, FmtError> {
    let store = session.store().ok_or(FmtError::NoProject)?;
    let files = store.canonical_rewrite().map_err(FmtError::Render)?;
    Ok(Plan { files })
}

/// The lines and files the open kept verbatim, which a format keeps too: quarantined lines (EW-CORE-005, -006, -007,
/// -009, a limit), malformed values of known variants (EW-CORE-017) and unknown kinds, members or files of a newer
/// format (EW-CORE-016), in the report's order.
pub fn kept_lines(report: &OpenReport) -> Vec<KeptLine> {
    kept_where(report, |_| true)
}

/// [`kept_lines`] that stay after `ew fmt --repair`: every one but the broken lines, which the repair discards.
pub fn kept_after_repair(report: &OpenReport) -> Vec<KeptLine> {
    kept_where(report, |f| !f.is_broken_line())
}

fn kept_where(report: &OpenReport, keep: impl Fn(&Finding) -> bool) -> Vec<KeptLine> {
    report
        .findings
        .iter()
        .filter(|f| keep(f))
        .filter(|f| {
            matches!(
                f.kind,
                FindingKind::JsonSyntax
                    | FindingKind::ConflictMarkers
                    | FindingKind::SchemaViolation
                    | FindingKind::DuplicateRecord
                    | FindingKind::LimitExceeded
                    | FindingKind::MalformedVariant
                    | FindingKind::NewerData
            )
        })
        .filter_map(|f| {
            Some(KeptLine {
                file: f.file.as_ref()?.as_str().to_owned(),
                line: f.line,
                code: f.kind.code().unwrap_or_default().to_owned(),
                reason: f.message.clone(),
            })
        })
        .collect()
}

/// Checks that `plan` changes only form (the module documentation): every line of the files of the set on disk is in
/// their canonical bytes as often as on disk, and the canonical bytes hold nothing else.
///
/// # Errors
///
/// [`FmtError::Content`] naming the first line whose content would change; [`FmtError::Read`] when a file cannot be
/// read back.
pub fn verify_content(session: &Session, plan: &Plan) -> Result<(), FmtError> {
    verify_content_except(session, plan, &BTreeSet::new())
}

/// [`verify_content`] with the lines `discarded` (file and 1-based line on disk) left out of the files on disk: the
/// repair's check, whose canonical bytes no longer hold the broken lines.
fn verify_content_except(
    session: &Session,
    plan: &Plan,
    discarded: &BTreeSet<(String, u32)>,
) -> Result<(), FmtError> {
    let store = session.store().ok_or(FmtError::NoProject)?;
    let mut disk: BTreeMap<Content, Vec<(String, u32)>> = BTreeMap::new();
    let mut canonical: BTreeMap<Content, Vec<(String, u32)>> = BTreeMap::new();
    for (path, bytes) in &plan.files {
        let full = ew_store::join(store.root(), path.as_str());
        let old = match store.vfs().read(&full) {
            Ok(bytes) => bytes,
            // A file the set creates (the target of a moved record).
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(FmtError::Read {
                    file: path.as_str().to_owned(),
                    error,
                });
            }
        };
        for (line, content) in contents(path, &old) {
            if discarded.contains(&(path.as_str().to_owned(), line)) {
                continue;
            }
            disk.entry(content)
                .or_default()
                .push((path.as_str().to_owned(), line));
        }
        for (line, content) in contents(path, bytes.as_deref().unwrap_or_default()) {
            canonical
                .entry(content)
                .or_default()
                .push((path.as_str().to_owned(), line));
        }
    }
    for (content, places) in &disk {
        let kept = canonical.remove(content).unwrap_or_default();
        if kept.len() < places.len() {
            let (file, line) = places[kept.len()].clone();
            return Err(FmtError::Content {
                file,
                line,
                added: false,
            });
        }
        if kept.len() > places.len() {
            let (file, line) = kept[places.len()].clone();
            return Err(FmtError::Content {
                file,
                line,
                added: true,
            });
        }
    }
    if let Some((file, line)) = canonical.into_values().flatten().min() {
        return Err(FmtError::Content {
            file,
            line,
            added: true,
        });
    }
    Ok(())
}

/// Rewrites the rewrite set of the session's project in canonical form through the save protocol, after the content
/// check; a project already canonical writes nothing. The session must be a fresh editing open (the CLI refuses a
/// read-only one and one with a crashed session's journal before it gets here).
///
/// # Errors
///
/// [`FmtError`]: the content check refuses, a file cannot be read back, or the save fails (with
/// [`SaveError::Incomplete`] of a new generation the generation is decided and completes by roll-forward).
pub fn format(session: &mut Session) -> Result<Formatted, FmtError> {
    if session.store().is_none() {
        return Err(FmtError::NoProject);
    }
    if session.is_dirty() {
        return Err(FmtError::Dirty);
    }
    let plan = plan(session)?;
    verify_content(session, &plan)?;
    let files = plan.files();
    if plan.is_empty() {
        let generation = session.store().map_or(0, ew_store::Store::generation);
        return Ok(Formatted { generation, files });
    }
    let saved = session.save().map_err(FmtError::Save)?;
    Ok(Formatted {
        generation: saved.generation,
        files,
    })
}

/// The most characters of a discarded line's text that `ew fmt --repair` shows; a longer text is
/// cut there and ends with "…".
pub const SHOWN_CHARS: usize = 80;

/// The name of the backup `.ewz` between the project folder's name and the generation:
/// `<project folder>.before-repair-g<generation>.ewz` ([`backup_path`]).
pub const BACKUP_INFIX: &str = ".before-repair-g";

/// A broken line that [`repair`] discards (EW-CORE-005 at line level), from the store's own data.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiscardedLine {
    /// The file, relative to the project directory.
    pub file: String,
    /// The 1-based line in the generation before the repair.
    pub line: u32,
    /// Its bytes as the file held them (the backup holds them too).
    pub text: Vec<u8>,
}

impl DiscardedLine {
    /// The text as shown: invalid UTF-8 as U+FFFD, cut after [`SHOWN_CHARS`] characters with "…". Control characters
    /// are the printer's to escape.
    pub fn shown(&self) -> String {
        let text = String::from_utf8_lossy(&self.text);
        let mut chars = text.chars();
        let mut out: String = chars.by_ref().take(SHOWN_CHARS).collect();
        if chars.next().is_some() {
            out.push('\u{2026}');
        }
        out
    }
}

/// The backup [`repair`] wrote or found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Backup {
    /// The `.ewz` ([`backup_path`]).
    pub path: PathBuf,
    /// The generation it holds: the one before the repair.
    pub generation: u64,
    /// Whether it was there already, holding this generation's pack byte for byte, and was reused.
    pub reused: bool,
    /// How many of its files differ from the generation as ElektraWatt saved it: edited outside ElektraWatt since the
    /// last save (their bytes do not have the manifest's hash, or it is pending). The backup holds them as found, its
    /// manifest hashed to them ([`ewz::AsFound::edited`]).
    pub edited: usize,
}

/// What [`repair`] did.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Repaired {
    /// The generation on disk afterwards (the one before when there was nothing to repair).
    pub generation: u64,
    /// The lines discarded, in file and line order; empty when there was nothing to repair.
    pub discarded: Vec<DiscardedLine>,
    /// The files the save rewrote or removed, in path order: those that held the lines, and those the open found not in
    /// canonical form.
    pub files: Vec<Rewrite>,
    /// The backup; `None` when there was nothing to repair.
    pub backup: Option<Backup>,
}

/// Why [`repair`] did not repair.
#[derive(Debug)]
pub enum RepairError {
    /// The session has no project directory.
    NoProject,
    /// The session has unsaved changes: a repair writes only the saved project.
    Dirty,
    /// The session is read-only (a repair opens through [`StoreOptions::repair`](crate::StoreOptions), which the CLI
    /// refuses for every other forcing reason first).
    ReadOnly(ReadOnlyReason),
    /// The generation's files cannot be packed as found for the backup ([`ewz::pack_as_found`]): no manifest, an
    /// incomplete save, a listed file that cannot be read, a file changed on disk since the repair opened it
    /// ([`PackError::ChangedSinceOpen`]: run the repair again), an archive the reader would refuse (the entry cap).
    /// Nothing was written.
    Pack(PackError),
    /// The project folder has no name or no parent folder to hold the backup. Nothing was written.
    NoFolder(PathBuf),
    /// A file is at the backup's name and does not hold this generation's pack. Nothing was written.
    BackupTaken(PathBuf),
    /// The backup could not be written (its folder cannot be written, the disk is full, a read-back differs). Nothing
    /// of it is left and the project was not changed.
    Backup {
        /// The backup's path.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
    /// The content check or the rewrite set refused (a [`FmtError`]). Nothing was written.
    Format(FmtError),
    /// The save failed; the backup is in place. Before the save's commit point the generation on disk is as it was;
    /// with [`SaveError::Incomplete`] of the new generation it is decided and completes at the next open.
    Save {
        /// The save's error.
        error: SaveError,
        /// The backup.
        backup: Backup,
        /// What the repair decided: present only with [`SaveError::Incomplete`] of the new generation, whose next open
        /// completes it without the broken lines; `None` for a failure before the save's commit point, which
        /// left the generation and its broken lines as they were.
        decided: Option<Box<Decided>>,
    },
}

/// What a repair whose save is incomplete after its commit point decided ([`RepairError::Save`]): as [`Repaired`]
/// gives it on success.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Decided {
    /// The lines discarded, in file and line order.
    pub discarded: Vec<DiscardedLine>,
    /// The files the save rewrites or removes, in path order.
    pub files: Vec<Rewrite>,
}

impl fmt::Display for RepairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepairError::NoProject => f.write_str("the session has no project directory"),
            RepairError::Dirty => {
                f.write_str("the session has unsaved changes; a repair writes only the saved project")
            }
            RepairError::ReadOnly(reason) => write!(f, "the session is read-only ({reason:?})"),
            RepairError::Pack(e) => write!(f, "the backup cannot be packed: {}", pack_words(e)),
            RepairError::NoFolder(p) => {
                write!(f, "{} has no parent folder to hold the backup", p.display())
            }
            RepairError::BackupTaken(p) => write!(
                f,
                "{} already exists and does not hold this generation's backup",
                p.display()
            ),
            RepairError::Backup { path, error } => {
                write!(f, "the backup cannot be written ({}: {error})", path.display())
            }
            RepairError::Format(e) => e.fmt(f),
            RepairError::Save { error, .. } => write!(f, "the save failed: {error}"),
        }
    }
}

impl std::error::Error for RepairError {}

/// Why the backup cannot be packed, in the repair's words: the pack's own texts name steps (save first) that a
/// project that opens read-only cannot take.
fn pack_words(e: &PackError) -> String {
    match e {
        PackError::Generation(GenerationError::NoManifest) => "the project has no manifest.json".to_owned(),
        PackError::Generation(GenerationError::Incomplete) => "a save is incomplete".to_owned(),
        PackError::ChangedSinceOpen { path } => {
            format!("{path} changed on disk since the repair opened the project; run the repair again")
        }
        PackError::Generation(GenerationError::Io { path, error }) | PackError::Io { path, error } => {
            format!("{}: {error}", path.display())
        }
        PackError::Refused(why) => format!("an .ewz cannot hold it ({why})"),
        // Not `pack_as_found`'s: a pack as found takes a changed or pending file as it is.
        PackError::Generation(e @ (GenerationError::Changed { .. } | GenerationError::Pending { .. })) => {
            e.to_string()
        }
    }
}

/// A finding in words for a refusal: its rule and where (`EW-CORE-006 at pages/p1.jsonl:3`).
pub fn finding_words(f: &Finding) -> String {
    let code = f.code().unwrap_or("a finding");
    match (&f.file, f.line) {
        (Some(file), Some(line)) => format!("{code} at {file}:{line}"),
        (Some(file), None) => format!("{code} at {file}"),
        _ => code.to_owned(),
    }
}

/// The first finding of `report` that forces a read-only session and is not a broken line: what `ew fmt --repair`
/// does not repair and names in its refusal. `None` when the open has none in its list: then the one that forced it is
/// beyond the findings cap (the loader's own flag counted it), or the open is read-only for another reason.
pub fn beyond_repair(report: &OpenReport) -> Option<&Finding> {
    report
        .findings
        .iter()
        .find(|f| f.kind.forces_read_only() && !f.is_broken_line())
}

/// The backup's path for the store's project at `generation`: `<project folder>.before-repair-g<generation>.ewz` in
/// the parent folder of the project folder, as the file system names it (links and `.` resolved).
///
/// # Errors
///
/// [`RepairError::NoFolder`] when the project folder has no name or no parent.
pub fn backup_path(vfs: &dyn Vfs, root: &Path, generation: u64) -> Result<PathBuf, RepairError> {
    beside(vfs, root, &format!("{BACKUP_INFIX}{generation}"))
}

/// `<project folder><infix>.ewz` in the parent folder of the project folder at `root`, as the file system names it:
/// [`backup_path`]'s, and an upgrade's backup's.
///
/// # Errors
///
/// [`RepairError::NoFolder`] when the project folder has no name or no parent.
pub(crate) fn beside(vfs: &dyn Vfs, root: &Path, infix: &str) -> Result<PathBuf, RepairError> {
    let root = vfs.canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let (Some(name), Some(parent)) = (root.file_name(), root.parent()) else {
        return Err(RepairError::NoFolder(root.clone()));
    };
    let mut file = name.to_owned();
    file.push(format!("{infix}.{}", ewz::EXTENSION));
    Ok(parent.join(file))
}

/// The bytes read and compared in one step when an existing backup is checked or the written one read back.
const COMPARE_CHUNK: usize = 1 << 20;

/// Whether the regular file at `path` holds exactly `bytes`: its kind and length first, then its content chunk by chunk
/// (a file of another size is never read).
fn holds(vfs: &dyn Vfs, path: &Path, bytes: &[u8]) -> io::Result<bool> {
    let stat = vfs.stat(path)?;
    if stat.kind != EntryKind::File || stat.len != bytes.len() as u64 {
        return Ok(false);
    }
    let mut offset = 0u64;
    for chunk in bytes.chunks(COMPARE_CHUNK) {
        if vfs.read_at(path, offset, chunk.len())? != chunk {
            return Ok(false);
        }
        offset += chunk.len() as u64;
    }
    Ok(true)
}

/// Writes the backup `bytes` at `target` without replacing anything; `Ok(true)` when a file there
/// already held these bytes and is reused. Otherwise: the temporary name `<target>.tmp` beside it (a leftover of an
/// interrupted repair of this project removed first: the caller holds its lock), the bytes written, read back and
/// synced, moved to `target` by [`Vfs::rename_noreplace`] (on a file system without hard links, by [`Vfs::rename`]
/// after `target` is checked absent again, the race between the two accepted as for the extract) and the
/// folder synced. Any failure removes what this call created; a power loss leaves at most the temporary file.
pub(crate) fn write_backup(vfs: &dyn Vfs, target: &Path, bytes: &[u8]) -> Result<bool, RepairError> {
    let failed = |error: io::Error| RepairError::Backup {
        path: target.to_path_buf(),
        error,
    };
    let taken = || RepairError::BackupTaken(target.to_path_buf());
    match vfs.stat(target) {
        Ok(_) => {
            return if holds(vfs, target, bytes).map_err(failed)? {
                Ok(true)
            } else {
                Err(taken())
            };
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(failed(e)),
    }
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = target.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    // Only a target that this call made holds `bytes`: another file there is never removed.
    let undo = |placed: bool| {
        let _ = vfs.remove_file(&temp);
        if placed && holds(vfs, target, bytes).unwrap_or(false) {
            let _ = vfs.remove_file(target);
        }
        let _ = vfs.sync_dir(parent);
    };
    let _ = vfs.remove_file(&temp);
    if let Err(e) = vfs.create_new(&temp, bytes) {
        undo(false);
        return Err(failed(e));
    }
    let written = vfs
        .sync(&temp)
        .and_then(|()| holds(vfs, &temp, bytes))
        .and_then(|same| {
            if same {
                Ok(())
            } else {
                Err(io::Error::other(
                    "the bytes read back differ from the bytes written",
                ))
            }
        });
    if let Err(e) = written {
        undo(false);
        return Err(failed(e));
    }
    match vfs.rename_noreplace(&temp, target) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            undo(false);
            return Err(taken());
        }
        // No rename without replacement on this file system (no hard links): the target's absence is checked
        // again and the temporary file, synced and read back, renamed onto it, so that a power loss leaves at most the
        // temporary file, which a rerun removes. The race between the check and the rename is accepted, as ID-1175
        // accepts it for the extract.
        Err(e) if e.kind() == io::ErrorKind::Unsupported => {
            match vfs.stat(target) {
                Ok(_) => {
                    undo(false);
                    return Err(taken());
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => {
                    undo(false);
                    return Err(failed(e));
                }
            }
            // A failed rename moved nothing: the target is not ours.
            if let Err(e) = vfs.rename(&temp, target) {
                undo(false);
                return Err(failed(e));
            }
        }
        // A rename by link and unlink can fail after the link: the target is then ours, and goes too.
        Err(e) => {
            undo(true);
            return Err(failed(e));
        }
    }
    if let Err(e) = vfs.sync_dir(parent) {
        // The name may not be durable: take the backup back, so that the failure leaves none.
        undo(true);
        return Err(failed(e));
    }
    Ok(false)
}

/// `ew fmt --repair` (see the module documentation): the backup, then one save without the broken lines. The session
/// must be a fresh editing open with [`StoreOptions::repair`](crate::StoreOptions) (the CLI refuses an `.ewz`, a crashed
/// session's journal and every other forcing finding before it gets here). Without a broken line nothing is written and
/// the result holds no backup.
///
/// # Errors
///
/// [`RepairError`]: every variant but [`RepairError::Save`] wrote nothing; with it the backup is in place, and a save
/// incomplete after its commit point carries what the repair decided ([`Decided`]).
pub fn repair(session: &mut Session) -> Result<Repaired, RepairError> {
    let store = session.store().ok_or(RepairError::NoProject)?;
    if let Access::ReadOnly(reason) = store.access() {
        return Err(RepairError::ReadOnly(reason.clone()));
    }
    if session.is_dirty() {
        return Err(RepairError::Dirty);
    }
    // A store whose files cannot be packed as found refuses, before the answer that
    // nothing is to be repaired.
    let found = ewz::pack_as_found(store).map_err(RepairError::Pack)?;
    let discarded: Vec<DiscardedLine> = store
        .broken_lines()
        .into_iter()
        .map(|b| DiscardedLine {
            file: b.file.as_str().to_owned(),
            line: b.line,
            text: b.text,
        })
        .collect();
    if discarded.is_empty() {
        return Ok(Repaired {
            generation: store.generation(),
            discarded,
            files: Vec::new(),
            backup: None,
        });
    }
    let plan = Plan {
        files: store
            .repair_rewrite()
            .map_err(|e| RepairError::Format(FmtError::Render(e)))?,
    };
    let lines: BTreeSet<(String, u32)> = discarded.iter().map(|d| (d.file.clone(), d.line)).collect();
    verify_content_except(session, &plan, &lines).map_err(RepairError::Format)?;
    let target = backup_path(&**store.vfs(), store.root(), found.packed.generation)?;
    let reused = write_backup(&**store.vfs(), &target, &found.packed.bytes)?;
    let backup = Backup {
        path: target,
        generation: found.packed.generation,
        reused,
        edited: found.edited,
    };
    let Some(persistence) = session.persistence.as_mut() else {
        return Err(RepairError::NoProject);
    };
    match persistence.store.discard_broken_lines() {
        Ok(saved) => Ok(Repaired {
            generation: saved.generation,
            discarded,
            files: plan.files(),
            backup: Some(backup),
        }),
        Err(error) => {
            // Past the commit point of this save (a generation above the backup's), the repair has taken effect.
            let decided =
                matches!(&error, SaveError::Incomplete { generation, .. } if *generation > backup.generation)
                    .then(|| {
                        Box::new(Decided {
                            discarded,
                            files: plan.files(),
                        })
                    });
            Err(RepairError::Save {
                error,
                backup,
                decided,
            })
        }
    }
}

/// The content of one line (or of a whole library document) for the comparison of [`verify_content`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Content {
    /// One JSON value in a canonical spelling: members in key order, numbers by value, strings in NFC.
    Value(String),
    /// Bytes that are not one JSON value.
    Raw(Vec<u8>),
}

/// The contents of `bytes` as the file `path`: one per non-empty line of a record file (a byte order mark and CR line
/// ends aside), one for a non-empty library document. Each with its 1-based line.
fn contents(path: &FilePath, bytes: &[u8]) -> Vec<(u32, Content)> {
    let data = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if matches!(classify(path.as_str()), PathClass::LibDocument { .. }) {
        return if data.is_empty() {
            Vec::new()
        } else {
            vec![(1, content_of(data))]
        };
    }
    data.split(|b| *b == b'\n')
        .enumerate()
        .filter_map(|(i, line)| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                return None;
            }
            Some((u32::try_from(i + 1).unwrap_or(u32::MAX), content_of(line)))
        })
        .collect()
}

fn content_of(bytes: &[u8]) -> Content {
    let value = std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| ew_schema::parse_value(text).ok());
    match value {
        Some(mut value) => {
            ew_schema::normalize_nfc(&mut value);
            let mut out = String::new();
            spell(&value, &mut out);
            Content::Value(out)
        }
        None => Content::Raw(bytes.to_vec()),
    }
}

/// The largest integer a binary64 float holds exactly with all smaller ones (2^53).
const EXACT_FLOAT_INTEGERS: f64 = 9_007_199_254_740_992.0;

/// Writes `value` in the canonical spelling of [`Content::Value`].
fn spell(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.push_str(&i.to_string());
            } else if let Some(u) = n.as_u64() {
                out.push_str(&u.to_string());
            } else {
                let f = n.as_f64().unwrap_or(f64::NAN);
                if f.fract() == 0.0 && f.abs() < EXACT_FLOAT_INTEGERS {
                    // A whole number spelled as a float (`1.0`, `1e2`): the same value as the integer.
                    out.push_str(&(f as i64).to_string());
                } else {
                    out.push_str(&f.to_string());
                }
            }
        }
        Value::String(s) => out.push_str(&Value::String(s.clone()).to_string()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                spell(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut members: Vec<(&String, &Value)> = map.iter().collect();
            members.sort_by(|a, b| a.0.cmp(b.0));
            out.push('{');
            for (i, (key, item)) in members.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                spell(item, out);
            }
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(path: &str, bytes: &str) -> Vec<(u32, Content)> {
        contents(&FilePath::new(path.to_owned()).unwrap(), bytes.as_bytes())
    }

    #[test]
    fn form_compares_equal_and_content_does_not() {
        let a = records("devices/3f.jsonl", "{\"b\":1,\"a\":\"\u{e9}\",\"c\":[1,2]}\n");
        // Member order, white space, a CR line end, an escape, NFD and a float spelling of 1 are form.
        let b = records(
            "devices/3f.jsonl",
            "\u{feff}\n{ \"a\" : \"e\u{301}\", \"c\": [1, 2], \"b\": 1.0 }\r\n\n",
        );
        assert_eq!(a[0].1, b[0].1);
        assert_eq!(b[0].0, 2, "the line numbers of the file on disk");
        // An array's order, a dropped member and a changed value are content.
        for other in [
            "{\"b\":1,\"a\":\"\u{e9}\",\"c\":[2,1]}",
            "{\"a\":\"\u{e9}\",\"c\":[1,2]}",
            "{\"b\":2,\"a\":\"\u{e9}\",\"c\":[1,2]}",
            "{\"b\":1,\"a\":\"\u{e9}\",\"c\":[1,2],\"d\":null}",
        ] {
            assert_ne!(a[0].1, records("devices/3f.jsonl", other)[0].1, "{other}");
        }
        // A line that is not one JSON value compares by its bytes.
        assert_eq!(
            records("devices/3f.jsonl", "<<<<<<< ours\n")[0].1,
            Content::Raw(b"<<<<<<< ours".to_vec())
        );
        // Large integers keep their value; floats that are not whole compare by value.
        assert_ne!(
            records("p.jsonl", "9007199254740993")[0].1,
            records("p.jsonl", "9007199254740992")[0].1
        );
        assert_eq!(records("p.jsonl", "0.5")[0].1, records("p.jsonl", "5e-1")[0].1);
    }

    #[test]
    fn a_library_document_is_one_value() {
        let doc = "{\n  \"$k\": \"symbol\",\n  \"lines\": [1,\n 2]\n}\n";
        let found = records("lib/symbols/0de5c40f-dcf9-560b-98a1-71a96115aaea.json", doc);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 1);
    }
}
