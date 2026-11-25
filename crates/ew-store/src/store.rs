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

use ew_format::{
    Access as FormatAccess, FileHash, FilePath, Finding, FindingKind, Kept, Limits, LineKey, MANIFEST,
    Manifest, Migration, MigrationError, Migrations, OpaqueKind, PathClass, classify, crlf_to_lf, hash_text,
    jsonl_file, sha256, unknown_record_file,
};
use ew_model::{AnyKey, AnyRow, Asset, Model, ModelError};
use ew_schema::{AnyId, FORMAT, RELEASED_FORMATS};
use ew_schema::{AssetFormat, AssetRef, Sha256};

use crate::clock::{Clock, SystemClock};
use crate::journal::{self, Diff, Journal, RecoveryInfo, ReplayError, Segment};
use crate::lock::{
    Acquire, Holder, Identity, Lock, LockInfo, LockRecord, OsProbe, PROJECT_LOCK, ProcessProbe, STALE_AFTER,
    TAKEOVER_SETTLE, TakeoverError,
};
use crate::rows::{
    all_keys, document_row, file_of, follow_closure, is_library, record_row, row_line_key, row_text,
};
use crate::vfs::{EntryKind, OsVfs, Vfs, is_transient, join};

mod external;
mod open;
mod purge;
mod save;

pub use external::Incoming;
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
/// The most files [`SaveError::ExternalChange`]'s text names; it counts the others.
const EXTERNAL_CHANGE_NAMED: usize = 10;
/// The most stray names a move-aside tries for one file and generation ([`FilePath::unlisted`]).
const UNLISTED_ATTEMPTS: u32 = 1_000;

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
    /// Whether this is a release build (doc 05 §10.1: `unstable` projects open read-only): [`FORMAT`] is among the
    /// released formats.
    pub release_build: bool,
    /// The migration chain an older format's project runs through at open:
    /// [`ew_format::MIGRATIONS`]; a test passes its own.
    pub migrations: &'static Migrations,
    /// The formats released so far ([`RELEASED_FORMATS`]): a save marks its manifest `unstable` when the list is not
    /// empty and does not hold the format it writes. A test passes its own.
    pub released: &'static [&'static str],
    /// How long a read-only opener waits for a save in progress.
    pub read_only_wait: Duration,
    /// An open for `ew fmt --repair` only: with [`OpenMode::Edit`] the store takes the
    /// writer's lock as for any edit, and the integrity demotion does not demote when the only findings that force a
    /// read-only session are broken lines ([`Finding::is_broken_line`]), which the repair discards
    /// ([`Store::discard_broken_lines`]); a repair open that another finding made read-only still summarises a crashed
    /// session's journal ([`OpenReport::recovery`]), which the repair names first. `false` everywhere else.
    pub repair: bool,
    /// Whether the project folder lies in a cloud-synced folder, as the front-end found it with
    /// [`crate::cloud::detect`]: an open that finds a listed file missing (EW-CORE-003, a half-synced
    /// folder) waits [`crate::external::CLOUD_RECHECK`] and reads the project once more before it reports.
    pub cloud_synced: bool,
}

impl StoreOptions {
    /// The production options: `std::fs`, the system clock, the OS probe, this process with a fresh
    /// session, edit mode and the default state directory.
    pub fn new(app_version: &str) -> StoreOptions {
        StoreOptions {
            vfs: Arc::new(OsVfs),
            clock: Arc::new(SystemClock),
            probe: Arc::new(OsProbe),
            identity: Identity::current(app_version),
            mode: OpenMode::Edit,
            state_dir: default_state_dir(),
            release_build: RELEASED_FORMATS.contains(&FORMAT),
            migrations: &ew_format::MIGRATIONS,
            released: RELEASED_FORMATS,
            read_only_wait: READ_ONLY_WAIT,
            repair: false,
            cloud_synced: false,
        }
    }

    /// The start of a front-end that keeps journals: the state directory and its `journal/` made
    /// user-only, the leftover journal segments of every project older than [`journal::KEEP_LEFTOVERS`] deleted, and
    /// the projects whose leftover journals hold unsaved transactions, newest last change first, without a project
    /// whose lock is live ([`Lock::is_stale`]: another session has it open). Empty without a state directory.
    pub fn start_up_scan(&self) -> Vec<journal::LeftoverProject> {
        let Some(state) = &self.state_dir else {
            return Vec::new();
        };
        let vfs = &*self.vfs;
        vfs.user_only(state, true);
        vfs.user_only(&state.join("journal"), true);
        let now = self.clock.now();
        // A project whose lock is live is open in another session: its journal is neither pruned nor offered.
        let live = |project: &Path| {
            let lock = self.lock(project).read(now);
            matches!(lock, Ok(Some(info)) if !info.stale)
        };
        journal::scan_projects(vfs, state, now, &live)
    }

    /// The project's lock of `root` as the session of these options sees it.
    pub fn lock<'a>(&'a self, root: &'a Path) -> Lock<'a> {
        Lock {
            file: PROJECT_LOCK,
            vfs: &*self.vfs,
            dir: root,
            clock: &*self.clock,
            probe: &*self.probe,
            me: &self.identity,
            stale_after: STALE_AFTER,
        }
    }
}

/// The OS state directory of doc 05 §1: `%LOCALAPPDATA%\ElektraWatt\state` on Windows, `$XDG_STATE_HOME/elektrawatt`
/// (default `~/.local/state/elektrawatt`) on Linux and any other system (macOS is not supported). `None` when
/// the environment names none.
pub fn default_state_dir() -> Option<PathBuf> {
    // The per-user state directory (lock, journal) is outside the project; no project byte depends on it.
    let var = |n: &str| std::env::var_os(n).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        var("LOCALAPPDATA").map(|p| p.join("ElektraWatt").join("state"))
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
    /// The older format the project was migrated from in memory ([`FormatAccess::Migrate`]); its first
    /// save needs [`Store::confirm_upgrade`].
    pub migrated_from: Option<String>,
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
    /// An older major's migration failed: a step's error, or no step where one is needed, with the
    /// file and the JSON Pointer.
    Migration(MigrationError),
    /// A read-only opener met a save in progress for longer than the wait (doc 05 §6.4: exit code 2).
    ///
    /// Not returned by [`Store::open`]: after [`StoreOptions::read_only_wait`] a
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
    /// An `.ewz` opened read-only is refused unread.
    Ewz(crate::ewz::EwzError),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            OpenError::NotADirectory(p) => write!(f, "{} is not a project directory", p.display()),
            OpenError::Format(m) => write!(f, "the project cannot be opened: {m}"),
            OpenError::Migration(e) => write!(f, "the project cannot be migrated from its older format: {e}"),
            OpenError::SaveInProgress => f.write_str("a save is in progress; try again"),
            OpenError::NoProject(_) => f.write_str("the project holds no project record"),
            OpenError::Model { error, .. } => write!(f, "the project does not load: {error}"),
            OpenError::Takeover(e) => write!(f, "takeover failed: {e:?}"),
            OpenError::Exists(p) => write!(f, "{} already holds a project", p.display()),
            OpenError::Ewz(e) => write!(f, "the .ewz is refused: {e}"),
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
    /// Step 2: the lock and manifest pre-checks, and the reads of the files the save replaces or removes.
    PreCheck,
    /// Step 4: writing and syncing the temps.
    Temps,
    /// Step 5: the intent manifest and the directory syncs (the commit point follows).
    Intent,
    /// Step 6: renaming the temps.
    Renames,
    /// Step 7: renaming the intent over `manifest.json`.
    Manifest,
    /// The backup of a project of an older format, written before its first save: nothing of the project was
    /// written.
    Backup,
}

/// Why a save did not complete.
#[derive(Debug)]
pub enum SaveError {
    /// The session is read-only.
    ReadOnly(ReadOnlyReason),
    /// The lock was lost (taken over): the session is now read-only, nothing was written (doc 05 §5.4).
    LockLost(Option<Box<LockRecord>>),
    /// Files changed on disk since we loaded or wrote them: `manifest.json` (an external change, doc 05 §9, to be
    /// merged first), or files this save would replace or remove whose bytes, also in their CRLF-to-LF form, are no
    /// longer those we hold (doc 05 §6.2). Nothing was written.
    ExternalChange {
        /// The changed files, in path order.
        files: Vec<FilePath>,
    },
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
    /// the caller keeps its dirty set and saves again later (review, blocker 1).
    PendingIncomplete {
        /// The decided generation of the earlier save.
        generation: u64,
        /// Why its roll-forward failed.
        error: io::Error,
    },
    /// A record has no file or no JSON form.
    Format(ew_format::FormatError),
    /// The next generation would pass [`ew_format::MAX_GENERATION`], which the manifest's reader refuses: nothing
    /// was written. A copy in a new folder ([`Store::create`]) starts again at generation 0.
    GenerationLimit,
    /// The project was migrated from the older format `from` and its first save converts it, which needs the user's
    /// consent first ([`Store::confirm_upgrade`]): nothing was written.
    UpgradeConsent {
        /// The older format.
        from: String,
    },
    /// The project changed on disk, and the change collides with unsaved edits: the session holds it until a choice of
    /// mine or theirs is made for each colliding field. Nothing was written.
    ExternalCollisions,
    /// The project changed on disk and the change cannot be read: the session keeps its model and its
    /// edits. Nothing was written.
    ExternalUnreadable,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::ReadOnly(r) => write!(f, "read-only session: {r:?}"),
            SaveError::LockLost(_) => f.write_str("the lock was taken over; the session is read-only"),
            SaveError::ExternalChange { files } => {
                f.write_str("the project changed on disk")?;
                if !files.is_empty() {
                    let named: Vec<&str> = files
                        .iter()
                        .take(EXTERNAL_CHANGE_NAMED)
                        .map(FilePath::as_str)
                        .collect();
                    write!(f, " ({}", named.join(", "))?;
                    if files.len() > named.len() {
                        write!(f, " and {} more", files.len() - named.len())?;
                    }
                    f.write_str(")")?;
                }
                f.write_str("; merge the change first")
            }
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
            SaveError::GenerationLimit => f.write_str(
                "this project has been saved the greatest number of times its format allows; save it to a new \
                 folder (Save as\u{2026}), which starts the count again",
            ),
            SaveError::UpgradeConsent { from } => write!(
                f,
                "the project is in the older format {from}, and saving converts it to format {FORMAT}, which older \
                 versions of ElektraWatt cannot open; upgrade it first with `ew upgrade`, which keeps a backup"
            ),
            SaveError::ExternalCollisions => f.write_str(
                "the project changed on disk, and the change collides with your edits; choose yours or theirs for each \
                 field first",
            ),
            SaveError::ExternalUnreadable => f.write_str(
                "The project on disk changed and cannot be read; your edits are kept. Save a copy, or wait until the \
                 disk is complete.",
            ),
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
    /// Whether the `manifest.json` this save wrote is marked `unstable` (an unreleased format): the front
    /// ends warn on every such save. False when no manifest was written.
    pub unstable: bool,
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
    /// is touched (EW-CORE-002).
    BaseMoved(BaseMismatch),
}

/// The intent member that records the SHA-256 of the raw bytes of the `manifest.json` a save was based on
/// (`"base": "sha256:<hex>"`; absent when there was no manifest). The intent becomes `manifest.json`, so
/// every manifest written by a save carries the hash of the one it replaced. The roll-forward compares raw
/// bytes, never parsed generations: a conflicted or unparsable manifest whose bytes are unchanged rolls
/// forward.
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
/// delete.
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
    let (sniffed, size) = ew_schema::sniff(bytes).ok()?;
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
/// is not its base.
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
    disk: BTreeMap<FilePath, Held>,
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
    /// while there are any (doc 05 §13.3).
    quarantined: BTreeSet<FilePath>,
    /// The broken lines among the kept ones (EW-CORE-005 at line level), by file: what `ew fmt --repair` discards.
    /// In memory only; their bytes stay in `kept` until a repair's save adopts.
    broken: BTreeMap<FilePath, Broken>,
    journal: Option<Journal>,
    leftovers: Vec<Segment>,
    recovery: Option<RecoveryInfo>,
    closed: bool,
    /// The bytes of `manifest.json` as read at open (`None`: absent, unreadable or an intent's): the backup of an
    /// upgrade packs them ([`crate::ewz::pack_as_found`]).
    manifest_bytes: Option<Vec<u8>>,
    /// A project of an older format migrated in memory, until its first save passes its commit point.
    migrated: Option<Migrated>,
    /// The changed `manifest.json` a poll saw first, by the hash of its bytes (`None`: absent), and when
    /// ([`Store::poll`]).
    watch: Option<(Option<[u8; 32]>, std::time::SystemTime)>,
}

/// `manifest.json` as read: the manifest, the SHA-256 of its bytes and the bytes.
type FoundManifest = (Manifest, [u8; 32], Vec<u8>);

/// A store migrated from an older format ([`OpenReport::migrated_from`]).
#[derive(Clone, Debug)]
struct Migrated {
    /// The older format.
    from: String,
    /// The manifest as found, through the manifest steps of the chain: the save writes its `extra`.
    manifest: Manifest,
    /// Whether the user consented to the conversion ([`Store::confirm_upgrade`]).
    confirmed: bool,
}

/// A writer-owned file as the store holds it: the SHA-256 of its bytes as loaded or written, and the SHA-256 of their
/// CRLF-to-LF form when it differs.
///
/// `hash` is the raw hash: the manifest lists it and the plan compares the bytes it renders with it. `lf` serves only
/// the save's pre-check, which compares a record file or library document by its text, so that
/// a checkout that changed only its line endings is not a change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Held {
    hash: [u8; 32],
    lf: Option<[u8; 32]>,
}

impl Held {
    /// The held form of `bytes`.
    fn of(bytes: &[u8]) -> Held {
        let hash = sha256(bytes);
        Held {
            hash,
            lf: lf_hash(bytes, &hash),
        }
    }

    /// The hashes the pre-check accepts for this file: the raw one, and the LF form's when it differs.
    fn accepted(&self) -> Vec<[u8; 32]> {
        std::iter::once(self.hash).chain(self.lf).collect()
    }
}

/// The SHA-256 of the CRLF-to-LF form of `bytes` ([`crlf_to_lf`]) when it differs from `raw`, the hash of `bytes`.
fn lf_hash(bytes: &[u8], raw: &[u8; 32]) -> Option<[u8; 32]> {
    if !bytes.windows(2).any(|w| w == b"\r\n") {
        return None;
    }
    let lf = sha256(&crlf_to_lf(bytes));
    (lf != *raw).then_some(lf)
}

/// The broken lines of one file (EW-CORE-005 at line level): each line's key in `kept` with its
/// line number in the generation loaded, and whether the file holds another quarantined line, which keeps it among the
/// quarantined files after a repair.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Broken {
    lines: BTreeMap<LineKey, u32>,
    other_quarantined: bool,
}

/// A broken line of the generation on disk ([`Store::broken_lines`]): EW-CORE-005 at line level, a line that is not one
/// JSON value or not UTF-8, which `ew fmt --repair` discards.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BrokenLine {
    /// The file.
    pub file: FilePath,
    /// The 1-based line in the generation loaded.
    pub line: u32,
    /// Its bytes, without the line end, as the file holds them.
    pub text: Vec<u8>,
}

/// What an open has found so far, step by step (doc 05 §6.1).
struct Opening {
    findings: Vec<Finding>,
    removed: Vec<String>,
    rolled_forward: Option<u64>,
    repairable: bool,
    /// The project directory's entries, as last listed.
    listing: BTreeMap<String, EntryKind>,
    /// A read-only open's intent still there after its wait: a save in progress, read in memory.
    abandoned: Option<u64>,
}

/// The manifest an open read and what its format allows ([`Store::read_stamps`]).
struct Stamps {
    format: FormatAccess,
    /// The files the interrupted save removes, which are not orphans.
    pending_removal: BTreeSet<FilePath>,
    /// The intent of the interrupted save that a read-only open reads in memory.
    intent: Option<Manifest>,
    manifest: Option<Manifest>,
    /// The way from an older major's format to this one.
    migration: Option<Migration<'static>>,
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
        match self.opts.lock(&self.root).holder()? {
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
            Some(j) => j.tx(
                now,
                cmd,
                origin,
                &Diff {
                    before,
                    after,
                    keys,
                    on_disk: &|r| asset_on_disk(opaque, r),
                },
            ),
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
    /// Recovery acts on one journal at a time and never deletes an unreplayed one: only the summarised
    /// segment ([`Store::recovery`], the newest) is replayed and removed. The other crashed sessions'
    /// segments stay on disk and are offered at the next open, newest first; they leave only by their own
    /// replay or discard or by the age rule (`journal::KEEP_LEFTOVERS`). A segment whose base is not the
    /// generation on disk replays with the collision check (the before-values of its operations against the
    /// model); a collision refuses the replay. There is no merge of segments.
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
                &Diff {
                    before: model,
                    after: &recovered,
                    keys: &keys,
                    on_disk: &|r| asset_on_disk(opaque, r),
                },
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

    /// The broken lines of the generation loaded (EW-CORE-005 at line level: not one JSON value, or not UTF-8), in file
    /// and line order, from the store's own data, never the open's findings, whose list is capped: what
    /// [`Store::discard_broken_lines`] discards.
    pub fn broken_lines(&self) -> Vec<BrokenLine> {
        let mut out = Vec::new();
        for (file, broken) in &self.broken {
            let mut lines: Vec<BrokenLine> = self
                .kept
                .get(file)
                .into_iter()
                .flatten()
                .filter_map(|(key, text)| {
                    broken.lines.get(key).map(|line| BrokenLine {
                        file: file.clone(),
                        line: *line,
                        text: text.clone(),
                    })
                })
                .collect();
            lines.sort_by_key(|l| l.line);
            out.append(&mut lines);
        }
        out
    }

    /// `ew fmt --repair`'s save: one save of the generation loaded through the
    /// protocol, with no changed key, in which the files that hold broken lines ([`Store::broken_lines`]) are rewritten
    /// without them, in canonical form as `ew fmt` writes them, every other kept line byte for byte; generation g + 1
    /// (nothing is written without a broken line or a file to rewrite). The broken lines leave the store's kept lines
    /// only at the save's commit point (in `adopt`): a save that fails before it keeps all of them, and the next
    /// attempt discards the same lines. Unsaved edits of a caller's model are not this save's: it writes the baseline.
    ///
    /// # Errors
    ///
    /// As [`Store::save`].
    pub fn discard_broken_lines(&mut self) -> Result<SaveReport, SaveError> {
        let base = self.base.clone();
        self.save_inner(&base, Changes::Keys(&BTreeSet::new()), &BTreeSet::new(), true)
    }

    /// Update from library of the damaged library copies `keys`: one save of `model`,
    /// which holds the library's documents of those copies, that writes each over its damaged file, which the open kept
    /// as it was found, and lists it in the manifest, as a save writes an updated copy; nothing else is written. A store
    /// that the integrity findings made read-only takes the lock for the save and gives it back after, staying
    /// read-only: the caller opens the project again, writable when the integrity check passes. Refused before anything
    /// is written: a store read-only for another reason (another session holding the lock among them), another file
    /// that does not hold the bytes the manifest lists (other integrity findings, which this save would hide; another
    /// damaged copy, kept as found or missing or over the limit, keeps its manifest entry unchanged and stays damaged),
    /// and a damaged file that changed since the open.
    ///
    /// # Errors
    ///
    /// [`SaveError::ReadOnly`] with the reason: [`ReadOnlyReason::LockedBy`] when another session holds the lock,
    /// [`ReadOnlyReason::Integrity`] for another file's finding; otherwise as [`Store::save`].
    pub fn update_damaged_copies(
        &mut self,
        model: &Model,
        keys: &BTreeSet<AnyKey>,
    ) -> Result<SaveReport, SaveError> {
        let read_only = match &self.access {
            Access::Edit => false,
            Access::ReadOnly(ReadOnlyReason::Integrity) => true,
            Access::ReadOnly(reason) => return Err(SaveError::ReadOnly(reason.clone())),
        };
        let paths: BTreeSet<FilePath> = keys
            .iter()
            .filter_map(|&(kind, id)| Some(FilePath::lib(ew_format::lib_dir(kind)?, id)))
            .collect();
        // A damaged copy left as it is (one the open kept as found, or whose file it could not read: missing or over the
        // limit) keeps its manifest entry unchanged, and stays damaged with its findings.
        let left: BTreeMap<FilePath, FileHash> = self
            .manifest
            .as_ref()
            .map(|m| {
                m.files
                    .iter()
                    .filter(|(path, _)| {
                        !paths.contains(*path)
                            && matches!(classify(path.as_str()), PathClass::LibDocument { .. })
                            && (self.opaque.contains_key(*path) || !self.disk.contains_key(*path))
                    })
                    .map(|(path, listed)| (path.clone(), *listed))
                    .collect()
            })
            .unwrap_or_default();
        let as_listed = self.manifest.as_ref().is_some_and(|m| {
            m.files
                .iter()
                .filter(|(path, _)| !paths.contains(*path) && !left.contains_key(*path))
                .all(
                    |(path, listed)| match (self.disk.get(path), self.opaque.get(path), listed) {
                        (Some(held), _, FileHash::Sha256(hash)) => held.accepted().contains(hash),
                        (None, Some(held), _) => held == listed,
                        _ => false,
                    },
                )
        });
        if !as_listed {
            return Err(SaveError::ReadOnly(ReadOnlyReason::Integrity));
        }
        if read_only {
            let acquired = self
                .opts
                .lock(&self.root)
                .acquire()
                .map_err(|error| SaveError::Failed {
                    step: SaveStep::PreCheck,
                    path: join(&self.root, ew_format::LOCK),
                    error,
                })?;
            match acquired {
                Acquire::Held => {}
                Acquire::HeldBy(info) => {
                    return Err(SaveError::ReadOnly(ReadOnlyReason::LockedBy(Box::new(info))));
                }
                Acquire::NotWritable(why) => {
                    return Err(SaveError::ReadOnly(ReadOnlyReason::NotWritable(why)));
                }
            }
            self.holds_lock = true;
            self.access = Access::Edit;
        }
        // The damaged files as the open found them are what this save replaces, and only the copies are written.
        let kept = (
            self.opaque.clone(),
            self.disk.clone(),
            std::mem::take(&mut self.rewrite),
        );
        for path in &paths {
            if let Some(FileHash::Sha256(hash)) = self.opaque.remove(path) {
                self.disk.insert(path.clone(), Held { hash, lf: None });
            }
        }
        // The new manifest lists the copies left damaged with their entries as they were.
        self.opaque.extend(left);
        let saved = self.save(model, Changes::Keys(keys));
        if saved
            .as_ref()
            .is_err_and(|e| !matches!(e, SaveError::Incomplete { .. }))
        {
            (self.opaque, self.disk, self.rewrite) = kept;
        }
        if read_only {
            self.access = Access::ReadOnly(ReadOnlyReason::Integrity);
            self.release_lock();
        }
        saved
    }

    /// The access of the session.
    pub fn access(&self) -> &Access {
        &self.access
    }

    /// The generation on disk.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether this store's saves mark the `manifest.json` they write `unstable` (an unreleased format):
    /// what [`SaveReport::unstable`] says of a save that writes one, and so of the manifest [`Store::create`] wrote. The
    /// front ends warn on every such write.
    pub fn writes_unstable(&self) -> bool {
        unstable_flag(
            self.manifest.as_ref().filter(|_| self.keep_format),
            self.opts.released,
        )
    }

    /// The manifest of the generation on disk: the one opened or the one the last save wrote (`None`: the project has
    /// no usable manifest yet; the next save writes one). The content digest of a sign-off is computed from it
    /// ([`ew_format::content_digest`], doc 15 §5.4).
    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    /// The SHA-256 of the bytes of the project file `path` as this store loaded or last wrote them: a writer-owned
    /// file's raw hash (`Held::hash`) or an opaque file's; `None` for a file it holds no bytes of (not read at open).
    /// The repair's backup compares the bytes on disk with it ([`crate::ewz::pack_as_found`]).
    pub(crate) fn loaded_hash(&self, path: &FilePath) -> Option<[u8; 32]> {
        match (self.disk.get(path), self.opaque.get(path)) {
            (Some(held), _) => Some(held.hash),
            (None, Some(FileHash::Sha256(hash))) => Some(*hash),
            (None, Some(FileHash::Pending) | None) => None,
        }
    }

    /// The bytes of `manifest.json` as read at open or as the last save wrote them (`None`: none read).
    pub(crate) fn manifest_bytes(&self) -> Option<&[u8]> {
        self.manifest_bytes.as_deref()
    }

    /// The older format this project was migrated from in memory, until its first save passes its commit point:
    /// that save needs [`Store::confirm_upgrade`] first.
    pub fn migrated_from(&self) -> Option<&str> {
        self.migrated.as_ref().map(|m| m.from.as_str())
    }

    /// The user's consent to the conversion of a migrated project: its next save no longer refuses with
    /// [`SaveError::UpgradeConsent`]. The caller writes the backup first. Nothing for a project that was not migrated.
    pub fn confirm_upgrade(&mut self) {
        if let Some(migrated) = &mut self.migrated {
            migrated.confirmed = true;
        }
    }

    /// The project directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The I/O seam the store works through ([`StoreOptions::vfs`]); the release writes its archive through it too.
    pub fn vfs(&self) -> &Arc<dyn Vfs> {
        &self.opts.vfs
    }

    /// The per-user state directory ([`StoreOptions::state_dir`]); `None` when the store keeps nothing outside the
    /// project (an in-memory or new project, a test). The release keeps its run records there
    /// ([`crate::archive`]).
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
    /// unless it is torn, so a caller that undoes side effects of a failed save (the release's archive and
    /// tag) keeps them while this is `Some`.
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
    /// The writer-owned files and their hashes after the save.
    disk: BTreeMap<FilePath, Held>,
    /// The new key sets of the dirty files.
    contents: BTreeMap<FilePath, BTreeSet<AnyKey>>,
    /// The asset files this save writes, with their hashes (they join the opaque files).
    assets: BTreeMap<FilePath, FileHash>,
}

/// Whether the file at `full`, held with the SHA-256 values `held`, can be replaced or removed without losing a change
/// made outside the session (doc 05 §6.2): it is absent (nothing to overwrite), or a regular file whose
/// bytes, or their CRLF-to-LF form ([`crlf_to_lf`]), hash to one of `held`. A directory, a link or anything else in its
/// place is a change. Read in chunks of [`purge::SCAN_CHUNK`], so that a file grown outside the session is never held
/// whole.
fn unchanged_on_disk(vfs: &dyn Vfs, full: &Path, held: &[[u8; 32]]) -> io::Result<bool> {
    use sha2::Digest as _;
    let stat = match vfs.stat(full) {
        Ok(stat) => stat,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(e),
    };
    if stat.kind != EntryKind::File {
        return Ok(false);
    }
    let (mut raw, mut lf) = (sha2::Sha256::new(), sha2::Sha256::new());
    // A CR that ends a full chunk waits for the next chunk's first byte, so that no CRLF pair is split.
    let mut cr = false;
    let mut offset = 0u64;
    loop {
        let chunk = vfs.read_at(full, offset, purge::SCAN_CHUNK)?;
        let more = chunk.len() == purge::SCAN_CHUNK;
        offset += chunk.len() as u64;
        raw.update(&chunk);
        let mut part = Vec::with_capacity(chunk.len() + 1);
        if cr {
            part.push(b'\r');
        }
        part.extend_from_slice(&chunk);
        cr = more && part.last() == Some(&b'\r');
        if cr {
            part.pop();
        }
        lf.update(crlf_to_lf(&part));
        if !more {
            break;
        }
    }
    let (raw, lf): ([u8; 32], [u8; 32]) = (raw.finalize().into(), lf.finalize().into());
    Ok(held.contains(&raw) || held.contains(&lf))
}

/// Whether the regular files `a` and `b` hold the same bytes: their lengths first, then their content in chunks of
/// [`purge::SCAN_CHUNK`], so that neither is ever held whole.
fn same_bytes(vfs: &dyn Vfs, a: &Path, b: &Path) -> io::Result<bool> {
    let (sa, sb) = (vfs.stat(a)?, vfs.stat(b)?);
    if sa.kind != EntryKind::File || sb.kind != EntryKind::File || sa.len != sb.len {
        return Ok(false);
    }
    let mut offset = 0u64;
    loop {
        let ca = vfs.read_at(a, offset, purge::SCAN_CHUNK)?;
        if ca != vfs.read_at(b, offset, purge::SCAN_CHUNK)? {
            return Ok(false);
        }
        if ca.len() < purge::SCAN_CHUNK {
            return Ok(true);
        }
        offset += ca.len() as u64;
    }
}

/// The `unstable` flag a save writes: a save that
/// keeps the file's own format (`kept`, a newer minor) keeps the file's own flag, since `released` cannot know a newer
/// minor; a save that writes [`FORMAT`] marks it when `released` is not empty and does not hold it.
fn unstable_flag(kept: Option<&Manifest>, released: &[&str]) -> bool {
    match kept {
        Some(old) => old.unstable,
        None => !released.is_empty() && !released.contains(&FORMAT),
    }
}

/// The project row of an empty baseline (never written: a save writes the model's project).
fn placeholder_project() -> ew_schema::Project {
    serde_json::from_value(serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000000",
        "derivation_profile": "2027.1",
    }))
    .expect("a project with an ID and a profile parses")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::{DirSync, MemVfs};

    /// `bytes` in a file of a fresh in-memory file system, and its path.
    fn put(bytes: &[u8]) -> (MemVfs, PathBuf) {
        let vfs = MemVfs::new(DirSync::Explicit);
        let path = PathBuf::from("/p/pages/x.jsonl");
        vfs.put(&path, bytes);
        (vfs, path)
    }

    #[test]
    fn a_crlf_pair_across_a_chunk_boundary_is_one_line_end() {
        // D5's chunked comparison equals `crlf_to_lf` of the whole file: a CR that ends a full chunk is held for the
        // next chunk's LF, and a CR that ends the file stays.
        let chunk = purge::SCAN_CHUNK;
        let mut straddling = vec![b'a'; chunk - 1];
        straddling.extend_from_slice(b"\r\nb\r\n\r");
        let mut lone = vec![b'a'; chunk - 1];
        lone.extend_from_slice(b"\rx");
        let mut at_end = vec![b'a'; chunk - 1];
        at_end.push(b'\r');
        for bytes in [straddling, lone, at_end, b"a\r\nb".to_vec(), Vec::new()] {
            let (vfs, path) = put(&bytes);
            let lf = crlf_to_lf(&bytes);
            assert_eq!(unchanged_on_disk(&vfs, &path, &[sha256(&bytes)]).ok(), Some(true));
            assert_eq!(unchanged_on_disk(&vfs, &path, &[sha256(&lf)]).ok(), Some(true));
            let mut other = lf.clone();
            other.push(b'\n');
            assert_eq!(
                unchanged_on_disk(&vfs, &path, &[sha256(&other)]).ok(),
                Some(false)
            );
        }
    }

    #[test]
    fn a_held_crlf_file_accepts_its_raw_and_its_lf_hash() {
        // A record file held as CRLF bytes keeps the LF form's hash beside the raw one; LF bytes, or a lone
        // CR that the conversion keeps, have one hash only.
        let crlf = Held::of(b"a\r\nb\r\n");
        assert_eq!(crlf.hash, sha256(b"a\r\nb\r\n"));
        assert_eq!(crlf.lf, Some(sha256(b"a\nb\n")));
        assert_eq!(crlf.accepted(), [sha256(b"a\r\nb\r\n"), sha256(b"a\nb\n")]);
        for bytes in [&b"a\nb\n"[..], b"a\rb\n", b""] {
            let held = Held::of(bytes);
            assert_eq!((held.hash, held.lf), (sha256(bytes), None));
            assert_eq!(held.accepted(), [sha256(bytes)]);
        }
        // The disk's raw bytes and their LF form, each against either held hash.
        let (vfs, full) = put(b"a\nb\n");
        assert_eq!(unchanged_on_disk(&vfs, &full, &crlf.accepted()).ok(), Some(true));
        let (vfs, full) = put(b"a\r\nb\r\n");
        assert_eq!(unchanged_on_disk(&vfs, &full, &crlf.accepted()).ok(), Some(true));
        let (vfs, full) = put(b"a\nc\n");
        assert_eq!(unchanged_on_disk(&vfs, &full, &crlf.accepted()).ok(), Some(false));
    }

    #[test]
    fn an_absent_file_is_unchanged_and_a_directory_is_not() {
        let (vfs, path) = put(b"x\n");
        assert_eq!(
            unchanged_on_disk(&vfs, Path::new("/p/pages/y.jsonl"), &[sha256(b"x\n")]).ok(),
            Some(true)
        );
        assert_eq!(
            unchanged_on_disk(&vfs, Path::new("/p/pages"), &[sha256(b"x\n")]).ok(),
            Some(false)
        );
        assert_eq!(unchanged_on_disk(&vfs, &path, &[sha256(b"x\n")]).ok(), Some(true));
    }

    #[test]
    fn same_bytes_compares_lengths_then_every_chunk() {
        let chunk = purge::SCAN_CHUNK;
        let vfs = MemVfs::new(DirSync::Explicit);
        let mut a = vec![7u8; chunk + 3];
        let (pa, pb, pc, pd) = (
            Path::new("/p/a"),
            Path::new("/p/b"),
            Path::new("/p/c"),
            Path::new("/p/d"),
        );
        vfs.put(pa, &a);
        vfs.put(pb, &a);
        a[chunk + 1] = 8;
        vfs.put(pc, &a);
        vfs.put(pd, &a[..chunk]);
        assert_eq!(same_bytes(&vfs, pa, pb).ok(), Some(true));
        assert_eq!(
            same_bytes(&vfs, pa, pc).ok(),
            Some(false),
            "the second chunk differs"
        );
        assert_eq!(same_bytes(&vfs, pa, pd).ok(), Some(false), "the lengths differ");
        assert!(same_bytes(&vfs, pa, Path::new("/p/e")).is_err(), "an absent file");
    }

    /// A save that keeps a newer minor's stamp keeps its `unstable` flag, whatever this build's
    /// released list says; a save that writes [`FORMAT`] marks it by the list. (A store of [`FORMAT`] `0.x` cannot
    /// open a newer minor, every `0.y` being its own major, so the file is the manifest the save keeps.)
    #[test]
    fn a_kept_newer_minor_keeps_its_own_unstable_flag() {
        let released: &[&str] = &[FORMAT];
        let mut newer = Manifest::new(
            "1.3",
            "newer",
            "8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a".parse().expect("a UUID"),
            4,
        );
        assert!(!unstable_flag(Some(&newer), released), "a released newer minor");
        newer.unstable = true;
        assert!(unstable_flag(Some(&newer), released));
        assert!(unstable_flag(Some(&newer), &[]));
        assert!(!unstable_flag(None, released));
        assert!(unstable_flag(None, &["0.0"]));
        assert!(!unstable_flag(None, &[]));
    }
}
