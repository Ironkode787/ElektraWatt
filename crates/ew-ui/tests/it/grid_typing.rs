// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Typing into the window's tables and Properties as a cabinet run did, at 1920 × 1080: the PLC grid's
//! Function text cell is typed in place as the station view's is, a cell that takes no text keeps the keys from the
//! canvas's shortcuts, and Tab in Properties takes the value and moves to the next field.

use crate::support::{Tmp, dispatch, on_sample, settle};

use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::properties::{RowValue, find_row, row_label, visible_rows};
use serde_json::json;

type Window = egui_kittest::Harness<'static, ew_ui::app::App>;

/// Types `text` as a keyboard does: each character its key press and its text.
fn type_keys(h: &mut Window, text: &str) {
    for c in text.chars() {
        let key = egui::Key::from_name(&c.to_ascii_uppercase().to_string());
        if let Some(key) = key {
            h.input_mut().events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.input_mut().events.push(egui::Event::Text(c.to_string()));
        h.run_ok();
    }
    settle(h);
}

/// The PLC grid's channel rows: (row number from 1, its Target cell, its Function text cell).
fn channel_rows(h: &Window) -> Vec<(usize, String, String)> {
    let shell = h.state().shell();
    let table = shell
        .nav(NavKind::PlcGrid)
        .and_then(|n| n.table())
        .expect("the PLC grid");
    let col = |key: &str| table.columns.iter().position(|c| c.key == key).expect(key);
    let (target, text) = (col("target"), col("function_text"));
    shell
        .shown_rows(NavKind::PlcGrid)
        .iter()
        .enumerate()
        .filter(|(_, r)| r.role == NavRole::Channel.word())
        .filter_map(|(i, r)| {
            let node = table.nodes.iter().find(|n| n.key == r.key)?;
            Some((i + 1, node.cells[target].clone(), node.cells[text].clone()))
        })
        .collect()
}

/// A click on the PLC grid's Function text cell of row `row`.
fn click_function_text(h: &mut Window, row: usize) {
    let at = h
        .get_all_by_role(egui::Role::Cell)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Function text") && l.contains(&format!(", row {row} of ")))
        })
        .expect("the cell")
        .rect()
        .center();
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(h);
}

/// The cabinet run (t0194): a click on a PLC grid Function text cell and typing. A wired channel's cell takes the
/// text, as the station view's does, and Enter gives it to the wired device; a channel nothing is wired to says why
/// in the station view's words, and no letter typed there reaches the canvas as a shortcut.
#[test]
fn the_plc_grids_function_text_is_typed_and_its_keys_stay_in_the_grid() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "102");
    dispatch(&mut h, "view.workspace_load", json!({"name": "PLC"}));
    settle(&mut h);
    let rows = channel_rows(&h);
    let wired = rows.iter().find(|r| !r.1.is_empty()).expect("a wired channel").0;
    let spare = rows
        .iter()
        .find(|r| r.1.is_empty())
        .expect("a channel nothing is wired to")
        .0;
    h.state_mut().shell_mut().set_dispatch_log(true);
    click_function_text(&mut h, wired);
    type_keys(&mut h, "Start button");
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    let text = channel_rows(&h)
        .into_iter()
        .find(|r| r.0 == wired)
        .expect("the row")
        .2;
    assert_eq!(text, "Start button", "the wired device's function text");
    click_function_text(&mut h, spare);
    type_keys(&mut h, "Start");
    let log = h.state_mut().shell_mut().take_dispatch_log();
    let leaked: Vec<&str> = log
        .iter()
        .map(|e| e.action.as_str())
        .filter(|a| {
            !["table.", "navigator.", "ui.", "plc.rack_channel", "props."]
                .iter()
                .any(|p| a.starts_with(p))
        })
        .collect();
    assert!(leaked.is_empty(), "keys that reached other shortcuts: {leaked:?}");
    let banner = h.state().shell().banner().unwrap_or_default().to_owned();
    assert!(
        banner.starts_with("Nothing is wired to this channel yet"),
        "{banner}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The text of Properties row `key` on show.
fn row_text(h: &Window, key: &str) -> String {
    let sheet = h.state().shell().properties().sheet().cloned().expect("a sheet");
    match find_row(&sheet, key).map(|(_, r)| r.value.clone()) {
        Some(RowValue::One { text, .. }) => text,
        _ => String::new(),
    }
}

/// The key of the Properties row labelled `label`.
fn row_key(h: &Window, label: &str) -> String {
    let shell = h.state().shell();
    let sheet = shell.properties().sheet().expect("a sheet");
    visible_rows(
        sheet,
        &shell.properties_collapsed(),
        shell.properties_show_advanced(),
        shell.properties_filter(),
    )
    .iter()
    .find(|r| row_label(r.section, r.row) == label)
    .map(|r| r.row.key.clone())
    .unwrap_or_else(|| panic!("the row {label}"))
}

/// The row whose editor is open.
fn editing(h: &Window) -> Option<String> {
    h.state().shell().properties().edit().map(|e| e.row.clone())
}

/// The cabinet run (t0004): in the project's Properties a title typed, Tab, the next text typed. Tab takes the value
/// and opens the next row's field, so each text lands in its own row; Shift+Tab goes back to the row before.
#[test]
fn tab_in_properties_takes_the_value_and_opens_the_next_field() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "102");
    dispatch(&mut h, "props.project", json!({}));
    settle(&mut h);
    settle(&mut h);
    let title = row_key(&h, "Document title");
    dispatch(&mut h, "props.edit", json!({ "row": title }));
    settle(&mut h);
    type_keys(&mut h, "Clamp station");
    h.key_press(egui::Key::Tab);
    settle(&mut h);
    let next = editing(&h).expect("the next row's field is open");
    assert_ne!(next, title, "Tab moved on");
    type_keys(&mut h, "Second");
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert_eq!(
        row_text(&h, &title),
        "Clamp station",
        "the first text kept its row"
    );
    assert_eq!(row_text(&h, &next), "Second", "the second its own");
    // Shift+Tab: back to the row before.
    dispatch(&mut h, "props.edit", json!({ "row": next }));
    settle(&mut h);
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
    settle(&mut h);
    assert_eq!(editing(&h).as_deref(), Some(title.as_str()), "Shift+Tab");
    h.state_mut().shell_mut().shutdown();
}
