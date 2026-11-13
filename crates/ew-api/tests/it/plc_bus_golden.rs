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

/// The rows of the report kind `kind` on Golden A as CSV (the shared source of PDF, XLSX and CSV, doc 09 §3).
fn golden_rows(kind: &str) -> Vec<String> {
    let batch = golden_common::batch();
    let s = golden_common::session(&batch);
    let q = s.query();
    let block = batch
        .records
        .iter()
        .find_map(|r| match r {
            ew_schema::Record::ReportBlock(b) => Some(b.clone()),
            _ => None,
        })
        .expect("a report block");
    let def = ew_reports::Registry::builtin()
        .by_name(kind)
        .expect("a registered kind");
    ew_reports::csv::rows_csv(&q, &def.rows(&q, &block))
        .lines()
        .map(str::to_owned)
        .collect()
}

/// FR-BUS-008: Golden A's network address list has two rows, JE-N1000-03 (master, external) and -201U1 (slave, its
/// address `Tbd`), and its bus cable list one, -3W1 from the external end to -3X1.
#[test]
fn golden_network_address_and_bus_cable_lists() {
    let net = golden_rows("network_address_list");
    assert_eq!(net.len(), 3, "a header and two rows: {net:#?}");
    assert!(
        net[1].contains("JE-N1000-03") && net[1].contains(",yes,"),
        "{}",
        net[1]
    );
    assert!(net[2].contains("-201U1") && net[2].contains("TBD"), "{}", net[2]);
    let cables = golden_rows("bus_cable_list");
    assert_eq!(cables.len(), 2, "{cables:#?}");
    assert!(cables[1].starts_with("data,-3W1,"), "{}", cables[1]);
    assert!(cables[1].contains("-3X1"), "{}", cables[1]);
}

/// FR-BUS-007, FR-REP-045: the bus overview walks the DP segment from the master end (the external end, -3W1,
/// -3X1 with its termination on, -201U1:DP), and the PLC card list holds the seven modules with 8 used, 20 spare and
/// 28 deactivated channels.
#[test]
fn golden_bus_overview_and_card_list() {
    let bus = golden_rows("bus_overview");
    assert_eq!(bus.len(), 5, "{bus:#?}");
    assert!(bus[1].contains("JE-N1000-03"), "{}", bus[1]);
    assert!(bus[2].contains("-3W1"), "{}", bus[2]);
    assert!(bus[3].contains("-3X1") && bus[3].contains("on"), "{}", bus[3]);
    assert!(bus[4].contains("-201U1") && bus[4].contains("TBD"), "{}", bus[4]);
    let cards = golden_rows("plc_card_list");
    assert_eq!(cards.len(), 8, "{cards:#?}");
    let sum = |col: usize| -> i64 {
        cards[1..]
            .iter()
            .map(|l| {
                l.split(',')
                    .nth(col)
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(0)
            })
            .sum()
    };
    // the first field is the row kind
    assert_eq!((sum(9), sum(10), sum(11), sum(12)), (8, 20, 0, 28));
    assert!(cards[1].contains("QB0"), "{}", cards[1]);
}
