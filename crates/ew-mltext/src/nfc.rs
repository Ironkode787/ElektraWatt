// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Unicode Normalization Form C.

use std::borrow::Cow;

use unicode_normalization::UnicodeNormalization;

/// Whether `s` is in Unicode Normalization Form C (UAX #15). Every stored text and identifier is
/// (NFR-I18N-04).
///
/// ```
/// assert!(ew_mltext::is_nfc("Ü"));
/// assert!(!ew_mltext::is_nfc("U\u{308}"));
/// ```
pub fn is_nfc(s: &str) -> bool {
    unicode_normalization::is_nfc(s)
}

/// `s` in Normalization Form C, borrowed when it already is.
///
/// The normalisation step of "NFC-normalised on input" (FR-NUM-016, FR-I18N-011). Importers and the
/// reference-designation parser of `ew-rds`, which has no Unicode dependency, call this first.
///
/// ```
/// assert_eq!(ew_mltext::to_nfc("U\u{308}1"), "Ü1");
/// ```
pub fn to_nfc(s: &str) -> Cow<'_, str> {
    if is_nfc(s) {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(s.nfc().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_vectors() {
        // Composed and ASCII text is NFC; decomposed sequences and singletons are not.
        for s in ["", "-101F1", "Ü", "ñ", "测试", "Ω"] {
            assert!(is_nfc(s), "{s:?}");
            assert!(matches!(to_nfc(s), Cow::Borrowed(_)));
        }
        for (raw, nfc) in [
            ("U\u{308}", "Ü"),
            ("n\u{303}", "ñ"),
            // OHM SIGN is a canonical singleton of GREEK CAPITAL LETTER OMEGA.
            ("\u{2126}", "\u{3a9}"),
            // ANGSTROM SIGN likewise of LATIN CAPITAL LETTER A WITH RING ABOVE.
            ("\u{212b}", "\u{c5}"),
        ] {
            assert!(!is_nfc(raw), "{raw:?}");
            assert_eq!(to_nfc(raw), nfc);
        }
        // NFC is not NFKC: compatibility characters stay.
        assert_eq!(to_nfc("\u{ff21}"), "\u{ff21}");
    }
}
