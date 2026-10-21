// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wire numbers (doc 08 §5.2, doc 20 §8.1, FR-NUM-019, FR-NUM-028; FT-100) on synthetic models: devices with
//! one CP each, drawn at chosen cells of the QET frame, wired by single connections.
//!
//! - `column_comes_before_row`: the From end's cell is compared by column, then row.
//! - `the_to_cell_is_the_second_key`: wires from one From cell are ordered by the To cell, before the wire ID.
//! - `the_to_page_compares_before_the_to_cell`: a wire to another page sorts after the wires that stay on the
//!   From page, whatever the To cell.
//! - `a_tie_is_decided_by_the_wire_id_and_keeps_its_range`: equal cells, the wire ID decides, `tie` gives the
//!   counter range of the group.
//! - `a_core_takes_no_counter`: a cable core inside a page run is in the order but unnumbered.
//! - `a_wire_on_no_page_is_last_and_unnumbered`.
//! - `a_terminal_in_a_strip_segment_has_a_cell_for_the_orientation`: a wire from a segment-only terminal to a
//!   device below it on one page starts at the terminal.
//! - `counters_run_per_page_and_the_page_token_is_the_folio_label`.

mod common;

use common::{Fx, id, uid};
use ew_derive::{PhysicalWiresOf, PotentialPartition, Q, WireNumbers, orient_wire};
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
const SYM_SEG: u128 = 0x513;
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
    let mut r = json!({
        "$k": "connection", "id": uid(n), "a": {"cp": [uid(a), 0]}, "b": {"cp": [uid(b), 0]},
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
    // Four wires with the same From cell and To cell, and one more elsewhere that sorts first.
    node(&mut fx, 0x1001, P1, 4, 1);
    node(&mut fx, 0x1002, P1, 4, 1);
    node(&mut fx, 0x1011, P1, 9, 6);
    node(&mut fx, 0x1012, P1, 9, 6);
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
