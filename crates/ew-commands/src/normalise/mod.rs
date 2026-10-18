// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normalisers (ARCH §5.2 step 2; the registry is doc 06 §3.2): deterministic passes that add ops to the
//! transaction of the command that ran.
//!
//! The spine subset, in registry order within one pass ([`ORDER`], asserted by a test):
//!
//! | # | ID | Effect |
//! |---|---|---|
//! | 2 | `cp_instantiation` | No-op recorder: CPs are derived from the FnDef or part pattern and only overrides persist (doc 06 §3.2 row 2), so there is nothing to store |
//! | 4 | `online_numbering` | Gives a placed numbered device without a counter the next free counter of its page scope, and a device whose page-based tag collides (after a move of its main representation) the next free one (FR-NUM-007, FR-NUM-029) |
//! | 7 | `connection_split` | A new representation whose two CPs lie on one segment of a connection's route splits that connection (FR-CON-008, doc 06 §5.3) |
//! | 8 | `junction_cleanup` | A junction left with two connections dissolves into one; with one, it is removed together with that connection; with none, it is removed (CI-04) |
//! | 12 | `wire_numbering` | Online wire numbering is off in v0 (no scheme before WP-S1.11): every new connection is `NumberSpec::Unassigned` |
//!
//! A pass runs every normaliser in order; a normaliser whose input another one creates later in the same
//! pass sees it in the next pass. The pipeline runs passes until one changes nothing, at most three
//! ([`MAX_PASSES`]); a third pass that still changes something is non-convergence, a bug
//! ([`crate::CmdError::Internal`]). Every normaliser breaks ties by ID.

mod connection_split;
mod cp_instantiation;
mod junction_cleanup;
mod online_numbering;
mod wire_numbering;

pub use connection_split::{ConnectionSplit, split_connection};
pub use cp_instantiation::CpInstantiation;
pub use junction_cleanup::JunctionCleanup;
pub use online_numbering::OnlineNumbering;
pub use wire_numbering::WireNumbering;

use crate::error::CmdError;
use crate::tx::TxBuilder;

/// One normaliser of the registry.
pub trait Normaliser: Send + Sync {
    /// The stable ID (`online_numbering`).
    fn id(&self) -> &'static str;

    /// One run over the transaction; returns whether it changed anything. Must be idempotent: a second
    /// run on its own output changes nothing.
    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError>;
}

/// The registry order within one pass: the IDs of [`registry`], asserted by a test.
pub const ORDER: [&str; 5] = [
    "cp_instantiation",
    "online_numbering",
    "connection_split",
    "junction_cleanup",
    "wire_numbering",
];

/// The maximum number of passes.
pub const MAX_PASSES: usize = 3;

/// The normalisers in registry order.
pub fn registry() -> Vec<Box<dyn Normaliser>> {
    vec![
        Box::new(CpInstantiation),
        Box::new(OnlineNumbering),
        Box::new(ConnectionSplit),
        Box::new(JunctionCleanup),
        Box::new(WireNumbering),
    ]
}

/// Runs passes of `normalisers` until one changes nothing; returns the number of passes run (1 when the
/// command's output was already normal).
pub fn normalise(tx: &mut TxBuilder<'_>, normalisers: &[Box<dyn Normaliser>]) -> Result<usize, CmdError> {
    let command = tx.source();
    for pass in 1..=MAX_PASSES {
        let mut changed = false;
        for n in normalisers {
            tx.set_source(n.id());
            changed |= n.run(tx)?;
        }
        if !changed {
            tx.set_source(command);
            return Ok(pass);
        }
    }
    tx.set_source(command);
    Err(CmdError::internal(format!(
        "normalisers did not converge within {MAX_PASSES} passes after `{command}`"
    )))
}
