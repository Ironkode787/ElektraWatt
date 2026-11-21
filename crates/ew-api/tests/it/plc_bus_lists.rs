// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus and PLC report kinds on the sample (FR-BUS-008, FR-REP-045): the network address list, the bus cable
//! list and the PLC card list of the CPU -104A1 with its two cards and the valve terminal -201K1 on PROFIBUS DP.

/// The rows of the report kind `kind` on the sample as CSV, without the header (the shared source of PDF, XLSX and
/// CSV, doc 09 §3).
fn sample_rows(kind: &str) -> Vec<String> {
    let s = crate::common::sample();
    let q = s.query();
    let block = q
        .report_blocks()
        .iter()
        .next()
        .map(|(_, b)| (**b).clone())
        .expect("a report block");
    let def = ew_reports::Registry::builtin()
        .by_name(kind)
        .expect("a registered kind");
    ew_reports::csv::rows_csv(&q, &def.rows(&q, &block))
        .lines()
        .skip(1)
        .map(str::to_owned)
        .collect()
}

/// FR-BUS-008: the network address list has a row per station and logical net: the CPU -104A1 as the DP master
/// (address 2) and on its Ethernet net (its IP address), the valve terminal -201K1 as a DP slave (address 3); the bus
/// cable list has the DP cable -105W1 from the CPU's port to the valve terminal's.
#[test]
fn the_network_address_and_bus_cable_lists() {
    let net = sample_rows("network_address_list");
    assert_eq!(net.len(), 3, "{net:#?}");
    let row = |station: &str, address: &str| {
        net.iter()
            .find(|l| l.contains(&format!(",{station},")) && l.contains(&format!(",{address},")))
            .unwrap_or_else(|| panic!("no row of {station} at {address}: {net:#?}"))
    };
    assert!(row("-104A1", "2").contains(",yes,"), "the CPU is the DP master");
    assert!(row("-201K1", "3").contains(",no,"), "the valve terminal is a slave");
    assert!(row("-104A1", "192.168.0.10").contains(",yes,"));
    let cables = sample_rows("bus_cable_list");
    assert_eq!(cables.len(), 1, "{cables:#?}");
    assert!(cables[0].starts_with("data,-105W1,PROFIBUS DP,"), "{}", cables[0]);
    assert!(cables[0].contains(",-104A1:DP,-201K1:DP,"), "{}", cables[0]);
}

/// FR-REP-045: the PLC card list holds the input card -104A1-A1 (one used channel, seven spare), the output card
/// -104A1-A2 (two used, six spare, from QB1) and the valve terminal's eight outputs (four valves, the other four
/// deactivated).
#[test]
fn the_plc_card_list() {
    let cards = sample_rows("plc_card_list");
    assert_eq!(cards.len(), 3, "{cards:#?}");
    let sum = |col: usize| -> i64 {
        cards
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
    assert_eq!((sum(9), sum(10), sum(11), sum(12)), (7, 13, 0, 4));
    let card = |module: &str| {
        cards
            .iter()
            .find(|l| l.contains(&format!(",{module},")))
            .unwrap_or_else(|| panic!("no row of {module}"))
    };
    assert!(card("-104A1-A1").contains(",IB0,"), "{}", card("-104A1-A1"));
    assert!(card("-104A1-A2").contains(",QB1,"), "{}", card("-104A1-A2"));
}
