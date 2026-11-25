// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A legend where its box is narrow (doc 12 §5.4): its identifiers print whole and its descriptions wrap at
//! their words while the box has the height for them, else they are cut and every row still prints; a legend draws
//! no drop shadow and a light header.

use std::sync::Arc;

use ew_dl::{Brush, ItemKind, Layer};
use ew_scene::embedded::{EmbeddedContent, EmbeddedTable, EmbeddedView, LOOKUP_HEADER_FILL, Lookup};
use ew_scene::forms::{PageBindings, SheetMap};
use ew_scene::page::{FrameView, PageFinding, PageView, page_display_list};
use ew_scene::table::{TableColumn, TableFinding, TableRow, TableRowKind};
use ew_schema::{FormDef, LibCopy, LibDocument};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2, Rect};

use crate::common::blank_page;

const FRAME: &str = include_str!("../../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json");

/// The starter A4 frame's page-to-sheet map.
const SHEET: SheetMap = SheetMap {
    scale_ppb: 986_282_578,
    offset: Pt2 {
        x: Nm(9_790_000),
        y: Nm(9_790_000),
    },
};

/// The rows: position, tag, quantity, part number, description.
const ROWS: [[&str; 5]; 3] = [
    [
        "1",
        "-UC1",
        "1",
        "EW-GEN-ENC-1",
        "Wall enclosure 600 × 800 × 250 mm",
    ],
    [
        "2",
        "-101F1",
        "1",
        "EW-GEN-MCB-2P",
        "Miniature circuit breaker, 2 poles",
    ],
    [
        "3",
        "-102Q1",
        "1",
        "EW-GEN-CON-3P",
        "Contactor with three main contacts and one auxiliary contact",
    ],
];

/// What a legend 60 mm wide and `high` hundredths of a millimetre high prints: its texts, the columns of its cut
/// texts, whether it reports rows it does not draw, and its fills.
fn legend(high: i64) -> (Vec<String>, Vec<usize>, bool, Vec<Brush>) {
    let p = |x: i64, y: i64| SHEET.unapply(Pt2::new(Nm(x * 10_000), Nm(y * 10_000)));
    let columns = [
        ("Pos", 8),
        ("DT", 18),
        ("Qty", 10),
        ("Part number", 24),
        ("Description", 40),
    ]
    .map(|(h, w)| TableColumn {
        header: h.to_owned(),
        weight: w,
    })
    .to_vec();
    let table = EmbeddedTable {
        title: "LEGEND".to_owned(),
        columns,
        headers: true,
        flow_columns: 1,
        rows: ROWS
            .iter()
            .map(|r| TableRow {
                kind: TableRowKind::Data,
                cells: r.iter().map(|s| (*s).to_owned()).collect(),
                links: Vec::new(),
            })
            .collect(),
        note: None,
        lookup: Some(Lookup {
            order: vec![2, 3, 1, 0],
            prose: vec![4],
        }),
    };
    let view = PageView {
        frame: Some(FrameView {
            form: match LibDocument::from_json(FRAME).expect("the frame") {
                LibDocument::Form(LibCopy { item, .. }) => Arc::<FormDef>::new(item),
                _ => panic!("not a form"),
            },
            title_block: None,
            bindings: PageBindings::new(),
        }),
        embedded: vec![EmbeddedView {
            rect: Rect::from_points(p(20_000, 2_000), p(26_000, 2_000 + high)),
            content: EmbeddedContent::Table(table),
        }],
        ..blank_page(0x100)
    };
    let l = page_display_list(&view, Layer::Tables, bundled(), builtin());
    let mut texts = Vec::new();
    let mut fills = Vec::new();
    for item in &l.list.items {
        match &item.kind {
            ItemKind::Glyphs(run) => texts.push(run.text.clone()),
            ItemKind::Path(path) => fills.extend(path.fill),
            _ => {}
        }
    }
    let cut = l
        .findings
        .iter()
        .filter_map(|f| match f {
            PageFinding::Table(TableFinding::Overflow { column, .. }) => Some(*column),
            _ => None,
        })
        .collect();
    let lost = l
        .findings
        .iter()
        .any(|f| matches!(f, PageFinding::Table(TableFinding::TooManyRows { .. })));
    (texts, cut, lost, fills)
}

/// 60 mm wide and high enough, every identifier prints whole and each description prints whole in lines of its words.
#[test]
fn a_narrow_legend_wraps_its_descriptions_and_keeps_its_identifiers_whole() {
    let (texts, cut, lost, fills) = legend(12_000);
    assert!(cut.is_empty() && !lost, "{cut:?} {texts:?}");
    for row in ROWS {
        for whole in &row[..4] {
            assert!(texts.iter().any(|t| t == whole), "{whole}: {texts:?}");
        }
        // The description is not one run but its lines, which read it whole.
        assert!(!texts.iter().any(|t| t == row[4]), "{texts:?}");
        let words: Vec<&str> = row[4].split_whitespace().collect();
        let printed: Vec<&str> = texts
            .iter()
            .filter(|t| row[4].contains(t.trim()))
            .flat_map(|t| t.split_whitespace())
            .collect();
        assert!(
            words.iter().all(|w| printed.contains(w)),
            "{words:?} in {texts:?}"
        );
    }
    // No drop shadow, and a light header.
    assert_eq!(fills, [Brush::Solid(LOOKUP_HEADER_FILL)]);
}

/// 60 mm wide with the height of its rows in one line only, the legend prints every row and every identifier whole, and
/// cuts the descriptions instead of wrapping them.
#[test]
fn a_legend_without_the_height_to_wrap_cuts_its_descriptions_and_keeps_its_rows() {
    // The title band (9.78 mm), the header and three rows of 6.52 mm, and 2 mm to spare.
    let (texts, cut, lost, _) = legend(978 + 4 * 652 + 200);
    assert!(!lost, "{texts:?}");
    assert!(!cut.is_empty() && cut.iter().all(|c| *c == 4), "{cut:?}");
    for row in ROWS {
        for whole in &row[..4] {
            assert!(texts.iter().any(|t| t == whole), "{whole}: {texts:?}");
        }
    }
}
