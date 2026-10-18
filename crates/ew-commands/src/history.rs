// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project-wide undo stack (ADR-0008, ARCH §5.3).
//!
//! - One stack for the whole project, bounded at 500 steps or 256 MB (estimated from the ops' rows,
//!   [`crate::rows::op_bytes`]); the oldest entries are dropped first, but the newest entry is always kept.
//! - Undo applies the inverse ops of a step as a **new** revision; redo applies the step's ops again, also
//!   as a new revision (the session does both; this type only keeps the entries and the cursor).
//! - Coalescing: an undoable transaction whose `coalesce_key` equals that of the step directly below the
//!   cursor merges into it (one drag, one field edit). The merged step keeps the first transaction's
//!   context and label and concatenates the ops.
//! - Non-undoable transactions are **marker lines**. A marker without ops (an export) is transparent:
//!   undo and redo pass over it. A marker with ops (an empty-project import checkpoint) is a barrier: undo
//!   stops at it.
//! - A saved marker drives [`History::is_dirty`]. Every entry that changed the model carries a unique state
//!   number; the model is clean when the state below the cursor is the saved one.

use std::sync::Arc;

use ew_model::Rev;

use crate::error::CmdError;
use crate::rows::op_bytes;
use crate::transaction::Transaction;

/// The bounds of the undo stack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HistoryLimits {
    /// The maximum number of entries (steps and marker lines).
    pub max_steps: usize,
    /// The maximum estimated size of all entries' ops, in bytes.
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
    bytes: usize,
    state: u64,
    merged: u32,
}

impl HistoryEntry {
    /// The (possibly coalesced) transaction.
    pub fn transaction(&self) -> &Transaction {
        &self.tx
    }

    /// The revision that committed it (for a coalesced step: the last merged transaction's).
    pub fn rev(&self) -> Rev {
        self.rev
    }

    /// The estimated size of its ops in bytes.
    pub fn bytes(&self) -> usize {
        self.bytes
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

    /// The estimated size of all entries in bytes.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Records a committed transaction at revision `rev`: drops the redo branch, then merges into the
    /// step below the cursor when the coalescing key matches, else pushes a new entry; finally enforces the
    /// bounds.
    pub fn push(&mut self, tx: Transaction, rev: Rev) {
        for dropped in self.entries.drain(self.cursor..) {
            self.bytes -= dropped.bytes;
        }
        let bytes: usize = tx.ops.iter().map(op_bytes).sum();
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
            let mut merged = (*top.tx).clone();
            merged.ops.extend(tx.ops);
            merged.command = tx.command;
            merged.origin = tx.origin;
            top.tx = Arc::new(merged);
            top.rev = rev;
            top.bytes += bytes;
            top.state = state;
            top.merged += 1;
        } else {
            self.entries.push(HistoryEntry {
                tx: Arc::new(tx),
                rev,
                bytes,
                state,
                merged: 0,
            });
        }
        self.bytes += bytes;
        self.cursor = self.entries.len();
        self.trim();
    }

    /// Drops the oldest entries while a bound is exceeded, keeping at least the newest one.
    fn trim(&mut self) {
        while self.entries.len() > 1
            && (self.entries.len() > self.limits.max_steps || self.bytes > self.limits.max_bytes)
        {
            let old = self.entries.remove(0);
            self.bytes -= old.bytes;
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
        self.cursor = index;
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
        self.cursor = cursor;
    }
}
