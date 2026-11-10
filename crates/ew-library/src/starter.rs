// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The starter library: the shipped symbols and function definitions of `content/symbols`,
//! `content/symbols/qet10` and `content/fndefs`, compiled in, so that a new project without symbol copies can place
//! symbols. Each item is one library document in the `lib/` JSON form that `library.embed` takes (doc 05 §13.2);
//! [`items`] lists them in key order, the key being the file stem (`ew.iec.mcb-1p@qet10`, `ew.fn.mcb-1p`).
//!
//! The picker of the place tool lists the starter symbols a project has not embedded; choosing one embeds the symbol
//! and its function definition through `library.embed` (the host's commit, an undo step of its own).

/// The library ID of the starter, the system library `ew-starter`: equal to `ew_interop_api::SYSTEM_LIBRARY`
/// (`ew-library` does not depend on `ew-interop-api`, so `ew-api`'s tests assert the equality).
pub const LIBRARY: ew_ids::Uuid = ew_ids::Uuid::from_u128(0x7202fd8b_1024_4bf1_9e99_670772eaa589);

/// The starter's library key.
pub const KEY: &str = "ew-starter";

/// What a starter item is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StarterKind {
    /// A symbol (`$k` `symbol`).
    Symbol,
    /// A function definition (`$k` `fndef`).
    Fndef,
}

/// One starter item: its key (the file stem), its kind and its library document as JSON text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StarterItem {
    /// The key, the file stem (`ew.iec.mcb-1p@qet10`).
    pub key: &'static str,
    /// Symbol or function definition.
    pub kind: StarterKind,
    /// The library document (`lib/` JSON form).
    pub json: &'static str,
}

macro_rules! content {
    ($path:literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/", $path))
    };
}

/// The items in key order (byte order of the keys): every `*.json` of the three folders, which the build script
/// lists.
const ITEMS: &[StarterItem] = include!(concat!(env!("OUT_DIR"), "/starter_items.rs"));

/// Every starter item, in key order.
pub fn items() -> &'static [StarterItem] {
    ITEMS
}

/// The starter item with this key.
pub fn item(key: &str) -> Option<&'static StarterItem> {
    ITEMS.binary_search_by(|i| i.key.cmp(key)).ok().map(|i| &ITEMS[i])
}

#[cfg(test)]
mod tests {
    use ew_schema::LibDocument;

    use super::*;

    #[test]
    fn every_item_parses_as_the_library_document_library_embed_takes() {
        for i in items() {
            let doc = LibDocument::from_json(i.json).unwrap_or_else(|e| panic!("{}: {e}", i.key));
            let (lib, key) = match (i.kind, &doc) {
                (StarterKind::Symbol, LibDocument::Symbol(c)) => {
                    (c.library_id.as_uuid(), c.item.meta.key.clone())
                }
                (StarterKind::Fndef, LibDocument::Fndef(c)) => {
                    (c.library_id.as_uuid(), c.item.meta.key.clone())
                }
                _ => panic!("{}: kind", i.key),
            };
            assert_eq!(key.as_ref().map(|k| k.as_str()), Some(i.key));
            assert_eq!(lib, LIBRARY, "{}: names the starter library", i.key);
        }
        assert_eq!(
            item("ew.iec.mcb-1p@qet10").map(|i| i.kind),
            Some(StarterKind::Symbol)
        );
        assert_eq!(item("ew.fn.mcb-1p").map(|i| i.kind), Some(StarterKind::Fndef));
        assert!(item("no.such.key").is_none());
        assert!(items().windows(2).all(|w| w[0].key < w[1].key), "key order");
    }
}
