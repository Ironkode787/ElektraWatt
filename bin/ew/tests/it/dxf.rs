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

/// `ew import pdf` keeps the PDF in the user's underlay cache, not in the project; `ew import dxf --as underlay
/// --embed` puts the drawing into the project; `ew export dxf` writes the underlay's layer only with `--underlays`.
#[test]
fn underlays_are_placed_kept_outside_or_embedded_and_printed_only_when_asked() {
    let t = Tmp::new("ew-dxf-underlay");
    let project = t.tiny("p.ew");
    let pdf_file = t.path("plan.pdf");
    let out = t.ew(&args([&"export", &"pdf", &project, &"--out", &pdf_file]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let out = t.ew(&args([&"import", &"pdf", &pdf_file, &"--on", &"1", &"--project", &project]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out), "Placed plan.pdf as an underlay on page 1, kept outside the project.\n");
    let cached = std::fs::read_dir(t.path("cache").join("underlays")).unwrap().count();
    assert_eq!(cached, 1, "the PDF is in the underlay cache");
    assert!(!project.join("assets").exists() || std::fs::read_dir(project.join("assets")).unwrap().count() == 0);
    let out = t.ew(&args([&"import", &"pdf", &pdf_file, &"--on", &"7", &"--project", &project]));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stderr(&out), "error: The project has no drawn page 7.\n");

    let dxf = t.path("plant.dxf");
    std::fs::write(&dxf, drawing(Some(4))).unwrap();
    let out = t.ew(&args([&"import", &"dxf", &dxf, &"--as", &"underlay", &"--on", &"1", &"--embed", &"--project", &project]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).ends_with("embedded in the project.\n"), "{}", stdout(&out));
    assert_eq!(std::fs::read_dir(project.join("assets")).unwrap().count(), 1);

    let plain = t.path("plain");
    let out = t.ew(&args([&"export", &"dxf", &project, &"--out", &plain]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = std::fs::read_to_string(plain.join("1.dxf")).unwrap();
    assert!(!text.contains("  8\nEW-UNDERLAY\n"), "no underlay unless printed");
    let printed = t.path("printed");
    let out = t.ew(&args([&"export", &"dxf", &project, &"--out", &printed, &"--underlays"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = std::fs::read_to_string(printed.join("1.dxf")).unwrap();
    assert!(text.contains("  8\nEW-UNDERLAY\n"), "the printed underlay on its layer");
}

/// `ew import dxf --as outline` writes the drawing's lines as a library part's front outline, its next patch version.
#[test]
fn a_drawing_becomes_a_parts_front_outline() {
    let t = Tmp::new("ew-dxf-outline");
    let lib = t.path("data").join("libraries").join("std.ewlib");
    let out = t.ew(&args([&"lib", &"create", &lib, &"--key", &"std", &"--name", &"Standard", &"--licence", &"CC0-1.0"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let list = t.path("supplier.csv");
    let map = t.path("supplier.toml");
    std::fs::write(&list, "Maker,Order code,Text,Category\nPhoenix Contact,3209510,Feed-through terminal,terminal\n").unwrap();
    std::fs::write(&map, "[columns]\nmanufacturer = \"Maker\"\npart_number = \"Order code\"\ndescription = \"Text\"\nkind = \"Category\"\n").unwrap();
    let out = t.ew(&args([&"lib", &"import", &"parts", &"std", &list, &"--map", &map, &"--as-of", &"2026-01-01"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let dxf = t.path("front.dxf");
    std::fs::write(&dxf, drawing(Some(4))).unwrap();
    let out = t.ew(&args([&"import", &"dxf", &dxf, &"--as", &"outline", &"--library", &lib, &"--part", &"3209510"]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).contains("version 1.0.1"), "{}", stdout(&out));
    let out = t.ew(&args([&"import", &"dxf", &dxf, &"--as", &"outline", &"--library", &lib, &"--part", &"nope"]));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stderr(&out), "error: The library holds no part nope.\n");
}

/// A fake ODA File Converter written by the test: it copies the DWG of its input folder (the test writes DXF text into
/// it) as a DXF into its output folder, as the real converter writes its output.
fn fake_converter(t: &Tmp) -> std::path::PathBuf {
    if cfg!(windows) {
        let path = t.path("converter.cmd");
        std::fs::write(&path, "@echo off\r\ncopy /y \"%~1\\*.dwg\" \"%~2\\drawing.dxf\" >nul\r\n").unwrap();
        path
    } else {
        let path = t.path("converter.sh");
        std::fs::write(&path, "#!/bin/sh\ncp \"$1\"/*.dwg \"$2\"/drawing.dxf\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
}

/// `ew import dwg` runs the configured converter on a copy and imports its DXF, the conversion among the findings;
/// without a converter it says how to give one (FR-IMP-036, EW-IMP-257).
#[test]
fn a_dwg_imports_through_the_users_converter_and_without_one_the_refusal_says_how() {
    let t = Tmp::new("ew-dwg");
    let project = t.tiny("p.ew");
    let dwg = t.path("plant.dwg");
    std::fs::write(&dwg, drawing(Some(4))).unwrap();
    let out = t
        .ew_command(&args([&"import", &"dwg", &dwg, &"--project", &project]))
        .env_remove("EW_DWG_CONVERTER")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(stderr(&out), format!("error: {}\n", ew_api::dxf::DWG_HOW_TO));
    let converter = fake_converter(&t);
    let out = t.ew(&args([&"import", &"dwg", &dwg, &"--project", &project, &"--converter", &converter]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("EW-IMP-257 info: plant.dwg was converted to DXF by the ODA File Converter"), "{text}");
    assert!(text.ends_with("Imported plant.dxf as page 2 with 2 items.\n"), "{text}");
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
