// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.16 part 1, P1-1 (decision D1; doc 08 §3.5 step 2b): a potential with no definition and no source point
//! that is entered through a pass-through pair is named after its supply, `<root>/<tag>:<designation>`, before
//! the fallback `<tag>:<designation>` of its first CP.
//!
//! The fixture: an external supply point on L (defined), fuse -F1 (pass-through 1→2), switch -S51 (1→2) and a
//! lamp; and, apart, two functions on a wire with no upstream. Tests:
//!
//! - `supply_names_follow_the_pass_through_chain`: the potential between -F1 and -S51 is `L/1F1:2`, the one after
//!   -S51 `L/1S51:2` (the root is L, not `L/1F1:2`; the page-based tags of folio 1 are `-1F1` and `-1S51`, printed
//!   without the aspect sign), the potential with no upstream keeps the fallback `-1K7:2`.
//! - `supply_names_do_not_depend_on_ids_or_runs`: twice-run equal, and a copy with permuted IDs names alike.

mod common;

use common::{Fx, id, uid};
use ew_derive::{NameSource, PotentialNameOf, PotentialPartition, Q};
use serde_json::{Value, json};

const FNDEF: u128 = 0x500;
const FNDEF_IP: u128 = 0x501;
const FNDEF_PASS: u128 = 0x502;
const SYM: u128 = 0x510;
const SYM_IP: u128 = 0x511;
const SYM_PASS: u128 = 0x512;
const P1: u128 = 0x100;

/// The IDs of one copy of the fixture: (supply point, -F1, -S51, lamp, lone A, lone B) functions and the five
/// connections, so that a second copy can permute them.
struct Ids {
    ip: u128,
    fuse: u128,
    switch: u128,
    lamp: u128,
    lone: (u128, u128),
    wires: [u128; 4],
    def: u128,
}

const FIRST: Ids = Ids {
    ip: 0x1101,
    fuse: 0x1001,
    switch: 0x1002,
    lamp: 0x1003,
    lone: (0x1004, 0x1005),
    wires: [0x2001, 0x2002, 0x2003, 0x2004],
    def: 0x5001,
};

/// The same drawing with the IDs in another order (a new project mints random IDs, ID-677).
const PERMUTED: Ids = Ids {
    ip: 0x1001,
    fuse: 0x1005,
    switch: 0x1101,
    lamp: 0x1004,
    lone: (0x1003, 0x1002),
    wires: [0x2004, 0x2003, 0x2001, 0x2002],
    def: 0x5009,
};

fn cp(f: u128, idx: u16) -> Value {
    json!({"cp": [uid(f), idx]})
}

fn wire(fx: &mut Fx, n: u128, a: Value, b: Value) {
    fx.push(json!({"$k": "connection", "id": uid(n), "page": uid(P1), "a": a, "b": b}));
}

/// A function `n` of `def` drawn with `sym` on P1 in drawing row `row` (not from its ID, so a copy with other
/// IDs draws alike), on its own device with tag letter and counter.
fn func(fx: &mut Fx, n: u128, (def, sym): (u128, u128), (letter, counter): (&str, u32), row: i64) {
    fx.device(n + 0x20_000, letter, Some(counter));
    fx.function(n, Some(n + 0x20_000), def, true);
    fx.rep(n + 0x10_000, P1, sym, &[n], 0, row * 8_000_000);
}

fn fixture(ids: &Ids) -> Fx {
    let mut fx = Fx::new();
    fx.fndef(FNDEF, ["Misc", "Misc", "Plain"], &[(0, "1"), (1, "2")]);
    fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)]);
    fx.fndef(FNDEF_IP, ["Misc", "Misc", "Ip"], &[(0, "1")]);
    fx.symbol(SYM_IP, FNDEF_IP, json!("origin"), &[(0, 0, 0, 0)]);
    fx.lib_item(
        "fndef",
        FNDEF_PASS,
        json!({
            "meta": {}, "category": ["Misc", "Misc", "Pass"], "contact_kind": "pole",
            "cps": [{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}],
            "internal": [{"a": 0, "b": 1, "kind": {"pass_through": {"opens": true}}}],
        }),
    );
    fx.symbol(
        SYM_PASS,
        FNDEF_PASS,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)],
    );
    fx.page(P1, 1, None, "multi_line", &[], "one");
    // The external supply point roots the flow.
    fx.node(0x900, "location", "J", None);
    fx.push(json!({
        "$k": "external_ref", "id": uid(0x910), "location": uid(0x900), "device": "JE-N1", "connection": "L",
    }));
    fx.push(json!({
        "$k": "function", "id": uid(ids.ip), "def": uid(FNDEF_IP),
        "kind": {"ip": {"name": "SUPPLY", "direction": "source", "external": uid(0x910)}},
    }));
    fx.rep(ids.ip + 0x10_000, P1, SYM_IP, &[ids.ip], 0, 0);
    let pass = (FNDEF_PASS, SYM_PASS);
    let plain = (FNDEF, SYM);
    func(&mut fx, ids.fuse, pass, ("F", 1), 1);
    func(&mut fx, ids.switch, pass, ("S", 51), 2);
    func(&mut fx, ids.lamp, plain, ("E", 1), 3);
    func(&mut fx, ids.lone.0, plain, ("K", 7), 4);
    func(&mut fx, ids.lone.1, plain, ("K", 8), 5);
    wire(&mut fx, ids.wires[0], cp(ids.ip, 0), cp(ids.fuse, 0));
    wire(&mut fx, ids.wires[1], cp(ids.fuse, 1), cp(ids.switch, 0));
    wire(&mut fx, ids.wires[2], cp(ids.switch, 1), cp(ids.lamp, 0));
    wire(&mut fx, ids.wires[3], cp(ids.lone.0, 1), cp(ids.lone.1, 0));
    fx.push(json!({"$k": "potential_def", "id": uid(ids.def), "at": cp(ids.fuse, 0), "name": "L"}));
    fx
}

/// Every potential's (name, source kind), sorted by name.
fn names(q: &Q) -> Vec<(String, &'static str)> {
    let part = q.get::<PotentialPartition>(());
    let mut out: Vec<(String, &'static str)> = part
        .potentials()
        .map(|p| {
            let n = q.get::<PotentialNameOf>(p.id);
            let kind = match n.name.source {
                NameSource::Def(_) => "def",
                NameSource::SourceIp(_) => "source",
                NameSource::Supply { .. } => "supply",
                NameSource::Fallback(_) => "fallback",
            };
            (n.name.text.clone(), kind)
        })
        .collect();
    out.sort();
    out
}

/// The names: the page-based scheme tags the devices `-1F1`, `-1S51`, `-1K7`; the supply name drops the leading
/// aspect sign, the fallback keeps it.
fn want() -> Vec<(String, &'static str)> {
    vec![
        ("-1K7:2".to_owned(), "fallback"),
        ("L".to_owned(), "def"),
        ("L/1F1:2".to_owned(), "supply"),
        ("L/1S51:2".to_owned(), "supply"),
    ]
}

#[test]
fn supply_names_follow_the_pass_through_chain() {
    let m = fixture(&FIRST).build();
    let q = Q::new(&m);
    assert_eq!(names(&q), want());
    // The supply source names the function of the pair.
    let part = q.get::<PotentialPartition>(());
    let after_switch = part
        .potentials()
        .map(|p| q.get::<PotentialNameOf>(p.id))
        .find(|n| n.name.text == "L/1S51:2")
        .expect("L/1S51:2");
    assert_eq!(
        after_switch.name.source,
        NameSource::Supply {
            via: id(FIRST.switch)
        }
    );
}

#[test]
fn supply_names_do_not_depend_on_ids_or_runs() {
    let m = fixture(&FIRST).build();
    let first = names(&Q::new(&m));
    assert_eq!(names(&Q::new(&m)), first, "twice-run equal");
    let permuted = fixture(&PERMUTED).build();
    assert_eq!(names(&Q::new(&permuted)), first, "permuted IDs");
}
