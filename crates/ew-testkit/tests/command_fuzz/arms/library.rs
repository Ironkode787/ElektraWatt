// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for library references, embedded copies, parts and macros.

use super::*;

/// A library document `template` (the `lib/` JSON form with `$k`) re-lineaged as the copy of `item` of
/// `library`: library, item, copy identity `v5(NS_LIBCOPY, library ‖ item)` and hash made consistent.
pub(super) fn relineaged(template: &Value, library: &str, item: &str) -> Option<Value> {
    let mut copy = template.clone();
    copy["library_id"] = json!(library);
    copy["item_id"] = json!(item);
    let lib: ew_ids::Uuid = library.parse().ok()?;
    let it: ew_ids::Uuid = item.parse().ok()?;
    let id: ew_ids::Id<()> = ew_ids::v5::lib_copy_id(lib, it);
    copy["id"] = serde_json::to_value(id).ok()?;
    let doc = ew_schema::LibDocument::from_value(copy.clone()).ok()?;
    let hash = ew_commands::commands::library_copy_hash(&doc)?;
    copy["sha256"] = json!(format!("sha256:{}", hex(&hash)));
    Some(copy)
}

/// Whether a symbol or a part's function template of the snapshot names the FnDef copy `id` through a
/// `LibRef`.
pub(super) fn named_through_lib_ref(q: &ew_derive::Q, id: ew_ids::Id<ew_schema::FnDef>) -> bool {
    let resolves = |r: &ew_schema::LibRef| {
        ew_ids::v5::lib_copy_id::<ew_schema::FnDef>(r.library.as_uuid(), r.item.as_uuid()) == id
    };
    q.symbols().iter().any(|(_, s)| resolves(&s.item.fndef))
        || q.parts()
            .iter()
            .any(|(_, p)| p.item.function_template.iter().any(|t| resolves(&t.fndef)))
}

/// The first half of the FT-316 shape: a symbol copy with a new lineage whose function definition is a new
/// item the project does not embed. Well-formed: a `LibRef` to an item that is not embedded names nothing
/// (NFR-REL-11). A later [`lib_ref_fndef_step`] embeds that function definition.
pub fn lib_ref_symbol_step(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (_, template) = rng.pick(&live.fixture.symbol_json)?;
    let mut template = template.clone();
    template["fndef"]["library"] = json!(rng.uuid());
    template["fndef"]["item"] = json!(rng.uuid());
    let named = (
        template["fndef"]["library"]
            .as_str()?
            .parse::<ew_ids::Uuid>()
            .ok()?,
        template["fndef"]["item"].as_str()?.parse::<ew_ids::Uuid>().ok()?,
    );
    let copy = relineaged(&template, &rng.uuid(), &rng.uuid())?;
    let id = copy["id"].clone();
    Some(
        Step::cmd("library.embed", json!({"copies": [copy]}))
            .counts(&[(Table::Symbols, 1), (Table::Fndefs, 0)])
            .check(move |_, after| {
                let id = typed(&id).ok_or("a copy ID")?;
                if !after.symbols().contains(id) {
                    return Err("the embedded symbol is not in the project".to_owned());
                }
                let fndef: ew_ids::Id<ew_schema::FnDef> = ew_ids::v5::lib_copy_id(named.0, named.1);
                (!after.fndefs().contains(fndef))
                    .then_some(())
                    .ok_or_else(|| "the symbol's function definition is embedded already".to_owned())
            }),
    )
}

/// The CP indices of a function-definition copy in its JSON form.
pub(super) fn fndef_cps(copy: &Value) -> std::collections::BTreeSet<u16> {
    copy["cps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["idx"].as_u64().and_then(|i| u16::try_from(i).ok()))
        .collect()
}

/// The second half of the FT-316 shape: the function definition that a live `LibRef` names but the project
/// does not embed, embedded with that lineage (from a template that has every CP the naming symbols draw), so
/// the `LibRef` becomes a link. Undoing this embedding removes a copy that a record names, which the history
/// path must allow: the undo-all oracle (e) restores the state before it (ADR-0009 amendment 7). Without such
/// a `LibRef` the first half is built instead.
pub fn lib_ref_fndef_step(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let Some(target) = rng.pick(&live.unembedded_fndef_refs) else {
        return lib_ref_symbol_step(live, rng);
    };
    let fitting: Vec<&Value> = live
        .fixture
        .fndef_json
        .iter()
        .map(|(_, t)| t)
        .filter(|t| fndef_cps(t).is_superset(&target.fn_cps))
        .collect();
    let template = *rng.pick(&fitting)?;
    let copy = relineaged(template, target.library.as_str()?, target.item.as_str()?)?;
    let id = copy["id"].clone();
    Some(
        Step::cmd("library.embed", json!({"copies": [copy]}))
            .counts(&[(Table::Fndefs, 1)])
            .check(move |_, after| {
                let id = typed(&id).ok_or("a copy ID")?;
                if !after.fndefs().contains(id) {
                    return Err("the embedded function definition is not in the project".to_owned());
                }
                named_through_lib_ref(after, id)
                    .then_some(())
                    .ok_or_else(|| "no LibRef names the embedded function definition".to_owned())
            }),
    )
}

pub(super) fn library_embed(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (_, template) = rng.pick(&live.fixture.form_json)?;
    // A copy of the form with a new lineage: item, library, copy identity and hash made consistent.
    let consistent =
        |rng: &mut SplitMix64| -> Option<Value> { relineaged(template, &rng.uuid(), &rng.uuid()) };
    match rng.below(12) {
        // The command refuses: the hash is not the hash of the item's members (EW-CORE-014).
        0 => {
            let mut c = consistent(rng)?;
            c["sha256"] = json!(format!("sha256:{}", "00".repeat(32)));
            Some(Step::cmd("library.embed", json!({"copies": [c]})))
        }
        // The command refuses: the copy ID is not the copy identity.
        1 => {
            let mut c = consistent(rng)?;
            c["id"] = json!(rng.uuid());
            Some(Step::cmd("library.embed", json!({"copies": [c]})))
        }
        // The command refuses: an item of the project itself is created, not embedded.
        2 => {
            let mut c = consistent(rng)?;
            c["library_id"] = live.fixture.project_id.clone();
            Some(Step::cmd("library.embed", json!({"copies": [c]})))
        }
        // The command refuses: an item the project has embedded already.
        3 => {
            let mut c = template.clone();
            c["$k"] = json!("form");
            Some(Step::cmd("library.embed", json!({"copies": [c]})))
        }
        // The command refuses: the same item twice, and no copies at all.
        4 => {
            let c = consistent(rng)?;
            Some(Step::cmd("library.embed", json!({"copies": [c.clone(), c]})))
        }
        5 => Some(Step::cmd("library.embed", json!({"copies": []}))),
        // The FT-316 shape: a symbol naming a function definition that is not embedded, then (on a later step)
        // that function definition; the undo-all oracle undoes both.
        6 => lib_ref_symbol_step(live, rng),
        7 => lib_ref_fndef_step(live, rng),
        _ => {
            let c = consistent(rng)?;
            let id = c["id"].clone();
            Some(
                Step::cmd("library.embed", json!({"copies": [c]}))
                    .counts(&[(Table::Forms, 1)])
                    .check(move |_, after| {
                        let id = typed(&id).ok_or("a copy ID")?;
                        after
                            .forms()
                            .contains(id)
                            .then_some(())
                            .ok_or_else(|| "the embedded copy is not in the project".to_owned())
                    }),
            )
        }
    }
}

/// `part.assign` (WP-S1.19 D8): a held part, or a part of another library embedded with the command, assigned to a
/// plain device. Refused: no devices, a device twice or unknown, a part neither held nor given, `replace` without
/// `from`, `from` without `replace`, a quantity of 0, a copy listed twice.
pub(super) fn part_assign(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (held, template) = rng.pick(&live.fixture.part_json)?;
    let device = rng.pick(&live.plain_devices)?.clone();
    let cmd = |p: Value| Step::cmd("part.assign", p);
    let uses = move |after: &ew_derive::Q, device: &Value, part: &Value| -> Result<(), String> {
        let d = typed::<ew_schema::Device>(device).ok_or("a device ID")?;
        let p = typed::<ew_schema::Part>(part).ok_or("a part ID")?;
        after
            .devices()
            .get(d)
            .is_some_and(|dev| dev.parts.iter().any(|u| u.part.copy == p))
            .then_some(())
            .ok_or_else(|| "the device does not use the assigned part".to_owned())
    };
    match rng.below(12) {
        0 => Some(cmd(json!({"devices": [], "part": held}))),
        1 => Some(cmd(json!({"devices": [device, device], "part": held}))),
        2 => Some(cmd(json!({"devices": [rng.uuid()], "part": held})).refuse()),
        3 => Some(cmd(json!({"devices": [device], "part": rng.uuid()})).refuse()),
        4 => Some(cmd(json!({"devices": [device], "part": held, "mode": "replace"}))),
        5 => Some(cmd(json!({"devices": [device], "part": held, "from": held}))),
        6 => Some(cmd(json!({"devices": [device], "part": held, "qty": "0"}))),
        7 => {
            let c = relineaged(template, &rng.uuid(), &rng.uuid())?;
            let id = c["id"].clone();
            Some(cmd(
                json!({"devices": [device], "part": id, "copies": [c.clone(), c]}),
            ))
        }
        // Either: the device may or may not use the part it replaces.
        8 => {
            let (part, d) = (held.clone(), device.clone());
            Some(
                cmd(json!({"devices": [device], "part": held, "mode": "replace", "from": held}))
                    .check(move |_, after| uses(after, &d, &part)),
            )
        }
        // A part of another library, embedded with the command.
        9 | 10 => {
            let c = relineaged(template, &rng.uuid(), &rng.uuid())?;
            let id = c["id"].clone();
            let d = device.clone();
            Some(
                cmd(json!({"devices": [device], "part": id, "copies": [c], "mode": "replace_all", "qty": "2"}))
                    .counts(&[(Table::Parts, 1)])
                    .check(move |_, after| uses(after, &d, &id)),
            )
        }
        _ => {
            let (part, d) = (held.clone(), device.clone());
            Some(
                cmd(json!({"devices": [device], "part": held}))
                    .counts(&[(Table::Parts, 0)])
                    .check(move |_, after| uses(after, &d, &part)),
            )
        }
    }
}

// WP-S2.07 part 4a
/// `part.edit_copy` (Bulk, D20, D21): a held project-local part copy (Golden A's are) edited in place with a new
/// description. Refused: a copy the project does not hold, a fork for a project-local copy, a changed CP pattern, an
/// empty part number, the unit `pcs`, a technical key twice, and the unchanged item.
pub(super) fn part_edit_copy(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (held, template) = rng.pick(&live.fixture.part_json)?;
    let project = live.fixture.project_id.as_str()?;
    if template["library_id"].as_str()? != project {
        return None;
    }
    let mut item = template.as_object()?.clone();
    for k in ["$k", "$v", "id", "library_id", "item_id", "version", "sha256"] {
        item.remove(k);
    }
    let item = Value::Object(item);
    let with = |k: &str, v: Value| {
        let mut p = item.clone();
        p[k] = v;
        p
    };
    let cmd = |p: Value| Step::cmd("part.edit_copy", p);
    match rng.below(10) {
        0 => Some(cmd(json!({"copy": rng.uuid(), "part": item})).refuse()),
        1 => Some(cmd(
            json!({"copy": held, "part": with("description", json!("x")), "fork": rng.uuid()}),
        )),
        2 => Some(cmd(
            json!({"copy": held, "part": with("cp_pattern", json!([{"idx": 99, "designation": "99"}]))}),
        )),
        3 => Some(cmd(
            json!({"copy": held, "part": with("part_number", json!(" "))}),
        )),
        4 => Some(cmd(json!({"copy": held, "part": with("unit", json!("pcs"))}))),
        5 => Some(cmd(
            json!({"copy": held, "part": with("tech_data", json!([{"key": "k", "value": "A"}, {"key": "k", "value": "B"}]))}),
        )),
        // Either: an earlier step may have edited the copy, so its own template is no longer the held item.
        6 => Some(cmd(json!({"copy": held, "part": item}))),
        _ => {
            let description = format!("fuzz {}", &rng.uuid()[..8]);
            let (id, text) = (held.clone(), description.clone());
            Some(
                cmd(json!({"copy": held, "part": with("description", json!(description))}))
                    .counts(&[(Table::Parts, 0)])
                    .check(move |_, after| {
                        let id = typed::<ew_schema::Part>(&id).ok_or("a copy ID")?;
                        let copy = after.parts().get(id).ok_or("the edited copy is gone")?;
                        (copy.item.description.as_neutral() == Some(text.as_str()))
                            .then_some(())
                            .ok_or_else(|| "the copy does not carry the new description".to_owned())
                    }),
            )
        }
    }
}

// WP-S1.19 part 5
/// `macro.insert` (D12, D13): a window macro of one device with its main function drawn by an embedded symbol,
/// carried in the command and inserted at a grid point of a page that takes it; the new device's ID is
/// v5(instance, local). Refused: the macro not carried, no anchor, an unknown value set, a fragment of another grid,
/// a point off the grid, a macro whose hash is not its item's.
pub(super) fn macro_insert(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
    let rep = json!({"$k": "representation", "id": r, "functions": [f], "page": page.id, "symbol": symbol.id,
        "rep_type": symbol.rep_type, "pos": anchor});
    let (library, item) = (rng.uuid(), rng.uuid());
    let doc = |grid: (u64, u64)| -> Option<Value> {
        let fragment = json!({"format": "0.1", "project": live.fixture.project_id, "grid": [grid.0, grid.1],
            "page_kind": page.kind, "anchor": anchor, "records": [device, function, rep], "copies": [],
            "nodes": []});
        let template = json!({"$k": "macro", "id": item, "library_id": library, "item_id": item,
            "version": "1.0.0", "sha256": format!("sha256:{}", "00".repeat(32)), "name": "fuzz", "kind": "window",
            "variants": [{"rep_type": symbol.rep_type, "key": "A", "fragment": fragment}],
            "value_sets": [{"name": "one"}]});
        relineaged(&template, &library, &item)
    };
    let good = doc(page.grid)?;
    let id = good["id"].clone();
    let instance = rng.uuid();
    // FT-672: a window macro turns about its anchor by any of the eight orientations
    let orient = *rng.pick(&["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"])?;
    let params = |copies: Value| json!({"copies": copies, "macro": id, "page": page.id, "at": at, "orient": orient, "instance": instance});
    let cmd = |p: Value| Step::cmd("macro.insert", p);
    match rng.below(10) {
        0 => Some(cmd(params(json!([])))),
        1 => {
            let mut p = params(json!([good]));
            p.as_object_mut()?.remove("at");
            Some(cmd(p))
        }
        2 => {
            let mut p = params(json!([good]));
            p["value_set"] = json!("none");
            Some(cmd(p))
        }
        3 => {
            let other = if page.grid == (4_000_000, 1) {
                (7_937_500, 3)
            } else {
                (4_000_000, 1)
            };
            let mut p = params(json!([doc(other)?]));
            p["macro"] = p["copies"][0]["id"].clone();
            Some(cmd(p))
        }
        4 => {
            let mut p = params(json!([good]));
            p["at"] = json!([at[0].as_i64()? + 1, at[1]]);
            Some(cmd(p))
        }
        5 => {
            let mut c = good.clone();
            c["sha256"] = json!(format!("sha256:{}", "00".repeat(32)));
            Some(cmd(params(json!([c]))))
        }
        _ => {
            let local: ew_ids::Uuid = d.parse().ok()?;
            let inst: ew_ids::Uuid = instance.parse().ok()?;
            let new = ew_ids::Uuid::new_v5(&inst, local.hyphenated().to_string().as_bytes());
            Some(
                cmd(params(json!([good])))
                    .counts(&[(Table::Devices, 1), (Table::Functions, 1), (Table::Reps, 1)])
                    .check(move |_, after| {
                        after
                            .devices()
                            .contains(ew_ids::Id::from_uuid(new))
                            .then_some(())
                            .ok_or_else(|| "the device is not v5(instance, local)".to_owned())
                    }),
            )
        }
    }
}

/// `library.update` (Bulk, WP-S1.19 D9): a held part copy replaced by another version with another description.
/// Refused: no copies, a copy the project does not hold, a version that is not semantic, a copy twice, a mapping for a
/// copy not given, a CP mapping that is not injective (an old CP twice, or two old CPs onto one new), a version that
/// names a manufacturer the project does not hold without embedding it (FT-640). The version is drawn at random, so
/// an update may be a downgrade of an earlier one: either.
pub(super) fn library_update(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (held, template) = rng.pick(&live.fixture.part_json)?;
    let library = template["library_id"].as_str()?.to_owned();
    let item = template["item_id"].as_str()?.to_owned();
    let cmd = |p: Value| Step::cmd("library.update", p);
    let next = |rng: &mut SplitMix64, version: String| -> Option<Value> {
        let mut t = template.clone();
        t["version"] = json!(version);
        t["description"] = json!(format!("fuzz {}", &rng.uuid()[..8]));
        relineaged(&t, &library, &item)
    };
    match rng.below(10) {
        0 => Some(cmd(json!({"copies": []}))),
        1 => {
            let c = relineaged(template, &rng.uuid(), &rng.uuid())?;
            Some(cmd(json!({"copies": [c]})))
        }
        2 => {
            let c = next(rng, "one".to_owned())?;
            Some(cmd(json!({"copies": [c]})))
        }
        3 => {
            let c = next(rng, "9.0.0".to_owned())?;
            Some(cmd(json!({"copies": [c.clone(), c]})))
        }
        4 => {
            let c = next(rng, "9.0.0".to_owned())?;
            Some(cmd(json!({"copies": [c], "cp_maps": [{"copy": rng.uuid(), "map": []}]})).refuse())
        }
        5 => {
            let idx: Vec<u64> = template["cp_pattern"]
                .as_array()?
                .iter()
                .filter_map(|c| c["idx"].as_u64())
                .collect();
            let (a, b) = (*idx.first()?, *idx.get(1)?);
            let c = next(rng, "9.0.0".to_owned())?;
            let map = if rng.below(2) == 0 {
                json!([[a, a], [b, a]])
            } else {
                json!([[a, a], [a, b]])
            };
            Some(cmd(
                json!({"copies": [c], "cp_maps": [{"copy": held, "map": map}]}),
            ))
        }
        // Refused (FT-640): the new version names a manufacturer that is a fresh UUID, which the project does not
        // hold and the command does not embed (a dangling reference).
        6 => {
            let mut t = template.clone();
            t["version"] = json!("9.0.0");
            t["manufacturer"]["item"] = json!(rng.uuid());
            let c = relineaged(&t, &library, &item)?;
            Some(cmd(json!({"copies": [c]})).refuse())
        }
        _ => {
            let minor = rng.below(50);
            let c = next(rng, format!("1.{minor}.0"))?;
            let description = c["description"].clone();
            let id = held.clone();
            Some(
                cmd(json!({"copies": [c]}))
                    .counts(&[(Table::Parts, 0)])
                    .check(move |_, after| {
                        let id = typed::<ew_schema::Part>(&id).ok_or("a copy ID")?;
                        let copy = after.parts().get(id).ok_or("the updated copy is gone")?;
                        (json!(copy.item.description) == description)
                            .then_some(())
                            .ok_or_else(|| "the copy does not carry the new description".to_owned())
                    }),
            )
        }
    }
}

/// `parts.import` (Bulk): project-local part copies. A copy with a new key is inserted, one whose key the
/// project has is skipped; another library, a wrong identity or hash, a duplicate, no copies and a non-part
/// document are refused.
pub(super) fn parts_import(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (_, template) = rng.pick(&live.fixture.part_json)?;
    let project = live.fixture.project_id.as_str()?.to_owned();
    // A project-local copy with a fresh key and a fresh item, identity and hash made consistent.
    let fresh = |rng: &mut SplitMix64| -> Option<Value> {
        let mut t = template.clone();
        t["meta"]["key"] = json!(format!("fuzz.{}", &rng.uuid().replace('-', "")[..12]));
        relineaged(&t, &project, &rng.uuid())
    };
    match rng.below(10) {
        // The command refuses: a copy of another library (that is library.embed's).
        0 => {
            let mut t = template.clone();
            t["meta"]["key"] = json!(format!("fuzz.{}", &rng.uuid().replace('-', "")[..12]));
            let c = relineaged(&t, &rng.uuid(), &rng.uuid())?;
            Some(Step::cmd("parts.import", json!({"copies": [c]})))
        }
        // The command refuses: the copy ID is not the copy identity.
        1 => {
            let mut c = fresh(rng)?;
            c["id"] = json!(rng.uuid());
            Some(Step::cmd("parts.import", json!({"copies": [c]})))
        }
        // The command refuses: the hash is not the hash of the item's members.
        2 => {
            let mut c = fresh(rng)?;
            c["sha256"] = json!(format!("sha256:{}", "00".repeat(32)));
            Some(Step::cmd("parts.import", json!({"copies": [c]})))
        }
        // The command refuses: the same copy twice, and no copies at all.
        3 => {
            let c = fresh(rng)?;
            Some(Step::cmd("parts.import", json!({"copies": [c.clone(), c]})))
        }
        4 => Some(Step::cmd("parts.import", json!({"copies": []}))),
        // The command refuses: not a part document.
        5 => {
            let (_, form) = rng.pick(&live.fixture.form_json)?;
            Some(Step::cmd("parts.import", json!({"copies": [form.clone()]})))
        }
        // Accepted, nothing inserted: the key exists in the project already.
        6 => Some(
            Step::cmd("parts.import", json!({"copies": [template.clone()]})).counts(&[(Table::Parts, 0)]),
        ),
        _ => {
            let c = fresh(rng)?;
            let id = c["id"].clone();
            Some(
                Step::cmd("parts.import", json!({"copies": [c]}))
                    .counts(&[(Table::Parts, 1)])
                    .check(move |_, after| {
                        let id = typed(&id).ok_or("a copy ID")?;
                        after
                            .parts()
                            .contains(id)
                            .then_some(())
                            .ok_or_else(|| "the imported part is not in the project".to_owned())
                    }),
            )
        }
    }
}
