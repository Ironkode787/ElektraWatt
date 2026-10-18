// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The strict and lenient reference-designation grammars.

use std::collections::BTreeMap;

use crate::designation::Level;
use crate::{Aspect, Designation, KnownCodes, ParseError, ParseErrorKind, RdsSyntax};

/// A deviation from the strict grammar that [`parse_lenient`] accepted (FR-NUM-015).
///
/// Each kind is reported at most once per level, connection point or signal, at the byte offset (into
/// the input) of its first occurrence there. The designation keeps the text as found; importers decide
/// what to propose (ADR-0026 decision 7: letters are kept as found).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum Tolerance {
    /// Leading or trailing white space was removed.
    SurroundingWhitespace,
    /// A lower-case ASCII letter: `-k1`.
    Lowercase {
        /// Byte offset of the first lower-case letter in the component.
        offset: usize,
    },
    /// A non-ASCII character: `+Halle-Ü1`.
    NonAscii {
        /// Byte offset of the first such character in the component.
        offset: usize,
    },
    /// An ASCII character outside the strict set that is no separator here: `_`, or `.` in a level.
    OtherCharacter {
        /// Byte offset of the first such character in the component.
        offset: usize,
        /// The character.
        ch: char,
    },
    /// Aspects out of canonical order, reordered: `+L1=P01-K1` reads as `=P01+L1-K1`.
    AspectOrder,
}

/// The result of [`parse_lenient`]: a designation with what was tolerated, or the text kept as an opaque
/// tag with the reason (FR-NUM-015: "S51" imports as an opaque frozen tag).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LenientParse {
    /// The text parsed, possibly with tolerated deviations (none means it is strictly valid).
    Parsed {
        /// The designation.
        designation: Designation,
        /// What was tolerated, in input order.
        tolerated: Vec<Tolerance>,
    },
    /// The text does not parse even leniently; it is kept as found, without surrounding white space.
    Opaque {
        /// The trimmed input.
        text: String,
        /// Why it is opaque (offsets into the original input).
        reason: ParseError,
    },
}

impl LenientParse {
    /// The designation, unless opaque.
    pub fn designation(&self) -> Option<&Designation> {
        match self {
            LenientParse::Parsed { designation, .. } => Some(designation),
            LenientParse::Opaque { .. } => None,
        }
    }

    /// Whether the text was kept as opaque text.
    pub fn is_opaque(&self) -> bool {
        matches!(self, LenientParse::Opaque { .. })
    }
}

struct Item {
    aspect: Aspect,
    level: Level,
    offset: usize,
}

/// The tokens of a designation, before the strict or lenient checks.
struct Scan {
    items: Vec<Item>,
    connection_point: Option<(String, usize)>,
    signal: Option<(String, usize)>,
}

fn error(kind: ParseErrorKind, offset: usize) -> ParseError {
    ParseError { kind, offset }
}

/// Splits `text` (at byte `base` of the caller's input) into levels, a connection point and a signal.
/// Known codes are matched first, longest first; otherwise a level runs to the next boundary.
fn scan(text: &str, base: usize, syntax: &RdsSyntax, known: &KnownCodes) -> Result<Scan, ParseError> {
    if text.is_empty() {
        return Err(error(ParseErrorKind::Empty, base));
    }
    let mut out = Scan {
        items: Vec::new(),
        connection_point: None,
        signal: None,
    };
    let mut current: Option<Aspect> = None;
    let mut pos = 0;
    while pos < text.len() {
        let rest = &text[pos..];
        if let Some(tail) = rest.strip_prefix(';') {
            if out.items.is_empty() {
                return Err(error(ParseErrorKind::MissingPrefix, base + pos));
            }
            if tail.is_empty() {
                return Err(error(ParseErrorKind::EmptySignal, base + pos));
            }
            out.signal = Some((tail.to_owned(), base + pos + 1));
            break;
        }
        if let Some(tail) = rest.strip_prefix(':') {
            if out.items.is_empty() {
                return Err(error(ParseErrorKind::MissingPrefix, base + pos));
            }
            let end = tail.find(';').unwrap_or(tail.len());
            if end == 0 {
                return Err(error(ParseErrorKind::EmptyConnectionPoint, base + pos));
            }
            out.connection_point = Some((tail[..end].to_owned(), base + pos + 1));
            pos += 1 + end;
            continue;
        }
        let Some((aspect, sign_len)) = syntax.token_at(rest, current) else {
            let ch = rest.chars().next().unwrap_or_default();
            let kind = if pos == 0 {
                ParseErrorKind::MissingPrefix
            } else {
                ParseErrorKind::UnexpectedCharacter(ch)
            };
            return Err(error(kind, base + pos));
        };
        pos += sign_len;
        let rest = &text[pos..];
        let (len, node) = match known.longest_match(aspect, rest, syntax) {
            Some((len, node)) => (len, Some(node)),
            None => {
                let len = rest
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(rest.len()))
                    .find(|&i| syntax.at_boundary(&rest[i..], aspect))
                    .unwrap_or(rest.len());
                (len, None)
            }
        };
        if len == 0 {
            return Err(error(ParseErrorKind::EmptyLevel, base + pos));
        }
        out.items.push(Item {
            aspect,
            level: Level::parsed(&rest[..len], node),
            offset: base + pos,
        });
        pos += len;
        current = Some(aspect);
    }
    Ok(out)
}

/// Consecutive items of one aspect form a run (one aspect with its levels). Returns the runs in input
/// order with the offset of their first level.
fn runs(items: Vec<Item>) -> Vec<(Aspect, usize, Vec<Level>)> {
    let mut runs: Vec<(Aspect, usize, Vec<Level>)> = Vec::new();
    for item in items {
        match runs.last_mut() {
            Some((aspect, _, levels)) if *aspect == item.aspect => levels.push(item.level),
            _ => runs.push((item.aspect, item.offset, vec![item.level])),
        }
    }
    runs
}

/// Checks that no aspect has two runs; returns whether the runs are in canonical order.
fn check_runs(runs: &[(Aspect, usize, Vec<Level>)]) -> Result<bool, ParseError> {
    let mut ordered = true;
    for (i, (aspect, offset, _)) in runs.iter().enumerate() {
        if runs[..i].iter().any(|(a, _, _)| a == aspect) {
            return Err(error(ParseErrorKind::RepeatedAspect, *offset));
        }
        if i > 0 && runs[i - 1].0 > *aspect {
            ordered = false;
        }
    }
    Ok(ordered)
}

fn strict_level_char(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit()
}

fn strict_tail_char(c: char) -> bool {
    strict_level_char(c) || matches!(c, '+' | '-' | '.')
}

fn first_invalid(text: &str, offset: usize, ok: fn(char) -> bool) -> Result<(), ParseError> {
    match text.char_indices().find(|&(_, c)| !ok(c)) {
        Some((i, c)) => Err(error(ParseErrorKind::InvalidCharacter(c), offset + i)),
        None => Ok(()),
    }
}

fn assemble(
    runs: Vec<(Aspect, usize, Vec<Level>)>,
    connection_point: Option<(String, usize)>,
    signal: Option<(String, usize)>,
) -> Result<Designation, ParseError> {
    if let Some((_, offset)) = &connection_point {
        if !runs.iter().any(|(a, _, _)| *a == Aspect::Product) {
            return Err(error(ParseErrorKind::ConnectionPointWithoutProduct, offset - 1));
        }
    }
    let aspects: BTreeMap<Aspect, Vec<Level>> = runs.into_iter().map(|(a, _, l)| (a, l)).collect();
    Ok(Designation::from_parts(
        aspects,
        connection_point.map(|(s, _)| s),
        signal.map(|(s, _)| s),
    ))
}

/// Parses `text` with the **strict** IEC 81346 grammar: for typed input and for checking derived
/// designations (EW-RDS-001, FR-NUM-015).
///
/// ```text
/// designation = run { run } [ ":" tail ] [ ";" tail ]
/// run         = sign level { ( sign | separator ) level }    one aspect, its signs from `syntax`
/// level       = known code or alias of the aspect            atomic, longest match first
///             | ( "A"…"Z" | "0"…"9" ) { "A"…"Z" | "0"…"9" }
/// tail        = ( "A"…"Z" | "0"…"9" | "+" | "-" | "." ) { … }
/// ```
///
/// - Aspects appear in canonical order, each at most once (as one run of levels).
/// - A level is upper-case letters and digits: `P01`, `101F1`, `FIELD`, `2416` (page-based numbering and
///   number-only levels are grammatical; I and O are a separate check, EW-RDS-007). A known code of the
///   aspect is taken whole first, so `J-E-2416` stays one level when it is known.
/// - A connection point needs a product aspect: `-X1031:1`. A signal follows `;`.
/// - No white space anywhere; the text is expected in NFC (`ew_mltext::to_nfc`).
///
/// # Errors
///
/// The first violation as a [`ParseError`].
///
/// ```
/// use ew_rds::{Aspect, KnownCodes, RdsSyntax, parse_strict};
///
/// let syntax = RdsSyntax::default();
/// let mut known = KnownCodes::new();
/// known.add_code(Aspect::Location, "J-E-2416").unwrap();
///
/// let d = parse_strict("=P01+J-E-2416-102G1", &syntax, &known).unwrap();
/// assert_eq!(d.levels(Aspect::Location)[0].text(), "J-E-2416");
/// assert_eq!(d.levels(Aspect::Product)[0].text(), "102G1");
///
/// // Without the known code, the same text splits at every prefix character.
/// let d = parse_strict("=P01+J-E-2416-102G1", &syntax, &KnownCodes::new()).unwrap();
/// assert_eq!(d.levels(Aspect::Location)[0].text(), "J");
/// assert_eq!(d.levels(Aspect::Product).len(), 3);
///
/// assert!(parse_strict("S51", &syntax, &known).is_err());
/// ```
pub fn parse_strict(text: &str, syntax: &RdsSyntax, known: &KnownCodes) -> Result<Designation, ParseError> {
    if let Some((i, _)) = text.char_indices().find(|(_, c)| c.is_whitespace()) {
        let kind = if text.trim().is_empty() {
            ParseErrorKind::Empty
        } else {
            ParseErrorKind::Whitespace
        };
        return Err(error(kind, i));
    }
    let scan = scan(text, 0, syntax, known)?;
    for item in &scan.items {
        if item.level.node().is_none() {
            first_invalid(item.level.text(), item.offset, strict_level_char)?;
        }
    }
    for (tail, offset) in scan.connection_point.iter().chain(&scan.signal) {
        first_invalid(tail, *offset, strict_tail_char)?;
    }
    let runs = runs(scan.items);
    if !check_runs(&runs)? {
        let offset = runs.windows(2).find(|w| w[0].0 > w[1].0).map_or(0, |w| w[1].1);
        return Err(error(ParseErrorKind::AspectOrder, offset));
    }
    assemble(runs, scan.connection_point, scan.signal)
}

/// The tolerances of one component (level, connection point or signal), or the reason it is opaque.
fn tolerate(
    text: &str,
    offset: usize,
    strict_ok: fn(char) -> bool,
    slash_ok: bool,
    out: &mut Vec<Tolerance>,
) -> Result<(), ParseError> {
    let (mut lower, mut non_ascii, mut other) = (false, false, false);
    for (i, c) in text.char_indices() {
        let at = offset + i;
        if strict_ok(c) {
            continue;
        }
        if c.is_control() || (c == '/' && !slash_ok) {
            return Err(error(ParseErrorKind::InvalidCharacter(c), at));
        } else if c.is_ascii_lowercase() {
            if !lower {
                out.push(Tolerance::Lowercase { offset: at });
            }
            lower = true;
        } else if !c.is_ascii() {
            if !non_ascii {
                out.push(Tolerance::NonAscii { offset: at });
            }
            non_ascii = true;
        } else {
            if !other {
                out.push(Tolerance::OtherCharacter { offset: at, ch: c });
            }
            other = true;
        }
    }
    Ok(())
}

fn lenient(
    text: &str,
    syntax: &RdsSyntax,
    known: &KnownCodes,
) -> Result<(Designation, Vec<Tolerance>), ParseError> {
    let base = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    let mut tolerated = Vec::new();
    if trimmed.is_empty() {
        return Err(error(ParseErrorKind::Empty, 0));
    }
    if trimmed.len() != text.len() {
        tolerated.push(Tolerance::SurroundingWhitespace);
    }
    if let Some((i, _)) = trimmed.char_indices().find(|(_, c)| c.is_whitespace()) {
        return Err(error(ParseErrorKind::Whitespace, base + i));
    }
    let scan = scan(trimmed, base, syntax, known)?;
    // Input order: levels, then the connection point, then the signal.
    for item in &scan.items {
        if item.level.node().is_none() {
            tolerate(
                item.level.text(),
                item.offset,
                strict_level_char,
                false,
                &mut tolerated,
            )?;
        }
    }
    for (tail, offset) in scan.connection_point.iter().chain(&scan.signal) {
        tolerate(tail, *offset, strict_tail_char, true, &mut tolerated)?;
    }
    let runs = runs(scan.items);
    if !check_runs(&runs)? {
        tolerated.push(Tolerance::AspectOrder);
    }
    let designation = assemble(runs, scan.connection_point, scan.signal)?;
    Ok((designation, tolerated))
}

/// Parses `text` with the **lenient** import grammar (FR-NUM-015): what the strict grammar accepts,
/// plus the deviations listed as [`Tolerance`]s; anything else is kept as opaque text with the reason.
///
/// Tolerated, beyond [`parse_strict`]: surrounding white space; lower-case letters, non-ASCII characters
/// and other ASCII characters (such as `_`, or `.` inside a level) that are no separator; aspects out of
/// canonical order. Still opaque: a missing leading sign (`S51`), white space inside, a control character,
/// `/` inside a level (a page reference), an empty level, an aspect in two separate places, and a
/// connection point without a product aspect.
///
/// ```
/// use ew_rds::{LenientParse, KnownCodes, RdsSyntax, Tolerance, parse_lenient};
///
/// let syntax = RdsSyntax::default();
/// let known = KnownCodes::new();
///
/// // "S51" imports as an opaque (frozen) tag.
/// assert!(parse_lenient("S51", &syntax, &known).is_opaque());
///
/// // A lower-case tag parses, and says so.
/// match parse_lenient(" -k1", &syntax, &known) {
///     LenientParse::Parsed { designation, tolerated } => {
///         assert_eq!(designation.to_string(), "-k1");
///         assert_eq!(tolerated, [Tolerance::SurroundingWhitespace, Tolerance::Lowercase { offset: 2 }]);
///     }
///     LenientParse::Opaque { .. } => unreachable!(),
/// }
/// ```
pub fn parse_lenient(text: &str, syntax: &RdsSyntax, known: &KnownCodes) -> LenientParse {
    match lenient(text, syntax, known) {
        Ok((designation, tolerated)) => LenientParse::Parsed {
            designation,
            tolerated,
        },
        Err(reason) => LenientParse::Opaque {
            text: text.trim().to_owned(),
            reason,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn syntax() -> RdsSyntax {
        RdsSyntax::default()
    }

    fn golden_known() -> KnownCodes {
        let mut k = KnownCodes::new();
        k.add_code(Aspect::Function, "P01").unwrap();
        k.add_code(Aspect::Location, "J-E-2416").unwrap();
        k.add_alias(Aspect::Location, "JE2416", "J-E-2416").unwrap();
        k.add_code(Aspect::Location, "J-E-206").unwrap();
        k.add_code(Aspect::Location, "FIELD").unwrap();
        k
    }

    fn strict(s: &str) -> Result<Designation, ParseError> {
        parse_strict(s, &syntax(), &golden_known())
    }

    fn texts(d: &Designation, a: Aspect) -> Vec<&str> {
        d.levels(a).iter().map(Level::text).collect()
    }

    fn err(s: &str) -> (ParseErrorKind, usize) {
        let e = strict(s).unwrap_err();
        (e.kind, e.offset)
    }

    #[test]
    fn golden_designations() {
        let d = strict("=P01+J-E-2416-102G1").unwrap();
        assert_eq!(texts(&d, Aspect::Function), ["P01"]);
        assert_eq!(texts(&d, Aspect::Location), ["J-E-2416"]);
        assert_eq!(texts(&d, Aspect::Product), ["102G1"]);
        assert_eq!(d.aspects().count(), 3);
        assert_eq!(d.to_string(), "=P01+J-E-2416-102G1");

        let d = strict("=P01+J-E-2416-X1031:1").unwrap();
        assert_eq!(texts(&d, Aspect::Product), ["X1031"]);
        assert_eq!(d.connection_point(), Some("1"));

        let d = strict("=P01+J-E-2416-201U3-Q1").unwrap();
        assert_eq!(texts(&d, Aspect::Product), ["201U3", "Q1"]);

        let d = strict("-101F1").unwrap();
        assert_eq!(texts(&d, Aspect::Product), ["101F1"]);

        let d = strict("==FA=P01++SITE+J-E-2416&EFS-101F1:N").unwrap();
        assert_eq!(
            d.aspects().map(|(a, _)| a).collect::<Vec<_>>(),
            [
                Aspect::FunctionalAssignment,
                Aspect::Function,
                Aspect::InstallationSite,
                Aspect::Location,
                Aspect::DocumentKind,
                Aspect::Product
            ]
        );
        assert_eq!(d.to_string(), "==FA=P01++SITE+J-E-2416&EFS-101F1:N");

        // Every aspect, the CP and a signal.
        let all = "==FA=P01++SITE+FIELD&EFS#U1%T2-K1:A1;RUN";
        assert_eq!(strict(all).unwrap().to_string(), all);
    }

    #[test]
    fn atomic_node_codes() {
        // FR-PRJ-017: never re-parsed into +J, -E, -2416.
        let d = strict("+J-E-2416-X1031").unwrap();
        assert_eq!(texts(&d, Aspect::Location), ["J-E-2416"]);
        assert_eq!(texts(&d, Aspect::Product), ["X1031"]);
        let node = d.levels(Aspect::Location)[0].node().unwrap();
        assert_eq!((node.code.as_str(), node.via_alias), ("J-E-2416", false));

        // The longer code wins over its prefix "J-E-206" … and a non-boundary is not a match.
        let d = strict("+J-E-206-X1").unwrap();
        assert_eq!(texts(&d, Aspect::Location), ["J-E-206"]);
        let d = strict("+J-E-2061").unwrap();
        assert_eq!(texts(&d, Aspect::Location), ["J"]);
        assert_eq!(texts(&d, Aspect::Product), ["E", "2061"]);

        // Aliases resolve to the node and are replaced on request (FR-PRJ-028).
        let mut d = strict("+JE2416-X1031:1").unwrap();
        let l = &d.levels(Aspect::Location)[0];
        assert_eq!((l.text(), l.code()), ("JE2416", "J-E-2416"));
        assert!(l.node().unwrap().via_alias);
        d.resolve_aliases();
        assert_eq!(d.to_string(), "+J-E-2416-X1031:1");

        // Known codes are per aspect: a product level "J-E-2416" is not atomic.
        let d = strict("-J-E-2416").unwrap();
        assert_eq!(texts(&d, Aspect::Product), ["J", "E", "2416"]);
    }

    #[test]
    fn strict_rejections() {
        assert_eq!(err(""), (ParseErrorKind::Empty, 0));
        assert_eq!(err("  "), (ParseErrorKind::Empty, 0));
        assert_eq!(err("S51"), (ParseErrorKind::MissingPrefix, 0));
        assert_eq!(err("1X1:PE"), (ParseErrorKind::MissingPrefix, 0));
        assert_eq!(err(":1"), (ParseErrorKind::MissingPrefix, 0));
        assert_eq!(err(";S"), (ParseErrorKind::MissingPrefix, 0));
        assert_eq!(err("-k1"), (ParseErrorKind::InvalidCharacter('k'), 1));
        assert_eq!(err("-K_1"), (ParseErrorKind::InvalidCharacter('_'), 2));
        assert_eq!(err("-K1.1"), (ParseErrorKind::InvalidCharacter('.'), 3));
        assert_eq!(err("-KÜ1"), (ParseErrorKind::InvalidCharacter('Ü'), 2));
        assert_eq!(err(" -K1"), (ParseErrorKind::Whitespace, 0));
        assert_eq!(err("-K 1"), (ParseErrorKind::Whitespace, 2));
        assert_eq!(err("-K1 "), (ParseErrorKind::Whitespace, 3));
        assert_eq!(err("-"), (ParseErrorKind::EmptyLevel, 1));
        assert_eq!(err("-K1-"), (ParseErrorKind::EmptyLevel, 4));
        assert_eq!(err("=+L1"), (ParseErrorKind::EmptyLevel, 1));
        assert_eq!(err("-K1:"), (ParseErrorKind::EmptyConnectionPoint, 3));
        assert_eq!(err("-K1:;S"), (ParseErrorKind::EmptyConnectionPoint, 3));
        assert_eq!(err("-K1;"), (ParseErrorKind::EmptySignal, 3));
        assert_eq!(err("-K1:a"), (ParseErrorKind::InvalidCharacter('a'), 4));
        assert_eq!(err("-K1:1/A"), (ParseErrorKind::InvalidCharacter('/'), 5));
        assert_eq!(err("-K1;R/N"), (ParseErrorKind::InvalidCharacter('/'), 5));
        assert_eq!(err("=P01:1"), (ParseErrorKind::ConnectionPointWithoutProduct, 4));
        assert_eq!(err("+L1=P01-K1"), (ParseErrorKind::AspectOrder, 4));
        assert_eq!(err("-K1+L1"), (ParseErrorKind::AspectOrder, 4));
        assert_eq!(err("=P01+L1=P02"), (ParseErrorKind::RepeatedAspect, 8));
        assert_eq!(err("=P01/101"), (ParseErrorKind::InvalidCharacter('/'), 4));
    }

    #[test]
    fn unexpected_characters_after_a_connection_point_cannot_occur() {
        // The connection point runs to ';' or the end, so prefix characters inside it are its own.
        let d = strict("-G1:L+").unwrap();
        assert_eq!(d.connection_point(), Some("L+"));
        let d = strict("-G1:-").unwrap();
        assert_eq!(d.connection_point(), Some("-"));
        let d = strict("-X1:1.1;S").unwrap();
        assert_eq!((d.connection_point(), d.signal()), (Some("1.1"), Some("S")));
    }

    #[test]
    fn separators() {
        let mut s = syntax();
        s.set_separator(Aspect::Location, Some(".")).unwrap();
        let k = golden_known();
        let d = parse_strict("+J-E-2416.DOOR-K1", &s, &k).unwrap();
        assert_eq!(texts(&d, Aspect::Location), ["J-E-2416", "DOOR"]);
        assert_eq!(d.format(&s), "+J-E-2416.DOOR-K1");
        // The repeated sign is accepted too and formats with the separator.
        let d = parse_strict("+J-E-2416+DOOR-K1", &s, &k).unwrap();
        assert_eq!(d.format(&s), "+J-E-2416.DOOR-K1");
        // The separator only separates in its own aspect.
        let e = parse_strict("-K1.1", &s, &k).unwrap_err();
        assert_eq!(e.kind, ParseErrorKind::InvalidCharacter('.'));
        // A custom sign.
        s.set_sign(Aspect::Type, "%%").unwrap();
        let d = parse_strict("%%T1-K1", &s, &k).unwrap();
        assert_eq!(texts(&d, Aspect::Type), ["T1"]);
        let e = parse_strict("%T1", &s, &k).unwrap_err();
        assert_eq!(e.kind, ParseErrorKind::MissingPrefix);
    }

    fn lenient_of(s: &str) -> LenientParse {
        parse_lenient(s, &syntax(), &golden_known())
    }

    #[test]
    fn lenient_tolerances() {
        let cases: [(&str, &str, Vec<Tolerance>); 8] = [
            ("=P01+J-E-2416-102G1", "=P01+J-E-2416-102G1", vec![]),
            (" -K1\t", "-K1", vec![Tolerance::SurroundingWhitespace]),
            ("-k1", "-k1", vec![Tolerance::Lowercase { offset: 1 }]),
            (
                "+Halle-Ü1",
                "+Halle-Ü1",
                vec![
                    Tolerance::Lowercase { offset: 2 },
                    Tolerance::NonAscii { offset: 7 },
                ],
            ),
            (
                "-K_1",
                "-K_1",
                vec![Tolerance::OtherCharacter { offset: 2, ch: '_' }],
            ),
            (
                "-Q0.1",
                "-Q0.1",
                vec![Tolerance::OtherCharacter { offset: 3, ch: '.' }],
            ),
            (
                "+JE2416-X1031:1a",
                "+JE2416-X1031:1a",
                vec![Tolerance::Lowercase { offset: 15 }],
            ),
            (
                "-X1+J-E-2416=P01",
                "=P01+J-E-2416-X1",
                vec![Tolerance::AspectOrder],
            ),
        ];
        for (input, formatted, expected) in cases {
            match lenient_of(input) {
                LenientParse::Parsed {
                    designation,
                    tolerated,
                } => {
                    assert_eq!(designation.to_string(), formatted, "{input}");
                    assert_eq!(tolerated, expected, "{input}");
                }
                LenientParse::Opaque { reason, .. } => panic!("{input}: {reason}"),
            }
        }
        // A strictly valid designation parses leniently without tolerances, to the same value.
        let lp = lenient_of("=P01+J-E-2416-X1031:1");
        assert_eq!(lp.designation(), Some(&strict("=P01+J-E-2416-X1031:1").unwrap()));
        assert!(!lp.is_opaque());
    }

    #[test]
    fn lenient_opaque() {
        for (input, kind, offset) in [
            ("S51", ParseErrorKind::MissingPrefix, 0),
            (" S51 ", ParseErrorKind::MissingPrefix, 1),
            ("1X1:PE", ParseErrorKind::MissingPrefix, 0),
            ("-K 1", ParseErrorKind::Whitespace, 2),
            ("", ParseErrorKind::Empty, 0),
            ("   ", ParseErrorKind::Empty, 0),
            ("-K1-", ParseErrorKind::EmptyLevel, 4),
            ("=P01+L1=P02", ParseErrorKind::RepeatedAspect, 8),
            ("=P01/101", ParseErrorKind::InvalidCharacter('/'), 4),
            ("-K\u{7}1", ParseErrorKind::InvalidCharacter('\u{7}'), 2),
            ("=P01:1", ParseErrorKind::ConnectionPointWithoutProduct, 4),
        ] {
            match lenient_of(input) {
                LenientParse::Opaque { text, reason } => {
                    assert_eq!(text, input.trim(), "{input:?}");
                    assert_eq!((reason.kind, reason.offset), (kind, offset), "{input:?}");
                }
                LenientParse::Parsed { designation, .. } => panic!("{input:?} parsed as {designation}"),
            }
        }
        // A slash inside a connection point is tolerated (clamp notation "1/a").
        assert!(!lenient_of("-X1:1/A").is_opaque());
    }

    fn arb_level() -> impl Strategy<Value = String> {
        "[A-Z0-9]{1,6}"
    }

    fn arb_designation() -> impl Strategy<Value = Designation> {
        (
            proptest::collection::btree_map(
                proptest::sample::select(Aspect::ALL.to_vec()),
                proptest::collection::vec(arb_level(), 1..3),
                1..5,
            ),
            proptest::option::of("[A-Z0-9+.-]{1,4}"),
            proptest::option::of("[A-Z0-9]{1,4}"),
        )
            .prop_map(|(aspects, cp, signal)| {
                let mut d = Designation::new();
                for (a, levels) in &aspects {
                    for l in levels {
                        d.push(*a, l).unwrap();
                    }
                }
                if aspects.contains_key(&Aspect::Product) {
                    d.set_connection_point(cp.as_deref()).unwrap();
                }
                d.set_signal(signal.as_deref()).unwrap();
                d
            })
    }

    proptest! {
        /// format ∘ parse_strict is the identity on formatted designations.
        #[test]
        fn strict_round_trip(d in arb_designation()) {
            let s = d.to_string();
            let parsed = parse_strict(&s, &syntax(), &KnownCodes::new()).unwrap();
            prop_assert_eq!(parsed.to_string(), s.clone());
            prop_assert_eq!(&parsed, &d);
            // Lenient agrees, without tolerances.
            match parse_lenient(&s, &syntax(), &KnownCodes::new()) {
                LenientParse::Parsed { designation, tolerated } => {
                    prop_assert_eq!(designation, d);
                    prop_assert!(tolerated.is_empty());
                }
                LenientParse::Opaque { .. } => prop_assert!(false, "opaque: {}", s),
            }
        }

        /// With the codes known, a code containing prefix characters always comes back as one level.
        #[test]
        fn known_codes_stay_atomic(parts in proptest::collection::vec("[A-Z0-9]{1,4}", 2..4), product in arb_level()) {
            let code = parts.join("-");
            let mut known = KnownCodes::new();
            known.add_code(Aspect::Location, &code).unwrap();
            let text = format!("+{code}-{product}");
            let d = parse_strict(&text, &syntax(), &known).unwrap();
            let location: Vec<&str> = d.levels(Aspect::Location).iter().map(Level::text).collect();
            prop_assert_eq!(location, vec![code.as_str()]);
            prop_assert_eq!(d.levels(Aspect::Product).len(), 1);
            prop_assert_eq!(d.to_string(), text);
        }

        /// Neither parser panics, and lenient accepts everything strict accepts.
        #[test]
        fn never_panics(s in "[-=+&#%:;./ a-zA-Z0-9_Ü]{0,16}") {
            let strict = parse_strict(&s, &syntax(), &golden_known());
            let lenient = parse_lenient(&s, &syntax(), &golden_known());
            if let Ok(d) = strict {
                prop_assert_eq!(lenient.designation(), Some(&d));
            }
        }
    }
}
