// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Potential and net highlighting (WP-S2.06 §7.3, D14; FR-UI-018, FT-362): view state of the window, never a model
//! edit, never an undo entry, never saved.
//!
//! A window holds up to [`MAX_HIGHLIGHTS`] highlights, each a potential or the net of a connection
//! ([`HighlightTarget`]) in a **slot** 0 to 3: the first free slot, and a fifth highlight takes the oldest one's slot.
//! Slot n has the Okabe–Ito colour [`SLOT_COLOURS`]`[n]` and the hatch angle [`SLOT_ANGLES`]`[n]`, so the pattern
//! carries the meaning as well as the colour (FR-UI-025). The members (the connection IDs, `Session::highlight_members`)
//! are re-read once per revision.
//!
//! **Drawing.** Every drawn segment of a member on the shown page is a band [`BAND_PT`] points wide (square caps of
//! half the width at both ends) filled with the slot colour at [`BAND_ALPHA`], crossed by hatch lines
//! [`HATCH_WIDTH_PT`] wide in the full colour, [`HATCH_PITCH_PT`] apart at the slot's angle. The hatch lines of a slot
//! are one family of parallel screen lines `p · n = k × pitch` (`n` the normal of the angle), so the bands of one
//! highlight join without seams. [`pieces`] gives the bands and their hatch segments in screen pixels for the window's
//! painter; [`paint_at`] decides one pixel for the headless snapshot by the same geometry.

use std::sync::Arc;

pub use ew_api::output::HighlightTarget;
use ew_api::output::Uuid;

/// The most highlights a window shows.
pub const MAX_HIGHLIGHTS: usize = 4;

/// The slot colours (Okabe–Ito): orange, sky blue, bluish green, vermillion.
pub const SLOT_COLOURS: [(u8, u8, u8); MAX_HIGHLIGHTS] = [
    (0xE6, 0x9F, 0x00),
    (0x56, 0xB4, 0xE9),
    (0x00, 0x9E, 0x73),
    (0xD5, 0x5E, 0x00),
];

/// The slots' hatch angles in degrees, counter-clockwise from the screen's x axis.
pub const SLOT_ANGLES: [u16; MAX_HIGHLIGHTS] = [45, 135, 0, 90];

/// The band's width in logical points.
pub const BAND_PT: f64 = 8.0;

/// A hatch line's width in logical points.
pub const HATCH_WIDTH_PT: f64 = 2.0;

/// The distance of the hatch lines in logical points.
pub const HATCH_PITCH_PT: f64 = 6.0;

/// The band's opacity: 30 % (77 of 255).
pub const BAND_ALPHA: u8 = 77;

/// One highlight of the window.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Highlight {
    /// What it shows.
    pub target: HighlightTarget,
    /// Its slot, 0 to 3.
    pub slot: u8,
    /// The potential's name or the connection's label (the row's label), for the describe text.
    pub name: String,
    /// The members (connection IDs, sorted) at revision `rev`.
    pub members: Arc<Vec<Uuid>>,
    /// The revision the members were read at (`None`: not yet read).
    pub rev: Option<u64>,
}

impl Highlight {
    /// The describe word of the target: `potential` or `connection`.
    pub fn word(&self) -> &'static str {
        match self.target {
            HighlightTarget::Potential(_) => "potential",
            HighlightTarget::Net(_) => "connection",
        }
    }

    /// The slot's colour.
    pub fn colour(&self) -> (u8, u8, u8) {
        SLOT_COLOURS[usize::from(self.slot) % MAX_HIGHLIGHTS]
    }
}

/// The highlights of a window, oldest first.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Highlights {
    items: Vec<Highlight>,
}

impl Highlights {
    /// The highlights, oldest first.
    pub fn items(&self) -> &[Highlight] {
        &self.items
    }

    /// Whether there is none.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The slot of a target, when it is highlighted.
    pub fn slot_of(&self, target: &HighlightTarget) -> Option<u8> {
        self.items.iter().find(|h| h.target == *target).map(|h| h.slot)
    }

    /// Adds a highlight of `target` in the first free slot, or (all four taken) in the oldest one's slot, which it
    /// replaces; returns the slot. A target already shown keeps its slot.
    pub fn add(&mut self, target: HighlightTarget, name: &str) -> u8 {
        if let Some(s) = self.slot_of(&target) {
            return s;
        }
        let slot = (0..MAX_HIGHLIGHTS as u8)
            .find(|s| !self.items.iter().any(|h| h.slot == *s))
            .unwrap_or_else(|| self.items.remove(0).slot);
        self.items.push(Highlight {
            target,
            slot,
            name: name.to_owned(),
            members: Arc::new(Vec::new()),
            rev: None,
        });
        slot
    }

    /// Removes the highlight of `target`; whether there was one.
    pub fn remove(&mut self, target: &HighlightTarget) -> bool {
        let before = self.items.len();
        self.items.retain(|h| h.target != *target);
        self.items.len() != before
    }

    /// Removes every highlight; whether there was one.
    pub fn clear(&mut self) -> bool {
        let any = !self.items.is_empty();
        self.items.clear();
        any
    }

    /// The highlights whose members are not of revision `rev`.
    pub(crate) fn stale_mut(&mut self, rev: Option<u64>) -> impl Iterator<Item = &mut Highlight> + '_ {
        self.items.iter_mut().filter(move |h| h.rev != rev)
    }
}

/// One band of a highlight on screen with its hatch segments, in physical pixels of the pane.
#[derive(Clone, PartialEq, Debug)]
pub struct HatchPiece {
    /// The band's corners in order (a convex quadrilateral).
    pub band: [(f64, f64); 4],
    /// The hatch segments clipped to the band.
    pub hatches: Vec<((f64, f64), (f64, f64))>,
}

/// The unit direction of a slot's hatch lines on screen (y down): `(cos θ, −sin θ)`.
pub fn hatch_direction(slot: u8) -> (f64, f64) {
    let a = f64::from(SLOT_ANGLES[usize::from(slot) % MAX_HIGHLIGHTS]).to_radians();
    (round9(a.cos()), round9(-a.sin()))
}

/// A value rounded to 9 decimals (`cos 90°` is exactly 0, so the pixel decisions do not depend on the last bit).
fn round9(v: f64) -> f64 {
    (v * 1e9).round() / 1e9
}

/// The normal of a slot's hatch lines: the direction turned by 90°.
fn hatch_normal(slot: u8) -> (f64, f64) {
    let d = hatch_direction(slot);
    (-d.1, d.0)
}

fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

/// The bands of a polyline given in screen pixels, with the hatch segments of slot `slot`, at `ppp` physical pixels per
/// logical point: one piece per segment of non-zero length.
pub fn pieces(points: &[(f64, f64)], slot: u8, ppp: f64) -> Vec<HatchPiece> {
    let half = BAND_PT * ppp / 2.0;
    let pitch = HATCH_PITCH_PT * ppp;
    let d = hatch_direction(slot);
    let n = hatch_normal(slot);
    let mut out = Vec::new();
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        if !(len.is_finite() && len > 0.0) {
            continue;
        }
        let u = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let v = (-u.1, u.0);
        let p = |base: (f64, f64), su: f64, sv: f64| {
            (
                base.0 + u.0 * su * half + v.0 * sv * half,
                base.1 + u.1 * su * half + v.1 * sv * half,
            )
        };
        let band = [p(a, -1.0, 1.0), p(b, 1.0, 1.0), p(b, 1.0, -1.0), p(a, -1.0, -1.0)];
        // The hatch lines p · n = k × pitch that cross the band, clipped to it (Cyrus–Beck along d).
        let along: Vec<f64> = band.iter().map(|c| dot(*c, n)).collect();
        let lo = along.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = along.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mut hatches = Vec::new();
        let (k0, k1) = ((lo / pitch).ceil() as i64, (hi / pitch).floor() as i64);
        for k in k0..=k1 {
            let c = k as f64 * pitch;
            let origin = (n.0 * c, n.1 * c);
            if let Some((t0, t1)) = clip_line(origin, d, &band) {
                hatches.push((
                    (origin.0 + d.0 * t0, origin.1 + d.1 * t0),
                    (origin.0 + d.0 * t1, origin.1 + d.1 * t1),
                ));
            }
        }
        out.push(HatchPiece { band, hatches });
    }
    out
}

/// The parameter range of the line `origin + t × d` inside the convex quadrilateral `q` (corners in order).
fn clip_line(origin: (f64, f64), d: (f64, f64), q: &[(f64, f64); 4]) -> Option<(f64, f64)> {
    // The quadrilateral's orientation: inside is on the same side of every edge as the centroid.
    let c = (
        q.iter().map(|p| p.0).sum::<f64>() / 4.0,
        q.iter().map(|p| p.1).sum::<f64>() / 4.0,
    );
    let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
    for i in 0..4 {
        let (p, r) = (q[i], q[(i + 1) % 4]);
        let e = (r.0 - p.0, r.1 - p.1);
        let mut normal = (-e.1, e.0);
        if dot(normal, (c.0 - p.0, c.1 - p.1)) < 0.0 {
            normal = (-normal.0, -normal.1);
        }
        // Inside: normal · (x − p) ≥ 0.
        let num = dot(normal, (origin.0 - p.0, origin.1 - p.1));
        let den = dot(normal, d);
        if den == 0.0 {
            if num < 0.0 {
                return None;
            }
        } else {
            let t = -num / den;
            if den > 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 < t1 && t0.is_finite() && t1.is_finite()).then_some((t0, t1))
}

/// Whether the point `p` lies in the convex quadrilateral `q`.
fn inside(q: &[(f64, f64); 4], p: (f64, f64)) -> bool {
    let mut sign = 0.0f64;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let cross = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
        if cross != 0.0 {
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return false;
            }
        }
    }
    true
}

/// How one pixel of a highlight is painted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paint {
    /// The band: the slot colour at [`BAND_ALPHA`].
    Band,
    /// A hatch line: the slot colour.
    Hatch,
}

/// How the pixel whose centre is `p` (screen pixels) is painted by a highlight of slot `slot` with these bands: a hatch
/// line where its centre lies within half a hatch width of one of the slot's lines, else the band, inside any band;
/// `None` outside every band.
pub fn paint_at(bands: &[HatchPiece], slot: u8, ppp: f64, p: (f64, f64)) -> Option<Paint> {
    if !bands.iter().any(|b| inside(&b.band, p)) {
        return None;
    }
    let pitch = HATCH_PITCH_PT * ppp;
    let s = dot(p, hatch_normal(slot)) / pitch;
    let off = (s - s.round()).abs() * pitch;
    Some(if off < HATCH_WIDTH_PT * ppp / 2.0 {
        Paint::Hatch
    } else {
        Paint::Band
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(n: u8) -> HighlightTarget {
        HighlightTarget::Net(Uuid::from_bytes([n; 16]))
    }

    #[test]
    fn slots_fill_in_order_and_a_fifth_replaces_the_oldest() {
        let mut h = Highlights::default();
        assert_eq!(h.add(target(1), "a"), 0);
        assert_eq!(h.add(target(2), "b"), 1);
        assert_eq!(h.add(target(2), "b"), 1, "a shown target keeps its slot");
        assert_eq!(h.add(target(3), "c"), 2);
        assert_eq!(h.add(target(4), "d"), 3);
        assert_eq!(h.add(target(5), "e"), 0, "the oldest (slot 0) is replaced");
        assert_eq!(h.slot_of(&target(1)), None);
        assert!(h.remove(&target(3)));
        assert_eq!(h.add(target(6), "f"), 2, "the first free slot");
        assert!(h.clear());
        assert!(!h.clear());
    }

    #[test]
    fn directions_and_hatch_segments() {
        assert_eq!(
            hatch_direction(0),
            (round9(0.5f64.sqrt()), round9(-(0.5f64.sqrt())))
        );
        assert_eq!(hatch_direction(2), (1.0, 0.0));
        assert_eq!(hatch_direction(3), (0.0, -1.0));
        // A horizontal segment 60 px long: the band is 8 px high and 68 px long; the 45° hatches cross it.
        let p = pieces(&[(10.0, 50.0), (70.0, 50.0)], 0, 1.0);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].band[0], (6.0, 54.0));
        assert_eq!(p[0].band[2], (74.0, 46.0));
        // The band spans (68 + 8) / √2 ≈ 53.7 px across the 45° lines, 6 px apart: 8 or 9 of them.
        assert!((8..=9).contains(&p[0].hatches.len()), "{}", p[0].hatches.len());
        for (a, b) in &p[0].hatches {
            // Each hatch runs at 45°: Δx = −Δy (y down), inside the band's rows.
            assert!(((b.0 - a.0) + (b.1 - a.1)).abs() < 1e-6, "{a:?} {b:?}");
            assert!(a.1 <= 54.0 + 1e-6 && a.1 >= 46.0 - 1e-6);
        }
        // A pixel on a hatch line is a hatch, between two lines the band, outside nothing.
        let (a, b) = p[0].hatches[3];
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        assert_eq!(paint_at(&p, 0, 1.0, mid), Some(Paint::Hatch));
        let n = hatch_normal(0);
        let between = (mid.0 + n.0 * 3.0, mid.1 + n.1 * 3.0);
        assert_eq!(paint_at(&p, 0, 1.0, between), Some(Paint::Band));
        assert_eq!(paint_at(&p, 0, 1.0, (40.0, 60.0)), None);
        // A zero-length segment draws nothing.
        assert!(pieces(&[(1.0, 1.0), (1.0, 1.0)], 0, 1.0).is_empty());
    }
}
