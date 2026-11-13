// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The part a source reader hands to the parts import.

use serde_json::{Map, Value};

/// One part as a source reader gives it: what the source says, before the target decides who the manufacturer is, what
/// the item ID is and whether the target holds the part already (`ew-import-catalog`'s `Placer`). The members of the
/// `Part` that depend on none of those are its JSON `body` (the `lib/` JSON form, read strictly when the part is
/// placed).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PartDraft {
    /// The row's line, or the part's number in its file: the location of the report entries.
    pub line: u32,
    /// The part number as the source has it; "A / B" (with white space around the `/`) names two candidates.
    pub part_number: String,
    /// The manufacturer as the source names it: a master record's key, name, alias or brand, else a new manufacturer.
    pub manufacturer: String,
    /// The part's JSON members but `meta`, `manufacturer` and `part_number`.
    pub body: Map<String, Value>,
    /// Members of `meta` beyond `key`, `sources` and `as_of` (`licence`, `foreign_terms`).
    pub meta: Map<String, Value>,
    /// `meta.sources`: where the data came from.
    pub sources: Vec<String>,
}
