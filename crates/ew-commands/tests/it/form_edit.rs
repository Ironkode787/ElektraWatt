// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `form.edit` (doc 09 §3.9): the first change of the starter title block makes it and the three starter A4 frames that
//! name it the project's own, and every page, report block and the frame defaults name the forks; a second change is
//! made in place; undo gives the starter copies back.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::commands::CreateReportBlock;
use ew_commands::{AnyCommand, CmdError, Engine, Origin};
use ew_ids::{Id, Uuid, v5};
use ew_model::Model;
use ew_model::fixture::{PAGE_101, SEC1, mini_project_parts};
use ew_schema::{
    ContinuationPolicy, EmptyPolicy, FormDef, LibCopy, LibDocument, Page, Record, ReportBlock, ReportKind,
    ReportSource,
};
use serde_json::{Value, json};

const BLOCK: &str = "496e5258-a93b-5354-84af-c2c185e84d54";
const GRID: &str = "c80af135-34f8-574c-8f52-756bd24c7bd0";
const COVER: &str = "81ee0d6d-6c0e-55e4-b7fd-1896f594051f";
const NOGRID: &str = "ec586fad-385b-599d-a76d-59c893506f00";
const A3_GRID: &str = "7cfb3e56-563f-5160-a961-4be49a033a96";

fn form_id(s: &str) -> Id<FormDef> {
    Id::from_uuid(s.parse().expect("a UUID"))
}

fn starter(file: &str) -> LibDocument {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../content/forms")
        .join(file);
    LibDocument::from_json(&std::fs::read_to_string(path).expect("the starter form")).expect("a form")
}

/// The mini project with the starter forms; page 101 prints on the A4 grid frame.
fn model() -> Model {
    model_where(|_| {})
}

/// [`model`] with each starter form changed by `edit` as it is read.
fn model_where(edit: impl Fn(&mut LibDocument)) -> Model {
    let (p, mut records, _) = mini_project_parts();
    for r in &mut records {
        if let Record::Page(page) = r
            && page.id == PAGE_101
        {
            page.form = Some(form_id(GRID));
        }
    }
    let mut lib = library();
    for f in [
        "ew.titleblock.qet-a4.json",
        "ew.titleblock.qet-a3.json",
        "ew.frame.qet-a4-landscape-grid.json",
        "ew.frame.qet-a4-landscape-cover.json",
        "ew.frame.qet-a4-landscape-nogrid.json",
        "ew.frame.qet-a3-landscape-grid.json",
    ] {
        let mut doc = starter(f);
        edit(&mut doc);
        lib.push(doc);
    }
    Model::from_records(p, records, lib).expect("a valid model")
}

fn commit(e: &mut Engine, name: &str, params: Value) {
    let cmd = AnyCommand {
        name: name.to_owned(),
        params,
    };
    let p = e.preview(&cmd).unwrap_or_else(|err| panic!("{name}: {err:?}"));
    e.commit(p).unwrap();
}

fn copy(e: &Engine, id: Id<FormDef>) -> Option<LibCopy<FormDef>> {
    e.model()
        .table::<LibCopy<FormDef>>()
        .get(&id)
        .map(|c| (**c).clone())
}

/// The title block's JSON with its second cell's text changed.
fn changed(e: &Engine, block: Id<FormDef>, text: &str) -> Value {
    let mut item = serde_json::to_value(&copy(e, block).expect("the block").item).unwrap();
    item["cells"][1]["label"] = json!(text);
    item
}

#[test]
fn the_first_change_forks_the_starter_block_and_its_frames_and_re_points_every_reference() {
    let mut e = engine(model());
    let project = e.model().project().id.as_uuid();
    commit(
        &mut e,
        "project.settings.set",
        json!({"member": "frames", "value": {"default": GRID, "per_page_type": [{"page_type": "title_page", "form": COVER}]}}),
    );
    let block = CreateReportBlock {
        source: ReportSource::Kind(ReportKind::Toc),
        form: form_id(NOGRID),
        section: Some(SEC1),
        first_folio: Some(990),
        title: Default::default(),
        manual: Default::default(),
        sort: vec![],
        group: None,
        continuation: ContinuationPolicy::default(),
        empty: EmptyPolicy::default(),
        embedded: None,
        columns: Vec::new(),
        form_copies: Vec::new(),
        layout_page: None,
    };
    let r = e.execute(&block).expect("report_block.create");
    let report: Id<ReportBlock> = typed(r.outcome.primary.expect("the block"));
    let before = lines(e.model());
    let forks: Vec<Uuid> = (1..=4).map(|n| Uuid::from_u128(0xf0 + n)).collect();
    let fork = |n: usize| -> Id<FormDef> { v5::lib_copy_id(project, forks[n]) };
    let item = changed(&e, form_id(BLOCK), "Drawing no.:");
    commit(
        &mut e,
        "form.edit",
        json!({"copy": BLOCK, "form": item, "forks": forks.iter().map(Uuid::to_string).collect::<Vec<_>>()}),
    );
    // The block's fork, then its frames' in copy ID order: the cover, the grid and the no-grid frame.
    let new_block = copy(&e, fork(0)).expect("the block's fork");
    assert_eq!(new_block.library_id.as_uuid(), project);
    assert_eq!(
        new_block.item.cells[1]
            .label
            .as_ref()
            .map(|l| ew_derive::text_in(l, None).to_owned()),
        Some("Drawing no.:".to_owned())
    );
    assert!(
        new_block.item.meta.extra.contains_key("source"),
        "it says what it is a fork of"
    );
    for (n, frame) in [(1, COVER), (2, GRID), (3, NOGRID)] {
        let f = copy(&e, fork(n)).unwrap_or_else(|| panic!("the fork of {frame}"));
        assert_eq!(f.item.title_block, Some(fork(0)), "{frame} names the new block");
        assert_eq!(
            copy(&e, form_id(frame))
                .expect("the starter copy stays")
                .item
                .title_block,
            Some(form_id(BLOCK))
        );
    }
    assert!(
        copy(&e, form_id(A3_GRID)).is_some_and(|f| f.item.title_block != Some(fork(0))),
        "a frame of another block is not touched"
    );
    let page = e
        .model()
        .table::<Page>()
        .get(&PAGE_101)
        .expect("page 101")
        .clone();
    assert_eq!(page.form, Some(fork(2)));
    let report = e
        .model()
        .table::<ReportBlock>()
        .get(&report)
        .expect("the block")
        .clone();
    assert_eq!(report.form, fork(3));
    let frames = e
        .model()
        .project()
        .settings
        .extra
        .get("frames")
        .cloned()
        .expect("the defaults");
    assert_eq!(frames["default"], json!(fork(2).to_string()));
    assert_eq!(frames["per_page_type"][0]["form"], json!(fork(1).to_string()));

    // A second change is made in place: the same copy, no fork.
    let item = changed(&e, fork(0), "Drawing:");
    let refused = e.preview(&AnyCommand {
        name: "form.edit".to_owned(),
        params: json!({"copy": fork(0).to_string(), "form": item, "forks": [Uuid::from_u128(0xf9).to_string()]}),
    });
    assert!(
        matches!(refused, Err(CmdError::InvalidParams { .. })),
        "a copy of the project's own is not forked"
    );
    commit(
        &mut e,
        "form.edit",
        json!({"copy": fork(0).to_string(), "form": item}),
    );
    assert_eq!(
        copy(&e, fork(0)).expect("in place").item.cells[1]
            .label
            .as_ref()
            .map(|l| ew_derive::text_in(l, None).to_owned()),
        Some("Drawing:".to_owned())
    );
    e.undo(&Origin::Gui).expect("undo the second");
    e.undo(&Origin::Gui).expect("undo the first");
    assert_eq!(lines(e.model()), before, "undo gives the starter copies back");
}

/// A table's cells stay inside the block, at least 2 mm each way, side by side; nothing changed is refused.
#[test]
fn what_a_title_block_cannot_be_is_refused() {
    let mut e = engine(model());
    let forks: Vec<String> = (1..=4).map(|n| Uuid::from_u128(0xf0 + n).to_string()).collect();
    let block = copy(&e, form_id(BLOCK)).expect("the block");
    let item = serde_json::to_value(&block.item).unwrap();
    let mut refuse = |item: Value, at: &str| {
        let r = e.preview(&AnyCommand {
            name: "form.edit".to_owned(),
            params: json!({"copy": BLOCK, "form": item, "forks": forks}),
        });
        match r {
            Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, at),
            Err(CmdError::Rejected { .. }) => assert_eq!(at, ""),
            other => panic!("expected a refusal at {at}, got {other:?}"),
        }
    };
    refuse(item.clone(), "");
    let mut small = item.clone();
    small["cells"][1]["rect"][1][1] = small["cells"][1]["rect"][0][1].clone();
    refuse(small, "/form/cells/1");
    let mut over = item.clone();
    over["cells"][2]["rect"] = over["cells"][1]["rect"].clone();
    refuse(over, "/form/cells/2");
    let mut out = item;
    out["cells"][1]["rect"][1][0] = json!(900_000_000);
    refuse(out, "/form/cells/1");
}

/// `frame.set_grid` (doc 09 §3.9, doc 06 §2.4): 10 columns from 1 and 6 rows share the drawing area exactly, so its
/// outer edges stay; a title block that grows over the drawing of a page of the frame is refused, naming the page.
#[test]
fn columns_and_rows_share_the_drawing_area_and_a_block_does_not_grow_over_drawing() {
    let mut e = engine(model());
    let project = e.model().project().id.as_uuid();
    let fork = Uuid::from_u128(0xf1);
    let old = copy(&e, form_id(GRID))
        .expect("the grid frame")
        .item
        .grid
        .expect("its grid");
    commit(
        &mut e,
        "frame.set_grid",
        json!({"frame": GRID, "columns": {"count": 10, "first": 1}, "rows": {"count": 6},
            "forks": [fork.to_string()]}),
    );
    let new_id: Id<FormDef> = v5::lib_copy_id(project, fork);
    let new = copy(&e, new_id)
        .expect("the frame's fork")
        .item
        .grid
        .expect("its grid");
    let length =
        |step: ew_units::Grid, n: u16| (u128::from(step.num()) * u128::from(n), u128::from(step.den()));
    let same = |a: (u128, u128), b: (u128, u128)| a.0 * b.1 == b.0 * a.1;
    assert_eq!(
        (new.columns.count, new.columns.start_index, new.rows.count),
        (10, 1, 6)
    );
    assert!(
        same(length(new.columns.width, 10), length(old.columns.width, 17)),
        "the width stays exactly"
    );
    assert!(
        same(length(new.rows.height, 6), length(old.rows.height, 8)),
        "the height stays exactly"
    );
    let page = e
        .model()
        .table::<Page>()
        .get(&PAGE_101)
        .expect("page 101")
        .clone();
    assert_eq!(page.form, Some(new_id), "the page prints on the fork");

    // A title block 165 mm higher leaves the rows 4 mm: the drawing of page 101 lies where it would grow.
    let block = copy(&e, form_id(BLOCK)).expect("the block").item.sheet.height;
    let forks: Vec<String> = (2..=5).map(|n| Uuid::from_u128(0xf0 + n).to_string()).collect();
    let r = e.preview(&AnyCommand {
        name: "frame.set_grid".to_owned(),
        params: json!({"frame": new_id.to_string(), "title_block_height": block.0 + 165_000_000, "forks": forks}),
    });
    match r {
        Err(CmdError::Rejected { text, .. }) => assert!(text.to_string().contains("101"), "{text}"),
        other => panic!("expected the refusal naming page 101, got {other:?}"),
    }
}

/// `form.make_own` (doc 09 §3.9, the journey's step 9): the cover given a title block of its own (its frame forked
/// first) prints that copy; page 101's frame keeps the shared block, and a change of the cover's block leaves it as it
/// was. A page given a frame of its own names its copy.
#[test]
fn the_cover_given_its_own_block_leaves_page_101s_block_as_it_was() {
    let mut e = engine(model());
    let project = e.model().project().id.as_uuid();
    let (own, cover_fork) = (Uuid::from_u128(0xa1), Uuid::from_u128(0xa2));
    commit(
        &mut e,
        "form.make_own",
        json!({"frame": COVER, "forks": [own.to_string(), cover_fork.to_string()]}),
    );
    let own_id: Id<FormDef> = v5::lib_copy_id(project, own);
    let cover = copy(&e, v5::lib_copy_id(project, cover_fork)).expect("the cover's fork");
    assert_eq!(cover.item.title_block, Some(own_id));
    assert_eq!(
        copy(&e, form_id(GRID)).expect("the grid frame").item.title_block,
        Some(form_id(BLOCK))
    );
    let item = changed(&e, own_id, "Cover drawing:");
    commit(
        &mut e,
        "form.edit",
        json!({"copy": own_id.to_string(), "form": item}),
    );
    let block = copy(&e, form_id(BLOCK)).expect("the shared block");
    assert_ne!(
        block.item.cells[1]
            .label
            .as_ref()
            .map(|l| ew_derive::text_in(l, None).to_owned()),
        Some("Cover drawing:".to_owned()),
        "page 101's block is as it was"
    );
    let page_own = Uuid::from_u128(0xa3);
    commit(
        &mut e,
        "form.make_own",
        json!({"page": PAGE_101.to_string(), "forks": [page_own.to_string()]}),
    );
    let page = e
        .model()
        .table::<Page>()
        .get(&PAGE_101)
        .expect("page 101")
        .clone();
    assert_eq!(page.form, Some(v5::lib_copy_id(project, page_own)));
}

/// The mini project of [`model`] whose starter title block also has a text of its own at 9 mm from its top.
fn model_with_block_text() -> Model {
    model_where(|doc| {
        if let LibDocument::Form(block) = doc
            && block.id == form_id(BLOCK)
        {
            block.item.graphics.push(
                serde_json::from_value(
                    json!({"text": {"pos": [10_000_000, 9_000_000], "text": "Note", "align": "left"}}),
                )
                .expect("a text"),
            );
            block.sha256 = ew_schema::Sha256(
                ew_format::library_copy_hash(&LibDocument::Form(block.clone())).expect("a hash"),
            );
        }
    })
}

/// A lower title block (doc 09 §3.9) scales its cells and its texts with it, and is refused where a cell would be lower
/// than 2 mm: the starter block's rows of 4.5 mm stand 18 mm high, so 7 mm leaves them 1.75 mm.
#[test]
fn a_lower_title_block_scales_its_texts_and_keeps_its_cells_2_mm_high() {
    let mut e = engine(model_with_block_text());
    let project = e.model().project().id.as_uuid();
    let forks: Vec<Uuid> = (1..=6).map(|n| Uuid::from_u128(0xc0 + n)).collect();
    let forks_json: Vec<String> = forks.iter().map(Uuid::to_string).collect();
    let old = copy(&e, form_id(BLOCK)).expect("the block").item.sheet.height;
    let r = e.preview(&AnyCommand {
        name: "frame.set_grid".to_owned(),
        params: json!({"frame": GRID, "title_block_height": 7_000_000, "forks": forks_json}),
    });
    match r {
        Err(CmdError::InvalidParams { path, text, .. }) => {
            assert_eq!(path, "/title_block_height");
            assert!(text.to_string().contains("2 mm"), "{text}");
        }
        other => panic!("expected a refusal of the height, got {other:?}"),
    }
    let h = old.0 / 2;
    commit(
        &mut e,
        "frame.set_grid",
        json!({"frame": GRID, "title_block_height": h, "forks": forks_json}),
    );
    // The forks in `forked_copies` order: the grid frame, then its block.
    let block = copy(&e, v5::lib_copy_id(project, forks[1])).expect("the block's fork");
    let text_y = block.item.graphics.iter().find_map(|g| match g {
        ew_schema::Primitive::Text(t) => Some(t.pos.y.0),
        _ => None,
    });
    assert_eq!(
        text_y,
        Some(9_000_000 * h / old.0),
        "the text keeps its place in the block"
    );
}

/// Where the title block of `frame` ends, from the sheet's top: the header band, the rows and the block's height, as an
/// exact fraction (numerator, denominator).
fn block_bottom(frame: &FormDef, block: &FormDef) -> (i128, i128) {
    let g = frame.grid.as_ref().expect("a frame with a grid");
    let (hn, hd) = g
        .header_band
        .map_or((0, 1), |h| (i128::from(h.num()), i128::from(h.den())));
    let (rn, rd) = (i128::from(g.rows.height.num()), i128::from(g.rows.height.den()));
    let rows = (rn * i128::from(g.rows.count), rd);
    let den = hd * rows.1;
    (hn * rows.1 + rows.0 * hd + i128::from(block.sheet.height.0) * den, den)
}

/// The copy the project made of the form copy `original` (its `fork_of`).
fn fork_of(e: &Engine, original: Id<FormDef>) -> LibCopy<FormDef> {
    let item = copy(e, original).expect("the original").item_id.as_uuid().to_string();
    e.model()
        .table::<LibCopy<FormDef>>()
        .iter()
        .map(|(_, c)| c)
        .find(|c| c.item.meta.extra.get("source").and_then(|s| s["fork_of"]["item"].as_str()) == Some(item.as_str()))
        .map(|c| (**c).clone())
        .unwrap_or_else(|| panic!("a fork of {original:?}"))
}

/// A title block's height is the block's (doc 09 §3.9): lowered 6 mm from page 101's frame, it ends where it ended on
/// every frame that names it, the cover's and the no-grid frame's as well, each one's rows taking up the difference.
#[test]
fn the_title_blocks_height_set_from_one_frame_fits_every_frame_that_names_it() {
    let mut e = engine(model());
    let block = copy(&e, form_id(BLOCK)).expect("the block").item;
    let frames = [COVER, GRID, NOGRID];
    let before: Vec<(i128, i128)> = frames
        .iter()
        .map(|f| block_bottom(&copy(&e, form_id(f)).expect("the frame").item, &block))
        .collect();
    let forks: Vec<String> = (1..=4).map(|n| Uuid::from_u128(0xf0 + n).to_string()).collect();
    commit(
        &mut e,
        "frame.set_grid",
        json!({"frame": GRID, "title_block_height": block.sheet.height.0 - 6_000_000, "forks": forks}),
    );
    let lower = fork_of(&e, form_id(BLOCK)).item;
    assert_eq!(lower.sheet.height.0, block.sheet.height.0 - 6_000_000);
    for (f, (n, d)) in frames.iter().zip(before) {
        let (an, ad) = block_bottom(&fork_of(&e, form_id(f)).item, &lower);
        assert_eq!(an * d, n * ad, "the block of {f} ends where it ended");
    }
}
