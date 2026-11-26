// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device tree of assemblies and valve terminals (doc 03 §7, doc 10 §3.7, doc 11 §3.2, §4.3):
//! `assembly.attach` (`AttachChildren`), `valve.add` (`AddValve`) and `device.link` (`LinkDevices`).
//!
//! The typed `assembly`, `plc_station` and `plc_module` aspects are set with `fields.set`; adding a
//! `plc_module` instantiates the module's channels (normaliser N1, `channel_instantiation`).

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use std::sync::Arc;

use ew_schema::{
    Device, DeviceAspects, DeviceLink, DeviceLinkKind, Extra, FnDef, FnRole, Kind, LibCopy, Page, PartUse,
    Representation, StructureAssignment, TagScheme, TagSpec,
};
use ew_units::{Nm, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::device::insert_function;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, UuidS};
use crate::tx::TxBuilder;

/// One child of [`AttachChildren`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChildSpec {
    /// The child device.
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
    /// Its position inside the parent (manifold order, 1-based); absent: no position (an unordered child
    /// such as a mounting plate).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
}

/// The ancestors of `device` (its parent, the parent's parent, …), at most as many as there are devices.
fn ancestors(tx: &TxBuilder<'_>, device: Id<Device>) -> Vec<Id<Device>> {
    let mut out = Vec::new();
    let mut at = tx.get::<Device>(device).and_then(|d| d.parent);
    while let Some(p) = at {
        if out.contains(&p) {
            break; // a cycle the invariants report (EW-INV-009)
        }
        out.push(p);
        at = tx.get::<Device>(p).and_then(|d| d.parent);
    }
    out
}

/// The positions taken among the children of `parent`, except `skip`: position → child. The children come from the
/// parent index (`TxBuilder::children_of`), not a scan of the device table, so an edit of an unrelated device leaves a
/// prepared attach valid (ADR-0008).
fn taken_positions(
    tx: &TxBuilder<'_>,
    parent: Id<Device>,
    skip: &BTreeSet<Id<Device>>,
) -> BTreeMap<u32, Id<Device>> {
    tx.children_of(parent)
        .into_iter()
        .filter(|c| !skip.contains(c))
        .filter_map(|c| tx.get::<Device>(c))
        .filter_map(|d| d.position.map(|p| (p, d.id)))
        .collect()
}

/// `assembly.attach` (`AttachChildren`): makes existing devices children of a parent device (doc 03 §7:
/// modules of a valve terminal under its head device, a mounting plate under its enclosure), with their
/// positions (FR-DEV-022: child order is `Device.position`, never the drawing).
///
/// Sets `Device.parent = parent` and `Device.position` of every child (a child given without a position
/// keeps none). At least one child; children distinct and different from the parent; all exist
/// ([`CmdError::NotFound`]). Rejected: a position 0 (positions are 1-based, at `/children/i/position`); two
/// children of the command at one position; a position another child of the parent already holds (no
/// push insert in v0: move that child first); a child that is the parent's ancestor (the assembly graph is
/// acyclic, EW-INV-009). With `swap`, a held position is exchanged instead: its holder takes the old position of the
/// child that comes (refused when that child had no position on this parent). A child that has another parent moves to
/// this one. A child tag (`tag.scheme =
/// child`, not frozen) whose counter equals its old position follows the new position (N-FL-2, doc 11
/// §4.2); any other tag is not touched. A child tag is set with `fields.set`, a valve added with `valve.add`.
/// The generated assembly overviews, slots, break marks and the PLC overview follow from the tree (they are
/// derived). A valve terminal drawn as its positions (`BoxSpec.slots`) has the symbols of its valves on that page
/// moved into their cells, their tubes following. Outcome: the parent.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachChildren {
    /// The parent device.
    #[schemars(with = "UuidS")]
    pub parent: Id<Device>,
    /// The children, at least one.
    pub children: Vec<ChildSpec>,
    /// A requested position another child of the parent holds is swapped: that child takes the old position of
    /// the child that comes, which must be a child of the parent with a position; default false (refused).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub swap: bool,
}

impl Command for AttachChildren {
    const NAME: &'static str = "assembly.attach";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.children.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/children",
                text!("refusal-params-no-child"),
            ));
        }
        tx.require::<Device>(self.parent)?;
        let mut seen = BTreeSet::new();
        let mut positions: BTreeMap<u32, Id<Device>> = BTreeMap::new();
        for (i, c) in self.children.iter().enumerate() {
            if !seen.insert(c.device) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/children/{i}/device"),
                    text!("refusal-params-child-twice"),
                ));
            }
            if c.device == self.parent {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/children/{i}/device"),
                    text!("refusal-assembly-own-child"),
                ));
            }
            if c.position == Some(0) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/children/{i}/position"),
                    text!("refusal-params-positions-one-based"),
                ));
            }
            tx.require::<Device>(c.device)?;
            if let Some(p) = c.position {
                if let Some(other) = positions.insert(p, c.device) {
                    return tx.reject(text!(
                        "refusal-assembly-position-twice",
                        p.to_string(),
                        crate::words::device(tx, other),
                        crate::words::device(tx, c.device)
                    ));
                }
            }
        }
        if let Some(c) = ancestors(tx, self.parent).into_iter().find(|a| seen.contains(a)) {
            return tx.reject(text!(
                "refusal-assembly-cycle",
                crate::words::device(tx, c),
                crate::words::device(tx, self.parent)
            ));
        }
        let taken = taken_positions(tx, self.parent, &seen);
        let mut swaps: Vec<(Id<Device>, Option<u32>)> = Vec::new();
        for (p, child) in &positions {
            if let Some(holder) = taken.get(p) {
                let old = tx
                    .get::<Device>(*child)
                    .filter(|d| d.parent == Some(self.parent))
                    .and_then(|d| d.position);
                if self.swap && old.is_some() {
                    swaps.push((*holder, old));
                    continue;
                }
                return tx.reject(text!(
                    "refusal-assembly-position-held",
                    p.to_string(),
                    crate::words::device(tx, self.parent),
                    crate::words::device(tx, *holder),
                    crate::words::device(tx, *child)
                ));
            }
        }
        let moves: Vec<(Id<Device>, Option<u32>)> = self
            .children
            .iter()
            .map(|c| (c.device, c.position))
            .chain(swaps)
            .collect();
        for (device, position) in moves {
            let parent = self.parent;
            tx.update::<Device>(device, |d| {
                // N-FL-2 (doc 11 §3.2, §4.2): a child tag keeps its counter equal to its position; a frozen
                // tag or a counter that differed from the old position is the user's and stays
                let follows = d.tag.scheme == TagScheme::Child
                    && d.tag.frozen.is_none()
                    && d.tag.counter.is_some()
                    && d.tag.counter == d.position;
                if follows && position.is_some() {
                    d.tag.counter = position;
                }
                d.parent = Some(parent);
                d.position = position;
            })?;
        }
        seat_reps(tx, self.parent)?;
        Ok(Outcome::primary((Kind::Device, self.parent.cast())))
    }
}

/// Whether `device` has a function on a FnDef of contact kind `valve` (as the transaction holds it).
fn is_valve_device(tx: &TxBuilder<'_>, device: Id<Device>) -> bool {
    tx.functions_of_device(device).into_iter().any(|f| {
        tx.get::<ew_schema::Function>(f)
            .and_then(|f| tx.get::<LibCopy<FnDef>>(f.def))
            .is_some_and(|d| d.item.contact_kind == ew_schema::ContactKind::Valve)
    })
}

/// Whether `device` uses a part whose fluid section taps a manifold gallery (a valve terminal's supply plate).
fn is_supply_plate(tx: &TxBuilder<'_>, device: &Device) -> bool {
    device.parts.iter().any(|u| {
        tx.get::<LibCopy<ew_schema::Part>>(u.part.copy).is_some_and(|p| {
            p.item
                .sections
                .fluid
                .as_ref()
                .is_some_and(|f| f.galleries.iter().any(|g| g.tap.is_some()))
        })
    })
}

/// Whether `device` may sit on a valve terminal's position, as the transaction holds it ([`ew_derive::fits_terminal`]:
/// a valve without a coil of its own to wire).
pub(crate) fn fits_terminal(tx: &TxBuilder<'_>, device: Id<Device>) -> bool {
    let defs: Vec<_> = tx
        .functions_of_device(device)
        .into_iter()
        .filter_map(|f| tx.get::<ew_schema::Function>(f))
        .filter_map(|f| tx.get::<LibCopy<FnDef>>(f.def))
        .collect();
    ew_derive::fits_terminal(defs.iter().map(|d| &d.item))
}

/// Whether `device` is a valve terminal: a PLC station of role `valve_terminal`, or a multipin terminal (no station, a
/// part that names its plug's pins; `ew_derive::is_multipin_terminal`).
fn is_terminal(tx: &TxBuilder<'_>, device: Id<Device>) -> bool {
    tx.get::<Device>(device)
        .is_some_and(|t| match &t.aspects.plc_station {
            Some(s) => s.role == ew_schema::StationRole::ValveTerminal,
            None => t.parts.iter().any(|u| {
                tx.get::<LibCopy<ew_schema::Part>>(u.part.copy)
                    .is_some_and(|c| c.item.sections.fluid.as_ref().is_some_and(|f| !f.pins.is_empty()))
            }),
        })
}

/// One cell of a valve terminal as the transaction holds it: its module, its position and the child that holds it.
type CellIn = (Id<Device>, u32, Option<Id<Device>>);

/// The cells of valve terminal `terminal` as the transaction holds them (`ew_derive::terminal_cells`): per cell, left
/// to right, its module, its position and the child that holds it (a valve or a blanking plate; of two, the lower ID).
fn cells_in(tx: &TxBuilder<'_>, terminal: Id<Device>) -> Vec<CellIn> {
    let mut modules: Vec<(u32, Id<Device>)> = tx
        .children_of(terminal)
        .into_iter()
        .filter_map(|c| tx.get::<Device>(c).map(|d| (d.position.unwrap_or(u32::MAX), c)))
        .collect();
    modules.sort();
    let mut out = Vec::new();
    for m in std::iter::once(terminal).chain(modules.into_iter().map(|(_, m)| m)) {
        let Some(count) = tx
            .get::<Device>(m)
            .and_then(|d| crate::commands::fluid::position_count(tx, &d))
        else {
            continue;
        };
        let mut children = tx.children_of(m);
        children.sort();
        for p in 1..=u32::from(count) {
            let holder = children.iter().copied().find(|c| {
                tx.get::<Device>(*c).is_some_and(|d| {
                    d.position == Some(p)
                        && (d.kind == Some(ew_schema::DeviceKind::BlankingPlate) || is_valve_device(tx, *c))
                })
            });
            out.push((m, p, holder));
        }
    }
    out
}

/// The slotted boxes of valve terminal `terminal` (`BoxSpec.slots`): each representation with its frame on its page,
/// each cell as wide as the valve drawn in it on that page needs (`ew_derive::box_frame`, as the transaction holds
/// it).
fn slotted_boxes(
    tx: &TxBuilder<'_>,
    terminal: Id<Device>,
) -> Vec<(Arc<Representation>, ew_derive::SlotFrame)> {
    let mut reps: Vec<Id<Representation>> = tx
        .functions_of_device(terminal)
        .into_iter()
        .flat_map(|f| tx.reps_of_function(f))
        .collect();
    reps.sort();
    reps.dedup();
    let cells = cells_in(tx, terminal);
    reps.into_iter()
        .filter_map(|r| tx.get::<Representation>(r))
        .filter_map(|r| {
            let slots = r.box_.as_ref()?.slots.as_ref()?;
            let grid = tx.get::<Page>(r.page)?.grid;
            let extents: Vec<Option<(Nm, Nm)>> = cells
                .iter()
                .map(|(_, _, holder)| {
                    let valve = holder.filter(|h| is_valve_device(tx, *h))?;
                    reps_on(tx, valve, r.page, true)
                        .iter()
                        .filter_map(|v| {
                            let s = tx.get::<LibCopy<ew_schema::SymbolDef>>(v.symbol)?;
                            ew_derive::graphics_x_extent(&s.item)
                        })
                        .max_by_key(|(min, max)| *max - *min)
                })
                .collect();
            let frame = ew_derive::SlotFrame::new(grid, r.pos, slots.pitch).with_cells(&extents);
            Some((r, frame))
        })
        .collect()
}

/// The terminal a position of `parent` belongs to: `parent` itself when it is a valve terminal, else its parent when
/// that is one.
fn terminal_of(tx: &TxBuilder<'_>, parent: Id<Device>) -> Option<Id<Device>> {
    if is_terminal(tx, parent) {
        return Some(parent);
    }
    tx.get::<Device>(parent)?.parent.filter(|p| is_terminal(tx, *p))
}

/// The cell of a slotted box of `terminal` that representation `rep` stands in (the middle of its symbol's graphics,
/// at the height of its insertion point): (module, position); `None` when it stands in no cell.
fn cell_under(
    tx: &TxBuilder<'_>,
    terminal: Id<Device>,
    rep: Id<Representation>,
) -> Option<(Id<Device>, u32)> {
    let r = tx.get::<Representation>(rep)?;
    let (min, max) = tx
        .get::<LibCopy<ew_schema::SymbolDef>>(r.symbol)
        .and_then(|s| ew_derive::graphics_x_extent(&s.item))
        .unwrap_or((Nm(0), Nm(0)));
    let mid = i64::try_from((i128::from(min.0) + i128::from(max.0)) / 2).ok()?;
    let at = Pt2::new(Nm(r.pos.x.0.checked_add(mid)?), r.pos.y);
    let cells = cells_in(tx, terminal);
    slotted_boxes(tx, terminal)
        .into_iter()
        .filter(|(b, _)| b.page == r.page)
        .find_map(|(_, frame)| frame.cell_at(cells.len(), at))
        .map(|i| (cells[i].0, cells[i].1))
}

/// The representations of `device` on `page` that are no box, in ID order: of its valve functions only with
/// `valves_only`.
fn reps_on(
    tx: &TxBuilder<'_>,
    device: Id<Device>,
    page: Id<Page>,
    valves_only: bool,
) -> Vec<Arc<Representation>> {
    let mut reps: Vec<Arc<Representation>> = tx
        .functions_of_device(device)
        .into_iter()
        .filter(|f| {
            !valves_only
                || tx
                    .get::<ew_schema::Function>(*f)
                    .and_then(|f| tx.get::<LibCopy<FnDef>>(f.def))
                    .is_some_and(|d| d.item.contact_kind == ew_schema::ContactKind::Valve)
        })
        .flat_map(|f| tx.reps_of_function(f))
        .filter_map(|r| tx.get::<Representation>(r))
        .filter(|r| r.page == page && r.box_.is_none())
        .collect();
    reps.sort_by_key(|r| r.id);
    reps.dedup_by_key(|r| r.id);
    reps
}

/// Whether function `f` is on a valve terminal's supply FnDef ([`ew_derive::is_supply_def`]).
fn is_supply_function(tx: &TxBuilder<'_>, f: Id<ew_schema::Function>) -> bool {
    tx.get::<ew_schema::Function>(f)
        .and_then(|f| tx.get::<LibCopy<FnDef>>(f.def))
        .is_some_and(|d| ew_derive::is_supply_def(&d.item))
}

/// The supply of valve terminal `terminal` as the transaction holds it (`ew_derive::terminal_supply`): the terminal's
/// own lowest-ID function on a supply FnDef, else of its children without a position in ID order, of the first supply
/// plate (a child whose part taps a gallery) its lowest-ID function. `None` when it has neither.
fn supply_of(tx: &TxBuilder<'_>, terminal: Id<Device>) -> Option<Id<ew_schema::Function>> {
    let mut children: Vec<Id<Device>> = tx
        .children_of(terminal)
        .into_iter()
        .filter(|c| tx.get::<Device>(*c).is_some_and(|d| d.position.is_none()))
        .collect();
    children.sort();
    std::iter::once(terminal).chain(children).find_map(|d| {
        let mut fns = tx.functions_of_device(d);
        fns.sort();
        let plate = d != terminal && tx.get::<Device>(d).is_some_and(|d| is_supply_plate(tx, &d));
        fns.iter()
            .copied()
            .find(|f| d == terminal && is_supply_function(tx, *f))
            .or_else(|| fns.first().copied().filter(|_| plate))
    })
}

/// The symbol that draws a function on FnDef copy `def` in a valve terminal's box on `page`: the lowest-ID
/// embedded fluid symbol on the page's grid that draws `def`; `None` when the project embeds none (the window embeds
/// it with the command that fills a position, `Session::cell_copies`).
fn cell_symbol(tx: &TxBuilder<'_>, def: Id<FnDef>, page: Id<Page>) -> Option<Id<ew_schema::SymbolDef>> {
    let grid = tx.get::<Page>(page)?.grid;
    tx.iter::<LibCopy<ew_schema::SymbolDef>>()
        .into_iter()
        .find(|s| {
            s.item.rep_type == ew_schema::RepType::Fluid
                && s.item.grid == grid
                && ew_ids::v5::lib_copy_id::<FnDef>(
                    s.item.fndef.library.as_uuid(),
                    s.item.fndef.item.as_uuid(),
                ) == def
        })
        .map(|s| s.id)
}

/// A new upright representation of `function` with symbol copy `symbol` at `pos` on `page` (a valve in its cell,
/// the terminal's supply in its supply end), with the symbol's arrangement `arrangement` when it has one of that name;
/// nothing when a point of it would leave the page's range.
fn draw_in_box(
    tx: &mut TxBuilder<'_>,
    function: Id<ew_schema::Function>,
    symbol: Id<ew_schema::SymbolDef>,
    (page, pos): (Id<Page>, Pt2),
    arrangement: Option<&str>,
) -> Result<(), CmdError> {
    let arrangement = arrangement.and_then(|a| {
        tx.get::<LibCopy<ew_schema::SymbolDef>>(symbol)?
            .item
            .arrangements
            .iter()
            .find(|x| x.name.as_str() == a)
            .map(|x| x.name.clone())
    });
    let record = Representation {
        id: tx.mint(),
        v: 1,
        functions: vec![function],
        page,
        symbol,
        variant: 0,
        pos,
        orient: Default::default(),
        rep_type: ew_schema::RepType::Fluid,
        pin_map: Default::default(),
        text_slots: Default::default(),
        arrangement,
        anchor: None,
        box_: None,
        source_ref: None,
        locked: false,
        extra: Extra::new(),
    };
    if crate::geometry::rep_cps_checked(tx, &record).is_err() {
        return Ok(());
    }
    tx.insert(record).map(|_| ())
}

/// The function of `device` that a valve in a cell is drawn by: its lowest-ID valve function.
fn valve_function(tx: &TxBuilder<'_>, device: Id<Device>) -> Option<Id<ew_schema::Function>> {
    let mut fns: Vec<_> = tx
        .functions_of_device(device)
        .into_iter()
        .filter(|f| {
            tx.get::<ew_schema::Function>(*f)
                .and_then(|f| tx.get::<LibCopy<FnDef>>(f.def))
                .is_some_and(|d| d.item.contact_kind == ew_schema::ContactKind::Valve)
        })
        .collect();
    fns.sort();
    fns.first().copied()
}

/// Whether function `f` is drawn on any page.
fn drawn(tx: &TxBuilder<'_>, f: Id<ew_schema::Function>) -> bool {
    !tx.reps_of_function(f).is_empty()
}

/// The symbols of a valve terminal's valves and its supply on the pages where the terminal is drawn as its positions:
/// the valves' functions made first, as normaliser N-FL-1 makes them, so that a valve that has just taken
/// a position is drawn in the same step. On each such page every representation of a valve function of the valve
/// holding cell `i` moves to the cell's valve point, and the first representation of the supply to the supply end
/// ([`supply_of`]: a supply plate's, or the terminal's own); their tubes follow as `rep.move` makes them follow. On the
/// first such page (the lowest box ID), a valve drawn on no page gets its symbol in its cell, and the supply drawn on no
/// page gets its symbol in the supply end; then each box takes the cell widths its valves need there
/// (`ew_derive::SlotFrame::with_cells`: a valve wider than a cell widens its own), its outline growing to the right; a terminal without a supply gets its own (a function on the embedded supply
/// FnDef, `ew_derive::is_supply_def`), so that its ports 1, 3 and 5 take a tube there. A symbol is drawn only when the
/// project embeds one for the page ([`cell_symbol`]). A valve drawn on another page stays there. Nothing for a parent of
/// no valve terminal or a terminal not drawn as its positions.
pub(crate) fn seat_reps(tx: &mut TxBuilder<'_>, parent: Id<Device>) -> Result<(), CmdError> {
    let Some(terminal) = terminal_of(tx, parent) else {
        return Ok(());
    };
    let boxes = slotted_boxes(tx, terminal);
    if boxes.is_empty() {
        return Ok(());
    }
    crate::normalise::Normaliser::run(&crate::normalise::ValveFunctions, tx)?;
    let cells = cells_in(tx, terminal);
    if let Some((first, frame)) = boxes.first() {
        for (i, (_, _, holder)) in cells.iter().enumerate() {
            let Some(f) = holder
                .filter(|h| is_valve_device(tx, *h))
                .and_then(|v| valve_function(tx, v))
                .filter(|f| !drawn(tx, *f))
            else {
                continue;
            };
            let Some(def) = tx.get::<ew_schema::Function>(f).map(|f| f.def) else {
                continue;
            };
            if let Some(symbol) = cell_symbol(tx, def, first.page) {
                let extent = tx
                    .get::<LibCopy<ew_schema::SymbolDef>>(symbol)
                    .and_then(|s| ew_derive::graphics_x_extent(&s.item));
                draw_in_box(tx, f, symbol, (first.page, frame.valve_at(i, extent)), None)?;
            }
        }
        let supply = match supply_of(tx, terminal) {
            Some(f) => Some(f),
            None => {
                let def = tx
                    .iter::<LibCopy<FnDef>>()
                    .into_iter()
                    .find(|d| ew_derive::is_supply_def(&d.item))
                    .map(|d| d.id)
                    .filter(|d| cell_symbol(tx, *d, first.page).is_some());
                match def {
                    Some(def) => Some(insert_function(tx, Some(terminal), def, FnRole::Aux)?),
                    None => None,
                }
            }
        };
        if let Some(f) = supply.filter(|f| !drawn(tx, *f))
            && let Some(def) = tx.get::<ew_schema::Function>(f).map(|f| f.def)
            && let Some(symbol) = cell_symbol(tx, def, first.page)
        {
            // The terminal's own supply prints its ports, not the terminal's tag a second time.
            let own = tx.get::<ew_schema::Function>(f).and_then(|f| f.device) == Some(terminal);
            draw_in_box(
                tx,
                f,
                symbol,
                (first.page, frame.supply_at()),
                own.then_some("ports"),
            )?;
        }
    }
    let supply = supply_of(tx, terminal);
    // With the valves drawn, each cell is as wide as its valve needs (a 5/3 valve): the box keeps its corner and
    // grows or shrinks to the right, and the valves of the cells to its right move with their tubes.
    for (b, frame) in slotted_boxes(tx, terminal) {
        let outline = frame.outline(cells.len());
        if b.box_.as_ref().is_some_and(|x| x.outline != outline) {
            tx.update::<Representation>(b.id, |r| {
                if let Some(x) = r.box_.as_mut() {
                    x.outline = outline;
                }
            })?;
        }
        let mut targets: Vec<(Id<Representation>, Pt2)> = Vec::new();
        for (i, (_, _, holder)) in cells.iter().enumerate() {
            let Some(valve) = holder.filter(|h| is_valve_device(tx, *h)) else {
                continue;
            };
            for r in reps_on(tx, valve, b.page, true) {
                let extent = tx
                    .get::<LibCopy<ew_schema::SymbolDef>>(r.symbol)
                    .and_then(|s| ew_derive::graphics_x_extent(&s.item));
                targets.push((r.id, frame.valve_at(i, extent)));
            }
        }
        let mut reps: Vec<Id<Representation>> = supply.map(|f| tx.reps_of_function(f)).unwrap_or_default();
        reps.sort();
        if let Some(r) = reps
            .into_iter()
            .filter_map(|r| tx.get::<Representation>(r))
            .find(|r| r.page == b.page && r.box_.is_none())
        {
            targets.push((r.id, frame.supply_at()));
        }
        for (rep, to) in targets {
            let Some(r) = tx.get::<Representation>(rep) else {
                continue;
            };
            if r.pos == to {
                continue;
            }
            let Ok(delta) = crate::geometry::sub(to, r.pos) else {
                continue;
            };
            crate::commands::device::MoveReps {
                reps: vec![rep],
                delta,
                page: None,
                steps: None,
                items: Vec::new(),
                routes: Vec::new(),
                connect: Vec::new(),
                into: None,
                keep: None,
            }
            .run_move(tx)?;
        }
    }
    Ok(())
}

/// The `into` of `device.place` and `rep.move` (a valve placed inside a valve terminal's box would stay
/// a loose device): `device`, drawn by representation `rep` inside the box of the valve terminal `terminal`, joins it as
/// `assembly.attach` makes it (with its checks), in the same transaction. A valve that fits a terminal
/// ([`fits_terminal`]) takes the position of the cell `rep` stands in when the terminal is drawn as its positions (a
/// valve of the terminal at another position swaps with the cell's holder; with `replace` the holder is removed with its
/// delete plan; any other valve onto a held cell is refused in words), else keeps its position when it is the terminal's already, else takes the terminal's first free valve
/// position (1 … n, n its `fluid.positions` or `plc.positions.count`); a supply plate (a part that taps a gallery) joins
/// without a position. The terminal itself, any other device (a stand-alone valve with a coil of its own, a coil, a
/// cylinder), and a valve when no position is free, are left as they are: the window says so. Refused: a `terminal`
/// that is no valve terminal.
pub(crate) fn seat_on_terminal(
    tx: &mut TxBuilder<'_>,
    device: Id<Device>,
    terminal: Id<Device>,
    rep: Option<Id<Representation>>,
    replace: bool,
) -> Result<(), CmdError> {
    tx.require::<Device>(terminal)?;
    if !is_terminal(tx, terminal) {
        return tx.reject(text!(
            "refusal-seat-not-terminal",
            crate::words::device(tx, terminal)
        ));
    }
    let d = tx.require::<Device>(device)?;
    if device == terminal {
        return Ok(());
    }
    if !fits_terminal(tx, device) {
        if is_supply_plate(tx, &d) && d.parent != Some(terminal) {
            AttachChildren {
                parent: terminal,
                children: vec![ChildSpec {
                    device,
                    position: None,
                }],
                swap: false,
            }
            .run(tx)?;
        } else if d.parent == Some(terminal) {
            seat_reps(tx, terminal)?;
        }
        return Ok(());
    }
    let mine = d.parent.filter(|p| terminal_of(tx, *p) == Some(terminal));
    if let Some((module, position)) = rep.and_then(|r| cell_under(tx, terminal, r)) {
        let holder = cells_in(tx, terminal)
            .into_iter()
            .find(|(m, p, _)| *m == module && *p == position)
            .and_then(|(_, _, h)| h)
            .filter(|h| *h != device);
        if let Some(holder) = holder.filter(|_| replace) {
            let h = tx.require::<Device>(holder)?;
            crate::commands::fluid::remove_child(tx, &h, position, module)?;
        } else if let Some(holder) = holder.filter(|_| mine != Some(module)) {
            return tx.reject(text!(
                "refusal-seat-cell-held",
                position.to_string(),
                crate::words::device(tx, module),
                crate::words::device(tx, holder)
            ));
        }
        return AttachChildren {
            parent: module,
            children: vec![ChildSpec {
                device,
                position: Some(position),
            }],
            swap: true,
        }
        .run(tx)
        .map(|_| ());
    }
    if mine.is_some() {
        return seat_reps(tx, terminal);
    }
    let t = tx.require::<Device>(terminal)?;
    let count = crate::commands::fluid::position_count(tx, &t).unwrap_or(0);
    let taken = taken_positions(tx, terminal, &BTreeSet::from([device]));
    let Some(position) = (1..=u32::from(count)).find(|p| !taken.contains_key(p)) else {
        return Ok(());
    };
    AttachChildren {
        parent: terminal,
        children: vec![ChildSpec {
            device,
            position: Some(position),
        }],
        swap: false,
    }
    .run(tx)
    .map(|_| ())
}

/// `valve_terminal.draw_slots` (`DrawSlots`, doc 11 §3.2): redraws the free box of a valve terminal as its valve
/// positions, which a page drawn before the cells existed shows as one box (a page that exists is never redrawn by itself).
///
/// The box representation `rep` keeps its ID, page and insertion point (its top-left corner) and becomes a box with one
/// cell per position (`BoxSpec.slots`, [`ew_derive::SlotFrame`]: the supply end, then the cells of the terminal's
/// positions and of its modules' in order, each [`ew_derive::CELL_STEPS`] grid steps wide or as wide as its valve
/// needs), turned upright, without box
/// CPs or inner graphics; the symbols of the terminal's valves on that page move into their cells and its supply plate
/// into the supply end, their tubes following, and its valves and supply drawn on no page are drawn there
/// ([`seat_reps`]; `copies` embeds the symbols first, as `part.assign` embeds). Refused, in words: a representation that
/// draws no valve terminal, one that is no box or is drawn as its positions already, a terminal without valve positions,
/// and a box CP with a connection. Previewed, one undo step. Outcome: the representation.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawSlots {
    /// The terminal's box.
    #[schemars(with = "UuidS")]
    pub rep: Id<Representation>,
    /// The library documents to embed (`lib/` JSON form): the symbols of the valves and the supply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<serde_json::Value>,
}

impl Command for DrawSlots {
    const NAME: &'static str = "valve_terminal.draw_slots";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        crate::commands::assign::embed_closure(Self::NAME, tx, &self.copies)?;
        let rep = tx.require::<Representation>(self.rep)?;
        let device = rep
            .functions
            .first()
            .and_then(|f| tx.get::<ew_schema::Function>(*f))
            .and_then(|f| f.device)
            .filter(|d| is_terminal(tx, *d));
        let Some(device) = device else {
            return tx.reject(text!(
                "refusal-draw-slots-not-terminal",
                crate::words::symbol(tx, self.rep)
            ));
        };
        let name = crate::words::device(tx, device);
        let Some(old) = rep.box_.clone() else {
            return tx.reject(text!("refusal-draw-slots-no-box", name));
        };
        if old.slots.is_some() {
            return tx.reject(text!("refusal-draw-slots-already", name));
        }
        let n = cells_in(tx, device).len();
        if n == 0 {
            return tx.reject(text!("refusal-draw-slots-no-positions", name));
        }
        let connected = rep.functions.iter().any(|f| {
            old.cps
                .iter()
                .any(|c| !tx.connections_at(ew_model::EndpointKey::Cp(*f, c.idx)).is_empty())
        });
        if connected {
            return tx.reject(text!("refusal-draw-slots-connected", name));
        }
        let grid = tx.require::<Page>(rep.page)?.grid;
        let pitch = ew_derive::SlotFrame::pitch(grid);
        let frame = ew_derive::SlotFrame::new(grid, rep.pos, pitch);
        let mut next = (*rep).clone();
        next.pos = frame.origin();
        next.orient = Default::default();
        next.box_ = Some(ew_schema::BoxSpec {
            outline: frame.outline(n),
            cps: Vec::new(),
            inner: Vec::new(),
            slots: Some(ew_schema::BoxSlots {
                pitch,
                extra: Extra::new(),
            }),
            extra: old.extra.clone(),
        });
        tx.put(next);
        seat_reps(tx, device)?;
        Ok(Outcome::primary((Kind::Representation, self.rep.cast())))
    }
}

/// `valve_terminal.draw_valves` (`DrawValves`, doc 11 §3.2): Draw its valves in the terminal. A valve terminal
/// drawn as its positions whose valves were put on their positions without a symbol (from the rack, the PLC list, a
/// project made before the cells drew their valves) has them drawn in their cells, and its supply in its supply end ([`seat_reps`]); `copies`
/// embeds their symbols first, as `part.assign` embeds. A valve drawn on another page stays there. Refused, in words: a
/// device that is no valve terminal, and a terminal drawn as its positions on no page. One undo step. Outcome: the
/// terminal.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawValves {
    /// The valve terminal.
    #[schemars(with = "UuidS")]
    pub terminal: Id<Device>,
    /// The library documents to embed (`lib/` JSON form): the symbols of the valves and the supply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<serde_json::Value>,
}

impl Command for DrawValves {
    const NAME: &'static str = "valve_terminal.draw_valves";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        crate::commands::assign::embed_closure(Self::NAME, tx, &self.copies)?;
        tx.require::<Device>(self.terminal)?;
        if !is_terminal(tx, self.terminal) {
            return tx.reject(text!(
                "refusal-seat-not-terminal",
                crate::words::device(tx, self.terminal)
            ));
        }
        if slotted_boxes(tx, self.terminal).is_empty() {
            return tx.reject(text!(
                "refusal-draw-valves-not-drawn",
                crate::words::device(tx, self.terminal)
            ));
        }
        seat_reps(tx, self.terminal)?;
        Ok(Outcome::primary((Kind::Device, self.terminal.cast())))
    }
}

/// `valve.add` (`AddValve`, doc 11 §4.3 `valve.add{module, position?, part}`): adds a valve child to a
/// module (sub-base) of a valve terminal.
///
/// Creates `Device {tag: {letter, counter: position, scheme: child}, parent: module, position, structure,
/// parts: [part]}` (doc 11 §3.2: the child rule prints `{parent}-{letter}{position}`; normaliser N-FL-2
/// keeps the counter equal to the position) and, with `fndef`, its main function `{def: fndef, role: main,
/// kind: normal}`. A part whose template has an entry on a FnDef of contact kind `valve` ignores `fndef`: normaliser
/// N-FL-1 (`valve_functions`) makes the valve and coil functions from the part. The position is the
/// requested one, else the lowest free position of the module (1, 2, …). The module, the part copy and the FnDef
/// copy must exist. Rejected: position 0 (at `/position`); a requested position another child of the module holds
/// (no push insert in v0); a part with a PLC section (a station or module part, which normaliser N0 would attach to a
/// station instead). The slot-count bound of the module's part is EW-FL-013's prevent rule. The
/// coil-to-channel binding follows from the module part's solenoid-position
/// map through normaliser N2 (`solenoid_binding`) when the module has channels; `plc.bind_channel` binds
/// otherwise. The state (spare valves) and the function text are set with `fields.set`.
/// Outcome: the valve.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddValve {
    /// The module (sub-base) the valve sits on.
    #[schemars(with = "UuidS")]
    pub module: Id<Device>,
    /// The requested position (1-based); default the lowest free one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
    /// The valve's part use.
    pub part: PartUse,
    /// The letter code; default `Q` (directional valves in the shipped EN 81346-2:2009 set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub letter: Option<Ident>,
    /// The FnDef copy of the valve's main function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub fndef: Option<Id<FnDef>>,
    /// Own structure assignment; default none (inherited from the parent).
    #[serde(default, skip_serializing_if = "StructureAssignment::is_empty")]
    pub structure: StructureAssignment,
}

/// Refuses a PLC station or module part as a valve: normaliser N0 would attach it to a station.
pub(crate) fn refuse_plc_part(tx: &TxBuilder<'_>, part: &LibCopy<ew_schema::Part>) -> Result<(), CmdError> {
    if part.item.sections.plc.is_some() {
        return tx.reject(text!(
            "refusal-valve-add-plc-part",
            part.item.part_number.as_str()
        ));
    }
    Ok(())
}

impl Command for AddValve {
    const NAME: &'static str = "valve.add";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.position == Some(0) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/position",
                text!("refusal-params-positions-one-based"),
            ));
        }
        tx.require::<Device>(self.module)?;
        let part = tx.require::<LibCopy<ew_schema::Part>>(self.part.part.copy)?;
        refuse_plc_part(tx, &part)?;
        if let Some(fndef) = self.fndef {
            tx.require::<LibCopy<FnDef>>(fndef)?;
        }
        // A part with a valve entry gives the functions through N-FL-1
        let from_part = part.item.function_template.iter().any(|e| {
            let def: Id<FnDef> = ew_ids::v5::lib_copy_id(e.fndef.library.as_uuid(), e.fndef.item.as_uuid());
            tx.get::<LibCopy<FnDef>>(def)
                .is_some_and(|d| d.item.contact_kind == ew_schema::ContactKind::Valve)
        });
        let taken = taken_positions(tx, self.module, &BTreeSet::new());
        let position = match self.position {
            Some(p) => {
                if let Some(holder) = taken.get(&p) {
                    // The module and the holder in words.
                    return tx.reject(text!(
                        "refusal-valve-position-held",
                        p.to_string(),
                        crate::words::device(tx, self.module),
                        crate::words::device(tx, *holder)
                    ));
                }
                p
            }
            None => (1..).find(|p| !taken.contains_key(p)).unwrap_or(u32::MAX),
        };
        let letter = self
            .letter
            .clone()
            .unwrap_or_else(|| "Q".parse().expect("`Q` is an identifier"));
        let id: Id<Device> = tx.mint();
        tx.insert(Device {
            id,
            v: 1,
            tag: TagSpec {
                scheme: TagScheme::Child,
                ..TagSpec::new(letter, Some(position))
            },
            kind: None,
            parent: Some(self.module),
            position: Some(position),
            structure: self.structure.clone(),
            parts: vec![self.part.clone()],
            accessory_choices: Vec::new(),
            function_text: MlText::default(),
            process_tag: None,
            tech_data: Default::default(),
            state: Default::default(),
            source_ref: None,
            aspects: DeviceAspects::default(),
            extra: Extra::new(),
        })?;
        if let Some(def) = self.fndef.filter(|_| !from_part) {
            insert_function(tx, Some(id), def, FnRole::Main)?;
        }
        // On a terminal drawn as its positions the valve stands in its cell.
        seat_reps(tx, self.module)?;
        Ok(Outcome::primary((Kind::Device, id.cast())))
    }
}

/// `device.link` (`LinkDevices`, doc 11 §4.3 `device.link{from, to, Drives}`): a functional link between
/// two devices, such as a valve that drives a field device (doc 11 §3.3: the PLC overview and the I/O
/// list print the driven device's process tag).
///
/// Creates `DeviceLink {from, to, kind}`. Both devices exist ([`CmdError::NotFound`]); rejected: `from =
/// to` (at `/to`), and a link of the same kind between the same devices that exists already. Deleting
/// either device deletes the link (cascade). Outcome: the link.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkDevices {
    /// The driving or linked device.
    #[schemars(with = "UuidS")]
    pub from: Id<Device>,
    /// The driven or linked device.
    #[schemars(with = "UuidS")]
    pub to: Id<Device>,
    /// The relation.
    pub kind: DeviceLinkKind,
}

impl Command for LinkDevices {
    const NAME: &'static str = "device.link";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.from == self.to {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/to",
                text!("refusal-device-link-itself"),
            ));
        }
        tx.require::<Device>(self.from)?;
        tx.require::<Device>(self.to)?;
        // A valve does not drive a device of its own parent chain.
        if self.kind == DeviceLinkKind::Drives {
            let mut at = self.from;
            while let Some(p) = tx.get::<Device>(at).and_then(|d| d.parent) {
                if p == self.to {
                    return tx.reject(text!(
                        "refusal-device-link-holder",
                        crate::words::device(tx, self.from),
                        crate::words::device(tx, self.to)
                    ));
                }
                if p == self.from {
                    break;
                }
                at = p;
            }
        }
        if tx
            .iter::<DeviceLink>()
            .into_iter()
            .any(|l| l.from == self.from && l.to == self.to && l.kind == self.kind)
        {
            return tx.reject(text!(
                "refusal-device-link-twice",
                crate::words::device(tx, self.from),
                crate::words::device(tx, self.to)
            ));
        }
        let id: Id<DeviceLink> = tx.mint();
        tx.insert(DeviceLink {
            id,
            v: 1,
            from: self.from,
            to: self.to,
            kind: self.kind.clone(),
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::DeviceLink, id.cast())))
    }
}
