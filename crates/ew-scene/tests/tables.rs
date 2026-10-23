// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `tables` layer of a generated report page (FT-53; doc 20 §16): the house table drawn from a
//! [`TableView`], its findings and its place on the sheet.

use std::sync::Arc;

use ew_dl::{Brush, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::forms::PageBindings;
use ew_scene::page::{FrameView, PageFinding, PageKind, PageView, page_display_list, page_scene};
use ew_scene::table::{HEADER_FILL, TableColumn, TableFinding, TableRow, TableRowKind, TableView};
use ew_schema::{FormDef, LibCopy, LibDocument};
use ew_text::bundled;
use ew_units::Nm;

const FRAME: &str = include_str!("../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json");
const TITLE_BLOCK: &str = include_str!("../../../content/forms/ew.titleblock.qet-a4.json");

fn form(text: &str) -> Arc<FormDef> {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Form(LibCopy { item, .. }) => Arc::new(item),
        _ => panic!("not a form"),
    }
}

/// A generated page with the no-grid frame and the given table.
fn view(table: Option<TableView>) -> PageView {
    PageView {
        page: Id::from_uuid(Uuid::from_u128(0x100)),
        kind: PageKind::Schematic,
        grid: ew_units::Grid::MM4,
        language: None,
        frame: Some(FrameView {
            form: form(FRAME),
            title_block: Some(form(TITLE_BLOCK)),
            bindings: PageBindings::new(),
        }),
        reps: Vec::new(),
        connections: Vec::new(),
        junctions: Vec::new(),
        wire_marker: None,
        table,
        embedded: Vec::new(),
        break_marks: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Default::default(),
    }
}

fn table(rows: &[&[&str]]) -> TableView {
    TableView {
        columns: ["Folio", "Title", "Location"]
            .iter()
            .zip([6_342, 13_517, 6_237])
            .map(|(h, weight)| TableColumn {
                header: (*h).to_owned(),
                weight,
            })
            .collect(),
        rows: rows
            .iter()
            .map(|r| TableRow {
                kind: TableRowKind::Data,
                cells: r.iter().map(|c| (*c).to_owned()).collect(),
                links: Vec::new(),
            })
            .collect(),
        note: None,
    }
}

fn glyph_texts(list: &ew_dl::DisplayList) -> Vec<String> {
    list.items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_layer_is_empty_without_a_table() {
    let l = page_display_list(&view(None), Layer::Tables, bundled());
    assert!(l.list.is_empty() && l.findings.is_empty());
}

#[test]
fn the_house_table_has_a_grey_header_a_grid_and_the_texts_in_row_order() {
    let v = view(Some(table(&[
        &["1", "Title Page", ""],
        &["2", "Table of Contents 1", "+J-E-2416"],
    ])));
    let l = page_display_list(&v, Layer::Tables, bundled());
    // Header texts, then the row texts in row order (an empty cell prints nothing).
    assert_eq!(
        glyph_texts(&l.list),
        [
            "Folio",
            "Title",
            "Location",
            "1",
            "Title Page",
            "2",
            "Table of Contents 1",
            "+J-E-2416"
        ]
    );
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    // The header fill comes first, #C0C0C0 and unstroked.
    let ItemKind::Path(fill) = &l.list.items[0].kind else {
        panic!("the header fill comes first")
    };
    assert_eq!(fill.fill, Some(Brush::Solid(HEADER_FILL)));
    assert!(fill.stroke.is_none());
    // The grid: the outline, the 2 separators below the header and the 2 column separators.
    let strokes = l
        .list
        .items
        .iter()
        .filter(|i| matches!(&i.kind, ItemKind::Path(p) if p.stroke.is_some()))
        .count();
    assert_eq!(strokes, 1 + 2 + 2);
}

#[test]
fn the_table_prints_at_the_house_position_on_the_sheet() {
    let v = view(Some(table(&[&["1", "Title Page", ""], &["2", "Contents", ""]])));
    let scene = page_scene(&v, bundled());
    let layer = scene.layer(Layer::Tables).expect("the layer");
    let outline = layer
        .items
        .iter()
        .find_map(|i| match &i.kind {
            ItemKind::Path(p) if p.stroke.is_some() => p.path.bounds(),
            _ => None,
        })
        .expect("the outline");
    // Doc 20 §16: x 16.31 to 277.27 mm, from y 15.01 mm, a header and two rows of 6.52 mm; the frame's
    // page-to-sheet map is inverted, so the printed figures hold to a nanometre or two of rounding.
    let printed = scene.map.apply_rect(outline);
    let close = |a: Nm, nm: i64| (a.0 - nm).abs() <= 3;
    assert!(close(printed.min().x, 16_310_000), "{printed:?}");
    assert!(close(printed.max().x, 277_270_000), "{printed:?}");
    assert!(close(printed.min().y, 15_010_000), "{printed:?}");
    assert!(close(printed.max().y, 15_010_000 + 3 * 6_520_000), "{printed:?}");
}

#[test]
fn text_that_does_not_fit_is_shrunk_and_reported_never_cut() {
    let long = "A description far too long for the narrow location column of the house form, and then some";
    let v = view(Some(table(&[&["1", "Title", long]])));
    let l = page_display_list(&v, Layer::Tables, bundled());
    assert!(
        l.findings.iter().any(|f| matches!(
            f,
            PageFinding::Table(TableFinding::Overflow {
                row: Some(0),
                column: 2,
                ..
            })
        )),
        "{:?}",
        l.findings
    );
    assert!(glyph_texts(&l.list).iter().any(|s| s == long));
}

#[test]
fn rows_beyond_the_page_capacity_are_not_drawn_and_are_reported() {
    let rows: Vec<Vec<String>> = (0..26)
        .map(|i| vec![i.to_string(), String::new(), String::new()])
        .collect();
    let refs: Vec<Vec<&str>> = rows
        .iter()
        .map(|r| r.iter().map(String::as_str).collect())
        .collect();
    let slices: Vec<&[&str]> = refs.iter().map(Vec::as_slice).collect();
    let l = page_display_list(&view(Some(table(&slices))), Layer::Tables, bundled());
    assert!(
        l.findings.iter().any(|f| matches!(
            f,
            PageFinding::Table(TableFinding::TooManyRows { given: 26, drawn: 23 })
        )),
        "{:?}",
        l.findings
    );
    // The header's three texts and the first column of the 23 rows drawn.
    assert_eq!(glyph_texts(&l.list).len(), 3 + 23);
    // 23 rows fit without a finding.
    let l = page_display_list(&view(Some(table(&slices[..23]))), Layer::Tables, bundled());
    assert!(l.findings.is_empty(), "{:?}", l.findings);
}

#[test]
fn a_note_takes_one_row_and_a_heading_spans_the_columns() {
    let mut t = table(&[]);
    t.note = Some("Not derived yet".to_owned());
    let l = page_display_list(&view(Some(t)), Layer::Tables, bundled());
    assert_eq!(
        glyph_texts(&l.list),
        ["Folio", "Title", "Location", "Not derived yet"]
    );

    let mut t = table(&[&["1", "Title", "+J"]]);
    t.rows.insert(
        0,
        TableRow {
            kind: TableRowKind::Heading,
            cells: vec!["Bridges".to_owned()],
            links: Vec::new(),
        },
    );
    let l = page_display_list(&view(Some(t)), Layer::Tables, bundled());
    assert_eq!(
        glyph_texts(&l.list),
        ["Folio", "Title", "Location", "Bridges", "1", "Title", "+J"]
    );
    // No column separator through the heading row: the outline, 2 row separators and the two column
    // separators in two runs (the header and the data row) each.
    let strokes = l
        .list
        .items
        .iter()
        .filter(|i| matches!(&i.kind, ItemKind::Path(p) if p.stroke.is_some()))
        .count();
    assert_eq!(strokes, 1 + 2 + 2 * 2);
}

#[test]
fn a_linked_cell_carries_a_link_source() {
    let mut t = table(&[&["1", "Title", "+J"]]);
    let d = ew_dl::Destination::Cp(Uuid::from_u128(0x77));
    t.rows[0].links = vec![(1, d)];
    let l = page_display_list(&view(Some(t)), Layer::Tables, bundled());
    let linked: Vec<&ew_dl::Item> = l
        .list
        .items
        .iter()
        .filter(|i| i.link == Some(ew_dl::Link::Source(d)))
        .collect();
    assert!(!linked.is_empty());
    assert!(linked.iter().all(|i| matches!(i.kind, ItemKind::Glyphs(_))));
}
