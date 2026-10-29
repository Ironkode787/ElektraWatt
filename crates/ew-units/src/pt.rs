// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Pt2` and `Rect`.

use std::ops::{Add, Neg, Sub};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Nm;

/// A 2D point or offset in nanometres.
///
/// On pages the origin is the **top-left corner of the sheet, x to the right and y downwards**
/// (domain model §3.1; QET's convention, so imported geometry needs no flip). The same type serves as an
/// offset between points.
///
/// `Ord` is lexicographic (x, then y), for sorted containers; it carries no geometric meaning. The
/// operators follow Rust's integer overflow semantics; the `checked_*` methods return `None` instead.
///
/// **JSON:** `[x, y]`, each an [`Nm`] integer.
///
/// ```
/// use ew_units::{Nm, Pt2};
///
/// let p = Pt2::new(Nm(42_333_333), Nm(15_875_000));
/// assert_eq!(serde_json::to_string(&p).unwrap(), "[42333333,15875000]");
/// assert_eq!(p + Pt2::new(Nm(1), Nm(-1)), Pt2::new(Nm(42_333_334), Nm(15_874_999)));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Pt2 {
    /// Horizontal coordinate, growing to the right.
    pub x: Nm,
    /// Vertical coordinate, growing downwards.
    pub y: Nm,
}

/// A 2D offset (a vector, not a position) in nanometres, such as a part's rail offset (WP-S1.19 §6.1, FT-604): the
/// representation of [`Pt2`], which serves as an offset between points, under the name the specification gives a
/// vector. **JSON:** `[dx, dy]`, each an [`Nm`] integer, exactly as a [`Pt2`].
pub type Vec2Nm = Pt2;

impl Pt2 {
    /// The origin `(0, 0)`: the top-left corner of the sheet.
    pub const ORIGIN: Pt2 = Pt2 {
        x: Nm::ZERO,
        y: Nm::ZERO,
    };

    /// The point `(x, y)`.
    pub const fn new(x: Nm, y: Nm) -> Pt2 {
        Pt2 { x, y }
    }

    /// `self + rhs` component-wise, or `None` on overflow.
    pub const fn checked_add(self, rhs: Pt2) -> Option<Pt2> {
        match (self.x.checked_add(rhs.x), self.y.checked_add(rhs.y)) {
            (Some(x), Some(y)) => Some(Pt2 { x, y }),
            _ => None,
        }
    }

    /// `self - rhs` component-wise, or `None` on overflow.
    pub const fn checked_sub(self, rhs: Pt2) -> Option<Pt2> {
        match (self.x.checked_sub(rhs.x), self.y.checked_sub(rhs.y)) {
            (Some(x), Some(y)) => Some(Pt2 { x, y }),
            _ => None,
        }
    }
}

impl Add for Pt2 {
    type Output = Pt2;
    fn add(self, rhs: Pt2) -> Pt2 {
        Pt2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Pt2 {
    type Output = Pt2;
    fn sub(self, rhs: Pt2) -> Pt2 {
        Pt2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Neg for Pt2 {
    type Output = Pt2;
    fn neg(self) -> Pt2 {
        Pt2 {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Serialize for Pt2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.x, self.y).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Pt2 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (x, y) = <(Nm, Nm)>::deserialize(deserializer)?;
        Ok(Pt2 { x, y })
    }
}

/// An axis-aligned, closed rectangle `[min.x, max.x] × [min.y, max.y]`, always normalised
/// (`min ≤ max` on both axes). A rectangle may be degenerate (zero width or height).
///
/// **Not persisted by this crate:** `ew-units` keeps `Rect` without serde. The persisted form is doc 05
/// §3.2's `[[x0,y0],[x1,y1]]` (two [`Pt2`], minimum corner first), implemented in `ew-schema`.
///
/// ```
/// use ew_units::{Nm, Pt2, Rect};
///
/// let r = Rect::from_points(Pt2::new(Nm(10), Nm(0)), Pt2::new(Nm(0), Nm(5)));
/// assert_eq!(r.min(), Pt2::new(Nm(0), Nm(0)));
/// assert_eq!((r.width(), r.height()), (Nm(10), Nm(5)));
/// assert!(r.contains(Pt2::new(Nm(10), Nm(5))));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rect {
    min: Pt2,
    max: Pt2,
}

impl Rect {
    /// The smallest rectangle containing both points, in any order.
    pub const fn from_points(a: Pt2, b: Pt2) -> Rect {
        let (x0, x1) = if a.x.0 <= b.x.0 { (a.x, b.x) } else { (b.x, a.x) };
        let (y0, y1) = if a.y.0 <= b.y.0 { (a.y, b.y) } else { (b.y, a.y) };
        Rect {
            min: Pt2 { x: x0, y: y0 },
            max: Pt2 { x: x1, y: y1 },
        }
    }

    /// The zero-size rectangle at `p`.
    pub const fn from_point(p: Pt2) -> Rect {
        Rect { min: p, max: p }
    }

    /// The corner with the smallest coordinates (top-left on a sheet).
    pub const fn min(self) -> Pt2 {
        self.min
    }

    /// The corner with the largest coordinates (bottom-right on a sheet).
    pub const fn max(self) -> Pt2 {
        self.max
    }

    /// `max.x − min.x`. Overflows like `i64` subtraction if the span exceeds `i64::MAX`.
    pub fn width(self) -> Nm {
        self.max.x - self.min.x
    }

    /// `max.y − min.y`. Overflows like `i64` subtraction if the span exceeds `i64::MAX`.
    pub fn height(self) -> Nm {
        self.max.y - self.min.y
    }

    /// Whether `p` lies inside or on the boundary.
    pub const fn contains(self, p: Pt2) -> bool {
        self.min.x.0 <= p.x.0 && p.x.0 <= self.max.x.0 && self.min.y.0 <= p.y.0 && p.y.0 <= self.max.y.0
    }

    /// The smallest rectangle containing both.
    pub fn union(self, other: Rect) -> Rect {
        Rect {
            min: Pt2 {
                x: self.min.x.min(other.min.x),
                y: self.min.y.min(other.min.y),
            },
            max: Pt2 {
                x: self.max.x.max(other.max.x),
                y: self.max.y.max(other.max.y),
            },
        }
    }

    /// Whether the two closed rectangles share at least one point (touching edges count).
    pub const fn intersects(self, other: Rect) -> bool {
        self.min.x.0 <= other.max.x.0
            && other.min.x.0 <= self.max.x.0
            && self.min.y.0 <= other.max.y.0
            && other.min.y.0 <= self.max.y.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_SAFE_INTEGER;
    use proptest::prelude::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    #[test]
    fn pt_arithmetic() {
        assert_eq!(Pt2::ORIGIN, Pt2::default());
        assert_eq!(p(1, 2) + p(3, 4), p(4, 6));
        assert_eq!(p(1, 2) - p(3, 4), p(-2, -2));
        assert_eq!(-p(1, -2), p(-1, 2));
        assert_eq!(p(1, 2).checked_add(p(3, 4)), Some(p(4, 6)));
        assert_eq!(p(i64::MAX, 0).checked_add(p(1, 0)), None);
        assert_eq!(p(0, i64::MIN).checked_sub(p(0, 1)), None);
        assert_eq!(p(5, 5).checked_sub(p(1, 2)), Some(p(4, 3)));
    }

    #[test]
    fn pt_json() {
        assert_eq!(
            serde_json::to_string(&p(42_333_333, 15_875_000)).unwrap(),
            "[42333333,15875000]"
        );
        assert_eq!(serde_json::from_str::<Pt2>("[-1,2]").unwrap(), p(-1, 2));
        for bad in [
            "[1]",
            "[1,2,3]",
            "[1.0,2]",
            "{\"x\":1,\"y\":2}",
            "[1,9007199254740992]",
            "1",
        ] {
            assert!(serde_json::from_str::<Pt2>(bad).is_err(), "{bad}");
        }
        assert!(serde_json::to_string(&p(0, MAX_SAFE_INTEGER + 1)).is_err());
    }

    #[test]
    fn rect_normalises_and_measures() {
        let r = Rect::from_points(p(10, -5), p(-10, 5));
        assert_eq!(r.min(), p(-10, -5));
        assert_eq!(r.max(), p(10, 5));
        assert_eq!(r.width(), Nm(20));
        assert_eq!(r.height(), Nm(10));
        assert_eq!(Rect::from_point(p(3, 4)).width(), Nm(0));
    }

    #[test]
    fn rect_contains_is_closed() {
        let r = Rect::from_points(p(0, 0), p(10, 10));
        assert!(r.contains(p(0, 0)));
        assert!(r.contains(p(10, 10)));
        assert!(r.contains(p(5, 10)));
        assert!(!r.contains(p(11, 5)));
        assert!(!r.contains(p(5, -1)));
    }

    #[test]
    fn rect_union_and_intersects() {
        let a = Rect::from_points(p(0, 0), p(10, 10));
        let b = Rect::from_points(p(10, 10), p(20, 30));
        let c = Rect::from_points(p(11, 0), p(20, 5));
        assert_eq!(a.union(b), Rect::from_points(p(0, 0), p(20, 30)));
        assert!(a.intersects(b), "touching corners intersect");
        assert!(b.intersects(a));
        assert!(!a.intersects(c));
        assert!(!c.intersects(a));
        assert!(b.intersects(c) == c.intersects(b));
    }

    proptest! {
        #[test]
        fn pt_serde_round_trip(x in -MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER, y in -MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER) {
            let s = serde_json::to_string(&p(x, y)).unwrap();
            prop_assert_eq!(serde_json::from_str::<Pt2>(&s).unwrap(), p(x, y));
        }

        #[test]
        fn rect_union_contains_both(a in any::<(i32, i32, i32, i32)>(), b in any::<(i32, i32, i32, i32)>()) {
            let ra = Rect::from_points(p(a.0.into(), a.1.into()), p(a.2.into(), a.3.into()));
            let rb = Rect::from_points(p(b.0.into(), b.1.into()), p(b.2.into(), b.3.into()));
            let u = ra.union(rb);
            for q in [ra.min(), ra.max(), rb.min(), rb.max()] {
                prop_assert!(u.contains(q));
            }
            prop_assert_eq!(ra.intersects(rb), rb.intersects(ra));
            prop_assert!(u.intersects(ra));
        }
    }
}
