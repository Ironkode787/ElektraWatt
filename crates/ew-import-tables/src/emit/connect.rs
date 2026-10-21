// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connections, junctions, conductors, potential definitions and bonds (doc 20 §7, §8, §10; doc 03 §9.2).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_schema::{
    AcDc, BevelSpec, Bond, BondKind, BridgeKind, BridgeSpec, BusLine, CircuitClass, Conductor, Connection,
    ConnectionKind, Endpoint, Junction, JunctionStyle, JunctionWiring, NumberSpec, Page, PotentialDef,
    PotentialEarthing, PotentialEnv, Record, WireOverrides, WireProps,
};
use ew_units::Pt2;

use super::parts::ROLES;
use super::{ConnInfo, Cx, PlaceError, cell_point, fresh, parse_cell, place};
use crate::ids::{record_id, source_ref};
use crate::table::{Row, Table, parse_ident};

/// The file name of `connections.csv`, the file of the findings about connections dropped with a junction.
const CONNECTIONS_FILE: &str = "connections.csv";

/// Registers the IDs of the valid `junctions.csv` rows, so that connections may end at them.
///
/// A row is valid here when its key is the first of its kind, its page is known and its cell parses. Nothing
/// is reported: [`emit_junctions`] reports every invalid row.
pub(crate) fn register_junctions(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for row in table.rows() {
        let key = row.key();
        if key.is_empty() || !seen.insert(key) || cx.junctions.contains_key(key) {
            continue;
        }
        let page_known = row.get("page").is_some_and(|p| cx.pages.contains_key(p));
        let cell_ok = row.get("cell").is_some_and(|c| parse_cell(c).is_ok());
        if page_known && cell_ok {
            cx.junctions.insert(key.to_owned(), record_id("junctions", key));
        }
    }
}

/// `conductors.csv` → `conductor`. The cable must be a device with the cable aspect (EW-INV-006).
pub(crate) fn emit_conductors(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.conductors) {
            continue;
        }
        let mut r = row.reader();
        let cable = r.req("cable");
        let core = r.req_ident("core");
        let colour = r.colour("colour");
        let csa = r.csa("csa");
        let shield = r.flag("shield");
        let pair = r.uint::<u8>("pair");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(cable), Some(core)) = (cable, core) else {
            continue;
        };
        let Some(cable_id) = cx.devices.get(cable).filter(|d| d.cable).map(|d| d.id) else {
            cx.unresolved(row, &format!("`{cable}` is not a cable device (EW-INV-006)"));
            continue;
        };
        let id = record_id::<Conductor>("conductors", row.key());
        cx.conductors.insert(row.key().to_owned(), id);
        cx.records.push(Record::Conductor(Conductor {
            id,
            v: 1,
            cable: cable_id,
            core,
            colour,
            csa,
            shield,
            pair,
            source_ref: Some(source_ref("conductors", row.key())),
            extra: Default::default(),
        }));
    }
}

/// The kind of a connection row before its references are resolved.
enum PendingKind<'t> {
    /// A wire.
    Wire,
    /// A cable core, by `conductors` key.
    Core(&'t str),
    /// A terminal bridge.
    Bridge(BridgeSpec),
    /// A bus connection, with its cable device by `devices` key (doc 10 §2.7).
    Bus(Option<&'t str>),
}

/// `connections.csv` → `connection`. The `wire` column is documentation and is not read.
pub(crate) fn emit_connections(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &cx.connections) {
            continue;
        }
        let mut r = row.reader();
        let a = r.req("a");
        let b = r.req("b");
        let page = r.text("page");
        let cable = r.text("cable");
        let kind = match r.text("kind") {
            None | Some("wire") => Some(PendingKind::Wire),
            Some("bus") => Some(PendingKind::Bus(cable)),
            Some("core") => r.req("core").map(PendingKind::Core),
            Some("bridge") => {
                let kind = r
                    .choice(
                        "bridge_kind",
                        &[
                            ("plug_in", BridgeKind::PlugIn),
                            ("saddle", BridgeKind::Saddle),
                            ("wire", BridgeKind::Wire),
                            ("inter_level", BridgeKind::InterLevel),
                        ],
                    )
                    .unwrap_or(BridgeKind::PlugIn);
                let group = r.req_uint::<u16>("bridge_group");
                let shaft = r.uint::<u8>("bridge_shaft").unwrap_or(1);
                group.map(|group| {
                    PendingKind::Bridge(BridgeSpec {
                        kind,
                        group,
                        shaft,
                        extra: Default::default(),
                    })
                })
            }
            Some(other) => {
                r.fail(format!(
                    "column `kind`: `{other}` is not supported (wire, core, bridge or bus)"
                ));
                None
            }
        };
        if cable.is_some() && !matches!(kind, Some(PendingKind::Bus(_))) {
            r.fail("column `cable` is only read for kind `bus`");
        }
        let overrides = WireOverrides {
            csa: r.csa("csa"),
            colour: r.colour("colour"),
            show_marker: r.flag("show_marker"),
            ..WireOverrides::default()
        };
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(a), Some(b), Some(kind)) = (a, b, kind) else {
            continue;
        };
        let a = match cx.endpoint(a) {
            Ok(e) => e,
            Err(m) => {
                cx.unresolved(row, &m);
                continue;
            }
        };
        let b = match cx.endpoint(b) {
            Ok(e) => e,
            Err(m) => {
                cx.unresolved(row, &m);
                continue;
            }
        };
        if a == b {
            cx.malformed(row, "both endpoints are the same (CI-01)");
            continue;
        }
        let page = match page {
            None => None,
            Some(p) => match cx.pages.get(p) {
                Some(info) => Some(info.id),
                None => {
                    cx.unresolved(row, &format!("unknown page `{p}`"));
                    continue;
                }
            },
        };
        let kind = match kind {
            PendingKind::Wire => ConnectionKind::Wire,
            PendingKind::Bridge(spec) => ConnectionKind::Bridge(spec),
            PendingKind::Bus(cable) => {
                let cable = match cable {
                    None => None,
                    Some(key) => match cx.devices.get(key).filter(|d| d.cable) {
                        Some(d) => Some(d.id),
                        None => {
                            cx.unresolved(row, &format!("`{key}` in column `cable` is not a cable device"));
                            continue;
                        }
                    },
                };
                ConnectionKind::Bus(BusLine {
                    cable,
                    extra: Default::default(),
                })
            }
            PendingKind::Core(key) => match cx.conductors.get(key) {
                Some(id) => ConnectionKind::Core(*id),
                None => {
                    cx.unresolved(row, &format!("unknown conductor `{key}`"));
                    continue;
                }
            },
        };
        let id = record_id::<Connection>("connections", row.key());
        cx.connections.insert(
            row.key().to_owned(),
            ConnInfo {
                id,
                a: a.clone(),
                b: b.clone(),
            },
        );
        cx.records.push(Record::Connection(Connection {
            id,
            v: 1,
            a,
            b,
            page,
            route: Vec::new(),
            kind,
            overrides,
            number: NumberSpec::default(),
            source_ref: Some(source_ref("connections", row.key())),
            extra: Default::default(),
        }));
    }
}

/// A `junctions.csv` row that read cleanly and awaits the sequence check (CI-04).
struct PendingJunction<'t> {
    /// The row.
    row: Row<'t>,
    /// The record ID.
    id: Id<Junction>,
    /// The page.
    page: Id<Page>,
    /// The position on the page grid.
    at: Pt2,
    /// The style.
    style: JunctionStyle,
    /// The wiring, absent for the style's default.
    wiring: Option<JunctionWiring>,
    /// The `connections` keys of the sequence.
    sequence: Vec<&'t str>,
}

/// The connections ending at each junction, built once from `cx.connections` (a connection is listed under each
/// junction it ends at), and the connections dropped with a junction so far. Lets the CI-04 check and the
/// cascade run in time linear in the connections (doc 14 §11), with one pass over the records at the end.
#[derive(Default)]
struct JunctionIndex {
    /// Junction → `connections` keys ending at it, in key order.
    ends: BTreeMap<Id<Junction>, Vec<String>>,
    /// The record IDs of the dropped connections.
    doomed: BTreeSet<Id<Connection>>,
}

impl JunctionIndex {
    fn new(cx: &Cx<'_>) -> JunctionIndex {
        let mut ends: BTreeMap<Id<Junction>, Vec<String>> = BTreeMap::new();
        for (key, c) in &cx.connections {
            let (a, b) = (junction_of(&c.a), junction_of(&c.b));
            for j in [a, b].into_iter().flatten() {
                let list = ends.entry(j).or_default();
                if list.last() != Some(key) {
                    list.push(key.clone());
                }
            }
        }
        JunctionIndex {
            ends,
            doomed: BTreeSet::new(),
        }
    }
}

fn junction_of(e: &Endpoint) -> Option<Id<Junction>> {
    match e {
        Endpoint::Junction(j) => Some(*j),
        _ => None,
    }
}

/// Drops junction `key` (`id`) from the context and every connection that ends at it, each with a finding.
/// Returns the other junctions those connections end at (they must be checked again).
fn drop_junction(cx: &mut Cx<'_>, ix: &mut JunctionIndex, key: &str, id: Id<Junction>) -> Vec<Id<Junction>> {
    cx.junctions.remove(key);
    let mut touched = Vec::new();
    for conn_key in ix.ends.get(&id).map(Vec::as_slice).unwrap_or_default() {
        let Some(conn) = cx.connections.remove(conn_key) else {
            continue;
        };
        ix.doomed.insert(conn.id);
        touched.extend(
            [junction_of(&conn.a), junction_of(&conn.b)]
                .into_iter()
                .flatten()
                .filter(|j| *j != id),
        );
        cx.findings.at(
            Code::UNRESOLVED_REFERENCE,
            Severity::Error,
            CONNECTIONS_FILE,
            Some(conn_key),
            None,
            format!("row dropped: its junction `{key}` was dropped"),
        );
    }
    touched
}

/// Why a junction row is refused: a reference that does not resolve (EW-IMP-003) or another problem (EW-IMP-002).
enum Refusal {
    /// A reference does not resolve.
    Unresolved(String),
    /// Any other problem.
    Malformed(String),
}

/// CI-04: the sequence names existing connections that end at the junction, each once, and every connection
/// ending at the junction is in the sequence.
fn check_sequence(
    cx: &Cx<'_>,
    ix: &JunctionIndex,
    key: &str,
    id: Id<Junction>,
    sequence: &[&str],
) -> Result<Vec<Id<Connection>>, Refusal> {
    let ends_here = |e: &Endpoint| matches!(e, Endpoint::Junction(j) if *j == id);
    let mut listed: BTreeSet<&str> = BTreeSet::new();
    let mut ids = Vec::with_capacity(sequence.len());
    for entry in sequence {
        let Some(conn) = cx.connections.get(*entry) else {
            return Err(Refusal::Unresolved(format!(
                "sequence names unknown connection `{entry}`"
            )));
        };
        if !ends_here(&conn.a) && !ends_here(&conn.b) {
            return Err(Refusal::Malformed(format!(
                "connection `{entry}` does not end at junction `{key}` (CI-04)"
            )));
        }
        if !listed.insert(entry) {
            return Err(Refusal::Malformed(format!(
                "connection `{entry}` is listed twice (CI-04)"
            )));
        }
        ids.push(conn.id);
    }
    for conn_key in ix.ends.get(&id).map(Vec::as_slice).unwrap_or_default() {
        if cx.connections.contains_key(conn_key) && !listed.contains(conn_key.as_str()) {
            return Err(Refusal::Malformed(format!(
                "connection `{conn_key}` ends at junction `{key}` but is missing from its sequence (CI-04)"
            )));
        }
    }
    Ok(ids)
}

/// `junctions.csv` → `junction`. A junction whose row is malformed or whose sequence is inconsistent with the
/// connections is dropped together with every connection ending at it (CI-04); the drop cascades to junctions
/// that lose a connection thereby.
pub(crate) fn emit_junctions(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut ix = JunctionIndex::new(cx);
    let mut seen = BTreeMap::<String, ()>::new();
    let mut pending: Vec<PendingJunction<'_>> = Vec::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        seen.insert(row.key().to_owned(), ());
        let mut r = row.reader();
        let page = r.req("page");
        let cell = r.req("cell").and_then(|c| match parse_cell(c) {
            Ok(c) => Some(c),
            Err(e) => {
                r.fail(e);
                None
            }
        });
        let dx = r.int("dx").unwrap_or(0);
        let dy = r.int("dy").unwrap_or(0);
        let style = r.req_choice(
            "style",
            &[
                ("dot", JunctionStyle::Dot),
                ("bevel", JunctionStyle::Bevel(BevelSpec::default())),
            ],
        );
        let wiring = r.choice(
            "wiring",
            &[("star", JunctionWiring::Star), ("chain", JunctionWiring::Chain)],
        );
        let sequence = if r.req("sequence").is_some() {
            r.list("sequence")
        } else {
            Vec::new()
        };
        let id = record_id::<Junction>("junctions", row.key());
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            drop_junction(cx, &mut ix, row.key(), id);
            continue;
        }
        let (Some(page), Some(cell), Some(style)) = (page, cell, style) else {
            continue;
        };
        let Some(page_info) = cx.pages.get(page).copied() else {
            cx.unresolved(row, &format!("unknown page `{page}`"));
            drop_junction(cx, &mut ix, row.key(), id);
            continue;
        };
        let placed = cell_point(cell, dx, dy)
            .ok_or(PlaceError::OutOfRange)
            .and_then(|t| place(page_info.grid, t, Pt2::ORIGIN));
        let at = match placed {
            Ok(at) => at,
            Err(e) => {
                cx.malformed(row, e.message());
                drop_junction(cx, &mut ix, row.key(), id);
                continue;
            }
        };
        cx.junctions.entry(row.key().to_owned()).or_insert(id);
        pending.push(PendingJunction {
            row,
            id,
            page: page_info.id,
            at,
            style,
            wiring,
            sequence,
        });
    }

    // CI-04 with a worklist: every junction is checked once, and again only when a connection ending at it was
    // dropped with another junction (the check then fails, as that connection is in its sequence or index).
    // Each check costs its sequence plus its index entry, so the cascade is linear in the connections.
    let slot: BTreeMap<Id<Junction>, usize> = pending.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let mut alive = vec![true; pending.len()];
    let mut queued = vec![true; pending.len()];
    let mut queue: VecDeque<usize> = (0..pending.len()).collect();
    let mut sequences: Vec<Vec<Id<Connection>>> = vec![Vec::new(); pending.len()];
    while let Some(i) = queue.pop_front() {
        queued[i] = false;
        if !alive[i] {
            continue;
        }
        let p = &pending[i];
        match check_sequence(cx, &ix, p.row.key(), p.id, &p.sequence) {
            Ok(ids) => sequences[i] = ids,
            Err(refusal) => {
                let (row, id) = (p.row, p.id);
                match refusal {
                    Refusal::Unresolved(m) => cx.unresolved(row, &m),
                    Refusal::Malformed(m) => cx.malformed(row, &m),
                }
                alive[i] = false;
                for j in drop_junction(cx, &mut ix, row.key(), id) {
                    if let Some(&k) = slot.get(&j)
                        && alive[k]
                        && !queued[k]
                    {
                        queued[k] = true;
                        queue.push_back(k);
                    }
                }
            }
        }
    }
    if !ix.doomed.is_empty() {
        let doomed = std::mem::take(&mut ix.doomed);
        cx.records
            .retain(|r| !matches!(r, Record::Connection(c) if doomed.contains(&c.id)));
    }

    for (i, p) in pending.into_iter().enumerate() {
        if !alive[i] {
            continue;
        }
        let sequence = std::mem::take(&mut sequences[i]);
        cx.records.push(Record::Junction(Junction {
            id: p.id,
            v: 1,
            page: p.page,
            at: p.at,
            style: p.style,
            wiring: p.wiring,
            sequence,
            source_ref: Some(source_ref("junctions", p.row.key())),
            extra: Default::default(),
        }));
    }
}

/// `potentials.csv` → `potential_def`. The potential name is the row key.
pub(crate) fn emit_potentials(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let name = match parse_ident(row.key()) {
            Ok(n) => Some(n),
            Err(e) => {
                r.fail(format!("the key: {e}"));
                None
            }
        };
        let at = r.req("at");
        let ptype = r.choice("ptype", ROLES);
        let voltage = r.dec3("voltage");
        let env = PotentialEnv {
            ac_dc: r.choice("ac_dc", &[("ac", AcDc::Ac), ("dc", AcDc::Dc)]),
            frequency: r.dec3("frequency"),
            earthing: r.choice(
                "earthing",
                &[
                    ("pelv", PotentialEarthing::Pelv),
                    ("selv", PotentialEarthing::Selv),
                    ("floating", PotentialEarthing::Floating),
                ],
            ),
            extra: Default::default(),
        };
        let circuit_class = r.choice(
            "circuit_class",
            &[
                ("mains", CircuitClass::Mains),
                ("control", CircuitClass::Control),
                ("analogue", CircuitClass::Analogue),
                ("bus", CircuitClass::Bus),
                ("pe", CircuitClass::Pe),
            ],
        );
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(name), Some(at)) = (name, at) else {
            continue;
        };
        let at = match cx.endpoint(at) {
            Ok(e) => e,
            Err(m) => {
                cx.unresolved(row, &m);
                continue;
            }
        };
        seen.insert(row.key().to_owned(), ());
        cx.records.push(Record::PotentialDef(PotentialDef {
            id: record_id::<PotentialDef>("potentials", row.key()),
            v: 1,
            at,
            name,
            ptype,
            voltage,
            env,
            circuit_class,
            props: WireProps::default(),
            source_ref: Some(source_ref("potentials", row.key())),
            extra: Default::default(),
        }));
    }
}

/// `bonds.csv` → `bond`.
pub(crate) fn emit_bonds(cx: &mut Cx<'_>, table: Option<&Table>) {
    let Some(table) = table else { return };
    let mut seen = BTreeMap::<String, ()>::new();
    for row in table.rows() {
        if !fresh(&mut cx.findings, row, &seen) {
            continue;
        }
        let mut r = row.reader();
        let a = r.req("a");
        let b = r.req("b");
        let kind = r.req_choice(
            "kind",
            &[
                ("pelv_0v_pe", BondKind::Pelv0vPe),
                ("fe_pe", BondKind::FePe),
                ("equipotential", BondKind::Equipotential),
                ("shield", BondKind::Shield),
            ],
        );
        let via = r.text("via");
        if let Err(e) = r.finish() {
            cx.malformed(row, &e);
            continue;
        }
        let (Some(a), Some(b), Some(kind)) = (a, b, kind) else {
            continue;
        };
        let a = match cx.endpoint(a) {
            Ok(e) => e,
            Err(m) => {
                cx.unresolved(row, &m);
                continue;
            }
        };
        let b = match cx.endpoint(b) {
            Ok(e) => e,
            Err(m) => {
                cx.unresolved(row, &m);
                continue;
            }
        };
        if a == b {
            cx.malformed(row, "both endpoints are the same");
            continue;
        }
        let via = match via {
            None => None,
            Some(k) => match cx.connections.get(k) {
                Some(c) => Some(c.id),
                None => {
                    cx.unresolved(row, &format!("unknown connection `{k}` in column `via`"));
                    continue;
                }
            },
        };
        seen.insert(row.key().to_owned(), ());
        cx.records.push(Record::Bond(Bond {
            id: record_id::<Bond>("bonds", row.key()),
            v: 1,
            a,
            b,
            kind,
            via,
            source_ref: Some(source_ref("bonds", row.key())),
            extra: Default::default(),
        }));
    }
}
