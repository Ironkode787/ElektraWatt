// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where a page that a command makes goes (doc 06 §2.3, "Where a made page goes"), in a new project of the company
//! standard: its schematic page 1 and the table of contents in the front matter, the sections Wiring Definition,
//! Pneumatics and Nomenclature empty. A page of a kind goes to the section for that kind, never into the front matter
//! because a front-matter page was on show; where no section is for the kind, the command makes one and says so.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::dialogs::{DialogAnswer, spec};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use serde_json::json;

/// A new project of the company standard, page 1 shown.
fn job(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.empty_shell(Mode::Gui);
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
    s
}

/// The New page form's field `name`.
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

/// The folio and section number of the page shown.
fn shown(s: &ShellModel) -> (String, Option<u32>) {
    let row = &s.page_rows()[s.page_index()];
    (row.folio.clone(), row.section.as_ref().map(|(n, _)| *n))
}

/// The id of the page of folio `folio`.
fn page_id(s: &ShellModel, folio: &str) -> String {
    s.page_rows()
        .iter()
        .find(|r| r.folio == folio)
        .map(|r| r.id.to_string())
        .expect("the page")
}

/// New page… right-clicked on the schematic page 1 of the front matter proposes the wiring section, not the page
/// after 1; a fluid page the Pneumatics section, a layout page Nomenclature, as 902 of the reference project.
#[test]
fn new_page_on_a_front_matter_page_proposes_the_section_for_its_kind() {
    let tmp = Tmp::new("made-page-new");
    let mut s = job(&tmp);
    let at = format!("p:{}", page_id(&s, "1"));
    done(&mut s, "page.new", json!({"at": at}));
    assert_eq!(
        (field(&s, "folio"), field(&s, "section")),
        ("101".into(), "1".into())
    );
    s.set_form_field("type", "fluid");
    assert_eq!(
        (field(&s, "folio"), field(&s, "section")),
        ("201".into(), "2".into())
    );
    s.set_form_field("type", "panel_layout");
    assert_eq!(field(&s, "section"), "9");
    // A title page stays in the front matter.
    s.set_form_field("type", "title_page");
    assert_eq!(field(&s, "section"), "");
    s.shutdown();
}

/// A page of a kind no section is for (a graphic page) makes its section, numbered after the highest over the hundred
/// after the last, titled with the kind, in the same undo step as the page; the toast says so.
#[test]
fn a_page_of_a_kind_no_section_is_for_makes_its_section_and_says_so() {
    let tmp = Tmp::new("made-page-section");
    let mut s = job(&tmp);
    done(&mut s, "page.new", json!({"type": "graphic"}));
    assert_eq!(
        (field(&s, "folio"), field(&s, "section")),
        ("1000".into(), "10".into())
    );
    assert_eq!(s.submit_form(), Outcome::Done);
    assert_eq!(shown(&s), ("1000".into(), Some(10)));
    assert!(
        s.project_sections()
            .iter()
            .any(|(n, t)| *n == 10 && t == "Graphic"),
        "{:?}",
        s.project_sections()
    );
    assert!(
        s.notifications().entries().any(|n| n
            .text
            .starts_with("Page 1000 is made: section 10 Graphic (folios 1000 to 1099)")),
        "the toast says so"
    );
    done(&mut s, "edit.undo", json!({}));
    assert!(
        !s.project_sections().iter().any(|(n, _)| *n == 10),
        "one undo step"
    );
    assert!(!s.page_rows().iter().any(|r| r.folio == "1000"));
    s.shutdown();
}

/// Create enclosure's layout page, made from the schematic page 1, goes to Nomenclature (it went to the front matter
/// as page 3 while the dialog spoke of 101).
#[test]
fn the_layout_page_of_create_enclosure_goes_to_its_section() {
    let tmp = Tmp::new("made-page-layout");
    let mut s = job(&tmp);
    done(&mut s, "layout.enclosure_new", json!({}));
    let DialogAnswer::Action(action, params) = spec(s.dialog().unwrap(), &s).primary.answer else {
        panic!("the Primary is its own action")
    };
    assert_eq!(s.dispatch(action, &params), Outcome::Done);
    let (_, section) = shown(&s);
    assert_eq!(section, Some(9), "{:?}", shown(&s));
    s.shutdown();
}

/// Move to page's "To a new page" from the schematic page 1 makes its page in the wiring section.
#[test]
fn to_a_new_page_from_a_front_matter_page_goes_to_the_wiring_section() {
    let tmp = Tmp::new("made-page-move");
    let mut s = job(&tmp);
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-MCB-1", "template": 0}),
    );
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.page_to_sheet((sn.page_of_index(20), sn.page_of_index(20)));
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    let device = s.canvas().unwrap().page().tables.reps[0]
        .device
        .expect("the breaker");
    done(&mut s, "nav.show", json!({"record": device.to_string()}));
    done(&mut s, "edit.move_to_page", json!({"page": "new"}));
    assert_eq!(shown(&s), ("101".into(), Some(1)));
    s.shutdown();
}

/// Copy pages at an offset onto folios no section holds makes the section for the copies, and its card says so before
/// anything is made (it refused: "no section's folio range holds folio 301").
#[test]
fn copy_pages_at_an_offset_makes_the_section_it_needs_and_its_card_says_so() {
    let tmp = Tmp::new("made-page-copy");
    let mut s = job(&tmp);
    let page = json!({"folio": "101", "type": "multi_line", "title": "Supply", "section": "1"});
    done(&mut s, "page.new", page);
    done(&mut s, "pages.copy", json!({"pages": ["101"], "offset": 200}));
    let notices = s.preview_pane().notice_words().join("\n");
    assert!(
        notices.contains("section 10 Wiring Definition (folios 300 to 399) is made for it"),
        "{notices}"
    );
    done(&mut s, "preview.apply", json!({}));
    let copy = s
        .page_rows()
        .iter()
        .find(|r| r.folio == "301")
        .cloned()
        .expect("the copy");
    assert_eq!(copy.section.map(|(n, _)| n), Some(10));
    s.shutdown();
}

/// A station's "a page per card", drawn from the schematic page 1, makes its pages in the wiring section.
#[test]
fn a_station_s_pages_per_card_go_to_the_wiring_section() {
    let tmp = Tmp::new("made-page-station");
    let mut s = job(&tmp);
    let before: Vec<String> = s.page_rows().iter().map(|r| r.folio.clone()).collect();
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(&mut s, "plc.rack_create", json!({}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-DI-1"}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-DO-1"}));
    done(&mut s, "plc.rack_next", json!({"step": "draw"}));
    let new: Vec<(String, Option<u32>)> = s
        .page_rows()
        .iter()
        .filter(|r| !before.contains(&r.folio) && !r.generated)
        .map(|r| (r.folio.clone(), r.section.as_ref().map(|(n, _)| *n)))
        .collect();
    assert_eq!(new, [("101".into(), Some(1)), ("102".into(), Some(1))]);
    s.shutdown();
}
