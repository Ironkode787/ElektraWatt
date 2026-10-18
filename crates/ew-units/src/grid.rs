// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Grid`: a rational grid step.

use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::json::{MAX_SAFE_INTEGER, SafeInt};
use crate::{Nm, UnitsError, arith};

/// A rational grid step of `num / den` nanometres (ADR-0005).
///
/// Stored in lowest terms with `den ≥ 1` and `num ≥ den` (a step of at least 1 nm, since coordinates are
/// whole nanometres); both terms are at most 2⁵³ − 1. These bounds keep every computation below exact in
/// `i128`.
///
/// Points snap to grid **indices** `k` ([`snap`](Grid::snap)); the stored coordinate is
/// `round(k × num / den)` ([`at`](Grid::at)), an error of at most 0.5 nm. Snapping never compares floats.
///
/// **JSON:** `{"num":…,"den":…}`, reduced. Deserialising rejects zero, unreduced, sub-nanometre and
/// out-of-range steps and unknown members.
///
/// ```
/// use ew_units::{Grid, Nm};
///
/// let g = Grid::QET_10PX; // 10 px at 96 dpi = 7 937 500 / 3 nm
/// assert_eq!(g.at(3), Some(Nm(7_937_500)));
/// assert_eq!(g.at(1), Some(Nm(2_645_833))); // 2 645 833.33… rounded
/// assert_eq!(g.snap(Nm(2_645_833)), 1);
/// assert_eq!(serde_json::to_string(&g).unwrap(), r#"{"num":7937500,"den":3}"#);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "GridRepr", into = "GridRepr")]
pub struct Grid {
    num: u64,
    den: u64,
}

/// The JSON shape of [`Grid`]. Deserialised by hand so that only the object form is accepted (a derived
/// impl would also take `[num, den]`).
#[derive(Clone, Copy, Serialize)]
struct GridRepr {
    num: SafeInt,
    den: SafeInt,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum GridField {
    Num,
    Den,
}

impl<'de> Deserialize<'de> for GridRepr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("Grid", &["num", "den"], GridVisitor)
    }
}

struct GridVisitor;

impl<'de> Visitor<'de> for GridVisitor {
    type Value = GridRepr;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(r#"an object {"num": …, "den": …}"#)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<GridRepr, A::Error> {
        let (mut num, mut den) = (None, None);
        while let Some(field) = map.next_key()? {
            let slot = match field {
                GridField::Num => &mut num,
                GridField::Den => &mut den,
            };
            if slot.is_some() {
                return Err(de::Error::custom("duplicate grid member"));
            }
            *slot = Some(map.next_value()?);
        }
        match (num, den) {
            (Some(num), Some(den)) => Ok(GridRepr { num, den }),
            (None, _) => Err(de::Error::missing_field("num")),
            (_, None) => Err(de::Error::missing_field("den")),
        }
    }
}

impl From<Grid> for GridRepr {
    fn from(g: Grid) -> GridRepr {
        // Both terms are at most 2^53 - 1 by construction, so the casts are exact.
        GridRepr {
            num: SafeInt(g.num as i64),
            den: SafeInt(g.den as i64),
        }
    }
}

impl TryFrom<GridRepr> for Grid {
    type Error = UnitsError;
    fn try_from(r: GridRepr) -> Result<Grid, UnitsError> {
        let (num, den) = (r.num.0, r.den.0);
        if num <= 0 || den <= 0 {
            return Err(UnitsError::ZeroGrid);
        }
        let (num, den) = (num.unsigned_abs(), den.unsigned_abs());
        if gcd(num, den) != 1 {
            return Err(UnitsError::UnreducedGrid);
        }
        Grid::new(num, den)
    }
}

const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl Grid {
    /// The QET 10 px wiring grid: 10 × 25.4 / 96 mm = `7_937_500 / 3` nm (research G4).
    pub const QET_10PX: Grid = Grid {
        num: 7_937_500,
        den: 3,
    };
    /// One QET scene pixel: 25.4 / 96 mm = `793_750 / 3` nm.
    pub const QET_1PX: Grid = Grid { num: 793_750, den: 3 };
    /// A 4 mm step, `4_000_000 / 1` nm. (ADR-0027 proposes it as a connection grid; it is still only
    /// proposed, so this is a plain constant.)
    pub const MM4: Grid = Grid {
        num: 4_000_000,
        den: 1,
    };

    /// The step `num / den` nm, reduced to lowest terms.
    ///
    /// # Errors
    ///
    /// [`UnitsError::ZeroGrid`] if either term is zero, [`UnitsError::GridTooFine`] if the step is below
    /// 1 nm, [`UnitsError::IntegerOutOfRange`] if a reduced term exceeds 2⁵³ − 1.
    pub const fn new(num: u64, den: u64) -> Result<Grid, UnitsError> {
        if num == 0 || den == 0 {
            return Err(UnitsError::ZeroGrid);
        }
        let g = gcd(num, den);
        let (num, den) = (num / g, den / g);
        if num < den {
            return Err(UnitsError::GridTooFine);
        }
        if num > MAX_SAFE_INTEGER as u64 {
            return Err(UnitsError::IntegerOutOfRange);
        }
        Ok(Grid { num, den })
    }

    /// A step of `n` whole millimetres.
    ///
    /// # Errors
    ///
    /// [`UnitsError::ZeroGrid`] for `n = 0`, [`UnitsError::IntegerOutOfRange`] if `n` mm exceeds
    /// 2⁵³ − 1 nm.
    pub const fn mm(n: u64) -> Result<Grid, UnitsError> {
        match n.checked_mul(1_000_000) {
            Some(num) => Grid::new(num, 1),
            None => Err(UnitsError::IntegerOutOfRange),
        }
    }

    /// The numerator, in nm.
    pub const fn num(self) -> u64 {
        self.num
    }

    /// The denominator (≥ 1).
    pub const fn den(self) -> u64 {
        self.den
    }

    /// The grid index nearest to `x`: `round(x × den / num)`, exact ties **toward zero** (a point exactly
    /// halfway between two grid positions snaps to the one nearer the origin; doc 07 §3 rule 5).
    ///
    /// Computed exactly in `i128`. Because the step is at least 1 nm, `|k| ≤ |x|`, so the result always
    /// fits.
    pub fn snap(self, x: Nm) -> i64 {
        let k = arith::div_round(i128::from(x.0) * i128::from(self.den), i128::from(self.num));
        // |k| <= |x| <= 2^63 (see above); i64::MIN maps to itself only for num == den == 1.
        i64::try_from(k).unwrap_or(if k < 0 { i64::MIN } else { i64::MAX })
    }

    /// The coordinate of grid index `k`: `round(k × num / den)` nm, exact ties toward zero; `None` if it
    /// overflows `i64`. The error against the exact rational position is at most 0.5 nm.
    pub fn at(self, k: i64) -> Option<Nm> {
        let v = arith::div_round(i128::from(k) * i128::from(self.num), i128::from(self.den));
        i64::try_from(v).ok().map(Nm)
    }

    /// The point `x` snapped to the nearest grid position (`at(snap(x))`); `None` if that overflows.
    pub fn snap_point(self, x: Nm) -> Option<Nm> {
        self.at(self.snap(x))
    }

    /// The index of the grid cell containing `x`, for cells of width `num / den` starting at `origin`:
    /// `floor((x − origin) × den / num)` by exact Euclidean division in `i128` (FR-PAG-020).
    ///
    /// **Boundary rule:** cell `j` is the half-open interval `[origin + j·w, origin + (j+1)·w)`, so a point
    /// exactly on a boundary belongs to the right-hand (or lower) cell. The result saturates at the `i64`
    /// limits, which no real page reaches.
    ///
    /// **Pitfall** (doc 06 §5.1): a stored coordinate is rounded to the nearest nm, so a point that lies
    /// on a QET boundary in theory can be stored up to 0.5 nm to its left. Callers that need the exact
    /// cell of an on-grid point therefore evaluate the lookup on the point's exact grid position, not on
    /// the rounded `Nm`.
    ///
    /// ```
    /// use ew_units::{Grid, Nm};
    ///
    /// let col = Grid::mm(10).unwrap();
    /// assert_eq!(col.cell(Nm::from_mm(25), Nm::from_mm(5)), 2);
    /// assert_eq!(col.cell(Nm::from_mm(15), Nm::from_mm(5)), 1); // on the boundary: right-hand cell
    /// assert_eq!(col.cell(Nm::from_mm(4), Nm::from_mm(5)), -1);
    /// ```
    pub fn cell(self, x: Nm, origin: Nm) -> i64 {
        let d = (i128::from(x.0) - i128::from(origin.0)) * i128::from(self.den);
        let c = d.div_euclid(i128::from(self.num));
        i64::try_from(c).unwrap_or(if c < 0 { i64::MIN } else { i64::MAX })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn constants_are_reduced_and_exact() {
        for g in [Grid::QET_10PX, Grid::QET_1PX, Grid::MM4] {
            assert_eq!(Grid::new(g.num(), g.den()), Ok(g));
            assert_eq!(gcd(g.num(), g.den()), 1);
        }
        assert_eq!(Grid::QET_10PX.at(3), Some(Nm(7_937_500)));
        assert_eq!(Grid::QET_1PX.at(30), Some(Nm(7_937_500)));
        assert_eq!(Grid::MM4.at(-2), Some(Nm(-8_000_000)));
        assert_eq!(Grid::mm(4), Ok(Grid::MM4));
    }

    #[test]
    fn new_reduces_and_rejects() {
        assert_eq!(Grid::new(15_875_000, 6), Ok(Grid::QET_10PX));
        assert_eq!(Grid::new(0, 3), Err(UnitsError::ZeroGrid));
        assert_eq!(Grid::new(3, 0), Err(UnitsError::ZeroGrid));
        assert_eq!(Grid::new(1, 3), Err(UnitsError::GridTooFine));
        assert_eq!(Grid::new(2, 2), Ok(Grid { num: 1, den: 1 }));
        assert_eq!(Grid::new(1 << 53, 1), Err(UnitsError::IntegerOutOfRange));
        assert_eq!(Grid::new((1 << 53) - 1, 1).map(Grid::num), Ok((1 << 53) - 1));
        assert_eq!(Grid::mm(0), Err(UnitsError::ZeroGrid));
        assert_eq!(Grid::mm(u64::MAX), Err(UnitsError::IntegerOutOfRange));
        assert_eq!(Grid::mm(9_007_199_255), Err(UnitsError::IntegerOutOfRange));
    }

    #[test]
    fn snap_ties_toward_zero() {
        let g = Grid::mm(2).unwrap();
        assert_eq!(g.snap(Nm::from_mm(1)), 0);
        assert_eq!(g.snap(Nm::from_mm(-1)), 0);
        assert_eq!(g.snap(Nm::from_mm(3)), 1);
        assert_eq!(g.snap(Nm::from_mm(-3)), -1);
        assert_eq!(g.snap(Nm(1_000_001)), 1);
        assert_eq!(g.snap(Nm(-1_000_001)), -1);
        assert_eq!(g.snap(Nm::from_mm(5) - Nm(1)), 2);
        assert_eq!(g.snap_point(Nm(2_999_999)), Some(Nm::from_mm(2)));
    }

    #[test]
    fn snap_extremes_fit() {
        let unit = Grid::new(1, 1).unwrap();
        assert_eq!(unit.snap(Nm(i64::MIN)), i64::MIN);
        assert_eq!(unit.snap(Nm(i64::MAX)), i64::MAX);
        assert_eq!(Grid::QET_1PX.snap(Nm(i64::MAX)), 34_859_988_800_711);
        assert_eq!(unit.at(i64::MIN), Some(Nm(i64::MIN)));
        assert_eq!(Grid::MM4.at(i64::MAX), None);
    }

    #[test]
    fn qet_rounding() {
        // k·7937500/3 has remainder 0, 1/3 or 2/3 of a nm: never a tie.
        assert_eq!(Grid::QET_10PX.at(1), Some(Nm(2_645_833)));
        assert_eq!(Grid::QET_10PX.at(2), Some(Nm(5_291_667)));
        assert_eq!(Grid::QET_10PX.at(-1), Some(Nm(-2_645_833)));
        assert_eq!(Grid::QET_10PX.at(-2), Some(Nm(-5_291_667)));
    }

    #[test]
    fn at_ties_toward_zero() {
        let g = Grid::new(5, 2).unwrap(); // 2.5 nm
        assert_eq!(g.at(1), Some(Nm(2)));
        assert_eq!(g.at(-1), Some(Nm(-2)));
        assert_eq!(g.at(3), Some(Nm(7)));
        assert_eq!(g.at(-3), Some(Nm(-7)));
        assert_eq!(g.at(2), Some(Nm(5)));
    }

    #[test]
    fn cell_boundary_rule() {
        let w = Grid::QET_10PX; // 2 645 833.33… nm columns
        // Boundary 3 lies exactly on 7 937 500 nm: right-hand cell.
        assert_eq!(w.cell(Nm(7_937_500), Nm(0)), 3);
        assert_eq!(w.cell(Nm(7_937_499), Nm(0)), 2);
        assert_eq!(w.cell(Nm(0), Nm(0)), 0);
        assert_eq!(w.cell(Nm(-1), Nm(0)), -1);
        assert_eq!(w.cell(Nm(-7_937_500), Nm(0)), -3);
        assert_eq!(w.cell(Nm(-7_937_501), Nm(0)), -4);
        // With an origin.
        assert_eq!(w.cell(Nm(7_937_500 + 10), Nm(10)), 3);
        // Saturation at the limits.
        let unit = Grid::new(1, 1).unwrap();
        assert_eq!(unit.cell(Nm(i64::MAX), Nm(i64::MIN)), i64::MAX);
        assert_eq!(unit.cell(Nm(i64::MIN), Nm(i64::MAX)), i64::MIN);
    }

    #[test]
    fn json_encoding() {
        assert_eq!(
            serde_json::to_string(&Grid::QET_10PX).unwrap(),
            r#"{"num":7937500,"den":3}"#
        );
        assert_eq!(
            serde_json::from_str::<Grid>(r#"{"num":4000000,"den":1}"#).unwrap(),
            Grid::MM4
        );
        assert_eq!(
            serde_json::from_str::<Grid>(r#"{"den":3,"num":7937500}"#).unwrap(),
            Grid::QET_10PX
        );
        for bad in [
            r#"{"num":15875000,"den":6}"#,
            r#"{"num":0,"den":1}"#,
            r#"{"num":5,"den":0}"#,
            r#"{"num":-5,"den":1}"#,
            r#"{"num":1,"den":3}"#,
            r#"{"num":4000000.0,"den":1}"#,
            r#"{"num":9007199254740992,"den":1}"#,
            r#"{"num":4000000}"#,
            r#"{"num":4000000,"den":1,"x":0}"#,
            r#"{"num":4000000,"num":4000000,"den":1}"#,
            r#"[4000000,1]"#,
        ] {
            assert!(serde_json::from_str::<Grid>(bad).is_err(), "{bad}");
        }
    }

    fn grids() -> impl Strategy<Value = Grid> {
        prop_oneof![
            Just(Grid::QET_10PX),
            Just(Grid::QET_1PX),
            Just(Grid::MM4),
            (1u64..1_000_000_000, 1u64..1_000).prop_filter_map("too fine", |(n, d)| Grid::new(n, d).ok()),
        ]
    }

    proptest! {
        #[test]
        fn snap_inverts_at(g in grids(), k in -1_000_000_000i64..1_000_000_000) {
            let x = g.at(k).unwrap();
            prop_assert_eq!(g.snap(x), k);
        }

        /// The named grids over a wide index range (up to ±2 × 10¹² steps of 4 mm = ±8 × 10⁹ m).
        #[test]
        fn snap_inverts_at_wide(k in -2_000_000_000_000i64..2_000_000_000_000) {
            for g in [Grid::QET_10PX, Grid::QET_1PX, Grid::MM4] {
                prop_assert_eq!(g.snap(g.at(k).unwrap()), k);
            }
        }

        /// |at(k) − k·num/den| ≤ 0.5 nm, by cross-multiplication: |2·at·den − 2·k·num| ≤ den.
        #[test]
        fn at_error_at_most_half_nm(g in grids(), k in -1_000_000_000i64..1_000_000_000) {
            let x = i128::from(g.at(k).unwrap().0);
            let (num, den) = (i128::from(g.num()), i128::from(g.den()));
            prop_assert!((2 * x * den - 2 * i128::from(k) * num).abs() <= den);
        }

        /// The snapped index is the nearest one: no other index is strictly closer.
        #[test]
        fn snap_is_nearest(g in grids(), x in -1_000_000_000_000i64..1_000_000_000_000) {
            let k = g.snap(Nm(x));
            let (num, den, x) = (i128::from(g.num()), i128::from(g.den()), i128::from(x));
            let dist = |k: i128| (k * num - x * den).abs();
            let k = i128::from(k);
            prop_assert!(dist(k) <= dist(k - 1));
            prop_assert!(dist(k) <= dist(k + 1));
            // Ties go toward zero.
            if dist(k) == dist(k - 1) { prop_assert!(k <= 0); }
            if dist(k) == dist(k + 1) { prop_assert!(k >= 0); }
        }

        /// Cell `j` is [origin + j·w, origin + (j+1)·w).
        #[test]
        fn cell_is_half_open(g in grids(), x in -1_000_000_000_000i64..1_000_000_000_000, o in -1_000_000i64..1_000_000) {
            let j = i128::from(g.cell(Nm(x), Nm(o)));
            let (num, den) = (i128::from(g.num()), i128::from(g.den()));
            let rel = (i128::from(x) - i128::from(o)) * den;
            prop_assert!(j * num <= rel);
            prop_assert!(rel < (j + 1) * num);
        }

        #[test]
        fn serde_round_trip(g in grids()) {
            let s = serde_json::to_string(&g).unwrap();
            prop_assert_eq!(serde_json::from_str::<Grid>(&s).unwrap(), g);
        }
    }
}
