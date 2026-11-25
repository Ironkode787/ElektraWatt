// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The wires layer of a page (see the module documentation of `page`): the connections, the junctions, the
//! bridges and the level ties, and the lines a wire marker crosses.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Builder<'_> {
    pub(super) fn wires(&mut self, view: &PageView) {
        let cps = cp_map(view);
        let page = page_rect(view);
        let stroke = wire_stroke(view.grid);
        // The drawn polylines in connection-ID order (the hop rule of `crate::hops` reads them all first), each with
        // its stroke: a fluid pilot or drain line is dashed 4:2 in the page's wire pen. An unrouted
        // connection is the dashed right-angled line between its ends in the thinnest pen, and no wire: it
        // takes and makes no hop.
        let mut drawn: Vec<(Id<Connection>, Vec<Pt2>)> = Vec::new();
        let mut strokes: Vec<Stroke> = Vec::new();
        // A flexible fluid line (a hose, ISO 1219-1) is drawn as an arc between its ends.
        let mut flexible: Vec<bool> = Vec::new();
        let mut unrouted: Vec<(Id<Connection>, Vec<Pt2>)> = Vec::new();
        for cv in sorted_connections(view) {
            let c = &cv.connection;
            let Some(pts) = polyline_of(view, &cps, c, page) else {
                self.out
                    .findings
                    .push(PageFinding::UnresolvedEnd { connection: c.id });
                continue;
            };
            if pts.len() < 2 {
                continue;
            }
            if c.route.is_empty() {
                unrouted.push((c.id, pts));
                continue;
            }
            drawn.push((c.id, pts));
            flexible.push(matches!(&c.kind, ConnectionKind::Fluid(f) if f.line_kind == LineKind::Flexible));
            strokes.push(match &c.kind {
                ConnectionKind::Fluid(f) if matches!(f.line_kind, LineKind::Pilot | LineKind::Drain) => {
                    stroke.clone().dashed(DashPattern::Dash42)
                }
                _ => stroke.clone(),
            });
        }
        let hopped = match view.crossings {
            Crossings::Plain => Vec::new(),
            Crossings::Hops => hops(&drawn.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>()),
        };
        // The wires of one pin that share their run out of it part with a bevel (docs/design/06a-drafting.md §3.4).
        let mut lines: Vec<Vec<Pt2>> = drawn.iter().map(|(_, p)| p.clone()).collect();
        let mut at_pin: BTreeMap<(Uuid, u16), Vec<usize>> = BTreeMap::new();
        for (k, (id, _)) in drawn.iter().enumerate() {
            if let Some(cv) = view.connections.iter().find(|c| c.connection.id == *id) {
                for e in [&cv.connection.a, &cv.connection.b] {
                    if let Endpoint::Cp(f, idx) = e {
                        at_pin.entry((f.as_uuid(), *idx)).or_default().push(k);
                    }
                }
            }
        }
        let pins: Vec<(Pt2, Vec<usize>)> = at_pin
            .into_iter()
            .filter_map(|(key, ks)| Some((cps.get(&key)?.0, ks)))
            .collect();
        let step = Nm(view.grid.at(1).map_or(0, |n| n.0) - view.grid.at(0).map_or(0, |n| n.0));
        crate::route::bevel_partings(&mut lines, &pins, step);
        // A text a line runs through stays readable: the line is cut round it.
        let cut = |path: Path| crate::knockout::knock_out(&path, &view.knockouts).unwrap_or(path);
        for (k, (id, _)) in drawn.iter().enumerate() {
            let pts = &lines[k];
            let path = match hopped.get(k) {
                _ if flexible[k] => flexible_arc(pts),
                Some(h) if !h.is_empty() => hop_path(&drawn[k].1, h),
                _ => Path::polyline(pts),
            };
            self.push(
                Item::path(cut(path), Some(strokes[k].clone()), None).with_entity(EntityRef(id.as_uuid())),
            );
        }
        for (id, pts) in &unrouted {
            let dashed = Stroke::pen(Pen::P013).dashed(DashPattern::Dash42);
            self.push(
                Item::path(cut(Path::polyline(pts)), Some(dashed), None).with_entity(EntityRef(id.as_uuid())),
            );
        }
        let mut junctions: Vec<&JunctionView> = view.junctions.iter().collect();
        junctions.sort_by_key(|j| j.junction.id);
        for jv in junctions {
            let j = &jv.junction;
            let entity = EntityRef(j.id.as_uuid());
            let orient = match &j.style {
                JunctionStyle::Bevel(b) => b.orient,
                _ => Orient2::R0,
            };
            match &jv.symbol {
                Some(s) => {
                    let key = self.geometry(s, orient);
                    self.push(Item::instance(key, Transform::from_translation(j.at)).with_entity(entity));
                }
                None if matches!(j.style, JunctionStyle::Dot) => self.push(
                    Item::path(Path::circle(j.at, JUNCTION_DOT_RADIUS), None, Some(Brush::BLACK))
                        .with_entity(entity),
                ),
                None => {}
            }
        }
        self.bridges(view);
    }

    /// The bridges of the page's strips (module documentation, "Bridges").
    fn bridges(&mut self, view: &PageView) {
        let stroke = wire_stroke(view.grid);
        // The vertical leads of the symbols drawn alone: (x, the insertion point's y, the pin's y).
        let reps: BTreeMap<Uuid, &RepView> = view.reps.iter().map(|r| (r.rep.id.as_uuid(), r)).collect();
        let leads: Vec<(i64, i64, i64)> = page_drawn_cps(view)
            .values()
            .filter(|(_, dir, _)| matches!(dir, Dir4::Up | Dir4::Down))
            .filter_map(|(at, _, rep)| {
                let c = reps.get(rep).filter(|r| r.terminals.is_empty())?.rep.pos;
                (c.x == at.x && c.y != at.y).then_some((at.x.0, c.y.0, at.y.0))
            })
            .collect();
        // A hop a quarter of a grid step high, so it stays clear of the bodies and of a block's other bridge.
        let r = view.grid.at(1).map_or(crate::hops::HOP_RADIUS, |s| s.0 / 4);
        for points in Self::bridge_rows(view) {
            let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
                continue;
            };
            let y = first.y.0;
            // A lead the bridge does not join, crossed between its points, is hopped over.
            let crossed: Vec<&(i64, i64, i64)> = leads
                .iter()
                .filter(|(x, c, at)| {
                    first.y == last.y
                        && first.x.0 < *x
                        && *x < last.x.0
                        && (*c).min(*at) < y
                        && y < (*c).max(*at)
                        && !points.iter().any(|p| p.x.0 == *x)
                })
                .collect();
            let mut xs: Vec<i64> = crossed.iter().map(|(x, _, _)| *x).collect();
            xs.sort_unstable();
            xs.dedup();
            // A level-1 bridge (half way along the leads) hops towards the bodies, a level-2 bridge (nearer the clamps)
            // towards the clamps, so the two never meet.
            let below = crossed.first().is_some_and(|(_, c, at)| {
                let towards_pin = (y - c).abs() > (at - y).abs();
                (at > c) == towards_pin
            });
            let path = if xs.is_empty() {
                Path::line(first, last)
            } else {
                crate::hops::hop_path_with(&[first, last], &[(0, xs)], r, below)
            };
            self.push(Item::path(path, Some(stroke.clone()), None));
            for p in points {
                self.push(Item::path(
                    Path::circle(p, JUNCTION_DOT_RADIUS),
                    None,
                    Some(Brush::BLACK),
                ));
            }
        }
        let tie = Stroke::pen(Pen::P013).dashed(DashPattern::Dash42);
        for (a, b) in Self::level_ties(view) {
            self.push(Item::path(Path::line(a, b), Some(tie.clone()), None));
        }
    }

    /// The ties of the page's blocks (module documentation, "Level ties"): per pair of neighbouring symbols of one
    /// function, one of them drawn through a pin map (a block's level from 2), that stand on one height at most three
    /// grid steps apart, the dashed line between their bodies half a grid step above their middle, [`LEVEL_TIE_GAP`]
    /// clear of each body.
    pub(crate) fn level_ties(view: &PageView) -> Vec<(Pt2, Pt2)> {
        let most = view.grid.at(3).unwrap_or(Nm(15_000_000));
        let mut by_function: BTreeMap<Uuid, Vec<&RepView>> = BTreeMap::new();
        for rv in &view.reps {
            if let [f] = rv.rep.functions.as_slice()
                && rv.terminals.is_empty()
                && rv.symbol.is_some()
            {
                by_function.entry(f.as_uuid()).or_default().push(rv);
            }
        }
        let mut out = Vec::new();
        for (_, mut reps) in by_function {
            if reps.len() < 2 || reps.iter().all(|r| r.rep.pin_map.is_empty()) {
                continue;
            }
            reps.sort_by_key(|r| (r.rep.pos.y, r.rep.pos.x, r.rep.id));
            for w in reps.windows(2) {
                let (a, b) = (&w[0].rep, &w[1].rep);
                let (Some(da), Some(db)) = (&w[0].symbol, &w[1].symbol) else {
                    continue;
                };
                if a.pos.y != b.pos.y || b.pos.x - a.pos.x > most {
                    continue;
                }
                // The bodies: the extent of the symbols' drawn graphics (their texts are left out).
                let drawn = |def: &SymbolDef, orient| {
                    symbol::lower(def, orient)
                        .graphics
                        .items
                        .iter()
                        .filter_map(|i| match &i.kind {
                            ItemKind::Path(p) => p.path.bounds(),
                            _ => None,
                        })
                        .reduce(Rect::union)
                };
                let (Some(ga), Some(gb)) = (drawn(da, a.orient), drawn(db, b.orient)) else {
                    continue;
                };
                let x0 = a.pos.x + ga.max().x + LEVEL_TIE_GAP;
                let x1 = b.pos.x + gb.min().x - LEVEL_TIE_GAP;
                // Half a grid step above the bodies' middle: clear of the numbers printed beside the bodies and of a
                // bridge across the lower leads.
                let y = a.pos.y - Nm(view.grid.at(1).unwrap_or(Nm(2_000_000)).0 / 2);
                if x0 < x1 {
                    out.push((Pt2::new(x0, y), Pt2::new(x1, y)));
                }
            }
        }
        out
    }

    /// The lines of the page's bridges (module documentation, "Bridges"): per bridge in the view's order, per row of
    /// its terminals, the points it joins along the line, at least two; the line runs from the first to the last.
    pub(crate) fn bridge_rows(view: &PageView) -> Vec<Vec<Pt2>> {
        let cps = page_drawn_cps(view);
        let reps: BTreeMap<Uuid, &RepView> = view.reps.iter().map(|r| (r.rep.id.as_uuid(), r)).collect();
        let mut out = Vec::new();
        for bridge in &view.bridges {
            // Each terminal's point half way along its bridged lead, by the way the lead leaves and the line across it.
            let mut rows: BTreeMap<(u8, i64), Vec<Pt2>> = BTreeMap::new();
            for (function, idx) in &bridge.clamps {
                let Some((at, dir, rep)) = cps.get(&(function.as_uuid(), *idx)) else {
                    continue;
                };
                let Some(rv) = reps.get(rep) else {
                    continue;
                };
                // The terminal's centre: its insertion point where it is drawn alone; in a strip segment, half way
                // between its side-1 and side-2 points.
                let c = if rv.terminals.is_empty() {
                    rv.rep.pos
                } else {
                    let Some(t) = rv.terminals.iter().find(|t| t.function == *function) else {
                        continue;
                    };
                    let side = |idxs: &[u16]| {
                        idxs.iter()
                            .find_map(|i| cps.get(&(function.as_uuid(), *i)))
                            .map(|(p, _, _)| *p)
                    };
                    let (Some(a), Some(b)) = (side(&t.side1), side(&t.side2)) else {
                        continue;
                    };
                    Pt2::new(
                        Nm(a.x.0 + (b.x.0 - a.x.0).div_euclid(2)),
                        Nm(a.y.0 + (b.y.0 - a.y.0).div_euclid(2)),
                    )
                };
                // A block's level from 2 (a symbol drawn through a pin map) is bridged three quarters of the way to its
                // clamp, so its bridge runs apart from the level-1 bridge along the same leads.
                let (num, den) = if rv.rep.pin_map.is_empty() { (1, 2) } else { (3, 4) };
                let mid = Pt2::new(
                    Nm(c.x.0 + ((at.x.0 - c.x.0) * num).div_euclid(den)),
                    Nm(c.y.0 + ((at.y.0 - c.y.0) * num).div_euclid(den)),
                );
                let key = match dir {
                    Dir4::Up => (0, mid.y.0),
                    Dir4::Down => (1, mid.y.0),
                    Dir4::Left => (2, mid.x.0),
                    Dir4::Right => (3, mid.x.0),
                    Dir4::Unknown(_) => continue,
                };
                rows.entry(key).or_default().push(mid);
            }
            for ((side, _), mut points) in rows {
                points.sort_by_key(|p| if side < 2 { p.x } else { p.y });
                points.dedup();
                if points.len() >= 2 {
                    out.push(points);
                }
            }
        }
        out
    }

    /// The crossings of the connections `ids` drawn on the page with the line through `at` (horizontal or
    /// vertical), along the line ([`ew_drafting::marks::crossing`]); a connection the page does not draw, or one the line misses,
    /// has none.
    pub(crate) fn marker_crossings(
        view: &PageView,
        ids: &[Id<Connection>],
        at: Pt2,
        horizontal: bool,
    ) -> Vec<Nm> {
        let cps = cp_map(view);
        let page = page_rect(view);
        ids.iter()
            .filter_map(|id| view.connections.iter().find(|c| c.connection.id == *id))
            .filter_map(|cv| polyline_of(view, &cps, &cv.connection, page))
            .filter_map(|pts| ew_drafting::marks::crossing(&pts, at, horizontal))
            .collect()
    }
}

/// The arc of a flexible fluid line through the ends of its route `pts` (ISO 1219-1, a hose): a quadratic curve whose
/// control point stands a quarter of the chord off the chord's middle, to the left of the direction from the first
/// end to the last; the straight line where both ends coincide.
fn flexible_arc(pts: &[Pt2]) -> Path {
    let (Some(a), Some(b)) = (pts.first(), pts.last()) else {
        return Path::polyline(pts);
    };
    let (dx, dy) = (i128::from(b.x.0 - a.x.0), i128::from(b.y.0 - a.y.0));
    if dx == 0 && dy == 0 {
        return Path::polyline(pts);
    }
    // The middle of the chord, moved by a quarter of the chord turned a quarter left: (-dy, dx) / 4.
    let mid = |p: i64, q: i64| (i128::from(p) + i128::from(q)) / 2;
    let sat = |v: i128| Nm(i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX }));
    let control = Pt2::new(sat(mid(a.x.0, b.x.0) - dy / 4), sat(mid(a.y.0, b.y.0) + dx / 4));
    Path::builder().move_to(*a).quad_to(control, *b).build()
}
