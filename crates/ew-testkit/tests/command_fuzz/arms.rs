// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The generator arms: one per registered command, each building a step from the live records and saying what
//! the model must do with it (the validity oracle of [`super::step`]).
//!
//! [`ARMS`] is the table the coverage test compares with the command registry
//! (`every_registered_command_has_a_generator_arm`): a new command fails that test until it has an arm here.
//! An arm returns `None` when the snapshot has nothing to target (the no-op rule of doc 18 §3.3).
//!
//! Every arm mixes three kinds of steps, by a dice roll:
//! - well-formed steps built from the live records and the command's own rules, [`Verdict::Accept`] where the
//!   generator models every rule that can refuse them (or [`Verdict::Either`] where prevent rules or the
//!   structural invariants may);
//! - malformed steps, [`Verdict::Refuse`]: a record that does not exist, a position off the grid, a duplicate
//!   key, a cycle: the model must refuse them and change nothing;
//! - after-commit checks: counts of the records the command creates, the primary record, and field values.

use serde_json::{Value, json};

use super::live::{Live, PageInfo, id_json, id_text};
use super::rng::SplitMix64;
use super::step::{Step, Table, typed};

/// The unit of a move: three steps of the page grid in nm (7 937 500 nm, a multiple of the grid of the golden
/// pages).
pub const MOVE_UNIT: i64 = 7_937_500;

/// One generator arm.
pub struct Arm {
    /// The canonical command name.
    pub command: &'static str,
    /// The relative frequency among the steps.
    pub weight: usize,
    /// Builds a step, or `None` when the snapshot has no target.
    pub build: fn(&Live, &mut SplitMix64) -> Option<Step>,
}

/// Every arm, in command-name order. The coverage test compares the names with the registry.
pub const ARMS: &[Arm] = &[
    Arm {
        command: "assembly.attach",
        weight: 3,
        build: assembly_attach,
    },
    Arm {
        command: "bond.create",
        weight: 2,
        build: bond_create,
    },
    Arm {
        command: "box.black",
        weight: 2,
        build: box_black,
    },
    Arm {
        command: "cable.create",
        weight: 2,
        build: cable_create,
    },
    Arm {
        command: "conductor.add",
        weight: 2,
        build: conductor_add,
    },
    Arm {
        command: "connection.delete",
        weight: 3,
        build: connection_delete,
    },
    Arm {
        command: "connection.draw",
        weight: 4,
        build: connection_draw,
    },
    Arm {
        command: "connection.route",
        weight: 3,
        build: connection_route,
    },
    Arm {
        command: "device.create",
        weight: 3,
        build: device_create,
    },
    Arm {
        command: "device.link",
        weight: 2,
        build: device_link,
    },
    Arm {
        command: "device.place",
        weight: 6,
        build: device_place,
    },
    Arm {
        command: "edit.delete",
        weight: 3,
        build: edit_delete,
    },
    Arm {
        command: "external_ref.create",
        weight: 2,
        build: external_ref_create,
    },
    Arm {
        command: "fields.set",
        weight: 8,
        build: fields_set,
    },
    Arm {
        command: "ip.continue",
        weight: 2,
        build: ip_continue,
    },
    Arm {
        command: "ip.place",
        weight: 3,
        build: ip_place,
    },
    Arm {
        command: "junction.edit",
        weight: 3,
        build: junction_edit,
    },
    Arm {
        command: "library.embed",
        weight: 2,
        build: library_embed,
    },
    Arm {
        command: "numbering.renumber",
        weight: 2,
        build: numbering_renumber,
    },
    Arm {
        command: "page.create",
        weight: 3,
        build: page_create,
    },
    Arm {
        command: "page.delete",
        weight: 1,
        build: page_delete,
    },
    Arm {
        command: "page.renumber",
        weight: 2,
        build: page_renumber,
    },
    Arm {
        command: "plc.bind_channel",
        weight: 3,
        build: plc_bind_channel,
    },
    Arm {
        command: "plc.config_project.create",
        weight: 2,
        build: plc_config_project_create,
    },
    Arm {
        command: "plc.set_module_part",
        weight: 1,
        build: plc_set_module_part,
    },
    Arm {
        command: "potential.define",
        weight: 3,
        build: potential_define,
    },
    Arm {
        command: "project.settings.set",
        weight: 1,
        build: project_settings_set,
    },
    Arm {
        command: "project.supply.set",
        weight: 2,
        build: project_supply_set,
    },
    Arm {
        command: "rep.move",
        weight: 5,
        build: rep_move,
    },
    Arm {
        command: "rep.set_anchor_policy",
        weight: 2,
        build: rep_set_anchor_policy,
    },
    Arm {
        command: "revision.create",
        weight: 3,
        build: revision_create,
    },
    Arm {
        command: "rep.transform",
        weight: 4,
        build: rep_transform,
    },
    Arm {
        command: "report_block.create",
        weight: 2,
        build: report_block_create,
    },
    Arm {
        command: "section.create",
        weight: 2,
        build: section_create,
    },
    Arm {
        command: "structure.create_node",
        weight: 2,
        build: structure_create_node,
    },
    Arm {
        command: "terminal.add",
        weight: 3,
        build: terminal_add,
    },
    Arm {
        command: "terminal.designate",
        weight: 3,
        build: terminal_designate,
    },
    Arm {
        command: "valve.add",
        weight: 3,
        build: valve_add,
    },
];

/// The representation types a page type allows (`allowed_rep_types` of `ew-commands`, in JSON form).
fn allowed(kind: &str) -> &'static [&'static str] {
    match kind {
        "multi_line" => &["multi_line", "contact_image", "strip_segment", "graphic"],
        "single_line" => &["single_line"],
        "overview" | "assembly_overview" => &["overview"],
        "bus_topology" => &["topology", "graphic"],
        "fluid" => &["fluid", "multi_line"],
        "graphic" => &["graphic"],
        _ => &[],
    }
}

fn pick2<'a, T>(items: &'a [T], rng: &mut SplitMix64) -> Option<(&'a T, &'a T)> {
    if items.len() < 2 {
        return None;
    }
    let a = rng.below(items.len());
    let mut b = rng.below(items.len() - 1);
    if b >= a {
        b += 1;
    }
    Some((&items[a], &items[b]))
}

fn cp_end(function: &Value, idx: u16) -> Value {
    json!({"cp": {"function": function, "idx": idx}})
}

fn small_idx(rng: &mut SplitMix64) -> u16 {
    u16::try_from(rng.below(4)).unwrap_or(0)
}

/// A letter of the identifier sets of the golden project.
fn letter(rng: &mut SplitMix64) -> &'static str {
    ["K", "Q", "F", "S", "X", "A", "M"][rng.below(7)]
}

// ---------------------------------------------------------------------------------------------------------
// rep.*, fields.set, deletes, renumbers, junctions, terminals, connections, potentials (the first 11 arms)

fn rep_move(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let rep = rng.pick(&live.reps)?.clone();
    let (mut k, mut m) = (rng.range(-2, 2), rng.range(-2, 2));
    if k == 0 && m == 0 {
        k = 1;
        m = -1;
    }
    let delta = [k * MOVE_UNIT, m * MOVE_UNIT];
    let rep_id = rep.clone();
    Some(
        Step::cmd("rep.move", json!({"reps": [rep], "delta": delta})).check(move |before, after| {
            let id = typed(&rep_id).ok_or("a rep ID")?;
            let (b, a) = (before.reps().get(id), after.reps().get(id));
            match (b, a) {
                (Some(b), Some(a))
                    if a.pos.x.0 == b.pos.x.0 + delta[0]
                        && a.pos.y.0 == b.pos.y.0 + delta[1]
                        && a.page == b.page =>
                {
                    Ok(())
                }
                (Some(_), Some(_)) => {
                    Err("the representation is not at its old position plus the delta".into())
                }
                _ => Err("the moved representation disappeared".into()),
            }
        }),
    )
}

fn rep_transform(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let rep = rng.pick(&live.reps)?.clone();
    match rng.below(10) {
        // Refused: `rotate` counts quarter turns, 0 to 3.
        0 => Some(
            Step::cmd(
                "rep.transform",
                json!({"reps": [rep], "rotate": 4, "mirror": false}),
            )
            .refuse(),
        ),
        // Refused: nothing to transform.
        1 => Some(Step::cmd("rep.transform", json!({"reps": [], "rotate": 1})).refuse()),
        // Refused: the representation does not exist.
        2 => Some(Step::cmd("rep.transform", json!({"reps": [rng.uuid()], "rotate": 1})).refuse()),
        _ => {
            let rotate = u8::try_from(rng.below(4)).unwrap_or(0);
            let mirror = rng.chance(2);
            let id = rep.clone();
            Some(
                Step::cmd(
                    "rep.transform",
                    json!({"reps": [rep], "rotate": rotate, "mirror": mirror}),
                )
                .check(move |before, after| {
                    let id = typed(&id).ok_or("a rep ID")?;
                    let (b, a) = (
                        before.reps().get(id).ok_or("the representation was not there")?,
                        after
                            .reps()
                            .get(id)
                            .ok_or("the transformed representation disappeared")?,
                    );
                    // The new orientation is the old one, turned, then mirrored.
                    let mut want = b.orient.then(ew_units::Orient2::from_parts(false, rotate));
                    if mirror {
                        want = want.then(ew_units::Orient2::M0);
                    }
                    if a.orient != want {
                        return Err(format!("orientation is {:?}, expected {want:?}", a.orient));
                    }
                    (a.pos == b.pos && a.page == b.page)
                        .then_some(())
                        .ok_or_else(|| "a transform moved the insertion point or the page".to_owned())
                }),
            )
        }
    }
}

fn fields_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(7)
        && let Some(step) = revision_fields_set(live, rng)
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
            let colour = *rng.pick(&["BK", "RD", "BU", "WH", "GNYE"])?;
            Some(one("connection", c, "overrides.colour", json!([colour])))
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
                one("device", &a.device, "assembly.bom_basis", json!(value))
                    .accept()
                    .check(move |_, after| {
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
        // the config did not hold before is refused (decision log ID-371).
        17 => {
            let a = rng.pick(&live.assemblies)?;
            let outside: Vec<&Value> = live
                .devices
                .iter()
                .filter(|d| !a.subtree.contains(d) && !a.covers.contains(&id_text(d)))
                .collect();
            let n = rng.below(3);
            let mut covers: Vec<Value> = (0..n).filter_map(|_| rng.pick(&a.subtree).cloned()).collect();
            let mut out_of_scope = false;
            if rng.chance(4)
                && let Some(d) = rng.pick(&outside)
            {
                covers.push((*d).clone());
                out_of_scope = true;
            }
            covers.sort_by_key(Value::to_string);
            covers.dedup();
            let configs = json!([{"order_code": format!("FZ{}", rng.below(1000)), "covers": covers}]);
            let step = one("device", &a.device, "assembly.configs", configs);
            Some(if out_of_scope {
                step.refuse()
            } else {
                step.accept()
            })
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
        // Refused: a path that is not settable, or a value the field does not take.
        23 => {
            let d = rng.pick(&live.devices)?;
            let path = *rng.pick(&["id", "parent", "position", "aspects.strip", "terminal.strip"])?;
            Some(one("device", d, path, json!(rng.below(9))).refuse())
        }
        24 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(one("device", d, "tag.counter", json!("not-a-number")).refuse())
        }
        // Refused: a target that does not exist, or one listed twice.
        _ => {
            if rng.chance(2) {
                Some(one("device", &json!(rng.uuid()), "function_text", json!("X")).refuse())
            } else {
                let d = rng.pick(&live.plain_devices)?;
                Some(
                    Step::cmd(
                        "fields.set",
                        json!({"kind": "device", "targets": [d, d], "path": "function_text", "value": "X"}),
                    )
                    .refuse(),
                )
            }
        }
    }
}

fn connection_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(12) {
        return Some(Step::cmd("connection.delete", json!({"connections": [rng.uuid()]})).refuse());
    }
    let c = rng.pick(&live.connections)?.clone();
    Some(
        Step::cmd("connection.delete", json!({"connections": [c]}))
            .accept()
            .check(move |_, after| {
                let id = typed(&c).ok_or("a connection ID")?;
                (!after.connections().contains(id))
                    .then_some(())
                    .ok_or_else(|| "the deleted connection still exists".to_owned())
            }),
    )
}

fn edit_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(8)
        && let Some(r) = rng.pick(&live.revisions)
    {
        let target = json!({"kind": "revision", "id": r.id});
        let step = Step::cmd("edit.delete", json!({"targets": [target]}));
        // Only the open revision can be deleted: a released or withdrawn one is immutable (FR-REV-009).
        return Some(if r.state == "open" {
            step.accept()
                .counts(&[(Table::Revisions, -1)])
                .check(|_, after| title_block_follows(after))
        } else {
            step.refuse()
        });
    }
    if rng.chance(12) {
        let target = json!({"kind": "device", "id": rng.uuid()});
        return Some(Step::cmd("edit.delete", json!({"targets": [target]})).refuse());
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

fn page_renumber(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(10) {
        0 => {
            let (section, first) = rng.pick(&live.sections)?;
            Some(
                Step::cmd(
                    "page.renumber",
                    json!({"scope": {"section": section}, "start": first, "step": 0}),
                )
                .refuse(),
            )
        }
        1 => Some(
            Step::cmd(
                "page.renumber",
                json!({"scope": {"section": rng.uuid()}, "start": 1, "step": 1}),
            )
            .refuse(),
        ),
        _ => {
            let (section, first) = rng.pick(&live.sections)?;
            let step = 1 + rng.below(2);
            Some(
                Step::cmd(
                    "page.renumber",
                    json!({"scope": {"section": section}, "start": first, "step": step}),
                )
                .counts(&[(Table::Pages, 0)]),
            )
        }
    }
}

fn numbering_renumber(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(12) {
        return Some(
            Step::cmd(
                "numbering.renumber",
                json!({"scope": {"pages": [rng.uuid()]}, "options": {}, "include_frozen": false}),
            )
            .refuse(),
        );
    }
    let scope = match rng.below(3) {
        0 => json!("project"),
        1 => json!({"pages": [rng.pick(&live.pages)?]}),
        _ => {
            let n = 1 + rng.below(4);
            let selection: Vec<&Value> = (0..n).filter_map(|_| rng.pick(&live.plain_devices)).collect();
            json!({"selection": selection})
        }
    };
    let options = json!({"close_gaps": rng.chance(2), "only_nonconforming": rng.chance(2)});
    Some(
        Step::cmd(
            "numbering.renumber",
            json!({"scope": scope, "options": options, "include_frozen": rng.chance(4)}),
        )
        .counts(&[(Table::Devices, 0)]),
    )
}

fn junction_edit(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let junction = rng.pick(&live.junctions)?.clone();
    let wiring = *rng.pick(&["star", "chain", "style_default"])?;
    match rng.below(10) {
        // Refused: the junction does not exist.
        0 => Some(Step::cmd("junction.edit", json!({"junction": rng.uuid(), "wiring": wiring})).refuse()),
        // Refused: a sequence must list the connections at the junction, and a new ID is none of them.
        1 => Some(
            Step::cmd(
                "junction.edit",
                json!({"junction": junction, "sequence": [rng.uuid()]}),
            )
            .refuse(),
        ),
        _ => {
            let id = junction.clone();
            Some(
                Step::cmd("junction.edit", json!({"junction": junction, "wiring": wiring}))
                    .accept()
                    .check(move |_, after| {
                        let j = typed(&id)
                            .and_then(|i| after.junctions().get(i).cloned())
                            .ok_or("the junction disappeared")?;
                        let have = serde_json::to_value(&j.wiring).map_err(|e| e.to_string())?;
                        // `style_default` clears the stored member.
                        let want = if wiring == "style_default" {
                            Value::Null
                        } else {
                            json!(wiring)
                        };
                        (have == want)
                            .then_some(())
                            .ok_or_else(|| format!("wiring is {have}, expected {want}"))
                    }),
            )
        }
    }
}

fn terminal_designate(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(10) {
        // Refused: a terminal listed twice.
        0 => {
            let strip = rng.pick(&live.strips)?;
            Some(
                Step::cmd(
                    "terminal.designate",
                    json!({"assignments": [
                        {"terminal": strip[0].0, "designation": "Z1"},
                        {"terminal": strip[0].0, "designation": "Z2"},
                    ]}),
                )
                .refuse(),
            )
        }
        // Refused: a device that is not a terminal.
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(
                Step::cmd(
                    "terminal.designate",
                    json!({"assignments": [{"terminal": d, "designation": "Z1"}]}),
                )
                .refuse(),
            )
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

fn connection_draw(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (page, functions) = rng.pick(&live.page_functions)?;
    if rng.chance(14) {
        let bad = json!(rng.uuid());
        return Some(
            Step::cmd(
                "connection.draw",
                json!({"a": cp_end(&bad, 0), "b": cp_end(&functions[0], 0), "page": page}),
            )
            .refuse(),
        );
    }
    let (a, b) = pick2(functions, rng)?;
    // A wire between two CPs is one new connection (no endpoint on a connection, so nothing is split).
    Some(
        Step::cmd(
            "connection.draw",
            json!({"a": cp_end(a, small_idx(rng)), "b": cp_end(b, small_idx(rng)), "page": page}),
        )
        .counts(&[(Table::Connections, 1)]),
    )
}

fn potential_define(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (page, functions) = rng.pick(&live.page_functions)?;
    match rng.below(10) {
        // Refused: a potential is defined at a CP or a junction, not on a connection.
        0 => {
            let c = rng.pick(&live.connections)?;
            Some(
                Step::cmd(
                    "potential.define",
                    json!({"at": {"on_connection": {"connection": c, "at": [0, 0]}}, "name": "FZ1"}),
                )
                .refuse(),
            )
        }
        // Refused: the function does not exist.
        1 => Some(
            Step::cmd(
                "potential.define",
                json!({"at": cp_end(&json!(rng.uuid()), 0), "name": "FZ1"}),
            )
            .refuse(),
        ),
        _ => {
            let _ = page;
            let f = rng.pick(functions)?;
            Some(
                Step::cmd(
                    "potential.define",
                    json!({"at": cp_end(f, small_idx(rng)), "name": format!("FZ{}", rng.below(1000))}),
                )
                .counts(&[(Table::PotentialDefs, 1)]),
            )
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Placement: device.place, ip.place, ip.continue, box.black, rep.set_anchor_policy, connection.route

fn pages_taking_reps(live: &Live) -> Vec<&PageInfo> {
    live.page_info
        .iter()
        .filter(|p| !allowed(&p.kind).is_empty())
        .collect()
}

/// A position on the grid of `page`: `(t1, t2)` grid units of `num` nm, inside the page area.
fn on_page(page: &PageInfo, rng: &mut SplitMix64) -> Value {
    Live::grid_pos(page, rng.range(2, 38), rng.range(2, 24))
}

fn device_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let pages = pages_taking_reps(live);
    let page = *rng.pick(&pages)?;
    let fits: Vec<_> = live
        .symbols
        .iter()
        .filter(|s| allowed(&page.kind).contains(&s.rep_type.as_str()))
        .collect();
    let symbol = *rng.pick(&fits)?;
    let pos = on_page(page, rng);
    let base = |target: Value| json!({"page": page.id, "symbol": symbol.id, "pos": pos, "target": target});
    match rng.below(20) {
        // Refused: the page does not exist.
        18 => {
            let mut p = base(json!({"new_device": {"letter": "K"}}));
            p["page"] = json!(rng.uuid());
            Some(Step::cmd("device.place", p).refuse())
        }
        // Refused: the position is off the connection grid.
        17 => {
            let mut p = base(json!({"new_device": {"letter": "K"}}));
            let n = i64::try_from(page.grid.0).unwrap_or(1);
            p["pos"] = json!([3 * n + 1, 4 * n]);
            Some(Step::cmd("device.place", p).refuse())
        }
        // Refused: a representation type the page type does not allow.
        19 => {
            let wrong: Vec<_> = live
                .symbols
                .iter()
                .filter(|s| !allowed(&page.kind).contains(&s.rep_type.as_str()))
                .collect();
            let w = *rng.pick(&wrong)?;
            let mut p = base(json!({"new_device": {"letter": "K"}}));
            p["symbol"] = w.id.clone();
            Some(Step::cmd("device.place", p).refuse())
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
            Some(if connections == 0 { step.accept() } else { step })
        }
    }
}

fn ip_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (symbol, def) = rng.pick(&live.ip_syms)?;
    let pages: Vec<_> = live.page_info.iter().filter(|p| p.kind == "multi_line").collect();
    let page = *rng.pick(&pages)?;
    let direction = *rng.pick(&["source", "target", "undirected"])?;
    let mut p = json!({
        "page": page.id, "symbol": symbol, "fndef": def, "pos": on_page(page, rng),
        "name": format!("FZ-{}", rng.below(1000)), "direction": direction,
        "star_source": rng.chance(4),
    });
    match rng.below(12) {
        0 => {
            p["page"] = json!(rng.uuid());
            Some(Step::cmd("ip.place", p).refuse())
        }
        1 => {
            let n = i64::try_from(page.grid.0).unwrap_or(1);
            p["pos"] = json!([2 * n + 1, 3 * n]);
            Some(Step::cmd("ip.place", p).refuse())
        }
        2 => {
            p["external"] = json!(rng.uuid());
            Some(Step::cmd("ip.place", p).refuse())
        }
        k => {
            if k == 3 {
                p["external"] = json!(rng.pick(&live.ext_refs)?);
            }
            Some(Step::cmd("ip.place", p).accept().counts(&[
                (Table::Reps, 1),
                (Table::Functions, 1),
                (Table::Devices, 0),
            ]))
        }
    }
}

fn ip_continue(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let ip = rng.pick(&live.ip_infos)?;
    match rng.below(8) {
        // Refused: the target page is the page of the interruption point.
        0 => Some(Step::cmd("ip.continue", json!({"ip": ip.function, "page": ip.page})).refuse()),
        // Refused: not an interruption point.
        1 => {
            let f = live.functions.iter().filter(|f| !f.ip).collect::<Vec<_>>();
            let f = rng.pick(&f)?;
            let page = rng.pick(&live.pages)?;
            Some(Step::cmd("ip.continue", json!({"ip": f.id, "page": page})).refuse())
        }
        _ => {
            // Another page of the same grid, so that the original position is on its grid too.
            let pages: Vec<_> = live
                .page_info
                .iter()
                .filter(|p| p.kind == "multi_line" && p.id != ip.page && p.grid == ip.grid)
                .collect();
            let page = *rng.pick(&pages)?;
            let name = ip.name.clone();
            let function = ip.function.clone();
            Some(
                Step::cmd("ip.continue", json!({"ip": ip.function, "page": page.id}))
                    .accept()
                    .counts(&[(Table::Reps, 1), (Table::Functions, 1)])
                    .check(move |_, after| {
                        // The partner carries the name of the original.
                        let found = after.functions().iter().any(|(id, f)| {
                            matches!(&f.kind, ew_schema::FunctionKind::Ip(s) if s.name.as_str() == name)
                                && serde_json::to_value(id).ok().as_ref() != Some(&function)
                        });
                        found
                            .then_some(())
                            .ok_or_else(|| format!("no partner named `{name}` afterwards"))
                    }),
            )
        }
    }
}

fn box_black(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (symbol, def) = rng.pick(&live.bb_syms)?;
    let pages: Vec<_> = live
        .page_info
        .iter()
        .filter(|p| p.kind == "multi_line" || p.kind == "fluid")
        .collect();
    let page = *rng.pick(&pages)?;
    let n = i64::try_from(page.grid.0).unwrap_or(1);
    let target = json!({"new_device": {"letter": "A", "fndef": def}});
    let mut p = json!({"page": page.id, "symbol": symbol, "pos": on_page(page, rng), "target": target});
    match rng.below(10) {
        // Refused: an outline of fewer than three points.
        0 => {
            p["box"] = json!({"outline": [[0, 0], [4 * n, 0]]});
            Some(Step::cmd("box.black", p).refuse())
        }
        // Refused: two box CPs with the same index.
        1 => {
            p["box"] = json!({
                "outline": [[0, 0], [4 * n, 0], [4 * n, 3 * n], [0, 3 * n]],
                "cps": [{"idx": 0, "dir": "right", "pos": [4 * n, n]}, {"idx": 0, "dir": "left", "pos": [0, n]}],
            });
            Some(Step::cmd("box.black", p).refuse())
        }
        k => {
            if k < 5 {
                p["box"] = json!({
                    "outline": [[0, 0], [4 * n, 0], [4 * n, 3 * n], [0, 3 * n]],
                    "cps": [{"idx": 0, "dir": "right", "pos": [4 * n, n]}, {"idx": 1, "dir": "left", "pos": [0, n]}],
                });
            }
            Some(Step::cmd("box.black", p).counts(&[
                (Table::Reps, 1),
                (Table::Functions, 1),
                (Table::Devices, 1),
            ]))
        }
    }
}

fn rep_set_anchor_policy(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(10) {
        0 => Some(Step::cmd("rep.set_anchor_policy", json!({"reps": [], "anchor": "origin"})).refuse()),
        1 => {
            let r = rng.pick(&live.reps)?;
            Some(
                Step::cmd(
                    "rep.set_anchor_policy",
                    json!({"reps": [r, r], "anchor": "origin"}),
                )
                .refuse(),
            )
        }
        2 => Some(
            Step::cmd(
                "rep.set_anchor_policy",
                json!({"reps": [rng.uuid()], "anchor": "origin"}),
            )
            .refuse(),
        ),
        _ => {
            let n = 1 + rng.below(3);
            let mut reps: Vec<Value> = (0..n).filter_map(|_| rng.pick(&live.reps).cloned()).collect();
            reps.sort_by_key(Value::to_string);
            reps.dedup();
            let anchor = match rng.below(5) {
                0 => json!("origin"),
                1 => json!("graphic_centre"),
                2 => json!({"named": [rng.range(-3, 3) * MOVE_UNIT, rng.range(-3, 3) * MOVE_UNIT]}),
                3 => json!({"slot": "tag"}),
                _ => Value::Null,
            };
            let expect = anchor.clone();
            let ids = reps.clone();
            Some(
                Step::cmd("rep.set_anchor_policy", json!({"reps": reps, "anchor": anchor}))
                    .accept()
                    .check(move |_, after| {
                        for id in &ids {
                            let r = typed(id)
                                .and_then(|i| after.reps().get(i).cloned())
                                .ok_or("rep gone")?;
                            let have = serde_json::to_value(&r.anchor).map_err(|e| e.to_string())?;
                            if have != expect {
                                return Err(format!("anchor is {have}, not {expect}"));
                            }
                        }
                        Ok(())
                    }),
            )
        }
    }
}

fn connection_route(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let r = rng.pick(&live.routes)?;
    let conn = r.connection.clone();
    let as_json =
        |route: &[(i64, i64)]| -> Value { Value::Array(route.iter().map(|(x, y)| json!([x, y])).collect()) };
    let verified = |step: Step, expect: Vec<(i64, i64)>, conn: Value| {
        step.accept().check(move |_, after| {
            let c = typed(&conn)
                .and_then(|i| after.connections().get(i).cloned())
                .ok_or("connection gone")?;
            let have: Vec<(i64, i64)> = c.route.iter().map(|p| (p.x.0, p.y.0)).collect();
            (have == expect)
                .then_some(())
                .ok_or_else(|| format!("the route is {have:?}, not {expect:?}"))
        })
    };
    let (a, b, n) = (r.a, r.b, r.grid_num);
    // A grid coordinate between the ends, for the bends.
    let mid = ((a.1 + b.1) / 2 / n + rng.range(-2, 2)) * n;
    match rng.below(10) {
        // Clearing the route is always valid (an empty route has no geometry).
        0 => Some(verified(
            Step::cmd("connection.route", json!({"connection": conn, "route": []})),
            Vec::new(),
            conn,
        )),
        // Refused: the first vertex does not meet its CP.
        1 => {
            let route = [(a.0 + 1, a.1), (b.0, a.1), b];
            Some(
                Step::cmd(
                    "connection.route",
                    json!({"connection": conn, "route": as_json(&route)}),
                )
                .refuse(),
            )
        }
        // Refused: a bend off the connection grid (and not on the line of an end).
        2 if mid + 1 != a.1 && mid + 1 != b.1 => {
            let route = [a, (a.0, mid + 1), (b.0, mid + 1), b];
            Some(
                Step::cmd(
                    "connection.route",
                    json!({"connection": conn, "route": as_json(&route)}),
                )
                .refuse(),
            )
        }
        // Refused: the connection does not exist.
        3 => {
            let route = [a, (b.0, a.1), b];
            Some(
                Step::cmd(
                    "connection.route",
                    json!({"connection": rng.uuid(), "route": as_json(&route)}),
                )
                .refuse(),
            )
        }
        // A dog-leg: its bend takes the x of one end and the y of the other, so both are the coordinates of
        // an end that meets its CP, whether or not they are on the grid.
        4..=6 => {
            let route = vec![a, (b.0, a.1), b];
            Some(verified(
                Step::cmd(
                    "connection.route",
                    json!({"connection": conn, "route": as_json(&route)}),
                ),
                route,
                conn,
            ))
        }
        // A Z-shaped route through a bend line on the grid.
        _ => {
            let route = vec![a, (a.0, mid), (b.0, mid), b];
            Some(verified(
                Step::cmd(
                    "connection.route",
                    json!({"connection": conn, "route": as_json(&route)}),
                ),
                route,
                conn,
            ))
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Pages, sections, structure, report blocks, external references

fn page_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let kind = *rng.pick(&["multi_line", "single_line", "overview", "fluid", "graphic"])?;
    let title = rng.word(10);
    match rng.below(12) {
        // Refused: report pages are generated (EW-INV-012).
        0 => Some(Step::cmd("page.create", json!({"number": 5000, "kind": "report"})).refuse()),
        // Refused: no folio.
        1 => Some(Step::cmd("page.create", json!({"kind": kind})).refuse()),
        // Refused: a folio another page of the section has.
        2 => {
            let p = rng.pick(&live.page_info)?;
            let mut params = json!({"number": p.number, "kind": kind});
            if let Some(s) = &p.section {
                params["section"] = s.clone();
            }
            if let Some(l) = &p.label {
                params["label"] = json!(l);
            }
            Some(Step::cmd("page.create", params).refuse())
        }
        // Refused: the section does not exist.
        3 => Some(
            Step::cmd(
                "page.create",
                json!({"number": 4000, "section": rng.uuid(), "kind": kind}),
            )
            .refuse(),
        ),
        // Refused: the page to insert after does not exist.
        4 => Some(Step::cmd("page.create", json!({"after": rng.uuid(), "kind": kind})).refuse()),
        // After a page: the folio is proposed.
        5 | 6 => {
            let p = rng.pick(&live.pages)?;
            Some(
                Step::cmd("page.create", json!({"after": p, "kind": kind, "title": title}))
                    .counts(&[(Table::Pages, 1)]),
            )
        }
        // A page of no section.
        7 => Some(
            Step::cmd(
                "page.create",
                json!({"number": 10_000 + rng.below(10_000), "kind": kind, "title": title}),
            )
            .counts(&[(Table::Pages, 1)]),
        ),
        // An explicit free folio in the range of a section.
        _ => {
            let s = rng.pick(&live.section_info)?;
            let taken: std::collections::BTreeSet<u32> = live
                .page_info
                .iter()
                .filter(|p| p.section.as_ref() == Some(&s.id))
                .map(|p| p.number)
                .chain(std::iter::once(s.range.0))
                .collect();
            let span = s.range.1.saturating_sub(s.range.0).min(60);
            let free: Vec<u32> = (s.range.0..=s.range.0 + span)
                .filter(|n| !taken.contains(n))
                .collect();
            let n = *rng.pick(&free)?;
            Some(
                Step::cmd(
                    "page.create",
                    json!({"section": s.id, "number": n, "kind": kind, "title": title}),
                )
                .counts(&[(Table::Pages, 1)]),
            )
        }
    }
}

fn page_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(10) {
        0 => Some(Step::cmd("page.delete", json!({"pages": [rng.uuid()]})).refuse()),
        1 => Some(Step::cmd("page.delete", json!({"pages": []})).refuse()),
        _ => {
            // The golden project keeps most of its pages: the run deletes while there are enough.
            if live.pages.len() < 6 {
                return None;
            }
            let page = rng.pick(&live.pages)?.clone();
            let delete_devices = rng.chance(3);
            let id = page.clone();
            Some(
                Step::cmd(
                    "page.delete",
                    json!({"pages": [page], "delete_devices": delete_devices}),
                )
                .check(move |before, after| {
                    let id = typed(&id).ok_or("a page ID")?;
                    if after.pages().contains(id) {
                        return Err("the deleted page still exists".into());
                    }
                    let (b, a) = (before.pages().len(), after.pages().len());
                    (a + 1 == b)
                        .then_some(())
                        .ok_or_else(|| format!("{b} pages before and {a} after: one page was deleted"))
                }),
            )
        }
    }
}

fn section_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let number = live.section_info.iter().map(|s| s.number).max().unwrap_or(0) + 1;
    let free_from = live.section_info.iter().map(|s| s.range.1).max().unwrap_or(0) + 1;
    match rng.below(10) {
        // Refused: the range starts after it ends.
        0 => Some(
            Step::cmd(
                "section.create",
                json!({"number": number, "folio_range": [free_from + 50, free_from]}),
            )
            .refuse(),
        ),
        // Refused: a number a section has.
        1 => {
            let s = rng.pick(&live.section_info)?;
            Some(
                Step::cmd(
                    "section.create",
                    json!({"number": s.number, "folio_range": [free_from, free_from + 9]}),
                )
                .refuse(),
            )
        }
        // Refused: overlaps the range of a section.
        2 => {
            let s = rng.pick(&live.section_info)?;
            Some(
                Step::cmd(
                    "section.create",
                    json!({"number": number, "folio_range": [s.range.0, s.range.0 + 3]}),
                )
                .refuse(),
            )
        }
        _ => {
            let from = free_from + u32::try_from(rng.below(40)).unwrap_or(0);
            let to = from + u32::try_from(rng.below(50)).unwrap_or(0);
            Some(
                Step::cmd(
                    "section.create",
                    json!({"number": number, "folio_range": [from, to], "title": rng.word(8), "divider": rng.chance(2)}),
                )
                .accept()
                .counts(&[(Table::Sections, 1)]),
            )
        }
    }
}

const ASPECTS: [&str; 7] = [
    "functional_assignment",
    "function",
    "installation_site",
    "location",
    "document_kind",
    "user_defined",
    "type",
];

fn structure_create_node(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let aspect = *rng.pick(&ASPECTS)?;
    let code = format!("FZ{}", rng.below(100_000));
    match rng.below(10) {
        // Refused: a parent of another aspect (EW-INV-011).
        0 => {
            let other = live.nodes.iter().find(|n| n.aspect != aspect)?;
            Some(
                Step::cmd(
                    "structure.create_node",
                    json!({"aspect": aspect, "code": code, "parent": other.id}),
                )
                .refuse(),
            )
        }
        // Refused: a sibling with the same code.
        1 => {
            let n = rng.pick(&live.nodes)?;
            let mut p = json!({"aspect": n.aspect, "code": n.code});
            if let Some(parent) = &n.parent {
                p["parent"] = parent.clone();
            }
            Some(Step::cmd("structure.create_node", p).refuse())
        }
        // Refused: the product aspect is the device tree.
        2 => Some(
            Step::cmd(
                "structure.create_node",
                json!({"aspect": "product", "code": code}),
            )
            .refuse(),
        ),
        // Refused: the parent does not exist.
        3 => Some(
            Step::cmd(
                "structure.create_node",
                json!({"aspect": aspect, "code": code, "parent": rng.uuid()}),
            )
            .refuse(),
        ),
        _ => {
            let same: Vec<_> = live.nodes.iter().filter(|n| n.aspect == aspect).collect();
            let mut p = json!({"aspect": aspect, "code": code, "description": rng.word(10)});
            if rng.chance(2)
                && let Some(parent) = rng.pick(&same)
            {
                p["parent"] = parent.id.clone();
            }
            if rng.chance(4) {
                p["aliases"] = json!([format!("AL{}", rng.below(100))]);
            }
            Some(
                Step::cmd("structure.create_node", p)
                    .accept()
                    .counts(&[(Table::Nodes, 1)]),
            )
        }
    }
}

fn report_block_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    const KINDS: [&str; 14] = [
        "profile_tables",
        "part_data",
        "toc",
        "structure_overview",
        "parts_summary",
        "parts_list",
        "device_list",
        "terminal_diagram",
        "terminal_overview",
        "cable_overview_diagrams",
        "wire_list",
        "plc_overview",
        "io_list",
        "potential_overview",
    ];
    let form = rng.pick(&live.forms)?;
    let kind = *rng.pick(&KINDS)?;
    match rng.below(10) {
        // Refused: a page-oriented block needs its first folio (EW-INV-014).
        0 => Some(
            Step::cmd(
                "report_block.create",
                json!({"source": {"kind": kind}, "form": form}),
            )
            .refuse(),
        ),
        // Refused: the form copy does not exist.
        1 => Some(
            Step::cmd(
                "report_block.create",
                json!({"source": {"kind": kind}, "form": rng.uuid(), "first_folio": 6000}),
            )
            .refuse(),
        ),
        _ => {
            let s = rng.pick(&live.section_info)?;
            let taken: std::collections::BTreeSet<u32> = live
                .page_info
                .iter()
                .filter(|p| p.section.as_ref() == Some(&s.id))
                .map(|p| p.number)
                .chain(std::iter::once(s.range.0))
                .collect();
            let free: Vec<u32> = (s.range.0..=s.range.1.min(s.range.0 + 80))
                .filter(|n| !taken.contains(n))
                .collect();
            let n = *rng.pick(&free)?;
            Some(
                Step::cmd(
                    "report_block.create",
                    json!({"source": {"kind": kind}, "form": form, "section": s.id, "first_folio": n, "title": rng.word(8)}),
                )
                .counts(&[(Table::ReportBlocks, 1)]),
            )
        }
    }
}

fn external_ref_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(8) {
        // Refused: the node is not a location.
        0 => {
            let n = live.nodes.iter().find(|n| n.aspect != "location")?;
            Some(Step::cmd("external_ref.create", json!({"location": n.id, "device": "X1"})).refuse())
        }
        // Refused: the node does not exist.
        1 => Some(
            Step::cmd(
                "external_ref.create",
                json!({"location": rng.uuid(), "device": "X1"}),
            )
            .refuse(),
        ),
        _ => {
            let location = rng.pick(&live.locations)?;
            let mut p = json!({"location": location, "device": format!("JE-{}", rng.word(6))});
            if rng.chance(2) {
                p["connection"] = json!(rng.word(2));
            }
            if rng.chance(3) {
                p["project"] = json!(format!("P{}", rng.below(100)));
            }
            Some(
                Step::cmd("external_ref.create", p)
                    .accept()
                    .counts(&[(Table::ExternalRefs, 1)]),
            )
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Devices, terminals, cables

fn part_use(parts: &[Value], rng: &mut SplitMix64) -> Option<Value> {
    let part = rng.pick(parts)?;
    Some(json!({"part": {"copy": part}, "qty": "1"}))
}

fn device_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let mut p = json!({"letter": letter(rng)});
    let mut functions = 0;
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
        functions = 1;
    }
    if rng.chance(5) {
        p["strip"] = json!({});
    }
    if rng.chance(4) {
        p["function_text"] = json!(rng.word(8));
    }
    Some(
        Step::cmd("device.create", p)
            .accept()
            .counts(&[(Table::Devices, 1), (Table::Functions, functions)]),
    )
}

fn terminal_add(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
        // Refused: a designation the strip has.
        0 => {
            let (_, d) = rng.pick(&strip.terminals)?;
            Some(
                Step::cmd(
                    "terminal.add",
                    json!({"strip": strip.id, "fndef": fndef, "terminals": [{"designation": d}]}),
                )
                .refuse(),
            )
        }
        // Refused: not a strip.
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(
                Step::cmd(
                    "terminal.add",
                    json!({"strip": d, "fndef": fndef, "terminals": [{"designation": "Z1"}]}),
                )
                .refuse(),
            )
        }
        // Refused: nothing to add.
        2 => Some(
            Step::cmd(
                "terminal.add",
                json!({"strip": strip.id, "fndef": fndef, "terminals": []}),
            )
            .refuse(),
        ),
        // Refused: a sequence number the strip has.
        3 => {
            let seq = rng
                .pick(&strip.seqs.iter().copied().collect::<Vec<_>>())?
                .to_owned();
            Some(
                Step::cmd(
                    "terminal.add",
                    json!({"strip": strip.id, "fndef": fndef, "terminals": [{"designation": fresh(rng), "seq": seq}]}),
                )
                .refuse(),
            )
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
                .accept()
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

fn conductor_specs(rng: &mut SplitMix64, n: usize, taken: &std::collections::BTreeSet<String>) -> Vec<Value> {
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
            let colour = *["BK", "BU", "RD", "GNYE"].get(rng.below(4)).unwrap_or(&"BK");
            json!({"core": core, "colour": [colour], "csa": "1.5"})
        })
        .collect()
}

fn cable_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let n = 1 + rng.below(4);
    let mut conductors = conductor_specs(rng, n, &std::collections::BTreeSet::new());
    let mut p = json!({"letter": "W", "function_text": rng.word(6)});
    if rng.chance(4) {
        p["parts"] = json!([part_use(&live.parts, rng)?]);
    }
    // Refused: two conductors with the same core.
    if rng.chance(8) {
        let again = conductors[0].clone();
        conductors.push(again);
        p["conductors"] = json!(conductors);
        return Some(Step::cmd("cable.create", p).refuse());
    }
    p["conductors"] = json!(conductors);
    let n = isize::try_from(n).unwrap_or(1);
    Some(
        Step::cmd("cable.create", p)
            .accept()
            .counts(&[(Table::Devices, 1), (Table::Conductors, n)]),
    )
}

fn conductor_add(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cable = rng.pick(&live.cables)?;
    match rng.below(10) {
        // Refused: a core the cable has.
        0 => {
            let core = rng
                .pick(&cable.cores.iter().cloned().collect::<Vec<_>>())?
                .to_owned();
            Some(
                Step::cmd(
                    "conductor.add",
                    json!({"cable": cable.id, "conductors": [{"core": core}]}),
                )
                .refuse(),
            )
        }
        // Refused: not a cable.
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(
                Step::cmd(
                    "conductor.add",
                    json!({"cable": d, "conductors": [{"core": "C1"}]}),
                )
                .refuse(),
            )
        }
        // Refused: nothing to add.
        2 => Some(Step::cmd("conductor.add", json!({"cable": cable.id, "conductors": []})).refuse()),
        _ => {
            let n = 1 + rng.below(3);
            let conductors = conductor_specs(rng, n, &cable.cores);
            let n = isize::try_from(n).unwrap_or(1);
            Some(
                Step::cmd(
                    "conductor.add",
                    json!({"cable": cable.id, "conductors": conductors}),
                )
                .accept()
                .counts(&[(Table::Conductors, n), (Table::Devices, 0)]),
            )
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Bonds, links, the device tree, valves, PLC records, supply, settings, library copies

fn bond_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let kind = *rng.pick(&["pelv_0v_pe", "fe_pe", "equipotential", "shield"])?;
    let (_, functions) = rng.pick(&live.page_functions)?;
    match rng.below(8) {
        // Refused: both ends are the same point.
        0 => {
            let f = rng.pick(functions)?;
            Some(
                Step::cmd(
                    "bond.create",
                    json!({"a": cp_end(f, 0), "b": cp_end(f, 0), "kind": kind}),
                )
                .refuse(),
            )
        }
        // Refused: a bond ends at a CP or a junction, not on a connection.
        1 => {
            let c = rng.pick(&live.connections)?;
            let f = rng.pick(functions)?;
            Some(
                Step::cmd(
                    "bond.create",
                    json!({"a": {"on_connection": {"connection": c, "at": [0, 0]}}, "b": cp_end(f, 0), "kind": kind}),
                )
                .refuse(),
            )
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

fn device_link(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let kind = *rng.pick(&["drives", "linkage"])?;
    match rng.below(8) {
        // Refused: a device cannot link to itself.
        0 => {
            let d = rng.pick(&live.devices)?;
            Some(Step::cmd("device.link", json!({"from": d, "to": d, "kind": kind})).refuse())
        }
        // Refused: the link exists already.
        1 => {
            let (from, to, kind) = rng
                .pick(&live.links.iter().collect::<Vec<_>>())?
                .to_owned()
                .clone();
            Some(Step::cmd("device.link", json!({"from": from, "to": to, "kind": kind})).refuse())
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
                    .accept()
                    .counts(&[(Table::DeviceLinks, 1)]),
            )
        }
    }
}

fn assembly_attach(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (parent, child) = pick2(&live.tree, rng)?;
    let ancestors = |dev: &super::live::TreeDev| -> Vec<String> {
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
        // Refused: a device cannot be its own child.
        0 => Some(
            Step::cmd(
                "assembly.attach",
                json!({"parent": parent.id, "children": [{"device": parent.id}]}),
            )
            .refuse(),
        ),
        // Refused: positions are 1-based.
        1 => Some(
            Step::cmd(
                "assembly.attach",
                json!({"parent": parent.id, "children": [{"device": child.id, "position": 0}]}),
            )
            .refuse(),
        ),
        // Refused: the child is listed twice.
        2 => Some(
            Step::cmd(
                "assembly.attach",
                json!({"parent": parent.id, "children": [{"device": child.id}, {"device": child.id}]}),
            )
            .refuse(),
        ),
        // Refused: the child is an ancestor of the parent (the assembly graph is acyclic).
        3 => {
            let up = ancestors(parent);
            let a = live.tree.iter().find(|t| up.contains(&t.key))?;
            Some(
                Step::cmd(
                    "assembly.attach",
                    json!({"parent": parent.id, "children": [{"device": a.id}]}),
                )
                .refuse(),
            )
        }
        // Refused: the parent does not exist.
        4 => Some(
            Step::cmd(
                "assembly.attach",
                json!({"parent": rng.uuid(), "children": [{"device": child.id}]}),
            )
            .refuse(),
        ),
        // Refused: a position another child of the parent holds (no push insert).
        5 => {
            let holder = live.tree.iter().find(|t| {
                t.parent.as_deref() == Some(parent.key.as_str()) && t.position.is_some() && t.key != child.key
            })?;
            if ancestors(parent).contains(&child.key) {
                return None;
            }
            Some(
                Step::cmd(
                    "assembly.attach",
                    json!({"parent": parent.id, "children": [{"device": child.id, "position": holder.position}]}),
                )
                .refuse(),
            )
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
                .accept()
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

fn valve_add(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let module = rng.pick(&live.tree)?;
    let part = part_use(&live.parts, rng)?;
    let taken: std::collections::BTreeSet<u32> = live
        .tree
        .iter()
        .filter(|t| t.parent.as_deref() == Some(module.key.as_str()))
        .filter_map(|t| t.position)
        .collect();
    match rng.below(8) {
        // Refused: positions are 1-based.
        0 => Some(
            Step::cmd(
                "valve.add",
                json!({"module": module.id, "part": part, "position": 0}),
            )
            .refuse(),
        ),
        // Refused: a position another child holds.
        1 => {
            let p = *rng.pick(&taken.iter().copied().collect::<Vec<_>>())?;
            Some(
                Step::cmd(
                    "valve.add",
                    json!({"module": module.id, "part": part, "position": p}),
                )
                .refuse(),
            )
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
            let functions = if rng.chance(2) {
                p["fndef"] = rng.pick(&live.fndefs)?.id.clone();
                1
            } else {
                0
            };
            let module_id = module.id.clone();
            Some(
                Step::cmd("valve.add", p)
                    .accept()
                    .counts(&[(Table::Devices, 1), (Table::Functions, functions)])
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
                            other => Err(format!("{} new children of the module, not one", other.len())),
                        }
                    }),
            )
        }
    }
}

fn plc_config_project_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let profile = "siemens-tia-intl@2027.1";
    match rng.below(8) {
        // Refused: a name another configuration project has.
        0 => {
            let name = rng
                .pick(&live.cfg_names.iter().cloned().collect::<Vec<_>>())?
                .to_owned();
            Some(
                Step::cmd(
                    "plc.config_project.create",
                    json!({"name": name, "profile": profile}),
                )
                .refuse(),
            )
        }
        // Refused: not `id@version`.
        1 => Some(
            Step::cmd(
                "plc.config_project.create",
                json!({"name": format!("FZ{}", rng.below(10_000)), "profile": "no-version"}),
            )
            .refuse(),
        ),
        // Refused: the CPU device does not exist.
        2 => Some(
            Step::cmd(
                "plc.config_project.create",
                json!({"name": format!("FZ{}", rng.below(10_000)), "profile": profile, "cpu": rng.uuid()}),
            )
            .refuse(),
        ),
        _ => {
            let mut name = format!("FZ{}", rng.below(10_000));
            while live.cfg_names.contains(&name) {
                name.push('x');
            }
            let mut p = json!({"name": name, "profile": profile});
            if rng.chance(2) {
                p["cpu"] = rng.pick(&live.devices)?.clone();
            }
            Some(
                Step::cmd("plc.config_project.create", p)
                    .accept()
                    .counts(&[(Table::PlcConfigProjects, 1)]),
            )
        }
    }
}

fn plc_bind_channel(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let ch = rng.pick(&live.channels)?;
    match rng.below(10) {
        // Refused: a CP of a function outside the module's subtree.
        0 => {
            let (f, idx) = ch.outside.as_ref()?;
            Some(
                Step::cmd(
                    "plc.bind_channel",
                    json!({"channel": ch.id, "endpoint": cp_end(f, *idx)}),
                )
                .refuse(),
            )
        }
        // Refused: the function has no such CP.
        1 => {
            let (f, _) = rng.pick(&ch.targets)?;
            Some(
                Step::cmd(
                    "plc.bind_channel",
                    json!({"channel": ch.id, "endpoint": cp_end(f, 99)}),
                )
                .refuse(),
            )
        }
        // Refused: a junction is not a signal CP.
        2 => {
            let j = rng.pick(&live.junctions)?;
            Some(
                Step::cmd(
                    "plc.bind_channel",
                    json!({"channel": ch.id, "endpoint": {"junction": {"junction": j}}}),
                )
                .refuse(),
            )
        }
        // Refused: the channel does not exist.
        3 => {
            let (f, idx) = rng.pick(&ch.targets)?;
            Some(
                Step::cmd(
                    "plc.bind_channel",
                    json!({"channel": rng.uuid(), "endpoint": cp_end(f, *idx)}),
                )
                .refuse(),
            )
        }
        // Unbinding is always valid.
        4 => {
            let channel = ch.id.clone();
            Some(
                Step::cmd("plc.bind_channel", json!({"channel": ch.id, "endpoint": null}))
                    .accept()
                    .check(move |_, after| {
                        let c =
                            typed(&channel).and_then(|i| after.table::<ew_schema::Channel>().get(i).cloned());
                        c.is_some_and(|c| c.binding.is_none())
                            .then_some(())
                            .ok_or_else(|| "the channel is still bound".to_owned())
                    }),
            )
        }
        _ => {
            let (f, idx) = rng.pick(&ch.targets)?.clone();
            let (channel, function) = (ch.id.clone(), f.clone());
            Some(
                Step::cmd(
                    "plc.bind_channel",
                    json!({"channel": ch.id, "endpoint": cp_end(&f, idx)}),
                )
                .accept()
                .check(move |_, after| {
                    let want = ew_schema::Endpoint::Cp(typed(&function).ok_or("a function ID")?, idx);
                    let id = typed::<ew_schema::Channel>(&channel).ok_or("a channel ID")?;
                    let c = after
                        .table::<ew_schema::Channel>()
                        .get(id)
                        .cloned()
                        .ok_or("channel gone")?;
                    if c.binding.as_ref() != Some(&want) {
                        return Err(format!("the channel is bound to {:?}, not the CP", c.binding));
                    }
                    // A CP is in at most one channel (FR-PLC-005).
                    let n = after
                        .table::<ew_schema::Channel>()
                        .iter()
                        .filter(|(_, o)| o.binding.as_ref() == Some(&want))
                        .count();
                    (n == 1)
                        .then_some(())
                        .ok_or_else(|| format!("{n} channels are bound to the same CP"))
                }),
            )
        }
    }
}

/// `plc.set_module_part` (Bulk: the runner previews and commits it). A well-formed step sets a module's
/// part to any part copy (or keeps it) and leaves no channel of the module at or above the new part's
/// channel count: a part without channels removes all eight channels of a golden module with their bindings.
fn plc_set_module_part(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let m = rng.pick(&live.modules)?.clone();
    match rng.below(8) {
        // Refused: the module does not exist.
        0 => Some(Step::cmd("plc.set_module_part", json!({"module": rng.uuid()})).refuse()),
        // Refused: the part copy does not exist.
        1 => Some(
            Step::cmd(
                "plc.set_module_part",
                json!({"module": m, "parts": [{"part": {"copy": rng.uuid()}, "qty": "1"}]}),
            )
            .refuse(),
        ),
        k => {
            let mut p = json!({"module": m});
            if k > 2 {
                p["parts"] = json!([part_use(&live.parts, rng)?]);
            }
            let module = m.clone();
            Some(
                Step::cmd("plc.set_module_part", p)
                    .accept()
                    .counts(&[(Table::Devices, 0)])
                    .check(move |before, after| {
                        let id = typed::<ew_schema::Device>(&module).ok_or("a device ID")?;
                        let has_aspect = after
                            .devices()
                            .get(id)
                            .is_some_and(|d| d.aspects.plc_module.is_some());
                        let count = if has_aspect {
                            ew_derive::module_part(after, id)
                                .and_then(|(_, s)| s.channels.map(|c| c.count))
                                .unwrap_or(0)
                        } else {
                            0
                        };
                        let surplus = after
                            .table::<ew_schema::Channel>()
                            .iter()
                            .filter(|(_, c)| c.module == id && c.number >= count)
                            .count();
                        if surplus != 0 {
                            return Err(format!("{surplus} surplus channels remain"));
                        }
                        // no over-deletion: every channel below the count survives with its binding
                        for (cid, c) in before.table::<ew_schema::Channel>().iter() {
                            if c.module != id || c.number >= count {
                                continue;
                            }
                            match after.table::<ew_schema::Channel>().get(cid) {
                                Some(a) if a.binding == c.binding => {}
                                Some(_) => return Err(format!("channel {} lost its binding", c.number)),
                                None => return Err(format!("channel {} below the count is gone", c.number)),
                            }
                        }
                        Ok(())
                    }),
            )
        }
    }
}

fn project_supply_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(8) {
        // Refused: a member that is not settable in v0.
        0 => {
            let path = *rng.pick(&["phases", "u_nom", "earthing", "origin", "origin.ik_max", "bogus"])?;
            Some(Step::cmd("project.supply.set", json!({"path": path, "value": 1})).refuse())
        }
        // Refused: a value the member does not take.
        1 => Some(Step::cmd("project.supply.set", json!({"path": "origin.source", "value": 5})).refuse()),
        // Refused: an external reference that does not exist.
        2 => Some(
            Step::cmd(
                "project.supply.set",
                json!({"path": "origin.source", "value": rng.uuid()}),
            )
            .refuse(),
        ),
        _ => {
            if !live.has_supply {
                return None;
            }
            let (value, expect) = if rng.chance(4) {
                (Value::Null, None)
            } else {
                let e = rng.pick(&live.ext_refs)?.clone();
                (e.clone(), Some(e))
            };
            Some(
                Step::cmd(
                    "project.supply.set",
                    json!({"path": "origin.source", "value": value}),
                )
                .accept()
                .check(move |_, after| {
                    let have = after
                        .project()
                        .supply
                        .as_ref()
                        .and_then(|s| s.origin.as_ref())
                        .and_then(|o| o.source)
                        .map(id_json);
                    (have == expect)
                        .then_some(())
                        .ok_or_else(|| format!("origin.source is {have:?}, not {expect:?}"))
                }),
            )
        }
    }
}

fn project_settings_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let member = *rng.pick(&["numbering", "colour_profile", "size_profile", "xref_format"])?;
    match rng.below(8) {
        // Refused: not a settable member.
        0 => {
            let m = *rng.pick(&["identifier_set", "frames", "wire_default", "bogus"])?;
            Some(Step::cmd("project.settings.set", json!({"member": m, "value": {}})).refuse())
        }
        // Removing a member lets the derivation profile's default apply.
        1 | 2 => Some(Step::cmd("project.settings.set", json!({"member": member, "value": null})).accept()),
        // Setting a member to the value it has changes nothing.
        _ => {
            let value = live
                .fixture
                .initial_settings
                .get(member)
                .or_else(|| live.settings.get(member))?
                .clone();
            Some(Step::cmd("project.settings.set", json!({"member": member, "value": value})).accept())
        }
    }
}

/// A library document `template` (the `lib/` JSON form with `$k`) re-lineaged as the copy of `item` of
/// `library`: library, item, copy identity `v5(NS_LIBCOPY, library ‖ item)` and hash made consistent.
fn relineaged(template: &Value, library: &str, item: &str) -> Option<Value> {
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
fn named_through_lib_ref(q: &ew_derive::Q, id: ew_ids::Id<ew_schema::FnDef>) -> bool {
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
            .accept()
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
fn fndef_cps(copy: &Value) -> std::collections::BTreeSet<u16> {
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
            .accept()
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

fn library_embed(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (_, template) = rng.pick(&live.fixture.form_json)?;
    // A copy of the form with a new lineage: item, library, copy identity and hash made consistent.
    let consistent =
        |rng: &mut SplitMix64| -> Option<Value> { relineaged(template, &rng.uuid(), &rng.uuid()) };
    match rng.below(12) {
        // Refused: the hash is not the hash of the item's members (EW-CORE-014).
        0 => {
            let mut c = consistent(rng)?;
            c["sha256"] = json!(format!("sha256:{}", "00".repeat(32)));
            Some(Step::cmd("library.embed", json!({"copies": [c]})).refuse())
        }
        // Refused: the copy ID is not the copy identity.
        1 => {
            let mut c = consistent(rng)?;
            c["id"] = json!(rng.uuid());
            Some(Step::cmd("library.embed", json!({"copies": [c]})).refuse())
        }
        // Refused: an item of the project itself is created, not embedded.
        2 => {
            let mut c = consistent(rng)?;
            c["library_id"] = live.fixture.project_id.clone();
            Some(Step::cmd("library.embed", json!({"copies": [c]})).refuse())
        }
        // Refused: an item the project has embedded already.
        3 => {
            let mut c = template.clone();
            c["$k"] = json!("form");
            Some(Step::cmd("library.embed", json!({"copies": [c]})).refuse())
        }
        // Refused: the same item twice, and no copies at all.
        4 => {
            let c = consistent(rng)?;
            Some(Step::cmd("library.embed", json!({"copies": [c.clone(), c]})).refuse())
        }
        5 => Some(Step::cmd("library.embed", json!({"copies": []})).refuse()),
        // The FT-316 shape: a symbol naming a function definition that is not embedded, then (on a later step)
        // that function definition; the undo-all oracle undoes both.
        6 => lib_ref_symbol_step(live, rng),
        7 => lib_ref_fndef_step(live, rng),
        _ => {
            let c = consistent(rng)?;
            let id = c["id"].clone();
            Some(
                Step::cmd("library.embed", json!({"copies": [c]}))
                    .accept()
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

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

// ---------------------------------------------------------------------------------------------------------
// Revisions (doc 03 §18, FT-165): revision.create, fields.set and edit.delete on a revision

/// The greatest major number among the labels of the form `V<major>.<minor>`.
fn max_major(live: &Live) -> u32 {
    live.revisions
        .iter()
        .filter_map(|r| r.label.strip_prefix('V')?.split('.').next()?.parse().ok())
        .max()
        .unwrap_or(0)
}

/// A label no revision has, above every major number (the order of labels is the release gate's rule).
fn fresh_label(live: &Live, rng: &mut SplitMix64) -> String {
    format!(
        "V{}.{}",
        max_major(live) + 1 + u32::try_from(rng.below(3)).unwrap_or(0),
        rng.below(5)
    )
}

fn role(rng: &mut SplitMix64) -> &'static str {
    [
        "customer",
        "end_user",
        "panel_builder",
        "author",
        "checker",
        "engineer",
    ][rng.below(6)]
}

/// The oracle of lane A's R5: the title block's `revision.current.label` follows the records. With an open
/// revision, the open one is the current one (it wins over released ones), so its label is the printed one.
/// The title block is read cold over the snapshot; a profile that does not bind the path prints nothing, which
/// is no violation.
fn title_block_follows(after: &ew_derive::Q) -> Result<(), String> {
    let open: Vec<_> = after
        .table::<ew_schema::Revision>()
        .iter()
        .filter(|(_, r)| r.is_open())
        .map(|(_, r)| r.label.as_str().to_owned())
        .collect();
    let Some(page) = after.pages().ids().next() else {
        return Ok(());
    };
    let tb = after.get::<ew_derive::TitleBlockValues>(ew_derive::PageRef::Drawn(page));
    let printed = match tb.get("revision.current.label") {
        Some(ew_derive::TbValue::Text(t)) => Some(t.clone()),
        _ => None,
    };
    match (open.as_slice(), printed) {
        ([label], Some(printed)) if *label != printed => Err(format!(
            "the title block prints the revision `{printed}`, the open revision is `{label}`"
        )),
        _ => Ok(()),
    }
}

fn revision_create(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let label = fresh_label(live, rng);
    let open = live.revisions.iter().any(|r| r.state == "open");
    let create = |p: Value| Step::cmd("revision.create", p);
    match rng.below(12) {
        // Refused: a label another revision has (EW-INV-017).
        0 => {
            let r = rng.pick(&live.revisions)?;
            Some(create(json!({"label": r.label, "state": "released"})).refuse())
        }
        // Refused: the checker is the author's party (EW-INV-018).
        1 => Some(
            create(json!({"label": label, "state": "released", "author": "engineer", "checker": "engineer"}))
                .refuse(),
        ),
        // Refused: a second open revision (EW-INV-016).
        2 if open => Some(create(json!({"label": label})).refuse()),
        // Refused: a historic released revision once a release record exists.
        3 if live.any_release => Some(create(json!({"label": label, "state": "released"})).refuse()),
        // Refused: two notes on one page (EW-INV-019).
        4 => {
            let page = rng.pick(&live.pages)?;
            Some(
                create(json!({"label": label, "state": "released", "page_notes": [{"page": page}, {"page": page}]}))
                    .refuse(),
            )
        }
        // Refused: a note on a page that does not exist.
        5 => Some(
            create(json!({"label": label, "state": "released", "page_notes": [{"page": rng.uuid()}]}))
                .refuse(),
        ),
        _ => {
            // The open revision when there is none; else a historic released one, unless a release record exists.
            let state = if !open {
                "open"
            } else if !live.any_release {
                "released"
            } else {
                return None;
            };
            let mut p = json!({"label": label, "state": state, "description": rng.word(10)});
            if rng.chance(2) {
                p["date"] = json!(format!(
                    "20{}-0{}-1{}",
                    20 + rng.below(6),
                    1 + rng.below(9),
                    rng.below(9)
                ));
            }
            if rng.chance(2) {
                let (a, b) = (role(rng), role(rng));
                if a != b {
                    p["author"] = json!(a);
                    p["checker"] = json!(b);
                }
            }
            if rng.chance(2) {
                let n = 1 + rng.below(2);
                let mut pages: Vec<Value> = (0..n).filter_map(|_| rng.pick(&live.pages).cloned()).collect();
                pages.sort_by_key(Value::to_string);
                pages.dedup();
                p["page_notes"] = Value::Array(pages.iter().map(|g| json!({"page": g})).collect());
            }
            Some(
                create(p)
                    .counts(&[(Table::Revisions, 1)])
                    .check(|_, after| title_block_follows(after)),
            )
        }
    }
}

pub fn revision_fields_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let one = |target: &Value, path: &str, value: Value| {
        Step::cmd(
            "fields.set",
            json!({"kind": "revision", "targets": [target], "path": path, "value": value}),
        )
    };
    // A released or withdrawn revision is immutable (FR-REV-009, EW-INV-020).
    if rng.chance(6) {
        let closed: Vec<_> = live.revisions.iter().filter(|r| r.state != "open").collect();
        let r = *rng.pick(&closed)?;
        let path = *rng.pick(&["label", "date", "description", "page_notes", "author"])?;
        return Some(one(&r.id, path, json!("X")).refuse());
    }
    let r = live.revisions.iter().find(|r| r.state == "open")?;
    let id = r.id.clone();
    let record = move |q: &ew_derive::Q| {
        typed::<ew_schema::Revision>(&id).and_then(|i| q.table::<ew_schema::Revision>().get(i).cloned())
    };
    match rng.below(14) {
        // Refused: the state is not settable.
        0 => Some(one(&r.id, "state", json!("released")).refuse()),
        // Refused: two notes on one page (EW-INV-019).
        1 => {
            let p = rng.pick(&live.pages)?;
            Some(one(&r.id, "page_notes", json!([{"page": p}, {"page": p}])).refuse())
        }
        // Refused: a note takes no member besides `page` and `description`.
        2 => {
            let p = rng.pick(&live.pages)?;
            Some(one(&r.id, "page_notes", json!([{"page": p, "bogus": 1}])).refuse())
        }
        // Refused: a note on a page that does not exist.
        3 => Some(one(&r.id, "page_notes", json!([{"page": rng.uuid()}])).refuse()),
        // Refused: a label another revision has (EW-INV-017).
        4 => {
            let other = live
                .revisions
                .iter()
                .find(|o| o.id != r.id && o.state != "withdrawn")?;
            Some(one(&r.id, "label", json!(other.label)).refuse())
        }
        // Refused: the author's party as the checker (EW-INV-018).
        5 => {
            let author = r.author.clone()?;
            Some(one(&r.id, "checker", json!(author)).refuse())
        }
        6 => {
            let date = format!("20{}-0{}-2{}", 20 + rng.below(6), 1 + rng.below(9), rng.below(8));
            let want = json!(date);
            Some(one(&r.id, "date", want.clone()).accept().check(move |_, after| {
                let have = record(after).and_then(|r| serde_json::to_value(r.date).ok());
                (have == Some(want.clone()))
                    .then_some(())
                    .ok_or_else(|| format!("date is {have:?}, expected {want}"))
            }))
        }
        7 => Some(one(&r.id, "description", json!(rng.word(12))).accept()),
        8 => Some(one(&r.id, "date", Value::Null).accept().check(move |_, after| {
            record(after)
                .is_some_and(|r| r.date.is_none())
                .then_some(())
                .ok_or_else(|| "the date is still set after null".to_owned())
        })),
        9 | 10 => {
            let n = rng.below(3);
            let mut pages: Vec<Value> = (0..n).filter_map(|_| rng.pick(&live.pages).cloned()).collect();
            pages.sort_by_key(Value::to_string);
            pages.dedup();
            let notes = Value::Array(
                pages
                    .iter()
                    .map(|g| json!({"page": g, "description": rng.word(6)}))
                    .collect(),
            );
            Some(one(&r.id, "page_notes", notes).accept().check(move |_, after| {
                let have: Vec<Value> = record(after)
                    .map(|r| r.page_notes.iter().map(|n| id_json(n.page)).collect())
                    .unwrap_or_default();
                (have == pages)
                    .then_some(())
                    .ok_or_else(|| format!("the notes are on {have:?}, expected {pages:?}"))
            }))
        }
        // The author, never the checker's party.
        11 => {
            let new = role(rng);
            if r.checker.as_deref() == Some(new) {
                return None;
            }
            Some(one(&r.id, "author", json!(new)).accept())
        }
        12 => {
            let new = role(rng);
            if r.author.as_deref() == Some(new) {
                return None;
            }
            Some(one(&r.id, "checker", json!(new)).accept())
        }
        // A new label above every other (the order of labels is the release gate's rule); the title block
        // prints it.
        _ => {
            let label = fresh_label(live, rng);
            let want = label.clone();
            Some(one(&r.id, "label", json!(label)).accept().check(move |_, after| {
                let have = record(after).map(|r| r.label.as_str().to_owned());
                if have.as_deref() != Some(want.as_str()) {
                    return Err(format!("label is {have:?}, expected {want}"));
                }
                title_block_follows(after)
            }))
        }
    }
}
