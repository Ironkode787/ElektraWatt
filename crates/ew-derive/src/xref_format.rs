// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cross-reference text formatter (doc 06 §2.8, doc 03 §5.1 `XrefFormat`, FR-XRF-002, FR-XRF-003,
//! FR-XRF-008, FR-XRF-025).
//!
//! Pure string functions: no [`Q`](crate::Q), no model access. A format string is parsed once into
//! [`XrefToken`]s (`{page}`-style variables and the QET `%F`-style variables are both understood),
//! rendered per target with [`render_item`], laid out as the matrix with [`render_list`] and wrapped
//! with [`render_wrapper`] (for example `"∞ {list}"`). The byte range of every shown item is kept so
//! that a renderer can make each item a hit target.

use std::ops::Range;

use crate::profile::XrefMatrix;

/// One token of a cross-reference format string.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum XrefToken {
    /// Literal text.
    Lit(String),
    /// `{page}` or QET `%F`: the folio label of the target page.
    Page,
    /// `{page_index}` or QET `%f`: the 1-based position in `page_order` counting every page record (drawn,
    /// divider and the base page of each report block) but not report continuation pages.
    PageIndex,
    /// `{row}` or QET `%l`.
    Row,
    /// `{column}` or QET `%c`.
    Column,
    /// `{function}` or QET `%M`: the target page's `=` structure, rendered (`=P01`).
    Function,
    /// `{location}` or QET `%LM`: the target page's `+` structure, rendered (`+J-E-2416`).
    Location,
    /// `{list}`: the list of items (only meaningful in a wrapper format such as `∞ {list}`).
    List,
}

fn push_lit(out: &mut Vec<XrefToken>, lit: &mut String) {
    if !lit.is_empty() {
        out.push(XrefToken::Lit(std::mem::take(lit)));
    }
}

/// Parses a format string.
///
/// `{name}` with a known name and the QET variables (`%F`, `%f`, `%l`, `%c`, `%M`, `%LM`, `%%`) become
/// tokens; an unknown `{…}`, an unknown `%x`, a trailing `%` and an unclosed `{` stay literal. Adjacent
/// literal pieces are merged and no empty [`XrefToken::Lit`] is produced.
#[must_use]
pub fn parse_format(format: &str) -> Vec<XrefToken> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut rest = format;
    while let Some(c) = rest.chars().next() {
        if c == '{' {
            match rest.find('}') {
                Some(end) => {
                    let token = match &rest[1..end] {
                        "page" => Some(XrefToken::Page),
                        "page_index" => Some(XrefToken::PageIndex),
                        "row" => Some(XrefToken::Row),
                        "column" => Some(XrefToken::Column),
                        "function" => Some(XrefToken::Function),
                        "location" => Some(XrefToken::Location),
                        "list" => Some(XrefToken::List),
                        _ => None,
                    };
                    match token {
                        Some(t) => {
                            push_lit(&mut out, &mut lit);
                            out.push(t);
                        }
                        None => lit.push_str(&rest[..=end]),
                    }
                    rest = &rest[end + 1..];
                }
                None => {
                    lit.push_str(rest);
                    rest = "";
                }
            }
        } else if c == '%' {
            let after = &rest[1..];
            if let Some(tail) = after.strip_prefix("LM") {
                push_lit(&mut out, &mut lit);
                out.push(XrefToken::Location);
                rest = tail;
                continue;
            }
            let token = match after.chars().next() {
                Some('F') => Some(XrefToken::Page),
                Some('f') => Some(XrefToken::PageIndex),
                Some('l') => Some(XrefToken::Row),
                Some('c') => Some(XrefToken::Column),
                Some('M') => Some(XrefToken::Function),
                Some('%') => {
                    lit.push('%');
                    rest = &after[1..];
                    continue;
                }
                _ => None,
            };
            match token {
                Some(t) => {
                    push_lit(&mut out, &mut lit);
                    out.push(t);
                    // All variable letters are ASCII: one byte.
                    rest = &after[1..];
                }
                None => {
                    lit.push('%');
                    rest = after;
                }
            }
        } else {
            lit.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    push_lit(&mut out, &mut lit);
    out
}

/// The first piece of a format that [`parse_format`] keeps as literal text although it looks like a token:
/// an unknown `{…}`, an unclosed `{`, an unknown `%x` or a trailing `%`. `None` when every token is known.
pub(crate) fn unknown_token(format: &str) -> Option<String> {
    let mut rest = format;
    while let Some(c) = rest.chars().next() {
        if c == '{' {
            let Some(end) = rest.find('}') else {
                return Some(rest.to_owned());
            };
            if !matches!(
                &rest[1..end],
                "page" | "page_index" | "row" | "column" | "function" | "location" | "list"
            ) {
                return Some(rest[..=end].to_owned());
            }
            rest = &rest[end + 1..];
        } else if c == '%' {
            let after = &rest[1..];
            if let Some(tail) = after.strip_prefix("LM") {
                rest = tail;
                continue;
            }
            match after.chars().next() {
                Some('F' | 'f' | 'l' | 'c' | 'M' | '%') => rest = &after[1..],
                Some(other) => return Some(format!("%{other}")),
                None => return Some("%".to_owned()),
            }
        } else {
            rest = &rest[c.len_utf8()..];
        }
    }
    None
}

/// The values one target supplies to an item format.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ItemValues {
    /// The target page's folio label (`"102"`).
    pub page: String,
    /// The 1-based position in page order; `None` prints `?`.
    pub page_index: Option<usize>,
    /// The row label (`"A"`); `None` prints `?`.
    pub row: Option<String>,
    /// The column label (`"1"`); `None` prints `?`.
    pub column: Option<String>,
    /// The rendered `=` structure (`"=P01"`) or empty.
    pub function: String,
    /// The rendered `+` structure (`"+J-E-2416"`) or empty.
    pub location: String,
}

/// How one item is rendered.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ItemOptions {
    /// The same-page "zone only" form (FR-XRF-025).
    pub zone_only: bool,
    /// A structure prefix (`"+J-E-206"`), or `None`.
    pub prefix: Option<String>,
}

fn is_page_part(token: &XrefToken) -> bool {
    matches!(
        token,
        XrefToken::Page | XrefToken::PageIndex | XrefToken::Function | XrefToken::Location
    )
}

fn is_value_token(token: &XrefToken) -> bool {
    matches!(
        token,
        XrefToken::Page | XrefToken::PageIndex | XrefToken::Row | XrefToken::Column
    )
}

fn push_value(out: &mut String, token: &XrefToken, values: &ItemValues) {
    match token {
        XrefToken::Lit(s) => out.push_str(s),
        XrefToken::Page => out.push_str(&values.page),
        XrefToken::PageIndex => match values.page_index {
            Some(n) => out.push_str(&n.to_string()),
            None => out.push('?'),
        },
        XrefToken::Row => out.push_str(values.row.as_deref().unwrap_or("?")),
        XrefToken::Column => out.push_str(values.column.as_deref().unwrap_or("?")),
        XrefToken::Function => out.push_str(&values.function),
        XrefToken::Location => out.push_str(&values.location),
        XrefToken::List => {}
    }
}

/// Renders one item (an `ip` or `aux_to_main` format) for one target.
///
/// `zone_only` drops the page parts (page, page index, function, location) with their separator, see
/// `zone_only_item`; `prefix` names the target's structure in front of the first value (ignored for a
/// zone-only item and for a format that names the structure itself, see [`names_structure`]).
#[must_use]
pub fn render_item(tokens: &[XrefToken], values: &ItemValues, options: &ItemOptions) -> String {
    if options.zone_only {
        return zone_only_item(tokens, values);
    }
    let mut out = String::new();
    let first_value = match &options.prefix {
        Some(_) if !names_structure(tokens) => tokens.iter().position(is_value_token),
        _ => None,
    };
    let prefix = options.prefix.as_deref().unwrap_or("");
    // Index of the literal before the first value when it ends with '/' (then that '/' is the separator).
    let slash_lit = first_value
        .and_then(|i| i.checked_sub(1))
        .filter(|&j| matches!(&tokens[j], XrefToken::Lit(s) if s.ends_with('/')));
    for (i, token) in tokens.iter().enumerate() {
        if Some(i) == first_value && slash_lit.is_none() {
            out.push_str(prefix);
            out.push('/');
        }
        match token {
            XrefToken::Lit(s) if Some(i) == slash_lit => {
                out.push_str(&s[..s.len() - 1]);
                out.push_str(prefix);
                out.push('/');
            }
            _ => push_value(&mut out, token, values),
        }
    }
    out
}

/// The zone-only form (FR-XRF-025): every run of consecutive page parts is dropped together with its
/// separator. The separator is the literal right after the run when a row or column follows it
/// (`"{page}-{row}{column}"` → `"E1"`, `"({page}-{row}{column})"` → `"(E1)"`); a run behind the cell, with
/// nothing to separate it from, takes the literal right before it (`"{row}{column}/{page}"` → `"E1"`,
/// `"[{row}{column}/{page}]"` → `"[E1]"`).
fn zone_only_item(tokens: &[XrefToken], values: &ItemValues) -> String {
    let mut keep = vec![true; tokens.len()];
    let mut i = 0;
    while i < tokens.len() {
        if !is_page_part(&tokens[i]) {
            i += 1;
            continue;
        }
        let mut end = i;
        while end < tokens.len() && is_page_part(&tokens[end]) {
            keep[end] = false;
            end += 1;
        }
        let cell_follows = tokens[end..]
            .iter()
            .any(|t| matches!(t, XrefToken::Row | XrefToken::Column));
        if cell_follows {
            if matches!(tokens.get(end), Some(XrefToken::Lit(_))) {
                keep[end] = false;
            }
        } else if i > 0 && matches!(tokens[i - 1], XrefToken::Lit(_)) {
            keep[i - 1] = false;
        }
        i = end;
    }
    let mut out = String::new();
    for (token, keep) in tokens.iter().zip(keep) {
        if keep {
            push_value(&mut out, token, values);
        }
    }
    out
}

/// Whether the tokens name the structure explicitly (`{function}` or `{location}` present).
#[must_use]
pub fn names_structure(tokens: &[XrefToken]) -> bool {
    tokens
        .iter()
        .any(|t| matches!(t, XrefToken::Function | XrefToken::Location))
}

/// A rendered text with the byte range of each shown item in it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Rendered {
    /// The text.
    pub text: String,
    /// The byte range of each shown item in `text`.
    pub items: Vec<Range<usize>>,
}

/// Lays out already-rendered items as the matrix (FR-XRF-008).
///
/// `per_row == 0` means unlimited per row, `max == 0` unlimited in total. Items are joined by `", "`
/// within a row and rows by `"\n"`; when items were cut, `", "` and the overflow mark follow the last
/// shown item (the mark is not an item range).
#[must_use]
pub fn render_list(items: &[String], matrix: &XrefMatrix) -> Rendered {
    render_list_more(items, false, matrix)
}

/// [`render_list`] for items cut by the caller: `more` says that further items exist beyond `items`, which
/// the overflow mark then stands for. A caller that formats only the first `matrix.max` items of a long
/// list passes `more = true` and never formats the rest.
#[must_use]
pub(crate) fn render_list_more(items: &[String], more: bool, matrix: &XrefMatrix) -> Rendered {
    let max = usize::from(matrix.max);
    let per_row = usize::from(matrix.per_row);
    let shown = if max == 0 {
        items.len()
    } else {
        items.len().min(max)
    };
    let mut text = String::new();
    let mut ranges = Vec::with_capacity(shown);
    for (i, item) in items.iter().take(shown).enumerate() {
        if i > 0 {
            text.push_str(if per_row != 0 && i % per_row == 0 {
                "\n"
            } else {
                ", "
            });
        }
        let start = text.len();
        text.push_str(item);
        ranges.push(start..text.len());
    }
    if shown < items.len() || more {
        text.push_str(", ");
        text.push_str(&matrix.overflow);
    }
    Rendered { text, items: ranges }
}

/// Substitutes a rendered list into a wrapper format (`"∞ {list}"`), shifting the item ranges.
///
/// Only the first `{list}` carries the ranges; later ones insert the text only. A wrapper without a
/// `{list}` yields no ranges.
#[must_use]
pub fn render_wrapper(tokens: &[XrefToken], list: &Rendered) -> Rendered {
    let mut text = String::new();
    let mut items = Vec::new();
    let mut placed = false;
    for token in tokens {
        match token {
            XrefToken::Lit(s) => text.push_str(s),
            XrefToken::List => {
                if !placed {
                    placed = true;
                    let offset = text.len();
                    items = list
                        .items
                        .iter()
                        .map(|r| r.start + offset..r.end + offset)
                        .collect();
                }
                text.push_str(&list.text);
            }
            _ => {}
        }
    }
    Rendered { text, items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn values() -> ItemValues {
        ItemValues {
            page: "102".into(),
            page_index: Some(6),
            row: Some("A".into()),
            column: Some("1".into()),
            function: "=P01".into(),
            location: "+J-E-2416".into(),
        }
    }

    fn item(format: &str, v: &ItemValues, o: &ItemOptions) -> String {
        render_item(&parse_format(format), v, o)
    }

    fn plain(format: &str) -> String {
        item(format, &values(), &ItemOptions::default())
    }

    fn zone(format: &str) -> String {
        let v = ItemValues {
            page: "1".into(),
            row: Some("H".into()),
            column: Some("16".into()),
            ..ItemValues::default()
        };
        let o = ItemOptions {
            zone_only: true,
            prefix: None,
        };
        item(format, &v, &o)
    }

    fn prefixed(format: &str, page: &str, row: &str, column: &str) -> String {
        let v = ItemValues {
            page: page.into(),
            row: Some(row.into()),
            column: Some(column.into()),
            ..ItemValues::default()
        };
        let o = ItemOptions {
            zone_only: false,
            prefix: Some("+J-E-206".into()),
        };
        item(format, &v, &o)
    }

    fn matrix(per_row: u8, max: u8) -> XrefMatrix {
        XrefMatrix {
            per_row,
            max,
            overflow: "…".into(),
        }
    }

    fn strings(n: usize) -> Vec<String> {
        (0..n)
            .map(|i| char::from(b'a' + u8::try_from(i % 26).unwrap()).to_string())
            .collect()
    }

    /// How an item example is rendered.
    enum Render {
        /// With [`values`] and the default options.
        Plain,
        /// Zone-only, for page 1 cell H16 ([`zone`]).
        Zone,
        /// With the prefix `+J-E-206` for (page, row, column) ([`prefixed`]).
        Prefixed(&'static str, &'static str, &'static str),
        /// Zone-only with the prefix `+J-E-206`, for page 1 cell H16: zone-only ignores the prefix.
        ZonePrefixed,
    }

    /// Rendered item examples: (name, how, format, expected text).
    #[test]
    fn item_examples() {
        use Render::{Plain, Prefixed, Zone, ZonePrefixed};
        #[rustfmt::skip]
        let cases = [
            // EPLAN presets.
            ("eplan page.column", Plain, "/{page}.{column}", "/102.1"),
            ("eplan page.row column", Plain, "/{page}.{row}{column}", "/102.A1"),
            // Zone only.
            ("zone brace preset", Zone, "{page}-{row}{column}", "H16"),
            ("zone eplan preset", Zone, "/{page}.{row}{column}", "/H16"),
            ("zone qet preset", Zone, "%F-%l%c", "H16"),
            ("zone full structure", Zone, "{function}{location}/{page}.{row}{column}", "H16"),
            // Prefix.
            ("prefix eplan", Prefixed("102", "A", "1"), "/{page}.{row}{column}", "+J-E-206/102.A1"),
            ("prefix in brackets", Prefixed("201", "E", "1"), "({page}-{row}{column})", "(+J-E-206/201-E1)"),
            ("prefix brace preset", Prefixed("102", "A", "1"), "{page}-{row}{column}", "+J-E-206/102-A1"),
            ("prefix after a literal", Prefixed("102", "A", "1"), "x/{page}", "x+J-E-206/102"),
            // No value token: no prefix.
            ("no value token, no prefix", Prefixed("102", "A", "1"), "abc", "abc"),
            // First value token is a row: the prefix goes in front of it.
            ("prefix before a row", Prefixed("102", "A", "1"), "[{row}]", "[+J-E-206/A]"),
            ("zone-only ignores the prefix", ZonePrefixed, "{page}-{row}{column}", "H16"),
            // The list token and Unicode.
            ("unicode literal", Plain, "∞ {page}", "∞ 102"),
            // A list token in an item format prints nothing.
            ("list token in an item", Plain, "a{list}b", "ab"),
        ];
        for (name, how, format, expected) in cases {
            let got = match how {
                Plain => plain(format),
                Zone => zone(format),
                Prefixed(page, row, column) => prefixed(format, page, row, column),
                ZonePrefixed => {
                    let o = ItemOptions {
                        zone_only: true,
                        prefix: Some("+J-E-206".into()),
                    };
                    let v = ItemValues {
                        page: "1".into(),
                        row: Some("H".into()),
                        column: Some("16".into()),
                        ..ItemValues::default()
                    };
                    item(format, &v, &o)
                }
            };
            assert_eq!(got, expected, "case {name}");
        }
    }

    /// Parse examples: (name, format, expected tokens).
    #[test]
    fn parse_examples() {
        use XrefToken::{Column, List, Lit, Page, Row};
        // The QET preset equals the brace preset.
        assert_eq!(
            parse_format("%F-%l%c"),
            parse_format("{page}-{row}{column}"),
            "case qet preset equals brace preset"
        );
        let cases: [(&str, &str, Vec<XrefToken>); 6] = [
            (
                "brace preset",
                "{page}-{row}{column}",
                vec![Page, Lit("-".into()), Row, Column],
            ),
            // `%%` is a percent sign, and literals merge.
            ("percent percent inside", "a%%b", vec![Lit("a%b".into())]),
            ("percent percent alone", "%%", vec![Lit("%".into())]),
            ("literals merge", "a{foo}%%%q{x", vec![Lit("a{foo}%%q{x".into())]),
            ("empty", "", vec![]),
            ("list token and unicode", "∞ {list}", vec![Lit("∞ ".into()), List]),
        ];
        for (name, format, expected) in cases {
            assert_eq!(parse_format(format), expected, "case {name}");
        }
    }
    #[test]
    fn qet_page_index_prints_position() {
        // FR-XRF-003
        assert_eq!(plain("%f-%l%c"), "6-A1");
        let none = ItemValues {
            page_index: None,
            ..values()
        };
        assert_eq!(item("{page_index}", &none, &ItemOptions::default()), "?");
    }

    #[test]
    fn full_structure_format_ignores_prefix() {
        let f = "{function}{location}/{page}.{row}{column}";
        assert_eq!(plain(f), "=P01+J-E-2416/102.A1");
        let o = ItemOptions {
            zone_only: false,
            prefix: Some("+X".into()),
        };
        assert_eq!(item(f, &values(), &o), "=P01+J-E-2416/102.A1");
        assert!(names_structure(&parse_format(f)));
        assert!(names_structure(&parse_format("%M%LM")));
        assert!(!names_structure(&parse_format("%F-%l%c")));
        assert_eq!(plain("%M %LM"), "=P01 +J-E-2416");
    }

    #[test]
    fn missing_cell_prints_question_marks() {
        let v = ItemValues {
            page: "102".into(),
            ..ItemValues::default()
        };
        assert_eq!(
            item("{page}-{row}{column}", &v, &ItemOptions::default()),
            "102-??"
        );
    }

    #[test]
    fn unknown_names_stay_literal() {
        assert_eq!(parse_format("{foo}"), vec![XrefToken::Lit("{foo}".into())]);
        assert_eq!(parse_format("{}"), vec![XrefToken::Lit("{}".into())]);
        assert_eq!(parse_format("%q"), vec![XrefToken::Lit("%q".into())]);
        assert_eq!(plain("a{foo}b%qc"), "a{foo}b%qc");
        assert_eq!(parse_format("100%"), vec![XrefToken::Lit("100%".into())]);
        assert_eq!(parse_format("%"), vec![XrefToken::Lit("%".into())]);
        assert_eq!(parse_format("%L"), vec![XrefToken::Lit("%L".into())]);
        assert_eq!(parse_format("%∞"), vec![XrefToken::Lit("%∞".into())]);
    }

    #[test]
    fn unclosed_brace_is_literal_to_the_end() {
        assert_eq!(
            parse_format("{page}-{page"),
            vec![XrefToken::Page, XrefToken::Lit("-{page".into())]
        );
        assert_eq!(parse_format("{"), vec![XrefToken::Lit("{".into())]);
    }

    #[test]
    fn lm_is_checked_before_l() {
        assert_eq!(parse_format("%LM"), vec![XrefToken::Location]);
        assert_eq!(parse_format("%l"), vec![XrefToken::Row]);
        assert_eq!(
            parse_format("%lM"),
            vec![XrefToken::Row, XrefToken::Lit("M".into())]
        );
    }

    #[test]
    fn zone_only_drops_the_separator_of_a_trailing_page_part() {
        // The page part behind the cell takes the literal in front of it, not the closing one.
        assert_eq!(zone("{row}{column}/{page}"), "H16");
        assert_eq!(zone("{row}{column}-{page_index}"), "H16");
        assert_eq!(zone("[{row}{column}/{page}]"), "[H16]");
        assert_eq!(zone("({page}-{row}{column})"), "(H16)");
        // Nothing but page parts: nothing is left.
        assert_eq!(zone("{page}"), "");
        assert_eq!(zone("{function}{location}/{page}"), "");
    }

    /// List matrices: (name, item count, per row, max, expected text, expected item ranges if checked, item count).
    #[test]
    fn matrix_cases() {
        #[rustfmt::skip]
        let cases = [
            ("3x9 of 2", 2, 3, 9, "a, b", Some(vec![0..1, 3..4]), 2),
            ("3x9 of 5", 5, 3, 9, "a, b, c\nd, e", None, 5),
            ("3x9 of 9", 9, 3, 9, "a, b, c\nd, e, f\ng, h, i", None, 9),
            ("3x9 of 11 is cut", 11, 3, 9, "a, b, c\nd, e, f\ng, h, i, …", None, 9),
            ("3x9 of none", 0, 3, 9, "", Some(vec![]), 0),
            // Unlimited.
            ("one row, no max", 7, 0, 0, "a, b, c, d, e, f, g", None, 7),
            ("3 per row, no max", 7, 3, 0, "a, b, c\nd, e, f\ng", None, 7),
            ("one row, max 4 of 7", 7, 0, 4, "a, b, c, d, …", None, 4),
            ("one row, max 4 of 4", 4, 0, 4, "a, b, c, d", None, 4),
            ("1 per row, no max", 3, 1, 0, "a\nb\nc", None, 3),
        ];
        for (name, n, per_row, max, text, items, count) in cases {
            let r = render_list(&strings(n), &matrix(per_row, max));
            assert_eq!(r.text, text, "case {name}");
            assert_eq!(r.items.len(), count, "case {name}");
            if let Some(items) = items {
                assert_eq!(r.items, items, "case {name}");
            }
        }
    }

    #[test]
    fn cut_lists_carry_the_overflow_mark() {
        // A caller that formatted only the first `max` items says that more exist.
        let m = matrix(3, 4);
        let r = render_list_more(&strings(4), true, &m);
        assert_eq!(
            r.text,
            "a, b, c
d, …"
        );
        assert_eq!(r.items.len(), 4);
        assert_eq!(
            render_list_more(&strings(4), false, &m).text,
            "a, b, c
d"
        );
        assert_eq!(render_list_more(&[], false, &m), Rendered::default());
    }

    /// Wrappers around a rendered list: (name, list, wrapper format, expected text, expected item ranges).
    #[test]
    fn wrapper_cases() {
        let example = Rendered {
            text: "3-B2, 106-G3".into(),
            items: vec![0..4, 6..12],
        };
        let ab = Rendered {
            text: "a, b".into(),
            items: vec![0..1, 3..4],
        };
        let cases = [
            (
                "the example",
                &example,
                "∞ {list}",
                "∞ 3-B2, 106-G3",
                vec![4..8, 10..16],
            ),
            ("no list token", &ab, "x{page}y", "xy", vec![]),
            // The items are those of the first list token.
            (
                "two list tokens",
                &ab,
                "{list}|{list}",
                "a, b|a, b",
                vec![0..1, 3..4],
            ),
        ];
        for (name, list, format, text, items) in cases {
            let w = render_wrapper(&parse_format(format), list);
            assert_eq!(w.text, text, "case {name}");
            assert_eq!(w.items, items, "case {name}");
        }
    }

    proptest! {
        #[test]
        fn parse_and_render_never_panic(s in "\\PC*") {
            let tokens = parse_format(&s);
            for zone_only in [false, true] {
                for prefix in [None, Some("+J".to_string())] {
                    let o = ItemOptions { zone_only, prefix };
                    let _ = render_item(&tokens, &ItemValues::default(), &o);
                }
            }
            let _ = names_structure(&tokens);
            let _ = render_wrapper(&tokens, &Rendered::default());
        }

        #[test]
        fn literal_formats_render_unchanged(s in "[^{}%]*") {
            let tokens = parse_format(&s);
            prop_assert_eq!(render_item(&tokens, &values(), &ItemOptions::default()), s.clone());
            let o = ItemOptions { zone_only: true, prefix: None };
            prop_assert_eq!(render_item(&tokens, &values(), &o), s);
        }

        /// The item ranges of a list and of a wrapper around it are ordered and slice back to the items, over
        /// random items and the fixed sets (empty, Unicode and repeated items), random matrices and the 255
        /// limits, and random wrapper literals.
        #[test]
        fn ranges_slice_back(
            items in prop_oneof![
                proptest::collection::vec("\\PC{0,6}", 0..30),
                Just(vec!["3-B2".to_string(), "106-G3".into(), "∞".into(), String::new()]),
                Just(strings(20)),
                Just(vec!["+J-E-206/102.A1".to_string(); 7]),
            ],
            per_row in prop_oneof![0u8..6, Just(255u8)],
            max in prop_oneof![0u8..12, Just(255u8)],
            wrapper in "[^{}%]{0,5}",
        ) {
            let r = render_list(&items, &matrix(per_row, max));
            let shown = if max == 0 { items.len() } else { items.len().min(usize::from(max)) };
            prop_assert_eq!(r.items.len(), shown);
            let mut last_end = 0;
            for (range, expected) in r.items.iter().zip(&items) {
                prop_assert!(range.start >= last_end);
                prop_assert!(range.start <= range.end);
                prop_assert_eq!(&r.text[range.clone()], expected.as_str());
                last_end = range.end;
            }
            let format = format!("{wrapper}{{list}}");
            let w = render_wrapper(&parse_format(&format), &r);
            prop_assert_eq!(w.items.len(), r.items.len());
            for (range, expected) in w.items.iter().zip(&items) {
                prop_assert_eq!(&w.text[range.clone()], expected.as_str());
            }
        }
    }
}
