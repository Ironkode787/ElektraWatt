// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A valve terminal drawn as its valve positions (doc 11 §3.2): what each cell of a slotted box holds
//! ([`terminal_cells`]), which device may take a cell ([`fits_terminal`]), and the geometry of the cells
//! ([`SlotFrame`]).
//!
//! The box (`BoxSpec.slots`) stands on the page grid: a supply end of [`END_STEPS`] grid steps on the left (where the
//! terminal's supply ports 1, 3 and 5 are drawn), then one cell of `pitch` per valve position, left to right, all
//! [`HEIGHT_STEPS`] high. A valve stands in its cell with its working ports 2 and 4 on the top edge, where a tube leaves
//! upwards; its ports 1, 3 and 5 end on the sub-base, a band along the bottom four steps high, which the scene draws
//! with the galleries through it ([`GALLERY_PORTS`], one row each, which join those ports in the model, doc 11 §5.2)
//! and the cell numbers under them.

use ew_ids::Id;
use ew_schema::{ContactKind, Device, FluidPortRole, FnDef, Primitive, SymbolDef};
use ew_units::{Grid, Nm, Pt2};

use crate::{Q, ValveSlot, ValveSlots};

/// The width of one cell, in grid steps: a 5/2 valve of the 12 mm module (39 mm with its solenoid and spring) with a
/// step to spare.
pub const CELL_STEPS: i64 = 11;
/// The width of the supply end, in grid steps: the supply plate's symbol (six steps).
pub const END_STEPS: i64 = 6;
/// The height of the box, in grid steps.
pub const HEIGHT_STEPS: i64 = 8;
/// The row of a valve's insertion point (its working ports), in grid steps below the top edge: on it.
const VALVE_TOP: i64 = 0;
/// The row of the sub-base's top edge, where a valve's ports 1, 3 and 5 end (a valve symbol is four steps high).
pub const BAND_TOP: i64 = 4;
/// The galleries the sub-base draws, one row each below its top edge (the first one step down): the supply 1 and the
/// exhausts 3 and 5.
pub const GALLERY_PORTS: [FluidPortRole; 3] = [
    FluidPortRole::Supply1,
    FluidPortRole::Exhaust3,
    FluidPortRole::Exhaust5,
];
/// The row of the supply plate's insertion point: its ports end on the bottom edge.
const SUPPLY_TOP: i64 = 3;

/// One cell of a slotted box: the module whose position it is (the terminal itself, or a sub-base), the position,
/// and what it holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ValveCell {
    /// The module the position belongs to.
    pub module: Id<Device>,
    /// The position, 1-based, on its module.
    pub position: u32,
    /// What it holds.
    pub slot: ValveSlot,
}

/// The cells of a valve terminal, left to right: the positions of the terminal itself, then those of each child
/// module in position order (`valve_slots` of each); empty for a device without valve positions.
pub fn terminal_cells(q: &Q, terminal: Id<Device>) -> Vec<ValveCell> {
    let mut modules = vec![terminal];
    modules.extend(crate::fluid::ordered_children(q, terminal));
    let mut out = Vec::new();
    for m in modules {
        let slots = q.get::<ValveSlots>(m);
        for (p, slot) in (1u32..).zip(slots.slots.iter()) {
            out.push(ValveCell {
                module: m,
                position: p,
                slot: slot.clone(),
            });
        }
    }
    out
}

/// Whether a device with functions on the FnDefs `defs` may sit on a valve terminal's position: it has a valve function
/// and no coil of its own to wire (a coil FnDef with a physical CP: a stand-alone valve's A1, A2). A valve for a
/// terminal has an internal coil the terminal drives, or none drawn yet.
pub fn fits_terminal<'a>(defs: impl IntoIterator<Item = &'a FnDef>) -> bool {
    let mut valve = false;
    for d in defs {
        match d.contact_kind {
            ContactKind::Valve => valve = true,
            ContactKind::Coil if d.cps.iter().any(|c| c.physical) => return false,
            _ => {}
        }
    }
    valve
}

/// Whether a FnDef is a valve terminal's supply (the supply plate's, filed under `ValveTerminal`): its ports 1, 3 and 5
/// feed the galleries, on a supply plate or on the terminal itself.
pub fn is_supply_def(def: &FnDef) -> bool {
    def.category.0.as_str() == "ValveTerminal"
}

/// [`fits_terminal`] of a device of the model.
pub fn device_fits_terminal(q: &Q, device: Id<Device>) -> bool {
    let functions = q.functions();
    let fndefs = q.fndefs();
    let defs: Vec<_> = q
        .idx()
        .functions_of_device(device)
        .filter_map(|f| fndefs.get(functions.get(f)?.def))
        .collect();
    fits_terminal(defs.iter().map(|d| &d.item))
}

/// The left and right ends of a symbol's graphics, relative to its insertion point (texts left out); `None` for a
/// symbol without graphics of these kinds.
pub fn graphics_x_extent(symbol: &SymbolDef) -> Option<(Nm, Nm)> {
    let mut xs: Vec<i64> = Vec::new();
    for p in &symbol.graphics {
        match p {
            Primitive::Line(l) => xs.extend([l.from.x.0, l.to.x.0]),
            Primitive::Polyline(l) => xs.extend(l.points.iter().map(|p| p.x.0)),
            Primitive::Polygon(l) => xs.extend(l.points.iter().map(|p| p.x.0)),
            Primitive::Rect(r) => xs.extend([r.rect.0.min().x.0, r.rect.0.max().x.0]),
            Primitive::Ellipse(r) => xs.extend([r.rect.0.min().x.0, r.rect.0.max().x.0]),
            Primitive::Arc(r) => xs.extend([r.rect.0.min().x.0, r.rect.0.max().x.0]),
            Primitive::Circle(c) => xs.extend([
                c.centre.x.0.saturating_sub(c.r.0),
                c.centre.x.0.saturating_add(c.r.0),
            ]),
            _ => {}
        }
    }
    Some((Nm(*xs.iter().min()?), Nm(*xs.iter().max()?)))
}

/// The geometry of a slotted box on its page: the grid, the grid indices of its top-left corner, the cell width in
/// grid steps, and the width of each cell that needs more (a 5/3 valve).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotFrame {
    grid: Grid,
    x0: i64,
    y0: i64,
    cell: i64,
    /// The width of cell `i` in grid steps, at least `cell`; a cell after the last is `cell` wide.
    widths: Vec<i64>,
}

impl SlotFrame {
    /// The frame of a box at `pos` (its top-left corner) whose cells are `pitch` wide, on a page of grid `grid`; the
    /// pitch is taken in whole grid steps (at least one).
    pub fn new(grid: Grid, pos: Pt2, pitch: Nm) -> SlotFrame {
        let cell = grid.snap(pitch).max(1);
        SlotFrame {
            grid,
            x0: grid.snap(pos.x),
            y0: grid.snap(pos.y),
            cell,
            widths: Vec::new(),
        }
    }

    /// The frame with each cell as wide as the valve drawn in it needs: `extents[i]` is the graphics extent of
    /// the widest symbol in cell `i` ([`graphics_x_extent`]; `None`: nothing drawn there). A cell is the pitch wide,
    /// and a valve wider than that (a 5/3 valve, fifteen steps) takes its own width and a step to spare; the cells to
    /// its right move with it, and the box grows to the right.
    pub fn with_cells(mut self, extents: &[Option<(Nm, Nm)>]) -> SlotFrame {
        let pitch = self.grid.at(self.cell).unwrap_or(Nm(0));
        self.widths = extents
            .iter()
            .map(|e| match e {
                Some((min, max)) if *max - *min > pitch => {
                    let w = *max - *min;
                    let steps = self.grid.snap(w);
                    let steps = if self.grid.at(steps).is_some_and(|at| at < w) {
                        steps.saturating_add(1)
                    } else {
                        steps
                    };
                    steps.saturating_add(1).max(self.cell)
                }
                _ => self.cell,
            })
            .collect();
        self
    }

    /// The cell width of a new box on grid `grid`: [`CELL_STEPS`] grid steps.
    pub fn pitch(grid: Grid) -> Nm {
        grid.at(CELL_STEPS).unwrap_or(Nm(0))
    }

    fn x(&self, k: i64) -> Nm {
        self.grid.at(self.x0.saturating_add(k)).unwrap_or(Nm(0))
    }

    fn y(&self, k: i64) -> Nm {
        self.grid.at(self.y0.saturating_add(k)).unwrap_or(Nm(0))
    }

    /// The top-left corner.
    pub fn origin(&self) -> Pt2 {
        Pt2::new(self.x(0), self.y(0))
    }

    /// The grid column of the left edge of cell `i` (0-based), relative to the box: the supply end, then the cells
    /// before it.
    fn col(&self, i: usize) -> i64 {
        let known = i.min(self.widths.len());
        let wide: i64 = self.widths[..known].iter().fold(0, |a, w| a.saturating_add(*w));
        let rest = i64::try_from(i - known).unwrap_or(i64::MAX / 2);
        END_STEPS
            .saturating_add(wide)
            .saturating_add(self.cell.saturating_mul(rest))
    }

    /// The outline of a box of `n` cells, relative to its top-left corner (clockwise from it).
    pub fn outline(&self, n: usize) -> Vec<Pt2> {
        let o = self.origin();
        let w = self.x(self.col(n)) - o.x;
        let h = self.y(HEIGHT_STEPS) - o.y;
        vec![
            Pt2::ORIGIN,
            Pt2::new(w, Nm(0)),
            Pt2::new(w, h),
            Pt2::new(Nm(0), h),
        ]
    }

    /// The left and right x of cell `i` (0-based), on the page.
    pub fn cell_x(&self, i: usize) -> (Nm, Nm) {
        (self.x(self.col(i)), self.x(self.col(i + 1)))
    }

    /// The y of the top edge, of the sub-base's top edge and of the bottom edge, on the page.
    pub fn rows(&self) -> (Nm, Nm, Nm) {
        (self.y(0), self.y(BAND_TOP), self.y(HEIGHT_STEPS))
    }

    /// The right x of the supply end, on the page.
    pub fn end_x(&self) -> Nm {
        self.x(END_STEPS)
    }

    /// The y of the row of gallery `k` of [`GALLERY_PORTS`], on the page.
    pub fn gallery_y(&self, k: usize) -> Nm {
        self.y(BAND_TOP
            .saturating_add(1)
            .saturating_add(i64::try_from(k).unwrap_or(0)))
    }

    /// The cell of a box of `n` cells that the page point `p` lies in (0-based); `None` outside every cell.
    pub fn cell_at(&self, n: usize, p: Pt2) -> Option<usize> {
        let (top, _, bottom) = self.rows();
        if p.y < top || p.y > bottom {
            return None;
        }
        (0..n).find(|i| {
            let (l, r) = self.cell_x(*i);
            p.x >= l && p.x < r
        })
    }

    /// The insertion point of a valve in cell `i` whose graphics span `extent` (relative x): centred in the cell on the
    /// nearest grid column, its working ports [`VALVE_TOP`] steps below the top edge.
    pub fn valve_at(&self, i: usize, extent: Option<(Nm, Nm)>) -> Pt2 {
        let (l, r) = self.cell_x(i);
        let (min, max) = extent.unwrap_or((Nm(0), Nm(0)));
        let centre = (i128::from(l.0) + i128::from(r.0)) / 2;
        let half = (i128::from(min.0) + i128::from(max.0)) / 2;
        let x = i64::try_from(centre - half).unwrap_or(l.0);
        Pt2::new(self.grid.snap_point(Nm(x)).unwrap_or(l), self.y(VALVE_TOP))
    }

    /// The insertion point of the supply plate's symbol: at the left edge, its ports on the bottom edge.
    pub fn supply_at(&self) -> Pt2 {
        Pt2::new(self.x(0), self.y(SUPPLY_TOP))
    }
}

/// The frame of box representation `rep` of a valve terminal drawn as its positions (`BoxSpec.slots`) on its page,
/// each cell as wide as the valve drawn in it there needs ([`SlotFrame::with_cells`]: the widest symbol of the cell's
/// valve on the box's page); `None` for a representation that is no such box or a page that is gone.
pub fn box_frame(q: &Q, rep: &ew_schema::Representation) -> Option<SlotFrame> {
    let slots = rep.box_.as_ref()?.slots.as_ref()?;
    let grid = q.pages().get(rep.page)?.grid;
    let terminal = rep
        .functions
        .first()
        .and_then(|f| q.functions().get(*f))
        .and_then(|f| f.device);
    let frame = SlotFrame::new(grid, rep.pos, slots.pitch);
    let Some(terminal) = terminal else {
        return Some(frame);
    };
    let (reps, symbols) = (q.reps(), q.symbols());
    let extents: Vec<Option<(Nm, Nm)>> = terminal_cells(q, terminal)
        .iter()
        .map(|c| match c.slot {
            ValveSlot::Valve { device, .. } => crate::fluid::valve_functions(q, device)
                .into_iter()
                .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
                .filter_map(|r| reps.get(r))
                .filter(|r| r.page == rep.page && r.box_.is_none())
                .filter_map(|r| graphics_x_extent(&symbols.get(r.symbol)?.item))
                .max_by_key(|(min, max)| *max - *min),
            _ => None,
        })
        .collect();
    Some(frame.with_cells(&extents))
}

/// The supply of a valve terminal drawn in its box's supply end: the lowest-ID function of the terminal, else of a
/// child without a position, whose device's part taps a gallery and that has a fluid CP of the tap's designation (the
/// supply plate's function), or, of the terminal, that is a supply ([`is_supply_def`]: its own supply ports).
/// `None` when the terminal has neither.
pub fn terminal_supply(q: &Q, terminal: Id<Device>) -> Option<Id<ew_schema::Function>> {
    let devices = q.devices();
    let parts = q.parts();
    let functions = q.functions();
    let fndefs = q.fndefs();
    let mut candidates = vec![terminal];
    candidates.extend(
        crate::fluid::ordered_children(q, terminal)
            .into_iter()
            .filter(|c| devices.get(*c).is_some_and(|d| d.position.is_none())),
    );
    candidates.into_iter().find_map(|d| {
        let dev = devices.get(d)?;
        let taps: Vec<String> = dev
            .parts
            .iter()
            .filter_map(|u| parts.get(u.part.copy))
            .filter_map(|p| p.item.sections.fluid.clone())
            .flat_map(|f| {
                f.galleries
                    .into_iter()
                    .filter_map(|g| g.tap.map(|t| t.as_str().to_owned()))
            })
            .collect();
        let mut fns: Vec<_> = q.idx().functions_of_device(d).collect();
        fns.sort();
        fns.into_iter().find(|f| {
            q.get::<crate::FunctionCps>(*f)
                .iter()
                .any(|c| crate::is_fluid(&c.discipline) && taps.contains(&c.designation))
                || d == terminal
                    && functions
                        .get(*f)
                        .and_then(|f| fndefs.get(f.def))
                        .is_some_and(|d| is_supply_def(&d.item))
        })
    })
}

/// Whether a device is a multipin valve terminal (doc 10 §2.3): no PLC station, and a part whose fluid section
/// names its plug's pins (`fluid.pins`); its solenoids are wired through the plug to outputs of a PLC card elsewhere.
pub fn is_multipin_terminal(q: &Q, device: Id<Device>) -> bool {
    let Some(dev) = q.devices().get(device) else {
        return false;
    };
    let parts = q.parts();
    dev.aspects.plc_station.is_none()
        && dev.parts.iter().any(|u| {
            parts
                .get(u.part.copy)
                .and_then(|c| c.item.sections.fluid.as_ref().map(|f| !f.pins.is_empty()))
                .unwrap_or(false)
        })
}

/// Whether a device is a valve terminal of either kind: with a bus node (a PLC station, [`crate::is_valve_terminal`])
/// or multipin ([`is_multipin_terminal`]).
pub fn is_fluid_terminal(q: &Q, device: Id<Device>) -> bool {
    crate::is_valve_terminal(q, device) || is_multipin_terminal(q, device)
}

/// The plug pin a solenoid of a valve on a multipin valve terminal is wired through: the terminal and the pin's
/// designation, the part's `fluid.pins` entry of `solenoid` at the valve's position index (position − 1); `None` for a
/// valve on no multipin terminal or a solenoid its plug does not name.
pub fn solenoid_pin(q: &Q, valve: Id<Device>, solenoid: &str) -> Option<(Id<Device>, String)> {
    let dev = q.devices().get(valve)?;
    let terminal = dev.parent.filter(|t| is_multipin_terminal(q, *t))?;
    let index = dev.position?.checked_sub(1)?;
    let parts = q.parts();
    let t = q.devices().get(terminal)?;
    let pin = t.parts.iter().find_map(|u| {
        let fluid = parts.get(u.part.copy)?.item.sections.fluid.clone()?;
        let entry = fluid
            .pins
            .iter()
            .find(|e| e.solenoid.as_str() == solenoid)?
            .clone();
        crate::eval_map(&entry.pin, ew_plcaddr::MapVar::P, index)
    })?;
    Some((terminal, pin.to_string()))
}

/// What one pin of a multipin valve terminal's plug carries ([`plug_pins`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlugPin {
    /// A solenoid of a valve position: the position (1-based), the solenoid, and the valve that holds the position.
    Solenoid {
        /// The position.
        position: u32,
        /// The solenoid ("14").
        solenoid: String,
        /// The valve at the position, if any.
        valve: Option<Id<Device>>,
    },
    /// The common of the solenoids: a pin of the plug's pattern that no position's solenoid takes.
    Common,
}

/// The pins of a multipin valve terminal's plug by designation: each pin a solenoid of a position takes (the
/// part's `fluid.pins` at every position index), with the valve that holds the position, and every other pin of the
/// terminal's main function the common. Empty for any other device.
pub fn plug_pins(q: &Q, terminal: Id<Device>) -> Vec<(String, PlugPin)> {
    if !is_multipin_terminal(q, terminal) {
        return Vec::new();
    }
    let Some(dev) = q.devices().get(terminal) else {
        return Vec::new();
    };
    let parts = q.parts();
    let Some(fluid) = dev.parts.iter().find_map(|u| {
        parts
            .get(u.part.copy)?
            .item
            .sections
            .fluid
            .clone()
            .filter(|f| !f.pins.is_empty())
    }) else {
        return Vec::new();
    };
    let slots = q.get::<ValveSlots>(terminal);
    let mut out: Vec<(String, PlugPin)> = Vec::new();
    for (p, slot) in (1u32..).zip(slots.slots.iter()) {
        let valve = match slot {
            ValveSlot::Valve { device, .. } => Some(*device),
            _ => None,
        };
        for e in &fluid.pins {
            if let Some(pin) = crate::eval_map(&e.pin, ew_plcaddr::MapVar::P, p - 1) {
                out.push((
                    pin.to_string(),
                    PlugPin::Solenoid {
                        position: p,
                        solenoid: e.solenoid.as_str().to_owned(),
                        valve,
                    },
                ));
            }
        }
    }
    if let Some(main) = *q.get::<crate::MainFunction>(terminal) {
        for cp in q.get::<crate::FunctionCps>(main).iter() {
            if !out.iter().any(|(d, _)| *d == cp.designation) {
                out.push((cp.designation.clone(), PlugPin::Common));
            }
        }
    }
    out
}

/// The output channel wired to the plug pin of solenoid `solenoid` of `valve` on a multipin valve terminal: of
/// the channels whose traced target is the terminal's main function, the lowest-ID one whose traced pin is that pin;
/// `None` for a valve on no multipin terminal or a pin no output reaches.
pub fn wired_solenoid_channel(q: &Q, valve: Id<Device>, solenoid: &str) -> Option<Id<ew_schema::Channel>> {
    let (terminal, pin) = solenoid_pin(q, valve, solenoid)?;
    let main = (*q.get::<crate::MainFunction>(terminal))?;
    let idx = q
        .get::<crate::FunctionCps>(main)
        .iter()
        .find(|c| c.designation == pin)?
        .idx;
    let targeted = q.get::<crate::ChannelsByTarget>(());
    let channels = q.table::<ew_schema::Channel>();
    let mut hits: Vec<Id<ew_schema::Channel>> = targeted
        .targets
        .get(&main)?
        .iter()
        .copied()
        .filter(|c| {
            channels.get(*c).is_some_and(|ch| {
                q.get::<crate::ChannelTargets>(ch.module)
                    .iter()
                    .any(|t| t.channel == *c && t.pin.is_some_and(|p| p.function == main && p.idx == idx))
            })
        })
        .collect();
    hits.sort();
    hits.first().copied()
}
