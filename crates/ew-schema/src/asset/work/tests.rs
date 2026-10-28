// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FT-470 and FT-471: the work bounds' arithmetic, hostile documents generated here (no committed binaries), and
//! the real-exporter corpus kept under the limits.

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::super::{
    AssetError, MAX_SVG_ARC_PIECES, MAX_SVG_CSS_WORK, MAX_SVG_EXPANSION, MAX_SVG_MARKER_WORK,
    MAX_SVG_PATH_SEGMENTS, SvgHazard, sniff, svg_hazard, svg_work,
};
use super::*;

const NS: &str =
    r#"xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 4 4""#;

use super::radius_pieces as pieces;

#[test]
fn path_segments_count_commands_moves_after_a_close_and_arc_pieces() {
    let cases: &[(&str, u64)] = &[
        ("", 0),
        ("M0 0 L1 1 L2 2", 3),
        // The further pairs of a move-to are line-tos.
        ("M0 0 1 1 2 2", 3),
        ("m0 0 l1 1", 2),
        ("M0 0 H1 V1 h1 v1", 5),
        ("M0 0 C1 1 2 2 3 3 S4 4 5 5 Q6 6 7 7 T8 8", 5),
        // A close-path, then a line-to: svgtypes inserts a move-to; a repeated close-path adds nothing.
        ("M0 0 L1 1 Z L2 2", 5),
        ("M0 0 L1 1 Z Z Z", 3),
        ("M0 0 L1 1 Z M2 2 L3 3", 5),
        // Arc flags written without separators.
        ("M0 0 a1 1 0 0110 0", 1 + pieces(5.0)),
        // A radius of at most 1e-5 is a line.
        ("M0 0 A0.00001 5 0 0 0 10 0", 2),
        // An arc whose radii fit its chord keeps them.
        ("M0 0 A5 5 0 0 1 10 0", 1 + pieces(5.0)),
    ];
    for (d, expected) in cases {
        assert_eq!(path_segments(d), *expected, "{d}");
    }
}

#[test]
fn arcs_scaled_up_over_their_chord_count_more_pieces_and_too_many_are_unbounded() {
    // rx = 1, ry = 1000 over a chord of 2000: kurbo scales both by 1000, an ellipse of radius 1e6.
    let scaled = path_segments("M0 0 A1 1000 0 0 0 2000 0");
    assert_eq!(scaled, 1 + pieces(1.0e6));
    assert!(pieces(1.0e6) > pieces(1000.0));
    // The bound follows the current point through relative commands.
    let relative = path_segments("m0 0 h2000 a1 1000 0 0 0 -2000 0");
    assert_eq!(relative, scaled + 1);
    // The largest radius a number may have stays within the per-arc cap.
    assert_eq!(pieces(1.0e9), 55);
    assert_eq!(
        path_segments("M0 0 A1000000000 1000000000 0 0 1 1000000000 0"),
        56
    );
    // kurbo scales rx = 2e-5, ry = 1e9 over a chord of 1e9 by 2.5e13: an ellipse of radius 2.5e22, about 4,000
    // cubics that svgtypes drains quadratically. The path is unbounded.
    assert!(pieces(2.5e22) > MAX_SVG_ARC_PIECES);
    assert_eq!(
        path_segments("M0 0 A0.00002 1000000000 0 0 0 1000000000 0"),
        u64::MAX
    );
}

#[test]
fn path_data_the_cursor_cannot_follow_is_bounded_roughly() {
    // A missing number: the rest is bounded by its digit runs and closes, plus one.
    assert_eq!(path_segments("M0 0 L1"), 1 + 2);
    // An arc that cannot be read cannot be shown to stay within the per-arc cap.
    assert_eq!(path_segments("M0 0 A1 1 0 2 0 5 5"), u64::MAX);
    // A number after a close-path stops svgtypes; the bound continues roughly.
    assert!(path_segments("M0 0 Z 1 1") >= 2);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The bound is total (no panic, no overflow) on any path data, and never below the commands a well-formed
    /// path of line-tos has.
    #[test]
    fn path_segments_is_total(d in "[MmLlHhVvCcSsQqTtAaZz0-9 ,.eE+-]{0,80}") {
        let _ = path_segments(&d);
    }

    #[test]
    fn line_paths_count_one_per_point(points in proptest::collection::vec((0u16..1000, 0u16..1000), 1..40)) {
        let d: String = points.iter().enumerate()
            .map(|(k, (x, y))| format!("{}{x} {y} ", if k == 0 { "M" } else { "L" }))
            .collect();
        prop_assert_eq!(path_segments(&d), points.len() as u64);
    }
}

#[test]
fn lengths_are_bounded_in_user_units() {
    let cases: &[(&str, f64)] = &[
        ("5", 5.0),
        (" 3 ", 3.0),
        ("5px", 5.0),
        ("1in", 96.0),
        ("-2pc", 32.0),
        ("1e9in", 9.6e10),
        ("auto", 0.0),
        ("", 0.0),
        ("2em", f64::from(f32::MAX)),
        ("50%", f64::from(f32::MAX)),
        ("x", f64::from(f32::MAX)),
    ];
    for (value, expected) in cases {
        assert_eq!(length_bound(value), *expected, "{value}");
    }
}

/// A shape's local attribute names and values, as the scanner hands them over.
type Attrs = Vec<(&'static str, String)>;

#[test]
fn shapes_count_their_segments() {
    let attrs = |pairs: &[(&'static str, &str)]| -> Attrs {
        pairs.iter().map(|(a, v)| (*a, (*v).to_owned())).collect()
    };
    let quarter = pieces(5.0);
    let cases: Vec<(&str, Attrs, u64)> = vec![
        ("line", attrs(&[("x2", "1")]), 2),
        ("polyline", attrs(&[("points", "0,0 1,1 2,2")]), 6),
        ("polygon", attrs(&[("points", "0,0 1,1 2,2")]), 7),
        ("rect", attrs(&[("width", "4"), ("height", "4")]), 5),
        ("rect", attrs(&[("rx", "5")]), 4 * quarter + 6),
        ("circle", attrs(&[("r", "5")]), 4 * quarter + 2),
        ("ellipse", attrs(&[("rx", "1"), ("ry", "5")]), 4 * quarter + 2),
        ("path", attrs(&[("d", "M0 0 L1 1")]), 2),
        ("g", attrs(&[("d", "M0 0 L1 1")]), 0),
    ];
    for (name, attrs, expected) in &cases {
        assert_eq!(shape_segments(name, attrs), *expected, "{name}");
    }
    // A radius in `em` can be as large as f32 allows: millions of pieces.
    assert!(shape_segments("circle", &attrs(&[("r", "1em")])) > 10_000_000);
}

#[test]
fn declarations_name_their_markers_per_property() {
    let t = |s: &str| vec![s.to_owned()];
    let none: Vec<String> = Vec::new();
    let cases: Vec<(&str, MarkerTargets)> = vec![
        ("fill:url(#g)", [none.clone(), none.clone(), none.clone()]),
        (
            "marker-end:url(#a);fill:red",
            [none.clone(), none.clone(), t("a")],
        ),
        (
            "marker-start: url(#s); marker-mid:url(#m)",
            [t("s"), t("m"), none.clone()],
        ),
        ("marker:url(#m)", [t("m"), t("m"), t("m")]),
        // A comment can hide a declaration boundary: every target counts for every property.
        ("marker-start:/*;fill:*/url(#a)", [t("a"), t("a"), t("a")]),
        ("marker-end:url(#a;b)", [t("a;b"), t("a;b"), t("a;b")]),
    ];
    for (text, expected) in &cases {
        assert_eq!(declaration_markers(text), *expected, "{text}");
    }
    let mut out: MarkerTargets = Default::default();
    attribute_markers("marker", "url(#m)", &mut out);
    attribute_markers("marker-end", "url(#e)", &mut out);
    attribute_markers("fill", "url(#f)", &mut out);
    assert_eq!(out, [t("m"), t("m"), vec!["m".to_owned(), "e".to_owned()]]);
}

#[test]
fn plain_groups_have_exact_shapes() {
    let c = |text: &str| compound_of(text);
    assert_eq!(
        group_shape("g a > b + c"),
        Shape::Exact(vec![
            (Comb::First, c("g")),
            (Comb::Descendant, c("a")),
            (Comb::Child, c("b")),
            (Comb::Adjacent, c("c")),
        ])
    );
    assert_eq!(
        group_shape("a ~ b").clone(),
        Shape::Exact(vec![(Comb::First, c("a")), (Comb::Any, c("b"))])
    );
    assert_eq!(
        c(".a.b[x=y]:first-child"),
        Compound {
            len: 21,
            named: vec!["class".into(), "class".into(), "x".into()],
            pseudo: 1
        }
    );
    // A quote, a parenthesis or a backslash can hide white space inside a compound.
    assert!(matches!(
        group_shape(r#"[x="a b"] c"#),
        Shape::Uniform { parts: 3, .. }
    ));
    assert!(matches!(group_shape("a:lang(x y) b"), Shape::Uniform { .. }));
    assert_eq!(last_class(".a .b"), Some("b".into()));
    assert_eq!(last_class("g.c:first-child"), Some("c".into()));
    assert_eq!(last_class(".a > g"), None);
    assert_eq!(last_class(".caf\u{e9}"), None);
}

#[test]
fn selector_spans_end_where_a_valid_selector_can_end() {
    assert_eq!(selector_spans(".a{fill:red}.b,.c{x:y}"), vec![".a", ".b", ".c"]);
    // A body cannot hold a valid selector: it ends at `;` or `}`.
    assert_eq!(selector_spans("a{b c d e;f g h}"), vec!["a"]);
    // An attribute value string keeps its commas and braces inside the span.
    assert_eq!(
        selector_spans(r#"a a [x=",}"] b b{}"#),
        vec![r#"a a [x=",}"] b b"#]
    );
    assert_eq!(selector_spans("a:lang({) b{}"), vec!["a:lang({) b"]);
    // `@`, `%`, `(` and quotes outside brackets fail a selector.
    assert_eq!(
        selector_spans("@keyframes k{50%{o:0}}.x{}"),
        vec!["keyframes k", ".x"]
    );
    // A selector may also end at a `/` or the end.
    assert_eq!(selector_spans("a b/c d"), vec!["a b", "c d"]);
    // Unclosed brackets and strings inside brackets are cuts (each found by one look-up).
    assert_eq!(selector_spans("a [x b"), vec!["x b"]);
    assert_eq!(selector_spans(r#"a [x=" b] c{"#), vec![" b] c"]);
    assert_eq!(selector_spans(r#"a [x="]" b] c{"#), vec![r#"a [x="]" b] c"#]);
    assert!(selector_spans(&"[\"".repeat(20_000)).is_empty());
    let sheet = "/* x } a b c */ .d{} /* no landing */";
    let clean = " .d{} ";
    assert_eq!(sheet_selectors(sheet, clean), Some(vec![" .d", " a b c *"]));
    assert_eq!(sheet_selectors("/* } */*a{}", "*a{}"), None);
    assert_eq!(sheet_selectors(r"\61{}", r"\61{}"), None);
    assert!(may_hold_font_shorthand("font: 12px serif"));
    assert!(may_hold_font_shorthand("x{font/**/:a}"));
    assert!(!may_hold_font_shorthand("font-family: a; myfont: b"));
}

/// An element of the matching oracle: a chain of elements where every chain element but the root has one
/// previous sibling (a leaf).
#[derive(Clone, Copy)]
enum Model {
    Chain(usize),
    Sibling(usize),
}

impl Model {
    fn parent(self) -> Option<Model> {
        match self {
            Model::Chain(0) => None,
            Model::Chain(k) | Model::Sibling(k) => Some(Model::Chain(k - 1)),
        }
    }

    fn previous(self) -> Option<Model> {
        match self {
            Model::Chain(k) if k > 0 => Some(Model::Sibling(k)),
            _ => None,
        }
    }
}

/// simplecss 0.2.2's `matches_impl` on the model, the first compound never matching and every other always (the
/// most backtracking), counting one step per compound tried and per parent or sibling fetched.
fn simulate(combs: &[Comb], idx: usize, at: Model, steps: &mut u64) -> bool {
    *steps += 1;
    if idx == 0 {
        return false;
    }
    match combs[idx] {
        Comb::Descendant | Comb::Any => {
            let mut p = at.parent();
            while let Some(e) = p {
                *steps += 1;
                if simulate(combs, idx - 1, e, steps) {
                    return true;
                }
                p = e.parent();
            }
            false
        }
        Comb::Child => {
            *steps += 1;
            at.parent().is_some_and(|e| simulate(combs, idx - 1, e, steps))
        }
        Comb::Adjacent => {
            *steps += 1;
            at.previous().is_some_and(|e| simulate(combs, idx - 1, e, steps))
        }
        Comb::First => unreachable!(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The selector cost is an upper bound of simplecss's backtracking (oracle: the matcher simulated on a chain).
    #[test]
    fn selector_costs_bound_the_simulated_matcher(
        combs in proptest::collection::vec(
            prop_oneof![Just(Comb::Descendant), Just(Comb::Child), Just(Comb::Adjacent)], 0..6),
        depth in 0usize..13,
        any in any::<bool>(),
    ) {
        let mut all = vec![Comb::First];
        all.extend(combs);
        let empty = BTreeMap::new();
        let stats = Stats { attrs: 0, value_len: &empty, sibling_walk: 1 };
        // Every compound costs one step: no text, no attribute reads.
        let unit = Compound::default();
        let shape = Shape::Exact(all.iter().enumerate()
            .map(|(i, c)| (if any && i > 0 { Comb::Any } else { *c }, unit.clone()))
            .collect());
        let mut steps = 0;
        simulate(&all, all.len() - 1, Model::Chain(depth), &mut steps);
        let bound = selector_costs(&shape, &stats)[depth];
        prop_assert!(steps <= bound, "{} > {}", steps, bound);
        let uniform = Shape::Uniform { parts: all.len() as u64, compound: unit };
        prop_assert!(steps <= selector_costs(&uniform, &stats)[depth]);
    }
}

#[test]
fn descendant_costs_are_binomial_and_saturate() {
    let empty = BTreeMap::new();
    let stats = Stats {
        attrs: 0,
        value_len: &empty,
        sibling_walk: 1,
    };
    // `x g g g g g` against an element with 63 ancestors: more than C(63, 5) = 7,028,847 calls.
    let costs = selector_costs(&group_shape("x g g g g g"), &stats);
    assert!(costs[63] > 7_028_847, "{}", costs[63]);
    // One step, two bytes of text, one class read.
    assert_eq!(selector_costs(&group_shape(".a"), &stats)[63], 4);
    // Fifty descendant combinators saturate rather than overflow.
    let deep = vec!["g"; 50].join(" ");
    assert_eq!(selector_costs(&uniform(&deep), &stats)[63], u64::MAX);
}

fn polylines(count: usize, attrs: &str) -> String {
    let points: String = (0..100).map(|k| format!("{k},{k} ")).collect();
    (0..count)
        .map(|_| format!(r#"<polyline points="{points}" {attrs}/>"#))
        .collect()
}

/// FT-470: marker applications over inheriting shapes and their vertices are bounded before usvg runs.
#[test]
fn marker_applications_beyond_the_budget_are_refused() {
    let marker = r#"<defs><marker id="m"><path d="M0 0 L1 1"/></marker></defs>"#;
    let big_marker = format!(
        r#"<defs><marker id="m">{}</marker></defs>"#,
        r#"<path d="M0 0 L1 1"/>"#.repeat(400)
    );
    let vertices = |n: usize| -> String {
        let mut d = String::from("M0 0");
        for k in 1..n {
            d.push_str(&format!(" L{k} {}", k % 2));
        }
        d
    };
    let arcs = " A0.00002 1000000000 0 0 0 1000000000 0 A0.00002 1000000000 0 0 0 0 0".repeat(10);
    let cases: Vec<(&str, String, Option<SvgHazard>)> = vec![
        (
            "600 polylines of 100 points under an inherited marker-mid",
            format!(
                r#"<svg {NS}>{marker}<g marker-mid="url(#m)">{}</g></svg>"#,
                polylines(600, "")
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "half as many stay within the budget",
            format!(
                r#"<svg {NS}>{marker}<g marker-mid="url(#m)">{}</g></svg>"#,
                polylines(300, "")
            ),
            None,
        ),
        (
            "an end marker is one application per shape",
            format!(
                r#"<svg {NS}>{marker}<g marker-end="url(#m)">{}</g></svg>"#,
                polylines(600, "")
            ),
            None,
        ),
        (
            "a marker-mid through a class rule",
            format!(
                r#"<svg {NS}><style>.k {{ marker-mid: url(#m) }}</style>{marker}{}</svg>"#,
                polylines(600, r#"class="k""#)
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "a marker-mid in a style attribute",
            format!(
                r#"<svg {NS}>{marker}<g style="marker-mid:url(#m)">{}</g></svg>"#,
                polylines(600, "")
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "nested content: 600 vertices times a marker of 401 elements",
            format!(
                r#"<svg {NS}>{big_marker}<path d="{}" marker-mid="url(#m)"/></svg>"#,
                vertices(600)
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "400 vertices times the same marker stay within the budget",
            format!(
                r#"<svg {NS}>{big_marker}<path d="{}" marker-mid="url(#m)"/></svg>"#,
                vertices(400)
            ),
            None,
        ),
        (
            "shapes converted again under a use that inherits a marker",
            format!(
                r##"<svg {NS}>{marker}<defs><g id="lines">{}</g></defs><g marker-mid="url(#m)"><use href="#lines"/></g></svg>"##,
                polylines(300, "")
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "twenty arcs of a tiny radius over a long chord",
            format!(r#"<svg {NS}>{marker}<path d="M0 0{arcs}" marker-mid="url(#m)"/></svg>"#),
            Some(SvgHazard::TooComplex),
        ),
        (
            "Visio arrowheads through a class rule",
            format!(
                r##"<svg {NS}><style>.st2 {{ marker-end: url(#m) }} .st3 {{ fill: #00f }}</style><defs><g id="lend"><path d="M1 -1 L0 0 L1 1"/></g><marker id="m" class="st3"><use xlink:href="#lend"/></marker></defs>{}</svg>"##,
                r#"<path class="st2" d="M0 0 L10 0 L10 10"/>"#.repeat(200)
            ),
            None,
        ),
    ];
    for (what, text, expected) in &cases {
        assert_eq!(svg_hazard(text.as_bytes()), *expected, "{what}");
    }
    // The refusal is the parser-burden one, with its re-export advice.
    let refused = &cases[0].1;
    let text = sniff(refused.as_bytes()).unwrap_err().to_string();
    assert_eq!(text, AssetError::UnsafeSvg(SvgHazard::TooComplex).to_string());
    assert!(
        text.contains("fewer nested groups, clones, markers and style rules"),
        "{text}"
    );
}

/// The expansion counts the references inside elements with an id that nothing references: 1,000 such wrappers
/// each converting a 1,001-element group are a million elements, past [`MAX_SVG_EXPANSION`] (the bound summed
/// only the references outside every id before).
#[test]
fn references_inside_unreferenced_ids_count_toward_the_expansion() {
    let document = |wrappers: usize| -> String {
        let mut t = format!(r#"<svg {NS}><defs><g id="big">"#);
        for k in 0..1000 {
            t.push_str(&format!(r#"<path id="p{k}" d="M0 0 H1"/>"#));
        }
        t.push_str("</g></defs>");
        for k in 0..wrappers {
            t.push_str(&format!(r##"<g id="w{k}"><use href="#big"/></g>"##));
        }
        t.push_str("</svg>");
        t
    };
    assert_eq!(svg_hazard(document(1000).as_bytes()), Some(SvgHazard::TooComplex));
    let work = svg_work(document(100).as_bytes()).unwrap();
    // 2 + 1,001 + 200 elements, and 100 conversions of the 1,001-element group.
    assert_eq!(work.expansion, 1_203 + 100 * 1_001);
    assert!(work.expansion <= MAX_SVG_EXPANSION);
}

fn nested(depth: usize, inner: &str) -> String {
    format!("{}{inner}{}", "<g>".repeat(depth), "</g>".repeat(depth))
}

/// FT-471: selector matching and declaration application beyond the budget are refused before simplecss runs.
#[test]
fn style_work_beyond_the_budget_is_refused() {
    let leaf = r#"<path d="M0 0 H1"/>"#;
    let descendant_rules = vec!["x g g g g g"; 400].join(", ");
    let hidden_comma = r#"a a a a a [x=","] a a a a a"#;
    let stars = vec!["*"; 4000].join(",");
    let sixty: String = (0..60).map(|k| format!("p{k}: red; ")).collect();
    let style = "fill:red;".repeat(50_000);
    let cases: Vec<(&str, String, Option<SvgHazard>)> = vec![
        (
            "400 six-part descendant selectors against a chain of 60",
            format!(
                r#"<svg {NS}><style>{descendant_rules} {{ fill: red }}</style>{}</svg>"#,
                nested(60, leaf)
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "one of them stays within the budget",
            format!(
                r#"<svg {NS}><style>x g g g g g {{ fill: red }}</style>{}</svg>"#,
                nested(60, leaf)
            ),
            None,
        ),
        (
            "a comma inside an attribute string hides eleven compounds from the six-part cap",
            format!(
                r#"<svg {NS}><style>{hidden_comma} {{ fill: red }}</style>{}</svg>"#,
                nested(60, r#"<a x=","/>"#)
                    .replace("<g>", "<a>")
                    .replace("</g>", "</a>")
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "4,000 universal groups of 60 declarations applied to 2,000 elements",
            format!(
                r#"<svg {NS}><style>{stars} {{ {sixty} }}</style>{}</svg>"#,
                leaf.repeat(2_000)
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "a style attribute of 50,000 declarations converted 10,000 times",
            format!(
                r##"<svg {NS}><defs><g id="s"><path d="M0 0 H1" style="{style}"/></g></defs>{}</svg>"##,
                r##"<use href="#s"/>"##.repeat(10_000)
            ),
            Some(SvgHazard::TooComplex),
        ),
    ];
    for (what, text, expected) in &cases {
        assert_eq!(svg_hazard(text.as_bytes()), *expected, "{what}");
    }
}

/// FT-470 follow-up: the path segments of every shape conversion, markers or not, are bounded before usvg runs.
#[test]
fn path_segments_beyond_the_budget_are_refused() {
    let large_arcs =
        "A1000000000 1000000000 0 0 1 1000000000 0 A1000000000 1000000000 0 0 1 0 0 ".repeat(40_000);
    let circles: String = (0..2_000)
        .map(|k| format!(r#"<circle cx="{k}" cy="{k}" r="5"/><rect width="9" height="9" rx="2"/>"#))
        .collect();
    let mut wrapped = format!(r#"<svg {NS}><defs><g id="lines">"#);
    wrapped.push_str(&polylines(100, ""));
    wrapped.push_str("</g></defs>");
    wrapped.push_str(&r##"<use href="#lines"/>"##.repeat(250));
    wrapped.push_str("</svg>");
    let cases: Vec<(&str, String, Option<SvgHazard>)> = vec![
        (
            "one arc of a tiny radius over a long chord",
            format!(
                r#"<svg {NS}><path d="M0 0 A0.00002 1000000000 0 0 0 1000000000 0" fill="black"/></svg>"#
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "a circle whose radius in em nested font sizes make as large as f32 allows",
            format!(
                r#"<svg {NS}><g font-size="1000000000"><g font-size="1000000000em"><g font-size="1000000000em"><g font-size="1000000000em"><circle r="340em"/></g></g></g></g></svg>"#
            ),
            Some(SvgHazard::TooComplex),
        ),
        (
            "80,000 arcs of the largest radius",
            format!(r#"<svg {NS}><path d="M0 0 {large_arcs}" fill="black"/></svg>"#),
            Some(SvgHazard::TooComplex),
        ),
        (
            "100 polylines of 100 points converted 251 times through use",
            wrapped,
            Some(SvgHazard::TooComplex),
        ),
        (
            "2,000 circles and 2,000 rounded rectangles",
            format!(r#"<svg {NS}>{circles}</svg>"#),
            None,
        ),
    ];
    for (what, text, expected) in &cases {
        assert_eq!(svg_hazard(text.as_bytes()), *expected, "{what}");
    }
    let work = svg_work(cases[4].1.as_bytes()).unwrap();
    assert_eq!(
        work.segments,
        2_000 * (4 * pieces(5.0) + 2) + 2_000 * (4 * pieces(2.0) + 6)
    );
}

/// The real-exporter corpus (FT-418) stays under both limits with the margins `MAX_SVG_MARKER_WORK` and
/// `MAX_SVG_CSS_WORK` document: the largest bounds are pinned here, so a change of the arithmetic shows.
#[test]
fn the_exporter_corpus_stays_far_below_the_work_limits() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/svg-exporters");
    let mut largest: BTreeMap<&str, (u64, String)> = BTreeMap::new();
    let mut accepted = 0;
    for dir in std::fs::read_dir(&root).unwrap() {
        let dir = dir.unwrap().path();
        if !dir.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&dir).unwrap() {
            let file = file.unwrap().path();
            let Ok(work) = svg_work(&std::fs::read(&file).unwrap()) else {
                continue;
            };
            accepted += 1;
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            for (bound, value) in [
                ("segments", work.segments),
                ("markers", work.markers),
                ("css", work.css),
            ] {
                let entry = largest.entry(bound).or_insert((0, String::new()));
                if value > entry.0 {
                    *entry = (value, name.clone());
                }
            }
        }
    }
    assert_eq!(accepted, 24);
    assert_eq!(
        largest["segments"],
        (33_896, "beetle-range-css-hex.svg".to_owned())
    );
    assert!(largest["segments"].0 * 120 < MAX_SVG_PATH_SEGMENTS);
    assert_eq!(largest["markers"], (48, "spaghetti-markers.svg".to_owned()));
    assert_eq!(
        largest["css"],
        (430_667_782, "treaty-map-55-groups.svg".to_owned())
    );
    assert!(largest["markers"].0 * 4_000 < MAX_SVG_MARKER_WORK);
    assert!(largest["css"].0 * 39 < MAX_SVG_CSS_WORK);
}
