// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.20 on Golden A (GA-1, step 5): the valves' port roles and internal coils (FL-2), the derived valve addresses
//! from the coil functions (FL-5) and the function-text slot's process-tag fallback on folio 203 (D14).

use crate::golden_common;

use ew_api::output::page_view;
use ew_api::{AnyCommand, Session};
use ew_derive::{FunctionCps, PageOrder};
use ew_ids::Id;
use ew_schema::{ContactKind, Device, Endpoint, FluidPortRole};
use serde_json::json;

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
