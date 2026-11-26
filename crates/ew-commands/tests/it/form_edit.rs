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
        lib.push(starter(f));
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
