// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP triage's row 8 in the window: in the Potentials table, a potential known only at its connection points
//! (its definition deleted) has an editable Type cell (its AccessKit name no longer ends ", read-only")
//! while its name stays read-only (ID-752); a double click on it opens the cell editor, and the typed type
//! defines the potential in one step.

mod support;

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
    let name = format!("{}, read-only", cell_name(shell, node, "Potential"));
    assert!(h.query_by_label(&ty).is_some(), "{ty} is editable");
    assert!(
        h.query_by_label(&format!("{ty}, read-only")).is_none(),
        "{ty} is not read-only"
    );
    assert!(h.query_by_label(&name).is_some(), "{name}");

    let at = h.get_by_label(&ty).rect().center();
    double_click(&mut h, at);
    let edit = h
        .state()
        .shell()
        .nav(NavKind::Potentials)
        .and_then(|n| n.table_mode())
        .and_then(|m| m.edit().cloned())
        .expect("the double click opens the cell editor");
    assert_eq!(edit.column, "kind");
    let steps = h.state().shell().recorded_commands().len();
    let shell = h.state_mut().shell_mut();
    done(
        shell,
        "table.input",
        json!({"kind": "potentials", "text": "signal"}),
    );
    done(shell, "table.commit", json!({"kind": "potentials"}));
    settle(&mut h);
    assert_eq!(h.state().shell().recorded_commands().len(), steps + 1);
    assert_eq!(
        h.state().shell().recorded_commands().last().unwrap().cmd,
        "potential.define"
    );
    h.state_mut().shell_mut().shutdown();
}
