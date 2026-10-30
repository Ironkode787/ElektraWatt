// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus tables (WP-S1.21 D11; doc 20 §11.1): `logical_nets.csv`, `physical_nets.csv` and `bus_ports.csv` on the
//! golden tables, and the rows that are refused when the golden bus tables are replaced by small ones.

mod common;

use ew_ids::Id;
use ew_import_tables::ids::record_id;
use ew_interop_api::{Code, ImportBatch, ImportInput};
use ew_schema::{
    AddrValue, BusMedium, BusPort, BusSystem, Endpoint, Function, LogicalNet, NetAddress, OnOff, PhysicalNet,
    PortRole, Record,
};

fn port<'b>(b: &'b ImportBatch, key: &str) -> Option<&'b BusPort> {
    let id: Id<Function> = record_id("functions", key);
    b.records
        .iter()
        .find_map(|r| match r {
            Record::Function(f) if f.id == id => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("function {key} is emitted"))
        .bus_port
        .as_ref()
}

fn logical_nets(b: &ImportBatch) -> Vec<&LogicalNet> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::LogicalNet(n) => Some(n),
            _ => None,
        })
        .collect()
}

fn physical_nets(b: &ImportBatch) -> Vec<&PhysicalNet> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::PhysicalNet(n) => Some(n),
            _ => None,
        })
        .collect()
}

/// BU-1's import half: the golden DP nets and the two ports, without a finding.
#[test]
fn the_golden_dp_nets_and_ports_are_typed() {
    let b = common::import(&common::golden_inputs()).expect("the golden tables import");
    assert!(b.report.entries.is_empty(), "{:#?}", b.report.entries);
    let lnets = logical_nets(&b);
    assert_eq!(lnets.len(), 1);
    let l = lnets[0];
    assert_eq!(
        (l.name.as_str(), &l.bus, l.rate),
        ("PROFIBUS DP", &BusSystem::ProfibusDp, None)
    );
    assert_eq!(l.master, Some(Endpoint::Cp(record_id("functions", "DP@3"), 0)));
    let pnets = physical_nets(&b);
    assert_eq!(pnets.len(), 1);
    assert_eq!(
        (pnets[0].name.as_str(), &pnets[0].medium),
        ("DP", &BusMedium::Copper)
    );
    let slave = port(&b, "-201U1#bus").expect("the station port");
    assert_eq!(
        (&slave.role, &slave.address, slave.logical_net, slave.physical_net),
        (&PortRole::Slave, &AddrValue::Tbd, Some(l.id), Some(pnets[0].id))
    );
    let plug = port(&b, "-3X1").expect("the plug's port");
    assert_eq!(
        (&plug.role, &plug.address, &plug.termination, plug.logical_net),
        (
            &PortRole::Passive,
            &AddrValue::NotApplicable,
            &Some(OnOff::On),
            None
        )
    );
}

const LNETS: &str = "key,bus,name,master,rate\n";
const PNETS: &str = "key,name,medium,order_hint\n";
const PORTS: &str = "key,interface,port_label,bus,role,address,termination,logical_net,physical_net\n";

/// The golden tables with the three bus tables replaced.
fn run(lnets: &str, pnets: &str, ports: &str) -> ImportBatch {
    let replaced = [
        ("logical_nets.csv", format!("{LNETS}{lnets}")),
        ("physical_nets.csv", format!("{PNETS}{pnets}")),
        ("bus_ports.csv", format!("{PORTS}{ports}")),
    ];
    let inputs: Vec<ImportInput> = common::golden_inputs()
        .into_iter()
        .map(|i| match replaced.iter().find(|(n, _)| *n == i.name()) {
            Some((n, text)) => ImportInput::new(n, text.as_bytes().to_vec()).expect("a name"),
            None => i,
        })
        .collect();
    common::import(&inputs).expect("the import succeeds")
}

fn messages(b: &ImportBatch, code: u16) -> Vec<&str> {
    b.report
        .with_code(Code::imp(code))
        .map(|e| e.message.as_str())
        .collect()
}

const NETS: (&str, &str) = ("DP,profibus_dp,DP,DP@3,\n", "DP,DP,copper,\n");

/// A PROFIBUS station address is typed; an address of another bus system is not (yet) and is refused.
#[test]
fn a_numeric_address_is_typed_on_profibus_only() {
    let b = run(NETS.0, NETS.1, "-201U1#bus,DP,,profibus_dp,slave,3,,DP,DP\n");
    assert!(b.report.entries.is_empty(), "{:#?}", b.report.entries);
    assert_eq!(
        port(&b, "-201U1#bus").expect("a port").address,
        AddrValue::Value(NetAddress::Profibus(3))
    );
    for row in [
        "-201U1#bus,DP,,profinet,device,3,,,DP\n",
        "-201U1#bus,DP,,profibus_dp,slave,300,,DP,DP\n",
    ] {
        let b = run(NETS.0, NETS.1, row);
        let found = messages(&b, 2);
        assert_eq!(found.len(), 1, "{row}: {:#?}", b.report.entries);
        assert!(
            found[0].contains("is not tbd, not_applicable or a PROFIBUS station address"),
            "{found:?}"
        );
        assert!(port(&b, "-201U1#bus").is_none(), "{row}");
    }
}

/// An unknown function or net key drops the row (EW-IMP-003); a rate of 0 is malformed (EW-IMP-002).
#[test]
fn an_unknown_key_or_a_zero_rate_drops_the_row() {
    for (lnets, ports, what) in [
        (
            "DP,profibus_dp,DP,NOPE,\n",
            "",
            "unknown function `NOPE` in column `master`",
        ),
        (
            NETS.0,
            "NOPE,DP,,profibus_dp,slave,tbd,,DP,DP\n",
            "unknown function `NOPE`",
        ),
        (
            NETS.0,
            "-201U1#bus,DP,,profibus_dp,slave,tbd,,XX,DP\n",
            "unknown logical net `XX`",
        ),
        (
            NETS.0,
            "-201U1#bus,DP,,profibus_dp,slave,tbd,,DP,XX\n",
            "unknown physical net `XX`",
        ),
    ] {
        let b = run(lnets, NETS.1, ports);
        let found = messages(&b, 3);
        assert_eq!(found.len(), 1, "{ports}: {:#?}", b.report.entries);
        assert!(found[0].contains(what), "{found:?}");
    }
    let b = run("DP,profibus_dp,DP,,0\n", NETS.1, "");
    let found = messages(&b, 2);
    assert_eq!(found.len(), 1, "{:#?}", b.report.entries);
    assert!(found[0].contains("a data rate of 0 bit/s"), "{found:?}");
    assert!(logical_nets(&b).is_empty());
}
