// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command logs (`.ewcmd`, doc 17 §3.5): parsing and `Session::run_script`.

use ew_api::{CmdError, CmdLog, CmdLogError, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;
const HEADER: &str = r#"{"$schema":"ew.cmdlog/1","label":"test log"}"#;

fn session() -> Session {
    let project = serde_json::from_str(PROJECT).unwrap();
    Session::from_records(
        project,
        [],
        [],
        SessionOptions {
            ids: Some(seeded_ids(7)),
            ..SessionOptions::default()
        },
    )
    .unwrap()
}

fn log(lines: &[&str]) -> CmdLog {
    let mut text = String::from(HEADER);
    for l in lines {
        text.push('\n');
        text.push_str(l);
    }
    text.push('\n');
    CmdLog::parse(&text).unwrap()
}

fn page(number: u32) -> String {
    format!(r#"{{"cmd":"page.create","params":{{"number":{number}}}}}"#)
}

fn malformed(text: &str) -> (usize, String) {
    match CmdLog::parse(text) {
        Err(CmdLogError::Malformed { line, message }) => (line, message),
        other => panic!("expected a malformed log, got {other:?}"),
    }
}

#[test]
fn a_log_has_a_header_and_one_command_per_line() {
    let l = log(&[
        &page(1),
        r#"{"cmd":"page.renumber","params":{"scope":"project","start":10}}"#,
    ]);
    assert_eq!(l.label(), Some("test log"));
    assert_eq!(l.len(), 2);
    assert!(!l.is_empty());
    let names: Vec<&str> = l.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["page.create", "page.renumber"]);
    // `params` may be absent; empty lines are ignored; a header without a label is fine.
    let bare = CmdLog::parse("{\"$schema\":\"ew.cmdlog/1\"}\n\n{\"cmd\":\"page.create\"}\n").unwrap();
    assert_eq!(bare.label(), None);
    assert_eq!(bare.commands().next().unwrap().params, serde_json::json!({}));
}

#[test]
fn a_malformed_log_names_its_line() {
    assert_eq!(malformed("").0, 1);
    assert_eq!(
        malformed(r#"{"cmd":"page.create"}"#).0,
        1,
        "the first line must be the header"
    );
    assert_eq!(
        malformed("{\"$schema\":\"ew.cmdlog/2\"}").0,
        1,
        "a foreign schema"
    );
    assert_eq!(malformed(&format!("{HEADER}\nnot json")).0, 2);
    assert_eq!(malformed(&format!("{HEADER}\n[1]")).0, 2);
    assert_eq!(
        malformed(&format!("{HEADER}\n{{\"params\":{{}}}}")).0,
        2,
        "no cmd"
    );
    assert_eq!(
        malformed(&format!("{HEADER}\n{{\"cmd\":\"x\",\"params\":3}}")).0,
        2
    );
    let (line, message) = malformed(&format!("{HEADER}\n{}\n{{\"cmd\":\"x\",\"extra\":1}}", page(1)));
    assert_eq!(line, 3);
    assert!(message.contains("extra"), "{message}");
}

#[test]
fn every_line_is_one_transaction_with_the_command_log_origin() {
    let mut s = session();
    let receipts = s
        .run_script(
            &log(&[
                &page(1),
                &page(2),
                // A bulk command goes through preview and commit.
                r#"{"cmd":"page.renumber","params":{"scope":"project","start":10}}"#,
            ]),
            "variants/x.ewcmd",
        )
        .unwrap();
    assert_eq!(receipts.len(), 3);
    assert_eq!(s.history().entries().len(), 3);
    for entry in s.history().entries() {
        assert_eq!(entry.origin().label(), "cmdlog:variants/x.ewcmd");
    }
    s.undo().unwrap();
    s.undo().unwrap();
    s.undo().unwrap();
    assert_eq!(s.world().rev.0, 6, "one undo per line, each a new revision");
}

#[test]
fn an_unknown_command_refuses_the_whole_log_before_its_first_line() {
    let mut s = session();
    let err = s
        .run_script(&log(&[&page(1), r#"{"cmd":"no.such","params":{}}"#]), "x.ewcmd")
        .unwrap_err();
    assert!(
        matches!(
            &err,
            CmdLogError::Refused {
                line: 3,
                error: CmdError::UnknownCommand { .. }
            }
        ),
        "{err:?}"
    );
    assert_eq!(s.world().rev.0, 0, "nothing ran");
    assert!(err.to_string().contains("line 3"));
}

#[test]
fn a_failing_line_stops_the_replay_and_keeps_the_earlier_lines() {
    let mut s = session();
    let err = s
        .run_script(&log(&[&page(1), &page(1), &page(2)]), "x.ewcmd")
        .unwrap_err();
    match err {
        CmdLogError::Failed { line, committed, .. } => assert_eq!((line, committed), (3, 1)),
        other => panic!("{other:?}"),
    }
    assert_eq!(s.history().entries().len(), 1);
}

#[test]
fn references_resolve_just_before_their_line_runs() {
    let mut s = session();
    let l = log(&[
        r#"{"cmd":"section.create","params":{"number":1,"folio_range":[100,199],"title":{"en":"S"}}}"#,
        r#"{"cmd":"page.create","params":{"section":{"ref":"section:1"},"number":101}}"#,
        r#"{"cmd":"page.create","params":{"section":{"ref":"section:1"},"number":105}}"#,
        r#"{"cmd":"page.renumber","params":{"scope":{"section":{"ref":"section:1"}},"start":101,"step":1}}"#,
    ]);
    assert_eq!(s.run_script(&l, "x.ewcmd").unwrap().len(), 4);
    let q = s.query();
    let mut numbers: Vec<u32> = q.pages().iter().map(|(_, p)| p.folio.number).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, [101, 102], "105 closed up to 102");
}

#[test]
fn an_unresolvable_reference_stops_the_replay() {
    let mut s = session();
    for reference in ["section:7", "section:x", "tag:-101F1", "folio:101", "nonsense"] {
        let line =
            format!(r#"{{"cmd":"page.create","params":{{"section":{{"ref":"{reference}"}},"number":1}}}}"#);
        let err = s.run_script(&log(&[&line]), "x.ewcmd").unwrap_err();
        assert!(
            matches!(err, CmdLogError::Reference { line: 2, .. }),
            "{reference}: {err:?}"
        );
    }
    assert_eq!(s.world().rev.0, 0);
}
