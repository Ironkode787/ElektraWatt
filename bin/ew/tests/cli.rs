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

/// A missing input is exit 2 with an `error:` line, one case per subcommand: `ew import qet` of a missing file,
/// `ew check` and `ew verify-derived` of a missing project.
#[test]
fn missing_file_is_exit_2() {
    let t = Tmp::new();
    let (qet, project) = (t.0.join("nope.qet"), t.0.join("nope.ew"));
    let cases: [(&str, Vec<&std::ffi::OsStr>); 3] = [
        (
            "missing_file_is_exit_2",
            vec!["import".as_ref(), "qet".as_ref(), qet.as_os_str()],
        ),
        (
            "check_missing_project_is_exit_2",
            vec!["check".as_ref(), project.as_os_str()],
        ),
        (
            "verify_derived_missing_project_is_exit_2",
            vec!["verify-derived".as_ref(), project.as_os_str()],
        ),
    ];
    for (name, args) in cases {
        let o = ew(&args);
        assert_eq!(o.status.code(), Some(2), "case {name}");
        assert!(stderr(&o).contains("error:"), "case {name}: {}", stderr(&o));
    }
}

#[test]
fn usage_errors_are_exit_2() {
    assert_eq!(ew(&["import".as_ref(), "qet".as_ref()]).status.code(), Some(2));
    assert_eq!(ew(&["bogus".as_ref()]).status.code(), Some(2));
    assert_eq!(ew(&[]).status.code(), Some(2));
}

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// `ew check` of Golden A in each output format: JSON (the check result of the pre-delivery scheme, exit 1) and
/// Markdown (the report title); neither leaves a lock behind.
#[test]
fn check_golden_a_in_each_format() {
    let check = |format: &str| {
        ew(&[
            "check".as_ref(),
            GOLDEN_A.as_ref(),
            "--format".as_ref(),
            format.as_ref(),
        ])
    };
    let o = check("json");
    // Golden A has unwaived errors (doc 20 §18.1: AC-21 exits 1); today EW-CORE-033 on -3W1 is one.
    let code = o.status.code();
    assert_eq!(code, Some(1), "case json: {}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["$schema"], "ew.check-result/1");
    assert_eq!(v["scheme"], "pre-delivery");
    assert_eq!(v["exit_code"].as_i64(), Some(1));
    assert!(!std::path::Path::new(GOLDEN_A).join(".~ew.lock").exists());
    let o = check("md");
    assert!(
        matches!(o.status.code(), Some(0 | 1)),
        "case check_markdown_title"
    );
    assert!(
        stdout(&o).starts_with("# Check report: pre-delivery"),
        "case check_markdown_title: {}",
        stdout(&o)
    );
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
fn check_unknown_scheme_is_exit_2() {
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--scheme".as_ref(),
        "bogus".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(2));
}

/// P5-2 (WP-S1.16 D6): `ew check --lang nl-NL` prints the Dutch text of a rule with an nl-NL pattern (EW-POT-013 on
/// T55) and the en-GB text of one without (EW-CORE-044 has none; Golden A's EW-CORE-043 has one); the default and
/// `--lang en-GB` print the en-GB message; an unknown language is a usage error. The counts and the exit status do not
/// depend on the language.
#[test]
fn check_lang_renders_the_messages_in_a_shipped_language() {
    let check = |lang: Option<&str>| {
        let mut args: Vec<&std::ffi::OsStr> = vec!["check".as_ref(), GOLDEN_A.as_ref()];
        if let Some(l) = lang {
            args.push(std::ffi::OsStr::new("--lang"));
            args.push(std::ffi::OsStr::new(l));
        }
        ew(&args)
    };
    let en = check(None);
    assert_eq!(en.status.code(), Some(1));
    let en_text = stdout(&en);
    assert!(
        en_text.contains("T55: T55 switches the neutral N-55 → N-55T; load E55 stays connected to L-55."),
        "{en_text}"
    );
    assert_eq!(stdout(&check(Some("en-GB"))), en_text);
    let nl = check(Some("nl-NL"));
    assert_eq!(nl.status.code(), Some(1));
    let nl_text = stdout(&nl);
    assert!(
        nl_text.contains(
            "T55: T55 schakelt de nulleider N-55 → N-55T; belasting E55 blijft verbonden met L-55."
        ),
        "{nl_text}"
    );
    assert!(
        !nl_text.contains("placeholder part \"End bracket\"")
            && nl_text.contains("plaatsvervangend artikel \"End bracket\" (artikelnummer TBD)"),
        "{nl_text}"
    );
    // The same lines in the same order, the totals line equal.
    assert_eq!(nl_text.lines().count(), en_text.lines().count());
    assert_eq!(nl_text.lines().last(), en_text.lines().last());
    let bad = check(Some("xx-YY"));
    assert_eq!(bad.status.code(), Some(2));
    assert!(
        stderr(&bad).contains("unknown message language"),
        "{}",
        stderr(&bad)
    );
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn sql(query: &str) -> Output {
    ew(&["sql".as_ref(), GOLDEN_A.as_ref(), query.as_ref()])
}

/// An inline query may start with an SQL comment; the count prints as CSV, stderr reports the row count, and the
/// read leaves no lock behind.
#[test]
fn sql_inline_query_may_start_with_a_comment() {
    let o = sql("-- the terminal count of doc 20 §9\nSELECT count(*) AS n FROM api_v1_terminal");
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), "n\n28\n");
    assert!(stderr(&o).contains("1 rows"), "{}", stderr(&o));
    assert!(!std::path::Path::new(GOLDEN_A).join(".~ew.lock").exists());
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
fn sql_schema_lists_views() {
    let o = ew(&["sql".as_ref(), "schema".as_ref()]);
    assert_eq!(o.status.code(), Some(0));
    let out = stdout(&o);
    assert!(out.contains("api_v1_terminal"), "{out}");
    assert!(out.contains("api_v1_wire"), "{out}");
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

const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

fn expected(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(EXPECTED).join(path)).unwrap()
}

/// `ew export cables` and `ew export bom` print the expected CSVs of the golden project, one case per list: the
/// cable list of doc 20 §10 (`expected/csv/cables.csv`, with its row count on stderr and no lock left behind); the
/// parts list by default, with `--mode parts` and with its alias `component` (the 27 rows of folios 901 and 901.1);
/// and the assembly list of doc 20 §14.2 (A1 and A2 in place of rows 16-21, the "Covers" column).
#[test]
fn export_csv_is_the_expected_csv() {
    let cases: [(&str, &[&str], &str); 5] = [
        ("export_cables_is_the_expected_csv", &["cables"], "csv/cables.csv"),
        (
            "export_bom_parts_is_the_expected_csv default",
            &["bom"],
            "csv/bom_parts.csv",
        ),
        (
            "export_bom_parts_is_the_expected_csv --mode parts",
            &["bom", "--mode", "parts"],
            "csv/bom_parts.csv",
        ),
        (
            "export_bom_parts_is_the_expected_csv --mode component",
            &["bom", "--mode", "component"],
            "csv/bom_parts.csv",
        ),
        (
            "export_bom_assembly_is_the_expected_csv",
            &["bom", "--mode", "assembly"],
            "csv/bom_assembly.csv",
        ),
    ];
    for (name, args, file) in cases {
        let mut a: Vec<&std::ffi::OsStr> = vec!["export".as_ref(), args[0].as_ref(), GOLDEN_A.as_ref()];
        a.extend(args[1..].iter().map(|s| std::ffi::OsStr::new(*s)));
        let o = ew(&a);
        assert_eq!(o.status.code(), Some(0), "case {name}: {}", stderr(&o));
        assert_eq!(stdout(&o), expected(file), "case {name}");
        if name == "export_cables_is_the_expected_csv" {
            assert!(stderr(&o).contains("20 rows"), "case {name}: {}", stderr(&o));
            assert!(
                !std::path::Path::new(GOLDEN_A).join(".~ew.lock").exists(),
                "case {name}"
            );
        }
    }
}

/// A command that writes a file says so on stdout and writes it, one case per writer: `ew sql --out` (the CSV),
/// `ew sql dump` (an SQLite file) and `ew export cables --out` (the expected cable list).
#[test]
fn out_writes_a_file() {
    let t = Tmp::new();
    let (csv, sqlite, cables) = (t.0.join("r.csv"), t.0.join("p.sqlite"), t.0.join("cables.csv"));
    type Check = fn(&[u8]);
    let cases: [(&str, Vec<&std::ffi::OsStr>, &str, &PathBuf, Check); 3] = [
        (
            "sql_out_writes_csv_file",
            vec![
                "sql".as_ref(),
                GOLDEN_A.as_ref(),
                "SELECT count(*) AS n FROM api_v1_terminal".as_ref(),
                "--out".as_ref(),
                csv.as_os_str(),
            ],
            "result written to",
            &csv,
            |bytes| assert_eq!(bytes, b"n\n28\n"),
        ),
        (
            "sql_dump_writes_sqlite_file",
            vec![
                "sql".as_ref(),
                "dump".as_ref(),
                GOLDEN_A.as_ref(),
                sqlite.as_os_str(),
            ],
            "projection written to",
            &sqlite,
            |bytes| assert!(bytes.starts_with(b"SQLite format 3\0")),
        ),
        (
            "export_out_writes_a_file",
            vec![
                "export".as_ref(),
                "cables".as_ref(),
                GOLDEN_A.as_ref(),
                "--out".as_ref(),
                cables.as_os_str(),
            ],
            "export written to",
            &cables,
            |bytes| assert_eq!(String::from_utf8_lossy(bytes), expected("csv/cables.csv")),
        ),
    ];
    for (name, args, says, out, check) in cases {
        let o = ew(&args);
        assert_eq!(o.status.code(), Some(0), "case {name}: {}", stderr(&o));
        assert!(stdout(&o).contains(says), "case {name}: {}", stdout(&o));
        if name == "export_out_writes_a_file" {
            assert!(stdout(&o).starts_with(says), "case {name}: {}", stdout(&o));
        }
        let bytes = std::fs::read(out).unwrap();
        in_case(name, || check(&bytes));
    }
}

/// Runs `check`, naming `case` in its failure message.
fn in_case(case: &str, check: impl FnOnce()) {
    if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)) {
        let msg = e
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| e.downcast_ref::<&str>().copied())
            .unwrap_or("a panic without a message");
        panic!("case {case}: {msg}");
    }
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
    // Every swept family, without pinning their number (CONTRIBUTING.md: no totals of growing sets).
    let families = format!("verify-derived: {} families, ", ew_api::verify::SWEPT.len());
    assert!(
        out.starts_with(&families) && out.ends_with(" keys, 0 differences, 0 oracle mismatches\n"),
        "{out}"
    );
    assert!(!dir.join(".~ew.lock").exists());
}

/// FT-465: the parts list of a project without a `parts_summary` block (a new project from the built-in template, as the
/// blind drafter's self-test makes) is a refusal, exit 2, and the one-line message names the block, the export, where the
/// block comes from, and that there is nothing to export; no file is written.
#[test]
fn export_bom_without_the_block_is_exit_2_and_names_the_cause() {
    let t = Tmp::new();
    let dir = tiny_project(&t);
    let out = t.0.join("bom.csv");
    for mode in ["parts", "assembly"] {
        let o = ew(&[
            "export".as_ref(),
            "bom".as_ref(),
            dir.as_os_str(),
            "--mode".as_ref(),
            mode.as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ]);
        assert_eq!(o.status.code(), Some(2), "{mode}: {}", stdout(&o));
        let err = stderr(&o);
        assert_eq!(err.lines().count(), 1, "one line: {err}");
        for want in [
            "error: the project has no report block of kind `parts_summary`",
            "the bill of materials is the rows of that block",
            "company-standard wizard creates it as folio 901",
            "a project from the built-in template has none",
            "nothing to export",
        ] {
            assert!(err.contains(want), "{mode}: `{want}` not in `{err}`");
        }
        assert!(!out.exists(), "a refused export writes no file");
    }
    // The cable list says the same about its own block, and names it.
    let o = ew(&["export".as_ref(), "cables".as_ref(), dir.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("no report block of kind `cable_overview_diagrams`"),
        "{}",
        stderr(&o)
    );
    assert!(
        stderr(&o).contains("the cable list is the rows of that block") && stderr(&o).contains("folio 906"),
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
    let journals = journal_files(&home);
    assert_eq!(journals.len(), 1);
    let log = std::path::Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    let o = ew_isolated(&home, &["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(o.status.code(), Some(2), "{}{}", stdout(&o), stderr(&o));
    assert_eq!(
        stderr(&o),
        format!(
            "error: {}: a crashed session left unsaved changes in its journal; recover or discard them first: discard \
             it in ElektraWatt (File > Discard unsaved changes of the earlier session) or wait for `ew recover` \
             (FT-252); nothing was applied\n",
            project.display()
        )
    );
    assert!(tree(&project) == before, "nothing is applied");
    // FM-5 (FT-253): the other session's journal is still there, unchanged. The second half, a clean close deletes only
    // its own segment, is `Journal::close` (`crates/ew-store/src/journal.rs`), shown by
    // `recovery_replays_the_newest_journal_and_keeps_the_other_unchanged` (`crates/ew-store/tests/recovery.rs`) and
    // `the_app_path_recovers_one_journal_at_a_time` (`crates/ew-api/tests/recovery.rs`).
    assert!(journal_files(&home) == journals, "the journal is still there");
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

/// WP-S1.18 part 3 (D10): `ew revision approve|release|withdraw` run human-only commands, confirmed by the typed
/// revision label; without a terminal (stdout captured here) each exits 2 before opening the project, and nothing is
/// written. The typed confirmation is a unit test with an injected reader and writer.
#[test]
fn revision_commands_are_refused_off_a_terminal() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let dir = golden_copy(&t);
    let manifest = std::fs::read(dir.join("manifest.json")).unwrap();
    let dir_arg = dir.to_str().unwrap();
    for args in [
        &["revision", "approve", dir_arg, "--as", "checker"][..],
        &["revision", "release", "V2.0", dir_arg],
        &["revision", "withdraw", "V1.0", dir_arg, "--reason", "wrong"],
    ] {
        let args: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
        let o = ew_isolated(&home, &args);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}{}", stdout(&o), stderr(&o));
        assert!(
            stderr(&o).contains("HumanOnly: needs an interactive terminal"),
            "{}",
            stderr(&o)
        );
        assert!(!dir.join(".~ew.lock").exists(), "the project was not opened");
    }
    assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), manifest);
    let o = ew_isolated(&home, &["revision".as_ref(), "approve".as_ref(), dir.as_os_str()]);
    assert_eq!(o.status.code(), Some(2), "`--as` is required: {}", stderr(&o));
}

/// The recorded logs replay through `ew apply`: one log on a copy of Golden A, applied once, of
/// - WP-S2.04 part 4a (§12 item 2): `project.drawing.set` (`marker_mode`), `box.structure`, `item.place`,
///   `marker.place` and `rep.move`;
/// - WP-S2.05 part 1, PG10 (§9 item 2): the property grid pilot's `--record` log (`pilot_log.rs`, which the headless
///   test of `elektrawatt` proves is the record of its script and replays in-process to the records it saved);
/// - WP-S2.05 part 2, FR6 (§9 item 2): the find-and-replace script's `--record` log (`find_log.rs`: three
///   `fields.set_many` lines, which the headless test of `elektrawatt` proves is the record of its script).
///
/// Every line applies and the saved records are those the same log gives in-process (the spec's replay path), the
/// minted page items and structure boxes compared by content.
#[test]
fn the_recorded_logs_replay_through_ew_apply() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let (golden, _) = ew_api::Session::open_read_only(&project, "test").unwrap();
    let tables = golden.page_tables("101").unwrap();
    let page = tables.page.to_string();
    let wires: Vec<String> = tables
        .connections
        .iter()
        .filter(|c| c.kind == "wire")
        .take(2)
        .map(|c| c.id.to_string())
        .collect();
    let rep = tables.reps[0].id.to_string();
    drop(golden);
    let header = "{\"$schema\":\"ew.cmdlog/1\",\"label\":\"logs\"}";
    let mut lines = vec![
        serde_json::json!({"cmd": "project.drawing.set", "params": {"marker_mode": "explicit"}}),
        serde_json::json!({"cmd": "box.structure", "params": {"page": page,
            "outline": [[0, 0], [10_000_000, 0], [10_000_000, 10_000_000]], "style": "dotted", "caption": "Box",
            "assigns": {}}}),
        serde_json::json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": "40x60", "at": [50_800_000, 30_480_000]}}}}),
        serde_json::json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"graphic": {"shape": {"line": {"a": [0, 0], "b": [10_000_000, 0]}}, "dash": "dashed"}}}}),
        serde_json::json!({"cmd": "marker.place", "params": {"page": page, "connections": wires,
            "at": [60_000_000, 40_000_000]}}),
    ];
    let text = |lines: &[serde_json::Value]| {
        let mut out = String::from(header);
        for l in lines {
            out.push('\n');
            out.push_str(&l.to_string());
        }
        // The pilot's and the find-and-replace script's recorded lines follow the page-item lines.
        for l in pilot_log::PILOT_LOG.lines().chain(find_log::FIND_LOG.lines()) {
            out.push('\n');
            out.push_str(l);
        }
        out.push('\n');
        out
    };
    // A minted record cannot be named by a later line (no reference names a page item), so `rep.move`'s `items`
    // is replayed by `ew-commands`' tests; here the representation moves alone after the items are placed.
    lines.push(serde_json::json!({"cmd": "rep.move", "params": {"reps": [rep], "delta": [7_937_500, 0]}}));
    let text = text(&lines);
    let log = t.0.join("logs.ewcmd");
    std::fs::write(&log, &text).unwrap();
    let applied = ew_isolated(&home, &["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    assert_eq!(find_log::FIND_LOG.lines().count(), 3);
    let n = 6 + pilot_log::PILOT_LOG.lines().count() + find_log::FIND_LOG.lines().count();
    assert!(
        stdout(&applied).contains(&format!("applied {n} of {n} lines; saved")),
        "{}",
        stdout(&applied)
    );
    let fresh = t.0.join("fresh.ew");
    copy_dir(std::path::Path::new(GOLDEN_A), &fresh);
    let (mut replayed, _) = ew_api::Session::open_read_only(&fresh, "test").unwrap();
    replayed
        .run_script(&ew_api::CmdLog::parse(&text).unwrap(), "logs.ewcmd")
        .unwrap();
    let (saved, _) = ew_api::Session::open_read_only(&project, "test").unwrap();
    let diff = replayed.diff_from(&saved.world()).unwrap();
    // A log line cannot name a minted page item or structure box, so the `ew apply` run and the in-process replay
    // mint different IDs for both kinds; `model_lines` lists them since the generated key list, so the diff reports
    // each of them as removed and added. No other record may differ, and the items and boxes themselves are equal by
    // content, IDs aside.
    use ew_api::ChangeKind;
    let mut removed: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut added: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for change in &diff.model.changes {
        let kind = change.key.0.name();
        assert!(
            matches!(kind, "page_item" | "structure_box"),
            "the replay changed a {kind} record: {change:?}"
        );
        match change.change {
            ChangeKind::Removed => *removed.entry(kind).or_default() += 1,
            ChangeKind::Added => *added.entry(kind).or_default() += 1,
            ChangeKind::Changed => panic!("a {kind} record differs by members: {change:?}"),
        }
    }
    assert_eq!(
        removed, added,
        "per kind, the removed and added minted records balance"
    );
    // The two projects' items and boxes on 101 by content: a marker's connections are the golden connection IDs,
    // which the log names, so they are equal on both sides.
    let contents = |s: &ew_api::Session| {
        let t = s.page_tables("101").expect("page 101");
        let mut items: Vec<_> = t
            .items
            .iter()
            .map(|i| (i.kind.clone(), i.bounds, i.connections.clone()))
            .collect();
        items.sort();
        let mut boxes: Vec<_> = t
            .structure_boxes
            .iter()
            .map(|b| (b.outline.clone(), b.style.clone(), b.caption.clone()))
            .collect();
        boxes.sort();
        (items, boxes)
    };
    assert_eq!(
        contents(&replayed),
        contents(&saved),
        "the page items and structure boxes by content, IDs aside"
    );
    let saved_tables = saved.page_tables("101").unwrap();
    assert_eq!(saved_tables.items.len(), 3);
    assert_eq!(saved_tables.structure_boxes.len(), 1);
}

const GOLDEN_PARTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../golden/J-E-2416/tables/parts.csv"
);

/// A fresh saved project without parts (FT-410's import target).
fn parts_target(t: &Tmp, name: &str) -> PathBuf {
    let project = serde_json::from_str(
        r#"{"id":"3b9e51c4-8a27-7d10-b5f3-6c0a2e9d4f18","derivation_profile":"2027.1"}"#,
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
    let dir = t.0.join(name);
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    session.create(&dir, store).unwrap();
    session.close();
    dir
}

/// The part copies of a `lib/parts` folder by `item_id`, without the members that legitimately differ between
/// projects (ids, hash, provenance, the library of the manufacturer and of an enclosure's plate part).
fn comparable_parts(dir: &std::path::Path) -> std::collections::BTreeMap<String, serde_json::Value> {
    let mut out = std::collections::BTreeMap::new();
    for (_, bytes) in tree(dir) {
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let item = v["item_id"].as_str().unwrap().to_owned();
        let o = v.as_object_mut().unwrap();
        for k in ["id", "library_id", "sha256", "sources"] {
            o.remove(k);
        }
        if let Some(m) = o.get_mut("meta").and_then(|m| m.as_object_mut()) {
            m.remove("sources");
        }
        if let Some(m) = o.get_mut("manufacturer").and_then(|m| m.as_object_mut()) {
            m.remove("library");
        }
        // The enclosure section's plate part is project-local too (WP-S4.01 D13).
        if let Some(p) = o
            .get_mut("sections")
            .and_then(|s| s.get_mut("enclosure"))
            .and_then(|e| e.get_mut("plate"))
            .and_then(|p| p.as_object_mut())
        {
            p.remove("library");
        }
        assert!(out.insert(item, v).is_none(), "one copy per item");
    }
    out
}

/// FT-410 acceptance a: `ew import parts` creates the golden project's 29 part copies, and the recorded command
/// replays through `ew apply` to the same bytes.
#[test]
fn import_parts_creates_the_golden_part_copies_and_replays_through_apply() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let a = parts_target(&t, "parts-a.ew");
    let b = t.0.join("parts-b.ew");
    copy_dir(&a, &b);
    let o = ew_isolated(
        &home,
        &[
            "import".as_ref(),
            "parts".as_ref(),
            GOLDEN_PARTS.as_ref(),
            "--project".as_ref(),
            a.as_os_str(),
        ],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        stdout(&o),
        "imported parts.csv: 29 created, 0 skipped, 0 errors; saved\n"
    );
    let golden = std::path::Path::new(GOLDEN_A).join("lib/parts");
    let mine = tree(&a.join("lib/parts"));
    assert_eq!(mine.len(), 29);
    assert_eq!(mine.len(), tree(&golden).len());
    assert_eq!(comparable_parts(&a.join("lib/parts")), comparable_parts(&golden));

    let id = {
        let (session, _) = ew_api::Session::open_reader(&b, "test", "test").expect("the copy opens");
        let id = session.query().project().id.as_uuid();
        session.close();
        id
    };
    let parts = ew_api::import::read_parts_tables(std::path::Path::new(GOLDEN_PARTS), id).unwrap();
    let log = format!(
        "{}\n{}\n",
        r#"{"$schema":"ew.cmdlog/1","label":"test"}"#,
        serde_json::json!({"cmd": "parts.import", "params": parts.params()})
    );
    let log_path = t.0.join("parts.ewcmd");
    std::fs::write(&log_path, log).unwrap();
    let o = ew_isolated(&home, &["apply".as_ref(), b.as_os_str(), log_path.as_os_str()]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), stderr(&o));
    assert!(
        tree(&a.join("lib/parts")) == tree(&b.join("lib/parts")),
        "the replay differs"
    );
}

/// FT-410 acceptance b: a second import skips every key and changes nothing.
#[test]
fn a_second_import_parts_skips_every_key() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let a = parts_target(&t, "parts-a.ew");
    let run = || {
        ew_isolated(
            &home,
            &[
                "import".as_ref(),
                "parts".as_ref(),
                GOLDEN_PARTS.as_ref(),
                "--project".as_ref(),
                a.as_os_str(),
            ],
        )
    };
    assert_eq!(run().status.code(), Some(0));
    let before = tree(&a.join("lib"));
    let o = run();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        stdout(&o),
        "imported parts.csv: 0 created, 29 skipped, 0 errors; saved\n"
    );
    assert!(
        tree(&a.join("lib")) == before,
        "a second import changed the library"
    );
}

/// FT-410 acceptance c: a refused table names its line and column and writes nothing.
#[test]
fn import_parts_refusals_name_line_and_column() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let a = parts_target(&t, "parts-a.ew");
    let before = tree(&a);
    let run = |file: &std::path::Path| {
        ew_isolated(
            &home,
            &[
                "import".as_ref(),
                "parts".as_ref(),
                file.as_os_str(),
                "--project".as_ref(),
                a.as_os_str(),
            ],
        )
    };
    let d1 = t.0.join("t1");
    std::fs::create_dir_all(&d1).unwrap();
    std::fs::write(d1.join("parts.csv"), "key,part_number,kind\nA,A1,device\n").unwrap();
    let o = run(&d1.join("parts.csv"));
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("line 1") && stderr(&o).contains("manufacturer"),
        "{}",
        stderr(&o)
    );
    let d2 = t.0.join("t2");
    std::fs::create_dir_all(&d2).unwrap();
    std::fs::write(
        d2.join("parts.csv"),
        "key,manufacturer,part_number,kind\nA,Acme,A1,device\nB,Acme,B1,gizmo\n",
    )
    .unwrap();
    let o = run(&d2.join("parts.csv"));
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("line 3") && stderr(&o).contains("column `kind`"),
        "{}",
        stderr(&o)
    );
    assert!(tree(&a) == before, "a refused table changed the project");
    let o = run(&t.0.join("missing").join("parts.csv"));
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

/// FT-504 (beside JB8's `ew-api` test): `ew import parts` with a `parts.csv` and sibling tables that a spreadsheet
/// saved with a byte-order mark creates the same 29 part copies as the golden tables without it.
#[test]
fn import_parts_with_a_bom_prefixed_table_creates_the_same_copies() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let tables = std::path::Path::new(GOLDEN_PARTS).parent().unwrap();
    let marked = t.0.join("bom-tables");
    std::fs::create_dir_all(&marked).unwrap();
    for file in [
        "parts.csv",
        "part_cps.csv",
        "part_templates.csv",
        "cable_cores.csv",
        "part_plc.csv",
    ] {
        let text = std::fs::read_to_string(tables.join(file)).unwrap();
        assert!(!text.starts_with('\u{feff}'), "{file} has no BOM of its own");
        std::fs::write(marked.join(file), format!("\u{feff}{text}")).unwrap();
    }
    let import = |table: &std::path::Path, project: &std::path::Path| {
        ew_isolated(
            &home,
            &[
                "import".as_ref(),
                "parts".as_ref(),
                table.as_os_str(),
                "--project".as_ref(),
                project.as_os_str(),
            ],
        )
    };
    let plain = parts_target(&t, "parts-plain.ew");
    let o = import(std::path::Path::new(GOLDEN_PARTS), &plain);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let bom = parts_target(&t, "parts-bom.ew");
    let o = import(&marked.join("parts.csv"), &bom);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        stdout(&o),
        "imported parts.csv: 29 created, 0 skipped, 0 errors; saved\n"
    );
    assert!(stderr(&o).is_empty(), "{}", stderr(&o));
    assert_eq!(tree(&bom.join("lib/parts")).len(), 29);
    assert_eq!(
        comparable_parts(&bom.join("lib/parts")),
        comparable_parts(&plain.join("lib/parts")),
        "the same part copies as without the BOM"
    );
}

/// `ew lib` (WP-S1.19 part 2): create a user library, list the tiers, search the starter, rebuild the index; the
/// index lives in the cache directory.
#[test]
fn lib_create_list_search_and_rebuild() {
    let tmp = Tmp::new();
    let data = tmp.0.join("data");
    let cache = tmp.0.join("cache");
    let run = |args: &[&str]| {
        Command::new(bin_exe())
            .args(args)
            .env("EW_CONTENT_DIR", CONTENT)
            .env("EW_DATA_DIR", &data)
            .env("EW_CACHE_DIR", &cache)
            .output()
            .unwrap()
    };
    let mine = data.join("libraries").join("mine.ewlib");
    let mine_arg = mine.to_str().unwrap();
    let o = run(&[
        "lib",
        "create",
        mine_arg,
        "--key",
        "mine",
        "--name",
        "Mine",
        "--licence",
        "CC0-1.0",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).starts_with("created mine ("), "{}", stdout(&o));
    assert!(mine.join("library.json").is_file());
    // The same directory again: not empty.
    let o = run(&[
        "lib",
        "create",
        mine_arg,
        "--key",
        "mine",
        "--name",
        "Mine",
        "--licence",
        "CC0-1.0",
    ]);
    assert_eq!(o.status.code(), Some(2));
    let o = run(&["lib", "list", "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let list: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    // CL-4 (WP-S1.22 D3 (d)): an object naming its schema, the libraries under `libraries`.
    assert_eq!(list["$schema"], "ew.lib-list-result/1");
    let rows: Vec<(String, String, bool)> = list["libraries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["key"].as_str().unwrap().to_owned(),
                l["tier"].as_str().unwrap().to_owned(),
                l["writable"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("ew-starter".to_owned(), "system".to_owned(), false),
            ("ew-starter-parts".to_owned(), "system".to_owned(), false),
            ("mine".to_owned(), "user".to_owned(), true),
        ]
    );
    let o = run(&["lib", "list"]);
    assert!(stdout(&o).contains("user\tmine\t0.1.0\tMine\t"), "{}", stdout(&o));
    let o = run(&["lib", "search", "ew.fn.mcb-1pn", "--kind", "fndef"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).starts_with("fndef\tew.fn.mcb-1pn\t"), "{}", stdout(&o));
    let o = run(&["lib", "search", "A9P52610"]);
    assert_eq!((o.status.code(), stdout(&o).as_str()), (Some(0), ""));
    assert_eq!(stderr(&o).trim(), "nothing found");
    let o = run(&["lib", "index", "--rebuild"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    // The count includes the compiled-in starter parts (WP-S2.18 D5), so it is not pinned.
    assert!(
        stdout(&o).starts_with("index rebuilt: ") && stdout(&o).contains(" parts, "),
        "{}",
        stdout(&o)
    );
    assert!(cache.join("library-index-v1.sqlite").is_file());
    // A library in the system tier is refused.
    let system = std::path::Path::new(CONTENT).join("libraries").join("new.ewlib");
    let o = run(&[
        "lib",
        "create",
        system.to_str().unwrap(),
        "--key",
        "n",
        "--name",
        "N",
        "--licence",
        "CC0-1.0",
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(!system.exists());
}

/// A supplier list with its own headers (invented generic parts and public catalogue numbers).
const SUPPLIER_LIST: &str = "Maker,Order code,Text,Category\n\
Schneider Electric,A9P52610,Circuit breaker,device\n\
Phoenix Contact,3209510,Feed-through terminal,terminal\n\
Phoenix Contact,3209510,Feed-through terminal,terminal\n\
Generic Panels,,No number,device\n";

const SUPPLIER_MAP: &str = "[columns]\nmanufacturer = \"Maker\"\npart_number = \"Order code\"\n\
description = \"Text\"\nkind = \"Category\"\n";

/// WP-S1.19 part 4: `ew lib import parts` writes a mapped list into a user library, refuses changed content under
/// the published version unless `--bump`, and `ew import parts --map` imports the same list into a project.
#[test]
fn lib_import_parts_and_import_parts_with_a_map() {
    let tmp = Tmp::new();
    let home = tmp.0.join("home");
    let data = tmp.0.join("data");
    let cache = tmp.0.join("cache");
    let run = |args: &[&std::ffi::OsStr]| {
        let mut cmd = Command::new(bin_exe());
        cmd.args(args)
            .env_remove("EW_CONTENT_DIR")
            .env("EW_DATA_DIR", &data)
            .env("EW_CACHE_DIR", &cache);
        for d in [
            "HOME",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "XDG_STATE_HOME",
            "XDG_DATA_HOME",
        ] {
            let dir = home.join(d.to_ascii_lowercase());
            std::fs::create_dir_all(&dir).unwrap();
            cmd.env(d, &dir);
        }
        cmd.output().unwrap()
    };
    let lib = data.join("libraries").join("std.ewlib");
    let o = run(&[
        "lib".as_ref(),
        "create".as_ref(),
        lib.as_os_str(),
        "--key".as_ref(),
        "std".as_ref(),
        "--name".as_ref(),
        "Standard".as_ref(),
        "--licence".as_ref(),
        "CC0-1.0".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let list = tmp.0.join("supplier.csv");
    let map = tmp.0.join("supplier.toml");
    std::fs::write(&list, SUPPLIER_LIST).unwrap();
    std::fs::write(&map, SUPPLIER_MAP).unwrap();
    let import = |extra: &[&str]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "lib".as_ref(),
            "import".as_ref(),
            "parts".as_ref(),
            "std".as_ref(),
            list.as_os_str(),
            "--map".as_ref(),
            map.as_os_str(),
            "--as-of".as_ref(),
            "2026-10-29".as_ref(),
        ];
        args.extend(extra.iter().map(std::ffi::OsStr::new));
        run(&args)
    };
    let o = import(&[]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(
        out.contains("EW-IMP-002 error: line 5: "),
        "the rejected row with its line: {out}"
    );
    assert!(
        out.ends_with(
            "imported supplier.csv into std: 2 new, 0 updated, 0 unchanged, 1 duplicate, 1 rejected; \
             written: 4 added, 0 updated, 0 unchanged (generation 2)\n"
        ),
        "{out}"
    );
    // The changed list: refused without `--bump`, nothing written; written as 1.0.1 with it.
    std::fs::write(
        &list,
        SUPPLIER_LIST.replace("Circuit breaker", "Miniature circuit breaker"),
    )
    .unwrap();
    let o = import(&[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("line 2: A9P52610 (1.0.0)") && stderr(&o).contains("`--bump`"),
        "{}",
        stderr(&o)
    );
    let o = import(&["--bump"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        stdout(&o).contains(
            ": 0 new, 1 updated, 1 unchanged, 1 duplicate, 1 rejected; written: 0 added, 1 updated"
        ),
        "{}",
        stdout(&o)
    );
    let o = run(&["lib".as_ref(), "search".as_ref(), "A9P52610".as_ref()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("A9P52610"), "{}", stdout(&o));
    // An unknown library is refused.
    let o = run(&[
        "lib".as_ref(),
        "import".as_ref(),
        "parts".as_ref(),
        "nope".as_ref(),
        list.as_os_str(),
        "--map".as_ref(),
        map.as_os_str(),
        "--as-of".as_ref(),
        "2026-10-29".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("no library `nope`"), "{}", stderr(&o));

    // Into a project: the manufacturers resolve through the library written above.
    let project = parts_target(&tmp, "mapped.ew");
    let into_project = || {
        run(&[
            "import".as_ref(),
            "parts".as_ref(),
            list.as_os_str(),
            "--map".as_ref(),
            map.as_os_str(),
            "--as-of".as_ref(),
            "2026-10-29".as_ref(),
            "--project".as_ref(),
            project.as_os_str(),
        ])
    };
    let o = into_project();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        stdout(&o).ends_with(
            "imported supplier.csv: 2 new, 0 updated, 0 unchanged, 1 duplicate, 1 rejected; 2 created, 0 skipped; saved\n"
        ),
        "{}",
        stdout(&o)
    );
    assert_eq!(tree(&project.join("lib/parts")).len(), 2);
    let o = into_project();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        stdout(&o).contains(": 0 new, 0 updated, 2 unchanged, 1 duplicate, 1 rejected; 0 created, 0 skipped"),
        "{}",
        stdout(&o)
    );
}

// ---------------------------------------------------------------------------------------------------------
// AC-21 through the CLI (WP-S1.22 part 1, CL-6)

/// The built packs of AC-21's subset, as the `ew-api` test `ac21_four_pack_subset_equals_findings_mvp_csv` names them.
const AC21_PACKS: [&str; 6] = ["EW-CORE", "EW-IEC81346", "EW-TERM", "EW-POT", "EW-PLC", "EW-BUS"];

/// The open findings of the built packs that doc 20 §18.1 does not list (FT-638: the process valves are supplied by
/// others and exempt once `supply_scope` exists, FT-209), as the `ew-api` test holds them.
const AC21_KNOWN_EXCESS: [(&str, &str); 8] = [
    ("EW-CORE-041", "=P01+FIELD-203Q1"),
    ("EW-CORE-041", "=P01+FIELD-203Q2"),
    ("EW-CORE-041", "=P01+FIELD-203Q3"),
    ("EW-CORE-041", "=P01+FIELD-203Q4"),
    ("EW-CORE-041", "=P01+FIELD-203Q5"),
    ("EW-CORE-041", "=P01+FIELD-203Q6"),
    ("EW-CORE-041", "=P01+FIELD-203Q7"),
    ("EW-CORE-041", "=P01+FIELD-203Q8"),
];

/// Splits one line of `findings_mvp.csv` into fields: commas outside double quotes separate; `""` is a quote.
fn csv_fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let (mut quoted, mut chars) = (false, line.chars().peekable());
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                chars.next();
                out.last_mut().unwrap().push('"');
            }
            ('"', _) => quoted = !quoted,
            (',', false) => out.push(String::new()),
            (c, _) => out.last_mut().unwrap().push(c),
        }
    }
    out
}

/// A copy of Golden A with W-1 (doc 20 §18.1 EF-19: the EW-CORE-041 finding on -XPE waived) recorded from the GUI
/// origin and saved, as the `ew-api` fixture `golden_a_with_w1` makes it.
fn golden_a_with_w1(t: &Tmp) -> PathBuf {
    use ew_api::checks::{CheckMode, PRE_DELIVERY};
    let copy = golden_copy(t);
    let mut store = ew_api::StoreOptions::new("cl-6");
    store.state_dir = Some(t.0.join("w1-state"));
    let (mut s, _) = ew_api::Session::open(
        &copy,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    let fingerprint = s
        .check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .iter()
        .find(|f| f.rule == "EW-CORE-041" && f.subject == "=P01+J-E-2416-XPE")
        .expect("EW-CORE-041 on -XPE")
        .fingerprint
        .to_string();
    let waive = ew_api::AnyCommand {
        name: "check.waive".into(),
        params: serde_json::json!({
            "rule": "EW-CORE-041", "fingerprint": fingerprint,
            "reason": {"en-GB": "PE stud is part of NSYPMM107"}, "author": "engineer", "date": "2025-04-14"
        }),
    };
    s.execute_with(&waive, ew_api::Submit::from(ew_api::Origin::Gui))
        .expect("W-1 commits from the GUI");
    s.save().expect("the copy saves");
    s.close();
    copy
}

/// CL-6 (AC-21 through the CLI): on Golden A's copy with W-1, `ew check --scheme pre-delivery --json` exits 1, and its
/// findings of the built packs equal the rows of `findings_mvp.csv` that are not pending, with FT-638's known
/// excess, as the `ew-api` test asserts on the library. The pending rows are listed by package; each names a rule no
/// pack of this build reports. When the last pending package lands, every row is compared (doc 20 §18.1's 42 errors,
/// 32 warnings, 31 infos and 1 waived) and the pending list is empty.
#[test]
fn ac21_through_the_cli_equals_findings_mvp_csv_for_the_built_packs() {
    type Groups = std::collections::BTreeMap<(String, String), Vec<String>>;
    let t = Tmp::new();
    let copy = golden_a_with_w1(&t);
    let o = ew_isolated(
        &t.0.join("home"),
        &[
            "check".as_ref(),
            "--scheme".as_ref(),
            "pre-delivery".as_ref(),
            "--json".as_ref(),
            "--project".as_ref(),
            copy.as_os_str(),
        ],
    );
    assert_eq!(
        o.status.code(),
        Some(1),
        "doc 20 §18.1: `ew check` exits 1: {}",
        stderr(&o)
    );
    let result: ew_api::cli_results::CheckResult = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(ew_api::cli_results::to_text(&result), stdout(&o));
    let pack_of: std::collections::BTreeMap<&str, &str> = result
        .rules
        .iter()
        .map(|r| (r.id.as_str(), r.pack.as_str()))
        .collect();
    let mut got = Groups::new();
    for f in &result.findings {
        if !pack_of
            .get(f.rule.as_str())
            .is_some_and(|p| AC21_PACKS.contains(p))
        {
            continue;
        }
        let open = f.state == ew_api::checks::report::ResultState::Open;
        if open
            && AC21_KNOWN_EXCESS
                .iter()
                .any(|(r, s)| *r == f.rule && *s == f.subject)
        {
            continue;
        }
        let state = if open {
            serde_json::to_value(f.severity)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        } else {
            "waived".to_owned()
        };
        got.entry((f.rule.clone(), state))
            .or_default()
            .push(f.subject.clone());
    }
    for v in got.values_mut() {
        v.sort();
    }
    let csv = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/ew-api/tests/expected/findings_mvp.csv"),
    )
    .unwrap();
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("ef,rule,severity,count,subjects,targets,defect,pending")
    );
    let mut want = Groups::new();
    let mut pending: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for line in lines {
        let f = csv_fields(line);
        assert_eq!(f.len(), 8, "{line}");
        if f[7].is_empty() {
            let mut subjects: Vec<String> = f[4]
                .split("; ")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            subjects.sort();
            want.insert((f[1].clone(), f[2].clone()), subjects);
        } else {
            assert!(
                !pack_of.contains_key(f[1].as_str()),
                "{} {} is run by this build: compare its row (clear its pending column)",
                f[0],
                f[1]
            );
            pending
                .entry(f[7].clone())
                .or_default()
                .push(format!("{} {}", f[0], f[1]));
        }
    }
    for (package, rows) in &pending {
        println!("AC-21 rows pending {package}: {}", rows.join(", "));
    }
    assert_eq!(got, want, "AC-21's built-pack subset through `ew check --json`");
    for (rule, subject) in AC21_KNOWN_EXCESS {
        assert!(
            result
                .findings
                .iter()
                .any(|f| f.rule == rule && f.subject == subject),
            "{rule} on {subject} is gone: update AC21_KNOWN_EXCESS with FT-638"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// WP-S1.22 part 3: `ew fmt`, `ew recover` and `ew import csv`

/// The journal files of the isolated `ew` (`home`) with their bytes.
fn journal_files(home: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let dir = isolated_state_dir(home);
    if !dir.exists() {
        return std::collections::BTreeMap::new();
    }
    tree(&dir)
        .into_iter()
        .filter(|(name, _)| name.ends_with(".jsonl"))
        .collect()
}

/// A session on `project` with the journals of the isolated `ew` (`home`) that runs `cmds` and is dropped without a
/// save or a clean close: its journal stays, as a crashed session's does.
fn crashed_session(home: &std::path::Path, project: &std::path::Path, cmds: &[ew_api::AnyCommand]) {
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = Some(isolated_state_dir(home));
    let (mut session, opened) = ew_api::Session::open(
        project,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    assert_eq!(opened.access, ew_api::Access::Edit);
    for c in cmds {
        session.execute(c).unwrap();
    }
    drop(session);
}

fn new_page(number: u32) -> ew_api::AnyCommand {
    ew_api::AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({"number": number, "title": {"en": "T"}}),
    }
}

/// The page labels of the saved project.
fn saved_pages(project: &std::path::Path) -> Vec<String> {
    let (s, _) = ew_api::Session::open_reader(project, "test", "test").unwrap();
    let labels = s.page_labels().to_vec();
    s.close();
    labels
}

/// The files of a project directory but its manifest, and the manifest's `files` member.
fn files_and_hashes(
    dir: &std::path::Path,
) -> (std::collections::BTreeMap<String, Vec<u8>>, serde_json::Value) {
    let mut files = tree(dir);
    let manifest: serde_json::Value =
        serde_json::from_slice(&files.remove("manifest.json").expect("a manifest")).unwrap();
    (files, manifest["files"].clone())
}

/// `ew <args>` isolated under `home`, with `extra` after them.
fn ew_in(home: &std::path::Path, args: &[&std::ffi::OsStr], extra: &[&str]) -> Output {
    let mut all: Vec<&std::ffi::OsStr> = args.to_vec();
    all.extend(extra.iter().map(std::ffi::OsStr::new));
    ew_isolated(home, &all)
}

/// Swaps the first two lines of the project file `rel`.
fn swap_first_lines(project: &std::path::Path, rel: &str) {
    let file = project.join(rel);
    let text = std::fs::read_to_string(&file).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    lines.swap(0, 1);
    std::fs::write(&file, format!("{}\n", lines.join("\n"))).unwrap();
}

/// FM-1 (FR-EXT-008): `ew fmt --check` on Golden A's copy exits 0; after two lines of `devices/71.jsonl` are swapped it
/// exits 1 naming that file and writes nothing; `ew fmt` rewrites that file only (ruling 6 of cycle 41: "`ew fmt`
/// writes only the rewrite set: save through the plan that starts from `Loaded.rewrite`, never `Changes::All`"); then
/// `ew fmt --check` exits 0 and the files equal the original bytes. The manifest is a new generation by necessity; its
/// file hashes equal the original ones.
#[test]
fn fmt_check_names_a_swapped_file_and_fmt_restores_its_bytes() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let fmt = |extra: &[&str]| ew_in(&home, &["fmt".as_ref(), project.as_os_str()], extra);
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    let original = files_and_hashes(&project);
    assert_eq!(
        run(&fmt(&["--check"])),
        (
            Some(0),
            String::new(),
            "fmt: every file is in canonical form\n".to_owned()
        )
    );

    swap_first_lines(&project, "devices/71.jsonl");
    let swapped = tree(&project);
    assert_eq!(
        run(&fmt(&["--check"])),
        (
            Some(1),
            "would rewrite devices/71.jsonl\n".to_owned(),
            "fmt: 1 file is not in canonical form; nothing was written\n".to_owned()
        )
    );
    assert!(tree(&project) == swapped, "`--check` writes nothing");

    assert_eq!(
        run(&fmt(&[])),
        (
            Some(0),
            "rewrote devices/71.jsonl\n".to_owned(),
            "fmt: rewrote 1 file; saved generation 2\n".to_owned()
        )
    );
    assert_eq!(
        run(&fmt(&["--check"])),
        (
            Some(0),
            String::new(),
            "fmt: every file is in canonical form\n".to_owned()
        )
    );
    assert!(
        files_and_hashes(&project) == original,
        "the files equal the original bytes"
    );
    let after = tree(&project);
    let changed: Vec<&String> = after
        .keys()
        .filter(|k| after.get(*k) != swapped.get(*k))
        .collect();
    assert_eq!(
        changed,
        ["devices/71.jsonl", "manifest.json"],
        "only the rewrite set and the manifest"
    );
    assert!(journal_files(&home).is_empty(), "the format's journal is closed");
}

/// FM-2 (ADR-0011's preservation): a line with an unknown member survives `ew fmt` verbatim, and a quarantined line
/// (EW-CORE-007, a schema violation) is listed as kept and stays in its file, while the file they share is rewritten
/// for a swap. A line that is not JSON (EW-CORE-005) makes the project read-only: `ew fmt --check` lists it, and `ew
/// fmt` is refused and writes nothing, so no line it cannot parse is lost.
#[test]
fn fmt_keeps_unknown_members_and_quarantined_lines_verbatim() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let rel = "devices/71.jsonl";
    let text = std::fs::read_to_string(project.join(rel)).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let third = lines[2].strip_suffix('}').unwrap().to_owned();
    let unknown = format!("{third},\"zz_future\":{{\"a\":[1,2]}}}}");
    lines[2] = unknown.clone();
    let quarantined = r#"{"$k":"device","id":"0e9d8b7a-6c5d-7e4f-8a3b-2c1d0e9f8a71","tag":5}"#;
    lines.push(quarantined.to_owned());
    lines.swap(0, 1);
    std::fs::write(project.join(rel), format!("{}\n", lines.join("\n"))).unwrap();
    let at = lines.len();

    let (_, report) = ew_api::Session::open_reader(&project, "test", "test").unwrap();
    let kept = ew_api::fmt::kept_lines(&report);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(
        (kept[0].file.as_str(), kept[0].line, kept[0].code.as_str()),
        (rel, Some(u32::try_from(at).unwrap()), "EW-CORE-007")
    );
    let kept_text = format!("kept {rel}:{at} (EW-CORE-007): {}\n", kept[0].reason);

    let fmt = |extra: &[&str]| ew_in(&home, &["fmt".as_ref(), project.as_os_str()], extra);
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    assert_eq!(
        run(&fmt(&["--check"])),
        (
            Some(1),
            format!("would rewrite {rel}\n"),
            format!("{kept_text}fmt: 1 file is not in canonical form; nothing was written\n")
        )
    );
    assert_eq!(
        run(&fmt(&[])),
        (
            Some(0),
            format!("rewrote {rel}\n"),
            format!("{kept_text}fmt: rewrote 1 file; saved generation 2\n")
        )
    );
    let written = std::fs::read_to_string(project.join(rel)).unwrap();
    let written: Vec<&str> = written.lines().collect();
    assert!(
        written.contains(&unknown.as_str()),
        "the unknown member's line, verbatim: {written:#?}"
    );
    assert!(
        written.contains(&quarantined),
        "the quarantined line, verbatim: {written:#?}"
    );
    assert_eq!(written.len(), lines.len(), "no line is lost");
    let o = fmt(&["--check"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));

    // A line that is not JSON: the reader lists it, the writer is refused (the project opens read-only).
    let broken = t.0.join("broken.ew");
    copy_dir(std::path::Path::new(GOLDEN_A), &broken);
    swap_first_lines(&broken, rel);
    let mut text = std::fs::read_to_string(broken.join(rel)).unwrap();
    text.push_str("{\"$k\":\"device\",\n");
    std::fs::write(broken.join(rel), &text).unwrap();
    let before = tree(&broken);
    let (_, report) = ew_api::Session::open_reader(&broken, "test", "test").unwrap();
    let kept = ew_api::fmt::kept_lines(&report);
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(kept[0].code, "EW-CORE-005");
    let kept_text = format!(
        "kept {rel}:{} (EW-CORE-005): {}\n",
        kept[0].line.unwrap(),
        kept[0].reason
    );
    let fmt = |extra: &[&str]| ew_in(&home, &["fmt".as_ref(), broken.as_os_str()], extra);
    assert_eq!(
        run(&fmt(&["--check"])),
        (
            Some(1),
            format!("would rewrite {rel}\n"),
            format!("{kept_text}fmt: 1 file is not in canonical form; nothing was written\n")
        )
    );
    assert_eq!(
        run(&fmt(&[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the project opens read-only (Integrity)\n",
                broken.display()
            )
        )
    );
    assert!(tree(&broken) == before, "nothing is written");
}

/// FM-3: `ew fmt` on a project with a crashed session's journal exits 2, pointing at `ew recover`, and writes nothing,
/// though a file is out of canonical form; the journal stays.
#[test]
fn fmt_refuses_a_project_with_a_crashed_session_journal() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    swap_first_lines(&project, "devices/71.jsonl");
    crashed_session(&home, &project, &[new_page(7777)]);
    let before = tree(&project);
    let journals = journal_files(&home);
    assert_eq!(journals.len(), 1);
    let o = ew_isolated(&home, &["fmt".as_ref(), project.as_os_str()]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with \
                 `ew recover`; nothing was written\n",
                project.display()
            )
        )
    );
    assert!(tree(&project) == before, "nothing is written");
    assert!(journal_files(&home) == journals, "the journal stays");
}

/// FM-4 (FT-252): `ew recover` prints the journal's summary and changes nothing; `--discard` off a terminal exits 2
/// before the project is opened (the typed paths are unit tests with the injected terminal, `cli/project_files.rs`);
/// `--replay` saves the journal's transactions and removes it.
#[test]
fn recover_summarises_and_replays_a_crashed_session_journal() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    crashed_session(&home, &project, &[new_page(7777), new_page(7778)]);
    let before = tree(&project);
    let journals = journal_files(&home);
    let recover = |extra: &[&str]| ew_in(&home, &["recover".as_ref(), project.as_os_str()], extra);
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    let summary = "a crashed session left 2 unsaved transactions in its journal:\n  1. page.create\n  2. page.create\nthe \
                   project is unchanged since that session last opened or saved it\n";
    assert_eq!(
        run(&recover(&[])),
        (
            Some(0),
            format!(
                "{summary}`ew recover --replay` replays and saves them; `ew recover --discard` deletes them\n"
            ),
            String::new()
        )
    );
    assert!(
        tree(&project) == before && journal_files(&home) == journals,
        "the summary changes nothing"
    );

    assert_eq!(
        run(&recover(&["--discard"])),
        (
            Some(2),
            String::new(),
            "error: HumanOnly: needs an interactive terminal (`recover --discard` is confirmed by typing `discard`; \
             `ew recover` prints the summary without one); nothing was discarded\n"
                .to_owned()
        )
    );
    assert!(
        tree(&project) == before && journal_files(&home) == journals,
        "off a terminal nothing changes"
    );

    assert_eq!(
        run(&recover(&["--replay"])),
        (
            Some(0),
            format!(
                "{summary}replayed 2 transactions of the crashed session's journal; saved generation 2\n"
            ),
            String::new()
        )
    );
    assert!(journal_files(&home).is_empty(), "the journal is removed");
    let pages = saved_pages(&project);
    assert!(
        pages.contains(&"7777".to_owned()) && pages.contains(&"7778".to_owned()),
        "{pages:?}"
    );
    assert_eq!(
        run(&recover(&[])),
        (
            Some(0),
            "no journal of a crashed session: nothing to recover\n".to_owned(),
            String::new()
        )
    );
}

/// Ruling 2 of cycle 41: "Replaying a segment whose base is not the open generation behaves exactly as the existing
/// code does for that case today (refused or applied)". A crashed session set page 1's title; another session set the
/// same title and saved; `ew recover --replay` is refused (exit 1) and keeps the journal and the project unchanged.
#[test]
fn recover_refuses_a_replay_that_collides_with_a_later_save() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    // A page's ID: its file name (`pages/<id>.jsonl`).
    let page = std::fs::read_dir(project.join("pages"))
        .unwrap()
        .map(|e| {
            let name = e.unwrap().file_name().to_string_lossy().into_owned();
            name.trim_end_matches(".jsonl").to_owned()
        })
        .min()
        .expect("a page");
    let title = |text: &str| ew_api::AnyCommand {
        name: "fields.set".into(),
        params: serde_json::json!({"kind": "page", "targets": [page], "path": "title", "value": {"en": text}}),
    };
    crashed_session(&home, &project, &[title("from the crashed session")]);
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    let (mut other, _) = ew_api::Session::open(
        &project,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    other.execute(&title("from a later session")).unwrap();
    other.save().unwrap();
    other.close();
    let before = tree(&project);
    let journals = journal_files(&home);

    let o = ew_isolated(
        &home,
        &["recover".as_ref(), project.as_os_str(), "--replay".as_ref()],
    );
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(1),
            "a crashed session left 1 unsaved transaction in its journal:\n  1. fields.set\nthe project was saved by \
             another session since; a replay is refused when it meets a field changed in that save\n"
                .to_owned(),
            "error: the replay is refused: 1 field of the journal changed on disk too; the journal is kept and \
             nothing was saved (`ew recover --discard` deletes it)\n"
                .to_owned()
        )
    );
    assert!(tree(&project) == before, "nothing is saved");
    assert!(journal_files(&home) == journals, "the journal is kept, unchanged");
}

/// Ruling 2 of cycle 41: "`ew recover` checks that it got edit access ... Then it refuses with exit 2 and the same
/// text `ew apply` uses for a held lock; it never acts read-only." While this test process holds the project's lock,
/// `ew recover` and `ew apply` exit 2 with the same text, `ew apply`'s as built: the read-only reason a second open
/// sees, with the lock's holder (FR-COL-006); the crashed session's journal stays.
#[test]
fn recover_and_apply_refuse_a_held_lock_with_the_same_text() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let project = golden_copy(&t);
    crashed_session(&home, &project, &[new_page(7777)]);
    let journals = journal_files(&home);
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    let (holder, opened) = ew_api::Session::open(
        &project,
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    assert_eq!(opened.access, ew_api::Access::Edit);
    // A second open in this process sees the lock as the binary does (the holder is alive and has another session).
    let mut second = ew_api::StoreOptions::new("test");
    second.state_dir = None;
    let (reader, seen) = ew_api::Session::open(
        &project,
        second,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .unwrap();
    let ew_api::Access::ReadOnly(reason @ ew_api::ReadOnlyReason::LockedBy(info)) = &seen.access else {
        panic!("the second open is read-only by the lock: {:?}", seen.access);
    };
    assert!(
        info.record
            .as_ref()
            .is_some_and(|r| !r.user.is_empty() && !r.host.is_empty()),
        "the reason names the holder: {info:?}"
    );
    let expected = format!(
        "error: {}: the project opens read-only ({reason:?})\n",
        project.display()
    );
    reader.close();
    let before = tree(&project);
    let log = std::path::Path::new(VARIANTS).join("b2-renumber-pages.ewcmd");
    for args in [
        vec!["recover".as_ref(), project.as_os_str()],
        vec!["recover".as_ref(), project.as_os_str(), "--replay".as_ref()],
        vec!["apply".as_ref(), project.as_os_str(), log.as_os_str()],
    ] {
        let o = ew_isolated(&home, &args);
        assert_eq!(
            (o.status.code(), stdout(&o), stderr(&o)),
            (Some(2), String::new(), expected.clone()),
            "{args:?}"
        );
    }
    assert!(tree(&project) == before, "nothing is written");
    assert!(journal_files(&home) == journals, "the journal stays");
    holder.close();
}

const GOLDEN_TABLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/tables");

/// A copy of the golden tables in `t` as `name`.
fn tables_copy(t: &Tmp, name: &str) -> PathBuf {
    let copy = t.0.join(name);
    copy_dir(std::path::Path::new(GOLDEN_TABLES), &copy);
    copy
}

/// What `ew import csv` prints on stdout for the import of `tables` in `mode` (the finding lines, from the library
/// itself), ending with `last` when given.
fn import_lines(tables: &std::path::Path, mode: ew_api::import::ImportMode, last: Option<&str>) -> String {
    let library = ew_api::import::StarterLibrary::from_dir(std::path::Path::new(CONTENT)).unwrap();
    let inputs = ew_api::import::read_tables(tables).unwrap();
    let batch = ew_api::import::import_tables(&inputs, &library, mode).unwrap();
    let mut out = String::new();
    for e in &batch.report.entries {
        let severity = match e.severity {
            ew_api::import::Severity::Info => "info",
            ew_api::import::Severity::Warning => "warning",
            ew_api::import::Severity::Error => "error",
        };
        let at = match &e.location {
            Some(l) => match l.line {
                Some(n) => format!("{}:{n}: ", l.file),
                None => format!("{}: ", l.file),
            },
            None => String::new(),
        };
        out.push_str(&format!("{} {severity}: {at}{}\n", e.code, e.message));
    }
    if let Some(last) = last {
        out.push_str(last);
        out.push('\n');
    }
    out
}

/// `ew import csv` with the starter content.
fn import_csv(
    home: &std::path::Path,
    tables: &std::path::Path,
    out: &std::path::Path,
    extra: &[&str],
) -> Output {
    let content = std::path::Path::new(CONTENT);
    ew_in(
        home,
        &[
            "import".as_ref(),
            "csv".as_ref(),
            tables.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--content".as_ref(),
            content.as_os_str(),
        ],
        extra,
    )
}

/// FM-6 (FT-27) through the binary: the golden tables import to a new project that is in canonical form and opens at
/// generation 1 (its model and check findings against the bootstrap are `crates/ew-api/tests/import_csv.rs`); the
/// files that differ from `golden/J-E-2416.ew` are printed (reported, not required). Without the starter content, or
/// into an `--out` that holds something, nothing is written (exit 2).
#[test]
fn import_csv_writes_the_golden_tables_as_a_new_project() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let tables = std::path::Path::new(GOLDEN_TABLES);
    let out = t.0.join("new.ew");
    let o = import_csv(&home, tables, &out, &[]);
    let n = ew_api::import::read_tables(tables).unwrap().len();
    let library = ew_api::import::StarterLibrary::from_dir(std::path::Path::new(CONTENT)).unwrap();
    let batch = ew_api::import::import_tables(
        &ew_api::import::read_tables(tables).unwrap(),
        &library,
        ew_api::import::ImportMode::Strict,
    )
    .unwrap();
    let last = format!(
        "imported {n} tables into {}: {} records, {} library documents",
        out.display(),
        batch.records.len(),
        batch.lib.len()
    );
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(0),
            import_lines(tables, ew_api::import::ImportMode::Strict, Some(&last)),
            String::new()
        )
    );
    let o = ew_isolated(&home, &["fmt".as_ref(), out.as_os_str(), "--check".as_ref()]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "the new project is canonical: {}",
        stderr(&o)
    );
    let (s, report) = ew_api::Session::open_reader(&out, "test", "test").unwrap();
    assert_eq!(report.generation, 1);
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    s.close();
    let written = tree(&out);
    let golden = tree(std::path::Path::new(GOLDEN_A));
    let differ: std::collections::BTreeSet<&String> = golden
        .keys()
        .chain(written.keys())
        .filter(|k| golden.get(*k) != written.get(*k))
        .collect();
    println!("FM-6 through `ew import csv`: the files that differ from golden/J-E-2416.ew: {differ:?}");

    // An `--out` that holds something: refused, kept.
    let o = import_csv(&home, tables, &out, &[]);
    assert_eq!(
        (o.status.code(), stderr(&o)),
        (
            Some(2),
            format!(
                "error: {}: not an empty directory; the new project needs a new or empty directory\n",
                out.display()
            )
        )
    );
    assert!(tree(&out) == written, "the project there is kept");
    // Without the starter content.
    let other = t.0.join("other.ew");
    let o = ew_isolated(
        &home,
        &[
            "import".as_ref(),
            "csv".as_ref(),
            tables.as_os_str(),
            "--out".as_ref(),
            other.as_os_str(),
        ],
    );
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            "error: `import csv` needs the starter content (`--content <dir>` or EW_CONTENT_DIR): the tables name its \
             symbols, function definitions and forms; nothing was written\n"
                .to_owned()
        )
    );
    assert!(!other.exists());
}

/// FM-6: a row with a foreign colour code is reported as the library reports it, EW-IMP-002 under `--strict` (the
/// default: nothing is written, exit 1) and EW-IMP-008 under `--lenient` (written, exit 0) (ID-375); an unreadable
/// table exits 2 with its line, and its column where the reader names one, before anything is written.
#[test]
fn import_csv_strict_lenient_and_unreadable_tables() {
    let t = Tmp::new();
    let home = t.0.join("home");
    let tables = tables_copy(&t, "tables");
    let conductors = tables.join("conductors.csv");
    let text = std::fs::read_to_string(&conductors).unwrap();
    let brown = text.replacen("-106W1:BN,-106W1,BN,BN,", "-106W1:BN,-106W1,BN,BR,", 1);
    assert_ne!(brown, text);
    std::fs::write(&conductors, brown).unwrap();

    let strict = t.0.join("strict.ew");
    let o = import_csv(&home, &tables, &strict, &[]);
    let lines = import_lines(&tables, ew_api::import::ImportMode::Strict, None);
    assert!(lines.contains("EW-IMP-002 error: conductors.csv:6: "), "{lines}");
    // The dropped conductor's two connections are dropped with it (EW-IMP-003).
    let errors = lines
        .lines()
        .filter(|l| l.split(' ').nth(1) == Some("error:"))
        .count();
    assert_eq!(errors, 3, "{lines}");
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(1),
            lines,
            "error: the import made 3 error findings under `--strict` (the default); nothing was written\n"
                .to_owned()
        )
    );
    assert!(!strict.exists(), "nothing is written");
    let o = import_csv(&home, &tables, &strict, &["--strict"]);
    assert_eq!(o.status.code(), Some(1), "`--strict` is the default");

    let lenient = t.0.join("lenient.ew");
    let o = import_csv(&home, &tables, &lenient, &["--lenient"]);
    let library = ew_api::import::StarterLibrary::from_dir(std::path::Path::new(CONTENT)).unwrap();
    let batch = ew_api::import::import_tables(
        &ew_api::import::read_tables(&tables).unwrap(),
        &library,
        ew_api::import::ImportMode::Lenient,
    )
    .unwrap();
    let n = ew_api::import::read_tables(&tables).unwrap().len();
    let last = format!(
        "imported {n} tables into {}: {} records, {} library documents",
        lenient.display(),
        batch.records.len(),
        batch.lib.len()
    );
    let lines = import_lines(&tables, ew_api::import::ImportMode::Lenient, Some(&last));
    assert!(
        lines.contains("EW-IMP-008 warning: conductors.csv:6: "),
        "{lines}"
    );
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (Some(0), lines, String::new())
    );
    assert!(lenient.join("manifest.json").is_file());

    // Unreadable tables: a ragged row (its line) and a cell over the reader's limit (its line and column).
    let devices = tables.join("devices.csv");
    let original = std::fs::read_to_string(&devices).unwrap();
    let rows = original.lines().count();
    let fields = original.lines().next().unwrap().split(',').count();
    std::fs::write(&devices, format!("{original}a,b\n")).unwrap();
    let out = t.0.join("unreadable.ew");
    let o = import_csv(&home, &tables, &out, &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: devices.csv: line {}: expected {fields} fields, found 2; nothing was imported\n",
                rows + 1
            )
        )
    );
    let long = "x".repeat(40_000);
    std::fs::write(
        &devices,
        format!("{original}-K9,{long}{}\n", ",".repeat(fields - 2)),
    )
    .unwrap();
    let o = import_csv(&home, &tables, &out, &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: devices.csv: line {}, column 2: cell longer than 32768 bytes; nothing was imported\n",
                rows + 1
            )
        )
    );
    assert!(!out.exists(), "nothing is written");
}
