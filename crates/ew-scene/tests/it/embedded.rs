// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The embedded cover tables of a drawn page (FR-REP-028; doc 20 §17.3): drawn into the `tables` layer inside
//! the block's rectangle, with a grid and the texts in reading order.

use std::sync::Arc;

use ew_dl::{ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::embedded::{CoverTables, EmbeddedContent, EmbeddedView, RegionTable};
use ew_scene::forms::PageBindings;
use ew_scene::page::{FrameView, PageKind, PageView, page_display_list};
use ew_schema::{FormDef, LibCopy, LibDocument};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2, Rect};

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
        content: EmbeddedContent::Cover(CoverTables {
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
        }),
    }
}

fn view(embedded: Vec<EmbeddedView>) -> PageView {
    PageView {
        page: Id::from_uuid(Uuid::from_u128(0x100)),
        kind: PageKind::Schematic,
        grid: ew_units::Grid::QET_10PX,
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
        table: None,
        embedded,
        break_marks: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Default::default(),
        items: Vec::new(),
        structure_boxes: Vec::new(),
        panel: None,
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
    let EmbeddedContent::Cover(c) = &mut v.content;
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
