// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 5, PX-3 over Golden A: the PLC overview box of a module embedded on an overview page (FR-PLC-052;
//! doc 10 §5), drawn through WP-S4.01 part 4b's embedded path.
//!
//! FR-PLC-052's acceptance: "The overview box of -201U3 shows Q0.0 … Q0.7; its four bound rows print the cells of their
//! valves on folio 203; drawing on it changes no potential." Golden A has no such box (WP-S3.06 D4 forbids adding one),
//! so the tests add a `plc_overview` block selecting -201U3 to the session.
//!
//! Tests:
//! - `px3_the_box_of_201u3_on_201_lists_its_eight_channels_with_the_cells_of_folio_203`
//! - `px3_a_module_selection_lists_that_module_only_and_a_station_selection_is_unchanged`
//! - `px3_the_box_changes_no_potential`

use crate::golden_common;

use ew_api::Session;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::{PageOrder, PageRef, PotentialPartition, Q};
use ew_dl::{ItemKind, Layer};
use ew_import_tables::ids::record_id;
use ew_reports::BlockRows;
use ew_scene::forms::SheetMap;
use ew_schema::{AnyId, Bounds, Record, ReportBlock, ReportKind, ReportSource};

/// The frame form's page-to-sheet map (doc 20 §17.1).
const SHEET: SheetMap = SheetMap {
    scale_ppb: 986_282_578,
    offset: Pt2 {
        x: Nm(9_790_000),
        y: Nm(9_790_000),
    },
};

fn drawn(q: &Q, folio: &str) -> PageRef {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == folio)
        .map(|e| e.page)
        .unwrap_or_else(|| panic!("no folio {folio}"))
}

/// A page rectangle from printed hundredths of a millimetre.
fn printed_rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Bounds {
    let p = |x: i64, y: i64| SHEET.unapply(Pt2::new(Nm(x * 10_000), Nm(y * 10_000)));
    Bounds(Rect::from_points(p(x0, y0), p(x1, y1)))
}

/// A `plc_overview` block embedded on folio 201 beside its PART DATA (printed 100.00–230.00 × 16.31–93.00 mm: a
/// title band, a header row and eight rows of 6.52 mm), selecting `include`; built like 201's PART DATA block.
fn overview_box(include: &[&str]) -> ReportBlock {
    let batch = golden_common::batch();
    let mut b = batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if golden_common::source_key(r) == Some("report_blocks|part-data-201") => {
                Some(b.clone())
            }
            _ => None,
        })
        .expect("201's PART DATA block");
    b.id = Id::from_uuid(Uuid::from_u128(0x4c42_0000_0001));
    b.source = ReportSource::Kind(ReportKind::PlcOverview);
    b.source_ref = None;
    b.manual.include = include
        .iter()
        .map(|tag| AnyId::from_uuid(record_id::<ew_schema::Device>("devices", tag).as_uuid()))
        .collect::<Vec<_>>()
        .into();
    let embed = b.embedded.as_mut().expect("embedded");
    embed.page = record_id("pages", "201");
    embed.rect = printed_rect(10_000, 1_631, 23_000, 9_300);
    b
}

fn session_with(block: ReportBlock) -> Session {
    golden_common::session_with(&golden_common::batch(), |_, records| {
        records.push(Record::ReportBlock(block))
    })
}

/// The block's rows as rendered texts, one vector per row.
fn rows(q: &Q, id: Id<ReportBlock>) -> Vec<Vec<String>> {
    let set = q.get::<BlockRows>(id);
    (0..set.rows.len())
        .map(|i| ew_reports::csv::row_texts(q, &set, i))
        .collect()
}

/// PX-3: the box of -201U3 on 201 draws "PLC OVERVIEW", the six headers and the eight channels Q0.0 … Q0.7 with their
/// numbers, symbols, function texts and states; the bound rows print the frame cells of their valves' coils on folio
/// 203; the cells link to the bound CPs; nothing overflows and PART DATA still draws.
#[test]
fn px3_the_box_of_201u3_on_201_lists_its_eight_channels_with_the_cells_of_folio_203() {
    let block = overview_box(&["-201U3"]);
    let id = block.id;
    let s = session_with(block);
    let q = s.query();
    let l = ew_api::output::page_layer(&q, drawn(&q, "201"), Layer::Tables);
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    let texts: Vec<String> = l
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect();
    let expected: Vec<&str> = vec![
        "PART DATA",
        "Order Code 1",
        "51E-F13GCQP-D+H2GG",
        "Order Code 2",
        "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS",
    ];
    assert_eq!(texts[..expected.len()], expected[..], "PART DATA first");
    let rows = rows(&q, id);
    assert_eq!(rows.len(), 8);
    // the printed columns of each row: channel, address, symbol, function, state, page
    let printed: Vec<[&str; 6]> = rows
        .iter()
        .map(|r| [&r[3], &r[2], &r[6], &r[7], &r[10], &r[11]].map(String::as_str))
        .collect();
    assert_eq!(printed, EXPECTED_ROWS);
    let mut box_texts: Vec<&str> = vec![
        "PLC OVERVIEW",
        "Channel",
        "Address",
        "Symbol",
        "Function text",
        "State",
        "Page",
    ];
    for r in &EXPECTED_ROWS {
        box_texts.extend(r.iter().filter(|t| !t.is_empty()));
    }
    assert_eq!(texts[expected.len()..], box_texts[..]);
    // the bound rows link their page cell to the coil's CP (doc 10 §5)
    let set = q.get::<BlockRows>(id);
    let linked = set.rows.iter().filter(|r| !r.links.is_empty()).count();
    let bound = EXPECTED_ROWS.iter().filter(|r| !r[5].is_empty()).count();
    assert_eq!(linked, bound);
}

/// The printed cells of -201U3's eight rows on Golden A: channel, address, symbol, function text, state, page.
const EXPECTED_ROWS: [[&str; 6]; 8] = [
    ["0", "Q0.0", "", "", "deactivated", ""],
    ["1", "Q0.1", "", "TA05-V-15", "used", "203-H1"],
    ["2", "Q0.2", "", "", "deactivated", ""],
    ["3", "Q0.3", "", "TA05-V-33", "used", "203-H3"],
    ["4", "Q0.4", "", "", "deactivated", ""],
    ["5", "Q0.5", "", "TA05-V-18", "used", "203-H5"],
    ["6", "Q0.6", "", "", "deactivated", ""],
    ["7", "Q0.7", "", "TA05-V-17", "used", "203-H7"],
];

/// PX-3: a selection naming a module lists that module's rows only; one naming the station lists all 56 (folio 908's
/// block, unchanged).
#[test]
fn px3_a_module_selection_lists_that_module_only_and_a_station_selection_is_unchanged() {
    let module = overview_box(&["-201U3"]);
    let station = overview_box(&["-201U1"]);
    let both = overview_box(&["-201U3", "-202U2"]);
    let q = session_with(module.clone()).query();
    let modules: Vec<String> = rows(&q, module.id).into_iter().map(|r| r[0].clone()).collect();
    assert_eq!(modules, vec!["-201U3".to_owned(); 8]);
    let q = session_with(station.clone()).query();
    let golden = golden_common::block(&golden_common::batch(), "908");
    assert_eq!(rows(&q, station.id), rows(&q, golden));
    assert_eq!(rows(&q, station.id).len(), 56);
    let q = session_with(both.clone()).query();
    let modules: Vec<String> = rows(&q, both.id).into_iter().map(|r| r[0].clone()).collect();
    let mut expected = vec!["-201U3".to_owned(); 8];
    expected.extend(vec!["-202U2".to_owned(); 8]);
    assert_eq!(modules, expected, "in plc_rows order");
}

/// PX-3: "drawing on it changes no potential": the box has no connection points, so Golden A's potential partition is
/// the same with and without it.
#[test]
fn px3_the_box_changes_no_potential() {
    let without = crate::common::golden_session();
    let with = session_with(overview_box(&["-201U3"]));
    let a = without.query().get::<PotentialPartition>(());
    let b = with.query().get::<PotentialPartition>(());
    assert_eq!(*a, *b);
}
