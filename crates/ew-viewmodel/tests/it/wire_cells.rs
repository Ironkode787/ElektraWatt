// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Connections table's cells of a wire set that wire alone (doc 13 §2.3): a star wire's colour goes to its own
//! carrier, never to the feed arm the star's other wires share; a bridge row's Length cell is no cell of a wire.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::navigators::table::cell_reason;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A writable copy of the sample with the Connections table open.
fn connections(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "navigator.show", json!({"kind": "connections"}));
    done(
        &mut s,
        "navigator.table",
        json!({"kind": "connections", "on": true}),
    );
    s
}

/// The row labelled `row` and the index of the column `key`.
fn at(s: &ShellModel, row: &str, key: &str) -> (usize, usize) {
    let t = s.nav(NavKind::Connections).and_then(|n| n.table()).unwrap();
    let c = t.columns.iter().position(|c| c.key == key).unwrap();
    let n = (t.nodes.iter())
        .position(|n| n.table_row && n.label == row)
        .unwrap_or_else(|| panic!("no row {row}"));
    (n, c)
}

fn cell(s: &ShellModel, row: &str, key: &str) -> String {
    let (n, c) = at(s, row, key);
    let t = s.nav(NavKind::Connections).and_then(|n| n.table()).unwrap();
    t.nodes[n].cells[c].clone()
}

/// 101.4, 101.5 and 101.6 leave -X1011:PE through one star, and 101.6 lands on -X1031:PE through the chain 103.7
/// leaves it by: a colour typed for 101.6 is set on one connection of 101.6, the first record of its row, that no other
/// wire stands for, so neither the star's feed arm nor the chain's arm.
#[test]
fn a_star_wires_colour_reaches_that_wire_alone() {
    let tmp = Tmp::new("star-colour");
    let mut s = connections(&tmp);
    let (own, others) = {
        let t = s.nav(NavKind::Connections).and_then(|n| n.table()).unwrap();
        let (n, _) = at(&s, "101.6", "colour");
        let others: Vec<String> = (t.nodes.iter().enumerate())
            .filter(|(i, m)| m.table_row && *i != n)
            .flat_map(|(_, m)| m.records.iter().map(ToString::to_string))
            .collect();
        let own: Vec<String> = t.nodes[n].records.iter().map(ToString::to_string).collect();
        (own, others)
    };
    assert!(own.iter().any(|r| others.contains(r)), "the star's shared arm");
    done(
        &mut s,
        "table.set",
        json!({"kind": "connections", "column": "colour", "value": "RD", "rows": ["101.6"]}),
    );
    let set = s
        .recorded_commands()
        .last()
        .expect("a committed step")
        .params
        .clone();
    assert_eq!(set["targets"], json!([own[0]]), "{set}");
    assert!(!others.contains(&own[0]), "{set}");
    assert_eq!(cell(&s, "101.6", "colour"), "RD");
}

/// A bridge has no length: the Length cell of a bridge row is read-only, a wire's is not.
#[test]
fn a_bridge_rows_length_cell_is_read_only() {
    let tmp = Tmp::new("bridge-length");
    let s = connections(&tmp);
    let t = s.nav(NavKind::Connections).and_then(|n| n.table()).unwrap();
    let reason = |row: &str| {
        let (n, c) = at(&s, row, "length");
        cell_reason(t, &t.nodes[n], c)
    };
    assert!(reason("BR1").is_some(), "BR1's length is editable");
    assert_eq!(reason("101.1"), None);
}

/// A wire's To end typed in its cell is set by hand on the connection that lands on it, in the words of the window.
#[test]
fn a_typed_end_is_set_by_hand_on_the_connection_that_lands_on_it() {
    let tmp = Tmp::new("end-by-hand");
    let mut s = connections(&tmp);
    done(
        &mut s,
        "table.set",
        json!({"kind": "connections", "column": "to_end", "value": "Bare", "rows": ["101.1"]}),
    );
    let set = s
        .recorded_commands()
        .last()
        .expect("a committed step")
        .params
        .clone();
    assert_eq!(set["kind"], json!("connection"), "{set}");
    assert!(
        matches!(set["path"].as_str(), Some("overrides.end_a" | "overrides.end_b")),
        "{set}"
    );
    assert_eq!(set["value"], json!({"treatment": "bare"}), "{set}");
    assert_eq!(cell(&s, "101.1", "to_end"), "bare");
}
