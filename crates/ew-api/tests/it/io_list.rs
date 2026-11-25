// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Over the sample project: the `io_list` report kind (FR-PLC-023; doc 10 §5).
//!
//! What these tests rely on, quoted:
//! - "the 20 that this build fills ..., the three added with their writers".
//! - "read `extra.params.include_deactivated` provisionally as `parts_summary` reads `mode`, a value of
//!   another type giving the default and one `DeriveNote::MalformedShim` naming the block and the key"; read through
//!   `ew_reports::kind_params`, which names the block "`report block <first folio>
//!   params.<key>`".
//! - "`io_list` takes `ew_derive::stations(q)`'s order (configuration project, order, ID)".
//!
//! The sample's channels, as its script makes them: the PLC's input card -104A1-A1 and output card -104A1-A2, eight
//! channels each, with the position switch -104B1 on one input and the two signal lamps on two outputs; and the valve
//! terminal -201K1's eight outputs, of which its four valves' coils take every second one and the other four are
//! deactivated. The default lists 20 rows (7 used, 13 spare); `include_deactivated: true` adds the 4 deactivated.
//! The other PLC and bus lists' rows are `ew-reports`' `plc_bus_lists.rs`.

use crate::common;

use ew_api::output::{RowExport, row_export};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::Q;
use ew_ids::Id;
use ew_reports::BlockRows;
use ew_reports::csv::row_texts;
use ew_schema::{Record, ReportBlock, ReportKind, ReportSource};
use serde_json::{Value, json};

/// The first folio of the test's `io_list` block (the sample has none).
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

/// The sample with an `io_list` block made by `report_block.create` on the form of its PLC overview, and the block.
fn sample_with_io_list() -> (Session, ReportBlock) {
    let mut s = common::sample();
    let form = {
        let q = s.query();
        q.report_blocks()
            .iter()
            .find(|(_, b)| b.source == ReportSource::Kind(ReportKind::PlcOverview))
            .map(|(_, b)| b.form)
            .expect("the PLC overview's block")
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

/// A session over the sample's records with `block` added (a block value no command writes: `params`).
fn sample_with_block(block: ReportBlock) -> Session {
    let (project, mut records, lib) = common::sample_records();
    records.push(Record::ReportBlock(block));
    common::session_of(project, records, lib)
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

/// The cells of a row by column: module, part, kind, target, target pin, target page, state.
fn key_cells(r: &[String]) -> [&str; 7] {
    [&r[5], &r[6], &r[9], &r[16], &r[17], &r[18], &r[19]].map(String::as_str)
}

/// The row of the position switch's input: -104B1's contact 14 on the input card, drawn at 104-F2.
fn switch_row(rows: &[Vec<String>]) -> &Vec<String> {
    rows.iter()
        .find(|r| r[16] == "-104B1")
        .expect("the position switch's input")
}

#[test]
fn the_default_io_list_lists_20_rows_of_used_and_spare_channels() {
    let (s, block) = sample_with_io_list();
    let q = s.query();
    let rows = printed(&q, block.id);
    assert_eq!(rows.len(), 20);
    let state = |r: &Vec<String>| r[19].clone();
    assert_eq!(rows.iter().filter(|r| state(r) == "used").count(), 7);
    assert_eq!(rows.iter().filter(|r| state(r) == "spare").count(), 13);
    assert_eq!(
        key_cells(switch_row(&rows)),
        ["-104A1-A1", "EW-GEN-DI-1", "DI", "-104B1", "14", "104-F2", "used"]
    );
    // Every row names the configuration project and its module; the modules in station order, each module's channels
    // in channel order.
    assert!(rows.iter().all(|r| r[0] == "PLC1"), "{rows:?}");
    let mut modules: Vec<&str> = rows.iter().map(|r| r[5].as_str()).collect();
    modules.dedup();
    assert_eq!(modules.len(), 3, "{modules:?}");
    assert!(modules.contains(&"-104A1-A1") && modules.contains(&"-104A1-A2") && modules.contains(&"-201K1"));
    for module in &modules {
        let channels: Vec<u32> = rows
            .iter()
            .filter(|r| r[5] == *module)
            .map(|r| r[7].parse().expect("a channel number"))
            .collect();
        assert!(channels.windows(2).all(|w| w[0] < w[1]), "{module}: {channels:?}");
    }
    // The valve terminal's four valves, each on its coil.
    let valves: Vec<&str> = rows
        .iter()
        .filter(|r| r[5] == "-201K1")
        .map(|r| r[16].as_str())
        .collect();
    assert_eq!(valves, ["-201K1-Q1", "-201K1-Q2", "-201K1-Q3", "-201K1-Q4"]);
}

#[test]
fn include_deactivated_lists_24_rows_in_place() {
    let (_, block) = sample_with_io_list();
    let s = sample_with_block(with_params(block.clone(), json!({"include_deactivated": true})));
    let q = s.query();
    let rows = printed(&q, block.id);
    assert_eq!(rows.len(), ew_testdir::sample_facts().channels);
    let deactivated: Vec<&str> = rows
        .iter()
        .filter(|r| r[19] == "deactivated")
        .map(|r| r[7].as_str())
        .collect();
    assert_eq!(
        deactivated,
        ["1", "3", "5", "7"],
        "the valve terminal's odd channels, in place"
    );
    let terminal: Vec<&str> = rows
        .iter()
        .filter(|r| r[5] == "-201K1")
        .map(|r| r[7].as_str())
        .collect();
    assert_eq!(terminal, ["0", "1", "2", "3", "4", "5", "6", "7"]);
    // `false` is the default
    let s = sample_with_block(with_params(block.clone(), json!({"include_deactivated": false})));
    assert_eq!(printed(&s.query(), block.id).len(), 20);
}

#[test]
fn a_parameter_that_is_not_a_boolean_gives_20_rows() {
    let (_, block) = sample_with_io_list();
    for value in [json!("yes"), json!(1)] {
        let s = sample_with_block(with_params(block.clone(), json!({"include_deactivated": value})));
        let q = Q::new(&s.world().model);
        assert_eq!(q.get::<BlockRows>(block.id).rows.len(), 20, "{value}");
    }
}

/// `ew export report --block 950 --format csv` (the CLI calls `row_export`): the header and the 20 rows.
#[test]
fn the_export_by_block_writes_the_header_and_20_rows() {
    let (s, block) = sample_with_io_list();
    let q = s.query();
    let out = row_export(&q, &RowExport::Report, Some(&FOLIO.to_string())).expect("the I/O list's rows");
    assert_eq!(out.kind, "io_list");
    assert_eq!(out.rows, 20);
    assert_eq!(out.blocks, [block.id.to_string()]);
    let lines: Vec<&str> = out.csv.lines().collect();
    assert_eq!(lines.len(), 21);
    assert_eq!(
        lines[0],
        "row,Configuration project,CPU,Station,Bus address,Slot,Module,Part,Channel,Designation,Kind,Address,Symbol,\
         Data type,Comment,Function text,Process tag,Target,Target pin,Target page,State"
    );
    let rows = printed(&q, block.id);
    let switch = rows
        .iter()
        .position(|r| r[16] == "-104B1")
        .expect("the switch's row");
    assert_eq!(lines[switch + 1], format!("data,{}", rows[switch].join(",")));
}

#[test]
fn the_rows_are_deterministic() {
    let (s, block) = sample_with_io_list();
    let first = printed(&s.query(), block.id);
    assert_eq!(printed(&s.query(), block.id), first, "twice-run equal");
    let again = sample_with_block(block.clone());
    assert_eq!(printed(&again.query(), block.id), first, "a second session");
    // the records in reverse order
    let (project, mut records, lib) = common::sample_records();
    records.push(Record::ReportBlock(block.clone()));
    records.reverse();
    let reversed = common::session_of(project, records, lib);
    assert_eq!(printed(&reversed.query(), block.id), first, "record order");
}
