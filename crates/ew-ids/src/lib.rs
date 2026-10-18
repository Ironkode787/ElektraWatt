// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed identifiers `Id<T>`: UUIDv7 for new records, UUIDv5 for instantiated, imported and generated ones,
//! plus a per-session `u32` interner.
//!
//! Implements ADR-0004 (`docs/adr/0004-identity-uuidv7-uuidv5.md`, with amendments 1–3), the identity
//! table of the domain model (§4.1) and the ID-related rules of the project format
//! (`docs/design/05-project-format-and-persistence.md` §2.2 sharding, §3.2 encoding, §3.3 line order).
//!
//! - [`Id<T>`]: a UUID typed by the record kind it names. Tags, folios and designations are properties,
//!   never identity.
//! - [`V7Generator`] mints **UUIDv7** for authored records by RFC 9562 §6.2 method 3: sub-millisecond
//!   clock bits in `rand_a`, 62 fresh random bits in `rand_b`, never a counter ([`v7`] module).
//! - The [`v5`] module computes the **UUIDv5** identities of ADR-0004: connection points, generated report
//!   pages, imported records, template functions, PLC channels, embedded library copies and committed
//!   routes.
//! - [`Interner<T>`] maps IDs to dense per-session [`Dense<T>`] indexes for hot structures.
//!
//! # What is persisted
//!
//! Only [`Id<T>`], as the canonical lower-case hyphenated UUID string (doc 05 §3.2), for example
//! `"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2"`. Deserialising is strict; [`Id::parse_lenient`] accepts the
//! other common UUID spellings for importers.
//!
//! # What is not persisted
//!
//! [`Dense<T>`] values and the [`Interner<T>`] are session-local and never written to files or exports
//! (ADR-0004), so they have no serde implementation. Computed v5 identities of connection points are
//! never stored either; only overrides that carry authored data are.
//!
//! # Features
//!
//! - `testkit`: `v7::SeededSource`, a reproducible UUIDv7 source for tests and snapshots
//!   (doc 18 §2.2). **Test-only by policy:** the release pipeline asserts that the feature is absent
//!   (CI-R-09).
//!
//! # Example
//!
//! ```
//! use ew_ids::{Id, v5};
//!
//! struct Function;
//! struct Cp;
//!
//! let f: Id<Function> = Id::new_v7();
//! assert_eq!(f.as_uuid().get_version_num(), 7);
//!
//! // Connection points are computed, never stored: v5(function, idx).
//! let cp: Id<Cp> = v5::cp_id(f, 2);
//! assert_eq!(cp, v5::cp_id(f, 2));
//! assert_ne!(cp, v5::cp_id::<_, Cp>(f, 1));
//!
//! // Files carry the canonical string.
//! let text = f.to_string();
//! assert_eq!(text.parse::<Id<Function>>().unwrap(), f);
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod id;
mod interner;
pub mod v5;
pub mod v7;

pub use id::{Id, ParseIdError};
pub use interner::{Dense, Interner};
pub use uuid::Uuid;
pub use v7::{EntropyError, IdSource, SystemSource, UnixTime, V7Generator};
