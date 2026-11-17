// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device box of a part (doc 07 §2.5, "The device box"): the pins a part of one's own has and where they stand
//! on the box a device of it is drawn as.
//!
//! **The pins** are the part's CP pattern, drawn by its main function, a black box. The part editor's draft holds
//! them as rows ([`PinRow`]: name and potential) which a person adds to, removes from and renames; Save writes the
//! pattern and the main template entry's pins. A part with a PLC section, with bus interfaces, or drawn with a
//! symbol of its own keeps its pins: they come from its channels, its ports or its symbol ([`PinsFrom`]).
//!
//! **The layout** ([`ew_schema::PartBox`], `Part.box`) is the box's size and, for each pin, the edge it stands on
//! and its distance along that edge. [`steps`] reads it onto a connection grid: distances taken up to whole grid
//! steps, every pin at least one step from the corners and from its neighbours, the box widened where the pins need
//! it. A part without a layout is laid out as the black box tool always did ([`split`], [`spread`]): the first
//! half of the pins on top, the rest below, a source's outputs below.

use ew_commands::commands::ReshapeBox;
use ew_derive::ew_ids::{Id, Uuid};
use ew_mltext::Ident;
use ew_schema::{BoxCp, CpData, Part, PotentialType, Representation, TemplateFn};
pub use ew_schema::{Dir4, PartBox, PartBoxPin};
use ew_units::{Grid, Nm, Pt2};

use crate::part_editor::{PartDraft, PartDraftError, enum_words, message};
use crate::{AnyCommand, Session};

/// The connection grid's step the device box is edited on, in nanometres: the native 4 mm.
pub const EDIT_STEP: i64 = 4_000_000;

/// The height in grid steps of a box that was given none.
const DEFAULT_HEIGHT: i64 = 5;

/// One pin of a part's draft.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinRow {
    /// The pin's index in the CP pattern, which a placed device's wires hold on to.
    pub idx: u16,
    /// Its printed designation.
    pub name: String,
    /// The potential it carries, as the file spells it (`l_plus`, `pe`), else empty.
    pub role: String,
}

/// Where a part's pins come from when they are not the person's to change here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PinsFrom {
    /// The channels of its PLC section.
    Channels,
    /// The ports of its bus interfaces.
    Ports,
    /// The symbol it is drawn with.
    Symbol,
}

/// A device box on a connection grid: its size and its pins in whole grid steps.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BoxSteps {
    /// The box's width in steps.
    pub w: i64,
    /// Its height in steps.
    pub h: i64,
    /// The pins: (index, the edge by the direction a wire leaves in, steps along the edge from the top-left corner).
    pub pins: Vec<(u16, Dir4, i64)>,
}

/// The potentials a pin takes, as the file spells them, in the list's order.
pub fn pin_roles() -> Vec<String> {
    enum_words::<PotentialType>()
}

/// The pins of a box without a layout in their order along its edges and how many of them stand on top: with some
/// but not all of them among `outputs` (a source's outputs) the others on top and those below, else the first half
/// on top.
pub fn split(cps: &[u16], outputs: &[u16]) -> (Vec<u16>, usize) {
    let n = cps.len();
    let out = |c: &&u16| outputs.contains(c);
    let below = cps.iter().filter(out).count();
    if below > 0 && below < n {
        let inputs = cps.iter().filter(|c| !out(c));
        (inputs.chain(cps.iter().filter(out)).copied().collect(), n - below)
    } else {
        (cps.to_vec(), n.div_ceil(2))
    }
}

/// The grid offset of pin `k` of `m` on an edge `width` grid steps long: the pins one common pitch apart, the largest
/// whole number of grid steps that fits `m + 1` times into the edge (at least one), and the group centred on the edge
/// (half a step to the left when it cannot be centred exactly). Every pin lies at least one grid step from both
/// corners whenever `width ≥ m + 1`.
pub fn spread(width: i64, m: i64, k: i64) -> i64 {
    let pitch = (width / (m + 1)).max(1);
    let start = (width - (m - 1) * pitch) / 2;
    start + k * pitch
}

fn horizontal(side: &Dir4) -> bool {
    matches!(side, Dir4::Up | Dir4::Down)
}

/// The device box of the pins `cps` (in the pattern's order) on a connection grid of `step` nanometres: with
/// `layout` each listed pin on its edge in the layout's order, each distance (from the corner to the first pin, from
/// pin to pin, from the last pin to the far corner) taken up to whole steps and at least one, so that a layout drawn
/// on another grid keeps its order and sides and comes out no tighter; a pin the layout does not list on the bottom
/// edge after the others. Without a layout, [`split`] and [`spread`] on a box as wide as its longer row needs.
pub fn steps(cps: &[u16], outputs: &[u16], layout: Option<&PartBox>, step: i64) -> BoxSteps {
    let step = step.max(1);
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX / 4);
    let Some(layout) = layout else {
        let (order, top) = split(cps, outputs);
        let bottom = order.len() - top;
        let w = (count(top.max(bottom)) + 1).max(4);
        let pins = order
            .iter()
            .enumerate()
            .map(|(i, idx)| {
                if i < top {
                    (*idx, Dir4::Up, spread(w, count(top), count(i)))
                } else {
                    (*idx, Dir4::Down, spread(w, count(bottom), count(i - top)))
                }
            })
            .collect();
        return BoxSteps {
            w,
            h: DEFAULT_HEIGHT,
            pins,
        };
    };
    // A distance in whole steps, taken up to the next step: a layout drawn on another grid comes out no tighter.
    let up = |nm: i64| -> i64 {
        let nm = i128::from(nm.max(0));
        i64::try_from((nm + i128::from(step) - 1) / i128::from(step)).unwrap_or(i64::MAX / 4)
    };
    let mut sides: [Vec<(u16, i64)>; 4] = Default::default();
    let slot = |side: &Dir4| match side {
        Dir4::Up => 0,
        Dir4::Down => 1,
        Dir4::Left => 2,
        _ => 3,
    };
    let mut listed = Vec::new();
    for pin in layout.pins.iter().filter(|p| cps.contains(&p.idx)) {
        if !listed.contains(&pin.idx) {
            listed.push(pin.idx);
            sides[slot(&pin.side)].push((pin.idx, pin.at.0));
        }
    }
    // Along each edge in the order of their distances, each the distance from the one before (from the corner for
    // the first) in whole steps after it, at least one; then the edge's length past the last pin, at least a step.
    let edge = |length: Nm, side: &mut Vec<(u16, i64)>| -> i64 {
        side.sort_by_key(|(_, at)| *at);
        let (mut last_nm, mut last) = (0_i64, 0_i64);
        for (_, at) in side.iter_mut() {
            let k = last.saturating_add(up(at.saturating_sub(last_nm)).max(1));
            last_nm = *at;
            *at = k;
            last = k;
        }
        last.saturating_add(up(length.0.saturating_sub(last_nm)).max(1))
    };
    let [top, bottom, left, right] = &mut sides;
    let w = edge(layout.width, top).max(edge(layout.width, bottom));
    let h = edge(layout.height, left).max(edge(layout.height, right));
    for idx in cps.iter().filter(|c| !listed.contains(c)) {
        let last = sides[1].last().map_or(0, |(_, at)| *at);
        sides[1].push((*idx, last + 1));
    }
    let need =
        |a: &[(u16, i64)], b: &[(u16, i64)]| a.iter().chain(b).map(|(_, at)| at + 1).max().unwrap_or(0);
    let w = w.max(need(&sides[0], &sides[1])).max(2);
    let h = h.max(need(&sides[2], &sides[3])).max(2);
    let dirs = [Dir4::Up, Dir4::Down, Dir4::Left, Dir4::Right];
    let pins = sides
        .iter()
        .zip(dirs)
        .flat_map(|(side, dir)| side.iter().map(move |(idx, at)| (*idx, dir.clone(), *at)))
        .collect();
    BoxSteps { w, h, pins }
}

impl BoxSteps {
    /// Sizes the box `w` by `h` steps with the change of width made at its left edge when `from_left` (else at its
    /// right) and of height at its top when `from_top` (else at its bottom): the pins on the edge that moves go with
    /// it, the others stay where they stand, and the box stops where a pin would come nearer than one step to its
    /// moving edge.
    pub fn resize(&mut self, w: i64, h: i64, from_left: bool, from_top: bool) {
        // The least and the most distance of the pins on the top and bottom edges (`true`) or the left and right ones.
        let extent = |across: bool| {
            let at = self
                .pins
                .iter()
                .filter(|p| horizontal(&p.1) == across)
                .map(|p| p.2);
            (at.clone().min(), at.max())
        };
        let fit = |want: i64, now: i64, (lo, hi): (Option<i64>, Option<i64>), from_start: bool| {
            let need = if from_start {
                lo.map_or(0, |lo| now - lo + 1)
            } else {
                hi.map_or(0, |hi| hi + 1)
            };
            let length = want.max(need).max(2);
            (length, if from_start { length - now } else { 0 })
        };
        let (w, dx) = fit(w, self.w, extent(true), from_left);
        let (h, dy) = fit(h, self.h, extent(false), from_top);
        for pin in &mut self.pins {
            pin.2 += if horizontal(&pin.1) { dx } else { dy };
        }
        self.w = w;
        self.h = h;
    }

    /// The layout the part stores for the box, on a grid of `step` nanometres.
    pub fn layout(&self, step: i64) -> PartBox {
        PartBox {
            width: Nm(self.w * step),
            height: Nm(self.h * step),
            pins: self
                .pins
                .iter()
                .map(|(idx, side, at)| PartBoxPin {
                    idx: *idx,
                    side: side.clone(),
                    at: Nm(at * step),
                    extra: Default::default(),
                })
                .collect(),
            extra: Default::default(),
        }
    }

    /// [`BoxSteps::on_page`] for a grid given as its `(num, den)` nanometres per step, as the page tables give it.
    pub fn on_grid(&self, grid: (i64, i64), at: (i64, i64)) -> Option<PlacedBox> {
        let grid = Grid::new(u64::try_from(grid.0).ok()?, u64::try_from(grid.1).ok()?).ok()?;
        self.on_page(grid, at)
    }

    /// The box on a page of grid `grid` with its top-left corner at the grid point nearest `at` (page nm): the
    /// insertion point, the outline relative to it (the top-left corner first, then clockwise) and each pin's index,
    /// point relative to it and direction, as `box.black` and `box.reshape` take them. `None` when a coordinate
    /// overflows.
    pub fn on_page(&self, grid: Grid, at: (i64, i64)) -> Option<PlacedBox> {
        let (kx, ky) = (grid.snap(Nm(at.0)), grid.snap(Nm(at.1)));
        let x = |k: i64| grid.at(kx + k).map(|n| n.0);
        let y = |k: i64| grid.at(ky + k).map(|n| n.0);
        let pos = (x(0)?, y(0)?);
        let (w, h) = (x(self.w)? - pos.0, y(self.h)? - pos.1);
        let mut cps = Vec::with_capacity(self.pins.len());
        for (idx, side, k) in &self.pins {
            let point = match side {
                Dir4::Up => (x(*k)? - pos.0, 0),
                Dir4::Down => (x(*k)? - pos.0, h),
                Dir4::Left => (0, y(*k)? - pos.1),
                _ => (w, y(*k)? - pos.1),
            };
            cps.push((*idx, point, side.clone()));
        }
        Some((pos, vec![(0, 0), (w, 0), (w, h), (0, h)], cps))
    }
}

/// A device box on a page: the insertion point (page nm), the outline relative to it and the pins `(index, point
/// relative to it, direction)`.
pub type PlacedBox = ((i64, i64), Vec<(i64, i64)>, Vec<(u16, (i64, i64), Dir4)>);

impl PartDraft {
    /// Where the pins come from when they are not edited here; `None` for a part whose pins are the person's to
    /// change: a part drawn as a box, or one with neither pins nor functions yet.
    pub fn pins_from(&self) -> Option<PinsFrom> {
        // A section without a role is none (a part made by an earlier build's form may carry one).
        let card = self
            .original
            .sections
            .plc
            .as_ref()
            .is_some_and(|s| !matches!(s.role, ew_schema::PlcRole::Unknown(_)));
        if card || !self.plc.role.trim().is_empty() {
            Some(PinsFrom::Channels)
        } else if !self.bus.is_empty() {
            Some(PinsFrom::Ports)
        } else if self.fndef.is_some() || !(self.box_part || self.original.function_template.is_empty()) {
            Some(PinsFrom::Symbol)
        } else {
            None
        }
    }

    /// The place of the part's main function in its template (`place.pick`'s `template`): the entry a box is drawn
    /// from.
    pub fn main_entry(&self) -> u16 {
        let at = self
            .original
            .function_template
            .iter()
            .position(|t| t.main)
            .unwrap_or(0);
        u16::try_from(at).unwrap_or(0)
    }

    fn pin_indices(&self) -> Vec<u16> {
        self.pins.iter().map(|p| p.idx).collect()
    }

    /// The device box as it will be drawn, on the 4 mm grid it is edited on.
    pub fn box_steps(&self) -> BoxSteps {
        steps(&self.pin_indices(), &self.outputs, self.box_.as_ref(), EDIT_STEP)
    }

    /// Changes the layout with `change` and keeps what [`steps`] makes of it: every edit leaves a whole layout.
    fn relayout(&mut self, change: impl FnOnce(&mut BoxSteps)) {
        let mut b = self.box_steps();
        change(&mut b);
        let layout = b.layout(EDIT_STEP);
        let checked = steps(&self.pin_indices(), &self.outputs, Some(&layout), EDIT_STEP);
        self.box_ = Some(checked.layout(EDIT_STEP));
    }

    /// Adds a pin named `name` (else the next number that no pin has) and returns its index. On a box with a layout
    /// it stands on the bottom edge, after the others.
    pub fn add_pin(&mut self, name: Option<&str>) -> u16 {
        let idx = self.pins.iter().map(|p| p.idx + 1).max().unwrap_or(0);
        let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => n.to_owned(),
            None => (1..)
                .map(|n: u32| n.to_string())
                .find(|n| !self.pins.iter().any(|p| p.name == *n))
                .unwrap_or_default(),
        };
        self.pins.push(PinRow {
            idx,
            name,
            role: String::new(),
        });
        if self.box_.is_some() {
            self.relayout(|_| {});
        }
        idx
    }

    /// Removes pin `idx`; whether the part had it.
    pub fn remove_pin(&mut self, idx: u16) -> bool {
        let before = self.pins.len();
        self.pins.retain(|p| p.idx != idx);
        if let Some(b) = &mut self.box_ {
            b.pins.retain(|p| p.idx != idx);
        }
        self.pins.len() != before
    }

    /// Names pin `idx` `name` (a designation as a part prints it) and, with `role`, gives it the potential it
    /// carries (one of [`pin_roles`], or empty for none).
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Field`] on `pins` for a pin the part does not have, a name that is empty or no designation,
    /// and a role that is none of the list.
    pub fn set_pin(
        &mut self,
        idx: u16,
        name: Option<&str>,
        role: Option<&str>,
    ) -> Result<(), PartDraftError> {
        let refuse = |key, arg: &str| PartDraftError::Field(message("pins", key, &[arg]));
        let Some(pin) = self.pins.iter_mut().find(|p| p.idx == idx) else {
            return Err(refuse(
                ew_commands::text_key!("error-part-pin-unknown"),
                &idx.to_string(),
            ));
        };
        if let Some(name) = name {
            let name = name.trim();
            if name.is_empty() || Ident::from_input(name).is_err() {
                return Err(refuse(ew_commands::text_key!("error-part-pin-name"), name));
            }
            name.clone_into(&mut pin.name);
        }
        if let Some(role) = role {
            if !role.is_empty() && !pin_roles().iter().any(|r| r == role) {
                return Err(refuse(ew_commands::text_key!("error-part-pin-role"), role));
            }
            role.clone_into(&mut pin.role);
        }
        Ok(())
    }

    /// Puts pin `idx` on the edge `side` at `at` nanometres along it (rounded to the grid, kept clear of the corners
    /// and of its neighbours); whether the part has the pin.
    pub fn place_pin(&mut self, idx: u16, side: Dir4, at: i64) -> bool {
        if !self.pins.iter().any(|p| p.idx == idx) {
            return false;
        }
        let k = ((at as f64) / (EDIT_STEP as f64)).round() as i64;
        self.relayout(|b| {
            let length = if horizontal(&side) { b.w } else { b.h };
            let k = k.clamp(1, (length - 1).max(1));
            // A pin already at that place gives way: the two change places.
            let own = b.pins.iter().find(|p| p.0 == idx).map(|p| (p.1.clone(), p.2));
            if let (Some((old_side, old_at)), Some(other)) = (
                own,
                b.pins.iter_mut().find(|p| p.0 != idx && p.1 == side && p.2 == k),
            ) {
                other.1 = old_side;
                other.2 = old_at;
            }
            if let Some(pin) = b.pins.iter_mut().find(|p| p.0 == idx) {
                pin.1 = side;
                pin.2 = k;
            }
        });
        true
    }

    /// Moves pin `idx` in the order the pin table lists them (the edges top, bottom, left, right, each from its
    /// start): onto the edge `side`, before pin `before` there, or last on it. On its own edge the pins keep the
    /// edge's places and change order; on another edge that edge's pins are spread anew. Whether the part has the pin.
    pub fn move_pin(&mut self, idx: u16, side: Dir4, before: Option<u16>) -> bool {
        if !self.pins.iter().any(|p| p.idx == idx) {
            return false;
        }
        self.relayout(|b| {
            let same = b.pins.iter().any(|p| p.0 == idx && p.1 == side);
            let mut row: Vec<(u16, i64)> = b
                .pins
                .iter()
                .filter(|p| p.1 == side && p.0 != idx)
                .map(|p| (p.0, p.2))
                .collect();
            row.sort_by_key(|(_, at)| *at);
            let at = before
                .and_then(|other| row.iter().position(|(i, _)| *i == other))
                .unwrap_or(row.len());
            let mut places: Vec<i64> = b.pins.iter().filter(|p| p.1 == side).map(|p| p.2).collect();
            places.sort_unstable();
            row.insert(at, (idx, 0));
            let m = i64::try_from(row.len()).unwrap_or(1);
            let length = if horizontal(&side) { b.w } else { b.h };
            // A longer row may need a longer edge.
            let length = length.max(m + 1);
            if horizontal(&side) {
                b.w = length;
            } else {
                b.h = length;
            }
            for (k, (i, _)) in row.iter().enumerate() {
                let Some(pin) = b.pins.iter_mut().find(|p| p.0 == *i) else {
                    continue;
                };
                pin.1 = side.clone();
                pin.2 = if same {
                    places[k]
                } else {
                    spread(length, m, i64::try_from(k).unwrap_or(0))
                };
            }
        });
        true
    }

    /// Sizes the box `width` by `height` nanometres, each rounded to the grid, the width changed at the left edge when
    /// `from_left` and the height at the top when `from_top` ([`BoxSteps::resize`]): no smaller than its pins need.
    pub fn size_box(&mut self, width: i64, height: i64, from_left: bool, from_top: bool) {
        let whole = |nm: i64| ((nm as f64) / (EDIT_STEP as f64)).round() as i64;
        self.relayout(|b| b.resize(whole(width), whole(height), from_left, from_top));
    }
}

/// The pins of a part as a draft's rows.
pub(crate) fn pin_rows(part: &Part) -> Vec<PinRow> {
    part.cp_pattern
        .iter()
        .map(|cp| PinRow {
            idx: cp.idx,
            name: cp.designation.as_str().to_owned(),
            role: cp
                .potential_role
                .as_ref()
                .map(crate::part_editor::stored_word)
                .unwrap_or_default(),
        })
        .collect()
}

impl Session {
    /// The draft's pins and device box over the part `p`: the CP pattern with the rows' names and potentials (a kept
    /// pin keeps its other members), the main template entry drawing every pin (a part without a template gets the
    /// black box as its main function), and the layout. A part whose pins come from elsewhere
    /// ([`PartDraft::pins_from`]) is left as it is.
    ///
    /// # Errors
    ///
    /// [`PartDraftError::Field`] on `pins` for a name that is no designation.
    pub(crate) fn apply_pins(&self, draft: &PartDraft, p: &mut Part) -> Result<(), PartDraftError> {
        if draft.pins_from().is_some() {
            return Ok(());
        }
        let bad = |why: String| {
            PartDraftError::Field(message(
                "pins",
                ew_commands::text_key!("error-part-value"),
                &[&why],
            ))
        };
        if draft.pins != pin_rows(&draft.original) {
            let mut pattern = Vec::with_capacity(draft.pins.len());
            for row in &draft.pins {
                let designation = Ident::from_input(row.name.trim()).map_err(|e| bad(e.to_string()))?;
                let role = (!row.role.is_empty())
                    .then(|| serde_json::from_value(serde_json::Value::String(row.role.clone())))
                    .transpose()
                    .map_err(|e| bad(e.to_string()))?;
                let cp = match draft.original.cp_pattern.iter().find(|c| c.idx == row.idx) {
                    Some(held) => CpData {
                        designation,
                        potential_role: role,
                        ..held.clone()
                    },
                    None => {
                        let mut cp: CpData = serde_json::from_value(serde_json::json!({
                            "idx": row.idx,
                            "designation": designation.as_str(),
                        }))
                        .map_err(|e| bad(e.to_string()))?;
                        cp.potential_role = role;
                        cp
                    }
                };
                pattern.push(cp);
            }
            p.cp_pattern = ew_schema::KeyedList::from_vec(pattern).map_err(|e| bad(e.to_string()))?;
            let pins: Vec<u16> = p.cp_pattern.iter().map(|c| c.idx).collect();
            if p.function_template.is_empty() {
                if !pins.is_empty()
                    && let Some(ew_schema::LibDocument::Fndef(def)) =
                        crate::part_new::starter_doc("ew.fn.blackbox")
                {
                    p.function_template = vec![TemplateFn {
                        fndef: crate::part_new::fndef_ref(&def),
                        main: true,
                        cps: pins,
                        extra: Default::default(),
                    }];
                    if p.category.is_none() {
                        p.category = Some(def.item.category.clone());
                    }
                }
            } else {
                // The main entry draws the box: it takes the new pins; no entry keeps a pin that is gone.
                let main = p.function_template.iter().position(|t| t.main).unwrap_or(0);
                for (i, entry) in p.function_template.iter_mut().enumerate() {
                    entry.cps.retain(|c| pins.contains(c));
                    if i == main {
                        let new: Vec<u16> = pins.iter().copied().filter(|c| !entry.cps.contains(c)).collect();
                        entry.cps.extend(new);
                    }
                }
            }
        }
        if draft.box_ != draft.original.box_ {
            p.box_.clone_from(&draft.box_);
        }
        Ok(())
    }

    /// The project's copy of library item `item` among its parts (the part a Save made or changed: a new part's
    /// item, a fork's item); `None` when the project holds none.
    pub fn part_copy_of_item(&self, item: Uuid) -> Option<Uuid> {
        let q = self.query();
        q.parts()
            .iter()
            .find(|(_, c)| c.item_id.as_uuid() == item)
            .map(|(id, _)| id.as_uuid())
    }

    /// The part copies the project holds.
    pub fn held_part_copies(&self) -> Vec<Uuid> {
        self.query().parts().iter().map(|(id, _)| id.as_uuid()).collect()
    }

    /// The boxes drawn of devices that use the part copy `copy`, each the drawing of a function the part's main
    /// template entry made: what a changed layout asks to redraw.
    pub fn boxes_of_part(&self, copy: Uuid) -> Vec<Id<Representation>> {
        let q = self.query();
        let Some(part) = q.parts().get(Id::from_uuid(copy)) else {
            return Vec::new();
        };
        let main = part
            .item
            .function_template
            .iter()
            .position(|t| t.main)
            .unwrap_or(0);
        let mut out: Vec<Id<Representation>> =
            q.reps()
                .iter()
                .filter(|(_, rep)| {
                    rep.box_.is_some()
                        && rep.functions.iter().any(|f| {
                            q.functions().get(*f).is_some_and(|function| {
                                function.template_entry.map(usize::from) == Some(main)
                                    && function.device.and_then(|d| q.devices().get(d)).is_some_and(|d| {
                                        d.parts.iter().any(|u| u.part.copy.as_uuid() == copy)
                                    })
                            })
                        })
                })
                .map(|(id, _)| id)
                .collect();
        out.sort();
        out
    }

    /// The `box.reshape` that draws the box `rep` as the layout of the part copy `copy` says, where it stands: its
    /// outline the layout's rectangle and its pins on their edges, on the page's grid. `None` for a part without a
    /// layout, a representation that is no box, or a box on a page that is gone.
    pub fn redraw_box_command(&self, copy: Uuid, rep: Id<Representation>) -> Option<AnyCommand> {
        let q = self.query();
        let part = q.parts().get(Id::from_uuid(copy))?;
        let layout = part.item.box_.as_ref()?;
        let record = q.reps().get(rep)?;
        record.box_.as_ref()?;
        let page = q.pages().get(record.page)?;
        let pins: Vec<u16> = part.item.cp_pattern.iter().map(|c| c.idx).collect();
        let step = page.grid.at(1)?.0;
        let (_, outline, cps) =
            steps(&pins, &[], Some(layout), step).on_page(page.grid, (record.pos.x.0, record.pos.y.0))?;
        let point = |p: (i64, i64)| Pt2::new(Nm(p.0), Nm(p.1));
        AnyCommand::of(&ReshapeBox {
            rep,
            outline: outline.into_iter().map(point).collect(),
            cps: Some(
                cps.into_iter()
                    .map(|(idx, pos, dir)| BoxCp {
                        idx,
                        pos: point(pos),
                        dir,
                        extra: Default::default(),
                    })
                    .collect(),
            ),
        })
        .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(idx: u16, side: Dir4, at: i64) -> PartBoxPin {
        PartBoxPin {
            idx,
            side,
            at: Nm(at),
            extra: Default::default(),
        }
    }

    /// A layout is read onto the grid as it says: each pin on its edge at its distance, the box its size; a pin too
    /// near the one before it moves one step on, and the box widens for a pin past its edge.
    #[test]
    fn a_layout_is_read_onto_the_grid_with_room_for_every_pin() {
        let layout = PartBox {
            width: Nm(16_000_000),
            height: Nm(12_000_000),
            pins: vec![
                pin(0, Dir4::Up, 4_000_000),
                pin(1, Dir4::Up, 4_000_000),
                pin(2, Dir4::Left, 8_000_000),
                pin(3, Dir4::Down, 24_000_000),
            ],
            extra: Default::default(),
        };
        let b = steps(&[0, 1, 2, 3, 4], &[], Some(&layout), EDIT_STEP);
        assert_eq!(
            b.pins,
            [
                (0, Dir4::Up, 1),
                (1, Dir4::Up, 2),
                (3, Dir4::Down, 6),
                // Not listed: on the bottom edge, after the others.
                (4, Dir4::Down, 7),
                (2, Dir4::Left, 2),
            ]
        );
        assert_eq!((b.w, b.h), (8, 3));
        // What the part stores reads back as itself.
        assert_eq!(
            steps(&[0, 1, 2, 3, 4], &[], Some(&b.layout(EDIT_STEP)), EDIT_STEP),
            b
        );
    }

    /// Without a layout the box is the black box tool's: the first half of the pins on top, the rest below, a
    /// source's outputs below.
    #[test]
    fn a_part_without_a_layout_has_its_pins_half_on_top_and_half_below() {
        let b = steps(&[0, 1, 2, 3, 4], &[], None, EDIT_STEP);
        let on = |side: Dir4| -> Vec<u16> { b.pins.iter().filter(|p| p.1 == side).map(|p| p.0).collect() };
        assert_eq!((on(Dir4::Up), on(Dir4::Down)), (vec![0, 1, 2], vec![3, 4]));
        let source = steps(&[0, 1, 2, 3], &[0, 1], None, EDIT_STEP);
        let below: Vec<u16> = source
            .pins
            .iter()
            .filter(|p| p.1 == Dir4::Down)
            .map(|p| p.0)
            .collect();
        assert_eq!(below, [0, 1]);
    }
}
