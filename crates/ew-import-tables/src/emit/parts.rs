// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project-local part copies (doc 20 §6.1, §10, §14.1): `parts.csv` with its CP patterns, function templates
//! and cable cores. `library_id` is the project ID (doc 05 §13.2), `item_id = v5(ns, "parts|{key}")`.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_ids::v5;
use ew_schema::{
    CableCore, ClampKey, Confidence, CpData, ItemMeta, KeyedList, LibCopy, LibDocument, LibItem, LibRef,
    Library, OrderUnit, Part, PartKind, PartSections, PotentialType, Sha256, Side, TemplateFn,
};
use serde::Serialize;
use sha2::Digest;

use super::{CpInfo, Cx, PartInfo, TemplateInfo, fresh};
use crate::ids::{record_id, source_key};
use crate::table::{Rd, Row, Table, parse_ident};

const KINDS: &[(&str, PartKind)] = &[
    ("device", PartKind::Device),
    ("terminal", PartKind::Terminal),
    ("cable", PartKind::Cable),
    ("wire", PartKind::Wire),
    ("ferrule", PartKind::Ferrule),
    ("marker", PartKind::Marker),
    ("duct", PartKind::Duct),
    ("rail", PartKind::Rail),
    ("busbar", PartKind::Busbar),
    ("end_bracket", PartKind::EndBracket),
    ("enclosure", PartKind::Enclosure),
    ("mounting_plate", PartKind::MountingPlate),
    ("tube", PartKind::Tube),
    ("accessory", PartKind::Accessory),
    ("configured", PartKind::Configured),
];

/// Potential roles of CP cells.
pub(crate) const ROLES: &[(&str, PotentialType)] = &[
    ("l1", PotentialType::L1),
    ("l2", PotentialType::L2),
    ("l3", PotentialType::L3),
    ("l", PotentialType::L),
    ("n", PotentialType::N),
    ("pe", PotentialType::Pe),
    ("pen", PotentialType::Pen),
    ("fe", PotentialType::Fe),
    ("l_plus", PotentialType::LPlus),
    ("l_minus", PotentialType::LMinus),
    ("m", PotentialType::M),
    ("signal", PotentialType::Signal),
    ("undefined", PotentialType::Undefined),
];

/// Terminal sides of CP cells.
pub(crate) const SIDES: &[(&str, Side)] = &[("side1", Side::Side1), ("side2", Side::Side2)];

/// A clamp cell: `a` … `z`, `s<n>` (shaft) or `rail`.
pub(crate) fn parse_clamp(s: &str) -> Result<ClampKey, String> {
    if s == "rail" {
        return Ok(ClampKey::RailFoot);
    }
    if let Some(n) = s.strip_prefix('s')
        && let Ok(n) = n.parse::<u8>()
    {
        return Ok(ClampKey::Shaft(n));
    }
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_lowercase() => Ok(ClampKey::Clamp(c)),
        _ => Err(format!("`{s}` is not a clamp (a–z, s<n>, rail)")),
    }
}

fn clamp(r: &mut Rd<'_>, col: &str) -> Option<ClampKey> {
    let s = r.text(col)?;
    match parse_clamp(s) {
        Ok(c) => Some(c),
        Err(e) => {
            r.fail(format!("column `{col}`: {e}"));
            None
        }
    }
}

/// The SHA-256 of the item's compact JSON (doc 05 §13.2, EW-CORE-014).
fn item_sha256<T: Serialize>(item: &T) -> Sha256 {
    let json = serde_json::to_string(item).unwrap_or_default();
    Sha256(sha2::Sha256::digest(json.as_bytes()).into())
}

/// Groups the rows of a child table by their `part` cell, reporting rows of unknown parts.
fn by_part<'t>(
    cx: &mut Cx<'_>,
    table: Option<&'t Table>,
    parts: &BTreeMap<String, ()>,
) -> BTreeMap<String, Vec<Row<'t>>> {
    let mut out: BTreeMap<String, Vec<Row<'t>>> = BTreeMap::new();
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.into_iter().flat_map(Table::rows) {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        seen.insert(row.key().to_owned(), ());
        match row.get("part") {
            Some(p) if parts.contains_key(p) => out.entry(p.to_owned()).or_default().push(row),
            Some(p) => cx.unresolved(row, &format!("unknown part `{p}`")),
            None => cx.malformed(row, "column `part` is empty"),
        }
    }
    out
}

/// Emits every part as a project-local library copy and fills `cx.parts`.
pub(crate) fn emit(
    cx: &mut Cx<'_>,
    parts: Option<&Table>,
    part_cps: Option<&Table>,
    templates: Option<&Table>,
    cores: Option<&Table>,
) {
    let Some(parts) = parts else { return };
    let mut keys = BTreeMap::new();
    let mut heads = Vec::new();
    for row in parts.rows() {
        if !fresh(&mut cx.findings, row, &keys) {
            continue;
        }
        let mut r = row.reader();
        let manufacturer = r.req_ident("manufacturer");
        let part_number = r.req_ident("part_number");
        let kind = r.choice("kind", KINDS).unwrap_or_default();
        let type_designation = r.ident("type_designation");
        let description = r.mltext("description");
        let unit = r
            .choice(
                "unit",
                &[
                    ("piece", OrderUnit::Piece),
                    ("m", OrderUnit::Metre),
                    ("set", OrderUnit::Set),
                ],
            )
            .unwrap_or_default();
        let test_data = r.flag("test_data");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(manufacturer), Some(part_number)) = (manufacturer, part_number) else {
            continue;
        };
        keys.insert(row.key().to_owned(), ());
        heads.push((
            row,
            manufacturer,
            part_number,
            kind,
            type_designation,
            description,
            unit,
            test_data,
        ));
    }

    let mut cps = by_part(cx, part_cps, &keys);
    let mut tpls = by_part(cx, templates, &keys);
    let mut cable = by_part(cx, cores, &keys);
    let library = Id::<Library>::from_uuid(cx.project.as_uuid());

    for (row, manufacturer, part_number, kind, type_designation, description, unit, test_data) in heads {
        let key = row.key().to_owned();
        // CP pattern.
        let mut pattern = BTreeMap::new();
        let mut cp_data = Vec::new();
        for crow in cps.remove(&key).unwrap_or_default() {
            let mut r = crow.reader();
            let idx = r.req_uint::<u16>("idx");
            let designation = r.req_ident("designation");
            let potential_role = r.choice("potential_role", ROLES);
            let level = r.uint::<u8>("level");
            let side = r.choice("side", SIDES);
            let clamp = clamp(&mut r, "clamp");
            if let Some(i) = idx
                && pattern.contains_key(&i)
            {
                r.fail(format!("pattern index {i} repeated"));
            }
            if let Err(e) = r.finish() {
                cx.malformed(crow, &e);
                continue;
            }
            let (Some(idx), Some(designation)) = (idx, designation) else {
                continue;
            };
            pattern.insert(
                idx,
                CpInfo {
                    idx,
                    designation: designation.clone(),
                    side: side.clone(),
                    clamp: clamp.clone(),
                },
            );
            cp_data.push(CpData {
                idx,
                designation,
                potential_role,
                level,
                side,
                clamp,
                extra: Default::default(),
            });
        }
        // Function templates, in `n` order.
        let mut template_rows = Vec::new();
        for trow in tpls.remove(&key).unwrap_or_default() {
            let mut r = trow.reader();
            let n = r.req_uint::<u32>("n");
            let fndef = r.req("fndef").map(str::to_owned);
            let main = r.flag("main");
            let list: Vec<&str> = r.list("cps");
            if list.is_empty() {
                r.fail("column `cps` is empty (a template function lists its pattern indexes)");
            }
            let mut idxs = Vec::new();
            for i in list {
                match i.parse::<u16>() {
                    Ok(i) if pattern.contains_key(&i) => idxs.push(i),
                    _ => r.fail(format!(
                        "column `cps`: `{i}` is not an index of the part's CP pattern"
                    )),
                }
            }
            if let Err(e) = r.finish() {
                cx.malformed(trow, &e);
                continue;
            }
            let (Some(n), Some(fndef)) = (n, fndef) else {
                continue;
            };
            let Some(lib_ref) = cx.lib.fndef_ref(&fndef) else {
                cx.unresolved(trow, &format!("unknown FnDef `{fndef}`"));
                continue;
            };
            cx.used_lib.insert(fndef.clone());
            template_rows.push((n, fndef, main, idxs, lib_ref));
        }
        template_rows.sort_by_key(|t| t.0);
        // Cable cores, in `n` order.
        let mut core_rows = Vec::new();
        for crow in cable.remove(&key).unwrap_or_default() {
            let mut r = crow.reader();
            let n = r.req_uint::<u32>("n");
            let core = r.req_ident("core");
            let colour = r.colour("colour");
            let csa = r.csa("csa");
            let shield = r.flag("shield");
            let pair = r.uint::<u8>("pair");
            if let Err(e) = r.finish() {
                cx.malformed(crow, &e);
                continue;
            }
            let (Some(n), Some(core)) = (n, core) else {
                continue;
            };
            core_rows.push((
                n,
                CableCore {
                    core,
                    colour,
                    csa,
                    shield,
                    pair,
                    extra: Default::default(),
                },
            ));
        }
        core_rows.sort_by_key(|c| c.0);

        let Ok(cp_pattern) = KeyedList::from_vec(cp_data) else {
            cx.malformed(row, "CP pattern indexes repeat");
            continue;
        };
        let mut manufacturer_extra = ew_schema::Extra::new();
        manufacturer_extra.insert("key".into(), serde_json::Value::String(manufacturer.to_string()));
        let mut meta = ItemMeta {
            key: parse_ident(&key).ok(),
            sources: vec![format!("golden.table:{}", source_key("parts", &key))],
            ..ItemMeta::default()
        };
        if test_data {
            for section in ["part", "cp_pattern", "function_template", "sections"] {
                meta.confidence.insert(section.into(), Confidence::Test);
            }
        }
        let item = Part {
            meta,
            kind,
            manufacturer: LibRef {
                library,
                item: record_id::<LibItem>("manufacturers", manufacturer.as_str()),
                version_req: "*".into(),
                extra: manufacturer_extra,
            },
            part_number,
            type_designation,
            order_number: None,
            gtin: None,
            description,
            category: None,
            tech_data: KeyedList::default(),
            free_props: KeyedList::default(),
            unit,
            cp_pattern,
            function_template: template_rows
                .iter()
                .map(|(_, _, main, idxs, lib_ref)| TemplateFn {
                    fndef: lib_ref.clone(),
                    main: *main,
                    cps: idxs.clone(),
                    extra: Default::default(),
                })
                .collect(),
            sections: PartSections {
                cable_cores: core_rows.into_iter().map(|c| c.1).collect(),
                extra: Default::default(),
            },
            extra: Default::default(),
        };
        let item_id = record_id::<LibItem>("parts", &key);
        let copy = LibCopy {
            id: v5::lib_copy_id(library.as_uuid(), item_id.as_uuid()),
            v: 1,
            library_id: library,
            item_id,
            version: "1.0.0".into(),
            sha256: item_sha256(&item),
            item,
        };
        cx.parts.insert(
            key,
            PartInfo {
                id: copy.id,
                pattern,
                templates: template_rows
                    .into_iter()
                    .map(|(_, fndef, main, cps, _)| TemplateInfo { fndef, main, cps })
                    .collect(),
            },
        );
        cx.local_lib.push(LibDocument::Part(copy));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps() {
        assert_eq!(parse_clamp("b"), Ok(ClampKey::Clamp('b')));
        assert_eq!(parse_clamp("s1"), Ok(ClampKey::Shaft(1)));
        assert_eq!(parse_clamp("rail"), Ok(ClampKey::RailFoot));
        assert!(parse_clamp("B").is_err());
        assert!(parse_clamp("ab").is_err());
    }
}
