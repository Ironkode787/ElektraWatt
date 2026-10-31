// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The atomic release and the as-built archive (doc 05 §14.1, §14.2; doc 15 §5.5; WP-S1.18 part 4, D11 to D13):
//! [`Session::release`] runs the six steps, [`Session::incomplete_release`] finds what a crash left, and
//! [`Session::discard_incomplete_release`] and [`Session::complete_release`] settle it.
//!
//! **The six steps** (doc 05 §14.1), in order:
//!
//! 1. **Gate** on the saved generation ([`Session::release_gate`]): every blocker refuses. The archive root, an existing
//!    archive of the label (an incomplete release), the git worktree, the executable, the consent and an existing tag
//!    are judged here too, so that every refusal comes before anything is written.
//! 2. **Freeze**: the freeze transactions (none at MVP: no numbering scheme freezes at release yet, D13) through the
//!    ordinary path, then the save of generation g. Every page digest must be equal before and after the freeze, or
//!    the freeze is undone and the release aborts.
//! 3. **Git commit** of the project directory (`Release <label>`) when it lies in a worktree.
//! 4. **Archive** from generation g read back from disk and the release context: built ([`ew_store::archive::build`]),
//!    written as a temp, synced, read back, verified (the sums, and `project/` against manifest g) and renamed into
//!    place ([`ew_store::archive::write_atomically`]).
//! 5. **Tag** `ew/rev/<label>` on the commit, its message carrying the archive's SHA-256.
//! 6. **Stamp**: `revision.release` with the release record, prepared on a fork, saved as generation g+1, then
//!    committed (a barrier in the history; the session stays saved).
//!
//! **Atomicity** (D13, NFR-REL-15). A failure in steps 4 to 6 leaves the revision open and removes the temp and the
//! archive and tag this run created; the freeze stays as an ordinary undoable transaction. The stamp is saved before
//! the session commits it, so a failed save leaves the model as it was. A crash leaves at most one complete archive
//! that no release record names; [`Session::incomplete_release`] reports it at the next open.
//!
//! **Evidence** (FT-814). Two copies of one project directory (an operating-system copy, a restore, a Save as) carry
//! the same project ID, and at the same generation write the same archive bytes, so nothing in an archive or in the
//! project files tells whose run wrote it. Before its archive has its name, the run records it durably in the per-user
//! state directory ([`ew_store::archive::RunRecord`]: the project directory, the archive, its SHA-256 and modification
//! time, and the run's mark, which the tag's message carries as `Run: <mark>`), and removes the record when the run is
//! settled. A record left stale (its archive gone, or its revision released with the archive's SHA-256 by a later
//! roll-forward or completion) is removed when the project opens for editing and when
//! [`Session::incomplete_release`] looks; a removal that fails is reported ([`Session::take_release_troubles`]).
//! [`Session::discard_incomplete_release`] removes an archive only with a matching record of this project
//! directory, and the tag only when its message also carries the record's mark; without the record (a copy, a wiped
//! state directory, a session without one) everything is kept ([`ReleaseError::Unproven`]). The archive temps are named
//! by the key of the project directory, so a copy's live temp is never removed either.
//!
//! **Reachability.** Only [`Session::release`] and [`Session::complete_release`] submit `revision.release`, from a human
//! origin (the GUI, a confirmed CLI subcommand); [`Session::execute`] and [`Session::preview`] refuse it, and a command
//! log, a script, MCP and a plugin get the exposure error.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use ew_commands::Command as _;
use ew_commands::commands::{ApprovedPack, ReleaseRevision, ReleasedArchive};
use ew_derive::ew_ids::Id;
use ew_mltext::Ident;
use ew_model::AnyKey;
use ew_schema::{Date, Digest128, Page, ReleaseRecord, Revision};
use ew_store::archive::{self, ArchiveError, WriteError};
use ew_store::git::{self, Committed, Git, GitError};
use ew_store::zip::ZipLimits;
use ew_store::{Changes, GenerationError, SaveError, Vfs};

use crate::output::{
    ExportOptions, OutputError, Parallelism, PdfDate, changed_pages, page_digests, page_uuid,
};
use crate::release::{FOUR_EYES_RULE, Gate, GateError};
use crate::{AnyCommand, CmdError, OpenError, OpenReport, Origin, Session, Sha256, Submit, checks};

/// The release context (doc 05 §14.1 step 4): the label of the open revision and the release date. The archive is
/// built from generation g plus this context only, so the same saved state and context give the same bytes (RA-5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReleaseContext {
    /// The open revision's label, typed to confirm.
    pub label: String,
    /// The release date (the front-end reads the clock): `released_on`, the PDF's date.
    pub date: Date,
}

/// How a release reaches outside the project directory.
#[derive(Clone, Copy)]
pub struct ReleaseOptions<'a> {
    /// The archive root; `None`: the default root beside the project directory (`<parent>/<name>.ew-archive`).
    pub archive_root: Option<&'a Path>,
    /// The git port ([`ew_store::git::GitExe`] in production).
    pub git: &'a dyn Git,
    /// Whether the person agreed to the git commit and tag (the typed confirmation named them). A release inside a
    /// worktree without it is refused.
    pub git_consent: bool,
    /// The threads of the PDF and the page digests; never changes a byte.
    pub parallelism: Parallelism,
}

impl fmt::Debug for ReleaseOptions<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReleaseOptions")
            .field("archive_root", &self.archive_root)
            .field("git_consent", &self.git_consent)
            .field("parallelism", &self.parallelism)
            .finish_non_exhaustive()
    }
}

/// What a release will do, judged before anything is written ([`Session::release_plan`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReleasePlan {
    /// The gate on the saved generation (it passes).
    pub gate: Gate,
    /// The archive file the release writes.
    pub archive: PathBuf,
    /// The git worktree the project lies in, with the tag the release creates there.
    pub git: Option<(PathBuf, String)>,
    /// Whether the archive root lies inside that worktree (doc 05 §14.2: EW warns; it never adds an archive to git).
    pub archive_in_worktree: bool,
}

/// A completed release.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Released {
    /// The released revision.
    pub revision: Id<Revision>,
    /// Its label.
    pub label: String,
    /// The released generation g (the archive's).
    pub generation: u64,
    /// The generation that holds the stamp, g+1.
    pub stamped: u64,
    /// The archive.
    pub archive: PathBuf,
    /// The SHA-256 of the archive.
    pub sha256: Sha256,
    /// The git commit step's outcome and the tag, when the project lies in a worktree.
    pub git: Option<(Committed, String)>,
    /// Whether the stamp's save passed its commit point but did not complete: generation g+1 is decided and completes
    /// by roll-forward ([`Session::retry_save`], or the next open).
    pub save_incomplete: bool,
}

/// An archive that a crashed release left: complete, in place, and named by no release record (doc 05 §14.1 "On
/// failure"): the open revision's label is still open ([`Session::incomplete_release`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncompleteRelease {
    /// The open revision.
    pub revision: Id<Revision>,
    /// Its label.
    pub label: String,
    /// The archive.
    pub archive: PathBuf,
    /// The SHA-256 of its bytes.
    pub sha256: Sha256,
    /// What its `ARCHIVE.json` says, when it reads: the release context's date, the generation and the record's facts.
    pub facts: Option<(Date, u64, ReleaseRecord)>,
    /// Why completing it is refused now, if it is (a damaged archive, another project's or revision's, a project that
    /// changed since it was written): then the person discards it, unless [`IncompleteRelease::foreign`] or
    /// [`IncompleteRelease::unproven`] says it is kept.
    pub refusal: Option<String>,
    /// Why the archive is not this project's incomplete release, if it is not: its `ARCHIVE.json` names another project
    /// or another revision, or does not read (a root may be shared, and the file name carries only the label). Then it
    /// is neither completed nor discarded: it is kept ([`ReleaseError::Foreign`]).
    pub foreign: Option<String>,
    /// Why a discard keeps the archive although it names this project and the open revision, if it does (FT-814): no
    /// run record of this project directory in the state directory matches it, so it is not provably this directory's
    /// own leftover (a copy of the directory names the same project). Then it is not discarded
    /// ([`ReleaseError::Unproven`]); completing it is unaffected. `None` when the archive is foreign.
    pub unproven: Option<String>,
}

/// What [`Session::discard_incomplete_release`] removed, and the tag it kept.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Discarded {
    /// The archive removed.
    pub archive: PathBuf,
    /// The tag `ew/rev/<label>` removed: its message carried the archive's SHA-256 and the crashed run's mark (this
    /// project directory's run created it).
    pub tag_removed: Option<String>,
    /// The tag `ew/rev/<label>` kept: it exists, but its message does not carry both the archive's SHA-256 and the
    /// crashed run's mark (another release's, another copy's in the same repository, or someone else's).
    pub tag_kept: Option<String>,
}

/// An as-built archive opened for reproduction (D14, FR-REV-010; [`Session::open_archive`]): `project/` as a read-only
/// session in memory, with the release context and the archive's other entries.
pub struct OpenedArchive {
    /// The archived generation as a read-only session (no lock, no journal, no state directory).
    pub session: Session,
    /// What opening `project/` found.
    pub report: OpenReport,
    /// The release context of `ARCHIVE.json`: the label and the release date.
    pub context: ReleaseContext,
    /// The archived generation g.
    pub generation: u64,
    /// The released revision `ARCHIVE.json` names.
    pub revision: Id<Revision>,
    /// Every entry of the archive by path.
    entries: BTreeMap<String, Vec<u8>>,
}

impl fmt::Debug for OpenedArchive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenedArchive")
            .field("context", &self.context)
            .field("generation", &self.generation)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl OpenedArchive {
    /// The archive's entry at `path` (`pdf/RM-001_V1.0.pdf`), if it has one.
    pub fn entry(&self, path: &str) -> Option<&[u8]> {
        self.entries.get(path).map(Vec::as_slice)
    }

    /// The archived PDF (the one `pdf/` entry) with its path.
    pub fn archived_pdf(&self) -> Option<(&str, &[u8])> {
        self.entries
            .iter()
            .find(|(p, _)| p.starts_with("pdf/"))
            .map(|(p, d)| (p.as_str(), d.as_slice()))
    }

    /// The archived report rows (`reports/*.csv`) by path.
    pub fn archived_reports(&self) -> BTreeMap<&str, &[u8]> {
        self.entries
            .iter()
            .filter(|(p, _)| p.starts_with("reports/"))
            .map(|(p, d)| (p.as_str(), d.as_slice()))
            .collect()
    }

    /// The PDF re-exported from `project/` with the release context (RU-1: equal to [`OpenedArchive::archived_pdf`]),
    /// with its path in the archive's layout.
    ///
    /// # Errors
    ///
    /// [`OutputError`] when the PDF cannot be written.
    pub fn export_pdf(&self, parallelism: Parallelism) -> Result<(String, Vec<u8>), OutputError> {
        self.session.archived_pdf(&self.context, parallelism)
    }

    /// The report rows re-derived from `project/` (RU-2: equal to [`OpenedArchive::archived_reports`]), by path.
    pub fn derived_reports(&self) -> BTreeMap<String, Vec<u8>> {
        self.session.archived_reports()
    }
}

/// Why an archive could not be opened for reproduction.
#[derive(Debug)]
pub enum ArchiveOpenError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file is larger than an archive reader accepts (doc 05 §12: 2 GiB).
    TooLarge {
        /// Its size in bytes.
        len: u64,
    },
    /// The container, the layout or the sums are wrong, or `project/` differs from its manifest.
    Archive(ArchiveError),
    /// `ARCHIVE.json` is missing or does not read.
    Description,
    /// `project/` does not open.
    Open(OpenError),
}

impl fmt::Display for ArchiveOpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveOpenError::Io(e) => write!(f, "the archive could not be read: {e}"),
            ArchiveOpenError::TooLarge { len } => write!(
                f,
                "the archive holds {len} bytes, more than an archive may (2 GiB); it was not read"
            ),
            ArchiveOpenError::Archive(e) => e.fmt(f),
            ArchiveOpenError::Description => {
                f.write_str("the archive has no readable ARCHIVE.json, so it names no release")
            }
            ArchiveOpenError::Open(e) => write!(f, "the archived project does not open: {e:?}"),
        }
    }
}

impl std::error::Error for ArchiveOpenError {}

/// What went wrong in a release's housekeeping ([`ReleaseTrouble`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TroubleKind {
    /// A run record of this project directory could not be removed (its path is the record's).
    RecordNotRemoved,
    /// The folder of the run records could not be read (its path is the folder's).
    RecordsNotRead,
    /// The run record could not be updated with the written archive's modification time, so a discard after a crash
    /// keeps the archive (its path is the archive's).
    RecordNotUpdated,
}

/// One piece of a release's housekeeping trouble ([`Session::take_release_troubles`], FT-842): what happened, the
/// file it concerns and the error. Its `Display` is the text the CLI prints (with the full path); a window shows
/// [`ReleaseTrouble::file_name`] only.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReleaseTrouble {
    /// What happened.
    pub what: TroubleKind,
    /// The file or folder it concerns, in full.
    pub path: PathBuf,
    /// The error, in words (the CLI prints it).
    pub error: String,
    /// The error's kind: the window words the cause from it, never from the OS text (cycle 41, FIX41b ruling 6).
    pub kind: std::io::ErrorKind,
}

impl ReleaseTrouble {
    /// The last component of [`ReleaseTrouble::path`]: the file or folder name, never its path.
    pub fn file_name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }
}

impl fmt::Display for ReleaseTrouble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (path, error) = (self.path.display(), &self.error);
        match self.what {
            TroubleKind::RecordNotRemoved => {
                write!(f, "the release record {path} could not be removed: {error}")
            }
            TroubleKind::RecordsNotRead => {
                write!(f, "the release records in {path} could not be read: {error}")
            }
            TroubleKind::RecordNotUpdated => write!(
                f,
                "the release record of {path} could not be updated with the archive's time, so a discard after a crash \
                 keeps the archive: {error}"
            ),
        }
    }
}

/// Why a release (or settling an incomplete one) was refused or failed.
#[derive(Debug)]
pub enum ReleaseError {
    /// The session's origin may not release: the release is `HumanOnly` (the GUI, a confirmed CLI subcommand).
    HumanOnly {
        /// The origin.
        origin: Origin,
    },
    /// The gate could not run.
    Gate(GateError),
    /// The session is read-only: a release needs the write lock.
    ReadOnly,
    /// The gate has blockers, listed.
    Blocked(Vec<checks::Finding>),
    /// The archive root cannot be used.
    ArchiveRoot {
        /// The root.
        root: PathBuf,
        /// Why.
        why: &'static str,
    },
    /// An archive of the label exists: an incomplete release, discarded or completed first.
    Incomplete {
        /// The archive.
        archive: PathBuf,
    },
    /// The project lies in a git worktree, and the release commits and tags there: the person must agree.
    GitConsent {
        /// The worktree.
        worktree: PathBuf,
        /// The tag.
        tag: String,
    },
    /// The tag of the label exists in the worktree already.
    TagExists {
        /// The tag.
        tag: String,
    },
    /// A git step before the archive failed (a missing executable, a refused commit): nothing was written.
    Git(GitError),
    /// A freeze transaction was refused.
    Freeze(CmdError),
    /// The freeze would change printed pages (doc 15 §5.5): it was undone.
    FreezeChangesPages {
        /// The pages whose digests the freeze changed.
        pages: Vec<Id<Page>>,
    },
    /// A save failed (the freeze's, or the stamp's: then the archive and the tag of this run were removed).
    Save(SaveError),
    /// Generation g could not be read back for the archive.
    Generation(GenerationError),
    /// The PDF could not be written.
    Output(OutputError),
    /// The archive could not be built, or an incomplete one does not read.
    Archive(ArchiveError),
    /// The archive could not be written.
    Write(WriteError),
    /// The tag could not be created: the archive of this run was removed.
    Tag(GitError),
    /// The stamp was refused: the archive and the tag of this run were removed.
    Stamp(CmdError),
    /// The stamp is saved (generation g+1 says Released) but the session could not take it: reopen the project.
    /// The archive and the tag stay, since the saved release record names them.
    Unsettled(CmdError),
    /// The stamp's save failed before its commit point, but its intent is still in the project directory: the next
    /// open rolls the release forward, or reports it as an incomplete release. The archive and the tag stay.
    Undecided(SaveError),
    /// An I/O failure on the archive root or the incomplete archive.
    Io {
        /// The path.
        path: PathBuf,
        /// The error.
        error: std::io::Error,
    },
    /// There is no incomplete release to settle.
    NoIncomplete,
    /// The incomplete release cannot be completed (it says why): discard it.
    NotCompletable(String),
    /// The archive of the label in the archive root is not this project's incomplete release (it says why): it is
    /// neither completed nor discarded.
    Foreign(String),
    /// The archive names this project and the open revision, but no run record of this project directory shows that
    /// its own release wrote it (FT-814): a discard removes nothing.
    Unproven {
        /// Why (it names the archive).
        why: String,
        /// Whether the incomplete release can be completed instead.
        completable: bool,
    },
}

impl fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseError::HumanOnly { origin } => write!(
                f,
                "a release is human-only (Revision ▸ Release, ew revision release); the origin {origin:?} may not \
                 release"
            ),
            ReleaseError::Gate(e) => e.fmt(f),
            ReleaseError::ReadOnly => {
                f.write_str("the project is open read-only; a release needs the write lock")
            }
            ReleaseError::Blocked(list) => write!(f, "the release gate has {} blocker(s)", list.len()),
            ReleaseError::ArchiveRoot { root, why } => {
                write!(f, "the archive root {}: {why}", root.display())
            }
            ReleaseError::Incomplete { archive } => write!(
                f,
                "the archive {} exists: an incomplete release; complete or discard it first, or name another archive \
                 folder",
                archive.display()
            ),
            ReleaseError::GitConsent { worktree, tag } => write!(
                f,
                "the project lies in the git worktree {}: the release commits the project directory and creates the \
                 tag {tag}; confirm that first",
                worktree.display()
            ),
            ReleaseError::TagExists { tag } => write!(
                f,
                "the git tag {tag} exists already; a release never moves a tag (delete it, or discard the incomplete \
                 release)"
            ),
            ReleaseError::Git(e) => write!(f, "{e}; nothing was released"),
            ReleaseError::Freeze(e) => write!(f, "the freeze was refused: {e}"),
            ReleaseError::FreezeChangesPages { pages } => write!(
                f,
                "the freeze would change {} printed page(s); it was undone and the release aborted (doc 15 §5.5)",
                pages.len()
            ),
            ReleaseError::Save(e) => write!(f, "the save failed: {e}"),
            ReleaseError::Generation(e) => write!(f, "the saved generation could not be read back: {e}"),
            ReleaseError::Output(e) => write!(f, "the PDF could not be written: {e}"),
            ReleaseError::Archive(e) => e.fmt(f),
            ReleaseError::Write(e) => e.fmt(f),
            ReleaseError::Tag(e) => write!(f, "{e}; the archive of this run was removed"),
            ReleaseError::Stamp(e) => write!(
                f,
                "the stamp was refused: {e}; the archive and tag of this run were removed"
            ),
            ReleaseError::Unsettled(e) => write!(
                f,
                "the release is saved, but the session could not take its stamp ({e}); reopen the project"
            ),
            ReleaseError::Undecided(e) => write!(
                f,
                "the stamp's save failed ({e}) and left its intent behind; reopen the project: it completes the \
                 release, or offers to discard or complete it"
            ),
            ReleaseError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            ReleaseError::NoIncomplete => f.write_str("there is no incomplete release"),
            ReleaseError::NotCompletable(why) => {
                write!(f, "the incomplete release cannot be completed: {why}")
            }
            ReleaseError::Foreign(why) => write!(f, "{why}; it is kept"),
            ReleaseError::Unproven {
                why,
                completable: true,
            } => write!(
                f,
                "{why}; it is kept: complete the release with it, or remove the file by hand if you know it is a \
                 leftover"
            ),
            ReleaseError::Unproven {
                why,
                completable: false,
            } => write!(
                f,
                "{why}; it is kept: remove the file by hand if you know it is a leftover, or name another archive \
                 folder"
            ),
        }
    }
}

impl std::error::Error for ReleaseError {}

/// The archive's description, `ARCHIVE.json` (doc 05 §14.2).
fn archive_json(
    session: &Session,
    revision: Id<Revision>,
    ctx: &ReleaseContext,
    generation: u64,
    manifest: &ew_store::ew_format::Manifest,
    record: &ReleaseRecord,
) -> Vec<u8> {
    let value = serde_json::json!({
        "context": {"date": ctx.date.to_string(), "label": ctx.label},
        "derivation_profile": record.derivation_profile,
        "format": manifest.format,
        "generation": generation,
        "generator": format!("ElektraWatt {}", env!("CARGO_PKG_VERSION")),
        "project": session.query().project().id.to_string(),
        "release": record,
        "revision": {"id": revision.to_string(), "label": ctx.label},
        "rule_packs": record.rule_packs,
    });
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
    text.push('\n');
    text.into_bytes()
}

/// The facts `ARCHIVE.json` holds: the project, the revision, the release context's date, the generation and the
/// record.
fn archive_facts(bytes: &[u8]) -> Option<(String, Id<Revision>, Date, u64, ReleaseRecord)> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let project = v.get("project")?.as_str()?.to_owned();
    let revision = v.get("revision")?.get("id")?.as_str()?.parse().ok()?;
    let date = v.get("context")?.get("date")?.as_str()?.parse().ok()?;
    let generation = v.get("generation")?.as_u64()?;
    let record = serde_json::from_value(v.get("release")?.clone()).ok()?;
    Some((project, revision, date, generation, record))
}

/// The tag message of a release (doc 05 §14.1 step 5): it carries the archive's SHA-256 and, from the release itself,
/// the run's mark (`Run: <mark>`, FT-814), which the run's record in the state directory holds too.
fn tag_message(label: &str, name: &str, sha256: &Sha256, run: Option<&str>) -> String {
    let mut message = format!("Release {label}\n\nArchive: {name}\nSHA-256: {sha256}\n");
    if let Some(run) = run {
        message.push_str("Run: ");
        message.push_str(run);
        message.push('\n');
    }
    message
}

/// Whether a tag's message carries the archive's SHA-256 (the line [`tag_message`] writes): then the tag is this
/// archive's release's, and a completion may keep it.
fn tag_carries(message: &str, sha256: &Sha256) -> bool {
    let line = format!("SHA-256: {sha256}");
    message.lines().any(|l| l.trim() == line)
}

/// Whether a tag's message carries the run's mark (the line [`tag_message`] writes): then this project directory's run
/// created it, and a discard may delete it (FT-814; another copy in the same repository shares the tag names).
fn tag_of_run(message: &str, run: &str) -> bool {
    let line = format!("Run: {run}");
    message.lines().any(|l| l.trim() == line)
}

/// Refuses an origin that may not release (`revision.release` is `HumanOnly`: the GUI, a confirmed CLI subcommand).
fn human_only(origin: &Origin) -> Result<(), ReleaseError> {
    if ReleaseRevision::meta().exposure.allows(origin) {
        Ok(())
    } else {
        Err(ReleaseError::HumanOnly {
            origin: origin.clone(),
        })
    }
}

/// The project's ID: what an archive's `ARCHIVE.json` must name to be this project's.
fn project_id(session: &Session) -> String {
    session.query().project().id.to_string()
}

impl Session {
    /// The archive root of `options` (the default one beside the project directory when `None`), refused inside the
    /// project directory (ADR-0013).
    fn archive_root(&self, root: Option<&Path>) -> Result<PathBuf, ReleaseError> {
        let store = self.store().ok_or(ReleaseError::Gate(GateError::NoProject))?;
        let project = store.root();
        let chosen = match root {
            Some(r) => r.to_path_buf(),
            None => archive::default_root(project).ok_or_else(|| ReleaseError::ArchiveRoot {
                root: project.to_path_buf(),
                why: "the project directory has no parent, so there is no default archive root; name one",
            })?,
        };
        let vfs = self.release_vfs()?;
        if archive::lies_inside(&*vfs, &chosen, project) {
            return Err(ReleaseError::ArchiveRoot {
                root: chosen,
                why: "it lies inside the project directory; an archive is kept outside the project (ADR-0013)",
            });
        }
        Ok(chosen)
    }

    /// The I/O seam of the session's store.
    fn release_vfs(&self) -> Result<std::sync::Arc<dyn Vfs>, ReleaseError> {
        self.store()
            .map(|s| std::sync::Arc::clone(s.vfs()))
            .ok_or(ReleaseError::Gate(GateError::NoProject))
    }

    /// The owner marker of the project directory's archive temps ([`archive::temp_name`]): the key of the project
    /// directory ([`archive::path_key`]). A copy of the directory elsewhere carries the same project ID but has another
    /// key, so it never removes a temp of this directory's live run, nor this directory one of its (FT-814).
    fn temp_owner(&self) -> Result<String, ReleaseError> {
        let store = self.store().ok_or(ReleaseError::Gate(GateError::NoProject))?;
        Ok(archive::path_key(&**store.vfs(), store.root()))
    }

    /// The file of this project directory's run record for `archive` ([`archive::run_record_path`]); `None` without a
    /// state directory.
    fn run_record_path(&self, archive: &Path) -> Option<PathBuf> {
        let store = self.store()?;
        let state = store.state_dir()?;
        let vfs = &**store.vfs();
        Some(archive::run_record_path(
            state,
            &archive::path_text(vfs, store.root()),
            &archive::path_text(vfs, archive),
        ))
    }

    /// What this project directory's run records for the archive it writes (FT-814), with the run's mark: the record
    /// [`archive::write_atomically`] writes before the archive has its name (it fills in the archive, its SHA-256 and
    /// its modification time). `None` without a state directory: then there is no record, and a discard after a crash
    /// keeps the archive (the release itself goes on).
    fn run_record_at(
        &self,
        revision: Id<Revision>,
        label: &str,
        generation: u64,
        run: &str,
    ) -> Option<archive::RecordAt<'_>> {
        let store = self.store()?;
        let state = store.state_dir()?;
        let vfs = &**store.vfs();
        Some(archive::RecordAt {
            state,
            record: archive::RunRecord {
                project_dir: archive::path_text(vfs, store.root()),
                archive: String::new(),
                project: project_id(self),
                revision: revision.to_string(),
                label: label.to_owned(),
                generation,
                sha256: String::new(),
                modified: None,
                run: run.to_owned(),
            },
        })
    }

    /// Removes the run record at `path`, if any. A failure is kept for the front-end
    /// ([`Session::take_release_troubles`]): a record left behind names an archive whose bytes and modification time a
    /// later file must both match, and the next open removes it once its archive is gone or released.
    fn remove_run_record(&self, path: Option<&Path>) {
        if let (Some(path), Some(store)) = (path, self.store())
            && let Err(e) = archive::remove_run_record(&**store.vfs(), path)
        {
            self.release_trouble(path, &e);
        }
    }

    /// Keeps a failed removal of the release record `path` for the front-end.
    fn release_trouble(&self, path: &Path, error: &std::io::Error) {
        self.release_troubles.borrow_mut().push(ReleaseTrouble {
            what: TroubleKind::RecordNotRemoved,
            path: path.to_path_buf(),
            error: error.to_string(),
            kind: error.kind(),
        });
    }

    /// The release's housekeeping trouble since the last call, in words, oldest first: a run record of this project
    /// directory that could not be removed (when a release is settled, or when a stale one is removed as the project
    /// opens or [`Session::incomplete_release`] looks). Nothing is lost by it (a record only allows a discard of what
    /// matches it exactly); the front-end shows each as a warning (the CLI its `Display` text, the window its words
    /// with the file name only, FT-842).
    pub fn take_release_troubles(&self) -> Vec<ReleaseTrouble> {
        std::mem::take(&mut *self.release_troubles.borrow_mut())
    }

    /// Removes this project directory's stale run records (FT-814): those whose archive is gone (a crash before the
    /// archive had its name, a discard by hand) or whose revision is released with the archive's SHA-256 (the run
    /// was settled by the next open's roll-forward or a completion, but its record stayed). A record left behind
    /// would let a restored pre-release copy of this directory at the same path discard the released archive. Only in
    /// an editing session (the write lock is ours, so no run of this directory is writing); a failed removal is kept
    /// ([`Session::take_release_troubles`]).
    pub(crate) fn remove_stale_run_records(&self) {
        let Some(store) = self.store() else { return };
        let Some(state) = store.state_dir() else { return };
        if !matches!(store.access(), crate::Access::Edit) {
            return;
        }
        let vfs = &**store.vfs();
        let project_dir = archive::path_text(vfs, store.root());
        let records = match archive::run_records_of(vfs, state, &project_dir) {
            Ok(records) => records,
            Err(e) => {
                self.release_troubles.borrow_mut().push(ReleaseTrouble {
                    what: TroubleKind::RecordsNotRead,
                    path: state.join(archive::RUNS_DIR),
                    error: e.to_string(),
                    kind: e.kind(),
                });
                return;
            }
        };
        let q = self.query();
        for (path, record) in records {
            let gone = matches!(
                vfs.stat(Path::new(&record.archive)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound
            );
            let released = q.table::<Revision>().iter().any(|(_, r)| {
                r.id.to_string() == record.revision
                    && r.release
                        .as_ref()
                        .and_then(|rel| rel.archive.as_ref())
                        .and_then(|a| a.sha256)
                        .is_some_and(|sha| sha.to_string() == record.sha256)
            });
            if (gone || released)
                && let Err(e) = archive::remove_run_record(vfs, &path)
            {
                self.release_trouble(&path, &e);
            }
        }
    }

    /// Removes the archive this run wrote after a failure in steps 5 or 6 (D13), then its run record. When the archive
    /// cannot be removed, the record stays with it: the archive is then an incomplete release that a discard removes.
    fn take_back(&self, archive_path: &Path, run_record: Option<&Path>) {
        let Some(store) = self.store() else { return };
        if archive::remove(&**store.vfs(), archive_path).is_ok() {
            self.remove_run_record(run_record);
        }
    }

    /// The run record of this project directory that proves its own release wrote the incomplete release's archive
    /// (FT-814): the record of the directory and the archive, naming this project, the revision, the label, the
    /// archive's generation, its SHA-256 and its modification time. Else why the archive is not provably this
    /// directory's leftover (it names the archive).
    fn run_evidence(&self, found: &IncompleteRelease) -> Result<archive::RunRecord, String> {
        let name = archive::file_name(&found.label);
        let unproven = |why: &str| {
            format!(
                "the archive {name} in that folder is not provably a leftover of this project folder's own release \
                 ({why})"
            )
        };
        let store = self.store().ok_or_else(|| GateError::NoProject.to_string())?;
        let Some(state) = store.state_dir() else {
            return Err(unproven(
                "this session keeps no release records: it has no state directory",
            ));
        };
        let vfs = &**store.vfs();
        let project_dir = archive::path_text(vfs, store.root());
        let archive_text = archive::path_text(vfs, &found.archive);
        let path = archive::run_record_path(state, &project_dir, &archive_text);
        let record = archive::read_run_record(vfs, &path)
            .filter(|r| r.project_dir == project_dir && r.archive == archive_text)
            .ok_or_else(|| {
                unproven(
                    "no release from this folder recorded writing it, and a copy of the folder names the same \
                     project",
                )
            })?;
        let modified = vfs.stat(&found.archive).ok().and_then(|s| s.modified);
        let same = record.project == project_id(self)
            && record.revision == found.revision.to_string()
            && record.label == found.label
            && found
                .facts
                .as_ref()
                .is_some_and(|(_, generation, _)| *generation == record.generation)
            && record.sha256 == found.sha256.to_string()
            && archive::same_time(record.modified, modified);
        if same {
            Ok(record)
        } else {
            Err(unproven(
                "the release from this folder recorded another file under that name",
            ))
        }
    }

    /// What [`Session::release`] would do for the open revision `label`, judged without writing anything: the gate
    /// (which must pass), the archive, the worktree and its tag. Refused as the release would be before step 2.
    ///
    /// # Errors
    ///
    /// [`ReleaseError`]: the origin, the gate, a read-only session, blockers, the archive root, an existing archive of
    /// the label, git (missing, an existing tag, no consent when `options.git_consent` is checked by the caller).
    pub fn release_plan(
        &self,
        label: &str,
        integrity: &[checks::IntegrityItem],
        options: &ReleaseOptions<'_>,
    ) -> Result<ReleasePlan, ReleaseError> {
        self.release_plan_as(&self.submit_origin.clone(), label, integrity, options)
    }

    /// [`Session::release_plan`] for a submission from `origin` (cycle 41, ruling 1: the `HumanOnly` gate is judged per
    /// submission, by the origin of the one that asks, never by the session's): the window passes `Origin::Gui` only
    /// for a dialog a person confirmed at the window, and a scripted origin is refused here.
    ///
    /// # Errors
    ///
    /// As [`Session::release_plan`], [`ReleaseError::HumanOnly`] judged by `origin`.
    pub fn release_plan_as(
        &self,
        origin: &Origin,
        label: &str,
        integrity: &[checks::IntegrityItem],
        options: &ReleaseOptions<'_>,
    ) -> Result<ReleasePlan, ReleaseError> {
        human_only(origin)?;
        self.judge_release(label, integrity, options)
    }

    /// What a release of the open revision `label` would do, judged without writing anything and without the origin's
    /// check (a dialog shows it before anyone confirms): the gate, which must pass, the archive, the worktree and its
    /// tag. The release itself judges again, from the origin that asks ([`Session::release_plan_as`]).
    ///
    /// # Errors
    ///
    /// As [`Session::release_plan`] but for the origin.
    pub fn judge_release(
        &self,
        label: &str,
        integrity: &[checks::IntegrityItem],
        options: &ReleaseOptions<'_>,
    ) -> Result<ReleasePlan, ReleaseError> {
        let gate = self.release_gate(label, integrity).map_err(ReleaseError::Gate)?;
        if !matches!(
            self.store().map(ew_store::Store::access),
            Some(crate::Access::Edit)
        ) {
            return Err(ReleaseError::ReadOnly);
        }
        let blockers: Vec<checks::Finding> = gate.blockers().into_iter().cloned().collect();
        if !blockers.is_empty() {
            return Err(ReleaseError::Blocked(blockers));
        }
        let vfs = self.release_vfs()?;
        let root = self.archive_root(options.archive_root)?;
        let path = root.join(archive::file_name(label));
        match vfs.stat(&path) {
            Ok(_) => return Err(ReleaseError::Incomplete { archive: path }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ReleaseError::Io { path, error }),
        }
        let project = self.store().map(|s| s.root().to_path_buf()).unwrap_or_default();
        let worktree = options
            .git
            .worktree(&*vfs, &project)
            .map_err(|error| ReleaseError::Io {
                path: project.clone(),
                error,
            })?;
        let (git, archive_in_worktree) = match worktree {
            None => (None, false),
            Some(w) => {
                let tag = git::tag_name(label).map_err(ReleaseError::Git)?;
                options.git.available().map_err(ReleaseError::Git)?;
                if options
                    .git
                    .tag_exists(&project, &tag)
                    .map_err(ReleaseError::Git)?
                {
                    return Err(ReleaseError::TagExists { tag });
                }
                let inside = archive::lies_inside(&*vfs, &root, &w);
                (Some((w, tag)), inside)
            }
        };
        Ok(ReleasePlan {
            gate,
            archive: path,
            git,
            archive_in_worktree,
        })
    }

    /// Releases the open revision (doc 05 §14.1; WP-S1.18 part 4, D11 to D13): the six steps of the module
    /// documentation, from the session's origin, which must be human. Returns the release; the session is saved at
    /// g+1 with the revision released.
    ///
    /// # Errors
    ///
    /// [`ReleaseError`]: every refusal of [`Session::release_plan`] and a missing consent come before anything is
    /// written; a failure of steps 4 to 6 leaves the revision open with no archive or tag of this run.
    pub fn release(
        &mut self,
        ctx: &ReleaseContext,
        integrity: &[checks::IntegrityItem],
        options: &ReleaseOptions<'_>,
    ) -> Result<Released, ReleaseError> {
        let origin = self.submit_origin.clone();
        self.release_with(&origin, ctx, integrity, options, &[])
    }

    /// [`Session::release`] for a submission from `origin` (cycle 41, ruling 1: judged per submission; see
    /// [`Session::release_plan_as`]). Every transaction of the release (the freeze, the stamp) is submitted from
    /// `origin`.
    ///
    /// # Errors
    ///
    /// As [`Session::release`], [`ReleaseError::HumanOnly`] judged by `origin`.
    pub fn release_as(
        &mut self,
        origin: &Origin,
        ctx: &ReleaseContext,
        integrity: &[checks::IntegrityItem],
        options: &ReleaseOptions<'_>,
    ) -> Result<Released, ReleaseError> {
        self.release_with(origin, ctx, integrity, options, &[])
    }

    /// [`Session::release_as`] with the freeze transactions of step 2 (the seam of the digest assertion's test; at MVP
    /// the release freezes nothing).
    pub(crate) fn release_with(
        &mut self,
        origin: &Origin,
        ctx: &ReleaseContext,
        integrity: &[checks::IntegrityItem],
        options: &ReleaseOptions<'_>,
        freeze: &[AnyCommand],
    ) -> Result<Released, ReleaseError> {
        // Step 1: the gate and every refusal that needs no write.
        let plan = self.release_plan_as(origin, &ctx.label, integrity, options)?;
        if let (Some((worktree, tag)), false) = (&plan.git, options.git_consent) {
            return Err(ReleaseError::GitConsent {
                worktree: worktree.clone(),
                tag: tag.clone(),
            });
        }
        let parallelism = options.parallelism;
        // Step 2: the freeze, guarded by the page digests, then generation g.
        let before = page_digests(&self.query(), parallelism);
        let mut frozen = 0;
        for cmd in freeze {
            let submit = Submit::from(origin.clone());
            if let Err(e) = self.execute_with(cmd, submit) {
                self.undo_freeze(origin, frozen);
                return Err(ReleaseError::Freeze(e));
            }
            frozen += 1;
        }
        if frozen > 0 {
            let after = page_digests(&self.query(), parallelism);
            if after != before {
                let pages = before
                    .keys()
                    .chain(after.keys())
                    .filter(|p| before.get(p) != after.get(p))
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                self.undo_freeze(origin, frozen);
                return Err(ReleaseError::FreezeChangesPages { pages });
            }
            self.save().map_err(ReleaseError::Save)?;
        }
        let (content_digest, generation) = self.content_digest().map_err(ReleaseError::Gate)?;
        let project = self.store().map(|s| s.root().to_path_buf()).unwrap_or_default();
        // Step 3: the project directory committed at g.
        let committed = match &plan.git {
            Some(_) => Some(
                options
                    .git
                    .commit_dir(&project, &format!("Release {}", ctx.label))
                    .map_err(ReleaseError::Git)?,
            ),
            None => None,
        };
        // Step 4: the archive.
        let record = self.release_record(&plan, ctx, content_digest, generation, &before, None);
        let (bytes, manifest) = self.archive_bytes(&plan.gate, ctx, generation, &record, parallelism)?;
        let vfs = self.release_vfs()?;
        let root = plan.archive.parent().map(Path::to_path_buf).unwrap_or_default();
        let verify = |back: &[u8]| -> Result<(), ArchiveError> {
            let limits = ZipLimits {
                max_ratio: u64::MAX,
                ..ZipLimits::default()
            };
            let entries = archive::read(back, &limits)?;
            archive::check_project(&entries, &manifest.0, &manifest.1)
        };
        let owner = self.temp_owner()?;
        // The run's record (FT-814), written before the archive has its name: the evidence a discard after a crash
        // needs that this directory's run wrote the archive. Its mark goes into the tag's message.
        let run = ew_store::lock::new_session_id();
        let record_at = self.run_record_at(plan.gate.revision, &ctx.label, generation, &run);
        let written = archive::write_atomically(
            &*vfs, &root, &ctx.label, &owner, generation, &bytes, &verify, record_at,
        )
        .map_err(ReleaseError::Write)?;
        let sha256 = Sha256(written.sha256);
        let run_record = written.run_record.clone();
        if let Some(e) = &written.record_trouble {
            // FIX40b (finding 2): the archive is kept; only the discard evidence after a crash is lost.
            self.release_troubles.borrow_mut().push(ReleaseTrouble {
                what: TroubleKind::RecordNotUpdated,
                path: written.path.clone(),
                error: e.to_string(),
                kind: e.kind(),
            });
        }
        // Step 5: the tag.
        if let Some((_, tag)) = &plan.git {
            let name = archive::file_name(&ctx.label);
            if let Err(e) = options.git.tag(
                &project,
                tag,
                &tag_message(&ctx.label, &name, &sha256, Some(&run)),
            ) {
                self.take_back(&written.path, run_record.as_deref());
                return Err(ReleaseError::Tag(e));
            }
        }
        // Step 6: the stamp.
        let tag = plan.git.as_ref().map(|(_, t)| t.clone());
        let stamped = self.stamp(origin, &plan.gate, &record, &written.path, sha256, tag.as_deref());
        let (stamped, save_incomplete) = match stamped {
            Ok(s) => {
                self.remove_run_record(run_record.as_deref());
                s
            }
            // The stamp is saved: the release record names the archive and the tag.
            Err(e @ ReleaseError::Unsettled(_)) => {
                self.remove_run_record(run_record.as_deref());
                return Err(e);
            }
            // The next open completes the release or reports the incomplete one: the record stays with the archive (and
            // goes at that open when the roll-forward released the revision).
            Err(e @ ReleaseError::Undecided(_)) => return Err(e),
            Err(e) => {
                if let Some(t) = &tag {
                    let _ = options.git.delete_tag(&project, t);
                }
                self.take_back(&written.path, run_record.as_deref());
                return Err(e);
            }
        };
        Ok(Released {
            revision: plan.gate.revision,
            label: ctx.label.clone(),
            generation,
            stamped,
            archive: written.path,
            sha256,
            git: committed.zip(tag),
            save_incomplete,
        })
    }

    /// Undoes the last `n` freeze transactions (an aborted release).
    fn undo_freeze(&mut self, origin: &Origin, n: usize) {
        for _ in 0..n {
            let _ = self.undo(origin);
        }
        // The freeze ran on the saved generation (the gate needs one): undone, the model is that generation again, so
        // its keys leave the dirty set and the content digest is the saved one again.
        if !self.engine.is_dirty()
            && let Some(p) = self.persistence.as_mut()
        {
            p.unsaved.clear();
        }
    }

    /// The release record's facts (doc 15 §5.5), without the archive: the sign-off as `approval`, the content digest of
    /// g, the derivation profile, the gate's packs, every page digest and the changed pages; the git tag when the
    /// project lies in a worktree.
    fn release_record(
        &self,
        plan: &ReleasePlan,
        ctx: &ReleaseContext,
        content_digest: Sha256,
        generation: u64,
        digests: &BTreeMap<Id<Page>, Digest128>,
        archive: Option<ew_schema::ArchiveRef>,
    ) -> ReleaseRecord {
        let q = self.query();
        let revision = q.table::<Revision>().get(plan.gate.revision).cloned();
        let approval = revision.as_ref().and_then(|r| r.sign_off.clone());
        // The first release has no earlier one: every page is new (doc 15 §5.6, `p ∉ r_prev`); later releases take the
        // digest or page-note branch of `changed_pages`.
        let changed: Vec<Id<Page>> = match (ew_derive::last_released_revision(&q), &revision) {
            (Some(_), Some(r)) => changed_pages(&q, r)
                .into_iter()
                .map(|p| Id::from_uuid(page_uuid(p)))
                .collect(),
            _ => q
                .get::<ew_derive::PageOrder>(())
                .entries
                .iter()
                .map(|e| Id::from_uuid(page_uuid(e.page)))
                .collect(),
        };
        ReleaseRecord {
            approval,
            archive,
            changed_pages: Some(changed),
            content_digest: Some(content_digest),
            derivation_profile: Some(q.project().derivation_profile.clone()),
            generation: Some(generation),
            git_tag: plan.git.as_ref().and_then(|(_, t)| t.parse::<Ident>().ok()),
            page_digests: Some(digests.clone()),
            released_on: Some(ctx.date),
            rule_packs: Some(
                plan.gate
                    .packs
                    .iter()
                    .map(|p| ew_schema::PackVersion {
                        id: p.id.parse().ok(),
                        sha256: Some(Sha256(p.sha256)),
                        version: p.version.parse().ok(),
                        extra: ew_schema::Extra::new(),
                    })
                    .collect(),
            ),
            extra: ew_schema::Extra::new(),
        }
    }

    /// The archive's PDF entry (doc 05 §14.2): its path `pdf/<drawing id>_<label>.pdf` and the PDF/A-2b of the current
    /// snapshot dated with the release context. [`Session::open_archive`]'s reproduction calls the same function on the
    /// archived `project/` (D14, RU-1).
    fn archived_pdf(
        &self,
        ctx: &ReleaseContext,
        parallelism: Parallelism,
    ) -> Result<(String, Vec<u8>), OutputError> {
        let q = self.query();
        let drawing = q
            .project()
            .master
            .drawing_id
            .as_ref()
            .or(q.project().master.object_id.as_ref())
            .map_or_else(|| "project".to_owned(), |d| d.as_str().to_owned());
        let pdf = self.export_pdf(&ExportOptions {
            parallelism,
            date: Some(PdfDate::ymd(ctx.date.year(), ctx.date.month(), ctx.date.day())),
            ..ExportOptions::default()
        })?;
        Ok((
            format!(
                "pdf/{}_{}.pdf",
                archive::sanitise(&drawing),
                archive::sanitise(&ctx.label)
            ),
            pdf.bytes,
        ))
    }

    /// The archive's frozen report rows (FR-REV-011): `reports/<report-block id>.csv` of every report block of the
    /// current snapshot. [`OpenedArchive::derived_reports`] calls it on the archived `project/` (D14, RU-2).
    fn archived_reports(&self) -> BTreeMap<String, Vec<u8>> {
        let q = self.query();
        q.report_blocks()
            .ids()
            .map(|block| {
                let rows = ew_reports::rows_of(&q, block);
                (
                    format!("reports/{block}.csv"),
                    ew_reports::csv::rows_csv(&q, &rows).into_bytes(),
                )
            })
            .collect()
    }

    /// The archive's bytes (doc 05 §14.2, D11) from generation g read back from disk and the release context, with the
    /// manifest and its bytes the verification checks `project/` against.
    #[allow(clippy::type_complexity)]
    fn archive_bytes(
        &self,
        gate: &Gate,
        ctx: &ReleaseContext,
        generation: u64,
        record: &ReleaseRecord,
        parallelism: Parallelism,
    ) -> Result<(Vec<u8>, (ew_store::ew_format::Manifest, Vec<u8>)), ReleaseError> {
        let store = self.store().ok_or(ReleaseError::Gate(GateError::NoProject))?;
        let g = store.read_generation().map_err(ReleaseError::Generation)?;
        let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        entries.insert(
            format!("{}{}", archive::PROJECT_PREFIX, ew_store::ew_format::MANIFEST),
            g.manifest_bytes.clone(),
        );
        for (path, bytes) in g.files {
            entries.insert(format!("{}{path}", archive::PROJECT_PREFIX), bytes);
        }
        let (pdf_name, pdf) = self
            .archived_pdf(ctx, parallelism)
            .map_err(ReleaseError::Output)?;
        entries.insert(pdf_name, pdf);
        entries.extend(self.archived_reports());
        let mut check = checks::report::to_json(&gate.report);
        if let Some(o) = check.as_object_mut() {
            o.insert(
                "packs".into(),
                gate.packs
                    .iter()
                    .map(|p| {
                        serde_json::json!({"id": p.id, "version": p.version, "sha256": Sha256(p.sha256).to_string()})
                    })
                    .collect(),
            );
            o.insert("disclaimer".into(), checks::report::DISCLAIMER.into());
            o.insert(
                "sign_off".into(),
                serde_json::to_value(&record.approval).unwrap_or_default(),
            );
        }
        let mut text = serde_json::to_string_pretty(&check).unwrap_or_default();
        text.push('\n');
        entries.insert("check/report.json".to_owned(), text.into_bytes());
        entries.insert(
            archive::DESCRIPTION_ENTRY.to_owned(),
            archive_json(self, gate.revision, ctx, generation, &g.manifest, record),
        );
        let bytes = archive::build(&entries).map_err(ReleaseError::Archive)?;
        Ok((bytes, (g.manifest, g.manifest_bytes)))
    }

    /// Step 6: `revision.release` prepared on a fork, its candidate saved as g+1, then committed without a journal entry
    /// (the save holds it) and marked saved. Returns the generation and whether the save is incomplete.
    fn stamp(
        &mut self,
        origin: &Origin,
        gate: &Gate,
        record: &ReleaseRecord,
        archive: &Path,
        sha256: Sha256,
        tag: Option<&str>,
    ) -> Result<(u64, bool), ReleaseError> {
        let name = archive
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let cmd = ReleaseRevision {
            revision: gate.revision,
            released_on: record
                .released_on
                .unwrap_or_else(|| Date::new(1980, 1, 1).expect("a date")),
            generation: record.generation.unwrap_or_default(),
            content_digest: record.content_digest.unwrap_or(gate.content_digest),
            derivation_profile: record
                .derivation_profile
                .clone()
                .unwrap_or_else(|| self.query().project().derivation_profile.clone()),
            rule_packs: record
                .rule_packs
                .iter()
                .flatten()
                .filter_map(|p| {
                    Some(ApprovedPack {
                        id: p.id.clone()?,
                        version: p.version.clone()?,
                        sha256: p.sha256?,
                    })
                })
                .collect(),
            page_digests: record.page_digests.clone().unwrap_or_default(),
            changed_pages: record.changed_pages.clone().unwrap_or_default(),
            archive: ReleasedArchive { name, sha256 },
            git_tag: tag.and_then(|t| t.parse().ok()),
        };
        let submit = Submit::from(origin.clone());
        let preview = self
            .engine
            .preview_with(&cmd, submit)
            .map_err(ReleaseError::Stamp)?;
        let keys: BTreeSet<AnyKey> = preview.diff().changes.iter().map(|c| c.key).collect();
        let candidate = preview.candidate().clone();
        let Some(p) = self.persistence.as_mut() else {
            return Err(ReleaseError::Gate(GateError::NoProject));
        };
        let before = p.store.generation();
        let (generation, incomplete) = match p.store.save(&candidate, Changes::Keys(&keys)) {
            Ok(report) => (report.generation, false),
            Err(SaveError::Incomplete { generation, .. }) if generation > before => (generation, true),
            // Before its commit point, unless the store could not delete its intent: then the next open rolls the
            // stamp forward (or finds the intent torn), so the archive and the tag must stay (D13).
            Err(e) => {
                return Err(match p.store.leftover_intent() {
                    Ok(None) => ReleaseError::Save(e),
                    _ => ReleaseError::Undecided(e),
                });
            }
        };
        // The candidate is on disk: commit it (the snapshot is unchanged since the preview, so it is swapped in as is).
        let result = self.engine.commit(preview);
        let changes = self.engine.drain_changes();
        let touched: Vec<AnyKey> = changes
            .iter()
            .flat_map(|c| c.touched_keys.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if let Some(memo) = &self.memo {
            memo.advance(self.engine.model(), &touched);
        }
        self.changes.extend(changes);
        if let Some(p) = self.persistence.as_mut() {
            p.recovered = false;
        }
        self.engine.mark_saved();
        result.map_err(ReleaseError::Unsettled)?;
        Ok((generation, incomplete))
    }

    /// The incomplete release a crash left (doc 05 §14.1 "On failure"): an archive of the open revision's label in the
    /// archive root (`archive_root`, else the default one), which no release record names because the revision is still
    /// open. `None` when there is no open revision or no such archive. The archive is read and checked: a damaged one,
    /// one of another project or revision, or one whose generation is not the saved one is reported with the reason it
    /// cannot be completed ([`IncompleteRelease::refusal`]); a damaged one or one of another project or revision is not
    /// this project's at all and is kept ([`IncompleteRelease::foreign`]). One of this project and revision that no run
    /// record of this project directory matches is not provably this directory's leftover, and a discard keeps it
    /// ([`IncompleteRelease::unproven`], FT-814). This directory's stale run records go first (their archive gone or
    /// released), as when the project opens.
    ///
    /// # Errors
    ///
    /// [`ReleaseError`]: no project directory, an unusable archive root, an I/O failure reading the archive.
    pub fn incomplete_release(
        &self,
        archive_root: Option<&Path>,
    ) -> Result<Option<IncompleteRelease>, ReleaseError> {
        self.remove_stale_run_records();
        let Some((revision, label)) = self
            .query()
            .table::<Revision>()
            .iter()
            .find(|(_, r)| r.is_open())
            .map(|(_, r)| (r.id, r.label.as_str().to_owned()))
        else {
            return Ok(None);
        };
        let vfs = self.release_vfs()?;
        let root = self.archive_root(archive_root)?;
        let path = root.join(archive::file_name(&label));
        let bytes = match vfs.read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ReleaseError::Io { path, error }),
        };
        let sha256 = Sha256(ew_store::ew_format::sha256(&bytes));
        let limits = ZipLimits {
            max_ratio: u64::MAX,
            ..ZipLimits::default()
        };
        let name = archive::file_name(&label);
        let mut found = IncompleteRelease {
            revision,
            label,
            archive: path,
            sha256,
            facts: None,
            refusal: None,
            foreign: None,
            unproven: None,
        };
        let foreign = |found: &mut IncompleteRelease, why: String| {
            found.refusal = Some(why.clone());
            found.foreign = Some(why);
        };
        let entries = match archive::read(&bytes, &limits) {
            Ok(e) => e,
            Err(e) => {
                foreign(
                    &mut found,
                    format!("the archive {name} in that folder is damaged ({e}), so it names no project"),
                );
                return Ok(Some(found));
            }
        };
        let Some((project, of, date, generation, record)) = entries
            .get(archive::DESCRIPTION_ENTRY)
            .and_then(|b| archive_facts(b))
        else {
            foreign(
                &mut found,
                format!(
                    "the archive {name} in that folder has no readable ARCHIVE.json, so it names no project"
                ),
            );
            return Ok(Some(found));
        };
        found.facts = Some((date, generation, record.clone()));
        if project != project_id(self) {
            foreign(
                &mut found,
                format!("the archive {name} in that folder belongs to another project"),
            );
        } else if of != revision {
            foreign(
                &mut found,
                format!("the archive {name} in that folder belongs to another revision"),
            );
        } else {
            found.refusal = self.completable(&entries, generation, &record).err();
            found.unproven = self.run_evidence(&found).err();
        }
        Ok(Some(found))
    }

    /// Whether an incomplete release's archive is the saved generation: no unsaved edits, generation and content
    /// digest equal, `project/` equal to the generation on disk.
    fn completable(
        &self,
        entries: &BTreeMap<String, Vec<u8>>,
        generation: u64,
        record: &ReleaseRecord,
    ) -> Result<(), String> {
        let (digest, saved) = self.content_digest().map_err(|e| e.to_string())?;
        if saved != generation || record.content_digest != Some(digest) {
            return Err(format!(
                "the project changed since the archive was written (generation {generation}, now {saved})"
            ));
        }
        let g = self
            .store()
            .ok_or_else(|| GateError::NoProject.to_string())?
            .read_generation()
            .map_err(|e| e.to_string())?;
        archive::check_project(entries, &g.manifest, &g.manifest_bytes).map_err(|e| e.to_string())
    }

    /// Discards the incomplete release (doc 05 §14.1 "On failure"): deletes the tag `ew/rev/<label>` when the project
    /// lies in a worktree and the tag's message carries the archive's SHA-256 and the crashed run's mark (this project
    /// directory's run created it; any other tag of the label is kept), then the archive, the leftover temps of its
    /// name that this project directory wrote, and the run's record. Only an archive whose `ARCHIVE.json` names this
    /// project and the open revision, and that a run record of this project directory proves its own (FT-814), is
    /// discarded. The revision stays open.
    ///
    /// # Errors
    ///
    /// [`ReleaseError`]: the origin, a read-only session, [`ReleaseError::NoIncomplete`], [`ReleaseError::Foreign`] and
    /// [`ReleaseError::Unproven`] (nothing is removed), git, I/O.
    pub fn discard_incomplete_release(
        &mut self,
        options: &ReleaseOptions<'_>,
    ) -> Result<Discarded, ReleaseError> {
        let origin = self.submit_origin.clone();
        self.discard_incomplete_release_as(&origin, options)
    }

    /// [`Session::discard_incomplete_release`] for a submission from `origin` (cycle 41, ruling 1: judged per
    /// submission).
    ///
    /// # Errors
    ///
    /// As [`Session::discard_incomplete_release`], [`ReleaseError::HumanOnly`] judged by `origin`.
    pub fn discard_incomplete_release_as(
        &mut self,
        origin: &Origin,
        options: &ReleaseOptions<'_>,
    ) -> Result<Discarded, ReleaseError> {
        self.settle_guard(origin)?;
        let found = self
            .incomplete_release(options.archive_root)?
            .ok_or(ReleaseError::NoIncomplete)?;
        if let Some(why) = found.foreign {
            return Err(ReleaseError::Foreign(why));
        }
        let evidence = self.run_evidence(&found).map_err(|why| ReleaseError::Unproven {
            why,
            completable: found.refusal.is_none(),
        })?;
        let record_path = self.run_record_path(&found.archive);
        let vfs = self.release_vfs()?;
        let project = self.store().map(|s| s.root().to_path_buf()).unwrap_or_default();
        let mut tag_removed = None;
        let mut tag_kept = None;
        let worktree = options
            .git
            .worktree(&*vfs, &project)
            .map_err(|error| ReleaseError::Io {
                path: project.clone(),
                error,
            })?;
        if worktree.is_some()
            && let Ok(tag) = git::tag_name(&found.label)
        {
            options.git.available().map_err(ReleaseError::Git)?;
            match options
                .git
                .tag_message(&project, &tag)
                .map_err(ReleaseError::Git)?
            {
                Some(message)
                    if tag_carries(&message, &found.sha256) && tag_of_run(&message, &evidence.run) =>
                {
                    options
                        .git
                        .delete_tag(&project, &tag)
                        .map_err(ReleaseError::Git)?;
                    tag_removed = Some(tag);
                }
                Some(_) => tag_kept = Some(tag),
                None => {}
            }
        }
        archive::remove(&*vfs, &found.archive).map_err(|error| ReleaseError::Io {
            path: found.archive.clone(),
            error,
        })?;
        let root = found.archive.parent().map(Path::to_path_buf).unwrap_or_default();
        let owner = self.temp_owner()?;
        for temp in archive::leftover_temps(&*vfs, &root, &found.label, &owner).unwrap_or_default() {
            let _ = archive::remove(&*vfs, &temp);
        }
        self.remove_run_record(record_path.as_deref());
        Ok(Discarded {
            archive: found.archive,
            tag_removed,
            tag_kept,
        })
    }

    /// Completes the incomplete release (doc 05 §14.1 "On failure"): the gate on the saved generation, which must be
    /// the archive's, then steps 3, 5 and 6 with the archive's own facts (`ARCHIVE.json`): the commit (nothing when the
    /// project directory is committed already), the tag when it is missing, the stamp. A consent is needed inside a
    /// worktree, as for [`Session::release`]. A tag it creates carries the mark of this project directory's run record
    /// of the archive, when there is one (so a discard after a crash still removes it); a completed release removes
    /// that record. Completing never writes or removes the archive: a copy of the project directory that completes
    /// another copy's archive stamps only its own revision (its `project/` must equal the copy's saved generation).
    ///
    /// # Errors
    ///
    /// [`ReleaseError`]: [`ReleaseError::NoIncomplete`], [`ReleaseError::Foreign`] (not this project's: it is kept),
    /// [`ReleaseError::NotCompletable`] (discard it then; also when the tag of the label exists but its message does not
    /// carry the archive's SHA-256), the gate's refusals, git, the stamp and its save (the archive stays: it is still
    /// the incomplete release).
    pub fn complete_release(
        &mut self,
        integrity: &[checks::IntegrityItem],
        options: &ReleaseOptions<'_>,
    ) -> Result<Released, ReleaseError> {
        let origin = self.submit_origin.clone();
        self.complete_release_as(&origin, integrity, options)
    }

    /// [`Session::complete_release`] for a submission from `origin` (cycle 41, ruling 1: judged per submission); the
    /// stamp is submitted from `origin`.
    ///
    /// # Errors
    ///
    /// As [`Session::complete_release`], [`ReleaseError::HumanOnly`] judged by `origin`.
    pub fn complete_release_as(
        &mut self,
        origin: &Origin,
        integrity: &[checks::IntegrityItem],
        options: &ReleaseOptions<'_>,
    ) -> Result<Released, ReleaseError> {
        self.settle_guard(origin)?;
        let found = self
            .incomplete_release(options.archive_root)?
            .ok_or(ReleaseError::NoIncomplete)?;
        if let Some(why) = found.foreign {
            return Err(ReleaseError::Foreign(why));
        }
        let evidence = self.run_evidence(&found).ok();
        let record_path = self.run_record_path(&found.archive);
        if let Some(why) = found.refusal {
            return Err(ReleaseError::NotCompletable(why));
        }
        let Some((_, generation, record)) = found.facts else {
            return Err(ReleaseError::NotCompletable(
                "its ARCHIVE.json does not read".to_owned(),
            ));
        };
        let gate = self
            .release_gate(&found.label, integrity)
            .map_err(ReleaseError::Gate)?;
        let blockers: Vec<checks::Finding> = gate.blockers().into_iter().cloned().collect();
        if !blockers.is_empty() {
            return Err(ReleaseError::Blocked(blockers));
        }
        let vfs = self.release_vfs()?;
        let project = self.store().map(|s| s.root().to_path_buf()).unwrap_or_default();
        let worktree = options
            .git
            .worktree(&*vfs, &project)
            .map_err(|error| ReleaseError::Io {
                path: project.clone(),
                error,
            })?;
        let mut git_done = None;
        let mut created_tag = None;
        if let Some(w) = worktree {
            let tag = git::tag_name(&found.label).map_err(ReleaseError::Git)?;
            if !options.git_consent {
                return Err(ReleaseError::GitConsent { worktree: w, tag });
            }
            options.git.available().map_err(ReleaseError::Git)?;
            // An existing tag of the label is reused only when it is this archive's (its message carries the SHA-256);
            // judged before the commit, so that a refusal writes nothing.
            let existing = options
                .git
                .tag_message(&project, &tag)
                .map_err(ReleaseError::Git)?;
            if let Some(message) = &existing
                && !tag_carries(message, &found.sha256)
            {
                return Err(ReleaseError::NotCompletable(format!(
                    "the git tag {tag} exists and names another archive; it is kept"
                )));
            }
            let committed = options
                .git
                .commit_dir(&project, &format!("Release {}", found.label))
                .map_err(ReleaseError::Git)?;
            if existing.is_none() {
                let name = archive::file_name(&found.label);
                let run = evidence.as_ref().map(|r| r.run.as_str());
                options
                    .git
                    .tag(
                        &project,
                        &tag,
                        &tag_message(&found.label, &name, &found.sha256, run),
                    )
                    .map_err(ReleaseError::Tag)?;
                created_tag = Some(tag.clone());
            }
            git_done = Some((committed, tag));
        }
        let mut record = record;
        record.git_tag = git_done.as_ref().and_then(|(_, t)| t.parse().ok());
        let stamped = self.stamp(
            origin,
            &gate,
            &record,
            &found.archive,
            found.sha256,
            git_done.as_ref().map(|(_, t)| t.as_str()),
        );
        let (stamped, save_incomplete) = match stamped {
            Ok(s) => {
                self.remove_run_record(record_path.as_deref());
                s
            }
            Err(e @ ReleaseError::Unsettled(_)) => {
                self.remove_run_record(record_path.as_deref());
                return Err(e);
            }
            Err(e @ ReleaseError::Undecided(_)) => return Err(e),
            Err(e) => {
                if let Some(t) = &created_tag {
                    let _ = options.git.delete_tag(&project, t);
                }
                return Err(e);
            }
        };
        Ok(Released {
            revision: found.revision,
            label: found.label,
            generation,
            stamped,
            archive: found.archive,
            sha256: found.sha256,
            git: git_done,
            save_incomplete,
        })
    }

    /// The git worktree and the tag that completing the incomplete release of `label` would commit and tag in, as
    /// [`Session::complete_release_as`] judges them; `None` outside a git worktree. The window asks for the person's
    /// consent with it before it completes (cycle 41, FIX41b ruling 2: "completing an incomplete release asks, like a
    /// release").
    ///
    /// # Errors
    ///
    /// [`ReleaseError::Gate`] without a project directory, [`ReleaseError::Io`] when the worktree cannot be looked up,
    /// [`ReleaseError::Git`] for a label that is no tag name.
    pub fn settle_git(&self, label: &str, git: &dyn Git) -> Result<Option<(PathBuf, String)>, ReleaseError> {
        let vfs = self.release_vfs()?;
        let project = self.store().map(|s| s.root().to_path_buf()).unwrap_or_default();
        let worktree = git.worktree(&*vfs, &project).map_err(|error| ReleaseError::Io {
            path: project.clone(),
            error,
        })?;
        match worktree {
            None => Ok(None),
            Some(w) => Ok(Some((w, git::tag_name(label).map_err(ReleaseError::Git)?))),
        }
    }

    /// Opens an as-built archive for reproduction (doc 05 §14.3; WP-S1.18 D14, FR-REV-010): the file is read whole
    /// (refused above 2 GiB) and checked as the release checks it (the layout, `SHA256SUMS`, and `project/` against its
    /// own manifest), then `project/` opens read-only from memory (no lock, no journal, no state directory) with the
    /// release context of `ARCHIVE.json`. Exporting its PDF with that context gives the archived PDF's bytes
    /// ([`OpenedArchive::export_pdf`]), and its report rows equal the frozen `reports/` ([`OpenedArchive::derived_reports`]).
    /// `app_version` names the program in the store's diagnostics.
    ///
    /// # Errors
    ///
    /// [`ArchiveOpenError`]: the file, its size, the archive, `ARCHIVE.json`, the project.
    pub fn open_archive(path: &Path, app_version: &str) -> Result<OpenedArchive, ArchiveOpenError> {
        use ew_store::mem::{DirSync, MemVfs};
        let limits = ZipLimits {
            // Bounded by the expanded sizes (2 GiB, 512 MiB per entry); the archive's own text compresses better than
            // 200 : 1, as the release's verification allows.
            max_ratio: u64::MAX,
            ..ZipLimits::default()
        };
        let disk = ew_store::OsVfs;
        let len = disk.stat(path).map_err(ArchiveOpenError::Io)?.len;
        if len > limits.max_total_bytes {
            return Err(ArchiveOpenError::TooLarge { len });
        }
        let bytes = disk.read(path).map_err(ArchiveOpenError::Io)?;
        let entries = archive::read(&bytes, &limits).map_err(ArchiveOpenError::Archive)?;
        let (_, revision, date, generation, _) = entries
            .get(archive::DESCRIPTION_ENTRY)
            .and_then(|b| archive_facts(b))
            .ok_or(ArchiveOpenError::Description)?;
        let label = serde_json::from_slice::<serde_json::Value>(&entries[archive::DESCRIPTION_ENTRY])
            .ok()
            .and_then(|v| Some(v.get("context")?.get("label")?.as_str()?.to_owned()))
            .ok_or(ArchiveOpenError::Description)?;
        let manifest_entry = format!("{}{}", archive::PROJECT_PREFIX, ew_store::ew_format::MANIFEST);
        let manifest_bytes = entries.get(&manifest_entry).ok_or_else(|| {
            ArchiveOpenError::Archive(ArchiveError::Layout(format!("no `{manifest_entry}`")))
        })?;
        let manifest =
            ew_store::ew_format::Manifest::parse(manifest_bytes, &ew_store::ew_format::Limits::default())
                .map_err(|e| {
                    ArchiveOpenError::Archive(ArchiveError::Layout(format!("`{manifest_entry}`: {e:?}")))
                })?;
        archive::check_project(&entries, &manifest, manifest_bytes).map_err(ArchiveOpenError::Archive)?;
        let root = Path::new("/archive");
        let vfs = std::sync::Arc::new(MemVfs::new(DirSync::Explicit));
        vfs.put_dir(root);
        for (name, data) in &entries {
            if let Some(rel) = name.strip_prefix(archive::PROJECT_PREFIX) {
                vfs.put(&root.join(rel), data);
            }
        }
        let mut store = crate::StoreOptions::new(app_version);
        store.mode = ew_store::OpenMode::ReadOnly;
        store.state_dir = None;
        store.vfs = vfs;
        let options = crate::SessionOptions {
            origin: Origin::Cli {
                subcommand: "archive".to_owned(),
                confirmed: false,
            },
            ..crate::SessionOptions::default()
        };
        let (session, report) =
            Session::open(root, store, options, crate::Recovery::Keep).map_err(ArchiveOpenError::Open)?;
        Ok(OpenedArchive {
            session,
            report,
            context: ReleaseContext { label, date },
            generation,
            revision,
            entries,
        })
    }

    /// The guard of settling an incomplete release: a human origin and the write lock.
    fn settle_guard(&self, origin: &Origin) -> Result<(), ReleaseError> {
        human_only(origin)?;
        if !matches!(
            self.store().map(ew_store::Store::access),
            Some(crate::Access::Edit)
        ) {
            return Err(ReleaseError::ReadOnly);
        }
        Ok(())
    }

    /// Refuses `revision.release` outside the release ([`Session::release`], [`Session::complete_release`]): its
    /// parameters are the release's facts. With the exposure error for an origin its exposure refuses, else as rejected.
    pub(crate) fn release_only(&self, name: &str, origin: &Origin) -> Result<(), CmdError> {
        if name != ReleaseRevision::NAME {
            return Ok(());
        }
        let exposure = ReleaseRevision::meta().exposure;
        if !exposure.allows(origin) {
            return Err(CmdError::Exposure {
                command: ReleaseRevision::NAME.into(),
                exposure,
                origin: origin.clone(),
            });
        }
        Err(CmdError::Rejected {
            source: ReleaseRevision::NAME.to_owned(),
            reason: format!(
                "the release's stamp is written only by the release (Revision ▸ Release, ew revision release), after the \
                 gate ({FOUR_EYES_RULE} and the release scheme), the as-built archive and the git tag (WP-S1.18 D13)"
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    //! RA-4 (D13): the freeze of step 2 through the seam of [`Session::release_with`] (no numbering scheme freezes at
    //! release yet, so [`Session::release`] freezes nothing).

    use std::path::{Path, PathBuf};

    use ew_schema::{PartyRole, RevisionState};

    use super::*;
    use crate::{Approval, Recovery, SessionOptions, StoreOptions};

    /// A worktree whose tag step fails (step 5), the rest succeeding; or no worktree at all.
    struct Port {
        worktree: bool,
    }

    impl Git for Port {
        fn worktree(&self, _: &dyn Vfs, _: &Path) -> std::io::Result<Option<PathBuf>> {
            Ok(self.worktree.then(|| PathBuf::from("/")))
        }
        fn available(&self) -> Result<(), GitError> {
            Ok(())
        }
        fn commit_dir(&self, _: &Path, _: &str) -> Result<Committed, GitError> {
            Ok(Committed::Made)
        }
        fn tag_exists(&self, _: &Path, _: &str) -> Result<bool, GitError> {
            Ok(false)
        }
        fn tag_message(&self, _: &Path, _: &str) -> Result<Option<String>, GitError> {
            Ok(None)
        }
        fn tag(&self, _: &Path, _: &str, _: &str) -> Result<(), GitError> {
            Err(GitError::Failed {
                step: "tag",
                code: Some(128),
            })
        }
        fn delete_tag(&self, _: &Path, _: &str) -> Result<(), GitError> {
            Ok(())
        }
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("a directory");
        for e in std::fs::read_dir(from).expect("a readable directory") {
            let e = e.expect("an entry");
            let target = to.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), &target).expect("a copied file");
            }
        }
    }

    fn cmd(name: &str, params: serde_json::Value) -> AnyCommand {
        AnyCommand {
            name: name.into(),
            params,
        }
    }

    fn state(s: &Session) -> RevisionState {
        s.query()
            .table::<Revision>()
            .iter()
            .find(|(_, r)| r.label.as_str() == "V1.0")
            .map(|(_, r)| r.state.clone())
            .expect("revision V1.0")
    }

    /// FIX39a (finding 6): every pack a gate can evaluate (the pack of every registered rule) has an ID and a version
    /// that parse as an `Ident`, so [`Session::release_record`]'s `parse().ok()` never drops a pack from the record
    /// (and the approval's `ident()` never refuses one).
    #[test]
    fn every_gate_pack_id_and_version_is_an_ident() {
        let mut packs: Vec<&str> = checks::rules().iter().map(|r| r.meta().pack).collect();
        packs.sort_unstable();
        packs.dedup();
        assert!(!packs.is_empty());
        for pack in packs {
            assert!(pack.parse::<Ident>().is_ok(), "pack ID {pack:?}");
        }
        assert!(
            checks::SHIPPED_PACK_VERSION.parse::<Ident>().is_ok(),
            "pack version {:?}",
            checks::SHIPPED_PACK_VERSION
        );
    }

    /// RA-4: a freeze that changes a page digest (a moved symbol) is undone and aborts the release before anything is
    /// written; a freeze that changes no page (the drawing settings of a project without crossings) is saved as g and
    /// stays an ordinary undoable transaction when a later step fails (a refused tag); then the release completes at the
    /// generation after the freeze.
    #[test]
    fn ra4_a_freeze_that_changes_a_page_digest_aborts_the_release() {
        let base = ew_testdir::fresh_dir("ew-release-ra4");
        let dir = base.join("release-min");
        let root = base.join("archives");
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/fixtures/release-min"),
            &dir,
        );
        let mut store = StoreOptions::new("wp-s1-18-part4");
        store.state_dir = Some(base.join("state"));
        let (mut s, report) =
            Session::open(&dir, store, SessionOptions::default(), Recovery::Keep).expect("the fixture opens");
        let integrity = crate::integrity_items(&report);
        let approval = Approval {
            label: "V1.0".into(),
            checker: PartyRole::Checker,
            date: Date::new(2026, 3, 2).expect("a date"),
            waivers_ack: Vec::new(),
        };
        s.approve(&approval, &integrity).expect("approved");
        s.save().expect("saved");
        let (signed, g) = s.content_digest().expect("saved");
        let ctx = ReleaseContext {
            label: "V1.0".into(),
            date: Date::new(2026, 3, 9).expect("a date"),
        };
        let no_git = Port { worktree: false };
        let options = ReleaseOptions {
            archive_root: Some(&root),
            git: &no_git,
            git_consent: true,
            parallelism: Parallelism::Serial,
        };

        let q = s.query();
        let rep = q.reps().ids().next().expect("a representation");
        let page = q.reps().get(rep).expect("the rep").page;
        let step = i64::try_from(q.pages().get(page).expect("the page").grid.num()).expect("a step");
        drop(q);
        let moved = [cmd(
            "rep.move",
            serde_json::json!({"reps": [rep.to_string()], "delta": [2 * step, 0]}),
        )];
        match s.release_with(&Origin::Gui, &ctx, &integrity, &options, &moved) {
            Err(ReleaseError::FreezeChangesPages { pages }) => assert_eq!(pages, [page]),
            other => panic!("expected FreezeChangesPages, got {other:?}"),
        }
        assert!(!s.is_dirty(), "the freeze is undone");
        assert_eq!(s.content_digest().expect("saved"), (signed, g));
        assert_eq!(state(&s), RevisionState::Open);
        assert!(!root.exists(), "nothing written");

        let hops = [cmd(
            "project.drawing.set",
            serde_json::json!({"crossings": "hops"}),
        )];
        let failing = Port { worktree: true };
        let with_tag = ReleaseOptions {
            git: &failing,
            ..options
        };
        assert!(matches!(
            s.release_with(&Origin::Gui, &ctx, &integrity, &with_tag, &hops),
            Err(ReleaseError::Tag(_))
        ));
        assert_eq!(state(&s), RevisionState::Open);
        let (frozen, g1) = s.content_digest().expect("the freeze is saved");
        assert_eq!(g1, g + 1);
        assert_ne!(frozen, signed);
        assert_eq!(std::fs::read_dir(&root).expect("the root").count(), 0);
        s.undo(&Origin::Gui).expect("the freeze stays undoable");
        assert!(s.is_dirty());
        s.save().expect("saved");
        assert_eq!(s.content_digest().expect("saved").0, signed);

        let released = s
            .release_with(&Origin::Gui, &ctx, &integrity, &options, &hops)
            .expect("released");
        assert_eq!(released.generation, g + 3, "the freeze's save is generation g");
        let record = s
            .query()
            .table::<Revision>()
            .get(released.revision)
            .and_then(|r| r.release.clone())
            .expect("a record");
        assert_eq!(record.generation, Some(released.generation));
        assert_ne!(record.content_digest, Some(signed), "the digest after the freeze");
        assert_eq!(state(&s), RevisionState::Released);
        s.close();
        let _ = std::fs::remove_dir_all(&base);
    }
}
