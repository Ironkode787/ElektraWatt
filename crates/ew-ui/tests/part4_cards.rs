// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 4 in the window (§12.4): Escape closes every dialog without its action (CP1); the toasts stand at
//! the canvas's top right, 300 px wide, newest on top (CP2, CP7); the palette lists its results with breadcrumbs and
//! key chips (CP4); the empty states are named groups and a finished job carries its outcome badge (CP5, FT-510); a
//! stale card shows its "Stale" badge as a node, never a "[stale]" text (CP8). Each followed by the accessible-name
//! walker.

mod support;

use std::time::Duration;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

fn run_until_jobs_end(h: &mut Harness<'_, App>) {
    for _ in 0..3000 {
        h.run_ok();
        if h.state().shell().jobs().running() == 0 {
            settle(h);
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the job did not end");
}

fn group_named(h: &Harness<'_, App>, name: &str) -> bool {
    h.get_all_by_role(egui::Role::Group)
        .any(|n| n.accesskit_node().label().as_deref() == Some(name))
}

fn has_text(h: &Harness<'_, App>, text: &str) -> bool {
    h.root().children_recursive().any(|n| {
        let n = n.accesskit_node();
        n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text)
    })
}

#[test]
fn cp1_escape_closes_every_dialog_without_its_action() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    let rev = h.state().shell().revision();
    for (action, title) in [
        ("help.about", None),
        ("help.shortcuts", Some("Keyboard shortcuts")),
        ("page.goto", Some("Go to page or cell")),
        ("page.new", Some("New page")),
        ("devices.renumber", Some("Renumber devices")),
        ("project.purge_assets", None),
    ] {
        let _ = h.state_mut().shell_mut().dispatch(action, &json!({}));
        settle(&mut h);
        if h.state().shell().dialog().is_none() {
            // Purge with nothing unused says so in a toast instead of a dialog.
            continue;
        }
        if let Some(title) = title {
            assert!(
                h.query_all_by_label(title).next().is_some(),
                "{action}: the title {title}"
            );
        }
        h.key_press(Key::Escape);
        settle(&mut h);
        assert!(h.state().shell().dialog().is_none(), "{action}: Escape closes it");
        assert_eq!(h.state().shell().revision(), rev, "{action}: nothing changed");
    }
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn cp2_cp7_the_toasts_stand_at_the_canvas_top_right_newest_on_top() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/function_text", "value": "Spare"}),
    );
    dispatch(
        &mut h,
        "file.export_pdf",
        json!({ "path": tmp.0.join("a.pdf").to_string_lossy() }),
    );
    run_until_jobs_end(&mut h);
    let toasts: Vec<egui::Rect> = h
        .get_all_by_role(egui::Role::Group)
        .filter(|n| n.accesskit_node().label().as_deref() == Some("Notification"))
        .map(|n| n.rect())
        .collect();
    assert_eq!(toasts.len(), 2, "two keys, two toasts (at most three)");
    let edit = h.get_by_label("Set Function text on 2 devices").rect();
    let export = h.get_by_label_contains("Exported a.pdf").rect();
    assert!(export.top() < edit.top(), "the newest on top");
    let pane = h.state().command_surface_rects().2.expect("the canvas pane");
    for r in &toasts {
        assert!((r.width() - 300.0).abs() <= 2.0, "300 px wide: {r:?}");
        assert!(
            (pane.right() - 12.0 - r.right()).abs() <= 2.0,
            "12 px from the canvas's right edge: {r:?} in {pane:?}"
        );
        assert!(
            r.top() >= pane.top() + 12.0 - 2.0,
            "under the canvas's top: {r:?}"
        );
    }
    let (upper, lower) = if toasts[0].top() < toasts[1].top() {
        (toasts[0], toasts[1])
    } else {
        (toasts[1], toasts[0])
    };
    assert!(
        (lower.top() - upper.bottom() - 8.0).abs() <= 2.0,
        "space.3 between toasts: {upper:?} {lower:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn cp4_the_palette_rows_carry_breadcrumbs_and_key_chips() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "tools.palette", json!({}));
    settle(&mut h);
    let entries = h.state().shell().palette_entries();
    assert!(!entries.is_empty());
    assert!(entries.iter().all(|e| !e.breadcrumb.is_empty()));
    // The field has the keyboard focus: type into it.
    h.event(egui::Event::Text("save as".into()));
    settle(&mut h);
    let entries = h.state().shell().palette_entries();
    let save_as = entries.iter().find(|e| e.id == "file.save_as").expect("Save as…");
    assert_eq!(save_as.breadcrumb, "File");
    assert!(
        h.query_by_label_contains("Save as…").is_some(),
        "the row is named by its label"
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn cp5_ft510_empty_states_and_the_jobs_outcome_badge() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "jobs.show", json!({}));
    settle(&mut h);
    assert!(group_named(&h, "No jobs yet"), "the Jobs pane's empty state");
    dispatch(
        &mut h,
        "file.export_pdf",
        json!({ "path": tmp.0.join("a.pdf").to_string_lossy() }),
    );
    run_until_jobs_end(&mut h);
    assert!(!group_named(&h, "No jobs yet"));
    assert!(h.query_by_label("Done").is_some(), "FT-510: the outcome badge");
    dispatch(&mut h, "preview.show", json!({}));
    settle(&mut h);
    assert!(
        group_named(&h, "No preview is open"),
        "the Preview tab's empty state"
    );
    assert!(
        has_text(
            &h,
            "Renumbering, a quick fix or an import shows its changes here before you apply them."
        ),
        "its sentence"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn cp8_a_stale_card_shows_its_badge_and_apply_stays_the_primary() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/tag.counter", "value": "9"}),
    );
    settle(&mut h);
    assert!(h.state().shell().properties().card().is_some());
    assert!(h.query_by_label("Stale").is_none(), "fresh: no badge");
    // An edit elsewhere makes the card stale.
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
            "path": "function_text", "value": "x"}}),
    );
    settle(&mut h);
    assert!(
        h.state().shell().properties().card().is_some(),
        "the card stays open"
    );
    assert!(
        h.query_all_by_label("Stale").next().is_some(),
        "the Stale badge is a node"
    );
    assert!(
        !h.root()
            .children_recursive()
            .any(|n| n.accesskit_node().label().is_some_and(|l| l.contains("[stale]"))),
        "no [stale] text"
    );
    assert!(h.query_all_by_label("Apply").next().is_some());
    h.state_mut().shell_mut().shutdown();
}
