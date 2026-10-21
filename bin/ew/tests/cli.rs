// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of the `ew` binary.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");
const LOST: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors><conductor terminal1="999" terminal2="998"/></conductors></diagram></project>"#;
const PROJECT: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors/></diagram></project>"#;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-cli-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    fn project(&self) -> PathBuf {
        self.project_with(PROJECT)
    }

    fn project_with(&self, xml: &str) -> PathBuf {
        let p = self.0.join("t.qet");
        std::fs::write(&p, xml).unwrap();
        p
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ew(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ew"))
        .args(args)
        .env_remove("EW_CONTENT_DIR")
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn version() {
    let o = ew(&["--version".as_ref()]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(stdout(&o), format!("ew {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn import_summary_with_content() {
    let t = Tmp::new();
    let f = t.project();
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--content".as_ref(),
        CONTENT.as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let out = stdout(&o);
    assert!(out.starts_with("imported t.qet:"), "{out}");
    assert!(out.contains("report:"), "{out}");
    assert!(out.contains("derived: 1 pages"), "{out}");
    assert!(
        !out.contains(&t.0.to_string_lossy().into_owned()),
        "the name is never a path"
    );
}

#[test]
fn import_without_content_warns() {
    let t = Tmp::new();
    let f = t.project();
    let o = ew(&["import".as_ref(), "qet".as_ref(), f.as_os_str()]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stderr).contains("warning: no starter content"));
}

#[test]
fn report_json_to_stdout() {
    let t = Tmp::new();
    let f = t.project();
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--content".as_ref(),
        CONTENT.as_ref(),
        "--report".as_ref(),
        "json".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0));
    let out = stdout(&o);
    let v: serde_json::Value = serde_json::from_str(&out).expect("stdout is exactly the report");
    assert_eq!(v["source"], "t.qet");
    assert!(v["groups"].is_array());
    assert!(v["live"].is_array());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("imported t.qet:"), "{err}");
    assert!(err.contains("derived: 1 pages"), "{err}");
}

#[test]
fn strict_passes_clean_project() {
    let t = Tmp::new();
    let f = t.project();
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--strict".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn strict_fails_on_error_finding_lenient_does_not() {
    let t = Tmp::new();
    let f = t.project_with(LOST);
    let lenient = ew(&["import".as_ref(), "qet".as_ref(), f.as_os_str()]);
    assert_eq!(
        lenient.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&lenient.stderr)
    );
    let strict = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--strict".as_ref(),
    ]);
    assert_eq!(strict.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&strict.stderr).contains("strict:"));
}

#[test]
fn report_csv_to_file() {
    let t = Tmp::new();
    let f = t.project();
    let out_file = t.0.join("report.csv");
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--report".as_ref(),
        "csv".as_ref(),
        "--report-out".as_ref(),
        out_file.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0));
    assert!(stdout(&o).contains(&format!("report written to {}", out_file.display())));
    let text = std::fs::read_to_string(&out_file).unwrap();
    assert!(text.starts_with("code,severity"), "{text}");
}

#[test]
fn missing_file_is_exit_2() {
    let t = Tmp::new();
    let missing = t.0.join("nope.qet");
    let o = ew(&["import".as_ref(), "qet".as_ref(), missing.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("error:"));
}

#[test]
fn usage_errors_are_exit_2() {
    assert_eq!(ew(&["import".as_ref(), "qet".as_ref()]).status.code(), Some(2));
    assert_eq!(ew(&["bogus".as_ref()]).status.code(), Some(2));
    assert_eq!(ew(&[]).status.code(), Some(2));
}

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

#[test]
fn check_golden_a_json() {
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
    ]);
    // Golden A has unwaived errors (doc 20 §18.1: AC-21 exits 1); today EW-CORE-033 on -3W1 is one.
    let code = o.status.code();
    assert_eq!(code, Some(1), "{}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["$schema"], "ew.check-result/1");
    assert_eq!(v["scheme"], "pre-delivery");
    assert_eq!(v["exit_code"].as_i64(), Some(1));
    assert!(!std::path::Path::new(GOLDEN_A).join(".~ew.lock").exists());
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// A damaged project: the integrity findings of opening join the counts and the exit code (doc 15 §4.2).
#[test]
fn check_counts_integrity_errors_of_a_damaged_project() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("damaged.ew");
    copy_dir(std::path::Path::new(GOLDEN_A), &dir);
    // A JSON syntax error in a page file (EW-CORE-005, error, opens read-only).
    let pages = dir.join("pages");
    let page = std::fs::read_dir(&pages)
        .unwrap()
        .map(|e| e.unwrap().path())
        .min()
        .unwrap();
    let mut text = std::fs::read_to_string(&page).unwrap();
    text.push_str("{not json\n");
    std::fs::write(&page, text).unwrap();
    let o = ew(&[
        "check".as_ref(),
        dir.as_os_str(),
        "--format".as_ref(),
        "json".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(1), "{}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    let rules: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["rule"].as_str())
        .collect();
    assert!(rules.contains(&"EW-CORE-005"), "{rules:?}");
}

#[test]
fn check_markdown_title() {
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--format".as_ref(),
        "md".as_ref(),
    ]);
    assert!(matches!(o.status.code(), Some(0 | 1)));
    assert!(
        stdout(&o).starts_with("# Check report: pre-delivery"),
        "{}",
        stdout(&o)
    );
}

#[test]
fn check_unknown_scheme_is_exit_2() {
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--scheme".as_ref(),
        "bogus".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(2));
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn sql(query: &str) -> Output {
    ew(&["sql".as_ref(), GOLDEN_A.as_ref(), query.as_ref()])
}

#[test]
fn sql_count_terminals() {
    let o = sql("SELECT count(*) AS n FROM api_v1_terminal");
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), "n\n28\n");
    assert!(stderr(&o).contains("1 rows"), "{}", stderr(&o));
    assert!(!std::path::Path::new(GOLDEN_A).join(".~ew.lock").exists());
}

#[test]
fn sql_inline_query_may_start_with_a_comment() {
    let o = sql("-- the terminal count of doc 20 §9\nSELECT count(*) AS n FROM api_v1_terminal");
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), "n\n28\n");
}

#[test]
fn sql_huge_timeout_is_a_usage_error_not_a_panic() {
    let o = ew(&[
        "sql".as_ref(),
        GOLDEN_A.as_ref(),
        "SELECT 1".as_ref(),
        "--timeout".as_ref(),
        "18446744073709551615".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(stderr(&o).contains("--timeout"), "{}", stderr(&o));
}

#[test]
fn sql_attach_is_denied() {
    let o = sql("ATTACH 'other.db' AS x");
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("statement denied by the projection sandbox (ATTACH)"),
        "{}",
        stderr(&o)
    );
}

#[test]
fn sql_statement_gate_blocks_vacuum_into() {
    let t = Tmp::new();
    let target = t.0.join("x.db");
    let path = target.to_string_lossy().replace('\'', "''");
    let o = sql(&format!("SELECT 1; VACUUM INTO '{path}'"));
    assert_eq!(o.status.code(), Some(2), "{}", stdout(&o));
    assert!(!target.exists());
}

#[test]
fn sql_json_natural_order() {
    let o = ew(&[
        "sql".as_ref(),
        GOLDEN_A.as_ref(),
        "SELECT designation FROM api_v1_terminal WHERE strip_tag = '-X1031' ORDER BY designation COLLATE ew_natural LIMIT 3"
            .as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["$schema"], "ew.sql-result/1");
    assert_eq!(v["rows"], serde_json::json!([["1"], ["2"], ["3"]]));
}

#[test]
fn sql_query_from_file() {
    let t = Tmp::new();
    let f = t.0.join("q.sql");
    std::fs::write(
        &f,
        "SELECT count(*) AS n FROM api_v1_wire WHERE kind = 'bridge'\n",
    )
    .unwrap();
    let o = ew(&[
        "sql".as_ref(),
        GOLDEN_A.as_ref(),
        "--file".as_ref(),
        f.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), "n\n6\n");
}

#[test]
fn sql_dump_writes_sqlite_file() {
    let t = Tmp::new();
    let out = t.0.join("p.sqlite");
    let o = ew(&[
        "sql".as_ref(),
        "dump".as_ref(),
        GOLDEN_A.as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("projection written to"), "{}", stdout(&o));
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"SQLite format 3\0"));
}

#[test]
fn sql_schema_lists_views() {
    let o = ew(&["sql".as_ref(), "schema".as_ref()]);
    assert_eq!(o.status.code(), Some(0));
    let out = stdout(&o);
    assert!(out.contains("api_v1_terminal"), "{out}");
    assert!(out.contains("api_v1_wire"), "{out}");
}

#[test]
fn sql_out_writes_csv_file() {
    let t = Tmp::new();
    let out = t.0.join("r.csv");
    let o = ew(&[
        "sql".as_ref(),
        GOLDEN_A.as_ref(),
        "SELECT count(*) AS n FROM api_v1_terminal".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("result written to"), "{}", stdout(&o));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "n\n28\n");
}

#[test]
fn sql_random_is_denied_and_missing_project_is_exit_2() {
    let o = sql("SELECT random()");
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("function random"), "{}", stderr(&o));
    let t = Tmp::new();
    let missing = t.0.join("nope.ew");
    let o = ew(&["sql".as_ref(), missing.as_os_str(), "SELECT 1".as_ref()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("error:"));
}

#[test]
fn check_missing_project_is_exit_2() {
    let t = Tmp::new();
    let missing = t.0.join("nope.ew");
    let o = ew(&["check".as_ref(), missing.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("error:"));
}

const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

fn expected(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(EXPECTED).join(path)).unwrap()
}

/// `ew export cables` writes the cable list of doc 20 §10 exactly as `expected/csv/cables.csv`.
#[test]
fn export_cables_is_the_expected_csv() {
    let o = ew(&["export".as_ref(), "cables".as_ref(), GOLDEN_A.as_ref()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), expected("csv/cables.csv"));
    assert!(stderr(&o).contains("20 rows"), "{}", stderr(&o));
    assert!(!std::path::Path::new(GOLDEN_A).join(".~ew.lock").exists());
}

#[test]
fn export_out_writes_a_file() {
    let t = Tmp::new();
    let out = t.0.join("cables.csv");
    let o = ew(&[
        "export".as_ref(),
        "cables".as_ref(),
        GOLDEN_A.as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).starts_with("export written to"), "{}", stdout(&o));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), expected("csv/cables.csv"));
}

/// `ew export bom` lists the parts by default (`--mode parts` and its alias `component`): the 27 rows of folios
/// 901 and 901.1.
#[test]
fn export_bom_parts_is_the_expected_csv() {
    for args in [&[][..], &["--mode", "parts"][..], &["--mode", "component"][..]] {
        let mut a: Vec<&std::ffi::OsStr> = vec!["export".as_ref(), "bom".as_ref(), GOLDEN_A.as_ref()];
        a.extend(args.iter().map(|s| std::ffi::OsStr::new(*s)));
        let o = ew(&a);
        assert_eq!(o.status.code(), Some(0), "{args:?}: {}", stderr(&o));
        assert_eq!(stdout(&o), expected("csv/bom_parts.csv"), "{args:?}");
    }
}

/// The golden project holds no configured product yet (D-41, cycle 15), so the assembly list is the parts list;
/// the replacement of rows 16-21 by A1 and A2 is tested over a model with configured parts (`ew-api`,
/// `tests/export_bom.rs`).
#[test]
fn export_bom_assembly_runs_over_the_golden_project() {
    let o = ew(&[
        "export".as_ref(),
        "bom".as_ref(),
        GOLDEN_A.as_ref(),
        "--mode".as_ref(),
        "assembly".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), expected("csv/bom_parts.csv"));
}

#[test]
fn export_errors_are_exit_2() {
    let t = Tmp::new();
    let missing = t.0.join("nope.ew");
    for args in [
        &["export", "cables"][..],
        &["export", "wires"][..],
        &["export", "bom", "--format", "xlsx"][..],
    ] {
        // A command line without a project that no ancestor of the working directory holds, or a bad option.
        let mut a: Vec<&std::ffi::OsStr> = args.iter().map(|s| std::ffi::OsStr::new(*s)).collect();
        if args.len() == 2 && args[1] == "cables" {
            a.push(missing.as_os_str());
        }
        let o = ew(&a);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
        assert!(stderr(&o).contains("error:"), "{args:?}");
    }
}

/// A small saved project: one page, no report blocks.
fn tiny_project(t: &Tmp) -> PathBuf {
    let project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .unwrap();
    let mut session =
        ew_api::Session::from_records(project, [], [], ew_api::SessionOptions::default()).unwrap();
    session
        .execute(&ew_api::AnyCommand {
            name: "page.create".into(),
            params: serde_json::json!({"number": 1, "title": "Cover"}),
        })
        .unwrap();
    let dir = t.0.join("tiny.ew");
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    session.create(&dir, store).unwrap();
    session.close();
    dir
}

/// `ew verify-derived` over a small project: exit 0 and the summary line (every family, no difference).
#[test]
fn verify_derived_tiny_project_is_clean() {
    let t = Tmp::new();
    let dir = tiny_project(&t);
    let o = ew(&["verify-derived".as_ref(), dir.as_os_str()]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), stderr(&o));
    let out = stdout(&o);
    assert!(
        out.starts_with("verify-derived: 50 families, ")
            && out.ends_with(" keys, 0 differences, 0 oracle mismatches\n"),
        "{out}"
    );
    assert!(!dir.join(".~ew.lock").exists());
}

/// A project without the report block cannot export: exit 2 with the reason.
#[test]
fn export_without_the_block_is_exit_2() {
    let t = Tmp::new();
    let dir = tiny_project(&t);
    let o = ew(&["export".as_ref(), "cables".as_ref(), dir.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("no report block of kind `cable_overview_diagrams`"),
        "{}",
        stderr(&o)
    );
}

/// `--out` to a path that cannot be written is exit 2 with the path in the message, and writes nothing.
#[test]
fn export_out_to_an_unwritable_path_is_exit_2() {
    let t = Tmp::new();
    let out = t.0.join("no-such-dir").join("cables.csv");
    for what in ["cables", "bom"] {
        let o = ew(&[
            "export".as_ref(),
            what.as_ref(),
            GOLDEN_A.as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ]);
        assert_eq!(o.status.code(), Some(2), "{what}: {}", stdout(&o));
        assert!(
            stderr(&o).contains("error:") && stderr(&o).contains("cables.csv"),
            "{}",
            stderr(&o)
        );
        assert!(!out.exists());
    }
}

#[test]
fn verify_derived_missing_project_is_exit_2() {
    let t = Tmp::new();
    let missing = t.0.join("nope.ew");
    let o = ew(&["verify-derived".as_ref(), missing.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("error:"));
}

/// `ew verify-derived` over Golden A: every memoised value equals its cold recomputation with the oracle on.
/// The oracle re-runs every memoised read, about 90 s in a debug build, so the test is run by hand
/// (`cargo nextest run -p ew --run-ignored only`) and by the acceptance runner's AC-16 at the library level.
#[test]
#[ignore = "about 90 s in a debug build (the oracle re-runs every memoised read)"]
fn verify_derived_golden_a_is_clean() {
    let o = ew(&["verify-derived".as_ref(), GOLDEN_A.as_ref()]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert!(
        stdout(&o).contains("0 differences, 0 oracle mismatches"),
        "{}",
        stdout(&o)
    );
}
