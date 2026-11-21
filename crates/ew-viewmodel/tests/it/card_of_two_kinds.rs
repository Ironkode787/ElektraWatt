// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A card of two channel kinds (FT-1766, doc 10 §2.6): a part made in the part editor with four inputs and four
//! outputs is one card, its outputs numbered on after its inputs and each kind addressed from bit 0 of its own range;
//! a second kind that goes the same way as the first is refused in words at its field.

use crate::common::{Tmp, done, new_project};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Dialog, ShellModel};
use serde_json::json;

/// A new module part with the PLC fields `fields`, its identity Example `number`.
fn card(s: &mut ShellModel, number: &str, fields: &[(&str, &str)]) {
    done(s, "part.new", json!({"plc": "module"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", number)]
        .iter()
        .chain(fields)
    {
        done(s, "part.field", json!({"field": field, "value": value}));
    }
}

/// The addresses the PLC grid lists.
fn addresses(s: &ShellModel) -> Vec<String> {
    let n = s.nav(NavKind::Plc).expect("the PLC navigator");
    let t = n.table().expect("its table");
    let column = t
        .columns
        .iter()
        .position(|c| c.key == "address")
        .expect("the address column");
    t.nodes
        .iter()
        .filter_map(|node| node.cells.get(column).cloned())
        .filter(|a| !a.is_empty())
        .collect()
}

#[test]
fn a_card_of_inputs_and_outputs_is_one_part() {
    let tmp = Tmp::new("card-of-two-kinds");
    let mut s = new_project(&tmp);
    // The same way twice is refused at the second kind.
    card(
        &mut s,
        "DI-DI",
        &[
            ("plc.kind", "di"),
            ("plc.count", "4"),
            ("plc.kind2", "di"),
            ("plc.count2", "4"),
        ],
    );
    let _ = s.dispatch("part.save", &json!({}));
    match s.dialog() {
        Some(Dialog::PartEdit(e)) => assert_eq!(
            e.draft.message("plc.kind2").map(|m| m.key),
            Some("error-part-plc-kind2-way")
        ),
        other => panic!("the part editor stays open: {other:?}"),
    }
    done(&mut s, "ui.dismiss", json!({}));
    // Four inputs and four outputs.
    card(
        &mut s,
        "DIO-8",
        &[
            ("plc.kind", "di"),
            ("plc.count", "4"),
            ("plc.kind2", "do"),
            ("plc.count2", "4"),
            ("plc.pin_of", "{n+1}"),
        ],
    );
    done(&mut s, "part.save", json!({}));
    assert!(s.dialog().is_none(), "saved: {:?}", s.dialog());
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(&mut s, "plc.rack_add", json!({"part": "DIO-8"}));
    done(&mut s, "plc.rack_create", json!({}));
    done(&mut s, "navigator.show", json!({"kind": "plc"}));
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    let all = addresses(&s);
    let bits = |area: char| -> Vec<String> {
        let mut b: Vec<String> = all
            .iter()
            .filter(|a| a.starts_with(area))
            .filter_map(|a| a.rsplit_once('.').map(|(_, bit)| bit.to_owned()))
            .collect();
        b.sort();
        b
    };
    assert_eq!(bits('I'), ["0", "1", "2", "3"], "the inputs: {all:?}");
    assert_eq!(
        bits('Q'),
        ["0", "1", "2", "3"],
        "the outputs from bit 0 of their range: {all:?}"
    );
}
