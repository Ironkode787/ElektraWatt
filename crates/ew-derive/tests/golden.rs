// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden project (doc 20) as an acceptance fixture of `ew-derive`: assertion tables against the
//! values printed in the specification.
//!
//! Covered so far (fixture: `tests/common/golden.rs`):
//!
//! - **Device tags** (doc 20 §2.1 rule 1, §6.1, §6.2, §9; FR-NUM-001, FR-NUM-002, FR-DEV-021): full and
//!   displayed tags of the frozen tags S51, F55, T55, E55, S56, E56 and 1X1 with its terminals, of the
//!   page-based tags of folios 3, 101, 102 and 201, of the strips -X1031 and -X1032 (strip rule) with their
//!   terminals, of the valves -201U3-Q1 and -201U4-Q4 (child rule) and of the field devices of +FIELD:
//!   `tags_match_the_specification`.
//! - **Page order** (doc 20 §4.3 sections, §5 pages; FR-PAG-005, FR-PAG-009, FR-PAG-013): folio labels with
//!   the generated dividers, Prev and Next, the TOC fixpoint: `page_order_matches_the_specification`.
//! - **Table of contents** (doc 20 §5; doc 09 §3.1; FR-REP-003): one row per page with title, function
//!   and location, and the generated divider descriptions: `toc_rows_match_the_specification`.
//! - **Determinism** (NFR-REL-07): the same project built in another record and library order gives equal
//!   page order, TOC and tags: `derived_values_do_not_depend_on_record_order`.
//!
//! - **Anchor cells** (doc 20 §6.1 "Main rep."; FR-PAG-020, FR-SYM-010, D-14): the frame cell of the main
//!   representation of 26 devices on folios 3, 101, 102, 103, 106, 201 and 202, with origin and named
//!   anchor policies, on the QET 10 px grid: `anchor_cells_match_the_main_rep_cells`.
//! - **Title blocks** (doc 20 §4.1, §4.2, §5, §17.2; doc 09 §2.6, §2.7): the full value sets of folio 101,
//!   folio 201 and the divider of section 1: `title_block_value_sets_match_the_specification`.
//! - **BOM** (doc 20 §14.1 and §14.2; FR-REP-004 to FR-REP-006): the 21 component rows of the fixture with
//!   position, quantity, unit, part number, description, devices and folios, and assembly mode with two
//!   configured products on -201U1: `bom_component_rows_match_the_specification`,
//!   `bom_assembly_rows_replace_the_covered_rows`.
//! - **Determinism** (NFR-REL-07): the same project built in another record and library order gives equal
//!   page order, TOC, tags, anchor cells, title blocks and BOM rows in both modes:
//!   `derived_values_do_not_depend_on_record_order`, and in 16 seeded shuffles of the records and library
//!   documents: `derived_values_do_not_depend_on_record_order_shuffled`.
//!
//! The fixture is a subset of Golden A (see `tests/common/golden.rs`): the quantities of the strips and of
//! the 533361 and 571334 rows count the fixture's devices, not the 20, 5, 7 and 28 of doc 20.
//! The generated folios (2, 901, 903 to 911) are not in the fixture (WP-S3.03).

mod common;
#[path = "common/golden.rs"]
mod golden;

use std::collections::BTreeMap;

use common::{Fx, id};
use ew_derive::{
    BomMode, BomRow, BomRows, DeviceTag, DisplayedTag, PageOrder, PageRef, Q, RepAnchorCell, TbValue,
    TitleBlockValues, TocRow, TocRows,
};
use ew_mltext::MlText;
use ew_schema::{Date, OrderUnit};
use ew_units::{Dec3, Grid};
use golden::*;

fn ml(text: &str) -> MlText {
    MlText::with_lang("en".parse().expect("language tag"), text)
}

/// (device, expected full designation, page it is displayed on, expected displayed tag).
fn tag_table() -> Vec<(u128, &'static str, u128, &'static str)> {
    vec![
        (DEV_S51, "=P01+J-E-2416-S51", PAGE_101, "S51"),
        (DEV_101F1, "=P01+J-E-2416-101F1", PAGE_101, "-101F1"),
        (DEV_F55, "=P01+J-E-2416-F55", PAGE_101, "F55"),
        (DEV_T55, "=P01+J-E-2416-T55", PAGE_101, "T55"),
        (DEV_E55, "=P01+J-E-2416-E55", PAGE_101, "E55"),
        (DEV_S56, "=P01+J-E-2416-S56", PAGE_101, "S56"),
        (DEV_E56, "=P01+J-E-2416-E56", PAGE_101, "E56"),
        (DEV_1X1, "=P01+J-E-2416-1X1", PAGE_101, "1X1"),
        (term_1x1(1), "=P01+J-E-2416-1X1:1", PAGE_101, "1X1:1"),
        (term_1x1(2), "=P01+J-E-2416-1X1:2", PAGE_101, "1X1:2"),
        (term_1x1(3), "=P01+J-E-2416-1X1:PE", PAGE_101, "1X1:PE"),
        (DEV_102G1, "=P01+J-E-2416-102G1", PAGE_102, "-102G1"),
        (DEV_102F1, "=P01+J-E-2416-102F1", PAGE_102, "-102F1"),
        (DEV_102F2, "=P01+J-E-2416-102F2", PAGE_102, "-102F2"),
        (DEV_3X1, "=P01+J-E-2416-3X1", PAGE_3, "-3X1"),
        (DEV_3W1, "=P01+J-E-2416-3W1", PAGE_3, "-3W1"),
        (DEV_X1031, "=P01+J-E-2416-X1031", PAGE_103, "-X1031"),
        (term_x1031(1), "=P01+J-E-2416-X1031:1", PAGE_103, "-X1031:1"),
        (term_x1031(5), "=P01+J-E-2416-X1031:5", PAGE_103, "-X1031:5"),
        (DEV_X1032, "=P01+J-E-2416-X1032", PAGE_103, "-X1032"),
        (term_x1032(2), "=P01+J-E-2416-X1032:2", PAGE_103, "-X1032:2"),
        (dev_201u(1), "=P01+J-E-2416-201U1", PAGE_201, "-201U1"),
        (dev_201u(2), "=P01+J-E-2416-201U2", PAGE_201, "-201U2"),
        (dev_201u(3), "=P01+J-E-2416-201U3", PAGE_201, "-201U3"),
        (dev_201u(9), "=P01+J-E-2416-201U9", PAGE_201, "-201U9"),
        (valve(3, 1), "=P01+J-E-2416-201U3-Q1", PAGE_203, "-201U3-Q1"),
        (valve(3, 4), "=P01+J-E-2416-201U3-Q4", PAGE_203, "-201U3-Q4"),
        (valve(4, 1), "=P01+J-E-2416-201U4-Q1", PAGE_203, "-201U4-Q1"),
        (valve(4, 4), "=P01+J-E-2416-201U4-Q4", PAGE_203, "-201U4-Q4"),
        (field_203q(1), "=P01+FIELD-203Q1", PAGE_203, "+FIELD-203Q1"),
        (field_203q(8), "=P01+FIELD-203Q8", PAGE_203, "+FIELD-203Q8"),
    ]
}

#[test]
fn tags_match_the_specification() {
    let m = golden().build();
    let q = Q::new(&m);
    let table = tag_table();
    assert!(table.len() >= 20);
    for (dev, full, page, displayed) in table {
        let tag = q.get::<DeviceTag>(id(dev));
        let tag = tag
            .as_ref()
            .as_ref()
            .unwrap_or_else(|| panic!("no tag for {dev:#x}"));
        assert_eq!(tag.full, full, "full tag of device {dev:#x}");
        assert_eq!(
            *q.get::<DisplayedTag>((id(dev), id(page))),
            displayed,
            "displayed tag of device {dev:#x} on page {page:#x}"
        );
    }
}

#[test]
fn page_order_matches_the_specification() {
    let m = golden().build();
    let q = Q::new(&m);
    let order = q.get::<PageOrder>(());
    assert_eq!(
        order.labels(),
        [
            "1", "3", "100", "101", "102", "103", "106", "200", "201", "202", "203", "204", "205", "206",
            "900", "902"
        ]
    );
    assert!(order.fixpoint_reached);
    assert_eq!(order.toc_pages, 1);

    let drawn = |folio: u32| PageRef::Drawn(id(page(folio)));
    // Folio 1 has no Previous page, the last page no Next page (FR-PAG-013).
    assert!(order.prev(drawn(1)).is_none());
    assert_eq!(order.next(drawn(1)).map(|e| e.folio.as_str()), Some("3"));
    assert!(order.next(drawn(902)).is_none());
    assert_eq!(order.prev(drawn(902)).map(|e| e.folio.as_str()), Some("900"));
    // The dividers are generated from the sections (FR-PAG-009).
    assert_eq!(
        order.prev(drawn(101)).map(|e| e.page),
        Some(PageRef::Divider(id(SEC_1)))
    );
    assert_eq!(
        order.next(drawn(106)).map(|e| e.page),
        Some(PageRef::Divider(id(SEC_2)))
    );
    assert_eq!(
        order.prev(drawn(902)).map(|e| e.page),
        Some(PageRef::Divider(id(SEC_9)))
    );
}

fn row<'a>(rows: &'a [TocRow], folio: &str) -> &'a TocRow {
    rows.iter()
        .find(|r| r.folio == folio)
        .unwrap_or_else(|| panic!("no TOC row for folio {folio}"))
}

#[test]
fn toc_rows_match_the_specification() {
    let m = golden().build();
    let q = Q::new(&m);
    let order = q.get::<PageOrder>(());
    let rows = q.get::<TocRows>(());
    // One row per page, in page order.
    assert_eq!(rows.len(), order.len());
    for (row, entry) in rows.iter().zip(&order.entries) {
        assert_eq!(row.page, entry.page);
        assert_eq!(row.folio, entry.folio);
    }

    let r101 = row(&rows, "101");
    assert_eq!(r101.page, PageRef::Drawn(id(PAGE_101)));
    assert_eq!(r101.title, ml("Power Supply 1 - Main Current"));
    assert_eq!(r101.location, "+J-E-2416");
    assert_eq!(r101.function, "=P01");

    // Front matter carries the structure too; folio 1 prints the title page.
    let r1 = row(&rows, "1");
    assert_eq!(r1.title, ml("Title Page"));
    assert_eq!(
        (r1.function.as_str(), r1.location.as_str()),
        ("=P01", "+J-E-2416")
    );

    // Dividers print the structure shared by the section's pages (FR-PAG-009).
    for (folio, section, text) in [
        (
            "100",
            SEC_1,
            "=P01+J-E-2416 - Section 1 - Page 100 - 199 - Wiring Definition",
        ),
        (
            "200",
            SEC_2,
            "=P01+J-E-2416 - Section 2 - Page 200 - 299 - Pneumatics",
        ),
        (
            "900",
            SEC_9,
            "=P01+J-E-2416 - Section 9 - Page 900 - 999 - Nomenclature",
        ),
    ] {
        let r = row(&rows, folio);
        assert_eq!(r.page, PageRef::Divider(id(section)));
        assert_eq!(r.title, ml(text), "divider {folio}");
        assert_eq!(r.function, "=P01");
        assert_eq!(r.location, "+J-E-2416");
    }
}

// ---------------------------------------------------------------------------------------------------------
// Anchor cells (doc 20 §6.1 "Main rep.")

/// (device, page of its main representation, expected anchor cell as printed: row letter, column).
fn anchor_table() -> Vec<(u128, u128, &'static str)> {
    vec![
        (DEV_3X1, PAGE_3, "B2"),
        (DEV_3W1, PAGE_3, "E2"),
        (DEV_S51, PAGE_101, "B2"),
        (DEV_101F1, PAGE_101, "B8"),
        (DEV_F55, PAGE_101, "B12"),
        (DEV_T55, PAGE_101, "C12"),
        (DEV_E55, PAGE_101, "E12"),
        (DEV_S56, PAGE_101, "D14"),
        (DEV_E56, PAGE_101, "E14"),
        (DEV_1X1, PAGE_101, "D2"),
        (term_1x1(1), PAGE_101, "D2"),
        (DEV_102G1, PAGE_102, "B6"),
        (DEV_102F1, PAGE_102, "C7"),
        (DEV_102F2, PAGE_102, "E7"),
        (DEV_X1031, PAGE_103, "A3"),
        (DEV_X1032, PAGE_103, "F3"),
        (DEV_106W1, PAGE_106, "F5"),
        (DEV_106W2, PAGE_106, "F11"),
        (dev_201u(1), PAGE_201, "E1"),
        (dev_201u(2), PAGE_201, "E2"),
        (dev_201u(3), PAGE_201, "E4"),
        (dev_201u(4), PAGE_201, "E6"),
        (dev_201u(5), PAGE_201, "E8"),
        (dev_201u(6), PAGE_201, "E10"),
        (dev_201u(7), PAGE_201, "E12"),
        (dev_201u(8), PAGE_201, "E14"),
        (dev_201u(9), PAGE_201, "E15"),
        (DEV_202U1, PAGE_202, "E1"),
    ]
}

#[test]
fn anchor_cells_match_the_main_rep_cells() {
    let m = golden().build();
    let q = Q::new(&m);
    let table = anchor_table();
    assert!(table.len() >= 10);
    for (dev, page, cell) in table {
        let rep = id(rep_of(dev));
        let cell_of = q.get::<RepAnchorCell>(rep);
        assert_eq!(
            (*cell_of).as_ref().map(ToString::to_string).as_deref(),
            Some(cell),
            "anchor cell of device {dev:#x}"
        );
        assert_eq!(q.reps().get(rep).expect("representation").page, id(page));
    }
    // The cells of the printed doc 20 form: row letter, then column, as cross-references print them.
    let cell = q.get::<RepAnchorCell>(id(rep_of(DEV_S51)));
    let cell = (*cell).as_ref().expect("cell");
    assert_eq!((cell.column, cell.row), (2, 1));
    assert_eq!((cell.column_label.as_str(), cell.row_label.as_str()), ("2", "B"));
}

#[test]
fn representations_sit_on_the_qet_grid_and_some_anchors_are_named() {
    let m = golden().build();
    let q = Q::new(&m);
    let mut named = 0;
    for (rep_id, rep) in q.reps().iter() {
        for coordinate in [rep.pos.x, rep.pos.y] {
            let k = Grid::QET_10PX.snap(coordinate);
            assert_eq!(
                Grid::QET_10PX.at(k),
                Some(coordinate),
                "representation {rep_id:?} is off the grid"
            );
        }
        let symbol = q.symbols().get(rep.symbol).expect("symbol");
        if matches!(symbol.item.anchor, ew_schema::AnchorPolicy::Named(_)) {
            named += 1;
        }
    }
    // -101F1, F55 and -102F1 draw the MCB symbol with a named anchor.
    assert_eq!(named, 3);
}

// ---------------------------------------------------------------------------------------------------------
// Title blocks (doc 20 §4.1, §4.2, §5, §17.2)

fn text(s: &str) -> TbValue {
    TbValue::Text(s.to_owned())
}

fn date(y: u16, m: u8, d: u8) -> TbValue {
    TbValue::Date(Date::new(y, m, d).expect("date"))
}

fn ml_value(s: &str) -> TbValue {
    TbValue::Ml(ml(s))
}

/// The values every page of the fixture shares.
fn shared() -> Vec<(&'static str, TbValue)> {
    vec![
        ("project.description", ml_value("Expansion Control Cabinets")),
        (
            "project.long_description",
            ml_value("Expansion Control Cabinets <end user>"),
        ),
        ("project.document_title", ml_value("Electrical Definition")),
        ("project.reference", text("<project ref>")),
        ("project.drawing_id", text("<drawing id>")),
        ("project.object", text("J-E-2416")),
        ("project.created", date(2025, 3, 3)),
        ("project.edited", date(2025, 4, 14)),
        ("project.supply", text("1 x 230 VAC + N + PE, 50 Hz")),
        (
            "project.logo",
            TbValue::Asset(logo().parse().expect("logo digest")),
        ),
        ("project.notice", ml_value("<confidentiality notice>")),
        ("revision.current.label", text("V2.0")),
        ("revision.current.state", text("open")),
        ("revision.current.date", date(2025, 4, 14)),
        ("revision.current.author.name", ml_value("<engineer>")),
        ("party.customer.name", ml_value("<customer>")),
        ("party.end_user.name", ml_value("<end user>")),
        ("party.end_user.address.street", text("<address>")),
        ("party.end_user.address.postcode", text("<postcode>")),
        ("party.end_user.address.city", text("<city>")),
        ("party.end_user.address.country", text("<country>")),
        ("party.panel_builder.name", ml_value("<panel builder>")),
        ("party.engineer.name", ml_value("<engineer>")),
    ]
}

fn expected(pairs: Vec<(&'static str, TbValue)>) -> BTreeMap<String, TbValue> {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

#[test]
fn title_block_value_sets_match_the_specification() {
    let m = golden().build();
    let q = Q::new(&m);
    let block = |page: PageRef| (*q.get::<TitleBlockValues>(page)).clone();
    // The fixture has 16 pages: 1, 3, 100, 101, 102, 103, 106, 200, 201 ... 206, 900, 902.
    assert_eq!(q.get::<PageOrder>(()).len(), 16);

    // Folio 101: Prev is the divider 100, Next is 102; 4th of 16.
    let mut want = shared();
    want.extend([
        ("page.folio", text("101")),
        ("page.prev", text("100")),
        ("page.next", text("102")),
        ("page.title", ml_value("Power Supply 1 - Main Current")),
        ("page.description", ml_value("Expansion Control Cabinets")),
        ("page.function", text("=P01")),
        ("page.last_changed_in", text("V1.0")),
        ("page.location", text("+J-E-2416")),
        ("doc.page_index", text("4")),
        ("doc.page_count", text("16")),
    ]);
    let tb = block(PageRef::Drawn(id(PAGE_101)));
    assert_eq!(tb.values, expected(want));
    // The checker is unset (doc 20 U-03): the path is absent, not empty.
    assert_eq!(tb.get("revision.current.checker.name"), None);

    // Folio 201: 9th of 16, Prev is the divider 200.
    let mut want = shared();
    want.extend([
        ("page.folio", text("201")),
        ("page.prev", text("200")),
        ("page.next", text("202")),
        ("page.title", ml_value("Valve Manifold Overview 1")),
        ("page.description", ml_value("Expansion Control Cabinets")),
        ("page.function", text("=P01")),
        ("page.last_changed_in", text("V1.0")),
        ("page.location", text("+J-E-2416")),
        ("doc.page_index", text("9")),
        ("doc.page_count", text("16")),
    ]);
    assert_eq!(
        block(PageRef::Drawn(id(PAGE_201))).values,
        expected(want),
        "folio 201"
    );

    // The divider of section 1: 3rd of 16, between folio 3 and folio 101.
    let mut want = shared();
    want.extend([
        ("page.folio", text("100")),
        ("page.prev", text("3")),
        ("page.next", text("101")),
        (
            "page.title",
            ml_value("=P01+J-E-2416 - Section 1 - Page 100 - 199 - Wiring Definition"),
        ),
        ("page.description", ml_value("Expansion Control Cabinets")),
        ("page.function", text("=P01")),
        ("page.location", text("+J-E-2416")),
        ("doc.page_index", text("3")),
        ("doc.page_count", text("16")),
    ]);
    assert_eq!(
        block(PageRef::Divider(id(SEC_1))).values,
        expected(want),
        "divider of section 1"
    );
}

// ---------------------------------------------------------------------------------------------------------
// BOM (doc 20 §14.1, §14.2)

/// One expected row: position, quantity (whole pieces or metres), unit, part number, description, devices
/// in tag order, folios.
type BomExpect = (
    u32,
    i64,
    OrderUnit,
    &'static str,
    &'static str,
    Vec<u128>,
    Vec<u32>,
);

/// Doc 20 §14.1 rows 1 to 21 as the fixture covers them. Rows 13, 14, 19 and 21 count the fixture's
/// terminals, sub-bases and valves (5 of 20, 3 of 5, 6 of 7, 8 of 28); row 2 is a test input of 10 m.
fn component_table() -> Vec<BomExpect> {
    let pc = OrderUnit::Piece;
    let valves: Vec<u128> = [3, 4]
        .into_iter()
        .flat_map(|sub| (1..=4).map(move |q| valve(sub, q)))
        .collect();
    vec![
        (
            1,
            1,
            pc.clone(),
            "532216",
            "PROFIBUS DP plug with terminating resistor",
            vec![DEV_3X1],
            vec![3],
        ),
        (
            2,
            10,
            OrderUnit::Metre,
            "6XV1830-0EH10",
            "PROFIBUS FC standard cable",
            vec![DEV_3W1],
            vec![3],
        ),
        (
            3,
            1,
            pc.clone(),
            "533356",
            "Main switch, 2-pole, 40 A",
            vec![DEV_S51],
            vec![101],
        ),
        (
            4,
            2,
            pc.clone(),
            "UK 5 N",
            "Feed-through terminal",
            vec![term_1x1(1), term_1x1(2)],
            vec![101],
        ),
        (
            5,
            1,
            pc.clone(),
            "USLKG 5",
            "PE terminal",
            vec![term_1x1(3)],
            vec![101],
        ),
        (
            6,
            1,
            pc.clone(),
            "A9P52610",
            "Miniature circuit breaker 1P+N, C, 10 A",
            vec![DEV_101F1],
            vec![101],
        ),
        (
            7,
            1,
            pc.clone(),
            "5SY4204-7",
            "Miniature circuit breaker 1P+N, C, 4 A",
            vec![DEV_F55],
            vec![101],
        ),
        (
            8,
            1,
            pc.clone(),
            "SK 3110.000",
            "Thermostat",
            vec![DEV_T55],
            vec![101],
        ),
        (
            9,
            1,
            pc.clone(),
            "SK 3102.000",
            "Enclosure heater",
            vec![DEV_E55],
            vec![101],
        ),
        (
            10,
            1,
            pc.clone(),
            "2904602",
            "Power supply 1 AC / 24 V DC / 20 A",
            vec![DEV_102G1],
            vec![102],
        ),
        (
            11,
            1,
            pc.clone(),
            "A9F78116",
            "Miniature circuit breaker 1P, B, 16 A",
            vec![DEV_102F1],
            vec![102],
        ),
        (
            12,
            1,
            pc.clone(),
            "MICO 4.10",
            "Electronic load distribution, 4 channels",
            vec![DEV_102F2],
            vec![102],
        ),
        (
            13,
            5,
            pc.clone(),
            "PT 2.5-TWIN",
            "Feed-through terminal, 3 connections",
            (1..=X1031_TERMINALS).map(term_x1031).collect(),
            vec![103],
        ),
        (
            14,
            3,
            pc.clone(),
            "PT 2.5-TWIN-PE",
            "PE terminal, 3 connections",
            (1..=X1032_TERMINALS).map(term_x1032).collect(),
            vec![103],
        ),
        (
            15,
            2,
            pc.clone(),
            "NEBU-G78W5-K-2-N-LE5",
            "Connecting cable 7/8\", 5-pin, open end",
            vec![DEV_106W1, DEV_106W2],
            vec![106],
        ),
        (
            16,
            1,
            pc.clone(),
            "195740",
            "CPX-FB13 bus node, PROFIBUS DP",
            vec![dev_201u(1)],
            vec![201],
        ),
        (
            17,
            2,
            pc.clone(),
            "550208",
            "CPX-M-GE-EV-S-7/8-5POL interlinking block with supply",
            vec![dev_201u(1), DEV_202U1],
            vec![201, 202],
        ),
        (
            18,
            1,
            pc.clone(),
            "552286",
            "VMPA-FB-EPLM-G pneumatic interface",
            vec![dev_201u(2)],
            vec![201],
        ),
        (
            19,
            6,
            pc.clone(),
            "533361",
            "VMPA1-FB-EMG-8 electronic module, 4 positions",
            (3..=8).map(dev_201u).collect(),
            vec![201],
        ),
        (
            20,
            1,
            pc.clone(),
            "533354",
            "VMPA1-FB-SP supply plate",
            vec![dev_201u(9)],
            vec![201],
        ),
        (
            21,
            8,
            pc,
            "571334",
            "5/2 monostable solenoid valve",
            valves,
            vec![203],
        ),
    ]
}

fn rows_of(q: &Q, mode: BomMode) -> Vec<BomRow> {
    q.get::<BomRows>(mode).as_ref().clone()
}

#[test]
fn bom_component_rows_match_the_specification() {
    let m = golden().build();
    let q = Q::new(&m);
    let rows = rows_of(&q, BomMode::Component);
    let table = component_table();
    assert_eq!(
        rows.iter().map(|r| r.part_number.as_str()).collect::<Vec<_>>(),
        table.iter().map(|t| t.3).collect::<Vec<_>>(),
        "row order"
    );
    for (row, (pos, qty, unit, number, description, devices, folios)) in rows.iter().zip(table) {
        assert_eq!(row.pos, pos, "{number}: position");
        assert_eq!(row.qty, Dec3(qty * 1_000), "{number}: quantity");
        assert_eq!(row.unit, unit, "{number}: unit");
        assert_eq!(row.description.as_neutral(), Some(description), "{number}");
        assert_eq!(
            row.devices,
            devices.into_iter().map(id).collect::<Vec<_>>(),
            "{number}: devices"
        );
        assert_eq!(
            row.pages,
            folios
                .into_iter()
                .map(|f| PageRef::Drawn(id(page(f))))
                .collect::<Vec<_>>(),
            "{number}: folios"
        );
        assert!(!row.configured, "{number}");
    }
    assert_eq!(rows.len(), 21);
    // S56 and E56 have no parts and no rows (EF-04); field devices have none either.
    for row in &rows {
        for dev in [DEV_S56, DEV_E56, field_203q(1), field_203q(8)] {
            assert!(!row.devices.contains(&id(dev)));
        }
    }
    // The part's manufacturer reference names the synthetic manufacturer.
    let plug = serde_json::to_value(&rows[0].manufacturer).expect("manufacturer");
    assert_eq!(plug["item"], serde_json::json!(common::uid(MFR_FESTO)));
}

#[test]
fn bom_assembly_rows_replace_the_covered_rows() {
    let m = golden().build();
    let q = Q::new(&m);
    let component = rows_of(&q, BomMode::Component);
    let assembly = rows_of(&q, BomMode::Assembly);

    // Rows 1 to 15 stay (same order, positions and quantities); rows 16 to 21 are gone: -201U1 heads the
    // assembly, and -201U2 ... -201U9, -202U1 and the valves are its descendants.
    assert_eq!(assembly.len(), 15 + 2);
    for (a, c) in assembly.iter().zip(component.iter().take(15)) {
        assert_eq!((&a.part_number, a.pos, a.qty), (&c.part_number, c.pos, c.qty));
        assert_eq!(a.devices, c.devices);
        assert!(!a.configured);
    }
    let configured = &assembly[15..];
    assert!(configured.iter().all(|r| r.configured));
    assert_eq!(configured.iter().map(|r| r.pos).collect::<Vec<_>>(), [1, 2]);
    for r in configured {
        assert_eq!(r.qty, Dec3::ONE);
        assert_eq!(r.unit, OrderUnit::Set);
        assert_eq!(r.devices, [id(dev_201u(1))]);
        assert_eq!(r.pages, [PageRef::Drawn(id(PAGE_201))]);
    }
    // Doc 20 §14.2: the CPX order code is A1 and the MPA order code is A2. Both rows sit at the head's cell,
    // so the head's part order (CPX before MPA) decides, not the part numbers (32P-... sorts first).
    let codes: Vec<&str> = configured.iter().map(|r| r.part_number.as_str()).collect();
    assert_eq!(codes, [CODE_CPX, CODE_MPA]);
}

// ---------------------------------------------------------------------------------------------------------
// Determinism

/// The `Debug` text of every derived value this file covers, by name.
fn derived_text(fx: &Fx) -> Vec<(String, String)> {
    let m = fx.build();
    let q = Q::new(&m);
    let mut out = vec![
        ("page order".to_owned(), format!("{:?}", q.get::<PageOrder>(()))),
        ("toc".to_owned(), format!("{:?}", q.get::<TocRows>(()))),
    ];
    let mut devices = all_devices();
    devices.sort_unstable();
    for d in devices {
        out.push((format!("tag {d:#x}"), format!("{:?}", q.get::<DeviceTag>(id(d)))));
    }
    let mut reps: Vec<_> = q.reps().ids().collect();
    reps.sort_unstable();
    for r in reps {
        out.push((
            format!("anchor cell {r:?}"),
            format!("{:?}", q.get::<RepAnchorCell>(r)),
        ));
    }
    for entry in &q.get::<PageOrder>(()).entries {
        out.push((
            format!("title block {:?}", entry.page),
            format!("{:?}", q.get::<TitleBlockValues>(entry.page)),
        ));
    }
    for mode in [BomMode::Component, BomMode::Assembly] {
        out.push((format!("bom {mode:?}"), format!("{:?}", q.get::<BomRows>(mode))));
    }
    out
}

#[test]
fn derived_values_do_not_depend_on_record_order() {
    let a = derived_text(&golden());
    let b = derived_text(&golden_reversed());
    assert_eq!(a.len(), b.len());
    // Page order, TOC, tags, one anchor cell per representation, one title block per page, two BOMs.
    // Every device of the fixture has one representation.
    assert_eq!(a.len(), 2 + 2 * all_devices().len() + 16 + 2);
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.0, y.0);
        assert_eq!(x.1, y.1, "{}", x.0);
    }
}

/// A deterministic Fisher-Yates shuffle driven by a 64-bit LCG (Knuth's MMIX constants) seeded with `seed`.
fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    for i in (1..items.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}

#[test]
fn derived_values_do_not_depend_on_record_order_shuffled() {
    let forward = derived_text(&golden());
    for seed in 0..16u64 {
        let mut fx = golden();
        shuffle(&mut fx.records, seed);
        shuffle(&mut fx.lib, seed.wrapping_add(1_000));
        let shuffled = derived_text(&fx);
        assert_eq!(forward.len(), shuffled.len(), "seed {seed}");
        for (x, y) in forward.iter().zip(&shuffled) {
            assert_eq!(x.0, y.0, "seed {seed}");
            assert_eq!(x.1, y.1, "seed {seed}: {}", x.0);
        }
    }
}
