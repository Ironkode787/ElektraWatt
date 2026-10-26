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
    AnyCommand, BomMode, CmdLog, EntropyError, Export, IdSource, Recovery, Session, SessionOptions,
    StoreOptions, UnixTime,
};
use ew_ui::ew_viewmodel::script::{self, NoExtension};
use ew_ui::ew_viewmodel::shell::{IdSourceFactory, Mode, Resources, ShellConfig, ShellModel};
use ew_ui::ew_viewmodel::tools::ToolCx;
use ew_ui::ew_viewmodel::tools::wire::{WireTarget, resolve_target};
use serde_json::json;

use support::{
    GOLDEN_A, GppStart, MarkerFallback, SheetMap, Tmp, all_done, demo_103_bridges, demo_103_pi10,
    demo_103_strips, demo_103_wiring, demo_terminal_part_steps, demo_terminal_steps,
    external_connection_anchors, golden_wiring, gpp_start, gpp_start_demo, junction_style_steps,
    marker_steps, part_steps, path, placement_steps, placement_steps_with, potential_steps,
    rehearsal_connection_id, run, steps, structure_box_steps, typed_tag_steps, wiring_oracle, wiring_steps,
    xref_texts,
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

/// The replay-order guard's clock and entropy pattern, counting in either direction.
struct WiringIds {
    millis: u64,
    n: u64,
    ascending: bool,
}

impl IdSource for WiringIds {
    fn now(&mut self) -> UnixTime {
        if self.ascending {
            self.millis += 1;
        } else {
            self.millis -= 1;
        }
        UnixTime {
            millis: self.millis,
            sub_ms_nanos: 0,
        }
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        if self.ascending {
            self.n += 1;
        } else {
            self.n -= 1;
        }
        let bits = self.n.to_be_bytes();
        for (i, byte) in buf.iter_mut().enumerate() {
            *byte = bits[i % bits.len()];
        }
        Ok(())
    }
}

fn ascending_wiring_ids() -> Box<dyn IdSource + Send> {
    Box::new(WiringIds {
        millis: 0,
        n: 0,
        ascending: true,
    })
}

fn descending_wiring_ids() -> Box<dyn IdSource + Send> {
    Box::new(WiringIds {
        millis: (1 << 48) - 1,
        n: u64::MAX,
        ascending: false,
    })
}

/// A split must hit only its intended stored CP-to-CP wire, including at an interior bend.
fn assert_unique_split_target(shell: &ShellModel, point: (i64, i64), far: &[String], name: &str) {
    let canvas = shell.canvas().unwrap();
    let page = canvas.page();
    let tables = &page.tables;
    let contains = |line: &[(i64, i64)]| {
        line.first() != Some(&point)
            && line.last() != Some(&point)
            && line.windows(2).any(|w| {
                let (a, b) = (w[0], w[1]);
                (a.0 == point.0 && b.0 == point.0 && (a.1.min(b.1)..=a.1.max(b.1)).contains(&point.1))
                    || (a.1 == point.1 && b.1 == point.1 && (a.0.min(b.0)..=a.0.max(b.0)).contains(&point.0))
            })
    };
    let hits: Vec<_> = tables
        .connections
        .iter()
        .filter(|c| c.stored_route && contains(&c.polyline))
        .collect();
    assert_eq!(hits.len(), 1, "{name}: ambiguous split at {point:?} for {far:?}");
    let connection = hits[0];
    let mut labels: Vec<_> = [connection.a, connection.b]
        .into_iter()
        .map(|end| match end {
            TableEnd::Cp { function, idx } => tables
                .cps
                .iter()
                .find(|cp| cp.function == function && cp.idx == idx)
                .unwrap()
                .label
                .clone(),
            TableEnd::Junction { .. } => panic!("{name}: split target must still join two CPs"),
        })
        .collect();
    labels.sort();
    let mut expected = far[..2].to_vec();
    expected.sort();
    assert_eq!(labels, expected, "{name}: intended first wire at {point:?}");
    let cx = ToolCx {
        page,
        points_per_nm: canvas.points_per_nm(),
        pane: canvas.pane(),
        snapper: &page.snapper,
        selection: shell.selection(),
    };
    let target = resolve_target(&cx, page.snapper.page_to_sheet(point), false);
    assert!(
        matches!(target, WireTarget::Segment { connection: id, at } if id.0 == connection.id && at == point),
        "{name}: production wire target must be the unique intended segment at {point:?}: {target:?}"
    );
}

/// The real authoring actions must create the same topology regardless of new connections' ID order.
#[test]
fn gpp_folio_102_wiring_survives_both_new_id_orders() {
    let exe = bin_exe();
    let tmp = Tmp::new();
    let GppStart {
        dir: placed,
        strip_1x1,
        functions,
    } = gpp_start(&exe, &tmp);
    let map = {
        let (s, _) = Session::open_read_only(&placed, "test").unwrap();
        SheetMap::of_grid(&s.page_grid("102").unwrap())
    };
    let (placements, _) = placement_steps(102, map, &strip_1x1, &functions);
    let mut place = steps(&[
        ("file.open", json!({"dir": path(&placed)})),
        ("page.goto", json!({"folio": "102"})),
    ]);
    place.extend(placements);
    place.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "order-place.jsonl", &place, None));
    let mut tags = steps(&[("file.open", json!({"dir": path(&placed)}))]);
    tags.extend(typed_tag_steps(102, &placed));
    tags.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "order-tags.jsonl", &tags, None));

    let (wiring, commits, points) = wiring_steps(102, map);
    assert_eq!(commits, 15);
    let golden_far: Vec<_> = {
        let (s, _) = Session::open_read_only(Path::new(GOLDEN_A), "test").unwrap();
        let tables = s.page_tables("102").unwrap();
        tables
            .junctions
            .iter()
            .map(|j| junction_far_ends(&tables, j))
            .collect()
    };
    assert_eq!(points.len(), 4);
    assert_eq!(golden_far.len(), points.len());
    let styles = junction_style_steps(102, map, &points);
    let sources: [(&str, IdSourceFactory); 2] = [
        ("ascending", ascending_wiring_ids),
        ("descending", descending_wiring_ids),
    ];
    let mut minted_orders = Vec::new();
    for (name, source) in sources {
        let job = tmp.path(&format!("{name}.ew"));
        support::copy_dir(&placed, &job);
        let config = ShellConfig::in_dir(&tmp.path(&format!("config-{name}")), "0.0.0-test").with_resources(
            Resources {
                content_dir: Some(Path::new(support::CONTENT).to_path_buf()),
                ..Resources::default()
            },
        );
        let mut shell = ShellModel::new(Mode::Headless, config);
        shell.set_id_source_for_tests(Some(source));
        let mut actions = steps(&[
            ("file.open", json!({"dir": path(&job)})),
            ("page.goto", json!({"folio": "102"})),
            ("view.zoom_actual", json!({})),
        ]);
        actions.extend(wiring.iter().cloned());
        actions.extend(styles.iter().cloned());
        actions.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
        let parsed = script::parse(&support::script_text(&actions)).unwrap();
        let mut split_checks = 0;
        for (i, step) in parsed.iter().enumerate() {
            if step.action == "canvas.activate" && i > 0 && parsed[i - 1].action == "cursor.set" {
                for (point, far) in points.iter().zip(&golden_far) {
                    let (x, y) = map.mm(*point);
                    if parsed[i - 1].params == json!({"x_mm": x, "y_mm": y}) {
                        assert_unique_split_target(&shell, *point, far, name);
                        split_checks += 1;
                    }
                }
            }
            let result = script::run_step(&mut shell, &mut NoExtension, i + 1, step);
            assert!(result.ok, "{name}: {}", result.line);
        }
        assert_eq!(split_checks, 4, "{name}: every junction split was checked");
        shell.shutdown();
        let (s, _) = Session::open_read_only(&job, "test").unwrap();
        let tables = s.page_tables("102").unwrap();
        // The first connection of each of the first two junctions keeps its ID when split;
        // those two wires are authored in this order by the shared generator.
        let first_wire = |at| tables.junctions.iter().find(|j| j.at == at).unwrap().sequence[0];
        let minted_order = first_wire(points[0]).cmp(&first_wire(points[1]));
        assert_eq!(
            minted_order,
            if name == "ascending" {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            },
            "{name}: the injected source must reverse actual connection IDs"
        );
        minted_orders.push(minted_order);
        eprintln!("folio-102 wiring: {name}, actual connection ID order {minted_order:?}");
        let w23 = rehearsal_connection_id(&tables, "W23");
        let edge = tables
            .connections
            .iter()
            .find(|c| c.id.to_string() == w23)
            .unwrap();
        let (junction, function, idx) = match (edge.a, edge.b) {
            (TableEnd::Junction { id }, TableEnd::Cp { function, idx })
            | (TableEnd::Cp { function, idx }, TableEnd::Junction { id }) => (id, function, idx),
            _ => panic!("{name}: W23 must join a junction to GND#2"),
        };
        assert_eq!(
            tables
                .cps
                .iter()
                .find(|c| c.function == function && c.idx == idx)
                .unwrap()
                .label,
            "-102F2:GND#2"
        );
        let j6 = tables.junctions.iter().find(|j| j.id == junction).unwrap();
        assert_eq!(
            junction_far_ends(&tables, j6),
            ["-102G1:−#2", "-102F2:GND#2", "0VDC"]
        );
        let oracle = wiring_oracle(&tables);
        assert_eq!(oracle.counts, (19, 4), "{name}");
        assert_eq!(oracle, golden_wiring(102), "{name}: complete folio-102 wiring");
    }
    assert_ne!(minted_orders[0], minted_orders[1]);
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
    // E1/E2 of the review of round a28-r5: every 101 group is drawn by the tool, and the one 102 fallback is the
    // W20 to W23 group, in golden order (found by its end labels as `marker_steps` finds its members).
    assert!(
        fallbacks101.is_empty(),
        "no 101 marker group falls back: {fallbacks101:?}"
    );
    assert_eq!(
        fallbacks102.len(),
        1,
        "one 102 marker group falls back: {fallbacks102:?}"
    );
    let w20_to_w23: Vec<String> = ["W20", "W21", "W22", "W23"]
        .iter()
        .map(|wire| rehearsal_connection_id(&tables102, wire))
        .collect();
    assert_eq!(
        fallbacks102[0].connections, w20_to_w23,
        "the 102 fallback's connections are W20, W21, W22 and W23 in order"
    );
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

/// WP-S2.04 §13.6's G-PP rehearsal on the folio-103 demo variant (round a28-r8): folios 101 and 102 as the golden
/// rehearsal draws them, folio 103 without strip segments (the strips by `strip.new`, the 15 terminals by Place,
/// their parts, the wiring to the terminals' own CPs, PI10's three bridges, the other golden bridges and the bridge
/// part quantities), then the save, the PDF export and the oracle of the demo variant.
#[test]
fn gpp_rehearsal_on_the_folio_103_demo_variant() {
    let exe = bin_exe();
    let tmp = Tmp::new();
    let GppStart {
        dir: job,
        strip_1x1,
        functions,
    } = gpp_start_demo(&exe, &tmp);
    assert!(functions.is_empty(), "the demo start's function map is empty");
    // The three folios' sheet maps and grids.
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
    let mut executed: BTreeMap<String, usize> = BTreeMap::new();
    // Script 1: the representations of 101, 102 and 103 (103 without its strip segments).
    let mut script1 = steps(&[("file.open", json!({ "dir": path(&job) }))]);
    let mut runs = 0;
    for (folio, map) in [(101, map101), (102, map102)] {
        let (placements, left) = placement_steps(folio, map, &strip_1x1, &functions);
        runs += left;
        script1.extend(steps(&[("page.goto", json!({"folio": folio.to_string()}))]));
        script1.extend(placements);
    }
    let (placements103, left103) = placement_steps_with(103, map103, &strip_1x1, &functions, &|r| {
        r["rep_type"].as_str() != Some("strip_segment")
    });
    runs += left103;
    script1.extend(steps(&[("page.goto", json!({"folio": "103"}))]));
    script1.extend(placements103);
    script1.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "gpp-demo-1.jsonl", &script1, None));
    // The typed tags of 101 to 103, the parts of 101 and 102 (the demo terminals' parts are B3) and the golden
    // potential definitions.
    let mut tags = Vec::new();
    for folio in [101, 102, 103] {
        tags.extend(typed_tag_steps(folio, &job));
    }
    let parts: Vec<(u32, String, AnyCommand)> = [101, 102]
        .into_iter()
        .flat_map(|folio| {
            part_steps(folio, &job)
                .into_iter()
                .map(move |(k, c)| (folio, k, c))
        })
        .collect();
    let potentials = potential_steps(&[101, 102, 103], &job);
    assert_eq!(potentials.len(), 18, "the golden potential definitions");
    {
        let mut store = StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state-gpp-demo-tags"));
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
            s.run_script(&log, "gpp-demo-tags.ewcmd").unwrap();
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
    // B1: the two strips by the tool, then their golden values member by member.
    let strips = demo_103_strips(&exe, &tmp, &job, &strip_1x1);
    for (name, n) in &strips.executed {
        *executed.entry(name.clone()).or_default() += n;
    }
    // B2: the 15 terminals by Place.
    let terminals = demo_terminal_steps(&job, &strips.x1031, &strips.x1032, map103);
    let mut script2 = steps(&[("file.open", json!({ "dir": path(&job) }))]);
    script2.extend(steps(&[("page.goto", json!({"folio": "103"}))]));
    script2.extend(terminals);
    script2.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "gpp-demo-2.jsonl", &script2, None));
    // B3: their golden parts.
    {
        let mut store = StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state-gpp-demo-terminals"));
        let (mut s, _) = Session::open(&job, store, SessionOptions::default(), Recovery::Keep).unwrap();
        for cmd in demo_terminal_part_steps(&job, &strips.x1031, &strips.x1032) {
            s.execute(&cmd).unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
            *executed.entry(cmd.name.clone()).or_default() += 1;
        }
        s.save().unwrap();
        s.close();
    }
    // Script 3: the wiring of 101 and 102, the demo's 103, the junction styles of 102 and the structure box of 101.
    let (wiring101, commits101, _points101) = wiring_steps(101, map101);
    let (wiring102, commits102, points102) = wiring_steps(102, map102);
    let (wiring103, commits103) = demo_103_wiring(&job, map103);
    let junction = junction_style_steps(102, map102, &points102);
    let structure_box = structure_box_steps(map101, grid101, &external_connection_anchors(101));
    let mut script3 = steps(&[("file.open", json!({ "dir": path(&job) }))]);
    for (folio, wiring) in [(101, &wiring101), (102, &wiring102), (103, &wiring103)] {
        script3.extend(steps(&[
            ("page.goto", json!({"folio": folio.to_string()})),
            ("view.zoom_actual", json!({})),
        ]));
        script3.extend(wiring.iter().cloned());
    }
    script3.extend(steps(&[("page.goto", json!({"folio": "102"}))]));
    script3.extend(junction.iter().cloned());
    script3.extend(steps(&[("page.goto", json!({"folio": "101"}))]));
    script3.extend(structure_box.iter().cloned());
    script3.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "gpp-demo-3.jsonl", &script3, None));
    // B5 (PI10): the demo's -X1031:1 to :4, before and after three strip.bridge steps. Its `connection.draw`
    // records are the tool's, not Session-executed "steps left".
    let pi10 = demo_103_pi10(&exe, &tmp, &job, &strips.x1031);
    // B6: the other golden bridges and the strips' bridge part quantities.
    let bridges = demo_103_bridges(&tmp, &job, &strips);
    for (name, n) in &bridges.executed {
        *executed.entry(name.clone()).or_default() += n;
    }
    // The marker groups of 101 and 102 (the golden rehearsal's), with E1/E2's assertions.
    let (tables101, tables102) = {
        let (s, _) = Session::open_read_only(&job, "test").unwrap();
        (
            s.page_tables("101").expect("page 101"),
            s.page_tables("102").expect("page 102"),
        )
    };
    let (markers101, fallbacks101) = marker_steps(101, &tables101, map101);
    let (markers102, fallbacks102) = marker_steps(102, &tables102, map102);
    assert!(
        fallbacks101.is_empty(),
        "no 101 marker group falls back: {fallbacks101:?}"
    );
    assert_eq!(
        fallbacks102.len(),
        1,
        "one 102 marker group falls back: {fallbacks102:?}"
    );
    let w20_to_w23: Vec<String> = ["W20", "W21", "W22", "W23"]
        .iter()
        .map(|wire| rehearsal_connection_id(&tables102, wire))
        .collect();
    assert_eq!(
        fallbacks102[0].connections, w20_to_w23,
        "the 102 fallback's connections are W20, W21, W22 and W23 in order"
    );
    let mut script4 = steps(&[("file.open", json!({ "dir": path(&job) }))]);
    script4.extend(steps(&[("page.goto", json!({"folio": "101"}))]));
    script4.extend(markers101.iter().cloned());
    script4.extend(steps(&[("page.goto", json!({"folio": "102"}))]));
    script4.extend(markers102.iter().cloned());
    script4.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(&exe, &tmp, "gpp-demo-4.jsonl", &script4, None));
    let mut fallback_commands = Vec::new();
    for (tables, fallbacks) in [(&tables101, &fallbacks101), (&tables102, &fallbacks102)] {
        for fallback in fallbacks {
            fallback_commands.push(fallback_command(tables, fallback));
        }
    }
    if !fallback_commands.is_empty() {
        let mut store = StoreOptions::new("test");
        store.state_dir = Some(tmp.path("state-gpp-demo-fallback"));
        let (mut s, _) = Session::open(&job, store, SessionOptions::default(), Recovery::Keep).unwrap();
        for cmd in &fallback_commands {
            s.execute(cmd).unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
            *executed.entry(cmd.name.clone()).or_default() += 1;
        }
        s.save().unwrap();
        s.close();
    }
    // The PDF export.
    let pdf = tmp.path("gpp-demo.pdf");
    let export = steps(&[
        ("file.open", json!({ "dir": path(&job) })),
        ("file.export_pdf", json!({ "path": path(&pdf) })),
        ("file.save", json!({})),
        ("file.close", json!({})),
    ]);
    all_done(&run(&exe, &tmp, "gpp-demo-5.jsonl", &export, None));
    // The oracle.
    let (final_s, _) = Session::open_read_only(&job, "test").unwrap();
    let (golden_s, _) = Session::open_read_only(Path::new(GOLDEN_A), "test").unwrap();
    let final_tables = |folio: &str| final_s.page_tables(folio).expect("a final page");
    let golden_tables_of = |folio: &str| golden_s.page_tables(folio).expect("a golden page");
    for (folio, reps, connections, junctions) in [(101u32, 17, 21, 2), (102, 12, 19, 4)] {
        let label = folio.to_string();
        let t = final_tables(&label);
        let oracle = wiring_oracle(&t);
        assert_eq!(t.reps.len(), reps, "representations on {folio}");
        assert_eq!(
            oracle.counts,
            (connections, junctions),
            "drawn connections and junctions on {folio}"
        );
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
            reps_of(&golden_tables_of(&label)),
            "the representations of {folio}"
        );
        assert_eq!(oracle, golden_wiring(folio), "the wiring of {folio}");
    }
    // The junctions of 102: style, wiring and bevel orientation, at the same far-end sequence.
    let final_102 = final_tables("102");
    let golden_102 = golden_tables_of("102");
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
    // The marker texts of 101 and 102 and the structure box of 101.
    assert_eq!(marker_texts(&final_tables("101")), ["2.5 BK BU", "2.5 BK BU"]);
    assert_eq!(
        marker_texts(&final_tables("102")),
        ["1.5 DBU DBU", "1.5 DBU DBU WH WH"]
    );
    let boxes = &final_tables("101").structure_boxes;
    assert_eq!(boxes.len(), 1, "one structure box on 101");
    assert_eq!(boxes[0].caption, "External Connection (+J-E-206 / JE-N1000-03)");
    // 103: 35 representations (the 20 interruption points and the 15 terminals), 20 drawn connections, no junction,
    // the 15 terminal devices on the two strips, and the golden wiring pairs (up to orientation).
    let final_103 = final_tables("103");
    assert_eq!(final_103.reps.len(), 35, "representations on 103");
    let oracle103 = wiring_oracle(&final_103);
    assert_eq!(
        oracle103.counts,
        (20, 0),
        "drawn connections and junctions on 103"
    );
    assert_eq!(
        oracle103.pairs,
        golden_wiring(103).pairs,
        "the wiring of the demo's 103"
    );
    let terminal_devices = {
        let q = final_s.query();
        q.devices()
            .iter()
            .filter(|(_, d)| {
                d.aspects.terminal.as_ref().is_some_and(|t| {
                    let strip = t.strip.to_string();
                    strip == strips.x1031 || strip == strips.x1032
                })
            })
            .count()
    };
    assert_eq!(terminal_devices, 15, "terminal devices on the two demo strips");
    // The parts list: Golden A's restricted 14 rows with the demo variant's four quantities.
    let bom = |s: &Session| {
        s.export_csv(Export::Bom(BomMode::Component))
            .expect("the 901 parts list")
    };
    let final_rows = parts_rows(&parse_csv(&bom(&final_s)));
    let golden_rows = parts_rows(&parse_csv(&bom(&golden_s)));
    let kept = |pos: &str| pos.parse::<u32>().is_ok_and(|p| matches!(p, 3..=14 | 26 | 27));
    let tbd: Vec<&str> = golden_rows
        .iter()
        .filter(|(pos, part, _)| kept(pos) && part == "TBD")
        .map(|(_, _, qty)| qty.as_str())
        .collect();
    assert_eq!(tbd, ["4", "6"], "Golden A's end bracket and plug-in bridge rows");
    let mut expected: Vec<(String, String)> = golden_rows
        .iter()
        .filter(|(pos, _, _)| kept(pos))
        .map(|(_, part, qty)| {
            let qty = match (part.as_str(), qty.as_str()) {
                ("PT 2.5-TWIN", _) => "11".to_owned(),
                ("PT 2.5-TWIN-PE", _) => "4".to_owned(),
                ("TBD", "6") => "3".to_owned(),
                _ => qty.clone(),
            };
            (part.clone(), qty)
        })
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
        "the 901 parts list of the demo variant: {actual:?}"
    );
    assert_eq!(actual, expected, "the demo variant's 901 parts list");
    // The PDF export exists and is a PDF.
    let bytes = std::fs::read(&pdf).expect("the exported PDF");
    assert!(bytes.starts_with(b"%PDF-"), "the export starts with %PDF-");
    // The lane's report.
    eprintln!(
        "gpp-demo: scripts {} + {} + {} + {} steps; wires {} + {} + {} commits; junctions of 102 {} steps; structure box {} steps",
        script1.len(),
        script2.len(),
        script3.len(),
        script4.len(),
        commits101,
        commits102,
        commits103,
        junction.len(),
        structure_box.len(),
    );
    eprintln!("gpp-demo: command.run steps left (Session-executed records): {executed:?}");
    eprintln!("gpp-demo: placement command.run steps: {runs}");
    eprintln!("gpp-demo: PI10 before: {:?}", pi10.before);
    eprintln!("gpp-demo: PI10 after: {:?}", pi10.after);
    eprintln!(
        "gpp-demo: PI10 group {} ({} page-less draws, revision {} -> {})",
        pi10.group, pi10.draws, pi10.rev_before, pi10.rev_after
    );
    eprintln!(
        "gpp-demo: bridge groups {:?}; bridges drawn {:?}",
        bridges.groups, bridges.drawn
    );
    eprintln!(
        "gpp-demo: 103 oracle: reps {} connections {} junctions {} terminals {}",
        final_103.reps.len(),
        oracle103.counts.0,
        oracle103.counts.1,
        terminal_devices
    );
    eprintln!("gpp-demo: 901 parts rows {actual:?}");
    final_s.close();
    golden_s.close();
}
