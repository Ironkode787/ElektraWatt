// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties explains itself (the owner's hands-on test of 0.1.0, FT-1648 to FT-1651), through the window: a
//! category folded by a click on its header and by Enter on it; a row's explanation in the tooltip of its label; a
//! connection point marked not used from its row and from the page's right-click menu; a wire's potential, its
//! voltage set from the wire, and the source of a derived colour.

use crate::support;

use egui::{Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::properties::{RowValue, find_row};
use ew_ui::ew_viewmodel::shell::Focus;
use serde_json::json;
use support::{Tmp, dispatch, settle};

/// -101F1's symbol on folio 101.
const F1_SYMBOL: &str = "7b76f7a3-8966-50ed-a194-36d38ac1c0a1";
/// W29 on folio 103 (`connections|W29`).
const W29: &str = "9eb2d865-fca4-59b1-87bf-c073d7190ceb";

/// FT-1650: a selected symbol shows its device's, its function's and its representation's sections. A click on the
/// representation's Placement header folds that category (its rows go, its chevron turns), a second click unfolds it,
/// and Enter on the focused header folds it again; the fold is kept when the selection changes and comes back.
#[test]
fn a_click_on_a_category_header_folds_it() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "101");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(&mut h, "select.set", json!({"ids": [F1_SYMBOL]}));
    settle(&mut h);
    let collapsed = |h: &Harness<'_, App>| {
        h.state()
            .shell()
            .properties_collapsed()
            .into_iter()
            .collect::<Vec<_>>()
    };
    let headers = |h: &Harness<'_, App>| {
        h.get_all_by_role(egui::Role::Button)
            .filter(|n| n.accesskit_node().label().as_deref() == Some("Placement"))
            .count()
    };
    // The device's section has a Placement category too, above the representation's.
    assert_eq!(headers(&h), 2);
    assert!(collapsed(&h).is_empty());
    last_placement(&h).scroll_to_me();
    settle(&mut h);
    last_placement(&h).click();
    settle(&mut h);
    assert_eq!(collapsed(&h), ["representation/placement"]);
    assert!(!drawn(&h, "Pos"), "the folded rows go");
    last_placement(&h).click();
    settle(&mut h);
    assert!(collapsed(&h).is_empty());
    assert!(drawn(&h, "Pos"));
    // From the keyboard: Enter and Space on the focused header.
    last_placement(&h).focus();
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(collapsed(&h), ["representation/placement"]);
    h.key_press(Key::Space);
    settle(&mut h);
    assert!(collapsed(&h).is_empty());
    h.key_press(Key::Space);
    settle(&mut h);
    // Another selection and back: the fold stays.
    dispatch(&mut h, "select.set", json!({"ids": []}));
    dispatch(&mut h, "select.set", json!({"ids": [F1_SYMBOL]}));
    settle(&mut h);
    assert_eq!(collapsed(&h), ["representation/placement"]);
    assert!(!drawn(&h, "Pos"));
    h.state_mut().shell_mut().shutdown();
}

/// FT-1648: the pointer resting on a row's label shows what the row is in its tooltip: the Tag row of -101F1 says how
/// the tag is made.
#[test]
fn a_rows_tooltip_explains_it() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "101");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    dispatch(&mut h, "view.reduce_motion", json!({"on": false}));
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(&mut h, "select.set", json!({"ids": [F1_SYMBOL]}));
    settle(&mut h);
    let help =
        ew_ui::ew_viewmodel::strings::field_help("device", "derived.tag").expect("the Tag row's explanation");
    let label = h
        .get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Tag"))
        .expect("the Tag row's label")
        .rect();
    h.hover_at(label.center());
    let start = h.ctx.input(|i| i.time);
    let mut shown = false;
    for i in 0..40 {
        h.input_mut().time = Some(start + f64::from(i) * 0.05);
        h.step();
        shown |= h
            .get_all_by_role(egui::Role::Label)
            .any(|n| n.accesskit_node().value().as_deref() == Some(help));
    }
    assert!(shown, "the tooltip: {help}");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1649 from Properties: a right-click on -101F1's first connection point row offers Mark not used; its click marks
/// the point not used in one undo step, and the row says so; the menu then offers Mark used.
#[test]
fn a_connection_point_is_marked_not_used_from_its_row() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "101");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(&mut h, "select.set", json!({"ids": [F1_SYMBOL]}));
    settle(&mut h);
    let key = "device/derived.cp[0]";
    let point = |h: &Harness<'_, App>| match find_row(h.state().shell().properties().sheet().unwrap(), key)
        .map(|(_, r)| r.value.clone())
    {
        Some(RowValue::One { json, text }) => (json, text),
        other => panic!("{other:?}"),
    };
    let (json, label) = point(&h);
    let (function, idx) = (
        json["function"].as_str().unwrap().parse().unwrap(),
        json["idx"].as_u64().unwrap() as u16,
    );
    assert!(!h.state().shell().cp_unused(function, idx));
    let rev = h.state().shell().revision();
    choose_from_row_menu(&mut h, "Connection point 1", "Mark not used");
    assert!(h.state().shell().cp_unused(function, idx), "marked not used");
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1), "one step");
    assert_eq!(point(&h).1, format!("{label} · not used"));
    choose_from_row_menu(&mut h, "Connection point 1", "Mark used");
    assert!(!h.state().shell().cp_unused(function, idx), "used again");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    assert!(h.state().shell().cp_unused(function, idx), "the undo");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1649 on the page: a right-click on -102F2's unconnected connection point 13 offers to mark that point not used;
/// its click marks it, and its unconnected-point finding drops to an info.
#[test]
fn a_connection_point_is_marked_not_used_from_the_page() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "102");
    let cp = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .cps
        .iter()
        .find(|c| c.label == "-102F2:13")
        .cloned()
        .expect("-102F2:13 on folio 102");
    let sheet = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .snapper
        .page_to_sheet(cp.at);
    let at = support::screen(&h, sheet);
    h.hover_at(at);
    settle(&mut h);
    h.event(egui::Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.event(egui::Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    settle(&mut h);
    h.get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Mark -102F2:13 not used"))
        .expect("Mark -102F2:13 not used in the page's menu")
        .click();
    settle(&mut h);
    assert!(h.state().shell().cp_unused(cp.function, cp.idx));
    // Its finding of an unconnected connection point is an info now.
    let findings: Vec<(String, String)> = h
        .state()
        .shell()
        .properties()
        .sheet()
        .map(|s| {
            s.header
                .findings
                .iter()
                .map(|f| (f.severity.clone(), f.message.clone()))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        findings
            .iter()
            .any(|(severity, message)| severity == "info" && message.contains("marked unused")),
        "{findings:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-1651: W29's sheet shows its potential's definition, with its type and voltage; the voltage is typed from the
/// wire's sheet and set in one undo step. Its colour's chip names the colour profile, and Go to source shows the
/// project's colour profile row.
#[test]
fn a_wire_shows_and_sets_its_potential_and_the_source_of_its_colour() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "103");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(&mut h, "select.set", json!({"ids": [W29]}));
    settle(&mut h);
    let sheet = h.state().shell().properties().sheet().unwrap().clone();
    assert!(
        sheet.sections.iter().any(|s| s.kind == "potential_def"),
        "the potential's definition"
    );
    assert!(find_row(&sheet, "potential_def/ptype").is_some());
    // Its point by its printed label, never its JSON.
    let at = find_row(&sheet, "potential_def/at").map(|(_, r)| r.value.clone());
    assert!(
        matches!(&at, Some(RowValue::One { text, .. }) if !text.starts_with('{') && text.contains(':')),
        "{at:?}"
    );
    let voltage = |h: &Harness<'_, App>| match find_row(
        h.state().shell().properties().sheet().unwrap(),
        "potential_def/voltage",
    )
    .map(|(_, r)| r.value.clone())
    {
        Some(RowValue::One { text, .. }) => Some(text),
        _ => None,
    };
    let before = voltage(&h);
    assert_ne!(before.as_deref(), Some("230 V"));
    // The keyboard: E focuses the grid, the cursor to the Voltage row, Enter opens it, 230, Enter.
    h.key_press(Key::E);
    settle(&mut h);
    assert_eq!(h.state().shell().focus(), Some(Focus::Properties));
    row_to(&mut h, "potential_def/voltage");
    h.key_press(Key::Enter);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    settle(&mut h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Voltage"))
        .expect("the Voltage field")
        .type_text("230");
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert!(
        voltage(&h).is_some_and(|v| v.starts_with("230")),
        "{:?}",
        voltage(&h)
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    assert_eq!(voltage(&h), before);
    // The colour comes from the colour profile: its chip says so, and Go to source shows that row.
    let colour = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Colour"))
        .expect("the Colour row");
    assert!(
        colour
            .accesskit_node()
            .description()
            .is_some_and(|d| d.contains("colour profile")),
        "{:?}",
        colour.accesskit_node().description()
    );
    choose_from_row_menu(&mut h, "Colour", "Go to source");
    assert_eq!(
        h.state().shell().properties_row().as_deref(),
        Some("project/settings.colour_profile")
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-1651, the owner's case: an interruption point whose potential no definition names shows its Potential row, and
/// that row's menu offers Define potential…, which opens its form at the point; the potential defined there shows its
/// voltage under the point's sheet.
#[test]
fn an_interruption_point_without_a_definition_offers_define_potential() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "101");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    dispatch(&mut h, "props.show", json!({"on": true}));
    // The interruption point J-E-206-L on folio 101; its potential is defined at 1X1:1/b, which goes.
    let ip = "f4fdc202-6465-54c2-9ef8-dacbf90449f4";
    dispatch(&mut h, "select.set", json!({ "ids": [ip] }));
    settle(&mut h);
    let def = h
        .state()
        .shell()
        .properties()
        .sheet()
        .and_then(|s| s.sections.iter().find(|s| s.kind == "potential_def"))
        .expect("the potential's definition")
        .targets[0];
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "potential_def", "id": def.to_string()}]}}),
    );
    dispatch(&mut h, "select.set", json!({ "ids": [ip] }));
    settle(&mut h);
    let sheet = h.state().shell().properties().sheet().unwrap().clone();
    assert!(sheet.sections.iter().all(|s| s.kind != "potential_def"));
    assert!(find_row(&sheet, "function/derived.potential").is_some());
    choose_from_row_menu(&mut h, "Potential", "Define potential…");
    assert_eq!(
        h.state()
            .shell()
            .palette()
            .and_then(|p| p.form.as_ref())
            .map(|f| f.action),
        Some("potential.draw")
    );
    // The form's submission.
    dispatch(
        &mut h,
        "potential.draw",
        json!({"name": "L", "type": "L", "voltage": "230"}),
    );
    dispatch(&mut h, "select.set", json!({ "ids": [ip] }));
    settle(&mut h);
    let sheet = h.state().shell().properties().sheet().unwrap().clone();
    let voltage = find_row(&sheet, "potential_def/voltage").map(|(_, r)| r.value.clone());
    assert!(
        matches!(&voltage, Some(RowValue::One { text, .. }) if text.starts_with("230")),
        "{voltage:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// Opens the actions menu of the row labelled `row` with a right-click and chooses `item`.
fn choose_from_row_menu(h: &mut Harness<'_, App>, row: &str, item: &str) {
    let node = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some(row))
        .unwrap_or_else(|| panic!("the row {row}"));
    node.scroll_to_me();
    settle(h);
    h.get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some(row))
        .unwrap()
        .click_secondary();
    settle(h);
    let items: Vec<String> = h
        .get_all_by_role(egui::Role::MenuItem)
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    h.get_all_by_role(egui::Role::MenuItem)
        .find(|n| n.accesskit_node().label().as_deref() == Some(item))
        .unwrap_or_else(|| panic!("{item} in {items:?}"))
        .click();
    settle(h);
}

/// Moves the grid's row cursor with the arrow keys until it is on `key` (bounded).
fn row_to(h: &mut Harness<'_, App>, key: &str) {
    h.key_press(Key::Home);
    settle(h);
    for _ in 0..300 {
        if h.state().shell().properties_row().as_deref() == Some(key) {
            return;
        }
        h.key_press(Key::ArrowDown);
        settle(h);
    }
    panic!("the row cursor did not reach {key}");
}

/// Whether the grid drew the row labelled `label` (its named `Group` node).
fn drawn(h: &Harness<'_, App>, label: &str) -> bool {
    h.get_all_by_role(egui::Role::Group)
        .any(|n| n.accesskit_node().label().as_deref() == Some(label))
}

/// The last Placement header of the grid in layout order: the representation's.
fn last_placement<'h>(h: &'h Harness<'_, App>) -> egui_kittest::Node<'h> {
    let mut all: Vec<_> = h
        .get_all_by_role(egui::Role::Button)
        .filter(|n| n.accesskit_node().label().as_deref() == Some("Placement"))
        .collect();
    all.sort_by(|a, b| a.rect().top().total_cmp(&b.rect().top()));
    all.pop().expect("a Placement header")
}
