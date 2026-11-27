// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The template for drilling of a surface (doc 13 §9.1), as numbers: the corner it lies in, what it covers and the
//! paper sheets it takes, which of them print and their numbers. The scene draws a sheet from it; the Drilling… dialog
//! counts the sheets.
//!
//! The template lies in **the corner nearest the holes** (the corner whose two edges enclose the holes in the smallest
//! rectangle; on a tie the first of bottom left, bottom right, top left, top right) and covers the surface from there to
//! 20 mm past the farthest hole's edge, within the sheet. A paper sheet holds its paper less 10 mm all round and a 12 mm
//! band at its foot; neighbouring sheets overlap by 10 mm. The orientation that needs the fewer sheets is taken
//! (portrait on a tie). A sheet that holds no part of a hole and neither of the two edges at the corner is not printed;
//! the printed sheets are numbered row by row from the corner.

use ew_ids::Id;
use ew_schema::{Corner, MountingSurface};
use ew_units::{Nm, Pt2, Rect};

use super::holes::SurfaceFeatures;
use crate::Q;

/// The margin of a paper sheet all round.
pub const TEMPLATE_MARGIN: Nm = Nm(10_000_000);
/// The band at a sheet's foot.
pub const TEMPLATE_BAND: Nm = Nm(12_000_000);
/// How far neighbouring sheets overlap.
pub const TEMPLATE_OVERLAP: Nm = Nm(10_000_000);
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
    q.get::<SurfaceFeatures>(surface)
        .iter()
        .filter_map(|f| {
            let (w, h) = f.made.extent();
            let a = Pt2::new(
                Nm(f.at.x.0.checked_sub(w.0 / 2)?),
                Nm(f.at.y.0.checked_sub(h.0 / 2)?),
            );
            let b = Pt2::new(Nm(a.x.0.checked_add(w.0)?), Nm(a.y.0.checked_add(h.0)?));
            Some(Rect::from_points(a, b))
        })
        .collect()
}

/// The template of `surface` on `paper` (doc 13 §9.1, the module documentation); `None` for a surface without holes or
/// a sheet.
pub fn template_sheets(q: &Q, surface: Id<MountingSurface>, paper: Paper) -> Option<TemplateSheets> {
    let s = q.table::<MountingSurface>().get(surface)?;
    let xs = s.usable.iter().map(|p| p.x.0);
    let ys = s.usable.iter().map(|p| p.y.0);
    let sheet = Rect::from_points(
        Pt2::new(Nm(xs.clone().min()?), Nm(ys.clone().min()?)),
        Pt2::new(Nm(xs.max()?), Nm(ys.max()?)),
    );
    let holes = hole_boxes(q, surface);
    let first = holes.first()?;
    let span = holes.iter().fold(*first, |a, b| a.union(*b));
    // The corner whose two edges enclose the holes in the smallest rectangle.
    let (lo, hi) = (sheet.min(), sheet.max());
    let reach = |left: bool, top: bool| {
        let w = if left {
            span.max().x.0 - lo.x.0
        } else {
            hi.x.0 - span.min().x.0
        };
        let h = if top {
            hi.y.0 - span.min().y.0
        } else {
            span.max().y.0 - lo.y.0
        };
        i128::from(w) * i128::from(h)
    };
    let corner = [
        (Corner::BottomLeft, true, false),
        (Corner::BottomRight, false, false),
        (Corner::TopLeft, true, true),
        (Corner::TopRight, false, true),
    ]
    .into_iter()
    .min_by_key(|(_, l, t)| reach(*l, *t))
    .map(|(c, _, _)| c)?;
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
    // The sheets each orientation needs; the fewer, portrait on a tie.
    let (short, long) = paper.sides();
    let count = |len: i64, room: i64| -> Option<u32> {
        let step = room - TEMPLATE_OVERLAP.0;
        if room <= 0 || step <= 0 {
            return None;
        }
        let more = (len - room).max(0);
        u32::try_from(1 + (more + step - 1) / step).ok()
    };
    let lay = |pw: Nm, ph: Nm| -> Option<((Nm, Nm), u32, u32)> {
        let room_w = pw.0 - 2 * TEMPLATE_MARGIN.0;
        let room_h = ph.0 - 2 * TEMPLATE_MARGIN.0 - TEMPLATE_BAND.0;
        Some((
            (pw, ph),
            count(cover.height().0, room_h)?,
            count(cover.width().0, room_w)?,
        ))
    };
    let portrait = lay(short, long)?;
    let landscape = lay(long, short)?;
    let ((pw, ph), rows, cols) = if landscape.1 * landscape.2 < portrait.1 * portrait.2 {
        landscape
    } else {
        portrait
    };
    let room_w = pw.0 - 2 * TEMPLATE_MARGIN.0;
    let room_h = ph.0 - 2 * TEMPLATE_MARGIN.0 - TEMPLATE_BAND.0;
    let (step_w, step_h) = (room_w - TEMPLATE_OVERLAP.0, room_h - TEMPLATE_OVERLAP.0);
    let mut tiles = Vec::new();
    let mut number = 0;
    for row in 0..rows {
        for col in 0..cols {
            let (c, r) = (i64::from(col), i64::from(row));
            let (ax, bx) = if left {
                (x0 + c * step_w, x0 + c * step_w + room_w)
            } else {
                (x1 - c * step_w - room_w, x1 - c * step_w)
            };
            let (ay, by) = if top {
                (y1 - r * step_h - room_h, y1 - r * step_h)
            } else {
                (y0 + r * step_h, y0 + r * step_h + room_h)
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
    Some(TemplateSheets {
        corner,
        sheet,
        cover,
        paper: (pw, ph),
        rows,
        cols,
        tiles,
    })
}
