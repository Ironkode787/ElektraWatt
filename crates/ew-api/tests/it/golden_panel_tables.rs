// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On Golden A: `Session::panel_tables` of the panel page 902: the carriers by their labels, R2's
//! end brackets, the free lengths of R1 and R2, R1's items, the five unplaced devices with their
//! badges and mount kinds, the slot of -201U9 on R3, the inverse projection of every
//! placement's clip point, and `None` for a schematic page.

use crate::golden_common;

use ew_api::output::{PanelTables, surface_of_page};
use ew_derive::ew_ids::Id;
use ew_import_tables::ids::record_id;
use ew_schema::{Device, MountKind, Placement};

const MM: i128 = 1_000_000;

/// An unplaced device's label, strip and assembly badges, size and mount label.
type UnplacedRow<'a> = (&'a str, bool, bool, Option<(i128, i128)>, &'a str);

/// A navigator node's parent, label, badges and cells.
type NodeRow<'a> = (Option<u32>, &'a str, Vec<&'a str>, Vec<&'a str>);

fn dev(key: &str) -> Id<Device> {
    record_id("devices", key)
}

fn placement(key: &str) -> Id<Placement> {
    record_id("placements", key)
}

fn tables() -> PanelTables {
    let s = golden_common::session(&golden_common::batch());
    s.panel_tables("902")
        .expect("902 is a panel page with a surface view")
}

fn mm(v: f64) -> i128 {
    (v * 1_000_000.0).round() as i128
}

#[test]
fn the_carriers_are_labelled_with_their_rail_data() {
    let t = tables();
    let labels: Vec<&str> = t.carriers.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Rail 1", "Rail 2", "Rail 3", "Duct 1", "Duct 2", "Duct 3", "Duct 4", "Duct 5"
        ]
    );
    assert_eq!(t.carriers[0].sentence, "rail 1");
    assert_eq!(t.carriers[0].kind, "rail");
    assert_eq!(t.carriers[3].kind, "duct");
    // SN-1: R2's four brackets.
    assert_eq!(
        t.carriers[1].brackets,
        vec![
            (20 * MM, 30 * MM),
            (mm(48.6), mm(58.6)),
            (mm(162.6), mm(172.6)),
            (mm(198.6), mm(208.6)),
        ]
    );
    // D12: the free lengths.
    assert_eq!(t.carriers[0].free_length, Some(341 * MM));
    assert_eq!(t.carriers[1].free_length, Some(mm(381.4)));
    // SN-2: R1's six intervals.
    let r1: Vec<(i128, i128)> = t.carriers[0].items.iter().map(|i| (i.s0, i.s1)).collect();
    assert_eq!(
        r1,
        vec![
            (61 * MM, 131 * MM),
            (131 * MM, 149 * MM),
            (149 * MM, 167 * MM),
            (167 * MM, 212 * MM),
            (212 * MM, 245 * MM),
            (245 * MM, 290 * MM),
        ]
    );
    // A duct has no rail data.
    assert_eq!(t.carriers[3].axis, None);
    assert!(t.carriers[3].items.is_empty());
    assert_eq!(t.carriers[3].free_length, None);
    assert_eq!(t.location, "+J-E-2416");
}

#[test]
fn the_unplaced_devices_come_in_unplaced_order_with_their_badges() {
    let t = tables();
    let rows: Vec<UnplacedRow> = t
        .unplaced
        .iter()
        .map(|u| {
            (
                u.label.as_str(),
                u.strip,
                u.assembly,
                u.size,
                u.mount_label.as_str(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("-201U9", false, true, Some((20 * MM, 107 * MM)), "TS 35×7.5"),
            ("E55", false, false, Some((50 * MM, 120 * MM)), "screw-mounted"),
            ("E56", false, false, None, ""),
            ("F55", false, false, Some((18 * MM, 90 * MM)), "TS 35×7.5"),
            ("S56", false, false, None, ""),
        ]
    );
    let f55 = &t.unplaced[3];
    assert_eq!(f55.device, dev("F55").as_uuid());
    assert_eq!(f55.part_number.as_deref(), Some("5SY4204-7"));
    assert_eq!(f55.mount_kind, Some(MountKind::Ts35x7_5));
    assert_eq!(f55.extent.width, 18 * MM);
    // PW-5: -201U9's slot on R3 (its predecessor -201U8 is there), on no other rail.
    let r3 = t.carriers[2].id;
    let u9 = &t.unplaced[0];
    assert_eq!(u9.slots.len(), 1);
    assert_eq!(u9.slots[0].0, r3);
}

/// Every placement's clip point (a rail's `s` on its axis, a surface's `(u, v)`), projected to the page and inverted,
/// comes back within 1 nm.
#[test]
fn the_inverse_projection_returns_every_clip_point() {
    let t = tables();
    assert_eq!(t.placements.len(), 20);
    for p in &t.placements {
        let point = match p.carrier {
            Some(c) => {
                let axis = t.carrier(c).and_then(|c| c.axis).expect("a rail with an axis");
                axis.point_at(i128::from(p.pose.0))
            }
            None => (i128::from(p.pose.0), i128::from(p.pose.1)),
        };
        let page = t.to_page(point);
        let back = surface_of_page(&t.projection, page);
        // The page is 1:10 on paper, so a page nm stands for about ten surface nm: the inverse lands within one page
        // nm's worth of the clip point, and projecting it again gives the page point back within 1 nm.
        let again = t.to_page(back);
        assert!(
            (i128::from(again.0) - i128::from(page.0)).abs() <= 1
                && (i128::from(again.1) - i128::from(page.1)).abs() <= 1,
            "{}: {point:?} -> {page:?} -> {back:?} -> {again:?}",
            p.tag
        );
        let tol = t.projection.den / t.projection.num.max(1) + 1;
        assert!(
            (back.0 - point.0).abs() <= tol && (back.1 - point.1).abs() <= tol,
            "{}: {point:?} -> {page:?} -> {back:?}",
            p.tag
        );
    }
    let f1 = t
        .placements
        .iter()
        .find(|p| p.placement == placement("-102F1"))
        .expect("-102F1 is placed");
    assert_eq!(f1.tag, "-102F1");
    assert_eq!(f1.carrier, Some(t.carriers[0].id));
}

#[test]
fn a_schematic_page_has_no_panel_tables() {
    let s = golden_common::session(&golden_common::batch());
    assert_eq!(s.panel_tables("101"), None);
    assert_eq!(s.panel_tables("no such folio"), None);
}

/// PW-1 in `ew-api`: the Unplaced devices navigator lists the five devices under +J-E-2416 in `unplaced`
/// order (ruling 9: PW-1's list read as a set) with their part, size and mount kind and their badges; after
/// `layout.place` of F55 its row is gone in the same revision.
#[test]
fn the_unplaced_navigator_lists_the_five_devices() {
    use ew_api::output::NavKind;
    let mut s = golden_common::session(&golden_common::batch());
    let t = s.navigator_table(NavKind::Unplaced);
    let columns: Vec<&str> = t.columns.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(columns, ["Device", "Part", "W × H (mm)", "Mount"]);
    assert!(t.columns.iter().all(|c| c.derived && c.route.is_none()));
    let rows: Vec<NodeRow> = t
        .nodes
        .iter()
        .map(|n| {
            (
                n.parent,
                n.label.as_str(),
                n.badges.iter().map(String::as_str).collect(),
                n.cells.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (None, "+J-E-2416", vec![], vec![]),
            (
                Some(0),
                "-201U9",
                vec!["assembly"],
                vec!["-201U9", "533354", "20 × 107", "TS 35×7.5"]
            ),
            (
                Some(0),
                "E55",
                vec![],
                vec!["E55", "SK 3102.000", "50 × 120", "screw-mounted"]
            ),
            (Some(0), "E56", vec!["unknown size"], vec!["E56", "", "", ""]),
            (
                Some(0),
                "F55",
                vec![],
                vec!["F55", "5SY4204-7", "18 × 90", "TS 35×7.5"]
            ),
            (Some(0), "S56", vec!["unknown size"], vec!["S56", "", "", ""]),
        ]
    );
    let tables = s.panel_tables("902").expect("902");
    let r1 = tables.carriers[0].carrier;
    let cmd = ew_api::AnyCommand {
        name: "layout.place".into(),
        params: serde_json::json!({
            "device": dev("F55"),
            "carrier": {"carrier": r1},
            "pose": {"t": [320_000_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]},
            "mode": "free",
        }),
    };
    s.execute(&cmd).expect("F55 places");
    let after = s.navigator_table(NavKind::Unplaced);
    let labels: Vec<&str> = after.nodes.iter().map(|n| n.label.as_str()).collect();
    assert_eq!(labels, ["+J-E-2416", "-201U9", "E55", "E56", "S56"]);
    assert_eq!(after.rev, t.rev + 1);
}
