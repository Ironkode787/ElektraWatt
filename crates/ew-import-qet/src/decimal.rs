// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact decimal numbers from QET attributes and their conversion to nanometres and angles (doc 14 §4.2).
//!
//! Qt writes coordinates with `QString::number`, so `12.5`, `-3.33333` and `1.23457e+06` all occur. A
//! [`Decimal`] holds such a string **exactly** as `mantissa × 10^exp`; arithmetic on it (`checked_add`, `checked_sub`,
//! `checked_mul`, `half`) is exact, and the conversions round once, half to even, in `i128`:
//!
//! - pixels to nanometres: `nm = round_half_even(m × 793_750 / (3 × 10^−e))` ([`Decimal::px_to_nm`]),
//!   with 1 QET px = 25.4 / 96 mm = 793 750 / 3 nm;
//! - degrees to [`Angle2`] (10⁻³ degree): `round_half_even(m × 10^(e + 3))` ([`Decimal::deg_to_angle`]).
//!
//! No value ever passes through `f64`. A point on the 10 px grid relative to an on-grid hotspot therefore
//! lands exactly on `Grid::QET_10PX.at(k)`: `10k px` is `k × 7_937_500 / 3` nm, whose fraction is 0, ⅓ or
//! ⅔, never a tie, so half-to-even here and ties-toward-zero in `Grid::at` agree (FR-IMP-008 "0 nm
//! deviation" as an equality on grid indices).
//!
//! QET stores primitive coordinates relative to the element's hotspot already. Wherever a coordinate must
//! be made relative to another origin, the subtraction happens on the exact decimals **before** conversion
//! (`x.checked_sub(origin).px_to_nm()`), so there is one rounding; rounding both and subtracting could be
//! off by 1 nm.

use std::fmt;

use ew_units::{Angle2, MAX_SAFE_INTEGER, Nm};

/// Nanometres per QET pixel, numerator: 25.4 mm / 96 = 793 750 / 3 nm.
const PX_NUM: i128 = 793_750;
/// Nanometres per QET pixel, denominator.
const PX_DEN: i128 = 3;
/// The literal exponent of [`Decimal::parse`] saturates here.
const EXP_LITERAL_MAX: i64 = 1_000_000;
/// Mantissas of parsed decimals are below this: 32 significant digits.
const MAX_MANTISSA: i128 = 10i128.pow(32);

/// Why a string is not a usable decimal.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DecimalError {
    /// Not of the form `[+-]digits[.digits][e[+-]digits]` (this includes `nan`, `inf` and the empty
    /// string).
    Syntax,
    /// More than 32 significant digits (counted from the first to the last non-zero digit).
    TooLong,
    /// The converted value is outside ±(2⁵³ − 1) nm or the `i32` range of [`Angle2`].
    OutOfRange,
}

impl fmt::Display for DecimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DecimalError::Syntax => "not a decimal number",
            DecimalError::TooLong => "too many significant digits",
            DecimalError::OutOfRange => "value out of range",
        })
    }
}

impl std::error::Error for DecimalError {}

/// An exact decimal `mantissa × 10^exp`, normalised so that the mantissa has no trailing zero digit (zero
/// is `0 × 10^0`). Two equal values therefore compare equal.
///
/// ```
/// # use ew_import_qet::decimal::Decimal;
/// let d = Decimal::parse("1.23457e+06").unwrap();
/// assert_eq!((d.mantissa(), d.exp()), (123_457, 1));
/// assert_eq!(Decimal::parse("12.50").unwrap(), Decimal::parse("125e-1").unwrap());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Decimal {
    mantissa: i128,
    exp: i32,
}

impl Decimal {
    /// Zero.
    pub const ZERO: Decimal = Decimal { mantissa: 0, exp: 0 };

    /// `mantissa × 10^exp`, normalised. The exponent saturates at `i32::MAX` (only reachable with an
    /// exponent within a few digits of it).
    pub fn new(mantissa: i128, exp: i32) -> Decimal {
        let (mut m, mut e) = (mantissa, exp);
        if m == 0 {
            return Decimal::ZERO;
        }
        while m % 10 == 0 {
            m /= 10;
            e = e.saturating_add(1);
        }
        Decimal { mantissa: m, exp: e }
    }

    /// An integer.
    pub fn from_int(n: i64) -> Decimal {
        Decimal::new(i128::from(n), 0)
    }

    /// The mantissa.
    pub const fn mantissa(self) -> i128 {
        self.mantissa
    }

    /// The power of ten.
    pub const fn exp(self) -> i32 {
        self.exp
    }

    /// Whether the value is zero.
    pub const fn is_zero(self) -> bool {
        self.mantissa == 0
    }

    /// Whether the value is below zero.
    pub const fn is_negative(self) -> bool {
        self.mantissa < 0
    }

    /// Parses `[+-]digits[.digits][(e|E)[+-]digits]` with at least one digit before the exponent;
    /// surrounding ASCII white space is ignored. This is the output grammar of `QString::number` (and of
    /// hand-edited files with a leading `+` or `.5`).
    ///
    /// The value is held exactly. Leading zeros cost nothing, and trailing zeros of the integer or fraction
    /// part are folded into the exponent (or dropped), so neither counts towards the length limit: at most
    /// **32 significant digits** (from the first to the last non-zero digit) are accepted. With
    /// |m| < 10³², `m × 793_750` and `m × 1000` always fit `i128`, so the conversions never overflow for a
    /// negative exponent and their "10^k ≥ 10^38 rounds to zero" shortcut stays exact. (`QString::number`
    /// writes at most 17 significant digits.)
    ///
    /// The exponent literal saturates at ±1 000 000. That changes no conversion result for any input
    /// shorter than a million characters: such values are out of range or round to zero either way. A total
    /// exponent that does not fit `i32` gives zero when negative (every conversion rounds it to zero) and
    /// [`DecimalError::OutOfRange`] when positive. Any other exponent is accepted; whether the converted
    /// value is in range is decided by [`Decimal::px_to_nm`] and [`Decimal::deg_to_angle`], so values below
    /// 0.5 nm are zero, not errors.
    ///
    /// # Errors
    ///
    /// [`DecimalError::Syntax`] for anything else, [`DecimalError::TooLong`] for more than 32 significant
    /// digits, [`DecimalError::OutOfRange`] for a non-zero value whose exponent exceeds `i32::MAX`.
    pub fn parse(s: &str) -> Result<Decimal, DecimalError> {
        let s = s.trim_matches(|c: char| c.is_ascii_whitespace());
        let b = s.as_bytes();
        let mut i = 0;
        let negative = match b.first() {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let mut acc = Digits::default();
        let mut digits = 0usize;
        while i < b.len() && b[i].is_ascii_digit() {
            acc.push(b[i] - b'0', false)?;
            digits += 1;
            i += 1;
        }
        // Zeros at the end of the integer part are worth a power of ten, unless a fraction digit follows.
        acc.held_int = acc.pending;
        if i < b.len() && b[i] == b'.' {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                acc.push(b[i] - b'0', true)?;
                digits += 1;
                i += 1;
            }
        }
        if digits == 0 {
            return Err(DecimalError::Syntax);
        }
        let mut literal: i64 = 0;
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            i += 1;
            let exp_negative = match b.get(i) {
                Some(b'-') => {
                    i += 1;
                    true
                }
                Some(b'+') => {
                    i += 1;
                    false
                }
                _ => false,
            };
            let start = i;
            let mut e: i64 = 0;
            while i < b.len() && b[i].is_ascii_digit() {
                e = (e * 10 + i64::from(b[i] - b'0')).min(EXP_LITERAL_MAX);
                i += 1;
            }
            if i == start {
                return Err(DecimalError::Syntax);
            }
            literal = if exp_negative { -e } else { e };
        }
        if i != b.len() {
            return Err(DecimalError::Syntax);
        }
        if acc.mantissa == 0 {
            return Ok(Decimal::ZERO);
        }
        let mut m = if negative { -acc.mantissa } else { acc.mantissa };
        let mut exp = acc.exp + literal;
        exp += i64::try_from(acc.held_int).map_err(|_| DecimalError::TooLong)?;
        while m % 10 == 0 {
            m /= 10;
            exp += 1;
        }
        match i32::try_from(exp) {
            Ok(exp) => Ok(Decimal { mantissa: m, exp }),
            Err(_) if exp < 0 => Ok(Decimal::ZERO),
            Err(_) => Err(DecimalError::OutOfRange),
        }
    }

    /// The two values brought to the common (smaller) exponent, or `None` on overflow.
    fn aligned(self, other: Decimal) -> Option<(i128, i128, i32)> {
        let e = self.exp.min(other.exp);
        let scale = |d: Decimal| -> Option<i128> {
            let shift = u32::try_from(d.exp.checked_sub(e)?).ok()?;
            d.mantissa.checked_mul(10i128.checked_pow(shift)?)
        };
        Some((scale(self)?, scale(other)?, e))
    }

    /// `self + other`, exactly. If either operand is zero the other is returned as it is, whatever the
    /// exponent gap.
    ///
    /// # Errors
    ///
    /// [`DecimalError::OutOfRange`] if the exact sum needs more digits than an `i128` holds.
    pub fn checked_add(self, other: Decimal) -> Result<Decimal, DecimalError> {
        if other.is_zero() {
            return Ok(self);
        }
        if self.is_zero() {
            return Ok(other);
        }
        let (a, b, e) = self.aligned(other).ok_or(DecimalError::OutOfRange)?;
        Ok(Decimal::new(a.checked_add(b).ok_or(DecimalError::OutOfRange)?, e))
    }

    /// `self − other`, exactly. `x − 0` is `x` for any exponent (the path of a coordinate made relative to
    /// an origin of zero).
    ///
    /// # Errors
    ///
    /// [`DecimalError::OutOfRange`] if the exact difference needs more digits than an `i128` holds.
    pub fn checked_sub(self, other: Decimal) -> Result<Decimal, DecimalError> {
        if other.is_zero() {
            return Ok(self);
        }
        let negated = Decimal {
            mantissa: other.mantissa.checked_neg().ok_or(DecimalError::OutOfRange)?,
            exp: other.exp,
        };
        self.checked_add(negated)
    }

    /// `self × other`, exactly.
    ///
    /// # Errors
    ///
    /// [`DecimalError::OutOfRange`] on overflow.
    pub fn checked_mul(self, other: Decimal) -> Result<Decimal, DecimalError> {
        let m = self
            .mantissa
            .checked_mul(other.mantissa)
            .ok_or(DecimalError::OutOfRange)?;
        let exp = self.exp.checked_add(other.exp).ok_or(DecimalError::OutOfRange)?;
        Ok(Decimal::new(m, exp))
    }

    /// `self / 2`, exactly (`m × 5 × 10^(e − 1)`).
    ///
    /// # Errors
    ///
    /// [`DecimalError::OutOfRange`] on overflow.
    pub fn half(self) -> Result<Decimal, DecimalError> {
        let m = self.mantissa.checked_mul(5).ok_or(DecimalError::OutOfRange)?;
        let exp = self.exp.checked_sub(1).ok_or(DecimalError::OutOfRange)?;
        Ok(Decimal::new(m, exp))
    }

    /// `m × num / den` rounded half to even, for `num`, `den` > 0; `None` on overflow.
    fn scaled_round(self, num: i128, den: i128) -> Option<i128> {
        if self.exp >= 0 {
            let p = 10i128.checked_pow(u32::try_from(self.exp).ok()?)?;
            let n = self.mantissa.checked_mul(num)?.checked_mul(p)?;
            Some(div_round_half_even(n, den))
        } else {
            let k = self.exp.unsigned_abs();
            let n = self.mantissa.checked_mul(num)?;
            // |m × num| < 10^38: with 10^k ≥ 10^38 the quotient is below one half and rounds to zero.
            if k >= 38 {
                return Some(0);
            }
            let d = den.checked_mul(10i128.pow(k))?;
            Some(div_round_half_even(n, d))
        }
    }

    /// The value as QET pixels converted to nanometres: `round_half_even(m × 793_750 / (3 × 10^−e))`.
    ///
    /// ```
    /// # use ew_import_qet::decimal::Decimal;
    /// use ew_units::{Grid, Nm};
    ///
    /// let px = |s| Decimal::parse(s).unwrap().px_to_nm().unwrap();
    /// assert_eq!(px("10"), Grid::QET_10PX.at(1).unwrap());
    /// assert_eq!(px("-30"), Grid::QET_10PX.at(-3).unwrap());
    /// assert_eq!(px("1.5"), Nm(396_875));
    /// ```
    ///
    /// # Errors
    ///
    /// [`DecimalError::OutOfRange`] if the result exceeds ±(2⁵³ − 1) nm (doc 05 §3.2).
    pub fn px_to_nm(self) -> Result<Nm, DecimalError> {
        let nm = self
            .scaled_round(PX_NUM, PX_DEN)
            .ok_or(DecimalError::OutOfRange)?;
        if nm.abs() > i128::from(MAX_SAFE_INTEGER) {
            return Err(DecimalError::OutOfRange);
        }
        i64::try_from(nm).map(Nm).map_err(|_| DecimalError::OutOfRange)
    }

    /// The value as degrees converted to 10⁻³ degree, rounded half to even, with the sign kept (QET's
    /// counter-clockwise arc angles map one to one; the caller negates Qt's clockwise text rotations).
    ///
    /// ```
    /// # use ew_import_qet::decimal::Decimal;
    /// use ew_units::Angle2;
    ///
    /// let deg = |s| Decimal::parse(s).unwrap().deg_to_angle().unwrap();
    /// assert_eq!(deg("-90"), Angle2(-90_000));
    /// assert_eq!(deg("12.3455"), Angle2(12_346)); // 12 345.5 → even
    /// assert_eq!(deg("12.3445"), Angle2(12_344));
    /// ```
    ///
    /// # Errors
    ///
    /// [`DecimalError::OutOfRange`] if the result does not fit an `i32`.
    pub fn deg_to_angle(self) -> Result<Angle2, DecimalError> {
        let v = self.scaled_round(1000, 1).ok_or(DecimalError::OutOfRange)?;
        i32::try_from(v).map(Angle2).map_err(|_| DecimalError::OutOfRange)
    }

    /// The value as a plain decimal string without exponent and without trailing zeros: `9`, `9.75`,
    /// `-0.5`, `1234570`. Used for style keys (`qet.9.75`).
    ///
    /// The string has |exp| digits, so callers bound the value first (the font parser bounds point sizes).
    pub fn to_plain_string(self) -> String {
        let neg = self.mantissa < 0;
        let digits = self.mantissa.unsigned_abs().to_string();
        let body = if self.exp >= 0 {
            format!(
                "{digits}{}",
                "0".repeat(usize::try_from(self.exp).unwrap_or_default())
            )
        } else {
            let k = usize::try_from(self.exp.unsigned_abs()).unwrap_or_default();
            if digits.len() > k {
                let (int, frac) = digits.split_at(digits.len() - k);
                format!("{int}.{frac}")
            } else {
                format!("0.{}{digits}", "0".repeat(k - digits.len()))
            }
        };
        if neg { format!("-{body}") } else { body }
    }
}

/// The digits of a literal collected by [`Decimal::parse`]: zeros after the first non-zero digit are held
/// back (`pending`) and multiplied in only when a later non-zero digit arrives.
#[derive(Default)]
struct Digits {
    mantissa: i128,
    /// Power of ten from the fraction digits already multiplied in (zero or negative).
    exp: i64,
    /// Zeros seen since the last non-zero digit.
    pending: usize,
    /// How many of the pending zeros belong to the integer part (set once it ends).
    held_int: usize,
}

impl Digits {
    fn push(&mut self, d: u8, fraction: bool) -> Result<(), DecimalError> {
        if d == 0 {
            if self.mantissa != 0 {
                self.pending = self.pending.saturating_add(1);
            } else if fraction {
                // Leading zero of the fraction: no digit yet, but the exponent moves.
                self.exp -= 1;
            }
            return Ok(());
        }
        let shift = u32::try_from(self.pending.saturating_add(1)).map_err(|_| DecimalError::TooLong)?;
        let m = 10i128
            .checked_pow(shift)
            .and_then(|scale| self.mantissa.checked_mul(scale))
            .and_then(|m| m.checked_add(i128::from(d)))
            .filter(|m| *m < MAX_MANTISSA)
            .ok_or(DecimalError::TooLong)?;
        if fraction {
            // Every fraction digit since the last flush, zeros included, moves the exponent.
            let moved = self.pending - self.held_int + 1;
            self.exp -= i64::try_from(moved).map_err(|_| DecimalError::TooLong)?;
        }
        self.mantissa = m;
        self.pending = 0;
        self.held_int = 0;
        Ok(())
    }
}

/// `n / d` rounded to the nearest integer, exact ties to the even neighbour; `d` must be positive.
///
/// ```
/// # use ew_import_qet::decimal::div_round_half_even;
/// assert_eq!(div_round_half_even(5, 2), 2);
/// assert_eq!(div_round_half_even(7, 2), 4);
/// assert_eq!(div_round_half_even(-5, 2), -2);
/// assert_eq!(div_round_half_even(-7, 3), -2);
/// ```
pub fn div_round_half_even(n: i128, d: i128) -> i128 {
    debug_assert!(d > 0);
    let q = n.div_euclid(d);
    let r = n.rem_euclid(d);
    match (2 * r).cmp(&d) {
        std::cmp::Ordering::Greater => q + 1,
        std::cmp::Ordering::Equal if q % 2 != 0 => q + 1,
        _ => q,
    }
}

/// The integer nearest to `√n` (never a tie, as `√n` is never `k + ½` for an integer `n`).
///
/// ```
/// # use ew_import_qet::decimal::isqrt_round;
/// assert_eq!(isqrt_round(0), 0);
/// assert_eq!(isqrt_round(2), 1);
/// assert_eq!(isqrt_round(3), 2);
/// assert_eq!(isqrt_round(25), 5);
/// ```
pub fn isqrt_round(n: u128) -> u128 {
    let r = n.isqrt();
    if n - r * r > r { r + 1 } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_units::Grid;

    fn d(s: &str) -> Decimal {
        Decimal::parse(s).unwrap()
    }

    #[test]
    fn parses_qt_number_output() {
        assert_eq!(d("12.5"), Decimal::new(125, -1));
        assert_eq!(d("-3.33333"), Decimal::new(-333_333, -5));
        assert_eq!(d("1.23457e+06"), Decimal::new(123_457, 1));
        assert_eq!(d("1E-3"), Decimal::new(1, -3));
        assert_eq!(d("+.5"), Decimal::new(5, -1));
        assert_eq!(d("5."), Decimal::from_int(5));
        assert_eq!(d(" 0 "), Decimal::ZERO);
        assert_eq!(d("-0.000"), Decimal::ZERO);
        assert_eq!(d("0e999999"), Decimal::ZERO);
        assert_eq!(d("100"), Decimal::new(1, 2));
    }

    #[test]
    fn rejects_non_numbers() {
        for bad in [
            "", " ", ".", "-", "e5", "1e", "1e+", "--1", "1.2.3", "nan", "NaN", "inf", "-inf", "0x10", "1,5",
            "1 5", "١",
        ] {
            assert_eq!(Decimal::parse(bad), Err(DecimalError::Syntax), "{bad:?}");
        }
        assert_eq!(Decimal::parse(&"9".repeat(40)), Err(DecimalError::TooLong));
    }

    /// FR-IMP-008: every 10 px grid point lands exactly on the grid position of its index.
    #[test]
    fn grid_points_are_exact() {
        let g = Grid::QET_10PX;
        for k in -2_000i64..=2_000 {
            let s = format!("{}", 10 * k);
            assert_eq!(d(&s).px_to_nm().unwrap(), g.at(k).unwrap(), "{s}");
            assert_eq!(g.snap(d(&s).px_to_nm().unwrap()), k);
        }
        // Relative to an on-grid hotspot, in Qt's exponent form too.
        let hotspot = d("1.2346e+06");
        let x = d("1.23461e+06");
        assert_eq!(
            x.checked_sub(hotspot).unwrap().px_to_nm().unwrap(),
            g.at(1).unwrap()
        );
    }

    #[test]
    fn px_conversion_rounds_half_to_even() {
        // 0.00024 px = 240 × 793 750 / (3 × 10^6) = 63.5 nm: a tie, to 64 (even).
        assert_eq!(d("0.00024").px_to_nm().unwrap(), Nm(64));
        assert_eq!(d("-0.00024").px_to_nm().unwrap(), Nm(-64));
        // 0.00072 px = 190.5 nm → 190.
        assert_eq!(d("0.00072").px_to_nm().unwrap(), Nm(190));
        // 1 px = 264 583.33… → 264 583; 2 px = 529 166.67 → 529 167.
        assert_eq!(d("1").px_to_nm().unwrap(), Nm(264_583));
        assert_eq!(d("2").px_to_nm().unwrap(), Nm(529_167));
        assert_eq!(d("-3.33333").px_to_nm().unwrap(), Nm(-881_944)); // −881 943.5625
        assert_eq!(d("1e-50").px_to_nm().unwrap(), Nm(0));
        assert_eq!(d("1e40").px_to_nm(), Err(DecimalError::OutOfRange));
        assert_eq!(d("4e10").px_to_nm(), Err(DecimalError::OutOfRange));
    }

    #[test]
    fn exact_arithmetic() {
        assert_eq!(d("0.1").checked_add(d("0.2")).unwrap(), d("0.3"));
        assert_eq!(d("12.5").checked_sub(d("2.5")).unwrap(), d("10"));
        assert_eq!(d("13").checked_mul(d("0.75")).unwrap(), d("9.75"));
        assert_eq!(d("7").half().unwrap(), d("3.5"));
        assert_eq!(
            d("1e10").checked_sub(d("1e-10")).unwrap(),
            d("9999999999.9999999999")
        );
    }

    #[test]
    fn angles() {
        assert_eq!(d("45").deg_to_angle().unwrap(), Angle2(45_000));
        assert_eq!(d("-0.0005").deg_to_angle().unwrap(), Angle2(0));
        assert_eq!(d("0.0015").deg_to_angle().unwrap(), Angle2(2));
        assert_eq!(d("3e6").deg_to_angle(), Err(DecimalError::OutOfRange));
    }

    #[test]
    fn plain_strings() {
        assert_eq!(d("9.00").to_plain_string(), "9");
        assert_eq!(d("9.75").to_plain_string(), "9.75");
        assert_eq!(d("-0.05").to_plain_string(), "-0.05");
        assert_eq!(d("1.23457e+06").to_plain_string(), "1234570");
        assert_eq!(Decimal::ZERO.to_plain_string(), "0");
    }

    #[test]
    fn helpers() {
        assert_eq!(div_round_half_even(3, 2), 2);
        assert_eq!(div_round_half_even(-3, 2), -2);
        assert_eq!(div_round_half_even(1, 3), 0);
        assert_eq!(div_round_half_even(2, 3), 1);
        for n in 0u128..2_000 {
            let r = isqrt_round(n);
            // r is the integer nearest to √n: (r − ½)² ≤ n < (r + ½)², i.e. 4r² − 4r + 1 ≤ 4n < 4r² + 4r + 1.
            assert!(r == 0 || 4 * r * r < 4 * n + 4 * r, "{n}");
            assert!(4 * n < 4 * r * r + 4 * r + 1, "{n}");
        }
    }

    #[test]
    fn tiny_values_round_to_zero() {
        let tiny = format!("0.{}1", "0".repeat(70));
        for s in ["1e-61", "-1e-61", tiny.as_str(), "1e-1000000", "1e-99999999999"] {
            let v = Decimal::parse(s).unwrap();
            assert_eq!(v.px_to_nm(), Ok(Nm(0)), "{s}");
            assert_eq!(v.deg_to_angle(), Ok(Angle2(0)), "{s}");
        }
        assert!(
            Decimal::parse("1e-61")
                .unwrap()
                .checked_sub(Decimal::ZERO)
                .is_ok()
        );
        assert_eq!(Decimal::ZERO.checked_sub(d("1e-61")).unwrap(), d("-1e-61"));
    }

    #[test]
    fn huge_values_are_out_of_range_on_conversion() {
        for s in ["1e40", "1e+99999999999"] {
            let v = Decimal::parse(s).unwrap();
            assert_eq!(v.px_to_nm(), Err(DecimalError::OutOfRange), "{s}");
        }
    }

    #[test]
    fn trailing_zeros_do_not_count() {
        assert_eq!(d(&format!("1.{}", "0".repeat(100))), Decimal::from_int(1));
        assert_eq!(d(&format!("1{}e-100", "0".repeat(100))), Decimal::from_int(1));
        let big = d(&format!("10{}", "0".repeat(40)));
        assert_eq!((big.mantissa(), big.exp()), (1, 41));
        assert_eq!(d("10.5"), Decimal::new(105, -1));
        assert_eq!(d("100.005"), Decimal::new(100_005, -3));
    }

    #[test]
    fn significant_digit_limit() {
        let ok = "0.12345678901234567890123456789012";
        assert_eq!(d(ok).mantissa(), 12_345_678_901_234_567_890_123_456_789_012);
        assert!(d(ok).px_to_nm().is_ok());
        assert_eq!(
            Decimal::parse("0.123456789012345678901234567890123"),
            Err(DecimalError::TooLong)
        );
        // 0.000123456… px = 32.66… nm: leading zeros do not count towards the limit.
        assert_eq!(d(&format!("0.000{}", &ok[2..])).px_to_nm(), Ok(Nm(33)));
        assert_eq!(
            Decimal::parse(&format!("1{}1", "0".repeat(31))),
            Err(DecimalError::TooLong)
        );
    }
}
