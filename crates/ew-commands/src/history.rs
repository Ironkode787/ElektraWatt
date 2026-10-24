// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project-wide undo stack (ADR-0008, ARCH §5.3).
//!
//! - One stack for the whole project, bounded at 500 steps or 256 MB of memory the history alone keeps
//!   alive (see "Memory accounting"); the oldest entries are dropped first, but the newest entry is always
//!   kept.
//! - Undo applies the inverse ops of a step as a **new** revision; redo applies the step's ops again, also
//!   as a new revision (the session does both; this type only keeps the entries and the cursor).
//! - Coalescing: an undoable transaction whose `coalesce_key` equals that of the step directly below the
//!   cursor merges into it (one drag, one field edit). The merged step keeps the first transaction's
//!   context and label, the last one's command and origin, and the strictest exposure class of all of them
//!   ([`Exposure::strictest`]; FT-404). The ops are **compacted by key**: a step has
//!   one op per key, in key order, with the `before` row of the first transaction that touched the key and
//!   the `after` row of the last, so 2,000 moves of one representation are one op. The merge happens in
//!   place (the transaction is cloned only when an entry shares it) and costs O(ops of the incoming
//!   transaction × log(ops of the step)) plus O(step) for the rare insertion of a new key. A key whose
//!   first `before` equals its last `after` keeps its (net no-op) op: it is harmless and keeps the step
//!   effective.
//! - Non-undoable transactions are **marker lines**. A marker without ops (an export) is transparent:
//!   undo and redo pass over it. A marker with ops (an empty-project import checkpoint) is a barrier: undo
//!   stops at it.
//! - A saved marker drives [`History::is_dirty`]. Every entry that changed the model carries a unique state
//!   number; the model is clean when the state below the cursor is the saved one.
//!
//! # Memory accounting
//!
//! The bound counts what the history **alone** keeps alive, estimated from the rows' heap sizes without
//! serialising ([`crate::heap`], F8). Each entry keeps two figures:
//!
//! - `before_bytes`: the `before` rows of its ops plus the ops themselves. The live model no longer holds
//!   these rows once the step is applied, so they always count (also while the step is undone: a
//!   simplification, the model shares them then, but the figure stays stable while the cursor moves).
//! - `after_bytes`: the `after` rows. While the step is applied they are the live model's rows (the same
//!   `Arc`s) and do not count; once the step is undone (in the redo branch) the model has dropped them and
//!   they count.
//!
//! [`History::bytes`] is the sum of `before_bytes` over all entries plus the sum of `after_bytes` over the
//! entries at and above the cursor. It is kept incrementally: a push adds `before_bytes`, [`History::undone`]
//! adds the `after_bytes` of the steps that cross into the redo branch, [`History::redone`] subtracts them,
//! and dropping an entry subtracts what it counts. A single step larger than the budget is still kept, alone
//! (the newest entry is never trimmed); this is how a huge import or replace stays undoable.

use std::sync::Arc;

use ew_model::{Op, Rev};

use crate::diff::ModelDiff;
use crate::error::CmdError;
use crate::meta::{Exposure, Origin};
use crate::registry::AnyCommand;
use crate::rows::{after_bytes, before_bytes};
use crate::transaction::{Transaction, TxContext};

/// The bounds of the undo stack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HistoryLimits {
    /// The maximum number of entries (steps and marker lines).
    pub max_steps: usize,
    /// The maximum estimated size of the rows only the history keeps alive, in bytes (see the module doc).
    pub max_bytes: usize,
}

impl Default for HistoryLimits {
    /// 500 steps or 256 MB (ARCH §5.3).
    fn default() -> HistoryLimits {
        HistoryLimits {
            max_steps: 500,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}

/// One history entry: an undo step or a marker line.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    tx: Arc<Transaction>,
    rev: Rev,
    before_bytes: usize,
    after_bytes: usize,
    state: u64,
    merged: u32,
}

impl HistoryEntry {
    /// The (possibly coalesced) transaction (feature `testkit` only: its ops carry whole rows, F9).
    #[cfg(feature = "testkit")]
    pub fn transaction(&self) -> &Transaction {
        &self.tx
    }

    /// The (possibly coalesced) transaction.
    pub(crate) fn tx(&self) -> &Transaction {
        &self.tx
    }

    /// The history panel's label: the command name.
    pub fn label(&self) -> &str {
        &self.tx.label
    }

    /// Who submitted the (last merged) transaction.
    pub fn origin(&self) -> &Origin {
        &self.tx.origin
    }

    /// The (last merged) command in its JSON form.
    pub fn command(&self) -> &AnyCommand {
        &self.tx.command
    }

    /// The strictest exposure class of the commands the step holds, recorded at commit (FT-404): `undo` and
    /// `redo` refuse a `HumanOnly` step from an origin that [`Exposure::allows_history`] does not admit.
    pub fn exposure(&self) -> Exposure {
        self.tx.exposure
    }

    /// The editor state an undo or redo of this step restores.
    pub fn context(&self) -> &TxContext {
        &self.tx.context
    }

    /// The coalescing key.
    pub fn coalesce_key(&self) -> Option<&str> {
        self.tx.coalesce_key.as_deref()
    }

    /// The number of ops of the step.
    pub fn op_count(&self) -> usize {
        self.tx.ops.len()
    }

    /// The records the step changes: change kind, key and changed member paths (no row values, F9).
    pub fn changes(&self) -> Result<ModelDiff, CmdError> {
        ModelDiff::of_ops(&self.tx.ops)
    }

    /// The revision that committed it (for a coalesced step: the last merged transaction's).
    pub fn rev(&self) -> Rev {
        self.rev
    }

    /// The estimated size in bytes of the entry's `before` rows and ops: what it counts towards the bound
    /// while it is applied.
    pub fn bytes(&self) -> usize {
        self.before_bytes
    }

    /// The estimated size in bytes of the entry's `after` rows: counted too while the entry is in the redo
    /// branch.
    pub fn after_bytes(&self) -> usize {
        self.after_bytes
    }

    /// How many transactions were merged into this step after the first.
    pub fn merged(&self) -> u32 {
        self.merged
    }

    /// Whether this is an undo step (not a marker line).
    pub fn is_step(&self) -> bool {
        self.tx.undoable
    }

    /// Whether this is a marker line that changed the model: undo cannot pass it.
    pub fn is_barrier(&self) -> bool {
        !self.tx.undoable && !self.tx.ops.is_empty()
    }

    /// Whether the entry changed the model.
    fn effective(&self) -> bool {
        !self.tx.ops.is_empty()
    }
}

/// The undo stack.
#[derive(Clone, Debug)]
pub struct History {
    limits: HistoryLimits,
    entries: Vec<HistoryEntry>,
    cursor: usize,
    bytes: usize,
    next_state: u64,
    floor_state: u64,
    saved: Option<u64>,
}

impl Default for History {
    fn default() -> History {
        History::new(HistoryLimits::default())
    }
}

impl History {
    /// An empty stack whose initial state is saved (a freshly loaded project is clean).
    pub fn new(limits: HistoryLimits) -> History {
        History {
            limits,
            entries: Vec::new(),
            cursor: 0,
            bytes: 0,
            next_state: 1,
            floor_state: 0,
            saved: Some(0),
        }
    }

    /// The bounds.
    pub fn limits(&self) -> HistoryLimits {
        self.limits
    }

    /// Every entry, oldest first; entries at and above [`History::cursor`] are the redo branch.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// The number of entries below the cursor (applied).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Every asset hash that an undo or redo entry may bring back: the SHA-256 texts mentioned in the JSON of the
    /// `before` and `after` rows of every op of every entry ([`ew_schema::sha256_mentions`]: any 64 hex digits, so
    /// a bare hash, a reference of a newer writer or an unknown member counts). The asset purge keeps the files of
    /// these hashes ("in no undo entry", doc 05 §13.3, FT-380), so that an undo never names an asset whose file a
    /// purge deleted. An over-approximation by design. O(the rows the history keeps).
    ///
    /// # Errors
    ///
    /// [`CmdError::Internal`] for a row without a JSON form.
    pub fn asset_mentions(&self) -> Result<std::collections::BTreeSet<ew_schema::Sha256>, CmdError> {
        let mut out = std::collections::BTreeSet::new();
        for entry in &self.entries {
            for op in &entry.tx.ops {
                let Op::Put { before, after, .. } = op;
                for row in [before, after].into_iter().flatten() {
                    let text = crate::rows::row_json(row)?.to_string();
                    ew_schema::sha256_mentions(text.as_bytes(), &mut out);
                }
            }
        }
        Ok(out)
    }

    /// The estimated size in bytes of what the history alone keeps alive: the `before` rows of every entry
    /// and the `after` rows of the entries at and above the cursor (module doc).
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Records a committed transaction at revision `rev`: drops the redo branch, then merges into the
    /// step below the cursor when the coalescing key matches, else pushes a new entry; finally enforces the
    /// bounds.
    pub fn push(&mut self, tx: Transaction, rev: Rev) {
        for dropped in self.entries.drain(self.cursor..) {
            self.bytes -= dropped.before_bytes + dropped.after_bytes;
        }
        let coalesce = tx.undoable
            && tx.coalesce_key.is_some()
            && self
                .entries
                .last()
                .is_some_and(|top| top.is_step() && top.tx.coalesce_key == tx.coalesce_key);
        let state = self.next_state;
        self.next_state += 1;
        if coalesce {
            let top = self.entries.last_mut().expect("checked above");
            let merged = Arc::make_mut(&mut top.tx);
            let (before, after) = merge_ops(&mut merged.ops, tx.ops);
            merged.command = tx.command;
            merged.origin = tx.origin;
            merged.exposure = merged.exposure.strictest(tx.exposure);
            top.before_bytes = top.before_bytes.saturating_add_signed(before);
            top.after_bytes = top.after_bytes.saturating_add_signed(after);
            // The step is applied: only its `before` rows count.
            self.bytes = self.bytes.saturating_add_signed(before);
            top.rev = rev;
            top.state = state;
            top.merged += 1;
        } else {
            let before_bytes = tx.ops.iter().map(before_bytes).sum();
            let after_bytes = tx.ops.iter().map(after_bytes).sum();
            self.bytes += before_bytes;
            self.entries.push(HistoryEntry {
                tx: Arc::new(tx),
                rev,
                before_bytes,
                after_bytes,
                state,
                merged: 0,
            });
        }
        self.cursor = self.entries.len();
        self.trim();
    }

    /// Drops the oldest entries while a bound is exceeded, keeping at least the newest one.
    fn trim(&mut self) {
        while self.entries.len() > 1
            && (self.entries.len() > self.limits.max_steps || self.bytes > self.limits.max_bytes)
        {
            let old = self.entries.remove(0);
            self.bytes -= old.before_bytes + if self.cursor == 0 { old.after_bytes } else { 0 };
            self.cursor = self.cursor.saturating_sub(1);
            if old.effective() {
                self.floor_state = old.state;
            }
        }
        if let Some(saved) = self.saved {
            let reachable =
                saved == self.floor_state || self.entries.iter().any(|e| e.effective() && e.state == saved);
            if !reachable {
                self.saved = None;
            }
        }
    }

    /// The state number of the model below the cursor.
    pub fn current_state(&self) -> u64 {
        self.entries[..self.cursor]
            .iter()
            .rev()
            .find(|e| e.effective())
            .map_or(self.floor_state, |e| e.state)
    }

    /// Marks the current state as saved.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.current_state());
    }

    /// Whether the model differs from the last saved state.
    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.current_state())
    }

    /// The index of the step the next undo reverts: transparent markers directly below the cursor are
    /// passed; a barrier or the bottom of the stack is an error.
    pub fn undo_index(&self) -> Result<usize, CmdError> {
        for i in (0..self.cursor).rev() {
            let e = &self.entries[i];
            if e.is_step() {
                return Ok(i);
            }
            if e.is_barrier() {
                return Err(CmdError::History {
                    reason: format!(
                        "`{}` is a non-undoable checkpoint; nothing before it can be undone",
                        e.tx.label
                    ),
                });
            }
        }
        Err(CmdError::History {
            reason: "nothing to undo".into(),
        })
    }

    /// The index of the step the next redo re-applies.
    pub fn redo_index(&self) -> Result<usize, CmdError> {
        (self.cursor..self.entries.len())
            .find(|i| self.entries[*i].is_step())
            .ok_or_else(|| CmdError::History {
                reason: "nothing to redo".into(),
            })
    }

    /// Whether an undo is possible.
    pub fn can_undo(&self) -> bool {
        self.undo_index().is_ok()
    }

    /// Whether a redo is possible.
    pub fn can_redo(&self) -> bool {
        self.redo_index().is_ok()
    }

    /// The entry at an index.
    pub fn entry(&self, index: usize) -> Option<&HistoryEntry> {
        self.entries.get(index)
    }

    /// Moves the cursor below the undone step `index` (after the session applied its inverse).
    pub fn undone(&mut self, index: usize) {
        if index < self.cursor {
            // The model no longer holds the `after` rows of the steps that leave the applied part.
            self.bytes += self.entries[index..self.cursor]
                .iter()
                .map(|e| e.after_bytes)
                .sum::<usize>();
            self.cursor = index;
        }
    }

    /// Moves the cursor above the redone step `index` and the transparent markers that follow it.
    pub fn redone(&mut self, index: usize) {
        let mut cursor = index + 1;
        while cursor < self.entries.len()
            && !self.entries[cursor].is_step()
            && !self.entries[cursor].is_barrier()
        {
            cursor += 1;
        }
        if cursor > self.cursor {
            // The `after` rows are the live model's again.
            self.bytes -= self.entries[self.cursor..cursor]
                .iter()
                .map(|e| e.after_bytes)
                .sum::<usize>();
            self.cursor = cursor;
        }
    }
}

/// Whether the ops are in strictly ascending key order (one op per key).
fn is_keyed(ops: &[Op]) -> bool {
    ops.windows(2).all(|w| w[0].key() < w[1].key())
}

/// The `before` and `after` bytes of a list of ops.
fn totals(ops: &[Op]) -> (usize, usize) {
    (
        ops.iter().map(before_bytes).sum(),
        ops.iter().map(after_bytes).sum(),
    )
}

/// Brings ops to one op per key in key order: a stable sort, then every run of one key becomes one op with
/// the first `before` and the last `after`.
fn compact(ops: &mut Vec<Op>) {
    if is_keyed(ops) {
        return;
    }
    ops.sort_by_key(Op::key);
    let mut out: Vec<Op> = Vec::with_capacity(ops.len());
    for op in ops.drain(..) {
        match out.last_mut() {
            Some(Op::Put { key, after, .. }) if *key == op.key() => {
                let Op::Put { after: last, .. } = op;
                *after = last;
            }
            _ => out.push(op),
        }
    }
    *ops = out;
}

/// Merges the ops of a later transaction into those of a step by key: an op on a key the step already has
/// replaces that op's `after` (the first `before` stays), any other op is added. Returns the change of the
/// step's `before` and `after` bytes.
fn merge_ops(into: &mut Vec<Op>, mut incoming: Vec<Op>) -> (isize, isize) {
    let mut before = 0isize;
    let mut after = 0isize;
    if !is_keyed(into) {
        // Only a first transaction that touched a key twice gets here.
        let old = totals(into);
        compact(into);
        let new = totals(into);
        before += new.0 as isize - old.0 as isize;
        after += new.1 as isize - old.1 as isize;
    }
    compact(&mut incoming);
    let mut fresh = Vec::new();
    for op in incoming {
        match into.binary_search_by_key(&op.key(), Op::key) {
            Ok(i) => {
                let Op::Put { after: new, .. } = op;
                let Op::Put { after: old, .. } = &mut into[i];
                after -= old.as_ref().map_or(0, crate::heap::row_bytes) as isize;
                after += new.as_ref().map_or(0, crate::heap::row_bytes) as isize;
                *old = new;
            }
            Err(_) => {
                before += before_bytes(&op) as isize;
                after += after_bytes(&op) as isize;
                fresh.push(op);
            }
        }
    }
    if !fresh.is_empty() {
        into.extend(fresh);
        into.sort_by_key(Op::key);
    }
    (before, after)
}
