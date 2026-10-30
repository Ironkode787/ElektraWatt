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

/// A strip without a segment takes the main representation of its first placed terminal, in both layers
/// (owner ruling for Golden B step B1): lowest page, then lowest designation in natural order ("2" before
/// "10"), then smallest ID; an unplaced terminal does not count. Its tag gets that page.
#[test]
fn a_strip_without_a_segment_is_placed_through_its_first_terminal() {
    let mut fx = base();
    fx.push(json!({"$k": "device", "id": uid(0x3000), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    // (terminal, designation, page): the first placed terminal is "2" on page 102.
    for (n, designation, page) in [
        (0x3010, "10", Some(PAGES[1])),
        (0x3020, "2", Some(PAGES[1])),
        (0x3030, "1", Some(PAGES[2])),
        (0x3040, "0", None),
    ] {
        fx.push(json!({"$k": "device", "id": uid(n), "tag": {"letter": "X"},
            "terminal": {"strip": uid(0x3000), "seq": 1, "designation": designation}}));
        fx.function(n + 1, Some(n), 0x500, true);
        if let Some(page) = page {
            fx.rep(n + 2, page, 0x510, &[n + 1], 0, 0);
        }
    }
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(*q.get::<MainRep>(id(0x3000)), Some(id(0x3022)));
    assert_eq!(
        ew_commands::numbering::main_page_in(&m, id(0x3000)),
        Some(id(PAGES[1]))
    );
    assert_eq!(
        q.get::<DeviceTag>(id(0x3000)).as_ref().as_ref().unwrap().product,
        "102X1"
    );
    assert_agree(&fx, &[0x3000]);
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

/// A device whose main function is not drawn is placed through its other functions, in both layers: the
/// smallest-ID representation that is not a contact image (a contact placed from a part before its coil). Its tag
/// gets that page instead of "?".
#[test]
fn a_device_without_a_drawn_main_function_is_placed_through_its_auxiliaries() {
    let mut fx = base();
    fx.device(0x1000, "K", Some(1))
        .function(0x1001, Some(0x1000), 0x500, true)
        .function(0x1002, Some(0x1000), 0x500, false)
        .function(0x1003, Some(0x1000), 0x500, false)
        .rep(0x1013, PAGES[2], 0x510, &[0x1003], 0, 0)
        .rep(0x1012, PAGES[1], 0x510, &[0x1002], 0, 0)
        .rep(0x1010, PAGES[0], 0x510, &[0x1002], 0, 0)
        .with("rep_type", json!("contact_image"));
    // Only a contact image of an auxiliary: unplaced in both layers.
    fx.device(0x2000, "K", Some(2))
        .function(0x2001, Some(0x2000), 0x500, true)
        .function(0x2002, Some(0x2000), 0x500, false)
        .rep(0x2010, PAGES[0], 0x510, &[0x2002], 0, 0)
        .with("rep_type", json!("contact_image"));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(*q.get::<MainRep>(id(0x1000)), Some(id(0x1012)));
    assert_eq!(
        ew_commands::numbering::main_page_in(&m, id(0x1000)),
        Some(id(PAGES[1]))
    );
    assert_eq!(product_of(&fx, 0x1000), "102K1");
    assert_eq!(*q.get::<MainRep>(id(0x2000)), None);
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

// ---------------------------------------------------------------------------------------------------------
// Anchor cells (FT-456): `ew_commands::numbering::anchor_cell` is the model-only copy of `RepAnchorCell` that
// online numbering orders by (normalisers see no derived data). The two must agree on every representation, for
// every anchor policy, orientation, slot override and frame resolution the derivation handles.

/// The golden grid frame's copy ID (`content/forms/ew.frame.qet-a4-landscape-grid.json`).
const FRAME_COPY: &str = "c80af135-34f8-574c-8f52-756bd24c7bd0";
/// The page every anchor case draws on.
const ANCHOR_PAGE: u128 = 0x701;
const MM: i64 = 1_000_000;
const ORIENTS: [&str; 8] = ["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"];

/// A frame form with its own grid: 3 columns from index 1 with the labels I and II (the third has none: no cell),
/// 4 rows skipping I and O, below a 10 mm header band.
fn labelled_form() -> serde_json::Value {
    json!({
        "kind": "frame",
        "sheet": {"width": 297_000_000, "height": 210_000_000},
        "grid": {
            "header_band": {"num": 10_000_000, "den": 1},
            "columns": {"count": 3, "start_index": 1, "width": {"num": 20_000_000, "den": 1}, "labels": ["I", "II"]},
            "rows": {"count": 4, "height": {"num": 20_000_000, "den": 1}, "skip_io": true},
        },
    })
}

/// One frame resolution of `ew_derive::FrameGridOf`: page type, `Page.form`, `frames` shim (`null`: none),
/// whether the page has cells at all.
type FrameCase = (&'static str, Option<String>, serde_json::Value, bool);

/// The frame resolutions of `ew_derive::FrameGridOf`, one per case.
fn frame_cases() -> Vec<FrameCase> {
    let custom = uid(0x600);
    let nogrid = uid(0x601);
    vec![
        // The page's own form.
        ("multi_line", Some(FRAME_COPY.into()), json!(null), true),
        // Nothing configured: the built-in default grid.
        ("multi_line", None, json!(null), true),
        // The shim's default.
        ("multi_line", None, json!({"default": custom}), true),
        // The shim's entry of the page type wins over its default.
        (
            "multi_line",
            None,
            json!({"default": FRAME_COPY, "per_page_type": [{"page_type": "multi_line", "form": custom}]}),
            true,
        ),
        // An entry of another page type does not apply.
        (
            "multi_line",
            None,
            json!({"default": custom, "per_page_type": [{"page_type": "fluid", "form": FRAME_COPY}]}),
            true,
        ),
        // The page's form wins over the shim; a form without a grid has no cells.
        (
            "multi_line",
            Some(nogrid.clone()),
            json!({"default": FRAME_COPY}),
            false,
        ),
        ("multi_line", None, json!({"default": nogrid}), false),
        // Forms the project does not hold fall through to the built-in grid.
        (
            "multi_line",
            None,
            json!({"default": uid(0x777), "per_page_type": [{"page_type": "multi_line", "form": uid(0x778)}]}),
            true,
        ),
        // A malformed matching entry falls through to the default; a malformed shim to the built-in grid.
        (
            "multi_line",
            None,
            json!({"default": custom, "per_page_type": [{"page_type": "multi_line", "form": 1}, 7]}),
            true,
        ),
        ("multi_line", None, json!("not an object"), true),
        (
            "multi_line",
            None,
            json!({"default": 5, "per_page_type": 3}),
            true,
        ),
        // Title, contents and divider pages: no grid without configuration; the shim's default or entry applies.
        ("title_page", None, json!(null), false),
        ("table_of_contents", None, json!({"default": FRAME_COPY}), true),
        ("section_divider", None, json!(null), false),
        (
            "title_page",
            None,
            json!({"per_page_type": [{"page_type": "title_page", "form": custom}]}),
            true,
        ),
    ]
}

/// One symbol of the anchor cases: copy ID, anchor policy, graphics, text slots.
type AnchorSymbol = (
    u128,
    serde_json::Value,
    Vec<serde_json::Value>,
    Vec<serde_json::Value>,
);

/// The symbols of the anchor cases, by copy ID: every anchor policy (`origin`, `named`, `slot` with and without a
/// matching slot, `graphic_centre` with and without graphics, a policy of a newer format).
fn anchor_symbols(fx: &mut Fx) -> Vec<u128> {
    let rect = |x0: i64, y0: i64, x1: i64, y1: i64| json!({"rect": {"rect": [[x0, y0], [x1, y1]]}});
    let slot = |kind: &str, x: i64, y: i64| json!({"kind": kind, "pos": [x, y], "align": "centre"});
    let symbols: Vec<AnchorSymbol> = vec![
        (0x520, json!("origin"), vec![], vec![]),
        (0x521, json!({"named": [16 * MM, 8 * MM]}), vec![], vec![]),
        (
            0x522,
            json!({"slot": "xref"}),
            vec![],
            vec![slot("tag", 0, 0), slot("xref", 8 * MM, -8 * MM)],
        ),
        (
            0x523,
            json!({"slot": "plc_address"}),
            vec![],
            vec![slot("tag", 16 * MM, 16 * MM)],
        ),
        (
            0x524,
            json!("graphic_centre"),
            vec![
                rect(0, 0, 16 * MM, 32 * MM),
                json!({"circle": {"centre": [-3, 5], "r": 8 * MM}}),
                json!({"ellipse": {"rect": [[0, -8 * MM], [33 * MM, 8 * MM + 1]]}}),
            ],
            vec![],
        ),
        (0x525, json!("graphic_centre"), vec![], vec![]),
        (0x526, json!("a_policy_of_a_newer_format"), vec![], vec![]),
    ];
    let mut out = Vec::new();
    for (n, anchor, graphics, slots) in symbols {
        fx.symbol(n, 0x500, anchor, &[(0, 0, 0, 0)]);
        let doc = fx.lib.last_mut().expect("the symbol");
        if !graphics.is_empty() {
            doc["graphics"] = json!(graphics);
        }
        if !slots.is_empty() {
            doc["text_slots"] = json!(slots);
        }
        out.push(n);
    }
    out
}

/// The point at grid index `(kx, ky)` of the QET 10 px grid, in nm.
fn qet(kx: i64, ky: i64) -> (i64, i64) {
    let g = ew_units::Grid::QET_10PX;
    (g.at(kx).expect("fits").0, g.at(ky).expect("fits").0)
}

/// Asserts that both layers give every representation of `m` the same anchor cell; returns how many have one.
fn assert_anchor_cells_agree(m: &ew_model::Model, what: &str) -> usize {
    let q = Q::new(m);
    let mut with_cell = 0;
    for (rep, _) in q.reps().iter() {
        let derived = (*q.get::<ew_derive::RepAnchorCell>(rep)).clone();
        let ours = ew_commands::numbering::anchor_cell_in(m, rep);
        assert_eq!(ours, derived, "{what}: representation {rep}");
        with_cell += usize::from(derived.is_some());
    }
    with_cell
}

/// A fixture with the golden frame, the labelled frame (0x600), a frame without a grid (0x601), the anchor
/// symbols and one device whose main function the representations draw; the page is added by the caller.
fn anchor_fixture() -> (Fx, Vec<u128>) {
    let mut fx = Fx::new();
    fx.lib(
        serde_json::from_str(include_str!(
            "../../../content/forms/ew.frame.qet-a4-landscape-grid.json"
        ))
        .unwrap(),
    );
    fx.lib(serde_json::from_str(include_str!("../../../content/forms/ew.titleblock.qet-a4.json")).unwrap());
    fx.lib_item("form", 0x600, labelled_form());
    fx.lib_item(
        "form",
        0x601,
        json!({"kind": "frame", "sheet": {"width": 297_000_000, "height": 210_000_000}}),
    );
    fx.fndef(0x500, ["ProtectionOvercurrent", "MCB", "1P"], &[(0, "1")]);
    let symbols = anchor_symbols(&mut fx);
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), 0x500, true);
    (fx, symbols)
}

/// Each frame case with representations of every anchor policy, in every orientation, at points inside, on the
/// boundaries of and outside the drawing area (and off the grid), with and without a slot override and a
/// representation's own anchor policy.
#[test]
fn anchor_cells_agree_with_the_command_layer() {
    let points: Vec<(i64, i64)> = vec![
        qet(11, 38),
        qet(53, 14),
        qet(101, 62),
        qet(1, 1),
        qet(103, 10),
        qet(104, 10),
        qet(10, 66),
        qet(-5, 10),
        (25_000_000, 15_000_000),
        (25_000_001, 34_999_999),
        (45_000_000, 90_000_000),
    ];
    for (case, (kind, form, frames, cells)) in frame_cases().into_iter().enumerate() {
        let (mut fx, symbols) = anchor_fixture();
        if !frames.is_null() {
            fx.project_set("settings", json!({ "frames": frames }));
        }
        fx.page(ANCHOR_PAGE, 101, None, kind, &[], "anchors");
        fx.record(ANCHOR_PAGE)["grid"] = json!({"num": 7_937_500, "den": 3});
        if let Some(form) = &form {
            fx.record(ANCHOR_PAGE)["form"] = json!(form);
        }
        let mut n = 0x10_000u128;
        for (si, symbol) in symbols.iter().enumerate() {
            for (pi, (x, y)) in points.iter().enumerate() {
                // Two opposite orientations per symbol and point, rotating with them: every symbol, point and
                // orientation occurs, at a quarter of the full grid.
                for oi in [(si + pi) % 8, (si + pi + 4) % 8] {
                    let orient = &ORIENTS[oi];
                    fx.rep(n, ANCHOR_PAGE, *symbol, &[0x1001], *x, *y);
                    fx.record(n)["orient"] = json!(orient);
                    // Every third: the xref slot (index 1) dragged 16 mm in symbol coordinates; every fifth: the
                    // representation's own policy overrides the symbol's.
                    if (si + oi + pi) % 3 == 0 {
                        fx.record(n)["text_slots"] = json!([{"slot": 1, "pos": [16 * MM, 0]}]);
                    }
                    if (si + oi + pi) % 5 == 0 {
                        fx.record(n)["anchor"] = json!({"named": [-8 * MM, 24 * MM]});
                    }
                    n += 1;
                }
            }
        }
        let m = fx.build();
        let with_cell = assert_anchor_cells_agree(&m, &format!("frame case {case}"));
        assert_eq!(with_cell > 0, cells, "frame case {case}: {with_cell} cells");
    }
}

proptest! {
    // 32 cases per gate (the fixed grid of `anchor_cells_agree_with_the_command_layer` covers the shapes);
    // PROPTEST_CASES overrides it.
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Random anchors (a representation's named point or a dragged slot), orientations and positions on the golden
    /// frame and on the labelled frame: both layers give the same cell.
    #[test]
    fn random_anchor_cells_agree_with_the_command_layer(
        labelled in any::<bool>(),
        reps in proptest::collection::vec(
            (0usize..8, -60i64..360, -60i64..260, -40i64..40, -40i64..40, any::<bool>()),
            1..12,
        ),
    ) {
        let (mut fx, symbols) = anchor_fixture();
        fx.page(ANCHOR_PAGE, 101, None, "multi_line", &[], "anchors");
        fx.record(ANCHOR_PAGE)["grid"] = json!({"num": 7_937_500, "den": 3});
        fx.record(ANCHOR_PAGE)["form"] = json!(if labelled { uid(0x600) } else { FRAME_COPY.into() });
        for (i, (orient, x, y, ax, ay, slot)) in reps.iter().enumerate() {
            let n = 0x10_000 + i as u128;
            // Positions in 1 mm steps plus an odd remainder, so on- and off-grid points both occur.
            fx.rep(n, ANCHOR_PAGE, symbols[2], &[0x1001], x * MM + x % 7, y * MM - y % 5);
            fx.record(n)["orient"] = json!(ORIENTS[*orient]);
            if *slot {
                fx.record(n)["text_slots"] = json!([{"slot": 1, "pos": [ax * MM, ay * MM]}]);
            } else {
                fx.record(n)["anchor"] = json!({"named": [ax * MM, ay * MM]});
            }
        }
        let m = fx.build();
        assert_anchor_cells_agree(&m, "random");
    }
}
