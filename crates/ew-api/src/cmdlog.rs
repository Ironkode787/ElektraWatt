// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command logs (`.ewcmd`, doc 17 §3.5): JSON Lines, one header, then one command per line, replayed by
//! [`Session::run_script`]. The golden variants (`golden/J-E-2416/variants/*.ewcmd`, doc 20 §2.2) are such
//! logs.
//!
//! ```text
//! {"$schema":"ew.cmdlog/1","label":"Golden B, step B2"}
//! {"cmd":"page.renumber","params":{"scope":{"section":{"ref":"section:1"}},"start":101,"step":1}}
//! ```
//!
//! **Header.** The first non-empty line is an object whose `$schema` is [`SCHEMA`]; `label` is a free text;
//! `base` and any other member are advisory and ignored.
//!
//! **Lines.** An object with `cmd` (a command name or alias of the registry) and `params` (an object; empty
//! when absent). No other member is allowed.
//!
//! **References.** Any JSON object `{"ref": "<kind>:<value>"}` inside `params` is replaced, immediately
//! before its line runs, by the value it names in the then-current model. v0 resolves `section:<number>` (the
//! section's ID; a reference kind of this implementation, not yet in doc 17 §3.5) and `id:<uuid>` (the text
//! itself, once a record with that ID exists). The tag and folio references of doc 17 §3.5 need the `find`
//! surface and are not resolved yet. A reference that cannot be resolved stops the replay with
//! [`CmdLogError::Reference`] (exit code 2, [`CmdLogError::exit_code`]), never as a failing command.
//!
//! **Semantics.** Each line commits as its own transaction with origin [`Origin::CmdLog`]: an interactive
//! command through `execute`, a bulk one as `preview` and `commit`, so every line is one undo step. Before the
//! first line runs, every command is looked up and checked against its exposure class (a `HumanOnly` command
//! anywhere refuses the whole log). A failing line stops the replay and leaves the earlier lines committed.

use std::fmt;

use ew_commands::Cost;
use serde_json::{Map, Value};

use ew_derive::Q;
use ew_schema::AnyId;

use crate::{AnyCommand, CmdError, Origin, Receipt, Session, Submit};

/// The `$schema` of a command log header.
pub const SCHEMA: &str = "ew.cmdlog/1";

/// A parsed command log.
#[derive(Clone, PartialEq, Debug)]
pub struct CmdLog {
    label: Option<String>,
    /// The commands with the 1-based line of the file they came from.
    lines: Vec<(usize, AnyCommand)>,
}

/// Why a command log could not be read or replayed. Line numbers are 1-based lines of the file.
#[derive(Debug)]
pub enum CmdLogError {
    /// The text is not a command log: a line that is not JSON, a missing or foreign header, a member that
    /// a line may not have.
    Malformed {
        /// The line.
        line: usize,
        /// What is wrong.
        message: String,
    },
    /// A `{"ref": …}` that cannot be resolved against the then-current model (unknown kind, no such
    /// section, no record with that ID); its line did not run, the lines before it are committed.
    Reference {
        /// The line.
        line: usize,
        /// What is wrong.
        message: String,
        /// How many lines were committed before it.
        committed: usize,
    },
    /// A line's command is unknown or its exposure class refuses command logs; nothing ran.
    Refused {
        /// The line.
        line: usize,
        /// The command pipeline's error.
        error: CmdError,
    },
    /// A line's command failed; the lines before it are committed.
    Failed {
        /// The line.
        line: usize,
        /// The command pipeline's error.
        error: CmdError,
        /// How many lines were committed before it.
        committed: usize,
    },
}

impl fmt::Display for CmdLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CmdLogError::Malformed { line, message } => write!(f, "command log, line {line}: {message}"),
            CmdLogError::Reference {
                line,
                message,
                committed,
            } => write!(
                f,
                "command log, line {line}: reference: {message} ({committed} earlier lines committed)"
            ),
            CmdLogError::Refused { line, error } => {
                write!(f, "command log, line {line}: not run: {error:?}")
            }
            CmdLogError::Failed {
                line,
                error,
                committed,
            } => write!(
                f,
                "command log, line {line}: {error:?} ({committed} earlier lines committed)"
            ),
        }
    }
}

impl std::error::Error for CmdLogError {}

impl CmdLogError {
    /// The process exit code of `ew apply` for this error (doc 17 §3.2, §3.5): 1 for a domain failure, a
    /// line that a prevent rule or the command's own domain rule refused ([`CmdError::Prevented`],
    /// [`CmdError::Rejected`]); 2 for everything that could not complete: a malformed log, an unresolvable
    /// reference, a refused command (unknown, exposure), and a failing line whose error is a usage error
    /// (invalid parameters, a missing record, a bulk command without preview) or an internal error.
    pub fn exit_code(&self) -> u8 {
        match self {
            CmdLogError::Failed {
                error: CmdError::Prevented { .. } | CmdError::Rejected { .. },
                ..
            } => 1,
            CmdLogError::Malformed { .. }
            | CmdLogError::Reference { .. }
            | CmdLogError::Refused { .. }
            | CmdLogError::Failed { .. } => 2,
        }
    }

    /// How many lines were committed before the replay stopped (0 when nothing ran).
    pub fn committed(&self) -> usize {
        match self {
            CmdLogError::Malformed { .. } | CmdLogError::Refused { .. } => 0,
            CmdLogError::Reference { committed, .. } | CmdLogError::Failed { committed, .. } => *committed,
        }
    }
}

fn malformed(line: usize, message: impl Into<String>) -> CmdLogError {
    CmdLogError::Malformed {
        line,
        message: message.into(),
    }
}

impl CmdLog {
    /// Parses a command log.
    ///
    /// # Errors
    ///
    /// [`CmdLogError::Malformed`] for the first line that is not valid.
    pub fn parse(text: &str) -> Result<CmdLog, CmdLogError> {
        let mut label = None;
        let mut header_seen = false;
        let mut lines = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            if raw.trim().is_empty() {
                continue;
            }
            let value: Value =
                serde_json::from_str(raw).map_err(|e| malformed(line, format!("not JSON: {e}")))?;
            let Value::Object(mut object) = value else {
                return Err(malformed(line, "a line is a JSON object"));
            };
            if !header_seen {
                header_seen = true;
                match object.get("$schema").and_then(Value::as_str) {
                    Some(SCHEMA) => {}
                    other => {
                        return Err(malformed(
                            line,
                            format!(
                                "the first line is the header with \"$schema\": \"{SCHEMA}\", found {other:?}"
                            ),
                        ));
                    }
                }
                label = object.get("label").and_then(Value::as_str).map(str::to_owned);
                continue;
            }
            let name = match object.remove("cmd") {
                Some(Value::String(name)) => name,
                _ => return Err(malformed(line, "`cmd` is a command name")),
            };
            let params = match object.remove("params") {
                None => Value::Object(Map::new()),
                Some(params @ Value::Object(_)) => params,
                Some(_) => return Err(malformed(line, "`params` is an object")),
            };
            if let Some(extra) = object.keys().next() {
                return Err(malformed(
                    line,
                    format!("unknown member `{extra}` (a line has `cmd` and `params`)"),
                ));
            }
            lines.push((line, AnyCommand { name, params }));
        }
        if !header_seen {
            return Err(malformed(1, "empty command log: the header line is missing"));
        }
        Ok(CmdLog { label, lines })
    }

    /// The header's `label`.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// The commands in order, as written (references unresolved).
    pub fn commands(&self) -> impl Iterator<Item = &AnyCommand> + '_ {
        self.lines.iter().map(|(_, c)| c)
    }

    /// The number of commands.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the log has no command.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

impl Session {
    /// Replays a command log (doc 17 §3.5; see the module documentation): every line is one transaction
    /// with origin [`Origin::CmdLog`] named `file`, so one [`Session::undo`] takes back one line. Returns the
    /// receipts in order.
    ///
    /// # Errors
    ///
    /// [`CmdLogError::Refused`] before anything ran (unknown command, exposure), [`CmdLogError::Reference`]
    /// or [`CmdLogError::Failed`] at a line; the earlier lines stay committed.
    pub fn run_script(&mut self, log: &CmdLog, file: &str) -> Result<Vec<Receipt>, CmdLogError> {
        let origin = Origin::CmdLog { file: file.into() };
        for (line, cmd) in &log.lines {
            let refused = |error| CmdLogError::Refused { line: *line, error };
            let Some(entry) = self.registry().get(&cmd.name) else {
                return Err(refused(CmdError::UnknownCommand {
                    name: cmd.name.clone(),
                }));
            };
            let meta = entry.meta();
            if !meta.exposure.allows(&origin) {
                return Err(refused(CmdError::Exposure {
                    command: meta.name.to_owned(),
                    exposure: meta.exposure,
                    origin: origin.clone(),
                }));
            }
        }
        let mut receipts = Vec::with_capacity(log.lines.len());
        for (line, cmd) in &log.lines {
            let params = self
                .resolve_refs(&cmd.params)
                .map_err(|message| CmdLogError::Reference {
                    line: *line,
                    message,
                    committed: receipts.len(),
                })?;
            let cmd = AnyCommand {
                name: cmd.name.clone(),
                params,
            };
            let submit = Submit::from(origin.clone());
            let bulk = self
                .registry()
                .get(&cmd.name)
                .is_some_and(|e| e.meta().cost == Cost::Bulk);
            let result = if bulk {
                self.preview_with(&cmd, submit.clone())
                    .and_then(|preview| self.commit(preview))
            } else {
                self.execute_with(&cmd, submit)
            };
            match result {
                Ok(receipt) => receipts.push(receipt),
                Err(error) => {
                    return Err(CmdLogError::Failed {
                        line: *line,
                        error,
                        committed: receipts.len(),
                    });
                }
            }
        }
        Ok(receipts)
    }

    /// `params` with every `{"ref": …}` replaced by what it names in the current model; `Err` with the
    /// reason a reference cannot be resolved.
    fn resolve_refs(&self, params: &Value) -> Result<Value, String> {
        let reference = |message: String| message;
        match params {
            Value::Object(map) if map.len() == 1 && map.contains_key("ref") => {
                let Some(text) = map["ref"].as_str() else {
                    return Err(reference("`ref` is a text `<kind>:<value>`".into()));
                };
                match text.split_once(':') {
                    Some(("id", uuid)) => {
                        let id: AnyId = serde_json::from_value(Value::String(uuid.to_owned()))
                            .map_err(|_| reference(format!("`{text}`: not a UUID")))?;
                        if !record_exists(&self.query(), id) {
                            return Err(reference(format!("`{text}`: the project has no such record")));
                        }
                        Ok(Value::String(uuid.to_owned()))
                    }
                    Some(("section", number)) => {
                        let number: u16 = number.parse().map_err(|_| {
                            reference(format!("`{text}`: a section number is a whole number"))
                        })?;
                        let q = self.query();
                        let id = q
                            .sections()
                            .iter()
                            .find(|(_, s)| s.number == number)
                            .map(|(id, _)| id)
                            .ok_or_else(|| {
                                reference(format!("`{text}`: the project has no section {number}"))
                            })?;
                        serde_json::to_value(id).map_err(|e| reference(format!("`{text}`: {e}")))
                    }
                    _ => Err(reference(format!(
                        "`{text}`: v0 resolves `section:<number>` and `id:<uuid>` only"
                    ))),
                }
            }
            Value::Object(map) => map
                .iter()
                .map(|(k, v)| Ok((k.clone(), self.resolve_refs(v)?)))
                .collect::<Result<Map<_, _>, _>>()
                .map(Value::Object),
            Value::Array(items) => items
                .iter()
                .map(|v| self.resolve_refs(v))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            other => Ok(other.clone()),
        }
    }
}

/// Whether the project holds a record with this ID: the project record or a row of any table.
fn record_exists(q: &Q, id: AnyId) -> bool {
    q.project().id.cast::<()>() == id
        || q.structure_nodes().contains(id.cast())
        || q.sections().contains(id.cast())
        || q.external_refs().contains(id.cast())
        || q.pages().contains(id.cast())
        || q.devices().contains(id.cast())
        || q.functions().contains(id.cast())
        || q.reps().contains(id.cast())
        || q.connections().contains(id.cast())
        || q.junctions().contains(id.cast())
        || q.conductors().contains(id.cast())
        || q.potential_defs().contains(id.cast())
        || q.bonds().contains(id.cast())
        || q.report_blocks().contains(id.cast())
        || q.parts().contains(id.cast())
        || q.symbols().contains(id.cast())
        || q.fndefs().contains(id.cast())
        || q.forms().contains(id.cast())
}
