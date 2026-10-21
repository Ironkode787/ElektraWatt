// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The colour and size profiles of the project settings (doc 03 §5.1, doc 08 §4.2, doc 20 §7.1, §7.2):
//! `colour_profile.csv` and `size_profile.csv` → `ProjectSettings.extra["colour_profile"]` and
//! `["size_profile"]`, in the JSON shapes the wire-property chain of `ew-derive` reads.
//!
//! The rows come from the tables; what a table cannot hold is the shipped company profile of doc 08 §4.2 (the
//! sample of the pilot basic project) and doc 20 §7.1: its key, name, basis, the company code DBU and the
//! size profile's class rules. They are constants of this module.
//!
//! # Colour profile
//!
//! `rows[]` = `{group, function, potential_type, select, colour, colour_name}` in table order: `group` is the
//! voltage class, `function` the function, `select` the `ptype`, `ac_dc`, `u_min`, `u_max` and `analogue`
//! columns (`ProfileSelect`), `colour` the `code` column as a `ColourSpec`; `potential_type` (the printed type,
//! "signal, 24 V DC") and `colour_name` (the printed colour, "Dark Blue") are the labels of the cover table,
//! which a selector and a `ColourSpec` of several codes cannot give.
//!
//! # Size profile
//!
//! `rows[]` = `{class, csa, codes}` for `control` and `mains`: `csa` is the one cross-section the class
//! column holds and `codes` the colour codes that have it, in table order (the cover's standard-size tables).

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{Cx, fresh};
use crate::emit::parts::ROLES;
use crate::table::Table;

/// The key of the shipped company colour profile (doc 08 §4.2, doc 20 §7.1).
const COLOUR_PROFILE_KEY: &str = "company.dbu-wh-24v";
/// Its name.
const COLOUR_PROFILE_NAME: &str = "DBU/WH 24 V";
/// Its basis text (doc 20 §7.1, D-37): printed on the cover under the colour table.
const COLOUR_PROFILE_BASIS: &str =
    "company convention; codes per IEC 60757, DBU is a company code (BU, dark shade)";
/// The key and name of the shipped company size profile (doc 08 §4.3: Control 0.75 mm², Mains 1.5 mm²).
const SIZE_PROFILE_KEY: &str = "company.0.75-1.5";
const SIZE_PROFILE_NAME: &str = "0.75/1.5";

/// The two settings members of the profile tables, `(member name, value)`; a member is left out when its
/// table is absent or holds no usable row.
pub(crate) fn emit(cx: &mut Cx<'_>, colour: Option<&Table>, size: Option<&Table>) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    if let Some(v) = colour_profile(cx, colour) {
        out.push(("colour_profile".to_owned(), v));
    }
    if let Some(v) = size_profile(cx, size) {
        out.push(("size_profile".to_owned(), v));
    }
    out
}

fn colour_profile(cx: &mut Cx<'_>, table: Option<&Table>) -> Option<Value> {
    let table = table?;
    let mut seen = BTreeMap::<String, ()>::new();
    let mut rows = Vec::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let group = r.text("voltage_class").map(str::to_owned);
        let function = r.text("function").map(str::to_owned);
        let potential_type = r.text("potential_type").map(str::to_owned);
        let ptype = r
            .choice("ptype", ROLES)
            .and_then(|_| r.text("ptype"))
            .map(str::to_owned);
        let ac_dc = r.choice("ac_dc", &[("ac", "ac"), ("dc", "dc")]);
        let u_min = r.dec3("u_min");
        let u_max = r.dec3("u_max");
        let analogue = r.flag("analogue");
        let colour = r.colour("code");
        let name = r.text("colour").map(str::to_owned);
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let Some(colour) = colour else {
            cx.malformed(row, "column `code` is empty");
            continue;
        };
        let mut select = serde_json::Map::new();
        if let Some(p) = ptype {
            select.insert("ptype".into(), json!([p]));
        }
        if let Some(a) = ac_dc {
            select.insert("ac_dc".into(), a.into());
        }
        if let Some(v) = u_min {
            select.insert("u_min".into(), v.to_string().into());
        }
        if let Some(v) = u_max {
            select.insert("u_max".into(), v.to_string().into());
        }
        if analogue {
            select.insert("analogue".into(), true.into());
        }
        seen.insert(row.key().to_owned(), ());
        let mut value = serde_json::Map::new();
        value.insert("group".into(), json!({ "en": group.unwrap_or_default() }));
        value.insert("function".into(), json!({ "en": function.unwrap_or_default() }));
        if let Some(t) = potential_type {
            value.insert("potential_type".into(), json!({ "en": t }));
        }
        value.insert("select".into(), Value::Object(select));
        value.insert("colour".into(), json!(colour));
        if let Some(n) = name {
            value.insert("colour_name".into(), json!({ "en": n }));
        }
        rows.push(Value::Object(value));
    }
    if rows.is_empty() {
        return None;
    }
    Some(json!({
        "key": COLOUR_PROFILE_KEY,
        "name": { "en": COLOUR_PROFILE_NAME },
        "basis": "company_convention",
        "basis_note": { "en": COLOUR_PROFILE_BASIS },
        "verified": true,
        // DBU is the company code of the profile: BU in the dark shade (doc 20 §7.1).
        "codes": [{ "code": "DBU", "base": "BU", "shade": "dark", "export_as": "DBU" }],
        "rows": rows,
    }))
}

fn size_profile(cx: &mut Cx<'_>, table: Option<&Table>) -> Option<Value> {
    let table = table?;
    let mut seen = BTreeMap::<String, ()>::new();
    // class → (csa, codes in table order)
    let mut classes: BTreeMap<&'static str, (Option<String>, Vec<String>)> = BTreeMap::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let control = r.dec3("control");
        let mains = r.dec3("mains");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        seen.insert(row.key().to_owned(), ());
        for (class, csa) in [("control", control), ("mains", mains)] {
            let Some(csa) = csa else { continue };
            let entry = classes.entry(class).or_default();
            let csa = csa.to_string();
            match &entry.0 {
                None => entry.0 = Some(csa),
                Some(c) if *c == csa => {}
                Some(c) => {
                    cx.malformed(
                        row,
                        &format!("column `{class}`: the size `{csa}` differs from `{c}` of an earlier row"),
                    );
                    continue;
                }
            }
            entry.1.push(row.key().to_owned());
        }
    }
    let rows: Vec<Value> = classes
        .into_iter()
        .filter_map(|(class, (csa, codes))| {
            csa.map(|csa| json!({ "class": class, "csa": csa, "codes": codes }))
        })
        .collect();
    if rows.is_empty() {
        return None;
    }
    // The class rules, the PE rule and the bond rule of the shipped profile (doc 08 §4.2).
    Some(json!({
        "key": SIZE_PROFILE_KEY,
        "name": { "en": SIZE_PROFILE_NAME },
        "verified": true,
        "class_rules": [
            {"select": {"ptype": ["pe", "pen", "fe"]}, "class": "pe"},
            {"select": {"discipline": "bus"}, "class": "bus"},
            {"select": {"analogue": true}, "class": "analogue"},
            {"select": {"ac_dc": "ac", "u_min": "50"}, "class": "mains"},
            {"select": {}, "class": "control"},
        ],
        "pe_class": "highest_at_ends",
        "bond_rule": {"colour_from": "pe", "class_from": "non_pe_side"},
        "rows": rows,
    }))
}
