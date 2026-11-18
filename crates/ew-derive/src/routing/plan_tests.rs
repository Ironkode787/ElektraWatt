// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page solve on hand-built and generated pages, judged by the layout
//! measures. Units are millimetres on a 1 mm grid.

use ew_ids::Uuid;
use ew_schema::Dir4;

use super::*;

const MM: i64 = 1_000_000;

fn mm(x: i64, y: i64) -> Point {
    (x * MM, y * MM)
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// An empty page on a 1 mm grid, a sheet from (−40, −40) to (80, 80) mm without a frame grid.
fn page() -> PageGeometry {
    PageGeometry {
        grid: (MM, 1),
        sheet: [-40 * MM, -40 * MM, 80 * MM, 80 * MM],
        ..PageGeometry::default()
    }
}

/// Adds CP `n` at `at` leaving `dir`, drawn by representation `n + 1000`, ending connection `c`.
fn add_cp(p: &mut PageGeometry, n: u128, at: Point, dir: Dir4, c: u128) {
    p.cps.push(CpPoint {
        id: id(n),
        at,
        dir: Some(dir),
        rep: id(n + 1000),
        connections: vec![id(c)],
    });
}

/// Adds connection `c` from CP `a` to CP `b` (not routed), with a supply rank.
fn add_job(
    p: &mut PageGeometry,
    c: u128,
    (a, a_at, a_dir): (u128, Point, Dir4),
    (b, b_at, b_dir): (u128, Point, Dir4),
    supply: Option<u8>,
) {
    add_cp(p, a, a_at, a_dir, c);
    add_cp(p, b, b_at, b_dir, c);
    p.connections.push(DrawnConnection {
        id: id(c),
        ends: [id(a), id(b)],
        routed: false,
        supply,
        polyline: vec![a_at, b_at],
    });
}

fn body(p: &mut PageGeometry, rep: u128, a: Point, b: Point) {
    p.bodies.push(Body {
        rep: id(rep),
        rect: [a.0, a.1, b.0, b.1],
    });
}

/// The page with the plan's routes stored, for the measures; a connection left unrouted is no wire (D3: it is
/// drawn as a dashed line, which the measures do not read).
fn routed(p: &PageGeometry, plan: &RoutePlan) -> PageGeometry {
    let mut out = p.clone();
    for c in &mut out.connections {
        if let Some((_, r)) = plan.routes.iter().find(|(id, _)| *id == c.id) {
            c.polyline = r.clone();
            c.routed = true;
        }
    }
    out.connections.retain(|c| c.routed);
    out
}

fn jobs(p: &PageGeometry) -> Vec<Uuid> {
    p.connections.iter().filter(|c| !c.routed).map(|c| c.id).collect()
}

#[test]
fn a_crossing_is_taken_over_a_detour_of_more_than_six_steps_and_a_shorter_detour_over_a_crossing() {
    // A U from (0, 0) down and back up to (20, 0) past a fixed wire at x = 10 that reaches down to `depth`: crossing
    // it at y = 1 costs 12; passing below it costs two steps more per step of depth.
    let case = |depth: i64| {
        let mut p = page();
        add_job(
            &mut p,
            1,
            (10, mm(0, 0), Dir4::Down),
            (11, mm(20, 0), Dir4::Down),
            None,
        );
        p.connections.push(DrawnConnection {
            id: id(2),
            routed: true,
            polyline: vec![mm(10, -1), mm(10, depth)],
            ..DrawnConnection::default()
        });
        route_page(&p, &[id(1)])
    };
    // Below a wire to y = 4: 8 more steps (16) against a crossing (12): the crossing.
    assert_eq!(
        case(4).routes,
        vec![(id(1), vec![mm(0, 0), mm(0, 1), mm(20, 1), mm(20, 0)])]
    );
    // Below a wire to y = 2: 4 more steps (8): the detour.
    assert_eq!(
        case(2).routes,
        vec![(id(1), vec![mm(0, 0), mm(0, 3), mm(20, 3), mm(20, 0)])]
    );
}

#[test]
fn a_supply_connection_claims_the_one_track_before_a_signal_connection() {
    // A one-track channel at y = 10 between two bodies; the signal wire is given first and could take it.
    let mut p = page();
    add_job(
        &mut p,
        2,
        (20, mm(1, 13), Dir4::Up),
        (21, mm(39, 13), Dir4::Up),
        None,
    );
    add_job(
        &mut p,
        1,
        (10, mm(0, 10), Dir4::Right),
        (11, mm(40, 10), Dir4::Left),
        Some(0),
    );
    body(&mut p, 900, mm(5, 0), mm(35, 9));
    body(&mut p, 901, mm(5, 11), mm(35, 20));
    let plan = route_page(&p, &[id(2), id(1)]);
    assert!(
        plan.routes.contains(&(id(1), vec![mm(0, 10), mm(40, 10)])),
        "{plan:?}"
    );
    assert_eq!(plan.unrouted, vec![]);
    let m = measure(&routed(&p, &plan));
    assert_eq!(m.hard(), 0, "{m:?}");
}

/// Three walls with a track at x = 10 and one at x = 20 between y = 0 and 20, a block above between the tracks, so
/// that job B, straight down the first track from (10, −5) to (10, 25), has no other way.
fn walls() -> PageGeometry {
    let mut p = page();
    p.drawing_area = Some([-10 * MM, -25 * MM, 30 * MM, 30 * MM]);
    body(&mut p, 900, mm(-10, 0), mm(9, 20));
    body(&mut p, 901, mm(11, 0), mm(19, 20));
    body(&mut p, 902, mm(21, 0), mm(30, 20));
    body(&mut p, 904, mm(11, -10), mm(19, 0));
    add_job(
        &mut p,
        2,
        (20, mm(10, -5), Dir4::Down),
        (21, mm(10, 25), Dir4::Up),
        None,
    );
    p
}

#[test]
fn a_rip_up_round_routes_what_the_first_pass_failed() {
    // B runs down a closed track at x = 10 from (10, −1) to (10, 21); A (the longer span, routed first) passes
    // through the track, in by a gap in its left wall at y = 10 and out by one in its right wall at y = 15, which
    // blocks B; A can also go round below the walls. The round rips A up, routes B and routes A round.
    let mut p = page();
    p.drawing_area = Some([-10 * MM, -5 * MM, 30 * MM, 40 * MM]);
    body(&mut p, 900, mm(-10, 0), mm(9, 9));
    body(&mut p, 901, mm(-5, 11), mm(9, 20));
    body(&mut p, 902, mm(11, 0), mm(19, 14));
    body(&mut p, 903, mm(11, 16), mm(19, 20));
    add_job(
        &mut p,
        2,
        (20, mm(10, -1), Dir4::Down),
        (21, mm(10, 21), Dir4::Up),
        None,
    );
    add_job(
        &mut p,
        1,
        (10, mm(-8, 10), Dir4::Right),
        (11, mm(25, 15), Dir4::Left),
        None,
    );
    let plan = route_page(&p, &jobs(&p));
    assert_eq!(plan.unrouted, vec![]);
    assert!(
        plan.routes.contains(&(id(2), vec![mm(10, -1), mm(10, 21)])),
        "{plan:?}"
    );
    let a = &plan.routes.iter().find(|(c, _)| *c == id(1)).unwrap().1;
    assert!(a.iter().all(|v| v.0 != 10 * MM), "{a:?}");
    assert_eq!(measure(&routed(&p, &plan)).hard(), 0);
}

#[test]
fn a_job_no_round_routes_is_unrouted_with_its_reason_and_nothing_drawn_through() {
    // C needs the same track as B and has no other: one of them stays unrouted, for the other wire, after a round.
    let mut p = walls();
    body(&mut p, 903, mm(19, 0), mm(21, 20));
    add_job(
        &mut p,
        3,
        (30, mm(10, -7), Dir4::Down),
        (31, mm(10, 27), Dir4::Up),
        None,
    );
    let plan = route_page(&p, &jobs(&p));
    assert_eq!(plan.routes.len() + plan.unrouted.len(), 2, "{plan:?}");
    assert_eq!(plan.unrouted.len(), 1, "{plan:?}");
    assert_eq!(plan.unrouted[0].1, Unrouted::Wires);
    let m = measure(&routed(&p, &plan));
    assert_eq!(m.hard(), 0, "{plan:?} {m:?}");
}

#[test]
fn three_mates_run_parallel_one_step_apart_in_their_order() {
    // An L1 L2 L3 stack at the left edge to a breaker's three poles lower right.
    let mut p = page();
    for (k, phase) in [(0, 0u8), (1, 1), (2, 2)] {
        let c = 1 + u128::from(phase);
        add_job(
            &mut p,
            c,
            (10 + u128::from(phase), mm(0, k), Dir4::Right),
            (20 + u128::from(phase), mm(22 - k, 10), Dir4::Up),
            Some(phase),
        );
    }
    let plan = route_page(&p, &jobs(&p));
    assert_eq!(plan.unrouted, vec![]);
    let m = measure(&routed(&p, &plan));
    assert_eq!((m.hard(), m.crossings.len()), (0, 0), "{plan:?}");
    // Each route turns once, and the corners step one grid step down and right: parallel at one step.
    let corner = |c: u128| {
        plan.routes
            .iter()
            .find(|(id_, _)| *id_ == id(c))
            .unwrap()
            .1
            .clone()
    };
    assert_eq!(corner(1), vec![mm(0, 0), mm(22, 0), mm(22, 10)]);
    assert_eq!(corner(2), vec![mm(0, 1), mm(21, 1), mm(21, 10)]);
    assert_eq!(corner(3), vec![mm(0, 2), mm(20, 2), mm(20, 10)]);
}

#[test]
fn the_plan_does_not_depend_on_the_order_of_the_lists_or_the_jobs() {
    let mut p = walls();
    add_job(
        &mut p,
        1,
        (10, mm(12, -20), Dir4::Down),
        (11, mm(12, 26), Dir4::Up),
        None,
    );
    add_job(
        &mut p,
        3,
        (30, mm(-5, -8), Dir4::Right),
        (31, mm(25, 28), Dir4::Left),
        Some(4),
    );
    let plan = route_page(&p, &jobs(&p));
    let mut q = p.clone();
    q.connections.reverse();
    q.cps.reverse();
    q.bodies.reverse();
    let mut js = jobs(&q);
    js.reverse();
    assert_eq!(route_page(&q, &js), plan);
    assert_eq!(route_page(&p, &jobs(&p)), plan);
}

#[test]
fn g_a_job_that_would_be_a_junction_s_fourth_arm_is_unrouted() {
    let mut p = page();
    p.junctions.push(JunctionPoint {
        id: id(50),
        at: mm(0, 0),
        connections: vec![id(1), id(2), id(3), id(4)],
    });
    for (c, to) in [(1, mm(0, -10)), (2, mm(10, 0)), (3, mm(0, 10))] {
        p.connections.push(DrawnConnection {
            id: id(c),
            ends: [id(50), Uuid::nil()],
            routed: true,
            polyline: vec![mm(0, 0), to],
            ..DrawnConnection::default()
        });
    }
    add_cp(&mut p, 60, mm(-10, 0), Dir4::Right, 4);
    p.connections.push(DrawnConnection {
        id: id(4),
        ends: [id(50), id(60)],
        polyline: vec![mm(0, 0), mm(-10, 0)],
        ..DrawnConnection::default()
    });
    let plan = route_page(&p, &[id(4)]);
    assert_eq!(plan.unrouted, vec![(id(4), Unrouted::FourArms)]);
    assert_eq!(plan.routes, vec![]);
}

#[test]
fn a_job_whose_stub_a_text_crosses_is_routed_across_the_text_and_says_so() {
    // The pin text of the next pole lies over a motor pole's stub, between the CP and
    // its stub node. No strict route exists, whichever end the CP is; the text's own representation is the CP's. The
    // page solve routes it across the text and names it (the persona re-test: one through a text is better than none),
    // and the Wire tool's search finds the same route across the text (ADR-0033 amendment 2: a text has a cost).
    let mut p = page();
    add_job(
        &mut p,
        1,
        (10, mm(0, 0), Dir4::Up),
        (11, mm(20, -10), Dir4::Left),
        None,
    );
    p.texts.push(TextBox {
        rep: id(1010),
        rect: tenths(-30, -6, 5, -3),
    });
    let ends = |k: usize| {
        let c = &p.cps[k];
        OneEnd {
            at: c.at,
            dir: Some(if k == 0 { Heading::Up } else { Heading::Left }),
            rep: Some(c.rep),
        }
    };
    let plan = route_page(&p, &[id(1)]);
    assert_eq!(plan.unrouted, vec![]);
    assert_eq!(plan.over_texts, vec![id(1)]);
    let one = route_one(&p, ends(0), ends(1), None).expect("a route across the text");
    assert!(crosses_text(&p, &one), "{one:?}");
    // Without the text, the same wire is routed.
    p.texts.clear();
    assert_eq!(route_page(&p, &[id(1)]).unrouted, vec![]);
}

/// RT1's page: symbols at the lattice `cells`, each a 2 × 2 mm body at (10 i, 10 j) with CP 2k on its left leaving
/// left and CP 2k + 1 on its right leaving right, and a connection for each of `pairs` between two free CPs (indices
/// modulo the CPs).
fn lattice(cells: &[(i64, i64)], pairs: &[(usize, usize)]) -> PageGeometry {
    let mut p = page();
    p.sheet = [-10 * MM, -10 * MM, 70 * MM, 70 * MM];
    // Symbol k: a 2 × 2 mm body at (10 i, 10 j), CP 2k on its left leaving left, 2k + 1 on its right leaving right.
    for (k, (i, j)) in cells.iter().enumerate() {
        let k = u128::try_from(k).unwrap();
        let (x, y) = (10 * i, 10 * j);
        p.bodies.push(Body {
            rep: id(1000 + k),
            rect: [x * MM, (y - 1) * MM, (x + 2) * MM, (y + 1) * MM],
        });
        for (n, at, dir) in [
            (2 * k, mm(x, y), Dir4::Left),
            (2 * k + 1, mm(x + 2, y), Dir4::Right),
        ] {
            p.cps.push(CpPoint {
                id: id(100 + n),
                at,
                dir: Some(dir),
                rep: id(1000 + k),
                connections: vec![],
            });
        }
    }
    let n_cps = p.cps.len();
    let mut used = std::collections::BTreeSet::new();
    for (c, (a, b)) in pairs.iter().enumerate() {
        let (a, b) = (a % n_cps, b % n_cps);
        if a == b || !used.insert(a) || !used.insert(b) {
            continue;
        }
        let c = id(5000 + u128::try_from(c).unwrap());
        p.cps[a].connections.push(c);
        p.cps[b].connections.push(c);
        p.connections.push(DrawnConnection {
            id: c,
            ends: [p.cps[a].id, p.cps[b].id],
            polyline: vec![p.cps[a].at, p.cps[b].at],
            ..DrawnConnection::default()
        });
    }
    p
}

/// A rectangle in tenths of a millimetre.
fn tenths(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
    [x0 * MM / 10, y0 * MM / 10, x1 * MM / 10, y1 * MM / 10]
}

/// Whether a text box, grown by a quarter grid step as the router grows it, lies across the stub of `cp` (the move
/// from its point one grid step along its direction): the stub's line strictly inside the box across it, and a
/// stretch of the stub inside the box along it.
fn across_stub(text: Rect, cp: &CpPoint) -> bool {
    let q = MM / 4;
    let [x0, y0, x1, y1] = [text[0] - q, text[1] - q, text[2] + q, text[3] + q];
    let (x, y) = cp.at;
    match cp.dir {
        Some(Dir4::Left) | Some(Dir4::Right) => {
            let sx = if cp.dir == Some(Dir4::Left) {
                x - MM
            } else {
                x + MM
            };
            y0 < y && y < y1 && x.min(sx).max(x0) < x.max(sx).min(x1)
        }
        Some(Dir4::Up) | Some(Dir4::Down) => {
            let sy = if cp.dir == Some(Dir4::Up) { y - MM } else { y + MM };
            x0 < x && x < x1 && y.min(sy).max(y0) < y.max(sy).min(y1)
        }
        _ => false,
    }
}

proptest::proptest! {
    /// RT1 and RT8: on a generated page of symbols at least four grid steps apart, each with two CPs on its left and
    /// right, the connections between random CPs: every route the solve returns breaks no hard rule by the measures,
    /// and a sparse page (at most three connections among six symbols on a 6 × 6 lattice) is routed in full.
    #[test]
    fn every_returned_route_keeps_the_hard_rules(
        cells in proptest::collection::btree_set((0i64..6, 0i64..6), 2..9),
        pairs in proptest::collection::vec((0usize..64, 0usize..64), 1..7),
    ) {
        let cells: Vec<(i64, i64)> = cells.into_iter().collect();
        let p = lattice(&cells, &pairs);
        let plan = route_page(&p, &jobs(&p));
        let m = measure(&routed(&p, &plan));
        let mine = |c: &Uuid| plan.routes.iter().any(|(r, _)| r == c);
        proptest::prop_assert!(!m.touches.iter().any(|t| mine(&t.connection)), "{:?}", m.touches);
        proptest::prop_assert!(!m.overlaps.iter().any(|o| mine(&o.connections[0]) && mine(&o.connections[1])), "{:?}", m.overlaps);
        proptest::prop_assert!(!m.through_bodies.iter().any(|t| mine(&t.connection)), "{:?}", m.through_bodies);
        if cells.len() >= 6 && p.connections.len() <= 3 {
            proptest::prop_assert_eq!(&plan.unrouted, &vec![]);
        }
    }

    /// RT1 among texts, junctions and fixed wires: RT1's page with text boxes near the symbols' stubs
    /// (across one, over one's node, beside one, above a body, below a CP) and near junctions; junctions in the middle
    /// of the lattice's squares, each with one to three fixed wires leaving it along the middle lines; and connections
    /// from free CPs to the junctions. Every route the solve returns keeps measures (a) to (d), no junction has a
    /// fourth arm (g), and a connection whose stub a text crosses is not routed (§5.1 rule 4 has no exemption).
    #[test]
    fn every_returned_route_keeps_the_hard_rules_among_texts_junctions_and_fixed_wires(
        cells in proptest::collection::btree_set((0i64..6, 0i64..6), 2..9),
        pairs in proptest::collection::vec((0usize..64, 0usize..64), 1..7),
        texts in proptest::collection::vec((0usize..64, 0usize..8), 1..7),
        hubs in proptest::collection::btree_map((0i64..5, 0i64..5), 1u8..15, 0..4),
        hub_jobs in proptest::collection::vec((0usize..64, 0usize..64), 0..5),
    ) {
        let cells: Vec<(i64, i64)> = cells.into_iter().collect();
        let mut p = lattice(&cells, &pairs);
        let hubs: Vec<((i64, i64), u8)> = hubs.into_iter().collect();
        // Junction h in the middle of a lattice square, with a fixed wire of 3 mm along a middle line for each bit of
        // its mask (right, down, left, up; the mask is below 15, so at most three).
        for (h, ((i, j), mask)) in hubs.iter().enumerate() {
            let h = u128::try_from(h).unwrap();
            let at = mm(10 * i + 5, 10 * j + 5);
            let mut arms = Vec::new();
            for (bit, (dx, dy)) in [(3, 0), (0, 3), (-3, 0), (0, -3)].into_iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    let c = id(7000 + 10 * h + u128::try_from(bit).unwrap());
                    arms.push(c);
                    p.connections.push(DrawnConnection {
                        id: c,
                        ends: [id(6000 + h), Uuid::nil()],
                        routed: true,
                        polyline: vec![at, (at.0 + dx * MM, at.1 + dy * MM)],
                        ..DrawnConnection::default()
                    });
                }
            }
            p.junctions.push(JunctionPoint { id: id(6000 + h), at, connections: arms });
        }
        let n_cps = p.cps.len();
        for (c, (h, a)) in hub_jobs.iter().enumerate() {
            if hubs.is_empty() || !p.cps[a % n_cps].connections.is_empty() {
                continue;
            }
            let (h, a) = (h % hubs.len(), a % n_cps);
            let c = id(8000 + u128::try_from(c).unwrap());
            p.cps[a].connections.push(c);
            p.junctions[h].connections.push(c);
            p.connections.push(DrawnConnection {
                id: c,
                ends: [p.cps[a].id, p.junctions[h].id],
                polyline: vec![p.cps[a].at, p.junctions[h].at],
                ..DrawnConnection::default()
            });
        }
        for (k, kind) in &texts {
            let (rect, rep) = if *kind < 6 {
                let k = k % cells.len();
                let (x, y) = (100 * cells[k].0, 100 * cells[k].1);
                let rect = match kind {
                    // Across the left stub, between the CP and its stub node.
                    0 => tenths(x - 7, y - 4, x - 3, y + 4),
                    // Across the right stub.
                    1 => tenths(x + 23, y - 4, x + 27, y + 4),
                    // A pin text over the left stub's node and beyond.
                    2 => tenths(x - 30, y - 5, x - 5, y + 5),
                    // Beside the right stub, above it.
                    3 => tenths(x + 20, y - 20, x + 40, y - 10),
                    // A tag above the body.
                    4 => tenths(x, y - 30, x + 20, y - 15),
                    // Below the left CP.
                    _ => tenths(x - 30, y + 10, x, y + 20),
                };
                (rect, id(1000 + u128::try_from(k).unwrap()))
            } else if hubs.is_empty() {
                continue;
            } else {
                let h = k % hubs.len();
                let (x, y) = (100 * hubs[h].0.0 + 50, 100 * hubs[h].0.1 + 50);
                let rect = if *kind == 6 {
                    // Beside the junction, above its right arm.
                    tenths(x + 10, y - 15, x + 30, y - 5)
                } else {
                    // Across the line left of the junction.
                    tenths(x - 16, y - 4, x - 12, y + 4)
                };
                (rect, id(9000 + u128::try_from(h).unwrap()))
            };
            p.texts.push(TextBox { rep, rect });
        }
        let plan = route_page(&p, &jobs(&p));
        let m = measure(&routed(&p, &plan));
        let mine = |c: &Uuid| plan.routes.iter().any(|(r, _)| r == c);
        // A route across a text gives up rule (d) alone, and the plan names it.
        let strict = |c: &Uuid| mine(c) && !plan.over_texts.contains(c);
        proptest::prop_assert!(!m.touches.iter().any(|t| mine(&t.connection)), "{:?}", m.touches);
        proptest::prop_assert!(!m.overlaps.iter().any(|o| mine(&o.connections[0]) || mine(&o.connections[1])), "{:?}", m.overlaps);
        proptest::prop_assert!(!m.through_bodies.iter().any(|t| mine(&t.connection)), "{:?}", m.through_bodies);
        proptest::prop_assert!(!m.through_texts.iter().any(|t| strict(&t.connection)), "{:?}", m.through_texts);
        proptest::prop_assert_eq!(&m.four_way, &vec![]);
        for cp in &p.cps {
            if p.texts.iter().any(|t| across_stub(t.rect, cp)) {
                proptest::prop_assert!(!cp.connections.iter().any(strict), "{:?} {:?}", cp, plan.routes);
            }
        }
    }
}

/// A rectangle in units of 10 µm (the persona pages' measured boxes).
fn um10(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
    [x0 * 10_000, y0 * 10_000, x1 * 10_000, y1 * 10_000]
}

/// The length of a polyline, nm.
fn length(line: &[Point]) -> i64 {
    line.windows(2)
        .map(|w| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs())
        .sum()
}

/// A page of the rc.2 persona runs: an A4 sheet at the QET 10 px grid with its frame's drawing area.
fn qet_page() -> PageGeometry {
    PageGeometry {
        grid: (7_937_500, 3),
        sheet: [0, 0, 275_166_667, 192_616_667],
        drawing_area: Some([5_291_667, 5_291_667, 275_166_667, 174_625_000]),
        ..PageGeometry::default()
    }
}

#[test]
fn a_text_is_crossed_rather_than_gone_round_by_a_far_longer_way_and_a_short_way_round_is_kept() {
    // A U from (0, 0) down and back up to (20, 0) past a text at x = 9..11 that reaches down from y = -20 to `depth`:
    // across it 22 mm, below it 24 + 2 × depth (ADR-0033 amendment 2: across only where the way round is more than
    // three fifths as long again).
    let case = |depth: i64| {
        let mut p = page();
        add_job(
            &mut p,
            1,
            (10, mm(0, 0), Dir4::Down),
            (11, mm(20, 0), Dir4::Down),
            None,
        );
        p.texts.push(TextBox {
            rep: id(2000),
            rect: tenths(90, -200, 110, depth * 10),
        });
        route_page(&p, &[id(1)])
    };
    // A way round a few steps deeper (2 × 4 steps more) against the crossing (the text's cost and a step more per
    // step on it): round.
    let short = case(3);
    assert_eq!(short.over_texts, Vec::<Uuid>::new());
    assert_eq!(short.unrouted, vec![]);
    // A way round 2 × 16 steps deeper: across.
    let far = case(16);
    assert_eq!(far.over_texts, vec![id(1)]);
    assert_eq!(
        far.routes,
        vec![(id(1), vec![mm(0, 0), mm(0, 1), mm(20, 1), mm(20, 0)])]
    );
}

/// The rc.2 persona run's switch -S1 and breaker -F1 side by side, the breaker to the switch's right, their tags and
/// part texts between them: the wire from the switch's pin 2 (below) to the breaker's pin 1 (above) went left round
/// the switch's tag and over the top, 90 mm for a 42 mm way. It crosses the switch's part text instead.
#[test]
fn the_wire_to_the_neighbour_is_the_short_one_across_a_text() {
    let mut p = qet_page();
    let (s, f) = (id(1001), id(1002));
    p.bodies.push(Body {
        rep: s,
        rect: um10(3043, 3440, 3506, 4233),
    });
    p.bodies.push(Body {
        rep: f,
        rect: um10(6482, 3440, 7011, 4498),
    });
    for (rep, rect) in [
        (s, um10(1824, 3592, 2910, 4060)),
        (s, um10(3638, 3667, 5197, 3991)),
        (s, um10(3241, 3440, 3347, 3692)),
        (s, um10(3241, 3969, 3347, 4221)),
        (f, um10(5274, 3724, 6350, 4192)),
        (f, um10(7144, 3799, 8872, 4124)),
        (f, um10(7144, 4130, 7567, 4454)),
        (f, um10(6681, 3440, 6787, 3692)),
        (f, um10(6681, 4234, 6787, 4486)),
    ] {
        p.texts.push(TextBox { rep, rect });
    }
    for (n, rep, at, dir, c) in [
        (10, s, (31_750_000, 34_395_833), Dir4::Up, None),
        (11, s, (31_750_000, 42_333_333), Dir4::Down, Some(1)),
        (12, f, (66_145_833, 34_395_833), Dir4::Up, Some(1)),
        (13, f, (66_145_833, 44_979_166), Dir4::Down, None),
    ] {
        p.cps.push(CpPoint {
            id: id(n),
            at,
            dir: Some(dir),
            rep,
            connections: c.map(id).into_iter().collect(),
        });
    }
    p.connections.push(DrawnConnection {
        id: id(1),
        ends: [id(11), id(12)],
        polyline: vec![p.cps[1].at, p.cps[2].at],
        ..DrawnConnection::default()
    });
    let end = |k: usize| OneEnd {
        at: p.cps[k].at,
        dir: Some(if k == 1 { Heading::Down } else { Heading::Up }),
        rep: Some(p.cps[k].rep),
    };
    let one = route_one(&p, end(1), end(2), None).expect("a route");
    let direct = (p.cps[2].at.0 - p.cps[1].at.0).abs() + (p.cps[2].at.1 - p.cps[1].at.1).abs();
    assert!(length(&one) * 2 < direct * 3, "{one:?}");
    assert!(crosses_text(&p, &one));
    let plan = route_page(&p, &[id(1)]);
    assert_eq!(plan.routes, vec![(id(1), one)]);
    assert_eq!(plan.over_texts, vec![id(1)]);
    let m = measure(&routed(&p, &plan));
    assert!(m.through_bodies.is_empty() && m.touches.is_empty(), "{m:?}");
}

/// The rc.2 persona run's PROFIBUS page: the CPU and a remote station at the top of the sheet, each with its port on
/// its box's top edge and three caption lines above-left of it, the remote's reaching up to a step under the frame:
/// the bus line ran across, round the remote's box and into its port from the right. It is the short line across the
/// remote's caption.
#[test]
fn the_bus_line_to_a_station_at_the_top_of_the_sheet_is_the_short_one() {
    let mut p = qet_page();
    let (cpu, rio) = (id(1001), id(1002));
    for (rep, rect) in [
        (cpu, um10(2646, 2117, 5292, 3704)),
        (cpu, um10(2646, 2117, 7144, 5027)),
        (rio, um10(15346, 2117, 17992, 3704)),
        (rio, um10(15346, 2117, 19844, 5027)),
    ] {
        p.bodies.push(Body { rep, rect });
    }
    for (rep, rect) in [
        (rio, um10(16474, 1582, 17198, 2051)),
        (rio, um10(15576, 1208, 17198, 1532)),
        (rio, um10(16394, 833, 17198, 1158)),
        (rio, um10(17293, 2217, 17617, 2535)),
        (cpu, um10(3774, 1582, 4498, 2051)),
        (cpu, um10(2822, 1208, 4498, 1532)),
        (cpu, um10(2712, 2183, 3213, 2435)),
        (cpu, um10(3694, 833, 4498, 1158)),
        (cpu, um10(4593, 2217, 4917, 2535)),
    ] {
        p.texts.push(TextBox { rep, rect });
    }
    for (n, rep, x) in [(10, cpu, 47_625_000), (11, rio, 174_625_000)] {
        p.cps.push(CpPoint {
            id: id(n),
            at: (x, 21_166_667),
            dir: Some(Dir4::Up),
            rep,
            connections: vec![id(1)],
        });
    }
    p.connections.push(DrawnConnection {
        id: id(1),
        ends: [id(10), id(11)],
        polyline: vec![p.cps[0].at, p.cps[1].at],
        ..DrawnConnection::default()
    });
    let plan = route_page(&p, &[id(1)]);
    assert_eq!(plan.unrouted, vec![]);
    let (_, line) = &plan.routes[0];
    let direct = p.cps[1].at.0 - p.cps[0].at.0;
    assert!(length(line) * 4 < direct * 5, "{line:?}");
    assert_eq!(line.len(), 4, "a U over the top: {line:?}");
    let m = measure(&routed(&p, &plan));
    assert!(m.through_bodies.is_empty() && m.touches.is_empty(), "{m:?}");
}

/// The rc.2 persona run's PLC page: an interruption point drawn on the CPU's M pin, which also has the 0 V wire to the
/// first card's M. The point's body lay over the pin's way out, so Route selection left the 0 V wire unrouted ("no
/// route keeps clear of the symbols"). A symbol drawn on an end's pin is that end's own: the wire leaves through it.
#[test]
fn a_wire_leaves_a_pin_through_an_interruption_point_drawn_on_it() {
    let mut p = page();
    add_job(
        &mut p,
        1,
        (10, mm(0, 0), Dir4::Down),
        (11, mm(20, 0), Dir4::Down),
        None,
    );
    body(&mut p, 1010, mm(-4, -6), mm(4, 0));
    body(&mut p, 1011, mm(16, -6), mm(24, 0));
    // The interruption point on pin 10: its one CP on the pin, its arrow just below it.
    p.cps.push(CpPoint {
        id: id(20),
        at: mm(0, 0),
        dir: Some(Dir4::Right),
        rep: id(1020),
        connections: vec![id(2)],
    });
    body(&mut p, 1020, mm(-1, 0), mm(1, 1));
    let plan = route_page(&p, &[id(1)]);
    assert_eq!(plan.unrouted, vec![]);
    let m = measure(&routed(&p, &plan));
    assert!(m.through_bodies.is_empty(), "{m:?}");
}

/// A page on the QET grid, whose step is a third of a nm off a whole number, with a wire from a pin straight down to
/// the pin below it, and a junction on that wire. The upper pin stands on the grid's own number of its column
/// (66 145 833), the lower one 1 nm beside it (66 145 834: its box's place plus the pin's offset, each rounded), so the
/// wire holds a step of 1 nm, and the junction stands on it. Returns the page with the wire from the junction to the
/// lower box's second pin, two columns to the right, as job 3, and that wire's ends.
fn junction_on_a_wire_with_a_step_of_one_nm() -> (PageGeometry, OneEnd, OneEnd) {
    let mut p = qet_page();
    let (upper, lower) = (id(1001), id(1002));
    p.bodies.push(Body {
        rep: upper,
        rect: [58_208_333, 82_020_833, 97_895_833, 100_541_667],
    });
    p.bodies.push(Body {
        rep: lower,
        rect: [60_854_167, 129_645_833, 92_604_167, 150_812_500],
    });
    let (pin, first, second) = (
        (66_145_833, 100_541_667),
        (66_145_834, 129_645_833),
        (71_437_500, 129_645_833),
    );
    let junction = (66_145_834, 113_770_833);
    for (n, rep, at, dir, c) in [
        (10, upper, pin, Dir4::Down, 1),
        (11, lower, first, Dir4::Up, 2),
        (12, lower, second, Dir4::Up, 3),
    ] {
        p.cps.push(CpPoint {
            id: id(n),
            at,
            dir: Some(dir),
            rep,
            connections: vec![id(c)],
        });
    }
    p.junctions.push(JunctionPoint {
        id: id(20),
        at: junction,
        connections: vec![id(1), id(2), id(3)],
    });
    for (c, ends, routed, polyline) in [
        (1, [10, 20], true, vec![pin, (pin.0, junction.1), junction]),
        (2, [20, 11], true, vec![junction, first]),
        (3, [20, 12], false, vec![junction, second]),
    ] {
        p.connections.push(DrawnConnection {
            id: id(c),
            ends: ends.map(id),
            routed,
            supply: None,
            polyline,
        });
    }
    let from = OneEnd {
        at: junction,
        dir: None,
        rep: None,
    };
    let to = OneEnd {
        at: second,
        dir: Some(Heading::Up),
        rep: Some(lower),
    };
    (p, from, to)
}

/// The owner's rc.4 wire: from a junction on a vertical wire to the pin beside the one below. The wire the junction
/// sits on reached it by a step of 1 nm, which the router took for a stretch of wire in the way of the first step to
/// the right: the route left to the left, went down and came back under the wire it started from. One corner is the
/// route, whichever end it is drawn from, by the Wire tool and by Reroute.
#[test]
fn a_wire_from_a_junction_takes_the_one_corner_past_a_step_of_one_nm() {
    let (p, from, to) = junction_on_a_wire_with_a_step_of_one_nm();
    let corner = (to.at.0, from.at.1);
    let plan = route_page(&p, &[id(3)]);
    assert_eq!(plan.unrouted, vec![]);
    assert_eq!(plan.routes, vec![(id(3), vec![from.at, corner, to.at])]);
    assert_eq!(route_one(&p, from, to, None), Some(vec![from.at, corner, to.at]));
    assert_eq!(route_one(&p, to, from, None), Some(vec![to.at, corner, from.at]));
    let m = measure(&routed(&p, &plan));
    assert_eq!(m.hard(), 0, "{m:?}");
}

/// The same page with the loop a file of the fourth candidate holds: the measures read the step of 1 nm as no
/// stretch of wire (no overlap with the wire that leaves the junction over it), no bend and no arm of the junction.
#[test]
fn a_step_of_one_nm_is_no_stretch_no_bend_and_no_arm() {
    let (mut p, from, to) = junction_on_a_wire_with_a_step_of_one_nm();
    let left = 63_500_000;
    let below = 116_416_667;
    p.connections[2].routed = true;
    p.connections[2].polyline = vec![from.at, (left, from.at.1), (left, below), (to.at.0, below), to.at];
    let m = measure(&p);
    assert_eq!(m.overlaps, vec![], "{m:?}");
    assert!(m.bends.iter().all(|b| b.connection != id(1)), "{:?}", m.bends);
    // The junction's arms are up, down and left (the loop): a wire from the right would be its fourth. (Read to the
    // nm, the wire from above arrives from the left over its step, and the junction seems to have two arms.)
    let mut q = p.clone();
    let far = (79_375_000, 113_770_833);
    q.cps.push(CpPoint {
        id: id(13),
        at: far,
        dir: Some(Dir4::Left),
        rep: id(1003),
        connections: vec![id(4)],
    });
    q.junctions[0].connections.push(id(4));
    q.connections.push(DrawnConnection {
        id: id(4),
        ends: [id(13), id(20)],
        routed: false,
        supply: None,
        polyline: vec![far, from.at],
    });
    assert_eq!(route_page(&q, &[id(4)]).unrouted, vec![(id(4), Unrouted::FourArms)]);
}

/// A reroute starts from the drawing: a wire that is drawn as well as the router would draw it keeps its route, also
/// where the router's tie rule would take another of the same cost, and a wire with a needless way round is mended.
/// (Routed from nothing, every wire took the tie rule's route and a person's layout was lost.)
#[test]
fn a_reroute_keeps_a_wire_that_is_well_drawn_and_mends_a_loop() {
    let mut p = page();
    // A Z between two pins that face each other six rows apart: the person stepped down in the middle, the tie rule
    // would step down last.
    add_job(
        &mut p,
        1,
        (10, mm(0, 0), Dir4::Right),
        (11, mm(10, 6), Dir4::Left),
        None,
    );
    let stepped = vec![mm(0, 0), mm(5, 0), mm(5, 6), mm(10, 6)];
    // Two pins in line, the wire between them drawn down, along and back up.
    add_job(
        &mut p,
        2,
        (20, mm(0, 20), Dir4::Right),
        (21, mm(10, 20), Dir4::Left),
        None,
    );
    let looped = vec![mm(0, 20), mm(2, 20), mm(2, 24), mm(8, 24), mm(8, 20), mm(10, 20)];
    for (c, line) in p.connections.iter_mut().zip([&stepped, &looped]) {
        c.routed = true;
        c.polyline.clone_from(line);
    }
    let plan = route_page(&p, &[id(1), id(2)]);
    assert_eq!(plan.unrouted, vec![]);
    assert_eq!(
        plan.routes,
        vec![(id(1), stepped), (id(2), vec![mm(0, 20), mm(10, 20)])]
    );
    // The same wires without a route are routed from nothing, by the tie rule.
    for c in &mut p.connections {
        c.routed = false;
    }
    let fresh = route_page(&p, &[id(1), id(2)]);
    assert_eq!(fresh.routes[0], (id(1), vec![mm(0, 0), mm(9, 0), mm(9, 6), mm(10, 6)]));
}

/// A straight wire between two pins on one grid line whose numbers differ by 1 nm keeps its ends' own numbers: the
/// route steps over between them, and a route found on the settled page ends on the points the page stores.
#[test]
fn a_route_ends_on_its_pins_own_numbers() {
    let (mut p, _, _) = junction_on_a_wire_with_a_step_of_one_nm();
    p.junctions.clear();
    p.connections.truncate(1);
    p.connections[0].ends = [id(10), id(11)];
    p.connections[0].routed = false;
    p.cps[1].connections = vec![id(1)];
    let (pin, first) = (p.cps[0].at, p.cps[1].at);
    let plan = route_page(&p, &[id(1)]);
    let (_, line) = &plan.routes[0];
    assert_eq!((line.first(), line.last()), (Some(&pin), Some(&first)), "{line:?}");
    assert!(
        line.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1),
        "{line:?}"
    );
    let m = measure(&routed(&p, &plan));
    assert_eq!((m.hard(), m.bend_total()), (0, 0), "{m:?}");
}
