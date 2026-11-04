// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The wire-colour view (WP-S2.08 D18, FR-UI-047, doc 16 §10.3): view state of the window, never a model edit and
//! never in a display list, so the PDF and the PNG exports never see it.
//!
//! While View ▸ Wire colours is on, every connection drawn on the shown page whose effective colour is known is
//! stroked [`STROKE_PT`] points wide over the sheet, with its code at the midpoint of its drawn path (by length). The
//! effective colour is a list of one or two codes (`Session::wire_colours`); the label is the codes as the wire list
//! prints them, concatenated (`DBU`, `GNYE`, `DBUWH`). Each code's display colour:
//!
//! - an IEC 60757 code: [`iec_rgb`];
//! - a company code the colour profile declares (FR-CON-019): its declared base, darkened or lightened by its declared
//!   shade ([`company_rgb`]), never read from its spelling;
//! - any other code (a foreign code, a company code the profile does not declare): no colour. A stroke with such a
//!   code is drawn as the unresolved pattern instead, [`UNRESOLVED_DASH_NM`] dashes of [`CASING_RGB`] with equal gaps,
//!   so it is never mistaken for a colour.
//!
//! One colour is a solid stroke; two alternate in [`DASH_NM`] (2 mm on the sheet) dashes, the first colour first. The
//! dashes are walked over the path clipped to the sheet, so their number is bounded by the sheet, not by the path
//! (C49-p8#5). A stroke with a colour under 3:1 against the white sheet ([`contrast_on_white`], NFR-A11Y-03) is cased:
//! a [`CASING_RGB`] outline [`CASING_PX`] physical pixel wide on both sides, painted under it.

pub use ew_api::output::CompanyColour;

/// The stroke's width in logical points.
pub const STROKE_PT: f64 = 3.0;

/// The casing's width in physical pixels, on each side of a cased stroke.
pub const CASING_PX: f64 = 1.0;

/// The casing's colour, and the unresolved pattern's (`slate-900`).
pub const CASING_RGB: (u8, u8, u8) = (0x1A, 0x20, 0x2C);

/// A two-colour wire's dash length on the sheet: 2 mm in nanometres.
pub const DASH_NM: f64 = 2_000_000.0;

/// The unresolved pattern's dash (and gap) length on the sheet: 1 mm in nanometres.
pub const UNRESOLVED_DASH_NM: f64 = 1_000_000.0;

/// The least contrast a stroke keeps against the white sheet without a casing (NFR-A11Y-03).
pub const MIN_CONTRAST: f64 = 3.0;

/// The display colour of an IEC 60757 code.
pub fn iec_rgb(code: &str) -> Option<(u8, u8, u8)> {
    Some(match code {
        "BK" => (0x00, 0x00, 0x00),
        "BN" => (0x8B, 0x4A, 0x1C),
        "RD" => (0xD7, 0x19, 0x1C),
        "OG" => (0xF5, 0x82, 0x1F),
        "YE" => (0xFF, 0xD7, 0x00),
        "GN" => (0x1A, 0x9A, 0x3B),
        "BU" => (0x1F, 0x5F, 0xD6),
        "VT" => (0x7B, 0x2C, 0xBF),
        "GY" => (0x80, 0x80, 0x80),
        "WH" => (0xFF, 0xFF, 0xFF),
        "PK" => (0xF2, 0x8C, 0xB8),
        "GD" => (0xC9, 0xA2, 0x27),
        "TQ" => (0x2B, 0xB5, 0xB0),
        "SR" => (0xB8, 0xB8, 0xB8),
        _ => return None,
    })
}

/// The IEC 60757 codes, in the order of IEC 60757.
pub const IEC_CODES: [&str; 14] = [
    "BK", "BN", "RD", "OG", "YE", "GN", "BU", "VT", "GY", "WH", "PK", "GD", "TQ", "SR",
];

/// The display colour of `code` as the profile declares it in `company`: its base's colour, each channel × 0.55 for
/// the `dark` shade, halfway to white for `light`; `None` when no declared code is `code` or its base is no IEC code.
pub fn company_rgb(code: &str, company: &[CompanyColour]) -> Option<(u8, u8, u8)> {
    let declared = company.iter().find(|c| c.code == code)?;
    let (r, g, b) = iec_rgb(&declared.base)?;
    let dark = |c: u8| (f64::from(c) * 0.55).round() as u8;
    let light = |c: u8| c + (255 - c) / 2;
    Some(match declared.shade.as_deref() {
        Some("dark") => (dark(r), dark(g), dark(b)),
        Some("light") => (light(r), light(g), light(b)),
        _ => (r, g, b),
    })
}

/// The display colour of one code: IEC 60757's, else the declared company code's; `None` when it resolves to neither.
pub fn colour_of(code: &str, company: &[CompanyColour]) -> Option<(u8, u8, u8)> {
    iec_rgb(code).or_else(|| company_rgb(code, company))
}

/// The WCAG 2 contrast ratio of `rgb` against white.
pub fn contrast_on_white((r, g, b): (u8, u8, u8)) -> f64 {
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    1.05 / (l + 0.05)
}

/// A piece of a stroke in sheet nanometres: its two ends, the index of its colour in [`WireStroke::colours`] and the
/// lane of its segment ([`assign_lanes`]).
pub type Piece = ((f64, f64), (f64, f64), usize, u8);

/// The distance of two lanes in logical points: a stroke and its casing on both sides, and a gap.
pub const LANE_PT: f64 = STROKE_PT + 2.0;

/// How far a stroke on `lane` stands off its path, in logical points, along the segment's normal ([`normal`]): lane 0
/// on it, then alternately one [`LANE_PT`] to one side and the other, further out (lane 1 +1, lane 2 −1, lane 3 +2 …).
pub fn lane_offset_pt(lane: u8) -> f64 {
    let k = f64::from(lane.div_ceil(2));
    if lane % 2 == 1 { k * LANE_PT } else { -k * LANE_PT }
}

/// The unit normal of the line through `a` and `b` (turned a quarter from its direction); `(0, 0)` for a point. One
/// normal per line (C49-p8b#1): the direction runs from the lower end to the higher (by x, then y), so two segments of
/// one line drawn in opposite directions share it, and a lane is the same side of the line for both.
pub fn normal(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (a, b) = if b < a { (b, a) } else { (a, b) };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len > 0.0 && len.is_finite() {
        (-dy / len, dx / len)
    } else {
        (0.0, 0.0)
    }
}

/// Where a stroke's code stands: the midpoint of its path, the normal of the segment it lies on and that segment's
/// lane, so the code sits on its own stroke (W49.2 F9).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LabelAt {
    /// The midpoint in sheet nanometres.
    pub at: (f64, f64),
    /// The unit normal of its segment.
    pub normal: (f64, f64),
    /// Its segment's lane.
    pub lane: u8,
}

/// Whether the segments `a`–`b` and `c`–`d` lie on one line and share a stretch of positive length.
///
/// Any `i64` ends (C49-p8b#4): the differences are taken in `i128`, the direction reduced to its lowest terms, and a
/// cross product beyond `i128` (a slant across most of the `i64` range) counts as no shared stretch.
fn overlaps(a: (i64, i64), b: (i64, i64), c: (i64, i64), d: (i64, i64)) -> bool {
    let v = (
        i128::from(b.0) - i128::from(a.0),
        i128::from(b.1) - i128::from(a.1),
    );
    if v == (0, 0) {
        return false;
    }
    let (mut x, mut y) = (v.0.unsigned_abs(), v.1.unsigned_abs());
    while y != 0 {
        (x, y) = (y, x % y);
    }
    // `x` is the greatest common divisor, at least 1 and at most |v.0| or |v.1|: the quotients fit.
    let g = i128::try_from(x).unwrap_or(1);
    let dir = (v.0 / g, v.1 / g);
    let on_line = |p: (i64, i64)| {
        let r = (
            i128::from(p.0) - i128::from(a.0),
            i128::from(p.1) - i128::from(a.1),
        );
        matches!((dir.0.checked_mul(r.1), dir.1.checked_mul(r.0)), (Some(x), Some(y)) if x == y)
    };
    if !on_line(c) || !on_line(d) {
        return false;
    }
    // On one line: the stretches compared along its longer axis.
    let axis = |p: (i64, i64)| {
        if v.0.abs() >= v.1.abs() {
            i128::from(p.0)
        } else {
            i128::from(p.1)
        }
    };
    let span = |p: (i64, i64), q: (i64, i64)| (axis(p).min(axis(q)), axis(p).max(axis(q)));
    let ((a0, a1), (c0, c1)) = (span(a, b), span(c, d));
    a0.max(c0) < a1.min(c1)
}

/// Gives each segment of each stroke a lane (W49.2 F9): a segment that shares a stretch of its line with segments of
/// strokes before it takes a lane none of them holds, so two wires drawn on one path stand side by side and each keeps
/// its colour or stripes; a segment alone keeps lane 0, on its path. FT-1283: a run of such segments one after the
/// other (a shared run that turns a corner) takes one lane, the lowest that none of the segments it shares with holds,
/// so the wire keeps its lane over the run and its corner turns with it ([`WireStroke::in_lanes`]). FT-1344: a segment
/// that shares only part of its stretch is first split where the shared part begins and ends, so only that part takes
/// the lane and the rest stays on the path.
pub fn assign_lanes(strokes: &mut [WireStroke]) {
    for j in 0..strokes.len() {
        let (earlier, rest) = strokes.split_at_mut(j);
        let stroke = &mut rest[0];
        stroke.polyline = split_at_shared(&stroke.polyline, earlier);
        // Per segment, the lanes the earlier strokes' segments it shares a stretch with hold; `None` when it shares
        // none.
        let used: Vec<Option<Vec<u8>>> = stroke
            .polyline
            .windows(2)
            .map(|w| {
                let mut shared = false;
                let used: Vec<u8> = earlier
                    .iter()
                    .flat_map(|e| {
                        e.polyline
                            .windows(2)
                            .zip(&e.lanes)
                            .filter(|(x, _)| overlaps(w[0], w[1], x[0], x[1]))
                            .map(|(_, l)| *l)
                    })
                    .inspect(|_| shared = true)
                    .collect();
                shared.then_some(used)
            })
            .collect();
        let mut lanes = vec![0u8; used.len()];
        let mut i = 0;
        while i < used.len() {
            if used[i].is_none() {
                i += 1;
                continue;
            }
            let end = (i..used.len()).find(|k| used[*k].is_none()).unwrap_or(used.len());
            let held: Vec<u8> = used[i..end].iter().flatten().flatten().copied().collect();
            let lane = (0..=u8::MAX).find(|l| !held.contains(l)).unwrap_or(0);
            lanes[i..end].fill(lane);
            i = end;
        }
        stroke.lanes = lanes;
    }
}

/// `polyline` with a vertex added on each segment wherever a segment of `earlier` that shares a stretch with it ends
/// inside it (FT-1344), in the segment's order: the drawn path is the same, and each piece is either shared along its
/// whole length or not at all.
fn split_at_shared(polyline: &[(i64, i64)], earlier: &[WireStroke]) -> Vec<(i64, i64)> {
    let Some(&first) = polyline.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    for w in polyline.windows(2) {
        let (a, b) = (w[0], w[1]);
        // Along the segment's longer axis, from `a` towards `b`.
        let along = |p: (i64, i64)| {
            let (dx, dy) = (i128::from(b.0) - i128::from(a.0), i128::from(b.1) - i128::from(a.1));
            if dx.abs() >= dy.abs() {
                (i128::from(p.0) - i128::from(a.0)) * dx.signum()
            } else {
                (i128::from(p.1) - i128::from(a.1)) * dy.signum()
            }
        };
        let length = along(b);
        let mut cuts: Vec<(i128, (i64, i64))> = earlier
            .iter()
            .flat_map(|e| e.polyline.windows(2))
            .filter(|x| overlaps(a, b, x[0], x[1]))
            .flat_map(|x| [x[0], x[1]])
            .map(|p| (along(p), p))
            .filter(|(t, _)| *t > 0 && *t < length)
            .collect();
        cuts.sort_unstable();
        cuts.dedup_by_key(|(t, _)| *t);
        out.extend(cuts.into_iter().map(|(_, p)| p));
        out.push(b);
    }
    out
}

/// The point where the lines through `a`–`b` and `c`–`d` cross; `None` for parallel lines.
fn crossing(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> Option<(f64, f64)> {
    let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
    let den = r.0 * s.1 - r.1 * s.0;
    let scale = (r.0.hypot(r.1) * s.0.hypot(s.1)).max(f64::MIN_POSITIVE);
    if den.is_nan() || den.abs() <= scale * 1e-9 {
        return None;
    }
    let t = ((c.0 - a.0) * s.1 - (c.1 - a.1) * s.0) / den;
    Some((a.0 + t * r.0, a.1 + t * r.1))
}

/// One connection of the view on the shown page.
#[derive(Clone, PartialEq, Debug)]
pub struct WireStroke {
    /// The connection.
    pub id: ew_api::output::Uuid,
    /// Its drawn path in sheet nanometres.
    pub polyline: Vec<(i64, i64)>,
    /// The sheet's size in nanometres: the dashes are walked over the path clipped to it.
    pub sheet: (i64, i64),
    /// Its effective colour's codes, concatenated, as the label shows them.
    pub code: String,
    /// The display colour of each code ([`colour_of`]); `None` for a code that resolves to no colour.
    pub colours: Vec<Option<(u8, u8, u8)>>,
    /// Whether it is cased: a colour under [`MIN_CONTRAST`] against white.
    pub cased: bool,
    /// The lane of each segment of the path ([`assign_lanes`]); 0 where none was given.
    pub lanes: Vec<u8>,
}

impl WireStroke {
    /// The stroke of the colour `codes` along `polyline` on a sheet of `sheet`.
    pub fn new(
        id: ew_api::output::Uuid,
        polyline: Vec<(i64, i64)>,
        sheet: (i64, i64),
        codes: &[String],
        company: &[CompanyColour],
    ) -> WireStroke {
        let colours: Vec<Option<(u8, u8, u8)>> = codes.iter().map(|c| colour_of(c, company)).collect();
        let cased = colours.iter().all(Option::is_some)
            && colours
                .iter()
                .flatten()
                .any(|c| contrast_on_white(*c) < MIN_CONTRAST);
        WireStroke {
            id,
            polyline,
            sheet,
            code: codes.concat(),
            colours,
            cased,
            lanes: Vec::new(),
        }
    }

    /// Whether a code resolves to no colour: the stroke is the unresolved pattern.
    pub fn unresolved(&self) -> bool {
        self.colours.is_empty() || self.colours.iter().any(Option::is_none)
    }

    /// The colour a piece of index `i` is painted in: the code's, or [`CASING_RGB`] for the unresolved pattern.
    pub fn piece_rgb(&self, i: usize) -> (u8, u8, u8) {
        if self.unresolved() {
            return CASING_RGB;
        }
        self.colours.get(i).copied().flatten().unwrap_or(CASING_RGB)
    }

    /// Whether a segment stands off its path: the strokes are drawn piece by piece.
    pub fn laned(&self) -> bool {
        self.lanes.iter().any(|l| *l > 0)
    }

    /// FT-1283: the stroke as it is drawn at `nm_per_pt` sheet nanometres per logical point: each segment moved off
    /// its path by its lane's offset ([`lane_offset_pt`] along [`normal`]), and the moved segments joined where their
    /// lines cross, so a laned stroke turns its corners in its lane and meets itself at every turn (a lane that ends
    /// at a corner turns onto the next segment's line). Two segments on one line in different lanes are joined by a
    /// step across. The result has no lanes: it is drawn on its path, a two-colour one's dashes running on over the
    /// joined corners. An unlaned stroke is itself.
    pub fn in_lanes(&self, nm_per_pt: f64) -> WireStroke {
        if !self.laned() || self.polyline.len() < 2 {
            return self.clone();
        }
        let pts: Vec<(f64, f64)> = self.polyline.iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
        let moved: Vec<((f64, f64), (f64, f64))> = pts
            .windows(2)
            .enumerate()
            .map(|(i, w)| {
                let n = normal(w[0], w[1]);
                let k = lane_offset_pt(self.lanes.get(i).copied().unwrap_or(0)) * nm_per_pt;
                let off = |p: (f64, f64)| (p.0 + n.0 * k, p.1 + n.1 * k);
                (off(w[0]), off(w[1]))
            })
            .collect();
        let mut path = vec![moved[0].0];
        let dist = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).hypot(p.1 - q.1);
        for (j, pair) in moved.windows(2).enumerate() {
            let ((a, b), (c, d)) = (pair[0], pair[1]);
            // A turn of nearly half a circle would put the join far out: there the two ends are joined straight.
            let v = pts[j + 1];
            let reach = 4.0 * dist(b, v).max(dist(c, v)) + 1.0;
            match crossing(a, b, c, d).filter(|x| dist(*x, v) <= reach) {
                Some(x) => path.push(x),
                None => {
                    path.push(b);
                    if c != b {
                        path.push(c);
                    }
                }
            }
        }
        path.push(moved[moved.len() - 1].1);
        WireStroke {
            polyline: path
                .iter()
                .map(|p| (p.0.round() as i64, p.1.round() as i64))
                .collect(),
            lanes: Vec::new(),
            ..self.clone()
        }
    }

    /// The segments of the path clipped to the sheet, in sheet nanometres, each with colour index 0 and its lane.
    pub fn segments(&self) -> Vec<Piece> {
        let (w, h) = (self.sheet.0 as f64, self.sheet.1 as f64);
        self.polyline
            .windows(2)
            .enumerate()
            .filter_map(|(i, s)| {
                let (a, b) = clip(
                    (s[0].0 as f64, s[0].1 as f64),
                    (s[1].0 as f64, s[1].1 as f64),
                    w,
                    h,
                )?;
                (a != b).then_some((a, b, 0, self.lanes.get(i).copied().unwrap_or(0)))
            })
            .collect()
    }

    /// The pieces to stroke, in sheet nanometres, each with the index of its colour ([`WireStroke::piece_rgb`]): the
    /// clipped segments for one colour; for two, the alternating [`DASH_NM`] dashes (the first colour first,
    /// continuing over the path's corners); for the unresolved pattern, [`UNRESOLVED_DASH_NM`] dashes without the
    /// gaps between them.
    pub fn pieces(&self) -> Vec<Piece> {
        let segments = self.segments();
        let (dash, keep_gaps) = if self.unresolved() {
            (UNRESOLVED_DASH_NM, false)
        } else if self.colours.len() >= 2 {
            (DASH_NM, true)
        } else {
            return segments;
        };
        let mut out = Vec::new();
        // The distance walked along the clipped path so far, and so the dash of each point.
        let mut walked = 0.0f64;
        for (a, b, _, lane) in segments {
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if !(len.is_finite() && len > 0.0) {
                continue;
            }
            let at = |d: f64| (a.0 + (b.0 - a.0) * d / len, a.1 + (b.1 - a.1) * d / len);
            let mut d = 0.0;
            while d < len {
                let n = (walked / dash).floor();
                let end = ((n + 1.0) * dash - walked).min(len - d);
                // Rounding never stalls the walk: a dash that ends where it starts takes the next one's length.
                let end = if end > 0.0 { end } else { dash.min(len - d) };
                let i = (n as usize) % 2;
                if keep_gaps || i == 0 {
                    out.push((at(d), at(d + end), i, lane));
                }
                d += end;
                walked += end;
            }
        }
        out
    }

    /// The midpoint of the drawn path by length, in sheet nanometres, with its segment's normal and lane (where the
    /// code stands); `None` for an empty path.
    pub fn midpoint(&self) -> Option<LabelAt> {
        let pts: Vec<(f64, f64)> = self.polyline.iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
        let first = *pts.first()?;
        let label = |i: usize, at: (f64, f64)| LabelAt {
            at,
            normal: pts.get(i + 1).map_or((0.0, 0.0), |b| normal(pts[i], *b)),
            lane: self.lanes.get(i).copied().unwrap_or(0),
        };
        let lens: Vec<f64> = pts
            .windows(2)
            .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
            .collect();
        let mut half = lens.iter().sum::<f64>() / 2.0;
        for (i, (w, len)) in pts.windows(2).zip(&lens).enumerate() {
            if half <= *len && *len > 0.0 {
                let t = half / len;
                return Some(label(
                    i,
                    (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t),
                ));
            }
            half -= len;
        }
        Some(label(0, first))
    }
}

/// The segment `a`–`b` clipped to the rectangle `0..=w` × `0..=h` (Liang–Barsky); `None` when it lies outside.
fn clip(a: (f64, f64), b: (f64, f64), w: f64, h: f64) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0), (dx, w - a.0), (-dy, a.1), (dy, h - a.1)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 <= t1).then_some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: (i64, i64) = (420_000_000, 297_000_000);

    fn company() -> Vec<CompanyColour> {
        let c = |code: &str, base: &str, shade: Option<&str>| CompanyColour {
            code: code.to_owned(),
            base: base.to_owned(),
            shade: shade.map(str::to_owned),
        };
        // `company.dbu-wh-24v`'s DBU (BU, dark) and a code whose spelling names no shade, NAVY (BU, dark).
        vec![
            c("DBU", "BU", Some("dark")),
            c("NAVY", "BU", Some("dark")),
            c("LGN", "GN", Some("light")),
            c("CU", "BN", None),
        ]
    }

    fn stroke(codes: &[&str], polyline: Vec<(i64, i64)>) -> WireStroke {
        let codes: Vec<String> = codes.iter().map(|c| (*c).to_owned()).collect();
        WireStroke::new(ew_api::output::Uuid::nil(), polyline, SHEET, &codes, &company())
    }

    #[test]
    fn company_codes_take_their_declared_base_and_shade_never_their_spelling() {
        let dbu = Some((0x11, 0x34, 0x76));
        assert_eq!(stroke(&["DBU"], vec![]).colours, [dbu]);
        assert_eq!(
            stroke(&["NAVY"], vec![]).colours,
            [dbu],
            "NAVY is declared BU, dark"
        );
        assert_eq!(stroke(&["LGN"], vec![]).colours, [Some((0x8C, 0xCC, 0x9D))]);
        assert_eq!(
            stroke(&["CU"], vec![]).colours,
            [iec_rgb("BN")],
            "no shade: the base"
        );
        // `company.dbu-wh-24v`'s two-colour wire: both colours, dashed.
        let s = stroke(&["DBU", "WH"], vec![(0, 0), (5_000_000, 0)]);
        assert_eq!(s.code, "DBUWH");
        assert_eq!(s.colours, [dbu, iec_rgb("WH")]);
        assert!(!s.unresolved());
        assert_eq!(s.pieces().iter().map(|p| p.2).collect::<Vec<_>>(), [0, 1, 0]);
        // An undeclared code (`LBU` is not in this profile) and a foreign one resolve to no colour: the pattern.
        for codes in [&["LBU"][..], &["XYZ"], &["BU", "BR"]] {
            let s = stroke(codes, vec![(0, 0), (5_000_000, 0)]);
            assert!(s.unresolved(), "{codes:?}");
            assert!(!s.cased);
            assert!(
                s.pieces()
                    .iter()
                    .all(|p| p.2 == 0 && s.piece_rgb(p.2) == CASING_RGB)
            );
            // 1 mm dashes with 1 mm gaps: three dashes on 5 mm.
            assert_eq!(s.pieces().len(), 3, "{codes:?}");
        }
    }

    #[test]
    fn every_colour_under_3_to_1_on_white_is_cased_and_no_other() {
        for code in IEC_CODES {
            let rgb = iec_rgb(code).unwrap();
            let s = stroke(&[code], vec![]);
            assert_eq!(s.cased, contrast_on_white(rgb) < MIN_CONTRAST, "{code}");
        }
        // The ratio itself: white 1:1, black 21:1, the examples of doc 16 §10.3 and the reviewer's (C49-p8#2).
        assert!((contrast_on_white((0xFF, 0xFF, 0xFF)) - 1.0).abs() < 1e-9);
        assert!((contrast_on_white((0, 0, 0)) - 21.0).abs() < 1e-9);
        for code in ["WH", "YE", "OG", "TQ", "GD", "PK", "SR"] {
            assert!(stroke(&[code], vec![]).cased, "{code}");
        }
        for code in ["BK", "BN", "RD", "GN", "BU", "VT", "GY"] {
            assert!(!stroke(&[code], vec![]).cased, "{code}");
        }
        assert!(!stroke(&["DBU"], vec![]).cased && stroke(&["LGN"], vec![]).cased);
        // A two-colour wire is cased when one of its colours is.
        assert!(stroke(&["GN", "YE"], vec![]).cased);
    }

    #[test]
    fn a_two_colour_wire_alternates_2_mm_dashes_over_its_corners() {
        let s = stroke(
            &["GN", "YE"],
            vec![(0, 0), (3_000_000, 0), (3_000_000, 2_000_000)],
        );
        assert_eq!(
            s.pieces(),
            [
                ((0.0, 0.0), (2_000_000.0, 0.0), 0, 0),
                ((2_000_000.0, 0.0), (3_000_000.0, 0.0), 1, 0),
                ((3_000_000.0, 0.0), (3_000_000.0, 1_000_000.0), 1, 0),
                ((3_000_000.0, 1_000_000.0), (3_000_000.0, 2_000_000.0), 0, 0),
            ]
        );
        let one = stroke(&["DBU"], vec![(0, 0), (0, 0), (5, 0)]);
        assert_eq!(one.pieces(), [((0.0, 0.0), (5.0, 0.0), 0, 0)]);
    }

    #[test]
    fn a_route_point_far_outside_the_sheet_is_clipped_before_dashing() {
        // C49-p8#5: a value the format allows; unclipped it gave 5e8 dashes.
        let far = 1_000_000_000_000_000_i64;
        let s = stroke(&["GN", "YE"], vec![(10_000_000, 10_000_000), (far, 10_000_000)]);
        let pieces = s.pieces();
        // The sheet is 420 mm wide: 205 dashes from x = 10 mm to its edge.
        assert_eq!(pieces.len(), 205);
        assert!(
            pieces
                .iter()
                .all(|(a, b, _, _)| a.0 <= SHEET.0 as f64 && b.0 <= SHEET.0 as f64)
        );
        let outside = stroke(&["GN", "YE"], vec![(-far, -far), (-far, far)]);
        assert_eq!(outside.pieces(), []);
    }

    /// W49.2 F9: a WH and a GNYE wire drawn on one path stand in two lanes, so both colours show, and each code sits
    /// on its own stroke's lane.
    #[test]
    fn two_wires_on_one_path_take_two_lanes_and_each_code_its_own() {
        let path = vec![(0, 10_000_000), (40_000_000, 10_000_000)];
        let mut strokes = vec![
            stroke(&["WH"], path.clone()),
            // Its path runs the other way and further, over the WH wire's stretch and beyond it, then turns.
            stroke(
                &["GN", "YE"],
                vec![
                    (60_000_000, 10_000_000),
                    (20_000_000, 10_000_000),
                    (20_000_000, 30_000_000),
                ],
            ),
            stroke(&["BU"], vec![(0, 50_000_000), (10_000_000, 50_000_000)]),
        ];
        assign_lanes(&mut strokes);
        assert_eq!(strokes[0].lanes, [0]);
        // FT-1344: split where the WH wire ends, only the shared stretch moves.
        assert_eq!(strokes[1].lanes, [0, 1, 0], "only the shared stretch moves");
        assert_eq!(strokes[2].lanes, [0], "alone on its path");
        assert!(!strokes[0].laned() && strokes[1].laned());
        // Every piece of the stripes on the shared stretch is in lane 1; both colours are there.
        let shared: Vec<_> = strokes[1]
            .pieces()
            .into_iter()
            .filter(|p| p.0.1 == p.1.1 && p.0.0.max(p.1.0) <= 40_000_000.0)
            .collect();
        assert!(shared.iter().all(|p| p.3 == 1));
        assert!(shared.iter().any(|p| p.2 == 0) && shared.iter().any(|p| p.2 == 1));
        // The codes: the WH one on its path, the GNYE one on lane 1 of the segment it lies on.
        let wh = strokes[0].midpoint().unwrap();
        assert_eq!((wh.at, wh.lane), ((20_000_000.0, 10_000_000.0), 0));
        let gnye = strokes[1].midpoint().unwrap();
        assert_eq!(
            (gnye.at, gnye.lane, gnye.normal),
            ((30_000_000.0, 10_000_000.0), 1, (0.0, 1.0))
        );
        assert_eq!(lane_offset_pt(0), 0.0);
        assert_eq!(lane_offset_pt(1), LANE_PT);
        assert_eq!(lane_offset_pt(2), -LANE_PT);
        assert_eq!(lane_offset_pt(3), 2.0 * LANE_PT);
    }

    /// C49-p8b#1: three wires on one stretch, the third drawn the other way, stand in three lanes on three sides of
    /// the path, and their three codes stand apart: no stroke and no code on another.
    #[test]
    fn three_wires_on_one_line_one_reversed_stand_apart() {
        let mut strokes = vec![
            stroke(&["BU"], vec![(0, 10_000_000), (40_000_000, 10_000_000)]),
            stroke(&["RD"], vec![(0, 10_000_000), (40_000_000, 10_000_000)]),
            stroke(&["BK"], vec![(40_000_000, 10_000_000), (0, 10_000_000)]),
        ];
        assign_lanes(&mut strokes);
        let lanes: Vec<Vec<u8>> = strokes.iter().map(|s| s.lanes.clone()).collect();
        assert_eq!(lanes, [[0], [1], [2]]);
        // How far each stroke's segment and each code stand off the path, in points along y.
        let side = |s: &WireStroke| {
            let (a, b, _, lane) = s.segments()[0];
            normal(a, b).1 * lane_offset_pt(lane)
        };
        let label = |s: &WireStroke| {
            let m = s.midpoint().unwrap();
            m.normal.1 * lane_offset_pt(m.lane)
        };
        let sides: Vec<f64> = strokes.iter().map(side).collect();
        assert_eq!(sides, [0.0, LANE_PT, -LANE_PT], "three strokes apart");
        let labels: Vec<f64> = strokes.iter().map(label).collect();
        assert_eq!(labels, sides, "each code on its own stroke");
    }

    /// FT-1283 (C49-p8b#5): a wire keeps one lane over a shared run that turns a corner, the lowest that none of the
    /// run's segments meets (not lane 1 on one segment and lane 2 on the next), and drawn in its lanes its corner is
    /// one point where the moved segments' lines cross.
    #[test]
    fn a_wire_keeps_its_lane_over_a_shared_run_and_its_corner_turns_with_it() {
        let ell = vec![
            (0, 10_000_000),
            (40_000_000, 10_000_000),
            (40_000_000, 30_000_000),
        ];
        let down = vec![(40_000_000, 10_000_000), (40_000_000, 30_000_000)];
        let mut strokes = vec![
            stroke(&["BU"], vec![(0, 10_000_000), (40_000_000, 10_000_000)]),
            stroke(&["RD"], down.clone()),
            stroke(&["BK"], down),
            stroke(&["GN"], ell.clone()),
        ];
        assign_lanes(&mut strokes);
        assert_eq!(
            strokes[2].lanes,
            [1],
            "the second wire down stands beside the first"
        );
        assert_eq!(
            strokes[3].lanes,
            [2, 2],
            "one lane over the run, clear of lanes 0 and 1"
        );
        // Drawn in its lanes at 1 000 nm a point: each segment moved its lane's offset off its line, the corner the
        // crossing of the moved lines (the horizontal line's normal is +y, the vertical one's −x).
        let k = (lane_offset_pt(2) * 1_000.0) as i64;
        assert_eq!(k, -(LANE_PT * 1_000.0) as i64);
        let drawn = strokes[3].in_lanes(1_000.0);
        assert_eq!(
            drawn.polyline,
            [
                (0, 10_000_000 + k),
                (40_000_000 - k, 10_000_000 + k),
                (40_000_000 - k, 30_000_000)
            ]
        );
        assert!(!drawn.laned(), "drawn on its moved path");
        // A two-segment run on one wire that a single earlier wire shares keeps lane 1 over the corner.
        let mut two = vec![stroke(&["BU"], ell.clone()), stroke(&["RD"], ell)];
        assign_lanes(&mut two);
        assert_eq!(two[1].lanes, [1, 1]);
        // An unlaned stroke is drawn as it is.
        assert_eq!(two[0].in_lanes(1_000.0), two[0]);
    }

    /// FT-1344 (ID-1358 ruling 4): a segment that shares only part of its stretch moves only that part into its lane;
    /// the rest stays on its path, joined to the shared part by a step across.
    #[test]
    fn only_the_shared_part_of_a_segment_takes_its_lane() {
        let mut strokes = vec![
            stroke(&["BU"], vec![(40_000_000, 0), (60_000_000, 0)]),
            stroke(&["RD"], vec![(100_000_000, 0), (0, 0)]),
        ];
        assign_lanes(&mut strokes);
        assert_eq!(strokes[0].lanes, [0]);
        assert_eq!(
            strokes[1].polyline,
            [
                (100_000_000, 0),
                (60_000_000, 0),
                (40_000_000, 0),
                (0, 0)
            ],
            "split where the shared part begins and ends"
        );
        assert_eq!(strokes[1].lanes, [0, 1, 0]);
        let k = (lane_offset_pt(1) * 1_000.0) as i64;
        assert_eq!(
            strokes[1].in_lanes(1_000.0).polyline,
            [
                (100_000_000, 0),
                (60_000_000, 0),
                (60_000_000, k),
                (40_000_000, k),
                (40_000_000, 0),
                (0, 0)
            ]
        );
    }

    /// C49-p8b#4: the lanes take ends anywhere in `i64`, without an overflow.
    #[test]
    fn lanes_at_the_ends_of_i64() {
        let (lo, hi) = (i64::MIN, i64::MAX);
        let mut strokes = vec![
            stroke(&["BU"], vec![(lo, 0), (hi, 0)]),
            stroke(&["RD"], vec![(hi, 0), (lo, 0)]),
            stroke(&["BK"], vec![(lo, lo), (hi, hi)]),
            stroke(&["GN"], vec![(lo, hi), (hi, lo)]),
            stroke(&["YE"], vec![(hi, hi), (lo, lo)]),
        ];
        assign_lanes(&mut strokes);
        let lanes: Vec<Vec<u8>> = strokes.iter().map(|s| s.lanes.clone()).collect();
        assert_eq!(
            lanes,
            [[0], [1], [0], [0], [1]],
            "the shared lines share, the crossing does not"
        );
    }

    #[test]
    fn the_code_stands_at_the_middle_of_the_path() {
        let s = stroke(&["BU"], vec![(0, 0), (4, 0), (4, 4)]);
        assert_eq!(s.midpoint().map(|l| l.at), Some((4.0, 0.0)));
        assert_eq!(
            stroke(&["BU"], vec![(1, 1)]).midpoint().map(|l| l.at),
            Some((1.0, 1.0))
        );
        assert_eq!(stroke(&["BU"], vec![]).midpoint(), None);
    }
}
