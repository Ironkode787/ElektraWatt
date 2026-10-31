// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 3 in the window without rendering: the IP tool's name field in the tool strip keeps the canvas keys
//! (P, D, B) while it has the focus (the `text_edit_focused` gate of the key routing in `app.rs`), and Enter sends the
//! name to the tool.

use crate::support;

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

/// The IP points of the page on show: (representation, name).
fn ips(h: &Harness<'_, App>) -> Vec<(String, String)> {
    h.state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .filter_map(|r| r.ip.as_ref().map(|ip| (r.id.to_string(), ip.name.clone())))
        .collect()
}

/// The text of the IP name field.
fn name_field_text(h: &Harness<'_, App>) -> Option<String> {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .accesskit_node()
        .value()
        .map(|v| v.to_string())
}

/// Clicks the IP name field (it takes the focus) and types `text` into it.
fn type_in_name_field(h: &mut Harness<'_, App>, text: &str) {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .click();
    settle(h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .type_text(text);
    settle(h);
}

/// The play-test of cycle 40 (finding 5): a name typed in Tool options and left by a click on the canvas, without
/// Enter, names the point that click places; the field empties (its own clear on the lost focus cancels nothing).
#[test]
fn a_typed_name_left_by_a_click_on_the_canvas_names_the_placed_point() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    type_in_name_field(&mut h, "0V");
    assert_eq!(click_canvas_centre(&mut h), vec!["0V".to_owned()]);
    // The field emptied when it lost the focus.
    assert_eq!(name_field_text(&h).unwrap_or_default(), "");
}

/// The cycle-41 review (finding 3): a name typed and then erased is cancelled, so the point the next click places
/// takes its automatic name, not the last text the field sent ("0").
#[test]
fn a_typed_name_erased_from_the_field_is_cancelled() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    type_in_name_field(&mut h, "0V");
    press(&mut h, Key::Backspace);
    press(&mut h, Key::Backspace);
    assert_eq!(name_field_text(&h).unwrap_or_default(), "");
    match ghost(&h) {
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { name, .. }) => assert_eq!(name, "IP"),
        other => panic!("not the IP tool's ghost: {other:?}"),
    }
    assert_eq!(click_canvas_centre(&mut h), vec!["IP".to_owned()]);
}

/// A click in the middle of the canvas (no Enter before it); returns the names of the interruption points it placed.
fn click_canvas_centre(h: &mut Harness<'_, App>) -> Vec<String> {
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let at = egui::pos2(
        ((bounds.x0 + bounds.x1) / 2.0) as f32,
        ((bounds.y0 + bounds.y1) / 2.0) as f32,
    );
    let before = ips(h);
    h.event(egui::Event::PointerMoved(at));
    settle(h);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(h);
    ips(h)
        .into_iter()
        .filter(|ip| !before.contains(ip))
        .map(|(_, name)| name)
        .collect()
}
