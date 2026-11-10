// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arms for connections, junctions, potentials and interruption points.

use super::*;

pub(super) fn connection_delete(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    if rng.chance(12) {
        return Some(Step::cmd("connection.delete", json!({"connections": [rng.uuid()]})).refuse());
    }
    let c = rng.pick(&live.connections)?.clone();
    Some(
        Step::cmd("connection.delete", json!({"connections": [c]})).check(move |_, after| {
            let id = typed(&c).ok_or("a connection ID")?;
            (!after.connections().contains(id))
                .then_some(())
                .ok_or_else(|| "the deleted connection still exists".to_owned())
        }),
    )
}

pub(super) fn junction_edit(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
                Step::cmd("junction.edit", json!({"junction": junction, "wiring": wiring})).check(
                    move |_, after| {
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
                    },
                ),
            )
        }
    }
}

pub(super) fn connection_draw(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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

// Cycle 33: the chain of bridges of Edit ▸ Bridge terminals.
pub(super) fn connection_draw_many(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (_, functions) = rng.pick(&live.page_functions)?;
    match rng.below(10) {
        // The command refuses: no entry.
        0 => Some(Step::cmd("connection.draw_many", json!({"connections": []}))),
        // Refused: the last entry names a function that does not exist; the first entry is not kept either.
        1 => {
            let (a, b) = pick2(functions, rng)?;
            let bad = json!(rng.uuid());
            Some(
                Step::cmd(
                    "connection.draw_many",
                    json!({"connections": [
                        {"a": cp_end(a, 0), "b": cp_end(b, 0)},
                        {"a": cp_end(&bad, 0), "b": cp_end(b, 1)},
                    ]}),
                )
                .refuse(),
            )
        }
        // One to three page-less connections between two CPs each (nothing is split): that many new connections.
        _ => {
            let n = 1 + rng.below(3);
            let mut connections = Vec::with_capacity(n);
            for _ in 0..n {
                let (a, b) = pick2(functions, rng)?;
                connections.push(json!({"a": cp_end(a, small_idx(rng)), "b": cp_end(b, small_idx(rng))}));
            }
            let count = isize::try_from(n).unwrap_or(1);
            Some(
                Step::cmd("connection.draw_many", json!({"connections": connections}))
                    .counts(&[(Table::Connections, count)]),
            )
        }
    }
}

pub(super) fn potential_define(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (page, functions) = rng.pick(&live.page_functions)?;
    match rng.below(10) {
        // The command refuses: a potential is defined at a CP or a junction, not on a connection.
        0 => {
            let c = rng.pick(&live.connections)?;
            Some(Step::cmd(
                "potential.define",
                json!({"at": {"on_connection": {"connection": c, "at": [0, 0]}}, "name": "FZ1"}),
            ))
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

pub(super) fn connection_draw_to_ip(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
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
    // FT-980 (ID-1146): a star source is a source point; a star with another direction is refused.
    let star_source = rng.chance(4) && direction == "source";
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
    match rng.below(12) {
        // Refused: the page does not exist.
        0 => {
            let (symbol, def) = end?;
            let p = step(cp_end(a, idx), &json!(rng.uuid()), ip(&symbol, &def));
            Some(Step::cmd("connection.draw_to_ip", p).refuse())
        }
        // The command refuses (FT-1064, ID-1146): a starred point whose direction is not source.
        10 => {
            let (symbol, def) = end?;
            let mut ip = ip(&symbol, &def);
            ip["direction"] = json!(*rng.pick(&["target", "undirected"])?);
            ip["star_source"] = json!(true);
            let p = step(cp_end(a, idx), &page.id, ip);
            Some(Step::cmd("connection.draw_to_ip", p))
        }
        // The command refuses (FT-1064, ID-1146): a starred external point, with a live external reference.
        11 => {
            let (symbol, def) = end?;
            let mut ip = ip(&symbol, &def);
            ip["external"] = json!(rng.pick(&live.ext_refs)?);
            ip["star_source"] = json!(true);
            let p = step(cp_end(a, idx), &page.id, ip);
            Some(Step::cmd("connection.draw_to_ip", p))
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
        // The command refuses: the insertion point is off the page's connection grid.
        2 => {
            let (symbol, def) = end?;
            let mut ip = ip(&symbol, &def);
            ip["pos"] = json!([2 * n + 1, 3 * n]);
            let p = step(cp_end(a, idx), &page.id, ip);
            Some(Step::cmd("connection.draw_to_ip", p))
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
        // The command refuses: a route that is off the grid and does not end at the interruption point's CP.
        4 => {
            let (symbol, def) = end?;
            let mut p = step(cp_end(a, idx), &page.id, ip(&symbol, &def));
            p["route"] = json!([[1, 1], [2, 2]]);
            Some(Step::cmd("connection.draw_to_ip", p))
        }
        // The wire and its interruption point: one function, one representation, one connection; some of the time
        // an external point, unstarred (an external point takes no star, ID-1146). Either: the generator does not
        // model the prevent rules.
        _ => {
            let (symbol, def) = end?;
            let mut ip = ip(&symbol, &def);
            if rng.chance(5)
                && let Some(e) = rng.pick(&live.ext_refs)
            {
                ip["external"] = json!(e);
                ip["star_source"] = json!(false);
            }
            let p = step(cp_end(a, idx), &page.id, ip);
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

pub(super) fn ip_arrange(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let mut by_page: std::collections::BTreeMap<String, Vec<&Value>> = std::collections::BTreeMap::new();
    for ip in &live.ip_infos {
        by_page.entry(ip.page.to_string()).or_default().push(&ip.function);
    }
    let groups: Vec<&Vec<&Value>> = by_page.values().filter(|g| g.len() >= 2).collect();
    let any_ip = rng.pick(&live.ip_infos)?;
    match rng.below(10) {
        // The command refuses: fewer than two interruption points.
        0 => {
            let ips = if rng.chance(2) {
                vec![]
            } else {
                vec![any_ip.function.clone()]
            };
            Some(Step::cmd("ip.arrange", json!({"ips": ips})))
        }
        // The command refuses: an interruption point twice.
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
            Some(Step::cmd("ip.arrange", json!({"ips": ips})))
        }
        // Refused: a function that does not exist.
        2 => Some(Step::cmd("ip.arrange", json!({"ips": [any_ip.function, rng.uuid()]})).refuse()),
        // The command refuses: a function that is not an interruption point.
        3 => {
            let plain: Vec<_> = live.functions.iter().filter(|f| !f.ip).collect();
            let f = rng.pick(&plain)?;
            Some(Step::cmd("ip.arrange", json!({"ips": [any_ip.function, f.id]})))
        }
        // The command refuses: interruption points of two pages.
        4 => {
            let other: Vec<_> = live.ip_infos.iter().filter(|i| i.page != any_ip.page).collect();
            let other = *rng.pick(&other)?;
            Some(Step::cmd(
                "ip.arrange",
                json!({"ips": [any_ip.function, other.function]}),
            ))
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

pub(super) fn ip_place(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let (symbol, def) = rng.pick(&live.ip_syms)?;
    let pages: Vec<_> = live.page_info.iter().filter(|p| p.kind == "multi_line").collect();
    let page = *rng.pick(&pages)?;
    let direction = *rng.pick(&["source", "target", "undirected"])?;
    let mut p = json!({
        "page": page.id, "symbol": symbol, "fndef": def, "pos": on_page(page, rng),
        "name": format!("FZ-{}", rng.below(1000)), "direction": direction,
        // FT-980 (ID-1146): a star source is a source point.
        "star_source": rng.chance(4) && direction == "source",
    });
    match rng.below(12) {
        0 => {
            p["page"] = json!(rng.uuid());
            Some(Step::cmd("ip.place", p).refuse())
        }
        1 => {
            let n = i64::try_from(page.grid.0).unwrap_or(1);
            p["pos"] = json!([2 * n + 1, 3 * n]);
            Some(Step::cmd("ip.place", p))
        }
        2 => {
            p["external"] = json!(rng.uuid());
            Some(Step::cmd("ip.place", p).refuse())
        }
        k => {
            if k == 3 {
                p["external"] = json!(rng.pick(&live.ext_refs)?);
                // An external point takes no star (ID-1146).
                p["star_source"] = json!(false);
            }
            Some(Step::cmd("ip.place", p).counts(&[
                (Table::Reps, 1),
                (Table::Functions, 1),
                (Table::Devices, 0),
            ]))
        }
    }
}

pub(super) fn ip_continue(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let ip = rng.pick(&live.ip_infos)?;
    match rng.below(8) {
        // The command refuses: the target page is the page of the interruption point.
        0 => Some(Step::cmd(
            "ip.continue",
            json!({"ip": ip.function, "page": ip.page}),
        )),
        // The command refuses: not an interruption point.
        1 => {
            let f = live.functions.iter().filter(|f| !f.ip).collect::<Vec<_>>();
            let f = rng.pick(&f)?;
            let page = rng.pick(&live.pages)?;
            Some(Step::cmd("ip.continue", json!({"ip": f.id, "page": page})))
        }
        _ => {
            // Another page of the same grid, so that the original position is on its grid too.
            let pages: Vec<_> = live
                .page_info
                .iter()
                .filter(|p| p.kind == "multi_line" && p.id != ip.page && p.grid == ip.grid)
                .collect();
            let page = *rng.pick(&pages)?;
            // The command refuses (FT-194): the partner of a target is a source, a second one of a name that has a source.
            if !ip.continues_cleanly {
                return Some(Step::cmd(
                    "ip.continue",
                    json!({"ip": ip.function, "page": page.id}),
                ));
            }
            let name = ip.name.clone();
            let function = ip.function.clone();
            Some(
                Step::cmd("ip.continue", json!({"ip": ip.function, "page": page.id}))
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

pub(super) fn connection_route(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let r = rng.pick(&live.routes)?;
    let conn = r.connection.clone();
    let as_json =
        |route: &[(i64, i64)]| -> Value { Value::Array(route.iter().map(|(x, y)| json!([x, y])).collect()) };
    let verified = |step: Step, expect: Vec<(i64, i64)>, conn: Value| {
        step.check(move |_, after| {
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
    // A well-formed route is tidy (FT-437): no vertex repeats the one before it and no segment runs over another.
    // A bend line through an end (or ends on one vertical or horizontal line) would repeat a vertex, and a Z whose
    // ends share an x doubles back on that line; such routes become the straight route between the ends.
    let tidy = |route: Vec<(i64, i64)>| -> Vec<(i64, i64)> {
        let mut out: Vec<(i64, i64)> = Vec::with_capacity(route.len());
        for v in route {
            if out.last() != Some(&v) {
                out.push(v);
            }
        }
        if out.len() > 2 && (a.0 == b.0 || a.1 == b.1) {
            out = vec![a, b];
        }
        out
    };
    // A grid coordinate between the ends, for the bends.
    let mid = ((a.1 + b.1) / 2 / n + rng.range(-2, 2)) * n;
    match rng.below(10) {
        // Clearing the route is always valid (an empty route has no geometry).
        0 => Some(verified(
            Step::cmd("connection.route", json!({"connection": conn, "route": []})),
            Vec::new(),
            conn,
        )),
        // The command refuses: the first vertex does not meet its CP.
        1 => {
            let route = [(a.0 + 1, a.1), (b.0, a.1), b];
            Some(Step::cmd(
                "connection.route",
                json!({"connection": conn, "route": as_json(&route)}),
            ))
        }
        // The command refuses: a bend off the connection grid (and not on the line of an end).
        2 if mid + 1 != a.1 && mid + 1 != b.1 => {
            let route = [a, (a.0, mid + 1), (b.0, mid + 1), b];
            Some(Step::cmd(
                "connection.route",
                json!({"connection": conn, "route": as_json(&route)}),
            ))
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
            let route = tidy(vec![a, (b.0, a.1), b]);
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
            let route = tidy(vec![a, (a.0, mid), (b.0, mid), b]);
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

// WP-S2.19 D12: several routes in one transaction, each as `connection.route` takes it.
pub(super) fn connection_route_many(live: &Live, rng: &mut SplitMix64) -> Option<Step> {
    let r = rng.pick(&live.routes)?;
    let conn = r.connection.clone();
    let (a, b) = (r.a, r.b);
    let route = |a: (i64, i64), b: (i64, i64)| -> Value {
        if a.0 == b.0 || a.1 == b.1 {
            json!([[a.0, a.1], [b.0, b.1]])
        } else {
            json!([[a.0, a.1], [b.0, a.1], [b.0, b.1]])
        }
    };
    match rng.below(6) {
        // The command refuses: no entry.
        0 => Some(Step::cmd("connection.route_many", json!({"routes": []}))),
        // Refused: the second entry names a connection that does not exist; the first is not kept either.
        1 => Some(
            Step::cmd(
                "connection.route_many",
                json!({"routes": [
                    {"connection": conn, "route": route(a, b)},
                    {"connection": rng.uuid(), "route": []},
                ]}),
            )
            .refuse(),
        ),
        // A dog-leg route from end to end and a cleared one, as two entries.
        _ => {
            let other = rng.pick(&live.routes)?;
            let mut routes = vec![json!({"connection": conn, "route": route(a, b)})];
            if other.connection != conn {
                routes.push(json!({"connection": other.connection, "route": []}));
            }
            Some(Step::cmd("connection.route_many", json!({"routes": routes})))
        }
    }
}
