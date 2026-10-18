// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device tags and structure (doc 03 §8.2–§8.5): small fixtures with one rule each.

mod common;

use common::{Fx, id, uid};
use ew_derive::{DeviceTag, DisplayedTag, MainRep, PageStructure, Q, StructureOf, TagSource};
use ew_rds::Aspect;
use serde_json::json;

const P01: u128 = 0x10;
const JE2416: u128 = 0x11;
const FIELD: u128 = 0x12;
const PAGE101: u128 = 0x101;
const PAGE103: u128 = 0x103;
const PAGE203: u128 = 0x203;
const FNDEF: u128 = 0x500;
const SYM: u128 = 0x510;

fn base() -> Fx {
    let mut fx = Fx::new();
    fx.node(P01, "function", "P01", None)
        .node(JE2416, "location", "J-E-2416", None)
        .node(FIELD, "location", "FIELD", None)
        .page(
            PAGE101,
            101,
            None,
            "multi_line",
            &[("function", P01), ("location", JE2416)],
            "Power",
        )
        .page(
            PAGE103,
            103,
            None,
            "multi_line",
            &[("function", P01), ("location", JE2416)],
            "Dist",
        )
        .page(
            PAGE203,
            203,
            None,
            "fluid",
            &[("function", P01), ("location", JE2416)],
            "Valves",
        )
        .fndef(
            FNDEF,
            ["ProtectionOvercurrent", "MCB", "1P"],
            &[(0, "1"), (1, "2")],
        )
        .symbol(
            SYM,
            FNDEF,
            json!("origin"),
            &[(0, 0, 0, 0), (1, 1, 0, 16_000_000)],
        );
    fx
}

/// A placed device: device `n`, its main function `n + 1`, its representation `n + 2` on `page`.
fn placed(fx: &mut Fx, n: u128, letter: &str, counter: Option<u32>, page: u128) {
    fx.device(n, letter, counter)
        .function(n + 1, Some(n), FNDEF, true)
        .rep(n + 2, page, SYM, &[n + 1], 40_000_000, 40_000_000);
}

#[test]
fn page_based_tag_full_and_displayed() {
    let mut fx = base();
    placed(&mut fx, 0x1000, "F", Some(1), PAGE101);
    let m = fx.build();
    let q = Q::new(&m);
    let tag = q.get::<DeviceTag>(id(0x1000));
    let tag = tag.as_ref().as_ref().unwrap();
    assert_eq!(tag.product, "101F1");
    assert_eq!(tag.full, "=P01+J-E-2416-101F1");
    assert_eq!(*q.get::<DisplayedTag>((id(0x1000), id(PAGE101))), "-101F1");
    assert!(matches!(&tag.source, TagSource::Scheme { key, .. } if key == "page-based"));
    assert_eq!(*q.get::<MainRep>(id(0x1000)), Some(id(0x1002)));
}

#[test]
fn displayed_tag_shows_aspects_that_differ_from_the_page() {
    let mut fx = base();
    placed(&mut fx, 0x1000, "Q", Some(1), PAGE203);
    fx.record(0x1000)["structure"] = json!({"location": uid(FIELD)});
    let m = fx.build();
    let q = Q::new(&m);
    let tag = q.get::<DeviceTag>(id(0x1000));
    let tag = tag.as_ref().as_ref().unwrap();
    assert_eq!(tag.full, "=P01+FIELD-203Q1");
    assert_eq!(*q.get::<DisplayedTag>((id(0x1000), id(PAGE203))), "+FIELD-203Q1");
}

#[test]
fn frozen_tag_hides_its_sign_on_pages_only() {
    let mut fx = base();
    placed(&mut fx, 0x1000, "S", Some(1), PAGE101);
    fx.record(0x1000)["tag"]["frozen"] = json!({"text": "S51", "sign": "hidden"});
    let m = fx.build();
    let q = Q::new(&m);
    let tag = q.get::<DeviceTag>(id(0x1000));
    let tag = tag.as_ref().as_ref().unwrap();
    assert_eq!(tag.full, "=P01+J-E-2416-S51");
    assert_eq!(tag.source, TagSource::Frozen);
    assert_eq!(*q.get::<DisplayedTag>((id(0x1000), id(PAGE101))), "S51");
}

#[test]
fn strip_rule_child_tags_and_terminals() {
    let mut fx = base();
    fx.project_set(
        "settings",
        json!({"numbering": {"default": "page-based", "schemes": [{
            "key": "page-based", "format": "{page}{letter}{counter}",
            "rules": [{"select": {"aspect": "strip"}, "format": "{letter}{page}{counter}"}]}]}}),
    );
    // Strip -X1031 on folio 103 and its terminal 1.
    placed(&mut fx, 0x1000, "X", Some(1), PAGE103);
    fx.record(0x1000)["strip"] = json!({});
    fx.device(0x1100, "X", None).with(
        "terminal",
        json!({"strip": uid(0x1000), "seq": 1, "designation": "1"}),
    );
    // Sub-base -101U3 (default scheme, page-based) and its valve -101U3-Q1 drawn on folio 203.
    placed(&mut fx, 0x2000, "U", Some(3), PAGE101);
    placed(&mut fx, 0x2100, "Q", Some(1), PAGE203);

    fx.record(0x2100)["parent"] = json!(uid(0x2000));
    fx.record(0x2100)["tag"]["scheme"] = json!("child");
    let m = fx.build();
    let q = Q::new(&m);
    let full = |n: u128| q.get::<DeviceTag>(id(n)).as_ref().as_ref().unwrap().full.clone();
    assert_eq!(full(0x1000), "=P01+J-E-2416-X1031");
    assert_eq!(full(0x1100), "=P01+J-E-2416-X1031:1");
    assert_eq!(*q.get::<DisplayedTag>((id(0x1100), id(PAGE103))), "-X1031:1");
    assert_eq!(full(0x2000), "=P01+J-E-2416-101U3");
    assert_eq!(full(0x2100), "=P01+J-E-2416-101U3-Q1");
    assert_eq!(*q.get::<DisplayedTag>((id(0x2100), id(PAGE203))), "-101U3-Q1");
    // The valve's structure comes from its parent, not from folio 203.
    let s = q.get::<StructureOf>(id(0x2100));
    assert_eq!(s.node(Aspect::Location), Some(id(JE2416)));
}

#[test]
fn unplaced_and_unnumbered_devices_print_placeholders() {
    let mut fx = base();
    fx.device(0x1000, "K", None);
    let m = fx.build();
    let q = Q::new(&m);
    let tag = q.get::<DeviceTag>(id(0x1000));
    assert_eq!(tag.as_ref().as_ref().unwrap().full, "-?K?");
    assert!(q.get::<DeviceTag>(id(0x9999)).is_none());
    assert!(q.get::<PageStructure>(id(0x9999)).slots.is_empty());
}
