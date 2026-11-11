// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.01 part 4b over Golden A: every embedded report block drawn through one path (D16 to D18).
//!
//! - `part_data_on_902_lists_the_enclosure_and_the_plate_under_its_title`
//! - `a_kind_without_a_layout_draws_every_column_and_reports_the_rows_it_cannot_hold`,
//!   `a_block_whose_rows_do_not_derive_draws_its_reason`, `the_tables_layer_does_not_depend_on_record_order`
//! - `the_legend_on_902_draws_20_plus_11_rows_and_20_plus_12_with_f55_placed` (WP-S4.02 EB-3: the end-bracket row
//!   is row 31; the block holds 2 × 20 rows, asserted first)
//! - `the_legend_exports_through_the_named_block_path_and_no_default_export_changes`
//! - `part_data_on_201_and_202_draws_both_order_codes_with_no_overflow`
//! - `an_embedded_block_draws_no_back_links`
//!
//! Folio 1's byte equality (LB-6) and every other page's (LB-10) are the expected files of `page_output`; LP-5 with the
//! legend on 902 is `page_output_panel::lp5_…` (a `layout.move` runs no `tables` family).

use crate::{common, golden_common};

use ew_api::output::{RowExport, page_layer, page_view, row_export};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::{PageOrder, PageRef, Q};
use ew_dl::{DisplayList, ItemKind, Layer};
use ew_import_tables::ids::record_id;
use ew_reports::BlockRows;
use ew_scene::embedded::{EmbeddedContent, EmbeddedTable, boxed_capacity};
use ew_scene::forms::SheetMap;
use ew_scene::page::PageFinding;
use ew_scene::table::TableColumn;
use ew_scene::table::TableFinding;
use ew_scene::table::TableLink;
use ew_schema::{Bounds, Carrier, Device, Page, Record, ReportBlock, ReportKind, ReportSource};
use serde_json::{Value, json};

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

/// The golden report block of a `report_blocks` key.
fn block(key: &str) -> ReportBlock {
    let want = format!("report_blocks|{key}");
    golden_common::batch()
        .records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if golden_common::source_key(r) == Some(want.as_str()) => Some(b.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no report block `{key}`"))
}

fn texts(list: &DisplayList) -> Vec<String> {
    list.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect()
}

/// The tables layer of a folio: its texts and findings.
fn tables(q: &Q, folio: &str) -> (Vec<String>, Vec<PageFinding>) {
    let l = page_layer(q, drawn(q, folio), Layer::Tables);
    (texts(&l.list), l.findings.clone())
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

fn run(session: &mut Session, name: &str, params: Value) {
    let cmd = AnyCommand {
        name: name.into(),
        params: params.clone(),
    };
    match session.execute(&cmd) {
        Err(CmdError::NeedsPreview { .. }) => session.preview(&cmd).and_then(|p| session.commit(p)),
        other => other,
    }
    .unwrap_or_else(|e| panic!("{name} {params}: {e:?}"));
}

/// A page rectangle from printed hundredths of a millimetre.
fn printed_rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Bounds {
    let p = |x: i64, y: i64| SHEET.unapply(Pt2::new(Nm(x * 10_000), Nm(y * 10_000)));
    Bounds(Rect::from_points(p(x0, y0), p(x1, y1)))
}

/// A block like 902's PART DATA, with its own ID, source, page and rect.
fn fixture_block(n: u128, source: ReportSource, page: Id<Page>, rect: Bounds) -> ReportBlock {
    let mut b = block("part-data-902");
    b.id = Id::from_uuid(Uuid::from_u128(0x4b42_0000 + n));
    b.source = source;
    b.source_ref = None;
    let embed = b.embedded.as_mut().expect("embedded");
    embed.page = page;
    embed.rect = rect;
    b
}

/// LB-3: "PART DATA on 902 lists exactly NSYPLA1074G and NSYPMM107 (no carrier line) and draws them under the title
/// "PART DATA"" (the source through the surface view: the orchestrator's).
#[test]
fn part_data_on_902_lists_the_enclosure_and_the_plate_under_its_title() {
    let s = common::golden_session();
    let q = s.query();
    let (texts, findings) = tables(&q, "902");
    assert!(findings.is_empty(), "{findings:?}");
    // The blocks print in the order of their rectangles: PART DATA (top 16.31 mm), then the legend (19.50 mm).
    assert_eq!(texts[..4], ["PART DATA", "NSYPLA1074G", "NSYPMM107", "LEGEND"]);
}

/// The position texts of the legend's two flow columns: the runs that start where each flow column's "Pos" header
/// starts.
fn legend_positions(list: &DisplayList) -> (Vec<String>, Vec<String>) {
    let runs: Vec<(String, Nm)> = list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some((run.text.clone(), run.origin.x)),
            _ => None,
        })
        .collect();
    let heads: Vec<Nm> = runs.iter().filter(|(t, _)| t == "Pos").map(|(_, x)| *x).collect();
    assert_eq!(heads.len(), 2, "one header per flow column");
    let column = |x: Nm| -> Vec<String> {
        runs.iter()
            .filter(|(t, at)| *at == x && t != "Pos" && t != "LEGEND")
            .map(|(t, _)| t.clone())
            .collect()
    };
    (column(heads[0]), column(heads[1]))
}

fn numbers(range: std::ops::RangeInclusive<usize>) -> Vec<String> {
    range.map(|n| n.to_string()).collect()
}

/// LB-7: the legend block on 902 draws D17's rect with the title "LEGEND", two flow columns each with its header, and
/// Golden A's 31 rows as 20 + 11 (WP-S4.02 EB-3: the end brackets are row 31; ruling 7: the block's capacity is
/// asserted first, at least 31); placing F55 on R1 draws 32 as 20 + 12 and renumbers; no text reports an overflow.
#[test]
fn the_legend_on_902_draws_20_plus_11_rows_and_20_plus_12_with_f55_placed() {
    let legend = block("legend-902");
    let rect = legend.embedded.as_ref().expect("embedded on 902").rect.0;
    let table = EmbeddedTable {
        title: "LEGEND".to_owned(),
        columns: ["Pos", "DT", "Qty", "Part number"]
            .iter()
            .zip([10, 22, 16, 35])
            .map(|(h, weight)| TableColumn {
                header: (*h).to_owned(),
                weight,
            })
            .collect(),
        headers: true,
        flow_columns: 2,
        rows: Vec::new(),
        note: None,
    };
    let (flows, rows) = boxed_capacity(rect, &table, SHEET);
    assert_eq!((flows, rows), (2, 20));
    assert!(flows * rows >= 31, "EB-3: the legend block holds the 31 rows");

    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let q = s.query();
    let l = page_layer(&q, drawn(&q, "902"), Layer::Tables);
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    let (first, second) = legend_positions(&l.list);
    assert_eq!((first, second), (numbers(1..=20), numbers(21..=31)));
    // Row 31: the end brackets, no DT, quantity 4, the TBD part.
    let all = texts(&l.list);
    let k = all.iter().position(|t| t == "31").expect("row 31");
    assert_eq!(all[k..k + 3], ["31", "4", "TBD"]);
    // The header of each flow column, in D17's order and sentence case.
    let all = texts(&l.list);
    let k = all.iter().position(|t| t == "LEGEND").expect("the title");
    assert_eq!(all[k + 1..k + 5], ["Pos", "DT", "Qty", "Part number"]);
    // The box: the legend's rect, 108.00–278.00 × 19.50–167.50 mm printed, its 2.61 mm shadow to the right and below.
    let b = l.list.bounds().map(|r| SHEET.apply_rect(r)).expect("items");
    let near = |a: Nm, mm100: i64| (a.0 - mm100 * 10_000).abs() <= 2_000;
    assert!(
        near(b.max().x, 27_800 + 261) && near(b.max().y, 16_750 + 261),
        "{b:?}"
    );
    drop(q);

    run(
        &mut s,
        "layout.place",
        json!({"device": id_json(record_id::<Device>("devices", "F55")),
               "carrier": {"carrier": id_json(record_id::<Carrier>("carriers", "R1"))},
               "pose": {"t": [400_000_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
    );
    let q = s.query();
    let l = page_layer(&q, drawn(&q, "902"), Layer::Tables);
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    let (first, second) = legend_positions(&l.list);
    assert_eq!((first, second), (numbers(1..=20), numbers(21..=32)));
}

/// LB-8: `ew export report --block <legend-902's ID> --format csv` writes a header and 31 rows with every column of the
/// kind, the description included (D18: the named-block path); the default blocks of `bom`, `cables`, `wires` and
/// `terminals` are not embedded blocks, so those exports are unchanged (their bytes: `golden_csv`).
#[test]
fn the_legend_exports_through_the_named_block_path_and_no_default_export_changes() {
    let s = common::golden_session();
    let q = s.query();
    let legend = block("legend-902");
    let out = row_export(&q, &RowExport::Report, Some(&legend.id.to_string())).expect("the legend's rows");
    assert_eq!(out.rows, 31);
    assert_eq!(out.kind, "enclosure_legend");
    assert_eq!(out.blocks, [legend.id.to_string()]);
    let header = out.csv.lines().next().expect("a header");
    assert_eq!(header, "row,Pos,DT,Manufacturer,Part number,Description,Qty,Unit");
    assert_eq!(out.csv.lines().count(), 32);
    // The last row: the end brackets (EB-3), the DT empty, quantity 4 pieces.
    let last = out.csv.lines().last().expect("a row");
    assert_eq!(last, "data,31,,TBD,TBD,End bracket,4,pc");
    let embedded: Vec<String> = [
        "legend-902",
        "part-data-201",
        "part-data-202",
        "part-data-902",
        "cover-tables",
    ]
    .iter()
    .map(|k| block(k).id.to_string())
    .collect();
    for what in [
        RowExport::Bom(ew_api::BomMode::Component),
        RowExport::Cables,
        RowExport::Wires,
        RowExport::Terminals { strip: None },
    ] {
        let out = row_export(&q, &what, None).expect("a default block");
        assert!(
            out.blocks.iter().all(|b| !embedded.contains(b)),
            "{what:?}: {:?}",
            out.blocks
        );
    }
}

/// LB-9: PART DATA on 201 and 202 draws "Order Code 1" 51E-F13GCQP-D+H2GG and "Order Code 2"
/// 32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS under the title "PART DATA", whole, with no overflow finding. The ruling on lane
/// A42's LB-9 stop: "PART DATA's label : part number weights are 3 : 10" (at 1 : 3 the 35-character code
/// overflows at the 1.8 mm floor).
#[test]
fn part_data_on_201_and_202_draws_both_order_codes_with_no_overflow() {
    let s = common::golden_session();
    let q = s.query();
    for folio in ["201", "202"] {
        let (texts, findings) = tables(&q, folio);
        assert!(findings.is_empty(), "folio {folio}: {findings:?}");
        assert_eq!(
            texts,
            [
                "PART DATA",
                "Order Code 1",
                "51E-F13GCQP-D+H2GG",
                "Order Code 2",
                "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS",
            ],
            "folio {folio}"
        );
    }
}

/// LB-6: a `wire_list` block (a kind that declares no layout) embedded on schematic page 101 draws every column under
/// a header row (the orchestrator's), the rows its rect holds, and reports the rest.
#[test]
fn a_kind_without_a_layout_draws_every_column_and_reports_the_rows_it_cannot_hold() {
    let batch = golden_common::batch();
    // 16.31–277.27 × 20.00–88.00 mm printed: 68 mm hold ten rows of 6.52 mm, a header and nine data rows.
    let wires = fixture_block(
        1,
        ReportSource::Kind(ReportKind::WireList),
        record_id("pages", "101"),
        printed_rect(1_631, 2_000, 27_727, 8_800),
    );
    let id = wires.id;
    let s = golden_common::session_with(&batch, |_, records| records.push(Record::ReportBlock(wires)));
    let q = s.query();
    let set = q.get::<BlockRows>(id);
    let labels: Vec<String> = set
        .columns
        .iter()
        .map(|c| c.label.as_neutral().expect("a neutral label").to_owned())
        .collect();
    assert!(
        set.rows.len() > 9,
        "the wire list has more rows than the rect holds"
    );
    let (texts, findings) = tables(&q, "101");
    assert_eq!(texts[..labels.len()], labels[..]);
    assert_eq!(
        findings[0],
        PageFinding::Table(TableFinding::TooManyRows {
            given: set.rows.len(),
            drawn: 9
        })
    );
    // Every column at weight 1 leaves the longest wire-list texts too wide: they shrink, are reported and drawn whole,
    // and only the nine drawn rows report.
    for f in &findings[1..] {
        match f {
            PageFinding::Table(TableFinding::Overflow {
                row: Some(r), text, ..
            }) => {
                assert!(*r < 9, "{f:?}");
                assert!(texts.contains(text), "{text} is drawn whole");
            }
            other => panic!("{other:?}"),
        }
    }
}

/// LB-6: a block whose rows do not derive draws its reason.
#[test]
fn a_block_whose_rows_do_not_derive_draws_its_reason() {
    let batch = golden_common::batch();
    let newer = fixture_block(
        2,
        ReportSource::Unknown(json!({"newer": {}})),
        record_id("pages", "101"),
        printed_rect(1_631, 2_000, 27_727, 4_000),
    );
    let s = golden_common::session_with(&batch, |_, records| records.push(Record::ReportBlock(newer)));
    let q = s.query();
    let (texts, findings) = tables(&q, "101");
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(
        texts,
        ["Not derived: `unknown source` is not supported in this build"]
    );
}

/// LB-6 and LB-10: the tables layers of the pages with embedded blocks do not depend on the record order.
#[test]
fn the_tables_layer_does_not_depend_on_record_order() {
    let a = common::golden_session();
    let batch = common::golden_batch();
    let (project, mut records) = common::split(&batch);
    records.reverse();
    let b = common::session_of(project, records, batch.lib.clone());
    let (qa, qb) = (a.query(), b.query());
    for folio in ["1", "201", "202", "902"] {
        let (la, lb) = (
            page_layer(&qa, drawn(&qa, folio), Layer::Tables),
            page_layer(&qb, drawn(&qb, folio), Layer::Tables),
        );
        assert_eq!(*la.list, *lb.list, "folio {folio}");
        assert_eq!(la.findings, lb.findings, "folio {folio}");
    }
}

/// WP-S3.06 ruling 4: an embedded block draws no back-links. `parts_summary`, `toc` and
/// `structure_overview` blocks embedded on 101 have rows that link (their generated pages draw those links), and their
/// boxes on 101 draw none.
#[test]
fn an_embedded_block_draws_no_back_links() {
    let batch = golden_common::batch();
    let kinds = [
        ReportKind::PartsSummary,
        ReportKind::Toc,
        ReportKind::StructureOverview,
    ];
    let blocks: Vec<ReportBlock> = kinds
        .iter()
        .zip(0i64..)
        .map(|(kind, n)| {
            let y = 2_000 + 3_000 * n;
            fixture_block(
                10 + u128::try_from(n).expect("a small index"),
                ReportSource::Kind(kind.clone()),
                record_id("pages", "101"),
                printed_rect(1_631, y, 27_727, y + 2_800),
            )
        })
        .collect();
    let ids: Vec<Id<ReportBlock>> = blocks.iter().map(|b| b.id).collect();
    let s = golden_common::session_with(&batch, |_, records| {
        records.extend(blocks.into_iter().map(Record::ReportBlock));
    });
    let q = s.query();
    for (id, kind) in ids.iter().zip(&kinds) {
        let set = q.get::<BlockRows>(*id);
        assert!(
            set.rows.iter().any(|r| !r.links.is_empty()),
            "{kind:?}'s rows link"
        );
    }
    let entry = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.folio == "101")
        .expect("folio 101")
        .clone();
    let view = page_view(&q, &entry);
    let tables: Vec<&EmbeddedTable> = view
        .embedded
        .iter()
        .filter_map(|e| match &e.content {
            EmbeddedContent::Table(t) => Some(t),
            EmbeddedContent::Cover(_) => None,
        })
        .collect();
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles.len(), 3, "{titles:?}");
    let links: Vec<(&str, &[TableLink])> = tables
        .iter()
        .flat_map(|t| t.rows.iter().map(|r| (t.title.as_str(), r.links.as_slice())))
        .filter(|(_, links)| !links.is_empty())
        .collect();
    assert_eq!(links, []);
}
