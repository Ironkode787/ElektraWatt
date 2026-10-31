// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-RDS-004: a full designation printed by more than one device. The devices are drawn on a page, so the
//! page-based numbering of the default profile gives them a complete designation (`-1K1`).

use crate::common;

use common::*;
use ew_checks::Severity;
use ew_derive::ew_schema::Kind;
use serde_json::json;

#[test]
fn rds004_cases() {
    let mut printed_twice = Fx::base();
    printed_twice.drawn_device(0x101, "K", Some(1), P1);
    printed_twice.drawn_device(0x102, "K", Some(1), P1);
    printed_twice.drawn_device(0x103, "K", Some(2), P1);
    let mut distinct = Fx::base();
    distinct.drawn_device(0x101, "K", Some(1), P1);
    distinct.drawn_device(0x102, "K", Some(2), P1);
    distinct.drawn_device(0x103, "Q", Some(1), P1);
    // The same letter and counter on another page is another designation (`-2K1`).
    distinct.drawn_device(0x104, "K", Some(1), P2);
    let mut unnumbered = Fx::base();
    // Two devices that are still waiting for a number print "-1K?": EW-IMP-122, not a duplicate.
    unnumbered.drawn_device(0x101, "K", None, P1);
    unnumbered.drawn_device(0x102, "K", None, P1);
    rule_table(
        "EW-RDS-004",
        vec![
            Case::reports("a designation printed twice", printed_twice.build(), |c, f| {
                assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                assert_eq!(f[0].code, "017004", "case {c}");
                assert_eq!(f[0].severity, Severity::Error, "case {c}");
                assert_eq!(f[0].discriminator, "", "case {c}");
                assert_eq!(
                    f[0].targets.iter().map(|t| (t.kind, t.id)).collect::<Vec<_>>(),
                    vec![(Kind::Device, uid(0x101)), (Kind::Device, uid(0x102))],
                    "case {c}"
                );
                assert!(
                    f[0].message.ends_with("is printed by 2 devices"),
                    "case {c}: {}",
                    f[0].message
                );
                assert!(f[0].subject.contains("K1"), "case {c}: {}", f[0].subject);
            }),
            Case::silent("distinct designations", distinct.build()),
            Case::silent("unnumbered designations are skipped", unnumbered.build()),
        ],
    );
}

#[test]
fn rds004_leaves_terminals_to_term002() {
    let mut fx = Fx::base();
    for (strip, n) in [(0x10, 1), (0x11, 2)] {
        fx.push(json!({"$k": "device", "id": us(strip), "tag": {"letter": "X", "counter": n}, "strip": {}}));
    }
    for (dev, strip) in [(0x201, 0x10), (0x202, 0x11)] {
        fx.push(json!({
            "$k": "device", "id": us(dev), "tag": {"letter": "X"}, "kind": "terminal",
            "terminal": {"strip": us(strip), "seq": 1, "designation": "1"}
        }));
    }
    assert!(findings_of(&fx.build(), "EW-RDS-004").is_empty());
}

#[test]
fn rds004_fingerprint_survives_a_renumbering() {
    // The fingerprint holds the devices, not the printed designation.
    let fingerprint = |counter: u32| {
        let mut fx = Fx::base();
        fx.drawn_device(0x101, "K", Some(counter), P1);
        fx.drawn_device(0x102, "K", Some(counter), P1);
        let f = findings_of(&fx.build(), "EW-RDS-004");
        assert_eq!(f.len(), 1);
        f[0].fingerprint
    };
    assert_eq!(fingerprint(1), fingerprint(7));
}
