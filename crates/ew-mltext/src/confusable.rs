// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Identifier hygiene by Unicode Technical Standard #39: confusables, mixed scripts, restricted characters.

use unicode_security::{GeneralSecurityProfile, MixedScript, skeleton};

/// One hygiene finding on an identifier (FR-NUM-016, FR-I18N-011).
///
/// Findings are data for the rule engine, never errors or panics: the checks decide the severity
/// (EW-RDS-009 reports [`Confusable`](IdentFinding::Confusable) and
/// [`MixedScript`](IdentFinding::MixedScript) as errors). Offsets are byte offsets into the identifier,
/// so a UI can highlight the character; `ch` names the code point (`U+0410`).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum IdentFinding {
    /// The identifier mixes letters of more than one script (UTS #39 §5.1: its resolved script set is
    /// empty), such as Latin and Cyrillic. Digits and punctuation belong to every script.
    MixedScript,
    /// A non-ASCII character whose UTS #39 confusable skeleton is ASCII text other than itself, so it
    /// looks like `prototype`: a Cyrillic `А` (U+0410) looks like `A`, an en dash like `-`.
    ///
    /// Reported when the identifier can pass for other text: it is mixed-script, or its skeleton as a
    /// whole is ASCII (a whole-script confusable such as `-К1` typed on a Cyrillic keyboard layout). A
    /// genuinely Cyrillic word such as `Пускатель` has lookalike letters but cannot pass for ASCII, so its
    /// letters are not reported.
    Confusable {
        /// Byte offset of the character.
        offset: usize,
        /// The character.
        ch: char,
        /// The ASCII text it can be mistaken for.
        prototype: String,
    },
    /// A non-ASCII character outside the UTS #39 General Security Profile (`Identifier_Status` is not
    /// `Allowed`): invisible and format characters, no-break spaces, compatibility forms such as `µ`.
    Restricted {
        /// Byte offset of the character.
        offset: usize,
        /// The character.
        ch: char,
    },
}

/// The hygiene findings of identifier text `s`: first [`IdentFinding::MixedScript`] if it applies, then
/// per-character findings in offset order (a character may be both confusable and restricted).
///
/// Pure ASCII text never has findings: the ASCII lookalikes `0`/`O` and `1`/`l` are not reported, because
/// tags, part numbers and PLC addresses are ASCII by design.
///
/// ```
/// use ew_mltext::{IdentFinding, ident_findings};
///
/// assert!(ident_findings("-101F1").is_empty());
/// // A Cyrillic А in a tag: mixed scripts, and the code point is named.
/// let f = ident_findings("-101F\u{410}1");
/// assert_eq!(f[0], IdentFinding::MixedScript);
/// assert_eq!(
///     f[1],
///     IdentFinding::Confusable { offset: 5, ch: '\u{410}', prototype: "A".to_owned() }
/// );
/// // Without a Latin letter the tag is single-script Cyrillic, but it still passes for ASCII.
/// assert_eq!(
///     ident_findings("-101\u{410}1"),
///     [IdentFinding::Confusable { offset: 4, ch: '\u{410}', prototype: "A".to_owned() }]
/// );
/// ```
pub fn ident_findings(s: &str) -> Vec<IdentFinding> {
    let mut findings = Vec::new();
    if s.is_ascii() {
        return findings;
    }
    let mixed = !s.is_single_script();
    if mixed {
        findings.push(IdentFinding::MixedScript);
    }
    let passes_for_other_text = mixed || skeleton(s).all(|c| c.is_ascii());
    let mut buf = [0u8; 4];
    for (offset, ch) in s.char_indices().filter(|(_, c)| !c.is_ascii()) {
        let own: &str = ch.encode_utf8(&mut buf);
        let prototype: String = skeleton(own).collect();
        if passes_for_other_text && !prototype.is_empty() && prototype.is_ascii() && prototype != own {
            findings.push(IdentFinding::Confusable {
                offset,
                ch,
                prototype,
            });
        }
        if !ch.identifier_allowed() {
            findings.push(IdentFinding::Restricted { offset, ch });
        }
    }
    findings
}

/// The UTS #39 confusable skeleton of `s`: strings that can be mistaken for each other have equal
/// skeletons, such as `-KA1` written with a Latin and with a Cyrillic `A`. For "looks like a duplicate"
/// checks; a skeleton is never displayed or stored.
///
/// ```
/// use ew_mltext::confusable_skeleton;
///
/// assert_eq!(confusable_skeleton("-K\u{410}1"), confusable_skeleton("-KA1"));
/// ```
pub fn confusable_skeleton(s: &str) -> String {
    skeleton(s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn ascii_identifiers_are_clean() {
        for s in [
            "-101F1",
            "=P01+J-E-2416-X1031:1",
            "A9F78116",
            "PT 2.5-TWIN",
            "Q0.1",
            "TA05-V-15",
            "51E-F13GCQP-D+H2GG",
            "O0l1I",
        ] {
            assert!(ident_findings(s).is_empty(), "{s}");
        }
    }

    #[test]
    fn single_script_non_ascii_is_clean() {
        for s in ["Ölfilter", "Lüfter-3", "Schütz", "测试-1", "Пускатель"] {
            assert!(ident_findings(s).is_empty(), "{s}: {:?}", ident_findings(s));
        }
    }

    #[test]
    fn cyrillic_a_in_a_tag() {
        assert_eq!(
            ident_findings("-K\u{410}1"),
            [
                IdentFinding::MixedScript,
                IdentFinding::Confusable {
                    offset: 2,
                    ch: '\u{410}',
                    prototype: "A".to_owned()
                },
            ]
        );
        // Greek capital omicron among Latin letters.
        assert!(ident_findings("F\u{39f}O").contains(&IdentFinding::MixedScript));
    }

    #[test]
    fn whole_script_confusable() {
        // A tag typed entirely on a Cyrillic layout: single-script, but it passes for "-KA1".
        let f = ident_findings("-\u{41a}\u{410}1");
        assert!(!f.contains(&IdentFinding::MixedScript));
        assert_eq!(
            f,
            [
                IdentFinding::Confusable {
                    offset: 1,
                    ch: '\u{41a}',
                    prototype: "K".to_owned()
                },
                IdentFinding::Confusable {
                    offset: 3,
                    ch: '\u{410}',
                    prototype: "A".to_owned()
                },
            ]
        );
    }

    #[test]
    fn confusable_punctuation_and_restricted_characters() {
        // EN DASH looks like a hyphen-minus (a typical paste from a word processor).
        let f = ident_findings("-X1\u{2013}1");
        assert!(
            f.iter()
                .any(|f| matches!(f, IdentFinding::Confusable { offset: 3, ch: '\u{2013}', prototype } if prototype == "-")),
            "{f:?}"
        );
        // A no-break space and a micro sign are restricted.
        assert!(ident_findings("PT\u{a0}2.5").contains(&IdentFinding::Restricted {
            offset: 2,
            ch: '\u{a0}'
        }));
        assert!(ident_findings("10\u{b5}F").contains(&IdentFinding::Restricted {
            offset: 2,
            ch: '\u{b5}'
        }));
        // A zero-width space is restricted.
        assert!(ident_findings("X\u{200b}1").contains(&IdentFinding::Restricted {
            offset: 1,
            ch: '\u{200b}'
        }));
    }

    #[test]
    fn skeletons() {
        assert_eq!(confusable_skeleton("-K\u{410}1"), confusable_skeleton("-KA1"));
        assert_ne!(confusable_skeleton("-KB1"), confusable_skeleton("-KA1"));
    }

    proptest! {
        #[test]
        fn ascii_never_has_findings(s in "[ -~]{0,24}") {
            prop_assert!(ident_findings(&s).is_empty());
        }

        #[test]
        fn never_panics(s in "\\PC{0,16}") {
            let _ = ident_findings(&s);
            let _ = confusable_skeleton(&s);
        }
    }
}
