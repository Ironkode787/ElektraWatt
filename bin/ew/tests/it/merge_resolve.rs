// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Through the `ew` binary: a copy of the sample whose page 103 holds a conflict record on the shield wire's colour
//! (the shield of -103W1), ours BU and theirs WH, set on the copy by the test (the sample's wires have no colour).
//! `ew check --mode release` reports EW-CORE-011 once, an error that blocks the release gate; CR-3: `ew merge
//! resolve` takes theirs or ours and saves the page canonically; in a session it is one undo step.

use std::path::PathBuf;
use std::process::Output;

use ew_api::Conflict;

use crate::merge_driver::{colour_conflict, page, page_with, shield_id};
use crate::support::{EwTmp as _, Tmp};

fn colour(c: &str) -> String {
    format!(r#""overrides":{{"csa":"2.5","colour":["{c}"]}}"#)
}

/// A copy of the sample in a fresh folder whose page 103 is the merge driver's result of MD-6's conflict variant: the
/// shield wire with BU in place, and the conflict record. Removed on drop.
struct Copy(Tmp);

impl Copy {
    fn new() -> Copy {
        let root = Tmp::new("ew-merge-resolve");
        let project = root.sample("sample.ew");
        std::fs::write(
            project.join(page()),
            format!("{}{}\n", page_with(&colour("BU")), colour_conflict()),
        )
        .unwrap();
        Copy(root)
    }

    fn project(&self) -> PathBuf {
        self.0.path("sample.ew")
    }

    fn page(&self) -> String {
        std::fs::read_to_string(self.project().join(page())).unwrap()
    }

    fn ew(&self, args: &[&str]) -> Output {
        self.0.ew_command(args).arg(self.project()).output().unwrap()
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
fn ew_check_release_reports_ew_core_011_once_as_an_open_error() {
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
            "connection from -X1031:SH/b to -103M1:PE on folio 103: ours and theirs differ in overrides.colour"
                .to_owned()
        )]
    );
    assert_eq!(o.status.code(), Some(1));
}

/// CR-4: the release gate of the sample's open revision counts the EW-CORE-011 finding among its blockers.
#[test]
fn the_release_gate_is_blocked_by_ew_core_011() {
    let c = Copy::new();
    let (session, report) = ew_api::Session::open_read_only(&c.project(), "test").expect("the copy opens");
    let gate = session
        .release_gate("V1.0", &ew_api::integrity_items(&report))
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

/// CR-3: `ew merge resolve` takes theirs (WH, with `--kind` and `--field`, and with `--json`, which prints
/// `ew.merge-resolve-result/1`) or ours (BU), removes the conflict and saves page 103 canonically: the sample's file
/// with the shield wire's csa 2.5 and the colour taken.
#[test]
fn merge_resolve_takes_a_side_and_saves_the_page_canonically() {
    let cases: [(&[&str], &str, &str, &str); 3] = [
        (
            &[
                "--take",
                "theirs",
                "--kind",
                "connection",
                "--field",
                "overrides.colour",
            ],
            "1 conflict resolved, theirs taken; saved\n",
            "WH",
            "theirs",
        ),
        (
            &["--take", "ours"],
            "1 conflict resolved, ours taken; saved\n",
            "BU",
            "ours",
        ),
        (
            &["--take", "theirs", "--json"],
            "{\n  \"$schema\": \"ew.merge-resolve-result/1\",\n  \"resolved\": 1,\n  \"take\": \"theirs\"\n}\n",
            "WH",
            "theirs json",
        ),
    ];
    for (flags, said, taken, name) in cases {
        let c = Copy::new();
        let mut args = vec!["merge", "resolve"];
        args.extend(flags);
        assert_eq!(
            result(&c.ew(&args)),
            (Some(0), said.to_owned(), String::new()),
            "{name}"
        );
        assert_eq!(c.page(), page_with(&colour(taken)), "{name}");
    }
}

/// A selection that matches no conflict is a usage error with its reason; nothing is written.
#[test]
fn merge_resolve_without_a_match_exits_2_with_the_reason() {
    let c = Copy::new();
    let before = c.page();
    let cases: [(&[&str], &str); 3] = [
        (&["--kind", "device"], "no conflict record of the kind `device`"),
        (
            &["--field", "overrides.csa"],
            "no conflict record on the member `overrides.csa`",
        ),
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
            (
                Some(2),
                String::new(),
                format!("error: {reason}; nothing was changed\n")
            )
        );
    }
    assert_eq!(c.page(), before);
}

/// CR-3 in a session: `merge.resolve` is one undo step, the conflict and BU back after it.
#[test]
fn merge_resolve_in_a_session_is_one_undo_step() {
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
            params: serde_json::json!({"choices": [{"conflict": shield_id(), "take": "theirs"}]}),
        })
        .expect("theirs is taken");
    assert_eq!(conflicts(&session), 0);
    session.undo(&ew_api::Origin::Gui).expect("one undo step");
    assert_eq!(conflicts(&session), 1);
    assert!(
        session.undo(&ew_api::Origin::Gui).is_err(),
        "nothing more to undo"
    );
    session.close();
}
