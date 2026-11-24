// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! On the sample: the valves of the valve terminal -201K1 with their port roles and internal coils (FL-2), the derived
//! valve addresses from the coil functions (FL-5), the closed ports of the spare valves, the drives links the working
//! lines make, the function-text slot's process-tag fallback and the routes of the tubes on folio 201.

use crate::common;

use ew_api::output::page_view;
use ew_api::{AnyCommand, Session};
use ew_derive::{FunctionCps, PageOrder};
use ew_ids::Id;
use ew_schema::{ConnectionKind, ContactKind, Device, Endpoint, FluidPortRole, Record};
use ew_units::{Grid, Pt2};
use serde_json::json;
use std::collections::BTreeMap;

fn device(s: &Session, tag: &str) -> Id<Device> {
    Id::from_uuid(
        s.resolve_reference(&format!("tag:{tag}"))
            .unwrap_or_else(|e| panic!("no device {tag}: {e:?}")),
    )
}

/// The slot texts printed by the representations of a device, on every page.
fn slot_texts(s: &Session, d: Id<Device>) -> Vec<String> {
    let q = s.query();
    let mut out = Vec::new();
    for entry in &q.get::<PageOrder>(()).entries {
        for rep in page_view(&q, entry).reps {
            let of_device = rep
                .rep
                .functions
                .iter()
                .any(|f| q.functions().get(*f).and_then(|f| f.device) == Some(d));
            if of_device {
                out.extend(rep.slots.iter().map(|s| s.text.clone()));
            }
        }
    }
    out
}

/// The four valves of the sample's valve terminal.
const VALVES: [&str; 4] = ["-201K1-Q1", "-201K1-Q2", "-201K1-Q3", "-201K1-Q4"];

fn valves(s: &Session) -> Vec<(Id<Device>, String)> {
    VALVES.iter().map(|t| (device(s, t), (*t).to_owned())).collect()
}

/// The sample with a process tag on each cylinder.
fn with_process_tags() -> Session {
    let mut s = common::sample();
    for (tag, process) in [("-201M1", "CL-1"), ("-201M2", "CL-2")] {
        let d = device(&s, tag);
        s.execute(&AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [d.to_string()], "path": "process_tag", "value": process}),
        })
        .unwrap_or_else(|e| panic!("{tag}: {e}"));
    }
    s
}

/// FL-2 (FR-FLU-001): each of the four valves has a valve function with the ports 1 to 5 and their roles, and a coil
/// function with the one non-physical CP 14, to which its channel is bound.
#[test]
fn fl2_every_valve_has_its_port_roles_and_an_internal_coil() {
    let s = common::sample();
    let q = s.query();
    let all = valves(&s);
    let bound = q.get::<ew_derive::ChannelsByCp>(());
    for (valve, tag) in all {
        let mut fns: Vec<_> = q.idx().functions_of_device(valve).collect();
        fns.sort();
        let kind = |f| {
            q.functions()
                .get(f)
                .and_then(|f| q.fndefs().get(f.def))
                .map(|d| d.item.contact_kind.clone())
        };
        let body: Vec<_> = fns
            .iter()
            .copied()
            .filter(|f| kind(*f) == Some(ContactKind::Valve))
            .collect();
        let coil: Vec<_> = fns
            .iter()
            .copied()
            .filter(|f| kind(*f) == Some(ContactKind::Coil))
            .collect();
        assert_eq!((body.len(), coil.len(), fns.len()), (1, 1, 2), "{tag}");
        let ports: Vec<(String, Option<FluidPortRole>)> = q
            .get::<FunctionCps>(body[0])
            .iter()
            .map(|c| (c.designation.clone(), c.port_role.clone()))
            .collect();
        assert_eq!(
            ports,
            [
                ("1".to_owned(), Some(FluidPortRole::Supply1)),
                ("2".to_owned(), Some(FluidPortRole::Working2)),
                ("3".to_owned(), Some(FluidPortRole::Exhaust3)),
                ("4".to_owned(), Some(FluidPortRole::Working4)),
                ("5".to_owned(), Some(FluidPortRole::Exhaust5)),
            ],
            "{tag}"
        );
        let cps = q.get::<FunctionCps>(coil[0]);
        let coil_cps: Vec<(u16, &str, bool)> = cps
            .iter()
            .map(|c| (c.idx, c.designation.as_str(), c.physical))
            .collect();
        assert_eq!(coil_cps, [(0, "14", false)], "{tag}");
        assert_eq!(
            ew_derive::solenoid_coil(&q, valve, &"14".parse().unwrap()),
            Some(ew_derive::CpRef::new(coil[0], 0)),
            "{tag}"
        );
        assert!(bound.is_bound(coil[0], 0), "{tag}: the coil's channel");
    }
}

/// FL-5 (FR-FLU-003, FR-FLU-005): -201K1-Q1 prints Q0.0, the address of the channel bound to its coil function; a
/// `plc.set_start` of the valve terminal re-renders it in the same commit, and undo restores it.
#[test]
fn fl5_the_valve_prints_the_address_of_its_coil_channel() {
    let mut s = common::sample();
    let valve = device(&s, "-201K1-Q1");
    let q = s.query();
    let coil = q
        .idx()
        .functions_of_device(valve)
        .find(|f| {
            q.functions()
                .get(*f)
                .and_then(|f| q.fndefs().get(f.def))
                .is_some_and(|d| d.item.contact_kind == ContactKind::Coil)
        })
        .expect("the coil function");
    let channel = q
        .table::<ew_schema::Channel>()
        .iter()
        .find(|(_, c)| c.binding == Some(Endpoint::Cp(coil, 0)))
        .map(|(_, c)| c.number);
    assert_eq!(channel, Some(0), "the coil is bound to channel 0 of -201K1");
    drop(q);
    assert!(slot_texts(&s, valve).iter().any(|t| t == "Q0.0"));
    let module = device(&s, "-201K1");
    let spec = s
        .parse_field_input("device", "plc_module.start.output", "Q10")
        .unwrap()
        .value;
    s.execute(&AnyCommand {
        name: "plc.set_start".into(),
        params: json!({"modules": [module.to_string()], "dir": "output", "spec": spec}),
    })
    .unwrap();
    let texts = slot_texts(&s, valve);
    assert!(texts.iter().any(|t| t == "Q10.0"), "{texts:?}");
    assert!(!texts.iter().any(|t| t == "Q0.0"), "{texts:?}");
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert!(slot_texts(&s, valve).iter().any(|t| t == "Q0.0"));
}

/// FL-6 (FR-FLU-004, FR-FLU-027): `valve_slots(-201K1)` gives four valves; `valve.add` on -201K1, whose positions are
/// all taken, is refused by EW-FL-013's prevent half with T12 and "every position is taken"; the release run reports
/// no EW-FL-002, EW-FL-004 or EW-FL-013 finding on the sample (its spare valves' ports are plugged).
#[test]
fn fl6_a_full_module_refuses_a_valve_and_the_sample_has_no_occupancy_defect() {
    let mut s = common::sample();
    let module = device(&s, "-201K1");
    let q = s.query();
    let slots = q.get::<ew_derive::ValveSlots>(module);
    assert_eq!(slots.count, 4);
    assert!(
        slots
            .slots
            .iter()
            .all(|v| matches!(v, ew_derive::ValveSlot::Valve { .. }))
    );
    assert!(slots.second.is_empty() && slots.outside.is_empty());
    let part = q
        .devices()
        .get(device(&s, "-201K1-Q1"))
        .and_then(|d| d.parts.first().map(|u| u.part.copy))
        .expect("the valve part");
    drop(q);
    let refusal = s
        .execute(&AnyCommand {
            name: "valve.add".into(),
            params: json!({"module": module.to_string(), "part": {"part": {"copy": part.to_string()}, "qty": "1"}}),
        })
        .expect_err("refused");
    assert_eq!(
        refusal.to_string(),
        "EW-FL-013: -201K1: position 5 is outside its 4 valve positions; every position is taken"
    );
    let report = s.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Release);
    let count = |rule: &str| report.findings.iter().filter(|f| f.rule == rule).count();
    assert_eq!(
        (count("EW-FL-002"), count("EW-FL-004"), count("EW-FL-013")),
        (0, 0, 0)
    );
}

/// The open EW-FL-001 findings of the release run.
fn open_ports(s: &Session) -> usize {
    s.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Release)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-FL-001")
        .count()
}

/// The valve body function (contact kind `valve`) of a device.
fn body(s: &Session, valve: Id<Device>) -> Id<ew_schema::Function> {
    let q = s.query();
    q.idx()
        .functions_of_device(valve)
        .find(|f| {
            q.functions()
                .get(*f)
                .and_then(|f| q.fndefs().get(f.def))
                .is_some_and(|d| d.item.contact_kind == ContactKind::Valve)
        })
        .expect("the valve body")
}

/// FL-9 (FR-FLU-028): the sample plugs its spare valves' working ports, so EW-FL-001 reports nothing; opening
/// -201K1-Q3's ports 2 and 4 makes it report that valve, plugging them again clears it; T9 and T10 whole.
#[test]
fn fl9_closed_ports_are_exempt_from_ew_fl_001() {
    let mut s = common::sample();
    assert_eq!(open_ports(&s), 0);
    let f = body(&s, device(&s, "-201K1-Q3"));
    let close = |idx: u16, closure: Option<serde_json::Value>| {
        let mut params = json!({"function": f.to_string(), "idx": idx});
        if let Some(c) = closure {
            params["closure"] = c;
        }
        AnyCommand {
            name: "fluid.port.close".into(),
            params,
        }
    };
    for idx in [1, 3] {
        s.execute(&close(idx, None)).unwrap();
    }
    assert_eq!(open_ports(&s), 1);
    for idx in [1, 3] {
        s.execute(&close(idx, Some(json!({"kind": "plug"})))).unwrap();
    }
    assert_eq!(open_ports(&s), 0);
    // T9: the coil's CP 14 is no fluid port; T10: -201K1-Q1:2 has a tube
    let coil = {
        let q = s.query();
        q.idx()
            .functions_of_device(device(&s, "-201K1-Q3"))
            .find(|g| *g != f)
            .expect("the coil")
    };
    let refusal = |s: &mut Session, cmd: &AnyCommand| s.execute(cmd).expect_err("refused").to_string();
    let t9 = AnyCommand {
        name: "fluid.port.close".into(),
        params: json!({"function": coil.to_string(), "idx": 0, "closure": {"kind": "plug"}}),
    };
    assert_eq!(
        refusal(&mut s, &t9),
        "fluid.port.close: -201K1-Q3:14 is not a fluid port"
    );
    let used = body(&s, device(&s, "-201K1-Q1"));
    let t10 = AnyCommand {
        name: "fluid.port.close".into(),
        params: json!({"function": used.to_string(), "idx": 1, "closure": {"kind": "plug"}}),
    };
    assert_eq!(
        refusal(&mut s, &t10),
        "fluid.port.close: -201K1-Q1:2 has a tube; delete the tube before closing the port"
    );
}

/// The sample's records without -201K1-Q1's `drives` link and its two working lines, or with the link pointing at
/// `retarget` instead.
fn without_q1s_lines(retarget: Option<Id<Device>>) -> Session {
    let sample = with_process_tags();
    let valve = device(&sample, "-201K1-Q1");
    let f = body(&sample, valve);
    let (project, mut records, lib) = common::sample_records();
    match retarget {
        None => records.retain(|r| match r {
            Record::Connection(c) => {
                !matches!(c.kind, ConnectionKind::Fluid(_))
                    || ![&c.a, &c.b]
                        .iter()
                        .any(|e| matches!(e, Endpoint::Cp(g, _) if *g == f))
            }
            Record::DeviceLink(l) => l.from != valve,
            _ => true,
        }),
        Some(to) => {
            for r in &mut records {
                if let Record::DeviceLink(l) = r
                    && l.from == valve
                {
                    l.to = to;
                }
            }
        }
    }
    let mut s = common::session_of(project, records, lib);
    for (tag, process) in [("-201M1", "CL-1"), ("-201M2", "CL-2")] {
        let d = device(&s, tag);
        s.execute(&AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "device", "targets": [d.to_string()], "path": "process_tag", "value": process}),
        })
        .unwrap_or_else(|e| panic!("{tag}: {e}"));
    }
    s
}

/// FL-10 (FR-FLU-029): on the sample without -201K1-Q1's `drives` link and its two working lines, drawing the first
/// line to the cylinder -201M1 adds the link (T22) and the valve prints the cylinder's process tag; drawing the second
/// adds no second link. A link to another actuator than the one the lines reach is EW-FL-011 (T18).
#[test]
fn fl10_the_first_working_line_links_the_valve() {
    let mut s = without_q1s_lines(None);
    let valve = device(&s, "-201K1-Q1");
    let actuator = device(&s, "-201M1");
    assert!(!slot_texts(&s, valve).iter().any(|t| t == "CL-1"));
    let f = body(&s, valve);
    let cylinder = {
        let q = s.query();
        q.idx()
            .functions_of_device(actuator)
            .find(|g| {
                q.functions()
                    .get(*g)
                    .and_then(|x| q.fndefs().get(x.def))
                    .is_some_and(|d| d.item.contact_kind != ContactKind::Valve && d.item.cps.len() == 2)
                    && q.get::<FunctionCps>(*g).iter().any(|c| c.designation == "A")
            })
            .expect("the actuator")
    };
    let draw = |port: u16, end: u16| AnyCommand {
        name: "connection.draw".into(),
        params: json!({
            "a": {"cp": {"function": f.to_string(), "idx": port}},
            "b": {"cp": {"function": cylinder.to_string(), "idx": end}},
            "kind": {"fluid": {}},
        }),
    };
    let links = |s: &Session| {
        s.query()
            .table::<ew_schema::DeviceLink>()
            .iter()
            .filter(|(_, l)| l.from == valve)
            .map(|(_, l)| l.to)
            .collect::<Vec<_>>()
    };
    let r = s.execute(&draw(1, 0)).unwrap();
    assert_eq!(r.outcome.notices, ["-201K1-Q1 now drives -201M1"]);
    assert_eq!(links(&s), [actuator]);
    assert!(slot_texts(&s, valve).iter().any(|t| t == "CL-1"));
    let r = s.execute(&draw(3, 1)).unwrap();
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    assert_eq!(links(&s), [actuator]);
    // T18: the link of -201K1-Q1 pointing at -201M2 while its lines reach -201M1
    let other = device(&s, "-201M2");
    let s = without_q1s_lines(Some(other));
    let online = s.check(&ew_api::checks::DRAFTING, ew_api::checks::CheckMode::Online);
    let t18: Vec<String> = online
        .findings
        .iter()
        .filter(|x| x.rule == "EW-FL-011")
        .map(|x| x.message())
        .collect();
    assert_eq!(
        t18,
        ["-201K1-Q1 drives -201M2, but its working-port lines reach -201M1"]
    );
}

/// D14 (FR-FLU-005): the function-text slot of the 5/2 valve falls back to the tracked process tag, so the two used
/// valves print their cylinder's tag; the spare valves keep their manual text "Spare".
#[test]
fn d14_the_used_valves_print_the_process_tag_of_their_field_device() {
    let s = with_process_tags();
    let expected = [
        ("-201K1-Q1", "CL-1"),
        ("-201K1-Q2", "CL-2"),
        ("-201K1-Q3", "Spare"),
        ("-201K1-Q4", "Spare"),
    ];
    for (tag, text) in expected {
        let texts = slot_texts(&s, device(&s, tag));
        assert!(texts.iter().any(|t| t == text), "{tag}: {texts:?}");
    }
}

/// Whether the axis-parallel segment `a`–`b` meets the closed box `lo`–`hi`.
fn meets(a: Pt2, b: Pt2, lo: Pt2, hi: Pt2) -> bool {
    a.x.min(b.x) <= hi.x && a.x.max(b.x) >= lo.x && a.y.min(b.y) <= hi.y && a.y.max(b.y) >= lo.y
}

/// The tubes on folio 201 carry stored routes. Each starts and ends where its CPs are drawn, runs orthogonally, is
/// taken again by `connection.route` (its ends and the connection grid, D3), keeps clear of every cylinder body, and
/// the two working lines of a valve never meet.
#[test]
fn the_working_lines_are_routed_clear_of_the_cylinder_bodies() {
    let mut s = common::sample();
    let q = s.query();
    let lines: Vec<(Id<ew_schema::Connection>, ew_schema::Connection)> = q
        .connections()
        .iter()
        .filter(|(_, c)| matches!(c.kind, ConnectionKind::Fluid(_)))
        .map(|(id, c)| (id, (**c).clone()))
        .collect();
    assert!(!lines.is_empty());
    let page = lines[0].1.page.expect("on folio 201");
    // the body of ew.iso.cylinder-double-acting@qet10 (the 12 mm module): the barrel from (5, 5) to (20, 45) px
    // from its position, its ports at x 0 on 2 mm leads
    let corner = Pt2::new(Grid::QET_1PX.at(5).unwrap(), Grid::QET_1PX.at(5).unwrap());
    let body_end = Pt2::new(Grid::QET_1PX.at(20).unwrap(), Grid::QET_1PX.at(45).unwrap());
    let bodies: Vec<(Pt2, Pt2)> = q
        .reps()
        .iter()
        .filter(|(_, r)| {
            r.page == page
                && q.symbols()
                    .get(r.symbol)
                    .and_then(|y| y.item.meta.key.as_ref())
                    .is_some_and(|k| k.as_str() == "ew.iso.cylinder-double-acting@qet10")
        })
        .map(|(_, r)| {
            // The barrel turned with the representation (the sample draws its cylinders upright, r270).
            let (a, b) = (
                r.pos.checked_add(r.orient.apply(corner)).unwrap(),
                r.pos.checked_add(r.orient.apply(body_end)).unwrap(),
            );
            (
                Pt2::new(a.x.min(b.x), a.y.min(b.y)),
                Pt2::new(a.x.max(b.x), a.y.max(b.y)),
            )
        })
        .collect();
    assert_eq!(bodies.len(), 2, "the two cylinders");
    let mut by_valve: BTreeMap<Id<ew_schema::Function>, Vec<Vec<Pt2>>> = BTreeMap::new();
    let valve_bodies: Vec<Id<ew_schema::Function>> = VALVES.iter().map(|t| body(&s, device(&s, t))).collect();
    for (id, c) in &lines {
        assert_eq!(c.page, Some(page), "{id}");
        let path = ew_derive::drawn_path(&q, c, page).expect("drawn on folio 201");
        let (k, route) = (path.len(), &c.route);
        assert!(route.len() >= 2, "{id}: {route:?}");
        assert_eq!((path[0], path[k - 1]), (route[0], route[route.len() - 1]), "{id}");
        for w in route.windows(2) {
            assert!(
                w[0].x == w[1].x || w[0].y == w[1].y,
                "{id}: {w:?} is not orthogonal"
            );
            for (lo, hi) in &bodies {
                assert!(!meets(w[0], w[1], *lo, *hi), "{id}: {w:?} meets a cylinder body");
            }
        }
        for e in [&c.a, &c.b] {
            if let Endpoint::Cp(f, _) = e
                && valve_bodies.contains(f)
            {
                by_valve.entry(*f).or_default().push(route.clone());
            }
        }
    }
    let pairs: Vec<&Vec<Vec<Pt2>>> = by_valve.values().filter(|r| r.len() == 2).collect();
    assert!(!pairs.is_empty(), "the used valves' two working lines");
    for pair in pairs {
        for a in pair[0].windows(2) {
            for b in pair[1].windows(2) {
                assert!(!meets(a[0], a[1], b[0], b[1]), "two lines of one valve meet");
            }
        }
    }
    drop(q);
    for (id, c) in &lines {
        let cmd = AnyCommand {
            name: "connection.route".into(),
            params: json!({"connection": id.to_string(), "route": c.route}),
        };
        s.execute(&cmd).unwrap_or_else(|e| panic!("{id}: {e}"));
    }
}
