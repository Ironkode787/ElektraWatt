// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part whose type is not known yet (doc 07 §3.3): `part.placeholder` gives devices one project-local
//! placeholder part numbered TBD with the person's words, which the parts list prints as a row of its own and the
//! check notes as a decision (info online, an error in release mode, no manufacturer asked); a real part chosen for
//! the devices replaces it in one step; a copy of the page carries it.

use ew_api::checks::{CheckMode, CheckReport, PRE_DELIVERY};
use ew_api::output::{RowExport, row_export};
use ew_api::{BomMode, Session};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, Part};
use serde_json::json;

use crate::common::{run_cmd, sample_records, session_of};

/// The sample as an editable session.
fn sample() -> Session {
    let (project, records, lib) = sample_records();
    session_of(project, records, lib)
}

/// The terminals 1 to 4 of the strip -X1021 (each with the starter part EW-GEN-TRM-1).
fn terminals(s: &Session) -> Vec<Id<Device>> {
    ["1", "2", "3", "4"]
        .iter()
        .map(|n| {
            Id::from_uuid(
                s.resolve_reference(&format!("tag:-X1021:{n}"))
                    .expect("a terminal"),
            )
        })
        .collect()
}

/// The part copies a device uses.
fn parts_of(s: &Session, d: Id<Device>) -> Vec<Id<Part>> {
    s.query()
        .devices()
        .get(d)
        .expect("the device")
        .parts
        .iter()
        .map(|u| u.part.copy)
        .collect()
}

/// The parts list's rows as (quantity, manufacturer, part number, description), the description up to its first comma.
fn parts_list(s: &Session) -> Vec<(String, String, String, String)> {
    let q = s.query();
    let out = row_export(&q, &RowExport::Bom(BomMode::Component), None).expect("the parts list");
    let mut lines = out.csv.lines();
    let header: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let col = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .unwrap_or_else(|| panic!("{name}"))
    };
    let (qty, maker, number, words) = (
        col("Qty"),
        col("Manufacturer"),
        col("Part number"),
        col("Description"),
    );
    lines
        .filter(|l| l.starts_with("data,"))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let get = |i: usize| f.get(i).copied().unwrap_or_default().trim_matches('"').to_owned();
            (get(qty), get(maker), get(number), get(words))
        })
        .collect()
}

/// The open findings of `rule` that target `device`, by severity name.
fn findings_on(report: &CheckReport, rule: &str, device: Id<Device>) -> Vec<String> {
    report
        .findings
        .iter()
        .filter(|f| f.is_open() && f.rule == rule)
        .filter(|f| f.targets.iter().any(|t| t.id.as_uuid() == device.as_uuid()))
        .map(|f| f.severity.name().to_owned())
        .collect()
}

/// Four terminals given "Grey terminal" in two steps share one placeholder: numbered TBD, the words as its description,
/// no manufacturer, a terminal part of the project. The parts list prints it as one row of four; online the check notes
/// it as info and asks no manufacturer, in release mode it is an error; a real part chosen for one of them replaces it on
/// all four in one step.
#[test]
fn terminals_of_unknown_type_share_one_placeholder_until_the_real_part_replaces_it() {
    let mut s = sample();
    let four = terminals(&s);
    let real = parts_of(&s, four[0])[0];
    let ids = |ds: &[Id<Device>]| ds.iter().map(ToString::to_string).collect::<Vec<_>>();
    run_cmd(
        &mut s,
        "part.placeholder",
        json!({"devices": ids(&four[..2]), "description": " Grey terminal ", "mode": "replace_all"}),
    );
    run_cmd(
        &mut s,
        "part.placeholder",
        json!({"devices": ids(&four[2..]), "description": "Grey terminal", "mode": "replace_all"}),
    );
    let placeholder = parts_of(&s, four[0]);
    assert_eq!(placeholder.len(), 1);
    let placeholder = placeholder[0];
    for d in &four {
        assert_eq!(parts_of(&s, *d), [placeholder], "one copy for all four");
    }
    let q = s.query();
    let copy = q.parts().get(placeholder).expect("the placeholder copy").clone();
    assert!(copy.item.is_placeholder());
    assert_eq!(
        copy.library_id.as_uuid(),
        q.project().id.as_uuid(),
        "the project's own"
    );
    assert_eq!(copy.item.description.as_neutral(), Some("Grey terminal"));
    assert_eq!(copy.item.kind, ew_schema::PartKind::Terminal);
    assert_eq!(ew_derive::manufacturer_name(&q, &copy.item.manufacturer), "");
    drop(q);

    let rows = parts_list(&s);
    let tbd: Vec<_> = rows.iter().filter(|r| r.2 == "TBD").collect();
    assert_eq!(
        tbd,
        [&(
            "4".to_owned(),
            String::new(),
            "TBD".to_owned(),
            "Grey terminal".to_owned()
        )],
        "{rows:?}"
    );

    let online = s.check(&PRE_DELIVERY, CheckMode::Online);
    let release = s.check(&PRE_DELIVERY, CheckMode::Release);
    for d in &four {
        assert!(findings_on(&online, "EW-CORE-041", *d).is_empty());
        assert!(findings_on(&online, "EW-CORE-042", *d).is_empty());
    }
    let placeholder_findings = |r: &CheckReport| {
        r.findings
            .iter()
            .filter(|f| f.is_open() && f.rule == "EW-CORE-043")
            .map(|f| f.severity.name().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(placeholder_findings(&online), ["info"]);
    assert_eq!(placeholder_findings(&release), ["error"]);

    // The real part, chosen for one of them through the part dialog's command, replaces the placeholder on all four.
    let users = s.placeholder_users(placeholder.as_uuid()).expect("a placeholder");
    let mut sorted: Vec<Uuid> = four.iter().map(|d| d.as_uuid()).collect();
    sorted.sort_unstable();
    assert_eq!(users, sorted);
    let cursor = s.history().cursor();
    run_cmd(
        &mut s,
        "part.assign",
        json!({"devices": users.iter().map(ToString::to_string).collect::<Vec<_>>(), "part": real.to_string(),
            "mode": "replace", "from": placeholder.to_string()}),
    );
    assert_eq!(s.history().cursor(), cursor + 1, "one undo step");
    for d in &four {
        assert_eq!(parts_of(&s, *d), [real]);
    }
    assert!(parts_list(&s).iter().all(|r| r.2 != "TBD"));
}

/// A placeholder needs words: an empty description is refused at its parameter.
#[test]
fn a_placeholder_without_words_is_refused() {
    let mut s = sample();
    let d = terminals(&s)[0];
    let err = s
        .execute(&ew_api::AnyCommand {
            name: "part.placeholder".into(),
            params: json!({"devices": [d.to_string()], "description": "  "}),
        })
        .expect_err("no words");
    match err {
        ew_api::CmdError::InvalidParams { path, message, .. } => {
            assert_eq!(path, "/description");
            assert!(message.contains("what the part is"), "{message}");
        }
        other => panic!("a parameter refusal: {other}"),
    }
}

/// A copy of the pages that draw devices with a placeholder carries it: the copies use the same placeholder copy.
#[test]
fn a_copy_of_the_pages_carries_the_placeholder() {
    let mut s = sample();
    let one = terminals(&s)[0];
    run_cmd(
        &mut s,
        "part.placeholder",
        json!({"devices": [one.to_string()], "description": "Grey terminal", "mode": "replace_all"}),
    );
    let placeholder = parts_of(&s, one)[0];
    let users = s
        .placeholder_users(placeholder.as_uuid())
        .expect("a placeholder")
        .len();
    let pages = ["101", "102", "103"]
        .iter()
        .map(|f| s.page_tables(f).expect("a drawn page").page)
        .collect();
    let rename = [("1L", "2L"), ("24V", "24V.2"), ("0V", "0V.2"), ("PE", "PE.2")]
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
    let plan = s
        .copy_pages(&ew_api::PageCopySpec {
            pages,
            folios: ew_api::CopyFolios::Offset(10),
            structure: Vec::new(),
            rename,
        })
        .expect("the copy of the pages");
    run_cmd(&mut s, &plan.command.name, plan.command.params.clone());
    let after = s
        .placeholder_users(placeholder.as_uuid())
        .expect("a placeholder")
        .len();
    assert_eq!(after, users + 1, "the copied terminal names the same placeholder");
}
