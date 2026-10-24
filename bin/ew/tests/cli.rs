// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of the `ew` binary.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

/// The property grid pilot's command log (WP-S2.05 part 1), recorded by `bin/elektrawatt/tests/grid/properties.rs`.
#[path = "../../elektrawatt/tests/grid/pilot_log.rs"]
mod pilot_log;

/// The find-and-replace script's command log (WP-S2.05 part 2), recorded by `bin/elektrawatt/tests/grid/find.rs`.
#[path = "../../elektrawatt/tests/grid/find_log.rs"]
mod find_log;

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
    Command::new(bin_exe())
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

/// FT-366: without `--scheme`, `ew check` runs the scheme the project names in `settings.extra.checks`
/// (`release_scheme` in the default mode `release`, `active_scheme` in mode `online`); `--scheme` still wins and a
/// project that names none gets `pre-delivery`.
#[test]
fn check_uses_the_schemes_the_project_names() {
    let t = Tmp::new();
    let dir = t.0.join("named.ew");
    copy_dir(std::path::Path::new(GOLDEN_A), &dir);
    let file = dir.join("project.jsonl");
    let text = std::fs::read_to_string(&file).unwrap();
    let (first, rest) = text.split_once('\n').unwrap();
    let mut record: serde_json::Value = serde_json::from_str(first).unwrap();
    record["settings"]["checks"] = serde_json::json!({"release_scheme": "full", "active_scheme": "drafting"});
    std::fs::write(&file, format!("{record}\n{rest}")).unwrap();
    let scheme_of = |extra: &[&str]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "check".as_ref(),
            dir.as_os_str(),
            "--format".as_ref(),
            "json".as_ref(),
        ];
        args.extend(extra.iter().map(|a| std::ffi::OsStr::new(*a)));
        let o = ew(&args);
        assert!(
            matches!(o.status.code(), Some(0 | 1)),
            "{}",
            String::from_utf8_lossy(&o.stderr)
        );
        let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
        v["scheme"].as_str().unwrap().to_owned()
    };
    assert_eq!(scheme_of(&[]), "full");
    assert_eq!(scheme_of(&["--mode", "online"]), "drafting");
    assert_eq!(scheme_of(&["--scheme", "migration"]), "migration");
    // The golden project names none: the default.
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
    ]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["scheme"], "pre-delivery");
}

/// FT-405 (decision log ID-602): a project naming a scheme this build does not ship (a newer writer, a hand edit) is
/// checked with `pre-delivery` and a notice on stderr naming the scheme; the exit status is the check's own, the same
/// as for the project without the slot. A shipped name in the other slot, or `--scheme`, prints no notice; only the
/// slot for the run's mode is read (ruling 5).
#[test]
fn check_notices_a_project_scheme_this_build_does_not_ship() {
    let t = Tmp::new();
    let dir = t.0.join("unshipped.ew");
    copy_dir(std::path::Path::new(GOLDEN_A), &dir);
    let file = dir.join("project.jsonl");
    let text = std::fs::read_to_string(&file).unwrap();
    let (first, rest) = text.split_once('\n').unwrap();
    let mut record: serde_json::Value = serde_json::from_str(first).unwrap();
    record["settings"]["checks"] =
        serde_json::json!({"release_scheme": "strict-2031", "active_scheme": "drafting"});
    std::fs::write(&file, format!("{record}\n{rest}")).unwrap();
    let run = |project: &std::path::Path, extra: &[&str]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "check".as_ref(),
            project.as_os_str(),
            "--format".as_ref(),
            "json".as_ref(),
        ];
        args.extend(extra.iter().map(|a| std::ffi::OsStr::new(*a)));
        let o = ew(&args);
        let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
        (
            o.status.code(),
            v["scheme"].as_str().unwrap().to_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    const NOTICE: &str = "project scheme 'strict-2031' is not shipped by this build; using pre-delivery";
    let (code, scheme, stderr) = run(&dir, &[]);
    assert_eq!(scheme, "pre-delivery");
    assert!(stderr.contains(NOTICE), "{stderr}");
    let (golden_code, _, golden_err) = run(std::path::Path::new(GOLDEN_A), &[]);
    assert_eq!(code, golden_code, "the exit status is unchanged");
    assert!(matches!(code, Some(0 | 1)), "{stderr}");
    assert!(!golden_err.contains("not shipped"), "{golden_err}");
    // Mode `online` reads the shipped `active_scheme`; `--scheme` reads no slot: no notice either way.
    let (_, scheme, stderr) = run(&dir, &["--mode", "online"]);
    assert_eq!(scheme, "drafting");
    assert!(!stderr.contains("not shipped"), "{stderr}");
    let (_, scheme, stderr) = run(&dir, &["--scheme", "full"]);
    assert_eq!(scheme, "full");
    assert!(!stderr.contains("not shipped"), "{stderr}");
    // An unshipped `active_scheme` is noticed in mode `online`.
    let text = std::fs::read_to_string(&file).unwrap();
    let (first, rest) = text.split_once('\n').unwrap();
    let mut record: serde_json::Value = serde_json::from_str(first).unwrap();
    record["settings"]["checks"] = serde_json::json!({"active_scheme": "live-2031"});
    std::fs::write(&file, format!("{record}\n{rest}")).unwrap();
    let (_, scheme, stderr) = run(&dir, &["--mode", "online"]);
    assert_eq!(scheme, "pre-delivery");
    assert!(
        stderr.contains("project scheme 'live-2031' is not shipped by this build; using pre-delivery"),
        "{stderr}"
    );
    // Only the slot for the run's mode is read (ruling 5): the release run ignores the unshipped online slot.
    let (_, scheme, stderr) = run(&dir, &[]);
    assert_eq!(scheme, "pre-delivery");
    assert!(!stderr.contains("not shipped"), "{stderr}");
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

/// The assembly list of the golden project (doc 20 §14.2): A1 and A2 in place of rows 16-21, the "Covers" column.
#[test]
fn export_bom_assembly_is_the_expected_csv() {
    let o = ew(&[
        "export".as_ref(),
        "bom".as_ref(),
        GOLDEN_A.as_ref(),
        "--mode".as_ref(),
        "assembly".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), expected("csv/bom_assembly.csv"));
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
        out.starts_with("verify-derived: 72 families, ")
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

// ---------------------------------------------------------------------------------------------------------
// `ew export pdf|svg` (FT-59, G-M1-01) and `ew apply` (FT-156)

/// `ew` in the environment of G-M1-01 as far as a test can set it: no display (`DISPLAY`, `WAYLAND_DISPLAY`
/// removed), no starter content (`EW_CONTENT_DIR` removed), no `SOURCE_DATE_EPOCH`, and the user directories
/// (home, application data, state, data and config) pointing at empty directories, so no user library and no
/// user state is read and the journal of a writer goes there. The export uses the bundled fonts only (the PDF's
/// digest holds that), and no subcommand uses a GPU.
fn ew_isolated(home: &std::path::Path, args: &[&std::ffi::OsStr]) -> Output {
    ew_isolated_with(home, args, None)
}

/// [`ew_isolated`] with `SOURCE_DATE_EPOCH` set to `epoch`.
fn ew_isolated_with(home: &std::path::Path, args: &[&std::ffi::OsStr], epoch: Option<&str>) -> Output {
    let dirs = [
        "HOME",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "XDG_STATE_HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
    ];
    let mut cmd = Command::new(bin_exe());
    cmd.args(args)
        .env_remove("EW_CONTENT_DIR")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("SOURCE_DATE_EPOCH");
    for d in dirs {
        let dir = home.join(d.to_ascii_lowercase());
        std::fs::create_dir_all(&dir).unwrap();
        cmd.env(d, &dir);
    }
    if let Some(e) = epoch {
        cmd.env("SOURCE_DATE_EPOCH", e);
    }
    cmd.output().unwrap()
}

/// The OS state directory the isolated `ew` uses for journals (`ew_store::default_state_dir` under the user
/// directories of [`ew_isolated`]).
fn isolated_state_dir(home: &std::path::Path) -> PathBuf {
    if cfg!(windows) {
        home.join("localappdata").join("ElektraWatt").join("state")
    } else if cfg!(target_os = "macos") {
        home.join("home")
            .join("Library/Application Support/ElektraWatt/state")
    } else {
        home.join("xdg_state_home").join("elektrawatt")
    }
}

/// Every file of a directory tree with its bytes, by relative path.
fn tree(dir: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    fn go(
        dir: &std::path::Path,
        base: &std::path::Path,
        out: &mut std::collections::BTreeMap<String, Vec<u8>>,
    ) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                go(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    go(dir, dir, &mut out);
    out
}

/// A copy of Golden A in `t`.
fn golden_copy(t: &Tmp) -> PathBuf {
    let copy = t.0.join("J-E-2416.ew");
    copy_dir(std::path::Path::new(GOLDEN_A), &copy);
    copy
}

/// G-M1-01: `ew export pdf golden/J-E-2416.ew` writes the 36 pages of the page order as the PDF whose SHA-256 is
/// `expected/pdf/sha256.txt`, on one thread and on four, writes nothing into the project and leaves no `.tmp`
/// file.
#[test]
fn export_pdf_of_golden_a_is_the_committed_pdf() {
    use sha2::{Digest, Sha256};
    let t = Tmp::new();
    let project = golden_copy(&t);
    let before = tree(&project);
    let want = expected("pdf/sha256.txt");
    for threads in ["1", "4"] {
        let out = t.0.join(format!("J-E-2416-{threads}.pdf"));
        let o = ew_isolated(
            &t.0.join("home"),
            &[
                "export".as_ref(),
                "pdf".as_ref(),
                project.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
                "--threads".as_ref(),
                threads.as_ref(),
            ],
        );
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert!(stdout(&o).contains(": 36 pages, 92 links"), "{}", stdout(&o));
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(
            format!("{:x}\n", Sha256::digest(&bytes)),
            want,
            "{threads} threads"
        );
        assert!(!t.0.join(format!("J-E-2416-{threads}.pdf.tmp")).exists());
    }
    assert!(tree(&project) == before, "the export wrote into the project");
}

/// `ew export svg --pages 101-106` writes one file per selected page; an unknown folio, a missing `--out` and a
/// missing project exit 2.
#[test]
fn export_svg_writes_the_selected_pages() {
    let t = Tmp::new();
    let dir = t.0.join("svg");
    let o = ew_isolated(
        &t.0.join("home"),
        &[
            "export".as_ref(),
            "svg".as_ref(),
            GOLDEN_A.as_ref(),
            "--out".as_ref(),
            dir.as_os_str(),
            "--pages".as_ref(),
            "101-106".as_ref(),
        ],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let files: Vec<String> = tree(&dir).into_keys().collect();
    assert_eq!(files, ["101.svg", "102.svg", "103.svg", "106.svg"]);
    let unknown = ew(&[
        "export".as_ref(),
        "svg".as_ref(),
        GOLDEN_A.as_ref(),
        "--out".as_ref(),
        dir.as_os_str(),
        "--pages".as_ref(),
        "104".as_ref(),
    ]);
    assert_eq!(unknown.status.code(), Some(2), "{}", stderr(&unknown));
    assert!(stderr(&unknown).contains("104"), "{}", stderr(&unknown));
    let no_out = ew(&["export".as_ref(), "pdf".as_ref(), GOLDEN_A.as_ref()]);
    assert_eq!(no_out.status.code(), Some(2));
    let missing = ew(&[
        "export".as_ref(),
        "pdf".as_ref(),
        t.0.join("nope").as_os_str(),
        "--out".as_ref(),
        t.0.join("x.pdf").as_os_str(),
    ]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(!t.0.join("x.pdf").exists());
}

const VARIANTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/variants");

/// `ew apply --preview` of Golden B step B2 prints the renumbered tags and changes nothing on disk; `ew apply`
/// commits and saves it (folio 106 becomes 104, which `ew export svg --pages 104` then finds), leaving no lock.
#[test]
fn apply_golden_b2_previews_then_saves() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let log = std::path::Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    let before = tree(&project);
    let preview = ew_isolated(
        &home,
        &[
            "apply".as_ref(),
            project.as_os_str(),
            log.as_os_str(),
            "--preview".as_ref(),
        ],
    );
    assert_eq!(preview.status.code(), Some(0), "{}", stderr(&preview));
    let out = stdout(&preview);
    assert!(out.starts_with("preview: 1 of 1 lines commit;"), "{out}");
    assert!(out.contains("-106W1 -> -104W1"), "{out}");
    // Doc 20 section 2.2, step B2: 2 tags (-106W1, -106W2) and 14 cross-reference texts change.
    assert!(out.contains(" 2 tags and 14 cross-references change"), "{out}");
    assert!(tree(&project) == before, "a preview wrote into the project");
    let svg = |pages: &str| {
        ew_isolated(
            &home,
            &[
                "export".as_ref(),
                "svg".as_ref(),
                project.as_os_str(),
                "--out".as_ref(),
                t.0.join("svg").as_os_str(),
                "--pages".as_ref(),
                pages.as_ref(),
            ],
        )
    };
    assert_eq!(
        svg("104").status.code(),
        Some(2),
        "no folio 104 before the replay"
    );
    let applied = ew_isolated(&home, &["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    assert!(
        stdout(&applied).contains("applied 1 of 1 lines; saved"),
        "{}",
        stdout(&applied)
    );
    assert!(tree(&project) != before, "the replay is saved");
    assert!(!project.join(".~ew.lock").exists(), "the lock is released");
    assert_eq!(svg("104").status.code(), Some(0), "folio 104 after the replay");
}

/// Exit codes of `ew apply` (doc 17 section 3.2): a line a domain rule refuses (a second page 7777) exits 1, the
/// line before it is saved, and with `--one-step` nothing is saved; a malformed log and an unresolvable
/// reference exit 2 and save nothing.
#[test]
fn apply_exit_codes_and_one_step() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let write = |name: &str, lines: &[&str]| {
        let p = t.0.join(name);
        let mut text = String::from("{\"$schema\":\"ew.cmdlog/1\",\"label\":\"test\"}\n");
        for l in lines {
            text.push_str(l);
            text.push('\n');
        }
        std::fs::write(&p, text).unwrap();
        p
    };
    let page = r#"{"cmd":"page.create","params":{"number":7777,"title":{"en":"T"}}}"#;
    let dup = write("dup.ewcmd", &[page, page]);
    let before = tree(&project);
    let one_step = ew_isolated(
        &home,
        &[
            "apply".as_ref(),
            project.as_os_str(),
            dup.as_os_str(),
            "--one-step".as_ref(),
        ],
    );
    assert_eq!(one_step.status.code(), Some(1), "{}", stderr(&one_step));
    assert!(
        stderr(&one_step).contains("nothing saved"),
        "{}",
        stderr(&one_step)
    );
    assert!(tree(&project) == before, "--one-step saved a partial replay");
    let bad_ref = write(
        "ref.ewcmd",
        &[
            r#"{"cmd":"fields.set","params":{"kind":"device","targets":[{"ref":"tag:-NOPE"}],"path":"tag.counter","value":1}}"#,
        ],
    );
    let o = ew_isolated(
        &home,
        &["apply".as_ref(), project.as_os_str(), bad_ref.as_os_str()],
    );
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    let malformed = write("bad.ewcmd", &["{\"cmd\":1}"]);
    let o = ew_isolated(
        &home,
        &["apply".as_ref(), project.as_os_str(), malformed.as_os_str()],
    );
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(tree(&project) == before, "a refused log saved something");
    let partial = ew_isolated(&home, &["apply".as_ref(), project.as_os_str(), dup.as_os_str()]);
    assert_eq!(partial.status.code(), Some(1), "{}", stderr(&partial));
    assert!(
        stdout(&partial).contains("applied 1 of 2 lines; saved"),
        "{}",
        stdout(&partial)
    );
    assert!(tree(&project) != before, "the committed line is saved");
    assert!(!project.join(".~ew.lock").exists());
}

/// `ew apply --preview` of Golden B step B1 (doc 20 section 2.2): the 10 device tags of the tag map change, the 28
/// terminal tags follow their strips (38 tag lines), and no cross-reference text changes; nothing is saved.
#[test]
fn apply_preview_of_golden_b1_prints_the_10_tag_changes() {
    let t = Tmp::new();
    let project = golden_copy(&t);
    let before = tree(&project);
    let log = std::path::Path::new(VARIANTS).join("b1-page-based-tags.ewcmd");
    let o = ew_isolated(
        &t.0.join("home"),
        &[
            "apply".as_ref(),
            project.as_os_str(),
            log.as_os_str(),
            "--preview".as_ref(),
        ],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.starts_with("preview: 2 of 2 lines commit;"), "{out}");
    assert!(out.contains(" 38 tags and 0 cross-references change"), "{out}");
    for (before, after) in [
        ("-X1031", "-103X1"),
        ("-X1032", "-103X2"),
        ("-XPE", "-101X2"),
        ("1X1", "-101X1"),
        ("E55", "-101E1"),
        ("E56", "-101E2"),
        ("F55", "-101F2"),
        ("S51", "-101S1"),
        ("S56", "-101S2"),
        ("T55", "-101T1"),
    ] {
        let line = format!(": {before} -> {after}");
        assert_eq!(
            out.lines()
                .filter(|l| l.starts_with("tag ") && l.ends_with(&line))
                .count(),
            1,
            "{line}: {out}"
        );
    }
    assert!(tree(&project) == before);
}

/// `SOURCE_DATE_EPOCH` (doc 17 section 3.1) sets the creation date and with it the document ID: the PDF differs
/// from the model-dated one, carries the date, and is the same bytes on one and four threads; an invalid value
/// exits 2 and writes nothing.
#[test]
fn source_date_epoch_dates_the_pdf() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let export = |name: &str, threads: &str, epoch: &str| {
        let out = t.0.join(name);
        let o = ew_isolated_with(
            &home,
            &[
                "export".as_ref(),
                "pdf".as_ref(),
                GOLDEN_A.as_ref(),
                "--out".as_ref(),
                out.as_os_str(),
                "--threads".as_ref(),
                threads.as_ref(),
            ],
            Some(epoch),
        );
        (o, out)
    };
    // 2023-11-14T22:13:20Z.
    let (o1, p1) = export("a.pdf", "1", "1700000000");
    assert_eq!(o1.status.code(), Some(0), "{}", stderr(&o1));
    let (o4, p4) = export("b.pdf", "4", "1700000000");
    assert_eq!(o4.status.code(), Some(0), "{}", stderr(&o4));
    let (a, b) = (std::fs::read(&p1).unwrap(), std::fs::read(&p4).unwrap());
    assert!(a == b, "1 and 4 threads differ");
    use sha2::{Digest, Sha256};
    assert_ne!(format!("{:x}\n", Sha256::digest(&a)), expected("pdf/sha256.txt"));
    let text = String::from_utf8_lossy(&a);
    assert!(
        text.contains("2023-11-14") || text.contains("D:20231114"),
        "the PDF carries the date of SOURCE_DATE_EPOCH"
    );
    let (bad, out) = export("c.pdf", "1", "yesterday");
    assert_eq!(bad.status.code(), Some(2), "{}", stderr(&bad));
    assert!(stderr(&bad).contains("SOURCE_DATE_EPOCH"), "{}", stderr(&bad));
    assert!(!out.exists());
}

/// `ew export svg` replaces the directory's SVG set as a whole (no stale page of an earlier export stays) and
/// refuses a directory that holds another file, leaving it untouched.
#[test]
fn export_svg_replaces_the_set_and_keeps_other_files() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let dir = t.0.join("svg");
    let svg = |pages: &str, dir: &std::path::Path| {
        ew_isolated(
            &home,
            &[
                "export".as_ref(),
                "svg".as_ref(),
                GOLDEN_A.as_ref(),
                "--out".as_ref(),
                dir.as_os_str(),
                "--pages".as_ref(),
                pages.as_ref(),
            ],
        )
    };
    assert_eq!(svg("1,2,101", &dir).status.code(), Some(0));
    assert_eq!(svg("101-106", &dir).status.code(), Some(0));
    let files: Vec<String> = tree(&dir).into_keys().collect();
    assert_eq!(files, ["101.svg", "102.svg", "103.svg", "106.svg"]);
    let mixed = t.0.join("mixed");
    std::fs::create_dir_all(&mixed).unwrap();
    std::fs::write(mixed.join("notes.txt"), "keep").unwrap();
    let o = svg("101", &mixed);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(stderr(&o).contains("notes.txt"), "{}", stderr(&o));
    let files: Vec<String> = tree(&mixed).into_keys().collect();
    assert_eq!(files, ["notes.txt"]);
    assert_eq!(std::fs::read_to_string(mixed.join("notes.txt")).unwrap(), "keep");
}

/// `ew apply` refuses a project whose crashed session left a journal (exit 2, naming the recovery), and applies
/// nothing.
#[test]
fn apply_refuses_a_project_with_a_crashed_session_journal() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    // A session that commits one edit and is dropped without a save or a clean close: its journal stays.
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = Some(isolated_state_dir(&home));
    let (mut session, _) = ew_api::Session::open(
        &project,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    session
        .execute(&ew_api::AnyCommand {
            name: "page.create".into(),
            params: serde_json::json!({"number": 7777, "title": {"en": "T"}}),
        })
        .unwrap();
    drop(session);
    let before = tree(&project);
    let log = std::path::Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    let o = ew_isolated(&home, &["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(o.status.code(), Some(2), "{}{}", stdout(&o), stderr(&o));
    assert!(stderr(&o).contains("ew recover"), "{}", stderr(&o));
    assert!(tree(&project) == before, "nothing is applied");
}

/// FT-336 (ruling 1, design-pass correction): `ew project checks set` submits the `HumanOnly` command
/// `project.checks.set` only after a confirmation typed on a controlling terminal (doc 17 section 2.3, FR-EXT-030).
/// Without a terminal (here: stdout captured, stdin empty or piped) it exits 2 before the project is opened: nothing
/// changes, no lock, no journal. A scheme that does not ship exits 1 without asking; a usage error (no scheme, a
/// `--confirm` or `--yes` flag) exits 2. The typed path is a unit test with an injected reader and writer (`cli.rs`).
#[test]
fn project_checks_set_needs_an_interactive_terminal() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let before = tree(&project);
    let set = |extra: &[&str]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "project".as_ref(),
            "checks".as_ref(),
            "set".as_ref(),
            project.as_os_str(),
        ];
        args.extend(extra.iter().map(|a| std::ffi::OsStr::new(*a)));
        ew_isolated(&home, &args)
    };
    let o = set(&["--active", "drafting", "--release", "full"]);
    assert_eq!(o.status.code(), Some(2), "{}{}", stdout(&o), stderr(&o));
    assert!(
        stderr(&o).contains("HumanOnly: needs an interactive terminal"),
        "{}",
        stderr(&o)
    );
    // The answer on a pipe does not count: a pipe is no terminal.
    let mut child = Command::new(bin_exe())
        .args(["project", "checks", "set"])
        .arg(&project)
        .args(["--active", "drafting"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().unwrap();
        let _ = stdin.write_all(
            b"drafting
",
        );
    }
    let piped = child.wait_with_output().unwrap();
    assert_eq!(piped.status.code(), Some(2), "{}", stderr(&piped));
    let o = set(&["--release", "lenient"]);
    assert_eq!(o.status.code(), Some(1), "{}{}", stdout(&o), stderr(&o));
    assert!(
        stderr(&o).contains("not a shipped check scheme"),
        "{}",
        stderr(&o)
    );
    for usage in [&[][..], &["--release", "full", "--confirm"][..], &["--yes"][..]] {
        let o = set(usage);
        assert_eq!(o.status.code(), Some(2), "{usage:?}: {}", stderr(&o));
    }
    assert!(tree(&project) == before, "nothing is changed, locked or written");
    let state = isolated_state_dir(&home);
    assert!(
        !state.exists() || std::fs::read_dir(&state).unwrap().next().is_none(),
        "no journal was opened"
    );
}

/// The path of the `ew` binary under test. nextest sets `NEXTEST_BIN_EXE_ew` at run time (the binaries of a
/// `--archive-file` run are unpacked elsewhere than the compile-time path, FT-353); a plain `cargo test` falls back
/// to the compile-time path.
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> std::path::PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_ew")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_BIN_EXE_ew"))) // bin-exe-fallback
}

/// FT-380: `ew project assets purge --preview` lists the asset files nothing names and changes nothing; the purge
/// itself is human-only (owner ruling, 2026-10-23) and exits 2 off a terminal with nothing deleted; an unknown option is
/// a usage error.
#[test]
fn project_assets_purge_previews_and_is_refused_off_a_terminal() {
    const X: &[u8] = br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#;
    const Y: &[u8] = br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#;
    let t = Tmp::new();
    let home = t.0.join("home");
    let dir = tiny_project(&t);
    // The logo X, saved, then Y, saved: X's file is left behind (a save never removes an asset file).
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    let (mut session, _) = ew_api::Session::open(
        &dir,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    for logo in [X, Y] {
        let cmd = ew_api::AnyCommand {
            name: "project.company.set".into(),
            params: serde_json::json!({ "logo": ew_api::base64_encode(logo) }),
        };
        let p = session.preview(&cmd).unwrap();
        session.commit(p).unwrap();
        session.save().unwrap();
    }
    session.close();
    let assets = |dir: &std::path::Path| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir.join("assets"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(assets(&dir).len(), 2);
    let purge = |extra: &[&str]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "project".as_ref(),
            "assets".as_ref(),
            "purge".as_ref(),
            dir.as_os_str(),
        ];
        args.extend(extra.iter().map(|a| std::ffi::OsStr::new(*a)));
        ew_isolated(&home, &args)
    };
    let o = purge(&["--preview"]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), stderr(&o));
    let listed: Vec<String> = stdout(&o).lines().map(str::to_owned).collect();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(
        listed[0].starts_with("remove assets/") && listed[0].contains(".svg ("),
        "{listed:?}"
    );
    assert!(
        stderr(&o).contains("would remove 1 asset file(s)"),
        "{}",
        stderr(&o)
    );
    assert_eq!(assets(&dir).len(), 2, "the preview changes nothing");

    // The purge itself is human-only: without a terminal (stdout captured here) it exits 2 before opening the
    // project, and nothing is deleted. The typed confirmation is a unit test with an injected reader and writer.
    let o = purge(&[]);
    assert_eq!(o.status.code(), Some(2), "{}{}", stdout(&o), stderr(&o));
    assert!(
        stderr(&o).contains("HumanOnly: needs an interactive terminal"),
        "{}",
        stderr(&o)
    );
    assert_eq!(assets(&dir).len(), 2, "nothing is deleted off a terminal");
    assert!(!dir.join(".~ew.lock").exists(), "the project was not opened");

    let o = purge(&["--all"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

/// WP-S2.05 part 1, PG10 (§9 item 2): the property grid pilot's `--record` log (`pilot_log.rs`, which the headless
/// test of `elektrawatt` proves is the record of its script and replays in-process to the records the script saved)
/// applied with `ew apply` gives the same records as that in-process replay: the spec's replay path.
#[test]
fn the_property_grid_pilot_log_replays_through_ew_apply() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let log = t.0.join("pilot.ewcmd");
    let text = format!(
        "{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"pilot\"}}\n{}",
        pilot_log::PILOT_LOG
    );
    std::fs::write(&log, &text).unwrap();
    let applied = ew_isolated(&home, &["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    let lines = pilot_log::PILOT_LOG.lines().count();
    assert!(
        stdout(&applied).contains(&format!("applied {lines} of {lines} lines; saved")),
        "{}",
        stdout(&applied)
    );
    // The same log in-process on a fresh copy.
    let fresh = t.0.join("fresh.ew");
    copy_dir(std::path::Path::new(GOLDEN_A), &fresh);
    let (mut replayed, _) = ew_api::Session::open_read_only(&fresh, "test").unwrap();
    replayed
        .run_script(&ew_api::CmdLog::parse(&text).unwrap(), "pilot.ewcmd")
        .unwrap();
    let (saved, _) = ew_api::Session::open_read_only(&project, "test").unwrap();
    let diff = replayed.diff_from(&saved.world()).unwrap();
    assert!(diff.model.is_empty(), "{:?}", diff.model);
}

// WP-S2.05 part 2
/// FR6 (§9 item 2): the find-and-replace script's `--record` log (`find_log.rs`: three `fields.set_many` lines, which
/// the headless test of `elektrawatt` proves is the record of its script) applied with `ew apply` gives the same records
/// as the same log replayed in-process.
#[test]
fn the_find_and_replace_log_replays_through_ew_apply() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let log = t.0.join("find.ewcmd");
    let text = format!(
        "{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"find\"}}
{}",
        find_log::FIND_LOG
    );
    std::fs::write(&log, &text).unwrap();
    let applied = ew_isolated(&home, &["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    let lines = find_log::FIND_LOG.lines().count();
    assert_eq!(lines, 3);
    assert!(
        stdout(&applied).contains(&format!("applied {lines} of {lines} lines; saved")),
        "{}",
        stdout(&applied)
    );
    let fresh = t.0.join("fresh.ew");
    copy_dir(std::path::Path::new(GOLDEN_A), &fresh);
    let (mut replayed, _) = ew_api::Session::open_read_only(&fresh, "test").unwrap();
    replayed
        .run_script(&ew_api::CmdLog::parse(&text).unwrap(), "find.ewcmd")
        .unwrap();
    let (saved, _) = ew_api::Session::open_read_only(&project, "test").unwrap();
    let diff = replayed.diff_from(&saved.world()).unwrap();
    assert!(diff.model.is_empty(), "{:?}", diff.model);
}
