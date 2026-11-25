// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless: `file.print` with Range Sections 1, Paper A5 and Scale to fit builds the print set
//! of the print set (its PDF is `ew print --sections 1 --paper A5 --fit`'s), the preview's page 1 of 6 shows the stamp, and
//! Escape (`ui.dismiss`) closes the dialog and prints nothing. The held-build flows are `ew-viewmodel`'s
//! `tests/it/print.rs`. The sample is only copied, never written.

use std::path::Path;

use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

use crate::support::{SAMPLE, Tmp, copy_dir, path};

#[test]
fn the_print_dialog_builds_pr1s_set_and_escape_prints_nothing() {
    let tmp = Tmp::new();
    let dir = tmp.path("sample.ew");
    copy_dir(Path::new(SAMPLE), &dir);
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.path("config"), "0.0.0-test"),
    );
    let run = |s: &mut ShellModel, action: &str, params: serde_json::Value| {
        assert_eq!(s.dispatch(action, &params), Outcome::Done, "{action} {params}");
    };
    run(&mut s, "file.open", json!({"dir": path(&dir), "read_only": true}));
    run(
        &mut s,
        "file.print",
        json!({"range": "sections", "sections": "1", "paper": "A5", "fit": true}),
    );
    let set = s.print_set().expect("the set is built").clone();
    assert!(
        set.pdf == crate::support::exported::section_1_on_a5(&dir),
        "the print set is `ew print`'s"
    );
    // Section 1: its divider 100 and the folios 101 to 105.
    assert_eq!(s.print_page_text().as_deref(), Some("1 of 6"));
    let d = s.describe();
    let preview: Vec<&str> = d
        .lines()
        .filter(|l| l.trim_start().starts_with("preview "))
        .collect();
    assert_eq!(preview, ["  preview \"1 of 6\" page 100 stamp \"scaled 70 %\""]);
    run(&mut s, "ui.dismiss", json!({}));
    assert!(s.print_dialog().is_none(), "Escape closes the dialog");
    assert!(s.take_print_request().is_none(), "and prints nothing");
    s.shutdown();
}
