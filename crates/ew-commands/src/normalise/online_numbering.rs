// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #4, online numbering (doc 06 §3.2, FR-NUM-007, FR-NUM-029), page-based scheme.
//!
//! **Candidates** (F3): the devices whose numbering this transaction changes: a new device, a device whose
//! tag letter or counter changed, and a device whose main page (the page of its main representation)
//! changed. A device that is merely touched (another field edited, a function or an auxiliary
//! representation changed) is not a candidate. For each candidate that takes part in numbering
//! ([`crate::numbering::numbered`]) and is placed, in the order of the anchor cells of the main
//! representations ([`crate::numbering::PlaceKey`]: column, then row, ties by the anchor point; FR-NUM-028,
//! FT-456), never in ID order (the IDs one transaction mints are not monotonic, so a replay would differ):
//!
//! - without a counter, it gets the next free counter of its scope;
//! - with a counter that another numbered device of the scope also holds, it gets the next free counter
//!   only when that collision is **new**, i.e. the two devices did not already hold the same tag in the
//!   base snapshot, and the other device is not a candidate (it was there first) or is a candidate earlier
//!   in that order.
//!
//! So moving -101F1's main representation onto a folio that already has an -F1 renumbers the moved device,
//! never the one that stayed, and editing an unrelated field of a device whose tag already collided in a
//! lenient or legacy project changes nothing but that field (FR-NUM-029 renumbers only when an edit
//! *makes* a tag collide). `fields.set` of a colliding counter or letter is rejected by the command itself
//! (D2), so this normaliser renumbers only new devices, moves and page changes.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_model::AnyRow;
use ew_schema::{Device, FnRole, Function, Kind, Representation};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::numbering::{PlaceKey, collided_in, main_page, main_page_in, number_devices, place_key};
use crate::tx::TxBuilder;

/// Normaliser #4.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnlineNumbering;

/// The devices touched by the transaction: changed device rows, devices of changed functions (old or new
/// `device`) and devices whose main function a changed representation draws (old or new row).
fn touched(tx: &TxBuilder<'_>) -> BTreeSet<Id<Device>> {
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
    // A strip without a segment is placed through its terminals (`numbering::main_rep`): a touched
    // terminal touches its strip (old and new).
    let strips: Vec<Id<Device>> = out
        .iter()
        .flat_map(|d| {
            let now = tx
                .get::<Device>(*d)
                .and_then(|dev| dev.aspects.terminal.as_ref().map(|t| t.strip));
            let before = tx.base_row(&(Kind::Device, d.cast())).and_then(|r| {
                r.get::<Device>()
                    .and_then(|dev| dev.aspects.terminal.as_ref().map(|t| t.strip))
            });
            [now, before]
        })
        .flatten()
        .collect();
    out.extend(strips);
    out
}

/// The candidates (see the module doc): touched devices that are new, whose tag letter or counter
/// changed, or whose main page changed.
fn candidates(tx: &TxBuilder<'_>) -> BTreeSet<Id<Device>> {
    let base = tx.base_model();
    touched(tx)
        .into_iter()
        .filter(|d| {
            let Some(now) = tx.get::<Device>(*d) else {
                return false;
            };
            let Some(before) = base.table::<Device>().get(d) else {
                return true;
            };
            now.tag.letter != before.tag.letter
                || now.tag.counter != before.tag.counter
                || main_page(tx, *d) != main_page_in(base, *d)
        })
        .collect()
}

impl Normaliser for OnlineNumbering {
    fn id(&self) -> &'static str {
        "online_numbering"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let candidates = candidates(tx);
        // The candidates in place order (FR-NUM-028, FT-456), never in ID order: the IDs one transaction mints
        // are not monotonic. The counters of each (scope, letter) group are loaded once per run (NFR-PERF-08)
        // by the shared loop.
        let mut keyed: Vec<(PlaceKey, Id<Device>)> =
            candidates.iter().map(|d| (place_key(tx, *d), *d)).collect();
        keyed.sort();
        let rank: BTreeMap<Id<Device>, usize> = keyed.iter().enumerate().map(|(i, (_, d))| (*d, i)).collect();
        let order: Vec<Id<Device>> = keyed.into_iter().map(|(_, d)| d).collect();
        number_devices(tx, &order, |tx, d, dev, group| match dev.tag.counter {
            None => true,
            Some(counter) => {
                let base = tx.base_model();
                // The other holder was there first: not a candidate, or a candidate earlier in place order.
                let earlier = |other: &Id<Device>| rank.get(other).is_none_or(|r| Some(r) < rank.get(&d));
                group
                    .holders(counter)
                    .any(|other| other != d && earlier(&other) && !collided_in(base, d, other))
            }
        })
    }
}
