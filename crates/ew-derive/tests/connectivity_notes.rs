// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! Connectivity notes and the linear partner lookup (WP-S1.07, cycle-7 A review findings F6, F7, F8, F13).
//!
//! - `star_group_of_400_members_derives`: one source and 399 targets derive through every query; the 3,999-target
//!   size, `star_group_of_4000_members_is_linear`, is a perf test (ignored, milestone deliveries only).
//! - `star_source_with_two_drawn_connections_is_no_hub_and_noted`: EW-POT-011, the wires end open.
//! - `star_source_without_a_drawn_connection_is_no_hub_and_noted`: EW-POT-011 with zero connections.
//! - `star_hub_with_one_feed_raises_no_note`: the positive control of the two tests above.
//! - `two_sources_without_star_flag_pick_the_lowest_source_id`: EW-CORE-051, never a target.
//! - `ip_without_a_connection_point_is_noted`: EW-CORE-051 for an IP whose definition has no CP.
//! - `rail_bonding_joins_pe_cps_only`: only the PE CPs of the rail-bonded terminals of a location join.
//! - `rail_bonded_terminal_without_pe_cp_is_noted`: EW-TERM-005, no rail bond.
//! - `unlocated_rail_bonded_terminals_do_not_bond`: EW-POT-008, no rail bond, in a project with two own locations.
//! - `unlocated_rail_bonded_terminals_join_the_only_own_location` and `..._join_the_implicit_main_location`:
//!   cycle 33 (G-PP ruling #20), one own location or none: the rail joins them, no EW-POT-008. A location an
//!   external reference names is own while an own device or page uses it (cycle 33 review finding 7).
//! - `scoped_pairing_without_the_node_pairs_project_wide_and_notes`: F13, pages without a location node.
//! - `scoped_pairing_keeps_locations_apart`: F13, different location nodes do not pair, the same node does.

mod common;

use std::collections::BTreeSet;

use common::{Fx, id, uid};
use ew_derive::{
    DeriveNote, EdgeKind, EndKind, PhysicalWiresOf, PotentialFlow, PotentialPartition, Q, RailLocation,
    StitchGraph, StitchGraphQ, WirePropsOf,
};
use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::Function;
use serde_json::{Value, json};

const FNDEF: u128 = 0x500;
const FNDEF_IP: u128 = 0x501;
const FNDEF_PE: u128 = 0x502;
const FNDEF_RAIL: u128 = 0x503;
const FNDEF_NO_CP: u128 = 0x504;
const SYM: u128 = 0x510;
const SYM_IP: u128 = 0x511;
const SYM_PE: u128 = 0x512;
const SYM_RAIL: u128 = 0x513;
const P1: u128 = 0x100;
const P2: u128 = 0x101;
const P3: u128 = 0x102;
const P4: u128 = 0x103;
const P5: u128 = 0x104;
const LOC_A: u128 = 0x6001;
const LOC_B: u128 = 0x6002;
const STRIP: u128 = 0x5000;

/// An endpoint of a connection.
#[derive(Clone, Copy)]
enum Ep {
    Cp(u128, u16),
}

impl Ep {
    fn json(self) -> Value {
        match self {
            Ep::Cp(f, i) => json!({"cp": [uid(f), i]}),
        }
    }
}

fn cp(f: u128) -> Ep {
    Ep::Cp(f, 0)
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

/// A connection `n` between `a` and `b`, on `page`.
fn wire(fx: &mut Fx, n: u128, page: u128, a: Ep, b: Ep) {
    fx.push(json!({
        "$k": "connection", "id": uid(n), "a": a.json(), "b": b.json(), "page": uid(page),
    }));
}

/// The codes of the connectivity notes whose detail contains `needle`.
fn notes_with(q: &mut Q, code: &str, needle: &str) -> Vec<(String, String)> {
    q.diagnostics()
        .notes()
        .into_iter()
        .filter_map(|n| match n {
            DeriveNote::Connectivity {
                code: c,
                subject,
                detail,
            } if c == code && detail.contains(needle) => Some((subject, detail)),
            _ => None,
        })
        .collect()
}

/// One source interruption point and `targets` targets derive through every query: the partner lookup, the
/// partition, the physical wires and their properties.
fn star_group_derives(targets: u128) {
    const SOURCE_FN: u128 = 0x1000;
    const SOURCE_IP: u128 = 0x1100;
    let plain = |i: u128| 0x100_0000 + i;
    let target = |i: u128| 0x200_0000 + i;
    let mut fx = base();
    func(&mut fx, SOURCE_FN, P1);
    ip(&mut fx, SOURCE_IP, P1, "BIG", "source");
    wire(&mut fx, 0x300_0000, P1, cp(SOURCE_FN), cp(SOURCE_IP));
    for i in 0..targets {
        let page = if i % 2 == 0 { P1 } else { P2 };
        func(&mut fx, plain(i), page);
        ip(&mut fx, target(i), page, "BIG", "target");
        wire(&mut fx, 0x300_0001 + i, page, cp(target(i)), cp(plain(i)));
    }
    let m = fx.build();
    let q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    let source: Id<Function> = id(SOURCE_IP);
    assert_eq!(g.ip_partners(source).len(), targets as usize);
    assert!(
        g.ip_partners(source).windows(2).all(|w| w[0] < w[1]),
        "sorted and unique"
    );
    for i in 0..targets {
        assert_eq!(g.ip_partners(id(target(i))), [source]);
    }
    assert!(
        g.ip_partners(id(SOURCE_FN)).is_empty(),
        "a plain function has no partners"
    );
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potential_count(), 1);
    let pid = part.potentials().next().unwrap().id;
    // A star group with a drawn feed is a hub (doc 08 §3.8): each target arm resolves to one wire that
    // shares the feed arm.
    let wires = q.get::<PhysicalWiresOf>(pid);
    assert_eq!(wires.wires.len(), targets as usize, "{:?}", wires.notes);
    assert!(
        wires
            .wires
            .iter()
            .all(|w| w.ends.iter().all(|e| e.kind == EndKind::Device))
    );
    let ids: BTreeSet<_> = wires.wires.iter().map(|w| w.id).collect();
    assert_eq!(ids.len(), targets as usize);
    let props = q.get::<WirePropsOf>(pid);
    assert_eq!(props.wires.len(), targets as usize);
    let _flow = q.get::<PotentialFlow>(pid);
}

/// The per-gate size of [`star_group_derives`]: one source and 399 targets.
#[test]
fn star_group_of_400_members_derives() {
    star_group_derives(399);
}

/// The size that shows the partner lookup is linear: one source and 3,999 targets.
#[test]
#[ignore = "perf: release-profile timing, milestone deliveries only"]
fn star_group_of_4000_members_is_linear() {
    star_group_derives(3_999);
}

/// Source IP 0x1101 with `source_ends` drawn connections to functions, and two targets, each wired to one
/// function: a star group of three.
fn star_of_three(source_ends: &[u128]) -> Fx {
    let mut fx = base();
    for f in [0x1001, 0x1002, 0x1003, 0x1004] {
        func(&mut fx, f, P1);
    }
    ip(&mut fx, 0x1101, P1, "S", "source");
    ip(&mut fx, 0x1102, P2, "S", "target");
    ip(&mut fx, 0x1103, P2, "S", "target");
    for (k, f) in source_ends.iter().enumerate() {
        wire(&mut fx, 0x2001 + k as u128, P1, cp(*f), cp(0x1101));
    }
    wire(&mut fx, 0x2011, P2, cp(0x1102), cp(0x1003));
    wire(&mut fx, 0x2012, P2, cp(0x1103), cp(0x1004));
    fx
}

#[test]
fn star_source_with_two_drawn_connections_is_no_hub_and_noted() {
    let m = star_of_three(&[0x1001, 0x1002]).build();
    let mut q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potential_count(), 1);
    let wires = q.get::<PhysicalWiresOf>(part.potentials().next().unwrap().id);
    let found = notes_with(&mut q, "EW-POT-011", "2 drawn connections");
    assert_eq!(found.len(), 1, "{:?}", q.diagnostics().notes());
    assert_eq!(found[0].0, uid(0x1101));
    assert!(found[0].1.contains("group S"), "{}", found[0].1);
    assert!(found[0].1.contains("end open"), "{}", found[0].1);
    assert!(
        wires
            .wires
            .iter()
            .any(|w| w.ends.iter().any(|e| e.kind == EndKind::Open)),
        "{:?}",
        wires.wires
    );
}

#[test]
fn star_source_without_a_drawn_connection_is_no_hub_and_noted() {
    let m = star_of_three(&[]).build();
    let mut q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    let pid = part
        .potential_at(EndpointKey::Cp(id(0x1003), 0))
        .expect("potential")
        .id;
    let _ = q.get::<PhysicalWiresOf>(pid);
    let found = notes_with(&mut q, "EW-POT-011", "0 drawn connections");
    assert_eq!(found.len(), 1, "{:?}", q.diagnostics().notes());
    assert_eq!(found[0].0, uid(0x1101));
}

#[test]
fn star_hub_with_one_feed_raises_no_note() {
    let m = star_of_three(&[0x1001]).build();
    let mut q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    let _ = q.get::<PhysicalWiresOf>(part.potentials().next().unwrap().id);
    assert!(
        notes_with(&mut q, "EW-POT-011", "").is_empty(),
        "{:?}",
        q.diagnostics().notes()
    );
}

#[test]
fn two_sources_without_star_flag_pick_the_lowest_source_id() {
    // The lowest ID is a target: it must not become the star source.
    let mut fx = base();
    for f in [0x1001, 0x1002, 0x1003] {
        func(&mut fx, f, P1);
    }
    ip(&mut fx, 0x1100, P1, "S", "target");
    ip(&mut fx, 0x1102, P2, "S", "source");
    ip(&mut fx, 0x1101, P2, "S", "source");
    wire(&mut fx, 0x2001, P1, cp(0x1001), cp(0x1100));
    wire(&mut fx, 0x2002, P2, cp(0x1002), cp(0x1101));
    wire(&mut fx, 0x2003, P2, cp(0x1003), cp(0x1102));
    let m = fx.build();
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert_eq!(g.ip_groups.len(), 1);
    assert_eq!(g.ip_groups[0].source, Some(id(0x1101)));
    let found = notes_with(&mut q, "EW-CORE-051", "2 source points and no star_source");
    assert_eq!(found.len(), 1, "{:?}", q.diagnostics().notes());
    assert_eq!(found[0].0, "interruption point S");
    assert!(found[0].1.contains("lowest ID source"), "{}", found[0].1);
}

#[test]
fn ip_without_a_connection_point_is_noted() {
    let mut fx = base();
    fx.fndef(FNDEF_NO_CP, ["Misc", "Misc", "NoCp"], &[]);
    fx.symbol(0x514, FNDEF_NO_CP, json!("origin"), &[]);
    fx.push(json!({
        "$k": "function", "id": uid(0x1101), "def": uid(FNDEF_NO_CP),
        "kind": {"ip": {"name": "Z", "direction": "source"}},
    }));
    fx.rep(0x11_101, P1, 0x514, &[0x1101], 0, 0);
    let m = fx.build();
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert!(g.ip_groups.is_empty());
    let found = notes_with(&mut q, "EW-CORE-051", "no connection point");
    assert_eq!(found.len(), 1, "{:?}", q.diagnostics().notes());
    assert_eq!(found[0].0, format!("interruption point Z ({})", uid(0x1101)));
    assert!(found[0].1.contains("pairs with nothing"), "{}", found[0].1);
}

/// A strip, the terminal FnDefs (PE role, rail-foot clamp) and their symbols, and the one location A.
fn terminal_base() -> Fx {
    let mut fx = base();
    // CP 1 is "PE" by role in the first definition and by rail-foot clamp in the second.
    fx.lib_item(
        "fndef",
        FNDEF_PE,
        json!({"meta": {}, "category": ["Misc", "Misc", "Pe"], "contact_kind": "pole", "cps": [
            {"idx": 0, "designation": "1"},
            {"idx": 1, "designation": "PE", "potential_role": "pe"},
        ]}),
    );
    fx.lib_item(
        "fndef",
        FNDEF_RAIL,
        json!({"meta": {}, "category": ["Misc", "Misc", "Rail"], "contact_kind": "pole", "cps": [
            {"idx": 0, "designation": "1"},
            {"idx": 1, "designation": "PE", "clamp": "rail_foot"},
        ]}),
    );
    fx.symbol(
        SYM_PE,
        FNDEF_PE,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)],
    );
    fx.symbol(
        SYM_RAIL,
        FNDEF_RAIL,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)],
    );
    fx.device(STRIP, "X", Some(1)).with("strip", json!({}));
    fx.node(LOC_A, "location", "A", None);
    fx
}

/// A rail-bonded terminal device `dev` with function `f` of `def` drawn with `symbol` on P1, in
/// `location` when given.
fn terminal(fx: &mut Fx, dev: u128, f: u128, def: u128, symbol: u128, location: Option<u128>) {
    fx.device(dev, "X", None).with(
        "terminal",
        json!({"strip": uid(STRIP), "seq": dev as u32, "designation": dev.to_string(), "rail_bonded": true}),
    );
    if let Some(loc) = location {
        fx.with("structure", json!({"location": uid(loc)}));
    }
    fx.function(f, Some(dev), def, true);
    fx.rep(f + 0x10_000, P1, symbol, &[f], 0, f as i64 * 1_000_000);
}

fn rail_bonds(g: &StitchGraph) -> Vec<(usize, usize, RailLocation)> {
    g.edges
        .iter()
        .filter_map(|e| match e.kind {
            EdgeKind::RailBond { location } => Some((e.a, e.b, location)),
            _ => None,
        })
        .collect()
}

#[test]
fn rail_bonding_joins_pe_cps_only() {
    const T1: u128 = 0x4001;
    const T2: u128 = 0x4002;
    let mut fx = terminal_base();
    // T1's PE CP has the PE role; T2's has the rail-foot clamp. Their CP 0 is neither.
    terminal(&mut fx, T1, 0x1001, FNDEF_PE, SYM_PE, Some(LOC_A));
    terminal(&mut fx, T2, 0x1002, FNDEF_RAIL, SYM_RAIL, Some(LOC_A));
    for f in [0x1003, 0x1004, 0x1005, 0x1006] {
        func(&mut fx, f, P1);
    }
    wire(&mut fx, 0x2001, P1, Ep::Cp(0x1001, 0), cp(0x1003));
    wire(&mut fx, 0x2002, P1, Ep::Cp(0x1001, 1), cp(0x1004));
    wire(&mut fx, 0x2003, P1, Ep::Cp(0x1002, 0), cp(0x1005));
    wire(&mut fx, 0x2004, P1, Ep::Cp(0x1002, 1), cp(0x1006));
    let m = fx.build();
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    let part = q.get::<PotentialPartition>(());
    let of = |f: u128, idx: u16| {
        part.potential_at(EndpointKey::Cp(id(f), idx))
            .expect("a potential")
            .id
    };
    // The PE CPs and what hangs on them are one potential ...
    let pe = of(0x1001, 1);
    for (f, idx) in [(0x1004, 0), (0x1002, 1), (0x1006, 0)] {
        assert_eq!(of(f, idx), pe);
    }
    // ... the other CPs stay in their own.
    let (a, b) = (of(0x1001, 0), of(0x1002, 0));
    assert_ne!(a, pe);
    assert_ne!(b, pe);
    assert_ne!(a, b);
    assert_eq!(of(0x1003, 0), a);
    assert_eq!(of(0x1005, 0), b);
    assert_eq!(part.potential_count(), 3);
    let bonds = rail_bonds(&g);
    assert_eq!(bonds.len(), 1, "{bonds:?}");
    assert_eq!(bonds[0].2, RailLocation::Node(id(LOC_A)));
    assert!(notes_with(&mut q, "EW-TERM-005", "").is_empty());
    assert!(notes_with(&mut q, "EW-POT-008", "").is_empty());
}

#[test]
fn rail_bonded_terminal_without_pe_cp_is_noted() {
    const T1: u128 = 0x4001;
    const T2: u128 = 0x4002;
    let mut fx = terminal_base();
    // T1 has a PE CP; T2 (a plain FnDef) has none, so it joins nothing.
    terminal(&mut fx, T1, 0x1001, FNDEF_PE, SYM_PE, Some(LOC_A));
    terminal(&mut fx, T2, 0x1002, FNDEF, SYM, Some(LOC_A));
    for f in [0x1003, 0x1004] {
        func(&mut fx, f, P1);
    }
    wire(&mut fx, 0x2001, P1, Ep::Cp(0x1001, 1), cp(0x1003));
    wire(&mut fx, 0x2002, P1, Ep::Cp(0x1002, 0), cp(0x1004));
    let m = fx.build();
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert!(rail_bonds(&g).is_empty(), "{:?}", rail_bonds(&g));
    let found = notes_with(&mut q, "EW-TERM-005", "no PE CP");
    assert_eq!(found.len(), 1, "{:?}", q.diagnostics().notes());
    assert_eq!(found[0].0, uid(T2));
    assert!(found[0].1.contains("joins no location PE node"), "{}", found[0].1);
}

/// Two unlocated rail-bonded PE terminals T1 and T2 (each PE CP wired to its own plain function) on `fx`.
fn two_unlocated_terminals(mut fx: Fx) -> ew_model::Model {
    terminal(&mut fx, 0x4001, 0x1001, FNDEF_PE, SYM_PE, None);
    terminal(&mut fx, 0x4002, 0x1002, FNDEF_PE, SYM_PE, None);
    for f in [0x1003, 0x1004] {
        func(&mut fx, f, P1);
    }
    wire(&mut fx, 0x2001, P1, Ep::Cp(0x1001, 1), cp(0x1003));
    wire(&mut fx, 0x2002, P1, Ep::Cp(0x1002, 1), cp(0x1004));
    fx.build()
}

/// Amended by cycle 33 (G-PP ruling #20): with one own location (A), the unlocated terminals join A's rail; an
/// external reference's location (B) is not an own location. No EW-POT-008.
#[test]
fn unlocated_rail_bonded_terminals_join_the_only_own_location() {
    let mut fx = terminal_base();
    fx.node(LOC_B, "location", "B", None);
    fx.push(
        json!({"$k": "external_ref", "id": uid(0x6100), "location": uid(LOC_B), "device": "JE",
        "connection": "PE"}),
    );
    let m = two_unlocated_terminals(fx);
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    let bonds = rail_bonds(&g);
    assert_eq!(bonds.len(), 1, "{bonds:?}");
    assert_eq!(bonds[0].2, RailLocation::Node(id(LOC_A)));
    let part = q.get::<PotentialPartition>(());
    assert_eq!(
        part.potential_count(),
        1,
        "both PE CPs and their wires are one potential"
    );
    assert!(notes_with(&mut q, "EW-POT-008", "").is_empty());
}

/// Cycle 33 review finding 7: a location that an external reference names is still own when an own device sits in
/// it. With A and B (named by an external reference, holding the own device -K1) both own, the unlocated terminals
/// join no rail and are noted (EW-POT-008), as with two own locations.
#[test]
fn a_location_named_by_an_external_reference_and_used_by_an_own_device_is_own() {
    let mut fx = terminal_base();
    fx.node(LOC_B, "location", "B", None);
    fx.push(
        json!({"$k": "external_ref", "id": uid(0x6100), "location": uid(LOC_B), "device": "JE",
        "connection": "PE"}),
    );
    fx.device(0x4100, "K", Some(1))
        .with("structure", json!({"location": uid(LOC_B)}));
    let m = two_unlocated_terminals(fx);
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert!(rail_bonds(&g).is_empty(), "{:?}", rail_bonds(&g));
    assert_eq!(q.get::<PotentialPartition>(()).potential_count(), 2);
    let found = notes_with(&mut q, "EW-POT-008", "2 own locations");
    let subjects: BTreeSet<String> = found.into_iter().map(|(s, _)| s).collect();
    assert_eq!(subjects, BTreeSet::from([uid(0x4001), uid(0x4002)]));
}

/// Amended by cycle 33 (G-PP ruling #20): with no location node at all, the unlocated terminals join the implicit
/// main location's rail.
#[test]
fn unlocated_rail_bonded_terminals_join_the_implicit_main_location() {
    let mut fx = terminal_base();
    fx.records.retain(|r| r["$k"] != "structure_node");
    let m = two_unlocated_terminals(fx);
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    let bonds = rail_bonds(&g);
    assert_eq!(bonds.len(), 1, "{bonds:?}");
    assert_eq!(bonds[0].2, RailLocation::Main);
    assert_eq!(q.get::<PotentialPartition>(()).potential_count(), 1);
    assert!(notes_with(&mut q, "EW-POT-008", "").is_empty());
}

/// With two own locations (A and B), an unlocated terminal joins no rail and is noted (EW-POT-008).
#[test]
fn unlocated_rail_bonded_terminals_do_not_bond() {
    const T1: u128 = 0x4001;
    const T2: u128 = 0x4002;
    let mut fx = terminal_base();
    fx.node(LOC_B, "location", "B", None);
    terminal(&mut fx, T1, 0x1001, FNDEF_PE, SYM_PE, None);
    terminal(&mut fx, T2, 0x1002, FNDEF_PE, SYM_PE, None);
    for f in [0x1003, 0x1004] {
        func(&mut fx, f, P1);
    }
    wire(&mut fx, 0x2001, P1, Ep::Cp(0x1001, 1), cp(0x1003));
    wire(&mut fx, 0x2002, P1, Ep::Cp(0x1002, 1), cp(0x1004));
    let m = fx.build();
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert!(rail_bonds(&g).is_empty(), "{:?}", rail_bonds(&g));
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potential_count(), 2);
    let found = notes_with(&mut q, "EW-POT-008", "no location");
    let subjects: BTreeSet<String> = found.into_iter().map(|(s, _)| s).collect();
    assert_eq!(subjects, BTreeSet::from([uid(T1), uid(T2)]));
    assert!(notes_with(&mut q, "EW-TERM-005", "").is_empty());
}

/// Two points `X` on `p` and `r`, each wired to its own plain function; `ip_pairing` scope `scope`.
fn scoped(scope: &str, pages: [u128; 2], fx: &mut Fx) {
    fx.project_set(
        "settings",
        json!({"identifier_set": "EN81346-2:2009", "ip_pairing": {"mode": "star", "scope": scope}}),
    );
    func(fx, 0x1001, pages[0]);
    func(fx, 0x1002, pages[1]);
    ip(fx, 0x1101, pages[0], "X", "source");
    ip(fx, 0x1102, pages[1], "X", "target");
    wire(fx, 0x2001, pages[0], cp(0x1001), cp(0x1101));
    wire(fx, 0x2002, pages[1], cp(0x1002), cp(0x1102));
}

#[test]
fn scoped_pairing_without_the_node_pairs_project_wide_and_notes() {
    for scope in ["location", "function"] {
        let mut fx = base();
        scoped(scope, [P1, P2], &mut fx);
        let m = fx.build();
        let mut q = Q::new(&m);
        let g = q.get::<StitchGraphQ>(());
        assert_eq!(g.ip_groups.len(), 1, "{scope}");
        assert_eq!(g.ip_groups[0].scope, None);
        assert_eq!(g.ip_groups[0].pairs.len(), 1);
        let aspect = if scope == "location" {
            "Location"
        } else {
            "Function"
        };
        let found = notes_with(&mut q, "EW-CORE-051", &format!("scope is {aspect}"));
        assert_eq!(found.len(), 2, "{scope}: {:?}", q.diagnostics().notes());
        let subjects: BTreeSet<String> = found.iter().map(|(s, _)| s.clone()).collect();
        assert_eq!(
            subjects,
            BTreeSet::from([
                format!("interruption point X ({})", uid(0x1101)),
                format!("interruption point X ({})", uid(0x1102)),
            ])
        );
        assert!(
            found[0].1.contains("paired in the project-wide group"),
            "{}",
            found[0].1
        );
        assert!(notes_with(&mut q, "EW-CORE-051", "no partner").is_empty());
    }
}

#[test]
fn scoped_pairing_keeps_locations_apart() {
    // Different location nodes: the same-named points stay apart (two unpaired notes), no scope note.
    let mut fx = base();
    fx.node(LOC_A, "location", "A", None);
    fx.node(LOC_B, "location", "B", None);
    fx.page(P3, 3, None, "multi_line", &[("location", LOC_A)], "three");
    fx.page(P4, 4, None, "multi_line", &[("location", LOC_B)], "four");
    scoped("location", [P3, P4], &mut fx);
    let m = fx.build();
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert_eq!(g.ip_groups.len(), 2);
    assert!(
        g.ip_groups
            .iter()
            .all(|g| g.pairs.is_empty() && g.scope.is_some())
    );
    assert_eq!(notes_with(&mut q, "EW-CORE-051", "no partner").len(), 2);
    assert!(notes_with(&mut q, "EW-CORE-051", "scope is").is_empty());

    // The same location node: the points pair.
    let mut fx = base();
    fx.node(LOC_A, "location", "A", None);
    fx.page(P3, 3, None, "multi_line", &[("location", LOC_A)], "three");
    fx.page(P5, 5, None, "multi_line", &[("location", LOC_A)], "five");
    scoped("location", [P3, P5], &mut fx);
    let m = fx.build();
    let mut q = Q::new(&m);
    let g = q.get::<StitchGraphQ>(());
    assert_eq!(g.ip_groups.len(), 1);
    assert_eq!(g.ip_groups[0].scope, Some(id(LOC_A)));
    assert_eq!(g.ip_groups[0].pairs.len(), 1);
    assert!(
        q.diagnostics()
            .notes()
            .iter()
            .all(|n| n.code() != Some("EW-CORE-051")),
        "{:?}",
        q.diagnostics().notes()
    );
}
