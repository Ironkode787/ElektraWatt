// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The black box tool (`black_box`; FR-DEV-013, FR-DEV-039) and the grips of a selected black box.
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | armed | `PointerMove`, `CursorMove` | armed | `Transient`: a grid marker at the snapped point |
//! | armed | `Press` or `Activate` | outline | the first corner is fixed; `Transient`: a grid marker at it |
//! | outline | `PointerMove`, `CursorMove` | outline | `Transient`: the rectangle to the snapped point (grown to the right until the default CPs fit), a marker per CP; a grid marker ringed by a CP marker at the first corner while the rectangle has no area |
//! | outline | `Dwell { ms ≥ 80 }` | outline | `Preview(box.black …)` |
//! | outline | `Press` or `Activate` | armed | `Commit(box.black …)` (a rectangle without width or height commits nothing) |
//! | outline | `Escape` | armed (the hint says a second Escape leaves the tool) | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |
//!
//! **The command** is `box.black {page, symbol: the black-box symbol, pos: the top-left corner, box: {outline: the
//! rectangle relative to pos (v0 top-left, then clockwise), cps}, target}`. **Default CPs** for a part target: the
//! template entry's CPs in template order, the first `⌈n/2⌉` on the top edge and the rest on the bottom edge, left to
//! right, leaving up and down; an entry with a source's outputs among its CPs (a power supply's + and −) has its inputs
//! on the top edge and its outputs on the bottom one (FT-1702); when an edge is too short the rectangle grows to the right until they fit one grid step
//! apart with one grid step to spare at each corner. On a wider box the CPs of an edge spread over its drawn length:
//! one common pitch, the largest whole number of grid steps that fits `m + 1` times into the edge, the group
//! centred (half a step to the left when it cannot be centred exactly). Without a part, no CPs: such a box takes pins of its
//! own afterwards, where it stands ([`super::box_pin`], rc.5).
//!
//! **A valve terminal drawn as its positions** ([`BlackBoxTool::with_slots`], doc 11 §3.2, rc.2): the box is sized by the
//! terminal's positions, not by two clicks; armed, the tool shows it dashed with its top-left corner at the pointer, and
//! one click commits `box.black` with `box: {outline, slots: {pitch}}` (`ew_api::geometry::slot_box`). **A multipin
//! terminal's plug** ([`BlackBoxTool::with_pins`]) is placed the same way, sized by its pins down its two sides
//! ([`pin_layout`]). Such a one-click box stands inside the frame's drawing area, moved there by whole grid steps where
//! it would leave it, and is refused with its size where it is larger than the area (`reason-box-too-large`) and on
//! another box (`reason-box-occupied`); the host ends the tool once it is placed (rc.2: one click, one box).
//!
//! **Grips** ([`grip_points`], [`reshape_by_grip`]): for an outline of `n` vertices in stored order, grips `0 … n−1`
//! are the vertices and grips `n … 2n−1` the midpoints of the edges `v0v1, …, v(n−1)v0`. On a rectangle a vertex grip
//! moves that corner and keeps the rectangle, an edge grip moves that edge perpendicular to itself; on any other
//! outline a vertex grip moves that vertex alone and an edge grip translates an axis-aligned edge (another edge is
//! refused, `reason-grip-not-axis`). Select drags them (and a box CP along the outline) and commits `box.reshape`.

use crate::key;
use crate::strings::Key;
use ew_api::AnyCommand;
use ew_api::geometry::{Dir, orient_dir, orient_point};
use ew_api::output::TableRep;
use ew_dl::EntityRef;
use serde_json::{Value, json};

use super::wire::PageGridMap;
use super::{
    Anchor, Button, CanvasEvent, Ghost, MarkerKind, Overlay, OverlayItem, Style, Token, Tool, ToolCx,
    ToolOutput, rect_of,
};

/// What a black box creates.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BoxTarget {
    /// A new device with this letter.
    NewDevice {
        /// The letter code.
        letter: String,
    },
    /// A device made from a part, the box drawing a template entry that is a black box.
    Part {
        /// The part copy's ID text.
        part: String,
        /// The template entry.
        template: u16,
        /// The new device's letter.
        letter: String,
        /// The part number (the label's).
        number: String,
        /// The entry's CPs in template order: (function CP index, designation).
        cps: Vec<(u16, String)>,
        /// The function CP indices among them that are a source's outputs, laid along the bottom edge.
        outputs: Vec<u16>,
        /// The part's device box (doc 07 §2.5): one click places the box as the part lays it out.
        layout: Option<ew_api::part_box::PartBox>,
    },
    /// Another representation of an unplaced black-box function (`existing_function`): the supply pins of a station
    /// placed first on a bus-topology page, drawn on a schematic page.
    Function {
        /// The function's ID text.
        function: String,
        /// Its CPs: (function CP index, designation).
        cps: Vec<(u16, String)>,
    },
    /// A new function of a device (`join_device`): a valve terminal made as a PLC station, drawn first on a fluid page
    /// as its positions (rc.2); its main function, or another when its bus port drawn on a bus page is that (rc.3).
    Device {
        /// The device's ID text.
        device: String,
        /// Whether the box is the device's main function.
        main: bool,
    },
    /// A station or one of its modules drawn on an electrical page by its box (`plc.draw_station` of the one box,
    /// rc.5): Place here on a PLC part that no electrical page draws, made with or without functions.
    Station {
        /// The station's ID text.
        station: String,
        /// The device's ID text: the station or a module of it.
        device: String,
        /// What the label names: the device's tag.
        label: String,
        /// The CPs of its part's black-box entry: (function CP index, designation).
        cps: Vec<(u16, String)>,
    },
    /// The interfaces of a device on a bus-topology page, a box each (`bus.place_ports`, doc 10 §2.7).
    Ports {
        /// The command's `target`: `{"device": {"device": id}}` or `{"from_part": {"part": id, "letter": l}}`.
        target: Value,
        /// What the label names (`part EW-GEN-CPU-2`, `-3A1`).
        label: String,
        /// The interfaces, in box order from left to right.
        interfaces: Vec<PortTarget>,
    },
}

/// One interface a [`BoxTarget::Ports`] draws.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PortTarget {
    /// The interface ("X1").
    pub interface: String,
    /// The port label of an interface listed port by port.
    pub port_label: Option<String>,
    /// The FnDef copy ID text a new port function takes.
    pub fndef: String,
    /// The box's sockets: (function CP index, label).
    pub sockets: Vec<(u16, String)>,
}

/// The boxes of a [`BoxTarget::Ports`] for the rectangle with corners `a` and `b` (page nm, grid points): the first
/// interface's box is the rectangle (grown to fit its sockets), each next one as wide and as high, two grid steps to
/// the right of the one before; `None` for a rectangle without height.
pub fn port_layouts(
    grid: PageGridMap,
    a: (i64, i64),
    b: (i64, i64),
    interfaces: &[PortTarget],
) -> Option<Vec<BoxLayout>> {
    let width = (grid.index(b.0) - grid.index(a.0)).abs();
    let mut x = grid.index(a.0.min(b.0));
    let mut out = Vec::with_capacity(interfaces.len());
    for i in interfaces {
        let l = layout(
            grid,
            (grid.coord(x), a.1),
            (grid.coord(x + width), b.1),
            &i.sockets,
            &[],
        )?;
        let right = l.0.0 + l.1.get(1).map_or(0, |p| p.0);
        x = grid.index(right) + 2;
        out.push(l);
    }
    Some(out)
}

/// A box CP on the page: its function CP index, its point and its direction.
pub type BoxCpAt = (u16, (i64, i64), Dir);

/// The laid-out box: the insertion point (page nm), the outline relative to it and the CPs `(idx, relative point,
/// direction)`.
pub type BoxLayout = ((i64, i64), Vec<(i64, i64)>, Vec<(u16, (i64, i64), Dir)>);

/// The box with corners `a` and `b` (page nm, grid points) and the CPs `cps` laid out on its top and bottom edges:
/// with some but not all of them among `outputs` (a source's outputs) the others on top and those below, else the first
/// half on top; `None` for a rectangle without height, or without width when no CP widens it.
pub fn layout(
    grid: PageGridMap,
    a: (i64, i64),
    b: (i64, i64),
    cps: &[(u16, String)],
    outputs: &[u16],
) -> Option<BoxLayout> {
    let (ax, bx) = (grid.index(a.0), grid.index(b.0));
    let (ay, by) = (grid.index(a.1), grid.index(b.1));
    let (kx0, mut kx1) = (ax.min(bx), ax.max(bx));
    let (ky0, ky1) = (ay.min(by), ay.max(by));
    if ky0 == ky1 {
        return None;
    }
    let n = cps.len();
    let indices: Vec<u16> = cps.iter().map(|c| c.0).collect();
    let (cps, top) = ew_api::part_box::split(&indices, outputs);
    let bottom = n - top;
    if n > 0 {
        let need = i64::try_from(top.max(bottom)).unwrap_or(i64::MAX / 2) + 1;
        if kx1 - kx0 < need {
            kx1 = kx0 + need;
        }
    }
    if kx0 == kx1 {
        return None;
    }
    let pos = (grid.coord(kx0), grid.coord(ky0));
    let (w, h) = (grid.coord(kx1) - pos.0, grid.coord(ky1) - pos.1);
    let outline = vec![(0, 0), (w, 0), (w, h), (0, h)];
    let width = kx1 - kx0;
    let mut out = Vec::with_capacity(n);
    for (i, idx) in cps.iter().enumerate() {
        let (k, m, y, dir) = if i < top {
            (i, top, 0, Dir::Up)
        } else {
            (i - top, bottom, h, Dir::Down)
        };
        let k = i64::try_from(k).unwrap_or(0);
        let m = i64::try_from(m).unwrap_or(1);
        out.push((*idx, (grid.coord(kx0 + spread(width, m, k)) - pos.0, y), dir));
    }
    Some((pos, outline, out))
}

/// The least distance between two pins of a plug (nm): a text of style `data.small` stands above each pin's wire.
const PIN_PITCH: i64 = 5_000_000;

/// The least width of a plug's box (nm): the pin designations inside stand clear of each other.
const PIN_BOX_WIDTH: i64 = 15_000_000;

/// The box of a multipin valve terminal's plug with its top-left corner at the grid point `at` (rc.2): its pins down its
/// left side (the first half, top to bottom) and its right side (the rest), the wires leaving sideways so that each pin's
/// designation inside and its text outside read horizontally; the pins `PIN_PITCH` apart and the box `PIN_BOX_WIDTH`
/// wide, each rounded up to whole grid steps; the first pin one pitch lower than the others' spacing, below the box's
/// cross-reference, the box as high as its longer side. `None` without pins.
pub fn pin_layout(grid: PageGridMap, at: (i64, i64), cps: &[(u16, String)]) -> Option<BoxLayout> {
    if cps.is_empty() {
        return None;
    }
    let step = grid.step().max(1);
    let steps = |nm: i64| ((nm + step - 1) / step).max(1);
    let (pitch, width) = (steps(PIN_PITCH), steps(PIN_BOX_WIDTH));
    let left = cps.len().div_ceil(2);
    let rows = i64::try_from(left).ok()?;
    let (kx, ky) = (grid.index(at.0), grid.index(at.1));
    let pos = (grid.coord(kx), grid.coord(ky));
    let w = grid.coord(kx + width) - pos.0;
    let h = grid.coord(ky + (rows + 2) * pitch) - pos.1;
    let outline = vec![(0, 0), (w, 0), (w, h), (0, h)];
    let out = cps
        .iter()
        .enumerate()
        .map(|(i, (idx, _))| {
            let (row, x, dir) = if i < left {
                (i, 0, Dir::Left)
            } else {
                (i - left, w, Dir::Right)
            };
            let row = i64::try_from(row).unwrap_or(0);
            (*idx, (x, grid.coord(ky + (row + 2) * pitch) - pos.1), dir)
        })
        .collect();
    Some((pos, outline, out))
}

/// Whether the box `outline` at `pos` (page nm) overlaps a black box on the page shown.
fn overlaps_a_box(cx: &ToolCx<'_>, pos: (i64, i64), outline: &[(i64, i64)]) -> bool {
    let bbox = |pts: &mut dyn Iterator<Item = (i64, i64)>| {
        pts.fold(None, |acc: Option<[i64; 4]>, (x, y)| {
            Some(acc.map_or([x, y, x, y], |r| {
                [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)]
            }))
        })
    };
    let Some(a) = bbox(&mut outline.iter().map(|p| (pos.0 + p.0, pos.1 + p.1))) else {
        return false;
    };
    cx.page
        .tables
        .reps
        .iter()
        .filter_map(|r| r.black_box.as_ref())
        .any(|b| {
            bbox(&mut b.outline.iter().copied())
                .is_some_and(|o| a[0] < o[2] && o[0] < a[2] && a[1] < o[3] && o[1] < a[3])
        })
}

/// `pos` moved by whole grid steps so that the box `outline` at it stands inside the frame's drawing area `area` (page
/// nm, [`super::ip::drawing_area`]); unmoved on a page without a frame grid. `Err` with the box's and the area's width
/// and height (nm) when the box is wider or higher than the area.
fn fit_in_area(
    area: Option<[i64; 4]>,
    grid: PageGridMap,
    pos: (i64, i64),
    outline: &[(i64, i64)],
) -> Result<(i64, i64), [i64; 4]> {
    let (Some(area), Some(&first)) = (area, outline.first()) else {
        return Ok(pos);
    };
    let (lo, hi) = outline.iter().fold((first, first), |(lo, hi), p| {
        ((lo.0.min(p.0), lo.1.min(p.1)), (hi.0.max(p.0), hi.1.max(p.1)))
    });
    let (w, h) = (hi.0 - lo.0, hi.1 - lo.1);
    let (aw, ah) = (area[2] - area[0], area[3] - area[1]);
    if w > aw || h > ah {
        return Err([w, h, aw, ah]);
    }
    // The coordinate `p` moved by whole grid steps until the box's edges `lo`, `hi` (relative to it) stand inside
    // `min … max`.
    let inward = |p: i64, lo: i64, hi: i64, min: i64, max: i64| -> i64 {
        let base = grid.index(p);
        let mut k = 0;
        while p + lo < min && grid.coord(base + k) + lo < min && k < 100_000 {
            k += 1;
        }
        while p + hi > max && grid.coord(base + k) + hi > max && k > -100_000 {
            k -= 1;
        }
        if k == 0 { p } else { grid.coord(base + k) }
    };
    Ok((
        inward(pos.0, lo.0, hi.0, area[0], area[2]),
        inward(pos.1, lo.1, hi.1, area[1], area[3]),
    ))
}

use ew_api::part_box::spread;

/// The JSON of a box CP (`{idx, pos, dir}`).
fn cp_json(idx: u16, pos: (i64, i64), dir: Dir) -> Value {
    json!({"idx": idx, "pos": [pos.0, pos.1], "dir": dir.word()})
}

/// The tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    Armed { at: Option<(i64, i64)> },
    Outline { first: (i64, i64), at: (i64, i64) },
}

/// The black box tool.
#[derive(Clone, Debug)]
pub struct BlackBoxTool {
    /// The black-box symbol copy's ID text.
    symbol: String,
    target: BoxTarget,
    state: State,
    /// Whether an Escape just dropped the first corner: the hint then says that a second Escape leaves the tool;
    /// the next click clears it.
    dropped: bool,
    /// The library documents the black box embeds (`box.black`'s `copies`, FT-656); the host sends only those the
    /// project still lacks.
    copies: Vec<Value>,
    /// The representation type the box takes where it is not the frame's own (`topology` on a bus-topology page).
    rep_type: Option<String>,
    /// The positions of a valve terminal drawn as its positions: one click places its box of that many cells.
    slots: Option<u16>,
    /// A multipin valve terminal's plug: one click places its box sized by its pins, down its two sides.
    pins: bool,
}

impl BlackBoxTool {
    /// The tool, armed, drawing `target` with the black-box symbol copy `symbol` (its ID text).
    pub fn new(symbol: String, target: BoxTarget) -> BlackBoxTool {
        BlackBoxTool {
            symbol,
            target,
            state: State::Armed { at: None },
            dropped: false,
            copies: Vec::new(),
            rep_type: None,
            slots: None,
            pins: false,
        }
    }

    /// The tool placing a valve terminal of `count` positions drawn as its positions (rc.2): one click, no outline.
    pub fn with_slots(mut self, count: Option<u16>) -> BlackBoxTool {
        self.slots = count.filter(|n| *n > 0);
        self
    }

    /// The tool placing a multipin valve terminal's plug (rc.2): one click, the box sized by its pins ([`pin_layout`]).
    pub fn with_pins(mut self, pins: bool) -> BlackBoxTool {
        self.pins = pins;
        self
    }

    /// The device box of a part target that lays one out.
    fn part_layout(&self) -> Option<&ew_api::part_box::PartBox> {
        match &self.target {
            BoxTarget::Part { layout, .. } => layout.as_ref(),
            _ => None,
        }
    }

    /// Whether one click places the box, sized by what it draws (a terminal's positions, a plug's pins, a part's
    /// device box); the host then ends the tool.
    pub fn one_click(&self) -> bool {
        self.slots.is_some() || self.pins || self.part_layout().is_some()
    }

    /// The box one click places at the armed point: its layout, and the cell width of a terminal drawn as its positions.
    /// A box that would leave the frame's drawing area is moved inside it by whole grid steps where it fits (the rc.2
    /// re-test: a terminal ran past the frame's edge, its positions cut off), and laid out afresh there: on a grid whose
    /// step is no whole number of nanometres (the QET grid) the same layout at another corner has its points a
    /// nanometre apart, and a moved one would leave its pins off the grid.
    fn one_click_box(&self, cx: &ToolCx<'_>) -> Option<(BoxLayout, Option<i64>)> {
        let State::Armed { at: Some(at) } = &self.state else {
            return None;
        };
        let grid = PageGridMap::new(cx.page.tables.grid);
        let ((pos, outline, cps), pitch) = self.box_at(cx, grid, *at)?;
        match fit_in_area(super::ip::drawing_area(cx.page), grid, pos, &outline) {
            Ok(inside) if inside != pos => self.box_at(cx, grid, inside),
            _ => Some(((pos, outline, cps), pitch)),
        }
    }

    /// The box one click places with its top-left corner at the grid point nearest `at`, and the cell width of a
    /// terminal drawn as its positions.
    fn box_at(&self, cx: &ToolCx<'_>, grid: PageGridMap, at: (i64, i64)) -> Option<(BoxLayout, Option<i64>)> {
        Some(match self.slots {
            Some(n) => {
                let (pos, outline, pitch) = ew_api::geometry::slot_box(cx.page.tables.grid, at, n)?;
                ((pos, outline, Vec::new()), Some(pitch))
            }
            None if self.pins => (pin_layout(grid, at, self.cps())?, None),
            // A part with a device box: the box as the part lays it out, its top-left corner at the pointer.
            None => {
                use ew_api::part_box::Dir4;
                let layout = self.part_layout()?;
                let pins: Vec<u16> = self.cps().iter().map(|c| c.0).collect();
                let (pos, outline, cps) =
                    ew_api::part_box::steps(&pins, self.outputs(), Some(layout), grid.step().max(1))
                        .on_grid(cx.page.tables.grid, at)?;
                let cps = cps
                    .into_iter()
                    .map(|(idx, p, dir)| {
                        let dir = match dir {
                            Dir4::Up => Dir::Up,
                            Dir4::Down => Dir::Down,
                            Dir4::Left => Dir::Left,
                            _ => Dir::Right,
                        };
                        (idx, p, dir)
                    })
                    .collect();
                ((pos, outline, cps), None)
            }
        })
    }

    /// The tool drawing boxes of representation type `rep_type` (`topology` on a bus-topology page).
    pub fn with_rep_type(mut self, rep_type: Option<String>) -> BlackBoxTool {
        self.rep_type = rep_type;
        self
    }

    /// The tool with the library documents its black boxes embed (FT-656).
    pub fn with_copies(mut self, copies: Vec<Value>) -> BlackBoxTool {
        self.copies = copies;
        self
    }

    /// The armed target.
    pub fn target(&self) -> &BoxTarget {
        &self.target
    }

    /// The records of the project the box names when it is put down (ID texts): the symbol copy unless the box
    /// embeds it, and the function or device it draws. An undo or a redo that takes one away ends the tool (rc.5, H2).
    pub fn records(&self) -> Vec<&str> {
        let embedded = self.copies.iter().any(|c| c["id"].as_str() == Some(&self.symbol));
        let mut out: Vec<&str> = (!embedded).then_some(self.symbol.as_str()).into_iter().collect();
        match &self.target {
            BoxTarget::Function { function, .. } => out.push(function),
            BoxTarget::Device { device, .. } => out.push(device),
            BoxTarget::Station { station, device, .. } => out.extend([station.as_str(), device.as_str()]),
            BoxTarget::NewDevice { .. } | BoxTarget::Part { .. } | BoxTarget::Ports { .. } => {}
        }
        out
    }

    fn cps(&self) -> &[(u16, String)] {
        match &self.target {
            BoxTarget::Part { cps, .. }
            | BoxTarget::Function { cps, .. }
            | BoxTarget::Station { cps, .. } => cps,
            BoxTarget::Ports { interfaces, .. } => interfaces.first().map_or(&[], |i| i.sockets.as_slice()),
            BoxTarget::NewDevice { .. } | BoxTarget::Device { .. } => &[],
        }
    }

    /// The target's source outputs among its CPs (FT-1702).
    fn outputs(&self) -> &[u16] {
        match &self.target {
            BoxTarget::Part { outputs, .. } => outputs,
            BoxTarget::NewDevice { .. }
            | BoxTarget::Ports { .. }
            | BoxTarget::Function { .. }
            | BoxTarget::Station { .. }
            | BoxTarget::Device { .. } => &[],
        }
    }

    /// The target's label text (`new F`, `part MICO 4.10`).
    fn label(&self) -> String {
        match &self.target {
            BoxTarget::NewDevice { letter } => crate::strings::format(key!("place-target-new"), &[letter]),
            BoxTarget::Part { number, .. } => crate::strings::format(key!("place-target-part"), &[number]),
            BoxTarget::Ports { label, .. } | BoxTarget::Station { label, .. } => label.clone(),
            BoxTarget::Function { .. } | BoxTarget::Device { .. } => {
                crate::strings::text(key!("place-target-function")).to_owned()
            }
        }
    }

    /// The layouts of the current rectangle: its box, or a box per interface of a ports target.
    fn current(&self, cx: &ToolCx<'_>) -> Option<Vec<BoxLayout>> {
        let State::Outline { first, at } = self.state else {
            return None;
        };
        let grid = PageGridMap::new(cx.page.tables.grid);
        match &self.target {
            BoxTarget::Ports { interfaces, .. } => port_layouts(grid, first, at, interfaces),
            _ => layout(grid, first, at, self.cps(), self.outputs()).map(|l| vec![l]),
        }
    }

    /// The `box.black` of the current rectangle, or the `bus.place_ports` of a ports target.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        let one = self.one_click_box(cx);
        let layouts = match &one {
            Some((layout, _)) => vec![layout.clone()],
            None => self.current(cx)?,
        };
        if let BoxTarget::Ports {
            target, interfaces, ..
        } = &self.target
        {
            let boxes: Vec<Value> = layouts
                .iter()
                .zip(interfaces)
                .map(|((pos, outline, cps), i)| {
                    let mut b = json!({
                        "interface": i.interface,
                        "fndef": i.fndef,
                        "pos": [pos.0, pos.1],
                        "box": {
                            "outline": outline.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
                            "cps": cps.iter().map(|(c, p, d)| cp_json(*c, *p, *d)).collect::<Vec<_>>(),
                        },
                    });
                    if let Some(l) = &i.port_label {
                        b["port_label"] = Value::String(l.clone());
                    }
                    b
                })
                .collect();
            let mut params = json!({
                "page": cx.page.tables.page.to_string(),
                "symbol": self.symbol,
                "target": target,
                "boxes": boxes,
            });
            if !self.copies.is_empty() {
                params["copies"] = Value::Array(self.copies.clone());
            }
            return Some(Box::new(AnyCommand {
                name: "bus.place_ports".into(),
                params,
            }));
        }
        let (pos, outline, cps) = layouts.into_iter().next()?;
        let target = match &self.target {
            BoxTarget::NewDevice { letter } => json!({"new_device": {"letter": letter}}),
            BoxTarget::Part {
                part,
                template,
                letter,
                ..
            } => json!({"from_part": {"part": part, "template": template, "letter": letter}}),
            BoxTarget::Function { function, .. } => json!({"existing_function": {"function": function}}),
            BoxTarget::Device { device, main } => {
                let role = if *main { "main" } else { "aux" };
                json!({"join_device": {"device": device, "role": role}})
            }
            BoxTarget::Ports { .. } => return None,
            BoxTarget::Station { .. } => Value::Null,
        };
        let mut b = json!({
            "outline": outline.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
        });
        if let Some((_, Some(pitch))) = one {
            b["slots"] = json!({ "pitch": pitch });
        }
        if !cps.is_empty() {
            b["cps"] = Value::Array(cps.iter().map(|(i, p, d)| cp_json(*i, *p, *d)).collect());
        }
        // A station's device: the station draws it, with or without its functions made yet.
        if let BoxTarget::Station { station, device, .. } = &self.target {
            let mut params = json!({
                "station": station,
                "page": cx.page.tables.page.to_string(),
                "symbol": self.symbol,
                "boxes": [{ "device": device, "pos": [pos.0, pos.1], "box": b }],
            });
            if !self.copies.is_empty() {
                params["copies"] = Value::Array(self.copies.clone());
            }
            return Some(Box::new(AnyCommand {
                name: "plc.draw_station".into(),
                params,
            }));
        }
        let mut params = json!({
            "page": cx.page.tables.page.to_string(),
            "symbol": self.symbol,
            "pos": [pos.0, pos.1],
            "box": b,
            "target": target,
        });
        if !self.copies.is_empty() {
            params["copies"] = Value::Array(self.copies.clone());
        }
        if let Some(t) = &self.rep_type {
            params["rep_type"] = Value::String(t.clone());
        }
        Some(Box::new(AnyCommand {
            name: "box.black".into(),
            params,
        }))
    }

    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        if let Some(((pos, outline, cps), _)) = self.one_click_box(cx) {
            let mut pts: Vec<(i64, i64)> = outline
                .iter()
                .map(|p| cx.snapper.page_to_sheet((pos.0 + p.0, pos.1 + p.1)))
                .collect();
            if let Some(&first) = pts.first() {
                pts.push(first);
            }
            let mut items = vec![OverlayItem::Polyline {
                points: pts,
                style: Style::dashed(Token::Ghost),
            }];
            for (_, p, _) in &cps {
                items.push(OverlayItem::Marker {
                    at: cx.snapper.page_to_sheet((pos.0 + p.0, pos.1 + p.1)),
                    kind: MarkerKind::Cp,
                });
            }
            // The label at the pointer says what a click does (the persona run took it for a tool of two corners).
            items.push(OverlayItem::Label {
                at: cx.snapper.page_to_sheet(pos),
                text: crate::strings::format(key!("place-target-click"), &[&self.label()]),
                anchor: Anchor::Right,
            });
            return Overlay::new(items);
        }
        match self.state {
            State::Armed { at: None } => Overlay::default(),
            // A chip at the pointer says how the box is drawn, where the person looks (rc.2: a part placed as a black
            // box asked for two corners in the bottom bar only).
            State::Armed { at: Some(at) } => Overlay::new(vec![
                OverlayItem::Marker {
                    at: cx.snapper.page_to_sheet(at),
                    kind: MarkerKind::Grid,
                },
                OverlayItem::Label {
                    at: cx.snapper.page_to_sheet(at),
                    text: crate::strings::text(key!("box-chip-armed")).to_owned(),
                    anchor: Anchor::Right,
                },
            ]),
            State::Outline { first, at } => {
                let mut items = Vec::new();
                let current = self.current(cx);
                // While the rectangle has no area the first corner is marked, so the click visibly registered
                // (the tester saw nothing after the first click).
                // A ring around the cross, so the fixed corner stands out from the pointer's grid marker at
                // 100 % zoom (the 7-pixel cross alone was missed).
                if current.is_none() {
                    let corner = cx.snapper.page_to_sheet(first);
                    items.push(OverlayItem::Marker {
                        at: corner,
                        kind: MarkerKind::Grid,
                    });
                    items.push(OverlayItem::Marker {
                        at: corner,
                        kind: MarkerKind::Cp,
                    });
                }
                for (pos, outline, cps) in current.into_iter().flatten() {
                    let mut pts: Vec<(i64, i64)> = outline
                        .iter()
                        .map(|p| cx.snapper.page_to_sheet((pos.0 + p.0, pos.1 + p.1)))
                        .collect();
                    if let Some(&first) = pts.first() {
                        pts.push(first);
                    }
                    items.push(OverlayItem::Polyline {
                        points: pts,
                        style: Style::dashed(Token::Ghost),
                    });
                    for (_, p, _) in &cps {
                        items.push(OverlayItem::Marker {
                            at: cx.snapper.page_to_sheet((pos.0 + p.0, pos.1 + p.1)),
                            kind: MarkerKind::Cp,
                        });
                    }
                }
                items.push(OverlayItem::Label {
                    at: cx.snapper.page_to_sheet(at),
                    text: self.label(),
                    anchor: Anchor::Right,
                });
                Overlay::new(items)
            }
        }
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let p = cx.snapper.snap_grid(at).page_nm;
        self.state = match self.state {
            State::Armed { .. } => State::Armed { at: Some(p) },
            State::Outline { first, .. } => State::Outline { first, at: p },
        };
        ToolOutput::Transient(self.overlay(cx))
    }

    fn click(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        self.dropped = false;
        if self.one_click() {
            if let Some(((pos, outline, _), _)) = self.one_click_box(cx) {
                // A box larger than the drawing area says so, with its size (the rc.2 re-test).
                let (area, grid) = (
                    super::ip::drawing_area(cx.page),
                    PageGridMap::new(cx.page.tables.grid),
                );
                if let Err(sizes) = fit_in_area(area, grid, pos, &outline) {
                    let mm = |nm: i64| ((nm + 500_000) / 1_000_000).to_string();
                    let [w, h, aw, ah] = sizes.map(mm);
                    let words = crate::strings::format(key!("reason-box-too-large"), &[&w, &h, &aw, &ah]);
                    return ToolOutput::RefusedWith(key!("reason-box-too-large"), words);
                }
                // A box never lands on another box (the persona run placed two terminals on one spot).
                if overlaps_a_box(cx, pos, &outline) {
                    return ToolOutput::Refused(key!("reason-box-occupied"));
                }
            }
            return match self.command(cx) {
                Some(cmd) => ToolOutput::Commit(cmd),
                None => ToolOutput::None,
            };
        }
        match self.state {
            State::Armed { at: None } => ToolOutput::None,
            State::Armed { at: Some(at) } => {
                self.state = State::Outline { first: at, at };
                ToolOutput::Transient(self.overlay(cx))
            }
            State::Outline { at, .. } => match self.command(cx) {
                Some(cmd) => {
                    self.state = State::Armed { at: Some(at) };
                    ToolOutput::Commit(cmd)
                }
                None => ToolOutput::None,
            },
        }
    }
}

impl Tool for BlackBoxTool {
    fn name(&self) -> &'static str {
        "black_box"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "armed",
            State::Outline { .. } => "outline",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, .. } | CanvasEvent::CursorMove { at } => self.moved(*at, cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                ..
            } => {
                let _ = self.moved(*at, cx);
                self.click(cx)
            }
            CanvasEvent::Activate => self.click(cx),
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command(cx) {
                Some(cmd) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Escape => match self.state {
                State::Outline { at, .. } => {
                    self.state = State::Armed { at: Some(at) };
                    self.dropped = true;
                    ToolOutput::Cancel
                }
                State::Armed { .. } => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> super::Hint {
        match self.state {
            State::Armed { .. } if self.dropped => {
                super::Hint::new(key!("hint-box-dropped"), &[super::ENTER, super::ESCAPE])
            }
            State::Armed { .. } if self.one_click() => {
                super::Hint::new(key!("hint-box-place"), &[super::ESCAPE])
            }
            State::Armed { .. } => super::Hint::new(key!("hint-box-armed"), &[super::ESCAPE]),
            State::Outline { .. } => {
                super::Hint::new(key!("hint-box-outline"), &[super::ENTER, super::ESCAPE])
            }
        }
    }

    fn ghost(&self) -> Option<Ghost> {
        let outline = match self.state {
            State::Outline { first, at } => {
                // The rectangle's four vertices, or none while it has no area (no height, or no width and no CP to
                // widen it; the corners are grid points, so equal coordinates are equal grid indices).
                if first.1 == at.1 || (first.0 == at.0 && self.cps().is_empty()) {
                    0
                } else {
                    4
                }
            }
            State::Armed { .. } => 0,
        };
        let cps = match &self.target {
            BoxTarget::Ports { interfaces, .. } => interfaces.iter().map(|i| i.sockets.len()).sum(),
            _ => self.cps().len(),
        };
        Some(Ghost::Box { outline, cps })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

// -------------------------------------------------------------------------------------------------------------------
// Grips

/// The inverse of orientation `o` (`0..=7`): the quarter turns swap, the mirrored ones are their own inverse.
pub fn orient_inverse(o: u8) -> u8 {
    match o % 8 {
        1 => 3,
        3 => 1,
        other => other,
    }
}

/// The grip points of an outline (page nm): the vertices, then the midpoints of the edges `v0v1, …, v(n−1)v0`.
pub fn grip_points(outline: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let n = outline.len();
    let mut out = outline.to_vec();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        out.push((a.0 + (b.0 - a.0) / 2, a.1 + (b.1 - a.1) / 2));
    }
    out
}

/// Whether an outline is an axis-aligned rectangle as the black box tool stores it (four vertices, `v0v1` and
/// `v2v3` horizontal, `v1v2` and `v3v0` vertical).
pub fn is_rectangle(o: &[(i64, i64)]) -> bool {
    o.len() == 4 && o[0].1 == o[1].1 && o[1].0 == o[2].0 && o[2].1 == o[3].1 && o[3].0 == o[0].0
}

/// Twice the signed area of a polygon (positive: clockwise on the sheet, whose y axis points down).
pub fn signed_area2(poly: &[(i64, i64)]) -> i128 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            i128::from(a.0) * i128::from(b.1) - i128::from(b.0) * i128::from(a.1)
        })
        .sum()
}

/// The outline after grip `grip` moved to `to` (page nm, absolute). Errors: `reason-grip-not-axis` for an edge grip
/// of an edge that is neither horizontal nor vertical; `reason-box-too-small` for an outline without area;
/// `reason-no-grip` for a grip the outline does not have.
pub fn reshape_by_grip(
    outline: &[(i64, i64)],
    grip: usize,
    to: (i64, i64),
) -> Result<Vec<(i64, i64)>, Key<'static>> {
    let n = outline.len();
    if n < 3 || grip >= 2 * n {
        return Err(key!("reason-no-grip"));
    }
    let mut out = outline.to_vec();
    if grip < n {
        let i = grip;
        if is_rectangle(outline) {
            for j in [(i + n - 1) % n, (i + 1) % n] {
                if outline[j].0 == outline[i].0 {
                    out[j].0 = to.0;
                }
                if outline[j].1 == outline[i].1 {
                    out[j].1 = to.1;
                }
            }
        }
        out[i] = to;
    } else {
        let i = grip - n;
        let j = (i + 1) % n;
        let (a, b) = (outline[i], outline[j]);
        if a.1 == b.1 && a.0 != b.0 {
            out[i].1 = to.1;
            out[j].1 = to.1;
        } else if a.0 == b.0 && a.1 != b.1 {
            out[i].0 = to.0;
            out[j].0 = to.0;
        } else {
            return Err(key!("reason-grip-not-axis"));
        }
    }
    let (before, after) = (signed_area2(outline), signed_area2(&out));
    if after == 0 {
        return Err(key!("reason-box-too-small"));
    }
    // A grip never turns the box inside out (its CPs would then point inwards).
    if before.signum() != after.signum() {
        return Err(key!("reason-box-inside-out"));
    }
    Ok(out)
}

/// Whether the pins `cps` of the box `outline` (page nm) still fit when its outline becomes `new` (the same vertices
/// moved), on the edges along x (`along_x`) or along y: an axis-aligned edge whose extent along its axis changed holds
/// its pins one grid step apart and one step to spare at each corner, as `box.reshape` re-centres them. A pin belongs to
/// the edge it lies on, at a corner to the one it leaves by
/// its direction.
fn pins_fit(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    cps: &[BoxCpAt],
    new: &[(i64, i64)],
    along_x: bool,
) -> bool {
    let n = outline.len();
    let area = signed_area2(outline);
    (0..n).all(|i| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let (na, nb) = (new[i], new[(i + 1) % n]);
        let horizontal = na.1 == nb.1 && na.0 != nb.0;
        let axis = |p: (i64, i64)| if horizontal { p.0 } else { p.1 };
        if horizontal != along_x || (na.0 != nb.0 && na.1 != nb.1) {
            return true;
        }
        if (axis(a).min(axis(b)), axis(a).max(axis(b))) == (axis(na).min(axis(nb)), axis(na).max(axis(nb))) {
            return true;
        }
        let mut at: Vec<i64> = cps
            .iter()
            .filter(|(_, p, dir)| {
                let edge = |j: usize| on_outline(&[outline[j], outline[(j + 1) % n]], *p);
                let leaves = |j: usize| outward(outline[j], outline[(j + 1) % n], area) == Some(*dir);
                let mine = (0..n).filter(|j| edge(*j)).collect::<Vec<_>>();
                mine.iter().find(|j| leaves(**j)).or(mine.first()) == Some(&i)
            })
            .map(|(_, p, _)| axis(*p))
            .collect();
        at.sort_unstable();
        at.dedup();
        let steps = (grid.index(axis(nb)) - grid.index(axis(na))).abs();
        let n = i64::try_from(at.len()).unwrap_or(i64::MAX / 4);
        at.is_empty() || steps > n
    })
}

/// Where a grip stops and the outline there ([`grip_stop`]).
pub type GripStop = ((i64, i64), Vec<(i64, i64)>);

/// Where grip `grip` of the box `outline` with the pins `cps` stops when dragged to the grid point `to` (page nm), and
/// the outline there: at `to`, else, where the pins would not fit ([`pins_fit`]) or the box would be lower than `depth`
/// grid steps while it gets lower (the pin texts of its top and bottom sides would touch, rc.5), as near to `to` as
/// they fit, moved back towards the grip's own point a grid step at a time on each axis that is too short. Errors as
/// [`reshape_by_grip`].
pub fn grip_stop(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    cps: &[BoxCpAt],
    depth: i64,
    grip: usize,
    to: (i64, i64),
) -> Result<GripStop, Key<'static>> {
    let height = |o: &[(i64, i64)]| {
        let ys = o.iter().map(|p| p.1);
        ys.clone().max().zip(ys.min()).map_or(0, |(a, b)| a - b)
    };
    let low =
        |new: &[(i64, i64)]| height(new) < height(outline) && height(new) < depth.saturating_mul(grid.step());
    let home = grip_points(outline).get(grip).copied().unwrap_or(to);
    let mut at = to;
    let mut new = reshape_by_grip(outline, grip, at)?;
    for along_x in [true, false] {
        let toward = |v: i64, h: i64| {
            let k = grid.index(v);
            grid.coord(if h > v { k + 1 } else { k - 1 })
        };
        while !pins_fit(grid, outline, cps, &new, along_x) || (!along_x && low(&new)) {
            let (v, h) = if along_x { (at.0, home.0) } else { (at.1, home.1) };
            if v == h {
                break;
            }
            let next = toward(v, h);
            // Never past the grip's own point.
            let next = if (h - v).abs() < (next - v).abs() { h } else { next };
            if along_x {
                at.0 = next;
            } else {
                at.1 = next;
            }
            new = reshape_by_grip(outline, grip, at)?;
        }
    }
    Ok((at, new))
}

/// The outward normal of edge `a`–`b` of a polygon with the signed area `area2`; `None` for an edge that is not
/// axis-aligned.
pub fn outward(a: (i64, i64), b: (i64, i64), area2: i128) -> Option<Dir> {
    let v = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
    if (v.0 != 0 && v.1 != 0) || v == (0, 0) || area2 == 0 {
        return None;
    }
    let n = if area2 > 0 { (v.1, -v.0) } else { (-v.1, v.0) };
    Dir::of_vector(n)
}

/// The grid point on the outline (page nm) nearest to `p`, with the edge's outward normal: per axis-aligned edge the
/// point projected, clamped and snapped along it, a grid step clear of both corners where the edge is long enough (a
/// pin never stands on a corner, rc.5); ties by the smaller edge index. `None` without such an edge.
pub fn nearest_on_outline(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    p: (i64, i64),
) -> Option<((i64, i64), Dir)> {
    let n = outline.len();
    let area = signed_area2(outline);
    let snap_between = |t: i64, a: i64, b: i64| {
        let (lo, hi) = (a.min(b), a.max(b));
        let (inner_lo, inner_hi) = (grid.coord(grid.index(lo) + 1), grid.coord(grid.index(hi) - 1));
        let (lo, hi) = if inner_lo <= inner_hi && lo < inner_lo && inner_hi < hi {
            (inner_lo, inner_hi)
        } else {
            (lo, hi)
        };
        let t = t.clamp(lo, hi);
        let k = grid.index(t);
        [k - 1, k, k + 1]
            .into_iter()
            .map(|k| grid.coord(k))
            .filter(|c| (lo..=hi).contains(c))
            .min_by_key(|c| ((c - t).abs(), (c - a).abs()))
            .unwrap_or(a)
    };
    let mut best: Option<(i128, usize, (i64, i64), Dir)> = None;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let q = if a.1 == b.1 && a.0 != b.0 {
            (snap_between(p.0, a.0, b.0), a.1)
        } else if a.0 == b.0 && a.1 != b.1 {
            (a.0, snap_between(p.1, a.1, b.1))
        } else {
            continue;
        };
        let Some(dir) = outward(a, b, area) else {
            continue;
        };
        let d = super::dist2(q, p);
        if best.is_none_or(|(bd, bi, ..)| d < bd || (d == bd && i < bi)) {
            best = Some((d, i, q, dir));
        }
    }
    best.map(|(_, _, q, d)| (q, d))
}

/// A pin's point on the page and the direction a wire leaves it.
type PinAt = ((i64, i64), Dir);

/// Where a drag of pin `idx` of the box `outline` with the pins `cps` (page nm) to `p` puts it, with the direction it
/// leaves in (rc.5: a pin dragged on the smallest box went round its corner onto the next side): a grid point at least
/// one step from both corners of an axis-aligned edge, no other pin's (read settled), nearest `p`. The pin keeps its
/// own side while `p` lies across from it (between that side's corners, them included); beyond them, the nearest side
/// that has a free point. A side too short for a point clear of its corners takes none. `None` where no side has a free point.
pub fn pin_target(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    cps: &[BoxCpAt],
    idx: u16,
    p: (i64, i64),
) -> Option<((i64, i64), Dir)> {
    let n = outline.len();
    let area = signed_area2(outline);
    let settle = |v: i64| grid.coord(grid.index(v));
    let taken = |q: (i64, i64)| {
        cps.iter()
            .any(|c| c.0 != idx && settle(c.1.0) == settle(q.0) && settle(c.1.1) == settle(q.1))
    };
    let me = cps.iter().find(|c| c.0 == idx).map(|c| c.1);
    // Each axis-aligned edge: its free points clear of the corners, and whether it holds the pin now.
    let mut best: Option<(bool, i128, usize, PinAt)> = None;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let along_x = a.1 == b.1 && a.0 != b.0;
        if !along_x && !(a.0 == b.0 && a.1 != b.1) {
            continue;
        }
        let Some(dir) = outward(a, b, area) else {
            continue;
        };
        let (lo, hi) = if along_x {
            (a.0.min(b.0), a.0.max(b.0))
        } else {
            (a.1.min(b.1), a.1.max(b.1))
        };
        let (k_lo, k_hi) = (grid.index(lo) + 1, grid.index(hi) - 1);
        if k_lo > k_hi {
            continue;
        }
        let point = |k: i64| {
            if along_x {
                (grid.coord(k), a.1)
            } else {
                (a.0, grid.coord(k))
            }
        };
        let own = me.is_some_and(|m| on_outline(&[a, b], m));
        let across = if along_x { p.0 } else { p.1 };
        let k = grid.index(across.clamp(grid.coord(k_lo), grid.coord(k_hi)));
        // The free point nearest the pointer along the edge: outward from the nearest grid point.
        let hit = (0..=(k_hi - k_lo))
            .flat_map(|d| [k - d, k + d])
            .filter(|k| (k_lo..=k_hi).contains(k))
            .map(point)
            .find(|q| !taken(*q));
        let Some(q) = hit else { continue };
        // The pin's own side wins while the pointer lies across from it, its corners included (the pointer snaps to
        // the grid, so a pointer just short of a corner reads as on it).
        let keep = own && (lo..=hi).contains(&across);
        let d = super::dist2(q, p);
        let better = match &best {
            None => true,
            Some((bk, bd, bi, ..)) => (keep, -d, std::cmp::Reverse(i)) > (*bk, -*bd, std::cmp::Reverse(*bi)),
        };
        if better {
            best = Some((keep, d, i, (q, dir)));
        }
    }
    best.map(|(.., at)| at)
}

/// The new pin a wire ending at the grid point `p` on the box `b` would take (rc.5): on a box whose pins are its own,
/// `p` on an axis-aligned edge, no corner and no pin's point (read settled); the smallest index the box does not use,
/// leaving along the edge's outward normal. `None` anywhere else.
pub fn free_edge_pin(grid: PageGridMap, b: &ew_api::output::TableBox, p: (i64, i64)) -> Option<BoxCpAt> {
    if !b.own_pins {
        return None;
    }
    let settle = |q: (i64, i64)| (grid.index(q.0), grid.index(q.1));
    if b.outline.iter().any(|v| settle(*v) == settle(p)) || b.cps.iter().any(|c| settle(c.1) == settle(p)) {
        return None;
    }
    let n = b.outline.len();
    let area = signed_area2(&b.outline);
    let dir = (0..n).find_map(|i| {
        let (a, c) = (b.outline[i], b.outline[(i + 1) % n]);
        on_outline(&[a, c], p).then(|| outward(a, c, area)).flatten()
    })?;
    let idx = (0..=u16::MAX).find(|i| !b.cps.iter().any(|c| c.0 == *i))?;
    Some((idx, p, dir))
}

/// Whether `p` lies on an edge of the closed outline (exact).
pub fn on_outline(outline: &[(i64, i64)], p: (i64, i64)) -> bool {
    let n = outline.len();
    (0..n).any(|i| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let cross =
            i128::from(b.0 - a.0) * i128::from(p.1 - a.1) - i128::from(b.1 - a.1) * i128::from(p.0 - a.0);
        cross == 0 && p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0) && p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1)
    })
}

/// The `box.reshape` of black box `rep` with the absolute outline `outline` and, when given, the absolute CPs
/// `(idx, point, direction)` (page nm), converted to the representation's frame (relative to its position, under the
/// inverse of its orientation).
pub fn reshape_command(rep: &TableRep, outline: &[(i64, i64)], cps: Option<&[BoxCpAt]>) -> Box<AnyCommand> {
    let names = rep
        .black_box
        .as_ref()
        .map(|b| b.names.clone())
        .unwrap_or_default();
    reshape_named(rep, outline, cps, &names)
}

/// [`reshape_command`] with the box's own pins named by `names` (function CP index, name; rc.5): a pin moved, added,
/// renamed or kept keeps the name it has there.
pub fn reshape_named(
    rep: &TableRep,
    outline: &[(i64, i64)],
    cps: Option<&[BoxCpAt]>,
    names: &[(u16, String)],
) -> Box<AnyCommand> {
    let inv = orient_inverse(rep.orient);
    let rel = |p: (i64, i64)| orient_point(inv, (p.0 - rep.pos.0, p.1 - rep.pos.1));
    let mut params = json!({
        "rep": rep.id.to_string(),
        "outline": outline.iter().map(|p| { let r = rel(*p); vec![r.0, r.1] }).collect::<Vec<_>>(),
    });
    if let Some(cps) = cps {
        params["cps"] = Value::Array(
            cps.iter()
                .map(|(i, p, d)| {
                    let mut cp = cp_json(*i, rel(*p), orient_dir(inv, *d));
                    if let Some((_, name)) = names.iter().find(|(j, _)| j == i) {
                        cp["name"] = Value::from(name.as_str());
                    }
                    cp
                })
                .collect(),
        );
    }
    Box::new(AnyCommand {
        name: "box.reshape".into(),
        params,
    })
}

/// The black box `e` of the page's tables, when it is one.
pub fn black_box_rep<'t>(cx: &'t ToolCx<'_>, e: EntityRef) -> Option<&'t TableRep> {
    cx.page
        .tables
        .reps
        .iter()
        .find(|r| r.id == e.0 && r.black_box.is_some())
}

/// The selection's one black box, when exactly one entity is selected and it is a black box.
pub fn selected_black_box<'t>(cx: &'t ToolCx<'_>) -> Option<&'t TableRep> {
    match cx.selection.items.as_slice() {
        [one] => black_box_rep(cx, *one),
        _ => None,
    }
}

/// How near a grip of a selected box a press takes it, in screen points (rc.5: a drag that missed the small handle
/// moved the whole box): the round handle the wire's grips have, and a margin.
pub const GRIP_SLOP_PT: f64 = 7.0;

/// What a press on a selected box takes ([`handle_at`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoxHandle {
    /// Grip `k` ([`grip_points`]).
    Grip(usize),
    /// The pin of this function CP index.
    Pin(u16),
}

/// The handle of the selected box `rep` under the sheet point `at`: a corner grip within [`GRIP_SLOP_PT`], else a pin
/// within [`crate::canvas::HOVER_SLOP_PT`], else an edge's middle grip within [`GRIP_SLOP_PT`]; the nearest of each
/// kind, ties by index.
pub fn handle_at(cx: &ToolCx<'_>, rep: &TableRep, at: (i64, i64)) -> Option<BoxHandle> {
    let b = rep.black_box.as_ref()?;
    let n = b.outline.len();
    let near = |p: (i64, i64), slop: f64| {
        let s = cx.snapper.page_to_sheet(p);
        cx.within(s, at, slop).then(|| super::dist2(s, at))
    };
    let grips = grip_points(&b.outline);
    let best = |range: std::ops::Range<usize>| {
        range
            .filter_map(|k| near(grips[k], GRIP_SLOP_PT).map(|d| (d, k)))
            .min()
            .map(|(_, k)| k)
    };
    if let Some(k) = best(0..n) {
        return Some(BoxHandle::Grip(k));
    }
    if let Some((_, idx)) = b
        .cps
        .iter()
        .filter_map(|(idx, p, _)| near(*p, crate::canvas::HOVER_SLOP_PT).map(|d| (d, *idx)))
        .min()
    {
        return Some(BoxHandle::Pin(idx));
    }
    best(n..2 * n).map(BoxHandle::Grip)
}

/// Which way grip `grip` of a box `outline` resizes it, for the pointer's shape before the press (rc.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GripResize {
    /// A vertical edge's middle: sideways.
    Sideways,
    /// A horizontal edge's middle: up and down.
    UpDown,
    /// The top-left or the bottom-right corner.
    CornerNwSe,
    /// The top-right or the bottom-left corner.
    CornerNeSw,
}

/// The [`GripResize`] of grip `grip` of `outline`; `None` for an edge that is neither horizontal nor vertical.
pub fn grip_resize(outline: &[(i64, i64)], grip: usize) -> Option<GripResize> {
    let n = outline.len();
    if grip < n {
        let (xs, ys) = (outline.iter().map(|p| p.0), outline.iter().map(|p| p.1));
        let mid = (
            (xs.clone().min()? + xs.max()?) / 2,
            (ys.clone().min()? + ys.max()?) / 2,
        );
        let p = outline[grip];
        return Some(if (p.0 < mid.0) == (p.1 < mid.1) {
            GripResize::CornerNwSe
        } else {
            GripResize::CornerNeSw
        });
    }
    let i = grip.checked_sub(n)?;
    let (a, b) = (*outline.get(i)?, outline[(i + 1) % n]);
    if a.1 == b.1 {
        Some(GripResize::UpDown)
    } else if a.0 == b.0 {
        Some(GripResize::Sideways)
    } else {
        None
    }
}

/// The overlay of a selected black box: the wire's round handle on each grip, hot under the pointer, and a dashed
/// square per CP, 3 points across each half (shapes, not colour, tell them apart; NFR-A11Y-03).
pub fn grips_overlay(cx: &ToolCx<'_>, rep: &TableRep, pointer: Option<(i64, i64)>) -> Overlay {
    let Some(b) = &rep.black_box else {
        return Overlay::default();
    };
    let hot = pointer.and_then(|at| handle_at(cx, rep, at));
    let r = cx.nm_of_points(3.0).round() as i64;
    let square = |p: (i64, i64), style: Style| {
        let s = cx.snapper.page_to_sheet(p);
        OverlayItem::Rect {
            r: rect_of((s.0 - r, s.1 - r), (s.0 + r, s.1 + r)),
            style,
        }
    };
    let mut items: Vec<OverlayItem> = grip_points(&b.outline)
        .into_iter()
        .enumerate()
        .map(|(k, p)| OverlayItem::Grip {
            at: cx.snapper.page_to_sheet(p),
            hot: hot == Some(BoxHandle::Grip(k)),
        })
        .collect();
    items.extend(
        b.cps
            .iter()
            .map(|(_, p, _)| square(*p, Style::dashed(Token::Ghost))),
    );
    Overlay::new(items)
}

/// The overlay of a reshape in progress: the new outline dashed and its CPs.
pub fn reshape_overlay(cx: &ToolCx<'_>, outline: &[(i64, i64)], cps: &[(i64, i64)]) -> Overlay {
    let mut pts: Vec<(i64, i64)> = outline.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect();
    if let Some(&first) = pts.first() {
        pts.push(first);
    }
    let mut items = vec![OverlayItem::Polyline {
        points: pts,
        style: Style::dashed(Token::Ghost),
    }];
    items.extend(cps.iter().map(|p| OverlayItem::Marker {
        at: cx.snapper.page_to_sheet(*p),
        kind: MarkerKind::Cp,
    }));
    Overlay::new(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rc.2 re-test: a terminal ran past the frame's edge. A box that would leave the drawing area moves inside it
    /// by whole grid steps; one larger than the area gives both sizes; without a frame grid it stays.
    #[test]
    fn a_one_click_box_moves_into_the_drawing_area_or_gives_its_size() {
        let g = PageGridMap::new((1_000_000, 1));
        let area = Some([10_000_000, 10_000_000, 100_000_000, 60_000_000]);
        let outline = [(0, 0), (30_000_000, 0), (30_000_000, 8_000_000), (0, 8_000_000)];
        assert_eq!(
            fit_in_area(area, g, (85_000_000, 58_000_000), &outline),
            Ok((70_000_000, 52_000_000)),
            "past the right and the bottom"
        );
        assert_eq!(
            fit_in_area(area, g, (4_000_000, 20_000_000), &outline),
            Ok((10_000_000, 20_000_000))
        );
        let wide = [(0, 0), (95_000_000, 0), (95_000_000, 8_000_000), (0, 8_000_000)];
        assert_eq!(
            fit_in_area(area, g, (20_000_000, 20_000_000), &wide),
            Err([95_000_000, 8_000_000, 90_000_000, 50_000_000])
        );
        assert_eq!(fit_in_area(None, g, (1, 2), &wide), Ok((1, 2)));
    }

    #[test]
    fn the_default_cps_go_half_on_top_and_grow_the_rectangle() {
        let g = PageGridMap::new((4_000_000, 1));
        let cps: Vec<(u16, String)> = (0..7).map(|i| (i, i.to_string())).collect();
        let (pos, outline, laid) =
            layout(g, (8_000_000, 12_000_000), (16_000_000, 36_000_000), &cps, &[]).unwrap();
        assert_eq!(pos, (8_000_000, 12_000_000));
        // Four on top need five grid steps of width: the rectangle grew from two to five.
        assert_eq!(
            outline,
            vec![(0, 0), (20_000_000, 0), (20_000_000, 24_000_000), (0, 24_000_000)]
        );
        assert_eq!(laid[0], (0, (4_000_000, 0), Dir::Up));
        assert_eq!(laid[3], (3, (16_000_000, 0), Dir::Up));
        assert_eq!(laid[4], (4, (4_000_000, 24_000_000), Dir::Down));
        assert_eq!(laid[6], (6, (12_000_000, 24_000_000), Dir::Down));
        assert!(
            layout(g, (0, 0), (5_000_000, 0), &cps, &[]).is_none(),
            "no height"
        );
        assert!(
            layout(g, (0, 0), (0, 5_000_000), &[], &[]).is_none(),
            "no width and no CP"
        );
    }

    /// On a box drawn wider than the CPs need, the CPs spread over the edge one common whole-grid pitch apart,
    /// the group centred, instead of packed from the left corner.
    #[test]
    fn the_default_cps_spread_over_the_drawn_edge_on_the_grid() {
        let g = PageGridMap::new((1_000_000, 1));
        let cps: Vec<(u16, String)> = (0..5).map(|i| (i, i.to_string())).collect();
        // Twelve steps wide: three on top at pitch 3 (12 / 4), from 3; two below at pitch 4 (12 / 3), from 4.
        let (_, outline, laid) = layout(g, (0, 0), (12_000_000, 6_000_000), &cps, &[]).unwrap();
        assert_eq!(outline[1], (12_000_000, 0), "the drawn width is kept");
        let xs: Vec<(i64, Dir)> = laid.iter().map(|(_, p, d)| (p.0 / 1_000_000, *d)).collect();
        assert_eq!(
            xs,
            [
                (3, Dir::Up),
                (6, Dir::Up),
                (9, Dir::Up),
                (4, Dir::Down),
                (8, Dir::Down)
            ]
        );
        // Every CP is a grid point strictly inside its edge, for any width the layout allows.
        for width in 1..40 {
            for m in 1..8 {
                let w = width.max(m + 1);
                let offsets: Vec<i64> = (0..m).map(|k| spread(w, m, k)).collect();
                assert!(
                    offsets[0] >= 1 && offsets[offsets.len() - 1] < w,
                    "{w} {m} {offsets:?}"
                );
                assert!(offsets.windows(2).all(|p| p[1] > p[0]), "{w} {m} {offsets:?}");
            }
        }
    }

    #[test]
    fn grips_keep_a_rectangle_and_move_edges_perpendicular() {
        let r = vec![(0, 0), (10, 0), (10, 5), (0, 5)];
        assert_eq!(grip_points(&r)[4..], [(5, 0), (10, 2), (5, 5), (0, 3)]);
        // Corner 2 to (12, 7): its neighbours follow on one axis each.
        assert_eq!(
            reshape_by_grip(&r, 2, (12, 7)).unwrap(),
            vec![(0, 0), (12, 0), (12, 7), (0, 7)]
        );
        // The right edge (grip 5 = edge v1v2) to x = 14.
        assert_eq!(
            reshape_by_grip(&r, 5, (14, 99)).unwrap(),
            vec![(0, 0), (14, 0), (14, 5), (0, 5)]
        );
        assert_eq!(reshape_by_grip(&r, 5, (0, 0)), Err(key!("reason-box-too-small")));
        // The right edge past the left one would turn the box inside out.
        assert_eq!(
            reshape_by_grip(&r, 5, (-4, 0)),
            Err(key!("reason-box-inside-out"))
        );
        assert_eq!(
            reshape_by_grip(&r, 2, (-3, 7)),
            Err(key!("reason-box-inside-out"))
        );
        assert_eq!(reshape_by_grip(&r, 8, (0, 0)), Err(key!("reason-no-grip")));
        // A triangle: a vertex alone; its diagonal edge refuses.
        let t = vec![(0, 0), (10, 0), (10, 5)];
        assert_eq!(
            reshape_by_grip(&t, 0, (2, 0)).unwrap(),
            vec![(2, 0), (10, 0), (10, 5)]
        );
        assert_eq!(reshape_by_grip(&t, 5, (0, 0)), Err(key!("reason-grip-not-axis")));
    }

    #[test]
    fn the_nearest_outline_point_is_on_the_grid_with_the_outward_normal() {
        let g = PageGridMap::new((1_000_000, 1));
        let r = vec![(0, 0), (10_000_000, 0), (10_000_000, 5_000_000), (0, 5_000_000)];
        assert_eq!(
            nearest_on_outline(g, &r, (3_400_000, 4_600_000)),
            Some(((3_000_000, 5_000_000), Dir::Down))
        );
        assert_eq!(
            nearest_on_outline(g, &r, (10_300_000, 2_200_000)),
            Some(((10_000_000, 2_000_000), Dir::Right))
        );
        assert!(on_outline(&r, (10_000_000, 2_000_000)));
        assert!(!on_outline(&r, (9_000_000, 2_000_000)));
        // Past a corner: a step clear of it, never on it (rc.5).
        assert_eq!(
            nearest_on_outline(g, &r, (-1_000_000, -1_000_000)),
            Some(((1_000_000, 0), Dir::Up))
        );
        assert_eq!(orient_inverse(1), 3);
        assert_eq!(orient_inverse(5), 5);
    }

    /// rc.5: a box made smaller goes no smaller than its pins need, one grid step each and one to spare at each corner:
    /// the grip stops there; a side whose length does not change does not hold it.
    #[test]
    fn a_grip_stops_where_the_pins_need_the_room() {
        let g = PageGridMap::new((1_000_000, 1));
        let r = vec![(0, 0), (10_000_000, 0), (10_000_000, 5_000_000), (0, 5_000_000)];
        let cps: Vec<BoxCpAt> = [2, 5, 8]
            .into_iter()
            .zip(0..)
            .map(|(x, i)| (i, (x * 1_000_000, 0), Dir::Up))
            .collect();
        // The right edge's grip (5: the midpoint of v1v2) dragged to x 2: three pins need four steps.
        let (at, new) = grip_stop(g, &r, &cps, 0, 5, (2_000_000, 2_000_000)).unwrap();
        assert_eq!(at, (4_000_000, 2_000_000));
        assert_eq!(new[1].0, 4_000_000);
        // To x 6 it goes all the way.
        assert_eq!(
            grip_stop(g, &r, &cps, 0, 5, (6_000_000, 2_000_000)).unwrap().0,
            (6_000_000, 2_000_000)
        );
        // The bottom edge's grip (6) up to y 1: the top and bottom sides keep their length, nothing holds it.
        assert_eq!(
            grip_stop(g, &r, &cps, 0, 6, (3_000_000, 1_000_000)).unwrap().0,
            (3_000_000, 1_000_000)
        );
    }
}
