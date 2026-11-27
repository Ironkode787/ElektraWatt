// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A single gland dropped near a row of glands snaps to the row's end at the pitch of the two nuts and the web between
//! holes (doc 12 §3.9); dropped far from it, it stays where it is dropped.

use crate::common;

const MM: i128 = 1_000_000;

#[test]
fn a_single_gland_snaps_to_the_end_of_a_row_and_stays_where_dropped_far_from_it() {
    let s = common::sample();
    let q = s.query();
    let bottom = q
        .table::<ew_schema::MountingSurface>()
        .iter()
        .find(|(_, f)| f.kind == ew_schema::SurfaceKind::Bottom)
        .map(|(id, _)| id.as_uuid())
        .expect("the sample's bottom");
    let cable = s.resolve_reference("tag:-103W1").expect("-103W1");
    drop(q);
    // -101W1's M20 stands at (80, 125) mm; -103W1's M25 goes 35 mm to its right (half of 27 and 33 mm nuts, a 5 mm
    // web), its own placed hole left out of the row.
    assert_eq!(
        s.gland_snap(bottom, cable, (112 * MM, 131 * MM)),
        (115 * MM, 125 * MM)
    );
    assert_eq!(
        s.gland_snap(bottom, cable, (52 * MM, 120 * MM)),
        (45 * MM, 125 * MM),
        "the row's left end"
    );
    assert_eq!(
        s.gland_snap(bottom, cable, (300 * MM, 200 * MM)),
        (300 * MM, 200 * MM)
    );
}
