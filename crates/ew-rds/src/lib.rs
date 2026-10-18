// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! IEC 81346 reference-designation parser and formatter (strict and lenient), with identifier sets as data.
//!
//! Implements the reference-designation parts of the domain model (`docs/design/03-domain-model.md` §5
//! aspects and signs, §8.1 structure of a designation, §8.4 atomic node codes), architecture §3 (`ew-rds`
//! row) and ADR-0026 (`docs/adr/0026-default-device-tag-identifier-set.md`, with amendments 1–2).
//!
//! - [`parse_strict`]: the IEC 81346 grammar for typed input and for checking derived designations
//!   (EW-RDS-001). [`parse_lenient`]: the import grammar, which reports what it tolerated
//!   ([`Tolerance`]) or keeps the text as an opaque tag (FR-NUM-015).
//! - [`KnownCodes`]: node codes and aliases are **atomic tokens**. The parser first matches the longest
//!   known code or alias of the aspect and only then splits at prefix characters, so `+J-E-2416` stays one
//!   code (FR-PRJ-017, FR-PRJ-028).
//! - [`Designation::format`] writes a designation with a project's [`RdsSyntax`] (signs and sub-level
//!   separators, FR-PRJ-015, FR-PRJ-016).
//! - [`IdentifierSet`]: the shipped sets EN 81346-2:2009 (the default), IEC 81346-2:2019 and IEC 60750
//!   legacy, generated from `data/identifier-sets.toml` (FR-NUM-003); conformance verdicts for EW-RDS-003
//!   and EW-RDS-007.
//!
//! # What is persisted
//!
//! | Type | JSON (doc 05 §3.2) | Example |
//! |---|---|---|
//! | [`Aspect`] | snake_case name | `"location"` |
//! | [`LetterCode`] | 1–3 letters `A`–`Z` | `"FC"` |
//!
//! Both deserialise strictly: anything but the exact names and upper-case letter codes is rejected.
//!
//! # What is not persisted
//!
//! - [`Designation`]: designations are derived from devices and structure nodes, never stored (ADR-0001,
//!   ADR-0026), so it has no serde implementation.
//! - [`RdsSyntax`] and [`IdentifierSet`] are values of project settings whose record forms are
//!   `ew-schema` decisions; the shipped sets are referenced by key ([`EN_81346_2_2009`] …).
//! - [`KnownCodes`] is built per parse from the project's structure nodes.
//!
//! # Text
//!
//! This crate has no Unicode dependency: the strict grammar is ASCII, and known codes match exactly. Pass
//! NFC text (`ew_mltext::to_nfc`) as every `Ident` is; numbering rules (formats such as
//! `{page}{letter}{counter}`) are WP-S1.11.
//!
//! # Example
//!
//! ```
//! use ew_rds::{Aspect, IdentifierSet, KnownCodes, LetterCheck, RdsSyntax, parse_strict};
//!
//! let syntax = RdsSyntax::default();
//! let mut known = KnownCodes::new();
//! known.add_code(Aspect::Location, "J-E-2416").unwrap();
//! known.add_alias(Aspect::Location, "JE2416", "J-E-2416").unwrap();
//!
//! // The reference's "+JE2416-X1031:1" resolves through the alias, and prints canonically.
//! let mut d = parse_strict("=P01+JE2416-X1031:1", &syntax, &known).unwrap();
//! assert_eq!(d.levels(Aspect::Location)[0].code(), "J-E-2416");
//! d.resolve_aliases();
//! assert_eq!(d.format(&syntax), "=P01+J-E-2416-X1031:1");
//!
//! // The default identifier set gives an MCB the letter F.
//! let set = IdentifierSet::default_set();
//! assert!(matches!(set.check_letter("ProtectionOvercurrent", None, "F"), LetterCheck::Preferred(_)));
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod aspect;
mod designation;
mod error;
mod known;
mod letter;
mod parse;
mod sets;
mod syntax;

pub use aspect::{Aspect, UnknownAspect};
pub use designation::{Designation, Level};
pub use error::{ParseError, ParseErrorKind, RdsError};
pub use known::{KnownCodes, NodeMatch, code_has_separators, spelling_key};
pub use letter::LetterCode;
pub use parse::{LenientParse, Tolerance, parse_lenient, parse_strict};
pub use sets::{
    EN_81346_2_2009, IEC_60750_LEGACY, IEC_81346_2_2019, IdentifierSet, LetterCheck, Mapping, ReservedLetter,
};
pub use syntax::RdsSyntax;
