// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The commit pipeline (ARCH §5.2), undo and redo (§5.3) and previews (§5.4): the engine behind
//! `ew_api::Session`.
//!
//! ```text
//! resolve   an AnyCommand is validated once against the engine's registry into its typed command
//! run       Command::run against a TxBuilder on the current snapshot (exposure checked first)
//! normalise the normaliser registry, passes until quiet: at most three productive passes and a
//!           confirming one (D1)
//! validate  Model::apply_with: structural invariants, then the PreventHook
//! commit    swap in the candidate Model, Rev + 1, push to History (coalescing)
//! notify    append a Change {rev, touched keys, touched pages} to the change feed
//! ```
//!
//! The engine is the only place that constructs a [`TxBuilder`]. It runs synchronously: the job pool of
//! WP-S1.05 will run previews on forks, with the same [`Preview`] and rebase check.
//!
//! **Prevent rules.** The hook is a `Box<dyn PreventHook>`; the default [`NoPrevent`] accepts everything,
//! because no prevent rule exists yet. `ew-checks` plugs in later: its hook builds the tracked query context
//! of `ew-derive` from the candidate and evaluates the project's prevent rules in its strictness (ADR-0009).

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use ew_ids::{Id, IdSource, SystemSource};
use ew_model::{AnyKey, AnyRow, ApplyOptions, Model, Op, PreventHook, PreventViolation, Rev, invert_batch};
use ew_schema::{Page, Representation};

use crate::command::{DynCommand, Outcome};
use crate::diff::ModelDiff;
use crate::error::CmdError;
use crate::history::{History, HistoryLimits};
use crate::meta::{CommandMeta, Cost, Origin};
use crate::normalise::{Normaliser, normalise};
use crate::registry::{AnyCommand, CommandRegistry};
use crate::transaction::{Submit, Transaction, TxContext};
use crate::tx::{ReadSet, TxBuilder};

/// The prevent hook used when none is plugged in: accepts every candidate.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPrevent;

impl PreventHook for NoPrevent {
    fn check(&self, _base: &Model, _candidate: &Model, _touched: &[AnyKey]) -> Result<(), PreventViolation> {
        Ok(())
    }
}

/// The current snapshot handed to readers: O(1) to take (ARCH §5.1 `Session::world`). The memo snapshot
/// joins it with WP-S1.05.
#[derive(Clone, Debug)]
pub struct World {
    /// The model.
    pub model: Model,
    /// Its revision.
    pub rev: Rev,
}

/// One entry of the change feed (ARCH §5.2 step 5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Change {
    /// The revision that made the change.
    pub rev: Rev,
    /// The keys of every row put, in key order.
    pub touched_keys: Vec<AnyKey>,
    /// The pages whose content changed, in ID order: pages put, pages of representations, connections,
    /// junctions and embedded report blocks put (old and new rows), and the pages of every representation
    /// of a function or device put.
    pub touched_pages: Vec<Id<Page>>,
}

/// What a committed command reports.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Receipt {
    /// The new revision; the current one when the command changed nothing.
    pub rev: Rev,
    /// The command's outcome.
    pub outcome: Outcome,
    /// Every key the transaction inserted (command and normalisers), in insertion order.
    pub created: Vec<AnyKey>,
    /// The number of normaliser passes (1: the command's output was already normal).
    pub passes: usize,
}

/// What undo or redo restored.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Restored {
    /// The new revision.
    pub rev: Rev,
    /// The editor state recorded with the step, to restore.
    pub context: TxContext,
    /// The step's label.
    pub label: String,
}

/// The source of engine identities: every [`Engine`] gets a fresh one at construction.
static NEXT_ENGINE: AtomicU64 = AtomicU64::new(1);

/// The snapshot a [`Preview`] was computed on: the engine that computed it, that engine's model
/// generation and the base revision. A preview is committed as prepared only on the same engine and
/// generation (F4); otherwise its command is re-run on the committing engine's current model.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PreviewBase {
    engine: u64,
    generation: u64,
    rev: Rev,
}

/// A prepared transaction on a fork (ADR-0008 "preview is a fork"): the candidate snapshot, its ops, the
/// read set for the rebase check, the entity and field diff and the affected pages. Nothing is committed
/// until [`Engine::commit`].
///
/// A preview remembers the engine and the model generation it was computed on. [`Engine::commit`] uses
/// its candidate and ops only on that engine; a preview from another engine or session, or from before
/// the model was replaced, is re-run on the current model.
///
/// The prepared ops are not public (F9): they carry whole rows, and the crates above `ew-commands` read
/// records only through tracked queries (ARCH §3 rule 1). [`Preview::diff`] summarises them.
#[derive(Clone, Debug)]
pub struct Preview {
    candidate: Model,
    ops: Vec<Op>,
    read_set: ReadSet,
    diff: ModelDiff,
    affected_pages: Vec<Id<Page>>,
    base: PreviewBase,
    command: AnyCommand,
    meta: CommandMeta,
    submit: Submit,
    outcome: Outcome,
    created: Vec<AnyKey>,
    passes: usize,
}

impl Preview {
    /// The candidate snapshot.
    pub fn candidate(&self) -> &Model {
        &self.candidate
    }

    /// The prepared ops (feature `testkit` only: they carry whole rows).
    #[cfg(feature = "testkit")]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// The number of prepared ops (one per changed key).
    pub fn op_count(&self) -> usize {
        self.ops.len()
    }

    /// The base reads of the command and the normalisers.
    pub fn read_set(&self) -> &ReadSet {
        &self.read_set
    }

    /// The entity and field changes (summaries: change kind, key, changed member paths).
    pub fn diff(&self) -> &ModelDiff {
        &self.diff
    }

    /// The pages whose content changes.
    pub fn affected_pages(&self) -> &[Id<Page>] {
        &self.affected_pages
    }

    /// The revision of the snapshot the preview ran on.
    pub fn base_rev(&self) -> Rev {
        self.base.rev
    }

    /// The command in its JSON form.
    pub fn command(&self) -> &AnyCommand {
        &self.command
    }

    /// The command's outcome.
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// The keys the transaction would insert.
    pub fn created(&self) -> &[AnyKey] {
        &self.created
    }
}

/// Options of [`Engine::new`].
pub struct EngineOptions {
    /// The bounds of the undo stack.
    pub history: HistoryLimits,
    /// The origin of submissions that name none (`execute`, `preview`).
    pub origin: Origin,
    /// The source of new IDs; the system clock and OS entropy when `None`.
    pub ids: Option<Box<dyn IdSource + Send>>,
    /// The prevent hook; [`NoPrevent`] when `None`.
    pub prevent: Option<Box<dyn PreventHook>>,
    /// The command registry; the built-in one when `None`.
    pub registry: Option<CommandRegistry>,
}

impl Default for EngineOptions {
    fn default() -> EngineOptions {
        EngineOptions {
            history: HistoryLimits::default(),
            origin: Origin::Gui,
            ids: None,
            prevent: None,
            registry: None,
        }
    }
}

impl std::fmt::Debug for EngineOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineOptions")
            .field("history", &self.history)
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

/// A transaction prepared on the current snapshot.
struct Prepared {
    candidate: Model,
    ops: Vec<Op>,
    read_set: ReadSet,
    outcome: Outcome,
    created: Vec<AnyKey>,
    passes: usize,
}

/// The editing engine: the current model, the history, the registry, the ID source, the prevent hook,
/// the normalisers and the change feed.
pub struct Engine {
    /// This engine's identity (from [`NEXT_ENGINE`]).
    id: u64,
    /// Bumped whenever the model is replaced other than by applying a batch, so that a preview of the
    /// old model is never committed as prepared. v0 has no such replacement yet (reload is WP-S1.09).
    generation: u64,
    model: Model,
    history: History,
    registry: CommandRegistry,
    ids: Box<dyn IdSource + Send>,
    prevent: Box<dyn PreventHook>,
    normalisers: Vec<Box<dyn Normaliser>>,
    changes: Vec<Change>,
    origin: Origin,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("rev", &self.model.rev())
            .field("history", &self.history.entries().len())
            .field("changes", &self.changes.len())
            .finish_non_exhaustive()
    }
}

impl Engine {
    /// An engine over a loaded model.
    pub fn new(model: Model, options: EngineOptions) -> Engine {
        Engine {
            id: NEXT_ENGINE.fetch_add(1, Ordering::Relaxed),
            generation: 0,
            model,
            history: History::new(options.history),
            registry: options
                .registry
                .unwrap_or_else(|| CommandRegistry::builtin().clone()),
            ids: options.ids.unwrap_or_else(|| Box::new(SystemSource)),
            prevent: options.prevent.unwrap_or_else(|| Box::new(NoPrevent)),
            normalisers: crate::normalise::registry(),
            changes: Vec::new(),
            origin: options.origin,
        }
    }

    /// The current snapshot.
    pub fn world(&self) -> World {
        World {
            model: self.model.clone(),
            rev: self.model.rev(),
        }
    }

    /// The current model.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// The command registry.
    pub fn registry(&self) -> &CommandRegistry {
        &self.registry
    }

    /// The undo stack.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Replaces the prevent hook.
    pub fn set_prevent_hook(&mut self, hook: Box<dyn PreventHook>) {
        self.prevent = hook;
    }

    /// Marks the current state as saved (the dirty flag, ARCH §5.3).
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// Whether the model differs from the last saved state.
    pub fn is_dirty(&self) -> bool {
        self.history.is_dirty()
    }

    /// Takes the change feed's entries since the last call.
    pub fn drain_changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
    }

    /// A submission from the engine's default origin.
    fn default_submit(&self) -> Submit {
        Submit::from(self.origin.clone())
    }

    /// Resolves an untyped command against this engine's registry.
    pub fn resolve(&self, cmd: &AnyCommand) -> Result<Box<dyn DynCommand>, CmdError> {
        self.registry.resolve(cmd)
    }

    /// The typed command behind a submission: an [`AnyCommand`] is validated once against **this**
    /// engine's registry and deserialised (F6, F17); a typed command is used as it is.
    fn typed<'c>(&self, cmd: &'c dyn DynCommand) -> Result<Typed<'c>, CmdError> {
        match cmd.as_any_command() {
            Some(any) => self.registry.resolve(any).map(Typed::Owned),
            None => Ok(Typed::Borrowed(cmd)),
        }
    }

    fn base(&self) -> PreviewBase {
        PreviewBase {
            engine: self.id,
            generation: self.generation,
            rev: self.model.rev(),
        }
    }

    /// Runs, normalises and validates a command on the current snapshot.
    fn prepare(&mut self, cmd: &dyn DynCommand, submit: &Submit) -> Result<Prepared, CmdError> {
        let meta = cmd.meta();
        if !meta.exposure.allows(&submit.origin) {
            return Err(CmdError::Exposure {
                command: meta.name.into(),
                exposure: meta.exposure,
                origin: submit.origin.clone(),
            });
        }
        let base = &self.model;
        let mut tx = TxBuilder::new(base, &mut *self.ids);
        tx.set_source(meta.name);
        let mut outcome = cmd.run(&mut tx)?;
        let passes = normalise(&mut tx, &self.normalisers)?;
        // the normalisers' notices follow the command's own
        for notice in tx.take_notices() {
            if !outcome.notices.contains(&notice) {
                outcome.notices.push(notice);
            }
        }
        let (ops, read_set, created) = tx.finish();
        let candidate = if ops.is_empty() {
            base.clone()
        } else {
            base.apply_with(
                &ops,
                base.rev().next(),
                &ApplyOptions {
                    prevent: Some(&*self.prevent),
                },
            )?
        };
        Ok(Prepared {
            candidate,
            ops,
            read_set,
            outcome,
            created,
            passes,
        })
    }

    /// Swaps in a prepared candidate, records the transaction and notifies.
    fn finalize(
        &mut self,
        prepared: Prepared,
        command: AnyCommand,
        meta: CommandMeta,
        submit: Submit,
    ) -> Receipt {
        let Prepared {
            candidate,
            ops,
            outcome,
            created,
            passes,
            ..
        } = prepared;
        let changed = !ops.is_empty();
        let rev = if changed {
            candidate.rev()
        } else {
            self.model.rev()
        };
        if changed {
            let change = Change {
                rev,
                touched_keys: ops.iter().map(Op::key).collect(),
                touched_pages: affected_pages(&self.model, &candidate, &ops),
            };
            self.model = candidate;
            self.changes.push(change);
        }
        if changed || !meta.undoable {
            self.history.push(
                Transaction {
                    ops,
                    command,
                    origin: submit.origin,
                    context: submit.context,
                    coalesce_key: submit.coalesce_key,
                    undoable: meta.undoable,
                    label: meta.name.into(),
                },
                rev,
            );
        }
        Receipt {
            rev,
            outcome,
            created,
            passes,
        }
    }

    /// Runs an interactive command through the whole pipeline (ARCH §5.2) from the default origin.
    pub fn execute(&mut self, cmd: &dyn DynCommand) -> Result<Receipt, CmdError> {
        let submit = self.default_submit();
        self.execute_with(cmd, submit)
    }

    /// Runs an interactive command through the whole pipeline with an explicit submission. A
    /// `Cost::Bulk` command is refused with [`CmdError::NeedsPreview`]: bulk commands always preview
    /// (ADR-0008).
    pub fn execute_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Receipt, CmdError> {
        let typed = self.typed(cmd)?;
        let cmd = typed.get();
        if cmd.cost(&self.model) == Cost::Bulk {
            return Err(CmdError::NeedsPreview {
                command: cmd.name().into(),
            });
        }
        let prepared = self.prepare(cmd, &submit)?;
        let command = AnyCommand::of(cmd)?;
        Ok(self.finalize(prepared, command, cmd.meta(), submit))
    }

    /// Prepares a command on a fork of the current snapshot from the default origin (synchronously in v0).
    pub fn preview(&mut self, cmd: &dyn DynCommand) -> Result<Preview, CmdError> {
        let submit = self.default_submit();
        self.preview_with(cmd, submit)
    }

    /// Prepares a command on a fork of the current snapshot with an explicit submission.
    pub fn preview_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Preview, CmdError> {
        let typed = self.typed(cmd)?;
        let cmd = typed.get();
        let base = self.base();
        let prepared = self.prepare(cmd, &submit)?;
        let diff = ModelDiff::of_ops(&prepared.ops)?;
        let affected = affected_pages(&self.model, &prepared.candidate, &prepared.ops);
        Ok(Preview {
            candidate: prepared.candidate,
            ops: prepared.ops,
            read_set: prepared.read_set,
            diff,
            affected_pages: affected,
            base,
            command: AnyCommand::of(cmd)?,
            meta: cmd.meta(),
            submit,
            outcome: prepared.outcome,
            created: prepared.created,
            passes: prepared.passes,
        })
    }

    /// Commits a preview after the rebase check (ADR-0008): on an unchanged snapshot the candidate is
    /// swapped in; if the snapshot moved on but none of the preview's reads changed, the prepared ops are
    /// applied to the current snapshot; otherwise (or if that apply fails) the command is re-run
    /// deterministically on the current snapshot.
    ///
    /// The first two paths require the preview to come from this engine and model generation (F4). A
    /// preview from another engine or session, or from before the model was replaced, always takes the
    /// re-run path: its command is validated against this engine's registry and run on the current model,
    /// so a foreign candidate never replaces the model.
    ///
    /// The commit is submitted as the preview was for a preview of this engine and generation; a foreign
    /// preview is committed from this engine's default origin (its own origin and context mean nothing
    /// here), so exposure is judged by the committing side. [`Engine::commit_with`] names the submission.
    pub fn commit(&mut self, preview: Preview) -> Result<Receipt, CmdError> {
        let ours = preview.base.engine == self.id && preview.base.generation == self.generation;
        let submit = if ours {
            preview.submit.clone()
        } else {
            self.default_submit()
        };
        self.commit_with(preview, submit)
    }

    /// [`Engine::commit`] with an explicit submission of the committing side: the exposure of the command
    /// is judged by `submit.origin` (also on the paths that apply a prepared candidate, where the
    /// preview's own origin is not trusted), and `submit` is what the history step records.
    pub fn commit_with(&mut self, preview: Preview, submit: Submit) -> Result<Receipt, CmdError> {
        let Preview {
            candidate,
            ops,
            read_set,
            base,
            command,
            meta,
            outcome,
            created,
            passes,
            ..
        } = preview;
        let base_rev = base.rev;
        let ours = base.engine == self.id && base.generation == self.generation;
        if ours && !meta.exposure.allows(&submit.origin) {
            return Err(CmdError::Exposure {
                command: meta.name.into(),
                exposure: meta.exposure,
                origin: submit.origin,
            });
        }
        if ours && base_rev == self.model.rev() {
            let prepared = Prepared {
                candidate,
                ops,
                read_set,
                outcome,
                created,
                passes,
            };
            return Ok(self.finalize(prepared, command, meta, submit));
        }
        if ours && read_set.unchanged_since(&self.model, base_rev) {
            let applied = if ops.is_empty() {
                Ok(self.model.clone())
            } else {
                self.model.apply_with(
                    &ops,
                    self.model.rev().next(),
                    &ApplyOptions {
                        prevent: Some(&*self.prevent),
                    },
                )
            };
            if let Ok(candidate) = applied {
                let prepared = Prepared {
                    candidate,
                    ops,
                    read_set,
                    outcome,
                    created,
                    passes,
                };
                return Ok(self.finalize(prepared, command, meta, submit));
            }
        }
        // Re-run. Our own preview's command came from a typed command of this registry: deserialise it
        // without validating it again. A foreign one is validated like any untyped submission.
        let cmd = if ours {
            self.registry.parse_trusted(&command)?
        } else {
            self.registry.resolve(&command)?
        };
        let meta = if ours { meta } else { cmd.meta() };
        let prepared = self.prepare(cmd.as_ref(), &submit)?;
        Ok(self.finalize(prepared, command, meta, submit))
    }

    /// Reverts the step below the cursor by applying its inverse ops as a new revision (ARCH §5.3).
    /// Structural invariants are checked; prevent rules are not (the restored state passed them).
    pub fn undo(&mut self) -> Result<Restored, CmdError> {
        let index = self.history.undo_index()?;
        let entry = self.history.entry(index).expect("undo index is in range");
        let tx = entry.tx().clone();
        let inverse = invert_batch(&tx.ops);
        let rev = self.apply_history_ops(&inverse, &tx.label)?;
        self.history.undone(index);
        Ok(Restored {
            rev,
            context: tx.context,
            label: tx.label,
        })
    }

    /// Re-applies the step above the cursor as a new revision.
    pub fn redo(&mut self) -> Result<Restored, CmdError> {
        let index = self.history.redo_index()?;
        let entry = self.history.entry(index).expect("redo index is in range");
        let tx = entry.tx().clone();
        let rev = self.apply_history_ops(&tx.ops, &tx.label)?;
        self.history.redone(index);
        Ok(Restored {
            rev,
            context: tx.context,
            label: tx.label,
        })
    }

    fn apply_history_ops(&mut self, ops: &[Op], label: &str) -> Result<Rev, CmdError> {
        let rev = self.model.rev().next();
        let next = self.model.apply(ops, rev).map_err(|e| CmdError::History {
            reason: format!("the model changed under the step `{label}`: {e}"),
        })?;
        let change = Change {
            rev,
            touched_keys: ops
                .iter()
                .map(Op::key)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            touched_pages: affected_pages(&self.model, &next, ops),
        };
        self.model = next;
        self.changes.push(change);
        Ok(rev)
    }
}

/// A submitted command or the typed command an [`AnyCommand`] resolved to.
enum Typed<'c> {
    Borrowed(&'c dyn DynCommand),
    Owned(Box<dyn DynCommand>),
}

impl Typed<'_> {
    fn get(&self) -> &dyn DynCommand {
        match self {
            Typed::Borrowed(c) => *c,
            Typed::Owned(c) => c.as_ref(),
        }
    }
}

/// The pages whose content a batch changes, in ID order (see [`Change::touched_pages`]).
pub fn affected_pages(before: &Model, after: &Model, ops: &[Op]) -> Vec<Id<Page>> {
    let mut pages: BTreeSet<Id<Page>> = BTreeSet::new();
    let reps_pages = |m: &Model, f: Id<ew_schema::Function>, out: &mut BTreeSet<Id<Page>>| {
        for r in m.indexes().reps_by_function().get(&f) {
            if let Some(rep) = m.table::<Representation>().get(r) {
                out.insert(rep.page);
            }
        }
    };
    let endpoint_pages = |e: &ew_schema::Endpoint, out: &mut BTreeSet<Id<Page>>| match e {
        ew_schema::Endpoint::Cp(f, _) => {
            reps_pages(before, *f, out);
            reps_pages(after, *f, out);
        }
        ew_schema::Endpoint::Junction(j) => {
            for m in [before, after] {
                if let Some(j) = m.table::<ew_schema::Junction>().get(j) {
                    out.insert(j.page);
                }
            }
        }
        _ => {}
    };
    let connection_page = |id: &Id<ew_schema::Connection>, out: &mut BTreeSet<Id<Page>>| {
        for m in [before, after] {
            if let Some(c) = m.table::<ew_schema::Connection>().get(id) {
                out.extend(c.page);
            }
        }
    };
    let device_pages = |d: Id<ew_schema::Device>, out: &mut BTreeSet<Id<Page>>| {
        for m in [before, after] {
            for f in m.indexes().functions_by_device().get(&d) {
                reps_pages(m, *f, out);
            }
        }
    };
    for op in ops {
        for row in [op.before(), op.after()].into_iter().flatten() {
            match row {
                AnyRow::PotentialDef(d) => endpoint_pages(&d.at, &mut pages),
                AnyRow::Bond(b) => {
                    endpoint_pages(&b.a, &mut pages);
                    endpoint_pages(&b.b, &mut pages);
                    if let Some(via) = &b.via {
                        connection_page(via, &mut pages);
                    }
                }
                AnyRow::Conductor(c) => {
                    device_pages(c.cable, &mut pages);
                    // The connection the conductor is assigned to: a core connection naming it.
                    for m in [before, after] {
                        for (id, conn) in m.table::<ew_schema::Connection>().iter() {
                            if conn.kind == ew_schema::ConnectionKind::Core(c.id) {
                                connection_page(id, &mut pages);
                            }
                        }
                    }
                }
                AnyRow::Page(p) => {
                    pages.insert(p.id);
                }
                AnyRow::Representation(r) => {
                    pages.insert(r.page);
                }
                AnyRow::Connection(c) => pages.extend(c.page),
                AnyRow::Junction(j) => {
                    pages.insert(j.page);
                }
                AnyRow::ReportBlock(b) => pages.extend(b.embedded.as_ref().map(|e| e.page)),
                AnyRow::Function(f) => {
                    reps_pages(before, f.id, &mut pages);
                    reps_pages(after, f.id, &mut pages);
                }
                AnyRow::Device(d) => device_pages(d.id, &mut pages),
                _ => {}
            }
        }
    }
    pages.into_iter().collect()
}
