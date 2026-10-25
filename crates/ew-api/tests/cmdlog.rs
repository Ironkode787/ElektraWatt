// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command logs (`.ewcmd`, doc 17 §3.5): parsing and `Session::run_script`.

mod golden_common;

use ew_api::{CmdError, CmdLog, CmdLogError, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;
use ew_derive::DeviceTag;

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
fn command_log_errors_print_the_command_error_text() {
    let error = CmdError::InvalidParams {
        command: "project.company.set".into(),
        path: "/logo".into(),
        message: "m".into(),
    };
    assert_eq!(
        CmdLogError::Failed {
            line: 3,
            error: error.clone(),
            committed: 2,
        }
        .to_string(),
        "command log, line 3: project.company.set: invalid parameters at `/logo`: m (2 earlier lines committed)"
    );
    assert_eq!(
        CmdLogError::Refused { line: 1, error }.to_string(),
        "command log, line 1: not run: project.company.set: invalid parameters at `/logo`: m"
    );
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
    s.undo(&ew_api::Origin::Gui).unwrap();
    s.undo(&ew_api::Origin::Gui).unwrap();
    s.undo(&ew_api::Origin::Gui).unwrap();
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
        assert_eq!(err.exit_code(), 2, "{reference}");
    }
    assert_eq!(s.world().rev.0, 0);
}

#[test]
fn a_missing_id_reference_stops_with_exit_2_and_counts_the_committed_lines() {
    let mut s = session();
    let missing = "0192f0a1-0000-7000-8000-000000000001";
    let line = format!(
        r#"{{"cmd":"fields.set","params":{{"kind":"page","targets":[{{"ref":"id:{missing}"}}],"path":"title","value":{{"en":"T"}}}}}}"#
    );
    let err = s.run_script(&log(&[&page(1), &line]), "x.ewcmd").unwrap_err();
    match &err {
        CmdLogError::Reference {
            line,
            message,
            committed,
        } => {
            assert_eq!((*line, *committed), (3, 1));
            assert!(message.contains(missing), "{message}");
        }
        other => panic!("expected a reference error, got {other:?}"),
    }
    assert_eq!(err.exit_code(), 2);
    assert_eq!(err.committed(), 1);
    assert_eq!(s.history().entries().len(), 1, "line 2 stays committed");

    // A malformed UUID is a reference error too, and an existing record's ID resolves.
    let err = s
        .run_script(
            &log(&[r#"{"cmd":"page.create","params":{"number":2,"section":{"ref":"id:nope"}}}"#]),
            "x.ewcmd",
        )
        .unwrap_err();
    assert!(
        matches!(err, CmdLogError::Reference { committed: 0, .. }),
        "{err:?}"
    );
    let page_id = s.query().pages().iter().map(|(id, _)| id).next().expect("page 1");
    let ok = format!(
        r#"{{"cmd":"fields.set","params":{{"kind":"page","targets":[{{"ref":"id:{page_id}"}}],"path":"title","value":{{"en":"T"}}}}}}"#
    );
    assert_eq!(s.run_script(&log(&[&ok]), "x.ewcmd").unwrap().len(), 1);
}

#[test]
fn exit_codes_follow_doc_17() {
    let mut s = session();
    let refused = s
        .run_script(&log(&[r#"{"cmd":"no.such","params":{}}"#]), "x.ewcmd")
        .unwrap_err();
    assert_eq!((refused.exit_code(), refused.committed()), (2, 0));
    // A domain refusal (a second page 1) is exit 1.
    let failed = s.run_script(&log(&[&page(1), &page(1)]), "x.ewcmd").unwrap_err();
    assert!(
        matches!(
            &failed,
            CmdLogError::Failed {
                error: CmdError::Prevented { .. } | CmdError::Rejected { .. },
                ..
            }
        ),
        "{failed:?}"
    );
    assert_eq!((failed.exit_code(), failed.committed()), (1, 1));
    let malformed = CmdLog::parse("{\"cmd\":\"page.create\"}\n").unwrap_err();
    assert_eq!(malformed.exit_code(), 2);
}

#[test]
fn usage_and_internal_errors_of_a_line_exit_2() {
    let mut s = session();
    // Invalid parameters: a usage error.
    let invalid = s
        .run_script(
            &log(&[&page(5), r#"{"cmd":"page.create","params":{"number":"seven"}}"#]),
            "x.ewcmd",
        )
        .unwrap_err();
    assert!(
        matches!(
            &invalid,
            CmdLogError::Failed {
                error: CmdError::InvalidParams { .. },
                committed: 1,
                ..
            }
        ),
        "{invalid:?}"
    );
    assert_eq!(invalid.exit_code(), 2);
    // A target that does not exist, named without a reference: a usage error too.
    let missing = s
        .run_script(
            &log(&[
                r#"{"cmd":"fields.set","params":{"kind":"page","targets":["0192f0a1-0000-7000-8000-000000000002"],"path":"title","value":{"en":"T"}}}"#,
            ]),
            "x.ewcmd",
        )
        .unwrap_err();
    assert!(
        matches!(
            &missing,
            CmdLogError::Failed {
                error: CmdError::NotFound { .. },
                ..
            }
        ),
        "{missing:?}"
    );
    assert_eq!(missing.exit_code(), 2);
}

/// The printed short tag of the device a `tag:` line targets, after the line ran.
fn short_tag(s: &Session, id: ew_derive::ew_ids::Id<ew_schema::Device>) -> String {
    s.query()
        .get::<DeviceTag>(id)
        .as_ref()
        .as_ref()
        .expect("a tag")
        .short()
}

#[test]
fn tag_and_folio_references_name_exactly_one_device_or_drawn_page() {
    let b = golden_common::batch();
    let mut s = golden_common::session(&b);
    let set = |target: &str, kind: &str, path: &str, value: &str| {
        format!(
            r#"{{"cmd":"fields.set","params":{{"kind":"{kind}","targets":[{{"ref":"{target}"}}],"path":"{path}","value":{value}}}}}"#
        )
    };
    // A short tag (frozen with hidden sign, product, terminal) and a full designation resolve.
    let l = log(&[
        &set("tag:S51", "device", "function_text", r#"{"en":"a"}"#),
        &set("tag:-X1031", "device", "function_text", r#"{"en":"b"}"#),
        &set("tag:-X1031:12", "device", "function_text", r#"{"en":"c"}"#),
        &set(
            "tag:=P01+J-E-2416-102F1",
            "device",
            "function_text",
            r#"{"en":"d"}"#,
        ),
        &set("folio:902", "page", "title", r#"{"en":"Layout"}"#),
    ]);
    assert_eq!(s.run_script(&l, "x.ewcmd").unwrap().len(), 5);
    let q = s.query();
    let en = |t: &ew_mltext::MlText| {
        serde_json::to_value(t).unwrap()["en"]
            .as_str()
            .unwrap_or("")
            .to_owned()
    };
    let texts: Vec<(String, String)> = q
        .devices()
        .iter()
        .filter(|(_, d)| ["a", "b", "c", "d"].contains(&en(&d.function_text).as_str()))
        .map(|(id, d)| (short_tag(&s, id), en(&d.function_text)))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(
        texts,
        [
            ("-102F1".to_owned(), "d".to_owned()),
            ("-X1031".to_owned(), "b".to_owned()),
            ("-X1031:12".to_owned(), "c".to_owned()),
            ("S51".to_owned(), "a".to_owned()),
        ]
    );
    let titled: Vec<u32> = q
        .pages()
        .iter()
        .filter(|(_, p)| en(&p.title) == "Layout")
        .map(|(_, p)| p.folio.number)
        .collect();
    assert_eq!(titled, [902]);
    drop(q);

    // A generated folio (report 903) and an unknown tag do not resolve.
    for (reference, needle) in [("tag:-999Z9", "no device"), ("folio:903", "no drawn page")] {
        let kind = if reference.starts_with("folio") {
            "page"
        } else {
            "device"
        };
        let path = if kind == "page" { "title" } else { "function_text" };
        let err = s
            .run_script(&log(&[&set(reference, kind, path, r#"{"en":"x"}"#)]), "x.ewcmd")
            .unwrap_err();
        match &err {
            CmdLogError::Reference { message, .. } => {
                assert!(message.contains(needle), "{reference}: {message}")
            }
            other => panic!("{reference}: expected a reference error, got {other:?}"),
        }
        assert_eq!(err.exit_code(), 2);
    }

    // An ambiguous tag does not resolve either: once S51 is frozen to "102F1", two devices print -102F1.
    // The first line commits; the second stops the replay.
    let l = log(&[
        &set("tag:S51", "device", "tag.frozen", r#"{"text":"102F1"}"#),
        &set("tag:-102F1", "device", "function_text", r#"{"en":"x"}"#),
    ]);
    let err = s.run_script(&l, "x.ewcmd").unwrap_err();
    match &err {
        CmdLogError::Reference {
            line,
            message,
            committed,
        } => {
            assert_eq!((*line, *committed), (3, 1));
            assert!(message.contains("2 matches"), "{message}");
        }
        other => panic!("expected a reference error, got {other:?}"),
    }
    assert_eq!(err.exit_code(), 2);
}

// WP-S2.05 part 2
/// A command whose cost depends on its parameters (`fields.set_many` above 500 targets) runs from a command log as
/// preview and commit, one undo step, like a command that is bulk by its metadata.
#[test]
fn a_set_many_above_500_targets_runs_from_a_log_as_one_step() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    let mut s = Session::open_read_only(&dir, "ElektraWatt test").unwrap().0;
    let q = s.query();
    let devices: Vec<String> = q.devices().ids().map(|d| d.as_uuid().to_string()).collect();
    let reps: Vec<String> = q.reps().ids().map(|r| r.as_uuid().to_string()).collect();
    let functions: Vec<String> = q.functions().ids().map(|f| f.as_uuid().to_string()).collect();
    let connections: Vec<String> = q.connections().ids().map(|c| c.as_uuid().to_string()).collect();
    let pages: Vec<String> = q.pages().ids().map(|p| p.as_uuid().to_string()).collect();
    let nodes: Vec<String> = q
        .table::<ew_schema::StructureNode>()
        .ids()
        .map(|n| n.as_uuid().to_string())
        .collect();
    drop(q);
    let total = devices.len() + reps.len() + functions.len() + connections.len() + pages.len() + nodes.len();
    assert!(total > 500, "{total}");
    let edits = serde_json::json!({"edits": [
        {"kind": "device", "targets": devices, "path": "process_tag", "value": "P-1"},
        {"kind": "function", "targets": functions, "path": "cp_overrides", "value": null},
        {"kind": "connection", "targets": connections, "path": "overrides.show_marker", "value": null},
        {"kind": "page", "targets": pages, "path": "description_override", "value": "Bulk"},
        {"kind": "structure_node", "targets": nodes, "path": "description", "value": "Bulk"},
        {"kind": "representation", "targets": reps, "path": "arrangement", "value": null}]});
    let line = serde_json::json!({"cmd": "fields.set_many", "params": edits}).to_string();
    let before = s.history().entries().len();
    let receipts = s
        .run_script(&log(&[&line]), "big.ewcmd")
        .expect("the line commits");
    assert_eq!(receipts.len(), 1);
    assert_eq!(s.history().entries().len(), before + 1, "one undo step");
}
