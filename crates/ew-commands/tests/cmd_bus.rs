// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 4 in the engine (D11): `bus.logical_net.create`, `bus.physical_net.create` and the bus members
//! through `fields.set` (a function's `bus_port`, a logical net's master and rate, a physical net's medium); an edit
//! writes only values this build knows.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{CreateLogicalNet, CreatePhysicalNet, KindName, SetFields};
use ew_commands::{CmdError, Engine, Origin, Receipt};
use ew_ids::Id;
use ew_schema::{
    AddrValue, BusMedium, BusRate, BusSystem, Endpoint, Function, Kind, LogicalNet, NetAddress, OnOff,
    PhysicalNet, PortRole,
};
use serde_json::{Value, json};

fn lnet(name: &str, bus: BusSystem, rate: Option<u32>) -> CreateLogicalNet {
    CreateLogicalNet {
        bus,
        name: ident(name),
        rate: rate.map(BusRate),
    }
}

fn pnet(name: &str) -> CreatePhysicalNet {
    CreatePhysicalNet {
        name: ident(name),
        medium: BusMedium::Copper,
    }
}

fn set(kind: KindName, target: ew_schema::AnyId, path: &str, value: Value) -> SetFields {
    SetFields {
        kind,
        targets: vec![target],
        path: path.into(),
        value,
    }
}

fn invalid(r: Result<Receipt, CmdError>) -> (String, String) {
    match r {
        Err(CmdError::InvalidParams { path, message, .. }) => (path, message),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

fn rejected(r: Result<Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected Rejected, got {other:?}"),
    }
}

/// The two nets of a PROFIBUS line in the mini golden project.
fn with_nets() -> (Engine, Id<LogicalNet>, Id<PhysicalNet>) {
    let mut e = engine(mini_golden());
    let r = e
        .execute(&lnet("DP", BusSystem::ProfibusDp, Some(1_500_000)))
        .expect("bus.logical_net.create");
    let l: Id<LogicalNet> = typed(r.outcome.primary.expect("the net"));
    assert!(r.created.contains(&(Kind::LogicalNet, l.cast())));
    let r = e.execute(&pnet("DP")).expect("bus.physical_net.create");
    let p: Id<PhysicalNet> = typed(r.outcome.primary.expect("the net"));
    (e, l, p)
}

#[test]
fn the_create_commands_insert_the_nets_in_one_step_each() {
    let (mut e, l, p) = with_nets();
    let net = e.model().table::<LogicalNet>().get(&l).expect("the net").clone();
    assert_eq!(net.name.as_str(), "DP");
    assert_eq!(net.bus, BusSystem::ProfibusDp);
    assert_eq!(net.rate, Some(BusRate(1_500_000)));
    assert_eq!(net.master, None);
    let phys = e.model().table::<PhysicalNet>().get(&p).expect("the net").clone();
    assert_eq!(phys.medium, BusMedium::Copper);
    assert!(phys.order_hint.is_empty());
    e.undo(&Origin::Gui).expect("undo the physical net");
    assert!(e.model().table::<PhysicalNet>().get(&p).is_none());
    assert!(e.model().table::<LogicalNet>().get(&l).is_some());
}

#[test]
fn the_create_commands_refuse_a_duplicate_name_a_zero_rate_and_a_newer_bus() {
    let (mut e, _, _) = with_nets();
    assert!(rejected(e.execute(&lnet("DP", BusSystem::Profinet, None))).contains("names are unique"));
    assert!(rejected(e.execute(&pnet("DP"))).contains("names are unique"));
    let (path, message) = invalid(e.execute(&lnet("DP2", BusSystem::ProfibusDp, Some(0))));
    assert_eq!(path, "/rate");
    assert!(message.contains("a data rate of 0 bit/s"), "{message}");
    let (path, message) = invalid(e.execute(&lnet("DP3", BusSystem::Unknown(json!("sercos")), None)));
    assert_eq!(path, "/bus");
    assert!(message.contains("is not a bus system"), "{message}");
}

/// A function's port is set whole, then member by member; a member of a function without a port, a value of a newer
/// format and a net that does not exist are refused.
#[test]
fn fields_set_writes_a_bus_port_and_refuses_what_it_cannot_write() {
    let (mut e, l, p) = with_nets();
    let f = FN_102F2;
    let (path, message) = invalid(e.execute(&set(
        KindName::Function,
        f.cast(),
        "bus_port.termination",
        json!("on"),
    )));
    assert_eq!(path, "/path");
    assert!(
        message.contains("has no bus port; set `bus_port` first"),
        "{message}"
    );

    e.execute(&set(
        KindName::Function,
        f.cast(),
        "bus_port",
        json!({"interface": "DP", "bus": "profibus_dp", "role": "slave", "address": "tbd",
            "logical_net": l.to_string(), "physical_net": p.to_string()}),
    ))
    .expect("the port");
    e.execute(&set(
        KindName::Function,
        f.cast(),
        "bus_port.address",
        json!({"value": {"profibus": 3}}),
    ))
    .expect("the address");
    e.execute(&set(
        KindName::Function,
        f.cast(),
        "bus_port.termination",
        json!("off"),
    ))
    .expect("the termination");
    let port = e
        .model()
        .table::<Function>()
        .get(&f)
        .and_then(|x| x.bus_port.clone())
        .expect("a port");
    assert_eq!(port.role, PortRole::Slave);
    assert_eq!(port.address, AddrValue::Value(NetAddress::Profibus(3)));
    assert_eq!(port.termination, Some(OnOff::Off));
    assert_eq!(port.logical_net, Some(l));

    let (_, message) = invalid(e.execute(&set(
        KindName::Function,
        f.cast(),
        "bus_port.address",
        json!({"value": {"profinet": {"name": "io-1"}}}),
    )));
    assert!(message.contains("is not a bus address"), "{message}");
    let (_, message) = invalid(e.execute(&set(
        KindName::Function,
        f.cast(),
        "bus_port.termination",
        json!("auto"),
    )));
    assert!(message.contains("is not a termination"), "{message}");
    assert!(
        e.execute(&set(
            KindName::Function,
            f.cast(),
            "bus_port.physical_net",
            json!(FN_101F1.to_string()),
        ))
        .is_err(),
        "a net that does not exist"
    );
}

/// A logical net's master is the CP of a function; a junction is refused; the rate cannot be 0.
#[test]
fn fields_set_writes_a_master_and_refuses_a_junction_and_a_zero_rate() {
    let (mut e, l, _) = with_nets();
    e.execute(&set(
        KindName::LogicalNet,
        l.cast(),
        "master",
        json!({"cp": [FN_102F2.to_string(), 0]}),
    ))
    .expect("the master");
    let net = e.model().table::<LogicalNet>().get(&l).expect("the net").clone();
    assert_eq!(net.master, Some(Endpoint::Cp(FN_102F2, 0)));
    let (_, message) = invalid(e.execute(&set(
        KindName::LogicalNet,
        l.cast(),
        "master",
        json!({"junction": FN_102F2.to_string()}),
    )));
    assert!(message.contains("a master is a CP"), "{message}");
    let (_, message) = invalid(e.execute(&set(KindName::LogicalNet, l.cast(), "rate", json!(0))));
    assert!(message.contains("a data rate of 0 bit/s"), "{message}");
}
