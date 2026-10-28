// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Session facade v0: construction, the interactive path, previews, undo and redo, the change feed,
//! the dirty flag, the registry and determinism under a seeded ID source.

use ew_api::{AnyCommand, CmdError, Origin, Session, SessionOptions, Submit};
fn model_lines(m: &ew_model::Model) -> Vec<(ew_model::AnyKey, String)> {
    ew_commands::rows::model_lines(m).expect("every record has a JSON form")
}
use ew_commands::testkit::seeded_ids;

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

fn session(seed: u64) -> Session {
    let project = serde_json::from_str(PROJECT).unwrap();
    Session::from_records(
        project,
        [],
        [],
        SessionOptions {
            ids: Some(seeded_ids(seed)),
            ..SessionOptions::default()
        },
    )
    .unwrap()
}

fn page(number: u32) -> AnyCommand {
    AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({ "number": number }),
    }
}

#[test]
fn execute_undo_redo_and_the_change_feed() {
    let mut s = session(1);
    assert_eq!(s.world().rev.0, 0);
    let r = s.execute(&page(1)).unwrap();
    assert_eq!(r.rev.0, 1);
    assert_eq!(s.world().rev.0, 1);
    assert!(s.is_dirty());
    let changes = s.drain_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].rev.0, 1);
    assert_eq!(changes[0].touched_pages.len(), 1);

    let u = s.undo(&Origin::Gui).unwrap();
    assert_eq!(u.rev.0, 2);
    assert_eq!(u.label, "page.create");
    assert!(!s.is_dirty());
    let r2 = s.redo(&Origin::Gui).unwrap();
    assert_eq!(r2.rev.0, 3);
    assert!(s.is_dirty());
    s.mark_saved();
    assert!(!s.is_dirty());
    assert_eq!(s.drain_changes().len(), 2);
    assert_eq!(s.history().entries().len(), 1);
}

/// FT-404: `Session::undo` and `Session::redo` take the caller's origin; a `project.checks.set` step is refused to the
/// command-log and script origins (nothing changes, nothing enters the change feed) and taken back by the GUI.
#[test]
fn undo_and_redo_of_a_human_only_step_need_the_gui() {
    let mut s = session(3);
    let checks = AnyCommand {
        name: "project.checks.set".into(),
        params: serde_json::json!({"release_scheme": "full"}),
    };
    s.execute_with(&checks, Submit::from(Origin::Gui)).unwrap();
    s.drain_changes();
    let rev = s.world().rev;
    for origin in [
        Origin::CmdLog {
            file: "setup.ewcmd".into(),
        },
        Origin::Script { name: "tidy".into() },
    ] {
        let err = s.undo(&origin).unwrap_err();
        assert!(
            matches!(&err, CmdError::Exposure { command, .. } if command == "project.checks.set"),
            "{origin:?}: {err:?}"
        );
        assert_eq!(s.world().rev, rev);
        assert!(s.drain_changes().is_empty());
    }
    s.undo(&Origin::Gui).unwrap();
    let undone = s.world().rev;
    let err = s.redo(&Origin::Script { name: "tidy".into() }).unwrap_err();
    assert!(matches!(err, CmdError::Exposure { .. }), "{err:?}");
    assert_eq!(s.world().rev, undone);
    s.redo(&Origin::Gui).unwrap();
    assert!(s.world().rev > undone);
}

#[test]
fn preview_commits_nothing_until_commit() {
    let mut s = session(2);
    let p = s.preview(&page(2)).unwrap();
    assert_eq!(s.world().rev.0, 0);
    assert_eq!(p.diff().len(), 1);
    assert!(s.drain_changes().is_empty());
    let r = s.commit(p).unwrap();
    assert_eq!(r.rev.0, 1);
    assert_eq!(s.drain_changes().len(), 1);
}

#[test]
fn bulk_commands_need_a_preview() {
    let mut s = session(3);
    let renumber = AnyCommand {
        name: "page.renumber".into(),
        params: serde_json::json!({ "scope": "project", "start": 1 }),
    };
    assert!(matches!(s.execute(&renumber), Err(CmdError::NeedsPreview { .. })));
}

#[test]
fn invalid_parameters_are_rejected_with_their_path() {
    let mut s = session(4);
    let bad = AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({ "number": 1, "bogus": true }),
    };
    match s.execute(&bad) {
        Err(CmdError::InvalidParams { command, .. }) => assert_eq!(command, "page.create"),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
    assert_eq!(s.world().rev.0, 0);
}

#[test]
fn the_origin_and_context_are_recorded() {
    let mut s = session(5);
    let submit = Submit::from(Origin::Cli {
        subcommand: "apply".into(),
        confirmed: false,
    });
    s.execute_with(&page(3), submit).unwrap();
    let entry = &s.history().entries()[0];
    assert_eq!(entry.origin().label(), "cli:apply");
    assert_eq!(entry.command().name, "page.create");
    assert_eq!(entry.label(), "page.create");
}

#[test]
fn the_registry_lists_every_command() {
    let s = session(6);
    let names: Vec<&str> = s.registry().names().collect();
    assert_eq!(names.len(), 55);
    assert!(names.contains(&"parts.import"));
    assert!(names.contains(&"fields.set_many"));
    assert!(names.contains(&"project.assets.purge"));
    assert!(names.contains(&"rep.exchange_symbol"));
    assert!(names.contains(&"project.drawing.set"));
    assert!(names.contains(&"connection.draw_to_ip"));
    assert!(names.contains(&"ip.arrange"));
    assert!(names.contains(&"box.reshape"));
    assert!(names.contains(&"device.place"));
    assert!(names.contains(&"revision.create"));
    assert!(names.contains(&"project.settings.set"));
    assert!(names.contains(&"project.company.set"));
    assert!(s.registry().get("symbol.place").is_some(), "aliases resolve");
}

/// `project.checks.set` takes the shipped check schemes of `ew-checks` and no other name: the command layer's
/// list (it cannot depend on `ew-checks`, doc 04 §3) equals the engine's.
#[test]
fn the_check_scheme_command_knows_the_shipped_schemes() {
    let engine: Vec<&str> = ew_api::checks::SCHEMES.iter().map(|s| s.name).collect();
    assert_eq!(engine, ew_commands::commands::SHIPPED_CHECK_SCHEMES);
}

#[test]
fn the_same_commands_and_seed_give_identical_records() {
    let mut a = session(9);
    let mut b = session(9);
    for n in [1, 2, 5] {
        a.execute(&page(n)).unwrap();
        b.execute(&page(n)).unwrap();
    }
    assert_eq!(model_lines(&a.world().model), model_lines(&b.world().model));
    let mut c = session(10);
    for n in [1, 2, 5] {
        c.execute(&page(n)).unwrap();
    }
    assert_ne!(
        model_lines(&a.world().model),
        model_lines(&c.world().model),
        "another seed mints other IDs"
    );
}
