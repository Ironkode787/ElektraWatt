// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wire numbers (doc 08 §5.2, doc 20 §8.1, FR-NUM-019, FR-NUM-028; FT-100) on synthetic models: devices with
//! one CP each, drawn at chosen cells of the QET frame, wired by single connections.
//!
//! - `column_comes_before_row`: the From end's cell is compared by column, then row.
//! - `the_to_cell_is_the_second_key`: wires from one From cell are ordered by the To cell, before the wire ID.
//! - `the_to_page_compares_before_the_to_cell`: a wire to another page sorts after the wires that stay on the
//!   From page, whatever the To cell.
//! - `a_tie_is_decided_by_the_wire_id_and_keeps_its_range`: equal cells, labels and route, the wire ID decides,
//!   `tie` gives the counter range of the group.
//! - FT-440, the tie-break among equal cells: `the_from_label_decides_before_the_wire_id_and_the_to_label`,
//!   `the_labels_compare_in_natural_order`, `the_to_label_decides_when_the_from_labels_are_equal`,
//!   `the_first_route_vertex_decides_when_the_labels_are_equal` (y, then x, a wire without a route last) and
//!   `the_route_is_read_from_the_from_end`.
//! - `a_core_takes_no_counter`: a cable core inside a page run is in the order but unnumbered.
//! - `a_wire_on_no_page_is_last_and_unnumbered`.
//! - `a_terminal_in_a_strip_segment_has_a_cell_for_the_orientation`: a wire from a segment-only terminal to a
//!   device below it on one page starts at the terminal.
//! - `counters_run_per_page_and_the_page_token_is_the_folio_label`.
//! - `the_flow_distance_decides_before_the_anchor_cell`: FT-101 rule 2, a source feeds two wires in a chain.
//! - `an_entry_through_a_pass_through_has_distance_zero`: the downstream CP of a pass-through pair is the
//!   entry of the potential behind it, so the wire from that CP starts there.
//! - `duplicated_pins_of_a_black_box_cost_no_hop`: two equal pins of a black box share one distance.
//! - `a_bridge_and_a_terminal_level_short_are_one_hop_each`.
//! - `a_rail_bond_is_not_a_hop`: a CP reachable only through a rail bond has no distance, rule 3 decides.
//! - `unreached_and_tied_ends_fall_to_the_anchor_cell`: no entry, and two ends at equal distance.
//! - `a_bond_wire_falls_to_the_anchor_cell`: a wire between two potentials is not compared by distance.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use common::{Fx, id, uid};
use ew_derive::{
    CpFlowDistanceOf, CpFlowDistances, CpRef, PhysicalWiresOf, PotentialPartition, Q, WireNumbers,
    orient_wire,
};
use ew_model::Model;
use ew_units::{Grid, Pt2};
use serde_json::{Value, json};

const P1: u128 = 0x101;
const P2: u128 = 0x102;
const FRAME_COPY: &str = "c80af135-34f8-574c-8f52-756bd24c7bd0";
const FN_PLAIN: u128 = 0x500;
const SYM_PLAIN: u128 = 0x510;
const FN_IP: u128 = 0x501;
const SYM_IP: u128 = 0x511;
const FN_TERM: u128 = 0x502;
const FN_STRIP: u128 = 0x503;
const FN_SRC: u128 = 0x504;
const FN_PASS: u128 = 0x505;
const FN_BOX: u128 = 0x506;
const FN_RAIL: u128 = 0x507;
const FN_TERM2: u128 = 0x508;
const FN_SHORT: u128 = 0x509;
const SYM_SEG: u128 = 0x513;
const SYM_SRC: u128 = 0x514;
const SYM_PASS: u128 = 0x515;
const SYM_BOX: u128 = 0x516;
const SYM_TERM: u128 = 0x517;
const SYM_RAIL: u128 = 0x518;
const SYM_SHORT: u128 = 0x519;
const REP: u128 = 0x10_000;

fn fixture() -> Fx {
    let mut fx = Fx::new();
    fx.lib(
        serde_json::from_str(include_str!(
            "../../../content/forms/ew.frame.qet-a4-landscape-grid.json"
        ))
        .unwrap(),
    );
    fx.lib(serde_json::from_str(include_str!("../../../content/forms/ew.titleblock.qet-a4.json")).unwrap());
    fx.fndef(FN_PLAIN, ["Misc", "Misc", "Plain"], &[(0, "1")]);
    fx.symbol(SYM_PLAIN, FN_PLAIN, json!("origin"), &[(0, 0, 0, 0)]);
    fx.fndef(FN_IP, ["Misc", "Misc", "Ip"], &[(0, "1")]);
    fx.symbol(SYM_IP, FN_IP, json!("origin"), &[(0, 0, 0, 0)]);
    fx.fndef(FN_STRIP, ["Misc", "Misc", "Strip"], &[(0, "1")]);
    fx.symbol(SYM_SEG, FN_STRIP, json!("origin"), &[]);
    // The strip segment of the library: a dashed rectangle, three grid steps high, on the QET 10 px grid.
    edit_lib(&mut fx, SYM_SEG, |d| {
        d["grid"] = json!({"num": 7_937_500, "den": 3});
        d["rep_type"] = json!("strip_segment");
        d["graphics"] = json!([{"rect": {"rect": [[0, 0], [5_291_667, 7_937_500]], "stroke": {"pen": {"width": 132_292}}}}]);
    });
    // A terminal: side 1 and side 2.
    fx.lib_item(
        "fndef",
        FN_TERM,
        json!({
            "meta": {}, "category": ["Misc", "Misc", "Terminal"], "contact_kind": "terminal",
            "cps": [
                {"idx": 0, "designation": "1", "level": 1, "side": "side1", "clamp": {"clamp": "a"}},
                {"idx": 1, "designation": "1", "level": 1, "side": "side2", "clamp": {"clamp": "b"}},
            ],
        }),
    );
    for (page, number) in [(P1, 101), (P2, 102)] {
        fx.page(page, number, None, "multi_line", &[], &format!("page {number}"));
        fx.record(page)["form"] = json!(FRAME_COPY);
        fx.record(page)["grid"] = json!({"num": 7_937_500, "den": 3});
    }
    fx
}

fn edit_lib(fx: &mut Fx, n: u128, f: impl FnOnce(&mut Value)) {
    let key = uid(n);
    let doc = fx
        .lib
        .iter_mut()
        .find(|d| d["id"] == json!(key))
        .unwrap_or_else(|| panic!("no library document {key}"));
    f(doc);
}

/// The point of the QET 10 px grid at the lower-left corner of the frame cell `(column, row)`: the frame
/// grid starts two steps in, a cell is 6 steps wide and 8 steps high (doc 20 §17.1).
fn at(column: i64, row: i64) -> Pt2 {
    let g = Grid::QET_10PX;
    Pt2::new(
        g.at(2 + 6 * column).expect("fits"),
        g.at(2 + 8 * row).expect("fits"),
    )
}

/// A plain device `n` with one CP, drawn on `page` in cell `(column, row)`.
fn node(fx: &mut Fx, n: u128, page: u128, column: i64, row: i64) {
    let p = at(column, row);
    fx.device(n + 0x20_000, "K", Some(n as u32));
    fx.function(n, Some(n + 0x20_000), FN_PLAIN, true);
    fx.rep(n + REP, page, SYM_PLAIN, &[n], p.x.0, p.y.0);
}

/// Like [`node`], with the device counter `counter` instead of one derived from `n`: two devices with one counter
/// print the same CP label.
fn node_tagged(fx: &mut Fx, n: u128, page: u128, column: i64, row: i64, counter: u32) {
    let p = at(column, row);
    fx.device(n + 0x20_000, "K", Some(counter));
    fx.function(n, Some(n + 0x20_000), FN_PLAIN, true);
    fx.rep(n + REP, page, SYM_PLAIN, &[n], p.x.0, p.y.0);
}

fn ip(fx: &mut Fx, n: u128, page: u128, name: &str, dir: &str, column: i64, row: i64) {
    let p = at(column, row);
    fx.push(json!({
        "$k": "function", "id": uid(n), "def": uid(FN_IP),
        "kind": {"ip": {"name": name, "direction": dir}},
    }));
    fx.rep(n + REP, page, SYM_IP, &[n], p.x.0, p.y.0);
}

fn wire(fx: &mut Fx, n: u128, page: Option<u128>, a: u128, b: u128) {
    wire_kind(fx, n, page, a, b, Value::Null);
}

fn wire_kind(fx: &mut Fx, n: u128, page: Option<u128>, a: u128, b: u128, kind: Value) {
    wire_cp(fx, n, page, (a, 0), (b, 0), kind);
}

/// A connection between CP `a.1` of function `a.0` and CP `b.1` of function `b.0`.
fn wire_cp(fx: &mut Fx, n: u128, page: Option<u128>, a: (u128, u16), b: (u128, u16), kind: Value) {
    let mut r = json!({
        "$k": "connection", "id": uid(n), "a": {"cp": [uid(a.0), a.1]}, "b": {"cp": [uid(b.0), b.1]},
    });
    if let Some(p) = page {
        r["page"] = json!(uid(p));
    }
    if !kind.is_null() {
        r["kind"] = kind;
    }
    fx.push(r);
}

/// The wires in the numbering order: wire ID, label, tie.
type Row = (u128, Option<String>, Option<(u32, u32)>);

fn order(m: &Model) -> Vec<Row> {
    let q = Q::new(m);
    q.get::<WireNumbers>(())
        .wires
        .iter()
        .map(|w| {
            (
                w.wire.as_uuid().as_u128(),
                w.number.as_ref().map(|n| n.label.clone()),
                w.number.as_ref().and_then(|n| n.tie),
            )
        })
        .collect()
}

fn ids(rows: &[Row]) -> Vec<u128> {
    rows.iter().map(|r| r.0).collect()
}

fn label(rows: &[Row], wire: u128) -> Option<String> {
    rows.iter().find(|r| r.0 == wire).and_then(|r| r.1.clone())
}

#[test]
fn column_comes_before_row() {
    let mut fx = fixture();
    // The sources: column 5 row A, and column 3 row C. A row-first order would put 0x2001 first.
    node(&mut fx, 0x1001, P1, 5, 0);
    node(&mut fx, 0x1002, P1, 3, 2);
    // The sinks share the last cell, after both sources in (row, column), so the sources are From.
    node(&mut fx, 0x1011, P1, 16, 7);
    node(&mut fx, 0x1012, P1, 16, 7);
    wire(&mut fx, 0x2001, Some(P1), 0x1001, 0x1011);
    wire(&mut fx, 0x2002, Some(P1), 0x1002, 0x1012);
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2002, 0x2001]);
    assert_eq!(label(&rows, 0x2002).as_deref(), Some("101.1"));
    assert_eq!(label(&rows, 0x2001).as_deref(), Some("101.2"));
}

#[test]
fn the_to_cell_is_the_second_key() {
    let mut fx = fixture();
    // Two sources in one cell, a third one column further left.
    node(&mut fx, 0x1001, P1, 2, 0);
    node(&mut fx, 0x1002, P1, 2, 0);
    node(&mut fx, 0x1003, P1, 1, 0);
    // To cells: column 10 for 0x2001 (the lower wire ID), column 8 for 0x2002, the last cell for 0x2003.
    node(&mut fx, 0x1011, P1, 10, 7);
    node(&mut fx, 0x1012, P1, 8, 7);
    node(&mut fx, 0x1013, P1, 16, 7);
    wire(&mut fx, 0x2001, Some(P1), 0x1001, 0x1011);
    wire(&mut fx, 0x2002, Some(P1), 0x1002, 0x1012);
    wire(&mut fx, 0x2003, Some(P1), 0x1003, 0x1013);
    let rows = order(&fx.build());
    // The From cell first (0x2003), then the To cell before the wire ID (0x2002 before 0x2001).
    assert_eq!(ids(&rows), vec![0x2003, 0x2002, 0x2001]);
}

#[test]
fn the_to_page_compares_before_the_to_cell() {
    let mut fx = fixture();
    node(&mut fx, 0x1001, P1, 2, 0);
    node(&mut fx, 0x1002, P1, 2, 0);
    // 0x2002 stays on page 1, its To cell is far to the right.
    node(&mut fx, 0x1011, P1, 15, 7);
    wire(&mut fx, 0x2002, Some(P1), 0x1001, 0x1011);
    // The other wire goes through an interruption-point pair to a device on page 2 whose cell (column 0) is
    // left of every cell of page 1; its carrier is the page-2 connection 0x2000, the lowest ID.
    ip(&mut fx, 0x1101, P1, "A", "source", 8, 1);
    ip(&mut fx, 0x1102, P2, "A", "target", 0, 0);
    node(&mut fx, 0x1012, P2, 0, 1);
    wire(&mut fx, 0x2001, Some(P1), 0x1002, 0x1101);
    wire(&mut fx, 0x2000, Some(P2), 0x1102, 0x1012);
    let m = fx.build();
    let rows = order(&m);
    assert_eq!(ids(&rows), vec![0x2002, 0x2000], "{rows:?}");
    // Both are numbered on the From page, folio 101.
    assert_eq!(label(&rows, 0x2002).as_deref(), Some("101.1"));
    assert_eq!(label(&rows, 0x2000).as_deref(), Some("101.2"));
}

#[test]
fn a_tie_is_decided_by_the_wire_id_and_keeps_its_range() {
    let mut fx = fixture();
    // Two wires with the same cells and the same CP labels (the two sources are devices of one tag, so are the
    // two sinks), no route; and one more wire elsewhere that sorts first. The wire IDs run against the order
    // of the declaration and of the device IDs.
    node_tagged(&mut fx, 0x1001, P1, 4, 1, 1);
    node_tagged(&mut fx, 0x1002, P1, 4, 1, 1);
    node_tagged(&mut fx, 0x1011, P1, 9, 6, 2);
    node_tagged(&mut fx, 0x1012, P1, 9, 6, 2);
    node(&mut fx, 0x1003, P1, 1, 1);
    node(&mut fx, 0x1013, P1, 2, 6);
    wire(&mut fx, 0x2003, Some(P1), 0x1001, 0x1011);
    wire(&mut fx, 0x2001, Some(P1), 0x1002, 0x1012);
    wire(&mut fx, 0x2002, Some(P1), 0x1003, 0x1013);
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2002, 0x2001, 0x2003]);
    assert_eq!(rows[0].2, None, "a wire alone is no tie");
    // The two wires of the tie keep the ID order and both carry the range of the group, counters 2 to 3.
    assert_eq!(rows[1], (0x2001, Some("101.2".to_owned()), Some((2, 3))));
    assert_eq!(rows[2], (0x2003, Some("101.3".to_owned()), Some((2, 3))));
}

#[test]
fn the_from_label_decides_before_the_wire_id_and_the_to_label() {
    let mut fx = fixture();
    // Two sources in one cell and two sinks in one cell: the same From and To cells. The wire with the lower
    // wire ID starts at the source with the higher label, so the ID order and the label order differ; its To
    // label is the lower one, so the To label would put it first if it came before the From label.
    node(&mut fx, 0x1001, P1, 4, 1);
    node(&mut fx, 0x1002, P1, 4, 1);
    node(&mut fx, 0x1011, P1, 9, 6);
    node(&mut fx, 0x1012, P1, 9, 6);
    wire(&mut fx, 0x2001, Some(P1), 0x1002, 0x1011);
    wire(&mut fx, 0x2002, Some(P1), 0x1001, 0x1012);
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2002, 0x2001]);
    assert_eq!(rows[0], (0x2002, Some("101.1".to_owned()), Some((1, 2))));
    assert_eq!(rows[1], (0x2001, Some("101.2".to_owned()), Some((1, 2))));
}

#[test]
fn the_labels_compare_in_natural_order() {
    let mut fx = fixture();
    // Counters whose digit count differs, 0x9 (K9) against 0x10 (K16): a text order would put K16 first.
    node(&mut fx, 0x10, P1, 4, 1);
    node(&mut fx, 0x9, P1, 4, 1);
    node(&mut fx, 0x1011, P1, 9, 6);
    wire(&mut fx, 0x2001, Some(P1), 0x10, 0x1011);
    wire(&mut fx, 0x2002, Some(P1), 0x9, 0x1011);
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2002, 0x2001], "K9 comes before K16");
}

#[test]
fn the_to_label_decides_when_the_from_labels_are_equal() {
    let mut fx = fixture();
    // Both wires start at the same CP and end in one cell; the wire with the lower ID ends at the higher label.
    node(&mut fx, 0x1001, P1, 4, 1);
    node(&mut fx, 0x1011, P1, 9, 6);
    node(&mut fx, 0x1012, P1, 9, 6);
    wire(&mut fx, 0x2001, Some(P1), 0x1001, 0x1012);
    wire(&mut fx, 0x2002, Some(P1), 0x1001, 0x1011);
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2002, 0x2001]);
    assert_eq!(rows[0], (0x2002, Some("101.1".to_owned()), Some((1, 2))));
}

/// A wire with a route, in nm.
fn wire_routed(fx: &mut Fx, n: u128, a: u128, b: u128, route: Value) {
    wire(fx, n, Some(P1), a, b);
    fx.record(n)["route"] = route;
}

#[test]
fn the_first_route_vertex_decides_when_the_labels_are_equal() {
    let mut fx = fixture();
    // Four sources of one tag in one cell, four sinks of one tag in one cell: equal cells and labels. The wire
    // IDs run against the order the routes ask for: the smaller y goes first, for equal y the smaller x, and a
    // wire without a route comes after those with one, whatever its ID.
    for k in 0..4 {
        node_tagged(&mut fx, 0x1001 + k, P1, 4, 1, 1);
        node_tagged(&mut fx, 0x1011 + k, P1, 9, 6, 2);
    }
    wire(&mut fx, 0x2000, Some(P1), 0x1001, 0x1011);
    wire_routed(&mut fx, 0x2001, 0x1002, 0x1012, json!([[0, 300], [10, 300]]));
    wire_routed(&mut fx, 0x2002, 0x1003, 0x1013, json!([[500, 100], [500, 300]]));
    wire_routed(&mut fx, 0x2003, 0x1004, 0x1014, json!([[200, 100], [500, 300]]));
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2003, 0x2002, 0x2001, 0x2000]);
    assert_eq!(rows[0].2, Some((1, 4)));
}

#[test]
fn the_route_is_read_from_the_from_end() {
    let mut fx = fixture();
    for k in 0..2 {
        node_tagged(&mut fx, 0x1001 + k, P1, 4, 1, 1);
        node_tagged(&mut fx, 0x1011 + k, P1, 9, 6, 2);
    }
    // 0x2001 runs from the From end (a is the source): its first vertex has y 200. 0x2002 is declared the other
    // way round (a is the sink), so its route runs towards the From end and the From end's vertex is the last
    // one, y 100; read from `a` it would be y 300 and sort after 0x2001.
    wire_routed(&mut fx, 0x2001, 0x1001, 0x1011, json!([[0, 200], [0, 0]]));
    wire_routed(&mut fx, 0x2002, 0x1012, 0x1002, json!([[0, 300], [0, 100]]));
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2002, 0x2001]);
}

#[test]
fn a_core_takes_no_counter() {
    let mut fx = fixture();
    fx.push(json!({"$k": "device", "id": uid(0x6000), "tag": {"letter": "W", "counter": 1}, "cable": {}}));
    fx.push(json!({"$k": "conductor", "id": uid(0x6101), "cable": uid(0x6000), "core": "BK"}));
    node(&mut fx, 0x1001, P1, 1, 0);
    node(&mut fx, 0x1002, P1, 2, 0);
    node(&mut fx, 0x1003, P1, 3, 0);
    node(&mut fx, 0x1011, P1, 16, 7);
    node(&mut fx, 0x1012, P1, 16, 7);
    node(&mut fx, 0x1013, P1, 16, 7);
    wire(&mut fx, 0x2001, Some(P1), 0x1001, 0x1011);
    // The core wire sits between the two plain wires in the order.
    wire_kind(
        &mut fx,
        0x2002,
        Some(P1),
        0x1002,
        0x1012,
        json!({"core": uid(0x6101)}),
    );
    wire(&mut fx, 0x2003, Some(P1), 0x1003, 0x1013);
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2001, 0x2002, 0x2003]);
    assert_eq!(label(&rows, 0x2001).as_deref(), Some("101.1"));
    assert_eq!(rows[1].1, None, "a cable core is not numbered");
    assert_eq!(
        label(&rows, 0x2003).as_deref(),
        Some("101.2"),
        "the core took no counter"
    );
}

#[test]
fn a_wire_on_no_page_is_last_and_unnumbered() {
    let mut fx = fixture();
    node(&mut fx, 0x1001, P1, 1, 0);
    node(&mut fx, 0x1002, P1, 16, 7);
    node(&mut fx, 0x1003, P2, 1, 0);
    node(&mut fx, 0x1004, P2, 16, 7);
    node(&mut fx, 0x1005, P1, 2, 0);
    node(&mut fx, 0x1006, P1, 16, 7);
    // The page-less wire has the lowest ID.
    wire(&mut fx, 0x2000, None, 0x1005, 0x1006);
    wire(&mut fx, 0x2002, Some(P2), 0x1003, 0x1004);
    wire(&mut fx, 0x2001, Some(P1), 0x1001, 0x1002);
    let rows = order(&fx.build());
    assert_eq!(ids(&rows), vec![0x2001, 0x2002, 0x2000]);
    assert_eq!(label(&rows, 0x2001).as_deref(), Some("101.1"));
    assert_eq!(
        label(&rows, 0x2002).as_deref(),
        Some("102.1"),
        "counters run per page"
    );
    assert_eq!(rows[2].1, None);
}

#[test]
fn counters_run_per_page_and_the_page_token_is_the_folio_label() {
    let mut fx = fixture();
    fx.record(P2)["folio"] = json!({"number": 102, "label": "102a"});
    for (i, page) in [P1, P1, P2, P2, P2].into_iter().enumerate() {
        let n = 0x1001 + i as u128;
        node(&mut fx, n, page, i as i64, 0);
        node(&mut fx, n + 0x10, page, 16, 7);
        wire(&mut fx, 0x2001 + i as u128, Some(page), n, n + 0x10);
    }
    let rows = order(&fx.build());
    let labels: Vec<Option<String>> = rows.iter().map(|r| r.1.clone()).collect();
    let want = ["101.1", "101.2", "102a.1", "102a.2", "102a.3"].map(|s| Some(s.to_owned()));
    assert_eq!(labels, want);
}

#[test]
fn a_terminal_in_a_strip_segment_has_a_cell_for_the_orientation() {
    let mut fx = fixture();
    // The strip X1 with one terminal, drawn only by a segment at the top of the page (column 4, row A).
    fx.device(0x4000, "X", Some(1)).with("strip", json!({}));
    fx.function(0x4001, Some(0x4000), FN_STRIP, true);
    let seg = at(4, 0);
    fx.rep(0x4001 + REP, P1, SYM_SEG, &[0x4001], seg.x.0, seg.y.0);
    fx.record(0x4001 + REP)["rep_type"] = json!("strip_segment");
    fx.device(0x4010, "X", None).with(
        "terminal",
        json!({"strip": uid(0x4000), "seq": 1, "designation": "1"}),
    );
    fx.function(0x4011, Some(0x4010), FN_TERM, true);
    // A device below the segment, in row D: it has an anchor cell, the terminal has none.
    node(&mut fx, 0x1001, P1, 8, 3);
    // The terminal's side-1 CP (idx 0) to the device.
    fx.push(json!({
        "$k": "connection", "id": uid(0x2001), "page": uid(P1),
        "a": {"cp": [uid(0x1001), 0]}, "b": {"cp": [uid(0x4011), 0]},
    }));
    let m = fx.build();
    let q = Q::new(&m);
    let partition = q.get::<PotentialPartition>(());
    let mut checked = 0;
    for p in partition.potentials() {
        for w in &q.get::<PhysicalWiresOf>(p.id).wires {
            let mut w = w.clone();
            orient_wire(&q, &mut w);
            // The terminal, at the top, is From; before FT-100 it had no cell and lost the orientation.
            assert_eq!(w.ends[0].cp.function, id(0x4011), "{w:?}");
            assert_eq!(w.ends[1].cp.function, id(0x1001));
            checked += 1;
        }
    }
    assert_eq!(checked, 1);
    // The wire is numbered, From at the terminal's cell (column 4 + 0, row A).
    let rows = order(&m);
    assert_eq!(label(&rows, 0x2001).as_deref(), Some("101.1"));
}

/// The source-output FnDef `FN_SRC` (one CP `+`) and its symbol `SYM_SRC`.
fn source_def(fx: &mut Fx) {
    fx.lib_item(
        "fndef",
        FN_SRC,
        json!({
            "meta": {}, "category": ["Misc", "Misc", "Source"], "contact_kind": "pole",
            "cps": [{"idx": 0, "designation": "+", "source_output": true}],
        }),
    );
    fx.symbol(SYM_SRC, FN_SRC, json!("origin"), &[(0, 0, 0, 0)]);
}

/// A source `n` (CP 0 is a source output) drawn on P1 in cell `(column, row)`.
fn source(fx: &mut Fx, n: u128, column: i64, row: i64) {
    let p = at(column, row);
    fx.device(n + 0x20_000, "G", Some(n as u32));
    fx.function(n, Some(n + 0x20_000), FN_SRC, true);
    fx.rep(n + REP, P1, SYM_SRC, &[n], p.x.0, p.y.0);
}

/// A FnDef `n` with the CPs `cps` (JSON), `contact_kind` and `internal` links, and a symbol `sym` drawing
/// them all at the origin.
fn def_with(fx: &mut Fx, n: u128, sym: u128, contact_kind: &str, cps: Value, internal: Value) {
    let count = cps.as_array().expect("a list").len() as u16;
    fx.lib_item(
        "fndef",
        n,
        json!({
            "meta": {}, "category": ["Misc", "Misc", "Multi"], "contact_kind": contact_kind,
            "cps": cps, "internal": internal,
        }),
    );
    let cps: Vec<(u16, u16, i64, i64)> = (0..count).map(|i| (i, i, 0, 0)).collect();
    fx.symbol(sym, n, json!("origin"), &cps);
}

/// A multi-CP function `n` of `def` (symbol `sym`) of kind `kind` (`None` is normal), drawn on P1 in cell
/// `(column, row)`.
fn place(fx: &mut Fx, n: u128, (def, sym): (u128, u128), kind: Option<&str>, column: i64, row: i64) {
    let p = at(column, row);
    fx.device(n + 0x20_000, "K", Some(n as u32));
    fx.function(n, Some(n + 0x20_000), def, true);
    if let Some(k) = kind {
        fx.with("kind", json!(k));
    }
    fx.rep(n + REP, P1, sym, &[n], p.x.0, p.y.0);
}

fn cpr(function: u128, idx: u16) -> CpRef {
    CpRef::new(id(function), idx)
}

/// The flow distances of the potential that holds CP `idx` of `function`.
fn flow(q: &Q, function: u128, idx: u16) -> Arc<CpFlowDistances> {
    let partition = q.get::<PotentialPartition>(());
    let p = partition
        .potential_at(cpr(function, idx).key())
        .unwrap_or_else(|| panic!("no potential at {function:#x}:{idx}"));
    q.get::<CpFlowDistanceOf>(p.id)
}

/// Every wire of the model oriented, by wire ID: its From end and its To end.
fn oriented(q: &Q) -> BTreeMap<u128, (CpRef, CpRef)> {
    let partition = q.get::<PotentialPartition>(());
    let mut out = BTreeMap::new();
    for p in partition.potentials() {
        for w in &q.get::<PhysicalWiresOf>(p.id).wires {
            let mut w = w.clone();
            orient_wire(q, &mut w);
            out.insert(w.id.as_uuid().as_u128(), (w.ends[0].cp, w.ends[1].cp));
        }
    }
    out
}

/// FT-101, doc 08 §3.8 rule 2: on one page the end with the smaller flow distance is From, before the anchor
/// cell decides. A source S (a source-output CP) in cell (8, D) feeds B in (2, A), and B feeds A in (0, A):
/// by cell A and B would come first, by flow distance S → B and B → A.
#[test]
fn the_flow_distance_decides_before_the_anchor_cell() {
    let mut fx = fixture();
    source_def(&mut fx);
    source(&mut fx, 0x1000, 8, 3);
    node(&mut fx, 0x1002, P1, 2, 0);
    node(&mut fx, 0x1001, P1, 0, 0);
    wire(&mut fx, 0x2001, Some(P1), 0x1002, 0x1000);
    wire(&mut fx, 0x2002, Some(P1), 0x1001, 0x1002);
    let m = fx.build();
    let q = Q::new(&m);
    let partition = q.get::<PotentialPartition>(());
    let mut from = Vec::new();
    for p in partition.potentials() {
        let d = q.get::<CpFlowDistanceOf>(p.id);
        assert_eq!(d.entries.len(), 1, "the source output is the one entry");
        for w in &q.get::<PhysicalWiresOf>(p.id).wires {
            let mut w = w.clone();
            orient_wire(&q, &mut w);
            from.push((w.id, w.ends[0].cp.function, w.ends[1].cp.function));
        }
    }
    from.sort();
    assert_eq!(
        from,
        vec![
            (id(0x2001), id(0x1000), id(0x1002)),
            (id(0x2002), id(0x1002), id(0x1001)),
        ]
    );
}

/// Entry through a pass-through (doc 08 §3.8 rule 2, FT-101): the potential behind the pair is entered at the
/// pair's CP on its side, at distance 0. P (cell (10, F)) is before the source S in the order of cells and X
/// is the first cell of the page, so rule 3 alone would start both wires at the wrong end.
#[test]
fn an_entry_through_a_pass_through_has_distance_zero() {
    let mut fx = fixture();
    source_def(&mut fx);
    def_with(
        &mut fx,
        FN_PASS,
        SYM_PASS,
        "pole",
        json!([{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}]),
        json!([{"a": 0, "b": 1, "kind": {"pass_through": {"opens": true}}}]),
    );
    source(&mut fx, 0x1000, 14, 6);
    place(&mut fx, 0x1001, (FN_PASS, SYM_PASS), None, 10, 5);
    node(&mut fx, 0x1002, P1, 0, 0);
    wire_cp(&mut fx, 0x2001, Some(P1), (0x1001, 0), (0x1000, 0), Value::Null);
    wire_cp(&mut fx, 0x2002, Some(P1), (0x1002, 0), (0x1001, 1), Value::Null);
    let m = fx.build();
    let q = Q::new(&m);
    let upstream = flow(&q, 0x1000, 0);
    assert_eq!(upstream.entries, vec![cpr(0x1000, 0)]);
    assert_eq!(upstream.get(cpr(0x1000, 0)), Some(0));
    assert_eq!(upstream.get(cpr(0x1001, 0)), Some(1));
    let downstream = flow(&q, 0x1002, 0);
    assert_eq!(
        downstream.entries,
        vec![cpr(0x1001, 1)],
        "the pair's CP on its side"
    );
    assert_eq!(downstream.get(cpr(0x1001, 1)), Some(0));
    assert_eq!(downstream.get(cpr(0x1002, 0)), Some(1));
    assert_eq!(
        downstream.get(cpr(0x1001, 0)),
        None,
        "the upstream side is another potential"
    );
    let from = oriented(&q);
    assert_eq!(from[&0x2001], (cpr(0x1000, 0), cpr(0x1001, 0)));
    assert_eq!(from[&0x2002], (cpr(0x1001, 1), cpr(0x1002, 0)));
}

/// Duplicated pins (doc 03 §9.3): the two `+` CPs of a black box are one pin, so they take the same distance
/// (no hop between them). Were the join a hop, CP 1 and X would tie at 2 and the cell (X first) would decide.
#[test]
fn duplicated_pins_of_a_black_box_cost_no_hop() {
    let mut fx = fixture();
    source_def(&mut fx);
    def_with(
        &mut fx,
        FN_BOX,
        SYM_BOX,
        "pole",
        json!([{"idx": 0, "designation": "+"}, {"idx": 1, "designation": "+"}]),
        json!([]),
    );
    source(&mut fx, 0x1000, 14, 6);
    place(&mut fx, 0x1001, (FN_BOX, SYM_BOX), Some("black_box"), 10, 5);
    node(&mut fx, 0x1002, P1, 0, 0);
    wire_cp(&mut fx, 0x2001, Some(P1), (0x1001, 0), (0x1000, 0), Value::Null);
    wire_cp(&mut fx, 0x2002, Some(P1), (0x1002, 0), (0x1001, 1), Value::Null);
    let m = fx.build();
    let q = Q::new(&m);
    let d = flow(&q, 0x1000, 0);
    assert_eq!(d.entries, vec![cpr(0x1000, 0)]);
    assert_eq!(d.get(cpr(0x1000, 0)), Some(0));
    assert_eq!(d.get(cpr(0x1001, 0)), Some(1));
    assert_eq!(
        d.get(cpr(0x1001, 1)),
        Some(1),
        "the duplicated pin: the same distance"
    );
    assert_eq!(d.get(cpr(0x1002, 0)), Some(2));
    let from = oriented(&q);
    assert_eq!(from[&0x2001], (cpr(0x1000, 0), cpr(0x1001, 0)));
    assert_eq!(from[&0x2002], (cpr(0x1001, 1), cpr(0x1002, 0)));
}

/// A `Bridge` connection and the terminal level rule (side 1 and side 2 of one level are shorted) are one hop
/// each. S - T1:0 (wire), T1:0 ~ T1:1 (level), T1:1 ~ T2:0 (bridge), T2:0 ~ T2:1 (level), T2:1 - X (wire).
#[test]
fn a_bridge_and_a_terminal_level_short_are_one_hop_each() {
    let mut fx = fixture();
    source_def(&mut fx);
    def_with(
        &mut fx,
        FN_TERM2,
        SYM_TERM,
        "terminal",
        json!([
            {"idx": 0, "designation": "1", "level": 1, "side": "side1", "clamp": {"clamp": "a"}},
            {"idx": 1, "designation": "1", "level": 1, "side": "side2", "clamp": {"clamp": "b"}},
        ]),
        json!([]),
    );
    let term = (FN_TERM2, SYM_TERM);
    source(&mut fx, 0x1000, 14, 6);
    place(&mut fx, 0x1001, term, None, 10, 5);
    place(&mut fx, 0x1002, term, None, 11, 5);
    node(&mut fx, 0x1003, P1, 0, 0);
    wire_cp(&mut fx, 0x2001, Some(P1), (0x1001, 0), (0x1000, 0), Value::Null);
    wire_cp(
        &mut fx,
        0x2002,
        None,
        (0x1001, 1),
        (0x1002, 0),
        json!({"bridge": {"kind": "plug_in", "group": 1, "shaft": 1}}),
    );
    wire_cp(&mut fx, 0x2003, Some(P1), (0x1003, 0), (0x1002, 1), Value::Null);
    let m = fx.build();
    let q = Q::new(&m);
    let d = flow(&q, 0x1000, 0);
    assert_eq!(d.entries, vec![cpr(0x1000, 0)]);
    let got: Vec<Option<u32>> = [
        (0x1000, 0),
        (0x1001, 0),
        (0x1001, 1),
        (0x1002, 0),
        (0x1002, 1),
        (0x1003, 0),
    ]
    .into_iter()
    .map(|(f, i)| d.get(cpr(f, i)))
    .collect();
    assert_eq!(got, [0, 1, 2, 3, 4, 5].map(Some));
    let from = oriented(&q);
    assert_eq!(from[&0x2001], (cpr(0x1000, 0), cpr(0x1001, 0)));
    assert_eq!(from[&0x2003], (cpr(0x1002, 1), cpr(0x1003, 0)));
}

/// A rail bond joins the PE CPs of the rail-bonded terminals of one location into one potential, but it is not
/// a hop: T2's PE CP and the X wired to it have no distance, so the wire between them falls to the cell
/// (X first). Were the bond a hop, T2:PE would be 2 and X 3, and T2 would start the wire.
#[test]
fn a_rail_bond_is_not_a_hop() {
    const STRIP: u128 = 0x4000;
    const LOC: u128 = 0x4100;
    let mut fx = fixture();
    source_def(&mut fx);
    def_with(
        &mut fx,
        FN_RAIL,
        SYM_RAIL,
        "pole",
        json!([{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "PE", "clamp": "rail_foot"}]),
        json!([]),
    );
    fx.device(STRIP, "X", Some(1)).with("strip", json!({}));
    fx.node(LOC, "location", "A", None);
    for (dev, f, column) in [(0x4001, 0x1001, 10), (0x4002, 0x1002, 11)] {
        fx.device(dev, "X", None).with(
            "terminal",
            json!({"strip": uid(STRIP), "seq": dev as u32, "designation": dev.to_string(), "rail_bonded": true}),
        );
        fx.with("structure", json!({"location": uid(LOC)}));
        let p = at(column, 5);
        fx.function(f, Some(dev), FN_RAIL, true);
        fx.rep(f + REP, P1, SYM_RAIL, &[f], p.x.0, p.y.0);
    }
    source(&mut fx, 0x1000, 14, 6);
    node(&mut fx, 0x1003, P1, 0, 0);
    wire_cp(&mut fx, 0x2001, Some(P1), (0x1001, 1), (0x1000, 0), Value::Null);
    wire_cp(&mut fx, 0x2002, Some(P1), (0x1003, 0), (0x1002, 1), Value::Null);
    let m = fx.build();
    let q = Q::new(&m);
    let d = flow(&q, 0x1000, 0);
    let partition = q.get::<PotentialPartition>(());
    let of = |f: u128, i: u16| partition.potential_at(cpr(f, i).key()).map(|p| p.id);
    assert_eq!(of(0x1001, 1), of(0x1002, 1), "the rail bond joins the PE CPs");
    assert_eq!(of(0x1002, 1), of(0x1003, 0));
    assert_eq!(d.entries, vec![cpr(0x1000, 0)]);
    assert_eq!(d.get(cpr(0x1000, 0)), Some(0));
    assert_eq!(d.get(cpr(0x1001, 1)), Some(1));
    assert_eq!(
        d.get(cpr(0x1002, 1)),
        None,
        "reachable only through the rail bond"
    );
    assert_eq!(d.get(cpr(0x1003, 0)), None);
    let from = oriented(&q);
    assert_eq!(from[&0x2001], (cpr(0x1000, 0), cpr(0x1001, 1)));
    assert_eq!(from[&0x2002], (cpr(0x1003, 0), cpr(0x1002, 1)));
}

/// Rule 2 needs two reached ends with different distances. A potential without a source has no entry and no
/// distances; two ends at the same distance tie. Both fall to the anchor cell. The tie: S feeds Y:0 and Z:1,
/// each function shorts its two CPs, and the wire joins Y:1 and Z:0, both at distance 2.
#[test]
fn unreached_and_tied_ends_fall_to_the_anchor_cell() {
    let mut fx = fixture();
    source_def(&mut fx);
    let shorted = (FN_SHORT, SYM_SHORT);
    def_with(
        &mut fx,
        FN_SHORT,
        SYM_SHORT,
        "pole",
        json!([{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}]),
        json!([{"a": 0, "b": 1, "kind": "short"}]),
    );
    // No entry: Y (row B) comes before X (row E) although the wire is written from X.
    node(&mut fx, 0x1101, P1, 7, 4);
    node(&mut fx, 0x1102, P1, 1, 1);
    wire(&mut fx, 0x2101, Some(P1), 0x1101, 0x1102);
    // The tie: Z (row D) is before Y (row E) although the wire is written from Y.
    source(&mut fx, 0x1000, 14, 6);
    place(&mut fx, 0x1001, shorted, None, 9, 4);
    place(&mut fx, 0x1002, shorted, None, 3, 3);
    wire_cp(&mut fx, 0x2001, Some(P1), (0x1001, 0), (0x1000, 0), Value::Null);
    wire_cp(&mut fx, 0x2002, Some(P1), (0x1002, 1), (0x1000, 0), Value::Null);
    wire_cp(&mut fx, 0x2003, Some(P1), (0x1001, 1), (0x1002, 0), Value::Null);
    let m = fx.build();
    let q = Q::new(&m);
    let none = flow(&q, 0x1101, 0);
    assert!(none.entries.is_empty() && none.distance.is_empty(), "{none:?}");
    let tied = flow(&q, 0x1000, 0);
    assert_eq!(tied.get(cpr(0x1001, 0)), Some(1));
    assert_eq!(tied.get(cpr(0x1002, 1)), Some(1));
    assert_eq!(tied.get(cpr(0x1001, 1)), Some(2));
    assert_eq!(tied.get(cpr(0x1002, 0)), Some(2));
    let from = oriented(&q);
    assert_eq!(from[&0x2101], (cpr(0x1102, 0), cpr(0x1101, 0)));
    assert_eq!(from[&0x2003], (cpr(0x1002, 0), cpr(0x1001, 1)));
    // The wires from the source are still decided by the distance (S is the last cell).
    assert_eq!(from[&0x2001], (cpr(0x1000, 0), cpr(0x1001, 0)));
    assert_eq!(from[&0x2002], (cpr(0x1000, 0), cpr(0x1002, 1)));
}

/// A bond wire joins two potentials, so rule 2 does not compare its ends even when both have distances: A is
/// at distance 1 in its potential and B at distance 2 in the other, yet B (the first cell) starts the wire.
#[test]
fn a_bond_wire_falls_to_the_anchor_cell() {
    let mut fx = fixture();
    source_def(&mut fx);
    // Potential 1: S1 - A. Potential 2: S2 - C - B.
    source(&mut fx, 0x1000, 14, 6);
    source(&mut fx, 0x1010, 15, 7);
    node(&mut fx, 0x1001, P1, 10, 5);
    node(&mut fx, 0x1011, P1, 12, 6);
    node(&mut fx, 0x1012, P1, 0, 0);
    wire(&mut fx, 0x2001, Some(P1), 0x1001, 0x1000);
    wire(&mut fx, 0x2002, Some(P1), 0x1011, 0x1010);
    wire(&mut fx, 0x2003, Some(P1), 0x1012, 0x1011);
    // The bond wire, written from A to B.
    wire(&mut fx, 0x2004, Some(P1), 0x1001, 0x1012);
    fx.push(json!({
        "$k": "bond", "id": uid(0x4001), "a": {"cp": [uid(0x1001), 0]}, "b": {"cp": [uid(0x1012), 0]},
        "kind": "pelv_0v_pe", "via": uid(0x2004),
    }));
    let m = fx.build();
    let q = Q::new(&m);
    let partition = q.get::<PotentialPartition>(());
    assert_eq!(partition.potential_count(), 2);
    let (a, b) = (flow(&q, 0x1001, 0), flow(&q, 0x1012, 0));
    assert_eq!(a.get(cpr(0x1001, 0)), Some(1));
    assert_eq!(b.get(cpr(0x1012, 0)), Some(2));
    let wires: Vec<_> = partition
        .potentials()
        .flat_map(|p| q.get::<PhysicalWiresOf>(p.id).wires.clone())
        .filter(|w| w.id == id(0x2004))
        .collect();
    assert!(!wires.is_empty());
    assert!(wires.iter().all(|w| w.potentials.len() == 2), "{wires:?}");
    let from = oriented(&q);
    assert_eq!(from[&0x2004], (cpr(0x1012, 0), cpr(0x1001, 0)));
    // The wires inside a potential are still decided by the distance.
    assert_eq!(from[&0x2001], (cpr(0x1000, 0), cpr(0x1001, 0)));
    assert_eq!(from[&0x2002], (cpr(0x1010, 0), cpr(0x1011, 0)));
    assert_eq!(from[&0x2003], (cpr(0x1011, 0), cpr(0x1012, 0)));
}
