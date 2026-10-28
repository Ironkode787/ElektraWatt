// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid in the window without rendering (WP-S2.05 part 1, acceptance checks PG5, PG9 and the window half
//! of PG11): AC-28's W29 step (W29 chosen through `select.set`, the chips, the colour through the editor, undo), D-06's property-grid
//! entry from the keyboard alone (override the description of folio 101, see the finding, reset, undo), the exact
//! counts of the window host, and the accessible-name walker after every step.

mod support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::properties::{Provenance, RowValue, find_row};
use ew_ui::ew_viewmodel::shell::{Focus, Outcome};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

/// W29 on folio 103 (`connections|W29`).
const W29: &str = "9eb2d865-fca4-59b1-87bf-c073d7190ceb";

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

/// The value text and provenance of row `key` of the grid's sheet.
fn row(h: &Harness<'_, App>, key: &str) -> (String, Provenance) {
    let sheet = h.state().shell().properties().sheet().expect("a sheet");
    let (_, r) = find_row(sheet, key).unwrap_or_else(|| panic!("no row {key}"));
    let text = match &r.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    };
    (text, r.provenance.clone())
}

/// Moves the grid's row cursor with the arrow keys until it is on `key` (bounded).
fn row_to(h: &mut Harness<'_, App>, key: &str) {
    press(h, Key::Home);
    for _ in 0..200 {
        if h.state().shell().properties_row().as_deref() == Some(key) {
            return;
        }
        press(h, Key::ArrowDown);
    }
    panic!("the row cursor did not reach {key}");
}

/// Types `text` into the grid's open field (select all first).
fn type_into_field(h: &mut Harness<'_, App>, label: &str, text: &str) {
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    settle(h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some(label))
        .unwrap_or_else(|| panic!("the field {label}"));
    field.type_text(text);
    settle(h);
}

#[test]
fn pg5_ac28_w29_colour_through_the_editor_and_undo() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    // W29 through `select.set` (the canvas picks by geometry since WP-S2.04 part 1, and the centre of a wire's
    // rectangle need not lie on the wire).
    dispatch(&mut h, "select.set", json!({"ids": [W29]}));
    settle(&mut h);
    let selected: Vec<String> = h
        .state()
        .shell()
        .selection()
        .items
        .iter()
        .map(|e| e.0.to_string())
        .collect();
    assert_eq!(selected, [W29]);
    // The chips of PG2.
    assert_eq!(
        row(&h, "connection/derived.csa"),
        (
            "0.75 mm²".to_owned(),
            Provenance::Derived {
                via: "potential 24VDC-1 · size profile '0.75/1.5' · class control".into()
            }
        )
    );
    assert_eq!(
        row(&h, "connection/derived.colour"),
        (
            "DBU".to_owned(),
            Provenance::Derived {
                via: "potential 24VDC-1 · colour profile 'DBU/WH 24 V'".into()
            }
        )
    );
    // The row's group names it with its value and chip.
    assert!(
        h.get_all_by_role(egui::Role::Group)
            .any(|n| n.accesskit_node().label().as_deref() == Some("Colour")),
        "the colour row is a named group"
    );
    // E on the canvas focuses the grid; the colour row; Enter opens its override editor; BU; Enter commits.
    press(&mut h, Key::E);
    assert_eq!(h.state().shell().focus(), Some(Focus::Properties));
    row_to(&mut h, "connection/derived.colour");
    press(&mut h, Key::Enter);
    assert!(
        h.state().shell().properties().edit().is_some(),
        "the editor is open"
    );
    // The colour editor: a combo of the IEC codes and the project's company codes, filtered by the typed text; Down
    // highlights the first, Enter chooses it.
    assert!(
        h.get_all_by_role(egui::Role::ListBox).next().is_some(),
        "the colour editor lists its options"
    );
    type_into_field(&mut h, "Colour", "BU");
    let options: Vec<String> = h
        .get_all_by_role(egui::Role::ListBoxOption)
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert_eq!(options.first().map(String::as_str), Some("BU"), "{options:?}");
    assert!(options.iter().all(|o| o.contains("BU")), "{options:?}");
    press(&mut h, Key::ArrowDown);
    press(&mut h, Key::Enter);
    assert_eq!(
        row(&h, "connection/derived.colour"),
        (
            "BU".to_owned(),
            Provenance::Overridden {
                note: "override (CDP)".into()
            }
        )
    );
    // Undo: derived DBU again.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    let (text, prov) = row(&h, "connection/derived.colour");
    assert_eq!(text, "DBU");
    assert!(matches!(prov, Provenance::Derived { .. }), "{prov:?}");
}

#[test]
fn pg9_d06_the_description_override_from_the_keyboard() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    // An empty selection shows the page on show.
    let sheet = h.state().shell().properties().sheet().expect("a sheet");
    assert_eq!(sheet.sections[0].kind, "page");
    assert_eq!(
        row(&h, "page/derived.description"),
        (
            "Expansion Control Cabinets".to_owned(),
            Provenance::Inherited {
                from: "project description".into()
            }
        )
    );
    // F6 to the grid, the description row, Enter (Override), the new text, Enter.
    while h.state().shell().focus() != Some(Focus::Properties) {
        press(&mut h, Key::F6);
    }
    row_to(&mut h, "page/derived.description");
    press(&mut h, Key::Enter);
    type_into_field(&mut h, "Description", "Control Cabinet Expansion");
    press(&mut h, Key::Enter);
    assert_eq!(
        row(&h, "page/derived.description"),
        (
            "Control Cabinet Expansion".to_owned(),
            Provenance::Overridden {
                note: "override · project description is 'Expansion Control Cabinets'".into()
            }
        )
    );
    let findings = &h.state().shell().properties().sheet().unwrap().header.findings;
    assert!(
        findings
            .iter()
            .any(|f| f.rule == "EW-CORE-023" && f.severity == "info"),
        "{findings:?}"
    );
    assert!(
        h.query_all_by_label_contains("EW-CORE-023").next().is_some(),
        "the header shows the finding"
    );
    // Reset through the row's actions (Shift+F10, Down to `reset`, Enter): the override and the finding go.
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    settle(&mut h);
    let actions: Vec<String> = {
        let sheet = h.state().shell().properties().sheet().unwrap();
        find_row(sheet, "page/derived.description")
            .unwrap()
            .1
            .actions
            .iter()
            .map(|a| a.word().to_owned())
            .collect()
    };
    let reset = actions
        .iter()
        .position(|a| a == "reset")
        .expect("reset is offered");
    for _ in 0..reset {
        press(&mut h, Key::ArrowDown);
    }
    press(&mut h, Key::Enter);
    let (text, prov) = row(&h, "page/derived.description");
    assert_eq!(text, "Expansion Control Cabinets");
    assert!(matches!(prov, Provenance::Inherited { .. }), "{prov:?}");
    assert!(
        !h.state()
            .shell()
            .properties()
            .sheet()
            .unwrap()
            .header
            .findings
            .iter()
            .any(|f| f.rule == "EW-CORE-023")
    );
    // Each one undo step: undo brings the override back, undo again the project's description.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    assert_eq!(row(&h, "page/derived.description").0, "Control Cabinet Expansion");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    assert_eq!(
        row(&h, "page/derived.description").0,
        "Expansion Control Cabinets"
    );
}

#[test]
fn pg11_the_window_fetches_a_sheet_once_per_key() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let calls = |h: &Harness<'_, App>| h.state().shell().session_calls();
    let n = calls(&h).property_sheet;
    // Frames without a change: no sheet.
    for _ in 0..5 {
        h.run_ok();
    }
    assert_eq!(calls(&h).property_sheet, n);
    // A selection change: one.
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-101F1"]}));
    settle(&mut h);
    assert_eq!(calls(&h).property_sheet, n + 1);
    // Row moves and category toggles from the keyboard: none.
    dispatch(&mut h, "props.focus", json!({}));
    settle(&mut h);
    press(&mut h, Key::ArrowDown);
    press(&mut h, Key::ArrowDown);
    press(&mut h, Key::ArrowLeft);
    press(&mut h, Key::ArrowRight);
    assert_eq!(calls(&h).property_sheet, n + 1);
    // A direct commit in the window: one preview, one commit, then one sheet.
    let before = calls(&h);
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/function_text", "value": "24 V distribution"}),
    );
    settle(&mut h);
    let after = calls(&h);
    assert_eq!(after.preview_with, before.preview_with + 1);
    assert_eq!(after.commit, before.commit + 1);
    assert_eq!(after.run_script, before.run_script);
    assert_eq!(after.execute_with, before.execute_with);
    assert_eq!(after.property_sheet, before.property_sheet + 1);
    // The toast of a multi-target commit leaves after the window's 8 seconds, never the model's.
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/function_text", "value": "Spare"}),
    );
    settle(&mut h);
    assert!(h.state().shell().toast().is_some());
    assert!(support::off_chrome(&h, "Undo").is_some(), "the toast's Undo");
}

/// The checkbox of a boolean row (§6.5) from the keyboard: Space on the focused row sets the other value; undo
/// restores it.
#[test]
fn a_boolean_row_is_a_checkbox() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-X1031:1"]}));
    settle(&mut h);
    assert!(
        h.get_all_by_role(egui::Role::CheckBox)
            .any(|n| n.accesskit_node().label().as_deref() == Some("Rail bonded")),
        "the boolean row is a named checkbox"
    );
    dispatch(&mut h, "props.focus", json!({}));
    settle(&mut h);
    row_to(&mut h, "device/terminal.rail_bonded");
    let before = row(&h, "device/terminal.rail_bonded").0;
    press(&mut h, Key::Space);
    let after = row(&h, "device/terminal.rail_bonded").0;
    assert_ne!(before, after);
    assert_eq!(after, if before == "yes" { "no" } else { "yes" });
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    settle(&mut h);
    assert_eq!(row(&h, "device/terminal.rail_bonded").0, before);
}

/// An option of a choice editor picked with the mouse (§6.5): the click on `BU` in the colour list chooses it,
/// although the field loses the focus to the click.
#[test]
fn a_choice_editor_option_is_picked_with_the_mouse() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    // WP-S2.17 (J32): at 1280 by 800 Properties is the lower half of the left column and the colour row sits in its
    // lower half, so the option list opens over the row, inside the window and above the status bar.
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(&mut h, "select.set", json!({ "ids": [W29] }));
    settle(&mut h);
    dispatch(&mut h, "props.focus", json!({}));
    settle(&mut h);
    row_to(&mut h, "connection/derived.colour");
    press(&mut h, Key::Enter);
    // "B" alone is no colour: a commit of the field instead of the click would fail.
    assert_options_inside_the_window(&h);
    type_into_field(&mut h, "Colour", "B");
    assert_options_inside_the_window(&h);
    let before = h.state().shell().recorded_commands().len();
    h.get_all_by_role(egui::Role::ListBoxOption)
        .find(|n| n.accesskit_node().label().as_deref() == Some("BU"))
        .expect("the option BU")
        .click();
    settle(&mut h);
    settle(&mut h);
    assert_eq!(
        h.state().shell().recorded_commands().len(),
        before + 1,
        "one commit: {:?}",
        h.state().shell().properties().error()
    );
    assert_eq!(
        row(&h, "connection/derived.colour"),
        (
            "BU".to_owned(),
            Provenance::Overridden {
                note: "override (CDP)".into()
            }
        )
    );
}

/// The option list of the open choice editor lies inside the window and above the status bar, and every option the
/// list shows (its rectangle meets the list's) lies inside the list: no option is cut off under the status bar.
fn assert_options_inside_the_window(h: &Harness<'_, App>) {
    let window = h.ctx.content_rect();
    // The status bar's top: `size.status` above the frame's bottom (the harness frames the app with 8 px).
    let tokens = ew_ui::theme::current(&h.ctx);
    let status_top = window.shrink(8.0).bottom() - tokens.size_status;
    let ready = h.get_by_role_and_label(egui::Role::Label, "Ready").rect();
    assert!(ready.top() >= status_top - 0.5, "{ready:?} under {status_top}");
    let list = h
        .get_all_by_role(egui::Role::ListBox)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Options of Colour"))
        .expect("the colour options")
        .rect();
    assert!(window.contains_rect(list), "{list:?} in {window:?}");
    assert!(list.bottom() <= status_top + 0.5, "{list:?} above {status_top}");
    let options: Vec<egui::Rect> = h
        .get_all_by_role(egui::Role::ListBoxOption)
        .map(|n| n.rect())
        .filter(|r| r.intersect(list).height() > 0.5)
        .collect();
    assert!(!options.is_empty(), "the list shows options");
    for r in &options {
        assert!(list.expand(0.5).contains_rect(*r), "{r:?} inside {list:?}");
        assert!(
            window.contains_rect(*r) && r.bottom() <= status_top + 0.5,
            "{r:?}"
        );
    }
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

/// J32 (doc 16a §6.8, the mockup's `.prow` and `.psec`): on folio 101's page sheet at 1280 by 800 every property
/// label is painted at the left edge of its 96-px label column (not centred), the group headings read as written
/// ("Main", not "MAIN"), the inherited Description shows its one-word chip ("inherited", no glyph) in its row, and
/// no painted text carries the old glyph chips ("^ inherited", "= authored", "f derived").
#[test]
fn j32_property_rows_are_left_aligned_with_one_word_chips() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    let tokens = ew_ui::theme::current(&h.ctx);
    let texts = painted_texts(&h);
    for label in ["Number", "Section", "Description", "Title"] {
        let column = h
            .get_all_by_role(egui::Role::Button)
            .find(|n| {
                n.accesskit_node().label().as_deref() == Some(label)
                    && (n.rect().width() - tokens.size_property_label).abs() < 0.5
            })
            .unwrap_or_else(|| panic!("the label {label}"))
            .rect();
        let (_, at) = texts
            .iter()
            .find(|(t, p)| t == label && column.expand(1.0).contains(*p))
            .unwrap_or_else(|| panic!("the painted label {label} in {column:?}"));
        assert!(
            (at.x - column.left()).abs() < 0.5,
            "{label} starts at {} in the column {column:?}",
            at.x
        );
    }
    let painted: Vec<&str> = texts.iter().map(|(t, _)| t.as_str()).collect();
    assert!(painted.contains(&"Main"), "{painted:?}");
    assert!(!painted.contains(&"MAIN"), "{painted:?}");
    let description = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Description"))
        .expect("the Description row")
        .rect();
    assert!(
        texts
            .iter()
            .any(|(t, p)| t == "inherited" && description.contains(*p)),
        "the one-word chip in {description:?}"
    );
    assert!(
        !painted
            .iter()
            .any(|t| t.starts_with("^ ") || t.starts_with("= ") || t.starts_with("f ")),
        "{painted:?}"
    );
}
