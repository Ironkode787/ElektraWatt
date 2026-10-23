// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The find pane in the window without rendering (WP-S2.05 part 2, acceptance checks FR3 in kittest and the window
//! half of FR6's exact counts): Ctrl+H opens find and replace with the text field focused, typing makes no search,
//! Enter runs it once, the 20 results are a named list box, Ctrl+Enter previews the replace in the card and a second
//! Ctrl+Enter applies it (one undo step), Ctrl+Z undoes it; Escape closes the card, then gives the focus back to the
//! canvas; the accessible-name walker after every step. Keyboard only.

mod support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Focus, Outcome};
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

fn results(h: &Harness<'_, App>) -> usize {
    h.state()
        .shell()
        .find_state()
        .results()
        .map_or(0, |r| r.hits.len())
}

#[test]
fn fr3_find_and_replace_from_the_keyboard() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "204");
    // Ctrl+H: find and replace below the page view, the focus in the pane and its text field.
    h.key_press_modifiers(Modifiers::COMMAND, Key::H);
    settle(&mut h);
    assert!(h.state().shell().find_shown());
    assert_eq!(h.state().shell().focus(), Some(Focus::Find));
    assert!(h.state().shell().find_state().replace_mode());
    assert!(
        h.get_all_by_role(egui::Role::Group)
            .any(|n| n.accesskit_node().label().as_deref() == Some("Find and replace")),
        "the pane is a named group"
    );
    // The scope through the pane's own actions (its checkboxes and filter field are named; see the walker).
    dispatch(&mut h, "find.fields", json!({"fields": ["function_text"]}));
    dispatch(&mut h, "find.folios", json!({"folios": ["204", "205", "206"]}));
    settle(&mut h);
    // Typing makes no search.
    let calls = h.state().shell().session_calls();
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Find"))
        .expect("the Find field");
    field.type_text("Spare");
    settle(&mut h);
    assert_eq!(h.state().shell().find_state().text(), "Spare");
    assert_eq!(
        h.state().shell().session_calls().find,
        calls.find,
        "typing makes no call"
    );
    // Tab to the replacement, type it, Enter runs the search: one call.
    h.key_press(Key::Tab);
    settle(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Replace with"))
        .expect("the replacement field");
    field.type_text("Reserve");
    settle(&mut h);
    assert_eq!(h.state().shell().find_state().replacement(), "Reserve");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(
        h.state().shell().session_calls().find,
        calls.find + 1,
        "Enter runs it once"
    );
    assert_eq!(results(&h), 20);
    let options: Vec<String> = h
        .get_all_by_role(egui::Role::ListBoxOption)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.contains("Function text"))
        .collect();
    assert_eq!(options.len(), 20, "{options:?}");
    assert!(
        options.iter().all(|o| o.contains("[[Spare]] -> Reserve")),
        "the matches are marked in the names: {options:?}"
    );
    // Down moves the cursor (no call).
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    assert_eq!(h.state().shell().find_state().row(), Some(1));
    // Ctrl+Enter: the card; Ctrl+Enter again: Apply, one undo step.
    let steps = h.state().shell().recorded_commands().len();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    let card = h.state().shell().properties().card().expect("the replace card");
    assert_eq!(card.changes, 20);
    assert!(
        h.get_all_by_role(egui::Role::Button)
            .any(|n| n.accesskit_node().label().as_deref() == Some("Apply")),
        "the card's Apply"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none());
    assert_eq!(h.state().shell().recorded_commands().len(), steps + 1);
    assert_eq!(
        h.state().shell().find_state().note(),
        Some("Replaced 20; run again to refresh")
    );
    // Ctrl+Z undoes the replace.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    dispatch(&mut h, "find.run", json!({}));
    settle(&mut h);
    assert_eq!(results(&h), 20, "undo restores all 20");
    // Escape: the card first, then the focus back to the canvas.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_some());
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_none());
    assert_eq!(h.state().shell().focus(), Some(Focus::Find));
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(matches!(h.state().shell().focus(), Some(Focus::Canvas(_))));
}
