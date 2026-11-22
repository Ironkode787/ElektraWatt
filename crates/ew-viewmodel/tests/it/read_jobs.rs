// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An import's reading runs as a job of the Jobs pane, off the window's thread (FT-1763): an EDZ read for
//! File ▸ Import… is listed while it runs, the shell answers meanwhile, its Cancel ends it with nothing imported, and
//! its dialog opens at its end. The test waits until the job stands at its gate before it cancels and releases, then
//! for the job's own end: no clock is in it.

use crate::common::{SAMPLE, Tmp, copy_dir, done, library_tiers};

use ew_viewmodel::shell::{ConfirmAnswer, Dialog, EplanStep, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;
use std::path::Path;

const EDZ: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-import-eplan/tests/fixtures/synthetic-psu-7z.edz"
);

/// The kinds of the running jobs.
fn running(s: &ShellModel) -> Vec<String> {
    s.jobs()
        .entries()
        .filter(|e| e.is_running())
        .map(|e| e.kind.word().to_owned())
        .collect()
}

#[test]
fn an_edz_is_read_as_a_job_with_cancel_and_its_dialog_opens_at_its_end() {
    let tmp = Tmp::new("read-job");
    let config =
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test").with_libraries(Some(library_tiers(&tmp.0)));
    let mut s = ShellModel::new(Mode::Gui, config);
    let project = tmp.sample_copy();
    copy_dir(Path::new(SAMPLE), &project);
    done(&mut s, "file.open", json!({"dir": project.to_string_lossy()}));
    let gate = ew_viewmodel::ew_api::ew_jobs::Gate::new();
    gate.hold("read");
    s.set_job_gate_for_tests(Some(gate.clone()));
    let import = json!({"file": EDZ});
    // Held at its start: listed as running, and no dialog yet.
    done(&mut s, "file.import_eplan", import.clone());
    assert_eq!(running(&s), ["read"], "the shell answers while the job runs");
    assert!(s.dialog().is_none(), "{:?}", s.dialog());
    gate.wait_reached("read");
    done(&mut s, "jobs.cancel", json!({}));
    gate.release();
    s.wait_jobs();
    assert!(s.dialog().is_none(), "nothing read: {:?}", s.dialog());
    let toast = s.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(
        toast,
        "The import of synthetic-psu-7z.edz was cancelled; nothing was imported."
    );
    // Read to its end: the EDZ's terms notice opens.
    s.set_job_gate_for_tests(None);
    done(&mut s, "file.import_eplan", import);
    s.wait_jobs();
    assert!(running(&s).is_empty());
    assert!(
        matches!(
            s.dialog(),
            Some(Dialog::ImportEplan {
                step: EplanStep::Terms,
                ..
            })
        ),
        "{:?}",
        s.dialog()
    );
    s.shutdown();
}

/// A report import makes a new project at its read's end and closes the open one, and the window answers while the
/// file is read (FT-1763): an edit made meanwhile is not thrown away unasked. The question comes again at the end, and
/// its Cancel keeps the project with the edit and says the report was not imported.
#[test]
fn an_edit_made_while_a_report_is_read_is_asked_about_and_kept() {
    let tmp = Tmp::new("report-read-edit");
    let mut s = tmp.sample_in(Mode::Gui, true, "101");
    let report = tmp.0.join("klemmen.csv");
    std::fs::write(
        &report,
        "Terminal strip,Terminal,Level,Part number\n-X1,1,1,\n-X1,2,1,\n-X1,2,2,\n",
    )
    .unwrap();
    let file = report.to_string_lossy().into_owned();
    done(&mut s, "file.import_eplan", json!({ "file": file }));
    let Some(Dialog::ImportEplan {
        step: EplanStep::Report { fits, .. },
        ..
    }) = s.dialog().cloned()
    else {
        panic!("the report step: {:?}", s.dialog());
    };
    let gate = ew_viewmodel::ew_api::ew_jobs::Gate::new();
    gate.hold("read");
    s.set_job_gate_for_tests(Some(gate.clone()));
    let dir = tmp.0.join("report.ew");
    let import = json!({"file": file, "how": "report", "preset": fits[0].0, "dir": dir.to_string_lossy()});
    done(&mut s, "file.import_eplan", import);
    done(&mut s, "ui.dismiss", json!({}));
    done(
        &mut s,
        "page.new",
        json!({"folio": "990", "type": "multi_line", "title": "Made while the report was read"}),
    );
    gate.wait_reached("read");
    gate.release();
    s.wait_jobs();
    assert!(
        matches!(s.dialog(), Some(Dialog::ConfirmDiscard { .. })),
        "asked again: {:?}",
        s.dialog()
    );
    assert_eq!(s.answer_confirm(ConfirmAnswer::Cancel), Outcome::Done);
    assert_eq!(s.project_name(), Some("sample.ew"), "not closed unasked");
    assert!(s.page_labels().iter().any(|l| l == "990"), "{:?}", s.page_labels());
    assert!(s.is_dirty() && !dir.exists());
    let toast = s.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(
        toast,
        "The report was not imported: the project changed while it was read and stays open; import the report again to \
         make its project."
    );
    s.shutdown();
}
