// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.07 part 3, PW-1 headless: `file.print` with Range Sections 1, Paper A5 and Scale to fit builds the print set
//! of PR-1 (its PDF is `golden/J-E-2416/expected/print/sha256.txt`), the preview's page 1 of 5 shows the stamp, and
//! Escape (`ui.dismiss`) closes the dialog and prints nothing. The held-build flows (PW-2) are `ew-viewmodel`'s
//! `tests/it/print.rs`. Golden A itself is only copied, never written.

use std::path::Path;

use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::support::{GOLDEN_A, Tmp, copy_dir, path};

const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

#[test]
fn the_print_dialog_builds_pr1s_set_and_escape_prints_nothing() {
    let tmp = Tmp::new();
    let dir = tmp.path("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
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
    let digest: String = Sha256::digest(&set.pdf)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let expected = std::fs::read_to_string(Path::new(EXPECTED).join("print/sha256.txt")).unwrap();
    assert_eq!(digest, expected.trim(), "PR-1's print set");
    assert_eq!(s.print_page_text().as_deref(), Some("1 of 5"));
    let d = s.describe();
    let preview: Vec<&str> = d
        .lines()
        .filter(|l| l.trim_start().starts_with("preview "))
        .collect();
    assert_eq!(preview, ["  preview \"1 of 5\" page 100 stamp \"scaled 70 %\""]);
    run(&mut s, "ui.dismiss", json!({}));
    assert!(s.print_dialog().is_none(), "Escape closes the dialog");
    assert!(s.take_print_request().is_none(), "and prints nothing");
    s.shutdown();
}
