// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Over the sample: the PLC overview box of a module embedded on a page (FR-PLC-052; doc 10 §5), drawn through the
//! embedded path.
//!
//! FR-PLC-052's acceptance: "The overview box of a module shows its channels; its bound rows print the cells of what
//! they drive; drawing on it changes no potential." The sample has no such box, so the tests add a `plc_overview`
//! block on folio 201 selecting the valve terminal -201K1 (eight outputs, four of them driving the valves on 201).
//!
//! Tests:
//! - `the_box_of_201k1_on_201_lists_its_eight_channels_with_the_cells_of_its_valves`
//! - `a_module_selection_lists_that_module_only_and_a_station_selection_its_cards`
//! - `the_box_changes_no_potential`

use crate::common;

use ew_api::Session;
use ew_derive::{PageOrder, PageRef, PotentialPartition, Q};
use ew_dl::{ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_reports::BlockRows;
use ew_scene::forms::SheetMap;
use ew_schema::{AnyId, Bounds, Record, ReportBlock, ReportKind, ReportSource};
use ew_units::{Nm, Pt2, Rect};

/// The frame form's page-to-sheet map.
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

/// A `plc_overview` block embedded on folio 201 (printed 100.00–230.00 × 16.31–93.00 mm: a title band, a header row
/// and eight rows of 6.52 mm), selecting the devices `include`; built from the sample's embedded legend block.
fn overview_box(include: &[&str]) -> ReportBlock {
    let s = common::sample_with_legend_box();
    let (_, records, _) = common::sample_records_with_legend_box();
    let mut b = records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if b.embedded.is_some() => Some(b.clone()),
            _ => None,
        })
        .expect("the sample's embedded legend");
    b.id = Id::from_uuid(Uuid::from_u128(0x4c42_0000_0001));
    b.source = ReportSource::Kind(ReportKind::PlcOverview);
    b.source_ref = None;
    // The kind's own printed columns, not the legend's.
    b.columns = Vec::new();
    b.manual.include = include
        .iter()
        .map(|tag| AnyId::from_uuid(s.resolve_reference(&format!("tag:{tag}")).expect("the device")))
        .collect::<Vec<_>>()
        .into();
    let page = s
        .page_rows()
        .into_iter()
        .find(|r| r.folio == "201")
        .expect("folio 201")
        .id;
    let embed = b.embedded.as_mut().expect("embedded");
    embed.page = Id::from_uuid(page);
    embed.rect = printed_rect(10_000, 1_631, 23_000, 9_300);
    b
}

fn session_with(block: ReportBlock) -> Session {
    let (project, mut records, lib) = common::sample_records_with_legend_box();
    records.push(Record::ReportBlock(block));
    common::session_of(project, records, lib)
}

/// The block's rows as rendered texts, one vector per row.
fn rows(q: &Q, id: Id<ReportBlock>) -> Vec<Vec<String>> {
    let set = q.get::<BlockRows>(id);
    (0..set.rows.len())
        .map(|i| ew_reports::csv::row_texts(q, &set, i))
        .collect()
}

/// PX-3: the box of -201K1 on 201 draws "PLC OVERVIEW", the six headers and the eight channels Q0.0 … Q0.7 with their
/// numbers, symbols, function texts and states; the bound rows print the frame cells of their valves on folio 201; the
/// cells link to the bound CPs; nothing overflows.
#[test]
fn the_box_of_201k1_on_201_lists_its_eight_channels_with_the_cells_of_its_valves() {
    let block = overview_box(&["-201K1"]);
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
    assert_eq!(texts, box_texts, "the box is the page's only table");
    // the bound rows link their page cell to the valve's CP (doc 10 §5)
    let set = q.get::<BlockRows>(id);
    let linked = set.rows.iter().filter(|r| !r.links.is_empty()).count();
    let bound = EXPECTED_ROWS.iter().filter(|r| !r[5].is_empty()).count();
    assert_eq!(linked, bound);
}

/// The printed cells of -201K1's eight rows: channel, address, symbol, function text, state, page. The sample's
/// script puts the valves on the even outputs and names the two spare valves "Spare"; the odd outputs are
/// deactivated.
const EXPECTED_ROWS: [[&str; 6]; 8] = [
    ["0", "Q0.0", "", "", "used", "201-E4"],
    ["1", "Q0.1", "", "", "deactivated", ""],
    ["2", "Q0.2", "", "", "used", "201-E6"],
    ["3", "Q0.3", "", "", "deactivated", ""],
    ["4", "Q0.4", "", "Spare", "used", "201-E8"],
    ["5", "Q0.5", "", "", "deactivated", ""],
    ["6", "Q0.6", "", "Spare", "used", "201-E9"],
    ["7", "Q0.7", "", "", "deactivated", ""],
];

/// PX-3: a selection naming a module lists that module's rows only; one naming the CPU's station lists the rows of
/// its two cards, as folio 908's block lists them.
#[test]
fn a_module_selection_lists_that_module_only_and_a_station_selection_its_cards() {
    let module = overview_box(&["-104A1-A1"]);
    let station = overview_box(&["-104A1"]);
    let both = overview_box(&["-104A1-A1", "-201K1"]);
    let q = session_with(module.clone()).query();
    let modules: Vec<String> = rows(&q, module.id).into_iter().map(|r| r[0].clone()).collect();
    assert_eq!(modules, vec!["-104A1-A1".to_owned(); 8]);
    let q = session_with(station.clone()).query();
    let page_908 = q
        .report_blocks()
        .iter()
        .find(|(_, b)| b.first_folio == Some(908))
        .map(|(id, _)| id)
        .expect("the PLC overview");
    let cards: Vec<Vec<String>> = rows(&q, page_908)
        .into_iter()
        .filter(|r| r[0].starts_with("-104A1-"))
        .collect();
    assert_eq!(cards.len(), 16, "two cards of eight channels");
    assert_eq!(rows(&q, station.id), cards);
    let q = session_with(both.clone()).query();
    let modules: Vec<String> = rows(&q, both.id).into_iter().map(|r| r[0].clone()).collect();
    let mut expected = vec!["-104A1-A1".to_owned(); 8];
    expected.extend(vec!["-201K1".to_owned(); 8]);
    assert_eq!(modules, expected, "in plc_rows order");
}

/// PX-3: "drawing on it changes no potential": the box has no connection points, so the sample's potential partition
/// is the same with and without it.
#[test]
fn the_box_changes_no_potential() {
    let without = common::sample_with_legend_box();
    let with = session_with(overview_box(&["-201K1"]));
    let a = without.query().get::<PotentialPartition>(());
    let b = with.query().get::<PotentialPartition>(());
    assert_eq!(*a, *b);
}
