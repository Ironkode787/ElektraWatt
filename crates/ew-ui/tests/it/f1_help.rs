// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F1 in the window opens the help for what is under the pointer: a button of the command bar its command's
//! section, a wire on the page the wires', a row of Properties its row's place in the properties reference. While the
//! pointer rests there the status bar says what F1 opens, and the button's tooltip ends with "F1: help".

use crate::support::{self, Tmp, dispatch, on_golden, screen, settle};
use egui::Key;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_viewmodel::{key, strings};
use serde_json::json;

/// The guide page the last F1 asked for.
fn opened(h: &egui_kittest::Harness<'_, ew_ui::app::App>) -> String {
    h.state().shell().last_help().unwrap_or_default().to_owned()
}

#[test]
fn f1_opens_the_page_of_what_the_pointer_rests_on() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    // A button of the command bar: its tool's section; the tooltip and the status bar say so.
    support::bar_button(&h, "Wire").hover();
    settle(&mut h);
    settle(&mut h);
    assert!(h.query_by_label("F1: Wire").is_some(), "the status bar names it");
    assert!(
        h.query_by_label_contains(strings::text(key!("tooltip-learn-more")))
            .is_some(),
        "the tooltip ends with F1: help"
    );
    h.key_press(Key::F1);
    settle(&mut h);
    assert_eq!(opened(&h), "reference/window/tools.html#wire");
    // A wire on the page: the wires' section.
    let (middle, _) = {
        let page = h.state().shell().canvas().unwrap().page();
        let wire = page
            .tables
            .connections
            .iter()
            .find(|c| c.kind == "wire" && c.line_kind.is_none() && c.polyline.len() >= 2)
            .expect("a wire on the page");
        let (a, b) = (wire.polyline[0], wire.polyline[1]);
        let mid = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
        (page.snapper.page_to_sheet(mid), wire.id)
    };
    let at = screen(&h, middle);
    h.hover_at(at);
    settle(&mut h);
    settle(&mut h);
    assert!(
        h.query_by_label("F1: Wires").is_some(),
        "the status bar names the wire's section"
    );
    h.key_press(Key::F1);
    settle(&mut h);
    assert_eq!(opened(&h), "reference/window/things.html#wires");
    // A row of Properties: its anchor in the properties reference.
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-102G1"]}));
    dispatch(&mut h, "props.show", json!({"on": true}));
    // The cursor row scrolls into view; the cursor goes elsewhere again, so the pointer alone names the row.
    dispatch(&mut h, "props.row", json!({"to": "device/process_tag"}));
    settle(&mut h);
    settle(&mut h);
    dispatch(&mut h, "ui.focus", json!({"area": "canvas"}));
    settle(&mut h);
    let label = strings::field_label("device", "process_tag");
    h.get_all_by_label(&label)
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Group)
        .expect("the Process tag row")
        .hover();
    settle(&mut h);
    settle(&mut h);
    h.key_press(Key::F1);
    settle(&mut h);
    assert_eq!(opened(&h), "reference/properties.html#device-process-tag");
    h.state_mut().shell_mut().shutdown();
}
