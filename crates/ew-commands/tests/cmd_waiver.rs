// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Waiver recording (doc 15 §2.5, FR-CHK-009; WP-S1.16 D7, FT-494): `check.waive` and `check.unwaive`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::*;
use ew_commands::{AnyCommand, CmdError, Engine, Exposure, Origin, Submit};
use ew_ids::Id;
use ew_schema::{Kind, Waiver};
use serde_json::{Value, json};

const FP: &str = "9C2E5A0B7D14F3E8A6C1B05D2E9F41AB";

fn any(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.to_owned(),
        params,
    }
}

fn waive(params: Value) -> AnyCommand {
    any("check.waive", params)
}

fn w1() -> Value {
    json!({"rule": "EW-CORE-041", "fingerprint": FP, "reason": {"en-GB": "PE stud is part of NSYPMM107"},
           "author": "engineer", "date": "2025-04-14"})
}

fn waivers(e: &Engine) -> Vec<Waiver> {
    e.model()
        .table::<Waiver>()
        .iter()
        .map(|(_, w)| (**w).clone())
        .collect()
}

/// `check.waive` records the waiver with the parameters as given (author and date never come from a clock), the
/// fingerprint in lower case, and is one undo step; `check.unwaive` removes it, also one step.
#[test]
fn waive_records_a_waiver_and_unwaive_removes_it_each_one_undo_step() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let outcome = e.execute(&waive(w1())).expect("the waiver commits");
    let [w] = waivers(&e).try_into().expect("one waiver");
    assert_eq!(outcome.outcome.primary, Some((Kind::Waiver, w.id.cast())));
    assert_eq!(w.rule.as_str(), "EW-CORE-041");
    assert_eq!(w.target.finding.as_deref(), Some(&*FP.to_ascii_lowercase()));
    assert_eq!(
        w.reason.get(&"en-GB".parse().unwrap()),
        Some("PE stud is part of NSYPMM107")
    );
    assert_eq!(w.author.as_ref().map(|a| a.as_str()), Some("engineer"));
    assert_eq!(w.date.map(|d| d.to_string()).as_deref(), Some("2025-04-14"));
    assert!(w.extra.is_empty());

    e.execute(&any("check.unwaive", json!({"waiver": w.id.to_string()})))
        .expect("the removal commits");
    assert!(waivers(&e).is_empty());
    e.undo(&Origin::Gui).expect("undo the removal");
    assert_eq!(waivers(&e).len(), 1);
    e.undo(&Origin::Gui).expect("undo the waiver");
    assert_eq!(lines(e.model()), before);
}

/// The parameter checks: the rule ID's form, the fingerprint's 32 hex digits, a reason of at least ten characters in
/// every language; a second waiver of the same finding is rejected naming the first; an unknown waiver is invalid.
#[test]
fn waive_checks_its_parameters_and_refuses_a_second_waiver_of_one_finding() {
    let mut e = engine(golden_model());
    let cases = [
        ("rule", json!("EW-CORE-41"), "/rule"),
        ("rule", json!("CORE-041"), "/rule"),
        ("rule", json!("EW-core-041"), "/rule"),
        ("fingerprint", json!("9c2e"), "/fingerprint"),
        ("fingerprint", json!("zz2e5a0b7d14f3e8a6c1b05d2e9f41ab"), "/fingerprint"),
        ("reason", json!({"en-GB": "too short"}), "/reason"),
        (
            "reason",
            json!({"en-GB": "long enough in English", "nl-NL": "te kort"}),
            "/reason",
        ),
        ("reason", json!({}), "/reason"),
    ];
    for (member, value, path) in cases {
        let mut params = w1();
        params[member] = value.clone();
        match e.execute(&waive(params)).unwrap_err() {
            CmdError::InvalidParams { path: got, .. } => assert_eq!(got, path, "{member} = {value}"),
            other => panic!("{member} = {value}: {other:?}"),
        }
    }
    assert!(waivers(&e).is_empty());

    e.execute(&waive(w1())).unwrap();
    let first = waivers(&e)[0].id;
    let mut again = w1();
    again["fingerprint"] = json!(FP.to_ascii_lowercase());
    match e.execute(&waive(again)).unwrap_err() {
        CmdError::Rejected { reason, .. } => {
            assert!(reason.contains(&first.to_string()), "{reason}");
        }
        other => panic!("{other:?}"),
    }
    // The same fingerprint under another rule is another finding.
    let mut other = w1();
    other["rule"] = json!("EW-POT-009");
    e.execute(&waive(other)).unwrap();
    assert_eq!(waivers(&e).len(), 2);

    let missing: Id<Waiver> = Id::from_uuid(ew_ids::Uuid::from_u128(0xdead));
    match e
        .execute(&any("check.unwaive", json!({"waiver": missing.to_string()})))
        .unwrap_err()
    {
        CmdError::InvalidParams { path, .. } => assert_eq!(path, "/waiver"),
        other => panic!("{other:?}"),
    }
}

/// P5-3: both commands are `HumanOnly`: a command log, MCP and an unconfirmed CLI get `CmdError::Exposure` and nothing
/// changes; the GUI and a confirmed CLI subcommand may waive.
#[test]
fn waive_and_unwaive_are_refused_from_a_script() {
    let mut e = engine(golden_model());
    let scripted = [
        Origin::CmdLog {
            file: "replay.ewcmd".into(),
        },
        Origin::Mcp {
            client: "agent".into(),
        },
        Origin::Cli {
            subcommand: "apply".into(),
            confirmed: false,
        },
    ];
    for origin in scripted.clone() {
        let err = e
            .execute_with(&waive(w1()), Submit::from(origin.clone()))
            .unwrap_err();
        assert_eq!(
            err,
            CmdError::Exposure {
                command: "check.waive".into(),
                exposure: Exposure::HumanOnly,
                origin: origin.clone(),
            }
        );
    }
    assert!(waivers(&e).is_empty());
    e.execute_with(
        &waive(w1()),
        Submit::from(Origin::Cli {
            subcommand: "waiver".into(),
            confirmed: true,
        }),
    )
    .expect("a confirmed CLI subcommand may waive");
    let id = waivers(&e)[0].id;
    for origin in scripted {
        let err = e
            .execute_with(
                &any("check.unwaive", json!({"waiver": id.to_string()})),
                Submit::from(origin.clone()),
            )
            .unwrap_err();
        assert!(matches!(err, CmdError::Exposure { .. }), "{origin:?}: {err:?}");
    }
    assert_eq!(waivers(&e).len(), 1);
}
