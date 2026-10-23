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
//! Not in v0: the memo snapshot in [`World`] and the job pool for previews (WP-S1.05), rule registration
//! (WP-S1.15), progress and cancellation of export jobs (`ew-jobs`), the save on a writer thread, and the undo
//! history of a recovered session (FR-COL-028: a recovered session opens dirty with an empty history).
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
//! session.undo().unwrap();
//! assert!(!session.is_dirty());
//! ```

#![forbid(unsafe_code)]

mod cmdlog;
pub mod import;
mod new_project;
pub mod output;
pub mod verify;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use ew_derive::{
    DeviceTag, FrameGridOf, FrameSettings, NumberingSettingsOf, Oracle, PageOrder, Q, ReportPagesProvider,
};
use ew_model::{AnyKey, Model, ModelError, PreventHook};
use ew_schema::{LibDocument, Project, Record};
use ew_store::{Changes, LockState, RecoverError, SaveReport, Store};

pub use cmdlog::{CmdLog, CmdLogError, SCHEMA as CMDLOG_SCHEMA, ScriptDiff};
/// The check engine (`ew check`, the message panel, the migration report): rules, schemes, findings.
pub use ew_checks as checks;
pub use ew_derive::{BomMode, DeriveNote, MemoStore, Mismatch, NoteSeverity};
/// The row sources of `ew export cables|bom` (doc 17 §3.3): what to export and why it could not.
pub use ew_reports::export::{Export, ExportError, Exported};
/// Assets (doc 05 §13.3): the reference a record stores, the sniffer and the base64 form of command parameters.
pub use ew_schema::{
    AssetError, AssetFormat, AssetRef, MAX_LOGO_BYTES, Sha256, base64_encode, sniff as sniff_asset,
};
/// The read-only SQL projection (`ew sql`, the acceptance SQL, ADR-0017): views, sandbox, results.
pub use ew_sqlview as sql;
pub use ew_store::{
    Access, OpenError, OpenReport, PurgeError, PurgeFile, PurgePlan, ReadOnlyReason, SaveError, StoreOptions,
};
pub use new_project::{NewProjectKind, ProjectTemplate};

pub use ew_commands::{
    AnyCommand, Change, ChangeKind, CmdError, CommandRegistry, DerivedChange, DerivedDiff, DynCommand,
    EntityChange, History, HistoryLimits, IdSource, ModelDiff, Origin, Preview, Receipt, Restored, Submit,
    TxContext, World,
};

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
}

/// How a session's derived values are evaluated (ADR-0002 decisions 2–3, amendment 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MemoMode {
    /// Every [`Session::query`] is cold (request-scoped cache only).
    Off,
    /// The `Memo` and `MemoLru` families are memoised in the session's [`MemoStore`], advanced with push
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
        }
    }
}

impl std::fmt::Debug for SessionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionOptions")
            .field("origin", &self.origin)
            .field("history", &self.history)
            .field("memo", &self.memo)
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
    q.diagnostics()
        .notes()
        .into_iter()
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
            }),
        }
    }
}

/// The integrity findings of opening a project as items for [`checks::CheckReport::with_integrity`]: every
/// store finding with a catalogue ID (EW-CORE-001 … 019, doc 05 §6.3) at its severity, located by its
/// project-relative file and line. Notices without an ID (line endings, foreign edits) are left out.
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
        let before = self.engine.model().clone();
        let result = self.engine.execute(cmd);
        self.committed(&before, cmd.name());
        result
    }

    /// [`Session::execute`] with an explicit origin, editor context or coalescing key.
    pub fn execute_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Receipt, CmdError> {
        let before = self.engine.model().clone();
        let result = self.engine.execute_with(cmd, submit);
        self.committed(&before, cmd.name());
        result
    }

    /// Prepares a command on a fork of the current snapshot: the candidate model, its ops, the read set,
    /// the entity and field diff and the affected pages. Nothing is committed. Synchronous in v0.
    ///
    /// `project.assets.purge` is refused here already when the purge would be refused (its file list is
    /// [`Session::purge_preview`]).
    pub fn preview(&mut self, cmd: &dyn DynCommand) -> Result<Preview, CmdError> {
        self.purge_gate(cmd.name())?;
        self.engine.preview(cmd)
    }

    /// [`Session::preview`] with an explicit submission.
    pub fn preview_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Preview, CmdError> {
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
    pub fn undo(&mut self) -> Result<Restored, CmdError> {
        let before = self.engine.model().clone();
        let result = self.engine.undo();
        self.committed(&before, "undo");
        result
    }

    /// Redoes the next step as a new revision.
    pub fn redo(&mut self) -> Result<Restored, CmdError> {
        let before = self.engine.model().clone();
        let result = self.engine.redo();
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

    /// Marks the current state as saved (the store calls this after a save, WP-S1.09).
    pub fn mark_saved(&mut self) {
        self.engine.mark_saved();
    }

    /// Whether the model differs from the last saved state.
    pub fn is_dirty(&self) -> bool {
        self.engine.is_dirty() || self.persistence.as_ref().is_some_and(|p| p.recovered)
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
            // ever added): journal it, so that a crash before the save does not lose its bytes (FT-337).
            let arrived = self.engine.model().assets().count() != before.assets().count();
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
                    // Collisions or an unreadable journal: the journal is kept and the disk state opens.
                    Err(RecoverError::Nothing | RecoverError::Replay(_) | RecoverError::Io(_)) => {}
                },
            }
        }
        let mut session = Session::new(model, options);
        session.persistence = Some(Persistence {
            store,
            unsaved,
            recovered,
        });
        Ok((session, report))
    }

    /// Opens a project directory read-only for a reader such as `ew check` (doc 17 §3.1: readers take no
    /// lock and read one generation): no lock, no roll-forward, no journal. `app_version` names the program
    /// in the store's diagnostics.
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
    /// [`MemoMode::Oracle`]).
    ///
    /// # Errors
    ///
    /// [`OpenError`] when the project cannot be opened; integrity findings are in the report.
    pub fn open_reader_with(
        dir: &Path,
        app_version: &str,
        subcommand: &str,
        memo: MemoMode,
    ) -> Result<(Session, OpenReport), OpenError> {
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
        let member = match mode {
            checks::CheckMode::Release => "release_scheme",
            checks::CheckMode::Online => "active_scheme",
        };
        let q = self.query();
        let name = q
            .project()
            .settings
            .extra
            .get(ew_commands::commands::CHECKS_MEMBER)?
            .get(member)?
            .as_str()?;
        checks::scheme(name)
    }

    /// Runs the check scheme `scheme` in `mode` over the current snapshot through the session's memo store
    /// (doc 15 §3): the findings, the totals and `ew check`'s exit code ([`checks::CheckReport`]).
    pub fn check(&self, scheme: &checks::Scheme, mode: checks::CheckMode) -> checks::CheckReport {
        checks::run(&self.query(), scheme, mode)
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
    /// it as the session's project (Save As, doc 05 §5.5).
    ///
    /// # Errors
    ///
    /// [`OpenError`] when the directory holds a project, the lock is held or the write fails.
    pub fn create(&mut self, dir: &Path, store: StoreOptions) -> Result<(), OpenError> {
        let model = self.engine.model().clone();
        let store = Store::create(dir, &model, store)?;
        self.persistence = Some(Persistence {
            store,
            unsaved: BTreeSet::new(),
            recovered: false,
        });
        self.engine.mark_saved();
        Ok(())
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
        })
    }

    /// [`Session::commit`] of a `project.assets.purge` preview: the purge with the listed hashes.
    fn commit_purge(&mut self, preview: Preview) -> Result<Receipt, CmdError> {
        let rejected = |reason: String| CmdError::Rejected {
            source: PurgeAssets::NAME.to_owned(),
            reason,
        };
        let cmd: PurgeAssets = serde_json::from_value(preview.command().params.clone())
            .map_err(|e| CmdError::invalid_params(PurgeAssets::NAME, "", e.to_string()))?;
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
            Err(e) => Err(rejected(e.to_string())),
        }
    }

    /// The project directory's store, if the session has one.
    pub fn store(&self) -> Option<&Store> {
        self.persistence.as_ref().map(|p| &p.store)
    }

    /// A clean close (doc 05 §8.1): deletes this session's journal and releases the lock. Unsaved changes
    /// are discarded; save first to keep them.
    pub fn close(self) {
        if let Some(p) = self.persistence {
            p.store.close();
        }
    }
}
