// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The connectivity emitters on tiny in-memory tables: connections, junctions (CI-04), conductors,
//! potentials and bonds.

mod common;

use ew_ids::Id;
use ew_import_tables::ids::record_id;
use ew_interop_api::{Code, ImportBatch, Severity};
use ew_schema::{
    BridgeKind, Conductor, Connection, ConnectionKind, Endpoint, Function, Junction, JunctionStyle,
    JunctionWiring, Record,
};
use ew_units::Grid;

const PAGES: &str = "key,number,kind,form,grid\n101,101,multi_line,ew.frame.qet-a4-landscape-grid,qet10\n";
const DEVICES: &str = "key,letter,counter,cable\n-1F1,F,1,\n-2F1,F,2,\n-3F1,F,3,\n-1W1,W,1,true\n";
const FUNCTIONS: &str =
    "key,device,fndef\n-1F1,-1F1,ew.fn.mcb-1pn\n-2F1,-2F1,ew.fn.mcb-1pn\n-3F1,-3F1,ew.fn.mcb-1pn\n";

/// The base tables plus `extra` (`(file, text)`), imported.
fn run(extra: &[(&str, &str)]) -> ImportBatch {
    let mut files = vec![
        ("project.csv", common::PROJECT_CSV),
        ("pages.csv", PAGES),
        ("devices.csv", DEVICES),
        ("functions.csv", FUNCTIONS),
    ];
    files.extend_from_slice(extra);
    common::import(&common::inputs(&files)).expect("the import succeeds")
}

fn function(key: &str) -> Id<Function> {
    record_id("functions", key)
}

fn cp(device: &str, idx: u16) -> Endpoint {
    Endpoint::Cp(function(device), idx)
}

fn connection<'a>(batch: &'a ImportBatch, key: &str) -> Option<&'a Connection> {
    let id: Id<Connection> = record_id("connections", key);
    batch.records.iter().find_map(|r| match r {
        Record::Connection(c) if c.id == id => Some(c),
        _ => None,
    })
}

fn junction<'a>(batch: &'a ImportBatch, key: &str) -> Option<&'a Junction> {
    let id: Id<Junction> = record_id("junctions", key);
    batch.records.iter().find_map(|r| match r {
        Record::Junction(j) if j.id == id => Some(j),
        _ => None,
    })
}

fn count(batch: &ImportBatch, code: u16, severity: Severity) -> usize {
    batch
        .report
        .with_code(Code::imp(code))
        .filter(|e| e.severity == severity)
        .count()
}

fn messages(batch: &ImportBatch, code: u16) -> Vec<String> {
    batch
        .report
        .with_code(Code::imp(code))
        .map(|e| e.message.clone())
        .collect()
}

/// The three connections of a star junction `J1` (feed, then two arms).
const STAR_CONNECTIONS: &str = "key,page,a,b\n\
    J1.feed,101,-1F1:2,@J1\n\
    W2,101,@J1,-2F1:1\n\
    W3,101,@J1,-3F1:1\n";

#[test]
fn a_direct_wire_is_emitted_between_the_cps() {
    let batch = run(&[("connections.csv", "key,page,a,b\nW1,101,-1F1:2,-2F1:N#2\n")]);
    assert!(!batch.report.has_errors(), "{:?}", batch.report.entries);
    let c = connection(&batch, "W1").expect("W1 is emitted");
    assert_eq!(c.a, cp("-1F1", 1));
    assert_eq!(c.b, cp("-2F1", 3));
    assert_eq!(c.kind, ConnectionKind::Wire);
    assert!(c.page.is_some());
    assert!(c.route.is_empty());
    assert_eq!(
        c.source_ref.as_ref().map(|s| s.key.as_str()),
        Some("connections|W1")
    );
}

#[test]
fn wire_overrides_are_read() {
    let batch = run(&[(
        "connections.csv",
        "key,a,b,csa,colour,show_marker,wire\nW1,-1F1:1,-2F1:1,2.5,BK,true,W1\n",
    )]);
    assert!(!batch.report.has_errors(), "{:?}", batch.report.entries);
    let c = connection(&batch, "W1").unwrap();
    assert!(c.overrides.csa.is_some());
    assert!(c.overrides.colour.is_some());
    assert!(c.overrides.show_marker);
    assert!(c.page.is_none());
}

#[test]
fn a_star_junction_loads_consistently() {
    let batch = run(&[
        ("connections.csv", STAR_CONNECTIONS),
        (
            "junctions.csv",
            "key,page,cell,style,wiring,sequence\nJ1,101,C11,dot,star,J1.feed;W3;W2\n",
        ),
    ]);
    assert!(!batch.report.has_errors(), "{:?}", batch.report.entries);
    assert_eq!(count(&batch, 2, Severity::Warning), 0);
    let j = junction(&batch, "J1").expect("J1 is emitted");
    assert_eq!(j.style, JunctionStyle::Dot);
    assert_eq!(j.wiring, Some(JunctionWiring::Star));
    let expected: Vec<Id<Connection>> = ["J1.feed", "W3", "W2"]
        .iter()
        .map(|k| record_id("connections", k))
        .collect();
    assert_eq!(j.sequence, expected, "the sequence order is kept");
    let jid = Endpoint::Junction(j.id);
    assert_eq!(connection(&batch, "J1.feed").unwrap().b, jid);
    assert_eq!(connection(&batch, "W2").unwrap().a, jid);
    assert_eq!(connection(&batch, "W3").unwrap().a, jid);
}

#[test]
fn a_bevel_junction_without_wiring_uses_the_default() {
    let batch = run(&[
        (
            "connections.csv",
            "key,page,a,b\nJ1.1,101,-1F1:2,@J1\nW2,101,@J1,-2F1:1\n",
        ),
        (
            "junctions.csv",
            "key,page,cell,style,sequence\nJ1,101,C6,bevel,J1.1;W2\n",
        ),
    ]);
    assert!(!batch.report.has_errors(), "{:?}", batch.report.entries);
    let j = junction(&batch, "J1").unwrap();
    assert!(matches!(j.style, JunctionStyle::Bevel(_)));
    assert_eq!(j.wiring, None);
}

#[test]
fn a_junction_missing_a_connection_is_dropped_with_its_connections() {
    let batch = run(&[
        ("connections.csv", STAR_CONNECTIONS),
        (
            "junctions.csv",
            "key,page,cell,style,sequence\nJ1,101,C11,dot,J1.feed;W2\n",
        ),
    ]);
    assert!(junction(&batch, "J1").is_none());
    assert!(!batch.records.iter().any(|r| matches!(r, Record::Junction(_))));
    for key in ["J1.feed", "W2", "W3"] {
        assert!(
            connection(&batch, key).is_none(),
            "{key} is dropped with the junction"
        );
    }
    let dropped: Vec<_> = batch
        .report
        .with_code(Code::imp(3))
        .filter(|e| {
            e.message
                .starts_with("row dropped: its junction `J1` was dropped")
        })
        .collect();
    assert_eq!(dropped.len(), 3);
    assert!(dropped.iter().all(|e| e.severity == Severity::Error));
    assert!(
        dropped
            .iter()
            .all(|e| e.location.as_ref().is_some_and(|l| l.file == "connections.csv"))
    );
    // The junction row itself is reported once.
    assert_eq!(count(&batch, 2, Severity::Error), 1);
}

#[test]
fn a_junction_naming_an_unknown_connection_is_dropped() {
    let batch = run(&[
        ("connections.csv", "key,page,a,b\nW1,101,-1F1:2,@J1\n"),
        (
            "junctions.csv",
            "key,page,cell,style,sequence\nJ1,101,C11,dot,W1;nope\n",
        ),
    ]);
    assert!(junction(&batch, "J1").is_none());
    assert!(connection(&batch, "W1").is_none());
    assert!(
        messages(&batch, 3)
            .iter()
            .any(|m| m.contains("unknown connection `nope`"))
    );
}

#[test]
fn a_junction_on_an_unknown_page_is_reported_and_connections_to_it_fail() {
    let batch = run(&[
        ("connections.csv", "key,a,b\nW1,-1F1:2,@J1\n"),
        (
            "junctions.csv",
            "key,page,cell,style,sequence\nJ1,999,C11,dot,W1\n",
        ),
    ]);
    assert!(junction(&batch, "J1").is_none());
    assert!(connection(&batch, "W1").is_none());
    let msgs = messages(&batch, 3);
    assert!(
        msgs.iter().any(|m| m.contains("unknown junction `J1`")),
        "{msgs:?}"
    );
    assert!(msgs.iter().any(|m| m.contains("unknown page `999`")), "{msgs:?}");
}

#[test]
fn a_dropped_junction_cascades_to_a_junction_linked_to_it() {
    let batch = run(&[
        (
            "connections.csv",
            "key,page,a,b\nF,101,-1F1:2,@J1\nX,101,@J1,@J2\nY,101,@J2,-2F1:1\n",
        ),
        (
            "junctions.csv",
            "key,page,cell,style,sequence\nJ1,101,C6,square,F;X\nJ2,101,C8,dot,X;Y\n",
        ),
    ]);
    assert!(
        !batch
            .records
            .iter()
            .any(|r| matches!(r, Record::Junction(_) | Record::Connection(_)))
    );
    assert!(batch.report.has_errors());
}

#[test]
fn unknown_device_unknown_cp_and_ambiguous_cp_are_unresolved() {
    let batch = run(&[(
        "connections.csv",
        "key,a,b\nW1,-9F1:1,-2F1:1\nW2,-1F1:9,-2F1:1\nW3,-1F1:N,-2F1:1\nW4,-1F1:1,-2F1:1\n",
    )]);
    assert_eq!(count(&batch, 3, Severity::Error), 3);
    for key in ["W1", "W2", "W3"] {
        assert!(connection(&batch, key).is_none(), "{key} is absent");
    }
    assert!(connection(&batch, "W4").is_some());
    let msgs = messages(&batch, 3);
    assert!(
        msgs.iter().any(|m| m.contains("unknown device `-9F1`")),
        "{msgs:?}"
    );
    assert!(msgs.iter().any(|m| m.contains("no CP `9`")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("ambiguous")), "{msgs:?}");
}

#[test]
fn a_duplicate_connection_key_keeps_the_first_row_only() {
    let batch = run(&[("connections.csv", "key,a,b\nW1,-1F1:1,-2F1:1\nW1,-1F1:2,-2F1:2\n")]);
    assert_eq!(count(&batch, 2, Severity::Error), 1);
    let all: Vec<_> = batch
        .records
        .iter()
        .filter(|r| matches!(r, Record::Connection(_)))
        .collect();
    assert_eq!(all.len(), 1);
    assert_eq!(connection(&batch, "W1").unwrap().a, cp("-1F1", 0));
}

#[test]
fn equal_endpoints_are_malformed() {
    let batch = run(&[("connections.csv", "key,a,b\nW1,-1F1:1,-1F1:1\n")]);
    assert!(connection(&batch, "W1").is_none());
    assert!(messages(&batch, 2).iter().any(|m| m.contains("CI-01")));
}

#[test]
fn an_unsupported_kind_and_an_unknown_page_are_refused() {
    let batch = run(&[(
        "connections.csv",
        "key,a,b,kind,page\nW1,-1F1:1,-2F1:1,bus,\nW2,-1F1:1,-2F1:2,,777\n",
    )]);
    assert!(connection(&batch, "W1").is_none());
    assert!(connection(&batch, "W2").is_none());
    assert!(messages(&batch, 2).iter().any(|m| m.contains("not supported")));
    assert!(
        messages(&batch, 3)
            .iter()
            .any(|m| m.contains("unknown page `777`"))
    );
}

#[test]
fn an_off_grid_junction_is_snapped_and_warned() {
    let batch = run(&[
        (
            "connections.csv",
            "key,page,a,b\nJ1.1,101,-1F1:2,@J1\nW2,101,@J1,-2F1:1\n",
        ),
        (
            "junctions.csv",
            "key,page,cell,dx,dy,style,sequence\nJ1,101,C11,5,,dot,J1.1;W2\n",
        ),
    ]);
    assert_eq!(count(&batch, 2, Severity::Warning), 1);
    assert!(!batch.report.has_errors(), "{:?}", batch.report.entries);
    let j = junction(&batch, "J1").expect("still emitted");
    let grid = Grid::QET_10PX;
    for v in [j.at.x, j.at.y] {
        assert_eq!(grid.at(grid.snap(v)), Some(v), "on the page grid");
    }
}

#[test]
fn a_duplicate_junction_key_is_reported_once() {
    let batch = run(&[
        (
            "connections.csv",
            "key,page,a,b\nJ1.1,101,-1F1:2,@J1\nW2,101,@J1,-2F1:1\n",
        ),
        (
            "junctions.csv",
            "key,page,cell,style,sequence\nJ1,101,C6,dot,J1.1;W2\nJ1,101,C7,dot,J1.1;W2\n",
        ),
    ]);
    assert_eq!(count(&batch, 2, Severity::Error), 1);
    assert!(junction(&batch, "J1").is_some());
    assert!(connection(&batch, "W2").is_some());
}

#[test]
fn a_core_connection_needs_a_known_conductor() {
    let batch = run(&[
        (
            "conductors.csv",
            "key,cable,core,colour,csa,shield,pair\n-1W1:A,-1W1,A,BK,1.5,,1\n-1W1:S,-1W1,S,,,true,\n",
        ),
        (
            "connections.csv",
            "key,a,b,kind,core\nW1,-1F1:1,-2F1:1,core,-1W1:A\nW2,-1F1:2,-2F1:2,core,-1W1:Z\nW3,-1F1:1,-3F1:1,core,\n",
        ),
    ]);
    assert!(!batch.records.is_empty());
    let id: Id<Conductor> = record_id("conductors", "-1W1:A");
    assert_eq!(connection(&batch, "W1").unwrap().kind, ConnectionKind::Core(id));
    assert!(connection(&batch, "W2").is_none());
    assert!(connection(&batch, "W3").is_none());
    assert!(
        messages(&batch, 3)
            .iter()
            .any(|m| m.contains("unknown conductor `-1W1:Z`"))
    );
    assert_eq!(
        count(&batch, 2, Severity::Error),
        1,
        "the empty `core` cell is malformed"
    );
    let cond = batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::Conductor(c) if c.id == id => Some(c),
            _ => None,
        })
        .expect("conductor emitted");
    assert_eq!(cond.cable, record_id("devices", "-1W1"));
    assert_eq!(cond.pair, Some(1));
    assert!(cond.colour.is_some() && cond.csa.is_some());
    let shield = batch
        .records
        .iter()
        .any(|r| matches!(r, Record::Conductor(c) if c.shield));
    assert!(shield);
}

#[test]
fn a_conductor_needs_a_cable_device() {
    let batch = run(&[("conductors.csv", "key,cable,core\nx:A,-1F1,A\ny:A,-9W9,A\n")]);
    assert!(!batch.records.iter().any(|r| matches!(r, Record::Conductor(_))));
    assert_eq!(count(&batch, 3, Severity::Error), 2);
    assert!(messages(&batch, 3).iter().any(|m| m.contains("EW-INV-006")));
}

#[test]
fn a_bridge_reads_its_spec() {
    let batch = run(&[(
        "connections.csv",
        "key,a,b,kind,bridge_group,bridge_kind,bridge_shaft\n\
         BR1.1,-1F1:1,-2F1:1,bridge,2,saddle,\n\
         BR1.2,-1F1:2,-2F1:2,bridge,,,\n",
    )]);
    let c = connection(&batch, "BR1.1").unwrap();
    match &c.kind {
        ConnectionKind::Bridge(b) => {
            assert_eq!((b.kind.clone(), b.group, b.shaft), (BridgeKind::Saddle, 2, 1));
        }
        other => panic!("not a bridge: {other:?}"),
    }
    assert!(connection(&batch, "BR1.2").is_none(), "bridge_group is required");
}

#[test]
fn a_bond_via_names_a_known_connection() {
    let batch = run(&[
        ("connections.csv", "key,a,b\nW27,-1F1:1,-2F1:1\n"),
        (
            "bonds.csv",
            "key,a,b,kind,via\nB1,-1F1:2,-2F1:2,pelv_0v_pe,W27\nB2,-1F1:2,-3F1:2,equipotential,W99\nB3,-1F1:2,-1F1:2,fe_pe,\n",
        ),
    ]);
    let via: Id<Connection> = record_id("connections", "W27");
    let bonds: Vec<_> = batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Bond(b) => Some(b),
            _ => None,
        })
        .collect();
    assert_eq!(bonds.len(), 1);
    assert_eq!(bonds[0].via, Some(via));
    assert_eq!(bonds[0].a, cp("-1F1", 1));
    assert!(
        messages(&batch, 3)
            .iter()
            .any(|m| m.contains("unknown connection `W99`"))
    );
    assert_eq!(
        count(&batch, 2, Severity::Error),
        1,
        "the bond with equal endpoints"
    );
}

#[test]
fn a_potential_reads_its_environment() {
    let batch = run(&[(
        "potentials.csv",
        "key,at,ptype,voltage,ac_dc,frequency,earthing,circuit_class\n\
         +24V,-1F1:2,l_plus,24,dc,,pelv,control\n\
         L-S,-2F1:1,,,,,,\n\
         \" lead\",-2F1:1,,,,,,\n\
         X1,-9F1:1,,,,,,\n",
    )]);
    let potentials: Vec<_> = batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::PotentialDef(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(potentials.len(), 2);
    let p = potentials
        .iter()
        .find(|p| p.name.as_str() == "+24V")
        .expect("+24V emitted");
    assert_eq!(p.at, cp("-1F1", 1));
    assert!(p.ptype.is_some() && p.voltage.is_some() && p.circuit_class.is_some());
    assert!(p.env.ac_dc.is_some() && p.env.earthing.is_some());
    let naming = potentials.iter().find(|p| p.name.as_str() == "L-S").unwrap();
    assert!(naming.ptype.is_none());
    assert_eq!(count(&batch, 2, Severity::Error), 1);
    assert_eq!(count(&batch, 3, Severity::Error), 1);
}
