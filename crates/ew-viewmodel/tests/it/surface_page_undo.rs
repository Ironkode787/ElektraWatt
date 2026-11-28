// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Undo and Redo of Surface… (`layout.surface_page`) where it makes a face of the cabinet (doc 12 §2.8): the face, its
//! panel layout page and its legend page are one step.

use crate::common;

use common::{Tmp, done, form};
use ew_viewmodel::shell::Mode;
use serde_json::json;

/// The sample's cabinet has no left side: Surface… makes it with its page in one step; Undo takes the face and its
/// pages back, so Surface… offers it as not made again, and Redo makes it as before.
#[test]
fn undo_takes_a_new_face_and_its_pages_back_and_redo_makes_them_again() {
    let tmp = Tmp::new("surface-page-undo");
    let mut s = tmp.sample_in(Mode::Gui, true, "902");
    let offered = |s: &mut ew_viewmodel::shell::ShellModel| {
        done(s, "layout.surface_page", json!({}));
        let options = form(s).fields[0].options.clone();
        done(s, "ui.dismiss", json!({}));
        options
    };
    let before = offered(&mut s);
    let left = before
        .iter()
        .map(|(v, _)| v.clone())
        .find(|v| v.ends_with("/left"))
        .expect("the cabinet's left side, not made yet");
    done(&mut s, "layout.surface_page", json!({"surface": left}));
    let page = common::state(&s)["page"].clone();
    assert_ne!(page, json!("902"), "Surface… went to the new face's page");
    let made = offered(&mut s);
    assert_ne!(made, before, "the left side is made, with its page");

    done(&mut s, "edit.undo", json!({}));
    assert_eq!(offered(&mut s), before, "Undo takes the face and its pages back");
    let gone = s.dispatch("page.goto", &json!({ "folio": page }));
    assert!(
        !matches!(gone, ew_viewmodel::shell::Outcome::Done),
        "the left side's page is gone: {gone:?}"
    );

    done(&mut s, "edit.redo", json!({}));
    assert_eq!(offered(&mut s), made, "Redo makes the face and its page again");
    done(&mut s, "page.goto", json!({ "folio": page }));
    s.shutdown();
}
