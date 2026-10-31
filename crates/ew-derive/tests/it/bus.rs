// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus queries of WP-S1.21 D11 (doc 10 §3.8, algorithm A5): `bus_segments(pnet)` and `net_members(lnet)`, on a
//! PROFIBUS line of four ports `F1 — F2 — F3 — F4` (cable W1 between F1 and F2, W2 between F3 and F4, F3 a passive
//! plug on the physical net only) and a lone port F6 of the same net.

// The oracle test edits a snapshot with the raw op layer, as the other incremental tests do.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::{Fx, id, uid};
use ew_derive::{BusSegment, BusSegments, NetMaster, NetMembers, NetMembersOf, Q, SegmentMember};
use ew_schema::{Device, Function, LogicalNet, PhysicalNet};
use serde_json::{Value, json};

const PAGE: u128 = 0x203;
const FNDEF: u128 = 0x500;
const PNET: u128 = 0x40;
const LNET: u128 = 0x41;
const F1: u128 = 0x301;
const F2: u128 = 0x302;
const F3: u128 = 0x303;
const F4: u128 = 0x304;
const F5: u128 = 0x305;
const F6: u128 = 0x306;
const OTHER_A: u128 = 0x307;
const OTHER_B: u128 = 0x308;
const W1: u128 = 0x401;
const W2: u128 = 0x402;

fn f(n: u128) -> SegmentMember {
    SegmentMember::Function(id(n))
}

fn w(n: u128) -> SegmentMember {
    SegmentMember::Cable(id(n))
}

fn bus(fx: &mut Fx, n: u128, a: u128, b: u128, cable: Option<u128>) {
    let mut kind = json!({"bus": {}});
    if let Some(c) = cable {
        kind["bus"]["cable"] = json!(uid(c));
    }
    fx.push(json!({
        "$k": "connection", "id": uid(n), "page": uid(PAGE),
        "a": {"cp": [uid(a), 1]}, "b": {"cp": [uid(b), 0]}, "kind": kind,
    }));
}

/// A device `n + 0x1000` with function `n`, its bus port `port` when given.
fn port_function(fx: &mut Fx, n: u128, port: Option<Value>) {
    fx.device(n + 0x1000, "K", Some(n as u32));
    fx.function(n, Some(n + 0x1000), FNDEF, true);
    if let Some(p) = port {
        fx.with("bus_port", p);
    }
}

fn port(role: &str, lnet: bool) -> Value {
    let mut p = json!({
        "interface": "DP", "bus": "profibus_dp", "role": role, "address": "tbd", "physical_net": uid(PNET),
    });
    if lnet {
        p["logical_net"] = json!(uid(LNET));
    }
    p
}

/// The line; `master` is the logical net's master function.
fn fixture(master: Option<u128>) -> Fx {
    let mut fx = Fx::new();
    fx.page(PAGE, 3, None, "multi_line", &[], "Bus");
    fx.fndef(FNDEF, ["Connector", "Bus", "Port"], &[(0, "1"), (1, "2")]);
    fx.push(json!({"$k": "physical_net", "id": uid(PNET), "name": "DP", "medium": "copper"}));
    let mut lnet = json!({"$k": "logical_net", "id": uid(LNET), "bus": "profibus_dp", "name": "DP"});
    if let Some(m) = master {
        lnet["master"] = json!({"cp": [uid(m), 0]});
    }
    fx.push(lnet);
    port_function(&mut fx, F1, Some(port("slave", true)));
    port_function(&mut fx, F2, Some(port("slave", true)));
    port_function(&mut fx, F3, Some(port("passive", false)));
    port_function(&mut fx, F4, Some(port("master", true)));
    port_function(&mut fx, F6, Some(port("slave", true)));
    // two functions on a bus connection but on no port of the net: no segment of it
    port_function(&mut fx, OTHER_A, None);
    port_function(&mut fx, OTHER_B, None);
    for (n, tag) in [(W1, 1), (W2, 2)] {
        fx.device(n, "W", Some(tag));
    }
    bus(&mut fx, 0x501, F1, F2, Some(W1));
    bus(&mut fx, 0x502, F2, F3, None);
    bus(&mut fx, 0x503, F3, F4, Some(W2));
    bus(&mut fx, 0x504, OTHER_A, OTHER_B, None);
    fx
}

fn segments(fx: &Fx) -> Vec<BusSegment> {
    let m = fx.build();
    Q::new(&m).get::<BusSegments>(id(PNET)).as_ref().clone()
}

/// A5: the master end starts the segment; the walk lists each cable before the node it reaches; the lone port F6 is
/// a segment of its own; the connection of the two functions without a port is no segment.
#[test]
fn a_line_starts_at_its_master_end() {
    let s = segments(&fixture(Some(F4)));
    assert_eq!(s.len(), 2, "{s:#?}");
    assert_eq!(s[0].members, [f(F4), w(W2), f(F3), f(F2), w(W1), f(F1)]);
    assert_eq!(s[0].ends, [id::<Function>(F4), id(F1)]);
    assert!(s[0].external_ends.is_empty());
    assert_eq!(s[0].logical_net, Some(id::<LogicalNet>(LNET)));
    assert!(!s[0].branched);
    assert_eq!(s[1].members, [f(F6)]);
    assert_eq!(s[1].ends, [id::<Function>(F6)]);
}

/// A5: without a master the end first in `order_hint` starts, else the smallest ID.
#[test]
fn without_a_master_the_order_hint_then_the_id_choose_the_start() {
    let s = segments(&fixture(None));
    assert_eq!(s[0].members, [f(F1), w(W1), f(F2), f(F3), w(W2), f(F4)]);
    let mut fx = fixture(None);
    fx.record(PNET)["order_hint"] = json!([uid(F4)]);
    let s = segments(&fx);
    assert_eq!(s[0].members, [f(F4), w(W2), f(F3), f(F2), w(W1), f(F1)]);
}

/// A5: a stub at F2 makes the segment branched; its end is a third end.
#[test]
fn a_stub_makes_the_segment_branched() {
    let mut fx = fixture(Some(F4));
    port_function(&mut fx, F5, Some(port("slave", true)));
    bus(&mut fx, 0x505, F2, F5, None);
    let s = segments(&fx);
    assert!(s[0].branched);
    assert_eq!(s[0].ends.len(), 3, "{:?}", s[0].ends);
    assert_eq!(s[0].ends[0], id::<Function>(F4));
    assert!(s[0].members.contains(&f(F5)));
}

/// `net_members(lnet)`: the ports naming the net in ID order (the passive plug F3 names no logical net) and the
/// master.
#[test]
fn net_members_are_the_ports_of_the_net_and_its_master() {
    let m = fixture(Some(F4)).build();
    let members = Q::new(&m).get::<NetMembersOf>(id(LNET));
    assert_eq!(
        *members,
        NetMembers {
            ports: vec![id(F1), id(F2), id(F4), id(F6)],
            master: Some(NetMaster::Port(id(F4))),
        }
    );
    let m = fixture(None).build();
    assert_eq!(Q::new(&m).get::<NetMembersOf>(id(LNET)).master, None);
}

type Edit = Box<dyn Fn(&ew_model::Model) -> Vec<ew_model::Op>>;

fn update<T>(model: &ew_model::Model, n: u128, f: impl FnOnce(&mut Value)) -> ew_model::Op
where
    T: ew_model::Entity + serde::Serialize + serde::de::DeserializeOwned,
{
    let key = T::key(id(n));
    let before = model.row(&key).expect("a row");
    let mut v = serde_json::to_value(&**before.get::<T>().expect("typed")).expect("serialises");
    f(&mut v);
    let after: T = serde_json::from_value(v).expect("parses");
    ew_model::Op::update(before, ew_model::AnyRow::of(after))
}

fn read_all(q: &Q) -> (Vec<BusSegment>, NetMembers) {
    (
        q.get::<BusSegments>(id::<PhysicalNet>(PNET)).as_ref().clone(),
        q.get::<NetMembersOf>(id::<LogicalNet>(LNET)).as_ref().clone(),
    )
}

/// The memoised bus families equal a cold run after each edit, with the oracle on: a termination set (no segment
/// change), the master moved, a connection deleted (the line splits), a port moved off the net, a cable device
/// deleted (the connection keeps no cable).
#[test]
fn memoised_bus_families_equal_a_cold_run_across_edits() {
    use ew_derive::MemoStore;
    use ew_memo::Oracle;
    use ew_model::{Entity, Op};
    use ew_schema::Connection;

    let m0 = fixture(Some(F4)).build();
    let store = MemoStore::new(&m0);
    store.set_oracle(Oracle::Every);
    assert_eq!(read_all(&store.query()), read_all(&Q::new(&m0)));
    let edits: Vec<(&str, Edit)> = vec![
        (
            "termination on",
            Box::new(|m| {
                vec![update::<Function>(m, F3, |v| {
                    v["bus_port"]["termination"] = json!("on")
                })]
            }),
        ),
        (
            "master moves to F1",
            Box::new(|m| {
                vec![update::<LogicalNet>(m, LNET, |v| {
                    v["master"] = json!({"cp": [uid(F1), 0]})
                })]
            }),
        ),
        (
            "the connection F2-F3 is deleted",
            Box::new(|m| {
                let key = Connection::key(id(0x502));
                vec![Op::delete(m.row(&key).expect("a row"))]
            }),
        ),
        (
            "F6 leaves the net",
            Box::new(|m| {
                vec![update::<Function>(m, F6, |v| {
                    v["bus_port"]
                        .as_object_mut()
                        .expect("a port")
                        .remove("physical_net");
                    v["bus_port"]
                        .as_object_mut()
                        .expect("a port")
                        .remove("logical_net");
                })]
            }),
        ),
        (
            "the cable W2 is deleted with its reference",
            Box::new(|m| {
                vec![
                    update::<Connection>(m, 0x503, |v| v["kind"] = json!({"bus": {}})),
                    Op::delete(m.row(&Device::key(id(W2))).expect("a row")),
                ]
            }),
        ),
    ];
    let mut model = m0;
    for (name, edit) in &edits {
        let ops = edit(&model);
        let keys: Vec<ew_model::AnyKey> = ops.iter().map(Op::key).collect();
        model = model
            .apply_unchecked(&ops, model.rev().next())
            .expect("the ops apply");
        store.advance(&model, &keys);
        assert_eq!(
            read_all(&store.query()),
            read_all(&Q::new(&model)),
            "after {name}"
        );
        assert!(
            store.take_mismatches().is_empty(),
            "oracle mismatches after {name}"
        );
    }
    // the split line: F1-F2 starts at the master F1, F3-F4 at its smallest end
    let (s, members) = read_all(&store.query());
    assert_eq!(s.len(), 2, "{s:#?}");
    assert_eq!(s[0].members, [f(F1), w(W1), f(F2)]);
    assert_eq!(s[1].members, [f(F3), f(F4)]);
    assert_eq!(members.ports, [id::<Function>(F1), id(F2), id(F4)]);
    assert_eq!(members.master, Some(NetMaster::Port(id(F1))));
}
