// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A symbol copy's shape (its outline flattened, its CPs with their directions and designations), the CP map of
//! a symbol exchange, and the copies a valve terminal's cells and the undrawn bus ports need.
//!
//! Not here: the records drawn on a page (`tables.rs`); the place catalogue (`catalogue.rs`).

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_derive::{FunctionCps, Q};
use ew_dl::{ItemKind, PathEl};
use ew_ids::{Id, Uuid};
use ew_library::starter;
use ew_schema::{ContactKind, Device, Dir4, FnDef, Function, LibCopy, LibDocument, SymbolDef};
use ew_units::{Orient2, Pt2, Rect};

use super::OUTLINE_TOLERANCE_NM;
use super::catalogue::{
    CatalogueInterface, box_symbol, drawn_page, fndef_of, key_or_id, part_interfaces, part_valve_def,
    rep_type_word, starter_docs, starter_fndef, starter_symbol,
};
use super::embeds::starter_id;
use crate::Session;
use crate::geometry::Dir;
use crate::output::join::fndef_copy;

/// A symbol copy's shape at orientation A (R0), relative to its insertion point, in page nm.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolShape {
    /// The symbol copy.
    pub copy: Uuid,
    /// Its key; the copy's UUID text for a copy without a key.
    pub key: String,
    /// Every symbol CP with its FnDef CP and direction, in symbol-CP order.
    pub cps: Vec<ShapeCp>,
    /// The bounds of the graphics without texts, `[x0, y0, x1, y1]`; the origin point without graphics.
    pub body: [i64; 4],
    /// Every stroked or filled path of the graphics (not texts) flattened to polylines with a tolerance of
    /// [`OUTLINE_TOLERANCE_NM`], in paint order; a closed subpath ends at its first point.
    pub outline: Vec<Vec<(i64, i64)>>,
    /// The representation type in its JSON spelling.
    pub rep_type: String,
    /// The FnDef copy the symbol draws (`v5(NS_LIBCOPY, library ‖ item)` of its `fndef` reference).
    pub fndef: Uuid,
    /// Whether that FnDef is a valve (contact kind `valve`): placed inside a valve terminal's box, it is a valve of its
    /// own that joins the terminal.
    pub valve: bool,
    /// Whether that FnDef is a contact (normally open, normally closed or change-over: a switch, a push-button, a
    /// sensor's contact): autoconnect never joins it to a PLC output.
    pub contact: bool,
    /// Whether that FnDef is a fitting (`ew_derive::is_fitting`): autoconnect never joins it to another fitting.
    pub fitting: bool,
}

/// One connection point of a [`SymbolShape`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ShapeCp {
    /// The symbol's CP index.
    pub symbol_cp: u16,
    /// The FnDef CP it draws.
    pub fn_cp: u16,
    /// Its position relative to the insertion point at R0.
    pub at: (i64, i64),
    /// The direction a wire leaves it at R0.
    pub dir: Dir,
    /// The designation of the FnDef CP it draws (`13`, `A1`), empty when the symbol's FnDef is not embedded
    /// or has no such CP.
    pub designation: String,
    /// Whether the FnDef CP it draws is a fluid port (a pneumatic, vacuum or hydraulic discipline): autoconnect joins a
    /// fluid port to a fluid port only.
    pub fluid: bool,
    /// The FnDef CP's side of EW-FL-012 (a valve's working or exhaust port): autoconnect never joins two sides that
    /// conflict.
    pub valve_port: Option<ew_derive::ValvePort>,
}

/// What `rep.exchange_symbol` without `pin_map` does to the function CPs a representation draws, from its current
/// symbol's shape and the new one ([`exchange_map`]).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ExchangeMap {
    /// The function CPs the new symbol draws, ascending.
    pub drawn: Vec<u16>,
    /// The function CPs the current symbol draws and the new one does not, ascending (the command lists them as
    /// notices, or refuses one that is connected on the page).
    pub unmapped: Vec<u16>,
    /// The first CP of the new symbol without a counterpart by designation whose index default names a function
    /// CP, `(symbol CP, FnDef CP)`: the command refuses the exchange (give `pin_map`), and `drawn` and `unmapped`
    /// are empty.
    pub unmatched: Option<(u16, u16)>,
}

/// The CP map `rep.exchange_symbol` derives without `pin_map` (`ew_commands`' `derive_pin_map`, the
/// command's own rule), for a representation that draws `current` and gets `new`: the function's CPs are those
/// `current` draws with their designations (`ShapeCp.designation`); when both shapes draw the same FnDef the map is
/// by index, else by designation, ASCII case-insensitively. A function CP the current symbol does not draw cannot
/// become unmapped, so `unmapped` is exact; `unmatched` judges only the CPs `current` draws, while the command judges
/// every CP of the function's definition (it may refuse at the commit where this predicts none).
pub fn exchange_map(current: &SymbolShape, new: &SymbolShape) -> ExchangeMap {
    let mut function: Vec<(u16, &str)> = Vec::new();
    for c in &current.cps {
        if !function.iter().any(|(i, _)| *i == c.fn_cp) {
            function.push((c.fn_cp, c.designation.as_str()));
        }
    }
    let symbol: Vec<(u16, u16, Option<&str>)> = new
        .cps
        .iter()
        .map(|c| {
            (
                c.symbol_cp,
                c.fn_cp,
                Some(c.designation.as_str()).filter(|d| !d.is_empty()),
            )
        })
        .collect();
    let drawn: BTreeSet<u16> =
        match ew_commands::commands::derive_pin_map(&function, current.fndef == new.fndef, &symbol) {
            Ok(None) => new
                .cps
                .iter()
                .map(|c| c.fn_cp)
                .filter(|f| function.iter().any(|(i, _)| i == f))
                .collect(),
            Ok(Some(entries)) => entries.into_iter().map(|(_, f)| f).collect(),
            Err(unmatched) => {
                return ExchangeMap {
                    unmatched: Some(unmatched),
                    ..ExchangeMap::default()
                };
            }
        };
    let mut unmapped: Vec<u16> = function
        .iter()
        .map(|(i, _)| *i)
        .filter(|i| !drawn.contains(i))
        .collect();
    unmapped.sort_unstable();
    ExchangeMap {
        drawn: drawn.into_iter().collect(),
        unmapped,
        unmatched: None,
    }
}

/// A schema direction as a [`Dir`] (an unknown direction reads `Up`).
pub(crate) fn dir_of(d: &Dir4) -> Dir {
    match d {
        Dir4::Up | Dir4::Unknown(_) => Dir::Up,
        Dir4::Down => Dir::Down,
        Dir4::Left => Dir::Left,
        Dir4::Right => Dir::Right,
    }
}

pub(super) fn pt(p: Pt2) -> (i64, i64) {
    (p.x.0, p.y.0)
}

/// The point at parameter `t` of a quadratic or cubic Bézier, rounded to the nearest nm.
fn bezier(points: &[Pt2], t: f64) -> (i64, i64) {
    let f = |a: f64, b: f64, c: Option<f64>, d: Option<f64>| -> f64 {
        let u = 1.0 - t;
        match (c, d) {
            (Some(c), None) => u * u * a + 2.0 * u * t * b + t * t * c,
            (Some(c), Some(d)) => u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d,
            _ => a,
        }
    };
    let coord = |sel: fn(Pt2) -> i64| -> i64 {
        let v: Vec<f64> = points.iter().map(|p| sel(*p) as f64).collect();
        f(v[0], v[1], v.get(2).copied(), v.get(3).copied()).round() as i64
    };
    (coord(|p| p.x.0), coord(|p| p.y.0))
}

/// How many segments flatten a Bézier with control points `points` to within `tol`: the uniform subdivision
/// bound `n ≥ √(c · M / tol)` with `M` the largest second difference (`c` = 1/4 for a quadratic, 3/4 for a cubic).
fn segments(points: &[Pt2], tol: f64) -> usize {
    let second = |a: Pt2, b: Pt2, c: Pt2| {
        let dx = a.x.0 as f64 - 2.0 * b.x.0 as f64 + c.x.0 as f64;
        let dy = a.y.0 as f64 - 2.0 * b.y.0 as f64 + c.y.0 as f64;
        (dx * dx + dy * dy).sqrt()
    };
    let (m, c) = if points.len() == 3 {
        (second(points[0], points[1], points[2]), 0.25)
    } else {
        (
            second(points[0], points[1], points[2]).max(second(points[1], points[2], points[3])),
            0.75,
        )
    };
    let n = (c * m / tol).sqrt().ceil();
    if n.is_finite() {
        (n as usize).clamp(1, 256)
    } else {
        1
    }
}

/// The subpaths of a path flattened to polylines.
pub(super) fn flatten(els: &[PathEl], tol: f64, out: &mut Vec<Vec<(i64, i64)>>) {
    let mut current: Vec<(i64, i64)> = Vec::new();
    let mut start: Option<Pt2> = None;
    let mut last = Pt2::ORIGIN;
    let flush = |current: &mut Vec<(i64, i64)>, out: &mut Vec<Vec<(i64, i64)>>| {
        if current.len() >= 2 {
            out.push(std::mem::take(current));
        } else {
            current.clear();
        }
    };
    for el in els {
        match *el {
            PathEl::MoveTo(p) => {
                flush(&mut current, out);
                current.push(pt(p));
                start = Some(p);
                last = p;
            }
            PathEl::LineTo(p) => {
                if current.is_empty() {
                    current.push(pt(last));
                }
                current.push(pt(p));
                last = p;
            }
            PathEl::QuadTo(c, p) => {
                if current.is_empty() {
                    current.push(pt(last));
                }
                let pts = [last, c, p];
                let n = segments(&pts, tol);
                for k in 1..n {
                    current.push(bezier(&pts, k as f64 / n as f64));
                }
                current.push(pt(p));
                last = p;
            }
            PathEl::CubicTo(c1, c2, p) => {
                if current.is_empty() {
                    current.push(pt(last));
                }
                let pts = [last, c1, c2, p];
                let n = segments(&pts, tol);
                for k in 1..n {
                    current.push(bezier(&pts, k as f64 / n as f64));
                }
                current.push(pt(p));
                last = p;
            }
            PathEl::ClosePath => {
                if let Some(s) = start {
                    if current.last() != Some(&pt(s)) {
                        current.push(pt(s));
                    }
                    last = s;
                }
                flush(&mut current, out);
            }
        }
    }
    flush(&mut current, out);
}

/// The shape of a symbol document.
pub(super) fn shape_of(q: &Q, copy: Uuid, def: &SymbolDef) -> SymbolShape {
    shape_with(copy, def, q.fndefs().get(fndef_copy(&def.fndef)).cloned())
}

/// The shape of a symbol document whose CP designations come from `fndef`.
fn shape_with(copy: Uuid, def: &SymbolDef, fndef: Option<Arc<LibCopy<FnDef>>>) -> SymbolShape {
    let designation = |fn_cp: u16| {
        fndef
            .as_ref()
            .and_then(|d| d.item.cps.iter().find(|c| c.idx == fn_cp))
            .map(|c| c.designation.as_str().to_owned())
            .unwrap_or_default()
    };
    let fluid = |fn_cp: u16| {
        fndef
            .as_ref()
            .and_then(|d| d.item.cps.iter().find(|c| c.idx == fn_cp))
            .is_some_and(|c| ew_derive::is_fluid(&c.discipline))
    };
    let valve_port = |fn_cp: u16| {
        let d = fndef.as_ref()?;
        let cp = d.item.cps.iter().find(|c| c.idx == fn_cp)?;
        ew_derive::ValvePort::of(&d.item, cp.port_role.as_ref())
    };
    let geometry = ew_scene::symbol::lower(def, Orient2::R0);
    let mut outline = Vec::new();
    for item in &geometry.graphics.items {
        if let ItemKind::Path(p) = &item.kind {
            flatten(&p.path.els, OUTLINE_TOLERANCE_NM, &mut outline);
        }
    }
    let body = body_of(&geometry).map_or([0; 4], |b| [b.min().x.0, b.min().y.0, b.max().x.0, b.max().y.0]);
    SymbolShape {
        copy,
        key: key_or_id(def.meta.key.as_ref(), copy),
        cps: def
            .cps
            .iter()
            .map(|cp| ShapeCp {
                symbol_cp: cp.idx,
                fn_cp: cp.fn_cp,
                at: pt(cp.pos),
                dir: dir_of(&cp.dir),
                designation: designation(cp.fn_cp),
                fluid: fluid(cp.fn_cp),
                valve_port: valve_port(cp.fn_cp),
            })
            .collect(),
        body,
        outline,
        rep_type: rep_type_word(&def.rep_type),
        fndef: fndef_copy(&def.fndef).as_uuid(),
        valve: fndef
            .as_ref()
            .is_some_and(|d| d.item.contact_kind == ContactKind::Valve),
        contact: fndef.as_ref().is_some_and(|d| {
            matches!(
                d.item.contact_kind,
                ContactKind::No | ContactKind::Nc | ContactKind::Co
            )
        }),
        fitting: fndef.as_ref().is_some_and(|d| ew_derive::is_fitting(&d.item)),
    }
}

/// The body of a symbol as the scene lowers it under an orientation: the bounds of its paths (its graphics without
/// texts, without their strokes), relative to its insertion point; `None` without a path.
pub(super) fn body_of(geometry: &ew_scene::symbol::SymbolGeometry) -> Option<Rect> {
    geometry
        .graphics
        .items
        .iter()
        .fold(None, |acc, item| match &item.kind {
            ItemKind::Path(p) => match (acc, p.path.bounds()) {
                (Some(a), Some(b)) => Some(a.union(b)),
                (a, b) => a.or(b),
            },
            _ => acc,
        })
}

impl Session {
    /// The embedded symbol copy `copy` at orientation A (R0), relative to its insertion point, in page nm:
    /// every symbol CP with its FnDef CP and direction, the bounds of its graphics without texts
    /// (`body`), every stroked or filled path flattened to polylines with a tolerance of 50 000 nm in paint order
    /// (`outline`). A starter symbol the project does not hold yet has the shape of its starter copy (the copy
    /// ID [`Session::starter_symbol_copy`] gives), its designations from the embedded FnDef, else the starter's
    /// (the Place tool floats it before the placement embeds it). `None` for any other copy.
    pub fn symbol_shape(&self, copy: Uuid) -> Option<SymbolShape> {
        let q = self.query();
        if let Some(symbol) = q.symbols().get(Id::from_uuid(copy)).cloned() {
            return Some(shape_of(&q, copy, &symbol.item));
        }
        let starter = starter_docs().iter().find_map(|(_, _, d)| match d {
            LibDocument::Symbol(c) if c.id.as_uuid() == copy => Some(c),
            _ => None,
        });
        if let Some(starter) = starter {
            return Some(shape_with(copy, &starter.item, fndef_of(&q, &starter.item.fndef)));
        }
        let symbol = self
            .libraries()
            .libraries()
            .into_iter()
            .flat_map(|l| self.libraries().documents(l.id, ew_schema::Kind::Symbol))
            .find_map(|doc| match doc {
                LibDocument::Symbol(s) if s.id.as_uuid() == copy => Some(s),
                _ => None,
            })?;
        let def = match self.libraries().resolve(&symbol.item.fndef).ok()? {
            LibDocument::Fndef(f) => Some(Arc::new(f)),
            _ => None,
        };
        Some(shape_with(copy, &symbol.item, def))
    }

    /// The copy ID the starter symbol `key` has once embedded, `v5(NS_LIBCOPY, library ‖ item)` (the ID of its
    /// starter copy); `None` when `key` is not a starter symbol. The Place tool names the symbol by it before the
    /// placement embeds it.
    pub fn starter_symbol_copy(&self, key: &str) -> Option<Uuid> {
        starter_symbol(key)
            .map(|c| c.id.as_uuid())
            .or_else(|| self.installed_symbol(key).map(|c| c.id.as_uuid()))
    }

    /// The library documents (`lib/` JSON form) a valve terminal drawn as its positions needs to draw its valves and its
    /// supply in its box (`seat_reps`), for the commands that fill a position or draw the box to embed: for the
    /// valve FnDef of every valve of the terminal of `device` (the terminal or one of its modules; `None` for a terminal
    /// not made yet) and of every part of `parts` (a valve about to take a position), and for the terminal's supply (its
    /// own, else the starter's supply FnDef, `ew_derive::is_supply_def`), on the grid of each page the terminal is drawn
    /// as its positions on and of `page`: the starter fluid symbol that draws it and its FnDef, each when the project
    /// embeds none ([`box_symbol`]). Empty when nothing is missing.
    pub fn cell_copies(
        &self,
        device: Option<Uuid>,
        parts: &[Uuid],
        page: Option<Uuid>,
    ) -> Vec<serde_json::Value> {
        let q = self.query();
        let terminal = device.map(Id::<Device>::from_uuid).and_then(|d| {
            if ew_derive::is_fluid_terminal(&q, d) {
                Some(d)
            } else {
                q.devices()
                    .get(d)?
                    .parent
                    .filter(|p| ew_derive::is_fluid_terminal(&q, *p))
            }
        });
        let mut grids: Vec<ew_units::Grid> = page
            .and_then(|p| q.pages().get(Id::from_uuid(p)).map(|p| p.grid))
            .into_iter()
            .collect();
        let mut defs: Vec<Id<FnDef>> = Vec::new();
        let def_of = |f: Id<Function>| q.functions().get(f).map(|f| f.def);
        if let Some(t) = terminal {
            for f in q.idx().functions_of_device(t) {
                for r in q.idx().reps_of_function(f) {
                    if let Some(r) = q.reps().get(r)
                        && r.box_.as_ref().is_some_and(|b| b.slots.is_some())
                        && let Some(p) = q.pages().get(r.page)
                        && !grids.contains(&p.grid)
                    {
                        grids.push(p.grid);
                    }
                }
            }
            for c in ew_derive::terminal_cells(&q, t) {
                if let ew_derive::ValveSlot::Valve { device, .. } = c.slot {
                    defs.extend(
                        ew_derive::valve_functions(&q, device)
                            .into_iter()
                            .filter_map(def_of),
                    );
                }
            }
        }
        for part in parts {
            let item = q
                .parts()
                .get(Id::from_uuid(*part))
                .map(|c| c.item.clone())
                .or_else(|| {
                    let r = self.library_part_ref(*part)?;
                    match self.libraries().resolve(&r) {
                        Ok(LibDocument::Part(c)) => Some(c.item),
                        _ => None,
                    }
                });
            defs.extend(item.and_then(|p| part_valve_def(&q, &p)));
        }
        let supply = terminal
            .and_then(|t| ew_derive::terminal_supply(&q, t))
            .and_then(def_of);
        defs.extend(supply.or_else(|| {
            starter_docs().iter().find_map(|(_, _, d)| match d {
                LibDocument::Fndef(c) if ew_derive::is_supply_def(&c.item) => Some(c.id),
                _ => None,
            })
        }));
        defs.sort();
        defs.dedup();
        let mut out: Vec<serde_json::Value> = Vec::new();
        let mut seen: BTreeSet<Uuid> = BTreeSet::new();
        for def in defs {
            for grid in &grids {
                let Some((symbol, true)) = box_symbol(&q, def, Some(*grid)) else {
                    continue;
                };
                if seen.insert(symbol.id.as_uuid())
                    && let Ok(v) = serde_json::to_value(LibDocument::Symbol(symbol))
                {
                    out.push(v);
                }
                if !q.fndefs().contains(def)
                    && seen.insert(def.as_uuid())
                    && let Some(c) = starter_fndef(def)
                    && let Ok(v) = serde_json::to_value(LibDocument::Fndef(c.clone()))
                {
                    out.push(v);
                }
            }
        }
        out
    }

    /// The interfaces of device `device`'s bus part (the first part use whose part has bus interfaces, normaliser
    /// N5's) that no representation on the drawn page `folio` draws ([`part_interfaces`]): what `bus.place_ports`
    /// still has to draw there. Empty for a device without such a part or an unknown page.
    pub fn undrawn_interfaces(&self, device: Uuid, folio: &str) -> Vec<CatalogueInterface> {
        let q = self.query();
        let Some((_, page)) = drawn_page(&q, folio) else {
            return Vec::new();
        };
        let id: Id<Device> = Id::from_uuid(device);
        let Some(part) = q.devices().get(id).and_then(|d| {
            d.parts.iter().find_map(|u| {
                q.parts()
                    .get(u.part.copy)
                    .filter(|c| ew_derive::has_bus_ports(&c.item))
            })
        }) else {
            return Vec::new();
        };
        let drawn: Vec<(String, Option<String>)> = q
            .idx()
            .functions_of_device(id)
            .filter(|f| {
                q.idx()
                    .reps_of_function(*f)
                    .any(|r| q.reps().get(r).is_some_and(|r| r.page == page))
            })
            .filter_map(|f| q.functions().get(f)?.bus_port.clone())
            .map(|p| (p.interface.to_string(), p.port_label.map(|l| l.to_string())))
            .collect();
        part_interfaces(&part.item)
            .into_iter()
            .filter(|i| !drawn.contains(&(i.interface.clone(), i.port_label.clone())))
            .collect()
    }

    /// The boxes on the drawn page `folio` that draw a port function, as earlier versions drew a bus device (a box per
    /// interface, doc 10 §2.7): each box's representation with its device, in representation ID order. What Draw as
    /// one symbol replaces.
    pub fn bus_boxes(&self, folio: &str) -> Vec<(Uuid, Uuid)> {
        let q = self.query();
        let Some((_, page)) = drawn_page(&q, folio) else {
            return Vec::new();
        };
        let mut out: Vec<(Uuid, Uuid)> = q
            .idx()
            .reps_by_page(page)
            .filter_map(|r| {
                let rep = q.reps().get(r)?;
                rep.box_.as_ref()?;
                let f = q.functions().get(*rep.functions.first()?)?;
                f.bus_port.as_ref()?;
                Some((r.as_uuid(), f.device?.as_uuid()))
            })
            .collect();
        out.sort();
        out
    }

    /// The bus symbol device `device` is drawn with on the drawn page `folio` (its bus part's, his own or the derived one
    /// on the page's grid, `ew_derive::part_bus_symbol`);
    /// `None` for a device without a bus part or an unknown page.
    pub fn device_bus_symbol(&self, device: Uuid, folio: &str) -> Option<super::BusSymbolShape> {
        let q = self.query();
        let (_, page) = drawn_page(&q, folio)?;
        let grid = q.pages().get(page)?.grid;
        let part = q
            .devices()
            .get(Id::<Device>::from_uuid(device))?
            .parts
            .iter()
            .find_map(|u| {
                q.parts()
                    .get(u.part.copy)
                    .filter(|c| ew_derive::has_bus_ports(&c.item))
            })?;
        Some(super::bus_symbol_shape(&ew_derive::part_bus_symbol(
            &q, &part.item, grid,
        )))
    }

    /// The CPs of function `function` as it derives them (`FunctionCps`): each CP index with its designation, in index
    /// order; empty for an unknown function. What a black box drawing the function lays on its edges.
    pub fn function_cp_list(&self, function: Uuid) -> Vec<(u16, String)> {
        let q = self.query();
        let mut out: Vec<(u16, String)> = q
            .get::<FunctionCps>(Id::from_uuid(function))
            .iter()
            .map(|c| (c.idx, c.designation.clone()))
            .collect();
        out.sort();
        out
    }

    /// The starter FnDef documents (`lib/` JSON form) among `fndefs` (copy IDs) that the project does not embed, each
    /// once: what `bus.place_ports` embeds for the port functions it makes.
    pub fn port_fndef_copies(&self, fndefs: &[Uuid]) -> Vec<serde_json::Value> {
        let q = self.query();
        let mut out: Vec<serde_json::Value> = Vec::new();
        let mut seen: BTreeSet<Uuid> = BTreeSet::new();
        for id in fndefs {
            if q.fndefs().contains(Id::from_uuid(*id)) || !seen.insert(*id) {
                continue;
            }
            if let Some(v) = starter::items()
                .iter()
                .find(|i| starter_id(i.json) == Some(*id))
                .and_then(|i| serde_json::from_str(i.json).ok())
            {
                out.push(v);
            }
        }
        out
    }
}
