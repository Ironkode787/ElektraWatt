// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.06 part 2 over Golden A: the open-items list (`open_items`, D7, D8). The golden CSV
//! `golden/J-E-2416/expected/csv/open_items.csv` is one of the kind CSVs that `report_kinds.rs` checks.
//!
//! The orchestrator's rulings of cycle 42 these tests rely on, quoted:
//! - Ruling 2: "`open_items` calls `run_rules` ... and keeps the findings that are `FindingState::Open` and
//!   `open_item_of` some row".
//! - Ruling 4: "A test in `crates/ew-api/tests/it/` asserts `ew_checks::CHECKS_MEMBER ==
//!   ew_commands::commands::CHECKS_MEMBER`".
//! - Ruling 5: OI-3 "asserts, on Golden A with an `open_items` block, no cycle note, no `NotDerived::Cycle`, and no
//!   EW-CORE-083 finding whose subject names a table rule in the same snapshot's release run".
//! - Ruling 8: "Order: the category's index in `OPEN_ITEM_RULES`, then `natural_key(subject)`, then the fingerprint".
//! - The ruling on stop 1: "Bless `open_items.csv` with the 24 rows Golden A derives as checked in ... OI-1's test
//!   asserts the 24 rows whole, in ruling 8's order, and that EW-PLC-012 and EW-POT-013 are absent. OI-2's waiver case
//!   uses a fixture copy with W-1 recorded and asserts that exactly the -XPE row leaves."

use std::collections::BTreeSet;

use crate::{common, golden_common};

use ew_api::checks::{self, CheckMode, FindingState, PRE_DELIVERY};
use ew_api::output::{RowExport, page_layer, row_export};
use ew_api::{
    AnyCommand, CmdError, NewProjectKind, Origin, ProjectTemplate, Session, SessionOptions, Submit,
};
use ew_derive::ew_ids::Id;
use ew_derive::{DeriveNote, PageOrder, PageRef, Q};
use ew_dl::Layer;
use ew_reports::csv::row_texts;
use ew_reports::{BlockRows, RowKey, RowSet, RowSetStatus, project_rows};
use ew_scene::forms::OverflowKind;
use ew_scene::page::PageFinding;
use ew_scene::table::TableFinding;
use ew_schema::{Record, ReportBlock};
use serde_json::{Value, json};

/// The rows of the open-items kind as printed texts.
fn texts(q: &Q, set: &RowSet) -> Vec<Vec<String>> {
    (0..set.rows.len()).map(|i| row_texts(q, set, i)).collect()
}

fn kind_rows(q: &Q) -> RowSet {
    project_rows(q, "open_items", None).expect("the open-items kind")
}

/// The `(rule, fingerprint)` of each row, from its row ID `name(<rule>/<fingerprint>)`.
fn row_keys(set: &RowSet) -> BTreeSet<(String, String)> {
    set.rows
        .iter()
        .map(|r| match r.id.0.as_slice() {
            [RowKey::Name(n)] => {
                let (rule, fp) = n.split_once('/').expect("rule/fingerprint");
                (rule.to_owned(), fp.to_owned())
            }
            other => panic!("row ID {other:?}"),
        })
        .collect()
}

/// The `(rule, fingerprint)` of the open findings of the open-item rules that `Session::check` reports in release mode
/// of the project's release scheme (else `pre-delivery`), EW-BUS-001 with discriminator `address` only.
fn checked_keys(s: &Session) -> BTreeSet<(String, String)> {
    let scheme = s
        .project_check_scheme(CheckMode::Release)
        .unwrap_or(&PRE_DELIVERY);
    s.check(scheme, CheckMode::Release)
        .findings
        .iter()
        .filter(|f| f.state == FindingState::Open && checks::open_item_of(f).is_some())
        .map(|f| (f.rule.to_owned(), f.fingerprint.to_string()))
        .collect()
}

fn run(session: &mut Session, name: &str, params: Value) {
    let cmd = AnyCommand {
        name: name.into(),
        params: params.clone(),
    };
    match session.execute(&cmd) {
        Err(CmdError::NeedsPreview { .. }) => session.preview(&cmd).and_then(|p| session.commit(p)),
        other => other,
    }
    .unwrap_or_else(|e| panic!("{name} {params}: {e:?}"));
}

fn gui(session: &mut Session, name: &str, params: Value) {
    session
        .execute_with(
            &AnyCommand {
                name: name.into(),
                params: params.clone(),
            },
            Submit::from(Origin::Gui),
        )
        .unwrap_or_else(|e| panic!("{name} {params}: {e:?}"));
}

/// One expected row: category, item, description, rule, severity, page.
fn row(cells: [&str; 6]) -> Vec<String> {
    cells.iter().map(|c| (*c).to_owned()).collect()
}

fn no_part(tag: &str) -> Vec<String> {
    row([
        "Device without part",
        tag,
        &format!("{tag} has no part; assign one, or mark it as supplied by others"),
        "EW-CORE-041",
        "warning",
        "",
    ])
}

fn unplaced(tag: &str) -> Vec<String> {
    row([
        "Unplaced device",
        tag,
        &format!(
            "{tag} is not placed in the layout of +J-E-2416; place it on a rail or on the mounting plate"
        ),
        "EW-LAY-001",
        "error",
        "",
    ])
}

fn placeholder(what: &str, users: &str) -> Vec<String> {
    row([
        "TBD value",
        &format!("TBD {what}"),
        &format!(
            "placeholder part \"{what}\" (part number TBD) is used by {users}; choose the part before release"
        ),
        "EW-CORE-043",
        "error",
        "",
    ])
}

/// OI-1 (the ruling on stop 1): the 24 rows Golden A derives as checked in, whole and in ruling 8's order. -XPE is open
/// (W-1 lives in a test copy only, WP-S1.16 §10.3) and the eight FIELD-203Q valves have no part until FT-209 (FT-638).
#[test]
fn oi1_golden_a_lists_its_24_open_items() {
    let s = common::golden_session();
    let q = s.query();
    let set = kind_rows(&q);
    let unconnected = |cp: &str| {
        row([
            "Unconnected connection point",
            cp,
            &format!("{cp} is not connected; connect it or mark it unused"),
            "EW-POT-009",
            "warning",
            "102",
        ])
    };
    let mut want = vec![
        row([
            "TBD value",
            "=P01+J-E-2416-201U1:DP",
            "=P01+J-E-2416-201U1:DP: the station address is TBD; enter it before release",
            "EW-BUS-001",
            "error",
            "",
        ]),
        placeholder("DIN rail TS 35", "=P01+J-E-2416-UC1"),
        placeholder(
            "End bracket",
            "=P01+J-E-2416-1X1, =P01+J-E-2416-X1031, =P01+J-E-2416-X1032",
        ),
        placeholder("Plug-in bridge", "=P01+J-E-2416-X1031, =P01+J-E-2416-X1032"),
        placeholder("Wiring duct 40\u{d7}60", "=P01+J-E-2416-UC1"),
    ];
    for n in 1..=8 {
        want.push(no_part(&format!("=P01+FIELD-203Q{n}")));
    }
    for t in ["E56", "S56", "XPE"] {
        want.push(no_part(&format!("=P01+J-E-2416-{t}")));
    }
    for t in ["201U9", "E55", "E56", "F55", "S56"] {
        want.push(unplaced(&format!("=P01+J-E-2416-{t}")));
    }
    want.push(unconnected("-102F2:13"));
    want.push(unconnected("-102F2:14"));
    want.push(row([
        "Missing length",
        "=P01+J-E-2416-3W1",
        "=P01+J-E-2416-3W1 has no routed, manual or part length",
        "EW-CORE-033",
        "error",
        "",
    ]));
    assert_eq!(texts(&q, &set), want);
    assert_eq!(
        set.columns.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
        ["category", "item", "description", "rule", "severity", "page"]
    );
    // EW-PLC-012's 28 and EW-POT-013's 2 are open in the release run and not open items.
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    for rule in ["EW-PLC-012", "EW-POT-013"] {
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == rule && f.state == FindingState::Open),
            "{rule} is open in the release run"
        );
        assert!(set.rows.iter().all(|r| !format!("{}", r.id).contains(rule)));
    }
    assert_eq!(row_keys(&set), checked_keys(&s));
}

/// OI-2: on Golden A the rows are exactly the open findings of the open-item rules that `Session::check` reports.
#[test]
fn oi2_golden_a_agrees_with_the_release_check() {
    let s = common::golden_session();
    assert_eq!(row_keys(&kind_rows(&s.query())), checked_keys(&s));
}

/// OI-2: a TBD channel address is listed once, by EW-PLC-015, and EW-CORE-044's finding for it is superseded.
#[test]
fn oi2_a_tbd_channel_address_is_listed_once() {
    let batch = golden_common::batch();
    let mut done = false;
    let s = golden_common::session_with(&batch, |_, records| {
        for r in records.iter_mut() {
            if let Record::Channel(c) = r
                && !done
            {
                c.address = ew_schema::AddrSpec::Tbd;
                done = true;
            }
        }
    });
    let set = kind_rows(&s.query());
    let rules: Vec<String> = row_keys(&set).into_iter().map(|(r, _)| r).collect();
    assert_eq!(rules.iter().filter(|r| *r == "EW-PLC-015").count(), 1);
    assert_eq!(rules.iter().filter(|r| *r == "EW-CORE-044").count(), 0);
    assert_eq!(row_keys(&set), checked_keys(&s));
}

/// OI-2 (the ruling on stop 1): with W-1 recorded on -XPE's EW-CORE-041 finding exactly the -XPE row leaves.
#[test]
fn oi2_the_waived_xpe_row_leaves() {
    let mut s = common::golden_session();
    let before = {
        let q = s.query();
        texts(&q, &kind_rows(&q))
    };
    let xpe = s
        .check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .into_iter()
        .find(|f| f.rule == "EW-CORE-041" && f.subject == "=P01+J-E-2416-XPE")
        .expect("-XPE's open finding");
    gui(
        &mut s,
        "check.waive",
        json!({"rule": "EW-CORE-041", "fingerprint": xpe.fingerprint.to_string(),
               "reason": {"en-GB": "PE stud is part of NSYPMM107"}, "author": "engineer", "date": "2025-04-14"}),
    );
    let q = s.query();
    let set = kind_rows(&q);
    let mut want = before;
    want.retain(|r| r[1] != "=P01+J-E-2416-XPE");
    assert_eq!(texts(&q, &set), want);
    assert_eq!(row_keys(&set), checked_keys(&s));
}

/// OI-2 (ruling 4): with the release scheme `drafting` set by `project.checks.set` the list runs in that scheme.
#[test]
fn oi2_the_projects_release_scheme_applies() {
    let mut s = common::golden_session();
    gui(
        &mut s,
        "project.checks.set",
        json!({"release_scheme": "drafting"}),
    );
    assert_eq!(
        s.project_check_scheme(CheckMode::Release),
        Some(&checks::DRAFTING)
    );
    assert_eq!(checks::release_scheme_of(&s.query()), &checks::DRAFTING);
    assert_eq!(row_keys(&kind_rows(&s.query())), checked_keys(&s));
}

/// Ruling 4: the two copies of the settings member agree.
#[test]
fn the_checks_member_agrees_with_the_command_layer() {
    assert_eq!(checks::CHECKS_MEMBER, ew_commands::commands::CHECKS_MEMBER);
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

/// The report block whose first folio is `folio`.
fn block_at(q: &Q, folio: u32) -> Id<ReportBlock> {
    q.report_blocks()
        .iter()
        .find(|(_, b)| b.first_folio == Some(folio))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no block at {folio}"))
}

/// The generated pages of the block at `folio`, in page order.
fn pages_of(q: &Q, folio: &str) -> Vec<(String, PageRef)> {
    q.get::<PageOrder>(())
        .entries
        .iter()
        .filter(|e| e.folio == folio || e.folio.starts_with(&format!("{folio}.")))
        .map(|e| (e.folio.clone(), e.page))
        .collect()
}

/// The query-cycle notes of a derivation of the page order over the session's snapshot.
fn cycle_notes(s: &Session) -> Vec<DeriveNote> {
    let mut q = s.query();
    let _ = q.get::<PageOrder>(());
    q.diagnostics()
        .notes()
        .into_iter()
        .filter(|n| matches!(n, DeriveNote::QueryCycle { .. }))
        .collect()
}

/// The two descriptions of Golden A that no column split of the house table holds at the 1.8 mm floor (probed: 184.95
/// and 164.31 mm with the cell padding; the other five columns need 102.78 mm at the floor, the table is 260.96 mm):
/// the only overflowing cells, shrunk to the floor and reported (`ShrinkLimit`), never clipped.
const TOO_LONG: [&str; 2] = [
    "placeholder part \"End bracket\" (part number TBD) is used by =P01+J-E-2416-1X1, =P01+J-E-2416-X1031, \
     =P01+J-E-2416-X1032; choose the part before release",
    "placeholder part \"Plug-in bridge\" (part number TBD) is used by =P01+J-E-2416-X1031, =P01+J-E-2416-X1032; \
     choose the part before release",
];

/// Ruling 5 for one snapshot: no cycle note, the block's rows derived and paginated without a placeholder page, no
/// overflow but the description cells of [`TOO_LONG`] (each at the floor, `ShrinkLimit`), and no EW-CORE-083 finding
/// naming a table rule in the release run.
fn assert_paginates(s: &Session, folio: &str) -> Vec<Vec<String>> {
    assert_eq!(cycle_notes(s), Vec::<DeriveNote>::new());
    let q = s.query();
    let block = block_at(&q, folio.parse().unwrap());
    let rows = q.get::<BlockRows>(block);
    assert_eq!(rows.status, RowSetStatus::Derived, "no NotDerived::Cycle");
    let pages = pages_of(&q, folio);
    assert!(!pages.is_empty(), "the block has pages");
    let order = q.get::<PageOrder>(());
    let mut overflows: Vec<(usize, String, OverflowKind)> = Vec::new();
    for (f, page) in &pages {
        assert!(
            order.generated(*page).is_some_and(|g| g.note.is_none()),
            "folio {f}: a placeholder page"
        );
        let layer = page_layer(&q, *page, Layer::Tables);
        overflows.extend(layer.findings.iter().filter_map(|f| match f {
            PageFinding::Table(TableFinding::Overflow {
                column, text, kind, ..
            }) => Some((*column, text.clone(), *kind)),
            _ => None,
        }));
    }
    let want: Vec<(usize, String, OverflowKind)> = TOO_LONG
        .iter()
        .map(|t| (2, (*t).to_owned(), OverflowKind::ShrinkLimit))
        .collect();
    assert_eq!(overflows, want, "only the descriptions too long for the table");
    let caught: Vec<String> = s
        .check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-083" && checks::OPEN_ITEM_RULES.iter().any(|o| f.subject == o.rule))
        .map(|f| f.message.clone())
        .collect();
    assert_eq!(caught, Vec::<String>::new());
    texts(&q, &rows)
}

/// A TBD placeholder part copy of Golden A's (a part numbered `TBD`) as a new copy described `what`.
fn new_placeholder(q: &Q, what: &str) -> ew_schema::LibCopy<ew_schema::Part> {
    let (_, held) = q
        .table::<ew_schema::LibCopy<ew_schema::Part>>()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "TBD")
        .expect("a placeholder part copy");
    let mut copy = ew_schema::LibCopy::clone(held);
    let item = ew_derive::ew_ids::Uuid::from_u128(0x5306_0002_0000_4000_8000_0000_0000_0001);
    copy.item_id = Id::from_uuid(item);
    // A test library of its own: `part.assign` embeds copies of other libraries only.
    copy.library_id = Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(
        0x5306_0002_0000_4000_8000_0000_0000_0002,
    ));
    copy.id = ew_derive::ew_ids::v5::lib_copy_id(copy.library_id.as_uuid(), item);
    copy.item.meta.key = Some("OI3-TBD".parse().expect("a key"));
    copy.item.description = ew_derive::ew_mltext::MlText::neutral(what);
    let doc = ew_schema::LibDocument::Part(copy.clone());
    let hash = ew_commands::commands::library_copy_hash(&doc).expect("a part hashes");
    copy.sha256 = ew_schema::Sha256(hash);
    copy
}

/// OI-3 (ruling 5, feasibility item 4): an `open_items` block at folio 914 paginates inside the page order with no
/// cycle; a change of the page order (a new block before it, a drawn page reorder) leaves its rows as they are; -102G1
/// given a TBD part adds one "TBD value" row and the block's pages follow in the same snapshot.
#[test]
fn oi3_the_open_items_block_paginates_inside_the_page_order() {
    let mut s = common::golden_session();
    let (form, section) = form_of_910(&s);
    run(
        &mut s,
        "report_block.create",
        json!({"source": {"kind": "open_items"}, "form": form, "section": section, "first_folio": 914,
               "title": "Open Items {n}", "empty": {"one_page": "No entries"}}),
    );
    let rows = assert_paginates(&s, "914");
    {
        let q = s.query();
        assert_eq!(rows, texts(&q, &kind_rows(&q)), "the block lists the kind's rows");
    }
    let csv = |s: &Session| {
        row_export(&s.query(), &RowExport::Kind("open_items".to_owned()), None)
            .expect("the export by kind")
            .csv
    };
    let csv_before = csv(&s);

    // A block before it moves the page order: the list's bytes stay.
    run(
        &mut s,
        "report_block.create",
        json!({"source": {"kind": "device_list"}, "form": form, "section": section, "first_folio": 912,
               "title": "Device List {n}"}),
    );
    assert_eq!(assert_paginates(&s, "914"), rows);
    assert_eq!(csv(&s), csv_before);

    // A drawn page reorder: the same open items (rule and fingerprint), whatever tags and folios now print.
    let keys_before = row_keys(&kind_rows(&s.query()));
    let (moved, before) = {
        let q = s.query();
        let drawn = |f: &str| -> Value {
            let page = q
                .get::<PageOrder>(())
                .entries
                .iter()
                .find(|e| e.folio == f)
                .unwrap_or_else(|| panic!("folio {f}"))
                .page;
            match page {
                PageRef::Drawn(id) => json!(id),
                other => panic!("folio {f}: {other:?}"),
            }
        };
        (drawn("205"), drawn("204"))
    };
    run(
        &mut s,
        "page.reorder",
        json!({"pages": [moved], "before": before}),
    );
    assert_paginates(&s, "914");
    assert_eq!(row_keys(&kind_rows(&s.query())), keys_before);
    assert_eq!(row_keys(&kind_rows(&s.query())), checked_keys(&s));

    // -102G1 given a new placeholder part: one more "TBD value" row, the block's pages follow.
    let rows_before = assert_paginates(&s, "914");
    let copy = new_placeholder(&s.query(), "Power supply 24 V");
    let g1 = *golden_common::tags(&s)
        .iter()
        .find(|(_, t)| t.ends_with("-102G1"))
        .expect("-102G1")
        .0;
    run(
        &mut s,
        "part.assign",
        json!({"devices": [g1], "part": copy.id,
               "copies": [serde_json::to_value(ew_schema::LibDocument::Part(copy.clone())).unwrap()]}),
    );
    let rows_after = assert_paginates(&s, "914");
    let added: Vec<&Vec<String>> = rows_after.iter().filter(|r| !rows_before.contains(r)).collect();
    assert_eq!(
        added,
        [&row([
            "TBD value",
            "TBD Power supply 24 V",
            "placeholder part \"Power supply 24 V\" (part number TBD) is used by =P01+J-E-2416-102G1; choose the \
             part before release",
            "EW-CORE-043",
            "error",
            "",
        ])]
    );
    // Ruling 8's order: in its category, by the natural order of the item ("TBD Plug-in bridge" < "TBD Power supply
    // 24 V" < "TBD Wiring duct 40×60"); every other row stays as it was.
    assert_eq!(rows_after[4][1], "TBD Power supply 24 V");
    let mut rest = rows_after.clone();
    rest.remove(4);
    assert_eq!(rest, rows_before);
}

/// OI-3 (doc 09 §7 item 4): a project without open items gives no rows, and the block with `empty` "No entries"
/// prints that one page.
#[test]
fn oi3_no_open_items_print_the_empty_page() {
    let mut s = Session::new_project(
        ProjectTemplate::Builtin,
        NewProjectKind::Schematic,
        SessionOptions::default(),
    )
    .expect("the built-in basic project");
    {
        let q = s.query();
        assert_eq!(kind_rows(&q).rows.len(), 0, "no open items");
    }
    let parse = |text: &str| serde_json::from_str::<Value>(text).expect("a shipped form parses");
    let repo = golden_common::repo();
    let nogrid = parse(
        &std::fs::read_to_string(repo.join("content/forms/ew.frame.qet-a4-landscape-nogrid.json")).unwrap(),
    );
    let title_block =
        parse(&std::fs::read_to_string(repo.join("content/forms/ew.titleblock.qet-a4.json")).unwrap());
    run(
        &mut s,
        "report_block.create",
        json!({"source": {"kind": "open_items"}, "form": nogrid["id"], "form_copies": [nogrid, title_block],
               "first_folio": 914, "title": "Open Items {n}", "empty": {"one_page": "No entries"}}),
    );
    assert_eq!(cycle_notes(&s), Vec::<DeriveNote>::new());
    let q = s.query();
    let pages = pages_of(&q, "914");
    assert_eq!(pages.len(), 1, "{pages:?}");
    let order = q.get::<PageOrder>(());
    let note = order
        .generated(pages[0].1)
        .and_then(|g| g.note.clone())
        .expect("the empty page's note");
    assert_eq!(note.as_neutral(), Some("No entries"));
}
