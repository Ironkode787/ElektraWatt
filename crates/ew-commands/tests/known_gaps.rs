// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Documented gaps of v0 (README "Known gaps"): tests that pin today's behaviour so a change is noticed.

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{KindName, SetFields};
use serde_json::json;

// `AddTerminals` itself refuses a duplicate designation, so the gap is shown through `fields.set` of
// `terminal.designation`, which leaves the duplicate to the prevent rules.
#[test]
fn prevent_rules_are_not_wired_until_wp_s1_15() {
    let mut e = engine(golden_model());
    // The strip DEV_X1031 holds terminals "1", "2" and "3": rename the second one to "1".
    let cmd = SetFields {
        kind: KindName::Device,
        targets: vec![DEV_T2.cast()],
        path: "terminal.designation".into(),
        value: json!("1"),
    };
    // Known gap (README "Known gaps"): no prevent rule is wired until WP-S1.15; when ew-checks plugs in, D-12
    // rejects this in strict mode and this test must be inverted.
    e.execute(&cmd)
        .expect("duplicate terminal designations commit while no prevent rule is wired");
}
