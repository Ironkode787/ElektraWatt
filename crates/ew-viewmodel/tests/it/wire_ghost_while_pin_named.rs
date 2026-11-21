// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire drawn to a drawn box's edge where no pin is asks for the pin's name there; the wire's ghost stays on the page
//! while the name is asked and typed, and the wire then ends at the new pin.

use crate::common;

use common::{Tmp, actual_size, cursor, done, grid_point, select_ids, sheet, tables};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::tools::OverlayItem;
use serde_json::json;

/// The grid cursor to grid index `k`.
fn cursor_k(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid_point(s, k);
    cursor(s, p);
}

/// The grid cursor to grid index `k`, then Enter.
fn enter_k(s: &mut ShellModel, k: (i64, i64)) {
    cursor_k(s, k);
    done(s, "canvas.activate", json!({}));
}

/// Whether the tool overlay draws a line through points (a wire's ghost; a pin's marker and name draw none).
fn has_ghost(s: &ShellModel) -> bool {
    s.tool_overlay()
        .items
        .iter()
        .any(|i| matches!(i, OverlayItem::Polyline { .. }))
}

#[test]
fn the_wire_s_ghost_stays_while_its_pin_is_named() {
    let tmp = Tmp::new("wire-ghost-pin");
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
    // A box drawn without a part from grid (30, 30) to (42, 38), with a pin L1 on its top edge.
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
    select_ids(&mut s, &[rep.to_string()]);
    let near_top = sheet(&s, grid_point(&s, (33, 31)));
    done(
        &mut s,
        "box.add_cp",
        json!({"x_mm": near_top.0 as f64 / 1e6, "y_mm": near_top.1 as f64 / 1e6}),
    );
    done(&mut s, "text.input", json!({"text": "L1"}));
    done(&mut s, "tool.finish", json!({}));
    // A wire from L1 round the box to its bottom edge, where no pin is.
    let l1 = tables(&s)
        .reps
        .iter()
        .find(|r| r.id == rep)
        .and_then(|r| r.black_box.clone())
        .expect("the box")
        .cps[0]
        .1;
    done(&mut s, "tool.wire", json!({}));
    cursor(&mut s, l1);
    done(&mut s, "canvas.activate", json!({}));
    for k in [(33, 26), (28, 26), (28, 42), (36, 42)] {
        enter_k(&mut s, k);
    }
    cursor_k(&mut s, (36, 38));
    assert!(has_ghost(&s), "the wire's ghost to the edge");
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.tool().name(), "box_pin", "the wire asks for a pin there");
    assert!(has_ghost(&s), "the ghost stays while the name is asked");
    done(&mut s, "text.input", json!({"text": "PE"}));
    assert!(has_ghost(&s), "and while it is typed");
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().name(), "wire", "back to the wire");
    let t = tables(&s);
    let pe = t
        .cps
        .iter()
        .find(|c| c.rep == rep && c.label.ends_with(":PE"))
        .expect("the new pin");
    assert_eq!(pe.connections.len(), 1, "the wire ends at the new pin");
}
