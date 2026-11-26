// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop lists on the sample (doc 13 §6, §9.3): a strip whose end plate was left off is EW-MFG-020 and its fix
//! adds one back, a bridge without a part is EW-MFG-021 and prints TBD, the cut increment rounds each piece once, and a
//! `tube_cut_list` block prints the cut list's Tubes group, which Report block… no longer offers; a wire part's rows
//! among their size's. The lists' rows on the whole sample are the kind CSVs of `report_kinds`.

use crate::common;

use ew_api::output::{RowExport, page_report_kinds, row_export};
use ew_api::{AnyCommand, Session};
use ew_checks::{CheckMode, FULL, Finding};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) {
    let cmd = AnyCommand {
        name: name.into(),
        params: params.clone(),
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .unwrap_or_else(|e| panic!("{name} {params}: {e:?}"));
}

/// The CSV lines of the export by kind `kind`, without the line ending.
fn lines(s: &Session, kind: &str) -> Vec<String> {
    let q = s.query();
    let out =
        row_export(&q, &RowExport::Kind(kind.to_owned()), None).unwrap_or_else(|e| panic!("{kind}: {e}"));
    out.csv.lines().map(str::to_owned).collect()
}

/// The findings of `rule` in `mode`.
fn findings(s: &Session, rule: &str, mode: CheckMode) -> Vec<Finding> {
    let q = s.query();
    ew_checks::run(&q, &FULL, mode)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

/// The ID of the device tagged `tag`.
fn device(s: &Session, tag: &str) -> String {
    s.resolve_reference(&format!("tag:{tag}"))
        .unwrap_or_else(|e| panic!("no device {tag}: {e:?}"))
        .to_string()
}

/// The project copy numbered `number`.
fn copy(s: &Session, number: &str) -> String {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(id, _)| id.to_string())
        .unwrap_or_else(|| panic!("no copy {number}"))
}

/// The rows of `-X1031` in the assembly lists: from its heading to the next heading.
fn x1031(s: &Session) -> Vec<String> {
    lines(s, "assembly_list")
        .into_iter()
        .skip_while(|l| !l.starts_with("heading,-X1031 "))
        .skip(1)
        .take_while(|l| !l.starts_with("heading,"))
        .collect()
}

/// EW-MFG-020: -X1031 with its end plate left off (a choice without a part) has none after SH; the quick fix adds the
/// end plate its terminals' parts name as the strip's own after SH, which the assembly list notes as added.
#[test]
fn a_strip_whose_end_plate_was_left_off_is_ew_mfg_020_and_its_fix_adds_one() {
    let mut s = common::sample();
    assert!(
        findings(&s, "EW-MFG-020", CheckMode::Online).is_empty(),
        "silent on the sample"
    );
    let strip = device(&s, "-X1031");
    let plate = copy(&s, "EW-GEN-EP-1");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [strip], "path": "accessory_choices", "value": [{"of": plate}]}),
    );
    assert!(
        !x1031(&s).iter().any(|l| l.contains("EW-GEN-EP-1")),
        "the end plate is off: {:?}",
        x1031(&s)
    );
    let found = findings(&s, "EW-MFG-020", CheckMode::Online);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0]
            .message()
            .ends_with("-X1031 has no end plate after its last terminal SH"),
        "{}",
        found[0].message()
    );
    let fix = found[0].fix.clone().expect("a quick fix");
    assert_eq!(
        ew_checks::messages::fix_label(&fix, "en-GB"),
        "Add the end plate EW-GEN-EP-1 after SH"
    );
    run(&mut s, &fix.command, fix.params);
    assert!(findings(&s, "EW-MFG-020", CheckMode::Online).is_empty());
    let rows = x1031(&s);
    assert!(
        rows.iter()
            .any(|l| l.starts_with("data,7,End plate,,EW-GEN-EP-1,") && l.ends_with(",added")),
        "{rows:#?}"
    );
}

/// EW-MFG-021: BR1 of -X1021 without its part is a warning online and an error in release, and the assembly list and
/// the wire list print TBD for it.
#[test]
fn a_bridge_without_a_part_is_ew_mfg_021_and_prints_tbd() {
    let mut s = common::sample();
    assert!(
        findings(&s, "EW-MFG-021", CheckMode::Release).is_empty(),
        "silent on the sample"
    );
    let carrier = {
        let q = s.query();
        let (_, _, g) = ew_derive::bridge_groups(&q)
            .into_iter()
            .find(|(label, _, _)| label == "BR1")
            .expect("BR1");
        g.carrier.to_string()
    };
    run(
        &mut s,
        "fields.set",
        json!({"kind": "connection", "targets": [carrier], "path": "overrides.wire_part", "value": null}),
    );
    let online = findings(&s, "EW-MFG-021", CheckMode::Online);
    assert_eq!(online.len(), 1, "{online:#?}");
    assert_eq!(online[0].severity, ew_checks::Severity::Warning);
    assert!(
        online[0].message().starts_with("The bridge BR1 of ")
            && online[0]
                .message()
                .ends_with("-X1021 (terminals 1 to 4) has no part"),
        "{}",
        online[0].message()
    );
    let release = findings(&s, "EW-MFG-021", CheckMode::Release);
    assert_eq!(release[0].severity, ew_checks::Severity::Error);
    assert!(
        lines(&s, "assembly_list")
            .iter()
            .any(|l| l.starts_with("data,,Bridge BR1,1 to 4,TBD,")),
        "the assembly list"
    );
    assert!(
        lines(&s, "wire_list")
            .iter()
            .any(|l| l.starts_with("data,BR1,") && l.contains(",TBD,")),
        "the wire list"
    );
}

/// Cut to 25 mm: the 470 mm rails are cut to 475 mm, in the cut list and in the assembly list's heading, and their
/// total is the sum of the rounded pieces; a wire keeps the length the wire list prints.
#[test]
fn the_cut_increment_rounds_each_piece_once() {
    let mut s = common::sample();
    let stock = serde_json::to_value(&s.query().project().settings.workshop.stock).unwrap();
    run(
        &mut s,
        "workshop.stock",
        json!({"stock": stock, "cut_increment": 25_000_000}),
    );
    let cut = lines(&s, "cut_list");
    assert!(
        cut.iter()
            .any(|l| l.starts_with("data,EW-GEN-RAIL-1,") && l.ends_with(",2,475,0.95")),
        "{cut:#?}"
    );
    assert!(cut.iter().any(|l| l.contains(",101.1,1,400,0.4")), "{cut:#?}");
    assert!(
        lines(&s, "assembly_list")
            .iter()
            .any(|l| l.starts_with("heading,-X1011 · on rail 1 from 0 to 48.2 mm · rail cut to 475 mm · ")),
        "the heading"
    );
}

/// A `tube_cut_list` block lists the cut list's Tubes group with the cut list's columns; Report block… offers the three
/// workshop lists and not the tube cut list.
#[test]
fn the_tube_cut_list_is_the_cut_lists_tubes_group() {
    let s = common::sample();
    let cut = lines(&s, "cut_list");
    let tubes: Vec<String> = cut
        .iter()
        .skip_while(|l| !l.starts_with("heading,Tubes,"))
        .skip(1)
        .cloned()
        .collect();
    assert!(!tubes.is_empty(), "the sample has tubes");
    let old = lines(&s, "tube_cut_list");
    assert_eq!(old[0], cut[0], "the cut list's columns");
    assert_eq!(old[1..], tubes[..]);
    let offered = page_report_kinds();
    assert!(!offered.contains(&"tube_cut_list"), "{offered:?}");
    for kind in ["assembly_list", "cut_list", "consumables"] {
        assert!(offered.contains(&kind), "{kind}: {offered:?}");
    }
}

/// The sample with two tubes of 1001 mm each (single connections) and a `tube_cut_list` block at folio 960 as a
/// project of 0.3.5 stored it: the old column choice (with a heading of its own on the part), a sort by the old count,
/// a row of one's own under the old keys and the first of the two tubes left out by hand (its row's identity in 0.3.5).
/// Returns the session, the block and the tube left out.
fn sample_with_old_tube_cut_list() -> (Session, String, String) {
    use ew_schema::{ConnectionKind, LengthSource, Record};
    let (project, mut records, lib) = common::sample_records();
    let (two, left_out) = {
        let s = common::session_of(project.clone(), records.clone(), lib.clone());
        let q = s.query();
        let tubes = q.get::<ew_derive::TubeRows>(());
        let single: Vec<_> = tubes.iter().filter(|t| t.constituents.len() == 1).collect();
        assert!(single.len() >= 3, "the sample has tubes of one connection each");
        (
            [single[0].constituents[0], single[1].constituents[0]],
            single[0].id.to_string(),
        )
    };
    for r in &mut records {
        if let Record::Connection(c) = r
            && two.contains(&c.id)
            && let ConnectionKind::Fluid(l) = &mut c.kind
        {
            l.length = LengthSource::Manual(ew_units::Nm::from_mm(1001));
        }
    }
    let wire_list = records
        .iter()
        .find_map(|r| match r {
            Record::ReportBlock(b) if b.first_folio == Some(907) => Some(b.clone()),
            _ => None,
        })
        .expect("the wire list block");
    let mut block = serde_json::to_value(&wire_list).unwrap();
    block["id"] = json!(ew_ids::Uuid::from_u128(0x5050_0001).to_string());
    block["source"] = json!({"kind": "tube_cut_list"});
    block["first_folio"] = json!(960);
    block["title"] = json!({"en-GB": "Tube cut list"});
    block["columns"] = json!([
        {"key": "material", "weight": 10}, {"key": "size", "weight": 10}, {"key": "colour", "weight": 8},
        {"key": "part", "weight": 16, "heading": {"en-GB": "Tube"}}, {"key": "count", "weight": 8},
        {"key": "length", "weight": 12}
    ]);
    block["sort"] = json!([{"key": "count"}]);
    block["manual"] = json!({"exclude": [left_out]});
    block["added_rows"] = json!([{"cells": {"material": "PA", "size": "8×6", "part": "EW-OWN-1", "count": "2",
        "length": "3"}}]);
    let block: ew_schema::ReportBlock = serde_json::from_value(block).expect("a report block");
    let id = block.id.to_string();
    records.push(Record::ReportBlock(block));
    (common::session_of(project, records, lib), id, left_out)
}

/// A `tube_cut_list` block of 0.3.5 reads its stored keys as the cut list's columns at one place: it prints Description
/// (material, size and colour, once, as wide as they were), Part number under its own heading "Tube", Pieces and Total;
/// it sorts by the pieces (the row of one's own, 2, before the 5 tubes without a length, the kind's order reversed); its
/// row of one's own keeps its texts; the row of the tube left out stays out; List columns… offers the new keys. The two
/// tubes of 1001 mm are two pieces of 1005 mm, 2.01 m in the cut list: each piece rounded once.
#[test]
fn an_old_tube_cut_list_block_reads_its_stored_keys_as_the_new_columns() {
    let (s, block, left_out) = sample_with_old_tube_cut_list();
    let printed = s.page_list_table("960").expect("folio 960 prints a list");
    assert_eq!(printed.columns, ["Description", "Tube", "Pieces", "Total m"]);
    let rows: Vec<Vec<String>> = printed.rows.iter().map(|r| r.cells.clone()).collect();
    assert_eq!(
        rows,
        [
            ["PA 8×6", "EW-OWN-1", "2", "3"],
            ["Tube PU 6 × 4 mm, blue", "EW-GEN-TUB-6", "5", ""],
        ],
        "sorted by the pieces; the row of the tube left out is out"
    );
    assert!(
        lines(&s, "cut_list")
            .iter()
            .any(|l| l.starts_with("data,EW-GEN-TUB-6,") && l.ends_with(",2,1005,2.01")),
        "two pieces of 1001 mm cut to 1005 mm"
    );
    let q = s.query();
    let set = q.get::<ew_reports::BlockRows>(ew_ids::Id::from_uuid(block.parse().unwrap()));
    assert!(
        set.rows
            .iter()
            .all(|r| !r.id.entities().any(|e| e.to_string() == left_out)),
        "the tube left out"
    );
    drop(q);
    let look = s
        .block_look(block.parse().unwrap(), "en-GB")
        .expect("the block's look");
    let shown: Vec<(&str, u32, &str)> = look
        .columns
        .iter()
        .filter(|c| c.shown)
        .map(|c| (c.key.as_str(), c.width, c.heading.as_str()))
        .collect();
    assert_eq!(
        shown,
        [
            ("description", 28, ""),
            ("part_number", 16, "Tube"),
            ("pieces", 8, ""),
            ("total", 12, "")
        ]
    );
    assert_eq!(
        look.added,
        [vec![
            ("part_number".to_owned(), "EW-OWN-1".to_owned()),
            ("description".to_owned(), "PA 8×6".to_owned()),
            ("pieces".to_owned(), "2".to_owned()),
            ("total".to_owned(), "3".to_owned()),
        ]]
    );
}

/// Every row of the cut list names what it is cut for: each rail and duct row its carriers, a cover row its duct's, a
/// cable row its cable, a wire row its wires, a tube row its tubes; the row is its first record's.
#[test]
fn every_cut_list_row_names_what_it_is_cut_for() {
    let s = common::sample();
    let q = s.query();
    let set = ew_reports::project_rows(&q, &ew_schema::ReportKind::CutList, None).expect("the cut list");
    for (i, r) in set.rows.iter().enumerate() {
        if r.kind != ew_reports::RowKind::Data {
            continue;
        }
        assert!(!r.sources.is_empty(), "row {i} names its records");
        let first = r.id.entities().next().expect("the row is a record's");
        assert_eq!(first, r.sources[0].as_uuid(), "row {i}");
    }
    let tubes = q.get::<ew_derive::TubeRows>(()).len();
    let named: usize = set
        .rows
        .iter()
        .filter(|r| {
            r.sources
                .iter()
                .any(|s| q.get::<ew_derive::TubeRows>(()).iter().any(|t| t.id.cast() == *s))
        })
        .map(|r| r.sources.len())
        .sum();
    assert_eq!(named, tubes, "every tube on one row");
}

/// A wire part in the stock: the 1.5 mm² BK wires take it, and their rows in the cut list and in the consumables stand
/// among the rows of their size and colour (after 0.75 mm², before 1.5 mm² BU), not before every size.
#[test]
fn a_wire_part_row_stands_among_the_rows_of_its_size() {
    let mut s = common::sample();
    let (part, doc) = common::wire_part(0x7201, "WIRE-1.5-BK", "1.5", &json!(["BK"]));
    let mut stock = serde_json::to_value(&s.query().project().settings.workshop.stock).unwrap();
    stock
        .as_array_mut()
        .expect("the stock")
        .push(json!({"copy": part.to_string()}));
    run(&mut s, "workshop.stock", json!({"stock": stock, "copies": [doc]}));
    for (kind, group) in [("cut_list", "heading,Wires,"), ("consumables", "heading,Wire,")] {
        let rows: Vec<String> = lines(&s, kind)
            .into_iter()
            .skip_while(|l| !l.starts_with(group))
            .skip(1)
            .take_while(|l| !l.starts_with("heading,"))
            .collect();
        let at = |what: &str| {
            rows.iter()
                .position(|l| l.contains(what))
                .unwrap_or_else(|| panic!("{kind}: no {what}: {rows:#?}"))
        };
        let part_row = at("WIRE-1.5-BK");
        assert!(
            at("0.75 mm² GNYE") < part_row && part_row < at("1.5 mm² BU"),
            "{kind}: {rows:#?}"
        );
    }
}
