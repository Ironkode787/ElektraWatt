// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! JB2: the PDF export as a job on a temporary copy of the sample project. Its bytes equal
//! `Session::export_pdf`'s; a cancel at the held gate `page` after page 2 ends it
//! `Cancelled`; a job made before an edit exports the earlier revision (snapshot isolation). The job runs on a thread
//! of `ew_jobs`, so the snapshot crosses threads and no `Session` call happens there.

use ew_api::ew_jobs::{Gate, Job, JobEnd, JobOptions, Progress};
use ew_api::output::ExportOptions;
use ew_api::{AnyCommand, Recovery, Session, SessionOptions, StoreOptions};
use ew_testdir::Tmp;

/// An editing session on a copy of the sample (the committed project is only read, never opened).
fn sample_copy(tmp: &Tmp) -> Session {
    let dir = tmp.sample("sample.ew");
    let mut store = StoreOptions::new("0.0.0-test");
    store.state_dir = Some(tmp.0.join("state"));
    let (session, report) = Session::open(&dir, store, SessionOptions::default(), Recovery::Keep).unwrap();
    assert!(report.recovery.is_none());
    session
}

fn pdf_job(name: &str, gate: Option<Gate>, job: ew_api::output::ExportJob) -> Job<ew_api::output::PdfExport> {
    Job::spawn(
        JobOptions {
            name: name.to_owned(),
            waker: None,
            gate,
        },
        move |ctl| job.run(ctl),
    )
    .unwrap()
}

#[test]
fn the_export_job_equals_the_synchronous_export_and_the_digest_and_keeps_its_snapshot() {
    let tmp = Tmp::new("ew-api-job-bytes");
    let mut s = sample_copy(&tmp);
    let sync = s.export_pdf(&ExportOptions::default()).unwrap();
    let labels = s.page_labels().len();
    // The job is made now and run after an edit: it exports this revision.
    let job = s.export_job(&ExportOptions::default());
    let rev = s.world().rev;
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({"number": 999, "title": "After the job was made"}),
    })
    .unwrap();
    assert_ne!(s.world().rev, rev);
    assert_eq!(s.page_labels().len(), labels + 1, "the edit added a page");
    let end = pdf_job("pdf", None, job).wait();
    let JobEnd::Done(out) = end else { panic!("{end:?}") };
    assert_eq!(
        out.pages,
        labels,
        "the snapshot's pages, not the edited model's"
    );
    assert!(
        out.bytes == sync.bytes,
        "the job's bytes are the synchronous export's"
    );
    assert_eq!(out.report, sync.report);
    assert_eq!(out.findings, sync.findings);
    s.close();
}

#[test]
fn a_cancel_at_the_held_page_gate_after_page_2_is_cancelled() {
    let tmp = Tmp::new("ew-api-job-cancel");
    let s = sample_copy(&tmp);
    let pages = u32::try_from(s.page_labels().len()).unwrap();
    let gate = Gate::new();
    gate.hold("page");
    let job = pdf_job("pdf", Some(gate.clone()), s.export_job(&ExportOptions::default()));
    gate.wait_reached("page");
    assert_eq!(
        job.progress(),
        Progress {
            done: 0,
            total: pages,
            phase: "building"
        },
        "page 1's scene is built"
    );
    gate.release();
    gate.wait_reached("page");
    assert_eq!(job.progress().done, 1, "page 2's scene is built");
    job.cancel();
    gate.release();
    assert_eq!(job.wait(), JobEnd::Cancelled);
    s.close();
}

#[test]
fn a_selection_the_page_order_does_not_have_fails_with_its_cause() {
    let tmp = Tmp::new("ew-api-job-fail");
    let s = sample_copy(&tmp);
    let options = ExportOptions {
        pages: ew_api::output::PageSelection::parse("nope").unwrap(),
        ..ExportOptions::default()
    };
    let end = pdf_job("pdf", None, s.export_job(&options)).wait();
    match end {
        JobEnd::Failed(cause) => {
            assert!(cause.starts_with("page selection:"), "{cause}");
            assert!(!cause.contains(&*tmp.0.to_string_lossy()), "no path: {cause}");
        }
        other => panic!("{other:?}"),
    }
    s.close();
}

// The print set as a job.

/// The set: section 1 on A5, scaled to fit.
fn section_1_on_a5() -> ew_api::output::print::PrintOptions {
    ew_api::output::print::PrintOptions {
        pages: ew_api::output::PageSelection::parse_sections("1").unwrap(),
        paper: ew_api::output::print::Paper::A5,
        fit: ew_api::output::print::Fit::ToPaper,
        ..Default::default()
    }
}

fn print_job(gate: Option<Gate>, job: ew_api::output::PrintSetJob) -> Job<ew_api::output::print::PrintSet> {
    Job::spawn(
        JobOptions {
            name: "print".to_owned(),
            waker: None,
            gate,
        },
        move |ctl| job.run(ctl),
    )
    .unwrap()
}

/// At the API: the job's set is `Session::print_set`'s, and a job made before an edit builds the
/// revision it was made on.
#[test]
fn the_print_job_builds_the_print_set_of_its_revision() {
    let tmp = Tmp::new("ew-api-job-print");
    let mut s = sample_copy(&tmp);
    let sync = s.print_set(&section_1_on_a5()).unwrap();
    let job = s.print_set_job(&section_1_on_a5());
    assert_eq!(job.rev(), s.world().rev.0);
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({"number": 150, "title": "Made after the print job"}),
    })
    .unwrap();
    let end = print_job(None, job).wait();
    let JobEnd::Done(set) = end else { panic!("{end:?}") };
    let labels: Vec<&str> = set.pages.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(
        labels,
        ["100", "101", "102", "103", "104", "105"],
        "the snapshot's section 1"
    );
    assert!(set == sync, "the job's set is the synchronous set");
    s.close();
}

/// At the API: a cancel at the held gate ends the print job `Cancelled`.
#[test]
fn a_cancelled_print_job_is_cancelled() {
    let tmp = Tmp::new("ew-api-job-print-cancel");
    let s = sample_copy(&tmp);
    let gate = Gate::new();
    gate.hold("page");
    let job = print_job(Some(gate.clone()), s.print_set_job(&section_1_on_a5()));
    gate.wait_reached("page");
    assert_eq!(
        job.progress(),
        Progress {
            done: 0,
            total: 6,

            phase: "building"
        }
    );
    job.cancel();
    gate.release();
    assert_eq!(job.wait(), JobEnd::Cancelled);
    s.close();
}
