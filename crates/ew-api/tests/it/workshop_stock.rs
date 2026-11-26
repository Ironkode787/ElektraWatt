// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock (doc 07 §2.5): a new project starts with the starter's ferrules and markers; `workshop.stock`
//! embeds what it carries once and refuses a part that is no wire, ferrule or marker; a wire without a part takes the
//! first stock wire part of its size and colour (`wire_part_of`); a wire part of another size is EW-MFG-022; Add the
//! generic stock adds the starter's ferrules and markers a stock lacks.

use crate::common;

use ew_api::{AnyCommand, Session};
use ew_derive::{PhysicalWiresOf, Q, WirePropsOf, wire_part_of};
use ew_ids::{Id, Uuid};
use ew_schema::{Csa, LibCopy, LibDocument, Part, PartKind};
use serde_json::{Value, json};

/// A wire part copy of a library of its own: `csa` mm² in `colour`, numbered `number`.
fn wire_part(n: u128, number: &str, csa: &str, colour: &Value) -> (Id<Part>, Value) {
    let library = Uuid::from_u128(0x5e47_0000_0000_4000_8000_0000_0000_0001);
    let item = Uuid::from_u128(n);
    let id: Id<Part> = ew_ids::v5::lib_copy_id(library, item);
    let mut doc = json!({
        "$k": "part", "id": id.to_string(), "library_id": library.to_string(), "item_id": item.to_string(),
        "version": "1.0.0", "sha256": format!("sha256:{}", "0".repeat(64)),
        "kind": "wire", "part_number": number,
        "manufacturer": {"library": library.to_string(), "item": library.to_string(), "version_req": "*"},
        "sections": {"wire": {"csa": csa, "colour": colour}},
    });
    let parsed = LibDocument::from_value(doc.clone()).expect("a part document");
    doc["sha256"] = json!(ew_schema::Sha256(ew_format::library_copy_hash(&parsed).unwrap()).to_string());
    (id, doc)
}

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
            .all(|n| matches!(n.1, PartKind::Ferrule | PartKind::Marker)),
        "{numbers:?}"
    );
}

/// A wire takes the first stock wire part of its size and colour; the stock command embeds its copy once and refuses
/// a part that is no wire, ferrule or marker; a wire part of another size given to the wire is EW-MFG-022.
#[test]
fn a_wire_takes_its_stock_part_and_a_part_of_another_size_is_reported() {
    let mut s = common::sample();
    let (p, wire, csa, colour) = a_wire(&s.query());
    let (other, other_doc) = wire_part(0x7101, "WIRE-OTHER", "16", &colour);
    let (fits, fits_doc) = wire_part(0x7102, "WIRE-FITS", &csa, &colour);
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
        refused.to_string().contains("is no wire, ferrule or marker"),
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
    let (own, own_doc) = wire_part(0x7103, "WIRE-OWN", "1.5", &colour);
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
