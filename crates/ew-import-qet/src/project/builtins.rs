// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in starter documents a project import references by key: the interruption-point FnDef
//! `ew.fn.ip`, the QET-compatible frame form and its title block. The host supplies them (`content/fndefs`,
//! `content/forms`, `content/symbols/qet10`); the importer never reads files itself. Repeated from
//! `ew-import-tables` in reduced form, because importers may not depend on each other.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Uuid;
use ew_schema::{FnDef, FormDef, LibCopy, LibDocument};

/// The built-in documents by key.
#[derive(Clone, Debug, Default)]
pub struct Builtins {
    by_key: BTreeMap<String, LibDocument>,
    key_by_copy: BTreeMap<Uuid, String>,
    key_by_item: BTreeMap<Uuid, String>,
}

/// The key, copy ID and item ID of a symbol, FnDef or form; `None` for other documents and documents
/// without a key.
fn identity(doc: &LibDocument) -> Option<(String, Uuid, Uuid)> {
    let (key, copy, item) = match doc {
        LibDocument::Symbol(c) => (c.item.meta.key.as_ref(), c.id.as_uuid(), c.item_id.as_uuid()),
        LibDocument::Fndef(c) => (c.item.meta.key.as_ref(), c.id.as_uuid(), c.item_id.as_uuid()),
        LibDocument::Form(c) => (c.item.meta.key.as_ref(), c.id.as_uuid(), c.item_id.as_uuid()),
        _ => return None,
    };
    Some((key?.to_string(), copy, item))
}

impl Builtins {
    /// The library of `docs`: symbols, FnDefs and forms with a `meta.key`; other documents and documents
    /// without a key are ignored, and of two documents with one key the first is kept.
    pub fn new(docs: impl IntoIterator<Item = LibDocument>) -> Builtins {
        let mut lib = Builtins::default();
        for doc in docs {
            let Some((key, copy, item)) = identity(&doc) else {
                continue;
            };
            if lib.by_key.contains_key(&key) {
                continue;
            }
            lib.key_by_copy.insert(copy, key.clone());
            lib.key_by_item.insert(item, key.clone());
            lib.by_key.insert(key, doc);
        }
        lib
    }

    /// The FnDef copy with `key`.
    pub fn fndef(&self, key: &str) -> Option<&LibCopy<FnDef>> {
        match self.by_key.get(key)? {
            LibDocument::Fndef(c) => Some(c),
            _ => None,
        }
    }

    /// The form copy with `key`.
    pub fn form(&self, key: &str) -> Option<&LibCopy<FormDef>> {
        match self.by_key.get(key)? {
            LibDocument::Form(c) => Some(c),
            _ => None,
        }
    }

    /// The documents of `keys` and everything they reference (a symbol's FnDef, a form's title block and
    /// continuation), in key order.
    pub fn closure(&self, keys: &BTreeSet<String>) -> Vec<LibDocument> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut todo: Vec<String> = keys.iter().cloned().collect();
        while let Some(key) = todo.pop() {
            let Some(doc) = self.by_key.get(&key) else {
                continue;
            };
            if !seen.insert(key) {
                continue;
            }
            match doc {
                LibDocument::Symbol(c) => {
                    if let Some(k) = self.key_by_item.get(&c.item.fndef.item.as_uuid()) {
                        todo.push(k.clone());
                    }
                }
                LibDocument::Form(c) => {
                    for r in [c.item.title_block, c.item.continuation].into_iter().flatten() {
                        if let Some(k) = self.key_by_copy.get(&r.as_uuid()) {
                            todo.push(k.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        seen.iter().filter_map(|k| self.by_key.get(k).cloned()).collect()
    }
}
