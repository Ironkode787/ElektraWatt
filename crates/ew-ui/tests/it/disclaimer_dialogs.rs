// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The engineering-verification disclaimer (NFR-COMP-04) is one line of the About dialog and nowhere else in the
//! window (the owner, 2026-11-12, FT-1711): the Approve and the Release dialog end with their buttons, in English and
//! in Dutch.

use crate::support;

use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::NodeT as _;
use ew_testdir::copy_dir;
use ew_ui::app::App;
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle};

const RELEASE_MIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/fixtures/release-min");

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
/// it shows Close only) show no disclaimer; the About dialog keeps its one line; in English and in Dutch.
#[test]
fn only_the_about_dialog_names_the_engineering_verification() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("release-min");
    copy_dir(Path::new(RELEASE_MIN), &dir);
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    for (language, disclaimer, approve, release) in [
        (
            "en",
            "ElektraWatt's checks do not replace engineering verification by a competent person.",
            "Approve revision V1.0",
            "Release revision V1.0?",
        ),
        (
            "nl",
            "De controles van ElektraWatt vervangen geen technische verificatie door een deskundige.",
            "Revisie V1.0 goedkeuren",
            "Revisie V1.0 vrijgeven?",
        ),
    ] {
        dispatch(&mut h, "view.language", json!({ "value": language }));
        for (action, title) in [("revision.approve", approve), ("revision.release", release)] {
            dispatch(&mut h, action, json!({}));
            settle(&mut h);
            settle(&mut h);
            let texts = texts(&h);
            assert!(texts.iter().any(|t| t == title), "{language} {action}: {texts:?}");
            assert!(
                !texts.iter().any(|t| t == disclaimer),
                "{language} {action}: a disclaimer in {texts:?}"
            );
            dispatch(&mut h, action, json!({ "cancel": true }));
            settle(&mut h);
        }
        dispatch(&mut h, "help.about", json!({}));
        settle(&mut h);
        settle(&mut h);
        let texts = texts(&h);
        assert!(
            texts.iter().any(|t| t == disclaimer),
            "{language}: About keeps the line: {texts:?}"
        );
        dispatch(&mut h, "ui.dismiss", json!({}));
        settle(&mut h);
    }
    h.state_mut().shell_mut().shutdown();
}
