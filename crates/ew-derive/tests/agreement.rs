// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Agreement with the command layer (ID-48): the main representation `device_tag` prints with is the one
//! whose page scopes the counters `ew-commands` assigns, and the devices the command layer numbers are
//! exactly the devices whose tag comes from the page-based scheme. If the two drifted, two devices with
//! distinct counters in the command layer's scope could print the same tag.
//!
//! **This test pins a provisional rule (design decision 1 of WP-S1.06).** Both layers take the main
//! representation as the smallest-ID representation of the smallest-ID main function that is not a contact
//! image (doc 03 §5.1). Doc 03 §8.2 step 4
//! (lowest page order, row, column among the counted representation types) replaces that rule in
//! WP-S1.11, and it must replace it in `ew_commands::numbering` and in `ew_derive::MainRep` in the same
//! change; this test is the guard that fails if only one of them moves.
//!
//! **Schemes.** The command layer numbers the default-scheme devices per (page of the main
//! representation, letter). `device_tag` applies a scheme of the project's settings or a named scheme
//! only when that keeps tags unique wherever the counters are (counter scope `page`, `{page}`, `{letter}`
//! and `{counter}` in every format); otherwise the profile's default scheme applies and a note says so.
//! The property below checks the consequence: two numbered devices print the same product designation
//! exactly when the command layer sees them collide.

mod common;

use common::{Fx, id, uid};
use ew_derive::{DeriveNote, DeviceTag, MainRep, Q, SchemeIssue, TagSource};
use ew_schema::{Device, TagScheme};
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
        let tag = tag.as_ref().as_ref().unwrap();
        let scheme_tag = matches!(tag.source, TagSource::Scheme { .. });
        let named = matches!(record.tag.scheme, TagScheme::Named(_));
        // Numbered devices take their tag from a scheme; a scheme tag of a device the command layer does
        // not number is a named-scheme device's (its counter is the user's).
        assert_eq!(
            scheme_tag && !named,
            ew_commands::numbering::numbered(record),
            "numbered {dev}"
        );
        // A scheme that applied keeps the tag unique per counter scope.
        if let TagSource::Scheme { format, .. } = &tag.source {
            if ew_commands::numbering::numbered(record) {
                for t in ["{page}", "{letter}", "{counter}"] {
                    assert!(format.contains(t), "{dev}: {format} lacks {t}");
                }
            }
        }
    }
}

/// Every pair of numbered devices prints the same product designation exactly when the command layer's
/// counters collide.
fn assert_unique_where_counters_are(fx: &Fx, devices: &[u128]) {
    let m = fx.build();
    let q = Q::new(&m);
    for a in devices {
        for b in devices {
            if a >= b {
                continue;
            }
            let (da, db): (ew_ids::Id<Device>, ew_ids::Id<Device>) = (id(*a), id(*b));
            let numbered = |d| ew_commands::numbering::numbered(q.devices().get(d).unwrap());
            if !(numbered(da) && numbered(db)) {
                continue;
            }
            let placed = |d| ew_commands::numbering::main_page_in(&m, d).is_some();
            if !(placed(da) && placed(db)) {
                continue;
            }
            let product = |d| q.get::<DeviceTag>(d).as_ref().as_ref().unwrap().product.clone();
            assert_eq!(
                product(da) == product(db),
                ew_commands::numbering::collided_in(&m, da, db),
                "{da} {} / {db} {}",
                product(da),
                product(db)
            );
        }
    }
}

/// A contact image is never the main representation, in both layers (doc 03 §5.1): the device's main
/// function is drawn first as a contact image on 101, then as a multi-line symbol on 102.
#[test]
fn contact_images_are_not_main_representations() {
    let mut fx = base();
    fx.device(0x1000, "K", Some(1))
        .function(0x1001, Some(0x1000), 0x500, true)
        .rep(0x1010, PAGES[0], 0x510, &[0x1001], 0, 0)
        .with("rep_type", json!("contact_image"))
        .rep(0x1011, PAGES[1], 0x510, &[0x1001], 0, 0);
    // Only a contact image: unplaced in both layers.
    fx.device(0x2000, "K", Some(2))
        .function(0x2001, Some(0x2000), 0x500, true)
        .rep(0x2010, PAGES[0], 0x510, &[0x2001], 0, 0)
        .with("rep_type", json!("contact_image"));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(*q.get::<MainRep>(id(0x1000)), Some(id(0x1011)));
    assert_eq!(*q.get::<MainRep>(id(0x2000)), None);
    assert_eq!(
        ew_commands::numbering::main_page_in(&m, id(0x1000)),
        Some(id(PAGES[1]))
    );
    assert_eq!(ew_commands::numbering::main_page_in(&m, id(0x2000)), None);
    assert_agree(&fx, &[0x1000, 0x2000]);
}

fn notes_of(fx: &Fx, devices: &[u128]) -> Vec<DeriveNote> {
    let m = fx.build();
    let mut q = Q::new(&m);
    for d in devices {
        let _ = q.get::<DeviceTag>(id(*d));
    }
    q.diagnostics().notes()
}

fn product_of(fx: &Fx, device: u128) -> String {
    let m = fx.build();
    let q = Q::new(&m);
    q.get::<DeviceTag>(id(device))
        .as_ref()
        .as_ref()
        .unwrap()
        .product
        .clone()
}

/// Two devices on two pages with equal letter and counter; the command layer accepts both.
fn two_pages(fx: &mut Fx) -> [u128; 2] {
    for (i, d) in [0x1000u128, 0x2000].into_iter().enumerate() {
        fx.device(d, "F", Some(1))
            .function(d + 1, Some(d), 0x500, true)
            .rep(d + 0x10, PAGES[i], 0x510, &[d + 1], 0, 0);
    }
    [0x1000, 0x2000]
}

/// A settings scheme that the command layer does not number (project scope, or no `{page}`) is not
/// applied: the profile's default scheme prints, and a note says why. A page-scoped scheme with all three
/// tokens is applied.
#[test]
fn settings_schemes_the_command_layer_does_not_number_are_not_applied() {
    let cases = [
        (
            json!({"key": "flat", "format": "{letter}{counter}", "counter_scope": "project"}),
            Some(SchemeIssue::CounterScope),
            "101F1",
        ),
        (
            json!({"key": "flat", "format": "{letter}{counter}"}),
            Some(SchemeIssue::FormatWithoutPage),
            "101F1",
        ),
        (
            json!({"key": "flat", "format": "{page}{counter}"}),
            Some(SchemeIssue::FormatWithoutCounter),
            "101F1",
        ),
        (
            json!({"key": "flat", "format": "{page}{letter}{counter}",
                   "rules": [{"select": {"letter": "F"}, "format": "{letter}{counter}"}]}),
            Some(SchemeIssue::FormatWithoutPage),
            "101F1",
        ),
        (
            json!({"key": "flat", "format": "{letter}{page}.{counter}"}),
            None,
            "F101.1",
        ),
    ];
    for (scheme, issue, want) in cases {
        let mut fx = base();
        fx.project_set(
            "settings",
            json!({"numbering": {"default": "flat", "schemes": [scheme]}}),
        );
        let devices = two_pages(&mut fx);
        assert_eq!(product_of(&fx, devices[0]), want, "{scheme}");
        let notes = notes_of(&fx, &devices);
        match issue {
            Some(issue) => assert_eq!(
                notes,
                [DeriveNote::SchemeNotApplied {
                    scheme: "flat".into(),
                    issue
                }],
                "{scheme}"
            ),
            None => assert!(notes.is_empty(), "{scheme}: {notes:?}"),
        }
        assert_agree(&fx, &devices);
        assert_unique_where_counters_are(&fx, &devices);
    }
}

/// A named scheme is applied when page-scoped and complete, else the profile's default scheme prints; an
/// unknown key resolves to the project's default and is noted. Named-scheme devices are not numbered by
/// the command layer.
#[test]
fn named_schemes_agree_with_the_command_layer() {
    let mut fx = base();
    fx.project_set(
        "settings",
        json!({"numbering": {"default": "page-based", "schemes": [
            {"key": "page-based", "format": "{page}{letter}{counter}"},
            {"key": "plc", "format": "{letter}{page}-{counter}"},
            {"key": "flat", "format": "{letter}{counter}", "counter_scope": "project"},
        ]}}),
    );
    for (d, scheme) in [(0x1000u128, "plc"), (0x2000, "flat"), (0x3000, "nope")] {
        fx.device(d, "K", Some(7))
            .with(
                "tag",
                json!({"letter": "K", "counter": 7, "scheme": {"named": scheme}}),
            )
            .function(d + 1, Some(d), 0x500, true)
            .rep(d + 0x10, PAGES[0], 0x510, &[d + 1], 0, 0);
    }
    let devices = [0x1000, 0x2000, 0x3000];
    assert_eq!(product_of(&fx, 0x1000), "K101-7");
    assert_eq!(product_of(&fx, 0x2000), "101K7");
    assert_eq!(product_of(&fx, 0x3000), "101K7");
    assert_eq!(
        notes_of(&fx, &devices),
        [
            DeriveNote::SchemeNotApplied {
                scheme: "flat".into(),
                issue: SchemeIssue::CounterScope
            },
            DeriveNote::SchemeNotApplied {
                scheme: "nope".into(),
                issue: SchemeIssue::UnknownKey
            },
        ]
    );
    assert_agree(&fx, &devices);
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
    /// Random letters and counters on random pages under a random settings scheme (applicable or not): two
    /// numbered devices print the same product designation exactly when the command layer's counters
    /// collide.
    #[test]
    fn tags_are_unique_wherever_the_counters_are(
        scheme in 0usize..4,
        devices in proptest::collection::vec((0usize..3, 0usize..2, 1u32..3, any::<bool>()), 2..8),
    ) {
        let formats = [
            json!(null),
            json!({"key": "s", "format": "{letter}{counter}", "counter_scope": "project"}),
            json!({"key": "s", "format": "{page}.{letter}{counter}"}),
            json!({"key": "s", "format": "{letter}{counter}"}),
        ];
        let mut fx = base();
        if !formats[scheme].is_null() {
            fx.project_set(
                "settings",
                json!({"numbering": {"default": "s", "schemes": [formats[scheme].clone()]}}),
            );
        }
        let mut ids = Vec::new();
        for (i, (page, letter, counter, contact_first)) in devices.iter().enumerate() {
            let d = 0x10_000 + (i as u128) * 0x100;
            ids.push(d);
            fx.device(d, ["F", "K"][*letter], Some(*counter))
                .function(d + 1, Some(d), 0x500, true);
            if *contact_first {
                fx.rep(d + 0x10, PAGES[(page + 1) % 3], 0x510, &[d + 1], 0, 0)
                    .with("rep_type", json!("contact_image"));
            }
            fx.rep(d + 0x20, PAGES[*page], 0x510, &[d + 1], 0, 0);
        }
        assert_agree(&fx, &ids);
        assert_unique_where_counters_are(&fx, &ids);
    }

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
