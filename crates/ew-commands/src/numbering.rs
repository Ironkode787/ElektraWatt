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
//! A strip without a segment takes the main representation of its first placed terminal, and a device whose main
//! function is not drawn takes the smallest-ID representation of its other functions ([`main_rep`]).
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
use ew_schema::{
    Device, FnRole, FormDef, FrameGrid, Function, LibCopy, Page, PageType, RepType, Representation,
    SymbolDef, TagScheme,
};

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

/// The auxiliary fallback of [`main_rep`]: when the main function is not drawn (a contact placed from a part's
/// function template before its coil), the smallest-ID representation of any function of the device that is not a
/// contact image, so the device is placed through it and gets a page for `{page}` and for numbering. A strip
/// without a segment tries its terminals first. `ew_derive::MainRep` applies the same rule.
fn aux_main_rep(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Arc<Representation>> {
    tx.functions_of_device(device)
        .into_iter()
        .flat_map(|f| tx.reps_of_function(f))
        .filter_map(|rid| tx.get::<Representation>(rid))
        .filter(|r| r.rep_type != RepType::ContactImage)
        .min_by_key(|r| r.id)
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
/// (Golden A's 1X1, owner ruling for Golden B step B1). A device whose main function is not drawn otherwise takes
/// the smallest-ID representation of its other functions that is not a contact image (the auxiliary fallback, a
/// contact placed from a part before its coil). `ew_derive::MainRep` applies the same rule.
pub fn main_rep(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Arc<Representation>> {
    if let Some(rep) = own_main_rep(tx, device) {
        return Some(rep);
    }
    if tx.get::<Device>(device)?.aspects.strip.is_none() {
        return aux_main_rep(tx, device);
    }
    strip_main_rep(tx, device).or_else(|| aux_main_rep(tx, device))
}

/// The strip fallback of [`main_rep`]: the own main representation of the strip's first placed terminal.
fn strip_main_rep(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Arc<Representation>> {
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

/// The two places the command layer reads records from: a transaction's view ([`TxBuilder`], with its
/// pending writes) and a snapshot ([`Model`]). [`anchor_cell`] and [`anchor_cell_in`] share one body over it.
trait Rows {
    fn row<T: ew_model::Entity>(&self, id: ew_model::RowId<T>) -> Option<Arc<T>>;
    fn project_settings_frames(&self) -> Option<serde_json::Value>;
}

impl Rows for TxBuilder<'_> {
    fn row<T: ew_model::Entity>(&self, id: ew_model::RowId<T>) -> Option<Arc<T>> {
        self.get::<T>(id)
    }

    fn project_settings_frames(&self) -> Option<serde_json::Value> {
        self.project().settings.extra.get("frames").cloned()
    }
}

impl Rows for Model {
    fn row<T: ew_model::Entity>(&self, id: ew_model::RowId<T>) -> Option<Arc<T>> {
        self.table::<T>().get(&id).cloned()
    }

    fn project_settings_frames(&self) -> Option<serde_json::Value> {
        self.project().settings.extra.get("frames").cloned()
    }
}

/// `anchor_cell(rep)`: the frame cell of a representation's anchor point, read from the model alone (no derived
/// query), equal to `ew_derive::RepAnchorCell` (doc 06 §5.1). Normalisers see only the transaction's model, so
/// online numbering orders by this copy; `ew-derive`'s `tests/agreement.rs` holds the two together, like
/// [`main_rep`] and `ew_derive::MainRep`.
///
/// The anchor point is `ew_derive::anchor_point_of` (the representation's or its symbol's anchor policy, mapped by
/// orientation and position); the frame grid is the page's own form, else the `frames` settings shim's entry of
/// the page type, else its default (`ew_derive::frame_form_from_settings`; a form the project does not hold falls
/// through), else the built-in grid, except on title, table-of-contents and divider pages (no grid); a held form
/// without a `grid` has no cells. `None` for a missing representation, symbol or page, outside the drawing area,
/// or on overflow.
pub fn anchor_cell(tx: &TxBuilder<'_>, rep: Id<Representation>) -> Option<ew_derive::Cell> {
    anchor_cell_with(tx, rep)
}

/// [`anchor_cell`] in a snapshot.
pub fn anchor_cell_in(model: &Model, rep: Id<Representation>) -> Option<ew_derive::Cell> {
    anchor_cell_with(model, rep)
}

/// The anchor point of a representation on its page (`ew_derive::anchor_point_of`), read from the model alone.
fn anchor_point_with(rows: &impl Rows, rep: &Representation) -> Option<ew_units::Pt2> {
    let symbol = rows.row::<LibCopy<SymbolDef>>(rep.symbol)?;
    ew_derive::anchor_point_of(&symbol.item, rep)
}

fn anchor_cell_with(rows: &impl Rows, rep: Id<Representation>) -> Option<ew_derive::Cell> {
    let rep = rows.row::<Representation>(rep)?;
    let point = anchor_point_with(rows, &rep)?;
    let page = rows.row::<Page>(rep.page)?;
    let grid = frame_grid_with(rows, &page)?;
    ew_derive::cell_in_grid(&grid, page.grid, point)
}

/// The frame grid of a page, by the rule of `ew_derive::FrameGridOf`.
fn frame_grid_with(rows: &impl Rows, page: &Page) -> Option<FrameGrid> {
    let held = |id: Id<FormDef>| rows.row::<LibCopy<FormDef>>(id).is_some();
    let form = page.form.filter(|f| held(*f)).or_else(|| {
        let frames = rows.project_settings_frames();
        let settings = ew_derive::FrameSettingsValue::parse(frames.as_ref(), |_| {});
        ew_derive::frame_form_from_settings(&settings, &page.kind, true, held)
    });
    match form {
        None if matches!(
            page.kind,
            PageType::TitlePage | PageType::TableOfContents | PageType::SectionDivider
        ) =>
        {
            None
        }
        None => Some(ew_derive::default_frame_grid()),
        Some(form) => rows.row::<LibCopy<FormDef>>(form)?.item.grid.clone(),
    }
}

/// The order online numbering gives the devices it numbers (FR-NUM-028, FT-456): by the anchor cell of the main
/// representation ([`anchor_cell`]: column, then row; a device without a cell after those with one), ties by the
/// anchor point (x, then y), then by ID (devices anchored on the same point). Unplaced devices are not numbered;
/// they sort with the devices without a cell. Never by ID alone: the IDs one transaction mints are not monotonic,
/// so an ID order would differ between a session and its replay.
pub type PlaceKey = (bool, Option<(u32, u32)>, Option<(i64, i64)>, Id<Device>);

/// The [`PlaceKey`] of a device in a transaction.
pub fn place_key(tx: &TxBuilder<'_>, device: Id<Device>) -> PlaceKey {
    let Some(rep) = main_rep(tx, device) else {
        return (true, None, None, device);
    };
    let cell = anchor_cell(tx, rep.id).map(|c| (c.column, c.row));
    let point = anchor_point_with(tx, &rep).map(|p| (p.x.0, p.y.0));
    (cell.is_none(), cell, point, device)
}

/// The page of a device's main representation in a snapshot (the base of a transaction), by the same
/// rule as [`main_page`].
pub fn main_page_in(model: &Model, device: Id<Device>) -> Option<Id<Page>> {
    if let Some(page) = own_main_page_in(model, device) {
        return Some(page);
    }
    let devices = model.table::<Device>();
    devices
        .get(&device)?
        .aspects
        .strip
        .as_ref()
        .and_then(|_| {
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
        })
        .or_else(|| aux_main_page_in(model, device))
}

/// The auxiliary fallback of [`main_page_in`] (see [`main_rep`]).
fn aux_main_page_in(model: &Model, device: Id<Device>) -> Option<Id<Page>> {
    let reps = model.table::<Representation>();
    model
        .indexes()
        .functions_by_device()
        .get(&device)
        .flat_map(|f| model.indexes().reps_by_function().get(f))
        .filter_map(|rid| reps.get(rid))
        .filter(|r| r.rep_type != RepType::ContactImage)
        .min_by_key(|r| r.id)
        .map(|r| r.page)
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
            // Any function, not only the main one: a device whose main function is not drawn is placed through
            // its other functions ([`main_rep`]); the `main_page` filter below keeps the scope exact.
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
/// The one loop of online numbering: normaliser #4 calls it with its candidates in [`PlaceKey`] order and its
/// collision rule (FR-NUM-028, FT-456; `edit.paste` no longer numbers its copies itself).
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
