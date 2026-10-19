// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! Connectivity derivations (WP-S1.07): page-local nets, stitch graph, potential partition, hostile shapes.
//!
//! - `local_net_joins_wires_and_junction_arms`: three CPs and a three-arm junction are one local net.
//! - `overview_and_bridge_connections_do_not_join_nets`: only a bridge joins, and only at potential level.
//! - `bond_via_is_cut`: the connection named as `Bond.via` is cut from the nets and listed in `cut`.
//! - `ip_pair_joins_across_pages`: an interruption-point pair joins two pages; aliases follow the definition.
//! - `unpaired_ip_is_noted_not_panicking`: a lonely interruption point is noted (EW-CORE-051).
//! - `self_loop_connection_is_harmless`: a connection from a CP to itself.
//! - `junction_cycle_is_finite`: a junction cycle terminates, with and without a CP.
//! - `dangling_endpoint_is_harmless`: a connection to a function that does not exist.
//! - `potential_id_prefers_definitions`: definition ID, EW-POT-017, else the minimum CP ID.
//! - `derived_values_do_not_depend_on_record_order`: partition and stitch graph under record permutations.
//! - `no_hash_containers_in_connectivity_sources`: no `HashMap`/`HashSet` in the connectivity sources.
//! - `queries_survive_a_device_parent_cycle`: the partition with rail-bonded terminals in a parent cycle.

mod common;

use common::{Fx, id, uid};
use ew_derive::{
    CpRef, DeriveNote, EdgeKind, Level, NetId, PageLocalNets, PotentialId, PotentialPartition, Q,
    StitchGraphQ,
};
use ew_ids::Id;
use ew_model::{AnyRow, EndpointKey, Entity, Model, Op, Rev};
use ew_schema::{Connection, Device, Endpoint, Junction, Page, PotentialDef, TagScheme};
use serde_json::{Value, json};

const FNDEF: u128 = 0x500;
const FNDEF_IP: u128 = 0x501;
const SYM: u128 = 0x510;
const SYM_IP: u128 = 0x511;
const P1: u128 = 0x100;
const P2: u128 = 0x101;

/// An endpoint of a connection.
#[derive(Clone, Copy)]
enum Ep {
    Cp(u128, u16),
    J(u128),
}

impl Ep {
    fn json(self) -> Value {
        match self {
            Ep::Cp(f, i) => json!({"cp": [uid(f), i]}),
            Ep::J(j) => json!({"junction": uid(j)}),
        }
    }

    fn key(self) -> EndpointKey {
        match self {
            Ep::Cp(f, i) => EndpointKey::Cp(id(f), i),
            Ep::J(j) => EndpointKey::Junction(id(j)),
        }
    }
}

fn cp(f: u128) -> Ep {
    Ep::Cp(f, 0)
}

fn cpref(f: u128) -> CpRef {
    CpRef::new(id(f), 0)
}

/// Two pages, a two-CP function definition with a symbol, and an interruption-point definition.
fn base() -> Fx {
    let mut fx = Fx::new();
    fx.fndef(FNDEF, ["Misc", "Misc", "Plain"], &[(0, "1"), (1, "2")]);
    fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)]);
    fx.fndef(FNDEF_IP, ["Misc", "Misc", "Ip"], &[(0, "1")]);
    fx.symbol(SYM_IP, FNDEF_IP, json!("origin"), &[(0, 0, 0, 0)]);
    fx.page(P1, 1, None, "multi_line", &[], "one");
    fx.page(P2, 2, None, "multi_line", &[], "two");
    fx
}

/// A plain function `n` (no device) drawn on `page`.
fn func(fx: &mut Fx, n: u128, page: u128) {
    fx.device(n + 0x20_000, "K", Some(n as u32));
    fx.function(n, Some(n + 0x20_000), FNDEF, true);
    fx.rep(n + 0x10_000, page, SYM, &[n], 0, n as i64 * 1_000_000);
}

/// An interruption point `n` named `name` drawn on `page`.
fn ip(fx: &mut Fx, n: u128, page: u128, name: &str, dir: &str) {
    fx.push(json!({
        "$k": "function", "id": uid(n), "def": uid(FNDEF_IP),
        "kind": {"ip": {"name": name, "direction": dir}},
    }));
    fx.rep(n + 0x10_000, page, SYM_IP, &[n], 0, n as i64 * 1_000_000);
}

/// A connection `n` of `kind` (`Value::Null` for a wire) between `a` and `b`, on `page` if given.
fn conn(fx: &mut Fx, n: u128, page: Option<u128>, a: Ep, b: Ep, kind: Value) {
    let mut r = json!({"$k": "connection", "id": uid(n), "a": a.json(), "b": b.json()});
    if let Some(p) = page {
        r["page"] = json!(uid(p));
    }
    if !kind.is_null() {
        r["kind"] = kind;
    }
    fx.push(r);
}

/// A dot junction `n` on `page` with `sequence`.
fn junction(fx: &mut Fx, n: u128, page: u128, sequence: &[u128]) {
    let seq: Vec<String> = sequence.iter().map(|c| uid(*c)).collect();
    fx.push(json!({
        "$k": "junction", "id": uid(n), "page": uid(page), "at": [0, 0], "style": "dot", "sequence": seq,
    }));
}

fn def(fx: &mut Fx, n: u128, at: Ep, name: &str) {
    fx.push(json!({"$k": "potential_def", "id": uid(n), "at": at.json(), "name": name}));
}

fn bridge() -> Value {
    json!({"bridge": {"kind": "plug_in", "group": 1, "shaft": 1}})
}

fn has_note(q: &mut Q, code: &str) -> bool {
    q.diagnostics()
        .notes()
        .iter()
        .any(|n| matches!(n, DeriveNote::Connectivity { code: c, .. } if *c == code))
}

#[test]
fn local_net_joins_wires_and_junction_arms() {
    let mut fx = base();
    for f in [0x1001, 0x1002, 0x1003] {
        func(&mut fx, f, P1);
    }
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), Ep::J(0x3001), Value::Null);
    conn(&mut fx, 0x2002, Some(P1), Ep::J(0x3001), cp(0x1002), Value::Null);
    conn(&mut fx, 0x2003, Some(P1), Ep::J(0x3001), cp(0x1003), Value::Null);
    junction(&mut fx, 0x3001, P1, &[0x2001, 0x2002, 0x2003]);
    let m = fx.build();
    let q = Q::new(&m);
    let local = q.get::<PageLocalNets>(id::<Page>(P1));
    assert_eq!(local.nets.len(), 1);
    let net = &local.nets[0];
    let cps: Vec<CpRef> = net.cps().collect();
    assert_eq!(cps.len(), 3);
    for f in [0x1001, 0x1002, 0x1003] {
        assert!(cps.contains(&cpref(f)));
    }
    assert_eq!(net.junctions().collect::<Vec<Id<Junction>>>(), vec![id(0x3001)]);
    let min = [0x1001, 0x1002, 0x1003]
        .iter()
        .map(|f| cpref(*f).id())
        .min()
        .unwrap();
    assert_eq!(net.id, NetId(min));
    let via = |at: EndpointKey| -> Vec<Id<Connection>> {
        net.members
            .iter()
            .find(|m| m.at == at)
            .expect("member")
            .via
            .clone()
    };
    assert_eq!(via(cp(0x1001).key()), vec![id(0x2001)]);
    assert_eq!(via(cp(0x1002).key()), vec![id(0x2002)]);
    assert_eq!(via(Ep::J(0x3001).key()), vec![id(0x2001), id(0x2002), id(0x2003)]);
}

#[test]
fn overview_and_bridge_connections_do_not_join_nets() {
    let mut fx = base();
    for f in [0x1001, 0x1002, 0x1003, 0x1004] {
        func(&mut fx, f, P1);
    }
    conn(
        &mut fx,
        0x2001,
        Some(P1),
        cp(0x1001),
        cp(0x1002),
        json!("overview"),
    );
    conn(&mut fx, 0x2002, Some(P1), cp(0x1003), cp(0x1004), bridge());
    let m = fx.build();
    let q = Q::new(&m);
    let local = q.get::<PageLocalNets>(id::<Page>(P1));
    assert!(local.nets.is_empty(), "{local:?}");
    for f in [0x1001, 0x1002, 0x1003, 0x1004] {
        assert!(local.net_of(cp(f).key()).is_none());
    }
    let g = q.get::<StitchGraphQ>(());
    let bridges: Vec<_> = g
        .edges
        .iter()
        .filter(|e| matches!(e.kind, EdgeKind::Bridge(_)))
        .collect();
    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].level, Level::Potential);
    assert_eq!(bridges[0].kind, EdgeKind::Bridge(id(0x2002)));
    let part = q.get::<PotentialPartition>(());
    let p3 = part.of(&g, cp(0x1003).key()).expect("potential");
    let p4 = part.of(&g, cp(0x1004).key()).expect("potential");
    assert_eq!(p3.id, p4.id);
    assert!(p3.cps.contains(&cpref(0x1003)) && p3.cps.contains(&cpref(0x1004)));
    // The overview connection joins nothing at any level.
    assert!(part.of(&g, cp(0x1001).key()).is_none_or(|p| p.id != p3.id));
}

#[test]
fn bond_via_is_cut() {
    let mut fx = base();
    for f in [0x1001, 0x1002, 0x1003, 0x1004] {
        func(&mut fx, f, P1);
    }
    // W joins A and B; stubs give each side a net of its own.
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1002), Value::Null);
    conn(&mut fx, 0x2002, Some(P1), cp(0x1001), cp(0x1003), Value::Null);
    conn(&mut fx, 0x2003, Some(P1), cp(0x1002), cp(0x1004), Value::Null);
    fx.push(json!({
        "$k": "bond", "id": uid(0x4001), "a": cp(0x1001).json(), "b": cp(0x1002).json(),
        "kind": "equipotential", "via": uid(0x2001),
    }));
    let m = fx.build();
    let q = Q::new(&m);
    let local = q.get::<PageLocalNets>(id::<Page>(P1));
    assert_eq!(local.cut, vec![id::<Connection>(0x2001)]);
    assert_eq!(local.nets.len(), 2);
    let na = local.net_of(cp(0x1001).key()).expect("net");
    let nb = local.net_of(cp(0x1002).key()).expect("net");
    assert_ne!(na.id, nb.id);
    assert!(!na.connections.contains(&id(0x2001)) && !nb.connections.contains(&id(0x2001)));
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potentials.len(), 2);
    assert_eq!(part.bonds.len(), 1);
    assert!(part.bonds[0].a.is_some() && part.bonds[0].b.is_some());
    assert_ne!(part.bonds[0].a, part.bonds[0].b);
}

/// A on page 1 -- IP X (source) .. IP X (target) -- B on page 2, with a definition named `def_name` at A.
fn ip_pair(def_name: &str) -> Model {
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    func(&mut fx, 0x1002, P2);
    ip(&mut fx, 0x1101, P1, "X", "source");
    ip(&mut fx, 0x1102, P2, "X", "target");
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1101), Value::Null);
    conn(&mut fx, 0x2002, Some(P2), cp(0x1102), cp(0x1002), Value::Null);
    def(&mut fx, 0x5001, cp(0x1001), def_name);
    fx.build()
}

#[test]
fn ip_pair_joins_across_pages() {
    for (def_name, aliases) in [("X", Vec::<String>::new()), ("Y", vec!["X".to_owned()])] {
        let m = ip_pair(def_name);
        let q = Q::new(&m);
        let g = q.get::<StitchGraphQ>(());
        assert!(g.edges.iter().any(|e| matches!(e.kind, EdgeKind::IpPair { .. })));
        let part = q.get::<PotentialPartition>(());
        assert_eq!(part.potentials.len(), 1, "{part:?}");
        let p = &part.potentials[0];
        assert!(p.cps.contains(&cpref(0x1001)) && p.cps.contains(&cpref(0x1002)));
        assert_eq!(p.name.text, def_name);
        assert_eq!(p.aliases, aliases);
        assert_eq!(p.id, PotentialId::Def(id::<PotentialDef>(0x5001)));
    }
}

#[test]
fn unpaired_ip_is_noted_not_panicking() {
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    ip(&mut fx, 0x1101, P1, "LONELY", "source");
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1101), Value::Null);
    let m = fx.build();
    let mut q = Q::new(&m);
    let _ = q.get::<PotentialPartition>(());
    assert!(has_note(&mut q, "EW-CORE-051"), "{:?}", q.diagnostics().notes());
}

/// `m` with connection `n`'s end `b` replaced, invariants skipped (CI-01, EW-INV-001).
fn with_end_b(m: &Model, n: u128, b: Ep) -> Model {
    let conn_id: Id<Connection> = id(n);
    let before = m.row(&Connection::key(conn_id)).expect("connection exists");
    let mut after = (**before.get::<Connection>().expect("a connection")).clone();
    after.b = match b {
        Ep::Cp(f, i) => Endpoint::Cp(id(f), i),
        Ep::J(j) => Endpoint::Junction(id(j)),
    };
    let m2 = m
        .apply_unchecked(&[Op::update(before, AnyRow::of(after))], Rev(1))
        .expect("op applies");
    assert!(m2.integrity_check().is_err(), "the probe violates an invariant");
    m2
}

/// Two functions joined by a wire `0x2001`.
fn two_functions() -> Fx {
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    func(&mut fx, 0x1002, P1);
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1002), Value::Null);
    fx
}

#[test]
fn self_loop_connection_is_harmless() {
    let m = with_end_b(&two_functions().build(), 0x2001, cp(0x1001));
    let q = Q::new(&m);
    let local = q.get::<PageLocalNets>(id::<Page>(P1));
    assert_eq!(local.nets.len(), 1);
    assert_eq!(local.nets[0].cps().collect::<Vec<_>>(), vec![cpref(0x1001)]);
    let _ = q.get::<StitchGraphQ>(());
    let _ = q.get::<PotentialPartition>(());
}

#[test]
fn junction_cycle_is_finite() {
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    conn(
        &mut fx,
        0x2001,
        Some(P1),
        Ep::J(0x3001),
        Ep::J(0x3002),
        Value::Null,
    );
    conn(
        &mut fx,
        0x2002,
        Some(P1),
        Ep::J(0x3002),
        Ep::J(0x3003),
        Value::Null,
    );
    conn(
        &mut fx,
        0x2003,
        Some(P1),
        Ep::J(0x3003),
        Ep::J(0x3001),
        Value::Null,
    );
    conn(&mut fx, 0x2004, Some(P1), Ep::J(0x3001), cp(0x1001), Value::Null);
    junction(&mut fx, 0x3001, P1, &[0x2001, 0x2003, 0x2004]);
    junction(&mut fx, 0x3002, P1, &[0x2001, 0x2002]);
    junction(&mut fx, 0x3003, P1, &[0x2002, 0x2003]);
    // Page 2: a cycle of junctions and no CP.
    conn(
        &mut fx,
        0x2011,
        Some(P2),
        Ep::J(0x3011),
        Ep::J(0x3012),
        Value::Null,
    );
    conn(
        &mut fx,
        0x2012,
        Some(P2),
        Ep::J(0x3012),
        Ep::J(0x3013),
        Value::Null,
    );
    conn(
        &mut fx,
        0x2013,
        Some(P2),
        Ep::J(0x3013),
        Ep::J(0x3011),
        Value::Null,
    );
    junction(&mut fx, 0x3011, P2, &[0x2011, 0x2013]);
    junction(&mut fx, 0x3012, P2, &[0x2011, 0x2012]);
    junction(&mut fx, 0x3013, P2, &[0x2012, 0x2013]);
    let m = fx.build();
    let q = Q::new(&m);
    let l1 = q.get::<PageLocalNets>(id::<Page>(P1));
    assert_eq!(l1.nets.len(), 1);
    assert_eq!(l1.nets[0].junctions().count(), 3);
    assert_eq!(l1.nets[0].cps().collect::<Vec<_>>(), vec![cpref(0x1001)]);
    let l2 = q.get::<PageLocalNets>(id::<Page>(P2));
    assert_eq!(l2.nets.len(), 1);
    assert_eq!(l2.nets[0].junctions().count(), 3);
    assert_eq!(l2.nets[0].id, NetId(Id::cast(id::<Junction>(0x3011))));
    let _ = q.get::<StitchGraphQ>(());
    let _ = q.get::<PotentialPartition>(());
}

#[test]
fn dangling_endpoint_is_harmless() {
    let m = with_end_b(&two_functions().build(), 0x2001, cp(0x9999));
    let q = Q::new(&m);
    let _ = q.get::<PageLocalNets>(id::<Page>(P1));
    let _ = q.get::<StitchGraphQ>(());
    let _ = q.get::<PotentialPartition>(());
}

#[test]
fn potential_id_prefers_definitions() {
    let build = |defs: &[(u128, Ep, &str)]| {
        let mut fx = base();
        func(&mut fx, 0x1001, P1);
        func(&mut fx, 0x1002, P1);
        conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1002), Value::Null);
        for (n, at, name) in defs {
            def(&mut fx, *n, *at, name);
        }
        fx.build()
    };
    // Two definitions, equal names: the smaller ID names the potential, no note.
    let m = build(&[(0x5002, cp(0x1002), "N"), (0x5001, cp(0x1001), "N")]);
    let mut q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potentials.len(), 1);
    assert_eq!(part.potentials[0].id, PotentialId::Def(id(0x5001)));
    assert_eq!(
        part.potentials[0].defs,
        vec![id::<PotentialDef>(0x5001), id(0x5002)]
    );
    assert!(!has_note(&mut q, "EW-POT-017"));
    // Different names: the first wins and the other is noted.
    let m = build(&[(0x5002, cp(0x1002), "Other"), (0x5001, cp(0x1001), "First")]);
    let mut q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potentials[0].id, PotentialId::Def(id(0x5001)));
    assert_eq!(part.potentials[0].name.text, "First");
    assert!(has_note(&mut q, "EW-POT-017"));
    // No definition: the minimum CP ID.
    let m = build(&[]);
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    let min = cpref(0x1001).id().min(cpref(0x1002).id());
    assert_eq!(part.potentials.len(), 1);
    assert_eq!(part.potentials[0].id, PotentialId::Cp(min));
}

/// A model with seven connections, a junction, an IP pair, a bridge and a definition.
fn scenario() -> Fx {
    let mut fx = base();
    for f in 0x1001..=0x1008 {
        func(&mut fx, f, if f % 2 == 0 { P2 } else { P1 });
    }
    ip(&mut fx, 0x1101, P1, "S", "source");
    ip(&mut fx, 0x1102, P2, "S", "target");
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), Ep::J(0x3001), Value::Null);
    conn(&mut fx, 0x2002, Some(P1), Ep::J(0x3001), cp(0x1003), Value::Null);
    conn(&mut fx, 0x2003, Some(P1), Ep::J(0x3001), cp(0x1101), Value::Null);
    conn(&mut fx, 0x2004, Some(P2), cp(0x1102), cp(0x1002), Value::Null);
    conn(&mut fx, 0x2005, Some(P2), cp(0x1004), cp(0x1006), Value::Null);
    conn(&mut fx, 0x2006, None, cp(0x1005), cp(0x1007), Value::Null);
    conn(&mut fx, 0x2007, Some(P2), cp(0x1006), cp(0x1008), bridge());
    junction(&mut fx, 0x3001, P1, &[0x2001, 0x2002, 0x2003]);
    def(&mut fx, 0x5001, cp(0x1001), "NET");
    fx
}

fn render(fx: &Fx) -> (String, String) {
    let m = fx.build();
    let q = Q::new(&m);
    (
        format!("{:?}", *q.get::<PotentialPartition>(())),
        format!("{:?}", *q.get::<StitchGraphQ>(())),
    )
}

#[test]
fn derived_values_do_not_depend_on_record_order() {
    let fx = scenario();
    let expected = render(&fx);
    assert!(expected.0.contains("NET"));
    let mut reversed = fx.clone();
    reversed.records.reverse();
    assert_eq!(render(&reversed), expected);
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    for _ in 0..3 {
        let mut shuffled = fx.clone();
        for i in (1..shuffled.records.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let j = ((state >> 33) as usize) % (i + 1);
            shuffled.records.swap(i, j);
        }
        assert_eq!(render(&shuffled), expected);
    }
}

#[test]
fn no_hash_containers_in_connectivity_sources() {
    let sources = [
        ("nets.rs", include_str!("../src/nets.rs")),
        ("stitch.rs", include_str!("../src/stitch.rs")),
        ("potentials.rs", include_str!("../src/potentials.rs")),
        ("wires.rs", include_str!("../src/wires.rs")),
        ("flow.rs", include_str!("../src/flow.rs")),
        ("wire_props.rs", include_str!("../src/wire_props.rs")),
        ("wire_profile.rs", include_str!("../src/wire_profile.rs")),
    ];
    for (name, text) in sources {
        assert!(!text.contains("HashMap"), "{name} uses HashMap");
        assert!(!text.contains("HashSet"), "{name} uses HashSet");
    }
}

#[test]
fn queries_survive_a_device_parent_cycle() {
    const STRIP: u128 = 0x5000;
    const T1: u128 = 0x4001;
    const T2: u128 = 0x4002;
    let mut fx = base();
    fx.device(STRIP, "X", Some(1)).with("strip", json!({}));
    for (n, seq) in [(T1, 1), (T2, 2)] {
        fx.device(n, "X", None).with(
            "terminal",
            json!({"strip": uid(STRIP), "seq": seq, "designation": seq.to_string(), "rail_bonded": true}),
        );
    }
    for (f, dev) in [(0x1001, T1), (0x1002, T2)] {
        fx.function(f, Some(dev), FNDEF, true);
        fx.rep(f + 0x10_000, P1, SYM, &[f], 0, 0);
    }
    func(&mut fx, 0x1003, P1);
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1003), Value::Null);
    conn(&mut fx, 0x2002, Some(P1), cp(0x1002), cp(0x1003), Value::Null);
    let m = fx.build();
    let ops: Vec<Op> = [(T1, T2), (T2, T1)]
        .iter()
        .map(|(n, parent)| {
            let dev: Id<Device> = id(*n);
            let before = m.row(&Device::key(dev)).expect("device exists");
            let mut after = (**before.get::<Device>().expect("a device")).clone();
            after.parent = Some(id(*parent));
            after.tag.scheme = TagScheme::Child;
            Op::update(before, AnyRow::of(after))
        })
        .collect();
    let m = m.apply_unchecked(&ops, Rev(1)).expect("ops apply");
    assert!(m.integrity_check().is_err(), "the probe violates an invariant");
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert!(!part.potentials.is_empty());
}
