// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The connectivity commands: `connection.draw`, `connection.route`, `connection.delete`, `junction.edit`,
//! `ip.place`, `ip.continue`, `potential.define`, with the junction creation and clean-up normalisers.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::commands::{
    ApplyLayout, Connect, ConnectMany, ContinueIp, CreateBond, DefinePotential, DeleteConnections,
    EditJunction, EndpointParam, JunctionPlace, PlaceIp, RouteConnection, WiringChoice,
};
use ew_commands::{CmdError, Engine};
use ew_ids::Id;
use ew_model::fixture::*;
use ew_model::{Entity, Model};
use ew_schema::{
    Bond, BondKind, Connection, ConnectionKind, Endpoint, Function, FunctionKind, IpDirection, Junction,
    JunctionStyle, JunctionWiring, Kind, PotentialDef, PotentialType, Record, RepType, Representation,
};
use ew_units::Orient2;

/// Reads a record of the engine's model.
fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> T {
    (**e.model().table::<T>().get(&id).expect("record exists")).clone()
}

fn has<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> bool {
    e.model().table::<T>().get(&id).is_some()
}

fn cp_param(f: Id<Function>, idx: u16) -> EndpointParam {
    EndpointParam::Cp { function: f, idx }
}

/// The wire `W` of the spec: on page 102 from `cp(FN_102F2, 4)` to `cp(FN_102F2, 5)`, 40 mm long.
const W: Id<Connection> = id(0x4100);

fn model_with_wire() -> Model {
    let (p, mut records, _) = mini_project_parts();
    let mut w = connection(W, Some(PAGE_102), cp(FN_102F2, 4), cp(FN_102F2, 5));
    w.route = vec![pt(0, 0), pt(4 * G, 0)];
    records.push(Record::Connection(w));
    Model::from_records(p, records, library()).expect("the mini model plus a wire")
}

fn draw(a: EndpointParam, b: EndpointParam, page: Option<Id<ew_schema::Page>>) -> Connect {
    Connect {
        a,
        b,
        page,
        route: vec![],
        kind: ConnectionKind::Wire,
        computed: false,
        dot: false,
    }
}

fn on_w(x: i64) -> EndpointParam {
    EndpointParam::OnConnection {
        connection: W,
        at: pt(x * G, 0),
    }
}

fn is_rejected<T: std::fmt::Debug>(r: Result<T, CmdError>) -> bool {
    matches!(r, Err(CmdError::Rejected { .. }))
}

// ---------------------------------------------------------------------------------------------------------
// connection.draw

#[test]
fn draw_between_two_cps_creates_one_unnumbered_connection() {
    let mut e = engine(mini_model());
    let mut cmd = draw(cp_param(FN_102G1, 1), cp_param(FN_T2, 2), Some(PAGE_102));
    cmd.route = vec![pt(0, 0), pt(G, 0)];
    let r = e.execute(&cmd).expect("connection.draw");
    let key = r.outcome.primary.expect("primary");
    assert_eq!(key.0, Kind::Connection);
    assert_eq!(r.created, vec![key]);
    let c = row(&e, typed::<Connection>(key));
    assert_eq!(c.a, cp(FN_102G1, 1));
    assert_eq!(c.b, cp(FN_T2, 2));
    assert_eq!(c.page, Some(PAGE_102));
    assert_eq!(c.route, vec![pt(0, 0), pt(G, 0)]);
    assert_eq!(c.kind, ConnectionKind::Wire);
    assert_eq!(c.number, ew_schema::NumberSpec::default());
}

/// A wire to a terminal that a strip segment draws ends where the page draws the terminal's side (the derivation's
/// strip-segment rule, `ew_derive::cp_point`): a route that starts there is drawn, one that starts a grid step
/// beside it is refused like a route beside a symbol's pin.
#[test]
fn a_route_to_a_terminal_on_a_strip_segment_ends_where_the_page_draws_it() {
    let mut lib = library();
    for doc in &mut lib {
        if let ew_schema::LibDocument::Fndef(c) = doc
            && c.id == FNDEF_TERMINAL
        {
            let mut cps = std::mem::take(&mut c.item.cps).into_vec();
            for (cp, side) in cps
                .iter_mut()
                .zip([ew_schema::Side::Side1, ew_schema::Side::Side2])
            {
                cp.side = Some(side);
            }
            c.item.cps = ew_schema::KeyedList::from_vec(cps).expect("distinct CP indexes");
        }
    }
    let (p, records, _) = mini_project_parts();
    let model = Model::from_records(p, records, lib).expect("the mini project with sided terminals");
    let at =
        ew_derive::cp_point(&ew_derive::Q::new(&model), FN_T2, 0, PAGE_102).expect("the segment draws T2");
    let wire = |from: ew_units::Pt2| {
        let mut cmd = draw(cp_param(FN_T2, 0), cp_param(FN_102F2, 4), Some(PAGE_102));
        cmd.route = vec![from, pt(from.x.0, from.y.0 - G)];
        cmd
    };
    let beside = pt(at.x.0 + G, at.y.0);
    assert!(is_rejected(engine(model.clone()).execute(&wire(beside))));
    engine(model)
        .execute(&wire(at))
        .expect("a route from the drawn terminal");
}

#[test]
fn draw_without_page_is_a_page_less_bridge() {
    let mut e = engine(mini_model());
    let mut cmd = draw(cp_param(FN_T1, 1), cp_param(FN_T2, 2), None);
    cmd.kind = ConnectionKind::Wire;
    let r = e.execute(&cmd).unwrap();
    let c = row(&e, typed::<Connection>(r.outcome.primary.unwrap()));
    assert_eq!(c.page, None);
}

#[test]
fn draw_on_a_connection_creates_a_junction_and_splits() {
    let mut e = engine(model_with_wire());
    let cmd = draw(
        EndpointParam::OnConnection {
            connection: W,
            at: pt(2 * G, 0),
        },
        cp_param(FN_T2, 1),
        Some(PAGE_102),
    );
    let r = e.execute(&cmd).expect("connection.draw");
    let new: Id<Connection> = typed(r.outcome.primary.unwrap());
    let junctions: Vec<Id<Junction>> = r
        .created
        .iter()
        .filter(|k| k.0 == Kind::Junction)
        .map(|k| typed(*k))
        .collect();
    assert_eq!(junctions.len(), 1);
    let j = row(&e, junctions[0]);
    let w2 = r
        .created
        .iter()
        .filter(|k| k.0 == Kind::Connection)
        .map(|k| typed::<Connection>(*k))
        .find(|c| *c != new)
        .expect("W''");
    assert_eq!(j.page, PAGE_102);
    assert_eq!(j.at, pt(2 * G, 0));
    // A wire ended on a wire is joined in the bevel form (docs/design/06a-drafting.md §7.6): a chain.
    assert!(matches!(j.style, JunctionStyle::Bevel(_)), "{:?}", j.style);
    assert_eq!(j.wiring, None);
    assert_eq!(j.sequence, vec![W, w2, new]);
    let w1 = row(&e, W);
    assert_eq!(w1.a, cp(FN_102F2, 4));
    assert_eq!(w1.b, Endpoint::Junction(j.id));
    assert_eq!(w1.route, vec![pt(0, 0), pt(2 * G, 0)]);
    let w2 = row(&e, w2);
    assert_eq!(w2.a, Endpoint::Junction(j.id));
    assert_eq!(w2.b, cp(FN_102F2, 5));
    assert_eq!(w2.route, vec![pt(2 * G, 0), pt(4 * G, 0)]);
    let n = row(&e, new);
    assert_eq!(n.a, Endpoint::Junction(j.id));
    assert_eq!(n.b, cp(FN_T2, 1));
    assert_eq!(n.page, Some(PAGE_102));
}

#[test]
fn draw_page_defaults_to_the_split_connections_page() {
    let mut e = engine(model_with_wire());
    let r = e.execute(&draw(on_w(1), cp_param(FN_T2, 1), None)).unwrap();
    let c = row(&e, typed::<Connection>(r.outcome.primary.unwrap()));
    assert_eq!(c.page, Some(PAGE_102));
}

#[test]
fn draw_between_two_connections_makes_two_junctions() {
    let (p, mut records, _) = mini_project_parts();
    let w = id(0x4100);
    let v: Id<Connection> = id(0x4101);
    let mut wc = connection(w, Some(PAGE_102), cp(FN_102F2, 4), cp(FN_102F2, 5));
    wc.route = vec![pt(0, 0), pt(4 * G, 0)];
    let mut vc = connection(v, Some(PAGE_102), cp(FN_T1, 1), cp(FN_T1, 2));
    vc.route = vec![pt(0, 5 * G), pt(4 * G, 5 * G)];
    records.push(Record::Connection(wc));
    records.push(Record::Connection(vc));
    let model = Model::from_records(p, records, library()).unwrap();
    let before = lines(&model);
    let mut e = engine(model);
    let cmd = draw(
        EndpointParam::OnConnection {
            connection: w,
            at: pt(G, 0),
        },
        EndpointParam::OnConnection {
            connection: v,
            at: pt(3 * G, 5 * G),
        },
        None,
    );
    let r = e.execute(&cmd).unwrap();
    let new = row(&e, typed::<Connection>(r.outcome.primary.unwrap()));
    let (Endpoint::Junction(ja), Endpoint::Junction(jb)) = (new.a.clone(), new.b.clone()) else {
        panic!("both ends are junctions");
    };
    assert_ne!(ja, jb);
    assert_eq!(row(&e, ja).at, pt(G, 0));
    assert_eq!(row(&e, jb).at, pt(3 * G, 5 * G));
    assert_eq!(row(&e, ja).sequence.len(), 3);
    assert_eq!(row(&e, jb).sequence.len(), 3);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn draw_at_an_existing_junction_appends_to_its_sequence() {
    let mut e = engine(mini_model());
    let cmd = draw(
        EndpointParam::Junction { junction: J5 },
        cp_param(FN_T1, 1),
        Some(PAGE_102),
    );
    let r = e.execute(&cmd).unwrap();
    let new: Id<Connection> = typed(r.outcome.primary.unwrap());
    assert_eq!(
        row(&e, J5).sequence,
        vec![CON_J5_IN, CON_W22, CON_W27, new],
        "the new connection joins the end of the sequence"
    );
    assert_eq!(row(&e, new).a, at_junction(J5));
}

#[test]
fn draw_rejects_the_same_endpoint_at_both_ends_and_a_page_less_connection() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    let r = e.execute(&draw(cp_param(FN_T1, 1), cp_param(FN_T1, 1), None));
    assert!(is_rejected(r));
    let r = e.execute(&draw(
        EndpointParam::Junction { junction: J5 },
        EndpointParam::Junction { junction: J5 },
        None,
    ));
    assert!(is_rejected(r));
    let cmd = draw(
        EndpointParam::OnConnection {
            connection: CON_BR1,
            at: pt(0, 0),
        },
        cp_param(FN_T2, 1),
        None,
    );
    assert!(is_rejected(e.execute(&cmd)));
    assert_eq!(lines(e.model()), before);
}

#[test]
fn draw_rejects_both_ends_on_the_same_connection() {
    let mut e = engine(model_with_wire());
    let r = e.execute(&draw(on_w(1), on_w(3), None));
    assert!(is_rejected(r));
}

#[test]
fn draw_names_a_missing_record() {
    let mut e = engine(mini_model());
    let ghost: Id<Function> = id(0xdead);
    let r = e.execute(&draw(cp_param(ghost, 0), cp_param(FN_T2, 1), None));
    assert!(matches!(r, Err(CmdError::NotFound { key }) if key == (Kind::Function, ghost.cast())));
    let ghost_c: Id<Connection> = id(0xdeae);
    let r = e.execute(&draw(
        EndpointParam::OnConnection {
            connection: ghost_c,
            at: pt(0, 0),
        },
        cp_param(FN_T2, 1),
        None,
    ));
    assert!(matches!(r, Err(CmdError::NotFound { .. })));
}

// ---------------------------------------------------------------------------------------------------------
// connection.draw_many

fn bridge(a: Id<Function>, b: Id<Function>) -> Connect {
    let mut c = draw(cp_param(a, 1), cp_param(b, 1), None);
    c.kind = ConnectionKind::Bridge(ew_schema::BridgeSpec {
        kind: ew_schema::BridgeKind::PlugIn,
        group: 7,
        shaft: 1,
        extra: ew_schema::Extra::new(),
    });
    c
}

#[test]
fn draw_many_draws_every_entry_in_one_undo_step() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    let cmd = ConnectMany {
        connections: vec![bridge(FN_T1, FN_T2), bridge(FN_T2, FN_T3)],
    };
    let r = e.execute(&cmd).expect("connection.draw_many");
    let first = row(&e, typed::<Connection>(r.outcome.primary.expect("primary")));
    assert_eq!((first.a.clone(), first.b.clone()), (cp(FN_T1, 1), cp(FN_T2, 1)));
    let new: Vec<Connection> = e
        .model()
        .table::<Connection>()
        .iter()
        .filter(|(_, c)| matches!(&c.kind, ConnectionKind::Bridge(s) if s.group == 7))
        .map(|(_, c)| (**c).clone())
        .collect();
    assert_eq!(new.len(), 2, "both bridges are drawn");
    assert!(new.iter().all(|c| c.page.is_none()), "page-less bridges");
    assert!(new.iter().any(|c| c.a == cp(FN_T2, 1) && c.b == cp(FN_T3, 1)));
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before, "one undo step takes back both");
}

#[test]
fn draw_many_refuses_no_entry_and_a_bad_entry_whole() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    let r = e.execute(&ConnectMany { connections: vec![] });
    assert!(
        matches!(&r, Err(CmdError::InvalidParams { path, .. }) if path == "/connections"),
        "{r:?}"
    );
    // The second entry has the same endpoint at both ends: nothing of the first is kept.
    let r = e.execute(&ConnectMany {
        connections: vec![bridge(FN_T1, FN_T2), bridge(FN_T3, FN_T3)],
    });
    assert!(
        matches!(&r, Err(CmdError::Rejected { source, text, .. })
            if source == "connection.draw_many" && text.to_string().starts_with("entry 1: ")),
        "{r:?}"
    );
    assert_eq!(lines(e.model()), before);
}

/// The cost is interactive up to 500 entries and Bulk above (`BULK_ABOVE`); more than
/// 10,000 entries (`DRAW_MANY_MAX`) are refused at `/connections` and nothing is drawn.
#[test]
fn draw_many_cost_and_entry_limit() {
    use ew_commands::{Command, Cost};
    let mut e = engine(mini_model());
    let many = |n: usize| ConnectMany {
        connections: vec![bridge(FN_T1, FN_T2); n],
    };
    assert_eq!(many(500).cost(e.model()), Cost::Interactive);
    assert_eq!(many(501).cost(e.model()), Cost::Bulk);
    assert_eq!(many(10_000).cost(e.model()), Cost::Bulk);
    let before = lines(e.model());
    let r = e.preview(&many(10_001));
    assert!(
        matches!(&r, Err(CmdError::InvalidParams { path, text, .. })
            if path == "/connections" && text.to_string().contains("10001 connections are more than the 10000")),
        "{r:?}"
    );
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// connection.route

#[test]
fn route_replaces_the_route_only_and_refuses_a_missing_connection() {
    let mut e = engine(mini_model());
    let before = row(&e, CON_W29);
    let route = vec![pt(0, 0), pt(0, G), pt(G, G)];
    let r = e
        .execute(&RouteConnection {
            connection: CON_W29,
            route: route.clone(),
        })
        .unwrap();
    assert_eq!(r.outcome.primary, Some((Kind::Connection, CON_W29.cast())));
    let after = row(&e, CON_W29);
    assert_eq!(after.route, route);
    assert_eq!(
        Connection {
            route: vec![],
            ..after
        },
        before
    );
    let ghost: Id<Connection> = id(0xdead);
    let r = e.execute(&RouteConnection {
        connection: ghost,
        route: vec![],
    });
    assert!(
        matches!(r, Err(CmdError::NotFound { .. })),
        "a missing connection"
    );
}

// ---------------------------------------------------------------------------------------------------------
// layout.apply and the form of a new junction

/// The wire W split by a wire from T2:1 ending on it at two steps, routed down: the junction and the new wire.
fn split_w(e: &mut Engine, dot: bool) -> (Id<Junction>, Id<Connection>) {
    let mut cmd = draw(on_w(2), cp_param(FN_T2, 1), Some(PAGE_102));
    cmd.dot = dot;
    let r = e.execute(&cmd).expect("connection.draw");
    let j = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Junction)
        .map(|k| typed::<Junction>(*k))
        .expect("a junction");
    (j, typed(r.outcome.primary.expect("the new wire")))
}

#[test]
fn a_wire_ended_on_a_wire_makes_a_bevel_and_with_the_dot_asked_for_a_dot() {
    let mut e = engine(model_with_wire());
    let (j, _) = split_w(&mut e, false);
    assert!(matches!(row(&e, j).style, JunctionStyle::Bevel(_)));
    let mut e = engine(model_with_wire());
    let (j, _) = split_w(&mut e, true);
    assert_eq!(row(&e, j).style, JunctionStyle::Dot);
}

#[test]
fn layout_apply_stores_a_plan_in_one_step_marks_its_routes_and_refuses_a_junction_left_without_its_routes() {
    let mut e = engine(model_with_wire());
    let w = row(&e, W);
    let route = vec![pt(0, 0), pt(0, G), pt(4 * G, G), pt(4 * G, 0)];
    let r = e
        .execute(&ApplyLayout {
            page: PAGE_102,
            parts: vec![],
            junctions: vec![],
            routes: vec![RouteConnection {
                connection: W,
                route: route.clone(),
            }],
            markers: vec![],
        })
        .expect("layout.apply");
    assert_eq!(r.outcome.primary, Some((Kind::Connection, W.cast())));
    let after = row(&e, W);
    assert_eq!(after.route, route);
    assert!(after.computed, "the program's route");
    // The route editor's route is the person's again.
    e.execute(&RouteConnection {
        connection: W,
        route: w.route.clone(),
    })
    .unwrap();
    assert!(!row(&e, W).computed);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(row(&e, W), w, "one undo step each");
    // Nothing to apply.
    let r = e.execute(&ApplyLayout {
        page: PAGE_102,
        parts: vec![],
        junctions: vec![],
        routes: vec![],
        markers: vec![],
    });
    assert!(matches!(&r, Err(CmdError::InvalidParams { .. })), "{r:?}");
    // A junction that moves without the route of a routed wire of it.
    let (j, _) = split_w(&mut e, false);
    let at = row(&e, j).at;
    let r = e.execute(&ApplyLayout {
        page: PAGE_102,
        parts: vec![],
        junctions: vec![JunctionPlace {
            junction: j,
            at: pt(at.x.0 + G, at.y.0),
            orient: None,
        }],
        routes: vec![],
        markers: vec![],
    });
    assert!(is_rejected(r));
    assert_eq!(row(&e, j).at, at);
}

#[test]
fn layout_apply_keeps_a_junction_s_form_and_sequence() {
    // A plan has no member that changes a wire's ends, a junction's form, its wires or its sequence: a dot stays a dot
    // whatever way it is told to point.
    let mut e = engine(model_with_wire());
    let (j, new) = split_w(&mut e, true);
    let before = row(&e, j);
    let ends = |e: &Engine| {
        before
            .sequence
            .iter()
            .map(|c| {
                let c = row(e, *c);
                (c.a, c.b)
            })
            .collect::<Vec<_>>()
    };
    let was = ends(&e);
    let n = row(&e, new);
    let to = n.route.last().copied();
    let r = e.execute(&ApplyLayout {
        page: PAGE_102,
        parts: vec![],
        junctions: vec![JunctionPlace {
            junction: j,
            at: before.at,
            orient: Some(Orient2::R90),
        }],
        routes: to
            .map(|_| RouteConnection {
                connection: new,
                route: n.route.clone(),
            })
            .into_iter()
            .collect(),
        markers: vec![],
    });
    assert!(r.is_ok(), "{r:?}");
    let after = row(&e, j);
    assert_eq!(after.style, JunctionStyle::Dot);
    assert_eq!(after.sequence, before.sequence);
    assert_eq!(ends(&e), was);
}

// ---------------------------------------------------------------------------------------------------------
// connection.delete

#[test]
fn delete_removes_the_connection() {
    let mut e = engine(mini_model());
    e.execute(&DeleteConnections {
        connections: vec![CON_W29],
    })
    .unwrap();
    assert!(!has(&e, CON_W29));
    assert!(has(&e, CON_W11A));
}

#[test]
fn delete_of_a_third_member_dissolves_the_junction() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    e.execute(&DeleteConnections {
        connections: vec![CON_W27],
    })
    .expect("connection.delete");
    assert!(!has(&e, J5), "a junction with two members dissolves");
    assert!(!has(&e, CON_W27));
    assert!(!has(&e, CON_W22), "the second member is merged into the first");
    let kept = row(&e, CON_J5_IN);
    assert_eq!(kept.a, cp(FN_102G1, 5));
    assert_eq!(kept.b, cp(FN_102F2, 2));
    let bond: Bond = row(&e, BOND_PE);
    assert_eq!(bond.via, None);
    assert_eq!(bond.kind, BondKind::Pelv0vPe);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

/// A junction that cannot dissolve because its two remaining wires carry different numbers is named by
/// its position and page, never by its ID.
#[test]
fn a_junction_that_cannot_merge_two_numbers_is_named_in_words() {
    let (p, mut records, _) = mini_project_parts();
    for r in &mut records {
        if let Record::Connection(c) = r {
            if c.id == CON_J5_IN {
                c.number = ew_schema::NumberSpec::Frozen(ident("101"));
            } else if c.id == CON_W22 {
                c.number = ew_schema::NumberSpec::Frozen(ident("102"));
            }
        }
        if let Record::Junction(j) = r {
            if j.id == J5 {
                j.at = ew_units::Pt2::new(ew_units::Nm(120_000_000), ew_units::Nm(45_500_000));
            }
        }
    }
    let mut e = engine(Model::from_records(p, records, library()).expect("valid"));
    match e.execute(&DeleteConnections {
        connections: vec![CON_W27],
    }) {
        Err(CmdError::Rejected { text, .. }) => assert_eq!(
            text.to_string(),
            "the junction at x 120, y 45.5 mm on page 102 would merge a wire numbered \"101\" and a wire \
             numbered \"102\", which carry different numbers; clear one of the numbers or delete the \
             wires separately"
        ),
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// The refusal of deleting `gone` at J5 (moved to x 120, y 45.5 mm) when `first` and `second` carry the assigned
/// counters 3 and 4.
fn assigned_junction_refusal(first: Id<Connection>, second: Id<Connection>, gone: Id<Connection>) -> String {
    let (p, mut records, _) = mini_project_parts();
    // without the PE bond that W27 realises, W27's wire is numbered like any other
    records.retain(|r| !matches!(r, Record::Bond(b) if b.via == Some(CON_W27)));
    let assigned = |counter: u32| {
        serde_json::from_value::<ew_schema::NumberSpec>(serde_json::json!({"assigned": {"counter": counter}}))
            .expect("an assigned number")
    };
    for r in &mut records {
        if let Record::Connection(c) = r {
            if c.id == first {
                c.number = assigned(3);
            } else if c.id == second {
                c.number = assigned(4);
            }
        }
        if let Record::Junction(j) = r {
            if j.id == J5 {
                j.at = ew_units::Pt2::new(ew_units::Nm(120_000_000), ew_units::Nm(45_500_000));
            }
        }
    }
    let mut e = engine(Model::from_records(p, records, library()).expect("valid"));
    match e.execute(&DeleteConnections {
        connections: vec![gone],
    }) {
        Err(CmdError::Rejected { text, .. }) => text.to_string(),
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// The junction refusal names the printed number, the counter only where the printed labels
/// coincide: the two members of one physical wire print the same number, so the refusal adds their counters;
/// two wires that print different numbers are named by the printed numbers alone.
#[test]
fn a_junction_that_cannot_merge_two_assigned_numbers_names_the_printed_numbers() {
    assert_eq!(
        assigned_junction_refusal(CON_J5_IN, CON_W22, CON_W27),
        "the junction at x 120, y 45.5 mm on page 102 would merge a wire numbered \"102.1\" with assigned number 3 \
         and a wire numbered \"102.1\" with assigned number 4, which carry different numbers; clear one of the numbers \
         or delete the wires separately",
        "the same printed number"
    );
    assert_eq!(
        assigned_junction_refusal(CON_W22, CON_W27, CON_J5_IN),
        "the junction at x 120, y 45.5 mm on page 102 would merge a wire numbered \"102.1\" and a wire numbered \
         \"102.3\", which carry different numbers; clear one of the numbers or delete the wires separately",
        "different printed numbers"
    );
}

#[test]
fn delete_of_two_members_removes_the_junction_with_the_last() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    e.execute(&DeleteConnections {
        connections: vec![CON_W27, CON_W22],
    })
    .expect("connection.delete");
    assert!(!has(&e, J5));
    assert!(!has(&e, CON_W27));
    assert!(!has(&e, CON_W22));
    assert!(!has(&e, CON_J5_IN), "the last member goes with its junction");
    assert_eq!(row(&e, BOND_PE).via, None);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn delete_rejects_an_empty_list_and_names_a_missing_connection() {
    let mut e = engine(mini_model());
    assert!(is_rejected(e.execute(&DeleteConnections { connections: vec![] })));
    let before = lines(e.model());
    let ghost: Id<Connection> = id(0xdead);
    let r = e.execute(&DeleteConnections {
        connections: vec![CON_W29, ghost],
    });
    assert!(matches!(r, Err(CmdError::NotFound { key }) if key == (Kind::Connection, ghost.cast())));
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// junction.edit

fn edit(junction: Id<Junction>) -> EditJunction {
    EditJunction {
        junction,
        style: None,
        wiring: None,
        sequence: None,
    }
}

#[test]
fn edit_junction_sets_wiring_style_and_sequence() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    let seq = vec![CON_W27, CON_J5_IN, CON_W22];
    let bevel = JunctionStyle::Bevel(Default::default());
    let r = e
        .execute(&EditJunction {
            junction: J5,
            style: Some(bevel.clone()),
            wiring: Some(WiringChoice::Chain),
            sequence: Some(seq.clone()),
        })
        .unwrap();
    assert_eq!(r.outcome.primary, Some((Kind::Junction, J5.cast())));
    let j = row(&e, J5);
    assert_eq!(j.style, bevel);
    assert_eq!(j.wiring, Some(JunctionWiring::Chain));
    assert_eq!(j.sequence, seq);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn edit_junction_wiring_choices() {
    let mut e = engine(mini_model());
    let mut star = edit(J5);
    star.wiring = Some(WiringChoice::StyleDefault);
    e.execute(&star).unwrap();
    assert_eq!(row(&e, J5).wiring, None, "the style default stores nothing");
    let mut star = edit(J5);
    star.wiring = Some(WiringChoice::Star);
    e.execute(&star).unwrap();
    assert_eq!(row(&e, J5).wiring, Some(JunctionWiring::Star));
}

#[test]
fn edit_junction_rejects_a_sequence_that_is_not_a_permutation_and_a_missing_junction() {
    let mut e = engine(mini_model());
    for seq in [
        vec![CON_J5_IN, CON_W22],
        vec![CON_J5_IN, CON_W22, CON_W22],
        vec![CON_J5_IN, CON_W22, CON_W27, CON_W29],
        vec![CON_J5_IN, CON_W22, CON_W29],
    ] {
        let mut cmd = edit(J5);
        cmd.sequence = Some(seq);
        assert!(is_rejected(e.execute(&cmd)));
    }
    let ghost: Id<Junction> = id(0xdead);
    assert!(matches!(e.execute(&edit(ghost)), Err(CmdError::NotFound { .. })));
}

// ---------------------------------------------------------------------------------------------------------
// ip.place / ip.continue

fn place(page: Id<ew_schema::Page>, name: &str, direction: IpDirection) -> PlaceIp {
    PlaceIp {
        page,
        symbol: SYM,
        fndef: Some(FNDEF_IP),
        pos: pt(3 * G, 2 * G),
        orient: Orient2::default(),
        name: ident(name),
        direction,
        star_source: false,
        sort: None,
        external: None,
        copies: Vec::new(),
    }
}

#[test]
fn place_ip_creates_the_function_and_its_representation() {
    let mut e = engine(mini_model());
    let mut cmd = place(PAGE_101, "L-2", IpDirection::Source);
    cmd.star_source = true;
    cmd.sort = Some(4);
    let r = e.execute(&cmd).expect("ip.place");
    let key = r.outcome.primary.unwrap();
    assert_eq!(key.0, Kind::Function);
    let f = row(&e, typed::<Function>(key));
    assert_eq!(f.device, None);
    assert_eq!(f.def, FNDEF_IP);
    assert_eq!(f.role, ew_schema::FnRole::Main);
    let FunctionKind::Ip(spec) = f.kind else {
        panic!("an IP function");
    };
    assert_eq!(spec.name, ident("L-2"));
    assert_eq!(spec.direction, IpDirection::Source);
    assert!(spec.star_source);
    assert_eq!(spec.sort, Some(4));
    assert_eq!(spec.external, None);
    let reps: Vec<Representation> = e
        .model()
        .table::<Representation>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .filter(|r| r.functions == vec![f.id])
        .collect();
    assert_eq!(reps.len(), 1);
    assert_eq!(reps[0].page, PAGE_101);
    assert_eq!(reps[0].symbol, SYM);
    assert_eq!(reps[0].pos, pt(3 * G, 2 * G));
    assert_eq!(reps[0].rep_type, RepType::MultiLine);
    assert_eq!(r.created.len(), 2);
}

#[test]
fn place_ip_without_fndef_resolves_the_symbols_def() {
    let mut e = engine(mini_model());
    let mut cmd = place(PAGE_101, "L-2", IpDirection::Source);
    cmd.symbol = SYM_TWO;
    cmd.fndef = None;
    let r = e.execute(&cmd).expect("ip.place");
    let f = row(&e, typed::<Function>(r.outcome.primary.unwrap()));
    assert_eq!(f.def, fndef_two());
}

#[test]
fn place_ip_rejects_an_unresolvable_def_and_missing_records() {
    let mut e = engine(mini_model());
    let mut cmd = place(PAGE_101, "L-2", IpDirection::Source);
    cmd.fndef = None;
    assert!(
        matches!(e.execute(&cmd), Err(CmdError::NotFound { .. })),
        "the fixture symbol's FnDef reference does not resolve"
    );
    let ghost_page: Id<ew_schema::Page> = id(0xdead);
    assert!(matches!(
        e.execute(&place(ghost_page, "L-2", IpDirection::Source)),
        Err(CmdError::NotFound { .. })
    ));
    let mut cmd = place(PAGE_101, "L-2", IpDirection::Source);
    cmd.symbol = id(0xdeae);
    assert!(matches!(e.execute(&cmd), Err(CmdError::NotFound { .. })));
    let mut cmd = place(PAGE_101, "L-2", IpDirection::Source);
    cmd.external = Some(id(0xdeaf));
    assert!(matches!(e.execute(&cmd), Err(CmdError::NotFound { .. })));
}

#[test]
fn an_ip_pair_is_placed_and_continued() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    let mut cmd = place(PAGE_101, "L-2", IpDirection::Source);
    cmd.orient = Orient2::default();
    let first: Id<Function> = typed(e.execute(&cmd).unwrap().outcome.primary.unwrap());
    let mut original_rep = e
        .model()
        .table::<Representation>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .find(|r| r.functions == vec![first])
        .unwrap();
    let r = e
        .execute(&ContinueIp {
            ip: first,
            page: PAGE_102,
            pos: None,
        })
        .expect("ip.continue");
    let partner: Id<Function> = typed(r.outcome.primary.unwrap());
    let f = row(&e, partner);
    let FunctionKind::Ip(spec) = f.kind else {
        panic!("an IP function");
    };
    assert_eq!(spec.name, ident("L-2"));
    assert_eq!(spec.direction, IpDirection::Target);
    assert!(!spec.star_source);
    assert_eq!(spec.external, None);
    assert_eq!(f.def, FNDEF_IP);
    let rep = e
        .model()
        .table::<Representation>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .find(|r| r.functions == vec![partner])
        .unwrap();
    assert_eq!(rep.page, PAGE_102);
    assert_eq!(rep.symbol, SYM);
    assert_eq!(rep.pos, original_rep.pos, "default position: the original's");
    original_rep.page = PAGE_102;
    original_rep.functions = vec![partner];
    original_rep.id = rep.id;
    assert_eq!(rep, original_rep);
    // Undo twice restores the mini model's bytes.
    e.undo(&ew_commands::Origin::Gui).unwrap();
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn continue_ip_reverses_the_direction_and_takes_a_position() {
    let mut e = engine(mini_model());
    let r = e
        .execute(&ContinueIp {
            ip: FN_IP_102,
            page: PAGE_101,
            pos: Some(pt(G, G)),
        })
        .unwrap();
    let partner: Id<Function> = typed(r.outcome.primary.unwrap());
    let FunctionKind::Ip(spec) = row(&e, partner).kind else {
        panic!("an IP function");
    };
    assert_eq!(spec.direction, IpDirection::Source);
    let rep = e
        .model()
        .table::<Representation>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .find(|r| r.functions == vec![partner])
        .unwrap();
    assert_eq!(rep.pos, pt(G, G));
    // An undirected point stays undirected.
    let p = typed::<Function>(
        e.execute(&place(PAGE_101, "L-9", IpDirection::Undirected))
            .unwrap()
            .outcome
            .primary
            .unwrap(),
    );
    let q = typed::<Function>(
        e.execute(&ContinueIp {
            ip: p,
            page: PAGE_102,
            pos: None,
        })
        .unwrap()
        .outcome
        .primary
        .unwrap(),
    );
    let FunctionKind::Ip(spec) = row(&e, q).kind else {
        panic!("an IP function");
    };
    assert_eq!(spec.direction, IpDirection::Undirected);
}

#[test]
fn continue_ip_rejections() {
    let mut e = engine(mini_model());
    let before = lines(e.model());
    // The original's own page.
    assert!(is_rejected(e.execute(&ContinueIp {
        ip: FN_IP_101,
        page: PAGE_101,
        pos: None,
    })));
    // Not an interruption point.
    assert!(is_rejected(e.execute(&ContinueIp {
        ip: FN_102G1,
        page: PAGE_101,
        pos: None,
    })));
    // Missing function or page.
    let ghost: Id<Function> = id(0xdead);
    assert!(matches!(
        e.execute(&ContinueIp {
            ip: ghost,
            page: PAGE_101,
            pos: None
        }),
        Err(CmdError::NotFound { .. })
    ));
    let ghost_page: Id<ew_schema::Page> = id(0xdeae);
    assert!(matches!(
        e.execute(&ContinueIp {
            ip: FN_IP_101,
            page: ghost_page,
            pos: None
        }),
        Err(CmdError::NotFound { .. })
    ));
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// potential.define

fn define(at: EndpointParam, name: &str) -> DefinePotential {
    DefinePotential {
        at,
        name: ident(name),
        ptype: Some(PotentialType::L1),
        voltage: Some(dec3("230")),
        env: Default::default(),
        circuit_class: None,
        props: Default::default(),
    }
}

#[test]
fn define_potential_at_a_cp_and_at_a_junction() {
    let mut e = engine(mini_model());
    let r = e
        .execute(&define(cp_param(FN_102G1, 4), "L1"))
        .expect("potential.define");
    let key = r.outcome.primary.unwrap();
    assert_eq!(key.0, Kind::PotentialDef);
    let d = row(&e, typed::<PotentialDef>(key));
    assert_eq!(d.at, cp(FN_102G1, 4));
    assert_eq!(d.name, ident("L1"));
    assert_eq!(d.ptype, Some(PotentialType::L1));
    assert_eq!(d.voltage, Some(dec3("230")));
    assert_eq!(r.created, vec![key]);
    let r = e
        .execute(&define(EndpointParam::Junction { junction: J5 }, "N"))
        .unwrap();
    let d = row(&e, typed::<PotentialDef>(r.outcome.primary.unwrap()));
    assert_eq!(d.at, at_junction(J5));
}

/// A point that defines a potential already refuses a second definition, naming the
/// first; nothing is written.
#[test]
fn define_potential_twice_at_one_point_is_refused_naming_the_first() {
    let mut e = engine(mini_model());
    e.execute(&define(cp_param(FN_102G1, 4), "L1"))
        .expect("potential.define");
    let before = lines(e.model());
    match e.execute(&define(cp_param(FN_102G1, 4), "L2")) {
        Err(CmdError::Rejected { text, .. }) => assert_eq!(
            text.to_string(),
            "this point already defines the potential L1; change that definition instead"
        ),
        other => panic!("expected a rejection, got {other:?}"),
    }
    assert_eq!(lines(e.model()), before);
}

#[test]
fn define_potential_on_a_connection_is_rejected() {
    let mut e = engine(model_with_wire());
    let before = lines(e.model());
    assert!(is_rejected(e.execute(&define(on_w(1), "L1"))));
    assert_eq!(lines(e.model()), before);
}

#[test]
fn define_potential_names_a_missing_record() {
    let mut e = engine(mini_model());
    let ghost: Id<Function> = id(0xdead);
    assert!(matches!(
        e.execute(&define(cp_param(ghost, 0), "L1")),
        Err(CmdError::NotFound { .. })
    ));
    let ghost_j: Id<Junction> = id(0xdeae);
    assert!(matches!(
        e.execute(&define(EndpointParam::Junction { junction: ghost_j }, "L1")),
        Err(CmdError::NotFound { .. })
    ));
}

// ---------------------------------------------------------------------------------------------------------
// A wire's own values follow the wire (doc 13 §2.3)

/// `-102F2:4`, `-102F2:5` and `-T2:1`: the three clamps of the T and star tests.
fn clamps3() -> [Endpoint; 3] {
    [cp(FN_102F2, 4), cp(FN_102F2, 5), cp(FN_T2, 1)]
}

/// What a person reads of a wire that nobody gave a value of its own.
const BARE: WireSeen = (None, None, [None, None]);

/// The JSON form of a colour as [`wire_seen`] gives it.
fn colour(code: &str) -> Option<String> {
    Some(format!("[\"{code}\"]"))
}

/// A wire drawn onto W, with the bevel a wire ending on a wire makes and with a dot: W keeps its typed length and both
/// its ends set by hand at their clamps, the new wire takes none of them, and when the new wire goes, W has them still.
#[test]
fn a_wire_drawn_onto_a_wire_leaves_the_wire_its_values_with_a_bevel_and_with_a_dot() {
    let [x, y, t] = clamps3();
    for dot in [false, true] {
        let model = with_wire_values(
            &model_with_wire(),
            (&x, &y),
            400,
            Some("BU"),
            [Some("bare"), Some("ring_lug")],
        );
        let mut e = engine(model);
        let (_, new) = split_w(&mut e, dot);
        let w = (
            Some(400),
            colour("BU"),
            [Some("bare".to_owned()), Some("ring_lug".to_owned())],
        );
        assert_eq!(wire_seen(e.model(), &x, &y), Some(w.clone()), "dot {dot}");
        // The new wire runs from -T2:1 to -102F2:5 under the bevel's chain, to -102F2:4 under the dot's star.
        let other = if dot { &x } else { &y };
        assert_eq!(wire_seen(e.model(), other, &t), Some(BARE), "dot {dot}");
        e.execute(&DeleteConnections {
            connections: vec![new],
        })
        .expect("the branch deleted");
        assert_eq!(
            wire_seen(e.model(), &x, &y),
            Some(w),
            "dot {dot}: the T dissolved"
        );
    }
}

/// The T of the dissolve tests: `-102F2:4` and `-102F2:5` through a junction of `style`, members drawn towards the
/// junction or away from it (`towards`), and a branch to `-T2:1`; the wire 4–5 with its own values, the branch's wire
/// with a length and an end of its own.
fn t_model(style: JunctionStyle, towards: [bool; 2]) -> (Model, [Id<Connection>; 3]) {
    let (p, mut records, _) = mini_project_parts();
    let j: Id<Junction> = id(0x4200);
    let ids: [Id<Connection>; 3] = [id(0x4101), id(0x4102), id(0x4103)];
    let [x, y, t] = clamps3();
    for (k, (end, towards)) in [(x.clone(), towards[0]), (y.clone(), towards[1])]
        .into_iter()
        .enumerate()
    {
        let c = if towards {
            connection(ids[k], Some(PAGE_102), end, Endpoint::Junction(j))
        } else {
            connection(ids[k], Some(PAGE_102), Endpoint::Junction(j), end)
        };
        records.push(Record::Connection(c));
    }
    records.push(Record::Connection(connection(
        ids[2],
        Some(PAGE_102),
        Endpoint::Junction(j),
        t.clone(),
    )));
    let dot = style == JunctionStyle::Dot;
    let mut jr = junction(j, PAGE_102, ids.to_vec());
    jr.style = style;
    jr.wiring = None;
    records.push(Record::Junction(jr));
    let model = Model::from_records(p, records, library()).expect("a T");
    let model = with_wire_values(
        &model,
        (&x, &y),
        250,
        Some("BU"),
        [Some("ring_lug"), Some("bare")],
    );
    // The branch's wire runs from the feed arm's clamp in a star, from the second member's in a chain.
    let branch = if dot { &x } else { &y };
    let model = with_wire_values(&model, (branch, &t), 300, None, [None, Some("bare")]);
    (model, ids)
}

/// A T that loses its branch dissolves into one connection, the wire 4–5, which keeps its own values, each end at its
/// clamp, whichever way its two members are drawn and whether the T is a star or a chain; nothing of the branch's wire
/// stays with it.
#[test]
fn a_t_that_dissolves_keeps_the_wire_s_values_however_it_is_drawn() {
    let [x, y, t] = clamps3();
    for style in [JunctionStyle::Dot, JunctionStyle::Bevel(Default::default())] {
        for towards in [[true, true], [true, false], [false, true], [false, false]] {
            let (model, ids) = t_model(style.clone(), towards);
            let mut e = engine(model);
            e.execute(&DeleteConnections {
                connections: vec![ids[2]],
            })
            .expect("the branch deleted");
            assert_eq!(
                wire_seen(e.model(), &x, &t),
                None,
                "{style:?} {towards:?}: no wire to -T2:1"
            );
            assert_eq!(
                wire_seen(e.model(), &y, &t),
                None,
                "{style:?} {towards:?}: no wire to -T2:1"
            );
            assert_eq!(
                wire_seen(e.model(), &x, &y),
                Some((
                    Some(250),
                    colour("BU"),
                    [Some("ring_lug".into()), Some("bare".into())]
                )),
                "{style:?} {towards:?}"
            );
        }
    }
}

/// A wire deleted from a star: the junction dissolves into the feed arm, which becomes the other wire's own, and that
/// wire keeps its values on it; the deleted wire's values are gone with it.
#[test]
fn a_wire_deleted_from_a_star_leaves_the_other_wire_its_values_on_the_feed_arm() {
    let [x, _, t] = clamps3();
    let (model, ids) = t_model(JunctionStyle::Dot, [true, false]);
    let mut e = engine(model);
    e.execute(&DeleteConnections {
        connections: vec![ids[1]],
    })
    .expect("a branch deleted");
    assert_eq!(
        wire_seen(e.model(), &x, &t),
        Some((Some(300), None, [None, Some("bare".into())]))
    );
    assert_eq!(
        row(&e, ids[0]).overrides.length,
        Some(ew_units::Nm(300 * MM)),
        "on the feed arm"
    );
}

/// A junction whose wiring changes from a star to a chain and back: the wire whose two clamps stay keeps its values;
/// the branch's wire now runs from another clamp, so it keeps its end at -T2:1 and nothing that does not divide (no
/// typed length).
#[test]
fn a_junction_s_wiring_changed_keeps_each_wire_s_values() {
    let [x, y, t] = clamps3();
    let (model, _) = t_model(JunctionStyle::Dot, [true, false]);
    let j: Id<Junction> = id(0x4200);
    let mut e = engine(model);
    let four_five = Some((
        Some(250),
        colour("BU"),
        [Some("ring_lug".to_owned()), Some("bare".to_owned())],
    ));
    let mut chain = edit(j);
    chain.wiring = Some(WiringChoice::Chain);
    e.execute(&chain).expect("a chain");
    assert_eq!(wire_seen(e.model(), &x, &y), four_five);
    assert_eq!(
        wire_seen(e.model(), &y, &t),
        Some((None, None, [None, Some("bare".into())]))
    );
    let mut star = edit(j);
    star.wiring = Some(WiringChoice::Star);
    e.execute(&star).expect("a star again");
    assert_eq!(wire_seen(e.model(), &x, &y), four_five);
    assert_eq!(
        wire_seen(e.model(), &x, &t),
        Some((None, None, [None, Some("bare".into())]))
    );
}

/// A wire left open beyond an interruption point keeps its end set by hand at its clamp (doc 13 §2.3): -101F1:1 runs
/// through the pair L-1 to a chain at -102F2:4 and -102F2:5; the wire on page 101 deleted, the wire to -102F2:4 starts
/// open at the point, made of the arm from the pair's other point and the chain's arm, neither of which lands on
/// -102F2:4 next to the point; it keeps its colour and the ring lug set by hand there, and loses its typed length.
#[test]
fn a_wire_left_open_beyond_an_interruption_point_keeps_its_end_at_its_clamp() {
    let (p, mut records, _) = mini_project_parts();
    let j: Id<Junction> = id(0x4300);
    let arms: [Id<Connection>; 3] = [id(0x4301), id(0x4302), id(0x4303)];
    records.retain(|r| !matches!(r, Record::Connection(c) if c.id == CON_W11B));
    records.push(Record::Connection(connection(
        arms[0],
        Some(PAGE_102),
        cp(FN_IP_102, 0),
        Endpoint::Junction(j),
    )));
    records.push(Record::Connection(connection(
        arms[1],
        Some(PAGE_102),
        Endpoint::Junction(j),
        cp(FN_102F2, 4),
    )));
    records.push(Record::Connection(connection(
        arms[2],
        Some(PAGE_102),
        Endpoint::Junction(j),
        cp(FN_102F2, 5),
    )));
    let mut chain = junction(j, PAGE_102, arms.to_vec());
    chain.style = JunctionStyle::Bevel(Default::default());
    chain.wiring = Some(JunctionWiring::Chain);
    records.push(Record::Junction(chain));
    let model = Model::from_records(p, records, library()).expect("a wire through a pair to a chain");
    let (feed, clamp) = (cp(FN_101F1, 1), cp(FN_102F2, 4));
    let model = with_wire_values(
        &model,
        (&feed, &clamp),
        400,
        Some("BU"),
        [Some("bare"), Some("ring_lug")],
    );
    let mut e = engine(model);
    e.execute(&DeleteConnections {
        connections: vec![CON_W11A],
    })
    .expect("the wire on page 101 deleted");
    assert_eq!(
        wire_seen(e.model(), &cp(FN_IP_101, 0), &clamp),
        Some((None, colour("BU"), [None, Some("ring_lug".into())]))
    );
}

/// The page 103 of the wire through two pairs of interruption points.
const PAGE_103: Id<ew_schema::Page> = id(0x4400);
/// The pair L-2: its source on page 102, its target on page 103.
const FN_L2_SRC: Id<Function> = id(0x4401);
const FN_L2_TGT: Id<Function> = id(0x4402);
/// The wire's constituent on page 102 between the pairs, and its last on page 103.
const CON_MID: Id<Connection> = id(0x4403);
const CON_LAST: Id<Connection> = id(0x4404);

/// One wire through two pairs of interruption points: -101F1:1 on page 101 to L-1, from L-1's partner on page 102 to
/// L-2 (the middle constituent, drawn from L-1's side, or from L-2's with `reversed`), from L-2's partner on page 103
/// to -102F2:4.
fn wire_through_two_pairs(reversed: bool) -> Model {
    let (p, mut records, lib) = mini_project_parts();
    records.retain(|r| !matches!(r, Record::Connection(c) if c.id == CON_W11B));
    let (a, b) = (cp(FN_IP_102, 0), cp(FN_L2_SRC, 0));
    let (a, b) = if reversed { (b, a) } else { (a, b) };
    records.extend([
        Record::Page(page(PAGE_103, Some(SEC1), 103, "Control 2")),
        Record::Function(ip_function(FN_L2_SRC, "L-2", IpDirection::Source)),
        Record::Function(ip_function(FN_L2_TGT, "L-2", IpDirection::Target)),
        Record::Representation(rep(id(0x4405), PAGE_102, vec![FN_L2_SRC])),
        Record::Representation(rep(id(0x4406), PAGE_103, vec![FN_L2_TGT])),
        Record::Connection(connection(CON_MID, Some(PAGE_102), a, b)),
        Record::Connection(connection(
            CON_LAST,
            Some(PAGE_103),
            cp(FN_L2_TGT, 0),
            cp(FN_102F2, 4),
        )),
    ]);
    Model::from_records(p, records, lib).expect("a wire through two pairs")
}

/// The middle constituent of a wire between two pairs of interruption points lands on neither end, and neither
/// neighbour shares an endpoint with it: its ends are read from the order the path passes the points (the rule of
/// `end_through` the mend `ebf311c46` added). Through its endpoint on L-1's side it leads to the end at -101F1:1, through
/// its endpoint on L-2's side to the end at -102F2:4, whichever way round it is drawn.
#[test]
fn the_middle_of_a_wire_between_two_pairs_leads_to_the_end_on_its_side() {
    for reversed in [false, true] {
        let model = wire_through_two_pairs(reversed);
        let q = ew_derive::Q::new(&model);
        let records = q.get::<ew_derive::WireRecordSet>(());
        let r = records
            .iter()
            .find(|r| r.wire.constituents.contains(&CON_MID))
            .expect("one wire through both pairs");
        let k = r
            .wire
            .constituents
            .iter()
            .position(|c| *c == CON_MID)
            .expect("the middle");
        let feed = (0..2)
            .find(|i| {
                r.wire.ends[*i].cp
                    == ew_derive::CpRef {
                        function: FN_101F1,
                        idx: 1,
                    }
            })
            .expect("an end at -101F1:1");
        let clamp = 1 - feed;
        let (l1_side, l2_side) = (!reversed, reversed);
        assert_eq!(
            ew_derive::end_through(&q, &r.wire, k, l1_side),
            Some(feed),
            "reversed {reversed}"
        );
        assert_eq!(
            ew_derive::end_through(&q, &r.wire, k, l2_side),
            Some(clamp),
            "reversed {reversed}"
        );
    }
}

/// The ends set by hand at both clamps of a wire through two pairs of interruption points stay when its middle, on
/// page 102 between the pairs, is drawn again (doc 13 §2.3).
#[test]
fn a_wire_through_two_pairs_keeps_its_ends_when_its_middle_is_drawn_again() {
    let (feed, clamp) = (cp(FN_101F1, 1), cp(FN_102F2, 4));
    let model = with_wire_values(
        &wire_through_two_pairs(false),
        (&feed, &clamp),
        400,
        Some("BU"),
        [Some("bare"), Some("ring_lug")],
    );
    let ends = || Some([Some("bare".to_owned()), Some("ring_lug".to_owned())]);
    assert_eq!(wire_seen(&model, &feed, &clamp).map(|w| w.2), ends());
    let mut e = engine(model);
    e.execute(&DeleteConnections {
        connections: vec![CON_MID],
    })
    .expect("the middle deleted");
    e.execute(&draw(
        cp_param(FN_IP_102, 0),
        cp_param(FN_L2_SRC, 0),
        Some(PAGE_102),
    ))
    .expect("the middle drawn again");
    assert_eq!(wire_seen(e.model(), &feed, &clamp).map(|w| w.2), ends());
}

/// A bond whose `via` is a connection of a chain changes the wiring as a deleted connection does (doc 13 §2.3): the via
/// leaves the chain's wires, and the wire the chain joins from -102F2:4 to -T2:1 instead keeps what divides of the two
/// it is made from and each end set by hand at its clamp, but no typed length.
#[test]
fn a_bond_through_a_chains_member_rewires_it_and_the_wires_keep_their_values() {
    let [x, _, t] = clamps3();
    let (model, ids) = t_model(JunctionStyle::Bevel(Default::default()), [true, false]);
    let mut e = engine(model);
    e.execute(&CreateBond {
        a: cp_param(FN_102F2, 4),
        b: cp_param(FN_T1, 1),
        kind: serde_json::from_value(serde_json::json!("shield")).expect("a bond kind"),
        via: Some(ids[1]),
    })
    .expect("the bond");
    assert_eq!(
        wire_seen(e.model(), &x, &t),
        Some((None, colour("BU"), [Some("ring_lug".into()), Some("bare".into())]))
    );
}
