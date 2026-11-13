// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File ▸ Import… of a drawing in the window (WP-S3.19, doc 14 §13): the picker's answer, a DXF, opens the dialog that
//! says what the file is and that its unit is assumed; Add as page makes a new graphic page and shows it; a second
//! import, Place under page, puts the drawing under the shown page as an underlay; each outcome is a toast; the
//! accessible-name walker after the dialog.

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_ui::app::Pick;
use ew_ui::ew_viewmodel::ew_api::Session;
use ew_ui::ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use serde_json::json;
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

/// A drawing without a unit: one rectangle.
const PLAN: &str = "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\n0\n90\n4\n70\n1\n10\n20\n20\n20\n10\n120\n20\n20\n\
10\n120\n20\n70\n10\n20\n20\n70\n0\nENDSEC\n0\nEOF\n";

#[test]
fn a_dxf_becomes_a_page_and_then_an_underlay_through_the_import_dialog() {
    let tmp = Tmp::new();
    let dxf = tmp.0.join("plan.dxf");
    std::fs::write(&dxf, PLAN).unwrap();
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(dxf.clone()));
    platform.answers.borrow_mut().push_back(Pick::Path(dxf.clone()));
    let dir = tmp.0.join("proj.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    let params = json!({"dir": dir.to_string_lossy(), "template": "builtin"});
    assert_eq!(shell.dispatch("file.new", &params), Outcome::Done);
    let mut h = harness_with(shell, platform, None);
    settle(&mut h);
    wait_for_page(&mut h);
    let pages_before = h.state().shell().page_rows().len();

    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    h.get_by_label("Import…").click();
    settle(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ImportDrawing { unitless: true, .. })
    ));
    h.get_by_label_contains("is a DXF drawing");
    h.get_by_label_contains("read in millimetres");
    support::assert_accessible(&h);
    h.get_by_label("Add as page").click();
    settle(&mut h);
    wait_for_page(&mut h);
    assert!(h.state().shell().dialog().is_none());
    assert_eq!(h.state().shell().page_rows().len(), pages_before + 1);
    let shown = h.state().shell().current_folio().map(str::to_owned).unwrap();
    // The outcome is a toast, not a banner: a banner is for a state of the whole project (doc 16a §6.11).
    assert_eq!(h.state().shell().banner(), None);
    let toast = h.state().shell().toast().map(|t| t.text.clone());
    assert_eq!(toast, Some(format!("Imported plan.dxf as page {shown}.")));

    support::open_menu(&mut h, &["File"]);
    settle(&mut h);
    h.get_by_label("Import…").click();
    settle(&mut h);
    h.get_by_label(&format!("Place under page {shown}")).click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    let toast = h.state().shell().toast().map(|t| t.text.clone());
    assert_eq!(toast, Some(format!("Placed plan.dxf under page {shown}.")));
    support::dispatch(&mut h, "file.save", json!({}));
    let (saved, _) = Session::open_read_only(&dir, "test").unwrap();
    let underlays: Vec<_> = saved
        .query()
        .page_items()
        .iter()
        .filter_map(|(_, i)| i.kind.underlay().map(|u| u.name.clone()))
        .collect();
    assert_eq!(underlays, ["plan.dxf"]);
}
