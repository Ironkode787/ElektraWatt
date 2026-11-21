// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A code written as a free text on a wire stands beside the wire, not on it (FT-1870; doc 06 §4.5): the Text tool
//! clicked on a horizontal stretch of the sample's page 102 sets the text on its baseline a millimetre above the line,
//! on a vertical stretch a millimetre right of it.

use crate::common;

use common::{Tmp, actual_size, at, canvas, done, sheet, tables};
use serde_json::{Value, json};

/// The middle of the first straight stretch of the page's wires that runs `horizontal` and is at least 8 mm long.
fn stretch(s: &ew_viewmodel::shell::ShellModel, horizontal: bool) -> (i64, i64) {
    tables(s)
        .connections
        .iter()
        .flat_map(|c| c.polyline.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>())
        .find_map(|((x0, y0), (x1, y1))| {
            let long = (x1 - x0).abs() + (y1 - y0).abs() >= 8_000_000;
            let ok = if horizontal { y0 == y1 } else { x0 == x1 };
            (ok && long).then_some(((x0 + x1) / 2, (y0 + y1) / 2))
        })
        .expect("a stretch")
}

/// Writes `text` with the Text tool clicked at the page point `p`; the commit's `free_text`.
fn write_at(s: &mut ew_viewmodel::shell::ShellModel, p: (i64, i64), text: &str) -> ((i64, i64), Value) {
    let point = sheet(s, p);
    done(s, "canvas.press", at(point, json!({})));
    let snapped = canvas(s)["ghost"]["at_nm"].clone();
    let snapped = (snapped[0].as_i64().unwrap(), snapped[1].as_i64().unwrap());
    done(s, "canvas.release", at(point, json!({})));
    done(s, "text.input", json!({"text": text}));
    done(s, "tool.finish", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "item.place");
    (snapped, last.params["kind"]["free_text"].clone())
}

#[test]
fn a_text_clicked_on_a_wire_stands_beside_it() {
    let tmp = Tmp::new("text-beside-wire");
    let mut s = actual_size(&tmp, "102");
    done(&mut s, "tool.text", json!({}));
    let (x, y) = stretch(&s, true);
    let (snapped, free) = write_at(&mut s, (x, y), "6A03");
    assert_eq!(
        free["at"],
        json!([snapped.0, y - 1_000_000]),
        "above the line: {free}"
    );
    assert_eq!(free["anchor"], "baseline", "on its baseline: {free}");
    let (x, y) = stretch(&s, false);
    let (snapped, free) = write_at(&mut s, (x, y), "6N");
    assert_eq!(
        free["at"],
        json!([x + 1_000_000, snapped.1]),
        "right of the line: {free}"
    );
    assert_eq!(free.get("anchor"), None);
}
