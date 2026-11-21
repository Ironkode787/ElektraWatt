// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A list's caption and legend (doc 09 §3.8, FT-1874): the caption prints in its own row above the table's header, the
//! legend in the row under the table's last row, each within its row, and the table takes the frame's list area.

use std::sync::Arc;

use ew_dl::{ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_scene::forms::PageBindings;
use ew_scene::page::{FrameView, PageKind, PageView, page_scene};
use ew_scene::table::{ROW_HEIGHT, TableColumn, TableRow, TableRowKind, TableView};
use ew_schema::{FormDef, LibCopy, LibDocument};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Pt2};

const FRAME: &str = include_str!("../../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json");
const TITLE_BLOCK: &str = include_str!("../../../../content/forms/ew.titleblock.qet-a4.json");

fn form(text: &str) -> Arc<FormDef> {
    match LibDocument::from_json(text).unwrap() {
        LibDocument::Form(LibCopy { item, .. }) => Arc::new(item),
        _ => panic!("not a form"),
    }
}

/// A generated page on the no-grid frame with a table of two rows, its caption and its legend.
fn view() -> PageView {
    let table = TableView {
        columns: ["Terminal", "Side 1"]
            .map(|h| TableColumn {
                header: h.to_owned(),
                weight: 1,
            })
            .to_vec(),
        rows: [["1", "-Q1:1"], ["2", "-Q1:3"]]
            .map(|r| TableRow {
                kind: TableRowKind::Data,
                cells: r.map(str::to_owned).to_vec(),
                links: Vec::new(),
            })
            .to_vec(),
        note: None,
        caption: Some("Terminal list".to_owned()),
        legend: Some("PE: earth".to_owned()),
    };
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
        table: Some(table),
        embedded: Vec::new(),
        break_marks: Vec::new(),
        valve_glyphs: Vec::new(),
        valve_cells: Vec::new(),
        plug_notes: Vec::new(),
        cp_targets: Vec::new(),
        crossings: Default::default(),
        items: Vec::new(),
        structure_boxes: Vec::new(),
        panel: None,
        strip_tags: Vec::new(),
    }
}

#[test]
fn a_caption_stands_above_the_header_and_a_legend_under_the_last_row() {
    let v = view();
    let scene = page_scene(&v, bundled(), builtin());
    let layer = scene.layer(Layer::Tables).expect("the layer");
    // The table's outline, printed: its header now stands one row lower, under the caption's row.
    let outline = layer
        .items
        .iter()
        .find_map(|i| match &i.kind {
            ItemKind::Path(p) if p.stroke.is_some() => p.path.bounds(),
            _ => None,
        })
        .expect("the outline");
    let table = scene.map.apply_rect(outline);
    let close = |a: Nm, nm: i64| (a.0 - nm).abs() <= 3;
    assert!(close(table.min().y, 15_010_000 + ROW_HEIGHT.0), "{table:?}");
    assert!(close(table.max().y, 15_010_000 + 4 * ROW_HEIGHT.0), "{table:?}");
    // Each line's baseline, printed, within its row: the caption above the table, the legend under it.
    let baseline = |text: &str| {
        layer
            .items
            .iter()
            .find_map(|i| match &i.kind {
                ItemKind::Glyphs(run) if run.text == text => Some(scene.map.apply(run.to_parent(Pt2::ORIGIN)).y),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{text} is drawn"))
    };
    let caption = baseline("Terminal list");
    assert!(table.min().y.0 - ROW_HEIGHT.0 < caption.0 && caption < table.min().y, "{caption:?}");
    let legend = baseline("PE: earth");
    assert!(table.max().y < legend && legend.0 < table.max().y.0 + ROW_HEIGHT.0, "{legend:?}");
}
