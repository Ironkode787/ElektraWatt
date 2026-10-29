// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support of the folio scripts (WP-S2.04 §13.6): the generator of a folio's UI script from Golden A's records,
//! read as the JSON Lines of the committed fixture (`golden/J-E-2416.ew`, never written).
//!
//! **What part 1 draws.** Each representation of the folio by the gesture of the part that owns it: a symbol with
//! the Place tool (`place.pick` with its key and the device's letter, then the grid cursor on the golden insertion
//! point and Enter, so no pointer and no hover reaches the transcript); a terminal with the Place tool's
//! `new_terminal` target on the strip the start project holds, with the golden designation; an interruption point
//! with the IP tool (part 3: `tool.ip`, `ip.external` with the printed text of its external reference, `ip.name`,
//! `tool.ip_direction {direction}`, the grid cursor on the golden insertion point and Enter); each placement of a
//! symbol with Alt held (`canvas.activate {alt}`), so that no autoconnect proposal adds a wire.
//!
//! **What part 2 adds** (§13.6): the golden tags typed through `command.run` `fields.set` on `tag:` references to
//! the tags the placements print ([`typed_tag_steps`], read from a run of the placements, so no minted ID enters the
//! script), and every connection and junction drawn with the Wire tool from the keyboard ([`wiring_steps`]); the
//! oracle ([`wiring_oracle`]) compares end labels and junction sequences with Golden A's.
//!
//! **What part 4b adds** (§13.6, PI9 and the G-PP rehearsal): the start of the rehearsal ([`gpp_start`]: pages 101 to
//! 103, the golden library copies, the strips -X1031/-X1032 with their terminals), the strip segments placed on their
//! strip's main function ([`placement_steps`]), the records no tool draws ([`part_steps`]), the junction editor's
//! golden styles and wiring ([`junction_style_steps`]), the structure box around the external points
//! ([`structure_box_steps`]), the four marker groups through the Marker tool ([`marker_steps`]) and the pages'
//! cross-reference texts ([`xref_texts`]). The script runs, the temporary directory and the start project are here
//! too, shared by `canvas.rs`, `create.rs` and `gpp_demo.rs`.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_ui::ew_viewmodel::ew_api::{AnyCommand, Session};
use serde_json::{Value, json};

/// The committed fixture of Golden A.
pub const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// The first match in `text` of the UUID pattern `[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}`, if any: the
/// check that no record ID reaches a transcript line or a describe text, which print the alias `@n` instead (FT-422,
/// decision log ID-630). Written here on its own, not with the printer's scanner, so that the two check each other.
pub fn first_uuid(text: &str) -> Option<&str> {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let b = text.as_bytes();
    'start: for i in 0..b.len() {
        let mut j = i;
        for (g, n) in GROUPS.iter().enumerate() {
            if g > 0 {
                if b.get(j) != Some(&b'-') {
                    continue 'start;
                }
                j += 1;
            }
            for _ in 0..*n {
                if !b.get(j).is_some_and(u8::is_ascii_hexdigit) {
                    continue 'start;
                }
                j += 1;
            }
        }
        // ASCII from `i` to `j`, so both are character boundaries.
        return Some(&text[i..j]);
    }
    None
}

/// The runs of a script test: `["a"]` per gate, `["a", "b"]` when `twice`. A script test runs its script once per
/// gate (transcript determinism per gate is `headless::the_transcript_is_deterministic`, A7); the deep nightly's
/// `headless::nightly_every_script_transcript_is_byte_identical_twice` (CI-N-02) runs each such test with `twice`, which
/// adds the second run on an equal start and the byte-identical comparison of the two transcripts.
pub fn runs(twice: bool) -> &'static [&'static str] {
    if twice { &["a", "b"] } else { &["a"] }
}

/// Asserts that no line of `transcript` holds a UUID, naming the step and the UUID when one does.
#[track_caller]
pub fn assert_no_uuid(transcript: &str) {
    for (n, line) in transcript.lines().enumerate() {
        if let Some(uuid) = first_uuid(line) {
            panic!("transcript line {n} holds the UUID {uuid}: {line}");
        }
    }
}

/// The records of a JSON Lines file.
fn lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Every record of the fixture's JSON Lines shards (the page and device shards and the top-level shards), by ID.
fn records() -> BTreeMap<String, Value> {
    let root = Path::new(GOLDEN_A);
    let mut files = Vec::new();
    collect_jsonl(root, &mut files);
    files.sort();
    let mut out = BTreeMap::new();
    for f in files {
        for r in lines(&f) {
            if let Some(id) = r["id"].as_str() {
                out.insert(id.to_owned(), r);
            }
        }
    }
    out
}

/// Every `*.jsonl` file below `dir`, recursively.
fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
        let path = e.expect("a directory entry").path();
        if path.is_dir() {
            collect_jsonl(&path, out);
        } else if path.extension().is_some_and(|x| x == "jsonl") {
            out.push(path);
        }
    }
}

/// The external references of the fixture (`structure.jsonl`), in record order.
pub fn external_refs() -> Vec<Value> {
    lines(&Path::new(GOLDEN_A).join("structure.jsonl"))
        .into_iter()
        .filter(|r| r["$k"] == "external_ref")
        .collect()
}

/// The printed text (`<device>:<connection>`) of the fixture's external reference `id`, as the IP tool's field offers
/// it.
pub fn external_label(id: &str) -> String {
    let r = external_refs()
        .into_iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("no external reference {id}"));
    match r["connection"].as_str() {
        Some(c) => format!("{}:{c}", r["device"].as_str().unwrap()),
        None => r["device"].as_str().unwrap().to_owned(),
    }
}

/// The key (`meta.key`) of every symbol copy of the fixture, by copy ID.
fn symbol_keys() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for e in std::fs::read_dir(Path::new(GOLDEN_A).join("lib/symbols")).unwrap() {
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(e.unwrap().path()).unwrap()).unwrap();
        out.insert(
            doc["id"].as_str().unwrap().to_owned(),
            doc["meta"]["key"].as_str().unwrap().to_owned(),
        );
    }
    out
}

/// One golden representation of a folio, as the oracle compares it.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct GoldenRep {
    /// The golden table's key (`F55`, `1X1:PE`, `L-1@101`).
    pub key: String,
    /// The symbol's key.
    pub symbol: String,
    /// The insertion point, page nm.
    pub pos: (i64, i64),
    /// The orientation `0..=7`.
    pub orient: u8,
}

/// The orientation index of a record's `orient` (absent: A).
fn orient_of(v: &Value) -> u8 {
    const NAMES: [&str; 8] = ["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"];
    v.as_str()
        .and_then(|s| NAMES.iter().position(|n| *n == s))
        .map_or(0, |i| i as u8)
}

/// The golden representations of `folio`, in key order.
pub fn golden_reps(folio: u32) -> Vec<GoldenRep> {
    let all = records();
    let keys = symbol_keys();
    let page = all
        .values()
        .find(|r| r["$k"] == "page" && r["folio"]["number"] == folio)
        .unwrap_or_else(|| panic!("no page {folio}"))["id"]
        .clone();
    let mut out: Vec<GoldenRep> = all
        .values()
        .filter(|r| r["$k"] == "representation" && r["page"] == page)
        .map(|r| GoldenRep {
            key: r["source_ref"]["key"]
                .as_str()
                .unwrap_or_default()
                .trim_start_matches("representations|")
                .to_owned(),
            symbol: keys[r["symbol"].as_str().unwrap()].clone(),
            pos: (r["pos"][0].as_i64().unwrap(), r["pos"][1].as_i64().unwrap()),
            orient: orient_of(&r["orient"]),
        })
        .collect();
    out.sort();
    out
}

/// The sheet map of a page: `sheet = offset + page × scale_ppb / 10⁹`, rounded half away from zero.
#[derive(Clone, Copy, Debug)]
pub struct SheetMap {
    /// Parts per 10⁹.
    pub scale_ppb: i128,
    /// The sheet point of the page origin, nm.
    pub offset: (i128, i128),
}

impl SheetMap {
    /// The map of a page's [`PageGrid`](ew_ui::ew_viewmodel::ew_api::output::PageGrid) (`Session::page_grid`).
    pub fn of_grid(grid: &ew_ui::ew_viewmodel::ew_api::output::PageGrid) -> SheetMap {
        SheetMap {
            scale_ppb: i128::from(grid.sheet_scale_ppb),
            offset: (
                i128::from(grid.sheet_offset_nm.0),
                i128::from(grid.sheet_offset_nm.1),
            ),
        }
    }

    /// The sheet point of a page point, in mm (six decimals are whole nm).
    pub fn mm(&self, p: (i64, i64)) -> (f64, f64) {
        let axis = |v: i64, off: i128| {
            let num = i128::from(v) * self.scale_ppb;
            let q = num / 1_000_000_000;
            let r = num % 1_000_000_000;
            let q = if 2 * r.abs() >= 1_000_000_000 {
                q + num.signum()
            } else {
                q
            };
            (off + q) as f64 / 1e6
        };
        (axis(p.0, self.offset.0), axis(p.1, self.offset.1))
    }
}

/// The steps of the UI script that place every representation of folio `folio` with part 1's gestures (see the
/// module documentation): `strip` is the ID of the strip the start project holds for the folio's terminals, and
/// `functions` maps a golden strip function's ID to the start project's (`place.pick` with `function`; a strip
/// segment is placed on the strip's main function). Also returns the number of `command.run` steps (the steps a
/// later part owns).
pub fn placement_steps(
    folio: u32,
    map: SheetMap,
    strip: &str,
    functions: &BTreeMap<String, String>,
) -> (Vec<(String, Value)>, usize) {
    placement_steps_with(folio, map, strip, functions, &|_| true)
}

/// [`placement_steps`] over the golden representations `keep` accepts (the folio-103 demo variant leaves the strip
/// segments out; every other caller keeps every representation).
pub fn placement_steps_with(
    folio: u32,
    map: SheetMap,
    strip: &str,
    functions: &BTreeMap<String, String>,
    keep: &dyn Fn(&Value) -> bool,
) -> (Vec<(String, Value)>, usize) {
    let all = records();
    let keys = symbol_keys();
    let page = all
        .values()
        .find(|r| r["$k"] == "page" && r["folio"]["number"] == folio)
        .unwrap()["id"]
        .clone();
    let mut reps: Vec<&Value> = all
        .values()
        .filter(|r| r["$k"] == "representation" && r["page"] == page)
        .collect();
    reps.sort_by_key(|r| r["source_ref"]["key"].as_str().unwrap_or_default().to_owned());
    let mut steps: Vec<(String, Value)> = Vec::new();
    let runs = 0;
    for r in reps {
        if !keep(r) {
            continue;
        }
        let symbol = keys[r["symbol"].as_str().unwrap()].clone();
        let pos = (r["pos"][0].as_i64().unwrap(), r["pos"][1].as_i64().unwrap());
        assert_eq!(
            orient_of(&r["orient"]),
            0,
            "part 1's generator places orientation A only"
        );
        let function = &all[r["functions"][0].as_str().unwrap()];
        if let Some(ip) = function["kind"].get("ip") {
            // WP-S2.04 part 3: the IP tool; an external connection point names its external reference by printed text
            // (the start project holds the references).
            steps.push(("tool.ip".into(), json!({})));
            if let Some(e) = ip["external"].as_str() {
                steps.push(("ip.external".into(), json!({ "external": external_label(e) })));
            }
            steps.push(("ip.name".into(), json!({"text": ip["name"]})));
            steps.push(("tool.ip_direction".into(), json!({"direction": ip["direction"]})));
            let (x, y) = map.mm(pos);
            steps.push(("cursor.set".into(), json!({"x_mm": x, "y_mm": y})));
            steps.push(("canvas.activate".into(), json!({})));
            steps.push(("ui.dismiss".into(), json!({})));
            continue;
        }
        let device = &all[function["device"].as_str().unwrap()];
        if let Some(b) = r.get("box") {
            let grid = &all[page.as_str().unwrap()]["grid"];
            let grid = (grid["num"].as_i64().unwrap(), grid["den"].as_i64().unwrap());
            steps.extend(black_box_steps(map, grid, pos, b, device));
            continue;
        }
        if r["rep_type"].as_str() == Some("strip_segment") {
            // WP-S2.04 §13.6: the strip segment is another representation of the strip's main function.
            let golden = r["functions"][0].as_str().unwrap();
            let target = functions
                .get(golden)
                .unwrap_or_else(|| panic!("no start-project function for the strip segment {golden}"));
            steps.push(("place.pick".into(), json!({"symbol": symbol, "function": target})));
            let (x, y) = map.mm(pos);
            steps.push(("cursor.set".into(), json!({"x_mm": x, "y_mm": y})));
            steps.push(("canvas.activate".into(), json!({"alt": true})));
            steps.push(("ui.dismiss".into(), json!({})));
            continue;
        }
        let pick = match device.get("terminal") {
            Some(t) => json!({"symbol": symbol, "strip": strip, "designation": t["designation"]}),
            None => json!({"symbol": symbol, "letter": device["tag"]["letter"]}),
        };
        let (x, y) = map.mm(pos);
        steps.push(("place.pick".into(), pick));
        steps.push(("cursor.set".into(), json!({"x_mm": x, "y_mm": y})));
        // With Alt: the placement sends no autoconnect proposals (part 2); the wires are the Wire tool's.
        steps.push(("canvas.activate".into(), json!({"alt": true})));
        steps.push(("ui.dismiss".into(), json!({})));
    }
    (steps, runs)
}

/// The part number of the fixture's part copy `id` (`lib/parts/<id>.json`).
fn part_number(id: &str) -> String {
    let path = Path::new(GOLDEN_A).join("lib/parts").join(format!("{id}.json"));
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc["part_number"].as_str().unwrap().to_owned()
}

/// A press, a move and a release of the pointer from `from` to `to` (page nm), with the Select tool.
fn drag(map: SheetMap, from: (i64, i64), to: (i64, i64)) -> Vec<(String, Value)> {
    let (a, b) = (map.mm(from), map.mm(to));
    vec![
        ("canvas.press".into(), json!({"x_mm": a.0, "y_mm": a.1})),
        ("canvas.pointer".into(), json!({"x_mm": b.0, "y_mm": b.1})),
        ("canvas.release".into(), json!({"x_mm": b.0, "y_mm": b.1})),
    ]
}

/// WP-S2.04 part 3: the steps that draw the golden black box `b` (a rectangle at `pos`, its device `device` made from a
/// part) with the black box tool and its CPs: `tool.black_box` with the part's template entry 0 and the device's letter,
/// the two corners by the grid cursor and Enter, then (Select, the box clicked) each CP dragged from the tool's
/// default layout to its golden point along the outline, a CP whose target is free first (ascending index).
fn black_box_steps(
    map: SheetMap,
    grid: (i64, i64),
    pos: (i64, i64),
    b: &Value,
    device: &Value,
) -> Vec<(String, Value)> {
    use ew_ui::ew_viewmodel::tools::black_box::layout;
    use ew_ui::ew_viewmodel::tools::wire::PageGridMap;
    let pt = |v: &Value| (v[0].as_i64().unwrap(), v[1].as_i64().unwrap());
    let outline: Vec<(i64, i64)> = b["outline"].as_array().unwrap().iter().map(pt).collect();
    assert_eq!(outline.len(), 4, "a rectangle");
    assert_eq!(outline[0], (0, 0), "the insertion point is the top-left corner");
    let far = (pos.0 + outline[2].0, pos.1 + outline[2].1);
    let mut golden: Vec<(u16, (i64, i64))> = b["cps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let p = pt(&c["pos"]);
            (c["idx"].as_u64().unwrap() as u16, (pos.0 + p.0, pos.1 + p.1))
        })
        .collect();
    golden.sort();
    let part = part_number(device["parts"][0]["part"]["copy"].as_str().unwrap());
    // At actual size a grid step is wider than the 4 points within which a press takes a grip or a CP.
    let mut steps: Vec<(String, Value)> = vec![
        ("view.zoom_actual".into(), json!({})),
        (
            "tool.black_box".into(),
            json!({"part": part, "template": 0, "letter": device["tag"]["letter"]}),
        ),
    ];
    for corner in [pos, far] {
        let (x, y) = map.mm(corner);
        steps.push(("cursor.set".into(), json!({"x_mm": x, "y_mm": y})));
        steps.push(("canvas.activate".into(), json!({})));
    }
    steps.push(("ui.dismiss".into(), json!({})));
    // The tool's default layout (the template's CPs in index order, which is the golden template order).
    let cps: Vec<(u16, String)> = golden.iter().map(|(i, _)| (*i, i.to_string())).collect();
    let g = PageGridMap::new(grid);
    let (origin, _, laid) = layout(g, pos, far, &cps).expect("the default layout");
    let mut current: Vec<(u16, (i64, i64))> = laid
        .iter()
        .map(|(i, p, _)| (*i, (origin.0 + p.0, origin.1 + p.1)))
        .collect();
    // Select the box: a click inside it, two grid steps in from its top-left corner (no grip, no CP there).
    let inside = (g.coord(g.index(pos.0) + 2), g.coord(g.index(pos.1) + 2));
    let (x, y) = map.mm(inside);
    steps.push(("canvas.press".into(), json!({"x_mm": x, "y_mm": y})));
    steps.push(("canvas.release".into(), json!({"x_mm": x, "y_mm": y})));
    loop {
        let pending: Vec<(u16, (i64, i64))> = golden
            .iter()
            .filter(|(i, p)| current.iter().any(|(j, q)| j == i && q != p))
            .copied()
            .collect();
        let Some(&(idx, to)) = pending
            .iter()
            .find(|(i, p)| !current.iter().any(|(j, q)| j != i && q == p))
        else {
            assert!(pending.is_empty(), "the CP moves form a cycle: {pending:?}");
            break;
        };
        let from = current.iter().find(|(j, _)| *j == idx).unwrap().1;
        steps.extend(drag(map, from, to));
        for c in &mut current {
            if c.0 == idx {
                c.1 = to;
            }
        }
    }
    steps.push(("ui.dismiss".into(), json!({})));
    steps
}

// -------------------------------------------------------------------------------------------------------------------
// WP-S2.04 part 2: typed tags, the wiring and the oracle of a folio

/// `num / den` rounded to the nearest integer, ties toward zero (`ew_units::Grid`'s rule).
fn div_round(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() > den { q + num.signum() } else { q }
}

/// The grid index and coordinate maps of a grid `(num, den)`.
fn grid_maps(grid: (i64, i64)) -> (impl Fn(i64) -> i64, impl Fn(i64) -> i64) {
    let (num, den) = (i128::from(grid.0), i128::from(grid.1));
    (
        move |v: i64| div_round(i128::from(v) * den, num) as i64,
        move |k: i64| div_round(i128::from(k) * num, den) as i64,
    )
}

/// The steps that type the golden tags of folio `folio` onto the devices the placement script made in the project
/// at `placed` (its placements done): for each golden device of the folio's representations, and the strips of its
/// terminals, whose golden tag is frozen, `command.run` `fields.set` `tag.frozen` with the golden value on the device
/// that prints the placed tag (a `tag:` reference, so no ID is minted into the script). Returns the steps.
pub fn typed_tag_steps(folio: u32, placed: &Path) -> Vec<(String, Value)> {
    use ew_ui::ew_viewmodel::ew_api::Session;
    let all = records();
    let keys = symbol_keys();
    let page = all
        .values()
        .find(|r| r["$k"] == "page" && r["folio"]["number"] == folio)
        .unwrap()["id"]
        .clone();
    let (s, _) = Session::open_read_only(placed, "test").unwrap();
    let tables = s.page_tables(&folio.to_string()).unwrap();
    // The tag a placed representation's CPs print (before the colon), by (symbol key, position).
    let printed = |symbol: &str, pos: (i64, i64)| -> Option<String> {
        let rep = tables
            .reps
            .iter()
            .find(|r| r.symbol_key == symbol && r.pos == pos)?;
        tables
            .cps
            .iter()
            .find(|c| c.rep == rep.id)
            .and_then(|c| c.label.split_once(':').map(|(t, _)| t.to_owned()))
    };
    let mut reps: Vec<&Value> = all
        .values()
        .filter(|r| r["$k"] == "representation" && r["page"] == page)
        .collect();
    reps.sort_by_key(|r| r["source_ref"]["key"].as_str().unwrap_or_default().to_owned());
    let mut done: Vec<String> = Vec::new();
    let mut steps = Vec::new();
    for r in reps {
        let function = &all[r["functions"][0].as_str().unwrap()];
        let Some(device_id) = function["device"].as_str() else {
            continue;
        };
        let device = &all[device_id];
        let symbol = keys[r["symbol"].as_str().unwrap()].clone();
        let pos = (r["pos"][0].as_i64().unwrap(), r["pos"][1].as_i64().unwrap());
        let Some(tag) = printed(&symbol, pos) else {
            continue;
        };
        // A terminal prints its strip's tag: the strip is the device to type.
        let (golden, id) = match device.get("terminal") {
            Some(t) => {
                let strip = t["strip"].as_str().unwrap();
                (&all[strip], strip.to_owned())
            }
            None => (device, device_id.to_owned()),
        };
        if done.contains(&id) {
            continue;
        }
        done.push(id);
        if let Some(frozen) = golden["tag"].get("frozen") {
            steps.push((
                "command.run".to_owned(),
                json!({"cmd": "fields.set", "params": {
                    "kind": "device",
                    "targets": [{"ref": format!("tag:{tag}")}],
                    "path": "tag.frozen",
                    "value": frozen,
                }}),
            ));
        }
    }
    steps
}

/// The golden tables of folio `folio`.
fn golden_tables(folio: u32) -> ew_ui::ew_viewmodel::ew_api::output::PageTables {
    let (s, _) = ew_ui::ew_viewmodel::ew_api::Session::open_read_only(Path::new(GOLDEN_A), "test").unwrap();
    s.page_tables(&folio.to_string()).unwrap()
}

/// The steps that draw every connection of folio `folio` with the Wire tool from the keyboard (the grid cursor on
/// each end, Enter), at actual size so that a grid point one step from a connection point is a target of its own.
/// A connection between two connection points is drawn as it is. A junction is drawn so that its sequence comes out
/// as the golden one: a wire from the far end of its first member to the far end of its second, then each further
/// member from its far end onto that wire at a split point along a CP's leaving direction (the junction is created
/// there and the wire split, `connection.draw`'s `on_connection`). For this golden rehearsal layout, folio 102's
/// common downward G1 supply bank uses the second end, starting one step away and advancing until its transverse
/// grid row (Up/Down) or column (Left/Right) differs from earlier receiving splits. These lanes avoid the shared
/// departure and return corridors. All other junctions use the first end, one step away, preserving the upward
/// F2-to-F1 junction's marker-crossing geometry. This is a golden-layout rule, not a general routing guarantee.
/// The junctions are drawn before the other connections. Returns the steps, the number of Wire tool commits,
/// and the point at which each golden junction (table order) is created.
// The tuple of §13.6: the steps, the commits and the junctions' split points.
#[allow(clippy::type_complexity)]
pub fn wiring_steps(folio: u32, map: SheetMap) -> (Vec<(String, Value)>, usize, Vec<(i64, i64)>) {
    wiring_steps_with(folio, map, None)
}

/// [`wiring_steps`] with an optional end-point map: `Some(f)` places every connection end at `f` of its golden
/// label instead of the golden point (the folio-103 demo variant routes to the terminals' own CPs); `None` keeps
/// [`wiring_steps`]'s behaviour.
#[allow(clippy::type_complexity)]
pub fn wiring_steps_with(
    folio: u32,
    map: SheetMap,
    cp_at: Option<&dyn Fn(&str) -> Option<(i64, i64)>>,
) -> (Vec<(String, Value)>, usize, Vec<(i64, i64)>) {
    use ew_ui::ew_viewmodel::ew_api::geometry::Dir;
    use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
    let t = golden_tables(folio);
    let (index, coord) = grid_maps(t.grid);
    let cp_of = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => t.cps.iter().find(|c| c.function == *function && c.idx == *idx),
        TableEnd::Junction { .. } => None,
    };
    let at = |cp: &ew_ui::ew_viewmodel::ew_api::output::TableCp| -> (i64, i64) {
        match cp_at {
            Some(f) => f(&cp.label).unwrap_or_else(|| panic!("no end point for the CP {}", cp.label)),
            None => cp.at,
        }
    };
    let mut steps: Vec<(String, Value)> = vec![
        ("view.zoom_actual".to_owned(), json!({})),
        ("tool.wire".to_owned(), json!({})),
    ];
    let mut commits = 0;
    let mut points: Vec<(i64, i64)> = Vec::new();
    let mut split_lanes = BTreeSet::new();
    let enter = |steps: &mut Vec<(String, Value)>, p: (i64, i64)| {
        let (x, y) = map.mm(p);
        steps.push(("cursor.set".to_owned(), json!({"x_mm": x, "y_mm": y})));
        steps.push(("canvas.activate".to_owned(), json!({})));
    };
    // The junctions first, with an isolated split point on each first wire.
    for j in &t.junctions {
        let far: Vec<_> = j
            .sequence
            .iter()
            .filter_map(|id| t.connections.iter().find(|c| c.id == *id))
            .filter_map(|c| match (c.a, c.b) {
                (TableEnd::Junction { id }, other) if id == j.id => cp_of(&other),
                (other, _) => cp_of(&other),
            })
            .collect();
        assert_eq!(
            far.len(),
            j.sequence.len(),
            "every member of a golden junction ends at a CP"
        );
        let (f0, f1) = (far[0], far[1]);
        enter(&mut steps, at(f0));
        enter(&mut steps, at(f1));
        commits += 1;
        // Distinct receiving lanes isolate the common downward supply bank from
        // earlier wires. Other junctions retain their original marker geometry.
        let use_receiving = folio == 102 && f0.dir == Some(Dir::Down);
        let split_cp = if use_receiving { f1 } else { f0 };
        let split_at = at(split_cp);
        let mut distance = 1;
        let stub = loop {
            let (point, lane) = match split_cp.dir {
                Some(Dir::Down) => {
                    let y = coord(index(split_at.1) + distance);
                    ((split_at.0, y), (0, y))
                }
                Some(Dir::Up) => {
                    let y = coord(index(split_at.1) - distance);
                    ((split_at.0, y), (0, y))
                }
                Some(Dir::Right) => {
                    let x = coord(index(split_at.0) + distance);
                    ((x, split_at.1), (1, x))
                }
                Some(Dir::Left) => {
                    let x = coord(index(split_at.0) - distance);
                    ((x, split_at.1), (1, x))
                }
                None => panic!("a split member without a direction"),
            };
            if !use_receiving || split_lanes.insert(lane) {
                break point;
            }
            distance += 1;
        };
        points.push(stub);
        for f in &far[2..] {
            enter(&mut steps, at(f));
            enter(&mut steps, stub);
            commits += 1;
        }
    }
    let members: Vec<_> = t
        .junctions
        .iter()
        .flat_map(|j| j.sequence.iter().copied())
        .collect();
    for c in &t.connections {
        if members.contains(&c.id) {
            continue;
        }
        let (Some(a), Some(b)) = (cp_of(&c.a), cp_of(&c.b)) else {
            continue;
        };
        enter(&mut steps, at(a));
        enter(&mut steps, at(b));
        commits += 1;
    }
    steps.push(("ui.dismiss".to_owned(), json!({})));
    (steps, commits, points)
}

/// What the oracle of §13.6 compares on a folio's tables: the connections between two connection points as sorted
/// label pairs, the junctions' sequences named by their far ends' labels, both sorted; and the counts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WiringOracle {
    /// The sorted label pairs of the CP-to-CP connections.
    pub pairs: Vec<(String, String)>,
    /// The junctions' sequences by far-end labels.
    pub sequences: Vec<Vec<String>>,
    /// Drawn connections and junctions.
    pub counts: (usize, usize),
}

/// The oracle values of `tables`.
pub fn wiring_oracle(t: &ew_ui::ew_viewmodel::ew_api::output::PageTables) -> WiringOracle {
    use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
    let label = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.label.clone()),
        TableEnd::Junction { .. } => None,
    };
    let mut pairs: Vec<(String, String)> = t
        .connections
        .iter()
        .filter_map(|c| {
            let (a, b) = (label(&c.a)?, label(&c.b)?);
            Some(if a <= b { (a, b) } else { (b, a) })
        })
        .collect();
    pairs.sort();
    let mut sequences: Vec<Vec<String>> = t
        .junctions
        .iter()
        .map(|j| {
            j.sequence
                .iter()
                .filter_map(|id| t.connections.iter().find(|c| c.id == *id))
                .map(|c| match (&c.a, &c.b) {
                    (TableEnd::Junction { id }, other) if *id == j.id => label(other).unwrap_or_default(),
                    (other, _) => label(other).unwrap_or_default(),
                })
                .collect()
        })
        .collect();
    sequences.sort();
    WiringOracle {
        pairs,
        sequences,
        counts: (t.connections.len(), t.junctions.len()),
    }
}

/// The oracle values of Golden A's folio `folio`.
pub fn golden_wiring(folio: u32) -> WiringOracle {
    wiring_oracle(&golden_tables(folio))
}

// -------------------------------------------------------------------------------------------------------------------
// The script runs and the project start (WP-S2.04 §13.6): the temporary directory, the headless runs, the wizard
// and the start project the folio tests and the G-PP rehearsal share.

/// The content tree of the source repository (the starter library of `file.new` and the wizard).
pub const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
pub struct Tmp(PathBuf);

impl Tmp {
    /// A new directory under the system temporary directory.
    pub fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-place-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }

    /// A path inside the directory.
    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// The directory.
    pub fn dir(&self) -> &Path {
        &self.0
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Copies the directory tree `from` to `to`.
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A path as the text a script line carries.
pub fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The text of a UI script of `steps`.
pub fn script_text(steps: &[(String, Value)]) -> String {
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\",\"label\":\"folio script\"}\n");
    for (action, params) in steps {
        text.push_str(&json!({"action": action, "params": params}).to_string());
        text.push('\n');
    }
    text
}

/// Runs a script headless through the binary at `exe` (with the content of the source tree), optionally recording
/// the committed commands.
pub fn run(exe: &Path, tmp: &Tmp, name: &str, steps: &[(String, Value)], record: Option<&Path>) -> Output {
    let file = tmp.path(name);
    std::fs::write(&file, script_text(steps)).unwrap();
    let mut cmd = Command::new(exe);
    cmd.args(["--headless", "--script"]).arg(&file);
    if let Some(r) = record {
        cmd.arg("--record").arg(r);
    }
    // The start's runs share one configuration (the wizard's company standard is the default template of `file.new`);
    // a folio run has its own, so that its recent-projects list depends on nothing before it.
    let config = if record.is_some() {
        tmp.path(&format!("config-{name}"))
    } else {
        tmp.path("config")
    };
    cmd.env("EW_CONFIG_DIR", config)
        .env("EW_CONTENT_DIR", CONTENT)
        .env_remove("EW_SAMPLE_DIR")
        .env_remove("EW_DOCS_DIR")
        .env_remove("SOURCE_DATE_EPOCH")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Every step of a transcript was done.
#[track_caller]
pub fn all_done(out: &Output) {
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}\n{text}",
        String::from_utf8_lossy(&out.stderr)
    );
    for line in text.lines().skip(1) {
        let step: Value = serde_json::from_str(line).unwrap();
        assert_eq!(step["outcome"], "done", "{step}");
    }
}

/// Script steps from `(action, params)` pairs.
pub fn steps(list: &[(&str, Value)]) -> Vec<(String, Value)> {
    list.iter().map(|(a, p)| ((*a).to_owned(), p.clone())).collect()
}

/// The `library.embed` parameters of starter documents of the content tree.
pub fn embed(files: &[&str]) -> Value {
    let copies: Vec<Value> = files
        .iter()
        .map(|f| serde_json::from_str(&std::fs::read_to_string(Path::new(CONTENT).join(f)).unwrap()).unwrap())
        .collect();
    json!({ "copies": copies })
}

/// The `library.embed` parameters of copies of the golden fixture's `lib/` directory.
pub fn embed_golden(files: &[&str]) -> Value {
    let copies: Vec<Value> = files
        .iter()
        .map(|f| {
            let p = Path::new(GOLDEN_A).join("lib").join(f);
            serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
        })
        .collect();
    json!({ "copies": copies })
}

/// The start of §13.6 for folios 101 and 102: a new project made by the company-standard wizard with its defaults,
/// pages 101 and 102 (multi-line, the QET grid), strip `X` for the terminals, the interruption-point symbols, the
/// golden part copies of 102's black boxes, and (part 3) the structure node +J-E-206 with Golden A's external
/// references (the Place tool embeds the symbols it places itself). Saved and closed.
pub fn start(exe: &Path, tmp: &Tmp) -> PathBuf {
    let company = tmp.path("company.ew");
    all_done(&run(
        exe,
        tmp,
        "wizard.jsonl",
        &steps(&[
            ("tools.company_standard", json!({})),
            ("wizard.finish", json!({ "dir": path(&company) })),
        ]),
        None,
    ));
    let job = tmp.path("start.ew");
    all_done(&run(
        exe,
        tmp,
        "start.jsonl",
        &steps(&[
            ("file.new", json!({ "dir": path(&job) })),
            (
                "command.run",
                json!({"cmd": "page.create", "params": {
                    "number": 101, "title": "Power Supply 1 - Main Current", "kind": "multi_line",
                    "grid": {"num": 7_937_500, "den": 3},
                }}),
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
            // WP-S2.04 part 3: page 102 and the golden part copies of its black boxes (-102G1, -102F2) with the
            // black-box symbol and FnDef copies their templates name.
            (
                "command.run",
                json!({"cmd": "page.create", "params": {
                    "number": 102, "title": "Power Supply 2 - Control Current", "kind": "multi_line",
                    "grid": {"num": 7_937_500, "den": 3},
                }}),
            ),
            (
                "command.run",
                json!({"cmd": "library.embed", "params": embed_golden(&[
                    "parts/68075c2c-1eed-5753-8749-88e37b4496d5.json",
                    "parts/6b848305-0ec3-5ef1-9d84-7277eb168d2a.json",
                    "fndefs/3d012b01-31ef-5047-a43d-2b74f9049911.json",
                    "symbols/faf09b45-ec6e-57d8-8be6-faf4c1596ddb.json",
                ])}),
            ),
            ("file.save", json!({})),
            ("file.close", json!({})),
        ]),
        None,
    ));
    // WP-S2.04 part 3 (§13.6's start): the structure node +J-E-206 and Golden A's external references, created in
    // the session (the node's minted ID is read back here, so no script names it); the folio scripts name the
    // references by their printed text.
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state-start"));
    let (mut s, _) = Session::open(
        &job,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    let run_cmd = |s: &mut Session, name: &str, params: Value| {
        let cmd = ew_ui::ew_viewmodel::ew_api::AnyCommand {
            name: name.into(),
            params,
        };
        s.execute(&cmd).unwrap_or_else(|e| panic!("{name}: {e:?}"))
    };
    let receipt = run_cmd(
        &mut s,
        "structure.create_node",
        json!({"aspect": "location", "code": "J-E-206"}),
    );
    let node = serde_json::to_value(receipt.outcome.primary.expect("the node").1).unwrap();
    for x in external_refs() {
        run_cmd(
            &mut s,
            "external_ref.create",
            json!({"location": node, "device": x["device"], "connection": x["connection"]}),
        );
    }
    s.save().unwrap();
    s.close();
    job
}

/// The representations of the project at `dir`: (symbol key, insertion point, orientation), sorted.
pub fn reps_of(dir: &Path) -> Vec<(String, (i64, i64), u8)> {
    let (s, _) = Session::open_read_only(dir, "test").unwrap();
    let mut out: Vec<_> = s
        .page_tables("101")
        .unwrap()
        .reps
        .iter()
        .map(|r| (r.symbol_key.clone(), r.pos, r.orient))
        .collect();
    out.sort();
    out
}

/// The devices and functions of a session as canonical JSON with their IDs left out (§12 item 2: equal records,
/// IDs aside): a device's record without its `id`; a function's without its `id` and with its device replaced by
/// that device's canonical text. Sorted.
pub fn records_of(s: &Session) -> (Vec<String>, Vec<String>) {
    let q = s.query();
    let mut device_text = std::collections::BTreeMap::new();
    let mut devices = Vec::new();
    for (id, d) in q.devices().iter() {
        let mut v = serde_json::to_value(&**d).unwrap();
        v.as_object_mut().unwrap().remove("id");
        let text = v.to_string();
        device_text.insert(id.to_string(), text.clone());
        devices.push(text);
    }
    let mut functions = Vec::new();
    for (_, f) in q.functions().iter() {
        let mut v = serde_json::to_value(&**f).unwrap();
        let o = v.as_object_mut().unwrap();
        o.remove("id");
        if let Some(Value::String(d)) = o.get("device").cloned() {
            o.insert("device".into(), Value::String(device_text[&d].clone()));
        }
        functions.push(v.to_string());
    }
    devices.sort();
    functions.sort();
    (devices, functions)
}

// -------------------------------------------------------------------------------------------------------------------
// The G-PP start (WP-S2.04 §13.6): the three folios, the strips -X1031/-X1032 with their terminals, the parts of
// the 101 terminals and the golden function IDs the strip segments are placed on.

/// The start of the G-PP rehearsal (§13.6): the saved project, the 1X1 strip the 101 terminals are placed on, and
/// the golden function IDs mapped to the start project's.
pub struct GppStart {
    /// The saved start project.
    pub dir: PathBuf,
    /// The 1X1 strip's device ID (the strip the folio-101 terminals are placed on).
    pub strip_1x1: String,
    /// Golden function ID → the start project's function ID, for the main functions of -X1031 and -X1032 and every
    /// terminal function of their 25 terminals (keys are Golden A's function UUID texts).
    pub functions: BTreeMap<String, String>,
}

/// The sorted file names below `golden/J-E-2416.ew/lib/<dir>`, each as `<dir>/<name>`.
fn golden_lib_files(dir: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(Path::new(GOLDEN_A).join("lib").join(dir))
        .unwrap()
        .map(|e| format!("{dir}/{}", e.unwrap().file_name().to_string_lossy()))
        .collect();
    out.sort();
    out
}

/// The IDs of the golden fixture's `lib/` documents.
fn golden_lib_ids() -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for dir in ["fndefs", "symbols", "parts", "forms"] {
        for file in golden_lib_files(dir) {
            let doc: Value = serde_json::from_str(
                &std::fs::read_to_string(Path::new(GOLDEN_A).join("lib").join(&file)).unwrap(),
            )
            .unwrap();
            out.insert(doc["id"].as_str().unwrap().to_owned());
        }
    }
    out
}

/// The `anchor` point of the fixture's symbol copy `id` (`pos + orient.apply(anchor)` is the representation's
/// anchor, `ew-derive`'s `anchor_point`).
fn symbol_anchor(id: &str) -> (i64, i64) {
    let path = Path::new(GOLDEN_A).join("lib/symbols").join(format!("{id}.json"));
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap();
    match &doc["anchor"] {
        Value::Null => (0, 0),
        Value::String(s) if s == "origin" => (0, 0),
        Value::Object(m) => match m.get("named") {
            Some(p) => (p[0].as_i64().unwrap(), p[1].as_i64().unwrap()),
            None => panic!("the fixture's symbol {id} has an anchor policy the generator does not read"),
        },
        other => panic!("the fixture's symbol {id} has the anchor policy {other}"),
    }
}

/// The anchor points of folio `folio`'s external connection points (the interruption points with an `external`
/// reference), in representation key order: the insertion point plus the symbol's anchor (Q7 of the research).
pub fn external_connection_anchors(folio: u32) -> Vec<(i64, i64)> {
    let all = records();
    let page = all
        .values()
        .find(|r| r["$k"] == "page" && r["folio"]["number"] == folio)
        .unwrap_or_else(|| panic!("no page {folio}"))["id"]
        .clone();
    let mut reps: Vec<&Value> = all
        .values()
        .filter(|r| r["$k"] == "representation" && r["page"] == page)
        .collect();
    reps.sort_by_key(|r| r["source_ref"]["key"].as_str().unwrap_or_default().to_owned());
    let mut out = Vec::new();
    for r in reps {
        let function = &all[r["functions"][0].as_str().unwrap()];
        let Some(ip) = function["kind"].get("ip") else {
            continue;
        };
        if ip.get("external").and_then(Value::as_str).is_none() {
            continue;
        }
        assert_eq!(
            orient_of(&r["orient"]),
            0,
            "the external connection points are placed at orientation A"
        );
        let anchor = symbol_anchor(r["symbol"].as_str().unwrap());
        let pos = (r["pos"][0].as_i64().unwrap(), r["pos"][1].as_i64().unwrap());
        out.push((pos.0 + anchor.0, pos.1 + anchor.1));
    }
    out
}

/// The main function (the default role) of the golden device record `device`.
fn golden_main_function(all: &BTreeMap<String, Value>, device: &Value) -> Option<Value> {
    all.values()
        .find(|r| {
            r["$k"] == "function"
                && r["device"] == device["id"]
                && r["role"].as_str().unwrap_or("main") == "main"
        })
        .cloned()
}

/// The golden device record of `key` (`devices/<key>` in the fixture's `source_ref`).
fn golden_device(all: &BTreeMap<String, Value>, key: &str) -> Value {
    all.values()
        .find(|r| r["$k"] == "device" && r["source_ref"]["key"] == format!("devices|{key}"))
        .unwrap_or_else(|| panic!("no golden device {key}"))
        .clone()
}

/// `gpp_start` (§13.6): the wizard's new project, pages 101, 102 and 103, the golden library copies, the structure
/// node +J-E-206 with Golden A's external references, and the three strips with the 25 terminals of -X1031 and
/// -X1032 and their parts. Saved and closed; asserts the pages and the terminals and returns the start.
pub fn gpp_start(exe: &Path, tmp: &Tmp) -> GppStart {
    gpp_start_common(exe, tmp, false)
}

/// The folio-103 demo variant's start (§13.6): exactly [`gpp_start`] except that the strips -X1031 and -X1032 and
/// their 25 terminals are not created (the 1X1 strip, the pages, the library, the node and the external references
/// are the same). [`GppStart::functions`] is empty.
pub fn gpp_start_demo(exe: &Path, tmp: &Tmp) -> GppStart {
    gpp_start_common(exe, tmp, true)
}

/// The shared part of [`gpp_start`] and [`gpp_start_demo`].
fn gpp_start_common(exe: &Path, tmp: &Tmp, demo: bool) -> GppStart {
    let all = records();
    let company = tmp.path("company.ew");
    all_done(&run(
        exe,
        tmp,
        "gpp-wizard.jsonl",
        &steps(&[
            ("tools.company_standard", json!({})),
            ("wizard.finish", json!({ "dir": path(&company) })),
        ]),
        None,
    ));
    let job = tmp.path(if demo { "gpp-demo.ew" } else { "gpp.ew" });
    let mut script = steps(&[("file.new", json!({ "dir": path(&job) }))]);
    for (number, title) in [
        (101, "Power Supply 1 - Main Current"),
        (102, "Power Supply 2 - Control Current"),
        (103, "Power Supply 3 - Control Current Distribution"),
    ] {
        script.push((
            "command.run".into(),
            json!({"cmd": "page.create", "params": {
                "number": number, "title": title, "kind": "multi_line",
                "grid": {"num": 7_937_500, "den": 3},
            }}),
        ));
    }
    // Every golden library document of the three kinds, files in sorted order; the forms come from the wizard.
    for dir in ["fndefs", "symbols", "parts"] {
        let files = golden_lib_files(dir);
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        script.push((
            "command.run".into(),
            json!({"cmd": "library.embed", "params": embed_golden(&refs)}),
        ));
    }
    // The IP and external-arrow starter documents of the content tree, only for those the golden copies do not
    // embed already (the same IDs: the fixture was made from the same content).
    let embedded = golden_lib_ids();
    let starters = [
        "symbols/qet10/ew.gen.ip@qet10.json",
        "fndefs/ew.fn.ip.json",
        "symbols/qet10/ew.gen.external-arrow@qet10.json",
        "fndefs/ew.fn.external-arrow.json",
    ];
    let missing: Vec<&str> = starters
        .iter()
        .copied()
        .filter(|f| {
            let doc: Value =
                serde_json::from_str(&std::fs::read_to_string(Path::new(CONTENT).join(f)).unwrap()).unwrap();
            !embedded.contains(doc["id"].as_str().unwrap())
        })
        .collect();
    if !missing.is_empty() {
        script.push((
            "command.run".into(),
            json!({"cmd": "library.embed", "params": embed(&missing)}),
        ));
    }
    script.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    all_done(&run(exe, tmp, "gpp-start.jsonl", &script, None));

    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state-gpp"));
    let (mut s, _) = Session::open(
        &job,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    let execute = |s: &mut Session, name: &str, params: Value| -> Value {
        let cmd = AnyCommand {
            name: name.into(),
            params,
        };
        let receipt = s.execute(&cmd).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        receipt
            .outcome
            .primary
            .map(|p| serde_json::to_value(p.1).unwrap())
            .unwrap_or(Value::Null)
    };
    // The structure node +J-E-206 and Golden A's external references, as `start` creates them.
    let node = execute(
        &mut s,
        "structure.create_node",
        json!({"aspect": "location", "code": "J-E-206"}),
    );
    for x in external_refs() {
        execute(
            &mut s,
            "external_ref.create",
            json!({"location": node, "device": x["device"], "connection": x["connection"]}),
        );
    }
    // The strips: all three of `gpp_start`; the demo variant holds only the 1X1 strip (its -X1031 and -X1032 are
    // created by `strip.new` on folio 103 and their terminals by Place).
    let mut strips: BTreeMap<String, String> = BTreeMap::new();
    let mut strip_main_fns: BTreeMap<String, String> = BTreeMap::new();
    let strip_keys: &[&str] = if demo {
        &["1X1"]
    } else {
        &["1X1", "-X1031", "-X1032"]
    };
    for &key in strip_keys {
        let golden = golden_device(&all, key);
        let mut params = serde_json::Map::new();
        params.insert("letter".into(), golden["tag"]["letter"].clone());
        if let Some(counter) = golden["tag"].get("counter") {
            params.insert("counter".into(), counter.clone());
        }
        if let Some(kind) = golden.get("kind") {
            params.insert("kind".into(), kind.clone());
        }
        if let Some(parts) = golden.get("parts") {
            params.insert("parts".into(), parts.clone());
        }
        if let Some(text) = golden.get("function_text") {
            params.insert("function_text".into(), text.clone());
        }
        if let Some(strip) = golden.get("strip") {
            params.insert("strip".into(), strip.clone());
        }
        if let Some(main) = golden_main_function(&all, &golden) {
            params.insert("main_fndef".into(), main["def"].clone());
            // Only the two strips of folio 103 are in the function map the folio scripts read; the 1X1 strip's
            // main function is not (E3 of the review of round a28-r5).
            if matches!(key, "-X1031" | "-X1032") {
                strip_main_fns.insert(key.to_owned(), main["id"].as_str().unwrap().to_owned());
            }
        }
        let id = execute(&mut s, "device.create", Value::Object(params));
        strips.insert(key.to_owned(), id.as_str().unwrap().to_owned());
    }
    let mut functions: BTreeMap<String, String> = BTreeMap::new();
    if !demo {
        // The terminals of -X1031 and -X1032 in `terminal.seq` order, each strip's one FnDef.
        let mut terminals: Vec<Value> = Vec::new();
        for key in ["-X1031", "-X1032"] {
            let strip = golden_device(&all, key);
            let mut devices: Vec<Value> = all
                .values()
                .filter(|r| r["$k"] == "device" && r["terminal"]["strip"] == strip["id"])
                .cloned()
                .collect();
            devices.sort_by_key(|d| d["terminal"]["seq"].as_u64().unwrap_or(0));
            let mut fndefs: Vec<String> = Vec::new();
            let mut specs: Vec<Value> = Vec::new();
            for d in devices {
                let main = golden_main_function(&all, &d).expect("a terminal function");
                let def = main["def"].as_str().unwrap().to_owned();
                if !fndefs.contains(&def) {
                    fndefs.push(def);
                }
                let mut spec = serde_json::Map::new();
                spec.insert("designation".into(), d["terminal"]["designation"].clone());
                if let Some(seq) = d["terminal"].get("seq") {
                    spec.insert("seq".into(), seq.clone());
                }
                if let Some(text) = d.get("function_text").filter(|t| !t.is_null()) {
                    spec.insert("function_text".into(), text.clone());
                }
                if d["terminal"]["rail_bonded"].as_bool() == Some(true) {
                    spec.insert("rail_bonded".into(), Value::Bool(true));
                }
                if let Some(levels) = d["terminal"]
                    .get("levels")
                    .filter(|l| l.as_array().is_some_and(|a| !a.is_empty()))
                {
                    spec.insert("levels".into(), levels.clone());
                }
                specs.push(Value::Object(spec));
                terminals.push(d.clone());
            }
            assert_eq!(
                fndefs.len(),
                1,
                "the terminals of {key} use {} function definitions; STOP: the generator places one FnDef per strip",
                fndefs.len()
            );
            execute(
                &mut s,
                "terminal.add",
                json!({"strip": strips[key].clone(), "fndef": fndefs[0].clone(), "terminals": specs}),
            );
        }
        // The new terminal device of each golden terminal: by its strip and designation.
        let golden_strip_ids: BTreeMap<String, String> = ["-X1031", "-X1032"]
            .into_iter()
            .map(|k| {
                (
                    golden_device(&all, k)["id"].as_str().unwrap().to_owned(),
                    strips[k].clone(),
                )
            })
            .collect();
        let pairs: Vec<(String, String, String)> = {
            let q = s.query();
            let mut out = Vec::new();
            for golden in &terminals {
                let strip = golden["terminal"]["strip"].as_str().unwrap();
                let new_strip = golden_strip_ids
                    .get(strip)
                    .unwrap_or_else(|| panic!("a terminal of a created strip: {strip}"));
                let key = ["-X1031", "-X1032"]
                    .into_iter()
                    .find(|k| strips[*k] == *new_strip)
                    .expect("a terminal of a created strip");
                let designation = golden["terminal"]["designation"].as_str().unwrap();
                let found = q.devices().iter().find_map(|(id, d)| {
                    let t = d.aspects.terminal.as_ref()?;
                    (t.strip.to_string() == *new_strip && t.designation.as_str() == designation)
                        .then(|| id.to_string())
                });
                out.push((
                    golden["id"].as_str().unwrap().to_owned(),
                    key.to_owned(),
                    found.unwrap_or_else(|| panic!("no new terminal {key}:{designation}")),
                ));
            }
            out
        };
        // The golden part uses of each terminal, `fields.set` on the new terminal device.
        for (golden_id, _, new_id) in &pairs {
            let golden = &all[golden_id];
            execute(
                &mut s,
                "fields.set",
                json!({"kind": "device", "targets": [new_id], "path": "parts", "value": golden["parts"]}),
            );
        }
        // The function map: the strips' main functions and every terminal function.
        {
            let q = s.query();
            let find = |device: &str| -> String {
                q.functions()
                    .iter()
                    .find_map(|(id, f)| {
                        let v = serde_json::to_value(&**f).unwrap();
                        (v["device"].as_str() == Some(device)
                            && v["role"].as_str().unwrap_or("main") == "main")
                            .then(|| id.to_string())
                    })
                    .unwrap_or_else(|| panic!("no main function of device {device}"))
            };
            for (key, golden_fn) in &strip_main_fns {
                functions.insert(golden_fn.clone(), find(&strips[key]));
            }
            for (golden_id, _, new_id) in &pairs {
                let golden = &all[golden_id];
                let main = golden_main_function(&all, golden).expect("a terminal function");
                functions.insert(main["id"].as_str().unwrap().to_owned(), find(new_id));
            }
        }
    }
    s.save().unwrap();
    s.close();
    // The start's assertion: the three pages, and the 25 terminals on the two strips, each with one part use.
    let (s, _) = Session::open_read_only(&job, "test").unwrap();
    for folio in ["101", "102", "103"] {
        assert!(
            s.page_tables(folio).is_some(),
            "the start project has page {folio}"
        );
    }
    if demo {
        assert_eq!(strips.len(), 1, "the demo start creates only the 1X1 strip");
        assert!(functions.is_empty(), "the demo start's function map is empty");
    } else {
        let q = s.query();
        let created: Vec<_> = q
            .devices()
            .iter()
            .filter(|(_, d)| {
                d.aspects.terminal.as_ref().is_some_and(|t| {
                    let strip = t.strip.to_string();
                    strip == strips["-X1031"] || strip == strips["-X1032"]
                })
            })
            .collect();
        assert_eq!(created.len(), 25, "25 terminal devices on -X1031 and -X1032");
        for (_, d) in created {
            assert_eq!(
                d.parts.len(),
                1,
                "each terminal of -X1031 and -X1032 has exactly one part use"
            );
        }
        drop(q);
    }
    drop(s);
    GppStart {
        dir: job,
        strip_1x1: strips["1X1"].clone(),
        functions,
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The folio-103 demo variant (WP-S2.04 §13.6, round a28-r8): the strips by `strip.new` with their golden values
// member by member, the 15 terminals by Place, their parts, the wiring to the terminals' own CPs, PI10's bridges and
// the other golden bridges with the strips' plug-in bridge part uses.

/// The 15 terminals of the demo variant that folio 103's drawn connections end at, in placement order.
pub const DEMO_X1031_TERMINALS: [&str; 11] = ["1", "2", "3", "4", "5", "7", "9", "11", "12", "13", "14"];
/// The -X1032 terminals of the demo variant.
pub const DEMO_X1032_TERMINALS: [&str; 4] = ["1", "2", "3", "4"];

/// The demo variant's two strips, by golden key.
pub struct DemoStrips {
    /// The -X1031 strip device.
    pub x1031: String,
    /// The -X1032 strip device.
    pub x1032: String,
    /// The golden plug-in bridge part use of each strip, by golden strip key (its quantity is set in B6).
    pub bridge_parts: BTreeMap<String, Value>,
    /// The Session-executed records, by command name.
    pub executed: BTreeMap<String, usize>,
}

/// The golden part copy `copy` as its document.
fn golden_part(copy: &str) -> Value {
    let path = Path::new(GOLDEN_A).join("lib/parts").join(format!("{copy}.json"));
    serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap()
}

/// Whether a golden part use is the plug-in bridge (by its part's description).
fn part_use_is_bridge(use_: &Value) -> bool {
    use_["part"]["copy"]
        .as_str()
        .is_some_and(|copy| golden_part(copy)["description"] == "Plug-in bridge")
}

/// The golden symbol key and copy ID of the representation `key` on folio `folio`.
fn golden_rep_symbol(folio: u32, key: &str) -> (String, String) {
    let all = records();
    let keys = symbol_keys();
    let page = all
        .values()
        .find(|r| r["$k"] == "page" && r["folio"]["number"] == folio)
        .unwrap_or_else(|| panic!("no page {folio}"))["id"]
        .clone();
    let rep = all
        .values()
        .find(|r| {
            r["$k"] == "representation"
                && r["page"] == page
                && r["source_ref"]["key"] == format!("representations|{key}")
        })
        .unwrap_or_else(|| panic!("no golden representation {key} on {folio}"));
    let copy = rep["symbol"].as_str().unwrap().to_owned();
    (keys[&copy].clone(), copy)
}

/// The golden connection record of `key` (e.g. `connections|BR1.1`).
fn golden_connection(key: &str) -> Value {
    records()
        .values()
        .find(|r| r["$k"] == "connection" && r["source_ref"]["key"] == key)
        .unwrap_or_else(|| panic!("no golden connection {key}"))
        .clone()
}

/// The bridge shaft's CP index on a terminal (the golden BR1.1 endpoint).
fn golden_bridge_shaft() -> u16 {
    u16::try_from(
        golden_connection("connections|BR1.1")["a"]["cp"][1]
            .as_u64()
            .unwrap(),
    )
    .unwrap()
}

/// A `fields.set` of one device path.
fn demo_fields(device: &str, path: &str, value: Value) -> AnyCommand {
    AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "device", "targets": [device], "path": path, "value": value}),
    }
}

/// B1: the two strips of the folio-103 demo variant by `strip.new`, then their golden values member by member
/// (`tag.<member>`, `strip.<member>`) and their parts without the plug-in bridge part use. A golden member whose
/// subpath the command refuses stops the lane: it is never skipped.
pub fn demo_103_strips(exe: &Path, tmp: &Tmp, job: &Path, strip_1x1: &str) -> DemoStrips {
    let all = records();
    let mut out = DemoStrips {
        x1031: String::new(),
        x1032: String::new(),
        bridge_parts: BTreeMap::new(),
        executed: BTreeMap::new(),
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for key in ["-X1031", "-X1032"] {
        let name = key.trim_start_matches('-');
        all_done(&run(
            exe,
            tmp,
            &format!("gpp-demo-strip-{name}.jsonl"),
            &steps(&[
                ("file.open", json!({"dir": path(job)})),
                ("page.goto", json!({"folio": "103"})),
                ("strip.new", json!({"letter": "X"})),
                ("file.save", json!({})),
                ("file.close", json!({})),
            ]),
            None,
        ));
        let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
        store.state_dir = Some(tmp.path(&format!("state-gpp-demo-strip-{name}")));
        let (mut s, _) = Session::open(
            job,
            store,
            ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
            ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
        )
        .unwrap();
        let new: Vec<String> = {
            let q = s.query();
            q.devices()
                .iter()
                .filter(|(id, d)| {
                    d.aspects.strip.is_some()
                        && id.to_string() != strip_1x1
                        && !seen.contains(&id.to_string())
                })
                .map(|(id, _)| id.to_string())
                .collect()
        };
        assert_eq!(new.len(), 1, "one new strip for {key}");
        let strip = new[0].clone();
        seen.insert(strip.clone());
        let golden = golden_device(&all, key);
        let mut fields: Vec<(String, Value)> = Vec::new();
        if let Some(tag) = golden["tag"].as_object() {
            for (member, value) in tag {
                fields.push((format!("tag.{member}"), value.clone()));
            }
        }
        if let Some(aspect) = golden["strip"].as_object() {
            for (member, value) in aspect {
                fields.push((format!("strip.{member}"), value.clone()));
            }
        }
        let mut parts = golden["parts"].as_array().cloned().unwrap_or_default();
        let bridge = parts
            .iter()
            .position(part_use_is_bridge)
            .unwrap_or_else(|| panic!("no plug-in bridge part use on golden {key}"));
        let bridge_use = parts.remove(bridge);
        assert!(
            !parts.iter().any(part_use_is_bridge),
            "one plug-in bridge part use on golden {key}"
        );
        out.bridge_parts.insert(key.to_owned(), bridge_use);
        fields.push(("parts".to_owned(), Value::Array(parts)));
        for (path, value) in fields {
            s.execute(&demo_fields(&strip, &path, value))
                .unwrap_or_else(|e| panic!("the demo strip {key}: fields.set {path} refused: {e:?}"));
            *out.executed.entry("fields.set".to_owned()).or_default() += 1;
        }
        s.save().unwrap();
        s.close();
        if key == "-X1031" {
            out.x1031 = strip;
        } else {
            out.x1032 = strip;
        }
    }
    out
}

/// B2: the steps that place the 15 terminals of the demo variant with Place at `P = T - O`, `T` the golden strip
/// segment's CP `<strip tag>:<d>/a` and `O` the embedded terminal symbol's fn-CP 0 (orientation A). Every insertion
/// point must be a point of folio 103's connection grid.
pub fn demo_terminal_steps(job: &Path, x1031: &str, x1032: &str, map: SheetMap) -> Vec<(String, Value)> {
    let golden = golden_tables(103);
    let (index, coord) = grid_maps(golden.grid);
    let (s, _) = Session::open_read_only(job, "test").unwrap();
    let tables101 = s.page_tables("101").expect("the placed page 101");
    let terminal_shape = |key: &str| {
        let rep = tables101
            .reps
            .iter()
            .find(|r| r.symbol_key == key)
            .unwrap_or_else(|| panic!("no placed representation {key}"));
        s.symbol_shape(rep.symbol)
            .unwrap_or_else(|| panic!("no embedded symbol shape {key}"))
    };
    let mut out = Vec::new();
    for (key, strip, golden_key, designations) in [
        ("-X1031", x1031, "1X1:1", DEMO_X1031_TERMINALS.as_slice()),
        ("-X1032", x1032, "1X1:PE", DEMO_X1032_TERMINALS.as_slice()),
    ] {
        let (symbol, _) = golden_rep_symbol(101, golden_key);
        let shape = terminal_shape(&symbol);
        let offset = shape
            .cps
            .iter()
            .find(|c| c.fn_cp == 0)
            .unwrap_or_else(|| panic!("the terminal symbol {symbol} has no fn CP 0"))
            .at;
        for d in designations {
            let label = format!("{key}:{d}/a");
            let t = golden
                .cps
                .iter()
                .find(|c| c.label == label)
                .unwrap_or_else(|| panic!("no golden strip CP {label}"));
            let p = (t.at.0 - offset.0, t.at.1 - offset.1);
            assert_eq!(
                coord(index(p.0)),
                p.0,
                "the terminal {label}: the x of its insertion point {p:?} is on the grid"
            );
            assert_eq!(
                coord(index(p.1)),
                p.1,
                "the terminal {label}: the y of its insertion point {p:?} is on the grid"
            );
            let (x, y) = map.mm(p);
            out.push((
                "place.pick".to_owned(),
                json!({"symbol": symbol.clone(), "strip": strip, "designation": d}),
            ));
            out.push(("cursor.set".to_owned(), json!({"x_mm": x, "y_mm": y})));
            out.push(("canvas.activate".to_owned(), json!({"alt": true})));
            out.push(("ui.dismiss".to_owned(), json!({})));
        }
    }
    out
}

/// B3: the `fields.set parts` commands of the 15 placed terminals, found by strip and designation and set to their
/// golden terminal's parts.
pub fn demo_terminal_part_steps(job: &Path, x1031: &str, x1032: &str) -> Vec<AnyCommand> {
    let all = records();
    let (s, _) = Session::open_read_only(job, "test").unwrap();
    let q = s.query();
    let mut out = Vec::new();
    for (key, strip, designations) in [
        ("-X1031", x1031, DEMO_X1031_TERMINALS.as_slice()),
        ("-X1032", x1032, DEMO_X1032_TERMINALS.as_slice()),
    ] {
        let strip_id = golden_device(&all, key)["id"].as_str().unwrap().to_owned();
        for d in designations {
            let golden = all
                .values()
                .find(|r| {
                    r["$k"] == "device"
                        && r["terminal"]["strip"].as_str() == Some(&strip_id)
                        && r["terminal"]["designation"].as_str() == Some(d)
                })
                .unwrap_or_else(|| panic!("no golden terminal {key}:{d}"));
            let device = q
                .devices()
                .iter()
                .find_map(|(id, dev)| {
                    let t = dev.aspects.terminal.as_ref()?;
                    (t.strip.to_string() == *strip && t.designation.as_str() == *d).then(|| id.to_string())
                })
                .unwrap_or_else(|| panic!("no placed terminal {key}:{d}"));
            out.push(AnyCommand {
                name: "fields.set".into(),
                params: json!({"kind": "device", "targets": [device], "path": "parts", "value": golden["parts"]}),
            });
        }
    }
    assert_eq!(out.len(), 15, "the 15 terminal part uses");
    out
}

/// B4: the wiring steps of the demo's folio 103, every end at the placed CP with the golden label.
pub fn demo_103_wiring(job: &Path, map: SheetMap) -> (Vec<(String, Value)>, usize) {
    let (s, _) = Session::open_read_only(job, "test").unwrap();
    let tables = s.page_tables("103").expect("the demo page 103");
    let cp_at: BTreeMap<String, (i64, i64)> = tables.cps.iter().map(|c| (c.label.clone(), c.at)).collect();
    let at = |label: &str| cp_at.get(label).copied();
    let (steps, commits, _) = wiring_steps_with(103, map, Some(&at));
    (steps, commits)
}

/// The CPs of a terminal representation that a drawn page connection ends at, as (label, potential).
fn demo_terminal_cps(
    tables: &ew_ui::ew_viewmodel::ew_api::output::PageTables,
    rep: &str,
) -> Vec<(String, Option<String>)> {
    tables
        .cps
        .iter()
        .filter(|c| c.rep.to_string() == rep && !c.connections.is_empty())
        .map(|c| (c.label.clone(), c.potential.clone()))
        .collect()
}

/// The values PI10 prints.
pub struct DemoPi10 {
    /// The potentials of the four terminals' drawn CPs before the bridges.
    pub before: Vec<(String, Option<String>)>,
    /// The same CPs' potentials after the three bridges.
    pub after: Vec<(String, Option<String>)>,
    /// The bridge group the three steps share.
    pub group: u64,
    /// The number of `connection.draw` records the three steps wrote.
    pub draws: usize,
    /// The project's revision before and after the three steps.
    pub rev_before: u64,
    pub rev_after: u64,
}

/// B5 (PI10): read the four terminals' drawn CP potentials, bridge -X1031:1 to :4 by three `strip.bridge` steps,
/// check the three page-less `connection.draw` records and read the potentials again.
pub fn demo_103_pi10(exe: &Path, tmp: &Tmp, job: &Path, x1031: &str) -> DemoPi10 {
    use ew_ui::ew_viewmodel::ew_api::CmdLog;
    let (s, _) = Session::open_read_only(job, "test").unwrap();
    let tables = s.page_tables("103").expect("the demo page 103");
    let reps: Vec<(String, String)> = {
        let q = s.query();
        DEMO_X1031_TERMINALS[..4]
            .iter()
            .map(|d| {
                let device = q
                    .devices()
                    .iter()
                    .find_map(|(id, dev)| {
                        let t = dev.aspects.terminal.as_ref()?;
                        (t.strip.to_string() == x1031 && t.designation.as_str() == *d).then(|| id.to_string())
                    })
                    .unwrap_or_else(|| panic!("no placed terminal -X1031:{d}"));
                let rep = tables
                    .reps
                    .iter()
                    .find(|r| r.device.is_some_and(|dev| dev.to_string() == device))
                    .unwrap_or_else(|| panic!("no representation of -X1031:{d}"));
                (
                    rep.id.to_string(),
                    rep.functions
                        .first()
                        .expect("a terminal representation has one function")
                        .to_string(),
                )
            })
            .collect()
    };
    let mut before = Vec::new();
    for (rep, _) in &reps {
        before.extend(demo_terminal_cps(&tables, rep));
    }
    let all_same_some = before.windows(2).all(|w| w[0].1.is_some() && w[0].1 == w[1].1);
    assert!(
        !before.is_empty() && !all_same_some,
        "before any bridge the potentials are not all one Some: {before:?}"
    );
    let mut script = steps(&[
        ("file.open", json!({"dir": path(job)})),
        ("page.goto", json!({"folio": "103"})),
    ]);
    let before_copy = tmp.path("gpp-demo-pi10-before.ew");
    copy_dir(job, &before_copy);
    for k in 0..3 {
        script.push((
            "select.set".to_owned(),
            json!({"ids": [reps[k].0.clone(), reps[k + 1].0.clone()]}),
        ));
        script.push(("strip.bridge".to_owned(), json!({})));
    }
    script.extend(steps(&[("file.save", json!({})), ("file.close", json!({}))]));
    let record = tmp.path("gpp-demo-bridges.ewcmd");
    all_done(&run(exe, tmp, "gpp-demo-bridge.jsonl", &script, Some(&record)));
    let log = CmdLog::parse(&std::fs::read_to_string(&record).unwrap()).unwrap();
    let commands: Vec<_> = log.commands().collect();
    assert_eq!(commands.len(), 3, "three strip.bridge commits");
    let shaft = golden_bridge_shaft();
    for (k, cmd) in commands.iter().enumerate() {
        assert_eq!(cmd.name, "connection.draw");
        assert!(
            cmd.params.get("page").is_none(),
            "the bridge is page-less: {}",
            cmd.params
        );
        let a = &cmd.params["a"]["cp"];
        let b = &cmd.params["b"]["cp"];
        assert_eq!(a["function"].as_str(), Some(reps[k].1.as_str()));
        assert_eq!(b["function"].as_str(), Some(reps[k + 1].1.as_str()));
        assert_eq!(a["idx"].as_u64(), Some(u64::from(shaft)));
        assert_eq!(b["idx"].as_u64(), Some(u64::from(shaft)));
        assert_eq!(cmd.params["kind"]["bridge"]["kind"], "plug_in");
        assert_eq!(cmd.params["kind"]["bridge"]["shaft"], 1);
        assert_eq!(cmd.params["kind"]["bridge"]["group"], 1);
    }
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state-gpp-demo-pi10-replay"));
    // The model revision is per session, so each step's "+1" is checked by replaying the recorded commands on the
    // copy taken before the script.
    let (mut replay, _) = Session::open(
        &before_copy,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    let rev_before = replay.world().rev.0;
    let mut rev_after = rev_before;
    for (k, cmd) in commands.iter().enumerate() {
        let receipt = replay
            .execute(*cmd)
            .unwrap_or_else(|e| panic!("the recorded {}: {e:?}", cmd.name));
        rev_after = receipt.rev.0;
        assert_eq!(
            rev_after,
            rev_before + u64::try_from(k + 1).unwrap(),
            "each strip.bridge step is one revision"
        );
    }
    replay.close();
    let (s, _) = Session::open_read_only(job, "test").unwrap();
    let tables = s.page_tables("103").expect("the demo page 103");
    let mut after = Vec::new();
    for (rep, _) in &reps {
        after.extend(demo_terminal_cps(&tables, rep));
    }
    for (label, potential) in &after {
        assert_eq!(
            potential.as_deref(),
            Some("24VDC-1"),
            "the potential of {label} after PI10"
        );
    }
    DemoPi10 {
        before,
        after,
        group: 1,
        draws: commands.len(),
        rev_before,
        rev_after,
    }
}

/// The values B6 prints.
pub struct DemoBridges {
    /// The distinct bridge groups per strip, by golden strip key.
    pub groups: BTreeMap<String, usize>,
    /// The golden bridge connections drawn.
    pub drawn: Vec<String>,
    /// The Session-executed records, by command name.
    pub executed: BTreeMap<String, usize>,
}

/// B6: the other golden bridges between placed terminals (all but BR1's three, already drawn), then the strips'
/// plug-in bridge part uses with the quantity of their distinct bridge groups.
pub fn demo_103_bridges(tmp: &Tmp, job: &Path, strips: &DemoStrips) -> DemoBridges {
    let all = records();
    let mut store = ew_ui::ew_viewmodel::ew_api::StoreOptions::new("test");
    store.state_dir = Some(tmp.path("state-gpp-demo-other-bridges"));
    let (mut s, _) = Session::open(
        job,
        store,
        ew_ui::ew_viewmodel::ew_api::SessionOptions::default(),
        ew_ui::ew_viewmodel::ew_api::Recovery::Keep,
    )
    .unwrap();
    let q = s.query();
    let mut demo_functions: BTreeMap<(String, String), String> = BTreeMap::new();
    for (id, dev) in q.devices().iter() {
        let Some(t) = dev.aspects.terminal.as_ref() else {
            continue;
        };
        let strip = t.strip.to_string();
        if strip != strips.x1031 && strip != strips.x1032 {
            continue;
        }
        let function = q.functions().iter().find_map(|(fid, f)| {
            let v = serde_json::to_value(&**f).unwrap();
            (v["device"].as_str() == Some(&id.to_string()) && v["role"].as_str().unwrap_or("main") == "main")
                .then(|| fid.to_string())
        });
        if let Some(function) = function {
            demo_functions.insert((strip, t.designation.as_str().to_owned()), function);
        }
    }
    let strip_ids: BTreeMap<String, String> = ["-X1031", "-X1032"]
        .into_iter()
        .map(|k| {
            (
                golden_device(&all, k)["id"].as_str().unwrap().to_owned(),
                k.to_owned(),
            )
        })
        .collect();
    let mut golden_terms: BTreeMap<String, (String, String)> = BTreeMap::new();
    for dev in all.values().filter(|r| r["$k"] == "device") {
        let Some(t) = dev.get("terminal") else {
            continue;
        };
        let Some(strip_id) = t["strip"].as_str() else {
            continue;
        };
        let Some(key) = strip_ids.get(strip_id) else {
            continue;
        };
        let Some(main) = golden_main_function(&all, dev) else {
            continue;
        };
        golden_terms.insert(
            main["id"].as_str().unwrap().to_owned(),
            (key.clone(), t["designation"].as_str().unwrap().to_owned()),
        );
    }
    let end = |v: &Value| -> Option<(String, u16)> {
        let cp = v.get("cp")?;
        Some((
            cp.get(0)?.as_str()?.to_owned(),
            u16::try_from(cp.get(1)?.as_u64()?).ok()?,
        ))
    };
    let mut existing: Vec<((String, u16), (String, u16))> = Vec::new();
    for (_, c) in q.connections().iter() {
        let v = serde_json::to_value(&**c).unwrap();
        if v["kind"].get("bridge").is_none() {
            continue;
        }
        if let (Some(a), Some(b)) = (end(&v["a"]), end(&v["b"])) {
            existing.push((a, b));
        }
    }
    let mut golden_bridges: Vec<&Value> = all
        .values()
        .filter(|r| r["$k"] == "connection" && r["kind"].get("bridge").is_some())
        .collect();
    golden_bridges.sort_by_key(|r| r["source_ref"]["key"].as_str().unwrap_or_default().to_owned());
    let mut commands: Vec<(String, AnyCommand)> = Vec::new();
    for c in golden_bridges {
        let (Some(a), Some(b)) = (end(&c["a"]), end(&c["b"])) else {
            continue;
        };
        let map = |cp: &(String, u16)| -> Option<(String, u16)> {
            let (key, designation) = golden_terms.get(&cp.0)?;
            let strip = if key == "-X1031" {
                &strips.x1031
            } else {
                &strips.x1032
            };
            let function = demo_functions.get(&(strip.clone(), designation.clone()))?;
            Some((function.clone(), cp.1))
        };
        let (Some(a), Some(b)) = (map(&a), map(&b)) else {
            continue;
        };
        if existing
            .iter()
            .any(|(x, y)| (x == &a && y == &b) || (x == &b && y == &a))
        {
            continue;
        }
        commands.push((
            c["source_ref"]["key"].as_str().unwrap().to_owned(),
            AnyCommand {
                name: "connection.draw".into(),
                params: json!({
                    "a": {"cp": {"function": a.0, "idx": a.1}},
                    "b": {"cp": {"function": b.0, "idx": b.1}},
                    "kind": c["kind"].clone(),
                }),
            },
        ));
    }
    let mut executed: BTreeMap<String, usize> = BTreeMap::new();
    for (_, cmd) in &commands {
        s.execute(cmd).unwrap_or_else(|e| panic!("{}: {e:?}", cmd.name));
        *executed.entry(cmd.name.clone()).or_default() += 1;
    }
    let q = s.query();
    let mut groups: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
    for (_, c) in q.connections().iter() {
        let v = serde_json::to_value(&**c).unwrap();
        let Some(bridge) = v["kind"].get("bridge") else {
            continue;
        };
        let Some(group) = bridge.get("group").and_then(Value::as_u64) else {
            continue;
        };
        let Some(function) = v["a"]["cp"][0].as_str() else {
            continue;
        };
        let strip = q
            .functions()
            .iter()
            .find(|(fid, _)| fid.to_string() == function)
            .and_then(|(_, f)| f.device)
            .and_then(|d| q.devices().get(d))
            .and_then(|d| d.aspects.terminal.as_ref())
            .map(|t| t.strip.to_string());
        if let Some(strip) = strip {
            groups.entry(strip).or_default().insert(group);
        }
    }
    let count31 = groups.get(&strips.x1031).map_or(0, BTreeSet::len);
    let count32 = groups.get(&strips.x1032).map_or(0, BTreeSet::len);
    assert_eq!((count31, count32), (2, 1), "the distinct bridge groups per strip");
    let mut parts_by_key: BTreeMap<String, Value> = BTreeMap::new();
    for (key, strip) in [("-X1031", &strips.x1031), ("-X1032", &strips.x1032)] {
        let device = q
            .devices()
            .iter()
            .find(|(id, _)| id.to_string() == *strip)
            .unwrap_or_else(|| panic!("no strip {key}"))
            .1;
        parts_by_key.insert(key.to_owned(), serde_json::to_value(&device.parts).unwrap());
    }
    drop(q);
    let mut out_groups = BTreeMap::new();
    for (key, strip, count) in [
        ("-X1031", &strips.x1031, count31),
        ("-X1032", &strips.x1032, count32),
    ] {
        let mut parts = parts_by_key[key].clone();
        let mut bridge = strips.bridge_parts[key].clone();
        bridge["qty"] = Value::from(count.to_string());
        parts
            .as_array_mut()
            .expect("the strip's parts are a list")
            .push(bridge);
        s.execute(&demo_fields(strip, "parts", parts))
            .unwrap_or_else(|e| panic!("the strip {key}: fields.set parts refused: {e:?}"));
        *executed.entry("fields.set".to_owned()).or_default() += 1;
        out_groups.insert(key.to_owned(), count);
    }
    s.save().unwrap();
    s.close();
    DemoBridges {
        groups: out_groups,
        drawn: commands.into_iter().map(|(key, _)| key).collect(),
        executed,
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The generators of the records no WP-S2.04 tool draws, the junction editor steps, the structure box and the marker
// tool (WP-S2.04 §13.6).

/// `num / den` floored, exact in `i128`.
fn floor_div(num: i128, den: i128) -> i128 {
    num.div_euclid(den)
}

/// `num / den` rounded up, exact in `i128`.
fn ceil_div(num: i128, den: i128) -> i128 {
    -((-num).div_euclid(den))
}

/// The steps that set the golden parts of every representation of `folio`: for each golden representation (sorted by
/// its key) whose golden device has a non-empty `parts` list, is no strip and is no black box placed by the black box
/// tool, `fields.set` on the placed device at the same symbol key and point. Returns (golden key, command) pairs, one
/// per device.
pub fn part_steps(folio: u32, placed: &Path) -> Vec<(String, AnyCommand)> {
    let all = records();
    let keys = symbol_keys();
    let page = all
        .values()
        .find(|r| r["$k"] == "page" && r["folio"]["number"] == folio)
        .unwrap_or_else(|| panic!("no page {folio}"))["id"]
        .clone();
    let (s, _) = Session::open_read_only(placed, "test").unwrap();
    let tables = s.page_tables(&folio.to_string()).expect("the placed page");
    let mut reps: Vec<&Value> = all
        .values()
        .filter(|r| r["$k"] == "representation" && r["page"] == page)
        .collect();
    reps.sort_by_key(|r| r["source_ref"]["key"].as_str().unwrap_or_default().to_owned());
    let mut out = Vec::new();
    let mut done: Vec<String> = Vec::new();
    for r in reps {
        let function = &all[r["functions"][0].as_str().unwrap()];
        let Some(device_id) = function["device"].as_str() else {
            continue;
        };
        let device = &all[device_id];
        if device.get("strip").is_some() || r.get("box").is_some() {
            continue;
        }
        let parts = &device["parts"];
        if parts.as_array().is_none_or(|a| a.is_empty()) {
            continue;
        }
        let symbol = keys[r["symbol"].as_str().unwrap()].clone();
        let pos = (r["pos"][0].as_i64().unwrap(), r["pos"][1].as_i64().unwrap());
        let placed_rep = tables
            .reps
            .iter()
            .find(|x| x.symbol_key == symbol && x.pos == pos)
            .unwrap_or_else(|| panic!("no placed representation {symbol} at {pos:?}"));
        let target = placed_rep
            .device
            .expect("a placed representation of a device")
            .to_string();
        if done.contains(&target) {
            continue;
        }
        done.push(target.clone());
        out.push((
            r["source_ref"]["key"]
                .as_str()
                .unwrap_or_default()
                .trim_start_matches("representations|")
                .to_owned(),
            AnyCommand {
                name: "fields.set".into(),
                params: json!({"kind": "device", "targets": [target], "path": "parts", "value": parts}),
            },
        ));
    }
    out
}

/// The far ends of a junction's sequence, by their printed labels (as the oracle names a junction).
fn junction_end_labels(
    t: &ew_ui::ew_viewmodel::ew_api::output::PageTables,
    j: &ew_ui::ew_viewmodel::ew_api::output::TableJunction,
) -> Vec<String> {
    use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
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

/// `potential_steps` (§13.6's golden potential definitions): for every golden `potential_def` record whose `at`
/// endpoint lies on one of `folios`, `potential.define` with exactly the golden members. A CP end `(function, idx)`
/// names the placed function of the representation at the same symbol key and point on that folio (at the same
/// index in its `functions`); a junction end names the placed junction with the same far-end labels. A definition
/// on one of the folios whose end does not map is an assertion failure naming it.
pub fn potential_steps(folios: &[u32], placed: &Path) -> Vec<(String, AnyCommand)> {
    use ew_ui::ew_viewmodel::ew_api::output::PageTables;
    let all = records();
    let keys = symbol_keys();
    let folio_of_page: BTreeMap<String, u32> = all
        .values()
        .filter(|r| r["$k"] == "page")
        .filter_map(|p| {
            Some((
                p["id"].as_str()?.to_owned(),
                u32::try_from(p["folio"]["number"].as_u64()?).ok()?,
            ))
        })
        .collect();
    let golden: BTreeMap<u32, PageTables> = folios.iter().map(|f| (*f, golden_tables(*f))).collect();
    let (s, _) = Session::open_read_only(placed, "test").unwrap();
    let placed_tables: BTreeMap<u32, PageTables> = folios
        .iter()
        .map(|f| (*f, s.page_tables(&f.to_string()).expect("a placed page")))
        .collect();
    let mut defs: Vec<&Value> = all.values().filter(|r| r["$k"] == "potential_def").collect();
    defs.sort_by_key(|r| r["source_ref"]["key"].as_str().unwrap_or_default().to_owned());
    let mut out = Vec::new();
    for def in defs {
        let label = def["source_ref"]["key"]
            .as_str()
            .unwrap_or_default()
            .trim_start_matches("potentials|")
            .to_owned();
        let at = &def["at"];
        let mapped = if let Some(cp) = at.get("cp") {
            let function = cp[0].as_str().unwrap_or_default().to_owned();
            let idx = u16::try_from(cp[1].as_u64().unwrap_or(u64::MAX)).expect("a CP index");
            // The only golden definition out of the requested folios is `DP` at -201U1 on folio 201 (the review of
            // round a28-r5, E4): skip only when the golden tables draw the function on none of the folios, never a
            // definition that is drawn there.
            let drawn = folios.iter().any(|folio| {
                golden[folio]
                    .cps
                    .iter()
                    .any(|cp| cp.function.to_string() == function)
            });
            if !drawn {
                continue;
            }
            // The golden representation of the CP's function on one of the folios.
            let mut found = None;
            for folio in folios {
                let mut reps: Vec<&Value> = all
                    .values()
                    .filter(|r| {
                        r["$k"] == "representation"
                            && r["page"]
                                .as_str()
                                .is_some_and(|p| folio_of_page.get(p) == Some(folio))
                    })
                    .filter(|r| {
                        r["functions"]
                            .as_array()
                            .is_some_and(|fs| fs.iter().any(|f| f.as_str() == Some(&function)))
                    })
                    .collect();
                reps.sort_by_key(|r| r["source_ref"]["key"].as_str().unwrap_or_default().to_owned());
                if let Some(rep) = reps.first() {
                    found = Some((*folio, (*rep).clone()));
                    break;
                }
            }
            let Some((folio, rep)) = found else {
                panic!(
                    "the potential {label}: its function {function} is drawn on {folios:?} but has no golden representation"
                );
            };
            let index = rep["functions"]
                .as_array()
                .unwrap_or_else(|| panic!("the potential {label}: no functions"))
                .iter()
                .position(|f| f.as_str() == Some(&function))
                .expect("the function of its representation");
            let symbol = keys[rep["symbol"].as_str().unwrap()].clone();
            let pos = (rep["pos"][0].as_i64().unwrap(), rep["pos"][1].as_i64().unwrap());
            let placed_rep = placed_tables[&folio]
                .reps
                .iter()
                .find(|r| r.symbol_key == symbol && r.pos == pos)
                .unwrap_or_else(|| {
                    panic!("the potential {label}: no placed representation {symbol} at {pos:?} on {folio}")
                });
            let placed_fn = *placed_rep.functions.get(index).unwrap_or_else(|| {
                panic!("the potential {label}: the placed representation has no function {index}")
            });
            json!({"cp": {"function": placed_fn.to_string(), "idx": idx}})
        } else if let Some(junction) = at.get("junction") {
            let golden_junction = junction.as_str().unwrap_or_default();
            let record = all
                .get(golden_junction)
                .unwrap_or_else(|| panic!("the potential {label}: no golden junction {golden_junction}"));
            let folio = record["page"]
                .as_str()
                .and_then(|p| folio_of_page.get(p))
                .copied()
                .unwrap_or_else(|| panic!("the potential {label}: the junction has no page"));
            if !folios.contains(&folio) {
                continue;
            }
            let gt = &golden[&folio];
            let golden_j = gt
                .junctions
                .iter()
                .find(|x| x.id.to_string() == golden_junction)
                .unwrap_or_else(|| {
                    panic!("the potential {label}: no golden junction {golden_junction} on {folio}")
                });
            let want = junction_end_labels(gt, golden_j);
            let t = &placed_tables[&folio];
            let placed_j = t
                .junctions
                .iter()
                .find(|x| junction_end_labels(t, x) == want)
                .unwrap_or_else(|| {
                    panic!("the potential {label}: no placed junction at {want:?} on {folio}")
                });
            json!({"junction": {"junction": placed_j.id.to_string()}})
        } else {
            panic!("the potential {label}: an endpoint the generator does not read: {at}");
        };
        let mut params = serde_json::Map::new();
        params.insert("at".into(), mapped);
        // Exactly the members the golden record has (the command denies unknown ones).
        for member in ["name", "ptype", "voltage", "env", "circuit_class"] {
            if let Some(v) = def.get(member) {
                params.insert(member.into(), v.clone());
            }
        }
        out.push((
            label,
            AnyCommand {
                name: "potential.define".into(),
                params: Value::Object(params),
            },
        ));
    }
    out
}

/// The golden `a` and `b` labels of the fixture's connection `wire`.
fn golden_wire_ends(wire: &str) -> (String, String) {
    let path = Path::new(GOLDEN_A)
        .parent()
        .unwrap()
        .join("J-E-2416/tables/connections.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.first() == Some(&wire) {
            return (fields[3].to_owned(), fields[4].to_owned());
        }
    }
    panic!("no golden connection {wire}");
}

/// The steps that give the golden junctions of `folio` their golden style and wiring (the junction editor through
/// the Select tool): for each golden junction in table order whose style is not `dot` or whose wiring is stored,
/// click its rehearsal point (the split point `wiring_steps` recorded), open the editor and set them.
pub fn junction_style_steps(folio: u32, map: SheetMap, points: &[(i64, i64)]) -> Vec<(String, Value)> {
    let tables = golden_tables(folio);
    assert_eq!(
        tables.junctions.len(),
        points.len(),
        "the golden junctions of {folio} and the split points of the wiring script"
    );
    let mut steps = Vec::new();
    for (j, at) in tables.junctions.iter().zip(points) {
        if j.style == "dot" && !j.explicit_wiring {
            continue;
        }
        let (x, y) = map.mm(*at);
        steps.push(("canvas.press".to_owned(), json!({"x_mm": x, "y_mm": y})));
        steps.push(("canvas.release".to_owned(), json!({"x_mm": x, "y_mm": y})));
        steps.push(("junction.open".to_owned(), json!({})));
        if j.style != "dot" {
            steps.push((
                "junction.style".to_owned(),
                json!({"style": j.style, "orient": j.bevel_orient}),
            ));
        }
        if j.explicit_wiring {
            steps.push(("junction.wiring".to_owned(), json!({"wiring": j.wiring})));
        }
        steps.push(("ui.dismiss".to_owned(), json!({})));
    }
    steps
}

/// The steps that draw the structure box around `anchors` (page nm) with the Structure box tool: the rectangle whose
/// corners are the connection-grid points one grid step outside the anchors' bounding box, then its fields, then
/// Enter.
pub fn structure_box_steps(map: SheetMap, grid: (i64, i64), anchors: &[(i64, i64)]) -> Vec<(String, Value)> {
    assert!(
        !anchors.is_empty(),
        "the structure box encloses at least one anchor"
    );
    let (_, coord) = grid_maps(grid);
    let exact = |v: i64| i128::from(v) * i128::from(grid.1);
    let num = i128::from(grid.0);
    let x0 = coord((floor_div(anchors.iter().map(|p| exact(p.0)).min().unwrap(), num) - 1) as i64);
    let x1 = coord((ceil_div(anchors.iter().map(|p| exact(p.0)).max().unwrap(), num) + 1) as i64);
    let y0 = coord((floor_div(anchors.iter().map(|p| exact(p.1)).min().unwrap(), num) - 1) as i64);
    let y1 = coord((ceil_div(anchors.iter().map(|p| exact(p.1)).max().unwrap(), num) + 1) as i64);
    let mut steps = steps(&[("tool.structure_box", json!({}))]);
    for corner in [(x0, y0), (x1, y1)] {
        let (x, y) = map.mm(corner);
        steps.push(("cursor.set".to_owned(), json!({"x_mm": x, "y_mm": y})));
        steps.push(("canvas.activate".to_owned(), json!({})));
    }
    steps.push(("tool.finish".to_owned(), json!({})));
    steps.push((
        "structure_box.fields".to_owned(),
        json!({
            "style": "dash_dot",
            "caption": "External Connection",
            "location": "+J-E-206",
            "external": "JE-N1000-03:L",
        }),
    ));
    steps.push(("canvas.activate".to_owned(), json!({})));
    steps.push(("ui.dismiss".to_owned(), json!({})));
    steps
}

/// A marker group the Marker tool's crossing line cannot place: its connections in golden order and its
/// cross-section; the rehearsal places it with `marker.place`.
#[derive(Debug)]
pub struct MarkerFallback {
    /// The rehearsal connection IDs, in golden order.
    pub connections: Vec<String>,
    /// The cross-section of the group ("2.5", "1.5").
    pub csa: String,
}

/// Whether the page tables' end label `label` names the golden table end `golden`. An exact match, except for the
/// golden table's terminal side words (`1X1:1/internal`, `1X1:1/field`, which select a terminal's CP by its side):
/// a terminal's two CPs print the same clamp (`1X1:1/a`) in the page tables, so those ends compare the terminal and
/// designation only.
fn golden_end_matches(label: &str, golden: &str) -> bool {
    if let Some((owner, selector)) = golden.rsplit_once('/')
        && matches!(selector, "internal" | "field")
    {
        return label.rsplit_once('/').map_or(label, |(have, _)| have) == owner;
    }
    label == golden
}

/// The crossing segment that lists exactly `ids` in that order, horizontal rows first (ascending), then vertical
/// columns, each in both directions.
fn crossing_segment(
    tables: &ew_ui::ew_viewmodel::ew_api::output::PageTables,
    lines: &[Vec<(i64, i64)>],
    ids: &[String],
) -> Option<((i64, i64), (i64, i64))> {
    use ew_ui::ew_viewmodel::tools::crossed_connections;
    let (index, coord) = grid_maps(tables.grid);
    let xs: Vec<i64> = lines.iter().flatten().map(|p| p.0).collect();
    let ys: Vec<i64> = lines.iter().flatten().map(|p| p.1).collect();
    let (min_x, max_x) = (*xs.iter().min()?, *xs.iter().max()?);
    let (min_y, max_y) = (*ys.iter().min()?, *ys.iter().max()?);
    let left = coord(index(min_x) - 1);
    let right = coord(index(max_x) + 1);
    let top = coord(index(min_y) - 1);
    let bottom = coord(index(max_y) + 1);
    let hits = |a: (i64, i64), b: (i64, i64)| -> Vec<String> {
        crossed_connections(tables, a, b)
            .into_iter()
            .map(|(e, _)| e.0.to_string())
            .collect()
    };
    let num = i128::from(tables.grid.0);
    let exact = |v: i64| i128::from(v) * i128::from(tables.grid.1);
    let row_from = floor_div(exact(min_y), num) + 1;
    let row_to = ceil_div(exact(max_y), num) - 1;
    for k in row_from..=row_to {
        let y = coord(k as i64);
        for (a, b) in [((left, y), (right, y)), ((right, y), (left, y))] {
            if hits(a, b) == ids {
                return Some((a, b));
            }
        }
    }
    let col_from = floor_div(exact(min_x), num) + 1;
    let col_to = ceil_div(exact(max_x), num) - 1;
    for k in col_from..=col_to {
        let x = coord(k as i64);
        for (a, b) in [((x, top), (x, bottom)), ((x, bottom), (x, top))] {
            if hits(a, b) == ids {
                return Some((a, b));
            }
        }
    }
    None
}

/// The rehearsal connection of the golden connection `wire`, matched by its printed end labels as the marker
/// generator matches a group's members (a junction end matches `@`). Panics naming the wire when none matches.
fn connection_for_wire<'a>(
    tables: &'a ew_ui::ew_viewmodel::ew_api::output::PageTables,
    wire: &str,
) -> &'a ew_ui::ew_viewmodel::ew_api::output::TableConnection {
    use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
    let (ga, gb) = golden_wire_ends(wire);
    let label = |e: TableEnd| -> Option<String> {
        match e {
            TableEnd::Cp { function, idx } => tables
                .cps
                .iter()
                .find(|c| c.function == function && c.idx == idx)
                .map(|c| c.label.clone()),
            TableEnd::Junction { .. } => None,
        }
    };
    let at = |e: TableEnd, golden: &str| {
        if golden.starts_with('@') {
            matches!(e, TableEnd::Junction { .. })
        } else {
            label(e).is_some_and(|l| golden_end_matches(&l, golden))
        }
    };
    tables
        .connections
        .iter()
        .find(|c| (at(c.a, &ga) && at(c.b, &gb)) || (at(c.a, &gb) && at(c.b, &ga)))
        .unwrap_or_else(|| panic!("no rehearsal connection for {wire} ({ga} / {gb})"))
}

/// The ID text of the rehearsal connection of the golden connection `wire`, found as [`marker_steps`] finds a
/// group's members.
pub fn rehearsal_connection_id(
    tables: &ew_ui::ew_viewmodel::ew_api::output::PageTables,
    wire: &str,
) -> String {
    connection_for_wire(tables, wire).id.to_string()
}

/// The steps of the four reference marker groups of doc 08 §4.5 on `folio` (101: [W4, W5] and [W7, W9] with 2.5;
/// 102: [W20, W21, W22, W23] and [W24, W25] with 1.5): the Marker tool's crossing line through a segment that
/// crosses exactly the group's connections of `tables` in golden order, its field, and Enter. A group no segment
/// crosses becomes a [`MarkerFallback`].
pub fn marker_steps(
    folio: u32,
    tables: &ew_ui::ew_viewmodel::ew_api::output::PageTables,
    map: SheetMap,
) -> (Vec<(String, Value)>, Vec<MarkerFallback>) {
    let groups: Vec<(Vec<&str>, &str)> = match folio {
        101 => vec![(vec!["W4", "W5"], "2.5"), (vec!["W7", "W9"], "2.5")],
        102 => vec![
            (vec!["W20", "W21", "W22", "W23"], "1.5"),
            (vec!["W24", "W25"], "1.5"),
        ],
        _ => panic!("no golden marker group on folio {folio}"),
    };
    let mut steps = Vec::new();
    let mut fallbacks = Vec::new();
    for (wires, csa) in groups {
        let mut ids = Vec::new();
        let mut lines = Vec::new();
        for wire in &wires {
            let connection = connection_for_wire(tables, wire);
            ids.push(connection.id.to_string());
            lines.push(connection.polyline.clone());
        }
        let Some((a, b)) = crossing_segment(tables, &lines, &ids) else {
            fallbacks.push(MarkerFallback {
                connections: ids,
                csa: csa.to_owned(),
            });
            continue;
        };
        steps.push(("tool.marker".to_owned(), json!({})));
        for p in [a, b] {
            let (x, y) = map.mm(p);
            steps.push(("cursor.set".to_owned(), json!({"x_mm": x, "y_mm": y})));
            steps.push(("canvas.activate".to_owned(), json!({})));
        }
        steps.push(("marker.fields".to_owned(), json!({"csa": csa})));
        steps.push(("tool.finish".to_owned(), json!({})));
        steps.push(("ui.dismiss".to_owned(), json!({})));
    }
    (steps, fallbacks)
}

/// Every cross-reference text drawn on the pages of `folios` whose link target page is one of them, as
/// (text, target cell) pairs, sorted: the representation's xref slots (`PageSlotsOf`) and the target's cell
/// (`Session::link_targets`).
pub fn xref_texts(s: &Session, folios: &[&str]) -> Vec<(String, String)> {
    use ew_ui::ew_viewmodel::ew_api::output::PageSlotsOf;
    use ew_ui::ew_viewmodel::ew_api::output::ew_dl::Destination;
    let q = s.query();
    let mut pages = Vec::new();
    let mut page_folio = BTreeMap::new();
    for (id, p) in q.pages().iter() {
        let label = p
            .folio
            .label
            .as_ref()
            .map(|l| l.as_str().to_owned())
            .unwrap_or_else(|| p.folio.number.to_string());
        if folios.contains(&label.as_str()) {
            page_folio.insert(id, label.clone());
            pages.push((id, label));
        }
    }
    let mut rep_folio = BTreeMap::new();
    for (id, r) in q.reps().iter() {
        rep_folio.insert(id.as_uuid(), page_folio.get(&r.page).cloned());
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for (id, _) in &pages {
        for (_, slots) in q.get::<PageSlotsOf>(*id).iter() {
            for slot in slots {
                let mut items: Vec<(String, Destination)> = Vec::new();
                for link in &slot.item_links {
                    items.push((slot.text[link.range.clone()].to_owned(), link.dest));
                }
                if let Some(dest) = slot.link {
                    items.push((slot.text.clone(), dest));
                }
                for (text, dest) in items {
                    let Destination::Representation(target) = dest else {
                        continue;
                    };
                    let target_folio = rep_folio.get(&target).and_then(|f| f.clone());
                    if !target_folio.is_some_and(|l| folios.contains(&l.as_str())) {
                        continue;
                    }
                    let link = s.link_targets(&[dest]);
                    let Some(link) = link[0].as_ref() else {
                        panic!("a cross-reference target that links to no page: {text}");
                    };
                    if !folios.contains(&link.folio.as_str()) {
                        continue;
                    }
                    out.push((text, link.cell.clone().unwrap_or_default()));
                }
            }
        }
    }
    out.sort();
    out
}
