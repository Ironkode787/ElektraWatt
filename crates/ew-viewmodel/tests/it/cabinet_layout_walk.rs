// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cabinet layout of a new project, walked as a panel builder does it (the enclosure made from a schematic page
//! is drawn on the layout page afterwards). Off a panel layout page the Create enclosure dialog says that it draws
//! nothing there and
//! offers to make the layout page; on the layout page it offers to draw the location's enclosure that exists; then a
//! rail, a duct, the cabinet's devices placed on the rail and item numbers.

use crate::common;

use common::{Tmp, at, done};
use ew_api::output::PanelTables;
use ew_viewmodel::dialogs::{DialogAnswer, spec};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

/// A new project with the multi-line page 101 and a breaker and a switch on it, at no location.
fn project(tmp: &Tmp) -> ShellModel {
    let config = tmp.config();
    let mut s = ShellModel::new(Mode::Gui, config);
    crate::common::make_project(&mut s, &tmp.0.join("cab.ew"), &[]);
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Supply"}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    for (part, k) in [("EW-GEN-MCB-1", (20, 20)), ("EW-GEN-SW-1", (40, 20))] {
        done(&mut s, "place.pick", json!({"part": part, "template": 0}));
        let sn = &s.canvas().unwrap().page().snapper;
        let p = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
        common::click(&mut s, p, json!({"alt": true}));
        done(&mut s, "tool.select", json!({}));
    }
    s
}

fn panel(s: &ShellModel) -> std::sync::Arc<PanelTables> {
    s.panel_tables().expect("the page shows a mounting plate")
}

/// A click at the surface point (mm) of the plate shown.
fn click_mm(s: &mut ShellModel, u: i128, v: i128) {
    let t = panel(s);
    let p = s
        .canvas()
        .unwrap()
        .page()
        .snapper
        .page_to_sheet(t.to_page((u * 1_000_000, v * 1_000_000)));
    done(s, "canvas.pointer", at(p, json!({})));
    common::click(s, p, json!({}));
}

/// The dialog's buttons by their labels: the Primary first.
fn buttons(s: &ShellModel) -> Vec<String> {
    let d = spec(s.dialog().expect("a dialog"), s);
    std::iter::once(d.primary.label)
        .chain(d.secondary.into_iter().map(|b| b.label))
        .collect()
}

#[test]
fn an_enclosure_made_elsewhere_is_drawn_on_the_layout_page_and_the_cabinet_is_filled() {
    let tmp = Tmp::new("cabinet-walk");
    let mut s = project(&tmp);
    // Off a layout page the dialog says what it does, and Create only makes the enclosure without drawing it.
    done(&mut s, "layout.enclosure_new", json!({}));
    let d = spec(s.dialog().unwrap(), &s);
    assert!(
        d.body
            .iter()
            .any(|l| l.starts_with("This page is not a panel layout page.")),
        "{:?}",
        d.body
    );
    assert_eq!(
        buttons(&s),
        [
            "Create and draw on a new panel layout page",
            "Create only",
            "Cancel"
        ]
    );
    let pages = s.page_rows().len();
    assert_eq!(s.submit_form(), Outcome::Done, "Create only");
    assert_eq!(s.page_rows().len(), pages, "no page made");
    // The layout page made afterwards: the dialog draws the enclosure that +CAB1 has, instead of refusing it.
    done(
        &mut s,
        "page.new",
        json!({"folio": "301", "type": "panel_layout", "title": "Cabinet layout"}),
    );
    assert!(s.panel_tables().is_none());
    done(&mut s, "layout.enclosure_new", json!({}));
    assert_eq!(buttons(&s)[0], "Draw -UC1 of +CAB1 on this page");
    assert_eq!(
        spec(s.dialog().unwrap(), &s).body,
        ["+CAB1 has the enclosure -UC1 already, part EW-GEN-ENC-1: the button draws it as it is."]
    );
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(s.dialog().is_none(), "the dialog closes");
    assert_eq!(panel(&s).location, "+CAB1");
    assert!(
        s.notifications()
            .entries()
            .any(|n| n.text == "Drew -UC1 of +CAB1 on page 301."),
        "the toast says where"
    );
    // A rail and a duct on the plate (520 × 720 mm).
    done(&mut s, "tool.rail_draw", json!({}));
    click_mm(&mut s, 40, 500);
    click_mm(&mut s, 480, 500);
    done(&mut s, "tool.duct_draw", json!({}));
    click_mm(&mut s, 40, 600);
    click_mm(&mut s, 480, 600);
    done(&mut s, "tool.finish", json!({}));
    done(&mut s, "tool.select", json!({}));
    assert_eq!(panel(&s).carriers.len(), 2, "a rail and a duct");
    // The devices have no location yet: Auto-fill rail offers them ticked (their parts go on a rail), and Fill gives
    // them the cabinet's and places them.
    done(&mut s, "layout.auto_fill_rail", json!({}));
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(panel(&s).placements.len() >= 2, "the breaker and the switch");
    // The page numbers its items.
    assert!(panel(&s).numbered);
    s.shutdown();
}

/// Off a layout page, the dialog's Primary makes the panel layout page after the page shown and draws there: the
/// enclosure made with it, then an enclosure the location has.
#[test]
fn off_a_layout_page_the_enclosure_is_drawn_on_a_new_layout_page() {
    let tmp = Tmp::new("cabinet-new-page");
    let mut s = project(&tmp);
    done(&mut s, "layout.enclosure_new", json!({}));
    let DialogAnswer::Action(action, params) = spec(s.dialog().unwrap(), &s).primary.answer else {
        panic!("the Primary is its own action")
    };
    assert_eq!(s.dispatch(action, &params), Outcome::Done);
    assert_eq!(s.current_folio(), Some("102"), "the page after 101");
    assert_eq!(
        ew_viewmodel::ew_api::output::page_kind_name(&s.page_rows()[s.page_index()].kind),
        "panel_layout"
    );
    assert_eq!(panel(&s).location, "+CAB1");
    // Back on 101 the location's enclosure is offered to be drawn on another new page.
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "layout.enclosure_new", json!({}));
    assert_eq!(
        buttons(&s),
        ["Draw -UC1 of +CAB1 on a new panel layout page", "Change its cabinet…", "Cancel"]
    );
    let DialogAnswer::Action(action, params) = spec(s.dialog().unwrap(), &s).primary.answer else {
        panic!("the Primary is its own action")
    };
    assert_eq!(s.dispatch(action, &params), Outcome::Done);
    assert_eq!(s.current_folio(), Some("103"));
    assert_eq!(panel(&s).location, "+CAB1");
    s.shutdown();
}
