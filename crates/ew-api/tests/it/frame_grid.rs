// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The columns and rows of the sample's frame set to 10 numbered from 1 and 6 lettered from A (doc 09 §3.9, the
//! journey's step 5): the preview lists the cross-references that print differently, page by page, before Apply.

use ew_api::AnyCommand;
use serde_json::json;

#[test]
fn ten_columns_and_six_rows_preview_the_cross_references_they_move() {
    let mut s = crate::common::sample();
    let facts = s.frame_facts("101").expect("page 101's frame");
    let cmd = AnyCommand {
        name: "frame.set_grid".to_owned(),
        params: json!({
            "frame": facts.frame.to_string(),
            "columns": {"count": 10, "first": 1},
            "rows": {"count": 6},
            "forks": facts.frame_forks.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
        }),
    };
    let preview = s.preview(&cmd).expect("the change previews");
    let diff = s.derived_diff(&preview);
    let moved: Vec<(String, String)> = diff
        .xrefs
        .iter()
        .map(|c| (c.before.clone(), c.after.clone()))
        .collect();
    for (before, after) in [("103-C1", "103-B1"), ("102-H0, 103-A1", "102-F1, 103-A1")] {
        assert!(
            moved.iter().any(|(b, a)| b == before && a == after),
            "{before} → {after} in {moved:?}"
        );
    }
}
