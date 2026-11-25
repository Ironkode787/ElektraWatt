// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A device put onto a wire leaves the wire's record on the part that keeps the wire's From end (FR-CON-008, doc 06
//! §5.3), so the number the page printed for the wire stays where it was printed.

use crate::common;

use common::*;
use ew_commands::commands::{PlaceSymbol, PlaceTarget};
use ew_ids::Id;
use ew_model::fixture::*;
use ew_model::{EndpointKey, Model};
use ew_schema::{AssignedNumber, Connection, Endpoint, Function, Kind, NumberSpec, Record};
use ew_units::Orient2;

/// The From end of the physical wire `wire` (doc 08 §3.8): the end its number is counted from.
fn from_end(model: &Model, wire: Id<Connection>) -> Endpoint {
    let q = ew_derive::Q::new(model);
    let c = q.connections().get(wire).expect("the wire").clone();
    let partition = q.get::<ew_derive::PotentialPartition>(());
    let potential = partition
        .potential_at(EndpointKey::of(&c.a).expect("a CP"))
        .expect("a potential");
    let wires = q.get::<ew_derive::PhysicalWiresOf>(potential.id);
    let mut physical = wires.get(wire).expect("a physical wire of its own").clone();
    ew_derive::orient_wire(&q, &mut physical);
    Endpoint::Cp(physical.ends[0].cp.function, physical.ends[0].cp.idx)
}

#[test]
fn the_part_with_the_from_end_keeps_the_wire_and_its_number() {
    let wire: Id<Connection> = id(0x4100);
    // The one wire drawn both ways round: whichever end the numbering counts from, its part keeps the wire.
    for reversed in [false, true] {
        let (mut a, mut b) = (cp(FN_102G1, 0), cp(FN_T1, 1));
        let mut route = vec![pt(0, -MM10), pt(0, 2 * MM10)];
        if reversed {
            std::mem::swap(&mut a, &mut b);
            route.reverse();
        }
        let mut original = connection(wire, Some(PAGE_102), a, b);
        original.route = route;
        original.number = NumberSpec::Assigned(AssignedNumber::new(7));
        let model = {
            let (p, mut records, _) = mini_project_parts();
            records.push(Record::Connection(original.clone()));
            Model::from_records(p, records, library()).expect("valid model")
        };
        let from = from_end(&model, wire);
        let mut e = engine(model);
        let r = e
            .execute(&PlaceSymbol {
                copies: Vec::new(),
                page: PAGE_102,
                symbol: SYM_TWO,
                pos: pt(0, 0),
                orient: Orient2::R0,
                rep_type: None,
                variant: 0,
                target: PlaceTarget::NewDevice {
                    letter: ident("F"),
                    fndef: None,
                    kind: None,
                },
                connect: vec![],
                into: None,
                replace: false,
            })
            .unwrap();
        let f: Id<Function> = created(&r, Kind::Function)[0];
        let kept = row(&e, wire);
        assert!(
            kept.a == from || kept.b == from,
            "reversed {reversed}: the wire keeps its From end {from:?}: {:?} to {:?}",
            kept.a,
            kept.b
        );
        assert_eq!(kept.number, original.number, "reversed {reversed}");
        let new: Vec<Id<Connection>> = created(&r, Kind::Connection);
        let half = row(&e, new[0]);
        assert_eq!(half.number, NumberSpec::Unassigned);
        assert_eq!(half.overrides, original.overrides);
        assert!(
            [&half.a, &half.b]
                .iter()
                .any(|end| matches!(end, Endpoint::Cp(fun, _) if *fun == f)),
            "the new part ends at the device"
        );
    }
}
