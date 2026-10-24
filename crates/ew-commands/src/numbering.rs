// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Online numbering of device counters, page-based scheme only (FR-NUM-007, FR-NUM-029). The full
//! numbering engine with schemes, structure scopes and tombstones is WP-S1.11.
//!
//! **Scope.** The printed tag of the page-based scheme is `{page}{letter}{counter}`; the page is the page
//! of the device's main representation. A counter is therefore unique per (page of the main
//! representation, letter). The main representation is the representation with the smallest ID that
//! draws the device's main function and is not a contact image (doc 03 §5.1: a contact image never counts
//! as the main representation; the main function: role `Main`, smallest ID if the data holds more).
//! A strip without a segment takes the main representation of its first placed terminal ([`main_rep`]).
//! `ew_derive::MainRep` applies the same rules; its `tests/agreement.rs` holds the two together.
//!
//! **Which devices take part.** A device takes part in page-based numbering when it has no terminal
//! aspect (a terminal is named by its strip and designation), no frozen tag and the default scheme. An
//! unplaced device (no main representation) has no scope: it keeps its counter, and a device without a
//! counter gets one when it is placed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, NaturalKey};
use ew_model::Model;
use ew_schema::{Device, FnRole, Function, Page, RepType, Representation, TagScheme};

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

/// The own main representation of a device: the smallest-ID representation of its main function that is
/// not a contact image.
fn own_main_rep(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Arc<Representation>> {
    let f = main_function(tx, device)?;
    tx.reps_of_function(f)
        .into_iter()
        .filter_map(|rid| tx.get::<Representation>(rid))
        .find(|r| r.rep_type != RepType::ContactImage)
}

/// The order key of a placed terminal for the strip fallback of [`main_rep`]: page (folio number, folio
/// label in natural order, page ID), then designation (natural order), then ID.
type TerminalKey = (u32, Option<NaturalKey>, Id<Page>, NaturalKey, Id<Device>);

fn terminal_key(page: &Page, designation: &Ident, terminal: Id<Device>) -> TerminalKey {
    (
        page.folio.number,
        page.folio.label.as_ref().map(Ident::natural_key),
        page.id,
        designation.natural_key(),
        terminal,
    )
}

/// The main representation of a device: the smallest-ID representation of its main function that is not
/// a contact image. A strip without a segment (a strip aspect and no such representation) takes the main
/// representation of its first placed terminal: lowest page, then lowest designation, then smallest ID
/// (Golden A's 1X1, owner ruling for Golden B step B1). `ew_derive::MainRep` applies the same rule.
pub fn main_rep(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Arc<Representation>> {
    if let Some(rep) = own_main_rep(tx, device) {
        return Some(rep);
    }
    tx.get::<Device>(device)?.aspects.strip.as_ref()?;
    tx.terminals_of_strip(device)
        .into_iter()
        .filter_map(|t| {
            let designation = tx
                .get::<Device>(t)?
                .aspects
                .terminal
                .as_ref()?
                .designation
                .clone();
            let rep = own_main_rep(tx, t)?;
            let page = tx.get::<Page>(rep.page)?;
            Some((terminal_key(&page, &designation, t), rep))
        })
        .min_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, rep)| rep)
}

/// The page of a device's main representation.
pub fn main_page(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Id<Page>> {
    main_rep(tx, device).map(|r| r.page)
}

/// The page of a device's main representation in a snapshot (the base of a transaction), by the same
/// rule as [`main_page`].
pub fn main_page_in(model: &Model, device: Id<Device>) -> Option<Id<Page>> {
    if let Some(page) = own_main_page_in(model, device) {
        return Some(page);
    }
    let devices = model.table::<Device>();
    devices.get(&device)?.aspects.strip.as_ref()?;
    let pages = model.table::<Page>();
    model
        .indexes()
        .terminals_by_strip()
        .get(&device)
        .filter_map(|t| {
            let designation = &devices.get(t)?.aspects.terminal.as_ref()?.designation;
            let page = pages.get(&own_main_page_in(model, *t)?)?;
            Some(terminal_key(page, designation, *t))
        })
        .min()
        .map(|key| key.2)
}

fn own_main_page_in(model: &Model, device: Id<Device>) -> Option<Id<Page>> {
    let functions = model.table::<Function>();
    let f = model
        .indexes()
        .functions_by_device()
        .get(&device)
        .find(|f| functions.get(f).is_some_and(|f| f.role == FnRole::Main))?;
    let reps = model.table::<Representation>();
    model
        .indexes()
        .reps_by_function()
        .get(f)
        .filter_map(|rid| reps.get(rid))
        .find(|r| r.rep_type != RepType::ContactImage)
        .map(|r| r.page)
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
                // A strip without a segment is placed through its terminals ([`main_rep`]).
                if let Some(t) = tx.get::<Device>(d).and_then(|dev| dev.aspects.terminal.clone()) {
                    devices.insert(t.strip);
                }
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

/// Numbers `devices` in the given order (FR-NUM-007): each one that takes part in page-based numbering
/// ([`numbered`]) and is placed (it has a [`main_page`]) gets the next free counter of its (main page, letter)
/// scope when `renumber` says so; the counters of each scope are loaded once ([`ScopeCounters`]) and kept up to
/// date. `renumber` sees the device, its record and its scope's counters. Returns whether a counter changed.
///
/// The one loop of online numbering: normaliser #4 calls it with its candidates in ID order and its collision
/// rule, `edit.paste` with the pasted devices in fragment order and "no counter yet" (WP-S2.05 part 3), because
/// the IDs one transaction mints are not monotonic. The paste's own call goes away once the normaliser orders new
/// devices by position (FR-NUM-028).
///
/// # Errors
///
/// A device that vanished between the read and the write ([`crate::error::CmdError::NotFound`]).
pub fn number_devices(
    tx: &mut TxBuilder<'_>,
    devices: &[Id<Device>],
    mut renumber: impl FnMut(&TxBuilder<'_>, Id<Device>, &Device, &ScopeCounters) -> bool,
) -> Result<bool, crate::error::CmdError> {
    let mut changed = false;
    let mut groups: BTreeMap<(CounterScope, Ident), ScopeCounters> = BTreeMap::new();
    for d in devices {
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
        let group = groups
            .entry((scope, dev.tag.letter.clone()))
            .or_insert_with(|| ScopeCounters::load(tx, &dev.tag.letter, scope));
        if renumber(tx, *d, &dev, group) {
            let next = group.next_free(Some(*d));
            tx.update::<Device>(*d, |dev| dev.tag.counter = Some(next))?;
            group.set_counter(*d, next);
            changed = true;
        }
    }
    Ok(changed)
}
