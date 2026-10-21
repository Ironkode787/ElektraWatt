// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in report kinds over the golden project (doc 20 §5, §14, §16): row counts, generated pages,
//! stable row IDs and determinism.

mod common;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{Record, ReportBlock};
use ew_derive::{DeviceTag, GeneratedPage, PageOrder, PageOrderV, PageRef, Q};
use ew_reports::render::{RenderOptions, render};
use ew_reports::{BlockRows, Cell, RowKind, RowSet, RowSetStatus};

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
        .map(|r| match &r.cells[col] {
            // A part number still to be decided prints "TBD".
            Cell::Tbd => "TBD".to_owned(),
            c => ident(c).to_owned(),
        })
        .collect()
}

#[test]
fn parts_summary_901_derives_the_golden_tables_bom() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let b = block("901");
    let rows = q.get::<BlockRows>(b);

    // Doc 20 §14.1 lists 27 rows: 901 holds 23 and the continuation page 901.1 holds the 4 TBD rows 24-27
    // (the rails, ducts, end brackets and bridges, FT-110).
    assert!(rows.is_derived());
    assert_eq!(rows.rows.len(), 27);
    assert_eq!(
        page_summary(&order, b),
        vec![
            ("901".to_owned(), "Partlist 1".to_owned(), 0..23),
            ("901.1".to_owned(), "Partlist 2".to_owned(), 23..27),
        ]
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

    // The doc 20 §14.1 order: placed rows by page order and position, the enclosure and the plate after
    // them, unplaced. The terminals of -X1031 and -X1032 have no representation of their own; they take
    // their strip's, so PT 2.5-TWIN and PT 2.5-TWIN-PE are rows 13 and 14 on folio 103, between -102F2 and
    // -106W1.
    let numbers = part_numbers(&rows);
    assert_eq!(
        numbers,
        [
            "532216",
            "6XV1830-0EH10",
            "533356",
            "UK 5 N",
            "USLKG 5",
            "A9P52610",
            "5SY4204-7",
            "SK 3110.000",
            "SK 3102.000",
            "2904602",
            "A9F78116",
            "MICO 4.10",
            "PT 2.5-TWIN",
            "PT 2.5-TWIN-PE",
            "NEBU-G78W5-K-2-N-LE5",
            "195740",
            "550208",
            "552286",
            "533361",
            "533354",
            "571334",
            "NSYPLA1074G",
            "NSYPMM107",
            // Rows 24-27, the TBD parts, in layout-legend order: rail, duct, then the layout-only
            // accessories (end bracket, bridge) by description.
            "TBD",
            "TBD",
            "TBD",
            "TBD",
        ]
    );
    let descriptions: Vec<String> = rows.rows[23..]
        .iter()
        .map(|r| match &r.cells[5] {
            Cell::Text(t) => text(t),
            other => panic!("not a text: {other:?}"),
        })
        .collect();
    assert_eq!(
        descriptions,
        [
            "DIN rail TS 35",
            "Wiring duct 40×60",
            "End bracket",
            "Plug-in bridge"
        ]
    );
    let placed = |i: usize| matches!(&rows.rows[i].cells[7], Cell::Folios(f) if !f.is_empty());
    assert!((0..21).all(placed));
    assert!((21..25).all(|i| !placed(i)));
    // The accessories keep the folios of their strips: the brackets 101 and 103, the bridges 103.
    let folios_of = |i: usize| -> Vec<String> {
        let Cell::Folios(folios) = &rows.rows[i].cells[7] else {
            panic!("folios column")
        };
        folios
            .iter()
            .map(|p| order.entry(*p).expect("in the page order").folio.clone())
            .collect()
    };
    assert_eq!(folios_of(25), ["101", "103"]);
    assert_eq!(folios_of(26), ["103"]);
    // Quantities: 2.192 m of rail and 3.81 m of duct from the carriers, 4 brackets, 6 bridges.
    let qty = |i: usize| match &rows.rows[i].cells[1] {
        Cell::Qty(q, _) => q.to_string(),
        other => panic!("not a quantity: {other:?}"),
    };
    assert_eq!([qty(23), qty(24), qty(25), qty(26)], ["2.192", "3.81", "4", "6"]);
    for i in [12, 13] {
        let Cell::Folios(folios) = &rows.rows[i].cells[7] else {
            panic!("folios column")
        };
        let labels: Vec<&str> = folios
            .iter()
            .map(|p| order.entry(*p).expect("in the page order").folio.as_str())
            .collect();
        assert_eq!(labels, ["103"], "row {}", i + 1);
        assert_eq!(ident(&rows.rows[i].cells[0]), (i + 1).to_string());
    }
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

/// Folio 908 (doc 20 §11.3, §16): 56 channels of -201U1 on three pages of 23, 23 and 10 rows; addresses
/// Q0.0 … Q6.7; 8 used, 20 spare and 28 deactivated; a used channel prints the driven process tag as function
/// text and process tag and the cell of its coil on 203; symbols are empty in Golden A.
#[test]
fn plc_overview_908_lists_the_56_channels_of_the_valve_terminal() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let b = block("908");
    assert_eq!(
        page_summary(&order, b),
        vec![
            ("908".to_owned(), "PLC Overview -201U1 1".to_owned(), 0..23),
            ("908.1".to_owned(), "PLC Overview -201U1 2".to_owned(), 23..46),
            ("908.2".to_owned(), "PLC Overview -201U1 3".to_owned(), 46..56),
        ]
    );
    let set = q.get::<BlockRows>(b);
    assert!(set.is_derived());
    assert_eq!(set.rows.len(), 56);
    let col = |k: &str| set.column(k).unwrap_or_else(|| panic!("column {k}"));
    let rows: Vec<Vec<String>> = (0..set.rows.len()).map(|i| printed(&q, &set, i)).collect();
    let addresses: Vec<&str> = rows.iter().map(|r| r[col("address")].as_str()).collect();
    let want: Vec<String> = (0..7)
        .flat_map(|b| (0..8).map(move |i| format!("Q{b}.{i}")))
        .collect();
    assert_eq!(addresses, want.iter().map(String::as_str).collect::<Vec<_>>());
    let count = |state: &str| rows.iter().filter(|r| r[col("state")] == state).count();
    assert_eq!((count("used"), count("spare"), count("deactivated")), (8, 20, 28));
    let used = &rows[1];
    assert_eq!(
        (
            used[col("module")].as_str(),
            used[col("target")].as_str(),
            used[col("function")].as_str(),
            used[col("process_tag")].as_str(),
            used[col("page")].as_str(),
        ),
        ("-201U3", "-201U3-Q1", "TA05-V-15", "TA05-V-15", "203-D1")
    );
    let spare = &rows[2 * 8 + 1];
    assert_eq!(
        (
            spare[col("target")].as_str(),
            spare[col("function")].as_str(),
            spare[col("process_tag")].as_str()
        ),
        ("-201U5-Q1", "Spare", "")
    );
    assert!(
        rows.iter().all(|r| r[col("symbol")].is_empty()),
        "symbols are empty in Golden A"
    );
    assert_eq!(rows[55][col("module")], "-202U2");
}

#[test]
fn not_derived_kinds_paginate_to_one_placeholder_page() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    // The connectivity kinds derive inside the pagination (their queries read the drawn page order), the PLC
    // overview from the channel records; only the kind without a row source keeps the placeholder page.
    for key in ["903", "904", "905", "906", "907", "908", "909"] {
        assert!(q.get::<BlockRows>(block(key)).is_derived(), "{key} is derived");
    }
    let (key, title) = ("911", "Revision Overview");
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

/// The printed cells of row `i` of a row set.
fn printed(q: &Q, set: &RowSet, i: usize) -> Vec<String> {
    let opts = RenderOptions::default();
    set.rows[i].cells.iter().map(|c| render(q, c, &opts)).collect()
}

/// The cover's colour and standard-size tables are rows of the profiles in the project settings (FT-159,
/// doc 20 §7.1, §7.2): the basis, the 15 colour rows, then the control and the mains sizes.
#[test]
fn the_cover_tables_follow_the_profiles() {
    let s = common::golden_session();
    let q = s.query();
    let set = q.get::<BlockRows>(block("cover-tables"));
    assert!(set.is_derived());
    // Per table its title; the 15 colour rows with the standards caption; 3 + 3 size rows; the note.
    assert_eq!(set.rows.len(), (1 + 15 + 1) + (1 + 3) + (1 + 3) + 1);
    let labels: Vec<String> = set.columns.iter().map(|c| text(&c.label)).collect();
    assert_eq!(
        labels,
        [
            "Voltage class",
            "Function",
            "Potential type",
            "Colour",
            "Code",
            "Size",
            "Table"
        ]
    );

    // The title of the colours is a group-header row; the cover prints the standards text of the profile
    // (doc 20 U-35), not its basis note, which stays metadata.
    assert_eq!(set.rows[0].kind, RowKind::GroupHeader);
    assert_eq!(printed(&q, &set, 0)[0], "CABLE COLOUR DEFINITIONS");
    assert_eq!(printed(&q, &set, 0)[6], "colours");
    assert_eq!(set.rows[16].kind, RowKind::GroupFooter);
    assert_eq!(printed(&q, &set, 16)[0], "IEC60757 / EN60204-1");

    // The 15 colour rows in the order of the profile.
    assert_eq!(
        printed(&q, &set, 1),
        ["24 VDC", "Positive", "L+", "Dark Blue", "DBU", "", "colours"]
    );
    assert_eq!(
        printed(&q, &set, 7),
        ["230 VAC", "Earth", "PE", "Yellow / Green", "GNYE", "", "colours"]
    );
    assert_eq!(
        printed(&q, &set, 14),
        [
            "Analogue Signals",
            "Negative",
            "signal \u{2212}",
            "White",
            "WH",
            "",
            "colours"
        ]
    );
    assert!((1..16).all(|i| set.rows[i].kind == RowKind::Data));

    // The sizes in the order of the size profile (doc 20 §7.2: DBU, WH, GNYE; BK, BU, GNYE), each table after
    // its title.
    assert_eq!(printed(&q, &set, 17)[0], "STANDARD SIZES CONTROL");
    assert_eq!(printed(&q, &set, 21)[0], "STANDARD SIZES MAINS");
    for (at, colour, code, group) in [
        (18, "Dark Blue", "DBU", "control"),
        (19, "White", "WH", "control"),
        (20, "Yellow / Green", "GNYE", "control"),
        (22, "Black", "BK", "mains"),
        (23, "Blue", "BU", "mains"),
        (24, "Yellow / Green", "GNYE", "mains"),
    ] {
        let size = if group == "control" {
            "0.75 mm\u{b2}"
        } else {
            "1.5 mm\u{b2}"
        };
        assert_eq!(
            printed(&q, &set, at),
            ["", "", "", colour, code, size, group],
            "size row {at}"
        );
    }
    // The note of the cover (doc 08 §4.2, doc 20 §7.2) closes the rows.
    let last = set.rows.len() - 1;
    assert_eq!(set.rows[last].kind, RowKind::GroupFooter);
    assert_eq!(
        printed(&q, &set, last)[0],
        "If size or sign is not indicated in diagram, standard size or sign is to be used."
    );
    assert_eq!(printed(&q, &set, last)[6], "note");
    // Row IDs are the group and the index inside it.
    let ids: std::collections::BTreeSet<String> = set.rows.iter().map(|r| r.id.to_string()).collect();
    assert_eq!(ids.len(), set.rows.len());
}

/// Editing the profile edits the cover table (AC-12); a project without profiles has no cover rows.
#[test]
fn the_cover_tables_follow_an_edited_profile_and_vanish_without_one() {
    let batch = common::golden_batch();
    let cover = block("cover-tables");
    let rows = |edit: &dyn Fn(&mut ew_derive::ew_schema::Project)| {
        let (mut project, records) = common::split(&batch);
        edit(&mut project);
        let s = common::session_of(project, records, batch.lib.clone());
        let q = s.query();
        let set = q.get::<BlockRows>(cover);
        (0..set.rows.len())
            .map(|i| printed(&q, &set, i))
            .collect::<Vec<_>>()
    };
    let base = rows(&|_| {});
    let edited = rows(&|p| {
        p.settings
            .extra
            .get_mut("colour_profile")
            .expect("colour profile")["rows"][0]["colour"] = serde_json::json!(["RD"]);
    });
    // Only the code of the first colour row changes.
    let changed: Vec<usize> = (0..base.len()).filter(|&i| base[i] != edited[i]).collect();
    assert_eq!(changed, [1]);
    assert_eq!(edited[1][4], "RD");
    let none = rows(&|p| {
        p.settings.extra.remove("colour_profile");
        p.settings.extra.remove("size_profile");
    });
    assert!(none.is_empty());
}

#[test]
fn embedded_blocks_have_no_page_and_derive_part_data() {
    let s = common::golden_session();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    for key in ["cover-tables", "part-data-201", "part-data-202", "part-data-902"] {
        assert!(pages_of(&order, block(key)).is_empty(), "{key} has no page");
    }

    // The cover's colour and size tables derive from the profiles (FT-159); see `the_cover_tables_...`.
    assert!(q.get::<BlockRows>(block("cover-tables")).is_derived());

    // Pages 201 and 202 both show devices whose root ancestor is -201U1 (-202U1 is its child), so both boxes
    // list -201U1 (doc 09 §3.4 repeats the box on 202). -201U1 carries the assembly aspect with order codes 1
    // and 2 (doc 20 §14.2, D-36), so the box lists them: "Order Code 1" 51E-F13GCQP-D+H2GG and "Order Code 2"
    // 32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS (doc 11 §5.5 step 6).
    let r201 = q.get::<BlockRows>(block("part-data-201"));
    let r202 = q.get::<BlockRows>(block("part-data-202"));
    assert!(r201.is_derived());
    assert_eq!(
        part_numbers(&r201),
        ["51E-F13GCQP-D+H2GG", "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS"]
    );
    assert_eq!(part_numbers(&r201), part_numbers(&r202));
    let labels: Vec<Cell> = r201.rows.iter().map(|r| r.cells[0].clone()).collect();
    assert_eq!(
        labels,
        ["Order Code 1", "Order Code 2"].map(|l| Cell::Text(ew_derive::ew_mltext::MlText::neutral(l)))
    );

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
