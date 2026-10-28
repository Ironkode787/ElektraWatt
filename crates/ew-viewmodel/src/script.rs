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
//! `dirty`, `page`, `page_index` (0-based), `page_count`, `can_undo`, `can_redo`, `banner`, `status`, `canvas`, `tabs`,
//! `focus`, `navigator`.
//! `canvas` (WP-S2.02 section 6.6) is `null` while no page is shown, else an object with, in this order: `folio`,
//! `zoom` (per cent rounded to 0.1), `view_nm` (`[x0, y0, x1, y1]`, the sheet rectangle the pane shows), `grid`
//! (`[num_nm, den]`, `null` when off or absent), `pointer_nm` (`[x, y]` or `null`), `hover` (the entity's alias or
//! `null`), `backend` (the label), `statistics` (whether the overlay is shown) and `renderer` (the `canvas_backend`
//! setting, `auto` or `cpu`; WP-S2.02 part 2), then (WP-S2.03 part 1) `cursor_nm` (the grid cursor, `[x, y]` or
//! `null`), `cell` (the status bar's cursor cell or `null`) and `link` (the hovered link: `{"folio", "cell"}`,
//! `{"external": true}` or `null`), then (WP-S2.03 part 2) `tool` (`{"name", "state"}`), `snap` (`{"kind", "at_nm",
//! "label"}` or `null`), `selection` (the selected entities' aliases in selection order), `preview` (`{"command",
//! "state": "ready" | "stale" | "failed", "tags", "xrefs"}` or `null`), `overlay` (the number of overlay items) and
//! `link_preview` (the link preview's target folio or `null`), then (WP-S2.04 part 1, §7.7) `ghost` (`null`, or
//! `{"kind": "place", "symbol", "variant", "rep_type", "at_nm", "target", "tag", "hints"}`, `{"kind": "move",
//! "count", "steps"}` or `{"kind": "exchange", "symbol", "unmapped"}`) and `picker` (`null` or `{"mode", "filter",
//! "rows", "row"}`; `mode` also `strip`, the strip chooser), then (WP-S2.04 part 2, §8.7) `route` (`null` or
//! `{"from", "bends", "points", "end"}`: the labels of the Wire tool's start and target, the bends fixed by clicks,
//! the vertices of the ghost) and `junction_editor` (`null` or `{"junction", "style", "wiring", "sequence"}`: the
//! junction named by its frame cell, never its UUID, and the members by their far endpoints' labels). After `canvas`
//! come `tabs` (`null` without a project, else
//! `{"focused": [g, i], "groups": [[{"id", "folio", "back", "forward"}, …], …]}`), `focus` (`navigator`, `canvas`,
//! `properties` or `null`) and `navigator` (`null`, or `{"mode", "sort", "filter", "row"}`), then (WP-S2.05 part 1)
//! `properties` (`null` without a project or while the pane is closed, else `{"title", "items", "kinds", "sections":
//! [{"kind", "targets", "rows"}, …], "missing", "row", "editing", "buffer", "error": {"row", "message"} | null,
//! "choices", "highlight" (the highlighted choice's index or null), "popup", "read_only"}`), `card` (`null` or `{"command", "state", "targets", "changes", "tags", "xrefs",
//! "origin"}`) and `toast` (`null` or `{"text", "undo"}`); then (WP-S2.05 part 2) `find`
//! (`null` while the find pane is closed, else `{"text", "replace" (the replacement, or null outside find and
//! replace), "regex", "match_case", "whole_word", "in_selection", "fields", "folios", "results" (the number of hits, or
//! null before a search), "truncated", "error", "row" (1-based, or null), "stale", "note"}`); the card's `origin` is
//! `find` for a replace, and `focus` may be `find`; then (WP-S2.05 part 3) `clipboard` (`null` while empty, else
//! `{"kind": "fragment", "records", "copies"}` or `{"kind": "text", "lines"}`), and the canvas `ghost` may be
//! `{"kind": "paste", "records", "at_nm", "dropped"}`. (WP-S2.04 part 4b) the `ghost` may also be
//! `{"kind": "text", …}`, `{"kind": "graphic", …}`, `{"kind": "marker", "connections", "text"}` or
//! `{"kind": "structure_box", "outline"}`, and (part 5) `{"kind": "image", "asset"}`. Nothing in it depends on time, process or location: two runs of one
//! script on equal inputs give equal bytes.
//!
//! **Record IDs** (decision log ID-630, FT-422). No UUID reaches a transcript line: every UUID of a step's line
//! (`state`, `message`, `describe`, a buffer or an error) is printed as its alias `@n` ([`crate::alias`]), numbered in
//! the order of first appearance in this run's output; a run starts a new table, which lasts across every project the
//! run opens or creates. A script may write `@n` wherever an action parameter takes a UUID or a reference
//! ([`crate::alias::ALIAS_PARAMS`]; `id:@n` in a reference or a typed link value); `--record` writes the resolved UUID.
//!
//! The core runner lives here; a [`StepExtension`] adds what needs a renderer (`view.snapshot`).

use std::fmt;
use std::io;

use serde_json::{Map, Value};

use crate::actions;
use crate::shell::{AccessMode, Focus, Outcome, ShellModel};
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
    /// The script ran to its end, or `app.quit` ended it (exit code 0 when `not_done` is 0, else 1). Without
    /// `--keep-going` a step that is not done ends the run as [`RunEnd::StepFailed`], so `not_done` is 0 here.
    Completed {
        /// Whether `app.quit` stopped the script before its last step.
        quit: bool,
        /// The steps that were refused or failed while `--keep-going` let the run go on (WP-S6.13 §5.5.1).
        not_done: usize,
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
            RunEnd::Completed { not_done: 0, .. } => 0,
            RunEnd::Completed { .. } | RunEnd::StepFailed { .. } => 1,
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
        format!("\"tabs\":{}", tabs_json(shell)),
        member(
            "focus",
            &match shell.focus() {
                None => Value::Null,
                // WP-S2.06 part 1: `navigator` for Pages, `navigator.<kind>` for the others (§6.8).
                Some(Focus::Navigator(crate::navigators::NavKind::Pages)) => Value::from("navigator"),
                Some(Focus::Navigator(k)) => Value::from(format!("navigator.{}", k.word())),
                Some(Focus::Canvas(_)) => Value::from("canvas"),
                Some(Focus::Properties) => Value::from("properties"),
                // WP-S2.05 part 2
                Some(Focus::Find) => Value::from("find"),
            },
        ),
        format!("\"navigator\":{}", navigator_json(shell)),
        format!("\"properties\":{}", properties_json(shell)),
        format!("\"card\":{}", card_json(shell)),
        format!("\"toast\":{}", toast_json(shell)),
        // WP-S2.05 part 2
        format!("\"find\":{}", find_json(shell)),
        // WP-S2.05 part 3
        format!("\"clipboard\":{}", clipboard_json(shell)),
    ])
}

// WP-S2.05 part 3
/// The `clipboard` member (§8.8): `null` while empty, `{"kind": "fragment", "records", "copies"}` or `{"kind": "text",
/// "lines"}`. Never the text: a fragment holds real record IDs (data, not machine text).
fn clipboard_json(shell: &ShellModel) -> String {
    use crate::clipboard::ClipKind;
    match shell.clipboard().kind() {
        None => "null".to_owned(),
        Some(ClipKind::Fragment { records, copies }) => object(vec![
            member("kind", &Value::from("fragment")),
            member("records", &Value::from(*records)),
            member("copies", &Value::from(*copies)),
        ]),
        Some(ClipKind::Text { lines }) => object(vec![
            member("kind", &Value::from("text")),
            member("lines", &Value::from(*lines)),
        ]),
    }
}

// WP-S2.05 part 2
/// The `find` member (§7.6): `null` while the find pane is closed.
fn find_json(shell: &ShellModel) -> String {
    if !shell.find_shown() {
        return "null".to_owned();
    }
    let f = shell.find_state();
    let (regex, match_case, whole_word, in_selection) = f.options();
    let results = f.results();
    let words: Vec<Value> = f
        .fields()
        .iter()
        .map(|x| Value::from(crate::find::field_word(*x)))
        .collect();
    let folios: Vec<Value> = f.folios().iter().map(|x| Value::from(x.as_str())).collect();
    let opt = |s: Option<&str>| s.map_or(Value::Null, Value::from);
    object(vec![
        member("text", &Value::from(f.text())),
        member(
            "replace",
            &if f.replace_mode() {
                Value::from(f.replacement())
            } else {
                Value::Null
            },
        ),
        member("regex", &Value::from(regex)),
        member("match_case", &Value::from(match_case)),
        member("whole_word", &Value::from(whole_word)),
        member("in_selection", &Value::from(in_selection)),
        member("fields", &Value::from(words)),
        member("folios", &Value::from(folios)),
        member(
            "results",
            &results.map_or(Value::Null, |r| Value::from(r.hits.len())),
        ),
        member("truncated", &Value::from(results.is_some_and(|r| r.truncated))),
        member("error", &opt(results.and_then(|r| r.error.as_deref()))),
        member("row", &f.row().map_or(Value::Null, |r| Value::from(r + 1))),
        member("stale", &Value::from(f.stale(shell.revision()))),
        member("note", &opt(f.note())),
    ])
}

/// The `properties` member (WP-S2.05 §6.11): `null` without a project or while the pane is closed.
fn properties_json(shell: &ShellModel) -> String {
    let props = shell.properties();
    let (true, Some(sheet)) = (shell.properties_shown(), props.sheet()) else {
        return "null".to_owned();
    };
    let kinds: Vec<String> = sheet
        .header
        .kinds
        .iter()
        .map(|(k, n)| member(k, &Value::from(*n)))
        .collect();
    let sections: Vec<String> = sheet
        .sections
        .iter()
        .map(|s| {
            object(vec![
                member("kind", &Value::from(s.kind.as_str())),
                member("targets", &Value::from(s.targets.len())),
                member("rows", &Value::from(s.rows.len())),
            ])
        })
        .collect();
    let opt = |s: Option<&str>| s.map_or(Value::Null, Value::from);
    let error = match props.error() {
        None => "null".to_owned(),
        Some((row, message)) => object(vec![
            member("row", &Value::from(row)),
            member("message", &Value::from(message)),
        ]),
    };
    object(vec![
        member("title", &Value::from(sheet.header.title.as_str())),
        member("items", &Value::from(sheet.header.items)),
        format!("\"kinds\":{}", object(kinds)),
        format!("\"sections\":[{}]", sections.join(",")),
        member("missing", &Value::from(sheet.missing)),
        member("row", &opt(shell.properties_row().as_deref())),
        member("editing", &opt(props.edit().map(|e| e.row.as_str()))),
        member(
            "buffer",
            &props
                .edit()
                .map_or(Value::Null, |e| Value::from(e.buffer.as_str())),
        ),
        format!("\"error\":{error}"),
        member(
            "choices",
            &Value::from(props.choices().map_or(0, |c| c.items.len())),
        ),
        member(
            "highlight",
            &props
                .choices()
                .and_then(|c| c.index)
                .map_or(Value::Null, Value::from),
        ),
        member("popup", &opt(props.popup().map(crate::properties::Popup::word))),
        member("read_only", &opt(sheet.read_only.as_deref())),
    ])
}

/// The `card` member: the inline preview card or `null`.
fn card_json(shell: &ShellModel) -> String {
    let (Some(card), Some(rev)) = (shell.properties().card(), shell.revision()) else {
        return "null".to_owned();
    };
    object(vec![
        member("command", &Value::from(card.preview.name.as_str())),
        member("state", &Value::from(card.preview.status(rev).word())),
        member("targets", &Value::from(card.targets)),
        member("changes", &Value::from(card.changes)),
        member("tags", &Value::from(card.preview.derived.tags.len())),
        member("xrefs", &Value::from(card.preview.derived.xrefs.len())),
        member("origin", &Value::from(card.origin.word())),
    ])
}

/// The `toast` member: the newest visible toast or `null`; `undo` says whether it offers Undo (WP-S2.09: the describe
/// text lists every visible toast with its action's validity).
fn toast_json(shell: &ShellModel) -> String {
    match shell.toast() {
        None => "null".to_owned(),
        Some(t) => object(vec![
            member("text", &Value::from(t.text.as_str())),
            member(
                "undo",
                &Value::from(t.action() == Some(crate::notifications::NoteAction::Undo)),
            ),
        ]),
    }
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
    let cursor = c.cursor().map_or(Value::Null, |(x, y)| Value::from(vec![x, y]));
    let cell = shell
        .shown_point()
        .and_then(|p| c.page().cell_text(p))
        .map_or(Value::Null, Value::from);
    let link = match c.hovered_link() {
        None => "null".to_owned(),
        Some(l) => match &l.target {
            None => object(vec![member("external", &Value::from(true))]),
            Some(t) => object(vec![
                member("folio", &Value::from(t.folio.as_str())),
                member("cell", &t.cell.as_deref().map_or(Value::Null, Value::from)),
            ]),
        },
    };
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
        member("cursor_nm", &cursor),
        member("cell", &cell),
        format!("\"link\":{link}"),
        format!("\"tool\":{}", tool_json(shell)),
        format!("\"snap\":{}", snap_json(shell)),
        member(
            "selection",
            &Value::from(
                shell
                    .selection()
                    .items
                    .iter()
                    .map(|e| e.0.to_string())
                    .collect::<Vec<_>>(),
            ),
        ),
        format!("\"preview\":{}", preview_json(shell)),
        member("overlay", &Value::from(shell.tool_overlay().len())),
        member(
            "link_preview",
            &shell
                .link_preview()
                .map_or(Value::Null, |lp| Value::from(lp.target.folio.as_str())),
        ),
        format!("\"ghost\":{}", ghost_json(shell)),
        format!("\"picker\":{}", picker_json(shell)),
        // WP-S2.04 part 2.
        format!("\"route\":{}", route_json(shell)),
        format!("\"junction_editor\":{}", junction_editor_json(shell)),
    ])
}

/// The `route` member of the canvas (WP-S2.04 §8.7): `null`, or `{"from", "bends", "points", "end"}`.
fn route_json(shell: &ShellModel) -> String {
    match shell.tool().route_info() {
        None => "null".to_owned(),
        Some(r) => object(vec![
            member("from", &Value::from(r.from)),
            member("bends", &Value::from(r.bends)),
            member("points", &Value::from(r.points)),
            member("end", &r.end.map_or(Value::Null, Value::from)),
        ]),
    }
}

/// The `junction_editor` member of the canvas (WP-S2.04 §8.7): `null`, or `{"junction" (its frame cell), "style",
/// "wiring", "sequence"}`.
fn junction_editor_json(shell: &ShellModel) -> String {
    match shell.junction_editor() {
        None => "null".to_owned(),
        Some(e) => object(vec![
            member("junction", &Value::from(e.cell.as_str())),
            member("style", &Value::from(e.style.as_str())),
            member("wiring", &Value::from(e.wiring.as_str())),
            member("sequence", &Value::from(e.sequence_labels())),
        ]),
    }
}

/// The `ghost` member of the canvas (WP-S2.04 §7.7): `null`, or `{"kind": "place", "symbol", "variant",
/// "rep_type", "at_nm", "target", "tag", "hints"}`, `{"kind": "move", "count", "steps"}` or `{"kind": "exchange",
/// "symbol", "unmapped"}`, and the part 4b forms (`text`, `graphic`, `marker`, `structure_box`) and part 5's `image`.
fn ghost_json(shell: &ShellModel) -> String {
    use crate::tools::Ghost;
    match shell.tool_ghost() {
        None => "null".to_owned(),
        Some(Ghost::Place {
            symbol,
            variant,
            rep_type,
            at_nm,
            target,
            tag,
            hints,
            ..
        }) => object(vec![
            member("kind", &Value::from("place")),
            member("symbol", &Value::from(symbol)),
            member("variant", &Value::from(variant.to_string())),
            member("rep_type", &Value::from(rep_type)),
            member(
                "at_nm",
                &at_nm.map_or(Value::Null, |(x, y)| Value::from(vec![x, y])),
            ),
            member("target", &Value::from(target)),
            member("tag", &tag.map_or(Value::Null, Value::from)),
            member("hints", &Value::from(hints)),
        ]),
        Some(Ghost::Move { count, steps, .. }) => object(vec![
            member("kind", &Value::from("move")),
            member("count", &Value::from(count)),
            member("steps", &Value::from(vec![steps.0, steps.1])),
        ]),
        // WP-S2.05 part 3
        Some(Ghost::Paste {
            records,
            at_nm,
            dropped,
        }) => object(vec![
            member("kind", &Value::from("paste")),
            member("records", &Value::from(records)),
            member(
                "at_nm",
                &at_nm.map_or(Value::Null, |(x, y)| Value::from(vec![x, y])),
            ),
            member("dropped", &Value::from(dropped)),
        ]),
        Some(Ghost::Exchange { symbol, unmapped }) => object(vec![
            member("kind", &Value::from("exchange")),
            member("symbol", &Value::from(symbol)),
            member("unmapped", &Value::from(unmapped)),
        ]),
        // WP-S2.04 part 3 (§9.6).
        Some(Ghost::Ip {
            name,
            direction,
            star,
            at_nm,
            external,
        }) => object(vec![
            member("kind", &Value::from("ip")),
            member("name", &Value::from(name)),
            member("direction", &Value::from(direction)),
            member("star", &Value::from(star)),
            member(
                "at_nm",
                &at_nm.map_or(Value::Null, |(x, y)| Value::from(vec![x, y])),
            ),
            // The external reference's printed text, as the describe line prints it (fix round, item 9).
            member("external", &external.map_or(Value::Null, Value::from)),
        ]),
        Some(Ghost::Box { outline, cps }) => object(vec![
            member("kind", &Value::from("box")),
            member("outline", &Value::from(outline)),
            member("cps", &Value::from(cps)),
        ]),
        // WP-S2.04 part 4b (§10.7).
        Some(Ghost::Text { at_nm, text, bind }) => object(vec![
            member("kind", &Value::from("text")),
            member(
                "at_nm",
                &at_nm.map_or(Value::Null, |(x, y)| Value::from(vec![x, y])),
            ),
            member("text", &Value::from(text)),
            member("bind", &bind.map_or(Value::Null, Value::from)),
        ]),
        Some(Ghost::Graphic {
            shape,
            points,
            anticlockwise,
        }) => object(vec![
            member("kind", &Value::from("graphic")),
            member("shape", &Value::from(shape)),
            member("points", &Value::from(points)),
            member("anticlockwise", &Value::from(anticlockwise)),
        ]),
        // WP-S2.04 part 4b: the Wire marker tool's group and the Structure box tool's outline.
        Some(Ghost::Marker { connections, text }) => object(vec![
            member("kind", &Value::from("marker")),
            member("connections", &Value::from(connections)),
            member("text", &text.map_or(Value::Null, Value::from)),
        ]),
        Some(Ghost::StructureBox { outline }) => object(vec![
            member("kind", &Value::from("structure_box")),
            member("outline", &Value::from(outline)),
        ]),
        // WP-S2.04 part 5 (§11).
        Some(Ghost::Image { asset }) => object(vec![
            member("kind", &Value::from("image")),
            member("asset", &asset.map_or(Value::Null, Value::from)),
        ]),
    }
}

/// The `picker` member of the canvas (WP-S2.04 §7.7): `null`, or `{"mode", "filter", "rows", "row"}`.
fn picker_json(shell: &ShellModel) -> String {
    match shell.picker() {
        None => "null".to_owned(),
        Some(p) => object(vec![
            member("mode", &Value::from(p.mode.word())),
            member("filter", &Value::from(p.filter.as_str())),
            member("rows", &Value::from(p.rows.len())),
            member("row", &Value::from(p.row)),
        ]),
    }
}

/// The `tool` member of the canvas: `{"name", "state"}`.
fn tool_json(shell: &ShellModel) -> String {
    let tool = shell.tool();
    object(vec![
        member("name", &Value::from(tool.name())),
        member("state", &Value::from(tool.state())),
    ])
}

/// The `snap` member of the canvas: the snap indicator `{"kind", "at_nm", "label"}` or `null`.
fn snap_json(shell: &ShellModel) -> String {
    match shell.snap_indicator() {
        None => "null".to_owned(),
        Some(s) => object(vec![
            member("kind", &Value::from(s.kind.word())),
            member("at_nm", &Value::from(vec![s.at_nm.0, s.at_nm.1])),
            member("label", &s.label.as_deref().map_or(Value::Null, Value::from)),
        ]),
    }
}

/// The `preview` member of the canvas: `{"command", "state", "tags", "xrefs"}` or `null`.
fn preview_json(shell: &ShellModel) -> String {
    match (shell.preview_state(), shell.preview_status()) {
        (Some(p), Some(status)) => object(vec![
            member("command", &Value::from(p.name.as_str())),
            member("state", &Value::from(status.word())),
            member("tags", &Value::from(p.derived.tags.len())),
            member("xrefs", &Value::from(p.derived.xrefs.len())),
        ]),
        _ => "null".to_owned(),
    }
}

/// The `tabs` member as JSON text: `null` without a project, else `{"focused": [g, i], "groups": [[{"id", "folio",
/// "back", "forward"}, …], …]}` with the members in this order.
fn tabs_json(shell: &ShellModel) -> String {
    let Some(area) = shell.area() else {
        return "null".to_owned();
    };
    let groups: Vec<String> = area
        .groups
        .iter()
        .map(|g| {
            let tabs: Vec<String> = g
                .tabs
                .iter()
                .map(|t| {
                    object(vec![
                        member("id", &Value::from(t.id)),
                        member(
                            "folio",
                            &shell
                                .tab_row(t)
                                .map_or(Value::Null, |r| Value::from(r.folio.as_str())),
                        ),
                        member("back", &Value::from(t.history.back_len())),
                        member("forward", &Value::from(t.history.forward_len())),
                    ])
                })
                .collect();
            format!("[{}]", tabs.join(","))
        })
        .collect();
    object(vec![
        member(
            "focused",
            &Value::from(vec![area.focused, area.focused_group().active]),
        ),
        format!("\"groups\":[{}]", groups.join(",")),
    ])
}

/// The `navigator` member as JSON text: `null` without a project, else `{"mode", "sort", "filter", "row"}`; `row` is
/// the key the row cursor was moved to, `null` while it follows the current page.
fn navigator_json(shell: &ShellModel) -> String {
    if !shell.has_project() {
        return "null".to_owned();
    }
    let nav = shell.navigator();
    let row = nav
        .moved_row(&shell.navigator_rows())
        .map_or(Value::Null, Value::from);
    object(vec![
        member("mode", &Value::from(nav.mode.word())),
        member("sort", &Value::from(nav.sort.word())),
        member("filter", &Value::from(nav.filter.as_str())),
        member("row", &row),
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
    // WP-S2.09 §6.3 item 5: the ends of the jobs that finished since the last step are taken first.
    shell.poll_jobs();
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
        members.push(member("describe", &Value::from(crate::describe::describe(shell))));
    }
    // WP-S6.13 part 1: `page.describe` carries the page's text in the same member.
    if step.action == "page.describe" {
        let text = shell.take_page_text();
        if let (Outcome::Done, Some(text)) = (&outcome, text) {
            members.push(member("describe", &Value::from(text)));
        }
    }
    // Every UUID of the line becomes its alias `@n`, numbered in the order of first appearance in the output (FT-422).
    StepResult {
        line: shell.alias_text(&object(members)),
        ok: outcome == Outcome::Done,
    }
}

/// Runs a parsed script: the header, then every step until one is refused or fails, or `app.quit` is done.
///
/// With `keep_going` (`--keep-going`, WP-S6.13 §5.5.1) a step that is not done is written as before and the run goes
/// on; the count of such steps is [`RunEnd::Completed`]'s `not_done`. Without it the first such step ends the run.
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
    keep_going: bool,
    sink: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<RunEnd> {
    shell.reset_aliases();
    sink(&transcript_header(&shell.config().app_version.clone()))?;
    let mut not_done = 0;
    for (i, step) in steps.iter().enumerate() {
        let result = run_step(shell, ext, i + 1, step);
        sink(&result.line)?;
        if !result.ok {
            if !keep_going {
                return Ok(RunEnd::StepFailed { step: i + 1 });
            }
            not_done += 1;
        }
        if shell.should_quit() {
            return Ok(RunEnd::Completed {
                quit: i + 1 < steps.len(),
                not_done,
            });
        }
    }
    Ok(RunEnd::Completed {
        quit: false,
        not_done,
    })
}

/// Runs a script that arrives line by line (a pipe), one transcript line per step, as the lines come in.
///
/// A line that is not valid ends the run with [`RunEnd::Usage`]; steps before it have run. `keep_going` is as for
/// [`run`]: a step that is not done does not end the run (a usage error still does).
///
/// # Errors
///
/// The sink's error or the line source's.
pub fn run_stream(
    shell: &mut ShellModel,
    ext: &mut dyn StepExtension,
    lines: &mut dyn Iterator<Item = io::Result<String>>,
    keep_going: bool,
    sink: &mut dyn FnMut(&str) -> io::Result<()>,
) -> io::Result<RunEnd> {
    shell.reset_aliases();
    sink(&transcript_header(&shell.config().app_version.clone()))?;
    let mut parser = ScriptParser::new();
    let mut number = 0;
    let mut not_done = 0;
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
            if !keep_going {
                return Ok(RunEnd::StepFailed { step: number });
            }
            not_done += 1;
        }
        if shell.should_quit() {
            return Ok(RunEnd::Completed { quit: true, not_done });
        }
    }
    Ok(match parser.finish() {
        Ok(()) => RunEnd::Completed {
            quit: false,
            not_done,
        },
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
        let end = run(&mut shell, &mut NoExtension, &steps, false, &mut |l| {
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
        run(&mut shell, &mut NoExtension, &steps, false, &mut |l| {
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
        let end = run_stream(&mut shell, &mut NoExtension, &mut lines, false, &mut |l| {
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
        let end = run(&mut shell, &mut NoExtension, &steps, false, &mut |_| Ok(())).unwrap();
        assert_eq!(
            end,
            RunEnd::Completed {
                quit: true,
                not_done: 0
            }
        );
        assert_eq!(end.exit_code(), 0);
    }

    /// WP-S6.13 BD1: with `--keep-going` a refused step in the middle does not end the run; every step has its line
    /// and the exit code is 1. Without the flag the transcript is today's: it stops at the refusal.
    #[test]
    fn keep_going_runs_a_script_past_a_refused_step() {
        let text = format!(
            "{HEADER}\n{}\n{}\n{}\n",
            r#"{"action":"help.about"}"#, r#"{"action":"file.save"}"#, r#"{"action":"ui.dismiss"}"#
        );
        let steps = parse(&text).unwrap();
        let go = |keep_going: bool| {
            let mut shell = ShellModel::for_tests();
            let mut lines = Vec::new();
            let end = run(&mut shell, &mut NoExtension, &steps, keep_going, &mut |l| {
                lines.push(l.to_owned());
                Ok(())
            })
            .unwrap();
            (end, lines)
        };
        let (end, kept) = go(true);
        assert_eq!(
            end,
            RunEnd::Completed {
                quit: false,
                not_done: 1
            }
        );
        assert_eq!(end.exit_code(), 1);
        // The header and the three steps, the refused one in the middle.
        assert_eq!(kept.len(), 4);
        assert!(kept[2].contains(r#""outcome":"refused""#));
        assert!(kept[3].contains(r#""action":"ui.dismiss""#) && kept[3].contains(r#""outcome":"done""#));
        let (end, stopped) = go(false);
        assert_eq!(end, RunEnd::StepFailed { step: 2 });
        assert_eq!(stopped.len(), 3);
        // The lines the two runs share are equal byte for byte.
        assert_eq!(stopped[..], kept[..3]);
        // The same through the stream runner.
        let mut shell = ShellModel::for_tests();
        let mut input = text.lines().map(|l| Ok(l.to_owned()));
        let mut out = Vec::new();
        let end = run_stream(&mut shell, &mut NoExtension, &mut input, true, &mut |l| {
            out.push(l.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            end,
            RunEnd::Completed {
                quit: false,
                not_done: 1
            }
        );
        assert_eq!(out, kept);
        // A usage error still ends the run, whatever the flag.
        let mut shell = ShellModel::for_tests();
        let mut bad = [HEADER, r#"{"action":"no.such"}"#]
            .into_iter()
            .map(|l| Ok(l.to_owned()));
        let end = run_stream(&mut shell, &mut NoExtension, &mut bad, true, &mut |_| Ok(())).unwrap();
        assert_eq!(end.exit_code(), 2);
    }
}
