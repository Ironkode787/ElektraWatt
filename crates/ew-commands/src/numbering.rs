// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Online numbering of device counters, page-based scheme only (FR-NUM-007, FR-NUM-029). The full
//! numbering engine with schemes, structure scopes and tombstones is not built yet.
//!
//! **Scope.** The printed tag of the page-based scheme is `{page}{letter}{counter}`; the page is the page
//! of the device's main representation. A counter is therefore unique per (page of the main
//! representation, letter). The main representation is the representation with the smallest ID that
//! draws the device's main function and is not a contact image (doc 03 §5.1: a contact image never counts
//! as the main representation; the main function: role `Main`, smallest ID if the data holds more).
//! A strip without a segment takes the main representation of its first placed terminal, and a device whose main
//! function is not drawn takes the smallest-ID representation of its other functions ([`main_rep`]: the one rule of
//! `ew_derive::MainRep`, over the transaction). A cable without a drawn definition line is placed on the page of its
//! first connected core or of the first bus line it carries ([`main_page`]), as `ew_derive::cable_core_page` places it.
//!
//! **Which devices take part.** A device takes part in page-based numbering when it has no terminal
//! aspect (a terminal is named by its strip and designation), no frozen tag and the default scheme, and is no earth
//! connection (an earth symbol without a part has no tag, doc 03 §8.2: it takes no counter and holds none). An
//! unplaced device (no main representation) has no scope: it keeps its counter, and a device without a
//! counter gets one when it is placed. An unplaced cable (no definition line, no connected core, no bus line) takes the
//! project's counter: the scope of the cables on no page, whose tags print no page.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, NaturalKey};
use ew_model::Model;
use ew_schema::{
    Conductor, Connection, ConnectionKind, Device, FnRole, FormDef, FrameGrid, Function, LibCopy, Page,
    PageType, RepType, Representation, SymbolDef, TagScheme,
};

use crate::tx::TxBuilder;

/// A numbering scope.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum CounterScope {
    /// The devices whose main representation is on this page.
    Page(Id<Page>),
    /// The cables placed on no page: no definition line, no connected core and no bus line.
    Project,
}

/// Whether a device takes part in page-based numbering.
pub fn numbered(device: &Device) -> bool {
    device.aspects.terminal.is_none()
        && device.tag.frozen.is_none()
        && device.tag.scheme == TagScheme::Default
}

/// Whether a device is an earth connection, which has no tag and so takes no counter and holds none: no part
/// use, and every function an earth point (`ew_derive::layout::is_earth_connection` read in the transaction).
pub(crate) fn earth_connection(tx: &TxBuilder<'_>, device: &Device) -> bool {
    let functions = tx.functions_of_device(device.id);
    device.parts.is_empty()
        && !functions.is_empty()
        && functions.iter().all(|f| {
            tx.get::<Function>(*f)
                .and_then(|f| tx.get::<LibCopy<ew_schema::FnDef>>(f.def))
                .is_some_and(|d| ew_derive::layout::is_earth_point(&d.item.category))
        })
}

/// The main function of a device: role `Main`, the smallest ID if several.
pub fn main_function(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Id<Function>> {
    tx.functions_of_device(device)
        .into_iter()
        .find(|f| tx.get::<Function>(*f).is_some_and(|f| f.role == FnRole::Main))
}

/// The main representation of a device in a transaction, by the rule of `ew_derive::MainRep`
/// ([`ew_derive::main_rep_with`] over the transaction's view): the smallest-ID representation of its main function
/// that is not a contact image; a strip without a segment takes the main representation of its first placed terminal
/// (lowest page, then lowest designation, then smallest ID); a device whose main function is not drawn otherwise takes
/// the smallest-ID representation of its other functions that is not a contact image.
pub fn main_rep(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Arc<Representation>> {
    ew_derive::main_rep_with(tx, device).and_then(|r| tx.get::<Representation>(r))
}

/// The records of the main-representation rule as a transaction reads them, with its pending writes.
impl ew_derive::MainRepRows for TxBuilder<'_> {
    fn main_function(&self, device: Id<Device>) -> Option<Id<Function>> {
        main_function(self, device)
    }

    fn functions_of_device(&self, device: Id<Device>) -> impl Iterator<Item = Id<Function>> {
        TxBuilder::functions_of_device(self, device).into_iter()
    }

    fn reps_of_function(&self, function: Id<Function>) -> impl Iterator<Item = Id<Representation>> {
        TxBuilder::reps_of_function(self, function).into_iter()
    }

    fn counted_rep_page(&self, rep: Id<Representation>) -> Option<Id<Page>> {
        self.get::<Representation>(rep)
            .filter(|r| r.rep_type != RepType::ContactImage)
            .map(|r| r.page)
    }

    fn is_strip(&self, device: Id<Device>) -> Option<bool> {
        self.get::<Device>(device).map(|d| d.aspects.strip.is_some())
    }

    fn terminals_of_strip(&self, strip: Id<Device>) -> impl Iterator<Item = Id<Device>> {
        TxBuilder::terminals_of_strip(self, strip).into_iter()
    }

    fn terminal_designation(&self, terminal: Id<Device>) -> Option<NaturalKey> {
        let d = self.get::<Device>(terminal)?;
        Some(d.aspects.terminal.as_ref()?.designation.natural_key())
    }

    fn folio(&self, page: Id<Page>) -> Option<(u32, Option<NaturalKey>)> {
        let page = self.get::<Page>(page)?;
        Some((
            page.folio.number,
            page.folio.label.as_ref().map(Ident::natural_key),
        ))
    }
}

/// The page of a device's main representation; for a cable without a drawn definition line, the page of its first
/// connected core or bus line: of the connections that are a core of one of its conductors or a bus line it carries
/// and lie on a page, the one of the smallest ID (`ew_derive::cable_core_page`).
pub fn main_page(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Id<Page>> {
    if let Some(rep) = main_rep(tx, device) {
        return Some(rep.page);
    }
    tx.get::<Device>(device)?.aspects.cable.as_ref()?;
    let conductors: BTreeSet<Id<Conductor>> = tx.conductors_of_cable(device).into_iter().collect();
    core_page(tx.iter::<Connection>().iter().map(|c| &**c), device, &conductors)
}

/// The page of the smallest-ID connection of `connections` that is a core of one of `conductors` or a bus line that
/// `cable` carries, and lies on a page.
fn core_page<'c>(
    connections: impl Iterator<Item = &'c Connection>,
    cable: Id<Device>,
    conductors: &BTreeSet<Id<Conductor>>,
) -> Option<Id<Page>> {
    connections
        .filter(|c| match &c.kind {
            ConnectionKind::Core(k) => conductors.contains(k),
            ConnectionKind::Bus(line) => line.cable == Some(cable),
            _ => false,
        })
        .filter_map(|c| c.page.map(|p| (c.id, p)))
        .min()
        .map(|(_, p)| p)
}

/// Whether a device is a cable on no page, in the project's scope ([`CounterScope::Project`]).
fn unplaced_cable(device: &Device, page: Option<Id<Page>>) -> bool {
    page.is_none() && device.aspects.cable.is_some()
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
/// online numbering orders by this copy; `tests/it/agreement.rs` holds the two together.
///
/// The anchor point is `ew_derive::anchor_point_of` (the representation's or its symbol's anchor policy, mapped by
/// orientation and position); the frame grid is the page's own form, else the `frames` settings shim's entry of
/// the page type, else its default (`ew_derive::frame_form_from_settings`; a form the project does not hold falls
/// through), else the built-in grid, except on title, table-of-contents and divider pages (no grid); a held form
/// without a `grid` has no cells, nor has a frame whose rulers are not drawn (`ew_derive::printed_cell`). `None` for
/// a missing representation, symbol or page, outside the drawing area, or on overflow.
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
    ew_derive::printed_cell(&grid, page.grid, point)
}

/// The frame form a drawn page prints on, by the rule of `ew_derive::frame_form_of`: its own held form, else the
/// frame defaults' entry of its type, else their default; `None` for the built-in frame.
fn frame_form_with(rows: &impl Rows, page: &Page) -> Option<Id<FormDef>> {
    let held = |id: Id<FormDef>| rows.row::<LibCopy<FormDef>>(id).is_some();
    page.form.filter(|f| held(*f)).or_else(|| {
        let frames = rows.project_settings_frames();
        let settings = ew_derive::FrameSettingsValue::parse(frames.as_ref());
        ew_derive::frame_form_from_settings(&settings, &page.kind, true, held)
    })
}

/// The frame form a page of a transaction prints on ([`frame_form_with`]).
pub(crate) fn frame_form(tx: &TxBuilder<'_>, page: &Page) -> Option<Id<FormDef>> {
    frame_form_with(tx, page)
}

/// The frame grid of a page in a transaction, by the rule of `ew_derive::FrameGridOf`.
pub(crate) fn frame_grid(tx: &TxBuilder<'_>, page: &Page) -> Option<FrameGrid> {
    frame_grid_with(tx, page)
}

/// The frame grid of a page, by the rule of `ew_derive::FrameGridOf`.
fn frame_grid_with(rows: &impl Rows, page: &Page) -> Option<FrameGrid> {
    match frame_form_with(rows, page) {
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

/// The order online numbering gives the devices it numbers (FR-NUM-028): by the anchor cell of the main
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
/// rule as [`main_page`]: `ew_derive::MainRep`, else `ew_derive::cable_core_page`.
pub fn main_page_in(model: &Model, device: Id<Device>) -> Option<Id<Page>> {
    let q = ew_derive::Q::new(model);
    let rep = *q.get::<ew_derive::MainRep>(device);
    rep.and_then(|r| q.reps().get(r).map(|r| r.page))
        .or_else(|| ew_derive::cable_core_page(&q, device))
}

/// Whether two devices held the same page-based tag in a snapshot: both numbered, the same letter and
/// counter, and the same page of the main representation, or both cables on no page.
pub fn collided_in(model: &Model, a: Id<Device>, b: Id<Device>) -> bool {
    let devices = model.table::<Device>();
    let (Some(da), Some(db)) = (devices.get(&a), devices.get(&b)) else {
        return false;
    };
    let (pa, pb) = (main_page_in(model, a), main_page_in(model, b));
    numbered(da)
        && numbered(db)
        && da.tag.letter == db.tag.letter
        && da.tag.counter.is_some()
        && da.tag.counter == db.tag.counter
        && pa == pb
        && (pa.is_some() || (unplaced_cable(da, pa) && unplaced_cable(db, pb)))
}

/// The numbered devices in a scope with a given letter, with their counters, in ID order.
pub fn devices_in_scope(
    tx: &TxBuilder<'_>,
    letter: &Ident,
    scope: CounterScope,
) -> Vec<(Id<Device>, Option<u32>)> {
    let page = match scope {
        CounterScope::Page(page) => page,
        CounterScope::Project => {
            return tx
                .iter::<Device>()
                .into_iter()
                .filter(|dev| numbered(dev) && dev.tag.letter == *letter && dev.aspects.cable.is_some())
                .filter(|dev| main_page(tx, dev.id).is_none())
                .map(|dev| (dev.id, dev.tag.counter))
                .collect();
        }
    };
    let mut devices: BTreeSet<Id<Device>> = BTreeSet::new();
    // A cable without a definition line is placed through its first connected core or bus line ([`main_page`]).
    for cid in tx.connections_on_page(page) {
        match tx.get::<Connection>(cid).map(|c| c.kind.clone()) {
            Some(ConnectionKind::Core(k)) => devices.extend(tx.get::<Conductor>(k).map(|c| c.cable)),
            Some(ConnectionKind::Bus(line)) => devices.extend(line.cable),
            _ => {}
        }
    }
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
            if !numbered(&dev)
                || dev.tag.letter != *letter
                || main_page(tx, d) != Some(page)
                || earth_connection(tx, &dev)
            {
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
/// collision rule (FR-NUM-028, `edit.paste` no longer numbers its copies itself).
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
        if !numbered(&dev) || earth_connection(tx, &dev) {
            continue;
        }
        let page = main_page(tx, *d);
        let scope = match page {
            Some(page) => CounterScope::Page(page),
            None if unplaced_cable(&dev, page) => CounterScope::Project,
            None => continue,
        };
        let group = groups
            .entry((scope, dev.tag.letter.clone()))
            .or_insert_with(|| ScopeCounters::load(tx, &dev.tag.letter, scope));
        if renumber(tx, *d, &dev, group) {
            let next = group.next_free(Some(*d));
            // A device that keeps its counter changes nothing (a cable placed from the project's scope).
            if dev.tag.counter != Some(next) {
                tx.update::<Device>(*d, |dev| dev.tag.counter = Some(next))?;
                changed = true;
            }
            group.set_counter(*d, next);
        }
    }
    Ok(changed)
}
