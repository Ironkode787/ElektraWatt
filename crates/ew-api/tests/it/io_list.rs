// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 5, PX-1 over Golden A: the `io_list` report kind (FR-PLC-023; doc 10 §5).
//!
//! The orchestrator's rulings of cycle 42 that these tests rely on, quoted:
//! - Ruling 4 (b): "the 20 that this build fills ..., the three added with their writers".
//! - Ruling 5 (a): "read `extra.params.include_deactivated` provisionally as `parts_summary` reads `mode`, a value of
//!   another type giving the default and one `DeriveNote::MalformedShim` naming the block and the key"; read through
//!   `ew_reports::kind_params` since WP-S3.06 part 1, whose ruling 6 names the block "`report block <first folio>
//!   params.<key>`".
//! - Ruling 10 (a): "`io_list` takes `ew_derive::stations(q)`'s order (configuration project, order, ID)".
//! - ID-950 ruling 1 (PX-1 as ruled): "On Golden A the default gives 28 rows (8 used, 20 spare) and
//!   `include_deactivated: true` gives 56 (the 28 deactivated added in place)".
//!
//! Tests:
//! - `the_default_io_list_lists_28_rows_of_used_and_spare_channels`
//! - `include_deactivated_lists_56_rows_in_place`
//! - `a_parameter_that_is_not_a_boolean_gives_28_rows_and_one_note`
//! - `the_export_by_block_writes_the_header_and_28_rows`
//! - `the_rows_are_deterministic`
//! - `px1_56_rows_paginate_on_the_house_table_without_an_overflow`

use crate::{common, golden_common};

use ew_api::output::{RowExport, page_layer, row_export};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::ew_ids::Id;
use ew_derive::{DeriveNote, PageOrder, Q};
use ew_dl::Layer;
use ew_reports::BlockRows;
use ew_reports::csv::row_texts;
use ew_scene::page::PageFinding;
use ew_scene::table::TableFinding;
use ew_schema::{Record, ReportBlock};
use serde_json::{Value, json};

/// The first folio of the test's `io_list` block (Golden A has none; forbids adding one).
const FOLIO: u32 = 950;

fn run(session: &mut Session, name: &str, params: Value) -> ew_api::Receipt {
    let cmd = AnyCommand {
        name: name.into(),
        params: params.clone(),
    };
    match session.execute(&cmd) {
        Err(CmdError::NeedsPreview { .. }) => session.preview(&cmd).and_then(|p| session.commit(p)),
        other => other,
    }
    .unwrap_or_else(|e| panic!("{name} {params}: {e:?}"))
}

/// Golden A with an `io_list` block made by `report_block.create` on the form of folio 908, and the block.
fn golden_with_io_list() -> (Session, ReportBlock) {
    let mut s = common::golden_session();
    let form = {
        let q = s.query();
        let plc = golden_common::block(&golden_common::batch(), "908");
        q.report_blocks().get(plc).expect("folio 908's block").form
    };
    let r = run(
        &mut s,
        "report_block.create",
        json!({"source": {"kind": "io_list"}, "form": form, "first_folio": FOLIO, "title": "I/O List {n}"}),
    );
    let id: Id<ReportBlock> = Id::from_uuid(r.outcome.primary.expect("the block").1.as_uuid());
    let block = s
        .query()
        .report_blocks()
        .get(id)
        .expect("created")
        .as_ref()
        .clone();
    (s, block)
}

/// A session over Golden A with `block` added (a block value no command writes: `params`).
fn golden_with_block(block: ReportBlock) -> Session {
    golden_common::session_with(&golden_common::batch(), |_, records| {
        records.push(Record::ReportBlock(block))
    })
}

fn with_params(mut block: ReportBlock, params: Value) -> ReportBlock {
    block.extra.insert("params".to_owned(), params);
    block
}

/// The printed rows of a block.
fn printed(q: &Q, id: Id<ReportBlock>) -> Vec<Vec<String>> {
    let set = q.get::<BlockRows>(id);
    assert!(set.is_derived(), "{:?}", set.status);
    (0..set.rows.len()).map(|i| row_texts(q, &set, i)).collect()
}

fn row(cells: [&str; 20]) -> Vec<String> {
    cells.iter().map(|c| (*c).to_owned()).collect()
}

/// The row of a used channel: -201U3 channel 1, Q0.1, bound to coil 14 of -201U3-Q1 drawn at 203-H1.
fn first_used() -> Vec<String> {
    row([
        "PLC1",
        "",
        "-201U1",
        "TBD",
        "1",
        "-201U3",
        "533361",
        "1",
        "1",
        "DO",
        "Q0.1",
        "",
        "Bool",
        "",
        "TA05-V-15",
        "TA05-V-15",
        "-201U3-Q1",
        "14",
        "203-H1",
        "used",
    ])
}

#[test]
fn the_default_io_list_lists_28_rows_of_used_and_spare_channels() {
    let (s, block) = golden_with_io_list();
    let q = s.query();
    let rows = printed(&q, block.id);
    assert_eq!(rows.len(), 28);
    let state = |r: &Vec<String>| r[19].clone();
    assert_eq!(rows.iter().filter(|r| state(r) == "used").count(), 8);
    assert_eq!(rows.iter().filter(|r| state(r) == "spare").count(), 20);
    assert_eq!(rows[0], first_used());
    assert_eq!(
        rows[27],
        row([
            "PLC1",
            "",
            "-201U1",
            "TBD",
            "7",
            "-202U2",
            "533361",
            "7",
            "7",
            "DO",
            "Q6.7",
            "",
            "Bool",
            "",
            "Spare",
            "",
            "-202U2-Q4",
            "14",
            "206-H7",
            "spare",
        ])
    );
    // doc 20 §11.3: modules -201U3 to -201U8 and -202U2, the odd bits of each byte, in module order
    let addresses: Vec<&str> = rows.iter().map(|r| r[10].as_str()).collect();
    let want: Vec<String> = (0..7)
        .flat_map(|b| [1, 3, 5, 7].map(|i| format!("Q{b}.{i}")))
        .collect();
    assert_eq!(addresses, want);
}

#[test]
fn include_deactivated_lists_56_rows_in_place() {
    let (_, block) = golden_with_io_list();
    let s = golden_with_block(with_params(block.clone(), json!({"include_deactivated": true})));
    let q = s.query();
    let rows = printed(&q, block.id);
    assert_eq!(rows.len(), 56);
    assert_eq!(
        rows[0],
        row([
            "PLC1",
            "",
            "-201U1",
            "TBD",
            "1",
            "-201U3",
            "533361",
            "0",
            "0",
            "DO",
            "Q0.0",
            "",
            "Bool",
            "",
            "",
            "",
            "",
            "",
            "",
            "deactivated",
        ])
    );
    assert_eq!(rows[1], first_used());
    let addresses: Vec<&str> = rows.iter().map(|r| r[10].as_str()).collect();
    let want: Vec<String> = (0..7)
        .flat_map(|b| (0..8).map(move |i| format!("Q{b}.{i}")))
        .collect();
    assert_eq!(addresses, want);
    let deactivated = rows.iter().filter(|r| r[19] == "deactivated").count();
    assert_eq!(deactivated, 28);
    // `false` is the default
    let s = golden_with_block(with_params(block.clone(), json!({"include_deactivated": false})));
    assert_eq!(printed(&s.query(), block.id).len(), 28);
}

#[test]
fn a_parameter_that_is_not_a_boolean_gives_28_rows_and_one_note() {
    let (_, block) = golden_with_io_list();
    for (value, shown) in [(json!("yes"), "\"yes\""), (json!(1), "1")] {
        let s = golden_with_block(with_params(block.clone(), json!({"include_deactivated": value})));
        let mut q = golden_common::plain(&s);
        assert_eq!(q.get::<BlockRows>(block.id).rows.len(), 28, "{value}");
        let notes: Vec<DeriveNote> = q
            .diagnostics()
            .notes()
            .into_iter()
            .filter(|n| matches!(n, DeriveNote::MalformedShim { .. }))
            .collect();
        assert_eq!(
            notes,
            [DeriveNote::MalformedShim {
                member: "report block 950 params.include_deactivated".to_owned(),
                detail: format!("expected true or false, found {shown}"),
            }],
            "{value}"
        );
        assert_eq!(
            notes[0].to_string(),
            format!(
                "report block 950 params.include_deactivated is malformed (expected true or false, found {shown}); \
                 the default applies"
            )
        );
    }
}

/// `ew export report --block 950 --format csv` (the CLI calls `row_export`): the header and the 28 rows.
#[test]
fn the_export_by_block_writes_the_header_and_28_rows() {
    let (s, block) = golden_with_io_list();
    let q = s.query();
    let out = row_export(&q, &RowExport::Report, Some(&FOLIO.to_string())).expect("the I/O list's rows");
    assert_eq!(out.kind, "io_list");
    assert_eq!(out.rows, 28);
    assert_eq!(out.blocks, [block.id.to_string()]);
    let lines: Vec<&str> = out.csv.lines().collect();
    assert_eq!(lines.len(), 29);
    assert_eq!(
        lines[0],
        "row,Configuration project,CPU,Station,Bus address,Slot,Module,Part,Channel,Designation,Kind,Address,Symbol,\
         Data type,Comment,Function text,Process tag,Target,Target pin,Target page,State"
    );
    assert_eq!(
        lines[1],
        "data,PLC1,,-201U1,TBD,1,-201U3,533361,1,1,DO,Q0.1,,Bool,,TA05-V-15,TA05-V-15,-201U3-Q1,14,203-H1,used"
    );
}

#[test]
fn the_rows_are_deterministic() {
    let (s, block) = golden_with_io_list();
    let first = printed(&s.query(), block.id);
    assert_eq!(printed(&s.query(), block.id), first, "twice-run equal");
    let again = golden_with_block(block.clone());
    assert_eq!(printed(&again.query(), block.id), first, "a second session");
    // the records in reverse order
    let batch = golden_common::batch();
    let (project, mut records) = common::split(&batch);
    records.push(Record::ReportBlock(block.clone()));
    records.reverse();
    let reversed = common::session_of(project, records, batch.lib.clone());
    assert_eq!(printed(&reversed.query(), block.id), first, "record order");
}

/// Stop 1 of the brief: the 56 rows of `include_deactivated: true` paginated on Golden A's house table give no
/// `TableFinding::Overflow`, so no cell shrinks below the 1.8 mm floor (`ew_scene::table::MIN_CAP_HEIGHT`).
#[test]
fn px1_56_rows_paginate_on_the_house_table_without_an_overflow() {
    let (_, block) = golden_with_io_list();
    let s = golden_with_block(with_params(block.clone(), json!({"include_deactivated": true})));
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let pages: Vec<_> = order
        .entries
        .iter()
        .filter(|e| e.folio == "950" || e.folio.starts_with("950."))
        .map(|e| (e.folio.clone(), e.page))
        .collect();
    let folios: Vec<&str> = pages.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(folios, ["950", "950.1", "950.2"]);
    for (folio, page) in pages {
        let layer = page_layer(&q, page, Layer::Tables);
        let overflows: Vec<&PageFinding> = layer
            .findings
            .iter()
            .filter(|f| matches!(f, PageFinding::Table(TableFinding::Overflow { .. })))
            .collect();
        assert!(overflows.is_empty(), "folio {folio}: {overflows:?}");
    }
}
