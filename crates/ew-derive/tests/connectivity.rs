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
//! - `self_loop_connection_is_harmless`: a connection from a CP to itself; every query terminates, no wire.
//! - `junction_cycle_is_finite`: a junction cycle terminates through every query, with and without a CP.
//! - `dangling_endpoint_is_harmless`: a connection to a function that does not exist; every query, no wire.
//! - `potential_id_prefers_definitions`: definition ID, EW-POT-017, else the minimum CP ID.
//! - `derived_values_do_not_depend_on_record_order`: partition and stitch graph under record permutations.
//! - `no_hash_containers_in_connectivity_sources`: no `HashMap`/`HashSet` in the connectivity sources.
//! - `queries_survive_a_device_parent_cycle`: every query with rail-bonded terminals in a parent cycle.
//! - `double_deck_terminal_keeps_its_levels_apart`: equal designations on two levels are not one pin (D2).
//! - `duplicate_pins_join_only_on_pattern_functions`: duplicated pins are one pin on black boxes only (D2).
//! - `pass_through_terminal_shorts_each_side`: a pass-through terminal shorts each side apart (D3).
//! - `override_provenance_names_the_constituent_that_carries_it`: level 7 names the constituent (F2).
//! - `hub_arm_is_carried_by_the_connection_touching_its_end`: hub arm carrier, doc 08 §3.8 (D1).
//! - `external_source_ip_names_an_undefined_potential`: an external source IP names, after non-external (D6).
//! - `ambiguous_junction_feed_is_noted`: EW-POT-011, a link that feeds both junctions or neither.
//! - `ring_is_broken_at_its_highest_connection`: EW-POT-012, the highest connection of a ring forms no wire.
//! - `conflicting_conductors_are_noted`: EW-POT-014, two constituents name different conductors.
//! - `conflicting_overrides_are_noted`: EW-POT-014, different `csa` overrides; the carrier's value applies.
//! - `star_ip_group_with_three_targets_is_a_hub`: one source and three targets are three wires (doc 08 §3.8).
//! - `chain_mode_pairs_in_order`: chain pairing (1,2), (3,4) in sort-key order.
//! - `potential_def_props_are_level_3`: `PotentialDef.props` is level 3 with its provenance.
//! - `project_default_is_level_1`: `wire_default` is level 1, the built-ins level 0.
//! - `malformed_colour_profile_is_noted`: a malformed `colour_profile` is noted and the built-in colour applies.
//! - `nested_junction_chain_fr_con_041`: a chain junction entering a chain junction: src -> d1 -> d2 -> d3.

mod common;

use common::{Fx, id, uid};
use ew_derive::{
    CpRef, DeriveNote, EdgeKind, EffectiveProps, Level, NameSource, NetId, PageLocalNets, PairMode,
    PhysicalWiresOf, PotentialFlow, PotentialId, PotentialPartition, PropSource, Q, ShortSource,
    StitchGraphQ, WirePropsOf,
};
use ew_ids::Id;
use ew_model::{AnyRow, EndpointKey, Entity, Model, Op, Rev};
use ew_schema::{Connection, Csa, Device, Endpoint, Function, Junction, Page, PotentialDef, TagScheme};
use ew_units::Dec3;
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

/// Every connectivity query for every potential of the partition (hostile-shape probes): they all terminate.
/// Returns the number of physical wires over all potentials, each counted once per potential that holds it,
/// after checking that `WirePropsOf` has one entry per wire and `PotentialFlow` answers for the potential.
fn all_queries(q: &Q) -> usize {
    let _ = q.get::<StitchGraphQ>(());
    let part = q.get::<PotentialPartition>(());
    let mut wires = 0;
    for p in &part.potentials {
        let physical = q.get::<PhysicalWiresOf>(p.id);
        let props = q.get::<WirePropsOf>(p.id);
        assert_eq!(props.wires.len(), physical.wires.len(), "{:?}", p.id);
        let _ = q.get::<PotentialFlow>(p.id);
        wires += physical.wires.len();
    }
    wires
}

#[test]
fn self_loop_connection_is_harmless() {
    let m = with_end_b(&two_functions().build(), 0x2001, cp(0x1001));
    let q = Q::new(&m);
    let local = q.get::<PageLocalNets>(id::<Page>(P1));
    assert_eq!(local.nets.len(), 1);
    assert_eq!(local.nets[0].cps().collect::<Vec<_>>(), vec![cpref(0x1001)]);
    // The loop closes a ring at once: it forms no wire (EW-POT-012).
    assert_eq!(all_queries(&q), 0);
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
    // One CP on page 1 and none on page 2: no wire ends at two CPs.
    assert_eq!(all_queries(&q), 0);
}

#[test]
fn dangling_endpoint_is_harmless() {
    let m = with_end_b(&two_functions().build(), 0x2001, cp(0x9999));
    let q = Q::new(&m);
    let _ = q.get::<PageLocalNets>(id::<Page>(P1));
    // The stale endpoint is still a node: the connection forms its one wire, ending at the missing CP.
    assert_eq!(all_queries(&q), 1);
    let part = q.get::<PotentialPartition>(());
    let wires = q.get::<PhysicalWiresOf>(part.potentials[0].id);
    assert_eq!(wires.wires[0].id, id::<Connection>(0x2001));
    assert_eq!(wires.wires[0].ends.map(|e| e.cp), [cpref(0x1001), cpref(0x9999)]);
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
        ("cps.rs", include_str!("../src/cps.rs")),
        ("pages.rs", include_str!("../src/pages.rs")),
        ("structure.rs", include_str!("../src/structure.rs")),
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
    // The two wires 0x2001 and 0x2002 end at the plain function and at one terminal each.
    assert_eq!(all_queries(&q), 2);
}

// ---------------------------------------------------------------------------------------------------------
// Review cycle 7 A: duplicate pins (D2), pass-through terminals (D3), override provenance (F2), hub-arm
// carrier (D1).

const FNDEF_T: u128 = 0x520;
const SYM_T: u128 = 0x530;

/// A four-CP FnDef `FNDEF_T` of `contact_kind` with `cps` and `internal` links, and its symbol `SYM_T`.
fn four_cp_def(fx: &mut Fx, contact_kind: &str, cps: Value, internal: Value) {
    fx.lib_item(
        "fndef",
        FNDEF_T,
        json!({
            "meta": {}, "category": ["Misc", "Misc", "Four"], "contact_kind": contact_kind,
            "cps": cps, "internal": internal,
        }),
    );
    fx.symbol(
        SYM_T,
        FNDEF_T,
        json!("origin"),
        &[
            (0, 0, 0, 0),
            (1, 1, 0, 1_000_000),
            (2, 2, 1_000_000, 0),
            (3, 3, 1_000_000, 1_000_000),
        ],
    );
}

/// Function `0x1500` of `FNDEF_T` (kind `kind`, `None` for normal) on P1; plain functions 0x1001..0x1004
/// wired to its CPs 0..3 by connections 0x2001..0x2004.
fn four_cp_circuit(fx: &mut Fx, kind: Option<&str>) -> Model {
    fx.device(0x21_500, "X", Some(500));
    fx.function(0x1500, Some(0x21_500), FNDEF_T, true);
    if let Some(k) = kind {
        fx.with("kind", json!(k));
    }
    fx.rep(0x11_500, P1, SYM_T, &[0x1500], 0, 0);
    for i in 0..4u16 {
        let f = 0x1001 + u128::from(i);
        func(fx, f, P1);
        conn(
            fx,
            0x2001 + u128::from(i),
            Some(P1),
            cp(f),
            Ep::Cp(0x1500, i),
            Value::Null,
        );
    }
    fx.build()
}

/// The potentials of a model as the plain functions 0x1001..0x1004 each holds.
fn groups(m: &Model) -> Vec<Vec<u128>> {
    let q = Q::new(m);
    let part = q.get::<PotentialPartition>(());
    let mut out: Vec<Vec<u128>> = part
        .potentials
        .iter()
        .map(|p| {
            (0x1001..=0x1004u128)
                .filter(|f| p.cps.contains(&cpref(*f)))
                .collect::<Vec<_>>()
        })
        .collect();
    out.sort();
    out
}

#[test]
fn double_deck_terminal_keeps_its_levels_apart() {
    // The reviewer's probe: a/L1, b/L1, a/L2, b/L2 (equal designations across the levels).
    let mut fx = base();
    four_cp_def(
        &mut fx,
        "terminal",
        json!([
            {"idx": 0, "designation": "a", "level": 1, "clamp": {"clamp": "a"}},
            {"idx": 1, "designation": "b", "level": 1, "clamp": {"clamp": "b"}},
            {"idx": 2, "designation": "a", "level": 2, "clamp": {"clamp": "a"}},
            {"idx": 3, "designation": "b", "level": 2, "clamp": {"clamp": "b"}},
        ]),
        json!([]),
    );
    let m = four_cp_circuit(&mut fx, None);
    assert_eq!(groups(&m), vec![vec![0x1001, 0x1002], vec![0x1003, 0x1004]]);
    let q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert!(!g.edges.iter().any(|e| matches!(
        e.kind,
        EdgeKind::Short {
            source: ShortSource::DuplicatePin,
            ..
        }
    )));
}

#[test]
fn duplicate_pins_join_only_on_pattern_functions() {
    let cps = json!([
        {"idx": 0, "designation": "+"},
        {"idx": 1, "designation": "+"},
        {"idx": 2, "designation": "-"},
        {"idx": 3, "designation": "-"},
    ]);
    // A black box (no part pattern: it falls back to its FnDef, still one pin per designation).
    let mut fx = base();
    four_cp_def(&mut fx, "black_box", cps.clone(), json!([]));
    let m = four_cp_circuit(&mut fx, Some("black_box"));
    assert_eq!(groups(&m), vec![vec![0x1001, 0x1002], vec![0x1003, 0x1004]]);
    // An FnDef-origin function: equal designations are separate points.
    let mut fx = base();
    four_cp_def(&mut fx, "pole", cps, json!([]));
    let m = four_cp_circuit(&mut fx, None);
    assert_eq!(
        groups(&m),
        vec![vec![0x1001], vec![0x1002], vec![0x1003], vec![0x1004]]
    );
}

#[test]
fn pass_through_terminal_shorts_each_side() {
    // A disconnect terminal: clamps a and b on each side, one opening pass-through between the sides.
    let mut fx = base();
    four_cp_def(
        &mut fx,
        "terminal",
        json!([
            {"idx": 0, "designation": "1", "level": 1, "side": "side1", "clamp": {"clamp": "a"}},
            {"idx": 1, "designation": "1", "level": 1, "side": "side1", "clamp": {"clamp": "b"}},
            {"idx": 2, "designation": "2", "level": 1, "side": "side2", "clamp": {"clamp": "a"}},
            {"idx": 3, "designation": "2", "level": 1, "side": "side2", "clamp": {"clamp": "b"}},
        ]),
        json!([{"a": 0, "b": 2, "kind": {"pass_through": {"opens": true}}}]),
    );
    let m = four_cp_circuit(&mut fx, None);
    assert_eq!(groups(&m), vec![vec![0x1001, 0x1002], vec![0x1003, 0x1004]]);
    let q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    let shorts: Vec<(u16, u16)> = g
        .edges
        .iter()
        .filter_map(|e| match e.kind {
            EdgeKind::Short {
                a,
                b,
                source: ShortSource::TerminalLevel(1),
                ..
            } => Some((a, b)),
            _ => None,
        })
        .collect();
    assert_eq!(shorts, vec![(0, 1), (2, 3)]);
    assert!(
        g.edges
            .iter()
            .any(|e| matches!(e.kind, EdgeKind::PassThrough { a: 0, b: 2, .. }))
    );
}

#[test]
fn override_provenance_names_the_constituent_that_carries_it() {
    // A -- 0x2001 -- X source .. X target -- 0x2002 -- B: the carrier is 0x2002 (next to the Target).
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    func(&mut fx, 0x1002, P2);
    ip(&mut fx, 0x1101, P1, "X", "source");
    ip(&mut fx, 0x1102, P2, "X", "target");
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1101), Value::Null);
    fx.with("overrides", json!({"csa": "2.5"}));
    conn(&mut fx, 0x2002, Some(P2), cp(0x1102), cp(0x1002), Value::Null);
    fx.with("overrides", json!({"insulation": "H07V-K"}));
    let m = fx.build();
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potentials.len(), 1);
    let pid = part.potentials[0].id;
    let wires = q.get::<PhysicalWiresOf>(pid);
    assert_eq!(wires.wires.len(), 1);
    let w = &wires.wires[0];
    assert_eq!(w.id, id::<Connection>(0x2002));
    assert_eq!(w.override_sources.csa, Some(id(0x2001)));
    assert_eq!(w.override_sources.insulation, Some(id(0x2002)));
    let props = q.get::<WirePropsOf>(pid);
    let e = &props.wires[&w.id];
    let csa = e.csa.as_ref().expect("a cross-section");
    assert_eq!(csa.source, PropSource::Override(id(0x2001)));
    let ins = e.insulation.as_ref().expect("an insulation");
    assert_eq!(ins.source, PropSource::Override(id(0x2002)));
}

#[test]
fn hub_arm_is_carried_by_the_connection_touching_its_end() {
    // The reviewer's probe: J (star) -- 0x2003 -- IP X target .. X source -- 0x2004 -- C. Doc 08 §3.8:
    // "carrier = connection of a that touches c", so 0x2004 carries the wire, not the leg at the Target.
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    func(&mut fx, 0x1002, P1);
    func(&mut fx, 0x1003, P2);
    ip(&mut fx, 0x1101, P1, "X", "target");
    ip(&mut fx, 0x1102, P2, "X", "source");
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), Ep::J(0x3001), Value::Null);
    conn(&mut fx, 0x2002, Some(P1), Ep::J(0x3001), cp(0x1002), Value::Null);
    conn(&mut fx, 0x2003, Some(P1), Ep::J(0x3001), cp(0x1101), Value::Null);
    conn(&mut fx, 0x2004, Some(P2), cp(0x1102), cp(0x1003), Value::Null);
    junction(&mut fx, 0x3001, P1, &[0x2001, 0x2002, 0x2003]);
    let m = fx.build();
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potentials.len(), 1);
    let wires = q.get::<PhysicalWiresOf>(part.potentials[0].id);
    let got: Vec<(Id<Connection>, Vec<Id<Connection>>)> = wires
        .wires
        .iter()
        .map(|w| (w.id, w.constituents.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (id(0x2002), vec![id(0x2001), id(0x2002)]),
            (id(0x2004), vec![id(0x2001), id(0x2003), id(0x2004)]),
        ]
    );
}

// ---------------------------------------------------------------------------------------------------------
// Review cycle 7 A, F9/F12: naming, feed ambiguity, rings, conflicts, hubs, pairing, property levels.

/// An interruption point `n` on `page` with the full spec `ip` (name, direction, sort, external ...).
fn ip_spec(fx: &mut Fx, n: u128, page: u128, spec: Value) {
    fx.push(json!({
        "$k": "function", "id": uid(n), "def": uid(FNDEF_IP), "kind": {"ip": spec},
    }));
    fx.rep(n + 0x10_000, page, SYM_IP, &[n], 0, n as i64 * 1_000_000);
}

/// Two plain functions 0x1001 and 0x1002 on P1 joined by wire 0x2001.
fn pair_fixture() -> Fx {
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    func(&mut fx, 0x1002, P1);
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1002), Value::Null);
    fx
}

/// The ID of the only potential of the partition.
fn only_potential(q: &Q) -> PotentialId {
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potentials.len(), 1, "{part:?}");
    part.potentials[0].id
}

/// Whether a note `code` about `subject` is present.
fn has_note_on(q: &mut Q, code: &str, subject: &str) -> bool {
    q.diagnostics().notes().iter().any(
        |n| matches!(n, DeriveNote::Connectivity { code: c, subject: s, .. } if *c == code && s == subject),
    )
}

#[test]
fn external_source_ip_names_an_undefined_potential() {
    let mut fx = pair_fixture();
    fx.node(0x900, "location", "J", None);
    fx.push(json!({
        "$k": "external_ref", "id": uid(0x910), "location": uid(0x900), "device": "JE-N1", "connection": "L",
    }));
    ip_spec(
        &mut fx,
        0x1101,
        P1,
        json!({"name": "SUPPLY", "direction": "source", "external": uid(0x910)}),
    );
    conn(&mut fx, 0x2002, Some(P1), cp(0x1101), cp(0x1001), Value::Null);
    // No definition and no other source point: the external one names the potential (D6).
    let m = fx.build();
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potentials.len(), 1, "{part:?}");
    let p = &part.potentials[0];
    assert_eq!(p.name.text, "SUPPLY");
    assert_eq!(p.name.source, NameSource::SourceIp(id(0x1101)));
    // A non-external source point on the same potential names it first; the external name is an alias.
    ip(&mut fx, 0x1102, P1, "INNER", "source");
    conn(&mut fx, 0x2003, Some(P1), cp(0x1102), cp(0x1002), Value::Null);
    let m = fx.build();
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potentials.len(), 1, "{part:?}");
    let p = &part.potentials[0];
    assert_eq!(p.name.text, "INNER");
    assert_eq!(p.name.source, NameSource::SourceIp(id(0x1102)));
    assert_eq!(p.aliases, vec!["SUPPLY".to_owned()]);
}

/// Two junctions J1 (0x3001) and J2 (0x3002) with A=0x1001 and B=0x1002. `seq1` and `seq2` list the
/// connection numbers of the sequences. Connection 0x2001 joins the junctions, 0x2002 is J1 -- A and 0x2003
/// is J2 -- B.
fn two_junctions(seq1: &[u128], seq2: &[u128]) -> Model {
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    func(&mut fx, 0x1002, P1);
    conn(
        &mut fx,
        0x2001,
        Some(P1),
        Ep::J(0x3001),
        Ep::J(0x3002),
        Value::Null,
    );
    conn(&mut fx, 0x2002, Some(P1), Ep::J(0x3001), cp(0x1001), Value::Null);
    conn(&mut fx, 0x2003, Some(P1), Ep::J(0x3002), cp(0x1002), Value::Null);
    junction(&mut fx, 0x3001, P1, seq1);
    junction(&mut fx, 0x3002, P1, seq2);
    fx.build()
}

#[test]
fn ambiguous_junction_feed_is_noted() {
    // EW-POT-011, doc 08 §3.2: the arm between two junctions is the feed arm of both ...
    let m = two_junctions(&[0x2001, 0x2002], &[0x2001, 0x2003]);
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let wires = q.get::<PhysicalWiresOf>(pid);
    assert!(has_note(&mut q, "EW-POT-011"), "{:?}", q.diagnostics().notes());
    // ... the tree is rooted at the lowest junction with an arm at a CP: one wire.
    assert_eq!(wires.wires.len(), 1);
    let w = &wires.wires[0];
    assert_eq!(w.id, id::<Connection>(0x2003));
    assert_eq!(
        w.constituents,
        vec![id::<Connection>(0x2002), id(0x2001), id(0x2003)]
    );
    assert_eq!(w.ends[0].cp, cpref(0x1001));
    assert_eq!(w.ends[1].cp, cpref(0x1002));

    // ... or of neither (both junctions list it last).
    let m = two_junctions(&[0x2002, 0x2001], &[0x2003, 0x2001]);
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let wires = q.get::<PhysicalWiresOf>(pid);
    assert!(has_note(&mut q, "EW-POT-011"), "{:?}", q.diagnostics().notes());
    assert_eq!(wires.wires.len(), 1);
    assert_eq!(wires.wires[0].id, id::<Connection>(0x2003));

    // A consistent tree is silent: J1 is fed by 0x2002, J2 by 0x2001.
    let m = two_junctions(&[0x2002, 0x2001], &[0x2001, 0x2003]);
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let wires = q.get::<PhysicalWiresOf>(pid);
    assert!(!has_note(&mut q, "EW-POT-011"), "{:?}", q.diagnostics().notes());
    assert_eq!(wires.wires.len(), 1);
}

#[test]
fn ring_is_broken_at_its_highest_connection() {
    // A ring of three CPs; the highest connection ID (0x2003) is neither authored first nor last.
    let mut fx = base();
    for f in [0x1001, 0x1002, 0x1003] {
        func(&mut fx, f, P1);
    }
    conn(&mut fx, 0x2002, Some(P1), cp(0x1001), cp(0x1002), Value::Null);
    conn(&mut fx, 0x2003, Some(P1), cp(0x1002), cp(0x1003), Value::Null);
    conn(&mut fx, 0x2001, Some(P1), cp(0x1003), cp(0x1001), Value::Null);
    let m = fx.build();
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let wires = q.get::<PhysicalWiresOf>(pid);
    let ids: Vec<Id<Connection>> = wires.wires.iter().map(|w| w.id).collect();
    assert_eq!(ids, vec![id::<Connection>(0x2001), id(0x2002)]);
    assert!(has_note_on(
        &mut q,
        "EW-POT-012",
        &id::<Connection>(0x2003).to_string()
    ));
    // The other two connections of the ring still form their wires.
    assert_eq!(wires.wires[0].ends.map(|e| e.cp), [cpref(0x1001), cpref(0x1003)]);
    assert_eq!(wires.wires[1].ends.map(|e| e.cp), [cpref(0x1001), cpref(0x1002)]);
}

/// A cable `W1` with conductors 0x6101 and 0x6102 and an interruption-point pair joining A=0x1001 and
/// B=0x1002 through connections 0x2001 (A side, at the Source) and 0x2002 (B side, at the Target: the
/// carrier). `kindN` and `overN` are the kind and the overrides of connection 0x200N (`Null`: none).
fn conflict_fixture(kind1: Value, over1: Value, kind2: Value, over2: Value) -> Model {
    let mut fx = base();
    fx.push(json!({"$k": "device", "id": uid(0x6000), "tag": {"letter": "W", "counter": 1}, "cable": {}}));
    for (n, core, csa) in [(0x6101, "1", "1"), (0x6102, "2", "4")] {
        fx.push(json!({"$k": "conductor", "id": uid(n), "cable": uid(0x6000), "core": core, "csa": csa}));
    }
    func(&mut fx, 0x1001, P1);
    func(&mut fx, 0x1002, P2);
    ip(&mut fx, 0x1101, P1, "X", "source");
    ip(&mut fx, 0x1102, P2, "X", "target");
    for (n, page, a, b, kind, over) in [
        (0x2001, P1, cp(0x1001), cp(0x1101), kind1, over1),
        (0x2002, P2, cp(0x1102), cp(0x1002), kind2, over2),
    ] {
        conn(&mut fx, n, Some(page), a, b, kind);
        if !over.is_null() {
            fx.with("overrides", over);
        }
    }
    fx.build()
}

#[test]
fn conflicting_conductors_are_noted() {
    let m = conflict_fixture(
        json!({"core": uid(0x6101)}),
        Value::Null,
        json!({"core": uid(0x6102)}),
        Value::Null,
    );
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let wires = q.get::<PhysicalWiresOf>(pid);
    assert_eq!(wires.wires.len(), 1);
    let w = &wires.wires[0];
    // The carrier's conductor applies (doc 08 §3.8): the leg at the Target point.
    assert_eq!(w.id, id::<Connection>(0x2002));
    assert_eq!(w.core, Some(id(0x6102)));
    assert!(has_note_on(&mut q, "EW-POT-014", &w.id.to_string()));
    let props = q.get::<WirePropsOf>(pid);
    let csa = props.wires[&w.id].csa.as_ref().expect("a cross-section");
    assert_eq!(csa.source, PropSource::Conductor(id(0x6102)));
    assert_eq!(csa.value, Csa::Mm2(Dec3(4_000)));
    // Agreeing conductors are silent.
    let m = conflict_fixture(
        json!({"core": uid(0x6102)}),
        Value::Null,
        json!({"core": uid(0x6102)}),
        Value::Null,
    );
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    assert_eq!(q.get::<PhysicalWiresOf>(pid).wires[0].core, Some(id(0x6102)));
    assert!(!has_note(&mut q, "EW-POT-014"));
}

#[test]
fn conflicting_overrides_are_noted() {
    let m = conflict_fixture(
        Value::Null,
        json!({"csa": "1.5"}),
        Value::Null,
        json!({"csa": "2.5"}),
    );
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let wires = q.get::<PhysicalWiresOf>(pid);
    let w = &wires.wires[0];
    assert_eq!(w.id, id::<Connection>(0x2002));
    assert!(has_note_on(&mut q, "EW-POT-014", &w.id.to_string()));
    // The carrier's value applies and names the carrier.
    let props = q.get::<WirePropsOf>(pid);
    let csa = props.wires[&w.id].csa.as_ref().expect("a cross-section");
    assert_eq!(csa.value, Csa::Mm2(Dec3(2_500)));
    assert_eq!(csa.source, PropSource::Override(id(0x2002)));
    // Equal values on both constituents are no conflict.
    let m = conflict_fixture(
        Value::Null,
        json!({"csa": "2.5"}),
        Value::Null,
        json!({"csa": "2.5"}),
    );
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let _ = q.get::<PhysicalWiresOf>(pid);
    assert!(!has_note(&mut q, "EW-POT-014"));
}

#[test]
fn star_ip_group_with_three_targets_is_a_hub() {
    // A -- 0x2001 -- IP X (source) | IP X (target) -- 0x2002 -- B, and likewise 0x2003 -- C, 0x2004 -- D.
    let mut fx = base();
    func(&mut fx, 0x1001, P1);
    for f in [0x1002, 0x1003, 0x1004] {
        func(&mut fx, f, P2);
    }
    ip(&mut fx, 0x1101, P1, "X", "source");
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), cp(0x1101), Value::Null);
    for (k, t) in [0x1102, 0x1103, 0x1104].into_iter().enumerate() {
        ip(&mut fx, t, P2, "X", "target");
        let b = 0x1002 + k as u128;
        conn(&mut fx, 0x2002 + k as u128, Some(P2), cp(t), cp(b), Value::Null);
    }
    let m = fx.build();
    let q = Q::new(&m);
    let pid = only_potential(&q);
    let wires = q.get::<PhysicalWiresOf>(pid);
    type Row = (Id<Connection>, Vec<Id<Connection>>, [CpRef; 2]);
    let got: Vec<Row> = wires
        .wires
        .iter()
        .map(|w| (w.id, w.constituents.clone(), w.ends.map(|e| e.cp)))
        .collect();
    // Doc 08 §3.8: "carrier = connection of a that touches c", the target leg; the feed arm comes first.
    assert_eq!(
        got,
        vec![
            (
                id(0x2002),
                vec![id(0x2001), id(0x2002)],
                [cpref(0x1001), cpref(0x1002)]
            ),
            (
                id(0x2003),
                vec![id(0x2001), id(0x2003)],
                [cpref(0x1001), cpref(0x1003)]
            ),
            (
                id(0x2004),
                vec![id(0x2001), id(0x2004)],
                [cpref(0x1001), cpref(0x1004)]
            ),
        ]
    );
    // The shared feed connection belongs to all three wires.
    let props = q.get::<WirePropsOf>(pid);
    assert_eq!(
        props.by_connection[&id::<Connection>(0x2001)],
        vec![id::<Connection>(0x2002), id(0x2003), id(0x2004)]
    );
}

#[test]
fn chain_mode_pairs_in_order() {
    // Four points named C with sort keys, authored out of order; each is wired to a function.
    let mut fx = base();
    fx.project_set(
        "settings",
        json!({"ip_pairing": {"mode": "chain", "scope": "project"}}),
    );
    for f in 0x1001..=0x1004 {
        func(&mut fx, f, P1);
    }
    // (point, sort key, function): the sort order is 0x1102, 0x1104, 0x1101, 0x1103.
    for (ipn, sort, f) in [
        (0x1101, 30, 0x1003),
        (0x1102, 10, 0x1001),
        (0x1103, 40, 0x1004),
        (0x1104, 20, 0x1002),
    ] {
        ip_spec(
            &mut fx,
            ipn,
            P1,
            json!({"name": "C", "direction": "source", "sort": sort}),
        );
        conn(
            &mut fx,
            ipn - 0x1101 + 0x2001,
            Some(P1),
            cp(f),
            cp(ipn),
            Value::Null,
        );
    }
    let m = fx.build();
    let q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert_eq!(g.ip_groups.len(), 1);
    let group = &g.ip_groups[0];
    assert_eq!(group.name, "C");
    assert_eq!(group.mode, PairMode::Chain);
    assert_eq!(
        group.members,
        vec![id::<Function>(0x1102), id(0x1104), id(0x1101), id(0x1103)]
    );
    assert_eq!(
        group.pairs,
        vec![(id::<Function>(0x1102), id(0x1104)), (id(0x1101), id(0x1103))]
    );
    assert!(group.unpaired.is_empty());
    // (1,2) and (3,4) in sort-key order: functions 0x1001 + 0x1002, and 0x1003 + 0x1004.
    assert_eq!(groups(&m), vec![vec![0x1001, 0x1002], vec![0x1003, 0x1004]]);
}

/// The effective properties of the only wire of the only potential of `m`.
fn single_wire_props(m: &Model) -> EffectiveProps {
    let q = Q::new(m);
    let pid = only_potential(&q);
    let props = q.get::<WirePropsOf>(pid);
    assert_eq!(props.wires.len(), 1);
    props.wires.values().next().expect("one wire").clone()
}

#[test]
fn potential_def_props_are_level_3() {
    let mut fx = pair_fixture();
    def(&mut fx, 0x5001, cp(0x1001), "N1");
    fx.with("props", json!({"csa": "2.5"}));
    let e = single_wire_props(&fx.build());
    let csa = e.csa.expect("a cross-section");
    assert_eq!(csa.value, Csa::Mm2(Dec3(2_500)));
    assert_eq!(csa.source, PropSource::PotentialDef(id(0x5001)));
    assert_eq!(csa.level(), 3);
    // A member the definition leaves unset falls through to the built-in default.
    assert_eq!(e.colour.expect("a colour").level(), 0);
}

#[test]
fn project_default_is_level_1() {
    let mut fx = pair_fixture();
    fx.project_set("settings", json!({"wire_default": {"csa": "1"}}));
    let e = single_wire_props(&fx.build());
    let csa = e.csa.expect("a cross-section");
    assert_eq!(csa.value, Csa::Mm2(Dec3(1_000)));
    assert_eq!(csa.source, PropSource::ProjectDefault);
    assert_eq!(csa.level(), 1);
    // The default sets no colour: the built-in one applies.
    assert_eq!(e.colour.expect("a colour").level(), 0);
    // Without the default and without profiles: the built-in size (Control, 0.75).
    let e = single_wire_props(&pair_fixture().build());
    let csa = e.csa.expect("a cross-section");
    assert_eq!(csa.value, Csa::Mm2(Dec3(750)));
    assert!(matches!(csa.source, PropSource::BuiltIn { .. }));
    assert_eq!(csa.level(), 0);
}

#[test]
fn malformed_colour_profile_is_noted() {
    let mut fx = pair_fixture();
    fx.project_set("settings", json!({"colour_profile": 42}));
    let m = fx.build();
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let props = q.get::<WirePropsOf>(pid);
    let notes = q.diagnostics().notes();
    assert!(
        notes.iter().any(|n| matches!(
            n,
            DeriveNote::MalformedShim { member, .. } if member == "settings.colour_profile"
        )),
        "{notes:?}"
    );
    // A malformed member is treated as absent, but it is present: no default-profile note for it.
    assert!(!notes.iter().any(|n| matches!(
        n,
        DeriveNote::DefaultWireProfile {
            member: "colour_profile"
        }
    )));
    let e = props.wires.values().next().expect("one wire");
    let colour = e.colour.as_ref().expect("a colour");
    assert!(matches!(colour.source, PropSource::BuiltIn { .. }), "{colour:?}");
}

#[test]
fn nested_junction_chain_fr_con_041() {
    // Doc 08 §3.2: a chain junction Jr (feed 0x2001 at S) whose third arm 0x2003 enters a second chain
    // junction Jn (its feed arm); src -> d1 -> d2 -> d3, every non-feed arm one wire.
    let mut fx = base();
    for f in 0x1001..=0x1004 {
        func(&mut fx, f, P1);
    }
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001), Ep::J(0x3001), Value::Null);
    conn(&mut fx, 0x2002, Some(P1), Ep::J(0x3001), cp(0x1002), Value::Null);
    conn(
        &mut fx,
        0x2003,
        Some(P1),
        Ep::J(0x3001),
        Ep::J(0x3002),
        Value::Null,
    );
    conn(&mut fx, 0x2004, Some(P1), Ep::J(0x3002), cp(0x1003), Value::Null);
    conn(&mut fx, 0x2005, Some(P1), Ep::J(0x3002), cp(0x1004), Value::Null);
    junction(&mut fx, 0x3001, P1, &[0x2001, 0x2002, 0x2003]);
    fx.with("wiring", json!("chain"));
    junction(&mut fx, 0x3002, P1, &[0x2003, 0x2004, 0x2005]);
    fx.with("wiring", json!("chain"));
    let m = fx.build();
    let mut q = Q::new(&m);
    let pid = only_potential(&q);
    let wires = q.get::<PhysicalWiresOf>(pid);
    assert!(!has_note(&mut q, "EW-POT-011"), "{:?}", q.diagnostics().notes());
    type Row = (Id<Connection>, [CpRef; 2], Vec<Id<Connection>>, Vec<Id<Junction>>);
    let got: Vec<Row> = wires
        .wires
        .iter()
        .map(|w| {
            (
                w.id,
                w.ends.map(|e| e.cp),
                w.constituents.clone(),
                w.junctions.clone(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            // src -> d1: carried by the arm that touches d1.
            (
                id(0x2002),
                [cpref(0x1001), cpref(0x1002)],
                vec![id(0x2001), id(0x2002)],
                vec![id(0x3001)]
            ),
            // d1 -> d2: from d1 through Jr into Jn.
            (
                id(0x2004),
                [cpref(0x1002), cpref(0x1003)],
                vec![id(0x2002), id(0x2003), id(0x2004)],
                vec![id(0x3001), id(0x3002)]
            ),
            // d2 -> d3: inside Jn.
            (
                id(0x2005),
                [cpref(0x1003), cpref(0x1004)],
                vec![id(0x2004), id(0x2005)],
                vec![id(0x3002)]
            ),
        ]
    );
}
