// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Over Golden A: the list kinds `device_list` and `terminal_overview`, the export by kind, the block
//! parameters and the revision overview's changed pages in the CSV.
//!
//! The kind CSVs `golden/J-E-2416/expected/csv/<kind>.csv` of every kind the export derives are
//! written by the export by kind
//! (`row_export(q, &RowExport::Kind(kind), None)`, what `ew export report --kind <kind> --format csv` prints);
//! `EW_BLESS=1 cargo nextest run -p ew-api --all-features --test it -E 'test(/^report_kinds::/)'` rewrites them.
//!
//! What these tests rely on, quoted:
//! - "a strip with no segment representation prints the cell of its `MainRep` ..., which is the
//!   \"101-D2\" RK-2 names; a strip with neither prints an empty cell".
//! - "`RowsError` gains four variants whose `Display` is D4's text exactly".
//! - "Every registered kind other than `terminal_diagram` exports (D4's refusal list is closed)".
//! - "`member` = `report block <first folio> params.<key>` ..., `detail` the lane's sentence".
//! - "`row_export` replaces the `pages` cell of every data row of a `revision_overview` row set with it,
//!   for `--block` and `--kind` alike".
//! - "`expected/csv/io_list.csv` ...: the default `io_list` rows of Golden A (28 data rows),
//!   written by `--kind io_list`".

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::{common, golden_common};

use ew_api::output::{RowExport, RowsError, page_layer, report_kinds, row_export};
use ew_api::{AnyCommand, CmdError, Session};
use ew_derive::ew_ids::Id;
use ew_derive::{DeriveNote, PageOrder, PageRef, Q};
use ew_dl::{Item, ItemKind, Layer};
use ew_reports::csv::row_texts;
use ew_reports::{BlockRows, Registry, project_rows};
use ew_scene::page::PageFinding;
use ew_scene::table::TableFinding;
use ew_schema::{Record, ReportBlock};
use serde_json::{Value, json};

fn expected_csv(kind: &str) -> PathBuf {
    golden_common::repo()
        .join("golden/J-E-2416/expected/csv")
        .join(format!("{kind}.csv"))
}

/// Whether `EW_BLESS=1` asks for the committed files to be rewritten.
// The flag only chooses whether the committed files are rewritten; the compared results never depend on it.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

fn export(q: &Q, kind: &str) -> ew_api::output::RowsCsv {
    row_export(q, &RowExport::Kind(kind.to_owned()), None).unwrap_or_else(|e| panic!("{kind}: {e}"))
}

/// The cells of a CSV line of the golden dialect (no field of these files is quoted with a comma inside, except the
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

/// Each kind CSV equals the export by kind on Golden A; twice-run equal. A kind the
/// export derives has its file.
#[test]
fn the_kind_csvs_equal_the_export_by_kind() {
    let s = common::golden_session();
    let q = s.query();
    // Every kind the export by kind derives on Golden A (`terminal_diagram` refuses: it lists one strip).
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
        if bless() {
            std::fs::write(&path, &out.csv).unwrap();
            continue;
        }
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 writes it)", path.display()));
        assert_eq!(out.csv, committed, "{kind} (EW_BLESS=1 rewrites it)");
    }
}

/// The device tags of folio 910's device rows in order, read from `expected/text/910*.csv` (the designation cell is
/// the text item before each "Device" kind cell of the tables layer).
fn tags_of_910() -> Vec<String> {
    let dir = golden_common::repo().join("golden/J-E-2416/expected/text");
    let mut tags = Vec::new();
    for folio in ["910", "910.1", "910.2", "910.3"] {
        let text = std::fs::read_to_string(dir.join(format!("{folio}.csv"))).unwrap();
        let mut previous = String::new();
        for line in text.lines().skip(1) {
            let cells = fields(line);
            if cells[1] != "tables" {
                continue;
            }
            if cells[3] == "Device" {
                tags.push(previous.clone());
            }
            previous = cells[3].clone();
        }
    }
    tags
}

/// RK-1 (FR-REP-008): the header and 68 data rows, the devices of folio 910 in its order, and the rows doc 20 §6.1
/// gives.
#[test]
fn rk1_the_device_list_lists_the_68_devices_of_folio_910() {
    let s = common::golden_session();
    let q = s.query();
    let out = export(&q, "device_list");
    assert_eq!(out.kind, "device_list");
    assert!(out.blocks.is_empty());
    assert_eq!(
        out.csv.lines().next(),
        Some("row,Tag,Function text,Parts,Technical data,Location (+),Main representation")
    );
    let rows = data(&out.csv);
    assert_eq!(rows.len(), 68);
    assert_eq!(out.rows, 68);
    let tags: Vec<String> = rows.iter().map(|r| r[1].clone()).collect();
    assert_eq!(tags, tags_of_910());
    let row = |short: &str| -> Vec<String> {
        rows.iter()
            .find(|r| r[1].ends_with(short))
            .unwrap_or_else(|| panic!("no row {short}"))
            .clone()
    };
    let f1 = row("-101F1");
    assert_eq!(
        (f1[1].as_str(), f1[3].as_str(), f1[6].as_str()),
        ("=P01+J-E-2416-101F1", "A9P52610", "101-B8")
    );
    let u1 = row("-201U1");
    assert_eq!((u1[3].as_str(), u1[6].as_str()), ("195740, 550208", "201-E1"));
    let uc1 = row("-UC1");
    assert_eq!((uc1[3].as_str(), uc1[6].as_str()), ("NSYPLA1074G", ""));
    for short in ["S56", "E56"] {
        assert_eq!(row(short)[3], "", "{short}: no part");
    }
}

/// FR-REP-010: the three strips' rows whole, in `terminal_diagram`'s default strip order (the
/// natural order of the printed tag, so "-X1031" and "-X1032" before "1X1").
#[test]
fn rk2_the_terminal_overview_lists_the_three_strips() {
    let s = common::golden_session();
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
                "-X1031",
                "20",
                "8",
                "12",
                "0",
                "20 \u{d7} PT 2.5-TWIN",
                "103-A3, 103-A10"
            ]),
            want([
                "data",
                "-X1032",
                "5",
                "4",
                "1",
                "0",
                "5 \u{d7} PT 2.5-TWIN-PE",
                "103-F3"
            ]),
            want([
                "data",
                "1X1",
                "3",
                "3",
                "0",
                "0",
                "2 \u{d7} UK 5 N, 1 \u{d7} USLKG 5",
                "101-D2"
            ]),
        ]
    );
    // the location as `structure_overview` prints it for the strip devices' location
    let locations: Vec<String> = data(&out.csv).into_iter().map(|r| r[2].clone()).collect();
    assert_eq!(locations, ["+J-E-2416", "+J-E-2416", "+J-E-2416"]);
}

/// RK-2: per strip, used, spare and other equal the counts of its `terminal_diagram` rows' State column.
#[test]
fn rk2_the_counts_agree_with_the_terminal_diagram_states() {
    let s = common::golden_session();
    let q = s.query();
    let overview = project_rows(&q, "terminal_overview", None).expect("the overview");
    for (i, row) in overview.rows.iter().enumerate() {
        let strip = row.id.entities().next().expect("the strip");
        let diagram = project_rows(&q, "terminal_diagram", Some(strip)).expect("the strip's diagram");
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

/// The device of Golden A with this short tag.
fn device(s: &Session, short: &str) -> Id<ew_schema::Device> {
    *golden_common::tags(s)
        .iter()
        .find(|(_, t)| t.ends_with(short))
        .unwrap_or_else(|| panic!("no device {short}"))
        .0
}

/// RK-2: a `reserved` terminal counts in Other; renumbering -X1031 to -X1041 (a frozen tag) keeps the row ID.
#[test]
fn rk2_a_reserved_terminal_counts_in_other_and_a_renumber_keeps_the_row() {
    let mut s = common::golden_session();
    let strip = device(&s, "-X1031");
    let row_of = |q: &Q| -> (ew_reports::RowSet, usize) {
        let set = project_rows(q, "terminal_overview", None).unwrap();
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
    // a spare terminal of -X1031 made reserved: one spare fewer, one other more
    let spare = {
        let q = s.query();
        let mut ts: Vec<_> = q
            .idx()
            .terminals_of_strip(strip)
            .filter(|t| {
                q.devices()
                    .get(*t)
                    .is_some_and(|d| d.state == ew_schema::DeviceState::Spare)
            })
            .collect();
        ts.sort();
        ts[0]
    };
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [spare.to_string()], "path": "state", "value": "reserved"}),
    );
    let q = s.query();
    let (set, i) = row_of(&q);
    let texts = row_texts(&q, &set, i);
    assert_eq!(
        texts[..7],
        [
            "-X1031",
            "+J-E-2416",
            "20",
            "8",
            "11",
            "1",
            "20 \u{d7} PT 2.5-TWIN"
        ]
    );
    drop(q);
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [strip.to_string()], "path": "tag.frozen", "value": {"text": "X1041"}}),
    );
    let q = s.query();
    let set = project_rows(&q, "terminal_overview", None).unwrap();
    let renamed = set
        .rows
        .iter()
        .position(|r| r.id == id_before)
        .expect("the row keeps its ID");
    assert_eq!(row_texts(&q, &set, renamed)[0], "-X1041");
}

/// The four refusals as `RowsError::to_string()`, whole.
#[test]
fn rk3_the_refusals_of_the_export_by_kind() {
    let s = common::golden_session();
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
fn rk3_every_registered_kind_but_terminal_diagram_exports() {
    let s = common::golden_session();
    let q = s.query();
    for name in Registry::builtin().names().filter(|n| *n != "terminal_diagram") {
        let out = export(&q, name);
        assert!(out.csv.starts_with("row,"), "{name}: {}", out.csv);
        assert_eq!(out.kind, name);
        assert_eq!(out.csv.lines().count(), out.rows + 1, "{name}");
    }
}

/// RK-3: `--kind parts_summary` writes the component-mode rows, the rows of `expected/csv/bom_parts.csv`.
#[test]
fn rk3_the_parts_summary_kind_is_the_component_bom() {
    let s = common::golden_session();
    let q = s.query();
    let committed = std::fs::read_to_string(expected_csv("bom_parts")).unwrap();
    assert_eq!(export(&q, "parts_summary").csv, committed);
}

/// The revision overview's CSV prints the changed pages as folio 911 does: V1.0 empty, V2.0
/// "101, 106"; the export by kind writes the same rows.
#[test]
fn rk6_the_revision_overview_csv_prints_the_changed_pages() {
    let s = common::golden_session();
    let q = s.query();
    let by_block = row_export(&q, &RowExport::Report, Some("911")).expect("folio 911's rows");
    let rows = data(&by_block.csv);
    let pages = |label: &str| -> String {
        let r = rows
            .iter()
            .find(|r| r[1] == label)
            .unwrap_or_else(|| panic!("{label}"));
        r.last().unwrap().clone()
    };
    assert_eq!(pages("V1.0"), "");
    assert_eq!(pages("V2.0"), "101, 106");
    assert_eq!(export(&q, "revision_overview").csv, by_block.csv);
}

/// Golden A with `block` added (a block value no command writes: `params`).
fn golden_with_block(block: ReportBlock) -> Session {
    golden_common::session_with(&golden_common::batch(), |_, records| {
        records.push(Record::ReportBlock(block))
    })
}

/// A `parts_summary` block at folio 960 on folio 901's form with `params` (a block of Golden A's data).
fn parts_block(params: Option<Value>) -> ReportBlock {
    let batch = golden_common::batch();
    let id = golden_common::block(&batch, "901");
    let mut block = batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if b.id == id => Some(b.clone()),
            _ => None,
        })
        .unwrap();
    block.id = Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(0x5306_0001));
    block.first_folio = Some(960);
    block.source_ref = None;
    block.extra.remove("params");
    if let Some(p) = params {
        block.extra.insert("params".to_owned(), p);
    }
    block
}

/// The malformed-parameter notes of a session's derivation of `block`.
fn notes_of(s: &Session, block: Id<ReportBlock>) -> (usize, Vec<DeriveNote>) {
    let mut q = golden_common::plain(s);
    let rows = q.get::<BlockRows>(block).rows.len();
    let notes = q
        .diagnostics()
        .notes()
        .into_iter()
        .filter(|n| matches!(n, DeriveNote::MalformedShim { .. }))
        .collect();
    (rows, notes)
}

/// `params.mode` "assembly" gives the assembly rows; "Assembly", 3 and an undeclared key give the
/// component rows and exactly one note naming the block in words and the key.
#[test]
fn rk5_the_parts_summary_mode_parameter() {
    let component = golden_with_block(parts_block(None));
    let block = parts_block(None).id;
    let (component_rows, notes) = notes_of(&component, block);
    assert!(notes.is_empty(), "{notes:?}");
    let assembly = golden_with_block(parts_block(Some(json!({"mode": "assembly"}))));
    let (assembly_rows, notes) = notes_of(&assembly, block);
    assert!(notes.is_empty(), "{notes:?}");
    assert_ne!(assembly_rows, component_rows, "the assembly rows differ");
    {
        let q = golden_common::plain(&assembly);
        let set = q.get::<BlockRows>(block);
        assert!(set.column("covers").is_some(), "assembly mode's Covers column");
    }
    let cases = [
        (
            json!({"mode": "Assembly"}),
            "report block 960 params.mode",
            "expected \"component\" or \"assembly\", found \"Assembly\"",
        ),
        (
            json!({"mode": 3}),
            "report block 960 params.mode",
            "expected \"component\" or \"assembly\", found 3",
        ),
        (
            json!({"modes": "assembly"}),
            "report block 960 params.modes",
            "`parts_summary` has no parameter of this name",
        ),
        (
            json!(["assembly"]),
            "report block 960 params",
            "expected an object of parameters, found [\"assembly\"]",
        ),
    ];
    for (params, member, detail) in cases {
        let s = golden_with_block(parts_block(Some(params.clone())));
        let (rows, notes) = notes_of(&s, block);
        assert_eq!(rows, component_rows, "{params}: the component rows");
        assert_eq!(
            notes,
            [DeriveNote::MalformedShim {
                member: member.to_owned(),
                detail: detail.to_owned(),
            }],
            "{params}"
        );
        assert_eq!(
            notes[0].to_string(),
            format!("{member} is malformed ({detail}); the default applies")
        );
    }
}

/// RK-5: a long undeclared key and a long value are quoted cut to 40 characters (hostile input). The diagnostics list
/// their notes sorted.
#[test]
fn rk5_a_long_key_or_value_is_quoted_cut() {
    let long = "k".repeat(1000);
    let s = golden_with_block(parts_block(Some(
        json!({ long.clone(): 1, "mode": "x".repeat(1000) }),
    )));
    let (_, notes) = notes_of(&s, parts_block(None).id);
    assert_eq!(
        notes,
        [
            DeriveNote::MalformedShim {
                member: format!("report block 960 params.{}…", "k".repeat(40)),
                detail: "`parts_summary` has no parameter of this name".to_owned(),
            },
            DeriveNote::MalformedShim {
                member: "report block 960 params.mode".to_owned(),
                detail: format!(
                    "expected \"component\" or \"assembly\", found \"{}…",
                    "x".repeat(39)
                ),
            },
        ]
    );
}

/// The form and section of folio 910's block (the house table on `ew.frame.qet-a4-landscape-nogrid`).
fn form_of_910(s: &Session) -> (Value, Value) {
    let q = s.query();
    let b = q
        .report_blocks()
        .get(golden_common::block(&golden_common::batch(), "910"))
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

/// RK-4 (stop 1): `device_list` and `terminal_overview` blocks made by `report_block.create` at folios 912 and 913
/// paginate with the house layout, no placeholder page and no overflow (no cell below the 1.8 mm floor); the TOC
/// lists them; every other page keeps its folio and order.
#[test]
fn rk4_the_list_kinds_paginate_on_the_house_table() {
    let mut s = common::golden_session();
    let before = folios_but(&s.query(), &[]);
    let (form, section) = form_of_910(&s);
    for (kind, folio, title) in [
        ("device_list", 912, "Device List {n}"),
        ("terminal_overview", 913, "Terminal Overview {n}"),
    ] {
        run(
            &mut s,
            "report_block.create",
            json!({"source": {"kind": kind}, "form": form, "section": section, "first_folio": folio,
                "title": title}),
        );
    }
    let q = s.query();
    assert_eq!(folios_but(&q, &["912", "913"]), before);
    let order = q.get::<PageOrder>(());
    let pages: Vec<(String, PageRef)> = order
        .entries
        .iter()
        .filter(|e| {
            ["912", "913"]
                .iter()
                .any(|b| e.folio == *b || e.folio.starts_with(&format!("{b}.")))
        })
        .map(|e| (e.folio.clone(), e.page))
        .collect();
    let folios: Vec<&str> = pages.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(folios, ["912", "912.1", "912.2", "913"]);
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
    let toc = q.get::<BlockRows>(golden_common::block(&golden_common::batch(), "2"));
    let listed: Vec<String> = (0..toc.rows.len())
        .map(|i| row_texts(&q, &toc, i)[0].clone())
        .collect();
    for folio in ["912", "912.1", "912.2", "913"] {
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
