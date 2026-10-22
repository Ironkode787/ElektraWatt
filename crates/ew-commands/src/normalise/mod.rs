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
//! | 3 | `channel_instantiation` | N1: a new module, or a device that gained the `plc_module` aspect or changed its parts, gets the channels of its part data with v5 IDs and the part's channel kind (doc 10 §3.3; surplus channels stay until `plc.set_module_part` removes them after its preview) |
//! | 4 | `online_numbering` | Gives a placed numbered device without a counter the next free counter of its page scope, and a device whose page-based tag collides (after a move of its main representation) the next free one (FR-NUM-007, FR-NUM-029) |
//! | 7 | `connection_split` | A new representation whose two CPs lie on one segment of a connection's route splits that connection (FR-CON-008, doc 06 §5.3) |
//! | 8 | `junction_cleanup` | A junction left with two connections dissolves into one; with one, it is removed together with that connection; with none, it is removed (CI-04) |
//! | 12 | `wire_numbering` | Online wire numbering is off in v0 (no scheme before WP-S1.11): every new connection is `NumberSpec::Unassigned` |
//! | 13 | `solenoid_binding` | N2: a valve added to or moved within a module whose part has a solenoid-position map, or a new channel of such a module, binds the declared coil to the channel the map assigns when both are free (doc 10 §3.3, FR-PLC-051) |
//! | 14 | `module_pin_binding` | N3: a new unbound channel of a module whose part has a channel → pin map binds the module pin (doc 10 §3.3) |
//!
//! A pass runs every normaliser in order; a normaliser whose input another one creates later in the same
//! pass sees it in the next pass. The pipeline runs passes until one changes nothing. [`MAX_PASSES`]
//! counts **productive** passes (D1): up to three passes may change something, then a confirming pass
//! must change nothing; a fourth productive pass is non-convergence, a bug
//! ([`crate::CmdError::Internal`]). Every normaliser breaks ties by ID and is idempotent on its own
//! output.

pub(crate) mod channel_instantiation;
mod connection_split;
mod cp_instantiation;
mod junction_cleanup;
mod module_pin_binding;
mod online_numbering;
mod solenoid_binding;
mod wire_numbering;

pub use channel_instantiation::ChannelInstantiation;
pub use connection_split::{ConnectionSplit, split_connection};
pub use cp_instantiation::CpInstantiation;
pub use junction_cleanup::JunctionCleanup;
pub use module_pin_binding::ModulePinBinding;
pub use online_numbering::OnlineNumbering;
pub use solenoid_binding::SolenoidBinding;
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
pub const ORDER: [&str; 8] = [
    "cp_instantiation",
    "channel_instantiation",
    "online_numbering",
    "connection_split",
    "junction_cleanup",
    "wire_numbering",
    "solenoid_binding",
    "module_pin_binding",
];

/// The maximum number of productive passes (passes that change something); one confirming pass follows.
pub const MAX_PASSES: usize = 3;

/// The normalisers in registry order.
pub fn registry() -> Vec<Box<dyn Normaliser>> {
    vec![
        Box::new(CpInstantiation),
        Box::new(ChannelInstantiation),
        Box::new(OnlineNumbering),
        Box::new(ConnectionSplit),
        Box::new(JunctionCleanup),
        Box::new(WireNumbering),
        Box::new(SolenoidBinding),
        Box::new(ModulePinBinding),
    ]
}

/// Runs passes of `normalisers` until one changes nothing; returns the number of passes run, the
/// confirming one included (1 when the command's output was already normal, at most
/// `MAX_PASSES + 1`).
pub fn normalise(tx: &mut TxBuilder<'_>, normalisers: &[Box<dyn Normaliser>]) -> Result<usize, CmdError> {
    let command = tx.source();
    let mut productive = 0;
    let result = loop {
        let mut changed = false;
        for n in normalisers {
            tx.set_source(n.id());
            match n.run(tx) {
                Ok(c) => changed |= c,
                Err(e) => {
                    tx.set_source(command);
                    return Err(e);
                }
            }
        }
        if !changed {
            break Ok(productive + 1);
        }
        productive += 1;
        if productive > MAX_PASSES {
            break Err(CmdError::internal(format!(
                "normalisers did not converge: {productive} passes after `{command}` changed something \
                 (at most {MAX_PASSES} may)"
            )));
        }
    };
    tx.set_source(command);
    result
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A normaliser that reports a change on its first `productive` runs, then none.
    struct Flaky {
        productive: usize,
        runs: AtomicUsize,
    }

    impl Normaliser for Flaky {
        fn id(&self) -> &'static str {
            "flaky"
        }

        fn run(&self, _tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
            Ok(self.runs.fetch_add(1, Ordering::Relaxed) < self.productive)
        }
    }

    fn passes(productive: usize) -> Result<usize, CmdError> {
        let project = serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
        )
        .expect("project");
        let model = ew_model::Model::new(project);
        let mut ids = ew_ids::SystemSource;
        let mut tx = TxBuilder::new(&model, &mut ids);
        tx.set_source("test.command");
        let normalisers: Vec<Box<dyn Normaliser>> = vec![Box::new(Flaky {
            productive,
            runs: AtomicUsize::new(0),
        })];
        let result = normalise(&mut tx, &normalisers);
        assert_eq!(tx.source(), "test.command", "the command's source is restored");
        result
    }

    /// D1: `MAX_PASSES` counts productive passes; one confirming pass follows them.
    #[test]
    fn max_passes_counts_productive_passes_and_one_confirming_pass() {
        assert_eq!(passes(0).expect("normal"), 1);
        assert_eq!(passes(1).expect("one productive pass"), 2);
        assert_eq!(
            passes(MAX_PASSES).expect("three productive passes"),
            MAX_PASSES + 1
        );
        let err = passes(MAX_PASSES + 1).expect_err("a fourth productive pass is non-convergence");
        assert!(matches!(err, CmdError::Internal { .. }), "{err:?}");
    }
}
