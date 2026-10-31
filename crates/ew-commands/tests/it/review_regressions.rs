// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Regressions of the independent re-review of `ew-commands`: a split point on a wire at a CP-exact
//! coordinate that is 1 nm off a rational grid (N1), and the origin of a re-run foreign preview (N3).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{Connect, ConnectHint, CreatePage, EndpointParam, PlaceSymbol, PlaceTarget};
use ew_commands::geometry::on_grid;
use ew_commands::{
    CmdError, Command, CommandMeta, CommandRegistry, Engine, EngineOptions, Exposure, Origin, Outcome,
    Submit, TxBuilder,
};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    Connection, ConnectionKind, Device, FnRole, Function, Junction, Kind, Record, Representation,
};
use ew_units::{Nm, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------------------------------------
// Helpers

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn rejection<T: std::fmt::Debug>(r: Result<T, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

fn cp_param(function: Id<Function>, idx: u16) -> EndpointParam {
    EndpointParam::Cp { function, idx }
}

// ---------------------------------------------------------------------------------------------------------
// N1: a split point on a wire at a CP-exact coordinate that is off the rational grid

/// One QET grid step is 7_937_500 / 3 nm, so two steps are 5_291_666.67 nm: the grid point is 5_291_667,
/// and a CP drawn at 5_291_666 is 1 nm off it.
const Y_CP: i64 = 5_291_666;
const Y_GRID: i64 = 5_291_667;

/// Two placed devices whose second CP (10 mm below the insertion point) is drawn at `y = Y_CP`, at x = 0
/// and x = 4G. The positions are in the records, not placed by a command (a command would refuse them).
fn model_with_cps_off_the_grid() -> (Model, Id<Function>, Id<Function>) {
    let (p, mut records, _) = mini_golden_parts();
    let mut fns = Vec::new();
    for (n, x) in [(0u128, 0i64), (1, 4 * G)] {
        let (dev, f, r): (Id<Device>, Id<Function>, Id<Representation>) =
            (id(0x1000 + n), id(0x1100 + n), id(0x1200 + n));
        records.push(Record::Device(device(dev, "K", 1 + n as u32)));
        records.push(Record::Function(function(f, dev, FNDEF_CONTACT, FnRole::Main)));
        let mut rep = rep(r, PAGE_102, vec![f]);
        rep.symbol = SYM_TWO;
        rep.pos = pt(x, Y_CP - MM10);
        records.push(Record::Representation(rep));
        fns.push(f);
    }
    let model = Model::from_records(p, records, library()).expect("valid model");
    (model, fns[0], fns[1])
}

/// The engine with the wire drawn between the two CPs: its ends are CP-exact, its route is horizontal at
/// `Y_CP`.
fn engine_with_wire() -> (Engine, Id<Connection>, Id<Function>) {
    let (model, fa, fb) = model_with_cps_off_the_grid();
    let grid = grid();
    assert!(!on_grid(grid, Nm(Y_CP)), "the CP is off the grid");
    assert!(on_grid(grid, Nm(Y_GRID)), "the grid point next to it");
    assert!(on_grid(grid, Nm(2 * G)) && on_grid(grid, Nm(4 * G)));
    let mut e = engine(model);
    let r = e
        .execute(&Connect {
            a: cp_param(fa, 1),
            b: cp_param(fb, 1),
            page: Some(PAGE_102),
            route: vec![pt(0, Y_CP), pt(4 * G, Y_CP)],
            kind: ConnectionKind::Wire,
        })
        .expect("a wire between two CPs is accepted");
    let w: Id<Connection> = typed(r.outcome.primary.expect("primary"));
    (e, w, fa)
}

fn branch(w: Id<Connection>, at: Pt2, to: Id<Function>) -> Connect {
    Connect {
        a: EndpointParam::OnConnection { connection: w, at },
        b: cp_param(to, 0),
        page: None,
        route: vec![],
        kind: ConnectionKind::Wire,
    }
}

#[test]
fn n1_a_wire_between_off_grid_cps_can_be_branched_on_its_constant_coordinate() {
    let (mut e, w, fa) = engine_with_wire();
    let r = e
        .execute(&branch(w, pt(2 * G, Y_CP), fa))
        .expect("the split point is CP-exact on the segment");
    let junctions: Vec<Id<Junction>> = r
        .created
        .iter()
        .filter(|k| k.0 == Kind::Junction)
        .map(|k| typed(*k))
        .collect();
    assert_eq!(junctions.len(), 1);
    let j = row(&e, junctions[0]);
    assert_eq!(j.at, pt(2 * G, Y_CP));
    assert_eq!(j.sequence.len(), 3);
    assert_eq!(row(&e, w).route, vec![pt(0, Y_CP), pt(2 * G, Y_CP)]);
    let second = row(&e, j.sequence[1]);
    assert_eq!(second.route, vec![pt(2 * G, Y_CP), pt(4 * G, Y_CP)]);
}

#[test]
fn n1_the_neighbouring_grid_line_is_not_on_the_route() {
    let (mut e, w, fa) = engine_with_wire();
    let before = lines(e.model());
    let reason = rejection(e.execute(&branch(w, pt(2 * G, Y_GRID), fa)));
    assert!(reason.contains("not on the route"), "{reason}");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn n1_an_off_grid_x_is_still_rejected() {
    let (mut e, w, fa) = engine_with_wire();
    let before = lines(e.model());
    // y is the CP-exact coordinate of the segment, x is on the segment but not on the grid.
    let reason = rejection(e.execute(&branch(w, pt(2 * G + 1, Y_CP), fa)));
    assert_eq!(
        reason,
        "the branch point at x 15.9, y 5.3 mm on page 102 is not on the drawing grid"
    );
    assert_eq!(lines(e.model()), before);
}

#[test]
fn n1_a_hint_onto_the_wire_takes_the_same_rule() {
    let (mut e, w, _) = engine_with_wire();
    let hint = |at: Pt2| PlaceSymbol {
        copies: Vec::new(),
        page: PAGE_102,
        symbol: SYM_TWO,
        pos: pt(2 * G, 2 * G),
        orient: Orient2::R0,
        rep_type: None,
        variant: 0,
        target: PlaceTarget::NewDevice {
            letter: ident("F"),
            fndef: None,
            kind: None,
        },
        connect: vec![ConnectHint {
            cp: 0,
            to: EndpointParam::OnConnection { connection: w, at },
        }],
    };

    let reason = rejection(e.execute(&hint(pt(2 * G, Y_GRID))));
    assert!(reason.contains("not on the route"), "{reason}");
    let reason = rejection(e.execute(&hint(pt(2 * G + 1, Y_CP))));
    assert_eq!(
        reason,
        "the branch point at x 15.9, y 5.3 mm on page 102 is not on the drawing grid"
    );

    let r = e
        .execute(&hint(pt(2 * G, Y_CP)))
        .expect("the hint splits the wire at its CP-exact coordinate");
    let junction = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Junction)
        .map(|k| typed::<Junction>(*k))
        .expect("a junction");
    assert_eq!(row(&e, junction).at, pt(2 * G, Y_CP));
    assert_eq!(row(&e, w).route, vec![pt(0, Y_CP), pt(2 * G, Y_CP)]);
}

// ---------------------------------------------------------------------------------------------------------
// N3: a foreign preview is re-run with the committing origin

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct HumanOnlyNoop {}

impl Command for HumanOnlyNoop {
    const NAME: &'static str = "test.human_only";

    fn meta() -> CommandMeta {
        let mut meta = CommandMeta::edit(Self::NAME);
        meta.exposure = Exposure::HumanOnly;
        meta
    }

    fn run(&self, _tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        Ok(Outcome::default())
    }
}

fn engine_with_human_only(origin: Origin) -> Engine {
    let mut registry = CommandRegistry::builtin().clone();
    registry.register::<HumanOnlyNoop>();
    Engine::new(
        golden_model(),
        EngineOptions {
            registry: Some(registry),
            ids: Some(ew_commands::testkit::seeded_ids(SEED)),
            origin,
            ..EngineOptions::default()
        },
    )
}

fn script() -> Origin {
    Origin::Script { name: "n3".into() }
}

#[test]
fn n3_a_foreign_gui_preview_committed_with_a_script_origin_is_refused() {
    let mut a = engine_with_human_only(Origin::Gui);
    let mut b = engine_with_human_only(Origin::Gui);
    let p = b
        .preview_with(&HumanOnlyNoop {}, Submit::from(Origin::Gui))
        .expect("the GUI may submit a human-only command");
    let err = a
        .commit_with(p, Submit::from(script()))
        .expect_err("the committing origin is judged, not the preview's");
    assert!(
        matches!(&err, CmdError::Exposure { origin, .. } if *origin == script()),
        "{err:?}"
    );
    assert!(a.history().entries().is_empty());
}

#[test]
fn n3_a_foreign_preview_committed_with_the_default_origin_is_judged_by_it() {
    let mut a = engine_with_human_only(script());
    let mut b = engine_with_human_only(Origin::Gui);
    let p = b
        .preview_with(&HumanOnlyNoop {}, Submit::from(Origin::Gui))
        .expect("preview");
    let err = a.commit(p).expect_err("refused by exposure");
    assert!(matches!(err, CmdError::Exposure { .. }), "{err:?}");
}

#[test]
fn n3_the_committing_origin_is_recorded_for_a_foreign_re_run() {
    let mut a = engine_with_human_only(Origin::Gui);
    let mut b = engine_with_human_only(Origin::Gui);
    let p = b
        .preview_with(&CreatePage::at(None, 1), Submit::from(Origin::Gui))
        .expect("preview");
    let committing = Origin::Mcp { client: "n3".into() };
    a.commit_with(p, Submit::from(committing.clone()))
        .expect("an edit command is open to every origin");
    let entry = a.history().entries().last().expect("a history step");
    assert_eq!(entry.origin(), &committing);
}

#[test]
fn n3_a_same_engine_preview_is_judged_by_the_committing_origin_too() {
    let mut a = engine_with_human_only(Origin::Gui);
    let p = a
        .preview_with(&HumanOnlyNoop {}, Submit::from(Origin::Gui))
        .expect("preview");
    let err = a
        .commit_with(p, Submit::from(script()))
        .expect_err("a prepared candidate does not skip the exposure check");
    assert!(matches!(err, CmdError::Exposure { .. }), "{err:?}");
    // The plain commit keeps the preview's submission.
    let p = a
        .preview_with(&HumanOnlyNoop {}, Submit::from(Origin::Gui))
        .expect("preview");
    a.commit(p).expect("the GUI preview commits from the GUI");
}
