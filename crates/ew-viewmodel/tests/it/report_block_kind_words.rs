// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `report_block.new` takes a report's kind by its key or by its name, as its refusal lists them (FT-1868): the
//! refusal named the kinds in words and the action took only keys.

use crate::common::{Tmp, done, failed};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_report_kind_is_taken_by_the_name_the_refusal_offers() {
    let tmp = Tmp::new("report-kind-words");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    let message = failed(&mut s, "report_block.new", json!({"kind": "no such report", "folio": "950"}));
    // "…; use one of: Bill of materials (bom), …": the first kind's name and key.
    let first = message
        .split(": ")
        .last()
        .and_then(|list| list.split(", ").next())
        .unwrap_or_else(|| panic!("{message}"));
    let (name, key) = first
        .trim_end_matches('.')
        .trim_end_matches(')')
        .split_once(" (")
        .unwrap_or_else(|| panic!("a name and its key: {message}"));
    assert!(!name.is_empty() && !key.contains(' '), "{message}");
    done(&mut s, "report_block.new", json!({"kind": name, "folio": "950"}));
    done(&mut s, "report_block.new", json!({"kind": key, "folio": "2950"}));
}
