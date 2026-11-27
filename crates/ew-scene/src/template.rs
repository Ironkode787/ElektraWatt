// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One sheet of a template for drilling (doc 13 §9.1) as a display list at full size (1:1 on its paper, y down): the
//! part of the surface it shows inside a 10 mm margin, its holes at true size with a centre cross 4 mm past their edge
//! and their words, the two edges at the template's corner thick with their words, the edge margin thin dashed, the
//! hinge's ticks and the datum's "0" where they fall on it, the strips its neighbours print again tinted, its cut edges
//! dashed, the crosses where four sheets meet, and the band at its foot (its words, a map of the sheets and a 100 mm
//! bar). No device, label, carrier, frame or title block: it is a template for holes.
//!
//! The numbers come from `ew_derive::layout::template_sheets`; the words from the caller, in the project's language.

use ew_dl::{Brush, Color, DashPattern, DisplayList, EntityRef, Item, Path, PathBuilder, Pen, Stroke};
use ew_schema::Align;
use ew_text::{FontSet, TextStyle};
use ew_units::{Angle2, Nm, Pt2, Rect};

use crate::page::{Builder, TextSpec};
use crate::panel::LAYOUT_LABEL_STYLE;

/// The margin of a sheet all round (the printed part's edge).
const MARGIN: Nm = Nm(10_000_000);
/// The band at a sheet's foot.
const BAND: Nm = Nm(12_000_000);
/// The strip neighbouring sheets share.
const OVERLAP: Nm = Nm(10_000_000);
/// How far a hole's centre cross reaches past its edge.
const CROSS_PAST: Nm = Nm(4_000_000);
/// The text of a hole's words and of the band.
const TEXT: Nm = Nm(2_000_000);
/// The tint of a strip a neighbour prints again.
const TINT: Color = Color::from_rgb8(0xe8, 0xec, 0xf2);

/// One hole of a template sheet, in the surface frame.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplateHole {
    /// Its centre.
    pub at: Pt2,
    /// Its size across: a round hole's diameter twice, a rectangle's width and height.
    pub size: (Nm, Nm),
    /// A round hole (else a rectangle).
    pub round: bool,
    /// Its words: number, how it is made and its owner ("H3 · Ø 22.3 · -102S1").
    pub words: String,
}

/// One sheet of a template, as the caller gives it (the module documentation).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TemplateView {
    /// The paper's width and height.
    pub paper: (Nm, Nm),
    /// The part of the surface the sheet shows, in the surface frame.
    pub area: Rect,
    /// The holes.
    pub holes: Vec<TemplateHole>,
    /// The two edges at the template's corner, each from one end to the other in the surface frame, with its words
    /// ("Left edge of the door: lay this edge on it").
    pub edges: Vec<(Pt2, Pt2, String)>,
    /// The edge margin's inner line.
    pub margin: Option<Rect>,
    /// The hinge's edge, from one end to the other, ticked.
    pub hinge: Option<(Pt2, Pt2)>,
    /// The datum's corner.
    pub datum: Option<Pt2>,
    /// A neighbour on the right and one above: their strips tinted on this sheet.
    pub tint: (bool, bool),
    /// A neighbour on the left and one below: this sheet's cut edges, dashed.
    pub cut: (bool, bool),
    /// The points where four sheets meet, in the surface frame.
    pub joins: Vec<Pt2>,
    /// The band's lines, top down.
    pub band: Vec<String>,
    /// The map: rows, columns, this sheet's row and column (from the map's top-left) and the printed ones.
    pub map: (u32, u32, (u32, u32), Vec<(u32, u32)>),
    /// The scale bar's words.
    pub bar: String,
}

/// The display list of the sheet `v` (the module documentation).
pub fn template_sheet(v: &TemplateView, fonts: &FontSet, styles: &ew_text::TextStyleTable) -> DisplayList {
    let mut b = Builder::new(fonts, styles);
    let style = TextStyle {
        size: TEXT,
        ..b.style_quiet(LAYOUT_LABEL_STYLE)
    };
    let (pw, ph) = v.paper;
    let a = v.area;
    // A surface point on the paper.
    let at = |p: Pt2| Pt2::new(Nm(MARGIN.0 + p.x.0 - a.min().x.0), Nm(MARGIN.0 + a.max().y.0 - p.y.0));
    let content = Rect::from_points(at(Pt2::new(a.min().x, a.max().y)), at(Pt2::new(a.max().x, a.min().y)));
    let thin = Stroke::pen(Pen::P013);
    let push = |b: &mut Builder<'_>, path: Path, stroke: Option<Stroke>, fill: Option<Brush>| {
        b.push(Item::path(path, stroke, fill));
    };
    // The strips the neighbours print again, tinted, under everything.
    let (cmin, cmax) = (content.min(), content.max());
    if v.tint.0 {
        let r = Rect::from_points(Pt2::new(Nm(cmax.x.0 - OVERLAP.0), cmin.y), cmax);
        push(&mut b, Path::rect(r), None, Some(Brush::Solid(TINT)));
    }
    if v.tint.1 {
        let r = Rect::from_points(cmin, Pt2::new(cmax.x, Nm(cmin.y.0 + OVERLAP.0)));
        push(&mut b, Path::rect(r), None, Some(Brush::Solid(TINT)));
    }
    let dashed = || Some(thin.clone().dashed(DashPattern::Dash42));
    if v.cut.0 {
        push(&mut b, Path::line(cmin, Pt2::new(cmin.x, cmax.y)), dashed(), None);
    }
    if v.cut.1 {
        push(&mut b, Path::line(Pt2::new(cmin.x, cmax.y), cmax), dashed(), None);
    }
    // A segment of the surface within the sheet, on the paper.
    let clipped = |p: Pt2, q: Pt2| -> Option<(Pt2, Pt2)> {
        let (x0, x1) = (p.x.0.min(q.x.0).max(a.min().x.0), p.x.0.max(q.x.0).min(a.max().x.0));
        let (y0, y1) = (p.y.0.min(q.y.0).max(a.min().y.0), p.y.0.max(q.y.0).min(a.max().y.0));
        (x0 <= x1 && y0 <= y1).then(|| (at(Pt2::new(Nm(x0), Nm(y0))), at(Pt2::new(Nm(x1), Nm(y1)))))
    };
    if let Some(m) = v.margin {
        let (lo, hi) = (m.min(), m.max());
        for (p, q) in [
            (lo, Pt2::new(hi.x, lo.y)),
            (Pt2::new(hi.x, lo.y), hi),
            (Pt2::new(lo.x, hi.y), hi),
            (lo, Pt2::new(lo.x, hi.y)),
        ] {
            if let Some((p, q)) = clipped(p, q) {
                push(&mut b, Path::line(p, q), dashed(), None);
            }
        }
    }
    if let Some((p, q)) = v.hinge {
        // Ticks every 50 mm along the hinge edge, 3 mm long, outward of nothing: across the edge.
        let vertical = p.x == q.x;
        let (from, to) = if vertical { (p.y.0.min(q.y.0), p.y.0.max(q.y.0)) } else { (p.x.0.min(q.x.0), p.x.0.max(q.x.0)) };
        let mut t = from + 50_000_000;
        while t < to {
            let c = if vertical { Pt2::new(p.x, Nm(t)) } else { Pt2::new(Nm(t), p.y) };
            if a.contains(c) {
                let c = at(c);
                let (d0, d1) = if vertical {
                    (Pt2::new(Nm(c.x.0 - 1_500_000), c.y), Pt2::new(Nm(c.x.0 + 1_500_000), c.y))
                } else {
                    (Pt2::new(c.x, Nm(c.y.0 - 1_500_000)), Pt2::new(c.x, Nm(c.y.0 + 1_500_000)))
                };
                push(&mut b, Path::line(d0, d1), Some(thin.clone()), None);
            }
            t += 50_000_000;
        }
    }
    let text = |b: &mut Builder<'_>, words: &str, p: Pt2, angle: Angle2, align: &Align| {
        let shaped = fonts.shape(words, &style, None);
        b.text_shaped(
            &TextSpec {
                text: words,
                at: p,
                angle,
                align,
                style_key: LAYOUT_LABEL_STYLE,
                entity: EntityRef(ew_ids::Uuid::nil()),
                link: None,
                items: &[],
            },
            &style,
            &shaped,
        );
    };
    // The two edges at the corner, thick, with their words inside the sheet beside them.
    for (p, q, words) in &v.edges {
        if let Some((p0, q0)) = clipped(*p, *q) {
            push(&mut b, Path::line(p0, q0), Some(Stroke::pen(Pen::P052)), None);
            let mid = Pt2::new(Nm((p0.x.0 + q0.x.0) / 2), Nm((p0.y.0 + q0.y.0) / 2));
            if p0.x == q0.x {
                let inside = if p0.x.0 <= cmin.x.0 + 1 { 2_500_000 } else { -2_500_000 };
                text(&mut b, words, Pt2::new(Nm(mid.x.0 + inside), mid.y), Angle2(90_000), &Align::Centre);
            } else {
                let inside = if p0.y.0 <= cmin.y.0 + 1 { 2_500_000 } else { -2_500_000 };
                text(&mut b, words, Pt2::new(mid.x, Nm(mid.y.0 + inside)), Angle2::ZERO, &Align::Centre);
            }
        }
    }
    if let Some(d) = v.datum.filter(|d| a.contains(*d)) {
        let d = at(d);
        text(&mut b, "0", Pt2::new(Nm(d.x.0 + 1_000_000), Nm(d.y.0 - 1_000_000)), Angle2::ZERO, &Align::BottomLeft);
    }
    // The holes at true size, each with its centre cross and its words.
    for h in &v.holes {
        let (hw, hh) = (h.size.0.0 / 2, h.size.1.0 / 2);
        let r = Rect::from_points(
            Pt2::new(Nm(h.at.x.0 - hw), Nm(h.at.y.0 - hh)),
            Pt2::new(Nm(h.at.x.0 + hw), Nm(h.at.y.0 + hh)),
        );
        if !r.intersects(a) {
            continue;
        }
        let c = at(h.at);
        let outline = if h.round { Path::circle(c, Nm(hw)) } else { Path::rect(Rect::from_points(at(Pt2::new(r.min().x, r.max().y)), at(Pt2::new(r.max().x, r.min().y)))) };
        push(&mut b, outline, Some(Stroke::pen(Pen::P026)), None);
        let (rx, ry) = (hw + CROSS_PAST.0, hh + CROSS_PAST.0);
        let cross = PathBuilder::default()
            .move_to(Pt2::new(Nm(c.x.0 - rx), c.y))
            .line_to(Pt2::new(Nm(c.x.0 + rx), c.y))
            .move_to(Pt2::new(c.x, Nm(c.y.0 - ry)))
            .line_to(Pt2::new(c.x, Nm(c.y.0 + ry)))
            .build();
        push(&mut b, cross, Some(thin.clone()), None);
        text(&mut b, &h.words, Pt2::new(Nm(c.x.0 + hw + 1_000_000), Nm(c.y.0 - hh - 1_000_000)), Angle2::ZERO, &Align::BottomLeft);
    }
    // The crosses where four sheets meet.
    for j in v.joins.iter().filter(|j| a.contains(**j)) {
        let c = at(*j);
        let arm = 4_000_000;
        let cross = PathBuilder::default()
            .move_to(Pt2::new(Nm(c.x.0 - arm), c.y))
            .line_to(Pt2::new(Nm(c.x.0 + arm), c.y))
            .move_to(Pt2::new(c.x, Nm(c.y.0 - arm)))
            .line_to(Pt2::new(c.x, Nm(c.y.0 + arm)))
            .build();
        push(&mut b, cross, Some(Stroke::pen(Pen::P026)), None);
    }
    // The band: its words on the left, the map on the right, the 100 mm bar on its last line.
    let top = ph.0 - MARGIN.0 - BAND.0;
    for (i, line) in v.band.iter().enumerate() {
        let y = top + 2_500_000 * (i64::try_from(i).unwrap_or(0) + 1);
        text(&mut b, line, Pt2::new(MARGIN, Nm(y)), Angle2::ZERO, &Align::BottomLeft);
    }
    let bar_y = top + BAND.0 - 500_000;
    let bar = PathBuilder::default()
        .move_to(Pt2::new(MARGIN, Nm(bar_y)))
        .line_to(Pt2::new(Nm(MARGIN.0 + 100_000_000), Nm(bar_y)));
    let bar = (0..=10).fold(bar, |p, k| {
        let x = MARGIN.0 + k * 10_000_000;
        p.move_to(Pt2::new(Nm(x), Nm(bar_y))).line_to(Pt2::new(Nm(x), Nm(bar_y - 1_500_000)))
    });
    push(&mut b, bar.build(), Some(Stroke::pen(Pen::P026)), None);
    text(&mut b, &v.bar, Pt2::new(Nm(MARGIN.0 + 103_000_000), Nm(bar_y)), Angle2::ZERO, &Align::BottomLeft);
    let (rows, cols, (this_r, this_c), printed) = &v.map;
    let cell = 2_500_000i64;
    let mw = cell * i64::from(*cols);
    let (mx, my) = (pw.0 - MARGIN.0 - mw, top + 1_000_000);
    for r in 0..*rows {
        for c in 0..*cols {
            let p = Pt2::new(Nm(mx + cell * i64::from(c)), Nm(my + cell * i64::from(r)));
            let q = Pt2::new(Nm(p.x.0 + cell), Nm(p.y.0 + cell));
            let fill = if (r, c) == (*this_r, *this_c) {
                Some(Brush::BLACK)
            } else if printed.contains(&(r, c)) {
                Some(Brush::Solid(TINT))
            } else {
                None
            };
            push(&mut b, Path::rect(Rect::from_points(p, q)), Some(thin.clone()), fill);
        }
    }
    b.finish().list
}
