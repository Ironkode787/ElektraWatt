// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Surface… (doc 12 §3.8) on the sample's layout 902: it opens the chooser of -UC1's faces; choosing the door makes
//! its record, page 904 after the plate's 902 with the door's view and its legend page, in one undo step; a second
//! Surface… of the door goes to its page.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, Outcome};
use serde_json::json;

#[test]
fn the_door_gets_its_page_and_legend_in_one_undo_step() {
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
        words
            .iter()
            .any(|w| w.starts_with("Door \u{b7} 600 \u{d7} 800 mm") && w.ends_with("new page")),
        "{words:?}"
    );
    // The door is chosen first: the first face without a page.
    assert!(
        form.fields[0]
            .options
            .iter()
            .any(|(v, _)| *v == form.fields[0].value && v.ends_with("/door"))
    );
    let pages = s.page_rows().len();
    assert_eq!(s.submit_form(), Outcome::Done);
    assert_eq!(s.current_folio(), Some("904"));
    assert!(
        s.page_rows().iter().any(|r| r.folio == "904.1"),
        "its legend page"
    );
    assert_eq!(s.page_rows().len(), pages + 2);
    // Surface… of the door again goes to its page.
    done(&mut s, "page.goto", json!({"folio": "902"}));
    done(&mut s, "layout.surface_page", json!({"surface": "door"}));
    assert_eq!(s.current_folio(), Some("904"));
    assert_eq!(s.page_rows().len(), pages + 2);
    // One undo takes back the door, its page and its legend.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.page_rows().len(), pages);
    // No door record is left: the chooser offers the door to be made.
    done(&mut s, "layout.surface_page", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the chooser: {:?}", s.dialog())
    };
    assert!(form.fields[0].options.iter().any(|(v, _)| v.ends_with("/door")));
    s.shutdown();
}
