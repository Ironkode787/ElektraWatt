// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 4 on Golden A (acceptance BU-2, BU-3, BU-4, BU-5, BU-6 = AC-31; D11, D12): the DP segment runs from
//! the external end over -3W1 and -3X1 to -201U1:DP; the external partner JE-N1000-03 is the master and in no BOM;
//! EW-BUS-004 gives one info for the external end and no error while -3X1 is On, an error once it is Off; EW-BUS-001
//! reports the TBD station address once (EF-03) and supersedes EW-CORE-044.

use crate::golden_common;

use std::collections::BTreeMap;

use ew_api::checks::{CheckMode, CheckReport, Finding, PRE_DELIVERY, Severity};
use ew_api::{AnyCommand, BomMode, Export, Session};
use ew_derive::{
    BusSegments, NetMaster, NetMembersOf, Q, SegmentMember, bus_address_text, bus_termination_text,
    external_partner_text,
};
use ew_ids::Id;
use ew_schema::{Device, Function, LogicalNet, PhysicalNet};
use serde_json::json;

fn ids() -> BTreeMap<String, String> {
    golden_common::ids_by_key(&golden_common::batch())
}

fn id<T>(ids: &BTreeMap<String, String>, key: &str) -> Id<T> {
    ids.get(key)
        .unwrap_or_else(|| panic!("{key} is in the golden batch"))
        .parse()
        .expect("an ID")
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

fn set_termination(s: &mut Session, plug: Id<Function>, value: &str) {
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({
            "kind": "function", "targets": [plug.to_string()], "path": "bus_port.termination", "value": value,
        }),
    })
    .unwrap_or_else(|e| panic!("fields.set bus_port.termination {value}: {e:?}"));
}

/// The external end of the DP segment, printed as EW-BUS-004 names it.
const EXTERNAL_END: &str = "external end DP (+J-E-206 / JE-N1000-03)";

/// BU-2 (D11, A5): `bus_segments(DP)` holds one segment, ordered from the master end: the external interruption point
/// DP, the cable -3W1, the plug -3X1 and the station port -201U1:DP; the external end is the IP, and the segment is
/// on the logical net DP.
#[test]
fn bu2_one_segment_from_the_external_end_to_the_station_port() {
    let ids = ids();
    let s = golden_common::session(&golden_common::batch());
    let q: Q = s.query();
    let pnet: Id<PhysicalNet> = id(&ids, "physical_nets|DP");
    let lnet: Id<LogicalNet> = id(&ids, "logical_nets|DP");
    let ip: Id<Function> = id(&ids, "functions|DP@3");
    let cable: Id<Device> = id(&ids, "devices|-3W1");
    let plug: Id<Function> = id(&ids, "functions|-3X1");
    let port: Id<Function> = id(&ids, "functions|-201U1#bus");
    let segments = q.get::<BusSegments>(pnet);
    assert_eq!(segments.len(), 1, "{segments:#?}");
    let segment = &segments[0];
    assert_eq!(
        segment.members,
        [
            SegmentMember::Function(ip),
            SegmentMember::Cable(cable),
            SegmentMember::Function(plug),
            SegmentMember::Function(port),
        ]
    );
    assert_eq!(segment.external_ends, [ip]);
    assert!(
        segment.ends.contains(&ip) && segment.ends.contains(&port),
        "{:?}",
        segment.ends
    );
    assert_eq!(segment.logical_net, Some(lnet));
    assert!(!segment.branched);
}

/// BU-4 (FR-BUS-005): the master of the logical net DP is the external partner JE-N1000-03 (the external
/// interruption point DP of folio 3), the station port -201U1:DP is its one member, and the partner is no device: it
/// is in neither BOM.
#[test]
fn bu4_the_external_partner_is_the_master_and_in_no_bom() {
    let ids = ids();
    let s = golden_common::session(&golden_common::batch());
    let q: Q = s.query();
    let lnet: Id<LogicalNet> = id(&ids, "logical_nets|DP");
    let members = q.get::<NetMembersOf>(lnet);
    assert_eq!(members.ports, [id::<Function>(&ids, "functions|-201U1#bus")]);
    match members.master {
        Some(NetMaster::External { ip, external }) => {
            assert_eq!(ip, id::<Function>(&ids, "functions|DP@3"));
            assert_eq!(external_partner_text(&q, external), "+J-E-206 / JE-N1000-03");
        }
        other => panic!("the master is the external partner, not {other:?}"),
    }
    assert!(
        s.resolve_reference("JE-N1000-03").is_err(),
        "the partner is no device"
    );
    for mode in [BomMode::Component, BomMode::Assembly] {
        let bom = s.export_csv(Export::Bom(mode)).expect("a BOM");
        assert!(!bom.contains("JE-N1000-03"), "{mode:?}: {bom}");
    }
}

/// BU-5's derived half (the page bytes are the committed expected files): the station port prints "Node TBD" and the
/// plug's termination derives "Switch to "ON"" (drawn by no slot of folio 3 yet).
#[test]
fn bu5_the_port_texts() {
    let ids = ids();
    let s = golden_common::session(&golden_common::batch());
    let q: Q = s.query();
    let functions = q.functions();
    let port = functions
        .get(id(&ids, "functions|-201U1#bus"))
        .expect("the station port");
    let plug = functions.get(id(&ids, "functions|-3X1")).expect("the plug");
    let port = port.bus_port.as_ref().expect("a bus port");
    let plug = plug.bus_port.as_ref().expect("a bus port");
    assert_eq!(bus_address_text(port).as_deref(), Some("Node TBD"));
    assert_eq!(bus_termination_text(port), None, "the station port has no switch");
    assert_eq!(bus_address_text(plug), None, "a passive port has no address");
    assert_eq!(bus_termination_text(plug).as_deref(), Some("Switch to \"ON\""));
}

/// BU-6 = AC-31 (D12; doc 20 §18.1 EF-03, EF-16): with -3X1 On, EW-BUS-004 reports no error and one info for the
/// external end; EW-BUS-001 reports the TBD station address of -201U1:DP once, and EW-CORE-044's finding for it is
/// superseded. BU-3: with -3X1 Off, EW-BUS-004 reports one error on -3X1 (the switch prints "Switch to "OFF""), the
/// info stays, and On again clears the error.
#[test]
fn bu6_ac31_the_bus_checks_on_golden_a() {
    let ids = ids();
    let mut s = golden_common::session(&golden_common::batch());
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert_eq!(
        open(&report, "EW-BUS-004"),
        [(Severity::Info, EXTERNAL_END.to_owned(), "external".to_owned())]
    );
    assert_eq!(
        open(&report, "EW-BUS-001"),
        [(
            Severity::Error,
            "=P01+J-E-2416-201U1:DP".to_owned(),
            "address".to_owned()
        )]
    );
    assert_eq!(open(&report, "EW-CORE-044"), [], "superseded by EW-BUS-001");
    assert_eq!(report.exit_code(), 1, "EF-03 blocks release");

    let plug: Id<Function> = id(&ids, "functions|-3X1");
    set_termination(&mut s, plug, "off");
    let text = s
        .query()
        .functions()
        .get(plug)
        .and_then(|f| f.bus_port.as_ref().and_then(bus_termination_text));
    assert_eq!(text.as_deref(), Some("Switch to \"OFF\""));
    let off = s.check(&PRE_DELIVERY, CheckMode::Release);
    let mut got = open(&off, "EW-BUS-004");
    got.sort();
    let mut want = vec![
        (Severity::Error, "=P01+J-E-2416-3X1".to_owned(), "end".to_owned()),
        (Severity::Info, EXTERNAL_END.to_owned(), "external".to_owned()),
    ];
    want.sort();
    assert_eq!(got, want);

    set_termination(&mut s, plug, "on");
    let on = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert_eq!(
        open(&on, "EW-BUS-004"),
        [(Severity::Info, EXTERNAL_END.to_owned(), "external".to_owned())]
    );
}
