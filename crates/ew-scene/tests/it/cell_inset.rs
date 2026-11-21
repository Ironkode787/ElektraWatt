// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A form cell's own inset (`ew.cell.inset`, FT-1765): a QElectroTech title block's cell holds its text on its edges,
//! as QET draws it; a cell without the member keeps the house inset, and a value out of range is the house inset.

use ew_dl::ItemKind;
use ew_scene::forms::{CELL_PADDING, PageBindings, lower_form};
use ew_schema::FormDef;
use ew_text::{builtin, bundled};
use serde_json::{Value, json};

/// The start and end of the one line a 40 × 8 mm cell aligned `align` prints, with the cell's `inset` member.
fn line(align: &str, inset: Value) -> (i64, i64) {
    let mut cell = json!({
        "rect": [[0, 0], [40_000_000, 8_000_000]],
        "value": "{page.title}",
        "style": "text.normal",
        "align": align,
        "overflow": "error",
    });
    if !inset.is_null() {
        cell["ew.cell.inset"] = inset;
    }
    let form: FormDef = serde_json::from_value(json!({
        "kind": "title_block",
        "sheet": {"width": 40_000_000, "height": 8_000_000},
        "cells": [cell],
        "graphics": [],
    }))
    .unwrap();
    let mut b = PageBindings::new();
    b.text("page.title", "K1").unwrap();
    let out = lower_form(&form, None, &b, bundled(), builtin());
    let runs: Vec<(i64, i64)> = out
        .display_list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(r) => Some((r.origin.x.0, r.origin.x.0 + r.layout_box.max().x.0)),
            _ => None,
        })
        .collect();
    assert_eq!(runs.len(), 1);
    runs[0]
}

#[test]
fn a_cell_without_an_inset_holds_its_text_on_its_edges() {
    assert_eq!(line("middle_left", json!(0)).0, 0);
    assert_eq!(line("middle_right", json!(0)).1, 40_000_000);
    assert_eq!(line("middle_left", Value::Null).0, CELL_PADDING.0);
    assert_eq!(line("middle_right", Value::Null).1, 40_000_000 - CELL_PADDING.0);
    for out_of_range in [json!(-1), json!(20_000_000), json!("none")] {
        assert_eq!(line("middle_left", out_of_range).0, CELL_PADDING.0);
    }
}
