// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 5 through the built binary (acceptance check IM2): a UI script brings a placeholder SVG into a copy
//! of Golden A, arms the Image tool, picks the asset and places it by two grid-cursor corners; two runs give
//! byte-identical transcripts without a UUID, the ghost member and describe line name the asset, `page.describe` lists
//! the `image` item, the saved project holds it, and the `--record` log replayed on a third copy (the session's
//! command-log replay, what `ew apply` runs) gives the same item. Golden A itself is only copied, never written.

mod support;

use std::path::Path;

use ew_ui::ew_viewmodel::ew_api::{CmdLog, Session};
use serde_json::{Value, json};
use support::{GOLDEN_A, Tmp, all_done, assert_no_uuid, copy_dir, path, run, steps};

/// The placeholder asset: an SVG of 40 × 10 user units (no company's artwork).
const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10" fill="#123456"/></svg>"##;
/// Its hash.
const SVG_HASH: &str = "sha256:ec57251790a327f9afde596a256f674c950610537531d4ed0b07bb248882a5d1";

/// The path of the `elektrawatt` binary under test (nextest's run-time path, else the compile-time one).
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> std::path::PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

/// The script: the asset into the project (the company logo command, taken off again), the Image tool, the pick, two
/// corners on the grid cursor, the describe texts.
fn script(dir: &Path) -> Vec<(String, Value)> {
    let b64 = ew_ui::ew_viewmodel::ew_api::base64_encode(SVG);
    steps(&[
        ("file.open", json!({ "dir": path(dir) })),
        ("page.goto", json!({"folio": "101"})),
        (
            "command.run",
            json!({"cmd": "project.company.set", "params": {"logo": b64}}),
        ),
        (
            "command.run",
            json!({"cmd": "project.company.set", "params": {"logo": ""}}),
        ),
        ("tool.image", json!({})),
        ("ui.describe", json!({})),
        ("image.pick", json!({ "asset": SVG_HASH })),
        ("ui.describe", json!({})),
        ("cursor.set", json!({"x_mm": 40.0, "y_mm": 60.0})),
        ("canvas.activate", json!({})),
        ("cursor.set", json!({"x_mm": 100.0, "y_mm": 90.0})),
        ("ui.describe", json!({})),
        ("canvas.activate", json!({})),
        ("page.describe", json!({})),
        ("ui.describe", json!({})),
        ("file.save", json!({})),
        ("file.close", json!({})),
    ])
}

#[test]
fn im2_an_image_is_placed_by_a_script_twice_identically_and_replays() {
    let tmp = Tmp::new();
    let mut transcripts = Vec::new();
    let mut records = Vec::new();
    for n in ["a", "b"] {
        let dir = tmp.path(n).join("J-E-2416.ew");
        copy_dir(Path::new(GOLDEN_A), &dir);
        let record = tmp.path(&format!("{n}.ewcmd"));
        let out = run(
            &bin_exe(),
            &tmp,
            &format!("{n}.jsonl"),
            &script(&dir),
            Some(&record),
        );
        all_done(&out);
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(
            !text.contains(&*tmp.dir().to_string_lossy()),
            "a path in the transcript"
        );
        transcripts.push(text);
        records.push(record);
    }
    assert_eq!(transcripts[0], transcripts[1], "two runs, one transcript");
    assert_no_uuid(&transcripts[0]);

    // (Line 0 of a transcript is its header: step n is line n.)
    let lines: Vec<Value> = transcripts[0]
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let canvas = |n: usize| lines[n]["state"]["canvas"].clone();
    assert_eq!(canvas(5)["tool"], json!({"name": "image", "state": "picking"}));
    assert_eq!(canvas(5)["ghost"], json!({"kind": "image", "asset": null}));
    assert_eq!(canvas(7)["tool"], json!({"name": "image", "state": "armed"}));
    assert_eq!(canvas(7)["ghost"], json!({"kind": "image", "asset": SVG_HASH}));
    assert!(
        lines[8]["describe"]
            .as_str()
            .unwrap()
            .contains(&format!("canvas ghost item image \"{SVG_HASH}\"")),
        "the describe line names the asset"
    );
    assert_eq!(canvas(10)["tool"]["state"], "placing");
    assert_eq!(
        canvas(13)["tool"]["state"],
        "armed",
        "armed again with the asset kept"
    );
    assert!(
        transcripts[0].contains("item image cell"),
        "page.describe lists the image item"
    );

    // The saved project holds one image item; the `--record` log replayed on a third copy gives the same item.
    let (scripted, _) = Session::open_read_only(&tmp.path("a").join("J-E-2416.ew"), "test").unwrap();
    let rows = |s: &Session| {
        s.page_tables("101")
            .unwrap()
            .items
            .iter()
            .filter(|i| i.kind == "image")
            .map(|i| i.bounds)
            .collect::<Vec<_>>()
    };
    let placed = rows(&scripted);
    assert_eq!(placed.len(), 1, "one image on 101");
    // The save wrote the asset the item names: the project reopened from disk holds it (the logo was taken off, so the
    // item is the only record that names it), as a file `assets/<hash>.svg`.
    let hash: ew_ui::ew_viewmodel::ew_api::Sha256 = SVG_HASH.parse().unwrap();
    assert!(
        scripted.asset(&hash).is_some(),
        "the reopened project holds the image's asset"
    );
    let file = SVG_HASH.trim_start_matches("sha256:");
    assert!(
        tmp.path("a")
            .join("J-E-2416.ew")
            .join("assets")
            .join(format!("{file}.svg"))
            .is_file(),
        "assets/<hash>.svg is on disk"
    );
    let [x0, y0, x1, y1] = placed[0];
    assert!(x1 > x0 && y1 > y0);
    assert!(
        ((x1 - x0) - 4 * (y1 - y0)).abs() <= 2,
        "the 4:1 ratio of the SVG: {:?}",
        placed[0]
    );

    let replay = tmp.path("c").join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &replay);
    let log = CmdLog::parse(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    let names: Vec<&str> = log.commands().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names.iter().filter(|n| **n == "item.place").count(),
        1,
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
    s.run_script(&log, "a.ewcmd").unwrap();
    assert_eq!(
        rows(&s),
        placed,
        "the replay's item equals the scripted session's"
    );
    assert!(s.asset(&hash).is_some(), "the replay brought the asset in too");
    s.close();
}
