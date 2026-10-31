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
//! page of scale 1:n has `num/den = 1/n`; a page drawn not to scale draws the surface at 1:1.
//!
//! # Layers
//!
//! | Layer | Items, in order |
//! |---|---|
//! | [`Layer::Surface`](ew_dl::Layer::Surface) | the enclosure outline ([`Pen::P052`]), the plate outline ([`Pen::P026`]), the usable polygon thin dashed ([`Pen::P013`], 4:2) |
//! | [`Layer::Carriers`](ew_dl::Layer::Carriers) | per carrier by ID: its outline rectangles ([`Pen::P026`]); a rail's axis as a chain line ([`Pen::P013`], dash-dot); a duct's "W×H" label in mm, in the layout-label style, centred on its longest segment and turned to read bottom to top on a vertical one |
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

use std::sync::Arc;

use ew_dl::{DashPattern, EntityRef, Item, Path, Pen, Stroke};
use ew_ids::Id;
use ew_schema::{Align, Carrier, Enclosure, MountingSurface, PageItem, Placement, Scale};
use ew_text::Shaped;
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
    /// The numerator of the scale.
    pub num: i64,
    /// The denominator of the scale (positive).
    pub den: i64,
}

/// `a / b` rounded to the nearest integer, halves away from zero (`b > 0`).
fn div_round(a: i128, b: i128) -> i128 {
    let q = a / b;
    let r = a % b;
    if 2 * r.abs() >= b { q + a.signum() } else { q }
}

impl Projection {
    /// The projection of a view at `datum` on a page of `scale`: 1:n, or 1:1 for a page drawn not to scale (or a
    /// scale of 0, which the format does not forbid).
    pub fn of(datum: Pt2, scale: &Scale) -> Projection {
        let den = match scale {
            Scale::OneTo(n) if *n > 0 => i64::from(*n),
            _ => 1,
        };
        Projection { datum, num: 1, den }
    }

    /// A length of the surface on the page, rounded once.
    pub fn length(&self, l: i128) -> i128 {
        div_round(l * i128::from(self.num), i128::from(self.den.max(1)))
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
    /// stays within when it can ([`Layer::Labels`](ew_dl::Layer::Labels)).
    pub bounds: Option<Rect>,
    /// The carriers on the surface ([`Layer::Carriers`](ew_dl::Layer::Carriers)).
    pub carriers: Vec<CarrierDraw>,
    /// The placements on the surface ([`Layer::Placements`](ew_dl::Layer::Placements)).
    pub placements: Vec<PlacementDraw>,
    /// The placements' labels ([`Layer::Labels`](ew_dl::Layer::Labels)).
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

    /// [`Layer::Carriers`](ew_dl::Layer::Carriers).
    pub(crate) fn panel_carriers(&mut self, view: &PageView) {
        let Some(panel) = &view.panel else { return };
        let p = panel.projection;
        let mut carriers: Vec<&CarrierDraw> = panel.carriers.iter().collect();
        carriers.sort_by_key(|c| c.carrier);
        for c in carriers {
            let entity = EntityRef(c.carrier.as_uuid());
            let outline: Vec<Rect> = c.outline.iter().filter_map(|r| p.rect(*r)).collect();
            for r in &outline {
                self.push(Item::path(Path::rect(*r), Some(Stroke::pen(Pen::P026)), None).with_entity(entity));
            }
            if let Some((a, b)) = c.axis
                && let (Some(a), Some(b)) = (p.point(a), p.point(b))
            {
                self.push(Item::path(
                    Path::line(a, b),
                    Some(Stroke::pen(Pen::P013).dashed(DashPattern::DashDot)),
                    None,
                ));
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
        let p = panel.projection;
        let bounds = panel.bounds.and_then(|b| p.rect(b));
        let mut labels: Vec<&PanelLabel> = panel.labels.iter().collect();
        labels.sort_by_key(|l| l.placement);
        for l in labels {
            if l.text.is_empty() {
                continue;
            }
            let Some(fp) = p.rect(l.footprint) else { continue };
            let entity = EntityRef(l.placement.as_uuid());
            let style = self.style(LAYOUT_LABEL_STYLE, entity);
            let shaped = match &l.shaped {
                Some(s) => Arc::clone(s),
                None => Arc::new(self.fonts.shape(&l.text, &style, None)),
            };
            let m = self.fonts.vertical_metrics(&style);
            let across = m.cap_height.0 + if has_descender(&l.text) { m.descent.0 } else { 0 };
            let w = Nm(shaped.advance().0 + 2 * LABEL_PAD.0);
            let h = Nm(across + 2 * LABEL_PAD.0);
            let (at, angle, align) = match fit(w, h, fp.width(), fp.height()) {
                Some(LabelFit::Horizontal) => (centre(fp), Angle2::ZERO, Align::Centre),
                Some(LabelFit::Turned) => (centre(fp), Angle2(90_000), Align::Centre),
                _ => {
                    let cx = centre(fp).x;
                    let top = fp.min().y;
                    let above_top = Nm(top.0 - LEADER_GAP.0 - h.0);
                    let below = bounds.is_some_and(|b| above_top < b.min().y);
                    let (edge, end, align) = if below {
                        let bottom = fp.max().y;
                        (bottom, Nm(bottom.0 + LEADER_GAP.0), Align::TopCentre)
                    } else {
                        (top, Nm(top.0 - LEADER_GAP.0), Align::BottomCentre)
                    };
                    self.push(
                        Item::path(
                            Path::line(Pt2::new(cx, end), Pt2::new(cx, edge)),
                            Some(Stroke::pen(Pen::P013)),
                            None,
                        )
                        .with_entity(entity),
                    );
                    (Pt2::new(cx, end), Angle2::ZERO, align)
                }
            };
            self.text_shaped(
                &TextSpec {
                    text: &l.text,
                    at,
                    angle,
                    align: &align,
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
    let m = fonts.vertical_metrics(style);
    let across = m.cap_height.0 + if has_descender(text) { m.descent.0 } else { 0 };
    fit(
        Nm(shaped.advance().0 + 2 * LABEL_PAD.0),
        Nm(across + 2 * LABEL_PAD.0),
        fw,
        fh,
    )
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
