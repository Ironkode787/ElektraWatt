// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Natural-sort keys: digit runs compare by value, so X2 sorts before X10.

use std::cmp::Ordering;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::canonical_combining_class;
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};

use crate::to_nfc;

// Level-1 element classes, in sort order. 0 ends a level, so a shorter string sorts first.
const END: u8 = 0x00;
const SPACE: u8 = 0x01;
const PUNCTUATION: u8 = 0x02;
const SYMBOL: u8 = 0x03;
const NUMBER: u8 = 0x04;
const LETTER: u8 = 0x05;
// Element marker of levels 2 and 3 (above END, so the prefix rule holds there too).
const CHAR: u8 = 0x01;

/// A byte string whose order (`Ord`, equally `memcmp`) is the natural order of the text it was made from
/// (FR-NUM-018, FR-I18N-010).
///
/// Keys are derived values: they are never persisted, so the byte layout is private to this crate. The
/// **order** is the contract, specified at [`natural_key`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct NaturalKey(Vec<u8>);

impl NaturalKey {
    /// The key bytes, for a byte-wise comparing index or SQL collation.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

fn push_code_point(key: &mut Vec<u8>, c: char) {
    let v = u32::from(c);
    // Code points fit in 21 bits.
    key.extend_from_slice(&v.to_be_bytes()[1..]);
}

/// The level-1 class of a non-digit character, by Unicode general category: separators (Z*), ASCII white
/// space and control characters are white space, P* is punctuation, S* is a symbol, everything else
/// (letters, marks, other numbers such as `²`, unassigned code points) sorts with the letters.
fn class(c: char) -> u8 {
    if c.is_ascii_whitespace() || c.is_control() {
        return SPACE;
    }
    match c.general_category() {
        GeneralCategory::SpaceSeparator
        | GeneralCategory::LineSeparator
        | GeneralCategory::ParagraphSeparator => SPACE,
        GeneralCategory::ConnectorPunctuation
        | GeneralCategory::DashPunctuation
        | GeneralCategory::OpenPunctuation
        | GeneralCategory::ClosePunctuation
        | GeneralCategory::InitialPunctuation
        | GeneralCategory::FinalPunctuation
        | GeneralCategory::OtherPunctuation => PUNCTUATION,
        GeneralCategory::MathSymbol
        | GeneralCategory::CurrencySymbol
        | GeneralCategory::ModifierSymbol
        | GeneralCategory::OtherSymbol => SYMBOL,
        _ => LETTER,
    }
}

/// The decimal value (0-9) of a character of general category Nd, or `None`.
///
/// Unicode encodes every Nd script as contiguous ascending runs of ten digits starting at zero, and
/// adjacent scripts (the mathematical digits) as whole multiples of ten, so the value is the offset from the
/// start of the contiguous Nd run, modulo 10.
fn decimal_value(c: char) -> Option<u8> {
    if let Some(d) = c.to_digit(10) {
        return u8::try_from(d).ok();
    }
    if c.general_category() != GeneralCategory::DecimalNumber {
        return None;
    }
    let mut offset = 0u32;
    let mut cur = u32::from(c);
    while let Some(prev) = cur.checked_sub(1).and_then(char::from_u32) {
        if prev.general_category() != GeneralCategory::DecimalNumber {
            break;
        }
        offset += 1;
        cur -= 1;
    }
    u8::try_from(offset % 10).ok()
}

/// The natural-sort key of `s`, the ElektraWatt natural order (version 1).
///
/// The order is deterministic and independent of the OS locale (NFR-I18N-03): it depends only on the
/// text, Unicode normalisation data and Rust's Unicode lower-case mapping. Strings are compared on three
/// levels; a later level only breaks ties of the earlier ones.
///
/// 1. **Primary.** The NFC text is decomposed (NFD), combining marks are dropped and letters are
///    lower-cased. The result is split into elements: each run of decimal digits (general category Nd,
///    ASCII or any script, mixed runs allowed) is **one number** that compares by its decimal value
///    (leading zeros ignored), every other character is one element. Elements are classed by Unicode
///    general category (separators Z* and ASCII white space are white space, P* punctuation, S* symbols,
///    everything else letters) and compare by class first, `white space < punctuation < symbols < numbers <
///    letters` (the class order of the Unicode collation root: `-` before `+` before digits before
///    letters), then by code point. So the en dash (punctuation) sorts before `+` (a symbol). A string that
///    is a prefix of another sorts first.
/// 2. **Secondary.** The lower-cased NFC text, code point by code point: accents (`A1 < Ä1 < A2`) and
///    leading zeros (`X01 < X1`).
/// 3. **Tertiary.** The NFC text, code point by code point: case, upper case first (`K1 < k1 < K2`).
///
/// Two strings get equal keys exactly when their NFC forms are equal, so the order is total and
/// consistent with identity of identifiers. Case-insensitive *equality* (FR-NUM-017) is a separate
/// project setting, see [`Ident::comparison_key`](crate::Ident::comparison_key).
///
/// ```
/// use ew_mltext::natural_key;
///
/// let mut tags = vec!["X10", "X2", "x1", "X1"];
/// tags.sort_by_key(|t| natural_key(t));
/// assert_eq!(tags, ["X1", "x1", "X2", "X10"]);
/// ```
pub fn natural_key(s: &str) -> NaturalKey {
    let nfc = to_nfc(s);
    let mut key = Vec::with_capacity(nfc.len() * 12 + 2);

    let primary: Vec<char> = nfc
        .chars()
        .nfd()
        .filter(|&c| canonical_combining_class(c) == 0)
        .flat_map(char::to_lowercase)
        .collect();
    let mut i = 0;
    while i < primary.len() {
        let c = primary[i];
        if decimal_value(c).is_some() {
            let mut digits = Vec::new();
            while let Some(d) = primary.get(i).copied().and_then(decimal_value) {
                digits.push(d);
                i += 1;
            }
            let significant = &digits[digits.iter().take_while(|&&d| d == 0).count()..];
            key.push(NUMBER);
            let len = u32::try_from(significant.len()).unwrap_or(u32::MAX);
            key.extend_from_slice(&len.to_be_bytes());
            key.extend(significant.iter().map(|&d| b'0' + d));
        } else {
            key.push(class(c));
            push_code_point(&mut key, c);
            i += 1;
        }
    }
    key.push(END);

    for c in nfc.chars().flat_map(char::to_lowercase) {
        key.push(CHAR);
        push_code_point(&mut key, c);
    }
    key.push(END);

    for c in nfc.chars() {
        key.push(CHAR);
        push_code_point(&mut key, c);
    }
    NaturalKey(key)
}

/// `a` against `b` in natural order: `natural_key(a).cmp(&natural_key(b))`.
///
/// ```
/// use std::cmp::Ordering;
/// use ew_mltext::natural_cmp;
///
/// assert_eq!(natural_cmp("-X1031:2", "-X1031:10"), Ordering::Less);
/// ```
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    natural_key(a).cmp(&natural_key(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sorted(items: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = items.iter().map(|s| (*s).to_owned()).collect();
        v.sort_by_key(|s| natural_key(s));
        v
    }

    #[test]
    fn digit_runs_compare_by_value() {
        assert_eq!(
            sorted(&["X10", "X2", "X1", "X100", "X20", "X3"]),
            ["X1", "X2", "X3", "X10", "X20", "X100"]
        );
        // Terminals of -X1031: 1, 2, … 10, … 20, not 1, 10, 11 … (FR-I18N-010).
        let terminals: Vec<String> = (1..=20).rev().map(|n| n.to_string()).collect();
        let refs: Vec<&str> = terminals.iter().map(String::as_str).collect();
        let expected: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
        assert_eq!(sorted(&refs), expected);
        // Folios in folio order, labels after their number.
        assert_eq!(
            sorted(&["101a", "100", "3", "101.1", "1", "101", "2", "903"]),
            ["1", "2", "3", "100", "101", "101.1", "101a", "903"]
        );
        // Numbers longer than any integer type.
        assert_eq!(
            sorted(&["A123456789012345678901234567890", "A99999999999999999999"]),
            ["A99999999999999999999", "A123456789012345678901234567890"]
        );
    }

    #[test]
    fn golden_tags() {
        assert_eq!(
            sorted(&[
                "-201U9", "-101F1", "-201U10", "-102G1", "-102F2", "-102F1", "-3W1", "-106W1"
            ]),
            [
                "-3W1", "-101F1", "-102F1", "-102F2", "-102G1", "-106W1", "-201U9", "-201U10"
            ]
        );
        assert_eq!(
            sorted(&["-X1031:10", "-X1031:2", "-X1031:1", "-X1031:PE"]),
            ["-X1031:1", "-X1031:2", "-X1031:10", "-X1031:PE"]
        );
    }

    #[test]
    fn class_order() {
        // white space < punctuation (-) < symbols (+) < numbers < letters
        let order = [" ", "-", "+", "0", "9", "A", "Ä", "Z", "Ω"];
        for w in order.windows(2) {
            assert_eq!(natural_cmp(w[0], w[1]), Ordering::Less, "{w:?}");
        }
        assert_eq!(natural_cmp("", " "), Ordering::Less);
        assert_eq!(natural_cmp("_", "+"), Ordering::Less);
        assert_eq!(natural_cmp("+J-E-2416", "-101F1"), Ordering::Greater);
    }

    #[test]
    fn unicode_general_categories() {
        // Non-ASCII punctuation is punctuation, not a symbol: en dash and ellipsis before `+`.
        assert_eq!(natural_cmp("\u{2013}", "+"), Ordering::Less);
        assert_eq!(natural_cmp("\u{2026}", "+"), Ordering::Less);
        assert_eq!(natural_cmp("\u{ab}", "\u{20ac}"), Ordering::Less);
        // Symbols: euro (Sc), multiplication sign (Sm) sort after punctuation, before digits.
        assert_eq!(natural_cmp("-", "\u{20ac}"), Ordering::Less);
        assert_eq!(natural_cmp("\u{d7}", "0"), Ordering::Less);
        // Non-ASCII white space (no-break space, ideographic space) is white space.
        assert_eq!(natural_cmp("\u{a0}", "-"), Ordering::Less);
        assert_eq!(natural_cmp("\u{3000}", "-"), Ordering::Less);
        // Other numbers (superscript two) are not decimal digits: they sort with the letters.
        assert_eq!(natural_cmp("9", "\u{b2}"), Ordering::Less);
    }

    #[test]
    fn non_ascii_decimal_digits_are_numbers() {
        // Arabic-Indic 12 against 3: by value, so after; against 13: before.
        assert_eq!(natural_cmp("\u{661}\u{662}", "3"), Ordering::Greater);
        assert_eq!(natural_cmp("\u{661}\u{662}", "13"), Ordering::Less);
        assert_eq!(
            sorted(&["X\u{661}\u{660}", "X9", "X\u{662}"]),
            ["X\u{662}", "X9", "X\u{661}\u{660}"]
        );
        // Mixed scripts form one number: 1 followed by Arabic-Indic 2 is 12, above 11 and below 13.
        assert_eq!(natural_cmp("1\u{662}", "11"), Ordering::Greater);
        assert_eq!(natural_cmp("1\u{662}", "13"), Ordering::Less);
        assert_eq!(decimal_value('\u{ff19}'), Some(9));
        assert_eq!(decimal_value('\u{1d7ff}'), Some(9));
        assert_eq!(decimal_value('\u{1d7ce}'), Some(0));
        assert_eq!(decimal_value('\u{1d7d8}'), Some(0));
        assert_eq!(decimal_value('a'), None);
        assert_eq!(natural_cmp("\u{669}", "A"), Ordering::Less);
        // Same value, different script: distinguished by the later levels, never equal.
        assert_ne!(natural_key("\u{661}"), natural_key("1"));
    }

    #[test]
    fn levels() {
        // Primary ignores case and accents; secondary orders accents and leading zeros; tertiary case.
        assert_eq!(sorted(&["x10", "X9"]), ["X9", "x10"]);
        assert_eq!(sorted(&["k1", "K2", "K1"]), ["K1", "k1", "K2"]);
        assert_eq!(sorted(&["A2", "Ä1", "A1"]), ["A1", "Ä1", "A2"]);
        assert_eq!(sorted(&["X1", "X01", "X2"]), ["X01", "X1", "X2"]);
        assert_eq!(sorted(&["X1A", "X1", "X", "X1.1"]), ["X", "X1", "X1.1", "X1A"]);
        assert_ne!(natural_key("X01"), natural_key("X1"));
        assert_ne!(natural_key("k1"), natural_key("K1"));
    }

    #[test]
    fn canonical_equivalents_are_equal() {
        assert_eq!(natural_key("U\u{308}1"), natural_key("Ü1"));
        assert_eq!(natural_cmp("\u{2126}", "\u{3a9}"), Ordering::Equal);
        // Compatibility variants are not equal (NFC, not NFKC).
        assert_ne!(natural_key("\u{ff21}"), natural_key("A"));
    }

    #[test]
    fn locale_independent_case_mapping() {
        // No Turkish special case: I lower-cases to i everywhere (NFR-I18N-03, tr-TR).
        assert_eq!(sorted(&["i2", "I1"]), ["I1", "i2"]);
        assert_eq!(natural_cmp("İ1", "I1"), Ordering::Greater);
        assert_eq!(natural_cmp("İ1", "I2"), Ordering::Less);
    }

    proptest! {
        /// The order laws: antisymmetry with equality exactly for canonically equal strings, and transitivity.
        #[test]
        fn order_laws(
            x in "[ -~ÄäÖöÜüß]{0,8}", y in "[ -~ÄäÖöÜüß]{0,8}",
            a in "[-+.0-9A-Za-z]{0,6}", b in "[-+.0-9A-Za-z]{0,6}", c in "[-+.0-9A-Za-z]{0,6}",
        ) {
            let xy = natural_cmp(&x, &y);
            prop_assert_eq!(xy, natural_cmp(&y, &x).reverse(), "antisymmetric");
            prop_assert_eq!(xy == Ordering::Equal, to_nfc(&x) == to_nfc(&y), "identity");
            let mut v = [a, b, c];
            v.sort_by_key(|s| natural_key(s));
            prop_assert!(natural_cmp(&v[0], &v[1]) != Ordering::Greater, "transitive");
            prop_assert!(natural_cmp(&v[1], &v[2]) != Ordering::Greater, "transitive");
            prop_assert!(natural_cmp(&v[0], &v[2]) != Ordering::Greater, "transitive");
        }

        /// A proper prefix sorts first.
        #[test]
        fn prefix_sorts_first(s in "[-+.:0-9A-Za-z ]{0,8}", t in "[-+.:0-9A-Za-z ]{1,4}") {
            prop_assert_eq!(natural_cmp(&s, &format!("{s}{t}")), Ordering::Less);
        }
    }
}
