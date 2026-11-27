// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A connection point drawn in the symbol editor brings the text that writes its number, where a device box prints a
//! pin's name (doc 07 §2.5 and §5): inside the box one millimetre from the point, turned a quarter on the upper and
//! lower edges and read from below, level on the left and right ones. A point on the lower edge so has its number above
//! it inside the box, not across it, and the drawing shows the text the page prints.

use crate::common::{Tmp, done};
use ew_api::symbols::schema::{Align, SlotKind};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_new_point_s_number_stands_inside_the_box_at_the_point_turned_by_its_edge() {
    let tmp = Tmp::new("symbol-point-number");
    let mut shell = tmp.empty_shell(Mode::Headless);
    crate::common::new_drawing(&mut shell, "Supply");
    // A box with pins of its own, named as drawn.
    done(
        &mut shell,
        "parts.field",
        json!({"field": "symbol.what", "value": "box"}),
    );
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool":"rectangle","x":0,"y":0,"x2":16,"y2":12}),
    );
    // (point, designation, direction) → (its number's place in mm, its angle in degrees, its alignment).
    let points = [
        ((4, 12), "+V", "down", ((4.0, 11.0), 90, Align::MiddleLeft)),
        ((4, 0), "L", "up", ((4.0, 1.0), 90, Align::MiddleRight)),
        ((0, 8), "PE", "left", ((1.0, 8.0), 0, Align::MiddleLeft)),
        ((16, 8), "FG", "right", ((15.0, 8.0), 0, Align::MiddleRight)),
    ];
    for ((x, y), name, direction, _) in &points {
        done(
            &mut shell,
            "symbol.cp",
            json!({"x":x,"y":y,"designation":name,"direction":direction}),
        );
    }
    let draft = &shell.symbol_editor().unwrap().draft;
    for (k, (_, name, _, ((x, y), degrees, align))) in points.iter().enumerate() {
        let idx = draft.symbol.cps[k].idx;
        let slots: Vec<_> = draft
            .symbol
            .text_slots
            .iter()
            .filter(|s| s.kind == SlotKind::CpDesignation(idx))
            .collect();
        assert_eq!(slots.len(), 1, "{name}: one text writes its number");
        let s = slots[0];
        let mm = |v: i64| v as f64 / 1e6;
        assert_eq!((mm(s.pos.x.0), mm(s.pos.y.0)), (*x, *y), "{name}: its place");
        assert_eq!(s.rot.0, degrees * 1000, "{name}: its angle");
        assert_eq!(&s.align, align, "{name}: its alignment");
        assert!(s.visible, "{name}: shown");
    }
    assert!(draft.validate().is_empty(), "{:?}", draft.validate());
    shell.shutdown();
}
