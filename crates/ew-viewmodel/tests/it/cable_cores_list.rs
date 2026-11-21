// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cable.from_wires` takes its `cores` as a list of core names in the wires' order, as it takes them as a text
//! (FT-1865): a script that gives a list gets those cores, not the ones the form proposes.

use crate::common;

use common::{Tmp, done, form, select_ids, tables};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The form's rows of the selected wires: each wire in words with its core.
fn rows(s: &mut ShellModel) -> Vec<(String, String)> {
    done(s, "cable.from_wires", json!({}));
    let f = form(s);
    done(s, "ui.dismiss", json!({}));
    f.table
        .unwrap()
        .rows
        .into_iter()
        .map(|r| (r.key, r.cells[0].clone()))
        .collect()
}

#[test]
fn a_list_of_cores_is_taken_in_the_wires_order() {
    let tmp = Tmp::new("cores-list");
    let mut s = tmp.sample_in(Mode::Gui, true, "101");
    let wires: Vec<String> = tables(&s)
        .connections
        .iter()
        .filter(|c| c.kind == "wire")
        .take(2)
        .map(|c| c.id.to_string())
        .collect();
    assert_eq!(wires.len(), 2, "two wires on the page");
    select_ids(&mut s, &wires);
    let proposed = rows(&mut s);
    let type_text = {
        done(&mut s, "cable.from_wires", json!({}));
        let f = form(&s);
        done(&mut s, "ui.dismiss", json!({}));
        f.fields.iter().find(|f| f.name == "type").unwrap().value.clone()
    };
    // The proposed cores the other way round.
    let swapped: Vec<String> = proposed.iter().rev().map(|(_, core)| core.clone()).collect();
    assert_ne!(proposed[0].1, proposed[1].1, "two distinct cores: {proposed:?}");

    done(&mut s, "cable.from_wires", json!({"type": type_text, "cores": swapped}));

    select_ids(&mut s, &wires);
    let made = rows(&mut s);
    let expected: Vec<(String, String)> = proposed
        .iter()
        .zip(&swapped)
        .map(|((wire, _), core)| (wire.clone(), core.clone()))
        .collect();
    let mut made_sorted = made.clone();
    made_sorted.sort();
    let mut expected_sorted = expected;
    expected_sorted.sort();
    assert_eq!(made_sorted, expected_sorted, "the cores given, wire by wire");
}
