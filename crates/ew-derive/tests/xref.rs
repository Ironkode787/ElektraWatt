// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! Cross-references (WP-S1.08; doc 06 §2.6-§2.8, FR-XRF-001 ... FR-XRF-025): relation, text, links and the
//! notes, on small models built with `Fx`. Every page uses the built-in default frame (17 columns of
//! 15.875 mm after a 5.29 mm header band, rows A-H); `at(column, row)` gives a point inside a cell and
//! `cell(column, row)` the cell's label, so the expected texts are written from the placed cells.
//!
//! - `unpaired_ip_prints_nothing_and_is_noted`
//! - `star_source_lists_every_target_in_page_order_with_the_matrix`
//! - `star_overflow_after_max`
//! - `chain_mode_odd_count_leaves_the_last_unpaired`
//! - `scope_location_separates_names`
//! - `external_point_prints_location_and_device`
//! - `target_without_anchor_cell_prints_question_marks`
//! - `device_with_two_main_functions_is_refused_at_load`
//! - `device_with_two_main_functions_is_noted_on_a_raw_model`
//! - `aux_without_placed_main_prints_nothing`
//! - `aux_inside_its_own_black_box_prints_nothing`
//! - `same_page_zone_only`
//! - `structure_prefix_when_different`
//! - `malformed_xref_settings_fall_back_to_the_profile`
//! - `contact_image_rows`
//! - `coil_drawn_twice_adds_no_contact_rows`
//! - `contact_rows_sort_by_sequence_then_designation`
//! - `partial_xref_settings_apply_with_defaults`
//! - `strip_segments_are_never_main_representations_or_targets`
//! - `zone_only_drops_page_parts_with_their_separators`
//! - `empty_formats_print_nothing`
//! - `star_formats_only_the_shown_items`
//! - `page_index_counts_dividers_but_not_continuations`
//! - `dangling_external_ref_is_refused_at_load`
//! - `dangling_external_ref_prints_nothing_and_is_noted_on_a_raw_model`
//! - `hostile_shapes_are_refused_at_load`
//! - `hostile_shapes_on_raw_models_print_nothing`
//! - `same_function_reps_refer_to_the_main_rep`
//! - `ten_thousand_targets_are_formatted_without_quadratic_work`
//! - `xref_texts_do_not_depend_on_record_order`
//! - `hostile_shapes_do_not_panic`

mod common;

use common::{Fx, id, uid};
use ew_derive::{DeriveNote, PairMode, Q, XrefKind, XrefRelationOf, XrefSettingsOf, XrefText, XrefTextOf};
use ew_ids::Id;
use ew_model::{AnyRow, Entity, Model, ModelError, Op, Rev};
use ew_schema::{Device, ExternalRef, FnRole, Function, LibDocument, Project, Record, Representation};
use serde_json::{Value, json};

const FNDEF: u128 = 0x500;
const FNDEF_IP: u128 = 0x501;
const FNDEF_NO_CP: u128 = 0x504;
const FNDEF_A: u128 = 0x505;
const FNDEF_B: u128 = 0x506;
const FNDEF_C: u128 = 0x507;
const SYM: u128 = 0x510;
const SYM_IP: u128 = 0x511;
const SYM_NO_CP: u128 = 0x514;
const SYM_A: u128 = 0x515;
const SYM_B: u128 = 0x516;
const SYM_C: u128 = 0x517;
const EN81346: &str = "EN81346-2:2009";

/// The page `k` (ID `0x100 + k`).
const fn pg(k: u128) -> u128 {
    0x100 + k
}

/// A point inside the cell `(column, row)` of the default frame.
fn at(column: i64, row: i64) -> (i64, i64) {
    (10_000_000 + column * 15_875_000, 15_000_000 + row * 21_166_667)
}

/// The label of the cell `(column, row)` of the default frame.
fn cell(column: i64, row: i64) -> String {
    format!("{}{column}", "ABCDEFGH".as_bytes()[row as usize] as char)
}

/// Function definitions and symbols (origin anchors), no pages.
fn lib() -> Fx {
    let mut fx = Fx::new();
    fx.fndef(FNDEF, ["Misc", "Misc", "Plain"], &[(0, "1"), (1, "2")]);
    fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)]);
    fx.fndef(FNDEF_IP, ["Misc", "Misc", "Ip"], &[(0, "1")]);
    fx.symbol(SYM_IP, FNDEF_IP, json!("origin"), &[(0, 0, 0, 0)]);
    fx.fndef(FNDEF_NO_CP, ["Misc", "Misc", "Ip"], &[]);
    fx.symbol(SYM_NO_CP, FNDEF_NO_CP, json!("origin"), &[]);
    fx.fndef(FNDEF_A, ["Misc", "Misc", "Contact"], &[(0, "13"), (1, "14")]);
    fx.symbol(
        SYM_A,
        FNDEF_A,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)],
    );
    fx.fndef(FNDEF_B, ["Misc", "Misc", "Contact"], &[(0, "21"), (1, "22")]);
    fx.symbol(
        SYM_B,
        FNDEF_B,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)],
    );
    fx.fndef(FNDEF_C, ["Misc", "Misc", "Contact"], &[(0, "113"), (1, "114")]);
    fx.symbol(
        SYM_C,
        FNDEF_C,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)],
    );
    fx
}

/// The library and the pages with folios 1 to 5 (`pg(1)` .. `pg(5)`), no structure.
fn base() -> Fx {
    let mut fx = lib();
    for k in 1..=5 {
        fx.page(pg(k), k as u32, None, "multi_line", &[], "page");
    }
    fx
}

/// Project settings: the identifier set, an optional IP pairing and further members.
fn settings(fx: &mut Fx, ip_pairing: Option<Value>, more: &[(&str, Value)]) {
    let mut s = json!({"identifier_set": EN81346});
    if let Some(p) = ip_pairing {
        s["ip_pairing"] = p;
    }
    for (k, v) in more {
        s[*k] = v.clone();
    }
    fx.project_set("settings", s);
}

/// The profile's xref format with members replaced.
fn format_with(changes: &[(&str, Value)]) -> Value {
    let mut f = json!({
        "key": "test", "ip": "{page}-{row}{column}", "aux_to_main": "({page}-{row}{column})",
        "main_to_aux": "∞ {list}", "structure": "when_different", "same_page": "full",
        "matrix": {"per_row": 3, "max": 9, "overflow": "…"},
    });
    for (k, v) in changes {
        f[*k] = v.clone();
    }
    f
}

/// The representation ID of function `n` (see `ip` and `func`).
const fn rep_of(n: u128) -> u128 {
    n + 0x10_000
}

/// An interruption point `n` named `name` with direction `dir`, drawn on `page` in cell `(column, row)`.
fn ip(fx: &mut Fx, n: u128, page: u128, name: &str, dir: &str, (column, row): (i64, i64)) {
    fx.push(json!({
        "$k": "function", "id": uid(n), "def": uid(FNDEF_IP),
        "kind": {"ip": {"name": name, "direction": dir}},
    }));
    let (x, y) = at(column, row);
    fx.rep(rep_of(n), page, SYM_IP, &[n], x, y);
}

/// A function `n` of device `device` with definition and symbol `(def, sym)`, drawn on `page` in a cell;
/// `main` sets the role.
fn func(
    fx: &mut Fx,
    n: u128,
    device: u128,
    (def, sym): (u128, u128),
    main: bool,
    page: u128,
    (column, row): (i64, i64),
) {
    fx.function(n, Some(device), def, main);
    let (x, y) = at(column, row);
    fx.rep(rep_of(n), page, sym, &[n], x, y);
}

/// The ID of the representation of function `n`.
fn rid(n: u128) -> Id<Representation> {
    id(rep_of(n))
}

fn text(q: &Q, rep: u128) -> String {
    q.get::<XrefTextOf>(id(rep)).text.clone()
}

/// The items of a text's links, in text order: `(shown text, target rep)`.
fn shown(t: &XrefText) -> Vec<(String, Id<Representation>)> {
    t.links
        .iter()
        .map(|l| (t.text[l.range.clone()].to_owned(), l.rep))
        .collect()
}

fn xref_notes(q: &mut Q, code: &str) -> Vec<(String, String)> {
    q.diagnostics()
        .notes()
        .into_iter()
        .filter_map(|n| match n {
            DeriveNote::Xref {
                code: c,
                subject,
                detail,
            } if c == code => Some((subject, detail)),
            _ => None,
        })
        .collect()
}

fn core_051(q: &mut Q) -> usize {
    q.diagnostics()
        .notes()
        .into_iter()
        .filter(|n| {
            matches!(
                n,
                DeriveNote::Connectivity {
                    code: "EW-CORE-051",
                    ..
                }
            )
        })
        .count()
}

fn malformed(q: &mut Q) -> Vec<String> {
    q.diagnostics()
        .notes()
        .into_iter()
        .filter_map(|n| match n {
            DeriveNote::MalformedShim { member, detail } if member == "settings.xref_format" => Some(detail),
            _ => None,
        })
        .collect()
}

/// The model of `fx`, or the error that refuses the records (`Fx::build` without the panic).
fn load(fx: &Fx) -> Result<Model, ModelError> {
    let project: Project = serde_json::from_value(fx.project.clone()).expect("project parses");
    let records: Vec<Record> = fx
        .records
        .iter()
        .map(|r| serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("record {r}: {e}")))
        .collect();
    let lib: Vec<LibDocument> = fx
        .lib
        .iter()
        .map(|d| serde_json::from_value(d.clone()).unwrap_or_else(|e| panic!("lib document {d}: {e}")))
        .collect();
    Model::from_records(project, records, lib)
}

/// The integrity code under which the model refuses to load `fx`.
fn refused_with(fx: &Fx) -> &'static str {
    match load(fx) {
        Err(ModelError::Invariant { id, .. }) => id,
        Err(other) => panic!("refused, but not by an invariant: {other:?}"),
        Ok(_) => panic!("the model accepted the records"),
    }
}

/// A raw model: `m` with `ops` applied through `Model::apply_unchecked`, which skips the invariants. The
/// model refuses such records at load (and every command refuses to produce them), so the derivation's
/// fallback branches for them are reached only this way.
fn raw(m: &Model, ops: &[Op]) -> Model {
    let m2 = m.apply_unchecked(ops, Rev(1)).expect("ops apply");
    assert!(
        m2.integrity_check().is_err(),
        "the probe violates an invariant, else the model would have refused it at load"
    );
    m2
}

/// The row of record `key`.
fn row_of<E: Entity>(m: &Model, key: Id<E::Marker>) -> AnyRow {
    m.row(&E::key(key)).expect("the record exists")
}

#[test]
fn unpaired_ip_prints_nothing_and_is_noted() {
    let mut fx = base();
    ip(&mut fx, 0x1101, pg(1), "X", "source", (2, 1));
    let m = fx.build();
    let mut q = Q::new(&m);
    let rel = q.get::<XrefRelationOf>(rid(0x1101));
    assert_eq!(rel.kind, XrefKind::None);
    assert!(rel.targets.is_empty());
    let t = q.get::<XrefTextOf>(rid(0x1101));
    assert_eq!(t.text, "");
    assert!(t.links.is_empty());
    assert_eq!(core_051(&mut q), 1, "{:?}", q.diagnostics().notes());
}

/// The star of the sort-order tests: a source on page 1 at B1 and five targets on scrambled pages.
/// Targets are `(function, folio, column, row)`; the page order is the folio order.
const STAR_TARGETS: [(u128, u32, i64, i64); 5] = [
    (0x1105, 40, 1, 0),
    (0x1102, 20, 4, 2),
    (0x1104, 20, 4, 1),
    (0x1103, 30, 0, 0),
    (0x1101, 20, 3, 1),
];

fn star_fixture() -> Fx {
    let mut fx = lib();
    for (k, folio) in [(1, 1), (4, 40), (2, 20), (3, 30)] {
        fx.page(pg(k), folio, None, "multi_line", &[], "page");
    }
    let page = |folio: u32| {
        pg(match folio {
            20 => 2,
            30 => 3,
            _ => 4,
        })
    };
    ip(&mut fx, 0x1100, pg(1), "S", "source", (1, 1));
    for (n, folio, column, row) in STAR_TARGETS {
        ip(&mut fx, n, page(folio), "S", "target", (column, row));
    }
    fx
}

#[test]
fn star_source_lists_every_target_in_page_order_with_the_matrix() {
    let m = star_fixture().build();
    let q = Q::new(&m);
    // Page order, then row, then column: 20: B3 (0x1101), B4 (0x1104), C4 (0x1102); 30: A0; 40: A1.
    let expect = [
        (0x1101, "20-B3"),
        (0x1104, "20-B4"),
        (0x1102, "20-C4"),
        (0x1103, "30-A0"),
        (0x1105, "40-A1"),
    ];
    let source = q.get::<XrefTextOf>(rid(0x1100));
    assert_eq!(source.text, "20-B3, 20-B4, 20-C4\n30-A0, 40-A1");
    assert_eq!(
        shown(&source),
        expect.map(|(n, s)| (s.to_owned(), rid(n))).to_vec(),
        "links in text order"
    );
    let rel = q.get::<XrefRelationOf>(rid(0x1100));
    assert_eq!(
        rel.kind,
        XrefKind::Ip {
            mode: PairMode::Star,
            star_source: true
        }
    );
    assert_eq!(rel.targets.len(), 5);
    for (n, _) in expect {
        // Each target prints the source's cell on page 1.
        let t = q.get::<XrefTextOf>(rid(n));
        assert_eq!(t.text, format!("1-{}", cell(1, 1)));
        assert_eq!(shown(&t), vec![(t.text.clone(), rid(0x1100))]);
        let rel = q.get::<XrefRelationOf>(rid(n));
        assert!(matches!(
            rel.kind,
            XrefKind::Ip {
                star_source: false,
                ..
            }
        ));
    }
}

#[test]
fn star_overflow_after_max() {
    let mut fx = base();
    ip(&mut fx, 0x1100, pg(1), "S", "source", (0, 0));
    for i in 0..11 {
        ip(&mut fx, 0x1101 + i as u128, pg(2), "S", "target", (i, 0));
    }
    let m = fx.build();
    let q = Q::new(&m);
    let t = q.get::<XrefTextOf>(rid(0x1100));
    assert_eq!(t.text, "2-A0, 2-A1, 2-A2\n2-A3, 2-A4, 2-A5\n2-A6, 2-A7, 2-A8, …");
    assert_eq!(t.links.len(), 9, "the overflow mark is no item");
    assert_eq!(q.get::<XrefRelationOf>(rid(0x1100)).targets.len(), 11);
    assert_eq!(shown(&t)[8], ("2-A8".to_owned(), rid(0x1109)));
}

#[test]
fn chain_mode_odd_count_leaves_the_last_unpaired() {
    let mut fx = base();
    settings(&mut fx, Some(json!({"mode": "chain", "scope": "project"})), &[]);
    // Chain order: page position, row, column.
    ip(&mut fx, 0x1101, pg(1), "C", "undirected", (0, 0));
    ip(&mut fx, 0x1102, pg(1), "C", "undirected", (5, 0));
    ip(&mut fx, 0x1103, pg(2), "C", "undirected", (2, 2));
    let m = fx.build();
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1101)), format!("1-{}", cell(5, 0)));
    assert_eq!(text(&q, rep_of(0x1102)), format!("1-{}", cell(0, 0)));
    assert_eq!(text(&q, rep_of(0x1103)), "");
    let rel = q.get::<XrefRelationOf>(rid(0x1101));
    assert!(matches!(
        rel.kind,
        XrefKind::Ip {
            mode: PairMode::Chain,
            ..
        }
    ));
    assert_eq!(rel.targets.len(), 1);
    assert_eq!(q.get::<XrefRelationOf>(rid(0x1103)).kind, XrefKind::None);
    assert_eq!(core_051(&mut q), 1, "{:?}", q.diagnostics().notes());
}

#[test]
fn scope_location_separates_names() {
    // FR-XRF-010: with scope location, "L-1" pairs inside its location only.
    let mut fx = lib();
    fx.node(0x6001, "location", "A", None);
    fx.node(0x6002, "location", "B", None);
    settings(&mut fx, Some(json!({"mode": "star", "scope": "location"})), &[]);
    fx.page(pg(1), 1, None, "multi_line", &[("location", 0x6001)], "a1");
    fx.page(pg(2), 2, None, "multi_line", &[("location", 0x6001)], "a2");
    fx.page(pg(3), 3, None, "multi_line", &[("location", 0x6002)], "b1");
    fx.page(pg(4), 4, None, "multi_line", &[("location", 0x6002)], "b2");
    ip(&mut fx, 0x1101, pg(1), "L-1", "source", (1, 0));
    ip(&mut fx, 0x1102, pg(2), "L-1", "target", (2, 0));
    ip(&mut fx, 0x1103, pg(3), "L-1", "source", (3, 0));
    ip(&mut fx, 0x1104, pg(4), "L-1", "target", (4, 0));
    let m = fx.build();
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1101)), format!("2-{}", cell(2, 0)));
    assert_eq!(text(&q, rep_of(0x1102)), format!("1-{}", cell(1, 0)));
    assert_eq!(text(&q, rep_of(0x1103)), format!("4-{}", cell(4, 0)));
    assert_eq!(text(&q, rep_of(0x1104)), format!("3-{}", cell(3, 0)));
    assert_eq!(core_051(&mut q), 0, "{:?}", q.diagnostics().notes());
}

#[test]
fn external_point_prints_location_and_device() {
    let mut fx = base();
    // The doc example "+J-E-206": one location node whose code is "J-E-206".
    fx.node(0x6003, "location", "J-E-206", None);
    // A nested J / E / 206 path.
    fx.node(0x6004, "location", "J", None);
    fx.node(0x6005, "location", "E", Some(0x6004));
    fx.node(0x6006, "location", "206", Some(0x6005));
    fx.push(json!({
        "$k": "external_ref", "id": uid(0x7101), "location": uid(0x6003), "device": "JE-N1000-03",
    }));
    fx.push(json!({
        "$k": "external_ref", "id": uid(0x7102), "project": "OTHER", "location": uid(0x6003),
        "device": "JE-N1000-04",
    }));
    fx.push(json!({
        "$k": "external_ref", "id": uid(0x7103), "location": uid(0x6006), "device": "JE-N1000-05",
    }));
    for (n, e, col) in [(0x1101, 0x7101, 0), (0x1102, 0x7102, 1), (0x1103, 0x7103, 2)] {
        fx.push(json!({
            "$k": "function", "id": uid(n), "def": uid(FNDEF_IP),
            "kind": {"ip": {"name": "EXT", "direction": "target", "external": uid(e)}},
        }));
        let (x, y) = at(col, 0);
        fx.rep(rep_of(n), pg(1), SYM_IP, &[n], x, y);
    }
    let m = fx.build();
    let q = Q::new(&m);
    let t = q.get::<XrefTextOf>(rid(0x1101));
    assert_eq!(t.text, "+J-E-206 / JE-N1000-03");
    assert!(t.links.is_empty(), "an external target has no link");
    assert!(matches!(
        q.get::<XrefRelationOf>(rid(0x1101)).kind,
        XrefKind::External(_)
    ));
    assert_eq!(text(&q, rep_of(0x1102)), "OTHER +J-E-206 / JE-N1000-04");
    // A nested path prints with the default RDS syntax, which repeats the aspect sign per level.
    assert_eq!(text(&q, rep_of(0x1103)), "+J+E+206 / JE-N1000-05");
}

/// An interruption point 0x1101 at A0 of page 1 naming the external reference 0x7101 (a location and a
/// device); the record exists.
fn external_fixture() -> Fx {
    let mut fx = base();
    fx.node(0x6003, "location", "J-E-206", None);
    fx.push(json!({
        "$k": "external_ref", "id": uid(0x7101), "location": uid(0x6003), "device": "JE-N1000-03",
    }));
    fx.push(json!({
        "$k": "function", "id": uid(0x1101), "def": uid(FNDEF_IP),
        "kind": {"ip": {"name": "EXT", "direction": "target", "external": uid(0x7101)}},
    }));
    let (x, y) = at(0, 0);
    fx.rep(rep_of(0x1101), pg(1), SYM_IP, &[0x1101], x, y);
    fx
}

#[test]
fn dangling_external_ref_is_refused_at_load() {
    // The model refuses a connection point that names a missing external reference (EW-INV-001): the
    // derivation's fallback is reached on a raw model only, see the next test.
    let mut fx = external_fixture();
    fx.records.retain(|r| r["$k"] != "external_ref");
    assert_eq!(refused_with(&fx), "EW-INV-001");
}

#[test]
fn dangling_external_ref_prints_nothing_and_is_noted_on_a_raw_model() {
    let m = external_fixture().build();
    assert_eq!(text(&Q::new(&m), rep_of(0x1101)), "+J-E-206 / JE-N1000-03");
    let m = raw(&m, &[Op::delete(row_of::<ExternalRef>(&m, id(0x7101)))]);
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1101)), "");
    assert!(q.get::<XrefTextOf>(rid(0x1101)).links.is_empty());
    assert!(matches!(
        q.get::<XrefRelationOf>(rid(0x1101)).kind,
        XrefKind::External(_)
    ));
    let notes = xref_notes(&mut q, "EW-CORE-053");
    assert_eq!(notes.len(), 1, "{:?}", q.diagnostics().notes());
    assert!(notes[0].0.contains(&uid(0x7101)), "{notes:?}");
}

#[test]
fn target_without_anchor_cell_prints_question_marks() {
    let mut fx = base();
    // A title page has no frame grid.
    fx.page(pg(6), 6, None, "title_page", &[], "title");
    ip(&mut fx, 0x1100, pg(1), "S", "source", (1, 1));
    // Outside the frame (1 m to the right) on a framed page.
    fx.push(json!({
        "$k": "function", "id": uid(0x1101), "def": uid(FNDEF_IP),
        "kind": {"ip": {"name": "S", "direction": "target"}},
    }));
    fx.rep(
        rep_of(0x1101),
        pg(2),
        SYM_IP,
        &[0x1101],
        1_000_000_000,
        15_000_000,
    );
    ip(&mut fx, 0x1102, pg(6), "S", "target", (1, 1));
    let m = fx.build();
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1100)), "2-??, 6-??");
    let notes = xref_notes(&mut q, "EW-CORE-053");
    assert_eq!(notes.len(), 2, "{notes:?}");
    // The targets themselves print the source's cell.
    assert_eq!(text(&q, rep_of(0x1101)), format!("1-{}", cell(1, 1)));
    assert_eq!(text(&q, rep_of(0x1102)), format!("1-{}", cell(1, 1)));
}

/// A device 0x2000 with a main function 0x1001 drawn on page 1 at A1 and an auxiliary 0x1002 drawn on
/// page 2 at C3.
fn device_fixture() -> Fx {
    let mut fx = base();
    fx.device(0x2000, "K", Some(1));
    func(&mut fx, 0x1001, 0x2000, (FNDEF, SYM), true, pg(1), (1, 0));
    func(&mut fx, 0x1002, 0x2000, (FNDEF, SYM), false, pg(2), (3, 2));
    fx
}

/// The device fixture plus a second function 0x1003 of the device drawn on page 3 at B0 (an auxiliary).
fn two_mains_fixture() -> Fx {
    let mut fx = device_fixture();
    func(&mut fx, 0x1003, 0x2000, (FNDEF, SYM), false, pg(3), (0, 1));
    fx
}

#[test]
fn device_with_two_main_functions_is_refused_at_load() {
    // EW-INV-002: the model refuses a second main function; the derivation's note is reached on a raw model
    // only, see the next test.
    let mut fx = device_fixture();
    func(&mut fx, 0x1003, 0x2000, (FNDEF, SYM), true, pg(3), (0, 1));
    assert_eq!(refused_with(&fx), "EW-INV-002");
}

#[test]
fn device_with_two_main_functions_is_noted_on_a_raw_model() {
    // EW-INV-002: the second main function (larger ID) is treated as an auxiliary.
    let m = two_mains_fixture().build();
    let f: Id<Function> = id(0x1003);
    let before = row_of::<Function>(&m, f);
    let mut after = (**before.get::<Function>().expect("a function")).clone();
    after.role = FnRole::Main;
    let m = raw(&m, &[Op::update(before, AnyRow::of(after))]);
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1003)), format!("(1-{})", cell(1, 0)));
    assert_eq!(q.get::<XrefRelationOf>(rid(0x1003)).kind, XrefKind::AuxToMain);
    let notes = xref_notes(&mut q, "EW-INV-002");
    assert_eq!(notes.len(), 1, "{:?}", q.diagnostics().notes());
    assert!(notes[0].1.contains(&uid(0x1003)), "{}", notes[0].1);
    // The main representation lists both auxiliaries, in page order.
    assert_eq!(
        text(&q, rep_of(0x1001)),
        format!("∞ 2-{}, 3-{}", cell(3, 2), cell(0, 1))
    );
}

#[test]
fn aux_without_placed_main_prints_nothing() {
    let mut fx = base();
    fx.device(0x2000, "K", Some(1));
    fx.function(0x1001, Some(0x2000), FNDEF, true);
    func(&mut fx, 0x1002, 0x2000, (FNDEF, SYM), false, pg(2), (3, 2));
    let m = fx.build();
    let mut q = Q::new(&m);
    let t = q.get::<XrefTextOf>(rid(0x1002));
    assert_eq!(t.text, "");
    assert!(t.links.is_empty());
    assert_eq!(q.get::<XrefRelationOf>(rid(0x1002)).kind, XrefKind::None);
    assert_eq!(
        xref_notes(&mut q, "EW-CORE-053").len(),
        1,
        "{:?}",
        q.diagnostics().notes()
    );
}

#[test]
fn aux_inside_its_own_black_box_prints_nothing() {
    // Doc 06 §2.7: the main function is a black box on page 1 at A0 (outline 30 mm square).
    let build = |aux_at: (i64, i64)| {
        let mut fx = base();
        fx.device(0x2000, "K", Some(1));
        func(&mut fx, 0x1001, 0x2000, (FNDEF, SYM), true, pg(1), (0, 0));
        fx.record(rep_of(0x1001))["box"] = json!({
            "outline": [[0, 0], [30_000_000, 0], [30_000_000, 30_000_000], [0, 30_000_000]],
        });
        fx.function(0x1002, Some(0x2000), FNDEF, false);
        fx.rep(rep_of(0x1002), pg(1), SYM, &[0x1002], aux_at.0, aux_at.1);
        fx.build()
    };
    // The box covers x 10..40 mm, y 15..45 mm.
    let m = build((20_000_000, 25_000_000));
    let q = Q::new(&m);
    assert_eq!(q.get::<XrefRelationOf>(rid(0x1002)).kind, XrefKind::InOwnBox);
    assert_eq!(text(&q, rep_of(0x1002)), "");
    let main = q.get::<XrefRelationOf>(rid(0x1001));
    assert_eq!(main.kind, XrefKind::None);
    assert!(main.targets.is_empty());
    assert_eq!(text(&q, rep_of(0x1001)), "");

    // Outside the box (same page, C3): listed, and it refers back.
    let m = build(at(3, 2));
    let q = Q::new(&m);
    assert_eq!(q.get::<XrefRelationOf>(rid(0x1002)).kind, XrefKind::AuxToMain);
    assert_eq!(text(&q, rep_of(0x1002)), format!("(1-{})", cell(0, 0)));
    assert_eq!(text(&q, rep_of(0x1001)), format!("∞ 1-{}", cell(3, 2)));
}

#[test]
fn same_page_zone_only() {
    // FR-XRF-025: two "24VDC-9" points on one page.
    let build = |changes: &[(&str, Value)]| {
        let mut fx = base();
        settings(&mut fx, None, &[("xref_format", format_with(changes))]);
        ip(&mut fx, 0x1101, pg(1), "24VDC-9", "source", (3, 5));
        ip(&mut fx, 0x1102, pg(1), "24VDC-9", "target", (8, 1));
        fx.build()
    };
    let m = build(&[("same_page", json!("zone_only"))]);
    let q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1102)), "F3");
    assert_eq!(text(&q, rep_of(0x1101)), "B8");
    // Full: the page stays.
    let m = build(&[]);
    assert_eq!(text(&Q::new(&m), rep_of(0x1102)), "1-F3");
    // EPLAN style.
    let m = build(&[
        ("ip", json!("/{page}.{row}{column}")),
        ("same_page", json!("zone_only")),
    ]);
    assert_eq!(text(&Q::new(&m), rep_of(0x1102)), "/F3");
    let m = build(&[("ip", json!("/{page}.{row}{column}"))]);
    assert_eq!(text(&Q::new(&m), rep_of(0x1102)), "/1.F3");
}

#[test]
fn structure_prefix_when_different() {
    // FR-XRF-004: the source is on page 1 (location A), the targets on page 2 (A) and page 3 (B).
    let build = |changes: &[(&str, Value)]| {
        let mut fx = lib();
        fx.node(0x6001, "location", "A", None);
        fx.node(0x6002, "location", "B", None);
        settings(&mut fx, None, &[("xref_format", format_with(changes))]);
        fx.page(pg(1), 1, None, "multi_line", &[("location", 0x6001)], "one");
        fx.page(pg(2), 2, None, "multi_line", &[("location", 0x6001)], "two");
        fx.page(pg(3), 3, None, "multi_line", &[("location", 0x6002)], "three");
        ip(&mut fx, 0x1100, pg(1), "S", "source", (0, 0));
        ip(&mut fx, 0x1101, pg(2), "S", "target", (1, 0));
        ip(&mut fx, 0x1102, pg(3), "S", "target", (2, 0));
        fx.build()
    };
    let m = build(&[]);
    let q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1100)), "2-A1, +B/3-A2");
    // From the page in B the source in A differs.
    assert_eq!(text(&q, rep_of(0x1102)), "+A/1-A0");
    assert_eq!(text(&q, rep_of(0x1101)), "1-A0");
    let m = build(&[("structure", json!("always"))]);
    let q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1100)), "+A/2-A1, +B/3-A2");
    assert_eq!(text(&q, rep_of(0x1101)), "+A/1-A0");
    // A format that names the structure itself gets no prefix.
    let m = build(&[("ip", json!("{location}/{page}-{row}{column}"))]);
    assert_eq!(text(&Q::new(&m), rep_of(0x1100)), "+A/2-A1, +B/3-A2");
}

#[test]
fn malformed_xref_settings_fall_back_to_the_profile() {
    let reference = {
        let m = star_fixture().build();
        let q = Q::new(&m);
        text(&q, rep_of(0x1100))
    };
    assert_eq!(reference, "20-B3, 20-B4, 20-C4\n30-A0, 40-A1");
    for bad in [json!(5), format_with(&[("structure", json!("sometimes"))])] {
        let mut fx = star_fixture();
        settings(&mut fx, None, &[("xref_format", bad.clone())]);
        let m = fx.build();
        let mut q = Q::new(&m);
        assert_eq!(text(&q, rep_of(0x1100)), reference, "{bad}");
        let notes = malformed(&mut q);
        assert!(!notes.is_empty(), "{bad}: {:?}", q.diagnostics().notes());
        let effective = q.get::<XrefSettingsOf>(());
        assert_eq!(effective.format.ip, "{page}-{row}{column}");
    }
    // An unknown same_page value alone: noted, the default (full) applies.
    let mut fx = star_fixture();
    settings(
        &mut fx,
        None,
        &[("xref_format", format_with(&[("same_page", json!("half"))]))],
    );
    let m = fx.build();
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1100)), reference);
    assert_eq!(malformed(&mut q).len(), 1);
}

/// A main function 0x1001 (main representation page 1 at A0, a contact image on page 3 at D4) with
/// auxiliaries 0x1002 (definition 21-22, page 2 at B1) and 0x1003 (definition 13-14, page 1 at C4).
fn contact_fixture() -> Fx {
    let mut fx = base();
    fx.device(0x2000, "K", Some(1));
    func(&mut fx, 0x1001, 0x2000, (FNDEF, SYM), true, pg(1), (0, 0));
    // The aux with the larger designation has the smaller ID, so the order is by designation.
    func(&mut fx, 0x1002, 0x2000, (FNDEF_B, SYM_B), false, pg(2), (1, 1));
    func(&mut fx, 0x1003, 0x2000, (FNDEF_A, SYM_A), false, pg(1), (4, 2));
    let (x, y) = at(4, 3);
    fx.rep(0x9001, pg(3), SYM, &[0x1001], x, y);
    fx.with("rep_type", json!("contact_image"));
    fx
}

#[test]
fn contact_image_rows() {
    let m = contact_fixture().build();
    let q = Q::new(&m);
    let t = q.get::<XrefTextOf>(id(0x9001));
    assert_eq!(t.text, format!("13-14 1-{}, 21-22 2-{}", cell(4, 2), cell(1, 1)));
    assert_eq!(q.get::<XrefRelationOf>(id(0x9001)).kind, XrefKind::ContactImage);
    assert_eq!(
        shown(&t),
        vec![
            (format!("1-{}", cell(4, 2)), rid(0x1003)),
            (format!("2-{}", cell(1, 1)), rid(0x1002)),
        ]
    );
    // The main representation lists the auxiliaries in page order, not the contact image.
    assert_eq!(
        text(&q, rep_of(0x1001)),
        format!("∞ 1-{}, 2-{}", cell(4, 2), cell(1, 1))
    );
}

#[test]
fn coil_drawn_twice_adds_no_contact_rows() {
    // The reviewer's fixture: the main function (the coil) is drawn a second time on page 4. That drawing is
    // listed by the main representation ("same function") but is no contact row.
    let reference = {
        let m = contact_fixture().build();
        text(&Q::new(&m), 0x9001)
    };
    let mut fx = contact_fixture();
    let (x, y) = at(2, 2);
    fx.rep(0x2_0002, pg(4), SYM, &[0x1001], x, y);
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(text(&q, 0x9001), reference);
    assert_eq!(
        reference,
        format!("13-14 1-{}, 21-22 2-{}", cell(4, 2), cell(1, 1))
    );
    let rel = q.get::<XrefRelationOf>(id(0x9001));
    assert_eq!(rel.kind, XrefKind::ContactImage);
    assert_eq!(rel.targets.len(), 2);
    assert!(rel.targets.iter().all(|t| t.role == ew_derive::TargetRole::Aux));
    // The second drawing of the coil refers to the main representation, which lists it.
    assert_eq!(
        text(&q, rep_of(0x1001)),
        format!("∞ 1-{}, 2-{}, 4-{}", cell(4, 2), cell(1, 1), cell(2, 2))
    );
    assert_eq!(text(&q, 0x2_0002), format!("(1-{})", cell(0, 0)));
}

#[test]
fn contact_rows_sort_by_sequence_then_designation() {
    // Doc 06 §2.8: by contact sequence digit, then designation: "113-114" (sequence 11) follows "21-22"
    // (sequence 2), which a plain string sort would put first.
    let mut fx = contact_fixture();
    func(&mut fx, 0x1004, 0x2000, (FNDEF_C, SYM_C), false, pg(1), (7, 7));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(
        text(&q, 0x9001),
        format!(
            "13-14 1-{}, 21-22 2-{}, 113-114 1-{}",
            cell(4, 2),
            cell(1, 1),
            cell(7, 7)
        )
    );
}

/// A function 0x1001 of a device drawn as an overview (page 1 at A0, the smaller ID) and as a multi-line
/// representation (page 2 at B1).
fn same_function_fixture() -> Fx {
    let mut fx = base();
    fx.device(0x2000, "K", Some(1));
    fx.function(0x1001, Some(0x2000), FNDEF, true);
    let (x, y) = at(0, 0);
    fx.rep(0x1_0001, pg(1), SYM, &[0x1001], x, y);
    fx.with("rep_type", json!("overview"));
    let (x, y) = at(1, 1);
    fx.rep(0x1_0002, pg(2), SYM, &[0x1001], x, y);
    fx
}

#[test]
fn same_function_reps_refer_to_the_main_rep() {
    let m = same_function_fixture().build();
    let q = Q::new(&m);
    // FR-XRF-016: the main rep (smallest ID) lists the other, which prints the main rep.
    let main = q.get::<XrefRelationOf>(id(0x1_0001));
    assert_eq!(main.kind, XrefKind::MainToAux);
    assert_eq!(main.targets.len(), 1);
    assert_eq!(text(&q, 0x1_0001), format!("∞ 2-{}", cell(1, 1)));
    let other = q.get::<XrefRelationOf>(id(0x1_0002));
    assert_eq!(other.kind, XrefKind::AuxToMain);
    assert_eq!(text(&q, 0x1_0002), format!("(1-{})", cell(0, 0)));
}

#[test]
fn ten_thousand_targets_are_formatted_without_quadratic_work() {
    const N: i64 = 10_000;
    let mut fx = base();
    ip(&mut fx, 0x10, pg(1), "BIG", "source", (2, 3));
    for i in 0..N {
        ip(
            &mut fx,
            0x1000 + i as u128,
            pg(2),
            "BIG",
            "target",
            (i % 17, (i / 17) % 8),
        );
    }
    let m = fx.build();
    let q = Q::new(&m);
    let start = std::time::Instant::now();
    let source = q.get::<XrefTextOf>(rid(0x10));
    assert_eq!(source.links.len(), 9);
    assert!(source.text.ends_with(", …"));
    assert_eq!(source.text.lines().count(), 3);
    // 10 000 targets over 136 cells: the first nine items are the A0 copies (smallest IDs first).
    assert!(source.text.starts_with("2-A0, 2-A0, 2-A0\n"));
    let rel = q.get::<XrefRelationOf>(rid(0x10));
    assert_eq!(rel.targets.len(), N as usize);
    // Sorted by row, then column, then ID: the first target is A0 with the smallest ID.
    assert_eq!(rel.targets[0].rep, rid(0x1000));
    let key = |t: &ew_derive::XrefTarget| (t.cell.as_ref().map(|c| (c.row, c.column)), t.rep);
    assert!(rel.targets.windows(2).all(|w| key(&w[0]) < key(&w[1])));
    let want = format!("1-{}", cell(2, 3));
    for i in 0..N {
        let t = q.get::<XrefTextOf>(rid(0x1000 + i as u128));
        assert_eq!(t.text, want, "target {i}");
        assert_eq!(t.links.len(), 1);
    }
    eprintln!("10 000 targets: {:?}", start.elapsed());
}

/// The texts, links and kinds of every representation of `m`, sorted by representation ID.
fn all_texts(m: &Model) -> Vec<(Id<Representation>, XrefText, XrefKind)> {
    let q = Q::new(m);
    let mut ids: Vec<Id<Representation>> = q.reps().iter().map(|(r, _)| r).collect();
    ids.sort();
    ids.into_iter()
        .map(|r| {
            (
                r,
                q.get::<XrefTextOf>(r).as_ref().clone(),
                q.get::<XrefRelationOf>(r).kind.clone(),
            )
        })
        .collect()
}

#[test]
fn xref_texts_do_not_depend_on_record_order() {
    // The star of test 2, a contact image and same-function representations in one model.
    let mut fx = star_fixture();
    fx.device(0x2000, "K", Some(1));
    func(&mut fx, 0x1201, 0x2000, (FNDEF, SYM), true, pg(1), (0, 0));
    func(&mut fx, 0x1202, 0x2000, (FNDEF_B, SYM_B), false, pg(2), (1, 1));
    func(&mut fx, 0x1203, 0x2000, (FNDEF_A, SYM_A), false, pg(1), (4, 2));
    let (x, y) = at(4, 3);
    fx.rep(0x9001, pg(2), SYM, &[0x1201], x, y);
    fx.with("rep_type", json!("contact_image"));
    fx.device(0x2001, "K", Some(2));
    fx.function(0x1301, Some(0x2001), FNDEF, true);
    let (x, y) = at(6, 6);
    fx.rep(0x1_3001, pg(1), SYM, &[0x1301], x, y);
    fx.with("rep_type", json!("overview"));
    let (x, y) = at(7, 7);
    fx.rep(0x1_3002, pg(2), SYM, &[0x1301], x, y);
    let forward = all_texts(&fx.build());
    assert!(forward.iter().any(|(_, t, _)| !t.text.is_empty()));
    assert!(forward.iter().any(|(_, _, k)| *k == XrefKind::ContactImage));
    let mut reversed = fx.clone();
    reversed.records.reverse();
    assert_eq!(forward, all_texts(&reversed.build()));
}

/// Asserts that every representation of `reps` has no cross-reference: kind `None`, no targets, no text.
fn assert_no_xref(m: &Model, reps: &[u128]) {
    let q = Q::new(m);
    for r in reps {
        let rel = q.get::<XrefRelationOf>(id(*r));
        assert_eq!(rel.kind, XrefKind::None, "{r:#x}");
        assert!(rel.targets.is_empty());
        let t = q.get::<XrefTextOf>(id(*r));
        assert_eq!(t.text, "", "{r:#x}");
        assert!(t.links.is_empty());
    }
}

#[test]
fn hostile_shapes_do_not_panic() {
    // A representation that does not exist.
    let m = base().build();
    assert_no_xref(&m, &[0xdead]);

    // A representation without functions: the model accepts it.
    let (x, y) = at(1, 1);
    let mut fx = base();
    fx.rep(0x8001, pg(1), SYM, &[], x, y);
    assert_no_xref(&fx.build(), &[0x8001]);

    // An interruption point without a connection point: the model accepts it.
    let mut fx = base();
    fx.push(json!({
        "$k": "function", "id": uid(0x1101), "def": uid(FNDEF_NO_CP),
        "kind": {"ip": {"name": "X", "direction": "source"}},
    }));
    fx.rep(rep_of(0x1101), pg(1), SYM_NO_CP, &[0x1101], x, y);
    assert_no_xref(&fx.build(), &[rep_of(0x1101)]);
}

#[test]
fn hostile_shapes_are_refused_at_load() {
    let (x, y) = at(1, 1);
    // A representation naming a missing function (EW-INV-001).
    let mut fx = base();
    fx.rep(0x8002, pg(1), SYM, &[0x7777], x, y);
    assert_eq!(refused_with(&fx), "EW-INV-001");

    // A function whose device is missing (EW-INV-001).
    let mut fx = base();
    func(&mut fx, 0x1001, 0x2999, (FNDEF, SYM), false, pg(1), (1, 1));
    assert_eq!(refused_with(&fx), "EW-INV-001");
}

#[test]
fn hostile_shapes_on_raw_models_print_nothing() {
    // The shapes of the previous test, reached through `Model::apply_unchecked` so that the derivation's
    // own guards for a missing function and a missing device run.
    let mut fx = base();
    fx.device(0x2000, "K", Some(1));
    fx.function(0x1000, Some(0x2000), FNDEF, true);
    func(&mut fx, 0x1001, 0x2000, (FNDEF, SYM), false, pg(1), (1, 1));
    let m = fx.build();

    // The function of the representation is gone.
    let missing_function = raw(&m, &[Op::delete(row_of::<Function>(&m, id(0x1001)))]);
    assert_no_xref(&missing_function, &[rep_of(0x1001)]);

    // The device of the function is gone: an auxiliary without a main function prints nothing.
    let missing_device = raw(&m, &[Op::delete(row_of::<Device>(&m, id(0x2000)))]);
    assert_no_xref(&missing_device, &[rep_of(0x1001)]);
}

#[test]
fn partial_xref_settings_apply_with_defaults() {
    // Doc 03 §5.1 gives `structure` and `same_page` defaults and `key` is optional: a format that names
    // only what it changes applies and is not malformed.
    let mut fx = star_fixture();
    settings(
        &mut fx,
        None,
        &[(
            "xref_format",
            json!({
                "ip": "X{page}", "aux_to_main": "({page}-{row}{column})", "main_to_aux": "∞ {list}",
                "matrix": {"per_row": 3, "max": 9, "overflow": "…"},
            }),
        )],
    );
    let m = fx.build();
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1100)), "X20, X20, X20\nX30, X40");
    assert_eq!(text(&q, rep_of(0x1101)), "X1");
    assert!(malformed(&mut q).is_empty(), "{:?}", q.diagnostics().notes());
    let effective = q.get::<XrefSettingsOf>(());
    assert_eq!(effective.format.ip, "X{page}");
    assert_eq!(effective.format.structure, "when_different");
    assert_eq!(effective.format.same_page, "full");
    assert_eq!(effective.format.key, "custom");
}

/// A device 0x2000 whose main function 0x1001 is drawn as a strip segment (page 1, the smaller ID) and as
/// a schematic symbol (page 2 at B1), with an auxiliary 0x1002 on page 3 at C3.
fn strip_segment_fixture() -> Fx {
    let mut fx = base();
    fx.device(0x2000, "K", Some(1));
    fx.function(0x1001, Some(0x2000), FNDEF, true);
    let (x, y) = at(0, 0);
    fx.rep(0x8001, pg(1), SYM, &[0x1001], x, y);
    fx.with("rep_type", json!("strip_segment"));
    let (x, y) = at(1, 1);
    fx.rep(rep_of(0x1001), pg(2), SYM, &[0x1001], x, y);
    func(&mut fx, 0x1002, 0x2000, (FNDEF, SYM), false, pg(3), (3, 2));
    fx
}

#[test]
fn strip_segments_are_never_main_representations_or_targets() {
    // Doc 08 §6.7. The segment has the smaller ID, yet the schematic symbol is the main representation of
    // the cross-references: the auxiliary refers to it, it lists the auxiliary, and the segment neither
    // refers nor is referred to.
    let m = strip_segment_fixture().build();
    let q = Q::new(&m);
    let segment = q.get::<XrefRelationOf>(id(0x8001));
    assert_eq!(segment.kind, XrefKind::None);
    assert!(segment.targets.is_empty());
    assert_eq!(text(&q, 0x8001), "");
    assert_eq!(text(&q, rep_of(0x1002)), format!("(2-{})", cell(1, 1)));
    let aux = q.get::<XrefRelationOf>(rid(0x1002));
    assert_eq!(aux.targets.len(), 1);
    assert_eq!(aux.targets[0].rep, rid(0x1001));
    let main = q.get::<XrefRelationOf>(rid(0x1001));
    assert_eq!(main.kind, XrefKind::MainToAux);
    assert_eq!(main.targets.len(), 1, "the segment is not a same-function target");
    assert_eq!(text(&q, rep_of(0x1001)), format!("∞ 3-{}", cell(3, 2)));

    // An interruption point drawn as a strip segment is no partner either.
    let mut fx = base();
    ip(&mut fx, 0x1100, pg(1), "S", "source", (1, 1));
    ip(&mut fx, 0x1101, pg(2), "S", "target", (2, 2));
    fx.with("rep_type", json!("strip_segment"));
    let m = fx.build();
    let q = Q::new(&m);
    assert!(q.get::<XrefRelationOf>(rid(0x1100)).targets.is_empty());
    assert_eq!(text(&q, rep_of(0x1100)), "");
}

#[test]
fn aux_without_main_representation_is_noted_by_how_the_main_is_drawn() {
    // The main function is drawn only as a strip segment: the note says so.
    let mut fx = base();
    fx.device(0x2000, "K", Some(1));
    fx.function(0x1001, Some(0x2000), FNDEF, true);
    let (x, y) = at(0, 0);
    fx.rep(0x8001, pg(1), SYM, &[0x1001], x, y);
    fx.with("rep_type", json!("strip_segment"));
    func(&mut fx, 0x1002, 0x2000, (FNDEF, SYM), false, pg(3), (3, 2));
    let m = fx.build();
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1002)), "");
    let notes = xref_notes(&mut q, "EW-CORE-053");
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].1.contains("drawn only as a strip segment"), "{notes:?}");

    // The main function is unplaced: it has no representation at all.
    let mut fx = base();
    fx.device(0x2000, "K", Some(1));
    fx.function(0x1001, Some(0x2000), FNDEF, true);
    func(&mut fx, 0x1002, 0x2000, (FNDEF, SYM), false, pg(2), (3, 2));
    let m = fx.build();
    let mut q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1002)), "");
    let notes = xref_notes(&mut q, "EW-CORE-053");
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].1.contains("has no representation"), "{notes:?}");
}

#[test]
fn zone_only_drops_page_parts_with_their_separators() {
    // FR-XRF-025: two "24VDC-9" points on one page; the page parts go with the literal that separates them
    // from the cell, in front of it or behind it.
    let build = |ip_format: &str| {
        let mut fx = base();
        settings(
            &mut fx,
            None,
            &[(
                "xref_format",
                format_with(&[("ip", json!(ip_format)), ("same_page", json!("zone_only"))]),
            )],
        );
        ip(&mut fx, 0x1101, pg(1), "24VDC-9", "source", (3, 5));
        ip(&mut fx, 0x1102, pg(1), "24VDC-9", "target", (8, 1));
        ip(&mut fx, 0x1103, pg(2), "24VDC-9", "target", (4, 4));
        fx.build()
    };
    // (format, the same-page form of a cell, the form of a cell on another page)
    type Forms = (&'static str, fn(&str) -> String, fn(&str, &str) -> String);
    let cases: [Forms; 4] = [
        ("{row}{column}/{page}", |c| c.into(), |p, c| format!("{c}/{p}")),
        (
            "[{row}{column}/{page}]",
            |c| format!("[{c}]"),
            |p, c| format!("[{c}/{p}]"),
        ),
        ("{page}/{row}{column}", |c| c.into(), |p, c| format!("{p}/{c}")),
        (
            "({page}-{row}{column})",
            |c| format!("({c})"),
            |p, c| format!("({p}-{c})"),
        ),
    ];
    for (format, zone, full) in cases {
        let m = build(format);
        let q = Q::new(&m);
        let text = |n: u128| q.get::<XrefTextOf>(rid(n)).text.clone();
        // The source lists the same-page target (page part dropped) and the one on page 2 (kept).
        assert_eq!(
            text(0x1101),
            format!("{}, {}", zone(&cell(8, 1)), full("2", &cell(4, 4))),
            "{format}"
        );
        assert_eq!(text(0x1102), zone(&cell(3, 5)), "{format}");
        assert_eq!(text(0x1103), full("1", &cell(3, 5)), "{format}");
    }
}

#[test]
fn empty_formats_print_nothing() {
    // An empty `ip` format yields no text and no links, for the interruption points, the main list and the
    // contact image rows alike (the main list's wrapper would otherwise print a bare "∞ ").
    let build = |changes: &[(&str, Value)]| {
        let mut fx = contact_fixture();
        ip(&mut fx, 0x1100, pg(1), "S", "source", (6, 6));
        ip(&mut fx, 0x1101, pg(2), "S", "target", (6, 6));
        settings(&mut fx, None, &[("xref_format", format_with(changes))]);
        fx.build()
    };
    let m = build(&[("ip", json!(""))]);
    let q = Q::new(&m);
    for r in [0x1100, 0x1101, rep_of(0x1001), 0x9001] {
        let t = q.get::<XrefTextOf>(id(r));
        assert_eq!(t.text, "", "{r:#x}");
        assert!(t.links.is_empty(), "{r:#x}");
    }
    // The auxiliaries print with their own format.
    assert_eq!(text(&q, rep_of(0x1002)), format!("(1-{})", cell(0, 0)));
    // An empty `aux_to_main` format silences the auxiliaries only.
    let m = build(&[("aux_to_main", json!(""))]);
    let q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1002)), "");
    assert!(q.get::<XrefTextOf>(rid(0x1002)).links.is_empty());
    assert_ne!(text(&q, rep_of(0x1001)), "");
}

#[test]
fn star_formats_only_the_shown_items() {
    // FR-XRF-008: ten targets, the matrix shows at most `max`; the links are the shown items.
    let build = |max: u8| {
        let mut fx = base();
        let matrix = json!({"per_row": 0, "max": max, "overflow": "…"});
        settings(
            &mut fx,
            None,
            &[("xref_format", format_with(&[("matrix", matrix)]))],
        );
        ip(&mut fx, 0x1100, pg(1), "S", "source", (0, 0));
        for i in 0..10 {
            ip(&mut fx, 0x1101 + i as u128, pg(2), "S", "target", (i, 0));
        }
        fx.build()
    };
    let m = build(9);
    let q = Q::new(&m);
    let t = q.get::<XrefTextOf>(rid(0x1100));
    assert_eq!(t.links.len(), 9);
    assert_eq!(t.text, "2-A0, 2-A1, 2-A2, 2-A3, 2-A4, 2-A5, 2-A6, 2-A7, 2-A8, …");
    assert_eq!(q.get::<XrefRelationOf>(rid(0x1100)).targets.len(), 10);
    let m = build(10);
    let t = Q::new(&m).get::<XrefTextOf>(rid(0x1100));
    assert_eq!(t.links.len(), 10);
    assert!(!t.text.ends_with('…'));
    let m = build(3);
    let t = Q::new(&m).get::<XrefTextOf>(rid(0x1100));
    assert_eq!(t.text, "2-A0, 2-A1, 2-A2, …");
    // 0 is unlimited.
    let m = build(0);
    assert_eq!(Q::new(&m).get::<XrefTextOf>(rid(0x1100)).links.len(), 10);
}

#[test]
fn page_index_counts_dividers_but_not_continuations() {
    // FR-XRF-003: `%f` is the position in `page_order` counting every page record, as in QET: the divider
    // page of a section counts, so "L-1" on folio 102 of a section with a divider prints 4 (front page,
    // divider, 101, 102). Report base pages count and continuation pages do not: the golden project's
    // "6-A1" is asserted in `ew-api` (`golden_xref`), which has the report engine.
    let mut fx = lib();
    fx.section(0x5001, 1, (100, 199), "Section", true);
    fx.page(pg(1), 1, None, "multi_line", &[], "front");
    fx.page(pg(2), 101, Some(0x5001), "multi_line", &[], "a");
    fx.page(pg(3), 102, Some(0x5001), "multi_line", &[], "b");
    settings(
        &mut fx,
        None,
        &[("xref_format", format_with(&[("ip", json!("%f-%l%c"))]))],
    );
    ip(&mut fx, 0x1100, pg(1), "L-1", "source", (0, 0));
    ip(&mut fx, 0x1101, pg(3), "L-1", "target", (0, 0));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(text(&q, rep_of(0x1100)), "4-A0");
    assert_eq!(text(&q, rep_of(0x1101)), "1-A0");
    let order = q.get::<ew_derive::PageOrder>(());
    assert_eq!(order.entries.len(), 4, "{:?}", order.entries);
}
