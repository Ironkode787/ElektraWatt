// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The line order of a JSON Lines file (doc 05 §3.3).

use ew_schema::{AnyId, Kind, Record};
use serde_json::Value;

/// The kind part of a line's sort key: ranked kinds by rank, then every other `$k` by its text, byte-wise.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Group {
    Ranked(u8),
    Unranked(String),
}

/// The ID part: the ID's bytes 8–15 as a big-endian `u64`, then the full ID; lines without an ID last.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum IdOrder {
    Id(u64, AnyId),
    Missing,
}

/// The sort key of one line (doc 05 §3.3): kind rank, then the ID's bytes 8–15 read as a big-endian
/// `u64` (the random bits), then the full ID. Kinds the registry does not rank (unknown kinds of a newer
/// format) sort after every ranked kind, by their `$k` text byte-wise. A reserved kind sorts by its rank.
///
/// The last component, `seq`, only orders lines with an equal kind and ID: 0 for a record of the model, the
/// original line number for a line kept verbatim (a duplicate, a quarantined line), so a file with such
/// lines is written in one deterministic order.
///
/// ```
/// use ew_format::LineKey;
///
/// let page = LineKey::new("page", Some("8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a".parse().unwrap()));
/// let rep = LineKey::new("representation", Some("6f03d8a5-c912-5e7b-8a40-1d5b39e2f7c6".parse().unwrap()));
/// let unknown = LineKey::new("frobnicator", None);
/// assert!(page < rep && rep < unknown);
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LineKey {
    group: Group,
    id: IdOrder,
    seq: u64,
}

impl LineKey {
    /// The key of a line of kind `kind_name` with ID `id`.
    pub fn new(kind_name: &str, id: Option<AnyId>) -> LineKey {
        let group = match Kind::from_name(kind_name).and_then(Kind::rank) {
            Some(rank) => Group::Ranked(rank),
            None => Group::Unranked(kind_name.to_owned()),
        };
        LineKey {
            group,
            id: id.map_or(IdOrder::Missing, |id| IdOrder::Id(id.sort_key(), id)),
            seq: 0,
        }
    }

    /// The key of a record; `None` when it has no `$k` (a record value without one).
    pub fn of(record: &Record) -> Option<LineKey> {
        let id = record.id().map(AnyId::from_uuid);
        Some(LineKey::new(record.kind_name()?, id))
    }

    /// The key of a parsed line's value, from its `$k` and `id` members when they are well formed; a value
    /// without a string `$k` sorts after every kind (as the kind `""` of no rank would not: it gets the
    /// largest possible group).
    pub fn of_value(value: &Value) -> LineKey {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<AnyId>().ok());
        match value.get("$k").and_then(Value::as_str) {
            Some(k) => LineKey::new(k, id),
            None => LineKey::keyless(),
        }
    }

    /// The key of a line that is not a JSON object with a `$k` (a syntax error kept verbatim): after every
    /// keyed line.
    pub fn keyless() -> LineKey {
        LineKey {
            // U+10FFFF sorts after every `$k` a file can hold in UTF-8 byte order.
            group: Group::Unranked("\u{10ffff}".to_owned()),
            id: IdOrder::Missing,
            seq: 0,
        }
    }

    /// The same key with the tie-breaker `seq`.
    pub fn with_seq(mut self, seq: u64) -> LineKey {
        self.seq = seq;
        self
    }

    /// The ID, when the line has one.
    pub fn id(&self) -> Option<AnyId> {
        match self.id {
            IdOrder::Id(_, id) => Some(id),
            IdOrder::Missing => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> AnyId {
        s.parse().unwrap()
    }

    #[test]
    fn rank_then_random_bits_then_full_id() {
        // Same bytes 8–15, so the full ID decides; the timestamp bytes 0–7 never come first.
        let a = id("ffffffff-ffff-7fff-8000-000000000001");
        let b = id("00000000-0000-7000-8000-000000000002");
        assert!(LineKey::new("device", Some(a)) < LineKey::new("device", Some(b)));
        let c = id("00000000-0000-7000-8000-000000000001");
        assert!(LineKey::new("device", Some(c)) < LineKey::new("device", Some(a)));
        // Rank 31 before 32 whatever the IDs.
        assert!(LineKey::new("device", Some(b)) < LineKey::new("function", Some(c)));
        // Reserved kinds sort by rank: `revision` (60) before `conflict` (255), both before unknown kinds.
        assert!(LineKey::new("revision", Some(b)) < LineKey::new("conflict", Some(c)));
        assert!(LineKey::new("conflict", Some(c)) < LineKey::new("aaa", Some(c)));
        assert!(LineKey::new("aaa", Some(c)) < LineKey::new("aab", None));
        assert!(LineKey::new("zzz", None) < LineKey::keyless());
        assert!(LineKey::new("device", Some(a)) < LineKey::new("device", Some(a)).with_seq(3));
    }
}
