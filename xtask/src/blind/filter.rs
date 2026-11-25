// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The harness filter and the typed `key=value` arguments.
//!
//! The reference file `actions.txt` (of the reference, written by `ew-viewmodel`'s `blind_reference` test) is the only source of the
//! actions and their parameter signatures: the filter never imports the product, and reads the file by the window's
//! own signature grammar (`ew-signature`, a crate of no product code). Every step the agent sends is checked here
//! before it reaches the product; a refused step is never sent.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};
use ew_signature::{ActionLine, parse_action_line};
use serde_json::{Map, Number, Value};

use super::run::{Kind, canonical};

/// The most a step's parameters may hold, as compact JSON.
pub const MAX_PARAMS_BYTES: usize = 64 * 1024;

/// The action registry as the harness knows it.
#[derive(Clone, Default, Debug)]
pub struct Reference {
    /// The actions, in registry order.
    pub actions: Vec<ActionLine>,
}

impl Reference {
    /// The action `id`.
    pub fn find(&self, id: &str) -> Option<&ActionLine> {
        self.actions.iter().find(|a| a.id == id)
    }
}

/// Parses `actions.txt`, one action a line, each read by the window's own grammar ([`ew_signature::parse_action_line`]),
/// which `ew-viewmodel`'s test that writes the file reads every line back with.
pub fn parse_reference(text: &str) -> Result<Reference> {
    let mut actions = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let action = parse_action_line(line).map_err(|e| anyhow!("actions.txt line {}: {e}", i + 1))?;
        actions.push(action);
    }
    if actions.is_empty() {
        bail!("actions.txt holds no action");
    }
    Ok(Reference { actions })
}

/// A step as the agent wrote it.
#[derive(Clone, PartialEq, Debug)]
pub struct StepIn {
    /// The action ID.
    pub action: String,
    /// The parameters (an object).
    pub params: Map<String, Value>,
}

impl StepIn {
    /// The parameters as a JSON value.
    pub fn params_value(&self) -> Value {
        Value::Object(self.params.clone())
    }

    /// The key of the exact step for the broken rule, acting on the selection of digest `selection`.
    pub fn step_key(&self, selection: Option<&str>) -> String {
        super::run::step_key(&self.action, &canonical(&self.params_value()), selection)
    }
}

/// The actions the agent never runs: the back door, the pixels and the ones that reach the sample project.
///
/// `first_run.finish` with `start` `sample` and `settings.set` with the key `basic_project` are refused by their parameters.
const BLOCKED: [&str; 6] = [
    "command.run",
    "view.snapshot",
    "canvas.snapshot",
    "canvas.preview_snapshot",
    "file.open_sample",
    "help.tutorials",
];

/// The actions a check run never sends although they are of kind `View`: the canvas actions that can reach a
/// tool's commit. A press, a release and a click at the grid cursor (`canvas.activate`, with or without Alt) finish
/// what an armed tool draws, and `tool.finish` commits it; `canvas.drag*` is the drag family, should the registry ever
/// register one. The checker keeps navigation, zoom, pan, tabs, the page navigator, pointing (`canvas.pointer`,
/// `canvas.dwell`), the grid cursor, `tool.select`, `tool.measure` and `look`.
pub const CHECK_REFUSED: [&str; 5] = [
    "canvas.press",
    "canvas.release",
    "canvas.activate",
    "canvas.activate_alt",
    "tool.finish",
];

/// Whether a check run refuses the action `id` although it is of kind `View` (see [`CHECK_REFUSED`]).
pub fn check_refuses(id: &str) -> bool {
    CHECK_REFUSED.contains(&id) || id.starts_with("canvas.drag")
}

/// Whether `text` holds a UUID (8-4-4-4-12 hexadecimal), anywhere.
pub fn has_uuid(text: &str) -> bool {
    super::normalise::first_uuid(text).is_some()
}

/// Whether any string of `value`, at any depth (object keys included), holds a UUID.
fn value_has_uuid(value: &Value) -> bool {
    match value {
        Value::String(s) => has_uuid(s),
        Value::Array(a) => a.iter().any(value_has_uuid),
        Value::Object(m) => m.iter().any(|(k, v)| has_uuid(k) || value_has_uuid(v)),
        _ => false,
    }
}

/// Whether `name` is a bare relative name: not empty, no `/`, `\`, `:`, no `..`.
pub fn bare_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', ':']) && !name.contains("..")
}

/// Checks one step against the filter's rules in the table's order. `broken` holds the keys of the steps marked broken and
/// `selection` the digest of the selection the step would act on.
///
/// # Errors
///
/// The text of the refusal (`harness: …`).
pub fn check_step(
    reference: &Reference,
    kind: Kind,
    step: &StepIn,
    broken: &BTreeSet<String>,
    selection: Option<&str>,
) -> Result<(), String> {
    let Some(action) = reference.find(&step.action) else {
        return Err(format!("harness: unknown action {}", step.action));
    };
    let id = action.id.as_str();
    let not_available = || format!("harness: {id} is not available to the drafter");
    let str_param = |name: &str| step.params.get(name).and_then(Value::as_str);
    if BLOCKED.contains(&id)
        || (id == "first_run.finish" && str_param("start") == Some("sample"))
        || (id == "settings.set" && str_param("key") == Some("basic_project"))
    {
        return Err(not_available());
    }
    // A check run: only actions of kind `View` and `file.open`, `app.quit`, and of those not the canvas actions that can
    // reach a tool's commit.
    if kind == Kind::Check && (!(action.view || id == "file.open" || id == "app.quit") || check_refuses(id)) {
        return Err("harness: the checker does not change the project".to_owned());
    }
    // Every parameter name is in the signature.
    for name in step.params.keys() {
        if !action.params.iter().any(|p| &p.name == name) {
            return Err(format!("harness: {id} has no parameter {name}"));
        }
    }
    // Every `path`-typed parameter is a bare relative name inside the run's work folder.
    for p in action.params.iter().filter(|p| p.ty == "path") {
        match step.params.get(&p.name) {
            None | Some(Value::Null) => {}
            Some(Value::String(s)) if bare_name(s) => {}
            Some(_) => return Err("harness: paths are names inside the run's work folder".to_owned()),
        }
    }
    // No ID of the model in any parameter: a label, a position or an alias `@n`.
    if value_has_uuid(&step.params_value()) {
        return Err("harness: use a label or a position, not an ID".to_owned());
    }
    if canonical(&step.params_value()).len() > MAX_PARAMS_BYTES {
        return Err("harness: parameters too large".to_owned());
    }
    if broken.contains(&step.step_key(selection)) {
        return Err("harness: marked broken, move on".to_owned());
    }
    Ok(())
}

/// Types the `key=value` arguments of `do <run> <action> …` by the action's signature: `mm`, `int` and `uint`
/// as numbers, `bool` as `true` or `false`, `text`, `page`, `path`, `ref` and `enum(…)` as strings (`folio=101` is the string
/// `"101"`), `json` and `list(…)` as JSON; `null` leaves an optional parameter out.
///
/// # Errors
///
/// The text of the refusal (`harness: …`).
pub fn parse_args(reference: &Reference, action: &str, args: &[String]) -> Result<StepIn, String> {
    let Some(a) = reference.find(action) else {
        return Err(format!("harness: unknown action {action}"));
    };
    let mut params = Map::new();
    for arg in args {
        let Some((name, text)) = arg.split_once('=') else {
            return Err(format!("harness: `{arg}` is not key=value"));
        };
        let Some(p) = a.params.iter().find(|p| p.name == name) else {
            return Err(format!("harness: {action} has no parameter {name}"));
        };
        if text == "null" {
            if p.optional {
                continue;
            }
            return Err(format!("harness: {name} is required"));
        }
        params.insert(name.to_owned(), typed_value(&p.name, &p.ty, text)?);
    }
    Ok(StepIn {
        action: action.to_owned(),
        params,
    })
}

fn typed_value(name: &str, ty: &str, text: &str) -> Result<Value, String> {
    match ty {
        "mm" => number(text).ok_or_else(|| format!("harness: {name} is a number")),
        "int" => text
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| format!("harness: {name} is a whole number")),
        "uint" => text
            .parse::<u64>()
            .map(Value::from)
            .map_err(|_| format!("harness: {name} is a whole number, 0 or more")),
        "bool" => match text {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(format!("harness: {name} is true or false")),
        },
        "json" => serde_json::from_str(text).map_err(|_| format!("harness: {name} is JSON")),
        t if t.starts_with("list(") => match serde_json::from_str::<Value>(text) {
            Ok(v @ Value::Array(_)) => Ok(v),
            _ => Err(format!("harness: {name} is a JSON list")),
        },
        _ => Ok(Value::String(text.to_owned())),
    }
}

/// A real number: a whole one stays whole (`254` is `254`, not `254.0`).
fn number(text: &str) -> Option<Value> {
    if let Ok(i) = text.parse::<i64>() {
        return Some(Value::from(i));
    }
    let f: f64 = text.parse().ok()?;
    Number::from_f64(f).map(Value::Number)
}

/// Reads a batch: 1 to 20 JSON Lines `{"action":…,"params":{…}}`, sent as written.
///
/// # Errors
///
/// The text of the refusal (`harness: …`).
pub fn parse_batch(text: &str) -> Result<Vec<StepIn>, String> {
    let mut steps = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let at = i + 1;
        let v: Value = serde_json::from_str(line).map_err(|_| format!("harness: line {at} is not JSON"))?;
        let Value::Object(mut o) = v else {
            return Err(format!("harness: line {at} is not an object"));
        };
        let Some(Value::String(action)) = o.remove("action") else {
            return Err(format!("harness: line {at} has no action"));
        };
        let params = match o.remove("params") {
            None => Map::new(),
            Some(Value::Object(m)) => m,
            Some(_) => return Err(format!("harness: line {at}: params is an object")),
        };
        if let Some(extra) = o.keys().next() {
            return Err(format!(
                "harness: line {at} has the member {extra}; a line has action and params"
            ));
        }
        steps.push(StepIn { action, params });
    }
    if steps.is_empty() || steps.len() > 20 {
        return Err("harness: a batch has 1 to 20 steps".to_owned());
    }
    Ok(steps)
}

/// A small action list in the form of `actions.txt`, for the tests of the harness: lines of the registry's list,
/// enough for the actions the tests name. The real list is the product's: `ew-viewmodel`'s `blind_reference` test
/// writes and checks it, so the harness's tests build no product.
#[cfg(test)]
pub const TEST_ACTIONS: &str = r#"file.new Command "New project…" menu "File" shortcut "Ctrl+N" context global params "dir?:path, template?:path, pages?:bool, discard?:bool"
file.open Command "Open…" menu "File" shortcut "Ctrl+O" context global params "dir?:path, read_only?:bool, discard?:bool"
file.open_sample Command "Open sample project" menu "File" shortcut none context global params "discard?:bool"
file.save Command "Save" menu "File" shortcut "Ctrl+S" context global params ""
file.save_as Command "Save as…" menu "File" shortcut "Ctrl+Shift+S" context global params "dir?:path"
file.export_pdf Command "Export PDF…" menu "Output" shortcut "Ctrl+Shift+E" context global params "path?:path, replace?:bool, wait?:bool"
app.quit Command "Quit" menu "File" shortcut "Ctrl+Q" context global params "discard?:bool"
edit.undo Command "Undo" menu "Edit" shortcut "Ctrl+Z" context global params ""
edit.redo Command "Redo" menu "Edit" shortcut "Ctrl+Shift+Z" context global params ""
page.next View "Next page" menu "Page" shortcut "Page Down" context global params ""
nav.goto View "Go to page or cell…" menu "Page" shortcut "Ctrl+G" context global params "text?:text"
nav.back View "Back" menu "Page" shortcut "Alt+Arrow Left" context global params ""
nav.follow View "Follow cross-reference" menu "Page" shortcut "F" context canvas params "x_mm?:mm, y_mm?:mm, group?:uint"
canvas.activate View "Click at the grid cursor" menu none shortcut "Enter" context canvas params "alt?:bool"
view.zoom_in View "Zoom in" menu "View" shortcut "Ctrl+Plus" context global params "group?:uint"
view.fit_page View "Fit page" menu "View" shortcut "Home" context global params "group?:uint"
view.pan_left View "Pan left" menu "View ▸ Pan" shortcut "Ctrl+Arrow Left" context global params "group?:uint"
tool.select View "Select" menu "Draw" shortcut none context global params ""
tool.measure View "Measure" menu "Draw" shortcut none context global params ""
tool.wire Command "Wire" menu "Draw" shortcut "W" context canvas params ""
strip.new Command "Terminal strip" menu "Draw" shortcut none context global params "letter?:text, tag?:text, dialog?:bool"
tool.finish View "Finish" menu none shortcut "Shift+Enter" context canvas hidden params ""
canvas.activate_alt View "Click with Alt at the grid cursor" menu none shortcut "Ctrl+Enter" context canvas hidden params ""
image.file View "Image file" menu none shortcut none context canvas hidden params "path:path"
tab.new View "New tab" menu "Window" shortcut "Ctrl+T" context global params ""
help.about View "About ElektraWatt" menu "Help" shortcut none context global params ""
help.tutorials Command "Tutorials" menu "Help ▸ Tutorials" shortcut none context global params "dir?:path, tutorial?:text"
command.run Command "Run model command" menu none shortcut none context global needs-parameters params "cmd:text, params?:json"
view.snapshot View "Write page snapshot" menu none shortcut none context global headless params "path:path, dpi?:uint"
canvas.snapshot View "Write canvas snapshot" menu none shortcut none context global headless params "path:path, group?:uint"
view.zoom_to View "Zoom to" menu none shortcut none context global hidden params "percent:json, x_mm?:mm, y_mm?:mm, group?:uint"
view.pan View "Pan the canvas" menu none shortcut none context global hidden params "dx:json, dy:json, group?:uint"
canvas.pointer View "Point at the canvas" menu none shortcut none context global hidden params "x_mm?:mm, y_mm?:mm, group?:uint, shift?:bool, ctrl?:bool, alt?:bool"
canvas.preview_snapshot View "Link preview snapshot" menu none shortcut none context global headless params "path:path"
canvas.press View "Press on the canvas" menu none shortcut none context canvas hidden params "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint"
canvas.release View "Release on the canvas" menu none shortcut none context canvas hidden params "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint"
canvas.dwell View "Pointer at rest" menu none shortcut none context canvas hidden params "ms:uint"
ui.describe View "Describe the user interface" menu none shortcut none context global headless params ""
ui.dismiss View "Dismiss popup or dialog" menu none shortcut "Escape" context global hidden params ""
page.goto View "Go to page…" menu none shortcut none context global hidden params "folio:text"
tab.activate View "Show tab" menu none shortcut none context global hidden params "group:uint, index:uint"
cursor.set View "Set cursor" menu none shortcut none context canvas hidden params "x_mm?:mm, y_mm?:mm, group?:uint"
pages.row View "Move in the page navigator" menu none shortcut none context navigator hidden params "key?:text, step?:int, to?:enum(first|last)"
pages.open View "Open page" menu none shortcut none context navigator hidden params "key?:text, folio?:text, new_tab?:bool"
props.set Command "Set a property" menu none shortcut none context global hidden params "row?:text, value?:text, choice?:uint"
settings.set Command "Change a setting" menu none shortcut none context global hidden params "key:text, value:json"
first_run.finish Command "First run: finish" menu none shortcut none context global hidden params "start?:enum(wizard|sample|qet|new|none)"
select.set View "Select records" menu none shortcut none context global hidden params "ids?:list(ref), refs?:list(ref)"
find.text View "Type the search text" menu none shortcut none context find hidden params "text:text"
find.fields View "Choose the fields to search" menu none shortcut none context find hidden params "fields:list(text)"
page.describe View "Describe the page content" menu none shortcut none context global headless params "folio?:text"
"#;

/// [`TEST_ACTIONS`] parsed.
#[cfg(test)]
pub fn test_reference() -> Reference {
    parse_reference(TEST_ACTIONS).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(action: &str, params: Value) -> StepIn {
        StepIn {
            action: action.to_owned(),
            params: params.as_object().cloned().unwrap_or_default(),
        }
    }

    fn check(kind: Kind, s: &StepIn) -> Result<(), String> {
        check_step(&test_reference(), kind, s, &BTreeSet::new(), None)
    }

    /// BD6: every refusal of the filter with its text.
    #[test]
    fn every_refusal_of_the_filter_has_its_text() {
        let free = Kind::Free;
        assert_eq!(check(free, &step("edit.undo", json!({}))), Ok(()));
        assert_eq!(
            check(free, &step("no.such.action", json!({}))),
            Err("harness: unknown action no.such.action".to_owned())
        );
        for id in [
            "command.run",
            "view.snapshot",
            "canvas.snapshot",
            "canvas.preview_snapshot",
            "file.open_sample",
            "help.tutorials",
        ] {
            assert_eq!(
                check(free, &step(id, json!({}))),
                Err(format!("harness: {id} is not available to the drafter")),
                "{id}"
            );
        }
        assert_eq!(
            check(free, &step("first_run.finish", json!({"start": "sample"}))),
            Err("harness: first_run.finish is not available to the drafter".to_owned())
        );
        assert_eq!(
            check(free, &step("first_run.finish", json!({"start": "none"}))),
            Ok(())
        );
        assert_eq!(
            check(
                free,
                &step("settings.set", json!({"key": "basic_project", "value": "x"}))
            ),
            Err("harness: settings.set is not available to the drafter".to_owned())
        );
        assert_eq!(
            check(
                free,
                &step("settings.set", json!({"key": "language", "value": "en"}))
            ),
            Ok(())
        );
        // A path with a separator, `..`, a drive colon or nothing.
        let paths = "harness: paths are names inside the run's work folder";
        for bad in ["a/b", "..", "a\\b", "C:x", "../x", "x..y", ""] {
            assert_eq!(
                check(free, &step("file.open", json!({"dir": bad}))),
                Err(paths.to_owned()),
                "{bad:?}"
            );
        }
        assert_eq!(
            check(
                free,
                &step("file.new", json!({"dir": "project", "template": "dir/x"}))
            ),
            Err(paths.to_owned()),
            "a template with a separator"
        );
        assert_eq!(
            check(
                free,
                &step("file.new", json!({"dir": "project", "template": "builtin"}))
            ),
            Ok(())
        );
        assert_eq!(
            check(free, &step("file.open", json!({"dir": 3}))),
            Err(paths.to_owned()),
            "a path is a string"
        );
        // A parameter the signature does not list.
        assert_eq!(
            check(free, &step("edit.undo", json!({"steps": 2}))),
            Err("harness: edit.undo has no parameter steps".to_owned())
        );
        // A UUID, nested in a list or an object, in any position of a value.
        let uuid = "0197a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b";
        let id_refusal = "harness: use a label or a position, not an ID";
        assert_eq!(
            check(free, &step("select.set", json!({"ids": ["@1", uuid]}))),
            Err(id_refusal.to_owned())
        );
        assert_eq!(
            check(free, &step("select.set", json!({"refs": [format!("id:{uuid}")]}))),
            Err(id_refusal.to_owned())
        );
        // The alias is admitted, bare and as `id:@n`.
        assert_eq!(
            check(
                free,
                &step(
                    "select.set",
                    json!({"ids": ["@1"], "refs": ["id:@2", "tag:-101F1"]})
                )
            ),
            Ok(())
        );
        // Too large.
        let big = "x".repeat(MAX_PARAMS_BYTES);
        assert_eq!(
            check(free, &step("find.text", json!({"text": big}))),
            Err("harness: parameters too large".to_owned())
        );
        let at_limit = "x".repeat(MAX_PARAMS_BYTES - r#"{"text":""}"#.len());
        assert_eq!(check(free, &step("find.text", json!({"text": at_limit}))), Ok(()));
        // Marked broken: the exact step only.
        let broken: BTreeSet<String> = [
            step("edit.redo", json!({})).step_key(None),
            step("edit.undo", json!({})).step_key(Some("0123456789ab")),
        ]
        .into();
        assert_eq!(
            check_step(
                &test_reference(),
                free,
                &step("edit.redo", json!({})),
                &broken,
                None
            ),
            Err("harness: marked broken, move on".to_owned())
        );
        assert_eq!(
            check_step(
                &test_reference(),
                free,
                &step("pages.row", json!({"to": "first"})),
                &broken,
                None
            ),
            Ok(())
        );
        // The same parameters on another selection are another step; on the same selection the same.
        let undo = step("edit.undo", json!({}));
        assert_eq!(
            check_step(&test_reference(), free, &undo, &broken, Some("0123456789ab")),
            Err("harness: marked broken, move on".to_owned())
        );
        for other in [None, Some("ba9876543210")] {
            assert_eq!(check_step(&test_reference(), free, &undo, &broken, other), Ok(()));
        }
    }

    #[test]
    fn a_check_run_only_looks() {
        let refusal = Err("harness: the checker does not change the project".to_owned());
        assert_eq!(check(Kind::Check, &step("edit.undo", json!({}))), refusal);
        assert_eq!(check(Kind::Check, &step("file.save", json!({}))), refusal);
        assert_eq!(
            check(Kind::Check, &step("file.save_as", json!({"dir": "x"}))),
            refusal
        );
        // Only the registry's kind decides: a canvas action is a `View`, and the checker works on a copy.
        assert_eq!(
            check(
                Kind::Check,
                &step("canvas.pointer", json!({"x_mm": 1, "y_mm": 2}))
            ),
            Ok(())
        );
        assert_eq!(
            check(Kind::Check, &step("file.open", json!({"dir": "project"}))),
            Ok(())
        );
        assert_eq!(check(Kind::Check, &step("app.quit", json!({}))), Ok(()));
        assert_eq!(
            check(Kind::Check, &step("page.goto", json!({"folio": "101"}))),
            Ok(())
        );
        assert_eq!(check(Kind::Check, &step("page.describe", json!({}))), Ok(()));
        assert_eq!(check(Kind::Free, &step("edit.undo", json!({}))), Ok(()));
    }

    /// `image.file` reads a file of the host, so its `path` is confined to
    /// a name inside the run's work folder like every other path parameter, in a check run as well (it is a `View`).
    #[test]
    fn image_file_reads_only_inside_the_work_folder() {
        let paths = Err("harness: paths are names inside the run's work folder".to_owned());
        for kind in [Kind::Free, Kind::Brief, Kind::Check] {
            for bad in ["../x.png", "C:/x.png", "/etc/passwd", "a/b.png", "..\\x.png"] {
                assert_eq!(
                    check(kind, &step("image.file", json!({"path": bad}))),
                    paths,
                    "{kind:?} {bad:?}"
                );
            }
            assert_eq!(
                check(kind, &step("image.file", json!({"path": "logo.png"}))),
                Ok(()),
                "{kind:?}: a name inside the work folder"
            );
        }
    }

    /// A check run refuses the canvas actions that can reach a tool's commit (press, release, a click at the grid
    /// cursor with or without Alt, finish, and any drag), with the checker's refusal text, and keeps navigation, zoom,
    /// pan, tabs, the page navigator, pointing and the looks. A drafter run is not affected.
    #[test]
    fn a_check_run_refuses_what_can_reach_a_tools_commit_and_keeps_the_views() {
        let refusal = Err("harness: the checker does not change the project".to_owned());
        let canvas = json!({"x_mm": 10, "y_mm": 20});
        for (id, params) in [
            ("canvas.press", canvas.clone()),
            ("canvas.release", canvas.clone()),
            ("canvas.activate", json!({})),
            ("canvas.activate_alt", json!({})),
            ("tool.finish", json!({})),
        ] {
            assert_eq!(check(Kind::Check, &step(id, params.clone())), refusal, "{id}");
            assert_eq!(
                check(Kind::Free, &step(id, params)),
                Ok(()),
                "{id} stays the drafter's"
            );
        }
        // A drag, should the registry register one, is refused by its family name (the committed registry has none).
        let mut with_drag = test_reference();
        for id in ["canvas.drag", "canvas.drag_to"] {
            with_drag.actions.push(ActionLine {
                id: id.to_owned(),
                view: true,
                params: Vec::new(),
            });
            assert_eq!(
                check_step(
                    &with_drag,
                    Kind::Check,
                    &step(id, json!({})),
                    &BTreeSet::new(),
                    None
                ),
                refusal,
                "{id}"
            );
        }
        // What a checker keeps: navigation, zoom, pan, tabs, the navigator, pointing, the cursor, the looks, the view tools.
        for (id, params) in [
            ("nav.goto", json!({"text": "101"})),
            ("nav.follow", json!({})),
            ("nav.back", json!({})),
            ("page.goto", json!({"folio": "102"})),
            ("page.next", json!({})),
            ("view.zoom_in", json!({})),
            ("view.zoom_to", json!({"percent": 50})),
            ("view.fit_page", json!({})),
            ("view.pan", json!({"dx": 1, "dy": 2})),
            ("view.pan_left", json!({})),
            ("tab.new", json!({})),
            ("tab.activate", json!({"group": 0, "index": 0})),
            ("pages.open", json!({"folio": "103"})),
            ("canvas.pointer", canvas.clone()),
            ("canvas.dwell", json!({"ms": 100})),
            ("cursor.set", canvas.clone()),
            ("tool.select", json!({})),
            ("tool.measure", json!({})),
            ("ui.describe", json!({})),
            ("page.describe", json!({"folio": "101"})),
            ("file.open", json!({"dir": "project"})),
            ("app.quit", json!({})),
        ] {
            assert_eq!(check(Kind::Check, &step(id, params)), Ok(()), "{id} is kept");
        }
    }

    /// BD7: the `key=value` parser types by signature.
    #[test]
    fn key_value_arguments_are_typed_by_the_signature() {
        let r = test_reference();
        let args = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let s = parse_args(&r, "nav.goto", &args(&["text=101"])).unwrap();
        assert_eq!(
            s.params["text"],
            json!("101"),
            "a text is a string even when it looks like a number"
        );
        let s = parse_args(&r, "page.goto", &args(&["folio=101"])).unwrap();
        assert_eq!(s.params["folio"], json!("101"));
        let s = parse_args(
            &r,
            "canvas.press",
            &args(&["x_mm=254", "y_mm=127.5", "shift=true", "button=middle", "group=1"]),
        )
        .unwrap();
        assert_eq!(s.params["x_mm"], json!(254));
        assert!(s.params["x_mm"].is_i64(), "a whole number stays whole");
        assert_eq!(s.params["y_mm"], json!(127.5));
        assert_eq!(s.params["shift"], json!(true));
        assert_eq!(s.params["button"], json!("middle"));
        assert_eq!(s.params["group"], json!(1));
        // json and list types read JSON.
        let s = parse_args(&r, "find.fields", &args(&[r#"fields=["tag","text"]"#])).unwrap();
        assert_eq!(s.params["fields"], json!(["tag", "text"]));
        let s = parse_args(&r, "view.zoom_to", &args(&["percent=62.5"])).unwrap();
        assert_eq!(s.params["percent"], json!(62.5));
        // `null` leaves an optional parameter out and refuses a required one.
        let s = parse_args(&r, "canvas.press", &args(&["x_mm=1", "y_mm=2", "button=null"])).unwrap();
        assert!(!s.params.contains_key("button"));
        assert_eq!(
            parse_args(&r, "canvas.press", &args(&["x_mm=null"])).unwrap_err(),
            "harness: x_mm is required"
        );
        // Errors.
        assert_eq!(
            parse_args(&r, "canvas.press", &args(&["x_mm=abc"])).unwrap_err(),
            "harness: x_mm is a number"
        );
        assert_eq!(
            parse_args(&r, "canvas.press", &args(&["shift=yes"])).unwrap_err(),
            "harness: shift is true or false"
        );
        assert_eq!(
            parse_args(&r, "canvas.press", &args(&["colour=red"])).unwrap_err(),
            "harness: canvas.press has no parameter colour"
        );
        assert_eq!(
            parse_args(&r, "canvas.press", &args(&["x_mm"])).unwrap_err(),
            "harness: `x_mm` is not key=value"
        );
        assert_eq!(
            parse_args(&r, "no.such", &[]).unwrap_err(),
            "harness: unknown action no.such"
        );
        assert_eq!(
            parse_args(&r, "tab.activate", &args(&["group=-1", "index=0"])).unwrap_err(),
            "harness: group is a whole number, 0 or more"
        );
    }

    #[test]
    fn a_batch_has_one_to_twenty_steps() {
        let one = r#"{"action":"edit.undo"}"#;
        assert_eq!(parse_batch(one).unwrap().len(), 1);
        let twenty = std::iter::repeat_n(one, 20).collect::<Vec<_>>().join("\n");
        assert_eq!(parse_batch(&twenty).unwrap().len(), 20);
        let twenty_one = std::iter::repeat_n(one, 21).collect::<Vec<_>>().join("\n");
        assert_eq!(
            parse_batch(&twenty_one).unwrap_err(),
            "harness: a batch has 1 to 20 steps"
        );
        assert_eq!(parse_batch("").unwrap_err(), "harness: a batch has 1 to 20 steps");
        assert!(parse_batch("not json").unwrap_err().contains("not JSON"));
        assert!(parse_batch(r#"{"params":{}}"#).unwrap_err().contains("no action"));
        assert!(
            parse_batch(r#"{"action":"a","extra":1}"#)
                .unwrap_err()
                .contains("extra")
        );
        assert!(
            parse_batch(r#"{"action":"a","params":[1]}"#)
                .unwrap_err()
                .contains("params")
        );
    }
}
