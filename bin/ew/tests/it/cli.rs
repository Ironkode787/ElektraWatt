// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of the `ew` binary.

use std::path::PathBuf;
use std::process::Output;

use crate::fmt_and_recover::{put_as_saved, sample_generation};
use crate::support::{
    CONTENT, PARTS, SAMPLE, Tmp, copy_dir, device_file, renumber_log, stderr, stdout, tree,
};

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

/// `ew <args>` in a fresh folder of its own ([`Tmp::ew`]).
fn ew(args: &[&std::ffi::OsStr]) -> Output {
    Tmp::new("ew-cli-run").ew(args)
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
        SAMPLE.as_ref(),
        "--scheme".as_ref(),
        "bogus".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(2), "an unknown --scheme: {}", stderr(&o));
}

/// `ew check` of the sample in each output format: JSON (the check result of the pre-delivery scheme, exit 0: the sample
/// is clean) and Markdown (the report title); neither leaves a lock behind.
#[test]
fn check_the_sample_in_each_format() {
    let check = |format: &str| {
        ew(&[
            "check".as_ref(),
            SAMPLE.as_ref(),
            "--format".as_ref(),
            format.as_ref(),
        ])
    };
    let o = check("json");
    let code = o.status.code();
    assert_eq!(code, Some(0), "case json: {}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["$schema"], "ew.check-result/1");
    assert_eq!(v["scheme"], "pre-delivery");
    assert_eq!(v["exit_code"].as_i64(), Some(0));
    assert!(!std::path::Path::new(SAMPLE).join(".~ew.lock").exists());
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
    copy_dir(std::path::Path::new(SAMPLE), &dir);
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
    copy_dir(std::path::Path::new(SAMPLE), &dir);
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
    // The sample names none: the default.
    let o = ew(&[
        "check".as_ref(),
        SAMPLE.as_ref(),
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
    copy_dir(std::path::Path::new(SAMPLE), &dir);
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
    let (sample_code, _, sample_err) = run(std::path::Path::new(SAMPLE), &[]);
    assert_eq!(code, sample_code, "the exit status is unchanged");
    assert!(matches!(code, Some(0 | 1)), "{stderr}");
    assert!(!sample_err.contains("not shipped"), "{sample_err}");
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

/// P5-2: `ew check --lang nl-NL` prints the Dutch text of the rules with an nl-NL pattern (EW-POT-006 on the sample's
/// shield wire, set to 1.5 mm² for the test);
/// the default and `--lang en-GB` print the en-GB message; an unknown language is a usage error. The counts and the
/// exit status do not depend on the language. A rule without an nl-NL pattern falls back to en-GB in `ew-checks`'
/// `messages` tests.
#[test]
fn check_lang_renders_the_messages_in_a_shipped_language() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let log = t.0.join("shield.ewcmd");
    std::fs::write(
        &log,
        format!(
            "{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"the shield set below its line conductors\"}}\n{}\n",
            serde_json::json!({"cmd": "fields.set", "params": {"kind": "connection",
                "targets": [crate::merge_driver::shield_id()], "path": "overrides.csa", "value": "1.5"}})
        ),
    )
    .unwrap();
    let o = t.ew(&["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let check = |lang: Option<&str>| {
        let mut args: Vec<&std::ffi::OsStr> = vec!["check".as_ref(), project.as_os_str()];
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
        en_text.contains("-103W1/SHIELD: PE wire -103W1/SHIELD has 1.5 mm², below the 2.5 mm² required"),
        "{en_text}"
    );
    assert_eq!(stdout(&check(Some("en-GB"))), en_text);
    let nl = check(Some("nl-NL"));
    assert_eq!(nl.status.code(), Some(1));
    let nl_text = stdout(&nl);
    assert!(
        nl_text.contains("-103W1/SHIELD: PE-draad -103W1/SHIELD heeft 1,5 mm², minder dan de 2,5 mm²"),
        "{nl_text}"
    );
    // The same lines in the same order, the totals line equal.
    assert_eq!(nl_text.lines().count(), en_text.lines().count());
    assert_eq!(nl_text.lines().last(), en_text.lines().last());
    let bad = check(Some("xx-YY"));
    assert_eq!(bad.status.code(), Some(2));
    assert!(
        stderr(&bad).contains("[possible values: en-GB, nl-NL]"),
        "{}",
        stderr(&bad)
    );
}

fn sql(query: &str) -> Output {
    ew(&["sql".as_ref(), SAMPLE.as_ref(), query.as_ref()])
}

/// An inline query may start with an SQL comment; the count prints as CSV, stderr reports the row count, and the
/// read leaves no lock behind.
#[test]
fn sql_inline_query_may_start_with_a_comment() {
    let o = sql("-- the sample's terminals\nSELECT count(*) AS n FROM api_v1_terminal");
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        stdout(&o),
        format!("n\n{}\n", ew_testdir::sample_facts().terminals)
    );
    assert!(stderr(&o).contains("1 rows"), "{}", stderr(&o));
    assert!(!std::path::Path::new(SAMPLE).join(".~ew.lock").exists());
}

#[test]
fn sql_huge_timeout_is_a_usage_error_not_a_panic() {
    let o = ew(&[
        "sql".as_ref(),
        SAMPLE.as_ref(),
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
        SAMPLE.as_ref(),
        "SELECT designation FROM api_v1_terminal WHERE strip_tag = '-X1021' ORDER BY designation COLLATE ew_natural LIMIT 3"
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
    let o = ew(&["sql".as_ref(), SAMPLE.as_ref(), "--file".as_ref(), f.as_os_str()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    // The two bridges of -X1021 (terminals 1 to 4 and 5 to 8).
    assert_eq!(stdout(&o), "n\n2\n");
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
    Tmp::new("ew-cli-run")
        .ew_command(args)
        .env("EW_DEBUG_PANIC", site)
        .output()
        .unwrap()
}

/// The last line of a command's stderr (a caught panic's message from the default hook comes first).
fn last_line(o: &Output) -> String {
    stderr(o).lines().last().unwrap_or_default().to_owned()
}

/// At the call site: `ew sql`'s statement panics inside `run_sql`'s guard; the run ends
/// with S1 and exit 2, prints no result, and the copy of the sample's files are as they were.
#[test]
fn sql_a_panicking_statement_ends_with_s1_and_changes_nothing() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
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
    let project = sample_copy(&t);
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

/// At the call site: `ew import table --apply`'s read panics inside
/// `run_import_table`'s guard; the run ends with `error: ` and S2 with its first letter in lower case, exit 2, and the
/// copy of the sample's files are as they were.
#[test]
fn import_table_a_panicking_read_ends_with_s2_and_changes_nothing() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
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

/// A recursive-CTE bomb in `ew sql` on a copy of the sample stops at its deadline with
/// exit 2, and the copy's files are as they were.
#[test]
fn sql_a_recursive_cte_bomb_stops_at_its_deadline_and_changes_nothing() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
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

/// `ew export cables` and `ew export bom` print their lists of the sample: the cable list, with its row count on stderr
/// (the overview's heading and 3 cables, then each cable's heading and its 5, 5 and 3 cores, shields included: 20) and
/// no lock left behind; the parts list by default, with `--mode parts` and with its alias `component`, the same bytes;
/// and the assembly list, the parts list with the column "Covers", empty in the sample, which holds no assembly.
#[test]
fn export_csv_prints_the_lists() {
    let export = |args: &[&str]| {
        let mut a: Vec<&std::ffi::OsStr> = vec!["export".as_ref(), args[0].as_ref(), SAMPLE.as_ref()];
        a.extend(args[1..].iter().map(|s| std::ffi::OsStr::new(*s)));
        let o = ew(&a);
        assert_eq!(o.status.code(), Some(0), "{args:?}: {}", stderr(&o));
        o
    };
    let cables = export(&["cables"]);
    assert_eq!(stderr(&cables), "20 rows\n");
    assert_eq!(stdout(&cables).lines().count(), 21, "the header and 20 rows");
    assert!(!std::path::Path::new(SAMPLE).join(".~ew.lock").exists());
    let parts = stdout(&export(&["bom"]));
    for mode in ["parts", "component"] {
        assert_eq!(stdout(&export(&["bom", "--mode", mode])), parts, "--mode {mode}");
    }
    let assembly = stdout(&export(&["bom", "--mode", "assembly"]));
    let with_covers: String = parts
        .lines()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                format!("{l},Covers\n")
            } else {
                format!("{l},\n")
            }
        })
        .collect();
    assert_eq!(assembly, with_covers);
}

/// A command that writes a file says so on stdout and writes it, one case per writer: `ew sql --out` (the CSV),
/// `ew sql dump` (an SQLite file) and `ew export cables --out` (the cable list it prints).
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
                SAMPLE.as_ref(),
                "SELECT count(*) AS n FROM api_v1_terminal".as_ref(),
                "--out".as_ref(),
                csv.as_os_str(),
            ],
            "result written to",
            &csv,
            |bytes| assert_eq!(bytes, b"n\n18\n"),
        ),
        (
            "sql_dump_writes_sqlite_file",
            vec![
                "sql".as_ref(),
                "dump".as_ref(),
                SAMPLE.as_ref(),
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
                SAMPLE.as_ref(),
                "--out".as_ref(),
                cables.as_os_str(),
            ],
            "export written to",
            &cables,
            |bytes| assert!(String::from_utf8_lossy(bytes).starts_with("row,Cable,Cable type,")),
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
pub(crate) fn tiny_project(t: &Tmp) -> PathBuf {
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
            SAMPLE.as_ref(),
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

/// `ew verify-derived` over the sample: every memoised value equals its cold recomputation with the oracle on.
/// The oracle re-runs every memoised read, about 90 s in a debug build, so the test is run by hand
/// (`cargo nextest run -p ew --run-ignored only`) and by `ew-api`'s `memo_oracle` tests at the library level.
#[test]
#[ignore = "deep run: the oracle re-runs every memoised read"]
fn verify_derived_of_the_sample_is_clean() {
    let o = ew(&["verify-derived".as_ref(), SAMPLE.as_ref()]);
    assert_eq!(o.status.code(), Some(0), "{}", stdout(&o));
    assert!(
        stdout(&o).contains("0 differences, 0 oracle mismatches"),
        "{}",
        stdout(&o)
    );
}

/// `ew verify-migration`: exit 0 and the summary for a project against its own PDF; exit 1 and the deviations against
/// the PDF of another drawing, unless they are declared; exit 2 and a sentence for a reference that is no PDF and
/// for a declaration that is no list.
#[test]
fn verify_migration_exits_by_what_it_finds() {
    let t = Tmp::new("ew-cli-test");
    let dir = tiny_project(&t);
    let own = t.0.join("own.pdf");
    let o = ew(&[
        "export".as_ref(),
        "pdf".as_ref(),
        dir.as_os_str(),
        "--out".as_ref(),
        own.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let o = ew(&[
        "verify-migration".as_ref(),
        dir.as_os_str(),
        "--reference".as_ref(),
        own.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), stderr(&o));
    assert!(stdout(&o).starts_with("verify-migration: "), "{}", stdout(&o));
    assert!(stdout(&o).contains(", 0 deviations, 0 expected deltas found, 0 expected deltas not found"));
    // The raster was compared, not skipped.
    assert!(!stdout(&o).contains(" 0 cells compared"), "{}", stdout(&o));

    // Another drawing: the sample's page 101 as the reference of the one-page project.
    let other = t.0.join("other.pdf");
    let o = ew(&[
        "export".as_ref(),
        "pdf".as_ref(),
        SAMPLE.as_ref(),
        "--pages".as_ref(),
        "101".as_ref(),
        "--out".as_ref(),
        other.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let o = ew(&[
        "verify-migration".as_ref(),
        dir.as_os_str(),
        "--reference".as_ref(),
        other.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(1), "{}{}", stdout(&o), stderr(&o));
    assert!(stdout(&o).contains(": text-missing: "), "{}", stdout(&o));
    let declared = t.0.join("declared.json");
    std::fs::write(
        &declared,
        r#"[{"kind":"text-missing"},{"kind":"raster-cell"},{"kind":"size-differs"},{"kind":"text-moved"}]"#,
    )
    .unwrap();
    let o = ew(&[
        "verify-migration".as_ref(),
        dir.as_os_str(),
        "--reference".as_ref(),
        other.as_os_str(),
        "--expected".as_ref(),
        declared.as_os_str(),
    ]);
    // Every deviation but those declared remains; a declared kind that was not found is named.
    assert!(stdout(&o).contains(" (expected): "), "{}", stdout(&o));

    let bad = t.0.join("bad.pdf");
    std::fs::write(&bad, b"not a pdf").unwrap();
    let o = ew(&[
        "verify-migration".as_ref(),
        dir.as_os_str(),
        "--reference".as_ref(),
        bad.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(2), "{}", stdout(&o));
    assert!(
        stderr(&o).starts_with("error: the reference PDF: "),
        "{}",
        stderr(&o)
    );
    std::fs::write(&declared, "{}").unwrap();
    let o = ew(&[
        "verify-migration".as_ref(),
        dir.as_os_str(),
        "--reference".as_ref(),
        own.as_os_str(),
        "--expected".as_ref(),
        declared.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        stderr(&o).contains("the expected deltas are no list of deltas"),
        "{}",
        stderr(&o)
    );
}

// ---------------------------------------------------------------------------------------------------------
// `ew export pdf|svg` (G-M1-01) and `ew apply`

/// A copy of the sample in `t`.
pub(crate) fn sample_copy(t: &Tmp) -> PathBuf {
    t.sample("sample.ew")
}

/// G-M1-01: `ew export pdf <sample>` writes every page of the page order as one PDF, the same bytes on one thread and
/// on four, writes nothing into the project and leaves no `.tmp` file. The PDF's structure and its links are pinned by
/// `ew-api`'s page-output tests.
#[test]
fn export_pdf_writes_every_page_alike_on_one_and_four_threads() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let before = tree(&project);
    let order = t.ew(&[
        "query".as_ref(),
        project.as_os_str(),
        "page_order".as_ref(),
        "-".as_ref(),
    ]);
    assert_eq!(order.status.code(), Some(0), "{}", stderr(&order));
    let pages = stdout(&order).lines().count();
    let mut first: Option<Vec<u8>> = None;
    for threads in ["1", "4"] {
        let out = t.0.join(format!("sample-{threads}.pdf"));
        let o = t.ew(&[
            "export".as_ref(),
            "pdf".as_ref(),
            project.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--threads".as_ref(),
            threads.as_ref(),
        ]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert!(
            stdout(&o).contains(&format!(": {pages} pages, ")),
            "{}",
            stdout(&o)
        );
        let bytes = std::fs::read(&out).unwrap();
        assert!(
            *first.get_or_insert_with(|| bytes.clone()) == bytes,
            "{threads} threads"
        );
        assert!(!t.0.join(format!("sample-{threads}.pdf.tmp")).exists());
    }
    assert!(tree(&project) == before, "the export wrote into the project");
}

/// `ew export svg --pages 101-105` writes one file per selected page; an unknown folio, a missing `--out` and a
/// missing project exit 2.
#[test]
fn export_svg_writes_the_selected_pages() {
    let t = Tmp::new("ew-cli-test");
    let dir = t.0.join("svg");
    let o = t.ew(&[
        "export".as_ref(),
        "svg".as_ref(),
        SAMPLE.as_ref(),
        "--out".as_ref(),
        dir.as_os_str(),
        "--pages".as_ref(),
        "101-105".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let files: Vec<String> = tree(&dir)
        .into_keys()
        .filter(|n| n != ew_api::file::FILE_SET_MARK)
        .collect();
    assert_eq!(files, ["101.svg", "102.svg", "103.svg", "104.svg", "105.svg"]);
    let unknown = ew(&[
        "export".as_ref(),
        "svg".as_ref(),
        SAMPLE.as_ref(),
        "--out".as_ref(),
        dir.as_os_str(),
        "--pages".as_ref(),
        "106".as_ref(),
    ]);
    assert_eq!(unknown.status.code(), Some(2), "{}", stderr(&unknown));
    assert!(stderr(&unknown).contains("106"), "{}", stderr(&unknown));
    let no_out = ew(&["export".as_ref(), "pdf".as_ref(), SAMPLE.as_ref()]);
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

/// `ew apply --preview` of a renumbering of section 1 from 111 prints the renumbered tags and changes nothing on disk;
/// `ew apply` commits and saves it (folio 101 becomes 111, which `ew export svg --pages 111` then finds), leaving no
/// lock.
#[test]
fn apply_of_a_renumbering_previews_then_saves() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let log = renumber_log(&t);
    let before = tree(&project);
    let preview = t.ew(&[
        "apply".as_ref(),
        project.as_os_str(),
        log.as_os_str(),
        "--preview".as_ref(),
    ]);
    assert_eq!(preview.status.code(), Some(0), "{}", stderr(&preview));
    let out = stdout(&preview);
    assert!(out.starts_with("preview: 1 of 1 lines commit;"), "{out}");
    assert!(out.contains("-101F1 -> -111F1"), "{out}");
    assert!(out.contains(" cross-references change"), "{out}");
    assert!(tree(&project) == before, "a preview wrote into the project");
    let svg = |pages: &str| {
        t.ew(&[
            "export".as_ref(),
            "svg".as_ref(),
            project.as_os_str(),
            "--out".as_ref(),
            t.0.join("svg").as_os_str(),
            "--pages".as_ref(),
            pages.as_ref(),
        ])
    };
    assert_eq!(
        svg("111").status.code(),
        Some(2),
        "no folio 111 before the replay"
    );
    let applied = t.ew(&["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    assert!(
        stdout(&applied).contains("applied 1 of 1 lines; saved"),
        "{}",
        stdout(&applied)
    );
    assert!(tree(&project) != before, "the replay is saved");
    assert!(!project.join(".~ew.lock").exists(), "the lock is released");
    assert_eq!(svg("111").status.code(), Some(0), "folio 111 after the replay");
}

/// Exit codes of `ew apply` (doc 17 section 3.2): a line a domain rule refuses (a second page 7777) exits 1, the
/// line before it is saved, and with `--one-step` nothing is saved; a malformed log and an unresolvable
/// reference exit 2 and save nothing.
#[test]
fn apply_exit_codes_and_one_step() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
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
    let one_step = t.ew(&[
        "apply".as_ref(),
        project.as_os_str(),
        dup.as_os_str(),
        "--one-step".as_ref(),
    ]);
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
    let o = t.ew(&["apply".as_ref(), project.as_os_str(), bad_ref.as_os_str()]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    let malformed = write("bad.ewcmd", &["{\"cmd\":1}"]);
    let o = t.ew(&["apply".as_ref(), project.as_os_str(), malformed.as_os_str()]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(tree(&project) == before, "a refused log saved something");
    let partial = t.ew(&["apply".as_ref(), project.as_os_str(), dup.as_os_str()]);
    assert_eq!(partial.status.code(), Some(1), "{}", stderr(&partial));
    assert!(
        stdout(&partial).contains("applied 1 of 2 lines; saved"),
        "{}",
        stdout(&partial)
    );
    assert!(tree(&project) != before, "the committed line is saved");
    assert!(!project.join(".~ew.lock").exists());
}

/// `SOURCE_DATE_EPOCH` (doc 17 section 3.1) sets the creation date and with it the document ID: the PDF differs
/// from the model-dated one, carries the date, and is the same bytes on one and four threads; an invalid value
/// exits 2 and writes nothing.
#[test]
fn source_date_epoch_dates_the_pdf() {
    let t = Tmp::new("ew-cli-test");
    let export = |name: &str, threads: &str, epoch: &str| {
        let out = t.0.join(name);
        let o = t.ew_at(
            &[
                "export".as_ref(),
                "pdf".as_ref(),
                SAMPLE.as_ref(),
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
    let undated = t.0.join("u.pdf");
    let o = t.ew(&[
        "export".as_ref(),
        "pdf".as_ref(),
        SAMPLE.as_ref(),
        "--out".as_ref(),
        undated.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        std::fs::read(&undated).unwrap() != a,
        "the model-dated PDF differs"
    );
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
    let dir = t.0.join("svg");
    let svg = |pages: &str, dir: &std::path::Path| {
        t.ew(&[
            "export".as_ref(),
            "svg".as_ref(),
            SAMPLE.as_ref(),
            "--out".as_ref(),
            dir.as_os_str(),
            "--pages".as_ref(),
            pages.as_ref(),
        ])
    };
    assert_eq!(svg("1,2,101", &dir).status.code(), Some(0));
    assert_eq!(svg("101-105", &dir).status.code(), Some(0));
    let files: Vec<String> = tree(&dir)
        .into_keys()
        .filter(|n| n != ew_api::file::FILE_SET_MARK)
        .collect();
    assert_eq!(files, ["101.svg", "102.svg", "103.svg", "104.svg", "105.svg"]);
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
    let project = sample_copy(&t);
    // A session that commits one edit and is dropped without a save or a clean close: its journal stays.
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = Some(t.state_dir());
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
    let journals = journal_files(&t);
    assert_eq!(journals.len(), 1);
    let log = renumber_log(&t);
    let o = t.ew(&["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
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
    // The other session's journal is still there, unchanged. The second half, a clean close deletes only
    // its own segment, is `Journal::close` (`crates/ew-store/src/journal.rs`), shown by
    // `recovery_replays_the_newest_journal_and_keeps_the_other_unchanged` (`crates/ew-store/tests/it/recovery.rs`)
    // and `the_app_path_recovers_one_journal_at_a_time` (`crates/ew-api/tests/it/recovery.rs`).
    assert!(journal_files(&t) == journals, "the journal is still there");
}

/// Each kind of subcommand that prints `journal_refusal`'s text (`ew apply`, `ew import parts`, a human-only
/// subcommand) has a whole-text test. `ew import parts` refuses a project whose crashed session
/// left a journal with `ew apply`'s text (exit 2, before the table is read) and writes nothing; the journal stays. The
/// human-only kind is a unit test with the injected terminal (`cli/project_files.rs`).
#[test]
fn import_parts_refuses_a_project_with_a_crashed_session_journal() {
    let t = Tmp::new("ew-cli-test");
    let project = parts_target(&t, "parts-journal.ew");
    crashed_session(&t, &project, &[new_page(7777)]);
    let before = tree(&project);
    let journals = journal_files(&t);
    assert_eq!(journals.len(), 1);
    let o = t.ew(&[
        "import".as_ref(),
        "parts".as_ref(),
        PARTS.as_ref(),
        "--project".as_ref(),
        project.as_os_str(),
    ]);
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
    assert!(journal_files(&t) == journals, "the journal stays");
}

/// `ew project checks set` submits the `HumanOnly` command
/// `project.checks.set` only after a confirmation typed on a controlling terminal (doc 17 section 2.3, FR-EXT-030).
/// Without a terminal (here: stdout captured, stdin empty or piped) it exits 2 before the project is opened: nothing
/// changes, no lock, no journal. A scheme that does not ship exits 1 without asking; a usage error (no scheme, a
/// `--confirm` or `--yes` flag) exits 2. The typed path is a unit test with an injected reader and writer (`cli.rs`).
#[test]
fn project_checks_set_needs_an_interactive_terminal() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let before = tree(&project);
    let set = |extra: &[&str]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "project".as_ref(),
            "checks".as_ref(),
            "set".as_ref(),
            project.as_os_str(),
        ];
        args.extend(extra.iter().map(|a| std::ffi::OsStr::new(*a)));
        t.ew(&args)
    };
    let o = set(&["--active", "drafting", "--release", "full"]);
    assert_eq!(o.status.code(), Some(2), "{}{}", stdout(&o), stderr(&o));
    assert_eq!(
        stderr(&o),
        "error: HumanOnly: needs an interactive terminal (`project checks set` is confirmed by typing the scheme \
         name); nothing was changed\n"
    );
    // The answer on a pipe does not count: a pipe is no terminal.
    let mut child = t
        .ew_command(&["project", "checks", "set"])
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
    let state = t.state_dir();
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
        t.ew(&args)
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
    let dir = sample_copy(&t);
    let manifest = std::fs::read(dir.join("manifest.json")).unwrap();
    let dir_arg = dir.to_str().unwrap();
    for args in [
        &["revision", "approve", dir_arg, "--as", "checker"][..],
        &["revision", "release", "V2.0", dir_arg],
        &["revision", "withdraw", "V1.0", dir_arg, "--reason", "wrong"],
    ] {
        let args: Vec<&std::ffi::OsStr> = args.iter().map(std::ffi::OsStr::new).collect();
        let o = t.ew(&args);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}{}", stdout(&o), stderr(&o));
        assert!(
            stderr(&o).contains("HumanOnly: needs an interactive terminal"),
            "{}",
            stderr(&o)
        );
        assert!(!dir.join(".~ew.lock").exists(), "the project was not opened");
    }
    assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), manifest);
    let o = t.ew(&["revision".as_ref(), "approve".as_ref(), dir.as_os_str()]);
    assert_eq!(o.status.code(), Some(2), "`--as` is required: {}", stderr(&o));
}

/// The recorded logs replay through `ew apply`: one log on a copy of the sample, applied once, of
/// - `project.settings.set` (the drawing setting `marker_mode`), `box.structure`, `item.place`,
///   `marker.place` and `rep.move`;
/// - `fields.set` and `fields.set_many`, the commands the property grid and find and replace record, their targets
///   named by tag reference (the headless tests of `elektrawatt` replay each script's own `--record` log in-process
///   and compare it with the records the script saved).
///
/// Every line applies and the saved records are those the same log gives in-process (the spec's replay path), the
/// minted page items and structure boxes compared by content.
#[test]
fn the_recorded_logs_replay_through_ew_apply() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let (sample, _) = ew_api::Session::open_read_only(&project, "test").unwrap();
    let tables = sample.page_tables("101").unwrap();
    let page = tables.page.to_string();
    let wires: Vec<String> = tables
        .connections
        .iter()
        .filter(|c| c.kind == "wire")
        .take(2)
        .map(|c| c.id.to_string())
        .collect();
    let rep = tables.reps[0].id.to_string();
    let (items_before, boxes_before) = (tables.items.len(), tables.structure_boxes.len());
    drop(sample);
    let header = "{\"$schema\":\"ew.cmdlog/1\",\"label\":\"logs\"}";
    let mut lines = vec![
        serde_json::json!({"cmd": "project.settings.set", "params": {"member": "drawing", "value": {"marker_mode": "explicit"}}}),
        serde_json::json!({"cmd": "box.structure", "params": {"page": page,
            "outline": [[0, 0], [10_000_000, 0], [10_000_000, 10_000_000]], "style": "dotted", "caption": "Box",
            "assigns": {}}}),
        serde_json::json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": "40x60", "at": [50_800_000, 30_480_000]}}}}),
        serde_json::json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"graphic": {"shape": {"line": {"a": [0, 0], "b": [10_000_000, 0]}}, "dash": "dashed"}}}}),
        serde_json::json!({"cmd": "marker.place", "params": {"page": page, "connections": wires,
            "at": [60_000_000, 40_000_000]}}),
        serde_json::json!({"cmd": "fields.set", "params": {"kind": "device", "path": "function_text",
            "targets": [{"ref": "tag:-102F1"}], "value": "Supply 24 V"}}),
        serde_json::json!({"cmd": "fields.set_many", "params": {"edits": [
            {"kind": "device", "path": "tag.frozen", "targets": [{"ref": "tag:-102S1"}],
                "value": {"sign": "shown", "text": "102S1"}},
            {"kind": "device", "path": "function_text", "targets": [{"ref": "tag:-102S2"}], "value": "Reserve"}]}}),
    ];
    let text = |lines: &[serde_json::Value]| {
        let mut out = String::from(header);
        for l in lines {
            out.push('\n');
            out.push_str(&l.to_string());
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
    let applied = t.ew(&["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(applied.status.code(), Some(0), "{}", stderr(&applied));
    let n = lines.len();
    assert!(
        stdout(&applied).contains(&format!("applied {n} of {n} lines; saved")),
        "{}",
        stdout(&applied)
    );
    let fresh = t.0.join("fresh.ew");
    copy_dir(std::path::Path::new(SAMPLE), &fresh);
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
    // The two projects' items and boxes on 101 by content: a marker's connections are the sample's connection IDs,
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
    // The log places a free text, a line and a marker on 101, beside the items the sample holds there.
    assert_eq!(saved_tables.items.len(), items_before + 3);
    assert_eq!(saved_tables.structure_boxes.len(), boxes_before + 1);
}

/// A fresh saved project without parts (the import target).
fn parts_target(t: &Tmp, name: &str) -> PathBuf {
    saved_project(t, name, "3b9e51c4-8a27-7d10-b5f3-6c0a2e9d4f18")
}

/// `ew import parts` creates a part copy per row of the test tables' `parts.csv`, and the recorded command
/// replays through `ew apply` to the same bytes.
#[test]
fn import_parts_creates_a_copy_per_row_and_replays_through_apply() {
    let t = Tmp::new("ew-cli-test");
    let a = parts_target(&t, "parts-a.ew");
    let b = t.0.join("parts-b.ew");
    copy_dir(&a, &b);
    let o = t.ew(&[
        "import".as_ref(),
        "parts".as_ref(),
        PARTS.as_ref(),
        "--project".as_ref(),
        a.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let rows = ew_testdir::table_rows("parts.csv");
    assert_eq!(
        stdout(&o),
        format!("imported parts.csv: {rows} created, 0 skipped, 0 errors; saved\n")
    );
    let mine = tree(&a.join("lib/parts"));
    assert_eq!(mine.len(), rows);

    let parts = {
        let (session, _) = ew_api::Session::open_reader(&b, "test", "test").expect("the copy opens");
        let parts = ew_api::import::read_parts_tables(std::path::Path::new(PARTS), &session).unwrap();
        session.close();
        parts
    };
    let log = format!(
        "{}\n{}\n",
        r#"{"$schema":"ew.cmdlog/1","label":"test"}"#,
        serde_json::json!({"cmd": "parts.import", "params": parts.params()})
    );
    let log_path = t.0.join("parts.ewcmd");
    std::fs::write(&log_path, log).unwrap();
    let o = t.ew(&["apply".as_ref(), b.as_os_str(), log_path.as_os_str()]);
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
    let a = parts_target(&t, "parts-a.ew");
    let run = || {
        t.ew(&[
            "import".as_ref(),
            "parts".as_ref(),
            PARTS.as_ref(),
            "--project".as_ref(),
            a.as_os_str(),
        ])
    };
    assert_eq!(run().status.code(), Some(0));
    let before = tree(&a.join("lib"));
    let o = run();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let rows = ew_testdir::table_rows("parts.csv");
    assert_eq!(
        stdout(&o),
        format!("imported parts.csv: 0 created, {rows} skipped, 0 errors; saved\n")
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
    let a = parts_target(&t, "parts-a.ew");
    let before = tree(&a);
    let run = |file: &std::path::Path| {
        t.ew(&[
            "import".as_ref(),
            "parts".as_ref(),
            file.as_os_str(),
            "--project".as_ref(),
            a.as_os_str(),
        ])
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

/// `ew lib`: create a user library, list the tiers, search the starter, rebuild the index; the
/// index lives in the cache directory.
#[test]
fn lib_create_list_search_and_rebuild() {
    let tmp = Tmp::new("ew-cli-test");
    let data = tmp.0.join("data");
    let cache = tmp.0.join("cache");
    let run = |args: &[&str]| tmp.ew_with(args, true);
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
    let data = tmp.0.join("data");
    let run = |args: &[&std::ffi::OsStr]| tmp.ew(args);
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

/// The journal files of the isolated `ew` in `t` with their bytes.
pub(crate) fn journal_files(t: &Tmp) -> std::collections::BTreeMap<String, Vec<u8>> {
    let dir = t.state_dir();
    if !dir.exists() {
        return std::collections::BTreeMap::new();
    }
    tree(&dir)
        .into_iter()
        .filter(|(name, _)| name.ends_with(".jsonl"))
        .collect()
}

/// A session on `project` with the journals of the isolated `ew` in `t` that runs `cmds` and is dropped without a
/// save or a clean close: its journal stays, as a crashed session's does.
pub(crate) fn crashed_session(t: &Tmp, project: &std::path::Path, cmds: &[ew_api::AnyCommand]) {
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = Some(t.state_dir());
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

pub(crate) fn new_page(number: u32) -> ew_api::AnyCommand {
    ew_api::AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({"number": number, "title": {"en": "T"}}),
    }
}

/// The page labels of the saved project.
pub(crate) fn saved_pages(project: &std::path::Path) -> Vec<String> {
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

/// `ew <args>` isolated in `t`, with `extra` after them.
pub(crate) fn ew_in(t: &Tmp, args: &[&std::ffi::OsStr], extra: &[&str]) -> Output {
    let mut all: Vec<&std::ffi::OsStr> = args.to_vec();
    all.extend(extra.iter().map(std::ffi::OsStr::new));
    t.ew(&all)
}

/// Swaps the first two lines of the project file `rel`.
pub(crate) fn swap_first_lines(project: &std::path::Path, rel: &str) {
    let file = project.join(rel);
    let text = std::fs::read_to_string(&file).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    lines.swap(0, 1);
    std::fs::write(&file, format!("{}\n", lines.join("\n"))).unwrap();
}

/// `ew fmt --check` on the sample's copy exits 0; after two lines of a device file are swapped it
/// exits 1 naming that file and writes nothing; `ew fmt` rewrites that file only (it saves through the plan that starts from
/// `Loaded.rewrite`, never `Changes::All`); then
/// `ew fmt --check` exits 0 and the files equal the original bytes. The manifest is a new generation by necessity; its
/// file hashes equal the original ones.
#[test]
fn fmt_check_names_a_swapped_file_and_fmt_restores_its_bytes() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let fmt = |extra: &[&str]| ew_in(&t, &["fmt".as_ref(), project.as_os_str()], extra);
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

    let rel = device_file(&project);
    swap_first_lines(&project, &rel);
    let swapped = tree(&project);
    assert_eq!(
        run(&fmt(&["--check"])),
        (
            Some(1),
            format!("would rewrite {rel}\n"),
            "fmt: 1 file is not in canonical form; nothing was written\n".to_owned()
        )
    );
    assert!(tree(&project) == swapped, "`--check` writes nothing");

    assert_eq!(
        run(&fmt(&[])),
        (
            Some(0),
            format!("rewrote {rel}\n"),
            format!(
                "fmt: rewrote 1 file; saved generation {}\n",
                sample_generation() + 1
            )
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
        [&rel, "manifest.json"],
        "only the rewrite set and the manifest"
    );
    assert!(journal_files(&t).is_empty(), "the format's journal is closed");
}

/// A line with an unknown member survives `ew fmt` verbatim, and a quarantined line
/// (EW-CORE-007, a schema violation) is listed as kept and stays in its file, while the file they share is rewritten
/// for a swap. A line that is not JSON (EW-CORE-005) makes the project read-only: `ew fmt --check` lists it, and `ew
/// fmt` is refused and writes nothing, so no line it cannot parse is lost.
#[test]
fn fmt_keeps_unknown_members_and_quarantined_lines_verbatim() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let rel = device_file(&project);
    let rel = rel.as_str();
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

    let fmt = |extra: &[&str]| ew_in(&t, &["fmt".as_ref(), project.as_os_str()], extra);
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
            format!(
                "{kept_text}fmt: rewrote 1 file; saved generation {}\n",
                sample_generation() + 1
            )
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
    copy_dir(std::path::Path::new(SAMPLE), &broken);
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
    let fmt = |extra: &[&str]| ew_in(&t, &["fmt".as_ref(), broken.as_os_str()], extra);
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

/// A writer refused on a project whose damaged library copy a device uses names the
/// device, the copy in words and the way out, and writes nothing.
#[test]
fn a_writer_names_the_damaged_copy_a_device_uses() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    // The sample holds starter parts only: -101F1 takes a supplier's part the test tables import.
    let o = t.ew(&[
        "import".as_ref(),
        "parts".as_ref(),
        PARTS.as_ref(),
        "--project".as_ref(),
        project.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let file = copy_file(&project, "5SY4204-7");
    let copy = file.file_stem().unwrap().to_string_lossy().into_owned();
    let log = t.0.join("part.ewcmd");
    std::fs::write(
        &log,
        format!(
            "{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"test\"}}\n{}\n",
            serde_json::json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
                "path": "parts", "value": [{"part": {"copy": copy}, "qty": "1"}]}})
        ),
    )
    .unwrap();
    let o = t.ew(&["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    std::fs::write(&file, "{\"$k\": \"part\",\n").unwrap();
    let before = tree(&project);
    let o = ew_in(&t, &["fmt".as_ref(), project.as_os_str()], &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the project opens read-only: the device -101F1 uses the library copy of its part, \
                 which is damaged; update the copy from its library first; nothing was written\n",
                project.display()
            )
        )
    );
    assert!(tree(&project) == before, "nothing is written");
}

/// A writer refused on a project whose damaged library copy is of a part an installed library holds
/// (the compiled-in starter parts' EW-GEN-MCB-4 of the sample's -101F1) names the part and its library, whole.
#[test]
fn a_writer_names_the_damaged_copy_of_an_installed_part_whole() {
    let t = Tmp::new("ew-cli-test");
    let (project, file) = starter_mcb_project(&t);
    std::fs::write(&file, "{\"$k\": \"part\",\n").unwrap();
    let before = tree(&project);
    let o = ew_in(&t, &["fmt".as_ref(), project.as_os_str()], &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: {}: the project opens read-only: the device -101F1 uses the library copy of part EW-GEN-MCB-4 \
                 from ElektraWatt starter parts, which is damaged; update the copy from its library first; nothing \
                 was written\n",
                project.display()
            )
        )
    );
    assert!(tree(&project) == before, "nothing is written");
}

/// A writer refused on a project whose damaged library copy no device names (the form page 1 names: in the sample the
/// title page takes its form from the project's frames, so the test names it on the page)
/// names the copy and the record that names it, and writes nothing.
#[test]
fn a_writer_names_the_damaged_copy_a_page_names() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let rel = crate::support::page_file(&project, 1);
    let page = std::fs::read_to_string(project.join(&rel)).unwrap();
    let named = page.replacen(
        ",\"title\":",
        ",\"form\":\"81ee0d6d-6c0e-55e4-b7fd-1896f594051f\",\"title\":",
        1,
    );
    assert_ne!(named, page);
    put_as_saved(&project, &rel, named.as_bytes());
    std::fs::write(
        project.join("lib/forms/81ee0d6d-6c0e-55e4-b7fd-1896f594051f.json"),
        "{\"$k\": \"form\",\n",
    )
    .unwrap();
    let before = tree(&project);
    let o = ew_in(&t, &["fmt".as_ref(), project.as_os_str()], &[]);
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

/// The file of the copy of the part `number` in the project `dir` (`lib/parts/<id>.json`).
fn copy_file(dir: &std::path::Path, number: &str) -> PathBuf {
    let needle = format!("\"part_number\": \"{number}\"");
    std::fs::read_dir(dir.join("lib/parts"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| std::fs::read_to_string(p).unwrap().contains(&needle))
        .unwrap_or_else(|| panic!("no copy of {number}"))
}

/// A copy of the sample in `t` and the file of its copy of the compiled-in starter part EW-GEN-MCB-4 (-101F1's).
fn starter_mcb_project(t: &Tmp) -> (PathBuf, PathBuf) {
    let project = sample_copy(t);
    let file = copy_file(&project, "EW-GEN-MCB-4");
    (project, file)
}

/// `ew update-copies` writes a damaged copy of a starter part from the compiled-in library, byte for
/// byte as the assignment's save wrote it, says so with the version written, and the project then opens for
/// editing (`ew fmt` writes nothing).
#[test]
fn update_copies_writes_a_damaged_copy_from_its_library() {
    let t = Tmp::new("ew-cli-test");
    let (project, file) = starter_mcb_project(&t);
    let original = std::fs::read(&file).unwrap();
    let version = serde_json::from_slice::<serde_json::Value>(&original).unwrap()["version"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&file, "{\"$k\": \"part\",\n").unwrap();
    let o = ew_in(&t, &["update-copies".as_ref(), project.as_os_str()], &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(0),
            format!(
                "updated the library copy of part EW-GEN-MCB-4 at version {version} from its library\nthe project \
                 opens for editing\n"
            ),
            String::new()
        )
    );
    assert!(
        std::fs::read(&file).unwrap() == original,
        "the library's copy is written"
    );
    let fmt = ew_in(
        &t,
        &["fmt".as_ref(), project.as_os_str(), "--check".as_ref()],
        &[],
    );
    assert_eq!(fmt.status.code(), Some(0), "{}", stderr(&fmt));
}

/// `ew update-copies` on a project whose damaged copy no library holds (a supplier's part the test tables
/// import into the sample, which holds starter parts only) is refused in words, exit 1, and writes nothing.
#[test]
fn update_copies_refuses_a_copy_no_library_holds() {
    let t = Tmp::new("ew-cli-test");
    let project = sample_copy(&t);
    let o = t.ew(&[
        "import".as_ref(),
        "parts".as_ref(),
        PARTS.as_ref(),
        "--project".as_ref(),
        project.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    std::fs::write(copy_file(&project, "5SY4204-7"), "{\"$k\": \"part\",\n").unwrap();
    let before = tree(&project);
    let o = ew_in(&t, &["update-copies".as_ref(), project.as_os_str()], &[]);
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
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
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
    let project = sample_copy(&t);
    let (small, big) = (png(2, 1), png(65536, 32768));
    let page = crate::support::page_file(&project, 101);
    let page = page.trim_start_matches("pages/").trim_end_matches(".jsonl");
    let place = format!(
        r#"{{"cmd":"item.place","params":{{"page":"{page}","kind":{{"image":{{"data":"{}","rect":[[0,0],[2000000,1000000]]}}}}}}}}"#,
        base64(&small)
    );
    let log = t.0.join("place.ewcmd");
    std::fs::write(
        &log,
        format!("{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"test\"}}\n{place}\n"),
    )
    .unwrap();
    let applied = t.ew(&["apply".as_ref(), project.as_os_str(), log.as_os_str()]);
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
    let o = t.ew(&["fmt".as_ref(), project.as_os_str()]);
    assert_eq!(
        (o.status.code(), stderr(&o)),
        (
            Some(0),
            format!("notice: {notice}\nfmt: every file is in canonical form; nothing was written\n")
        )
    );
}
