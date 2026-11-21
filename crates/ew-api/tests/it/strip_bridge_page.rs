// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sample's control page 102 draws the bridges of -X1021 (FT-1908, doc 08 §6.5): terminals 1 to 4 and 5 to 8 are
//! two bridges, each four terminals in one row, and each is drawn as a line across its terminals' bridged leads with a
//! dot on each, in the wires layer that every writer draws.

use crate::common;

use ew_api::output::{TableCp, page_layer};
use ew_derive::PageOrder;
use ew_dl::{ItemKind, Layer, PathEl};
use ew_units::{Nm, Pt2};

#[test]
fn page_102_draws_the_bridges_of_x1021_across_its_terminals() {
    let s = common::sample();
    let t = s.page_tables("102").expect("folio 102");
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let entry = order
        .entries
        .iter()
        .find(|e| e.folio == "102")
        .expect("folio 102");
    let wires = page_layer(&q, entry.page, Layer::Wires);
    // Each bridge joins clamp a (CP 0) of its terminals: the point half way between the terminal's centre and that
    // clamp.
    let mid = |designation: &str| {
        let label = format!("-X1021:{designation}/a");
        let cp: &TableCp = t
            .cps
            .iter()
            .find(|c| c.label == label && c.idx == 0)
            .unwrap_or_else(|| panic!("{label}"));
        let rep = t.reps.iter().find(|r| r.id == cp.rep).expect("its terminal");
        let (c, at) = (rep.pos, cp.at);
        Pt2::new(
            Nm(c.0 + (at.0 - c.0).div_euclid(2)),
            Nm(c.1 + (at.1 - c.1).div_euclid(2)),
        )
    };
    let unowned: Vec<_> = wires
        .list
        .items
        .iter()
        .filter(|i| i.entity.is_none())
        .filter_map(|i| match &i.kind {
            ItemKind::Path(p) => Some(p),
            _ => None,
        })
        .collect();
    for group in [["1", "2", "3", "4"], ["5", "6", "7", "8"]] {
        let points = group.map(mid);
        assert!(
            points.iter().all(|p| p.y == points[0].y),
            "the four stand in one row: {points:?}"
        );
        let line = [PathEl::MoveTo(points[0]), PathEl::LineTo(points[3])];
        assert!(
            unowned.iter().any(|p| p.path.els == line && p.stroke.is_some()),
            "the bridge of {group:?} as one line"
        );
        for p in points {
            assert!(
                unowned.iter().any(|d| d.fill.is_some()
                    && d.path.bounds().is_some_and(|b| {
                        let half = |a: Nm, z: Nm| Nm((a.0 + z.0) / 2);
                        Pt2::new(half(b.min().x, b.max().x), half(b.min().y, b.max().y)) == p
                    })),
                "a dot at {p:?}"
            );
        }
    }
}
