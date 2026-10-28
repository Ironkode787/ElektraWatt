// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! JB2 (WP-S2.09 part 1, D4, §6.2): the PDF export as a job on a temporary copy of Golden A. Its bytes equal
//! `Session::export_pdf`'s and the committed digest; a cancel at the held gate `page` after page 2 ends it
//! `Cancelled`; a job made before an edit exports the earlier revision (snapshot isolation). The job runs on a thread
//! of `ew_jobs`, so the snapshot crosses threads and no `Session` call happens there.

use std::path::{Path, PathBuf};

use ew_api::ew_jobs::{Gate, Job, JobEnd, JobOptions, Progress};
use ew_api::output::ExportOptions;
use ew_api::{AnyCommand, Recovery, Session, SessionOptions, StoreOptions};
use sha2::{Digest, Sha256};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const DIGEST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../golden/J-E-2416/expected/pdf/sha256.txt"
);

/// A temporary directory, removed on drop.
struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Tmp {
        let dir = std::env::temp_dir().join(format!("ew-api-job-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// An editing session on a copy of Golden A (the committed project is only read, never opened).
fn golden_copy(tmp: &Tmp) -> Session {
    let dir = tmp.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut store = StoreOptions::new("0.0.0-test");
    store.state_dir = Some(tmp.0.join("state"));
    let (session, report) = Session::open(&dir, store, SessionOptions::default(), Recovery::Keep).unwrap();
    assert!(report.recovery.is_none());
    session
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
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
fn jb2_the_export_job_equals_the_synchronous_export_and_the_digest_and_keeps_its_snapshot() {
    let tmp = Tmp::new("bytes");
    let mut s = golden_copy(&tmp);
    let sync = s.export_pdf(&ExportOptions::default()).unwrap();
    let expected = std::fs::read_to_string(DIGEST).unwrap();
    assert_eq!(
        sha256_hex(&sync.bytes),
        expected.trim(),
        "the synchronous export is the golden PDF"
    );
    // The job is made now and run after an edit: it exports this revision.
    let job = s.export_job(&ExportOptions::default());
    let rev = s.world().rev;
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({"number": 999, "title": "After the job was made"}),
    })
    .unwrap();
    assert_ne!(s.world().rev, rev);
    assert_eq!(s.page_labels().len(), 37, "the edit added a page");
    let end = pdf_job("pdf", None, job).wait();
    let JobEnd::Done(out) = end else { panic!("{end:?}") };
    assert_eq!(out.pages, 36, "the snapshot's pages, not the edited model's");
    assert!(
        out.bytes == sync.bytes,
        "the job's bytes are the synchronous export's"
    );
    assert_eq!(out.report, sync.report);
    assert_eq!(out.findings, sync.findings);
    s.close();
}

#[test]
fn jb2_a_cancel_at_the_held_page_gate_after_page_2_is_cancelled() {
    let tmp = Tmp::new("cancel");
    let s = golden_copy(&tmp);
    let gate = Gate::new();
    gate.hold("page");
    let job = pdf_job("pdf", Some(gate.clone()), s.export_job(&ExportOptions::default()));
    gate.wait_reached("page");
    assert_eq!(
        job.progress(),
        Progress {
            done: 0,
            total: 36,
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
fn jb2_a_selection_the_page_order_does_not_have_fails_with_its_cause() {
    let tmp = Tmp::new("fail");
    let s = golden_copy(&tmp);
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
