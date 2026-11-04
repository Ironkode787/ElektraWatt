// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start-up dialog and the bundled `PRIVACY.md` in the window (WP-S6.05 D11, D16; PV-6's window half, PV-10): the
//! offer of an earlier session's crash log with its buttons, and the privacy buttons opening the bundled copy.

use crate::support;

use egui::Key;
use egui_kittest::kittest::Queryable;
use ew_ui::ew_viewmodel::shell::startup::{CRASH_DIR, ENDED_LINE};
use ew_ui::ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use support::{Scripted, Tmp, harness_with, settle};

/// A crash log of a session whose panic ended it, at 2026-11-04 10:15 UTC.
fn crash_log(tmp: &Tmp) -> std::path::PathBuf {
    let dir = tmp.0.join("config").join("state").join(CRASH_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("20261104T101500Z-42.log");
    std::fs::write(
        &path,
        format!("ElektraWatt 0.0.0-test crash log\nTime: 2026-11-04T10:15:07Z\nBacktrace:\n   0: main\n\n{ENDED_LINE}\n"),
    )
    .unwrap();
    path
}

fn started(config: ShellConfig) -> ShellModel {
    let mut shell = ShellModel::new(Mode::Gui, config);
    shell.start_up();
    shell
}

#[test]
fn the_start_up_dialog_offers_the_crash_log() {
    let tmp = Tmp::new();
    let log = crash_log(&tmp);
    let platform = Scripted::default();
    let mut h = harness_with(started(tmp.config()), platform.clone(), None);
    settle(&mut h);
    let folder = log.parent().unwrap().to_string_lossy().into_owned();
    assert!(h.query_by_label("ElektraWatt stopped unexpectedly").is_some());
    assert!(
        h.query_by_label(&format!(
            "ElektraWatt stopped unexpectedly on 2026-11-04 10:15 UTC; no project had unsaved changes. Its log is in \
             {folder}."
        ))
        .is_some()
    );
    // Open log folder hands the folder to the system's file manager; the offer stays.
    h.get_by_label("Open log folder").click();
    settle(&mut h);
    assert_eq!(platform.opened.borrow().len(), 1);
    assert!(platform.opened.borrow()[0].starts_with("file://"));
    assert!(h.state().shell().startup_offer().is_some());
    // Escape is Not now: the offer closes and the log is renamed, so it is never offered again.
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(h.state().shell().startup_offer().is_none());
    assert!(!log.exists());
    assert!(log.with_file_name("20261104T101500Z-42.offered.log").exists());
}

#[test]
fn delete_log_deletes_the_log_and_closes_the_dialog() {
    let tmp = Tmp::new();
    let log = crash_log(&tmp);
    let mut h = harness_with(started(tmp.config()), Scripted::default(), None);
    settle(&mut h);
    h.get_by_label("Delete log").click();
    settle(&mut h);
    assert!(h.state().shell().startup_offer().is_none());
    assert!(!log.exists());
    assert!(h.query_by_label("ElektraWatt stopped unexpectedly").is_none());
}

/// PV-10: with `PRIVACY.md` in the installation, About's and the first-run step's privacy buttons open it locally (the
/// fall-back to the repository's copy when there is none is `part2.rs`'s).
#[test]
fn the_privacy_buttons_open_the_bundled_privacy_md() {
    let tmp = Tmp::new();
    let file = tmp.0.join("PRIVACY.md");
    std::fs::write(&file, "# Privacy\n").unwrap();
    let mut resources = tmp.config_with_resources().resources;
    resources.privacy_file = Some(file);
    let config = tmp.config().with_resources(resources);
    let shell = ShellModel::new(Mode::Gui, config.clone());
    let target = ew_ui::ew_viewmodel::about::privacy_target(&shell);
    assert!(
        target.starts_with("file://") && target.ends_with("/PRIVACY.md"),
        "{target}"
    );
    let platform = Scripted::default();
    let mut h = harness_with(shell, platform.clone(), None);
    settle(&mut h);
    support::open_menu(&mut h, &["Help"]);
    settle(&mut h);
    h.get_by_label("About ElektraWatt").click();
    settle(&mut h);
    h.get_by_label("Privacy (PRIVACY.md)").click();
    settle(&mut h);
    assert_eq!(*platform.opened.borrow(), [target.clone()]);

    let mut shell = ShellModel::new(Mode::Gui, config);
    shell.start_first_run_if_needed();
    let platform = Scripted::default();
    let mut h = harness_with(shell, platform.clone(), None);
    settle(&mut h);
    h.get_by_label("Next").click();
    settle(&mut h);
    h.get_by_label("Skip").click();
    settle(&mut h);
    h.get_by_label("Privacy (PRIVACY.md)").click();
    settle(&mut h);
    assert_eq!(*platform.opened.borrow(), [target]);
}
