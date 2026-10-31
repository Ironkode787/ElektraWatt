// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project store: open with roll-forward, save protocol, incremental saves, lock and journal (doc 05
//! §5–§8).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ew_format::ew_schema::{AnyId, FORMAT};
use ew_format::ew_schema::{AssetFormat, AssetRef, Sha256};
use ew_format::{
    Access as FormatAccess, FileHash, FilePath, Finding, FindingKind, Kept, Limits, LineKey, MANIFEST,
    Manifest, OpaqueKind, PathClass, classify, hash_text, jsonl_file, sha256, unknown_record_file,
};
use ew_model::{AnyKey, AnyRow, Asset, Model, ModelError};

use crate::clock::{Clock, SystemClock};
use crate::journal::{self, Journal, RecoveryInfo, ReplayError, Segment};
use crate::lock::{
    self, Acquire, Holder, Identity, LockInfo, LockRecord, OsProbe, ProcessProbe, STALE_AFTER, TakeoverError,
};
use crate::rows::{
    all_keys, document_row, file_of, follow_closure, is_library, record_row, row_line_key, row_text,
};
use crate::vfs::{EntryKind, OsVfs, Vfs, is_transient, join};

mod purge;

pub use purge::{PurgeError, PurgeFile, PurgePlan};

/// How long a read-only opener waits for a save in progress (doc 05 §6.1, §6.4: 200 ms steps, 5 s).
pub const READ_ONLY_WAIT: Duration = Duration::from_secs(5);
/// The poll step of that wait.
pub const READ_ONLY_POLL: Duration = Duration::from_millis(200);
/// The backoff of a rename that antivirus or a sync client blocks (doc 05 §5.2 step 6: 10 … 1,000 ms).
pub const RENAME_BACKOFF: [Duration; 5] = [
    Duration::from_millis(10),
    Duration::from_millis(30),
    Duration::from_millis(100),
    Duration::from_millis(300),
    Duration::from_millis(1_000),
];
/// The deepest directory level the store lists below the project root.
const MAX_WALK_DEPTH: usize = 16;

/// Whether a session edits or only reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpenMode {
    /// Take the lock and edit; read-only when another session holds it.
    Edit,
    /// Never take the lock, never roll forward (a CLI reader, a viewer).
    ReadOnly,
}

/// The options of a store.
#[derive(Clone, Debug)]
pub struct StoreOptions {
    /// The file system.
    pub vfs: Arc<dyn Vfs>,
    /// The clock.
    pub clock: Arc<dyn Clock>,
    /// Process liveness for the stale-lock rule.
    pub probe: Arc<dyn ProcessProbe>,
    /// Who we are (the lock record, the journal's session).
    pub identity: Identity,
    /// Edit or read-only.
    pub mode: OpenMode,
    /// The OS state directory for the journal; `None`: no journal ("unsaved changes are not
    /// crash-protected", doc 05 §8.1).
    pub state_dir: Option<PathBuf>,
    /// `lock.stale_after` (doc 05 §7.4).
    pub stale_after: Duration,
    /// The reader limits (doc 05 §3.6).
    pub limits: Limits,
    /// Whether this is a release build (doc 05 §10.1: `unstable` projects open read-only).
    pub release_build: bool,
    /// How long a read-only opener waits for a save in progress.
    pub read_only_wait: Duration,
}

impl StoreOptions {
    /// The production options: `std::fs`, the system clock, the OS probe, this process with a fresh
    /// session, edit mode, the default state directory, a 60-minute stale age, the default limits.
    pub fn new(app_version: &str) -> StoreOptions {
        StoreOptions {
            vfs: Arc::new(OsVfs),
            clock: Arc::new(SystemClock),
            probe: Arc::new(OsProbe),
            identity: Identity::current(app_version),
            mode: OpenMode::Edit,
            state_dir: default_state_dir(),
            stale_after: STALE_AFTER,
            limits: Limits::default(),
            release_build: !cfg!(debug_assertions),
            read_only_wait: READ_ONLY_WAIT,
        }
    }
}

/// The OS state directory of doc 05 §1: `%LOCALAPPDATA%\ElektraWatt\state` on Windows,
/// `~/Library/Application Support/ElektraWatt/state` on macOS, `$XDG_STATE_HOME/elektrawatt` (default
/// `~/.local/state/elektrawatt`) elsewhere. `None` when the environment names none.
pub fn default_state_dir() -> Option<PathBuf> {
    // The per-user state directory (lock, journal) is outside the project; no project byte depends on it.
    let var = |n: &str| std::env::var_os(n).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        var("LOCALAPPDATA").map(|p| p.join("ElektraWatt").join("state"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|p| p.join("Library/Application Support/ElektraWatt/state"))
    } else {
        var("XDG_STATE_HOME")
            .map(|p| p.join("elektrawatt"))
            .or_else(|| var("HOME").map(|p| p.join(".local/state/elektrawatt")))
    }
}

/// Why a session is read-only.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ReadOnlyReason {
    /// The caller asked for it.
    Requested,
    /// Another session holds the lock (doc 05 §7.3).
    LockedBy(Box<LockInfo>),
    /// The lock cannot be created (EACCES, EROFS: a read-only share).
    NotWritable(String),
    /// An integrity finding forces it (EW-CORE-002, -003, -005, -006, a limit).
    Integrity,
    /// The format stamps (doc 05 §10.1).
    Format(&'static str),
    /// The lock was lost (taken over) during the session (FR-COL-026).
    LockLost(Option<Box<LockRecord>>),
}

/// The access a store has.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Access {
    /// We hold the lock and may save.
    Edit,
    /// Reading only.
    ReadOnly(ReadOnlyReason),
}

/// What an open found.
#[derive(Clone, PartialEq, Debug)]
pub struct OpenReport {
    /// Edit or read-only, with the reason.
    pub access: Access,
    /// The generation loaded (0: no manifest).
    pub generation: u64,
    /// The format decision of the manifest (doc 05 §10.1).
    pub format: FormatAccess,
    /// The integrity findings, sorted.
    pub findings: Vec<Finding>,
    /// The generation an interrupted save was rolled forward to (doc 05 §5.3).
    pub rolled_forward: Option<u64>,
    /// Temps and torn intents deleted (doc 05 §5.3 row 1).
    pub removed: Vec<String>,
    /// A leftover journal of another session (doc 05 §8.3).
    pub recovery: Option<RecoveryInfo>,
    /// Whether unsaved changes are crash-protected (a journal is open).
    pub journaled: bool,
}

/// Why a project does not open.
#[derive(Debug)]
pub enum OpenError {
    /// An I/O error.
    Io {
        /// The path.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
    /// The path is not a project directory.
    NotADirectory(PathBuf),
    /// The format refuses (doc 05 §10.1: `min_reader` above this reader, unreadable stamps).
    Format(String),
    /// An older major needs the migration chain (WP-S1.17).
    Migration(String),
    /// A read-only opener met a save in progress for longer than the wait (doc 05 §6.4: exit code 2).
    ///
    /// Not returned by [`Store::open`] since review cycle 8: after [`StoreOptions::read_only_wait`] a
    /// read-only opener reads the intent's generation in memory instead (a crashed writer's lock stays
    /// non-stale for `lock.stale_after` where the process probe cannot tell, on Windows). Kept for the CLI's
    /// exit code until the design pass settles doc 05 §6.4.
    SaveInProgress,
    /// The files hold no project record.
    NoProject(Vec<Finding>),
    /// The model refuses the records (a duplicate, a broken invariant, a dangling reference).
    Model {
        /// The model's error.
        error: ModelError,
        /// The format findings.
        findings: Vec<Finding>,
    },
    /// A takeover did not happen.
    Takeover(Box<TakeoverError>),
    /// `create` into a directory that already holds a project.
    Exists(PathBuf),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            OpenError::NotADirectory(p) => write!(f, "{} is not a project directory", p.display()),
            OpenError::Format(m) => write!(f, "the project cannot be opened: {m}"),
            OpenError::Migration(m) => write!(f, "the project needs a format migration: {m}"),
            OpenError::SaveInProgress => f.write_str("a save is in progress; try again"),
            OpenError::NoProject(_) => f.write_str("the project holds no project record"),
            OpenError::Model { error, .. } => write!(f, "the project does not load: {error}"),
            OpenError::Takeover(e) => write!(f, "takeover failed: {e:?}"),
            OpenError::Exists(p) => write!(f, "{} already holds a project", p.display()),
        }
    }
}

impl std::error::Error for OpenError {}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> OpenError + '_ {
    move |error| OpenError::Io {
        path: path.to_path_buf(),
        error,
    }
}

/// The canonical project directory of one generation, read back and verified ([`Store::read_generation`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Generation {
    /// The generation.
    pub generation: u64,
    /// Its manifest.
    pub manifest: Manifest,
    /// The bytes of `manifest.json`.
    pub manifest_bytes: Vec<u8>,
    /// The bytes of every file the manifest lists, by path.
    pub files: BTreeMap<FilePath, Vec<u8>>,
}

/// Why a generation could not be read back ([`Store::read_generation`]).
#[derive(Debug)]
pub enum GenerationError {
    /// The project has no manifest yet.
    NoManifest,
    /// A save is incomplete: the generation on disk is being completed.
    Incomplete,
    /// A file is not what the manifest says (or `manifest.json` is not the one loaded): changed on disk.
    Changed {
        /// The project-relative path.
        path: String,
    },
    /// The manifest's hash of a file is pending (a merged project).
    Pending {
        /// The project-relative path.
        path: String,
    },
    /// A file could not be read.
    Io {
        /// The path.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
}

impl fmt::Display for GenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerationError::NoManifest => f.write_str("the project has no manifest yet; save it first"),
            GenerationError::Incomplete => f.write_str("a save is incomplete; wait for it to complete"),
            GenerationError::Changed { path } => {
                write!(
                    f,
                    "{path} changed on disk since it was saved (an external change)"
                )
            }
            GenerationError::Pending { path } => {
                write!(
                    f,
                    "the manifest's hash of {path} is pending (a merged project); save it first"
                )
            }
            GenerationError::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for GenerationError {}

/// Which rows changed since the last save.
#[derive(Clone, Copy, Debug)]
pub enum Changes<'a> {
    /// The keys put since the last save (the transaction log's dirty set, doc 05 §5.1).
    Keys(&'a BTreeSet<AnyKey>),
    /// Every row: a full save, the oracle of NFR-REL-16.
    All,
}

/// The steps of the save protocol (doc 05 §5.2), for error reports and the fault-injection harness.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SaveStep {
    /// Step 2: the lock and manifest pre-checks.
    PreCheck,
    /// Step 4: writing and syncing the temps.
    Temps,
    /// Step 5: the intent manifest and the directory syncs (the commit point follows).
    Intent,
    /// Step 6: renaming the temps.
    Renames,
    /// Step 7: renaming the intent over `manifest.json`.
    Manifest,
}

/// Why a save did not complete.
#[derive(Debug)]
pub enum SaveError {
    /// The session is read-only.
    ReadOnly(ReadOnlyReason),
    /// The lock was lost (taken over): the session is now read-only, nothing was written (doc 05 §5.4).
    LockLost(Option<Box<LockRecord>>),
    /// `manifest.json` changed since we loaded or wrote it: an external change (doc 05 §9) must be
    /// merged first; nothing was written.
    ExternalChange,
    /// A failure before the commit point: generation g is intact, our temps are deleted, the project stays
    /// dirty (NFR-REL-03).
    Failed {
        /// The step.
        step: SaveStep,
        /// The path.
        path: PathBuf,
        /// The error.
        error: io::Error,
    },
    /// A failure after the commit point of **this** save: generation `generation` is decided, holds the
    /// model this save was given, and completes by roll-forward ([`Store::retry_pending`], and at the next
    /// open). "Save incomplete" (doc 05 §5.4). The caller may count the model as saved.
    Incomplete {
        /// The decided generation.
        generation: u64,
        /// The step.
        step: SaveStep,
        /// The error.
        error: io::Error,
    },
    /// An **earlier** save (generation `generation`) passed its commit point but is still incomplete, and
    /// completing it failed again: this save wrote nothing. The model given to this save is not on disk;
    /// the caller keeps its dirty set and saves again later (review cycle 8, blocker 1).
    PendingIncomplete {
        /// The decided generation of the earlier save.
        generation: u64,
        /// Why its roll-forward failed.
        error: io::Error,
    },
    /// A record has no file or no JSON form.
    Format(ew_format::FormatError),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::ReadOnly(r) => write!(f, "read-only session: {r:?}"),
            SaveError::LockLost(_) => f.write_str("the lock was taken over; the session is read-only"),
            SaveError::ExternalChange => f.write_str("the project changed on disk; merge the change first"),
            SaveError::Failed { step, path, error } => {
                write!(
                    f,
                    "save failed at {step:?} ({}): {error}; nothing changed on disk",
                    path.display()
                )
            }
            SaveError::Incomplete {
                generation,
                step,
                error,
            } => write!(
                f,
                "save incomplete at {step:?} (generation {generation}): {error}"
            ),
            SaveError::PendingIncomplete { generation, error } => write!(
                f,
                "the earlier save of generation {generation} is still incomplete ({error}); nothing was saved"
            ),
            SaveError::Format(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<ew_format::FormatError> for SaveError {
    fn from(e: ew_format::FormatError) -> SaveError {
        SaveError::Format(e)
    }
}

/// What a save did.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SaveReport {
    /// The generation now on disk.
    pub generation: u64,
    /// The files written.
    pub written: Vec<FilePath>,
    /// The files removed.
    pub removed: Vec<FilePath>,
    /// Whether `manifest.json` was written (false: an unchanged project wrote nothing).
    pub manifest_written: bool,
    /// Removals that failed (reported as EW-CORE-004 at the next open).
    pub leftovers: Vec<FilePath>,
    /// A journal write that failed (the save itself succeeded).
    pub journal_error: Option<String>,
    /// What the save (or a roll-forward completed since the last save) found and did on disk: EW-CORE-004
    /// for each file that sat at a path the save writes without being in the baseline, moved aside to its
    /// [`FilePath::unlisted`] name, which the finding names.
    pub findings: Vec<Finding>,
}

/// The state of the lock after a heartbeat.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LockState {
    /// We hold it; its modification time was refreshed.
    Held,
    /// We lost it: the session is now read-only (FR-COL-026).
    Lost(Option<LockRecord>),
    /// A read-only session holds no lock.
    NotHeld,
}

/// The result of a recovery (doc 05 §8.2).
#[derive(Clone, Debug)]
pub struct Recovered {
    /// The model with the journalled transactions replayed.
    pub model: Model,
    /// The keys they touched (the dirty set of the next save).
    pub keys: BTreeSet<AnyKey>,
    /// The number of transactions replayed.
    pub transactions: usize,
    /// What the replay could not restore (assets the journal carried damaged, EW-CORE-015); the next save's
    /// report carries them too.
    pub findings: Vec<Finding>,
}

/// Why a recovery failed.
#[derive(Debug)]
pub enum RecoverError {
    /// There is no leftover journal.
    Nothing,
    /// The replay failed.
    Replay(ReplayError),
    /// The journal could not be written.
    Io(io::Error),
}

impl fmt::Display for RecoverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecoverError::Nothing => f.write_str("no journal to recover"),
            RecoverError::Replay(e) => e.fmt(f),
            RecoverError::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for RecoverError {}

/// The outcome of a roll-forward attempt (doc 05 §5.3).
enum Roll {
    /// The intent was completed to this generation; the findings name files moved aside (EW-CORE-004).
    Completed(u64, Vec<Finding>),
    /// The intent was torn (unreadable): deleted with its temps.
    Torn,
    /// For some file neither the target nor its temp matches the intent: EW-CORE-002.
    NotRepairable(Vec<FilePath>),
    /// The bytes of `manifest.json` are not those the save was based on (the intent's [`BASE_MEMBER`]): the
    /// project changed after the save was decided, and completing it would overwrite that change. Nothing
    /// is touched (EW-CORE-002, review cycle 8, 5(j)).
    BaseMoved(BaseMismatch),
}

/// The intent member that records the SHA-256 of the raw bytes of the `manifest.json` a save was based on
/// (`"base": "sha256:<hex>"`; absent when there was no manifest). The intent becomes `manifest.json`, so
/// every manifest written by a save carries the hash of the one it replaced. The roll-forward compares raw
/// bytes, never parsed generations: a conflicted or unparsable manifest whose bytes are unchanged rolls
/// forward (review cycle 8, 5(j) rework, owner ruling).
pub const BASE_MEMBER: &str = "base";

/// Why an intent is not completed: its recorded base and the `manifest.json` on disk, as `sha256:<hex>`
/// texts (`None`: no base recorded / no manifest on disk).
#[derive(Clone, PartialEq, Eq, Debug)]
struct BaseMismatch {
    recorded: Option<String>,
    on_disk: Option<String>,
}

/// The recorded base of `intent` against the raw hash of the manifest on disk: `None` when they agree
/// (both absent included), the mismatch otherwise. A recorded base that is not a string counts as absent.
fn base_check(intent: &Manifest, on_disk: Option<String>) -> Option<BaseMismatch> {
    let recorded = intent
        .extra
        .get(BASE_MEMBER)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    (recorded != on_disk).then_some(BaseMismatch { recorded, on_disk })
}

/// Whether `path` is a temp the save protocol creates (doc 05 §5.2 steps 4 and 5): the intent
/// `manifest.json.tmp-<g>`, or `<file>.tmp-<g>` of a writer-owned file (a record file or a library
/// document of a known directory). Other `*.tmp-*` names (a user's or another tool's) are not ours to
/// delete (review cycle 8, 5(f)).
fn is_save_temp(path: &str) -> bool {
    match classify(path) {
        PathClass::Intent { .. } => true,
        PathClass::Temp { target, .. } => matches!(
            classify(&target),
            PathClass::Records(_)
                | PathClass::LibDocument { .. }
                | PathClass::Opaque(OpaqueKind::Asset { .. })
        ),
        _ => false,
    }
}

/// Whether the asset's file is among `opaque` with the asset's hash: its bytes are on disk.
fn asset_on_disk(opaque: &BTreeMap<FilePath, FileHash>, r: &AssetRef) -> bool {
    FilePath::new(r.path()).is_ok_and(|path| opaque.get(&path) == Some(&FileHash::Sha256(r.sha256.0)))
}

/// The asset of an opaque file `path` with `bytes` hashing to `hash`: an `assets/<sha256>.<ext>` file whose name
/// is its hash and whose bytes sniff as the format of its extension; `None` otherwise.
fn asset_of(path: &FilePath, hash: &[u8; 32], bytes: Option<&Vec<u8>>) -> Option<Asset> {
    let PathClass::Opaque(OpaqueKind::Asset { sha256: named }) = classify(path.as_str()) else {
        return None;
    };
    let bytes = bytes?;
    let ext = path.as_str().rsplit_once('.')?.1;
    let format = AssetFormat::from_ext(ext)?;
    if named != *hash {
        return None;
    }
    let (sniffed, size) = ew_format::ew_schema::sniff(bytes).ok()?;
    (sniffed == format).then(|| {
        Asset::new(
            AssetRef {
                sha256: Sha256(*hash),
                format,
                size,
            },
            bytes.as_slice(),
        )
    })
}

/// EW-CORE-002 for an interrupted save of generation `g` that is not applied because the manifest on disk
/// is not its base (review cycle 8, 5(j)).
fn base_moved(g: u64, m: &BaseMismatch) -> Finding {
    let what = match (&m.recorded, &m.on_disk) {
        (None, _) => "its intent records no base manifest, but manifest.json exists".to_owned(),
        (Some(r), None) => format!("it was based on manifest.json {r}, which is now gone"),
        (Some(r), Some(d)) => format!("it was based on manifest.json {r}, which is now {d}"),
    };
    Finding::new(
        FindingKind::MixedGeneration,
        FilePath::new(format!("{MANIFEST}.tmp-{g}")).ok(),
        None,
        format!(
            "an interrupted save of generation {g} is not completed, so that it cannot overwrite a newer \
             state: {what}; its changes stay in the interrupted session's journal"
        ),
    )
}

/// The newest intent above generation `current` in `listing`: the save a read-only opener waits for or
/// reads in memory, and an editing opener rolls forward.
fn newest_intent(listing: &BTreeMap<String, EntryKind>, current: u64) -> Option<u64> {
    listing
        .keys()
        .filter_map(|p| match classify(p) {
            PathClass::Intent { generation } if generation > current => Some(generation),
            _ => None,
        })
        .max()
}

/// An open project directory: the baseline of the last load or save, the lock and the journal.
///
/// Everything a save writes is a function of the model and this baseline, so the same model saves to the
/// same bytes (NFR-REL-07), and an incremental save equals a full one (NFR-REL-16).
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    opts: StoreOptions,
    access: Access,
    holds_lock: bool,
    /// The model as of the last load or save (old rows and old files of the dirty set).
    base: Model,
    generation: u64,
    manifest: Option<Manifest>,
    /// The SHA-256 of `manifest.json` as on disk (`None`: absent).
    manifest_hash: Option<[u8; 32]>,
    /// Whether the manifest's format stamp is kept (a newer minor, doc 05 §10.1).
    keep_format: bool,
    /// Writer-owned files (record files and library documents of known kinds) on disk, with their hashes.
    disk: BTreeMap<FilePath, [u8; 32]>,
    /// Files kept by hash only.
    opaque: BTreeMap<FilePath, FileHash>,
    /// The model keys of every writer-owned file.
    contents: BTreeMap<FilePath, BTreeSet<AnyKey>>,
    /// Lines written verbatim, as their bytes: unknown kinds and quarantined lines (a line that is not
    /// UTF-8 included), per file.
    kept: BTreeMap<FilePath, Vec<(LineKey, Vec<u8>)>>,
    /// Records with malformed variants: their original line while the row equals the loaded one.
    untouched: BTreeMap<AnyKey, (Vec<u8>, AnyRow)>,
    /// Library documents with malformed variants, likewise.
    untouched_docs: BTreeMap<AnyKey, (Vec<u8>, AnyRow)>,
    /// Files to rewrite at the next save (non-canonical at open, misplaced records).
    rewrite: BTreeSet<FilePath>,
    /// The generation of a save that passed its commit point but did not complete.
    pending: Option<u64>,
    /// The removals of that save (doc 05 §5.2 step 8), done when it completes.
    pending_removes: Vec<FilePath>,
    /// Findings of roll-forwards completed outside a save, handed out with the next save's report.
    notes: Vec<Finding>,
    /// Whether the next save writes every asset the model holds, not only the named ones (Save As: kept lines
    /// and unknown members may name assets that no known record names).
    all_assets: bool,
    /// The files that hold quarantined lines (EW-CORE-005, EW-CORE-007), kept verbatim: the asset purge is refused
    /// while there are any (doc 05 §13.3, FT-380).
    quarantined: BTreeSet<FilePath>,
    journal: Option<Journal>,
    leftovers: Vec<Segment>,
    recovery: Option<RecoveryInfo>,
    closed: bool,
}

/// An opened project.
#[derive(Debug)]
pub struct Opened {
    /// The store.
    pub store: Store,
    /// The model loaded.
    pub model: Model,
    /// What the open found.
    pub report: OpenReport,
}

impl Store {
    // -----------------------------------------------------------------------------------------------------
    // Open

    /// Opens a project directory (doc 05 §6.1): lock, roll-forward or temp clean-up, manifest, hashes,
    /// parse, integrity, model, leftover journal.
    ///
    /// # Errors
    ///
    /// [`OpenError`]; integrity findings are not errors: they are in the report, and those that force it
    /// make the session read-only.
    pub fn open(root: &Path, opts: StoreOptions) -> Result<Opened, OpenError> {
        Store::open_inner(root, opts, false)
    }

    /// Takes over a stale lock (doc 05 §7.4; the caller has confirmed) and opens the project for editing.
    ///
    /// # Errors
    ///
    /// [`OpenError::Takeover`] when the lock is not stale or another instance won; otherwise as
    /// [`Store::open`].
    pub fn take_over(root: &Path, opts: StoreOptions) -> Result<Opened, OpenError> {
        let vfs = &*opts.vfs;
        match lock::take_over(
            vfs,
            root,
            &*opts.clock,
            opts.stale_after,
            &opts.identity,
            &*opts.probe,
        )
        .map_err(io_err(root))?
        {
            Ok(_) => Store::open_inner(root, opts, true),
            Err(e) => Err(OpenError::Takeover(Box::new(e))),
        }
    }

    fn open_inner(root: &Path, opts: StoreOptions, locked: bool) -> Result<Opened, OpenError> {
        let vfs = Arc::clone(&opts.vfs);
        match vfs.stat(root) {
            Ok(s) if s.kind == EntryKind::Dir => {}
            Ok(_) => return Err(OpenError::NotADirectory(root.to_path_buf())),
            Err(e) => return Err(io_err(root)(e)),
        }
        let mut access = Access::Edit;
        let mut holds_lock = locked;
        if opts.mode == OpenMode::ReadOnly {
            access = Access::ReadOnly(ReadOnlyReason::Requested);
        } else if !locked {
            match lock::acquire(
                &*vfs,
                root,
                &*opts.clock,
                opts.stale_after,
                &opts.identity,
                &*opts.probe,
            )
            .map_err(io_err(root))?
            {
                Acquire::Held => holds_lock = true,
                Acquire::HeldBy(info) => access = Access::ReadOnly(ReadOnlyReason::LockedBy(Box::new(info))),
                Acquire::NotWritable(why) => access = Access::ReadOnly(ReadOnlyReason::NotWritable(why)),
            }
        }
        let mut store = Store {
            root: root.to_path_buf(),
            opts,
            access,
            holds_lock,
            base: Model::new(placeholder_project()),
            generation: 0,
            manifest: None,
            manifest_hash: None,
            keep_format: false,
            disk: BTreeMap::new(),
            opaque: BTreeMap::new(),
            contents: BTreeMap::new(),
            kept: BTreeMap::new(),
            untouched: BTreeMap::new(),
            untouched_docs: BTreeMap::new(),
            rewrite: BTreeSet::new(),
            pending: None,
            pending_removes: Vec::new(),
            notes: Vec::new(),
            all_assets: false,
            quarantined: BTreeSet::new(),
            journal: None,
            leftovers: Vec::new(),
            recovery: None,
            closed: false,
        };
        // From here on, an error drops `store`, which releases the lock.
        let result = store.load();
        match result {
            Ok((model, report)) => Ok(Opened { store, model, report }),
            Err(e) => Err(e),
        }
    }

    fn load(&mut self) -> Result<(Model, OpenReport), OpenError> {
        let vfs = Arc::clone(&self.opts.vfs);
        let mut findings: Vec<Finding> = Vec::new();
        let mut removed = Vec::new();
        let mut rolled_forward = None;
        let mut repairable = true;
        let mut abandoned: Option<u64> = None;

        // Intents and temps (doc 05 §5.3, §6.1).
        let mut listing = self.walk().map_err(io_err(&self.root))?;
        if self.holds_lock {
            let current = self
                .read_manifest()
                .ok()
                .flatten()
                .map_or(0, |(m, _)| m.generation);
            if let Some(newest) = newest_intent(&listing, current) {
                match self.roll_forward(newest).map_err(io_err(&self.root))? {
                    Roll::Completed(g, moved) => {
                        rolled_forward = Some(g);
                        findings.extend(moved);
                    }
                    Roll::Torn => {}
                    Roll::NotRepairable(files) => {
                        repairable = false;
                        for f in files {
                            findings.push(Finding::new(
                                FindingKind::MixedGeneration,
                                Some(f),
                                None,
                                format!(
                                    "an interrupted save of generation {newest} cannot roll forward: \
                                     neither the file nor its temp matches the intent"
                                ),
                            ));
                        }
                    }
                    Roll::BaseMoved(mismatch) => {
                        repairable = false;
                        findings.push(base_moved(newest, &mismatch));
                    }
                }
            }
            if repairable {
                listing = self.walk().map_err(io_err(&self.root))?;
                for path in listing.keys() {
                    if is_save_temp(path) {
                        let full = join(&self.root, path);
                        if vfs.remove_file(&full).is_ok() {
                            removed.push(path.clone());
                        }
                    }
                }
                // Files a save removed after its manifest rename but before it could delete them (doc
                // 05 §5.3 row 3) are named by the crashed session's journal.
                if let Ok(Some((m, _))) = self.read_manifest() {
                    self.prepare_journal_dir(Some(m.project));
                    for seg in &self.leftovers {
                        for path in seg.removes_of(m.generation) {
                            let Ok(p) = FilePath::new(path.clone()) else {
                                continue;
                            };
                            // An asset file is in `removes` only when the purge removed it.
                            let owned = matches!(
                                classify(p.as_str()),
                                PathClass::Records(_)
                                    | PathClass::LibDocument { .. }
                                    | PathClass::Opaque(OpaqueKind::Asset { .. })
                            );
                            if owned
                                && !m.files.contains_key(&p)
                                && vfs.remove_file(&join(&self.root, p.as_str())).is_ok()
                            {
                                removed.push(path);
                            }
                        }
                    }
                }
                lock::prune_stale_locks(&*vfs, &self.root, self.opts.clock.now(), self.opts.stale_after);
                listing = self.walk().map_err(io_err(&self.root))?;
            }
        } else {
            // Read-only openers never roll forward. Only an intent above the manifest's generation is a save
            // in progress (an older one is a leftover the next editing open deletes). Behind a live writer
            // they wait for its save to finish; behind a crashed one (a stale lock, or none) waiting is
            // pointless. Either way, an intent still there after the wait is read in memory: the intent's
            // generation, file by file from the target or its temp, and nothing is written. (On Windows a
            // crashed writer's lock stays non-stale for `lock.stale_after`: the process probe cannot tell.)
            let current = |store: &Store| {
                store
                    .read_manifest()
                    .ok()
                    .flatten()
                    .map_or(0, |(m, _)| m.generation)
            };
            let mut newest = newest_intent(&listing, current(self));
            if newest.is_some() && !self.writer_gone() {
                let mut waited = Duration::ZERO;
                while newest.is_some() && waited < self.opts.read_only_wait {
                    self.opts.clock.sleep(READ_ONLY_POLL);
                    waited += READ_ONLY_POLL;
                    listing = self.walk().map_err(io_err(&self.root))?;
                    newest = newest_intent(&listing, current(self));
                }
            }
            abandoned = newest;
        }

        // The manifest and the format decision (doc 05 §4, §10.1).
        let mut format = FormatAccess::ReadWrite;
        let mut pending_removal: BTreeSet<FilePath> = BTreeSet::new();
        let mut intent = abandoned.and_then(|g| {
            let bytes = vfs.read(&join(&self.root, &format!("{MANIFEST}.tmp-{g}"))).ok()?;
            // A torn intent never committed: generation g on disk is the truth.
            Manifest::parse(&bytes, &self.opts.limits)
                .ok()
                .filter(|m| m.generation == g && !m.merged)
        });
        if let Some(i) = &intent {
            // As for a roll-forward: an intent whose recorded base is not the manifest on disk, byte for
            // byte, is not applied.
            let on_disk = self.manifest_base().map_err(io_err(&self.root))?;
            if let Some(mismatch) = base_check(i, on_disk) {
                findings.push(base_moved(i.generation, &mismatch));
                intent = None;
            }
        }
        if let Some(intent) = &intent {
            if let Ok(Some((old, _))) = self.read_manifest() {
                pending_removal = old
                    .files
                    .keys()
                    .filter(|p| !intent.files.contains_key(*p))
                    .cloned()
                    .collect();
            }
            // A file the interrupted save creates exists only as its temp so far.
            for path in intent.files.keys() {
                if !listing.contains_key(path.as_str()) && listing.contains_key(&path.temp(intent.generation))
                {
                    listing.insert(path.as_str().to_owned(), EntryKind::File);
                }
            }
        }
        let manifest = match intent
            .clone()
            .map(|m| Ok(Some((m, [0u8; 32]))))
            .unwrap_or_else(|| self.read_manifest())
        {
            Ok(Some((m, hash))) => {
                self.manifest_hash = Some(hash);
                Some(m)
            }
            Ok(None) => None,
            Err(e) => {
                if let Ok(bytes) = vfs.read(&join(&self.root, MANIFEST)) {
                    self.manifest_hash = Some(sha256(&bytes));
                }
                findings.push(Finding::new(
                    FindingKind::ManifestUnreadable,
                    FilePath::new(MANIFEST).ok(),
                    None,
                    e,
                ));
                None
            }
        };
        if manifest.is_none()
            && self.manifest_hash.is_none()
            && listing
                .keys()
                .any(|p| matches!(classify(p), PathClass::Records(_)))
        {
            findings.push(Finding::new(
                FindingKind::ManifestUnreadable,
                FilePath::new(MANIFEST).ok(),
                None,
                "manifest.json is missing; it is rebuilt at the next save",
            ));
        }
        if let Some(m) = &manifest {
            format = ew_format::reader_access(m, FORMAT, self.opts.release_build);
            match &format {
                FormatAccess::Refuse(why) => return Err(OpenError::Format(why.clone())),
                FormatAccess::Migrate => return Err(OpenError::Migration(m.format.clone())),
                FormatAccess::ReadOnly(why) => self.demote(ReadOnlyReason::Format(why)),
                FormatAccess::ReadWriteNewer => self.keep_format = true,
                FormatAccess::ReadWrite => {}
            }
            self.generation = m.generation;
        }

        // Plan, read, parse (doc 05 §6.1, §6.2, §6.3).
        let limits = self.opts.limits;
        let plan = ew_format::plan(manifest.as_ref(), &listing, &limits);
        // The files the interrupted save removes are not orphans.
        findings.extend(plan.findings.into_iter().filter(|f| {
            !(f.kind == FindingKind::Orphan && f.file.as_ref().is_some_and(|p| pending_removal.contains(p)))
        }));
        let mut files = BTreeMap::new();
        for path in plan.read {
            let full = join(&self.root, path.as_str());
            match vfs.stat(&full) {
                Ok(s) if s.len > limits.max_file_bytes => {
                    findings.push(Finding::new(
                        FindingKind::LimitExceeded,
                        Some(path),
                        None,
                        format!("{} bytes, more than the limit; not read", s.len),
                    ));
                    continue;
                }
                _ => {}
            }
            let read = match (&intent, vfs.read(&full)) {
                (Some(intent), target) => {
                    let wanted = intent.files.get(&path);
                    match target {
                        Ok(bytes) if wanted.is_none_or(|h| h.matches(&bytes)) => Ok(bytes),
                        target => {
                            let temp = vfs.read(&join(&self.root, &path.temp(intent.generation)));
                            // A live writer may rename the temp between the two reads: read the target
                            // once more before calling the file mixed.
                            let temp = match temp {
                                Ok(bytes) if wanted.is_some_and(|h| h.matches(&bytes)) => Ok(bytes),
                                _ => vfs.read(&full),
                            };
                            match temp {
                                Ok(bytes) if wanted.is_some_and(|h| h.matches(&bytes)) => Ok(bytes),
                                _ => {
                                    findings.push(Finding::new(
                                        FindingKind::MixedGeneration,
                                        Some(path.clone()),
                                        None,
                                        format!(
                                            "an interrupted save of generation {}: neither the file nor its \
                                             temp matches the intent",
                                            intent.generation
                                        ),
                                    ));
                                    target
                                }
                            }
                        }
                    }
                }
                (None, target) => target,
            };
            match read {
                Ok(bytes) => {
                    files.insert(path, bytes);
                }
                Err(e) => findings.push(Finding::new(
                    FindingKind::MissingFile,
                    Some(path),
                    None,
                    format!("cannot be read: {e}"),
                )),
            }
        }
        let loaded = ew_format::load(manifest.as_ref(), &files, &limits);
        // The loader's list is capped at `max_findings`; its flag also counts the findings it dropped
        // (review cycle 8, major 2: a thousand duplicates must not hide a line the writer would lose).
        let loader_read_only = loaded.read_only;
        findings.extend(loaded.findings);
        findings.sort();
        let Some(project) = loaded.project else {
            return Err(OpenError::NoProject(findings));
        };
        let mut model = match Model::from_records(project, loaded.records, loaded.lib) {
            Ok(m) => m,
            Err(error) => return Err(OpenError::Model { error, findings }),
        };
        // The asset store (doc 05 §13.3): every asset file whose bytes match the hash in its name and the format
        // of its extension. Others stay opaque files only (EW-CORE-015 reports a wrong hash).
        for (path, hash) in &loaded.opaque {
            if let Some(asset) = asset_of(path, hash, files.get(path)) {
                model.put_asset(asset);
            }
        }
        if !repairable || loader_read_only || findings.iter().any(|f| f.kind.forces_read_only()) {
            self.demote(ReadOnlyReason::Integrity);
        }

        // The baseline.
        for (path, hash) in &loaded.hashes {
            if matches!(
                classify(path.as_str()),
                PathClass::Records(_) | PathClass::LibDocument { .. }
            ) && !loaded.opaque.contains_key(path)
            {
                self.disk.insert(path.clone(), *hash);
            }
        }
        self.opaque = loaded
            .opaque
            .into_iter()
            .map(|(p, h)| (p, FileHash::Sha256(h)))
            .collect();
        for line in loaded.kept {
            if matches!(line.kept, Kept::Quarantined) {
                self.quarantined.insert(line.file.clone());
            }
            match line.kept {
                Kept::UnknownKind | Kept::Quarantined => {
                    self.kept
                        .entry(line.file)
                        .or_default()
                        .push((line.key, line.text));
                }
                Kept::Untouched { original } => {
                    if let Some(row) = record_row(*original) {
                        self.untouched.insert(row.key(), (line.text, row));
                    }
                }
            }
        }
        for doc in loaded.kept_docs {
            if let Some(row) = document_row(*doc.original) {
                self.untouched_docs.insert(row.key(), (doc.text, row));
            }
        }
        self.rewrite = loaded.rewrite;
        for key in all_keys(&model) {
            if let Some(file) = file_of(&model, &key).map_err(|e| OpenError::Format(e.to_string()))? {
                self.contents.entry(file).or_default().insert(key);
            }
        }
        self.manifest = manifest;
        self.base = model.clone();

        // The journal (doc 05 §8): only an editing session journals.
        if self.access == Access::Edit {
            self.prepare_journal_dir(Some(model.project().id.cast()));
            // Leftovers without unsaved transactions (a session that crashed or was dropped right after a
            // save, or never committed) hold nothing to recover: delete them. Of the rest, the one with the
            // latest change is offered (doc 05 §8.2 step 1).
            let generation = self.generation;
            let (mut keep, empty): (Vec<Segment>, Vec<Segment>) = std::mem::take(&mut self.leftovers)
                .into_iter()
                .partition(|seg| !seg.unsaved(generation).is_empty());
            if let Some(j) = &self.journal {
                j.discard(&empty);
            }
            keep.sort_by_key(|seg| std::cmp::Reverse(seg.last_change(generation)));
            self.leftovers = keep;
            self.recovery = journal::summarise(&self.leftovers, self.generation, self.manifest_hash.as_ref());
            if let Some(j) = &mut self.journal {
                if j.start(self.generation, self.manifest_hash.as_ref()).is_err() {
                    self.journal = None;
                }
            }
        } else {
            self.journal = None;
            self.leftovers.clear();
        }
        if self.access != Access::Edit && self.holds_lock {
            self.release_lock();
        }
        let report = OpenReport {
            access: self.access.clone(),
            generation: self.generation,
            format,
            findings,
            rolled_forward,
            removed,
            recovery: self.recovery.clone(),
            journaled: self.journal.is_some(),
        };
        Ok((model, report))
    }

    /// Creates the journal handle for project `project` and reads the leftovers of other sessions (once).
    fn prepare_journal_dir(&mut self, project: Option<AnyId>) {
        if self.journal.is_some() {
            return;
        }
        let (Some(state), Some(project)) = (self.opts.state_dir.clone(), project) else {
            return;
        };
        let j = Journal::new(
            Arc::clone(&self.opts.vfs),
            &state,
            project,
            &self.root,
            &self.opts.identity.session,
            &self.opts.identity.app_version,
        );
        self.leftovers = j.leftovers(self.opts.clock.now());
        self.journal = Some(j);
    }

    /// Whether no live session holds the lock: the lock is stale, absent or unreadable as a record. A
    /// read-only opener then treats an intent as abandoned by a crashed writer.
    fn writer_gone(&self) -> bool {
        if let Access::ReadOnly(ReadOnlyReason::LockedBy(info)) = &self.access {
            return info.stale;
        }
        match lock::read_lock(
            &*self.opts.vfs,
            &self.root,
            self.opts.clock.now(),
            self.opts.stale_after,
            &self.opts.identity,
            &*self.opts.probe,
        ) {
            Ok(None) => true,
            Ok(Some(info)) => info.stale,
            Err(_) => false,
        }
    }

    /// The SHA-256 of the raw bytes of `manifest.json` as `sha256:<hex>`, `None` when absent: what an intent
    /// records as its [`BASE_MEMBER`].
    fn manifest_base(&self) -> io::Result<Option<String>> {
        match self.opts.vfs.read(&join(&self.root, MANIFEST)) {
            Ok(bytes) => Ok(Some(hash_text(&sha256(&bytes)))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Reads and parses `manifest.json`: `Ok(None)` when absent, `Err(message)` when unusable.
    fn read_manifest(&self) -> Result<Option<(Manifest, [u8; 32])>, String> {
        let path = join(&self.root, MANIFEST);
        match self.opts.vfs.read(&path) {
            Ok(bytes) => Manifest::parse(&bytes, &self.opts.limits)
                .map(|m| Some((m, sha256(&bytes))))
                .map_err(|e| e.to_string()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Every entry below the root by project-relative path (links not followed; `.ew/`, `.git/` and
    /// `.~ew.requests/` not entered), up to the file limit.
    fn walk(&self) -> io::Result<BTreeMap<String, EntryKind>> {
        let mut out = BTreeMap::new();
        let mut stack: Vec<(String, usize)> = vec![(String::new(), 0)];
        while let Some((rel, depth)) = stack.pop() {
            let dir = if rel.is_empty() {
                self.root.clone()
            } else {
                join(&self.root, &rel)
            };
            let entries = match self.opts.vfs.list(&dir) {
                Ok(e) => e,
                Err(e) if e.kind() == io::ErrorKind::NotFound && !rel.is_empty() => continue,
                Err(e) => return Err(e),
            };
            for (name, kind) in entries {
                let path = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                if kind == EntryKind::Dir
                    && depth < MAX_WALK_DEPTH
                    && !(rel.is_empty() && matches!(name.as_str(), ".ew" | ".git" | ".~ew.requests"))
                {
                    stack.push((path.clone(), depth + 1));
                }
                out.insert(path, kind);
                if out.len() > self.opts.limits.max_files + 64 {
                    return Ok(out);
                }
            }
        }
        Ok(out)
    }

    /// Completes the save of generation `g` from its intent (doc 05 §5.3), when the raw bytes of
    /// `manifest.json` are still those the intent records as its base ([`BASE_MEMBER`]). A file at a target
    /// path that the base does not list and that does not already hold the intent's bytes is moved aside
    /// first ([`Store::move_aside`]).
    fn roll_forward(&mut self, g: u64) -> io::Result<Roll> {
        let vfs = Arc::clone(&self.opts.vfs);
        let intent_rel = format!("{MANIFEST}.tmp-{g}");
        let intent_path = join(&self.root, &intent_rel);
        let delete_temps = |store: &Store| -> io::Result<()> {
            for path in store.walk()?.keys() {
                if let PathClass::Temp { generation, .. } | PathClass::Intent { generation } = classify(path)
                {
                    if generation == g && is_save_temp(path) {
                        let _ = vfs.remove_file(&join(&store.root, path));
                    }
                }
            }
            Ok(())
        };
        let intent_bytes = vfs.read(&intent_path)?;
        let intent = match Manifest::parse(&intent_bytes, &self.opts.limits) {
            Ok(m) if m.generation == g && !m.merged => m,
            _ => {
                delete_temps(self)?;
                return Ok(Roll::Torn);
            }
        };
        // The base: the manifest the save started from, byte for byte (its pre-check, doc 05 §5.2 step 2).
        if let Some(mismatch) = base_check(&intent, self.manifest_base()?) {
            return Ok(Roll::BaseMoved(mismatch));
        }
        // The base's file list, when it parses. A conflicted or otherwise unusable manifest (EW-CORE-001)
        // made the save read every writer-owned file present, so all of them were its baseline.
        let old = self.read_manifest().ok().flatten().map(|(m, _)| m);
        let listed = |p: &FilePath| old.as_ref().is_none_or(|m| m.files.contains_key(p));
        let mut renames = Vec::new();
        let mut missing = Vec::new();
        for (path, hash) in &intent.files {
            let FileHash::Sha256(h) = hash else {
                missing.push(path.clone());
                continue;
            };
            let target = join(&self.root, path.as_str());
            if vfs.read(&target).is_ok_and(|b| sha256(&b) == *h) {
                continue;
            }
            let temp = join(&self.root, &path.temp(g));
            if vfs.read(&temp).is_ok_and(|b| sha256(&b) == *h) {
                let occupied = !listed(path)
                    && vfs
                        .stat(&target)
                        .is_ok_and(|s| matches!(s.kind, EntryKind::File | EntryKind::Link));
                renames.push((path.clone(), temp, target, occupied));
            } else {
                missing.push(path.clone());
            }
        }
        if !missing.is_empty() {
            return Ok(Roll::NotRepairable(missing));
        }
        let mut dirs = BTreeSet::new();
        let mut moved = Vec::new();
        for (path, temp, target, occupied) in renames {
            if occupied {
                moved.push(self.move_aside(&path, g)?);
            }
            self.rename_retrying(&temp, &target)?;
            if let Some(parent) = target.parent() {
                dirs.insert(parent.to_path_buf());
            }
        }
        // Files of the old generation that the new one no longer lists: removed before the manifest
        // rename, while the old manifest still names them.
        if let Some(old) = &old {
            for path in old.files.keys() {
                // An asset file leaves a manifest only through the purge (a save never removes one).
                let owned = matches!(
                    classify(path.as_str()),
                    PathClass::Records(_)
                        | PathClass::LibDocument { .. }
                        | PathClass::Opaque(OpaqueKind::Asset { .. })
                );
                if owned && !intent.files.contains_key(path) {
                    let _ = vfs.remove_file(&join(&self.root, path.as_str()));
                }
            }
        }
        for dir in dirs.iter().filter(|d| **d != self.root) {
            vfs.sync_dir(dir)?;
        }
        self.rename_retrying(&intent_path, &join(&self.root, MANIFEST))?;
        vfs.sync_dir(&self.root)?;
        Ok(Roll::Completed(g, moved))
    }

    /// Moves the file at `path`, which the baseline does not list, to a free [`FilePath::unlisted`] name
    /// for a save of generation `g`, so that the save never renames over another user's file (review cycle
    /// 8, owner ruling). The rename does not replace: a taken name is skipped. Returns the EW-CORE-004
    /// finding naming the new path.
    fn move_aside(&self, path: &FilePath, g: u64) -> io::Result<Finding> {
        const ATTEMPTS: u32 = 1_000;
        let vfs = &*self.opts.vfs;
        let from = join(&self.root, path.as_str());
        for n in 0..ATTEMPTS {
            let aside = path.unlisted(g, n);
            let to = join(&self.root, aside.as_str());
            let moved = match vfs.rename_noreplace(&from, &to) {
                Ok(()) => true,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => false,
                // No rename without replacement here (doc 05 §7.4 step 3): check the name, then rename.
                Err(e) if e.kind() == io::ErrorKind::Unsupported => match vfs.stat(&to) {
                    Ok(_) => false,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        self.rename_retrying(&from, &to)?;
                        true
                    }
                    Err(e) => return Err(e),
                },
                Err(e) => return Err(e),
            };
            if moved {
                return Ok(Finding::new(
                    FindingKind::Orphan,
                    Some(aside),
                    None,
                    format!(
                        "found at {path}, which manifest.json does not list (another user's or a sync \
                         client's file); moved here so that the save of generation {g} does not overwrite it"
                    ),
                ));
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{path}: no free name to move the unlisted file aside"),
        ))
    }

    fn rename_retrying(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut attempt = 0;
        loop {
            match self.opts.vfs.rename(from, to) {
                Ok(()) => return Ok(()),
                Err(e) if is_transient(&e) && attempt < RENAME_BACKOFF.len() => {
                    self.opts.clock.sleep(RENAME_BACKOFF[attempt]);
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn demote(&mut self, reason: ReadOnlyReason) {
        if self.access == Access::Edit {
            self.access = Access::ReadOnly(reason);
        }
    }

    fn release_lock(&mut self) {
        if self.holds_lock {
            let _ = lock::release(&*self.opts.vfs, &self.root, &self.opts.identity);
            self.holds_lock = false;
        }
    }

    // -----------------------------------------------------------------------------------------------------
    // Create

    /// Writes `model` as generation 1 into `root` (a new project, Save As), which must not hold a
    /// manifest, and keeps it open for editing.
    ///
    /// # Errors
    ///
    /// [`OpenError::Exists`] when `root` already holds a project; I/O and lock errors; a save error as
    /// [`OpenError::Format`].
    pub fn create(root: &Path, model: &Model, opts: StoreOptions) -> Result<Store, OpenError> {
        let vfs = Arc::clone(&opts.vfs);
        vfs.create_dir_all(root).map_err(io_err(root))?;
        if vfs.stat(&join(root, MANIFEST)).is_ok() {
            return Err(OpenError::Exists(root.to_path_buf()));
        }
        let Opened { mut store, .. } = Store::open_inner_empty(root, opts)?;
        // Records of reserved or unknown kinds without a source file: placed by their rule.
        for record in model.unknown_records() {
            let Some(file) = unknown_record_file(match record {
                ew_format::ew_schema::Record::Unknown(v) => v,
                _ => continue,
            }) else {
                return Err(OpenError::Format(format!(
                    "no file for a record of kind {:?}",
                    record.kind_name()
                )));
            };
            let line = ew_format::record_line(record).map_err(|e| OpenError::Format(e.to_string()))?;
            let key = LineKey::of(record).unwrap_or_else(LineKey::keyless);
            store.kept.entry(file).or_default().push((key, line.into_bytes()));
        }
        store.all_assets = true;
        let saved = store.save(model, Changes::All);
        store.all_assets = false;
        saved.map_err(|e| OpenError::Format(e.to_string()))?;
        store.prepare_journal_dir(Some(model.project().id.cast()));
        if let Some(j) = &mut store.journal {
            if j.start(store.generation, store.manifest_hash.as_ref()).is_err() {
                store.journal = None;
            }
        }
        Ok(store)
    }

    /// An editing store over a directory without a project.
    fn open_inner_empty(root: &Path, opts: StoreOptions) -> Result<Opened, OpenError> {
        let vfs = Arc::clone(&opts.vfs);
        let held = lock::acquire(
            &*vfs,
            root,
            &*opts.clock,
            opts.stale_after,
            &opts.identity,
            &*opts.probe,
        )
        .map_err(io_err(root))?;
        let access = match held {
            Acquire::Held => Access::Edit,
            Acquire::HeldBy(info) => Access::ReadOnly(ReadOnlyReason::LockedBy(Box::new(info))),
            Acquire::NotWritable(why) => Access::ReadOnly(ReadOnlyReason::NotWritable(why)),
        };
        let store = Store {
            root: root.to_path_buf(),
            holds_lock: access == Access::Edit,
            access: access.clone(),
            opts,
            base: Model::new(placeholder_project()),
            generation: 0,
            manifest: None,
            manifest_hash: None,
            keep_format: false,
            disk: BTreeMap::new(),
            opaque: BTreeMap::new(),
            contents: BTreeMap::new(),
            kept: BTreeMap::new(),
            untouched: BTreeMap::new(),
            untouched_docs: BTreeMap::new(),
            rewrite: BTreeSet::new(),
            pending: None,
            pending_removes: Vec::new(),
            notes: Vec::new(),
            all_assets: false,
            quarantined: BTreeSet::new(),
            journal: None,
            leftovers: Vec::new(),
            recovery: None,
            closed: false,
        };
        if let Access::ReadOnly(reason) = access {
            return Err(OpenError::Format(format!("cannot create: {reason:?}")));
        }
        let model = store.base.clone();
        Ok(Opened {
            store,
            model,
            report: OpenReport {
                access: Access::Edit,
                generation: 0,
                format: FormatAccess::ReadWrite,
                findings: Vec::new(),
                rolled_forward: None,
                removed: Vec::new(),
                recovery: None,
                journaled: false,
            },
        })
    }

    // -----------------------------------------------------------------------------------------------------
    // Save

    /// Saves `model` (doc 05 §5.2): serialise the files of the dirty set, drop those equal to their
    /// hashes, then temps with `sync_all`, the intent manifest, directory syncs, `save-begin` in the
    /// journal (the commit point), renames, the manifest rename, removals, `save-done`. An unchanged
    /// project writes nothing.
    ///
    /// # Errors
    ///
    /// [`SaveError`]: before the commit point nothing on disk changed; after it the generation is decided
    /// and completes by roll-forward.
    pub fn save(&mut self, model: &Model, changes: Changes<'_>) -> Result<SaveReport, SaveError> {
        self.save_with(model, changes, &BTreeSet::new())
    }

    /// [`Store::save`] that also removes the opaque files `drop` (the asset purge, doc 05 §13.3): they leave the
    /// manifest of the new generation, are journalled in `save-begin{removes}` and deleted in step 8 with the
    /// save's other removals, so a crash completes their removal by roll-forward or leaves the old generation.
    /// A path of `drop` that is not an opaque file of the baseline is ignored.
    fn save_with(
        &mut self,
        model: &Model,
        changes: Changes<'_>,
        drop: &BTreeSet<FilePath>,
    ) -> Result<SaveReport, SaveError> {
        if let Access::ReadOnly(reason) = &self.access {
            return Err(SaveError::ReadOnly(reason.clone()));
        }
        // An earlier save that is still incomplete must complete first; when it cannot, this save writes
        // nothing and says so with its own error, never with the earlier save's `Incomplete` (review cycle
        // 8, blocker 1: the caller would count this model as saved).
        if self.pending.is_some() {
            if let Err(e) = self.retry_pending() {
                return Err(match e {
                    SaveError::Incomplete {
                        generation, error, ..
                    } => SaveError::PendingIncomplete { generation, error },
                    other => other,
                });
            }
        }
        // Step 2: the lock still holds our session, and the manifest is what we last loaded or wrote.
        match lock::holder(&*self.opts.vfs, &self.root, &self.opts.identity) {
            Ok(Holder::Us) => {}
            Ok(Holder::Other(rec)) => return Err(self.lose_lock(rec)),
            Ok(Holder::Nobody) => return Err(self.lose_lock(None)),
            Err(error) => {
                return Err(SaveError::Failed {
                    step: SaveStep::PreCheck,
                    path: join(&self.root, ew_format::LOCK),
                    error,
                });
            }
        }
        let manifest_path = join(&self.root, MANIFEST);
        let on_disk = match self.opts.vfs.read(&manifest_path) {
            Ok(b) => Some(sha256(&b)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(SaveError::Failed {
                    step: SaveStep::PreCheck,
                    path: manifest_path,
                    error,
                });
            }
        };
        if on_disk != self.manifest_hash {
            return Err(SaveError::ExternalChange);
        }

        // Step 3: serialise the dirty files.
        let mut plan = self.plan(model, changes)?;
        let drop: BTreeSet<FilePath> = drop
            .iter()
            .filter(|p| self.opaque.contains_key(*p) && !plan.writes.contains_key(*p))
            .cloned()
            .collect();
        plan.removes.extend(drop.iter().cloned());
        let g = self.generation;
        let g1 = g + 1;
        let mut manifest = Manifest::new(
            FORMAT,
            &self.opts.identity.app_version,
            model.project().id.cast(),
            g1,
        );
        if self.keep_format {
            if let Some(old) = &self.manifest {
                manifest.format.clone_from(&old.format);
                manifest.min_reader.clone_from(&old.min_reader);
            }
        }
        if let Some(old) = &self.manifest {
            manifest.extra.clone_from(&old.extra);
            // The base of the previous save; this save records its own below.
            manifest.extra.remove(BASE_MEMBER);
        }
        manifest.files = plan
            .disk
            .iter()
            .map(|(p, h)| (p.clone(), FileHash::Sha256(*h)))
            .chain(
                self.opaque
                    .iter()
                    .filter(|(p, _)| !drop.contains(*p))
                    .map(|(p, h)| (p.clone(), *h)),
            )
            .chain(plan.assets.iter().map(|(p, h)| (p.clone(), *h)))
            .collect();
        let manifest_same = self.manifest.as_ref().is_some_and(|old| {
            let mut a = old.clone();
            a.generation = g1;
            a.generator.clone_from(&manifest.generator);
            a.extra.remove(BASE_MEMBER);
            a == manifest
        });
        // The raw bytes this save is based on, for the base check of a roll-forward (review cycle 8,
        // 5(j) rework): absent when there is no manifest.
        if let Some(h) = &on_disk {
            manifest
                .extra
                .insert(BASE_MEMBER.to_owned(), serde_json::Value::from(hash_text(h)));
        }
        if plan.writes.is_empty() && plan.removes.is_empty() && manifest_same {
            self.adopt(model, plan.contents, plan.disk);
            return Ok(SaveReport {
                generation: g,
                findings: std::mem::take(&mut self.notes),
                ..SaveReport::default()
            });
        }
        let manifest_text = manifest.to_text();
        let vfs = Arc::clone(&self.opts.vfs);

        // Steps 4 and 5: temps, intent, directory syncs. Any failure deletes our temps; g stays intact.
        let mut created: Vec<PathBuf> = Vec::new();
        let mut moved: Vec<Finding> = Vec::new();
        let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
        dirs.insert(self.root.clone());
        let pre = (|| -> Result<(), (SaveStep, PathBuf, io::Error)> {
            for (path, bytes) in &plan.writes {
                let target = join(&self.root, path.as_str());
                let mut dir = target.parent().map(Path::to_path_buf);
                if let Some(d) = &dir {
                    vfs.create_dir_all(d)
                        .map_err(|e| (SaveStep::Temps, d.clone(), e))?;
                }
                while let Some(d) = dir {
                    if d == self.root || !d.starts_with(&self.root) {
                        break;
                    }
                    dir = d.parent().map(Path::to_path_buf);
                    dirs.insert(d);
                }
                // Never rename over a file the baseline does not list (another user's file, a sync
                // client's copy): move it aside first (review cycle 8, owner ruling). Its directory is
                // synced with the temps below, so the move is durable before the commit point.
                if !self.disk.contains_key(path) {
                    match vfs.stat(&target) {
                        Ok(s) if matches!(s.kind, EntryKind::File | EntryKind::Link) => moved.push(
                            self.move_aside(path, g1)
                                .map_err(|e| (SaveStep::Temps, target.clone(), e))?,
                        ),
                        // A directory or a device: the rename of step 6 fails and reports it.
                        Ok(_) => {}
                        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                        Err(e) => return Err((SaveStep::Temps, target.clone(), e)),
                    }
                }
                let temp = join(&self.root, &path.temp(g1));
                let _ = vfs.remove_file(&temp);
                vfs.create_new(&temp, bytes)
                    .map_err(|e| (SaveStep::Temps, temp.clone(), e))?;
                created.push(temp.clone());
                vfs.sync(&temp).map_err(|e| (SaveStep::Temps, temp.clone(), e))?;
            }
            let intent = join(&self.root, &format!("{MANIFEST}.tmp-{g1}"));
            let _ = vfs.remove_file(&intent);
            vfs.create_new(&intent, manifest_text.as_bytes())
                .map_err(|e| (SaveStep::Intent, intent.clone(), e))?;
            created.push(intent.clone());
            vfs.sync(&intent)
                .map_err(|e| (SaveStep::Intent, intent.clone(), e))?;
            // Subdirectories first, the root (which holds the intent) last: a durable intent name implies
            // durable temp names, so a power loss never leaves an intent whose temps are gone.
            for d in dirs
                .iter()
                .filter(|d| **d != self.root)
                .chain(std::iter::once(&self.root))
            {
                vfs.sync_dir(d).map_err(|e| (SaveStep::Intent, d.clone(), e))?;
            }
            Ok(())
        })();
        // Files moved aside stay moved whatever happens next; the next successful save reports them.
        self.notes.append(&mut moved);
        if let Err((step, path, error)) = pre {
            for temp in created.iter().rev() {
                let _ = vfs.remove_file(temp);
            }
            // Make the removal of a possibly durable intent durable too, so that a later power loss cannot
            // bring back a generation this session reported as not saved.
            let _ = vfs.sync_dir(&self.root);
            return Err(SaveError::Failed { step, path, error });
        }
        let mut report = SaveReport {
            generation: g1,
            written: plan.writes.keys().cloned().collect(),
            removed: Vec::new(),
            manifest_written: true,
            leftovers: Vec::new(),
            journal_error: None,
            findings: Vec::new(),
        };
        let removes: Vec<FilePath> = plan.removes.iter().cloned().collect();
        if let Some(j) = &mut self.journal {
            if let Err(e) = j.save_begin(g1, plan.writes.len(), &removes) {
                report.journal_error = Some(e.to_string());
            }
        }
        // Commit point: generation g1 is decided.
        let manifest_hash = sha256(manifest_text.as_bytes());
        self.opaque.retain(|p, _| !drop.contains(p));
        self.opaque.extend(plan.assets);
        self.adopt(model, plan.contents, plan.disk);
        self.manifest = Some(manifest);
        self.manifest_hash = Some(manifest_hash);
        self.generation = g1;
        self.pending = Some(g1);
        self.pending_removes.clone_from(&removes);

        // Step 6: renames.
        for path in plan.writes.keys() {
            let temp = join(&self.root, &path.temp(g1));
            let target = join(&self.root, path.as_str());
            if let Err(error) = self.rename_retrying(&temp, &target) {
                return Err(SaveError::Incomplete {
                    generation: g1,
                    step: SaveStep::Renames,
                    error,
                });
            }
        }
        // The renamed names are made durable before the manifest names the new generation: with
        // per-directory sync semantics (ext4, APFS) a durable manifest rename must never precede a durable
        // data-file rename, or a power loss could leave manifest g+1 over files of g with no intent left.
        for d in dirs.iter().filter(|d| **d != self.root) {
            if let Err(error) = vfs.sync_dir(d) {
                return Err(SaveError::Incomplete {
                    generation: g1,
                    step: SaveStep::Renames,
                    error,
                });
            }
        }
        // Step 7: the manifest, then the root directory (which also holds the renamed root files).
        let intent = join(&self.root, &format!("{MANIFEST}.tmp-{g1}"));
        if let Err(error) = self.rename_retrying(&intent, &manifest_path) {
            return Err(SaveError::Incomplete {
                generation: g1,
                step: SaveStep::Manifest,
                error,
            });
        }
        if let Err(error) = vfs.sync_dir(&self.root) {
            return Err(SaveError::Incomplete {
                generation: g1,
                step: SaveStep::Manifest,
                error,
            });
        }
        self.pending = None;
        self.pending_removes.clear();
        // Step 8: removals.
        for path in removes {
            match vfs.remove_file(&join(&self.root, path.as_str())) {
                Ok(()) => report.removed.push(path),
                Err(e) if e.kind() == io::ErrorKind::NotFound => report.removed.push(path),
                Err(_) => report.leftovers.push(path),
            }
        }
        // Step 9: bookkeeping.
        if let Some(j) = &mut self.journal {
            if let Err(e) = j.save_done(g1, &manifest_hash) {
                report.journal_error = Some(e.to_string());
            }
        }
        report.findings = std::mem::take(&mut self.notes);
        Ok(report)
    }

    /// Completes a save that passed its commit point (doc 05 §5.4: retried every 5 s by the caller).
    ///
    /// # Errors
    ///
    /// [`SaveError::Incomplete`] while it still fails.
    pub fn retry_pending(&mut self) -> Result<Option<u64>, SaveError> {
        let Some(g) = self.pending else {
            return Ok(None);
        };
        let incomplete = |error| SaveError::Incomplete {
            generation: g,
            step: SaveStep::Renames,
            error,
        };
        let done = |store: &mut Store| {
            store.pending = None;
            for path in std::mem::take(&mut store.pending_removes) {
                let _ = store.opts.vfs.remove_file(&join(&store.root, path.as_str()));
            }
            if let (Some(j), Some(h)) = (&mut store.journal, store.manifest_hash) {
                let _ = j.save_done(g, &h);
            }
            Ok(Some(g))
        };
        // The manifest rename happened (`manifest.json` holds the bytes of our intent); only a directory
        // sync failed.
        let decided = self.manifest_hash.as_ref().map(hash_text);
        if decided.is_some() && self.manifest_base().ok().flatten() == decided {
            if let Err(e) = self.opts.vfs.sync_dir(&self.root) {
                return Err(incomplete(e));
            }
            return done(self);
        }
        // Like the pre-check of a save, the roll-forward requires the raw manifest the save started from
        // (its intent's recorded base, review cycle 8, 5(j)); otherwise an external change happened and
        // completing the save would overwrite it.
        match self.roll_forward(g) {
            Ok(Roll::Completed(_, moved)) => {
                self.notes.extend(moved);
                done(self)
            }
            Ok(Roll::Torn) => Err(incomplete(io::Error::other("the intent manifest is unreadable"))),
            Ok(Roll::NotRepairable(files)) => Err(incomplete(io::Error::other(format!(
                "{} files match neither the intent nor a temp (EW-CORE-002)",
                files.len()
            )))),
            Ok(Roll::BaseMoved(mismatch)) => {
                Err(incomplete(io::Error::other(base_moved(g, &mismatch).message)))
            }
            Err(e) => Err(incomplete(e)),
        }
    }

    fn lose_lock(&mut self, rec: Option<LockRecord>) -> SaveError {
        self.holds_lock = false;
        let rec = rec.map(Box::new);
        self.access = Access::ReadOnly(ReadOnlyReason::LockLost(rec.clone()));
        SaveError::LockLost(rec)
    }

    /// Makes `model` the baseline after a successful plan.
    fn adopt(
        &mut self,
        model: &Model,
        contents: BTreeMap<FilePath, BTreeSet<AnyKey>>,
        disk: BTreeMap<FilePath, [u8; 32]>,
    ) {
        for (path, keys) in contents {
            if keys.is_empty() {
                self.contents.remove(&path);
            } else {
                self.contents.insert(path, keys);
            }
        }
        self.disk = disk;
        self.rewrite.clear();
        self.base = model.clone();
    }

    /// Step 3 of doc 05 §5.2: the bytes of every dirty file and the files to remove.
    fn plan(&self, model: &Model, changes: Changes<'_>) -> Result<SavePlan, SaveError> {
        let closure: BTreeSet<AnyKey> = match changes {
            Changes::Keys(keys) => follow_closure(keys, &self.base, model),
            Changes::All => {
                let mut all = all_keys(model);
                all.extend(self.contents.values().flatten().copied());
                all
            }
        };
        let mut dirty: BTreeSet<FilePath> = self.rewrite.clone();
        if matches!(changes, Changes::All) {
            dirty.extend(self.disk.keys().cloned());
            dirty.extend(self.kept.keys().cloned());
            dirty.extend(self.contents.keys().cloned());
        }
        let mut contents: BTreeMap<FilePath, BTreeSet<AnyKey>> = BTreeMap::new();
        let entry = |path: &FilePath, contents: &mut BTreeMap<FilePath, BTreeSet<AnyKey>>| {
            if !contents.contains_key(path) {
                contents.insert(path.clone(), self.contents.get(path).cloned().unwrap_or_default());
            }
        };
        for key in &closure {
            let old = file_of(&self.base, key)?;
            let new = file_of(model, key)?;
            if let Some(o) = &old {
                entry(o, &mut contents);
                debug_assert!(
                    contents[o].contains(key) || self.base.row(key).is_none(),
                    "{key:?} is not in its old file {o}"
                );
                if let Some(set) = contents.get_mut(o) {
                    set.remove(key);
                }
                dirty.insert(o.clone());
            }
            if let Some(n) = &new {
                entry(n, &mut contents);
                if let Some(set) = contents.get_mut(n) {
                    set.insert(*key);
                }
                dirty.insert(n.clone());
            }
        }
        for path in &dirty {
            entry(path, &mut contents);
        }
        let mut writes = BTreeMap::new();
        let mut removes = BTreeSet::new();
        let mut disk = self.disk.clone();
        for path in &dirty {
            if self.opaque.contains_key(path) {
                continue;
            }
            let keys = &contents[path];
            let bytes = self.render(model, path, keys)?;
            if bytes.is_empty() {
                if disk.remove(path).is_some() {
                    removes.insert(path.clone());
                }
                continue;
            }
            let hash = sha256(&bytes);
            if disk.get(path) != Some(&hash) {
                disk.insert(path.clone(), hash);
                writes.insert(path.clone(), bytes);
            }
        }
        // The assets the records name that have no file yet (doc 05 §13.3); in Save As every asset the model
        // holds. A save never removes an asset file.
        let mut assets = BTreeMap::new();
        let wanted: Vec<Sha256> = if self.all_assets {
            model.assets().map(|a| *a.sha256()).collect()
        } else {
            model.referenced_assets().into_keys().collect()
        };
        for sha in &wanted {
            let Some(asset) = model.asset(sha) else {
                continue; // named but not held: nothing to write (the check reports it)
            };
            // `assets/<64 hex>.<ext>` is always a valid path.
            let Ok(path) = FilePath::new(asset.reference().path()) else {
                continue;
            };
            // On disk with its bytes: nothing to do. A file under that name whose bytes are not the asset's
            // (EW-CORE-015) is replaced by the asset the model holds.
            if self.opaque.get(&path) == Some(&FileHash::Sha256(sha.0)) {
                continue;
            }
            writes.insert(path.clone(), asset.bytes().to_vec());
            assets.insert(path, FileHash::Sha256(sha.0));
        }
        Ok(SavePlan {
            writes,
            removes,
            disk,
            contents,
            assets,
        })
    }

    /// The bytes of one writer-owned file from the model and the kept lines.
    fn render(&self, model: &Model, path: &FilePath, keys: &BTreeSet<AnyKey>) -> Result<Vec<u8>, SaveError> {
        if let PathClass::LibDocument { .. } = classify(path.as_str()) {
            let Some(key) = keys.iter().find(|k| is_library(k)) else {
                return Ok(Vec::new());
            };
            let Some(row) = model.row(key) else {
                return Ok(Vec::new());
            };
            if let Some((text, original)) = self.untouched_docs.get(key) {
                if row.same(original) {
                    return Ok(text.clone());
                }
            }
            return Ok(row_text(&row)?.into_bytes());
        }
        let mut lines: Vec<(LineKey, Vec<u8>)> = Vec::with_capacity(keys.len());
        for key in keys {
            let Some(row) = model.row(key) else { continue };
            let text = match self.untouched.get(key) {
                Some((text, original)) if row.same(original) => text.clone(),
                _ => row_text(&row)?.into_bytes(),
            };
            lines.push((row_line_key(key), text));
        }
        if let Some(kept) = self.kept.get(path) {
            lines.extend(kept.iter().cloned());
        }
        Ok(jsonl_file(lines))
    }

    /// Every writer-owned file as a full save of `model` would write it from scratch (the oracle of
    /// NFR-REL-16), plus the opaque files' hashes. For tests and `ew fmt --check`.
    ///
    /// # Errors
    ///
    /// [`SaveError::Format`] for a record without a file or a JSON form.
    pub fn render_all(&self, model: &Model) -> Result<BTreeMap<FilePath, Vec<u8>>, SaveError> {
        let mut contents: BTreeMap<FilePath, BTreeSet<AnyKey>> = BTreeMap::new();
        for key in all_keys(model) {
            if let Some(file) = file_of(model, &key)? {
                contents.entry(file).or_default().insert(key);
            }
        }
        for path in self.kept.keys() {
            contents.entry(path.clone()).or_default();
        }
        let mut out = BTreeMap::new();
        for (path, keys) in &contents {
            let bytes = self.render(model, path, keys)?;
            if !bytes.is_empty() {
                out.insert(path.clone(), bytes);
            }
        }
        Ok(out)
    }

    /// The rewrite set of the generation on disk in canonical form (`ew fmt`, WP-S1.22 D9): each file the loader
    /// found not in canonical form or holding a record of another file (`Loaded::rewrite`), with the bytes the next
    /// save writes for it (`None`: the save removes it, nothing is left in it). Files already canonical are not in
    /// it: this is the save plan with no changed record, never a full rewrite. Rendered from the generation on disk
    /// (unsaved edits are not in it); kept lines (quarantined, unknown kinds, untouched malformed values) are in the
    /// bytes verbatim, as in every save. Nothing is written.
    ///
    /// # Errors
    ///
    /// [`SaveError::Format`] for a record without a file or a JSON form.
    pub fn canonical_rewrite(&self) -> Result<BTreeMap<FilePath, Option<Vec<u8>>>, SaveError> {
        let plan = self.plan(&self.base, Changes::Keys(&BTreeSet::new()))?;
        let mut out: BTreeMap<FilePath, Option<Vec<u8>>> = plan
            .writes
            .into_iter()
            .filter(|(path, _)| {
                matches!(
                    classify(path.as_str()),
                    PathClass::Records(_) | PathClass::LibDocument { .. }
                )
            })
            .map(|(path, bytes)| (path, Some(bytes)))
            .collect();
        out.extend(plan.removes.into_iter().map(|path| (path, None)));
        Ok(out)
    }

    // -----------------------------------------------------------------------------------------------------
    // Lock, journal, recovery, close

    /// The heartbeat (doc 05 §7.2): refreshes the lock's modification time and re-reads its session.
    /// Call it every [`HEARTBEAT`](crate::HEARTBEAT).
    ///
    /// # Errors
    ///
    /// An I/O error reading or touching the lock (the lock is then still considered held).
    pub fn heartbeat(&mut self) -> io::Result<LockState> {
        if !self.holds_lock {
            return Ok(LockState::NotHeld);
        }
        match lock::holder(&*self.opts.vfs, &self.root, &self.opts.identity)? {
            Holder::Us => {
                self.opts
                    .vfs
                    .set_modified(&join(&self.root, ew_format::LOCK), self.opts.clock.now())?;
                Ok(LockState::Held)
            }
            Holder::Other(rec) => {
                let _ = self.lose_lock(rec.clone());
                Ok(LockState::Lost(rec))
            }
            Holder::Nobody => {
                let _ = self.lose_lock(None);
                Ok(LockState::Lost(None))
            }
        }
    }

    /// Journals a committed transaction (doc 05 §8.1): the field-level diff of `keys` between `before`
    /// and `after`. A no-op without a journal.
    ///
    /// # Errors
    ///
    /// An I/O error of the journal (the edit itself stands; the caller shows "not crash-protected").
    pub fn journal_commit(
        &mut self,
        before: &Model,
        after: &Model,
        keys: &[AnyKey],
        cmd: &str,
        origin: &str,
    ) -> io::Result<()> {
        let now = self.opts.clock.now();
        let opaque = &self.opaque;
        match &mut self.journal {
            Some(j) => j.tx(now, cmd, origin, before, after, keys, &|r| {
                asset_on_disk(opaque, r)
            }),
            None => Ok(()),
        }
    }

    /// The leftover journal found at open, if any.
    pub fn recovery(&self) -> Option<&RecoveryInfo> {
        self.recovery.as_ref()
    }

    /// Replays the leftover journal onto `model`, the generation just opened (doc 05 §8.2). The result is
    /// journalled in this session's segment, and the replayed leftover is deleted.
    ///
    /// Recovery acts on one journal at a time and never deletes an unreplayed one (project owner (delegated),
    /// 2026-10-31): only the summarised segment ([`Store::recovery`], the newest) is replayed and removed. The
    /// other crashed sessions' segments stay on disk and are offered at the next open, newest first; they leave only
    /// by their own replay or discard or by the age rule (`journal::KEEP_LEFTOVERS`). A segment whose base is not the
    /// generation on disk replays with the collision check (the before-values of its operations against the model);
    /// a collision refuses the replay. There is no merge of segments.
    ///
    /// # Errors
    ///
    /// [`RecoverError`]; with collisions (the base moved and both sides changed a field) nothing is applied and
    /// nothing is removed.
    pub fn recover(&mut self, model: &Model) -> Result<Recovered, RecoverError> {
        let Some(info) = self.recovery.clone() else {
            return Err(RecoverError::Nothing);
        };
        let Some(seg) = self.leftovers.first().cloned() else {
            return Err(RecoverError::Nothing);
        };
        let txs = seg.unsaved(self.generation);
        let (recovered, keys) =
            journal::replay(model, &txs, !info.base_matches).map_err(RecoverError::Replay)?;
        // Assets the journal carried damaged (EW-CORE-015): their bytes are not recovered.
        let findings: Vec<Finding> = txs
            .iter()
            .filter_map(|e| match e {
                journal::Entry::Tx {
                    seq,
                    cmd,
                    damaged_assets,
                    ..
                } if *damaged_assets > 0 => Some(Finding::new(
                    FindingKind::AssetMismatch,
                    None,
                    None,
                    format!(
                        "the journal's transaction {seq} ({cmd}) carried {damaged_assets} asset(s) whose bytes do \
                         not match their reference; they are not recovered"
                    ),
                )),
                _ => None,
            })
            .collect();
        self.notes.extend(findings.iter().cloned());
        let opaque = &self.opaque;
        if let Some(j) = &mut self.journal {
            j.tx(
                self.opts.clock.now(),
                "recover",
                "journal",
                model,
                &recovered,
                &keys,
                &|r| asset_on_disk(opaque, r),
            )
            .map_err(RecoverError::Io)?;
            j.discard(std::slice::from_ref(&seg));
        }
        // `seg` is the first leftover (above); the others stay for the next open.
        self.leftovers.remove(0);
        self.recovery = None;
        Ok(Recovered {
            model: recovered,
            keys: keys.into_iter().collect(),
            transactions: txs.len(),
            findings,
        })
    }

    /// "Discard": deletes the summarised leftover journal ([`Store::recovery`]) and only that one; the other
    /// crashed sessions' segments stay for the next open ([`Store::recover`]). Without a summary (none found, or
    /// already recovered or discarded in this session) nothing is deleted.
    pub fn discard_recovery(&mut self) {
        if self.recovery.take().is_none() || self.leftovers.is_empty() {
            return;
        }
        let seg = self.leftovers.remove(0);
        if let Some(j) = &self.journal {
            j.discard(std::slice::from_ref(&seg));
        }
    }

    /// The access of the session.
    pub fn access(&self) -> &Access {
        &self.access
    }

    /// The generation on disk.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The manifest of the generation on disk: the one opened or the one the last save wrote (`None`: the project has
    /// no usable manifest yet; the next save writes one). The content digest of a sign-off is computed from it
    /// ([`ew_format::content_digest`], doc 15 §5.4).
    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    /// The project directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The I/O seam the store works through ([`StoreOptions::vfs`]); the release writes its archive through it too
    /// (WP-S1.18 D13).
    pub fn vfs(&self) -> &Arc<dyn Vfs> {
        &self.opts.vfs
    }

    /// The per-user state directory ([`StoreOptions::state_dir`]); `None` when the store keeps nothing outside the
    /// project (an in-memory or new project, a test). The release keeps its run records there ([`crate::archive`],
    /// FT-814).
    pub fn state_dir(&self) -> Option<&Path> {
        self.opts.state_dir.as_deref()
    }

    /// The canonical project directory of the generation on disk, read back through the seam and verified (doc 05
    /// §14.1 step 4: the archive's `project/`): the bytes of `manifest.json`, which must be the manifest this store
    /// loaded or wrote, and of every file it lists, each with the manifest's SHA-256.
    ///
    /// # Errors
    ///
    /// [`GenerationError`]: no manifest, a save is incomplete, the manifest changed on disk, a pending hash (a merged
    /// project), a file that is missing, unreadable or not the manifest's.
    pub fn read_generation(&self) -> Result<Generation, GenerationError> {
        let manifest = self.manifest.as_ref().ok_or(GenerationError::NoManifest)?;
        if self.pending.is_some() {
            return Err(GenerationError::Incomplete);
        }
        let vfs = &*self.opts.vfs;
        let read = |rel: &str| {
            let path = join(&self.root, rel);
            vfs.read(&path)
                .map_err(|error| GenerationError::Io { path, error })
        };
        let manifest_bytes = read(MANIFEST)?;
        if Some(sha256(&manifest_bytes)) != self.manifest_hash {
            return Err(GenerationError::Changed {
                path: MANIFEST.to_owned(),
            });
        }
        let mut files = BTreeMap::new();
        for (path, hash) in &manifest.files {
            let FileHash::Sha256(h) = hash else {
                return Err(GenerationError::Pending {
                    path: path.to_string(),
                });
            };
            let bytes = read(path.as_str())?;
            if sha256(&bytes) != *h {
                return Err(GenerationError::Changed {
                    path: path.to_string(),
                });
            }
            files.insert(path.clone(), bytes);
        }
        Ok(Generation {
            generation: self.generation,
            manifest: manifest.clone(),
            manifest_bytes,
            files,
        })
    }

    /// Whether a save is incomplete (passed its commit point, roll-forward pending).
    pub fn is_incomplete(&self) -> bool {
        self.pending.is_some()
    }

    /// The newest intent manifest above the current generation that is on disk (doc 05 §5.3): a save whose temps
    /// the store could not delete after a failure before its commit point. The next editing open rolls it forward
    /// unless it is torn, so a caller that undoes side effects of a failed save (the release's archive and tag, WP-S1.18
    /// D13) keeps them while this is `Some`.
    ///
    /// # Errors
    ///
    /// An I/O error listing the project directory; the answer is then unknown.
    pub fn leftover_intent(&self) -> io::Result<Option<u64>> {
        Ok(newest_intent(&self.walk()?, self.generation))
    }

    /// The journal directory, when a journal is open.
    pub fn journal_dir(&self) -> Option<&Path> {
        self.journal.as_ref().map(Journal::dir)
    }

    /// A clean close (doc 05 §8.1): deletes this session's journal segment and releases the lock. Call it
    /// only when the model is saved or its changes are discarded; dropping the store releases the lock but
    /// keeps the journal for recovery.
    pub fn close(mut self) {
        if let Some(j) = &mut self.journal {
            j.close();
        }
        self.release_lock();
        self.closed = true;
    }

    /// Simulates a crash for tests: forgets the store without releasing the lock or the journal.
    #[doc(hidden)]
    pub fn crash(mut self) {
        self.holds_lock = false;
        self.closed = true;
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if !self.closed {
            self.release_lock();
        }
    }
}

/// The result of [`Store::plan`].
struct SavePlan {
    writes: BTreeMap<FilePath, Vec<u8>>,
    removes: BTreeSet<FilePath>,
    /// The writer-owned files and hashes after the save.
    disk: BTreeMap<FilePath, [u8; 32]>,
    /// The new key sets of the dirty files.
    contents: BTreeMap<FilePath, BTreeSet<AnyKey>>,
    /// The asset files this save writes, with their hashes (they join the opaque files).
    assets: BTreeMap<FilePath, FileHash>,
}

/// The project row of an empty baseline (never written: a save writes the model's project).
fn placeholder_project() -> ew_format::ew_schema::Project {
    serde_json::from_value(serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000000",
        "derivation_profile": "2027.1",
    }))
    .expect("a project with an ID and a profile parses")
}
