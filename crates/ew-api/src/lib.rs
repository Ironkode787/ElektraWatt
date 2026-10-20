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
//! Not in v0: the memo snapshot in [`World`] and the job pool for previews (WP-S1.05), rule registration
//! (WP-S1.15), export jobs, the save on a writer thread, and the undo history of a recovered session
//! (FR-COL-028: a recovered session opens dirty with an empty history).
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

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use ew_derive::{DeviceTag, FrameGridOf, FrameSettings, NumberingSettingsOf, Oracle, Q, ReportPagesProvider};
use ew_model::{AnyKey, Model, ModelError, PreventHook};
use ew_schema::{LibDocument, Project, Record};
use ew_store::{
    Changes, LockState, OpenError, OpenReport, RecoverError, SaveError, SaveReport, Store, StoreOptions,
};

pub use ew_derive::{DeriveNote, MemoStore, Mismatch, NoteSeverity};

pub use ew_commands::{
    AnyCommand, Change, CmdError, CommandRegistry, DynCommand, History, HistoryLimits, IdSource, Origin,
    Preview, Receipt, Restored, Submit, TxContext, World,
};

use ew_commands::{Engine, EngineOptions};

/// Options of a session.
pub struct SessionOptions {
    /// The origin of the transactions this front-end submits (GUI, CLI subcommand, MCP client, …).
    pub origin: Origin,
    /// The bounds of the undo stack.
    pub history: HistoryLimits,
    /// The source of new IDs; the system clock and OS entropy when `None`. Tests pass a seeded one.
    pub ids: Option<Box<dyn IdSource + Send>>,
    /// The prevent-rule hook (`ew-checks` from WP-S1.15); none when `None`.
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
                    prevent: options.prevent,
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
    pub fn preview(&mut self, cmd: &dyn DynCommand) -> Result<Preview, CmdError> {
        self.engine.preview(cmd)
    }

    /// [`Session::preview`] with an explicit submission.
    pub fn preview_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Preview, CmdError> {
        self.engine.preview_with(cmd, submit)
    }

    /// Commits a preview after the rebase check; re-runs the command if its reads changed.
    pub fn commit(&mut self, preview: Preview) -> Result<Receipt, CmdError> {
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
    /// [`ReadOnlyReason::Requested`](ew_store::ReadOnlyReason::Requested). After the commit point of this
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
