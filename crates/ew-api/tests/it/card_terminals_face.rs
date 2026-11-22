// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `terminal.face_card` (docs/design/06a-drafting.md §4.8, doc 10 §3.6, FT-2083): a terminal between a card and its
//! field device takes the card's wire on its pin that faces the card, the field's on the other, for the terminals of a
//! card or of a strip, in one undo step.

use crate::common;

use ew_api::output::{PageTables, TableCp};
use ew_api::{AnyCommand, CmdError, Origin, Session};
use ew_ids::Uuid;
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) -> Result<(), CmdError> {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .map(|_| ())
}

/// The CP labelled `label` on folio 104.
fn cp<'a>(t: &'a PageTables, label: &str) -> &'a TableCp {
    t.cps
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("{label} on 104"))
}

/// The sample's folio 104 with terminal 71 of strip -X1031 between the input card -104A1-A1 and its field device
/// -104B1, wired the wrong way round: the card's channel pin 2, which faces up, to the terminal's upper pin, and
/// the terminal's lower pin to the sensor above it. With the terminal's function.
fn wired_wrong() -> (Session, Uuid) {
    let mut s = common::sample();
    let t = s.page_tables("104").expect("folio 104");
    let page = t.page.to_string();
    let (card, sensor) = (cp(&t, "-104A1-A1:2").clone(), cp(&t, "-104B1:14").clone());
    let wire = card
        .connections
        .iter()
        .find(|c| sensor.connections.contains(c))
        .expect("the sensor wired to the card");
    run(
        &mut s,
        "connection.delete",
        json!({"connections": [wire.to_string()]}),
    )
    .expect("deleted");
    if let Some(embed) = s.starter_embed("ew.iec.terminal@qet10") {
        run(&mut s, "library.embed", embed).expect("embedded");
    }
    let symbol = s
        .starter_symbol_copy("ew.iec.terminal@qet10")
        .expect("the starter terminal");
    let strip = s.resolve_reference("tag:-X1031").expect("strip -X1031");
    let before: Vec<Uuid> = s.query().functions().ids().map(|f| f.as_uuid()).collect();
    // Between the sensor's lower pin (row 49 of the QET grid) and the card's channel pin (row 57), in their column.
    let row53 = (53 * 7_937_500 + 1) / 3;
    run(
        &mut s,
        "device.place",
        json!({"page": page, "symbol": symbol.to_string(), "pos": [card.at.0, row53],
            "target": {"new_terminal": {"strip": strip.to_string(), "designation": "71"}}}),
    )
    .expect("placed");
    let terminal = s
        .query()
        .functions()
        .ids()
        .map(|f| f.as_uuid())
        .find(|f| !before.contains(f))
        .expect("the terminal's function");
    let (upper, lower) = pins(&s, terminal);
    let end = |c: &TableCp| json!({"cp": {"function": c.function.to_string(), "idx": c.idx}});
    for (a, b) in [(end(&card), end(&upper)), (end(&lower), end(&sensor))] {
        run(&mut s, "connection.draw", json!({"a": a, "b": b, "page": page})).expect("drawn");
    }
    (s, terminal)
}

/// The terminal's upper and lower pin on 104.
fn pins(s: &Session, terminal: Uuid) -> (TableCp, TableCp) {
    let t = s.page_tables("104").expect("folio 104");
    let mut cps: Vec<TableCp> = t.cps.iter().filter(|c| c.function == terminal).cloned().collect();
    cps.sort_by_key(|c| c.at.1);
    assert_eq!(cps.len(), 2, "a terminal of two pins");
    (cps[0].clone(), cps[1].clone())
}

/// Whether the card's channel pin 2 is wired to the terminal's lower pin and the sensor to its upper pin.
fn faces(s: &Session, terminal: Uuid) -> bool {
    let t = s.page_tables("104").expect("folio 104");
    let (upper, lower) = pins(s, terminal);
    let joined = |a: &TableCp, b: &TableCp| a.connections.iter().any(|c| b.connections.contains(c));
    joined(cp(&t, "-104A1-A1:2"), &lower) && joined(cp(&t, "-104B1:14"), &upper)
}

#[test]
fn the_card_s_wire_goes_to_the_terminal_s_pin_that_faces_the_card() {
    for target in ["card", "strip"] {
        let (mut s, terminal) = wired_wrong();
        assert!(!faces(&s, terminal), "wired the wrong way round");
        let device = match target {
            "card" => s.resolve_reference("tag:-104A1-A1"),
            _ => s.resolve_reference("tag:-X1031"),
        }
        .expect("the device");
        let params = json!({ target: device.to_string() });
        run(&mut s, "terminal.face_card", params.clone()).expect("faced");
        assert!(faces(&s, terminal), "by the {target}");
        // Nothing left to change: refused in words.
        let again = run(&mut s, "terminal.face_card", params).expect_err("nothing to do");
        assert!(again.to_string().contains("faces the card already"), "{again}");
        // One undo step.
        s.undo(&Origin::Gui).expect("undo");
        assert!(!faces(&s, terminal));
    }
}
