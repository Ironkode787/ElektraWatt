// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The importer's copy of the carrier-path rule (`ew_import_tables::carrier_path_problem`; an importer may not depend
//! on `ew-derive`) refuses what the rule (`ew_derive::layout::check_path`) refuses, for the same segment and in the
//! rule's words, over the kinds the importer reads.

use ew_derive::layout::check_path;
use ew_import_tables::carrier_path_problem;
use ew_schema::CarrierKind;
use ew_units::{Nm, Pt2};

fn path(points: &[(i64, i64)]) -> Vec<Pt2> {
    points.iter().map(|&(x, y)| Pt2::new(Nm(x), Nm(y))).collect()
}

#[test]
fn the_importers_carrier_check_is_the_rules() {
    let paths: &[&[(i64, i64)]] = &[
        &[],
        &[(0, 0)],
        &[(0, 0), (35_000_000, 0)],
        &[(0, 0), (0, -35_000_000)],
        &[(0, 0), (0, 0)],
        &[(0, 0), (10, 10)],
        &[(0, 0), (10, 0), (10, 20)],
        &[(0, 0), (10, 0), (10, 0), (10, 20)],
        &[(0, 0), (10, 0), (10, 20), (30, 40)],
    ];
    for kind in [CarrierKind::Rail, CarrierKind::Duct] {
        for points in paths {
            let p = path(points);
            let copy = carrier_path_problem(&kind, &p);
            let rule = check_path(&kind, &p).err().map(|e| e.to_string());
            assert_eq!(
                copy.is_some(),
                rule.is_some(),
                "{kind:?} {points:?}: {copy:?} against {rule:?}"
            );
            if let (Some(copy), Some(rule)) = (copy, rule) {
                assert!(
                    rule.starts_with(&copy),
                    "{kind:?} {points:?}: {copy:?} against {rule:?}"
                );
            }
        }
    }
}
