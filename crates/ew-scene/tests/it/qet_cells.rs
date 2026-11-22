// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A QElectroTech title block's cell as QElectroTech prints it (FT-1941, doc 14 §4.10): `fit_width` scales a text
//! too wide for its cell whole, without a floor and never for its height; `ew.cell.pixel` lays its text out with every
//! advance a whole pixel of QElectroTech's screen.

use ew_dl::{ItemKind, ShapedRun};
use ew_scene::forms::{FormFinding, PageBindings, lower_form};
use ew_schema::FormDef;
use ew_text::{builtin, bundled};
use serde_json::{Value, json};

/// One QET px, nm.
const PX: i64 = 264_583;

/// The runs a `width` × `height` cell aligned `align` under `overflow` prints for `text`, with the cell's `extra`
/// members, and the findings.
fn cell(
    width: i64,
    height: i64,
    align: &str,
    overflow: Value,
    extra: Value,
    text: &str,
) -> (Vec<ShapedRun>, Vec<FormFinding>) {
    let mut cell = json!({
        "rect": [[0, 0], [width, height]],
        "value": "{page.title}",
        "style": "text.normal",
        "align": align,
        "overflow": overflow,
        "ew.cell.inset": 0,
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        cell[k] = v.clone();
    }
    let form: FormDef = serde_json::from_value(json!({
        "kind": "title_block",
        "sheet": {"width": width, "height": height},
        "cells": [cell],
        "graphics": [],
    }))
    .unwrap();
    let mut b = PageBindings::new();
    b.text("page.title", text).unwrap();
    let out = lower_form(&form, None, &b, bundled(), builtin());
    let runs = out
        .display_list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some((**r).clone()),
            _ => None,
        })
        .collect();
    (runs, out.findings)
}

#[test]
fn fit_width_scales_a_text_too_wide_whole_to_the_width_without_a_floor() {
    let em = builtin().resolve("text.normal").unwrap();
    let text = "A text far wider than its small cell, which QElectroTech scales down to the width";
    let wide = bundled().shape(text, &em, None).advance().0;
    // A cell a tenth as wide: the text at a tenth of its size, under any floor a shrinking cell keeps.
    let (w, h) = (wide / 10, 4_000_000);
    let (runs, findings) = cell(w, h, "middle_left", json!("fit_width"), json!({}), text);
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(runs.len(), 1);
    let r = &runs[0];
    assert!((r.layout_box.max().x.0 - w).abs() <= 1_000, "{:?}", r.layout_box);
    assert!((r.size.0 - em.size.0 / 10).abs() <= 10, "{:?}", r.size);
    // Placed in the cell's height scaled by the same share, about its middle: top-aligned, the line's top stands
    // (1 − share) / 2 of the height below the cell's top.
    let (top, _) = cell(w, h, "top_left", json!("fit_width"), json!({}), text);
    let scaled = em.with_size(top[0].size);
    let kept = i128::from(h) * i128::from(top[0].size.0) / i128::from(em.size.0);
    let ascent = bundled().vertical_metrics(&scaled).ascent.0;
    assert_eq!(top[0].origin.y.0, ((i128::from(h) - kept) / 2) as i64 + ascent);
}

#[test]
fn fit_width_never_shrinks_a_text_for_its_height_and_never_wraps() {
    let em = builtin().resolve("text.normal").unwrap();
    // Four lines in a cell one line high, each line narrower than the cell: drawn at their size, not reported.
    let text = "One\nTwo\nThree\nFour";
    let (runs, findings) = cell(
        40_000_000,
        3_000_000,
        "centre",
        json!("fit_width"),
        json!({"ew.cell.max_lines": 4}),
        text,
    );
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(runs.len(), 4);
    assert!(runs.iter().all(|r| r.size == em.size));
    // A line wider than the cell is not broken: the whole text is scaled by the widest line.
    let long = "A first line far too long for this cell\nShort";
    let (runs, _) = cell(
        20_000_000,
        8_000_000,
        "centre",
        json!("fit_width"),
        json!({"ew.cell.max_lines": 2}),
        long,
    );
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r.size == runs[0].size && r.size < em.size));
}

#[test]
fn a_cell_with_a_pixel_lays_its_text_out_in_whole_pixels() {
    let em = builtin().resolve("text.normal").unwrap();
    let text = "Creation: 05-08-2026";
    let pixel = json!({"ew.cell.pixel": PX});
    let (runs, findings) = cell(
        60_000_000,
        5_000_000,
        "middle_right",
        json!("error"),
        pixel.clone(),
        text,
    );
    assert!(findings.is_empty(), "{findings:?}");
    let r = &runs[0];
    // Every pen position a whole number of pixels from the line's start, and the line too.
    let start = r.glyphs[0].x.0;
    assert!(
        r.glyphs.iter().all(|g| (g.x.0 - start) % PX == 0),
        "{:?}",
        r.glyphs
    );
    assert_eq!((r.layout_box.max().x.0 - start) % PX, 0);
    // Each advance the face's own, rounded to a whole pixel.
    let exact = bundled().shape(text, &em, None);
    let ends: Vec<i64> = exact
        .run
        .glyphs
        .iter()
        .skip(1)
        .map(|g| g.x.0)
        .chain([exact.advance().0])
        .collect();
    let rounded: i64 = exact
        .run
        .glyphs
        .iter()
        .zip(&ends)
        .map(|(g, end)| (2 * (end - g.x.0) + PX).div_euclid(2 * PX) * PX)
        .sum();
    assert_eq!(r.layout_box.max().x.0 - start, rounded);
    assert_ne!(rounded, exact.advance().0, "the text is one whose rounding shows");
    // Right-aligned, it still ends on the cell's edge.
    assert_eq!(r.origin.x.0 + r.layout_box.max().x.0, 60_000_000);
    // A pixel scales with a text that `fit_width` scales.
    let (runs, _) = cell(
        10_000_000,
        5_000_000,
        "middle_left",
        json!("fit_width"),
        pixel,
        text,
    );
    let scaled = i128::from(PX) * i128::from(runs[0].size.0) / i128::from(em.size.0);
    let g = &runs[0].glyphs;
    let steps: Vec<i128> = g.windows(2).map(|w| i128::from(w[1].x.0 - w[0].x.0)).collect();
    assert!(runs[0].size < em.size);
    assert!(steps.iter().all(|s| s % scaled == 0), "{steps:?} by {scaled}");
}

#[test]
fn a_pixel_out_of_range_lays_the_text_out_as_the_face_does() {
    let em = builtin().resolve("text.normal").unwrap();
    let text = "Creation: 05-08-2026";
    let exact = bundled().shape(text, &em, None).advance();
    for value in [json!(0), json!(-PX), json!(20_000_000), json!("px")] {
        let (runs, _) = cell(
            60_000_000,
            5_000_000,
            "middle_left",
            json!("error"),
            json!({"ew.cell.pixel": value}),
            text,
        );
        assert_eq!(runs[0].layout_box.max().x, exact, "{value}");
    }
}
