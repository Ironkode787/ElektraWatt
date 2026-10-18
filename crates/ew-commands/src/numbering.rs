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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::Ident;
use ew_model::Model;
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

/// The page of a device's main representation in a snapshot (the base of a transaction), by the same
/// rule as [`main_page`].
pub fn main_page_in(model: &Model, device: Id<Device>) -> Option<Id<Page>> {
    let functions = model.table::<Function>();
    let f = model
        .indexes()
        .functions_by_device()
        .get(&device)
        .find(|f| functions.get(f).is_some_and(|f| f.role == FnRole::Main))?;
    let rep = model.indexes().reps_by_function().get(f).next()?;
    model.table::<Representation>().get(rep).map(|r| r.page)
}

/// Whether two devices held the same page-based tag in a snapshot: both numbered, the same letter and
/// counter, and the same page of the main representation.
pub fn collided_in(model: &Model, a: Id<Device>, b: Id<Device>) -> bool {
    let devices = model.table::<Device>();
    let (Some(da), Some(db)) = (devices.get(&a), devices.get(&b)) else {
        return false;
    };
    numbered(da)
        && numbered(db)
        && da.tag.letter == db.tag.letter
        && da.tag.counter.is_some()
        && da.tag.counter == db.tag.counter
        && main_page_in(model, a).is_some_and(|p| main_page_in(model, b) == Some(p))
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
///
/// Scans the scope once; code that asks repeatedly for one scope loads a [`ScopeCounters`] instead.
pub fn next_free_counter(
    tx: &TxBuilder<'_>,
    letter: &Ident,
    scope: CounterScope,
    exclude: Option<Id<Device>>,
) -> u32 {
    ScopeCounters::load(tx, letter, scope).next_free(exclude)
}

/// The counters of one (scope, letter) group, loaded once and kept up to date by [`Self::set_counter`], so
/// that many collision checks and "next free counter" asks in one run cost one scan of the page instead of
/// one each (NFR-PERF-08).
#[derive(Clone, Debug)]
pub struct ScopeCounters {
    /// The counter of every numbered device of the group.
    members: BTreeMap<Id<Device>, Option<u32>>,
    /// The devices that hold each counter (no empty sets).
    holders: BTreeMap<u32, BTreeSet<Id<Device>>>,
    /// A lower bound of the lowest counter nobody holds.
    hint: u32,
}

impl ScopeCounters {
    /// The group (`letter`, `scope`) as the transaction sees it now (the devices of [`devices_in_scope`]).
    pub fn load(tx: &TxBuilder<'_>, letter: &Ident, scope: CounterScope) -> ScopeCounters {
        let mut out = ScopeCounters {
            members: BTreeMap::new(),
            holders: BTreeMap::new(),
            hint: 1,
        };
        for (d, counter) in devices_in_scope(tx, letter, scope) {
            out.members.insert(d, counter);
            if let Some(c) = counter {
                out.holders.entry(c).or_default().insert(d);
            }
        }
        out
    }

    /// The devices of the group that hold `counter`, in ID order.
    pub fn holders(&self, counter: u32) -> impl Iterator<Item = Id<Device>> + '_ {
        self.holders.get(&counter).into_iter().flatten().copied()
    }

    /// The lowest positive counter that no device of the group holds, other than `exclude`.
    pub fn next_free(&mut self, exclude: Option<Id<Device>>) -> u32 {
        // `hint` becomes the lowest counter nobody holds.
        while self.hint < u32::MAX && self.holders.contains_key(&self.hint) {
            self.hint += 1;
        }
        // A device holds one counter, so the only counter below `hint` that can be free when `exclude` is
        // left out is the excluded device's own, when nobody else holds it.
        let own = exclude
            .and_then(|d| self.members.get(&d).copied().flatten().map(|c| (d, c)))
            .filter(|(d, c)| self.holders.get(c).is_some_and(|h| h.len() == 1 && h.contains(d)))
            .map(|(_, c)| c);
        own.map_or(self.hint, |c| c.min(self.hint))
    }

    /// Records that `device` (a member of the group) now holds `counter`.
    pub fn set_counter(&mut self, device: Id<Device>, counter: u32) {
        let old = self.members.insert(device, Some(counter)).flatten();
        if let Some(old) = old {
            if let Some(h) = self.holders.get_mut(&old) {
                h.remove(&device);
                if h.is_empty() {
                    self.holders.remove(&old);
                    self.hint = self.hint.min(old).max(1);
                }
            }
        }
        self.holders.entry(counter).or_default().insert(device);
    }
}
