// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-RDS-004: a full designation printed by more than one device. The devices are drawn on a page, so the
//! page-based numbering of the default profile gives them a complete designation (`-1K1`).

mod common;

use common::*;
use ew_checks::Severity;
use ew_derive::ew_schema::Kind;
use serde_json::json;

#[test]
fn rds004_reports_a_designation_printed_twice() {
    let mut fx = Fx::base();
    fx.drawn_device(0x101, "K", Some(1), P1);
    fx.drawn_device(0x102, "K", Some(1), P1);
    fx.drawn_device(0x103, "K", Some(2), P1);
    let f = findings_of(&fx.build(), "EW-RDS-004");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017004");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "");
    assert_eq!(
        f[0].targets.iter().map(|t| (t.kind, t.id)).collect::<Vec<_>>(),
        vec![(Kind::Device, uid(0x101)), (Kind::Device, uid(0x102))]
    );
    assert!(
        f[0].message.ends_with("is printed by 2 devices"),
        "{}",
        f[0].message
    );
    assert!(f[0].subject.contains("K1"), "{}", f[0].subject);
}

#[test]
fn rds004_distinct_designations_have_no_finding() {
    let mut fx = Fx::base();
    fx.drawn_device(0x101, "K", Some(1), P1);
    fx.drawn_device(0x102, "K", Some(2), P1);
    fx.drawn_device(0x103, "Q", Some(1), P1);
    // The same letter and counter on another page is another designation (`-2K1`).
    fx.drawn_device(0x104, "K", Some(1), P2);
    assert!(findings_of(&fx.build(), "EW-RDS-004").is_empty());
}

#[test]
fn rds004_skips_unnumbered_designations() {
    // Two devices that are still waiting for a number print "-1K?": EW-IMP-122, not a duplicate.
    let mut fx = Fx::base();
    fx.drawn_device(0x101, "K", None, P1);
    fx.drawn_device(0x102, "K", None, P1);
    assert!(findings_of(&fx.build(), "EW-RDS-004").is_empty());
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
