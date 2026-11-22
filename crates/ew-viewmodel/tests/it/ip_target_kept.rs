// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Interruption point tool keeps the direction a person chose (a persona run chose Target, and after each target
//! the tool turned to Source by itself: every later mark was a source it did not mean): with Target chosen and no
//! source drawn yet, the next points are targets too, and the bar says that the name still has no source and that D
//! makes the point its source. The check then says of each target where it stands and what it lacks, in a
//! draughtsman's words.

use crate::common;

use common::{Tmp, at, done, new_project};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::tools::Ghost;
use serde_json::json;

/// Moves the grid cursor to the grid index `k` of the page on show.
fn cursor_k(s: &mut ShellModel, k: (i64, i64)) {
    let page = std::sync::Arc::clone(s.canvas().unwrap().page());
    let p = (page.snapper.page_of_index(k.0), page.snapper.page_of_index(k.1));
    done(s, "cursor.set", at(page.snapper.page_to_sheet(p), json!({})));
}

fn direction(s: &ShellModel) -> &'static str {
    match s.tool_ghost() {
        Some(Ghost::Ip { direction, .. }) => direction,
        other => panic!("the IP tool's ghost: {other:?}"),
    }
}

#[test]
fn a_chosen_target_stays_and_the_check_says_what_each_target_lacks() {
    let tmp = Tmp::new("ip-target-kept");
    let mut s = new_project(&tmp);
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "ip.name", json!({"text": "24V"}));
    done(&mut s, "tool.ip_direction", json!({"direction": "target"}));
    for (i, k) in [(40, 20), (40, 30), (40, 40)].into_iter().enumerate() {
        cursor_k(&mut s, k);
        assert_eq!(direction(&s), "target", "point {i}: the chosen direction holds");
        if i > 0 {
            let note = s.tool().role_note().unwrap_or_default();
            assert!(note.contains("one more target of 24V, which has no source yet"), "{note}");
        }
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "navigator.show", json!({"kind": "messages"}));
    done(&mut s, "messages.check", json!({}));
    let table = s.nav(NavKind::Messages).and_then(|n| n.table()).cloned().expect("the Messages table");
    let said: Vec<&str> = table
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding && n.cells[1] == "EW-CORE-051")
        .map(|n| n.cells[2].as_str())
        .collect();
    assert!(!said.is_empty(), "the targets are reported");
    for m in said {
        assert!(
            m.starts_with("interruption point 24V at 101-")
                && m.contains("is a target without a source: place where 24V comes from, its source"),
            "{m}"
        );
    }
}
