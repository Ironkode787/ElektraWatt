// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for devices and their parts: placement, terminals, cables, bonds, assemblies, valves, waivers.

use super::*;

pub(super) fn terminal_designate(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(10) {
        // The command refuses: a terminal listed twice.
        0 => {
            let strip = rng.pick(&live.strips)?;
            Some(Step::cmd(
                "terminal.designate",
                json!({"assignments": [
                    {"terminal": strip[0].0, "designation": "Z1"},
                    {"terminal": strip[0].0, "designation": "Z2"},
                ]}),
            ))
        }
        // The command refuses: a device that is not a terminal.
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(Step::cmd(
                "terminal.designate",
                json!({"assignments": [{"terminal": d, "designation": "Z1"}]}),
            ))
        }
        // Refused: nothing to assign.
        2 => Some(Step::cmd("terminal.designate", json!({"assignments": []})).refuse()),
        // Swap two designations of one strip (the swap passes the prevent rule; a re-designation may not).
        _ => {
            let strip = rng.pick(&live.strips)?;
            let (a, b) = pick2(strip, rng)?;
            let (ta, tb, da, db) = (a.0.clone(), b.0.clone(), a.1.clone(), b.1.clone());
            Some(
                Step::cmd(
                    "terminal.designate",
                    json!({"assignments": [
                        {"terminal": a.0, "designation": b.1},
                        {"terminal": b.0, "designation": a.1},
                    ]}),
                )
                .check(move |_, after| {
                    let designation = |t: &Value| -> Option<Value> {
                        let d = typed(t).and_then(|i| after.devices().get(i).cloned())?;
                        d.aspects.terminal.as_ref().map(|t| json!(t.designation.as_str()))
                    };
                    // The two designations were exchanged.
                    if designation(&ta) == Some(db.clone()) && designation(&tb) == Some(da.clone()) {
                        Ok(())
                    } else {
                        Err(format!(
                            "the designations were not exchanged: {:?} and {:?}, expected {db} and {da}",
                            designation(&ta),
                            designation(&tb)
                        ))
                    }
                }),
            )
        }
    }
}

pub(super) fn pages_taking_reps(live: &Live) -> Vec<&PageInfo> {
    live.page_info
        .iter()
        .filter(|p| !allowed(&p.kind).is_empty())
        .collect()
}

/// A position on the grid of `page`: `(t1, t2)` grid units of `num` nm, inside the page area.
pub(super) fn on_page(page: &PageInfo, rng: &mut SplitMix64) -> Value {
    Live::grid_pos(page, rng.range(2, 38), rng.range(2, 24))
}

/// A part copy whose function-template entries are all embedded and that has exactly one main entry (`one_main`; or
/// does not), with one of its entries that is a black box (or not) and that entry's function-definition copy.
pub(super) fn template_entry<'a>(
    live: &'a Live,
    rng: &mut SplitMix64,
    black: bool,
    one_main: bool,
) -> Option<(&'a PartTemplate, usize, &'a Value)> {
    let candidates: Vec<(&PartTemplate, usize)> = live
        .part_templates
        .iter()
        .filter(|t| t.entries.iter().all(Option::is_some) && (t.mains == 1) == one_main)
        .flat_map(|t| {
            t.entries
                .iter()
                .enumerate()
                .filter(move |(_, e)| e.as_ref().is_some_and(|(_, bb)| *bb == black))
                .map(move |(i, _)| (t, i))
        })
        .collect();
    let (t, i) = *rng.pick(&candidates)?;
    let (fndef, _) = t.entries[i].as_ref()?;
    Some((t, i, fndef))
}

/// A designation that no terminal of the strip has.
pub(super) fn fresh_designation(strip: &StripInfo, rng: &mut SplitMix64) -> String {
    loop {
        let d = format!("Z{}", rng.below(10_000));
        if !strip.terminals.iter().any(|(_, t)| *t == d) {
            break d;
        }
    }
}

/// The `from_part` target of a part copy's entry.
pub(super) fn from_part_target(part: &Value, template: usize, letter: &str) -> Value {
    json!({"from_part": {"part": part, "template": template, "letter": letter}})
}

/// FT-656: the held part copy `part` as an item of another library (a new copy with a consistent lineage and hash),
/// for a placement that embeds it through `copies`; `None` when the fixture has no JSON of the part.
pub(super) fn part_of_another_library(live: &Live, part: &Value, rng: &mut SplitMix64) -> Option<Value> {
    let (_, template) = live.fixture.part_json.iter().find(|(id, _)| id == part)?;
    relineaged(template, &rng.uuid(), &rng.uuid())
}

/// FT-656: the held part copy `part` at another hash (its part number changed, the hash made consistent, the lineage
/// kept), which a placement's `copies` must refuse ("update from library first"); `None` without the fixture's JSON.
pub(super) fn part_at_another_hash(live: &Live, part: &Value) -> Option<Value> {
    let (_, template) = live.fixture.part_json.iter().find(|(id, _)| id == part)?;
    let mut changed = template.clone();
    let number = changed["part_number"].as_str()?.to_owned();
    changed["part_number"] = json!(format!("{number}-FT656"));
    relineaged(
        &changed,
        template["library_id"].as_str()?,
        template["item_id"].as_str()?,
    )
}

pub(super) fn device_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let pages = pages_taking_reps(live);
    let page = *rng.pick(&pages)?;
    let fits: Vec<&SymInfo> = live
        .symbols
        .iter()
        .filter(|s| allowed(&page.kind).contains(&s.rep_type.as_str()))
        .collect();
    let symbol = *rng.pick(&fits)?;
    let pos = on_page(page, rng);
    let base = |target: Value| json!({"page": page.id, "symbol": symbol.id, "pos": pos, "target": target});
    match rng.below(26) {
        // A new device made from a part copy (FR-UI-017): a function per template entry, the placed entry drawn.
        20 => {
            let (t, i, fndef) = template_entry(live, rng, false, true)?;
            let same: Vec<&SymInfo> = fits
                .iter()
                .copied()
                .filter(|s| s.fndef.as_ref() == Some(fndef))
                .collect();
            let symbol = *rng.pick(&same)?;
            let mut p = base(from_part_target(&t.part, i, letter(rng)));
            p["symbol"] = symbol.id.clone();
            let entries = isize::try_from(t.entries.len()).unwrap_or(1);
            // FT-656: one in three placements brings its part as another library's item in `copies`, embedded in the
            // same transaction (one more part copy).
            let mut parts = 0;
            if rng.chance(3)
                && let Some(c) = part_of_another_library(live, &t.part, rng)
            {
                p["target"] = from_part_target(&c["id"], i, letter(rng));
                p["copies"] = json!([c]);
                parts = 1;
            }
            Some(Step::cmd("device.place", p).counts(&[
                (Table::Reps, 1),
                (Table::Functions, entries),
                (Table::Devices, 1),
                (Table::Connections, 0),
                (Table::Parts, parts),
            ]))
        }
        // A `from_part` target that cannot be made: the command refuses it; refused: a part copy that does not exist.
        21 => {
            let target = match rng.below(8) {
                // FT-656: `copies` holding a held part at another hash ("update from library first"), or the same copy
                // twice.
                6 | 7 => {
                    let (t, i, _) = template_entry(live, rng, false, true)?;
                    let copies = if rng.chance(2) {
                        json!([part_at_another_hash(live, &t.part)?])
                    } else {
                        let c = part_of_another_library(live, &t.part, rng)?;
                        json!([c.clone(), c])
                    };
                    let mut p = base(from_part_target(&t.part, i, "K"));
                    p["copies"] = copies;
                    return Some(Step::cmd("device.place", p));
                }
                // Refused: the part copy does not exist.
                0 => {
                    let p = base(from_part_target(&json!(rng.uuid()), 0, "K"));
                    return Some(Step::cmd("device.place", p).refuse());
                }
                // A template index out of range.
                1 => {
                    let t = rng.pick(&live.part_templates)?;
                    from_part_target(&t.part, t.entries.len() + rng.below(3), "K")
                }
                // A black-box entry belongs to `box.black`.
                2 => {
                    let (t, i, _) = template_entry(live, rng, true, true)?;
                    from_part_target(&t.part, i, "K")
                }
                // The symbol draws another function definition than the entry's.
                3 => {
                    let (t, i, fndef) = template_entry(live, rng, false, true)?;
                    let other: Vec<&SymInfo> = fits
                        .iter()
                        .copied()
                        .filter(|s| s.fndef.as_ref() != Some(fndef))
                        .collect();
                    let other = *rng.pick(&other)?;
                    let mut p = base(from_part_target(&t.part, i, "K"));
                    p["symbol"] = other.id.clone();
                    return Some(Step::cmd("device.place", p));
                }
                // A template entry whose function definition the project does not embed.
                4 => {
                    let t = live
                        .part_templates
                        .iter()
                        .find(|t| t.entries.iter().any(Option::is_none))?;
                    from_part_target(&t.part, 0, "K")
                }
                // A template without exactly one main entry: the device would not have exactly one main function
                // (EW-INV-002), so the invariant refuses it.
                _ => {
                    let (t, i, _) = template_entry(live, rng, false, false)?;
                    from_part_target(&t.part, i, "K")
                }
            };
            Some(Step::cmd("device.place", base(target)))
        }
        // A new terminal of a strip (AC-28), drawn with a terminal symbol.
        22 | 23 => {
            let strip = rng.pick(&live.strip_infos)?;
            let terminal = live.term_fndef.as_ref()?;
            let same: Vec<&SymInfo> = fits
                .iter()
                .copied()
                .filter(|s| s.fndef.as_ref() == Some(terminal))
                .collect();
            let symbol = *rng.pick(&same)?;
            let designation = fresh_designation(strip, rng);
            let mut target = json!({"strip": strip.id, "designation": designation});
            let mut seq = None;
            if rng.chance(3) {
                let highest = strip.seqs.iter().next_back().copied().unwrap_or(0);
                let next = highest + 1 + u32::try_from(rng.below(4)).unwrap_or(0);
                target["seq"] = json!(next);
                seq = Some(next);
            }
            let mut p = base(json!({"new_terminal": target}));
            p["symbol"] = symbol.id.clone();
            let strip_id = strip.id.clone();
            Some(
                Step::cmd("device.place", p)
                    .counts(&[
                        (Table::Reps, 1),
                        (Table::Functions, 1),
                        (Table::Devices, 1),
                        (Table::Connections, 0),
                    ])
                    .check(move |before, after| {
                        let strip = typed(&strip_id).ok_or("a strip ID")?;
                        let have = |q: &ew_derive::Q| -> std::collections::BTreeMap<String, u32> {
                            q.idx()
                                .terminals_of_strip(strip)
                                .filter_map(|t| q.devices().get(t).and_then(|d| d.aspects.terminal.clone()))
                                .map(|t| (t.designation.as_str().to_owned(), t.seq))
                                .collect()
                        };
                        let (b, a) = (have(before), have(after));
                        let new = a
                            .get(&designation)
                            .ok_or("the new terminal is not in the strip")?;
                        let want = seq.unwrap_or(b.values().max().copied().unwrap_or(0) + 1);
                        (*new == want && a.len() == b.len() + 1)
                            .then_some(())
                            .ok_or_else(|| {
                                format!("the new terminal has sequence number {new}, expected {want}")
                            })
                    }),
            )
        }
        // The command refuses: a `new_terminal` target that cannot be made.
        24 | 25 => {
            let strip = rng.pick(&live.strip_infos)?;
            let target = match rng.below(3) {
                // A designation the strip has.
                0 => {
                    let (_, d) = rng.pick(&strip.terminals)?;
                    json!({"strip": strip.id, "designation": d})
                }
                // Not a strip.
                1 => {
                    let d = rng.pick(&live.plain_devices)?;
                    json!({"strip": d, "designation": "Z1"})
                }
                // A sequence number the strip has.
                _ => {
                    let seqs: Vec<u32> = strip.seqs.iter().copied().collect();
                    let seq = *rng.pick(&seqs)?;
                    json!({"strip": strip.id, "designation": fresh_designation(strip, rng), "seq": seq})
                }
            };
            Some(Step::cmd("device.place", base(json!({"new_terminal": target}))))
        }
        // Refused: the page does not exist.
        18 => {
            let mut p = base(json!({"new_device": {"letter": "K"}}));
            p["page"] = json!(rng.uuid());
            Some(Step::cmd("device.place", p).refuse())
        }
        // The command refuses: the position is off the connection grid.
        17 => {
            let mut p = base(json!({"new_device": {"letter": "K"}}));
            let n = i64::try_from(page.grid.0).unwrap_or(1);
            p["pos"] = json!([3 * n + 1, 4 * n]);
            Some(Step::cmd("device.place", p))
        }
        // The command refuses: a representation type the page type does not allow.
        19 => {
            let wrong: Vec<_> = live
                .symbols
                .iter()
                .filter(|s| !allowed(&page.kind).contains(&s.rep_type.as_str()))
                .collect();
            let w = *rng.pick(&wrong)?;
            let mut p = base(json!({"new_device": {"letter": "K"}}));
            p["symbol"] = w.id.clone();
            Some(Step::cmd("device.place", p))
        }
        // Another representation of an existing function.
        14 | 15 => {
            let candidates: Vec<_> = live.functions.iter().filter(|f| !f.ip).collect();
            let f = rng.pick(&candidates)?;
            Some(
                Step::cmd(
                    "device.place",
                    base(json!({"existing_function": {"function": f.id}})),
                )
                .counts(&[(Table::Reps, 1), (Table::Functions, 0), (Table::Devices, 0)]),
            )
        }
        // A new auxiliary function of an existing device.
        16 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(
                Step::cmd(
                    "device.place",
                    base(json!({"join_device": {"device": d, "fndef": symbol.fndef, "role": "aux"}})),
                )
                .counts(&[(Table::Reps, 1), (Table::Functions, 1), (Table::Devices, 0)]),
            )
        }
        // A new device with its main function, sometimes with an autoconnect hint.
        _ => {
            symbol.fndef.as_ref()?;
            let mut p = base(json!({"new_device": {"letter": letter(rng)}}));
            let mut connections = 0;
            if rng.chance(6)
                && let (Some(cp), Some((_, functions))) =
                    (rng.pick(&symbol.cps), rng.pick(&live.page_functions))
            {
                let f = rng.pick(functions)?;
                p["connect"] = json!([{"cp": cp, "to": cp_end(f, 0)}]);
                connections = 1;
            }
            let step = Step::cmd("device.place", p).counts(&[
                (Table::Reps, 1),
                (Table::Functions, 1),
                (Table::Devices, 1),
                (Table::Connections, connections),
            ]);
            // A hint wires to a function on the page, which the model may refuse for its own reasons.
            Some(step)
        }
    }
}

pub(super) fn part_use(parts: &[Value], rng: &mut SplitMix64) -> Option<Value> {
    let part = rng.pick(parts)?;
    Some(json!({"part": {"copy": part}, "qty": "1"}))
}

/// The FnDef copies of the template entries of the part copy `part` with their contact kinds (`None`: not embedded).
pub(super) fn template_kinds(
    q: &ew_derive::Q,
    part: &ew_schema::LibCopy<ew_schema::Part>,
) -> Vec<(ew_ids::Id<ew_schema::FnDef>, Option<ew_schema::ContactKind>)> {
    part.item
        .function_template
        .iter()
        .map(|e| {
            let def = ew_ids::v5::lib_copy_id(e.fndef.library.as_uuid(), e.fndef.item.as_uuid());
            (def, q.fndefs().get(def).map(|d| d.item.contact_kind.clone()))
        })
        .collect()
}

/// The check that the Functions table grew by a new device's functions (ID-1493): the command's own main function
/// on the FnDef copy `main`, and those normaliser N-FL-1 (`valve_functions`, WP-S1.20 D6) gives a device with a valve
/// part (a fluid section with solenoids): its valve entry's unless the main function is of contact kind valve, and
/// one per coil entry but one on the main function's FnDef. With `from_part` (`valve.add`, D7) a part with an entry
/// of contact kind valve drops the command's own main function.
pub(super) fn functions_added(
    part: Option<Value>,
    main: Option<Value>,
    from_part: bool,
) -> impl Fn(&ew_derive::Q, &ew_derive::Q) -> Result<(), String> {
    use ew_schema::ContactKind;
    move |before, after| {
        let copy = part
            .as_ref()
            .and_then(typed::<ew_schema::Part>)
            .and_then(|id| before.parts().get(id));
        let entries = copy.map(|c| template_kinds(before, c)).unwrap_or_default();
        let valve_entry = entries.iter().any(|(_, k)| *k == Some(ContactKind::Valve));
        let main = main
            .as_ref()
            .and_then(typed::<ew_schema::FnDef>)
            .filter(|_| !(from_part && valve_entry));
        let main_kind = main.and_then(|d| before.fndefs().get(d).map(|c| c.item.contact_kind.clone()));
        let mut want = isize::from(main.is_some());
        if copy.is_some_and(|c| ew_derive::valve_solenoids(&c.item).is_some()) {
            let coils = entries.iter().filter(|(_, k)| *k == Some(ContactKind::Coil));
            want += isize::from(valve_entry && main_kind != Some(ContactKind::Valve));
            want += isize::try_from(coils.clone().count()).unwrap_or(0);
            want -= isize::from(coils.clone().any(|(d, _)| Some(*d) == main));
        }
        let (b, a) = (Table::Functions.count(before), Table::Functions.count(after));
        if isize::try_from(a).unwrap_or(0) - isize::try_from(b).unwrap_or(0) == want {
            Ok(())
        } else {
            Err(format!(
                "{:?} went from {b} to {a} records, expected {want:+}",
                Table::Functions
            ))
        }
    }
}

pub(super) fn device_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let mut p = json!({"letter": letter(rng)});
    match rng.below(10) {
        // Refused: the part copy does not exist.
        0 => {
            p["parts"] = json!([{"part": {"copy": rng.uuid()}, "qty": "1"}]);
            return Some(Step::cmd("device.create", p).refuse());
        }
        // Refused: the function definition does not exist.
        1 => {
            p["main_fndef"] = json!(rng.uuid());
            return Some(Step::cmd("device.create", p).refuse());
        }
        _ => {}
    }
    if rng.chance(2) {
        p["parts"] = json!([part_use(&live.parts, rng)?]);
    }
    if rng.chance(2) {
        p["main_fndef"] = rng.pick(&live.fndefs)?.id.clone();
    }
    if rng.chance(5) {
        p["strip"] = json!({});
    }
    if rng.chance(4) {
        p["function_text"] = json!(rng.word(8));
    }
    let part = p.get("parts").map(|u| u[0]["part"]["copy"].clone());
    let main = p.get("main_fndef").cloned();
    Some(
        Step::cmd("device.create", p)
            .counts(&[(Table::Devices, 1)])
            .check(functions_added(part, main, false)),
    )
}

pub(super) fn terminal_add(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let fndef = live
        .term_fndef
        .as_ref()
        .or_else(|| live.fndefs.first().map(|f| &f.id))?;
    let strip = rng.pick(&live.strip_infos)?;
    let taken: std::collections::BTreeSet<&str> = strip.terminals.iter().map(|(_, d)| d.as_str()).collect();
    let fresh = |rng: &mut SplitMix64| loop {
        let d = format!("Z{}", rng.below(10_000));
        if !taken.contains(d.as_str()) {
            break d;
        }
    };
    match rng.below(10) {
        // The command refuses: a designation the strip has.
        0 => {
            let (_, d) = rng.pick(&strip.terminals)?;
            Some(Step::cmd(
                "terminal.add",
                json!({"strip": strip.id, "fndef": fndef, "terminals": [{"designation": d}]}),
            ))
        }
        // The command refuses: not a strip.
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(Step::cmd(
                "terminal.add",
                json!({"strip": d, "fndef": fndef, "terminals": [{"designation": "Z1"}]}),
            ))
        }
        // The command refuses: nothing to add.
        2 => Some(Step::cmd(
            "terminal.add",
            json!({"strip": strip.id, "fndef": fndef, "terminals": []}),
        )),
        // The command refuses: a sequence number the strip has.
        3 => {
            let seq = rng
                .pick(&strip.seqs.iter().copied().collect::<Vec<_>>())?
                .to_owned();
            Some(Step::cmd(
                "terminal.add",
                json!({"strip": strip.id, "fndef": fndef, "terminals": [{"designation": fresh(rng), "seq": seq}]}),
            ))
        }
        _ => {
            let n = 1 + rng.below(3);
            let mut names: Vec<String> = Vec::new();
            while names.len() < n {
                let d = fresh(rng);
                if !names.contains(&d) {
                    names.push(d);
                }
            }
            let terminals: Vec<Value> = names
                .iter()
                .map(
                    |d| json!({"designation": d, "function_text": rng.word(4), "rail_bonded": rng.chance(4)}),
                )
                .collect();
            let n = isize::try_from(n).unwrap_or(1);
            let (strip_id, designations) = (strip.id.clone(), names.clone());
            Some(
                Step::cmd(
                    "terminal.add",
                    json!({"strip": strip.id, "fndef": fndef, "terminals": terminals}),
                )
                .counts(&[(Table::Devices, n), (Table::Functions, n)])
                .check(move |_, after| {
                    let strip = typed(&strip_id).ok_or("a strip ID")?;
                    let have: std::collections::BTreeSet<String> = after
                        .idx()
                        .terminals_of_strip(strip)
                        .filter_map(|t| after.devices().get(t).and_then(|d| d.aspects.terminal.clone()))
                        .map(|t| t.designation.as_str().to_owned())
                        .collect();
                    designations
                        .iter()
                        .all(|d| have.contains(d))
                        .then_some(())
                        .ok_or_else(|| "a new terminal is not in the strip afterwards".to_owned())
                }),
            )
        }
    }
}

/// The strip's terminals in mounting order ((seq, ID) order) after a step, as JSON IDs.
pub(super) fn mounting_order(after: &ew_derive::Q, strip: &Value) -> Result<Vec<(u32, Value)>, String> {
    let strip = typed::<ew_schema::Device>(strip).ok_or("a strip ID")?;
    let mut order: Vec<(u32, ew_ids::Id<ew_schema::Device>)> = after
        .idx()
        .terminals_of_strip(strip)
        .filter_map(|t| {
            after
                .devices()
                .get(t)
                .and_then(|d| d.aspects.terminal.as_ref().map(|a| (a.seq, t)))
        })
        .collect();
    order.sort();
    Ok(order.into_iter().map(|(seq, t)| (seq, id_json(t))).collect())
}

// WP-S2.07 part 1
pub(super) fn terminal_insert(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let fndef = live
        .term_fndef
        .as_ref()
        .or_else(|| live.fndefs.first().map(|f| &f.id))?;
    let strip = rng.pick(&live.strip_infos)?;
    let taken: std::collections::BTreeSet<&str> = strip.terminals.iter().map(|(_, d)| d.as_str()).collect();
    let fresh = |rng: &mut SplitMix64| loop {
        let d = format!("Z{}", rng.below(10_000));
        if !taken.contains(d.as_str()) {
            break d;
        }
    };
    let cmd = |p: Value| Step::cmd("terminal.insert", p);
    match rng.below(12) {
        // The command refuses: a designation the strip has.
        0 => {
            let (_, d) = rng.pick(&strip.terminals)?;
            Some(cmd(
                json!({"strip": strip.id, "fndef": fndef, "terminals": [{"designation": d}]}),
            ))
        }
        // The command refuses: not a strip.
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(cmd(
                json!({"strip": d, "fndef": fndef, "terminals": [{"designation": "Z1"}]}),
            ))
        }
        // Refused: nothing to insert.
        2 => Some(cmd(json!({"strip": strip.id, "fndef": fndef, "terminals": []})).refuse()),
        // The command refuses: a spec with `seq` (the position follows from `before`).
        3 => Some(cmd(json!({"strip": strip.id, "fndef": fndef,
                "terminals": [{"designation": fresh(rng), "seq": 1 + rng.below(30)}]}))),
        // The command refuses: the same designation twice in the list.
        4 => {
            let d = fresh(rng);
            Some(cmd(json!({"strip": strip.id, "fndef": fndef,
                    "terminals": [{"designation": d}, {"designation": d}]})))
        }
        // `before` is not a terminal of the strip: a plain device (the command refuses it), or refused: a device that
        // does not exist.
        5 => {
            let (before, dangling) = match rng.pick(&live.plain_devices) {
                Some(d) if rng.chance(2) => (d.clone(), false),
                _ => (json!(rng.uuid()), true),
            };
            let step = cmd(json!({"strip": strip.id, "before": before, "fndef": fndef,
                "terminals": [{"designation": fresh(rng)}]}));
            Some(if dangling { step.refuse() } else { step })
        }
        _ => {
            let n = 1 + rng.below(3);
            let mut names: Vec<String> = Vec::new();
            while names.len() < n {
                let d = fresh(rng);
                if !names.contains(&d) {
                    names.push(d);
                }
            }
            let terminals: Vec<Value> = names
                .iter()
                .map(|d| json!({"designation": d, "function_text": rng.word(4)}))
                .collect();
            let mut p = json!({"strip": strip.id, "fndef": fndef, "terminals": terminals});
            let mut order: Vec<Value> = strip.order.iter().map(|(_, t)| t.clone()).collect();
            let mut at = order.len();
            if !strip.order.is_empty() && !rng.chance(4) {
                let before = rng.pick(&strip.order)?.1.clone();
                at = order.iter().position(|t| *t == before)?;
                p["before"] = before;
            }
            // The command refuses (CI-06, EW-TERM-004): new terminals between two terminals of one bridge group.
            order.insert(at, Value::Null);
            if strip.splits_a_bridge_group(&order) {
                return Some(cmd(p));
            }
            let total = strip.order.len() + n;
            let n = isize::try_from(n).unwrap_or(1);
            let (strip_id, designations) = (strip.id.clone(), names);
            let unique_before = strip.seqs.len() == strip.order.len();
            Some(
                cmd(p)
                    .counts(&[(Table::Devices, n), (Table::Functions, n)])
                    .check(move |_, after| {
                        let strip = typed::<ew_schema::Device>(&strip_id).ok_or("a strip ID")?;
                        let have: std::collections::BTreeSet<String> = after
                            .idx()
                            .terminals_of_strip(strip)
                            .filter_map(|t| after.devices().get(t).and_then(|d| d.aspects.terminal.clone()))
                            .map(|t| t.designation.as_str().to_owned())
                            .collect();
                        if !designations.iter().all(|d| have.contains(d)) {
                            return Err("a new terminal is not in the strip afterwards".to_owned());
                        }
                        let order = mounting_order(after, &strip_id)?;
                        let seqs: std::collections::BTreeSet<u32> = order.iter().map(|(s, _)| *s).collect();
                        if order.len() != total {
                            return Err(format!(
                                "the strip has {} terminals, expected {total}",
                                order.len()
                            ));
                        }
                        if unique_before && seqs.len() != order.len() {
                            return Err("the strip's sequence numbers are no longer unique".to_owned());
                        }
                        Ok(())
                    }),
            )
        }
    }
}

// WP-S2.07 part 1
pub(super) fn terminal_move(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let strips: Vec<&StripInfo> = live.strip_infos.iter().filter(|s| s.order.len() >= 2).collect();
    let strip = *rng.pick(&strips)?;
    let ids: Vec<Value> = strip.order.iter().map(|(_, t)| t.clone()).collect();
    let cmd = |p: Value| Step::cmd("terminal.move", p);
    match rng.below(12) {
        // Refused: nothing to move.
        0 => Some(cmd(json!({"terminals": []})).refuse()),
        // The command refuses: a terminal listed twice.
        1 => {
            let t = rng.pick(&ids)?;
            Some(cmd(json!({"terminals": [t, t]})))
        }
        // The command refuses: a device that is not a terminal.
        2 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(cmd(json!({"terminals": [ids[0], d]})))
        }
        // The command refuses: terminals of two strips.
        3 => {
            let other = strips.iter().find(|s| s.id != strip.id)?;
            Some(cmd(json!({"terminals": [ids[0], other.order[0].1]})))
        }
        // The command refuses: `before` among the moved terminals.
        4 => Some(cmd(json!({"terminals": [ids[0], ids[1]], "before": ids[1]}))),
        // The command refuses: an order that does not change (a terminal before its successor, or the last to the end).
        5 => {
            let i = rng.below(ids.len());
            Some(match ids.get(i + 1) {
                Some(next) => cmd(json!({"terminals": [ids[i]], "before": next})),
                None => cmd(json!({"terminals": [ids[i]]})),
            })
        }
        _ => {
            // One or two terminals, put before another terminal of the strip or at the end.
            let mut moved: Vec<usize> = vec![rng.below(ids.len())];
            if ids.len() >= 3 && rng.chance(3) {
                let j = rng.below(ids.len());
                if !moved.contains(&j) {
                    moved.push(j);
                }
            }
            let rest: Vec<usize> = (0..ids.len()).filter(|k| !moved.contains(k)).collect();
            let before = if rng.chance(4) {
                None
            } else {
                Some(*rng.pick(&rest)?)
            };
            let mut p = json!({"terminals": moved.iter().map(|&k| ids[k].clone()).collect::<Vec<_>>()});
            if let Some(b) = before {
                p["before"] = ids[b].clone();
            }
            // The new order, as the command computes it: no change is a refusal.
            moved.sort_unstable();
            let block: Vec<Value> = moved.iter().map(|&k| ids[k].clone()).collect();
            let mut order: Vec<Value> = rest.iter().map(|&k| ids[k].clone()).collect();
            let at = match before {
                Some(b) => order.iter().position(|t| *t == ids[b])?,
                None => order.len(),
            };
            order.splice(at..at, block.clone());
            // The command refuses: no change, or (CI-06, EW-TERM-004) an order that splits a bridge group.
            if order == ids || strip.splits_a_bridge_group(&order) {
                return Some(cmd(p));
            }
            let seqs_before: Vec<u32> = strip.order.iter().map(|(s, _)| *s).collect();
            let (strip_id, before_id) = (strip.id.clone(), before.map(|b| ids[b].clone()));
            Some(cmd(p).counts(&[(Table::Devices, 0)]).check(move |_, after| {
                let now = mounting_order(after, &strip_id)?;
                let mut seqs: Vec<u32> = now.iter().map(|(s, _)| *s).collect();
                seqs.sort_unstable();
                if seqs != seqs_before {
                    return Err(format!(
                        "the strip's sequence numbers changed: {seqs:?}, were {seqs_before:?}"
                    ));
                }
                let ids: Vec<&Value> = now.iter().map(|(_, t)| t).collect();
                let start = ids
                    .iter()
                    .position(|t| **t == block[0])
                    .ok_or("a moved terminal is gone")?;
                let end = start + block.len();
                let together = ids
                    .get(start..end)
                    .is_some_and(|w| w.iter().copied().eq(block.iter()));
                let placed = match &before_id {
                    Some(b) => ids.get(end).is_some_and(|t| *t == b),
                    None => end == ids.len(),
                };
                if together && placed {
                    Ok(())
                } else {
                    Err(format!(
                        "the moved terminals are not directly before {before_id:?}: order {ids:?}"
                    ))
                }
            }))
        }
    }
}

pub(super) fn conductor_specs(
    rng: &mut SplitMix64,
    n: usize,
    taken: &std::collections::BTreeSet<String>,
) -> Vec<Value> {
    let mut cores: Vec<String> = Vec::new();
    while cores.len() < n {
        let c = format!("C{}", rng.below(1000));
        if !taken.contains(&c) && !cores.contains(&c) {
            cores.push(c);
        }
    }
    cores
        .into_iter()
        .map(|core| {
            // Green-yellow is the two IEC 60757 codes GN and YE (EW-CORE-031 refuses a code `GNYE`).
            let colour: &[&str] = [&["BK"][..], &["BU"], &["RD"], &["GN", "YE"]]
                .get(rng.below(4))
                .copied()
                .unwrap_or(&["BK"]);
            json!({"core": core, "colour": colour, "csa": "1.5"})
        })
        .collect()
}

/// `check.waive` (WP-S1.16 part 5, D7): a `HumanOnly` command, which the fuzzer's session (the GUI) may submit. A
/// well-formed waiver (a rule ID, 32 hex digits, a reason of at least ten characters) of a finding no waiver of
/// that rule covers yet commits one waiver with the fingerprint in lower case; a malformed rule ID or fingerprint,
/// a short reason and a finding a waiver already covers (in either case) are refused.
pub(super) fn check_waive(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let hex =
        |rng: &mut SplitMix64| -> String { (0..16).map(|_| format!("{:02x}", rng.below(256))).collect() };
    let params = |rule: &str, fingerprint: &str, reason: &str| {
        json!({"rule": rule, "fingerprint": fingerprint, "reason": {"en-GB": reason},
               "author": "engineer", "date": "2025-04-14"})
    };
    let reason = "accepted for the fuzz run";
    match rng.below(6) {
        0 => Some(Step::cmd("check.waive", params("EW-CORE-41", &hex(rng), reason))),
        1 => Some(Step::cmd(
            "check.waive",
            params("EW-CORE-041", &rng.word(31), reason),
        )),
        2 => Some(Step::cmd(
            "check.waive",
            params("EW-CORE-041", &hex(rng), "too short"),
        )),
        3 => {
            let (_, rule, fingerprint) = rng.pick(&live.waivers)?;
            Some(Step::cmd(
                "check.waive",
                params(rule, &fingerprint.to_ascii_uppercase(), reason),
            ))
        }
        _ => {
            let fingerprint = hex(rng).to_ascii_uppercase();
            let rule = *rng.pick(&["EW-CORE-041", "EW-POT-013", "EW-RDS-003"])?;
            if live
                .waivers
                .iter()
                .any(|(_, r, f)| r == rule && f.eq_ignore_ascii_case(&fingerprint))
            {
                return None;
            }
            let lower = fingerprint.to_ascii_lowercase();
            Some(
                Step::cmd("check.waive", params(rule, &fingerprint, reason))
                    .counts(&[(Table::Waivers, 1)])
                    .check(move |_, after| {
                        after
                            .table::<ew_schema::Waiver>()
                            .iter()
                            .any(|(_, w)| {
                                w.rule.as_str() == rule && w.target.finding.as_deref() == Some(&lower)
                            })
                            .then_some(())
                            .ok_or_else(|| format!("no waiver of {rule} on {lower}"))
                    }),
            )
        }
    }
}

/// `check.unwaive` (WP-S1.16 part 5, D7): removing an existing waiver commits and drops one waiver; a waiver that
/// does not exist is refused.
pub(super) fn check_unwaive(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(4) {
        return Some(Step::cmd("check.unwaive", json!({"waiver": rng.uuid()})).refuse());
    }
    let (waiver, _, _) = rng.pick(&live.waivers)?;
    Some(Step::cmd("check.unwaive", json!({"waiver": waiver})).counts(&[(Table::Waivers, -1)]))
}

pub(super) fn cable_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let n = 1 + rng.below(4);
    let mut conductors = conductor_specs(rng, n, &std::collections::BTreeSet::new());
    let mut p = json!({"letter": "W", "function_text": rng.word(6)});
    if rng.chance(4) {
        p["parts"] = json!([part_use(&live.parts, rng)?]);
    }
    // The command refuses: two conductors with the same core.
    if rng.chance(8) {
        let again = conductors[0].clone();
        conductors.push(again);
        p["conductors"] = json!(conductors);
        return Some(Step::cmd("cable.create", p));
    }
    p["conductors"] = json!(conductors);
    let n = isize::try_from(n).unwrap_or(1);
    Some(Step::cmd("cable.create", p).counts(&[(Table::Devices, 1), (Table::Conductors, n)]))
}

pub(super) fn conductor_add(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cable = rng.pick(&live.cables)?;
    match rng.below(10) {
        // The command refuses: a core the cable has.
        0 => {
            let core = rng
                .pick(&cable.cores.iter().cloned().collect::<Vec<_>>())?
                .to_owned();
            Some(Step::cmd(
                "conductor.add",
                json!({"cable": cable.id, "conductors": [{"core": core}]}),
            ))
        }
        // Any device (FT-469): the command refuses one that is no cable and adds a conductor to a cable.
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(Step::cmd(
                "conductor.add",
                json!({"cable": d, "conductors": [{"core": "C1"}]}),
            ))
        }
        // The command refuses: nothing to add.
        2 => Some(Step::cmd(
            "conductor.add",
            json!({"cable": cable.id, "conductors": []}),
        )),
        _ => {
            let n = 1 + rng.below(3);
            let conductors = conductor_specs(rng, n, &cable.cores);
            let n = isize::try_from(n).unwrap_or(1);
            Some(
                Step::cmd(
                    "conductor.add",
                    json!({"cable": cable.id, "conductors": conductors}),
                )
                .counts(&[(Table::Conductors, n), (Table::Devices, 0)]),
            )
        }
    }
}

pub(super) fn bond_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let kind = *rng.pick(&["pelv_0v_pe", "fe_pe", "equipotential", "shield"])?;
    let (_, functions) = rng.pick(&live.page_functions)?;
    match rng.below(8) {
        // The command refuses: both ends are the same point.
        0 => {
            let f = rng.pick(functions)?;
            Some(Step::cmd(
                "bond.create",
                json!({"a": cp_end(f, 0), "b": cp_end(f, 0), "kind": kind}),
            ))
        }
        // The command refuses: a bond ends at a CP or a junction, not on a connection.
        1 => {
            let c = rng.pick(&live.connections)?;
            let f = rng.pick(functions)?;
            Some(Step::cmd(
                "bond.create",
                json!({"a": {"on_connection": {"connection": c, "at": [0, 0]}}, "b": cp_end(f, 0), "kind": kind}),
            ))
        }
        // Refused: `via` does not exist.
        2 => {
            let (a, b) = pick2(functions, rng)?;
            Some(
                Step::cmd(
                    "bond.create",
                    json!({"a": cp_end(a, 0), "b": cp_end(b, 0), "kind": kind, "via": rng.uuid()}),
                )
                .refuse(),
            )
        }
        _ => {
            let (a, b) = pick2(functions, rng)?;
            let mut p = json!({"a": cp_end(a, small_idx(rng)), "b": cp_end(b, small_idx(rng)), "kind": kind});
            if rng.chance(3) {
                p["via"] = rng.pick(&live.connections)?.clone();
            }
            Some(Step::cmd("bond.create", p).counts(&[(Table::Bonds, 1)]))
        }
    }
}

pub(super) fn device_link(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let kind = *rng.pick(&["drives", "linkage"])?;
    match rng.below(8) {
        // The command refuses: a device cannot link to itself.
        0 => {
            let d = rng.pick(&live.devices)?;
            Some(Step::cmd(
                "device.link",
                json!({"from": d, "to": d, "kind": kind}),
            ))
        }
        // The command refuses: the link exists already.
        1 => {
            let (from, to, kind) = rng
                .pick(&live.links.iter().collect::<Vec<_>>())?
                .to_owned()
                .clone();
            Some(Step::cmd(
                "device.link",
                json!({"from": from, "to": to, "kind": kind}),
            ))
        }
        // Refused: a device that does not exist.
        2 => {
            let d = rng.pick(&live.devices)?;
            Some(Step::cmd("device.link", json!({"from": d, "to": rng.uuid(), "kind": kind})).refuse())
        }
        _ => {
            let (a, b) = pick2(&live.devices, rng)?;
            let key = (id_text(a), id_text(b), kind.to_owned());
            if live.links.contains(&key) {
                return None;
            }
            Some(
                Step::cmd("device.link", json!({"from": a, "to": b, "kind": kind}))
                    .counts(&[(Table::DeviceLinks, 1)]),
            )
        }
    }
}

pub(super) fn assembly_attach(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (parent, child) = pick2(&live.tree, rng)?;
    let ancestors = |dev: &super::super::live::TreeDev| -> Vec<String> {
        let mut out = Vec::new();
        let mut at = dev.parent.clone();
        while let Some(p) = at {
            if out.contains(&p) || out.len() > live.tree.len() {
                break;
            }
            at = live
                .tree
                .iter()
                .find(|t| t.key == p)
                .and_then(|t| t.parent.clone());
            out.push(p);
        }
        out
    };
    match rng.below(10) {
        // The command refuses: a device cannot be its own child.
        0 => Some(Step::cmd(
            "assembly.attach",
            json!({"parent": parent.id, "children": [{"device": parent.id}]}),
        )),
        // The command refuses: positions are 1-based.
        1 => Some(Step::cmd(
            "assembly.attach",
            json!({"parent": parent.id, "children": [{"device": child.id, "position": 0}]}),
        )),
        // The command refuses: the child is listed twice.
        2 => Some(Step::cmd(
            "assembly.attach",
            json!({"parent": parent.id, "children": [{"device": child.id}, {"device": child.id}]}),
        )),
        // The command refuses: the child is an ancestor of the parent (the assembly graph is acyclic).
        3 => {
            let up = ancestors(parent);
            let a = live.tree.iter().find(|t| up.contains(&t.key))?;
            Some(Step::cmd(
                "assembly.attach",
                json!({"parent": parent.id, "children": [{"device": a.id}]}),
            ))
        }
        // Refused: the parent does not exist.
        4 => Some(
            Step::cmd(
                "assembly.attach",
                json!({"parent": rng.uuid(), "children": [{"device": child.id}]}),
            )
            .refuse(),
        ),
        // The command refuses: a position another child of the parent holds (no push insert).
        5 => {
            let holder = live.tree.iter().find(|t| {
                t.parent.as_deref() == Some(parent.key.as_str()) && t.position.is_some() && t.key != child.key
            })?;
            if ancestors(parent).contains(&child.key) {
                return None;
            }
            Some(Step::cmd(
                "assembly.attach",
                json!({"parent": parent.id, "children": [{"device": child.id, "position": holder.position}]}),
            ))
        }
        _ => {
            if ancestors(parent).contains(&child.key) {
                return None;
            }
            // A free position of the parent, or none.
            let taken: std::collections::BTreeSet<u32> = live
                .tree
                .iter()
                .filter(|t| t.parent.as_deref() == Some(parent.key.as_str()) && t.key != child.key)
                .filter_map(|t| t.position)
                .collect();
            let position = if rng.chance(3) {
                None
            } else {
                (1..40).find(|p| !taken.contains(p))
            };
            let mut spec = json!({"device": child.id});
            if let Some(p) = position {
                spec["position"] = json!(p);
            }
            let (parent_id, child_id) = (parent.id.clone(), child.id.clone());
            Some(
                Step::cmd(
                    "assembly.attach",
                    json!({"parent": parent.id, "children": [spec]}),
                )
                .check(move |_, after| {
                    let c = typed(&child_id)
                        .and_then(|i| after.devices().get(i).cloned())
                        .ok_or("child gone")?;
                    let want = typed(&parent_id);
                    if c.parent != want {
                        return Err(format!("the child's parent is {:?}, not the parent", c.parent));
                    }
                    (c.position == position)
                        .then_some(())
                        .ok_or_else(|| format!("the child's position is {:?}, not {position:?}", c.position))
                }),
            )
        }
    }
}

pub(super) fn valve_add(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let module = rng.pick(&live.tree)?;
    let part = part_use(&live.parts, rng)?;
    let taken: std::collections::BTreeSet<u32> = live
        .tree
        .iter()
        .filter(|t| t.parent.as_deref() == Some(module.key.as_str()))
        .filter_map(|t| t.position)
        .collect();
    match rng.below(8) {
        // The command refuses: positions are 1-based.
        0 => Some(Step::cmd(
            "valve.add",
            json!({"module": module.id, "part": part, "position": 0}),
        )),
        // The command refuses: a position another child holds.
        1 => {
            let p = *rng.pick(&taken.iter().copied().collect::<Vec<_>>())?;
            Some(Step::cmd(
                "valve.add",
                json!({"module": module.id, "part": part, "position": p}),
            ))
        }
        // Refused: the module does not exist.
        2 => Some(Step::cmd("valve.add", json!({"module": rng.uuid(), "part": part})).refuse()),
        // Refused: the part copy does not exist.
        3 => Some(
            Step::cmd(
                "valve.add",
                json!({"module": module.id, "part": {"part": {"copy": rng.uuid()}, "qty": "1"}}),
            )
            .refuse(),
        ),
        _ => {
            let mut p = json!({"module": module.id, "part": part});
            let expected = if rng.chance(2) {
                let free = (1..60).find(|n| !taken.contains(n) && *n > 20)?;
                p["position"] = json!(free);
                free
            } else {
                (1..).find(|n| !taken.contains(n))?
            };
            if rng.chance(2) {
                p["fndef"] = rng.pick(&live.fndefs)?.id.clone();
            }
            let (copy, fndef) = (p["part"]["part"]["copy"].clone(), p.get("fndef").cloned());
            let module_id = module.id.clone();
            Some(
                Step::cmd("valve.add", p)
                    .counts(&[(Table::Devices, 1)])
                    .check(functions_added(Some(copy), fndef, true))
                    .check(move |before, after| {
                        let want = typed(&module_id);
                        let new: Vec<_> = after
                            .devices()
                            .iter()
                            .filter(|(id, d)| d.parent == want && !before.devices().contains(*id))
                            .collect();
                        match new.as_slice() {
                            [(_, d)] if d.position == Some(expected) && d.parts.len() == 1 => Ok(()),
                            [(_, d)] => Err(format!(
                                "the valve is at position {:?}, not {expected}",
                                d.position
                            )),
                            // ID-1493: a part with a PLC section of role module makes the new device a PLC module,
                            // which normaliser N0 attaches to the project's only station when the module has no
                            // station ancestor (WP-S1.21 D4).
                            [] if after.devices().iter().any(|(id, d)| {
                                !before.devices().contains(id)
                                    && d.aspects.plc_module.is_some()
                                    && d.parent
                                        .and_then(|s| after.devices().get(s))
                                        .is_some_and(|s| s.aspects.plc_station.is_some())
                            }) =>
                            {
                                Ok(())
                            }
                            other => Err(format!("{} new children of the module, not one", other.len())),
                        }
                    }),
            )
        }
    }
}
