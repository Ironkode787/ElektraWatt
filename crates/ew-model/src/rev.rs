// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Revisions, rows and tables (ARCH §4.1, ADR-0001 decision 1, ADR-0003).

use std::fmt;
use std::sync::Arc;

use ew_ids::Id;
use imbl::OrdMap;

use crate::entity::Entity;

/// A model revision: monotonic on the main line (ARCH §4.1).
///
/// Every row, table and index key carries the revision of the transaction that last changed it
/// (`changed_at`). The memo layer (ADR-0002) verifies a memo by comparing these stamps with the revision it
/// was computed at; undo applies inverse ops as a **new** revision, so stamps never go backwards
/// (ADR-0008).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Rev(pub u64);

impl Rev {
    /// The revision of a freshly loaded model.
    pub const ZERO: Rev = Rev(0);

    /// The next revision.
    #[must_use]
    pub const fn next(self) -> Rev {
        Rev(self.0 + 1)
    }
}

impl fmt::Display for Rev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

/// One stored row: the record behind an `Arc` and the revision that last changed it.
#[derive(Debug)]
pub struct Row<T> {
    pub(crate) value: Arc<T>,
    pub(crate) changed_at: Rev,
}

impl<T> Clone for Row<T> {
    fn clone(&self) -> Row<T> {
        Row {
            value: Arc::clone(&self.value),
            changed_at: self.changed_at,
        }
    }
}

impl<T> Row<T> {
    pub(crate) fn new(value: Arc<T>, changed_at: Rev) -> Row<T> {
        Row { value, changed_at }
    }

    /// The record.
    pub fn value(&self) -> &Arc<T> {
        &self.value
    }

    /// The revision that last changed this row.
    pub fn changed_at(&self) -> Rev {
        self.changed_at
    }
}

/// The ID type of a table's rows: `Id<T>` for records, `Id<Item>` for a `LibCopy<Item>`.
pub type RowId<T> = Id<<T as Entity>::Marker>;

/// All rows of one kind, in ID order, on a persistent map (O(1) clone).
///
/// There is no public mutation: rows change only through [`Model::apply_with`](crate::Model::apply_with)
/// (edits) and [`Model::apply_history`](crate::Model::apply_history) (undo and redo).
#[derive(Debug)]
pub struct Table<T: Entity> {
    pub(crate) rows: OrdMap<RowId<T>, Row<T>>,
    pub(crate) changed_at: Rev,
}

impl<T: Entity> Clone for Table<T> {
    fn clone(&self) -> Table<T> {
        Table {
            rows: self.rows.clone(),
            changed_at: self.changed_at,
        }
    }
}

impl<T: Entity> Default for Table<T> {
    fn default() -> Table<T> {
        Table {
            rows: OrdMap::new(),
            changed_at: Rev::ZERO,
        }
    }
}

impl<T: Entity> Table<T> {
    raw_read! {
        /// The record with this ID.
        fn get(&self, id: &RowId<T>) -> Option<&Arc<T>> {
            self.rows.get(id).map(|row| &row.value)
        }
    }

    raw_read! {
        /// The row (record and `changed_at`) with this ID.
        fn row(&self, id: &RowId<T>) -> Option<&Row<T>> {
            self.rows.get(id)
        }
    }

    raw_read! {
        /// Whether a row with this ID exists.
        fn contains(&self, id: &RowId<T>) -> bool {
            self.rows.contains_key(id)
        }
    }

    raw_read! {
        /// Every record in ID order.
        fn iter(&self) -> impl Iterator<Item = (&RowId<T>, &Arc<T>)> + '_ {
            self.rows.iter().map(|(id, row)| (id, &row.value))
        }
    }

    raw_read! {
        /// Every row in ID order.
        fn rows(&self) -> impl Iterator<Item = (&RowId<T>, &Row<T>)> + '_ {
            self.rows.iter()
        }
    }

    raw_read! {
        /// Every ID in order.
        fn ids(&self) -> impl Iterator<Item = RowId<T>> + '_ {
            self.rows.keys().copied()
        }
    }

    raw_read! {
        /// The number of rows.
        fn len(&self) -> usize {
            self.rows.len()
        }
    }

    raw_read! {
        /// Whether the table is empty.
        fn is_empty(&self) -> bool {
            self.rows.is_empty()
        }
    }

    raw_read! {
        /// The revision that last inserted, changed or removed a row of this table.
        fn changed_at(&self) -> Rev {
            self.changed_at
        }
    }

    raw_read! {
        /// Whether both tables hold the same IDs with equal records (`changed_at` stamps are ignored).
        fn same_rows(&self, other: &Table<T>) -> bool {
            self.rows.len() == other.rows.len()
                && self
                    .rows
                    .iter()
                    .zip(other.rows.iter())
                    .all(|((ia, ra), (ib, rb))| {
                        ia == ib && (Arc::ptr_eq(&ra.value, &rb.value) || ra.value == rb.value)
                    })
        }
    }

    pub(crate) fn put(&mut self, id: RowId<T>, value: Option<Arc<T>>, rev: Rev) {
        match value {
            Some(value) => {
                self.rows.insert(id, Row::new(value, rev));
            }
            None => {
                self.rows.remove(&id);
            }
        }
        self.changed_at = rev;
    }
}
