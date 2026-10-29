// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawing fixture builder: JSON records plus library documents (function definitions and symbols), as
//! the connectivity tests of `ew-derive` write them.

#![allow(dead_code)]

use ew_derive::ew_schema::{LibDocument, Record};
use ew_model::Model;
use serde_json::{Value, json};

use super::{project, record, us};

/// The model of JSON `records` and library documents `lib` under a project with `settings`.
pub fn model_json(settings: Value, records: &[Value], lib: &[Value]) -> Model {
    let records: Vec<Record> = records.iter().cloned().map(record).collect();
    let lib: Vec<LibDocument> = lib
        .iter()
        .map(|d| serde_json::from_value(d.clone()).unwrap_or_else(|e| panic!("lib document {d}: {e}")))
        .collect();
    Model::from_records(project(settings), records, lib).expect("the model passes the integrity check")
}

/// The library of the fixtures' library copies.
const LIBRARY: u128 = 0x7000;

/// A drawing fixture under construction.
#[derive(Clone, Debug, Default)]
pub struct Fx {
    /// The project settings (`Value::Null`: none).
    pub settings: Value,
    /// The records, each with `$k`.
    pub records: Vec<Value>,
    /// The library documents.
    pub lib: Vec<Value>,
}

/// An endpoint of a connection.
#[derive(Clone, Copy, Debug)]
pub enum Ep {
    /// A connection point: function and CP index.
    Cp(u128, u16),
    /// A junction.
    J(u128),
}

impl Ep {
    fn json(self) -> Value {
        match self {
            Ep::Cp(f, i) => json!({"cp": [us(f), i]}),
            Ep::J(j) => json!({"junction": us(j)}),
        }
    }
}

/// CP 0 of function `f`.
pub fn cp(f: u128) -> Ep {
    Ep::Cp(f, 0)
}

/// The two-CP function definition.
pub const FNDEF: u128 = 0x500;
/// The interruption-point function definition.
pub const FNDEF_IP: u128 = 0x501;
/// The pass-through function definition (CPs 0 and 1, an opening pass-through between them).
pub const FNDEF_PASS: u128 = 0x502;
/// The function definition with a PE-role CP 1.
pub const FNDEF_PE: u128 = 0x503;
/// The interruption-point definition without a connection point.
pub const FNDEF_IP0: u128 = 0x504;
/// The function definition with an M-role (0 V) CP 1.
pub const FNDEF_M: u128 = 0x505;
const SYM_M: u128 = 0x515;
const SYM: u128 = 0x510;
const SYM_IP: u128 = 0x511;
const SYM_PASS: u128 = 0x512;
const SYM_PE: u128 = 0x513;
const SYM_IP0: u128 = 0x514;
/// The first page.
pub const P1: u128 = 0x100;
/// The second page.
pub const P2: u128 = 0x101;

impl Fx {
    /// An empty fixture.
    pub fn new() -> Fx {
        Fx::default()
    }

    /// Adds a record (with `$k`).
    pub fn push(&mut self, record: Value) -> &mut Fx {
        self.records.push(record);
        self
    }

    /// Sets a member of the last record pushed.
    pub fn with(&mut self, key: &str, value: Value) -> &mut Fx {
        self.records.last_mut().expect("a record")[key] = value;
        self
    }

    /// A library item of kind `k` with copy ID `n`.
    fn lib_item(&mut self, k: &str, n: u128, item: &Value) {
        let mut doc = json!({
            "$k": k, "id": us(n), "library_id": us(LIBRARY), "item_id": us(n + 0x1_0000),
            "version": "1.0.0", "sha256": format!("sha256:{}", format!("{:02x}", n % 251).repeat(32)),
        });
        for (key, v) in item.as_object().expect("item is an object") {
            doc[key] = v.clone();
        }
        self.lib.push(doc);
    }

    /// A function definition with copy ID `n`, its symbol `sym`, CPs `(idx, designation, extra members)`
    /// and internal links.
    fn fndef(&mut self, n: u128, sym: u128, cps: &[(u16, &str, Value)], internal: &Value) {
        self.fndef_with(n, sym, cps, internal, &json!({}));
    }

    /// A function definition as [`Fx::fndef`] gives it, with the extra item members `extra` (such as `flags`).
    pub fn fndef_with(
        &mut self,
        n: u128,
        sym: u128,
        cps: &[(u16, &str, Value)],
        internal: &Value,
        extra: &Value,
    ) {
        let cp_json: Vec<Value> = cps
            .iter()
            .map(|(idx, d, extra)| {
                let mut c = json!({"idx": idx, "designation": d});
                if let Some(m) = extra.as_object() {
                    for (k, v) in m {
                        c[k] = v.clone();
                    }
                }
                c
            })
            .collect();
        let mut item = json!({
            "meta": {}, "category": ["Misc", "Misc", "Plain"], "contact_kind": "pole",
            "cps": cp_json, "internal": internal,
        });
        for (k, v) in extra.as_object().expect("extra is an object") {
            item[k] = v.clone();
        }
        self.lib_item("fndef", n, &item);
        let pins: Vec<Value> = (0..cps.len())
            .map(|i| json!({"idx": i, "fn_cp": i, "pos": [0, i as i64 * 1_000_000], "dir": "up"}))
            .collect();
        self.lib_item(
            "symbol",
            sym,
            &json!({
                "meta": {}, "name": "symbol",
                "fndef": {"library": us(LIBRARY), "item": us(n + 0x1_0000), "version_req": "^1"},
                "grid": {"num": 4_000_000, "den": 1}, "cps": pins, "anchor": "origin",
            }),
        );
    }

    /// Two pages and the function and interruption-point definitions with their symbols.
    pub fn base() -> Fx {
        let mut fx = Fx::new();
        let none = Value::Null;
        let no_links = json!([]);
        fx.fndef(
            FNDEF,
            SYM,
            &[(0, "1", none.clone()), (1, "2", none.clone())],
            &no_links,
        );
        fx.fndef(FNDEF_IP, SYM_IP, &[(0, "1", none.clone())], &no_links);
        fx.fndef(
            FNDEF_PASS,
            SYM_PASS,
            &[(0, "1", none.clone()), (1, "2", none.clone())],
            &json!([{"a": 0, "b": 1, "kind": {"pass_through": {"opens": true}}}]),
        );
        fx.fndef(
            FNDEF_PE,
            SYM_PE,
            &[(0, "1", none), (1, "PE", json!({"potential_role": "pe"}))],
            &no_links,
        );
        fx.fndef(FNDEF_IP0, SYM_IP0, &[], &no_links);
        fx.fndef(
            FNDEF_M,
            SYM_M,
            &[(0, "1", Value::Null), (1, "-", json!({"potential_role": "m"}))],
            &no_links,
        );
        for (n, page) in [(P1, 1), (P2, 2)] {
            fx.push(json!({
                "$k": "page", "id": us(n), "folio": {"number": page}, "kind": "multi_line",
                "structure": {}, "title": {"en": "page"}, "grid": {"num": 4_000_000, "den": 1},
            }));
        }
        fx
    }

    /// A device with tag letter and counter.
    pub fn device(&mut self, n: u128, letter: &str, counter: Option<u32>) -> &mut Fx {
        let mut tag = json!({"letter": letter});
        if let Some(c) = counter {
            tag["counter"] = json!(c);
        }
        self.push(json!({"$k": "device", "id": us(n), "tag": tag}))
    }

    /// A function `n` of the definition `def` (symbol `sym`) with its own device (tag `-K<n>`), drawn on `page`.
    pub fn func_of(&mut self, n: u128, def: u128, sym: u128, page: u128) {
        self.device(n + 0x20_000, "K", Some(n as u32));
        self.push(json!({"$k": "function", "id": us(n), "device": us(n + 0x20_000), "def": us(def)}));
        self.rep(n, sym, page);
    }

    fn rep(&mut self, n: u128, sym: u128, page: u128) {
        self.push(json!({
            "$k": "representation", "id": us(n + 0x10_000), "functions": [us(n)], "page": us(page),
            "symbol": us(sym), "pos": [0, n as i64 * 1_000_000],
        }));
    }

    /// A device `n` with tag letter and counter, and one function drawn on `page` (so that the page-based
    /// numbering gives it a complete designation).
    pub fn drawn_device(&mut self, n: u128, letter: &str, counter: Option<u32>, page: u128) {
        self.device(n, letter, counter);
        self.push(json!({"$k": "function", "id": us(n + 0x1000), "device": us(n), "def": us(FNDEF)}));
        self.rep(n + 0x1000, SYM, page);
    }

    /// A function `n` of the PE-role definition (`pe`) or the M-role one, whose device has the tag `-K<counter>`
    /// whatever its ID: for ID-permutation tests, where the printed key must not follow the ID order.
    pub fn func_role_tagged(&mut self, n: u128, pe: bool, counter: u32, page: u128) {
        let (def, sym) = if pe { (FNDEF_PE, SYM_PE) } else { (FNDEF_M, SYM_M) };
        self.device(n + 0x20_000, "K", Some(counter));
        self.push(json!({"$k": "function", "id": us(n), "device": us(n + 0x20_000), "def": us(def)}));
        self.rep(n, sym, page);
    }

    /// A plain two-CP function `n` drawn on `page`, its CP `idx` marked unused.
    pub fn func_unused(&mut self, n: u128, idx: u16, page: u128) {
        self.device(n + 0x20_000, "K", Some(n as u32));
        self.push(json!({
            "$k": "function", "id": us(n), "device": us(n + 0x20_000), "def": us(FNDEF),
            "cp_overrides": [{"idx": idx, "unused": true}],
        }));
        self.rep(n, SYM, page);
    }

    /// A plain two-CP function `n` whose device has the tag `-K<counter>`, drawn on `page`.
    pub fn func_tagged(&mut self, n: u128, counter: u32, page: u128) {
        self.device(n + 0x20_000, "K", Some(counter));
        self.push(json!({"$k": "function", "id": us(n), "device": us(n + 0x20_000), "def": us(FNDEF)}));
        self.rep(n, SYM, page);
    }

    /// A plain two-CP function `n` drawn on `page`.
    pub fn func(&mut self, n: u128, page: u128) {
        self.func_of(n, FNDEF, SYM, page);
    }

    /// A function `n` with a PE-role CP 1, drawn on `page`.
    pub fn func_pe(&mut self, n: u128, page: u128) {
        self.func_of(n, FNDEF_PE, SYM_PE, page);
    }

    /// A function `n` with an M-role CP 1, drawn on `page`.
    pub fn func_m(&mut self, n: u128, page: u128) {
        self.func_of(n, FNDEF_M, SYM_M, page);
    }

    /// A pass-through function `n` drawn on `page`.
    pub fn pass(&mut self, n: u128, page: u128) {
        self.func_of(n, FNDEF_PASS, SYM_PASS, page);
    }

    /// An interruption point `n` named `name` (direction `dir`) drawn on `page`, with a connection point.
    pub fn ip(&mut self, n: u128, page: u128, name: &str, dir: &str) {
        self.ip_of(n, (FNDEF_IP, SYM_IP), page, name, dir);
    }

    /// An interruption point `n` whose definition has no connection point.
    pub fn ip_without_cp(&mut self, n: u128, page: u128, name: &str, dir: &str) {
        self.ip_of(n, (FNDEF_IP0, SYM_IP0), page, name, dir);
    }

    fn ip_of(&mut self, n: u128, (def, sym): (u128, u128), page: u128, name: &str, dir: &str) {
        self.push(json!({
            "$k": "function", "id": us(n), "def": us(def),
            "kind": {"ip": {"name": name, "direction": dir}},
        }));
        self.rep(n, sym, page);
    }

    /// A terminal device `n` on strip `strip` (rail-bonded), in `location` when given, with a PE function
    /// `f` drawn on `page` when `pe` is set.
    pub fn rail_terminal(&mut self, n: u128, strip: u128, location: Option<u128>, pe_function: Option<u128>) {
        self.device(n, "X", None).with(
            "terminal",
            json!({"strip": us(strip), "seq": n as u32, "designation": n.to_string(), "rail_bonded": true}),
        );
        if let Some(loc) = location {
            self.with("structure", json!({"location": us(loc)}));
        }
        if let Some(f) = pe_function {
            self.push(json!({"$k": "function", "id": us(f), "device": us(n), "def": us(FNDEF_PE)}));
            self.rep(f, SYM_PE, P1);
        }
    }

    /// A connection `n` between `a` and `b` on `page`; `extra` members (kind, core, overrides) are merged.
    pub fn conn(&mut self, n: u128, page: u128, a: Ep, b: Ep, extra: &Value) {
        let mut r = json!({"$k": "connection", "id": us(n), "page": us(page), "a": a.json(), "b": b.json()});
        if let Some(m) = extra.as_object() {
            for (k, v) in m {
                r[k] = v.clone();
            }
        }
        self.push(r);
    }

    /// A dot junction `n` on `page` with the connection `sequence`.
    pub fn junction(&mut self, n: u128, page: u128, sequence: &[u128]) {
        let seq: Vec<String> = sequence.iter().map(|c| us(*c)).collect();
        self.push(json!({
            "$k": "junction", "id": us(n), "page": us(page), "at": [0, 0], "style": "dot", "sequence": seq,
        }));
    }

    /// A potential definition `n` at `at` with `extra` members (`ptype`, `voltage`, ...).
    pub fn def(&mut self, n: u128, at: Ep, extra: &Value) {
        let mut r = json!({"$k": "potential_def", "id": us(n), "at": at.json(), "name": "P"});
        if let Some(m) = extra.as_object() {
            for (k, v) in m {
                r[k] = v.clone();
            }
        }
        self.push(r);
    }

    /// The model.
    pub fn build(&self) -> Model {
        model_json(self.settings.clone(), &self.records, &self.lib)
    }
}
