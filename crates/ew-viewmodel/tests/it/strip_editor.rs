// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strip editor of WP-S2.07 part 1 (§6) on copies of Golden A, headless: the `Order` sort, the `pos` cells and the
//! accessory rows (SE7), the Group column and the Properties Group row (SE8), insert, delete and move (SE9), and the
//! describe lines (SE14).

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// The column key of the Terminal strips column labelled `label`.
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

/// The rows on show in display order: (key, the cell of the column labelled `label`).
fn column(shell: &ShellModel, label: &str) -> Vec<(String, String)> {
    let n = shell.nav(NavKind::Strips).unwrap();
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
fn texts(shell: &ShellModel, label: &str) -> Vec<String> {
    column(shell, label).into_iter().map(|(_, t)| t).collect()
}

/// The key of the row on show whose cell in column `label` is `text`.
fn row(shell: &ShellModel, label: &str, text: &str) -> String {
    column(shell, label)
        .into_iter()
        .find(|(_, t)| t == text)
        .unwrap_or_else(|| panic!("no row with {label} {text}"))
        .0
}

/// The cursor row's key.
fn cursor(shell: &ShellModel) -> String {
    shell
        .nav(NavKind::Strips)
        .and_then(|n| n.cursor_key())
        .expect("a cursor row")
}

fn steps(shell: &ShellModel) -> usize {
    shell.recorded_commands().len()
}

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

/// A writable copy of Golden A, headless, with the strip editor open on `strip`.
fn editor(tmp: &Tmp, strip: &str) -> ShellModel {
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "strip.edit", json!({ "strip": strip }));
    shell
}

/// Puts the cursor on the row `key`.
fn at(shell: &mut ShellModel, key: &str) {
    done(shell, "table.cell", json!({"kind": "strips", "row": key}));
}

/// Marks the rows `keys`.
fn mark(shell: &mut ShellModel, keys: &[String]) {
    done(shell, "navigator.select", json!({"kind": "strips", "keys": keys}));
}

const SEQ_1_TO_20: [&str; 20] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19",
    "20",
];

/// SE7 (D2, D3, §6.2): the Terminal strips table sorts by `Order` by default (the strips in natural tag order, each
/// strip's terminals in sequence order); `strip.edit` filters it to one strip (`strip:-x1031`, 20 rows) with the cursor
/// on its first row; accessory rows are placed and numbered by D3 (`after_seq` 0, two at 4, 20); the read-only reasons
/// of §6.2 refuse an edit of a Pos cell, an accessory row's cell and a Bridge part cell of a terminal without a bridge.
#[test]
fn se7_order_sort_pos_cells_accessory_rows_and_reasons() {
    let tmp = Tmp::new("se7");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "strips", "on": true}),
    );
    let header = lines_with(&shell, "navigator \"Terminal strips\"");
    assert!(header[0].contains(" sort \"Order\" ascending "), "{header:?}");
    let mut strips = texts(&shell, "Strip");
    strips.dedup();
    // The as-built natural order of the tags (`ew_mltext::natural_cmp`): `-` sorts before a digit.
    assert_eq!(strips, ["-X1031", "-X1032", "1X1"]);
    let x1031: Vec<String> = column(&shell, "Strip")
        .into_iter()
        .zip(texts(&shell, "Pos"))
        .filter(|((_, s), _)| s == "-X1031")
        .map(|(_, p)| p)
        .collect();
    assert_eq!(x1031, SEQ_1_TO_20);

    // The strip editor on -X1031.
    done(&mut shell, "strip.edit", json!({"strip": "-X1031"}));
    let header = lines_with(&shell, "navigator \"Terminal strips\"");
    assert!(
        header[0].contains(" sort \"Order\" ascending filter \"strip:-x1031\" rows 20 showing 1 to 20"),
        "{header:?}"
    );
    assert_eq!(texts(&shell, "Pos"), SEQ_1_TO_20);
    assert_eq!(texts(&shell, "Terminal"), SEQ_1_TO_20);
    assert_eq!(cursor(&shell), row(&shell, "Terminal", "1"));

    // Accessories: after 20, two after 4 (the second added on the first one's row), and at the start (the strip row,
    // reached in the tree view).
    let k20 = row(&shell, "Terminal", "20");
    at(&mut shell, &k20);
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(cursor(&shell), row(&shell, "Pos", "20.1"));
    let k4 = row(&shell, "Terminal", "4");
    at(&mut shell, &k4);
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(cursor(&shell), row(&shell, "Pos", "4.1"));
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(cursor(&shell), row(&shell, "Pos", "4.2"));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "strips", "on": false}),
    );
    done(
        &mut shell,
        "navigator.filter",
        json!({"kind": "strips", "text": ""}),
    );
    mark(&mut shell, &["-X1031".to_owned()]);
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Added End bracket at the start of -X1031\" undo"]
    );
    done(&mut shell, "strip.edit", json!({"strip": "-X1031"}));
    let mut expected: Vec<&str> = vec!["0.1"];
    expected.extend(&SEQ_1_TO_20[..4]);
    expected.extend(["4.1", "4.2"]);
    expected.extend(&SEQ_1_TO_20[4..]);
    expected.push("20.1");
    assert_eq!(texts(&shell, "Pos"), expected);
    let labels = texts(&shell, "Function text");
    assert_eq!(labels[0], "End bracket");
    assert_eq!(labels[5], "End bracket");
    assert_eq!(texts(&shell, "Part")[0], "TBD");
    assert_eq!(texts(&shell, "Terminal")[0], "");

    // The read-only reasons (§6.2).
    let k13 = row(&shell, "Terminal", "13");
    let pos = col(&shell, "Pos");
    let message = failed(
        &mut shell,
        "table.edit",
        json!({"kind": "strips", "row": k13, "column": pos}),
    );
    assert!(
        message.contains("move rows with Alt+Up and Alt+Down"),
        "{message}"
    );
    let acc = row(&shell, "Pos", "20.1");
    let label = col(&shell, "Function text");
    let message = failed(
        &mut shell,
        "table.edit",
        json!({"kind": "strips", "row": acc, "column": label}),
    );
    assert!(message.contains("accessory row"), "{message}");
    // The accessory row's Part cell is editable (D5).
    let part = col(&shell, "Part");
    done(
        &mut shell,
        "table.edit",
        json!({"kind": "strips", "row": acc, "column": part}),
    );
    assert_eq!(
        lines_with(&shell, "  editing "),
        ["  editing \"Part\" buffer \"TBD\""]
    );
    done(&mut shell, "ui.dismiss", json!({}));
    // A terminal without a bridge: a new terminal at the end of -X1031.
    let k20 = row(&shell, "Pos", "20.1");
    at(&mut shell, &k20);
    done(&mut shell, "strip.insert", json!({}));
    let new = cursor(&shell);
    let bridge_part = col(&shell, "Bridge part");
    let message = failed(
        &mut shell,
        "table.edit",
        json!({"kind": "strips", "row": new, "column": bridge_part}),
    );
    assert!(message.contains("the terminal has no bridge"), "{message}");
    shell.shutdown();
}

/// SE8 (D8, D13, FT-553): the Group column and the Properties Group row. "Reserve" on -X1031:7 and :8 splits the
/// spare group 5–10 into 5–6, 7–8, 9–10 with the old labels kept; one `fields.set` of `strip.groups` and one undo step;
/// the Properties Group row of the two terminals does the same; the Bridge part fill over BR1 is one `fields.set` of
/// the carrier's part (D6).
#[test]
fn se8_group_labels_through_the_table_and_the_properties_row() {
    let tmp = Tmp::new("se8");
    let mut shell = editor(&tmp, "-X1031");
    let before = texts(&shell, "Group");
    let group = col(&shell, "Group");
    let keys: Vec<String> = ["7", "8"].iter().map(|n| row(&shell, "Terminal", n)).collect();
    let n = steps(&shell);
    done(
        &mut shell,
        "table.set",
        json!({"kind": "strips", "column": group, "value": "Reserve", "rows": keys}),
    );
    assert_eq!(steps(&shell), n + 1, "one step");
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "fields.set", "{params}");
    assert_eq!(params["path"], "strip.groups");
    let after = texts(&shell, "Group");
    let mut expected = before.clone();
    expected[6] = "Reserve".into();
    expected[7] = "Reserve".into();
    assert_eq!(after, expected);
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(texts(&shell, "Group"), before, "one undo step");

    // The Properties Group row of the same two terminals.
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:-X1031:7", "tag:-X1031:8"]}),
    );
    let rows = lines_with(&shell, "    row \"device/derived.group\"");
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].contains("\"Group\" \"Spare\""), "{rows:?}");
    let n = steps(&shell);
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/derived.group", "value": "Reserve"}),
    );
    if shell.properties().card().is_some() {
        done(&mut shell, "preview.apply", json!({}));
    }
    assert_eq!(steps(&shell), n + 1, "one step");
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "fields.set", "{params}");
    assert_eq!(params["path"], "strip.groups");
    assert_eq!(texts(&shell, "Group"), expected);
    // The same text again builds no command.
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/derived.group", "value": "Reserve"}),
    );
    assert_eq!(steps(&shell), n + 1, "no command for the same text");

    // The Bridge part over BR1 (terminals 1 to 4): one `fields.set` on the group's carrier.
    let bridge_part = col(&shell, "Bridge part");
    let keys: Vec<String> = ["1", "2", "3", "4"]
        .iter()
        .map(|n| row(&shell, "Terminal", n))
        .collect();
    let n = steps(&shell);
    done(
        &mut shell,
        "table.set",
        json!({"kind": "strips", "column": bridge_part, "value": "TBD", "rows": keys}),
    );
    if shell.properties().card().is_some() {
        done(&mut shell, "preview.apply", json!({}));
    }
    assert_eq!(steps(&shell), n + 1, "one step");
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "fields.set", "{params}");
    assert_eq!(params["path"], "overrides.wire_part");
    assert_eq!(params["targets"].as_array().map(Vec::len), Some(1), "{params}");
    assert_eq!(
        &texts(&shell, "Bridge part")[..5],
        ["TBD", "TBD", "TBD", "TBD", ""]
    );
    shell.shutdown();
}

/// SE9 (§6.4, D12, D14, FR-TRM-015): Insert before -X1031:5 (one `terminal.insert`, the toast, the cursor on the new
/// row); Ctrl+Delete on two terminal rows opens the card with their bridges and wires and commits nothing; on an
/// accessory row it commits with the toast; on a mix it is refused; Alt+Up on :15 three times puts it at seq 12's place
/// in three steps (inside bridge group BR5), undone by three undos; a move that takes :2 out of BR1 fails with
/// EW-TERM-004 (CI-06, a prevent invariant). (Enter-to-place, §6.8, is descoped from part 1.)
#[test]
fn se9_insert_delete_and_move_rows() {
    let tmp = Tmp::new("se9");
    let mut shell = editor(&tmp, "-X1031");
    // Insert.
    let k5 = row(&shell, "Terminal", "5");
    at(&mut shell, &k5);
    let n = steps(&shell);
    done(&mut shell, "strip.insert", json!({}));
    assert_eq!(steps(&shell), n + 1);
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "terminal.insert", "{params}");
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Inserted 1 terminal before -X1031:5\" undo"]
    );
    let terminals = texts(&shell, "Terminal");
    assert_eq!(terminals[..6], ["1", "2", "3", "4", "21", "5"]);
    assert_eq!(cursor(&shell), row(&shell, "Terminal", "21"));
    let pos: Vec<String> = (1..=21).map(|n| n.to_string()).collect();
    assert_eq!(texts(&shell, "Pos"), pos);
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(texts(&shell, "Terminal"), SEQ_1_TO_20);

    // Delete two terminal rows: the card, nothing committed.
    let keys: Vec<String> = ["5", "6"].iter().map(|n| row(&shell, "Terminal", n)).collect();
    mark(&mut shell, &keys);
    let (n, rev) = (steps(&shell), shell.revision());
    done(&mut shell, "strip.delete_rows", json!({}));
    let card = shell.properties().card().expect("the card (D12)");
    assert_eq!(card.reason, "reason-strip-delete");
    assert_eq!(card.preview.name, "edit.delete");
    // Two devices and two functions, BR2's connection and the wire on terminal 5's side 1, at the least.
    assert!(card.changes >= 6, "{}", card.changes);
    assert_eq!((steps(&shell), shell.revision()), (n, rev), "nothing committed");
    done(&mut shell, "preview.cancel", json!({}));

    // Delete an accessory row: committed with the toast.
    let k20 = row(&shell, "Terminal", "20");
    at(&mut shell, &k20);
    done(&mut shell, "strip.add_accessory", json!({}));
    let acc = row(&shell, "Pos", "20.1");
    mark(&mut shell, std::slice::from_ref(&acc));
    let n = steps(&shell);
    done(&mut shell, "strip.delete_rows", json!({}));
    assert_eq!(steps(&shell), n + 1);
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "fields.set", "{params}");
    assert_eq!(params["path"], "strip.accessories");
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Removed 1 accessory\" undo"]
    );
    assert_eq!(texts(&shell, "Pos"), SEQ_1_TO_20);
    // A mix: refused.
    done(&mut shell, "edit.undo", json!({}));
    let acc = row(&shell, "Pos", "20.1");
    let k20 = row(&shell, "Terminal", "20");
    mark(&mut shell, &[acc.clone(), k20]);
    assert_eq!(
        refused(&mut shell, "strip.delete_rows", json!({})),
        "reason-strip-mixed-rows"
    );
    assert_eq!(
        refused(&mut shell, "strip.move", json!({"step": -1})),
        "reason-strip-mixed-rows"
    );
    done(&mut shell, "edit.undo", json!({}));

    // Move :15 up three times: it stays inside BR5 (11 to 20), so every bridge group stays consecutive (CI-06).
    let k15 = row(&shell, "Terminal", "15");
    mark(&mut shell, std::slice::from_ref(&k15));
    let n = steps(&shell);
    for k in 1..=3 {
        done(&mut shell, "strip.move", json!({"step": -1}));
        assert_eq!(steps(&shell), n + k);
        assert_eq!(last(&shell).0, "terminal.move");
    }
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Moved 1 terminal of -X1031\" undo"]
    );
    let terminals = texts(&shell, "Terminal");
    assert_eq!(terminals[10..15], ["11", "15", "12", "13", "14"]);
    assert_eq!(cursor(&shell), k15, "the cursor follows the row");
    // The first row cannot move up.
    let k1 = row(&shell, "Terminal", "1");
    mark(&mut shell, std::slice::from_ref(&k1));
    assert_eq!(
        refused(&mut shell, "strip.move", json!({"step": -1})),
        "reason-strip-move-none"
    );
    for _ in 0..3 {
        done(&mut shell, "edit.undo", json!({}));
    }
    assert_eq!(texts(&shell, "Terminal"), SEQ_1_TO_20, "three undo steps");
    // Down, and before a designation / the end.
    let k2 = row(&shell, "Terminal", "2");
    mark(&mut shell, std::slice::from_ref(&k2));
    done(&mut shell, "strip.move", json!({"step": 1}));
    assert_eq!(texts(&shell, "Terminal")[..4], ["1", "3", "2", "4"]);
    // To the end: :2 would leave bridge group BR1 (1 to 4); terminal.move refuses, naming the group and the way
    // out (FT-641, CI-06); nothing is committed.
    let n = steps(&shell);
    match shell.dispatch("strip.move", &json!({"before": "end"})) {
        Outcome::Failed { message } => assert!(
            message.contains(
                "the move would split bridge group 1 (terminals 1, 3, 2, 4); remove terminal 2 from the group first"
            ),
            "{message}"
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(steps(&shell), n, "nothing committed");
    assert_eq!(texts(&shell, "Terminal")[..4], ["1", "3", "2", "4"]);
    done(&mut shell, "strip.move", json!({"before": "1"}));
    assert_eq!(texts(&shell, "Terminal")[..3], ["2", "1", "3"]);
    shell.shutdown();
}

/// SE9's context (§6.9): every gesture refuses `reason-not-strip-editor` while the Terminal strips navigator is closed.
#[test]
fn se9_gestures_need_the_strip_editor() {
    let tmp = Tmp::new("se9-context");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    for (action, params) in [
        ("strip.insert", json!({})),
        ("strip.delete_rows", json!({})),
        ("strip.move", json!({"step": 1})),
        ("strip.add_accessory", json!({})),
        ("strip.unbridge", json!({})),
    ] {
        assert_eq!(
            refused(&mut shell, action, params),
            "reason-not-strip-editor",
            "{action}"
        );
    }
    assert_eq!(
        failed(&mut shell, "strip.edit", json!({"strip": "-X9"})),
        failed(
            &mut shell,
            "navigator.select",
            json!({"kind": "strips", "keys": ["-X9"]})
        ),
        "an unknown strip is the navigator's key error"
    );
    shell.shutdown();
}

/// Runs the shortcut bound to `action` the way the window does: `shortcut_action` picks the action and parameters for
/// the focus, then dispatch.
fn key(shell: &mut ShellModel, action: &'static str) -> &'static str {
    let (a, p) = shell.shortcut_action(action);
    done(shell, a, p);
    a
}

/// The WP-S2.07 part 1 ruling: Alt+Up and Alt+Down (`pages.move_up`, `pages.move_down`) run `strip.move` while the
/// Terminal strips navigator has the focus, one undo step each; elsewhere they keep `pages.move_*`.
#[test]
fn alt_arrows_move_strip_rows_while_the_strips_navigator_has_the_focus() {
    let tmp = Tmp::new("se9-alt-arrows");
    let mut shell = editor(&tmp, "-X1031");
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "strips"}),
    );
    assert_eq!(shell.focused_navigator(), Some(NavKind::Strips));
    let k2 = row(&shell, "Terminal", "2");
    mark(&mut shell, std::slice::from_ref(&k2));
    let n = steps(&shell);
    // Alt+Down: the row moves down one place, one step.
    assert_eq!(key(&mut shell, "pages.move_down"), "strip.move");
    assert_eq!(texts(&shell, "Terminal")[..4], ["1", "3", "2", "4"]);
    assert_eq!(steps(&shell), n + 1);
    assert_eq!(last(&shell).0, "terminal.move");
    // Alt+Up: back.
    assert_eq!(key(&mut shell, "pages.move_up"), "strip.move");
    assert_eq!(texts(&shell, "Terminal"), SEQ_1_TO_20);
    assert_eq!(steps(&shell), n + 2);
    // At the first row Alt+Up is refused as `strip.move` is; nothing changes.
    let k1 = row(&shell, "Terminal", "1");
    mark(&mut shell, std::slice::from_ref(&k1));
    let (a, p) = shell.shortcut_action("pages.move_up");
    assert_eq!((a, &p), ("strip.move", &json!({"step": -1})));
    assert_eq!(refused(&mut shell, a, p), "reason-strip-move-none");
    assert_eq!(texts(&shell, "Terminal"), SEQ_1_TO_20);
    assert_eq!(steps(&shell), n + 2);
    // The Pages navigator or the canvas focused: the page moves keep the keys.
    for area in [
        json!({"area": "navigator", "kind": "pages"}),
        json!({"area": "canvas"}),
    ] {
        done(&mut shell, "ui.focus", area);
        for action in ["pages.move_up", "pages.move_down"] {
            assert_eq!(shell.shortcut_action(action), (action, Value::Null));
        }
    }
    // Other bindings are untouched in the strips navigator.
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "strips"}),
    );
    assert_eq!(
        shell.shortcut_action("strip.delete_rows"),
        ("strip.delete_rows", Value::Null)
    );
    shell.shutdown();
}

/// SE14 (§6.10): the describe lines of the strip editor: the header with `sort "Order"` and the `strip:` filter, an
/// accessory row printed with the row word `accessory` and its cells.
#[test]
fn se14_describe_lines() {
    let tmp = Tmp::new("se14");
    let mut shell = editor(&tmp, "-X1031");
    let k20 = row(&shell, "Terminal", "20");
    at(&mut shell, &k20);
    done(&mut shell, "strip.add_accessory", json!({}));
    let d = shell.describe();
    assert!(
        d.contains(
            "navigator \"Terminal strips\" kind strips view tree table sort \"Order\" ascending filter \"strip:-x1031\" rows 21 showing 1 to 21\n  columns \"Strip\" \"Pos\" \"Terminal\" \"Function text\" \"Group\" \"Part\" \"Potential\" \"Side 1 (clamp a)\" \"Side 2 (clamp b)\" \"Bridge\" \"Bridge part\" \"State\"\n  row \"-X1031\" \"1\" \"1\" \"1+\" \"Pneumatics\" \"PT 2.5-TWIN\" \"24VDC-1\" \"-102F2:Out1\" \"-201U1:5 via -106W1 BN\" \"BR1\" \"\" \"used\"\n"
        ),
        "{d}"
    );
    assert!(
        d.contains(
            "\n  accessory \"-X1031\" \"20.1\" \"\" \"End bracket\" \"\" \"TBD\" \"\" \"\" \"\" \"\" \"\" \"\" cursor \"Strip\"\n"
        ),
        "{d}"
    );
    shell.shutdown();
}
