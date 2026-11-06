// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.07 part 3: the print pre-dialog of the shell. PW-2 (the set is built as a job: an edit made while it builds
//! is applied at once, the set holds the revision it started from, a cancel leaves no file), the empty state, the
//! Pages field's refusals, the disabled print buttons and the toasts of the window's print, every text whole. PW-1's
//! digest is `bin/elektrawatt`'s `tests/it/print.rs` and `ew-api`'s `export_job.rs`.

use crate::common::{Tmp, done, refused};
use ew_viewmodel::ew_api::ew_jobs::Gate;
use ew_viewmodel::jobs::JobKind;
use ew_viewmodel::shell::print::{PrintEnd, PrintVia};
use ew_viewmodel::shell::{Outcome, ShellModel};
use serde_json::json;

/// The preview's lines of the describe text, and the dialog's own lines before them.
fn dialog_lines(s: &ShellModel) -> Vec<String> {
    let d = s.describe();
    let start = d
        .lines()
        .position(|l| l.trim_start().starts_with("field \"Range\""))
        .expect("the print dialog's lines");
    d.lines()
        .skip(start)
        .take_while(|l| !l.trim_start().starts_with("button"))
        .map(|l| l.trim_start().to_owned())
        .collect()
}

fn failed(s: &mut ShellModel, action: &str, params: serde_json::Value) -> String {
    match s.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

fn toasts(s: &ShellModel) -> Vec<String> {
    s.notifications()
        .visible()
        .iter()
        .map(|n| n.text.clone())
        .collect()
}

/// PR-1's options: Range Sections 1, Paper A5, Scale to fit.
fn section_1_on_a5(wait: bool) -> serde_json::Value {
    json!({"range": "sections", "sections": "1", "paper": "A5", "fit": true, "wait": wait})
}

fn labels(s: &ShellModel) -> Vec<String> {
    s.print_set()
        .expect("a built set")
        .pages
        .iter()
        .map(|p| p.label.clone())
        .collect()
}

/// Waits for the running build to end, as `wait_print` does, but polling a bounded number of times (FT-1120: a join
/// that never returns would hold the run): fails, naming `step`, when the build is still running after 600 polls of
/// 100 ms.
fn wait_print_bounded(s: &mut ShellModel, step: &str) {
    for _ in 0..600 {
        s.poll_jobs();
        if s.print_progress().is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("the print set build did not end: {step}");
}

#[test]
fn pw2_an_edit_while_the_set_builds_is_applied_and_the_set_keeps_its_revision() {
    let tmp = Tmp::new("pw2");
    let mut s = tmp.shell_in(ew_viewmodel::shell::Mode::Headless, true, "100");
    // Held after the writer: a hold stays across releases, so a hold at `page` would stop every page.
    let gate = Gate::new();
    gate.hold("written");
    s.set_job_gate_for_tests(Some(gate.clone()));
    done(&mut s, "file.print", section_1_on_a5(false));
    gate.wait_reached("written");
    assert_eq!(
        dialog_lines(&s).last().unwrap(),
        "preview print \"Writing the print set…\""
    );
    assert_eq!(
        s.print_disabled_reason().as_deref(),
        Some("The print set is still being built")
    );
    assert_eq!(refused(&mut s, "print.send", json!({})), "reason-print-not-built");
    // The build never blocks the window: an edit runs at once.
    done(
        &mut s,
        "page.new",
        json!({"folio": "107", "type": "multi_line", "title": "Made while printing"}),
    );
    assert!(s.page_rows().iter().any(|r| r.folio == "107"), "applied at once");
    gate.release();
    wait_print_bounded(&mut s, "after the release at `written`");
    assert_eq!(
        labels(&s),
        ["100", "101", "102", "103", "106"],
        "the revision it started from"
    );
    s.shutdown();
}

#[test]
fn pw2_cancel_while_the_set_builds_closes_the_dialog_and_leaves_no_file() {
    let tmp = Tmp::new("pw2-cancel");
    let mut s = tmp.shell("100");
    let before = files(&tmp.0);
    let gate = Gate::new();
    gate.hold("page");
    s.set_job_gate_for_tests(Some(gate.clone()));
    done(&mut s, "file.print", section_1_on_a5(false));
    gate.wait_reached("page");
    assert_eq!(
        dialog_lines(&s).last().unwrap(),
        "preview print \"Building the print set… 0 %\""
    );
    // An option changed while it builds cancels that build and starts one for the new options.
    let generation = s.print_dialog().unwrap().generation;
    done(&mut s, "print.set", json!({"mono": true, "wait": false}));
    assert_eq!(s.print_dialog().unwrap().generation, generation + 1);
    assert!(s.print_progress().is_some(), "a new build runs");
    done(&mut s, "ui.dismiss", json!({}));
    gate.release();
    assert!(s.print_dialog().is_none());
    assert!(s.print_progress().is_none(), "the build is cancelled");
    assert!(s.take_print_request().is_none(), "nothing prints");
    assert_eq!(files(&tmp.0), before, "no file");
    s.shutdown();
}

/// Every file under `dir`, sorted.
fn files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_dialog_reads_its_options_and_the_empty_state() {
    let tmp = Tmp::new("print-empty");
    let mut s = tmp.shell("100");
    done(&mut s, "file.print", json!({"range": "sections"}));
    let lines = dialog_lines(&s);
    assert_eq!(lines[0], "field \"Range\": \"Sections\"");
    assert!(
        lines[1..]
            .iter()
            .take_while(|l| l.starts_with("check \"Section"))
            .count()
            >= 2,
        "{lines:?}"
    );
    assert_eq!(
        lines[lines.len() - 4..],
        [
            "field \"Paper\": \"Sheet size\"",
            "check \"Scale to fit\" not ticked",
            "check \"Monochrome\" not ticked",
            "preview \"No pages to print\" \"Tick a section or type the pages to print\"",
        ]
    );
    assert_eq!(s.print_disabled_reason().as_deref(), Some("No pages are chosen"));
    done(&mut s, "print.set", json!({"range": "pages", "pages": ""}));
    assert_eq!(
        dialog_lines(&s)[1..],
        [
            "field \"Pages\": \"\"",
            "field \"Paper\": \"Sheet size\"",
            "check \"Scale to fit\" not ticked",
            "check \"Monochrome\" not ticked",
            "preview \"No pages to print\" \"Tick a section or type the pages to print\"",
        ]
    );
    s.shutdown();
}

#[test]
fn the_pages_field_shows_the_parsers_and_the_page_orders_refusal() {
    let tmp = Tmp::new("print-pages");
    let mut s = tmp.shell("100");
    done(
        &mut s,
        "file.print",
        json!({"range": "pages", "pages": "100,,101"}),
    );
    assert_eq!(
        dialog_lines(&s)[1..3],
        [
            "field \"Pages\": \"100,,101\"",
            "error: The pages could not be read. Type folio labels or ranges, separated by commas.",
        ]
    );
    assert_eq!(
        s.print_disabled_reason().as_deref(),
        Some("\"100,,101\" is not a page range of this project")
    );
    // FT-1124 (ID-1225): the page order's refusal is the same sentence, once, under the field; the preview is empty.
    done(&mut s, "print.set", json!({"pages": "999"}));
    assert_eq!(
        dialog_lines(&s)[1..3],
        [
            "field \"Pages\": \"999\"",
            "error: The pages could not be read. Type folio labels or ranges, separated by commas.",
        ]
    );
    assert_eq!(
        s.print_dialog().map(|d| d.build.clone()),
        Some(ew_viewmodel::shell::print::PrintBuild::Empty)
    );
    // FT-1172, FT-1190: Print… says why in words; the refusal is told by its kind.
    assert_eq!(
        s.print_disabled_reason().as_deref(),
        Some("\"999\" is not a page range of this project")
    );
    done(&mut s, "print.set", json!({"pages": "101-102"}));
    assert_eq!(s.print_page_text().as_deref(), Some("1 of 2"));
    done(&mut s, "print.page", json!({"to": "next"}));
    assert_eq!(s.print_page_text().as_deref(), Some("2 of 2"));
    done(&mut s, "print.page", json!({"to": "next"}));
    assert_eq!(s.print_page_text().as_deref(), Some("2 of 2"), "clamped");
    assert_eq!(
        failed(
            &mut s,
            "print.set",
            json!({"range": "sections", "sections": "99"})
        ),
        "no section `99` (the sections are 1, 2, 9)"
    );
    s.shutdown();
}

#[test]
fn print_send_hands_the_set_to_the_window_and_each_end_has_one_toast() {
    let tmp = Tmp::new("print-send");
    let mut s = tmp.shell("100");
    assert_eq!(refused(&mut s, "print.send", json!({})), "reason-no-print-dialog");
    done(&mut s, "file.print", json!({"range": "current", "mono": true}));
    assert_eq!(s.print_disabled_reason(), None);
    done(&mut s, "print.send", json!({"via": "viewer"}));
    assert!(s.print_dialog().is_none(), "the dialog closes");
    let request = s.take_print_request().expect("a request");
    assert_eq!(request.via, PrintVia::Viewer);
    assert!(request.monochrome);
    assert_eq!(request.title, "J-E-2416");
    assert_eq!(request.set.pages.len(), 1);
    assert!(s.take_print_request().is_none(), "taken once");
    let cases = [
        (PrintEnd::Printed, "Sent J-E-2416 to the printer."),
        (PrintEnd::Opened, "Opened J-E-2416 in your PDF viewer."),
        (
            PrintEnd::FellBackToPrinter,
            "Printing through the print dialog failed, so J-E-2416 went to the default printer.",
        ),
        (
            PrintEnd::FellBackToViewer,
            "Printing could not start, so J-E-2416 opened in your PDF viewer to print from there.",
        ),
        (
            PrintEnd::Failed("`lp` failed with exit status: 1".to_owned()),
            "Printing failed: `lp` failed with exit status: 1. Export a PDF and print it from your PDF viewer.",
        ),
        (
            PrintEnd::Failed("`lp` said: no default destination.".to_owned()),
            "Printing failed: `lp` said: no default destination. Export a PDF and print it from your PDF viewer.",
        ),
        // FT-1192: a cause that ends in an ellipsis keeps it whole.
        (
            PrintEnd::Failed("`lp` said: printer busy...".to_owned()),
            "Printing failed: `lp` said: printer busy... Export a PDF and print it from your PDF viewer.",
        ),
        (
            PrintEnd::Unavailable,
            "Printing is not available in this window. Export a PDF and print it from your PDF viewer.",
        ),
    ];
    for (end, text) in cases {
        done(&mut s, "file.print", json!({"range": "current"}));
        done(&mut s, "print.send", json!({}));
        let _ = s.take_print_request().expect("a request");
        s.print_done(end);
        assert_eq!(s.toast().map(|t| t.text.as_str()), Some(text));
    }
    let before = toasts(&s);
    done(&mut s, "file.print", json!({"range": "current"}));
    done(&mut s, "print.send", json!({}));
    let _ = s.take_print_request();
    s.print_done(PrintEnd::Cancelled);
    assert_eq!(toasts(&s), before, "a cancel says nothing");
    s.shutdown();
}

/// C52-p3#3 (FT-1157, ID-1417): the print the window hands to its platform is a running entry of the Jobs pane; its
/// Cancel sets the request's cancel signal, which the platform hands to the print job, and the print's end takes the
/// entry away.
#[test]
fn a_running_print_is_a_job_whose_cancel_reaches_the_print() {
    use std::sync::atomic::Ordering::Relaxed;
    let tmp = Tmp::new("print-cancel");
    let mut s = tmp.shell("100");
    done(&mut s, "file.print", json!({"range": "current"}));
    done(&mut s, "print.send", json!({}));
    let request = s.take_print_request().expect("a request");
    let running: Vec<(JobKind, String, bool)> = s
        .jobs()
        .entries()
        .map(|e| (e.kind, e.name.clone(), e.is_running()))
        .collect();
    assert_eq!(running, [(JobKind::Print, "J-E-2416".to_owned(), true)]);
    assert!(!request.cancel.load(Relaxed));
    done(&mut s, "jobs.cancel", json!({}));
    assert!(request.cancel.load(Relaxed), "the Cancel reaches the print");
    s.print_done(PrintEnd::Cancelled);
    assert_eq!(s.jobs().entries().count(), 0, "the end takes the entry away");
    assert_eq!(refused(&mut s, "jobs.cancel", json!({})), "reason-no-running-job");
    s.shutdown();
}

/// C52-p6#2 (ID-1424): a way that hands a file over cannot take it back: a print cancelled in the Jobs pane that the
/// printer or the viewer already had says so in words, in English and Dutch, rather than telling it as sent.
#[test]
fn a_cancel_too_late_for_a_handed_over_print_says_where_it_went() {
    let tmp = Tmp::new("print-cancel-late");
    let mut s = tmp.shell("100");
    let cases = [
        (
            "en",
            PrintEnd::Printed,
            "The printer already had J-E-2416 when you cancelled; cancel it in the system's print queue.",
        ),
        (
            "en",
            PrintEnd::FellBackToViewer,
            "J-E-2416 was already open in your PDF viewer when you cancelled; close it there.",
        ),
        (
            "nl",
            PrintEnd::FellBackToPrinter,
            "De printer had J-E-2416 al toen u annuleerde; annuleer het in de afdrukwachtrij van het systeem.",
        ),
        (
            "nl",
            PrintEnd::Opened,
            "J-E-2416 was al geopend in uw PDF-viewer toen u annuleerde; sluit het daar.",
        ),
    ];
    for (language, end, text) in cases {
        done(&mut s, "view.language", json!({ "value": language }));
        done(&mut s, "file.print", json!({"range": "current"}));
        done(&mut s, "print.send", json!({}));
        let _ = s.take_print_request().expect("a request");
        done(&mut s, "jobs.cancel", json!({}));
        s.print_done(end);
        assert_eq!(s.toast().map(|t| t.text.as_str()), Some(text));
    }
    s.shutdown();
}
