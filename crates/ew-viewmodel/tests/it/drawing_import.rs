// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawing imported through `file.import_drawing` on a writable copy of the sample (WP-S3.19, doc 14 §13): the import
//! runs as a job of the Jobs pane that Cancel ends with nothing imported, and a drawing added as a page takes a folio no
//! page has. Not here: File ▸ Import…, its dialog and its toasts in the window (`ew-ui`'s `drawing_import.rs`).

use crate::common::{Tmp, done};

use ew_viewmodel::shell::Mode;
use serde_json::json;

/// A drawing without a unit: one rectangle.
const PLAN: &str = "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\n0\n90\n4\n70\n1\n10\n20\n20\n20\n10\n120\n20\n20\n\
10\n120\n20\n70\n10\n20\n20\n70\n0\nENDSEC\n0\nEOF\n";

/// FT-1753: a drawing import runs as a job of the Jobs pane, off the window's thread: held at its start, it is listed
/// as running, and its Cancel ends it with nothing imported and the cancel said in words. The test waits until the job
/// stands at its gate before it cancels and releases (a release before the job arrived left it held for good on a
/// loaded machine), then for the job's own end: no clock is in it.
#[test]
fn a_drawing_import_runs_as_a_job_with_cancel() {
    let tmp = Tmp::new("drawing-cancel");
    let dxf = tmp.0.join("plan.dxf");
    std::fs::write(&dxf, PLAN).unwrap();
    let mut s = tmp.sample_in(Mode::Gui, true, "101");
    let pages = s.page_rows().len();
    let gate = ew_viewmodel::ew_api::ew_jobs::Gate::new();
    gate.hold("drawing");
    s.set_job_gate_for_tests(Some(gate.clone()));
    done(
        &mut s,
        "file.import_drawing",
        json!({"file": dxf.to_string_lossy(), "as": "page", "units": "mm"}),
    );
    let running: Vec<String> = s
        .jobs()
        .entries()
        .filter(|e| e.is_running())
        .map(|e| e.kind.word().to_owned())
        .collect();
    assert_eq!(running, ["import"], "the shell answers while the job runs");
    gate.wait_reached("drawing");
    done(&mut s, "jobs.cancel", json!({}));
    gate.release();
    s.wait_jobs();
    assert_eq!(s.page_rows().len(), pages, "nothing imported");
    let toast = s.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(
        toast,
        "The import of plan.dxf was cancelled; nothing was imported."
    );
    s.shutdown();
}

/// On the sample, whose last drawn page is followed by generated report pages, a drawing imported as a page takes a folio
/// no page has, after every page of its section.
#[test]
fn a_drawing_page_takes_a_folio_no_page_has() {
    let tmp = Tmp::new("drawing-folio");
    let dxf = tmp.0.join("plan.dxf");
    std::fs::write(&dxf, PLAN).unwrap();
    let mut s = tmp.sample_in(Mode::Gui, true, "101");
    let before: Vec<String> = s.page_rows().iter().map(|r| r.folio.clone()).collect();
    done(
        &mut s,
        "file.import_drawing",
        json!({"file": dxf.to_string_lossy(), "as": "page", "units": "mm"}),
    );
    s.wait_jobs();
    let shown = s.current_folio().map(str::to_owned).unwrap();
    assert!(!before.contains(&shown), "{shown} is a new folio: {before:?}");
    let rows = s.page_rows();
    assert_eq!(
        rows.iter().filter(|r| r.folio == shown).count(),
        1,
        "one page {shown}"
    );
    s.shutdown();
}
