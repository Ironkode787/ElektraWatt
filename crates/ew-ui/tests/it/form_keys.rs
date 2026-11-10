// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A form dialog's submit keys and refusals (rulings 2 and 7, window check W49.1-A):
//! Ctrl+Enter submits a form dialog from any focus, and the form takes the key before the Ctrl+Enter of what lies
//! under it (the find pane's replace); a refused submit says why in the form and keeps it open, whether it came from a
//! click on the Primary, Enter on the focused Primary or Ctrl+Enter.

use crate::support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::properties::CardOrigin;
use ew_ui::ew_viewmodel::shell::{Dialog, Focus};
use serde_json::json;
use support::{Tmp, dispatch, on_golden, settle};

/// Types `text` as keyboard input into whatever has the focus.
fn type_keys(h: &mut Harness<'_, App>, text: &str) {
    h.event(egui::Event::Text(text.to_owned()));
    settle(h);
}

/// The open form's action and refusal, if a form is open.
fn form(h: &Harness<'_, App>) -> Option<(&'static str, Option<String>)> {
    match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => Some((f.action, f.error.clone())),
        _ => None,
    }
}

#[test]
fn ctrl_enter_submits_a_form_from_any_focus_before_the_keys_under_it() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    // The find pane with a replacement typed and the search run: its Ctrl+Enter would build the replace card.
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    type_keys(&mut h, "Spare");
    h.key_press(Key::Tab);
    settle(&mut h);
    type_keys(&mut h, "Reserve");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().shell().focus(), Some(Focus::Find));
    assert!(!h.ctx.text_edit_focused());
    // Ctrl+Shift+N: Renumber devices…, whose form has no text field (its first field is a choice).
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::N);
    settle(&mut h);
    assert_eq!(form(&h), Some(("devices.renumber", None)));
    // A refused submit makes no card, so the find pane's Ctrl+Enter, had it run, would leave its own.
    scope_selection(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    refused_in_the_form(&h, "Ctrl+Enter over the find pane");
    assert!(
        h.state().shell().properties().card().is_none(),
        "no find card: {:?}",
        h.state().shell().properties().card().map(|c| &c.origin)
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    assert_eq!(form(&h), None);
    // With the find pane's replace card open, its Ctrl+Enter would apply it: the form takes the key, nothing applied.
    dispatch(&mut h, "find.replace_all", json!({}));
    settle(&mut h);
    assert!(matches!(
        h.state().shell().properties().card().map(|c| &c.origin),
        Some(CardOrigin::Find { .. })
    ));
    let recorded = h.state().shell().recorded_commands().len();
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::N);
    settle(&mut h);
    assert_eq!(form(&h), Some(("devices.renumber", None)));
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    assert_eq!(form(&h), None, "Ctrl+Enter submitted the form");
    assert_eq!(
        h.state().shell().recorded_commands().len(),
        recorded,
        "the find card not applied"
    );
    let card = h.state().shell().properties().card().expect("the renumber card");
    assert!(
        matches!(card.origin, CardOrigin::Bulk),
        "the form's card, not the find pane's replace: {:?}",
        card.origin
    );
    h.state_mut().shell_mut().shutdown();
}

/// `key` with `modifiers` as one event, so one frame: kittest runs a frame per queued event, and
/// `key_press_modifiers` queues the modifier keys too.
fn chord_in_one_frame(h: &mut Harness<'_, App>, modifiers: Modifiers, key: Key) {
    h.event(egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    });
    h.step();
}

/// The find pane, focused, under the frame that opens a form (Ctrl+Shift+N) and the next one (Ctrl+Enter): on the
/// form's first frame no modal layer stands yet, so the pane still holds egui's focus and reads its own keys; only
/// the frame's consume keeps Ctrl+Enter with the form (C49-p5b#2).
#[track_caller]
fn open_the_form_and_ctrl_enter_in_one_frame(h: &mut Harness<'_, App>, scope: Option<&str>) {
    let pane = ew_ui::ew_viewmodel::strings::text(ew_ui::ew_viewmodel::key!("pane-find"));
    assert_eq!(h.state().shell().focus(), Some(Focus::Find));
    assert!(h.get_by_role_and_label(egui::Role::Group, pane).is_focused());
    chord_in_one_frame(h, Modifiers::COMMAND | Modifiers::SHIFT, Key::N);
    assert_eq!(form(h), Some(("devices.renumber", None)));
    if let Some(scope) = scope {
        h.state_mut().shell_mut().set_form_field("scope", scope);
    }
    chord_in_one_frame(h, Modifiers::COMMAND, Key::Enter);
    settle(h);
}

#[test]
fn ctrl_enter_on_the_forms_first_frame_stays_with_the_form() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    type_keys(&mut h, "Spare");
    h.key_press(Key::Tab);
    settle(&mut h);
    type_keys(&mut h, "Reserve");
    h.key_press(Key::Enter);
    settle(&mut h);
    // A refused submit makes no card: the find pane's Ctrl+Enter would have built its replace card.
    open_the_form_and_ctrl_enter_in_one_frame(&mut h, Some("selection"));
    refused_in_the_form(&h, "Ctrl+Enter on the form's first frame");
    assert!(
        h.state().shell().properties().card().is_none(),
        "no find card: {:?}",
        h.state().shell().properties().card().map(|c| &c.origin)
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    assert_eq!(form(&h), None);
    // An accepted submit on the form's first frame: the form's card.
    open_the_form_and_ctrl_enter_in_one_frame(&mut h, None);
    assert_eq!(form(&h), None, "Ctrl+Enter submitted the form");
    let card = h.state().shell().properties().card().expect("the renumber card");
    assert!(matches!(card.origin, CardOrigin::Bulk), "{:?}", card.origin);
    h.state_mut().shell_mut().shutdown();
}

/// Sets the open form's scope to Selection (which clears its last refusal) with nothing selected.
fn scope_selection(h: &mut Harness<'_, App>) {
    h.state_mut().shell_mut().set_form_field("scope", "selection");
    settle(h);
    assert_eq!(form(h), Some(("devices.renumber", None)));
}

/// The refusal's whole text in the form and as the dialog's label.
#[track_caller]
fn refused_in_the_form(h: &Harness<'_, App>, how: &str) {
    const WHY: &str = "No device is selected. Select the devices first, or choose another scope.";
    assert_eq!(form(h), Some(("devices.renumber", Some(WHY.to_owned()))), "{how}");
    assert!(h.query_by_label(WHY).is_some(), "{how}: shown in the form");
}

#[test]
fn w49_1a_a_refused_renumber_says_why_in_the_form_by_click_enter_and_ctrl_enter() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    assert!(h.state().shell().selection().is_empty());
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::N);
    settle(&mut h);
    // A click on the Primary.
    scope_selection(&mut h);
    support::off_chrome(&h, "Renumber")
        .expect("the form's Renumber")
        .click();
    settle(&mut h);
    refused_in_the_form(&h, "a click");
    // Ctrl+Enter.
    scope_selection(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    refused_in_the_form(&h, "Ctrl+Enter");
    // Enter on the Primary, reached with Tab.
    scope_selection(&mut h);
    let mut tabs = 0;
    while !support::off_chrome(&h, "Renumber")
        .expect("the form's Renumber")
        .accesskit_node()
        .is_focused()
    {
        tabs += 1;
        assert!(tabs <= 12, "Tab reaches the Primary");
        h.key_press(Key::Tab);
        settle(&mut h);
    }
    h.key_press(Key::Enter);
    settle(&mut h);
    refused_in_the_form(&h, "Enter");
    h.state_mut().shell_mut().shutdown();
}
