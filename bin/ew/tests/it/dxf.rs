// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew import dxf` (doc 14 §13): a drawing as a new graphic page of a project, again as a second page, refused in words
//! without a unit; and its blocks as symbol drafts with their connection points on the native grid (FR-IMP-034).

use crate::support::{EwTmp, Tmp, args, stderr, stdout};

/// A DXF of `$INSUNITS` `units` (none when `None`) with a block `TERM4` of a rectangle and four pin attribute
/// definitions, one insert of it and a line.
fn drawing(units: Option<u8>) -> String {
    let mut pairs: Vec<(u16, String)> = vec![(0, "SECTION".into()), (2, "HEADER".into())];
    pairs.extend([(9, "$ACADVER".into()), (1, "AC1015".into())]);
    if let Some(u) = units {
        pairs.extend([(9, "$INSUNITS".into()), (70, u.to_string())]);
    }
    pairs.extend([(0, "ENDSEC".into()), (0, "SECTION".into()), (2, "BLOCKS".into())]);
    pairs.extend([(0, "BLOCK".into()), (8, "0".into()), (2, "TERM4".into()), (70, "2".into())]);
    pairs.extend([(10, "0".into()), (20, "0".into())]);
    pairs.extend([(0, "LWPOLYLINE".into()), (8, "0".into()), (90, "4".into()), (70, "1".into())]);
    for (x, y) in [(0, 0), (12, 0), (12, 8), (0, 8)] {
        pairs.extend([(10, x.to_string()), (20, y.to_string())]);
    }
    for (n, x) in [(1, 0), (2, 4), (3, 8), (4, 12)] {
        pairs.extend([(0, "ATTDEF".into()), (8, "0".into()), (10, x.to_string()), (20, "0".into())]);
        pairs.extend([(40, "1".into()), (1, String::new()), (3, "Pin".into()), (2, format!("PIN{n}"))]);
        pairs.push((70, "1".into()));
    }
    pairs.extend([(0, "ENDBLK".into()), (8, "0".into()), (0, "ENDSEC".into())]);
    pairs.extend([(0, "SECTION".into()), (2, "ENTITIES".into())]);
    pairs.extend([(0, "INSERT".into()), (5, "1A".into()), (8, "0".into()), (2, "TERM4".into())]);
    pairs.extend([(10, "50".into()), (20, "50".into())]);
    pairs.extend([(0, "LINE".into()), (5, "1B".into()), (8, "WIRES".into()), (10, "0".into()), (20, "0".into())]);
    pairs.extend([(11, "100".into()), (21, "0".into()), (0, "ENDSEC".into()), (0, "EOF".into())]);
    pairs.iter().map(|(c, v)| format!("{c}\n{v}\n")).collect()
}

#[test]
fn a_drawing_becomes_a_graphic_page_twice_and_a_missing_unit_is_refused_in_words() {
    let t = Tmp::new("ew-dxf");
    let project = t.tiny("p.ew");
    let file = t.path("plant.dxf");
    std::fs::write(&file, drawing(Some(4))).unwrap();
    let out = t.ew(&args([&"import", &"dxf", &file, &"--project", &project]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).ends_with("Imported plant.dxf as page 2 with 2 items.\n"), "{}", stdout(&out));
    // The same file again: a copy on the next page.
    let out = t.ew(&args([&"import", &"dxf", &file, &"--project", &project]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).ends_with("as page 3 with 2 items.\n"), "{}", stdout(&out));
    let (reopened, _) = ew_api::Session::open_read_only(&project, "test").unwrap();
    let graphic = reopened
        .query()
        .pages()
        .iter()
        .filter(|(_, p)| p.kind == ew_api::symbols::schema::PageType::Graphic)
        .count();
    assert_eq!(graphic, 2);

    let bare = t.path("bare.dxf");
    std::fs::write(&bare, drawing(None)).unwrap();
    let out = t.ew(&args([&"import", &"dxf", &bare, &"--project", &project]));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        stderr(&out),
        "error: the drawing names no unit; give it with --units (mm, cm, m, in, …)\n"
    );
    let out = t.ew(&args([&"import", &"dxf", &bare, &"--project", &project, &"--units", &"mm"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).contains("EW-IMP-251 warning: The drawing names no unit; it was read in millimetres as chosen."));
}

#[test]
fn the_blocks_of_a_drawing_become_symbol_drafts_with_their_pins_on_the_grid() {
    let t = Tmp::new("ew-dxf-symbol");
    let file = t.path("vendor.dxf");
    std::fs::write(&file, drawing(Some(4))).unwrap();
    let out = t.ew(&args([&"import", &"dxf", &file, &"--as", &"symbol"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let drafts: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let draft = &drafts[0];
    let cps = draft["symbol"]["cps"].as_array().unwrap();
    assert_eq!(cps.len(), 4);
    for cp in cps {
        for v in cp["pos"].as_array().unwrap() {
            assert_eq!(v.as_i64().unwrap() % 4_000_000, 0, "{cp} lies on the 4 mm grid");
        }
    }
    let names: Vec<&str> = draft["function"]["cps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["designation"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["1", "2", "3", "4"]);
    assert_eq!(draft["symbol"]["meta"]["foreign_terms"], true);
}
