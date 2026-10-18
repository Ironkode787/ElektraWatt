// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in library the tables reference by key (doc 05 §13.2, doc 07 §9.1): starter symbols, FnDefs and
//! forms as embedded copies. The host supplies the documents (`content/symbols/qet10`, `content/fndefs`,
//! `content/forms`); the importer never reads files itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ew_ids::{Id, Uuid};
use ew_schema::{FnDef, FormDef, LibCopy, LibDocument, LibItem, LibRef, Library, SymbolDef};

/// **Provisional** library ID of EW's built-in system items, the value of `ew_import_qet::BUILTIN_LIBRARY` and
/// of the starter content's `library_id` (`content/symbols/README.md`), until the system library gets its own
/// decided identity (WP-S1.19a). Repeated here because importers may not depend on each other.
pub const BUILTIN_LIBRARY: Uuid = Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

/// Why a set of documents is not a usable built-in library.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LibraryError {
    /// A document without `meta.key`.
    MissingKey {
        /// The copy ID.
        id: String,
    },
    /// Two documents with the same key.
    DuplicateKey {
        /// The key.
        key: String,
    },
    /// A document of a kind the tables cannot reference (parts, unknown kinds).
    UnsupportedKind {
        /// The key.
        key: String,
    },
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::MissingKey { id } => write!(f, "library document {id} has no key"),
            LibraryError::DuplicateKey { key } => write!(f, "two library documents have the key {key}"),
            LibraryError::UnsupportedKind { key } => {
                write!(
                    f,
                    "library document {key} is not a symbol, function definition or form"
                )
            }
        }
    }
}

impl std::error::Error for LibraryError {}

/// The built-in symbols, FnDefs and forms, by key.
#[derive(Clone, Debug, Default)]
pub struct BuiltinLibrary {
    by_key: BTreeMap<String, LibDocument>,
    key_by_copy: BTreeMap<Uuid, String>,
    key_by_item: BTreeMap<Uuid, String>,
}

impl BuiltinLibrary {
    /// The library of `docs` (symbols, FnDefs and forms, each with a `meta.key`).
    ///
    /// # Errors
    ///
    /// [`LibraryError`] for a document without key, a repeated key or a part or unknown document.
    pub fn new(docs: impl IntoIterator<Item = LibDocument>) -> Result<BuiltinLibrary, LibraryError> {
        let mut lib = BuiltinLibrary::default();
        for doc in docs {
            let (key, copy, item) = match &doc {
                LibDocument::Symbol(c) => (c.item.meta.key.clone(), c.id.as_uuid(), c.item_id.as_uuid()),
                LibDocument::Fndef(c) => (c.item.meta.key.clone(), c.id.as_uuid(), c.item_id.as_uuid()),
                LibDocument::Form(c) => (c.item.meta.key.clone(), c.id.as_uuid(), c.item_id.as_uuid()),
                LibDocument::Part(c) => {
                    return Err(LibraryError::UnsupportedKind {
                        key: c
                            .item
                            .meta
                            .key
                            .as_ref()
                            .map_or_else(|| c.id.to_string(), |k| k.to_string()),
                    });
                }
                _ => {
                    return Err(LibraryError::UnsupportedKind {
                        key: doc.kind().map_or("unknown", |k| k.name()).to_owned(),
                    });
                }
            };
            let key = key
                .ok_or_else(|| LibraryError::MissingKey { id: copy.to_string() })?
                .to_string();
            if lib.by_key.contains_key(&key) {
                return Err(LibraryError::DuplicateKey { key });
            }
            lib.key_by_copy.insert(copy, key.clone());
            lib.key_by_item.insert(item, key.clone());
            lib.by_key.insert(key, doc);
        }
        Ok(lib)
    }

    /// The number of documents.
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    /// Whether the library is empty.
    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    /// The document with `key`.
    pub fn get(&self, key: &str) -> Option<&LibDocument> {
        self.by_key.get(key)
    }

    /// The symbol copy with `key`.
    pub fn symbol(&self, key: &str) -> Option<&LibCopy<SymbolDef>> {
        match self.by_key.get(key)? {
            LibDocument::Symbol(c) => Some(c),
            _ => None,
        }
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

    /// A reference to the FnDef `key` as a part template names it: `{library, item, version_req}` with the
    /// key in `extra["key"]` (content README "Identity").
    pub(crate) fn fndef_ref(&self, key: &str) -> Option<LibRef> {
        let c = self.fndef(key)?;
        let mut extra = ew_schema::Extra::new();
        extra.insert("key".into(), serde_json::Value::String(key.to_owned()));
        Some(LibRef {
            library: Id::<Library>::from_uuid(c.library_id.as_uuid()),
            item: Id::<LibItem>::from_uuid(c.item_id.as_uuid()),
            version_req: caret(&c.version),
            extra,
        })
    }

    /// The documents of `keys` and everything they reference (a symbol's FnDef, a form's title block and
    /// continuation), sorted by kind, then copy ID.
    pub(crate) fn closure(&self, keys: &BTreeSet<String>) -> Vec<LibDocument> {
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
        let mut docs: Vec<LibDocument> = seen.iter().filter_map(|k| self.by_key.get(k).cloned()).collect();
        docs.sort_by_key(|d| (d.kind().map(|k| k.name()), doc_id(d)));
        docs
    }
}

/// The copy ID of a library document (`None` for an unknown kind).
pub(crate) fn doc_id(d: &LibDocument) -> Option<Uuid> {
    match d {
        LibDocument::Part(c) => Some(c.id.as_uuid()),
        LibDocument::Symbol(c) => Some(c.id.as_uuid()),
        LibDocument::Fndef(c) => Some(c.id.as_uuid()),
        LibDocument::Form(c) => Some(c.id.as_uuid()),
        _ => None,
    }
}

/// `^MAJOR.MINOR` for a 0.x version, `^MAJOR` otherwise (the requirement forms of the starter content).
fn caret(version: &str) -> String {
    let mut parts = version.split('.');
    match (parts.next(), parts.next()) {
        (Some("0"), Some(minor)) => format!("^0.{minor}"),
        (Some(major), _) => format!("^{major}"),
        _ => "*".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::caret;

    #[test]
    fn caret_requirements() {
        assert_eq!(caret("0.1.0"), "^0.1");
        assert_eq!(caret("1.0.0"), "^1");
    }
}
