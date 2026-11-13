// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The DXF→symbol wizard's automatic steps (doc 14 §6.2, FR-IMP-034) and the part front outline (FR-IMP-035).
//!
//! For every named block: (1) its base point is the symbol origin, scaled to nanometres by the unit; (2) an attribute
//! definition tagged `TAG`, `DT`, `BMK` or `LABEL` gives the tag slot, one tagged `DESC…` or `FUNC…` the function-text
//! slot; (3) one tagged `PIN`, `CONN`, `TERM` or `X` with a suffix gives a connection point at its insertion point,
//! designated by its default value, else the suffix; (4) without them, the points and small circles on a layer named
//! `CONN…`, `PIN…` or `EW-CP` give the connection points, numbered in drawing order; (5) otherwise the author places
//! them with the symbol editor's connection-point tool. (6) Each connection point is snapped to the native connection
//! grid; one that moves more than 0.1 mm is reported (EW-IMP-256) with the uniform scale that would put the points'
//! spacing on the grid. (7) Its direction is the outward normal of the nearest side of the drawing's bounding box.
//! Step 8, saving into a user library with provenance and foreign terms, is the host's (`ew_api::symbols`).

use std::collections::BTreeSet;

use ew_interop_api::ew_schema::{
    Align, ArcPrim, Bounds, CirclePrim, Dash as LibDash, Dir4, LinePrim, Pen, PolygonPrim, PolylinePrim,
    Primitive, Stroke, TextPrim,
};
use ew_interop_api::{Code, ImportError, Limits, ReportEntry, Severity};
use ew_mltext::{Ident, MlText};
use ew_units::{Angle2, Grid, Nm, Pt2, Rect};

use crate::emit::{Unit, bounds, words};
use crate::geom::{Affine, Converter, Geo, HAlign, Pattern, TOLERANCE_NM};
use crate::parse::{Entity, parse};

/// The most a connection point may move when snapped before it is reported: 0.1 mm.
pub const SNAP_REPORT_NM: i64 = 100_000;

/// The radius up to which a circle on a connection layer marks a connection point: 2 mm.
const CP_CIRCLE_NM: f64 = 2_000_000.0;

/// One connection point of a block symbol.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolPin {
    /// The printed designation.
    pub designation: String,
    /// The position on the native grid, in the symbol frame (nm, y down).
    pub pos: Pt2,
    /// The direction a wire leaves it.
    pub dir: Dir4,
}

/// A text slot found in a block's attribute definitions.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolSlot {
    /// The position in the symbol frame.
    pub pos: Pt2,
    /// The angle.
    pub angle: Angle2,
}

/// One block as a symbol (doc 14 §6.2's steps 1 to 7).
#[derive(Clone, PartialEq, Debug)]
pub struct BlockSymbol {
    /// The block's name.
    pub name: String,
    /// The drawing in the symbol frame (nm, y down, the base point at the origin).
    pub graphics: Vec<Primitive>,
    /// The connection points, in designation order of their source.
    pub pins: Vec<SymbolPin>,
    /// The tag slot (step 2).
    pub tag: Option<SymbolSlot>,
    /// The function-text slot (step 2).
    pub function_text: Option<SymbolSlot>,
    /// What the steps noticed: EW-IMP-251 (no unit), EW-IMP-256 (a point snapped), a block without connection points.
    pub entries: Vec<ReportEntry>,
}

fn to_nm(v: f64) -> Option<i64> {
    let r = v.round_ties_even();
    (r.is_finite() && r.abs() < 4.0e15).then_some(r as i64)
}

/// The attribute definitions of a block whose tag matches a CP pattern: `(PIN|CONN|TERM|X)_?(\w+)`, the suffix.
fn cp_suffix(tag: &str) -> Option<&str> {
    let upper = tag.to_ascii_uppercase();
    for prefix in ["PIN", "CONN", "TERM", "X"] {
        if let Some(rest) = upper.strip_prefix(prefix) {
            let rest = rest.strip_prefix('_').unwrap_or(rest);
            if !rest.is_empty() && rest.chars().all(|c| c.is_alphanumeric() || c == '_') {
                let start = tag.len() - rest.len();
                return Some(&tag[start..]);
            }
        }
    }
    None
}

/// The blocks of `bytes` as symbols on `grid`, read in `unit` (the header's when `None`); `file` names the source.
///
/// # Errors
///
/// As [`crate::convert`]: no unit, no DXF, a limit.
pub fn symbols(
    bytes: &[u8],
    limits: &Limits,
    unit: Option<Unit>,
    grid: Grid,
    file: &str,
) -> Result<Vec<BlockSymbol>, ImportError> {
    let d = parse(bytes, limits)?;
    let header = d.header_int("INSUNITS").and_then(Unit::from_insunits);
    let unit = header.or(unit).ok_or_else(|| ImportError::Unsupported {
        message: crate::NO_UNIT.to_owned(),
    })?;
    let scale = unit.nm();
    let mut out = Vec::new();
    for block in d.blocks.values() {
        if block.flags & (1 | 4) != 0 || block.name.starts_with('*') {
            continue;
        }
        let mut conv = Converter::new(&d, TOLERANCE_NM / scale, None);
        conv.block_shapes(&block.name)?;
        let shapes = std::mem::take(&mut conv.shapes);
        if shapes.is_empty() && !block.entities.iter().any(|e| e.kind == "ATTDEF") {
            continue;
        }
        let base = block.base;
        let local = |p: (f64, f64)| -> Option<Pt2> {
            Some(Pt2::new(
                Nm(to_nm((p.0 - base.0) * scale)?),
                Nm(to_nm(-(p.1 - base.1) * scale)?),
            ))
        };
        let frame = |p: (f64, f64)| -> Option<Pt2> {
            // Shapes are already relative to the base point.
            Some(Pt2::new(Nm(to_nm(p.0 * scale)?), Nm(to_nm(-p.1 * scale)?)))
        };
        let mut graphics = Vec::new();
        for s in &shapes {
            if let Some(p) = primitive(&s.geo, s.dash, s.width, scale, &frame) {
                graphics.push(p);
            }
        }
        let bbox = bounds(&shapes).and_then(|[lo, hi]| Some(Rect::from_points(frame(lo)?, frame(hi)?)));
        let mut entries: Vec<ReportEntry> = Vec::new();
        if header.is_none() {
            entries.push(ReportEntry::new(
                Code::imp(251),
                Severity::Warning,
                format!(
                    "The drawing names no unit; it was read in {} as chosen.",
                    words(unit)
                ),
            ));
        }
        // Steps 2 and 3: the attribute definitions.
        let mut tag = None;
        let mut function_text = None;
        let mut raw_pins: Vec<(String, Pt2)> = Vec::new();
        for e in block.entities.iter().filter(|e| e.kind == "ATTDEF") {
            let name = e.str(2).unwrap_or_default().trim().to_owned();
            let upper = name.to_ascii_uppercase();
            let ocs = Affine::ocs(e.extrusion());
            let at = if e.i(72, 0) != 0 || e.i(73, 0) != 0 {
                e.pt(11)
            } else {
                e.pt(10)
            };
            let Some(pos) = local(ocs.apply(at)) else { continue };
            let slot = || SymbolSlot {
                pos,
                angle: Angle2(((e.f(50, 0.0) * 1000.0).round_ties_even().rem_euclid(360_000.0)) as i32),
            };
            if matches!(upper.as_str(), "TAG" | "DT" | "BMK" | "LABEL") {
                tag.get_or_insert_with(slot);
            } else if upper.starts_with("DESC") || upper.starts_with("FUNC") {
                function_text.get_or_insert_with(slot);
            } else if let Some(suffix) = cp_suffix(&name) {
                let default = e.str(1).unwrap_or_default().trim().to_owned();
                raw_pins.push((
                    if default.is_empty() {
                        suffix.to_owned()
                    } else {
                        default
                    },
                    pos,
                ));
            }
        }
        // Step 4: points and small circles on a connection layer.
        if raw_pins.is_empty() {
            let mut n = 0;
            for e in block.entities.iter().filter(|e| on_cp_layer(e)) {
                let centre = match e.kind.as_str() {
                    "POINT" => Some(e.pt(10)),
                    "CIRCLE" if e.f(40, f64::INFINITY) * scale <= CP_CIRCLE_NM => {
                        Some(Affine::ocs(e.extrusion()).apply(e.pt(10)))
                    }
                    _ => None,
                };
                if let Some(pos) = centre.and_then(local) {
                    n += 1;
                    raw_pins.push((n.to_string(), pos));
                }
            }
        }
        if raw_pins.is_empty() {
            entries.push(ReportEntry::new(
                Code::DROPPED_BY_DESIGN,
                Severity::Info,
                format!(
                    "The block {} names no connection points; place them with the connection-point tool.",
                    block.name
                ),
            ));
        }
        // Steps 6 and 7: snap to the grid, direction by the nearest side.
        let mut moved = 0i64;
        let mut pins = Vec::new();
        let mut seen = BTreeSet::new();
        for (designation, pos) in raw_pins {
            let snap = |v: Nm| grid.snap_point(v).unwrap_or(v);
            let snapped = Pt2::new(snap(pos.x), snap(pos.y));
            moved = moved
                .max((snapped.x.0 - pos.x.0).abs())
                .max((snapped.y.0 - pos.y.0).abs());
            if !seen.insert(designation.clone()) {
                continue;
            }
            pins.push(SymbolPin {
                dir: direction(bbox, pos),
                designation,
                pos: snapped,
            });
        }
        if moved > SNAP_REPORT_NM {
            let spacing = min_spacing(&pins);
            let pitch = grid.num() as f64 / grid.den() as f64;
            let proposal = spacing
                .filter(|s| *s > 0.0)
                .map(|s| {
                    let steps = (s / pitch).round().max(1.0);
                    format!(
                        " Scaling the drawing by {:.3} would put their spacing on the grid.",
                        steps * pitch / s
                    )
                })
                .unwrap_or_default();
            entries.push(ReportEntry::new(
                Code::imp(256),
                Severity::Warning,
                format!(
                    "Connection points of the block {} moved by up to {:.2} mm onto the connection grid.{proposal}",
                    block.name,
                    moved as f64 / 1.0e6
                ),
            ));
        }
        for entry in &mut entries {
            entry.location = Some(ew_interop_api::SourceLocation {
                file: file.to_owned(),
                item: Some(block.name.clone()),
                ..Default::default()
            });
        }
        out.push(BlockSymbol {
            name: block.name.clone(),
            graphics,
            pins,
            tag,
            function_text,
            entries,
        });
    }
    Ok(out)
}

/// Whether an entity lies on a connection layer (`CONN…`, `PIN…`, `EW-CP`).
fn on_cp_layer(e: &Entity) -> bool {
    let l = e.layer().to_ascii_uppercase();
    l.starts_with("CONN") || l.starts_with("PIN") || l == "EW-CP"
}

/// The outward normal of the side of `bbox` nearest to `p` (y down: the top side is `Up`).
fn direction(bbox: Option<Rect>, p: Pt2) -> Dir4 {
    let Some(b) = bbox else { return Dir4::Up };
    let (lo, hi) = (b.min(), b.max());
    let d = [
        ((p.y.0 - lo.y.0).abs(), Dir4::Up),
        ((hi.x.0 - p.x.0).abs(), Dir4::Right),
        ((hi.y.0 - p.y.0).abs(), Dir4::Down),
        ((p.x.0 - lo.x.0).abs(), Dir4::Left),
    ];
    d.into_iter()
        .min_by_key(|(dist, _)| *dist)
        .map_or(Dir4::Up, |(_, dir)| dir)
}

/// The smallest non-zero distance along x or y between two connection points, in nm.
fn min_spacing(pins: &[SymbolPin]) -> Option<f64> {
    let mut best: Option<i64> = None;
    for (i, a) in pins.iter().enumerate() {
        for b in &pins[i + 1..] {
            for d in [(a.pos.x.0 - b.pos.x.0).abs(), (a.pos.y.0 - b.pos.y.0).abs()] {
                if d > 0 && best.is_none_or(|x| d < x) {
                    best = Some(d);
                }
            }
        }
    }
    best.map(|b| b as f64)
}

/// A shape as a symbol primitive in the frame `frame` maps to.
fn primitive(
    geo: &Geo,
    dash: Pattern,
    width: f64,
    scale: f64,
    frame: &dyn Fn((f64, f64)) -> Option<Pt2>,
) -> Option<Primitive> {
    let pen = if width > 0.0 {
        Pen::Width(Nm(to_nm(width * scale)?))
    } else if width < 0.0 {
        Pen::Width(Nm(to_nm(-width * 1.0e6)?))
    } else {
        Pen::P026
    };
    let stroke = Some(Stroke {
        pen,
        dash: match dash {
            Pattern::Solid => LibDash::Solid,
            Pattern::Dashed => LibDash::Dash42,
            Pattern::Dotted => LibDash::Dot12,
            Pattern::DashDot => LibDash::DashDot,
        },
        colour: Default::default(),
        extra: Default::default(),
    });
    Some(match geo {
        Geo::Poly { pts, closed } => {
            let points: Vec<Pt2> = pts.iter().map(|p| frame(*p)).collect::<Option<_>>()?;
            if *closed && points.len() >= 3 {
                Primitive::Polygon(PolygonPrim {
                    points,
                    closed: true,
                    stroke,
                    fill: None,
                    extra: Default::default(),
                })
            } else if points.len() == 2 {
                Primitive::Line(LinePrim {
                    from: points[0],
                    to: points[1],
                    stroke,
                    extra: Default::default(),
                })
            } else {
                Primitive::Polyline(PolylinePrim {
                    points,
                    stroke,
                    extra: Default::default(),
                })
            }
        }
        Geo::Fill { pts } => Primitive::Polygon(PolygonPrim {
            points: pts.iter().map(|p| frame(*p)).collect::<Option<_>>()?,
            closed: true,
            stroke,
            fill: Some(ew_interop_api::ew_schema::Colour::Rgb(
                ew_interop_api::ew_schema::Rgb([0, 0, 0]),
            )),
            extra: Default::default(),
        }),
        Geo::Arc { c, r, start, sweep } => {
            let lo = frame((c.0 - r, c.1 + r))?;
            let hi = frame((c.0 + r, c.1 - r))?;
            let rect = Bounds::new(lo, hi).ok()?;
            if (sweep.abs() - 360.0).abs() < 1e-9 {
                Primitive::Circle(CirclePrim {
                    centre: frame(*c)?,
                    r: Nm(to_nm(r * scale)?),
                    stroke,
                    fill: None,
                    extra: Default::default(),
                })
            } else {
                Primitive::Arc(ArcPrim {
                    rect,
                    start: Angle2(((start * 1000.0).round_ties_even().rem_euclid(360_000.0)) as i32),
                    sweep: Angle2((sweep * 1000.0).round_ties_even().clamp(-360_000.0, 360_000.0) as i32),
                    stroke,
                    extra: Default::default(),
                })
            }
        }
        Geo::Dot(p) => Primitive::Circle(CirclePrim {
            centre: frame(*p)?,
            r: Nm(100_000),
            stroke: None,
            fill: Some(ew_interop_api::ew_schema::Colour::Rgb(
                ew_interop_api::ew_schema::Rgb([0, 0, 0]),
            )),
            extra: Default::default(),
        }),
        Geo::Text {
            at,
            angle,
            text,
            align,
            ..
        } => Primitive::Text(TextPrim {
            pos: frame(*at)?,
            text: MlText::neutral(text),
            style: Ident::from_input("text.normal").ok(),
            align: match align {
                HAlign::Left => Align::MiddleLeft,
                HAlign::Centre => Align::Centre,
                HAlign::Right => Align::MiddleRight,
            },
            angle: Angle2(((angle * 1000.0).round_ties_even().rem_euclid(360_000.0)) as i32),
            extra: Default::default(),
        }),
    })
}

/// The part front outline of a DXF drawing (FR-IMP-035): every line, polyline, arc, circle, ellipse and spline of the
/// model space as a polyline in the part frame (nm, the extents' lower-left corner at the origin, y up), curves
/// flattened within 0.01 mm. Texts, fills and points are not outline.
///
/// # Errors
///
/// As [`crate::convert`].
pub fn outline(bytes: &[u8], limits: &Limits, unit: Option<Unit>) -> Result<Vec<Vec<Pt2>>, ImportError> {
    let d = parse(bytes, limits)?;
    let unit = d
        .header_int("INSUNITS")
        .and_then(Unit::from_insunits)
        .or(unit)
        .ok_or_else(|| ImportError::Unsupported {
            message: crate::NO_UNIT.to_owned(),
        })?;
    let scale = unit.nm();
    let mut conv = Converter::new(&d, TOLERANCE_NM / scale, None);
    let model: Vec<Entity> = d.entities.iter().filter(|e| !e.paper()).cloned().collect();
    conv.entities(&model, Affine::IDENTITY, "", 0)?;
    let lines: Vec<(Vec<(f64, f64)>, bool)> = conv
        .shapes
        .iter()
        .filter_map(|s| match &s.geo {
            Geo::Poly { pts, closed } => Some((pts.clone(), *closed)),
            Geo::Arc { c, r, start, sweep } => Some((
                crate::geom::arc_points(*c, *r, *start, *sweep, TOLERANCE_NM / scale),
                false,
            )),
            _ => None,
        })
        .collect();
    let lo = lines
        .iter()
        .flat_map(|(pts, _)| pts.iter())
        .fold((f64::INFINITY, f64::INFINITY), |a, p| {
            (a.0.min(p.0), a.1.min(p.1))
        });
    let mut out = Vec::new();
    for (pts, closed) in lines {
        let mut line: Vec<Pt2> = pts
            .iter()
            .map(|p| {
                Some(Pt2::new(
                    Nm(to_nm((p.0 - lo.0) * scale)?),
                    Nm(to_nm((p.1 - lo.1) * scale)?),
                ))
            })
            .collect::<Option<_>>()
            .ok_or_else(|| ImportError::Malformed {
                message: "an outline coordinate is outside the range of a part".to_owned(),
                line: None,
            })?;
        if closed {
            if let Some(first) = line.first().copied() {
                line.push(first);
            }
        }
        line.dedup();
        if line.len() >= 2 {
            out.push(line);
        }
    }
    Ok(out)
}
