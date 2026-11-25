// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Over the sample project: the open-items list (`open_items`). The sample has no open item of its own; the tests
//! make four with commands (a TBD station address, a device without a part, an unplaced device, a cable without a
//! length).
//!
//! What these tests rely on, quoted:
//! - "`open_items` calls `run_rules` ... and keeps the findings that are `FindingState::Open` and
//!   `open_item_of` some row".
//! - The release scheme that `project.checks.set` writes is the one the list runs in (so the settings member
//!   of the check layer and of the command layer agree).
//! - "asserts, with an `open_items` block, no cycle note, no `NotDerived::Cycle`, and no
//!   EW-CORE-083 finding whose subject names a table rule in the same snapshot's release run".
//! - "Order: the category's index in `OPEN_ITEM_RULES`, then `natural_key(subject)`, then the fingerprint".

use std::collections::BTreeSet;

use crate::common;

use ew_api::checks::{self, CheckMode, FindingState, PRE_DELIVERY};
use ew_api::output::{RowExport, page_layer, row_export};
use ew_api::{
    AnyCommand, CmdError, NewProjectKind, Origin, ProjectTemplate, Session, SessionOptions, Submit,
};
use ew_derive::{PageOrder, PageRef, Q};
use ew_dl::Layer;
use ew_ids::Id;
use ew_reports::csv::row_texts;
use ew_reports::{BlockRows, RowKey, RowSet, RowSetStatus, project_rows};
use ew_scene::forms::OverflowKind;
use ew_scene::page::PageFinding;
use ew_scene::table::TableFinding;
use ew_schema::{Record, ReportBlock, ReportKind};
use serde_json::{Value, json};

/// The rows of the open-items kind as printed texts.
fn texts(q: &Q, set: &RowSet) -> Vec<Vec<String>> {
    (0..set.rows.len()).map(|i| row_texts(q, set, i)).collect()
}

fn kind_rows(q: &Q) -> RowSet {
    project_rows(q, &ReportKind::OpenItems, None).expect("the open-items kind")
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
        &format!(
            "{tag} has no part, so the parts list cannot order it. Choose one in its Properties (Part)."
        ),
        "EW-CORE-041",
        "warning",
        "",
    ])
}

fn unplaced(tag: &str) -> Vec<String> {
    row([
        "Unplaced device",
        tag,
        &format!("{tag} is not placed in the layout of +CAB; place it on a rail or on the mounting plate"),
        "EW-LAY-001",
        "error",
        "",
    ])
}

/// The device printed as `tag`.
fn device(s: &Session, tag: &str) -> String {
    s.resolve_reference(tag).expect(tag).to_string()
}

/// The sample with four open items made by commands: the valve terminal's station address TBD, the lamp -102E1
/// without its part, the breaker -102F1 taken off its rail, the supply cable -101W1 without its length.
fn with_open_items() -> Session {
    let mut s = common::sample();
    let station = {
        let q = s.query();
        let vtm = ew_ids::Id::from_uuid(s.resolve_reference("-201K1").unwrap());
        q.idx()
            .functions_of_device(vtm)
            .find(|f| q.functions().get(*f).is_some_and(|f| f.bus_port.is_some()))
            .expect("the valve terminal's port")
            .to_string()
    };
    run(
        &mut s,
        "fields.set",
        json!({"kind": "function", "targets": [station], "path": "bus_port.address", "value": "tbd"}),
    );
    let lamp = device(&s, "-102E1");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [lamp], "path": "parts", "value": []}),
    );
    let placement = {
        let q = s.query();
        let f1 = s.resolve_reference("-102F1").unwrap();
        q.table::<ew_schema::Placement>()
            .iter()
            .find(|(_, p)| p.device.as_uuid() == f1)
            .map(|(id, _)| id.to_string())
            .expect("-102F1's placement")
    };
    run(&mut s, "layout.remove", json!({"placement": placement}));
    let cable = device(&s, "-101W1");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [cable], "path": "cable.length", "value": null}),
    );
    s
}

/// The four rows, whole and in order: by category, then by item.
#[test]
fn the_sample_lists_its_open_items() {
    let s = with_open_items();
    let q = s.query();
    let set = kind_rows(&q);
    let want = vec![
        row([
            "TBD value",
            "+CAB-201K1:DP",
            "+CAB-201K1:DP: the station address is TBD; enter it before release",
            "EW-BUS-001",
            "error",
            "",
        ]),
        no_part("+CAB-102E1"),
        unplaced("+CAB-102F1"),
        row([
            "Missing length",
            "-101W1",
            "-101W1 has no length yet: type it in its Properties, or route the cable in the layout",
            "EW-CORE-033",
            "error",
            "",
        ]),
    ];
    assert_eq!(texts(&q, &set), want);
    assert_eq!(
        set.columns.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
        ["category", "item", "description", "rule", "severity", "page"]
    );
    assert_eq!(row_keys(&set), checked_keys(&s));
    // The unchanged sample has none.
    assert_eq!(kind_rows(&common::sample().query()).rows.len(), 0);
}

/// A TBD channel address is listed once, by EW-PLC-015, and EW-CORE-044's finding for it is superseded.
#[test]
fn a_tbd_channel_address_is_listed_once() {
    let (project, mut records, lib) = common::sample_records();
    let mut done = false;
    for r in records.iter_mut() {
        if let Record::Channel(c) = r
            && !done
        {
            c.address = ew_schema::AddrSpec::Tbd;
            done = true;
        }
    }
    let s = common::session_of(project, records, lib);
    let set = kind_rows(&s.query());
    let rules: Vec<String> = row_keys(&set).into_iter().map(|(r, _)| r).collect();
    assert_eq!(rules.iter().filter(|r| *r == "EW-PLC-015").count(), 1);
    assert_eq!(rules.iter().filter(|r| *r == "EW-CORE-044").count(), 0);
    assert_eq!(row_keys(&set), checked_keys(&s));
}

/// With a waiver recorded on -102E1's EW-CORE-041 finding exactly the -102E1 row leaves.
#[test]
fn the_waived_row_leaves() {
    let mut s = with_open_items();
    let before = {
        let q = s.query();
        texts(&q, &kind_rows(&q))
    };
    let lamp = s
        .check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .into_iter()
        .find(|f| f.rule == "EW-CORE-041" && f.subject == "+CAB-102E1")
        .expect("-102E1's open finding");
    gui(
        &mut s,
        "check.waive",
        json!({"rule": "EW-CORE-041", "fingerprint": lamp.fingerprint.to_string(),
               "reason": {"en-GB": "Supplied with the machine"}, "author": "engineer", "date": "2025-04-14"}),
    );
    let q = s.query();
    let set = kind_rows(&q);
    let mut want = before;
    want.retain(|r| r[1] != "+CAB-102E1");
    assert_eq!(texts(&q, &set), want);
    assert_eq!(row_keys(&set), checked_keys(&s));
}

/// With the release scheme `drafting` set by `project.checks.set` the list runs in that scheme.
#[test]
fn the_projects_release_scheme_applies() {
    let mut s = with_open_items();
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

/// The form and section of folio 910's block (the house table on `ew.frame.qet-a4-landscape-nogrid`).
fn form_of_910(s: &Session) -> (Value, Value) {
    let q = s.query();
    let b = q
        .report_blocks()
        .get(block_at(&q, 910))
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

/// Ruling 5 for one snapshot: no cycle note, the block's rows derived and paginated without a placeholder page, no
/// overflow (the sample's descriptions fit the house table), and no EW-CORE-083 finding naming a table rule in the
/// release run.
fn assert_paginates(s: &Session, folio: &str) -> Vec<Vec<String>> {
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
    assert_eq!(
        overflows,
        Vec::<(usize, String, OverflowKind)>::new(),
        "no overflow"
    );
    let caught: Vec<String> = s
        .check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-083" && checks::OPEN_ITEM_RULES.iter().any(|o| f.subject == o.rule.id))
        .map(|f| f.message().clone())
        .collect();
    assert_eq!(caught, Vec::<String>::new());
    texts(&q, &rows)
}

/// A TBD placeholder part copy (a part numbered `TBD`), made from the sample's power supply part, as a new copy
/// described `what`.
fn new_placeholder(q: &Q, what: &str) -> ew_schema::LibCopy<ew_schema::Part> {
    let (_, held) = q
        .table::<ew_schema::LibCopy<ew_schema::Part>>()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "EW-GEN-PSU-1")
        .expect("the power supply's part copy");
    let mut copy = ew_schema::LibCopy::clone(held);
    copy.item.part_number = "TBD".parse().expect("a part number");
    let item = ew_ids::Uuid::from_u128(0x5306_0002_0000_4000_8000_0000_0000_0001);
    copy.item_id = Id::from_uuid(item);
    // A test library of its own: `part.assign` embeds copies of other libraries only.
    copy.library_id = Id::from_uuid(ew_ids::Uuid::from_u128(0x5306_0002_0000_4000_8000_0000_0000_0002));
    copy.id = ew_ids::v5::lib_copy_id(copy.library_id.as_uuid(), item);
    copy.item.meta.key = Some("OI3-TBD".parse().expect("a key"));
    copy.item.description = ew_mltext::MlText::neutral(what);
    let doc = ew_schema::LibDocument::Part(copy.clone());
    let hash = ew_format::library_copy_hash(&doc).expect("a part hashes");
    copy.sha256 = ew_schema::Sha256(hash);
    copy
}

/// An `open_items` block at folio 914 paginates inside the page order with no
/// cycle; a change of the page order (a new block before it, a drawn page reorder) leaves its rows as they are; -101G1
/// given a TBD part adds one "TBD value" row and the block's pages follow in the same snapshot.
#[test]
fn the_open_items_block_paginates_inside_the_page_order() {
    let mut s = with_open_items();
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
        (drawn("103"), drawn("102"))
    };
    run(
        &mut s,
        "page.reorder",
        json!({"pages": [moved], "before": before}),
    );
    assert_paginates(&s, "914");
    assert_eq!(row_keys(&kind_rows(&s.query())), keys_before);
    assert_eq!(row_keys(&kind_rows(&s.query())), checked_keys(&s));

    // -101G1 given a new placeholder part: one more "TBD value" row, the block's pages follow.
    let rows_before = assert_paginates(&s, "914");
    let copy = new_placeholder(&s.query(), "Power supply 24 V");
    let g1 = s.resolve_reference("-101G1").expect("the power supply");
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
            "placeholder part \"Power supply 24 V\" (part number TBD) is used by +CAB-101G1; choose the part before \
             release",
            "EW-CORE-043",
            "error",
            "",
        ])]
    );
    // Ruling 8's order: in its category, by the natural order of the item ("+CAB-201K1:DP" < "TBD Power supply
    // 24 V"); every other row stays as it was.
    assert_eq!(rows_after[1][1], "TBD Power supply 24 V");
    let mut rest = rows_after.clone();
    rest.remove(1);
    assert_eq!(rest, rows_before);
}

/// A project without open items gives no rows, and the block with `empty` "No entries"
/// prints that one page.
#[test]
fn no_open_items_print_the_empty_page() {
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
    let repo = common::repo();

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
