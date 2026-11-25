// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1926: the new pages of an overview that runs over several pages are siblings, `104.1`, `104.2`, `104.3`, not
//! sub-pages of one another (doc 11 §5.5).

use crate::common;

use ew_api::AnyCommand;
use ew_schema::PageType;
use serde_json::json;

/// The sample's CPU station in a band one overview module wide: its three members (the head and the two cards) stand
/// one a page, on three new pages after its page 104.
#[test]
fn an_overview_over_several_pages_numbers_them_as_siblings() {
    let mut s = common::sample();
    let station = s.resolve_reference("tag:-104A1").expect("the CPU");
    let mut params = s.overview_params(station).expect("the parameters");
    let left = params["left"].as_i64().expect("left");
    // The wide overview module is 42.333 mm wide: one fits, a second does not.
    params["right"] = json!(left + 42_333_333 + 7_937_500);
    let cmd = AnyCommand {
        name: "valve_terminal.generate_overview".into(),
        params,
    };
    let preview = s.preview(&cmd).expect("the preview");
    s.commit(preview).expect("the overview");
    let labels = s.page_labels();
    let q = s.query();
    let overview: Vec<String> = q
        .pages()
        .iter()
        .filter(|(_, p)| p.kind == PageType::AssemblyOverview)
        .filter_map(|(_, p)| p.folio.label.as_ref().map(|l| l.to_string()))
        .collect();
    for label in ["104.1", "104.2", "104.3"] {
        assert!(overview.iter().any(|l| l == label), "{overview:?} in {labels:?}");
    }
    assert!(!overview.iter().any(|l| l.starts_with("104.1.")), "{overview:?}");
}
