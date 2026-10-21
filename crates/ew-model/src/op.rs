// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The op layer (ADR-0008, ARCH §5.1): invertible row puts and [`Model::apply`].

use ew_schema::Kind;

use crate::entity::{AnyKey, AnyRow, TABLE_KINDS, get_row, put_row};
use crate::error::ModelError;
use crate::invariants::{self, Scope};
use crate::model::Model;
use crate::prevent::ApplyOptions;
use crate::rev::Rev;

impl PartialEq for AnyRow {
    fn eq(&self, other: &AnyRow) -> bool {
        self.same(other)
    }
}

impl Eq for AnyRow {}

/// One invertible change of one row (ADR-0008): `before` is the row the op was prepared against (`None`:
/// the row did not exist), `after` the row it leaves (`None`: the row is removed).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Op {
    /// Puts `after` under `key`.
    Put {
        /// The row's kind and ID.
        key: AnyKey,
        /// The current row the op expects.
        before: Option<AnyRow>,
        /// The row it leaves.
        after: Option<AnyRow>,
    },
}

impl Op {
    /// Inserts a new row.
    pub fn insert(row: AnyRow) -> Op {
        Op::Put {
            key: row.key(),
            before: None,
            after: Some(row),
        }
    }

    /// Removes an existing row (`row` is the current one).
    pub fn delete(row: AnyRow) -> Op {
        Op::Put {
            key: row.key(),
            before: Some(row),
            after: None,
        }
    }

    /// Replaces an existing row; both rows have the same key ([`Model::apply`] rejects a mismatch).
    pub fn update(before: AnyRow, after: AnyRow) -> Op {
        Op::Put {
            key: before.key(),
            before: Some(before),
            after: Some(after),
        }
    }

    /// The row's key.
    pub fn key(&self) -> AnyKey {
        match self {
            Op::Put { key, .. } => *key,
        }
    }

    /// The row the op expects.
    pub fn before(&self) -> Option<&AnyRow> {
        match self {
            Op::Put { before, .. } => before.as_ref(),
        }
    }

    /// The row the op leaves.
    pub fn after(&self) -> Option<&AnyRow> {
        match self {
            Op::Put { after, .. } => after.as_ref(),
        }
    }

    /// The inverse op: `before` and `after` swapped.
    #[must_use]
    pub fn invert(&self) -> Op {
        match self {
            Op::Put { key, before, after } => Op::Put {
                key: *key,
                before: after.clone(),
                after: before.clone(),
            },
        }
    }
}

/// The inverse of a batch: every op inverted, in reverse order (a key may be put more than once).
pub fn invert_batch(ops: &[Op]) -> Vec<Op> {
    ops.iter().rev().map(Op::invert).collect()
}

/// Checks that an op's rows match its key and that the key names a table or the project row.
fn validate(
    model: &Model,
    key: &AnyKey,
    before: Option<&AnyRow>,
    after: Option<&AnyRow>,
) -> Result<(), ModelError> {
    let malformed = |reason| Err(ModelError::Malformed { key: *key, reason });
    for row in [before, after].into_iter().flatten() {
        if row.key() != *key {
            return malformed("a row does not match the op's key");
        }
    }
    if key.0 == Kind::Project {
        if key.1 != model.project.value.id.cast::<()>() {
            return malformed("the project row cannot be re-keyed");
        }
        if before.is_none() || after.is_none() {
            return malformed("the project row cannot be inserted or removed");
        }
    } else if !TABLE_KINDS.contains(&key.0) {
        return malformed("the kind has no table in this model");
    }
    Ok(())
}

impl Model {
    /// Applies a batch of ops as revision `rev` with no prevent rules; see [`Model::apply_with`].
    pub fn apply(&self, ops: &[Op], rev: Rev) -> Result<Model, ModelError> {
        self.apply_with(ops, rev, &ApplyOptions::default())
    }

    /// Applies a batch of ops as revision `rev` and returns the new snapshot; `self` is unchanged.
    ///
    /// 1. `rev` must be greater than [`Model::rev`].
    /// 2. The ops run in order. Each op's `before` must be the current row (pointer or value equality),
    ///    else [`ModelError::StaleBefore`]: the optimistic-rebase signal of ADR-0008. Within a batch a
    ///    later op on the same key therefore expects the earlier op's `after`.
    /// 3. Every touched row, its table and every touched index key is stamped `changed_at = rev`; the
    ///    base indexes are maintained by delta (debug builds rebuild them and compare, ARCH §4.1).
    /// 4. The structural invariants are evaluated on the candidate, limited to the touched keys and their
    ///    index neighbours (ADR-0009 decision 1); the first violation rejects the whole batch. Removing a
    ///    record that a `forbid` link still names is [`ModelError::Referenced`]; `cascade` and `unlink`
    ///    are not executed here (see `Model::delete_plan`, behind `raw-read`), so a remaining referrer is
    ///    a violation of EW-INV-001.
    /// 5. The prevent hook, if any, checks the candidate against `self`, the base snapshot.
    ///
    /// `apply(ops)` followed by `apply(invert_batch(ops))` restores every row value and index entry; only
    /// the `changed_at` stamps differ.
    pub fn apply_with(&self, ops: &[Op], rev: Rev, opts: &ApplyOptions<'_>) -> Result<Model, ModelError> {
        let (m, scope) = self.apply_rows(ops, rev)?;
        invariants::check(&m, &scope)?;
        if let Some(hook) = opts.prevent {
            let touched: Vec<AnyKey> = scope.keys.iter().copied().collect();
            hook.check(self, &m, &touched)
                .map_err(|v| ModelError::Prevented {
                    rule: v.rule,
                    message: v.message,
                    keys: v.keys,
                })?;
        }
        Ok(m)
    }

    raw_read! {
        /// Steps 1–3 of [`Model::apply_with`] only: the ops run with the stale-`before` check and the
        /// index deltas, but **no** structural invariant and no prevent rule is evaluated. This is the
        /// planning snapshot of a transaction in progress, whose transient states are legal (ADR-0009
        /// decision 1): `TxBuilder` plans a removal on it. The result must never be committed; the
        /// transaction's final batch goes through [`Model::apply_with`].
        fn apply_unchecked(&self, ops: &[Op], rev: Rev) -> Result<Model, ModelError> {
            self.apply_rows(ops, rev).map(|(m, _)| m)
        }
    }

    /// Runs the ops of a batch (steps 1–3 of [`Model::apply_with`]) and returns the candidate with the
    /// scope of the invariant check.
    fn apply_rows(&self, ops: &[Op], rev: Rev) -> Result<(Model, Scope), ModelError> {
        if rev <= self.rev {
            return Err(ModelError::RevNotIncreasing {
                current: self.rev,
                given: rev,
            });
        }
        let mut m = self.clone();
        m.rev = rev;
        let mut scope = Scope::default();
        for op in ops {
            let Op::Put { key, before, after } = op;
            validate(&m, key, before.as_ref(), after.as_ref())?;
            let current = get_row(&m, key);
            let fresh = match (before, &current) {
                (None, None) => true,
                (Some(b), Some(c)) => b.same(c),
                _ => false,
            };
            if !fresh {
                return Err(ModelError::StaleBefore { key: *key });
            }
            put_row(&mut m, key, after.as_ref(), rev);
            m.idx.update(current.as_ref(), after.as_ref(), rev);
            scope.touch(*key, current);
        }
        #[cfg(debug_assertions)]
        {
            let rebuilt = crate::index::BaseIndexes::rebuild(&m);
            assert!(
                rebuilt.same_entries(&m.idx),
                "ew-model: incrementally maintained base indexes differ from rebuilt ones after revision {rev}"
            );
        }
        Ok((m, scope))
    }
}
