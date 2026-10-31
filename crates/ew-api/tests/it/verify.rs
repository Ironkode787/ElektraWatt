// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew verify-derived` at the library level (`ew_api::verify`, FR-EXT-005): the memoised values of every
//! family equal their cold recomputation over an empty model, over the golden project and after an edit; a
//! session that does not memoise cannot be verified.

use crate::golden_common;

use ew_api::verify::{SWEPT, VerifyError, verify_derived};
use ew_api::{MemoMode, Session, SessionOptions};

fn empty(memo: MemoMode) -> Session {
    let project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .expect("a project row");
    Session::from_records(
        project,
        [],
        [],
        SessionOptions {
            memo,
            ..SessionOptions::default()
        },
    )
    .expect("an empty model")
}

/// The guard: the swept families are the `Memo` and `MemoLru` families of the three `FAMILIES` lists, the
/// derived data of `ew-derive`, the report engine of `ew-reports` and the page output of `ew-api` (the views and
/// the per-layer page display lists, FT-59). A promotion or demotion of a family fails here.
#[test]
fn the_swept_families_are_the_memoised_families_of_the_three_lists() {
    use ew_derive::Policy;
    let mut memoised: Vec<&str> = ew_derive::FAMILIES
        .iter()
        .chain(ew_reports::FAMILIES)
        .chain(ew_api::output::FAMILIES)
        .filter(|(_, p)| matches!(p, Policy::Memo | Policy::MemoLru))
        .map(|(n, _)| *n)
        .collect();
    memoised.sort_unstable();
    assert_eq!(memoised, SWEPT);
    for family in [
        "report_rows",
        "report_block_pages",
        "tag_class_members",
        "page_display_list_frame",
        "page_display_list_wires",
    ] {
        assert!(SWEPT.contains(&family), "{family} is not swept");
    }
}

#[test]
fn an_empty_model_has_no_difference() {
    let report = verify_derived(&empty(MemoMode::On)).expect("a memoising session");
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(report.keys.keys().copied().collect::<Vec<_>>(), SWEPT);
}

/// With the memo store off a memoised and a cold read are one computation: nothing to verify, an error rather
/// than an always-clean report.
#[test]
fn a_session_without_a_memo_store_is_refused() {
    assert_eq!(
        verify_derived(&empty(MemoMode::Off)).unwrap_err(),
        VerifyError::MemoOff
    );
    assert!(verify_derived(&empty(MemoMode::Oracle)).is_ok());
}

#[test]
fn the_golden_project_has_no_difference_before_and_after_an_edit() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let before = verify_derived(&s).expect("a memoising session");
    assert!(before.is_clean(), "{:?}", before.differences);
    assert_eq!(before.keys.keys().copied().collect::<Vec<_>>(), SWEPT);
    assert!(before.keys["report_rows"] >= 11, "every report block is read");
    assert!(before.total_keys() > 1000, "{}", before.total_keys());
    // One command through the commit pipeline; the memo store follows it by push invalidation.
    let rename = serde_json::json!({
        "cmd": "fields.set",
        "params": {
            "kind": "device",
            "targets": [{"ref": "tag:T55"}],
            "path": "tag.frozen",
            "value": {"text": "T57", "sign": "hidden"},
        },
    });
    let log = ew_api::CmdLog::parse(&format!("{{\"$schema\":\"ew.cmdlog/1\"}}\n{rename}\n"))
        .expect("a command log");
    s.run_script(&log, "verify.ewcmd").expect("the rename commits");
    let after = verify_derived(&s).expect("a memoising session");
    assert!(after.is_clean(), "{:?}", after.differences);
    assert_eq!(after.keys, before.keys, "the edit adds and removes no key");
}
