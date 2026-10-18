// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command registry (doc 17 §2.1): the single list of commands with their metadata and parameter
//! schemas, and the untyped [`AnyCommand`] of scripts, plugins and MCP.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use schemars::generate::SchemaSettings;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::command::{Command, DynCommand, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::tx::TxBuilder;

type ParseFn = fn(Value) -> Result<Box<dyn DynCommand>, serde_json::Error>;

/// One registered command type.
#[derive(Clone)]
pub struct RegisteredCommand {
    meta: CommandMeta,
    schema: Value,
    parse: ParseFn,
}

impl RegisteredCommand {
    /// The metadata.
    pub fn meta(&self) -> &CommandMeta {
        &self.meta
    }

    /// The parameter schema (JSON Schema 2020-12, titled with the command name).
    pub fn schema(&self) -> &Value {
        &self.schema
    }
}

impl std::fmt::Debug for RegisteredCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredCommand")
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}

/// The registry of command types, keyed by canonical name, with aliases.
#[derive(Clone, Debug, Default)]
pub struct CommandRegistry {
    entries: BTreeMap<&'static str, RegisteredCommand>,
    aliases: BTreeMap<&'static str, &'static str>,
}

/// Whether a name has the form `[a-z_]+ "." [a-z_.]+` (doc 17 §2.1).
fn valid_name(name: &str) -> bool {
    let Some((head, tail)) = name.split_once('.') else {
        return false;
    };
    !head.is_empty()
        && head.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        && !tail.is_empty()
        && tail
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'_' || b == b'.')
}

/// The parameter schema of a command type.
fn params_schema<C: Command>() -> Value {
    let generator = SchemaSettings::draft2020_12().into_generator();
    let mut schema = generator.into_root_schema_for::<C>().to_value();
    if let Value::Object(map) = &mut schema {
        map.insert("title".into(), Value::String(C::NAME.into()));
    }
    schema
}

fn parse<C: Command>(params: Value) -> Result<Box<dyn DynCommand>, serde_json::Error> {
    let cmd: C = serde_json::from_value(params)?;
    Ok(Box::new(cmd))
}

impl CommandRegistry {
    /// An empty registry.
    pub fn new() -> CommandRegistry {
        CommandRegistry::default()
    }

    /// The registry of every built-in command ([`crate::commands::register_all`]).
    pub fn builtin() -> &'static CommandRegistry {
        static BUILTIN: OnceLock<CommandRegistry> = OnceLock::new();
        BUILTIN.get_or_init(|| {
            let mut r = CommandRegistry::new();
            crate::commands::register_all(&mut r);
            r
        })
    }

    /// Registers a command type.
    ///
    /// # Panics
    ///
    /// If `C::meta().name` differs from `C::NAME`, a name or alias is malformed, or a name or alias is
    /// taken: a registry is built once from code, so these are programming errors.
    pub fn register<C: Command>(&mut self) {
        let meta = C::meta();
        assert_eq!(meta.name, C::NAME, "CommandMeta.name must equal Command::NAME");
        for name in std::iter::once(&meta.name).chain(meta.aliases) {
            assert!(valid_name(name), "malformed command name `{name}`");
            assert!(
                !self.entries.contains_key(name) && !self.aliases.contains_key(name),
                "command name `{name}` registered twice"
            );
        }
        for alias in meta.aliases {
            self.aliases.insert(alias, meta.name);
        }
        self.entries.insert(
            meta.name,
            RegisteredCommand {
                meta,
                schema: params_schema::<C>(),
                parse: parse::<C>,
            },
        );
    }

    /// The command registered under a canonical name or an alias.
    pub fn get(&self, name: &str) -> Option<&RegisteredCommand> {
        let canonical = self.aliases.get(name).copied().unwrap_or(name);
        self.entries.get(canonical)
    }

    /// Every registered command in name order.
    pub fn commands(&self) -> impl Iterator<Item = &RegisteredCommand> + '_ {
        self.entries.values()
    }

    /// Every canonical name in order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.keys().copied()
    }

    /// Validates an untyped command against its schema and deserialises it into the typed command. The name
    /// is looked up first (an unknown name is [`CmdError::UnknownCommand`] whatever the parameters are),
    /// then the size limits of [`crate::check_limits`] are applied to the parameter document, then the
    /// schema.
    pub fn resolve(&self, cmd: &AnyCommand) -> Result<Box<dyn DynCommand>, CmdError> {
        let entry = self.get(&cmd.name).ok_or_else(|| CmdError::UnknownCommand {
            name: cmd.name.clone(),
        })?;
        let name = entry.meta.name;
        crate::jsonschema::check_limits(&cmd.params)
            .map_err(|e| CmdError::invalid_params(name, e.path, e.message))?;
        crate::jsonschema::validate(&entry.schema, &cmd.params)
            .map_err(|e| CmdError::invalid_params(name, e.path, e.message))?;
        (entry.parse)(cmd.params.clone()).map_err(|e| CmdError::invalid_params(name, "", e.to_string()))
    }

    /// Deserialises an untyped command that this registry's own typed command produced
    /// ([`AnyCommand::of`] in a [`crate::Preview`] or a history step) without validating it against the
    /// schema again: the re-run path of [`crate::Engine::commit`].
    pub(crate) fn parse_trusted(&self, cmd: &AnyCommand) -> Result<Box<dyn DynCommand>, CmdError> {
        let entry = self.get(&cmd.name).ok_or_else(|| CmdError::UnknownCommand {
            name: cmd.name.clone(),
        })?;
        let name = entry.meta.name;
        (entry.parse)(cmd.params.clone()).map_err(|e| CmdError::invalid_params(name, "", e.to_string()))
    }

    /// The snapshot files of `crates/ew-commands/schemas/`: one `<name>.schema.json` per command and an
    /// `index.json` catalogue, as `(file name, contents)` sorted by name. Canonical text: two-space
    /// indentation, keys sorted, LF and a final newline.
    pub fn schema_files(&self) -> Vec<(String, String)> {
        fn render(value: &Value) -> String {
            let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
            text.push('\n');
            text
        }
        let mut files: Vec<(String, String)> = self
            .entries
            .values()
            .map(|e| (format!("{}.schema.json", e.meta.name), render(&e.schema)))
            .collect();
        let catalogue: Vec<Value> = self
            .entries
            .values()
            .map(|e| {
                let m = &e.meta;
                json!({
                    "name": m.name,
                    "params_version": m.params_version,
                    "cost": m.cost,
                    "undoable": m.undoable,
                    "exposure": m.exposure,
                    "stability": m.stability,
                    "aliases": m.aliases,
                    "confirmation": m.confirmation,
                    "schema": format!("{}.schema.json", m.name),
                })
            })
            .collect();
        files.push(("index.json".into(), render(&json!({ "commands": catalogue }))));
        files.sort();
        files
    }
}

/// An untyped command: a name (or alias) and its parameters as JSON, from scripts, plugins, MCP and
/// command logs (doc 17 §2.1).
///
/// An [`crate::Engine`] resolves an `AnyCommand` once against its own registry (it finds the value through
/// [`DynCommand::as_any_command`]) and never validates it again. The [`DynCommand`] impl below is a
/// convenience for code outside an engine: it resolves through the built-in registry, which applies the size
/// limits and validates the parameters against the command's JSON Schema, once per [`DynCommand::cost`] and
/// once per [`DynCommand::run`] call.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AnyCommand {
    /// The command name or alias.
    pub name: String,
    /// The parameters.
    pub params: Value,
}

impl AnyCommand {
    /// The JSON form of a typed command (the journal records the canonical name).
    pub fn of(cmd: &dyn DynCommand) -> Result<AnyCommand, CmdError> {
        Ok(AnyCommand {
            name: cmd.name().into(),
            params: cmd.to_json()?,
        })
    }

    fn resolved(&self) -> Result<Box<dyn DynCommand>, CmdError> {
        CommandRegistry::builtin().resolve(self)
    }
}

/// The name reported for an [`AnyCommand`] whose name is not registered.
const UNKNOWN: &str = "unknown.command";

impl DynCommand for AnyCommand {
    fn name(&self) -> &'static str {
        CommandRegistry::builtin()
            .get(&self.name)
            .map_or(UNKNOWN, |e| e.meta.name)
    }

    fn meta(&self) -> CommandMeta {
        CommandRegistry::builtin()
            .get(&self.name)
            .map_or(CommandMeta::edit(UNKNOWN), |e| e.meta)
    }

    fn cost(&self, model: &ew_model::Model) -> Cost {
        self.resolved().map_or(Cost::Interactive, |c| c.cost(model))
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        self.resolved()?.run(tx)
    }

    fn to_json(&self) -> Result<Value, CmdError> {
        Ok(self.params.clone())
    }

    fn as_any_command(&self) -> Option<&AnyCommand> {
        Some(self)
    }
}
