// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawn potential is given its type and voltage in the Potentials table (G-PP triage row 8, amending ID-752): on a
//! copy of Golden A whose 19 potential definitions are deleted (every potential is then known only at its connection
//! points, as on a freshly drawn folio), typing Type and Voltage of L, N, +24V and 0V creates each one's
//! `potential_def` at its CP in one undo step, the profile colours them BK, BU, DBU and WH, and the wire markers of
//! folios 101 and 102 read "2.5 BK BU" and "1.5 DBU DBU WH WH". The other definition cells of such a row stay
//! read-only, a gesture over two such rows is refused per cell, and a defined row's Voltage sets `env.ac_dc` too.

mod common;

use common::{Tmp, done};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

/// The column key of the Potentials column labelled `label`.
fn col(shell: &ShellModel, label: &str) -> String {
    let t = shell
        .nav(NavKind::Potentials)
        .and_then(|n| n.table())
        .expect("a table");
    t.columns
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no column {label}"))
        .key
        .clone()
}

/// The Potentials row named `name`: (key, record kind, the cell of the column labelled `label`).
fn row(shell: &ShellModel, name: &str, label: &str) -> (String, String, String) {
    let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
    let c = t.columns.iter().position(|c| c.label == label).unwrap();
    let n = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Potential && n.label == name)
        .unwrap_or_else(|| panic!("no potential {name}"));
    (
        n.key.clone(),
        n.kinds.first().cloned().unwrap_or_default(),
        n.cells[c].clone(),
    )
}

/// The name of the potential the page text prints at CP `cp` of `folio`.
fn potential_at(shell: &mut ShellModel, folio: &str, cp: &str) -> String {
    done(shell, "page.describe", json!({ "folio": folio }));
    let text = shell.take_page_text().expect("the page text");
    let head = format!("    cp \"{cp}\" ");
    let line = text
        .lines()
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("no cp {cp} on {folio}:\n{text}"));
    line.rsplit_once("potential \"")
        .map(|(_, n)| n.trim_end_matches('"').to_owned())
        .unwrap_or_else(|| panic!("no potential at {cp}: {line}"))
}

/// Types `text` into the cell (row named `name`, column labelled `label`) and commits it.
fn type_cell(shell: &mut ShellModel, name: &str, label: &str, text: &str) {
    let (key, _, _) = row(shell, name, label);
    let column = col(shell, label);
    done(
        shell,
        "table.edit",
        json!({"kind": "potentials", "row": key, "column": column}),
    );
    done(shell, "table.input", json!({"kind": "potentials", "text": text}));
    let out = shell.dispatch("table.commit", &json!({"kind": "potentials"}));
    assert_eq!(out, Outcome::Done, "{name} {label} {text}");
}

/// The texts of a wire marker placed over the wires of the page shown whose end labels are `ends` (in that order).
fn marker_text(shell: &mut ShellModel, folio: &str, ends: &[(&str, &str)]) -> String {
    done(shell, "page.goto", json!({ "folio": folio }));
    let page = shell.canvas().unwrap().page();
    let wires = ew_viewmodel::describe::page_wires(page);
    let ids: Vec<String> = ends
        .iter()
        .map(|(a, b)| {
            wires
                .iter()
                .find(|(_, x, y)| x == a && y == b)
                .unwrap_or_else(|| panic!("no wire {a} -> {b}"))
                .0
                .0
                .to_string()
        })
        .collect();
    let page_id = page.tables.page.to_string();
    done(
        shell,
        "command.run",
        json!({"cmd": "marker.place", "params": {"page": page_id, "connections": ids, "at": [60_000_000, 40_000_000]}}),
    );
    let tables = std::sync::Arc::clone(&shell.canvas().unwrap().page().tables);
    let texts: Vec<String> = tables
        .items
        .iter()
        .filter(|i| i.kind == "wire_marker")
        .filter_map(|i| i.text.clone())
        .collect();
    assert_eq!(texts.len(), 1, "{texts:?}");
    done(shell, "edit.undo", json!({}));
    texts[0].clone()
}

/// A writable copy of Golden A with every potential definition deleted (one step), the Potentials table on.
fn undefined(tmp: &Tmp) -> ShellModel {
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    done(&mut shell, "navigator.table", json!({"kind": "potentials"}));
    let defs: Vec<Value> = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .filter(|n| n.kinds == ["potential_def"])
            .flat_map(|n| n.records.iter())
            .map(|r| json!({"kind": "potential_def", "id": r.to_string()}))
            .collect()
    };
    assert!(defs.len() >= 4, "{defs:?}");
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": defs}}),
    );
    shell
}

const W4_W5: [(&str, &str); 2] = [("1X1:1/a", "S51:2"), ("1X1:2/a", "S51:4")];
const W20_TO_W23: [(&str, &str); 4] = [
    ("junction 1", "-102F1:1"),
    ("-102G1:+#2", "junction 1"),
    ("junction 2", "-102F2:GND#1"),
    ("junction 3", "-102F2:GND#2"),
];

/// Done criterion of row 8: L, N, 24 V and 0 V of folios drawn without definitions print BK, BU, DBU and WH by the
/// project's profile once Type and Voltage are typed in the Potentials table; each typed cell is one step, the first
/// a `potential.define` at the CP with the potential's name, the second a `fields.set_many` of the voltage and its
/// current; the markers read "2.5 BK BU" and "1.5 DBU DBU WH WH".
#[test]
fn typed_type_and_voltage_define_drawn_potentials_and_colour_their_wires() {
    let tmp = Tmp::new("potdef-colours");
    let mut shell = undefined(&tmp);
    let l = potential_at(&mut shell, "101", "S51:2");
    let n = potential_at(&mut shell, "101", "S51:4");
    let plus = potential_at(&mut shell, "102", "-102F1:1");
    let zero = potential_at(&mut shell, "102", "-102F2:GND#1");
    let before_101 = marker_text(&mut shell, "101", &W4_W5);
    let before_102 = marker_text(&mut shell, "102", &W20_TO_W23);
    assert_ne!(before_101, "2.5 BK BU", "without definitions N is not BU");
    assert_ne!(before_102, "1.5 DBU DBU WH WH");
    for name in [&l, &n, &plus, &zero] {
        assert_eq!(
            row(&shell, name, "Type").1,
            "cp",
            "{name} is known only at its CPs"
        );
    }

    // L and N: Type first (it defines the potential), then Voltage (the definition's voltage and current).
    for (name, ptype, voltage) in [(&l, "L", "230 V AC"), (&n, "N", "230 V AC")] {
        let steps = shell.recorded_commands().len();
        type_cell(&mut shell, name, "Type", ptype);
        assert_eq!(shell.recorded_commands().len(), steps + 1, "{name}: one step");
        let last = shell.recorded_commands().last().unwrap();
        assert_eq!(last.cmd, "potential.define", "{name}");
        assert_eq!(last.params["name"], name.as_str());
        assert!(last.params["at"]["cp"]["function"].is_string(), "{}", last.params);
        let (_, kind, shown) = row(&shell, name, "Type");
        assert_eq!(kind, "potential_def", "{name} is defined now");
        assert_eq!(shown, ptype, "{name}");

        type_cell(&mut shell, name, "Voltage", voltage);
        assert_eq!(shell.recorded_commands().len(), steps + 2, "{name}: one step");
        let last = shell.recorded_commands().last().unwrap();
        assert_eq!(last.cmd, "fields.set_many", "{name}: {}", last.params);
        assert_eq!(row(&shell, name, "Voltage").2, voltage, "{name}");
    }
    // +24V and 0V show the type of their supply's CP roles (L+, M): Voltage defines them; the same Type typed again on
    // the now defined row changes nothing.
    for (name, ptype, voltage) in [(&plus, "L+", "24 V DC"), (&zero, "M", "24 V DC")] {
        assert_eq!(row(&shell, name, "Type").2, ptype, "{name}: the role's type");
        let steps = shell.recorded_commands().len();
        type_cell(&mut shell, name, "Voltage", voltage);
        assert_eq!(shell.recorded_commands().len(), steps + 1, "{name}: one step");
        let last = shell.recorded_commands().last().unwrap();
        assert_eq!(last.cmd, "potential.define", "{name}");
        assert_eq!(last.params["voltage"], "24", "{}", last.params);
        assert_eq!(last.params["env"]["ac_dc"], "dc", "{}", last.params);
        assert_eq!(row(&shell, name, "Voltage").2, voltage, "{name}");
        type_cell(&mut shell, name, "Type", ptype);
        assert_eq!(
            shell.recorded_commands().len(),
            steps + 1,
            "{name}: nothing to change"
        );
    }
    for (name, colour) in [(&l, "BK"), (&n, "BU"), (&plus, "DBU"), (&zero, "WH")] {
        assert_eq!(row(&shell, name, "Colour").2, colour, "{name}");
    }
    assert_eq!(marker_text(&mut shell, "101", &W4_W5), "2.5 BK BU");
    assert_eq!(marker_text(&mut shell, "102", &W20_TO_W23), "1.5 DBU DBU WH WH");
    shell.shutdown();
}

/// Round 2's ruling: on an undefined row, typing the Type it already shows (from its CP roles) defines it, one
/// `potential.define` with that type; typed again on the now defined row it adds no step (the same-text rule).
#[test]
fn typing_the_shown_type_on_an_undefined_row_defines_it() {
    let tmp = Tmp::new("potdef-shown");
    let mut shell = undefined(&tmp);
    let plus = potential_at(&mut shell, "102", "-102F1:1");
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    let (_, kind, shown) = row(&shell, &plus, "Type");
    assert_eq!(
        (kind.as_str(), shown.as_str()),
        ("cp", "L+"),
        "the role's type on show"
    );
    let steps = shell.recorded_commands().len();
    type_cell(&mut shell, &plus, "Type", "L+");
    assert_eq!(shell.recorded_commands().len(), steps + 1, "one step");
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "potential.define");
    assert_eq!(last.params["ptype"], "l_plus", "{}", last.params);
    assert_eq!(row(&shell, &plus, "Type").1, "potential_def");
    type_cell(&mut shell, &plus, "Type", "L+");
    assert_eq!(
        shell.recorded_commands().len(),
        steps + 1,
        "a defined row: nothing to change"
    );
    shell.shutdown();
}

/// One gesture defines one potential in one step: Undo takes the definition back and the row is known only at its CP
/// again; Type and Voltage pasted together on the row define it with both and the current; the potential's other
/// definition cells stay read-only (ID-752); a paste over two undefined rows is refused per cell.
#[test]
fn one_step_one_row_and_the_cells_that_stay_read_only() {
    let tmp = Tmp::new("potdef-rules");
    let mut shell = undefined(&tmp);
    let l = potential_at(&mut shell, "101", "S51:2");
    let n = potential_at(&mut shell, "101", "S51:4");
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));

    type_cell(&mut shell, &l, "Type", "L");
    assert_eq!(row(&shell, &l, "Type").1, "potential_def");
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(row(&shell, &l, "Type").1, "cp", "Undo takes the definition back");

    // The name, class, colour and cross-section of an undefined potential stay read-only.
    let (key, _, _) = row(&shell, &l, "Type");
    for label in ["Potential", "Class", "Colour"] {
        let column = col(&shell, label);
        let m = failed(
            &mut shell,
            "table.edit",
            json!({"kind": "potentials", "row": key, "column": column}),
        );
        assert_eq!(m, format!("{label} is read-only here: no record of this kind."));
    }

    // Type and Voltage pasted together: one potential.define with both and the current.
    let ty = col(&shell, "Type");
    let steps = shell.recorded_commands().len();
    done(&mut shell, "table.cell", json!({"row": key, "column": ty}));
    done(&mut shell, "table.paste", json!({"text": "L\t230 V AC\n"}));
    assert_eq!(shell.recorded_commands().len(), steps + 1);
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "potential.define");
    assert_eq!(last.params["ptype"], "l");
    assert_eq!(last.params["voltage"], "230");
    assert_eq!(last.params["env"]["ac_dc"], "ac");
    assert_eq!(row(&shell, &l, "Voltage").2, "230 V AC");
    done(&mut shell, "edit.undo", json!({}));

    // Two undefined rows in one gesture: each defining cell is invalid, nothing commits.
    let (k1, _, _) = row(&shell, &l, "Type");
    let (k2, _, _) = row(&shell, &n, "Type");
    let voltage = col(&shell, "Voltage");
    let steps = shell.recorded_commands().len();
    done(
        &mut shell,
        "table.set",
        json!({"kind": "potentials", "rows": [k1, k2], "column": voltage, "value": "230 V AC"}),
    );
    let invalid = shell
        .describe()
        .lines()
        .filter(|l| {
            l.ends_with("invalid \"a potential without a definition is defined on its own row alone\"")
        })
        .count();
    assert_eq!(invalid, 2, "{}", shell.describe());
    assert_eq!(shell.recorded_commands().len(), steps, "nothing commits");
    failed(&mut shell, "preview.apply", json!({}));
    done(&mut shell, "preview.cancel", json!({}));
    assert_eq!(shell.recorded_commands().len(), steps);
    shell.shutdown();
}
