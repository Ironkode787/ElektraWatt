// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S6.04 D13 (NFR-COMP-04, SE-3): the Approve and the Release dialog end with the engineering-verification
//! disclaimer, the About dialog's sentence, as their last line before the buttons, in English and in Dutch.

use crate::support;

use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::NodeT as _;
use ew_ui::app::App;
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle};

const RELEASE_MIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/fixtures/release-min");

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

/// The texts of the window's last frame (each node's label, then its value; a label is often its value too) in the
/// order AccessKit lists them.
fn texts(h: &Harness<'_, App>) -> Vec<String> {
    h.root()
        .children_recursive()
        .flat_map(|n| [n.accesskit_node().label(), n.accesskit_node().value()])
        .flatten()
        .collect()
}

/// The Approve dialog (the gate passes but for the sign-off) and the Release dialog (blocked until the sign-off, so
/// it shows Close only) end with the disclaimer, then their buttons, in English and in Dutch.
#[test]
fn the_approve_and_release_dialogs_end_with_the_disclaimer() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("release-min");
    copy_dir(Path::new(RELEASE_MIN), &dir);
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    for (language, disclaimer, approve, approve_buttons, release, release_buttons) in [
        (
            "en",
            "ElektraWatt's checks do not replace engineering verification by a competent person.",
            "Approve revision V1.0",
            ["Cancel", "Approve"].as_slice(),
            "Release revision V1.0?",
            ["Close"].as_slice(),
        ),
        (
            "nl",
            "De controles van ElektraWatt vervangen geen technische verificatie door een deskundige.",
            "Revisie V1.0 goedkeuren",
            ["Annuleren", "Goedkeuren"].as_slice(),
            "Revisie V1.0 vrijgeven?",
            ["Sluiten"].as_slice(),
        ),
    ] {
        dispatch(&mut h, "view.language", json!({ "value": language }));
        for (action, title, buttons) in [
            ("revision.approve", approve, approve_buttons),
            ("revision.release", release, release_buttons),
        ] {
            dispatch(&mut h, action, json!({}));
            settle(&mut h);
            settle(&mut h);
            let texts = texts(&h);
            assert!(texts.iter().any(|t| t == title), "{language} {action}: {texts:?}");
            let at = texts
                .iter()
                .rposition(|t| t == disclaimer)
                .unwrap_or_else(|| panic!("{language} {action}: no disclaimer in {texts:?}"));
            let after: Vec<&str> = texts[at + 1..].iter().map(String::as_str).collect();
            // The footer adds its buttons right to left in the platform's order (`dialog::footer_order`): Windows
            // shows the Primary left of Cancel, so it is added last; Linux shows it right of Cancel, added first.
            let buttons: Vec<&str> = if cfg!(target_os = "windows") {
                buttons.to_vec()
            } else {
                buttons.iter().rev().copied().collect()
            };
            assert_eq!(after, buttons, "{language} {action}: {texts:?}");
            dispatch(&mut h, action, json!({ "cancel": true }));
            settle(&mut h);
        }
    }
    h.state_mut().shell_mut().shutdown();
}
