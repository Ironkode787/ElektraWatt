// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `library.embed` parameters that place a starter or an installed library's symbol or part: the copies the
//! project does not hold yet, and an installed symbol by its qualified address.
//!
//! Not here: the starter documents and the catalogue rows (`catalogue.rs`); the copies a valve terminal's cells
//! and the bus ports need (`shape.rs`).

use std::collections::BTreeSet;

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::{LibCopy, LibDocument, LibRef, SymbolDef};
use ew_library::starter;

use super::catalogue::{starter_docs, starter_symbol};
use crate::Session;
use crate::output::join::fndef_copy;

impl Session {
    // Qualified addresses distinguish a library master from an older project copy with the same key.
    pub(crate) fn installed_symbol(&self, address: &str) -> Option<LibCopy<SymbolDef>> {
        self.libraries()
            .libraries()
            .into_iter()
            .flat_map(|l| self.libraries().documents(l.id, ew_schema::Kind::Symbol))
            .find_map(|doc| match doc {
                LibDocument::Symbol(s) if format!("library:{}:{}", s.library_id, s.item_id) == address => {
                    Some(s)
                }
                _ => None,
            })
    }

    /// The `library.embed` parameters (`{"copies": [...]}`) that embed the starter symbol `key` and its function
    /// definition, leaving out what the project has embedded already (by copy ID, which names the item at any
    /// version); `None` when `key` is not a starter symbol or both are embedded. The host commits it through the
    /// normal path.
    pub fn starter_embed(&self, key: &str) -> Option<serde_json::Value> {
        let q = self.query();
        let symbol = starter_symbol(key)?;
        let item = |id: Uuid| starter::items().iter().find(|i| starter_id(i.json) == Some(id));
        let mut copies: Vec<serde_json::Value> = Vec::new();
        if !q.symbols().contains(symbol.id) {
            copies.push(serde_json::from_str(item(symbol.id.as_uuid())?.json).ok()?);
        }
        let def = fndef_copy(&symbol.item.fndef);
        if !q.fndefs().contains(def)
            && let Some(i) = item(def.as_uuid())
        {
            copies.push(serde_json::from_str(i.json).ok()?);
        }
        if copies.is_empty() {
            return None;
        }
        Some(serde_json::json!({ "copies": copies }))
    }

    /// Documents needed to place a starter key or qualified installed-library symbol.
    ///
    /// # Errors
    /// A library reference cannot be read, or its closure conflicts with a project-held version.
    pub fn symbol_embed(&self, key: &str) -> Result<Option<serde_json::Value>, crate::ApiError> {
        if let Some(symbol) = self.installed_symbol(key) {
            let reference = LibRef {
                library: symbol.library_id,
                item: symbol.item_id,
                version_req: format!("={}", symbol.version),
                extra: ew_schema::Extra::new(),
            };
            let copies = self.closure_docs(&[reference])?;
            return Ok((!copies.is_empty()).then(|| serde_json::json!({"copies":copies})));
        }
        Ok(self.starter_embed(key))
    }

    /// The `library.embed` parameters (`{"copies": [...]}`) that make part copy `part`'s function template
    /// placeable with the catalogue symbol `symbol`: first the starter symbol `symbol` and its FnDef, each
    /// when `symbol` names a starter symbol and the project does not embed it (as [`Session::starter_embed`]
    /// gives them), then, in template order, the FnDef of every `function_template` entry that the project
    /// does not embed, taken from the starter library by the entry's FnDef copy ID; no copy twice. `Ok(None)`
    /// when nothing is missing or the project has no part copy `part`. `Err(keys)` when an entry's FnDef is
    /// neither embedded nor in the starter library: `keys` lists those entries' FnDef keys in template order
    /// (the reference's `key` member, else the FnDef copy ID as lower-case UUID text), and nothing is to be
    /// embedded.
    ///
    /// `part` may name a library's part the project does not hold (a catalogue row with `library`):
    /// its closure (the part, its template FnDefs, its manufacturer; [`Session::embed_command`]'s reference
    /// form) follows in the same parameters; a closure that does not resolve is `Err` with the library's
    /// message. The picker sends the `copies` with the placement itself (`device.place` or `box.black`,
    /// FT-656), so the placement is one undo step.
    pub fn starter_embed_part(
        &self,
        part: Uuid,
        symbol: &str,
    ) -> Result<Option<serde_json::Value>, Vec<String>> {
        let q = self.query();
        let library = if q.parts().contains(Id::from_uuid(part)) {
            None
        } else {
            self.library_part_ref(part)
        };
        let held = q.parts().get(Id::from_uuid(part)).map(|c| c.item.clone());
        let from_library = library.as_ref().and_then(|r| match self.libraries().resolve(r) {
            Ok(LibDocument::Part(c)) => Some(c.item),
            _ => None,
        });
        let Some(part) = held.or(from_library) else {
            return Ok(None);
        };
        let item = |id: Uuid| starter::items().iter().find(|i| starter_id(i.json) == Some(id));
        let mut copies: Vec<serde_json::Value> = Vec::new();
        let mut seen: BTreeSet<Uuid> = BTreeSet::new();
        if self.installed_symbol(symbol).is_some()
            && let Some(params) = self.symbol_embed(symbol).map_err(|e| vec![e.to_string()])?
            && let Some(documents) = params["copies"].as_array()
        {
            for document in documents {
                let id = serde_json::from_value::<Uuid>(document["id"].clone())
                    .map_err(|e| vec![e.to_string()])?;
                if seen.insert(id) {
                    copies.push(document.clone());
                }
            }
        }
        let push = |copies: &mut Vec<serde_json::Value>, id: Uuid| -> bool {
            let Some(i) = item(id) else { return false };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(i.json) else {
                return false;
            };
            copies.push(v);
            true
        };
        if let Some(starter) = starter_symbol(symbol) {
            // The starter symbol and its FnDef as `starter_embed` gives them: an item the starter library lacks is
            // skipped (never a reason for `Ok(None)`), and the symbol is pushed even when its FnDef is not.
            if !q.symbols().contains(starter.id) && push(&mut copies, starter.id.as_uuid()) {
                seen.insert(starter.id.as_uuid());
            }
            let def = fndef_copy(&starter.item.fndef);
            if !q.fndefs().contains(def) && push(&mut copies, def.as_uuid()) {
                seen.insert(def.as_uuid());
            }
        }
        // The template's FnDefs, in template order: the first copy of an embedded one is not pushed twice.
        let mut missing: Vec<String> = Vec::new();
        for entry in &part.function_template {
            let id = fndef_copy(&entry.fndef);
            if q.fndefs().contains(id) || seen.contains(&id.as_uuid()) {
                continue;
            }
            if !push(&mut copies, id.as_uuid()) {
                missing.push(
                    entry
                        .fndef
                        .extra
                        .get("key")
                        .and_then(serde_json::Value::as_str)
                        .map_or_else(|| id.to_string(), str::to_owned),
                );
                continue;
            }
            seen.insert(id.as_uuid());
        }
        if !missing.is_empty() {
            return Err(missing);
        }
        if let Some(r) = library {
            let docs = self
                .closure_docs(std::slice::from_ref(&r))
                .map_err(|e| vec![e.to_string()])?;
            for doc in docs {
                let Some(id) = doc.copy_meta().map(|c| c.id) else {
                    continue;
                };
                if seen.insert(id) {
                    copies.push(serde_json::to_value(&doc).map_err(|e| vec![e.to_string()])?);
                }
            }
        }
        if copies.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::json!({ "copies": copies })))
    }

    /// The library documents of `copies` (`lib/` JSON form) that the project does not hold, by copy ID, in their
    /// order (FT-656): the Place and black box tools carry the copies they armed with, and the host sends a placement
    /// only those it still needs, so a placement after the first one carries none (and one after an undo of the
    /// first carries them again). A document that does not read, or of a kind the project keeps no table of here,
    /// is kept (the command checks it).
    pub fn missing_copies(&self, copies: &[serde_json::Value]) -> Vec<serde_json::Value> {
        let q = self.query();
        copies
            .iter()
            .filter(|v| {
                let held = match LibDocument::from_value((*v).clone()) {
                    Ok(LibDocument::Part(c)) => q.parts().contains(c.id),
                    Ok(LibDocument::Symbol(c)) => q.symbols().contains(c.id),
                    Ok(LibDocument::Fndef(c)) => q.fndefs().contains(c.id),
                    Ok(LibDocument::Form(c)) => q.forms().contains(c.id),
                    Ok(LibDocument::Manufacturer(c)) => q.manufacturers().contains(c.id),
                    _ => false,
                };
                !held
            })
            .cloned()
            .collect()
    }
}

/// The `id` of a starter document's JSON text.
pub(super) fn starter_id(json: &str) -> Option<Uuid> {
    starter_docs()
        .iter()
        .zip(starter::items())
        .find(|(_, i)| i.json == json)
        .and_then(|((_, _, d), _)| d.copy_meta().map(|c| c.id))
}
