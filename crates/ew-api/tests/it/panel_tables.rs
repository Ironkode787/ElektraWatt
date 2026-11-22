// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On the sample: `Session::panel_tables` of the panel page 902: the carriers by their labels, the rails' items in their
//! auto-filled run and their free lengths, an unplaced device with its size and mount kind (the set-up removes
//! -102F1's placement: the sample places every device), the inverse projection of every placement's clip point, and
//! `None` for a schematic page.

use crate::common;

use ew_api::Session;
use ew_api::output::{PanelTables, surface_of_page};
use ew_schema::{MountKind, Record};

const MM: i128 = 1_000_000;

/// The length of each of the sample's two rails: the parts list's 0.94 m of rail over two.
const RAIL: i128 = 470 * MM;

/// A navigator node's parent, label, badges and cells.
type NodeRow<'a> = (Option<u32>, &'a str, Vec<&'a str>, Vec<&'a str>);

fn tables() -> PanelTables {
    common::sample()
        .panel_tables("902")
        .expect("902 is a panel page with a surface view")
}

/// The sample without -102F1's placement.
fn without_f1_placement() -> Session {
    let s = common::sample();
    let f1 = s.resolve_reference("tag:-102F1").expect("-102F1");
    let (project, mut records, lib) = common::sample_records();
    records.retain(|r| !matches!(r, Record::Placement(p) if p.device.as_uuid() == f1));
    common::session_of(project, records, lib)
}

#[test]
fn the_carriers_are_labelled_with_their_rail_data() {
    let t = tables();
    let labels: Vec<&str> = t.carriers.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["Rail 1", "Rail 2", "Duct 1"]);
    assert_eq!(t.carriers[0].sentence, "rail 1");
    assert_eq!(t.carriers[0].kind, "rail");
    assert_eq!(t.carriers[2].kind, "duct");
    // The sample's strips declare no end-bracket part: no bracket is derived.
    assert!(t.carriers.iter().all(|c| c.brackets.is_empty()));
    // Auto-fill rail runs each rail's items edge to edge from its start; the free length is what the run leaves.
    for rail in &t.carriers[..2] {
        let items: Vec<(i128, i128)> = rail.items.iter().map(|i| (i.s0, i.s1)).collect();
        assert_eq!(items.first().map(|i| i.0), Some(0), "{}", rail.label);
        assert!(
            items.windows(2).all(|w| w[0].1 == w[1].0),
            "{}: {items:?}",
            rail.label
        );
        let end = items.last().map_or(0, |i| i.1);
        assert_eq!(rail.free_length, Some(RAIL - end), "{}", rail.label);
    }
    // A duct has no rail data.
    assert_eq!(t.carriers[2].axis, None);
    assert!(t.carriers[2].items.is_empty());
    assert_eq!(t.carriers[2].free_length, None);
    assert_eq!(t.location, "+CAB");
}

#[test]
fn an_unplaced_device_comes_with_its_size_and_mount_kind() {
    let s = without_f1_placement();
    let t = s.panel_tables("902").expect("902");
    let [f1] = t.unplaced.as_slice() else {
        panic!("one unplaced device: {:?}", t.unplaced)
    };
    assert_eq!(
        (f1.label.as_str(), f1.strip, f1.assembly, f1.mount_label.as_str()),
        ("-102F1", false, false, "TS 35×7.5")
    );
    assert_eq!(f1.device, s.resolve_reference("tag:-102F1").expect("-102F1"));
    assert_eq!(f1.part_number.as_deref(), Some("EW-GEN-MCB-1"));
    assert_eq!(f1.mount_kind, Some(MountKind::Ts35x7_5));
    let (width, _) = f1.size.expect("the part's size");
    assert_eq!(f1.extent.width, width);
    // A slot is a member's place after its predecessor in an assembly: a breaker has none.
    assert!(f1.slots.is_empty());
}

/// Every placement's clip point (a rail's `s` on its axis, a surface's `(u, v)`), projected to the page and inverted,
/// comes back within 1 nm.
#[test]
fn the_inverse_projection_returns_every_clip_point() {
    let t = tables();
    assert!(!t.placements.is_empty());
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
        // The page is 1:5 on paper, so a page nm stands for about five surface nm: the inverse lands within one page
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
        .find(|p| p.tag == "-102F1")
        .expect("-102F1 is placed");
    assert!(f1.carrier.is_some(), "on a rail");
}

#[test]
fn a_schematic_page_has_no_panel_tables() {
    let s = common::sample();
    assert_eq!(s.panel_tables("101"), None);
    assert_eq!(s.panel_tables("no such folio"), None);
}

/// PW-1 in `ew-api`: the Unplaced devices navigator lists the unplaced device under +CAB with its part, size and mount
/// kind; after `layout.place` of -102F1 on rail 2 its row is gone in the same revision.
#[test]
fn the_unplaced_navigator_lists_the_unplaced_device() {
    use ew_api::output::NavKind;
    let mut s = without_f1_placement();
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
    let tables = s.panel_tables("902").expect("902");
    let (w, h) = tables.unplaced[0].size.expect("its size");
    let size = format!("{} × {}", w / MM, h / MM);
    assert_eq!(
        rows,
        vec![
            (None, "+CAB", vec![], vec![]),
            (
                Some(0),
                "-102F1",
                vec![],
                vec!["-102F1", "EW-GEN-MCB-1", size.as_str(), "TS 35×7.5"]
            ),
        ]
    );
    let r2 = tables.carriers[1].carrier;
    let f1 = s.resolve_reference("tag:-102F1").expect("-102F1");
    let cmd = ew_api::AnyCommand {
        name: "layout.place".into(),
        params: serde_json::json!({
            "device": f1.to_string(),
            "carrier": {"carrier": r2},
            "pose": {"t": [400_000_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]},
            "mode": "free",
        }),
    };
    s.execute(&cmd).expect("-102F1 places");
    let after = s.navigator_table(NavKind::Unplaced);
    assert!(
        after.nodes.iter().all(|n| n.label != "-102F1"),
        "{:?}",
        after.nodes.iter().map(|n| &n.label).collect::<Vec<_>>()
    );
    assert_eq!(after.rev, t.rev + 1);
}
