// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Property tests of the command layer (ARCH §13, doc 18 §3.3): random command sequences over the golden
//! model, with the laws of undo, previews, normaliser convergence, both renumbering commands and
//! determinism. Steps name their targets by index into the live records, so a sequence stays meaningful
//! whatever the earlier steps did; a step that cannot be resolved is skipped, and a command error is fine
//! (the command refused the edit), but `CmdError::Internal` and panics are failures.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use std::sync::Arc;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    Connect, CreatePage, DeleteConnections, DeleteSelection, DeviceScope, EndpointParam, KindName, MoveReps,
    PageScope, PlaceSymbol, PlaceTarget, RenumberDevices, RenumberOptions, RenumberPages, SetFields,
    TargetRef, TransformReps,
};
use ew_commands::{CmdError, Cost, DynCommand, Engine, Receipt};
use ew_model::{AnyKey, Entity, Model};
use ew_schema::{Connection, Device, Function, Kind, Representation};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// One generated step. Targets are indexes into the live records of the current model.
#[derive(Clone, Debug)]
enum Step {
    Place {
        two_cp: bool,
        letter: u8,
        page: bool,
        x: i64,
        y: i64,
    },
    Move {
        rep: usize,
        dx: i64,
        dy: i64,
    },
    Transform {
        rep: usize,
        rotate: u8,
        mirror: bool,
    },
    Draw {
        f1: usize,
        cp1: u16,
        f2: usize,
        cp2: u16,
    },
    DrawOnConnection {
        conn: usize,
        pick: usize,
        f: usize,
        cp: u16,
    },
    DeleteConnection {
        conn: usize,
    },
    SetText {
        device: usize,
        text: u8,
    },
    DeleteRep {
        rep: usize,
    },
    RenumberAll,
    RenumberSection {
        start_101: bool,
    },
    CreatePage,
    Undo,
    Redo,
}

const LETTERS: [&str; 3] = ["F", "K", "Q"];
const UNIT: i64 = G;

fn edit_step() -> BoxedStrategy<Step> {
    prop_oneof![
        3 => (any::<bool>(), 0u8..3, any::<bool>(), 0i64..6, 0i64..6)
            .prop_map(|(two_cp, letter, page, x, y)| Step::Place { two_cp, letter, page, x, y }),
        2 => (0usize..64, -3i64..=3, -3i64..=3).prop_map(|(rep, dx, dy)| Step::Move { rep, dx, dy }),
        1 => (0usize..64, 0u8..4, any::<bool>())
            .prop_map(|(rep, rotate, mirror)| Step::Transform { rep, rotate, mirror }),
        3 => (0usize..64, 0u16..2, 0usize..64, 0u16..2)
            .prop_map(|(f1, cp1, f2, cp2)| Step::Draw { f1, cp1, f2, cp2 }),
        2 => (0usize..64, 0usize..8, 0usize..64, 0u16..2)
            .prop_map(|(conn, pick, f, cp)| Step::DrawOnConnection { conn, pick, f, cp }),
        1 => (0usize..64).prop_map(|conn| Step::DeleteConnection { conn }),
        1 => (0usize..64, 0u8..4).prop_map(|(device, text)| Step::SetText { device, text }),
        1 => (0usize..64).prop_map(|rep| Step::DeleteRep { rep }),
        1 => Just(Step::RenumberAll),
        1 => any::<bool>().prop_map(|start_101| Step::RenumberSection { start_101 }),
        1 => Just(Step::CreatePage),
    ]
    .boxed()
}

fn steps(with_history: bool) -> BoxedStrategy<Vec<Step>> {
    let step = if with_history {
        prop_oneof![
            10 => edit_step(),
            1 => Just(Step::Undo),
            1 => Just(Step::Redo),
        ]
        .boxed()
    } else {
        edit_step()
    };
    prop::collection::vec(step, 1..=10).boxed()
}

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 128,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// The `i % len`-th live record of a table, in ID order.
fn nth<T: Entity>(model: &Model, i: usize) -> Option<Arc<T>> {
    let table = model.table::<T>();
    let len = table.iter().count();
    if len == 0 {
        return None;
    }
    table.iter().nth(i % len).map(|(_, row)| Arc::clone(row))
}

fn boxed(cmd: impl DynCommand + 'static) -> Option<Box<dyn DynCommand>> {
    Some(Box::new(cmd))
}

/// The command of a step on the current model; `None` for undo, redo and unresolvable steps.
fn build(model: &Model, step: &Step) -> Option<Box<dyn DynCommand>> {
    match step {
        Step::Place {
            two_cp,
            letter,
            page,
            x,
            y,
        } => boxed(PlaceSymbol {
            copies: Vec::new(),
            page: if *page { PAGE_101 } else { PAGE_102 },
            symbol: if *two_cp { SYM_TWO } else { SYM },
            pos: pt(x * UNIT, y * UNIT),
            orient: Default::default(),
            rep_type: None,
            variant: 0,
            target: PlaceTarget::NewDevice {
                letter: ident(LETTERS[usize::from(*letter) % LETTERS.len()]),
                fndef: if *two_cp { None } else { Some(FNDEF_CONTACT) },
                kind: None,
            },
            connect: vec![],
        }),
        Step::Move { rep, dx, dy } => boxed(MoveReps {
            items: Vec::new(),
            steps: None,
            reps: vec![nth::<Representation>(model, *rep)?.id],
            delta: pt(dx * UNIT, dy * UNIT),
            page: None,
        }),
        Step::Transform { rep, rotate, mirror } => boxed(TransformReps {
            reps: vec![nth::<Representation>(model, *rep)?.id],
            rotate: *rotate,
            mirror: *mirror,
            variant: None,
        }),
        Step::Draw { f1, cp1, f2, cp2 } => boxed(Connect {
            a: EndpointParam::Cp {
                function: nth::<Function>(model, *f1)?.id,
                idx: *cp1,
            },
            b: EndpointParam::Cp {
                function: nth::<Function>(model, *f2)?.id,
                idx: *cp2,
            },
            page: Some(PAGE_102),
            route: vec![],
            kind: Default::default(),
        }),
        Step::DrawOnConnection { conn, pick, f, cp } => {
            let c = nth::<Connection>(model, *conn)?;
            let at = if c.route.is_empty() {
                pt(G, G)
            } else {
                c.route[pick % c.route.len()]
            };
            boxed(Connect {
                a: EndpointParam::OnConnection { connection: c.id, at },
                b: EndpointParam::Cp {
                    function: nth::<Function>(model, *f)?.id,
                    idx: *cp,
                },
                page: None,
                route: vec![],
                kind: Default::default(),
            })
        }
        Step::DeleteConnection { conn } => boxed(DeleteConnections {
            connections: vec![nth::<Connection>(model, *conn)?.id],
        }),
        Step::SetText { device, text } => boxed(SetFields {
            kind: KindName::Device,
            targets: vec![nth::<Device>(model, *device)?.id.cast()],
            path: "function_text".into(),
            value: serde_json::json!(format!("text {text}")),
        }),
        Step::DeleteRep { rep } => boxed(DeleteSelection {
            targets: vec![TargetRef {
                kind: KindName::Representation,
                id: nth::<Representation>(model, *rep)?.id.cast(),
            }],
        }),
        Step::RenumberAll => boxed(renumber_devices()),
        Step::RenumberSection { start_101 } => boxed(renumber_pages(if *start_101 { 101 } else { 300 })),
        Step::CreatePage => {
            let mut c = CreatePage::at(None, 0);
            c.number = None;
            c.after = Some(PAGE_102);
            boxed(c)
        }
        Step::Undo | Step::Redo => None,
    }
}

fn renumber_devices() -> RenumberDevices {
    RenumberDevices {
        scope: DeviceScope::Project,
        scheme: Default::default(),
        include_frozen: false,
        options: RenumberOptions::default(),
    }
}

fn renumber_pages(start: u32) -> RenumberPages {
    RenumberPages {
        scope: PageScope::Section(SEC1),
        start,
        step: 1,
        keep_gaps: false,
    }
}

/// What one step did.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Ran {
    Skipped,
    Cmd(Result<Receipt, CmdError>),
    Undo(Result<(), CmdError>),
    Redo(Result<(), CmdError>),
}

fn no_internal<T>(r: Result<T, CmdError>) -> Result<Result<T, CmdError>, TestCaseError> {
    match r {
        Err(CmdError::Internal { reason }) => Err(TestCaseError::fail(format!("internal error: {reason}"))),
        other => Ok(other),
    }
}

/// Runs a command by preview and commit.
fn via_preview(e: &mut Engine, cmd: &dyn DynCommand) -> Result<Receipt, CmdError> {
    let preview = e.preview(cmd)?;
    e.commit(preview)
}

/// Runs one step. Interactive commands are executed, or previewed and committed when `previews` is set;
/// bulk commands are always previewed and committed. `CmdError::Internal` fails the case.
fn run_step(e: &mut Engine, step: &Step, previews: bool) -> Result<Ran, TestCaseError> {
    match step {
        Step::Undo => Ok(Ran::Undo(no_internal(
            e.undo(&ew_commands::Origin::Gui).map(|_| ()),
        )?)),
        Step::Redo => Ok(Ran::Redo(no_internal(
            e.redo(&ew_commands::Origin::Gui).map(|_| ()),
        )?)),
        _ => {
            let Some(cmd) = build(e.model(), step) else {
                return Ok(Ran::Skipped);
            };
            let result = if previews || cmd.cost(e.model()) == Cost::Bulk {
                via_preview(e, &*cmd)
            } else {
                e.execute(&*cmd)
            };
            Ok(Ran::Cmd(no_internal(result)?))
        }
    }
}

fn run_all(e: &mut Engine, sequence: &[Step]) -> Result<(), TestCaseError> {
    for step in sequence {
        run_step(e, step, false)?;
    }
    Ok(())
}

/// The rows of one kind, with one member of an object member of their JSON removed.
fn rows_without(model: &Model, kind: Kind, members: &[(&str, &str)]) -> Vec<(AnyKey, serde_json::Value)> {
    lines(model)
        .into_iter()
        .filter(|(key, _)| key.0 == kind)
        .map(|(key, text)| {
            let mut value: serde_json::Value = serde_json::from_str(&text).expect("a row is JSON");
            for member in members {
                if let Some(obj) = value.get_mut(member.0).and_then(|v| v.as_object_mut()) {
                    obj.remove(member.1);
                }
            }
            (key, value)
        })
        .collect()
}

fn lines_except(model: &Model, kind: Kind) -> Vec<(AnyKey, String)> {
    lines(model)
        .into_iter()
        .filter(|(key, _)| key.0 != kind)
        .collect()
}

proptest! {
    #![proptest_config(config())]

    /// `undo(all) ∘ apply(cmds) = id`.
    #[test]
    fn undo_of_everything_restores_the_initial_model(sequence in steps(false)) {
        let initial = golden_model();
        let initial_lines = lines(&initial);
        let mut e = engine(initial.clone());
        run_all(&mut e, &sequence)?;
        while e.undo(&ew_commands::Origin::Gui).is_ok() {}
        prop_assert_eq!(lines(e.model()), initial_lines);
        prop_assert!(e.model().content_eq(&initial));
        prop_assert!(!e.is_dirty());
    }

    /// Preview and commit equal execute, after every step.
    #[test]
    fn preview_then_commit_equals_execute(sequence in steps(true)) {
        let mut a = engine(golden_model());
        let mut b = engine(golden_model());
        for step in &sequence {
            let ra = run_step(&mut a, step, false)?;
            let rb = run_step(&mut b, step, true)?;
            prop_assert_eq!(ra, rb, "step {:?}", step);
            prop_assert_eq!(lines(a.model()), lines(b.model()), "after {:?}", step);
        }
    }

    /// The normalisers converge within MAX_PASSES (plus the confirming pass).
    #[test]
    fn normalisers_converge(sequence in steps(true)) {
        let mut e = engine(golden_model());
        for step in &sequence {
            if let Ran::Cmd(Ok(receipt)) = run_step(&mut e, step, false)? {
                prop_assert!(receipt.passes <= ew_commands::normalise::MAX_PASSES + 1, "{} passes after {:?}", receipt.passes, step);
            }
        }
    }

    /// Undo restores the exact bytes of the previous state, redo those of the step (AC-18).
    #[test]
    fn every_step_undoes_to_exact_bytes(sequence in steps(false)) {
        let mut e = engine(golden_model());
        for step in &sequence {
            let before = lines(e.model());
            let rev = e.model().rev();
            let Ran::Cmd(Ok(_)) = run_step(&mut e, step, false)? else {
                continue;
            };
            if e.model().rev() == rev {
                continue;
            }
            let after = lines(e.model());
            prop_assert!(e.undo(&ew_commands::Origin::Gui).is_ok(), "undo of {:?}", step);
            prop_assert_eq!(lines(e.model()), before, "undo of {:?}", step);
            prop_assert!(e.redo(&ew_commands::Origin::Gui).is_ok(), "redo of {:?}", step);
            prop_assert_eq!(lines(e.model()), after, "redo of {:?}", step);
        }
    }

    /// `numbering.renumber` keeps every reference and the topology, and is idempotent.
    #[test]
    fn device_renumbering_keeps_references_and_is_idempotent(sequence in steps(true)) {
        let mut e = engine(golden_model());
        run_all(&mut e, &sequence)?;
        let before = e.model().clone();
        let cmd = renumber_devices();
        let receipt = via_preview(&mut e, &cmd);
        prop_assert!(receipt.is_ok(), "renumber failed: {:?}", receipt);
        let after = e.model();
        // Every non-device row, connections and their endpoints among them, is byte-identical.
        prop_assert_eq!(lines_except(&before, Kind::Device), lines_except(after, Kind::Device));
        // Devices differ at most in their counter.
        prop_assert_eq!(
            rows_without(&before, Kind::Device, &[("tag", "counter")]),
            rows_without(after, Kind::Device, &[("tag", "counter")])
        );
        let again = e.preview(&cmd);
        prop_assert!(
            again.as_ref().is_ok_and(|p| p.ops().is_empty()),
            "second renumber: {:?}",
            again.map(|p| p.ops().len())
        );
    }

    /// `page.renumber` changes only the folio numbers of pages and the sub-page labels that carry them (FT-475).
    #[test]
    fn page_renumbering_keeps_references(sequence in steps(true), start_101 in any::<bool>()) {
        let mut e = engine(golden_model());
        run_all(&mut e, &sequence)?;
        let before = e.model().clone();
        let cmd = renumber_pages(if start_101 { 101 } else { 300 });
        // A folio collision outside the scope is a legitimate refusal.
        if no_internal(via_preview(&mut e, &cmd))?.is_err() {
            return Ok(());
        }
        let after = e.model();
        prop_assert_eq!(lines_except(&before, Kind::Page), lines_except(after, Kind::Page));
        prop_assert_eq!(
            rows_without(&before, Kind::Page, &[("folio", "number"), ("folio", "label")]),
            rows_without(after, Kind::Page, &[("folio", "number"), ("folio", "label")])
        );
    }

    /// The same sequence on two engines with the same seed gives identical models.
    #[test]
    fn a_sequence_is_deterministic(sequence in steps(true)) {
        let mut a = engine(golden_model());
        let mut b = engine(golden_model());
        for step in &sequence {
            let ra = run_step(&mut a, step, false)?;
            let rb = run_step(&mut b, step, false)?;
            prop_assert_eq!(ra, rb);
        }
        prop_assert_eq!(lines(a.model()), lines(b.model()));
    }
}

/// The generator is not vacuous: a fixed sequence of every step kind mostly commits.
#[test]
fn the_step_kinds_commit() {
    let mut e = engine(golden_model());
    let sequence = [
        Step::Place {
            two_cp: true,
            letter: 0,
            page: false,
            x: 1,
            y: 1,
        },
        Step::Place {
            two_cp: false,
            letter: 1,
            page: true,
            x: 2,
            y: 1,
        },
        Step::Move {
            rep: 3,
            dx: 1,
            dy: -1,
        },
        Step::Transform {
            rep: 3,
            rotate: 1,
            mirror: true,
        },
        Step::Draw {
            f1: 0,
            cp1: 0,
            f2: 5,
            cp2: 1,
        },
        Step::DrawOnConnection {
            conn: 0,
            pick: 0,
            f: 3,
            cp: 0,
        },
        Step::DeleteConnection { conn: 1 },
        Step::SetText { device: 0, text: 1 },
        Step::DeleteRep { rep: 0 },
        Step::RenumberAll,
        Step::RenumberSection { start_101: false },
        Step::CreatePage,
    ];
    let committed: Vec<bool> = sequence
        .iter()
        .map(|s| matches!(run_step(&mut e, s, false), Ok(Ran::Cmd(Ok(_)))))
        .collect();
    assert!(committed.iter().filter(|c| **c).count() >= 9, "{committed:?}");
}
