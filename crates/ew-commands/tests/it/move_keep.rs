// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `rep.move` to another page with `keep` (doc 06 §3.4): a wire whose other end stays behind is kept as a pair of
//! interruption points, the source where the wire left the page left, the target at the moved pin; a junction stays
//! with the larger part; a pair's name taken goes on in its series; a junction joined to another, and a wire with no
//! free grid point on its path, are refused in words; one undo step.
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
    LibCopy, LibDocument, Record, Representation, SymbolCp, SymbolDef,
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

/// An interruption-point symbol `symbol` drawing the FnDef `item` of `library`: one CP at its insertion point.
fn ip_symbol(symbol: Id<SymbolDef>, library: Uuid, item: Uuid) -> LibCopy<SymbolDef> {
    let mut ip = symbol_using(symbol, library, item);
    ip.item.cps = ew_schema::KeyedList::from_vec(vec![SymbolCp {
        idx: 0,
        fn_cp: 0,
        pos: pt(0, 0),
        dir: Dir4::Left,
        discipline: Default::default(),
        extra: Extra::new(),
    }])
    .expect("one CP");
    ip
}

/// The library of these tests: the mini project's, the two-CP symbol and the interruption-point symbol.
fn lib() -> Vec<LibDocument> {
    let mut lib = library();
    lib.push(LibDocument::Fndef(fndef_copy_for(LIBRARY_X, ITEM_IP, &["1"])));
    lib.push(LibDocument::Symbol(ip_symbol(SYM_IP, LIBRARY_X, ITEM_IP)));
    lib
}

/// The library of interruption-point documents the project does not hold.
const LIBRARY_NEW: Uuid = Uuid::from_u128(0x9320_0000);

/// `doc` as the document of `item` of [`LIBRARY_NEW`] (`lib/` JSON form: its copy identity as its ID and the hash of
/// its item), with its copy ID.
fn new_document(doc: LibDocument, item: Uuid) -> (Uuid, serde_json::Value) {
    let identity: Id<()> = ew_ids::v5::lib_copy_id(LIBRARY_NEW, item);
    let mut v = serde_json::to_value(&doc).expect("a library document");
    v["library_id"] = serde_json::json!(LIBRARY_NEW.to_string());
    v["item_id"] = serde_json::json!(item.to_string());
    v["id"] = serde_json::json!(identity.to_string());
    let hash = ew_format::library_copy_hash(&LibDocument::from_value(v.clone()).expect("a document"))
        .expect("a known kind");
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    v["sha256"] = serde_json::json!(format!("sha256:{hex}"));
    (identity.as_uuid(), v)
}

/// `keep` with an interruption-point symbol and its FnDef the project does not hold, as their documents.
fn keep_new() -> KeepAsIps {
    let (fn_item, sym_item) = (Uuid::from_u128(0x9320_0001), Uuid::from_u128(0x9320_0002));
    let (_, fndef) = new_document(
        LibDocument::Fndef(fndef_copy_for(LIBRARY_NEW, fn_item, &["1"])),
        fn_item,
    );
    let (symbol, doc) = new_document(
        LibDocument::Symbol(ip_symbol(id(0x85), LIBRARY_NEW, fn_item)),
        sym_item,
    );
    KeepAsIps {
        source: Id::from_uuid(symbol),
        target: Id::from_uuid(symbol),
        copies: vec![fndef, doc],
    }
}

/// The mini project with the symbols A to D on page 102, wired by `wires` (each `(id, a, b, route)`), with the
/// junctions `junctions` (each `(id, at, wires)`).
fn model(
    wires: &[(u128, Endpoint, Endpoint, Vec<Pt2>)],
    junctions: &[(Id<Junction>, Pt2, Vec<u128>)],
) -> Model {
    model_with(wires, junctions, |_| {})
}

/// [`model`] with its records changed by `edit` before the model is made.
fn model_with(
    wires: &[(u128, Endpoint, Endpoint, Vec<Pt2>)],
    junctions: &[(Id<Junction>, Pt2, Vec<u128>)],
    edit: impl FnOnce(&mut Vec<Record>),
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
    edit(&mut records);
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

/// The interruption point's symbol and FnDef copies `keep` brings are embedded only when the move cuts a wire: a move
/// that cuts none leaves the project's library copies as they were; one that cuts a wire embeds both and draws its
/// pair with them.
#[test]
fn the_points_documents_are_embedded_only_when_the_move_cuts_a_wire() {
    let held = |e: &ew_commands::Engine| {
        (
            e.model().table::<LibCopy<SymbolDef>>().iter().count(),
            e.model().table::<LibCopy<ew_schema::FnDef>>().iter().count(),
        )
    };
    let keep = keep_new();
    let mut e = engine(model(&[], &[]));
    let before = held(&e);
    e.execute(&MoveReps {
        keep: Some(keep.clone()),
        ..move_to_101(&[0])
    })
    .expect("the move");
    assert_eq!(
        row::<Representation>(&e, REPS[0]).page,
        PAGE_101,
        "the symbol moves"
    );
    assert_eq!(held(&e), before, "no library copy is added");
    let mut e = engine(model(
        &[(0x4a0, cp(FNS[0], 0), cp(FNS[1], 0), vec![at(0), at(1)])],
        &[],
    ));
    e.execute(&MoveReps {
        keep: Some(keep.clone()),
        ..move_to_101(&[0])
    })
    .expect("the move");
    assert_eq!(
        held(&e),
        (before.0 + 1, before.1 + 1),
        "the point's symbol and FnDef"
    );
    let drawn = rows::<Representation>(&e)
        .into_iter()
        .filter(|r| r.symbol == keep.source)
        .count();
    assert_eq!(drawn, 2, "the pair is drawn with the new symbol");
}

/// The name of the pair of the plain wire from A to B, the one move of A makes on the mini project.
fn plain_pair_name() -> String {
    let mut e = engine(model(
        &[(0x4a0, cp(FNS[0], 0), cp(FNS[1], 0), vec![at(0), at(1)])],
        &[],
    ));
    e.execute(&move_to_101(&[0])).expect("the move");
    ips(&e)[0].0.clone()
}

/// A pair whose name an interruption point of the project has already takes the next name of its series.
#[test]
fn a_pair_whose_name_is_taken_goes_on_in_its_series() {
    let name = plain_pair_name();
    let mut e = engine(model_with(
        &[(0x4a0, cp(FNS[0], 0), cp(FNS[1], 0), vec![at(0), at(1)])],
        &[],
        |records| {
            for r in records.iter_mut() {
                if let Record::Function(f) = r
                    && [FN_IP_101, FN_IP_102].contains(&f.id)
                    && let FunctionKind::Ip(spec) = &mut f.kind
                {
                    spec.name = name.parse().expect("an identifier");
                }
            }
        },
    ));
    e.execute(&move_to_101(&[0])).expect("the move");
    let pair = ips(&e);
    let next = ew_commands::commands::next_in_series(&name);
    assert_eq!(
        pair.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
        [next.as_str(), next.as_str()],
        "{name} is taken"
    );
}

/// Two wires of one potential cut by one move make two pairs: the second takes the next name of the series, and its
/// source point the next free grid point of its path, the first pair's standing at the moved pin's old place.
#[test]
fn two_pairs_of_one_potential_in_one_move_take_two_names_and_two_places() {
    let name = plain_pair_name();
    let mut e = engine(model(
        &[
            (0x4a0, cp(FNS[0], 0), cp(FNS[1], 0), vec![at(0), at(1)]),
            (
                0x4a1,
                cp(FNS[0], 0),
                cp(FNS[2], 0),
                vec![at(0), pt(8 * G, 6 * G), pt(2 * G, 6 * G), at(2)],
            ),
        ],
        &[],
    ));
    let r = e.execute(&move_to_101(&[0])).expect("the move");
    let next = ew_commands::commands::next_in_series(&name);
    let pairs = ips(&e);
    assert_eq!(pairs.len(), 4, "{pairs:?}");
    // One step of the page's grid down the second wire's first segment.
    let grid = row::<ew_schema::Page>(&e, PAGE_102).grid;
    let below = grid.at(grid.snap(at(0).y) + 1).expect("a grid point");
    for (n, source) in [(&name, at(0)), (&next, Pt2::new(at(0).x, below))] {
        let of: Vec<_> = pairs.iter().filter(|p| &p.0 == n).collect();
        assert_eq!(of.len(), 2, "{n}: {pairs:?}");
        assert!(
            of.iter()
                .any(|p| (&p.1, p.2, p.3) == (&IpDirection::Source, PAGE_102, source)),
            "{n}: {pairs:?}"
        );
        assert!(
            of.iter()
                .any(|p| (&p.1, p.2, p.3) == (&IpDirection::Target, PAGE_101, at(0))),
            "{n}: {pairs:?}"
        );
    }
    assert_eq!(r.outcome.notices.len(), 2, "{:?}", r.outcome.notices);
}

/// A wire whose old path has no free grid point left is refused in words, and nothing is written: the unrouted wire
/// from A to C runs on no grid line, so its only point is A's pin, where the pin of B stands that stays.
#[test]
fn a_wire_without_a_free_grid_point_on_its_path_is_refused_in_words() {
    let mut e = engine(model_with(
        &[(0x4a0, cp(FNS[0], 0), cp(FNS[2], 0), Vec::new())],
        &[],
        |records| {
            for r in records.iter_mut() {
                if let Record::Representation(rep) = r
                    && rep.id == REPS[1]
                {
                    rep.pos = at(0);
                }
            }
        },
    ));
    let rev = e.model().rev();
    let why = reason(e.execute(&move_to_101(&[0])));
    assert!(why.contains("has no free grid point left on its path"), "{why}");
    assert_eq!(e.model().rev(), rev, "nothing is written");
}

/// A wire kept as a pair of interruption points is one wire still (doc 13 §2.3): it keeps its typed length, its colour
/// and each end set by hand at its clamp, whichever of its ends moves, and each end is held once (the joining wire on
/// the new page takes no copy of the wire's values).
#[test]
fn a_wire_kept_as_a_pair_keeps_its_values_each_end_once() {
    let w = 0x4a0;
    let (x, y) = (cp(FNS[0], 0), cp(FNS[1], 0));
    for moving in [0, 1] {
        let plain = model(&[(w, x.clone(), y.clone(), vec![at(0), at(1)])], &[]);
        let mut e = engine(with_wire_values(
            &plain,
            (&x, &y),
            400,
            Some("BU"),
            [Some("bare"), Some("ring_lug")],
        ));
        e.execute(&move_to_101(&[moving])).expect("the move");
        assert_eq!(
            wire_seen(e.model(), &x, &y),
            Some((
                Some(400),
                Some("[\"BU\"]".into()),
                [Some("bare".into()), Some("ring_lug".into())]
            )),
            "symbol {moving} moved"
        );
        let held: usize = rows::<Connection>(&e)
            .iter()
            .map(|c| usize::from(c.overrides.end_a.is_some()) + usize::from(c.overrides.end_b.is_some()))
            .sum();
        assert_eq!(held, 2, "symbol {moving} moved: each end held once");
    }
}
