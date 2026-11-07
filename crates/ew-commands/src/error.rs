// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The error of running, normalising, validating, committing, undoing and redoing a command.

use std::fmt;

use ew_model::{AnyKey, ModelError};

use crate::meta::{Exposure, Origin};
use crate::text::{Text, text};

/// Why a command was not committed. A failed command leaves the session unchanged.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CmdError {
    /// No command of this name (or alias) is registered.
    UnknownCommand {
        /// The name that was asked for.
        name: String,
    },
    /// The parameters do not satisfy the command's JSON Schema, or do not deserialise into its parameter
    /// struct.
    InvalidParams {
        /// The command's canonical name.
        command: String,
        /// The JSON Pointer of the offending value (`""` for the whole parameter object).
        path: String,
        /// What is wrong, in English.
        message: String,
        /// The same by key, which the window renders in its language (FT-1395); boxed, so that the error stays small.
        text: Box<Text>,
    },
    /// A record the command names does not exist.
    NotFound {
        /// The missing key.
        key: AnyKey,
    },
    /// The command refused the edit (a domain rule of the command itself), with the reason and remedy.
    Rejected {
        /// The command's canonical name, or the normaliser's ID.
        source: String,
        /// Cause and remedy, in English.
        reason: String,
        /// The same by key, which the window renders in its language (FT-1337); `None` for a refusal of the
        /// API's own, which has only its English text.
        text: Option<Text>,
    },
    /// The origin may not submit a command of this exposure class (doc 17 §2.3, FR-EXT-030).
    Exposure {
        /// The command's canonical name.
        command: String,
        /// Its exposure class.
        exposure: Exposure,
        /// The refused origin.
        origin: Origin,
    },
    /// A `Cost::Bulk` command was submitted through `execute`; it runs as `preview` then `commit`
    /// (ADR-0008: bulk commands always preview).
    NeedsPreview {
        /// The command's canonical name.
        command: String,
    },
    /// A prevent rule of the plugged-in hook rejected the batch (ADR-0009 decision 3, strict mode): the rule
    /// ID such as `EW-TERM-002` and the finding's message, with a proposed alternative where the rule gives
    /// one.
    Prevented {
        /// The rule ID.
        rule: String,
        /// The finding's message.
        finding: String,
        /// The records involved.
        keys: Vec<AnyKey>,
        /// The finding's message arguments, by Fluent variable name, from which the window renders the rule's message
        /// from the check catalogue in its language (FT-1488, ID-1457); empty for a rule the catalogue does not word.
        args: Box<[(&'static str, String)]>,
        /// The finding by key, for a rule of the commands' own (EW-REL-004), which the window renders in its language;
        /// `None` for a rule of the check catalogue.
        text: Option<Box<Text>>,
    },
    /// The candidate snapshot was rejected by `Model::apply_with`: a structural invariant, a forbid link, the
    /// delete-time rule EW-INV-021 (a library copy that a `LibRef` names), a prevent rule or a stale op. Beside it the
    /// error by key, its records in words, which the window renders in its language (FT-1488, ID-1457); `None` for an
    /// error only a defect reaches (a revision that does not increase, a malformed op), which stays English.
    Model(ModelError, Option<Box<Text>>),
    /// Undo or redo cannot apply: there is no step, a checkpoint blocks it, or the model changed under it.
    History {
        /// What is wrong, in English.
        reason: String,
        /// Which of the four it is, which the window words in its language (FT-1488, ID-1457).
        block: HistoryBlock,
    },
    /// A bug: normalisers did not converge within three passes, or an internal step failed.
    Internal {
        /// What went wrong.
        reason: String,
    },
}

/// Why undo or redo cannot apply ([`CmdError::History`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HistoryBlock {
    /// A non-undoable checkpoint, the command `label`, stands before the step.
    Checkpoint {
        /// The checkpoint's command name.
        label: String,
    },
    /// There is no step to undo.
    NothingToUndo,
    /// There is no step to redo.
    NothingToRedo,
    /// The model changed under the step of the command `label`.
    Changed {
        /// The step's command name.
        label: String,
    },
}

impl CmdError {
    /// A refusal by a command or normaliser: its English text is `text`'s.
    pub fn rejected(source: impl Into<String>, text: Text) -> CmdError {
        CmdError::Rejected {
            source: source.into(),
            reason: text.to_string(),
            text: Some(text),
        }
    }

    /// An internal error.
    pub fn internal(reason: impl Into<String>) -> CmdError {
        CmdError::Internal {
            reason: reason.into(),
        }
    }

    /// Invalid parameters of `command` at the JSON Pointer `path`: its English text is `text`'s.
    pub fn invalid_params(command: impl Into<String>, path: impl Into<String>, text: Text) -> CmdError {
        CmdError::InvalidParams {
            command: command.into(),
            path: path.into(),
            message: text.to_string(),
            text: Box::new(text),
        }
    }
}

impl From<ModelError> for CmdError {
    fn from(e: ModelError) -> CmdError {
        match e {
            ModelError::NotFound { key } => CmdError::NotFound { key },
            ModelError::Prevented {
                rule,
                message,
                keys,
                args,
            } => CmdError::Prevented {
                rule,
                finding: message,
                keys,
                args,
                text: None,
            },
            // FT-1488: without the model at hand, a record by its kind.
            other => {
                let text = model_error_text(&other, &|(kind, _)| crate::words::kind_words(kind));
                CmdError::Model(other, text.map(Box::new))
            }
        }
    }
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CmdError::UnknownCommand { name } => write!(f, "unknown command `{name}`"),
            CmdError::InvalidParams {
                command,
                path,
                message,
                ..
            } => write!(f, "{command}: invalid parameters at `{path}`: {message}"),
            CmdError::NotFound { key } => write!(f, "no record {}:{}", key.0, key.1),
            CmdError::Rejected { source, reason, .. } => write!(f, "{source}: {reason}"),
            CmdError::Exposure {
                command,
                exposure,
                origin,
            } => write!(
                f,
                "{command}: exposure {exposure:?} is refused for origin {}",
                origin.label()
            ),
            CmdError::NeedsPreview { command } => {
                write!(f, "{command} is a bulk command: run it with preview and commit")
            }
            CmdError::Prevented { rule, finding, .. } => write!(f, "{rule}: {finding}"),
            CmdError::Model(e, _) => write!(f, "{e}"),
            CmdError::History { reason, .. } => write!(f, "history: {reason}"),
            CmdError::Internal { reason } => write!(f, "internal error: {reason}"),
        }
    }
}

impl std::error::Error for CmdError {}

/// A model error by key, its records named by `name` (FT-1488, ID-1457): a record a forbid link still names, a stale
/// step, a duplicate, an invariant by its code; `None` for an error only a defect reaches, and for a prevent rule or a
/// missing record, which [`CmdError`] carries as their own errors.
pub(crate) fn model_error_text(
    e: &ModelError,
    name: &dyn Fn(AnyKey) -> crate::text::TextArg,
) -> Option<Text> {
    Some(match e {
        ModelError::Referenced { target, referrer, .. } => {
            text!("refusal-model-referenced", name(*target), name(*referrer))
        }
        ModelError::StaleBefore { .. } => text!("refusal-model-stale"),
        ModelError::DuplicateRecord { key } => text!("refusal-model-duplicate", name(*key)),
        ModelError::DuplicateUnknownRecord { kind, .. } => text!("refusal-model-duplicate-unknown", kind),
        ModelError::Invariant { id, keys, .. } => {
            let statement = invariant_text(id)?;
            match keys.first() {
                Some(k) => text!("refusal-model-invariant", *id, statement, name(*k)),
                None => text!("refusal-model-invariant-bare", *id, statement),
            }
        }
        ModelError::RevNotIncreasing { .. }
        | ModelError::Malformed { .. }
        | ModelError::Prevented { .. }
        | ModelError::NotFound { .. } => return None,
    })
}

/// [`CmdError::from`] with the records of a model error named in words on `tx`'s model (FT-1488): a device by its tag,
/// a page by its folio, as a command's refusal names them.
pub(crate) fn worded(e: ModelError, tx: &crate::tx::TxBuilder<'_>) -> CmdError {
    match CmdError::from(e) {
        CmdError::Model(e, _) => {
            let text = model_error_text(&e, &|k| crate::words::record(tx, k));
            CmdError::Model(e, text.map(Box::new))
        }
        other => other,
    }
}

/// The statement of the invariant `id` in words (FT-1488); `None` for an ID this build does not know.
fn invariant_text(id: &str) -> Option<Text> {
    use ew_model as i;
    Some(match id {
        i::REFS_RESOLVE => text!("refusal-inv-001"),
        i::ONE_MAIN_FUNCTION => text!("refusal-inv-002"),
        i::DEVICE_IFF_NOT_IP => text!("refusal-inv-003"),
        i::ENDPOINTS_DIFFER => text!("refusal-inv-ci-01"),
        i::REP_ONE_DEVICE => text!("refusal-inv-004"),
        i::PIN_MAP_UNIQUE => text!("refusal-inv-005"),
        i::JUNCTION_SEQUENCE => text!("refusal-inv-ci-04"),
        i::CONDUCTOR_CABLE => text!("refusal-inv-006"),
        i::TERMINAL_STRIP => text!("refusal-inv-007"),
        i::STRUCTURE_ACYCLIC => text!("refusal-inv-008"),
        i::DEVICE_ACYCLIC => text!("refusal-inv-009"),
        i::EXCLUSIVE_ASPECTS => text!("refusal-inv-010"),
        i::STRUCTURE_SAME_ASPECT => text!("refusal-inv-011"),
        i::NO_STORED_REPORT_PAGE => text!("refusal-inv-012"),
        i::IP_ONE_REPRESENTATION => text!("refusal-inv-013"),
        i::REPORT_BLOCK_FOLIO => text!("refusal-inv-014"),
        i::INHERITANCE_ACYCLIC => text!("refusal-inv-015"),
        i::ONE_OPEN_REVISION => text!("refusal-inv-016"),
        i::REVISION_LABEL_UNIQUE => text!("refusal-inv-017"),
        i::REVISION_FOUR_EYES => text!("refusal-inv-018"),
        i::REVISION_PAGE_NOTE_UNIQUE => text!("refusal-inv-019"),
        i::REVISION_IMMUTABLE => text!("refusal-inv-020"),
        i::LIB_REF_KEEP_ALIVE => text!("refusal-inv-021"),
        i::ITEM_PAGE_EXISTS => text!("refusal-inv-022"),
        i::MARKER_CONNECTIONS => text!("refusal-inv-023"),
        i::FLUID_LINES => text!("refusal-inv-024"),
        _ => return None,
    })
}

/// A parameter that does not deserialise, by key (FT-1494, ID-1457): serde's sentence, what it got and the expected
/// value in words where serde names a plain kind, any other part as serde writes it. Its English is serde's own text.
pub fn serde_error_text(e: &serde_json::Error) -> Text {
    let full = e.to_string();
    let at = format!(" at line {} column {}", e.line(), e.column());
    match full.strip_suffix(&at) {
        Some(body) if e.line() > 0 => text!(
            "refusal-serde-at",
            serde_message_text(body),
            e.line().to_string(),
            e.column().to_string()
        ),
        _ => serde_message_text(&full),
    }
}

/// [`serde_error_text`] of a message without its position.
fn serde_message_text(m: &str) -> Text {
    let pair = |rest: &str| {
        rest.rsplit_once(", expected ")
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
    };
    let named = |rest: &str, tail: &str| rest.strip_suffix(tail).map(str::to_owned);
    if let Some((got, want)) = m.strip_prefix("invalid type: ").and_then(pair) {
        return text!("refusal-serde-invalid-type", serde_got(&got), serde_want(&want));
    }
    if let Some((got, want)) = m.strip_prefix("invalid value: ").and_then(pair) {
        return text!("refusal-serde-invalid-value", serde_got(&got), serde_want(&want));
    }
    if let Some((n, want)) = m.strip_prefix("invalid length ").and_then(pair) {
        return text!("refusal-serde-invalid-length", n, serde_want(&want));
    }
    if let Some(rest) = m.strip_prefix("unknown variant `") {
        if let Some((name, want)) = rest.split_once("`, expected ") {
            return text!("refusal-serde-unknown-variant", name, want);
        }
        if let Some(name) = named(rest, "`, there are no variants") {
            return text!("refusal-serde-unknown-variant-none", name);
        }
    }
    if let Some(rest) = m.strip_prefix("unknown field `") {
        if let Some((name, want)) = rest.split_once("`, expected ") {
            return text!("refusal-serde-unknown-field", name, want);
        }
        if let Some(name) = named(rest, "`, there are no fields") {
            return text!("refusal-serde-unknown-field-none", name);
        }
    }
    if let Some(name) = m.strip_prefix("missing field `").and_then(|r| named(r, "`")) {
        return text!("refusal-serde-missing-field", name);
    }
    if let Some(name) = m.strip_prefix("duplicate field `").and_then(|r| named(r, "`")) {
        return text!("refusal-serde-duplicate-field", name);
    }
    // A type's own message (`Error::custom`) as it is.
    text!("refusal-serde-other", m)
}

/// What serde got (its `Unexpected`), in words where it is a plain kind.
fn serde_got(got: &str) -> crate::text::TextArg {
    let value = |prefix: &str| got.strip_prefix(prefix).map(str::to_owned);
    if let Some(v) = value("string ") {
        return text!("refusal-serde-got-string", v).into();
    }
    if let Some(v) = value("integer ") {
        return text!("refusal-serde-got-integer", v).into();
    }
    if let Some(v) = value("floating point ") {
        return text!("refusal-serde-got-float", v).into();
    }
    if let Some(v) = value("boolean ") {
        return text!("refusal-serde-got-boolean", v).into();
    }
    match got {
        "null" => text!("refusal-serde-got-null").into(),
        "sequence" => text!("refusal-serde-got-sequence").into(),
        "map" => text!("refusal-serde-got-map").into(),
        other => other.into(),
    }
}

/// What serde expected, in words where it is a plain kind.
fn serde_want(want: &str) -> crate::text::TextArg {
    match want {
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => {
            text!("refusal-serde-want-whole", want).into()
        }
        "f32" | "f64" => text!("refusal-serde-want-number", want).into(),
        "a string" => text!("refusal-serde-want-string").into(),
        "a boolean" => text!("refusal-serde-want-boolean").into(),
        "a sequence" => text!("refusal-serde-want-sequence").into(),
        "a map" => text!("refusal-serde-want-map").into(),
        other => other.into(),
    }
}

/// A violation of the value encodings, by key (FT-1494, ID-1457): every [`ew_schema::SchemaError`], its English the
/// error's own text.
pub fn schema_error_text(e: &ew_schema::SchemaError) -> Text {
    use ew_schema::SchemaError as E;
    match e {
        E::Float => text!("refusal-schema-float"),
        E::IntegerOutOfRange => text!("refusal-schema-integer-range"),
        E::ReversedRect => text!("refusal-schema-reversed-rect"),
        E::InvalidDate => text!("refusal-schema-date"),
        E::InvalidHash => text!("refusal-schema-hash"),
        E::ColourCount => text!("refusal-schema-colour-count"),
        E::MissingKind => text!("refusal-schema-missing-kind"),
        E::UnknownKind(k) => text!("refusal-schema-unknown-kind", k),
        E::WrongFamily(k) => text!("refusal-schema-wrong-family", k),
        E::UnsupportedVersion {
            kind,
            found,
            expected,
        } => text!(
            "refusal-schema-version",
            *kind,
            found.to_string(),
            expected.to_string()
        ),
        E::Json { message } => text!("refusal-schema-json", message),
        E::DuplicateMember { path, key } => text!("refusal-schema-duplicate-member", key, path),
        E::UnsortedList { path } => text!("refusal-schema-unsorted", path),
        E::DuplicateKey { path, key } => text!("refusal-schema-duplicate-key", key, path),
        E::MalformedVariant(m) => text!(
            "refusal-schema-malformed-variant",
            &m.variant,
            &m.path,
            &m.message
        ),
        E::Invalid { path, message } => text!("refusal-schema-invalid", message, path),
        E::CompanyCodeIsIec(code) => text!("refusal-schema-company-code-iec", code),
    }
}

/// An identifier or a multilingual text that is refused, by key (FT-1494, ID-1457): every [`ew_mltext::TextError`],
/// its English the error's own text.
pub fn ident_error_text(e: &ew_mltext::TextError) -> Text {
    use ew_mltext::TextError as E;
    match e {
        E::InvalidLangTag => text!("refusal-ident-lang-tag"),
        E::NonCanonicalLangTag => text!("refusal-ident-lang-tag-canonical"),
        E::NotNfc => text!("refusal-ident-not-nfc"),
        E::EmptyIdent => text!("refusal-ident-empty"),
        E::SurroundingWhitespace => text!("refusal-ident-whitespace"),
        E::ControlCharacter => text!("refusal-ident-control"),
        E::EmptyLanguageText => text!("refusal-ident-empty-language"),
        E::NoLanguages => text!("refusal-ident-no-languages"),
        E::UnsortedLanguages => text!("refusal-ident-unsorted-languages"),
        // A reason of a newer `ew-mltext`, as it prints.
        _ => text!("refusal-serde-other", e.to_string()),
    }
}

/// An address text (`start` false) or a module-start text (`start` true) of `profile` that does not parse, by key
/// (FT-1494, ID-1457): the profile not at hand, else the text with the profile's example. Its English is the text of
/// `ew_derive::parse_plc_address` and `ew_derive::parse_plc_start`.
pub(crate) fn plc_text_refusal(
    at_hand: &[std::sync::Arc<ew_derive::ew_plcaddr::AddressProfile>],
    profile: &str,
    typed: &str,
    start: bool,
) -> Text {
    match ew_derive::pinned_profile(at_hand, profile) {
        None if start => text!("refusal-plc-start-profile-gone", profile),
        None => text!("refusal-plc-address-profile-gone", profile),
        Some(p) if start => text!(
            "refusal-plc-start-text",
            typed,
            profile,
            ew_derive::start_example(&p, ew_derive::ew_plcaddr::Area::Q)
        ),
        Some(p) => text!("refusal-plc-address-text", typed, profile, p.example()),
    }
}

/// A carrier path that is refused, by key (FT-1494, ID-1457): every [`ew_derive::layout::PathError`], its English the
/// error's own text.
pub fn path_error_text(e: &ew_derive::layout::PathError) -> Text {
    use ew_derive::layout::PathError as E;
    match e {
        E::RailPoints(n) => text!("refusal-carrier-rail-points", n.to_string()),
        E::DuctPoints(n) => text!("refusal-carrier-duct-points", n.to_string()),
        E::ZeroLength(i) => text!("refusal-carrier-zero-length", i.to_string()),
        E::Diagonal(i) => text!("refusal-carrier-diagonal", i.to_string()),
    }
}

/// An image the asset gate refuses, by key (FT-1504, ID-1457): every [`ew_schema::AssetError`], its English the
/// error's own text, so the CLI prints what it printed.
pub fn asset_error_text(e: &ew_schema::AssetError) -> Text {
    use ew_schema::{AssetError as E, SvgHazard as H};
    match e {
        E::UnknownFormat => text!("refusal-asset-unknown-format"),
        E::TooLarge { format, len, limit } => text!(
            "refusal-asset-too-large",
            format.ext(),
            len.to_string(),
            limit.to_string()
        ),
        E::TooManyNodes => text!(
            "refusal-asset-too-many-nodes",
            ew_schema::MAX_SVG_NODES.to_string()
        ),
        E::TooManyPixels {
            format,
            width,
            height,
        } => text!(
            "refusal-asset-too-many-pixels",
            format.ext(),
            width.to_string(),
            height.to_string(),
            ew_schema::MAX_RASTER_PIXELS.to_string()
        ),
        E::Damaged(format) => text!("refusal-asset-damaged", format.ext()),
        E::Base64 => text!("refusal-asset-base64"),
        E::UnsafeSvg(h) => {
            let found = match h {
                H::NotUtf8 => text!("refusal-asset-svg-not-utf8"),
                H::DoctypeSubset => text!("refusal-asset-svg-doctype-subset"),
                H::Entity => text!("refusal-asset-svg-entity"),
                H::Script => text!("refusal-asset-svg-script"),
                H::EventAttribute => text!("refusal-asset-svg-event-attribute"),
                H::ForeignObject => text!("refusal-asset-svg-foreign-object"),
                H::ExternalReference => text!("refusal-asset-svg-external-reference"),
                H::Malformed => text!("refusal-asset-svg-malformed"),
                H::TooComplex => text!("refusal-asset-svg-too-complex"),
                H::HugeNumber => text!("refusal-asset-svg-huge-number"),
                H::Unsupported => text!("refusal-asset-svg-unsupported"),
            };
            let advice = match h {
                H::NotUtf8 | H::Malformed => text!("refusal-asset-advice-plain-utf8"),
                H::DoctypeSubset | H::Entity => text!("refusal-asset-advice-editing-data"),
                H::Script | H::EventAttribute => text!("refusal-asset-advice-no-code"),
                H::ForeignObject => text!("refusal-asset-advice-html"),
                H::ExternalReference => text!("refusal-asset-advice-self-contained"),
                H::TooComplex => text!("refusal-asset-advice-simplify"),
                H::HugeNumber => text!("refusal-asset-advice-scale"),
                H::Unsupported => text!("refusal-asset-advice-flatten"),
            };
            text!("refusal-asset-unsafe-svg", found, advice)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FT-1504: every asset refusal's keyed English is the error's own text, byte for byte.
    #[test]
    fn every_asset_refusal_keeps_its_english() {
        use ew_schema::{AssetError as E, AssetFormat as F, SvgHazard as H};
        let mut errors = vec![
            E::UnknownFormat,
            E::TooLarge {
                format: F::Png,
                len: 9,
                limit: 8,
            },
            E::TooManyNodes,
            E::TooManyPixels {
                format: F::Jpeg,
                width: 70_000,
                height: 2,
            },
            E::Damaged(F::Png),
            E::Base64,
        ];
        errors.extend(
            [
                H::NotUtf8,
                H::DoctypeSubset,
                H::Entity,
                H::Script,
                H::EventAttribute,
                H::ForeignObject,
                H::ExternalReference,
                H::Malformed,
                H::TooComplex,
                H::HugeNumber,
                H::Unsupported,
            ]
            .map(E::UnsafeSvg),
        );
        for e in &errors {
            assert_eq!(asset_error_text(e).to_string(), e.to_string());
        }
    }

    /// FT-1488: a model error carries its text by key beside it, its records by their kind where no model is at hand;
    /// its printed form is the model error's, as before; an error only a defect reaches has no text.
    #[test]
    fn a_model_error_carries_its_text_by_key() {
        use ew_schema::{AnyId, Kind};
        let id = AnyId::from_uuid(ew_ids::Uuid::nil());
        let referenced = ModelError::Referenced {
            link: "terminal.strip".into(),
            target: (Kind::Device, id),
            referrer: (Kind::Device, id),
        };
        let invariant = ModelError::Invariant {
            id: ew_model::REVISION_LABEL_UNIQUE,
            message: "m".into(),
            keys: vec![(Kind::Revision, id)],
        };
        let malformed = ModelError::Malformed {
            key: (Kind::Device, id),
            reason: "r",
        };
        let texts: Vec<(String, Option<String>)> = [referenced, invariant, malformed]
            .into_iter()
            .map(|m| {
                let printed = m.to_string();
                match CmdError::from(m) {
                    CmdError::Model(m, text) => {
                        assert_eq!(m.to_string(), printed);
                        (printed, text.map(|t| t.to_string()))
                    }
                    other => panic!("{other:?}"),
                }
            })
            .collect();
        let words: Vec<Option<&str>> = texts.iter().map(|(_, t)| t.as_deref()).collect();
        assert_eq!(
            words,
            [
                Some("a device cannot be removed: a device still names it"),
                Some("EW-INV-017: revision labels are unique (a revision)"),
                None,
            ]
        );
    }

    /// FT-1494: serde's errors keep their English byte for byte through their keys, a position included; a plain kind
    /// is worded, any other part kept as serde writes it.
    #[test]
    fn every_serde_refusal_keeps_its_english() {
        #[derive(serde::Deserialize, Debug)]
        #[serde(deny_unknown_fields)]
        #[allow(dead_code)]
        struct S {
            n: u32,
            b: bool,
            s: String,
            l: Vec<u8>,
            e: Option<E>,
        }
        #[derive(serde::Deserialize, Debug)]
        #[allow(dead_code)]
        enum E {
            A,
            B,
        }
        let cases = [
            serde_json::json!({"n": "x", "b": true, "s": "", "l": []}),
            serde_json::json!({"n": -1, "b": true, "s": "", "l": []}),
            serde_json::json!({"n": 1.5, "b": true, "s": "", "l": []}),
            serde_json::json!({"n": 1, "b": null, "s": "", "l": []}),
            serde_json::json!({"n": 1, "b": true, "s": 5, "l": []}),
            serde_json::json!({"n": 1, "b": true, "s": "", "l": {}}),
            serde_json::json!({"n": 1, "b": "yes", "s": "", "l": []}),
            serde_json::json!({"n": 1, "b": true, "s": "", "l": [], "e": "C"}),
            serde_json::json!({"n": 1, "b": true, "s": "", "l": [], "z": 1}),
            serde_json::json!({"n": 1, "b": true, "s": ""}),
            serde_json::json!([1, true]),
        ];
        let mut keys = Vec::new();
        for case in cases {
            let e = serde_json::from_value::<S>(case).unwrap_err();
            let t = serde_error_text(&e);
            assert_eq!(t.to_string(), e.to_string());
            keys.push(t.key());
        }
        let e = serde_json::from_str::<S>("{\"n\": 1, \"n\": 2}").unwrap_err();
        assert_eq!(serde_error_text(&e).to_string(), e.to_string());
        assert_eq!(serde_error_text(&e).key(), "refusal-serde-at");
        assert_eq!(
            keys,
            [
                "refusal-serde-invalid-type",
                "refusal-serde-invalid-value",
                "refusal-serde-invalid-type",
                "refusal-serde-invalid-type",
                "refusal-serde-invalid-type",
                "refusal-serde-invalid-type",
                "refusal-serde-invalid-type",
                "refusal-serde-unknown-variant",
                "refusal-serde-unknown-field",
                "refusal-serde-missing-field",
                "refusal-serde-invalid-length",
            ]
        );
    }

    /// FT-1494: every encoding error, identifier error and carrier path error keeps its English through its key.
    #[test]
    fn every_parse_refusal_keeps_its_english() {
        use ew_schema::{MalformedVariant, SchemaError as S};
        let schema = [
            S::Float,
            S::IntegerOutOfRange,
            S::ReversedRect,
            S::InvalidDate,
            S::InvalidHash,
            S::ColourCount,
            S::MissingKind,
            S::UnknownKind("x".into()),
            S::WrongFamily("x".into()),
            S::UnsupportedVersion {
                kind: "device",
                found: 9,
                expected: 1,
            },
            S::Json {
                message: "m at line 1 column 2".into(),
            },
            S::DuplicateMember {
                path: "/a".into(),
                key: "k".into(),
            },
            S::UnsortedList { path: "/a/1".into() },
            S::DuplicateKey {
                path: "/a/1".into(),
                key: "k".into(),
            },
            S::MalformedVariant(MalformedVariant {
                path: "/a".into(),
                variant: "v".into(),
                message: "m".into(),
            }),
            S::Invalid {
                path: "/a".into(),
                message: "m".into(),
            },
            S::CompanyCodeIsIec("RD".into()),
        ];
        for e in &schema {
            assert_eq!(schema_error_text(e).to_string(), e.to_string());
        }
        use ew_mltext::TextError as T;
        for e in [
            T::InvalidLangTag,
            T::NonCanonicalLangTag,
            T::NotNfc,
            T::EmptyIdent,
            T::SurroundingWhitespace,
            T::ControlCharacter,
            T::EmptyLanguageText,
            T::NoLanguages,
            T::UnsortedLanguages,
        ] {
            assert_eq!(ident_error_text(&e).to_string(), e.to_string());
        }
        use ew_derive::layout::PathError as P;
        for e in [
            P::RailPoints(3),
            P::DuctPoints(1),
            P::ZeroLength(0),
            P::Diagonal(2),
        ] {
            assert_eq!(path_error_text(&e).to_string(), e.to_string());
        }
    }

    /// FT-1494: an address or a start text that does not parse, and a profile not at hand, keep the parsers' English.
    #[test]
    fn the_plc_text_refusals_keep_their_english() {
        let at_hand = ew_derive::builtin_profiles();
        let profile = ew_derive::default_profile();
        for (profile, typed) in [(profile.as_str(), "X9"), ("nope@1", "Q1")] {
            let e = ew_derive::parse_plc_address(at_hand, profile, typed).unwrap_err();
            assert_eq!(plc_text_refusal(at_hand, profile, typed, false).to_string(), e);
            let e = ew_derive::parse_plc_start(at_hand, profile, typed).unwrap_err();
            assert_eq!(plc_text_refusal(at_hand, profile, typed, true).to_string(), e);
        }
    }
}
