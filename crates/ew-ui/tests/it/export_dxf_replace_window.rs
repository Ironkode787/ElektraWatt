// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Export DXF… into the folder of an earlier set asks first (doc 09 §4.9, doc 16a §6.12): the set's files may hold a
//! person's later edits in a CAD program, which undo does not bring back, so the window opens a confirmation whose
//! Primary is Cancel; Cancel leaves the folder as it was, Replace writes the set. At 1920 × 1080.

use crate::support::{Tmp, dispatch, finish_jobs, on_sample, settle};

use egui_kittest::kittest::Queryable as _;
use ew_viewmodel::shell::Dialog;
use serde_json::json;

#[test]
fn exporting_dxf_over_an_earlier_set_asks_and_cancel_keeps_it() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "101");
    h.set_size(egui::Vec2::new(1920.0, 1080.0));
    let dir = tmp.0.join("dxf-set");
    let set = json!({"dir": dir.to_string_lossy()});
    dispatch(&mut h, "file.export_dxf", set.clone());
    finish_jobs(&mut h);
    let page = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "dxf"))
        .expect("a page file");
    std::fs::write(&page, b"edited in a CAD program").unwrap();

    dispatch(&mut h, "file.export_dxf", set.clone());
    settle(&mut h);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::ConfirmReplaceDxf { folder, .. }) if folder == "dxf-set"),
        "the export asks: {:?}",
        h.state().shell().dialog()
    );
    assert!(
        h.query_by_label("Replace the DXF set in dxf-set?").is_some(),
        "the question names the folder"
    );
    h.get_by_label("Cancel").click();
    settle(&mut h);
    finish_jobs(&mut h);
    assert!(h.state().shell().dialog().is_none(), "Cancel closes the question");
    assert_eq!(
        std::fs::read(&page).unwrap(),
        b"edited in a CAD program",
        "Cancel leaves the folder as it was"
    );

    dispatch(&mut h, "file.export_dxf", set);
    settle(&mut h);
    h.get_by_label("Replace").click();
    settle(&mut h);
    finish_jobs(&mut h);
    assert!(h.state().shell().dialog().is_none(), "Replace gives way");
    assert_ne!(
        std::fs::read(&page).unwrap(),
        b"edited in a CAD program",
        "Replace writes the set"
    );
    h.state_mut().shell_mut().shutdown();
}
