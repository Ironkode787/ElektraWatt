// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strip editor on copies of the sample, headless, with a bridge part imported (the sample's end bracket is the
//! starter's, which its terminals bring with them): the `Order` sort, the `pos` cells and the accessory rows, the Group
//! column and the Properties Group row, insert, delete and move, and the describe lines.

use crate::common;

use common::{
    Tmp, done, failed, lines_with, refused, strip_at, strip_col, strip_column, strip_cursor, strip_row,
    strip_texts,
};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

fn steps(shell: &ShellModel) -> usize {
    shell.recorded_commands().len()
}

fn last(shell: &ShellModel) -> (String, Value) {
    let s = shell.recorded_commands().last().expect("a committed step");
    (s.cmd.clone(), s.params.clone())
}

/// A writable copy of the sample on folio 102, headless, with the accessory parts the sample lacks
/// ([`common::ACCESSORY_PARTS`]).
fn with_accessory_parts(tmp: &Tmp) -> ShellModel {
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    common::import_parts(&mut shell, tmp, common::ACCESSORY_PARTS);
    shell
}

/// [`with_accessory_parts`] with the strip editor open on `strip`.
fn editor(tmp: &Tmp, strip: &str) -> ShellModel {
    let mut shell = with_accessory_parts(tmp);
    done(&mut shell, "strip.edit", json!({ "strip": strip }));
    shell
}

/// Marks the rows `keys`.
fn mark(shell: &mut ShellModel, keys: &[String]) {
    done(shell, "navigator.select", json!({"kind": "strips", "keys": keys}));
}

/// Delete over the acted rows with the cell cursor in the key column (Strip), which takes them whole.
fn delete_rows(shell: &mut ShellModel) -> Outcome {
    done(shell, "table.cell", json!({"kind": "strips", "column": "strip"}));
    shell.dispatch("table.clear", &json!({"kind": "strips"}))
}

/// -X1021's terminals, numbered 1 to 8 by the sample's script (BR1 over 1 to 4, BR2 over 5 to 8).
const SEQ_1_TO_8: [&str; 8] = ["1", "2", "3", "4", "5", "6", "7", "8"];

/// SE7: the Terminal strips table sorts by `Order` by default (the strips in natural tag order, each
/// strip's terminals in sequence order); `strip.edit` filters it to one strip (`strip:-X1021`, 8 rows) with the cursor
/// on its first row; accessory rows are placed and numbered by the strip editor (`after_seq` 0, two at 4, 8); the read-only reasons
/// refuse an edit of a Pos cell, an accessory row's cell and a Bridge part cell of a terminal without a bridge.
#[test]
fn order_sort_pos_cells_accessory_rows_and_reasons() {
    let tmp = Tmp::new("se7");
    let mut shell = with_accessory_parts(&tmp);
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    done(
        &mut shell,
        "navigator.table",
        json!({"kind": "strips", "on": true}),
    );
    let header = lines_with(&shell, "navigator \"Terminal strips\"");
    assert!(header[0].contains(" sort \"Order\" ascending "), "{header:?}");
    let mut strips = strip_texts(&shell, "Strip");
    strips.dedup();
    // The as-built natural order of the tags (`ew_mltext::natural_cmp`): `-` sorts before a digit.
    assert_eq!(strips, ["-X1011", "-X1021", "-X1031"]);
    let x1021: Vec<String> = strip_column(&shell, "Strip")
        .into_iter()
        .zip(strip_texts(&shell, "Pos"))
        .filter(|((_, s), _)| s == "-X1021")
        .map(|(_, p)| p)
        .collect();
    assert_eq!(x1021, SEQ_1_TO_8);

    // The strip editor on -X1021.
    done(&mut shell, "strip.edit", json!({"strip": "-X1021"}));
    let header = lines_with(&shell, "navigator \"Terminal strips\"");
    assert!(
        header[0].contains(" sort \"Order\" ascending filter \"strip:-X1021\" rows 11 showing 1 to 11"),
        "{header:?}"
    );
    assert_eq!(strip_texts(&shell, "Pos"), SEQ_1_TO_8);
    assert_eq!(strip_texts(&shell, "Terminal"), SEQ_1_TO_8);
    assert_eq!(strip_cursor(&shell), strip_row(&shell, "Terminal", "1"));

    // Accessories: after 8, two after 4 (the second added on the first one's row), and at the start (the strip row,
    // reached in the tree view).
    let k8 = strip_row(&shell, "Terminal", "8");
    strip_at(&mut shell, &k8);
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(strip_cursor(&shell), strip_row(&shell, "Pos", "8.1"));
    let k4 = strip_row(&shell, "Terminal", "4");
    strip_at(&mut shell, &k4);
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(strip_cursor(&shell), strip_row(&shell, "Pos", "4.1"));
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(strip_cursor(&shell), strip_row(&shell, "Pos", "4.2"));
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
    mark(&mut shell, &["-X1021".to_owned()]);
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Added end bracket at the start of -X1021.\" undo"]
    );
    done(&mut shell, "strip.edit", json!({"strip": "-X1021"}));
    let mut expected: Vec<&str> = vec!["0.1"];
    expected.extend(&SEQ_1_TO_8[..4]);
    expected.extend(["4.1", "4.2"]);
    expected.extend(&SEQ_1_TO_8[4..]);
    expected.push("8.1");
    assert_eq!(strip_texts(&shell, "Pos"), expected);
    let labels = strip_texts(&shell, "Function text");
    assert_eq!(labels[0], "End bracket");
    assert_eq!(labels[5], "End bracket");
    assert_eq!(strip_texts(&shell, "Part")[0], "EW-GEN-EB-1");
    assert_eq!(strip_texts(&shell, "Terminal")[0], "");

    // The read-only reasons.
    let k6 = strip_row(&shell, "Terminal", "6");
    let pos = strip_col(&shell, "Pos");
    let message = failed(
        &mut shell,
        "table.edit",
        json!({"kind": "strips", "row": k6, "column": pos}),
    );
    assert!(
        message.contains("move rows with Alt+Up and Alt+Down"),
        "{message}"
    );
    let acc = strip_row(&shell, "Pos", "8.1");
    let label = strip_col(&shell, "Function text");
    let message = failed(
        &mut shell,
        "table.edit",
        json!({"kind": "strips", "row": acc, "column": label}),
    );
    assert!(message.contains("accessory row"), "{message}");
    // The accessory row's Part cell is editable.
    let part = strip_col(&shell, "Part");
    done(
        &mut shell,
        "table.edit",
        json!({"kind": "strips", "row": acc, "column": part}),
    );
    assert_eq!(
        lines_with(&shell, "  editing "),
        ["  editing \"Part\" buffer \"EW-GEN-EB-1\""]
    );
    done(&mut shell, "ui.dismiss", json!({}));
    // A terminal without a bridge: a new terminal at the end of -X1021.
    let k8 = strip_row(&shell, "Pos", "8.1");
    strip_at(&mut shell, &k8);
    done(&mut shell, "strip.insert", json!({}));
    let new = strip_cursor(&shell);
    let bridge_part = strip_col(&shell, "Bridge part");
    let message = failed(
        &mut shell,
        "table.edit",
        json!({"kind": "strips", "row": new, "column": bridge_part}),
    );
    assert!(message.contains("the terminal has no bridge"), "{message}");
    shell.shutdown();
}

/// SE8: the Group column and the Properties Group row. "Reserve" on -X1021:7 and :8 gives the two a group
/// label of their own, the others' labels kept; one `fields.set` of `strip.groups` and one undo step;
/// the Properties Group row of the two terminals does the same; the Bridge part fill over BR1 is one `fields.set` of
/// the carrier's part.
#[test]
fn group_labels_through_the_table_and_the_properties_row() {
    let tmp = Tmp::new("se8");
    let mut shell = editor(&tmp, "-X1021");
    let before = strip_texts(&shell, "Group");
    let group = strip_col(&shell, "Group");
    let keys: Vec<String> = ["7", "8"]
        .iter()
        .map(|n| strip_row(&shell, "Terminal", n))
        .collect();
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
    let after = strip_texts(&shell, "Group");
    let mut expected = before.clone();
    expected[6] = "Reserve".into();
    expected[7] = "Reserve".into();
    assert_eq!(after, expected);
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(strip_texts(&shell, "Group"), before, "one undo step");

    // The Properties Group row of the same two terminals.
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["tag:-X1021:7", "tag:-X1021:8"]}),
    );
    let rows = lines_with(&shell, "    row \"device/derived.group\"");
    assert_eq!(rows.len(), 1, "{rows:?}");
    // The sample labels none of -X1021's groups.
    assert!(rows[0].contains("\"Group\" none"), "{rows:?}");
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
    assert_eq!(strip_texts(&shell, "Group"), expected);
    // The same text again builds no command.
    done(
        &mut shell,
        "props.set",
        json!({"row": "device/derived.group", "value": "Reserve"}),
    );
    assert_eq!(steps(&shell), n + 1, "no command for the same text");

    // The Bridge part over BR1 (terminals 1 to 4): one `fields.set` on the group's carrier.
    let bridge_part = strip_col(&shell, "Bridge part");
    let keys: Vec<String> = ["1", "2", "3", "4"]
        .iter()
        .map(|n| strip_row(&shell, "Terminal", n))
        .collect();
    let n = steps(&shell);
    done(
        &mut shell,
        "table.set",
        json!({"kind": "strips", "column": bridge_part, "value": "BRIDGE-1", "rows": keys}),
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
        &strip_texts(&shell, "Bridge part")[..5],
        ["BRIDGE-1", "BRIDGE-1", "BRIDGE-1", "BRIDGE-1", ""]
    );
    shell.shutdown();
}

/// SE9 (FR-TRM-015): Insert before -X1021:5 (one `terminal.insert`, the toast, the cursor on the new
/// row); Delete with the cursor in the key column on two terminal rows deletes their terminals in one step, as the
/// page's Delete (Ctrl+Delete did the same through a card); on an accessory row it commits with the toast; on a mix it
/// is refused; Alt+Up on :8 three times puts it at seq 5's place
/// in three steps (inside bridge group BR2), undone by three undos; a move that takes :2 out of BR1 fails with
/// EW-TERM-004 (CI-06, a prevent invariant). (Enter-to-place is descoped from part 1.)
#[test]
fn insert_delete_and_move_rows() {
    let tmp = Tmp::new("se9");
    let mut shell = editor(&tmp, "-X1021");
    // Insert.
    let k5 = strip_row(&shell, "Terminal", "5");
    strip_at(&mut shell, &k5);
    let n = steps(&shell);
    done(&mut shell, "strip.insert", json!({}));
    assert_eq!(steps(&shell), n + 1);
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "terminal.insert", "{params}");
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Inserted 1 terminal before terminal 5 of -X1021.\" undo"]
    );
    let terminals = strip_texts(&shell, "Terminal");
    assert_eq!(terminals[..6], ["1", "2", "3", "4", "9", "5"]);
    assert_eq!(strip_cursor(&shell), strip_row(&shell, "Terminal", "9"));
    let pos: Vec<String> = (1..=9).map(|n| n.to_string()).collect();
    assert_eq!(strip_texts(&shell, "Pos"), pos);
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(strip_texts(&shell, "Terminal"), SEQ_1_TO_8);

    // Delete two terminal rows whole: one `edit.delete` of their terminals, no card.
    let keys: Vec<String> = ["5", "6"]
        .iter()
        .map(|n| strip_row(&shell, "Terminal", n))
        .collect();
    mark(&mut shell, &keys);
    let n = steps(&shell);
    assert_eq!(delete_rows(&mut shell), Outcome::Done);
    assert!(shell.properties().card().is_none(), "no card");
    assert_eq!(steps(&shell), n + 1, "one step");
    assert_eq!(last(&shell).0, "edit.delete");
    assert_eq!(strip_texts(&shell, "Terminal"), ["1", "2", "3", "4", "7", "8"]);
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(strip_texts(&shell, "Terminal"), SEQ_1_TO_8);

    // Delete an accessory row: committed with the toast.
    let k8 = strip_row(&shell, "Terminal", "8");
    strip_at(&mut shell, &k8);
    done(&mut shell, "strip.add_accessory", json!({}));
    let acc = strip_row(&shell, "Pos", "8.1");
    mark(&mut shell, std::slice::from_ref(&acc));
    let n = steps(&shell);
    assert_eq!(delete_rows(&mut shell), Outcome::Done);
    assert_eq!(steps(&shell), n + 1);
    let (cmd, params) = last(&shell);
    assert_eq!(cmd, "fields.set", "{params}");
    assert_eq!(params["path"], "strip.accessories");
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Removed 1 accessory.\" undo"]
    );
    assert_eq!(strip_texts(&shell, "Pos"), SEQ_1_TO_8);
    // A mix: refused.
    done(&mut shell, "edit.undo", json!({}));
    let acc = strip_row(&shell, "Pos", "8.1");
    let k8 = strip_row(&shell, "Terminal", "8");
    mark(&mut shell, &[acc.clone(), k8]);
    done(
        &mut shell,
        "table.cell",
        json!({"kind": "strips", "column": "strip"}),
    );
    assert_eq!(
        refused(&mut shell, "table.clear", json!({"kind": "strips"})),
        "reason-strip-mixed-rows"
    );
    assert_eq!(
        refused(&mut shell, "strip.move", json!({"step": -1})),
        "reason-strip-mixed-rows"
    );
    done(&mut shell, "edit.undo", json!({}));

    // Move :8 up three times: it stays inside BR2 (5 to 8), so every bridge group stays consecutive (CI-06).
    let k15 = strip_row(&shell, "Terminal", "8");
    mark(&mut shell, std::slice::from_ref(&k15));
    let n = steps(&shell);
    for k in 1..=3 {
        done(&mut shell, "strip.move", json!({"step": -1}));
        assert_eq!(steps(&shell), n + k);
        assert_eq!(last(&shell).0, "terminal.move");
    }
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Moved terminal 8 of -X1021.\" undo"]
    );
    let terminals = strip_texts(&shell, "Terminal");
    assert_eq!(terminals[3..8], ["4", "8", "5", "6", "7"]);
    assert_eq!(strip_cursor(&shell), k15, "the cursor follows the row");
    // The first row cannot move up.
    let k1 = strip_row(&shell, "Terminal", "1");
    mark(&mut shell, std::slice::from_ref(&k1));
    assert_eq!(
        refused(&mut shell, "strip.move", json!({"step": -1})),
        "reason-strip-move-none"
    );
    for _ in 0..3 {
        done(&mut shell, "edit.undo", json!({}));
    }
    assert_eq!(strip_texts(&shell, "Terminal"), SEQ_1_TO_8, "three undo steps");
    // Down, and before a designation / the end.
    let k2 = strip_row(&shell, "Terminal", "2");
    mark(&mut shell, std::slice::from_ref(&k2));
    done(&mut shell, "strip.move", json!({"step": 1}));
    assert_eq!(strip_texts(&shell, "Terminal")[..4], ["1", "3", "2", "4"]);
    // To the end: :2 would leave bridge group BR1 (1 to 4); terminal.move refuses, naming the group and the way
    // out (CI-06); nothing is committed.
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
    assert_eq!(strip_texts(&shell, "Terminal")[..4], ["1", "3", "2", "4"]);
    done(&mut shell, "strip.move", json!({"before": "1"}));
    assert_eq!(strip_texts(&shell, "Terminal")[..3], ["2", "1", "3"]);
    shell.shutdown();
}

/// SE9's context: every gesture refuses `reason-not-strip-editor` while the Terminal strips navigator is closed.
#[test]
fn gestures_need_the_strip_editor() {
    let tmp = Tmp::new("se9-context");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    for (action, params) in [
        ("strip.insert", json!({})),
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

/// Alt+Up and Alt+Down (`pages.move_up`, `pages.move_down`) run `strip.move` while the
/// Terminal strips navigator has the focus, one undo step each; elsewhere they keep `pages.move_*`.
#[test]
fn alt_arrows_move_strip_rows_while_the_strips_navigator_has_the_focus() {
    let tmp = Tmp::new("se9-alt-arrows");
    let mut shell = editor(&tmp, "-X1021");
    done(
        &mut shell,
        "ui.focus",
        json!({"area": "navigator", "kind": "strips"}),
    );
    assert_eq!(shell.focused_navigator(), Some(NavKind::Strips));
    let k2 = strip_row(&shell, "Terminal", "2");
    mark(&mut shell, std::slice::from_ref(&k2));
    let n = steps(&shell);
    // Alt+Down: the row moves down one place, one step.
    assert_eq!(key(&mut shell, "pages.move_down"), "strip.move");
    assert_eq!(strip_texts(&shell, "Terminal")[..4], ["1", "3", "2", "4"]);
    assert_eq!(steps(&shell), n + 1);
    assert_eq!(last(&shell).0, "terminal.move");
    // Alt+Up: back.
    assert_eq!(key(&mut shell, "pages.move_up"), "strip.move");
    assert_eq!(strip_texts(&shell, "Terminal"), SEQ_1_TO_8);
    assert_eq!(steps(&shell), n + 2);
    // At the first row Alt+Up is refused as `strip.move` is; nothing changes.
    let k1 = strip_row(&shell, "Terminal", "1");
    mark(&mut shell, std::slice::from_ref(&k1));
    let (a, p) = shell.shortcut_action("pages.move_up");
    assert_eq!((a, &p), ("strip.move", &json!({"step": -1})));
    assert_eq!(refused(&mut shell, a, p), "reason-strip-move-none");
    assert_eq!(strip_texts(&shell, "Terminal"), SEQ_1_TO_8);
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
        shell.shortcut_action("strip.insert"),
        ("strip.insert", Value::Null)
    );
    shell.shutdown();
}

/// SE14: the describe lines of the strip editor: the header with `sort "Order"` and the `strip:` filter, an
/// accessory row printed with the row word `accessory` and its cells.
#[test]
fn se14_describe_lines() {
    let tmp = Tmp::new("se14");
    let mut shell = editor(&tmp, "-X1021");
    let k8 = strip_row(&shell, "Terminal", "8");
    strip_at(&mut shell, &k8);
    done(&mut shell, "strip.add_accessory", json!({}));
    let d = shell.describe();
    assert!(
        d.contains(
            "navigator \"Terminal strips\" kind strips view tree table sort \"Order\" ascending filter \"strip:-X1021\" rows 12 showing 1 to 12\n  columns \"Strip\" \"Pos\" \"Terminal\" \"Function text\" \"Group\" \"Part\" \"Potential\" \"Side 1 (clamp a)\" \"Side 2 (clamp b)\" \"Bridge\" \"Bridge part\" \"State\"\n  accessory \"-X1021\" \"\" \"\" \"End bracket\" \"\" \"EW-GEN-EB-1\" \"with EW-GEN-TRM-1\" \"\" \"\" \"\" \"\" \"\"\n  row \"-X1021\" \"1\" \"1\" \"\" \"\" \"EW-GEN-TRM-1\" \"24V/102F1:2\" \"-102F1:2\" \"-102S2:11\" \"BR1\" \"\" \"used\"\n"
        ),
        "{d}"
    );
    assert!(
        d.contains(
            "\n  accessory \"-X1021\" \"8.1\" \"\" \"End bracket\" \"\" \"EW-GEN-EB-1\" \"\" \"\" \"\" \"\" \"\" \"\" cursor \"Strip\"\n"
        ),
        "{d}"
    );
    shell.shutdown();
}

/// An accessory at the start of a new strip without a tag yet reads the strip in words
/// in its toast and in its row's Strip cell, never by the placeholder tag.
#[test]
fn an_untagged_strips_accessory_row_names_it_in_words() {
    let tmp = Tmp::new("strip-untagged-accessory");
    let mut shell = with_accessory_parts(&tmp);
    done(&mut shell, "strip.new", json!({}));
    done(&mut shell, "navigator.show", json!({"kind": "strips"}));
    let key = shell
        .nav(NavKind::Strips)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == ew_viewmodel::navigators::NavRole::Strip && n.label.contains('?'))
                .map(|n| n.key.clone())
        })
        .expect("the new strip");
    mark(&mut shell, std::slice::from_ref(&key));
    done(&mut shell, "strip.add_accessory", json!({}));
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Added end bracket at the start of New terminal strip \u{b7} no tag yet.\" undo"]
    );
    mark(&mut shell, &[]);
    done(&mut shell, "navigator.row", json!({"kind": "strips", "key": key}));
    done(&mut shell, "strip.edit", json!({}));
    let rows = lines_with(&shell, "  accessory ");
    assert_eq!(
        rows,
        [
            "  accessory \"New terminal strip \u{b7} no tag yet\" \"0.1\" \"\" \"End bracket\" \"\" \"EW-GEN-EB-1\" \"\" \"\" \"\" \"\" \
             \"\" \"\" cursor \"Strip\""
        ]
    );
    let d = shell.describe();
    assert!(!d.contains("?X?") && !d.contains("X??"), "{d}");
    shell.shutdown();
}

/// In nl-NL the strip editor's columns, an accessory row's kind and the
/// accessory toast read in Dutch; the strip editor finds its columns and rows by those words.
#[test]
fn the_strip_editor_and_its_accessory_toast_read_in_dutch() {
    let tmp = Tmp::new("ft1289");
    let mut shell = with_accessory_parts(&tmp);
    done(&mut shell, "view.language", json!({"value": "nl"}));
    done(&mut shell, "strip.edit", json!({"strip": "-X1021"}));
    let labels: Vec<String> = shell
        .nav(NavKind::Strips)
        .and_then(|n| n.table())
        .expect("a table")
        .columns
        .iter()
        .map(|c| c.label.clone())
        .collect();
    assert_eq!(
        labels,
        [
            "Klemmenstrook",
            "Pos",
            "Klem",
            "Functietekst",
            "Groep",
            "Artikel",
            "Potentiaal",
            "Zijde 1 (aansluiting a)",
            "Zijde 2 (aansluiting b)",
            "Brug",
            "Brugartikel",
            "Status",
        ]
    );
    let k4 = strip_row(&shell, "Klem", "4");
    strip_at(&mut shell, &k4);
    done(&mut shell, "strip.add_accessory", json!({}));
    let accessory = strip_cursor(&shell);
    assert_eq!(accessory, strip_row(&shell, "Pos", "4.1"));
    let word = strip_column(&shell, "Functietekst")
        .into_iter()
        .find(|(k, _)| *k == accessory)
        .expect("the accessory row")
        .1;
    assert_eq!(word, "Eindsteun");
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Eindsteun toegevoegd na klem 4 van -X1021.\" undo"]
    );
    // The insert and the move name the terminal in words, in Dutch too.
    let k5 = strip_row(&shell, "Klem", "5");
    strip_at(&mut shell, &k5);
    done(&mut shell, "strip.insert", json!({}));
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"1 klem ingevoegd vóór klem 5 van -X1021.\" undo"]
    );
    let k8 = strip_row(&shell, "Klem", "8");
    mark(&mut shell, std::slice::from_ref(&k8));
    done(&mut shell, "strip.move", json!({"step": -1}));
    assert_eq!(
        lines_with(&shell, "toast "),
        ["toast \"Klem 8 van -X1021 verplaatst.\" undo"]
    );
    shell.shutdown();
}

/// A strip gesture's refusal reads in the window's language, the part kind in words.
#[test]
fn a_strip_gestures_refusal_reads_in_dutch() {
    let tmp = Tmp::new("ft1555-strip");
    let mut shell = editor(&tmp, "-X1021");

    let refusal = |shell: &mut ShellModel| match shell.dispatch(
        "strip.add_accessory",
        &json!({"kind": "end_clamp", "part": "4711"}),
    ) {
        Outcome::Failed { message } => message,
        other => panic!("{other:?}"),
    };
    let en = refusal(&mut shell);
    done(&mut shell, "view.language", json!({ "value": "nl" }));
    let nl = refusal(&mut shell);
    done(&mut shell, "view.language", json!({ "value": "en" }));
    shell.shutdown();
    assert!(
        en.contains("the project has no part `4711` of kind end bracket"),
        "{en}"
    );
    assert!(
        nl.contains("het project heeft geen artikel `4711` van soort eindsteun"),
        "{nl}"
    );
}
