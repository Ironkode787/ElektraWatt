// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew import edz` and `ew import parts-xml` (doc 14 §5.2, §5.3, §13): the parts of an EDZ into a user library with
//! the terms notice, again as a no-op, and the previewed update of a project's part copy (AC-38) that `--apply`
//! commits.

use crate::support::{EwTmp, Tmp, args, stderr, stdout};

/// The synthetic EDZ of `ew-import-eplan`'s tests: the part 2904602 in a 7-Zip container.
const EDZ_7Z: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../crates/ew-import-eplan/tests/fixtures/synthetic-psu-7z.edz"
);

/// A supplier's list with the part the EDZ is about, and its map.
const LIST: &str = "Maker,Order code,Text\nPhoenix Contact,2904602,Power supply\n";
const MAP: &str =
    "[columns]\nmanufacturer = \"Maker\"\npart_number = \"Order code\"\ndescription = \"Text\"\n";

#[test]
fn an_edz_goes_into_a_user_library_with_the_terms_notice_and_a_second_run_changes_nothing() {
    let t = Tmp::new("ew-edz-library");
    let library = t.path("data").join("libraries").join("mine.ewlib");
    let out = t.ew(&args([
        &"lib",
        &"create",
        &library,
        &"--key",
        &"mine",
        &"--name",
        &"Mine",
        &"--licence",
        &"NOASSERTION",
    ]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let run = || t.ew(&args([&"import", &"edz", &EDZ_7Z, &"--library", &"mine"]));
    let out = run();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("Data Portal"),
        "the notice: {}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("1 new, 0 updated, 0 unchanged, 0 duplicate, 0 rejected; written: 2 added"),
        "{}",
        stdout(&out)
    );
    let again = run();
    assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
    assert!(
        stdout(&again).contains("0 new, 0 updated, 1 unchanged"),
        "{}",
        stdout(&again)
    );
}

#[test]
fn a_parts_xml_goes_into_a_library_and_a_missing_target_is_a_usage_error() {
    let t = Tmp::new("ew-parts-xml-library");
    let library = t.path("data").join("libraries").join("mine.ewlib");
    let out = t.ew(&args([
        &"lib",
        &"create",
        &library,
        &"--key",
        &"mine",
        &"--name",
        &"Mine",
        &"--licence",
        &"NOASSERTION",
    ]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let xml = t.path("parts.xml");
    let mut text = String::from("<partsmanagement length-unit=\"mm\">");
    for n in 0..5 {
        text.push_str(&format!(
            "<part P_ARTICLE_PARTNR=\"P{n}\" P_ARTICLE_MANUFACTURER=\"Acme\"><functiontemplate connectionDesignation=\"1\"/></part>"
        ));
    }
    text.push_str("</partsmanagement>");
    std::fs::write(&xml, text).unwrap();
    let out = t.ew(&args([&"import", &"parts-xml", &xml, &"--library", &"mine"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(!stderr(&out).contains("Data Portal"), "{}", stderr(&out));
    assert!(stdout(&out).contains("5 new"), "{}", stdout(&out));
    let out = t.ew(&args([&"import", &"parts-xml", &xml]));
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--library"), "{}", stderr(&out));
}

/// `ew import eplan-report` writes a terminal diagram as a new project and says what it did not map.
#[test]
fn a_report_export_becomes_a_new_project() {
    let t = Tmp::new("ew-eplan-report");
    let report = t.path("terminals.csv");
    std::fs::write(
        &report,
        "Terminal strip,Terminal,Level,Part number,Remark\n-X1,1,1,,a\n-X1,2,1,,b\n-X1,2,2,,b\n",
    )
    .unwrap();
    let out_dir = t.path("project");
    let out = t.ew(&args([
        &"import",
        &"eplan-report",
        &report,
        &"--preset",
        &"terminals-en",
        &"--out",
        &out_dir,
    ]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("EW-IMP-206") && text.contains("EW-IMP-207"),
        "{text}"
    );
    let (s, _) = ew_api::Session::open_read_only(&out_dir, "test").unwrap();
    let terminals = s
        .query()
        .devices()
        .iter()
        .filter(|(_, d)| d.aspects.terminal.is_some())
        .count();
    assert_eq!(terminals, 2);
    let again = t.ew(&args([
        &"import",
        &"eplan-report",
        &report,
        &"--preset",
        &"terminals-de",
        &"--out",
        &t.path("other"),
    ]));
    assert_eq!(
        again.status.code(),
        Some(2),
        "a mapping the table does not fit is refused"
    );
}

#[test]
fn an_edz_previews_and_applies_the_update_of_a_project_part_copy() {
    let t = Tmp::new("ew-edz-project");
    let project = t.tiny("p.ew");
    let (list, map) = (t.path("list.csv"), t.path("map.toml"));
    std::fs::write(&list, LIST).unwrap();
    std::fs::write(&map, MAP).unwrap();
    let out = t.ew(&args([
        &"import",
        &"parts",
        &list,
        &"--map",
        &map,
        &"--project",
        &project,
    ]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let description = |project: &std::path::Path| {
        let (s, _) = ew_api::Session::open_read_only(project, "test").unwrap();
        let q = s.query();
        let part = q
            .parts()
            .iter()
            .find(|(_, c)| c.item.part_number.as_str() == "2904602")
            .map(|(_, c)| serde_json::to_string(&c.item.description).unwrap())
            .unwrap();
        (
            part,
            q.parts()
                .iter()
                .map(|(_, c)| c.version.clone())
                .collect::<Vec<_>>(),
        )
    };
    let (before, versions) = description(&project);
    assert_eq!(before, "\"Power supply\"");
    assert_eq!(versions, ["1.0.0"]);

    // The preview lists the changed fields and writes nothing.
    let out = t.ew(&args([&"import", &"edz", &EDZ_7Z, &"--project", &project]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("version 1.0.0 -> 1.0.1"), "{text}");
    assert!(text.contains("description"), "{text}");
    assert!(text.contains("nothing was written"), "{text}");
    assert_eq!(description(&project).0, before);

    // `--apply` commits it as one step and saves.
    let out = t.ew(&args([
        &"import",
        &"edz",
        &EDZ_7Z,
        &"--project",
        &project,
        &"--apply",
    ]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("1 part copies updated in one step; saved"),
        "{}",
        stdout(&out)
    );
    let (after, versions) = description(&project);
    assert!(after.contains("Stromversorgung"), "{after}");
    assert_eq!(versions, ["1.0.1"]);

    // Again: the copies hold the data; nothing to update.
    let out = t.ew(&args([&"import", &"edz", &EDZ_7Z, &"--project", &project]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
}
