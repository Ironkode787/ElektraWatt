// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Report… for a kind of list the set already has as a page:
//! the form says where it is under its Report field, and after Create the toast
//! names that page with Go to page beside Undo. The sample's cable overview from 906.

use crate::common;

use common::{Tmp, done, toast};
use ew_viewmodel::shell::{Dialog, Mode};
use serde_json::json;

#[test]
fn a_list_the_set_has_is_named_with_its_page() {
    let tmp = Tmp::new("report-kind-had");
    let mut s = tmp.sample_in(Mode::Gui, true, "101");
    done(
        &mut s,
        "report_block.new",
        json!({"kind": "cable_overview_diagrams"}),
    );
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the Report form: {:?}", s.dialog())
    };
    let note = s.form_note(&form, "kind").unwrap_or_default();
    assert!(
        note.contains("already has Cable overview and diagrams on page 906"),
        "{note}"
    );
    let _ = s.dispatch("ui.dismiss", &json!({}));
    done(
        &mut s,
        "report_block.new",
        json!({"kind": "cable_overview_diagrams", "folio": "960"}),
    );
    assert!(
        toast(&s).contains("The set already had one on page 906"),
        "{}",
        toast(&s)
    );
    let entry = s.notifications().entries().next().expect("the toast").clone();
    assert!(entry.actions().contains(&"undo"), "{:?}", entry.actions());
    done(
        &mut s,
        "notifications.run",
        json!({"entry": entry.serial, "action": "go_to"}),
    );
    assert_eq!(common::state(&s)["page"], "906");
}
