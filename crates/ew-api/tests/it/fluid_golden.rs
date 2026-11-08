// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.20 on Golden A (GA-1, step 5): the valves' port roles and internal coils (FL-2), the derived valve addresses
//! from the coil functions (FL-5) and the function-text slot's process-tag fallback on folio 203 (D14).

use crate::golden_common;

use ew_api::output::page_view;
use ew_api::{AnyCommand, Session};
use ew_derive::{FunctionCps, PageOrder};
use ew_ids::Id;
use ew_schema::{Connection, ContactKind, Device, Endpoint, FluidPortRole, Record};
use ew_units::{Grid, Pt2};
use serde_json::json;
use std::collections::BTreeMap;

fn device(s: &Session, tag: &str) -> Id<Device> {
    golden_common::tags(s)
        .into_iter()
        .find(|(_, t)| t == tag)
        .unwrap_or_else(|| panic!("no device {tag}"))
        .0
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

/// The valves of Golden A: the children `-20xUn-Qp`.
fn valves(s: &Session) -> Vec<(Id<Device>, String)> {
    let re = |t: &str| {
        let t = t.strip_prefix("-20").unwrap_or_default();
        t.len() == 6 && t.as_bytes()[1] == b'U' && t[3..].starts_with("-Q")
    };
    golden_common::tags(s)
        .into_iter()
        .filter(|(_, t)| re(t))
        .collect()
}

/// FL-2 (FR-FLU-001): each of the 28 valves has a valve function with the ports 1 to 5 and their roles, and a coil
/// function with the one non-physical CP 14, to which its channel is bound.
#[test]
fn fl2_every_valve_has_its_port_roles_and_an_internal_coil() {
    let s = golden_common::session(&golden_common::batch());
    let q = s.query();
    let all = valves(&s);
    assert_eq!(all.len(), 28);
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

/// FL-5 (FR-FLU-003, FR-FLU-005): -201U3-Q1 prints Q0.1, the address of the channel bound to its coil function; a
/// `plc.set_start` of -201U3 re-renders it in the same commit, and undo restores it.
#[test]
fn fl5_the_valve_prints_the_address_of_its_coil_channel() {
    let mut s = golden_common::session(&golden_common::batch());
    let valve = device(&s, "-201U3-Q1");
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
    assert_eq!(channel, Some(1), "the coil is bound to channel 1 of -201U3");
    drop(q);
    assert!(slot_texts(&s, valve).iter().any(|t| t == "Q0.1"));
    let module = device(&s, "-201U3");
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
    assert!(texts.iter().any(|t| t == "Q10.1"), "{texts:?}");
    assert!(!texts.iter().any(|t| t == "Q0.1"), "{texts:?}");
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert!(slot_texts(&s, valve).iter().any(|t| t == "Q0.1"));
}

/// FL-6 (FR-FLU-004, FR-FLU-027): `valve_slots(-202U2)` gives four valves; `valve.add` on -202U2, whose positions are
/// all taken, is refused by EW-FL-013's prevent half with T12 and "every position is taken"; the release run reports
/// no EW-FL-002 or EW-FL-013 finding on Golden A and EW-FL-004 its 20 spare valves (EF-14).
#[test]
fn fl6_a_full_module_refuses_a_valve_and_golden_a_has_no_occupancy_defect() {
    let mut s = golden_common::session(&golden_common::batch());
    let module = device(&s, "-202U2");
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
        .get(device(&s, "-202U2-Q1"))
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
        "EW-FL-013: -202U2: position 5 is outside its 4 valve positions; every position is taken"
    );
    let report = s.check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Release);
    let count = |rule: &str| report.findings.iter().filter(|f| f.rule == rule).count();
    assert_eq!(
        (count("EW-FL-002"), count("EW-FL-004"), count("EW-FL-013")),
        (0, 20, 0)
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

/// FL-9 (FR-FLU-028): with F1 to F16 drawn, EW-FL-001 reports the 20 spare valves; plugging -201U5-Q1:2 and :4
/// leaves 19, clearing the closures gives 20 again; T9 and T10 whole.
#[test]
fn fl9_closed_ports_are_exempt_from_ew_fl_001() {
    let mut s = golden_common::session(&golden_common::batch());
    assert_eq!(open_ports(&s), 20);
    let f = body(&s, device(&s, "-201U5-Q1"));
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
        s.execute(&close(idx, Some(json!({"kind": "plug"})))).unwrap();
    }
    assert_eq!(open_ports(&s), 19);
    for idx in [1, 3] {
        s.execute(&close(idx, None)).unwrap();
    }
    assert_eq!(open_ports(&s), 20);
    // T9: the coil's CP 14 is no fluid port; T10: -201U3-Q1:2 has F1
    let coil = {
        let q = s.query();
        q.idx()
            .functions_of_device(device(&s, "-201U5-Q1"))
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
        "fluid.port.close: -201U5-Q1:14 is not a fluid port"
    );
    let used = body(&s, device(&s, "-201U3-Q1"));
    let t10 = AnyCommand {
        name: "fluid.port.close".into(),
        params: json!({"function": used.to_string(), "idx": 1, "closure": {"kind": "plug"}}),
    };
    assert_eq!(
        refusal(&mut s, &t10),
        "fluid.port.close: -201U3-Q1:2 has a fluid line; delete the line before closing the port"
    );
}

/// FL-10 (FR-FLU-029): on Golden A without -201U3-Q1's `drives` link and F1, F2, drawing F1 adds the link (T22) and
/// folio 203 prints "TA05-V-15" at -201U3-Q1; drawing F2 adds no second link. A link to another actuator than the one
/// the lines reach is EW-FL-011 (T18).
#[test]
fn fl10_the_first_working_line_links_the_valve() {
    let batch = golden_common::batch();
    let ids = golden_common::ids_by_key(&batch);
    let valve_id = ids["devices|-201U3-Q1"].clone();
    let mut s = golden_common::session_with(&batch, |_, records| {
        records.retain(|r| match r {
            ew_schema::Record::Connection(_) => !matches!(
                golden_common::source_key(r),
                Some("connections|F1" | "connections|F2")
            ),
            ew_schema::Record::DeviceLink(l) => l.from.to_string() != valve_id,
            _ => true,
        });
    });
    let valve = device(&s, "-201U3-Q1");
    let actuator = device(&s, "-203Q1");
    assert!(!slot_texts(&s, valve).iter().any(|t| t == "TA05-V-15"));
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
    assert_eq!(r.outcome.notices, ["-201U3-Q1 now drives -203Q1"]);
    assert_eq!(links(&s), [actuator]);
    assert!(slot_texts(&s, valve).iter().any(|t| t == "TA05-V-15"));
    let r = s.execute(&draw(3, 1)).unwrap();
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    assert_eq!(links(&s), [actuator]);
    // T18: Golden A with the link of -201U3-Q1 pointing at -203Q2 while F1 and F2 reach -203Q1
    let q2 = ids["devices|-203Q2"].clone();
    let valve_id = ids["devices|-201U3-Q1"].clone();
    let s = golden_common::session_with(&batch, |_, records| {
        for r in records.iter_mut() {
            if let ew_schema::Record::DeviceLink(l) = r
                && l.from.to_string() == valve_id
            {
                l.to = q2.parse().expect("an ID");
            }
        }
    });
    let online = s.check(&ew_api::checks::DRAFTING, ew_api::checks::CheckMode::Online);
    let t18: Vec<&str> = online
        .findings
        .iter()
        .filter(|x| x.rule == "EW-FL-011")
        .map(|x| x.message.as_str())
        .collect();
    assert_eq!(
        t18,
        ["-201U3-Q1 drives -203Q2, but its working-port lines reach -203Q1"]
    );
}

/// D14 (FR-FLU-005): the function-text slot of the 5/2 valve falls back to the tracked process tag, so the eight used
/// valves of folio 203 print their field device's tag; the spare valves keep their manual text "Spare".
#[test]
fn d14_the_used_valves_print_the_process_tag_of_their_field_device() {
    let s = golden_common::session(&golden_common::batch());
    let expected = [
        ("-201U3-Q1", "TA05-V-15"),
        ("-201U3-Q2", "TA05-V-33"),
        ("-201U3-Q3", "TA05-V-18"),
        ("-201U3-Q4", "TA05-V-17"),
        ("-201U4-Q1", "TA05-V-36"),
        ("-201U4-Q2", "TA05-V-38"),
        ("-201U4-Q3", "TA05-V-16"),
        ("-201U4-Q4", "TA05-V-37"),
        ("-201U5-Q1", "Spare"),
    ];
    for (tag, text) in expected {
        let texts = slot_texts(&s, device(&s, tag));
        assert!(texts.iter().any(|t| t == text), "{tag}: {texts:?}");
    }
}

/// A point in QET px (the nearest px).
fn px(p: Pt2) -> (i64, i64) {
    (Grid::QET_1PX.snap(p.x), Grid::QET_1PX.snap(p.y))
}

/// Whether the axis-parallel segment `a`–`b` meets the closed box `lo`–`hi`.
fn meets(a: Pt2, b: Pt2, lo: Pt2, hi: Pt2) -> bool {
    a.x.min(b.x) <= hi.x && a.x.max(b.x) >= lo.x && a.y.min(b.y) <= hi.y && a.y.max(b.y) >= lo.y
}

/// FT-1315 (doc 20 ▲U-51): F1 to F16 carry stored routes on folio 203. Each starts and ends where its CPs are drawn,
/// runs orthogonally, is taken again by `connection.route` (its ends and the connection grid, D3), keeps clear of
/// every cylinder body, and the two lines of a valve never meet; F1 runs up from port 2 of its valve in row H, past
/// the process valve, left over the cylinder and down into A (the router's route, decision log ID-1503).
#[test]
fn ft1315_the_working_lines_are_routed_clear_of_the_cylinder_bodies() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let lines: Vec<(&str, &Connection)> = batch
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Connection(c) => golden_common::source_key(r)
                .and_then(|k| k.strip_prefix("connections|F"))
                .map(|k| (k, c)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 16);
    let q = s.query();
    let page = lines[0].1.page.expect("on folio 203");
    // the body of ew.iso.cylinder-double-acting@qet10: the rectangle (0, 0) to (20, 60) px from its position
    let body = Pt2::new(Grid::QET_1PX.at(20).unwrap(), Grid::QET_1PX.at(60).unwrap());
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
        .map(|(_, r)| (r.pos, r.pos.checked_add(body).unwrap()))
        .collect();
    assert_eq!(bodies.len(), 8);
    let mut routes = BTreeMap::new();
    for (n, c) in &lines {
        assert_eq!(c.page, Some(page), "F{n}");
        let path = ew_derive::drawn_path(&q, c, page).expect("drawn on folio 203");
        let (k, route) = (path.len(), &c.route);
        assert!(route.len() >= 2, "F{n}: {route:?}");
        assert_eq!((path[0], path[k - 1]), (route[0], route[route.len() - 1]), "F{n}");
        for w in route.windows(2) {
            assert!(
                w[0].x == w[1].x || w[0].y == w[1].y,
                "F{n}: {w:?} is not orthogonal"
            );
            for (lo, hi) in &bodies {
                assert!(!meets(w[0], w[1], *lo, *hi), "F{n}: {w:?} meets a cylinder body");
            }
        }
        routes.insert(n.parse::<u32>().unwrap(), (c.id, route.clone()));
    }
    for k in (1..=16).step_by(2) {
        for a in routes[&k].1.windows(2) {
            for b in routes[&(k + 1)].1.windows(2) {
                assert!(!meets(a[0], a[1], b[0], b[1]), "F{k} and F{} meet", k + 1);
            }
        }
    }
    let f1: Vec<(i64, i64)> = routes[&1].1.iter().copied().map(px).collect();
    assert_eq!(f1, [(140, 590), (140, 420), (80, 420), (80, 440), (90, 440)]);
    drop(q);
    for (n, (id, route)) in &routes {
        let cmd = AnyCommand {
            name: "connection.route".into(),
            params: json!({"connection": id.to_string(), "route": route}),
        };
        s.execute(&cmd).unwrap_or_else(|e| panic!("F{n}: {e}"));
    }
}

/// FT-1323: on folio 203 the eight used valves move their function-text slot (slot 1, the process tag) to (-30, 45)
/// px, below the valve body, which ends at 40 px; no other valve representation moves it, so the spare valves on 204
/// to 206 keep "Spare" at the symbol's (-30, 35) px.
#[test]
fn ft1323_the_process_tags_on_folio_203_sit_below_the_valve_body() {
    let s = golden_common::session(&golden_common::batch());
    let q = s.query();
    let mut moved = Vec::new();
    for (valve, tag) in valves(&s) {
        for f in q.idx().functions_of_device(valve) {
            for r in q.idx().reps_of_function(f) {
                let rep = q.reps().get(r).expect("the representation");
                let slots: Vec<(u16, Option<(i64, i64)>)> =
                    rep.text_slots.iter().map(|o| (o.slot, o.pos.map(px))).collect();
                if !slots.is_empty() {
                    assert_eq!(slots, [(1, Some((-30, 45)))], "{tag}");
                    moved.push(tag.clone());
                }
            }
        }
    }
    moved.sort();
    assert_eq!(
        moved,
        [
            "-201U3-Q1",
            "-201U3-Q2",
            "-201U3-Q3",
            "-201U3-Q4",
            "-201U4-Q1",
            "-201U4-Q2",
            "-201U4-Q3",
            "-201U4-Q4"
        ]
    );
}
