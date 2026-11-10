// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fixes of window check W49.3 in the collision dialog:
//!
//! - F1 (doc 16a §6.12): every opening of the collision dialog puts its first focus on its Primary, Apply, whatever
//!   opened it: a poll, a poll in the frame another dialog closed, a refused save. Enter then applies the choices shown
//!   (mine), never All theirs.

use crate::support;

use std::path::Path;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT as _;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::{AnyCommand, Recovery, Session, SessionOptions, StoreOptions};
use ew_ui::ew_viewmodel::shell::Dialog;
use ew_ui::ew_viewmodel::strings;
use serde_json::{Value, json};
use support::{Scripted, Tmp, copy_dir, dispatch, harness, settle, wait_for_page};

/// W20's ID, from its source key in the project's page files.
pub(crate) fn w20(dir: &Path) -> String {
    for e in std::fs::read_dir(dir.join("pages")).unwrap() {
        let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
        if let Some(line) = text.lines().find(|l| l.contains("\"key\":\"connections|W20\"")) {
            let v: Value = serde_json::from_str(line).unwrap();
            return v["id"].as_str().unwrap().to_owned();
        }
    }
    panic!("W20 is on no page");
}

fn colour(wire: &str, value: Value) -> AnyCommand {
    AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "connection", "targets": [wire], "path": "overrides.colour", "value": value}),
    }
}

/// A colleague's save of W20's colour `value` on a copy of `live`: its written files copied into `live`, the manifest last.
pub(crate) fn colleague(tmp: &Tmp, live: &Path, value: &str) {
    let copy = tmp.0.join("colleague.ew");
    let _ = std::fs::remove_dir_all(&copy);
    copy_dir(live, &copy);
    let _ = std::fs::remove_file(copy.join(".~ew.lock"));
    let mut store = StoreOptions::new("ElektraWatt test");
    store.state_dir = None;
    let (mut s, _) =
        Session::open(&copy, store, SessionOptions::default(), Recovery::Keep).expect("the copy opens");
    s.execute(&colour(&w20(&copy), json!([value])))
        .expect("the colleague's edit");
    let report = s.save().expect("the colleague saves");
    s.close();
    for f in &report.written {
        std::fs::copy(copy.join(f.as_str()), live.join(f.as_str())).unwrap();
    }
    std::fs::copy(copy.join("manifest.json"), live.join("manifest.json")).unwrap();
}

/// The window on a copy of Golden A with W20's colour BU edited here and WH saved by a colleague on disk.
fn collided(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir.clone()));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "connection", "targets": [w20(&dir)],
            "path": "overrides.colour", "value": ["BU"]}}),
    );
    colleague(tmp, &dir, "WH");
    settle(&mut h);
    h
}

/// The collision dialog is open with its Primary, Apply, focused; Enter applies mine: the dialog goes, and the window
/// keeps its own value (it is dirty: mine is not on disk), never theirs.
#[track_caller]
fn enter_applies_mine(h: &mut Harness<'_, App>, path: &str) {
    settle(h);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::ExternalChange { .. })),
        "{path}: the dialog is open"
    );
    let apply = strings::text(ew_ui::ew_viewmodel::key!("external-apply"));
    let node = support::off_chrome(h, apply).unwrap_or_else(|| panic!("{path}: no {apply}"));
    assert!(
        node.accesskit_node().is_focused(),
        "{path}: the Primary {apply} has the first focus"
    );
    h.key_press(Key::Enter);
    settle(h);
    assert_eq!(h.state().shell().dialog(), None, "{path}: Apply ran");
    assert!(
        h.state().shell().is_dirty(),
        "{path}: mine kept, theirs not taken"
    );
}

#[test]
fn f1_every_opening_of_the_collision_dialog_focuses_apply() {
    // A poll opens it.
    let tmp = Tmp::new();
    let mut h = collided(&tmp);
    dispatch(&mut h, "project.check_disk", json!({}));
    enter_applies_mine(&mut h, "a poll");

    // A poll in the frame another dialog closed: the change was found under that dialog and waited.
    let tmp = Tmp::new();
    let mut h = collided(&tmp);
    dispatch(&mut h, "help.about", json!({}));
    settle(&mut h);
    dispatch(&mut h, "project.check_disk", json!({}));
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::About)));
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(&mut h, "project.check_disk", json!({}));
    enter_applies_mine(&mut h, "a poll after another dialog");

    // A save refused for the collisions, after Decide later.
    let tmp = Tmp::new();
    let mut h = collided(&tmp);
    dispatch(&mut h, "project.check_disk", json!({}));
    settle(&mut h);
    dispatch(&mut h, "ui.dismiss", json!({}));
    settle(&mut h);
    let _ = h.state_mut().shell_mut().dispatch("file.save", &json!({}));
    enter_applies_mine(&mut h, "a refused save");
}

/// C49-p10b#4: a collision dialog rebuilt by a second change is opened again: the focus the user moved to All theirs
/// does not stay, the rebuilt dialog's first focus is Apply, and Enter applies mine.
#[test]
fn f1b_a_rebuilt_collision_dialog_takes_its_first_focus_again() {
    let tmp = Tmp::new();
    let mut h = collided(&tmp);
    dispatch(&mut h, "project.check_disk", json!({}));
    settle(&mut h);
    let theirs = strings::text(ew_ui::ew_viewmodel::key!("external-all-theirs"));
    for _ in 0..12 {
        if support::off_chrome(&h, theirs).is_some_and(|n| n.accesskit_node().is_focused()) {
            break;
        }
        h.key_press(Key::Tab);
        settle(&mut h);
    }
    assert!(
        support::off_chrome(&h, theirs).is_some_and(|n| n.accesskit_node().is_focused()),
        "All theirs has the focus"
    );
    let dir = tmp.0.join("g.ew");
    colleague(&tmp, &dir, "RD");
    dispatch(&mut h, "project.check_disk", json!({}));
    enter_applies_mine(&mut h, "a rebuilt dialog");
}

/// C50-p1#1 (W49.3b-N1): after Decide later, a save refused for the collisions shows its refusal in the banner with the
/// held saves' Choose… beside it, and Choose… opens the collision dialog again.
#[test]
fn a_refused_saves_banner_keeps_choose() {
    let tmp = Tmp::new();
    let mut h = collided(&tmp);
    dispatch(&mut h, "project.check_disk", json!({}));
    settle(&mut h);
    dispatch(&mut h, "ui.dismiss", json!({}));
    settle(&mut h);
    h.state_mut().dispatch_from_window("file.save", &json!({}));
    settle(&mut h);
    assert_eq!(
        h.state().shell().banner(),
        Some("Saving failed: the change on disk collides with your edits; choose yours or theirs first.")
    );
    let choose = strings::text(ew_ui::ew_viewmodel::key!("banner-external-choose"));
    support::off_chrome(&h, choose)
        .unwrap_or_else(|| panic!("the banner shows {choose}"))
        .click();
    settle(&mut h);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::ExternalChange { .. })),
        "Choose… opens the collision dialog"
    );
    h.state_mut().shell_mut().shutdown();
}
