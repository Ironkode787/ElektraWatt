// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 4b in the window without rendering (PI7, §10.5): the text popup takes the focus when the Text tool
//! opens it, Enter sends the text to the tool and places it, and Escape closes it without a commit.

mod support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::ew_viewmodel::strings;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": folio }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// The Text tool typing at a free point: the popup opens and its field takes the focus.
fn typing_text(h: &mut Harness<'_, App>) {
    dispatch(h, "tool.text", json!({}));
    dispatch(h, "cursor.set", json!({ "x_mm": 100.0, "y_mm": 100.0 }));
    dispatch(h, "canvas.activate", json!({}));
    settle(h);
    settle(h);
    assert_eq!(h.state().shell().tool().name(), "text");
    assert_eq!(h.state().shell().tool().state(), "typing");
    assert!(h.state().shell().text_popup().is_some());
}

/// The popup's text field by its accessible name.
fn label() -> &'static str {
    strings::text("action-text-input")
}

#[test]
fn the_text_popup_takes_the_text_and_enter_places_it() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    typing_text(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label()))
        .expect("the text popup's field");
    assert!(field.is_focused(), "the popup's field takes the focus");
    field.type_text("40x60");
    settle(&mut h);
    let rev = h.state().shell().revision();
    h.key_press(Key::Enter);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(
        h.state().shell().revision(),
        rev.map(|r| r + 1),
        "Enter commits one item.place"
    );
    let tables = h.state().shell().canvas().unwrap().page().tables.clone();
    let item = tables
        .items
        .iter()
        .find(|i| i.kind == "free_text" && i.text.as_deref() == Some("40x60"))
        .expect("the free text");
    assert_eq!(item.bind, None);
    assert!(h.state().shell().text_popup().is_none(), "the popup is gone");
    assert_eq!(h.state().shell().tool().name(), "text");
    assert_eq!(h.state().shell().tool().state(), "armed");
}

#[test]
fn escape_closes_the_text_popup_without_a_commit() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    typing_text(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label()))
        .expect("the text popup's field");
    field.type_text("40x60");
    settle(&mut h);
    let rev = h.state().shell().revision();
    h.key_press(Key::Escape);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().revision(), rev, "nothing committed");
    assert_eq!(h.state().shell().tool().name(), "text");
    assert_eq!(h.state().shell().tool().state(), "armed");
    assert!(h.state().shell().text_popup().is_none(), "the popup is closed");
    let tables = h.state().shell().canvas().unwrap().page().tables.clone();
    assert!(
        !tables
            .items
            .iter()
            .any(|i| i.kind == "free_text" && i.text.as_deref() == Some("40x60")),
        "the typed text is not placed"
    );
}
