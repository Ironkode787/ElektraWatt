// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Connection.computed` (docs/design/06a-drafting.md §2.5), optional and additive inside format 0.1: a connection
//! written before it reads as the person's route and is written back byte-equal; the mark round-trips where it is set.

use crate::common;

use common::fill;
use ew_schema::Record;

fn connection(extra: &str) -> String {
    fill(&format!(
        r#"{{"$k":"connection","id":"@CON_W7","a":{{"cp":["@FN_S51",0]}},"b":{{"cp":["@FN_F1",1]}},"route":[[0,0],[0,7937500]]{extra}}}"#
    ))
}

fn computed(line: &str) -> bool {
    let r: Record = serde_json::from_str(line).unwrap_or_else(|e| panic!("parse failed: {e}\n{line}"));
    assert_eq!(serde_json::to_string(&r).unwrap(), line, "not canonical");
    let Record::Connection(c) = r else {
        panic!("a connection");
    };
    c.computed
}

#[test]
fn a_route_written_before_the_mark_is_the_person_s_and_is_written_back_unchanged() {
    assert!(!computed(&connection("")));
    assert!(computed(&connection(r#","computed":true"#)));
}
