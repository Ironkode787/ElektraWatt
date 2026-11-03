// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.17 part 5 through the `ew` binary (D17, D19; decision log ID-1279): a copy of Golden A whose page 102 holds
//! MD-6's conflict record on W20's colour, ours BU and theirs WH, set on the copy by the test (Golden A's W20 has no
//! colour). CR-1 and CR-4: `ew check --mode release` reports EW-CORE-011 once, an error that blocks the release gate;
//! CR-3: `ew merge resolve` takes theirs or ours and saves the page canonically; in a session it is one undo step.
//! Golden A is only read and copied.

use std::path::{Path, PathBuf};
use std::process::Output;

use ew_api::Conflict;

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
/// Page 102.
const PAGE: &str = "pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl";
const W20_OVERRIDES: &str = r#""overrides":{"csa":"1.5","show_marker":true}"#;
/// MD-6's conflict record, as `ew merge-driver` writes it (bin/ew/tests/it/merge_driver.rs).
const CONFLICT: &str = r#"{"$k":"conflict","id":"1c5963b7-6c34-53d6-8fa6-9d02d4629711","kind":"connection","base":{"overrides":{}},"ours":{"overrides":{"colour":["BU"]}},"theirs":{"overrides":{"colour":["WH"]}}}"#;

/// Golden A's page-102 file with W20's overrides replaced.
fn page_with(overrides: &str) -> String {
    let base = std::fs::read_to_string(Path::new(GOLDEN_A).join(PAGE)).unwrap();
    let w20 = base
        .lines()
        .find(|l| l.contains("connections|W20"))
        .expect("W20")
        .to_owned();
    base.replace(&w20, &w20.replace(W20_OVERRIDES, overrides))
}

fn colour(c: &str) -> String {
    format!(r#""overrides":{{"csa":"2.5","colour":["{c}"],"show_marker":true}}"#)
}

/// A copy of Golden A in a fresh folder whose page 102 is the merge driver's result of MD-6's conflict variant: W20
/// with csa 2.5 and BU in place, and the conflict record. Removed on drop.
struct Copy(PathBuf);

impl Copy {
    fn new() -> Copy {
        let root = ew_testdir::fresh_dir("ew-merge-resolve");
        let project = root.join("J-E-2416.ew");
        super::conventions::copy_dir(Path::new(GOLDEN_A), &project);
        std::fs::write(project.join(PAGE), format!("{}{CONFLICT}\n", page_with(&colour("BU")))).unwrap();
        Copy(root)
    }

    fn project(&self) -> PathBuf {
        self.0.join("J-E-2416.ew")
    }

    fn page(&self) -> String {
        std::fs::read_to_string(self.project().join(PAGE)).unwrap()
    }

    fn ew(&self, args: &[&str]) -> Output {
        std::process::Command::new(super::conventions::bin_exe())
            .args(args)
            .arg(self.project())
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The exit code, stdout and stderr.
fn result(o: &Output) -> (Option<i32>, String, String) {
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// CR-1, CR-4: the project reports EW-CORE-011 once in release mode, an open error, which the release gate counts as
/// a blocker (doc 15 §4.8: every open error-severity finding of the gate's report).
#[test]
fn cr1_cr4_ew_check_release_reports_ew_core_011_once_as_an_open_error() {
    let c = Copy::new();
    let o = c.ew(&["check", "--mode", "release", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&o.stdout).expect("JSON");
    let findings: Vec<(String, String, String)> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["rule"] == "EW-CORE-011")
        .map(|f| {
            let s = |k: &str| f[k].as_str().unwrap_or_default().to_owned();
            (s("severity"), s("state"), s("message"))
        })
        .collect();
    assert_eq!(
        findings,
        [(
            "error".to_owned(),
            "open".to_owned(),
            "connection a junction → -102F1:1 on folio 102: ours and theirs differ in overrides.colour".to_owned()
        )]
    );
    assert_eq!(o.status.code(), Some(1));
}

/// CR-4: the release gate of Golden A's open revision counts the EW-CORE-011 finding among its blockers.
#[test]
fn cr4_the_release_gate_is_blocked_by_ew_core_011() {
    let c = Copy::new();
    let (session, report) = ew_api::Session::open_read_only(&c.project(), "test").expect("the copy opens");
    let gate = session
        .release_gate("V2.0", &ew_api::integrity_items(&report))
        .expect("the gate runs");
    let blockers: Vec<&str> = gate
        .blockers()
        .iter()
        .map(|f| f.rule)
        .filter(|r| *r == "EW-CORE-011")
        .collect();
    assert_eq!(blockers, ["EW-CORE-011"]);
    session.close();
}

/// CR-3: theirs writes WH into W20, removes the conflict and saves page 102 canonically: Golden A's file with W20's
/// csa 2.5 and colour WH.
#[test]
fn cr3_merge_resolve_theirs_writes_wh_and_saves_the_page_canonically() {
    let c = Copy::new();
    let o = c.ew(&[
        "merge",
        "resolve",
        "--take",
        "theirs",
        "--kind",
        "connection",
        "--field",
        "overrides.colour",
    ]);
    assert_eq!(
        result(&o),
        (Some(0), "1 conflict resolved, theirs taken; saved\n".to_owned(), String::new())
    );
    assert_eq!(c.page(), page_with(&colour("WH")));
}

/// CR-3: ours keeps BU and removes the conflict.
#[test]
fn cr3_merge_resolve_ours_keeps_bu() {
    let c = Copy::new();
    let o = c.ew(&["merge", "resolve", "--take", "ours"]);
    assert_eq!(
        result(&o),
        (Some(0), "1 conflict resolved, ours taken; saved\n".to_owned(), String::new())
    );
    assert_eq!(c.page(), page_with(&colour("BU")));
}

/// A selection that matches no conflict is a usage error with its reason; nothing is written.
#[test]
fn merge_resolve_without_a_match_exits_2_with_the_reason() {
    let c = Copy::new();
    let before = c.page();
    let cases: [(&[&str], &str); 3] = [
        (&["--kind", "device"], "no conflict record of the kind `device`"),
        (&["--field", "overrides.csa"], "no conflict record on the member `overrides.csa`"),
        (
            &["--kind", "file", "--field", "overrides.colour"],
            "no conflict record of the kind `file` on the member `overrides.colour`",
        ),
    ];
    for (flags, reason) in cases {
        let mut args = vec!["merge", "resolve", "--take", "theirs"];
        args.extend(flags);
        assert_eq!(
            result(&c.ew(&args)),
            (Some(2), String::new(), format!("error: {reason}; nothing was changed\n"))
        );
    }
    assert_eq!(c.page(), before);
}

/// CR-3 in a session: `merge.resolve` is one undo step, the conflict and BU back after it.
#[test]
fn cr3_merge_resolve_in_a_session_is_one_undo_step() {
    let c = Copy::new();
    let mut store = ew_api::StoreOptions::new("test");
    store.state_dir = None;
    let (mut session, _) = ew_api::Session::open(
        &c.project(),
        store,
        ew_api::SessionOptions::default(),
        ew_api::Recovery::Keep,
    )
    .expect("the copy opens");
    let conflicts = |s: &ew_api::Session| s.query().table::<Conflict>().iter().count();
    assert_eq!(conflicts(&session), 1);
    session
        .execute(&ew_api::AnyCommand {
            name: "merge.resolve".into(),
            params: serde_json::json!({"choices": [{"conflict": "1c5963b7-6c34-53d6-8fa6-9d02d4629711", "take": "theirs"}]}),
        })
        .expect("theirs is taken");
    assert_eq!(conflicts(&session), 0);
    session.undo(&ew_api::Origin::Gui).expect("one undo step");
    assert_eq!(conflicts(&session), 1);
    assert!(session.undo(&ew_api::Origin::Gui).is_err(), "nothing more to undo");
    session.close();
}
