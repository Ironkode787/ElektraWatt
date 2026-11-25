// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New page… asked for on a front-matter page: a layout page, whose kind no section holds yet, is proposed in the first
//! section as a wiring page is, never in the front matter.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, ShellModel};
use serde_json::json;

fn field(s: &ShellModel, name: &str) -> String {
    match s.dialog() {
        Some(Dialog::Form(f)) => f
            .fields
            .iter()
            .find(|x| x.name == name)
            .map(|x| x.value.clone())
            .unwrap_or_default(),
        other => panic!("the New page form: {other:?}"),
    }
}

#[test]
fn a_layout_page_asked_for_on_the_title_page_goes_into_a_section() {
    let tmp = Tmp::new("new-page-front");
    let mut s = tmp.empty_shell(Mode::Gui);
    // A project of the company standard: its title page 1 in the front matter, its sections (wiring, pneumatics, lists).
    done(&mut s, "tools.company_standard", json!({}));
    let company = tmp.0.join("company.ew");
    done(
        &mut s,
        "wizard.finish",
        json!({ "dir": company.to_string_lossy() }),
    );
    done(&mut s, "wizard.cancel", json!({}));
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("job.ew").to_string_lossy(), "template": company.to_string_lossy()}),
    );
    done(&mut s, "page.goto", json!({"folio": "1"}));
    done(&mut s, "page.new", json!({}));
    let first = (field(&s, "folio"), field(&s, "section"));
    assert!(
        !first.1.is_empty(),
        "a wiring page goes into a section: {first:?}"
    );
    s.set_form_field("type", "panel_layout");
    assert_eq!(
        (field(&s, "folio"), field(&s, "section")),
        first,
        "the layout page keeps the section's folio"
    );
    // So does New page… asked for a layout page at once.
    let _ = s.dispatch("ui.dismiss", &json!({}));
    done(&mut s, "page.new", json!({"type": "panel_layout"}));
    assert_eq!((field(&s, "folio"), field(&s, "section")), first);
}
