// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A list's pages print what its block chooses (doc 09 §3.8): FT-1874's columns with their headings and widths, the
//! caption, the legend and the rows the model does not hold, in the project's language; FT-1826's list pages on the
//! set's paper; FT-1805's terminal diagram that names each terminal's strip. Over copies of the sample.

use crate::common;

use ew_api::Session;
use ew_api::output::{PageFrameOf, PageTableOf, RowExport, page_layer, row_export};
use ew_derive::ew_mltext::LangTag;
use ew_derive::{PageOrder, PageRef, text_in};
use ew_dl::{Item, ItemKind, Layer};
use ew_reports::{BlockRows, Registry};
use ew_scene::table::TableView;
use serde_json::{Value, json};

/// The ID of the sample's page-oriented block whose first folio is `folio`.
fn block_at(s: &Session, folio: u32) -> String {
    let q = s.query();
    q.report_blocks()
        .iter()
        .find(|(_, b)| b.first_folio == Some(folio) && b.embedded.is_none())
        .map(|(id, _)| id.as_uuid().to_string())
        .unwrap_or_else(|| panic!("a block at {folio}"))
}

fn set(s: &mut Session, block: &str, path: &str, value: Value) {
    common::run_cmd(
        s,
        "fields.set",
        json!({"kind": "report_block", "targets": [block], "path": path, "value": value}),
    );
}

/// The generated pages of the block at `folio`, in page order.
fn pages_of(s: &Session, folio: u32) -> Vec<PageRef> {
    let block = block_at(s, folio);
    s.query()
        .get::<PageOrder>(())
        .entries
        .iter()
        .filter(|e| matches!(e.page, PageRef::Generated { block: b, .. } if b.as_uuid().to_string() == block))
        .map(|e| e.page)
        .collect()
}

/// The printed table of `page`.
fn table(s: &Session, page: PageRef) -> TableView {
    s.query()
        .get::<PageTableOf>(page)
        .as_ref()
        .clone()
        .expect("a list page")
}

/// The texts of the glyph runs of `page`'s frame layer, groups included.
fn frame_texts(s: &Session, page: PageRef) -> Vec<String> {
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(g) => walk(&g.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&page_layer(&s.query(), page, Layer::Frame).list.items, &mut out);
    out
}

/// FT-1874: a block's own columns print in its order, each with its width and its heading (or the kind's), and its
/// caption and legend print on every page; the CSV keeps every column of the kind.
#[test]
fn a_list_prints_the_columns_headings_widths_caption_and_legend_its_block_chooses() {
    let mut s = common::sample();
    let first = pages_of(&s, 903)[0];
    let before = table(&s, first);
    let at = |key: &str| {
        before
            .columns
            .iter()
            .position(|c| c.header == english("terminal_diagram", key))
    };
    let (side1, terminal) = (at("side1").expect("side 1"), at("terminal").expect("terminal"));
    let block = block_at(&s, 903);
    set(
        &mut s,
        &block,
        "columns",
        json!([
            {"key": "side1", "weight": 30},
            {"key": "terminal", "weight": 10, "heading": "Klem"},
        ]),
    );
    set(&mut s, &block, "caption", json!("Terminals of the sample"));
    set(&mut s, &block, "legend", json!("PE: protective earth"));
    let after = table(&s, first);
    let heads: Vec<(&str, u32)> = after
        .columns
        .iter()
        .map(|c| (c.header.as_str(), c.weight))
        .collect();
    assert_eq!(
        heads,
        [(english("terminal_diagram", "side1").as_str(), 30), ("Klem", 10)]
    );
    assert_eq!(after.caption.as_deref(), Some("Terminals of the sample"));
    assert_eq!(after.legend.as_deref(), Some("PE: protective earth"));
    // The same rows, their cells those of the two columns; the caption and the legend take a row each.
    for (row, was) in after.rows.iter().zip(&before.rows) {
        assert_eq!(row.cells, [was.cells[side1].clone(), was.cells[terminal].clone()]);
    }
    let q = s.query();
    let rows = q.get::<BlockRows>(ew_derive::ew_ids::Id::from_uuid(block.parse().expect("a UUID")));
    let lists = pages_of(&s, 903);
    let listed: usize = lists.iter().map(|p| table(&s, *p).rows.len()).sum();
    assert_eq!(listed, rows.rows.len(), "every row is on a page");
    // The CSV keeps every column of the kind.
    let csv = row_export(&q, &RowExport::Report, Some("903"))
        .expect("903's rows")
        .csv;
    let header = csv.lines().next().expect("a header");
    for c in Registry::builtin()
        .by_name("terminal_diagram")
        .expect("the kind")
        .columns()
    {
        assert!(header.contains(c.english()), "{} in {header}", c.key);
    }
}

/// The English heading of the column `key` of report kind `kind`.
fn english(kind: &str, key: &str) -> String {
    Registry::builtin()
        .by_name(kind)
        .and_then(|k| k.columns().into_iter().find(|c| c.key == key))
        .map(|c| c.english().to_owned())
        .unwrap_or_else(|| panic!("{kind} has {key}"))
}

/// FT-1874: in a project whose language is Dutch a list prints its headings and its title block's labels in Dutch;
/// before, every heading and label printed its English.
#[test]
fn a_dutch_project_prints_its_list_heads_and_title_block_labels_in_dutch() {
    let mut s = common::sample();
    common::run_cmd(
        &mut s,
        "project.languages.set",
        json!({"source": "nl-NL", "date": "dd_mm_yyyy", "decimal_separator": "comma"}),
    );
    let nl: LangTag = "nl-NL".parse().expect("a tag");
    let page = pages_of(&s, 907)[0];
    let heads: Vec<String> = table(&s, page).columns.into_iter().map(|c| c.header).collect();
    let kind = Registry::builtin()
        .by_name("wire_list")
        .expect("the kind")
        .columns();
    let dutch: Vec<String> = kind
        .iter()
        .map(|c| text_in(&c.label, Some(&nl)).to_owned())
        .collect();
    let english: Vec<String> = kind.iter().map(|c| c.english().to_owned()).collect();
    assert_eq!(heads, dutch);
    assert_ne!(heads, english);
    let texts = frame_texts(&s, page);
    assert!(texts.iter().any(|t| t.starts_with("Omschrijving:")), "{texts:?}");
    assert!(!texts.iter().any(|t| t.starts_with("Description:")), "{texts:?}");
}

/// FT-1874: a row of the block's own is listed with the kind's rows, by the block's sorting among them, in the CSV
/// too, and stays when the model changes: the list is made anew from the model and the block.
#[test]
fn rows_of_your_own_are_listed_sorted_and_kept_when_the_model_changes() {
    let mut s = common::sample();
    let block = block_at(&s, 903);
    set(
        &mut s,
        &block,
        "added_rows",
        json!([{"cells": {"terminal": "1a", "label": "spare"}}]),
    );
    let id = ew_derive::ew_ids::Id::from_uuid(block.parse().expect("a UUID"));
    let terminal = |s: &Session| {
        let rows = s.query().get::<BlockRows>(id);
        let at = rows.column("terminal").expect("a terminal column");
        rows.rows
            .iter()
            .position(|r| r.id.to_string().starts_with("n:added"))
            .map(|i| (i, rows.rows.len(), rows.rows[i].cells[at].clone()))
    };
    let (i, n, cell) = terminal(&s).expect("the added row");
    assert_eq!(
        (i, cell),
        (n - 1, ew_reports::Cell::Ident("1a".to_owned())),
        "unsorted: after the kind's rows"
    );
    // Sorted by terminal, it stands among the model's rows ("1a" before "2").
    set(&mut s, &block, "sort", json!([{"key": "terminal"}]));
    let (i, n, _) = terminal(&s).expect("the added row");
    assert!(i < n - 1, "{i} of {n}");
    let csv = row_export(&s.query(), &RowExport::Report, Some("903"))
        .expect("903's rows")
        .csv;
    assert!(
        csv.lines().any(|l| l.contains(",1a,") && l.contains("spare")),
        "{csv}"
    );
    // A change of the model leaves it where it is.
    let page = common::first_page(&s);
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "page", "targets": [page.to_string()], "path": "title", "value": "Supply"}),
    );
    assert_eq!(terminal(&s).map(|(i, _, _)| i), Some(i));
}

/// FT-1805: the terminal diagram names each terminal's strip, so two strips' terminals of one designation read apart.
#[test]
fn two_strips_terminals_of_one_designation_read_apart_in_the_terminal_diagram() {
    let s = common::sample();
    let mut pe = Vec::new();
    for page in pages_of(&s, 903) {
        let t = table(&s, page);
        let col = |key: &str| {
            t.columns
                .iter()
                .position(|c| c.header == english("terminal_diagram", key))
                .unwrap_or_else(|| panic!("a {key} column"))
        };
        let (strip, terminal) = (col("strip"), col("terminal"));
        pe.extend(
            t.rows
                .iter()
                .filter(|r| r.cells.get(terminal).is_some_and(|c| c == "PE"))
                .map(|r| r.cells[strip].clone()),
        );
    }
    assert!(pe.len() > 1, "the sample has more than one terminal PE: {pe:?}");
    assert!(pe.iter().all(|s| !s.is_empty()), "{pe:?}");
    let strips: std::collections::BTreeSet<&String> = pe.iter().collect();
    assert_eq!(
        strips.len(),
        pe.len(),
        "every terminal PE names another strip: {pe:?}"
    );
}

/// FT-1826: a set whose frame is the starter A3 frame prints its lists on it, as many rows to a page as its list area
/// holds, more than the A4 one.
#[test]
fn a_set_on_a3_prints_its_lists_on_a3() {
    let mut s = common::sample();
    let a3 = ew_api::shipped_frames(ew_api::output::A3_GRID_FRAME).expect("the shipped frames");
    common::run_cmd(&mut s, "project.frames.set", a3);
    let q = s.query();
    let page = pages_of(&s, 907)[0];
    let frame = q.get::<PageFrameOf>(page).as_ref().clone().expect("a frame");
    assert_eq!(
        (frame.form.sheet.width.0, frame.form.sheet.height.0),
        (420_000_000, 297_000_000)
    );
    let a3_rows = frame
        .form
        .list_area(frame.title_block.as_deref(), false, false)
        .rows;
    assert!(a3_rows > ew_schema::HOUSE_ROWS_PER_PAGE, "{a3_rows}");
    let id = ew_derive::ew_ids::Id::from_uuid(block_at(&s, 907).parse().expect("a UUID"));
    let all = q.get::<BlockRows>(id).rows.len();
    let first = table(&s, page).rows.len();
    assert_eq!(first, all.min(usize::try_from(a3_rows).expect("rows")));
}
