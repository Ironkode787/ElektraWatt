// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.09 part 1 in the window (§10.1): the save dialog's answer replaces an existing file through the export job
//! (JB4); a running export is a named progress indicator with Cancel and, after 200 ms, the status bar's job field
//! (JB3); the toasts stack and the notifications log opens from the status bar (JB6); the recovery dialog (JB7); each
//! followed by the accessible-name walker (JB9).

mod support;

use std::time::Duration;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::{App, Pick};
use ew_ui::ew_viewmodel::ew_api::ew_jobs::Gate;
use ew_ui::ew_viewmodel::shell::{AccessMode, Dialog, Mode, Outcome, Prompt, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, harness_with, settle};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// Frames until no job runs (bounded); the job's thread wakes the window, the frame takes its end.
fn run_until_jobs_end(h: &mut Harness<'_, App>) {
    for _ in 0..3000 {
        h.run_ok();
        if h.state().shell().jobs().running() == 0 {
            settle(h);
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the job did not end");
}

#[test]
fn jb4_the_save_dialogs_answer_replaces_an_existing_file_through_a_job() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let pdf = tmp.0.join("out.pdf");
    std::fs::write(&pdf, b"an older file").unwrap();
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(pdf.clone()));
    let mut h = harness(&tmp, platform.clone(), Some(dir));
    settle(&mut h);
    assert!(h.state().shell().has_project());
    // The save dialog asked about the existing file itself: its answer carries `replace`, nothing is refused.
    dispatch(&mut h, "file.export_pdf", Value::Null);
    run_until_jobs_end(&mut h);
    assert_eq!(platform.asked.borrow().as_slice(), [Prompt::SavePdf]);
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "the file was replaced");
    let kib = bytes.len().div_ceil(1024);
    let text = format!("Exported out.pdf: 36 pages, {kib} KiB");
    assert!(h.query_by_label(&text).is_some(), "the toast");
    assert!(
        h.query_by_label("Notifications 1").is_some(),
        "the status bar's field"
    );
    // The Jobs pane keeps the result with Open.
    dispatch(&mut h, "jobs.show", json!({}));
    settle(&mut h);
    assert!(h.query_by_label(&format!("36 pages, {kib} KiB")).is_some());
    // Open PDF: the toast's and the pane's buttons both hand the file to the system's viewer.
    let opens = h.get_all_by_label("Open").count();
    assert_eq!(opens, 2, "the toast's and the pane's");
    h.get_all_by_label("Open").next().unwrap().click();
    settle(&mut h);
    let opened = platform.opened.borrow().clone();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(
        opened[0].starts_with("file://") && opened[0].ends_with("/out.pdf"),
        "{opened:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn jb3_a_running_export_is_a_named_progress_indicator_with_cancel_and_the_job_field() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let pdf = tmp.0.join("held.pdf");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    let gate = Gate::new();
    gate.hold("page");
    shell.set_job_gate_for_tests(Some(gate.clone()));
    let mut h = harness_with(shell, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "jobs.show", json!({}));
    dispatch(
        &mut h,
        "file.export_pdf",
        json!({ "path": pdf.to_string_lossy() }),
    );
    gate.wait_reached("page");
    settle(&mut h);
    let bar = h
        .get_all_by_role(egui::Role::ProgressIndicator)
        .find(|n| n.accesskit_node().label().as_deref() == Some("PDF export held.pdf"))
        .expect("the running entry is a named progress indicator");
    assert_eq!(bar.accesskit_node().numeric_value(), Some(0.0));
    // The job field shows once the job is 200 ms old (a job that ends sooner shows none).
    std::thread::sleep(Duration::from_millis(250));
    settle(&mut h);
    assert!(h.query_by_label("PDF export 0 %").is_some(), "the job field");
    h.get_by_label("Cancel").click();
    settle(&mut h);
    gate.release();
    run_until_jobs_end(&mut h);
    assert!(!pdf.exists());
    assert!(h.query_by_label("Export cancelled; no file written").is_some());
    assert!(
        h.query_by_label("cancelled; no file written").is_some(),
        "the pane's entry"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn jb6_the_toasts_stack_and_the_log_opens_from_the_status_bar() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(
        &mut h,
        "select.set",
        json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(
        &mut h,
        "props.set",
        json!({"row": "device/function_text", "value": "Spare"}),
    );
    dispatch(
        &mut h,
        "file.export_pdf",
        json!({ "path": tmp.0.join("a.pdf").to_string_lossy() }),
    );
    run_until_jobs_end(&mut h);
    assert_eq!(
        h.state().shell().notifications().visible().len(),
        2,
        "two keys, two toasts"
    );
    assert!(h.query_by_label("Set Function text on 2 devices").is_some());
    assert!(h.query_by_label_contains("Exported a.pdf").is_some());
    h.get_by_label("Notifications 2").click();
    settle(&mut h);
    assert!(h.state().shell().notifications().popup_open());
    assert!(h.query_by_label("Notifications").is_some(), "the log's title");
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(
        !h.state().shell().notifications().popup_open(),
        "Escape closes it"
    );
    // A toast's Close takes it off the screen; the log keeps it.
    let before = h.state().shell().notifications().visible().len();
    h.get_all_by_label("Close")
        .next()
        .expect("a toast's Close")
        .click();
    settle(&mut h);
    assert_eq!(h.state().shell().notifications().visible().len(), before - 1);
    assert_eq!(h.state().shell().notifications().len(), 2);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn jb7_the_recovery_dialog_offers_recover_open_read_only_and_discard() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    {
        let mut crashed = ShellModel::new(Mode::Gui, tmp.config());
        assert_eq!(
            crashed.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() })),
            Outcome::Done
        );
        assert_eq!(
            crashed.dispatch(
                "command.run",
                &json!({"cmd": "page.create", "params": {"number": 996}})
            ),
            Outcome::Done
        );
        // A dirty end without a clean close: the journal stays.
        crashed.shutdown();
    }
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    assert!(matches!(
        h.state().shell().dialog(),
        Some(Dialog::Recovery { .. })
    ));
    assert!(h.query_by_label("Recover unsaved changes").is_some());
    assert!(
        h.query_by_label("g.ew has unsaved changes from a session that ended unexpectedly.")
            .is_some()
    );
    assert!(h.query_by_label("Unsaved changes: 1").is_some());
    // FT-503: the elapsed time (the journal was written moments ago), the UTC stamp only on hover.
    assert!(h.query_by_label("Last change: just now").is_some());
    assert!(h.query_by_label_contains(" UTC").is_none());
    assert!(h.query_by_label("Open read-only").is_some());
    assert!(h.query_by_label("Discard...").is_some());
    h.get_by_label("Recover").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(shell.dialog(), None);
    assert_eq!(shell.access(), Some(AccessMode::Edit));
    assert!(shell.is_dirty());
    assert!(shell.page_labels().iter().any(|l| l == "996"));
    assert!(h.query_by_label("Recovered 1 change; save to keep it").is_some());
    h.state_mut()
        .shell_mut()
        .dispatch("file.close", &json!({"discard": true}));
    h.state_mut().shell_mut().shutdown();
}
