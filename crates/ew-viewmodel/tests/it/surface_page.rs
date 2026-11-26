// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Surface… (doc 12 §3.8) on the sample's layout 902: it opens the chooser of -UC1's faces with their sizes and pages;
//! choosing the bottom makes its record, page 905 after the plate's 902 (903 and the door's 904 are taken) with the
//! bottom's view and its legend (a page block that prints its first page when the bottom holds something), in one undo
//! step; Surface… of the door goes to its page 904.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, Outcome};
use serde_json::json;

#[test]
fn a_face_gets_its_page_and_legend_in_one_undo_step() {
    let tmp = Tmp::new("surface-page");
    let mut s = tmp.sample_in(Mode::Gui, true, "902");
    done(&mut s, "layout.surface_page", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the chooser: {:?}", s.dialog())
    };
    let words: Vec<&str> = form.fields[0].options.iter().map(|(_, w)| w.as_str()).collect();
    assert!(
        words
            .iter()
            .any(|w| w.starts_with("Mounting plate") && w.ends_with("page 902")),
        "{words:?}"
    );
    assert!(
        words.iter().any(
            |w| w.starts_with("Door \u{b7} 600 \u{d7} 800 mm \u{b7} hinged left") && w.ends_with("page 904")
        ),
        "{words:?}"
    );
    // The first face without a page is chosen first.
    assert!(
        form.fields[0].value.ends_with("/left"),
        "{}",
        form.fields[0].value
    );
    let bottom = form.fields[0]
        .options
        .iter()
        .find(|(v, _)| v.ends_with("/bottom"))
        .map(|(v, _)| v.clone())
        .expect("the bottom is offered");
    done(&mut s, "ui.dismiss", json!({}));
    let pages = s.page_rows().len();
    done(&mut s, "layout.surface_page", json!({"surface": bottom}));
    assert_eq!(s.current_folio(), Some("905"));
    assert_eq!(s.legend_offer(), ew_api::output::LegendOffer::Paged, "its legend");
    assert_eq!(
        s.page_rows().len(),
        pages + 1,
        "the empty bottom's legend prints no page yet"
    );
    // Surface… of the door goes to its page.
    done(&mut s, "layout.surface_page", json!({"surface": "door"}));
    assert_eq!(s.current_folio(), Some("904"));
    // One undo takes back the bottom, its page and its legend.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.page_rows().len(), pages);
    done(&mut s, "layout.surface_page", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the chooser: {:?}", s.dialog())
    };
    assert!(
        form.fields[0].options.iter().any(|(v, _)| v.ends_with("/bottom")),
        "no bottom record is left"
    );
    assert_eq!(s.submit_form(), Outcome::Done, "the chooser draws the left side");
    s.shutdown();
}
