// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Medium-severity review findings: junction clean-up keeps assigned numbers (F10), `device.place` and
//! `rep.move` checks and hints (F11), `affected_pages` for potentials, bonds and conductors (F14), and the
//! rule ID of a prevent-rule rejection (F17).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    ConnectHint, CreateDevice, DefinePotential, DeleteSelection, EndpointParam, KindName, MoveReps,
    PlaceSymbol, PlaceTarget, TargetRef,
};
use ew_commands::{CmdError, CommandRegistry, Engine, EngineOptions};
use ew_ids::Id;
use ew_ids::v7::SeededSource;
use ew_model::{Entity, Model, PreventHook, PreventViolation};
use ew_schema::{Connection, Junction, Kind, NumberSpec, Page, PageType, Record, RepType, Representation};
use ew_units::Orient2;

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn exists<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> bool {
    e.model().table::<T>().get(&id).is_some()
}

fn golden_with(f: impl FnOnce(&mut Vec<Record>)) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    f(&mut records);
    Model::from_records(p, records, library()).expect("valid model")
}

/// The golden model with `PAGE_101` turned into a page of the given type.
fn with_page_type(kind: PageType) -> Model {
    golden_with(|records| {
        for r in records.iter_mut() {
            if let Record::Page(p) = r {
                if p.id == PAGE_101 {
                    p.kind = kind.clone();
                }
            }
        }
    })
}

fn place(page: Id<Page>, rep_type: Option<RepType>) -> PlaceSymbol {
    PlaceSymbol {
        page,
        symbol: SYM_TWO,
        pos: pt(0, 0),
        orient: Orient2::R0,
        rep_type,
        variant: 0,
        target: PlaceTarget::NewDevice {
            letter: ident("F"),
            fndef: None,
            kind: None,
        },
        connect: vec![],
    }
}

fn rejection_reason<T: std::fmt::Debug>(r: Result<T, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// F11 (a): no dangling alias

#[test]
fn registry_has_no_dangling_alias() {
    let registry = CommandRegistry::builtin();
    assert!(registry.get("device.place_from_part").is_none());
    for command in registry.commands() {
        let meta = command.meta();
        for alias in meta.aliases {
            let resolved = registry.get(alias).expect("alias resolves");
            assert_eq!(resolved.meta().name, meta.name, "alias `{alias}`");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// F11 (b): the representation type must suit the page type

#[test]
fn place_refuses_a_rep_type_the_page_does_not_allow() {
    let mut e = engine(golden_model());
    let reason = rejection_reason(e.execute(&place(PAGE_102, Some(RepType::Overview))));
    assert!(
        reason.contains("Overview") && reason.contains("MultiLine") && reason.contains("ContactImage"),
        "{reason}"
    );
    assert_eq!(e.model().rev().0, 0);

    // The same type on an overview page, and a multi-line type on the multi-line page, are fine.
    let mut e = engine(with_page_type(PageType::Overview));
    e.execute(&place(PAGE_101, Some(RepType::Overview)))
        .expect("overview on overview");
    rejection_reason(e.execute(&place(PAGE_101, Some(RepType::MultiLine))));
    let mut e = engine(golden_model());
    e.execute(&place(PAGE_102, Some(RepType::ContactImage)))
        .expect("contact image on multi-line");
    // The default is the symbol's type (multi-line).
    e.execute(&place(PAGE_102, None)).expect("default type");

    // A generated page takes no representations.
    let mut e = engine(with_page_type(PageType::TableOfContents));
    let reason = rejection_reason(e.execute(&place(PAGE_101, None)));
    assert!(reason.contains("no representations"), "{reason}");
}

#[test]
fn move_refuses_a_page_whose_type_does_not_allow_the_rep() {
    let mut e = engine(with_page_type(PageType::Overview));
    let reason = rejection_reason(e.execute(&MoveReps {
        reps: vec![REP_102G1],
        delta: pt(0, 0),
        page: Some(PAGE_101),
    }));
    assert!(
        reason.contains("MultiLine") && reason.contains("Overview"),
        "{reason}"
    );
    assert_eq!(e.model().rev().0, 0);
    assert_eq!(row::<Representation>(&e, REP_102G1).page, PAGE_102);
}

// ---------------------------------------------------------------------------------------------------------
// F11 (c), (d): hints

const W: Id<Connection> = id(0x4200);

fn model_with_wire() -> Model {
    golden_with(|records| {
        let mut w = connection(W, Some(PAGE_102), cp(FN_102F2, 4), cp(FN_102F2, 5));
        w.route = vec![pt(0, 0), pt(4 * G, 0)];
        records.push(Record::Connection(w));
    })
}

#[test]
fn a_hint_onto_a_wire_splits_it_with_a_junction() {
    let mut e = engine(model_with_wire());
    let mut cmd = place(PAGE_102, None);
    cmd.pos = pt(2 * G, 2 * G);
    cmd.connect = vec![ConnectHint {
        cp: 0,
        to: EndpointParam::OnConnection {
            connection: W,
            at: pt(2 * G, 0),
        },
    }];
    let r = e.execute(&cmd).expect("hint onto a wire");
    assert!(r.outcome.notices.is_empty());
    let junctions: Vec<Id<Junction>> = r
        .created
        .iter()
        .filter(|k| k.0 == Kind::Junction)
        .map(|k| typed(*k))
        .collect();
    assert_eq!(junctions.len(), 1);
    let j = row(&e, junctions[0]);
    assert_eq!(j.at, pt(2 * G, 0));
    assert_eq!(j.page, PAGE_102);
    let new_wires: Vec<Id<Connection>> = r
        .created
        .iter()
        .filter(|k| k.0 == Kind::Connection)
        .map(|k| typed(*k))
        .collect();
    assert_eq!(new_wires.len(), 2, "the second half of W and the hint's wire");
    assert_eq!(j.sequence.len(), 3);
    assert_eq!(j.sequence[0], W);
    assert!(j.sequence[1..].iter().all(|c| new_wires.contains(c)));
    let hint_wire = row(&e, j.sequence[2]);
    assert_eq!(hint_wire.b, at_junction(junctions[0]));
    assert_eq!(hint_wire.page, Some(PAGE_102));
    let first = row(&e, W);
    assert_eq!(first.b, at_junction(junctions[0]));
    assert_eq!(first.route, vec![pt(0, 0), pt(2 * G, 0)]);
    let second = row(&e, j.sequence[1]);
    assert_eq!(second.a, at_junction(junctions[0]));
    assert_eq!(second.b, cp(FN_102F2, 5));
}

#[test]
fn a_hint_onto_a_wire_off_the_grid_is_rejected() {
    let mut e = engine(model_with_wire());
    let mut cmd = place(PAGE_102, None);
    cmd.pos = pt(2 * G, 2 * G);
    cmd.connect = vec![ConnectHint {
        cp: 0,
        to: EndpointParam::OnConnection {
            connection: W,
            at: pt(2 * G + 1, 0),
        },
    }];
    let reason = rejection_reason(e.execute(&cmd));
    assert!(reason.contains("/connect/0/to/on_connection/at"), "{reason}");
}

#[test]
fn a_hint_whose_target_vanished_is_dropped_and_reported() {
    let mut e = engine(golden_model());
    let mut cmd = place(PAGE_102, None);
    cmd.connect = vec![ConnectHint {
        cp: 0,
        to: EndpointParam::Junction { junction: id(0xccc) },
    }];
    let r = e.execute(&cmd).expect("the command still succeeds");
    assert_eq!(r.outcome.notices.len(), 1, "{:?}", r.outcome.notices);
    assert!(r.outcome.notices[0].contains("hint 1"), "{:?}", r.outcome.notices);
    assert!(
        r.created.iter().all(|k| k.0 != Kind::Connection),
        "no wire for the dropped hint"
    );
    // An unknown symbol CP is still a malformed command.
    cmd.connect = vec![ConnectHint {
        cp: 9,
        to: EndpointParam::Junction { junction: id(0xccc) },
    }];
    rejection_reason(e.execute(&cmd));
}

// ---------------------------------------------------------------------------------------------------------
// F14: pages affected by potentials

fn define_at(at: EndpointParam) -> DefinePotential {
    DefinePotential {
        at,
        name: ident("L1"),
        ptype: None,
        voltage: None,
        env: Default::default(),
        circuit_class: None,
        props: Default::default(),
    }
}

#[test]
fn potential_define_notifies_the_page_of_its_cp() {
    let mut e = engine(golden_model());
    e.drain_changes();
    e.execute(&define_at(EndpointParam::Cp {
        function: FN_102F2,
        idx: 0,
    }))
    .expect("potential.define");
    let changes = e.drain_changes();
    let last = changes.last().expect("a change entry");
    assert!(last.touched_pages.contains(&PAGE_102), "{:?}", last.touched_pages);
}

#[test]
fn potential_define_at_a_junction_notifies_the_junction_page() {
    let mut e = engine(golden_model());
    e.drain_changes();
    e.execute(&define_at(EndpointParam::Junction { junction: J5 }))
        .expect("potential.define");
    let changes = e.drain_changes();
    let last = changes.last().expect("a change entry");
    assert_eq!(last.touched_pages, vec![PAGE_102]);
}

// ---------------------------------------------------------------------------------------------------------
// F17: the rule ID of a prevent rejection

struct RejectAll;

impl PreventHook for RejectAll {
    fn check(
        &self,
        _base: &Model,
        _candidate: &Model,
        _touched: &[ew_model::AnyKey],
    ) -> Result<(), PreventViolation> {
        Err(PreventViolation {
            rule: "EW-TEST-001".to_owned(),
            message: "no candidate passes".to_owned(),
            keys: vec![],
        })
    }
}

#[test]
fn a_prevent_rule_rejection_carries_its_rule_id() {
    let mut e = Engine::new(
        golden_model(),
        EngineOptions {
            ids: Some(Box::new(SeededSource::new(SEED))),
            prevent: Some(Box::new(RejectAll)),
            ..EngineOptions::default()
        },
    );
    let err = e
        .execute(&CreateDevice {
            letter: ident("F"),
            counter: None,
            kind: None,
            parts: vec![],
            function_text: Default::default(),
            structure: Default::default(),
            strip: None,
            main_fndef: None,
        })
        .unwrap_err();
    match &err {
        CmdError::Prevented { rule, finding, .. } => {
            assert_eq!(rule, "EW-TEST-001");
            assert_eq!(finding, "no candidate passes");
        }
        other => panic!("expected Prevented, got {other:?}"),
    }
    assert_eq!(err.to_string(), "EW-TEST-001: no candidate passes");
}

// ---------------------------------------------------------------------------------------------------------
// F10: junction clean-up keeps assigned numbers

const JX: Id<Junction> = id(0x5100);
const WA: Id<Connection> = id(0x4300);
const WB: Id<Connection> = id(0x4301);
const WC: Id<Connection> = id(0x4302);

/// A junction with three wires WA (to CP 10), WB (to CP 11) and WC (to CP 12), listed in that order, with
/// the numbers of WA and WB as given.
fn model_with_star(na: NumberSpec, nb: NumberSpec) -> Model {
    golden_with(|records| {
        let mut a = connection(WA, Some(PAGE_102), cp(FN_102F2, 10), at_junction(JX));
        a.number = na;
        let mut b = connection(WB, Some(PAGE_102), at_junction(JX), cp(FN_102F2, 11));
        b.number = nb;
        let c = connection(WC, Some(PAGE_102), at_junction(JX), cp(FN_102F2, 12));
        records.push(Record::Junction(junction(JX, PAGE_102, vec![WA, WB, WC])));
        records.push(Record::Connection(a));
        records.push(Record::Connection(b));
        records.push(Record::Connection(c));
    })
}

fn delete_wc() -> DeleteSelection {
    DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Connection,
            id: WC.cast(),
        }],
    }
}

#[test]
fn dissolving_keeps_the_member_that_carries_a_number() {
    let frozen = NumberSpec::Frozen(ident("7"));
    let mut e = engine(model_with_star(NumberSpec::Unassigned, frozen.clone()));
    e.execute(&delete_wc()).expect("edit.delete");
    assert!(!exists(&e, JX));
    assert!(!exists(&e, WA), "the unnumbered first member is the one removed");
    let kept = row(&e, WB);
    assert_eq!(kept.number, frozen);
    assert_eq!(kept.a, cp(FN_102F2, 10));
    assert_eq!(kept.b, cp(FN_102F2, 11));
}

#[test]
fn dissolving_keeps_the_first_member_when_numbers_are_equal_or_absent() {
    let frozen = NumberSpec::Frozen(ident("7"));
    for (na, nb) in [
        (NumberSpec::Unassigned, NumberSpec::Unassigned),
        (frozen.clone(), frozen.clone()),
        (frozen.clone(), NumberSpec::Unassigned),
    ] {
        let mut e = engine(model_with_star(na, nb));
        e.execute(&delete_wc()).expect("edit.delete");
        assert!(exists(&e, WA));
        assert!(!exists(&e, WB));
    }
}

#[test]
fn dissolving_two_different_numbers_is_rejected() {
    let mut e = engine(model_with_star(
        NumberSpec::Frozen(ident("5")),
        NumberSpec::Frozen(ident("7")),
    ));
    let reason = rejection_reason(e.execute(&delete_wc()));
    assert!(reason.contains("\"5\"") && reason.contains("\"7\""), "{reason}");
    assert!(reason.contains("clear one of the numbers"), "{reason}");
    assert_eq!(e.model().rev().0, 0);
    assert!(exists(&e, WC) && exists(&e, JX));
}
