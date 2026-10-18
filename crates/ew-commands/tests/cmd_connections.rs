// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The connectivity commands: `connection.draw`, `connection.route`, `connection.delete`, `junction.edit`,
//! `ip.place`, `ip.continue`, `potential.define`, with the junction creation and clean-up normalisers.

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    Connect, ContinueIp, DefinePotential, DeleteConnections, EditJunction, EndpointParam, PlaceIp,
    RouteConnection, WiringChoice,
};
use ew_commands::{CmdError, Engine};
use ew_ids::Id;
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
    let (p, mut records, _) = mini_golden_parts();
    let mut w = connection(W, Some(PAGE_102), cp(FN_102F2, 4), cp(FN_102F2, 5));
    w.route = vec![pt(0, 0), pt(4 * G, 0)];
    records.push(Record::Connection(w));
    Model::from_records(p, records, library()).expect("golden plus wire")
}

fn draw(a: EndpointParam, b: EndpointParam, page: Option<Id<ew_schema::Page>>) -> Connect {
    Connect {
        a,
        b,
        page,
        route: vec![],
        kind: ConnectionKind::Wire,
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
    let mut e = engine(golden_model());
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

#[test]
fn draw_without_page_is_a_page_less_bridge() {
    let mut e = engine(golden_model());
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
    assert_eq!(j.style, JunctionStyle::Dot);
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
    let (p, mut records, _) = mini_golden_parts();
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
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn draw_at_an_existing_junction_appends_to_its_sequence() {
    let mut e = engine(golden_model());
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
fn draw_rejects_the_same_endpoint_at_both_ends() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let r = e.execute(&draw(cp_param(FN_T1, 1), cp_param(FN_T1, 1), None));
    assert!(is_rejected(r));
    let r = e.execute(&draw(
        EndpointParam::Junction { junction: J5 },
        EndpointParam::Junction { junction: J5 },
        None,
    ));
    assert!(is_rejected(r));
    assert_eq!(lines(e.model()), before);
}

#[test]
fn draw_rejects_both_ends_on_the_same_connection() {
    let mut e = engine(model_with_wire());
    let r = e.execute(&draw(on_w(1), on_w(3), None));
    assert!(is_rejected(r));
}

#[test]
fn draw_on_a_page_less_connection_is_rejected() {
    let mut e = engine(golden_model());
    let cmd = draw(
        EndpointParam::OnConnection {
            connection: CON_BR1,
            at: pt(0, 0),
        },
        cp_param(FN_T2, 1),
        None,
    );
    assert!(is_rejected(e.execute(&cmd)));
}

#[test]
fn draw_names_a_missing_record() {
    let mut e = engine(golden_model());
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

#[test]
fn draw_undo_restores_bytes() {
    let mut e = engine(model_with_wire());
    let before = lines(e.model());
    e.execute(&draw(on_w(2), cp_param(FN_T2, 1), None)).unwrap();
    assert_ne!(lines(e.model()), before);
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// connection.route

#[test]
fn route_replaces_the_route_only() {
    let mut e = engine(golden_model());
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
}

#[test]
fn route_of_a_missing_connection_is_not_found() {
    let mut e = engine(golden_model());
    let ghost: Id<Connection> = id(0xdead);
    let r = e.execute(&RouteConnection {
        connection: ghost,
        route: vec![],
    });
    assert!(matches!(r, Err(CmdError::NotFound { .. })));
}

#[test]
fn route_undo_restores_bytes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&RouteConnection {
        connection: CON_W29,
        route: vec![pt(0, 0), pt(G, 0)],
    })
    .unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// connection.delete

#[test]
fn delete_removes_the_connection() {
    let mut e = engine(golden_model());
    e.execute(&DeleteConnections {
        connections: vec![CON_W29],
    })
    .unwrap();
    assert!(!has(&e, CON_W29));
    assert!(has(&e, CON_W11A));
}

#[test]
fn delete_of_a_third_member_dissolves_the_junction() {
    let mut e = engine(golden_model());
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
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn delete_of_two_members_removes_the_junction_with_the_last() {
    let mut e = engine(golden_model());
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
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn delete_rejects_an_empty_list() {
    let mut e = engine(golden_model());
    assert!(is_rejected(e.execute(&DeleteConnections { connections: vec![] })));
}

#[test]
fn delete_names_a_missing_connection() {
    let mut e = engine(golden_model());
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
    let mut e = engine(golden_model());
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
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn edit_junction_wiring_choices() {
    let mut e = engine(golden_model());
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
fn edit_junction_rejects_a_sequence_that_is_not_a_permutation() {
    let mut e = engine(golden_model());
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
}

#[test]
fn edit_missing_junction_is_not_found() {
    let mut e = engine(golden_model());
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
    }
}

#[test]
fn place_ip_creates_the_function_and_its_representation() {
    let mut e = engine(golden_model());
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
    let mut e = engine(golden_model());
    let mut cmd = place(PAGE_101, "L-2", IpDirection::Source);
    cmd.symbol = SYM_TWO;
    cmd.fndef = None;
    let r = e.execute(&cmd).expect("ip.place");
    let f = row(&e, typed::<Function>(r.outcome.primary.unwrap()));
    assert_eq!(f.def, fndef_two());
}

#[test]
fn place_ip_rejects_an_unresolvable_def_and_missing_records() {
    let mut e = engine(golden_model());
    let mut cmd = place(PAGE_101, "L-2", IpDirection::Source);
    cmd.fndef = None;
    assert!(
        matches!(e.execute(&cmd), Err(CmdError::NotFound { .. })),
        "the golden symbol's FnDef reference does not resolve"
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
fn place_ip_undo_restores_bytes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&place(PAGE_101, "L-2", IpDirection::Undirected))
        .unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn an_ip_pair_is_placed_and_continued() {
    let mut e = engine(golden_model());
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
    // Undo twice restores the golden bytes.
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn continue_ip_reverses_the_direction_and_takes_a_position() {
    let mut e = engine(golden_model());
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
    let mut e = engine(golden_model());
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

#[test]
fn continue_ip_undo_restores_bytes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    // FN_IP_101 sits on page 101; page 102 gets another partner.
    e.execute(&ContinueIp {
        ip: FN_IP_101,
        page: PAGE_102,
        pos: None,
    })
    .unwrap();
    e.undo().unwrap();
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
fn define_potential_at_a_cp() {
    let mut e = engine(golden_model());
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
}

#[test]
fn define_potential_at_a_junction() {
    let mut e = engine(golden_model());
    let r = e
        .execute(&define(EndpointParam::Junction { junction: J5 }, "N"))
        .unwrap();
    let d = row(&e, typed::<PotentialDef>(r.outcome.primary.unwrap()));
    assert_eq!(d.at, at_junction(J5));
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
    let mut e = engine(golden_model());
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

#[test]
fn define_potential_undo_restores_bytes() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&define(cp_param(FN_102G1, 4), "L1")).unwrap();
    e.undo().unwrap();
    assert_eq!(lines(e.model()), before);
}
