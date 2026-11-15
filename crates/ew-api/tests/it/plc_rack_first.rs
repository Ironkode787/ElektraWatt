// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F17, F18 and F19 of the rc.1 test, on the starter parts: a device that nothing describes as a PLC module is found
//! by EW-PLC-017 and taken out of the rack by its quick fix; a copy of a valve terminal is a station again; a station
//! that is its own module is one row of the PLC navigator, a station drawn nowhere says so; `plc.add_station` makes a
//! station with its modules in one undo step, or a bus device a station of a PLC.

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::output::{NOT_DRAWN, NavKind, NavRole};
use ew_api::{AnyCommand, Origin, Session};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, StationRole};
use serde_json::{Value, json};

use crate::common::{library_part, new_project};
use crate::plc_window::place;

fn run(s: &mut Session, name: &str, params: Value) -> Result<(), String> {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .map(|_| ())
    .map_err(|e| format!("{e}"))
}

fn device(s: &Session, id: Uuid) -> Option<std::sync::Arc<Device>> {
    s.query().devices().get(Id::from_uuid(id)).cloned()
}

fn channels(s: &Session, module: Uuid) -> usize {
    s.query()
        .table::<ew_schema::Channel>()
        .iter()
        .filter(|(_, c)| c.module.as_uuid() == module)
        .count()
}

fn plc_017(s: &Session) -> Vec<ew_api::checks::Finding> {
    s.check(&PRE_DELIVERY, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-PLC-017")
        .collect()
}

/// The library documents `parts` need that the project lacks, each once.
fn copies(s: &Session, parts: &[Uuid]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for p in parts {
        for d in s.module_part_copies(*p) {
            if !out.contains(&d) {
                out.push(d);
            }
        }
    }
    out
}

/// F17: the owner's state, reached as the first candidate let it be reached: a valve terminal's copy kept the
/// station's own module without the station (here: the station removed), its part became a bus node's without PLC
/// data, and it was attached to the CPU. The window no longer offers it as a card; EW-PLC-017 names it; its fix takes
/// it out of the rack with the eight channels no part gives, and the project is sound again.
#[test]
fn a_bus_node_in_the_rack_is_found_and_taken_out_again() {
    let mut s = new_project(171);
    let cpu = place(&mut s, "EW-GEN-CPU-1", 1);
    let node = place(&mut s, "EW-GEN-VTM-1", 2);
    assert_eq!(channels(&s, node), 8, "the terminal's channels");
    run(&mut s, "fields.set", json!({"kind": "device", "targets": [node.to_string()],
        "path": "plc_station", "value": null}))
    .expect("the station removed");
    assert!(
        !s.plc_cards_without_cpu().iter().any(|(c, _)| *c == node),
        "a station's own module is no card"
    );
    let pnd = library_part(&s, "EW-GEN-PND-1");
    let c = copies(&s, &[pnd]);
    run(&mut s, "fields.set", json!({"kind": "device", "targets": [node.to_string()], "path": "parts",
        "value": [{"part": {"copy": pnd.to_string()}, "qty": "1"}], "copies": c}))
    .expect("the bus node's part");
    run(&mut s, "assembly.attach", json!({"parent": cpu.to_string(),
        "children": [{"device": node.to_string(), "position": 1}]}))
    .expect("attached as the old window did");
    assert!(s.not_a_module(node));

    let found = plc_017(&s);
    let [f] = found.as_slice() else {
        panic!("one finding: {found:#?}")
    };
    assert!(f.message.contains("describes no PLC module"), "{}", f.message);
    let fix = f.fix.clone().expect("a quick fix");
    run(&mut s, &fix.command, fix.params).expect("the fix");
    let d = device(&s, node).expect("the device stays");
    assert_eq!(d.parent, None, "out of the rack");
    assert!(d.aspects.plc_module.is_none());
    assert_eq!(channels(&s, node), 0, "no channel that no part gives");
    assert!(plc_017(&s).is_empty());
    assert!(s.bus_device(node), "a bus device, to be given to a PLC as a station");
}

/// F17: a copy of a valve terminal is a valve terminal, a station of its own, not the terminal's own module alone.
#[test]
fn a_pasted_valve_terminal_is_a_station_again() {
    let mut s = new_project(172);
    let vt = place(&mut s, "EW-GEN-VTM-1", 1);
    let q = s.query();
    let reps: Vec<Uuid> = q
        .idx()
        .functions_of_device(Id::from_uuid(vt))
        .flat_map(|f| q.idx().reps_of_function(f).collect::<Vec<_>>())
        .map(|r| r.as_uuid())
        .collect();
    drop(q);
    let text = s.copy_fragment(&reps).expect("copied").text;
    let fragment = ew_api::window::parse_fragment(&text).expect("reads").params;
    let page = crate::common::first_page(&s);
    // 24 steps of the page's connection grid (7937500 nm each) to the right
    let x = fragment["anchor"][0].as_i64().expect("x") + 24 * 7_937_500;
    let y = fragment["anchor"][1].as_i64().expect("y");
    let cmd = AnyCommand {
        name: "edit.paste".into(),
        params: json!({"page": page.to_string(), "at": [x, y], "fragment": fragment}),
    };
    let preview = s.preview(&cmd).expect("the paste");
    s.commit(preview).expect("committed");
    let q = s.query();
    let pasted: Vec<_> = q
        .devices()
        .iter()
        .filter(|(id, d)| id.as_uuid() != vt && d.parts.len() == 1 && d.aspects.plc_module.is_some())
        .map(|(_, d)| d.clone())
        .collect();
    let [copy] = pasted.as_slice() else {
        panic!("one copy: {pasted:?}")
    };
    assert_eq!(
        copy.aspects.plc_station.as_ref().map(|p| p.role.clone()),
        Some(StationRole::ValveTerminal)
    );
}

/// F17: a valve terminal is one row of the PLC navigator with its channels under it; a station drawn on no page carries
/// the badge `not drawn`.
#[test]
fn a_station_that_is_its_own_module_is_one_row() {
    let mut s = new_project(173);
    let vt = place(&mut s, "EW-GEN-VTM-1", 1);
    let t = s.navigator_table(NavKind::Plc);
    let at = t
        .nodes
        .iter()
        .position(|n| n.role == NavRole::Station && n.records.first() == Some(&vt))
        .expect("the station row");
    let children: Vec<NavRole> = t
        .nodes
        .iter()
        .filter(|n| n.parent == u32::try_from(at).ok())
        .map(|n| n.role)
        .collect();
    assert!(!children.contains(&NavRole::Module), "{children:?}");
    assert!(children.contains(&NavRole::Channel), "{children:?}");
    assert!(!t.nodes[at].badges.iter().any(|b| b == NOT_DRAWN), "drawn");

    let cpu = library_part(&s, "EW-GEN-CPU-1");
    let c = copies(&s, &[cpu]);
    run(&mut s, "plc.add_station", json!({"part": {"part": {"copy": cpu.to_string()}, "qty": "1"},
        "copies": c}))
    .expect("a station not drawn");
    let t = s.navigator_table(NavKind::Plc);
    let undrawn = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Station && n.records.first() != Some(&vt))
        .expect("the new station");
    assert!(undrawn.badges.iter().any(|b| b == NOT_DRAWN), "{:?}", undrawn.badges);
}

/// F18: `plc.add_station` makes the CPU with its two modules, drawn nowhere, in a new configuration project, in one
/// undo step; the modules have their channels and addresses.
#[test]
fn a_new_station_is_made_with_its_modules_in_one_step() {
    let mut s = new_project(174);
    let before = s.world().model;
    let [cpu, di, dout] = ["EW-GEN-CPU-1", "EW-GEN-DI-1", "EW-GEN-DO-1"].map(|n| library_part(&s, n));
    let part = |p: Uuid| json!({"part": {"part": {"copy": p.to_string()}, "qty": "1"}});
    let c = copies(&s, &[cpu, di, dout]);
    run(&mut s, "plc.add_station", json!({
        "part": part(cpu)["part"], "modules": [part(di), part(dout)], "copies": c}))
    .expect("the station");
    let q = s.query();
    let (station, d) = q
        .devices()
        .iter()
        .find(|(_, d)| d.aspects.plc_station.is_some())
        .map(|(id, d)| (id.as_uuid(), d.clone()))
        .expect("the station");
    drop(q);
    assert_eq!(d.aspects.plc_station.as_ref().map(|p| p.role.clone()), Some(StationRole::Cpu));
    let view = s.rack_view(station).expect("its rack");
    let modules: Vec<_> = view.rows.iter().filter_map(|r| r.module).collect();
    assert_eq!(modules.len(), 2, "{:?}", view.rows);
    for m in &modules {
        assert_eq!(channels(&s, *m), 8);
    }
    assert!(view.rows.iter().any(|r| r.gives == "8 DI"), "{:?}", view.rows);
    assert!(view.rows.iter().any(|r| r.addresses.starts_with('Q')), "{:?}", view.rows);
    s.undo(&Origin::Gui).expect("one undo");
    assert!(s.world().model.content_eq(&before), "one step");
}

/// F19: a bus device whose part says nothing of a PLC becomes a station of a PLC with an open rack: an I/O device that
/// takes modules; a module part is refused as a station.
#[test]
fn a_bus_device_is_given_to_a_plc() {
    let mut s = new_project(175);
    let cpu = place(&mut s, "EW-GEN-CPU-1", 1);
    let cfg = s.plc_config_projects()[0].0;
    let pnd = library_part(&s, "EW-GEN-PND-1");
    let c = copies(&s, &[pnd]);
    run(&mut s, "plc.add_station", json!({"part": {"part": {"copy": pnd.to_string()}, "qty": "1"},
        "copies": c}))
    .expect_err("a part without PLC data is no station part");
    let card = place(&mut s, "EW-GEN-DI-1", 2);
    assert!(!s.bus_device(card), "a module is no bus device");
    run(&mut s, "plc.add_station", json!({"device": card.to_string(), "config_project": cfg.to_string()}))
        .expect_err("a module goes into a rack");
    assert!(!s.bus_device(cpu), "the CPU is a station already");

    run(&mut s, "device.create", json!({"letter": "A", "counter": 9})).expect("a device");
    let node = s
        .query()
        .devices()
        .iter()
        .find(|(_, d)| d.tag.counter == Some(9) && d.parts.is_empty())
        .map(|(id, _)| id.as_uuid())
        .expect("the device");
    let c = copies(&s, &[pnd]);
    run(&mut s, "fields.set", json!({"kind": "device", "targets": [node.to_string()], "path": "parts",
        "value": [{"part": {"copy": pnd.to_string()}, "qty": "1"}], "copies": c}))
    .expect("its part: a PROFINET device without PLC data");
    assert!(s.bus_device(node));
    run(&mut s, "plc.add_station", json!({"device": node.to_string(), "config_project": cfg.to_string()}))
        .expect("a station of PLC1");
    let d = device(&s, node).expect("the device");
    let station = d.aspects.plc_station.as_ref().expect("a station");
    assert_eq!(station.role, StationRole::IoDevice);
    assert_eq!(station.config_project.as_uuid(), cfg);
    let view = s.rack_view(node).expect("its rack");
    assert!(!view.described, "the part does not describe its modules");
    assert_eq!(view.rows.iter().filter(|r| r.module.is_none()).count(), 1, "an open rack: one free row");
    let di = library_part(&s, "EW-GEN-DI-1");
    let c = copies(&s, &[di]);
    run(&mut s, "plc.add_module", json!({"station": node.to_string(),
        "part": {"part": {"copy": di.to_string()}, "qty": "1"}, "count": 2, "copies": c}))
    .expect("two modules at once");
    let view = s.rack_view(node).expect("its rack");
    assert_eq!(view.rows.iter().filter(|r| r.module.is_some()).count(), 2);
    assert!(!s.bus_device(node), "a station is not offered again");
}
