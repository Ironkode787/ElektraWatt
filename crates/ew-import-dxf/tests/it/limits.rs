// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The limits of a hostile drawing (doc 14 §11): one memory budget for the pairs the file holds and everything the
//! conversion expands, copies and flattens; handles that cannot grow a key; loops that end with their data.

use std::collections::BTreeSet;

use ew_import_dxf::ew_interop_api::{ImportError, Limits};
use ew_import_dxf::{MAX_BUDGET, Space, convert};

use crate::common::Dxf;

fn read(d: &Dxf) -> Result<ew_import_dxf::Converted, ImportError> {
    convert(
        &d.ascii(),
        &Limits::default(),
        None,
        &Space::Model,
        &BTreeSet::new(),
        "a.dxf",
    )
}

/// The refusal's subject.
fn refused(d: &Dxf) -> String {
    match read(d) {
        Err(ImportError::LimitExceeded { what, .. }) => what,
        other => panic!("expected a limit, got {:?}", other.map(|c| c.items.len())),
    }
}

/// A drawing of the blocks `(name, entities)` whose model space holds `model`.
fn drawing(blocks: &[(&str, &[(i32, &str)])], model: &[(i32, &str)]) -> Dxf {
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("BLOCKS");
    for (name, body) in blocks {
        d.ps(&[(0, "BLOCK"), (8, "0"), (2, *name), (70, "0"), (10, "0"), (20, "0")]);
        d.ps(body);
        d.ps(&[(0, "ENDBLK"), (8, "0")]);
    }
    d.end();
    d.section("ENTITIES");
    d.ps(model);
    d.end();
    d
}

#[test]
fn a_block_that_inserts_itself_twice_is_refused_by_the_budget() {
    // Two inserts of itself at every level: 2^32 expansions before the nesting limit, without a shape to count.
    let twice = [(0, "INSERT"), (8, "0"), (2, "A"), (10, "1"), (20, "0")];
    let body: Vec<(i32, &str)> = twice.iter().chain(twice.iter()).copied().collect();
    let d = drawing(
        &[("A", &body)],
        &[(0, "INSERT"), (8, "0"), (2, "A"), (10, "0"), (20, "0")],
    );
    assert_eq!(refused(&d), "DXF memory budget");
}

#[test]
fn nested_arrays_of_inserts_are_refused_by_the_budget() {
    // 1000 × 1000 cells of a block that holds 1000 × 1000 cells of another, which is empty: each array is within the
    // expansion limit, the arrays together are 10^12 cells.
    let array = |name: &'static str| {
        vec![
            (0, "INSERT"),
            (8, "0"),
            (2, name),
            (10, "0"),
            (20, "0"),
            (70, "1000"),
            (71, "1000"),
            (44, "1"),
            (45, "1"),
        ]
    };
    let inner = array("EMPTY");
    let d = drawing(&[("EMPTY", &[]), ("CELL", &inner)], &array("CELL"));
    assert_eq!(refused(&d), "DXF memory budget");
}

#[test]
fn more_pairs_than_the_budget_holds_are_refused_before_they_are_held() {
    // Each entity costs the budget more than 100 bytes, so this is over it by a wide margin.
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("ENTITIES");
    for _ in 0..MAX_BUDGET / 100 {
        d.p(0, "POINT");
    }
    d.end();
    assert_eq!(refused(&d), "DXF memory budget");
}

#[test]
fn a_handle_that_is_not_one_cannot_grow_the_keys() {
    // A block of one line holding a handle of 100,000 characters, inserted at 32 levels of nesting: each level of
    // keys would carry the handle again.
    let long = "A".repeat(100_000);
    let mut blocks: Vec<(String, Vec<(i32, String)>)> = vec![(
        "L0".to_owned(),
        vec![
            (0, "LINE".to_owned()),
            (5, long.clone()),
            (8, "0".to_owned()),
            (10, "0".to_owned()),
            (20, "0".to_owned()),
            (11, "1".to_owned()),
            (21, "1".to_owned()),
        ],
    )];
    for level in 1..30 {
        blocks.push((
            format!("L{level}"),
            vec![
                (0, "INSERT".to_owned()),
                (5, long.clone()),
                (8, "0".to_owned()),
                (2, format!("L{}", level - 1)),
                (10, "1".to_owned()),
                (20, "0".to_owned()),
            ],
        ));
    }
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("BLOCKS");
    for (name, body) in &blocks {
        d.ps(&[(0, "BLOCK"), (8, "0"), (2, name.as_str()), (70, "0"), (10, "0"), (20, "0")]);
        for (c, v) in body {
            d.p(*c, v);
        }
        d.ps(&[(0, "ENDBLK"), (8, "0")]);
    }
    d.end();
    d.section("ENTITIES");
    d.ps(&[(0, "INSERT"), (5, "1A"), (8, "0"), (2, "L29"), (10, "0"), (20, "0")]);
    d.end();
    let c = read(&d).unwrap();
    assert_eq!(c.items.len(), 1, "the line is imported");
    assert!(c.items[0].0.len() < 1000, "its key does not hold the handles");
}

#[test]
fn a_hatch_whose_counts_outrun_its_pairs_ends_with_them() {
    // `91 10000`, then 10,000 loops each claiming 100,000 edges and holding none: about 10^9 iterations if a count
    // were believed.
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("ENTITIES");
    d.ps(&[(0, "HATCH"), (8, "0"), (2, "SOLID"), (70, "1"), (91, "10000")]);
    for _ in 0..10_000 {
        d.ps(&[(92, "0"), (93, "100000")]);
    }
    d.end();
    let c = read(&d).unwrap();
    assert!(c.items.is_empty(), "no loop has an edge");
}

#[test]
fn nul_is_no_character_of_an_imported_text() {
    let mut d = Dxf::new("AC1015", Some(4));
    d.section("ENTITIES");
    d.ps(&[(0, "TEXT"), (8, "0"), (10, "0"), (20, "0"), (40, "2.5"), (1, "A%%000B")]);
    d.end();
    let c = read(&d).unwrap();
    let text = c.items.iter().find_map(|(_, _, k)| match k {
        ew_import_dxf::ew_interop_api::ew_schema::PageItemKind::FreeText(t) => {
            t.text.as_neutral().map(str::to_owned)
        }
        _ => None,
    });
    assert_eq!(text.as_deref(), Some("AB"));
}
