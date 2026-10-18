// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Revision stamps and durability levels.

use std::fmt;

/// A revision of the host's inputs. Revisions only grow; the host stamps every input (row, index key,
/// table) with the revision of its last change, and advances the store with [`Store::advance`] after every
/// commit.
///
/// [`Store::advance`]: crate::Store::advance
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Rev(pub u64);

impl Rev {
    /// The revision before any change.
    pub const ZERO: Rev = Rev(0);
}

impl fmt::Display for Rev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

/// How rarely an input changes (ADR-0002 decision 4). Ordered from the least to the most durable.
///
/// The durability of a memo is the minimum durability of its dependencies (an input reports its own; a memo
/// dependency contributes the dependency memo's durability; a memo without dependencies is
/// [`Durability::Library`]). The rule that lets a memo skip verification is in [`Store::advance`].
///
/// [`Store::advance`]: crate::Store::advance
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Durability {
    /// Drawing data: changes with almost every commit.
    Drawing = 0,
    /// Project settings and profiles: change now and then.
    Settings = 1,
    /// Embedded library copies (symbols, forms, function definitions): change rarely.
    Library = 2,
}

impl Durability {
    /// Every level, least durable first.
    pub const ALL: [Durability; 3] = [Durability::Drawing, Durability::Settings, Durability::Library];

    /// The level's index into per-level tables (0, 1, 2).
    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}
