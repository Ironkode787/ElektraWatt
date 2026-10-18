// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dependency keys: the inputs and memos a query read.

/// A query family registered in a [`Store`](crate::Store), numbered in registration order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FamilyId(pub(crate) u32);

/// One key of a query family: the index of its memo slot. Slots are never reused within a store, so a
/// `(FamilyId, SlotId)` names one memo for the store's lifetime (and in its forks).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SlotId(pub(crate) u32);

impl SlotId {
    /// The slot index.
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

/// What a query read: an input of the host (`I`, e.g. a row, an index key or a table) or another memo.
///
/// The derived order (inputs before memos, then by key) is only used for lookups and for the deterministic
/// order of the reverse-dependency index.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DepKey<I> {
    /// An input of the host.
    Input(I),
    /// A memo of a query family.
    Memo(FamilyId, SlotId),
}

/// A memo across families.
pub(crate) type MemoRef = (FamilyId, SlotId);
