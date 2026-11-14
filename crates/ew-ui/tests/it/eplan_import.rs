// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File ▸ Import… of EPLAN data in the window (WP-S3.18, doc 14 §5, §13), at 1920 × 1080: an EDZ shows the Data Portal
//! terms notice once per session, then what it means for the project; Update previews the changed fields of the
//! project's part copy on the card (AC-38); Add to my parts library writes the user's library; a report spreadsheet
//! offers the mappings that fit its headers and makes a new project.

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_ui::app::Pick;
use ew_ui::ew_viewmodel::ew_api::library::LibraryTiers;
use ew_ui::ew_viewmodel::shell::{Dialog, EplanStep, Mode, Outcome, ShellModel};
use serde_json::json;
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

const EDZ: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-import-eplan/tests/fixtures/synthetic-psu-7z.edz"
);

fn open_golden(tmp: &Tmp, files: &[&std::path::Path]) -> egui_kittest::Harness<'static, ew_ui::app::App> {
    let platform = Scripted::default();
    for f in files {
        platform.answers.borrow_mut().push_back(Pick::Path((*f).to_path_buf()));
    }
    let tiers = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let config = tmp.config().with_libraries(Some(tiers));
    let mut shell = ShellModel::new(Mode::Gui, config);
    let project = tmp.golden("proj.ew");
    assert_eq!(
        shell.dispatch("file.open", &json!({"dir": project.to_string_lossy()})),
        Outcome::Done
    );
    let mut h = harness_with(shell, platform, None);
    h.set_size(egui::Vec2::new(1920.0, 1080.0));
    settle(&mut h);
    wait_for_page(&mut h);
    h
}

fn import_menu(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>) {
    support::open_menu(h, &["File"]);
    settle(h);
    h.get_by_label("Import…").click();
    settle(h);
}

#[test]
fn an_edz_asks_the_terms_once_then_updates_the_part_copy_on_the_card_and_fills_the_library() {
    let tmp = Tmp::new();
    let edz = tmp.0.join("psu.edz");
    std::fs::copy(EDZ, &edz).unwrap();
    let mut h = open_golden(&tmp, &[&edz, &edz, &edz]);

    import_menu(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ImportEplan {
            step: EplanStep::Terms,
            ..
        })
    ));
    h.get_by_label_contains("terms restrict");
    support::assert_accessible(&h);
    h.get_by_label("Continue").click();
    settle(&mut h);
    let Some(Dialog::ImportEplan {
        step: EplanStep::Parts { read, matched, changed },
        ..
    }) = h.state().shell().dialog().cloned()
    else {
        panic!("the parts step: {:?}", h.state().shell().dialog());
    };
    assert_eq!((read, matched, changed), (1, 1, 1));
    h.get_by_label_contains("The project has a copy of 1 of them");
    support::assert_accessible(&h);
    h.get_by_label("Update 1 part copies").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    assert!(
        h.state().shell().properties().card().is_some(),
        "the card of the update lists the changed fields"
    );
    support::dispatch(&mut h, "ui.dismiss", json!({}));

    // The notice is shown once per session: the second import goes to the parts step; the library takes the part.
    import_menu(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ImportEplan {
            step: EplanStep::Parts { .. },
            ..
        })
    ));
    h.get_by_label("Add to my parts library").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    let toast = h.state().shell().toast().map(|t| t.text.clone());
    assert_eq!(
        toast.as_deref(),
        Some("psu.edz: 1 parts added to your parts library, 0 updated, 0 not imported.")
    );
    assert!(tmp.0.join("data").join("libraries").join("user-parts.ewlib").is_dir());
}

#[test]
fn a_report_offers_the_mappings_that_fit_and_becomes_a_new_project() {
    let tmp = Tmp::new();
    let report = tmp.0.join("klemmen.csv");
    std::fs::write(
        &report,
        "Terminal strip,Terminal,Level,Part number\n-X1,1,1,\n-X1,2,1,\n-X1,2,2,\n",
    )
    .unwrap();
    let mut h = open_golden(&tmp, &[&report]);
    import_menu(&mut h);
    let Some(Dialog::ImportEplan {
        step: EplanStep::Report { fits, .. },
        ..
    }) = h.state().shell().dialog().cloned()
    else {
        panic!("the report step: {:?}", h.state().shell().dialog());
    };
    assert_eq!(fits.len(), 1, "only the English terminal diagram fits these headers");
    h.get_by_label_contains("is an EPLAN report");
    support::assert_accessible(&h);
    let dir = tmp.0.join("report.ew");
    support::dispatch(&mut h, "ui.dismiss", json!({}));
    let outcome = h.state_mut().shell_mut().dispatch(
        "file.import_eplan",
        &json!({"file": report.to_string_lossy(), "how": "report", "preset": fits[0].0,
                "dir": dir.to_string_lossy(), "discard": true}),
    );
    assert_eq!(outcome, Outcome::Done);
    // A project without drawings has no page: its tool buttons are not judged here (see the report).
    h.run_ok();
    assert!(dir.join("manifest.json").is_file());
    let toast = h.state().shell().toast().map(|t| t.text.clone());
    assert_eq!(
        toast.as_deref(),
        Some("Imported klemmen.csv as a new project with 3 devices.")
    );
}
