// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Transform`: an integer placement (orientation variant, angle, translation).

use ew_units::{Angle2, Orient2, Pt2};

use crate::num;

/// A placement in sheet coordinates: first the orientation variant, then a rotation by `angle` about the
/// origin, then the translation.
///
/// `apply(p) = rotate(orient.apply(p), angle) + translate`, with the crate's angle sense: positive
/// counter-clockwise on the sheet ([`num`] module docs). [`Orient2::R90`] is a quarter turn **clockwise**,
/// so `Transform::from_orient(Orient2::R90)` and `Transform::from_angle(Angle2(-90_000))` are the same map.
///
/// Symbol instances keep `angle == Angle2::ZERO` (ADR-0005: symbol representations use [`Orient2`]); free
/// texts and graphics may carry an angle. With an angle that is a multiple of 90° every operation is exact
/// integer arithmetic; other angles round each mapped point to whole nanometres ([`num::rotate`]).
///
/// ```
/// use ew_dl::Transform;
/// use ew_units::{Angle2, Nm, Orient2, Pt2};
///
/// let t = Transform::new(Pt2::new(Nm(100), Nm(0)), Orient2::M0, Angle2(90_000));
/// // (3, 1) → mirror → (−3, 1) → +90° → (1, 3) → translate → (101, 3).
/// assert_eq!(t.apply(Pt2::new(Nm(3), Nm(1))), Pt2::new(Nm(101), Nm(3)));
/// assert_eq!(
///     Transform::from_orient(Orient2::R90).apply(Pt2::new(Nm(1), Nm(2))),
///     Transform::from_angle(Angle2(-90_000)).apply(Pt2::new(Nm(1), Nm(2))),
/// );
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Transform {
    /// Translation, applied last.
    pub translate: Pt2,
    /// Orientation variant, applied first.
    pub orient: Orient2,
    /// Rotation about the origin, counter-clockwise on the sheet, applied after `orient`.
    pub angle: Angle2,
}

impl Transform {
    /// The identity.
    pub const IDENTITY: Transform = Transform {
        translate: Pt2::ORIGIN,
        orient: Orient2::R0,
        angle: Angle2::ZERO,
    };

    /// A transform from its parts.
    pub const fn new(translate: Pt2, orient: Orient2, angle: Angle2) -> Transform {
        Transform {
            translate,
            orient,
            angle,
        }
    }

    /// A pure translation.
    pub const fn from_translation(translate: Pt2) -> Transform {
        Transform::new(translate, Orient2::R0, Angle2::ZERO)
    }

    /// A pure orientation variant.
    pub const fn from_orient(orient: Orient2) -> Transform {
        Transform::new(Pt2::ORIGIN, orient, Angle2::ZERO)
    }

    /// A pure rotation about the origin.
    pub const fn from_angle(angle: Angle2) -> Transform {
        Transform::new(Pt2::ORIGIN, Orient2::R0, angle)
    }

    /// Whether this is the identity map (an angle of a whole number of turns counts as zero).
    pub const fn is_identity(self) -> bool {
        self.translate.x.0 == 0
            && self.translate.y.0 == 0
            && matches!(self.orient, Orient2::R0)
            && self.angle.normalized().0 == 0
    }

    /// Whether every operation with this transform is exact integer arithmetic: the angle is a multiple
    /// of 90°.
    pub const fn is_exact(self) -> bool {
        self.angle.0 % 90_000 == 0
    }

    /// Maps `p`: orientation, then rotation about the origin, then translation. Exact when
    /// [`is_exact`](Transform::is_exact); otherwise the rotated offset is rounded to whole nanometres, ties
    /// toward zero, before the translation is added.
    pub fn apply(self, p: Pt2) -> Pt2 {
        num::rotate(self.orient.apply(p), self.angle, Pt2::ORIGIN) + self.translate
    }

    /// The composition "first `self`, then `next`", like [`Orient2::then`]:
    /// `self.then(next).apply(p) ≈ next.apply(self.apply(p))`.
    ///
    /// The orientations compose exactly. A rotation moved past a mirrored variant changes sign
    /// (`M · R(a) = R(−a) · M`), so the angle is `next.angle ± self.angle`, normalised to `[0°, 360°)`. The
    /// translation is `next.apply(self.translate)`. The result equals sequential application **exactly**
    /// when both angles are multiples of 90°, and within 2 nm per coordinate otherwise (sequential
    /// application rounds twice, the composite once plus the rounded translation).
    ///
    /// ```
    /// use ew_dl::Transform;
    /// use ew_units::{Angle2, Nm, Orient2, Pt2};
    ///
    /// let a = Transform::new(Pt2::new(Nm(10), Nm(0)), Orient2::R90, Angle2::ZERO);
    /// let b = Transform::new(Pt2::new(Nm(0), Nm(5)), Orient2::M0, Angle2(180_000));
    /// let p = Pt2::new(Nm(3), Nm(4));
    /// assert_eq!(a.then(b).apply(p), b.apply(a.apply(p)));
    /// ```
    pub fn then(self, next: Transform) -> Transform {
        let inner = if next.orient.is_mirrored() {
            -i64::from(self.angle.0)
        } else {
            i64::from(self.angle.0)
        };
        let angle = (i64::from(next.angle.0) + inner).rem_euclid(360_000) as i32;
        Transform {
            translate: next.apply(self.translate),
            orient: self.orient.then(next.orient),
            angle: Angle2(angle),
        }
    }

    /// The f64 affine map in nm for renderers and picking (`ew-scene`, `ew-render2d`, `ew-render-cpu`).
    /// The sine and cosine come from [`num::sin_cos`], so multiples of 90° give exact matrix entries; the
    /// export path never uses this conversion for its own arithmetic.
    ///
    /// ```
    /// use ew_dl::Transform;
    /// use ew_units::{Angle2, Nm, Orient2, Pt2};
    ///
    /// let t = Transform::new(Pt2::new(Nm(7), Nm(-2)), Orient2::M90, Angle2(270_000));
    /// let p = Pt2::new(Nm(3), Nm(5));
    /// let q = t.to_affine() * kurbo::Point::new(3.0, 5.0);
    /// let want = t.apply(p);
    /// assert_eq!((q.x, q.y), (want.x.0 as f64, want.y.0 as f64));
    /// ```
    pub fn to_affine(self) -> kurbo::Affine {
        let (s, c) = num::sin_cos(self.angle);
        let [[o00, o01], [o10, o11]] = self.orient.matrix().map(|row| row.map(f64::from));
        // R = [[c, s], [−s, c]] (counter-clockwise on the sheet); M = R · O.
        let m00 = c * o00 + s * o10;
        let m01 = c * o01 + s * o11;
        let m10 = c * o10 - s * o00;
        let m11 = c * o11 - s * o01;
        // kurbo: x' = a·x + c·y + e, y' = b·x + d·y + f, coefficients [a, b, c, d, e, f].
        kurbo::Affine::new([
            m00,
            m10,
            m01,
            m11,
            self.translate.x.0 as f64,
            self.translate.y.0 as f64,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_units::Nm;
    use proptest::prelude::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    #[test]
    fn identity() {
        assert_eq!(Transform::default(), Transform::IDENTITY);
        assert!(Transform::IDENTITY.is_identity());
        assert!(Transform::from_angle(Angle2(360_000)).is_identity());
        assert!(!Transform::from_orient(Orient2::M0).is_identity());
        assert_eq!(Transform::IDENTITY.apply(p(5, -7)), p(5, -7));
        let t = Transform::new(p(1, 2), Orient2::M270, Angle2(12_345));
        assert_eq!(Transform::IDENTITY.then(t), t);
        assert_eq!(t.then(Transform::IDENTITY), t);
    }

    #[test]
    fn order_of_operations() {
        // Orient first, then angle, then translation.
        let t = Transform::new(p(1_000, 0), Orient2::R90, Angle2(90_000));
        // (2, 0) → R90 → (0, 2) → +90° → (2, 0) → +(1000, 0).
        assert_eq!(t.apply(p(2, 0)), p(1_002, 0));
        assert!(t.is_exact());
        assert!(!Transform::from_angle(Angle2(45_000)).is_exact());
    }

    #[test]
    fn mirrored_composition_negates_the_inner_angle() {
        let a = Transform::from_angle(Angle2(30_000));
        let b = Transform::from_orient(Orient2::M0);
        let c = a.then(b);
        assert_eq!(c.orient, Orient2::M0);
        assert_eq!(c.angle, Angle2(330_000));
        let q = p(1_000_000, 0);
        assert_eq!(c.apply(q), b.apply(a.apply(q)));
    }

    #[test]
    fn affine_matches_apply_for_exact_transforms() {
        for o in Orient2::ALL {
            for a in [0, 90_000, 180_000, 270_000, -90_000] {
                let t = Transform::new(p(11, -13), o, Angle2(a));
                let aff = t.to_affine();
                for q in [p(1, 0), p(0, 1), p(-7, 3)] {
                    let r = aff * kurbo::Point::new(q.x.0 as f64, q.y.0 as f64);
                    let want = t.apply(q);
                    assert_eq!((r.x, r.y), (want.x.0 as f64, want.y.0 as f64), "{t:?} {q:?}");
                }
            }
        }
    }

    fn orients() -> impl Strategy<Value = Orient2> {
        (0usize..8).prop_map(|i| Orient2::ALL[i])
    }

    fn coord() -> impl Strategy<Value = i64> {
        -1_000_000_000_000i64..1_000_000_000_000
    }

    proptest! {
        #[test]
        fn composition_is_exact_for_quarter_turns(
            oa in orients(), ob in orients(), qa in -8i32..8, qb in -8i32..8,
            ta in (coord(), coord()), tb in (coord(), coord()), q in (coord(), coord()),
        ) {
            let a = Transform::new(p(ta.0, ta.1), oa, Angle2(qa * 90_000));
            let b = Transform::new(p(tb.0, tb.1), ob, Angle2(qb * 90_000));
            let q = p(q.0, q.1);
            prop_assert_eq!(a.then(b).apply(q), b.apply(a.apply(q)));
        }

        #[test]
        fn composition_is_close_for_any_angle(
            oa in orients(), ob in orients(), aa in any::<i32>(), ab in any::<i32>(),
            ta in (coord(), coord()), tb in (coord(), coord()), q in (coord(), coord()),
        ) {
            let a = Transform::new(p(ta.0, ta.1), oa, Angle2(aa));
            let b = Transform::new(p(tb.0, tb.1), ob, Angle2(ab));
            let q = p(q.0, q.1);
            let (c, s) = (a.then(b).apply(q), b.apply(a.apply(q)));
            prop_assert!((c.x.0 - s.x.0).abs() <= 2 && (c.y.0 - s.y.0).abs() <= 2, "{c:?} vs {s:?}");
        }

        #[test]
        fn composition_is_associative_for_quarter_turns(
            o in (orients(), orients(), orients()), q in (-4i32..4, -4i32..4, -4i32..4),
            t in ((-1_000i64..1_000, -1_000i64..1_000), (-1_000i64..1_000, -1_000i64..1_000), (-1_000i64..1_000, -1_000i64..1_000)),
        ) {
            let a = Transform::new(p(t.0.0, t.0.1), o.0, Angle2(q.0 * 90_000));
            let b = Transform::new(p(t.1.0, t.1.1), o.1, Angle2(q.1 * 90_000));
            let c = Transform::new(p(t.2.0, t.2.1), o.2, Angle2(q.2 * 90_000));
            prop_assert_eq!(a.then(b).then(c), a.then(b.then(c)));
        }

        #[test]
        fn affine_is_close_to_apply(o in orients(), a in any::<i32>(), t in (coord(), coord()), q in (-1_000_000_000i64..1_000_000_000, -1_000_000_000i64..1_000_000_000)) {
            let tr = Transform::new(p(t.0, t.1), o, Angle2(a));
            let r = tr.to_affine() * kurbo::Point::new(q.0 as f64, q.1 as f64);
            let want = tr.apply(p(q.0, q.1));
            prop_assert!((r.x - want.x.0 as f64).abs() <= 0.5 + 1e-3 && (r.y - want.y.0 as f64).abs() <= 0.5 + 1e-3);
        }
    }
}
