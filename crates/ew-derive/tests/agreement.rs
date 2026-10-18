// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Agreement with the command layer (ID-48): the main representation `device_tag` prints with is the one
//! whose page scopes the counters `ew-commands` assigns, and the devices the command layer numbers are
//! exactly the devices whose tag comes from the page-based scheme. If the two drifted, two devices with
//! distinct counters in the command layer's scope could print the same tag.
//!
//! **This test pins a provisional rule (design decision 1 of WP-S1.06).** Both layers take the main
//! representation as the smallest-ID representation of the smallest-ID main function. Doc 03 §8.2 step 4
//! (lowest page order, row, column among the counted representation types) replaces that rule in
//! WP-S1.11, and it must replace it in `ew_commands::numbering` and in `ew_derive::MainRep` in the same
//! change; this test is the guard that fails if only one of them moves.

mod common;

use common::{Fx, id, uid};
use ew_derive::{DeviceTag, MainRep, Q, TagSource};
use ew_schema::Device;
use proptest::prelude::*;
use serde_json::json;

const PAGES: [u128; 3] = [0x101, 0x102, 0x103];

fn base() -> Fx {
    let mut fx = Fx::new();
    for (i, p) in PAGES.iter().enumerate() {
        fx.page(*p, 101 + i as u32, None, "multi_line", &[], "page");
    }
    fx.fndef(
        0x500,
        ["ProtectionOvercurrent", "MCB", "1P"],
        &[(0, "1"), (1, "2")],
    )
    .symbol(0x510, 0x500, json!("origin"), &[(0, 0, 0, 0)]);
    fx
}

/// Checks agreement for every device of a model.
fn assert_agree(fx: &Fx, devices: &[u128]) {
    let m = fx.build();
    let q = Q::new(&m);
    for d in devices {
        let dev: ew_ids::Id<Device> = id(*d);
        let ours = q
            .get::<MainRep>(dev)
            .and_then(|r| q.reps().get(r).map(|r| r.page));
        assert_eq!(
            ours,
            ew_commands::numbering::main_page_in(&m, dev),
            "main page of {dev}"
        );
        let record = q.devices().get(dev).unwrap();
        let tag = q.get::<DeviceTag>(dev);
        let scheme_tag = matches!(tag.as_ref().as_ref().unwrap().source, TagSource::Scheme { .. });
        assert_eq!(
            scheme_tag,
            ew_commands::numbering::numbered(record),
            "numbered {dev}"
        );
    }
}

#[test]
fn several_main_functions_and_representations() {
    let mut fx = base();
    // Device A: the main function is drawn three times on two pages (the smallest rep ID wins), aux
    // functions are drawn on other pages with smaller rep IDs.
    fx.device(0x1000, "F", Some(1))
        .function(0x1003, Some(0x1000), 0x500, false)
        .function(0x1001, Some(0x1000), 0x500, false)
        .function(0x1002, Some(0x1000), 0x500, true)
        .rep(0x1010, PAGES[0], 0x510, &[0x1003], 0, 0)
        .rep(0x1012, PAGES[2], 0x510, &[0x1002], 0, 0)
        .rep(0x1011, PAGES[1], 0x510, &[0x1001], 0, 0)
        .rep(0x1014, PAGES[0], 0x510, &[0x1002], 0, 0);
    // Device B: frozen; device C: unplaced; device D: a terminal of strip E.
    fx.device(0x2000, "S", None)
        .with(
            "tag",
            json!({"letter": "S", "frozen": {"text": "S51", "sign": "hidden"}}),
        )
        .function(0x2001, Some(0x2000), 0x500, true)
        .rep(0x2002, PAGES[1], 0x510, &[0x2001], 0, 0);
    fx.device(0x3000, "K", None);
    fx.device(0x5000, "X", Some(1))
        .with("strip", json!({}))
        .function(0x5001, Some(0x5000), 0x500, true)
        .rep(0x5002, PAGES[2], 0x510, &[0x5001], 0, 0);
    fx.device(0x4000, "X", None).with(
        "terminal",
        json!({"strip": uid(0x5000), "seq": 1, "designation": "1"}),
    );
    assert_agree(&fx, &[0x1000, 0x2000, 0x3000, 0x4000, 0x5000]);
}

proptest! {
    /// Random devices (one main function, aux functions) drawn by random representations on random pages.
    #[test]
    fn main_page_agrees_with_the_command_layer(
        devices in proptest::collection::vec(
            proptest::collection::vec(proptest::collection::vec((0usize..3, 0u128..64), 0..3), 1..4),
            1..6,
        )
    ) {
        let mut fx = base();
        let mut ids = Vec::new();
        let mut used = std::collections::BTreeSet::new();
        for (di, functions) in devices.iter().enumerate() {
            let d = 0x10_000 + (di as u128) * 0x1000;
            ids.push(d);
            fx.device(d, "F", Some(1));
            for (fi, reps) in functions.iter().enumerate() {
                let f = d + 0x100 + fi as u128;
                fx.function(f, Some(d), 0x500, fi == 0);
                for (page, salt) in reps {
                    // Representation IDs vary independently of the function order.
                    let mut r = 0x100_000 + salt * 0x100 + f % 0x100 + (di as u128) * 0x10_000;
                    while !used.insert(r) {
                        r += 1;
                    }
                    fx.rep(r, PAGES[*page], 0x510, &[f], 0, 0);
                }
            }
        }
        assert_agree(&fx, &ids);
    }
}
