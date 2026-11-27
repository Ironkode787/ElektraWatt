// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The HUD of a panel page counts the cables' glands that wait (doc 12 §3.9): on the bottom's page with its button
//! Place cable glands, on the plate's with Bottom; a page of a door or a wall never says the plate's first steps (the
//! location sentence the bottom's page said with its glands placed).

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::command_bar::hud;
use ew_viewmodel::shell::Mode;
use serde_json::json;

const MM: i128 = 1_000_000;

#[test]
fn the_hud_counts_the_glands_that_wait_and_offers_their_page_or_their_placing() {
    let tmp = Tmp::new("glands-hud");
    let mut s = tmp.sample_in(Mode::Gui, true, "905");
    let h = hud(&s);
    assert!(
        !h.prompt.contains("location"),
        "the bottom's page with its glands placed: {}",
        h.prompt
    );
    assert!(h.buttons.is_empty(), "{:?}", h.buttons);

    // The two glands taken off the bottom wait again: a band around the row at 80 and 115 mm, 125 mm up (the bottom's
    // datum is its frame's corner), then Delete.
    let t = s.panel_tables().expect("905's tables");
    let sheet = |u: i128, v: i128| s.canvas().unwrap().page().snapper.page_to_sheet(t.to_page((u * MM, v * MM)));
    let (from, to) = (sheet(50, 155), sheet(145, 95));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    done(&mut s, "canvas.release", at(to, json!({})));
    assert_eq!(s.selection().items.len(), 2, "the two glands");
    done(&mut s, "edit.delete_selection", json!({}));
    let h = hud(&s);
    assert_eq!(h.prompt, "2 glands to place");
    let place: Vec<_> = h.buttons.iter().map(|b| (b.label.as_str(), b.action)).collect();
    assert_eq!(place, [("Place cable glands", "layout.glands_place")]);
    done(&mut s, "page.goto", json!({"folio": "902"}));
    let h = hud(&s);
    assert_eq!(h.prompt, "2 glands to place");
    assert_eq!(h.buttons[0].label, "Bottom");
    assert_eq!(h.buttons[0].action, "layout.surface_page");

    // The button places them in a row on the bottom.
    done(&mut s, "page.goto", json!({"folio": "905"}));
    done(&mut s, "layout.glands_place", json!({}));
    let h = hud(&s);
    assert!(!h.prompt.contains("glands to place"), "{}", h.prompt);
    s.shutdown();
}
