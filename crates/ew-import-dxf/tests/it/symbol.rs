// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The DXF→symbol wizard's automatic steps (doc 14 §6.2, FR-IMP-034) and the part outline (FR-IMP-035).

use ew_import_dxf::ew_interop_api::Limits;
use ew_import_dxf::ew_interop_api::ew_schema::Dir4;
use ew_import_dxf::{outline, symbols};
use ew_units::{Grid, Nm, Pt2};

use crate::common::Dxf;

/// A drawing with block `B`: a 12 × 8 rectangle, and `extra` pairs inside the block.
fn block(extra: &[(i32, &str)]) -> Vec<u8> {
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("BLOCKS");
    d.ps(&[(0, "BLOCK"), (8, "0"), (2, "B"), (70, "2"), (10, "0"), (20, "0")]);
    d.ps(&[(0, "LWPOLYLINE"), (8, "0"), (90, "4"), (70, "1"), (10, "0"), (20, "0"), (10, "12"), (20, "0")]);
    d.ps(&[(10, "12"), (20, "8"), (10, "0"), (20, "8")]);
    d.ps(extra);
    d.ps(&[(0, "ENDBLK"), (8, "0")]);
    d.end();
    d.ascii()
}

fn attdef<'a>(tag: &'a str, x: &'a str, y: &'a str, default: &'a str) -> Vec<(i32, &'a str)> {
    vec![(0, "ATTDEF"), (8, "0"), (10, x), (20, y), (40, "1"), (1, default), (2, tag), (70, "1")]
}

#[test]
fn pin_attribute_definitions_become_four_points_on_the_grid_with_their_directions_and_the_tag_slot() {
    let mut extra = Vec::new();
    for (tag, x, y) in [("PIN1", "0", "4"), ("PIN2", "12", "4"), ("PIN_3", "4", "0"), ("PIN4", "8", "8")] {
        extra.extend(attdef(tag, x, y, ""));
    }
    extra.extend(attdef("TAG", "0", "10", "-K?"));
    let s = symbols(&block(&extra), &Limits::default(), None, Grid::MM4, "v.dxf").unwrap();
    assert_eq!(s.len(), 1);
    let b = &s[0];
    let p = |x: i64, y: i64| Pt2::new(Nm(x * 1_000_000), Nm(y * 1_000_000));
    let pins: Vec<(&str, Pt2, &Dir4)> = b.pins.iter().map(|p| (p.designation.as_str(), p.pos, &p.dir)).collect();
    // y down in the symbol frame: the drawing's y = 8 is −8.
    assert_eq!(
        pins,
        [
            ("1", p(0, -4), &Dir4::Left),
            ("2", p(12, -4), &Dir4::Right),
            ("3", p(4, 0), &Dir4::Down),
            ("4", p(8, -8), &Dir4::Up),
        ]
    );
    assert_eq!(b.tag.as_ref().map(|t| t.pos), Some(p(0, -10)));
    assert!(b.entries.is_empty(), "{:?}", b.entries);
}

#[test]
fn points_on_a_connection_layer_are_the_fallback_and_a_point_off_the_grid_is_snapped_and_reported() {
    let extra = [
        (0, "POINT"),
        (8, "CONN"),
        (10, "0"),
        (20, "3"),
        (0, "CIRCLE"),
        (8, "EW-CP"),
        (10, "12"),
        (20, "4"),
        (40, "0.5"),
    ];
    let s = symbols(&block(&extra), &Limits::default(), None, Grid::MM4, "v.dxf").unwrap();
    let b = &s[0];
    assert_eq!(b.pins.len(), 2);
    assert_eq!(b.pins[0].pos, Pt2::new(Nm(0), Nm(-4_000_000)), "(0, 3) snaps to (0, 4) on the 4 mm grid");
    let snapped: Vec<String> = b.entries.iter().map(|e| e.code.to_string()).collect();
    assert_eq!(snapped, ["EW-IMP-256"]);
}

#[test]
fn a_block_without_connection_points_says_so() {
    let s = symbols(&block(&[]), &Limits::default(), None, Grid::MM4, "v.dxf").unwrap();
    assert!(s[0].pins.is_empty());
    assert_eq!(s[0].entries[0].code.to_string(), "EW-IMP-004");
}

#[test]
fn the_outline_is_in_the_part_frame_from_the_lower_left_corner() {
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("ENTITIES");
    d.ps(&[(0, "LWPOLYLINE"), (8, "0"), (90, "4"), (70, "1"), (10, "100"), (20, "50"), (10, "145"), (20, "50")]);
    d.ps(&[(10, "145"), (20, "140"), (10, "100"), (20, "140")]);
    d.end();
    let lines = outline(&d.ascii(), &Limits::default(), None).unwrap();
    let mm = |x: i64, y: i64| Pt2::new(Nm(x * 1_000_000), Nm(y * 1_000_000));
    assert_eq!(lines, [vec![mm(0, 0), mm(45, 0), mm(45, 90), mm(0, 90), mm(0, 0)]]);
}
