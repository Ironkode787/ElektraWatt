// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The commit pipeline end to end with `page.create`: execute, preview and commit, the rebase check, the
//! change feed, the exposure check and the normaliser order.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::CreatePage;
use ew_commands::normalise::{ORDER, registry};
use ew_commands::{AnyCommand, CmdError, Command, Cost, Exposure, Origin, Submit};
use ew_model::Entity;
use ew_schema::{Kind, Page};

fn create_after(after: ew_ids::Id<Page>) -> CreatePage {
    let mut c = CreatePage::at(None, 0);
    c.number = None;
    c.after = Some(after);
    c
}

#[test]
fn normaliser_order_is_the_registry_order() {
    let ids: Vec<&str> = registry().iter().map(|n| n.id()).collect();
    assert_eq!(ids, ORDER);
}

#[test]
fn execute_commits_one_revision_and_notifies() {
    let mut e = engine(golden_model());
    let r = e.execute(&create_after(PAGE_102)).expect("page.create");
    assert_eq!(r.rev.0, 1);
    let key = r.outcome.primary.expect("primary");
    assert_eq!(key.0, Kind::Page);
    assert_eq!(r.created, vec![key]);
    let page = e.model().table::<Page>().get(&typed(key)).expect("page").clone();
    assert_eq!(page.folio.number, 103, "after 102 in 100-199 the proposal is 103");
    let changes = e.drain_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].touched_keys, vec![key]);
    assert_eq!(changes[0].touched_pages, vec![typed::<Page>(key)]);
    assert!(e.drain_changes().is_empty());
    assert!(e.is_dirty());
}

#[test]
fn folio_proposal_takes_a_sub_page_when_the_next_number_is_used() {
    let mut e = engine(golden_model());
    let r = e.execute(&create_after(PAGE_101)).expect("page.create");
    let page = e
        .model()
        .table::<Page>()
        .get(&typed(r.outcome.primary.unwrap()))
        .unwrap()
        .clone();
    assert_eq!(page.folio.number, 101);
    assert_eq!(page.folio.label.as_ref().map(|l| l.as_str()), Some("1"));
}

#[test]
fn undo_and_redo_are_new_revisions_and_restore_bytes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&create_after(PAGE_102)).unwrap();
    let after = lines(e.model());
    let u = e.undo(&Origin::Gui).unwrap();
    assert_eq!(u.rev.0, 2);
    assert_eq!(lines(e.model()), before);
    assert!(!e.is_dirty());
    let r = e.redo(&Origin::Gui).unwrap();
    assert_eq!(r.rev.0, 3);
    assert_eq!(lines(e.model()), after);
    assert!(e.undo(&Origin::Gui).is_ok());
    assert!(matches!(e.undo(&Origin::Gui), Err(CmdError::History { .. })));
}

#[test]
fn preview_then_commit_equals_execute() {
    let mut a = engine(golden_model());
    a.execute(&create_after(PAGE_102)).unwrap();
    let mut b = engine(golden_model());
    let p = b.preview(&create_after(PAGE_102)).unwrap();
    assert_eq!(p.diff().len(), 1);
    assert_eq!(b.model().rev().0, 0, "a preview commits nothing");
    b.commit(p).unwrap();
    assert_eq!(lines(a.model()), lines(b.model()));
}

#[test]
fn commit_after_an_unrelated_change_applies_the_prepared_ops() {
    let mut e = engine(golden_model());
    let p = e.preview(&create_after(PAGE_102)).unwrap();
    let prepared_key = p.ops()[0].key();
    // A change the preview did not read: a page in no section.
    e.execute(&CreatePage::at(None, 1)).unwrap();
    let r = e.commit(p).unwrap();
    assert_eq!(r.rev.0, 2);
    assert!(
        e.model().row(&prepared_key).is_some(),
        "the prepared page was applied as is"
    );
}

#[test]
fn commit_after_a_conflicting_change_reruns_the_command() {
    let mut e = engine(golden_model());
    let p = e.preview(&create_after(PAGE_102)).unwrap();
    let prepared_key = p.ops()[0].key();
    // Folio 103 is taken meanwhile: the proposal must move to a sub-page.
    e.execute(&CreatePage::at(Some(SEC1), 103)).unwrap();
    let r = e.commit(p).unwrap();
    let key = r.outcome.primary.unwrap();
    assert_ne!(key, prepared_key, "re-run on the current snapshot");
    let page = e.model().table::<Page>().get(&typed(key)).unwrap().clone();
    assert_eq!(
        (page.folio.number, page.folio.label.as_ref().map(|l| l.as_str())),
        (102, Some("1"))
    );
}

#[test]
fn any_command_runs_through_the_registry() {
    let mut e = engine(golden_model());
    let cmd = AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({ "after": PAGE_102.to_string() }),
    };
    let r = e.execute(&cmd).unwrap();
    assert_eq!(r.outcome.primary.unwrap().0, Page::KIND);
    let unknown = AnyCommand {
        name: "page.nope".into(),
        params: serde_json::json!({}),
    };
    assert!(matches!(
        e.execute(&unknown),
        Err(CmdError::UnknownCommand { .. })
    ));
}

#[test]
fn rejected_commands_change_nothing() {
    let mut e = engine(golden_model());
    let err = e.execute(&CreatePage::at(Some(SEC1), 101)).unwrap_err();
    assert!(matches!(err, CmdError::Rejected { .. }), "{err:?}");
    assert_eq!(e.model().rev().0, 0);
    assert!(e.history().entries().is_empty());
}

#[test]
fn every_v0_command_is_an_edit_command_but_the_check_schemes() {
    let r = ew_commands::CommandRegistry::builtin();
    assert_eq!(r.names().count(), 53);
    for c in r.commands() {
        // ADR-0008 amendment 3: the release scheme decides a check outcome, so its command is `HumanOnly`; the asset
        // purge deletes files that do not come back (owner ruling, 2026-10-23).
        let expected = if matches!(c.meta().name, "project.checks.set" | "project.assets.purge") {
            Exposure::HumanOnly
        } else {
            Exposure::Edit
        };
        assert_eq!(c.meta().exposure, expected, "{}", c.meta().name);
        // ADR-0008 amendment 1: the asset purge deletes files, a non-undoable marker line (FT-380).
        assert_eq!(
            c.meta().undoable,
            c.meta().name != "project.assets.purge",
            "{}",
            c.meta().name
        );
    }
    assert_eq!(CreatePage::meta().cost, Cost::Interactive);
    assert!(Exposure::HumanOnly.allows(&Origin::Gui));
    assert!(!Exposure::HumanOnly.allows(&Origin::Mcp { client: "x".into() }));
    // D7: a CLI submission of a human-only command needs the terminal confirmation.
    let cli = |confirmed| Origin::Cli {
        subcommand: "purge".into(),
        confirmed,
    };
    assert!(Exposure::HumanOnly.allows(&cli(true)));
    assert!(!Exposure::HumanOnly.allows(&cli(false)));
    assert!(Exposure::Import.allows(&cli(false)));
    assert!(Exposure::Edit.allows(&cli(false)));
    let json = serde_json::to_value(cli(false)).expect("serialisable");
    let back: Origin = serde_json::from_value(serde_json::json!({"cli": {"subcommand": "purge"}}))
        .expect("`confirmed` defaults to false");
    assert_eq!(back, cli(false));
    assert_eq!(
        json,
        serde_json::json!({"cli": {"subcommand": "purge", "confirmed": false}})
    );
    let _ = Submit::from(Origin::Gui);
}

/// FT-404: the strictness order of a coalesced step and who may undo or redo a step of each class.
#[test]
fn exposure_strictness_and_the_history_rule() {
    use Exposure::{Edit, HumanOnly, Import, Internal};
    let order = [Edit, Import, HumanOnly, Internal];
    for (i, a) in order.iter().enumerate() {
        for (j, b) in order.iter().enumerate() {
            assert_eq!(a.strictest(*b), order[i.max(j)], "{a:?} {b:?}");
        }
    }
    let cli = |confirmed| Origin::Cli {
        subcommand: "apply".into(),
        confirmed,
    };
    let scripted = [
        cli(false),
        Origin::CmdLog {
            file: "a.ewcmd".into(),
        },
        Origin::Script { name: "s".into() },
        Origin::Mcp { client: "c".into() },
        Origin::Plugin { id: "p".into() },
        Origin::Python {
            script: "p.py".into(),
        },
        Origin::Import { lineage: "l".into() },
        Origin::Merge,
        Origin::External,
    ];
    for origin in [Origin::Gui, cli(true)] {
        for class in order {
            assert!(class.allows_history(&origin), "{class:?} {origin:?}");
        }
    }
    for origin in &scripted {
        assert!(Edit.allows_history(origin), "{origin:?}");
        assert!(Import.allows_history(origin), "{origin:?}");
        assert!(!HumanOnly.allows_history(origin), "{origin:?}");
        assert!(!Internal.allows_history(origin), "{origin:?}");
    }
}
