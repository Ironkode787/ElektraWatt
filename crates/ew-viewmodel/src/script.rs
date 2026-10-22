// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The UI script and its transcript (WP-S2.01 part 1, spec section 6): the headless mode's input and output.
//!
//! **Script** (`ew.uiscript/1`, JSON Lines). The first non-empty line is the header
//! `{"$schema":"ew.uiscript/1","label":"..."}`; every further line is `{"action":"<id>","params":{...}}` with
//! `params` optional. A line has no other member, an action ID is one of the registry. The header, like that of
//! `ew.cmdlog/1`, ignores members other than `$schema` and `label`.
//!
//! **Transcript** (`ew.uitranscript/1`, JSON Lines). A header `{"$schema":"ew.uitranscript/1","version":"..."}`,
//! then one object per step: `step`, `action`, `outcome` (`done`, `refused` or `failed`), `reason` (the key of a
//! refusal or null), `message` (the text of a failure or null), `state`, and for `ui.describe` the member
//! `describe`. `state` holds, in this order: `title`, `project` (the directory name, never a path), `access`,
//! `dirty`, `page`, `page_index` (0-based), `page_count`, `can_undo`, `can_redo`, `banner`, `status`, `canvas`.
//! `canvas` (WP-S2.02 section 6.6) is `null` while no page is shown, else an object with, in this order: `folio`,
//! `zoom` (per cent rounded to 0.1), `view_nm` (`[x0, y0, x1, y1]`, the sheet rectangle the pane shows), `grid`
//! (`[num_nm, den]`, `null` when off or absent), `pointer_nm` (`[x, y]` or `null`), `hover` (the entity's UUID or
//! `null`), `backend` (the label), `statistics` (whether the overlay is shown) and `renderer` (the `canvas_backend`
//! setting, `auto` or `cpu`; WP-S2.02 part 2). Nothing in it depends on time, process or location: two runs of one
//! script on equal inputs give equal bytes.
//!
//! The core runner lives here; a [`StepExtension`] adds what needs a renderer (`view.snapshot`).

use std::fmt;
use std::io;

use serde_json::{Map, Value};

use crate::actions;
use crate::shell::{AccessMode, Outcome, ShellModel};
use crate::strings;

/// The `$schema` of a script header.
pub const SCRIPT_SCHEMA: &str = "ew.uiscript/1";

/// The `$schema` of the transcript header.
pub const TRANSCRIPT_SCHEMA: &str = "ew.uitranscript/1";

/// One step of a script.
#[derive(Clone, PartialEq, Debug)]
pub struct Step {
    /// The 1-based line of the script file.
    pub line: usize,
    /// The action ID.
    pub action: String,
    /// The parameters (an object).
    pub params: Value,
}

/// A script that cannot be run: exit code 2.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScriptError {
    /// A line that is not valid: not JSON, no header, a foreign header, a missing or unknown member.
    Malformed {
        /// The 1-based line.
        line: usize,
        /// What is wrong.
        message: String,
    },
    /// An action ID that the registry does not have.
    UnknownAction {
        /// The 1-based line.
        line: usize,
        /// The ID.
        id: String,
    },
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::Malformed { line, message } => write!(f, "script line {line}: {message}"),
            ScriptError::UnknownAction { line, id } => {
                write!(
                    f,
                    "script line {line}: {}",
                    strings::format("error-unknown-action", &[id])
                )
            }
        }
    }
}

impl std::error::Error for ScriptError {}

fn malformed(line: usize, message: impl Into<String>) -> ScriptError {
    ScriptError::Malformed {
        line,
        message: message.into(),
    }
}

/// Reads a script line by line (a file at once, a pipe as it arrives).
#[derive(Clone, Debug, Default)]
pub struct ScriptParser {
    header_seen: bool,
    line: usize,
    label: Option<String>,
}

impl ScriptParser {
    /// A parser at the start of a script.
    pub fn new() -> ScriptParser {
        ScriptParser::default()
    }

    /// The `label` of the header, once read.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Reads one line: `Ok(None)` for a blank line or the header, `Ok(Some(step))` for a step.
    ///
    /// # Errors
    ///
    /// [`ScriptError`] for a line that is not valid or names an action the registry does not have.
    pub fn feed(&mut self, raw: &str) -> Result<Option<Step>, ScriptError> {
        self.line += 1;
        let line = self.line;
        if raw.trim().is_empty() {
            return Ok(None);
        }
        let value: Value =
            serde_json::from_str(raw).map_err(|e| malformed(line, format!("not JSON: {e}")))?;
        let Value::Object(mut object) = value else {
            return Err(malformed(line, "a line is a JSON object"));
        };
        if !self.header_seen {
            self.header_seen = true;
            match object.get("$schema").and_then(Value::as_str) {
                Some(SCRIPT_SCHEMA) => {}
                other => {
                    return Err(malformed(
                        line,
                        format!(
                            "the first line is the header with \"$schema\": \"{SCRIPT_SCHEMA}\", found {other:?}"
                        ),
                    ));
                }
            }
            self.label = object.get("label").and_then(Value::as_str).map(str::to_owned);
            return Ok(None);
        }
        let action = match object.remove("action") {
            Some(Value::String(a)) => a,
            _ => return Err(malformed(line, "`action` is an action ID")),
        };
        let params = match object.remove("params") {
            None => Value::Object(Map::new()),
            Some(p @ Value::Object(_)) => p,
            Some(_) => return Err(malformed(line, "`params` is an object")),
        };
        if let Some(extra) = object.keys().next() {
            return Err(malformed(
                line,
                format!("unknown member `{extra}` (a line has `action` and `params`)"),
            ));
        }
        if actions::find(&action).is_none() {
            return Err(ScriptError::UnknownAction { line, id: action });
        }
        Ok(Some(Step { line, action, params }))
    }

    /// Whether the header has been read.
    pub fn header_seen(&self) -> bool {
        self.header_seen
    }

    /// The error of a script that ended before its header.
    pub fn finish(&self) -> Result<(), ScriptError> {
        if self.header_seen {
            Ok(())
        } else {
            Err(malformed(1, "empty script: the header line is missing"))
        }
    }
}

/// Parses a whole script; a bad line or an unknown action ID anywhere is an error before any step runs.
///
/// # Errors
///
/// The first [`ScriptError`].
pub fn parse(text: &str) -> Result<Vec<Step>, ScriptError> {
    let mut parser = ScriptParser::new();
    let mut steps = Vec::new();
    for raw in text.lines() {
        if let Some(step) = parser.feed(raw)? {
            steps.push(step);
        }
    }
    parser.finish()?;
    Ok(steps)
}

/// Runs the actions a core shell cannot (those that need a renderer), before `dispatch` sees them.
pub trait StepExtension {
    /// Handles `action` and returns its outcome, or `None` to leave it to [`ShellModel::dispatch`].
    fn handle(&mut self, shell: &mut ShellModel, action: &str, params: &Value) -> Option<Outcome>;
}

/// The extension that handles nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoExtension;

impl StepExtension for NoExtension {
    fn handle(&mut self, _: &mut ShellModel, _: &str, _: &Value) -> Option<Outcome> {
        None
    }
}

/// How a run ended.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RunEnd {
    /// Every step is done (exit code 0). `quit` says `app.quit` ended the script early.
    Completed {
        /// Whether `app.quit` stopped the script before its last step.
        quit: bool,
    },
    /// A step was refused or failed and the run stopped there (exit code 1).
    StepFailed {
        /// The 1-based number of the step.
        step: usize,
    },
    /// The script could not be read or names an unknown action (exit code 2).
    Usage(ScriptError),
}

impl RunEnd {
    /// The process exit code of the headless mode.
    pub fn exit_code(&self) -> i32 {
        match self {
            RunEnd::Completed { .. } => 0,
            RunEnd::StepFailed { .. } => 1,
            RunEnd::Usage(_) => 2,
        }
    }
}

/// `"key":value` for a JSON member.
fn member(key: &str, value: &Value) -> String {
    format!("{}:{}", Value::from(key), value)
}

fn object(members: Vec<String>) -> String {
    format!("{{{}}}", members.join(","))
}

/// The header line of a transcript.
pub fn transcript_header(version: &str) -> String {
    object(vec![
        member("$schema", &Value::from(TRANSCRIPT_SCHEMA)),
        member("version", &Value::from(version)),
    ])
}

/// The observable state of the shell as the `state` object of a transcript step, members in the order of the
/// module documentation.
pub fn state_json(shell: &ShellModel) -> String {
    let opt = |s: Option<&str>| s.map_or(Value::Null, Value::from);
    let access = match shell.access() {
        None => Value::Null,
        Some(AccessMode::Edit) => Value::from("edit"),
        Some(AccessMode::ReadOnly { .. }) => Value::from("read-only"),
    };
    let page_index = if shell.page_count() > 0 {
        Value::from(shell.page_index())
    } else {
        Value::Null
    };
    object(vec![
        member("title", &Value::from(shell.title())),
        member("project", &opt(shell.project_name())),
        member("access", &access),
        member("dirty", &Value::from(shell.is_dirty())),
        member("page", &opt(shell.current_folio())),
        member("page_index", &page_index),
        member("page_count", &Value::from(shell.page_count())),
        member("can_undo", &Value::from(shell.can_undo())),
        member("can_redo", &Value::from(shell.can_redo())),
        member("banner", &opt(shell.banner())),
        member("status", &Value::from(shell.status_fields(false))),
        format!("\"canvas\":{}", canvas_json(shell)),
    ])
}

/// The `canvas` member of the state as JSON text, members in the documented order (see the module
/// documentation).
fn canvas_json(shell: &ShellModel) -> String {
    let Some(c) = shell.canvas() else {
        return "null".to_owned();
    };
    let grid = c
        .page()
        .grid
        .filter(|_| shell.grid_on())
        .map_or(Value::Null, |g| Value::from(vec![g.num_nm, g.den]));
    let pointer = c.pointer().map_or(Value::Null, |(x, y)| Value::from(vec![x, y]));
    let hover = c
        .hover()
        .map_or(Value::Null, |(e, _)| Value::from(e.0.to_string()));
    object(vec![
        member("folio", &Value::from(c.page().folio.as_str())),
        member("zoom", &Value::from(crate::canvas::round_tenth(c.zoom() * 100.0))),
        member("view_nm", &Value::from(c.view_nm().to_vec())),
        member("grid", &grid),
        member("pointer_nm", &pointer),
        member("hover", &hover),
        member("backend", &Value::from(shell.canvas_backend().label())),
        member("statistics", &Value::from(shell.frame_stats_shown())),
        member("renderer", &Value::from(renderer_word(shell))),
    ])
}

/// The `canvas_backend` setting as the transcript and the describe text name it: `auto` or `cpu`.
pub fn renderer_word(shell: &ShellModel) -> &'static str {
    if shell.cpu_canvas_setting() { "cpu" } else { "auto" }
}

/// What one executed step produced.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StepResult {
    /// The transcript line of the step (no newline).
    pub line: String,
    /// Whether the step is done.
    pub ok: bool,
}

/// Runs step number `number` (1-based) and returns its transcript line.
pub fn run_step(
    shell: &mut ShellModel,
    ext: &mut dyn StepExtension,
    number: usize,
    step: &Step,
) -> StepResult {
    let outcome = match actions::find(&step.action).map(|m| (m.enabled)(shell)) {
        Some(Err(reason_key)) => Outcome::Refused { reason_key },
        _ => match ext.handle(shell, &step.action, &step.params) {
            Some(outcome) => outcome,
            None => shell.dispatch(&step.action, &step.params),
        },
    };
    let (word, reason, message) = match &outcome {
        Outcome::Done => ("done", Value::Null, Value::Null),
        Outcome::Refused { reason_key } => ("refused", Value::from(*reason_key), Value::Null),
        Outcome::Failed { message } => ("failed", Value::Null, Value::from(message.as_str())),
    };
    let mut members = vec![
        member("step", &Value::from(number)),
        member("action", &Value::from(step.action.as_str())),
        member("outcome", &Value::from(word)),
        member("reason", &reason),
        member("message", &message),
        format!("\"state\":{}", state_json(shell)),
    ];
    if step.action == "ui.describe" && outcome == Outcome::Done {
        members.push(member("describe", &Value::from(shell.describe())));
    }
    StepResult {
        line: object(members),
        ok: outcome == Outcome::Done,
    }
}

/// Runs a parsed script: the header, then every step until one is refused or fails, or `app.quit` is done.
///
/// `sink` receives each transcript line (no newline) as it is produced. Returns how the run ended; an I/O error
/// of the sink is returned as is.
///
/// # Errors
///
/// The sink's error.
pub fn run(
    shell: &mut ShellModel,
    ext: &mut dyn StepExtension,
    steps: &[Step],
    sink: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<RunEnd> {
    sink(&transcript_header(&shell.config().app_version.clone()))?;
    for (i, step) in steps.iter().enumerate() {
        let result = run_step(shell, ext, i + 1, step);
        sink(&result.line)?;
        if !result.ok {
            return Ok(RunEnd::StepFailed { step: i + 1 });
        }
        if shell.should_quit() {
            return Ok(RunEnd::Completed {
                quit: i + 1 < steps.len(),
            });
        }
    }
    Ok(RunEnd::Completed { quit: false })
}

/// Runs a script that arrives line by line (a pipe), one transcript line per step, as the lines come in.
///
/// A line that is not valid ends the run with [`RunEnd::Usage`]; steps before it have run.
///
/// # Errors
///
/// The sink's error or the line source's.
pub fn run_stream(
    shell: &mut ShellModel,
    ext: &mut dyn StepExtension,
    lines: &mut dyn Iterator<Item = io::Result<String>>,
    sink: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<RunEnd> {
    sink(&transcript_header(&shell.config().app_version.clone()))?;
    let mut parser = ScriptParser::new();
    let mut number = 0;
    for raw in lines {
        let raw = raw?;
        let step = match parser.feed(&raw) {
            Ok(None) => continue,
            Ok(Some(step)) => step,
            Err(e) => return Ok(RunEnd::Usage(e)),
        };
        number += 1;
        let result = run_step(shell, ext, number, &step);
        sink(&result.line)?;
        if !result.ok {
            return Ok(RunEnd::StepFailed { step: number });
        }
        if shell.should_quit() {
            return Ok(RunEnd::Completed { quit: true });
        }
    }
    Ok(match parser.finish() {
        Ok(()) => RunEnd::Completed { quit: false },
        Err(e) => RunEnd::Usage(e),
    })
}

/// The `ew.cmdlog/1` text of the committed `command.run` steps of the shell (`--record`): a header labelled
/// `recorded by elektrawatt <version>`, then one `{"cmd":..,"params":..}` line per step in commit order.
pub fn record_log(shell: &ShellModel) -> String {
    let mut out = String::new();
    out.push_str(&object(vec![
        member("$schema", &Value::from(ew_api::CMDLOG_SCHEMA)),
        member(
            "label",
            &Value::from(format!("recorded by elektrawatt {}", shell.config().app_version)),
        ),
    ]));
    out.push('\n');
    for step in shell.recorded_commands() {
        out.push_str(&object(vec![
            member("cmd", &Value::from(step.cmd.as_str())),
            member("params", &step.params),
        ]));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = r#"{"$schema":"ew.uiscript/1","label":"t"}"#;

    #[test]
    fn a_script_parses_into_steps() {
        let text = format!(
            "{HEADER}\n\n{}\n{}\n",
            r#"{"action":"file.open","params":{"dir":"x"}}"#, r#"{"action":"edit.undo"}"#
        );
        let steps = parse(&text).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].action, "file.open");
        assert_eq!(steps[0].line, 3);
        assert_eq!(steps[1].params, serde_json::json!({}));
    }

    #[test]
    fn a_missing_or_foreign_header_is_malformed() {
        assert!(matches!(
            parse(r#"{"action":"edit.undo"}"#),
            Err(ScriptError::Malformed { line: 1, .. })
        ));
        assert!(matches!(
            parse(r#"{"$schema":"ew.cmdlog/1"}"#),
            Err(ScriptError::Malformed { line: 1, .. })
        ));
        assert!(matches!(parse(""), Err(ScriptError::Malformed { .. })));
        assert!(matches!(
            parse("not json"),
            Err(ScriptError::Malformed { line: 1, .. })
        ));
    }

    #[test]
    fn unknown_members_and_unknown_actions_are_errors_before_any_step() {
        let bad_member = format!("{HEADER}\n{}", r#"{"action":"edit.undo","extra":1}"#);
        assert!(matches!(
            parse(&bad_member),
            Err(ScriptError::Malformed { line: 2, .. })
        ));
        let bad_params = format!("{HEADER}\n{}", r#"{"action":"edit.undo","params":[1]}"#);
        assert!(matches!(parse(&bad_params), Err(ScriptError::Malformed { .. })));
        let unknown = format!(
            "{HEADER}\n{}\n{}",
            r#"{"action":"edit.undo"}"#, r#"{"action":"no.such"}"#
        );
        assert_eq!(
            parse(&unknown),
            Err(ScriptError::UnknownAction {
                line: 3,
                id: "no.such".into()
            })
        );
    }

    #[test]
    fn a_refused_step_stops_the_run_with_exit_code_one() {
        let mut shell = ShellModel::for_tests();
        let text = format!(
            "{HEADER}\n{}\n{}\n",
            r#"{"action":"file.save"}"#, r#"{"action":"help.about"}"#
        );
        let steps = parse(&text).unwrap();
        let mut lines = Vec::new();
        let end = run(&mut shell, &mut NoExtension, &steps, &mut |l| {
            lines.push(l.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(end, RunEnd::StepFailed { step: 1 });
        assert_eq!(end.exit_code(), 1);
        // The header and the refused step; the second step did not run.
        assert_eq!(lines.len(), 2);
        let step: Value = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(step["outcome"], "refused");
        assert_eq!(step["reason"], "reason-no-project");
        assert_eq!(step["state"]["project"], Value::Null);
        assert_eq!(step["state"]["title"], "ElektraWatt");
    }

    #[test]
    fn the_transcript_members_are_in_the_documented_order() {
        let mut shell = ShellModel::for_tests();
        let steps = parse(&format!("{HEADER}\n{}\n", r#"{"action":"ui.describe"}"#)).unwrap();
        let mut lines = Vec::new();
        run(&mut shell, &mut NoExtension, &steps, &mut |l| {
            lines.push(l.to_owned());
            Ok(())
        })
        .unwrap();
        assert!(lines[0].starts_with(r#"{"$schema":"ew.uitranscript/1","version":""#));
        let order = [
            "\"step\"",
            "\"action\"",
            "\"outcome\"",
            "\"reason\"",
            "\"message\"",
            "\"state\"",
            "\"describe\"",
        ];
        let at: Vec<usize> = order.iter().map(|k| lines[1].find(k).unwrap()).collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{}", lines[1]);
        let state_order = [
            "\"title\"",
            "\"project\"",
            "\"access\"",
            "\"dirty\"",
            "\"page\"",
            "\"page_index\"",
            "\"page_count\"",
            "\"can_undo\"",
            "\"can_redo\"",
            "\"banner\"",
            "\"status\"",
            "\"canvas\"",
        ];
        let state = &lines[1][lines[1].find("\"state\"").unwrap()..];
        let at: Vec<usize> = state_order.iter().map(|k| state.find(k).unwrap()).collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{state}");
    }

    #[test]
    fn a_stream_runs_each_line_as_it_arrives() {
        let mut shell = ShellModel::for_tests();
        let input = [
            HEADER.to_owned(),
            r#"{"action":"help.about"}"#.to_owned(),
            r#"{"action":"no.such"}"#.to_owned(),
            r#"{"action":"ui.dismiss"}"#.to_owned(),
        ];
        let mut lines = input.into_iter().map(Ok);
        let mut out = Vec::new();
        let end = run_stream(&mut shell, &mut NoExtension, &mut lines, &mut |l| {
            out.push(l.to_owned());
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            end,
            RunEnd::Usage(ScriptError::UnknownAction { line: 3, .. })
        ));
        assert_eq!(end.exit_code(), 2);
        // The header and the one step that ran before the bad line.
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn quit_ends_the_script() {
        let mut shell = ShellModel::for_tests();
        let steps = parse(&format!(
            "{HEADER}\n{}\n{}\n",
            r#"{"action":"app.quit"}"#, r#"{"action":"help.about"}"#
        ))
        .unwrap();
        let end = run(&mut shell, &mut NoExtension, &steps, &mut |_| Ok(())).unwrap();
        assert_eq!(end, RunEnd::Completed { quit: true });
        assert_eq!(end.exit_code(), 0);
    }
}
