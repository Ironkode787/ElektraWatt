// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 4 in the window, on a writable copy of Golden A, folio 902 (PS-7's kittest flows): Draw ▸ Layout's
//! Duct button arming Draw duct with its options card; the Create enclosure dialog with its two choice lists and its
//! refusal under the fields, and its Create disabled without a part; the Auto-fill rail dialog's line and its Fill
//! button; Rail 1's Properties with its free length. The drawing, the commits and the poses are the viewmodel's
//! `tests/it/layout_setup.rs`.

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_902(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": "902" }));
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

#[test]
fn the_duct_button_arms_draw_duct_with_its_options_card() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "view.command_tab", json!({ "tab": "draw" }));
    settle(&mut h);
    assert!(
        h.query_all_by_label("Grid step").next().is_none(),
        "no card before the tool runs"
    );
    h.get_by_role_and_label(egui::Role::Button, "Duct").click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "duct_draw");
    assert!(
        h.query_all_by_label("Grid step").next().is_some(),
        "the grid's tab list"
    );
    assert!(h.query_all_by_label("Part").next().is_some(), "the part's label");
    // Golden A's duct part gives no size, so the card asks for it (C44-p4#3).
    assert!(
        h.query_all_by_label("Width (mm)").next().is_some(),
        "the width's label"
    );
    assert!(
        h.query_all_by_label("Height (mm)").next().is_some(),
        "the height's label"
    );
    h.get_by_role_and_label(egui::Role::Tab, "1 mm").click();
    settle(&mut h);
    assert_eq!(h.state().shell().layout_options().grid_mm, 1);
    dispatch(&mut h, "ui.dismiss", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    assert!(
        h.query_all_by_label("Grid step").next().is_none(),
        "the card goes with the tool"
    );
}

/// ID-1139 (W45.2 F2): Draw duct armed after Draw rail sizes its own card: Golden A's duct part gives no size, so the
/// card shows Width and Height, each at least its desired width (48 px), and Height reads its value whole (60).
#[test]
fn the_duct_card_after_the_rail_card_keeps_its_fields_width() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "tool.rail_draw", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "rail_draw");
    dispatch(&mut h, "tool.duct_draw", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "duct_draw");
    let fields: Vec<(String, String, f32)> = ["Width (mm)", "Height (mm)"]
        .into_iter()
        .map(|label| {
            let field = h
                .get_all_by_role(egui::Role::TextInput)
                .find(|n| n.accesskit_node().label().as_deref() == Some(label))
                .unwrap_or_else(|| panic!("the field {label}"));
            (
                label.to_owned(),
                field.accesskit_node().value().unwrap_or_default().to_owned(),
                field.rect().width(),
            )
        })
        .collect();
    for (label, _, width) in &fields {
        assert!(*width >= 48.0, "{label} is {width} px wide");
    }
    assert_eq!(fields[1].1, "60");
}

#[test]
fn the_create_enclosure_dialog_keeps_its_refusal_under_the_fields() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "layout.enclosure_new", json!({}));
    settle(&mut h);
    assert!(
        h.query_all_by_label("Create enclosure").next().is_some(),
        "the title"
    );
    assert!(h.query_all_by_label("Location").next().is_some());
    assert!(h.query_all_by_label("Enclosure part").next().is_some());
    assert!(
        h.query_all_by_label_contains("+J-E-2416").next().is_some(),
        "the page's location chosen"
    );
    h.get_by_label("Create").click();
    settle(&mut h);
    let message = match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => f.error.clone().expect("the refusal"),
        other => panic!("{other:?}"),
    };
    assert!(
        h.query_all_by_label(&message).next().is_some(),
        "under the fields: {message}"
    );
}

/// W44.2 F1: with its part emptied, the dialog's Create is disabled; a click on it sends nothing and leaves no
/// refusal under the fields.
#[test]
fn create_is_disabled_without_a_part() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "layout.enclosure_new", json!({}));
    settle(&mut h);
    h.state_mut().shell_mut().set_form_field("part", "");
    settle(&mut h);
    let rev = h.state().shell().revision();
    h.get_by_label("Create").click();
    settle(&mut h);
    match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => assert_eq!(f.error, None, "nothing sent"),
        other => panic!("{other:?}"),
    }
    assert_eq!(h.state().shell().revision(), rev);
}

#[test]
fn the_auto_fill_dialog_says_which_devices_it_fills_with() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "layout.auto_fill_rail", json!({}));
    settle(&mut h);
    assert!(
        h.query_all_by_label("Auto-fill rail").next().is_some(),
        "the title"
    );
    assert!(
        h.query_all_by_label("Fills with every unplaced device that fits the rail.")
            .next()
            .is_some(),
        "the devices' line"
    );
    assert!(
        h.query_all_by_label("Fill").next().is_some(),
        "the primary button"
    );
    assert!(h.query_all_by_label("Order").next().is_some());
}

#[test]
fn rail_1s_properties_show_its_free_length() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    let r1 = h
        .state()
        .shell()
        .panel_tables()
        .unwrap()
        .carriers
        .iter()
        .find(|c| c.label == "Rail 1")
        .unwrap()
        .id;
    dispatch(&mut h, "select.set", json!({ "ids": [r1.to_string()] }));
    settle(&mut h);
    assert!(h.query_all_by_label("Free length").next().is_some());
    assert!(h.query_all_by_label("341 mm").next().is_some());
}

/// The placement of device `tag` on 902.
fn placement_of(h: &Harness<'_, App>, tag: &str) -> String {
    h.state()
        .shell()
        .panel_tables()
        .unwrap()
        .placements
        .iter()
        .find(|p| p.tag == tag)
        .unwrap_or_else(|| panic!("no placement {tag}"))
        .id
        .to_string()
}

/// The text of the focused text field.
fn focused_field(h: &Harness<'_, App>) -> (String, egui::Rect) {
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.is_focused())
        .unwrap_or_else(|| {
            panic!(
                "the focused field: {:?}",
                h.get_all_by_role(egui::Role::TextInput)
                    .map(|n| (
                        n.accesskit_node().label(),
                        n.accesskit_node().value(),
                        n.is_focused()
                    ))
                    .collect::<Vec<_>>()
            )
        });
    (
        field.accesskit_node().value().unwrap_or_default().to_owned(),
        field.rect(),
    )
}

/// W44.4 F-h: -102F1's Realised part edited, cancelled, then the same row of -201U1's placement edited: the editor
/// shows -201U1's own part, not -102F1's.
#[test]
fn the_editor_of_another_selection_shows_its_own_value() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    let row = json!({"row": "placement/derived.realised_part"});
    let f1 = placement_of(&h, "-102F1");
    dispatch(&mut h, "select.set", json!({"ids": [f1]}));
    settle(&mut h);
    dispatch(&mut h, "props.edit", row.clone());
    settle(&mut h);
    assert_eq!(focused_field(&h).0, "A9F78116");
    dispatch(&mut h, "props.cancel", json!({}));
    settle(&mut h);
    let u1 = placement_of(&h, "-201U1");
    dispatch(&mut h, "select.set", json!({"ids": [u1]}));
    settle(&mut h);
    dispatch(&mut h, "props.edit", row);
    settle(&mut h);
    let buffer = h
        .state()
        .shell()
        .properties()
        .edit()
        .expect("an editor")
        .buffer
        .clone();
    assert_eq!(
        (focused_field(&h).0, buffer.as_str()),
        ("195740".to_owned(), "195740")
    );
}

/// C44-p6#4 (W44.4 F-h, the frames' case): with -102F1's Realised part editor focused, `select.set` on -201U1's
/// placement and `props.edit` on the same row with no frame between: the field keeps its focus across the change,
/// and still shows -201U1's own part.
#[test]
fn the_focused_editor_takes_the_new_selections_value() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    let row = json!({"row": "placement/derived.realised_part"});
    let f1 = placement_of(&h, "-102F1");
    let u1 = placement_of(&h, "-201U1");
    dispatch(&mut h, "select.set", json!({"ids": [f1]}));
    settle(&mut h);
    dispatch(&mut h, "props.edit", row.clone());
    settle(&mut h);
    assert_eq!(focused_field(&h).0, "A9F78116");
    dispatch(&mut h, "select.set", json!({"ids": [u1]}));
    dispatch(&mut h, "props.edit", row);
    settle(&mut h);
    let buffer = h
        .state()
        .shell()
        .properties()
        .edit()
        .expect("an editor")
        .buffer
        .clone();
    assert_eq!(
        (focused_field(&h).0, buffer.as_str()),
        ("195740".to_owned(), "195740")
    );
}

/// W44.4 F-f (doc 16a §6.8): the editor's text starts `space.icon` (6 px) inside the field.
#[test]
fn the_editors_text_is_padded_from_the_fields_edge() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "select.set", json!({"ids": []}));
    settle(&mut h);
    dispatch(&mut h, "props.edit", json!({"row": "page/derived.surface"}));
    settle(&mut h);
    let (text, rect) = focused_field(&h);
    assert_eq!(text, "-UC1 plate");
    let at = painted_texts(&h)
        .into_iter()
        .find(|(t, p)| t == "-UC1 plate" && rect.expand(1.0).contains(*p))
        .map(|(_, p)| p)
        .unwrap_or_else(|| {
            panic!(
                "the field's text in {rect:?}: {:?}",
                painted_texts(&h)
                    .into_iter()
                    .filter(|(t, _)| t.contains("UC1"))
                    .collect::<Vec<_>>()
            )
        });
    assert!((at.x - rect.left() - 6.0).abs() < 1.0, "{at:?} in {rect:?}");
}

/// W44.4 F-d: a panel page's Surface, Label mode and Datum rows sit under their own heading, "Surface view".
#[test]
fn the_surface_view_rows_have_their_own_heading() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "select.set", json!({"ids": []}));
    settle(&mut h);
    let headings: Vec<String> = h
        .get_all_by_role(egui::Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l == "Surface view" || l == "Placement")
        .collect();
    // Placement keeps the page's own placement fields (its scale, its form); the view's rows follow under theirs.
    assert_eq!(headings, ["Placement", "Surface view"]);
}

/// W44.4 F-e: F2 (`props.edit`) on a row below the pane's fold scrolls the row into view, so its editor shows inside
/// the pane.
#[test]
fn an_editor_opened_out_of_view_scrolls_into_view() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-102F1"]}));
    settle(&mut h);
    let pane_name = ew_ui::ew_viewmodel::strings::text(ew_ui::ew_viewmodel::key!("pane-properties"));
    let pane = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some(pane_name))
        .expect("the Properties pane")
        .rect();
    let last = h
        .state()
        .shell()
        .properties()
        .sheet()
        .unwrap()
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .rev()
        .find(|r| {
            r.actions
                .contains(&ew_ui::ew_viewmodel::ew_api::output::RowAction::Edit)
                && !matches!(r.ty.inner(), ew_ui::ew_viewmodel::properties::FieldType::Bool)
        })
        .map(|r| r.key.clone())
        .expect("an editable row");
    let row_name = {
        let sheet = h.state().shell().properties().sheet().unwrap();
        let (section, row) = ew_ui::ew_viewmodel::properties::find_row(sheet, &last).unwrap();
        ew_ui::ew_viewmodel::properties::row_label(section, row)
    };
    let before = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some(row_name.as_str()))
        .map(|n| n.rect())
        .expect("the row");
    assert!(
        before.top() >= pane.bottom(),
        "{last} starts below the fold: {before:?} {pane:?}"
    );
    dispatch(&mut h, "props.edit", json!({ "row": last }));
    settle(&mut h);
    let (_, field) = focused_field(&h);
    assert!(
        pane.contains_rect(field),
        "{last}: the editor {field:?} inside the pane {pane:?}"
    );
}

/// The texts the last frame painted, with where each starts.
fn painted_texts(h: &Harness<'_, App>) -> Vec<(String, egui::Pos2)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.pos)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// FT-1284 (ID-1315 ruling 4, doc 16a §6.12): Create enclosure with its Create disabled at open (its part emptied
/// before the first frame) and no text field gives its first focus to its first enabled control, the location list.
#[test]
fn create_enclosure_disabled_at_open_focuses_its_first_control() {
    let tmp = Tmp::new();
    let mut h = on_902(&tmp);
    dispatch(&mut h, "layout.enclosure_new", json!({}));
    h.state_mut().shell_mut().set_form_field("part", "");
    settle(&mut h);
    let Some(Dialog::Form(form)) = h.state().shell().dialog().cloned() else {
        panic!("{:?}", h.state().shell().dialog());
    };
    assert!(
        form.fields.iter().all(|f| !f.is_text()),
        "two lists, no text field"
    );
    let spec = ew_ui::ew_viewmodel::dialogs::spec(&Dialog::Form(form), h.state().shell());
    assert_eq!(
        spec.primary.disabled.as_deref(),
        Some("Type or choose a part number")
    );
    let focused: Vec<(egui::Role, Option<String>)> = h
        .root()
        .children_recursive()
        .filter(|n| n.accesskit_node().is_focused())
        .map(|n| (n.accesskit_node().role(), n.accesskit_node().label()))
        .collect();
    assert_eq!(focused, [(egui::Role::ComboBox, Some("Location".to_owned()))]);
}
