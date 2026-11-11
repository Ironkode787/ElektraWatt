// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid side: the fluid partition `fluid_nets`, which joins fluid lines and their junctions
//! and leaves every electrical net as it is.

use crate::common;

use common::{Fx, id, uid};
use ew_derive::{
    FluidNets, NetId, PotentialPartition, Q, reached_device, tracked_process_tag, tracked_target,
};
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

const FNDEF_CYL: u128 = 0x702;

/// [`valves`] plus two cylinders -Q11 and -Q12 (consumer ports A and B) with process tags.
fn with_cylinders() -> Fx {
    let mut fx = valves();
    fx.lib_item(
        "fndef",
        FNDEF_CYL,
        json!({"meta": {}, "category": ["Actuator", "Cylinder", "DoubleActing"], "contact_kind": "black_box", "cps": [
            {"idx": 0, "designation": "A", "discipline": "pneumatic", "port_role": "consumer"},
            {"idx": 1, "designation": "B", "discipline": "pneumatic", "port_role": "consumer"},
        ]}),
    );
    for (dev, f, n, tag) in [(0x110, 0x220, 11, "TA05-V-15"), (0x111, 0x221, 12, "TA05-V-33")] {
        fx.device(dev, "Q", Some(n))
            .with("process_tag", json!(tag))
            .function(f, Some(dev), FNDEF_CYL, true);
    }
    fx.record(0x100)["process_tag"] = json!("OWN-1");
    fx
}

/// D13 (FL-10, FL-11): a line from a working port reaches the cylinder; the tracked target is the `drives` link's
/// device first, else the reached device; the process tag follows it, else the valve's own.
#[test]
fn the_tracked_target_is_the_drives_link_else_the_reached_device() {
    let mut fx = with_cylinders();
    line(&mut fx, 0x400, cp(0x200, 1), cp(0x220, 0), json!({"fluid": {}}));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(reached_device(&q, id(0x100)), Some(id(0x110)));
    assert_eq!(tracked_target(&q, id(0x100)), Some(id(0x110)));
    assert_eq!(tracked_process_tag(&q, id(0x100)).as_deref(), Some("TA05-V-15"));
    // A supply port's line reaches nothing: only working ports count.
    assert_eq!(reached_device(&q, id(0x101)), None);
    // A `drives` link wins over the reached device.
    let mut fx = with_cylinders();
    line(&mut fx, 0x400, cp(0x200, 1), cp(0x220, 0), json!({"fluid": {}}));
    fx.push(json!({"$k": "device_link", "id": uid(0x500), "from": uid(0x100), "to": uid(0x111), "kind": "drives"}));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(tracked_target(&q, id(0x100)), Some(id(0x111)));
    assert_eq!(tracked_process_tag(&q, id(0x100)).as_deref(), Some("TA05-V-33"));
    // Two reached devices: none is the target, and the valve's own process tag stands.
    let mut fx = with_cylinders();
    line(&mut fx, 0x400, cp(0x200, 1), cp(0x220, 0), json!({"fluid": {}}));
    line(&mut fx, 0x401, cp(0x200, 1), cp(0x221, 0), json!({"fluid": {}}));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(reached_device(&q, id(0x100)), None);
    assert_eq!(tracked_process_tag(&q, id(0x100)).as_deref(), Some("OWN-1"));
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
