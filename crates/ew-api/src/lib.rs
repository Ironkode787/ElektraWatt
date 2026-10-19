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
//! Not in v0: persistence and the journal (WP-S1.09), the memo snapshot in [`World`] and the job pool for
//! previews (WP-S1.05), rule registration (WP-S1.15) and export jobs.
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

use std::sync::Arc;

use ew_derive::{DeviceTag, FrameGridOf, FrameSettings, NumberingSettingsOf, Q, ReportPagesProvider};
use ew_model::{Model, ModelError, PreventHook};
use ew_schema::{LibDocument, Project, Record};

pub use ew_derive::{DeriveNote, NoteSeverity};

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
}

impl Default for SessionOptions {
    fn default() -> SessionOptions {
        SessionOptions {
            origin: Origin::Gui,
            history: HistoryLimits::default(),
            ids: None,
            prevent: None,
        }
    }
}

impl std::fmt::Debug for SessionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionOptions")
            .field("origin", &self.origin)
            .field("history", &self.history)
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

/// One open project: the facade every front-end (GUI, `ew` CLI, scripts, MCP) talks to.
pub struct Session {
    engine: Engine,
    load_findings: Vec<DerivationFinding>,
    /// The report engine installed on every `Q` the session builds (WP-S3.03).
    reports: Arc<dyn ReportPagesProvider>,
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
        Session {
            load_findings,
            reports: ew_reports::engine(),
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
    pub fn query(&self) -> Q {
        self.query_over(&self.engine.world().model)
    }

    /// [`Session::query`] over another snapshot (a preview's candidate model, an export fork).
    pub fn query_over(&self, model: &Model) -> Q {
        Q::new(model).with_report_pages(Arc::clone(&self.reports))
    }

    /// Runs an interactive command through the commit pipeline (ARCH §5.2) from the session's origin.
    /// A bulk command is refused with [`CmdError::NeedsPreview`].
    pub fn execute(&mut self, cmd: &dyn DynCommand) -> Result<Receipt, CmdError> {
        self.engine.execute(cmd)
    }

    /// [`Session::execute`] with an explicit origin, editor context or coalescing key.
    pub fn execute_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Receipt, CmdError> {
        self.engine.execute_with(cmd, submit)
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
        self.engine.commit(preview)
    }

    /// Undoes the last step as a new revision; returns the editor context to restore.
    pub fn undo(&mut self) -> Result<Restored, CmdError> {
        self.engine.undo()
    }

    /// Redoes the next step as a new revision.
    pub fn redo(&mut self) -> Result<Restored, CmdError> {
        self.engine.redo()
    }

    /// The undo stack.
    pub fn history(&self) -> &History {
        self.engine.history()
    }

    /// Takes the change-feed entries since the last call.
    pub fn drain_changes(&mut self) -> Vec<Change> {
        self.engine.drain_changes()
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
        self.engine.is_dirty()
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
}
