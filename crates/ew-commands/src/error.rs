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
    },
    /// The candidate snapshot was rejected by `Model::apply_with`: a structural invariant, a forbid link, the
    /// delete-time rule EW-INV-021 (a library copy that a `LibRef` names), a prevent rule or a stale op.
    Model(ModelError),
    /// Undo or redo cannot apply: there is no step, a checkpoint blocks it, or the model changed under it.
    History {
        /// What is wrong.
        reason: String,
    },
    /// A bug: normalisers did not converge within three passes, or an internal step failed.
    Internal {
        /// What went wrong.
        reason: String,
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
            ModelError::Prevented { rule, message, keys } => CmdError::Prevented {
                rule,
                finding: message,
                keys,
            },
            other => CmdError::Model(other),
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
            CmdError::Model(e) => write!(f, "{e}"),
            CmdError::History { reason } => write!(f, "history: {reason}"),
            CmdError::Internal { reason } => write!(f, "internal error: {reason}"),
        }
    }
}

impl std::error::Error for CmdError {}

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
}
