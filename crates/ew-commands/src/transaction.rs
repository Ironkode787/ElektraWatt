// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Transaction` layer of ADR-0008: the ops of one command with its journal metadata.

use ew_ids::Id;
use ew_model::{AnyKey, Op};
use ew_schema::Page;
use ew_units::Pt2;

use crate::meta::Origin;
use crate::registry::AnyCommand;

/// The visible part of the canvas when the command was submitted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Viewport {
    /// The model point at the centre of the view.
    pub centre: Pt2,
    /// The zoom in thousandths (1000 = 100 %).
    pub zoom_milli: u32,
}

/// The editor state an undo or redo restores (ARCH §5.3: "each entry restores page, viewport and
/// selection").
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TxContext {
    /// The open page.
    pub page: Option<Id<Page>>,
    /// The viewport.
    pub viewport: Option<Viewport>,
    /// The selected records.
    pub selection: Vec<AnyKey>,
}

/// One committed transaction: `Vec<Op>` plus `{command, origin, context, coalesce_key, undoable}`
/// (ADR-0008). `Op` is `ew-model`'s; the ops already include everything the normalisers added.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Transaction {
    /// The ops, one per changed key in key order (for a coalesced history step: the concatenation of the
    /// merged transactions' ops).
    pub ops: Vec<Op>,
    /// The command in its JSON form, as the journal and the macro recorder keep it (the canonical name).
    pub command: AnyCommand,
    /// Who submitted it.
    pub origin: Origin,
    /// The editor state at submission.
    pub context: TxContext,
    /// Consecutive transactions with the same key merge into one undo step (one drag, one field edit).
    pub coalesce_key: Option<String>,
    /// Whether it enters the undo stack; otherwise it is a marker line.
    pub undoable: bool,
    /// The history panel's label: the command name.
    pub label: String,
}

/// How a command is submitted: everything of a [`Transaction`] that does not come from the command.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Submit {
    /// Who submits it.
    pub origin: Origin,
    /// The editor state.
    pub context: TxContext,
    /// The coalescing key, if the gesture continues the previous one.
    pub coalesce_key: Option<String>,
}

impl Submit {
    /// A submission from `origin` with an empty context and no coalescing.
    pub fn from(origin: Origin) -> Submit {
        Submit {
            origin,
            ..Submit::default()
        }
    }

    /// The same submission with a coalescing key.
    #[must_use]
    pub fn coalescing(mut self, key: impl Into<String>) -> Submit {
        self.coalesce_key = Some(key.into());
        self
    }
}
