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

use super::live::{Live, PageInfo, PartTemplate, StripInfo, SymInfo, id_json, id_text};
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
    // WP-S2.04 part 3
    Arm {
        command: "box.reshape",
        weight: 3,
        build: box_reshape,
    },
    // WP-S2.04 part 4a
    Arm {
        command: "box.structure",
        weight: 2,
        build: box_structure,
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
    // WP-S2.04 part 3
    Arm {
        command: "connection.draw_to_ip",
        weight: 3,
        build: connection_draw_to_ip,
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
    // WP-S2.05 part 3
    Arm {
        command: "edit.paste",
        weight: 3,
        build: edit_paste,
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
    // WP-S2.05 part 2
    Arm {
        command: "fields.set_many",
        weight: 3,
        build: fields_set_many,
    },
    // WP-S2.04 part 3
    Arm {
        command: "ip.arrange",
        weight: 3,
        build: ip_arrange,
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
    // WP-S2.04 part 4a
    Arm {
        command: "item.place",
        weight: 3,
        build: item_place,
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
    // WP-S2.04 part 4a
    Arm {
        command: "marker.place",
        weight: 3,
        build: marker_place,
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
        command: "parts.import",
        weight: 2,
        build: parts_import,
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
        command: "project.assets.purge",
        weight: 1,
        build: project_assets_purge,
    },
    Arm {
        command: "project.checks.set",
        weight: 1,
        build: project_checks_set,
    },
    Arm {
        command: "project.company.set",
        weight: 1,
        build: project_company_set,
    },
    Arm {
        command: "project.drawing.set",
        weight: 1,
        build: project_drawing_set,
    },
    Arm {
        command: "project.frames.set",
        weight: 1,
        build: project_frames_set,
    },
    Arm {
        command: "project.languages.set",
        weight: 1,
        build: project_languages_set,
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
        command: "rep.exchange_symbol",
        weight: 3,
        build: rep_exchange_symbol,
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
    // WP-S2.04 part 4a: page items and structure boxes move with `items`.
    if rng.chance(4)
        && let Some(step) = rep_move_items(live, rng)
    {
        return Some(step);
    }
    let rep = rng.pick(&live.reps)?.clone();
    match rng.below(8) {
        // Refused: grid steps beside a non-zero delta.
        0 => {
            let p = json!({"reps": [rep], "delta": [MOVE_UNIT, 0], "steps": [1, 0]});
            return Some(Step::cmd("rep.move", p).refuse());
        }
        // Refused: grid steps with a target page (a move to another page takes a delta).
        1 => {
            let page = rng.pick(&live.pages)?;
            let p = json!({"reps": [rep], "steps": [1, -1], "page": page});
            return Some(Step::cmd("rep.move", p).refuse());
        }
        // Grid steps: the representation lands on the grid point at its grid index plus the steps. Either: a
        // representation off the grid, or a route that cannot follow, is refused.
        2 | 3 => {
            let steps = [rng.range(-2, 2), rng.range(-2, 2)];
            let rep_id = rep.clone();
            return Some(
                Step::cmd("rep.move", json!({"reps": [rep], "steps": steps})).check(move |before, after| {
                    let id = typed(&rep_id).ok_or("a rep ID")?;
                    let (Some(b), Some(a)) = (before.reps().get(id), after.reps().get(id)) else {
                        return Err("the moved representation disappeared".into());
                    };
                    let grid = after
                        .pages()
                        .get(a.page)
                        .ok_or("the page of the representation")?
                        .grid;
                    let at = |x: ew_units::Nm, k: i64| grid.at(grid.snap(x) + k);
                    if a.page == b.page
                        && Some(a.pos.x) == at(b.pos.x, steps[0])
                        && Some(a.pos.y) == at(b.pos.y, steps[1])
                    {
                        Ok(())
                    } else {
                        Err("the representation is not at its grid index plus the steps".into())
                    }
                }),
            );
        }
        _ => {}
    }
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

/// `rep.move` with `items` alone: a page item or structure box moves by a delta; refused across pages, with an
/// item that does not exist, or with nothing named.
fn rep_move_items(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let mut all: Vec<(Value, Value)> = live.page_items.clone();
    all.extend(live.structure_boxes.iter().cloned());
    match rng.below(6) {
        0 => Some(Step::cmd("rep.move", json!({"reps": [], "delta": [MOVE_UNIT, 0]})).refuse()),
        1 => Some(
            Step::cmd(
                "rep.move",
                json!({"reps": [], "items": [rng.uuid()], "delta": [MOVE_UNIT, 0]}),
            )
            .refuse(),
        ),
        2 => {
            let (item, _) = rng.pick(&all)?.clone();
            let page = rng.pick(&live.pages)?.clone();
            let p = json!({"reps": [], "items": [item], "delta": [MOVE_UNIT, 0], "page": page});
            Some(Step::cmd("rep.move", p).refuse())
        }
        _ => {
            let (item, _) = rng.pick(&all)?.clone();
            let delta = [rng.range(-2, 2) * MOVE_UNIT, rng.range(-2, 2) * MOVE_UNIT];
            Some(
                Step::cmd("rep.move", json!({"reps": [], "items": [item], "delta": delta}))
                    .accept()
                    .counts(&[(Table::PageItems, 0), (Table::StructureBoxes, 0)]),
            )
        }
    }
}

/// A random graphic shape on `page`, valid unless `degenerate`.
fn shape(page: &PageInfo, rng: &mut SplitMix64, degenerate: bool) -> Value {
    let p = |rng: &mut SplitMix64| Live::grid_pos(page, rng.range(1, 60), rng.range(1, 40));
    let a = p(rng);
    if degenerate {
        return match rng.below(4) {
            0 => json!({"line": {"a": a.clone(), "b": a}}),
            1 => json!({"polygon": {"points": [a.clone(), a.clone(), a]}}),
            2 => json!({"circle": {"centre": a, "r": 0}}),
            _ => json!({"arc": {"centre": a, "r": 1_000_000, "sweep": 0}}),
        };
    }
    let b = Live::grid_pos(page, rng.range(61, 99), rng.range(41, 79));
    match rng.below(6) {
        0 => json!({"line": {"a": a, "b": b}}),
        1 => json!({"polyline": {"points": [a, b, p(rng)]}}),
        2 => json!({"rect": {"min": b, "max": a}}),
        3 => json!({"circle": {"centre": a, "r": 1_000_000 + rng.range(0, 5_000_000)}}),
        4 => json!({"arc": {"centre": a, "r": 2_000_000, "start": 90_000, "sweep": -180_000}}),
        _ => json!({"polygon": {"points": [[0, 0], [5_000_000, 0], [0, 5_000_000]]}}),
    }
}

/// `item.place`: graphics and free texts on a page; refused on a missing page, with a degenerate shape, an unknown
/// bindable name, an empty text without `bind`, an angle off the quarter turns or an unknown member.
fn item_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let page = rng.pick(&live.page_info)?;
    let cmd = |p: Value| Step::cmd("item.place", p);
    let at = Live::grid_pos(page, rng.range(1, 99), rng.range(1, 70));
    match rng.below(10) {
        0 => Some(cmd(json!({"page": rng.uuid(), "kind": {"free_text": {"text": "x", "at": at}}})).refuse()),
        1 => Some(
            cmd(json!({"page": page.id, "kind": {"graphic": {"shape": shape(page, rng, true)}}})).refuse(),
        ),
        2 => Some(
            cmd(json!({"page": page.id, "kind": {"free_text": {"bind": "project.reference", "at": at}}}))
                .refuse(),
        ),
        3 => Some(cmd(json!({"page": page.id, "kind": {"free_text": {"text": "", "at": at}}})).refuse()),
        4 => Some(
            cmd(json!({"page": page.id, "kind": {"free_text": {"text": "x", "at": at, "angle": 45_000}}}))
                .refuse(),
        ),
        5 => Some(
            cmd(json!({"page": page.id, "kind": {"graphic": {"shape": shape(page, rng, false)}, "z": 1}}))
                .refuse(),
        ),
        6 | 7 => {
            let dash = ["solid", "dashed", "dotted", "dash_dot"][rng.below(4)];
            let p = json!({"page": page.id, "kind": {"graphic": {"shape": shape(page, rng, false),
                "dash": dash, "fill": rng.chance(2)}}});
            Some(cmd(p).accept().counts(&[(Table::PageItems, 1)]))
        }
        _ => {
            let text = if rng.chance(3) {
                json!({"bind": ew_derive::BINDABLE[rng.below(6)], "at": at})
            } else {
                let angle = [0, 90_000, 180_000, 270_000][rng.below(4)];
                let align = ["left", "centre", "right"][rng.below(3)];
                json!({"text": "40x60", "at": at, "angle": angle, "align": align})
            };
            Some(
                cmd(json!({"page": page.id, "kind": {"free_text": text}}))
                    .accept()
                    .counts(&[(Table::PageItems, 1)]),
            )
        }
    }
}

/// `marker.place`: a group of a page's connections; refused with no connection, a duplicate, a connection of
/// another page or one that does not exist, an angle other than 0 or 90000, a colour count that does not match or a
/// foreign colour code.
fn marker_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (page, conns) = rng.pick(&live.page_connections)?.clone();
    let cmd = |p: Value| Step::cmd("marker.place", p);
    let at = json!([MOVE_UNIT, MOVE_UNIT]);
    let one = rng.pick(&conns)?.clone();
    match rng.below(10) {
        0 => Some(cmd(json!({"page": page, "connections": [], "at": at})).refuse()),
        1 => Some(cmd(json!({"page": page, "connections": [one.clone(), one], "at": at})).refuse()),
        2 => Some(cmd(json!({"page": page, "connections": [rng.uuid()], "at": at})).refuse()),
        3 => {
            let other = live
                .page_connections
                .iter()
                .filter(|(p, _)| *p != page)
                .flat_map(|(_, c)| c.iter())
                .next()
                .cloned()?;
            Some(cmd(json!({"page": page, "connections": [other], "at": at})).refuse())
        }
        4 => Some(cmd(json!({"page": page, "connections": [one], "at": at, "angle": 45_000})).refuse()),
        5 => Some(
            cmd(json!({"page": page, "connections": [one], "at": at, "colours": [["BK"], ["BU"]]})).refuse(),
        ),
        6 => Some(
            cmd(json!({"page": page, "connections": [one], "at": at, "colours": [[{"foreign": "XX"}]]}))
                .refuse(),
        ),
        _ => {
            let mut chosen: Vec<Value> = conns.clone();
            chosen.truncate(1 + rng.below(3));
            if rng.chance(2) {
                chosen.reverse();
            }
            let n = chosen.len();
            let mut p = json!({"page": page, "connections": chosen, "at": at});
            if rng.chance(2) {
                p["angle"] = json!(90_000);
            }
            if rng.chance(2) {
                p["csa"] = json!("1.5");
                p["colours"] = Value::Array((0..n).map(|_| json!(["BK"])).collect());
            }
            Some(cmd(p).accept().counts(&[(Table::PageItems, 1)]))
        }
    }
}

/// `box.structure`: a box on a page assigning a location node; refused with fewer than three vertices, on a missing
/// page, with a node in a slot of another aspect or a missing external reference.
fn box_structure(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let page = rng.pick(&live.page_info)?;
    let cmd = |p: Value| Step::cmd("box.structure", p);
    let outline = json!([[0, 0], [50_000_000, 0], [50_000_000, 30_000_000], [0, 30_000_000]]);
    let assigns = match rng.pick(&live.locations) {
        Some(n) => json!({"location": n}),
        None => json!({}),
    };
    match rng.below(8) {
        0 => Some(
            cmd(json!({"page": page.id, "outline": [[0, 0], [1, 1]], "style": "dotted", "assigns": {}}))
                .refuse(),
        ),
        1 => Some(
            cmd(json!({"page": rng.uuid(), "outline": outline, "style": "dotted", "assigns": {}})).refuse(),
        ),
        2 => Some(
            cmd(
                json!({"page": page.id, "outline": outline, "style": "dotted", "assigns": {},
                "external": rng.uuid()}),
            )
            .refuse(),
        ),
        3 => {
            let wrong = live.nodes.iter().find(|n| n.aspect != "location")?;
            Some(
                cmd(json!({"page": page.id, "outline": outline, "style": "dotted",
                    "assigns": {"location": wrong.id}}))
                .refuse(),
            )
        }
        _ => {
            let style = ["dotted", "dash_dot"][rng.below(2)];
            let mut p = json!({"page": page.id, "outline": outline, "style": style, "assigns": assigns});
            if rng.chance(2) {
                p["caption"] = json!("External Connection");
            }
            if let Some(e) = rng.pick(&live.ext_refs) {
                p["external"] = e.clone();
            }
            Some(cmd(p).accept().counts(&[(Table::StructureBoxes, 1)]))
        }
    }
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

fn rep_exchange_symbol(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let rep = rng.pick(&live.rep_infos)?;
    let fits_page = |s: &&SymInfo| allowed(&rep.page_kind).contains(&s.rep_type.as_str());
    match rng.below(12) {
        // Refused: the representation does not exist.
        0 => {
            let symbol = rng.pick(&live.symbols)?;
            let p = json!({"rep": rng.uuid(), "symbol": symbol.id});
            Some(Step::cmd("rep.exchange_symbol", p).refuse())
        }
        // Refused: the symbol does not exist.
        1 => Some(
            Step::cmd(
                "rep.exchange_symbol",
                json!({"rep": rep.id, "symbol": rng.uuid()}),
            )
            .refuse(),
        ),
        // Refused: an interruption point keeps its symbol.
        2 => {
            let ips: Vec<_> = live.rep_infos.iter().filter(|r| r.ip).collect();
            let ip = *rng.pick(&ips)?;
            let symbol = rng.pick(&live.symbols)?;
            Some(Step::cmd("rep.exchange_symbol", json!({"rep": ip.id, "symbol": symbol.id})).refuse())
        }
        // Refused: a representation type the page does not allow.
        3 => {
            let wrong: Vec<_> = live.symbols.iter().filter(|s| !fits_page(s)).collect();
            let symbol = *rng.pick(&wrong)?;
            Some(Step::cmd("rep.exchange_symbol", json!({"rep": rep.id, "symbol": symbol.id})).refuse())
        }
        // Refused: an explicit CP map that names a symbol CP the symbol does not have.
        4 => {
            let symbol = rng.pick(&live.symbols)?;
            let p = json!({"rep": rep.id, "symbol": symbol.id,
                "pin_map": [{"symbol_cp": 60_000, "function_cp": 0}]});
            Some(Step::cmd("rep.exchange_symbol", p).refuse())
        }
        // Refused: an explicit CP map that maps a symbol CP twice.
        5 => {
            let symbol = rng.pick(&live.symbols)?;
            let p = json!({"rep": rep.id, "symbol": symbol.id,
                "pin_map": [{"symbol_cp": 0, "function_cp": 0}, {"symbol_cp": 0, "function_cp": 1}]});
            Some(Step::cmd("rep.exchange_symbol", p).refuse())
        }
        // The symbol of another function definition or of the same one. Either: a connected function CP that the
        // new symbol does not draw, and a symbol CP that has no counterpart by designation, are refused (the
        // generator does not model the CPs).
        _ => {
            if rep.ip {
                return None;
            }
            let any: Vec<&SymInfo> = live.symbols.iter().filter(fits_page).collect();
            let same: Vec<&SymInfo> = any
                .iter()
                .copied()
                .filter(|s| s.fndef.as_ref() == Some(&rep.fndef))
                .collect();
            let symbol = if rng.chance(3) || same.is_empty() {
                *rng.pick(&any)?
            } else {
                *rng.pick(&same)?
            };
            let mut p = json!({"rep": rep.id, "symbol": symbol.id});
            if rng.chance(5) {
                let cp = rng.pick(&symbol.cps)?;
                p["pin_map"] = json!([{"symbol_cp": cp, "function_cp": 0}]);
            }
            let (rep_id, symbol_id) = (rep.id.clone(), symbol.id.clone());
            Some(
                Step::cmd("rep.exchange_symbol", p)
                    .counts(&[
                        (Table::Reps, 0),
                        (Table::Functions, 0),
                        (Table::Devices, 0),
                        (Table::Connections, 0),
                    ])
                    .check(move |before, after| {
                        let id = typed(&rep_id).ok_or("a rep ID")?;
                        let new = typed(&symbol_id).ok_or("a symbol ID")?;
                        let (Some(b), Some(a)) = (before.reps().get(id), after.reps().get(id)) else {
                            return Err("the representation disappeared".into());
                        };
                        (a.symbol == new
                            && a.variant == 0
                            && (a.pos, a.orient, a.page) == (b.pos, b.orient, b.page)
                            && a.functions == b.functions)
                            .then_some(())
                            .ok_or_else(|| {
                                "the representation is not the old one with the new symbol".to_owned()
                            })
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

// WP-S2.05 part 3
/// `edit.paste`: a fragment of one new device with its main function drawn by an embedded symbol on a page that takes
/// it, pasted at a grid point of that page (the copies are held, so nothing is embedded), with a free-text page item
/// on the source page; refused: no record, another grid, a point off the grid, a record of a kind that is not pasted,
/// and a representation whose function is not in the fragment (a mandatory reference). The boundary case pastes a
/// wire marker that names a connection outside the fragment: the marker is left out, the paste succeeds.
fn edit_paste(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
        0 => Some(Step::cmd("edit.paste", params(&at, fragment(Vec::new(), page.grid))).refuse()),
        1 => {
            let other = if page.grid == (4_000_000, 1) {
                (7_937_500, 3)
            } else {
                (4_000_000, 1)
            };
            Some(Step::cmd("edit.paste", params(&at, fragment(all(), other))).refuse())
        }
        2 => {
            let off = json!([at[0].as_i64()? + 1, at[1]]);
            Some(Step::cmd("edit.paste", params(&off, fragment(all(), page.grid))).refuse())
        }
        3 => {
            let foreign = json!({"$k": "no_such_kind", "id": rng.uuid()});
            Some(Step::cmd("edit.paste", params(&at, fragment(vec![foreign], page.grid))).refuse())
        }
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
                .accept()
                .counts(&[
                    (Table::Reps, 1),
                    (Table::Functions, 1),
                    (Table::Devices, 1),
                    (Table::Connections, 0),
                    (Table::PageItems, 0),
                ]),
            )
        }
        _ => Some(
            Step::cmd("edit.paste", params(&at, fragment(all(), page.grid)))
                .accept()
                .counts(&[
                    (Table::Reps, 1),
                    (Table::Functions, 1),
                    (Table::Devices, 1),
                    (Table::Connections, 0),
                    (Table::PageItems, 1),
                ]),
        ),
    }
}

// WP-S2.05 part 2
/// `fields.set_many`: one function text per device (distinct devices, one entry each), checked afterwards; refused: no
/// entry, a (target, path) pair in two entries, and an entry `fields.set` would refuse (a path that is not settable).
fn fields_set_many(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let entry = |device: &Value, value: &str| json!({"kind": "device", "targets": [device], "path": "function_text", "value": value});
    match rng.below(8) {
        0 => Some(Step::cmd("fields.set_many", json!({"edits": []})).refuse()),
        1 => {
            let d = rng.pick(&live.plain_devices)?;
            Some(
                Step::cmd(
                    "fields.set_many",
                    json!({"edits": [entry(d, "A"), entry(d, "B")]}),
                )
                .refuse(),
            )
        }
        2 => {
            let d = rng.pick(&live.plain_devices)?;
            let bad = json!({"kind": "device", "targets": [d], "path": "parent", "value": null});
            Some(Step::cmd("fields.set_many", json!({"edits": [entry(d, "A"), bad]})).refuse())
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
// WP-S2.04 part 3: connection.draw_to_ip, ip.arrange, box.reshape

fn connection_draw_to_ip(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (page_id, functions) = rng.pick(&live.page_functions)?;
    let page = live
        .page_info
        .iter()
        .find(|p| &p.id == page_id && p.kind == "multi_line")?;
    let a = rng.pick(functions)?;
    let n = i64::try_from(page.grid.0).unwrap_or(1);
    let name = format!("FZ-{}", rng.below(1000));
    let direction = *rng.pick(&["source", "target", "undirected"])?;
    let pos = on_page(page, rng);
    let star_source = rng.chance(4);
    let sort = rng.chance(3).then(|| rng.range(1, 9));
    let idx = small_idx(rng);
    let end = rng.pick(&live.ip_end_syms).cloned();
    let ip = |symbol: &Value, def: &Value| {
        let mut ip = json!({
            "symbol": symbol, "fndef": def, "pos": pos, "name": name, "direction": direction,
            "star_source": star_source,
        });
        if let Some(sort) = sort {
            ip["sort"] = json!(sort);
        }
        ip
    };
    let step = |a: Value, page: &Value, ip: Value| json!({"a": a, "page": page, "ip": ip});
    match rng.below(10) {
        // Refused: the page does not exist.
        0 => {
            let (symbol, def) = end?;
            let p = step(cp_end(a, idx), &json!(rng.uuid()), ip(&symbol, &def));
            Some(Step::cmd("connection.draw_to_ip", p).refuse())
        }
        // Refused: the first end names a function that does not exist, or a junction that does not.
        1 => {
            let (symbol, def) = end?;
            let a = if rng.chance(2) {
                cp_end(&json!(rng.uuid()), idx)
            } else {
                json!({"junction": {"junction": rng.uuid()}})
            };
            let p = step(a, &page.id, ip(&symbol, &def));
            Some(Step::cmd("connection.draw_to_ip", p).refuse())
        }
        // Refused: the insertion point is off the page's connection grid.
        2 => {
            let (symbol, def) = end?;
            let mut ip = ip(&symbol, &def);
            ip["pos"] = json!([2 * n + 1, 3 * n]);
            let p = step(cp_end(a, idx), &page.id, ip);
            Some(Step::cmd("connection.draw_to_ip", p).refuse())
        }
        // Refused: the symbol does not exist, or its function definition is not embedded.
        3 => {
            let (symbol, def) = end?;
            let ip = if rng.chance(2) {
                ip(&json!(rng.uuid()), &def)
            } else {
                ip(&symbol, &json!(rng.uuid()))
            };
            let p = step(cp_end(a, idx), &page.id, ip);
            Some(Step::cmd("connection.draw_to_ip", p).refuse())
        }
        // Refused: a route that is off the grid and does not end at the interruption point's CP.
        4 => {
            let (symbol, def) = end?;
            let mut p = step(cp_end(a, idx), &page.id, ip(&symbol, &def));
            p["route"] = json!([[1, 1], [2, 2]]);
            Some(Step::cmd("connection.draw_to_ip", p).refuse())
        }
        // The wire and its interruption point: one function, one representation, one connection. Either: the
        // generator does not model the prevent rules.
        _ => {
            let (symbol, def) = end?;
            let p = step(cp_end(a, idx), &page.id, ip(&symbol, &def));
            let name = name.clone();
            Some(
                Step::cmd("connection.draw_to_ip", p)
                    .counts(&[
                        (Table::Reps, 1),
                        (Table::Functions, 1),
                        (Table::Devices, 0),
                        (Table::Connections, 1),
                    ])
                    .check(move |_, after| {
                        let found = after.functions().iter().any(
                            |(_, f)| matches!(&f.kind, ew_schema::FunctionKind::Ip(s) if s.name.as_str() == name),
                        );
                        found
                            .then_some(())
                            .ok_or_else(|| format!("no interruption point named `{name}` afterwards"))
                    }),
            )
        }
    }
}

fn ip_arrange(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let mut by_page: std::collections::BTreeMap<String, Vec<&Value>> = std::collections::BTreeMap::new();
    for ip in &live.ip_infos {
        by_page.entry(ip.page.to_string()).or_default().push(&ip.function);
    }
    let groups: Vec<&Vec<&Value>> = by_page.values().filter(|g| g.len() >= 2).collect();
    let any_ip = rng.pick(&live.ip_infos)?;
    match rng.below(10) {
        // Refused: fewer than two interruption points.
        0 => {
            let ips = if rng.chance(2) {
                vec![]
            } else {
                vec![any_ip.function.clone()]
            };
            Some(Step::cmd("ip.arrange", json!({"ips": ips})).refuse())
        }
        // Refused: an interruption point twice.
        1 => {
            let other = rng.pick(&live.ip_infos)?;
            let ips = if rng.chance(2) {
                vec![any_ip.function.clone(), any_ip.function.clone()]
            } else {
                vec![
                    any_ip.function.clone(),
                    other.function.clone(),
                    any_ip.function.clone(),
                ]
            };
            Some(Step::cmd("ip.arrange", json!({"ips": ips})).refuse())
        }
        // Refused: a function that does not exist.
        2 => Some(Step::cmd("ip.arrange", json!({"ips": [any_ip.function, rng.uuid()]})).refuse()),
        // Refused: a function that is not an interruption point.
        3 => {
            let plain: Vec<_> = live.functions.iter().filter(|f| !f.ip).collect();
            let f = rng.pick(&plain)?;
            Some(Step::cmd("ip.arrange", json!({"ips": [any_ip.function, f.id]})).refuse())
        }
        // Refused: interruption points of two pages.
        4 => {
            let other: Vec<_> = live.ip_infos.iter().filter(|i| i.page != any_ip.page).collect();
            let other = *rng.pick(&other)?;
            Some(Step::cmd("ip.arrange", json!({"ips": [any_ip.function, other.function]})).refuse())
        }
        // Interruption points of one page in a new order: each takes the slot of its place in the order, the
        // slots being the old positions sorted by (y, x).
        _ => {
            let group = *rng.pick(&groups)?;
            let mut ips: Vec<Value> = group.iter().map(|f| (*f).clone()).collect();
            for i in (1..ips.len()).rev() {
                ips.swap(i, rng.below(i + 1));
            }
            let keep = 2 + rng.below(ips.len() - 1);
            ips.truncate(keep);
            let set_sort = rng.chance(2);
            let mut p = json!({"ips": ips});
            if set_sort {
                p["set_sort"] = json!(true);
            }
            Some(
                Step::cmd("ip.arrange", p)
                    .accept()
                    .counts(&[(Table::Reps, 0), (Table::Functions, 0), (Table::Connections, 0)])
                    .check(move |before, after| {
                        let ids: Vec<ew_ids::Id<ew_schema::Function>> = ips
                            .iter()
                            .map(typed)
                            .collect::<Option<_>>()
                            .ok_or("a function ID")?;
                        let at = |q: &ew_derive::Q, f: ew_ids::Id<ew_schema::Function>| {
                            let rep = q.idx().reps_of_function(f).next()?;
                            let rep = q.reps().get(rep)?;
                            Some((rep.pos.y.0, rep.pos.x.0))
                        };
                        let mut slots: Vec<(i64, i64)> = ids
                            .iter()
                            .map(|f| at(before, *f))
                            .collect::<Option<_>>()
                            .ok_or("an interruption point without a representation")?;
                        slots.sort_unstable();
                        for (i, f) in ids.iter().enumerate() {
                            let got = at(after, *f).ok_or("the representation disappeared")?;
                            if got != slots[i] {
                                return Err(format!(
                                    "interruption point {i} is at {got:?}, its slot is {:?}",
                                    slots[i]
                                ));
                            }
                            let now = after.functions().get(*f).ok_or("the function disappeared")?;
                            let was = before.functions().get(*f).ok_or("the function was not there")?;
                            let (ew_schema::FunctionKind::Ip(now), ew_schema::FunctionKind::Ip(old)) =
                                (&now.kind, &was.kind)
                            else {
                                return Err("not an interruption point".into());
                            };
                            let want = if set_sort {
                                Some(i32::try_from(i + 1).unwrap_or(i32::MAX))
                            } else {
                                old.sort
                            };
                            if now.sort != want {
                                return Err(format!("sort of {i} is {:?}, expected {want:?}", now.sort));
                            }
                        }
                        Ok(())
                    }),
            )
        }
    }
}

/// What a reshaped box must satisfy afterwards: the outline as given, the CPs of these indices, each on the
/// rectangle's outline and, in absolute terms, on the page grid.
fn box_after_check(
    rep: Value,
    outline: Vec<(i64, i64)>,
    mut idxs: Vec<u16>,
) -> impl Fn(&ew_derive::Q, &ew_derive::Q) -> Result<(), String> + 'static {
    idxs.sort_unstable();
    move |_, after| {
        let id = typed(&rep).ok_or("a rep ID")?;
        let r = after.reps().get(id).ok_or("the representation disappeared")?;
        let spec = r.box_.as_ref().ok_or("the box disappeared")?;
        let got: Vec<(i64, i64)> = spec.outline.iter().map(|p| (p.x.0, p.y.0)).collect();
        if got != outline {
            return Err(format!("the outline is {got:?}, expected {outline:?}"));
        }
        let mut have: Vec<u16> = spec.cps.iter().map(|c| c.idx).collect();
        have.sort_unstable();
        if have != idxs {
            return Err(format!("the CPs are {have:?}, expected {idxs:?}"));
        }
        let grid = after.pages().get(r.page).ok_or("the page of the box")?.grid;
        let (l, t, rt, b) = (outline[0].0, outline[0].1, outline[2].0, outline[2].1);
        for c in &spec.cps {
            let (x, y) = (c.pos.x.0, c.pos.y.0);
            let on_vertical = (x == l || x == rt) && (t..=b).contains(&y);
            let on_horizontal = (y == t || y == b) && (l..=rt).contains(&x);
            if !(on_vertical || on_horizontal) {
                return Err(format!("CP {} at ({x}, {y}) is off the outline", c.idx));
            }
            let o = r.orient.apply(c.pos);
            let abs = (r.pos.x.0 + o.x.0, r.pos.y.0 + o.y.0);
            let on_grid = |v: i64| grid.snap_point(ew_units::Nm(v)) == Some(ew_units::Nm(v));
            if !(on_grid(abs.0) && on_grid(abs.1)) {
                return Err(format!("CP {} at {abs:?} is off the grid", c.idx));
            }
        }
        Ok(())
    }
}

fn box_reshape(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let b = rng.pick(&live.box_infos)?;
    let n = b.grid_num;
    let (x0, y0) = b.outline[0];
    let (x1, y1) = b.outline[2];
    let x1w = x1 + rng.range(1, 3) * n;
    let widened = json!([[x0, y0], [x1w, y0], [x1w, y1], [x0, y1]]);
    let widened_pts = vec![(x0, y0), (x1w, y0), (x1w, y1), (x0, y1)];
    let idxs: Vec<u16> = b.cps.iter().map(|c| c.0).collect();
    let counts = [(Table::Reps, 0), (Table::Functions, 0), (Table::Connections, 0)];
    match rng.below(11) {
        // Refused: the representation does not exist.
        0 => Some(Step::cmd("box.reshape", json!({"rep": rng.uuid(), "outline": widened})).refuse()),
        // Refused: a representation that has no box.
        1 => {
            let rep = rng.pick(&live.plain_reps)?;
            Some(Step::cmd("box.reshape", json!({"rep": rep, "outline": widened})).refuse())
        }
        // Refused: an outline of fewer than three vertices.
        2 => Some(
            Step::cmd(
                "box.reshape",
                json!({"rep": b.rep, "outline": [[x0, y0], [x1w, y0]]}),
            )
            .refuse(),
        ),
        // Refused: a vertex off the page's connection grid.
        3 => {
            let outline = json!([[x0, y0], [x1w, y0], [x1w + 1, y1], [x0, y1]]);
            Some(Step::cmd("box.reshape", json!({"rep": b.rep, "outline": outline})).refuse())
        }
        // Refused: two CPs with one index.
        4 => {
            let cps = json!([
                {"idx": 0, "pos": [x0, y0], "dir": "up"},
                {"idx": 0, "pos": [x1w, y0], "dir": "up"},
            ]);
            Some(
                Step::cmd(
                    "box.reshape",
                    json!({"rep": b.rep, "outline": widened, "cps": cps}),
                )
                .refuse(),
            )
        }
        // Refused: a CP off the outline, or a CP off the grid.
        5 | 6 => {
            let pos = if rng.chance(2) {
                [x0 - n, y0 - n]
            } else {
                [x0 + 1, y0]
            };
            let cps = json!([{"idx": 0, "pos": pos, "dir": "up"}]);
            Some(
                Step::cmd(
                    "box.reshape",
                    json!({"rep": b.rep, "outline": widened, "cps": cps}),
                )
                .refuse(),
            )
        }
        // No CPs at all. Either: a CP that a connection on the page ends at may not be dropped.
        7 => Some(
            Step::cmd(
                "box.reshape",
                json!({"rep": b.rep, "outline": widened, "cps": []}),
            )
            .counts(&counts),
        ),
        // The CPs listed again, those on the right edge moved with it.
        8 => {
            let cps: Vec<Value> = b
                .cps
                .iter()
                .map(|(idx, (x, y), dir)| {
                    let x = if *x == x1 { x1w } else { *x };
                    json!({"idx": idx, "pos": [x, y], "dir": dir})
                })
                .collect();
            Some(
                Step::cmd(
                    "box.reshape",
                    json!({"rep": b.rep, "outline": widened, "cps": cps}),
                )
                .accept()
                .counts(&counts)
                .check(box_after_check(b.rep.clone(), widened_pts, idxs)),
            )
        }
        // The CPs re-projected onto the widened rectangle.
        _ => Some(
            Step::cmd("box.reshape", json!({"rep": b.rep, "outline": widened}))
                .accept()
                .counts(&counts)
                .check(box_after_check(b.rep.clone(), widened_pts, idxs)),
        ),
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

/// A part copy whose function-template entries are all embedded and that has exactly one main entry (`one_main`; or
/// does not), with one of its entries that is a black box (or not) and that entry's function-definition copy.
fn template_entry<'a>(
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
fn fresh_designation(strip: &StripInfo, rng: &mut SplitMix64) -> String {
    loop {
        let d = format!("Z{}", rng.below(10_000));
        if !strip.terminals.iter().any(|(_, t)| *t == d) {
            break d;
        }
    }
}

/// The `from_part` target of a part copy's entry.
fn from_part_target(part: &Value, template: usize, letter: &str) -> Value {
    json!({"from_part": {"part": part, "template": template, "letter": letter}})
}

fn device_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
            Some(Step::cmd("device.place", p).accept().counts(&[
                (Table::Reps, 1),
                (Table::Functions, entries),
                (Table::Devices, 1),
                (Table::Connections, 0),
            ]))
        }
        // Refused: a `from_part` target that cannot be made.
        21 => {
            let target = match rng.below(6) {
                // The part copy does not exist.
                0 => from_part_target(&json!(rng.uuid()), 0, "K"),
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
                    return Some(Step::cmd("device.place", p).refuse());
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
            Some(Step::cmd("device.place", base(target)).refuse())
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
                    .accept()
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
        // Refused: a `new_terminal` target that cannot be made.
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
            Some(Step::cmd("device.place", base(json!({"new_terminal": target}))).refuse())
        }
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
    match rng.below(15) {
        // Refused: a template without exactly one main entry (EW-INV-002: the device needs exactly one main
        // function).
        14 => {
            let (t, i, _) = template_entry(live, rng, true, false)?;
            p["target"] = from_part_target(&t.part, i, "A");
            Some(Step::cmd("box.black", p).refuse())
        }
        // A black-box entry of a part copy: the device with a function per template entry, the entry drawn.
        10 | 11 => {
            let (t, i, _) = template_entry(live, rng, true, true)?;
            p["target"] = from_part_target(&t.part, i, "A");
            let entries = isize::try_from(t.entries.len()).unwrap_or(1);
            Some(Step::cmd("box.black", p).accept().counts(&[
                (Table::Reps, 1),
                (Table::Functions, entries),
                (Table::Devices, 1),
            ]))
        }
        // Refused: an entry that is no black box belongs to `device.place`.
        12 => {
            let (t, i, _) = template_entry(live, rng, false, true)?;
            p["target"] = from_part_target(&t.part, i, "A");
            Some(Step::cmd("box.black", p).refuse())
        }
        // Refused: a black box is no terminal.
        13 => {
            let strip = rng.pick(&live.strip_infos)?;
            p["target"] =
                json!({"new_terminal": {"strip": strip.id, "designation": fresh_designation(strip, rng)}});
            Some(Step::cmd("box.black", p).refuse())
        }
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
    match rng.below(12) {
        // FT-341: the form from the library with its closure; the golden project holds the same items, so the
        // held copies are used and only the block is new.
        10 => {
            let (nogrid, title_block) = shipped_report_forms();
            let s = rng.pick(&live.section_info)?;
            Some(
                Step::cmd(
                    "report_block.create",
                    json!({"source": {"kind": kind}, "form": nogrid["id"], "form_copies": [nogrid, title_block],
                           "section": s.id, "first_folio": 6100 + rng.below(50)}),
                )
                .counts(&[(Table::ReportBlocks, 1)]),
            )
        }
        // Refused: a form copy whose hash is not its item's.
        11 => {
            let (mut nogrid, _) = shipped_report_forms();
            nogrid["sheet"]["width"] = json!(1 + rng.below(1000));
            Some(
                Step::cmd(
                    "report_block.create",
                    json!({"source": {"kind": kind}, "form": nogrid["id"], "form_copies": [nogrid],
                           "first_folio": 6200}),
                )
                .refuse(),
            )
        }
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

/// The shipped frame without the grid and the title block it names (`content/forms`), in the `lib/` JSON form.
fn shipped_report_forms() -> (Value, Value) {
    let parse = |text: &str| serde_json::from_str::<Value>(text).expect("a shipped form parses");
    (
        parse(include_str!(
            "../../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"
        )),
        parse(include_str!(
            "../../../../content/forms/ew.titleblock.qet-a4.json"
        )),
    )
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
            let candidates: Vec<_> = live
                .plain_devices
                .iter()
                .filter(|id| !live.cables.iter().any(|cable| cable.id == **id))
                .collect();
            let d = rng.pick(&candidates)?;
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

/// The check schemes `project.checks.set` takes (`ew_commands::commands::SHIPPED_CHECK_SCHEMES`).
const CHECK_SCHEMES: [&str; 4] = ["drafting", "full", "migration", "pre-delivery"];

/// The stored `checks` settings member (`None` when absent).
fn checks_member(q: &ew_derive::Q) -> Option<Value> {
    q.project().settings.extra.get("checks").cloned()
}

fn project_checks_set(_live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(6) {
        // Refused: a scheme that does not ship.
        0 => {
            let member = *rng.pick(&["active_scheme", "release_scheme"])?;
            let name = *rng.pick(&["lenient", "strict", "pre_delivery", "Full"])?;
            Some(Step::cmd("project.checks.set", json!({ member: name })).refuse())
        }
        // Refused: an unknown member.
        1 => Some(Step::cmd("project.checks.set", json!({"scheme": "full"})).refuse()),
        _ => {
            let mut params = serde_json::Map::new();
            for member in ["active_scheme", "release_scheme"] {
                if rng.chance(2) {
                    params.insert(member.into(), json!(*rng.pick(&CHECK_SCHEMES)?));
                }
            }
            let expect = params.clone();
            Some(
                Step::cmd("project.checks.set", Value::Object(params))
                    .accept()
                    .check(move |_, after| {
                        let stored = checks_member(after);
                        for member in ["active_scheme", "release_scheme"] {
                            let have = stored.as_ref().and_then(|c| c.get(member));
                            if have != expect.get(member) {
                                return Err(format!(
                                    "checks.{member} is {have:?}, not {:?}",
                                    expect.get(member)
                                ));
                            }
                        }
                        Ok(())
                    }),
            )
        }
    }
}

/// The neutral name of the first panel-builder party, and the stored notice.
fn company_of(q: &ew_derive::Q) -> (Option<String>, Option<Value>) {
    let project = q.project();
    let name = project
        .parties
        .iter()
        .find(|p| p.role == ew_schema::PartyRole::PanelBuilder)
        .map(|p| p.name.as_neutral().unwrap_or_default().to_owned());
    let notice = project
        .extra
        .get("ew.company")
        .and_then(|c| c.get("notice"))
        .cloned();
    (name, notice)
}

/// A placeholder logo of `w × h` user units (no company's artwork).
fn placeholder_logo(w: u32, h: u32) -> Vec<u8> {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}"/></svg>"#)
        .into_bytes()
}

/// `project.assets.purge` (FT-380): a non-undoable, `HumanOnly` marker whose file side runs in the session (the
/// fuzzer's session submits as the GUI, which `HumanOnly` accepts). Every purgeable asset
/// (`{}`) commits as a marker when the session is saved (a session without a project directory has nothing to purge)
/// and is refused while it has unsaved changes; the model never changes. A listed hash that is no purgeable asset, and
/// a malformed one, are refused.
fn project_assets_purge(_live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    // The checks of an `Either` step run only when the revision moved, which a purge must never do.
    let unchanged = |_: &ew_derive::Q, _: &ew_derive::Q| Err("the purge moved the revision".to_owned());
    match rng.below(4) {
        0 => {
            let hex: String = (0..32).map(|_| format!("{:02x}", rng.below(256))).collect();
            Some(
                Step::cmd(
                    "project.assets.purge",
                    json!({"assets": [format!("sha256:{hex}")]}),
                )
                .refuse(),
            )
        }
        1 => Some(Step::cmd("project.assets.purge", json!({"assets": [rng.word(8)]})).refuse()),
        _ => Some(Step::cmd("project.assets.purge", json!({})).check(unchanged)),
    }
}

fn project_company_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(11) {
        // Refused: no member at all, or a value of the wrong type.
        0 => Some(Step::cmd("project.company.set", json!({})).refuse()),
        1 => Some(Step::cmd("project.company.set", json!({"name": 5})).refuse()),
        // FT-337: a logo joins the asset store and the company member names it; empty removes it.
        8 | 9 => {
            let (w, h) = (1 + rng.below(400) as u32, 1 + rng.below(400) as u32);
            let bytes = placeholder_logo(w, h);
            let remove = rng.chance(4);
            let text = if remove {
                String::new()
            } else {
                ew_schema::base64_encode(&bytes)
            };
            Some(
                Step::cmd("project.company.set", json!({"logo": text}))
                    .accept()
                    .check(move |_, after| {
                        let logo = ew_schema::company_logo(&after.project().extra);
                        match (remove, logo) {
                            (true, None) => Ok(()),
                            (false, Some(r)) if r.size == Some((w, h)) => Ok(()),
                            (_, other) => Err(format!("the logo is {other:?} (removed: {remove})")),
                        }
                    }),
            )
        }
        // Refused: a logo that is no image.
        10 => Some(
            Step::cmd(
                "project.company.set",
                json!({"logo": ew_schema::base64_encode(rng.word(12).as_bytes())}),
            )
            .refuse(),
        ),
        // Refused: the personal-data flag alone, without a panel-builder party to carry it.
        2 if !live.has_panel_builder => {
            Some(Step::cmd("project.company.set", json!({"personal_data": rng.chance(2)})).refuse())
        }
        _ => {
            let name = if rng.chance(4) {
                String::new()
            } else {
                format!("{} Panels", rng.word(6))
            };
            let notice = if rng.chance(3) {
                String::new()
            } else {
                format!("Property of {}.", rng.word(8))
            };
            let mut params = json!({"name": name, "notice": notice});
            if !name.is_empty() && rng.chance(2) {
                params["personal_data"] = json!(rng.chance(2));
            }
            let party_before = live.has_panel_builder;
            Some(
                Step::cmd("project.company.set", params)
                    .accept()
                    .check(move |_, after| {
                        let (have_name, have_notice) = company_of(after);
                        let want_name = (party_before || !name.is_empty()).then(|| name.clone());
                        if have_name != want_name {
                            return Err(format!("the panel builder is {have_name:?}, not {want_name:?}"));
                        }
                        let want_notice = (!notice.is_empty()).then(|| json!(notice));
                        if have_notice != want_notice {
                            return Err(format!("the notice is {have_notice:?}, not {want_notice:?}"));
                        }
                        Ok(())
                    }),
            )
        }
    }
}

/// The page types a frame-defaults entry may name (every type but `report`).
fn project_drawing_set(_live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(8) {
        // Refused: no member given.
        0 => Some(Step::cmd("project.drawing.set", json!({})).refuse()),
        // Refused: an unknown value, a value of the wrong type, an unknown member.
        1 => Some(Step::cmd("project.drawing.set", json!({"crossings": "loops"})).refuse()),
        // WP-S2.04 part 4a: `marker_mode`, an unknown value refused, a known one or `null` accepted.
        7 if rng.chance(2) => Some(Step::cmd("project.drawing.set", json!({"marker_mode": "some"})).refuse()),
        7 => {
            let mode = [
                json!("all"),
                json!("deviations"),
                json!("explicit"),
                json!("none"),
                Value::Null,
            ][rng.below(5)]
            .clone();
            Some(Step::cmd("project.drawing.set", json!({"marker_mode": mode})).accept())
        }
        2 => Some(Step::cmd("project.drawing.set", json!({"crossings": rng.below(9)})).refuse()),
        3 => Some(
            Step::cmd(
                "project.drawing.set",
                json!({"crossings": "hops", "shadows": true}),
            )
            .refuse(),
        ),
        _ => {
            let value = [json!("plain"), json!("hops"), Value::Null][rng.below(3)].clone();
            let want = value.clone();
            Some(
                Step::cmd("project.drawing.set", json!({"crossings": value}))
                    .accept()
                    .check(move |_, after| {
                        let drawing = after.project().settings.extra.get("drawing").cloned();
                        let crossings = drawing.as_ref().and_then(|d| d.get("crossings")).cloned();
                        match (&want, &crossings, &drawing) {
                            (Value::Null, None, None) => Ok(()),
                            (Value::Null, None, Some(Value::Object(m))) if !m.is_empty() => Ok(()),
                            (w, Some(c), _) if !w.is_null() && w == c => Ok(()),
                            _ => Err(format!(
                                "the drawing settings are {drawing:?} after setting `crossings` to {want}"
                            )),
                        }
                    }),
            )
        }
    }
}

const FRAME_PAGE_TYPES: [&str; 8] = [
    "title_page",
    "table_of_contents",
    "section_divider",
    "multi_line",
    "single_line",
    "fluid",
    "graphic",
    "panel_layout",
];

fn project_frames_set(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("project.frames.set", p);
    match rng.below(10) {
        // Refused: a form that does not exist.
        0 => Some(cmd(json!({"default": rng.uuid()})).refuse()),
        // Refused: an entry for report pages, or the same page type twice.
        1 => {
            let form = rng.pick(&live.frame_forms)?.clone();
            Some(cmd(json!({"per_page_type": [{"page_type": "report", "form": form}]})).refuse())
        }
        2 => {
            let form = rng.pick(&live.frame_forms)?.clone();
            let t = *rng.pick(&FRAME_PAGE_TYPES)?;
            Some(
                cmd(json!({"per_page_type": [
                    {"page_type": t, "form": form},
                    {"page_type": t, "form": form},
                ]}))
                .refuse(),
            )
        }
        // Refused: a form that is no frame (a title block).
        3 => {
            let form = rng.pick(&live.other_forms)?.clone();
            Some(cmd(json!({"default": form})).refuse())
        }
        // Removing the frame defaults lets the built-in frames apply.
        4 => Some(cmd(json!({})).accept().check(
            |_, after| match after.project().settings.extra.get("frames") {
                None => Ok(()),
                Some(v) => Err(format!("the frame defaults are still {v}")),
            },
        )),
        _ => {
            let default = rng.pick(&live.frame_forms)?.clone();
            let mut entries = Vec::new();
            let mut types: Vec<&str> = FRAME_PAGE_TYPES.to_vec();
            for _ in 0..rng.below(3) {
                let i = rng.below(types.len());
                let t = types.remove(i);
                entries.push(json!({"page_type": t, "form": rng.pick(&live.frame_forms)?.clone()}));
            }
            let expect_default = default.clone();
            let expect_len = entries.len();
            Some(
                cmd(json!({"default": default, "per_page_type": entries}))
                    .accept()
                    .check(move |_, after| {
                        let frames = after.get::<ew_derive::FrameSettings>(());
                        let default = frames.default.map(id_json);
                        if default.as_ref() != Some(&expect_default)
                            || frames.per_page_type.len() != expect_len
                        {
                            return Err(format!(
                                "the frame defaults read back as {default:?} with {} entries",
                                frames.per_page_type.len()
                            ));
                        }
                        Ok(())
                    }),
            )
        }
    }
}

fn project_languages_set(_live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("project.languages.set", p);
    match rng.below(8) {
        // Refused: translations without a source, the source as a translation, an unknown format.
        0 => Some(cmd(json!({"translations": ["de"]})).refuse()),
        1 => Some(cmd(json!({"source": "en", "translations": ["en"]})).refuse()),
        2 => Some(cmd(json!({"date": "mm/dd/yyyy"})).refuse()),
        _ => {
            let mut params = serde_json::Map::new();
            if !rng.chance(4) {
                params.insert("source".into(), json!(*rng.pick(&["en", "nl", "de"])?));
                if rng.chance(3) {
                    params.insert("translations".into(), json!(["fr"]));
                }
            }
            if !rng.chance(4) {
                params.insert("date".into(), json!(*rng.pick(&["dd_mm_yyyy", "iso8601"])?));
            }
            if !rng.chance(4) {
                params.insert("decimal_separator".into(), json!(*rng.pick(&["point", "comma"])?));
            }
            let expect = params.clone();
            Some(cmd(Value::Object(params)).accept().check(move |_, after| {
                let project = after.project();
                let source = project.languages.as_ref().map(|l| json!(l.source));
                let date = project.settings.formatting.date.as_ref().map(|d| json!(d));
                let separator = project
                    .settings
                    .formatting
                    .decimal_separator
                    .as_ref()
                    .map(|d| json!(d));
                let want = (
                    expect.get("source").cloned(),
                    expect.get("date").cloned(),
                    expect.get("decimal_separator").cloned(),
                );
                if (source.clone(), date.clone(), separator.clone()) != want {
                    return Err(format!(
                        "languages and formats read back as {:?}, not {want:?}",
                        (source, date, separator)
                    ));
                }
                Ok(())
            }))
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

/// `parts.import` (Bulk): project-local part copies. A copy with a new key is inserted, one whose key the
/// project has is skipped; another library, a wrong identity or hash, a duplicate, no copies and a non-part
/// document are refused.
fn parts_import(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (_, template) = rng.pick(&live.fixture.part_json)?;
    let project = live.fixture.project_id.as_str()?.to_owned();
    // A project-local copy with a fresh key and a fresh item, identity and hash made consistent.
    let fresh = |rng: &mut SplitMix64| -> Option<Value> {
        let mut t = template.clone();
        t["meta"]["key"] = json!(format!("fuzz.{}", &rng.uuid().replace('-', "")[..12]));
        relineaged(&t, &project, &rng.uuid())
    };
    match rng.below(10) {
        // Refused: a copy of another library (that is library.embed's).
        0 => {
            let mut t = template.clone();
            t["meta"]["key"] = json!(format!("fuzz.{}", &rng.uuid().replace('-', "")[..12]));
            let c = relineaged(&t, &rng.uuid(), &rng.uuid())?;
            Some(Step::cmd("parts.import", json!({"copies": [c]})).refuse())
        }
        // Refused: the copy ID is not the copy identity.
        1 => {
            let mut c = fresh(rng)?;
            c["id"] = json!(rng.uuid());
            Some(Step::cmd("parts.import", json!({"copies": [c]})).refuse())
        }
        // Refused: the hash is not the hash of the item's members.
        2 => {
            let mut c = fresh(rng)?;
            c["sha256"] = json!(format!("sha256:{}", "00".repeat(32)));
            Some(Step::cmd("parts.import", json!({"copies": [c]})).refuse())
        }
        // Refused: the same copy twice, and no copies at all.
        3 => {
            let c = fresh(rng)?;
            Some(Step::cmd("parts.import", json!({"copies": [c.clone(), c]})).refuse())
        }
        4 => Some(Step::cmd("parts.import", json!({"copies": []})).refuse()),
        // Refused: not a part document.
        5 => {
            let (_, form) = rng.pick(&live.fixture.form_json)?;
            Some(Step::cmd("parts.import", json!({"copies": [form.clone()]})).refuse())
        }
        // Accepted, nothing inserted: the key exists in the project already.
        6 => Some(
            Step::cmd("parts.import", json!({"copies": [template.clone()]}))
                .accept()
                .counts(&[(Table::Parts, 0)]),
        ),
        _ => {
            let c = fresh(rng)?;
            let id = c["id"].clone();
            Some(
                Step::cmd("parts.import", json!({"copies": [c]}))
                    .accept()
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
