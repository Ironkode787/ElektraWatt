// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP rehearsal of WP-S2.04 §13.6: folios 101 to 103 drawn by generated UI scripts on a new project made by
//! the company-standard wizard, its 901 parts list and its PDF export checked against Golden A. Every step is a
//! script through the built binary except the records no tool draws yet (typed tags, parts and a marker group the
//! Marker tool's crossing line does not meet), which are `command.run` steps the Session executes; the test prints
//! them, the "command.run steps left" of the lane.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ew_ui::ew_viewmodel::ew_api::output::{PageTables, RowValue, TableEnd, TableJunction};
use ew_ui::ew_viewmodel::ew_api::{
    AnyCommand, BomMode, CmdLog, Export, Recovery, Session, SessionOptions, StoreOptions,
};
use serde_json::json;

use support::{
    GOLDEN_A, GppStart, MarkerFallback, SheetMap, Tmp, all_done, external_connection_anchors, golden_wiring,
    gpp_start, junction_style_steps, marker_steps, part_steps, path, placement_steps, potential_steps, run,
    steps, structure_box_steps, typed_tag_steps, wiring_oracle, wiring_steps, xref_texts,
};

/// The path of the `elektrawatt` binary under test. nextest sets `NEXTEST_BIN_EXE_elektrawatt` at run time (the
/// binaries of an `--archive-file` run are unpacked elsewhere than the compile-time path, FT-353); a plain
/// `cargo test` falls back to the compile-time path.
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> std::path::PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

/// The far ends of a junction's sequence, by their printed labels (the oracle's naming of a junction).
fn junction_far_ends(t: &PageTables, j: &TableJunction) -> Vec<String> {
    let label = |e: TableEnd| match e {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == function && c.idx == idx)
            .map(|c| c.label.clone()),
        TableEnd::Junction { .. } => None,
    };
    j.sequence
        .iter()
        .filter_map(|id| t.connections.iter().find(|c| c.id == *id))
        .map(|c| match (c.a, c.b) {
            (TableEnd::Junction { id }, other) if id == j.id => label(other).unwrap_or_default(),
            (other, _) => label(other).unwrap_or_default(),
        })
        .collect()
}

/// The `marker.place` of a fallback group: its page, its connections, the midpoint of their polylines' bounding
/// box and its cross-section.
fn fallback_command(tables: &PageTables, fallback: &MarkerFallback) -> AnyCommand {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for id in &fallback.connections {
        let connection = tables
            .connections
            .iter()
            .find(|c| c.id.to_string() == *id)
            .expect("the fallback's connection");
        for p in &connection.polyline {
            xs.push(p.0);
            ys.push(p.1);
        }
    }
    let at = (
        (xs.iter().min().unwrap() + xs.iter().max().unwrap()) / 2,
        (ys.iter().min().unwrap() + ys.iter().max().unwrap()) / 2,
    );
    AnyCommand {
        name: "marker.place".into(),
        params: json!({
            "page": tables.page.to_string(),
            "connections": fallback.connections,
            "at": [at.0, at.1],
            "csa": fallback.csa,
        }),
    }
}

/// The rows of a CSV text (the golden dialect of `ew-reports`: quotes doubled, quoted fields may hold commas and
/// line breaks; LF line ends).
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
        } else {
            match c {
                '"' if field.is_empty() => quoted = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                '\r' => {}
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// The data rows of a parts-list CSV as `(pos, part number, quantity)`.
fn parts_rows(rows: &[Vec<String>]) -> Vec<(String, String, String)> {
    let header = &rows[0];
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("no {name} column in {header:?}"))
    };
    let (pos, qty, part) = (col("Pos"), col("Qty"), col("Part number"));
    rows[1..]
        .iter()
        .filter(|r| r[0] == "data")
        .map(|r| (r[pos].clone(), r[part].clone(), r[qty].clone()))
        .collect()
}

/// The wire-marker texts of a page, sorted.
fn marker_texts(t: &PageTables) -> Vec<String> {
    let mut out: Vec<String> = t
        .items
        .iter()
        .filter(|i| i.kind == "wire_marker")
        .filter_map(|i| i.text.clone())
        .collect();
    out.sort();
    out
}

/// WP-S2.04 §13.6's G-PP rehearsal: the start of `gpp_start`, the placements of 101 to 103 in one script, the typed
/// tags and the parts through the Session, the wiring, the junction styles of 102 and the structure box of 101, the
/// four marker groups, the PDF export and the 901 parts list. The oracle of §13.6 checks the counts, the
/// representations, the wiring, the junctions, the cross-reference texts, the page items, the box and the parts
/// list; the test prints the `command.run` steps left and the marker fallbacks.
#[test]
fn gpp_rehearsal_draws_folios_101_to_103_saves_exports_and_lists_the_parts() {
    let exe = bin_exe();
    let tmp = Tmp::new();
    let GppStart {
        dir: job,
        strip_1x1,
        functions,
    } = gpp_start(&exe, &tmp);
    // The three folios' sheet maps and grids, read from the start project.
    let (map101, grid101, map102, map103) = {
        let (s, _) = Session::open_read_only(&job, "test").unwrap();
        let g1 = s.page_grid("101").expect("page 101");
        let g2 = s.page_grid("102").expect("page 102");
        let g3 = s.page_grid("103").expect("page 103");
        (
            SheetMap::of_grid(&g1),
            (g1.num_nm, g1.den),
            SheetMap::of_grid(&g2),
            SheetMap::of_grid(&g3),
        )
    };
    // Script 1: the representations of 101, 102 and 103 (one run).
    let mut script1 = steps(&[("file.open", json!({ "dir": path(&job) }))]);
    let mut runs = 0;
    for (folio, map) in [(101, map101), (102, map102), (103, map103)] {
        let (placements, left) = placement_steps(folio, map, &strip_1x1, &functions);
        runs += left;
        script1.extend(steps(&[("page.goto", json!({"folio": folio.to_string()}))]));
        script1.extend(placements);
    }
    script1.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "gpp-1.jsonl", &script1, None));
    // The tagged devices and the parts the tools do not draw, read from the placed project before the Session opens
    // it for writing.
    let mut tags = Vec::new();
    for folio in [101, 102, 103] {
        tags.extend(typed_tag_steps(folio, &job));
    }
    let parts: Vec<(u32, String, AnyCommand)> = [101, 102, 103]
        .into_iter()
        .flat_map(|folio| {
            part_steps(folio, &job)
                .into_iter()
                .map(move |(k, c)| (folio, k, c))
        })
        .collect();
    // §13.6's golden potential definitions of the three folios: `command.run` steps like the tags and parts.
    let potentials = potential_steps(&[101, 102, 103], &job);
    assert!(!potentials.is_empty(), "the golden potentials of 101 to 103");
    // The Session's records: the typed tags as a script (they are `command.run` steps), then every parts command,
    // then the potential definitions.
    let mut executed: BTreeMap<String, usize> = BTreeMap::new();
    {
        let mut store = StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state-gpp-tags"));
        let (mut s, _) = Session::open(&job, store, SessionOptions::default(), Recovery::Keep).unwrap();
        if !tags.is_empty() {
            let mut text = String::from("{\"$schema\":\"ew.cmdlog/1\"}\n");
            for (action, params) in &tags {
                assert_eq!(action, "command.run", "the typed tags are command.run steps");
                text.push_str(&json!({"cmd": params["cmd"], "params": params["params"]}).to_string());
                text.push('\n');
                *executed
                    .entry(params["cmd"].as_str().unwrap().to_owned())
                    .or_default() += 1;
            }
            let log = CmdLog::parse(&text).unwrap();
            s.run_script(&log, "gpp-tags.ewcmd").unwrap();
        }
        for (_, _, cmd) in &parts {
            s.execute(cmd).unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
            *executed.entry(cmd.name.clone()).or_default() += 1;
        }
        for (label, cmd) in &potentials {
            s.execute(cmd)
                .unwrap_or_else(|e| panic!("potential {label}: {e:?}"));
            *executed.entry(cmd.name.clone()).or_default() += 1;
        }
        s.save().unwrap();
        s.close();
    }
    // Script 2: the wiring of the three folios at actual size, the golden junction styles and wiring of 102, and the
    // structure box of 101.
    let (wiring101, commits101, _points101) = wiring_steps(101, map101);
    let (wiring102, commits102, points102) = wiring_steps(102, map102);
    let (wiring103, commits103, _points103) = wiring_steps(103, map103);
    let junction = junction_style_steps(102, map102, &points102);
    let structure_box = structure_box_steps(map101, grid101, &external_connection_anchors(101));
    let mut script2 = steps(&[("file.open", json!({ "dir": path(&job) }))]);
    for (folio, wiring) in [(101, &wiring101), (102, &wiring102), (103, &wiring103)] {
        script2.extend(steps(&[
            ("page.goto", json!({"folio": folio.to_string()})),
            ("view.zoom_actual", json!({})),
        ]));
        script2.extend(wiring.iter().cloned());
    }
    script2.extend(steps(&[("page.goto", json!({"folio": "102"}))]));
    script2.extend(junction.iter().cloned());
    script2.extend(steps(&[("page.goto", json!({"folio": "101"}))]));
    script2.extend(structure_box.iter().cloned());
    script2.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "gpp-2.jsonl", &script2, None));
    // Script 3: the marker groups of 101 and 102, from the saved project's page tables (the drawn routes).
    let (tables101, tables102) = {
        let (s, _) = Session::open_read_only(&job, "test").unwrap();
        (
            s.page_tables("101").expect("page 101"),
            s.page_tables("102").expect("page 102"),
        )
    };
    let (markers101, fallbacks101) = marker_steps(101, &tables101, map101);
    let (markers102, fallbacks102) = marker_steps(102, &tables102, map102);
    let mut script3 = steps(&[("file.open", json!({ "dir": path(&job) }))]);
    script3.extend(steps(&[("page.goto", json!({"folio": "101"}))]));
    script3.extend(markers101.iter().cloned());
    script3.extend(steps(&[("page.goto", json!({"folio": "102"}))]));
    script3.extend(markers102.iter().cloned());
    script3.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "gpp-3.jsonl", &script3, None));
    // The groups no crossing line meets, placed with `marker.place` through the Session.
    let mut fallback_commands = Vec::new();
    for (tables, fallbacks) in [(&tables101, &fallbacks101), (&tables102, &fallbacks102)] {
        for fallback in fallbacks {
            fallback_commands.push(fallback_command(tables, fallback));
        }
    }
    if !fallback_commands.is_empty() {
        let mut store = StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state-gpp-fallback"));
        let (mut s, _) = Session::open(&job, store, SessionOptions::default(), Recovery::Keep).unwrap();
        for cmd in &fallback_commands {
            s.execute(cmd).unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
            *executed.entry(cmd.name.clone()).or_default() += 1;
        }
        s.save().unwrap();
        s.close();
    }
    // The PDF export of the project.
    let pdf = tmp.path("gpp.pdf");
    let export = steps(&[
        ("file.open", json!({ "dir": path(&job) })),
        ("file.export_pdf", json!({ "path": path(&pdf) })),
        ("file.save", json!({})),
        ("file.close", json!({})),
    ]);
    all_done(&run(&exe, &tmp, "gpp-4.jsonl", &export, None));
    // The lane's report: the `command.run` steps left and the marker fallbacks (before the oracle, so that it shows
    // even when an oracle item differs).
    eprintln!(
        "gpp: scripts {} + {} + {} steps; wires {} + {} + {} commits; junctions of 102 {} steps; structure box {} steps",
        script1.len(),
        script2.len(),
        script3.len(),
        commits101,
        commits102,
        commits103,
        junction.len(),
        structure_box.len(),
    );
    eprintln!("gpp: command.run steps left (Session-executed records): {executed:?}");
    eprintln!("gpp: placement command.run steps: {runs}");
    eprintln!("gpp: marker fallbacks 101: {fallbacks101:?}");
    eprintln!("gpp: marker fallbacks 102: {fallbacks102:?}");
    // The oracle of §13.6 on the final project, Golden A for the expectations.
    let (final_s, _) = Session::open_read_only(&job, "test").unwrap();
    let (golden_s, _) = Session::open_read_only(Path::new(GOLDEN_A), "test").unwrap();
    let final_tables = |folio: &str| final_s.page_tables(folio).expect("a final page");
    let golden_tables = |folio: &str| golden_s.page_tables(folio).expect("a golden page");
    // Counts: representations, drawn connections, junctions.
    for (folio, reps, connections, junctions) in [(101u32, 17, 21, 2), (102, 12, 19, 4), (103, 23, 20, 0)] {
        let label = folio.to_string();
        let t = final_tables(&label);
        assert_eq!(t.reps.len(), reps, "representations on {folio}");
        let oracle = wiring_oracle(&t);
        assert_eq!(
            oracle.counts,
            (connections, junctions),
            "drawn connections and junctions on {folio}"
        );
        // The representations as (symbol key, point, orientation) equal Golden A's.
        let reps_of = |t: &PageTables| {
            let mut v: Vec<_> = t
                .reps
                .iter()
                .map(|r| (r.symbol_key.clone(), r.pos, r.orient))
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            reps_of(&t),
            reps_of(&golden_tables(&label)),
            "the representations of {folio}"
        );
        // The drawn connections' end labels and the junctions' sequences equal Golden A's.
        assert_eq!(oracle, golden_wiring(folio), "the wiring of {folio}");
    }
    // The junctions of 102: style, wiring and bevel orientation, at the same far-end sequence.
    let final_102 = final_tables("102");
    let golden_102 = golden_tables("102");
    assert_eq!(final_102.junctions.len(), 4);
    for j in &final_102.junctions {
        let far = junction_far_ends(&final_102, j);
        let golden_j = golden_102
            .junctions
            .iter()
            .find(|g| junction_far_ends(&golden_102, g) == far)
            .unwrap_or_else(|| panic!("no golden junction at {far:?}"));
        assert_eq!(j.style, golden_j.style, "the style of {far:?}");
        assert_eq!(j.wiring, golden_j.wiring, "the wiring of {far:?}");
        assert_eq!(
            j.bevel_orient, golden_j.bevel_orient,
            "the bevel orientation of {far:?}"
        );
    }
    // The cross-reference texts whose target lies on 101, 102 or 103.
    let folios = ["101", "102", "103"];
    let final_xrefs = xref_texts(&final_s, &folios);
    assert_eq!(
        final_xrefs.len(),
        20,
        "the 20 cross-reference texts of Q8: {final_xrefs:?}"
    );
    assert_eq!(
        final_xrefs,
        xref_texts(&golden_s, &folios),
        "the cross-reference texts of 101 to 103"
    );
    // The page items: the two marker groups of each page and the structure box of 101.
    assert_eq!(marker_texts(&final_tables("101")), ["2.5 BK BU", "2.5 BK BU"]);
    assert_eq!(
        marker_texts(&final_tables("102")),
        ["1.5 DBU DBU", "1.5 DBU DBU WH WH"]
    );
    let boxes = &final_tables("101").structure_boxes;
    assert_eq!(boxes.len(), 1, "one structure box on 101");
    assert_eq!(boxes[0].caption, "External Connection (+J-E-206 / JE-N1000-03)");
    // The three external connection points' derived location: their cross-reference (a public derived row of the
    // property sheet) prints +J-E-206.
    let external_functions: BTreeSet<String> = {
        let q = final_s.query();
        let mut out = BTreeSet::new();
        for (_, f) in q.functions().iter() {
            let v = serde_json::to_value(&**f).unwrap();
            if v["kind"]["ip"]["external"].is_string() {
                out.insert(v["id"].as_str().unwrap().to_owned());
            }
        }
        out
    };
    assert_eq!(external_functions.len(), 3);
    let external_reps: Vec<_> = final_tables("101")
        .reps
        .iter()
        .filter(|r| {
            r.functions
                .first()
                .is_some_and(|f| external_functions.contains(&f.to_string()))
        })
        .map(|r| r.id)
        .collect();
    assert_eq!(external_reps.len(), 3, "the three external connection points");
    for rep in external_reps {
        let sheet = final_s.property_sheet(&[rep]);
        let row = sheet
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.key == "representation/derived.xref")
            .expect("the representation's derived cross-reference");
        let RowValue::One { text, .. } = &row.value else {
            panic!(
                "the derived cross-reference differs over one target: {:?}",
                row.value
            );
        };
        assert_eq!(
            text, "+J-E-206 / JE-N1000-03",
            "the external point's derived location"
        );
    }
    // The PDF export exists and is a PDF.
    let bytes = std::fs::read(&pdf).expect("the exported PDF");
    assert!(bytes.starts_with(b"%PDF-"), "the export starts with %PDF-");
    // The parts list 901: golden §14.1 rows 3 to 14, 26 and 27, by (part number, quantity).
    let bom = |s: &Session| {
        s.export_csv(Export::Bom(BomMode::Component))
            .expect("the 901 parts list")
    };
    let final_rows = parts_rows(&parse_csv(&bom(&final_s)));
    let golden_rows = parts_rows(&parse_csv(&bom(&golden_s)));
    let kept = |pos: &str| pos.parse::<u32>().is_ok_and(|p| matches!(p, 3..=14 | 26 | 27));
    let mut expected: Vec<(String, String)> = golden_rows
        .iter()
        .filter(|(pos, _, _)| kept(pos))
        .map(|(_, part, qty)| (part.clone(), qty.clone()))
        .collect();
    let mut actual: Vec<(String, String)> = final_rows
        .iter()
        .map(|(_, part, qty)| (part.clone(), qty.clone()))
        .collect();
    assert_eq!(expected.len(), 14, "the 14 golden §14.1 rows: {expected:?}");
    expected.sort();
    actual.sort();
    assert_eq!(
        actual.len(),
        14,
        "the 901 parts list of the rehearsal: {actual:?}"
    );
    assert_eq!(actual, expected, "the 901 parts list");
    final_s.close();
    golden_s.close();
}
