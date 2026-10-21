// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The export job's page display lists are memoised per layer and incremental (FT-59; ARCH §6.4, doc 09 §4.1: "a
//! wire edit never rebuilds the title block").
//!
//! Each test exports the golden project once through the session's memo store, commits one edit and exports
//! again; the commit statistics of the memo store say which layer families ran again, and the second export
//! equals a cold export of the edited model. The memo oracle (`tests/memo_oracle.rs`, the sweep of
//! `ew_api::verify`) compares every layer with its cold value at every state of its edit sequence.

mod golden_common;

use std::collections::BTreeMap;

use ew_api::output::{
    ExportOptions, FrameLayer, LabelsLayer, PageEmbeddedOf, PageFrameOf, PageTableOf, SymbolsLayer,
    TablesLayer, WiresLayer, export_pdf,
};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::{PageOrder, PageRef, Query, TitleBlockValues};
use serde_json::json;

/// The re-executions of every layer family in the statistics of the last commit.
fn layer_runs(s: &Session) -> BTreeMap<&'static str, u64> {
    let stats = s.memo().expect("a memoising session").commit_stats();
    [
        FrameLayer::FAMILY,
        SymbolsLayer::FAMILY,
        WiresLayer::FAMILY,
        LabelsLayer::FAMILY,
        TablesLayer::FAMILY,
    ]
    .into_iter()
    .map(|f| (f, stats.family(f).re_executions))
    .collect()
}

/// The first of `candidates` the session accepts (a bulk command through preview and commit).
fn apply(s: &mut Session, candidates: &[AnyCommand]) {
    for c in candidates {
        let result = match s.execute(c) {
            Err(CmdError::NeedsPreview { .. }) => s.preview(c).and_then(|p| s.commit(p)),
            other => other,
        };
        if result.is_ok() {
            return;
        }
    }
    panic!("every candidate was refused: {candidates:?}");
}

/// The second export after an edit is the cold export of the edited model, byte for byte.
fn assert_equals_cold(s: &Session) -> Vec<u8> {
    let memo = s
        .export_pdf(&ExportOptions::default())
        .expect("the memoised export");
    let cold =
        export_pdf(&s.query_over(&s.world().model), &ExportOptions::default()).expect("the cold export");
    assert!(
        memo.bytes == cold.bytes,
        "the memoised export differs from the cold one"
    );
    memo.bytes
}

/// A colour override on a wire of folio 101: the wires and labels of that page may run again, the generated wire
/// list prints the new colour, and no frame (title block) and no symbol layer runs again.
#[test]
fn a_wire_edit_rebuilds_no_title_block() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let before = s
        .export_pdf(&ExportOptions::default())
        .expect("the first export")
        .bytes;
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let page = order
        .entries
        .iter()
        .find_map(|e| match e.page {
            PageRef::Drawn(p) if e.folio == "101" => Some(p),
            _ => None,
        })
        .expect("folio 101 is drawn");
    let wire = q
        .idx()
        .connections_by_page(page)
        .next()
        .expect("folio 101 has a wire");
    drop(q);
    let candidates: Vec<AnyCommand> = [json!(["RD"]), json!("RD")]
        .into_iter()
        .map(|value| AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "connection", "targets": [wire], "path": "overrides.colour", "value": value}),
        })
        .collect();
    apply(&mut s, &candidates);
    let after = assert_equals_cold(&s);
    let runs = layer_runs(&s);
    assert_eq!(runs[FrameLayer::FAMILY], 0, "a title block was rebuilt: {runs:?}");
    assert_eq!(
        runs[SymbolsLayer::FAMILY],
        0,
        "a symbol layer was rebuilt: {runs:?}"
    );
    assert!(runs[WiresLayer::FAMILY] >= 1, "the edited page's wires: {runs:?}");
    assert!(
        runs[WiresLayer::FAMILY] <= 2 && runs[LabelsLayer::FAMILY] <= 2,
        "only the edited page's wires and labels: {runs:?}"
    );
    assert!(before != after, "the wire list prints the new colour");
}

/// `page.renumber` of section 1 (folio 106 becomes 104, Golden B step B2): exactly the frames whose title-block
/// values change (Prev/Next, the folio) and exactly the tables layers whose printed table, embedded blocks or frame
/// form change (the TOC rows, the report cells that print a folio) run again; no symbol and no wire layer does.
#[test]
fn a_renumber_rebuilds_only_the_title_blocks_and_tables_that_change() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    s.export_pdf(&ExportOptions::default()).expect("the first export");
    // The cold inputs of the frame and the tables layers, by page.
    let inputs = |s: &Session| {
        let q = s.query_over(&s.world().model);
        q.get::<PageOrder>(())
            .entries
            .iter()
            .map(|e| {
                let embedded = match e.page {
                    PageRef::Drawn(p) => (*q.get::<PageEmbeddedOf>(p)).clone(),
                    _ => Vec::new(),
                };
                let frame = (*q.get::<PageFrameOf>(e.page)).clone();
                let title_block = (*q.get::<TitleBlockValues>(e.page)).clone();
                let tables = ((*q.get::<PageTableOf>(e.page)).clone(), embedded, frame);
                (e.page, (title_block, tables))
            })
            .collect::<BTreeMap<_, _>>()
    };
    let old = inputs(&s);
    golden_common::run(&mut s, "b2-renumber-pages.ewcmd");
    let new = inputs(&s);
    let changed = |pick: &dyn Fn(&(TitleBlock, Tables)) -> String| {
        new.iter()
            .filter(|(page, v)| old.get(*page).map(pick) != Some(pick(v)))
            .count()
    };
    let frames = changed(&|v| format!("{:?}", v.0));
    let tables = changed(&|v| format!("{:?}", v.1));
    assert!(
        frames > 0 && frames < new.len(),
        "{frames} of {} title blocks change",
        new.len()
    );
    assert!(
        tables > 0 && tables < new.len(),
        "{tables} of {} tables layers' inputs change",
        new.len()
    );
    assert_equals_cold(&s);
    let runs = layer_runs(&s);
    let count = |n: usize| u64::try_from(n).expect("a page count");
    assert_eq!(
        runs[FrameLayer::FAMILY],
        count(frames),
        "the frames that run again are the changed title blocks: {runs:?}"
    );
    assert_eq!(
        runs[TablesLayer::FAMILY],
        count(tables),
        "the tables layers that run again are those whose inputs changed: {runs:?}"
    );
    assert_eq!(
        runs[SymbolsLayer::FAMILY],
        0,
        "no symbol layer runs again: {runs:?}"
    );
    assert_eq!(runs[WiresLayer::FAMILY], 0, "no wire layer runs again: {runs:?}");
}

type TitleBlock = ew_derive::TitleBlock;
type Tables = (
    Option<ew_scene::table::TableView>,
    Vec<ew_scene::embedded::EmbeddedView>,
    Option<ew_api::output::FrameForms>,
);
