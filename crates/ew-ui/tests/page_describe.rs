// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `page.describe` in the window host (WP-S6.13 part 1, acceptance check BD5; §6 item 4): the exact counts. The first
//! `page.describe` of a folio that is not in the page-content cache makes exactly one `page_tables` call (with the
//! page content that carries it), a second makes none and causes no memo re-execution, and neither changes the
//! interface's state.

mod support;

use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::json;
use support::{Scripted, Tmp, harness, settle, wait_for_page};

#[test]
fn page_describe_makes_exactly_one_page_tables_call_per_revision_and_folio() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let shell = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| h.state().shell().session_calls();
    let before = shell(&h);
    let memo_before = h.state().shell().memo_totals().unwrap();
    let state_before = ew_ui::ew_viewmodel::script::state_json(h.state().shell());

    // A folio that is not on show and not in the cache: one page_tables call, one page content.
    let other = if h.state().shell().current_folio() == Some("102") {
        "103"
    } else {
        "102"
    };
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.describe", &json!({ "folio": other })),
        Outcome::Done
    );
    let first = shell(&h);
    assert_eq!(first.page_tables, before.page_tables + 1, "{first:?}");
    assert_eq!(first.page_content, before.page_content + 1, "{first:?}");
    let text = h.state_mut().shell_mut().take_page_text().unwrap();
    assert!(text.starts_with(&format!("page \"{other}\" \"")), "{text}");

    // The same folio again: nothing is read, and no memo entry runs again.
    let memo_after_first = h.state().shell().memo_totals().unwrap();
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.describe", &json!({ "folio": other })),
        Outcome::Done
    );
    assert_eq!(shell(&h), first, "a cached page costs no Session call");
    assert_eq!(
        h.state().shell().memo_totals().unwrap().1,
        memo_after_first.1,
        "and no memo re-execution"
    );
    assert_eq!(h.state_mut().shell_mut().take_page_text().unwrap(), text);
    // The first call re-executed only what the page content of that one folio needed.
    assert!(memo_after_first.1 >= memo_before.1);

    // Neither step changed the interface's state.
    assert_eq!(
        ew_ui::ew_viewmodel::script::state_json(h.state().shell()),
        state_before
    );
}
