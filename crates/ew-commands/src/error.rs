// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The error of running, normalising, validating, committing, undoing and redoing a command.

use std::fmt;

use ew_model::{AnyKey, ModelError};

use crate::meta::{Exposure, Origin};

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
        /// What is wrong.
        message: String,
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
        /// Cause and remedy.
        reason: String,
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
    /// A refusal by a command or normaliser.
    pub fn rejected(source: impl Into<String>, reason: impl Into<String>) -> CmdError {
        CmdError::Rejected {
            source: source.into(),
            reason: reason.into(),
        }
    }

    /// An internal error.
    pub fn internal(reason: impl Into<String>) -> CmdError {
        CmdError::Internal {
            reason: reason.into(),
        }
    }

    /// Invalid parameters of `command` at the JSON Pointer `path`.
    pub fn invalid_params(
        command: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> CmdError {
        CmdError::InvalidParams {
            command: command.into(),
            path: path.into(),
            message: message.into(),
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
            } => write!(f, "{command}: invalid parameters at `{path}`: {message}"),
            CmdError::NotFound { key } => write!(f, "no record {}:{}", key.0, key.1),
            CmdError::Rejected { source, reason } => write!(f, "{source}: {reason}"),
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
