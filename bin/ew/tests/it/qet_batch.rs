// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew import qet --batch <dir> --out <dir>` (FR-IMP-023): a project and a report per file, a summary of all, a file that
//! cannot be read a row of it, `--strict` an exit code.

use crate::support::{EwTmp, Tmp, args, stderr, stdout};

const GOOD: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors/></diagram></project>"#;
const LOST: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors><conductor terminal1="999" terminal2="998"/></conductors></diagram></project>"#;

/// `ew lib import qet-elements` converts a folder of elements into a library and says what it could not convert.
#[test]
fn a_collection_folder_is_converted_into_a_library() {
    let t = Tmp::new("ew-qet-elements");
    let library = t.path("data").join("libraries").join("qet.ewlib");
    let make = t.ew(&args([
        &"lib",
        &"create",
        &library,
        &"--key",
        &"qet",
        &"--name",
        &"QET",
        &"--licence",
        &"LicenseRef-QET-elements",
    ]));
    assert_eq!(make.status.code(), Some(0), "{}", stderr(&make));
    let folder = t.path("elements");
    std::fs::create_dir_all(folder.join("a")).unwrap();
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/ew-import-qet/tests/fixtures/contact_no.elmt"
    );
    std::fs::copy(fixture, folder.join("a").join("contact_no.elmt")).unwrap();
    std::fs::write(folder.join("bad.elmt"), "no").unwrap();
    let out = t.ew(&args([&"lib", &"import", &"qet-elements", &"qet", &folder]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("converted 1 elements into qet, 1 files not converted; written:"),
        "{}",
        stdout(&out)
    );
}

#[test]
fn a_batch_writes_a_project_and_a_report_per_file_and_a_summary() {
    let t = Tmp::new("ew-qet-batch");
    let dir = t.path("in");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.qet"), GOOD).unwrap();
    std::fs::write(dir.join("b.qet"), LOST).unwrap();
    std::fs::write(dir.join("c.qet"), "not xml").unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    let out_dir = t.path("out");
    let out = t.ew(&args([&"import", &"qet", &"--batch", &dir, &"--out", &out_dir]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    for stem in ["a", "b"] {
        assert!(out_dir.join(stem).join("manifest.json").is_file(), "{stem}");
        assert!(out_dir.join(format!("{stem}.report.md")).is_file(), "{stem}");
    }
    assert!(!out_dir.join("c").exists(), "a refused file writes no project");
    let summary = std::fs::read_to_string(out_dir.join("summary.md")).unwrap();
    assert!(summary.contains("| a.qet | imported |"), "{summary}");
    assert!(summary.contains("| c.qet | refused"), "{summary}");
    assert!(summary.contains("3 files: 2 imported, 1 refused"), "{summary}");
    assert!(stdout(&out).contains("summary written to"), "{}", stdout(&out));
    // `--strict`: a lost connection or a refused file is exit 1 (the summary is written all the same).
    let strict = t.ew(&args([
        &"import",
        &"qet",
        &"--batch",
        &dir,
        &"--out",
        &t.path("out2"),
        &"--strict",
    ]));
    assert_eq!(strict.status.code(), Some(1), "{}", stderr(&strict));
    // No `--out`, no file and a directory without projects are usage errors.
    assert_eq!(
        t.ew(&args([&"import", &"qet", &"--batch", &dir])).status.code(),
        Some(2)
    );
    let empty = t.path("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let none = t.ew(&args([
        &"import",
        &"qet",
        &"--batch",
        &empty,
        &"--out",
        &t.path("out3"),
    ]));
    assert_eq!(none.status.code(), Some(2));
    assert!(stderr(&none).contains("no .qet files"), "{}", stderr(&none));
}
