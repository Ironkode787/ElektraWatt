// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP triage's row 8 in the window, as amended by WP-S1.16 D4 (FT-577): in the Potentials table, a potential
//! known only at its connection points (its definition deleted) has editable Name and Type cells (their AccessKit
//! names do not end ", read-only"); a double click on the Name opens the cell editor, and the typed name defines the
//! potential in one step.

use crate::support;

use egui::{Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::{Outcome, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The four pointer events of a double click at `at`, all in one frame (as `tests/data_grid.rs` gives them).
fn double_click(h: &mut Harness<'static, App>, at: egui::Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    for pressed in [true, false, true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    settle(h);
}

/// The accessible name of the cell (row `node`, column labelled `label`) of the Potentials table on show.
fn cell_name(shell: &ShellModel, node: u32, label: &str) -> String {
    let n = shell.nav(NavKind::Potentials).unwrap();
    let t = n.table().unwrap();
    let c = t.columns.iter().position(|c| c.label == label).unwrap();
    let rows = n.visible();
    let at = rows
        .iter()
        .position(|r| r.node == node)
        .expect("the row is on show")
        + 1;
    let text = &t.nodes[node as usize].cells[c];
    let head = if text.is_empty() {
        label.to_owned()
    } else {
        format!("{label} {text}")
    };
    format!("{head}, row {at} of {}", rows.len())
}

#[test]
fn the_type_cell_of_an_undefined_potential_is_editable() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h: Harness<'static, App> = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    done(shell, "navigator.show", json!({"kind": "potentials"}));
    done(shell, "navigator.table", json!({"kind": "potentials"}));
    // FT-515's situation: 24VDC-4's definition deleted, the potential is known only at its CPs.
    let def = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .find(|n| n.role == NavRole::Potential && n.label == "24VDC-4")
            .map(|n| n.records[0])
            .expect("24VDC-4")
    };
    done(
        shell,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "potential_def", "id": def.to_string()}]}}),
    );
    let (node, label) = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        let i = t
            .nodes
            .iter()
            .position(|n| n.table_row && n.kinds == ["cp"])
            .expect("a potential known only at a CP");
        (u32::try_from(i).unwrap(), t.nodes[i].label.clone())
    };
    done(
        shell,
        "navigator.filter",
        json!({"kind": "potentials", "text": label}),
    );
    done(
        shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "potentials"}),
    );
    settle(&mut h);

    // The pane shows the key column and Type; Voltage is right of the pane (the table scrolls there).
    let shell = h.state().shell();
    let ty = cell_name(shell, node, "Type");
    let name = cell_name(shell, node, "Potential");
    for cell in [&ty, &name] {
        assert!(h.query_by_label(cell).is_some(), "{cell} is editable");
        assert!(
            h.query_by_label(&format!("{cell}, read-only")).is_none(),
            "{cell} is not read-only"
        );
    }

    let at = h.get_by_label(&name).rect().center();
    double_click(&mut h, at);
    let edit = h
        .state()
        .shell()
        .nav(NavKind::Potentials)
        .and_then(|n| n.table_mode())
        .and_then(|m| m.edit().cloned())
        .expect("the double click opens the cell editor");
    assert_eq!(edit.column, "potential");
    let steps = h.state().shell().recorded_commands().len();
    let shell = h.state_mut().shell_mut();
    done(
        shell,
        "table.input",
        json!({"kind": "potentials", "text": "24VDC-9"}),
    );
    done(shell, "table.commit", json!({"kind": "potentials"}));
    settle(&mut h);
    assert_eq!(h.state().shell().recorded_commands().len(), steps + 1);
    let last = h.state().shell().recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "potential.define");
    assert_eq!(last.params["name"], "24VDC-9", "{}", last.params);
    h.state_mut().shell_mut().shutdown();
}

/// FT-1239's row with a context menu, in the window `h` on a copy of Golden A: the Potentials table, 24VDC-4's
/// definition deleted, filtered to the first row with a point to define its potential at; the accessible name of its
/// Potential cell and the point (the census opens its menu too, FT-1424).
pub(crate) fn a_row_to_define(
    h: &mut Harness<'static, App>,
) -> (String, (impl std::fmt::Display + use<>, u16)) {
    let shell = h.state_mut().shell_mut();
    done(shell, "navigator.show", json!({"kind": "potentials"}));
    done(shell, "navigator.table", json!({"kind": "potentials"}));
    let def = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .find(|n| n.role == NavRole::Potential && n.label == "24VDC-4")
            .map(|n| n.records[0])
            .expect("24VDC-4")
    };
    done(
        shell,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "potential_def", "id": def.to_string()}]}}),
    );
    let (node, label, at) = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        let i = t
            .nodes
            .iter()
            .position(|n| n.table_row && n.define_at.is_some())
            .expect("a potential with a point to define it at");
        (
            u32::try_from(i).unwrap(),
            t.nodes[i].label.clone(),
            t.nodes[i].define_at.unwrap(),
        )
    };
    done(
        shell,
        "navigator.filter",
        json!({"kind": "potentials", "text": label}),
    );
    settle(h);
    (cell_name(h.state().shell(), node, "Potential"), at)
}

/// FT-1239: a right click on a Potentials row with a definition point opens its context menu with Define potential…;
/// it opens the form, which defines the row's potential at the row's point.
#[test]
fn a_potentials_rows_context_menu_defines_its_potential() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h: Harness<'static, App> = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    let (name, at) = a_row_to_define(&mut h);
    h.get_by_label(&name).click_secondary();
    settle(&mut h);
    h.get_by_label("Define potential…").click();
    settle(&mut h);
    let form = h
        .state()
        .shell()
        .palette()
        .and_then(|p| p.form.clone())
        .expect("the form");
    assert_eq!(form.action, "potential.draw");
    let steps = h.state().shell().recorded_commands().len();
    let shell = h.state_mut().shell_mut();
    shell.set_palette_field(0, "FEED-W");
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    assert_eq!(shell.recorded_commands().len(), steps + 1);
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "potential.define");
    assert_eq!(
        last.params["at"],
        json!({"cp": {"function": at.0.to_string(), "idx": at.1}})
    );
    shell.shutdown();
}

/// ID-1278 ruling 8 (W49.1 M): Define potential… whose voltage does not read ("abc") shows its failure under the
/// palette form's fields only, never as a banner as well; corrected and run again, it defines the potential with no
/// banner either, and the form with its error is gone.
#[test]
fn a_failed_define_stays_in_its_form_and_no_banner_follows() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h: Harness<'static, App> = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    // Golden A's definitions deleted, as the model's test of Define potential… does: its wires define anew.
    done(shell, "navigator.show", json!({"kind": "potentials"}));
    done(shell, "navigator.table", json!({"kind": "potentials"}));
    let defs: Vec<Value> = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .filter(|n| n.kinds == ["potential_def"])
            .flat_map(|n| n.records.iter())
            .map(|r| json!({"kind": "potential_def", "id": r.to_string()}))
            .collect()
    };
    done(
        shell,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": defs}}),
    );
    done(shell, "page.goto", json!({"folio": "102"}));
    let wire = shell.canvas().unwrap().page().tables.connections[0]
        .id
        .to_string();
    done(shell, "select.set", json!({"ids": [wire]}));
    h.state_mut().dispatch_from_window("potential.draw", &json!({}));
    settle(&mut h);
    let error = |h: &Harness<'static, App>| {
        h.state()
            .shell()
            .palette()
            .and_then(|p| p.form.as_ref())
            .and_then(|f| f.error.clone())
    };
    let shell = h.state_mut().shell_mut();
    shell.set_palette_field(0, "FEED-A");
    shell.set_palette_field(2, "abc");
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert_eq!(
        error(&h).as_deref(),
        Some("\"abc\" is not a voltage; type it as 24 V DC, 230 V AC or 24.")
    );
    assert_eq!(h.state().shell().banner(), None, "the form shows it");
    h.state_mut().shell_mut().set_palette_field(2, "24 V DC");
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert!(h.state().shell().palette().is_none(), "the form closes");
    assert_eq!(error(&h), None);
    assert_eq!(h.state().shell().banner(), None);
    let last = h.state().shell().recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "potential.define");
    // Run with its parameters, no form open: the failure is a banner, and the action's next success clears it.
    let wire = h.state().shell().canvas().unwrap().page().tables.connections[1]
        .id
        .to_string();
    done(h.state_mut().shell_mut(), "select.set", json!({"ids": [wire]}));
    let bad = json!({"name": "FEED-B", "voltage": "abc"});
    h.state_mut().dispatch_from_window("potential.draw", &bad);
    settle(&mut h);
    assert_eq!(
        h.state().shell().banner(),
        Some("\"abc\" is not a voltage; type it as 24 V DC, 230 V AC or 24.")
    );
    // FT-1260: reopening its form is no success: the banner stays, the form open over it.
    h.state_mut().dispatch_from_window("potential.draw", &json!({}));
    settle(&mut h);
    assert!(
        h.state().shell().palette().is_some_and(|p| p.form.is_some()),
        "the form is open"
    );
    assert_eq!(
        h.state().shell().banner(),
        Some("\"abc\" is not a voltage; type it as 24 V DC, 230 V AC or 24."),
        "opening the form keeps the banner"
    );
    h.key_press(egui::Key::Escape);
    settle(&mut h);
    let good = json!({"name": "FEED-B", "voltage": "24 V DC"});
    h.state_mut().dispatch_from_window("potential.draw", &good);
    settle(&mut h);
    assert_eq!(h.state().shell().banner(), None, "its success clears it");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1205 (ID-1392): Escape in a context menu closes the menu and nothing else: the selection stays, as Escape in the
/// app menu does (FT-1130).
#[test]
fn escape_in_a_context_menu_keeps_the_selection() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h: Harness<'static, App> = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    done(shell, "navigator.show", json!({"kind": "potentials"}));
    done(shell, "navigator.table", json!({"kind": "potentials"}));
    // As above: 24VDC-4's definition deleted, so its row offers Define potential….
    let def = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .find(|n| n.role == NavRole::Potential && n.label == "24VDC-4")
            .map(|n| n.records[0])
            .expect("24VDC-4")
    };
    done(
        shell,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "potential_def", "id": def.to_string()}]}}),
    );
    let (node, label) = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        let i = t
            .nodes
            .iter()
            .position(|n| n.table_row && n.define_at.is_some())
            .expect("a potential with a point to define it at");
        (u32::try_from(i).unwrap(), t.nodes[i].label.clone())
    };
    done(
        shell,
        "navigator.filter",
        json!({"kind": "potentials", "text": label}),
    );
    done(shell, "select.set", json!({"refs": ["-102F1"]}));
    settle(&mut h);
    let name = cell_name(h.state().shell(), node, "Potential");
    h.get_by_label(&name).click_secondary();
    settle(&mut h);
    assert!(
        h.query_by_label("Define potential…").is_some(),
        "the context menu"
    );
    let selected = h.state().shell().selection().items.clone();
    assert!(!selected.is_empty());
    h.key_press(egui::Key::Escape);
    settle(&mut h);
    assert!(h.query_by_label("Define potential…").is_none(), "the menu closed");
    assert_eq!(
        h.state().shell().selection().items,
        selected,
        "the selection stays"
    );
    h.state_mut().shell_mut().shutdown();
}
