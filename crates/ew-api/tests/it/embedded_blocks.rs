// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded report blocks drawn through one path, on the sample project: its legend on the panel page 902, and blocks
//! a test adds to the sample's records (a PART DATA box on 902; a wire list, a block of a newer source and three
//! linking kinds on 101).
//!
//! - `part_data_on_902_lists_the_enclosure_and_the_plate_under_its_title`
//! - `a_kind_without_a_layout_draws_every_column_and_reports_the_rows_it_cannot_hold`,
//!   `a_block_whose_rows_do_not_derive_draws_its_reason`, `the_tables_layer_does_not_depend_on_record_order`
//! - `the_legend_exports_through_the_named_block_path_and_no_default_export_changes`
//! - `an_embedded_block_draws_no_back_links`
//!
//! How a boxed table fills its flow columns, reports the rows beyond its capacity and draws PART DATA's order codes is
//! `ew-scene`'s `embedded.rs`; the legend's rows are `ew-derive`'s `layout_bom.rs` and `ew-reports`'
//! `enclosure_legend.rs`. Every page's bytes are the expected files of `page_output`.

use crate::common;

use ew_api::output::{RowExport, page_layer, page_view, row_export};
use ew_api::Session;
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::{PageOrder, PageRef, Q};
use ew_dl::{DisplayList, ItemKind, Layer};
use ew_reports::BlockRows;
use ew_scene::embedded::{EmbeddedContent, EmbeddedTable};
use ew_scene::forms::SheetMap;
use ew_scene::page::PageFinding;
use ew_scene::table::TableFinding;
use ew_scene::table::TableLink;
use ew_schema::{Bounds, Page, Record, ReportBlock, ReportKind, ReportSource};
use serde_json::json;

/// The frame form's page-to-sheet map (the QET frames of the sample's pages).
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

/// A page rectangle from printed hundredths of a millimetre.
fn printed_rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Bounds {
    let p = |x: i64, y: i64| SHEET.unapply(Pt2::new(Nm(x * 10_000), Nm(y * 10_000)));
    Bounds(Rect::from_points(p(x0, y0), p(x1, y1)))
}

/// The sample's one embedded block: the enclosure legend on 902.
fn legend(records: &[Record]) -> ReportBlock {
    records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if b.embedded.is_some() => Some(b.clone()),
            _ => None,
        })
        .expect("the sample's legend")
}

/// The ID of folio `folio`'s page among the records.
fn page_id(records: &[Record], folio: u32) -> Id<Page> {
    records
        .iter()
        .find_map(|r| match r {
            Record::Page(p) if p.folio.number == folio => Some(p.id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("folio {folio}"))
}

/// A block of the legend's form with its own ID, source, page and rect, and the kind's own columns and title.
fn fixture_block(records: &[Record], n: u128, source: ReportSource, page: Id<Page>, rect: Bounds) -> ReportBlock {
    let mut b = legend(records);
    b.id = Id::from_uuid(Uuid::from_u128(0x4b42_0000 + n));
    b.source = source;
    b.columns = Vec::new();
    b.sort = Vec::new();
    b.group = None;
    b.title = Default::default();
    b.source_ref = None;
    let embed = b.embedded.as_mut().expect("embedded");
    embed.page = page;
    embed.rect = rect;
    b
}

/// A session over the sample's records, `edit` applied first.
fn sample_with(edit: impl FnOnce(&mut Vec<Record>)) -> Session {
    let (project, mut records, lib) = common::sample_records();
    edit(&mut records);
    common::session_of(project, records, lib)
}

/// LB-3: a PART DATA box on the panel page 902 lists exactly the enclosure's and the plate's parts, EW-GEN-ENC-1 and
/// EW-GEN-PLT-1 (no carrier line), under the title "PART DATA" (the source through the surface view). It stands in
/// place of the legend, whose rows the sample's box does not hold.
#[test]
fn part_data_on_902_lists_the_enclosure_and_the_plate_under_its_title() {
    let s = sample_with(|records| {
        let legend = legend(records);
        let rect = legend.embedded.as_ref().unwrap().rect;
        let page = legend.embedded.as_ref().unwrap().page;
        let part_data = fixture_block(records, 3, ReportSource::Kind(ReportKind::PartData), page, rect);
        records.retain(|r| !matches!(r, Record::ReportBlock(b) if b.id == legend.id));
        records.push(Record::ReportBlock(part_data));
    });
    let q = s.query();
    let (texts, findings) = tables(&q, "902");
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(texts, ["PART DATA", "EW-GEN-ENC-1", "EW-GEN-PLT-1"]);
}

/// LB-8: `ew export report --block <the legend's ID> --format csv` writes a header and every row of the legend, all
/// of them, whatever its box draws (D18: the named-block path); the default blocks of `bom`, `cables`, `wires` and
/// `terminals` are not embedded blocks.
#[test]
fn the_legend_exports_through_the_named_block_path_and_no_default_export_changes() {
    let s = common::sample();
    let q = s.query();
    let (_, records, _) = common::sample_records();
    let legend = legend(&records);
    let out = row_export(&q, &RowExport::Report, Some(&legend.id.to_string())).expect("the legend's rows");
    assert_eq!(out.rows, q.get::<BlockRows>(legend.id).rows.len());
    assert_eq!(out.kind, "enclosure_legend");
    assert_eq!(out.blocks, [legend.id.to_string()]);
    assert_eq!(out.csv.lines().count(), out.rows + 1);
    let header = out.csv.lines().next().expect("a header");
    assert!(header.starts_with("row,Pos,DT,"), "{header}");
    // The legend's first rows: the enclosure and its plate.
    let lines: Vec<&str> = out.csv.lines().collect();
    assert!(lines[1].starts_with("data,1,-UC1,"), "{}", lines[1]);
    assert!(lines[2].starts_with("data,2,-UC1-plate,"), "{}", lines[2]);
    for what in [
        RowExport::Bom(ew_api::BomMode::Component),
        RowExport::Cables,
        RowExport::Wires,
        RowExport::Terminals { strip: None },
    ] {
        let out = row_export(&q, &what, None).expect("a default block");
        assert!(
            !out.blocks.contains(&legend.id.to_string()),
            "{what:?}: {:?}",
            out.blocks
        );
    }
}

/// LB-6: a `wire_list` block (a kind that declares no layout) embedded on schematic page 101 draws every column under
/// a header row, the rows its rect holds, and reports the rest.
#[test]
fn a_kind_without_a_layout_draws_every_column_and_reports_the_rows_it_cannot_hold() {
    let id = Id::from_uuid(Uuid::from_u128(0x4b42_0001));
    let s = sample_with(|records| {
        // 16.31–277.27 × 20.00–88.00 mm printed: 68 mm hold ten rows of 6.52 mm, a header and nine data rows.
        let wires = fixture_block(
            records,
            1,
            ReportSource::Kind(ReportKind::WireList),
            page_id(records, 101),
            printed_rect(1_631, 2_000, 27_727, 8_800),
        );
        records.push(Record::ReportBlock(wires));
    });
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
    // Every column at weight 1 leaves the longest wire-list texts too wide: they shrink, are cut with an ellipsis at
    // the smallest size and reported (never printed over the next cell), and only the nine drawn rows report.
    for f in &findings[1..] {
        match f {
            PageFinding::Table(TableFinding::Overflow {
                row: Some(r), text, ..
            }) => {
                assert!(*r < 9, "{f:?}");
                assert!(
                    texts
                        .iter()
                        .any(|t| t.ends_with('…') && text.starts_with(t.trim_end_matches('…'))),
                    "{text} is drawn cut"
                );
            }
            other => panic!("{other:?}"),
        }
    }
}

/// LB-6: a block whose rows do not derive draws its reason.
#[test]
fn a_block_whose_rows_do_not_derive_draws_its_reason() {
    let s = sample_with(|records| {
        let newer = fixture_block(
            records,
            2,
            ReportSource::Unknown(json!({"newer": {}})),
            page_id(records, 101),
            printed_rect(1_631, 2_000, 27_727, 4_000),
        );
        records.push(Record::ReportBlock(newer));
    });
    let q = s.query();
    let (texts, findings) = tables(&q, "101");
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(
        texts,
        ["Not derived: `unknown source` is not supported in this build"]
    );
}

/// LB-6 and LB-10: the tables layer of the page with the embedded legend does not depend on the record order.
#[test]
fn the_tables_layer_does_not_depend_on_record_order() {
    let a = common::sample();
    let b = sample_with(|records| records.reverse());
    let (qa, qb) = (a.query(), b.query());
    let (la, lb) = (
        page_layer(&qa, drawn(&qa, "902"), Layer::Tables),
        page_layer(&qb, drawn(&qb, "902"), Layer::Tables),
    );
    assert!(!la.list.items.is_empty(), "the legend is drawn");
    assert_eq!(*la.list, *lb.list);
    assert_eq!(la.findings, lb.findings);
}

/// An embedded block draws no back-links. `parts_summary`, `toc` and
/// `structure_overview` blocks embedded on 101 have rows that link (their generated pages draw those links), and their
/// boxes on 101 draw none.
#[test]
fn an_embedded_block_draws_no_back_links() {
    let kinds = [
        ReportKind::PartsSummary,
        ReportKind::Toc,
        ReportKind::StructureOverview,
    ];
    let ids: Vec<Id<ReportBlock>> = (0..3u128)
        .map(|n| Id::from_uuid(Uuid::from_u128(0x4b42_0000 + 10 + n)))
        .collect();
    let s = sample_with(|records| {
        let page = page_id(records, 101);
        let blocks: Vec<ReportBlock> = kinds
            .iter()
            .zip(0i64..)
            .map(|(kind, n)| {
                let y = 2_000 + 3_000 * n;
                fixture_block(
                    records,
                    10 + u128::try_from(n).expect("a small index"),
                    ReportSource::Kind(kind.clone()),
                    page,
                    printed_rect(1_631, y, 27_727, y + 2_800),
                )
            })
            .collect();
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
