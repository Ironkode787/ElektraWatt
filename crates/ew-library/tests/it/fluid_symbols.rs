// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The lint of the shipped ISO 1219 pneumatic library (doc 11 §12, WP-S1.25): every symbol of representation type
//! `fluid` under `content/symbols` draws its function definition's ports only, each a fluid CP on the 4 mm grid with
//! a port role; every valve definition has a switching model whose paths name its CPs, whose rest and actuators
//! select a position it has, and whose solenoids are the ports 12, 14 or 10; and every symbol is named in en and nl.
//! The files are read as JSON, as the content audit reads them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn docs(dir: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(repo().join(dir)).expect("the content folder") {
        let p = e.expect("an entry").path();
        if p.extension().is_some_and(|x| x == "json") {
            out.push(serde_json::from_str(&std::fs::read_to_string(&p).expect("readable")).expect("JSON"));
        }
    }
    out
}

fn key(d: &Value) -> String {
    d["meta"]["key"].as_str().unwrap_or_default().to_owned()
}

/// Whether a definition is the process valve of a field device, whose ports are process ports, not fluid ports with a
/// role or a switching model (doc 11 §9.1).
fn process(fndef: &Value) -> bool {
    key(fndef) == "ew.fn.process-valve-2way"
}

/// The faults of one fluid symbol and its definition, in words.
fn lint(symbol: &Value, fndef: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let name = key(symbol);
    for lang in ["en", "nl"] {
        if symbol["name"][lang].as_str().is_none_or(str::is_empty) {
            out.push(format!("{name}: no {lang} name"));
        }
    }
    let cps: BTreeMap<u64, &Value> = fndef["cps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| Some((c["idx"].as_u64()?, c)))
        .collect();
    for c in symbol["cps"].as_array().into_iter().flatten() {
        let fluid = matches!(
            c["discipline"].as_str(),
            Some("pneumatic" | "vacuum" | "hydraulic")
        );
        let on_grid = c["pos"]
            .as_array()
            .is_some_and(|p| p.iter().all(|v| v.as_i64().is_some_and(|v| v % 4_000_000 == 0)));
        let def = c["fn_cp"].as_u64().and_then(|i| cps.get(&i));
        if !fluid {
            out.push(format!("{name}: CP {} is no fluid CP", c["idx"]));
        }
        if !on_grid {
            out.push(format!("{name}: CP {} is off the 4 mm grid", c["idx"]));
        }
        match def {
            None => out.push(format!("{name}: CP {} draws no port of its definition", c["idx"])),
            Some(d) if d["port_role"].is_null() && fndef["contact_kind"] == "valve" && !process(fndef) => {
                out.push(format!("{name}: port {} has no role", d["designation"]));
            }
            Some(_) => {}
        }
    }
    if fndef["contact_kind"] == "valve" && !process(fndef) {
        let Some(sections) = fndef["valve_model"]["sections"].as_array() else {
            out.push(format!("{name}: its valve definition has no switching model"));
            return out;
        };
        for s in sections {
            let positions = s["positions"].as_array().map_or(0, Vec::len) as u64;
            let mut indexes = Vec::new();
            for p in s["positions"].as_array().into_iter().flatten() {
                for path in p["paths"].as_array().into_iter().flatten() {
                    for (_, v) in path.as_object().into_iter().flatten() {
                        match v {
                            Value::Number(n) => indexes.extend(n.as_u64()),
                            Value::Object(m) => indexes.extend(m.values().filter_map(Value::as_u64)),
                            _ => {}
                        }
                    }
                }
            }
            for i in indexes {
                if !cps.contains_key(&i) {
                    out.push(format!("{name}: a path names CP {i}, which the definition lacks"));
                }
            }
            if s["rest"].as_u64().is_some_and(|r| r >= positions) {
                out.push(format!("{name}: its rest position is out of range"));
            }
            for a in s["actuation"].as_array().into_iter().flatten() {
                if a["selects"].as_u64().is_none_or(|p| p >= positions) {
                    out.push(format!("{name}: an actuator selects no position"));
                }
                if let Some(sol) = a["actuator"]["solenoid"]["solenoid"].as_str()
                    && !["10", "12", "14"].contains(&sol)
                {
                    out.push(format!("{name}: solenoid {sol} is not 10, 12 or 14"));
                }
            }
        }
    }
    out
}

#[test]
fn the_pneumatic_library_passes_its_lint() {
    let fndefs: BTreeMap<String, Value> = docs("content/fndefs").into_iter().map(|d| (key(&d), d)).collect();
    let mut checked = 0;
    let mut faults = Vec::new();
    for s in docs("content/symbols")
        .iter()
        .filter(|s| s["rep_type"] == "fluid")
    {
        let def_key = s["fndef"]["key"].as_str().unwrap_or_default();
        match fndefs.get(def_key) {
            Some(d) => faults.extend(lint(s, d)),
            None => faults.push(format!("{}: its definition {def_key} is not shipped", key(s))),
        }
        checked += 1;
    }
    assert!(checked > 0, "no fluid symbol was read");
    assert_eq!(faults, Vec::<String>::new());
}

#[test]
fn the_lint_names_a_path_to_a_missing_port_and_a_valve_without_model() {
    let symbol = serde_json::json!({"meta": {"key": "ew.iso.x"}, "name": {"en": "X"},
        "cps": [{"idx": 0, "fn_cp": 0, "pos": [0, 2000000], "discipline": "pneumatic"}]});
    let with_model = serde_json::json!({"meta": {"key": "ew.fn.x"}, "contact_kind": "valve",
        "cps": [{"idx": 0, "designation": "1", "port_role": "supply1"}],
        "valve_model": {"sections": [{"positions": [{"label": "a", "paths": [{"flow": {"from": 0, "to": 3}}]}],
            "rest": 2, "actuation": [{"actuator": {"solenoid": {"solenoid": "Y1"}}, "selects": 0}]}]}});
    assert_eq!(
        lint(&symbol, &with_model),
        [
            "ew.iso.x: no nl name",
            "ew.iso.x: CP 0 is off the 4 mm grid",
            "ew.iso.x: a path names CP 3, which the definition lacks",
            "ew.iso.x: its rest position is out of range",
            "ew.iso.x: solenoid Y1 is not 10, 12 or 14",
        ]
    );
    let mut without = with_model.clone();
    without["valve_model"] = Value::Null;
    assert_eq!(
        lint(&symbol, &without).last().map(String::as_str),
        Some("ew.iso.x: its valve definition has no switching model")
    );
}
