// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frame cells: `cell_of(point, page)` and `rep_anchor_cell(rep)` (doc 06 §5.1, FR-PAG-020, FR-SYM-010,
//! FR-XRF-005, D-14).
//!
//! # `cell_of`
//!
//! The page supplies its frame form ([`frame_grid`]) and its connection grid `g` (`Page.grid`). The point
//! is **snapped first**: its grid index `k = g.snap(x)` is evaluated on the exact rational position
//! `k·g.num/g.den`, so the 0.5 nm rounding of a stored coordinate cannot move an on-grid anchor across a
//! cell boundary (doc 06 §5.1 pitfall). With the drawing area starting at `x0 = a/b` and columns `w = c/d`
//! wide, the column is
//!
//! ```text
//! floor( (k·g.num·b·d − a·g.den·d) / (c·g.den·b) )        i128::div_euclid, exact
//! ```
//!
//! and likewise the row; a point on a boundary belongs to the right or lower cell. Every product is
//! checked: an overflow yields `None`, never a panic.
//!
//! # `rep_anchor_cell`
//!
//! The anchor point comes from [`SymbolDef::anchor`] in symbol coordinates ([`anchor_point`]), never from
//! rendered geometry (D-14); it is mapped by the representation's orientation and position and then looked
//! up with `cell_of` on the representation's page.

use std::fmt;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{AnchorPolicy, FormDef, FrameGrid, Page, Primitive, Representation, SymbolDef};
use ew_units::{Grid, Nm, Pt2};
use serde_json::Value;

use crate::Q;
use crate::query::{Policy, Query};

/// One cell of a page's frame grid.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Cell {
    /// The column number as printed: `start_index` + the 0-based column (0–16 on the QET frame).
    pub column: u32,
    /// The 0-based row.
    pub row: u32,
    /// The printed column label (`"16"`, or the frame's explicit label).
    pub column_label: String,
    /// The printed row label (`"H"`, or the frame's explicit label).
    pub row_label: String,
}

impl fmt::Display for Cell {
    /// Row then column, as cross-references print it: `H16`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.row_label, self.column_label)
    }
}

/// `cell_of(point, page)`: the frame cell of a page point, or `None` outside the drawing area or on a
/// page without a frame grid.
#[derive(Debug)]
pub struct CellOf;

impl Query for CellOf {
    type Key = (Id<Page>, Pt2);
    type Value = Option<Cell>;
    const FAMILY: &'static str = "cell_of";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<Cell>> {
        let (page, point) = *key;
        Arc::new(cell_of_page(q, page, point))
    }
}

/// `rep_anchor_cell(rep)`: the frame cell of a representation's anchor point, from the symbol's anchor
/// policy (never rendered geometry).
#[derive(Debug)]
pub struct RepAnchorCell;

impl Query for RepAnchorCell {
    type Key = Id<Representation>;
    type Value = Option<Cell>;
    const FAMILY: &'static str = "rep_anchor_cell";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<Cell>> {
        let Some(rep) = q.reps().get(*key) else {
            return Arc::new(None);
        };
        let Some(point) = anchor_point(q, *key) else {
            return Arc::new(None);
        };
        Arc::new(q.get::<CellOf>((rep.page, point)).as_ref().clone())
    }
}

// ---------------------------------------------------------------------------------------------------------
// The page's frame

/// The frame form of a page (doc 03 §5.1 `FrameDefaults`, FR-PAG-018): `Page.form`, else the entry of
/// `ProjectSettings.extra["frames"].per_page_type` whose `page_type` is the page's type, else its
/// `default`. The member is read leniently: anything malformed yields no form.
fn frame_form(q: &Q, page: Id<Page>) -> Option<Id<FormDef>> {
    let page = q.pages().get(page)?;
    if let Some(form) = page.form {
        return Some(form);
    }
    let project = q.project();
    let frames = project.settings.extra.get("frames")?.as_object()?;
    let kind = serde_json::to_value(&page.kind).ok();
    if let Some(entries) = frames.get("per_page_type") {
        for entry in entries.as_array()? {
            let Some(entry) = entry.as_object() else {
                continue;
            };
            if let (Some(Value::String(t)), Some(k)) = (entry.get("page_type"), kind.as_ref())
                && k.as_str() == Some(t.as_str())
            {
                return serde_json::from_value(entry.get("form")?.clone()).ok();
            }
        }
    }
    serde_json::from_value(frames.get("default")?.clone()).ok()
}

/// The frame grid (columns, rows, header band) of a page's frame form, cloned; `None` for a page without
/// a frame form or whose form has no grid.
pub fn frame_grid(q: &Q, page: Id<Page>) -> Option<FrameGrid> {
    let form = frame_form(q, page)?;
    q.forms().get(form)?.item.grid.clone()
}

// ---------------------------------------------------------------------------------------------------------
// cell_of

fn cell_of_page(q: &Q, page: Id<Page>, point: Pt2) -> Option<Cell> {
    let grid = frame_grid(q, page)?;
    let g = q.pages().get(page)?.grid;
    cell_in_grid(&grid, g, point)
}

/// The cell of `point` in `frame`, with the point snapped to the connection grid `g` first.
fn cell_in_grid(frame: &FrameGrid, g: Grid, point: Pt2) -> Option<Cell> {
    let (a, b) = frame
        .header_band
        .map_or((0, 1), |h| (i128::from(h.num()), i128::from(h.den())));
    let c = axis_cell(
        g.snap(point.x),
        g,
        (a, b),
        frame.columns.width,
        frame.columns.count,
    )?;
    let r = axis_cell(g.snap(point.y), g, (a, b), frame.rows.height, frame.rows.count)?;
    let column_label = if frame.columns.labels.is_empty() {
        (u32::from(frame.columns.start_index) + c).to_string()
    } else {
        frame.columns.labels.get(c as usize)?.as_str().to_owned()
    };
    let row_label = if frame.rows.labels.is_empty() {
        letter(r, frame.rows.skip_io)?
    } else {
        frame.rows.labels.get(r as usize)?.as_str().to_owned()
    };
    Some(Cell {
        column: u32::from(frame.columns.start_index) + c,
        row: r,
        column_label,
        row_label,
    })
}

/// The 0-based cell along one axis: `floor((k·g − origin) / step)` on exact rationals, if it lies in
/// `0..count`. `k` is the grid index of the point, `origin = a/b` nm, `step` the cell size.
fn axis_cell(k: i64, g: Grid, origin: (i128, i128), step: Grid, count: u16) -> Option<u32> {
    let (a, b) = origin;
    let (gn, gd) = (i128::from(g.num()), i128::from(g.den()));
    let (c, d) = (i128::from(step.num()), i128::from(step.den()));
    let x = i128::from(k).checked_mul(gn)?;
    let lhs = x.checked_mul(b)?.checked_mul(d)?;
    let rhs = a.checked_mul(gd)?.checked_mul(d)?;
    let den = c.checked_mul(gd)?.checked_mul(b)?;
    let index = lhs.checked_sub(rhs)?.div_euclid(den);
    if (0..i128::from(count)).contains(&index) {
        u32::try_from(index).ok()
    } else {
        None
    }
}

/// The `r`-th row letter of `A..Z`, skipping `I` and `O` when `skip_io`; `None` beyond `Z`.
fn letter(r: u32, skip_io: bool) -> Option<String> {
    let mut letters = ('A'..='Z').filter(|c| !(skip_io && matches!(c, 'I' | 'O')));
    letters.nth(r as usize).map(String::from)
}

// ---------------------------------------------------------------------------------------------------------
// rep_anchor_cell

/// The anchor point of a representation on its page, in nm: `rep.pos + rep.orient.apply(anchor)` with the
/// anchor taken from the symbol's [`AnchorPolicy`] (FR-SYM-010) in symbol coordinates; `None` for a missing
/// representation or symbol copy, or on overflow.
///
/// - `Origin`, `Unknown` and a `Slot` without a matching slot: the symbol origin.
/// - `Named(p)`: `p`.
/// - `Slot(kind)`: the `pos` of the first text slot of that kind. A representation's `SlotOverride.pos`
///   for that slot index replaces it and is read in **symbol coordinates**: like the symbol's own slot
///   position, it is a point of the symbol frame that the orientation then transforms (doc 07 §4.1 "the
///   anchor transforms with the variant's matrix like any point"; the override only moves the slot,
///   doc 06 §2.5).
/// - `GraphicCentre`: the centre of the integer bounding box of the symbol's graphics (lines, polylines,
///   polygons, rectangle corners, circle centre ± radius, ellipse and arc bounding rectangles, text
///   positions, image rectangles), `(min + max) / 2` with ties toward zero; the origin for no graphics.
pub fn anchor_point(q: &Q, rep: Id<Representation>) -> Option<Pt2> {
    let rep = q.reps().get(rep)?;
    let symbol = q.symbols().get(rep.symbol)?;
    let anchor = symbol_anchor(&symbol.item, rep)?;
    let offset = rep.orient.checked_apply(anchor)?;
    rep.pos.checked_add(offset)
}

/// The anchor in symbol coordinates.
fn symbol_anchor(symbol: &SymbolDef, rep: &Representation) -> Option<Pt2> {
    match &symbol.anchor {
        AnchorPolicy::Origin | AnchorPolicy::Unknown(_) => Some(Pt2::ORIGIN),
        AnchorPolicy::Named(p) => Some(*p),
        AnchorPolicy::Slot(kind) => {
            let Some((index, slot)) = symbol
                .text_slots
                .iter()
                .enumerate()
                .find(|(_, s)| s.kind == *kind)
            else {
                return Some(Pt2::ORIGIN);
            };
            let moved = u16::try_from(index).ok().and_then(|index| {
                rep.text_slots
                    .as_slice()
                    .iter()
                    .find(|o| o.slot == index)
                    .and_then(|o| o.pos)
            });
            Some(moved.unwrap_or(slot.pos))
        }
        AnchorPolicy::GraphicCentre => graphic_centre(&symbol.graphics),
    }
}

/// The centre of the integer bounding box of `graphics`; the origin if there is none, `None` on overflow.
fn graphic_centre(graphics: &[Primitive]) -> Option<Pt2> {
    /// An integer bounding box `[min x, min y, max x, max y]`, grown point by point.
    #[derive(Default)]
    struct Bb(Option<[i128; 4]>);

    impl Bb {
        fn add(&mut self, x: i128, y: i128) {
            let b = self.0.get_or_insert([x, y, x, y]);
            b[0] = b[0].min(x);
            b[1] = b[1].min(y);
            b[2] = b[2].max(x);
            b[3] = b[3].max(y);
        }

        fn pt(&mut self, p: Pt2) {
            self.add(i128::from(p.x.0), i128::from(p.y.0));
        }

        fn rect(&mut self, r: ew_units::Rect) {
            self.pt(r.min());
            self.pt(r.max());
        }
    }

    let mut bb = Bb::default();
    for g in graphics {
        match g {
            Primitive::Line(l) => {
                bb.pt(l.from);
                bb.pt(l.to);
            }
            Primitive::Polyline(p) => p.points.iter().for_each(|p| bb.pt(*p)),
            Primitive::Polygon(p) => p.points.iter().for_each(|p| bb.pt(*p)),
            Primitive::Rect(r) => bb.rect(r.rect.0),
            Primitive::Ellipse(e) => bb.rect(e.rect.0),
            Primitive::Arc(a) => bb.rect(a.rect.0),
            Primitive::Circle(c) => {
                let (x, y, r) = (
                    i128::from(c.centre.x.0),
                    i128::from(c.centre.y.0),
                    i128::from(c.r.0),
                );
                bb.add(x - r, y - r);
                bb.add(x + r, y + r);
            }
            Primitive::Text(t) => bb.pt(t.pos),
            Primitive::Image(i) => bb.rect(i.rect.0),
            Primitive::Unknown(_) => {}
        }
    }
    let Some([x0, y0, x1, y1]) = bb.0 else {
        return Some(Pt2::ORIGIN);
    };
    // Integer division truncates toward zero, which rounds the only possible fraction (a tie) toward zero.
    let x = i64::try_from((x0 + x1) / 2).ok()?;
    let y = i64::try_from((y0 + y1) / 2).ok()?;
    Some(Pt2::new(Nm(x), Nm(y)))
}
