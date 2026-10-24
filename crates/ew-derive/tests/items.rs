// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! Page items and structure boxes, derived (WP-S2.04 part 4a, §10.2; acceptance PI2's derivation half): the
//! innermost box around a representation's anchor point, `structure_of` taking the box's slot for an inherited
//! aspect (`SlotSource::StructureBox`), `ip_structure`, bound texts, captions and the stale-entry load notes.

mod common;

use common::{Fx, id, uid};
use ew_derive::{
    DeriveNote, IpStructure, Q, RepStructureBox, SlotSource, StructureOf, bound_text, box_caption,
    stale_marker_notes,
};
use ew_rds::Aspect;
use ew_schema::{Device, Function, Page, Representation, StructureBox, StructureNode};
use serde_json::{Value, json};

const LOC_2416: u128 = 0x10;
const LOC_206: u128 = 0x11;
const LOC_OTHER: u128 = 0x12;
const PAGE: u128 = 0x30;
const SYM: u128 = 0x40;
const FNDEF: u128 = 0x41;
const FNDEF_IP: u128 = 0x42;
const DEV: u128 = 0x100;
const STRIP: u128 = 0x101;
const TERM: u128 = 0x102;
const FN_DEV: u128 = 0x200;
const FN_IP: u128 = 0x201;
const FN_TERM: u128 = 0x202;
const REP_DEV: u128 = 0x300;
const REP_IP: u128 = 0x301;
const REP_TERM: u128 = 0x302;
const BOX_OUTER: u128 = 0x400;
const BOX_INNER: u128 = 0x401;
const EXT: u128 = 0x500;

/// A rectangle outline in nm.
fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Value {
    json!([[x0, y0], [x1, y0], [x1, y1], [x0, y1]])
}

/// One page of location `+J-E-2416` with a device at (40, 40) mm, an interruption point at (40, 60) mm and a terminal
/// of a strip at (40, 40) mm; the symbol's anchor is its origin.
fn fixture() -> Fx {
    let mut fx = Fx::new();
    fx.node(LOC_2416, "location", "J-E-2416", None)
        .node(LOC_206, "location", "J-E-206", None)
        .node(LOC_OTHER, "location", "J-E-999", None)
        .page(PAGE, 101, None, "multi_line", &[("location", LOC_2416)], "Power")
        .fndef(FNDEF, ["Switch", "Contact", "NO"], &[(0, "13"), (1, "14")])
        .fndef(FNDEF_IP, ["Interruption", "Point", "IP"], &[(0, "1")])
        .symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0)])
        .device(DEV, "Q", Some(1))
        .function(FN_DEV, Some(DEV), FNDEF, true)
        .rep(REP_DEV, PAGE, SYM, &[FN_DEV], 40_000_000, 40_000_000)
        .push(json!({"$k": "function", "id": uid(FN_IP), "def": uid(FNDEF_IP),
            "kind": {"ip": {"name": "L-1", "direction": "source"}}}))
        .rep(REP_IP, PAGE, SYM, &[FN_IP], 40_000_000, 60_000_000)
        .device(STRIP, "X", Some(1))
        .with("strip", json!({}))
        .device(TERM, "X", None)
        .with(
            "terminal",
            json!({"strip": uid(STRIP), "seq": 1, "designation": "1"}),
        )
        .function(FN_TERM, Some(TERM), FNDEF, true)
        .rep(REP_TERM, PAGE, SYM, &[FN_TERM], 40_000_000, 40_000_000)
        .push(
            json!({"$k": "external_ref", "id": uid(EXT), "location": uid(LOC_206), "device": "JE-N1000-03"}),
        );
    fx
}

/// A structure box on the page.
fn structure_box(n: u128, outline: Value, assigns: Value) -> Value {
    json!({"$k": "structure_box", "id": uid(n), "page": uid(PAGE), "outline": outline, "style": "dotted",
        "assigns": assigns})
}

fn location_of(q: &Q, dev: u128) -> Option<(u128, SlotSource)> {
    q.get::<StructureOf>(id::<Device>(dev))
        .slots
        .get(&Aspect::Location)
        .map(|s| (s.node.as_uuid().as_u128(), s.source))
}

/// FR-PRJ-020: the box around a device's main representation gives an inherited aspect its node
/// (`SlotSource::StructureBox`); outside the box the page's node applies; a terminal keeps its strip's structure.
#[test]
fn a_structure_box_assigns_the_location_of_the_devices_inside_it() {
    let mut fx = fixture();
    let m0 = fx.build();
    assert_eq!(
        location_of(&Q::new(&m0), DEV),
        Some((LOC_2416, SlotSource::MainRepPage))
    );
    fx.push(structure_box(
        BOX_OUTER,
        rect(30_000_000, 30_000_000, 80_000_000, 50_000_000),
        json!({"location": uid(LOC_206)}),
    ));
    let m1 = fx.build();
    let q = Q::new(&m1);
    assert_eq!(
        *q.get::<RepStructureBox>(id::<Representation>(REP_DEV)),
        Some(id::<StructureBox>(BOX_OUTER))
    );
    assert_eq!(location_of(&q, DEV), Some((LOC_206, SlotSource::StructureBox)));
    // A terminal inherits from its strip (step 2), never from a box directly; the strip's main representation is
    // its terminal's (the strip fallback of `main_rep`), inside the box, so the strip takes the box's location.
    assert_eq!(location_of(&q, STRIP), Some((LOC_206, SlotSource::StructureBox)));
    assert_eq!(location_of(&q, TERM), Some((LOC_206, SlotSource::Parent)));
    // The interruption point at (40, 60) mm lies outside: no box.
    assert_eq!(*q.get::<RepStructureBox>(id::<Representation>(REP_IP)), None);

    // Moved out of the box, the device reverts to the page's location.
    fx.record(REP_DEV)["pos"] = json!([90_000_000, 40_000_000]);
    let m2 = fx.build();
    assert_eq!(
        location_of(&Q::new(&m2), DEV),
        Some((LOC_2416, SlotSource::MainRepPage))
    );
}

/// The innermost box wins (by absolute area), ties by box ID; a box's `none` clears the aspect and an absent slot
/// leaves the page's; the device's own slot beats every box.
#[test]
fn the_innermost_box_wins_and_none_clears_the_aspect() {
    let mut fx = fixture();
    fx.push(structure_box(
        BOX_OUTER,
        rect(0, 0, 200_000_000, 200_000_000),
        json!({"location": uid(LOC_206)}),
    ))
    .push(structure_box(
        BOX_INNER,
        rect(35_000_000, 35_000_000, 45_000_000, 45_000_000),
        json!({"location": uid(LOC_OTHER)}),
    ));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(
        *q.get::<RepStructureBox>(id::<Representation>(REP_DEV)),
        Some(id::<StructureBox>(BOX_INNER))
    );
    assert_eq!(location_of(&q, DEV), Some((LOC_OTHER, SlotSource::StructureBox)));
    assert_eq!(
        *q.get::<RepStructureBox>(id::<Representation>(REP_IP)),
        Some(id::<StructureBox>(BOX_OUTER))
    );

    // Equal areas: the smaller ID.
    let mut tie = fixture();
    tie.push(structure_box(
        BOX_INNER,
        rect(30_000_000, 30_000_000, 50_000_000, 50_000_000),
        json!({"location": uid(LOC_OTHER)}),
    ))
    .push(structure_box(
        BOX_OUTER,
        rect(35_000_000, 35_000_000, 55_000_000, 55_000_000),
        json!({"location": uid(LOC_206)}),
    ));
    let m = tie.build();
    assert_eq!(
        *Q::new(&m).get::<RepStructureBox>(id::<Representation>(REP_DEV)),
        Some(id::<StructureBox>(BOX_OUTER)),
        "the smaller ID of two boxes of one area"
    );

    // `none` clears the aspect; an absent slot leaves the page's node.
    let mut none = fixture();
    none.push(structure_box(
        BOX_OUTER,
        rect(30_000_000, 30_000_000, 80_000_000, 50_000_000),
        json!({"location": "none"}),
    ));
    assert_eq!(location_of(&Q::new(&none.build()), DEV), None);
    let mut absent = fixture();
    absent.push(structure_box(
        BOX_OUTER,
        rect(30_000_000, 30_000_000, 80_000_000, 50_000_000),
        json!({"function": "none"}),
    ));
    assert_eq!(
        location_of(&Q::new(&absent.build()), DEV),
        Some((LOC_2416, SlotSource::MainRepPage))
    );

    // The device's own slot wins.
    let mut own = fixture();
    own.record(DEV)["structure"] = json!({"location": uid(LOC_OTHER)});
    own.push(structure_box(
        BOX_OUTER,
        rect(30_000_000, 30_000_000, 80_000_000, 50_000_000),
        json!({"location": uid(LOC_206)}),
    ));
    assert_eq!(
        location_of(&Q::new(&own.build()), DEV),
        Some((LOC_OTHER, SlotSource::Explicit))
    );
}

/// `ip_structure`: an interruption point's structure is its box's slots over its page's; a function that is no
/// interruption point has an empty one.
#[test]
fn ip_structure_is_the_box_over_the_page() {
    let mut fx = fixture();
    let m0 = fx.build();
    let q = Q::new(&m0);
    let page_loc = q.get::<IpStructure>(id::<Function>(FN_IP)).node(Aspect::Location);
    assert_eq!(page_loc, Some(id::<StructureNode>(LOC_2416)));
    assert!(q.get::<IpStructure>(id::<Function>(FN_DEV)).slots.is_empty());
    fx.push(structure_box(
        BOX_OUTER,
        rect(30_000_000, 55_000_000, 80_000_000, 70_000_000),
        json!({"location": uid(LOC_206)}),
    ));
    let m1 = fx.build();
    let q = Q::new(&m1);
    let s = q.get::<IpStructure>(id::<Function>(FN_IP));
    assert_eq!(s.node(Aspect::Location), Some(id::<StructureNode>(LOC_206)));
    assert_eq!(s.slots[&Aspect::Location].source, SlotSource::StructureBox);
    // The device outside that box keeps the page's location.
    assert_eq!(location_of(&q, DEV), Some((LOC_2416, SlotSource::MainRepPage)));
}

/// Bound texts print the title block's value of their placeholder on the page; `page.full_name` is the page's
/// structure and folio; another name prints nothing.
#[test]
fn bound_texts_print_the_title_block_value() {
    let mut fx = fixture();
    fx.project_set(
        "master",
        json!({"description": {"en": "Expansion Control Cabinets"}, "drawing_id": "D-1"}),
    );
    let m = fx.build();
    let q = Q::new(&m);
    let page = id::<Page>(PAGE);
    assert_eq!(
        bound_text(&q, page, "project.description").as_deref(),
        Some("Expansion Control Cabinets")
    );
    assert_eq!(bound_text(&q, page, "project.drawing_id").as_deref(), Some("D-1"));
    assert_eq!(bound_text(&q, page, "page.folio").as_deref(), Some("101"));
    assert_eq!(bound_text(&q, page, "page.title").as_deref(), Some("Power"));
    assert_eq!(
        bound_text(&q, page, "page.full_name").as_deref(),
        Some("+J-E-2416/101")
    );
    assert_eq!(bound_text(&q, page, "project.reference"), None);
}

/// A caption prints the external reference as the external cross-reference texts print it.
#[test]
fn a_caption_prints_its_external_reference() {
    let mut fx = fixture();
    fx.push(structure_box(BOX_OUTER, rect(0, 0, 1, 1), json!({})))
        .with("caption", json!("External Connection"))
        .with("external", json!(uid(EXT)));
    let m = fx.build();
    let q = Q::new(&m);
    let b = q
        .structure_boxes()
        .get(id::<StructureBox>(BOX_OUTER))
        .cloned()
        .unwrap();
    assert_eq!(
        box_caption(&q, &b),
        "External Connection (+J-E-206 / JE-N1000-03)"
    );
}

/// A wire marker of an older file that lists a deleted connection is a load note, not a refusal.
#[test]
fn a_stale_marker_entry_is_a_load_note() {
    let mut fx = fixture();
    fx.push(json!({"$k": "page_item", "id": uid(0x600), "page": uid(PAGE),
        "kind": {"wire_marker": {"connections": [uid(0x9999)], "at": [0, 0], "fields": ["csa", "colour"]}}}));
    let m = fx.build();
    let notes = stale_marker_notes(&Q::new(&m));
    assert_eq!(notes.len(), 1);
    assert!(matches!(&notes[0], DeriveNote::StaleMarkerEntry { detail, .. } if *detail == "does not exist"));
    assert!(notes[0].to_string().contains("is ignored"), "{}", notes[0]);
}
