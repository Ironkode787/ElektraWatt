// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rack and slot editor's view and `plc.add_module` (doc 10 §6, FR-PLC-028) on the sample: the CPU -104A1's two
//! cards in slot order and the valve terminal -201K1's four valve positions; a module added after the cards where its
//! part allows the slot, and refused by EW-PLC-011 where it does not; the overview of a station across pages.

use crate::common;

use ew_api::output::Uuid;
use ew_api::{AnyCommand, Session};
use ew_schema::LibDocument;
use serde_json::{Value, json};

/// The sample with the input card's part EW-GEN-DI-1 allowing the slots `allowed`.
fn sample_allowing(allowed: &[u16]) -> Session {
    let (project, records, lib) = common::sample_records();
    let lib = lib
        .iter()
        .map(|d| match d {
            LibDocument::Part(p) if p.item.part_number.as_str() == "EW-GEN-DI-1" => {
                let mut v = serde_json::to_value(d).expect("a document");
                v["sections"]["plc"]["allowed_slots"] = json!(allowed);
                LibDocument::from_value(v).expect("the edited part")
            }
            _ => d.clone(),
        })
        .collect();
    common::session_of(project, records, lib)
}

fn station(s: &Session) -> Uuid {
    s.resolve_reference("tag:-104A1").expect("the CPU")
}

fn add(s: &mut Session, part: &str) -> Result<(), String> {
    let station = station(s);
    let copy = s
        .query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == part)
        .map(|(id, _)| id.as_uuid())
        .expect("the module part");
    s.execute(&AnyCommand {
        name: "plc.add_module".into(),
        params: json!({"station": station.to_string(),
            "part": {"part": {"copy": copy.to_string()}, "qty": "1"}}),
    })
    .map(|_| ())
    .map_err(|e| format!("{e}"))
}

#[test]
fn the_cpu_in_the_rack_editor() {
    let mut s = sample_allowing(&[1, 2, 3]);
    let v = s.rack_view(station(&s)).expect("a station");
    assert_eq!(v.tag, "-104A1");
    assert!(!v.racked, "positional slots");
    let modules: Vec<(&str, u32)> = v
        .rows
        .iter()
        .filter(|r| r.module.is_some())
        .map(|r| (r.tag.as_str(), r.slot))
        .collect();
    assert_eq!(
        modules,
        [("-104A1-A1", 1), ("-104A1-A2", 2)],
        "the two cards in slot order"
    );
    let free: Vec<u32> = v
        .rows
        .iter()
        .filter(|r| r.module.is_none())
        .map(|r| r.slot)
        .collect();
    assert_eq!(free, [3], "the slot after the positional modules");
    assert_eq!(v.rows[0].addresses, "I0.0–I0.7");
    assert!(v.findings.is_empty());

    add(&mut s, "EW-GEN-DI-1").expect("slot 3 is allowed");
    let v = s.rack_view(station(&s)).unwrap();
    assert_eq!(v.rows.iter().filter(|r| r.module.is_some()).count(), 3);
    assert_eq!(
        v.rows
            .iter()
            .find(|r| r.slot == 3)
            .and_then(|r| r.module.map(|_| ())),
        Some(())
    );
}

/// The valve terminal is one module: its four valve positions hold the valves -201K1-Q1 … -201K1-Q4.
#[test]
fn the_valve_terminal_in_the_rack_editor_shows_its_valve_positions() {
    let s = common::sample();
    let v = s
        .rack_view(s.resolve_reference("tag:-201K1").expect("the valve terminal"))
        .expect("a station");
    let valves: Vec<(u32, Option<&str>)> = v
        .cells
        .iter()
        .map(|c| (c.position, c.at.valve.as_deref()))
        .collect();
    assert_eq!(
        valves,
        [
            (1, Some("-201K1-Q1")),
            (2, Some("-201K1-Q2")),
            (3, Some("-201K1-Q3")),
            (4, Some("-201K1-Q4")),
        ]
    );
    assert!(v.findings.is_empty());
}

#[test]
fn a_module_in_a_slot_its_part_does_not_allow_is_refused() {
    let mut s = sample_allowing(&[1, 2]);
    let rev = s.world().rev;
    let err = add(&mut s, "EW-GEN-DI-1").expect_err("slot 3 is not allowed");
    assert!(
        err.contains("EW-PLC-011") || err.contains("allows slots 1, 2"),
        "{err}"
    );
    assert_eq!(s.world().rev, rev, "nothing committed");
}

/// `plc.add_module {slot: 2}` on the CPU, whose second positional module is -104A1-A2: refused by EW-PLC-011, which
/// names both modules, and nothing is committed.
#[test]
fn a_module_into_a_slot_another_module_holds_is_refused() {
    let mut s = sample_allowing(&[]);
    let rev = s.world().rev;
    let station = station(&s);
    let copy = s
        .query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "EW-GEN-DI-1")
        .map(|(id, _)| id.as_uuid())
        .expect("the module part");
    let err = s
        .execute(&AnyCommand {
            name: "plc.add_module".into(),
            params: json!({"station": station.to_string(), "slot": 2,
                "part": {"part": {"copy": copy.to_string()}, "qty": "1"}}),
        })
        .expect_err("slot 2 holds -104A1-A2");
    let err = format!("{err}");
    assert!(
        err.contains("-104A1-A2") && err.contains("the same slot 2"),
        "{err}"
    );
    assert_eq!(s.world().rev, rev, "nothing committed");
}

#[test]
fn a_part_that_is_no_module_is_refused_in_words() {
    let mut s = sample_allowing(&[]);
    let station = station(&s);
    let cpu = s
        .library_parts()
        .into_iter()
        .find(|p| p.item.part_number.as_str() == "EW-GEN-CPU-1")
        .map(|p| p.id.as_uuid());
    let Some(cpu) = cpu else {
        return; // no starter library in this build's tiers
    };
    let copies: Vec<Value> = s.module_part_copies(cpu);
    let err = s
        .execute(&AnyCommand {
            name: "plc.add_module".into(),
            params: json!({"station": station.to_string(),
                "part": {"part": {"copy": cpu.to_string()}, "qty": "1"}, "copies": copies}),
        })
        .expect_err("a CPU is no module");
    assert!(format!("{err}").contains("is no PLC module"), "{err}");
}

/// `valve_terminal.generate_overview` (FR-FLU-033, doc 11 §5.5) of the CPU station in a band two overview modules
/// wide: its three members (the head and the two cards) fill new pages two at a time, never more; a second run
/// changes nothing. The break marks at each open end belong to a valve terminal's manifold, and the sample's valve
/// terminal is one module: they are not checked here.
#[test]
fn the_overview_breaks_onto_a_new_page_where_the_band_is_full() {
    use ew_schema::PageType;
    let mut s = common::sample();
    let pages = |s: &Session| {
        let q = s.query();
        q.pages()
            .iter()
            .filter(|(_, p)| p.kind == PageType::AssemblyOverview)
            .count()
    };
    let before = pages(&s);
    let mut params = s.overview_params(station(&s)).expect("the parameters");
    let left = params["left"].as_i64().expect("left");
    // The wide overview module is 42.333 mm wide; two fit, a third does not.
    params["right"] = json!(left + 2 * 42_333_333 + 7_937_500);
    // A bulk command: previewed, then committed.
    let run = |s: &mut Session| {
        let cmd = AnyCommand {
            name: "valve_terminal.generate_overview".into(),
            params: params.clone(),
        };
        let preview = s.preview(&cmd).expect("the preview");
        s.commit(preview).expect("the overview")
    };
    let receipt = run(&mut s);
    let notices: Vec<String> = receipt.outcome.notices.iter().map(ToString::to_string).collect();
    assert!(notices.iter().any(|n| n.contains("(new page)")), "{notices:?}");
    for n in &notices {
        let members = n.split(": ").nth(1).map_or(0, |m| m.split(", ").count());
        assert!(members <= 2, "{n}");
    }
    assert_eq!(pages(&s), before + 2, "three members two at a time");
    let receipt = run(&mut s);
    assert!(
        !receipt
            .outcome
            .notices
            .iter()
            .any(|n| n.to_string().contains("(new page)"))
    );
    assert_eq!(pages(&s), before + 2, "a refresh adds no page");
}
