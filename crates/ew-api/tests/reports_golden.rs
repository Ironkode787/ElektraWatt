// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in report kinds over the golden project (doc 20 §5, §14, §16): row counts, generated pages,
//! stable row IDs and determinism.

mod common;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{Record, ReportBlock};
use ew_derive::{DeviceTag, GeneratedPage, PageOrder, PageOrderV, PageRef, Q};
use ew_reports::{BlockRows, Cell, NotDerived, RowSet, RowSetStatus};

/// The block of a golden `report_blocks` row.
fn block(key: &str) -> Id<ReportBlock> {
    let want = format!("report_blocks|{key}");
    common::golden_batch()
        .records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if common::source_key(r) == Some(want.as_str()) => Some(b.id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no report block `{key}`"))
}

/// Every golden report block.
const KEYS: [&str; 15] = [
    "2",
    "901",
    "903",
    "904",
    "905",
    "906",
    "907",
    "908",
    "909",
    "910",
    "911",
    "cover-tables",
    "part-data-201",
    "part-data-202",
    "part-data-902",
];

/// A text as one string: the neutral text, else the first language.
fn text(t: &MlText) -> String {
    if let Some(n) = t.as_neutral() {
        return n.to_owned();
    }
    t.languages()
        .next()
        .and_then(|l| t.get(l))
        .unwrap_or_default()
        .to_owned()
}

/// The generated pages of a block in the page order.
fn pages_of(order: &PageOrderV, b: Id<ReportBlock>) -> Vec<&GeneratedPage> {
    order
        .entries
        .iter()
        .filter(|e| matches!(e.page, PageRef::Generated { block, .. } if block == b))
        .filter_map(|e| order.generated(e.page))
        .collect()
}

/// `(label, title, rows)` of every generated page of a block.
fn page_summary(order: &PageOrderV, b: Id<ReportBlock>) -> Vec<(String, String, std::ops::Range<u32>)> {
    pages_of(order, b)
        .into_iter()
        .map(|p| (p.label.clone(), text(&p.title), p.rows.clone()))
        .collect()
}

fn ident(c: &Cell) -> &str {
    match c {
        Cell::Ident(s) => s,
        other => panic!("not an ident: {other:?}"),
    }
}

/// The part numbers of column `part_number`.
fn part_numbers(rows: &RowSet) -> Vec<String> {
    let col = rows.column("part_number").expect("a part_number column");
    rows.rows
        .iter()
        .map(|r| ident(&r.cells[col]).to_owned())
        .collect()
}

#[test]
fn parts_summary_901_derives_the_golden_tables_bom() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let b = block("901");
    let rows = q.get::<BlockRows>(b);

    // Doc 20 §14.1 lists 27 rows (901 holds 23, 901.1 holds 4). The golden tables carry no parts for the
    // rails, ducts, end brackets and bridges (rows 24-27, all TBD), and the terminal rows 13 and 14 of the
    // document are unplaced here, so the derived BOM has 23 rows: one page, 901.
    assert!(rows.is_derived());
    assert_eq!(rows.rows.len(), 23);
    assert_eq!(
        page_summary(&order, b),
        vec![("901".to_owned(), "Partlist 1".to_owned(), 0..23)]
    );
    for r in &rows.rows {
        assert_eq!(r.cells.len(), rows.columns.len());
    }

    // Row 1: the PROFIBUS plug 532216 at -3X1 on folio 3.
    let first = &rows.rows[0];
    assert_eq!(ident(&first.cells[0]), "1");
    assert_eq!(ident(&first.cells[4]), "532216");
    let Cell::Range(tags) = &first.cells[6] else {
        panic!("tags column")
    };
    assert_eq!(tags.len(), 1);
    let tag = q.get::<DeviceTag>(tags[0]);
    assert!(tag.as_ref().as_ref().unwrap().full.ends_with("-3X1"));
    let Cell::Folios(folios) = &first.cells[7] else {
        panic!("folios column")
    };
    let labels: Vec<&str> = folios
        .iter()
        .map(|p| order.entry(*p).expect("in the page order").folio.as_str())
        .collect();
    assert_eq!(labels, ["3"]);

    // Placed rows first; the enclosure and the plate follow them, unplaced.
    let numbers = part_numbers(&rows);
    let placed = |i: usize| matches!(&rows.rows[i].cells[7], Cell::Folios(f) if !f.is_empty());
    assert!((0..19).all(placed));
    assert!((19..23).all(|i| !placed(i)));
    assert_eq!(
        &numbers[19..],
        ["NSYPLA1074G", "NSYPMM107", "PT 2.5-TWIN", "PT 2.5-TWIN-PE"]
    );
}

#[test]
fn toc_rows_follow_the_page_order() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let b = block("2");
    let n = order.len() as u32;

    let pages = pages_of(&order, b);
    let labels: Vec<&str> = pages.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["2", "2.1"]);
    let titles: Vec<String> = pages.iter().map(|p| text(&p.title)).collect();
    assert_eq!(titles, ["Table of Contents 1", "Table of Contents 2"]);
    // 23 rows per page; the second page ends at the page-order length (the TOC lists every page).
    assert_eq!(pages[0].rows, 0..23);
    assert_eq!(pages[1].rows, 23..n);

    let rows = q.get::<BlockRows>(b);
    assert_eq!(rows.rows.len() as u32, n);
    let folio = rows.column("folio").unwrap();
    let toc_labels: Vec<&str> = rows
        .rows
        .iter()
        .map(|r| {
            let Cell::Folio(p) = &r.cells[folio] else {
                panic!("folio cell")
            };
            order.entry(*p).unwrap().folio.as_str()
        })
        .collect();
    assert_eq!(toc_labels, order.labels());
}

#[test]
fn structure_overview_910_has_four_nodes_and_68_non_terminal_devices() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let b = block("910");
    let rows = q.get::<BlockRows>(b);
    assert!(rows.is_derived());
    // Doc 20 §16: 4 + 68 = 72 (the 28 terminal devices are left out).
    assert_eq!(rows.rows.len(), 72);
    let kind = rows.column("kind").unwrap();
    let structure = rows
        .rows
        .iter()
        .filter(|r| matches!(&r.cells[kind], Cell::Text(t) if text(t) == "Structure"))
        .count();
    assert_eq!(structure, 4);
    assert_eq!(
        page_summary(&order, b),
        vec![
            ("910".to_owned(), "Device Tag Overview 1".to_owned(), 0..23),
            ("910.1".to_owned(), "Device Tag Overview 2".to_owned(), 23..46),
            ("910.2".to_owned(), "Device Tag Overview 3".to_owned(), 46..69),
            ("910.3".to_owned(), "Device Tag Overview 4".to_owned(), 69..72),
        ]
    );
}

#[test]
fn not_derived_kinds_paginate_to_one_placeholder_page() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    for (key, title) in [
        ("903", "Terminal Diagram 1X1"),
        ("904", "Terminal Diagram -X1031"),
        ("905", "Terminal Diagram -X1032"),
        ("906", "Cable Overview and Diagrams"),
        ("907", "Wire List 1"),
        ("908", "PLC Overview -201U1 1"),
        ("909", "Potential Overview"),
        ("911", "Revision Overview"),
    ] {
        let b = block(key);
        assert!(
            matches!(q.get::<BlockRows>(b).status, RowSetStatus::NotDerived(_)),
            "{key} is not derived"
        );
        let pages = pages_of(&order, b);
        assert_eq!(pages.len(), 1, "{key}: one generated page");
        assert_eq!(text(&pages[0].title), title, "{key}");
        let note = text(pages[0].note.as_ref().unwrap_or_else(|| panic!("{key}: a note")));
        assert!(
            note.contains("awaits") || note.contains("no `"),
            "{key}: note `{note}` gives the reason"
        );
    }
}

#[test]
fn embedded_blocks_have_no_page_and_derive_part_data() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    for key in ["cover-tables", "part-data-201", "part-data-202", "part-data-902"] {
        assert!(pages_of(&order, block(key)).is_empty(), "{key} has no page");
    }

    assert_eq!(
        q.get::<BlockRows>(block("cover-tables")).status,
        RowSetStatus::NotDerived(NotDerived::NoRecordShape("colour_profile"))
    );

    // Pages 201 and 202 both show devices whose root ancestor is -201U1 (-202U1 is its child), so both boxes
    // list the part lines of -201U1 (195740, 550208; doc 09 §3.4 repeats the box on 202). Its descendants
    // are drawn as their own symbols, so the box does not repeat them. The golden tables carry no
    // configured product (▲U-24), so the lines are not "Order Code" rows.
    let r201 = q.get::<BlockRows>(block("part-data-201"));
    let r202 = q.get::<BlockRows>(block("part-data-202"));
    assert!(r201.is_derived());
    assert_eq!(part_numbers(&r201), ["195740", "550208"]);
    assert_eq!(part_numbers(&r201), part_numbers(&r202));
    assert!(r201.rows.iter().all(|r| r.cells[0] == Cell::Empty));

    // No representation on page 902 yet (placements arrive with ew-panel): no rows.
    let r902 = q.get::<BlockRows>(block("part-data-902"));
    assert!(r902.is_derived());
    assert!(r902.rows.is_empty());
}

#[test]
fn rows_and_page_order_do_not_depend_on_record_order() {
    let a = common::golden_session();
    let batch = common::golden_batch();
    let (project, mut records) = common::split(&batch);
    records.reverse();
    let b = common::session_of(project, records, batch.lib.clone());
    let (qa, qb): (Q, Q) = (a.query(), b.query());

    assert_eq!(qa.get::<PageOrder>(()).labels(), qb.get::<PageOrder>(()).labels());
    for key in KEYS {
        let id = block(key);
        let (ra, rb) = (qa.get::<BlockRows>(id), qb.get::<BlockRows>(id));
        assert_eq!(ra.rows.len(), rb.rows.len(), "{key}");
        let ids = |r: &RowSet| r.rows.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&ra), ids(&rb), "{key}: row IDs");
        assert_eq!(*ra, *rb, "{key}: rows");
    }
}

#[test]
fn rows_and_page_order_are_deterministic() {
    let (a, b) = (common::golden_session(), common::golden_session());
    let (qa, qb): (Q, Q) = (a.query(), b.query());
    assert_eq!(
        format!("{:?}", qa.get::<PageOrder>(())),
        format!("{:?}", qb.get::<PageOrder>(()))
    );
    for key in KEYS {
        let id = block(key);
        assert_eq!(
            format!("{:?}", qa.get::<BlockRows>(id)),
            format!("{:?}", qb.get::<BlockRows>(id)),
            "{key}"
        );
    }
}
