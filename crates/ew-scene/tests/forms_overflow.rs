// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cell overflow policies, `visible_if`, rotated text and logo placement of `lower_form`.

// A wall-clock timer of this test, asserted only with EW_PERF_ASSERT (the timing rule of doc 18 §3.16); no timestamp reaches a compared result.
#![allow(clippy::disallowed_methods)]

use std::time::Instant;

use ew_dl::{DisplayList, ImageFormat, ItemKind, ShapedRun, Transform};
use ew_scene::forms::{
    AssetRef, BindingFinding, BoundValue, CELL_PADDING, FormFinding, OverflowKind, PageBindings, lower_form,
};
use ew_schema::FormDef;
use ew_text::{builtin, bundled};
use ew_units::{Angle2, Nm, Pt2, Rect};
use serde_json::{Value, json};

/// A title-block form of the given sheet size with the given cells and graphics.
fn form_with(width: i64, height: i64, cells: Value, graphics: Value) -> FormDef {
    serde_json::from_value(json!({
        "kind": "title_block",
        "sheet": {"width": width, "height": height},
        "cells": cells,
        "graphics": graphics,
    }))
    .expect("form")
}

/// One cell over the whole sheet, bound to `{page.title}`; `extra` members are set on the cell.
fn cell_form(width: i64, height: i64, style: &str, align: &str, overflow: Value, extra: Value) -> FormDef {
    let mut cell = json!({
        "rect": [[0, 0], [width, height]],
        "value": "{page.title}",
        "style": style,
        "align": align,
        "overflow": overflow,
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        cell[k] = v.clone();
    }
    form_with(width, height, json!([cell]), json!([]))
}

fn bindings(title: &str) -> PageBindings {
    let mut b = PageBindings::new();
    b.text("page.title", title).unwrap();
    b
}

fn runs(list: &DisplayList) -> Vec<&ShapedRun> {
    list.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some(&**r),
            _ => None,
        })
        .collect()
}

fn overflow_kinds(findings: &[FormFinding]) -> Vec<OverflowKind> {
    findings
        .iter()
        .filter_map(|f| match f {
            FormFinding::Overflow { kind, .. } => Some(*kind),
            _ => None,
        })
        .collect()
}

const SHRINK_1_2: fn() -> Value = || json!({"shrink_to_fit": {"min_cap_height": 1_200_000}});

// -- ellipsis ---------------------------------------------------------------------------------------------

#[test]
fn ellipsis_cuts_and_reports() {
    let text = "Power Supply 1 - Main Current and a long continuation";
    let form = cell_form(
        40_000_000,
        8_000_000,
        "text.normal",
        "middle_left",
        json!("ellipsis"),
        json!({}),
    );
    let out = lower_form(&form, None, &bindings(text), bundled());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::Truncated]);
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    assert!(r[0].text.ends_with('\u{2026}'));
    assert!(text.starts_with(r[0].text.trim_end_matches('\u{2026}')));
    // The shown text fits the inset width; one more character would not (checked by the unit test).
    let adv = bundled()
        .shape(&r[0].text, &builtin().resolve("text.normal").unwrap(), None)
        .advance();
    assert!(adv.0 <= 40_000_000 - 2 * CELL_PADDING.0);
    // Text that fits is not touched.
    let out = lower_form(&form, None, &bindings("Short"), bundled());
    assert!(out.findings.is_empty());
    assert_eq!(runs(&out.display_list)[0].text, "Short");
}

#[test]
#[ignore = "perf: release-profile timing, milestone deliveries only"]
fn ellipsis_of_20000_chars_is_fast() {
    let text = "Ab c".repeat(5_000);
    assert_eq!(text.chars().count(), 20_000);
    let form = cell_form(
        60_000_000,
        8_000_000,
        "text.normal",
        "middle_left",
        json!("ellipsis"),
        json!({}),
    );
    let t = std::env::var_os("EW_PERF_ASSERT").is_some().then(Instant::now);
    let out = lower_form(&form, None, &bindings(&text), bundled());
    // Timing acquisition, reporting and the bound are opt-in.
    if let Some(t) = t {
        let elapsed = t.elapsed();
        eprintln!("ellipsis 20000 chars: {elapsed:?}");
        assert!(elapsed.as_secs_f64() < 2.0, "took {elapsed:?}");
    }
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::Truncated]);
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    assert!(r[0].text.ends_with('\u{2026}'));
    assert!(r[0].text.len() < text.len());
}

// -- error ------------------------------------------------------------------------------------------------

#[test]
fn error_policy_draws_the_text_unchanged_and_reports() {
    let text = "Power Supply 1 - Main Current and a long continuation";
    let form = cell_form(
        40_000_000,
        8_000_000,
        "text.normal",
        "middle_left",
        json!("error"),
        json!({}),
    );
    let out = lower_form(&form, None, &bindings(text), bundled());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::Error]);
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].text, text);
    assert_eq!(r[0].size, builtin().resolve("text.normal").unwrap().size);
}

// -- shrink_to_fit ----------------------------------------------------------------------------------------

#[test]
fn shrink_to_fit_reduces_the_size_until_it_fits() {
    let style = builtin().resolve("text.normal").unwrap();
    let form = cell_form(
        40_000_000,
        8_000_000,
        "text.normal",
        "middle_left",
        SHRINK_1_2(),
        json!({}),
    );
    let out = lower_form(&form, None, &bindings("Power Supply 1 - Main Current"), bundled());
    assert!(out.findings.is_empty(), "{:?}", out.findings);
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    assert!(r[0].size < style.size && r[0].size.0 > 0, "{:?}", r[0].size);
    let adv = bundled()
        .shape(&r[0].text, &style.with_size(r[0].size), None)
        .advance();
    assert!(adv.0 <= 40_000_000 - 2 * CELL_PADDING.0);
}

#[test]
fn shrink_to_fit_stops_at_the_minimum_cap_height_and_reports() {
    let style = builtin().resolve("text.normal").unwrap();
    let text = "A page title far too long for the cell of the title block. ".repeat(3);
    let form = cell_form(
        40_000_000,
        8_000_000,
        "text.normal",
        "middle_left",
        SHRINK_1_2(),
        json!({}),
    );
    let out = lower_form(&form, None, &bindings(&text), bundled());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::ShrinkLimit]);
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    // 1.2 mm cap height of Noto Sans (714 / 1000) is an em of about 1.68 mm.
    assert!((1_670_000..=1_690_000).contains(&r[0].size.0), "{:?}", r[0].size);
    assert!(r[0].size < style.size);
}

#[test]
fn shrink_to_fit_never_enlarges_a_small_style() {
    // cp.small is 1.322917 mm; a 1.2 mm cap height would need an em of about 1.68 mm, which is larger.
    let style = builtin().resolve("cp.small").unwrap();
    assert_eq!(style.size, Nm(1_322_917));
    let text = "A long text that cannot possibly fit into this small cell of a title block";
    let form = cell_form(
        20_000_000,
        8_000_000,
        "cp.small",
        "middle_left",
        SHRINK_1_2(),
        json!({}),
    );
    let out = lower_form(&form, None, &bindings(text), bundled());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::ShrinkLimit]);
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    assert!(r[0].size <= style.size, "{:?} > {:?}", r[0].size, style.size);
    assert_eq!(r[0].size, style.size);
}

// -- wrapping ---------------------------------------------------------------------------------------------

const WORDS: &str = "alpha beta gamma delta epsilon zeta eta theta iota kappa";

#[test]
fn wrapping_up_to_max_lines() {
    let max2 = json!({"ew.cell.max_lines": 2});
    // Fits in two lines at the full size: two runs, no finding.
    let form = cell_form(
        60_000_000,
        20_000_000,
        "text.normal",
        "top_left",
        json!("error"),
        max2.clone(),
    );
    let out = lower_form(&form, None, &bindings(WORDS), bundled());
    assert!(out.findings.is_empty(), "{:?}", out.findings);
    assert_eq!(runs(&out.display_list).len(), 2);
}

#[test]
fn wrapping_beyond_max_lines_follows_the_policy() {
    let max2 = json!({"ew.cell.max_lines": 2});
    let style = builtin().resolve("text.normal").unwrap();
    let text = WORDS;
    // error: drawn as is (more lines than allowed) and reported.
    let form = cell_form(
        40_000_000,
        30_000_000,
        "text.normal",
        "top_left",
        json!("error"),
        max2.clone(),
    );
    let out = lower_form(&form, None, &bindings(text), bundled());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::Error]);
    assert!(runs(&out.display_list).len() > 2);
    // shrink_to_fit: shrinks until two lines suffice.
    let form = cell_form(
        40_000_000,
        30_000_000,
        "text.normal",
        "top_left",
        SHRINK_1_2(),
        max2.clone(),
    );
    let out = lower_form(&form, None, &bindings(text), bundled());
    let r = runs(&out.display_list);
    assert!(r.len() <= 2, "{} runs", r.len());
    assert!(r[0].size < style.size);
    assert!(out.findings.is_empty(), "{:?}", out.findings);
    // shrink_to_fit with a minimum that is reached before two lines suffice: limit reported, more than two
    // lines drawn, size at the minimum (about 2.8 mm for a 2 mm cap height).
    let json = json!({"shrink_to_fit": {"min_cap_height": 2_000_000}});
    let form = cell_form(40_000_000, 30_000_000, "text.normal", "top_left", json, max2);
    let out = lower_form(&form, None, &bindings(text), bundled());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::ShrinkLimit]);
    let r = runs(&out.display_list);
    assert!(r.len() > 2);
    assert!((2_790_000..=2_810_000).contains(&r[0].size.0), "{:?}", r[0].size);
}

// -- zero-width cells -------------------------------------------------------------------------------------

#[test]
fn cell_narrower_than_its_insets_reports_and_draws_nothing() {
    // A 1 mm wide revision cell: narrower than the two 0.529 mm insets together.
    let form = cell_form(
        1_000_000,
        4_497_917,
        "data.small",
        "middle_left",
        SHRINK_1_2(),
        json!({}),
    );
    let out = lower_form(&form, None, &bindings("V3.0"), bundled());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::ShrinkLimit]);
    assert!(runs(&out.display_list).is_empty());
    // The other policies report under their own kind and also draw nothing.
    for (policy, kind) in [
        (json!("ellipsis"), OverflowKind::Truncated),
        (json!("error"), OverflowKind::Error),
    ] {
        let form = cell_form(
            1_000_000,
            4_497_917,
            "data.small",
            "middle_left",
            policy,
            json!({}),
        );
        let out = lower_form(&form, None, &bindings("V3.0"), bundled());
        assert_eq!(overflow_kinds(&out.findings), vec![kind]);
        assert!(runs(&out.display_list).is_empty());
    }
}

#[test]
fn shrinking_to_size_zero_draws_nothing() {
    // The inset width is 1 nm and there is no minimum cap height: the fitting size rounds to 0.
    let w = 2 * CELL_PADDING.0 + 1;
    let json = json!({"shrink_to_fit": {"min_cap_height": 0}});
    let form = cell_form(w, 4_497_917, "text.normal", "middle_left", json, json!({}));
    let out = lower_form(&form, None, &bindings("Wide text"), bundled());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::ShrinkLimit]);
    assert!(runs(&out.display_list).is_empty());
}

// -- visible_if -------------------------------------------------------------------------------------------

#[test]
fn visible_if_hides_a_cell_when_false() {
    let form = cell_form(
        40_000_000,
        8_000_000,
        "text.normal",
        "middle_left",
        json!("error"),
        json!({"visible_if": "{page.show}"}),
    );
    let with = |show: Option<&str>| {
        let mut b = bindings("Visible");
        if let Some(s) = show {
            b.text("page.show", s).unwrap();
        }
        lower_form(&form, None, &b, bundled())
    };
    for shown in ["true", "1", "yes"] {
        assert_eq!(runs(&with(Some(shown)).display_list).len(), 1, "{shown}");
    }
    for hidden in [Some("false"), Some("0"), Some(""), None] {
        let out = with(hidden);
        assert!(runs(&out.display_list).is_empty(), "{hidden:?}");
        assert!(out.display_list.items.is_empty(), "{hidden:?}");
    }
    // A hidden cell's own text overflow is not reported either.
    let form = cell_form(
        4_000_000,
        8_000_000,
        "text.normal",
        "middle_left",
        json!("error"),
        json!({"visible_if": "!{page.hide}"}),
    );
    let mut b = bindings("A text that is too long for this cell");
    b.text("page.hide", "true").unwrap();
    let out = lower_form(&form, None, &b, bundled());
    assert!(out.display_list.items.is_empty());
    assert!(overflow_kinds(&out.findings).is_empty());
}

// -- rotated text primitive -------------------------------------------------------------------------------

#[test]
fn rotated_text_primitive_is_placed_about_its_anchor() {
    let style = builtin().resolve("text.normal").unwrap();
    let cap = bundled().vertical_metrics(&style).cap_height;
    let d = cap.0.div_euclid(2);
    let pos = [10_000_000, 20_000_000];
    let lower = |angle: i32| {
        let graphics = json!([{"text": {"pos": pos, "text": "F", "align": "middle_left", "angle": angle}}]);
        let form = form_with(100_000_000, 100_000_000, json!([]), graphics);
        lower_form(&form, None, &PageBindings::new(), bundled())
    };
    let anchor = Pt2::new(Nm(pos[0]), Nm(pos[1]));
    // Unrotated: the baseline sits half a cap height below the anchor (middle alignment).
    let out = lower(0);
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].text, "F");
    assert_eq!(r[0].transform, Transform::IDENTITY);
    assert_eq!(r[0].origin, Pt2::new(anchor.x, Nm(anchor.y.0 + d)));
    // 90 degrees counter-clockwise: the offset (0, d) about the anchor turns to (d, 0); the run carries the
    // rotation and its glyph offsets stay in run space.
    let out = lower(90_000);
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].transform, Transform::from_angle(Angle2(90_000)));
    assert_eq!(r[0].origin, Pt2::new(Nm(anchor.x.0 + d), anchor.y));
    assert_eq!(r[0].glyphs.len(), 1);
    assert_eq!((r[0].glyphs[0].x, r[0].glyphs[0].y), (Nm::ZERO, Nm::ZERO));
    // 180 degrees: the offset turns to (0, -d).
    let out = lower(180_000);
    let r = runs(&out.display_list);
    assert_eq!(r[0].origin, Pt2::new(anchor.x, Nm(anchor.y.0 - d)));
}

// -- logo -------------------------------------------------------------------------------------------------

/// A cell over `width × height` bound to the logo placeholder.
fn logo_form(width: i64, height: i64) -> FormDef {
    let cell = json!({
        "rect": [[0, 0], [width, height]],
        "value": "{project.logo}",
        "style": "text.normal",
        "align": "centre",
        "overflow": "error",
    });
    form_with(width, height, json!([cell]), json!([]))
}

/// The image items of a display list: (format, hash, rect).
fn images(list: &DisplayList) -> Vec<(ImageFormat, [u8; 32], Rect)> {
    list.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Image(img) => Some((img.image.format, img.image.sha256, img.rect)),
            _ => None,
        })
        .collect()
}

fn logo(asset: AssetRef) -> PageBindings {
    let mut b = PageBindings::new();
    b.set("project.logo", BoundValue::Asset(asset)).unwrap();
    b
}

#[test]
fn logo_carries_its_format() {
    let form = logo_form(20_000_000, 20_000_000);
    for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Svg] {
        let out = lower_form(&form, None, &logo(AssetRef::new([9; 32], format)), bundled());
        let imgs = images(&out.display_list);
        assert_eq!(imgs.len(), 1);
        assert_eq!((imgs[0].0, imgs[0].1), (format, [9; 32]));
        // Unknown size: the image fills the inset.
        let pad = Pt2::new(CELL_PADDING, CELL_PADDING);
        let inset = Rect::from_points(pad, Pt2::new(Nm(20_000_000), Nm(20_000_000)) - pad);
        assert_eq!(imgs[0].2, inset);
        assert!(out.findings.is_empty());
    }
}

#[test]
fn logo_fit_preserves_the_aspect_ratio_and_centres() {
    let pad = CELL_PADDING.0;
    let form = logo_form(20_000_000, 20_000_000);
    let side = 20_000_000 - 2 * pad;
    // 2:1 image in a square cell: full width, half the height, centred vertically.
    let asset = AssetRef::new([1; 32], ImageFormat::Png).with_size(200, 100);
    let out = lower_form(&form, None, &logo(asset), bundled());
    let imgs = images(&out.display_list);
    assert_eq!(imgs.len(), 1);
    let r = imgs[0].2;
    assert_eq!((r.width(), r.height()), (Nm(side), Nm(side / 2)));
    assert_eq!(r.min(), Pt2::new(Nm(pad), Nm(pad + (side - side / 2) / 2)));
    // 1:2 image: full height, half the width, centred horizontally.
    let asset = AssetRef::new([1; 32], ImageFormat::Png).with_size(50, 100);
    let out = lower_form(&form, None, &logo(asset), bundled());
    let r = images(&out.display_list)[0].2;
    assert_eq!((r.width(), r.height()), (Nm(side / 2), Nm(side)));
    assert_eq!(r.min(), Pt2::new(Nm(pad + (side - side / 2) / 2), Nm(pad)));
    // The same ratio in other units gives the same placement; a wide box limits an image by its height.
    let wide = logo_form(60_000_000, 20_000_000);
    let asset = AssetRef::new([1; 32], ImageFormat::Svg).with_size(2, 1);
    let r = images(&lower_form(&wide, None, &logo(asset), bundled()).display_list)[0].2;
    let h = 20_000_000 - 2 * pad;
    assert_eq!((r.width(), r.height()), (Nm(h * 2), Nm(h)));
    assert_eq!(r.min().x, Nm(pad + (60_000_000 - 2 * pad - h * 2) / 2));
    assert_eq!(r.min().y, Nm(pad));
}

#[test]
fn logo_fit_rounds_ties_toward_zero() {
    // An inset of 19 nm: a 2:1 image is 9.5 nm high, a tie, which rounds to 9; the leftover 10 nm splits 5/5.
    let pad = CELL_PADDING.0;
    let form = logo_form(2 * pad + 19, 2 * pad + 19);
    let asset = AssetRef::new([1; 32], ImageFormat::Png).with_size(2, 1);
    let out = lower_form(&form, None, &logo(asset), bundled());
    let r = images(&out.display_list)[0].2;
    assert_eq!((r.width(), r.height()), (Nm(19), Nm(9)));
    assert_eq!(r.min(), Pt2::new(Nm(pad), Nm(pad + 5)));
    // 3:2 into 19 × 19: height 12.67 rounds to 13.
    let asset = AssetRef::new([1; 32], ImageFormat::Png).with_size(3, 2);
    let r = images(&lower_form(&form, None, &logo(asset), bundled()).display_list)[0].2;
    assert_eq!((r.width(), r.height()), (Nm(19), Nm(13)));
}

#[test]
fn logo_in_a_cell_smaller_than_its_insets_is_reported() {
    let form = logo_form(1_000_000, 1_000_000);
    let asset = AssetRef::new([1; 32], ImageFormat::Png).with_size(2, 1);
    let out = lower_form(&form, None, &logo(asset), bundled());
    assert!(images(&out.display_list).is_empty());
    assert_eq!(overflow_kinds(&out.findings), vec![OverflowKind::Error]);
}

// -- unresolved placeholders ------------------------------------------------------------------------------

fn unresolved(findings: &[FormFinding]) -> Vec<&str> {
    findings
        .iter()
        .filter_map(|f| match f {
            FormFinding::Binding {
                finding: BindingFinding::Unresolved { placeholder },
                ..
            } => Some(placeholder.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn unresolved_placeholder_draws_nothing_and_is_reported() {
    let cell = |value: &str| {
        json!({
            "rect": [[0, 0], [40_000_000, 8_000_000]],
            "value": value,
            "style": "text.normal",
            "align": "middle_left",
            "overflow": "error",
        })
    };
    let form = form_with(
        40_000_000,
        8_000_000,
        json!([cell("{revision.current.checker.name}")]),
        json!([]),
    );
    let out = lower_form(&form, None, &PageBindings::new(), bundled());
    assert!(runs(&out.display_list).is_empty());
    assert_eq!(unresolved(&out.findings), vec!["{revision.current.checker.name}"]);
    // A mixed template keeps its resolved parts and literals; the unresolved part is empty.
    let form = form_with(
        40_000_000,
        8_000_000,
        json!([cell("A {page.title} B {page.nope | upper} C")]),
        json!([]),
    );
    let out = lower_form(&form, None, &bindings("T"), bundled());
    let r = runs(&out.display_list);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].text, "A T B  C");
    assert_eq!(unresolved(&out.findings), vec!["{page.nope | upper}"]);
}

/// Lowers one cell with the template `value` and the title binding `T`; the texts drawn and the binding
/// findings.
fn lower_template(value: &str) -> (Vec<String>, Vec<BindingFinding>) {
    let cell = json!({
        "rect": [[0, 0], [40_000_000, 8_000_000]],
        "value": value,
        "style": "text.normal",
        "align": "middle_left",
        "overflow": "error",
    });
    let form = form_with(40_000_000, 8_000_000, json!([cell]), json!([]));
    let out = lower_form(&form, None, &bindings("T"), bundled());
    let texts = runs(&out.display_list).iter().map(|r| r.text.clone()).collect();
    let findings = out
        .findings
        .into_iter()
        .filter_map(|f| match f {
            FormFinding::Binding { finding, .. } => Some(finding),
            _ => None,
        })
        .collect();
    (texts, findings)
}

#[test]
fn unknown_scope_placeholder_renders_empty_with_its_finding() {
    let (texts, findings) = lower_template("{foo.bar}");
    assert!(texts.is_empty());
    assert_eq!(
        findings,
        vec![BindingFinding::UnknownScope {
            placeholder: "{foo.bar}".to_owned()
        }]
    );
    let (texts, _) = lower_template("A {foo.bar} {page.title}");
    assert_eq!(texts, vec!["A  T".to_owned()]);
}

#[test]
fn computed_placeholder_renders_empty_with_its_finding() {
    let (texts, findings) = lower_template("{= 1 + 1}");
    assert!(texts.is_empty());
    assert_eq!(
        findings,
        vec![BindingFinding::Computed {
            placeholder: "{= 1 + 1}".to_owned()
        }]
    );
}

#[test]
fn malformed_template_draws_nothing_with_its_finding() {
    for t in ["{page.title", "Title {page.title} {oops", "a } b"] {
        let (texts, findings) = lower_template(t);
        assert!(texts.is_empty(), "{t}: {texts:?}");
        assert_eq!(
            findings,
            vec![BindingFinding::Malformed {
                template: t.to_owned()
            }],
            "{t}"
        );
    }
}
