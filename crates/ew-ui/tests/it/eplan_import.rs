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
use std::path::Path;
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

const EDZ: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-import-eplan/tests/fixtures/synthetic-psu-7z.edz"
);

fn open_golden(tmp: &Tmp, files: &[&std::path::Path]) -> egui_kittest::Harness<'static, ew_ui::app::App> {
    let platform = Scripted::default();
    for f in files {
        platform
            .answers
            .borrow_mut()
            .push_back(Pick::Path((*f).to_path_buf()));
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
        step: EplanStep::Parts {
            read,
            matched,
            changed,
        },
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
    assert!(
        tmp.0
            .join("data")
            .join("libraries")
            .join("user-parts.ewlib")
            .is_dir()
    );
}

/// A report whose headers fit a shipped mapping offers it; Import as terminal diagram makes a new project without
/// pages, and the window's tool strip does not show Select enabled there (FT-1743).
#[test]
fn a_report_that_fits_a_mapping_becomes_a_new_project_and_a_pageless_project_judges_clean() {
    let tmp = Tmp::new();
    let report = tmp.0.join("klemmen.csv");
    std::fs::write(
        &report,
        "Terminal strip,Terminal,Level,Part number\n-X1,1,1,\n-X1,2,1,\n-X1,2,2,\n",
    )
    .unwrap();
    let dir = tmp.0.join("report.ew");
    let mut h = open_golden(&tmp, &[&report]);
    import_menu(&mut h);
    let Some(Dialog::ImportEplan {
        step: EplanStep::Report { fits, .. },
        ..
    }) = h.state().shell().dialog().cloned()
    else {
        panic!("the report step: {:?}", h.state().shell().dialog());
    };
    assert_eq!(
        fits.len(),
        1,
        "only the English terminal diagram fits these headers"
    );
    h.get_by_label_contains("is an EPLAN report");
    support::assert_accessible(&h);
    support::dispatch(&mut h, "ui.dismiss", json!({}));
    let outcome = h.state_mut().shell_mut().dispatch(
        "file.import_eplan",
        &json!({"file": report.to_string_lossy(), "how": "report", "preset": fits[0].0,
                "dir": dir.to_string_lossy(), "discard": true}),
    );
    assert_eq!(outcome, Outcome::Done);
    // The new project has no page: every enabled button is judged, Select among them.
    settle(&mut h);
    assert!(dir.join("manifest.json").is_file());
    let toast = h.state().shell().toast().map(|t| t.text.clone());
    assert_eq!(
        toast.as_deref(),
        Some("Imported klemmen.csv as a new project with 3 devices.")
    );
}

/// One File ▸ Import… takes every file and goes on by what the file is (FT-1741): a drawing to its dialog, an EDZ to
/// the terms, a parts XML to its parts, a parts table of the golden dialect to the column mapping, a report to its
/// mappings; the menus hold no separate entry for any of them.
#[test]
fn one_import_goes_on_by_what_the_file_is() {
    let tmp = Tmp::new();
    let dxf = tmp.0.join("plan.dxf");
    std::fs::write(&dxf, "0\nSECTION\n2\nENTITIES\n0\nEOF\n").unwrap();
    let edz = tmp.0.join("psu.edz");
    std::fs::copy(EDZ, &edz).unwrap();
    let xml = tmp.0.join("parts.xml");
    std::fs::write(
        &xml,
        "<partsmanagement><part P_ARTICLE_PARTNR=\"X1\" P_ARTICLE_MANUFACTURER=\"Acme\"/></partsmanagement>",
    )
    .unwrap();
    let parts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/tables/parts.csv");
    let report = tmp.0.join("report.csv");
    std::fs::write(&report, "Terminal strip,Terminal\n-X1,1\n").unwrap();
    let mut h = open_golden(&tmp, &[&dxf, &edz, &xml, &parts, &report]);
    import_menu(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ImportDrawing { .. })
    ));
    support::dispatch(&mut h, "ui.dismiss", json!({}));
    import_menu(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ImportEplan {
            step: EplanStep::Terms,
            ..
        })
    ));
    support::dispatch(&mut h, "ui.dismiss", json!({}));
    import_menu(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ImportEplan {
            step: EplanStep::Parts { .. },
            ..
        })
    ));
    support::dispatch(&mut h, "ui.dismiss", json!({}));
    import_menu(&mut h);
    assert!(
        h.state().shell().parts_map().is_some(),
        "a parts table: its column mapping"
    );
    support::dispatch(&mut h, "ui.dismiss", json!({}));
    import_menu(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::ImportEplan {
            step: EplanStep::Report { .. },
            ..
        })
    ));
    // The separate entries are in no menu.
    support::dispatch(&mut h, "ui.dismiss", json!({}));
    for menu in [["File"], ["Project"]] {
        support::open_menu(&mut h, &menu);
        settle(&mut h);
        for gone in [
            "Parts table…",
            "Import drawing…",
            "Import EPLAN data…",
            "Import QElectroTech project…",
        ] {
            assert!(h.query_by_label(gone).is_none(), "{gone} in {menu:?}");
        }
        support::dispatch(&mut h, "ui.dismiss", json!({}));
    }
}

/// FT-1742: a report that fits no mapping is mapped column by column; Import saves the mapping and makes the project;
/// the next file of the report scheme finds the saved mapping and offers it first.
#[test]
fn the_column_mapper_maps_a_report_and_saves_the_mapping_for_the_next_file() {
    let tmp = Tmp::new();
    let one = tmp.0.join("a").join("klemmen.csv");
    let two = tmp.0.join("b").join("klemmen.csv");
    for f in [&one, &two] {
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, "Title row\nLeiste;Klemme;Stufe\n-X1;1;1\n-X1;2;1\n").unwrap();
    }
    let mut h = open_golden(&tmp, &[&one, &two]);
    import_menu(&mut h);
    let Some(Dialog::ImportEplan {
        step: EplanStep::Report { fits, .. },
        ..
    }) = h.state().shell().dialog().cloned()
    else {
        panic!("the report step");
    };
    assert!(fits.is_empty(), "nothing fits these headers");
    h.get_by_label("Map the columns…").click();
    settle(&mut h);
    let map = h.state().shell().parts_map().expect("the mapping step");
    assert_eq!(
        map.columns.iter().map(|c| c.header.as_str()).collect::<Vec<_>>(),
        ["Leiste", "Klemme", "Stufe"],
        "the header row is found below the title"
    );
    assert!(!map.can_import(), "no column is mapped yet");
    h.get_by_label_contains("Choose what each column of klemmen.csv is");
    support::assert_accessible(&h);
    for (column, member) in [("Leiste", "strip"), ("Klemme", "designation"), ("Stufe", "level")] {
        support::dispatch(&mut h, "parts.map", json!({"column": column, "member": member}));
    }
    settle(&mut h);
    assert!(h.state().shell().parts_map().unwrap().can_import());
    let dir = tmp.0.join("mapped.ew");
    support::dispatch(
        &mut h,
        "parts.map_import",
        json!({"dir": dir.to_string_lossy(), "discard": true}),
    );
    settle(&mut h);
    assert!(dir.join("manifest.json").is_file());
    assert!(h.state().shell().parts_map().is_none());
    assert!(
        tmp.0
            .join("config")
            .join("imports")
            .join("eplan-report-klemmen.toml")
            .is_file()
    );
    // The next file of the scheme: the saved mapping is offered first.
    import_menu(&mut h);
    let Some(Dialog::ImportEplan {
        step: EplanStep::Report { fits, saved },
        ..
    }) = h.state().shell().dialog().cloned()
    else {
        panic!("the report step again: {:?}", h.state().shell().dialog());
    };
    assert!(saved && fits.len() == 1, "{fits:?}");
    h.get_by_label("Import as terminal diagram, with your saved mapping");
    support::assert_accessible(&h);
}
