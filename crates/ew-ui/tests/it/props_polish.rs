// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Properties polish in the window (kittest, no rendering), on a writable copy of Golden A: the
//! Device row's link with its tooltip, its click and F; the editor's text centred; no popup for a
//! choice list with no match; the list under the value cell; Home and End scrolling the cursor row
//! into view; a pre-filled form field replaced by what is typed.

use crate::support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Dialog, Focus};
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle};

/// F as a keyboard gives it: the key and the typed letter, in one frame.
fn type_f(h: &mut Harness<'_, App>) {
    h.key_press(Key::F);
    h.event(egui::Event::Text("f".to_owned()));
    settle(h);
}

/// Selects the placement of device `tag` on 902.
fn select_placement(h: &mut Harness<'_, App>, tag: &str) {
    let id = h
        .state()
        .shell()
        .panel_tables()
        .unwrap()
        .placements
        .iter()
        .find(|p| p.tag == tag)
        .unwrap_or_else(|| panic!("no placement {tag}"))
        .id
        .to_string();
    dispatch(h, "select.set", json!({ "ids": [id] }));
    settle(h);
}

/// The focused text field's rectangle.
fn focused_field(h: &Harness<'_, App>) -> egui::Rect {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.is_focused())
        .expect("a focused field")
        .rect()
}

/// The texts the last frame painted, each with its galley's rectangle.
fn painted_texts(h: &Harness<'_, App>) -> Vec<(String, egui::Rect)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_owned(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// The rectangle of the painted text `text` inside `inside`.
fn text_in(h: &Harness<'_, App>, text: &str, inside: egui::Rect) -> egui::Rect {
    painted_texts(h)
        .into_iter()
        .find(|(t, r)| t == text && inside.expand(1.0).contains_rect(*r))
        .map(|(_, r)| r)
        .unwrap_or_else(|| panic!("no text {text} in {inside:?}"))
}

/// "Properties' Device row is a link in the accent colour that a click or F ... follows to the
/// device on its main page, its tooltip the canvas link's hint". The row is a link node; hovered, its tooltip is
/// "Click or F: go to -102F1"; a click shows 102 with -102F1 selected.
#[test]
fn the_device_row_is_a_link_that_a_click_follows() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "902");
    select_placement(&mut h, "-102F1");
    let link = h.get_by_role_and_label(egui::Role::Link, "-102F1");
    link.hover();
    h.run_steps(40);
    assert!(
        h.query_by_label("Click or F: go to -102F1").is_some(),
        "the link's tooltip"
    );
    h.get_by_role_and_label(egui::Role::Link, "-102F1").click();
    settle(&mut h);
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(shell.current_folio(), Some("102"));
    assert_eq!(shell.properties().sheet().unwrap().header.title, "-102F1");
}

/// "... a click or F (`props.follow`, in the properties context of every keymap ...; typed, not
/// followed, while an editor field has the focus)". F on another row starts its edit with the letter; F in an open
/// field is typed; F on the Device row follows it and types nothing.
#[test]
fn f_follows_the_device_row_and_is_typed_elsewhere() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "902");
    select_placement(&mut h, "-102F1");
    dispatch(&mut h, "props.focus", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().focus(), Some(Focus::Properties));
    // On the Position row (no link) F starts the row's edit with the letter.
    dispatch(&mut h, "props.row", json!({"to": "placement/derived.position"}));
    settle(&mut h);
    type_f(&mut h);
    let edit = h.state().shell().properties().edit().cloned().expect("an edit");
    assert_eq!(
        (edit.row.as_str(), edit.buffer.as_str()),
        ("placement/derived.position", "f")
    );
    assert_eq!(h.state().shell().current_folio(), Some("902"));
    h.key_press(Key::Escape);
    settle(&mut h);
    // In the open field F is typed, not followed.
    dispatch(&mut h, "props.edit", json!({"row": "placement/derived.position"}));
    settle(&mut h);
    settle(&mut h);
    type_f(&mut h);
    let edit = h.state().shell().properties().edit().cloned().expect("an edit");
    assert_eq!(edit.buffer, "f");
    assert_eq!(h.state().shell().current_folio(), Some("902"));
    h.key_press(Key::Escape);
    settle(&mut h);
    // On the Device row F follows the link and types nothing.
    dispatch(&mut h, "props.focus", json!({}));
    dispatch(&mut h, "props.row", json!({"to": "placement/derived.device"}));
    settle(&mut h);
    type_f(&mut h);
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(shell.current_folio(), Some("102"));
    assert_eq!(shell.properties().sheet().unwrap().header.title, "-102F1");
    assert!(
        shell.properties().edit().is_none(),
        "no edit started: {:?}",
        shell.properties().edit()
    );
}

/// "the Properties editor's text is centred in its field as the value's is". The Scale row's text
/// centre, as a value and in its editor, within 1 px.
#[test]
fn the_editors_text_is_centred_as_the_values() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "902");
    dispatch(&mut h, "select.set", json!({"ids": []}));
    settle(&mut h);
    let cell = h
        .get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("1:10"))
        .expect("the Scale value")
        .rect();
    let value = text_in(&h, "1:10", cell);
    dispatch(&mut h, "props.edit", json!({"row": "page/scale"}));
    settle(&mut h);
    let field = focused_field(&h);
    let edited = text_in(&h, "1:10", field);
    assert!(
        (edited.center().y - field.center().y - (value.center().y - cell.center().y)).abs() <= 1.0,
        "the editor's {edited:?} in {field:?}, the value's {value:?} in {cell:?}"
    );
}

/// The options list box of the open choice editor, if any.
fn options(h: &Harness<'_, App>) -> Option<egui::Rect> {
    h.query_all_by_role(egui::Role::ListBox).next().map(|n| n.rect())
}

/// "a choice list with no match opens no popup". Label mode's list shows for "Ab" and is gone for
/// "zzz".
#[test]
fn a_choice_list_with_no_match_opens_no_popup() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "902");
    dispatch(&mut h, "select.set", json!({"ids": []}));
    settle(&mut h);
    dispatch(&mut h, "props.edit", json!({"row": "page/derived.label_mode"}));
    settle(&mut h);
    assert!(options(&h).is_some(), "the list on opening");
    dispatch(&mut h, "props.input", json!({"text": "Ab"}));
    settle(&mut h);
    assert!(
        options(&h).is_some(),
        "the list for a match: {:?} {:?}",
        h.state().shell().properties().choices(),
        h.state().shell().properties().edit()
    );
    dispatch(&mut h, "props.input", json!({"text": "zzz"}));
    settle(&mut h);
    assert_eq!(options(&h), None, "no popup for no match");
}

/// "the list starts under the value cell". At 1920 × 1080 and at 2560 × 1440 the list's frame starts at the field's
/// left edge (the list box inside it by the frame's padding) and stays inside the window.
#[test]
fn the_choice_list_starts_under_the_value_cell() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "902");
    for size in [(1920.0, 1080.0), (2560.0, 1440.0)] {
        h.set_size(egui::Vec2::new(size.0, size.1));
        settle(&mut h);
        dispatch(&mut h, "select.set", json!({"ids": []}));
        settle(&mut h);
        dispatch(&mut h, "props.edit", json!({"row": "page/derived.label_mode"}));
        settle(&mut h);
        settle(&mut h);
        let field = focused_field(&h);
        let list = options(&h).expect("the options");
        let tokens = ew_ui::theme::current(&h.ctx);
        let pad = tokens.space_2 + tokens.border_width;
        assert!(
            (list.left() - pad - field.left()).abs() <= 0.5,
            "{list:?} under {field:?}"
        );
        assert!(h.ctx.content_rect().contains_rect(list), "{list:?} at {size:?}");
        dispatch(&mut h, "props.cancel", json!({}));
        settle(&mut h);
    }
}

/// "Home and End scroll the cursor row into view". At 1920 × 1080 -102F1's rows run past the
/// pane's fold: End shows the last row, Home the first.
#[test]
fn home_and_end_scroll_the_cursor_row_into_view() {
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "902");
    h.set_size(egui::Vec2::new(1920.0, 1080.0));
    settle(&mut h);
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-102F1"]}));
    settle(&mut h);
    dispatch(&mut h, "props.focus", json!({}));
    settle(&mut h);
    let pane_name = ew_ui::ew_viewmodel::strings::text(ew_ui::ew_viewmodel::key!("pane-properties"));
    let pane = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some(pane_name))
        .expect("the Properties pane")
        .rect();
    let cursor_row = |h: &Harness<'_, App>| {
        let shell = h.state().shell();
        let key = shell.properties_row().expect("a cursor row");
        let sheet = shell.properties().sheet().unwrap();
        let (section, row) = ew_ui::ew_viewmodel::properties::find_row(sheet, &key).unwrap();
        let name = ew_ui::ew_viewmodel::properties::row_label(section, row);
        h.get_all_by_role(egui::Role::Group)
            .find(|n| n.accesskit_node().label().as_deref() == Some(name.as_str()))
            .map(|n| n.rect())
            .unwrap_or_else(|| panic!("the row {name}"))
    };
    h.key_press(Key::End);
    settle(&mut h);
    settle(&mut h);
    let last = cursor_row(&h);
    assert!(
        last.top() >= pane.top() && last.bottom() <= pane.bottom(),
        "End: {last:?} in {pane:?}"
    );
    h.key_press(Key::Home);
    settle(&mut h);
    settle(&mut h);
    let first = cursor_row(&h);
    assert!(
        first.top() >= pane.top() && first.bottom() <= pane.bottom(),
        "Home: {first:?} in {pane:?}"
    );
}

/// "a pre-filled form field selects its text when it takes the focus, as the Properties editor
/// does". Page ▸ New page's folio holds the proposal 4; typing 999 over it, without clearing it, gives 999.
#[test]
fn typing_over_a_pre_filled_field_replaces_it() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    support::open_menu(&mut h, &["Page"]);
    settle(&mut h);
    h.get_by_label("New page…").click();
    settle(&mut h);
    let folio = || match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => f.fields[0].value.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(folio(), "4");
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Folio"))
        .expect("the Folio field")
        .type_text("999");
    settle(&mut h);
    let folio = match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => f.fields[0].value.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(folio, "999");
}

/// A configuration project's Address profile row reads the chosen profile's name, never its
/// reference; hovered, its tooltip is the reference, the chooser's secondary line.
#[test]
fn the_address_profile_row_shows_the_name_its_reference_in_the_tooltip() {
    // Golden A's configuration project PLC1.
    const PLC1: &str = "4a1d3ee5-9e34-5065-b364-09f61b6e7c30";
    let tmp = Tmp::new();
    let mut h = support::on_golden(&tmp, "902");
    dispatch(&mut h, "navigator.show", json!({"kind": "plc"}));
    dispatch(
        &mut h,
        "navigator.select",
        json!({"kind": "plc", "keys": [format!("plc_config_project:{PLC1}")]}),
    );
    settle(&mut h);
    settle(&mut h);
    assert!(h.query_by_label("siemens-tia-intl@2027.1").is_none());
    let value = h.get_by_role_and_label(egui::Role::Button, "Siemens TIA (international)");
    value.hover();
    h.run_steps(40);
    assert!(
        h.query_by_label("siemens-tia-intl@2027.1").is_some(),
        "the row's tooltip"
    );
}
