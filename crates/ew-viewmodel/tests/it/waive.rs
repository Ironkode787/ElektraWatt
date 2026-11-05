// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-674: Waive… and Unwaive in the Messages pane on copies of Golden A (the reason prompt, its ten-character rule,
//! the author preference, the date the window passes, one `check.waive` or `check.unwaive` per step, the check toast's
//! waived count). FT-675: the canvas's conflict overlay of an unbonded merged 0 V/PE potential (Golden A without its
//! bond B1), which never reaches the page's display list.

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::navigators::{NavKind, NavNode, NavRole};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use serde_json::json;

/// The date the window would pass (a fixed one: the shell never reads a clock).
const DATE: &str = "2026-10-30";

/// The finding row of rule `rule` whose subject ends with `subject`.
fn finding(shell: &ShellModel, rule: &str, subject: &str) -> NavNode {
    let t = shell
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .cloned()
        .expect("the Messages table");
    let rows: Vec<NavNode> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding && n.cells[1] == rule && n.cells[3].ends_with(subject))
        .cloned()
        .collect();
    assert_eq!(rows.len(), 1, "one {rule} finding on {subject}");
    rows.into_iter().next().unwrap()
}

/// The state cell (`open`, `waived`) of the row with key `key`.
fn state(shell: &ShellModel, key: &str) -> String {
    let t = shell.nav(NavKind::Messages).and_then(|n| n.table()).unwrap();
    t.nodes
        .iter()
        .find(|n| n.key == key)
        .map(|n| n.cells[6].clone())
        .unwrap_or_else(|| panic!("no row {key}"))
}

fn waived(shell: &ShellModel) -> usize {
    shell.messages_counts().expect("the counts").waived
}

fn failed(shell: &mut ShellModel, action: &str, params: serde_json::Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

/// The reason, author and date of the waiver the last committed step recorded.
fn last_waive(shell: &ShellModel) -> serde_json::Value {
    let last = shell.recorded_commands().last().expect("a committed step");
    assert_eq!(last.cmd, "check.waive");
    last.params.clone()
}

#[test]
fn waive_from_the_prompt_commits_one_check_waive_and_unwaive_opens_the_finding_again() {
    let tmp = Tmp::new("waive-prompt");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    done(&mut shell, "messages.show", json!({"on": true}));
    let row = finding(&shell, "EW-CORE-033", "-3W1");
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": row.key}),
    );
    let detail = shell.messages_detail().expect("the detail");
    assert!(!detail.waived && detail.waiver.is_none());
    assert_eq!(detail.fingerprint.len(), 32);
    let before = waived(&shell);
    let rev = shell.revision();

    // Waive… opens the prompt: Reason empty, Author from the preference (none yet).
    done(&mut shell, "messages.waive", json!({"date": DATE}));
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!("the Waive prompt: {:?}", shell.dialog());
    };
    assert_eq!(form.action, "messages.waive");
    let fields: Vec<(&str, &str)> = form.fields.iter().map(|f| (f.name, f.value.as_str())).collect();
    assert_eq!(fields, [("reason", ""), ("author", "")]);

    // A nine-character reason is refused in the prompt; nothing is committed.
    shell.set_form_field("reason", "  too short ");
    shell.set_form_field("author", "A. Engineer");
    let out = shell.submit_form();
    let Outcome::Failed { message } = out else {
        panic!("{out:?}");
    };
    assert!(message.contains("at least 10 characters"), "{message}");
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!("the prompt stays");
    };
    assert_eq!(form.error.as_deref(), Some(message.as_str()));
    assert_eq!(shell.revision(), rev, "no commit");

    // A valid reason commits one `check.waive` with the reason, the author and the date.
    shell.set_form_field("reason", "Cable length is set on site");
    assert_eq!(shell.submit_form(), Outcome::Done);
    assert!(shell.dialog().is_none(), "the prompt closes");
    assert_eq!(shell.revision(), rev.map(|r| r + 1), "one undo step");
    let params = last_waive(&shell);
    assert_eq!(params["rule"], "EW-CORE-033");
    assert_eq!(params["fingerprint"], detail.fingerprint.as_str());
    assert_eq!(params["reason"], "Cable length is set on site");
    assert_eq!(params["author"], "A. Engineer");
    assert_eq!(params["date"], DATE);
    assert_eq!(shell.settings().author_name.as_deref(), Some("A. Engineer"));
    assert_eq!(state(&shell, &row.key), "waived");
    assert_eq!(waived(&shell), before + 1);
    let detail = shell.messages_detail().expect("the detail");
    assert!(detail.waived);
    assert!(detail.waiver.is_some(), "the waiver's ID for Unwaive");
    assert!(shell.describe().lines().any(|l| l == "  detail waived"));

    // The check toast's waived count follows (in its title, FT-664).
    done(&mut shell, "messages.check", json!({}));
    let toast = shell.toast().expect("the check toast").text.clone();
    let expect = if before + 1 == 1 {
        "1 waived".to_owned()
    } else {
        format!("{} waived", before + 1)
    };
    assert!(
        toast.starts_with("Check · ") && toast.contains(&expect),
        "{toast}"
    );

    // Undo takes the waiver back in one step; redo restores it.
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(state(&shell, &row.key), "open");
    done(&mut shell, "edit.redo", json!({}));
    assert_eq!(state(&shell, &row.key), "waived");

    // Unwaive commits one `check.unwaive`: the finding is open again.
    let rev = shell.revision();
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": row.key}),
    );
    done(&mut shell, "messages.unwaive", json!({}));
    assert_eq!(shell.revision(), rev.map(|r| r + 1));
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "check.unwaive");
    assert_eq!(state(&shell, &row.key), "open");
    assert_eq!(waived(&shell), before);
    done(&mut shell, "messages.check", json!({}));
    let toast = shell.toast().expect("the check toast").text.clone();
    assert_eq!(toast.starts_with("Check · "), before > 0, "{toast}");

    // The prompt opens again with the author the preference kept.
    done(&mut shell, "messages.waive", json!({"date": DATE}));
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!("the Waive prompt");
    };
    assert_eq!(form.fields[1].value, "A. Engineer");
    done(&mut shell, "ui.dismiss", json!({}));
    shell.shutdown();
}

#[test]
fn a_short_reason_an_open_or_waived_mismatch_and_headless_are_refused_without_a_commit() {
    let tmp = Tmp::new("waive-refusals");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    done(&mut shell, "messages.show", json!({"on": true}));
    let row = finding(&shell, "EW-CORE-033", "-3W1");
    let rev = shell.revision();
    // Nine characters after trimming, given as parameters (no prompt).
    let message = failed(
        &mut shell,
        "messages.waive",
        json!({"key": row.key, "reason": " 123456789 ", "author": "A", "date": DATE}),
    );
    assert!(message.contains("at least 10 characters"), "{message}");
    // No author, no date.
    let reason = "Cable length is set on site";
    let message = failed(
        &mut shell,
        "messages.waive",
        json!({"key": row.key, "reason": reason, "date": DATE}),
    );
    assert!(message.contains("author"), "{message}");
    let message = failed(
        &mut shell,
        "messages.waive",
        json!({"key": row.key, "reason": reason, "author": "A"}),
    );
    assert!(message.contains("date"), "{message}");
    assert_eq!(shell.revision(), rev, "nothing committed");
    // Unwaive on an open finding, Waive on a waived one.
    assert_eq!(
        refused(&mut shell, "messages.unwaive", json!({"key": row.key})),
        "reason-not-waived"
    );
    done(
        &mut shell,
        "messages.waive",
        json!({"key": row.key, "reason": reason, "author": "A", "date": DATE}),
    );
    assert_eq!(
        refused(
            &mut shell,
            "messages.waive",
            json!({"key": row.key, "reason": reason, "author": "A", "date": DATE})
        ),
        "reason-not-open-finding"
    );
    shell.shutdown();

    // Headless: `check.waive` is `HumanOnly`.
    let tmp = Tmp::new("waive-headless");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut shell, "messages.show", json!({"on": true}));
    let row = finding(&shell, "EW-CORE-033", "-3W1");
    assert_eq!(
        refused(
            &mut shell,
            "messages.waive",
            json!({"key": row.key, "reason": reason, "author": "A", "date": DATE})
        ),
        "reason-human-only"
    );
    shell.shutdown();
}

/// A writable window copy of Golden A on folio 103 with the bond B1 deleted: 0 V and PE merge over W27 and EW-POT-001
/// reports (as in `tests/bond.rs`).
fn without_b1(tmp: &Tmp) -> ShellModel {
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    let bond = {
        let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
        let rows: Vec<_> = t.nodes.iter().filter(|n| n.kinds == ["bond"]).collect();
        assert_eq!(rows.len(), 1, "Golden A has the one bond B1");
        rows[0].records[0]
    };
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "bond", "id": bond.to_string()}]}}),
    );
    shell
}

/// FT-675: while EW-POT-001 stands, the wires of the merged potential on 103 are in the conflict overlay; the page's
/// display list (what the canvas renders, prints and exports) is the same with the finding open and waived; once the
/// bond is declared the overlay is empty. Golden A itself has no overlay.
#[test]
fn the_conflict_overlay_lists_the_merged_potentials_wires_until_the_bond_is_declared() {
    let tmp = Tmp::new("conflict-golden");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    assert!(shell.conflict_members().is_empty(), "Golden A has its bond");
    assert!(shell.conflict_overlay(0).is_empty());
    shell.shutdown();

    let tmp = Tmp::new("conflict-overlay");
    let mut shell = without_b1(&tmp);
    let members = shell.conflict_members();
    assert!(!members.is_empty(), "the merged potential's wires");
    let overlay = shell.conflict_overlay(0);
    assert!(!overlay.is_empty(), "W27 and the 0 V/PE wires on 103");
    assert!(overlay.iter().all(|line| line.len() >= 2));
    let page = shell.page_content("103").expect("the page's display list");

    // Waived, the finding no longer stands: no overlay, and the page's display list is unchanged.
    done(&mut shell, "messages.show", json!({"on": true}));
    let conflict = {
        let t = shell.nav(NavKind::Messages).and_then(|n| n.table()).unwrap();
        t.nodes
            .iter()
            .find(|n| n.role == NavRole::Finding && n.cells[1] == "EW-POT-001" && n.cells[6] == "open")
            .cloned()
            .expect("the open EW-POT-001 finding")
    };
    done(
        &mut shell,
        "messages.waive",
        json!({"key": conflict.key, "reason": "Bond is declared on the panel drawing", "author": "A",
            "date": DATE}),
    );
    assert!(
        shell.conflict_overlay(0).is_empty(),
        "a waived finding does not stand"
    );
    assert_eq!(
        shell.page_content("103").expect("the page").pages,
        page.pages,
        "the overlay never reaches the page's display list"
    );
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(shell.conflict_overlay(0), overlay, "open again");

    // The bond declared on W27 (the finding's fix): the PE colour returns, no overlay.
    done(
        &mut shell,
        "select.set",
        json!({"refs": ["wire:\"PE-0VDC\" -> \"-X1032:2/a\""]}),
    );
    done(&mut shell, "bond.draw", json!({}));
    done(&mut shell, "preview.apply", json!({}));
    assert!(
        shell.conflict_members().is_empty(),
        "no EW-POT-001 after the bond"
    );
    assert!(shell.conflict_overlay(0).is_empty());
    shell.shutdown();
}

/// FT-696 (WP-S2.11 D9, GT-6): a connection point an EW-POT-001 finding names (a potential without a definition is
/// keyed by one) adds the wires that end at it to the overlay, in the page's connection order; a point without wires
/// is a closed ring one grid step across, centred on the point; nothing named, nothing drawn.
#[test]
fn ft696_a_named_connection_point_draws_its_wires_or_its_ring() {
    let tmp = Tmp::new("ft696");
    let mut shell = tmp.shell_in(Mode::Gui, true, "103");
    let page = std::sync::Arc::clone(shell.canvas().expect("folio 103").page());
    let sheet = |p: &(i64, i64)| page.snapper.page_to_sheet(*p);
    let wired = page
        .tables
        .cps
        .iter()
        .find(|c| !c.connections.is_empty())
        .expect("a wired CP");
    let lines = page.conflict_lines(&[], &[(wired.function, wired.idx)]);
    let expected: Vec<Vec<(i64, i64)>> = page
        .tables
        .connections
        .iter()
        .filter(|t| wired.connections.contains(&t.id))
        .map(|t| t.polyline.iter().map(sheet).collect())
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(lines, expected);
    let lone = page
        .tables
        .cps
        .iter()
        .find(|c| c.connections.is_empty())
        .expect("a CP without wires");
    let ring = page.conflict_lines(&[], &[(lone.function, lone.idx)]);
    assert_eq!(ring.len(), 1, "one ring");
    let ring = &ring[0];
    assert_eq!(ring.len(), 25);
    assert_eq!(ring.first(), ring.last(), "closed");
    let (cx, cy) = sheet(&lone.at);
    let step = page.tables.grid.0 / page.tables.grid.1;
    for &(x, y) in ring {
        let r = (((x - cx) as f64).powi(2) + ((y - cy) as f64).powi(2)).sqrt();
        assert!(
            (r - step as f64 / 2.0).abs() <= 1.0,
            "{r} nm from the point, half of {step}"
        );
    }
    assert_eq!(page.conflict_lines(&[], &[]), Vec::<Vec<(i64, i64)>>::new());
    shell.shutdown();
}

/// FT-696 (GT-6, ID-1375): a real EW-POT-001 finding on a potential without a definition (Golden A without B1 and
/// without its potential definitions, so 0 V and PE merge with no definition to key them) draws, through
/// `conflict_overlay`, exactly the wires of the connection point its discriminator names: the overlay is that of one
/// point of the page, and no definition gives members.
#[test]
fn ft696_an_undefined_potential_s_finding_draws_its_named_point() {
    let tmp = Tmp::new("ft696-finding");
    let mut shell = without_b1(&tmp);
    // Each pass deletes the definition a potential row stands for; a merged potential's next one then shows.
    for _ in 0..64 {
        let defs: Vec<serde_json::Value> = {
            let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
            t.nodes
                .iter()
                .filter(|n| n.kinds == ["potential_def"])
                .map(|n| json!({"kind": "potential_def", "id": n.records[0].to_string()}))
                .collect()
        };
        if defs.is_empty() {
            break;
        }
        done(
            &mut shell,
            "command.run",
            json!({"cmd": "edit.delete", "params": {"targets": defs}}),
        );
    }
    assert!(shell.conflict_members().is_empty(), "no definition gives members");
    let labels = shell.page_labels().to_vec();
    let mut drawn = 0;
    for folio in &labels {
        done(&mut shell, "page.goto", json!({ "folio": folio }));
        let overlay = shell.conflict_overlay(0);
        if overlay.is_empty() {
            continue;
        }
        drawn += 1;
        let page = std::sync::Arc::clone(shell.canvas().expect("the page").page());
        assert!(
            page.tables
                .cps
                .iter()
                .any(|c| page.conflict_lines(&[], &[(c.function, c.idx)]) == overlay),
            "folio {folio}: the overlay is one named point's"
        );
    }
    assert!(drawn > 0, "the finding's named point is drawn on its page");
    shell.shutdown();
}

/// FT-1363 (W50-F13): in a window a script drives, the Waive prompt's refusal of the human-only `check.waive` is worded
/// by the command's title under the fields, never the error's raw form with the exposure class and the origin.
#[test]
fn a_scripted_window_shows_the_waive_refusal_in_words() {
    let tmp = Tmp::new("waive-scripted");
    let dir = tmp.golden_copy();
    common::copy_dir(std::path::Path::new(common::GOLDEN_A), &dir);
    let mut shell = ShellModel::new_scripted(ew_viewmodel::shell::ShellConfig::in_dir(
        &tmp.0.join("config"),
        "0.0.0-test",
    ));
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "messages.show", json!({"on": true}));
    let row = finding(&shell, "EW-CORE-033", "-3W1");
    done(
        &mut shell,
        "navigator.row",
        json!({"kind": "messages", "key": row.key}),
    );
    done(&mut shell, "messages.waive", json!({"date": DATE}));
    shell.set_form_field("reason", "Cable length is set on site");
    shell.set_form_field("author", "A. Engineer");
    let Outcome::Failed { message } = shell.submit_form() else {
        panic!("refused for the script's origin")
    };
    assert_eq!(
        message,
        "Only a person can run Waive finding, in the window; a script or a plugin cannot"
    );
    let Some(Dialog::Form(form)) = shell.dialog() else {
        panic!("the prompt stays");
    };
    assert_eq!(form.error.as_deref(), Some(message.as_str()));
    shell.shutdown();
}
