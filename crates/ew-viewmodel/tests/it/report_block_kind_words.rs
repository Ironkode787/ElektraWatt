// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `report_block.new` takes a report's kind by the name its refusal lists, as well as by its key (FT-1868): the
//! refusal named the kinds in words and the action took only keys.

use crate::common::{Tmp, done, failed};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_report_kind_is_taken_by_the_name_the_refusal_offers() {
    let tmp = Tmp::new("report-kind-words");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    let message = failed(&mut s, "report_block.new", json!({"kind": "no such report", "folio": "950"}));
    // "…; use one of: Bus cable list, …": the first kind's name, in the words the refusal offers.
    let name = message
        .split(": ")
        .last()
        .and_then(|list| list.split(", ").next())
        .unwrap_or_else(|| panic!("{message}"))
        .trim_end_matches('.')
        .to_owned();
    assert!(name.contains(' ') || !name.contains('_'), "a name, not a key: {message}");
    done(&mut s, "report_block.new", json!({"kind": name, "folio": "950"}));
    // The key goes on as before.
    done(&mut s, "report_block.new", json!({"kind": "terminal_diagram", "folio": "2950"}));
}
