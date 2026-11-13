// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fluid 1.0 on Golden A and its A-R fluid overlay (doc 11 §5, §6, §8; doc 20 §2.4, §12, §18.2; WP-S1.25):
//! the EW-FL 1.0 findings of Golden A (AC-22 for EW-FL), and the overlay built with the fluid commands, whose tube
//! list, cut list, medium overview and propagation are AC-32's 1.0 part and the propagation test of doc 11 §11.2.

use crate::golden_common;

use std::collections::BTreeMap;

use ew_api::checks::{CheckMode, DRAFTING, PRE_DELIVERY, Release};
use ew_api::output::{RowExport, row_export};
use ew_api::{AnyCommand, Session, SessionOptions};
use ew_derive::{EndpointKey, FunctionCps, MediumKey, MediumZones, node_net};
use ew_ids::{Id, Uuid};
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, FluidLine, FluidPortRole, FnDef, FnRole, Function, LibDocument,
    Record,
};
use serde_json::json;

/// The open findings of `rule` as `severity subject`, sorted, in a run of `scheme` with the 1.0 content.
fn findings(s: &Session, scheme: &ew_api::checks::Scheme, mode: CheckMode, rule: &str) -> Vec<String> {
    let mut out: Vec<String> = s
        .check(&scheme.with_content(Release::V1_0), mode)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule && f.is_open())
        .map(|f| format!("{} {}", f.severity, f.subject))
        .collect();
    out.sort();
    out
}

/// AC-22 for EW-FL (doc 20 §18.2, doc 11 §11.2): with the 1.0 content, Golden A reports EF-20 (EW-FL-010 once, on the
/// valve terminal -201U1: gallery 1 is fed by nothing) and EF-21 (EW-FL-007 on the 16 lines F1 to F16), and nothing of
/// EW-FL-005, -006, -009 and -012; the pre-delivery scheme keeps EW-FL-008 and EW-FL-014 off, and enabled they give 2
/// (galleries 3 and 5 of -201U1) and 16. The MVP scheme runs none of them.
#[test]
fn golden_a_reports_ef_20_and_ef_21_with_the_1_0_rules() {
    let s = golden_common::session_with_profiles(&golden_common::batch());
    let run = |rule| findings(&s, &PRE_DELIVERY, CheckMode::Release, rule);
    assert_eq!(run("EW-FL-010"), ["warning =P01+J-E-2416-201U1"]);
    let lines = run("EW-FL-007");
    assert_eq!(lines.len(), 16, "{lines:?}");
    assert!(lines.iter().all(|l| l.starts_with("warning -20") && l.contains(" → -203Q")), "{lines:?}");
    for rule in ["EW-FL-005", "EW-FL-006", "EW-FL-009", "EW-FL-012", "EW-FL-008", "EW-FL-014"] {
        assert_eq!(run(rule), Vec::<String>::new(), "{rule}");
    }
    let online = |rule| findings(&s, &DRAFTING, CheckMode::Online, rule);
    assert_eq!(online("EW-FL-008"), ["info =P01+J-E-2416-201U1", "info =P01+J-E-2416-201U1"]);
    assert_eq!(online("EW-FL-014").len(), 16);
    let mvp = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert!(!mvp.findings.iter().any(|f| f.rule == "EW-FL-007" || f.rule == "EW-FL-010"));
}

const LIB: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

/// A function definition copy from its item JSON, in the built-in library.
fn fndef(item: u128, body: serde_json::Value) -> LibDocument {
    let item = Uuid::from_u128(item);
    let id: Id<FnDef> = ew_ids::v5::lib_copy_id(LIB, item);
    let mut doc = json!({"$k": "fndef", "id": id.to_string(), "library_id": LIB.to_string(),
        "item_id": item.to_string(), "version": "0.1.0", "sha256": format!("sha256:{}", "0".repeat(64)),
        "meta": {"licence": "CC-BY-SA-4.0", "sources": ["test"]}});
    for (k, v) in body.as_object().expect("an object") {
        doc[k] = v.clone();
    }
    LibDocument::from_json(&doc.to_string()).expect("the FnDef")
}

/// Golden A with the A-R fluid overlay's parts (doc 20 §2.4, ▲U-25): the supply plates -201U9 and -202U3 get a plate
/// function with ports 1, 3 and 5 tapping galleries 1, 3 and 5; an air-supply point -AS1 with one port feeds -201U9:1
/// through the supply line F17. Everything else of the overlay is done with commands (see the test).
fn overlay() -> (Session, BTreeMap<String, String>, Id<Connection>, Id<Function>) {
    let mut batch = golden_common::batch();
    let ids = golden_common::ids_by_key(&batch);
    let plate_def = fndef(
        0xf1_0001,
        json!({"category": ["ValveTerminal", "SupplyPlate", "Generic"], "contact_kind": "black_box", "cps": [
            {"idx": 0, "designation": "1", "discipline": "pneumatic", "port_role": "supply1"},
            {"idx": 1, "designation": "3", "discipline": "pneumatic", "port_role": "exhaust3"},
            {"idx": 2, "designation": "5", "discipline": "pneumatic", "port_role": "exhaust5"}]}),
    );
    let source_def = fndef(
        0xf1_0002,
        json!({"category": ["AirPreparation", "Source", "Generic"], "contact_kind": "black_box", "cps": [
            {"idx": 0, "designation": "1", "discipline": "pneumatic", "port_role": "generic"}]}),
    );
    let (plate, source) = (lib_id(&plate_def), lib_id(&source_def));
    batch.lib.push(plate_def);
    batch.lib.push(source_def);
    let plate_part: Id<ew_schema::Part> = {
        let d = id(&ids, "devices|-201U9");
        let b = golden_common::session(&golden_common::batch());
        let q = b.query();
        q.devices().get(d).expect("-201U9").parts[0].part.copy
    };
    for doc in batch.lib.iter_mut() {
        if let LibDocument::Part(p) = doc
            && p.id == plate_part
        {
            p.item.sections.fluid = Some(
                serde_json::from_value(json!({"galleries": [
                    {"gallery": "supply1", "tap": "1"}, {"gallery": "exhaust3", "tap": "3"},
                    {"gallery": "exhaust5", "tap": "5"}]}))
                .expect("the gallery data"),
            );
        }
    }
    let f17: Id<Connection> = Id::from_uuid(Uuid::from_u128(0xf1_0017));
    let source_fn: Id<Function> = Id::from_uuid(Uuid::from_u128(0xf1_0101));
    let s = golden_common::session_with(&batch, |p, records| {
        golden_common::with_profiles(p);
        for (n, key) in [(0xf1_0102u128, "devices|-201U9"), (0xf1_0103, "devices|-202U3")] {
            records.push(Record::Function(function(n, id(&ids, key), plate)));
        }
        let source_dev: Id<Device> = Id::from_uuid(Uuid::from_u128(0xf1_0100));
        records.push(Record::Device(
            serde_json::from_value(json!({"id": source_dev.to_string(), "tag": {"letter": "AS", "counter": 1}}))
                .expect("-AS1"),
        ));
        records.push(Record::Function(function(0xf1_0101, source_dev, source)));
        let plate_fn: Id<Function> = Id::from_uuid(Uuid::from_u128(0xf1_0102));
        records.push(Record::Connection(Connection {
            id: f17,
            v: 1,
            a: Endpoint::Cp(source_fn, 0),
            b: Endpoint::Cp(plate_fn, 0),
            page: None,
            route: Vec::new(),
            kind: ConnectionKind::Fluid(FluidLine::default()),
            overrides: Default::default(),
            number: Default::default(),
            source_ref: None,
            extra: Default::default(),
        }));
    });
    (s, ids, f17, source_fn)
}

fn lib_id(d: &LibDocument) -> Id<FnDef> {
    match d {
        LibDocument::Fndef(c) => c.id,
        _ => unreachable!(),
    }
}

fn id<T>(ids: &BTreeMap<String, String>, key: &str) -> Id<T> {
    ids.get(key).unwrap_or_else(|| panic!("no {key}")).parse().expect("an ID")
}

fn function(n: u128, device: Id<Device>, def: Id<FnDef>) -> Function {
    serde_json::from_value(json!({"id": Uuid::from_u128(n).to_string(), "device": device.to_string(),
        "def": def.to_string(), "role": if n == 0xf1_0101 { "main" } else { "aux" }}))
    .expect("a function")
}

fn run(s: &mut Session, name: &str, params: serde_json::Value) -> ew_api::Receipt {
    let cmd = AnyCommand { name: name.into(), params };
    let done = match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    };
    done.unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The cells of a CSV text by header, one map per data row.
fn csv_rows(csv: &str) -> Vec<BTreeMap<String, String>> {
    let mut lines = csv.lines();
    let header: Vec<String> = lines.next().unwrap_or_default().split(',').map(str::to_owned).collect();
    lines
        .map(|l| header.iter().cloned().zip(l.split(',').map(str::to_owned)).collect())
        .collect()
}

/// AC-32's 1.0 part and the propagation test (doc 11 §11.2, doc 20 §12): on the A-R overlay, the medium "compressed
/// air 6 bar" defined at the air-supply point reaches -201U9:1 through F17 and port 1 of all 28 valves through gallery
/// 1, ports 3 and 5 as exhaust; the default tube PU 6×4 BU and 1,200 mm on F1 to F17 give a tube list of 17 rows and
/// a cut list of one line of 17 pieces, 20.4 m; the medium overview has one row with 28 consumers; plugs on the spare
/// valves' working ports and silencers on the plates' exhausts clear EF-13, EF-20 and EF-21, and EF-14 stays.
#[test]
fn the_a_r_overlay_feeds_every_valve_and_lists_17_tubes() {
    let (mut s, _ids, f17, source_fn) = overlay();
    let made = run(
        &mut s,
        "fluid.medium.edit",
        json!({"set": {"name": "compressed air 6 bar", "p_nom": "6", "quality": "1-4-1", "colour": ["BU"]}}),
    );
    let medium = made.outcome.primary.expect("the medium").1;
    run(&mut s, "fluid.medium.define", json!({"function": source_fn.to_string(), "idx": 0, "medium": medium.to_string()}));
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "fluid", "value": {"default_tube": {"od": 6000000, "id": 4000000, "material": "pu", "colour": ["BU"]}}}),
    );
    let lines: Vec<String> = {
        let q = s.query();
        q.connections()
            .iter()
            .filter(|(_, c)| matches!(c.kind, ConnectionKind::Fluid(_)))
            .map(|(id, _)| id.to_string())
            .collect()
    };
    assert_eq!(lines.len(), 17);
    run(&mut s, "fluid.length.set", json!({"lines": lines, "length": 1_200_000_000}));
    // Plugs on the spare valves' working ports, silencers on the plates' exhausts.
    let closures: Vec<(Id<Function>, u16, &str)> = {
        let q = s.query();
        let mut out = Vec::new();
        for (f, func) in q.functions().iter() {
            let spare = func
                .device
                .and_then(|d| q.devices().get(d))
                .is_some_and(|d| d.state == ew_schema::DeviceState::Spare);
            for cp in q.get::<FunctionCps>(f).iter() {
                match cp.port_role {
                    Some(FluidPortRole::Working2 | FluidPortRole::Working4) if spare => out.push((f, cp.idx, "plug")),
                    Some(FluidPortRole::Exhaust3 | FluidPortRole::Exhaust5)
                        if func.def == ew_ids::v5::lib_copy_id::<FnDef>(LIB, Uuid::from_u128(0xf1_0001)) =>
                    {
                        out.push((f, cp.idx, "silencer"))
                    }
                    _ => {}
                }
            }
        }
        out
    };
    assert_eq!(closures.len(), 44, "40 plugs and 4 silencers");
    for (f, idx, kind) in closures {
        run(&mut s, "fluid.port.close", json!({"function": f.to_string(), "idx": idx, "closure": {"kind": kind}}));
    }

    // Propagation.
    let q = s.query();
    let zones = q.get::<MediumZones>(());
    let medium: Id<ew_schema::Medium> = medium.cast();
    let carried = |f: Id<Function>, role: FluidPortRole| -> Vec<MediumKey> {
        let cp = q.get::<FunctionCps>(f).iter().find(|c| c.port_role == Some(role.clone())).map(|c| c.idx);
        let Some(net) = cp.and_then(|i| node_net(&q, EndpointKey::Cp(f, i))) else { return Vec::new() };
        zones.keys(net).map(|(k, _)| k.clone()).collect()
    };
    let valve_fns: Vec<Id<Function>> = q
        .devices()
        .ids()
        .flat_map(|d| ew_derive::valve_functions(&q, d))
        .filter(|f| q.get::<FunctionCps>(*f).iter().any(|c| c.port_role == Some(FluidPortRole::Supply1)))
        .collect();
    assert_eq!(valve_fns.len(), 28);
    for f in &valve_fns {
        assert!(carried(*f, FluidPortRole::Supply1).contains(&MediumKey::Defined(medium)), "port 1");
        assert!(carried(*f, FluidPortRole::Exhaust3).contains(&MediumKey::Exhaust(medium)), "port 3");
        assert!(carried(*f, FluidPortRole::Exhaust5).contains(&MediumKey::Exhaust(medium)), "port 5");
    }
    drop(q);

    // The tube list, the cut list and the medium overview.
    let tubes = row_export(&s.query(), &RowExport::Kind("tube_list".into()), None).expect("the tube list");
    let rows = csv_rows(&tubes.csv);
    assert_eq!(rows.len(), 17, "{}", tubes.csv);
    for r in &rows {
        assert_eq!(
            (r["OD×ID"].as_str(), r["Material"].as_str(), r["Colour"].as_str(), r["Length"].as_str(), r["Source"].as_str()),
            ("6×4", "PU", "BU", "1.2 m", "manual"),
            "{r:?}"
        );
        assert_eq!(r["Medium"], "compressed air 6 bar");
    }
    assert!(rows.iter().any(|r| r["To"] == "-201U9:1"), "F17 ends at -201U9:1");
    let cut = row_export(&s.query(), &RowExport::Kind("tube_cut_list".into()), None).expect("the cut list");
    let cut = csv_rows(&cut.csv);
    assert_eq!(cut.len(), 1);
    assert_eq!((cut[0]["Pieces"].as_str(), cut[0]["Total length"].as_str()), ("17", "20.4 m"));
    let media = row_export(&s.query(), &RowExport::Kind("medium_overview".into()), None).expect("the overview");
    let media = csv_rows(&media.csv);
    assert_eq!(media.len(), 1);
    assert_eq!(media[0]["Count"], "28");

    // EF-13, EF-20 and EF-21 clear; EF-14 stays.
    let run1 = |rule| findings(&s, &PRE_DELIVERY, CheckMode::Release, rule);
    for rule in ["EW-FL-001", "EW-FL-007", "EW-FL-010", "EW-FL-005", "EW-FL-006", "EW-FL-012"] {
        assert_eq!(run1(rule), Vec::<String>::new(), "{rule}");
    }
    assert_eq!(run1("EW-FL-004").len(), 20);
    let _ = (f17, FnRole::Main, SessionOptions::default());
}
