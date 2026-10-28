// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Table mode of WP-S2.06 part 3 on copies of Golden A (acceptance checks TB1's reasons through the shell, TB2 to TB6,
//! and TB8's model half): the Terminal strips table filtered `x1031` and sorted by Terminal, a cell edit, fill down
//! (one `fields.set` with 5 targets, the toast, one undo step), pattern fill (`Spare_{n:02}` over the 12 spare
//! terminals, one `fields.set_many`), paste from Excel (header and position mapping, quoting, an ambiguous number
//! resolved on the card, read-only and past-the-end cells, same-record conflicts), the policy, copy and a paste back that
//! changes nothing, and the describe lines, words and counts.

mod common;

use common::{Tmp, done, refused};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// The column key of the column labelled `label` in navigator `kind`'s table.
fn col(shell: &ShellModel, kind: NavKind, label: &str) -> String {
    let t = shell.nav(kind).and_then(|n| n.table()).expect("a table");
    t.columns
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no column {label}"))
        .key
        .clone()
}

/// The rows on show of navigator `kind` in display order: (key, the cell of the column labelled `label`).
fn column(shell: &ShellModel, kind: NavKind, label: &str) -> Vec<(String, String)> {
    let n = shell.nav(kind).unwrap();
    let t = n.table().unwrap();
    let c = t.columns.iter().position(|c| c.label == label).unwrap();
    n.visible()
        .iter()
        .map(|r| {
            let node = &t.nodes[r.node as usize];
            (node.key.clone(), node.cells[c].clone())
        })
        .collect()
}

/// The texts of the column labelled `label` on show.
fn texts(shell: &ShellModel, kind: NavKind, label: &str) -> Vec<String> {
    column(shell, kind, label).into_iter().map(|(_, t)| t).collect()
}

/// The key of the Terminal strips row whose Terminal is `n`.
fn terminal(shell: &ShellModel, n: &str) -> String {
    column(shell, NavKind::Strips, "Terminal")
        .into_iter()
        .find(|(_, t)| t == n)
        .unwrap_or_else(|| panic!("no terminal {n}"))
        .0
}

/// The Function text of terminal `n`.
fn label_of(shell: &ShellModel, n: &str) -> String {
    let k = terminal(shell, n);
    shell.table_cell_text(NavKind::Strips, &k, &col(shell, NavKind::Strips, "Function text"))
}

/// A writable copy of Golden A, headless, with the Terminal strips table filtered `x1031` and sorted by Terminal.
fn strips(tmp: &Tmp, filter: &str) -> ShellModel {
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "strips", "on": true}),
    );
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "strips", "text": filter}),
    );
    let by = col(&shell, NavKind::Strips, "Terminal");
    done(&mut shell, "navigator.sort", json!({"kind": "strips", "by": by}));
    shell
}

/// Marks the rows `keys` of navigator `kind`.
fn mark(shell: &mut ShellModel, kind: NavKind, keys: &[String]) {
    done(
        shell,
        "navigator.select",
        json!({"kind": kind.word(), "keys": keys}),
    );
}

/// The committed steps so far.
fn steps(shell: &ShellModel) -> usize {
    shell.recorded_commands().len()
}

/// The last committed step: (command, parameters).
fn last(shell: &ShellModel) -> (String, Value) {
    let s = shell.recorded_commands().last().expect("a committed step");
    (s.cmd.clone(), s.params.clone())
}

fn lines_with(shell: &ShellModel, prefix: &str) -> Vec<String> {
    shell
        .describe()
        .lines()
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

#[track_caller]
fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action}: {other:?}"),
    }
}

/// TB2 (FR-UI-008, §8.4): Terminal strips in table mode filtered `x1031` and sorted by Terminal list 1, 2, … 10, …
/// 20; terminal 13's function text set to "Spare" by one cell edit (its own undo step); filling it down over terminals
/// 13 to 17 is one `fields.set` with the 5 terminals, a direct commit with the toast, one preview and one commit
/// (headless: one `run_script`), and one undo step that restores terminals 14 to 17.
#[test]
fn tb2_fill_down_over_five_terminals_is_one_fields_set_and_one_undo_step() {
    let tmp = Tmp::new("tb2");
    let mut shell = strips(&tmp, "x1031");
    let order: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
    assert_eq!(texts(&shell, NavKind::Strips, "Terminal"), order);
    let before: Vec<String> = ["13", "14", "15", "16", "17"]
        .iter()
        .map(|n| label_of(&shell, n))
        .collect();
    let fn_col = col(&shell, NavKind::Strips, "Function text");
    // One cell edit: F2, type, Enter.
    let k13 = terminal(&shell, "13");
    done(&mut shell, "table.cell", json!({"row": k13, "column": fn_col}));
    let calls = shell.session_calls().navigator_table;
    done(&mut shell, "table.edit", json!({}));
    assert_eq!(
        lines_with(&shell, "  editing "),
        [format!("  editing \"Function text\" buffer \"{}\"", before[0])]
    );
    done(&mut shell, "table.input", json!({"text": "Spare"}));
    assert_eq!(
        shell.session_calls().navigator_table,
        calls,
        "typing makes no table call"
    );
    let n = steps(&shell);
    done(&mut shell, "table.commit", json!({}));
    assert_eq!(steps(&shell), n + 1);
    assert_eq!(label_of(&shell, "13"), "Spare");
    assert!(lines_with(&shell, "  editing ").is_empty());
    // Fill down over 13 to 17.
    let keys: Vec<String> = ["13", "14", "15", "16", "17"]
        .iter()
        .map(|n| terminal(&shell, n))
        .collect();
    mark(&mut shell, NavKind::Strips, &keys);
    let calls = shell.session_calls();
    done(&mut shell, "table.fill_down", json!({}));
    let after = shell.session_calls();
    assert_eq!(after.preview_with, calls.preview_with + 1, "one preview");
    assert_eq!(after.run_script, calls.run_script + 1, "one commit (headless)");
    assert_eq!(steps(&shell), n + 2, "one step");
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "fields.set");
    assert_eq!(params["targets"].as_array().unwrap().len(), 5);
    assert_eq!(params["path"], "function_text");
    for t in ["13", "14", "15", "16", "17"] {
        assert_eq!(label_of(&shell, t), "Spare", "terminal {t}");
    }
    assert!(shell.properties().card().is_none(), "a direct commit");
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Set Function text on 5 terminals\" undo"]
    );
    // One undo step restores 14 to 17.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(label_of(&shell, "13"), "Spare");
    for (i, t) in ["14", "15", "16", "17"].iter().enumerate() {
        assert_eq!(label_of(&shell, t), before[i + 1], "terminal {t}");
    }
    shell.shutdown();
}

/// TB3 (§8.5): `Spare_{n:02}` over -X1031's 12 spare terminals gives `Spare_01` … `Spare_12` in one
/// `fields.set_many`; the dialog shows the first five values; every pattern error refuses the whole fill, a counter
/// that overflows or goes negative makes those cells invalid (the card).
#[test]
fn tb3_pattern_fill_over_the_twelve_spare_terminals_is_one_fields_set_many() {
    let tmp = Tmp::new("tb3");
    let mut shell = strips(&tmp, "x1031");
    let spare: Vec<String> = column(&shell, NavKind::Strips, "State")
        .into_iter()
        .filter(|(_, s)| s == "spare")
        .map(|(k, _)| k)
        .collect();
    assert_eq!(spare.len(), 12);
    mark(&mut shell, NavKind::Strips, &spare);
    let fn_col = col(&shell, NavKind::Strips, "Function text");
    done(&mut shell, "table.cell", json!({"column": fn_col}));
    // The dialog: Ctrl+Shift+D, the pattern, the preview of the first five.
    done(&mut shell, "table.pattern_fill", json!({}));
    done(&mut shell, "table.input", json!({"text": "Spare_{n:02}"}));
    assert_eq!(
        lines_with(&shell, "  pattern "),
        [
            "  pattern \"Spare_{n:02}\" values \"Spare_01\" \"Spare_02\" \"Spare_03\" \"Spare_04\" \"Spare_05\""
        ]
    );
    // Errors of the whole pattern.
    for (p, needle) in [
        ("Spare_{n", "unclosed brace"),
        ("{x}", "unknown expression {x}"),
        ("{col:nope}", "unknown column nope"),
        ("a}b", "closes nothing"),
    ] {
        let m = failed(&mut shell, "table.pattern_fill", json!({"pattern": p}));
        assert!(m.contains(needle), "{p}: {m}");
    }
    assert!(shell.properties().card().is_none());
    let n = steps(&shell);
    done(&mut shell, "table.commit", json!({}));
    assert_eq!(steps(&shell), n + 1);
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "fields.set_many");
    assert_eq!(params["edits"].as_array().unwrap().len(), 12);
    let filled: Vec<String> = spare
        .iter()
        .map(|k| shell.table_cell_text(NavKind::Strips, k, &fn_col))
        .collect();
    let expected: Vec<String> = (1..=12).map(|i| format!("Spare_{i:02}")).collect();
    assert_eq!(filled, expected);
    assert!(lines_with(&shell, "  pattern ").is_empty(), "the dialog closed");
    // `{{` and `}}` are literal braces; `{prev}` continues the row above.
    done(&mut shell, "table.pattern_fill", json!({"pattern": "{{{n}}}"}));
    assert_eq!(shell.table_cell_text(NavKind::Strips, &spare[1], &fn_col), "{2}");
    // A counter that overflows on row 2 and one that is negative on rows 1 and 2: invalid cells on the card.
    let m = n + 2;
    done(
        &mut shell,
        "table.pattern_fill",
        json!({"pattern": "{n*4611686018427387904}"}),
    );
    assert_eq!(steps(&shell), m, "nothing committed");
    let cells = lines_with(&shell, "  cell ");
    assert_eq!(cells.len(), 11, "{cells:?}");
    assert!(
        cells[0].ends_with(" invalid \"the counter overflows\""),
        "{cells:?}"
    );
    done(&mut shell, "preview.cancel", json!({}));
    done(&mut shell, "table.pattern_fill", json!({"pattern": "{n-3}"}));
    let cells = lines_with(&shell, "  cell ");
    assert_eq!(cells.len(), 2, "{cells:?}");
    assert!(cells[0].starts_with("  cell 1 "), "{cells:?}");
    assert!(
        cells[1].ends_with(" invalid \"the counter is negative\""),
        "{cells:?}"
    );
    shell.shutdown();
}

/// TB4 (§8.6, §8.7, D13): paste with a header row in another column order maps by label (an unmatched pasted column is
/// ignored); without one by position from the cursor; quoted cells with TAB and line breaks; `0,75` and `1,500` (the
/// latter ambiguous: the card, `table.resolve`, Apply); a read-only column and a row past the end are invalid; two rows
/// on one record with different values are invalid ("same record as row <n>"); Apply is refused until resolved or
/// dropped; a paste is one command and one undo step.
#[test]
fn tb4_paste_maps_parses_and_refuses_as_specified() {
    let tmp = Tmp::new("tb4");
    let mut shell = strips(&tmp, "x1031");
    let hide: Vec<String> = [
        "Group",
        "Part",
        "Potential",
        "Side 1 (clamp a)",
        "Side 2 (clamp b)",
        "Bridge",
    ]
    .iter()
    .map(|l| col(&shell, NavKind::Strips, l))
    .collect();
    done(
        &mut shell,
        "table.columns",
        json!({"kind": "strips", "hide": hide}),
    );
    assert_eq!(
        lines_with(&shell, "  columns "),
        ["  columns \"Strip\" \"Terminal\" \"Function text\" \"State\""]
    );
    // A header in another order: State, an unknown column, Function text.
    let k15 = terminal(&shell, "15");
    done(&mut shell, "table.cell", json!({"row": k15}));
    let n = steps(&shell);
    let calls = shell.session_calls();
    done(
        &mut shell,
        "table.paste",
        json!({"text": "State\tComment\tFunction text\r\nspare\tx\tA\r\nspare\ty\tB\r\n"}),
    );
    assert_eq!(shell.session_calls().preview_with, calls.preview_with + 1);
    assert_eq!(steps(&shell), n + 1, "one command");
    assert_eq!(last(&shell).0, "fields.set_many");
    assert_eq!(
        (label_of(&shell, "15"), label_of(&shell, "16")),
        ("A".into(), "B".into())
    );
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(label_of(&shell, "15"), "2-", "one undo step");
    assert_eq!(label_of(&shell, "16"), "2-");
    // No header: by position from the cursor column rightwards (Function text, then State).
    let fn_col = col(&shell, NavKind::Strips, "Function text");
    let k5 = terminal(&shell, "5");
    done(&mut shell, "table.cell", json!({"row": k5, "column": fn_col}));
    done(
        &mut shell,
        "table.paste",
        json!({"text": "P1\tspare\nP2\tspare\n"}),
    );
    assert_eq!(
        (label_of(&shell, "5"), label_of(&shell, "6")),
        ("P1".into(), "P2".into())
    );
    // Quoted cells with a TAB and a line break stay one cell each (the model refuses control characters in a text, so
    // they are invalid on the card); doubled quotes are one quote.
    let k7 = terminal(&shell, "7");
    done(&mut shell, "table.cell", json!({"row": k7, "column": fn_col}));
    done(
        &mut shell,
        "table.paste",
        json!({"text": "\"a\tb\"\r\n\"line\nbreak\"\r\n"}),
    );
    let reason = "a text has no line breaks or control characters";
    assert_eq!(
        lines_with(&shell, "  cell "),
        [
            format!("  cell 1 \"7\" \"Function text\" invalid \"{reason}\""),
            format!("  cell 2 \"8\" \"Function text\" invalid \"{reason}\""),
        ]
    );
    done(&mut shell, "preview.cancel", json!({}));
    done(
        &mut shell,
        "table.paste",
        json!({"text": "\"say \"\"hi\"\"\"\tspare\r\n"}),
    );
    assert_eq!(label_of(&shell, "7"), "say \"hi\"");
    // A read-only column and a row past the end: invalid; Apply refused; dropping both ends the gesture.
    done(
        &mut shell,
        "table.columns",
        json!({"kind": "strips", "show": [hide[1]]}),
    );
    let k20 = terminal(&shell, "20");
    done(&mut shell, "table.cell", json!({"row": k20, "column": hide[1]}));
    let n = steps(&shell);
    done(&mut shell, "table.paste", json!({"text": "X\r\nY\r\n"}));
    assert_eq!(
        lines_with(&shell, "  cell "),
        [
            "  cell 1 \"20\" \"Part\" invalid \"no command yet\"".to_owned(),
            "  cell 2 \"\" \"Part\" invalid \"no row\"".to_owned(),
        ]
    );
    let m = failed(&mut shell, "navigator.apply", json!({}));
    assert_eq!(m, "2 cells are invalid or ambiguous; resolve or drop them first.");
    done(&mut shell, "table.drop", json!({"cell": 2}));
    assert_eq!(lines_with(&shell, "  cell ").len(), 1);
    done(&mut shell, "table.drop", json!({"cell": 1}));
    assert!(shell.properties().card().is_none(), "nothing left to apply");
    assert_eq!(steps(&shell), n);
    shell.shutdown();
}

/// TB4's numbers: `0,75` reads as 0.75 and `1,500` is ambiguous (the card with its two readings); `table.resolve`
/// picks the decimal reading and Apply commits one command.
#[test]
fn tb4_an_ambiguous_number_is_resolved_on_the_card() {
    let tmp = Tmp::new("tb4-num");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "cables"}));
    done(&mut shell, "navigator.table", json!({"kind": "cables"}));
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "cables", "text": "-106W1"}),
    );
    let cable = col(&shell, NavKind::Cables, "Cable");
    done(
        &mut shell,
        "navigator.sort",
        json!({"kind": "cables", "by": cable}),
    );
    let t = shell.nav(NavKind::Cables).unwrap().table().unwrap().clone();
    let rows: Vec<(String, NavRole)> = shell
        .nav(NavKind::Cables)
        .unwrap()
        .visible()
        .iter()
        .map(|r| {
            (
                t.nodes[r.node as usize].key.clone(),
                t.nodes[r.node as usize].role,
            )
        })
        .collect();
    let first = rows
        .iter()
        .position(|(_, r)| *r == NavRole::Conductor)
        .expect("a core row");
    let csa = col(&shell, NavKind::Cables, "mm²");
    done(
        &mut shell,
        "table.cell",
        json!({"row": rows[first].0, "column": csa}),
    );
    let n = steps(&shell);
    done(&mut shell, "table.paste", json!({"text": "0,75\r\n1,500\r\n"}));
    assert_eq!(steps(&shell), n, "the card, nothing committed");
    let cells = lines_with(&shell, "  cell ");
    assert_eq!(cells.len(), 1, "{cells:?}");
    assert!(
        cells[0].starts_with("  cell 2 ")
            && cells[0].ends_with(" \"mm²\" ambiguous \"1.5 (decimal)\" \"1500 (thousands)\""),
        "{cells:?}"
    );
    assert!(lines_with(&shell, "card ").iter().any(|l| l.ends_with(" table")));
    // WP-S2.09 part 3 (§8.1): the table card opens in the Preview tab like every origin; it stays the navigator's.
    assert!(shell.preview_shown());
    let m = failed(&mut shell, "navigator.apply", json!({}));
    assert_eq!(m, "1 cell is invalid or ambiguous; resolve or drop it first.");
    done(&mut shell, "table.resolve", json!({"cell": 2, "choice": 1}));
    assert!(lines_with(&shell, "  cell ").is_empty());
    done(&mut shell, "navigator.apply", json!({}));
    assert_eq!(steps(&shell), n + 1, "one command");
    let vals = texts(&shell, NavKind::Cables, "mm²");
    assert_eq!(&vals[first..first + 2], ["0.75 mm²", "1.5 mm²"]);
    shell.shutdown();
}

/// TB4's conflicts (D13): two wire-list rows of one connection pasted with different colours: the second is invalid
/// ("same record as row <n>"); with the same colour they merge.
#[test]
fn tb4_two_rows_on_one_record_with_different_values_are_invalid() {
    let tmp = Tmp::new("tb4-same");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "connections"}));
    done(&mut shell, "navigator.table", json!({"kind": "connections"}));
    let n = shell.nav(NavKind::Connections).unwrap();
    let t = n.table().unwrap().clone();
    let vis: Vec<u32> = n.visible().iter().map(|r| r.node).collect();
    // Two neighbouring rows that share a connection.
    let at = (0..vis.len() - 1)
        .find(|&i| {
            let a = &t.nodes[vis[i] as usize].records;
            t.nodes[vis[i + 1] as usize].records.iter().any(|r| a.contains(r))
        })
        .expect("two rows of one connection");
    let (a, b) = (
        t.nodes[vis[at] as usize].clone(),
        t.nodes[vis[at + 1] as usize].clone(),
    );
    let colour = col(&shell, NavKind::Connections, "Colour");
    done(&mut shell, "table.cell", json!({"row": a.key, "column": colour}));
    let s = steps(&shell);
    done(&mut shell, "table.paste", json!({"text": "RD\nBU\n"}));
    assert_eq!(steps(&shell), s);
    let cells = lines_with(&shell, "  cell ");
    assert_eq!(
        cells,
        [format!(
            "  cell 2 \"{}\" \"Colour\" invalid \"same record as row {}\"",
            b.label,
            at + 1
        )]
    );
    done(&mut shell, "preview.cancel", json!({}));
    done(&mut shell, "table.paste", json!({"text": "RD\nRD\n"}));
    assert_eq!(steps(&shell), s + 1, "the same value merges");
    shell.shutdown();
}

/// TB5 (§8.3): designations of 2 terminals at once open the card (rule 2); a fill over 569 devices opens it (rule 1);
/// a single-cell edit of a potential's name opens none (one target); a clear of an enum is invalid.
#[test]
fn tb5_the_policy() {
    let tmp = Tmp::new("tb5");
    let mut shell = strips(&tmp, "x1031");
    let term = col(&shell, NavKind::Strips, "Terminal");
    let k15 = terminal(&shell, "15");
    done(&mut shell, "table.cell", json!({"row": k15, "column": term}));
    let n = steps(&shell);
    done(&mut shell, "table.paste", json!({"text": "98\n99\n"}));
    let card = shell.properties().card().expect("rule 2: the card");
    assert_eq!(card.reason, "reason-identifiers");
    assert_eq!(card.targets, 2);
    assert_eq!(steps(&shell), n);
    done(&mut shell, "preview.cancel", json!({}));
    // A clear of an enum (State) is invalid.
    let state = col(&shell, NavKind::Strips, "State");
    done(&mut shell, "table.cell", json!({"row": k15, "column": state}));
    done(&mut shell, "table.clear", json!({}));
    assert_eq!(
        lines_with(&shell, "  cell "),
        ["  cell 1 \"15\" \"State\" invalid \"cannot be cleared\""]
    );
    done(&mut shell, "preview.cancel", json!({}));
    // A clear of a text (Function text) sets the empty text.
    let fn_col = col(&shell, NavKind::Strips, "Function text");
    done(&mut shell, "table.cell", json!({"column": fn_col}));
    done(&mut shell, "table.clear", json!({}));
    assert_eq!(label_of(&shell, "15"), "");
    // A single-cell edit of a potential's name: one target, no card.
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    done(&mut shell, "navigator.table", json!({"kind": "potentials"}));
    let pot = col(&shell, NavKind::Potentials, "Potential");
    let (key, _) = column(&shell, NavKind::Potentials, "Potential")
        .into_iter()
        .find(|(_, t)| t == "24VDC-1")
        .expect("24VDC-1");
    done(
        &mut shell,
        "table.edit",
        json!({"kind": "potentials", "row": key, "column": pot}),
    );
    done(
        &mut shell,
        "table.input",
        json!({"kind": "potentials", "text": "24VDC-9"}),
    );
    let n = steps(&shell);
    done(&mut shell, "table.commit", json!({"kind": "potentials"}));
    assert!(shell.properties().card().is_none(), "one target: no card");
    assert_eq!(steps(&shell), n + 1);
    assert!(texts(&shell, NavKind::Potentials, "Potential").contains(&"24VDC-9".to_owned()));
    shell.shutdown();
}

/// TB5's rule 1: a fill down over more than 500 device rows (a copy of Golden A with 501 devices more, made through
/// the session) opens the card (`reason-bulk`).
#[test]
fn tb5_a_fill_over_more_than_500_rows_opens_the_card() {
    use ew_viewmodel::ew_api::{AnyCommand, Session, StoreOptions};
    let tmp = Tmp::new("tb5-bulk");
    let (mut session, _) = Session::open_read_only(std::path::Path::new(common::GOLDEN_A), "test").unwrap();
    for _ in 0..501 {
        session
            .execute(&AnyCommand {
                name: "device.create".into(),
                params: json!({"letter": "K"}),
            })
            .unwrap();
    }
    let dir = tmp.0.join("bulk.ew");
    let mut fresh = Session::new(session.world().model.clone(), Default::default());
    fresh.create(&dir, StoreOptions::new("test")).unwrap();
    fresh.close();
    let mut shell = ShellModel::new(
        Mode::Headless,
        ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(&mut shell, "file.open", json!({ "dir": dir.to_string_lossy() }));
    done(&mut shell, "navigator.show", json!({"kind": "devices"}));
    done(&mut shell, "navigator.table", json!({"kind": "devices"}));
    let keys: Vec<String> = column(&shell, NavKind::Devices, "Function text")
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert!(keys.len() > 500, "{}", keys.len());
    mark(&mut shell, NavKind::Devices, &keys);
    let fn_col = col(&shell, NavKind::Devices, "Function text");
    done(&mut shell, "table.cell", json!({"column": fn_col}));
    let rev = shell.revision();
    done(
        &mut shell,
        "table.set",
        json!({"kind": "devices", "column": fn_col, "value": "bulk", "all": true}),
    );
    let card = shell.properties().card().expect("rule 1: the card");
    assert_eq!(card.reason, "reason-bulk");
    assert_eq!(card.targets, keys.len());
    assert_eq!(shell.revision(), rev, "nothing committed before Apply");
    shell.shutdown();
}

/// TB6 (§8.6): copying -X1032's 5 terminals writes the TSV with the header row, byte-equal to the expected text;
/// pasting it back into the same rows changes nothing (no command, no preview).
#[test]
fn tb6_copy_then_paste_back_changes_nothing() {
    let tmp = Tmp::new("tb6");
    let mut shell = strips(&tmp, "x1032");
    let keys: Vec<String> = column(&shell, NavKind::Strips, "Terminal")
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(keys.len(), 5);
    mark(&mut shell, NavKind::Strips, &keys);
    done(&mut shell, "table.copy", json!({}));
    let text = shell.clipboard().text().unwrap().to_owned();
    assert_eq!(text, EXPECTED_X1032);
    done(&mut shell, "table.cell", json!({"row": keys[0]}));
    let (n, calls, rev) = (steps(&shell), shell.session_calls(), shell.revision());
    done(&mut shell, "table.paste", json!({}));
    assert_eq!(steps(&shell), n, "nothing committed");
    assert_eq!(
        shell.session_calls().preview_with,
        calls.preview_with,
        "no command to preview"
    );
    assert_eq!(shell.revision(), rev);
    assert!(shell.properties().card().is_none());
    shell.shutdown();
}

/// The TSV of -X1032's five terminals with the header row (CRLF after every row).
const EXPECTED_X1032: &str = "\
Strip\tTerminal\tFunction text\tGroup\tPart\tPotential\tSide 1 (clamp a)\tSide 2 (clamp b)\tBridge\tState\r\n\
-X1032\t1\tPE\t\tPT 2.5-TWIN-PE\tPE\t-202U1:3 via -106W2 GNYE\tE55:PE\tBR6\tused\r\n\
-X1032\t2\tPE\t\tPT 2.5-TWIN-PE\tPE\t-102G1:−\t-102G1:PE\tBR6\tused\r\n\
-X1032\t3\tPE\t\tPT 2.5-TWIN-PE\tPE\t-201U1:3 via -106W1 GNYE\t-201U1:FE\tBR6\tused\r\n\
-X1032\t4\tPE\t\tPT 2.5-TWIN-PE\tPE\t\t-202U1:FE\tBR6\tused\r\n\
-X1032\t5\tPE\t\tPT 2.5-TWIN-PE\tPE\t\t\tBR6\tspare\r\n";

/// TB8's model half (§8.8): the table's describe lines (header, `columns`, rows with the cursor column, the edit, the
/// pattern dialog), Pages in table mode, the refusal outside table mode, the six words and the 239 actions; switching
/// to the table, moving the cell cursor and typing make no `navigator_table` call.
#[test]
fn tb8_describe_lines_words_and_counts() {
    use ew_viewmodel::describe::{TABLE_WORDS, VOCABULARY};
    assert_eq!(TABLE_WORDS.len(), 6);
    for w in TABLE_WORDS {
        assert!(VOCABULARY.contains(w), "{w}");
    }
    // Merged with WP-S2.09 parts 2 and 3 (16 + 13 words, 5 + 8 actions).
    assert_eq!(VOCABULARY.len(), 344);
    assert_eq!(ew_viewmodel::actions::ACTIONS.len(), 252);
    for (id, keys) in [
        ("table.edit", &["F2"][..]),
        ("table.fill_down", &["Ctrl+D"]),
        ("table.pattern_fill", &["Ctrl+Shift+D"]),
        ("table.paste", &["Ctrl+V"]),
        ("table.copy", &["Ctrl+C"]),
        ("table.clear", &["Delete"]),
    ] {
        let a = ew_viewmodel::actions::find(id).unwrap();
        assert_eq!(a.shortcut, keys, "{id}");
    }
    let tmp = Tmp::new("tb8");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    assert_eq!(
        refused(&mut shell, "table.fill_down", json!({"kind": "strips"})),
        "reason-table-off"
    );
    let calls = shell.session_calls().navigator_table;
    done(&mut shell, "navigator.table", json!({"kind": "strips"}));
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "strips", "text": "x1031"}),
    );
    let by = col(&shell, NavKind::Strips, "Terminal");
    done(&mut shell, "navigator.sort", json!({"kind": "strips", "by": by}));
    done(
        &mut shell,
        "table.cell",
        json!({"kind": "strips", "step": 12, "across": 2}),
    );
    done(
        &mut shell,
        "table.input",
        json!({"kind": "strips", "text": "Spare"}),
    );
    assert_eq!(shell.session_calls().navigator_table, calls, "no table call");
    let d = shell.describe();
    assert!(
        d.contains(
            "navigator \"Terminal strips\" kind strips view tree table sort \"Terminal\" ascending filter \"x1031\" rows 20 showing 1 to 20\n  columns \"Strip\" \"Terminal\" \"Function text\" \"Group\" \"Part\" \"Potential\" \"Side 1 (clamp a)\" \"Side 2 (clamp b)\" \"Bridge\" \"State\"\n  row \"-X1031\" \"1\" \"1+\" \"Pneumatics\" \"PT 2.5-TWIN\" \"24VDC-1\" \"-102F2:Out1\" \"-201U1:5 via -106W1 BN\" \"BR1\" \"used\"\n"
        ),
        "{d}"
    );
    assert!(
        d.contains("\n  row \"-X1031\" \"13\" \"1-\" \"Pneumatics\" \"PT 2.5-TWIN\" \"0V\" \"\" \"-202U1:2 via -106W2 BU\" \"BR5\" \"used\" cursor \"Function text\"\n"),
        "{d}"
    );
    assert!(
        d.contains("\n  editing \"Function text\" buffer \"Spare\"\n"),
        "{d}"
    );
    // Escape ends the edit; the card and the toast are absent.
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "strips"}),
    );
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(lines_with(&shell, "  editing ").is_empty());
    // The Pages navigator in table mode prints as the others.
    done(&mut shell, "navigator.table", json!({"kind": "pages"}));
    let d = shell.describe();
    assert!(
        d.contains("navigator \"Pages\" kind pages view tree table sort \"Label\" ascending filter \"\" rows 36 showing 1 to 36\n  columns \"Folio\" \"Title\" \"Type\" \"Frame\" \"Generated\"\n"),
        "{d}"
    );
    assert!(!d.contains("navigator \"Pages\" mode "), "{d}");
    // A generated page's title is read-only.
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "pages"}),
    );
    let generated = column(&shell, NavKind::Pages, "Folio")
        .into_iter()
        .find(|(_, f)| f == "2")
        .unwrap()
        .0;
    let m = failed(
        &mut shell,
        "table.edit",
        json!({"kind": "pages", "row": generated, "column": "title"}),
    );
    assert_eq!(m, "Title is read-only here: generated.");
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "pages", "on": false}),
    );
    assert!(shell.describe().contains("navigator \"Pages\" mode "));
    shell.shutdown();
}

/// WP-S2.09 part 2 meets WP-S2.06 part 3: the Messages pane has no table mode (its rows are read-only findings with
/// their own header, detail block and quick-fix card); `navigator.table` fails and changes nothing.
#[test]
fn the_messages_pane_has_no_table_mode() {
    let tmp = Tmp::new("tb-messages");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "messages.show", json!({}));
    let m = failed(
        &mut shell,
        "navigator.table",
        json!({"kind": "messages", "on": true}),
    );
    assert_eq!(m, "The messages pane has no table mode; its rows are findings.");
    assert!(shell.nav(NavKind::Messages).unwrap().table_mode().is_none());
    assert!(shell.navigator_open(NavKind::Messages));
    shell.shutdown();
}
