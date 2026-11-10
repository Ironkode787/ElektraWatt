// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for field values, paste and delete.

use super::*;

pub(super) fn fields_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(7)
        && let Some(step) = revision_fields_set(live, rng)
    {
        return Some(step);
    }
    if rng.chance(9)
        && let Some(step) = fields_set_copies(live, rng)
    {
        return Some(step);
    }
    let one = |kind: &str, target: &Value, path: &str, value: Value| {
        Step::cmd(
            "fields.set",
            json!({"kind": kind, "targets": [target], "path": path, "value": value}),
        )
    };
    match rng.below(26) {
        0 => {
            let device = rng.pick(&live.plain_devices)?;
            let text = rng.word(8);
            Some(one("device", device, "function_text", json!(text)))
        }
        1 => {
            let device = rng.pick(&live.plain_devices)?;
            let value = if rng.chance(2) {
                json!("spare")
            } else {
                Value::Null
            };
            Some(one("device", device, "state", value))
        }
        2 => {
            let c = rng.pick(&live.connections)?;
            // A colour is a list of IEC 60757 codes: green-yellow is the two codes GN and YE.
            let colour: &[&str] = rng.pick(&[&["BK"][..], &["RD"], &["BU"], &["WH"], &["GN", "YE"]])?;
            Some(one("connection", c, "overrides.colour", json!(colour)))
        }
        3 => {
            let c = rng.pick(&live.connections)?;
            let csa = *rng.pick(&["0.75", "1.5", "2.5"])?;
            Some(one("connection", c, "overrides.csa", json!(csa)))
        }
        4 => {
            let d = rng.pick(&live.plain_devices)?;
            let scheme = *rng.pick(&["default", "child"])?;
            Some(one("device", d, "tag.scheme", json!(scheme)))
        }
        5 => {
            let s = rng.pick(&live.section_info)?;
            Some(one("section", &s.id, "title", json!(rng.word(10))))
        }
        6 => {
            let s = rng.pick(&live.section_info)?;
            Some(one("section", &s.id, "divider", json!(rng.chance(2))))
        }
        7 => {
            let n = rng.pick(&live.nodes)?;
            Some(one("structure_node", &n.id, "description", json!(rng.word(12))))
        }
        8 => {
            let n = rng.pick(&live.nodes)?;
            Some(one("structure_node", &n.id, "order", json!(rng.below(5))))
        }
        9 => {
            let e = rng.pick(&live.ext_refs)?;
            let value = if rng.chance(3) {
                Value::Null
            } else {
                json!(rng.word(4))
            };
            Some(one("external_ref", e, "connection", value))
        }
        10 => {
            let c = rng.pick(&live.conductors)?;
            let csa = *rng.pick(&["0.5", "1", "2.5", "4"])?;
            Some(one("conductor", c, "csa", json!(csa)))
        }
        11 => {
            let c = rng.pick(&live.conductors)?;
            let colour = *rng.pick(&["BK", "RD", "BU", "GN"])?;
            Some(one("conductor", c, "colour", json!([colour])))
        }
        12 => {
            let p = rng.pick(&live.potential_defs)?;
            let class = *rng.pick(&["mains", "control", "analogue", "bus", "pe"])?;
            Some(one("potential_def", p, "circuit_class", json!(class)))
        }
        13 => {
            let p = rng.pick(&live.potential_defs)?;
            let v = *rng.pick(&["24", "230", "400", "0"])?;
            Some(one("potential_def", p, "voltage", json!(v)))
        }
        14 => {
            let b = rng.pick(&live.blocks)?;
            Some(one("report_block", b, "title", json!(rng.word(10))))
        }
        15 => {
            let p = rng.pick(&live.pages)?;
            Some(one("page", p, "title", json!(rng.word(10))))
        }
        // The assembly aspect: the BOM basis.
        16 => {
            let a = rng.pick(&live.assemblies)?;
            let value = *rng.pick(&["assembly", "components"])?;
            let device = a.device.clone();
            let expect = value;
            Some(
                one("device", &a.device, "assembly.bom_basis", json!(value)).check(move |_, after| {
                    let d = typed(&device).and_then(|id| after.devices().get(id).cloned());
                    let basis = d
                        .and_then(|d| d.aspects.assembly.clone())
                        .map(|a| serde_json::to_value(&a.bom_basis).unwrap_or(Value::Null));
                    (basis == Some(json!(expect)))
                        .then_some(())
                        .ok_or_else(|| format!("bom_basis is not `{expect}` afterwards: {basis:?}"))
                }),
            )
        }
        // The assembly aspect: the configs; covers within the subtree are accepted, a cover outside it that
        // the config did not hold before is refused.
        17 => {
            let a = rng.pick(&live.assemblies)?;
            let outside: Vec<&Value> = live
                .devices
                .iter()
                .filter(|d| !a.subtree.contains(d) && !a.covers.contains(&id_text(d)))
                .collect();
            let n = rng.below(3);
            let mut covers: Vec<Value> = (0..n).filter_map(|_| rng.pick(&a.subtree).cloned()).collect();
            if rng.chance(4)
                && let Some(d) = rng.pick(&outside)
            {
                covers.push((*d).clone());
            }
            covers.sort_by_key(Value::to_string);
            covers.dedup();
            let configs = json!([{"order_code": format!("FZ{}", rng.below(1000)), "covers": covers}]);
            Some(one("device", &a.device, "assembly.configs", configs))
        }
        18 => {
            let s = rng.pick(&live.stations)?;
            let value = if rng.chance(3) {
                Value::Null
            } else {
                json!(rng.below(5))
            };
            Some(one("device", s, "plc_station.order", value))
        }
        19 => {
            let s = rng.pick(&live.stations)?;
            let role = *rng.pick(&["cpu", "head_station", "io_device", "valve_terminal", "drive"])?;
            Some(one("device", s, "plc_station.role", json!(role)))
        }
        20 => {
            let m = rng.pick(&live.modules)?;
            let value = if rng.chance(2) {
                json!("positional")
            } else {
                Value::Null
            };
            Some(one("device", m, "plc_module.slot", value))
        }
        // Adds the station aspect to a plain device, or removes it again.
        21 => {
            if rng.chance(3) {
                let s = rng.pick(&live.stations)?;
                Some(one("device", s, "plc_station", Value::Null))
            } else {
                let d = rng.pick(&live.plain_devices)?;
                let cfg = rng.pick(&live.cfgs)?;
                let role = *rng.pick(&["io_device", "head_station", "drive"])?;
                Some(one(
                    "device",
                    d,
                    "plc_station",
                    json!({"config_project": cfg, "role": role}),
                ))
            }
        }
        // Adds the module aspect (instantiating its channels, N1) or removes it.
        22 => {
            if rng.chance(3) {
                let m = rng.pick(&live.modules)?;
                Some(one("device", m, "plc_module", Value::Null))
            } else {
                let d = rng.pick(&live.plain_devices)?;
                Some(one("device", d, "plc_module", json!({})))
            }
        }
        // The command refuses: a path that is not settable, or a value the field does not take.
        23 => {
            let d = rng.pick(&live.devices)?;
            let path = *rng.pick(&["id", "parent", "position", "aspects.strip", "terminal.strip"])?;
            Some(one("device", d, path, json!(rng.below(9))))
        }
        24 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(one("device", d, "tag.counter", json!("not-a-number")))
        }
        // Refused: a target that does not exist; the command refuses one listed twice.
        _ => {
            if rng.chance(2) {
                Some(one("device", &json!(rng.uuid()), "function_text", json!("X")).refuse())
            } else {
                let d = rng.pick(&live.plain_devices)?;
                Some(Step::cmd(
                    "fields.set",
                    json!({"kind": "device", "targets": [d, d], "path": "function_text", "value": "X"}),
                ))
            }
        }
    }
}

/// `edit.paste`: a fragment of one new device with its main function drawn by an embedded symbol on a page that takes
/// it, pasted at a grid point of that page (the copies are held, so nothing is embedded), with a free-text page item
/// on the source page; refused: no record, another grid, a point off the grid, a record of a kind that is not pasted,
/// a representation whose function is not in the fragment (a mandatory reference), and an `adapt` that names no held
/// copy of another version or names one twice. The boundary case pastes a wire marker that names a
/// connection outside the fragment: the marker is left out, the paste succeeds.
pub(super) fn edit_paste(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let pages = pages_taking_reps(live);
    let page = *rng.pick(&pages)?;
    let fits: Vec<&SymInfo> = live
        .symbols
        .iter()
        .filter(|s| s.fndef.is_some() && allowed(&page.kind).contains(&s.rep_type.as_str()))
        .collect();
    let symbol = *rng.pick(&fits)?;
    let (d, f, r) = (rng.uuid(), rng.uuid(), rng.uuid());
    let anchor = on_page(page, rng);
    let at = on_page(page, rng);
    let device =
        json!({"$k": "device", "id": d, "tag": {"letter": letter(rng), "counter": 1 + rng.below(9)}});
    let function = json!({"$k": "function", "id": f, "device": d, "def": symbol.fndef});
    // The representation's own type is the symbol's (the default, multi-line, is not taken by every page type).
    let rep = json!({"$k": "representation", "id": r, "functions": [f], "page": page.id, "symbol": symbol.id,
        "rep_type": symbol.rep_type, "pos": anchor});
    let item = json!({"$k": "page_item", "id": rng.uuid(), "page": page.id,
        "kind": {"free_text": {"text": "40x60", "at": anchor}}});
    let fragment = |records: Vec<Value>, grid: (u64, u64)| {
        json!({"format": "0.1", "project": live.fixture.project_id, "grid": [grid.0, grid.1],
            "page_kind": page.kind, "anchor": anchor, "records": records, "copies": [], "nodes": []})
    };
    let params = |at: &Value, fragment: Value| json!({"page": page.id, "at": at, "fragment": fragment});
    let all = || vec![device.clone(), function.clone(), rep.clone(), item.clone()];
    match rng.below(11) {
        0 => Some(Step::cmd(
            "edit.paste",
            params(&at, fragment(Vec::new(), page.grid)),
        )),
        1 => {
            let other = if page.grid == (4_000_000, 1) {
                (7_937_500, 3)
            } else {
                (4_000_000, 1)
            };
            Some(Step::cmd("edit.paste", params(&at, fragment(all(), other))))
        }
        2 => {
            let off = json!([at[0].as_i64()? + 1, at[1]]);
            Some(Step::cmd("edit.paste", params(&off, fragment(all(), page.grid))))
        }
        3 => {
            let foreign = json!({"$k": "no_such_kind", "id": rng.uuid()});
            Some(Step::cmd(
                "edit.paste",
                params(&at, fragment(vec![foreign], page.grid)),
            ))
        }
        // Refused: a representation whose function is in neither the fragment nor the project (a dangling
        // mandatory reference).
        4 => Some(
            Step::cmd(
                "edit.paste",
                params(&at, fragment(vec![device.clone(), rep.clone()], page.grid)),
            )
            .refuse(),
        ),
        5 => {
            // The marker names a connection that is in neither the fragment nor the project: the reference is
            // cleared, the marker is left out, and the paste still commits.
            let marker = json!({"$k": "page_item", "id": rng.uuid(), "page": page.id,
                "kind": {"wire_marker": {"connections": [rng.uuid()], "at": anchor, "fields": ["csa"]}}});
            Some(
                Step::cmd(
                    "edit.paste",
                    params(
                        &at,
                        fragment(
                            vec![device.clone(), function.clone(), rep.clone(), marker],
                            page.grid,
                        ),
                    ),
                )
                .counts(&[
                    (Table::Reps, 1),
                    (Table::Functions, 1),
                    (Table::Devices, 1),
                    (Table::Connections, 0),
                    (Table::PageItems, 0),
                ]),
            )
        }
        // `adapt` naming no copy of the fragment that the project holds in another version, or one
        // listed twice, is refused at `/adapt/<i>`.
        6 => {
            let copy = rng.uuid();
            let adapt = if rng.below(2) == 0 {
                json!([copy])
            } else {
                json!([copy, copy])
            };
            let mut p = params(&at, fragment(all(), page.grid));
            p["adapt"] = adapt;
            Some(Step::cmd("edit.paste", p).refuse())
        }
        _ => Some(
            Step::cmd("edit.paste", params(&at, fragment(all(), page.grid))).counts(&[
                (Table::Reps, 1),
                (Table::Functions, 1),
                (Table::Devices, 1),
                (Table::Connections, 0),
                (Table::PageItems, 1),
            ]),
        ),
    }
}

/// `fields.set` with `copies` (D16): a connection's wire part set to a part of another library carried in the command
/// (embedded in the same transaction); refused: the copy listed twice, and a part neither held nor carried.
pub(super) fn fields_set_copies(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (_, template) = rng.pick(&live.fixture.part_json)?;
    let c = relineaged(template, &rng.uuid(), &rng.uuid())?;
    let id = c["id"].clone();
    let con = rng.pick(&live.connections)?.clone();
    let params = |copies: Value| {
        json!({"kind": "connection", "targets": [con], "path": "overrides.wire_part", "value": {"copy": id},
            "copies": copies})
    };
    match rng.below(4) {
        0 => Some(Step::cmd("fields.set", params(json!([c.clone(), c])))),
        1 => Some(Step::cmd("fields.set", params(json!([])))),
        _ => {
            let (part, connection) = (id.clone(), con.clone());
            Some(
                Step::cmd("fields.set", params(json!([c])))
                    .counts(&[(Table::Parts, 1)])
                    .check(move |_, after| {
                        let cid = typed::<ew_schema::Connection>(&connection).ok_or("a connection ID")?;
                        let pid = typed::<ew_schema::Part>(&part).ok_or("a part ID")?;
                        let set = after
                            .connections()
                            .get(cid)
                            .and_then(|c| c.overrides.wire_part.as_ref().map(|p| p.copy));
                        (set == Some(pid))
                            .then_some(())
                            .ok_or_else(|| "the wire part is not the embedded copy".to_owned())
                    }),
            )
        }
    }
}

/// `fields.set_many`: one function text per device (distinct devices, one entry each), checked afterwards; refused: no
/// entry, a (target, path) pair in two entries, and an entry `fields.set` would refuse (a path that is not settable).
pub(super) fn fields_set_many(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let entry = |device: &Value, value: &str| json!({"kind": "device", "targets": [device], "path": "function_text", "value": value});
    // The command's `copies` embed; an entry's own `copies` are refused.
    if rng.chance(9) {
        let (_, template) = rng.pick(&live.fixture.part_json)?;
        let c = relineaged(template, &rng.uuid(), &rng.uuid())?;
        let con = rng.pick(&live.connections)?;
        let e = json!({"kind": "connection", "targets": [con], "path": "overrides.wire_part", "value": {"copy": c["id"]}});
        return Some(if rng.chance(2) {
            let mut bad = e;
            bad["copies"] = json!([c]);
            Step::cmd("fields.set_many", json!({"edits": [bad]}))
        } else {
            Step::cmd("fields.set_many", json!({"edits": [e], "copies": [c]})).counts(&[(Table::Parts, 1)])
        });
    }
    match rng.below(8) {
        0 => Some(Step::cmd("fields.set_many", json!({"edits": []}))),
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(Step::cmd(
                "fields.set_many",
                json!({"edits": [entry(d, "A"), entry(d, "B")]}),
            ))
        }
        2 => {
            let d = rng.pick(&live.plain_devices)?;
            let bad = json!({"kind": "device", "targets": [d], "path": "parent", "value": null});
            Some(Step::cmd(
                "fields.set_many",
                json!({"edits": [entry(d, "A"), bad]}),
            ))
        }
        _ => {
            let n = 1 + rng.below(3);
            let mut devices: Vec<Value> = (0..n)
                .filter_map(|_| rng.pick(&live.plain_devices).cloned())
                .collect();
            devices.sort_by_key(Value::to_string);
            devices.dedup();
            if devices.is_empty() {
                return None;
            }
            let texts: Vec<String> = devices.iter().map(|_| rng.word(8)).collect();
            let edits: Vec<Value> = devices.iter().zip(&texts).map(|(d, t)| entry(d, t)).collect();
            let expect: Vec<(Value, String)> = devices.into_iter().zip(texts).collect();
            Some(
                Step::cmd("fields.set_many", json!({ "edits": edits })).check(move |_, after| {
                    for (d, text) in &expect {
                        let got = typed(d)
                            .and_then(|id| after.devices().get(id).cloned())
                            .map(|dev| serde_json::to_value(&dev.function_text).unwrap_or(Value::Null));
                        if got != Some(json!(text)) {
                            return Err(format!("the function text of {d} is {got:?}, not `{text}`"));
                        }
                    }
                    Ok(())
                }),
            )
        }
    }
}

pub(super) fn edit_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(8)
        && let Some(r) = rng.pick(&live.revisions)
    {
        let target = json!({"kind": "revision", "id": r.id});
        let step = Step::cmd("edit.delete", json!({"targets": [target]}));
        // Only the open revision can be deleted: a released or withdrawn one is immutable (FR-REV-009).
        return Some(if r.state == "open" {
            step.counts(&[(Table::Revisions, -1)])
                .check(|_, after| title_block_follows(after))
        } else {
            step
        });
    }
    if rng.chance(12) {
        let target = json!({"kind": "device", "id": rng.uuid()});
        return Some(Step::cmd("edit.delete", json!({"targets": [target]})).refuse());
    }
    // WP-S4.02 part 2 (FT-840): a layout record, drawn after the older branches.
    if rng.chance(10)
        && let Some(step) = edit_delete_layout(live, rng)
    {
        return Some(step);
    }
    let (kind, id) = if rng.chance(2) {
        ("representation", rng.pick(&live.reps)?.clone())
    } else {
        ("device", rng.pick(&live.devices)?.clone())
    };
    let target = json!({"kind": kind, "id": id});
    Some(
        Step::cmd("edit.delete", json!({"targets": [target]})).check(move |_, after| {
            let gone = if kind == "device" {
                typed(&id).is_some_and(|i| !after.devices().contains(i))
            } else {
                typed(&id).is_some_and(|i| !after.reps().contains(i))
            };
            gone.then_some(())
                .ok_or_else(|| format!("the deleted {kind} still exists"))
        }),
    )
}
