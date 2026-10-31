// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Poses on carriers (doc 12 §2.3, WP-S4.01 D5): quarter turns about `w`, the degrees of freedom per carrier kind,
//! the surface pose equivalent to a rail pose and the frame of a plate centred in its enclosure.

use std::fmt;

use ew_schema::MechData;
use ew_units::{Nm, Pose3, Pt2, Quat9};

use super::carrier::RailAxis;

/// `cos 45° = sin 45°` in 10⁻⁹, rounded: the `w` and `z` of a quarter turn about `w`.
const HALF_SQRT2: i64 = 707_106_781;

/// A rotation by `k · 90°` about the surface normal `w` (counter-clockwise seen from the front), `k ∈ 0..4`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Quarter(u8);

impl Quarter {
    /// No rotation.
    pub const NONE: Quarter = Quarter(0);
    /// A half turn.
    pub const HALF: Quarter = Quarter(2);

    /// `k · 90°`, `k` taken modulo 4.
    pub const fn new(k: u8) -> Quarter {
        Quarter(k % 4)
    }

    /// `k`, in `0..4`.
    pub const fn k(self) -> u8 {
        self.0
    }

    /// `self` followed by `other`.
    #[must_use]
    pub const fn then(self, other: Quarter) -> Quarter {
        Quarter::new(self.0 + other.0)
    }

    /// The sign-normalised unit quaternion of the rotation: `[1, 0, 0, 0]`, `[½√2, 0, 0, ½√2]`, `[0, 0, 0, 1]`,
    /// `[½√2, 0, 0, −½√2]` (in 10⁻⁹).
    pub fn quat(self) -> Quat9 {
        let [w, z] = match self.0 {
            0 => [Quat9::SCALE, 0],
            1 => [HALF_SQRT2, HALF_SQRT2],
            2 => [0, Quat9::SCALE],
            _ => [HALF_SQRT2, -HALF_SQRT2],
        };
        Quat9::new(w, 0, 0, z).expect("a quarter turn is a unit quaternion")
    }

    /// The quarter turn a quaternion stores, or `None` when it is another rotation. `x` and `y` must be 0, and `w`
    /// and `z` may differ from [`Quarter::quat`]'s by one unit in the last place (an importer's own rounding of
    /// `½√2`).
    pub fn of(q: Quat9) -> Option<Quarter> {
        let [w, x, y, z] = q.components();
        if x != 0 || y != 0 {
            return None;
        }
        (0..4).map(Quarter).find(|k| {
            let [cw, _, _, cz] = k.quat().components();
            (w - cw).abs() <= 1 && (z - cz).abs() <= 1
        })
    }

    /// `v` rotated by this quarter turn (exact).
    pub fn rotate(self, (u, v): (i128, i128)) -> (i128, i128) {
        match self.0 {
            0 => (u, v),
            1 => (-v, u),
            2 => (-u, -v),
            _ => (v, -u),
        }
    }
}

/// A length in nanometres as millimetres for messages: exact, without trailing fraction zeros (`96`, `7.5`,
/// `-0.000001`).
pub fn mm(nm: i128) -> String {
    let sign = if nm < 0 { "-" } else { "" };
    let a = nm.unsigned_abs();
    let (int, frac) = (a / 1_000_000, a % 1_000_000);
    if frac == 0 {
        format!("{sign}{int}")
    } else {
        let digits = format!("{frac:06}");
        format!("{sign}{int}.{}", digits.trim_end_matches('0'))
    }
}

/// What a pose is relative to, for [`check_pose`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoseOn {
    /// A rail with this axis: `t = (s, 0, 0)`, `s ∈ [0, L]`, a turn of 0° or 180°.
    Rail(RailAxis),
    /// A duct: it carries no placements (no degree of freedom).
    Duct,
    /// A mounting surface itself (a plate): `t = (u, v, 0)`, a turn of `k · 90°`.
    Surface,
}

/// Why a pose does not fit its carrier (doc 12 §2.3's degrees of freedom; EW-LAY-010 "pose off the carrier").
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PoseError {
    /// A rail pose with `t.y` or `t.z` other than 0.
    OffRailAxis,
    /// A rail pose turned by something other than 0° or 180° about `w`.
    RailTurn,
    /// A rail pose with `s` outside `[0, L]`: `(s, L)` in nanometres.
    OffRail(i128, i128),
    /// A surface pose with `t.z` other than 0.
    OffSurface,
    /// A surface pose turned by something other than `k · 90°` about `w`.
    SurfaceTurn,
    /// A pose on a duct.
    OnDuct,
}

impl fmt::Display for PoseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoseError::OffRailAxis => f.write_str("a pose on a rail lies on its axis: t = (s, 0, 0)"),
            PoseError::RailTurn => {
                f.write_str("a pose on a rail turns by 0° or 180° about the surface normal only")
            }
            PoseError::OffRail(s, l) => write!(
                f,
                "the pose is off the rail: s = {} mm is outside [0, {} mm]",
                mm(*s),
                mm(*l)
            ),
            PoseError::OffSurface => f.write_str("a pose on a surface lies in its plane: t = (u, v, 0)"),
            PoseError::SurfaceTurn => {
                f.write_str("a pose on a surface turns in steps of 90° about the surface normal only")
            }
            PoseError::OnDuct => f.write_str("a duct carries no placements"),
        }
    }
}

/// Checks a placement's pose against its carrier kind's degrees of freedom (doc 12 §2.3, D5). A pose on a surface is
/// not checked against the usable polygon: whether a footprint lies inside it is EW-LAY-010's (it needs the
/// footprint).
///
/// # Errors
///
/// The first [`PoseError`]: the axis or plane, then the turn, then the rail interval.
pub fn check_pose(on: PoseOn, pose: &Pose3) -> Result<(), PoseError> {
    let [x, y, z] = pose.t;
    match on {
        PoseOn::Rail(axis) => {
            if y != Nm::ZERO || z != Nm::ZERO {
                return Err(PoseError::OffRailAxis);
            }
            if !matches!(Quarter::of(pose.q), Some(Quarter::NONE | Quarter::HALF)) {
                return Err(PoseError::RailTurn);
            }
            let s = i128::from(x.0);
            if s < 0 || s > axis.length {
                return Err(PoseError::OffRail(s, axis.length));
            }
            Ok(())
        }
        PoseOn::Surface => {
            if z != Nm::ZERO {
                return Err(PoseError::OffSurface);
            }
            if Quarter::of(pose.q).is_none() {
                return Err(PoseError::SurfaceTurn);
            }
            Ok(())
        }
        PoseOn::Duct => Err(PoseError::OnDuct),
    }
}

/// The clip point of a part in the part frame (D5): its `rail_offset`, else the centre of its `W × H` footprint, else
/// `(0, 0)` (an unknown footprint is a square about the pose).
pub fn clip_offset(mech: Option<&MechData>) -> (i128, i128) {
    let Some(mech) = mech else {
        return (0, 0);
    };
    if let Some(off) = mech.rail_offset {
        return (i128::from(off.x.0), i128::from(off.y.0));
    }
    match (mech.width, mech.height) {
        (Some(w), Some(h)) => (i128::from(w.0) / 2, i128::from(h.0) / 2),
        _ => (0, 0),
    }
}

/// The surface pose equivalent to a pose on a rail (doc 12 §3.6, `rehome_to_surface`): the part keeps its place and
/// turn on the surface. `clip` is the clip point in the part frame ([`clip_offset`]; a strip's is its left end at
/// mid-height). The part origin is the clip point on the surface minus `clip` turned by the part's turn (the rail's
/// direction followed by the pose's 0° or 180°). `None` when the pose is not a valid rail pose or a coordinate leaves
/// the `Nm` range.
pub fn rehome_pose(axis: &RailAxis, pose: &Pose3, clip: (i128, i128)) -> Option<Pose3> {
    check_pose(PoseOn::Rail(*axis), pose).ok()?;
    let turn = axis.dir.then(Quarter::of(pose.q)?);
    let (pu, pv) = axis.point_at(i128::from(pose.t[0].0));
    let (cu, cv) = turn.rotate(clip);
    let u = i64::try_from(pu - cu).ok()?;
    let v = i64::try_from(pv - cv).ok()?;
    Some(Pose3 {
        t: [Nm(u), Nm(v), Nm::ZERO],
        q: turn.quat(),
    })
}

/// The frame and usable polygon of a plate surface centred in its enclosure (D5): `t = ((W − w) / 2, (H − h) / 2, 0)`
/// (rounded down to the nanometre), no turn, and the usable rectangle `(0, 0), (w, 0), (w, h), (0, h)` in the surface
/// frame. An unknown enclosure size gives the frame `t = (0, 0, 0)`; an unknown plate size an empty usable polygon
/// (and the frame at the origin).
pub fn plate_frame(
    enclosure: (Option<Nm>, Option<Nm>),
    plate: (Option<Nm>, Option<Nm>),
) -> (Pose3, Vec<Pt2>) {
    let (Some(pw), Some(ph)) = plate else {
        return (Pose3::IDENTITY, Vec::new());
    };
    let usable = vec![
        Pt2::new(Nm::ZERO, Nm::ZERO),
        Pt2::new(pw, Nm::ZERO),
        Pt2::new(pw, ph),
        Pt2::new(Nm::ZERO, ph),
    ];
    let t = match enclosure {
        (Some(ew), Some(eh)) => {
            let half = |outer: Nm, inner: Nm| {
                let d = (i128::from(outer.0) - i128::from(inner.0)).div_euclid(2);
                i64::try_from(d).map(Nm).unwrap_or(Nm::ZERO)
            };
            [half(ew, pw), half(eh, ph), Nm::ZERO]
        }
        _ => [Nm::ZERO; 3],
    };
    (
        Pose3 {
            t,
            q: Quat9::IDENTITY,
        },
        usable,
    )
}
