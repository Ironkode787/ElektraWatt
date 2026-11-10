// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Session` facade shared by every front-end: registers the SQL and plugin rule implementations at
//! session start and composes `.ewpp` export jobs.
//!
//! v0 (WP-S1.04) is the editing facade of ARCH §5.1: [`Session`] owns one project model and hands commands
//! to the commit pipeline of `ew-commands` ([`ew_commands::Engine`]): `execute` for interactive commands,
//! `preview` and `commit` for bulk ones, `undo`, `redo`, the history and the change feed. Front-ends set
//! the origin of their transactions; the exposure check, the normalisers and validation live in
//! `ew-commands`, so no front-end can bypass them.
//!
//! **Persistence** (WP-S1.09). [`Session::open`] opens a project directory through `ew-store` (lock,
//! roll-forward, verification, integrity findings, leftover journal); [`Session::create`] writes the
//! session's model as a new project. With a store attached, every committed transaction (execute, commit,
//! undo, redo) is journalled as its field-level diff (doc 05 §8.1), its touched keys join the dirty set,
//! and [`Session::save`] writes only the files of that set (doc 05 §5). [`Session::close`] is the clean
//! close; dropping a session releases the lock and keeps the journal for recovery.
//!
//! **Command logs** (doc 17 §3.5). [`Session::run_script`] replays a parsed [`CmdLog`] (`.ewcmd`, JSON Lines):
//! one transaction per line with origin [`Origin::CmdLog`], so one [`Session::undo`] takes back one line.
//!
//! **Page output** (FT-59, [`output`]). [`Session::export_pdf`] and [`Session::export_svg`] run the export job: the
//! page views and one memoised `page_display_list` family per layer through the session's memo store, the page
//! phase on `ew_export::par_map`, then the PDF or SVG writer with the bundled fonts.
//!
//! The PDF export also runs as a job ([`Session::export_job`], WP-S2.09 part 1): a snapshot with a cold `Q` on a
//! thread of [`ew_jobs`], with progress and cooperative cancel.
//!
//! **The release gate and the sign-off** (WP-S1.18 part 3). [`Session::release_gate`] runs the gate on the saved
//! generation with its content digest; [`Session::approve`] records the checker's sign-off (`revision.approve`, which
//! only it submits).
//!
//! **The release and the as-built archive** (WP-S1.18 part 4). [`Session::release`] runs the six steps of doc 05 §14.1
//! (gate, freeze, git commit, archive, tag, stamp) atomically; [`Session::incomplete_release`] finds the archive a
//! crashed release left, which [`Session::complete_release`] or [`Session::discard_incomplete_release`] settles.
//!
//! Not in v0: the memo snapshot in [`World`] and the job pool for previews (WP-S1.05, FT-497), rule registration
//! (WP-S1.15), the save on a writer thread, and the undo history of a recovered session (FR-COL-028: a recovered
//! session opens dirty with an empty history).
//!
//! **Derivation findings.** What the derivation layer could not do as the project asks (an unknown pinned
//! derivation profile, a numbering scheme the command layer does not number, the built-in default frame, a
//! malformed provisional settings member) reaches the front-end as a [`DerivationFinding`]:
//! [`Session::load_findings`] holds the project-level ones found when the session opened (ADR-0003: an
//! unknown profile is never silent; it carries `EW-CORE-018` at error severity), and
//! [`Session::derivation_findings`] runs a full pass over the current snapshot on demand.
//!
//! # Example
//!
//! ```
//! use ew_api::{AnyCommand, Session, SessionOptions};
//!
//! let project = serde_json::from_str(
//!     r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
//! )
//! .unwrap();
//! let mut session = Session::from_records(project, [], [], SessionOptions::default()).unwrap();
//!
//! let cmd = AnyCommand {
//!     name: "page.create".into(),
//!     params: serde_json::json!({"number": 1, "title": "Cover"}),
//! };
//! let receipt = session.execute(&cmd).unwrap();
//! assert_eq!(receipt.rev, session.world().rev);
//! assert_eq!(session.drain_changes().len(), 1);
//!
//! session.undo(&ew_api::Origin::Gui).unwrap();
//! assert!(!session.is_dirty());
//! ```

#![forbid(unsafe_code)]

/// The keyed [`Text`] of a refusal of `ew-api`'s own (FT-1421), as `ew-commands`' own `text!` gives one: a key that
/// [`TEXTS`] lacks, or a wrong number of arguments, fails the build.
macro_rules! api_text {
    ($k:literal $(, $a:expr)* $(,)?) => {
        $crate::Text::checked_by_macro(
            const {
                let Some(template) = ew_commands::text_english($k) else {
                    panic!(concat!("unknown text key: ", $k));
                };
                assert!(
                    ew_commands::text_arity(template) == <[&str]>::len(&[$(stringify!($a)),*]),
                    concat!("the wrong number of arguments for ", $k)
                );
                $k
            },
            vec![$($crate::TextArg::from($a)),*],
        )
    };
}

// WP-S1.19 part 4: the CSV and XLSX parts import with its column maps (D11).
pub mod catalog;
// WP-S1.22 part 1: the JSON results of the `ew` subcommands and their published schemas (D3, D5).
pub mod cli_results;
mod cmdlog;
// WP-S1.17 part 6: external change (D20 to D23)
mod external_change;
pub mod fmt;
pub mod geometry;
pub mod import;
mod library_ops;
mod macros;
mod new_project;
pub mod output;
mod part_editor;
mod part_offers;
pub mod symbols;
// WP-S1.22 part 4: the public queries of `ew query` and `--explain` (D11, D12).
pub mod queries;
// WP-S1.18 part 3: the release gate and the sign-off (D8 to D10).
mod release;
// WP-S1.18 part 4: the atomic release and the as-built archive (D11 to D13).
mod release_run;
// WP-S2.04 part 2: the router the canvas tools call (§4.3, §8.1).
pub mod routing;
// WP-S1.17 part 2: `.ewz` opened read-only and the export of the saved generation (D8, D9).
mod transport;
pub mod verify;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use ew_derive::{
    DeviceTag, FrameGridOf, FrameSettings, NumberingSettingsOf, Oracle, PageOrder, Q, ReportPagesProvider,
};
use ew_model::{AnyKey, Model, ModelError, PreventHook};
use ew_schema::{LibDocument, Project, Record};
use ew_store::{Changes, LockState, SaveReport, Store};

pub use cmdlog::{CmdLog, CmdLogError, SCHEMA as CMDLOG_SCHEMA, ScriptDiff};
pub use ew_commands::commands::{ChangeClass, CopyDiff, FieldChange, classify_update};
pub use library_ops::{
    ApiError, AssignMode, CopyState, CopyStatus, CopyUpdate, CpMap, UpdatePlan, VersionConflict,
    unreadable_profiles,
};
pub use macros::{
    InsertAt, MacroConflict, MacroError, MacroScope, MacroSpec, MacroValues, SavedMacro, macro_item_id,
};
pub use part_editor::{DRAFT_FIELDS, DraftMessage, PART_KINDS, PartDraft, PartDraftError, PartEditPlan};
pub use part_offers::{Fit, OfferView, PartOffer, PartOfferQuery, PartPick, PartTarget};

/// The reference of an image file as the asset store makes it ([`sniff_asset`] and the SHA-256 of the bytes): what a
/// front-end reads from a file the user picked or dropped before it asks `item.place` to store it (WP-S2.04 part 5).
/// It is a gate that brings an image in, so it applies the pixel limit ([`ew_schema::pixel_limit`], ID-1417).
///
/// # Errors
///
/// The [`AssetError`] of bytes that are no SVG, PNG or JPEG file, or exceed a limit.
pub fn asset_ref_of(bytes: &[u8]) -> Result<AssetRef, AssetError> {
    use sha2::Digest;
    let (format, size) = ew_schema::sniff(bytes)?;
    ew_schema::pixel_limit(format, size)?;
    Ok(AssetRef {
        sha256: Sha256(sha2::Sha256::digest(bytes).into()),
        format,
        size,
    })
}

/// An image of the project that no drawing shows ([`Session::undrawn_images`], decision log ID-1417).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UndrawnImage {
    /// Whether it is the company logo.
    pub logo: bool,
    /// The folios of the pages whose image items show it, in page order.
    pub folios: Vec<String>,
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
}

/// The check engine (`ew check`, the message panel, the migration report): rules, schemes, findings.
pub use ew_checks as checks;
pub use ew_derive::{BomMode, DeriveNote, MemoStore, Mismatch, NoteSeverity};
/// The job runner (WP-S2.09 part 1, D3): the shell names it through `ew-api` (no workspace edge of its own).
pub use ew_jobs;
/// The libraries (WP-S1.19 part 2): tiers, the `.ewlib` reader and writer, the index and search. The shell and the
/// CLI name them through `ew-api` (no workspace edge of their own).
pub use ew_library as library;
/// NFC normalisation (the form every `Ident` and text key takes): the shell names it through `ew-api` (no workspace
/// edge of its own).
pub use ew_mltext::to_nfc;
/// The row sources of `ew export cables|bom` (doc 17 §3.3): what to export and why it could not.
pub use ew_reports::export::{Export, ExportError, Exported};
/// The project's decimal separator (`settings.formatting.decimal_separator`, FT-366): the shell writes millimetres
/// with it.
pub use ew_schema::DecimalSeparator;
/// The format this version writes, and the migration chain a project of an older format runs through at open
/// (WP-S1.17 D10, D11): `ew upgrade` names the format, and a test passes its own chain in [`StoreOptions`].
pub use ew_schema::FORMAT;
/// The project's revision label format (doc 15 §5.2, FR-REV-002): the new-project wizard checks the first label
/// against it, as `revision.create` does (EW-REL-004).
pub use ew_schema::LabelFormat;
/// The party roles of a revision's author and checker (doc 03 `Party`): `ew revision approve --as <role>` names one.
pub use ew_schema::PartyRole;
/// The text styles the built-in basic project of a new project writes (ADR-0032 decision 3): the wizard's readable
/// choice of "Pin numbers and markers" (WP-S7.03 D11).
pub use ew_schema::new_project_text_styles;
/// Assets (doc 05 §13.3): the reference a record stores, the sniffer and the base64 form of command parameters.
pub use ew_schema::{
    AssetError, AssetFormat, AssetRef, MAX_LOGO_BYTES, Sha256, SvgHazard, base64_encode, pixel_limit,
    sniff as sniff_asset,
};
/// The conflict records a merge leaves (doc 05 §11.2; WP-S1.17 D17): `ew merge resolve` selects them by kind and member.
pub use ew_schema::{Conflict, ConflictBody, FILE_CONFLICT_KIND};
/// The read-only SQL projection (`ew sql`, the acceptance SQL, ADR-0017): views, sandbox, results.
pub use ew_sqlview as sql;
/// The three-way merge of one project file that `ew merge-driver` runs (WP-S1.17 D15), with the reader's limits, the
/// shipped migration chain and the manifest reader that gives ours its format.
pub use ew_store::ew_format::{
    FileMerge, Limits, MIGRATIONS, Manifest, MergeSide, holds_newer_records, merge_file,
};
pub use ew_store::ew_format::{FormatStep, MigrationError, Migrations};
/// The summary of a crashed session's journal ([`OpenReport::recovery`]) and why a replay was refused
/// ([`Session::recovery_refusal`]): the CLI's `ew recover` and the shell name them through `ew-api`.
pub use ew_store::journal::{Collision, RecoveryInfo, ReplayError};
pub use ew_store::{
    Access, Clock, OpenError, OpenMode, OpenReport, OsVfs, PurgeError, PurgeFile, PurgePlan, ReadOnlyReason,
    RecoverError, SaveError, SaveStep, StoreOptions, SystemClock, Vfs,
};
/// The as-built archive's layout and atomic write, and the git port of a release (doc 05 §14; WP-S1.18 D11 to D13):
/// the CLI and the shell name them through `ew-api` (no workspace edge of their own); likewise `.ewz` transport
/// (doc 05 §12; WP-S1.17 D8, D9).
pub use ew_store::{archive, cloud, ewz, external, git, zip};
pub use external_change::{DiskPoll, ExternalApplied, ExternalError};
pub use new_project::{NewProjectKind, ProjectTemplate, SaveAsBasicError};
pub use release::{Approval, ApproveError, FOUR_EYES_RULE, Gate, GateError, role_name};
pub use release_run::{
    ArchiveOpenError, Discarded, IncompleteRelease, OpenedArchive, ReleaseContext, ReleaseError,
    ReleaseOptions, ReleasePlan, ReleaseTrouble, Released, TroubleKind,
};
pub use transport::{EwzExport, ExportEwzError};

pub use ew_commands::{
    AnyCommand, Change, ChangeKind, CmdError, CommandRegistry, DerivedChange, DerivedDiff, DynCommand,
    EntityChange, History, HistoryLimits, IdSource, ModelDiff, Origin, Preview, Receipt, Restored, Submit,
    World,
};
/// A command's registered metadata, its cost and its exposure class (WP-S2.05 §4.2): the property grid's guard that
/// builds only `Edit` commands and its edit policy read them through [`Session::command_meta`].
pub use ew_commands::{CommandMeta, Cost, Exposure, HistoryBlock};
/// A command's refusal by key (FT-1337): the window renders it in its language, the CLI prints its English.
pub use ew_commands::{
    TEXTS, Text, TextArg, asset_error_text, comma_joined, ident_error_text, path_error_text,
    schema_error_text, serde_error_text,
};
/// The types of [`IdSource`]'s methods, so that a test can supply its own source of new IDs
/// ([`SessionOptions::ids`]).
pub use ew_derive::ew_ids::{EntropyError, UnixTime};

use ew_commands::commands::PurgeAssets;
use ew_commands::{Command as _, Engine, EngineOptions};

/// Options of a session.
pub struct SessionOptions {
    /// The origin of the transactions this front-end submits (GUI, CLI subcommand, MCP client, …).
    pub origin: Origin,
    /// The bounds of the undo stack.
    pub history: HistoryLimits,
    /// The source of new IDs; the system clock and OS entropy when `None`. Tests pass a seeded one.
    pub ids: Option<Box<dyn IdSource + Send>>,
    /// The prevent-rule hook; `None` installs the shipped prevent rules of `ew-checks` ([`ShippedPrevent`]).
    /// Tests that need an unchecked engine pass their own hook.
    pub prevent: Option<Box<dyn PreventHook>>,
    /// Whether [`Session::query`] memoises across requests (ADR-0002 amendment 4), and with oracle mode.
    pub memo: MemoMode,
    /// The library tiers [`Session::libraries`] opens (WP-S1.19 part 2); `None`: the compiled-in starter only, with
    /// an in-memory index and no file read (tests, `ew apply`). The GUI and the CLI pass
    /// [`library::LibraryTiers::for_user`].
    pub libraries: Option<library::LibraryTiers>,
}

/// How a session's derived values are evaluated (ADR-0002 decisions 2–3, amendment 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MemoMode {
    /// Every [`Session::query`] is cold (request-scoped cache only).
    Off,
    /// The `Memo` families are memoised in the session's [`MemoStore`], advanced with push
    /// invalidation after every commit.
    #[default]
    On,
    /// As `On`, and every memoised read is also computed cold and compared (CI, debug sessions); read the
    /// mismatches with [`Session::take_oracle_mismatches`].
    Oracle,
}

impl Default for SessionOptions {
    fn default() -> SessionOptions {
        SessionOptions {
            origin: Origin::Gui,
            history: HistoryLimits::default(),
            ids: None,
            prevent: None,
            memo: MemoMode::default(),
            libraries: None,
        }
    }
}

impl std::fmt::Debug for SessionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionOptions")
            .field("origin", &self.origin)
            .field("history", &self.history)
            .field("memo", &self.memo)
            .field("libraries", &self.libraries)
            .finish_non_exhaustive()
    }
}

/// One finding of the derivation layer, from a [`DeriveNote`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DerivationFinding {
    /// The catalogue code where one is assigned (`EW-CORE-018` for an unknown derivation profile).
    pub code: Option<&'static str>,
    /// The severity.
    pub severity: NoteSeverity,
    /// What happened, in English (Fluent keys arrive with the message-list UI).
    pub message: String,
    /// The note itself.
    pub note: DeriveNote,
}

impl From<DeriveNote> for DerivationFinding {
    fn from(note: DeriveNote) -> DerivationFinding {
        DerivationFinding {
            code: note.code(),
            severity: note.severity(),
            message: note.to_string(),
            note,
        }
    }
}

/// The project-level derivation notes of a snapshot: the pinned profile and the provisional settings
/// members, each parsed once (cheap: no device or page is derived).
fn project_notes(model: &Model) -> Vec<DerivationFinding> {
    let mut q = Q::new(model);
    let _ = q.get::<NumberingSettingsOf>(());
    let _ = q.get::<FrameSettings>(());
    // WP-S2.04 §10.1: a wire marker of an older file that lists a deleted connection, or one of another page, is
    // reported, not refused.
    let stale = ew_derive::stale_marker_notes(&q);
    q.diagnostics()
        .notes()
        .into_iter()
        .chain(stale)
        .map(DerivationFinding::from)
        .collect()
}

/// The shipped prevent rules of `ew-checks` as the model's commit validator (ADR-0009 decision 3, doc 15
/// §3.5): every session installs it unless [`SessionOptions::prevent`] names another hook.
///
/// It builds cold tracked-read contexts over the base and the candidate snapshot and asks
/// [`checks::prevent`] for the violations the batch creates; the first one (in finding order) rejects the
/// batch as `ModelError::Prevented`, which the command layer reports as [`CmdError::Prevented`] with the
/// rule ID and the message (for EW-TERM-002 the duplicate set and the remedy). Interactive and scripted edits
/// are validated strictly whatever the project's strictness (FR-CHK-011).
#[derive(Clone, Copy, Debug, Default)]
pub struct ShippedPrevent;

impl PreventHook for ShippedPrevent {
    fn check(
        &self,
        base: &Model,
        candidate: &Model,
        touched: &[AnyKey],
    ) -> Result<(), ew_model::PreventViolation> {
        let keys: Vec<(ew_schema::Kind, ew_schema::AnyId)> = touched.iter().map(|k| (k.0, k.1)).collect();
        let violations = checks::prevent(&Q::new(base), &Q::new(candidate), &keys);
        match violations.into_iter().next() {
            None => Ok(()),
            Some(f) => Err(ew_model::PreventViolation {
                rule: f.rule.to_owned(),
                message: f.message,
                keys: f.targets.iter().map(|t| (t.kind, t.id)).collect(),
                // FT-1488: the window renders the rule's message from them in its language.
                args: f.args,
            }),
        }
    }
}

/// The integrity findings of opening a project as items for [`checks::CheckReport::with_integrity`]: every
/// store finding with a catalogue ID (EW-CORE-001 … 019 but 011, a registered rule over the model; doc 05 §6.3) at its
/// severity, located by its project-relative file and line. Notices without an ID (line endings, foreign edits) are
/// left out.
pub fn integrity_items(report: &OpenReport) -> Vec<checks::IntegrityItem> {
    use ew_store::ew_format::Severity as S;
    report
        .findings
        .iter()
        .filter_map(|f| {
            let rule = f.kind.code()?;
            let location = match (&f.file, f.line) {
                (Some(file), Some(line)) => format!("{file}:{line}"),
                (Some(file), None) => file.to_string(),
                (None, _) => String::new(),
            };
            Some(checks::IntegrityItem {
                rule: rule.to_owned(),
                severity: match f.kind.severity() {
                    S::Error => checks::Severity::Error,
                    S::Warning => checks::Severity::Warning,
                    S::Info => checks::Severity::Info,
                },
                location,
                message: f.message.clone(),
            })
        })
        .collect()
}

/// The library copies the open dropped as damaged (FT-1500): a loader finding names the copy's file and the model
/// lacks the copy. In key order.
fn dropped_copies(q: &Q, report: &OpenReport) -> Vec<AnyKey> {
    use ew_schema::{Kind, LibCopy, MacroDef};
    use ew_store::ew_format::{PathClass, classify, lib_dir};
    let held = |&(kind, id): &AnyKey| match kind {
        Kind::Part => q.parts().contains(id.cast()),
        Kind::Symbol => q.symbols().contains(id.cast()),
        Kind::Fndef => q.fndefs().contains(id.cast()),
        Kind::Form => q.forms().contains(id.cast()),
        Kind::Macro => q.table::<LibCopy<MacroDef>>().contains(id.cast()),
        Kind::Manufacturer => q.manufacturers().contains(id.cast()),
        Kind::PlcProfile => q.plc_profiles().contains(id.cast()),
        _ => true,
    };
    let mut dropped: Vec<AnyKey> = report
        .findings
        .iter()
        .filter_map(|f| match classify(f.file.as_ref()?.as_str()) {
            PathClass::LibDocument { dir, id } => Kind::ALL
                .into_iter()
                .find(|k| k.is_library() && lib_dir(*k) == Some(dir))
                .map(|k| (k, id)),
            _ => None,
        })
        .filter(|key| !held(key))
        .collect();
    dropped.sort();
    dropped.dedup();
    dropped
}

/// A library copy the open dropped as damaged and the device that names it ([`Session::damaged_copy`], FT-1383).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DamagedCopy {
    /// The tag of the device that names the copy (`-101K1`); `None` for a record of no device.
    pub device: Option<String>,
    /// The copy's file, relative to the project folder (`lib/parts/<id>.json`): the finding's detail, never a user
    /// text's (FT-1396).
    pub file: String,
    /// The part the copy holds, as an installed library gives it: its part number and the library's name; `None` when
    /// no installed library holds it, or the copy is no part's.
    pub part: Option<(String, String)>,
    /// The copy in words, keyed for the reader's language (FT-1500, ID-1458): its kind word and its own name or key as
    /// an installed library gives them, else its kind, and the record that names it ("the library copy of form A3
    /// named by page 103").
    pub words: Text,
    /// Whether no record names the copy and the model passes its integrity check (FT-1500): only another library
    /// copy's link names it, or nothing does, so a Save As makes a project without it rather than being refused.
    pub left_out: bool,
    /// When [`DamagedCopy::left_out`]: every copy the open dropped that a Save As leaves out, each in words as
    /// [`DamagedCopy::words`] names the first, in key order (C53-p3#6, ID-1481); empty otherwise.
    pub left_out_words: Vec<Text>,
    /// Whether an installed library holds a readable document of the copy the banner names (FT-1499, ID-1486; of any
    /// copy the open dropped when [`DamagedCopy::left_out`], whose banner names them all, C53-p4#5): the banner then
    /// offers Update from library ([`Session::update_damaged_copies`]).
    pub in_library: bool,
}

/// What [`Session::update_damaged_copies`] did (FT-1499, ID-1486): the damaged library copies written from their
/// libraries and those no installed library holds a readable document of, each in words as
/// [`DamagedCopy::words`] names a copy, in key order. Both empty for a project without a damaged copy.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CopiesUpdated {
    /// The copies written from their libraries, each with the version written ("… at version 0.1.1", C53-p4#4).
    pub updated: Vec<Text>,
    /// The copies left damaged.
    pub not_updated: Vec<Text>,
}

/// What [`Session::open`] does with a leftover journal of a crashed session (doc 05 §8.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Recovery {
    /// Keep it and report it ([`OpenReport::recovery`]): the caller asks the user. "Open without
    /// recovering" keeps the journal for 30 days.
    #[default]
    Keep,
    /// Replay it: the session opens dirty with the recovered transactions.
    Apply,
    /// Delete it.
    Discard,
}

/// The project directory a session saves to.
struct Persistence {
    store: Store,
    /// The keys put since the last save: the dirty set of the next save (doc 05 §5.1).
    unsaved: BTreeSet<AnyKey>,
    /// Recovered changes that the history does not know about.
    recovered: bool,
    /// Why [`Recovery::Apply`] did not replay the crashed session's journal (collisions, an unreadable journal); the
    /// journal is kept and the disk state is open.
    refusal: Option<RecoverError>,
    /// Whether the user consented to converting a project migrated from an older format ([`Session::confirm_upgrade`]):
    /// in memory only, so a session closed before its save leaves the next open asking again (WP-S1.17 D11).
    consented: bool,
    /// The upgrade's backup, once written ([`Session::upgrade_backup`]).
    backup: Option<std::path::PathBuf>,
    /// The external-change state: the held change, the cloud rechecks (WP-S1.17 D20 to D22).
    watch: external_change::Watch,
    /// The library copies the open dropped as damaged (a loader finding names the file, the model lacks the copy), in
    /// key order (FT-1500); none for a project Save As wrote.
    dropped: Vec<AnyKey>,
}

/// The run of `ew check --integrity` ([`Session::check_integrity`]): every rule it is given, nothing switched off, the
/// MVP content; its report names it `integrity`.
const INTEGRITY_RUN: checks::Scheme = checks::Scheme {
    name: "integrity",
    purpose: "project integrity; ew check --integrity",
    off: &[],
    lenient: false,
    content: checks::Release::Mvp,
};

/// A purge refusal by key (FT-1421), which the window renders in its language; its English is the store's own text,
/// [`PurgeError`]'s.
pub fn purge_refusal(e: &PurgeError) -> Text {
    match e {
        PurgeError::ReadOnly(_) => api_text!("refusal-purge-read-only"),
        PurgeError::Quarantined(files) => api_text!(
            "refusal-purge-quarantined",
            files.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(", ")
        ),
        PurgeError::Unsaved => api_text!("refusal-purge-unsaved"),
        PurgeError::Recovery => api_text!("refusal-purge-recovery"),
        PurgeError::Incomplete(_) => api_text!("refusal-purge-incomplete"),
        PurgeError::ExternalChange => api_text!("refusal-purge-external"),
        // C52-p5#5: one image has its own form.
        PurgeError::Stale(hashes) if hashes.len() == 1 => api_text!("refusal-purge-stale-one"),
        PurgeError::Stale(hashes) => api_text!("refusal-purge-stale", hashes.len().to_string()),
        PurgeError::Io { .. } => api_text!("refusal-purge-unreadable"),
        PurgeError::Save { error, .. } => api_text!("refusal-purge-save", error.to_string()),
        PurgeError::HumanOnly => api_text!("refusal-purge-human-only"),
    }
}

/// The most names an upgrade's backup tries: `.pre-<format>`, then `-2` up to this.
const UPGRADE_BACKUP_NAMES: u32 = 1_000;

/// Writes the backup of a project migrated from the older format `from` (WP-S1.17 D11, decision log ID-1221 ruling 5):
/// the generation as found ([`ewz::pack_as_found`], its manifest's bytes as read) at `<project folder>.pre-<from>.ewz`
/// beside the project folder, read back and verified ([`fmt::write_backup`]); past a name that holds another file,
/// at `.pre-<from>-2.ewz`, `-3` and so on; a name that holds these bytes is reused. Its path.
///
/// # Errors
///
/// [`SaveError::Failed`] at [`SaveStep::Backup`]: the generation cannot be packed, the backup cannot be written, or
/// every name is taken. Nothing of the project was written, and no backup is left.
fn write_upgrade_backup(store: &Store, from: &str) -> Result<std::path::PathBuf, SaveError> {
    let failed = |path: std::path::PathBuf, error: std::io::Error| SaveError::Failed {
        step: SaveStep::Backup,
        path,
        error,
    };
    let other = |path: &Path, e: &dyn std::fmt::Display| {
        failed(path.to_path_buf(), std::io::Error::other(e.to_string()))
    };
    let found = ewz::pack_as_found(store).map_err(|e| match e {
        ewz::PackError::Generation(ew_store::GenerationError::Io { path, error })
        | ewz::PackError::Io { path, error } => failed(path, error),
        e => other(store.root(), &e),
    })?;
    let vfs = &**store.vfs();
    let mut first = None;
    for n in 1..=UPGRADE_BACKUP_NAMES {
        let infix = if n == 1 {
            format!(".pre-{from}")
        } else {
            format!(".pre-{from}-{n}")
        };
        let target = fmt::beside(vfs, store.root(), &infix).map_err(|e| other(store.root(), &e))?;
        match fmt::write_backup(vfs, &target, &found.packed.bytes) {
            Ok(_) => return Ok(target),
            Err(fmt::RepairError::BackupTaken(_)) => {
                first.get_or_insert(target);
            }
            Err(fmt::RepairError::Backup { path, error }) => return Err(failed(path, error)),
            Err(e) => return Err(other(&target, &e)),
        }
    }
    let path = first.unwrap_or_else(|| store.root().to_path_buf());
    Err(failed(
        path,
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("the first {UPGRADE_BACKUP_NAMES} names of the backup hold other files"),
        ),
    ))
}

/// One open project: the facade every front-end (GUI, `ew` CLI, scripts, MCP) talks to.
///
/// # Threads
///
/// A `Session` is not `Send` (nor `Sync`): its memo store ([`MemoStore`], [`MemoMode::On`] by default) shares
/// the `ew-memo` store with the `Q`s it hands out through an `Rc` and `RefCell`s. The thread that owns the
/// session, the UI thread (ARCH §2), drives it; work on other threads takes a snapshot ([`Session::world`],
/// O(1)) and derives from it with a cold `Q`. The intent is checked at compile time:
///
/// ```compile_fail
/// fn must_be_send<T: Send>() {}
/// must_be_send::<ew_api::Session>();
/// ```
pub struct Session {
    engine: Engine,
    load_findings: Vec<DerivationFinding>,
    /// The report engine installed on every `Q` the session builds (WP-S3.03).
    reports: Arc<dyn ReportPagesProvider>,
    /// The memo store of [`Session::query`] (`None`: [`MemoMode::Off`]).
    memo: Option<MemoStore>,
    persistence: Option<Persistence>,
    /// Change-feed entries taken from the engine for the journal, not yet drained by the front-end.
    changes: Vec<Change>,
    /// The origin's name for the journal.
    origin: String,
    /// The origin of the session's own submissions (the exposure of `project.assets.purge` is judged by it).
    submit_origin: Origin,
    /// The tiers of [`SessionOptions::libraries`], opened at the first [`Session::libraries`].
    library_tiers: Option<library::LibraryTiers>,
    /// The opened libraries, with the error that made [`Session::libraries`] fall back to the starter.
    libraries: std::sync::OnceLock<(library::Registry, Option<library::LibraryError>)>,
    /// The release's housekeeping trouble not yet taken by the front-end ([`Session::take_release_troubles`]).
    release_troubles: std::cell::RefCell<Vec<release_run::ReleaseTrouble>>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("engine", &self.engine)
            .field("load_findings", &self.load_findings)
            .finish_non_exhaustive()
    }
}

impl Session {
    /// A session over a loaded model.
    pub fn new(model: Model, options: SessionOptions) -> Session {
        let load_findings = project_notes(&model);
        // `Gui`, `Cli { .. }` give `gui`, `cli`: the variant name, for the journal's `origin`.
        let origin = format!("{:?}", options.origin)
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        let reports = ew_reports::engine();
        let memo = match options.memo {
            MemoMode::Off => None,
            mode => {
                let store = MemoStore::new(&model).with_report_pages(Arc::clone(&reports));
                if mode == MemoMode::Oracle {
                    store.set_oracle(Oracle::Every);
                }
                Some(store)
            }
        };
        Session {
            library_tiers: options.libraries,
            libraries: std::sync::OnceLock::new(),
            release_troubles: std::cell::RefCell::new(Vec::new()),
            submit_origin: options.origin.clone(),
            load_findings,
            reports,
            memo,
            persistence: None,
            changes: Vec::new(),
            origin,
            engine: Engine::new(
                model,
                EngineOptions {
                    history: options.history,
                    origin: options.origin,
                    ids: options.ids,
                    prevent: Some(
                        options
                            .prevent
                            .unwrap_or_else(|| Box::new(ShippedPrevent) as Box<dyn PreventHook>),
                    ),
                    registry: None,
                },
            ),
        }
    }

    /// A session over records (`Model::from_records`: tables, indexes and the integrity check).
    pub fn from_records(
        project: Project,
        records: impl IntoIterator<Item = Record>,
        lib: impl IntoIterator<Item = LibDocument>,
        options: SessionOptions,
    ) -> Result<Session, ModelError> {
        Ok(Session::new(Model::from_records(project, records, lib)?, options))
    }

    /// The bytes of the asset with this hash, with its reference, if the project holds it (doc 05 §13.3: renderers
    /// resolve an image's bytes by hash).
    pub fn asset(&self, sha256: &ew_schema::Sha256) -> Option<(AssetRef, Arc<[u8]>)> {
        self.engine
            .model()
            .asset(sha256)
            .map(|a| (*a.reference(), Arc::clone(a.bytes())))
    }

    /// The images of the project that no drawing shows (FT-1378, decision log ID-1417): a PNG or JPEG over the pixel
    /// limit ([`ew_schema::pixel_limit`]) that an earlier build took in, which the open keeps and never drops. Each is
    /// named by where it shows, the company logo or the folios of the pages whose image items show it; in hash order.
    pub fn undrawn_images(&self) -> Vec<UndrawnImage> {
        let q = self.query();
        let model = self.engine.model();
        let over = |sha: &ew_schema::Sha256| {
            let r = *model.asset(sha)?.reference();
            match ew_schema::pixel_limit(r.format, r.size) {
                Err(AssetError::TooManyPixels { width, height, .. }) => Some((width, height)),
                _ => None,
            }
        };
        // Where each image shows: `None` for the company logo, else a folio, in page order.
        let mut shows: Vec<(ew_schema::Sha256, Option<String>)> = Vec::new();
        if let Some(logo) = ew_schema::company_logo(&q.project().extra) {
            shows.push((logo.sha256, None));
        }
        let mut pages: Vec<(ew_derive::ew_ids::Id<ew_schema::Page>, ew_schema::Sha256)> = Vec::new();
        for (_, item) in q.page_items().iter() {
            if let ew_schema::PageItemKind::Image(image) = &item.kind {
                pages.push((item.page, image.asset));
            }
        }
        pages.sort();
        pages.dedup();
        for e in &q.get::<PageOrder>(()).entries {
            if let ew_derive::PageRef::Drawn(page) = e.page {
                for (_, sha) in pages.iter().filter(|(p, _)| *p == page) {
                    shows.push((*sha, Some(e.folio.clone())));
                }
            }
        }
        let mut found: std::collections::BTreeMap<ew_schema::Sha256, UndrawnImage> = Default::default();
        for (sha, folio) in shows {
            let Some((width, height)) = over(&sha) else {
                continue;
            };
            let image = found.entry(sha).or_insert_with(|| UndrawnImage {
                logo: false,
                folios: Vec::new(),
                width,
                height,
            });
            match folio {
                None => image.logo = true,
                Some(folio) => image.folios.push(folio),
            }
        }
        found.into_values().collect()
    }

    /// [`Session::undrawn_images`] one sentence each, keyed for the reader's language (FT-1503, decision log ID-1458):
    /// the window's open banner and `ew`'s notices say the same words. Which image (the company logo, or the folios
    /// that show it), its size in pixels, that it is kept but not drawn, and the remedy.
    pub fn undrawn_image_notices(&self) -> Vec<Text> {
        let mut notices = Vec::new();
        for image in self.undrawn_images() {
            let (w, h) = (image.width.to_string(), image.height.to_string());
            if image.logo {
                notices.push(api_text!("banner-logo-not-drawn", &w, &h));
            }
            match image.folios.len() {
                0 => {}
                1 => notices.push(api_text!(
                    "banner-image-not-drawn",
                    image.folios.join(", "),
                    &w,
                    &h
                )),
                _ => notices.push(api_text!(
                    "banner-image-not-drawn-folios",
                    image.folios.join(", "),
                    &w,
                    &h
                )),
            }
        }
        notices
    }

    /// The libraries of [`SessionOptions::libraries`] (WP-S1.19 §5.2), opened (and reindexed where they changed) at
    /// the first call. When they cannot be opened the session falls back to the compiled-in starter
    /// ([`library::Registry::starter_only`]) and [`Session::library_error`] says why: a project never depends on
    /// its libraries (NFR-REL-11).
    pub fn libraries(&self) -> &library::Registry {
        &self.opened_libraries().0
    }

    /// Why [`Session::libraries`] holds only the starter, if opening the tiers failed.
    pub fn library_error(&self) -> Option<&library::LibraryError> {
        self.opened_libraries().1.as_ref()
    }

    /// The libraries for a write ([`library::Registry::write`], `create`, `take_over_stale`, `rebuild_index`); a
    /// library write is a Session operation, not a project command (WP-S1.19 D12).
    pub fn libraries_mut(&mut self) -> &mut library::Registry {
        self.opened_libraries();
        &mut self.libraries.get_mut().expect("opened above").0
    }

    fn opened_libraries(&self) -> &(library::Registry, Option<library::LibraryError>) {
        self.libraries.get_or_init(|| match &self.library_tiers {
            None => (library::Registry::starter_only(), None),
            Some(tiers) => match library::Registry::open(tiers) {
                Ok(r) => (r, None),
                Err(e) => (library::Registry::starter_only(), Some(e)),
            },
        })
    }

    /// The part search of WP-S1.19 D6 over [`Session::libraries`], with `held` filled from the project's part copies
    /// (their library and item IDs; the caller's own `held` entries are kept).
    pub fn part_search(&self, q: &library::PartQuery) -> Vec<library::PartHit> {
        let mut q = q.clone();
        let query = self.query();
        q.held.extend(
            query
                .parts()
                .iter()
                .map(|(_, c)| (c.library_id.as_uuid(), c.item_id.as_uuid())),
        );
        q.held.sort_unstable();
        q.held.dedup();
        self.libraries().search(&q)
    }

    /// The current snapshot, O(1).
    pub fn world(&self) -> World {
        self.engine.world()
    }

    /// A tracked-read context over the current snapshot with the report engine installed, so that
    /// `page_order`, the TOC, Prev/Next and title blocks include the generated report pages (doc 09 §3.1,
    /// FR-REP-001). Request-scoped: build one per evaluation (a frame, an export pass).
    ///
    /// With [`MemoMode::On`] (the default) the `Q` memoises across requests in the session's memo store.
    pub fn query(&self) -> Q {
        match &self.memo {
            Some(memo) => memo.query(),
            None => self.query_over(&self.engine.world().model),
        }
    }

    /// The printed folio labels of the derived page order (`page_order`: drawn pages, section dividers and
    /// generated report pages), in order.
    pub fn page_labels(&self) -> Vec<String> {
        self.query()
            .get::<PageOrder>(())
            .entries
            .iter()
            .map(|e| e.folio.clone())
            .collect()
    }

    /// The layout spaces of the project with their locations, in ID order (WP-S4.01, FR-LAY-001).
    pub fn layout_spaces(
        &self,
    ) -> Vec<(
        ew_derive::ew_ids::Id<ew_schema::LayoutSpace>,
        ew_derive::ew_ids::Id<ew_schema::StructureNode>,
    )> {
        self.query()
            .table::<ew_schema::LayoutSpace>()
            .iter()
            .map(|(id, s)| (id, s.location))
            .collect()
    }

    /// `unplaced(space)` (FR-LAY-013, doc 12 §2.6; WP-S4.01 part 2): the mountable devices of the space's location
    /// without a placement, in natural order of their full tag ([`ew_derive::layout::Unplaced`]). Empty for a space
    /// that is not in the project.
    pub fn unplaced(
        &self,
        space: ew_derive::ew_ids::Id<ew_schema::LayoutSpace>,
    ) -> Vec<ew_derive::ew_ids::Id<ew_schema::Device>> {
        self.query()
            .get::<ew_derive::layout::Unplaced>(space)
            .as_ref()
            .clone()
    }

    /// The session's memo store (statistics, memory, oracle mode); `None` with [`MemoMode::Off`].
    pub fn memo(&self) -> Option<&MemoStore> {
        self.memo.as_ref()
    }

    /// The oracle's mismatches since the last call ([`MemoMode::Oracle`]): memoised values that differ from
    /// a cold run, which is a memo-engine bug.
    pub fn take_oracle_mismatches(&self) -> Vec<Mismatch> {
        self.memo
            .as_ref()
            .map(MemoStore::take_mismatches)
            .unwrap_or_default()
    }

    /// [`Session::query`] over another snapshot (a preview's candidate model, an export fork): cold.
    pub fn query_over(&self, model: &Model) -> Q {
        Q::new(model).with_report_pages(Arc::clone(&self.reports))
    }

    /// Runs an interactive command through the commit pipeline (ARCH §5.2) from the session's origin.
    /// A bulk command is refused with [`CmdError::NeedsPreview`].
    pub fn execute(&mut self, cmd: &dyn DynCommand) -> Result<Receipt, CmdError> {
        self.approval_only(cmd.name(), &self.submit_origin)?;
        let before = self.engine.model().clone();
        let result = self.engine.execute(cmd);
        self.committed(&before, cmd.name());
        result
    }

    /// [`Session::execute`] with an explicit origin.
    pub fn execute_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Receipt, CmdError> {
        self.approval_only(cmd.name(), &submit.origin)?;
        let before = self.engine.model().clone();
        let result = self.engine.execute_with(cmd, submit);
        self.committed(&before, cmd.name());
        result
    }

    /// Prepares a command on a fork of the current snapshot: the candidate model, its ops, the read set,
    /// the entity and field diff and the affected pages. Nothing is committed. Synchronous in v0.
    ///
    /// `project.assets.purge` is refused here already when the purge would be refused (its file list is
    /// [`Session::purge_preview`]). `revision.approve` is refused here and in [`Session::execute`]: its parameters are
    /// the release gate's, so only [`Session::approve`] submits it.
    pub fn preview(&mut self, cmd: &dyn DynCommand) -> Result<Preview, CmdError> {
        self.approval_only(cmd.name(), &self.submit_origin)?;
        self.purge_gate(cmd.name())?;
        self.engine.preview(cmd)
    }

    /// [`Session::preview`] with an explicit submission.
    pub fn preview_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Preview, CmdError> {
        self.approval_only(cmd.name(), &submit.origin)?;
        self.purge_gate(cmd.name())?;
        self.engine.preview_with(cmd, submit)
    }

    /// The derived values a preview changes (doc 03 §8.6 "diff_query", v0 subset): the printed device tags
    /// and cross-reference texts that differ between the current snapshot (read through the session's memo
    /// store) and the preview's candidate (a cold read). A preview made on an older snapshot is compared
    /// with the current one; preview the command again after other commits.
    pub fn derived_diff(&self, preview: &Preview) -> DerivedDiff {
        ew_commands::derived_diff(&self.query(), &self.query_over(preview.candidate()))
    }

    /// Commits a preview after the rebase check; re-runs the command if its reads changed.
    ///
    /// The preview of `project.assets.purge` runs the purge ([`Session::purge_assets`] with the listed hashes) and
    /// commits the command's marker line only when the files are removed.
    pub fn commit(&mut self, preview: Preview) -> Result<Receipt, CmdError> {
        if preview.command().name == PurgeAssets::NAME {
            return self.commit_purge(preview);
        }
        let before = self.engine.model().clone();
        let name = preview.command().name.clone();
        let result = self.engine.commit(preview);
        self.committed(&before, &name);
        result
    }

    /// Undoes the last step as a new revision; returns the editor context to restore.
    ///
    /// `origin` is who asks: a step that holds a `HumanOnly` command (`project.checks.set`) is undone only from the GUI
    /// and a confirmed CLI subcommand (ADR-0008 amendment 3's note, FT-404); a command log, script, MCP, plugin or
    /// Python origin gets [`CmdError::Exposure`] and the session is unchanged.
    ///
    /// # Errors
    ///
    /// [`CmdError::History`] when there is nothing to undo, a checkpoint blocks it or the model changed under the
    /// step; [`CmdError::Exposure`] when `origin` may not undo the step.
    pub fn undo(&mut self, origin: &Origin) -> Result<Restored, CmdError> {
        let before = self.engine.model().clone();
        let result = self.engine.undo(origin);
        self.committed(&before, "undo");
        result
    }

    /// Redoes the next step as a new revision, with [`Session::undo`]'s origin rule.
    ///
    /// # Errors
    ///
    /// [`CmdError::History`] when there is nothing to redo or the model changed under the step;
    /// [`CmdError::Exposure`] when `origin` may not redo the step.
    pub fn redo(&mut self, origin: &Origin) -> Result<Restored, CmdError> {
        let before = self.engine.model().clone();
        let result = self.engine.redo(origin);
        self.committed(&before, "redo");
        result
    }

    /// The undo stack.
    pub fn history(&self) -> &History {
        self.engine.history()
    }

    /// Takes the change-feed entries since the last call.
    pub fn drain_changes(&mut self) -> Vec<Change> {
        let mut out = std::mem::take(&mut self.changes);
        out.extend(self.engine.drain_changes());
        out
    }

    /// The command registry: names, metadata and parameter schemas.
    pub fn registry(&self) -> &CommandRegistry {
        self.engine.registry()
    }

    /// The registered metadata of the command named `name` (a canonical name or an alias); `None` for an
    /// unregistered name.
    pub fn command_meta(&self, name: &str) -> Option<CommandMeta> {
        self.registry().get(name).map(|e| *e.meta())
    }

    /// Marks the current state as saved (the store calls this after a save, WP-S1.09).
    pub fn mark_saved(&mut self) {
        self.engine.mark_saved();
    }

    /// Whether the model differs from the last saved state: a project migrated from an older format differs until its
    /// first save in an editing session (WP-S1.17 D11, decision log ID-1230 ruling 4); a read-only one never saves it.
    pub fn is_dirty(&self) -> bool {
        self.engine.is_dirty()
            || self.persistence.as_ref().is_some_and(|p| {
                p.recovered || (p.store.migrated_from().is_some() && *p.store.access() == Access::Edit)
            })
    }

    /// The project-level derivation findings of the model the session opened with, in canonical order:
    /// an unknown pinned profile (`EW-CORE-018`, error), malformed provisional settings members.
    pub fn load_findings(&self) -> &[DerivationFinding] {
        &self.load_findings
    }

    /// Every derivation finding of the current snapshot, in canonical order: the project-level ones plus
    /// those of deriving every device tag and every page's frame (schemes not applied, the default frame,
    /// query cycles). A cold full pass, O(devices + pages); call it on demand (a check run, an export).
    pub fn derivation_findings(&self) -> Vec<DerivationFinding> {
        let mut q = self.query();
        let _ = q.get::<NumberingSettingsOf>(());
        let _ = q.get::<FrameSettings>(());
        let devices: Vec<_> = q.devices().ids().collect();
        for d in devices {
            let _ = q.get::<DeviceTag>(d);
        }
        let pages: Vec<_> = q.pages().ids().collect();
        for p in pages {
            let _ = q.get::<FrameGridOf>(p);
        }
        q.diagnostics()
            .notes()
            .into_iter()
            .map(DerivationFinding::from)
            .collect()
    }

    // ---------------------------------------------------------------------------------------------------------
    // Persistence (WP-S1.09)

    /// After a mutating call: takes the change feed's new entries (kept for [`Session::drain_changes`]),
    /// advances the memo store with their keys (push invalidation), adds them to the dirty set and journals
    /// the transaction (doc 05 §8.1).
    fn committed(&mut self, before: &Model, cmd: &str) {
        let changes = self.engine.drain_changes();
        if changes.is_empty() {
            // A transaction without row changes that brought an asset (a lost logo picked again; assets are only
            // ever added): journal it, so that a crash before the save does not lose its bytes (FT-337), and bring the
            // memo store along, whose readers of the asset store answer differently now (the revision is the same).
            let arrived = self.engine.model().assets_stamp() != before.assets_stamp();
            if arrived && let Some(memo) = &self.memo {
                memo.advance(self.engine.model(), &[]);
            }
            if let (true, Some(p)) = (arrived, &mut self.persistence) {
                let _ = p
                    .store
                    .journal_commit(before, self.engine.model(), &[], cmd, &self.origin);
            }
            return;
        }
        let keys: Vec<AnyKey> = changes
            .iter()
            .flat_map(|c| c.touched_keys.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if let Some(memo) = &self.memo {
            memo.advance(self.engine.model(), &keys);
        }
        let Some(p) = &mut self.persistence else {
            self.changes.extend(changes);
            return;
        };
        p.unsaved.extend(keys.iter().copied());
        // A journal failure leaves the edit standing; the store reports the journal as unavailable.
        let _ = p
            .store
            .journal_commit(before, self.engine.model(), &keys, cmd, &self.origin);
        self.changes.extend(changes);
    }

    /// Opens a project directory (doc 05 §6.1) and starts a session on it: read-write when the lock is
    /// ours, read-only otherwise ([`OpenReport::access`]). `recovery` says what happens to a leftover
    /// journal of a crashed session.
    ///
    /// # Errors
    ///
    /// [`OpenError`] when the project cannot be opened; integrity findings are in the report.
    pub fn open(
        dir: &Path,
        store: StoreOptions,
        options: SessionOptions,
        recovery: Recovery,
    ) -> Result<(Session, OpenReport), OpenError> {
        let opened = Store::open(dir, store)?;
        let (mut store, mut model, report) = (opened.store, opened.model, opened.report);
        let mut unsaved = BTreeSet::new();
        let mut recovered = false;
        let mut refusal = None;
        if store.recovery().is_some() {
            match recovery {
                Recovery::Keep => {}
                Recovery::Discard => store.discard_recovery(),
                Recovery::Apply => match store.recover(&model) {
                    Ok(r) => {
                        model = r.model;
                        unsaved = r.keys;
                        recovered = true;
                    }
                    // Collisions or an unreadable journal: the journal is kept, the disk state opens and the
                    // refusal is reported (`Session::recovery_refusal`).
                    Err(e) => refusal = Some(e),
                },
            }
        }
        let mut session = Session::new(model, options);
        let dropped = dropped_copies(&session.query(), &report);
        session.persistence = Some(Persistence {
            store,
            unsaved,
            recovered,
            refusal,
            consented: false,
            backup: None,
            watch: external_change::Watch::default(),
            dropped,
        });
        // FT-814: this directory's stale release-run records (archive gone or released) go now.
        session.remove_stale_run_records();
        Ok((session, report))
    }

    /// Opens a project directory read-only for a reader such as `ew check` (doc 17 §3.1: readers take no
    /// lock and read one generation): no lock, no roll-forward, no journal; an `.ewz` given as `dir` opens from memory
    /// ([`Session::open_ewz`], WP-S1.17 D9). `app_version` names the program in the store's diagnostics.
    ///
    /// # Errors
    ///
    /// [`OpenError`] when the project cannot be opened; integrity findings are in the report.
    pub fn open_read_only(dir: &Path, app_version: &str) -> Result<(Session, OpenReport), OpenError> {
        Session::open_reader(dir, app_version, "check")
    }

    /// [`Session::open_read_only`] for the reader subcommand `subcommand` (`check`, `sql`), which names the
    /// origin of the session.
    ///
    /// # Errors
    ///
    /// [`OpenError`] when the project cannot be opened; integrity findings are in the report.
    pub fn open_reader(
        dir: &Path,
        app_version: &str,
        subcommand: &str,
    ) -> Result<(Session, OpenReport), OpenError> {
        Session::open_reader_with(dir, app_version, subcommand, MemoMode::default())
    }

    /// [`Session::open_reader`] with the memo mode of the session (`ew verify-derived` opens with
    /// [`MemoMode::Oracle`]). A path that names an `.ewz` ([`ewz::is_ewz`]: the extension `ewz`, not a directory) opens
    /// through [`Session::open_ewz_with`], so every reader takes one as its project.
    ///
    /// # Errors
    ///
    /// [`OpenError`] when the project cannot be opened ([`OpenError::Ewz`] for a refused `.ewz`); integrity findings
    /// are in the report.
    pub fn open_reader_with(
        dir: &Path,
        app_version: &str,
        subcommand: &str,
        memo: MemoMode,
    ) -> Result<(Session, OpenReport), OpenError> {
        if ewz::is_ewz(&ew_store::OsVfs, dir) {
            return Session::open_ewz_with(dir, StoreOptions::new(app_version), subcommand, memo);
        }
        let mut store = StoreOptions::new(app_version);
        store.mode = ew_store::OpenMode::ReadOnly;
        store.state_dir = None;
        let options = SessionOptions {
            origin: Origin::Cli {
                subcommand: subcommand.to_owned(),
                confirmed: false,
            },
            memo,
            ..SessionOptions::default()
        };
        Session::open(dir, store, options, Recovery::Keep)
    }

    /// Builds the read-only SQL projection of the current snapshot (ADR-0017 decision 3: `ew sql`, exports and
    /// release checks build it on their own fork at the opened generation, so it is fresh by construction).
    /// `checks` fills `api_v1_message`; `None` leaves it empty. `api_v1_meta` carries the revision, the store
    /// generation and this program's version.
    ///
    /// # Errors
    ///
    /// [`sql::BuildError`] when SQLite fails while the tables are written.
    pub fn sql_projection(
        &self,
        checks: Option<&checks::CheckReport>,
    ) -> Result<sql::Projection, sql::BuildError> {
        let world = self.world();
        let input = sql::BuildInput {
            rev: i64::try_from(world.rev.0).ok(),
            generation: self.store().and_then(|s| i64::try_from(s.generation()).ok()),
            ew_version: env!("CARGO_PKG_VERSION"),
            as_of: None,
            checks,
        };
        sql::Projection::build(&self.query(), &input)
    }

    /// The shipped check scheme the project names for a run in `mode` (`settings.extra.checks`, set by
    /// `project.checks.set`, FT-366): `release_scheme` for [`checks::CheckMode::Release`] (the release gate and
    /// `ew check`), `active_scheme` for [`checks::CheckMode::Online`]. `None` when the project names none or
    /// names a scheme that does not ship (a newer or damaged file): the caller's default applies.
    pub fn project_check_scheme(&self, mode: checks::CheckMode) -> Option<&'static checks::Scheme> {
        checks::scheme(&self.project_check_scheme_name(mode)?)
    }

    /// The name the project gives the check scheme of a run in `mode` (the member [`Session::project_check_scheme`]
    /// reads), whether or not this build ships it; `None` when the project names none. `ew check` prints a notice for
    /// a name this build does not ship before it falls back to `pre-delivery` (FT-405, decision log ID-602). The lookup
    /// is `ew-checks`' ([`checks::project_scheme_name`]), which the open-items list shares (WP-S3.06 D8).
    pub fn project_check_scheme_name(&self, mode: checks::CheckMode) -> Option<String> {
        checks::project_scheme_name(&self.query(), mode)
    }

    /// Runs the check scheme `scheme` in `mode` over the current snapshot through the session's memo store
    /// (doc 15 §3): the findings, the totals and `ew check`'s exit code ([`checks::CheckReport`]). With merge evidence the
    /// model does not hold ([`Session::merge_evidence`]), EW-CORE-013's groups join a run in a mode that runs the rule,
    /// online (the window: the message panel, its counts, the property grid's chips) and release (WP-S1.17 D29).
    pub fn check(&self, scheme: &checks::Scheme, mode: checks::CheckMode) -> checks::CheckReport {
        self.check_over(&self.query(), scheme, mode)
    }

    /// [`Session::check`] over the snapshot of `q`.
    pub(crate) fn check_over(
        &self,
        q: &Q,
        scheme: &checks::Scheme,
        mode: checks::CheckMode,
    ) -> checks::CheckReport {
        let report = self.with_layout_findings(q, checks::run(q, scheme, mode));
        if self.merge_evidence() {
            report.with_merge_evidence(q)
        } else {
            report
        }
    }

    /// The project-integrity check of `ew check --integrity` (FR-CHK-012; WP-S1.17 D29, decision log ID-1284 ruling 5):
    /// the store's integrity findings of opening it (`opened`) and every registered rule whose ID lies in EW-CORE-001
    /// to -019 (EW-CORE-011's conflict records and EW-CORE-013's collisions after a merge), in release mode, with merge
    /// evidence as [`Session::check`] takes it. Its exit code is 1 on any error.
    pub fn check_integrity(&self, opened: &OpenReport) -> checks::CheckReport {
        let q = self.query();
        let integrity: Vec<&'static dyn checks::Rule> = checks::rules()
            .iter()
            .copied()
            .filter(|r| {
                r.meta()
                    .id
                    .strip_prefix("EW-CORE-")
                    .and_then(|n| n.parse::<u32>().ok())
                    .is_some_and(|n| (1..=19).contains(&n))
            })
            .collect();
        let report = match checks::run_table_rules(&q, &INTEGRITY_RUN, checks::CheckMode::Release, &integrity)
        {
            Ok(report) => report,
            Err(cycle) => panic!("{cycle}"),
        };
        let report = if self.merge_evidence() {
            report.with_merge_evidence(&q)
        } else {
            report
        };
        report.with_integrity(integrity_items(opened))
    }

    /// Whether the project carries merge evidence the model does not hold (WP-S1.17 D29): its manifest is a merged one
    /// (`merged: true`), a merge or rebase is in progress in its git worktree, or [`Session::assume_merged`] was called
    /// (`ew check --integrity --after-merge`, the post-merge hook). Conflict records, the model's own evidence, are
    /// EW-CORE-013's rule's.
    pub fn merge_evidence(&self) -> bool {
        let Some(p) = self.persistence.as_ref() else {
            return false;
        };
        p.watch.after_merge
            || p.store.manifest().is_some_and(|m| m.merged)
            || git::merging(&**p.store.vfs(), p.store.root()).unwrap_or(false)
    }

    /// Counts the project as just merged for EW-CORE-013 ([`Session::merge_evidence`]): `ew check --integrity
    /// --after-merge`, which the post-merge hook runs.
    pub fn assume_merged(&mut self) {
        if let Some(p) = self.persistence.as_mut() {
            p.watch.after_merge = true;
        }
    }

    /// The CSV text of an export over the current snapshot (`ew export cables|bom`, doc 17 §3.3): the rows
    /// of the project's report block of the matching kind, through the same row source and CSV writer as
    /// the folio (`ew_reports::export`).
    ///
    /// # Errors
    ///
    /// [`ExportError`] when the project has no block of the kind or the block does not derive.
    pub fn export_csv(&self, what: Export) -> Result<String, ExportError> {
        ew_reports::export::export_csv(&self.query(), what)
    }

    /// [`Session::export_csv`] with the number of rows written.
    ///
    /// # Errors
    ///
    /// [`ExportError`] when the project has no block of the kind or the block does not derive.
    pub fn export(&self, what: Export) -> Result<Exported, ExportError> {
        ew_reports::export::export(&self.query(), what)
    }

    /// Writes the session's model as a new project into `dir` (a directory without a project) and keeps
    /// it as the session's project (Save As, doc 05 §5.5). Once the new project exists, the previous one is
    /// closed cleanly: its journal (the edits that are now saved in the new project) is deleted and its lock
    /// released, so reopening it neither offers to recover them nor opens it read-only (FT-499). When the
    /// write fails, the previous project and its journal stay as they were.
    ///
    /// # Errors
    ///
    /// [`OpenError`] when the directory holds a project, the lock is held or the write fails.
    pub fn create(&mut self, dir: &Path, store: StoreOptions) -> Result<(), OpenError> {
        let model = self.engine.model().clone();
        let store = Store::create(dir, &model, store)?;
        let previous = self.persistence.replace(Persistence {
            store,
            unsaved: BTreeSet::new(),
            recovered: false,
            refusal: None,
            consented: false,
            backup: None,
            watch: external_change::Watch::default(),
            dropped: Vec::new(),
        });
        if let Some(p) = previous {
            p.store.close();
        }
        self.engine.mark_saved();
        Ok(())
    }

    /// FT-1383 (ID-1418): the first record, in the integrity check's order, that names a library copy the open
    /// dropped as damaged, as the device's tag, the copy's project-relative file and, from the installed libraries, the
    /// copy's part (FT-1396), with the copy and that record in words (FT-1500); when the model passes the check, the
    /// first copy the open dropped that no record names ([`DamagedCopy::left_out`]); `None` when there is neither. A
    /// project with a named one is read-only, and [`Session::create`] refuses it: a copy without the damaged file would
    /// not open.
    pub fn damaged_copy(&self) -> Option<DamagedCopy> {
        let model = self.engine.model();
        let (referrer, kind, copy) = match model.integrity_check() {
            Err(ModelError::Invariant { id, keys, .. }) => {
                let (&[referrer, (kind, copy)], true) = (keys.as_slice(), id == ew_model::REFS_RESOLVE)
                else {
                    return None;
                };
                (Some(referrer), kind, copy)
            }
            Err(_) => return None,
            // FT-1500 (ID-1458): a dropped copy that only another copy's library link names, or nothing.
            Ok(()) => {
                let &(kind, copy) = self.persistence.as_ref()?.dropped.first()?;
                (None, kind, copy)
            }
        };
        let file = ew_store::ew_format::FilePath::lib(ew_store::ew_format::lib_dir(kind)?, copy).to_string();
        let q = self.query();
        let device = referrer.and_then(|referrer| match referrer.0 {
            ew_schema::Kind::Device => Some(referrer.1.cast()),
            ew_schema::Kind::Function => q.functions().get(referrer.1.cast()).and_then(|f| f.device),
            ew_schema::Kind::Representation => q
                .table::<ew_schema::Representation>()
                .get(referrer.1.cast())
                .and_then(|r| r.functions.iter().find_map(|f| q.functions().get(*f)?.device)),
            _ => None,
        });
        let device = device.map(|d| {
            q.get::<DeviceTag>(d)
                .as_ref()
                .as_ref()
                .map(|t| t.short())
                .unwrap_or_default()
        });
        let installed = self.installed_document(kind, copy);
        let part = match &installed {
            Some((LibDocument::Part(c), library)) => Some((c.item.part_number.to_string(), library.clone())),
            _ => None,
        };
        let words =
            ew_commands::damaged_copy_words(model, kind, installed.as_ref().map(|(d, _)| d), referrer);
        // C53-p3#6 (ID-1481): a Save As leaves out every dropped copy no record names, and names them all.
        let left_out_words = match referrer {
            Some(_) => Vec::new(),
            None => self
                .persistence
                .as_ref()
                .map(|p| p.dropped.as_slice())
                .unwrap_or_default()
                .iter()
                .map(|&(kind, copy)| {
                    let installed = self.installed_document(kind, copy);
                    ew_commands::damaged_copy_words(model, kind, installed.as_ref().map(|(d, _)| d), None)
                })
                .collect(),
        };
        // C53-p4#5: the update is offered for the copy the banner names, or for any copy of a banner that names them all.
        let in_library = match referrer {
            Some(_) => installed.is_some(),
            None => self.persistence.as_ref().is_some_and(|p| {
                p.dropped
                    .iter()
                    .any(|&(kind, copy)| self.installed_document(kind, copy).is_some())
            }),
        };
        Some(DamagedCopy {
            device,
            file,
            part,
            words,
            left_out: referrer.is_none(),
            left_out_words,
            in_library,
        })
    }

    /// Update from library of the damaged library copies (FT-1499, decision log ID-1486): every copy the open dropped
    /// as damaged that an installed library holds is read and checked as `library.embed` reads a library's document,
    /// before anything is written, and the store then writes those copies over their damaged files in one save
    /// ([`Store::update_damaged_copies`]); a copy no library holds, or whose document the check refuses, stays damaged
    /// and is named. The session is not opened again: its caller opens the project again, writable when the integrity
    /// check passes.
    ///
    /// # Errors
    ///
    /// The refusal in words, nothing written: no copy can be updated from a library, the libraries' versions would
    /// leave a project that does not open, another session holds the project, the project is read-only for another
    /// reason or has other integrity findings, or the save fails.
    pub fn update_damaged_copies(&mut self) -> Result<CopiesUpdated, Text> {
        let dropped = self
            .persistence
            .as_ref()
            .map(|p| p.dropped.clone())
            .unwrap_or_default();
        if dropped.is_empty() {
            return Ok(CopiesUpdated::default());
        }
        let model = self.engine.model().clone();
        let mut engine = ew_commands::Engine::new(
            model.clone(),
            ew_commands::EngineOptions {
                prevent: Some(Box::new(crate::ShippedPrevent)),
                ..ew_commands::EngineOptions::default()
            },
        );
        let mut done = CopiesUpdated::default();
        let mut keys = BTreeSet::new();
        for &(kind, copy) in &dropped {
            let installed = self.installed_document(kind, copy);
            let words =
                ew_commands::damaged_copy_words(&model, kind, installed.as_ref().map(|(d, _)| d), None);
            let Some(value) = installed.and_then(|(doc, _)| serde_json::to_value(&doc).ok()) else {
                done.not_updated.push(words);
                continue;
            };
            // The library's current version is written, named by its version (C53-p4#4, ID-1491): the damaged copy's
            // own version cannot be read, so the change is not classified.
            let version = value["version"].as_str().unwrap_or_default().to_owned();
            let cmd = ew_commands::commands::EmbedLibraryCopies { copies: vec![value] };
            let Ok(cmd) = AnyCommand::of(&cmd) else {
                done.not_updated.push(words);
                continue;
            };
            match engine.execute(&cmd) {
                Ok(_) => {
                    keys.insert((kind, copy));
                    done.updated
                        .push(api_text!("refusal-words-copy-at-version", words, version));
                }
                // The library's version breaks a record of the project (an invariant or a prevent rule of the
                // engine's own check): the misfit, nothing written (C53-p4#3).
                Err(ew_commands::CmdError::Model(..) | ew_commands::CmdError::Prevented { .. }) => {
                    return Err(api_text!("refusal-update-copies-misfit"));
                }
                Err(_) => done.not_updated.push(words),
            }
        }
        let (Some(p), false) = (self.persistence.as_mut(), keys.is_empty()) else {
            let all = ew_commands::comma_joined(done.not_updated.clone());
            return all.map_or(Ok(done), |all| {
                Err(api_text!("refusal-update-copies-no-library", all))
            });
        };
        // The project must open after it: every violation left is one the copies still damaged explain.
        let missing: BTreeSet<AnyKey> = dropped.iter().filter(|k| !keys.contains(*k)).copied().collect();
        if ew_model::unexplained_by_dropped_copies(&engine.model().integrity_violations(), &missing).is_some()
        {
            return Err(api_text!("refusal-update-copies-misfit"));
        }
        match p.store.update_damaged_copies(engine.model(), &keys) {
            Ok(_) => Ok(done),
            Err(ew_store::SaveError::ReadOnly(ReadOnlyReason::LockedBy(_))) => {
                Err(api_text!("refusal-update-copies-locked"))
            }
            Err(ew_store::SaveError::ReadOnly(ReadOnlyReason::Integrity)) => {
                Err(api_text!("refusal-update-copies-findings"))
            }
            Err(ew_store::SaveError::ReadOnly(_)) => Err(api_text!("refusal-update-copies-read-only")),
            Err(e) => Err(api_text!("refusal-update-copies-save", e.to_string())),
        }
    }

    /// The document of `kind` and the library's name of the installed item whose copy ID is `copy` (FT-1396, ID-1423;
    /// FT-1500): the words of a damaged copy, whose own file cannot be read.
    fn installed_document(
        &self,
        kind: ew_schema::Kind,
        copy: ew_derive::ew_ids::Id<()>,
    ) -> Option<(LibDocument, String)> {
        let item_id = |d: &LibDocument| match d {
            LibDocument::Part(c) => Some(c.item_id),
            LibDocument::Symbol(c) => Some(c.item_id),
            LibDocument::Fndef(c) => Some(c.item_id),
            LibDocument::Form(c) => Some(c.item_id),
            LibDocument::Macro(c) => Some(c.item_id),
            LibDocument::Manufacturer(c) => Some(c.item_id),
            LibDocument::PlcProfile(c) => Some(c.item_id),
            LibDocument::Unknown(_) => None,
        };
        let libraries = self.libraries();
        libraries.libraries().into_iter().find_map(|info| {
            libraries
                .documents(info.id, kind)
                .into_iter()
                .find(|d| {
                    item_id(d).is_some_and(|i| {
                        ew_derive::ew_ids::v5::lib_copy_id::<()>(info.id, i.as_uuid()) == copy
                    })
                })
                .map(|d| (d, info.name.clone()))
        })
    }

    /// Saves the current model (doc 05 §5.2): only the files of the transactions since the last save,
    /// nothing at all for an unchanged project. On success the history gets its saved marker.
    ///
    /// # Errors
    ///
    /// [`SaveError`]; without a project directory, [`SaveError::ReadOnly`] with
    /// [`ReadOnlyReason::Requested`]. After the commit point of this
    /// save ([`SaveError::Incomplete`] of a generation above the one before the call) the generation is
    /// decided: the session counts as saved and the store completes the save by roll-forward
    /// ([`Session::retry_save`]). Every other error, [`SaveError::PendingIncomplete`] (an earlier save is
    /// still incomplete) included, wrote nothing of this model: the session stays dirty and keeps its dirty
    /// set for the next save.
    pub fn save(&mut self) -> Result<SaveReport, SaveError> {
        let model = self.engine.model().clone();
        let Some(p) = &mut self.persistence else {
            return Err(SaveError::ReadOnly(ew_store::ReadOnlyReason::Requested));
        };
        // A held external change (WP-S1.17 D21, D22): nothing is written until it is applied or reads.
        Session::external_hold(p)?;
        // The first save of a migrated project after the consent: the backup, verified, before any project byte
        // (WP-S1.17 D11, decision log ID-1221 ruling 5); a read-only session writes neither.
        if p.consented
            && p.backup.is_none()
            && *p.store.access() == Access::Edit
            && let Some(from) = p.store.migrated_from()
        {
            p.backup = Some(write_upgrade_backup(&p.store, from)?);
            p.store.confirm_upgrade();
        }
        let before = p.store.generation();
        let result = p.store.save(&model, Changes::Keys(&p.unsaved));
        // Only edits that are on disk, or decided and durable in an intent, leave the dirty set (review
        // cycle 8, blocker 1): never on the strength of an earlier save's outcome.
        let on_disk = match &result {
            Ok(_) => true,
            Err(SaveError::Incomplete { generation, .. }) => *generation > before,
            Err(_) => false,
        };
        if on_disk {
            p.unsaved.clear();
            p.recovered = false;
            self.engine.mark_saved();
        }
        result
    }

    /// Completes an incomplete save (doc 05 §5.4: the caller retries every 5 s).
    ///
    /// # Errors
    ///
    /// [`SaveError::Incomplete`] while the roll-forward still fails.
    pub fn retry_save(&mut self) -> Result<Option<u64>, SaveError> {
        match &mut self.persistence {
            Some(p) => p.store.retry_pending(),
            None => Ok(None),
        }
    }

    /// The user's consent to converting a project that opened migrated from an older format
    /// ([`OpenReport::migrated_from`], WP-S1.17 D11), kept in memory: the next [`Session::save`] first writes the
    /// backup `<project folder>.pre-<format>.ewz` beside the project folder (`-2`, `-3` and so on past a name that holds
    /// another file), read back and verified, and then saves in this version's format. A session closed before that
    /// save writes nothing, and the next open asks again. Nothing for a project that was not migrated.
    pub fn confirm_upgrade(&mut self) {
        if let Some(p) = &mut self.persistence {
            p.consented = true;
        }
    }

    /// The backup the first save after [`Session::confirm_upgrade`] wrote (or reused), once written.
    pub fn upgrade_backup(&self) -> Option<&Path> {
        self.persistence.as_ref().and_then(|p| p.backup.as_deref())
    }

    /// The lock heartbeat (doc 05 §7.2); call it every [`ew_store::HEARTBEAT`].
    ///
    /// # Errors
    ///
    /// An I/O error touching the lock.
    pub fn heartbeat(&mut self) -> std::io::Result<LockState> {
        match &mut self.persistence {
            Some(p) => p.store.heartbeat(),
            None => Ok(LockState::NotHeld),
        }
    }

    // ---------------------------------------------------------------------------------------------------------
    // The asset purge (FT-380)

    /// What `project.assets.purge` would delete now (the preview, doc 05 §13.3): the asset files of the project
    /// directory whose hash no project file and no undo or redo entry of this session mentions. A session without a
    /// project directory has no asset files: its plan is empty.
    ///
    /// # Errors
    ///
    /// [`PurgeError`]: a read-only session, unsaved changes, quarantined lines, a crashed session's journal, an
    /// incomplete save, an external change, an unreadable file.
    pub fn purge_preview(&self) -> Result<PurgePlan, PurgeError> {
        let Some(p) = self.persistence.as_ref() else {
            return Ok(PurgePlan::default());
        };
        if self.is_dirty() || !p.unsaved.is_empty() || p.recovered {
            return Err(PurgeError::Unsaved);
        }
        let keep = self.undo_assets()?;
        p.store.purge_plan(&keep)
    }

    /// Runs the asset purge (doc 05 §13.3, FT-380): deletes the files of [`Session::purge_preview`], or only those of
    /// `only` (every one must still be purgeable, else [`PurgeError::Stale`] and nothing goes), through the save
    /// protocol, and records `project.assets.purge` as a marker line in the history. The model does not change.
    ///
    /// # Errors
    ///
    /// The refusals of [`Session::purge_preview`], [`PurgeError::Stale`], and [`PurgeError::Save`] when the save
    /// fails (an incomplete save above the old generation removed the files: the marker is recorded then too).
    pub fn purge_assets(
        &mut self,
        only: Option<&BTreeSet<ew_schema::Sha256>>,
    ) -> Result<(PurgePlan, SaveReport), PurgeError> {
        self.purge_files(only)
            .map(|(plan, report, _marker)| (plan, report))
    }

    /// The purge and its marker line; the receipt is the marker's (`Err` only if the engine refused the marker, which
    /// commits no ops; the files are removed then all the same).
    #[allow(clippy::type_complexity)]
    fn purge_files(
        &mut self,
        only: Option<&BTreeSet<ew_schema::Sha256>>,
    ) -> Result<(PurgePlan, SaveReport, Result<Receipt, CmdError>), PurgeError> {
        // `HumanOnly` (owner ruling, 2026-10-23): judged before any file is touched, by the session's origin.
        if !<PurgeAssets as ew_commands::Command>::meta()
            .exposure
            .allows(&self.submit_origin)
        {
            return Err(PurgeError::HumanOnly);
        }
        let dirty = self.is_dirty();
        let keep = self.undo_assets()?;
        let model = self.engine.model().clone();
        let before = self.persistence.as_ref().map_or(0, |p| p.store.generation());
        let result = match self.persistence.as_mut() {
            // No project directory: no asset files, nothing to purge.
            None => match only {
                Some(listed) if !listed.is_empty() => {
                    Err(PurgeError::Stale(listed.iter().copied().collect()))
                }
                _ => Ok((PurgePlan::default(), SaveReport::default())),
            },
            Some(p) if dirty || !p.unsaved.is_empty() || p.recovered => Err(PurgeError::Unsaved),
            Some(p) => p.store.purge(&model, &keep, only),
        };
        // The files that went (or go by roll-forward, after the commit point of an incomplete save).
        let listed = match &result {
            Ok((plan, _)) => plan.hashes(),
            Err(PurgeError::Save {
                error: SaveError::Incomplete { generation, .. },
                plan,
            }) if *generation > before => plan.hashes(),
            Err(_) => {
                return result.map(|(plan, report)| (plan, report, Err(CmdError::internal("unreachable"))));
            }
        };
        // The marker line, with the files that went as the command's parameters.
        let cmd = AnyCommand {
            name: PurgeAssets::NAME.to_owned(),
            params: serde_json::json!({
                "assets": listed.iter().map(ToString::to_string).collect::<Vec<_>>()
            }),
        };
        let receipt = self
            .engine
            .preview(&cmd)
            .and_then(|preview| self.engine.commit(preview));
        result.map(|(plan, report)| (plan, report, receipt))
    }

    /// The asset hashes the undo and redo entries mention (kept by the purge).
    fn undo_assets(&self) -> Result<BTreeSet<ew_schema::Sha256>, PurgeError> {
        self.engine
            .history()
            .asset_mentions()
            .map_err(|e| PurgeError::Io {
                path: "the undo history".to_owned(),
                error: std::io::Error::other(e.to_string()),
            })
    }

    /// Refuses the preview of `project.assets.purge` when the purge would be refused.
    fn purge_gate(&self, name: &str) -> Result<(), CmdError> {
        if name != PurgeAssets::NAME {
            return Ok(());
        }
        self.purge_preview().map(|_| ()).map_err(|e| CmdError::Rejected {
            source: PurgeAssets::NAME.to_owned(),
            reason: e.to_string(),
            text: Some(purge_refusal(&e)),
        })
    }

    /// [`Session::commit`] of a `project.assets.purge` preview: the purge with the listed hashes.
    fn commit_purge(&mut self, preview: Preview) -> Result<Receipt, CmdError> {
        let rejected = |e: PurgeError| CmdError::Rejected {
            source: PurgeAssets::NAME.to_owned(),
            reason: e.to_string(),
            text: Some(purge_refusal(&e)),
        };
        let cmd: PurgeAssets = serde_json::from_value(preview.command().params.clone()).map_err(|e| {
            CmdError::invalid_params(
                PurgeAssets::NAME,
                "",
                api_text!("refusal-params-detail", e.to_string()),
            )
        })?;
        let listed = cmd.listed()?;
        // `HumanOnly`, judged by the preview's own origin too (the session's is judged in `purge_files`), so that the
        // guard does not rest on the engine refusing a non-human preview before it exists.
        let exposure = <PurgeAssets as ew_commands::Command>::meta().exposure;
        if !exposure.allows(preview.origin()) {
            return Err(CmdError::Exposure {
                command: PurgeAssets::NAME.into(),
                exposure,
                origin: preview.origin().clone(),
            });
        }
        drop(preview);
        match self.purge_files(listed.as_ref()) {
            Ok((_, _, receipt)) => receipt,
            Err(e) => Err(rejected(e)),
        }
    }

    /// The project directory's store, if the session has one.
    pub fn store(&self) -> Option<&Store> {
        self.persistence.as_ref().map(|p| &p.store)
    }

    /// Why [`Recovery::Apply`] did not replay the crashed session's journal at [`Session::open`]: the base moved and
    /// both sides changed a field ([`RecoverError::Replay`] with [`ReplayError::Collisions`]), the journal does not
    /// replay, or it could not be journalled again. `None` when it replayed, or with another [`Recovery`].
    pub fn recovery_refusal(&self) -> Option<&RecoverError> {
        self.persistence.as_ref().and_then(|p| p.refusal.as_ref())
    }

    /// "Discard" after the user confirmed it (`ew recover --discard`): deletes the summarised journal of a crashed
    /// session ([`OpenReport::recovery`]) and only that one; the other crashed sessions' journals stay and are offered
    /// at the next open, one at a time (ruling 2 of cycle 41). Returns whether a journal was deleted.
    pub fn discard_recovery(&mut self) -> bool {
        let Some(p) = self.persistence.as_mut() else {
            return false;
        };
        if p.store.recovery().is_none() {
            return false;
        }
        p.store.discard_recovery();
        p.refusal = None;
        true
    }

    /// Saves, then closes (`ew recover --replay`): the clean close follows a save, and a save that passed its commit
    /// point ([`SaveError::Incomplete`] of this save: decided, it completes by roll-forward at the next open). After
    /// any other failure the session is dropped without its clean close, which would delete its journal: the journal,
    /// which holds every unsaved transaction (a replayed journal's among them), stays and is offered at the next open
    /// (doc 05 §8.3); the lock is released.
    ///
    /// # Errors
    ///
    /// The [`SaveError`] of [`Session::save`].
    pub fn save_and_close(mut self) -> Result<SaveReport, SaveError> {
        let result = self.save();
        if result.is_ok() || !self.is_dirty() {
            self.close();
        }
        result
    }

    /// A clean close (doc 05 §8.1): deletes this session's journal and releases the lock. Unsaved changes
    /// are discarded; save first to keep them.
    pub fn close(self) {
        if let Some(p) = self.persistence {
            p.store.close();
        }
    }
}
