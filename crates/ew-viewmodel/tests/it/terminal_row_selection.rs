// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! After a Terminal… row the selection is the row's terminals and nothing else: every terminal of the
//! page selected, "15 selected", where a Delete would have removed the strip already drawn.

use crate::common;

use common::{Tmp, done, tables};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// Terminal… with `params`, then a click at grid index `k`.
fn row(s: &mut ShellModel, params: serde_json::Value, k: (i64, i64)) {
    done(s, "tool.place_terminal", params);
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
    };
    common::click(s, p, json!({"alt": true}));
}

#[test]
fn after_a_row_only_its_terminals_are_selected() {
    let tmp = Tmp::new("terminal-row-selection");
    let mut s = tmp.empty_shell(Mode::Headless);
    crate::common::make_project(&mut s, &tmp.0.join("new.ew"), &[]);
    row(&mut s, json!({"strip": "new", "count": "4"}), (20, 10));
    let first: Vec<_> = tables(&s).reps.iter().map(|r| r.id).collect();
    done(&mut s, "select.set", json!({"ids": []}));
    // A second strip's row of three, lower down.
    row(&mut s, json!({"strip": "new", "count": "3"}), (20, 30));
    let laid: Vec<_> = tables(&s)
        .reps
        .iter()
        .filter(|r| !first.contains(&r.id))
        .map(|r| r.id)
        .collect();
    assert_eq!(laid.len(), 3);
    let selected = |s: &ShellModel| {
        let mut v: Vec<_> = s.selection().items.iter().map(|e| e.0).collect();
        v.sort();
        v
    };
    let mut want = laid.clone();
    want.sort();
    assert_eq!(selected(&s), want, "the row's terminals");
    // And still after the next action.
    done(&mut s, "view.zoom_actual", json!({}));
    assert_eq!(selected(&s), want, "nothing else joins the selection");
}
