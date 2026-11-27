// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock (doc 07 §2.5): a new project starts with the starter's ferrules and markers; `workshop.stock`
//! embeds what it carries once and refuses a part that is no wire, ferrule or marker; a wire without a part takes the
//! first stock wire part of its size and colour (`wire_part_of`); a wire part of another size is EW-MFG-022; Add the
//! generic stock adds the starter's ferrules and markers a stock lacks.

use crate::common;

use ew_api::{AnyCommand, Session};
use ew_derive::{PhysicalWiresOf, Q, WirePropsOf, wire_part_of};
use ew_ids::Id;
use ew_schema::{Csa, LibCopy, Part, PartKind};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) -> Result<ew_api::Receipt, ew_api::CmdError> {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
}

/// The first wire of the sample with a size in mm² and a colour: its carrier connection, size and colour.
fn a_wire(q: &Q) -> (ew_derive::PotentialId, Id<ew_schema::Connection>, String, Value) {
    let partition = q.get::<ew_derive::PotentialPartition>(());
    for p in partition.potentials().map(|p| p.id) {
        let props = q.get::<WirePropsOf>(p);
        for w in q.get::<PhysicalWiresOf>(p).wires.iter() {
            let Some(e) = props.wires.get(&w.id) else { continue };
            if let (Some(Csa::Mm2(csa)), Some(colour)) = (e.csa.as_ref().map(|c| &c.value), &e.colour) {
                return (
                    p,
                    w.id,
                    csa.to_string(),
                    serde_json::to_value(&colour.value).unwrap(),
                );
            }
        }
    }
    panic!("the sample has a wire with a size and a colour")
}

/// A new project's stock is the starter's ferrules and markers, in natural part-number order, each held.
#[test]
fn a_new_project_starts_with_the_generic_stock() {
    let s = common::new_project(11);
    let q = s.query();
    let numbers: Vec<(String, PartKind)> = q
        .project()
        .settings
        .workshop
        .stock
        .iter()
        .map(|r| {
            let c: &LibCopy<Part> = q.parts().get(r.copy).expect("a held stock part");
            (c.item.part_number.as_str().to_owned(), c.item.kind.clone())
        })
        .collect();
    assert_eq!(numbers.first().map(|n| n.0.as_str()), Some("EW-GEN-FER-0.5"));
    assert!(numbers.iter().any(|n| n.0 == "EW-GEN-FER-2X1.5"));
    assert!(numbers.iter().any(|n| n.0 == "EW-GEN-MRK-2"));
    assert!(
        numbers
            .iter()
            .all(|n| matches!(n.1, PartKind::Ferrule | PartKind::Marker | PartKind::Gland)),
        "{numbers:?}"
    );
}

/// A wire takes the first stock wire part of its size and colour; the stock command embeds its copy once and refuses
/// a part that is no wire, ferrule or marker; a wire part of another size given to the wire is EW-MFG-022.
#[test]
fn a_wire_takes_its_stock_part_and_a_part_of_another_size_is_reported() {
    let mut s = common::sample();
    let (p, wire, csa, colour) = a_wire(&s.query());
    let (other, other_doc) = common::wire_part(0x7101, "WIRE-OTHER", "16", &colour);
    let (fits, fits_doc) = common::wire_part(0x7102, "WIRE-FITS", &csa, &colour);
    let stock = json!([{"copy": other.to_string()}, {"copy": fits.to_string()}]);
    run(
        &mut s,
        "workshop.stock",
        json!({"stock": stock, "copies": [other_doc.clone(), fits_doc.clone()]}),
    )
    .expect("workshop.stock");
    let held = |s: &Session| s.query().parts().iter().filter(|(id, _)| *id == fits).count();
    assert_eq!(held(&s), 1);
    {
        let q = s.query();
        let props = q.get::<WirePropsOf>(p);
        assert_eq!(
            wire_part_of(&q, props.wires.get(&wire).expect("its props")).map(|r| r.copy),
            Some(fits)
        );
        // The wire's record, which the wire list, its exports and the SQL view read, names the same part.
        let records = q.get::<ew_derive::WireRecordSet>(());
        let record = records.iter().find(|r| r.wire.id == wire).expect("its record");
        assert_eq!(record.wire_part.as_ref().map(|r| r.copy), Some(fits));
    }
    // The same copies again: reused, not embedded twice; the stock in another order.
    let again = json!([{"copy": fits.to_string()}, {"copy": other.to_string()}]);
    run(
        &mut s,
        "workshop.stock",
        json!({"stock": again, "copies": [other_doc, fits_doc]}),
    )
    .expect("again");
    assert_eq!(held(&s), 1);
    // A part that is no wire, ferrule or marker.
    let breaker = s
        .query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "EW-GEN-MCB-1")
        .map(|(id, _)| id)
        .expect("the breaker");
    let refused = run(
        &mut s,
        "workshop.stock",
        json!({"stock": [{"copy": breaker.to_string()}]}),
    )
    .unwrap_err();
    assert!(
        refused
            .to_string()
            .contains("is no wire, ferrule, marker or cable gland"),
        "{refused}"
    );
    // The wire given the 16 mm² part.
    let none = |s: &Session| {
        let q = s.query();
        ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online)
            .findings
            .into_iter()
            .filter(|f| f.rule == "EW-MFG-022")
            .count()
    };
    assert_eq!(none(&s), 0, "silent on the sample");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "connection", "targets": [wire.to_string()], "path": "overrides.wire_part",
               "value": {"copy": other.to_string()}}),
    )
    .expect("the wire's part");
    assert_eq!(none(&s), 1);
    // The wire's size is a measured value (`DECIMAL`): Dutch writes its comma where the project sets no separator, and
    // a project that sets the comma has it in English too (the sample sets the point).
    assert!(
        csa.contains('.'),
        "the sample's wire has a size with decimals: {csa}"
    );
    let q = s.query();
    let mut found = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online)
        .findings
        .into_iter()
        .find(|f| f.rule == "EW-MFG-022")
        .expect("the finding");
    let comma = format!("{} mm²", csa.replace('.', ","));
    found.decimal_comma = None;
    let dutch = ew_checks::messages::render(&found, "nl-NL");
    assert!(dutch.contains(&comma), "{dutch}");
    found.decimal_comma = Some(true);
    let english = ew_checks::messages::render(&found, "en-GB");
    assert!(english.contains(&comma), "{english}");
}

/// What the cut list cuts to: a length sets it, a command without it keeps it, and `null` (the stock dialog's empty
/// Cut to) clears it, so the 5 mm of an unset increment holds again.
#[test]
fn an_empty_cut_to_clears_the_cut_increment() {
    let mut s = common::sample();
    let stock = serde_json::to_value(&s.query().project().settings.workshop.stock).unwrap();
    let cut = |s: &Session| s.query().project().settings.workshop.cut_increment;
    run(
        &mut s,
        "workshop.stock",
        json!({"stock": stock, "cut_increment": 10_000_000}),
    )
    .expect("cut to 10 mm");
    assert_eq!(cut(&s), Some(ew_units::Nm::from_mm(10)));
    run(&mut s, "workshop.stock", json!({"stock": [], "copies": []}))
        .expect("a stock without the cut increment");
    assert_eq!(cut(&s), Some(ew_units::Nm::from_mm(10)), "absent: it stays");
    run(
        &mut s,
        "workshop.stock",
        json!({"stock": stock, "cut_increment": null}),
    )
    .expect("Cut to emptied");
    assert_eq!(cut(&s), None);
}

/// Add the generic stock: a project without stock gets the starter's ferrules and markers; a project that holds them
/// all is offered nothing; one of its own stock parts stays first.
#[test]
fn add_the_generic_stock_adds_what_the_stock_lacks_after_its_own() {
    let mut s = common::sample();
    assert!(
        s.add_generic_stock_command().is_none(),
        "the sample holds the generic stock"
    );
    let (_, _, _, colour) = a_wire(&s.query());
    let (own, own_doc) = common::wire_part(0x7103, "WIRE-OWN", "1.5", &colour);
    run(
        &mut s,
        "workshop.stock",
        json!({"stock": [{"copy": own.to_string()}], "copies": [own_doc]}),
    )
    .expect("a stock of one wire part");
    let cmd = s
        .add_generic_stock_command()
        .expect("the generic stock is missing");
    run(&mut s, &cmd.name, cmd.params).expect("add the generic stock");
    let q = s.query();
    let stock = &q.project().settings.workshop.stock;
    assert_eq!(stock.first().map(|r| r.copy), Some(own));
    let number = |i: usize| {
        let c: &LibCopy<Part> = q.parts().get(stock[i].copy).expect("a held stock part");
        c.item.part_number.as_str().to_owned()
    };
    assert_eq!(number(1), "EW-GEN-FER-0.5");
    drop(q);
    assert!(s.add_generic_stock_command().is_none());
}
