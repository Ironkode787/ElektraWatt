// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the window without rendering (NFR-A11Y-01): the part dialog by keyboard alone (typing
//! searches, no row is active until Down takes one, Enter chooses it) and its AccessKit tree (the result list a
//! `ListBox` named "Parts" with `ListBoxOption` rows carrying position and set size, the star a button named "Favourite"); the part
//! editor by keyboard alone (its fields named by their labels, Enter saves into the `part` card). The accessible-name
//! walker runs after every step.

use crate::support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_viewmodel::shell::Dialog;
use serde_json::json;
use support::{Tmp, dispatch, settle};

fn on_sample(tmp: &Tmp) -> Harness<'static, App> {
    support::on_sample_first_page(tmp)
}

fn field<'a, 'h>(h: &'h Harness<'a, App>, name: &str) -> egui_kittest::Node<'h> {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no field {name}"))
}

#[test]
fn the_part_dialog_by_keyboard_and_its_accessibility_tree() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp);
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-101F1"]}));
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
    assert!(
        first.is_selected() != Some(true),
        "no row is chosen for the person"
    );
    assert!(
        first
            .label()
            .is_some_and(|l| l.starts_with("EW-GEN-MCB-4 (Generic)")),
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
    // Down from no row takes the first, a second Down the second.
    h.key_press(Key::ArrowDown);
    settle(&mut h);
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
    let (_, row) = ew_viewmodel::properties::find_row(sheet, "device/parts").unwrap();
    assert!(
        ew_viewmodel::properties::value_text(row).contains(&second),
        "the second row was chosen: {second}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// An empty search says it once, in the list, and Choose is disabled with its reason.
#[test]
fn the_empty_search_says_it_once_and_chooses_nothing() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp);
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-101F1"]}));
    dispatch(&mut h, "part.choose", json!({"use": 0}));
    dispatch(&mut h, "part.search", json!({"text": "zz-no-such-part"}));
    settle(&mut h);
    settle(&mut h);
    let said = "No part matches \u{201c}zz-no-such-part\u{201d}";
    assert_eq!(h.query_all_by_label(said).count(), 1, "said once");
    assert!(
        h.get_by_label("Choose").accesskit_node().is_disabled(),
        "Choose is disabled"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_part_editor_by_keyboard() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp);
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-101F1"]}));
    dispatch(&mut h, "parts.open", json!({}));
    settle(&mut h);
    settle(&mut h);
    assert!(
        h.state()
            .shell()
            .parts_place()
            .and_then(|p| p.sheet.as_ref())
            .is_some()
    );
    // The starter part is made the person's own, where its fields are typed into.
    dispatch(&mut h, "parts.own", json!({}));
    settle(&mut h);
    settle(&mut h);
    // Every field is named by its label.
    for label in ["Maker", "Part number", "Description", "Sold by"] {
        let _ = field(&h, label);
    }
    // The first text field has the focus; Tab moves to Description (after Part number, Type and Order number), End to
    // its end, where " C4" is typed.
    assert!(
        field(&h, "Maker").accesskit_node().is_focused(),
        "the first field has the focus"
    );
    for _ in 0..4 {
        h.key_press(Key::Tab);
        settle(&mut h);
    }
    assert!(field(&h, "Description").accesskit_node().is_focused());
    h.key_press(Key::End);
    h.event(egui::Event::Text(" C4".to_owned()));
    settle(&mut h);
    let Some(e) = h.state().shell().parts_place().and_then(|p| p.sheet.as_ref()) else {
        panic!("the part in Parts")
    };
    assert_eq!(
        e.sheet.text("description"),
        "Miniature circuit breaker, 2 poles C4"
    );
    h.key_press(Key::Enter);
    settle(&mut h);
    settle(&mut h);
    let message = h
        .state()
        .shell()
        .parts_place()
        .and_then(|p| p.sheet.as_ref())
        .and_then(|o| o.foot_message());
    assert!(message.is_some(), "Enter saves: a part needs its number first");
    // The technical data stand in a section of their own (picture A), each value named in words.
    dispatch(&mut h, "parts.open", json!({}));
    settle(&mut h);
    settle(&mut h);
    let _ = field(&h, "Rated current");
    assert!(
        h.get_all_by_role(egui::Role::TextInput)
            .all(|n| n.accesskit_node().label().as_deref() != Some("rated_current")),
        "a raw technical-data key"
    );
    h.state_mut().shell_mut().shutdown();
}
