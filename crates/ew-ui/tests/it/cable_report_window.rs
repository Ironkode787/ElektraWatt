// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1371 and FT-1372 (ID-1362) in the window (kittest, no rendering): the app menu's Draw ▸ Connect ▸ New cable…
//! opens its form, whose part list and Create make the cable; Output ▸ Report block… opens its form, whose Create adds
//! the block's pages. Each walk ends with the accessible-name walker (`settle`).

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_ui::ew_viewmodel::shell::{Dialog, ShellModel};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

/// Runs the frame until `done` holds of the shell (600 frames, 20 ms apart), then settles.
#[track_caller]
fn wait_until(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, what: &str, done: impl Fn(&ShellModel) -> bool) {
    for _ in 0..600 {
        h.run_ok();
        if done(h.state().shell()) {
            settle(h);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("{what} did not happen (dialog {:?})", h.state().shell().dialog());
}

/// The text of the toast on show.
fn toast(shell: &ShellModel) -> String {
    let state: serde_json::Value =
        serde_json::from_str(&ew_ui::ew_viewmodel::script::state_json(shell)).unwrap();
    state["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn new_cable_and_report_block_open_their_forms_from_the_menus_and_create() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    let rev = h.state().shell().revision();
    support::open_menu(&mut h, &["Draw", "Connect"]);
    settle(&mut h);
    h.get_by_label("New cable…").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "cable.new"));
    h.get_by_label("Create").click();
    wait_until(&mut h, "the cable form's close", |s| s.dialog().is_none());
    assert_ne!(h.state().shell().revision(), rev, "one step committed");
    assert!(toast(h.state().shell()).starts_with("Created cable -"), "{}", toast(h.state().shell()));
    let pages = h.state().shell().page_labels().len();
    support::open_menu(&mut h, &["Output"]);
    settle(&mut h);
    h.get_by_label("Report block…").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "report_block.new"));
    h.get_by_label("Create").click();
    wait_until(&mut h, "the report form's close", |s| s.dialog().is_none());
    assert!(h.state().shell().page_labels().len() > pages, "the block's pages join the page order");
    h.state_mut().shell_mut().shutdown();
}
