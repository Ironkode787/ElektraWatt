// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Online numbering of device counters, page-based scheme only (FR-NUM-007, FR-NUM-029). The full
//! numbering engine with schemes, structure scopes and tombstones is WP-S1.11.
//!
//! **Scope.** The printed tag of the page-based scheme is `{page}{letter}{counter}`; the page is the page
//! of the device's main representation. A counter is therefore unique per (page of the main
//! representation, letter). The main representation is the representation with the smallest ID that
//! draws the device's main function (the main function: role `Main`, smallest ID if the data holds more).
//!
//! **Which devices take part.** A device takes part in page-based numbering when it has no terminal
//! aspect (a terminal is named by its strip and designation), no frozen tag and the default scheme. An
//! unplaced device (no main representation) has no scope: it keeps its counter, and a device without a
//! counter gets one when it is placed.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{Device, FnRole, Function, Page, Representation, TagScheme};

use crate::tx::TxBuilder;

/// A numbering scope.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum CounterScope {
    /// The devices whose main representation is on this page.
    Page(Id<Page>),
}

/// Whether a device takes part in page-based numbering.
pub fn numbered(device: &Device) -> bool {
    device.aspects.terminal.is_none()
        && device.tag.frozen.is_none()
        && device.tag.scheme == TagScheme::Default
}

/// The main function of a device: role `Main`, the smallest ID if several.
pub fn main_function(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Id<Function>> {
    tx.functions_of_device(device)
        .into_iter()
        .find(|f| tx.get::<Function>(*f).is_some_and(|f| f.role == FnRole::Main))
}

/// The main representation of a device: the smallest-ID representation of its main function.
pub fn main_rep(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Arc<Representation>> {
    let f = main_function(tx, device)?;
    let rid = tx.reps_of_function(f).into_iter().next()?;
    tx.get::<Representation>(rid)
}

/// The page of a device's main representation.
pub fn main_page(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Id<Page>> {
    main_rep(tx, device).map(|r| r.page)
}

/// The numbered devices in a scope with a given letter, with their counters, in ID order.
pub fn devices_in_scope(
    tx: &TxBuilder<'_>,
    letter: &Ident,
    scope: CounterScope,
) -> Vec<(Id<Device>, Option<u32>)> {
    let CounterScope::Page(page) = scope;
    let mut devices: BTreeSet<Id<Device>> = BTreeSet::new();
    for rid in tx.reps_on_page(page) {
        let Some(rep) = tx.get::<Representation>(rid) else {
            continue;
        };
        for fid in rep
            .functions
            .iter()
            .chain(rep.pin_map.as_slice().iter().map(|p| &p.function))
        {
            let Some(f) = tx.get::<Function>(*fid) else {
                continue;
            };
            if f.role != FnRole::Main {
                continue;
            }
            if let Some(d) = f.device {
                devices.insert(d);
            }
        }
    }
    devices
        .into_iter()
        .filter_map(|d| {
            let dev = tx.get::<Device>(d)?;
            if !numbered(&dev) || dev.tag.letter != *letter || main_page(tx, d) != Some(page) {
                return None;
            }
            Some((d, dev.tag.counter))
        })
        .collect()
}

/// The lowest positive counter not used by another numbered device with this letter in the scope (gaps
/// are filled, FR-NUM-007). `exclude` leaves one device's own counter out.
pub fn next_free_counter(
    tx: &TxBuilder<'_>,
    letter: &Ident,
    scope: CounterScope,
    exclude: Option<Id<Device>>,
) -> u32 {
    let used: BTreeSet<u32> = devices_in_scope(tx, letter, scope)
        .into_iter()
        .filter(|(d, _)| Some(*d) != exclude)
        .filter_map(|(_, c)| c)
        .collect();
    (1..).find(|c| !used.contains(c)).unwrap_or(u32::MAX)
}
