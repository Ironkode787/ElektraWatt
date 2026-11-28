// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The template for drilling of a surface (doc 13 §9.1), as numbers: the corner it lies in, what it covers and the
//! paper sheets it takes, which of them print and their numbers. The scene draws a sheet from it; the Drilling… dialog
//! counts the sheets.
//!
//! The template lies in **the corner the hole table measures from** (the surface's datum corner, `surface_datum`), so
//! that it is laid against the two edges every number is measured from, and covers the surface from there to 20 mm past
//! the farthest hole's edge, within the sheet. A paper sheet holds its paper less 10 mm all round and a 12 mm band at its
//! foot; neighbouring sheets overlap by 10 mm, what lines two sheets up. Where a join would cut through a hole a sheet
//! can hold, the sheet ends earlier, so that the hole stands whole on the next one with its centre cross. Every hole
//! stands whole on one sheet or is drawn across the sheets, each marking its centre and its outline's corners, and the
//! first sheet names it: a hole larger than a sheet, and one a join cuts because no join clears it and its neighbour
//! (two holes whose spans overlap across a join). Every surface with holes gets its template. The orientation that needs the fewer sheets is taken (portrait on a tie). A
//! sheet that holds no part of a hole and neither of the two edges at the corner is not printed; the printed sheets are
//! numbered row by row from the corner.

use ew_ids::Id;
use ew_schema::{Corner, MountingSurface};
use ew_units::{Nm, Pt2, Rect};

use super::holes::SurfaceFeatures;
use crate::Q;

/// The margin of a paper sheet all round.
pub const TEMPLATE_MARGIN: Nm = Nm(10_000_000);
/// The band at a sheet's foot.
pub const TEMPLATE_BAND: Nm = Nm(12_000_000);
/// How far neighbouring sheets overlap: what a person needs to line two sheets up.
pub const TEMPLATE_OVERLAP: Nm = Nm(10_000_000);
/// How far a hole's centre cross reaches past its edge on a sheet.
pub const TEMPLATE_CROSS: Nm = Nm(4_000_000);
/// How far the template reaches past the farthest hole's edge.
pub const TEMPLATE_PAST: Nm = Nm(20_000_000);

/// A paper size a template prints on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Paper {
    /// ISO A4, 210 × 297 mm.
    #[default]
    A4,
    /// ISO A3, 297 × 420 mm.
    A3,
}

impl Paper {
    /// The stored words of every paper, in the order a choice lists them.
    pub const WORDS: [&'static str; 2] = ["a4", "a3"];

    /// Its short and its long side.
    pub const fn sides(self) -> (Nm, Nm) {
        match self {
            Paper::A4 => (Nm(210_000_000), Nm(297_000_000)),
            Paper::A3 => (Nm(297_000_000), Nm(420_000_000)),
        }
    }

    /// Its stored word: `a4`, `a3`.
    pub const fn word(self) -> &'static str {
        match self {
            Paper::A4 => "a4",
            Paper::A3 => "a3",
        }
    }

    /// The paper of a word (`a4`, `A3`); `None` for another.
    pub fn of_word(word: &str) -> Option<Paper> {
        match word.trim().to_ascii_lowercase().as_str() {
            "a4" => Some(Paper::A4),
            "a3" => Some(Paper::A3),
            _ => None,
        }
    }
}

/// One sheet of a template.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplateTile {
    /// Its row from the corner (0 at the corner).
    pub row: u32,
    /// Its column from the corner.
    pub col: u32,
    /// The part of the surface it shows, in the surface frame.
    pub area: Rect,
    /// Its number among the printed sheets (1 …); `None`: not printed.
    pub number: Option<u32>,
}

/// The template of a surface on one paper size.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplateSheets {
    /// The corner it lies in.
    pub corner: Corner,
    /// The surface's sheet, in its frame.
    pub sheet: Rect,
    /// What it covers, in the surface frame.
    pub cover: Rect,
    /// The paper's width and height as the sheets lie (landscape: the long side across).
    pub paper: (Nm, Nm),
    /// The rows and the columns of sheets.
    pub rows: u32,
    /// The columns.
    pub cols: u32,
    /// Every sheet, row by row from the corner.
    pub tiles: Vec<TemplateTile>,
    /// The numbers of the holes no sheet holds whole (larger than a sheet, or cut by a join that could not move clear
    /// of them): drawn across the sheets, each sheet marks their centre and their outline's corners.
    pub larger: Vec<u32>,
}

impl TemplateSheets {
    /// How many sheets print.
    pub fn printed(&self) -> usize {
        self.tiles.iter().filter(|t| t.number.is_some()).count()
    }

    /// Whether the corner's vertical edge is the sheet's left one (else its right one).
    pub fn left(&self) -> bool {
        matches!(
            self.corner,
            Corner::BottomLeft | Corner::TopLeft | Corner::Unknown(_)
        )
    }

    /// Whether the corner's horizontal edge is the sheet's top one (else its bottom one).
    pub fn top(&self) -> bool {
        matches!(self.corner, Corner::TopLeft | Corner::TopRight)
    }
}

/// The holes of `surface` as rectangles in its frame (their made size about their centre).
pub fn hole_boxes(q: &Q, surface: Id<MountingSurface>) -> Vec<Rect> {
    numbered_boxes(q, surface).into_iter().map(|(_, r)| r).collect()
}

/// [`hole_boxes`] with each hole's number.
fn numbered_boxes(q: &Q, surface: Id<MountingSurface>) -> Vec<(u32, Rect)> {
    q.get::<SurfaceFeatures>(surface)
        .iter()
        .filter_map(|f| {
            let (w, h) = f.made.extent();
            let a = Pt2::new(
                Nm(f.at.x.0.checked_sub(w.0 / 2)?),
                Nm(f.at.y.0.checked_sub(h.0 / 2)?),
            );
            let b = Pt2::new(Nm(a.x.0.checked_add(w.0)?), Nm(a.y.0.checked_add(h.0)?));
            Some((f.no, Rect::from_points(a, b)))
        })
        .collect()
}

/// Whether the drawings for drilling of `surface` can be made: its sheet's corners, its edge margin, and each hole's
/// centre and size lie within ±[`MAX_SAFE_INTEGER`](ew_units::MAX_SAFE_INTEGER) nm (about 9000 km), so that the sums
/// and differences the template and the DXF take of a few of them stay inside `i64`. A file may hold any coordinate; a
/// surface beyond that range is offered no template and no DXF. `false` for a missing surface.
pub fn drawable(q: &Q, surface: Id<MountingSurface>) -> bool {
    let Some(s) = q.table::<MountingSurface>().get(surface) else {
        return false;
    };
    let ok = |n: Nm| n.0.unsigned_abs() <= ew_units::MAX_SAFE_INTEGER.unsigned_abs();
    s.usable.iter().all(|p| ok(p.x) && ok(p.y))
        && s.edge_margin.is_none_or(ok)
        && q.get::<SurfaceFeatures>(surface).iter().all(|f| {
            let (w, h) = f.made.extent();
            ok(f.at.x) && ok(f.at.y) && ok(w) && ok(h)
        })
}

/// The template of `surface` on `paper` (doc 13 §9.1, the module documentation); `None` for a surface without holes or
/// a sheet, or one not [`drawable`].
pub fn template_sheets(q: &Q, surface: Id<MountingSurface>, paper: Paper) -> Option<TemplateSheets> {
    if !drawable(q, surface) {
        return None;
    }
    let s = q.table::<MountingSurface>().get(surface)?;
    let xs = s.usable.iter().map(|p| p.x.0);
    let ys = s.usable.iter().map(|p| p.y.0);
    let sheet = Rect::from_points(
        Pt2::new(Nm(xs.clone().min()?), Nm(ys.clone().min()?)),
        Pt2::new(Nm(xs.max()?), Nm(ys.max()?)),
    );
    let numbered = numbered_boxes(q, surface);
    let holes: Vec<Rect> = numbered.iter().map(|(_, r)| *r).collect();
    let first = holes.first()?;
    let span = holes.iter().fold(*first, |a, b| a.union(*b));
    let (lo, hi) = (sheet.min(), sheet.max());
    // The corner the hole table measures from; one of a newer format as the bottom-left one.
    let corner = match super::surfaces::surface_datum(q, s) {
        Corner::Unknown(_) => Corner::BottomLeft,
        c => c,
    };
    let (left, top) = (
        matches!(corner, Corner::BottomLeft | Corner::TopLeft),
        matches!(corner, Corner::TopLeft | Corner::TopRight),
    );
    let past = TEMPLATE_PAST.0;
    let x0 = if left {
        lo.x.0
    } else {
        (span.min().x.0 - past).max(lo.x.0)
    };
    let x1 = if left {
        (span.max().x.0 + past).min(hi.x.0)
    } else {
        hi.x.0
    };
    let y0 = if top {
        (span.min().y.0 - past).max(lo.y.0)
    } else {
        lo.y.0
    };
    let y1 = if top {
        hi.y.0
    } else {
        (span.max().y.0 + past).min(hi.y.0)
    };
    let cover = Rect::from_points(Pt2::new(Nm(x0), Nm(y0)), Pt2::new(Nm(x1), Nm(y1)));
    // Each hole's stretch across and up, measured from the corner's two edges.
    let across: Vec<(i64, i64)> = holes
        .iter()
        .map(|h| {
            if left {
                (h.min().x.0 - x0, h.max().x.0 - x0)
            } else {
                (x1 - h.max().x.0, x1 - h.min().x.0)
            }
        })
        .collect();
    let up: Vec<(i64, i64)> = holes
        .iter()
        .map(|h| {
            if top {
                (y1 - h.max().y.0, y1 - h.min().y.0)
            } else {
                (h.min().y.0 - y0, h.max().y.0 - y0)
            }
        })
        .collect();
    // The sheets each orientation needs; the fewer, portrait on a tie.
    let (short, long) = paper.sides();
    let lay = |pw: Nm, ph: Nm| {
        let room_w = pw.0 - 2 * TEMPLATE_MARGIN.0;
        let room_h = ph.0 - 2 * TEMPLATE_MARGIN.0 - TEMPLATE_BAND.0;
        (
            (pw, ph),
            (room_w, room_h),
            stretches(cover.width().0, room_w, &across),
            stretches(cover.height().0, room_h, &up),
        )
    };
    let portrait = lay(short, long);
    let landscape = lay(long, short);
    let size = |l: &(_, _, Vec<_>, Vec<_>)| l.2.len() * l.3.len();
    let ((pw, ph), (room_w, room_h), columns, lines) = if size(&landscape) < size(&portrait) {
        landscape
    } else {
        portrait
    };
    let (rows, cols) = (
        u32::try_from(lines.len()).unwrap_or(u32::MAX),
        u32::try_from(columns.len()).unwrap_or(u32::MAX),
    );
    // The holes no sheet holds whole, across or up: they are drawn across the sheets.
    let mut tiles = Vec::new();
    let mut number = 0;
    for (row, &(r0, r1)) in (0..).zip(&lines) {
        for (col, &(c0, c1)) in (0..).zip(&columns) {
            let (ax, bx) = if left {
                (x0 + c0, x0 + c1)
            } else {
                (x1 - c1, x1 - c0)
            };
            let (ay, by) = if top {
                (y1 - r1, y1 - r0)
            } else {
                (y0 + r0, y0 + r1)
            };
            let area = Rect::from_points(Pt2::new(Nm(ax), Nm(ay)), Pt2::new(Nm(bx), Nm(by)));
            let holds = holes.iter().any(|h| h.intersects(area));
            let printed = holds || row == 0 || col == 0;
            tiles.push(TemplateTile {
                row,
                col,
                area,
                number: printed.then(|| {
                    number += 1;
                    number
                }),
            });
        }
    }
    // Every hole stands whole on one sheet, or it is drawn across the sheets: one larger than a sheet, and one the moved
    // joins could not keep whole (two holes whose spans overlap across a join).
    let inside = |a: Rect, h: &Rect| {
        a.min().x <= h.min().x && a.min().y <= h.min().y && h.max().x <= a.max().x && h.max().y <= a.max().y
    };
    let larger = numbered
        .iter()
        .filter(|(_, h)| !tiles.iter().any(|t: &TemplateTile| inside(t.area, h)))
        .map(|(no, _)| *no)
        .collect();
    Some(TemplateSheets {
        corner,
        sheet,
        cover,
        paper: (pw, ph),
        rows,
        cols,
        tiles,
        larger,
    })
}

/// The stretches the sheets of one axis show, measured from the corner's edge: from 0 on until one reaches `len`, each
/// `room` long at most and overlapping the next by [`TEMPLATE_OVERLAP`]. Where a join would cut through a hole
/// (`holes`, as stretches of the axis) that one sheet can hold with its centre cross, or at least without it, the sheet
/// ends earlier, so that the next one holds the hole whole; a hole longer than `room` is cut.
fn stretches(len: i64, room: i64, holes: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let overlap = TEMPLATE_OVERLAP.0;
    let room = room.max(2 * overlap);
    let whole: Vec<(i64, i64)> = holes
        .iter()
        .filter_map(|&(a, b)| {
            let spare = room - (b - a);
            let c = TEMPLATE_CROSS.0.min(spare / 2);
            (spare >= 0).then_some((a - c, b + c))
        })
        .collect();
    let mut out = Vec::new();
    let mut start = 0;
    loop {
        let mut end = start + room;
        if end >= len {
            out.push((start, end));
            return out;
        }
        // Each step ends the sheet before a hole beyond its start, so the steps end.
        while let Some(&(a, _)) = whole
            .iter()
            .find(|&&(a, b)| a > start && a < end - overlap && b > end)
        {
            end = a + overlap;
        }
        out.push((start, end));
        start = end - overlap;
    }
}
