// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #4, online numbering (doc 06 §3.2, FR-NUM-007, FR-NUM-029), page-based scheme.
//!
//! The candidates are the devices this transaction touched: a changed device row, a changed function of
//! the device (old or new `device`), or a changed representation drawing one of its main functions (old or
//! new row). For each candidate in ID order that takes part in numbering ([`crate::numbering::numbered`])
//! and is placed:
//!
//! - without a counter, it gets the next free counter of its scope;
//! - with a counter that another numbered device of the scope also holds, it gets the next free counter
//!   when that other device is not a candidate (it was there first) or is a candidate with a smaller ID.
//!
//! So moving -101F1's main representation onto a folio that already has an -F1 renumbers the moved device,
//! never the one that stayed, and no transaction leaves two equal page-based tags among touched devices.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::AnyRow;
use ew_schema::{Device, FnRole, Function, Kind, Representation};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::numbering::{CounterScope, devices_in_scope, main_page, next_free_counter, numbered};
use crate::tx::TxBuilder;

/// Normaliser #4.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnlineNumbering;

/// The devices touched by the transaction.
fn candidates(tx: &TxBuilder<'_>) -> BTreeSet<Id<Device>> {
    let mut out = BTreeSet::new();
    let device_of = |f: Id<Function>| -> Option<Id<Device>> {
        let f = tx.get::<Function>(f)?;
        (f.role == FnRole::Main).then_some(f.device).flatten()
    };
    for key in tx.changed_keys() {
        match key.0 {
            Kind::Device => {
                out.insert(key.1.cast());
            }
            Kind::Function => {
                for row in [tx.row(&key), tx.base_row(&key)].into_iter().flatten() {
                    if let AnyRow::Function(f) = row {
                        out.extend(f.device);
                    }
                }
            }
            Kind::Representation => {
                for row in [tx.row(&key), tx.base_row(&key)].into_iter().flatten() {
                    if let AnyRow::Representation(r) = row {
                        let r: &Representation = &r;
                        for f in r
                            .functions
                            .iter()
                            .chain(r.pin_map.as_slice().iter().map(|p| &p.function))
                        {
                            out.extend(device_of(*f));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

impl Normaliser for OnlineNumbering {
    fn id(&self) -> &'static str {
        "online_numbering"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let candidates = candidates(tx);
        let mut changed = false;
        for d in &candidates {
            let Some(dev) = tx.get::<Device>(*d) else {
                continue;
            };
            if !numbered(&dev) {
                continue;
            }
            let Some(page) = main_page(tx, *d) else {
                continue;
            };
            let scope = CounterScope::Page(page);
            let renumber = match dev.tag.counter {
                None => true,
                Some(counter) => {
                    devices_in_scope(tx, &dev.tag.letter, scope)
                        .into_iter()
                        .any(|(other, c)| {
                            other != *d && c == Some(counter) && (!candidates.contains(&other) || other < *d)
                        })
                }
            };
            if renumber {
                let next = next_free_counter(tx, &dev.tag.letter, scope, Some(*d));
                tx.update::<Device>(*d, |dev| dev.tag.counter = Some(next))?;
                changed = true;
            }
        }
        Ok(changed)
    }
}
