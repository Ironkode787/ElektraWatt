// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire selected on the page marks its row in Connections (FT-1912): every connection row of the sample drawn on
//! pages 102 and 104, its record there selected alone (the selection a click on the wire makes), is marked.

use crate::common::{Tmp, done};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn every_selected_wire_marks_its_connections_row() {
    let tmp = Tmp::new("connections-mark-wire");
    let mut s = tmp.sample_in(Mode::Headless, true, "102");
    done(&mut s, "navigator.show", json!({"kind": "connections"}));
    let rows: Vec<(String, String, Vec<(String, String)>)> = {
        let table = s.nav(NavKind::Connections).and_then(|n| n.table()).expect("its table");
        table
            .nodes
            .iter()
            .filter(|n| n.role == NavRole::Connection)
            .map(|n| {
                let drawn = n
                    .drawables
                    .iter()
                    .map(|d| (d.folio.clone(), d.record.to_string()))
                    .collect();
                (n.key.clone(), n.label.clone(), drawn)
            })
            .collect()
    };
    let mut unmarked = Vec::new();
    for folio in ["102", "104"] {
        done(&mut s, "page.goto", json!({ "folio": folio }));
        for (key, label, drawn) in &rows {
            for (_, record) in drawn.iter().filter(|(f, _)| f == folio) {
                done(&mut s, "select.set", json!({ "ids": [record] }));
                let nav = s.nav(NavKind::Connections).expect("the Connections navigator");
                if !nav.marked().contains(key) {
                    unmarked.push(format!("{folio} {label}: marked {:?}", nav.marked()));
                }
            }
        }
    }
    assert!(unmarked.is_empty(), "{unmarked:#?}");
}
