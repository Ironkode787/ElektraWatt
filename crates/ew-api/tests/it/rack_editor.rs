// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rack and slot editor's view and `plc.add_module` (doc 10 §6, FR-PLC-028) on Golden A: the valve terminal's
//! seven modules in manifold order with their valve positions beside the stored order codes; a module added after
//! them where its part allows the slot, and refused by EW-PLC-011 where it does not.

use crate::common;

use ew_api::output::Uuid;
use ew_api::{AnyCommand, Session};
use ew_schema::LibDocument;
use serde_json::{Value, json};

/// Golden A with part 533361 allowing the slots `allowed`.
fn golden_allowing(allowed: &[u16]) -> Session {
    let batch = common::golden_batch();
    let (project, records) = common::split(&batch);
    let lib = batch
        .lib
        .iter()
        .map(|d| match d {
            LibDocument::Part(p) if p.item.part_number.as_str() == "533361" => {
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
    s.resolve_reference("tag:-201U1").expect("the valve terminal")
}

fn add(s: &mut Session, part: &str) -> Result<(), String> {
    let station = station(s);
    let copy = s
        .module_parts()
        .into_iter()
        .find(|p| p.text.starts_with(part))
        .expect("the module part")
        .copy;
    s.execute(&AnyCommand {
        name: "plc.add_module".into(),
        params: json!({"station": station.to_string(),
            "part": {"part": {"copy": copy.to_string()}, "qty": "1"}}),
    })
    .map(|_| ())
    .map_err(|e| format!("{e}"))
}

#[test]
fn the_valve_terminal_in_the_rack_editor() {
    let mut s = golden_allowing(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let v = s.rack_view(station(&s)).expect("a station");
    assert_eq!(v.tag, "-201U1");
    assert!(!v.racked, "positional slots");
    let modules: Vec<(&str, u32)> = v
        .rows
        .iter()
        .filter(|r| r.module.is_some())
        .map(|r| (r.tag.as_str(), r.slot))
        .collect();
    assert_eq!(
        modules,
        [
            ("-201U3", 1),
            ("-201U4", 2),
            ("-201U5", 3),
            ("-201U6", 4),
            ("-201U7", 5),
            ("-201U8", 6),
            ("-202U2", 7)
        ],
        "the seven modules in manifold order"
    );
    let free: Vec<u32> = v
        .rows
        .iter()
        .filter(|r| r.module.is_none())
        .map(|r| r.slot)
        .collect();
    assert_eq!(free, [8], "the slot after the positional modules");
    let first = &v.rows[0];
    assert_eq!(first.positions.len(), 4, "a sub-base with four valve positions");
    assert!(first.positions.iter().all(|(valve, _)| valve.is_some()));
    let codes: Vec<&str> = v.order_codes.iter().map(|(_, c)| c.as_str()).collect();
    assert!(codes.contains(&"51E-F13GCQP-D+H2GG"), "{codes:?}");
    assert!(v.findings.is_empty());

    add(&mut s, "533361").expect("slot 8 is allowed");
    let v = s.rack_view(station(&s)).unwrap();
    assert_eq!(v.rows.iter().filter(|r| r.module.is_some()).count(), 8);
    assert_eq!(
        v.rows
            .iter()
            .find(|r| r.slot == 8)
            .and_then(|r| r.module.map(|_| ())),
        Some(())
    );
}

#[test]
fn a_module_in_a_slot_its_part_does_not_allow_is_refused() {
    let mut s = golden_allowing(&[1, 2, 3, 4, 5, 6, 7]);
    let rev = s.world().rev;
    let err = add(&mut s, "533361").expect_err("slot 8 is not allowed");
    assert!(
        err.contains("EW-PLC-011") || err.contains("allows slots 1, 2, 3, 4, 5, 6, 7"),
        "{err}"
    );
    assert_eq!(s.world().rev, rev, "nothing committed");
}

/// `plc.add_module {slot: 3}` on the valve terminal, whose third positional module is -201U5: refused by EW-PLC-011,
/// which names both modules, and nothing is committed.
#[test]
fn a_module_into_a_slot_another_module_holds_is_refused() {
    let mut s = golden_allowing(&[]);
    let rev = s.world().rev;
    let station = station(&s);
    let copy = s
        .module_parts()
        .into_iter()
        .find(|p| p.text.starts_with("533361"))
        .expect("the module part")
        .copy;
    let err = s
        .execute(&AnyCommand {
            name: "plc.add_module".into(),
            params: json!({"station": station.to_string(), "slot": 3,
                "part": {"part": {"copy": copy.to_string()}, "qty": "1"}}),
        })
        .expect_err("slot 3 holds -201U5");
    let err = format!("{err}");
    assert!(err.contains("-201U5") && err.contains("the same slot 3"), "{err}");
    assert_eq!(s.world().rev, rev, "nothing committed");
}

#[test]
fn a_part_that_is_no_module_is_refused_in_words() {
    let mut s = golden_allowing(&[]);
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

/// `valve_terminal.generate_overview` (FR-FLU-033, doc 11 §5.5) in a band five overview modules wide: the members fill
/// 201 and 202 five at a time, never more, and the rest goes onto a new assembly-overview page, with a break mark at
/// each open end; a second run changes nothing.
#[test]
fn the_overview_breaks_onto_a_new_page_where_the_band_is_full() {
    use ew_derive::ew_schema::PageType;
    let mut s = golden_allowing(&[1, 2, 3, 4, 5, 6, 7, 8]);
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
    // Golden A's overview module is 15.875 mm wide; five fit, a sixth does not.
    params["right"] = json!(left + 5 * 15_875_000 + 7_937_500);
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
    let notices = receipt.outcome.notices;
    assert!(notices.iter().any(|n| n.contains("(new page)")), "{notices:?}");
    for n in &notices {
        let members = n.split(": ").nth(1).map_or(0, |m| m.split(", ").count());
        assert!(members <= 5, "{n}");
    }
    assert_eq!(pages(&s), before + 1, "one page more");
    let q = s.query();
    let marks = q.get::<ew_derive::AssemblyBreakMarks>(());
    assert!(marks.len() >= 4, "two marks at each of two breaks: {marks:?}");
    let receipt = run(&mut s);
    assert!(!receipt.outcome.notices.iter().any(|n| n.contains("(new page)")));
    assert_eq!(pages(&s), before + 1, "a refresh adds no page");
}
