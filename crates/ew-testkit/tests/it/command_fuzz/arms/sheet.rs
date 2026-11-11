// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for what stands on a sheet: representations, graphic items, markers and boxes.

use super::*;

pub(super) fn rep_move(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    // Page items and structure boxes move with `items`.
    if rng.chance(4)
        && let Some(step) = rep_move_items(live, rng)
    {
        return Some(step);
    }
    let rep = rng.pick(&live.reps)?.clone();
    match rng.below(8) {
        // The command refuses: grid steps beside a non-zero delta.
        0 => {
            let p = json!({"reps": [rep], "delta": [MOVE_UNIT, 0], "steps": [1, 0]});
            return Some(Step::cmd("rep.move", p));
        }
        // The command refuses: grid steps with a target page (a move to another page takes a delta).
        1 => {
            let page = rng.pick(&live.pages)?;
            let p = json!({"reps": [rep], "steps": [1, -1], "page": page});
            return Some(Step::cmd("rep.move", p));
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
        // The window's members. Refused: a route for a connection that does not exist. Either: a
        // connection's route cleared after the move (a route the move followed), and a hint whose junction does not
        // exist (dropped with a notice, before its CP is judged).
        7 => {
            let steps = [rng.range(-1, 1), rng.range(-1, 1)];
            return Some(match rng.below(3) {
                0 => Step::cmd(
                    "rep.move",
                    json!({"reps": [rep], "steps": steps, "routes": [{"connection": rng.uuid(), "route": []}]}),
                )
                .refuse(),
                1 => {
                    let conn = rng.pick(&live.routes)?.connection.clone();
                    Step::cmd(
                        "rep.move",
                        json!({"reps": [rep], "steps": steps, "routes": [{"connection": conn, "route": []}]}),
                    )
                }
                _ => Step::cmd(
                    "rep.move",
                    json!({"reps": [rep.clone()], "steps": steps, "connect": [
                        {"rep": rep, "cp": {"function": rng.uuid(), "idx": rng.below(4)}, "to": {"junction": {"junction": rng.uuid()}}}
                    ]}),
                ),
            });
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

/// `rep.move` with `items` alone: a page item or structure box moves by a delta, also across pages or with
/// nothing named (the command decides: it refuses a surface view, a move across pages and an empty selection);
/// refused: an item that does not exist.
pub(super) fn rep_move_items(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let mut all: Vec<(Value, Value)> = live.page_items.clone();
    all.extend(live.structure_boxes.iter().cloned());
    match rng.below(6) {
        0 => Some(Step::cmd(
            "rep.move",
            json!({"reps": [], "delta": [MOVE_UNIT, 0]}),
        )),
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
            Some(Step::cmd("rep.move", p))
        }
        _ => {
            let (item, _) = rng.pick(&all)?.clone();
            let delta = [rng.range(-2, 2) * MOVE_UNIT, rng.range(-2, 2) * MOVE_UNIT];
            Some(
                Step::cmd("rep.move", json!({"reps": [], "items": [item], "delta": delta}))
                    .counts(&[(Table::PageItems, 0), (Table::StructureBoxes, 0)]),
            )
        }
    }
}

/// A random graphic shape on `page`, valid unless `degenerate`.
pub(super) fn shape(page: &PageInfo, rng: &mut SplitMix64, degenerate: bool) -> Value {
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
/// bindable name, an empty text without `bind`, an angle off the quarter turns, an unknown member or (part 5) an
/// image whose asset the project does not hold.
pub(super) fn item_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let page = rng.pick(&live.page_info)?;
    let cmd = |p: Value| Step::cmd("item.place", p);
    let at = Live::grid_pos(page, rng.range(1, 99), rng.range(1, 70));
    match rng.below(13) {
        11 => {
            // The image file itself: a small valid PNG (the sniffer reads the header), stored with the item.
            let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
            png.extend_from_slice(&13u32.to_be_bytes());
            png.extend_from_slice(b"IHDR");
            png.extend_from_slice(&u32::try_from(rng.range(1, 64)).expect("small").to_be_bytes());
            png.extend_from_slice(&u32::try_from(rng.range(1, 64)).expect("small").to_be_bytes());
            png.extend_from_slice(&[8, 6, 0, 0, 0]);
            let data = ew_schema::base64_encode(&png);
            // The second corner is 1 to 20 move units right of and below the first, so the rectangle has area
            // whichever grid position the first one drew (`item.place` refuses a rectangle without).
            let (dx, dy) = (rng.range(1, 20), rng.range(1, 20));
            let (x, y) = (at[0].as_i64()?, at[1].as_i64()?);
            Some(
                cmd(json!({"page": page.id, "kind": {"image": {"data": data,
                    "rect": [at, [x + MOVE_UNIT * dx, y + MOVE_UNIT * dy]]}}}))
                .counts(&[(Table::PageItems, 1)]),
            )
        }
        12 => {
            // Junk bytes, or both members, or neither: refused.
            let junk: Vec<u8> = (0..8).flat_map(|_| rng.next().to_le_bytes()).collect();
            let image = match rng.below(3) {
                0 => {
                    json!({"data": ew_schema::base64_encode(&junk), "rect": [at, [MOVE_UNIT * 60, MOVE_UNIT * 40]]})
                }
                1 => {
                    json!({"data": ew_schema::base64_encode(&junk), "asset": format!("sha256:{}", hex(&junk[..32])),
                    "rect": [at, [MOVE_UNIT * 60, MOVE_UNIT * 40]]})
                }
                _ => json!({"rect": [at, [MOVE_UNIT * 60, MOVE_UNIT * 40]]}),
            };
            Some(cmd(json!({"page": page.id, "kind": {"image": image}})))
        }
        10 => {
            // A hash the project cannot hold: 32 random bytes.
            let bytes: Vec<u8> = (0..4).flat_map(|_| rng.next().to_le_bytes()).collect();
            let asset = format!("sha256:{}", hex(&bytes));
            Some(cmd(json!({"page": page.id, "kind": {"image": {"asset": asset,
                    "rect": [at, [MOVE_UNIT * 20, MOVE_UNIT * 20]]}}})))
        }
        0 => Some(cmd(json!({"page": rng.uuid(), "kind": {"free_text": {"text": "x", "at": at}}})).refuse()),
        1 => Some(cmd(
            json!({"page": page.id, "kind": {"graphic": {"shape": shape(page, rng, true)}}}),
        )),
        2 => Some(cmd(
            json!({"page": page.id, "kind": {"free_text": {"bind": "project.reference", "at": at}}}),
        )),
        3 => Some(cmd(
            json!({"page": page.id, "kind": {"free_text": {"text": "", "at": at}}}),
        )),
        4 => Some(cmd(
            json!({"page": page.id, "kind": {"free_text": {"text": "x", "at": at, "angle": 45_000}}}),
        )),
        5 => Some(
            cmd(json!({"page": page.id, "kind": {"graphic": {"shape": shape(page, rng, false)}, "z": 1}}))
                .refuse(),
        ),
        6 | 7 => {
            let dash = ["solid", "dashed", "dotted", "dash_dot"][rng.below(4)];
            let p = json!({"page": page.id, "kind": {"graphic": {"shape": shape(page, rng, false),
                "dash": dash, "fill": rng.chance(2)}}});
            Some(cmd(p).counts(&[(Table::PageItems, 1)]))
        }
        _ => {
            let text = if rng.chance(3) {
                json!({"bind": ew_derive::BINDABLE[rng.below(6)], "at": at})
            } else {
                let angle = [0, 90_000, 180_000, 270_000][rng.below(4)];
                let align = ["left", "centre", "right"][rng.below(3)];
                json!({"text": "40x60", "at": at, "angle": angle, "align": align})
            };
            Some(cmd(json!({"page": page.id, "kind": {"free_text": text}})).counts(&[(Table::PageItems, 1)]))
        }
    }
}

/// `marker.place`: a group of a page's connections; refused with no connection, a duplicate, a connection of
/// another page or one that does not exist, an angle other than 0 or 90000, a colour count that does not match or a
/// foreign colour code.
pub(super) fn marker_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (page, conns) = rng.pick(&live.page_connections)?.clone();
    let cmd = |p: Value| Step::cmd("marker.place", p);
    let at = json!([MOVE_UNIT, MOVE_UNIT]);
    let one = rng.pick(&conns)?.clone();
    match rng.below(10) {
        0 => Some(cmd(json!({"page": page, "connections": [], "at": at}))),
        1 => Some(cmd(
            json!({"page": page, "connections": [one.clone(), one], "at": at}),
        )),
        2 => Some(cmd(json!({"page": page, "connections": [rng.uuid()], "at": at})).refuse()),
        3 => {
            let other = live
                .page_connections
                .iter()
                .filter(|(p, _)| *p != page)
                .flat_map(|(_, c)| c.iter())
                .next()
                .cloned()?;
            Some(cmd(json!({"page": page, "connections": [other], "at": at})))
        }
        4 => Some(cmd(
            json!({"page": page, "connections": [one], "at": at, "angle": 45_000}),
        )),
        5 => Some(cmd(
            json!({"page": page, "connections": [one], "at": at, "colours": [["BK"], ["BU"]]}),
        )),
        6 => Some(cmd(
            json!({"page": page, "connections": [one], "at": at, "colours": [[{"foreign": "XX"}]]}),
        )),
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
            Some(cmd(p).counts(&[(Table::PageItems, 1)]))
        }
    }
}

/// `box.structure`: a box on a page assigning a location node; refused with fewer than three vertices, on a missing
/// page, with a node in a slot of another aspect or a missing external reference.
pub(super) fn box_structure(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let page = rng.pick(&live.page_info)?;
    let cmd = |p: Value| Step::cmd("box.structure", p);
    let outline = json!([[0, 0], [50_000_000, 0], [50_000_000, 30_000_000], [0, 30_000_000]]);
    let assigns = match rng.pick(&live.locations) {
        Some(n) => json!({"location": n}),
        None => json!({}),
    };
    match rng.below(8) {
        0 => Some(cmd(
            json!({"page": page.id, "outline": [[0, 0], [1, 1]], "style": "dotted", "assigns": {}}),
        )),
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
            Some(cmd(
                json!({"page": page.id, "outline": outline, "style": "dotted",
                    "assigns": {"location": wrong.id}}),
            ))
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
            Some(cmd(p).counts(&[(Table::StructureBoxes, 1)]))
        }
    }
}

pub(super) fn rep_transform(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let rep = rng.pick(&live.reps)?.clone();
    match rng.below(10) {
        // The command refuses: `rotate` counts quarter turns, 0 to 3.
        0 => Some(Step::cmd(
            "rep.transform",
            json!({"reps": [rep], "rotate": 4, "mirror": false}),
        )),
        // The command refuses: nothing to transform.
        1 => Some(Step::cmd("rep.transform", json!({"reps": [], "rotate": 1}))),
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
                    if a.page != b.page {
                        return Err("a transform moved the representation to another page".to_owned());
                    }
                    // A black box whose orientation changes turns about the centre of its bounding box (the
                    // outline's, else its CPs'), so its insertion point moves (`rep.transform`, "The pivot"): the
                    // centre, in grid half-steps, stays, or moves by the one half-step of a tie. A symbol keeps its
                    // insertion point.
                    let local = b.box_.as_ref().map_or(Vec::new(), |x| {
                        if x.outline.is_empty() {
                            x.cps.iter().map(|c| c.pos).collect()
                        } else {
                            x.outline.clone()
                        }
                    });
                    if local.is_empty() || a.orient == b.orient {
                        return (a.pos == b.pos)
                            .then_some(())
                            .ok_or_else(|| "a transform moved the insertion point".to_owned());
                    }
                    let grid = before
                        .pages()
                        .get(b.page)
                        .ok_or("the page of the representation")?
                        .grid;
                    let twice_centre = |r: &ew_schema::Representation| {
                        let pts: Vec<(i64, i64)> = local
                            .iter()
                            .map(|p| r.orient.apply(*p))
                            .map(|p| (grid.snap(p.x), grid.snap(p.y)))
                            .collect();
                        let span = |axis: fn(&(i64, i64)) -> i64| {
                            pts.iter().map(axis).min().unwrap_or(0) + pts.iter().map(axis).max().unwrap_or(0)
                        };
                        (
                            2 * grid.snap(r.pos.x) + span(|p| p.0),
                            2 * grid.snap(r.pos.y) + span(|p| p.1),
                        )
                    };
                    let (cb, ca) = (twice_centre(b), twice_centre(a));
                    ((cb.0 - ca.0).abs() <= 1 && (cb.1 - ca.1).abs() <= 1)
                        .then_some(())
                        .ok_or_else(|| {
                            format!(
                                "a black box's turn moved the centre of its bounding box from {cb:?} to {ca:?} grid half-steps"
                            )
                        })
                }),
            )
        }
    }
}

/// `rep.join_device`: a D3-eligible representation joins a device that takes its FnDef by D2's rule;
/// the steps the command refuses: a representation whose device has several functions, the representation's own
/// device, a device D2's rule refuses, a representation that does not exist.
pub(super) fn rep_join_device(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let cmd = |p: Value| Step::cmd("rep.join_device", p);
    match rng.below(10) {
        0 => {
            let rep = rng.pick(&live.join_blocked)?;
            let device = rng.pick(&live.plain_devices)?;
            Some(cmd(json!({"rep": rep, "device": device})))
        }
        1 => {
            let j = rng.pick(&live.joins)?;
            Some(cmd(json!({"rep": j.rep, "device": j.device})))
        }
        2 => {
            let j = rng.pick(&live.joins)?;
            let device = rng.pick(&j.refused)?;
            Some(cmd(json!({"rep": j.rep, "device": device})))
        }
        3 => {
            let device = rng.pick(&live.plain_devices)?;
            Some(cmd(json!({"rep": rng.uuid(), "device": device})).refuse())
        }
        _ => {
            let with: Vec<_> = live.joins.iter().filter(|j| !j.targets.is_empty()).collect();
            let j = *rng.pick(&with)?;
            let (device, slot) = rng.pick(&j.targets)?.clone();
            let rep = j.rep.clone();
            let step = cmd(json!({"rep": j.rep, "device": device}))
                .counts(&[
                    (Table::Devices, -1),
                    (Table::Functions, if slot { -1 } else { 0 }),
                    (Table::Reps, 0),
                    (Table::Connections, 0),
                ])
                .check(move |_, after| {
                    // The representation now draws a function of the target device.
                    let r = typed(&rep)
                        .and_then(|id| after.reps().get(id))
                        .ok_or_else(|| format!("representation {rep} is gone"))?;
                    let owner = r
                        .functions
                        .first()
                        .and_then(|f| after.functions().get(*f))
                        .and_then(|f| f.device);
                    (owner.map(id_json).as_ref() == Some(&device))
                        .then_some(())
                        .ok_or_else(|| format!("representation {rep} does not draw a function of {device}"))
                });
            Some(step)
        }
    }
}

/// A contact symbol placed as a new device on a multi-line page (one function, no part), and a
/// device created with an unplaced main function of the same FnDef (a slot).
pub(super) fn join_setup(live: &Live, rng: &mut SplitMix64) -> Option<Vec<Step>> {
    let pages: Vec<&PageInfo> = live.page_info.iter().filter(|p| p.kind == "multi_line").collect();
    let page = *rng.pick(&pages)?;
    let contacts: Vec<&SymInfo> = live
        .symbols
        .iter()
        .filter(|s| {
            s.rep_type == "multi_line" && s.fndef.as_ref().is_some_and(|d| live.contact_fndefs.contains(d))
        })
        .collect();
    let symbol = *rng.pick(&contacts)?;
    let place = json!({"page": page.id, "symbol": symbol.id, "pos": on_page(page, rng),
        "target": {"new_device": {"letter": "K"}}});
    let create = json!({"letter": "K", "main_fndef": symbol.fndef});
    Some(vec![
        Step::cmd("device.place", place),
        Step::cmd("device.create", create),
    ])
}

pub(super) fn rep_exchange_symbol(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
        // The command refuses: an interruption point keeps its symbol.
        2 => {
            let ips: Vec<_> = live.rep_infos.iter().filter(|r| r.ip).collect();
            let ip = *rng.pick(&ips)?;
            let symbol = rng.pick(&live.symbols)?;
            Some(Step::cmd(
                "rep.exchange_symbol",
                json!({"rep": ip.id, "symbol": symbol.id}),
            ))
        }
        // The command refuses: a representation type the page does not allow.
        3 => {
            let wrong: Vec<_> = live.symbols.iter().filter(|s| !fits_page(s)).collect();
            let symbol = *rng.pick(&wrong)?;
            Some(Step::cmd(
                "rep.exchange_symbol",
                json!({"rep": rep.id, "symbol": symbol.id}),
            ))
        }
        // The command refuses: an explicit CP map that names a symbol CP the symbol does not have.
        4 => {
            let symbol = rng.pick(&live.symbols)?;
            let p = json!({"rep": rep.id, "symbol": symbol.id,
                "pin_map": [{"symbol_cp": 60_000, "function_cp": 0}]});
            Some(Step::cmd("rep.exchange_symbol", p))
        }
        // The command refuses: an explicit CP map that maps a symbol CP twice.
        5 => {
            let symbol = rng.pick(&live.symbols)?;
            let p = json!({"rep": rep.id, "symbol": symbol.id,
                "pin_map": [{"symbol_cp": 0, "function_cp": 0}, {"symbol_cp": 0, "function_cp": 1}]});
            Some(Step::cmd("rep.exchange_symbol", p))
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

/// What a reshaped box must satisfy afterwards: the outline as given, the CPs of these indices, each on the
/// rectangle's outline and, in absolute terms, on the page grid.
pub(super) fn box_after_check(
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

pub(super) fn box_reshape(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
        // The command refuses: a representation that has no box.
        1 => {
            let rep = rng.pick(&live.plain_reps)?;
            Some(Step::cmd("box.reshape", json!({"rep": rep, "outline": widened})))
        }
        // The command refuses: an outline of fewer than three vertices.
        2 => Some(Step::cmd(
            "box.reshape",
            json!({"rep": b.rep, "outline": [[x0, y0], [x1w, y0]]}),
        )),
        // The command refuses: a vertex off the page's connection grid.
        3 => {
            let outline = json!([[x0, y0], [x1w, y0], [x1w + 1, y1], [x0, y1]]);
            Some(Step::cmd(
                "box.reshape",
                json!({"rep": b.rep, "outline": outline}),
            ))
        }
        // The command refuses: two CPs with one index.
        4 => {
            let cps = json!([
                {"idx": 0, "pos": [x0, y0], "dir": "up"},
                {"idx": 0, "pos": [x1w, y0], "dir": "up"},
            ]);
            Some(Step::cmd(
                "box.reshape",
                json!({"rep": b.rep, "outline": widened, "cps": cps}),
            ))
        }
        // The command refuses: a CP off the outline, or a CP off the grid.
        5 | 6 => {
            let pos = if rng.chance(2) {
                [x0 - n, y0 - n]
            } else {
                [x0 + 1, y0]
            };
            let cps = json!([{"idx": 0, "pos": pos, "dir": "up"}]);
            Some(Step::cmd(
                "box.reshape",
                json!({"rep": b.rep, "outline": widened, "cps": cps}),
            ))
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
                .counts(&counts)
                .check(box_after_check(b.rep.clone(), widened_pts, idxs)),
            )
        }
        // The CPs re-projected onto the widened rectangle.
        _ => Some(
            Step::cmd("box.reshape", json!({"rep": b.rep, "outline": widened}))
                .counts(&counts)
                .check(box_after_check(b.rep.clone(), widened_pts, idxs)),
        ),
    }
}

pub(super) fn box_black(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
        // The command refuses: a template without exactly one main entry (EW-INV-002: the device needs exactly one main
        // function).
        14 => {
            let (t, i, _) = template_entry(live, rng, true, false)?;
            p["target"] = from_part_target(&t.part, i, "A");
            Some(Step::cmd("box.black", p))
        }
        // A black-box entry of a part copy: the device with a function per template entry, the entry drawn.
        10 | 11 => {
            let (t, i, _) = template_entry(live, rng, true, true)?;
            p["target"] = from_part_target(&t.part, i, "A");
            let entries = isize::try_from(t.entries.len()).unwrap_or(1);
            // FT-656: one in three brings its part as another library's item in `copies`.
            let mut parts = 0;
            if rng.chance(3)
                && let Some(c) = part_of_another_library(live, &t.part, rng)
            {
                p["target"] = from_part_target(&c["id"], i, "A");
                p["copies"] = json!([c]);
                parts = 1;
            }
            Some(Step::cmd("box.black", p).counts(&[
                (Table::Reps, 1),
                (Table::Functions, entries),
                (Table::Devices, 1),
                (Table::Parts, parts),
            ]))
        }
        // The command refuses: an entry that is no black box belongs to `device.place`.
        12 => {
            let (t, i, _) = template_entry(live, rng, false, true)?;
            p["target"] = from_part_target(&t.part, i, "A");
            Some(Step::cmd("box.black", p))
        }
        // The command refuses: a black box is no terminal.
        13 => {
            let strip = rng.pick(&live.strip_infos)?;
            p["target"] =
                json!({"new_terminal": {"strip": strip.id, "designation": fresh_designation(strip, rng)}});
            Some(Step::cmd("box.black", p))
        }
        // The command refuses: an outline of fewer than three points.
        0 => {
            p["box"] = json!({"outline": [[0, 0], [4 * n, 0]]});
            Some(Step::cmd("box.black", p))
        }
        // The command refuses: two box CPs with the same index.
        1 => {
            p["box"] = json!({
                "outline": [[0, 0], [4 * n, 0], [4 * n, 3 * n], [0, 3 * n]],
                "cps": [{"idx": 0, "dir": "right", "pos": [4 * n, n]}, {"idx": 0, "dir": "left", "pos": [0, n]}],
            });
            Some(Step::cmd("box.black", p))
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

pub(super) fn rep_set_anchor_policy(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    match rng.below(10) {
        0 => Some(Step::cmd("rep.set_anchor_policy", json!({"reps": [], "anchor": "origin"})).refuse()),
        1 => {
            let r = rng.pick(&live.reps)?;
            Some(Step::cmd(
                "rep.set_anchor_policy",
                json!({"reps": [r, r], "anchor": "origin"}),
            ))
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
                Step::cmd("rep.set_anchor_policy", json!({"reps": reps, "anchor": anchor})).check(
                    move |_, after| {
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
                    },
                ),
            )
        }
    }
}
