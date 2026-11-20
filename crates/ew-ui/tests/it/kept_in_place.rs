// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Doc 06a §2.4 in the window, on a writable copy of Golden A, folio 101: a symbol kept in place shows a small pin on
//! its body's top-right corner while it is selected, and none when the selection leaves it.

use crate::support;

use serde_json::json;
use support::{Tmp, dispatch, on_golden, settle};

#[test]
fn a_selected_kept_symbol_shows_the_pin() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let rep = {
        let shell = h.state().shell();
        let page = shell.canvas().unwrap().page();
        page.tables
            .reps
            .iter()
            .find(|r| r.device.is_some() && !r.terminal && r.body[2] > r.body[0])
            .expect("a device's symbol")
            .clone()
    };
    dispatch(&mut h, "select.set", json!({ "ids": [rep.id.to_string()] }));
    settle(&mut h);
    assert!(
        h.state_mut().canvas_host().lock_marks(0).is_empty(),
        "not kept: no pin"
    );
    dispatch(&mut h, "rep.keep_in_place", json!({}));
    settle(&mut h);
    // The body's top-right corner on screen, as the last frame drew the page.
    let corner = {
        let page = h.state().shell().canvas().unwrap().page().clone();
        let [x0, y0, x1, y1] = rep.body;
        let host = h.state_mut().canvas_host();
        let pts: Vec<egui::Pos2> = [(x0, y0), (x1, y1)]
            .iter()
            .map(|p| {
                host.drawn_screen_of(0, page.snapper.page_to_sheet(*p))
                    .expect("a drawn canvas")
            })
            .collect();
        egui::pos2(pts[0].x.max(pts[1].x), pts[0].y.min(pts[1].y))
    };
    let size = ew_ui::theme::current(&h.ctx).icon_sm;
    let marks = h.state_mut().canvas_host().lock_marks(0).to_vec();
    let [m] = marks[..] else {
        panic!("one kept symbol selected, one pin: {marks:?}")
    };
    assert_eq!((m.width(), m.height()), (size, size));
    assert!(
        (m.right() - corner.x).abs() < 0.5 && (m.bottom() - corner.y).abs() < 0.5,
        "{m:?} at {corner:?}"
    );
    // Still kept, but no longer selected: no pin.
    dispatch(&mut h, "select.set", json!({ "ids": [] }));
    settle(&mut h);
    assert!(h.state_mut().canvas_host().lock_marks(0).is_empty());
}
