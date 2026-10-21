// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The colour and size profiles of the project settings (doc 03 §5.1, doc 08 §4.2, doc 20 §7.1, §7.2):
//! `profiles.csv`, `colour_profile.csv` and `size_profile.csv` → `ProjectSettings.extra["colour_profile"]` and
//! `["size_profile"]`, in the JSON shapes the wire-property chain of `ew-derive` reads. Everything comes from
//! the tables; the importer holds no profile data of its own.
//!
//! # `profiles.csv`
//!
//! One row per profile, told apart by `profile` (`colour` or `size`): `key` (the profile id), `name`, and for
//! the colour profile `basis` (an identifier such as `company_convention`), `basis_note` (the basis text),
//! `standards` (the standards text the cover prints, doc 20 U-35), `dbu_code` (`<code>:<base>:<shade>`, the
//! company code the profile declares) and `verified`; for the size profile `pe_class`, `class_rules` and
//! `bond_rule` (the last two as JSON text).
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
//! column holds and `codes` the colour codes that have it, each `{code, order}` with the `order` column of its
//! row (the cover lists them in that order), in table order.

use std::collections::BTreeMap;

use ew_interop_api::{Code, Severity};
use serde_json::{Value, json};

use super::{Cx, fresh};
use crate::emit::parts::ROLES;
use crate::table::{Rd, Row, Table};

/// The two settings members of the profile tables, `(member name, value)`; a member is left out when its
/// table is absent or holds no usable row, or `profiles.csv` has no row for it (reported).
pub(crate) fn emit(
    cx: &mut Cx<'_>,
    profiles: Option<&Table>,
    colour: Option<&Table>,
    size: Option<&Table>,
) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let mut headers = headers(cx, profiles);
    for (member, table, kind) in [
        ("colour_profile", colour, "colour"),
        ("size_profile", size, "size"),
    ] {
        let Some(table) = table else { continue };
        let Some(header) = headers.remove(kind) else {
            let file = table.file.clone();
            cx.findings.at(
                Code::UNRESOLVED_REFERENCE,
                Severity::Error,
                &file,
                None,
                None,
                format!("table ignored: profiles.csv has no `{kind}` row for it"),
            );
            continue;
        };
        let value = if kind == "colour" {
            colour_profile(cx, table, header)
        } else {
            size_profile(cx, table, header)
        };
        if let Some(v) = value {
            out.push((member.to_owned(), v));
        }
    }
    out
}

/// The header members of a profile (everything but the rows), as read from `profiles.csv`.
type Header = serde_json::Map<String, Value>;

/// `profiles.csv`: the header of each profile by its `profile` cell.
fn headers(cx: &mut Cx<'_>, table: Option<&Table>) -> BTreeMap<String, Header> {
    let mut out = BTreeMap::new();
    let Some(table) = table else { return out };
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let kind = r.req_choice("profile", &[("colour", "colour"), ("size", "size")]);
        let name = r.req("name").map(str::to_owned);
        let mut h = Header::new();
        h.insert("key".into(), row.key().into());
        if let Some(n) = &name {
            h.insert("name".into(), json!({ "en": n }));
        }
        h.insert("verified".into(), r.flag("verified").into());
        match kind {
            Some("colour") => colour_header(&mut r, &mut h),
            Some("size") => size_header(&mut r, &mut h),
            _ => {}
        }
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let Some(kind) = kind else { continue };
        if out.contains_key(kind) {
            cx.malformed(row, &format!("a second `{kind}` profile"));
            continue;
        }
        seen.insert(row.key().to_owned(), ());
        out.insert(kind.to_owned(), h);
    }
    out
}

fn colour_header(r: &mut Rd<'_>, h: &mut Header) {
    if let Some(b) = r.text("basis") {
        h.insert("basis".into(), b.into());
    }
    for col in ["basis_note", "standards"] {
        if let Some(t) = r.text(col) {
            h.insert(col.into(), json!({ "en": t }));
        }
    }
    if let Some(cell) = r.text("dbu_code") {
        let parts: Vec<&str> = cell.split(':').collect();
        match parts.as_slice() {
            [code, base, shade] => {
                h.insert(
                    "codes".into(),
                    json!([{ "code": code, "base": base, "shade": shade, "export_as": code }]),
                );
            }
            _ => r.fail(format!(
                "column `dbu_code`: `{cell}` is not <code>:<base>:<shade>"
            )),
        }
    }
}

fn size_header(r: &mut Rd<'_>, h: &mut Header) {
    if let Some(p) = r.text("pe_class") {
        h.insert("pe_class".into(), p.into());
    }
    for (col, member) in [("class_rules", "class_rules"), ("bond_rule", "bond_rule")] {
        if let Some(text) = r.text(col) {
            match serde_json::from_str::<Value>(text) {
                Ok(v) => {
                    h.insert(member.into(), v);
                }
                Err(e) => r.fail(format!("column `{col}`: not JSON: {e}")),
            }
        }
    }
}

fn colour_profile(cx: &mut Cx<'_>, table: &Table, header: Header) -> Option<Value> {
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
    let mut profile = header;
    profile.insert("rows".into(), Value::Array(rows));
    Some(Value::Object(profile))
}

fn size_profile(cx: &mut Cx<'_>, table: &Table, header: Header) -> Option<Value> {
    let mut seen = BTreeMap::<String, ()>::new();
    // class → (csa, codes with their order, in table order)
    let mut classes: BTreeMap<&'static str, (Option<String>, Vec<Value>)> = BTreeMap::new();
    for (index, row) in table.rows().enumerate() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let control = r.dec3("control");
        let mains = r.dec3("mains");
        let order = r.uint::<u32>("order");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        seen.insert(row.key().to_owned(), ());
        let order = order.unwrap_or_else(|| u32::try_from(index + 1).unwrap_or(u32::MAX));
        for (class, csa) in [("control", control), ("mains", mains)] {
            let Some(csa) = csa else { continue };
            let entry = classes.entry(class).or_default();
            let csa = csa.to_string();
            match &entry.0 {
                None => entry.0 = Some(csa),
                Some(c) if *c == csa => {}
                Some(c) => {
                    size_conflict(cx, row, class, &csa, c);
                    continue;
                }
            }
            entry.1.push(json!({ "code": row.key(), "order": order }));
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
    let mut profile = header;
    profile.insert("rows".into(), Value::Array(rows));
    Some(Value::Object(profile))
}

fn size_conflict(cx: &mut Cx<'_>, row: Row<'_>, class: &str, csa: &str, first: &str) {
    cx.malformed(
        row,
        &format!("column `{class}`: the size `{csa}` differs from `{first}` of an earlier row"),
    );
}
