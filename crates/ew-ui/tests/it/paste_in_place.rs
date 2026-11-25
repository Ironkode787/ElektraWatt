// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Edit ▸ Paste in place in the window: like Edit ▸ Paste it asks the platform for the system clipboard, and
//! the platform's answer puts the block down where it was copied, in one commit.

use crate::support;

use egui_kittest::kittest::Queryable;
use serde_json::json;
use support::{Tmp, dispatch, on_sample, settle};

/// -102F1 copied on 102 and pasted in place from the Edit menu on a new page of the same frame stands where it
/// stands on 102.
#[test]
fn menu_paste_in_place_asks_for_the_clipboard_and_puts_the_block_where_it_was() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "102");
    let (rep, pos) = {
        let tables = &h.state().shell().canvas().unwrap().page().tables;
        let f1 = tables
            .cps
            .iter()
            .find(|c| c.label == "-102F1:1")
            .expect("-102F1:1")
            .rep;
        let rep = tables.reps.iter().find(|r| r.id == f1).unwrap();
        (rep.id.to_string(), rep.pos)
    };
    dispatch(&mut h, "select.set", json!({ "ids": [rep] }));
    h.event(egui::Event::Copy);
    settle(&mut h);
    let text = h.state().shell().clipboard().text().expect("copied").to_owned();
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "106", "type": "multi_line", "title": "Copy"}),
    );
    dispatch(&mut h, "page.goto", json!({"folio": "106"}));
    settle(&mut h);
    support::open_menu(&mut h, &["Edit"]);
    settle(&mut h);
    h.get_by_label_contains("Paste in place").click();
    h.step();
    assert!(
        h.output().viewport_output.values().any(|v| v
            .commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::RequestPaste))),
        "the frame asks the platform for the clipboard"
    );
    let rev = h.state().shell().revision();
    // The platform's answer.
    h.event(egui::Event::Paste(text));
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "one commit");
    let pasted: Vec<(i64, i64)> = shell
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.pos)
        .collect();
    assert_eq!(pasted, [pos], "where -102F1 stands on 102");
}
