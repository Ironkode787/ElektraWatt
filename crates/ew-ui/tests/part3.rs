// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 3 in the window without rendering: the IP tool's name field in the tool strip keeps the canvas keys
//! (P, D, B) while it has the focus (the `text_edit_focused` gate of the key routing in `app.rs`), and Enter sends the
//! name to the tool.

mod support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
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

fn press(h: &mut Harness<'_, App>, key: Key) {
    h.key_press(key);
    settle(h);
}

fn ghost(h: &Harness<'_, App>) -> Option<ew_ui::ew_viewmodel::tools::Ghost> {
    h.state().shell().tool_ghost()
}

fn direction(h: &Harness<'_, App>) -> &'static str {
    match ghost(h) {
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { direction, .. }) => direction,
        other => panic!("not the IP tool's ghost: {other:?}"),
    }
}

#[test]
fn p_d_and_b_stay_with_the_ip_name_field_while_it_has_the_focus() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    // P on the canvas arms the IP tool; D turns its direction.
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    let first = direction(&h);
    press(&mut h, Key::D);
    let second = direction(&h);
    assert_ne!(first, second, "D reaches the tool from the canvas");
    // The name field in the tool strip takes the focus and the text.
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field");
    field.click();
    settle(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field");
    field.type_text("X1");
    settle(&mut h);
    // While it has the focus, P, D and B are the field's: the tool, its direction and the active tool stay.
    let rev = h.state().shell().revision();
    for k in [Key::P, Key::D, Key::B] {
        press(&mut h, k);
    }
    assert_eq!(h.state().shell().tool().name(), "ip", "B arms no black box tool");
    assert_eq!(direction(&h), second, "D does not reach the tool");
    assert_eq!(h.state().shell().revision(), rev);
    // Enter sends the name to the tool.
    press(&mut h, Key::Enter);
    match ghost(&h) {
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { name, .. }) => assert_eq!(name, "X1"),
        other => panic!("{other:?}"),
    }
}
