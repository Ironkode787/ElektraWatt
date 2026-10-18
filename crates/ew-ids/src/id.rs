// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Id<T>`: a typed UUID.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// A UUID that names a record of kind `T` (ADR-0004). Every reference in the model is an `Id<T>`.
///
/// `T` is a marker only: the type holds a [`Uuid`] and a `PhantomData<fn() -> T>`, so it is `Copy`,
/// `Send` and `Sync` for every `T`, and all its traits are implemented without bounds on `T`.
///
/// - **Text and JSON:** the canonical lower-case hyphenated form (doc 05 §3.2). [`FromStr`] and
///   deserialisation accept only that form; [`parse_lenient`](Id::parse_lenient) accepts the others.
/// - **Order:** `Ord` compares the 16 bytes (the natural UUID order). File line order is a different key,
///   [`line_order_key`](Id::line_order_key).
/// - **Storage keys:** [`shard`](Id::shard) (byte 15) and [`sort_key`](Id::sort_key) (bytes 8–15) use the
///   random bits of both UUIDv7 and UUIDv5.
///
/// ```
/// use ew_ids::Id;
///
/// struct Device;
/// let id: Id<Device> = "c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2".parse().unwrap();
/// assert_eq!(id.shard(), 0xc2);
/// assert_eq!(id.shard_name(), "c2");
/// assert_eq!(id.sort_key(), 0x8b03_9d2e_6f1a_47c2);
/// assert!("C7A90E35-42D8-5F61-8B03-9D2E6F1A47C2".parse::<Id<Device>>().is_err());
/// assert_eq!(Id::<Device>::parse_lenient("C7A90E35-42D8-5F61-8B03-9D2E6F1A47C2"), Ok(id));
/// ```
pub struct Id<T> {
    uuid: Uuid,
    kind: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// The ID with this UUID.
    pub const fn from_uuid(uuid: Uuid) -> Id<T> {
        Id {
            uuid,
            kind: PhantomData,
        }
    }

    /// The UUID.
    pub const fn as_uuid(&self) -> Uuid {
        self.uuid
    }

    /// The same UUID typed as another kind.
    ///
    /// An explicit escape hatch, for importers and generic code that must re-type an ID they know to be
    /// of kind `U`. Model code should not need it.
    pub const fn cast<U>(self) -> Id<U> {
        Id {
            uuid: self.uuid,
            kind: PhantomData,
        }
    }

    /// The shard of this ID: byte 15, the last byte (doc 05 §2.2). Random under both UUIDv7 and UUIDv5.
    pub const fn shard(&self) -> u8 {
        self.uuid.as_bytes()[15]
    }

    /// The shard as two lower-case hex digits, as in the file name `devices/<xx>.jsonl`.
    pub fn shard_name(&self) -> String {
        format!("{:02x}", self.shard())
    }

    /// Bytes 8–15 read as a big-endian `u64`: the random bits that order lines within a kind (doc 05
    /// §3.3). For UUIDv7 these are the variant bits and the 62-bit `rand_b`.
    pub const fn sort_key(&self) -> u64 {
        let b = self.uuid.as_bytes();
        u64::from_be_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]])
    }

    /// The line-order key within a kind (doc 05 §3.3): [`sort_key`](Id::sort_key), then the full UUID.
    pub const fn line_order_key(&self) -> (u64, Uuid) {
        (self.sort_key(), self.uuid)
    }

    /// Parses any UUID spelling the `uuid` crate accepts: hyphenated or simple, upper or lower case,
    /// braced (`{…}`) or URN (`urn:uuid:…`). For importers; files use [`FromStr`].
    ///
    /// # Errors
    ///
    /// [`ParseIdError`] if the text is not a UUID in any of these forms.
    pub fn parse_lenient(s: &str) -> Result<Id<T>, ParseIdError> {
        Uuid::try_parse(s).map(Id::from_uuid).map_err(|_| ParseIdError)
    }
}

/// Whether `s` is a canonical UUID: 36 characters, lower-case hex digits, hyphens at 8, 13, 18 and 23.
fn is_canonical(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_digit() || (b'a'..=b'f').contains(&c),
        })
}

/// A string that is not a UUID in the accepted form.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ParseIdError;

impl fmt::Display for ParseIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a canonical lower-case hyphenated UUID")
    }
}

impl std::error::Error for ParseIdError {}

impl<T> FromStr for Id<T> {
    type Err = ParseIdError;

    /// Strict parsing: only the canonical lower-case hyphenated form (doc 05 §3.2).
    fn from_str(s: &str) -> Result<Id<T>, ParseIdError> {
        if !is_canonical(s) {
            return Err(ParseIdError);
        }
        Uuid::try_parse(s).map(Id::from_uuid).map_err(|_| ParseIdError)
    }
}

impl<T> fmt::Display for Id<T> {
    /// The canonical lower-case hyphenated form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.uuid.hyphenated(), f)
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.uuid.hyphenated())
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Id<T> {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Id<T>) -> bool {
        self.uuid == other.uuid
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Id<T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Id<T>) -> Ordering {
        self.uuid.cmp(&other.uuid)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}

impl<T> From<Id<T>> for Uuid {
    fn from(id: Id<T>) -> Uuid {
        id.uuid
    }
}

impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut buf = [0u8; uuid::fmt::Hyphenated::LENGTH];
        serializer.serialize_str(self.uuid.hyphenated().encode_lower(&mut buf))
    }
}

impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(IdVisitor(PhantomData))
    }
}

struct IdVisitor<T>(PhantomData<fn() -> T>);

impl<T> Visitor<'_> for IdVisitor<T> {
    type Value = Id<T>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a canonical lower-case hyphenated UUID string")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Id<T>, E> {
        v.parse().map_err(E::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    struct Device;
    struct Page;

    const TEXT: &str = "c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2";

    fn id() -> Id<Device> {
        TEXT.parse().unwrap()
    }

    #[test]
    fn strict_parse_and_display() {
        assert_eq!(id().to_string(), TEXT);
        assert_eq!(format!("{:?}", id()), format!("Id({TEXT})"));
        for bad in [
            "C7A90E35-42D8-5F61-8B03-9D2E6F1A47C2",
            "c7a90e35-42d8-5f61-8b03-9D2e6f1a47c2",
            "c7a90e3542d85f618b039d2e6f1a47c2",
            "{c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2}",
            "urn:uuid:c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2",
            " c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2",
            "c7a90e35-42d8-5f61-8b03-9d2e6f1a47c",
            "c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2a",
            "c7a90e35_42d8-5f61-8b03-9d2e6f1a47c2",
            "g7a90e35-42d8-5f61-8b03-9d2e6f1a47c2",
            "",
        ] {
            assert_eq!(bad.parse::<Id<Device>>(), Err(ParseIdError), "{bad}");
        }
    }

    #[test]
    fn lenient_parse() {
        for ok in [
            TEXT,
            "C7A90E35-42D8-5F61-8B03-9D2E6F1A47C2",
            "c7a90e3542d85f618b039d2e6f1a47c2",
            "{c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2}",
            "urn:uuid:c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2",
        ] {
            assert_eq!(Id::<Device>::parse_lenient(ok), Ok(id()), "{ok}");
        }
        assert_eq!(Id::<Device>::parse_lenient("not a uuid"), Err(ParseIdError));
    }

    #[test]
    fn storage_keys() {
        let i = id();
        assert_eq!(i.shard(), 0xc2);
        assert_eq!(i.shard_name(), "c2");
        assert_eq!(i.sort_key(), 0x8b03_9d2e_6f1a_47c2);
        assert_eq!(i.line_order_key(), (0x8b03_9d2e_6f1a_47c2, i.as_uuid()));
        let low: Id<Device> = "00000000-0000-7000-8000-000000000005".parse().unwrap();
        assert_eq!(low.shard_name(), "05");
    }

    #[test]
    fn typing_and_traits() {
        let i = id();
        let p: Id<Page> = i.cast();
        assert_eq!(p.as_uuid(), i.as_uuid());
        assert_eq!(Id::<Device>::from_uuid(i.as_uuid()), i);
        assert_eq!(Uuid::from(i), i.as_uuid());
        // Usable in ordered sets without bounds on the marker type.
        let set: BTreeSet<Id<Device>> = [i, i].into_iter().collect();
        assert_eq!(set.len(), 1);
        fn assert_send_sync<X: Send + Sync + Copy>() {}
        assert_send_sync::<Id<std::rc::Rc<u8>>>();
    }

    #[test]
    fn json_encoding() {
        assert_eq!(serde_json::to_string(&id()).unwrap(), format!("\"{TEXT}\""));
        assert_eq!(
            serde_json::from_str::<Id<Device>>(&format!("\"{TEXT}\"")).unwrap(),
            id()
        );
        for bad in [
            "\"C7A90E35-42D8-5F61-8B03-9D2E6F1A47C2\"",
            "\"c7a90e3542d85f618b039d2e6f1a47c2\"",
            "\"{c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2}\"",
            "1",
            "null",
            "[1,2]",
        ] {
            assert!(serde_json::from_str::<Id<Device>>(bad).is_err(), "{bad}");
        }
    }

    proptest! {
        #[test]
        fn text_and_serde_round_trip(bytes in any::<[u8; 16]>()) {
            let i = Id::<Device>::from_uuid(Uuid::from_bytes(bytes));
            let s = i.to_string();
            prop_assert_eq!(s.parse::<Id<Device>>().unwrap(), i);
            let json = serde_json::to_string(&i).unwrap();
            prop_assert_eq!(json.clone(), format!("\"{s}\""));
            prop_assert_eq!(serde_json::from_str::<Id<Device>>(&json).unwrap(), i);
            prop_assert_eq!(Id::<Device>::parse_lenient(&s.to_uppercase()).unwrap(), i);
            prop_assert_eq!(i.shard(), bytes[15]);
            let key = i.sort_key().to_be_bytes();
            prop_assert_eq!(&key[..], &bytes[8..]);
        }
    }
}
