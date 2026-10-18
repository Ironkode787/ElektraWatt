// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Multilingual text `MlText` with neutral-xor-languages semantics, BCP 47 `LangTag`, NFC identifiers
//! `Ident`, confusable detection and natural-sort keys.
//!
//! The text value types of ADR-0005 (`docs/adr/0005-value-types-nm-dec3-mltext.md`) and the domain model
//! (`docs/design/03-domain-model.md` §3.3), with the JSON encodings of the project format
//! (`docs/design/05-project-format-and-persistence.md` §3.2). Every text field of the model is classified
//! once as translatable ([`MlText`]) or identifier ([`Ident`]); identifiers are never translated
//! (FR-I18N-002).
//!
//! # What is persisted
//!
//! | Type | JSON (doc 05 §3.2) | Example |
//! |---|---|---|
//! | [`MlText`] | string (neutral) or object keyed by sorted canonical BCP 47 tags | `"1+"`, `{"en-GB":"Power Supply 1 - Main Current"}` |
//! | [`LangTag`] | canonical tag string | `"nl-NL"` |
//! | [`Ident`] | NFC string, non-empty, without surrounding white space or control characters | `"A9F78116"` |
//!
//! Deserialisation is strict: non-NFC text, a non-canonical, unsorted or duplicate language key, an empty
//! language string or language object, and an identifier with surrounding white space are all rejected.
//! Setters and [`Ident::from_input`] normalise instead, and [`LangTag::parse_lenient`] reads EPLAN's
//! `de_DE` spelling for importers. Omitting the empty default text is the record writer's job
//! (`ew-schema`); [`MlText::is_empty`] serves its `skip_serializing_if`.
//!
//! # What is not persisted
//!
//! - [`NaturalKey`]: natural-sort keys are derived on demand ([`natural_key`]); only their order is
//!   specified.
//! - [`IdentFinding`]: hygiene findings are computed by the checks (EW-RDS-009), never stored.
//! - [`CaseSensitivity`] is the value of a project setting whose record form belongs to `ew-schema`.
//!
//! # Determinism
//!
//! Nothing here reads the OS locale (NFR-I18N-03). Case mapping is Unicode's default mapping (no Turkish
//! dotless-i rule), normalisation is NFC, and the natural order is this crate's own specified algorithm,
//! so navigators, reports and the SQL collation `ew_natural` agree on every platform. Language-sensitive
//! collation of display texts, where a locale is wanted, belongs to the query layer.
//!
//! # Example
//!
//! ```
//! use ew_mltext::{Ident, IdentFinding, LangTag, MlText, natural_cmp};
//! use std::cmp::Ordering;
//!
//! // Translatable text: neutral xor languages.
//! let en: LangTag = "en-GB".parse().unwrap();
//! let mut title = MlText::neutral("Wiring Definition");
//! title.promote_neutral(&en);
//! title.set_lang("nl-NL".parse().unwrap(), "Bedradingsdefinitie");
//! assert_eq!(title.text_for(&en), Some("Wiring Definition"));
//!
//! // Identifiers: NFC on input, flagged when confusable, sorted naturally.
//! let tag = Ident::from_input("-101F\u{410}").unwrap(); // a Cyrillic А
//! assert!(tag.findings().contains(&IdentFinding::MixedScript));
//! assert_eq!(natural_cmp("X2", "X10"), Ordering::Less);
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod confusable;
mod error;
mod ident;
mod langtag;
mod mltext;
mod natural;
mod nfc;

pub use confusable::{IdentFinding, confusable_skeleton, ident_findings};
pub use error::TextError;
pub use ident::{CaseSensitivity, Ident};
pub use langtag::LangTag;
pub use mltext::{MlText, MlTextView};
pub use natural::{NaturalKey, natural_cmp, natural_key};
pub use nfc::{is_nfc, to_nfc};
