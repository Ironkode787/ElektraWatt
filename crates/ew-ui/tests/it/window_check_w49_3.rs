// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fixes of window check W49.3 in the collision dialog (cycle 49, WP-S1.17 D24, decision log ID-1284):
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
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use ew_ui::ew_viewmodel::strings;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// A copy of `from` at `to`.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().map(Result::unwrap) {
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// W20's ID, from its source key in the project's page files.
fn w20(dir: &Path) -> String {
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

/// A colleague's save of W20's colour WH on a copy of `live`: its written files copied into `live`, the manifest last.
fn colleague(tmp: &Tmp, live: &Path) {
    let copy = tmp.0.join("colleague.ew");
    let _ = std::fs::remove_dir_all(&copy);
    copy_dir(live, &copy);
    let _ = std::fs::remove_file(copy.join(".~ew.lock"));
    let mut store = StoreOptions::new("ElektraWatt test");
    store.state_dir = None;
    let (mut s, _) =
        Session::open(&copy, store, SessionOptions::default(), Recovery::Keep).expect("the copy opens");
    s.execute(&colour(&w20(&copy), json!(["WH"])))
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
    colleague(tmp, &dir);
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
