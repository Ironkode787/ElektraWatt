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
//! on the top edge and its outputs on the bottom one; when an edge is too short the rectangle grows to the right until they fit one grid step
//! apart with one grid step to spare at each corner. On a wider box the CPs of an edge spread over its drawn length:
//! one common pitch, the largest whole number of grid steps that fits `m + 1` times into the edge, the group
//! centred (half a step to the left when it cannot be centred exactly). Without a part, no CPs: such a box takes pins of its
//! own afterwards, where it stands ([`super::box_pin`]).
//!
//! **A valve terminal drawn as its positions** ([`BlackBoxTool::with_slots`], doc 11 §3.2): the box is sized by the
//! terminal's positions, not by two clicks; armed, the tool shows it dashed with its top-left corner at the pointer, and
//! one click commits `box.black` with `box: {outline, slots: {pitch}}` (`ew_api::geometry::slot_box`). **A multipin
//! terminal's plug** ([`BlackBoxTool::with_pins`]) is placed the same way, sized by its pins down its two sides
//! ([`pin_layout`]). Such a one-click box stands inside the frame's drawing area, moved there by whole grid steps where
//! it would leave it, and is refused with its size where it is larger than the area (`reason-box-too-large`) and on
//! another box (`reason-box-occupied`); the host ends the tool once it is placed (one click, one box). **A device on a
//! bus page** ([`BoxTarget::Ports`], doc 10 §2.7) is placed the same way with its bus symbol, sized by its part
//! (`bus.place_ports`): inside the drawing area with its sockets, and refused where its body would cover a device
//! drawn on the page (`reason-ports-occupied`).
//!
//! **Grips** ([`grip_points`], [`reshape_by_grip`]): for an outline of `n` vertices in stored order, grips `0 … n−1`
//! are the vertices and grips `n … 2n−1` the midpoints of the edges `v0v1, …, v(n−1)v0`. On a rectangle a vertex grip
//! moves that corner and keeps the rectangle, an edge grip moves that edge perpendicular to itself; on any other
//! outline a vertex grip moves that vertex alone and an edge grip translates an axis-aligned edge (another edge is
//! refused, `reason-grip-not-axis`). Select drags them (and a box CP along the outline) and commits `box.reshape`.

use crate::key;
use ew_api::AnyCommand;
use ew_api::geometry::Dir;
use serde_json::{Value, json};

use super::wire::PageGridMap;
use super::{
    Anchor, Button, CanvasEvent, Ghost, MarkerKind, Overlay, OverlayItem, Style, Token, Tool, ToolCx,
    ToolOutput,
};

mod grips;
pub use grips::{
    BoxHandle, GripResize, GripStop, black_box_rep, free_edge_pin, grip_points, grip_resize, grip_stop,
    grips_overlay, handle_at, is_rectangle, nearest_on_outline, on_outline, orient_inverse, outward,
    pin_target, reshape_by_grip, reshape_command, reshape_named, reshape_overlay, selected_black_box,
    signed_area2,
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
        /// A PLC card's pins by kind, for a part without a device box: its box is a card's (doc 10 §3.6).
        card: Option<ew_api::output::CardPins>,
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
    /// as its positions; its main function, or another when its bus port drawn on a bus page is that.
    Device {
        /// The device's ID text.
        device: String,
        /// Whether the box is the device's main function.
        main: bool,
    },
    /// A station or one of its modules drawn on an electrical page by its box (`plc.draw_station` of the one box):
    /// Place here on a PLC part that no electrical page draws, made with or without functions.
    Station {
        /// The station's ID text.
        station: String,
        /// The device's ID text: the station or a module of it.
        device: String,
        /// What the label names: the device's tag.
        label: String,
        /// The CPs of its part's black-box entry: (function CP index, designation).
        cps: Vec<(u16, String)>,
        /// The part's device box (doc 07 §2.5): one click places the box as the part lays it out.
        layout: Option<ew_api::part_box::PartBox>,
        /// A PLC card's pins by kind, for a part without a device box: its box is a card's (doc 10 §3.6).
        card: Option<ew_api::output::CardPins>,
    },
    /// A device on a bus-topology page, drawn with its bus symbol (`bus.place_ports`, doc 10 §2.7).
    Ports {
        /// The command's `target`: `{"device": {"device": id}}` or `{"from_part": {"part": id, "letter": l}}`.
        target: Value,
        /// What the label names (`part EW-GEN-CPU-2`, `-3A1`).
        label: String,
        /// The symbol its part draws on the page: its body and sockets.
        shape: ew_api::output::BusSymbolShape,
    },
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

/// The box of a multipin valve terminal's plug with its top-left corner at the grid point `at`: its pins down its
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
    let Some(a) = bounds(outline.iter().map(|p| (pos.0 + p.0, pos.1 + p.1))) else {
        return false;
    };
    cx.page
        .tables
        .reps
        .iter()
        .filter_map(|r| r.black_box.as_ref())
        .any(|b| {
            bounds(b.outline.iter().copied())
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
    /// The library documents the black box embeds (`box.black`'s `copies`); the host sends only those the
    /// project still lacks.
    copies: Vec<Value>,
    /// The representation type the box takes where it is not the frame's own (`topology` on a bus-topology page).
    rep_type: Option<String>,
    /// The positions of a valve terminal drawn as its positions: one click places its box of that many cells.
    slots: Option<u16>,
    /// A multipin valve terminal's plug: one click places its box sized by its pins, down its two sides.
    pins: bool,
    /// Whether a part without a symbol of its own armed the tool from the Place tool: the hint says that it is drawn as
    /// a box.
    drawn_as_box: bool,
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
            drawn_as_box: false,
        }
    }

    /// The tool drawing a part that has no symbol of its own, armed by the Place tool: its hint says so.
    pub fn with_drawn_as_box(mut self, drawn_as_box: bool) -> BlackBoxTool {
        self.drawn_as_box = drawn_as_box;
        self
    }

    /// The tool placing a valve terminal of `count` positions drawn as its positions: one click, no outline.
    pub fn with_slots(mut self, count: Option<u16>) -> BlackBoxTool {
        self.slots = count.filter(|n| *n > 0);
        self
    }

    /// The tool placing a multipin valve terminal's plug: one click, the box sized by its pins ([`pin_layout`]).
    pub fn with_pins(mut self, pins: bool) -> BlackBoxTool {
        self.pins = pins;
        self
    }

    /// The device box of a part target, or of a station's device, that lays one out.
    fn part_layout(&self) -> Option<&ew_api::part_box::PartBox> {
        match &self.target {
            BoxTarget::Part { layout, .. } | BoxTarget::Station { layout, .. } => layout.as_ref(),
            _ => None,
        }
    }

    /// Whether one click places the box, sized by what it draws (a terminal's positions, a plug's pins, a part's
    /// device box); the host then ends the tool.
    pub fn one_click(&self) -> bool {
        self.slots.is_some()
            || self.pins
            || self.part_layout().is_some()
            || matches!(self.target, BoxTarget::Ports { .. })
    }

    /// The box one click places at the armed point: its layout, and the cell width of a terminal drawn as its positions.
    /// A box that would leave the frame's drawing area is moved inside it by whole grid steps where it fits (a terminal must not
    /// run past the frame's edge, its positions cut off), and laid out afresh there: on a grid whose
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
        // A bus symbol: its body and its sockets, the outline round them both, its top-left corner at the pointer.
        if let BoxTarget::Ports { shape, .. } = &self.target {
            let cps: Vec<BoxCpAt> = (0u16..)
                .zip(&shape.sockets)
                .map(|(i, p)| (i, *p, if p.1 < 0 { Dir::Up } else { Dir::Down }))
                .collect();
            let corners = [(shape.body[0], shape.body[1]), (shape.body[2], shape.body[3])];
            let b = bounds(shape.sockets.iter().copied().chain(corners))?;
            let outline = vec![(b[0], b[1]), (b[2], b[1]), (b[2], b[3]), (b[0], b[3])];
            return Some(((at, outline, cps), None));
        }
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

    /// The tool with the library documents its black boxes embed.
    pub fn with_copies(mut self, copies: Vec<Value>) -> BlackBoxTool {
        self.copies = copies;
        self
    }

    /// The armed target.
    pub fn target(&self) -> &BoxTarget {
        &self.target
    }

    /// The records of the project the box names when it is put down (ID texts): the symbol copy unless the box
    /// embeds it, and the function or device it draws. An undo or a redo that takes one away ends the tool.
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

    /// The pins by kind of a card target without a device box; `None` for any other target.
    fn card(&self) -> Option<&ew_api::output::CardPins> {
        match &self.target {
            BoxTarget::Part {
                card, layout: None, ..
            }
            | BoxTarget::Station {
                card, layout: None, ..
            } => card.as_ref().filter(|c| c.is_card()),
            _ => None,
        }
    }

    fn cps(&self) -> &[(u16, String)] {
        match &self.target {
            BoxTarget::Part { cps, .. }
            | BoxTarget::Function { cps, .. }
            | BoxTarget::Station { cps, .. } => cps,
            BoxTarget::NewDevice { .. } | BoxTarget::Device { .. } | BoxTarget::Ports { .. } => &[],
        }
    }

    /// The target's source outputs among its CPs.
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

    /// The layout of the current rectangle: its box.
    fn current(&self, cx: &ToolCx<'_>) -> Option<Vec<BoxLayout>> {
        let State::Outline { first, at } = self.state else {
            return None;
        };
        let grid = PageGridMap::new(cx.page.tables.grid);
        // A card: its box as Draw on a page lays it (doc 10 §3.6), its top-left corner at the rectangle's, in the size
        // its row needs; a row wider than the drawing area from there is its first box, the rest drawn later.
        if let Some(card) = self.card() {
            let corner = (first.0.min(at.0), first.1.min(at.1));
            let right = super::ip::drawing_area(cx.page).map_or(i64::MAX, |a| a[2]);
            let room = grid.index(right).saturating_sub(grid.index(corner.0));
            let steps = card.boxes(room).into_iter().next()?;
            return card_on_grid(&steps, cx.page.tables.grid, corner).map(|l| vec![l]);
        }
        layout(grid, first, at, self.cps(), self.outputs()).map(|l| vec![l])
    }

    /// Whether the body of a bus symbol at `pos` would cover a device drawn on the page shown (its box, else its body).
    fn covers_a_device(&self, cx: &ToolCx<'_>, pos: (i64, i64)) -> bool {
        let BoxTarget::Ports { shape, .. } = &self.target else {
            return false;
        };
        let a = [
            pos.0 + shape.body[0],
            pos.1 + shape.body[1],
            pos.0 + shape.body[2],
            pos.1 + shape.body[3],
        ];
        cx.page
            .tables
            .reps
            .iter()
            .filter(|r| r.device.is_some())
            .any(|r| {
                let o = r
                    .black_box
                    .as_ref()
                    .and_then(|b| bounds(b.outline.iter().copied()))
                    .unwrap_or(r.body);
                a[0] < o[2] && o[0] < a[2] && a[1] < o[3] && o[1] < a[3]
            })
    }

    /// The `box.black` of the current rectangle, or the `bus.place_ports` of a ports target.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        let one = self.one_click_box(cx);
        let layouts = match &one {
            Some((layout, _)) => vec![layout.clone()],
            None => self.current(cx)?,
        };
        if let BoxTarget::Ports { target, .. } = &self.target {
            let (pos, _, _) = layouts.first()?;
            let mut params = json!({
                "page": cx.page.tables.page.to_string(),
                "symbol": self.symbol,
                "target": target,
                "pos": [pos.0, pos.1],
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
            // The label at the pointer says what a click does (it is not a tool of two corners).
            items.push(OverlayItem::Label {
                at: cx.snapper.page_to_sheet(pos),
                text: crate::strings::format(key!("place-target-click"), &[&self.label()]),
                anchor: Anchor::Beside,
            });
            return Overlay::new(items);
        }
        match self.state {
            State::Armed { at: None } => Overlay::default(),
            // A chip at the pointer says how the box is drawn, where the person looks (a part placed as a black
            // box asked for two corners in the bottom bar only).
            State::Armed { at: Some(at) } => Overlay::new(vec![
                OverlayItem::Marker {
                    at: cx.snapper.page_to_sheet(at),
                    kind: MarkerKind::Grid,
                },
                OverlayItem::Label {
                    at: cx.snapper.page_to_sheet(at),
                    text: crate::strings::text(key!("box-chip-armed")).to_owned(),
                    anchor: Anchor::Beside,
                },
            ]),
            State::Outline { first, at } => {
                let mut items = Vec::new();
                let current = self.current(cx);
                // While the rectangle has no area the first corner is marked, so the click visibly registered.
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
                    anchor: Anchor::Beside,
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
                // A box larger than the drawing area says so, with its size.
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
                // A bus symbol never covers a device drawn on the page.
                if self.covers_a_device(cx, pos) {
                    return ToolOutput::Refused(key!("reason-ports-occupied"));
                }
                // A box never lands on another box (two terminals never stand on one spot).
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

/// A box `steps` on the page's grid `page_grid` (num, den), its top-left corner at the grid point nearest `at`, as a
/// [`BoxLayout`]: how a card's box is put on a page (Draw on a page and the black box tool alike).
pub fn card_on_grid(
    steps: &ew_api::part_box::BoxSteps,
    page_grid: (i64, i64),
    at: (i64, i64),
) -> Option<BoxLayout> {
    use ew_api::part_box::Dir4;
    let (pos, outline, cps) = steps.on_grid(page_grid, at)?;
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
    Some((pos, outline, cps))
}

/// The bounds `[x0, y0, x1, y1]` of points; `None` without one.
fn bounds(pts: impl Iterator<Item = (i64, i64)>) -> Option<[i64; 4]> {
    pts.fold(None, |acc: Option<[i64; 4]>, (x, y)| {
        Some(acc.map_or([x, y, x, y], |r| {
            [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)]
        }))
    })
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

    fn follows(&self, cmd: &dyn super::DynCommand) -> super::Follows {
        let json = cmd.to_json().ok();
        let ports = cmd.name() == "bus.place_ports";
        super::Follows {
            // A valve terminal drawn as its positions answers as a placement does, with what its positions hold.
            placed: (cmd.name() == "box.black"
                && json.as_ref().is_some_and(|j| j["box"].get("slots").is_some()))
            .then(super::Placement::default),
            // A station placed first on a bus page takes the tag of the page its box is drawn on now, which the toast
            // says.
            retag: true,
            drew_existing: ports && json.as_ref().is_some_and(|j| j["target"].get("device").is_some()),
            // A box one click places (a valve terminal's positions, a plug's pins) is one box: the tool gives way to
            // Select (a second click placed a second terminal on the first). So does a station put on a bus page: its
            // interfaces are all drawn there now, and the tool kept taking it ("-3A1 DP is drawn on this page
            // already"). So does a PLC part's box placed with Place here: the part is drawn now, and a second click
            // was refused.
            gives_way: self.one_click()
                || matches!(self.target(), BoxTarget::Ports { target, .. } if target.get("device").is_some())
                || matches!(self.target(), BoxTarget::Station { .. }),
            ..super::Follows::default()
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
            State::Armed { .. } if self.drawn_as_box => {
                super::Hint::new(key!("hint-box-part-armed"), &[super::ESCAPE])
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
            BoxTarget::Ports { shape, .. } => shape.sockets.len(),
            _ => self.cps().len(),
        };
        Some(Ghost::Box { outline, cps })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A re-test: a terminal ran past the frame's edge. A box that would leave the drawing area moves inside it
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
        // Past a corner: a step clear of it, never on it.
        assert_eq!(
            nearest_on_outline(g, &r, (-1_000_000, -1_000_000)),
            Some(((1_000_000, 0), Dir::Up))
        );
        assert_eq!(orient_inverse(1), 3);
        assert_eq!(orient_inverse(5), 5);
    }

    /// A box made smaller goes no smaller than its pins need, one grid step each and one to spare at each corner:
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
        let (at, new) = grip_stop(g, &r, &cps, (0, 0), 5, (2_000_000, 2_000_000)).unwrap();
        assert_eq!(at, (4_000_000, 2_000_000));
        assert_eq!(new[1].0, 4_000_000);
        // To x 6 it goes all the way.
        assert_eq!(
            grip_stop(g, &r, &cps, (0, 0), 5, (6_000_000, 2_000_000))
                .unwrap()
                .0,
            (6_000_000, 2_000_000)
        );
        // The bottom edge's grip (6) up to y 1: the top and bottom sides keep their length, nothing holds it.
        assert_eq!(
            grip_stop(g, &r, &cps, (0, 0), 6, (3_000_000, 1_000_000))
                .unwrap()
                .0,
            (3_000_000, 1_000_000)
        );
    }
}
