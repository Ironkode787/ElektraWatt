// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.19 D16 (RI1, RI2): EW's interruption-point forms. A point drawn with the source form (`ew.gen.ip`) or the
//! target form (`ew.gen.ip-target`) is never turned and is drawn with its direction's form: `ip.place` and
//! `rep.transform` refuse a contradiction in words, `ip.continue` gives the partner the other form, and a direction
//! change swaps the form (normaliser 18), refused in words when the project does not hold the twin.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::reason as rejection;
use common::*;

use ew_commands::commands::{ContinueIp, KindName, PlaceIp, SetFields, TransformReps};
use ew_ids::{Id, Uuid};
use ew_mltext::MlText;
use ew_model::Model;
use ew_schema::{Function, FunctionKind, IpDirection, LibDocument, LibItem, Representation, SymbolDef};
use ew_units::Orient2;
use serde_json::json;

/// The system library the forms come from.
const LIBRARY: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

/// The copy ID of the starter symbol `key`.
fn copy_of(key: &str) -> Id<SymbolDef> {
    let item: Id<LibItem> = ew_ids::v5::imported_id(LIBRARY, key);
    ew_ids::v5::lib_copy_id(LIBRARY, item.as_uuid())
}

/// A copy of the form `key`, named as the starter names it.
fn form(key: &str) -> LibDocument {
    let id = copy_of(key);
    let mut s = symbol(id);
    s.library_id = Id::from_uuid(LIBRARY);
    s.item_id = ew_ids::v5::imported_id(LIBRARY, key);
    s.item.meta.key = Some(ident(key));
    s.item.name = MlText::neutral("Interruption point");
    LibDocument::Symbol(s)
}

/// The mini golden model with the source form, and the target form when `twin`.
fn model(twin: bool) -> Model {
    let (p, records, _) = mini_golden_parts();
    let mut lib = library();
    lib.push(form("ew.gen.ip@qet10"));
    if twin {
        lib.push(form("ew.gen.ip-target@qet10"));
    }
    Model::from_records(p, records, lib).expect("valid model")
}

fn place(symbol: &str, direction: IpDirection, orient: Orient2) -> PlaceIp {
    PlaceIp {
        page: PAGE_101,
        symbol: copy_of(symbol),
        fndef: Some(FNDEF_IP),
        pos: pt(3 * G, 2 * G),
        orient,
        name: ident("X-1"),
        direction,
        star_source: false,
        sort: None,
        external: None,
        copies: Vec::new(),
    }
}

/// The representation of IP function `f`.
fn rep_of(e: &ew_commands::Engine, f: Id<Function>) -> Representation {
    e.model()
        .table::<Representation>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .find(|r| r.functions == vec![f])
        .expect("a representation")
}

const TURNED: &str =
    "an interruption point's arrow points the way its signal flows: change its direction instead";

#[test]
fn a_point_takes_its_direction_s_form_unturned() {
    let mut e = engine(model(true));
    // The source form for a source, the target form for a target.
    for (key, direction) in [
        ("ew.gen.ip@qet10", IpDirection::Source),
        ("ew.gen.ip-target@qet10", IpDirection::Target),
    ] {
        let r = e
            .execute(&place(key, direction, Orient2::default()))
            .expect("ip.place");
        let f: Id<Function> = typed(r.outcome.primary.unwrap());
        assert_eq!(rep_of(&e, f).symbol, copy_of(key));
    }
    // Turned, or with the other direction's form: refused in words.
    assert_eq!(
        rejection(e.execute(&place("ew.gen.ip@qet10", IpDirection::Source, Orient2::R90))),
        TURNED
    );
    assert_eq!(
        rejection(e.execute(&place("ew.gen.ip@qet10", IpDirection::Target, Orient2::default()))),
        "a source interruption point is drawn with the outgoing arrow, a target with the incoming arrow"
    );
    // An undirected point takes either form.
    e.execute(&place(
        "ew.gen.ip-target@qet10",
        IpDirection::Undirected,
        Orient2::default(),
    ))
    .expect("an undirected point");
}

#[test]
fn rep_transform_refuses_a_form_and_turns_any_other_symbol() {
    let mut e = engine(model(true));
    let r = e
        .execute(&place("ew.gen.ip@qet10", IpDirection::Source, Orient2::default()))
        .expect("ip.place");
    let rep = rep_of(&e, typed(r.outcome.primary.unwrap())).id;
    let turn = |rep: Id<Representation>| TransformReps {
        reps: vec![rep],
        rotate: 1,
        mirror: false,
        variant: None,
    };
    assert_eq!(rejection(e.execute(&turn(rep))), TURNED);
    // The fixture's own IP symbol is not one of EW's forms: it keeps a free orientation.
    e.execute(&turn(REP_IP_101)).expect("another IP symbol turns");
}

#[test]
fn continue_gives_the_partner_the_other_form() {
    let mut e = engine(model(true));
    let r = e
        .execute(&place("ew.gen.ip@qet10", IpDirection::Source, Orient2::default()))
        .expect("ip.place");
    let f: Id<Function> = typed(r.outcome.primary.unwrap());
    let r = e
        .execute(&ContinueIp {
            ip: f,
            page: PAGE_102,
            pos: None,
        })
        .expect("ip.continue");
    let partner = rep_of(&e, typed(r.outcome.primary.unwrap()));
    assert_eq!(partner.symbol, copy_of("ew.gen.ip-target@qet10"));
    assert_eq!(partner.orient, Orient2::default());
}

fn direction(f: Id<Function>, to: &str) -> SetFields {
    SetFields {
        kind: KindName::Function,
        targets: vec![f.cast()],
        path: "kind.ip.direction".into(),
        value: json!(to),
        copies: Vec::new(),
    }
}

#[test]
fn a_direction_change_swaps_the_form() {
    let mut e = engine(model(true));
    let r = e
        .execute(&place("ew.gen.ip@qet10", IpDirection::Source, Orient2::default()))
        .expect("ip.place");
    let f: Id<Function> = typed(r.outcome.primary.unwrap());
    e.execute(&direction(f, "target")).expect("fields.set");
    assert_eq!(rep_of(&e, f).symbol, copy_of("ew.gen.ip-target@qet10"));
    let row = e.model().table::<Function>().get(&f).map(|f| f.kind.clone());
    assert!(matches!(row, Some(FunctionKind::Ip(ip)) if ip.direction == IpDirection::Target));
    e.execute(&direction(f, "source")).expect("fields.set");
    assert_eq!(rep_of(&e, f).symbol, copy_of("ew.gen.ip@qet10"));
}

#[test]
fn without_the_twin_a_direction_change_is_refused_in_words() {
    let mut e = engine(model(false));
    let r = e
        .execute(&place("ew.gen.ip@qet10", IpDirection::Source, Orient2::default()))
        .expect("ip.place");
    let f: Id<Function> = typed(r.outcome.primary.unwrap());
    assert_eq!(
        rejection(e.execute(&direction(f, "target"))),
        "the project does not hold the incoming arrow of symbol Interruption point: place one interruption point of \
         that direction first"
    );
    assert_eq!(rep_of(&e, f).symbol, copy_of("ew.gen.ip@qet10"));
}
