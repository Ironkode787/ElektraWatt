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

// ---------------------------------------------------------------------------------------------------------
// The cover frame (doc 20 §17.3, D-30)

const FRAME_COVER: &str = include_str!("../../../content/forms/ew.frame.qet-a4-landscape-cover.json");
const FRAME_COVER_KEY: &str = "ew.frame.qet-a4-landscape-cover";

/// The sheet offset and scale of the QET frames: a printed coordinate is `offset + page × scale`.
const OFFSET_NM: i64 = 9_790_000;
const SCALE_PPB: i128 = 986_282_578;

/// A printed x or y (nm) as a page coordinate, rounded to the nanometre.
fn page_nm(printed: i64) -> i64 {
    let n = i128::from(printed - OFFSET_NM) * 1_000_000_000;
    i64::try_from((2 * n + SCALE_PPB) / (2 * SCALE_PPB)).unwrap()
}

/// A printed length (a line width, a shadow) as a page length.
fn page_len(printed: i64) -> i64 {
    let n = i128::from(printed) * 1_000_000_000;
    i64::try_from((2 * n + SCALE_PPB) / (2 * SCALE_PPB)).unwrap()
}

/// Printed millimetres as nanometres.
fn mm(v: f64) -> i64 {
    (v * 1_000_000.0).round() as i64
}

fn pt(x: f64, y: f64) -> Value {
    json!([page_nm(mm(x)), page_nm(mm(y))])
}

fn pen(width_mm: f64) -> Value {
    json!({"pen": {"width": page_len(mm(width_mm))}})
}

/// The geometry of doc 20 §17.3 in printed millimetres: the rules, the project data box and its separators.
const RULE_X: (f64, f64) = (11.09, 279.88);
const BOX: (f64, f64, f64, f64) = (13.70, 47.63, 118.09, 175.50);
const SEPARATORS: [f64; 3] = [84.16, 120.70, 154.62];

fn cover_graphics() -> Vec<Value> {
    let rule =
        |y: f64, w: f64| json!({"line": {"from": pt(RULE_X.0, y), "to": pt(RULE_X.1, y), "stroke": pen(w)}});
    let mut g = vec![
        rule(13.70, 1.305),
        rule(45.02, 0.522),
        json!({"rect": {
            "rect": [pt(BOX.0, BOX.1), pt(BOX.2, BOX.3)],
            "shadow": page_len(mm(2.61)),
            "stroke": pen(0.418),
        }}),
    ];
    for y in SEPARATORS {
        g.push(json!({"line": {"from": pt(BOX.0, y), "to": pt(BOX.2, y), "stroke": pen(0.261)}}));
    }
    g
}

/// The label of a cover row and the placeholder it prints; `None`: no placeholder path exists yet.
type CoverRow = (&'static str, Option<&'static str>);

/// The rows of the project data box by group (the separators of §17.3 bound them). The reference prints 15
/// properties; three have no placeholder path yet (the manufacturing year, the control current and the
/// consumption: `title_block` resolves none of them, `Quantity` has no text form), so their rows keep their
/// slot and print nothing.
const COVER_GROUPS: [&[CoverRow]; 4] = [
    &[
        ("Company / Customer", Some("{party.customer.name}")),
        ("Project Description", Some("{project.long_description}")),
        ("Drawing Identification", Some("{project.drawing_id}")),
        ("Project Reference", Some("{project.reference}")),
    ],
    &[
        ("End User", Some("{party.end_user.name}")),
        ("Address", Some("{party.end_user.address.street}")),
        (
            "Postcode / City",
            Some("{party.end_user.address.postcode} {party.end_user.address.city}"),
        ),
        ("Country", Some("{party.end_user.address.country}")),
    ],
    &[
        ("Manufacturing Year", None),
        ("Supply", Some("{project.supply}")),
        ("Control Current", None),
        ("Consumption", None),
    ],
    &[
        ("Created On", Some("{project.created | date}")),
        ("Edited On", Some("{project.edited | date}")),
        ("Engineer", Some("{party.engineer.name}")),
    ],
];

/// Label column and value column of the box, printed x.
const LABEL_X: (f64, f64) = (14.70, 55.00);
const VALUE_X: (f64, f64) = (55.00, 117.10);

fn cover_cell(
    x: (f64, f64),
    y: (f64, f64),
    label: Option<&str>,
    value: Option<&str>,
    style: &str,
    align: &str,
) -> Value {
    let mut cell = json!({"rect": [pt(x.0, y.0), pt(x.1, y.1)]});
    if let Some(l) = label {
        cell["label"] = json!({"en-GB": l});
    }
    if let Some(v) = value {
        cell["value"] = json!(v);
    }
    cell["style"] = json!(style);
    cell["align"] = json!(align);
    cell["overflow"] = json!({"shrink_to_fit": {"min_cap_height": 1_200_000}});
    cell
}

fn cover_cells() -> Vec<Value> {
    let mut cells = vec![
        cover_cell(
            RULE_X,
            (14.50, 32.00),
            None,
            Some("{project.object}"),
            "cover.title",
            "centre",
        ),
        cover_cell(
            RULE_X,
            (32.00, 44.50),
            None,
            Some("{project.document_title}"),
            "cover.subtitle",
            "centre",
        ),
    ];
    let mut tops = vec![BOX.1];
    tops.extend(SEPARATORS);
    let bottoms = [SEPARATORS[0], SEPARATORS[1], SEPARATORS[2], BOX.3];
    for ((group, top), bottom) in COVER_GROUPS.iter().zip(tops).zip(bottoms) {
        // Every group is cut in one slot per listed row.
        let slot = (bottom - top) / group.len() as f64;
        for (i, (label, value)) in group.iter().enumerate() {
            let Some(value) = value else { continue };
            let y = (top + slot * i as f64, top + slot * (i + 1) as f64);
            cells.push(cover_cell(
                LABEL_X,
                y,
                Some(label),
                None,
                "data.small",
                "middle_left",
            ));
            cells.push(cover_cell(
                VALUE_X,
                y,
                None,
                Some(value),
                "text.normal",
                "middle_left",
            ));
        }
    }
    cells
}

/// The cover frame: the QET A4 landscape frame without grid, plus the cover's rules, project data box, title,
/// subtitle and the cells bound to the project and its parties.
fn cover_text() -> String {
    let mut doc = serde_json::to_value(LibDocument::Form(form(FRAME_NOGRID))).unwrap();
    let (item, id) = identity(FRAME_COVER_KEY);
    doc["id"] = json!(id.to_string());
    doc["item_id"] = json!(item.to_string());
    doc["graphics"] = Value::Array(cover_graphics());
    doc["cells"] = Value::Array(cover_cells());
    doc["meta"] = json!({
        "key": FRAME_COVER_KEY,
        "licence": "CC-BY-SA-4.0",
        "sources": [
            "docs/design/20-golden-project.md §17.3",
            "docs/research/reference-drawing.md §1.1"
        ],
    });
    let mut copy = form(&doc.to_string());
    copy.sha256 = Sha256(sha2::Sha256::digest(serde_json::to_string(&copy.item).unwrap().as_bytes()).into());
    serde_json::to_string_pretty(&LibDocument::Form(copy)).unwrap() + "\n"
}

/// The committed cover frame is generated by `cover_text` (`EW_BLESS=1` rewrites it).
#[test]
fn the_cover_frame_is_the_generated_one() {
    let text = cover_text();
    if std::env::var_os("EW_BLESS").is_some_and(|v| v == "1") {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../content/forms/ew.frame.qet-a4-landscape-cover.json");
        std::fs::write(path, text.as_bytes()).unwrap();
        return;
    }
    assert!(
        FRAME_COVER == text,
        "content/forms/ew.frame.qet-a4-landscape-cover.json differs from the generated frame (EW_BLESS=1 rewrites it)"
    );
}

#[test]
fn the_cover_frame_parses_strictly_and_has_its_identity_and_hash() {
    let doc = LibDocument::from_json(FRAME_COVER).unwrap();
    let (loaded, malformed) = LibDocument::load(parse_value(FRAME_COVER).unwrap()).unwrap();
    assert!(malformed.is_empty(), "{malformed:?}");
    assert_eq!(loaded, doc);
    doc.validate().unwrap();
    assert_eq!(
        FRAME_COVER,
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
        "not canonical"
    );
    let copy = form(FRAME_COVER);
    let (item, id) = identity(FRAME_COVER_KEY);
    assert_eq!(
        (copy.library_id.as_uuid(), copy.item_id.as_uuid(), copy.id),
        (BUILTIN_LIBRARY, item, id)
    );
    let digest = sha2::Sha256::digest(serde_json::to_string(&copy.item).unwrap().as_bytes());
    assert_eq!(copy.sha256, Sha256(digest.into()), "stale sha256");
    assert_eq!(to_value!(copy.item.meta.key), json!(FRAME_COVER_KEY));
    assert_eq!(copy.item.meta.licence.as_deref(), Some("CC-BY-SA-4.0"));
}

/// The cover frame is the no-grid frame with graphics and cells: the same kind, sheet, transform, grid and title
/// block.
#[test]
fn the_cover_frame_is_the_nogrid_frame_plus_the_cover() {
    let (cover, nogrid) = (form(FRAME_COVER).item, form(FRAME_NOGRID).item);
    assert_eq!(cover.kind, FormKind::Frame);
    assert_eq!(cover.sheet, nogrid.sheet);
    assert_eq!(cover.page_to_sheet, nogrid.page_to_sheet);
    assert_eq!(cover.grid, nogrid.grid);
    assert_eq!(cover.title_block, nogrid.title_block);
    assert_eq!(cover.extra, nogrid.extra);
    // Two rules, the box and its three separators.
    assert_eq!(cover.graphics.len(), 6);
    assert!(matches!(cover.graphics[2], Primitive::Rect(_)));
    // The title, the subtitle and a label and a value cell per bound row (the box prints 12 rows).
    let rows = COVER_GROUPS
        .iter()
        .flat_map(|g| g.iter())
        .filter(|r| r.1.is_some())
        .count();
    assert_eq!(rows, 12);
    assert_eq!(cover.cells.len(), 2 + 2 * rows);
}

/// Every cell lies inside the sheet, uses a known style, has balanced braces and does not overlap another
/// cell; the box cells lie inside the box.
#[test]
fn the_cover_cells_are_inside_the_sheet_and_do_not_overlap() {
    let item = form(FRAME_COVER).item;
    let rects: Vec<_> = item
        .cells
        .iter()
        .map(|c| (c.rect.0.min(), c.rect.0.max()))
        .collect();
    for (n, (cell, (min, max))) in item.cells.iter().zip(&rects).enumerate() {
        assert!(
            STYLE_KEYS.contains(&cell.style.as_str()),
            "cell {n}: {}",
            cell.style
        );
        assert!(min.x >= Nm::ZERO && min.y >= Nm::ZERO, "cell {n}");
        assert!(
            max.x <= item.sheet.width && max.y <= item.sheet.height,
            "cell {n}"
        );
        if let Some(v) = &cell.value {
            assert_eq!(v.matches('{').count(), v.matches('}').count(), "cell {n}: {v}");
        }
    }
    for (i, a) in rects.iter().enumerate() {
        for (j, b) in rects.iter().enumerate().skip(i + 1) {
            let disjoint = a.1.x <= b.0.x || b.1.x <= a.0.x || a.1.y <= b.0.y || b.1.y <= a.0.y;
            assert!(disjoint, "cells {i} and {j} overlap");
        }
    }
    let (x0, y0) = (page_nm(mm(BOX.0)), page_nm(mm(BOX.1)));
    let (x1, y1) = (page_nm(mm(BOX.2)), page_nm(mm(BOX.3)));
    for (n, (min, max)) in rects.iter().enumerate().skip(2) {
        assert!(
            min.x.0 >= x0 && max.x.0 <= x1 && min.y.0 >= y0 && max.y.0 <= y1,
            "cell {n} leaves the box"
        );
    }
}

/// Printed coordinates map back to doc 20 §17.3 through the sheet transform (the rules, the box, the
/// separators).
#[test]
fn the_cover_rules_and_box_land_on_the_printed_figures() {
    let item = form(FRAME_COVER).item;
    let sheet = |nm: Nm| (i128::from(nm.0) * SCALE_PPB / 1_000_000_000) as i64 + OFFSET_NM;
    let near = |a: i64, b: f64| (a - mm(b)).abs() <= 2;
    let Primitive::Line(heavy) = &item.graphics[0] else {
        panic!("not a line")
    };
    assert!(
        near(sheet(heavy.from.y), 13.70)
            && near(sheet(heavy.from.x), 11.09)
            && near(sheet(heavy.to.x), 279.88)
    );
    let Primitive::Line(light) = &item.graphics[1] else {
        panic!("not a line")
    };
    assert!(near(sheet(light.from.y), 45.02));
    let Primitive::Rect(b) = &item.graphics[2] else {
        panic!("not a rect")
    };
    let (min, max) = (b.rect.0.min(), b.rect.0.max());
    assert!(near(sheet(min.x), 13.70) && near(sheet(min.y), 47.63));
    assert!(near(sheet(max.x), 118.09) && near(sheet(max.y), 175.50));
    for (prim, y) in item.graphics[3..].iter().zip(SEPARATORS) {
        let Primitive::Line(l) = prim else {
            panic!("not a line")
        };
        assert!(near(sheet(l.from.y), y), "separator at {y}");
    }
}
