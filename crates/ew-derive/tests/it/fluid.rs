// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid side of WP-S1.20: the fluid partition `fluid_nets` (D12), which joins fluid lines and their junctions
//! and leaves every electrical net as it is.

use crate::common;

use common::{Fx, id, uid};
use ew_derive::{FluidNets, NetId, PotentialPartition, Q};
use ew_model::EndpointKey;
use serde_json::json;

const FNDEF_VALVE: u128 = 0x700;
const FNDEF_COIL: u128 = 0x701;

/// Two valves -Q1 and -Q2 with pneumatic ports 1 and 2 and an electrical coil each, on page 0x10.
fn valves() -> Fx {
    let mut fx = Fx::new();
    fx.lib_item(
        "fndef",
        FNDEF_VALVE,
        json!({"meta": {}, "category": ["Valve", "Directional", "5/2"], "contact_kind": "valve", "cps": [
            {"idx": 0, "designation": "1", "discipline": "pneumatic", "port_role": "supply1"},
            {"idx": 1, "designation": "2", "discipline": "pneumatic", "port_role": "working2"},
        ]}),
    );
    fx.fndef(FNDEF_COIL, ["Coil", "Coil", "Coil"], &[(0, "A1"), (1, "A2")]);
    fx.page(0x10, 1, None, "multi_line", &[], "valves");
    for (dev, valve, coil, n) in [(0x100, 0x200, 0x210, 1), (0x101, 0x201, 0x211, 2)] {
        fx.device(dev, "Q", Some(n))
            .function(valve, Some(dev), FNDEF_VALVE, true)
            .function(coil, Some(dev), FNDEF_COIL, false);
    }
    fx
}

fn line(fx: &mut Fx, n: u128, a: serde_json::Value, b: serde_json::Value, kind: serde_json::Value) {
    fx.push(json!({"$k": "connection", "id": uid(n), "page": uid(0x10), "a": a, "b": b, "kind": kind}));
}

fn cp(f: u128, idx: u16) -> serde_json::Value {
    json!({"cp": [uid(f), idx]})
}

#[test]
fn fluid_lines_and_their_junction_make_one_fluid_net() {
    let mut fx = valves();
    fx.push(
        json!({"$k": "junction", "id": uid(0x300), "page": uid(0x10), "at": [0, 0], "style": "dot",
        "sequence": [uid(0x400), uid(0x401)]}),
    );
    line(
        &mut fx,
        0x400,
        cp(0x200, 1),
        json!({"junction": uid(0x300)}),
        json!({"fluid": {}}),
    );
    line(
        &mut fx,
        0x401,
        json!({"junction": uid(0x300)}),
        cp(0x201, 1),
        json!({"fluid": {"line_kind": "pilot"}}),
    );
    let m = fx.build();
    let q = Q::new(&m);
    let nets = q.get::<FluidNets>(());
    assert_eq!(nets.nets.len(), 1);
    let net = nets.net_of(EndpointKey::Cp(id(0x200), 1)).expect("a fluid net");
    assert_eq!(
        net.members,
        [
            EndpointKey::Cp(id(0x200), 1),
            EndpointKey::Cp(id(0x201), 1),
            EndpointKey::Junction(id(0x300)),
        ]
    );
    assert_eq!(net.connections, [id(0x400), id(0x401)]);
    assert_eq!(Some(net.id), NetId::of(net.members.iter().copied()));
    // A port that no fluid line reaches is in no fluid net.
    assert!(nets.net_of(EndpointKey::Cp(id(0x200), 0)).is_none());
}

#[test]
fn a_fluid_line_joins_no_electrical_net() {
    let mut fx = valves();
    line(&mut fx, 0x400, cp(0x200, 1), cp(0x201, 1), json!({"fluid": {}}));
    line(&mut fx, 0x401, cp(0x210, 0), cp(0x211, 0), json!("wire"));
    let m = fx.build();
    let q = Q::new(&m);
    let partition = q.get::<PotentialPartition>(());
    assert!(partition.potential_at(EndpointKey::Cp(id(0x200), 1)).is_none());
    assert!(partition.potential_at(EndpointKey::Cp(id(0x201), 1)).is_none());
    // The coils' wire is one potential, as without the fluid line.
    let a = partition
        .potential_at(EndpointKey::Cp(id(0x210), 0))
        .map(|p| p.id);
    assert!(a.is_some());
    assert_eq!(
        a,
        partition
            .potential_at(EndpointKey::Cp(id(0x211), 0))
            .map(|p| p.id)
    );
    // Only the fluid line is in the fluid partition.
    let nets = q.get::<FluidNets>(());
    assert_eq!(nets.nets.len(), 1);
    assert_eq!(nets.nets[0].connections, [id(0x400)]);
}
