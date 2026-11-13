// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! AC-22 for the EW-PLC and EW-BUS packs (doc 20 §18.1, §18.2; WP-S1.24): with the packs' 1.0 content the findings of
//! Golden A are those of §18.1 for the two packs, EF-22's EW-PLC-013 superseding EF-10's EW-POT-003, and none of the
//! other 1.0 rules reports anything (EF-24: no load above a rating, the parts state none).

use crate::golden_common;

use std::collections::BTreeMap;

use ew_api::checks::{self, CheckMode, PRE_DELIVERY, Release};

/// The rule's pack, as the registry says.
fn pack_of(rule: &str) -> &'static str {
    checks::rule(rule).map_or("", |r| r.meta().pack)
}

#[test]
fn ac22_plc_and_bus_packs_with_the_1_0_content() {
    let batch = golden_common::batch();
    let s = golden_common::session(&batch);
    let report = s.check(&PRE_DELIVERY.with_content(Release::V1_0), CheckMode::Release);
    let mut got: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for f in report.findings.iter().filter(|f| f.is_open()) {
        if matches!(pack_of(f.rule), "EW-PLC" | "EW-BUS") || f.rule == "EW-POT-003" {
            got.entry((f.rule.to_owned(), f.severity.name().to_owned()))
                .or_default()
                .push(f.subject.clone());
        }
    }
    for v in got.values_mut() {
        v.sort();
    }
    let counts: Vec<(&str, &str, usize)> = got
        .iter()
        .map(|((r, s), v)| (r.as_str(), s.as_str(), v.len()))
        .collect();
    assert_eq!(
        counts,
        [
            ("EW-BUS-001", "error", 1),
            ("EW-BUS-004", "info", 1),
            ("EW-PLC-012", "error", 28),
            ("EW-PLC-013", "warning", 2),
        ],
        "AC-22's EW-PLC and EW-BUS findings: EF-02, EF-03, EF-16 and EF-22 (EF-10 superseded)"
    );
    assert_eq!(
        got[&("EW-PLC-013".to_owned(), "warning".to_owned())],
        ["-201U1:3", "-202U1:3"],
        "EF-22 names the pins EF-10 named"
    );
    // the MVP scheme still reports EF-10 and none of the 1.0 rules
    let mvp = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert!(mvp.findings.iter().any(|f| f.rule == "EW-POT-003" && f.is_open()));
    assert!(!mvp.rules.iter().any(|r| r.id == "EW-PLC-013"));
}
