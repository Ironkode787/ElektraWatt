// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frame cells: `cell_of(point, page)` and `rep_anchor_cell(rep)` (doc 06 §5.1, FR-PAG-020, FR-SYM-010,
//! FR-XRF-005, D-14).
//!
//! # `cell_of`
//!
//! The page supplies its frame grid ([`FrameGridOf`]) and its connection grid `g` (`Page.grid`). Each
//! coordinate `x` is evaluated as an exact rational `p/s` nm (doc 06 §5.1):
//!
//! - **on-grid**: when `x` lies within the 1 nm rounding window of its nearest grid position
//!   (`k = g.snap(x)`, `|x − g.at(k)| ≤ 1 nm`), the exact grid position `k·g.num/g.den`, so the 0.5 nm
//!   rounding of a stored coordinate cannot move an on-grid anchor across a cell boundary (the pitfall);
//! - **off-grid** (free texts, the cursor): the stored nm value itself (`x/1`). Snapping such a point would
//!   move it up to half a grid step and, near a boundary, into the neighbouring cell.
//!
//! With the drawing area starting at `x0 = a/b` and columns `w = c/d` wide, the column is
//!
//! ```text
//! floor( (p·b·d − a·s·d) / (c·s·b) )        i128::div_euclid, exact
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
use ew_schema::{
    AnchorPolicy, Extra, FormDef, FrameColumns, FrameGrid, FrameRows, Page, PageType, Primitive,
    Representation, SymbolDef,
};
use ew_units::{Grid, Nm, Pt2};
use serde_json::Value;

use crate::Q;
use crate::note::DeriveNote;
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

/// One `per_page_type` entry of the `frames` shim.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FrameTypeEntry {
    /// The page type's snake_case spelling.
    pub page_type: String,
    /// The frame form; `None` when the entry's `form` is missing or not a form ID (noted).
    pub form: Option<Id<FormDef>>,
}

/// The parsed `ProjectSettings.extra["frames"]` member (doc 03 §5.1 `FrameDefaults`), read once per
/// [`Q`]. Malformed parts are dropped here and reported as [`DeriveNote::MalformedShim`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FrameSettingsValue {
    /// The default frame form; `None` when absent or malformed (noted).
    pub default: Option<Id<FormDef>>,
    /// The per-page-type entries in order; the first with the page's type applies.
    pub per_page_type: Vec<FrameTypeEntry>,
}

/// `frame_settings()`: the `frames` shim, parsed once per [`Q`] (and its malformed members noted once).
#[derive(Debug)]
pub struct FrameSettings;

/// The member name notes use for the `frames` shim.
const FRAMES_MEMBER: &str = "settings.frames";

impl Query for FrameSettings {
    type Key = ();
    type Value = FrameSettingsValue;
    const FAMILY: &'static str = "frame_settings";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, _key: &()) -> Arc<FrameSettingsValue> {
        let malformed = |detail: &str| {
            q.note(DeriveNote::MalformedShim {
                member: FRAMES_MEMBER.into(),
                detail: detail.into(),
            });
        };
        let mut out = FrameSettingsValue::default();
        let project = q.project();
        let Some(frames) = project.settings.extra.get("frames") else {
            return Arc::new(out);
        };
        let Some(frames) = frames.as_object() else {
            malformed("frames is not an object");
            return Arc::new(out);
        };
        if let Some(default) = frames.get("default") {
            out.default = form_id(default);
            if out.default.is_none() {
                malformed("default is not a form ID");
            }
        }
        match frames.get("per_page_type") {
            None => {}
            Some(Value::Array(entries)) => {
                for entry in entries {
                    let Some(entry) = entry.as_object() else {
                        malformed("a per_page_type entry is not an object");
                        continue;
                    };
                    let Some(Value::String(page_type)) = entry.get("page_type") else {
                        malformed("a per_page_type entry has no page_type string");
                        continue;
                    };
                    let form = entry.get("form").and_then(form_id);
                    if form.is_none() {
                        malformed("a per_page_type entry's form is not a form ID");
                    }
                    out.per_page_type.push(FrameTypeEntry {
                        page_type: page_type.clone(),
                        form,
                    });
                }
            }
            Some(_) => malformed("per_page_type is not an array"),
        }
        Arc::new(out)
    }
}

fn form_id(value: &Value) -> Option<Id<FormDef>> {
    serde_json::from_value(value.clone()).ok()
}

/// The built-in default frame grid: the QET-compatible A4 frame, 17 columns by 8 rows below a header band
/// (doc 03 §5.1, doc 20 §17.1).
///
/// The constants mirror the `grid` member of `content/forms/ew.frame.qet-a4-landscape-grid.json`
/// (`header_band` 15 875 000/3 nm, 17 columns of 15 875 000 nm, 8 rows of 63 500 000/3 nm) and the edges
/// `ew-scene`'s `FrameGeometry` computes from it (`header + k × step`); a test compares it with the file.
/// The file's `ew.frame.line_width` layout hint is not geometry and is left out.
pub fn default_frame_grid() -> FrameGrid {
    let grid = |num, den| Grid::new(num, den).expect("a valid built-in step");
    FrameGrid {
        header_band: Some(grid(15_875_000, 3)),
        columns: FrameColumns {
            count: 17,
            start_index: 0,
            width: grid(15_875_000, 1),
            labels: Vec::new(),
            extra: Extra::new(),
        },
        rows: FrameRows {
            count: 8,
            labels: Vec::new(),
            skip_io: false,
            height: grid(63_500_000, 3),
            extra: Extra::new(),
        },
        extra: Extra::new(),
    }
}

/// `frame_grid(page)`: the frame grid of a page, evaluated once per page per [`Q`].
///
/// The frame form is `Page.form`, else the entry of the `frames` shim whose `page_type` is the page's type,
/// else its `default` (doc 03 §5.1 `FrameDefaults`, FR-PAG-018). A malformed matching entry falls through
/// to the `default`, a malformed `default` to the built-in default (see [`FrameSettings`]). When no form is
/// configured at all the built-in default grid applies ([`default_frame_grid`]) and the query pushes
/// [`DeriveNote::DefaultFrame`], except on title, table-of-contents and divider pages, whose default is the
/// no-grid frame variant (`None`, no note; doc 03 §5.1). A configured form without a `grid` member, or whose copy is missing, has no
/// cells (`None`), as has a page that does not exist.
#[derive(Debug)]
pub struct FrameGridOf;

impl Query for FrameGridOf {
    type Key = Id<Page>;
    type Value = Option<FrameGrid>;
    const FAMILY: &'static str = "frame_grid";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Id<Page>) -> Arc<Option<FrameGrid>> {
        let Some(page) = q.pages().get(*key) else {
            return Arc::new(None);
        };
        let form = page.form.or_else(|| {
            let settings = q.get::<FrameSettings>(());
            let kind = serde_json::to_value(&page.kind).ok();
            let of_type = kind.as_ref().and_then(Value::as_str).and_then(|kind| {
                settings
                    .per_page_type
                    .iter()
                    .find(|e| e.page_type == kind)
                    .and_then(|e| e.form)
            });
            of_type.or(settings.default)
        });
        let Some(form) = form else {
            // Title, table-of-contents and divider pages use the no-grid frame variant (doc 03 §5.1).
            if matches!(
                page.kind,
                PageType::TitlePage | PageType::TableOfContents | PageType::SectionDivider
            ) {
                return Arc::new(None);
            }
            q.note(DeriveNote::DefaultFrame);
            return Arc::new(Some(default_frame_grid()));
        };
        Arc::new(q.forms().get(form).and_then(|f| f.item.grid.clone()))
    }
}

/// The frame grid (columns, rows, header band) of a page, cloned; see [`FrameGridOf`].
pub fn frame_grid(q: &Q, page: Id<Page>) -> Option<FrameGrid> {
    (*q.get::<FrameGridOf>(page)).clone()
}

// ---------------------------------------------------------------------------------------------------------
// cell_of

fn cell_of_page(q: &Q, page: Id<Page>, point: Pt2) -> Option<Cell> {
    let grid = q.get::<FrameGridOf>(page);
    let g = q.pages().get(page)?.grid;
    cell_in_grid(grid.as_ref().as_ref()?, g, point)
}

/// The cell of `point` in `frame`; each coordinate is read on the connection grid `g` when it lies within
/// the rounding window of a grid position, else as stored ([`exact_coordinate`]).
fn cell_in_grid(frame: &FrameGrid, g: Grid, point: Pt2) -> Option<Cell> {
    let (a, b) = frame
        .header_band
        .map_or((0, 1), |h| (i128::from(h.num()), i128::from(h.den())));
    let c = axis_cell(
        exact_coordinate(point.x, g)?,
        (a, b),
        frame.columns.width,
        frame.columns.count,
    )?;
    let r = axis_cell(
        exact_coordinate(point.y, g)?,
        (a, b),
        frame.rows.height,
        frame.rows.count,
    )?;
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

/// The half-width of the rounding window, in nm: a stored coordinate within it of a grid position is that
/// grid position (doc 06 §5.1).
const SNAP_WINDOW_NM: i128 = 1;

/// A coordinate as the exact rational `p/s` nm (`s > 0`) that the lookup evaluates: the exact grid position
/// `k·g.num/g.den` when `x` is within [`SNAP_WINDOW_NM`] of `g.at(k)` for `k = g.snap(x)`, else `x/1`.
/// `None` only on overflow.
fn exact_coordinate(x: Nm, g: Grid) -> Option<(i128, i128)> {
    let k = g.snap(x);
    match g.at(k) {
        Some(at) if (i128::from(x.0) - i128::from(at.0)).abs() <= SNAP_WINDOW_NM => Some((
            i128::from(k).checked_mul(i128::from(g.num()))?,
            i128::from(g.den()),
        )),
        _ => Some((i128::from(x.0), 1)),
    }
}

/// The 0-based cell along one axis: `floor((x − origin) / step)` on exact rationals, if it lies in
/// `0..count`. `x = p/s` nm (from [`exact_coordinate`]), `origin = a/b` nm, `step` the cell size.
fn axis_cell(x: (i128, i128), origin: (i128, i128), step: Grid, count: u16) -> Option<u32> {
    let (a, b) = origin;
    let (x, gd) = x;
    let (c, d) = (i128::from(step.num()), i128::from(step.den()));
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
/// anchor taken from the representation's [`AnchorPolicy`] override, else its symbol's (FR-SYM-010), in
/// symbol coordinates; `None` for a missing representation or symbol copy, or on overflow.
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

/// The anchor in symbol coordinates, from the representation's own policy (`Representation.anchor`, set by
/// `rep.set_anchor_policy`) or else its symbol copy's.
fn symbol_anchor(symbol: &SymbolDef, rep: &Representation) -> Option<Pt2> {
    match rep.anchor.as_ref().unwrap_or(&symbol.anchor) {
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
