// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of the `ew` binary.

use std::path::PathBuf;
use std::process::{Command, Output};

use crate::support::{CONTENT, GOLDEN_A, GOLDEN_PARTS, Tmp, bin_exe, copy_dir, stderr, stdout, tree};

/// The QElectroTech project of the import tests.
trait QetTmp {
    fn project(&self) -> PathBuf;
    fn project_with(&self, xml: &str) -> PathBuf;
}

impl QetTmp for Tmp {
    fn project(&self) -> PathBuf {
        self.project_with(PROJECT)
    }

    fn project_with(&self, xml: &str) -> PathBuf {
        let p = self.path("t.qet");
        std::fs::write(&p, xml).unwrap();
        p
    }
}

const LOST: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors><conductor terminal1="999" terminal2="998"/></conductors></diagram></project>"#;
const PROJECT: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors/></diagram></project>"#;

fn ew(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(bin_exe())
        .args(args)
        .env_remove("EW_CONTENT_DIR")
        .output()
        .unwrap()
}

#[test]
fn version() {
    let o = ew(&["--version".as_ref()]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(stdout(&o), format!("ew {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn import_summary_with_content() {
    let t = Tmp::new("ew-cli-test");
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

/// With `--out` the import is written as a new project with its run under `imports/`; without it
/// nothing is written; a target that holds something is refused.
#[test]
fn import_qet_out_writes_a_project_with_its_run() {
    let t = Tmp::new("ew-cli-test");
    let f = t.project();
    let before: Vec<_> = std::fs::read_dir(&t.0)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    let o = ew(&["import".as_ref(), "qet".as_ref(), f.as_os_str()]);
    assert_eq!(o.status.code(), Some(0));
    let after: Vec<_> = std::fs::read_dir(&t.0)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(before, after, "without --out nothing is written");
    let out = t.0.join("imported.ew");
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--content".as_ref(),
        CONTENT.as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(
        stdout(&o).ends_with(&format!("project written to {}\n", out.display())),
        "{}",
        stdout(&o)
    );
    let runs = std::fs::read_dir(out.join("imports")).unwrap().count();
    assert_eq!(runs, 1);
    assert!(out.join("manifest.json").is_file());
    let again = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(again.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&again.stderr)
            .lines()
            .last()
            .unwrap_or_default(),
        format!(
            "error: {}: not an empty directory; the new project needs a new or empty directory",
            out.display()
        )
    );
}

/// `--into` prints the rows and writes nothing; `--apply` commits and
/// saves; the same file again is unchanged; a file of another project is refused.
#[test]
fn import_qet_into_re_imports_into_the_earlier_import() {
    let t = Tmp::new("ew-cli-test");
    let f = t.project();
    let out = t.0.join("p.ew");
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let into = |file: &std::path::Path, apply: bool| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "import".as_ref(),
            "qet".as_ref(),
            file.as_os_str(),
            "--into".as_ref(),
            out.as_os_str(),
        ];
        if apply {
            args.push("--apply".as_ref());
        }
        ew(&args)
    };
    let same = into(&f, false);
    assert_eq!(
        same.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&same.stderr)
    );
    assert_eq!(
        stdout(&same),
        "Re-import of t.qet: 0 added, 0 changed in the source, 0 kept with your changes, 0 changed on both sides and kept, 0 removed in the source and kept, 1 unchanged.\n"
    );
    let renamed = t.project_with(&PROJECT.replace(r#"title="P""#, r#"title="Q""#));
    let manifest = std::fs::read(out.join("project.jsonl")).unwrap();
    let dry = into(&renamed, false);
    assert_eq!(
        dry.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert_eq!(
        stdout(&dry),
        "t.qet matches the earlier import of t.qet (100 % of its items). Re-import it into that import?\n\
         Re-import of t.qet: 0 added, 1 changed in the source, 0 kept with your changes, 0 changed on both sides and kept, 0 removed in the source and kept, 0 unchanged.\n\
         changed in the source: page 1\n\
         nothing was written: add --apply to commit the re-import\n"
    );
    assert_eq!(
        std::fs::read(out.join("project.jsonl")).unwrap(),
        manifest,
        "nothing was written"
    );
    let applied = into(&renamed, true);
    assert_eq!(
        applied.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert!(
        stdout(&applied).ends_with("re-import applied; saved\n"),
        "{}",
        stdout(&applied)
    );
    let again = into(&renamed, false);
    assert!(
        stdout(&again).ends_with(
            "Re-import of t.qet: 0 added, 0 changed in the source, 0 kept with your changes, 0 changed on both sides and kept, 0 removed in the source and kept, 1 unchanged.\n"
        ),
        "{}",
        stdout(&again)
    );
    let foreign = t.0.join("other.qet");
    std::fs::write(&foreign, LOST).unwrap();
    let refused = into(&foreign, false);
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&refused.stderr)
            .lines()
            .last()
            .unwrap_or_default(),
        "error: other.qet is not an earlier import of this project; import it into a new project instead."
    );
}

/// Two folios joined by a linked folio-report pair, each report wired to a lamp.
const REPORTS: &str = r#"<project version="0.100" title="T">
<diagram order="1" folio="1" title="A"><elements>
<element type="embed://import/lamp.elmt" uuid="{00000000-0000-4000-8000-000000000001}" x="100" y="100" orientation="0"><terminals/></element>
<element type="embed://import/next.elmt" uuid="{00000000-0000-4000-8000-000000000002}" x="300" y="100" orientation="0"><terminals/><links_uuids><link_uuid uuid="{00000000-0000-4000-8000-000000000003}"/></links_uuids></element>
</elements><conductors><conductor element1="{00000000-0000-4000-8000-000000000001}" terminal1="{00000000-0000-4000-9000-000000000001}" element2="{00000000-0000-4000-8000-000000000002}" terminal2="{00000000-0000-4000-9000-000000000002}"/></conductors></diagram>
<diagram order="2" folio="2" title="B"><elements>
<element type="embed://import/previous.elmt" uuid="{00000000-0000-4000-8000-000000000003}" x="100" y="100" orientation="0"><terminals/><links_uuids><link_uuid uuid="{00000000-0000-4000-8000-000000000002}"/></links_uuids></element>
<element type="embed://import/lamp.elmt" uuid="{00000000-0000-4000-8000-000000000004}" x="300" y="100" orientation="0"><terminals/></element>
</elements><conductors><conductor element1="{00000000-0000-4000-8000-000000000003}" terminal1="{00000000-0000-4000-9000-000000000003}" element2="{00000000-0000-4000-8000-000000000004}" terminal2="{00000000-0000-4000-9000-000000000001}"/></conductors></diagram>
<collection><category name="import">
<element name="lamp.elmt"><definition type="element" version="0.100.0" link_type="simple" width="20" height="20" hotspot_x="10" hotspot_y="10"><description><terminal x="10" y="0" orientation="e" uuid="{00000000-0000-4000-9000-000000000001}"/></description></definition></element>
<element name="next.elmt"><definition type="element" version="0.100.0" link_type="next_report" width="20" height="20" hotspot_x="10" hotspot_y="10"><description><terminal x="-10" y="0" orientation="w" uuid="{00000000-0000-4000-9000-000000000002}"/></description></definition></element>
<element name="previous.elmt"><definition type="element" version="0.100.0" link_type="previous_report" width="20" height="20" hotspot_x="10" hotspot_y="10"><description><terminal x="10" y="0" orientation="e" uuid="{00000000-0000-4000-9000-000000000003}"/></description></definition></element>
</category></collection></project>"#;

#[test]
fn import_without_content_keeps_folio_reports() {
    // Without `--content` the import takes the compiled-in documents, so the folio reports and their wires stay.
    let t = Tmp::new("ew-cli-test");
    let f = t.project_with(REPORTS);
    let o = ew(&["import".as_ref(), "qet".as_ref(), f.as_os_str()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains(" 2 connections,"), "{}", stdout(&o));
    assert!(stdout(&o).contains(" 0 lost connections"), "{}", stdout(&o));
}

#[test]
fn report_json_to_stdout() {
    let t = Tmp::new("ew-cli-test");
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
fn strict_fails_on_error_finding_lenient_does_not() {
    let t = Tmp::new("ew-cli-test");
    let clean = t.project();
    let o = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        clean.as_os_str(),
        "--strict".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
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

/// `--strict` that fails writes no project into `--out`, as `ew import csv --strict`; without it
/// the same file is written.
#[test]
fn strict_with_out_writes_nothing_when_it_fails() {
    let t = Tmp::new("ew-cli-test");
    let f = t.project_with(LOST);
    let out = t.0.join("strict.ew");
    let strict = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--content".as_ref(),
        CONTENT.as_ref(),
        "--strict".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(strict.status.code(), Some(1));
    let err = String::from_utf8_lossy(&strict.stderr);
    assert_eq!(err.lines().last(), Some("strict: nothing was written"), "{err}");
    assert!(!out.exists(), "nothing was written");
    let lenient = ew(&[
        "import".as_ref(),
        "qet".as_ref(),
        f.as_os_str(),
        "--content".as_ref(),
        CONTENT.as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(
        lenient.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&lenient.stderr)
    );
    assert!(out.join("manifest.json").is_file());
}

#[test]
fn report_csv_to_file() {
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
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
    let o = ew(&[
        "check".as_ref(),
        GOLDEN_A.as_ref(),
        "--scheme".as_ref(),
        "bogus".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(2), "an unknown --scheme: {}", stderr(&o));
}

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
    // The About dialog's sentence ends the report.
    assert!(
        stdout(&o).ends_with(
            "\n\nElektraWatt's checks do not replace engineering verification by a competent person.\n"
        ),
        "case check_markdown_disclaimer: {}",
        stdout(&o)
    );
}

/// A damaged project: the integrity findings of opening join the counts and the exit code (doc 15 §4.2).
#[test]
fn check_counts_integrity_errors_of_a_damaged_project() {
    let tmp = Tmp::new("ew-cli-test");
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

/// Without `--scheme`, `ew check` runs the scheme the project names in `settings.extra.checks`
/// (`release_scheme` in the default mode `release`, `active_scheme` in mode `online`); `--scheme` still wins and a
/// project that names none gets `pre-delivery`.
#[test]
fn check_uses_the_schemes_the_project_names() {
    let t = Tmp::new("ew-cli-test");
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

/// A project naming a scheme this build does not ship (a newer writer, a hand edit) is
/// checked with `pre-delivery` and a notice on stderr naming the scheme; the exit status is the check's own, the same
/// as for the project without the slot. A shipped name in the other slot, or `--scheme`, prints no notice; only the
/// slot for the run's mode is read.
#[test]
fn check_notices_a_project_scheme_this_build_does_not_ship() {
    let t = Tmp::new("ew-cli-test");
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
    // Only the slot for the run's mode is read: the release run ignores the unshipped online slot.
    let (_, scheme, stderr) = run(&dir, &[]);
    assert_eq!(scheme, "pre-delivery");
    assert!(!stderr.contains("not shipped"), "{stderr}");
}

/// P5-2: `ew check --lang nl-NL` prints the Dutch text of a rule with an nl-NL pattern (EW-POT-013 on
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
            && nl_text.contains("tijdelijk artikel \"End bracket\" (artikelnummer TBD)"),
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
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
    let missing = t.0.join("nope.ew");
    let o = ew(&["sql".as_ref(), missing.as_os_str(), "SELECT 1".as_ref()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("error:"));
}

/// `ew <args>` with `EW_DEBUG_PANIC` set to `site`: a debug build's test fault inside the panic guard of that site.
fn ew_faulted(site: &str, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(bin_exe())
        .args(args)
        .env_remove("EW_CONTENT_DIR")
        .env("EW_DEBUG_PANIC", site)
        .output()
        .unwrap()
}

/// The last line of a command's stderr (a caught panic's message from the default hook comes first).
fn last_line(o: &Output) -> String {
    stderr(o).lines().last().unwrap_or_default().to_owned()
}

/// SE-9 at the call site: `ew sql`'s statement panics inside `run_sql`'s guard; the run ends
/// with S1 and exit 2, prints no result, and the copy of Golden A's files are as they were.
#[test]
fn sql_a_panicking_statement_ends_with_s1_and_changes_nothing() {
    let t = Tmp::new("ew-cli-test");
    let project = golden_copy(&t);
    let before = tree(&project);
    let o = ew_faulted("sql", &["sql".as_ref(), project.as_os_str(), "SELECT 1".as_ref()]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert_eq!(
        last_line(&o),
        "error: the query stopped on an internal error; nothing was changed"
    );
    assert_eq!(stdout(&o), "");
    assert!(tree(&project) == before, "the copy's files changed");
}

/// The writing of `ew sql`'s result (CSV in both presets, JSON, the workbook) panics inside
/// `run_sql`'s guard; each run ends with S1 and exit 2, prints no result and writes no file.
#[test]
fn sql_a_panicking_result_writer_ends_with_s1_and_writes_nothing() {
    let t = Tmp::new("ew-cli-test");
    let project = golden_copy(&t);
    let out = t.0.join("result.xlsx");
    for format in [
        &["--format", "csv"][..],
        &["--format", "csv", "--preset", "rfc4180"],
        &["--format", "json"],
        &["--format", "xlsx", "--out", out.to_str().unwrap()],
    ] {
        let mut args: Vec<&std::ffi::OsStr> = vec!["sql".as_ref(), project.as_os_str(), "SELECT 1".as_ref()];
        args.extend(format.iter().map(std::ffi::OsStr::new));
        let o = ew_faulted("sql-write", &args);
        assert_eq!(o.status.code(), Some(2), "{format:?}: {}", stderr(&o));
        assert_eq!(
            last_line(&o),
            "error: the query stopped on an internal error; nothing was changed",
            "{format:?}"
        );
        assert_eq!(stdout(&o), "", "{format:?}");
        assert!(!out.exists(), "{format:?}");
    }
}

/// SE-9 at the call site: `ew import table --apply`'s read panics inside
/// `run_import_table`'s guard; the run ends with `error: ` and S2 with its first letter in lower case, exit 2, and the
/// copy of Golden A's files are as they were.
#[test]
fn import_table_a_panicking_read_ends_with_s2_and_changes_nothing() {
    let t = Tmp::new("ew-cli-test");
    let project = golden_copy(&t);
    let file = t.0.join("pages.csv");
    std::fs::write(&file, "id,Title\n").unwrap();
    let before = tree(&project);
    let o = ew_faulted(
        "table-read",
        &[
            "import".as_ref(),
            "table".as_ref(),
            file.as_os_str(),
            project.as_os_str(),
            "--table".as_ref(),
            "nav:pages".as_ref(),
            "--apply".as_ref(),
        ],
    );
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert_eq!(
        last_line(&o),
        "error: the table import stopped on an internal error while reading pages.csv; nothing was changed."
    );
    assert_eq!(stdout(&o), "");
    assert!(tree(&project) == before, "the copy's files changed");
}

/// SE-9 (NFR-REL-12): a recursive-CTE bomb in `ew sql` on a copy of Golden A stops at its deadline with
/// exit 2, and the copy's files are as they were.
#[test]
fn sql_a_recursive_cte_bomb_stops_at_its_deadline_and_changes_nothing() {
    let t = Tmp::new("ew-cli-test");
    let project = golden_copy(&t);
    let before = tree(&project);
    let o = ew(&[
        "sql".as_ref(),
        project.as_os_str(),
        "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT count(*) FROM c".as_ref(),
        "--timeout".as_ref(),
        "1".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert_eq!(
        stderr(&o),
        "error: the statement passed its deadline of 1000 ms\n"
    );
    assert_eq!(stdout(&o), "");
    assert!(tree(&project) == before, "the copy's files changed");
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
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
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
    saved_project(t, "tiny.ew", "7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37")
}

/// A fresh saved project `name` in `t` with the project ID `id`: one page, no parts.
fn saved_project(t: &Tmp, name: &str, id: &str) -> PathBuf {
    let project = serde_json::from_str(&format!(r#"{{"id":"{id}","derivation_profile":"2027.1"}}"#)).unwrap();
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

/// `ew verify-derived` over a small project: exit 0 and the summary line (every family, no difference).
#[test]
fn verify_derived_tiny_project_is_clean() {
    let t = Tmp::new("ew-cli-test");
    let dir = tiny_project(&t);
    let o = ew(&["verify-derived".as_ref(), dir.as_os_str()]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), stderr(&o));
    let out = stdout(&o);
    // Some families read, without pinning their number (CONTRIBUTING.md: no totals of growing sets).
    let families = out
        .strip_prefix("verify-derived: ")
        .and_then(|rest| rest.split_once(" families, "))
        .and_then(|(n, _)| n.parse::<usize>().ok());
    assert!(
        families.is_some_and(|n| n > 0) && out.ends_with(" keys, 0 differences, 0 oracle mismatches\n"),
        "{out}"
    );
    assert!(!dir.join(".~ew.lock").exists());
}

/// The parts list of a project without a `parts_summary` block (a new project from the built-in template, as the
/// blind drafter's self-test makes) is a refusal, exit 2, and the one-line message names the block, the export, where the
/// block comes from, and that there is nothing to export; no file is written.
#[test]
fn export_bom_without_the_block_is_exit_2_and_names_the_cause() {
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
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
#[ignore = "deep run: the oracle re-runs every memoised read"]
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
// `ew export pdf|svg` (G-M1-01) and `ew apply`

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
    } else {
        home.join("xdg_state_home").join("elektrawatt")
    }
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
    let t = Tmp::new("ew-cli-test");
    let project = golden_copy(&t);
    let before = tree(&project);
    let want = expected("pdf/sha256.txt");
    // The committed link count: 64 cross-reference items, 28 PLC overview rows and the report row links, a sum that
    // `ew-api`'s page-output test counts from the page views.
    let structure: serde_json::Value = serde_json::from_str(&expected("pdf/structure.json")).unwrap();
    let links = structure["links"].as_u64().expect("the committed link count");
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
        assert!(
            stdout(&o).contains(&format!(": 36 pages, {links} links")),
            "{}",
            stdout(&o)
        );
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
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
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

/// GP-UT (doc 20 section 2): `ew apply` of `variants/gp-ut.ewcmd` on a copy of Golden A removes -102F1
/// (its device, function, representation, layout placement and the potential +24V-F defined at its terminal 2) and
/// its four wires W20, W21, W24 and W25 (their six connection records, J3.1 and J4.1 being the junction segments, and
/// the junctions J3 and J4), and nothing else: every other record line is byte for byte Golden A's.
#[test]
fn apply_of_gp_ut_removes_102f1_and_its_four_wires_only() {
    let t = Tmp::new("ew-cli-test");
    let project = golden_copy(&t);
    let records = |dir: &std::path::Path| -> std::collections::BTreeSet<String> {
        tree(dir)
            .into_iter()
            .filter(|(rel, _)| rel.ends_with(".jsonl"))
            .flat_map(|(_, bytes)| {
                String::from_utf8(bytes)
                    .unwrap()
                    .lines()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let before = records(&project);
    let log = std::path::Path::new(VARIANTS).join("gp-ut.ewcmd");
    let o = ew_isolated(
        &t.0.join("home"),
        &["apply".as_ref(), project.as_os_str(), log.as_os_str()],
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(stdout(&o), "applied 1 of 1 lines; saved\n");
    let after = records(&project);
    assert!(after.is_subset(&before), "no record line is added or changed");
    let removed: Vec<String> = before
        .difference(&after)
        .map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            let name = v["source_ref"]["key"].as_str().unwrap_or("no source");
            format!("{} {name}", v["$k"].as_str().unwrap())
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(
        removed,
        [
            "connection connections|J3.1",
            "connection connections|J4.1",
            "connection connections|W20",
            "connection connections|W21",
            "connection connections|W24",
            "connection connections|W25",
            "device devices|-102F1",
            "function functions|-102F1",
            "junction junctions|J3",
            "junction junctions|J4",
            "placement no source",
            "potential_def potentials|+24V-F",
            "representation representations|-102F1",
        ]
    );
    // The placement is -102F1's.
    let placement = before
        .difference(&after)
        .find(|l| l.contains("\"$k\":\"placement\""))
        .unwrap();
    assert!(
        placement.contains("\"device\":\"ff321999-9715-51d0-8eb6-9b93acae6158\""),
        "{placement}"
    );
}

/// `SOURCE_DATE_EPOCH` (doc 17 section 3.1) sets the creation date and with it the document ID: the PDF differs
/// from the model-dated one, carries the date, and is the same bytes on one and four threads; an invalid value
/// exits 2 and writes nothing.
#[test]
fn source_date_epoch_dates_the_pdf() {
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
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

/// `ew apply` refuses a project whose crashed session left a journal (exit 2, naming `ew recover` and no follow-up
/// number), and applies nothing.
#[test]
fn apply_refuses_a_project_with_a_crashed_session_journal() {
    let t = Tmp::new("ew-cli-test");
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
            "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with \
             `ew recover`; nothing was written\n",
            project.display()
        )
    );
    assert!(tree(&project) == before, "nothing is applied");
    // FM-5: the other session's journal is still there, unchanged. The second half, a clean close deletes only
    // its own segment, is `Journal::close` (`crates/ew-store/src/journal.rs`), shown by
    // `recovery_replays_the_newest_journal_and_keeps_the_other_unchanged` (`crates/ew-store/tests/it/recovery.rs`)
    // and `the_app_path_recovers_one_journal_at_a_time` (`crates/ew-api/tests/it/recovery.rs`).
    assert!(journal_files(&home) == journals, "the journal is still there");
}

/// Each kind of subcommand that prints `journal_refusal`'s text (`ew apply`, `ew import parts`, a human-only
/// subcommand) has a whole-text test. `ew import parts` refuses a project whose crashed session
/// left a journal with `ew apply`'s text (exit 2, before the table is read) and writes nothing; the journal stays. The
/// human-only kind is a unit test with the injected terminal (`cli/project_files.rs`).
#[test]
fn import_parts_refuses_a_project_with_a_crashed_session_journal() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let project = parts_target(&t, "parts-journal.ew");
    crashed_session(&home, &project, &[new_page(7777)]);
    let before = tree(&project);
    let journals = journal_files(&home);
    assert_eq!(journals.len(), 1);
    let o = ew_isolated(
        &home,
        &[
            "import".as_ref(),
            "parts".as_ref(),
            GOLDEN_PARTS.as_ref(),
            "--project".as_ref(),
            project.as_os_str(),
        ],
    );
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
    assert!(tree(&project) == before, "nothing is imported");
    assert!(journal_files(&home) == journals, "the journal stays");
}

/// `ew project checks set` submits the `HumanOnly` command
/// `project.checks.set` only after a confirmation typed on a controlling terminal (doc 17 section 2.3, FR-EXT-030).
/// Without a terminal (here: stdout captured, stdin empty or piped) it exits 2 before the project is opened: nothing
/// changes, no lock, no journal. A scheme that does not ship exits 1 without asking; a usage error (no scheme, a
/// `--confirm` or `--yes` flag) exits 2. The typed path is a unit test with an injected reader and writer (`cli.rs`).
#[test]
fn project_checks_set_needs_an_interactive_terminal() {
    let t = Tmp::new("ew-cli-test");
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
    assert_eq!(
        stderr(&o),
        "error: HumanOnly: needs an interactive terminal (`project checks set` is confirmed by typing the scheme \
         name); nothing was changed\n"
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

/// `ew project assets purge --preview` lists the asset files nothing names and changes nothing; the purge
/// itself is human-only and exits 2 off a terminal with nothing deleted; an unknown option is
/// a usage error.
#[test]
fn project_assets_purge_previews_and_is_refused_off_a_terminal() {
    const X: &[u8] = br#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#;
    const Y: &[u8] = br#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#;
    let t = Tmp::new("ew-cli-test");
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

/// `ew revision approve|release|withdraw` run human-only commands, confirmed by the typed
/// revision label; without a terminal (stdout captured here) each exits 2 before opening the project, and nothing is
/// written. The typed confirmation is a unit test with an injected reader and writer.
#[test]
fn revision_commands_are_refused_off_a_terminal() {
    let t = Tmp::new("ew-cli-test");
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
/// - `project.drawing.set` (`marker_mode`), `box.structure`, `item.place`,
///   `marker.place` and `rep.move`;
/// - the property grid pilot's `--record` log (`pilot_log.rs`, which the headless
///   test of `elektrawatt` proves is the record of its script and replays in-process to the records it saved);
/// - the find-and-replace script's `--record` log (`find_log.rs`: three
///   `fields.set_many` lines, which the headless test of `elektrawatt` proves is the record of its script).
///
/// Every line applies and the saved records are those the same log gives in-process (the spec's replay path), the
/// minted page items and structure boxes compared by content.
#[test]
fn the_recorded_logs_replay_through_ew_apply() {
    let t = Tmp::new("ew-cli-test");
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
        for l in crate::pilot_log::PILOT_LOG
            .lines()
            .chain(crate::find_log::FIND_LOG.lines())
        {
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
    assert_eq!(crate::find_log::FIND_LOG.lines().count(), 3);
    let n = 6 + crate::pilot_log::PILOT_LOG.lines().count() + crate::find_log::FIND_LOG.lines().count();
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

/// A fresh saved project without parts (the import target).
fn parts_target(t: &Tmp, name: &str) -> PathBuf {
    saved_project(t, name, "3b9e51c4-8a27-7d10-b5f3-6c0a2e9d4f18")
}

/// The part copies of a `lib/parts` folder by `item_id`, without the members that legitimately differ between
/// projects (ids, hash, provenance, the licence that only the golden bootstrap states, the library of
/// the manufacturer and of an enclosure's plate part).
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
            m.remove("licence");
        }
        if let Some(m) = o.get_mut("manufacturer").and_then(|m| m.as_object_mut()) {
            m.remove("library");
        }
        // The enclosure section's plate part is project-local too.
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

/// `ew import parts` creates the golden project's 29 part copies, and the recorded command
/// replays through `ew apply` to the same bytes.
#[test]
fn import_parts_creates_the_golden_part_copies_and_replays_through_apply() {
    let t = Tmp::new("ew-cli-test");
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

/// A second import skips every key and changes nothing.
#[test]
fn a_second_import_parts_skips_every_key() {
    let t = Tmp::new("ew-cli-test");
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

/// A refused table names its line and column and writes nothing.
#[test]
fn import_parts_refusals_name_line_and_column() {
    let t = Tmp::new("ew-cli-test");
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

/// `ew import parts` with a `parts.csv` and sibling tables that a spreadsheet
/// saved with a byte-order mark creates the same 29 part copies as the golden tables without it.
#[test]
fn import_parts_with_a_bom_prefixed_table_creates_the_same_copies() {
    let t = Tmp::new("ew-cli-test");
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

/// `ew lib`: create a user library, list the tiers, search the starter, rebuild the index; the
/// index lives in the cache directory.
#[test]
fn lib_create_list_search_and_rebuild() {
    let tmp = Tmp::new("ew-cli-test");
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
    // An object naming its schema, the libraries under `libraries`.
    assert_eq!(list["$schema"], "ew.lib-list-result/1");
    // The content's other system libraries (the sample parts) are listed too; that set grows.
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
        .filter(|(k, t, _)| t != "system" || k.starts_with("ew-starter"))
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
    // The sample project's catalogue parts are a system library of the content.
    let o = run(&["lib", "search", "A9P52610"]);
    assert_eq!(
        (o.status.code(), stdout(&o).as_str()),
        (
            Some(0),
            "part\tA9P52610\tSchneider Electric\tMiniature circuit breaker 1P+N\treleased\t\
             ew-sample-parts/d882a316-b6fc-589f-85c1-a7b2ad9dcc77@0.1.0\n"
        )
    );
    let o = run(&["lib", "search", "3209510"]);
    assert_eq!((o.status.code(), stdout(&o).as_str()), (Some(0), ""));
    assert_eq!(stderr(&o).trim(), "nothing found");
    let o = run(&["lib", "index", "--rebuild"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    // The count includes the compiled-in starter parts, so it is not pinned.
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

/// `ew lib import parts` writes a mapped list into a user library, refuses changed content under
/// the published version unless `--bump`, and `ew import parts --map` imports the same list into a project.
#[test]
fn lib_import_parts_and_import_parts_with_a_map() {
    let tmp = Tmp::new("ew-cli-test");
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
// AC-21 through the CLI (CL-6)

/// The built packs of AC-21's subset, as the `ew-api` test `ac21_four_pack_subset_equals_findings_mvp_csv` names them.
const AC21_PACKS: [&str; 8] = [
    "EW-CORE",
    "EW-IEC81346",
    "EW-TERM",
    "EW-POT",
    "EW-PLC",
    "EW-BUS",
    "EW-LAYOUT",
    "EW-FL",
];

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
    let t = Tmp::new("ew-cli-test");
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
    // Doc 20 §18.2: the rules of release 1.0 run too. `findings_1_0.csv` adds EF-20, EF-21 and EF-22, and EF-22
    // (EW-PLC-013) supersedes EF-10 (EW-POT-003) on its pins.
    want.remove(&("EW-POT-003".to_owned(), "warning".to_owned()));
    let v1 = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/ew-api/tests/expected/findings_1_0.csv"),
    )
    .unwrap();
    for line in v1.lines().skip(1) {
        let f = csv_fields(line);
        let mut subjects: Vec<String> = f[4].split("; ").map(str::to_owned).collect();
        subjects.sort();
        want.insert((f[1].clone(), f[2].clone()), subjects);
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
// `ew fmt`, `ew recover` and `ew import csv`

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
/// exits 1 naming that file and writes nothing; `ew fmt` rewrites that file only (it saves through the plan that starts from
/// `Loaded.rewrite`, never `Changes::All`); then
/// `ew fmt --check` exits 0 and the files equal the original bytes. The manifest is a new generation by necessity; its
/// file hashes equal the original ones.
#[test]
fn fmt_check_names_a_swapped_file_and_fmt_restores_its_bytes() {
    let t = Tmp::new("ew-cli-test");
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
    let t = Tmp::new("ew-cli-test");
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
            // "error: <dir>: the project opens read-only: <reason>; nothing
            // was written", with "it has integrity findings; `ew check` lists them".
            format!(
                "error: {}: the project opens read-only: it has integrity findings; `ew check` lists them; nothing \
                 was written\n",
                broken.display()
            )
        )
    );
    assert!(tree(&broken) == before, "nothing is written");
}

/// FT-1383: a writer refused on a project whose damaged library copy a device uses names the
/// device, the copy in words and the way out, and writes nothing.
#[test]
fn a_writer_names_the_damaged_copy_a_device_uses() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let mut shards: Vec<_> = std::fs::read_dir(project.join("devices"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    shards.sort();
    let copy = shards
        .iter()
        .find_map(|p| {
            let text = std::fs::read_to_string(p).unwrap();
            let at = text.find("\"copy\":\"")? + "\"copy\":\"".len();
            Some(text[at..at + 36].to_owned())
        })
        .expect("a device names a part copy");
    let file = format!("lib/parts/{copy}.json");
    std::fs::write(project.join(&file), "{\"$k\": \"part\",\n").unwrap();
    let before = tree(&project);
    let o = ew_in(&home, &["fmt".as_ref(), project.as_os_str()], &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the project opens read-only: the device -201U8-Q1 uses the library copy of its part, \
                 which is damaged; update the copy from its library first; nothing was written\n",
                project.display()
            )
        )
    );
    assert!(tree(&project) == before, "nothing is written");
}

/// `ew apply`'s line that assigns the compiled-in starter part EW-GEN-MCB-2 to Golden A's F55: its part
/// copy is `lib/parts/4b259878-8a92-56e0-ac8e-8f23e7c66a6b.json`.
const STARTER_MCB_ASSIGN: &str = r#"{"cmd":"part.assign","params":{"copies":[{"$k":"part","category":["ProtectionOvercurrent","MCB","1P+N"],"cp_pattern":[{"designation":"1","idx":0},{"designation":"2","idx":1},{"designation":"N","idx":2},{"designation":"N","idx":3}],"description":{"en":"Miniature circuit breaker, 1 pole and neutral"},"function_template":[{"cps":[0,1,2,3],"fndef":{"item":"473bc9ca-9b42-5683-ada8-f5f0a4081772","key":"ew.fn.mcb-1pn","library":"7202fd8b-1024-4bf1-9e99-670772eaa589","version_req":"^0.1"},"main":true}],"id":"4b259878-8a92-56e0-ac8e-8f23e7c66a6b","item_id":"eb8a0c03-fd56-5637-b8f9-23434fd03a5d","library_id":"75d07cef-7a38-5814-a2ca-4610c08d9a8b","manufacturer":{"item":"f4260253-4a2b-5c11-a3d6-b01e2aa5efe6","library":"75d07cef-7a38-5814-a2ca-4610c08d9a8b","version_req":"^0.1"},"mech":{"depth":70000000,"height":85000000,"mount_kind":"ts35x7_5","width":36000000},"meta":{"key":"EW-GEN-MCB-2","licence":"CC0-1.0","sources":["ew.original"]},"part_number":"EW-GEN-MCB-2","sha256":"sha256:3eeb8d7541d8f9218a2fe6c5c069ec88a310aab147248f8f39364403bbe249e7","tech_data":[{"key":"characteristic","value":"C"},{"key":"rated_current","value":{"unit":"A","value":"10"}}],"version":"0.1.1"},{"$k":"manufacturer","id":"7ae048b6-63ea-5c01-83b2-e1a83b5b4414","item_id":"f4260253-4a2b-5c11-a3d6-b01e2aa5efe6","key":"Generic","library_id":"75d07cef-7a38-5814-a2ca-4610c08d9a8b","meta":{"licence":"CC0-1.0","sources":["ew.original"]},"name":{"en":"Generic"},"sha256":"sha256:f3df52cb1e83bcc3bda7957948246bcb5a5a958739eca73dce4191db5ad15d9c","version":"0.1.0"}],"devices":["594f6043-5d61-51b2-8f57-db20fe74a517"],"from":"f54fb48e-fc5d-5bd2-9d6a-67edb901dc3f","mode":"replace","part":"4b259878-8a92-56e0-ac8e-8f23e7c66a6b"}}"#;

/// A writer refused on a project whose damaged library copy is of a part an installed library holds
/// (the compiled-in starter parts' EW-GEN-MCB-2, assigned to F55 by `ew apply`) names the part and its library, whole.
#[test]
fn a_writer_names_the_damaged_copy_of_an_installed_part_whole() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let (project, file) = starter_mcb_project(&t, &home);
    std::fs::write(&file, "{\"$k\": \"part\",\n").unwrap();
    let before = tree(&project);
    let o = ew_in(&home, &["fmt".as_ref(), project.as_os_str()], &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the project opens read-only: the device F55 uses the library copy of part EW-GEN-MCB-2 \
                 from ElektraWatt starter parts, which is damaged; update the copy from its library first; nothing \
                 was written\n",
                project.display()
            )
        )
    );
    assert!(tree(&project) == before, "nothing is written");
}

/// A writer refused on a project whose damaged library copy no device names (the form page 1 names)
/// names the copy and the record that names it, and writes nothing.
#[test]
fn a_writer_names_the_damaged_copy_a_page_names() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let project = golden_copy(&t);
    std::fs::write(
        project.join("lib/forms/81ee0d6d-6c0e-55e4-b7fd-1896f594051f.json"),
        "{\"$k\": \"form\",\n",
    )
    .unwrap();
    let before = tree(&project);
    let o = ew_in(&home, &["fmt".as_ref(), project.as_os_str()], &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the project opens read-only: the library copy of a form named by page 1 is damaged; \
                 update the copy from its library first; nothing was written\n",
                project.display()
            )
        )
    );
    assert!(tree(&project) == before, "nothing is written");
}

/// A copy of Golden A in `t` whose F55 uses the compiled-in starter part EW-GEN-MCB-2, assigned by `ew apply`: the folder
/// and the part copy's file, as the save wrote it.
fn starter_mcb_project(t: &Tmp, home: &std::path::Path) -> (PathBuf, PathBuf) {
    let project = golden_copy(t);
    let log = t.0.join("assign.ewcmd");
    std::fs::write(
        &log,
        format!("{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"test\"}}\n{STARTER_MCB_ASSIGN}\n"),
    )
    .unwrap();
    let applied = ew_isolated(home, &["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    let file = project.join("lib/parts/4b259878-8a92-56e0-ac8e-8f23e7c66a6b.json");
    (project, file)
}

/// FT-1499: `ew update-copies` writes a damaged copy of a starter part from the compiled-in library, byte for
/// byte as the assignment's save wrote it, says so with the version written, and the project then opens for
/// editing (`ew fmt` writes nothing).
#[test]
fn update_copies_writes_a_damaged_copy_from_its_library() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let (project, file) = starter_mcb_project(&t, &home);
    let original = std::fs::read(&file).unwrap();
    std::fs::write(&file, "{\"$k\": \"part\",\n").unwrap();
    let o = ew_in(&home, &["update-copies".as_ref(), project.as_os_str()], &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(0),
            "updated the library copy of part EW-GEN-MCB-2 at version 0.1.1 from its library\nthe project opens for \
             editing\n"
                .to_owned(),
            String::new()
        )
    );
    assert!(
        std::fs::read(&file).unwrap() == original,
        "the library's copy is written"
    );
    let fmt = ew_in(
        &home,
        &["fmt".as_ref(), project.as_os_str(), "--check".as_ref()],
        &[],
    );
    assert_eq!(fmt.status.code(), Some(0), "{}", stderr(&fmt));
}

/// FT-1499: `ew update-copies` on a project whose damaged copy no library holds (Golden A's own part copy of
/// -201U8-Q1) is refused in words, exit 1, and writes nothing.
#[test]
fn update_copies_refuses_a_copy_no_library_holds() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let mut shards: Vec<_> = std::fs::read_dir(project.join("devices"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    shards.sort();
    let copy = shards
        .iter()
        .find_map(|p| {
            let text = std::fs::read_to_string(p).unwrap();
            let at = text.find("\"copy\":\"")? + "\"copy\":\"".len();
            Some(text[at..at + 36].to_owned())
        })
        .expect("a device names a part copy");
    std::fs::write(
        project.join(format!("lib/parts/{copy}.json")),
        "{\"$k\": \"part\",\n",
    )
    .unwrap();
    let before = tree(&project);
    let o = ew_in(&home, &["update-copies".as_ref(), project.as_os_str()], &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(1),
            String::new(),
            format!(
                "error: {}: no library on this computer holds a readable version of the library copy of a part; \
                 nothing was written\n",
                project.display()
            )
        )
    );
    assert!(tree(&project) == before, "nothing is written");
}

/// The header of a PNG file of `w` × `h` pixels: what the asset gate reads of it.
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend_from_slice(&13u32.to_be_bytes());
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b
}

/// The SHA-256 of `bytes` in hex.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `bytes` in standard base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A project that holds an image over the pixel limit, which an earlier build took in (a small
/// image placed on folio 101 by `ew apply`, then turned into a big one on disk with the manifest's hashes), is named in
/// the window's words by `ew check --integrity`, as a `notice:` line outside the counts and in the JSON's `notices`, and
/// by a writer's open.
#[test]
fn an_image_kept_but_not_drawn_is_a_notice_of_check_integrity_and_of_an_open() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let (small, big) = (png(2, 1), png(65536, 32768));
    let place = format!(
        r#"{{"cmd":"item.place","params":{{"page":"71260e59-ea02-5e16-8678-401d4044d9d7","kind":{{"image":{{"data":"{}","rect":[[0,0],[2000000,1000000]]}}}}}}}}"#,
        base64(&small)
    );
    let log = t.0.join("place.ewcmd");
    std::fs::write(
        &log,
        format!("{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"test\"}}\n{place}\n"),
    )
    .unwrap();
    let applied = ew_isolated(&home, &["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    // An earlier build took the big one in: the asset file, the record naming it and the manifest's hashes.
    let (from, to) = (sha256_hex(&small), sha256_hex(&big));
    let mut manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
    for (rel, bytes) in tree(&project) {
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if rel != "manifest.json" && text.contains(&from) {
            let changed = text.replace(&from, &to);
            manifest = manifest.replace(&sha256_hex(text.as_bytes()), &sha256_hex(changed.as_bytes()));
            std::fs::write(project.join(&rel), changed).unwrap();
        }
    }
    std::fs::remove_file(project.join(format!("assets/{from}.png"))).unwrap();
    std::fs::write(project.join(format!("assets/{to}.png")), &big).unwrap();
    std::fs::write(project.join("manifest.json"), manifest.replace(&from, &to)).unwrap();
    let notice = "The image on folio 101 is 65536 × 32768 pixels, more than a drawing can show, so it is kept but not \
                  drawn; scale it down and add it again.";
    let o = ew(&["check".as_ref(), project.as_os_str(), "--integrity".as_ref()]);
    assert_eq!(
        (o.status.code(), stderr(&o)),
        (
            Some(0),
            format!("notice: {notice}\nintegrity: 0 errors, 0 warnings, 0 infos, 0 waived\n")
        )
    );
    let o = ew(&[
        "check".as_ref(),
        project.as_os_str(),
        "--integrity".as_ref(),
        "--format".as_ref(),
        "json".as_ref(),
    ]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["notices"], serde_json::json!([notice]));
    assert_eq!(v["counts"]["errors"], 0);
    let o = ew_isolated(&home, &["fmt".as_ref(), project.as_os_str()]);
    assert_eq!(
        (o.status.code(), stderr(&o)),
        (
            Some(0),
            format!("notice: {notice}\nfmt: every file is in canonical form; nothing was written\n")
        )
    );
}

/// FM-3: `ew fmt` on a project with a crashed session's journal exits 2, pointing at `ew recover`, and writes nothing,
/// though a file is out of canonical form; the journal stays.
#[test]
fn fmt_refuses_a_project_with_a_crashed_session_journal() {
    let t = Tmp::new("ew-cli-test");
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

/// The first page file of `project` (`pages/<id>.jsonl`).
fn first_page_file(project: &std::path::Path) -> String {
    let first = tree(&project.join("pages"))
        .into_keys()
        .next()
        .expect("a page file");
    format!("pages/{first}")
}

/// `bytes` with `lines` inserted after their first line.
fn after_the_first_line(bytes: &[u8], lines: &[&[u8]]) -> Vec<u8> {
    let first = bytes.iter().position(|b| *b == b'\n').expect("a first line") + 1;
    let mut out = bytes[..first].to_vec();
    for line in lines {
        out.extend_from_slice(line);
        out.push(b'\n');
    }
    out.extend_from_slice(&bytes[first..]);
    out
}

/// Writes `bytes` to the project file `rel` with their hash in the manifest, as a save of those bytes writes it: lines
/// that came with the generation, not an edit outside ElektraWatt since the last save.
fn put_as_saved(project: &std::path::Path, rel: &str, bytes: &[u8]) {
    let hash = |b: &[u8]| format!("sha256:{}", sha256_hex(b));
    let (old, new) = (hash(&std::fs::read(project.join(rel)).unwrap()), hash(bytes));
    let manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
    assert_eq!(manifest.matches(&old).count(), 1, "the manifest lists {rel} once");
    std::fs::write(project.join("manifest.json"), manifest.replace(&old, &new)).unwrap();
    std::fs::write(project.join(rel), bytes).unwrap();
}

/// The backup `ew fmt --repair` writes for `project` at `generation`: beside the project folder, named
/// `<project folder>.before-repair-g<generation>.ewz`, as the file system names it.
fn backup_of(project: &std::path::Path, generation: u64) -> PathBuf {
    let canonical = std::fs::canonicalize(project).unwrap();
    let text = canonical.to_string_lossy().into_owned();
    let plain = PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text));
    let name = format!(
        "{}.before-repair-g{generation}.ewz",
        plain.file_name().unwrap().to_string_lossy()
    );
    plain.parent().unwrap().join(name)
}

/// The names of the files beside `project` that start with its folder's name and `.before-repair`.
fn beside_the_project(project: &std::path::Path) -> Vec<String> {
    let prefix = format!("{}.before-repair", project.file_name().unwrap().to_string_lossy());
    let mut names: Vec<String> = std::fs::read_dir(project.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefix))
        .collect();
    names.sort();
    names
}

/// EZ-5: `ew fmt --repair` on a page file of Golden A's copy with one broken line
/// prints the line (its text cut at 80 characters) and the backup, writes the backup `.ewz` beside the project folder
/// (generation 1 as it was, the line included) and a project without the line, which opens editable and canonical; a
/// second run has nothing to repair and writes nothing; run on the project as it was, it reuses the backup.
#[test]
fn fmt_repair_discards_a_broken_line_after_a_backup() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let project = golden_copy(&t);
    let rel = first_page_file(&project);
    let original = std::fs::read(project.join(&rel)).unwrap();
    let broken = format!("{{\"$k\":\"page\",\"title\":\"{}\"", "x".repeat(100));
    let bytes = after_the_first_line(&original, &[broken.as_bytes()]);
    put_as_saved(&project, &rel, &bytes);
    let saved = t.0.join("as-it-was.ew");
    copy_dir(&project, &saved);
    let before = tree(&project);
    let fmt = |extra: &[&str]| ew_in(&home, &["fmt".as_ref(), project.as_os_str()], extra);
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    let backup = backup_of(&project, 1);
    let shown: String = broken.chars().take(80).collect();
    let discarded = format!("discarded {rel}:2: {shown}\u{2026}\n");
    assert_eq!(
        run(&fmt(&["--repair"])),
        (
            Some(0),
            format!("{discarded}backup {} (generation 1)\n", backup.display()),
            "Repaired: discarded 1 line; saved generation 2\n".to_owned()
        )
    );
    let after = tree(&project);
    assert_eq!(after[&rel], original, "the page file without the line");
    let changed: Vec<&String> = after.keys().filter(|k| after.get(*k) != before.get(*k)).collect();
    assert_eq!(
        changed,
        ["manifest.json", &rel],
        "only the manifest and the page file"
    );
    let packed = ew_api::ewz::read(&std::fs::read(&backup).unwrap()).unwrap();
    assert_eq!(packed.generation(), 1);
    assert_eq!(packed.entries[&rel], bytes, "the backup holds the broken line");
    assert_eq!(
        beside_the_project(&project),
        ["J-E-2416.ew.before-repair-g1.ewz"],
        "no temporary file is left"
    );
    assert_eq!(
        run(&fmt(&["--check"])),
        (
            Some(0),
            String::new(),
            "fmt: every file is in canonical form\n".to_owned()
        )
    );

    let now = tree(&project);
    assert_eq!(
        run(&fmt(&["--repair"])),
        (Some(0), String::new(), "Nothing to repair\n".to_owned())
    );
    assert!(tree(&project) == now, "nothing is written");

    std::fs::remove_dir_all(&project).unwrap();
    copy_dir(&saved, &project);
    assert_eq!(
        run(&fmt(&["--repair"])),
        (
            Some(0),
            format!(
                "{discarded}backup {} (generation 1, already there and reused)\n",
                backup.display()
            ),
            "Repaired: discarded 1 line; saved generation 2\n".to_owned()
        )
    );
    assert_eq!(tree(&project)[&rel], original);
    assert!(journal_files(&home).is_empty(), "the repair's journal is closed");
}

/// EZ-5 on a hand-edited copy of Golden A: the broken line written by hand,
/// the manifest untouched (an edit outside ElektraWatt since the last save), and the same with the page file's hash
/// pending in a merged manifest. `ew export ewz` refuses the project, which is no verified generation; `ew fmt
/// --repair` writes the backup of the files as found, saying how many differ from the generation as saved, then the
/// project without the line, which opens editable and canonical; `ew extract` makes a project of the backup, its
/// manifest verified and hashed to the files, every listed file as it was before the repair.
#[test]
fn fmt_repair_backs_up_a_hand_edited_project_as_found() {
    const NOT_JSON: &[u8] = br#"{"$k":"page","id":"#;
    let hash = |b: &[u8]| format!("sha256:{}", sha256_hex(b));
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    for merged in [false, true] {
        let t = Tmp::new("ew-cli-test");
        let home = t.0.join("home");
        let project = golden_copy(&t);
        let rel = first_page_file(&project);
        let original = std::fs::read(project.join(&rel)).unwrap();
        let bytes = after_the_first_line(&original, &[NOT_JSON]);
        std::fs::write(project.join(&rel), &bytes).unwrap();
        let listed = if merged {
            let manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
            assert_eq!(manifest.matches(&hash(&original)).count(), 1);
            let pending = manifest.replace(&hash(&original), "sha256:pending").replacen(
                "\n  }\n}\n",
                "\n  },\n  \"merged\": true\n}\n",
                1,
            );
            assert!(pending.ends_with("\"merged\": true\n}\n"));
            std::fs::write(project.join("manifest.json"), pending).unwrap();
            "sha256:pending".to_owned()
        } else {
            hash(&original)
        };
        let before = tree(&project);
        let mut found = before.clone();
        let manifest = String::from_utf8(before["manifest.json"].clone()).unwrap();
        found.insert(
            "manifest.json".to_owned(),
            manifest.replace(&listed, &hash(&bytes)).into_bytes(),
        );

        let out = t.0.join("export.ewz");
        let export = ew_in(
            &home,
            &[
                "export".as_ref(),
                "ewz".as_ref(),
                project.as_os_str(),
                "--out".as_ref(),
                out.as_os_str(),
            ],
            &[],
        );
        let why = if merged {
            format!("the manifest's hash of {rel} is pending (a merged project); save it first")
        } else {
            format!("{rel} changed on disk since it was saved (an external change)")
        };
        assert_eq!(
            run(&export),
            (
                Some(2),
                String::new(),
                format!("error: {}: {why}\n", project.display())
            )
        );
        assert!(!out.exists(), "the export writes nothing");

        let backup = backup_of(&project, 1);
        let repair = ew_in(
            &home,
            &["fmt".as_ref(), project.as_os_str(), "--repair".as_ref()],
            &[],
        );
        assert_eq!(
            run(&repair),
            (
                Some(0),
                format!(
                    "discarded {rel}:2: {}\nbackup {} (generation 1, as found: 1 file edited outside EW)\n",
                    String::from_utf8_lossy(NOT_JSON),
                    backup.display()
                ),
                "Repaired: discarded 1 line; saved generation 2\n".to_owned()
            ),
            "merged: {merged}"
        );
        let after = tree(&project);
        assert_eq!(after[&rel], original, "the page file without the line");
        let changed: Vec<&String> = after.keys().filter(|k| after.get(*k) != before.get(*k)).collect();
        assert_eq!(
            changed,
            ["manifest.json", &rel],
            "only the manifest and the page file"
        );
        assert_eq!(
            run(&ew_in(
                &home,
                &["fmt".as_ref(), project.as_os_str(), "--check".as_ref()],
                &[]
            )),
            (
                Some(0),
                String::new(),
                "fmt: every file is in canonical form\n".to_owned()
            )
        );

        let restored = t.0.join("restored.ew");
        let extract = ew_in(
            &home,
            &[
                "extract".as_ref(),
                backup.as_os_str(),
                "--into".as_ref(),
                restored.as_os_str(),
            ],
            &[],
        );
        assert_eq!(
            run(&extract),
            (
                Some(0),
                format!(
                    "extracted into {}: generation 1, {} files\n",
                    restored.display(),
                    found.len()
                ),
                String::new()
            )
        );
        assert!(
            tree(&restored) == found,
            "every listed file as it was before the repair, the manifest hashed to them"
        );
    }
}

/// The refusals come before anything is written, each named, exit 2, in this order: an `.ewz`; a
/// crashed session's journal (here before a conflict marker the repair cannot discard); a finding other than a broken
/// line that forces read-only (EW-CORE-006), also one beyond the findings cap; a project whose backup cannot be packed
/// (no manifest; a page file edited outside ElektraWatt since the last save is packed as found); a backup name
/// holding another file. And `--repair` with `--check` is refused by the parser.
#[test]
fn fmt_repair_refusals_come_first_and_write_nothing() {
    const NOT_JSON: &[u8] = br#"{"$k":"page","id":"#;
    const MARKER: &[u8] = b"<<<<<<< HEAD";
    let run = |o: &Output| (o.status.code(), stdout(o), stderr(o));
    let repair = |home: &std::path::Path, p: &std::path::Path, extra: &[&str]| {
        ew_in(home, &["fmt".as_ref(), p.as_os_str(), "--repair".as_ref()], extra)
    };
    // The project of a fresh temporary directory, its first page file holding `lines` after its first line, written as
    // a save writes them (`saved`) or by hand.
    let project = |lines: &[&[u8]], saved: bool| {
        let t = Tmp::new("ew-cli-test");
        let p = tiny_project(&t);
        let rel = first_page_file(&p);
        let bytes = after_the_first_line(&std::fs::read(p.join(&rel)).unwrap(), lines);
        if saved {
            put_as_saved(&p, &rel, &bytes);
        } else {
            std::fs::write(p.join(&rel), &bytes).unwrap();
        }
        (t, p, rel)
    };

    // An `.ewz`.
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let ewz = t.0.join("x.ewz");
    std::fs::write(&ewz, b"not read").unwrap();
    assert_eq!(
        run(&repair(&home, &ewz, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: an .ewz opens read-only; extract it first\n",
                ewz.display()
            )
        )
    );

    // A crashed session's journal, before the conflict marker.
    let (t, p, _) = project(&[], true);
    let home = t.0.join("home");
    crashed_session(&home, &p, &[new_page(7777)]);
    let rel = first_page_file(&p);
    let bytes = after_the_first_line(&std::fs::read(p.join(&rel)).unwrap(), &[NOT_JSON, MARKER]);
    std::fs::write(p.join(&rel), &bytes).unwrap();
    let (before, journals) = (tree(&p), journal_files(&home));
    assert_eq!(
        run(&repair(&home, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: a crashed session left unsaved changes in its journal; replay or discard them first with \
                 `ew recover`; nothing was written\n",
                p.display()
            )
        )
    );
    assert!(
        tree(&p) == before && journal_files(&home) == journals,
        "nothing is written; the journal stays"
    );

    // A conflict marker (EW-CORE-006) beside a broken line.
    let (t, p, rel) = project(&[NOT_JSON, MARKER], false);
    let home = t.0.join("home");
    let before = tree(&p);
    assert_eq!(
        run(&repair(&home, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: EW-CORE-006 at {rel}:3 forces read-only, and `ew fmt --repair` discards only lines that \
                 are not JSON or not UTF-8 (EW-CORE-005); nothing was written; resolve it first (`ew check` lists the \
                 findings), then run `ew fmt --repair` again\n",
                p.display()
            )
        )
    );
    assert!(tree(&p) == before, "nothing is written");

    // The marker after 1,001 broken lines: its finding is beyond the cap of the open's list, and still refuses.
    let mut lines = vec![NOT_JSON; 1_001];
    lines.push(MARKER);
    let (t, p, _) = project(&lines, false);
    let home = t.0.join("home");
    let before = tree(&p);
    assert_eq!(
        run(&repair(&home, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: a finding beyond those the open lists forces read-only, and `ew fmt --repair` discards \
                 only lines that are not JSON or not UTF-8 (EW-CORE-005); nothing was written; resolve it first (`ew \
                 check` lists the findings), then run `ew fmt --repair` again\n",
                p.display()
            )
        )
    );
    assert!(tree(&p) == before, "nothing is written");

    // A project without a manifest: its files cannot be packed as found.
    let (t, p, _) = project(&[NOT_JSON], false);
    std::fs::remove_file(p.join("manifest.json")).unwrap();
    let home = t.0.join("home");
    let before = tree(&p);
    assert_eq!(
        run(&repair(&home, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the backup cannot be packed: the project has no manifest.json; nothing was written; keep a \
                 copy of the project folder and correct the broken lines by hand\n",
                p.display()
            )
        )
    );
    assert!(tree(&p) == before, "nothing is written");
    assert_eq!(beside_the_project(&p), Vec::<String>::new());

    // A file at the backup's name that is not this generation's backup.
    let (t, p, _) = project(&[NOT_JSON], true);
    let home = t.0.join("home");
    let backup = backup_of(&p, 1);
    std::fs::write(&backup, b"another file").unwrap();
    let before = tree(&p);
    assert_eq!(
        run(&repair(&home, &p, &[])),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: {} already exists and does not hold this generation's backup; nothing was written; move it \
                 out of the folder, then run `ew fmt --repair` again\n",
                p.display(),
                backup.display()
            )
        )
    );
    assert!(tree(&p) == before, "nothing is written");
    assert_eq!(std::fs::read(&backup).unwrap(), b"another file");

    // `--repair` with `--check`: the parser's refusal, before any open.
    assert_eq!(
        run(&repair(&home, &p, &["--check"])),
        (
            Some(2),
            String::new(),
            "error: the argument '--repair' cannot be used with '--check'\n\nUsage: ew fmt --repair <project-dir>\n\n\
             For more information, try '--help'.\n"
                .to_owned()
        )
    );
    drop(t);
}

/// FM-4: `ew recover` prints the journal's summary and changes nothing; `--discard` off a terminal exits 2
/// before the project is opened (the typed paths are unit tests with the injected terminal, `cli/project_files.rs`);
/// `--replay` saves the journal's transactions and removes it.
#[test]
fn recover_summarises_and_replays_a_crashed_session_journal() {
    let t = Tmp::new("ew-cli-test");
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

/// Replaying a segment whose base is not the open generation behaves exactly as the existing
/// code does for that case today (refused or applied). A crashed session set page 1's title; another session set the
/// same title and saved; `ew recover --replay` is refused (exit 1) and keeps the journal and the project unchanged.
#[test]
fn recover_refuses_a_replay_that_collides_with_a_later_save() {
    let t = Tmp::new("ew-cli-test");
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

/// `ew recover` checks that it got edit access; if not, it refuses with exit 2 and the same
/// text `ew apply` uses for a held lock; it never acts read-only. While this test process holds the project's lock,
/// `ew recover` and `ew apply` exit 2 with the same text, `ew apply`'s as built: the read-only reason a second open
/// sees, with the lock's holder (FR-COL-006); the crashed session's journal stays. The holder's user name
/// holds a right-to-left override (U+202E), which the refusal prints escaped.
#[test]
fn recover_and_apply_refuse_a_held_lock_with_the_same_text() {
    let t = Tmp::new("ew-cli-test");
    let home = t.0.join("home");
    let project = golden_copy(&t);
    crashed_session(&home, &project, &[new_page(7777)]);
    let journals = journal_files(&home);
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    store.identity.user = "ops\u{202E}fdp.exe".to_owned();
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
    let ew_api::Access::ReadOnly(ew_api::ReadOnlyReason::LockedBy(info)) = &seen.access else {
        panic!("the second open is read-only by the lock: {:?}", seen.access);
    };
    assert!(
        info.record
            .as_ref()
            .is_some_and(|r| !r.user.is_empty() && !r.host.is_empty()),
        "the reason names the holder: {info:?}"
    );
    // "another session holds the project" plus " (<user> on <host>, since
    // <time>)" when the lock record is readable (no `@`, no session or process ID).
    let holder_record = info.record.as_ref().unwrap();
    assert_eq!(holder_record.user, "ops\u{202E}fdp.exe");
    let expected = format!(
        "error: {}: the project opens read-only: another session holds the project (ops\\u{{202e}}fdp.exe on {}, since \
         {}); nothing was written\n",
        project.display(),
        holder_record.host,
        holder_record.started_at
    );
    let words = expected.split_once("read-only: ").unwrap().1;
    assert!(
        !words.contains(&holder_record.session) && !words.contains('@'),
        "{words}"
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

/// FM-6 through the binary: the golden tables import to a new project that is in canonical form and opens at
/// generation 1 (its model and check findings against the bootstrap are `crates/ew-api/tests/import_csv.rs`); the
/// files that differ from `golden/J-E-2416.ew` are printed (reported, not required). Without the starter content, or
/// into an `--out` that holds something, nothing is written (exit 2).
#[test]
fn import_csv_writes_the_golden_tables_as_a_new_project() {
    let t = Tmp::new("ew-cli-test");
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
/// default: nothing is written, exit 1) and EW-IMP-008 under `--lenient` (written, exit 0); an unreadable
/// table exits 2 with its line, and its column where the reader names one, before anything is written.
#[test]
fn import_csv_strict_lenient_and_unreadable_tables() {
    let t = Tmp::new("ew-cli-test");
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
