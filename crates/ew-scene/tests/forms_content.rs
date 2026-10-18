// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in QET A4 frames and title block under `content/forms`: strict parse, canonical text, identities,
//! geometry and cell layout.

use std::collections::BTreeSet;

use ew_ids::{Id, Uuid};
use ew_schema::{FormDef, FormKind, LibCopy, LibDocument, Primitive, Sha256, parse_value};
use ew_units::{Grid, Nm};
use serde_json::{Value, json};
use sha2::Digest;

const TITLE_BLOCK: &str = include_str!("../../../content/forms/ew.titleblock.qet-a4.json");
const FRAME_GRID: &str = include_str!("../../../content/forms/ew.frame.qet-a4-landscape-grid.json");
const FRAME_NOGRID: &str = include_str!("../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json");

const TITLE_BLOCK_KEY: &str = "ew.titleblock.qet-a4";
const FRAME_GRID_KEY: &str = "ew.frame.qet-a4-landscape-grid";
const FRAME_NOGRID_KEY: &str = "ew.frame.qet-a4-landscape-nogrid";

/// The provisional built-in library (decision ID-24), as in `ew-import-qet`.
const BUILTIN_LIBRARY: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

/// The style keys the text style table knows.
const STYLE_KEYS: [&str; 14] = [
    "cover.title",
    "section.id",
    "section.subtitle",
    "cover.subtitle",
    "box.title",
    "cover.table",
    "tag.main",
    "text.normal",
    "field.label",
    "data.small",
    "pin",
    "xref.device",
    "cp.small",
    "marker",
];

/// Internal lines of the title block in px: `(x0, y0, x1, y1)`, in file order.
const LINES_PX: [(i64, i64, i64, i64); 11] = [
    (46, 0, 46, 68),
    (624, 0, 624, 68),
    (728, 0, 728, 68),
    (832, 0, 832, 68),
    (936, 0, 936, 34),
    (468, 34, 468, 68),
    (988, 17, 988, 34),
    (728, 17, 1040, 17),
    (46, 34, 1040, 34),
    (46, 51, 624, 51),
    (728, 51, 832, 51),
];

/// Cells of the title block: `(x0, y0, x1, y1, label, value, style, align)`, in file order.
type CellRow = (
    i64,
    i64,
    i64,
    i64,
    Option<&'static str>,
    &'static str,
    &'static str,
    &'static str,
);

#[rustfmt::skip]
const CELLS_PX: [CellRow; 18] = [
    (0, 0, 46, 68, None, "{project.logo}", "text.normal", "centre"),
    (46, 0, 624, 34, None, "{page.title}", "text.normal", "middle_left"),
    (46, 34, 468, 51, Some("Project:"), "{project.reference}", "data.small", "middle_left"),
    (46, 51, 468, 68, Some("Description:"), "{page.description}", "data.small", "middle_left"),
    (468, 34, 624, 51, Some("Drawing:"), "{project.drawing_id}", "data.small", "middle_left"),
    (468, 51, 624, 68, Some("Revision:"), "{revision.current.label}", "data.small", "middle_left"),
    (624, 0, 728, 34, None, "{page.function}", "text.normal", "centre"),
    (624, 34, 728, 68, None, "{page.location}", "text.normal", "centre"),
    (728, 0, 832, 17, Some("End user:"), "{party.end_user.name}", "data.small", "middle_left"),
    (728, 17, 832, 34, Some("Object:"), "{project.object}", "data.small", "middle_left"),
    (728, 34, 832, 51, Some("Checked:"), "{revision.current.checker.name}", "data.small", "middle_left"),
    (728, 51, 832, 68, Some("Author:"), "{revision.current.author.name}", "data.small", "middle_left"),
    (832, 0, 936, 17, Some("Creation:"), "{project.created | date}", "pin", "centre"),
    (832, 17, 936, 34, Some("Saved:"), "{project.edited | date}", "pin", "centre"),
    (936, 0, 1040, 17, Some("Page:"), "{page.folio}", "text.normal", "centre"),
    (936, 17, 988, 34, Some("Prev:"), "{page.prev}", "pin", "centre"),
    (988, 17, 1040, 34, Some("Next:"), "{page.next}", "pin", "centre"),
    (832, 34, 1040, 68, None, "{project.notice}", "xref.device", "centre"),
];

fn px(k: i64) -> Nm {
    Grid::QET_1PX.at(k).unwrap()
}

fn form(text: &str) -> LibCopy<FormDef> {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Form(copy) => copy,
        other => panic!("not a form: {other:?}"),
    }
}

fn identity(key: &str) -> (Uuid, Id<FormDef>) {
    let item = Uuid::new_v5(&BUILTIN_LIBRARY, key.as_bytes());
    (item, ew_ids::v5::lib_copy_id(BUILTIN_LIBRARY, item))
}

macro_rules! to_value {
    ($value:expr) => {
        serde_json::to_value(&$value).unwrap()
    };
}

fn documents() -> [(&'static str, &'static str, &'static str); 3] {
    [
        (TITLE_BLOCK_KEY, TITLE_BLOCK, "title_block"),
        (FRAME_GRID_KEY, FRAME_GRID, "frame"),
        (FRAME_NOGRID_KEY, FRAME_NOGRID, "frame"),
    ]
}

#[test]
fn parse_strictly_validate_and_round_trip_to_canonical_text() {
    for (key, text, _) in documents() {
        let doc = LibDocument::from_json(text).unwrap();
        assert!(matches!(doc, LibDocument::Form(_)), "{key}");
        let (loaded, malformed) = LibDocument::load(parse_value(text).unwrap()).unwrap();
        assert!(malformed.is_empty(), "{key}: {malformed:?}");
        assert_eq!(loaded, doc, "{key}");
        doc.validate().unwrap();
        assert_eq!(
            text,
            serde_json::to_string_pretty(&doc).unwrap() + "\n",
            "{key}: not canonical"
        );
    }
}

#[test]
fn identities_hash_and_meta() {
    for (key, text, kind) in documents() {
        let copy = form(text);
        let (item, id) = identity(key);
        assert_eq!(copy.library_id.as_uuid(), BUILTIN_LIBRARY, "{key}");
        assert_eq!(copy.item_id.as_uuid(), item, "{key}");
        assert_eq!(copy.id, id, "{key}");
        assert_eq!(copy.version, "1.0.0", "{key}");
        let digest = sha2::Sha256::digest(serde_json::to_string(&copy.item).unwrap().as_bytes());
        assert_eq!(copy.sha256, Sha256(digest.into()), "{key}: stale sha256");
        assert_eq!(to_value!(copy.item.kind), json!(kind), "{key}");
        let meta = &copy.item.meta;
        assert_eq!(to_value!(meta.key), json!(key));
        assert_eq!(meta.licence.as_deref(), Some("CC-BY-SA-4.0"), "{key}");
        assert!(!meta.sources.is_empty(), "{key}");
    }
    assert_eq!(
        form(TITLE_BLOCK).item.meta.sources,
        [
            "docs/design/20-golden-project.md §17.2",
            "docs/design/09-reports-forms-output.md §2.7"
        ]
    );
    for text in [FRAME_GRID, FRAME_NOGRID] {
        assert_eq!(
            form(text).item.meta.sources,
            [
                "docs/design/20-golden-project.md §17.1",
                "docs/design/14-interop.md §4.2"
            ]
        );
    }
}

#[test]
fn frames_share_geometry_and_name_the_title_block() {
    let title_block = identity(TITLE_BLOCK_KEY).1;
    for (text, show_grid, estimated) in [(FRAME_GRID, None, None), (FRAME_NOGRID, Some(false), Some(true))] {
        let item = form(text).item;
        assert_eq!(item.kind, FormKind::Frame);
        assert_eq!(item.title_block, Some(title_block));
        assert_eq!(
            (item.sheet.width, item.sheet.height),
            (Nm(297_000_000), Nm(210_000_000))
        );
        let transform = item.page_to_sheet.as_ref().unwrap();
        assert_eq!(transform.scale_ppb, 986_282_578);
        assert_eq!(
            (transform.offset.x, transform.offset.y),
            (Nm(9_790_000), Nm(9_790_000))
        );
        let grid = item.grid.as_ref().unwrap();
        assert_eq!(grid.header_band, Some(Grid::new(15_875_000, 3).unwrap()));
        assert_eq!(grid.columns.count, 17);
        assert_eq!(grid.columns.width, Grid::new(15_875_000, 1).unwrap());
        assert_eq!(grid.columns.width.at(1), Some(px(60)));
        assert_eq!(grid.rows.count, 8);
        assert_eq!(grid.rows.height, Grid::new(63_500_000, 3).unwrap());
        assert_eq!(grid.rows.height.at(3), Some(Nm(63_500_000)));
        assert_eq!(grid.header_band.unwrap().at(3), Some(Nm(15_875_000)));
        assert_eq!(grid.extra.get("ew.frame.line_width"), Some(&json!(264_583)));
        assert_eq!(
            grid.extra.get("ew.frame.show_grid"),
            show_grid.map(Value::from).as_ref()
        );
        assert_eq!(
            item.extra.get("ew.layout.estimated"),
            estimated.map(Value::from).as_ref()
        );
        assert!(item.graphics.is_empty() && item.cells.is_empty() && item.data.is_none());
    }
}

#[test]
fn title_block_sheet_and_lines() {
    let item = form(TITLE_BLOCK).item;
    assert_eq!(item.kind, FormKind::TitleBlock);
    assert_eq!((item.sheet.width, item.sheet.height), (px(1040), px(68)));
    assert!(item.page_to_sheet.is_none() && item.grid.is_none() && item.title_block.is_none());
    assert_eq!(item.graphics.len(), LINES_PX.len());
    for (prim, (x0, y0, x1, y1)) in item.graphics.iter().zip(LINES_PX) {
        let Primitive::Line(line) = prim else {
            panic!("not a line: {prim:?}")
        };
        assert_eq!((line.from.x, line.from.y), (px(x0), px(y0)));
        assert_eq!((line.to.x, line.to.y), (px(x1), px(y1)));
        let stroke = to_value!(line.stroke);
        assert_eq!(stroke, json!({"pen": {"width": 264_583}}));
    }
}

#[test]
fn title_block_cells() {
    let item = form(TITLE_BLOCK).item;
    assert_eq!(item.cells.len(), CELLS_PX.len());
    for (n, (cell, (x0, y0, x1, y1, label, value, style, align))) in
        item.cells.iter().zip(CELLS_PX).enumerate()
    {
        let n = n + 1;
        let (min, max) = (cell.rect.0.min(), cell.rect.0.max());
        assert_eq!(
            (min.x, min.y, max.x, max.y),
            (px(x0), px(y0), px(x1), px(y1)),
            "cell {n}"
        );
        let expect_label = label.map_or(Value::Null, |l| json!({"en-GB": l}));
        assert_eq!(to_value!(cell.label), expect_label, "cell {n}");
        assert_eq!(cell.value.as_deref(), Some(value), "cell {n}");
        assert_eq!(to_value!(cell.style), json!(style), "cell {n}");
        assert_eq!(to_value!(cell.align), json!(align), "cell {n}");
        let overflow = if n == 1 {
            json!("error")
        } else {
            json!({"shrink_to_fit": {"min_cap_height": 1_200_000}})
        };
        assert_eq!(to_value!(cell.overflow), overflow, "cell {n}");
        assert!(cell.visible_if.is_none(), "cell {n}");
        let extra = if n == 18 {
            json!({"ew.cell.max_lines": 4})
        } else {
            json!({})
        };
        assert_eq!(to_value!(cell.extra), extra, "cell {n}");
        assert!(STYLE_KEYS.contains(&style), "cell {n}: style {style}");
        // inside the sheet
        assert!(min.x >= Nm::ZERO && min.y >= Nm::ZERO, "cell {n}");
        assert!(
            max.x <= item.sheet.width && max.y <= item.sheet.height,
            "cell {n}"
        );
    }
}

#[test]
fn title_block_cells_do_not_overlap() {
    let item = form(TITLE_BLOCK).item;
    let rects: Vec<_> = item
        .cells
        .iter()
        .map(|c| (c.rect.0.min(), c.rect.0.max()))
        .collect();
    for (i, a) in rects.iter().enumerate() {
        for (j, b) in rects.iter().enumerate().skip(i + 1) {
            let disjoint = a.1.x <= b.0.x || b.1.x <= a.0.x || a.1.y <= b.0.y || b.1.y <= a.0.y;
            assert!(disjoint, "cells {} and {} overlap", i + 1, j + 1);
        }
    }
}

/// Unit segments (one px long) along a horizontal or vertical segment given in px.
fn unit_segments(set: &mut BTreeSet<(bool, i64, i64)>, (x0, y0, x1, y1): (i64, i64, i64, i64)) {
    if y0 == y1 {
        // horizontal: (true, y, x) is the segment from x to x + 1 at height y
        (x0..x1).for_each(|x| {
            set.insert((true, y0, x));
        });
    } else {
        assert_eq!(x0, x1, "not axis-aligned");
        (y0..y1).for_each(|y| {
            set.insert((false, x0, y));
        });
    }
}

#[test]
fn cell_boundaries_are_exactly_the_lines_and_the_outline() {
    let item = form(TITLE_BLOCK).item;
    let to_px: std::collections::BTreeMap<Nm, i64> = (0..=1040).map(|k| (px(k), k)).collect();
    let coord = |nm: Nm| to_px[&nm];

    let mut cell_edges = BTreeSet::new();
    for cell in &item.cells {
        let (min, max) = (cell.rect.0.min(), cell.rect.0.max());
        let (x0, y0, x1, y1) = (coord(min.x), coord(min.y), coord(max.x), coord(max.y));
        for edge in [
            (x0, y0, x1, y0),
            (x0, y1, x1, y1),
            (x0, y0, x0, y1),
            (x1, y0, x1, y1),
        ] {
            unit_segments(&mut cell_edges, edge);
        }
    }

    let mut drawn = BTreeSet::new();
    for line in &LINES_PX {
        unit_segments(&mut drawn, *line);
    }
    for outline in [
        (0, 0, 1040, 0),
        (0, 68, 1040, 68),
        (0, 0, 0, 68),
        (1040, 0, 1040, 68),
    ] {
        unit_segments(&mut drawn, outline);
    }
    assert_eq!(cell_edges, drawn);
}
