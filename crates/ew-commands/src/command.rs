// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Command` layer of ADR-0008 (ARCH §5.1): typed commands and their object-safe companion.

use ew_model::{AnyKey, Model};
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::registry::AnyCommand;
use crate::tx::TxBuilder;

/// What a command reports besides its ops.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    /// The record the command is about (the new device of `device.place`, the new page of `page.create`).
    pub primary: Option<AnyKey>,
    /// A short human-readable note for the toast, if any.
    pub note: Option<String>,
    /// What the command dropped or adjusted instead of refusing, for the toast (an autoconnect hint whose
    /// target vanished, doc 06 §3.2 row 6), in the order they arose.
    pub notices: Vec<String>,
}

impl Outcome {
    /// An outcome naming its primary record.
    pub fn primary(key: AnyKey) -> Outcome {
        Outcome {
            primary: Some(key),
            note: None,
            notices: Vec::new(),
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

    /// The cost of this instance on the snapshot `model` it would run on; the metadata's default unless
    /// overridden (`edit.delete` counts the records its delete plan removes, D4).
    fn cost(&self, model: &Model) -> Cost {
        let _ = model;
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

    /// The cost of this instance on the snapshot it would run on.
    fn cost(&self, model: &Model) -> Cost;

    /// Runs the command.
    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError>;

    /// The parameters as JSON: the journal, the macro recorder and command logs. A parameter value that
    /// has no JSON form (a coordinate beyond ±(2⁵³ − 1), ADR-0005) is [`CmdError::Internal`], never a
    /// silent `null`.
    fn to_json(&self) -> Result<serde_json::Value, CmdError>;

    /// The untyped command behind this object, if it is one: the engine resolves an [`AnyCommand`]
    /// against its own registry once, before it reads the cost or runs it.
    fn as_any_command(&self) -> Option<&AnyCommand> {
        None
    }
}

impl<C: Command> DynCommand for C {
    fn name(&self) -> &'static str {
        C::NAME
    }

    fn meta(&self) -> CommandMeta {
        C::meta()
    }

    fn cost(&self, model: &Model) -> Cost {
        Command::cost(self, model)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        Command::run(self, tx)
    }

    fn to_json(&self) -> Result<serde_json::Value, CmdError> {
        serde_json::to_value(self)
            .map_err(|e| CmdError::internal(format!("{}: the parameters have no JSON form ({e})", C::NAME)))
    }
}
