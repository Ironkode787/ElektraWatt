// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Delete on a page leaves nothing behind unseen (doc 06 §3.4): the wire drawn to a deleted terminal goes with it, so
//! no wire stays drawn on no page; the toast says the terminal stays in the project and its lists, and its Remove them
//! from the project takes it out in one step.

use crate::common;

use common::{Tmp, done, tables};
use ew_api::output::{NavRole, TableRep};
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

#[test]
fn a_deleted_terminal_takes_its_wire_and_the_toast_offers_to_remove_it() {
    let tmp = Tmp::new("delete-leaves-nothing");
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(&mut s, "tool.place_terminal", json!({"strip": "new", "count": "2"}));
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(30), sn.page_of_index(20)))
    };
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    // The two terminals' upper pins wired together.
    let t = tables(&s);
    let mut reps: Vec<_> = t.reps.iter().filter(|r| r.terminal).cloned().collect();
    reps.sort_by_key(|r| r.pos);
    let upper = |rep: &TableRep| {
        t.cps
            .iter()
            .filter(|c| c.rep == rep.id)
            .min_by_key(|c| c.at.1)
            .cloned()
            .expect("a pin")
    };
    let (a, b) = (upper(&reps[0]), upper(&reps[1]));
    done(
        &mut s,
        "command.run",
        json!({"cmd": "connection.draw", "params": {"page": t.page.to_string(),
            "a": {"cp": {"function": a.function.to_string(), "idx": a.idx}},
            "b": {"cp": {"function": b.function.to_string(), "idx": b.idx}}}}),
    );
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
    assert_eq!(rows(&mut s, NavKind::Strips, NavRole::Terminal), 2);
    done(
        &mut s,
        "notifications.run",
        json!({"entry": toast.serial, "action": "remove_undrawn"}),
    );
    assert_eq!(rows(&mut s, NavKind::Strips, NavRole::Terminal), 1, "removed from the project");
}
