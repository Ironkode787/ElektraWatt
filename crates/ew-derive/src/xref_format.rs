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
    /// `{page_index}` or QET `%f`: the 1-based position in page order.
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
/// [`zone_only_item`]; `prefix` names the target's structure in front of the first value (ignored for a
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

    #[test]
    fn qet_preset_equals_brace_preset() {
        assert_eq!(parse_format("%F-%l%c"), parse_format("{page}-{row}{column}"));
        assert_eq!(
            parse_format("{page}-{row}{column}"),
            vec![
                XrefToken::Page,
                XrefToken::Lit("-".into()),
                XrefToken::Row,
                XrefToken::Column
            ]
        );
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
    fn eplan_presets() {
        assert_eq!(plain("/{page}.{column}"), "/102.1");
        assert_eq!(plain("/{page}.{row}{column}"), "/102.A1");
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
    fn percent_percent_and_literal_merge() {
        assert_eq!(parse_format("a%%b"), vec![XrefToken::Lit("a%b".into())]);
        assert_eq!(parse_format("%%"), vec![XrefToken::Lit("%".into())]);
        assert_eq!(
            parse_format("a{foo}%%%q{x"),
            vec![XrefToken::Lit("a{foo}%%q{x".into())]
        );
        assert_eq!(parse_format(""), Vec::<XrefToken>::new());
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
    fn list_token_and_unicode() {
        assert_eq!(
            parse_format("∞ {list}"),
            vec![XrefToken::Lit("∞ ".into()), XrefToken::List]
        );
        assert_eq!(plain("∞ {page}"), "∞ 102");
        // A list token in an item format prints nothing.
        assert_eq!(plain("a{list}b"), "ab");
    }

    #[test]
    fn zone_only_examples() {
        assert_eq!(zone("{page}-{row}{column}"), "H16");
        assert_eq!(zone("/{page}.{row}{column}"), "/H16");
        assert_eq!(zone("%F-%l%c"), "H16");
        assert_eq!(zone("{function}{location}/{page}.{row}{column}"), "H16");
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

    #[test]
    fn prefix_examples() {
        assert_eq!(
            prefixed("/{page}.{row}{column}", "102", "A", "1"),
            "+J-E-206/102.A1"
        );
        assert_eq!(
            prefixed("({page}-{row}{column})", "201", "E", "1"),
            "(+J-E-206/201-E1)"
        );
        assert_eq!(
            prefixed("{page}-{row}{column}", "102", "A", "1"),
            "+J-E-206/102-A1"
        );
        assert_eq!(prefixed("x/{page}", "102", "A", "1"), "x+J-E-206/102");
        // No value token: no prefix.
        assert_eq!(prefixed("abc", "102", "A", "1"), "abc");
        // First value token is a row: the prefix goes in front of it.
        assert_eq!(prefixed("[{row}]", "102", "A", "1"), "[+J-E-206/A]");
        // Zone-only ignores the prefix.
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
        assert_eq!(item("{page}-{row}{column}", &v, &o), "H16");
    }

    #[test]
    fn matrix_examples() {
        let m = matrix(3, 9);
        let r = render_list(&strings(2), &m);
        assert_eq!(r.text, "a, b");
        assert_eq!(r.items, vec![0..1, 3..4]);
        assert_eq!(render_list(&strings(5), &m).text, "a, b, c\nd, e");
        assert_eq!(render_list(&strings(9), &m).text, "a, b, c\nd, e, f\ng, h, i");
        let r = render_list(&strings(11), &m);
        assert_eq!(r.text, "a, b, c\nd, e, f\ng, h, i, …");
        assert_eq!(r.items.len(), 9);
        let empty = render_list(&[], &m);
        assert_eq!(empty, Rendered::default());
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

    #[test]
    fn matrix_unlimited() {
        assert_eq!(
            render_list(&strings(7), &matrix(0, 0)).text,
            "a, b, c, d, e, f, g"
        );
        assert_eq!(
            render_list(&strings(7), &matrix(3, 0)).text,
            "a, b, c\nd, e, f\ng"
        );
        assert_eq!(render_list(&strings(7), &matrix(0, 4)).text, "a, b, c, d, …");
        assert_eq!(render_list(&strings(4), &matrix(0, 4)).text, "a, b, c, d");
        assert_eq!(render_list(&strings(3), &matrix(1, 0)).text, "a\nb\nc");
    }

    #[test]
    fn wrapper_example() {
        let list = Rendered {
            text: "3-B2, 106-G3".into(),
            items: vec![0..4, 6..12],
        };
        let w = render_wrapper(&parse_format("∞ {list}"), &list);
        assert_eq!(w.text, "∞ 3-B2, 106-G3");
        assert_eq!(w.items, vec![4..8, 10..16]);
    }

    #[test]
    fn wrapper_edge_cases() {
        let list = Rendered {
            text: "a, b".into(),
            items: vec![0..1, 3..4],
        };
        let none = render_wrapper(&parse_format("x{page}y"), &list);
        assert_eq!(none.text, "xy");
        assert!(none.items.is_empty());
        let twice = render_wrapper(&parse_format("{list}|{list}"), &list);
        assert_eq!(twice.text, "a, b|a, b");
        assert_eq!(twice.items, vec![0..1, 3..4]);
    }

    #[test]
    fn ranges_slice_back_for_varied_inputs() {
        let sets: [Vec<String>; 3] = [
            vec!["3-B2".into(), "106-G3".into(), "∞".into(), String::new()],
            strings(20),
            vec!["+J-E-206/102.A1".into(); 7],
        ];
        for items in &sets {
            for (per_row, max) in [(0, 0), (3, 9), (1, 2), (4, 0), (0, 5), (255, 255)] {
                let r = render_list(items, &matrix(per_row, max));
                for (range, expected) in r.items.iter().zip(items) {
                    assert_eq!(&r.text[range.clone()], expected);
                }
                let w = render_wrapper(&parse_format("∞ {list}"), &r);
                for (range, expected) in w.items.iter().zip(items) {
                    assert_eq!(&w.text[range.clone()], expected);
                }
            }
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

        #[test]
        fn list_ranges_are_ordered_and_slice_back(
            items in proptest::collection::vec("\\PC{0,6}", 0..30),
            per_row in 0u8..6,
            max in 0u8..12,
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
        }

        #[test]
        fn wrapper_ranges_slice_back(
            items in proptest::collection::vec("\\PC{0,6}", 0..20),
            wrapper in "[^{}%]{0,5}",
        ) {
            let list = render_list(&items, &matrix(3, 9));
            let format = format!("{wrapper}{{list}}");
            let w = render_wrapper(&parse_format(&format), &list);
            prop_assert_eq!(w.items.len(), list.items.len());
            for (range, expected) in w.items.iter().zip(&items) {
                prop_assert_eq!(&w.text[range.clone()], expected.as_str());
            }
        }
    }
}
