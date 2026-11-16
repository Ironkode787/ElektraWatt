// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts list says what it leaves out (rc.2 persona run, entry 31), on a copy of Golden A: Output ▸ Parts list
//! opens the list and a toast says how many devices have no part and are not listed; its Select them selects exactly
//! the devices that the check's "Device without part" group names, so Edit ▸ Choose part… gives them one.

use crate::common;

use std::collections::BTreeSet;

use common::{Tmp, done};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::notifications::NoteAction;
use serde_json::json;

#[test]
fn the_parts_list_says_which_devices_it_leaves_out() {
    let tmp = Tmp::new("parts-unlisted");
    let mut s = tmp.shell("101");
    done(&mut s, "messages.show", json!({"on": true}));
    let named: BTreeSet<String> = s
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .expect("the Messages table")
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding && n.cells[1] == "EW-CORE-041")
        .flat_map(|n| n.records.iter().map(ToString::to_string))
        .collect();
    assert!(!named.is_empty(), "Golden A has devices without a part");

    done(&mut s, "lists.parts", json!({}));
    let toast = s.toast().cloned().expect("a toast");
    assert_eq!(toast.action(), Some(NoteAction::SelectUnlisted));
    assert_eq!(
        toast.text,
        format!(
            "{} devices have no part and are not in the parts list. Select them, then Edit ▸ Choose part… gives \
             them one.",
            named.len()
        )
    );
    done(
        &mut s,
        "notifications.run",
        json!({"entry": toast.serial, "action": "select_unlisted"}),
    );
    let selected: BTreeSet<String> = s.selection().items.iter().map(|e| e.0.to_string()).collect();
    assert_eq!(selected, named);
}
