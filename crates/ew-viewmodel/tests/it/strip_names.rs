// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 33 (G-PP findings 7 and 9, draft FT row 12) on copies of Golden A: the Terminal strips navigator takes a row by
//! what it prints (a terminal's tag `-X1031:1`, which is its strip's label, a colon and its own label, or a label that
//! only one row on show has), in `table.set` and `navigator.select`; and `place.pick` with a strip name the project
//! does not have fails with a message that lists the project's strips.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

/// The column key of the column labelled `label` in the Terminal strips table.
fn col(shell: &ShellModel, label: &str) -> String {
    let t = shell
        .nav(NavKind::Strips)
        .and_then(|n| n.table())
        .expect("a table");
    t.columns
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no column {label}"))
        .key
        .clone()
}

/// The internal key of -X1031's terminal `n`: the node labelled `n` whose parent is labelled `-X1031`.
fn x1031(shell: &ShellModel, n: &str) -> String {
    let t = shell
        .nav(NavKind::Strips)
        .and_then(|n| n.table())
        .expect("a table");
    t.nodes
        .iter()
        .find(|node| node.label == n && node.parent.is_some_and(|p| t.nodes[p as usize].label == "-X1031"))
        .unwrap_or_else(|| panic!("no -X1031:{n}"))
        .key
        .clone()
}

/// A headless writable copy of Golden A on 103 with the Terminal strips navigator open.
fn strips(tmp: &Tmp, table: bool) -> ShellModel {
    let mut shell = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    if table {
        done(
            &mut shell,
            "navigator.table",
            json!({"kind": "strips", "on": true}),
        );
    }
    shell
}

fn failed(shell: &mut ShellModel, action: &str, params: serde_json::Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

#[test]
fn table_set_takes_a_terminal_row_by_its_tag() {
    let tmp = Tmp::new("k33-strip-keys");
    let mut shell = strips(&tmp, true);
    let function_text = col(&shell, "Function text");
    let rows = [x1031(&shell, "1"), x1031(&shell, "2")];
    let steps = shell.recorded_commands().len();
    done(
        &mut shell,
        "table.set",
        json!({"kind": "strips", "column": function_text, "rows": ["-X1031:1", "-X1031:2"], "value": "K33"}),
    );
    assert_eq!(shell.recorded_commands().len(), steps + 1, "one committed step");
    for row in &rows {
        assert_eq!(
            shell.table_cell_text(NavKind::Strips, row, &function_text),
            "K33",
            "{row}"
        );
    }
    // A tag no row has, and a label several rows have (every strip has a terminal 1), name no row.
    for given in ["-X9999:1", "1"] {
        let message = failed(
            &mut shell,
            "table.set",
            json!({"kind": "strips", "column": function_text, "rows": [given], "value": "x"}),
        );
        assert_eq!(
            message,
            format!("No row {given} is on show in the strips navigator.")
        );
    }
    // The internal key still works.
    done(
        &mut shell,
        "table.set",
        json!({"kind": "strips", "column": function_text, "rows": [rows[0]], "value": "K33b"}),
    );
    assert_eq!(
        shell.table_cell_text(NavKind::Strips, &rows[0], &function_text),
        "K33b"
    );
}

#[test]
fn navigator_select_takes_a_row_by_its_tag_and_a_strip_by_its_label() {
    let tmp = Tmp::new("k33-strip-select");
    let mut shell = strips(&tmp, false);
    done(
        &mut shell,
        "navigator.select",
        json!({"kind": "strips", "keys": ["-X1031:3", "-X1032"]}),
    );
    let marked = shell.nav(NavKind::Strips).expect("the navigator").marked();
    assert!(marked.contains(&x1031(&shell, "3")), "{marked:?}");
    assert_eq!(marked.len(), 2, "{marked:?}");
}

#[test]
fn place_pick_with_an_unknown_strip_lists_the_strips() {
    let tmp = Tmp::new("k33-strip-unknown");
    let mut shell = tmp.shell_in(Mode::Headless, true, "103");
    let message = failed(
        &mut shell,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X??", "designation": "2"}),
    );
    assert!(
        message.starts_with("No terminal strip -X?? in the project; its strips are "),
        "{message}"
    );
    for strip in ["-X1031", "-X1032", "1X1"] {
        assert!(message.contains(strip), "{strip} in {message}");
    }
    assert!(shell.picker().is_none(), "nothing is armed");
    // A strip the project has still arms the Place tool.
    done(
        &mut shell,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031"}),
    );
}
