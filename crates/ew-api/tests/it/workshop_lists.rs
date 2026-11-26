// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop lists on the sample (doc 13 §6, §9.3): a strip whose end plate was left off is EW-MFG-020 and its fix
//! adds one back, a bridge without a part is EW-MFG-021 and prints TBD, the cut increment rounds each piece once, and a
//! `tube_cut_list` block prints the cut list's Tubes group, which Report block… no longer offers. The lists' rows on the
//! whole sample are the kind CSVs of `report_kinds`.

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
    let out = row_export(&q, &RowExport::Kind(kind.to_owned()), None).unwrap_or_else(|e| panic!("{kind}: {e}"));
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
    assert!(findings(&s, "EW-MFG-020", CheckMode::Online).is_empty(), "silent on the sample");
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
        found[0].message().ends_with("-X1031 has no end plate after its last terminal SH"),
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
        rows.iter().any(|l| l.starts_with("data,7,End plate,,EW-GEN-EP-1,") && l.ends_with(",added")),
        "{rows:#?}"
    );
}

/// EW-MFG-021: BR1 of -X1021 without its part is a warning online and an error in release, and the assembly list and
/// the wire list print TBD for it.
#[test]
fn a_bridge_without_a_part_is_ew_mfg_021_and_prints_tbd() {
    let mut s = common::sample();
    assert!(findings(&s, "EW-MFG-021", CheckMode::Release).is_empty(), "silent on the sample");
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
            && online[0].message().ends_with("-X1021 (terminals 1 to 4) has no part"),
        "{}",
        online[0].message()
    );
    let release = findings(&s, "EW-MFG-021", CheckMode::Release);
    assert_eq!(release[0].severity, ew_checks::Severity::Error);
    assert!(
        lines(&s, "assembly_list").iter().any(|l| l.starts_with("data,,Bridge BR1,1 to 4,TBD,")),
        "the assembly list"
    );
    assert!(
        lines(&s, "wire_list").iter().any(|l| l.starts_with("data,BR1,") && l.contains(",TBD,")),
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
        cut.iter().any(|l| l.starts_with("data,EW-GEN-RAIL-1,") && l.ends_with(",2,475,0.95")),
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
