// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The bus model typed within format 0.1 (WP-S1.21 D11, BU-1; ADR-0011 amendment 4): the `logical_net` and
//! `physical_net` kinds and the function's `bus_port` aspect round-trip byte for byte, a newer `NetAddress` form and
//! newer members survive a load and a save verbatim, a malformed known variant is kept by the loader (EW-CORE-017),
//! and a function without a port is written as before.

use crate::common;

use common::fill;
use ew_schema::{AddrValue, BusMedium, BusSystem, Kind, KindStatus, NetAddress, OnOff, PortRole, Record};

fn canonical(template: &str) -> Record {
    let line = fill(template);
    let record: Record = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(serde_json::to_string(&record).unwrap(), line, "not canonical");
    record
}

#[test]
fn the_bus_kinds_are_implemented_project_file_kinds() {
    for (kind, rank) in [(Kind::LogicalNet, 12), (Kind::PhysicalNet, 13)] {
        assert_eq!(kind.status(), KindStatus::Implemented);
        assert_eq!(kind.rank(), Some(rank));
        assert_eq!(kind.schema_version(), 1);
        assert_eq!(kind.file_rule(), ew_schema::FileRule::ProjectFile);
    }
}

#[test]
fn golden_bus_records_are_typed() {
    let Record::LogicalNet(net) = canonical(
        r#"{"$k":"logical_net","id":"@LNET_DP","bus":"profibus_dp","name":"PROFIBUS DP","master":{"cp":["@FN_X1031",0]}}"#,
    ) else {
        panic!("a logical net")
    };
    assert_eq!(net.bus, BusSystem::ProfibusDp);
    assert_eq!(net.rate, None);
    assert!(net.master.is_some());
    let Record::PhysicalNet(pnet) =
        canonical(r#"{"$k":"physical_net","id":"@PNET_DP","name":"DP","medium":"copper"}"#)
    else {
        panic!("a physical net")
    };
    assert_eq!(pnet.medium, BusMedium::Copper);
    let Record::Function(f) = canonical(
        r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","bus_port":{"interface":"DP","bus":"profibus_dp","role":"passive","address":"not_applicable","termination":"on","physical_net":"@PNET_DP"}}"#,
    ) else {
        panic!("a function")
    };
    let port = f.bus_port.expect("a port");
    assert_eq!(port.role, PortRole::Passive);
    assert_eq!(port.address, AddrValue::NotApplicable);
    assert_eq!(port.termination, Some(OnOff::On));
    let Record::Function(f) = canonical(
        r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","bus_port":{"interface":"DP","bus":"profibus_dp","role":"slave","address":{"value":{"profibus":3}}}}"#,
    ) else {
        panic!("a function")
    };
    assert_eq!(
        f.bus_port.unwrap().address,
        AddrValue::Value(NetAddress::Profibus(3))
    );
}

/// BU-1: an address form of a newer format (a PROFINET name, typed only by WP-S1.24), a newer bus system, role and
/// medium, and newer members at every depth survive a load and a save byte for byte.
#[test]
fn newer_bus_values_survive_a_load_and_a_save_verbatim() {
    let Record::Function(f) = canonical(
        r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","bus_port":{"interface":"X1","bus":"profinet","role":"device","address":{"value":{"profinet":{"ip":"192.168.0.10","name":"io-device-1"}}},"logical_net":"@LNET_DP","mrp_role":"client"}}"#,
    ) else {
        panic!("a function")
    };
    let port = f.bus_port.unwrap();
    assert!(matches!(port.address, AddrValue::Value(NetAddress::Unknown(_))));
    assert_eq!(port.extra.len(), 1);
    let Record::Function(f) = canonical(
        r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","bus_port":{"interface":"X","bus":"sercos","role":"observer","address":{"pending":{"since":"2026-10-30"}},"termination":"auto"}}"#,
    ) else {
        panic!("a function")
    };
    let port = f.bus_port.unwrap();
    assert!(matches!(port.bus, BusSystem::Unknown(_)));
    assert!(matches!(port.role, PortRole::Unknown(_)));
    assert!(matches!(port.address, AddrValue::Unknown(_)));
    assert!(matches!(port.termination, Some(OnOff::Unknown(_))));
    let Record::LogicalNet(net) = canonical(
        r#"{"$k":"logical_net","id":"@LNET_DP","bus":"profibus_dp","name":"DP","rate":1500000,"space":{"max":125,"min":0}}"#,
    ) else {
        panic!("a logical net")
    };
    assert_eq!(net.rate, Some(ew_schema::BusRate(1_500_000)));
    assert!(
        net.extra.contains_key("space"),
        "the address space stays in extra"
    );
    let Record::PhysicalNet(pnet) = canonical(
        r#"{"$k":"physical_net","id":"@PNET_DP","name":"DP","medium":{"wireless":{"band":"2.4 GHz"}}}"#,
    ) else {
        panic!("a physical net")
    };
    assert!(matches!(pnet.medium, BusMedium::Unknown(_)));
}

/// A known variant with a payload that does not parse (a PROFIBUS address outside a byte) is refused by the strict
/// reader and kept verbatim by the loader, which reports it (EW-CORE-017).
#[test]
fn a_malformed_profibus_address_is_kept_by_the_loader() {
    let line = fill(
        r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","bus_port":{"interface":"DP","bus":"profibus_dp","role":"slave","address":{"value":{"profibus":300}}}}"#,
    );
    assert!(serde_json::from_str::<Record>(&line).is_err());
    let (record, malformed) = Record::load(ew_schema::parse_value(&line).unwrap()).unwrap();
    assert_eq!(malformed.len(), 1, "{malformed:?}");
    assert_eq!(malformed[0].variant, "profibus");
    assert_eq!(serde_json::to_string(&record).unwrap(), line);
}

/// A function without a port, and a port without its optional members, are written without them: the typing adds
/// no byte to a project that has no bus records.
#[test]
fn a_function_without_a_port_is_written_as_before() {
    let line =
        fill(r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","template_entry":1}"#);
    let Record::Function(f) = canonical(
        r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","template_entry":1}"#,
    ) else {
        panic!("a function")
    };
    assert!(f.bus_port.is_none());
    assert!(
        !serde_json::to_string(&Record::Function(f))
            .unwrap()
            .contains("bus_port")
    );
    assert!(line.ends_with(r#""template_entry":1}"#));
}

/// The required members of the new kinds are refused when missing, at their path.
#[test]
fn missing_required_bus_members_are_refused() {
    for (template, path) in [
        (r#"{"$k":"logical_net","id":"@LNET_DP","name":"DP"}"#, ""),
        (r#"{"$k":"physical_net","id":"@PNET_DP","name":"DP"}"#, ""),
        (
            r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","bus_port":{"interface":"DP","bus":"profibus_dp","role":"slave"}}"#,
            "/bus_port",
        ),
    ] {
        let err = Record::from_json(&fill(template)).unwrap_err();
        assert!(err.path().unwrap_or("").starts_with(path), "{template}: {err}");
    }
}
