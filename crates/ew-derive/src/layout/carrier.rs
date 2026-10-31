// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Carrier paths (WP-S4.01 D5): the axis-parallel check, the length and a rail's axis.

use std::fmt;

use ew_schema::CarrierKind;
use ew_units::Pt2;

use super::pose::{Quarter, mm};

/// Why a carrier path is refused (D5: a rail is two points on one horizontal or vertical line, a duct an orthogonal
/// polyline; a segment at another angle is refused).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathError {
    /// A rail path that is not exactly two points.
    RailPoints(usize),
    /// A duct path of fewer than two points.
    DuctPoints(usize),
    /// Segment `i` (from point `i` to point `i + 1`) has no length.
    ZeroLength(usize),
    /// Segment `i` is neither horizontal nor vertical.
    Diagonal(usize),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::RailPoints(n) => write!(f, "a rail is two points, not {n}"),
            PathError::DuctPoints(n) => write!(f, "a duct is at least two points, not {n}"),
            PathError::ZeroLength(i) => write!(f, "segment {i} has no length"),
            PathError::Diagonal(i) => write!(
                f,
                "segment {i} is neither horizontal nor vertical; carriers are axis-parallel"
            ),
        }
    }
}

/// `b − a` as exact i128 components.
fn delta(a: Pt2, b: Pt2) -> (i128, i128) {
    (
        i128::from(b.x.0) - i128::from(a.x.0),
        i128::from(b.y.0) - i128::from(a.y.0),
    )
}

/// Checks a carrier path against D5. A carrier kind of a newer format is not checked (`Ok`): this build cannot
/// know its geometry, and the commands refuse to create one.
///
/// # Errors
///
/// The first [`PathError`] in point order.
pub fn check_path(kind: &CarrierKind, path: &[Pt2]) -> Result<(), PathError> {
    match kind {
        CarrierKind::Rail if path.len() != 2 => return Err(PathError::RailPoints(path.len())),
        CarrierKind::Duct if path.len() < 2 => return Err(PathError::DuctPoints(path.len())),
        CarrierKind::Rail | CarrierKind::Duct => {}
        CarrierKind::Unknown(_) => return Ok(()),
    }
    for (i, w) in path.windows(2).enumerate() {
        match delta(w[0], w[1]) {
            (0, 0) => return Err(PathError::ZeroLength(i)),
            (0, _) | (_, 0) => {}
            _ => return Err(PathError::Diagonal(i)),
        }
    }
    Ok(())
}

/// The length of a path in nanometres: the sum of its segments' lengths, exact for an axis-parallel path (each
/// segment's `|du| + |dv|`; a diagonal segment would count its Manhattan length, so check the path first).
pub fn path_length(path: &[Pt2]) -> i128 {
    path.windows(2)
        .map(|w| {
            let (du, dv) = delta(w[0], w[1]);
            du.abs() + dv.abs()
        })
        .sum()
}

/// The axis of an axis-parallel rail: where it starts, which way it runs and how long it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RailAxis {
    /// `path[0]`, in the surface frame.
    pub start: Pt2,
    /// The direction from `path[0]` to `path[1]` as quarter turns from `+u` (1 = `+v`, 2 = `−u`, 3 = `−v`).
    pub dir: Quarter,
    /// The length in nanometres, > 0.
    pub length: i128,
}

impl RailAxis {
    /// The axis of a rail path, or `None` when the path is not a valid rail (see [`check_path`]).
    pub fn of(path: &[Pt2]) -> Option<RailAxis> {
        let [a, b] = path else {
            return None;
        };
        let (du, dv) = delta(*a, *b);
        let (dir, length) = match (du.signum(), dv.signum()) {
            (1, 0) => (Quarter::new(0), du),
            (0, 1) => (Quarter::new(1), dv),
            (-1, 0) => (Quarter::new(2), -du),
            (0, -1) => (Quarter::new(3), -dv),
            _ => return None,
        };
        Some(RailAxis {
            start: *a,
            dir,
            length,
        })
    }

    /// The point at distance `s` from the start along the axis, in the surface frame (i128, unbounded).
    pub fn point_at(&self, s: i128) -> (i128, i128) {
        let (du, dv) = self.dir.rotate((s, 0));
        (i128::from(self.start.x.0) + du, i128::from(self.start.y.0) + dv)
    }

    /// The length as text in millimetres, for refusals.
    pub fn length_mm(&self) -> String {
        mm(self.length)
    }
}
