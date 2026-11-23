// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Delete and Cut on a page leave nothing behind unseen (doc 06 §3.4, §3.6): the wire drawn to a deleted or cut
//! terminal goes with it, so no wire stays drawn on no page, and the toast says so. A delete's toast says the terminal
//! stays in the project and its lists, and its Remove them from the project takes it out in one step; a cut's says
//! what it took, and its paste brings back the wires between the cut terminals only.

use crate::common;

use common::{Tmp, done, tables};
use ew_api::output::{NavRole, TableCp, TableRep};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::notifications::NoteAction;
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// The rows of role `role` in navigator `kind`'s table.
fn rows(s: &mut ShellModel, kind: NavKind, role: NavRole) -> usize {
    done(s, "navigator.show", json!({ "kind": kind.word() }));
    s.nav(kind)
        .and_then(|n| n.table())
        .map_or(0, |t| t.nodes.iter().filter(|n| n.role == role).count())
}

/// A new project with `count` terminals of a new strip placed on its first page, their symbols from left to right.
fn terminals(tmp: &Tmp, count: usize) -> (ShellModel, Vec<TableRep>) {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "tool.place_terminal",
        json!({"strip": "new", "count": count.to_string()}),
    );
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(30), sn.page_of_index(20)))
    };
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    let mut reps: Vec<TableRep> = tables(&s).reps.iter().filter(|r| r.terminal).cloned().collect();
    reps.sort_by_key(|r| r.pos);
    (s, reps)
}

/// The upper (`upper`) or lower pin of the terminal symbol `rep`.
fn pin(s: &ShellModel, rep: &TableRep, upper: bool) -> TableCp {
    let mut cps: Vec<TableCp> = tables(s)
        .cps
        .iter()
        .filter(|c| c.rep == rep.id)
        .cloned()
        .collect();
    cps.sort_by_key(|c| c.at.1);
    if upper {
        cps.remove(0)
    } else {
        cps.pop().expect("a pin")
    }
}

/// A wire drawn from pin `a` to pin `b` on the page shown.
fn wire(s: &mut ShellModel, a: &TableCp, b: &TableCp) {
    let page = tables(s).page.to_string();
    done(
        s,
        "command.run",
        json!({"cmd": "connection.draw", "params": {"page": page,
            "a": {"cp": {"function": a.function.to_string(), "idx": a.idx}},
            "b": {"cp": {"function": b.function.to_string(), "idx": b.idx}}}}),
    );
}

#[test]
fn a_deleted_terminal_takes_its_wire_and_the_toast_offers_to_remove_it() {
    let tmp = Tmp::new("delete-leaves-nothing");
    let (mut s, reps) = terminals(&tmp, 2);
    let (a, b) = (pin(&s, &reps[0], true), pin(&s, &reps[1], true));
    wire(&mut s, &a, &b);
    assert_eq!(rows(&mut s, NavKind::Connections, NavRole::Connection), 1);
    done(&mut s, "select.set", json!({"ids": [reps[1].id.to_string()]}));
    done(&mut s, "edit.delete_selection", json!({}));
    assert_eq!(
        rows(&mut s, NavKind::Connections, NavRole::Connection),
        0,
        "no wire stays drawn on no page"
    );
    let toast = s.toast().cloned().expect("the delete's toast");
    assert_eq!(
        toast.text,
        "1 wire that ended at the deleted symbols was deleted with them. \
         1 terminal is no longer drawn; it stays in the project and its lists."
    );
    assert_eq!(toast.actions(), [NoteAction::Undo, NoteAction::RemoveUndrawn]);
    assert!(
        toast.stays(),
        "a consequence the person may not want stays on screen"
    );
    assert_eq!(rows(&mut s, NavKind::Strips, NavRole::Terminal), 2);
    done(
        &mut s,
        "notifications.run",
        json!({"entry": toast.serial, "action": "remove_undrawn"}),
    );
    assert_eq!(
        rows(&mut s, NavKind::Strips, NavRole::Terminal),
        1,
        "removed from the project"
    );
}

/// In the Terminal strips table a row chosen whole (the cursor in its key column, Strip) is deleted by Delete, its
/// terminal with its symbol and wires, with the page Delete's toast; a cell elsewhere is cleared (`tb5_the_policy`).
#[test]
fn delete_in_the_strips_table_deletes_whole_rows() {
    let tmp = Tmp::new("delete-table-rows");
    let (mut s, reps) = terminals(&tmp, 3);
    let (a, b) = (pin(&s, &reps[0], true), pin(&s, &reps[1], true));
    wire(&mut s, &a, &b);
    done(&mut s, "select.set", json!({"ids": []}));
    done(&mut s, "navigator.show", json!({"kind": "strips"}));
    done(&mut s, "navigator.table", json!({"kind": "strips", "on": true}));
    let second = reps[1].device.expect("its terminal");
    let row = s
        .nav(NavKind::Strips)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.key.ends_with(&format!("terminal:{second}")))
                .map(|n| n.key.clone())
        })
        .expect("the second terminal's row");
    done(&mut s, "table.cell", json!({"row": row, "column": "strip"}));
    done(&mut s, "table.clear", json!({}));
    assert_eq!(
        s.toast().map(|t| t.text.as_str()),
        Some("1 wire that ended at the deleted symbols was deleted with them.")
    );
    assert_eq!(
        tables(&s).reps.iter().filter(|r| r.terminal).count(),
        2,
        "the terminal is deleted with its symbol"
    );
    assert_eq!(
        rows(&mut s, NavKind::Strips, NavRole::Terminal),
        2,
        "and from the strip"
    );
    assert_eq!(rows(&mut s, NavKind::Connections, NavRole::Connection), 0);
}

#[test]
fn a_cut_says_what_it_took_and_its_paste_keeps_the_wires_between_the_cut_terminals() {
    let tmp = Tmp::new("cut-says-what");
    let (mut s, reps) = terminals(&tmp, 3);
    // One wire between the two terminals cut, one from the second to the third, which stays.
    let (a, b) = (pin(&s, &reps[0], true), pin(&s, &reps[1], true));
    wire(&mut s, &a, &b);
    let (b, c) = (pin(&s, &reps[1], false), pin(&s, &reps[2], false));
    wire(&mut s, &b, &c);
    let ids: Vec<String> = reps[..2].iter().map(|r| r.id.to_string()).collect();
    done(&mut s, "select.set", json!({ "ids": ids }));
    done(&mut s, "edit.cut", json!({}));
    let toast = s.toast().cloned().expect("the cut's toast");
    assert_eq!(
        toast.text,
        "Cut 2 terminals. 2 wires that ended at them are removed with them; \
         a paste keeps the wires between the cut items only."
    );
    assert_eq!(
        toast.actions(),
        [NoteAction::Undo],
        "nothing to remove: the paste brings them back"
    );
    assert_eq!(rows(&mut s, NavKind::Connections, NavRole::Connection), 0);
    // The wires it removed are a consequence the person may not want: the toast stays until the next commit.
    assert!(toast.stays());
    done(&mut s, "select.set", json!({ "ids": [reps[2].id.to_string()] }));
    done(&mut s, "tool.rotate_cw", json!({}));
    assert!(
        s.notifications()
            .visible()
            .iter()
            .all(|n| n.serial != toast.serial),
        "gone with the next commit"
    );
    done(&mut s, "edit.paste_in_place", json!({}));
    assert_eq!(
        rows(&mut s, NavKind::Connections, NavRole::Connection),
        1,
        "the wire between the cut terminals comes back"
    );
}
