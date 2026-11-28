// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser `bond_wires`: a bond's wire (`Bond.via`) runs between the bond's two ends (`bond.create` refuses
//! another), whichever command changes the wire afterwards.
//!
//! - **Trigger.** A bond whose wire ran between its two ends before the transaction, where the transaction changed or
//!   removed the bond or its wire: a split by a wire drawn onto it, an end moved or reconnected, the wire joined into
//!   another or deleted.
//! - **Effect.** Such a bond whose wire is gone or no longer joins its two ends loses its wire in the same step, with
//!   a notice that names the bond and says to choose its conductor again; it reads as a bond made without a wire. A
//!   wire only re-routed or bent still joins the ends, and the bond keeps it.
//!
//! A bond the transaction does not touch, and one whose wire was off its ends already, is left as it is (opening a
//! project, or a command elsewhere, changes nothing of it), but a bond never names a removed wire: that one loses it
//! silently, as junction clean-up's removals and the delete policies leave it (doc 03 §4.2). Idempotent: after a run
//! the bond has no wire, so a second run finds nothing.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{Bond, Connection, Endpoint, Kind};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Whether the wire `c` joins the ends `a` and `b`, in either direction: the rule of a bond's wire, which `bond.create`
/// refuses to break and this normaliser keeps.
pub(crate) fn joins(c: &Connection, a: &Endpoint, b: &Endpoint) -> bool {
    (c.a == *a && c.b == *b) || (c.a == *b && c.b == *a)
}

/// Normaliser `bond_wires`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BondWires;

impl Normaliser for BondWires {
    fn id(&self) -> &'static str {
        "bond_wires"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let wires: BTreeSet<Id<Connection>> = tx
            .changed_of(Kind::Connection)
            .into_iter()
            .map(|k| k.1.cast())
            .collect();
        let bonds: BTreeSet<Id<Bond>> = tx
            .changed_of(Kind::Bond)
            .into_iter()
            .map(|k| k.1.cast())
            .collect();
        if wires.is_empty() && bonds.is_empty() {
            return Ok(false);
        }
        let mut changed = false;
        for bond in tx.iter::<Bond>() {
            let Some(via) = bond.via else {
                continue;
            };
            if !wires.contains(&via) && !bonds.contains(&bond.id) {
                continue;
            }
            // A wire that ran between the bond's ends before and does not now; one off them already is not this
            // command's doing and stays, unless it is gone (a bond never names a removed wire).
            let base_bond = tx
                .base_row(&(Kind::Bond, bond.id.cast()))
                .and_then(|r| r.get::<Bond>().cloned());
            let base_wire = tx
                .base_row(&(Kind::Connection, via.cast()))
                .and_then(|r| r.get::<Connection>().cloned());
            let ran = match (&base_bond, &base_wire) {
                (Some(b), Some(c)) => b.via == Some(via) && joins(c, &b.a, &b.b),
                _ => false,
            };
            let now = tx.get::<Connection>(via);
            match &now {
                Some(c) if !ran || joins(c, &bond.a, &bond.b) => continue,
                _ => {}
            }
            tx.update::<Bond>(bond.id, |b| b.via = None)?;
            if ran {
                tx.notice(text!(
                    "notice-bond-wire-lost",
                    crate::words::endpoint(tx, &bond.a),
                    crate::words::endpoint(tx, &bond.b)
                ));
            }
            changed = true;
        }
        Ok(changed)
    }
}
