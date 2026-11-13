// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shapes into page items (doc 14 §6.1, §6.2): the drawing unit to nanometres, `round_half_even(v × unit_nm)` after a
//! finiteness and range check, the y flip `y_sheet = y_top − y`, angles as `Angle2`, texts in the nearest built-in text
//! style, and the report entries of what the conversion noticed.

use std::collections::{BTreeMap, BTreeSet};

use ew_interop_api::ew_schema::{
    BUILTIN_TEXT_STYLES, FreeText, Graphic, GraphicShape, GraphicStroke, LineDash, MAX_SAFE_INTEGER,
    PageItemKind, ShapeArc, ShapeCircle, ShapeLine, ShapePoints, TextAlign, qet_px_em,
};
use ew_interop_api::{Code, ImportError, Limits, ReportEntry, Severity, SourceLocation};
use ew_mltext::{Ident, MlText};
use ew_units::{Angle2, Nm, Pt2, Rect};

use crate::geom::{Affine, Converter, Geo, HAlign, Pattern, Shape, TOLERANCE_NM};
use crate::parse::{Drawing, parse};

/// A drawing unit (`$INSUNITS`, or the user's choice when the file names none).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Unit {
    /// Micrometres.
    Micrometre,
    /// Millimetres.
    Millimetre,
    /// Centimetres.
    Centimetre,
    /// Decimetres.
    Decimetre,
    /// Metres.
    Metre,
    /// Inches.
    Inch,
    /// Feet.
    Foot,
    /// Yards.
    Yard,
}

impl Unit {
    /// Every unit, in the order the import dialog offers them.
    pub const ALL: [Unit; 8] = [
        Unit::Millimetre,
        Unit::Centimetre,
        Unit::Metre,
        Unit::Inch,
        Unit::Foot,
        Unit::Micrometre,
        Unit::Decimetre,
        Unit::Yard,
    ];

    /// Nanometres per unit.
    pub fn nm(self) -> f64 {
        match self {
            Unit::Micrometre => 1_000.0,
            Unit::Millimetre => 1_000_000.0,
            Unit::Centimetre => 10_000_000.0,
            Unit::Decimetre => 100_000_000.0,
            Unit::Metre => 1_000_000_000.0,
            Unit::Inch => 25_400_000.0,
            Unit::Foot => 304_800_000.0,
            Unit::Yard => 914_400_000.0,
        }
    }

    /// The unit of a `$INSUNITS` value; `None` for unitless (0) and the units no drawing of a cabinet uses.
    pub fn from_insunits(code: i64) -> Option<Unit> {
        Some(match code {
            1 => Unit::Inch,
            2 => Unit::Foot,
            4 => Unit::Millimetre,
            5 => Unit::Centimetre,
            6 => Unit::Metre,
            10 => Unit::Yard,
            13 => Unit::Micrometre,
            14 => Unit::Decimetre,
            _ => return None,
        })
    }

    /// The short name (`mm`, `cm`, `m`, `in`, `ft`, `um`, `dm`, `yd`), as `--units` takes it.
    pub fn name(self) -> &'static str {
        match self {
            Unit::Micrometre => "um",
            Unit::Millimetre => "mm",
            Unit::Centimetre => "cm",
            Unit::Decimetre => "dm",
            Unit::Metre => "m",
            Unit::Inch => "in",
            Unit::Foot => "ft",
            Unit::Yard => "yd",
        }
    }

    /// The unit of a short name.
    pub fn parse(name: &str) -> Option<Unit> {
        Unit::ALL.into_iter().find(|u| u.name() == name.trim())
    }
}

/// Which space of the drawing is read.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum Space {
    /// Model space.
    #[default]
    Model,
    /// The paper-space layout of this name.
    Layout(String),
}

impl Space {
    /// The layout part of a source key: `Model` or the layout's name.
    pub fn key(&self) -> &str {
        match self {
            Space::Model => "Model",
            Space::Layout(name) => name,
        }
    }
}

/// What the import dialog shows before an import (doc 14 §13 "DXF import"): the version, the unit, the layouts,
/// the layers and the named blocks, and the extents of the model space.
#[derive(Clone, PartialEq, Debug)]
pub struct Probe {
    /// `$ACADVER` (`AC1009` for R12, `AC1015` for 2000, `AC1032` for 2018).
    pub version: String,
    /// Whether the file is binary DXF.
    pub binary: bool,
    /// The unit the header names, `None` without one (the dialog then asks, EW-IMP-251).
    pub unit: Option<Unit>,
    /// `$MEASUREMENT` says metric (the unit proposed when the header names none: mm, else in).
    pub metric: bool,
    /// The paper-space layouts in tab order.
    pub layouts: Vec<String>,
    /// The layers and whether each is off or frozen, by name.
    pub layers: Vec<(String, bool)>,
    /// The named blocks (no anonymous or external block), by name: the symbol wizard's choice.
    pub blocks: Vec<String>,
    /// The model space's extents in drawing units, `[min, max]`, from its geometry; `None` when empty.
    pub extents: Option<[(f64, f64); 2]>,
}

/// The probe of `bytes` (see [`Probe`]).
///
/// # Errors
///
/// As the import: a file that is no DXF or exceeds a limit.
pub fn probe(bytes: &[u8], limits: &Limits) -> Result<Probe, ImportError> {
    let d = parse(bytes, limits)?;
    let unit = d.header_int("INSUNITS").and_then(Unit::from_insunits);
    let mut conv = Converter::new(&d, 1.0e-3, None);
    let model = model_entities(&d);
    conv.entities(&model, Affine::IDENTITY, "", 0)?;
    let extents = bounds(&conv.shapes);
    let mut blocks: Vec<String> = d
        .blocks
        .values()
        .filter(|b| b.flags & (1 | 4) == 0 && !b.name.starts_with('*'))
        .map(|b| b.name.clone())
        .collect();
    blocks.sort();
    Ok(Probe {
        version: d.version.clone(),
        binary: d.binary,
        unit,
        metric: d.header_int("MEASUREMENT").unwrap_or(1) != 0,
        layouts: d
            .layouts
            .iter()
            .filter(|l| !l.name.eq_ignore_ascii_case("Model"))
            .map(|l| l.name.clone())
            .collect(),
        layers: d.layers.values().map(|l| (l.name.clone(), l.hidden)).collect(),
        blocks,
        extents,
    })
}

/// The bounds of shapes in drawing units.
pub(crate) fn bounds(shapes: &[Shape]) -> Option<[(f64, f64); 2]> {
    let mut b: Option<[(f64, f64); 2]> = None;
    let mut add = |p: (f64, f64)| {
        if !(p.0.is_finite() && p.1.is_finite()) {
            return;
        }
        b = Some(match b {
            None => [p, p],
            Some([lo, hi]) => [(lo.0.min(p.0), lo.1.min(p.1)), (hi.0.max(p.0), hi.1.max(p.1))],
        });
    };
    for s in shapes {
        match &s.geo {
            Geo::Poly { pts, .. } | Geo::Fill { pts } => pts.iter().for_each(|p| add(*p)),
            Geo::Arc { c, r, .. } => {
                add((c.0 - r, c.1 - r));
                add((c.0 + r, c.1 + r));
            }
            Geo::Text { at, height, .. } => {
                add((at.0, at.1 - height / 2.0));
                add((at.0, at.1 + height / 2.0));
            }
            Geo::Dot(p) => add(*p),
        }
    }
    b
}

/// The model-space entities: those of ENTITIES outside paper space and owned by no paper-space block record.
fn model_entities(d: &Drawing) -> Vec<crate::parse::Entity> {
    let paper_records: BTreeSet<&String> = d
        .records
        .iter()
        .filter(|(_, name)| name.to_ascii_uppercase().starts_with("*PAPER_SPACE"))
        .map(|(h, _)| h)
        .collect();
    d.entities
        .iter()
        .filter(|e| !e.paper() && e.owner().is_none_or(|o| !paper_records.contains(&o)))
        .cloned()
        .collect()
}

/// The page items of one space, in page nanometres, y down: what the page import and a DXF underlay draw.
#[derive(Clone, PartialEq, Debug)]
pub struct Converted {
    /// Each item's source key (`dxf:<space>|<handle>`, `dxf:<space>|<insert>/<handle>`), its layer and what it draws.
    pub items: Vec<(String, Option<Ident>, PageItemKind)>,
    /// The bounds of the items on the page.
    pub extents: Option<Rect>,
    /// The unit the drawing was read in.
    pub unit: Unit,
    /// `$ACADVER`, `AC1009` when the header names none.
    pub version: String,
    /// What the conversion noticed (EW-IMP-002, -004, -251 … -255), in a stable order.
    pub entries: Vec<ReportEntry>,
}

/// Why a drawing cannot be read without a choice of its user.
pub const NO_UNIT: &str = "the drawing names no unit; choose one";

/// The page items of `space` of `bytes` read in `unit` (the header's unit when `None`); `show` names hidden layers
/// that are imported anyway; `file` names the source in the report.
///
/// # Errors
///
/// [`ImportError::Unsupported`] with [`NO_UNIT`] when neither the header nor `unit` names a unit, or for a layout the
/// drawing has not; else as [`parse`] and the expansion limits.
pub fn convert(
    bytes: &[u8],
    limits: &Limits,
    unit: Option<Unit>,
    space: &Space,
    show: &BTreeSet<String>,
    file: &str,
) -> Result<Converted, ImportError> {
    let d = parse(bytes, limits)?;
    let header = d.header_int("INSUNITS").and_then(Unit::from_insunits);
    let unit = header.or(unit).ok_or_else(|| ImportError::Unsupported {
        message: NO_UNIT.to_owned(),
    })?;
    let scale = unit.nm();
    let mut conv = Converter::new(&d, TOLERANCE_NM / scale, Some(show));
    let (entities, top_var) = match space {
        Space::Model => (model_entities(&d), "EXTMAX"),
        Space::Layout(name) => (layout_entities(&d, name)?, "PEXTMAX"),
    };
    conv.entities(&entities, Affine::IDENTITY, "", 0)?;
    let shapes = std::mem::take(&mut conv.shapes);
    let notes = std::mem::take(&mut conv.notes);
    // y_top: the top of the extents the header names (the page export writes the sheet's top there, so a page
    // re-imports in place), else of the geometry.
    let header_top = d
        .header_pt(top_var)
        .map(|p| p.1)
        .filter(|y| y.is_finite() && y.abs() < 1.0e15);
    let top = header_top
        .or_else(|| bounds(&shapes).map(|[_, hi]| hi.1))
        .unwrap_or(0.0);
    let loc = |item: Option<String>| SourceLocation {
        file: file.to_owned(),
        item,
        ..SourceLocation::default()
    };
    let mut entries = Vec::new();
    if header.is_none() {
        entries.push(
            ReportEntry::new(
                Code::imp(251),
                Severity::Warning,
                format!(
                    "The drawing names no unit; it was read in {} as chosen.",
                    words(unit)
                ),
            )
            .at(loc(None)),
        );
    }
    let mut cx = Emit {
        scale,
        top: round(top * scale),
        space: space.key().to_owned(),
        keys: BTreeMap::new(),
        items: Vec::new(),
        bad: Vec::new(),
    };
    for s in &shapes {
        cx.shape(s);
    }
    for key in &notes.malformed {
        entries.push(
            ReportEntry::new(
                Code::MALFORMED_ITEM,
                Severity::Error,
                "An item has a coordinate that is not a number and was not imported.",
            )
            .at(loc(Some(key.clone()))),
        );
    }
    for key in &cx.bad {
        entries.push(
            ReportEntry::new(
                Code::MALFORMED_ITEM,
                Severity::Error,
                "An item lies outside the coordinate range of a page and was not imported.",
            )
            .at(loc(Some(key.clone()))),
        );
    }
    for (key, why) in &notes.skipped {
        entries.push(
            ReportEntry::new(
                Code::LIMIT,
                Severity::Error,
                format!("An item was not imported: {why}."),
            )
            .at(loc(Some(key.clone()))),
        );
    }
    for (layer, n) in &notes.hidden {
        entries.push(
            ReportEntry::new(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                format!("{n} items on the layer {layer}, which is off or frozen, were not imported."),
            )
            .at(loc(None)),
        );
    }
    for (font, family) in &notes.shx {
        entries.push(
            ReportEntry::new(
                Code::imp(252),
                Severity::Warning,
                format!("Texts in the SHX font {font} are drawn in {family}; their widths may differ."),
            )
            .at(loc(None)),
        );
    }
    for (kind, n) in &notes.unsupported {
        entries.push(
            ReportEntry::new(
                Code::imp(253),
                Severity::Info,
                format!("{n} entities of the type {kind} were not imported."),
            )
            .at(loc(None)),
        );
    }
    let mut external = notes.external.clone();
    external.sort();
    external.dedup();
    for name in &external {
        entries.push(
            ReportEntry::new(
                Code::imp(254),
                Severity::Info,
                format!("The external file {name} was not opened; a frame stands in for it."),
            )
            .at(loc(None)),
        );
    }
    for (pattern, n) in &notes.patterns {
        entries.push(
            ReportEntry::new(
                Code::imp(255),
                Severity::Info,
                format!("{n} hatches in the pattern {pattern} were reduced to their outline."),
            )
            .at(loc(None)),
        );
    }
    let extents = item_bounds(&cx.items);
    Ok(Converted {
        items: cx.items,
        extents,
        unit,
        version: d.version.clone(),
        entries,
    })
}

/// A unit in words, for the report.
pub(crate) fn words(unit: Unit) -> &'static str {
    match unit {
        Unit::Micrometre => "micrometres",
        Unit::Millimetre => "millimetres",
        Unit::Centimetre => "centimetres",
        Unit::Decimetre => "decimetres",
        Unit::Metre => "metres",
        Unit::Inch => "inches",
        Unit::Foot => "feet",
        Unit::Yard => "yards",
    }
}

/// The entities of the paper-space layout `name`, with the model space drawn through its viewports.
fn layout_entities(d: &Drawing, name: &str) -> Result<Vec<crate::parse::Entity>, ImportError> {
    let layout = d
        .layouts
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| ImportError::Unsupported {
            message: format!("the drawing has no layout named {name}"),
        })?;
    let record = layout.record.clone();
    let record_name = record
        .as_ref()
        .and_then(|h| d.records.get(h))
        .map(|n| n.to_ascii_uppercase());
    // The active layout's entities are in ENTITIES (code 67); every other layout's in its *Paper_Space<n> block.
    let mut out: Vec<crate::parse::Entity> = match record_name.as_deref() {
        Some(n) if n != "*PAPER_SPACE" => d.blocks.get(n).map(|b| b.entities.clone()).unwrap_or_default(),
        _ => d
            .entities
            .iter()
            .filter(|e| e.paper() && (record.is_none() || e.owner().is_none() || e.owner() == record))
            .cloned()
            .collect(),
    };
    // Viewports (other than the layout's own, id 1) show the model space: scaled into the viewport and clipped to
    // it, each shape cut to the viewport's rectangle (`geom::clip`).
    let viewports: Vec<crate::parse::Entity> = out
        .iter()
        .filter(|e| e.kind == "VIEWPORT" && e.i(69, 2) != 1)
        .cloned()
        .collect();
    out.retain(|e| e.kind != "VIEWPORT");
    for vp in &viewports {
        let c = vp.pt(10);
        let (w, h) = (vp.f(40, 0.0), vp.f(41, 0.0));
        let view = vp.pt(12);
        let view_h = vp.f(45, 0.0);
        if !(w > 0.0 && h > 0.0 && view_h > 0.0) {
            continue;
        }
        let s = h / view_h;
        // Model space under the viewport's map: the view centre to the viewport centre at scale s
        // (the converter's `EW-VIEWPORT`).
        out.push(crate::parse::Entity {
            kind: "EW-VIEWPORT".to_owned(),
            pairs: vec![
                pair(10, c.0),
                pair(20, c.1),
                pair(40, w),
                pair(41, h),
                pair(12, view.0),
                pair(22, view.1),
                pair(45, s),
            ],
            children: model_entities(d),
            line: vp.line,
        });
        if let (Some(h), Some(e)) = (vp.handle(), out.last_mut()) {
            e.pairs.push(crate::pairs::Pair {
                code: 5,
                val: crate::pairs::Val::Str(h),
                line: None,
            });
        }
    }
    Ok(out)
}

fn pair(code: i32, v: f64) -> crate::pairs::Pair {
    crate::pairs::Pair {
        code,
        val: crate::pairs::Val::Num(v),
        line: None,
    }
}

/// `v` rounded half to even, `None` outside the format's safe range.
fn nm(v: f64) -> Option<i64> {
    let r = v.round_ties_even();
    (r.is_finite() && r.abs() <= MAX_SAFE_INTEGER as f64).then_some(r as i64)
}

fn round(v: f64) -> i64 {
    nm(v).unwrap_or(0)
}

/// An angle in degrees as an `Angle2` in `[0, 360°)`.
fn angle2(deg: f64) -> Angle2 {
    let m = (deg * 1000.0).round_ties_even().rem_euclid(360_000.0);
    Angle2(m as i32 % 360_000)
}

/// A sweep in degrees as an `Angle2` (signed, at most one turn).
fn sweep2(deg: f64) -> Angle2 {
    Angle2((deg * 1000.0).round_ties_even().clamp(-360_000.0, 360_000.0) as i32)
}

/// The cap height of the bundled face per em (Noto Sans: 714 of 1000 units).
const CAP_PER_EM: f64 = 0.714;

/// The built-in text style whose em height is nearest (by ratio) to that of a text of cap height `cap` nm; `None`
/// for `text.normal`, the default.
fn style_of(cap: f64) -> Option<Ident> {
    let em = cap / CAP_PER_EM;
    let best = BUILTIN_TEXT_STYLES
        .iter()
        .map(|(key, px, _)| (*key, qet_px_em(*px).0 as f64))
        .filter(|(_, e)| *e > 0.0)
        .min_by(|a, b| {
            let da = (em / a.1).ln().abs();
            let db = (em / b.1).ln().abs();
            da.total_cmp(&db).then(a.0.cmp(b.0))
        })?;
    (best.0 != "text.normal")
        .then(|| Ident::from_input(best.0).ok())
        .flatten()
}

/// The page items of the shapes.
struct Emit {
    scale: f64,
    top: i64,
    space: String,
    keys: BTreeMap<String, u32>,
    items: Vec<(String, Option<Ident>, PageItemKind)>,
    bad: Vec<String>,
}

impl Emit {
    fn pt(&self, (x, y): (f64, f64)) -> Option<Pt2> {
        let x = nm(x * self.scale)?;
        let y = self.top.checked_sub(nm(y * self.scale)?)?;
        (y.abs() <= MAX_SAFE_INTEGER).then_some(Pt2::new(Nm(x), Nm(y)))
    }

    fn key(&mut self, own: &str) -> String {
        let base = format!("dxf:{}|{own}", self.space);
        let n = self.keys.entry(base.clone()).or_default();
        *n += 1;
        if *n == 1 {
            base
        } else {
            format!("{base}~{}", *n - 1)
        }
    }

    fn shape(&mut self, s: &Shape) {
        let key = self.key(&s.key);
        let layer = Ident::from_input(&s.layer).ok();
        let Some(kind) = self.kind(s) else {
            self.bad.push(key);
            return;
        };
        if let Some(kind) = kind {
            self.items.push((key, layer, kind));
        }
    }

    /// The page item of a shape; `Some(None)` for a shape that draws nothing, `None` for one out of range.
    fn kind(&self, s: &Shape) -> Option<Option<PageItemKind>> {
        let stroke = if s.width > 0.0 {
            Some(nm(s.width * self.scale)?)
        } else if s.width < 0.0 {
            Some(nm(-s.width * 1_000_000.0)?)
        } else {
            None
        }
        .filter(|w| *w > 0)
        .map(|w| GraphicStroke {
            width: Nm(w),
            extra: Default::default(),
        });
        let dash = match s.dash {
            Pattern::Solid => LineDash::Solid,
            Pattern::Dashed => LineDash::Dashed,
            Pattern::Dotted => LineDash::Dotted,
            Pattern::DashDot => LineDash::DashDot,
        };
        let graphic = |shape: GraphicShape, fill: bool| {
            PageItemKind::Graphic(Graphic {
                shape,
                stroke: stroke.clone(),
                dash: dash.clone(),
                fill,
                extra: Default::default(),
            })
        };
        let line = match &s.geo {
            Geo::Poly { pts, closed } => Some((pts, *closed, false)),
            Geo::Fill { pts } => Some((pts, true, true)),
            _ => None,
        };
        Some(Some(match &s.geo {
            Geo::Poly { .. } | Geo::Fill { .. } => {
                let Some((pts, closed, fill)) = line else {
                    return Some(None);
                };
                let mut points = Vec::with_capacity(pts.len());
                for p in pts {
                    points.push(self.pt(*p)?);
                }
                points.dedup();
                if points.len() < 2 {
                    return Some(None);
                }
                let shape = if closed && points.len() >= 3 {
                    GraphicShape::Polygon(ShapePoints {
                        points,
                        extra: Default::default(),
                    })
                } else if points.len() == 2 {
                    GraphicShape::Line(ShapeLine {
                        a: points[0],
                        b: points[1],
                        extra: Default::default(),
                    })
                } else {
                    GraphicShape::Polyline(ShapePoints {
                        points,
                        extra: Default::default(),
                    })
                };
                graphic(shape, fill)
            }
            Geo::Arc { c, r, start, sweep } => {
                let centre = self.pt(*c)?;
                let r = Nm(nm(r * self.scale)?);
                if r.0 <= 0 {
                    return Some(None);
                }
                if (sweep.abs() - 360.0).abs() < 1e-9 {
                    graphic(
                        GraphicShape::Circle(ShapeCircle {
                            centre,
                            r,
                            extra: Default::default(),
                        }),
                        false,
                    )
                } else {
                    graphic(
                        GraphicShape::Arc(ShapeArc {
                            centre,
                            r,
                            start: angle2(*start),
                            sweep: sweep2(*sweep),
                            extra: Default::default(),
                        }),
                        false,
                    )
                }
            }
            Geo::Dot(p) => graphic(
                GraphicShape::Circle(ShapeCircle {
                    centre: self.pt(*p)?,
                    r: Nm(100_000),
                    extra: Default::default(),
                }),
                true,
            ),
            Geo::Text {
                at,
                height,
                angle,
                text,
                align,
            } => {
                let cap = height * self.scale;
                if !cap.is_finite() || cap <= 0.0 {
                    return Some(None);
                }
                PageItemKind::FreeText(FreeText {
                    text: MlText::neutral(text),
                    bind: None,
                    at: self.pt(*at)?,
                    angle: angle2(*angle),
                    style: style_of(cap),
                    align: match align {
                        HAlign::Left => None,
                        HAlign::Centre => Some(TextAlign::Centre),
                        HAlign::Right => Some(TextAlign::Right),
                    },
                    extra: Default::default(),
                })
            }
        }))
    }
}

/// The bounds of page items (graphics by their points, texts by their anchor).
pub(crate) fn item_bounds(items: &[(String, Option<Ident>, PageItemKind)]) -> Option<Rect> {
    let mut r: Option<Rect> = None;
    let mut add = |p: Pt2| {
        r = Some(match r {
            None => Rect::from_point(p),
            Some(b) => Rect::from_points(
                Pt2::new(b.min().x.min(p.x), b.min().y.min(p.y)),
                Pt2::new(b.max().x.max(p.x), b.max().y.max(p.y)),
            ),
        });
    };
    for (_, _, kind) in items {
        match kind {
            PageItemKind::Graphic(g) => match &g.shape {
                GraphicShape::Line(l) => {
                    add(l.a);
                    add(l.b);
                }
                GraphicShape::Polyline(p) | GraphicShape::Polygon(p) => p.points.iter().for_each(|q| add(*q)),
                GraphicShape::Circle(c) => {
                    add(Pt2::new(c.centre.x - c.r, c.centre.y - c.r));
                    add(Pt2::new(c.centre.x + c.r, c.centre.y + c.r));
                }
                GraphicShape::Arc(a) => {
                    add(Pt2::new(a.centre.x - a.r, a.centre.y - a.r));
                    add(Pt2::new(a.centre.x + a.r, a.centre.y + a.r));
                }
                GraphicShape::Rect(rc) => {
                    add(rc.min);
                    add(rc.max);
                }
                GraphicShape::Unknown(_) => {}
            },
            PageItemKind::FreeText(t) => add(t.at),
            _ => {}
        }
    }
    r
}
