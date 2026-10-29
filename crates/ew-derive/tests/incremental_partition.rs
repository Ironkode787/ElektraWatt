// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// A reader of ew-model's raw API (ADR-0001 amendment 2): the fixtures edit snapshots with the raw
// `Model::apply_unchecked`, so the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! The incremental potential partition against the cold one (WP-S1.07 part 2): a memoised value must equal
//! the cold value on every snapshot, whatever the edits and whatever the order the families are read in.
//!
//! - `incremental_partition_equals_cold`: a random sequence of edit batches (joins, splits, bridges, page-less
//!   connections, definitions, interruption-point renames, representation moves, rail-bonding, a second own
//!   location (cycle 33) and a page
//!   added and removed) over a fixture with several pages, interruption-point pairs, a bond with a via
//!   connection, terminals and a junction. After every batch the memoised partition, stitch graph, flow,
//!   pairing, local nets and the per-potential families equal the cold ones, the notes equal, and a second
//!   store in oracle mode (`Oracle::Every`) reports no mismatch.
//! - `joining_and_splitting_declares_only_the_touched_parts`: a wire between two potentials of one page, then
//!   its removal: the per-potential wire lists of the untouched potentials are not re-executed.
//! - `a_move_without_topology_change_re_executes_no_wire_list`: moving a representation re-executes no wire
//!   list.

mod common;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Write as _;

use common::{Fx, id, uid};
use ew_derive::Query;
use ew_derive::{
    FlowBfs, IpPairingQ, MemoStore, Oracle, PageLocalNets, PhysicalWiresOf, PotentialAttrsOf, PotentialFlow,
    PotentialId, PotentialNameOf, PotentialPartition, Q, StitchGraphQ, WirePropsOf,
};
use ew_ids::Id;
use ew_model::{AnyKey, AnyRow, Entity, Model, Op};
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, Function, Page, PotentialDef, Representation, StructureNode,
};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner, contextualize_config};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

// -------------------------------------------------------------------------------------------------------
// The fixture

const FN_PLAIN: u128 = 0x500;
const FN_IP: u128 = 0x501;
const FN_SHORT: u128 = 0x502;
const FN_PASS: u128 = 0x503;
const FN_TERM: u128 = 0x504;
const FN_PE: u128 = 0x505;
const SYM_PLAIN: u128 = 0x510;
const SYM_IP: u128 = 0x511;
const SYM_SHORT: u128 = 0x512;
const SYM_PASS: u128 = 0x513;
const SYM_TERM: u128 = 0x514;
const SYM_PE: u128 = 0x515;

const P1: u128 = 0x100;
const P2: u128 = 0x101;
const P3: u128 = 0x102;
/// A page without representations: added and removed by the edits.
const P4: u128 = 0x103;

const STRIP: u128 = 0x5000;
const LOC: u128 = 0x900;
/// A second own location: added and removed by the edits (cycle 33: an unlocated rail-bonded terminal joins the
/// only own location, and no rail once there are two).
const LOC2: u128 = 0x901;
const J1: u128 = 0x3001;

/// Every function of the fixture with its number of CPs; the representation of function `f` is `f + REP`.
const FUNCS: &[(u128, u16)] = &[
    // Plain two-CP functions.
    (0x1001, 2),
    (0x1002, 2),
    (0x1003, 2),
    (0x1004, 2),
    (0x1005, 2),
    (0x1009, 2),
    (0x100a, 2),
    (0x100b, 2),
    (0x100c, 2),
    // A function with a declared short between CPs 0 and 1.
    (0x1006, 3),
    // Two functions with a pass-through pair (a contact).
    (0x1007, 2),
    (0x1008, 2),
    // Terminals (two clamps of one pin, level 1) and rail-bonded PE terminals.
    (0x1011, 2),
    (0x1012, 2),
    (0x1013, 2),
    (0x1014, 2),
    (0x1015, 2),
    // Interruption points: A source P1, A target P2, B source P2, B target P3.
    (0x1101, 1),
    (0x1102, 1),
    (0x1103, 1),
    (0x1104, 1),
];
const REP: u128 = 0x10_000;
const IPS: [u128; 4] = [0x1101, 0x1102, 0x1103, 0x1104];
const TERMINAL_DEVICES: [u128; 5] = [0x4001, 0x4002, 0x4003, 0x4004, 0x4005];
/// Connections that edits never delete: the junction arms and the bond's via connection.
const PROTECTED: [u128; 4] = [0x2005, 0x2006, 0x2007, 0x2011];
/// The names definitions and interruption points take (they overlap on purpose).
const NAMES: [&str; 6] = ["L1", "L2", "A", "B", "N", "C"];
/// The base of the IDs the edits create.
const NEW: u128 = 0x9000;

/// An endpoint of a connection.
#[derive(Clone, Copy)]
enum Ep {
    Cp(u128, u16),
    J(u128),
}

impl Ep {
    fn json(self) -> Value {
        match self {
            Ep::Cp(f, i) => json!({"cp": [uid(f), i]}),
            Ep::J(j) => json!({"junction": uid(j)}),
        }
    }
}

fn cp(f: u128, i: u16) -> Ep {
    Ep::Cp(f, i)
}

/// A function `n` of FnDef `def` with its own device, drawn with `sym` on `page`.
fn func(fx: &mut Fx, n: u128, def: u128, sym: u128, page: u128) {
    fx.device(n + 0x20_000, "K", Some(n as u32));
    fx.function(n, Some(n + 0x20_000), def, true);
    fx.rep(n + REP, page, sym, &[n], 0, n as i64 * 1_000_000);
}

/// A terminal device `dev` of the strip with function `n` of `def`, drawn with `sym` on `page`.
fn terminal(fx: &mut Fx, dev: u128, n: u128, def: u128, sym: u128, page: u128, rail: bool) {
    let seq = dev - 0x4000;
    let mut t = json!({"strip": uid(STRIP), "seq": seq, "designation": seq.to_string()});
    if rail {
        t["rail_bonded"] = json!(true);
    }
    fx.device(dev, "X", None).with("terminal", t);
    if rail {
        fx.with("structure", json!({"location": uid(LOC)}));
    }
    fx.function(n, Some(dev), def, true);
    fx.rep(n + REP, page, sym, &[n], 0, n as i64 * 1_000_000);
}

/// An interruption point `n` named `name` drawn on `page`.
fn ip(fx: &mut Fx, n: u128, page: u128, name: &str, dir: &str) {
    fx.push(json!({
        "$k": "function", "id": uid(n), "def": uid(FN_IP),
        "kind": {"ip": {"name": name, "direction": dir}},
    }));
    fx.rep(n + REP, page, SYM_IP, &[n], 0, n as i64 * 1_000_000);
}

fn conn(fx: &mut Fx, n: u128, page: Option<u128>, a: Ep, b: Ep, kind: Value) {
    let mut r = json!({"$k": "connection", "id": uid(n), "a": a.json(), "b": b.json()});
    if let Some(p) = page {
        r["page"] = json!(uid(p));
    }
    if !kind.is_null() {
        r["kind"] = kind;
    }
    fx.push(r);
}

fn bridge() -> Value {
    json!({"bridge": {"kind": "plug_in", "group": 1, "shaft": 1}})
}

/// A FnDef `n` of `contact_kind` with `cps` and `internal` links, and its symbol `sym` with CPs on a column.
fn fndef_sym(fx: &mut Fx, n: u128, sym: u128, contact_kind: &str, cps: Value, internal: Value) {
    let count = cps.as_array().map_or(0, Vec::len);
    fx.lib_item(
        "fndef",
        n,
        json!({
            "meta": {}, "category": ["Misc", "Misc", "Fixture"], "contact_kind": contact_kind,
            "cps": cps, "internal": internal,
        }),
    );
    let pins: Vec<(u16, u16, i64, i64)> = (0..count as u16)
        .map(|i| (i, i, 0, i64::from(i) * 1_000_000))
        .collect();
    fx.symbol(sym, n, json!("origin"), &pins);
}

/// The fixture: 4 pages (the fourth empty), 21 functions, 4 interruption points (two pairs), 2 potential
/// definitions, a bond with a via connection, a junction with 3 arms, a bridge, a page-less connection, a
/// declared short, a pass-through pair, a strip with 5 terminals of which 2 are rail-bonded in one location.
fn fixture() -> Fx {
    let mut fx = Fx::new();
    fndef_sym(
        &mut fx,
        FN_PLAIN,
        SYM_PLAIN,
        "pole",
        json!([{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}]),
        json!([]),
    );
    fndef_sym(
        &mut fx,
        FN_IP,
        SYM_IP,
        "pole",
        json!([{"idx": 0, "designation": "1"}]),
        json!([]),
    );
    fndef_sym(
        &mut fx,
        FN_SHORT,
        SYM_SHORT,
        "pole",
        json!([{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}, {"idx": 2, "designation": "3"}]),
        json!([{"a": 0, "b": 1, "kind": "short"}]),
    );
    fndef_sym(
        &mut fx,
        FN_PASS,
        SYM_PASS,
        "pole",
        json!([{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}]),
        json!([{"a": 0, "b": 1, "kind": {"pass_through": {"opens": true}}}]),
    );
    fndef_sym(
        &mut fx,
        FN_TERM,
        SYM_TERM,
        "terminal",
        json!([
            {"idx": 0, "designation": "1", "level": 1, "clamp": {"clamp": "a"}},
            {"idx": 1, "designation": "1", "level": 1, "clamp": {"clamp": "b"}},
        ]),
        json!([]),
    );
    fndef_sym(
        &mut fx,
        FN_PE,
        SYM_PE,
        "pole",
        json!([
            {"idx": 0, "designation": "1"},
            {"idx": 1, "designation": "PE", "potential_role": "pe"},
        ]),
        json!([]),
    );
    fx.node(LOC, "location", "A", None);
    for (page, number) in [(P1, 1), (P2, 2), (P3, 3), (P4, 4)] {
        fx.page(page, number, None, "multi_line", &[], &format!("page {number}"));
    }
    fx.device(STRIP, "X", Some(1)).with("strip", json!({}));

    for (f, page) in [
        (0x1001, P1),
        (0x1002, P1),
        (0x1003, P2),
        (0x1004, P2),
        (0x1005, P3),
        (0x1009, P3),
        (0x100a, P3),
        (0x100b, P1),
        (0x100c, P2),
    ] {
        func(&mut fx, f, FN_PLAIN, SYM_PLAIN, page);
    }
    func(&mut fx, 0x1006, FN_SHORT, SYM_SHORT, P1);
    func(&mut fx, 0x1007, FN_PASS, SYM_PASS, P1);
    func(&mut fx, 0x1008, FN_PASS, SYM_PASS, P2);
    terminal(&mut fx, 0x4001, 0x1011, FN_TERM, SYM_TERM, P1, false);
    terminal(&mut fx, 0x4002, 0x1012, FN_TERM, SYM_TERM, P2, false);
    terminal(&mut fx, 0x4003, 0x1013, FN_TERM, SYM_TERM, P3, false);
    terminal(&mut fx, 0x4004, 0x1014, FN_PE, SYM_PE, P1, true);
    terminal(&mut fx, 0x4005, 0x1015, FN_PE, SYM_PE, P2, true);
    ip(&mut fx, 0x1101, P1, "A", "source");
    ip(&mut fx, 0x1102, P2, "A", "target");
    ip(&mut fx, 0x1103, P2, "B", "source");
    ip(&mut fx, 0x1104, P3, "B", "target");

    let w = Value::Null;
    conn(&mut fx, 0x2001, Some(P1), cp(0x1001, 0), cp(0x1006, 0), w.clone());
    conn(&mut fx, 0x2002, Some(P1), cp(0x1006, 2), cp(0x1007, 0), w.clone());
    conn(&mut fx, 0x2003, Some(P1), cp(0x1007, 1), cp(0x1101, 0), w.clone());
    conn(&mut fx, 0x2004, Some(P1), cp(0x1001, 1), cp(0x1011, 0), w.clone());
    // The junction with three arms.
    conn(&mut fx, 0x2005, Some(P1), cp(0x1002, 0), Ep::J(J1), w.clone());
    conn(&mut fx, 0x2006, Some(P1), Ep::J(J1), cp(0x1014, 1), w.clone());
    conn(&mut fx, 0x2007, Some(P1), Ep::J(J1), cp(0x1006, 1), w.clone());
    fx.push(json!({
        "$k": "junction", "id": uid(J1), "page": uid(P1), "at": [0, 0], "style": "dot",
        "sequence": [uid(0x2005), uid(0x2006), uid(0x2007)],
    }));
    conn(&mut fx, 0x2008, Some(P2), cp(0x1102, 0), cp(0x1003, 0), w.clone());
    conn(&mut fx, 0x2009, Some(P2), cp(0x1003, 1), cp(0x1008, 0), w.clone());
    conn(&mut fx, 0x200a, Some(P2), cp(0x1008, 1), cp(0x1103, 0), w.clone());
    conn(&mut fx, 0x200b, Some(P2), cp(0x1004, 0), cp(0x1012, 0), w.clone());
    conn(&mut fx, 0x200c, Some(P2), cp(0x1015, 1), cp(0x1004, 1), w.clone());
    conn(&mut fx, 0x200d, Some(P3), cp(0x1104, 0), cp(0x1005, 0), w.clone());
    conn(&mut fx, 0x200e, Some(P3), cp(0x1005, 1), cp(0x1013, 0), w.clone());
    // A page-less connection and a bridge.
    conn(&mut fx, 0x200f, None, cp(0x1012, 1), cp(0x1013, 1), w.clone());
    conn(&mut fx, 0x2010, Some(P1), cp(0x1002, 1), cp(0x1011, 1), bridge());
    // The bond's via connection with a stub on each side.
    conn(&mut fx, 0x2011, Some(P3), cp(0x1009, 0), cp(0x100a, 0), w.clone());
    conn(&mut fx, 0x2012, Some(P3), cp(0x1009, 0), cp(0x1013, 0), w.clone());
    conn(&mut fx, 0x2013, Some(P3), cp(0x100a, 0), cp(0x1005, 0), w);
    fx.push(json!({
        "$k": "bond", "id": uid(0x4101), "a": cp(0x1009, 0).json(), "b": cp(0x100a, 0).json(),
        "kind": "equipotential", "via": uid(0x2011),
    }));
    fx.push(json!({"$k": "potential_def", "id": uid(0x5001), "at": cp(0x1001, 0).json(), "name": "L1"}));
    fx.push(json!({"$k": "potential_def", "id": uid(0x5002), "at": cp(0x1003, 1).json(), "name": "L2"}));
    fx
}

// -------------------------------------------------------------------------------------------------------
// Rows and edits

fn typed<T: Entity + DeserializeOwned>(v: Value) -> AnyRow {
    AnyRow::of(serde_json::from_value::<T>(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}")))
}

/// The row `key` patched by `f` on its JSON.
fn patch<T: Entity + Serialize + DeserializeOwned>(
    model: &Model,
    key: &AnyKey,
    f: impl FnOnce(&mut Value),
) -> Option<Op> {
    let before = model.row(key)?;
    let mut v = serde_json::to_value(&**before.get::<T>()?).expect("serialises");
    f(&mut v);
    let after: T = serde_json::from_value(v).expect("parses");
    Some(Op::update(before, AnyRow::of(after)))
}

/// The page a function's representation is drawn on.
fn rep_page(model: &Model, f: u128) -> Option<Id<Page>> {
    model
        .table::<Representation>()
        .row(&id(f + REP))
        .map(|r| r.value().page)
}

fn pages(model: &Model) -> Vec<Id<Page>> {
    model.table::<Page>().ids().collect()
}

fn connections(model: &Model) -> Vec<std::sync::Arc<Connection>> {
    model
        .table::<Connection>()
        .rows()
        .map(|(_, r)| r.value().clone())
        .collect()
}

/// The CPs of the functions drawn on `page` (every CP when `None`).
fn cps_on(model: &Model, page: Option<Id<Page>>, avoid: &BTreeSet<u128>) -> Vec<(u128, u16)> {
    let mut out = Vec::new();
    for (f, n) in FUNCS {
        if avoid.contains(f) || page.is_some() && rep_page(model, *f) != page {
            continue;
        }
        out.extend((0..*n).map(|i| (*f, i)));
    }
    out
}

/// One generated edit: `kind` selects the edit, `a`, `b` and `c` choose among the candidates.
#[derive(Clone, Debug)]
struct Act {
    kind: u8,
    a: u16,
    b: u16,
    c: u16,
}

/// One generated step: a batch of edits and the seed of the order the families are read in.
#[derive(Clone, Debug)]
struct Step {
    acts: Vec<Act>,
    order: u64,
}

fn step() -> impl Strategy<Value = Step> {
    let act = (0u8..14, 0u16..64, 0u16..64, 0u16..64).prop_map(|(kind, a, b, c)| Act { kind, a, b, c });
    (vec(act, 1..=3), 0u64..=u64::MAX).prop_map(|(acts, order)| Step { acts, order })
}

/// What the ops of the current batch already use: a key is put at most once, and the batch stays consistent.
#[derive(Default)]
struct Batch {
    keys: BTreeSet<AnyKey>,
    /// Pages that a new connection or a moved representation refers to.
    used_pages: BTreeSet<Id<Page>>,
    deleted_pages: BTreeSet<Id<Page>>,
    /// Functions that a new connection ends at.
    touched: BTreeSet<u128>,
    /// Functions whose representation moves.
    moved: BTreeSet<u128>,
}

impl Batch {
    fn put(&mut self, op: Option<Op>) -> Option<Op> {
        let op = op?;
        self.keys.insert(op.key()).then_some(op)
    }
}

fn pick<T: Clone>(items: &[T], at: u16) -> Option<T> {
    (!items.is_empty()).then(|| items[usize::from(at) % items.len()].clone())
}

/// A new connection between two CPs of `pool`, on `page` when given.
fn add_connection(
    batch: &mut Batch,
    next: u128,
    pool: &[(u128, u16)],
    page: Option<Id<Page>>,
    kind: Value,
    a: u16,
    b: u16,
) -> Option<Op> {
    let ea = pick(pool, a)?;
    let eb = pick(pool, b)
        .filter(|e| *e != ea)
        .or_else(|| pick(pool, b + 1).filter(|e| *e != ea))?;
    let mut v = json!({
        "id": uid(NEW + next), "a": cp(ea.0, ea.1).json(), "b": cp(eb.0, eb.1).json(),
    });
    if let Some(p) = page {
        v["page"] = json!(uid(p.as_uuid().as_u128()));
        batch.used_pages.insert(p);
    }
    if !kind.is_null() {
        v["kind"] = kind;
    }
    batch.touched.extend([ea.0, eb.0]);
    Some(Op::insert(typed::<Connection>(v)))
}

/// The op of one edit over the snapshot `model`, or `None` when it is impossible there.
fn op_of(model: &Model, batch: &mut Batch, next: u128, act: &Act) -> Option<Op> {
    let Act { kind, a, b, c } = *act;
    let ops_page = || pick(&pages(model), a).filter(|_| true);
    let op = match kind {
        // A drawn wire between two CPs of one page: joins.
        0 => {
            let page = ops_page().filter(|p| !batch.deleted_pages.contains(p))?;
            let pool = cps_on(model, Some(page), &batch.moved);
            add_connection(batch, next, &pool, Some(page), Value::Null, b, c)
        }
        // A deletion: may split.
        1 | 3 | 5 => {
            let candidates: Vec<Id<Connection>> = connections(model)
                .iter()
                .filter(|c| !PROTECTED.contains(&c.id.as_uuid().as_u128()))
                .filter(|c| match kind {
                    3 => c.page.is_none(),
                    5 => matches!(c.kind, ConnectionKind::Bridge(_)),
                    _ => true,
                })
                .map(|c| c.id)
                .collect();
            let victim = pick(&candidates, b)?;
            model.row(&Connection::key(victim)).map(Op::delete)
        }
        // A page-less wire.
        2 => {
            let pool = cps_on(model, None, &batch.moved);
            add_connection(batch, next, &pool, None, Value::Null, b, c)
        }
        // A bridge.
        4 => {
            let page = ops_page().filter(|p| !batch.deleted_pages.contains(p))?;
            let pool = cps_on(model, Some(page), &batch.moved);
            add_connection(batch, next, &pool, Some(page), bridge(), b, c)
        }
        // A potential definition added, removed or renamed.
        6 => {
            let at = pick(&cps_on(model, None, &BTreeSet::new()), b)?;
            let name = NAMES[usize::from(c) % NAMES.len()];
            Some(Op::insert(typed::<PotentialDef>(
                json!({"id": uid(NEW + next), "at": cp(at.0, at.1).json(), "name": name}),
            )))
        }
        7 => {
            let defs: Vec<Id<PotentialDef>> = model.table::<PotentialDef>().ids().collect();
            model.row(&PotentialDef::key(pick(&defs, b)?)).map(Op::delete)
        }
        8 => {
            let defs: Vec<Id<PotentialDef>> = model.table::<PotentialDef>().ids().collect();
            let def = pick(&defs, b)?;
            let name = NAMES[usize::from(c) % NAMES.len()];
            patch::<PotentialDef>(model, &PotentialDef::key(def), |v| v["name"] = json!(name))
        }
        // An interruption point renamed: re-pairing.
        9 => {
            let f: Id<Function> = id(pick(&IPS, b)?);
            let name = NAMES[usize::from(c) % NAMES.len()];
            patch::<Function>(model, &Function::key(f), |v| {
                if let Some(spec) = v.get_mut("kind").and_then(|k| k.get_mut("ip")) {
                    spec["name"] = json!(name);
                }
            })
        }
        // A representation without connections moves to another page.
        10 => {
            let connected: BTreeSet<u128> = connections(model)
                .iter()
                .flat_map(|c| [c.a.clone(), c.b.clone()])
                .filter_map(|e| match e {
                    Endpoint::Cp(f, _) => Some(f.as_uuid().as_u128()),
                    _ => None,
                })
                .collect();
            let free: Vec<u128> = FUNCS
                .iter()
                .map(|(f, _)| *f)
                .filter(|f| !connected.contains(f) && !batch.touched.contains(f))
                .collect();
            let f = pick(&free, b)?;
            let now = rep_page(model, f)?;
            let targets: Vec<Id<Page>> = pages(model)
                .into_iter()
                .filter(|p| *p != now && !batch.deleted_pages.contains(p))
                .collect();
            let target = pick(&targets, c)?;
            let op = patch::<Representation>(model, &Representation::key(id(f + REP)), |v| {
                v["page"] = json!(uid(target.as_uuid().as_u128()));
            })?;
            batch.used_pages.insert(target);
            batch.moved.insert(f);
            Some(op)
        }
        // Rail-bonding toggled on a terminal.
        11 => {
            let dev: Id<Device> = id(pick(&TERMINAL_DEVICES, b)?);
            patch::<Device>(model, &Device::key(dev), |v| {
                if let Some(t) = v.get_mut("terminal") {
                    let on = t.get("rail_bonded").and_then(Value::as_bool).unwrap_or(false);
                    t["rail_bonded"] = json!(!on);
                }
            })
        }
        // Cycle 33: a second own location added or removed (nothing is placed in it).
        13 => {
            let loc2: Id<StructureNode> = id(LOC2);
            match model.row(&StructureNode::key(loc2)) {
                Some(row) => Some(Op::delete(row)),
                None => Some(Op::insert(typed::<StructureNode>(json!({
                    "id": uid(LOC2), "aspect": "location", "code": "B",
                })))),
            }
        }
        // The empty fourth page removed or added again.
        _ => {
            let p4: Id<Page> = id(P4);
            match model.row(&Page::key(p4)) {
                Some(row) => {
                    let used = FUNCS.iter().any(|(f, _)| rep_page(model, *f) == Some(p4))
                        || connections(model).iter().any(|c| c.page == Some(p4))
                        || batch.used_pages.contains(&p4);
                    if used {
                        return None;
                    }
                    batch.deleted_pages.insert(p4);
                    Some(Op::delete(row))
                }
                None => Some(Op::insert(typed::<Page>(json!({
                    "id": uid(P4), "folio": {"number": 4}, "kind": "multi_line", "structure": {},
                    "title": {"en": "page 4"}, "grid": {"num": 4_000_000, "den": 1},
                })))),
            }
        }
    };
    batch.put(op)
}

/// The ops of one step over `model`; `next` numbers the rows the edits create.
fn ops_of(model: &Model, step: &Step, next: &mut u128, cover: &mut Coverage) -> Vec<Op> {
    let mut batch = Batch::default();
    let mut ops = Vec::new();
    for act in &step.acts {
        *next += 1;
        if let Some(op) = op_of(model, &mut batch, *next, act) {
            cover.applied[usize::from(act.kind)] += 1;
            ops.push(op);
        }
    }
    ops
}

/// What a run of the property exercised (the property is not vacuous).
#[derive(Default, Debug)]
struct Coverage {
    /// Edits applied, per kind.
    applied: [usize; 14],
    /// Steps after which the cold partition differs from the one before.
    partition_changed: usize,
    /// Steps run.
    steps: usize,
    /// Executions of the partition by the incremental hook, and all executions of it, over the memoising
    /// stores.
    partition_incremental: u64,
    partition_executions: u64,
}

// -------------------------------------------------------------------------------------------------------
// Reading the families

/// A family read of the oracle: one query with its key.
#[derive(Clone, Copy, Debug)]
enum Fam {
    Partition,
    Stitch,
    Flow,
    Pairing,
    Name(PotentialId),
    Attrs(PotentialId),
    Wires(PotentialId),
    Props(PotentialId),
    PotFlow(PotentialId),
    Local(Id<Page>),
}

/// Reads `F(key)` on `memo` and, when given, on the cold `Q`, and compares.
fn eq<F: Query>(memo: &Q, cold: Option<&Q>, key: F::Key) -> Result<(), String> {
    let m = memo.get::<F>(key.clone());
    if let Some(cold) = cold {
        let c = cold.get::<F>(key.clone());
        if m != c {
            return Err(format!("{} {key:?}\n  memo: {m:?}\n  cold: {c:?}", F::FAMILY));
        }
    }
    Ok(())
}

fn read(fam: Fam, memo: &Q, cold: Option<&Q>) -> Result<(), String> {
    match fam {
        Fam::Partition => {
            eq::<PotentialPartition>(memo, cold, ())?;
            // The byte estimate is a running count the incremental maintenance keeps (review cycle 10,
            // finding 2); it is a function of the value, so it equals the cold value's.
            if let Some(cold) = cold {
                let (m, c) = (
                    memo.get::<PotentialPartition>(()),
                    cold.get::<PotentialPartition>(()),
                );
                let (mb, cb) = (PotentialPartition::bytes(&m), PotentialPartition::bytes(&c));
                if mb != cb {
                    return Err(format!("potential_partition bytes: memo {mb}, cold {cb}"));
                }
            }
            Ok(())
        }
        Fam::Stitch => eq::<StitchGraphQ>(memo, cold, ()),
        Fam::Flow => eq::<FlowBfs>(memo, cold, ()),
        Fam::Pairing => eq::<IpPairingQ>(memo, cold, ()),
        Fam::Name(p) => eq::<PotentialNameOf>(memo, cold, p),
        Fam::Attrs(p) => eq::<PotentialAttrsOf>(memo, cold, p),
        Fam::Wires(p) => eq::<PhysicalWiresOf>(memo, cold, p),
        Fam::Props(p) => eq::<WirePropsOf>(memo, cold, p),
        Fam::PotFlow(p) => eq::<PotentialFlow>(memo, cold, p),
        Fam::Local(page) => eq::<PageLocalNets>(memo, cold, page),
    }
}

/// The potentials of `model`, by the cold partition.
fn potential_ids(model: &Model) -> Vec<PotentialId> {
    let q = Q::new(model);
    q.get::<PotentialPartition>(())
        .potentials()
        .map(|p| p.id)
        .collect()
}

/// Every family read of `model`, in a deterministic order for `order` (the natural order when it is a
/// multiple of 4, else a shuffle: part dependencies make the order matter).
fn reads_of(model: &Model, order: u64) -> Vec<Fam> {
    let mut reads = vec![Fam::Partition, Fam::Stitch, Fam::Flow, Fam::Pairing];
    for p in potential_ids(model) {
        reads.extend([
            Fam::Name(p),
            Fam::Attrs(p),
            Fam::Wires(p),
            Fam::Props(p),
            Fam::PotFlow(p),
        ]);
    }
    reads.extend(pages(model).into_iter().map(Fam::Local));
    if order % 4 != 0 {
        // Fisher-Yates over a splitmix64 stream.
        let mut s = order;
        let mut rnd = move || {
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        };
        for i in (1..reads.len()).rev() {
            reads.swap(i, (rnd() % (i as u64 + 1)) as usize);
        }
    }
    reads
}

/// The memoised reads of `store` against a cold `Q` on `after`, then the notes, then the oracle store.
fn verify(after: &Model, store: &MemoStore, oracle: &MemoStore, order: u64) -> Result<(), TestCaseError> {
    let reads = reads_of(after, order);
    let mut memo = store.query();
    let mut cold = Q::new(after);
    for fam in &reads {
        read(*fam, &memo, Some(&cold)).map_err(|e| TestCaseError::fail(format!("{fam:?} differs: {e}")))?;
    }
    let (m, c) = (memo.diagnostics().notes(), cold.diagnostics().notes());
    if m != c {
        return Err(TestCaseError::fail(format!(
            "notes differ\n  memo: {m:?}\n  cold: {c:?}"
        )));
    }
    let mut second = oracle.query();
    // An eighth of the reads (the oracle re-runs every nested read cold: about 40 times the cost of the
    // comparison above). The others stay unread, so that later steps verify memos that missed commits.
    let skip = ((order >> 2) % 8) as usize;
    for fam in reads.iter().skip(skip).step_by(8) {
        read(*fam, &second, None).map_err(TestCaseError::fail)?;
    }
    let _ = second.diagnostics().notes();
    let mismatches = oracle.take_mismatches();
    if !mismatches.is_empty() {
        let mut text = String::new();
        for m in &mismatches {
            let _ = write!(text, "\n  {m:?}");
        }
        return Err(TestCaseError::fail(format!(
            "the oracle store found mismatches:{text}"
        )));
    }
    Ok(())
}

/// Applies a whole plan to a memoising store and an oracle store, verifying after every step.
fn run_plan(steps: &[Step], cover: &mut Coverage) -> Result<(), TestCaseError> {
    let mut model = fixture().build();
    let store = MemoStore::new(&model);
    let oracle = MemoStore::new(&model);
    oracle.set_oracle(Oracle::Every);
    // The first reads fill the memos, so that the edits have something to invalidate.
    verify(&model, &store, &oracle, 0)?;
    let mut next = 0u128;
    let mut trace = String::new();
    for (n, step) in steps.iter().enumerate() {
        let ops = ops_of(&model, step, &mut next, cover);
        cover.steps += 1;
        let _ = write!(trace, "\n  step {n}: {:?}", step.acts);
        let after = model
            .apply_unchecked(&ops, model.rev().next())
            .map_err(|e| TestCaseError::fail(format!("step {n}: ops do not apply: {e:?}")))?;
        let keys: Vec<AnyKey> = ops.iter().map(Op::key).collect();
        store.advance(&after, &keys);
        oracle.advance(&after, &keys);
        verify(&after, &store, &oracle, step.order).map_err(|e| {
            TestCaseError::fail(format!(
                "after step {n} (order {:#x}): {e}\nsteps:{trace}\nops: {ops:?}",
                step.order
            ))
        })?;
        if *Q::new(&model).get::<PotentialPartition>(()) != *Q::new(&after).get::<PotentialPartition>(()) {
            cover.partition_changed += 1;
        }
        model = after;
    }
    let stats = store.total_stats().family("potential_partition");
    cover.partition_incremental += stats.incremental;
    cover.partition_executions += stats.re_executions;
    Ok(())
}

#[test]
fn incremental_partition_equals_cold() {
    // `contextualize_config` applies the PROPTEST_* variables (the deep nightly's PROPTEST_CASES) that the
    // `proptest!` macro applies itself and a hand-built runner would otherwise ignore.
    let config = contextualize_config(Config {
        cases: 36,
        failure_persistence: None,
        ..Config::default()
    });
    let mut runner = TestRunner::new_with_rng(config, TestRng::deterministic_rng(RngAlgorithm::ChaCha));
    let cover = RefCell::new(Coverage::default());
    runner
        .run(&vec(step(), 1..12), |steps| {
            run_plan(&steps, &mut cover.borrow_mut())
        })
        .expect("the memoised partition equals the cold one");
    let cover = cover.into_inner();
    eprintln!("{cover:?}");
    assert!(
        cover.applied.iter().all(|n| *n >= 3),
        "every kind of edit is applied a few times: {cover:?}"
    );
    assert!(
        cover.partition_incremental > 0,
        "the partition is maintained by the incremental hook: {cover:?}"
    );
    assert!(
        cover.partition_changed * 3 >= cover.steps,
        "a third of the steps change the partition: {cover:?}"
    );
}

// -------------------------------------------------------------------------------------------------------
// Deterministic probes

/// The cold potentials of `model`.
fn potentials(model: &Model) -> Vec<PotentialId> {
    potential_ids(model)
}

/// Reads the wire list and the name of every potential of `model` through `q`, comparing with a cold read.
fn read_wires_and_names(model: &Model, q: &Q) {
    let cold = Q::new(model);
    for p in potentials(model) {
        eq::<PhysicalWiresOf>(q, Some(&cold), p).unwrap_or_else(|e| panic!("{e}"));
        eq::<PotentialNameOf>(q, Some(&cold), p).unwrap_or_else(|e| panic!("{e}"));
    }
}

fn apply(model: &Model, ops: &[Op]) -> Model {
    model
        .apply_unchecked(ops, model.rev().next())
        .expect("the ops apply")
}

/// How many potentials differ between two snapshots (absent in one, or with another topology).
fn changed_potentials(before: &Model, after: &Model) -> usize {
    let (qb, qa) = (Q::new(before), Q::new(after));
    let (pb, pa) = (qb.get::<PotentialPartition>(()), qa.get::<PotentialPartition>(()));
    let ids: BTreeSet<PotentialId> = pb.potentials().chain(pa.potentials()).map(|p| p.id).collect();
    ids.iter().filter(|i| pb.get(**i) != pa.get(**i)).count()
}

fn wire_list_re_executions(store: &MemoStore) -> u64 {
    store.commit_stats().family("physical_wires_of").re_executions
}

/// Joins the potentials of CPs `a` and `b` with a wire on page 2, then splits them again; after each edit
/// the wire lists re-executed are at most the potentials whose topology changed plus `slack` (the other end
/// of a bond whose end potential changed: its bond closure changed), and fewer than all.
fn join_and_split(a: (u128, u16), b: (u128, u16), slack: usize) {
    let m0 = fixture().build();
    let store = MemoStore::new(&m0);
    read_wires_and_names(&m0, &store.query());
    let at = |f: u128, i: u16| ew_model::EndpointKey::Cp(id(f), i);
    {
        let q = Q::new(&m0);
        let part = q.get::<PotentialPartition>(());
        let (pa, pb) = (part.potential_at(at(a.0, a.1)), part.potential_at(at(b.0, b.1)));
        assert!(
            pa.is_some() && pb.is_some() && pa.map(|p| p.id) != pb.map(|p| p.id),
            "two potentials"
        );
    }
    let wire = typed::<Connection>(json!({
        "id": uid(0x2100), "a": cp(a.0, a.1).json(), "b": cp(b.0, b.1).json(), "page": uid(P2),
    }));
    let insert = Op::insert(wire.clone());
    let m1 = apply(&m0, std::slice::from_ref(&insert));
    let delete = Op::delete(wire);
    let m2 = apply(&m1, std::slice::from_ref(&delete));

    for (label, before, after, op) in [("join", &m0, &m1, &insert), ("split", &m1, &m2, &delete)] {
        store.advance(after, &[op.key()]);
        read_wires_and_names(after, &store.query());
        let total = potentials(after).len();
        let bound = changed_potentials(before, after);
        let n = wire_list_re_executions(&store);
        assert!(bound >= 1, "{label}: the edit changes a potential");
        assert!(n >= 1, "{label}: a changed potential's wire list is recomputed");
        assert!(
            n as usize <= bound + slack,
            "{label}: {n} wire lists re-executed, {bound} potentials changed (+{slack})"
        );
        assert!(
            (n as usize) < total,
            "{label}: {n} wire lists re-executed of {total} potentials"
        );
    }
}

#[test]
fn joining_and_splitting_declares_only_the_touched_parts() {
    // CP 0x1003:1 and CP 0x1004:1: two potentials, neither of them an end of the bond.
    join_and_split((0x1003, 1), (0x1004, 1), 0);
    // CP 0x1003:1 and CP 0x1004:0, whose potential holds a bond end: the bond parts are per potential
    // (`PartKey::BondsOf`), so only the bond's other end is re-derived besides the changed potentials.
    join_and_split((0x1003, 1), (0x1004, 0), 1);
}

#[test]
fn a_move_without_topology_change_re_executes_no_wire_list() {
    let m0 = fixture().build();
    let store = MemoStore::new(&m0);
    read_wires_and_names(&m0, &store.query());

    // The representation of 0x1001 (two connections) moves on its page.
    let key = Representation::key(id(0x1001 + REP));
    let op = patch::<Representation>(&m0, &key, |v| v["pos"] = json!([123_456, 654_321])).expect("a rep");
    let m1 = apply(&m0, std::slice::from_ref(&op));
    store.advance(&m1, &[op.key()]);
    let q = store.query();
    read_wires_and_names(&m1, &q);
    let cold = Q::new(&m1);
    eq::<PotentialPartition>(&q, Some(&cold), ()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        wire_list_re_executions(&store),
        0,
        "no wire list is recomputed by a move"
    );
}
