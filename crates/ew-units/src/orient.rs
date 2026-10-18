// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Orient2`: the eight orientation variants of a symbol.

use serde::{Deserialize, Serialize};

use crate::{Nm, Pt2};

/// One of the eight orientation variants of a symbol representation (FR-SYM-004): four rotations, each
/// optionally mirrored. Symbol representations use only `Orient2`; free texts and graphics at other angles
/// use [`Angle2`](crate::Angle2).
///
/// The variants are integer matrices in sheet coordinates (origin top-left, **y down**; doc 07 §4.1):
///
/// - `R90` maps `(x, y) → (−y, x)`: a quarter turn **clockwise as seen on the sheet**; `R180` and `R270`
///   repeat it.
/// - `Mθ` **mirrors x first** (`x → −x`, about the symbol's vertical axis) and **then** rotates by θ.
///
/// | Variant | Letter | `(x, y) →` |
/// |---|---|---|
/// | `R0` | A | `( x,  y)` |
/// | `R90` | B | `(−y,  x)` |
/// | `R180` | C | `(−x, −y)` |
/// | `R270` | D | `( y, −x)` |
/// | `M0` | E | `(−x,  y)` |
/// | `M90` | F | `(−y, −x)` |
/// | `M180` | G | `( x, −y)` |
/// | `M270` | H | `( y,  x)` |
///
/// The letters are the variant keys of doc 06 (A–D rotations clockwise, E–H mirrored); whether EPLAN uses
/// the same order is unverified (doc 06 open question 4).
///
/// Because every matrix entry is 0 or ±1, an offset that is a multiple of a grid step stays a multiple of
/// it in all eight variants, with no rounding (doc 07 §4.1).
///
/// **JSON:** the lower-case variant name, `"r0"` … `"m270"`; the default `"r0"` is omitted by record
/// writers ([`is_default`](Orient2::is_default)).
///
/// ```
/// use ew_units::{Nm, Orient2, Pt2};
///
/// let p = Pt2::new(Nm(3), Nm(1));
/// assert_eq!(Orient2::R90.apply(p), Pt2::new(Nm(-1), Nm(3)));
/// assert_eq!(Orient2::M0.apply(p), Pt2::new(Nm(-3), Nm(1)));
/// assert_eq!(Orient2::R90.then(Orient2::R90), Orient2::R180);
/// assert_eq!(serde_json::to_string(&Orient2::M90).unwrap(), "\"m90\"");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orient2 {
    /// No rotation, not mirrored (letter A).
    #[default]
    R0,
    /// A quarter turn clockwise on the sheet (letter B).
    R90,
    /// A half turn (letter C).
    R180,
    /// Three quarter turns clockwise, i.e. a quarter turn counter-clockwise (letter D).
    R270,
    /// Mirrored about the vertical axis (letter E).
    M0,
    /// Mirrored, then a quarter turn clockwise (letter F).
    M90,
    /// Mirrored, then a half turn (letter G).
    M180,
    /// Mirrored, then three quarter turns clockwise (letter H).
    M270,
}

impl Orient2 {
    /// All eight variants in letter order A–H.
    pub const ALL: [Orient2; 8] = [
        Orient2::R0,
        Orient2::R90,
        Orient2::R180,
        Orient2::R270,
        Orient2::M0,
        Orient2::M90,
        Orient2::M180,
        Orient2::M270,
    ];

    /// The variant that mirrors (if `mirrored`) and then turns clockwise by `quarter_turns × 90°`
    /// (taken modulo 4).
    pub const fn from_parts(mirrored: bool, quarter_turns: u8) -> Orient2 {
        let i = (quarter_turns % 4) as usize + if mirrored { 4 } else { 0 };
        Orient2::ALL[i]
    }

    /// Whether the variant mirrors (before rotating).
    pub const fn is_mirrored(self) -> bool {
        matches!(self, Orient2::M0 | Orient2::M90 | Orient2::M180 | Orient2::M270)
    }

    /// The clockwise rotation in quarter turns, `0..=3`, applied after the mirror.
    pub const fn quarter_turns(self) -> u8 {
        match self {
            Orient2::R0 | Orient2::M0 => 0,
            Orient2::R90 | Orient2::M90 => 1,
            Orient2::R180 | Orient2::M180 => 2,
            Orient2::R270 | Orient2::M270 => 3,
        }
    }

    /// The clockwise rotation in degrees (0, 90, 180 or 270), applied after the mirror.
    pub const fn rotation(self) -> u16 {
        self.quarter_turns() as u16 * 90
    }

    /// Whether this is the default `R0` (omitted in JSON).
    pub const fn is_default(self) -> bool {
        matches!(self, Orient2::R0)
    }

    /// Maps `p` by this variant's integer matrix. Overflows like `i64` negation for a coordinate of
    /// `i64::MIN`; see [`checked_apply`](Orient2::checked_apply).
    pub const fn apply(self, p: Pt2) -> Pt2 {
        let x = if self.is_mirrored() { -p.x.0 } else { p.x.0 };
        let y = p.y.0;
        let (x, y) = match self.quarter_turns() {
            0 => (x, y),
            1 => (-y, x),
            2 => (-x, -y),
            _ => (y, -x),
        };
        Pt2 { x: Nm(x), y: Nm(y) }
    }

    /// Like [`apply`](Orient2::apply), but `None` if a negation overflows.
    pub const fn checked_apply(self, p: Pt2) -> Option<Pt2> {
        if p.x.0 == i64::MIN || p.y.0 == i64::MIN {
            None
        } else {
            Some(self.apply(p))
        }
    }

    /// The matrix `[[a, b], [c, d]]` with `x' = a·x + b·y` and `y' = c·x + d·y`.
    pub const fn matrix(self) -> [[i8; 2]; 2] {
        let e1 = self.apply(Pt2 { x: Nm(1), y: Nm(0) });
        let e2 = self.apply(Pt2 { x: Nm(0), y: Nm(1) });
        [[e1.x.0 as i8, e2.x.0 as i8], [e1.y.0 as i8, e2.y.0 as i8]]
    }

    /// The composition "first `self`, then `other`": `self.then(other).apply(p) ==
    /// other.apply(self.apply(p))`.
    pub const fn then(self, other: Orient2) -> Orient2 {
        // other ∘ self = R^r2 M^m2 R^r1 M^m1, and M R^r = R^(−r) M.
        let (r1, r2) = (self.quarter_turns(), other.quarter_turns());
        let r = if other.is_mirrored() {
            (4 + r2 - r1) % 4
        } else {
            (r1 + r2) % 4
        };
        Orient2::from_parts(self.is_mirrored() != other.is_mirrored(), r)
    }

    /// The inverse variant: `self.then(self.inverse()) == R0`. Mirrored variants are their own inverse.
    pub const fn inverse(self) -> Orient2 {
        if self.is_mirrored() {
            self
        } else {
            Orient2::from_parts(false, (4 - self.quarter_turns()) % 4)
        }
    }

    /// The variant letter of doc 06: `A`–`D` = `R0`–`R270`, `E`–`H` = `M0`–`M270`.
    pub const fn letter(self) -> char {
        match self {
            Orient2::R0 => 'A',
            Orient2::R90 => 'B',
            Orient2::R180 => 'C',
            Orient2::R270 => 'D',
            Orient2::M0 => 'E',
            Orient2::M90 => 'F',
            Orient2::M180 => 'G',
            Orient2::M270 => 'H',
        }
    }

    /// The variant for an upper-case letter `A`–`H`; `None` for any other character.
    pub const fn from_letter(c: char) -> Option<Orient2> {
        match c {
            'A'..='H' => Some(Orient2::ALL[(c as u32 - 'A' as u32) as usize]),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x), Nm(y))
    }

    fn mat_mul(a: [[i8; 2]; 2], b: [[i8; 2]; 2]) -> [[i8; 2]; 2] {
        [
            [
                a[0][0] * b[0][0] + a[0][1] * b[1][0],
                a[0][0] * b[0][1] + a[0][1] * b[1][1],
            ],
            [
                a[1][0] * b[0][0] + a[1][1] * b[1][0],
                a[1][0] * b[0][1] + a[1][1] * b[1][1],
            ],
        ]
    }

    #[test]
    fn matrices_match_the_stated_formulas() {
        let (x, y) = (3, 7);
        let expected = [
            (Orient2::R0, p(x, y)),
            (Orient2::R90, p(-y, x)),
            (Orient2::R180, p(-x, -y)),
            (Orient2::R270, p(y, -x)),
            (Orient2::M0, p(-x, y)),
            (Orient2::M90, p(-y, -x)),
            (Orient2::M180, p(x, -y)),
            (Orient2::M270, p(y, x)),
        ];
        for (o, q) in expected {
            assert_eq!(o.apply(p(x, y)), q, "{o:?}");
        }
        assert_eq!(Orient2::R90.matrix(), [[0, -1], [1, 0]]);
        assert_eq!(Orient2::M0.matrix(), [[-1, 0], [0, 1]]);
    }

    #[test]
    fn mirror_is_applied_before_rotation() {
        for o in Orient2::ALL {
            let rot = Orient2::from_parts(false, o.quarter_turns());
            let pre = if o.is_mirrored() { Orient2::M0 } else { Orient2::R0 };
            assert_eq!(pre.then(rot), o);
            assert_eq!(o.matrix(), mat_mul(rot.matrix(), pre.matrix()));
        }
    }

    /// The exhaustive `then` table agrees with matrix multiplication, so the eight variants form a
    /// closed group.
    #[test]
    fn then_table_is_matrix_product() {
        for a in Orient2::ALL {
            for b in Orient2::ALL {
                let c = a.then(b);
                assert_eq!(c.matrix(), mat_mul(b.matrix(), a.matrix()), "{a:?} then {b:?}");
                assert!(Orient2::ALL.contains(&c));
            }
        }
    }

    #[test]
    fn group_laws() {
        for a in Orient2::ALL {
            assert_eq!(a.then(Orient2::R0), a);
            assert_eq!(Orient2::R0.then(a), a);
            assert_eq!(a.then(a.inverse()), Orient2::R0);
            assert_eq!(a.inverse().then(a), Orient2::R0);
            for b in Orient2::ALL {
                for c in Orient2::ALL {
                    assert_eq!(a.then(b).then(c), a.then(b.then(c)));
                }
            }
        }
        assert_eq!(Orient2::R90.inverse(), Orient2::R270);
        assert_eq!(Orient2::M90.inverse(), Orient2::M90);
    }

    #[test]
    fn letters() {
        let letters: String = Orient2::ALL.iter().map(|o| o.letter()).collect();
        assert_eq!(letters, "ABCDEFGH");
        for o in Orient2::ALL {
            assert_eq!(Orient2::from_letter(o.letter()), Some(o));
        }
        assert_eq!(Orient2::from_letter('I'), None);
        assert_eq!(Orient2::from_letter('a'), None);
        assert_eq!(Orient2::from_letter('@'), None);
    }

    #[test]
    fn parts_and_rotation() {
        assert_eq!(Orient2::from_parts(true, 5), Orient2::M90);
        assert_eq!(Orient2::M270.rotation(), 270);
        assert!(Orient2::M0.is_mirrored());
        assert!(!Orient2::R180.is_mirrored());
        assert!(Orient2::default().is_default());
        assert!(!Orient2::R90.is_default());
    }

    #[test]
    fn checked_apply_guards_negation() {
        assert_eq!(Orient2::R180.checked_apply(p(i64::MIN, 0)), None);
        assert_eq!(Orient2::R0.checked_apply(p(0, i64::MIN)), None);
        assert_eq!(Orient2::R90.checked_apply(p(1, 2)), Some(p(-2, 1)));
    }

    #[test]
    fn json_encoding() {
        let names = ["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"];
        for (o, n) in Orient2::ALL.iter().zip(names) {
            assert_eq!(serde_json::to_string(o).unwrap(), format!("\"{n}\""));
            assert_eq!(serde_json::from_str::<Orient2>(&format!("\"{n}\"")).unwrap(), *o);
        }
        for bad in ["\"R90\"", "\"r45\"", "\"B\"", "1", "null"] {
            assert!(serde_json::from_str::<Orient2>(bad).is_err(), "{bad}");
        }
    }

    fn orients() -> impl Strategy<Value = Orient2> {
        (0usize..8).prop_map(|i| Orient2::ALL[i])
    }

    proptest! {
        #[test]
        fn inverse_undoes_apply(o in orients(), x in -1_000_000_000_000i64..1_000_000_000_000, y in -1_000_000_000_000i64..1_000_000_000_000) {
            prop_assert_eq!(o.inverse().apply(o.apply(p(x, y))), p(x, y));
        }

        #[test]
        fn then_is_composition(a in orients(), b in orients(), x in -1_000_000i64..1_000_000, y in -1_000_000i64..1_000_000) {
            prop_assert_eq!(a.then(b).apply(p(x, y)), b.apply(a.apply(p(x, y))));
        }

        /// CP offsets on the connection grid stay on it in every variant (doc 07 §4.1).
        #[test]
        fn grid_offsets_stay_on_grid(o in orients(), i in -10_000i64..10_000, j in -10_000i64..10_000) {
            let g = crate::Grid::QET_10PX;
            // An offset of (i, j) grid steps, as an exact rational point; test on the integer step count.
            let q = o.apply(p(i, j));
            prop_assert!(q.x.0.abs() <= 10_000 && q.y.0.abs() <= 10_000);
            // The mapped offset is again (i', j') whole steps, so its coordinate is g.at(i').
            let x = g.at(q.x.0).unwrap();
            prop_assert_eq!(g.snap(x), q.x.0);
            // On an integer grid the mapped coordinates are exact multiples.
            let mm4 = crate::Grid::MM4;
            let r = o.apply(p(mm4.at(i).unwrap().0, mm4.at(j).unwrap().0));
            prop_assert_eq!(r.x.0 % 4_000_000, 0);
            prop_assert_eq!(r.y.0 % 4_000_000, 0);
        }

        #[test]
        fn serde_round_trip(o in orients()) {
            let s = serde_json::to_string(&o).unwrap();
            prop_assert_eq!(serde_json::from_str::<Orient2>(&s).unwrap(), o);
        }
    }
}
