// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The library item a project's own copy was made from: `meta.source.fork_of` (doc 07 §2.1), read and written in one
//! place.

use serde_json::Value;

use crate::ItemMeta;

impl ItemMeta {
    /// The library item this item was forked from: `source.fork_of` (`{"library", "item", "version"}`), a member format
    /// 0.1 keeps untyped in [`ItemMeta::extra`] (doc 07 §2.1). `None` for an item that is no fork, or one whose
    /// library or item does not read as an ID.
    pub fn fork_of(&self) -> Option<ForkOf> {
        let fork = self.extra.get("source")?.get("fork_of")?;
        let id = |k: &str| fork.get(k)?.as_str()?.parse::<ew_ids::Uuid>().ok();
        Some(ForkOf {
            library: id("library")?,
            item: id("item")?,
            version: fork
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    /// Marks this item as forked from `fork` ([`ItemMeta::fork_of`]), replacing the `source` member.
    pub fn set_fork_of(&mut self, fork: &ForkOf) {
        self.extra.insert(
            "source".into(),
            serde_json::json!({"fork_of": {
                "library": fork.library.to_string(),
                "item": fork.item.to_string(),
                "version": fork.version,
            }}),
        );
    }
}

/// The library item a project's own copy was made from ([`ItemMeta::fork_of`]): the library, the item and its
/// version when it was forked.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ForkOf {
    /// The library of the item.
    pub library: ew_ids::Uuid,
    /// The item.
    pub item: ew_ids::Uuid,
    /// The item's version the fork was made of.
    pub version: String,
}
