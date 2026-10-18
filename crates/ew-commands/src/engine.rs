// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The commit pipeline (ARCH §5.2), undo and redo (§5.3) and previews (§5.4): the engine behind
//! `ew_api::Session`.
//!
//! ```text
//! run       Command::run against a TxBuilder on the current snapshot (exposure checked first)
//! normalise the normaliser registry, passes until quiet, at most three
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

use ew_ids::{Id, IdSource, SystemSource};
use ew_model::{AnyKey, AnyRow, ApplyOptions, Model, Op, PreventHook, PreventViolation, Rev, invert_batch};
use ew_schema::{Page, Representation};

use crate::command::{DynCommand, Outcome};
use crate::diff::{ModelDiff, diff_model};
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
    fn check(&self, _candidate: &Model, _touched: &[AnyKey]) -> Result<(), PreventViolation> {
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

/// A prepared transaction on a fork (ADR-0008 "preview is a fork"): the candidate snapshot, its ops, the
/// read set for the rebase check, the entity and field diff and the affected pages. Nothing is committed
/// until [`Engine::commit`].
#[derive(Clone, Debug)]
pub struct Preview {
    candidate: Model,
    ops: Vec<Op>,
    read_set: ReadSet,
    diff: ModelDiff,
    affected_pages: Vec<Id<Page>>,
    base_rev: Rev,
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

    /// The prepared ops.
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// The base reads of the command and the normalisers.
    pub fn read_set(&self) -> &ReadSet {
        &self.read_set
    }

    /// The entity and field changes, `diff_model(current, candidate)`.
    pub fn diff(&self) -> &ModelDiff {
        &self.diff
    }

    /// The pages whose content changes.
    pub fn affected_pages(&self) -> &[Id<Page>] {
        &self.affected_pages
    }

    /// The revision of the snapshot the preview ran on.
    pub fn base_rev(&self) -> Rev {
        self.base_rev
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
    base_rev: Rev,
}

/// The editing engine: the current model, the history, the registry, the ID source, the prevent hook,
/// the normalisers and the change feed.
pub struct Engine {
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
        let outcome = cmd.run(&mut tx)?;
        let passes = normalise(&mut tx, &self.normalisers)?;
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
            base_rev: base.rev(),
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
        if cmd.cost() == Cost::Bulk {
            return Err(CmdError::NeedsPreview {
                command: cmd.name().into(),
            });
        }
        let prepared = self.prepare(cmd, &submit)?;
        Ok(self.finalize(prepared, AnyCommand::of(cmd), cmd.meta(), submit))
    }

    /// Prepares a command on a fork of the current snapshot from the default origin (synchronously in v0).
    pub fn preview(&mut self, cmd: &dyn DynCommand) -> Result<Preview, CmdError> {
        let submit = self.default_submit();
        self.preview_with(cmd, submit)
    }

    /// Prepares a command on a fork of the current snapshot with an explicit submission.
    pub fn preview_with(&mut self, cmd: &dyn DynCommand, submit: Submit) -> Result<Preview, CmdError> {
        let prepared = self.prepare(cmd, &submit)?;
        let diff = diff_model(&self.model, &prepared.candidate);
        let affected = affected_pages(&self.model, &prepared.candidate, &prepared.ops);
        Ok(Preview {
            candidate: prepared.candidate,
            ops: prepared.ops,
            read_set: prepared.read_set,
            diff,
            affected_pages: affected,
            base_rev: prepared.base_rev,
            command: AnyCommand::of(cmd),
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
    pub fn commit(&mut self, preview: Preview) -> Result<Receipt, CmdError> {
        let Preview {
            candidate,
            ops,
            read_set,
            base_rev,
            command,
            meta,
            submit,
            outcome,
            created,
            passes,
            ..
        } = preview;
        if base_rev == self.model.rev() {
            let prepared = Prepared {
                candidate,
                ops,
                read_set,
                outcome,
                created,
                passes,
                base_rev,
            };
            return Ok(self.finalize(prepared, command, meta, submit));
        }
        if read_set.unchanged_since(&self.model, base_rev) {
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
                    base_rev: self.model.rev(),
                };
                return Ok(self.finalize(prepared, command, meta, submit));
            }
        }
        let cmd = self.registry.resolve(&command)?;
        let prepared = self.prepare(cmd.as_ref(), &submit)?;
        Ok(self.finalize(prepared, command, meta, submit))
    }

    /// Reverts the step below the cursor by applying its inverse ops as a new revision (ARCH §5.3).
    /// Structural invariants are checked; prevent rules are not (the restored state passed them).
    pub fn undo(&mut self) -> Result<Restored, CmdError> {
        let index = self.history.undo_index()?;
        let entry = self.history.entry(index).expect("undo index is in range");
        let tx = entry.transaction().clone();
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
        let tx = entry.transaction().clone();
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
    for op in ops {
        for row in [op.before(), op.after()].into_iter().flatten() {
            match row {
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
                AnyRow::Device(d) => {
                    for m in [before, after] {
                        for f in m.indexes().functions_by_device().get(&d.id) {
                            reps_pages(m, *f, &mut pages);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    pages.into_iter().collect()
}
