// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 4b in the window without rendering (PI7, §10.5): the text popup takes the focus when the Text tool
//! opens it, Enter sends the text to the tool and places it, and Escape closes it without a commit. The Wire marker
//! and Structure box tools arm on their shortcuts, and the marker's cross-section field in the tool strip sends
//! `marker.fields`.

use crate::support;
use ew_viewmodel::key;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::strings;
use ew_ui::ew_viewmodel::tools::Ghost;
use serde_json::json;
use support::{Tmp, dispatch, on_golden, screen, settle, wait_for_page};

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
    strings::text(key!("action-text-input"))
}

/// Opened by a click on the page, as a user opens it (the press and the release in
/// frames of their own), the popup's field takes the focus, so the first typing, in the next frame, lands in it.
#[test]
fn a_click_with_the_text_tool_gives_the_popup_s_field_the_focus() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "tool.text", json!({}));
    settle(&mut h);
    let pos = screen(&h, (100_000_000, 100_000_000));
    h.event(egui::Event::PointerMoved(pos));
    h.run_ok();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.run_ok();
    }
    h.event(egui::Event::Text("a".to_owned()));
    h.run_ok();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().state(), "typing");
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label()))
        .expect("the text popup's field");
    assert!(field.is_focused(), "the popup's field keeps the focus");
    assert!(
        matches!(h.state().shell().tool_ghost(), Some(Ghost::Text { text, .. }) if text == "a"),
        "the first typing reaches the field"
    );
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

#[test]
fn shift_w_and_shift_b_arm_the_marker_and_structure_box_tools() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    h.key_press_modifiers(Modifiers::SHIFT, Key::W);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "marker");
    h.key_press(Key::Escape);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    h.key_press_modifiers(Modifiers::SHIFT, Key::B);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "structure_box");
}

#[test]
fn the_marker_fields_in_the_tool_strip_send_the_cross_section() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "tool.marker", json!({}));
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "marker");
    let label = strings::text(key!("action-marker-csa"));
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label))
        .expect("the cross-section field");
    field.click();
    settle(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label))
        .expect("the cross-section field");
    field.type_text("2.5");
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    settle(&mut h);
    match h.state().shell().tool_ghost() {
        Some(Ghost::Marker { connections, text }) => {
            assert_eq!(connections, 0);
            assert_eq!(text.as_deref(), Some("2.5"));
        }
        other => panic!("not the Wire marker's ghost: {other:?}"),
    }
}

/// Edits an accessible field, leaving its draft unsubmitted unless Enter is requested.
fn type_field(h: &mut Harness<'_, App>, key: strings::Key<'_>, text: &str, enter: bool) {
    let label = strings::text(key);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label))
        .unwrap()
        .click();
    settle(h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    settle(h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label))
        .unwrap()
        .type_text(text);
    settle(h);
    if enter {
        h.key_press(Key::Enter);
        settle(h);
        settle(h);
    }
}

fn assert_field(h: &Harness<'_, App>, key: strings::Key<'_>, expected: &str) {
    let label = strings::text(key);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label))
        .unwrap();
    assert_eq!(
        field.accesskit_node().value().as_deref(),
        Some(expected),
        "{key:?}"
    );
}

fn activate(h: &mut Harness<'_, App>, action: &str) {
    dispatch(h, action, json!({}));
    settle(h);
    settle(h);
}

/// Re-arming the marker tool, after another tool or directly, empties its fields and its ghost text.
fn rearm_marker(h: &mut Harness<'_, App>) {
    activate(h, "tool.marker");
    for switch_away in [true, false] {
        type_field(h, key!("action-marker-csa"), "2.5", true);
        type_field(h, key!("action-marker-colours"), "BK BU", true);
        assert!(
            matches!(h.state().shell().tool_ghost(),Some(Ghost::Marker{text:Some(text),..}) if text=="2.5 BK BU")
        );
        if switch_away {
            activate(h, "tool.text");
        }
        activate(h, "tool.marker");
        assert_field(h, key!("action-marker-csa"), "");
        assert_field(h, key!("action-marker-colours"), "");
        assert!(matches!(
            h.state().shell().tool_ghost(),
            Some(Ghost::Marker {
                connections: 0,
                text: None
            })
        ));
    }
}

fn outline_box(h: &mut Harness<'_, App>) {
    for (x, y) in [(100.0, 100.0), (120.0, 120.0)] {
        dispatch(h, "cursor.set", json!({"x_mm":x,"y_mm":y}));
        dispatch(h, "canvas.activate", json!({}));
        settle(h);
    }
    dispatch(h, "tool.finish", json!({}));
    settle(h);
    assert_eq!(h.state().shell().tool().state(), "typing");
}

/// The marker tool and then the structure box tool in one window: re-arming either empties its visible fields.
#[test]
fn rearming_marker_and_structure_box_resets_visible_fields_to_tool_defaults() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    rearm_marker(&mut h);
    activate(&mut h, "tool.structure_box");
    for switch_away in [true, false] {
        type_field(&mut h, key!("action-structure-box-caption"), "Spare", true);
        type_field(&mut h, key!("action-structure-box-location"), "+J-E-206", true);
        if switch_away {
            activate(&mut h, "tool.text");
        }
        activate(&mut h, "tool.structure_box");
        assert_field(&h, key!("action-structure-box-caption"), "");
        assert_field(&h, key!("action-structure-box-location"), "");
        outline_box(&mut h);
        dispatch(&mut h, "tool.finish", json!({}));
        settle(&mut h);
        let last = h.state().shell().recorded_commands().last().unwrap();
        assert_eq!(last.cmd, "box.structure");
        assert!(last.params.get("caption").is_none_or(|v| v.is_null()));
        assert_eq!(last.params["assigns"], json!({}));
    }
}

#[test]
fn drawing_field_drafts_survive_frames_and_outline_transitions() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    activate(&mut h, "tool.marker");
    type_field(&mut h, key!("action-marker-csa"), "2.5", false);
    settle(&mut h);
    settle(&mut h);
    assert_field(&h, key!("action-marker-csa"), "2.5");
    assert!(matches!(
        h.state().shell().tool_ghost(),
        Some(Ghost::Marker { text: None, .. })
    ));
    dispatch(&mut h, "cursor.set", json!({"x_mm":100.0,"y_mm":100.0}));
    dispatch(&mut h, "canvas.activate", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().state(), "crossing");
    assert_field(&h, key!("action-marker-csa"), "2.5");
    type_field(&mut h, key!("action-marker-csa"), "2.5", true);
    assert!(
        matches!(h.state().shell().tool_ghost(),Some(Ghost::Marker{text:Some(text),..}) if text=="2.5"),
        "{:?}",
        h.state().shell().tool_ghost()
    );
    activate(&mut h, "tool.structure_box");
    type_field(&mut h, key!("action-structure-box-caption"), "Spare", false);
    outline_box(&mut h);
    assert_field(&h, key!("action-structure-box-caption"), "Spare");
    type_field(&mut h, key!("action-structure-box-caption"), "Spare", true);
    dispatch(&mut h, "tool.finish", json!({}));
    settle(&mut h);
    assert_eq!(
        h.state().shell().recorded_commands().last().unwrap().params["caption"],
        "Spare"
    );
}

#[test]
fn page_and_project_changes_do_not_keep_stale_drawing_fields() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    activate(&mut h, "tool.marker");
    type_field(&mut h, key!("action-marker-csa"), "2.5", true);
    dispatch(&mut h, "page.goto", json!({"folio":"102"}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    activate(&mut h, "tool.marker");
    assert_field(&h, key!("action-marker-csa"), "");
    activate(&mut h, "tool.structure_box");
    type_field(&mut h, key!("action-structure-box-caption"), "Spare", true);
    type_field(&mut h, key!("action-structure-box-location"), "+J-E-206", true);
    // Closing and reopening the project replaces the whole host; its epoch must still advance.
    dispatch(&mut h, "file.close", json!({}));
    settle(&mut h);
    let dir = tmp.0.join("g.ew");
    dispatch(&mut h, "file.open", json!({"dir":dir.to_string_lossy()}));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio":"101"}));
    settle(&mut h);
    activate(&mut h, "tool.structure_box");
    assert_field(&h, key!("action-structure-box-caption"), "");
    assert_field(&h, key!("action-structure-box-location"), "");
}

#[test]
fn text_popup_typing_previews_without_commit_and_enter_places_it() {
    use ew_ui::ew_viewmodel::tools::PreviewStatus;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    typing_text(&mut h);
    assert!(
        h.get_all_by_role(egui::Role::TextInput)
            .find(|n| n.accesskit_node().label().as_deref() == Some(label()))
            .expect("the text popup's field")
            .is_focused(),
        "the popup's field takes the focus"
    );
    let rev = h.state().shell().revision();
    let recorded = h.state().shell().recorded_commands().len();
    type_field(&mut h, key!("action-text-input"), "40x60", false);
    assert!(
        matches!(h.state().shell().tool_ghost(),Some(Ghost::Text{text,..}) if text=="40x60"),
        "{:?}",
        h.state().shell().tool_ghost()
    );
    assert!(h.state().shell().tool_overlay().labels().any(|s| s == "40x60"));
    dispatch(&mut h, "canvas.dwell", json!({"ms":80}));
    settle(&mut h);
    assert_eq!(h.state().shell().preview_status(), Some(PreviewStatus::Ready));
    assert_eq!(h.state().shell().revision(), rev);
    assert_eq!(h.state().shell().recorded_commands().len(), recorded);
    h.key_press(Key::Enter);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1));
    let added = &h.state().shell().recorded_commands()[recorded..];
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].cmd, "item.place");
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
fn text_edit_typing_updates_preview_before_enter() {
    use ew_ui::ew_viewmodel::tools::PreviewStatus;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let page = h.state().shell().canvas().unwrap().page().tables.page.to_string();
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd":"item.place","params":{"page":page,
        "kind":{"free_text":{"text":"original","at":[0,0]}}}}),
    );
    let id = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .items
        .iter()
        .find(|i| i.text.as_deref() == Some("original"))
        .unwrap()
        .id
        .to_string();
    dispatch(&mut h, "select.set", json!({"ids":[id]}));
    h.key_press(Key::F2);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "text_edit");
    let rev = h.state().shell().revision();
    let recorded = h.state().shell().recorded_commands().len();
    type_field(&mut h, key!("action-text-input"), "changed", false);
    assert!(matches!(h.state().shell().tool_ghost(),Some(Ghost::Text{text,..}) if text=="changed"));
    assert!(h.state().shell().tool_overlay().labels().any(|s| s == "changed"));
    dispatch(&mut h, "canvas.dwell", json!({"ms":80}));
    settle(&mut h);
    assert_eq!(h.state().shell().preview_status(), Some(PreviewStatus::Ready));
    assert_eq!(h.state().shell().revision(), rev);
    assert_eq!(h.state().shell().recorded_commands().len(), recorded);
    h.key_press(Key::Escape);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().revision(), rev);
    assert_eq!(h.state().shell().recorded_commands().len(), recorded);
    assert!(h.state().shell().text_popup().is_none());
    assert_eq!(
        h.state()
            .shell()
            .canvas()
            .unwrap()
            .page()
            .tables
            .items
            .iter()
            .find(|i| i.id.to_string() == id)
            .unwrap()
            .text
            .as_deref(),
        Some("original")
    );
}

fn tap(h: &mut Harness<'_, App>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.run_ok();
    h.run_ok();
}

#[test]
fn clicking_placed_text_shows_properties_and_f2_edit() {
    use ew_ui::ew_viewmodel::properties::{RowValue, find_row};
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    assert!(h.state().shell().properties_shown());
    typing_text(&mut h);
    type_field(&mut h, key!("action-text-input"), "40x60", true);
    let item = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .items
        .iter()
        .find(|i| i.kind == "free_text" && i.text.as_deref() == Some("40x60"))
        .expect("the placed text")
        .clone();
    h.key_press(Key::Escape);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    let [x0, y0, x1, y1] = item.bounds;
    let point = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .snapper
        .page_to_sheet((x0 + (x1 - x0) / 2, y0 + (y1 - y0) / 2));
    let pos = screen(&h, point);
    tap(&mut h, pos);
    settle(&mut h);
    let selection = &h.state().shell().selection().items;
    assert_eq!(selection.len(), 1, "the pointer selects only the placed text");
    assert_eq!(selection[0].0, item.id);
    let assert_sheet = |h: &Harness<'_, App>| {
        let sheet = h.state().shell().properties().sheet().unwrap();
        assert_eq!(sheet.header.items, 1);
        assert_eq!(sheet.missing, 0);
        assert_eq!(sheet.sections.len(), 1);
        assert_eq!(sheet.sections[0].kind, "page_item");
        assert_eq!(sheet.sections[0].targets, [item.id]);
        let (_, row) = find_row(sheet, "page_item/kind.free_text.text").expect("the text row");
        assert!(matches!(&row.value, RowValue::One { text, .. } if text == "40x60"));
        for key in [key!("props-gone"), key!("props-gone-title")] {
            assert!(
                h.query_all_by_label(strings::text(key)).next().is_none(),
                "{key:?} is absent from the accessible UI"
            );
        }
    };
    assert_sheet(&h);
    let rev = h.state().shell().revision();
    let recorded = h.state().shell().recorded_commands().len();
    h.key_press(Key::F2);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "text_edit");
    assert!(h.state().shell().text_popup().is_some());
    assert_field(&h, key!("action-text-input"), "40x60");
    type_field(&mut h, key!("action-text-input"), "Cancelled", false);
    h.key_press(Key::Escape);
    settle(&mut h);
    settle(&mut h);
    assert!(h.state().shell().text_popup().is_none());
    assert_eq!(h.state().shell().revision(), rev);
    assert_eq!(h.state().shell().recorded_commands().len(), recorded);
    assert_eq!(
        h.state()
            .shell()
            .canvas()
            .unwrap()
            .page()
            .tables
            .items
            .iter()
            .find(|i| i.id == item.id)
            .unwrap()
            .text
            .as_deref(),
        Some("40x60")
    );
    assert_eq!(h.state().shell().selection().items[0].0, item.id);
    assert_sheet(&h);
}
