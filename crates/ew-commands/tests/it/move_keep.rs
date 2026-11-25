// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `rep.move` to another page with `keep` (doc 06 §3.4): a wire whose other end stays behind is kept as a pair of
//! interruption points, the source where the wire left the page left, the target at the moved pin; a junction stays
//! with the larger part; a junction joined to another is refused in words; one undo step.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::Origin;
use ew_commands::commands::{KeepAsIps, MoveReps};
use ew_ids::{Id, Uuid};
use ew_model::Model;
use ew_model::fixture::*;
use ew_schema::{
    Connection, Device, Dir4, Endpoint, Extra, FnRole, Function, FunctionKind, IpDirection, Junction,
    LibDocument, Record, Representation, SymbolCp, SymbolDef,
};
use ew_units::Pt2;

/// An item whose FnDef has one CP, the interruption point's.
const ITEM_IP: Uuid = Uuid::from_u128(0x9311);
/// An interruption-point symbol: one CP at its insertion point.
const SYM_IP: Id<SymbolDef> = id(0x84);

const DEVS: [Id<Device>; 4] = [id(0x1a0), id(0x1a1), id(0x1a2), id(0x1a3)];
const FNS: [Id<Function>; 4] = [id(0x2a0), id(0x2a1), id(0x2a2), id(0x2a3)];
const REPS: [Id<Representation>; 4] = [id(0x3a0), id(0x3a1), id(0x3a2), id(0x3a3)];
const J1: Id<Junction> = id(0x5a0);
const J2: Id<Junction> = id(0x5a1);

/// The symbols A, B, C, D on page 102: their CP 0 at (8G, 4G), (2G, 4G), (2G, 8G) and (8G, 8G).
const AT: [(i64, i64); 4] = [(8, 4), (2, 4), (2, 8), (8, 8)];

fn at(i: usize) -> Pt2 {
    pt(AT[i].0 * G, AT[i].1 * G)
}

/// The library of these tests: the mini project's, the two-CP symbol and the interruption-point symbol.
fn lib() -> Vec<LibDocument> {
    let mut lib = library();
    lib.push(LibDocument::Fndef(fndef_copy_for(LIBRARY_X, ITEM_IP, &["1"])));
    let mut ip = symbol_using(SYM_IP, LIBRARY_X, ITEM_IP);
    ip.item.cps = ew_schema::KeyedList::from_vec(vec![SymbolCp {
        idx: 0,
        fn_cp: 0,
        pos: pt(0, 0),
        dir: Dir4::Left,
        discipline: Default::default(),
        extra: Extra::new(),
    }])
    .expect("one CP");
    lib.push(LibDocument::Symbol(ip));
    lib
}

/// The mini project with the symbols A to D on page 102, wired by `wires` (each `(id, a, b, route)`), with the
/// junctions `junctions` (each `(id, at, wires)`).
fn model(
    wires: &[(u128, Endpoint, Endpoint, Vec<Pt2>)],
    junctions: &[(Id<Junction>, Pt2, Vec<u128>)],
) -> Model {
    let (project, mut records, _) = mini_project_parts();
    for i in 0..4 {
        records.push(Record::Device(device(DEVS[i], "K", 71 + i as u32)));
        records.push(Record::Function(function(
            FNS[i],
            DEVS[i],
            fndef_two(),
            FnRole::Main,
        )));
        let mut r = rep(REPS[i], PAGE_102, vec![FNS[i]]);
        r.symbol = SYM_TWO;
        r.pos = at(i);
        records.push(Record::Representation(r));
    }
    for (n, a, b, route) in wires {
        let mut c = connection(id(*n), Some(PAGE_102), a.clone(), b.clone());
        c.route = route.clone();
        records.push(Record::Connection(c));
    }
    for (j, p, seq) in junctions {
        let mut jr = junction(*j, PAGE_102, seq.iter().map(|n| id(*n)).collect());
        jr.at = *p;
        records.push(Record::Junction(jr));
    }
    Model::from_records(project, records, lib()).expect("valid model")
}

fn keep() -> KeepAsIps {
    KeepAsIps {
        source: SYM_IP,
        target: SYM_IP,
        copies: Vec::new(),
    }
}

/// `rep.move` of the symbols `which` to page 101 with `keep`.
fn move_to_101(which: &[usize]) -> MoveReps {
    MoveReps {
        reps: which.iter().map(|i| REPS[*i]).collect(),
        delta: pt(0, 0),
        page: Some(PAGE_101),
        steps: None,
        items: Vec::new(),
        routes: Vec::new(),
        connect: Vec::new(),
        into: None,
        keep: Some(keep()),
    }
}

/// The interruption points the move made: (name, direction, page, the point of its CP), the source first.
fn ips(e: &ew_commands::Engine) -> Vec<(String, IpDirection, Id<ew_schema::Page>, Pt2)> {
    let mut out = Vec::new();
    for f in rows::<Function>(e) {
        let FunctionKind::Ip(spec) = &f.kind else { continue };
        if [FN_IP_101, FN_IP_102].contains(&f.id) {
            continue;
        }
        let r = rows::<Representation>(e)
            .into_iter()
            .find(|r| r.functions == vec![f.id])
            .expect("its representation");
        out.push((spec.name.to_string(), spec.direction.clone(), r.page, r.pos));
    }
    out.sort_by_key(|(_, d, _, _)| *d != IpDirection::Source);
    out
}

/// The wire whose ends are `x` and `y`, in either order.
fn wire_between(e: &ew_commands::Engine, x: &Endpoint, y: &Endpoint) -> Option<Connection> {
    rows::<Connection>(e)
        .into_iter()
        .find(|c| (c.a == *x && c.b == *y) || (c.a == *y && c.b == *x))
}

/// The CP of the point of `direction` the move made.
fn ip_end(e: &ew_commands::Engine, direction: IpDirection) -> Endpoint {
    let f = rows::<Function>(e)
        .into_iter()
        .filter(|f| ![FN_IP_101, FN_IP_102].contains(&f.id))
        .find(|f| matches!(&f.kind, FunctionKind::Ip(s) if s.direction == direction))
        .expect("the point");
    Endpoint::Cp(f.id, 0)
}

/// A plain wire: the source point stands where the wire left page 102, the wire shortened to it; the target point
/// stands on page 101 at the moved pin, joined to it by a wire of length zero; one name for the pair, said in the
/// move's notice; one undo step.
#[test]
fn a_wire_to_a_symbol_that_stays_becomes_a_pair_of_interruption_points() {
    let w = 0x4a0;
    let mut e = engine(model(
        &[(w, cp(FNS[0], 0), cp(FNS[1], 0), vec![at(0), at(1)])],
        &[],
    ));
    let before = lines(e.model());
    let r = e.execute(&move_to_101(&[0])).expect("the move");
    let pair = ips(&e);
    assert_eq!(pair.len(), 2, "{pair:?}");
    let (source, target) = (&pair[0], &pair[1]);
    assert_eq!((&source.1, source.2), (&IpDirection::Source, PAGE_102));
    assert_eq!((&target.1, target.2), (&IpDirection::Target, PAGE_101));
    assert_eq!(source.0, target.0, "one name for the pair");
    assert_eq!(source.3, at(0), "the source where the wire left the page");
    assert_eq!(target.3, at(0), "the target at the moved pin");
    let kept = row::<Connection>(&e, id(w));
    assert_eq!(kept.page, Some(PAGE_102));
    assert_eq!(
        (kept.a.clone(), kept.b.clone()),
        (ip_end(&e, IpDirection::Source), cp(FNS[1], 0))
    );
    assert_eq!(kept.route, vec![at(0), at(1)]);
    let joined =
        wire_between(&e, &ip_end(&e, IpDirection::Target), &cp(FNS[0], 0)).expect("the joining wire");
    assert_eq!((joined.page, joined.route.len()), (Some(PAGE_101), 0));
    assert!(
        r.outcome.notices.iter().any(|n| n.to_string().ends_with(&format!(
            "is kept as the pair of interruption points {}",
            source.0
        ))),
        "{:?}",
        r.outcome.notices
    );
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(lines(e.model()), before, "one undo step");
}

/// A junction with more wires to symbols that stay stays; the wire from it to the moved symbol becomes the pair.
#[test]
fn a_junction_stays_with_the_symbols_that_stay() {
    let j = pt(5 * G, 4 * G);
    let wires = [
        (0x4a1, cp(FNS[0], 0), at_junction(J1), vec![at(0), j]),
        (0x4a2, at_junction(J1), cp(FNS[1], 0), vec![j, at(1)]),
        (
            0x4a3,
            at_junction(J1),
            cp(FNS[2], 0),
            vec![j, pt(5 * G, 8 * G), at(2)],
        ),
    ];
    let mut e = engine(model(&wires, &[(J1, j, vec![0x4a1, 0x4a2, 0x4a3])]));
    e.execute(&move_to_101(&[0])).expect("the move");
    assert_eq!(row::<Junction>(&e, J1).page, PAGE_102, "the junction stays");
    let pair = ips(&e);
    assert_eq!(pair.len(), 2, "{pair:?}");
    assert_eq!(pair[0].3, at(0), "the source where the wire left");
    let kept = row::<Connection>(&e, id(0x4a1));
    assert_eq!((kept.page, kept.b.clone()), (Some(PAGE_102), at_junction(J1)));
}

/// A junction with more wires to moved symbols goes with them, those wires too; the wire from it to the symbol that
/// stays becomes the pair, its target point at the junction on the new page.
#[test]
fn a_junction_goes_with_the_moved_symbols_when_they_are_more() {
    let j = pt(5 * G, 4 * G);
    let wires = [
        (0x4a1, cp(FNS[0], 0), at_junction(J1), vec![at(0), j]),
        (0x4a2, at_junction(J1), cp(FNS[1], 0), vec![j, at(1)]),
        (
            0x4a3,
            at_junction(J1),
            cp(FNS[2], 0),
            vec![j, pt(5 * G, 8 * G), at(2)],
        ),
    ];
    let mut e = engine(model(&wires, &[(J1, j, vec![0x4a1, 0x4a2, 0x4a3])]));
    e.execute(&move_to_101(&[0, 1])).expect("the move");
    let junction = row::<Junction>(&e, J1);
    assert_eq!(junction.page, PAGE_101, "the junction goes with the larger part");
    for w in [0x4a1, 0x4a2] {
        assert_eq!(
            row::<Connection>(&e, id(w)).page,
            Some(PAGE_101),
            "its wires to the moved symbols"
        );
    }
    let pair = ips(&e);
    assert_eq!(pair.len(), 2, "{pair:?}");
    assert_eq!(
        (pair[0].2, pair[0].3),
        (PAGE_102, j),
        "the source where the junction stood"
    );
    assert_eq!(
        (pair[1].2, pair[1].3),
        (PAGE_101, j),
        "the target at the junction"
    );
    let kept = row::<Connection>(&e, id(0x4a3));
    assert_eq!(kept.page, Some(PAGE_102));
    assert_eq!(
        kept.route.first(),
        Some(&j),
        "shortened to the source point: {:?}",
        kept.route
    );
    let joined =
        wire_between(&e, &ip_end(&e, IpDirection::Target), &at_junction(J1)).expect("the joining wire");
    assert!(
        junction.sequence.contains(&joined.id),
        "the junction's wire order names the new wire"
    );
    assert!(!junction.sequence.contains(&id(0x4a3)));
}

/// A wire through two junctions is refused in words, and nothing is written.
#[test]
fn a_net_with_two_junctions_is_refused_in_words() {
    let (j1, j2) = (pt(5 * G, 4 * G), pt(5 * G, 8 * G));
    let wires = [
        (0x4a1, cp(FNS[0], 0), at_junction(J1), vec![at(0), j1]),
        (0x4a2, at_junction(J1), cp(FNS[1], 0), vec![j1, at(1)]),
        (0x4a3, at_junction(J1), at_junction(J2), vec![j1, j2]),
        (0x4a4, at_junction(J2), cp(FNS[2], 0), vec![j2, at(2)]),
        (0x4a5, at_junction(J2), cp(FNS[3], 0), vec![j2, at(3)]),
    ];
    let mut e = engine(model(
        &wires,
        &[
            (J1, j1, vec![0x4a1, 0x4a2, 0x4a3]),
            (J2, j2, vec![0x4a3, 0x4a4, 0x4a5]),
        ],
    ));
    let rev = e.model().rev();
    let why = reason(e.execute(&move_to_101(&[0])));
    assert!(why.contains("runs through more than one junction"), "{why}");
    assert_eq!(e.model().rev(), rev, "nothing is written");
}
