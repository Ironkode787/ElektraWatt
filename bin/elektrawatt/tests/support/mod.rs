// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support of the folio scripts (WP-S2.04 §13.6): the generator of a folio's UI script from Golden A's records,
//! read as the JSON Lines of the committed fixture (`golden/J-E-2416.ew`, never written).
//!
//! **What part 1 draws.** Each representation of the folio by the gesture of the part that owns it: a symbol with
//! the Place tool (`place.pick` with its key and the device's letter, then the grid cursor on the golden insertion
//! point and Enter, so no pointer and no hover reaches the transcript); a terminal with the Place tool's
//! `new_terminal` target on the strip the start project holds, with the golden designation; an interruption point
//! (the IP tool is part 3's) through `command.run` `ip.place` with the golden parameters, its external reference
//! left out. Connections, junctions and typed tags are not drawn by part 1's script (§13.6: the Wire tool is part
//! 2's; connections and typed tags would need the IDs the placements mint, which a static script cannot name).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

/// The committed fixture of Golden A.
pub const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

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
    let mut runs = 0;
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
            runs += 1;
            steps.push((
                "command.run".into(),
                json!({"cmd": "ip.place", "params": {
                    "page": {"ref": format!("folio:{folio}")},
                    "symbol": r["symbol"],
                    "pos": [pos.0, pos.1],
                    "name": ip["name"],
                    "direction": ip["direction"],
                }}),
            ));
            continue;
        }
        let device = &all[function["device"].as_str().unwrap()];
        let pick = match device.get("terminal") {
            Some(t) => json!({"symbol": symbol, "strip": strip, "designation": t["designation"]}),
            None => json!({"symbol": symbol, "letter": device["tag"]["letter"]}),
        };
        let (x, y) = map.mm(pos);
        steps.push(("place.pick".into(), pick));
        steps.push(("cursor.set".into(), json!({"x_mm": x, "y_mm": y})));
        steps.push(("canvas.activate".into(), json!({})));
        steps.push(("ui.dismiss".into(), json!({})));
    }
    (steps, runs)
}
