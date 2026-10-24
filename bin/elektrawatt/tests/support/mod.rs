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

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

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

/// Every record of the fixture's page and device shards, by ID.
fn records() -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for dir in ["pages", "devices"] {
        let mut files: Vec<_> = std::fs::read_dir(Path::new(GOLDEN_A).join(dir))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        for f in files {
            for r in lines(&f) {
                if let Some(id) = r["id"].as_str() {
                    out.insert(id.to_owned(), r);
                }
            }
        }
    }
    out
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
/// module documentation): `strip` is the ID of the strip the start project holds for the folio's terminals. Also
/// returns the number of `command.run` steps (the steps a later part owns).
pub fn placement_steps(folio: u32, map: SheetMap, strip: &str) -> (Vec<(String, Value)>, usize) {
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
/// member from its far end onto that wire one grid step from the first end along its leaving direction (the junction
/// is created there and the wire split, `connection.draw`'s `on_connection`); the junctions are drawn before the other
/// connections, so that no other wire passes their split points. Returns the steps and the number of Wire tool
/// commits.
pub fn wiring_steps(folio: u32, map: SheetMap) -> (Vec<(String, Value)>, usize) {
    use ew_ui::ew_viewmodel::ew_api::geometry::Dir;
    use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
    let t = golden_tables(folio);
    let (index, coord) = grid_maps(t.grid);
    let cp_of = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => t.cps.iter().find(|c| c.function == *function && c.idx == *idx),
        TableEnd::Junction { .. } => None,
    };
    let mut steps: Vec<(String, Value)> = vec![
        ("view.zoom_actual".to_owned(), json!({})),
        ("tool.wire".to_owned(), json!({})),
    ];
    let mut commits = 0;
    let enter = |steps: &mut Vec<(String, Value)>, p: (i64, i64)| {
        let (x, y) = map.mm(p);
        steps.push(("cursor.set".to_owned(), json!({"x_mm": x, "y_mm": y})));
        steps.push(("canvas.activate".to_owned(), json!({})));
    };
    // The junctions first: their split points (one step from a CP) then lie on no other wire.
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
        enter(&mut steps, f0.at);
        enter(&mut steps, f1.at);
        commits += 1;
        let stub = match f0.dir {
            Some(Dir::Down) => (f0.at.0, coord(index(f0.at.1) + 1)),
            Some(Dir::Up) => (f0.at.0, coord(index(f0.at.1) - 1)),
            Some(Dir::Right) => (coord(index(f0.at.0) + 1), f0.at.1),
            Some(Dir::Left) => (coord(index(f0.at.0) - 1), f0.at.1),
            None => panic!("a first member without a direction"),
        };
        for f in &far[2..] {
            enter(&mut steps, f.at);
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
        enter(&mut steps, a.at);
        enter(&mut steps, b.at);
        commits += 1;
    }
    steps.push(("ui.dismiss".to_owned(), json!({})));
    (steps, commits)
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
