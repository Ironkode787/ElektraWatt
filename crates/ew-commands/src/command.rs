// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Command` layer of ADR-0008 (ARCH §5.1): typed commands and their object-safe companion.

use ew_model::AnyKey;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::tx::TxBuilder;

/// What a command reports besides its ops.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// The record the command is about (the new device of `device.place`, the new page of `page.create`).
    pub primary: Option<AnyKey>,
    /// A short human-readable note for the toast, if any.
    pub note: Option<String>,
}

impl Outcome {
    /// An outcome naming its primary record.
    pub fn primary(key: AnyKey) -> Outcome {
        Outcome {
            primary: Some(key),
            note: None,
        }
    }
}

/// A typed command: serialisable intent with a stable dotted name and a JSON Schema for its parameters
/// (FR-EXT-015). The command struct *is* its parameter struct.
///
/// `run` must be deterministic in (snapshot, command): new IDs come from [`TxBuilder::mint`], never from
/// a clock or a global generator (ADR-0008 consequences). Not dyn-compatible; see [`DynCommand`].
pub trait Command: Serialize + DeserializeOwned + JsonSchema + Send + Sync + 'static {
    /// The stable dotted name (`device.place`); equal to `meta().name`.
    const NAME: &'static str;

    /// The registered metadata (doc 17 §2.1).
    fn meta() -> CommandMeta;

    /// The cost of this instance; the metadata's default unless overridden.
    fn cost(&self) -> Cost {
        Self::meta().cost
    }

    /// Runs the command against the transaction builder.
    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError>;
}

/// The object-safe companion of [`Command`] (ARCH §5.1): what tools hand to the session, and what
/// [`AnyCommand`](crate::AnyCommand) becomes after its parameters were validated.
pub trait DynCommand: Send + Sync {
    /// The canonical dotted name.
    fn name(&self) -> &'static str;

    /// The registered metadata.
    fn meta(&self) -> CommandMeta;

    /// The cost of this instance.
    fn cost(&self) -> Cost;

    /// Runs the command.
    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError>;

    /// The parameters as JSON: the journal, the macro recorder and command logs.
    fn to_json(&self) -> serde_json::Value;
}

impl<C: Command> DynCommand for C {
    fn name(&self) -> &'static str {
        C::NAME
    }

    fn meta(&self) -> CommandMeta {
        C::meta()
    }

    fn cost(&self) -> Cost {
        Command::cost(self)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        Command::run(self, tx)
    }

    fn to_json(&self) -> serde_json::Value {
        // A parameter struct of plain serde types always serialises; a failure is a bug in the struct.
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}
