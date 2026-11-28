// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Over the sample: the list kinds `device_list` and `terminal_overview`, the export by kind, the block parameters and
//! the revision overview's changed pages in the CSV.
//!
//! The kind CSVs `tests/expected/kinds/<kind>.csv` of every kind the export derives are written by the export by kind
//! on the sample (`row_export(q, &RowExport::Kind(kind), None)`, what `ew export report --kind <kind> --format csv`
//! prints); `EW_BLESS=1 cargo nextest run -p ew-api --all-features --test it -E 'test(/^report_kinds::/)'` rewrites
//! them. They are the only pin of the rows of each kind on a whole project.
//!
//! What these tests rely on, quoted:
//! - "a strip with no segment representation prints the cell of its `MainRep` ...; a strip with neither prints an
//!   empty cell".
//! - "`RowsError` gains four variants whose `Display` is the text exactly".
//! - "Every registered kind other than `terminal_diagram` exports (the refusal list is closed)".
//! - "`member` = `report block <first folio> params.<key>` ..., `detail` the lane's sentence".
//! - "`row_export` replaces the `pages` cell of every data row of a `revision_overview` row set with it,
//!   for `--block` and `--kind` alike".

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::common;

use ew_api::output::{RowExport, RowsError, page_layer, report_kinds, row_export};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::{PageOrder, PageRef, Q};
use ew_dl::{Item, ItemKind, Layer};
use ew_ids::Id;
use ew_reports::csv::row_texts;
use ew_reports::{BlockRows, Registry, project_rows};
use ew_scene::page::PageFinding;
use ew_scene::table::TableFinding;
use ew_schema::{Record, ReportBlock, ReportKind};
use serde_json::{Value, json};

fn expected_csv(kind: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/kinds")).join(format!("{kind}.csv"))
}

/// The report block printed from `folio`.
fn block_of(q: &Q, folio: u32) -> ReportBlock {
    q.report_blocks()
        .iter()
        .find(|(_, b)| b.first_folio == Some(folio))
        .map(|(_, b)| (**b).clone())
        .unwrap_or_else(|| panic!("no block of {folio}"))
}

fn export(q: &Q, kind: &str) -> ew_api::output::RowsCsv {
    row_export(q, &RowExport::Kind(kind.to_owned()), None).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

/// The cells of a CSV line of the `ew` dialect (no field of these files is quoted with a comma inside, except the
/// quoted ones, which this splitter keeps together).
fn fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The data lines of a CSV as cells.
fn data(csv: &str) -> Vec<Vec<String>> {
    csv.lines()
        .filter(|l| l.starts_with("data,"))
        .map(fields)
        .collect()
}

/// Each kind CSV equals the export by kind on the sample; twice-run equal. A kind the export derives has its file;
/// a kind the export by kind gives no rows of on the sample has its header only: the enclosure legend and the part data
/// (rows of an embedded block's selection only), the IO-Link port list, the medium overview, the numbering report (the
/// sample has no frozen tag or wire number; `ew-reports`' `numbering_report.rs` holds its rows), the open items and the
/// safety I/O list.
#[test]
fn the_kind_csvs_equal_the_export_by_kind() {
    let s = common::sample();
    let q = s.query();
    // Every kind the export by kind derives on the sample (`terminal_diagram` refuses: it lists one strip).
    let derived: Vec<(&str, ew_api::output::RowsCsv)> = report_kinds()
        .into_iter()
        .filter_map(|k| {
            row_export(&q, &RowExport::Kind(k.to_owned()), None)
                .ok()
                .map(|out| (k, out))
        })
        .collect();
    for (kind, out) in &derived {
        assert_eq!(export(&q, kind).csv, out.csv, "{kind}: twice-run equal");
        let path = expected_csv(kind);
        ew_testdir::compare_or_bless(&path, &out.csv);
    }
}

/// The designations of folio 910's device rows (the device tag overview), in its order.
fn tags_of_910(q: &Q) -> Vec<String> {
    let set = q.get::<BlockRows>(block_of(q, 910).id);
    (0..set.rows.len())
        .map(|i| row_texts(q, &set, i))
        .filter(|r| r[1] == "Device")
        .map(|r| r[0].clone())
        .collect()
}

/// The header and a row per device, the devices of folio 910 in its order, and the rows the sample's
/// script gives.
#[test]
fn the_device_list_lists_the_devices_of_folio_910() {
    let s = common::sample();
    let q = s.query();
    let out = export(&q, "device_list");
    assert_eq!(out.kind, "device_list");
    assert!(out.blocks.is_empty());
    assert_eq!(
        out.csv.lines().next(),
        Some("row,Tag,Function text,Parts,Technical data,Location (+),Main representation")
    );
    let rows = data(&out.csv);
    assert_eq!(out.rows, rows.len());
    let tags: Vec<String> = rows.iter().map(|r| r[1].clone()).collect();
    assert_eq!(tags, tags_of_910(&q));
    let row = |short: &str| -> Vec<String> {
        rows.iter()
            .find(|r| r[1].ends_with(short))
            .unwrap_or_else(|| panic!("no row {short}"))
            .clone()
    };
    let f1 = row("-101F1");
    assert_eq!(
        (f1[1].as_str(), f1[3].as_str(), f1[6].as_str()),
        ("+CAB-101F1", "EW-GEN-MCB-4", "101-E8")
    );
    let k1 = row("-201K1");
    assert_eq!((k1[3].as_str(), k1[6].as_str()), ("EW-GEN-VTM-1", "201-E2"));
    let uc1 = row("-UC1");
    assert_eq!((uc1[3].as_str(), uc1[6].as_str()), ("EW-GEN-ENC-1", ""));
    for short in ["-X1011", "-X1021", "-X1031"] {
        assert_eq!(row(short)[3], "", "{short}: no part");
    }
}

/// FR-REP-010: the three strips' rows whole, in `terminal_diagram`'s default strip order (the natural order of the
/// printed tag). Every terminal of the sample is wired: all used.
#[test]
fn the_terminal_overview_lists_the_three_strips() {
    let s = common::sample();
    let q = s.query();
    let out = export(&q, "terminal_overview");
    assert_eq!(
        out.csv.lines().next(),
        Some("row,Strip,Location (+),Terminals,Used,Spare,Other,Parts,Segments")
    );
    let rows: Vec<Vec<String>> = data(&out.csv)
        .into_iter()
        .map(|r| {
            let mut r = r;
            r.remove(2); // the location, asserted below
            r
        })
        .collect();
    let want = |cells: [&str; 8]| cells.iter().map(|c| (*c).to_owned()).collect::<Vec<String>>();
    assert_eq!(
        rows,
        [
            want([
                "data",
                "-X1011",
                "5",
                "5",
                "0",
                "0",
                "3 \u{d7} EW-GEN-TRM-1, 1 \u{d7} EW-GEN-TRM-1-BU, 1 \u{d7} EW-GEN-TRM-2",
                "101-B2"
            ]),
            want([
                "data",
                "-X1021",
                "8",
                "8",
                "0",
                "0",
                "8 \u{d7} EW-GEN-TRM-1",
                "102-B4"
            ]),
            want([
                "data",
                "-X1031",
                "5",
                "5",
                "0",
                "0",
                "3 \u{d7} EW-GEN-TRM-1, 2 \u{d7} EW-GEN-TRM-2",
                "103-F4"
            ]),
        ]
    );
    // the location as `structure_overview` prints it for the strip devices' location
    let locations: Vec<String> = data(&out.csv).into_iter().map(|r| r[2].clone()).collect();
    assert_eq!(locations, ["+CAB", "+CAB", "+CAB"]);
}

/// Per strip, used, spare and other equal the counts of its `terminal_diagram` rows' State column.
#[test]
fn the_counts_agree_with_the_terminal_diagram_states() {
    let s = common::sample();
    let q = s.query();
    let overview = project_rows(&q, &ReportKind::TerminalOverview, None).expect("the overview");
    for (i, row) in overview.rows.iter().enumerate() {
        let strip = row.id.entities().next().expect("the strip");
        let diagram =
            project_rows(&q, &ReportKind::TerminalDiagram, Some(strip)).expect("the strip's diagram");
        let state = diagram.column("state").unwrap();
        let states: Vec<String> = (0..diagram.rows.len())
            .map(|r| row_texts(&q, &diagram, r)[state].clone())
            .collect();
        let count = |f: &dyn Fn(&str) -> bool| states.iter().filter(|s| f(s)).count().to_string();
        let texts = row_texts(&q, &overview, i);
        assert_eq!(texts[2], states.len().to_string(), "terminals");
        assert_eq!(texts[3], count(&|s| s == "used"), "used");
        assert_eq!(texts[4], count(&|s| s == "spare"), "spare");
        assert_eq!(
            texts[5],
            count(&|s| matches!(s, "reserved" | "not fitted" | "unused")),
            "other"
        );
    }
}

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

/// The device tagged `tag`.
fn device(s: &Session, tag: &str) -> Id<ew_schema::Device> {
    Id::from_uuid(
        s.resolve_reference(&format!("tag:{tag}"))
            .unwrap_or_else(|e| panic!("no device {tag}: {e:?}")),
    )
}

/// A `reserved` terminal counts in Other; renumbering -X1031 to -X1041 (a frozen tag) keeps the row ID.
#[test]
fn a_reserved_terminal_counts_in_other_and_a_renumber_keeps_the_row() {
    let mut s = common::sample();
    let strip = device(&s, "-X1031");
    let row_of = |q: &Q| -> (ew_reports::RowSet, usize) {
        let set = project_rows(q, &ReportKind::TerminalOverview, None).unwrap();
        let i = set
            .rows
            .iter()
            .position(|r| r.id.entities().next() == Some(strip.as_uuid()))
            .expect("the strip's row");
        (set, i)
    };
    let id_before = {
        let q = s.query();
        let (set, i) = row_of(&q);
        set.rows[i].id.clone()
    };
    // the shield terminal of -X1031 made reserved: one used fewer, one other more
    let shield = device(&s, "-X1031:SH");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [shield.to_string()], "path": "state", "value": "reserved"}),
    );
    let q = s.query();
    let (set, i) = row_of(&q);
    let texts = row_texts(&q, &set, i);
    assert_eq!(
        texts[..7],
        [
            "-X1031",
            "+CAB",
            "5",
            "4",
            "0",
            "1",
            "3 \u{d7} EW-GEN-TRM-1, 2 \u{d7} EW-GEN-TRM-2"
        ]
    );
    drop(q);
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [strip.to_string()], "path": "tag.frozen", "value": {"text": "X1041"}}),
    );
    let q = s.query();
    let set = project_rows(&q, &ReportKind::TerminalOverview, None).unwrap();
    let renamed = set
        .rows
        .iter()
        .position(|r| r.id == id_before)
        .expect("the row keeps its ID");
    assert_eq!(row_texts(&q, &set, renamed)[0], "-X1041");
}

/// The four refusals as `RowsError::to_string()`, whole.
#[test]
fn the_refusals_of_the_export_by_kind() {
    let s = common::sample();
    let q = s.query();
    let kind = |k: &str| RowExport::Kind(k.to_owned());
    assert_eq!(
        row_export(&q, &kind("device_list"), Some("910")).unwrap_err(),
        RowsError::KindAndBlock
    );
    assert_eq!(
        RowsError::KindAndBlock.to_string(),
        "give `--block` or `--kind`, not both"
    );
    assert_eq!(
        RowsError::KindPages.to_string(),
        "`--kind` exports rows: a kind has pages only through a block, use `--block <block> --format pdf`"
    );
    let names: Vec<String> = Registry::builtin().names().map(str::to_owned).collect();
    let e = row_export(&q, &kind("device_lists"), None).unwrap_err();
    assert_eq!(
        e,
        RowsError::UnknownKind("device_lists".to_owned(), names.clone())
    );
    assert_eq!(
        e.to_string(),
        format!(
            "unknown report kind `device_lists`; the kinds of this build are: {}",
            names.join(", ")
        )
    );
    let e = row_export(&q, &kind("terminal_diagram"), None).unwrap_err();
    assert_eq!(e, RowsError::KindOneStrip);
    assert_eq!(
        e.to_string(),
        "`terminal_diagram` lists one strip: use `ew export terminals`"
    );
}

/// Every registered kind but `terminal_diagram` exports a header line without a panic.
#[test]
fn every_registered_kind_but_terminal_diagram_exports() {
    let s = common::sample();
    let q = s.query();
    for name in Registry::builtin().names().filter(|n| *n != "terminal_diagram") {
        let out = export(&q, name);
        assert!(out.csv.starts_with("row,"), "{name}: {}", out.csv);
        assert_eq!(out.kind, name);
        assert_eq!(out.csv.lines().count(), out.rows + 1, "{name}");
    }
}

/// `--kind parts_summary` writes the component-mode rows, the rows of the bill of materials' export.
#[test]
fn the_parts_summary_kind_is_the_component_bom() {
    let s = common::sample();
    let q = s.query();
    let bom = row_export(&q, &RowExport::Bom(ew_api::BomMode::Component), None).expect("the BOM");
    assert_eq!(export(&q, "parts_summary").csv, bom.csv);
}

/// The revision overview's CSV prints the changed pages as folio 911 does; the export by kind writes the same rows.
#[test]
fn the_revision_overview_csv_prints_the_changed_pages() {
    let s = common::sample();
    let q = s.query();
    let by_block = row_export(&q, &RowExport::Report, Some("911")).expect("folio 911's rows");
    let rows = data(&by_block.csv);
    let printed = s.page_list_table("911").expect("folio 911 has a table");
    assert_eq!(rows.len(), printed.rows.len());
    for (row, page) in rows.iter().zip(&printed.rows) {
        assert_eq!(row[1], page.cells[0], "the revision");
        assert_eq!(row.last(), page.cells.last(), "{}: the changed pages", row[1]);
    }
    assert_eq!(export(&q, "revision_overview").csv, by_block.csv);
}

/// The sample with `block` added (a block value no command writes: `params`).
fn sample_with_block(block: ReportBlock) -> Session {
    let (project, mut records, lib) = common::sample_records();
    records.push(Record::ReportBlock(block));
    common::session_of(project, records, lib)
}

/// A `parts_summary` block at folio 960 on folio 901's form with `params` (a block of the sample's data).
fn parts_block(params: Option<Value>) -> ReportBlock {
    let mut block = block_of(&common::sample().query(), 901);
    block.id = Id::from_uuid(ew_ids::Uuid::from_u128(0x5306_0001));
    block.first_folio = Some(960);
    block.source_ref = None;
    block.extra.remove("params");
    if let Some(p) = params {
        block.extra.insert("params".to_owned(), p);
    }
    block
}

/// A `Q` over the session's snapshot without the report engine.
fn plain(s: &Session) -> Q {
    Q::new(&s.world().model)
}

/// The number of rows of a session's derivation of `block`.
fn rows_of(s: &Session, block: Id<ReportBlock>) -> usize {
    plain(s).get::<BlockRows>(block).rows.len()
}

/// `params.mode` "assembly" gives the assembly rows; "Assembly", 3, an undeclared key and a `params` that is not an
/// object give the component rows. (The sample has no assembly, so its assembly rows list the same parts; the mode
/// shows in the assembly mode's Covers column.)
#[test]
fn the_parts_summary_mode_parameter() {
    let component = sample_with_block(parts_block(None));
    let block = parts_block(None).id;
    let component_rows = rows_of(&component, block);
    let assembly = sample_with_block(parts_block(Some(json!({"mode": "assembly"}))));
    {
        let q = plain(&assembly);
        let set = q.get::<BlockRows>(block);
        assert!(set.column("covers").is_some(), "assembly mode's Covers column");
    }
    for params in [
        json!({"mode": "Assembly"}),
        json!({"mode": 3}),
        json!({"modes": "assembly"}),
        json!(["assembly"]),
    ] {
        let s = sample_with_block(parts_block(Some(params.clone())));
        assert_eq!(rows_of(&s, block), component_rows, "{params}: the component rows");
        let q = plain(&s);
        assert!(
            q.get::<BlockRows>(block).column("covers").is_none(),
            "{params}: no Covers column"
        );
    }
}

/// The form and section of folio 910's block (the house table on `ew.frame.qet-a4-landscape-nogrid`).
fn form_of_910(s: &Session) -> (Value, Value) {
    let q = s.query();
    let b = q
        .report_blocks()
        .get(block_of(&q, 910).id)
        .expect("folio 910's block")
        .clone();
    (json!(b.form), json!(b.section))
}

/// The drawn and generated folios in page order, but those of the given blocks' first folios.
fn folios_but(q: &Q, but: &[&str]) -> Vec<String> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .map(|e| e.folio.clone())
        .filter(|f| !but.iter().any(|b| f == b || f.starts_with(&format!("{b}."))))
        .collect()
}

/// `device_list` and `terminal_overview` blocks made by `report_block.create` at folios 915 and 916
/// paginate with the house layout, no placeholder page and no overflow (no cell below the 1.8 mm floor); the TOC
/// lists them; every other page keeps its folio and order.
#[test]
fn the_list_kinds_paginate_on_the_house_table() {
    let mut s = common::sample();
    // The table of contents grows by the pages it lists, so its own folios are not compared.
    let before = folios_but(&s.query(), &["2"]);
    let (form, section) = form_of_910(&s);
    for (kind, folio, title) in [
        ("device_list", 915, "Device List {n}"),
        ("terminal_overview", 916, "Terminal Overview {n}"),
    ] {
        run(
            &mut s,
            "report_block.create",
            json!({"source": {"kind": kind}, "form": form, "section": section, "first_folio": folio,
                "title": title}),
        );
    }
    let q = s.query();
    assert_eq!(folios_but(&q, &["915", "916", "2"]), before);
    let order = q.get::<PageOrder>(());
    let pages: Vec<(String, PageRef)> = order
        .entries
        .iter()
        .filter(|e| {
            ["915", "916"]
                .iter()
                .any(|b| e.folio == *b || e.folio.starts_with(&format!("{b}.")))
        })
        .map(|e| (e.folio.clone(), e.page))
        .collect();
    let folios: Vec<&str> = pages.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(folios, ["915", "915.1", "916"]);
    let page_910 = order
        .entries
        .iter()
        .find(|e| e.folio == "910")
        .expect("folio 910")
        .page;
    let house_sizes = run_sizes(&page_layer(&q, page_910, Layer::Tables).list.items);
    for (folio, page) in &pages {
        assert!(
            order.generated(*page).is_some_and(|g| g.note.is_none()),
            "folio {folio}: a placeholder page"
        );
        let layer = page_layer(&q, *page, Layer::Tables);
        let overflows: Vec<&PageFinding> = layer
            .findings
            .iter()
            .filter(|f| matches!(f, PageFinding::Table(TableFinding::Overflow { .. })))
            .collect();
        assert!(overflows.is_empty(), "folio {folio}: {overflows:?}");
        // no text shrinks: every glyph run prints at a size folio 910's table prints
        let sizes = run_sizes(&layer.list.items);
        assert!(
            sizes.is_subset(&house_sizes),
            "folio {folio}: {sizes:?} against {house_sizes:?}"
        );
    }
    // the TOC lists the new pages
    let toc = q.get::<BlockRows>(block_of(&q, 2).id);
    let listed: Vec<String> = (0..toc.rows.len())
        .map(|i| row_texts(&q, &toc, i)[0].clone())
        .collect();
    for folio in ["915", "915.1", "916"] {
        assert!(
            listed.iter().any(|f| f == folio),
            "the TOC lists {folio}: {listed:?}"
        );
    }
}

/// The sizes of the glyph runs of a display list, nested groups included.
fn run_sizes(items: &[Item]) -> BTreeSet<i64> {
    let mut out = BTreeSet::new();
    for item in items {
        match &item.kind {
            ItemKind::Glyphs(run) => {
                out.insert(run.size.0);
            }
            ItemKind::Group(g) => out.extend(run_sizes(&g.items)),
            _ => {}
        }
    }
    out
}
