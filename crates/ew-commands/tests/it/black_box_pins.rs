// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A black box's pins are connection points as a symbol's pins are (the cycle-40 play-test's blocker and majors):
//!
//! - a black box dragged by grid steps, mirrored or turned still takes a wire on every pin, although its pins (stored
//!   as nm offsets from `pos`, `BoxCp.pos`) land up to 1 nm off the rational QET grid: the CP-exact rule of
//!   `geometry::require_route` covers them, as it covers a symbol's pins;
//! - a project saved with a pin 1 nm off the grid still wires (nothing rewrites stored coordinates);
//! - a route to a pin follows the box on a drag;
//! - a black box mirrors and turns within its own bounding box (a 90° turn about its centre, snapped to the grid),
//!   while a symbol keeps its insertion-point pivot;
//! - the grid refusals name the point in words: its position in mm and its page, no nm, no JSON Pointer.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    Connect, EndpointParam, MoveReps, PasteFragment, PlaceBlackBox, PlaceSymbol, PlaceTarget, TransformReps,
};
use ew_commands::route::{Heading, RouteEnd, RouteRequest, auto_route};
use ew_commands::{CmdError, Engine};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    BoxCp, BoxSpec, Connection, ConnectionKind, Dir4, Endpoint, Extra, FnRole, Function, FunctionKind,
    Record, Representation,
};
use ew_units::{Nm, Orient2, Pt2};
use serde_json::json;

// ---------------------------------------------------------------------------------------------------------
// Helpers

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> T {
    (**e.model().table::<T>().get(&id).expect("record exists")).clone()
}

/// The coordinate of grid index `k` of the fixture pages' QET grid (`7_937_500/3` nm).
fn at(k: i64) -> i64 {
    grid().at(k).expect("in range").0
}

/// The grid point at indices `(kx, ky)`.
fn gp(kx: i64, ky: i64) -> Pt2 {
    pt(at(kx), at(ky))
}

fn on_grid(p: Pt2) -> bool {
    grid().snap_point(p.x) == Some(p.x) && grid().snap_point(p.y) == Some(p.y)
}

/// One grid step, rounded down: 2_645_833 nm.
const STEP: i64 = 2_645_833;

/// The box of the black-box tool with corners at grid indices `(kx0, ky0)` and `(kx1, ky1)`: the insertion point is
/// the top-left corner, the outline the rectangle relative to it (top-left first, clockwise), and the CPs, indices 0,
/// 1, …, on the top edge at `kx0 + tops[i]` leaving up, then on the bottom edge at `kx0 + bottoms[i]` leaving down,
/// each stored as `grid.at(k) − pos` (`ew_viewmodel::tools::black_box::layout`).
fn tool_box(k0: (i64, i64), k1: (i64, i64), tops: &[i64], bottoms: &[i64]) -> (Pt2, BoxSpec) {
    let pos = gp(k0.0, k0.1);
    let (w, h) = (at(k1.0) - pos.x.0, at(k1.1) - pos.y.0);
    let outline = vec![pt(0, 0), pt(w, 0), pt(w, h), pt(0, h)];
    let mut cps = Vec::new();
    for (i, k) in tops.iter().enumerate() {
        cps.push(box_cp(i, pt(at(k0.0 + k) - pos.x.0, 0), Dir4::Up));
    }
    for (i, k) in bottoms.iter().enumerate() {
        cps.push(box_cp(tops.len() + i, pt(at(k0.0 + k) - pos.x.0, h), Dir4::Down));
    }
    let spec = BoxSpec {
        outline,
        cps,
        inner: vec![],
        extra: Extra::new(),
    };
    (pos, spec)
}

fn box_cp(i: usize, pos: Pt2, dir: Dir4) -> BoxCp {
    BoxCp {
        idx: u16::try_from(i).expect("few CPs"),
        pos,
        dir,
        extra: Extra::new(),
    }
}

/// The box of the play-test: 6 by 4 steps at grid index (31, 46), five pins on the top edge and four on the bottom,
/// at every grid residue (the bottom edge at index 50, residue 2 against the top's 1), so that a move by a step
/// count that is not a multiple of 3, on either axis, leaves some pins 1 nm off.
fn play_box() -> (Pt2, BoxSpec) {
    tool_box((31, 46), (37, 50), &[1, 2, 3, 4, 5], &[1, 2, 4, 5])
}

/// Places a black box with `box.black` on page 101 and returns its representation and function.
fn place_box(e: &mut Engine, pos: Pt2, spec: BoxSpec) -> (Id<Representation>, Id<Function>) {
    let r = e
        .execute(&PlaceBlackBox {
            copies: Vec::new(),
            page: PAGE_101,
            symbol: SYM,
            pos,
            orient: Orient2::R0,
            rep_type: None,
            box_: Some(spec),
            target: PlaceTarget::NewDevice {
                letter: ident("A"),
                fndef: Some(fndef_two()),
                kind: None,
            },
        })
        .expect("box.black");
    let rep: Id<Representation> = typed(r.outcome.primary.expect("primary"));
    let f = row(e, rep).functions[0];
    (rep, f)
}

fn heading(orient: Orient2, dir: &Dir4) -> Heading {
    let v = match dir {
        Dir4::Up => pt(0, -1),
        Dir4::Right => pt(1, 0),
        Dir4::Down => pt(0, 1),
        Dir4::Left | Dir4::Unknown(_) => pt(-1, 0),
    };
    match orient.apply(v) {
        p if p.y.0 < 0 => Heading::Up,
        p if p.x.0 > 0 => Heading::Right,
        p if p.y.0 > 0 => Heading::Down,
        _ => Heading::Left,
    }
}

/// The pins of a black box on the page, as the Wire tool's targets give them (`pos + orient(cp.pos)`, the editing
/// tables of `ew-api`): `(function CP index, point, leaving heading)`.
fn pins(e: &Engine, rep: Id<Representation>) -> Vec<(u16, Pt2, Heading)> {
    let r = row(e, rep);
    let b = r.box_.as_ref().expect("a black box");
    b.cps
        .iter()
        .map(|c| (c.idx, r.pos + r.orient.apply(c.pos), heading(r.orient, &c.dir)))
        .collect()
}

/// The absolute bounding box of a black box's outline: `(min, max)`.
fn bbox(e: &Engine, rep: Id<Representation>) -> (Pt2, Pt2) {
    let r = row(e, rep);
    let pts: Vec<Pt2> = r
        .box_
        .as_ref()
        .expect("a black box")
        .outline
        .iter()
        .map(|v| r.pos + r.orient.apply(*v))
        .collect();
    let min = pt(
        pts.iter().map(|p| p.x.0).min().expect("an outline"),
        pts.iter().map(|p| p.y.0).min().expect("an outline"),
    );
    let max = pt(
        pts.iter().map(|p| p.x.0).max().expect("an outline"),
        pts.iter().map(|p| p.y.0).max().expect("an outline"),
    );
    (min, max)
}

fn place_two(e: &mut Engine, page: Id<ew_schema::Page>, pos: Pt2) -> (Id<Representation>, Id<Function>) {
    let r = e
        .execute(&PlaceSymbol {
            copies: Vec::new(),
            page,
            symbol: SYM_TWO,
            pos,
            orient: Orient2::R0,
            rep_type: None,
            variant: 0,
            target: PlaceTarget::NewDevice {
                letter: ident("F"),
                fndef: None,
                kind: None,
            },
            connect: vec![],
        })
        .expect("device.place");
    let rep: Id<Representation> = typed(r.outcome.primary.expect("primary"));
    let f = row(e, rep).functions[0];
    (rep, f)
}

/// The route the Wire tool's router gives from `a` to `b` (no obstacles; the search area the ends' box grown by
/// four steps).
fn routed(a: (Pt2, Heading), b: (Pt2, Heading)) -> Vec<Pt2> {
    let grow = 4 * STEP + 4;
    let req = RouteRequest {
        from: RouteEnd {
            at: (a.0.x.0, a.0.y.0),
            dir: Some(a.1),
        },
        to: RouteEnd {
            at: (b.0.x.0, b.0.y.0),
            dir: Some(b.1),
        },
        obstacles: vec![],
        grid: (7_937_500, 3),
        area: [
            a.0.x.0.min(b.0.x.0) - grow,
            a.0.y.0.min(b.0.y.0) - grow,
            a.0.x.0.max(b.0.x.0) + grow,
            a.0.y.0.max(b.0.y.0) + grow,
        ],
        arrive: None,
        own: [vec![], vec![]],
        wires: vec![],
        reserved: vec![],
    };
    auto_route(&req).into_iter().map(|(x, y)| pt(x, y)).collect()
}

/// Draws, for every pin of the black box, a wire from the pin to a new two-CP symbol placed beyond it (above a pin
/// leaving up, below one leaving down, and so on), with the route the Wire tool's router gives; panics with the
/// refusal when one is refused. Returns the connections.
fn wire_every_pin(e: &mut Engine, rep: Id<Representation>, f: Id<Function>) -> Vec<Id<Connection>> {
    let mut out = Vec::new();
    for (idx, pin, h) in pins(e, rep) {
        let (kx, ky) = (grid().snap(pin.x), grid().snap(pin.y));
        // The other end: CP 1 (leaving down) of a symbol above, CP 0 (leaving up) of one below; a pin leaving
        // sideways goes to CP 1 of a symbol beside it and above.
        let (sym_at, other_idx, other_h) = match h {
            Heading::Up => (gp(kx + 2, ky - 12), 1, Heading::Down),
            Heading::Down => (gp(kx + 2, ky + 6), 0, Heading::Up),
            Heading::Left => (gp(kx - 6, ky - 9), 1, Heading::Down),
            Heading::Right => (gp(kx + 6, ky - 9), 1, Heading::Down),
        };
        let (_, other) = place_two(e, PAGE_101, sym_at);
        let other_at = if other_idx == 0 {
            sym_at
        } else {
            sym_at + pt(0, MM10)
        };
        let route = routed((pin, h), (other_at, other_h));
        assert_eq!(route.first(), Some(&pin));
        let r = e
            .execute(&Connect {
                a: EndpointParam::Cp { function: f, idx },
                b: EndpointParam::Cp {
                    function: other,
                    idx: other_idx,
                },
                page: Some(PAGE_101),
                route: route.clone(),
                kind: ConnectionKind::Wire,
            })
            .unwrap_or_else(|err| {
                panic!("the wire to pin {idx} at {pin:?} (route {route:?}) is refused: {err}")
            });
        out.push(typed(r.outcome.primary.expect("primary")));
    }
    out
}

fn steps(rep: Id<Representation>, s: [i64; 2]) -> MoveReps {
    MoveReps {
        items: Vec::new(),
        reps: vec![rep],
        delta: Pt2::ORIGIN,
        page: None,
        steps: Some(s),
    }
}

fn transform(rep: Id<Representation>, rotate: u8, mirror: bool) -> TransformReps {
    TransformReps {
        reps: vec![rep],
        rotate,
        mirror,
        variant: None,
    }
}

/// The reason of a rejection, or a panic naming what came back instead.
fn rejection<T: std::fmt::Debug>(r: Result<T, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// Findings 1 and 2: every pin wires after a drag, a mirror or a turn

#[test]
fn a_black_box_dragged_by_grid_steps_wires_to_every_pin() {
    for s in [[1, 0], [2, 0], [4, 0], [0, 1], [0, 2], [0, 4]] {
        let mut e = engine(golden_model());
        let (pos, spec) = play_box();
        let (rep, f) = place_box(&mut e, pos, spec);
        e.execute(&steps(rep, s)).expect("rep.move by steps");
        assert!(
            pins(&e, rep).iter().any(|(_, p, _)| !on_grid(*p)),
            "steps {s:?}: the move leaves some pin off the grid (the case of the play-test)"
        );
        let wires = wire_every_pin(&mut e, rep, f);
        assert_eq!(wires.len(), pins(&e, rep).len(), "steps {s:?}");
    }
}

#[test]
fn a_mirrored_or_turned_black_box_wires_to_every_pin() {
    // X (mirror left to right), Y (mirror top to bottom), then one to four quarter turns clockwise.
    let cases: Vec<Vec<(u8, bool)>> = vec![
        vec![(0, true)],
        vec![(2, true)],
        vec![(1, false)],
        vec![(1, false), (1, false)],
        vec![(1, false), (1, false), (1, false)],
        vec![(1, false), (1, false), (1, false), (1, false)],
        vec![(2, false)],
        vec![(3, false)],
    ];
    let mut off = 0;
    for case in cases {
        let mut e = engine(golden_model());
        let (pos, spec) = play_box();
        let (rep, f) = place_box(&mut e, pos, spec);
        for (rotate, mirror) in &case {
            e.execute(&transform(rep, *rotate, *mirror))
                .expect("rep.transform");
        }
        off += pins(&e, rep).iter().filter(|(_, p, _)| !on_grid(*p)).count();
        let wires = wire_every_pin(&mut e, rep, f);
        assert_eq!(wires.len(), pins(&e, rep).len(), "{case:?}");
    }
    assert!(
        off > 0,
        "some transform leaves a pin off the grid (the case of the play-test)"
    );
}

#[test]
fn a_black_box_saved_with_a_pin_one_nm_off_the_grid_still_wires() {
    let (dev, f, r): (Id<ew_schema::Device>, Id<Function>, Id<Representation>) =
        (id(0x7001), id(0x7101), id(0x7201));
    let (pos, mut spec) = play_box();
    spec.cps[1].pos.x = Nm(spec.cps[1].pos.x.0 + 1);
    let pin = pos + spec.cps[1].pos;
    assert!(!on_grid(pin));
    let (p, mut records, _) = mini_golden_parts();
    records.push(Record::Device(device(dev, "A", 7)));
    let mut func = function(f, dev, fndef_two(), FnRole::Main);
    func.kind = FunctionKind::BlackBox;
    records.push(Record::Function(func));
    let mut rec = rep(r, PAGE_101, vec![f]);
    rec.pos = pos;
    rec.box_ = Some(spec.clone());
    records.push(Record::Representation(rec));
    let model = Model::from_records(p, records, library()).expect("valid model");
    let mut e = engine(model);
    let before = row(&e, r);
    let wires = wire_every_pin(&mut e, r, f);
    assert_eq!(wires.len(), spec.cps.len());
    let w = row(&e, wires[1]);
    assert_eq!(w.route.first(), Some(&pin), "the wire ends on the stored pin");
    assert_eq!(row(&e, r), before, "nothing rewrites the stored box");
}

#[test]
fn a_route_to_a_black_box_pin_follows_the_box_on_a_drag_and_a_transform() {
    let mut e = engine(golden_model());
    let (pos, spec) = play_box();
    let (rep, f) = place_box(&mut e, pos, spec);
    let wires = wire_every_pin(&mut e, rep, f);
    let check = |e: &Engine, what: &str| {
        for (w, (idx, pin, _)) in wires.iter().zip(pins(e, rep)) {
            let c = row(e, *w);
            assert_eq!(c.a, Endpoint::Cp(f, idx));
            assert_eq!(
                c.route.first(),
                Some(&pin),
                "{what}: the route of pin {idx} follows it"
            );
            let orthogonal = c.route.windows(2).all(|s| s[0].x == s[1].x || s[0].y == s[1].y);
            assert!(orthogonal, "{what}: pin {idx}: {:?}", c.route);
        }
    };
    e.execute(&steps(rep, [1, 0])).expect("a step right");
    check(&e, "a step right");
    e.execute(&steps(rep, [0, -2])).expect("two steps up");
    check(&e, "two steps up");
    e.execute(&MoveReps {
        items: Vec::new(),
        reps: vec![rep],
        delta: pt(G, 0),
        page: None,
        steps: None,
    })
    .expect("a move by a delta");
    check(&e, "a delta");
    e.execute(&transform(rep, 2, true)).expect("mirror top to bottom");
    check(&e, "a mirror");
}

// ---------------------------------------------------------------------------------------------------------
// Findings 3 and 4: a black box mirrors and turns within its bounding box

#[test]
fn a_black_box_mirrors_and_turns_within_its_bounding_box() {
    for (rotate, mirror) in [(0, true), (2, true), (2, false), (1, false), (3, false)] {
        let mut e = engine(golden_model());
        let (pos, spec) = play_box();
        let (rep, _) = place_box(&mut e, pos, spec);
        let (min0, max0) = bbox(&e, rep);
        e.execute(&transform(rep, rotate, mirror)).expect("rep.transform");
        let after = row(&e, rep);
        assert!(on_grid(after.pos), "the insertion point stays on the grid");
        let (min1, max1) = bbox(&e, rep);
        let case = format!("rotate {rotate}, mirror {mirror}");
        if rotate % 2 == 0 {
            // The bounding box stays, to the 1 nm of the rational grid's rounding.
            for (a, b) in [
                (min0.x, min1.x),
                (min0.y, min1.y),
                (max0.x, max1.x),
                (max0.y, max1.y),
            ] {
                assert!(
                    (a.0 - b.0).abs() <= 1,
                    "{case}: {min0:?} {max0:?} became {min1:?} {max1:?}"
                );
            }
        } else {
            // A quarter turn about the bounding box's centre, snapped to the grid: the sides swap exactly and the
            // centre stays within half a step.
            assert_eq!(max1.x.0 - min1.x.0, max0.y.0 - min0.y.0, "{case}");
            assert_eq!(max1.y.0 - min1.y.0, max0.x.0 - min0.x.0, "{case}");
            let twice = |a: Nm, b: Nm| a.0 + b.0;
            assert!(
                (twice(min1.x, max1.x) - twice(min0.x, max0.x)).abs() <= STEP + 1,
                "{case}"
            );
            assert!(
                (twice(min1.y, max1.y) - twice(min0.y, max0.y)).abs() <= STEP + 1,
                "{case}"
            );
        }
    }
}

#[test]
fn a_quarter_turn_snaps_the_centre_to_the_grid_and_four_turns_bring_the_box_back() {
    let mut e = engine(golden_model());
    let (pos, spec) = play_box();
    let (rep, _) = place_box(&mut e, pos, spec);
    let start = bbox(&e, rep);
    // The 6 by 4 steps box turned about its centre (34, 48): the new top-left is the grid point at
    // (34 − 2, 48 − 3) = (32, 45).
    e.execute(&transform(rep, 1, false)).expect("a quarter turn");
    let (min, _) = bbox(&e, rep);
    assert!(
        (min.x.0 - at(32)).abs() <= 1 && (min.y.0 - at(45)).abs() <= 1,
        "{min:?}"
    );
    for _ in 0..3 {
        e.execute(&transform(rep, 1, false)).expect("a quarter turn");
    }
    let end = bbox(&e, rep);
    for (a, b) in [
        (start.0.x, end.0.x),
        (start.0.y, end.0.y),
        (start.1.x, end.1.x),
        (start.1.y, end.1.y),
    ] {
        assert!((a.0 - b.0).abs() <= 1, "{start:?} became {end:?}");
    }
    assert_eq!(row(&e, rep).orient, Orient2::R0);
}

/// The cycle-41 review (finding 1): a box whose width plus height is an odd number of steps (its centre falls between
/// grid points on every quarter turn) comes back to the nanometre after four `rep.transform` quarter turns either way,
/// and after a turn and its reverse; the review's cases, 5 by 4 and 3 by 2 steps at (31, 46), crept toward the origin.
#[test]
fn a_box_with_an_odd_width_plus_height_comes_back_after_four_quarter_turns() {
    for (k1, tops, bottoms) in [
        ((36, 50), &[1, 2, 3, 4][..], &[1, 3][..]),
        ((34, 48), &[1, 2][..], &[1, 2][..]),
    ] {
        for rotate in [1, 3] {
            let mut e = engine(golden_model());
            let (pos, spec) = tool_box((31, 46), k1, tops, bottoms);
            let (rep, _) = place_box(&mut e, pos, spec);
            for _ in 0..4 {
                e.execute(&transform(rep, rotate, false)).expect("a quarter turn");
            }
            let after = row(&e, rep);
            assert_eq!(
                (after.pos, after.orient),
                (pos, Orient2::R0),
                "to {k1:?}, rotate {rotate}"
            );
            e.execute(&transform(rep, rotate, false)).expect("a quarter turn");
            e.execute(&transform(rep, 4 - rotate, false))
                .expect("the reverse");
            let back = row(&e, rep);
            assert_eq!(
                (back.pos, back.orient),
                (pos, Orient2::R0),
                "to {k1:?}, rotate {rotate} and back"
            );
        }
    }
}

#[test]
fn a_symbol_keeps_its_insertion_point_pivot() {
    let mut e = engine(golden_model());
    let (rep, _) = place_two(&mut e, PAGE_101, gp(9, 9));
    for (rotate, mirror) in [(1, false), (0, true), (2, true)] {
        e.execute(&transform(rep, rotate, mirror)).expect("rep.transform");
        assert_eq!(row(&e, rep).pos, gp(9, 9));
    }
}

// ---------------------------------------------------------------------------------------------------------
// The grid refusals in words

#[test]
fn an_off_grid_wire_corner_is_refused_in_words() {
    let mut e = engine(golden_model());
    let (_, fa) = place_two(&mut e, PAGE_102, pt(G, G));
    let (_, fb) = place_two(&mut e, PAGE_102, pt(4 * G, G));
    let reason = rejection(e.execute(&Connect {
        a: EndpointParam::Cp { function: fa, idx: 0 },
        b: EndpointParam::Cp { function: fb, idx: 0 },
        page: Some(PAGE_102),
        route: vec![
            pt(G, G),
            pt(2 * G + 1, G),
            pt(2 * G + 1, 2 * G),
            pt(4 * G, 2 * G),
            pt(4 * G, G),
        ],
        kind: ConnectionKind::Wire,
    }));
    assert_eq!(
        reason,
        "the wire's corner at x 15.9, y 7.9 mm on page 102 is not on the drawing grid"
    );
}

#[test]
fn an_off_grid_position_is_refused_in_words() {
    let mut e = engine(golden_model());
    let reason = rejection(e.execute(&PlaceSymbol {
        copies: Vec::new(),
        page: PAGE_102,
        symbol: SYM_TWO,
        pos: pt(G + 500_000, 2 * G),
        orient: Orient2::R0,
        rep_type: None,
        variant: 0,
        target: PlaceTarget::NewDevice {
            letter: ident("K"),
            fndef: None,
            kind: None,
        },
        connect: vec![],
    }));
    assert_eq!(
        reason,
        "the position at x 8.4, y 15.9 mm on page 102 is not on the drawing grid"
    );
}

#[test]
fn a_step_move_of_an_off_grid_symbol_is_refused_in_words() {
    let (dev, f, r): (Id<ew_schema::Device>, Id<Function>, Id<Representation>) =
        (id(0x7002), id(0x7102), id(0x7202));
    let (p, mut records, _) = mini_golden_parts();
    records.push(Record::Device(device(dev, "K", 7)));
    records.push(Record::Function(function(f, dev, fndef_two(), FnRole::Main)));
    let mut rec = rep(r, PAGE_101, vec![f]);
    rec.symbol = SYM_TWO;
    rec.pos = pt(G + 500_000, 3 * G);
    records.push(Record::Representation(rec));
    let mut e = engine(Model::from_records(p, records, library()).expect("valid model"));
    let reason = rejection(e.execute(&steps(r, [1, 0])));
    assert_eq!(
        reason,
        "the symbol of -101K7 at x 8.4, y 23.8 mm on page 101 is not on the drawing grid, so it cannot be moved by \
         grid steps; move it onto the grid first"
    );
}

#[test]
fn an_off_grid_paste_point_is_refused_in_words() {
    let mut e = engine(golden_model());
    let fragment = json!({"format": "0.1", "project": uuid(0x7300).to_string(), "grid": [7_937_500, 3],
        "page_kind": "multi_line", "anchor": [0, 0],
        "records": [{"$k": "device", "id": uuid(0x7301).to_string(), "tag": {"letter": "F", "counter": 9}}]});
    let err = e
        .execute(&PasteFragment {
            page: PAGE_101,
            at: pt(G + 500_000, 2 * G),
            fragment: serde_json::from_value(fragment).expect("a fragment"),
            adapt: Vec::new(),
        })
        .unwrap_err();
    match err {
        CmdError::InvalidParams { path, message, .. } => {
            assert_eq!(path, "/at");
            assert_eq!(
                message,
                "the paste point at x 8.4, y 15.9 mm on page 101 is not on the drawing grid"
            );
        }
        other => panic!("{other:?}"),
    }
}
