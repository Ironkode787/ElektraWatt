// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `import_run` record (IC-8): typed within format 0.1, its canonical line round-trips byte for byte,
//! and an unknown member and an unknown `mode` or severity survive a read and a write.

use ew_schema::{Kind, Record, RunMode, RunSeverity};

const RUN: &str = concat!(
    r#"{"$k":"import_run","id":"019a4f00-0000-7000-8000-000000000001","#,
    r#""lineage":"5d0c51a4-7d8c-5b8e-9c1e-1f2a3b4c5d6e","importer":"qet","importer_version":"0.1.0","#,
    r#""source":{"name":"plant.qet","sha256":"sha256:"#,
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    r#"","size":12,"format":"qet-project","format_version":"0.80"},"#,
    r#""as_of":"2026-11-04","mode":"lenient","#,
    r#""entries":[{"code":"EW-IMP-006","severity":"info","defect":"D-12","message":"font substituted","#,
    r#""location":{"file":"plant.qet","folio":"101","line":7},"#,
    r#""targets":[{"kind":"device","id":"6a1b2c3d-0000-5000-8000-000000000002"}],"#,
    r#""fix":{"label":"use BN","command":"fields.set","params":{"value":"BN"}}}],"#,
    r#""stats":{"per_kind":{"device":1,"page":3},"dropped":2,"suppressed":{"EW-IMP-004":5}},"#,
    r#""record_hashes":{"6a1b2c3d-0000-5000-8000-000000000002":{"source":"00112233445566778899aabbccddeeff","#,
    r#""ew":"ffeeddccbbaa99887766554433221100"}}}"#
);

fn round_trip(line: &str) -> String {
    let record: Record = serde_json::from_str(line).unwrap_or_else(|e| panic!("parse failed: {e}\n{line}"));
    serde_json::to_string(&record).unwrap()
}

#[test]
fn an_import_run_round_trips_byte_for_byte() {
    assert_eq!(round_trip(RUN), RUN);
    let Record::ImportRun(run) = serde_json::from_str::<Record>(RUN).unwrap() else {
        panic!("not an import run");
    };
    assert_eq!(Kind::ImportRun.name(), "import_run");
    assert_eq!(run.mode, RunMode::Lenient);
    assert_eq!(run.entries[0].severity, RunSeverity::Info);
    assert_eq!(run.entries[0].targets[0].kind, "device");
    assert_eq!(run.stats.dropped, 2);
    assert_eq!(run.record_hashes.len(), 1);
}

#[test]
fn an_unknown_member_mode_and_severity_survive() {
    let newer = RUN
        .replace(r#""mode":"lenient""#, r#""mode":"forensic""#)
        .replace(r#""severity":"info""#, r#""severity":"notice""#)
        .replace(
            r#""suppressed":{"EW-IMP-004":5}"#,
            r#""suppressed":{"EW-IMP-004":5},"zz_stat":1"#,
        )
        .replace(
            r#""ew":"ffeeddccbbaa99887766554433221100"}}}"#,
            r#""ew":"ffeeddccbbaa99887766554433221100"}},"zz_new":[1]}"#,
        );
    assert_eq!(round_trip(&newer), newer);
    let Record::ImportRun(run) = serde_json::from_str::<Record>(&newer).unwrap() else {
        panic!("not an import run");
    };
    assert_eq!(run.mode, RunMode::Unknown(serde_json::json!("forensic")));
    assert_eq!(
        run.entries[0].severity,
        RunSeverity::Unknown(serde_json::json!("notice"))
    );
    assert!(run.extra.contains_key("zz_new"));
}

#[test]
fn a_minimal_run_writes_no_empty_members() {
    let line = concat!(
        r#"{"$k":"import_run","id":"019a4f00-0000-7000-8000-000000000001","#,
        r#""lineage":"5d0c51a4-7d8c-5b8e-9c1e-1f2a3b4c5d6e","importer":"qet","importer_version":"0.1.0","#,
        r#""source":{"name":"plant.qet","sha256":"sha256:"#,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        r#"","size":0,"format":"qet-project"},"mode":"strict"}"#
    );
    assert_eq!(round_trip(line), line);
}
