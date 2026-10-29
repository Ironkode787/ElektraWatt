// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bodies of `canvas.rs`'s folio-script tests (WP-S2.04 §13.6), shared with the deep nightly's
//! `headless::nightly_every_script_transcript_is_byte_identical_twice`: `canvas.rs` runs each with `twice`
//! false (one run of the script per gate, then the oracle and the `--record` replay), the nightly with `twice`
//! true (a second run on an equal start and the byte-identical comparison of the two transcripts).

use std::collections::BTreeMap;
use std::path::Path;

use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};

use super::bin_exe;
use super::support::{
    SheetMap, Tmp, all_done, copy_dir, golden_reps, path, placement_steps, records_of, reps_of, run, start,
    steps,
};

pub fn folio_101_is_placed_by_a_generated_script_twice_identically_and_replays_case(twice: bool) {
    let tmp = Tmp::new();
    let job = start(&bin_exe(), &tmp);
    // The page's sheet map and the strip's ID, read from the start project.
    let (session, _) = Session::open_read_only(&job, "test").unwrap();
    let grid = session.page_grid("101").expect("page 101");
    let map = SheetMap {
        scale_ppb: i128::from(grid.sheet_scale_ppb),
        offset: (
            i128::from(grid.sheet_offset_nm.0),
            i128::from(grid.sheet_offset_nm.1),
        ),
    };
    let q = session.query();
    let strip = q
        .devices()
        .iter()
        .find(|(_, d)| d.aspects.strip.is_some())
        .map(|(id, _)| id.to_string())
        .expect("the start project's strip");
    drop(q);
    drop(session);
    let (placements, runs) = placement_steps(101, map, &strip, &BTreeMap::new());
    // Two copies of the start: two runs of the same script give byte-identical transcripts.
    let mut transcripts = Vec::new();
    let mut records = Vec::new();
    for n in super::support::runs(twice) {
        let dir = tmp.path(n).join("folio.ew");
        copy_dir(&job, &dir);
        let mut script = steps(&[
            ("file.open", json!({ "dir": path(&dir) })),
            ("page.goto", json!({"folio": "101"})),
        ]);
        script.extend(placements.iter().cloned());
        script.extend(steps(&[
            ("ui.describe", json!({})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ]));
        let record = tmp.path(&format!("{n}.ewcmd"));
        let out = run(&bin_exe(), &tmp, &format!("{n}.jsonl"), &script, Some(&record));
        all_done(&out);
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(
            !text.contains(&*tmp.dir().to_string_lossy()),
            "a path in the transcript"
        );
        transcripts.push(text);
        records.push(record);
    }
    if twice {
        if let Some((a, b)) = transcripts[0]
            .lines()
            .zip(transcripts[1].lines())
            .find(|(a, b)| a != b)
        {
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "two runs, two transcripts: first difference at {at}: {} | {}",
                &a[at.saturating_sub(120)..(at + 80).min(a.len())],
                &b[at.saturating_sub(120)..(at + 80).min(b.len())]
            );
        }
        assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
    }
    // FT-422: no UUID in any transcript member or describe text.
    super::support::assert_no_uuid(&transcripts[0]);
    // The oracle of §13.6: 17 representations, each the golden symbol at the golden point and orientation.
    let golden: Vec<(String, (i64, i64), u8)> = {
        let mut v: Vec<_> = golden_reps(101)
            .into_iter()
            .map(|r| (r.symbol, r.pos, r.orient))
            .collect();
        v.sort();
        v
    };
    assert_eq!(golden.len(), 17);
    let placed = reps_of(&tmp.path("a").join("folio.ew"));
    assert_eq!(placed.len(), 17, "representations on 101");
    assert_eq!(placed, golden);
    // No step left to a later part: the six interruption points are the IP tool's (part 3).
    assert_eq!(runs, 0);
    // IB7: the IP tool placed Golden A's six interruption points of 101 with their names, directions, symbols
    // and points, the three external connection points with their external references.
    let ip_rows = |t: &ew_ui::ew_viewmodel::ew_api::output::PageTables| {
        let mut v: Vec<_> = t
            .reps
            .iter()
            .filter_map(|r| {
                r.ip.as_ref()
                    .map(|i| (i.name.clone(), i.direction.clone(), r.symbol_key.clone(), r.pos))
            })
            .collect();
        v.sort();
        v
    };
    let (golden_session, _) = Session::open_read_only(Path::new(super::support::GOLDEN_A), "test").unwrap();
    let (scripted_ips, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
    assert_eq!(
        ip_rows(&scripted_ips.page_tables("101").unwrap()),
        ip_rows(&golden_session.page_tables("101").unwrap())
    );
    let externals = scripted_ips
        .query()
        .functions()
        .iter()
        .filter(|(_, f)| serde_json::to_value(&***f).unwrap()["kind"]["ip"]["external"].is_string())
        .count();
    assert_eq!(
        externals, 3,
        "the external connection points name their references"
    );
    // `--record` replayed on a third copy of the start (the session's command-log replay, `ew apply`'s path) gives
    // the same representations.
    let replay = tmp.path("c").join("folio.ew");
    copy_dir(&job, &replay);
    let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| **n == "device.place").count(),
        11,
        "{names:?}"
    );
    assert_eq!(names.iter().filter(|n| **n == "ip.place").count(), 6);
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state"));
    let (mut s, _) = Session::open(
        &replay,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    s.run_script(&log, "a.ewcmd").unwrap();
    let mut replayed: Vec<_> = s
        .page_tables("101")
        .unwrap()
        .reps
        .iter()
        .map(|r| (r.symbol_key.clone(), r.pos, r.orient))
        .collect();
    replayed.sort();
    assert_eq!(
        replayed, placed,
        "the replay's records equal the scripted session's"
    );
    // The devices (tags, parts, terminal aspects) and functions too, IDs aside (fix round, item 7).
    let (scripted, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
    let (devices, functions) = records_of(&scripted);
    assert_eq!(records_of(&s), (devices.clone(), functions.clone()));
    assert!(
        devices.iter().filter(|d| d.contains("\"terminal\":{")).count() >= 3,
        "the terminals"
    );
    assert!(functions.len() >= 17);
    s.close();
}

/// WP-S2.04 part 2's share of §13.6 (WR9): folio 101 placed, its golden tags typed (`fields.set` through `command.run`
/// on `tag:` references), its 21 connections and 2 junctions drawn with the Wire tool from the keyboard; two runs give
/// byte-identical transcripts; the oracle: 17 representations, 21 drawn connections and 2 junctions, every
/// connection's end labels and every junction's sequence (by far-end labels) as on Golden A; the `--record` log
/// replayed on the start gives the same wiring and the same devices and functions, IDs aside.
pub fn folio_101_is_wired_by_a_generated_script_twice_identically_and_replays_case(twice: bool) {
    use super::support::{golden_wiring, typed_tag_steps, wiring_oracle, wiring_steps};
    let tmp = Tmp::new();
    let job = start(&bin_exe(), &tmp);
    let (session, _) = Session::open_read_only(&job, "test").unwrap();
    let grid = session.page_grid("101").expect("page 101");
    let map = SheetMap {
        scale_ppb: i128::from(grid.sheet_scale_ppb),
        offset: (
            i128::from(grid.sheet_offset_nm.0),
            i128::from(grid.sheet_offset_nm.1),
        ),
    };
    let q = session.query();
    let strip = q
        .devices()
        .iter()
        .find(|(_, d)| d.aspects.strip.is_some())
        .map(|(id, _)| id.to_string())
        .expect("the start project's strip");
    drop(q);
    drop(session);
    let (placements, runs) = placement_steps(101, map, &strip, &BTreeMap::new());
    // Phase A: the placements alone, to learn the tags they print.
    let placed = tmp.path("p").join("folio.ew");
    copy_dir(&job, &placed);
    let mut script = steps(&[
        ("file.open", json!({ "dir": path(&placed) })),
        ("page.goto", json!({"folio": "101"})),
    ]);
    script.extend(placements.iter().cloned());
    script.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(
        &bin_exe(),
        &tmp,
        "p.jsonl",
        &script,
        Some(&tmp.path("p.ewcmd")),
    ));
    let tags = typed_tag_steps(101, &placed);
    let (wiring, commits, _points) = wiring_steps(101, map);
    assert_eq!(commits, 19, "15 plain wires and 2 per junction");
    // The reference to the first device the tag steps type (a tag the placements print, no ID).
    let first_tag = tags[0].1["params"]["targets"][0]["ref"].clone();
    assert!(
        first_tag.as_str().is_some_and(|t| t.starts_with("tag:")),
        "{first_tag}"
    );
    // The full script, twice.
    let mut transcripts = Vec::new();
    let mut records = Vec::new();
    for n in super::support::runs(twice) {
        let dir = tmp.path(n).join("folio.ew");
        copy_dir(&job, &dir);
        let mut script = steps(&[
            ("file.open", json!({ "dir": path(&dir) })),
            ("page.goto", json!({"folio": "101"})),
        ]);
        script.extend(placements.iter().cloned());
        // FT-422: a device the script placed (its ID minted, so different in each run) selected and cleared: the
        // transcript names it by its alias, the same in both runs.
        script.extend(steps(&[
            ("select.set", json!({"refs": [first_tag.clone()]})),
            ("select.set", json!({"ids": []})),
        ]));
        script.extend(tags.iter().cloned());
        script.extend(wiring.iter().cloned());
        script.extend(steps(&[
            ("ui.describe", json!({})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ]));
        let record = tmp.path(&format!("{n}.ewcmd"));
        let out = run(&bin_exe(), &tmp, &format!("{n}.jsonl"), &script, Some(&record));
        all_done(&out);
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(
            !text.contains(&*tmp.dir().to_string_lossy()),
            "a path in the transcript"
        );
        transcripts.push(text);
        records.push(record);
    }
    if twice {
        if let Some((a, b)) = transcripts[0]
            .lines()
            .zip(transcripts[1].lines())
            .find(|(a, b)| a != b)
        {
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "two runs, two transcripts: first difference at {at}: {} | {}",
                &a[at.saturating_sub(120)..(at + 80).min(a.len())],
                &b[at.saturating_sub(120)..(at + 80).min(b.len())]
            );
        }
        assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
    }
    // FT-422: no UUID in any transcript member or describe text, and the minted device's alias in both runs.
    super::support::assert_no_uuid(&transcripts[0]);
    let selected: Vec<Value> = transcripts[0]
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|s| s["action"] == "select.set")
        .map(|s| s["state"]["canvas"]["selection"].clone())
        .collect();
    assert_eq!(selected.len(), 2, "{selected:?}");
    let alias = selected[0][0].as_str().unwrap_or_default();
    assert!(
        ew_ui::ew_viewmodel::alias::is_alias(alias),
        "the selection prints an alias: {selected:?}"
    );
    assert_eq!(selected[1], json!([]));
    // The oracle of §13.6.
    let golden = golden_wiring(101);
    assert_eq!(golden.counts, (21, 2));
    let (scripted, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
    let tables = scripted.page_tables("101").unwrap();
    assert_eq!(tables.reps.len(), 17, "representations on 101");
    let wired = wiring_oracle(&tables);
    assert_eq!(wired.counts, (21, 2), "drawn connections and junctions on 101");
    assert_eq!(wired.sequences, golden.sequences, "every junction sequence");
    assert_eq!(wired.pairs, golden.pairs, "every connection end labels");
    assert_eq!(wired.sequences, golden.sequences, "every junction's sequence");
    // Every drawn connection carries the route the Wire tool showed.
    assert!(tables.connections.iter().all(|c| c.stored_route));
    // The interruption points are the IP tool's (part 3); the typed tags stay `command.run` steps.
    assert_eq!(runs, 0);
    assert!(!tags.is_empty());
    // `--record` replayed on a third copy of the start.
    let replay = tmp.path("c").join("folio.ew");
    copy_dir(&job, &replay);
    let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| **n == "connection.draw").count(),
        19,
        "{names:?}"
    );
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state"));
    let (mut s, _) = Session::open(
        &replay,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    // The log names the functions and connections the session minted (a new project's IDs are random, FT-416):
    // each `connection.draw` end is mapped to the replay's record at the same place before it runs (a CP by its
    // point and index, a wire by its connection point end), every other line replays as recorded.
    let a_tables = scripted.page_tables("101").unwrap();
    for cmd in log.commands() {
        let mut cmd = cmd.clone();
        if cmd.name == "connection.draw" {
            let now = s.page_tables("101").unwrap();
            let map_cp = |f: &str, idx: u64| -> String {
                let at = a_tables
                    .cps
                    .iter()
                    .find(|c| c.function.to_string() == f && u64::from(c.idx) == idx)
                    .expect("a CP of the run")
                    .at;
                now.cps
                    .iter()
                    .find(|c| c.at == at && u64::from(c.idx) == idx)
                    .expect("the CP of the replay")
                    .function
                    .to_string()
            };
            for end in ["a", "b"] {
                let e = &mut cmd.params[end];
                if let Some(cp) = e.get_mut("cp") {
                    let f = map_cp(cp["function"].as_str().unwrap(), cp["idx"].as_u64().unwrap());
                    cp["function"] = Value::from(f);
                } else if let Some(on) = e.get_mut("on_connection") {
                    let id = on["connection"].as_str().unwrap().to_owned();
                    let w = a_tables
                        .connections
                        .iter()
                        .find(|c| c.id.to_string() == id)
                        .expect("the wire");
                    let ew_ui::ew_viewmodel::ew_api::output::TableEnd::Cp { function, idx } = w.a else {
                        panic!("the split wire starts at a CP");
                    };
                    let f = map_cp(&function.to_string(), u64::from(idx));
                    let mapped = now
                        .connections
                        .iter()
                        .find(|c| {
                            matches!(c.a, ew_ui::ew_viewmodel::ew_api::output::TableEnd::Cp { function, idx: i }
                                if function.to_string() == f && i == idx)
                        })
                        .expect("the wire of the replay")
                        .id;
                    on["connection"] = Value::from(mapped.to_string());
                }
            }
        }
        // One line through `run_script`, the replay's own path (references resolved there).
        let line = json!({"cmd": cmd.name, "params": cmd.params});
        let one = CmdLog::parse(&format!("{{\"$schema\":\"ew.cmdlog/1\"}}\n{line}\n")).unwrap();
        s.run_script(&one, "a.ewcmd")
            .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
    }
    let replayed = s.page_tables("101").unwrap();
    assert_eq!(wiring_oracle(&replayed), wired, "the replay's wiring");
    let routes = |t: &ew_ui::ew_viewmodel::ew_api::output::PageTables| {
        let mut r: Vec<Vec<(i64, i64)>> = t.connections.iter().map(|c| c.polyline.clone()).collect();
        r.sort();
        r
    };
    assert_eq!(
        routes(&replayed),
        routes(&tables),
        "the replay's routes: the router never runs on replay"
    );
    assert_eq!(records_of(&s), records_of(&scripted));
    s.close();
}

/// WP-S2.04 part 3's share of §13.6 (IB7): folio 102 placed by a generated script: its nine interruption points
/// with the IP tool, its two black boxes (-102G1, -102F2) with the black box tool from their parts and their CPs
/// dragged along the outline to the golden points (Select's CP drag, `box.reshape`), its contact with the Place
/// tool. Two runs give byte-identical transcripts without a UUID; the oracle: 12 representations, each the golden
/// symbol at the golden point and orientation, the interruption points with their names and directions, and both
/// boxes' outlines and CPs (points and directions) as on Golden A; the `--record` log replayed on the start (the
/// minted representation of a `box.reshape` mapped by its point) gives the same records.
pub fn folio_102_ips_and_black_boxes_are_placed_by_a_generated_script_twice_identically_and_replays_case(
    twice: bool,
) {
    use ew_ui::ew_viewmodel::ew_api::output::PageTables;
    let tmp = Tmp::new();
    let job = start(&bin_exe(), &tmp);
    let (session, _) = Session::open_read_only(&job, "test").unwrap();
    let grid = session.page_grid("102").expect("page 102");
    let map = SheetMap {
        scale_ppb: i128::from(grid.sheet_scale_ppb),
        offset: (
            i128::from(grid.sheet_offset_nm.0),
            i128::from(grid.sheet_offset_nm.1),
        ),
    };
    drop(session);
    let (placements, runs) = placement_steps(102, map, "", &BTreeMap::new());
    assert_eq!(runs, 0, "no command.run step");
    assert!(
        placements.iter().filter(|(a, _)| a == "tool.black_box").count() == 2,
        "two black boxes"
    );
    let mut transcripts = Vec::new();
    let mut records = Vec::new();
    for n in super::support::runs(twice) {
        let dir = tmp.path(n).join("folio.ew");
        copy_dir(&job, &dir);
        let mut script = steps(&[
            ("file.open", json!({ "dir": path(&dir) })),
            ("page.goto", json!({"folio": "102"})),
        ]);
        script.extend(placements.iter().cloned());
        script.extend(steps(&[
            ("ui.describe", json!({})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ]));
        let record = tmp.path(&format!("{n}.ewcmd"));
        let out = run(&bin_exe(), &tmp, &format!("{n}.jsonl"), &script, Some(&record));
        all_done(&out);
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(
            !text.contains(&*tmp.dir().to_string_lossy()),
            "a path in the transcript"
        );
        transcripts.push(text);
        records.push(record);
    }
    if twice {
        if let Some((a, b)) = transcripts[0]
            .lines()
            .zip(transcripts[1].lines())
            .find(|(a, b)| a != b)
        {
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "two runs, two transcripts: first difference at {at}: {} | {}",
                &a[at.saturating_sub(120)..(at + 80).min(a.len())],
                &b[at.saturating_sub(120)..(at + 80).min(b.len())]
            );
        }
        assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
    }
    super::support::assert_no_uuid(&transcripts[0]);
    // The oracle.
    type Boxes = Vec<((i64, i64), Vec<(i64, i64)>, Vec<(u16, (i64, i64), String)>)>;
    let boxes = |t: &PageTables| -> Boxes {
        let mut v: Boxes = t
            .reps
            .iter()
            .filter_map(|r| {
                let b = r.black_box.as_ref()?;
                let mut cps: Vec<_> = b
                    .cps
                    .iter()
                    .map(|(i, p, d)| (*i, *p, d.word().to_owned()))
                    .collect();
                cps.sort();
                Some((r.pos, b.outline.clone(), cps))
            })
            .collect();
        v.sort();
        v
    };
    let ip_rows = |t: &PageTables| {
        let mut v: Vec<_> = t
            .reps
            .iter()
            .filter_map(|r| {
                r.ip.as_ref()
                    .map(|i| (i.name.clone(), i.direction.clone(), r.symbol_key.clone(), r.pos))
            })
            .collect();
        v.sort();
        v
    };
    let (golden_session, _) = Session::open_read_only(Path::new(super::support::GOLDEN_A), "test").unwrap();
    let golden_tables = golden_session.page_tables("102").unwrap();
    let (scripted, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
    let tables = scripted.page_tables("102").unwrap();
    let reps = |t: &PageTables| {
        let mut v: Vec<_> = t
            .reps
            .iter()
            .map(|r| (r.symbol_key.clone(), r.pos, r.orient))
            .collect();
        v.sort();
        v
    };
    assert_eq!(golden_tables.reps.len(), 12);
    assert_eq!(reps(&tables), reps(&golden_tables), "the representations of 102");
    assert_eq!(
        ip_rows(&tables),
        ip_rows(&golden_tables),
        "the interruption points"
    );
    assert_eq!(boxes(&golden_tables).len(), 2);
    assert_eq!(boxes(&tables), boxes(&golden_tables), "the black boxes");
    // `--record` replayed on a third copy of the start: each `box.reshape` names the representation the run minted,
    // mapped to the replay's black box at the same point.
    let replay = tmp.path("c").join("folio.ew");
    copy_dir(&job, &replay);
    let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| **n == "box.black").count(),
        2,
        "{names:?}"
    );
    assert_eq!(names.iter().filter(|n| **n == "ip.place").count(), 9, "{names:?}");
    assert!(names.contains(&"box.reshape"));
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state"));
    let (mut s, _) = Session::open(
        &replay,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    for cmd in log.commands() {
        let mut cmd = cmd.clone();
        if cmd.name == "box.reshape" {
            let id = cmd.params["rep"].as_str().unwrap().to_owned();
            let at = tables
                .reps
                .iter()
                .find(|r| r.id.to_string() == id)
                .expect("the run's box")
                .pos;
            let now = s.page_tables("102").unwrap();
            let mapped = now
                .reps
                .iter()
                .find(|r| r.pos == at && r.black_box.is_some())
                .expect("the replay's box")
                .id;
            cmd.params["rep"] = Value::from(mapped.to_string());
        }
        let line = json!({"cmd": cmd.name, "params": cmd.params});
        let one = CmdLog::parse(&format!("{{\"$schema\":\"ew.cmdlog/1\"}}\n{line}\n")).unwrap();
        s.run_script(&one, "a.ewcmd")
            .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
    }
    let replayed = s.page_tables("102").unwrap();
    assert_eq!(reps(&replayed), reps(&tables));
    assert_eq!(boxes(&replayed), boxes(&tables));
    assert_eq!(records_of(&s), records_of(&scripted));
    s.close();
}

/// WP-S2.04 part 4b's share of §13.6 (PI9): folio 101's structure box around the three external connection
/// points and its two marker groups (W4/W5 and W7/W9, 2.5 BK BU) appended to the wiring script of
/// `folio_101_is_wired_by_a_generated_script_twice_identically_and_replays`; two runs give byte-identical
/// transcripts without a UUID; the oracle: one structure box with the caption "External Connection
/// (+J-E-206 / JE-N1000-03)", two wire markers "2.5 BK BU", no marker fallback, and the wiring oracle unchanged;
/// the `--record` log replayed on a third copy (each `connection.draw` and each `marker.place` mapped) gives the
/// same box outline, caption and marker texts and the same records.
pub fn folio_101_structure_box_and_markers_are_drawn_by_a_generated_script_twice_identically_and_replay_case(
    twice: bool,
) {
    use super::support::{
        external_connection_anchors, golden_wiring, marker_steps, potential_steps, structure_box_steps,
        typed_tag_steps, wiring_oracle, wiring_steps,
    };
    use ew_ui::ew_viewmodel::ew_api::output::{PageTables, TableConnection, TableEnd};
    let tmp = Tmp::new();
    let job = start(&bin_exe(), &tmp);
    let (session, _) = Session::open_read_only(&job, "test").unwrap();
    let grid = session.page_grid("101").expect("page 101");
    let map = SheetMap::of_grid(&grid);
    let q = session.query();
    let strip = q
        .devices()
        .iter()
        .find(|(_, d)| d.aspects.strip.is_some())
        .map(|(id, _)| id.to_string())
        .expect("the start project's strip");
    drop(q);
    drop(session);
    let (placements, runs) = placement_steps(101, map, &strip, &BTreeMap::new());
    // Phase A: the placements alone, to learn the tags they print.
    let placed = tmp.path("p").join("folio.ew");
    copy_dir(&job, &placed);
    let mut script = steps(&[
        ("file.open", json!({ "dir": path(&placed) })),
        ("page.goto", json!({"folio": "101"})),
    ]);
    script.extend(placements.iter().cloned());
    script.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(
        &bin_exe(),
        &tmp,
        "p.jsonl",
        &script,
        Some(&tmp.path("p.ewcmd")),
    ));
    let tags = typed_tag_steps(101, &placed);
    assert!(!tags.is_empty(), "the typed tags of 101");
    let (wiring, commits, _points) = wiring_steps(101, map);
    assert_eq!(commits, 19, "15 plain wires and 2 per junction");
    // The tags and the wiring of the phase-A copy, to learn the rehearsal's page tables: the marker search runs
    // on the drawn routes, not on Golden A's.
    let mut probe = steps(&[
        ("file.open", json!({ "dir": path(&placed) })),
        ("page.goto", json!({"folio": "101"})),
    ]);
    probe.extend(tags.iter().cloned());
    probe.extend(wiring.iter().cloned());
    probe.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(
        &bin_exe(),
        &tmp,
        "probe.jsonl",
        &probe,
        Some(&tmp.path("probe.ewcmd")),
    ));
    let (probe_session, _) = Session::open_read_only(&placed, "test").unwrap();
    let tables = probe_session.page_tables("101").unwrap();
    drop(probe_session);
    let (box_steps, marker) = (
        structure_box_steps(map, (grid.num_nm, grid.den), &external_connection_anchors(101)),
        marker_steps(101, &tables, map),
    );
    let (marker_steps_, fallbacks) = marker;
    assert!(
        fallbacks.is_empty(),
        "no marker fallback on 101: the Marker tool places both groups"
    );
    // The full script, twice.
    let first_tag = tags[0].1["params"]["targets"][0]["ref"].clone();
    let mut transcripts = Vec::new();
    let mut records = Vec::new();
    for n in super::support::runs(twice) {
        let dir = tmp.path(n).join("folio.ew");
        copy_dir(&job, &dir);
        let mut script = steps(&[
            ("file.open", json!({ "dir": path(&dir) })),
            ("page.goto", json!({"folio": "101"})),
        ]);
        script.extend(placements.iter().cloned());
        // A device the script placed, selected and cleared: the transcript names it by its alias (FT-422).
        script.extend(steps(&[
            ("select.set", json!({"refs": [first_tag.clone()]})),
            ("select.set", json!({"ids": []})),
        ]));
        script.extend(tags.iter().cloned());
        script.extend(wiring.iter().cloned());
        script.extend(box_steps.iter().cloned());
        script.extend(marker_steps_.iter().cloned());
        script.extend(steps(&[
            ("ui.describe", json!({})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ]));
        let record = tmp.path(&format!("{n}.ewcmd"));
        let out = run(&bin_exe(), &tmp, &format!("{n}.jsonl"), &script, Some(&record));
        all_done(&out);
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(
            !text.contains(&*tmp.dir().to_string_lossy()),
            "a path in the transcript"
        );
        transcripts.push(text);
        records.push(record);
    }
    if twice {
        if let Some((a, b)) = transcripts[0]
            .lines()
            .zip(transcripts[1].lines())
            .find(|(a, b)| a != b)
        {
            let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
            panic!(
                "two runs, two transcripts: first difference at {at}: {} | {}",
                &a[at.saturating_sub(120)..(at + 80).min(a.len())],
                &b[at.saturating_sub(120)..(at + 80).min(b.len())]
            );
        }
        assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
    }
    super::support::assert_no_uuid(&transcripts[0]);
    // §13.6's golden potential definitions of folio 101 through a Session (the script names no minted function);
    // they colour the marker groups.
    let apply_potentials = |dir: &Path, state: &str| {
        let potentials = potential_steps(&[101], dir);
        assert!(!potentials.is_empty(), "the golden potentials of folio 101");
        let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
        store.state_dir = Some(tmp.path(state));
        let (mut p, _) = Session::open(
            dir,
            store,
            ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
            ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
        )
        .unwrap();
        for (label, cmd) in &potentials {
            p.execute(cmd)
                .unwrap_or_else(|e| panic!("potential {label}: {e:?}"));
        }
        p.save().unwrap();
        p.close();
    };
    for n in super::support::runs(twice) {
        apply_potentials(&tmp.path(n).join("folio.ew"), &format!("state-pot-{n}"));
    }
    // The oracle.
    let (scripted, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
    let tables = scripted.page_tables("101").unwrap();
    assert_eq!(tables.reps.len(), 17, "representations on 101");
    assert_eq!(wiring_oracle(&tables), golden_wiring(101), "the wiring of 101");
    assert_eq!(tables.structure_boxes.len(), 1, "one structure box on 101");
    assert_eq!(
        tables.structure_boxes[0].caption,
        "External Connection (+J-E-206 / JE-N1000-03)"
    );
    let mut markers: Vec<String> = tables
        .items
        .iter()
        .filter(|i| i.kind == "wire_marker")
        .filter_map(|i| i.text.clone())
        .collect();
    markers.sort();
    assert_eq!(markers, ["2.5 BK BU", "2.5 BK BU"]);
    assert_eq!(runs, 0, "the placements leave no command.run step");
    // `--record` replayed on a third copy of the start: each `connection.draw` end (by its CP's point and index)
    // and each `marker.place` connection (by its two end labels) mapped to the replay's.
    let replay = tmp.path("c").join("folio.ew");
    copy_dir(&job, &replay);
    let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| **n == "connection.draw").count(),
        19,
        "{names:?}"
    );
    assert_eq!(
        names.iter().filter(|n| **n == "marker.place").count(),
        2,
        "{names:?}"
    );
    assert_eq!(
        names.iter().filter(|n| **n == "box.structure").count(),
        1,
        "{names:?}"
    );
    let ends = |t: &PageTables, c: &TableConnection| -> (Option<String>, Option<String>) {
        let label = |e: TableEnd| match e {
            TableEnd::Cp { function, idx } => t
                .cps
                .iter()
                .find(|x| x.function == function && x.idx == idx)
                .map(|x| x.label.clone()),
            TableEnd::Junction { .. } => None,
        };
        (label(c.a), label(c.b))
    };
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state"));
    let (mut s, _) = Session::open(
        &replay,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    for cmd in log.commands() {
        let mut cmd = cmd.clone();
        if cmd.name == "connection.draw" {
            let now = s.page_tables("101").unwrap();
            let map_cp = |f: &str, idx: u64| -> String {
                let at = tables
                    .cps
                    .iter()
                    .find(|c| c.function.to_string() == f && u64::from(c.idx) == idx)
                    .expect("a CP of the run")
                    .at;
                now.cps
                    .iter()
                    .find(|c| c.at == at && u64::from(c.idx) == idx)
                    .expect("the CP of the replay")
                    .function
                    .to_string()
            };
            for end in ["a", "b"] {
                let e = &mut cmd.params[end];
                if let Some(cp) = e.get_mut("cp") {
                    let f = map_cp(cp["function"].as_str().unwrap(), cp["idx"].as_u64().unwrap());
                    cp["function"] = Value::from(f);
                } else if let Some(on) = e.get_mut("on_connection") {
                    let id = on["connection"].as_str().unwrap().to_owned();
                    let w = tables
                        .connections
                        .iter()
                        .find(|c| c.id.to_string() == id)
                        .expect("the wire");
                    let TableEnd::Cp { function, idx } = w.a else {
                        panic!("the split wire starts at a CP");
                    };
                    let f = map_cp(&function.to_string(), u64::from(idx));
                    let mapped = now
                        .connections
                        .iter()
                        .find(|c| {
                            matches!(c.a, TableEnd::Cp { function, idx: i }
                                if function.to_string() == f && i == idx)
                        })
                        .expect("the wire of the replay")
                        .id;
                    on["connection"] = Value::from(mapped.to_string());
                }
            }
        }
        if cmd.name == "marker.place" {
            let now = s.page_tables("101").unwrap();
            let ids: Vec<String> = cmd.params["connections"]
                .as_array()
                .expect("the marker's connections")
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect();
            let mut mapped = Vec::new();
            for id in ids {
                let connection = tables
                    .connections
                    .iter()
                    .find(|c| c.id.to_string() == id)
                    .expect("the marker's connection");
                let want = ends(&tables, connection);
                let found = now
                    .connections
                    .iter()
                    .find(|c| {
                        let have = ends(&now, c);
                        have == want || (have.0 == want.1 && have.1 == want.0)
                    })
                    .expect("the marker's connection in the replay");
                mapped.push(found.id.to_string());
            }
            cmd.params["connections"] = json!(mapped);
        }
        let line = json!({"cmd": cmd.name, "params": cmd.params});
        let one = CmdLog::parse(&format!("{{\"$schema\":\"ew.cmdlog/1\"}}\n{line}\n")).unwrap();
        s.run_script(&one, "a.ewcmd")
            .unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
    }
    s.save().unwrap();
    s.close();
    // The same golden potentials on the replay (its functions are its own, so they map again).
    apply_potentials(&replay, "state-pot-replay");
    let (replayed_s, _) = Session::open_read_only(&replay, "test").unwrap();
    let replayed = replayed_s.page_tables("101").unwrap();
    assert_eq!(
        replayed.structure_boxes.len(),
        1,
        "one structure box in the replay"
    );
    assert_eq!(
        replayed.structure_boxes[0].outline, tables.structure_boxes[0].outline,
        "the box outline"
    );
    assert_eq!(
        replayed.structure_boxes[0].caption,
        "External Connection (+J-E-206 / JE-N1000-03)"
    );
    let mut replayed_markers: Vec<String> = replayed
        .items
        .iter()
        .filter(|i| i.kind == "wire_marker")
        .filter_map(|i| i.text.clone())
        .collect();
    replayed_markers.sort();
    assert_eq!(replayed_markers, markers, "the marker texts of the replay");
    assert_eq!(records_of(&replayed_s), records_of(&scripted));
    replayed_s.close();
}
