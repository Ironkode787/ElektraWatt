// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4a in the window without rendering (PS9, NFR-A11Y-01): the part dialog by keyboard alone (typing
//! searches, Down moves the active row, Enter chooses it) and its AccessKit tree (the result list a `ListBox` named
//! "Parts" with `ListBoxOption` rows carrying position and set size, the star a button named "Favourite"); the part
//! editor by keyboard alone (its fields named by their labels, Enter saves into the `part` card). The accessible-name
//! walker runs after every step.

use crate::support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::properties::CardOrigin;
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
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

fn field<'a, 'h>(h: &'h Harness<'a, App>, name: &str) -> egui_kittest::Node<'h> {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no field {name}"))
}

#[test]
fn the_part_dialog_by_keyboard_and_its_accessibility_tree() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "select.set", json!({"refs": ["tag:F55"]}));
    dispatch(&mut h, "part.choose", json!({"use": 0}));
    settle(&mut h);
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::PartChoose(_))));
    // The list and its rows.
    let list = h.get_by_role_and_label(egui::Role::ListBox, "Parts");
    let _ = list;
    let rows: Vec<_> = h.get_all_by_role(egui::Role::ListBoxOption).collect();
    assert!(rows.len() >= 2, "rows: {}", rows.len());
    let first = rows[0].accesskit_node();
    assert_eq!(first.data().position_in_set(), Some(1));
    assert_eq!(first.data().size_of_set(), Some(rows.len()));
    assert!(first.is_selected() == Some(true), "the first row is active");
    assert!(
        first
            .label()
            .is_some_and(|l| l.starts_with("EW-GEN-MCB-2 (Generic)")),
        "{:?}",
        first.label()
    );
    assert!(
        h.get_all_by_role_and_label(egui::Role::Button, "Favourite")
            .next()
            .is_some()
    );
    // Typing searches (the search field has the focus); Down moves the active row; Enter chooses it.
    let search = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().data().placeholder() == Some("Search parts"))
        .expect("the search field");
    assert!(
        search.accesskit_node().is_focused(),
        "the search field has the first focus"
    );
    h.event(egui::Event::Text("MCB".to_owned()));
    settle(&mut h);
    let Some(Dialog::PartChoose(c)) = h.state().shell().dialog() else {
        panic!("the part dialog")
    };
    assert_eq!(c.query.text, "MCB");
    let second = c.offers[1].part_number.clone();
    let n = h.state().shell().recorded_commands().len();
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "Enter chose");
    let shell = h.state().shell();
    assert_eq!(shell.recorded_commands().len(), n + 1);
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "part.assign", "{}", last.params);
    let sheet = shell.properties().sheet().unwrap();
    let (_, row) = ew_ui::ew_viewmodel::properties::find_row(sheet, "device/parts").unwrap();
    assert!(
        ew_ui::ew_viewmodel::properties::value_text(row).contains(&second),
        "the second row was chosen: {second}"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_part_editor_by_keyboard() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "select.set", json!({"refs": ["tag:F55"]}));
    dispatch(&mut h, "part.edit", json!({}));
    settle(&mut h);
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::PartEdit(_))));
    // Every field is named by its label.
    for label in [
        "Manufacturer",
        "Part number",
        "Description",
        "Unit",
        "Rated current",
    ] {
        let _ = field(&h, label);
    }
    // A technical-data key reads in words, never as the raw key.
    assert!(
        h.get_all_by_role(egui::Role::TextInput)
            .all(|n| n.accesskit_node().label().as_deref() != Some("rated_current")),
        "a raw technical-data key"
    );
    // The first field has the focus; Tab moves to Description (after Part number, Type designation, Order number,
    // GTIN), End to its end, where " C4" is typed.
    assert!(
        field(&h, "Manufacturer").accesskit_node().is_focused(),
        "the first field has the focus"
    );
    for _ in 0..5 {
        h.key_press(Key::Tab);
        settle(&mut h);
    }
    assert!(field(&h, "Description").accesskit_node().is_focused());
    h.key_press(Key::End);
    h.event(egui::Event::Text(" C4".to_owned()));
    settle(&mut h);
    let Some(Dialog::PartEdit(e)) = h.state().shell().dialog() else {
        panic!("the part editor")
    };
    assert_eq!(e.draft.text("description"), "Miniature circuit breaker 1P+N C4");
    h.key_press(Key::Enter);
    settle(&mut h);
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "Enter saved");
    let card = h.state().shell().properties().card().expect("the part card");
    assert!(matches!(card.origin, CardOrigin::Part(_)));
    h.state_mut().shell_mut().shutdown();
}
