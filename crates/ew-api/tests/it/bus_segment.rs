// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus segment of the sample project (page 105, AC-31's derived half): the PROFIBUS DP line runs from the CPU's
//! port -104A1:DP over the cable -105W1 to the valve terminal's port -201K1:DP; the CPU is the master; the ports print
//! their addresses and terminations; EW-BUS-004 reports an end switched off and EW-BUS-001 a TBD station address,
//! which supersedes EW-CORE-044. The segment's rules and their in-code fixtures are `ew-checks`' and `ew-derive`'s
//! `bus.rs`.

use crate::common;

use ew_api::checks::{CheckMode, CheckReport, Finding, PRE_DELIVERY, Severity};
use ew_api::{AnyCommand, Session};
use ew_derive::{
    BusSegments, NetMaster, NetMembersOf, Q, SegmentMember, bus_address_text, bus_termination_text,
};
use ew_ids::Id;
use ew_schema::{Device, Function, PhysicalNet};
use serde_json::json;

/// The bus port function of the device tagged `tag` on the sample.
fn port(s: &Session, tag: &str) -> Id<Function> {
    let q = s.query();
    let device: Id<Device> = Id::from_uuid(s.resolve_reference(tag).expect("the device"));
    let functions = q.functions();
    let mut ports: Vec<Id<Function>> = q
        .idx()
        .functions_of_device(device)
        .filter(|f| {
            functions
                .get(*f)
                .and_then(|f| f.bus_port.as_ref())
                .is_some_and(|p| p.interface.as_str() == "DP")
        })
        .collect();
    assert_eq!(ports.len(), 1, "{tag} has one DP port");
    ports.pop().unwrap()
}

/// The open findings of `rule` as (severity, subject, discriminator).
fn open(report: &CheckReport, rule: &str) -> Vec<(Severity, String, String)> {
    report
        .findings
        .iter()
        .filter(|f: &&Finding| f.rule == rule && f.is_open())
        .map(|f| (f.severity, f.subject.clone(), f.discriminator.clone()))
        .collect()
}

fn set_port(s: &mut Session, port: Id<Function>, path: &str, value: &str) {
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({
            "kind": "function", "targets": [port.to_string()], "path": path, "value": value,
        }),
    })
    .unwrap_or_else(|e| panic!("fields.set {path} {value}: {e:?}"));
}

/// The physical net of the line, which the window's bus tools do not make: created, and named by both ports.
fn with_physical_net(s: &mut Session) -> Id<PhysicalNet> {
    let r = s
        .execute(&AnyCommand {
            name: "bus.physical_net.create".into(),
            params: json!({"name": "DP", "medium": "copper"}),
        })
        .expect("bus.physical_net.create");
    let pnet: Id<PhysicalNet> = Id::from_uuid(r.outcome.primary.expect("the net").1.as_uuid());
    for tag in ["-104A1", "-201K1"] {
        let p = port(s, tag);
        set_port(s, p, "bus_port.physical_net", &pnet.to_string());
    }
    pnet
}

/// FR-BUS-007: the bus overview walks the DP segment from the master end: the CPU's port, the cable -105W1 and the
/// valve terminal's port.
#[test]
fn the_bus_overview_walks_the_segment_from_the_master() {
    let mut s = common::sample();
    with_physical_net(&mut s);
    let q = s.query();
    let block = q
        .report_blocks()
        .iter()
        .next()
        .map(|(_, b)| (**b).clone())
        .expect("a report block");
    let def = ew_reports::Registry::builtin()
        .by_name("bus_overview")
        .expect("a registered kind");
    let csv = ew_reports::csv::rows_csv(&q, &def.rows(&q, &block));
    let rows: Vec<&str> = csv.lines().skip(1).collect();
    assert_eq!(rows.len(), 3, "{rows:#?}");
    assert!(rows[0].contains("-104A1"), "{}", rows[0]);
    assert!(rows[1].contains("-105W1"), "{}", rows[1]);
    assert!(rows[2].contains("-201K1"), "{}", rows[2]);
}

/// BU-2 (D11, A5): the DP line is one segment, ordered from the master end: the CPU's port, the cable -105W1 and the
/// valve terminal's port; both ports are its ends, neither is external, and it is on a logical net.
#[test]
fn one_segment_from_the_master_port_to_the_station_port() {
    let mut s = common::sample();
    let pnet = with_physical_net(&mut s);
    let q: Q = s.query();
    let master = port(&s, "-104A1");
    let station = port(&s, "-201K1");
    let cable: Id<Device> = Id::from_uuid(s.resolve_reference("-105W1").expect("the bus cable"));
    let segments = q.get::<BusSegments>(pnet);
    assert_eq!(segments.len(), 1, "{segments:#?}");
    let segment = &segments[0];
    assert_eq!(
        segment.members,
        [
            SegmentMember::Function(master),
            SegmentMember::Cable(cable),
            SegmentMember::Function(station),
        ]
    );
    assert!(segment.external_ends.is_empty());
    assert!(
        segment.ends.contains(&master) && segment.ends.contains(&station),
        "{:?}",
        segment.ends
    );
    assert!(segment.logical_net.is_some());
    assert!(!segment.branched);
}

/// BU-4 (FR-BUS-005): the master of the logical net is the CPU's port, and the valve terminal's port is a member.
#[test]
fn the_cpu_port_is_the_master_of_the_net() {
    let s = common::sample();
    let q: Q = s.query();
    let master = port(&s, "-104A1");
    let station = port(&s, "-201K1");
    let lnet = q
        .functions()
        .get(station)
        .and_then(|f| f.bus_port.as_ref())
        .and_then(|p| p.logical_net)
        .expect("the station port names its net");
    let members = q.get::<NetMembersOf>(lnet);
    assert!(members.ports.contains(&station), "{members:?}");
    assert_eq!(members.master, Some(NetMaster::Port(master)));
}

/// BU-5's derived half: a port prints its station address ("Node 3") and its termination switch; an open address
/// prints "Node TBD"; a switch Off prints "Switch to "OFF"".
#[test]
fn the_port_texts() {
    let mut s = common::sample();
    let station = port(&s, "-201K1");
    let text = |s: &Session| {
        let q = s.query();
        let port = q.functions().get(station).unwrap().bus_port.clone().unwrap();
        (bus_address_text(&port), bus_termination_text(&port))
    };
    assert_eq!(
        text(&s),
        (Some("Node 3".to_owned()), Some("Switch to \"ON\"".to_owned()))
    );
    set_port(&mut s, station, "bus_port.address", "tbd");
    set_port(&mut s, station, "bus_port.termination", "off");
    assert_eq!(
        text(&s),
        (Some("Node TBD".to_owned()), Some("Switch to \"OFF\"".to_owned()))
    );
}

/// BU-6 = AC-31 (D12): the sample's line has no bus finding. BU-3: with the valve terminal's end Off, EW-BUS-004
/// reports one error at that end, and On again clears it. A TBD station address is one EW-BUS-001 error on its port,
/// worded in each language, and EW-CORE-044's finding for it is superseded.
#[test]
fn ac31_the_bus_checks_on_the_sample() {
    let mut s = common::sample();
    with_physical_net(&mut s);
    let station = port(&s, "-201K1");
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert_eq!(open(&report, "EW-BUS-004"), []);
    assert_eq!(open(&report, "EW-BUS-001"), []);

    set_port(&mut s, station, "bus_port.termination", "off");
    let off = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert_eq!(
        open(&off, "EW-BUS-004"),
        [(Severity::Error, "+CAB-201K1:DP".to_owned(), "end".to_owned())]
    );
    set_port(&mut s, station, "bus_port.termination", "on");
    let on = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert_eq!(open(&on, "EW-BUS-004"), []);

    set_port(&mut s, station, "bus_port.address", "tbd");
    let tbd = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert_eq!(
        open(&tbd, "EW-BUS-001"),
        [(Severity::Error, "+CAB-201K1:DP".to_owned(), "address".to_owned())]
    );
    assert_eq!(open(&tbd, "EW-CORE-044"), [], "superseded by EW-BUS-001");
    assert_eq!(tbd.exit_code(), 1, "a TBD address blocks release");
    let finding = tbd
        .findings
        .iter()
        .find(|f| f.rule == "EW-BUS-001")
        .expect("the finding");
    assert_eq!(
        ew_api::checks::messages::format(finding, "en-GB").as_deref(),
        Some(finding.message.as_str())
    );
    assert_ne!(
        ew_api::checks::messages::render(finding, "nl-NL"),
        finding.message,
        "worded in Dutch"
    );
}
