// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Transaction` layer of ADR-0008: the ops of one command with its journal metadata.

use ew_model::Op;

use crate::meta::{Exposure, Origin};
use crate::registry::AnyCommand;

/// One committed transaction: `Vec<Op>` plus `{command, origin, undoable}` (ADR-0008). `Op` is `ew-model`'s; the
/// ops already include everything the normalisers added.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Transaction {
    /// The ops, one per changed key in key order.
    pub ops: Vec<Op>,
    /// The command in its JSON form, as the journal and the macro recorder keep it (the canonical name).
    pub command: AnyCommand,
    /// Who submitted it.
    pub origin: Origin,
    /// Whether it enters the undo stack; otherwise it is a marker line.
    pub undoable: bool,
    /// The command's exposure class at commit. `undo` and `redo` refuse a `HumanOnly` step from an origin that
    /// could not have submitted it ([`Exposure::allows_history`]; ADR-0008 amendment 3's note, FT-404). Recorded at
    /// commit, not looked up at undo time, because the registry may no longer hold the command (a plugin unloaded).
    /// In memory only: the history is not persisted.
    pub exposure: Exposure,
    /// The history panel's label: the command name.
    pub label: String,
}

/// How a command is submitted: everything of a [`Transaction`] that does not come from the command.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Submit {
    /// Who submits it.
    pub origin: Origin,
}

impl Submit {
    /// A submission from `origin`.
    pub fn from(origin: Origin) -> Submit {
        Submit { origin }
    }
}
