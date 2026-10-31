// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Panel-layout pages (WP-S4.01 part 3, D9 and D10; doc 12 §5.1 to §5.3): the surface view of a `panel_layout` page
//! drawn into the panel layers of [`Layer::PANEL`](ew_dl::Layer::PANEL).
//!
//! The caller (`ew-api`, which holds `Q`) fills a [`PanelView`] from the derived layout queries, in the frame of the
//! mounting surface (`u` right, `v` up, nanometres): the surface's outlines, each carrier's outline from
//! `carrier_geometry`, each placement's footprint and outline from `placement_geometry` and each label's text from
//! `layout_label`. Nothing here re-derives geometry; it projects and draws.
//!
//! # Projection
//!
//! A surface point `(u, v)` lands on the page at `x = datum.x + u·num/den`, `y = datum.y − v·num/den` (paper `y`
//! points down, doc 12 §5.1), computed in i128 with one rounding to the nearest nanometre (half away from zero). A
//! page of scale 1:n whose frame maps the page to paper at `f` (`page_to_sheet.scale_ppb / 10⁹`, 1 without a map)
//! has `num/den = 1/(n·f)`, so that the stated scale holds on the printed sheet ([`Projection::on_sheet`]; cycle 41,
//! review part 1, finding 5): on Golden A's frames (`f` = 0.986282578) a 750 mm edge at 1:10 is 76.043 mm on the
//! page and 75.000 mm on paper. The datum stays in page coordinates. A page drawn not to scale draws the surface at
//! 1:1 on the page.
//!
//! # Layers
//!
//! | Layer | Items, in order |
//! |---|---|
//! | [`Layer::Surface`](ew_dl::Layer::Surface) | the enclosure outline ([`Pen::P052`]), the plate outline ([`Pen::P026`]), the usable polygon thin dashed ([`Pen::P013`], 4:2) |
//! | [`Layer::Carriers`](ew_dl::Layer::Carriers) | per carrier by ID: its outline rectangles ([`Pen::P026`]); a rail's axis as a chain line ([`Pen::P013`], dash-dot); a duct's "W×H" label in mm, in the layout-label style, centred on its longest segment and turned to read bottom to top on a vertical one. The outlines and the axis are cut around every placement label's knockout box (see below) |
//! | [`Layer::Placements`](ew_dl::Layer::Placements) | per placement by ID: its `front_2d` outline (FR-LAY-011), else its footprint rectangle ([`Pen::P026`]; an unknown footprint 4:2 dashed) |
//! | [`Layer::Labels`](ew_dl::Layer::Labels) | per placement by ID: its label (see below), after the page items' texts |
//!
//! Carrier outlines and duct labels carry the carrier's entity, placement and label items the placement's, so that
//! hit-testing on the page returns placements and carriers (FR-UI-007). The surface outlines and a rail's axis carry
//! none: the axis runs through the middle of every footprint on the rail, and its zero-area rectangle would win the
//! canvas's smallest-rectangle pick over the footprint a press lands on.
//!
//! # Labels (D10, FR-LAY-062)
//!
//! A label is printed in the style of the slot kind [`LAYOUT_LABEL_KIND`], bound to [`LAYOUT_LABEL_STYLE`]
//! (`xref.device`, 1.83 mm em printed, doc 20 §17.5), at its paper height at every scale: it is never scaled. Its ink
//! box is the shaped text's advance along the text by the face's cap height across it, plus the descent only when the
//! label has a descender glyph ([`has_descender`]), plus [`LABEL_PAD`] on every side (the orchestrator's ruling 5 of
//! cycle 41: ew-text's existing advance and vertical metrics, no new API). The fit test against the footprint's paper
//! rectangle takes the first of:
//!
//! 1. horizontal, centred on the footprint, when the box fits as it is;
//! 2. turned by 90° (reading bottom to top), centred, when the box fits turned;
//! 3. outside, above the footprint: the label's bottom [`LEADER_GAP`] above the footprint's top edge, centred on it,
//!    with a leader line from the label down to that edge; below the footprint instead when the label above would
//!    leave the surface's paper bounds.
//!
//! Balloons are 1.0 (doc 12 §5.2), so FR-LAY-062's last step is not built.
//!
//! # The knockout (cycle 41, window check W41a; the orchestrator's ruling 2)
//!
//! Every placement label, inside its footprint or outside with a leader, clears the carriers under it: its
//! **knockout box** is its ink box as placed ([`place_label`]: the advance along the text by the cap height across it,
//! plus the descent with a descender glyph, plus [`LABEL_PAD`] on every side, turned with the label) on the page. The
//! carriers layer cuts each outline rectangle and each rail axis around every knockout box: the parts of a stroke
//! inside a box (its edges included) are left out, the rest is drawn as the subpaths of the same item, so every
//! carrier keeps one item per rectangle and per axis (the axis's dash pattern starts again at each subpath, PDF
//! semantics). Only the carriers are cut: the surface outlines (drawn before), the footprints and the labels (drawn
//! after) stay whole. A duct's "W×H" text is not cut (glyphs cannot be clipped without a clip item); a placement label
//! on a duct label is not expected, since footprints do not sit on ducts.
//!
//! Cutting instead of a paper-coloured fill: `ew-dl` has no brush that resolves to the page background (a
//! [`Brush`](ew_dl::Brush) is a literal colour), and a fill at the end of the carriers layer would also erase the
//! surface outlines under the box.

use std::sync::Arc;

use ew_dl::{DashPattern, EntityRef, Item, Path, Pen, Stroke};
use ew_ids::Id;
use ew_schema::{Align, Carrier, Enclosure, MountingSurface, PageItem, Placement, Scale};
use ew_text::{Shaped, TextStyle};
use ew_units::{Angle2, Nm, Pt2, Rect};

use crate::page::{Builder, PageView, TextSpec};

/// The slot kind of a placement's label (doc 12 §5.2).
pub const LAYOUT_LABEL_KIND: &str = "layout.label";

/// The text style the label slot kind is bound to (doc 12 §5.2, doc 20 §17.5): `xref.device`.
pub const LAYOUT_LABEL_STYLE: &str = "xref.device";

/// The margin of a label's ink box on every side in the fit test: 0.1 mm (D10).
pub const LABEL_PAD: Nm = Nm(100_000);

/// The gap between a label placed outside its footprint and the footprint's edge, bridged by the leader: 1.5 mm.
pub const LEADER_GAP: Nm = Nm(1_500_000);

/// The page projection of a surface view (see the module documentation).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Projection {
    /// Where the surface's origin lands on the page.
    pub datum: Pt2,
    /// The numerator of the scale (positive).
    pub num: i128,
    /// The denominator of the scale (positive).
    pub den: i128,
}

/// `a / b` rounded to the nearest integer, halves away from zero (`b > 0`).
fn div_round(a: i128, b: i128) -> i128 {
    let q = a / b;
    let r = a % b;
    if 2 * r.abs() >= b { q + a.signum() } else { q }
}

/// The greatest common divisor of two positive numbers.
fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

impl Projection {
    /// The projection of a view at `datum` on a page of `scale` whose frame maps the page to paper at 1: 1:n, or
    /// 1:1 for a page drawn not to scale (or a scale of 0, which the format does not forbid). The same as
    /// [`on_sheet`](Projection::on_sheet) with [`PPB_ONE`](crate::forms::PPB_ONE).
    pub fn of(datum: Pt2, scale: &Scale) -> Projection {
        Projection::on_sheet(datum, scale, crate::forms::PPB_ONE)
    }

    /// The projection of a view at `datum` on a page of `scale` whose frame maps the page to paper at
    /// `sheet_ppb / 10⁹` (the form's `page_to_sheet.scale_ppb`, [`SheetMap`](crate::forms::SheetMap)): a page of
    /// scale 1:n draws a surface length `l` at `l / n / f` on the page, `f = sheet_ppb / 10⁹`, so that it prints at
    /// exactly 1:n on paper (`num/den = 10⁹ / (n · sheet_ppb)`, reduced). A page drawn not to scale draws the surface
    /// at 1:1 on the page whatever the frame's map: it states no scale. A map of 0 counts as 1.
    pub fn on_sheet(datum: Pt2, scale: &Scale, sheet_ppb: u32) -> Projection {
        let one = i128::from(crate::forms::PPB_ONE);
        let (num, den) = match scale {
            Scale::OneTo(n) if *n > 0 => {
                let ppb = if sheet_ppb == 0 { one } else { i128::from(sheet_ppb) };
                (one, i128::from(*n) * ppb)
            }
            _ => (1, 1),
        };
        let g = gcd(num, den);
        Projection {
            datum,
            num: num / g,
            den: den / g,
        }
    }

    /// A length of the surface on the page, rounded once.
    pub fn length(&self, l: i128) -> i128 {
        div_round(l * self.num, self.den.max(1))
    }

    /// The page point of the surface point `(u, v)`; `None` outside the `Nm` range.
    pub fn point(&self, p: Pt2) -> Option<Pt2> {
        let x = i128::from(self.datum.x.0) + self.length(i128::from(p.x.0));
        let y = i128::from(self.datum.y.0) - self.length(i128::from(p.y.0));
        Some(Pt2::new(Nm(i64::try_from(x).ok()?), Nm(i64::try_from(y).ok()?)))
    }

    /// The page rectangle of a surface rectangle; `None` outside the `Nm` range.
    pub fn rect(&self, r: Rect) -> Option<Rect> {
        Some(Rect::from_points(self.point(r.min())?, self.point(r.max())?))
    }

    /// The page points of surface points; `None` when one falls outside the `Nm` range.
    pub fn points(&self, pts: &[Pt2]) -> Option<Vec<Pt2>> {
        pts.iter().map(|p| self.point(*p)).collect()
    }
}

/// The view of one mounting surface on a panel-layout page, in the surface's frame (see the module documentation).
/// Each layer reads its own part; the caller fills the parts the layer it builds reads.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PanelView {
    /// The surface view's page item.
    pub item: Id<PageItem>,
    /// The page projection.
    pub projection: Projection,
    /// The surface's outlines ([`Layer::Surface`](ew_dl::Layer::Surface)); `None` draws none.
    pub surface: Option<SurfaceOutline>,
    /// The bounds of the surface in its frame (its usable polygon's), which a label placed outside its footprint
    /// stays within when it can ([`Layer::Labels`](ew_dl::Layer::Labels), and the knockout of
    /// [`Layer::Carriers`](ew_dl::Layer::Carriers)).
    pub bounds: Option<Rect>,
    /// The carriers on the surface ([`Layer::Carriers`](ew_dl::Layer::Carriers)).
    pub carriers: Vec<CarrierDraw>,
    /// The placements on the surface ([`Layer::Placements`](ew_dl::Layer::Placements)).
    pub placements: Vec<PlacementDraw>,
    /// The placements' labels ([`Layer::Labels`](ew_dl::Layer::Labels), and the knockout of
    /// [`Layer::Carriers`](ew_dl::Layer::Carriers)).
    pub labels: Vec<PanelLabel>,
}

/// The outlines of the shown surface.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SurfaceOutline {
    /// The surface.
    pub surface: Id<MountingSurface>,
    /// The enclosure and its outer outline in the surface's frame, when its size is known.
    pub enclosure: Option<(Id<Enclosure>, Rect)>,
    /// The plate's outline in the surface's frame, when known.
    pub plate: Option<Rect>,
    /// The usable polygon.
    pub usable: Vec<Pt2>,
}

/// One carrier as `carrier_geometry` gives it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CarrierDraw {
    /// The carrier.
    pub carrier: Id<Carrier>,
    /// A rail's axis (its two path points), drawn as a chain line; `None` for a duct.
    pub axis: Option<(Pt2, Pt2)>,
    /// The outline rectangles (one per path segment).
    pub outline: Vec<Rect>,
    /// A duct's width and height, printed "W×H" in millimetres; `None` for a rail.
    pub duct: Option<(Nm, Nm)>,
}

/// One placement as `placement_geometry` gives it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlacementDraw {
    /// The placement.
    pub placement: Id<Placement>,
    /// The footprint.
    pub footprint: Rect,
    /// Whether the footprint is unknown (drawn dashed, D5).
    pub unknown: bool,
    /// The `front_2d` outline placed in the surface's frame; empty draws the footprint (FR-LAY-011).
    pub outline: Vec<Vec<Pt2>>,
}

/// One placement's label.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PanelLabel {
    /// The placement.
    pub placement: Id<Placement>,
    /// The text (`layout_label`).
    pub text: String,
    /// The placement's footprint in the surface's frame.
    pub footprint: Rect,
    /// The text shaped in the label style (the memoised `shaped_text`); `None`: the scene shapes it.
    pub shaped: Option<Arc<Shaped>>,
}

/// How a label was placed (D10's fit steps), in the order they are tried.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LabelFit {
    /// Horizontal inside the footprint.
    Horizontal,
    /// Turned by 90° inside the footprint.
    Turned,
    /// Outside above the footprint, with a leader.
    Above,
    /// Outside below the footprint, with a leader.
    Below,
}

/// Whether a label has a glyph that reaches below the baseline: a lower-case `g`, `j`, `p`, `q`, `y` or one of
/// `, ; ( ) [ ] { } | _ / \ @ $`. Tags (capitals, digits, `-`, `=`, `+`, `:`, `.`) have none.
pub fn has_descender(text: &str) -> bool {
    text.chars().any(|c| "gjpqy,;()[]{}|_/\\@$".contains(c))
}

/// The fit step of a label whose ink box (padded) is `w × h` against a footprint of `fw × fh` on paper.
pub fn fit(w: Nm, h: Nm, fw: Nm, fh: Nm) -> Option<LabelFit> {
    if w <= fw && h <= fh {
        Some(LabelFit::Horizontal)
    } else if h <= fw && w <= fh {
        Some(LabelFit::Turned)
    } else {
        None
    }
}

/// A label as D10's fit places it on the page: what [`Layer::Labels`](ew_dl::Layer::Labels) draws and what
/// [`Layer::Carriers`](ew_dl::Layer::Carriers) cuts the carriers around (see the module documentation).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlacedLabel {
    /// The fit step taken.
    pub fit: LabelFit,
    /// The text's anchor on the page.
    pub at: Pt2,
    /// The text's angle (0 or 90°, reading bottom to top).
    pub angle: Angle2,
    /// The text's alignment at its anchor.
    pub align: Align,
    /// The leader of a label placed outside, from the label's end to the footprint's edge.
    pub leader: Option<(Pt2, Pt2)>,
    /// The knockout box: the ink box as placed, padded by [`LABEL_PAD`] on every side, on the page.
    pub knockout: Rect,
}

/// A label placed on the page with its text, entity, style and shaped text (the builder's working form).
type PlacedOnPage<'p> = (&'p str, PlacedLabel, EntityRef, TextStyle, Arc<Shaped>);

/// The padded ink box of a one-line text of advance `adv` anchored at `at` with the vertical alignment `valign`
/// (−1 top, 0 middle, 1 bottom) and centred horizontally, turned by `angle` about `at`: the layout rules of the
/// scene's text placement (`forms::text::place`), the cap height above the baseline and `desc` below it.
fn ink_box(at: Pt2, valign: i8, angle: Angle2, adv: Nm, m: &ew_text::VerticalMetrics, desc: Nm) -> Rect {
    let baseline = match valign {
        -1 => at.y.0 + m.ascent.0,
        1 => at.y.0 - m.descent.0,
        _ => at.y.0 + m.cap_height.0.div_euclid(2),
    };
    let x0 = at.x.0 - adv.0.div_euclid(2);
    let a = Pt2::new(Nm(x0 - LABEL_PAD.0), Nm(baseline - m.cap_height.0 - LABEL_PAD.0));
    let b = Pt2::new(Nm(x0 + adv.0 + LABEL_PAD.0), Nm(baseline + desc.0 + LABEL_PAD.0));
    if angle == Angle2::ZERO {
        Rect::from_points(a, b)
    } else {
        Rect::from_points(ew_dl::num::rotate(a, angle, at), ew_dl::num::rotate(b, angle, at))
    }
}

/// Places the label `text`, shaped as `shaped` in `style`, for the footprint `fp` (on the page) within the surface's
/// page `bounds` (D10's fit steps, see the module documentation).
pub fn place_label(
    fonts: &ew_text::FontSet,
    style: &ew_text::TextStyle,
    text: &str,
    shaped: &Shaped,
    fp: Rect,
    bounds: Option<Rect>,
) -> PlacedLabel {
    let m = fonts.vertical_metrics(style);
    let desc = if has_descender(text) { m.descent } else { Nm::ZERO };
    let adv = shaped.advance();
    let w = Nm(adv.0 + 2 * LABEL_PAD.0);
    let h = Nm(m.cap_height.0 + desc.0 + 2 * LABEL_PAD.0);
    match fit(w, h, fp.width(), fp.height()) {
        Some(f @ (LabelFit::Horizontal | LabelFit::Turned)) => {
            let at = centre(fp);
            let angle = if f == LabelFit::Turned { Angle2(90_000) } else { Angle2::ZERO };
            PlacedLabel {
                fit: f,
                at,
                angle,
                align: Align::Centre,
                leader: None,
                knockout: ink_box(at, 0, angle, adv, &m, desc),
            }
        }
        _ => {
            let cx = centre(fp).x;
            let top = fp.min().y;
            let above_top = Nm(top.0 - LEADER_GAP.0 - h.0);
            let below = bounds.is_some_and(|b| above_top < b.min().y);
            let (fit, edge, end, align, valign) = if below {
                let bottom = fp.max().y;
                (LabelFit::Below, bottom, Nm(bottom.0 + LEADER_GAP.0), Align::TopCentre, -1)
            } else {
                (LabelFit::Above, top, Nm(top.0 - LEADER_GAP.0), Align::BottomCentre, 1)
            };
            let at = Pt2::new(cx, end);
            PlacedLabel {
                fit,
                at,
                angle: Angle2::ZERO,
                align,
                leader: Some((at, Pt2::new(cx, edge))),
                knockout: ink_box(at, valign, Angle2::ZERO, adv, &m, desc),
            }
        }
    }
}

/// The pieces of the axis-parallel segment `a`–`b` outside every box of `boxes` (a box's edges count as inside), in
/// the order from `a` to `b`; pieces of no length are left out. A segment at another angle is returned whole (carriers
/// are axis-parallel, D5).
fn cut_segment(a: Pt2, b: Pt2, boxes: &[Rect]) -> Vec<(Pt2, Pt2)> {
    let horizontal = a.y == b.y;
    if !horizontal && a.x != b.x {
        return vec![(a, b)];
    }
    // The coordinate along the segment and the one across it.
    let (along, across): (fn(Pt2) -> i64, fn(Pt2) -> i64) = if horizontal {
        (|p| p.x.0, |p| p.y.0)
    } else {
        (|p| p.y.0, |p| p.x.0)
    };
    let at = |t: i64| if horizontal { Pt2::new(Nm(t), a.y) } else { Pt2::new(a.x, Nm(t)) };
    let (s, e) = (along(a), along(b));
    let (lo, hi) = (s.min(e), s.max(e));
    let c = across(a);
    let mut gaps: Vec<(i64, i64)> = boxes
        .iter()
        .filter(|r| across(r.min()) <= c && c <= across(r.max()))
        .map(|r| (along(r.min()).max(lo), along(r.max()).min(hi)))
        .filter(|(g0, g1)| g0 <= g1)
        .collect();
    if gaps.is_empty() {
        return vec![(a, b)];
    }
    gaps.sort_unstable();
    let mut pieces = Vec::new();
    let mut from = lo;
    for (g0, g1) in gaps {
        if g0 > from {
            pieces.push((from, g0));
        }
        from = from.max(g1);
    }
    if hi > from {
        pieces.push((from, hi));
    }
    if s > e {
        pieces.reverse();
        pieces.iter().map(|&(p0, p1)| (at(p1), at(p0))).collect()
    } else {
        pieces.iter().map(|&(p0, p1)| (at(p0), at(p1))).collect()
    }
}

/// The closed polyline `pts` (the last point joins the first) cut around `boxes` as one path of subpaths: `None` when
/// no box touches it (the caller draws it whole), an empty path when every edge lies inside a box.
fn cut_closed(pts: &[Pt2], boxes: &[Rect]) -> Option<Path> {
    let n = pts.len();
    let mut cut = false;
    let mut pieces: Vec<(Pt2, Pt2)> = Vec::new();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let p = cut_segment(a, b, boxes);
        cut |= p != [(a, b)];
        pieces.extend(p);
    }
    if !cut {
        return None;
    }
    Some(chain(&pieces, true))
}

/// One path of the pieces, each run of pieces that meet end to start one subpath; with `closed` the last run joins
/// the first when it ends where the first begins.
fn chain(pieces: &[(Pt2, Pt2)], closed: bool) -> Path {
    let mut runs: Vec<Vec<Pt2>> = Vec::new();
    for &(a, b) in pieces {
        match runs.last_mut() {
            Some(run) if run.last() == Some(&a) => run.push(b),
            _ => runs.push(vec![a, b]),
        }
    }
    if closed
        && runs.len() > 1
        && let (Some(first), Some(last)) = (runs.first(), runs.last())
        && last.last() == first.first()
    {
        let first = runs.remove(0);
        if let Some(last) = runs.last_mut() {
            last.extend(first.into_iter().skip(1));
        }
    }
    let mut b = Path::builder();
    for run in &runs {
        let mut it = run.iter();
        if let Some(&p) = it.next() {
            b = b.move_to(p);
        }
        for &p in it {
            b = b.line_to(p);
        }
    }
    b.build()
}

/// The millimetre text of a length: whole millimetres without decimals, else up to three decimals without trailing
/// zeros ("40", "37.5").
fn mm_text(n: Nm) -> String {
    let (whole, frac) = (n.0 / 1_000_000, (n.0 % 1_000_000).abs());
    if frac == 0 {
        return whole.to_string();
    }
    let sign = if n.0 < 0 && whole == 0 { "-" } else { "" };
    let micro = format!("{:06}", frac);
    let dec = micro[..3].trim_end_matches('0');
    if dec.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{dec}")
    }
}

/// The midpoint of the longest of `rects` (the first of equal ones) and whether it is taller than wide.
fn longest(rects: &[Rect]) -> Option<(Pt2, bool)> {
    let mut best: Option<(i64, Rect)> = None;
    for r in rects {
        let len = r.width().0.max(r.height().0);
        if best.is_none_or(|(l, _)| len > l) {
            best = Some((len, *r));
        }
    }
    best.map(|(_, r)| {
        let mid = |a: Nm, b: Nm| Nm(a.0 + (b.0 - a.0).div_euclid(2));
        (
            Pt2::new(mid(r.min().x, r.max().x), mid(r.min().y, r.max().y)),
            r.height() > r.width(),
        )
    })
}

/// The centre of a rectangle (halving toward −∞).
fn centre(r: Rect) -> Pt2 {
    let mid = |a: Nm, b: Nm| Nm(a.0 + (b.0 - a.0).div_euclid(2));
    Pt2::new(mid(r.min().x, r.max().x), mid(r.min().y, r.max().y))
}

impl Builder<'_> {
    /// [`Layer::Surface`](ew_dl::Layer::Surface).
    pub(crate) fn panel_surface(&mut self, view: &PageView) {
        let Some(panel) = &view.panel else { return };
        let Some(s) = &panel.surface else { return };
        let p = &panel.projection;
        if let Some(r) = s.enclosure.and_then(|(_, r)| p.rect(r)) {
            self.push(Item::path(Path::rect(r), Some(Stroke::pen(Pen::P052)), None));
        }
        if let Some(r) = s.plate.and_then(|r| p.rect(r)) {
            self.push(Item::path(Path::rect(r), Some(Stroke::pen(Pen::P026)), None));
        }
        if s.usable.len() >= 3
            && let Some(pts) = p.points(&s.usable)
        {
            self.push(Item::path(
                Path::polygon(&pts),
                Some(Stroke::pen(Pen::P013).dashed(DashPattern::Dash42)),
                None,
            ));
        }
    }

    /// The labels of `panel` as placed on the page, by placement ID, with their entity, style and shaped text (empty
    /// texts and footprints outside the `Nm` range left out). With `report`, an unknown label style is reported (the
    /// labels layer); the carriers layer reads the same placement quietly.
    fn placed_labels<'p>(&mut self, panel: &'p PanelView, report: bool) -> Vec<PlacedOnPage<'p>> {
        let p = panel.projection;
        let bounds = panel.bounds.and_then(|b| p.rect(b));
        let mut labels: Vec<&PanelLabel> = panel.labels.iter().collect();
        labels.sort_by_key(|l| l.placement);
        let mut out = Vec::with_capacity(labels.len());
        for l in labels {
            if l.text.is_empty() {
                continue;
            }
            let Some(fp) = p.rect(l.footprint) else { continue };
            let entity = EntityRef(l.placement.as_uuid());
            let style = if report {
                self.style(LAYOUT_LABEL_STYLE, entity)
            } else {
                self.style_quiet(LAYOUT_LABEL_STYLE)
            };
            let shaped = match &l.shaped {
                Some(s) => Arc::clone(s),
                None => Arc::new(self.fonts.shape(&l.text, &style, None)),
            };
            let placed = place_label(self.fonts, &style, &l.text, &shaped, fp, bounds);
            out.push((l.text.as_str(), placed, entity, style, shaped));
        }
        out
    }

    /// [`Layer::Carriers`](ew_dl::Layer::Carriers), the outlines and axes cut around the labels' knockout boxes.
    pub(crate) fn panel_carriers(&mut self, view: &PageView) {
        let Some(panel) = &view.panel else { return };
        let p = panel.projection;
        let knockouts: Vec<Rect> = self
            .placed_labels(panel, false)
            .into_iter()
            .map(|(_, l, ..)| l.knockout)
            .collect();
        let mut carriers: Vec<&CarrierDraw> = panel.carriers.iter().collect();
        carriers.sort_by_key(|c| c.carrier);
        for c in carriers {
            let entity = EntityRef(c.carrier.as_uuid());
            let outline: Vec<Rect> = c.outline.iter().filter_map(|r| p.rect(*r)).collect();
            for r in &outline {
                let (a, b) = (r.min(), r.max());
                let corners = [a, Pt2::new(a.x, b.y), b, Pt2::new(b.x, a.y)];
                let path = match cut_closed(&corners, &knockouts) {
                    None => Path::rect(*r),
                    Some(path) if path.is_empty() => continue,
                    Some(path) => path,
                };
                self.push(Item::path(path, Some(Stroke::pen(Pen::P026)), None).with_entity(entity));
            }
            if let Some((a, b)) = c.axis
                && let (Some(a), Some(b)) = (p.point(a), p.point(b))
            {
                let pieces = cut_segment(a, b, &knockouts);
                let path = if pieces == [(a, b)] {
                    Path::line(a, b)
                } else {
                    chain(&pieces, false)
                };
                if !path.is_empty() {
                    self.push(Item::path(
                        path,
                        Some(Stroke::pen(Pen::P013).dashed(DashPattern::DashDot)),
                        None,
                    ));
                }
            }
            if let (Some((w, h)), Some((at, vertical))) = (c.duct, longest(&outline)) {
                let text = format!("{}\u{d7}{}", mm_text(w), mm_text(h));
                self.text(&TextSpec {
                    text: &text,
                    at,
                    angle: if vertical { Angle2(90_000) } else { Angle2::ZERO },
                    align: &Align::Centre,
                    style_key: LAYOUT_LABEL_STYLE,
                    entity,
                    link: None,
                    items: &[],
                });
            }
        }
    }

    /// [`Layer::Placements`](ew_dl::Layer::Placements).
    pub(crate) fn panel_placements(&mut self, view: &PageView) {
        let Some(panel) = &view.panel else { return };
        let p = panel.projection;
        let mut placements: Vec<&PlacementDraw> = panel.placements.iter().collect();
        placements.sort_by_key(|d| d.placement);
        for d in placements {
            let entity = EntityRef(d.placement.as_uuid());
            let outline: Option<Vec<Vec<Pt2>>> = d.outline.iter().map(|l| p.points(l)).collect();
            match outline {
                Some(lines) if !lines.is_empty() => {
                    for line in lines.iter().filter(|l| l.len() >= 2) {
                        self.push(
                            Item::path(Path::polyline(line), Some(Stroke::pen(Pen::P026)), None)
                                .with_entity(entity),
                        );
                    }
                }
                _ => {
                    let Some(r) = p.rect(d.footprint) else { continue };
                    let stroke = if d.unknown {
                        Stroke::pen(Pen::P026).dashed(DashPattern::Dash42)
                    } else {
                        Stroke::pen(Pen::P026)
                    };
                    self.push(Item::path(Path::rect(r), Some(stroke), None).with_entity(entity));
                }
            }
        }
    }

    /// The panel labels of [`Layer::Labels`](ew_dl::Layer::Labels) (see the module documentation).
    pub(crate) fn panel_labels(&mut self, view: &PageView) {
        let Some(panel) = &view.panel else { return };
        for (text, l, entity, style, shaped) in self.placed_labels(panel, true) {
            if let Some((a, b)) = l.leader {
                self.push(Item::path(Path::line(a, b), Some(Stroke::pen(Pen::P013)), None).with_entity(entity));
            }
            self.text_shaped(
                &TextSpec {
                    text,
                    at: l.at,
                    angle: l.angle,
                    align: &l.align,
                    style_key: LAYOUT_LABEL_STYLE,
                    entity,
                    link: None,
                    items: &[],
                },
                &style,
                &shaped,
            );
        }
    }
}

/// How the label of `text` shaped as `shaped` fits a footprint of `fw × fh` on paper in `fonts` and `style` (the
/// scene's fit test, for callers and tests): `None` when it goes outside with a leader.
pub fn label_fit(
    fonts: &ew_text::FontSet,
    style: &ew_text::TextStyle,
    text: &str,
    shaped: &Shaped,
    fw: Nm,
    fh: Nm,
) -> Option<LabelFit> {
    let fp = Rect::from_points(Pt2::ORIGIN, Pt2::new(fw, fh));
    match place_label(fonts, style, text, shaped, fp, None).fit {
        f @ (LabelFit::Horizontal | LabelFit::Turned) => Some(f),
        LabelFit::Above | LabelFit::Below => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projection_rounds_once_half_away_from_zero() {
        let p = Projection::of(Pt2::new(Nm(25_000_000), Nm(167_000_000)), &Scale::OneTo(10));
        // The enclosure's corner (-50, -50) mm of the plate frame lands at (20, 172) mm.
        assert_eq!(
            p.point(Pt2::new(Nm(-50_000_000), Nm(-50_000_000))),
            Some(Pt2::new(Nm(20_000_000), Nm(172_000_000)))
        );
        assert_eq!(p.length(15), 2);
        assert_eq!(p.length(-15), -2);
        assert_eq!(p.length(14), 1);
        assert_eq!(p.length(-14), -1);
        assert_eq!(Projection::of(Pt2::ORIGIN, &Scale::NotToScale).den, 1);
        assert_eq!(Projection::of(Pt2::ORIGIN, &Scale::OneTo(0)).den, 1);
        assert_eq!((p.num, p.den), (1, 10), "reduced");
    }

    /// Ruling 1 of cycle 41's fix round: the stated scale holds on paper.
    #[test]
    fn the_projection_divides_by_the_frames_sheet_factor() {
        const PPB: u32 = 986_282_578;
        let datum = Pt2::new(Nm(25_000_000), Nm(167_000_000));
        let p = Projection::on_sheet(datum, &Scale::OneTo(10), PPB);
        // A 750 mm edge at 1:10 on a 0.986282578 frame: 750 / 10 / 0.986282578 = 76.04311… mm on the page …
        let page = p.length(750_000_000);
        assert_eq!(page, 76_043_116);
        // … and 75.000 mm printed (the sheet map multiplies by the factor), to the nanometre rounding.
        let printed = (page * i128::from(PPB) + 500_000_000) / 1_000_000_000;
        assert_eq!(printed, 75_000_000);
        // The datum stays in page coordinates; the enclosure's corner (-50, -50) mm lands 5 mm printed from it.
        assert_eq!(
            p.point(Pt2::new(Nm(-50_000_000), Nm(-50_000_000))),
            Some(Pt2::new(Nm(25_000_000 - 5_069_541), Nm(167_000_000 + 5_069_541)))
        );
        assert_eq!(p.point(Pt2::ORIGIN), Some(datum));
        // Not to scale claims nothing: 1:1 on the page whatever the map.
        let n = Projection::on_sheet(datum, &Scale::NotToScale, PPB);
        assert_eq!((n.num, n.den), (1, 1));
        // A frame without a map (and a map of 0) is `of`.
        assert_eq!(
            Projection::on_sheet(datum, &Scale::OneTo(10), crate::forms::PPB_ONE),
            Projection::of(datum, &Scale::OneTo(10))
        );
        assert_eq!(
            Projection::on_sheet(datum, &Scale::OneTo(10), 0),
            Projection::of(datum, &Scale::OneTo(10))
        );
        // Extremes stay exact in i128.
        let x = Projection::on_sheet(Pt2::ORIGIN, &Scale::OneTo(u32::MAX), u32::MAX);
        assert_eq!((x.num, x.den), (40_000_000, 737_869_762_604_784_681));
        assert_eq!(x.length(i128::from(i64::MAX)), 500_000_000);
    }

    fn mm(x: f64, y: f64) -> Pt2 {
        Pt2::new(Nm((x * 1e6).round() as i64), Nm((y * 1e6).round() as i64))
    }

    #[test]
    fn a_segment_is_cut_around_the_boxes_it_crosses() {
        let boxes = [
            Rect::from_points(mm(2.0, -1.0), mm(3.0, 1.0)),
            Rect::from_points(mm(2.5, -1.0), mm(4.0, 1.0)),
            Rect::from_points(mm(6.0, 2.0), mm(7.0, 3.0)),
        ];
        // Horizontal, left to right: the two overlapping boxes cut one gap; the box off the line cuts nothing.
        assert_eq!(
            cut_segment(mm(0.0, 0.0), mm(10.0, 0.0), &boxes),
            vec![(mm(0.0, 0.0), mm(2.0, 0.0)), (mm(4.0, 0.0), mm(10.0, 0.0))]
        );
        // Right to left keeps the direction.
        assert_eq!(
            cut_segment(mm(10.0, 0.0), mm(0.0, 0.0), &boxes),
            vec![(mm(10.0, 0.0), mm(4.0, 0.0)), (mm(2.0, 0.0), mm(0.0, 0.0))]
        );
        // On a box's edge counts as inside; a box over an end leaves no piece of no length.
        assert_eq!(
            cut_segment(mm(0.0, 1.0), mm(3.0, 1.0), &boxes),
            vec![(mm(0.0, 1.0), mm(2.0, 1.0))]
        );
        // Vertical.
        assert_eq!(
            cut_segment(mm(6.5, 0.0), mm(6.5, 5.0), &boxes),
            vec![(mm(6.5, 0.0), mm(6.5, 2.0)), (mm(6.5, 3.0), mm(6.5, 5.0))]
        );
        // Untouched and diagonal segments stay whole.
        assert_eq!(
            cut_segment(mm(0.0, 5.0), mm(9.0, 5.0), &boxes),
            vec![(mm(0.0, 5.0), mm(9.0, 5.0))]
        );
        assert_eq!(
            cut_segment(mm(0.0, 0.0), mm(9.0, 9.0), &boxes),
            vec![(mm(0.0, 0.0), mm(9.0, 9.0))]
        );
    }

    #[test]
    fn a_rectangle_is_cut_into_subpaths_that_keep_their_corners() {
        use ew_dl::PathEl::{LineTo, MoveTo};
        let corners = [mm(0.0, 0.0), mm(0.0, 4.0), mm(10.0, 4.0), mm(10.0, 0.0)];
        assert_eq!(cut_closed(&corners, &[]), None, "untouched: drawn whole");
        // A turned label's box across both long edges splits the outline in two, each around its corners.
        let across = [Rect::from_points(mm(4.0, -1.0), mm(5.0, 5.0))];
        let path = cut_closed(&corners, &across).expect("cut");
        assert_eq!(
            path.els,
            vec![
                MoveTo(mm(5.0, 4.0)),
                LineTo(mm(10.0, 4.0)),
                LineTo(mm(10.0, 0.0)),
                LineTo(mm(5.0, 0.0)),
                MoveTo(mm(4.0, 0.0)),
                LineTo(mm(0.0, 0.0)),
                LineTo(mm(0.0, 4.0)),
                LineTo(mm(4.0, 4.0)),
            ]
        );
        // A box over one edge only leaves one subpath that runs on around the start corner.
        let top = [Rect::from_points(mm(4.0, 3.5), mm(5.0, 4.5))];
        let path = cut_closed(&corners, &top).expect("cut");
        assert_eq!(
            path.els,
            vec![
                MoveTo(mm(5.0, 4.0)),
                LineTo(mm(10.0, 4.0)),
                LineTo(mm(10.0, 0.0)),
                LineTo(mm(0.0, 0.0)),
                LineTo(mm(0.0, 4.0)),
                LineTo(mm(4.0, 4.0)),
            ]
        );
        // A box over the whole outline leaves nothing.
        let all = [Rect::from_points(mm(-1.0, -1.0), mm(11.0, 5.0))];
        assert!(cut_closed(&corners, &all).expect("cut").is_empty());
    }

    #[test]
    fn the_fit_steps() {
        let mm = |v: f64| Nm((v * 1e6) as i64);
        assert_eq!(
            fit(mm(5.0), mm(1.5), mm(10.0), mm(7.0)),
            Some(LabelFit::Horizontal)
        );
        assert_eq!(fit(mm(6.0), mm(1.5), mm(1.8), mm(8.5)), Some(LabelFit::Turned));
        assert_eq!(fit(mm(6.0), mm(1.5), mm(0.6), mm(8.5)), None);
    }

    #[test]
    fn descenders_and_millimetres() {
        assert!(!has_descender("-101F1"));
        assert!(!has_descender("=P01+J-E-2416-X1031:1"));
        assert!(has_descender("Pump"));
        assert_eq!(mm_text(Nm(40_000_000)), "40");
        assert_eq!(mm_text(Nm(37_500_000)), "37.5");
        assert_eq!(mm_text(Nm(6_250_000)), "6.25");
    }
}
