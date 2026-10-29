// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-408 and FT-409 through the built binary (the pilot's prerequisites `page.new`, `structure.new` and
//! `external_ref.new`, decision log ID-619 and ID-623): no window, no GPU, no display.
//!
//! - **Pages.** On a new project (`file.new`) a script runs `page.new` for folios 101 to 103 and the describe text
//!   lists the three; two runs give byte-identical transcripts and the `--record` log replays on a copy of the start
//!   (the session's command-log replay, which `ew apply` runs).
//! - **Folio 101's external points.** The location `+J-E-206` and the external reference `JE-N1000-03` are made by the
//!   two actions on the new project; folio 101's generated placement script then places its interruption points with
//!   the external reference named by the three external arrows, and the arrows print "+J-E-206 / JE-N1000-03"
//!   (doc 20 §3.4 item 9, FR-XRF-012).
//! - **Navigators (WP-S2.06 part 1).** On a copy of Golden A a script marks -201U3 in the Devices navigator, opens it
//!   and reorders 106 before 103; two runs give byte-identical transcripts and the record replays.
//! - **Highlighting (WP-S2.06 part 2).** On a copy of Golden A a script highlights 24VDC-1 from the Potentials
//!   navigator, shows folios 102 and 103 and clears it; two runs give byte-identical transcripts and nothing is recorded
//!   (view state).
//!
//! The script tests run their script once per gate; the second run and the byte-identical comparison are the deep
//! nightly's (`headless::nightly_every_script_transcript_is_byte_identical_twice`, test clean-up 2026-10-30). Their
//! bodies and the helpers are `create/scripts.rs`.

mod support;

#[path = "create/scripts.rs"]
mod scripts;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_ui::ew_viewmodel::ew_api::output::RowValue;
use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use scripts::{CONTENT, Tmp, all_done, copy_dir, new_project, path, run, same_transcript, steps};
use serde_json::{Value, json};
use support::{SheetMap, golden_reps, placement_steps};

/// The folio-101 start of §13.6 on a project whose structure the two actions made: the page, the strip and the IP
/// symbols as in `canvas.rs`'s start, the location `+J-E-206` and its external reference `JE-N1000-03`.
fn folio_start(tmp: &Tmp) -> PathBuf {
    let embed = |files: &[&str]| -> Value {
        let copies: Vec<Value> = files
            .iter()
            .map(|f| {
                serde_json::from_str(&std::fs::read_to_string(Path::new(CONTENT).join(f)).unwrap()).unwrap()
            })
            .collect();
        json!({ "copies": copies })
    };
    new_project(
        tmp,
        "folio.ew",
        &[
            (
                "page.new",
                json!({"folio": "101", "type": "multi_line", "title": "Power Supply 1 - Main Current"}),
            ),
            (
                "structure.new",
                json!({"kind": "location", "designation": "J-E-206"}),
            ),
            (
                "external_ref.new",
                json!({"location": "J-E-206", "device": "JE-N1000-03"}),
            ),
            (
                "command.run",
                json!({"cmd": "device.create", "params": {"letter": "X", "counter": 1, "strip": {}}}),
            ),
            (
                "command.run",
                json!({"cmd": "library.embed", "params": embed(&[
                    "symbols/qet10/ew.gen.ip@qet10.json",
                    "fndefs/ew.fn.ip.json",
                    "symbols/qet10/ew.gen.external-arrow@qet10.json",
                    "fndefs/ew.fn.external-arrow.json",
                ])}),
            ),
        ],
    )
}

/// FT-409's acceptance (b): folio 101's external points print the texts of the node and the reference the two actions
/// made.
#[test]
fn folio_101s_external_points_print_the_structure_node_and_the_external_reference() {
    let tmp = Tmp::new();
    let job = folio_start(&tmp);
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
    // What the two actions made: one node `J-E-206` of the location aspect, one reference at it.
    let nodes: Vec<(String, String)> = q
        .structure_nodes()
        .iter()
        .map(|(_, n)| {
            (
                serde_json::to_value(&n.aspect)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
                n.code.as_str().to_owned(),
            )
        })
        .collect();
    assert_eq!(nodes, [("location".to_owned(), "J-E-206".to_owned())]);
    let refs: Vec<_> = q
        .external_refs()
        .iter()
        .map(|(id, e)| (id.to_string(), e.device.as_str().to_owned()))
        .collect();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].1, "JE-N1000-03");
    let external = refs[0].0.clone();
    drop(q);
    drop(session);
    // The placement script of §13.6 (WP-S2.04 part 3: the IP tool) with the three external arrows naming the
    // reference this project holds by its ID (the golden script names the golden references' printed texts).
    let (mut placements, runs) = placement_steps(101, map, &strip, &BTreeMap::new());
    let mut external_points = 0;
    for i in 0..placements.len() {
        if placements[i].0 == "ip.external" {
            let next = &placements[i + 1];
            assert_eq!(next.0, "ip.name");
            assert!(
                next.1["text"].as_str().is_some_and(|n| n.starts_with("J-E-206-")),
                "{next:?}"
            );
            placements[i].1["external"] = json!(format!("id:{external}"));
            external_points += 1;
        }
    }
    assert_eq!(external_points, 3, "the three external arrows of folio 101");
    assert_eq!(
        runs, 0,
        "the IP tool places every point: no command.run step is left"
    );
    let mut transcripts = Vec::new();
    let mut records = Vec::new();
    for n in ["a", "b"] {
        let dir = tmp.path(n).join("folio.ew");
        copy_dir(&job, &dir);
        let mut script = steps(&[
            ("file.open", json!({"dir": path(&dir)})),
            ("page.goto", json!({"folio": "101"})),
        ]);
        script.extend(placements.iter().cloned());
        script.extend(steps(&[
            ("ui.describe", json!({})),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ]));
        let record = tmp.path(&format!("{n}.ewcmd"));
        let text = all_done(&run(&tmp, n, &script, Some(&record)));
        support::assert_no_uuid(&text);
        transcripts.push(text);
        records.push(record);
    }
    same_transcript(&transcripts[0], &transcripts[1]);
    // The oracle of §13.6 still holds: 17 representations, each the golden symbol at the golden point.
    let mut golden: Vec<(String, (i64, i64), u8)> = golden_reps(101)
        .into_iter()
        .map(|r| (r.symbol, r.pos, r.orient))
        .collect();
    golden.sort();
    let (placed_session, _) = Session::open_read_only(&tmp.path("a").join("folio.ew"), "test").unwrap();
    let tables = placed_session.page_tables("101").unwrap();
    let mut placed: Vec<(String, (i64, i64), u8)> = tables
        .reps
        .iter()
        .map(|r| (r.symbol_key.clone(), r.pos, r.orient))
        .collect();
    placed.sort();
    assert_eq!(placed, golden);
    // The three external arrows print "+J-E-206 / JE-N1000-03" in their cross-reference row.
    let arrows: Vec<_> = tables
        .reps
        .iter()
        .filter(|r| r.ip.as_ref().is_some_and(|ip| ip.name.starts_with("J-E-206-")))
        .collect();
    assert_eq!(arrows.len(), 3);
    for rep in arrows {
        let sheet = placed_session.property_sheet(&[rep.id]);
        let row = sheet
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.key == "representation/derived.xref")
            .expect("the representation's derived cross-reference row");
        let RowValue::One { text, .. } = &row.value else {
            panic!("{:?}", row.value);
        };
        assert_eq!(text, "+J-E-206 / JE-N1000-03", "{:?}", rep.ip);
    }
    // The record replays on a copy of the start, where the node and the reference already are.
    let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    let replay = tmp.path("c").join("folio.ew");
    copy_dir(&job, &replay);
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
    assert_eq!(s.page_tables("101").unwrap().reps.len(), 17);
}
/// FT-408's acceptance (a): folios 101 to 103 on a new project.
#[test]
fn page_new_creates_folios_101_to_103_twice_identically_and_the_record_replays() {
    scripts::page_new_creates_folios_101_to_103_twice_identically_and_the_record_replays_case(false);
}

/// WP-S2.06 part 1 through the built binary (acceptance checks NV5, NV7 and NV10; §9 items 1 and 2): a script on a
/// copy of Golden A opens the Devices navigator with folio 102 shown, marks -201U3 by type-ahead and Space (the chips
/// 201-E4 and 203-B4), opens it (folio 201), reorders 106 before 103 through the card, and saves. Two runs give
/// byte-identical transcripts without a UUID; the `--record` log holds one `page.reorder` and replays on a third copy
/// to the same pages.
#[test]
fn a_navigator_script_runs_twice_identically_and_its_record_replays() {
    scripts::a_navigator_script_runs_twice_identically_and_its_record_replays_case(false);
}

// WP-S2.06 part 2
/// WP-S2.06 part 2 through the built binary (acceptance checks NP3 and NP6; §9 items 1 and 2): a script on a copy of
/// Golden A opens the further navigators, highlights 24VDC-1 from the Potentials navigator (type-ahead, then
/// `navigator.highlight` on the cursor row), shows folios 102 and 103 and clears the highlights. Two runs give
/// byte-identical transcripts without a UUID; the canvas section names the highlight on both folios; the `--record`
/// log is empty (a highlight is view state, never a command).
#[test]
fn a_highlight_script_runs_twice_identically_and_records_nothing() {
    scripts::a_highlight_script_runs_twice_identically_and_records_nothing_case(false);
}

/// WP-S2.06 part 3 through the built binary (acceptance checks TB2 and TB8; §9 items 1 and 2): a script on a copy of
/// Golden A shows the Terminal strips navigator as a table filtered `x1031` and sorted by Terminal, sets terminal 13's
/// function text to "Spare" by a cell edit, marks terminals 13 to 17 and fills down, copies the marked rows and saves.
/// Two runs give byte-identical transcripts without a UUID; the `--record` log holds the two `fields.set` steps and
/// replays on a third copy to the same Terminal strips table.
#[test]
fn a_table_script_runs_twice_identically_and_its_record_replays() {
    scripts::a_table_script_runs_twice_identically_and_its_record_replays_case(false);
}
