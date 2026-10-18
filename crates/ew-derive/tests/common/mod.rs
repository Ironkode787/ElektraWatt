// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A JSON fixture builder for the integration tests: records are written as the JSON of doc 05 §3 and
//! read through `ew-schema`, so a fixture reads like a project file.

#![allow(dead_code)]

use ew_ids::{Id, Uuid};
use ew_model::Model;
use ew_schema::{LibDocument, Project, Record};
use serde_json::{Value, json};

/// The canonical UUID text of `n`.
pub fn uid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

/// The ID `n`.
pub const fn id<T>(n: u128) -> Id<T> {
    Id::from_uuid(Uuid::from_u128(n))
}

/// A fixed fake digest for library copies.
pub fn sha(n: u8) -> String {
    format!("sha256:{}", format!("{n:02x}").repeat(32))
}

/// The provisional library of the fixtures' library copies.
pub const LIBRARY: u128 = 0x7000;

/// A fixture under construction.
#[derive(Clone, Debug)]
pub struct Fx {
    /// The project record (without `$k`).
    pub project: Value,
    /// The records, each with `$k`.
    pub records: Vec<Value>,
    /// The library documents, each with `$k` and copy metadata.
    pub lib: Vec<Value>,
}

impl Default for Fx {
    fn default() -> Self {
        Fx::new()
    }
}

impl Fx {
    /// A project with ID 1, profile 2027.1 and nothing else.
    pub fn new() -> Fx {
        Fx {
            project: json!({"id": uid(1), "derivation_profile": "2027.1"}),
            records: Vec::new(),
            lib: Vec::new(),
        }
    }

    /// Sets a top-level member of the project record.
    pub fn project_set(&mut self, key: &str, value: Value) -> &mut Fx {
        self.project
            .as_object_mut()
            .expect("project is an object")
            .insert(key.to_owned(), value);
        self
    }

    /// Adds a record (with `$k`).
    pub fn push(&mut self, record: Value) -> &mut Fx {
        self.records.push(record);
        self
    }

    /// Adds a library document (with `$k` and copy metadata).
    pub fn lib(&mut self, doc: Value) -> &mut Fx {
        self.lib.push(doc);
        self
    }

    /// Adds a library item of kind `k` with copy ID `n`, wrapping `item` with copy metadata.
    pub fn lib_item(&mut self, k: &str, n: u128, item: Value) -> &mut Fx {
        let mut doc = json!({
            "$k": k,
            "id": uid(n),
            "library_id": uid(LIBRARY),
            "item_id": uid(n + 0x1_0000),
            "version": "1.0.0",
            "sha256": sha((n % 251) as u8),
        });
        let map = doc.as_object_mut().expect("object");
        for (key, v) in item.as_object().expect("item is an object") {
            map.insert(key.clone(), v.clone());
        }
        self.lib(doc)
    }

    /// A function definition with copy ID `n` and CPs `(idx, designation)`.
    pub fn fndef(&mut self, n: u128, category: [&str; 3], cps: &[(u16, &str)]) -> &mut Fx {
        let cps: Vec<Value> = cps
            .iter()
            .map(|(idx, d)| json!({"idx": idx, "designation": d}))
            .collect();
        self.lib_item(
            "fndef",
            n,
            json!({"meta": {}, "category": category, "contact_kind": "pole", "cps": cps}),
        )
    }

    /// A symbol with copy ID `n` drawing FnDef copy `fndef` (by its item ID), with an anchor policy
    /// (`json!("origin")`, `json!({"named": [x, y]})` …) and CPs `(idx, fn_cp, x, y)` in nm.
    pub fn symbol(&mut self, n: u128, fndef: u128, anchor: Value, cps: &[(u16, u16, i64, i64)]) -> &mut Fx {
        let cps: Vec<Value> = cps
            .iter()
            .map(|(idx, fn_cp, x, y)| json!({"idx": idx, "fn_cp": fn_cp, "pos": [x, y], "dir": "up"}))
            .collect();
        self.lib_item(
            "symbol",
            n,
            json!({
                "meta": {},
                "name": "symbol",
                "fndef": {"library": uid(LIBRARY), "item": uid(fndef + 0x1_0000), "version_req": "^1"},
                "grid": {"num": 4_000_000, "den": 1},
                "cps": cps,
                "anchor": anchor,
            }),
        )
    }

    /// A structure node.
    pub fn node(&mut self, n: u128, aspect: &str, code: &str, parent: Option<u128>) -> &mut Fx {
        let mut r = json!({"$k": "structure_node", "id": uid(n), "aspect": aspect, "code": code});
        if let Some(p) = parent {
            r["parent"] = json!(uid(p));
        }
        self.push(r)
    }

    /// A section.
    pub fn section(
        &mut self,
        n: u128,
        number: u16,
        range: (u32, u32),
        title: &str,
        divider: bool,
    ) -> &mut Fx {
        self.push(json!({
            "$k": "section", "id": uid(n), "number": number, "folio_range": [range.0, range.1],
            "title": {"en": title}, "divider": divider,
        }))
    }

    /// A page with folio `number`, type `kind` ("multi_line" …), a structure `{aspect: node}` and a title.
    pub fn page(
        &mut self,
        n: u128,
        number: u32,
        section: Option<u128>,
        kind: &str,
        structure: &[(&str, u128)],
        title: &str,
    ) -> &mut Fx {
        let mut s = serde_json::Map::new();
        for (aspect, node) in structure {
            s.insert((*aspect).to_owned(), json!(uid(*node)));
        }
        let mut folio = json!({"number": number});
        if let Some(sec) = section {
            folio["section"] = json!(uid(sec));
        }
        self.push(json!({
            "$k": "page", "id": uid(n), "folio": folio, "kind": kind, "structure": s,
            "title": {"en": title}, "grid": {"num": 4_000_000, "den": 1},
        }))
    }

    /// A device with a tag letter and counter.
    pub fn device(&mut self, n: u128, letter: &str, counter: Option<u32>) -> &mut Fx {
        let mut tag = json!({"letter": letter});
        if let Some(c) = counter {
            tag["counter"] = json!(c);
        }
        self.push(json!({"$k": "device", "id": uid(n), "tag": tag}))
    }

    /// The record with ID `n`.
    ///
    /// # Panics
    ///
    /// When no record has that ID.
    pub fn record(&mut self, n: u128) -> &mut Value {
        let key = uid(n);
        self.records
            .iter_mut()
            .find(|r| r["id"] == json!(key))
            .unwrap_or_else(|| panic!("no record {key}"))
    }

    /// Sets a member of the last record pushed.
    pub fn with(&mut self, key: &str, value: Value) -> &mut Fx {
        let last = self.records.last_mut().expect("a record");
        last[key] = value;
        self
    }

    /// A function of a device with FnDef copy `def`; `main` sets role main (else aux).
    pub fn function(&mut self, n: u128, device: Option<u128>, def: u128, main: bool) -> &mut Fx {
        let mut r = json!({"$k": "function", "id": uid(n), "def": uid(def)});
        if let Some(d) = device {
            r["device"] = json!(uid(d));
        }
        if !main {
            r["role"] = json!("aux");
        }
        self.push(r)
    }

    /// A representation of functions on a page, drawn with symbol copy `symbol` at `(x, y)` nm.
    pub fn rep(&mut self, n: u128, page: u128, symbol: u128, functions: &[u128], x: i64, y: i64) -> &mut Fx {
        let functions: Vec<String> = functions.iter().map(|f| uid(*f)).collect();
        self.push(json!({
            "$k": "representation", "id": uid(n), "functions": functions, "page": uid(page),
            "symbol": uid(symbol), "pos": [x, y],
        }))
    }

    /// The model.
    ///
    /// # Panics
    ///
    /// When a record does not parse or the model violates a structural invariant.
    pub fn build(&self) -> Model {
        let project: Project = serde_json::from_value(self.project.clone()).expect("project parses");
        let records: Vec<Record> = self
            .records
            .iter()
            .map(|r| serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("record {r}: {e}")))
            .collect();
        let lib: Vec<LibDocument> = self
            .lib
            .iter()
            .map(|d| serde_json::from_value(d.clone()).unwrap_or_else(|e| panic!("lib document {d}: {e}")))
            .collect();
        Model::from_records(project, records, lib).unwrap_or_else(|e| panic!("model: {e:?}"))
    }
}
