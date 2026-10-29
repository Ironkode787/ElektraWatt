// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `terminal.insert` and `terminal.move` (WP-S2.07 §5.1, §5.2, D4; acceptance SE1, SE2) and `strip.accessories`
//! through `fields.set` (D5; SE3), on the mini golden's -X1031 grown to Golden A's shape: twenty terminals
//! designated 1 … 20 at seq 1 … 20, the groups 1–4 "Pneumatics", 5–10 "Spare", 11–14 "Pneumatics" and 15–20
//! "Spare", and the bridge BR1 from :1 to :3. The same examples on Golden A itself run in `ew-testkit`
//! (`tests/terminal_insert_move.rs`).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::BTreeMap;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddTerminals, CreateDevice, InsertTerminals, KindName, MoveTerminals, SetFields, TerminalSpec,
};
use ew_commands::{CmdError, Engine, Origin};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::ModelError;
use ew_schema::{Device, Kind, PartRef, PartUse, StripAccessory, TerminalGroup};
use serde_json::json;

/// The groups of Golden A's -X1031 as `(from_seq, to_seq, label)`.
const GROUPS: [(u32, u32, &str); 4] = [
    (1, 4, "Pneumatics"),
    (5, 10, "Spare"),
    (11, 14, "Pneumatics"),
    (15, 20, "Spare"),
];

fn spec(designation: &str) -> TerminalSpec {
    TerminalSpec {
        designation: ident(designation),
        seq: None,
        function_text: MlText::default(),
        levels: vec![],
        rail_bonded: false,
    }
}

fn set_strip(e: &mut Engine, path: &str, value: serde_json::Value) -> Result<(), CmdError> {
    e.execute(&SetFields {
        kind: KindName::Device,
        targets: vec![DEV_X1031.cast()],
        path: path.into(),
        value,
    })
    .map(|_| ())
}

/// The mini golden with -X1031 in Golden A's shape (see the module comment); the history is cleared.
fn golden_a_strip() -> Engine {
    let mut model_engine = engine(golden_model());
    let specs = (4..=20)
        .map(|n| TerminalSpec {
            seq: Some(n),
            ..spec(&n.to_string())
        })
        .collect();
    model_engine
        .execute(&AddTerminals {
            strip: DEV_X1031,
            fndef: FNDEF_TERMINAL,
            terminals: specs,
        })
        .expect("terminals 4 to 20 are added");
    let groups: Vec<_> = GROUPS
        .iter()
        .map(|(from, to, label)| json!({"from_seq": from, "to_seq": to, "label": label}))
        .collect();
    set_strip(&mut model_engine, "strip.groups", json!(groups)).expect("the groups are set");
    // A fresh history over the grown model; another seed, so new IDs do not repeat the set-up's.
    engine_seeded(model_engine.model().clone(), SEED + 1)
}

/// The terminal of -X1031 designated `designation`.
fn terminal(e: &Engine, designation: &str) -> Id<Device> {
    e.model()
        .table::<Device>()
        .iter()
        .find(|(_, d)| {
            d.aspects
                .terminal
                .as_ref()
                .is_some_and(|t| t.strip == DEV_X1031 && t.designation.as_str() == designation)
        })
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("terminal {designation}"))
}

/// Designation → seq of every terminal of -X1031.
fn seqs(e: &Engine) -> BTreeMap<String, u32> {
    e.model()
        .table::<Device>()
        .iter()
        .filter_map(|(_, d)| d.aspects.terminal.as_ref())
        .filter(|t| t.strip == DEV_X1031)
        .map(|t| (t.designation.as_str().to_owned(), t.seq))
        .collect()
}

fn strip_aspect(e: &Engine) -> ew_schema::Strip {
    e.model()
        .table::<Device>()
        .get(&DEV_X1031)
        .and_then(|d| d.aspects.strip.clone())
        .expect("the strip aspect")
}

fn groups(e: &Engine) -> Vec<(u32, u32, MlText)> {
    strip_aspect(e)
        .groups
        .iter()
        .map(|g: &TerminalGroup| (g.from_seq, g.to_seq, g.label.clone()))
        .collect()
}

fn group(from: u32, to: u32, label: &str) -> (u32, u32, MlText) {
    (from, to, MlText::neutral(label))
}

/// Every connection record as its JSON line.
fn connections(e: &Engine) -> Vec<String> {
    lines(e.model())
        .into_iter()
        .filter(|(k, _)| k.0 == Kind::Connection)
        .map(|(_, l)| l)
        .collect()
}

fn invalid<T: std::fmt::Debug>(r: Result<T, CmdError>) -> (String, String) {
    match r {
        Err(CmdError::InvalidParams { path, message, .. }) => (path, message),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

fn not_found<T: std::fmt::Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

fn insert(before: Option<Id<Device>>, designations: &[&str]) -> InsertTerminals {
    InsertTerminals {
        strip: DEV_X1031,
        before,
        fndef: FNDEF_TERMINAL,
        terminals: designations.iter().map(|d| spec(d)).collect(),
        parts: vec![],
    }
}

/// A new, empty strip -X9.
fn new_strip(e: &mut Engine) -> Id<Device> {
    let r = e
        .execute(&CreateDevice {
            letter: ident("X"),
            counter: Some(9),
            kind: None,
            parts: vec![],
            function_text: MlText::default(),
            structure: Default::default(),
            strip: Some(serde_json::from_value(json!({})).expect("an empty strip aspect")),
            main_fndef: None,
        })
        .expect("the strip is created");
    r.outcome.primary.expect("the strip").1.cast()
}

fn accessory(after_seq: u32) -> serde_json::Value {
    json!({"after_seq": after_seq, "kind": "end_clamp", "part": {"copy": PART_MCB}})
}

// ---------------------------------------------------------------------------------------------------------
// SE1: terminal.insert

/// §5.1's example: before :5 with one terminal designated 21.
#[test]
fn insert_before_5_shifts_the_following_terminals_and_groups() {
    let mut e = golden_a_strip();
    let before = lines(e.model());
    let bridges = connections(&e);
    let t5 = terminal(&e, "5");
    let r = e.execute(&insert(Some(t5), &["21"])).unwrap();
    let s = seqs(&e);
    for n in 1..=4u32 {
        assert_eq!(s[&n.to_string()], n, "terminal {n} keeps its seq");
    }
    for n in 5..=20u32 {
        assert_eq!(s[&n.to_string()], n + 1, "terminal {n} moves up by one");
    }
    assert_eq!(s["21"], 5, "the new terminal takes seq 5");
    assert_eq!(
        groups(&e),
        [
            group(1, 4, "Pneumatics"),
            group(6, 11, "Spare"),
            group(12, 15, "Pneumatics"),
            group(16, 21, "Spare"),
        ],
        "seq 5 is in no group"
    );
    // No designation changes: the twenty designations are where they were, and the new one is 21.
    let designations: Vec<&String> = s.keys().collect();
    assert_eq!(designations.len(), 21);
    assert_eq!(connections(&e), bridges, "no bridge or wire changes");
    // The outcome names the new terminal; it is a terminal device with a main function of the FnDef.
    let new = terminal(&e, "21");
    assert_eq!(r.outcome.primary, Some((Kind::Device, new.cast())));
    let device = e.model().table::<Device>().get(&new).cloned().expect("new");
    assert_eq!(device.tag.counter, Some(5), "the tag counter is the seq");
    assert!(device.parts.is_empty());
    // One undo step restores the project.
    assert_eq!(e.history().entries().len(), 1);
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

/// An accessory after 4 stays before the inserted terminal; one after 5 moves with terminal 5.
#[test]
fn insert_shifts_accessories_at_or_after_the_position() {
    let mut e = golden_a_strip();
    set_strip(
        &mut e,
        "strip.accessories",
        json!([accessory(4), accessory(5), accessory(20)]),
    )
    .unwrap();
    let t5 = terminal(&e, "5");
    e.execute(&insert(Some(t5), &["21"])).unwrap();
    let after: Vec<u32> = strip_aspect(&e)
        .accessories
        .iter()
        .map(|a: &StripAccessory| a.after_seq)
        .collect();
    assert_eq!(after, [4, 6, 21]);
}

/// Without `before` the terminals go after the last; several are numbered in list order with the parts given.
#[test]
fn insert_at_the_end_numbers_in_list_order_with_parts() {
    let mut e = golden_a_strip();
    let groups_before = groups(&e);
    let mut cmd = insert(None, &["A", "B"]);
    cmd.parts = vec![PartUse {
        part: PartRef::new(PART_MCB),
        qty: dec3("1"),
        extra: ew_schema::Extra::new(),
    }];
    let r = e.execute(&cmd).unwrap();
    let s = seqs(&e);
    assert_eq!((s["A"], s["B"]), (21, 22));
    assert_eq!(s["20"], 20);
    assert_eq!(groups(&e), groups_before);
    assert_eq!(r.outcome.primary, Some((Kind::Device, terminal(&e, "A").cast())));
    for d in ["A", "B"] {
        let device = e
            .model()
            .table::<Device>()
            .get(&terminal(&e, d))
            .cloned()
            .unwrap();
        assert_eq!(device.parts, cmd.parts, "{d}");
    }
}

/// An empty strip starts at seq 1.
#[test]
fn insert_into_an_empty_strip_starts_at_1() {
    let mut e = engine(golden_model());
    let strip = new_strip(&mut e);
    let r = e
        .execute(&InsertTerminals {
            strip,
            ..insert(None, &["1"])
        })
        .unwrap();
    let new: Id<Device> = r.outcome.primary.expect("primary").1.cast();
    let device = e.model().table::<Device>().get(&new).cloned().unwrap();
    assert_eq!(device.aspects.terminal.as_ref().map(|t| t.seq), Some(1));
}

#[test]
fn insert_rejections_name_their_pointer() {
    let mut e = golden_a_strip();
    let t5 = terminal(&e, "5");
    // Not a strip.
    let cmd = InsertTerminals {
        strip: DEV_101F1,
        ..insert(None, &["21"])
    };
    assert_eq!(invalid(e.execute(&cmd)).0, "/strip");
    // `before` of another strip, and `before` that is not a terminal.
    let other = InsertTerminals {
        strip: DEV_X1031,
        before: Some(DEV_101F1),
        ..insert(None, &["21"])
    };
    assert_eq!(invalid(e.execute(&other)).0, "/before");
    let strip_itself = insert(Some(DEV_X1031), &["21"]);
    assert_eq!(invalid(e.execute(&strip_itself)).0, "/before");
    // An empty list.
    assert_eq!(invalid(e.execute(&insert(Some(t5), &[]))).0, "/terminals");
    // A spec with `seq`.
    let mut with_seq = insert(Some(t5), &["21", "22"]);
    with_seq.terminals[1].seq = Some(7);
    let (path, message) = invalid(e.execute(&with_seq));
    assert_eq!(path, "/terminals/1/seq");
    assert_eq!(message, "the position follows from `before`; leave out `seq`");
    // A designation the strip has, and one given twice.
    assert_eq!(
        invalid(e.execute(&insert(Some(t5), &["21", "7"]))).0,
        "/terminals/1/designation"
    );
    assert_eq!(
        invalid(e.execute(&insert(Some(t5), &["21", "22", "21"]))).0,
        "/terminals/2/designation"
    );
    // Missing records.
    not_found(e.execute(&InsertTerminals {
        strip: id(0x9f0),
        ..insert(None, &["21"])
    }));
    not_found(e.execute(&insert(Some(id(0x9f1)), &["21"])));
    not_found(e.execute(&InsertTerminals {
        fndef: id(0x9f2),
        ..insert(None, &["21"])
    }));
    let mut dangling = insert(None, &["21"]);
    dangling.parts = vec![PartUse {
        part: PartRef::new(id(0x9f3)),
        qty: dec3("1"),
        extra: ew_schema::Extra::new(),
    }];
    not_found(e.execute(&dangling));
    assert!(e.history().entries().is_empty(), "nothing committed");
}

/// A shifted or new seq beyond `u32::MAX` is refused at `/terminals`.
#[test]
fn insert_rejects_a_seq_beyond_u32_max() {
    let mut e = golden_a_strip();
    let t20 = terminal(&e, "20");
    set_strip(&mut e, "strip.groups", json!([])).unwrap();
    e.execute(&SetFields {
        kind: KindName::Device,
        targets: vec![t20.cast()],
        path: "terminal.seq".into(),
        value: json!(u32::MAX),
    })
    .unwrap();
    let rev = e.model().rev();
    // Shifting :20 (seq u32::MAX) up overflows.
    let t5 = terminal(&e, "5");
    assert_eq!(invalid(e.execute(&insert(Some(t5), &["21"]))).0, "/terminals");
    // At the end there is no seq after u32::MAX.
    assert_eq!(invalid(e.execute(&insert(None, &["21"]))).0, "/terminals");
    assert_eq!(e.model().rev(), rev);
}

// ---------------------------------------------------------------------------------------------------------
// SE2: terminal.move

/// §5.2's example: :13 before :11.
#[test]
fn move_13_before_11_permutes_the_changed_range() {
    let mut e = golden_a_strip();
    set_strip(&mut e, "strip.accessories", json!([accessory(12)])).unwrap();
    let before = lines(e.model());
    let (groups_before, accessories_before) = (groups(&e), strip_aspect(&e).accessories);
    let bridges = connections(&e);
    let r = e
        .execute(&MoveTerminals {
            terminals: vec![terminal(&e, "13")],
            before: Some(terminal(&e, "11")),
        })
        .unwrap();
    assert_eq!(r.outcome.primary, None);
    let s = seqs(&e);
    assert_eq!((s["13"], s["11"], s["12"]), (11, 12, 13));
    for n in (1..=10u32).chain(14..=20) {
        assert_eq!(s[&n.to_string()], n, "terminal {n} keeps its seq");
    }
    assert_eq!(groups(&e), groups_before, "the groups stay (11–14 Pneumatics)");
    assert_eq!(
        strip_aspect(&e).accessories,
        accessories_before,
        "the accessories stay"
    );
    assert_eq!(connections(&e), bridges);
    assert_eq!(e.history().entries().len(), 2, "the accessories, then the move");
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

/// Without `before` the terminals go to the end, in strip order whatever the list order.
#[test]
fn move_to_the_end() {
    let mut e = golden_a_strip();
    let before = lines(e.model());
    let groups_before = groups(&e);
    e.execute(&MoveTerminals {
        terminals: vec![terminal(&e, "5"), terminal(&e, "2")],
        before: None,
    })
    .unwrap();
    let s = seqs(&e);
    assert_eq!(s["1"], 1);
    // 3, 4, 6, …, 20 close up from seq 2; then 2 and 5 take 19 and 20.
    assert_eq!((s["3"], s["4"], s["6"], s["20"]), (2, 3, 4, 18));
    assert_eq!((s["2"], s["5"]), (19, 20));
    assert_eq!(groups(&e), groups_before);
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn move_rejections_name_their_pointer() {
    let mut e = golden_a_strip();
    let (t1, t2, t3) = (terminal(&e, "1"), terminal(&e, "2"), terminal(&e, "3"));
    let mv = |terminals: Vec<Id<Device>>, before: Option<Id<Device>>| MoveTerminals { terminals, before };
    // An empty list.
    assert_eq!(invalid(e.execute(&mv(vec![], None))).0, "/terminals");
    // A terminal listed twice.
    assert_eq!(invalid(e.execute(&mv(vec![t1, t1], None))).0, "/terminals/1");
    // A device that is not a terminal.
    assert_eq!(
        invalid(e.execute(&mv(vec![t1, DEV_101F1], None))).0,
        "/terminals/1"
    );
    // `before` among the moved.
    assert_eq!(invalid(e.execute(&mv(vec![t1, t2], Some(t2)))).0, "/before");
    // `before` that is not a terminal of the strip.
    assert_eq!(invalid(e.execute(&mv(vec![t1], Some(DEV_101F1)))).0, "/before");
    // An order that does not change: :2 before :3, :20 to the end.
    let (path, message) = invalid(e.execute(&mv(vec![t2], Some(t3))));
    assert_eq!(
        (path.as_str(), message.as_str()),
        ("/terminals", "the terminals are already there")
    );
    let t20 = terminal(&e, "20");
    assert_eq!(invalid(e.execute(&mv(vec![t20], None))).0, "/terminals");
    // Missing records.
    not_found(e.execute(&mv(vec![id(0x9f0)], None)));
    not_found(e.execute(&mv(vec![t1], Some(id(0x9f1)))));
    assert!(e.history().entries().is_empty(), "nothing committed");
}

/// Terminals of two strips are refused at the first of the other strip.
#[test]
fn move_rejects_terminals_of_two_strips() {
    let mut e = golden_a_strip();
    let other = new_strip(&mut e);
    let r = e
        .execute(&InsertTerminals {
            strip: other,
            ..insert(None, &["1"])
        })
        .unwrap();
    let foreign: Id<Device> = r.outcome.primary.expect("terminal").1.cast();
    let t1 = terminal(&e, "1");
    let (path, _) = invalid(e.execute(&MoveTerminals {
        terminals: vec![t1, foreign],
        before: None,
    }));
    assert_eq!(path, "/terminals/1");
    let (path, _) = invalid(e.execute(&MoveTerminals {
        terminals: vec![t1],
        before: Some(foreign),
    }));
    assert_eq!(path, "/before");
}

// ---------------------------------------------------------------------------------------------------------
// SE3: strip.accessories through fields.set

#[test]
fn strip_accessories_are_set_by_fields_set() {
    let mut e = golden_a_strip();
    set_strip(&mut e, "strip.accessories", json!([accessory(20)])).unwrap();
    let accessories = strip_aspect(&e).accessories;
    assert_eq!(accessories.len(), 1);
    assert_eq!(accessories[0].after_seq, 20);
    assert_eq!(accessories[0].part, PartRef::new(PART_MCB));
    assert_eq!(e.history().entries().len(), 1);
}

/// A part copy that does not exist is refused by the link check at the end of the transaction (EW-INV-001).
#[test]
fn a_dangling_accessory_part_is_refused_by_the_link_check() {
    let mut e = golden_a_strip();
    let before = lines(e.model());
    let r = set_strip(
        &mut e,
        "strip.accessories",
        json!([{"after_seq": 20, "kind": "end_clamp", "part": {"copy": id::<ew_schema::Part>(0x9f4)}}]),
    );
    match r {
        Err(CmdError::Model(ModelError::Invariant { id, message, .. })) => {
            assert_eq!(id, "EW-INV-001");
            assert!(message.contains("strip.accessories[].part.copy"), "{message}");
        }
        other => panic!("expected the link check to refuse, got {other:?}"),
    }
    assert_eq!(lines(e.model()), before);
    assert!(e.history().entries().is_empty());
}
