// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A card of two channel kinds (doc 10 §2.6): a part made in the part editor with four inputs and four
//! outputs is one card, its outputs numbered on after its inputs and each kind addressed from bit 0 of its own range;
//! the first kind again is refused in words at its field. A card of inputs and analogue inputs is one card too:
//! the analogue words follow the inputs' bits in the one input range, from the next word.

use crate::common::{Tmp, done, new_project};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// A new module part with the PLC fields `fields`, its identity Example `number`.
fn card(s: &mut ShellModel, number: &str, fields: &[(&str, &str)]) {
    done(s, "parts.new", json!({"plc": "module"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", number)]
        .iter()
        .chain(fields)
    {
        done(s, "parts.field", json!({"field": field, "value": value}));
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
    // The first kind again is refused at the second kind.
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
    let _ = s.dispatch("parts.save", &json!({}));
    match s.parts_place().and_then(|p| p.sheet.as_ref()) {
        Some(e) => assert_eq!(
            e.sheet.message("plc.kind2").map(|m| m.key),
            Some("error-part-plc-kind2-way")
        ),
        None => panic!("the part stays open in Parts"),
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
    done(&mut s, "parts.save", json!({}));
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

/// four inputs and two analogue inputs are one card; its input range holds the four bits from bit 0 and the
/// two words from the next word on (doc 10 §2.6).
#[test]
fn a_card_of_inputs_and_analogue_inputs_is_one_part() {
    let tmp = Tmp::new("card-of-two-kinds-one-way");
    let mut s = new_project(&tmp);
    card(
        &mut s,
        "DI-AI",
        &[
            ("plc.kind", "di"),
            ("plc.count", "4"),
            ("plc.kind2", "ai"),
            ("plc.count2", "2"),
            ("plc.pin_of", "{n+1}"),
        ],
    );
    done(&mut s, "parts.save", json!({}));
    assert!(s.dialog().is_none(), "saved: {:?}", s.dialog());
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(&mut s, "plc.rack_add", json!({"part": "DI-AI"}));
    done(&mut s, "plc.rack_create", json!({}));
    done(&mut s, "navigator.show", json!({"kind": "plc"}));
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    let all = addresses(&s);
    let digital: Vec<(u32, String)> = all
        .iter()
        .filter_map(|a| a.strip_prefix('I')?.split_once('.'))
        .filter_map(|(byte, bit)| Some((byte.parse().ok()?, bit.to_owned())))
        .collect();
    let byte = digital.first().map(|(b, _)| *b).expect("an input");
    let mut bits: Vec<&str> = digital.iter().map(|(_, bit)| bit.as_str()).collect();
    bits.sort_unstable();
    assert_eq!(bits, ["0", "1", "2", "3"], "the inputs: {all:?}");
    let mut words: Vec<u32> = all
        .iter()
        .filter_map(|a| a.strip_prefix("IW")?.parse().ok())
        .collect();
    words.sort_unstable();
    assert_eq!(
        words,
        [byte + 2, byte + 4],
        "the analogue words after the inputs' word: {all:?}"
    );
}
