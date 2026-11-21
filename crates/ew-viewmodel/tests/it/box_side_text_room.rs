// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawn box goes no narrower than the pin texts of its left and right sides need, as it goes no lower than those
//! of its top and bottom sides: one rule for the four sides (doc 06 §8 item 13).

use crate::common;

use common::{Tmp, actual_size, at, cursor, done, grid_point, select_ids, sheet, tables};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The grid cursor to grid index `k`, then Enter.
fn enter_k(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid_point(s, k);
    cursor(s, p);
    done(s, "canvas.activate", json!({}));
}

/// The grid index of a page point.
fn index_of(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.grid_index_of(p.0), sn.grid_index_of(p.1))
}

/// The box `rep` of the page shown.
fn the_box(s: &ShellModel, rep: ew_api::output::Uuid) -> ew_api::output::TableBox {
    tables(s)
        .reps
        .iter()
        .find(|r| r.id == rep)
        .and_then(|r| r.black_box.clone())
        .expect("the box")
}

/// Add pin at grid index `k`, named `name`.
fn add_pin(s: &mut ShellModel, rep: ew_api::output::Uuid, k: (i64, i64), name: &str) {
    select_ids(s, &[rep.to_string()]);
    let p = sheet(s, grid_point(s, k));
    done(
        s,
        "box.add_cp",
        json!({"x_mm": p.0 as f64 / 1e6, "y_mm": p.1 as f64 / 1e6}),
    );
    done(s, "text.input", json!({ "text": name }));
    done(s, "tool.finish", json!({}));
}

#[test]
fn a_box_goes_no_narrower_than_the_texts_of_its_left_and_right_pins_need() {
    let tmp = Tmp::new("box-side-room");
    let mut s = actual_size(&tmp, "102");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "page.create", "params": {
            "number": 150, "title": "Box", "kind": "multi_line", "grid": {"num": 7_937_500, "den": 3},
        }}),
    );
    done(&mut s, "page.goto", json!({"folio": "150"}));
    done(&mut s, "view.zoom_actual", json!({}));
    // A box drawn without a part from grid (30, 30) to (42, 38), a pin on its left side and one on its right.
    done(
        &mut s,
        "tool.black_box",
        json!({"symbol": "ew.gen.blackbox@qet10", "letter": "A"}),
    );
    enter_k(&mut s, (30, 30));
    enter_k(&mut s, (42, 38));
    done(&mut s, "tool.select", json!({}));
    let rep = tables(&s)
        .reps
        .iter()
        .find(|r| r.black_box.is_some())
        .expect("the box")
        .id;
    add_pin(&mut s, rep, (31, 34), "IN24V");
    add_pin(&mut s, rep, (41, 34), "OUT24V");
    let b = the_box(&s, rep);
    let sides: Vec<i64> = b.cps.iter().map(|c| index_of(&s, c.1).0).collect();
    assert_eq!(sides, [30, 42], "one pin on each side");
    assert!(
        b.room.0 > 1,
        "the two texts need more than a step across: {:?}",
        b.room
    );
    assert_eq!(b.room.1, 0, "no pin on the top or the bottom");
    // The bottom-right corner dragged left to one step right of the left side.
    select_ids(&mut s, &[rep.to_string()]);
    let corner = *b.outline.iter().max_by_key(|p| (p.0, p.1)).expect("a corner");
    let k = index_of(&s, corner);
    let (from, to) = (sheet(&s, corner), sheet(&s, grid_point(&s, (31, k.1))));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    assert!(
        s.tool_hint().starts_with("The box goes no smaller"),
        "{}",
        s.tool_hint()
    );
    done(&mut s, "canvas.release", at(to, json!({})));
    let xs: Vec<i64> = the_box(&s, rep)
        .outline
        .iter()
        .map(|p| index_of(&s, *p).0)
        .collect();
    let width = xs.iter().max().unwrap() - xs.iter().min().unwrap();
    assert!(width >= b.room.0, "{width} {:?}", b.room);
    assert!(
        width < 12,
        "the box got narrower as far as the texts let it: {width}"
    );
}
