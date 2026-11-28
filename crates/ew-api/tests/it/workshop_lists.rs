// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop lists on the sample (doc 13 §6, §9.3): a strip whose end plate was left off is EW-MFG-020 and its fix
//! adds one back, a bridge without a part is EW-MFG-021 and prints TBD, the cut increment rounds each piece once, and a
//! `tube_cut_list` block prints the cut list's Tubes group, which Report block… no longer offers; the assembly heading
//! of a strip against another's bracket, turned, on no rail and of unknown width; a wire part's rows among their size's.
//! The lists' rows on the whole sample are the kind CSVs of `report_kinds`.

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
            .any(|l| l.starts_with("data,7,End plate,,,EW-GEN-EP-1,") && l.ends_with(",added")),
        "{rows:#?}"
    );
}

/// EW-MFG-021: BR1 of -X1021 without its part is a warning, online and in release (a project made before a bridge
/// could have a part releases as it did), and the assembly list and the wire list print TBD for it.
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
    assert_eq!(
        release[0].severity,
        ew_checks::Severity::Warning,
        "it refuses no release"
    );
    assert!(
        lines(&s, "assembly_list")
            .iter()
            .any(|l| l.starts_with("data,,Bridge BR1,,1 to 4,TBD,")),
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

/// The sample's records changed by `change` (given the IDs of -X1011, -X1021 and -X1031), as a session.
fn sample_changed(
    change: impl FnOnce(&mut Vec<ew_schema::Record>, [ew_ids::Id<ew_schema::Device>; 3]),
) -> Session {
    let (project, mut records, lib) = common::sample_records();
    let strips = {
        let s = common::session_of(project.clone(), records.clone(), lib.clone());
        ["-X1011", "-X1021", "-X1031"].map(|t| device(&s, t).parse().expect("an ID"))
    };
    change(&mut records, strips);
    common::session_of(project, records, lib)
}

/// The rows of the strip `tag` in the assembly lists: its heading, then its items up to the next heading.
fn strip_rows(s: &Session, tag: &str) -> Vec<String> {
    let all = lines(s, "assembly_list");
    let at = all
        .iter()
        // A heading with a comma in it stands in quotes.
        .position(|l| {
            l.starts_with(&format!("heading,{tag} ")) || l.starts_with(&format!("heading,\"{tag} "))
        })
        .unwrap_or_else(|| panic!("no heading of {tag}: {all:#?}"));
    let mut rows = vec![all[at].clone()];
    rows.extend(
        all[at + 1..]
            .iter()
            .take_while(|l| !l.starts_with("heading,"))
            .cloned(),
    );
    rows
}

/// The assembly list's heading in each of its forms (doc 13 §6.1): -X1011 put on -X1031's rail right after it starts
/// against -X1031's end bracket and has none of its own before it; -X1021 turned half round on its rail lists its last
/// terminal first; -X1031 taken off its rail (its placement removed) is on no rail with both its end brackets, and with
/// its terminal SH without a part (no width) its length is "at least".
#[test]
fn the_assembly_heading_says_against_turned_on_no_rail_and_at_least() {
    use ew_schema::{CarrierRef, Record};
    let s = sample_changed(|records, [x1011, x1021, x1031]| {
        let rail2 = records
            .iter()
            .find_map(|r| match r {
                Record::Placement(p) if p.device == x1031 => Some(p.carrier.clone()),
                _ => None,
            })
            .expect("-X1031 on a rail");
        // The rail holds the strips alone.
        records.retain(|r| {
            !matches!(r, Record::Placement(p)
                if p.carrier == rail2 && ![x1011, x1021, x1031].contains(&p.device))
        });
        for r in records.iter_mut() {
            let Record::Placement(p) = r else { continue };
            if p.device == x1031 {
                // -X1031 stands with its end brackets from 0 to 48.2 mm.
                p.pose.t[0] = ew_units::Nm::from_mm(10);
            } else if p.device == x1011 {
                p.carrier = rail2.clone();
                p.pose.t[0] = ew_units::Nm::from_um(48_200);
            } else if p.device == x1021 {
                p.pose.q = ew_units::Quat9::new(0, 0, 0, 1_000_000_000).expect("a half turn");
                p.pose.t[0] = ew_units::Nm::from_mm(270);
            }
        }
        assert!(matches!(rail2, CarrierRef::Carrier(_)));
    });
    let x1011 = strip_rows(&s, "-X1011");
    assert!(
        x1011[0].starts_with("heading,-X1011 · on rail ")
            && x1011[0].contains(" · against the end bracket of -X1031 · 5 terminals · "),
        "{x1011:#?}"
    );
    assert!(
        x1011[1].starts_with("data,1,Terminal,,L1,"),
        "no end bracket of its own before it: {x1011:#?}"
    );
    let x1021 = strip_rows(&s, "-X1021");
    assert!(
        x1021[0].contains(" · turned, its first terminal at the right · 8 terminals · "),
        "{x1021:#?}"
    );
    let terminals: Vec<&str> = x1021
        .iter()
        .filter_map(|l| {
            let cells: Vec<&str> = l.split(',').collect();
            (cells.get(2) == Some(&"Terminal")).then(|| cells[4])
        })
        .collect();
    assert_eq!(terminals, ["8", "7", "6", "5", "4", "3", "2", "1"], "{x1021:#?}");

    let s = sample_changed(|records, [_, _, x1031]| {
        records.retain(|r| !matches!(r, Record::Placement(p) if p.device == x1031));
        for r in records.iter_mut() {
            if let Record::Device(d) = r
                && d.aspects
                    .terminal
                    .as_ref()
                    .is_some_and(|t| t.strip == x1031 && t.designation.as_str() == "SH")
            {
                d.parts.clear();
            }
        }
    });
    let x1031 = strip_rows(&s, "-X1031");
    assert_eq!(
        x1031[0],
        "heading,-X1031 · on no rail · 5 terminals · at least 43 mm with its end brackets,,,,,,,,,,",
        "{x1031:#?}"
    );
    assert!(x1031[1].starts_with("data,1,End bracket,"), "{x1031:#?}");
    assert!(
        x1031.last().is_some_and(|l| l.starts_with("data,8,End bracket,")),
        "{x1031:#?}"
    );
    assert!(
        x1031
            .iter()
            .any(|l| l.starts_with("data,7,Terminal,,SH,") && l.contains(",width missing,")),
        "{x1031:#?}"
    );
}

/// A cable of 10001 mm is one piece of 10005 mm in the cut list (cut to 5 mm, once), 10.005 m in total.
#[test]
fn a_cable_with_a_length_is_cut_up_once() {
    use ew_schema::{LengthSource, Record};
    let s = sample_changed(|records, _| {
        let cable = records
            .iter_mut()
            .find_map(|r| match r {
                Record::Device(d) => d
                    .aspects
                    .cable
                    .as_mut()
                    .filter(|c| c.length == LengthSource::Manual(ew_units::Nm::from_mm(10_000))),
                _ => None,
            })
            .expect("a cable of 10 m");
        cable.length = LengthSource::Manual(ew_units::Nm::from_mm(10_001));
    });
    let cut = lines(&s, "cut_list");
    assert!(
        cut.iter()
            .any(|l| l.starts_with("data,EW-GEN-CBL-17,") && l.ends_with(",-101W1,1,10005,10.005")),
        "{cut:#?}"
    );
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

/// With the datum corner at the bottom right, every block of the assembly lists counts from that corner (doc 13 §6.1):
/// a rail from its right end (-X1011, at 0–48.2 mm from the left end of the 470 mm rail, stands at 421.8–470), a strip's
/// own block from the same end as its rail's (its PE terminal first, L1 last), and the door's devices from the door's
/// right edge (-101Q1's handle, at X 137.5 of the 600 mm door from the left, at X 462.5).
#[test]
fn the_assembly_lists_count_from_a_datum_corner_on_the_right() {
    let (mut project, records, lib) = common::sample_records();
    project.settings.workshop.datum = Some(ew_schema::Corner::BottomRight);
    let s = common::session_of(project, records, lib);
    let all = lines(&s, "assembly_list");
    let row = |starts: &str| {
        all.iter()
            .find(|l| l.starts_with(starts))
            .unwrap_or_else(|| panic!("no {starts}: {all:#?}"))
            .clone()
    };
    assert!(
        row("heading,\"Rail 1 ").contains("positions from its right end"),
        "{all:#?}"
    );
    let strip = all
        .iter()
        .find(|l| l.contains(",Strip,-X1011,"))
        .expect("-X1011 on its rail");
    assert!(strip.contains(",421.8–470,"), "{strip}");
    assert!(
        row("heading,-X1011 ").contains("on rail 1 from 421.8 to 470 mm"),
        "{all:#?}"
    );
    let terminals: Vec<String> = all
        .iter()
        .skip_while(|l| !l.starts_with("heading,-X1011 "))
        .skip(1)
        .take_while(|l| !l.starts_with("heading,"))
        .filter(|l| l.contains(",Terminal,"))
        .map(|l| l.split(',').nth(4).unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        (
            terminals.first().map(String::as_str),
            terminals.last().map(String::as_str)
        ),
        (Some("PE"), Some("L1")),
        "{terminals:?}"
    );
    let handle = all
        .iter()
        .find(|l| l.contains(",Device,-101Q1,") && l.contains("EW-GEN-HDL-1"))
        .expect("the door's handle");
    assert!(handle.contains("X 462.5, Y 622.5"), "{handle}");
}

/// A gap beside a device without a width is not measured from the page's placeholder (doc 13 §6.1): after the
/// sample's silencers at the end of rail 1 the list says that the place after the last of them is not known.
#[test]
fn a_gap_after_a_device_without_a_width_is_not_known() {
    let s = common::sample();
    let all = lines(&s, "assembly_list");
    let at = all
        .iter()
        .position(|l| l.contains(",Device,-201X2,"))
        .expect("-201X2 on rail 1");
    assert!(all[at].contains("width missing"), "{}", all[at]);
    let next = &all[at + 1];
    assert!(
        next.starts_with("data,,Gap,")
            && next.ends_with(",after -201X2: not known"),
        "{next}"
    );
    assert!(!next.contains('–'), "no span: {next}");
}
