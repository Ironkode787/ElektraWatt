// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Dec3`: fixed-point decimals with three fraction digits.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{UnitsError, arith};

/// A fixed-point decimal with three fraction digits, stored as `i64` thousandths (ADR-0005).
///
/// Holds cross-sections (mm²), currents (A), voltages (V), frequencies (Hz), pressures, temperatures,
/// masses, losses and part quantities, so sums and comparisons in checks are exact. The unit is not part
/// of this type: `Quantity { value: Dec3, unit }`, `Csa` and the unit set are deferred to `ew-schema`
/// (WP-S1.02). Rendering for people ("0,75 mm²" in nl and de) is a query, never stored text.
///
/// # Canonical text form (doc 05 §3.2)
///
/// At most three fraction digits, no trailing fraction zeros (and no `.` without digits), no leading
/// zeros, no `+`, no `-0`, no exponent, no white space, never a unit: `"0.75"`, `"40"`, `"-0.5"`, `"1"`.
/// [`Display`](fmt::Display) writes this form; [`FromStr`] accepts **only** this form.
/// [`parse_lenient`](Dec3::parse_lenient) accepts common variants for importers.
///
/// # Arithmetic
///
/// The `checked_*` methods return `None` on overflow (and division by zero). Products and quotients are
/// rounded to the nearest thousandth with exact ties **toward zero**.
///
/// **JSON:** the canonical string; deserialising is strict and rejects JSON numbers.
///
/// ```
/// use ew_units::Dec3;
///
/// let a: Dec3 = "0.75".parse().unwrap();
/// assert_eq!(a, Dec3(750));
/// assert_eq!(Dec3(40_000).to_string(), "40");
/// assert!("0.750".parse::<Dec3>().is_err());
/// assert_eq!(Dec3::parse_lenient(" +0,750 "), Ok(Dec3(750)));
/// assert_eq!(a.checked_mul(Dec3(2_000)), Some(Dec3(1_500)));
/// assert_eq!(serde_json::to_string(&a).unwrap(), "\"0.75\"");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Dec3(pub i64);

/// The pieces of a decimal string: `[+-]? digits* ([.,] digits*)?` with at least one digit.
struct Parts<'a> {
    sign: Option<u8>,
    int: &'a [u8],
    sep: Option<u8>,
    frac: &'a [u8],
}

fn split_digits(b: &[u8]) -> (&[u8], &[u8]) {
    let n = b.iter().take_while(|c| c.is_ascii_digit()).count();
    b.split_at(n)
}

fn tokenize(s: &str) -> Option<Parts<'_>> {
    let b = s.as_bytes();
    let (sign, b) = match b.split_first() {
        Some((&c @ (b'+' | b'-'), rest)) => (Some(c), rest),
        _ => (None, b),
    };
    let (int, b) = split_digits(b);
    let (sep, frac, b) = match b.split_first() {
        Some((&c @ (b'.' | b','), rest)) => {
            let (frac, rest) = split_digits(rest);
            (Some(c), frac, rest)
        }
        _ => (None, &b[..0], b),
    };
    if !b.is_empty() || (int.is_empty() && frac.is_empty()) {
        return None;
    }
    Some(Parts { sign, int, sep, frac })
}

/// The value of tokenised parts in thousandths; fraction digits beyond the third round half toward zero.
fn value(p: &Parts<'_>) -> Result<i64, UnitsError> {
    // Any integer part above 10¹⁶ is out of range (i64::MAX / 1000 ≈ 9.2 × 10¹⁵); stopping there keeps
    // the accumulator far inside i128 for arbitrarily long input.
    const INT_CUTOFF: i128 = 10_000_000_000_000_000;
    let mut m: i128 = 0;
    for &d in p.int {
        m = m * 10 + i128::from(d - b'0');
        if m > INT_CUTOFF {
            return Err(UnitsError::DecimalOverflow);
        }
    }
    let digit = |i: usize| p.frac.get(i).map_or(0, |&d| i128::from(d - b'0'));
    m = m * 1000 + digit(0) * 100 + digit(1) * 10 + digit(2);
    if let Some((&first, tail)) = p.frac.get(3..).and_then(<[u8]>::split_first) {
        // Strictly more than half a thousandth rounds away from zero; exactly half stays (toward zero).
        if first > b'5' || (first == b'5' && tail.iter().any(|&d| d != b'0')) {
            m += 1;
        }
    }
    let v = if p.sign == Some(b'-') { -m } else { m };
    i64::try_from(v).map_err(|_| UnitsError::DecimalOverflow)
}

fn is_canonical(p: &Parts<'_>, v: i64) -> bool {
    let sign_ok = match p.sign {
        None => true,
        Some(b'-') => v != 0,
        Some(_) => false,
    };
    let int_ok = !p.int.is_empty() && (p.int.len() == 1 || p.int[0] != b'0');
    let frac_ok = match p.sep {
        None => true,
        Some(b'.') => (1..=3).contains(&p.frac.len()) && p.frac.last() != Some(&b'0'),
        Some(_) => false,
    };
    sign_ok && int_ok && frac_ok
}

impl Dec3 {
    /// Zero.
    pub const ZERO: Dec3 = Dec3(0);
    /// One (1000 thousandths).
    pub const ONE: Dec3 = Dec3(1_000);

    /// The integer `n`, or `None` if `n × 1000` overflows.
    pub const fn from_int(n: i64) -> Option<Dec3> {
        match n.checked_mul(1_000) {
            Some(v) => Some(Dec3(v)),
            None => None,
        }
    }

    /// Lenient parsing for importers. Accepts, beyond the canonical form: surrounding white space, a
    /// leading `+`, leading zeros, trailing fraction zeros, a decimal comma instead of the point, a missing
    /// integer part (`".5"`) or fraction (`"5."`), and `-0`. A fourth or later fraction digit is **rounded**
    /// to the nearest thousandth with exact ties toward zero (`"0.0005"` → 0, `"0.00051"` → 0.001).
    ///
    /// Still rejected: exponents, thousands separators, units, more than one sign or separator, and any
    /// other character.
    ///
    /// # Errors
    ///
    /// [`UnitsError::InvalidDecimal`] for malformed input, [`UnitsError::DecimalOverflow`] for values
    /// outside the `i64` range of thousandths.
    pub fn parse_lenient(s: &str) -> Result<Dec3, UnitsError> {
        let p = tokenize(s.trim()).ok_or(UnitsError::InvalidDecimal)?;
        value(&p).map(Dec3)
    }

    /// `self + rhs`, or `None` on overflow.
    pub const fn checked_add(self, rhs: Dec3) -> Option<Dec3> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Dec3(v)),
            None => None,
        }
    }

    /// `self − rhs`, or `None` on overflow.
    pub const fn checked_sub(self, rhs: Dec3) -> Option<Dec3> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Dec3(v)),
            None => None,
        }
    }

    /// `−self`, or `None` for the minimum value.
    pub const fn checked_neg(self) -> Option<Dec3> {
        match self.0.checked_neg() {
            Some(v) => Some(Dec3(v)),
            None => None,
        }
    }

    /// `self × n` for an integer `n` (exact), or `None` on overflow.
    pub const fn checked_mul_int(self, n: i64) -> Option<Dec3> {
        match self.0.checked_mul(n) {
            Some(v) => Some(Dec3(v)),
            None => None,
        }
    }

    /// `self × rhs`, rounded to the nearest thousandth with exact ties toward zero; `None` on overflow.
    ///
    /// ```
    /// use ew_units::Dec3;
    ///
    /// // 0.005 × 0.1 = 0.0005: a tie, rounded toward zero.
    /// assert_eq!(Dec3(5).checked_mul(Dec3(100)), Some(Dec3(0)));
    /// assert_eq!(Dec3(-5).checked_mul(Dec3(100)), Some(Dec3(0)));
    /// // 0.015 × 0.1 = 0.0015: a tie, rounded toward zero to 0.001.
    /// assert_eq!(Dec3(15).checked_mul(Dec3(100)), Some(Dec3(1)));
    /// assert_eq!(Dec3(750).checked_mul(Dec3(1_500)), Some(Dec3(1_125)));
    /// ```
    pub fn checked_mul(self, rhs: Dec3) -> Option<Dec3> {
        let v = arith::div_round(i128::from(self.0) * i128::from(rhs.0), 1_000);
        i64::try_from(v).ok().map(Dec3)
    }

    /// `self ÷ rhs`, rounded to the nearest thousandth with exact ties toward zero; `None` if `rhs` is
    /// zero or the quotient overflows.
    ///
    /// ```
    /// use ew_units::Dec3;
    ///
    /// assert_eq!(Dec3(1_000).checked_div(Dec3(3_000)), Some(Dec3(333)));
    /// assert_eq!(Dec3(2_000).checked_div(Dec3(3_000)), Some(Dec3(667)));
    /// assert_eq!(Dec3(1_000).checked_div(Dec3::ZERO), None);
    /// ```
    pub fn checked_div(self, rhs: Dec3) -> Option<Dec3> {
        if rhs.0 == 0 {
            return None;
        }
        let v = arith::div_round(i128::from(self.0) * 1_000, i128::from(rhs.0));
        i64::try_from(v).ok().map(Dec3)
    }
}

impl fmt::Display for Dec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let a = self.0.unsigned_abs();
        let (int, mut frac) = (a / 1000, a % 1000);
        if self.0 < 0 {
            f.write_str("-")?;
        }
        write!(f, "{int}")?;
        if frac != 0 {
            let mut width = 3;
            while frac % 10 == 0 {
                frac /= 10;
                width -= 1;
            }
            write!(f, ".{frac:0width$}")?;
        }
        Ok(())
    }
}

impl FromStr for Dec3 {
    type Err = UnitsError;

    /// Strict parsing: accepts only the canonical form.
    ///
    /// # Errors
    ///
    /// [`UnitsError::NonCanonicalDecimal`] for input that [`Dec3::parse_lenient`] would accept,
    /// [`UnitsError::DecimalOverflow`] for out-of-range values, [`UnitsError::InvalidDecimal`] otherwise.
    fn from_str(s: &str) -> Result<Dec3, UnitsError> {
        match tokenize(s) {
            Some(p) => {
                let v = value(&p)?;
                if is_canonical(&p, v) {
                    Ok(Dec3(v))
                } else {
                    Err(UnitsError::NonCanonicalDecimal)
                }
            }
            None if tokenize(s.trim()).is_some() => Err(UnitsError::NonCanonicalDecimal),
            None => Err(UnitsError::InvalidDecimal),
        }
    }
}

impl Serialize for Dec3 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Dec3 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(Dec3Visitor)
    }
}

struct Dec3Visitor;

impl Visitor<'_> for Dec3Visitor {
    type Value = Dec3;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a canonical decimal string such as \"0.75\"")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Dec3, E> {
        v.parse().map_err(E::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn strict(s: &str) -> Result<Dec3, UnitsError> {
        s.parse()
    }

    #[test]
    fn display_is_canonical() {
        for (v, s) in [
            (0, "0"),
            (750, "0.75"),
            (40_000, "40"),
            (1_000, "1"),
            (1, "0.001"),
            (10, "0.01"),
            (100, "0.1"),
            (-500, "-0.5"),
            (-1, "-0.001"),
            (1_234_567, "1234.567"),
            (i64::MAX, "9223372036854775.807"),
            (i64::MIN, "-9223372036854775.808"),
        ] {
            assert_eq!(Dec3(v).to_string(), s);
            assert_eq!(strict(s), Ok(Dec3(v)), "{s}");
        }
    }

    #[test]
    fn strict_rejects_non_canonical() {
        for s in [
            "0.750", "1.0", "1.", "01", "00", "+1", "-0", "-0.0", "0,75", " 1", "1 ", ".5", "0.0005",
            "1.2345",
        ] {
            assert_eq!(strict(s), Err(UnitsError::NonCanonicalDecimal), "{s}");
        }
        for s in [
            "", "-", "+", ".", "1e3", "1E3", "1.5.2", "1,5,2", "--1", "1 A", "0x10", "١", "1_000", "1'000",
            "NaN",
        ] {
            assert_eq!(strict(s), Err(UnitsError::InvalidDecimal), "{s:?}");
        }
        for s in [
            "9223372036854775.808",
            "-9223372036854775.809",
            "99999999999999999999999999999999",
        ] {
            assert_eq!(strict(s), Err(UnitsError::DecimalOverflow), "{s}");
        }
    }

    #[test]
    fn lenient_variants() {
        for (s, v) in [
            ("0.75", 750),
            (" +0,750 ", 750),
            ("\t007.5\n", 7_500),
            ("-0", 0),
            ("-0,000", 0),
            (".5", 500),
            ("5.", 5_000),
            ("1.2345", 1_234),
            ("1.2355", 1_235),
            ("1.23551", 1_236),
            ("1.2356", 1_236),
            ("0.0005", 0),
            ("0.00050000", 0),
            ("0.00050001", 1),
            ("-0.0005", 0),
            ("-0.0006", -1),
            ("-1.2355", -1_235),
            ("9223372036854775.8074999", i64::MAX),
        ] {
            assert_eq!(Dec3::parse_lenient(s), Ok(Dec3(v)), "{s:?}");
        }
        for s in [
            "", "  ", "+", "1e3", "1.5.2", "1 000", "1 A", "+-1", "1,000.5", "abc",
        ] {
            assert_eq!(Dec3::parse_lenient(s), Err(UnitsError::InvalidDecimal), "{s:?}");
        }
        // A tie at the limit rounds toward zero and stays in range; anything above overflows.
        assert_eq!(Dec3::parse_lenient("9223372036854775.8075"), Ok(Dec3(i64::MAX)));
        assert_eq!(
            Dec3::parse_lenient("9223372036854775.8076"),
            Err(UnitsError::DecimalOverflow)
        );
    }

    #[test]
    fn arithmetic() {
        assert_eq!(Dec3::from_int(6), Some(Dec3(6_000)));
        assert_eq!(Dec3::from_int(i64::MAX), None);
        assert_eq!(Dec3(750).checked_add(Dec3(1_500)), Some(Dec3(2_250)));
        assert_eq!(Dec3(i64::MAX).checked_add(Dec3(1)), None);
        assert_eq!(Dec3(i64::MIN).checked_sub(Dec3(1)), None);
        assert_eq!(Dec3(i64::MIN).checked_neg(), None);
        assert_eq!(Dec3(6_000).checked_mul_int(3), Some(Dec3(18_000)));
        assert_eq!(Dec3(i64::MAX).checked_mul_int(2), None);
        // Products: 0.75 × 1.5 = 1.125; 0.005 × 0.1 = 0.0005 (tie → 0); 0.015 × 0.1 = 0.0015 (tie → 0.001).
        assert_eq!(Dec3(750).checked_mul(Dec3(1_500)), Some(Dec3(1_125)));
        assert_eq!(Dec3(5).checked_mul(Dec3(100)), Some(Dec3(0)));
        assert_eq!(Dec3(15).checked_mul(Dec3(100)), Some(Dec3(1)));
        assert_eq!(Dec3(-15).checked_mul(Dec3(100)), Some(Dec3(-1)));
        assert_eq!(Dec3(16).checked_mul(Dec3(100)), Some(Dec3(2)));
        assert_eq!(Dec3(i64::MAX).checked_mul(Dec3(2_000)), None);
        // Quotients: 1 / 8 = 0.125; 0.001 / 2 = 0.0005 (tie → 0); 0.003 / 2 = 0.0015 (tie → 0.001).
        assert_eq!(Dec3(1_000).checked_div(Dec3(8_000)), Some(Dec3(125)));
        assert_eq!(Dec3(1).checked_div(Dec3(2_000)), Some(Dec3(0)));
        assert_eq!(Dec3(3).checked_div(Dec3(2_000)), Some(Dec3(1)));
        assert_eq!(Dec3(-3).checked_div(Dec3(2_000)), Some(Dec3(-1)));
        assert_eq!(Dec3(3).checked_div(Dec3(-2_000)), Some(Dec3(-1)));
        assert_eq!(Dec3(1).checked_div(Dec3::ZERO), None);
        assert_eq!(Dec3(i64::MAX).checked_div(Dec3(1)), None);
        assert!(Dec3(1_500) > Dec3(750));
        assert_eq!(Dec3::default(), Dec3::ZERO);
        assert_eq!(Dec3::ONE.to_string(), "1");
    }

    #[test]
    fn json_encoding() {
        assert_eq!(serde_json::to_string(&Dec3(750)).unwrap(), "\"0.75\"");
        assert_eq!(serde_json::to_string(&Dec3(40_000)).unwrap(), "\"40\"");
        assert_eq!(serde_json::from_str::<Dec3>("\"1\"").unwrap(), Dec3(1_000));
        for bad in [
            "0.75",
            "1",
            "\"0.750\"",
            "\"+1\"",
            "\"-0\"",
            "\"1e3\"",
            "\"0,75\"",
            "null",
        ] {
            assert!(serde_json::from_str::<Dec3>(bad).is_err(), "{bad}");
        }
    }

    proptest! {
        #[test]
        fn display_parse_round_trip(v in any::<i64>()) {
            let s = Dec3(v).to_string();
            prop_assert_eq!(strict(&s), Ok(Dec3(v)));
            prop_assert_eq!(Dec3::parse_lenient(&s), Ok(Dec3(v)));
        }

        #[test]
        fn canonical_strings_round_trip(s in "-?(0|[1-9][0-9]{0,14})(\\.[0-9]{0,2}[1-9])?") {
            prop_assume!(s != "-0");
            let d = strict(&s).unwrap();
            prop_assert_eq!(d.to_string(), s);
        }

        #[test]
        fn serde_round_trip(v in any::<i64>()) {
            let s = serde_json::to_string(&Dec3(v)).unwrap();
            prop_assert_eq!(serde_json::from_str::<Dec3>(&s).unwrap(), Dec3(v));
        }

        /// Lenient parsing of decorated canonical text yields the same value.
        #[test]
        fn lenient_accepts_variants(v in -1_000_000_000_000i64..1_000_000_000_000, zeros in 0usize..4, plus in any::<bool>(), comma in any::<bool>()) {
            let a = v.unsigned_abs();
            let sign = if v < 0 { "-" } else if plus { "+" } else { "" };
            let sep = if comma { "," } else { "." };
            let s = format!("  {sign}{}{}{sep}{:03}{} ", "0".repeat(zeros), a / 1000, a % 1000, "0".repeat(zeros));
            prop_assert_eq!(Dec3::parse_lenient(&s), Ok(Dec3(v)));
        }

        /// Lenient rounding of extra fraction digits matches an exact reference.
        #[test]
        fn lenient_rounding_matches_reference(v in -1_000_000_000i64..1_000_000_000, extra in 0u32..10_000) {
            // The decimal v/1000 followed by four more digits `extra`: exact value (v·10⁴ ± extra) / 10⁷.
            let a = v.unsigned_abs();
            let sign = if v < 0 { "-" } else { "" };
            let s = format!("{sign}{}.{:03}{:04}", a / 1000, a % 1000, extra);
            let exact = (i128::from(a) * 10_000 + i128::from(extra)) * if v < 0 { -1 } else { 1 };
            let expected = arith::div_round(exact, 10_000);
            prop_assert_eq!(Dec3::parse_lenient(&s), Ok(Dec3(expected as i64)));
        }

        #[test]
        fn mul_div_match_reference(a in -1_000_000_000_000i64..1_000_000_000_000, b in -1_000_000i64..1_000_000) {
            let p = Dec3(a).checked_mul(Dec3(b)).unwrap().0;
            let exact = i128::from(a) * i128::from(b); // in 10⁻⁶
            prop_assert!((i128::from(p) * 1_000 - exact).abs() * 2 <= 1_000);
            if b != 0 {
                let q = Dec3(a).checked_div(Dec3(b)).unwrap().0;
                let num = i128::from(a) * 1_000;
                prop_assert!((i128::from(q) * i128::from(b) - num).abs() * 2 <= i128::from(b).abs());
            }
        }
    }
}
