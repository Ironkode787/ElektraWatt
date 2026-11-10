// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The embedded blocks of a drawn page (FR-REP-028; doc 20 §17.3): the cover tables and the boxed tables of every
//! other kind, drawn into the `tables` layer inside the block's rectangle, with a grid and the
//! texts in reading order.

use std::sync::Arc;

use ew_dl::{ItemKind, Layer};
use ew_scene::embedded::{
    CoverTables, EmbeddedContent, EmbeddedTable, EmbeddedView, RegionTable, boxed_capacity,
};
use ew_scene::forms::{PageBindings, SheetMap};
use ew_scene::page::{FrameView, PageFinding, PageView, page_display_list};
use ew_scene::table::{TableColumn, TableFinding, TableRow, TableRowKind};
use ew_schema::{FormDef, LibCopy, LibDocument};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2, Rect};

use crate::common::blank_page;

const FRAME: &str = include_str!("../../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json");
const TITLE_BLOCK: &str = include_str!("../../../../content/forms/ew.titleblock.qet-a4.json");

fn form(text: &str) -> Arc<FormDef> {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Form(LibCopy { item, .. }) => Arc::new(item),
        _ => panic!("not a form"),
    }
}

fn table(title: &str, header: &[&str], rows: &[&[&str]], caption: Option<&str>) -> RegionTable {
    RegionTable {
        title: title.to_owned(),
        header: header.iter().map(|s| (*s).to_owned()).collect(),
        rows: rows
            .iter()
            .map(|r| r.iter().map(|s| (*s).to_owned()).collect())
            .collect(),
        caption: caption.map(str::to_owned),
    }
}

/// The block of the golden cover (native page coordinates: 455, 145 px to 1015, 625 px).
fn rect() -> Rect {
    Rect::from_points(
        Pt2::new(Nm(120_381_321), Nm(38_366_287)),
        Pt2::new(Nm(268_553_867), Nm(165_368_429)),
    )
}

fn cover() -> EmbeddedView {
    EmbeddedView {
        rect: rect(),
        content: EmbeddedContent::Cover(Box::new(CoverTables {
            colours: table(
                "CABLE COLOUR DEFINITIONS",
                &["Voltage class", "Function", "Potential type", "Colour"],
                &[
                    &["24 VDC", "Positive", "L+", "Dark Blue"],
                    &["24 VDC", "0V", "M", "White"],
                ],
                Some("IEC60757 / EN60204-1"),
            ),
            control: table(
                "STANDARD SIZES CONTROL",
                &[],
                &[&["Dark Blue", "0.75 mm²"], &["White", "0.75 mm²"]],
                None,
            ),
            mains: table("STANDARD SIZES MAINS", &[], &[&["Black", "1.5 mm²"]], None),
            note: Some("If size or sign is not indicated in diagram.".to_owned()),
        })),
    }
}

fn view(embedded: Vec<EmbeddedView>) -> PageView {
    PageView {
        frame: Some(FrameView {
            form: form(FRAME),
            title_block: Some(form(TITLE_BLOCK)),
            bindings: PageBindings::new(),
        }),
        embedded,
        ..blank_page(0x100)
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
fn a_page_without_embedded_blocks_has_an_empty_tables_layer() {
    let l = page_display_list(&view(Vec::new()), Layer::Tables, bundled(), builtin());
    assert!(l.list.is_empty() && l.findings.is_empty());
}

#[test]
fn the_cover_tables_print_title_header_rows_and_caption_in_reading_order() {
    let l = page_display_list(&view(vec![cover()]), Layer::Tables, bundled(), builtin());
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    assert_eq!(
        glyph_texts(&l.list),
        [
            "CABLE COLOUR DEFINITIONS",
            "Voltage class",
            "Function",
            "Potential type",
            "Colour",
            "24 VDC",
            "Positive",
            "L+",
            "Dark Blue",
            "24 VDC",
            "0V",
            "M",
            "White",
            "IEC60757 / EN60204-1",
            "STANDARD SIZES CONTROL",
            "Dark Blue",
            "0.75 mm²",
            "White",
            "0.75 mm²",
            "STANDARD SIZES MAINS",
            "Black",
            "1.5 mm²",
            "If size or sign is not indicated in diagram.",
        ]
    );
    // The grid: per table the outline, the row separators and the column separators, each line once.
    let strokes = |l: &ew_dl::DisplayList| {
        l.items
            .iter()
            .filter(|i| matches!(&i.kind, ItemKind::Path(p) if p.stroke.is_some()))
            .count()
    };
    // Colours: 3 rows (header + 2) = outline + 2 row separators + 3 column separators; control: 2 rows =
    // outline + 1 + 1; mains: 1 row = outline + 0 + 1.
    assert_eq!(strokes(&l.list), (1 + 2 + 3) + (1 + 1 + 1) + (1 + 1));
}

#[test]
fn everything_lies_inside_the_blocks_rectangle_and_the_note_row_below_it() {
    let l = page_display_list(&view(vec![cover()]), Layer::Tables, bundled(), builtin());
    let r = rect();
    // The note sits in the row below the rectangle: a 24th of its height.
    let below = Nm(r.height().0 / 24);
    let b = l.list.bounds().expect("the tables have items");
    assert!(
        b.min().x >= r.min().x
            && b.max().x <= r.max().x
            && b.min().y >= r.min().y
            && b.max().y <= Nm(r.max().y.0 + below.0),
        "{b:?} outside {r:?}"
    );
}

#[test]
fn the_tables_layer_does_not_depend_on_the_order_of_the_embedded_blocks() {
    let mut other = cover();
    other.rect = Rect::from_points(
        Pt2::new(Nm(10_000_000), Nm(10_000_000)),
        Pt2::new(Nm(100_000_000), Nm(100_000_000)),
    );
    let a = page_display_list(
        &view(vec![cover(), other.clone()]),
        Layer::Tables,
        bundled(),
        builtin(),
    );
    let b = page_display_list(&view(vec![other, cover()]), Layer::Tables, bundled(), builtin());
    assert_eq!(a, b);
}

#[test]
fn a_text_that_does_not_fit_its_cell_shrinks_and_is_reported_not_clipped() {
    let mut v = cover();
    let EmbeddedContent::Cover(c) = &mut v.content else {
        unreachable!("the cover")
    };
    c.mains.rows[0][0] =
        "A colour name far too long for the cell it sits in, really much too long".to_owned();
    let l = page_display_list(&view(vec![v]), Layer::Tables, bundled(), builtin());
    assert_eq!(l.findings.len(), 1, "{:?}", l.findings);
    assert!(
        glyph_texts(&l.list)
            .iter()
            .any(|t| t.starts_with("A colour name far too long"))
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The boxed table of every other kind

/// The frame form's page-to-sheet map (doc 20 §17.1).
const SHEET: SheetMap = SheetMap {
    scale_ppb: 986_282_578,
    offset: Pt2 {
        x: Nm(9_790_000),
        y: Nm(9_790_000),
    },
};

/// A page rectangle from printed hundredths of a millimetre.
fn printed_rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
    let p = |x: i64, y: i64| SHEET.unapply(Pt2::new(Nm(x * 10_000), Nm(y * 10_000)));
    Rect::from_points(p(x0, y0), p(x1, y1))
}

fn data_row(cells: &[&str]) -> TableRow {
    TableRow {
        kind: TableRowKind::Data,
        cells: cells.iter().map(|s| (*s).to_owned()).collect(),
        links: Vec::new(),
    }
}

/// D17's legend: 108.00–278.00 × 19.50–167.50 mm printed, title "LEGEND", two flow columns with headers.
fn legend(rows: usize) -> EmbeddedView {
    let columns = [("Pos", 10), ("DT", 22), ("Qty", 16), ("Part number", 35)]
        .map(|(h, w)| TableColumn {
            header: h.to_owned(),
            weight: w,
        })
        .to_vec();
    EmbeddedView {
        rect: printed_rect(10_800, 1_950, 27_800, 16_750),
        content: EmbeddedContent::Table(EmbeddedTable {
            title: "LEGEND".to_owned(),
            columns,
            headers: true,
            flow_columns: 2,
            rows: (1..=rows)
                .map(|n| data_row(&[&n.to_string(), "-X1031", "1", "3044102"]))
                .collect(),
            note: None,
        }),
    }
}

/// The printed origin of the first glyph run whose text is `text`.
fn printed_origin(list: &ew_dl::DisplayList, text: &str) -> Pt2 {
    list.items
        .iter()
        .find_map(|i| match &i.kind {
            ItemKind::Glyphs(run) if run.text == text => Some(SHEET.apply(run.origin)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text {text:?}"))
}

/// Whether `a` is `hundredths` of a millimetre to within 0.02 mm.
fn near(a: Nm, hundredths: i64) -> bool {
    (a.0 - hundredths * 10_000).abs() <= 20_000
}

#[test]
fn the_legend_draws_twenty_rows_per_flow_column_each_under_its_header() {
    let l = page_display_list(&view(vec![legend(30)]), Layer::Tables, bundled(), builtin());
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    let header = ["Pos", "DT", "Qty", "Part number"];
    let row = |n: usize| [n.to_string(), "-X1031".into(), "1".into(), "3044102".into()];
    let mut want: Vec<String> = vec!["LEGEND".to_owned()];
    for flow in [1..=20, 21..=30] {
        want.extend(header.map(str::to_owned));
        for n in flow {
            want.extend(row(n));
        }
    }
    assert_eq!(glyph_texts(&l.list), want);
    // The second flow column starts 83 mm + the 4 mm gutter to the right, on the first row's line; rows are 6.52 mm.
    let (r1, r2, r21) = (
        printed_origin(&l.list, "1"),
        printed_origin(&l.list, "2"),
        printed_origin(&l.list, "21"),
    );
    assert!(near(r21.x - r1.x, 8_700), "{r1:?} {r21:?}");
    assert_eq!(r21.y, r1.y);
    assert!(near(r2.y - r1.y, 652), "{r1:?} {r2:?}");
    // Fills without a stroke: the shadow and the header row of each flow column.
    let fills = l
        .list
        .items
        .iter()
        .filter(|i| matches!(&i.kind, ItemKind::Path(p) if p.fill.is_some() && p.stroke.is_none()))
        .count();
    assert_eq!(fills, 1 + 2);
}

#[test]
fn rows_beyond_the_capacity_are_not_drawn_and_reported() {
    let v = legend(45);
    let EmbeddedContent::Table(t) = &v.content else {
        unreachable!("a table")
    };
    assert_eq!(boxed_capacity(v.rect, t, SHEET), (2, 20));
    let l = page_display_list(&view(vec![v.clone()]), Layer::Tables, bundled(), builtin());
    assert_eq!(
        l.findings,
        [PageFinding::Table(TableFinding::TooManyRows {
            given: 45,
            drawn: 40
        })]
    );
    let texts = glyph_texts(&l.list);
    assert!(texts.contains(&"40".to_owned()) && !texts.contains(&"41".to_owned()));
    // Lowering twice gives the same items.
    assert_eq!(
        l,
        page_display_list(&view(vec![v]), Layer::Tables, bundled(), builtin())
    );
}

#[test]
fn a_table_without_headers_draws_no_inner_grid_and_its_texts_inside_the_shadowed_box() {
    // 201's PART DATA box: 16.31–89.38 × 16.31–52.85 mm printed, label and part number at 3 : 10 (the ruling on
    // lane A42's LB-9 stop, cycle 42: "PART DATA's label : part number weights are 3 : 10").
    let v = EmbeddedView {
        rect: printed_rect(1_631, 1_631, 8_938, 5_285),
        content: EmbeddedContent::Table(EmbeddedTable {
            title: "PART DATA".to_owned(),
            columns: vec![
                TableColumn {
                    header: "Label".to_owned(),
                    weight: 3,
                },
                TableColumn {
                    header: "Part number".to_owned(),
                    weight: 10,
                },
            ],
            headers: false,
            flow_columns: 1,
            rows: vec![
                data_row(&["Order Code 1", "51E-F13GCQP-D+H2GG"]),
                data_row(&["Order Code 2", "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS"]),
            ],
            note: None,
        }),
    };
    let l = page_display_list(&view(vec![v]), Layer::Tables, bundled(), builtin());
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    assert_eq!(
        glyph_texts(&l.list),
        [
            "PART DATA",
            "Order Code 1",
            "51E-F13GCQP-D+H2GG",
            "Order Code 2",
            "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS"
        ]
    );
    let strokes: Vec<&ew_dl::PathItem> = l
        .list
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Path(p) if p.stroke.is_some() => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(strokes.len(), 1, "the box only");
    // The box at 0.418 mm printed; everything stays within the box and its 2.61 mm shadow.
    let width = SHEET.scale_len(strokes[0].stroke.as_ref().expect("a stroke").width);
    assert!((width.0 - 418_000).abs() <= 1, "{width:?}");
    let b = SHEET.apply_rect(l.list.bounds().expect("items"));
    assert!(
        near(b.max().x, 8_938 + 261) && near(b.max().y, 5_285 + 261),
        "{b:?}"
    );
    assert!(b.min().x.0 >= 16_300_000 && b.min().y.0 >= 16_300_000, "{b:?}");
    // The title band is 1.5 rows (9.78 mm): the first row's text lies below it.
    assert!(printed_origin(&l.list, "Order Code 1").y.0 > 16_310_000 + 9_780_000);
}

#[test]
fn a_block_whose_rows_do_not_derive_draws_its_reason_without_a_header() {
    let reason = "Not derived: `unknown source` is not supported in this build";
    let v = EmbeddedView {
        rect: printed_rect(1_631, 1_631, 18_938, 5_285),
        content: EmbeddedContent::Table(EmbeddedTable {
            title: String::new(),
            columns: Vec::new(),
            headers: true,
            flow_columns: 1,
            rows: Vec::new(),
            note: Some(reason.to_owned()),
        }),
    };
    let l = page_display_list(&view(vec![v]), Layer::Tables, bundled(), builtin());
    assert!(l.findings.is_empty(), "{:?}", l.findings);
    assert_eq!(glyph_texts(&l.list), [reason]);
    let fills = l
        .list
        .items
        .iter()
        .filter(|i| matches!(&i.kind, ItemKind::Path(p) if p.fill.is_some()))
        .count();
    assert_eq!(fills, 1, "the shadow only: no header row without columns");
}
