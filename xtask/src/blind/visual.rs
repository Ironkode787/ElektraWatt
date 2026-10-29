// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The visual run's side of the protocol (WP-S6.13 part 4, visual spec §3 and §4): `do` sends mouse and key inputs and
//! answers with one frame, `look` paints the current frame again (or a zoom of it), and nothing textual reaches the
//! agent. The inputs are checked here with the texts of the visual driver (`elektrawatt --headless --visual`), so that
//! a line the driver would refuse never reaches it: the harness has no edge to the product crates (D2), so the checks
//! are mirrored, and the product's own tests hold the same names.

use std::fmt::Write as _;

use anyhow::Result;
use serde_json::{Map, Value, json};

use super::protocol::{apply_caps, derive_state, ended, lost_message, mark_lost};
use super::run::{FRAME_LINE, INPUT_LINE, Run, Turn, append, step_line};
use super::session::{Backend, Lost};

/// The frame's width in pixels (1280 points at 1.2).
pub const FRAME_WIDTH: i64 = 1536;
/// The frame's height in pixels (800 points at 1.2 and the 34-pixel title strip).
pub const FRAME_HEIGHT: i64 = 994;
/// The largest zoom region in frame pixels.
pub const ZOOM_MAX: (i64, i64) = (768, 497);
/// The most inputs of one `do`.
pub const MAX_INPUTS: usize = 12;
/// The most milliseconds the `wait` inputs of one `do` may add up to.
pub const MAX_WAIT_MS: u64 = 10_000;

/// A text look, a page look, a diff or the menus in a visual run.
pub const NO_TEXT_VIEW: &str = "harness: a visual run has no text view";
/// An action line from the agent of a visual run.
pub const NO_ACTIONS: &str = "harness: in a visual run you act with the mouse and the keys";
/// What `do` prints when the application closed.
pub const APP_CLOSED: &str = "harness: the application has closed";
/// What `start` prints on a host without an offscreen adapter.
pub const NO_ADAPTER: &str = "harness: no offscreen adapter for a visual run";

/// The input names of the vocabulary (§4.1).
pub const INPUTS: [&str; 9] = [
    "click",
    "double_click",
    "right_click",
    "drag",
    "scroll",
    "type",
    "key",
    "hover",
    "wait",
];

/// The key names the harness accepts in a chord (any case): a subset of what the driver accepts (egui's key names and
/// its aliases), held equal by the driver's test `the_harness_key_names_are_keys`.
pub const KEY_NAMES: &[&str] = &[
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
    "enter",
    "return",
    "escape",
    "esc",
    "tab",
    "backspace",
    "delete",
    "del",
    "insert",
    "home",
    "end",
    "pageup",
    "pagedown",
    "pgup",
    "pgdn",
    "space",
    "arrowup",
    "arrowdown",
    "arrowleft",
    "arrowright",
    "up",
    "down",
    "left",
    "right",
    "minus",
    "plus",
    "equals",
    "comma",
    "period",
    "slash",
    "backslash",
    "semicolon",
    "-",
    "+",
    "=",
    ",",
    ".",
    "/",
    ";",
];

/// The integer members of the inputs (the `key=value` form types them as numbers).
const INTEGER_MEMBERS: [&str; 8] = ["x", "y", "to_x", "to_y", "dx", "dy", "repeat", "ms"];

fn check_mods(text: &str) -> Result<(), String> {
    for part in text.split('+') {
        let p = part.trim().to_ascii_lowercase();
        if !matches!(p.as_str(), "ctrl" | "shift" | "alt") {
            return Err(format!("unknown modifier `{p}`"));
        }
    }
    Ok(())
}

/// Checks a chord as the driver reads it: modifiers and at most one key joined by `+` (`ctrl+s`, `ctrl++`, `alt`).
///
/// # Errors
///
/// The driver's text.
pub fn check_chord(chord: &str) -> Result<(), String> {
    if chord.is_empty() || chord.len() > 40 {
        return Err("`chord` is 1 to 40 characters".to_owned());
    }
    let (head, last) = match chord.strip_suffix("++") {
        Some(head) => (head, "+"),
        None if chord == "+" => ("", "+"),
        None => chord.rsplit_once('+').unwrap_or(("", chord)),
    };
    if !head.is_empty() {
        check_mods(head)?;
    }
    let lower = last.to_ascii_lowercase();
    if matches!(lower.as_str(), "ctrl" | "shift" | "alt") || KEY_NAMES.contains(&lower.as_str()) {
        Ok(())
    } else {
        Err(format!("unknown key `{last}`"))
    }
}

fn coordinate(o: &Map<String, Value>, name: &str, limit: i64) -> Result<(), String> {
    match o.get(name).and_then(Value::as_i64) {
        Some(v) if (0..limit).contains(&v) => Ok(()),
        Some(_) => Err(format!("`{name}` is outside the frame (0 to {})", limit - 1)),
        None => Err(format!("`{name}` is an integer frame pixel")),
    }
}

fn point(o: &Map<String, Value>, x: &str, y: &str) -> Result<(), String> {
    coordinate(o, x, FRAME_WIDTH)?;
    coordinate(o, y, FRAME_HEIGHT)
}

fn mods_member(o: &Map<String, Value>) -> Result<(), String> {
    match o.get("mods") {
        None => Ok(()),
        Some(Value::String(s)) => check_mods(s),
        Some(_) => Err("`mods` is text like \"ctrl+shift\"".to_owned()),
    }
}

fn button_member(o: &Map<String, Value>) -> Result<(), String> {
    match o.get("button").map(Value::as_str) {
        None | Some(Some("left" | "right" | "middle")) => Ok(()),
        Some(_) => Err("`button` is left, right or middle".to_owned()),
    }
}

fn lines_member(o: &Map<String, Value>, name: &str) -> Result<i64, String> {
    match o.get(name) {
        None => Ok(0),
        Some(v) => match v.as_i64() {
            Some(n) if (-20..=20).contains(&n) => Ok(n),
            _ => Err(format!("`{name}` is a whole number of lines from -20 to 20")),
        },
    }
}

/// Checks one input object (visual spec §4.1) as the driver does, in its order, and returns the milliseconds it waits.
///
/// # Errors
///
/// The driver's text for the first problem.
pub fn check_input(o: &Map<String, Value>) -> Result<u64, String> {
    let kind = o
        .get("input")
        .and_then(Value::as_str)
        .ok_or_else(|| "`input` names the input".to_owned())?;
    let members: &[&str] = match kind {
        "click" => &["x", "y", "button", "mods"],
        "double_click" | "right_click" => &["x", "y", "mods"],
        "drag" => &["x", "y", "to_x", "to_y", "button", "mods"],
        "scroll" => &["x", "y", "dx", "dy", "mods"],
        "type" => &["text"],
        "key" => &["chord", "repeat"],
        "hover" => &["x", "y"],
        "wait" => &["ms"],
        other => return Err(format!("unknown input `{other}`")),
    };
    if let Some(extra) = o.keys().find(|k| *k != "input" && !members.contains(&k.as_str())) {
        return Err(format!("unknown member `{extra}` of `{kind}`"));
    }
    match kind {
        "click" => {
            point(o, "x", "y")?;
            button_member(o)?;
            mods_member(o)?;
        }
        "right_click" | "double_click" => {
            point(o, "x", "y")?;
            mods_member(o)?;
        }
        "drag" => {
            point(o, "x", "y")?;
            point(o, "to_x", "to_y")?;
            button_member(o)?;
            mods_member(o)?;
        }
        "scroll" => {
            let (dx, dy) = (lines_member(o, "dx")?, lines_member(o, "dy")?);
            if dx == 0 && dy == 0 {
                return Err("a scroll needs `dx` or `dy`".to_owned());
            }
            point(o, "x", "y")?;
            mods_member(o)?;
        }
        "type" => {
            let text = o
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| "`text` is the text to type".to_owned())?;
            let n = text.chars().count();
            if !(1..=200).contains(&n) || text.chars().any(char::is_control) {
                return Err("`text` is 1 to 200 characters without control characters".to_owned());
            }
        }
        "key" => {
            let chord = o
                .get("chord")
                .and_then(Value::as_str)
                .ok_or_else(|| "`chord` names the key, like \"ctrl+s\"".to_owned())?;
            check_chord(chord)?;
            if let Some(v) = o.get("repeat")
                && !matches!(v.as_u64(), Some(1..=20))
            {
                return Err("`repeat` is 1 to 20".to_owned());
            }
        }
        "hover" => point(o, "x", "y")?,
        _ => match o.get("ms").and_then(Value::as_u64) {
            Some(ms @ 100..=5000) => return Ok(ms),
            _ => return Err("`ms` is 100 to 5000".to_owned()),
        },
    }
    Ok(0)
}

/// Reads a batch of inputs: 1 to 12 JSON lines, each an input object; an action line is refused.
///
/// # Errors
///
/// The refusal text (`harness: …`).
pub fn parse_inputs(text: &str) -> Result<Vec<Map<String, Value>>, String> {
    let mut inputs = Vec::new();
    let mut waited = 0u64;
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let at = i + 1;
        let v: Value = serde_json::from_str(line).map_err(|_| format!("harness: line {at} is not JSON"))?;
        let Value::Object(o) = v else {
            return Err(format!("harness: line {at} is not an object"));
        };
        if o.contains_key("action") || !o.contains_key("input") {
            return Err(NO_ACTIONS.to_owned());
        }
        waited += check_input(&o).map_err(|e| format!("harness: line {at}: {e}"))?;
        inputs.push(o);
    }
    if inputs.is_empty() || inputs.len() > MAX_INPUTS {
        return Err(format!("harness: a batch has 1 to {MAX_INPUTS} inputs"));
    }
    if waited > MAX_WAIT_MS {
        return Err(format!("harness: a batch waits at most {MAX_WAIT_MS} ms"));
    }
    Ok(inputs)
}

/// Reads the `key=value` form of one input: `do <run> click x=412 y=96`; the integer members are numbers.
///
/// # Errors
///
/// The refusal text (`harness: …`).
pub fn parse_input_args(name: &str, args: &[String]) -> Result<Map<String, Value>, String> {
    if !INPUTS.contains(&name) {
        return Err(NO_ACTIONS.to_owned());
    }
    let mut o = Map::new();
    o.insert("input".to_owned(), Value::from(name));
    for arg in args {
        let Some((key, text)) = arg.split_once('=') else {
            return Err(format!("harness: `{arg}` is not key=value"));
        };
        let value = match text.parse::<i64>() {
            Ok(n) if INTEGER_MEMBERS.contains(&key) => Value::from(n),
            _ => Value::from(text),
        };
        o.insert(key.to_owned(), value);
    }
    let waited = check_input(&o).map_err(|e| format!("harness: {e}"))?;
    if waited > MAX_WAIT_MS {
        return Err(format!("harness: a batch waits at most {MAX_WAIT_MS} ms"));
    }
    Ok(o)
}

/// Reads `look --zoom <x> <y> <w> <h>`: a region in frame pixels, at most 768 × 497, inside the frame.
///
/// # Errors
///
/// The refusal text (`harness: …`).
pub fn parse_zoom(args: &[String]) -> Result<[i64; 4], String> {
    let bad = || {
        format!(
            "harness: look --zoom <x> <y> <w> <h>: a region of at most {} × {} frame pixels inside the frame",
            ZOOM_MAX.0, ZOOM_MAX.1
        )
    };
    let v: Vec<i64> = args.iter().filter_map(|a| a.parse().ok()).collect();
    let [x, y, w, h] = v[..] else { return Err(bad()) };
    if args.len() != 4
        || x < 0
        || y < 0
        || w < 1
        || h < 1
        || w > ZOOM_MAX.0
        || h > ZOOM_MAX.1
        || x + w > FRAME_WIDTH
        || y + h > FRAME_HEIGHT
    {
        return Err(bad());
    }
    Ok([x, y, w, h])
}

/// Records a harness refusal of a visual run that sent nothing (a refused batch, a text look): a turn line of kind
/// `refusal`, which needs no note, and one more refusal against the cap.
///
/// # Errors
///
/// A failure of the run folder.
pub fn refuse(run: &Run, text: &str) -> Result<String> {
    let rj = run.load()?;
    if let Some(end) = &rj.end {
        return Ok(ended(end));
    }
    run.push_turn(Turn {
        kind: "refusal".to_owned(),
        refusal: Some(text.to_owned()),
        ..Turn::default()
    })?;
    run.update(|j| {
        j.counts.refusals += 1;
        apply_caps(j, false);
    })?;
    Ok(format!("{text}\n"))
}

/// The file name of a turn's frame: `t0003.png`, a look's `t0003-l1.png`, a zoom's `t0003-z1.png`.
pub fn frame_name(turn: u32, look: Option<(char, u32)>) -> String {
    match look {
        None => format!("t{turn:04}.png"),
        Some((c, k)) => format!("t{turn:04}-{c}{k}.png"),
    }
}

/// The start frame's line, the first line of a visual run's `steps.uiscript` (§3.1 item 4).
pub fn start_frame_line() -> String {
    step_line(FRAME_LINE, &json!({ "frame": frame_name(0, None) }))
}

/// `do` in a visual run: every input of the batch is sent in order (no stop at a dead input, no broken rule), then the
/// batch's frame is painted. Prints exactly `frame frames/t<turn>.png`, or that the application has closed.
///
/// # Errors
///
/// A failure of the run folder or of the product that is not a lost session.
pub fn do_inputs(run: &Run, backend: &mut dyn Backend, inputs: &[Map<String, Value>]) -> Result<String> {
    let rj = run.load()?;
    if let Some(end) = &rj.end {
        return Ok(ended(end));
    }
    let st = derive_state(&run.turns()?);
    if let Some(n) = st.pending_note {
        return Ok(format!("harness: note required for turn {n}\n"));
    }
    let turn_no = st.turns + 1;
    let before = run.steps()?.len();
    let mut lines = 0u32;
    let mut sent_inputs = 0u32;
    let mut refused = 0u32;
    let mut failed = 0u32;
    let mut closed = false;
    let mut lost = false;
    let mut cap_hit = false;
    let mut send = |action: &str, params: Value| -> Result<Option<Value>> {
        let line = step_line(action, &params);
        match backend.execute(&super::session::Step {
            action: action.to_owned(),
            params,
        }) {
            Ok(sent) => {
                append(&run.path("steps.uiscript"), &format!("{line}\n"))?;
                append(&run.path("transcript.jsonl"), &format!("{}\n", sent.raw))?;
                lines += 1;
                Ok(Some(sent.value))
            }
            Err(e) if e.downcast_ref::<Lost>().is_some() => Ok(None),
            Err(e) => Err(e),
        }
    };
    for o in inputs {
        if rj.counts.steps + sent_inputs >= rj.caps.steps {
            cap_hit = true;
            break;
        }
        let Some(v) = send(INPUT_LINE, Value::Object(o.clone()))? else {
            lost = true;
            break;
        };
        sent_inputs += 1;
        for d in v["dispatched"].as_array().into_iter().flatten() {
            match d["outcome"].as_str() {
                Some("refused") => refused += 1,
                Some("failed") => failed += 1,
                _ => {}
            }
        }
        if v["closed"].as_bool() == Some(true) {
            closed = true;
            break;
        }
    }
    let file = frame_name(turn_no, None);
    if !closed && !lost {
        match send(FRAME_LINE, json!({ "frame": file }))? {
            Some(v) => closed = v["closed"].as_bool() == Some(true),
            None => lost = true,
        }
    }
    let ended_call = backend.end_call();
    if lost && lines == 0 {
        mark_lost(run);
        return Ok(lost_message(run));
    }
    ended_call?;
    let range = (lines > 0).then(|| {
        let first = u32::try_from(before + 1).unwrap_or(u32::MAX);
        [first, first + lines - 1]
    });
    run.push_turn(Turn {
        kind: "do".to_owned(),
        turn: Some(turn_no),
        steps: range,
        frame: (!closed && !lost).then(|| file.clone()),
        ..Turn::default()
    })?;
    run.update(|j| {
        j.counts.turns += 1;
        j.counts.steps += sent_inputs;
        j.counts.refused += refused;
        j.counts.failed += failed;
        if closed && j.end.is_none() {
            j.end = Some("app-closed".to_owned());
        }
        apply_caps(j, cap_hit);
    })?;
    let mut out = String::new();
    if lost {
        mark_lost(run);
        out.push_str(&lost_message(run));
    } else if closed {
        let _ = writeln!(out, "{APP_CLOSED}");
    } else {
        let _ = writeln!(out, "frame frames/{file}");
    }
    Ok(out)
}

/// `look` in a visual run: the current state painted again without input (`frames/t<turn>-l<k>.png`), or a region of
/// it enlarged twice (`--zoom`, `frames/t<turn>-z<k>.png`). At most three looks between two `do`.
///
/// # Errors
///
/// A failure of the run folder or of the product that is not a lost session.
pub fn look(run: &Run, backend: &mut dyn Backend, zoom: Option<[i64; 4]>) -> Result<String> {
    let rj = run.load()?;
    if let Some(end) = &rj.end {
        return Ok(ended(end));
    }
    let st = derive_state(&run.turns()?);
    if st.looks_since_do >= super::protocol::LOOKS_PER_TURN {
        return refuse(run, "harness: look cap per turn");
    }
    let k = st.looks_since_do + 1;
    let file = frame_name(st.turns, Some((if zoom.is_some() { 'z' } else { 'l' }, k)));
    let mut params = json!({ "frame": file, "settle": false });
    if let Some(z) = zoom {
        params["zoom"] = json!(z);
    }
    let sent = match backend.execute(&super::session::Step {
        action: FRAME_LINE.to_owned(),
        params: params.clone(),
    }) {
        Ok(s) => s,
        Err(e) if e.downcast_ref::<Lost>().is_some() => {
            mark_lost(run);
            return Ok(lost_message(run));
        }
        Err(e) => return Err(e),
    };
    let before = run.steps()?.len();
    append(
        &run.path("steps.uiscript"),
        &format!("{}\n", step_line(FRAME_LINE, &params)),
    )?;
    append(&run.path("transcript.jsonl"), &format!("{}\n", sent.raw))?;
    backend.end_call()?;
    let n = u32::try_from(before + 1).unwrap_or(u32::MAX);
    run.push_turn(Turn {
        kind: "look".to_owned(),
        steps: Some([n, n]),
        frame: Some(file.clone()),
        ..Turn::default()
    })?;
    run.update(|j| {
        j.counts.looks += 1;
        apply_caps(j, false);
    })?;
    Ok(format!("frames/{file}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::protocol::tests::{Fixture, fixture};
    use crate::blind::protocol::{PLAYTEST_HEADINGS, handback, note};
    use crate::blind::run::Kind;

    fn visual_fixture() -> Fixture {
        let f = fixture(Kind::Free);
        f.run
            .update(|j| {
                j.senses = Some("visual".to_owned());
                j.caps = Kind::Free.caps_for(crate::blind::run::Senses::Visual);
            })
            .unwrap();
        f
    }

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    fn inputs(lines: &[Value]) -> Vec<Map<String, Value>> {
        lines.iter().map(|v| obj(v.clone())).collect()
    }

    /// BV2: each input checks its members and bounds with the driver's texts.
    #[test]
    fn bv2_the_inputs_are_checked_with_the_drivers_texts() {
        let err = |v: Value| check_input(&obj(v)).unwrap_err();
        assert_eq!(
            err(json!({"input": "click", "x": 1536, "y": 0})),
            "`x` is outside the frame (0 to 1535)"
        );
        assert_eq!(
            err(json!({"input": "click", "x": 1, "y": 994})),
            "`y` is outside the frame (0 to 993)"
        );
        assert_eq!(
            err(json!({"input": "click", "x": 1.5, "y": 1})),
            "`x` is an integer frame pixel"
        );
        assert_eq!(
            err(json!({"input": "key", "chord": "ctrl+nokey"})),
            "unknown key `nokey`"
        );
        assert_eq!(
            err(json!({"input": "key", "chord": "hyper+s"})),
            "unknown modifier `hyper`"
        );
        assert_eq!(
            err(json!({"input": "type", "text": "x".repeat(201)})),
            "`text` is 1 to 200 characters without control characters"
        );
        assert_eq!(
            err(json!({"input": "type", "text": "a\tb"})),
            "`text` is 1 to 200 characters without control characters"
        );
        assert_eq!(err(json!({"input": "wait", "ms": 99})), "`ms` is 100 to 5000");
        assert_eq!(err(json!({"input": "wait", "ms": 5001})), "`ms` is 100 to 5000");
        assert_eq!(
            err(json!({"input": "scroll", "x": 1, "y": 1})),
            "a scroll needs `dx` or `dy`"
        );
        assert_eq!(
            err(json!({"input": "scroll", "x": 1, "y": 1, "dy": 21})),
            "`dy` is a whole number of lines from -20 to 20"
        );
        assert_eq!(
            err(json!({"input": "click", "x": 1, "y": 1, "button": "side"})),
            "`button` is left, right or middle"
        );
        assert_eq!(
            err(json!({"input": "key", "chord": "s", "repeat": 21})),
            "`repeat` is 1 to 20"
        );
        assert_eq!(
            err(json!({"input": "hover", "x": 1, "y": 1, "z": 2})),
            "unknown member `z` of `hover`"
        );
        assert_eq!(err(json!({"input": "fly"})), "unknown input `fly`");
        for chord in [
            "ctrl+s",
            "ctrl++",
            "shift+tab",
            "alt",
            "F2",
            "arrowdown",
            "escape",
            "ctrl+shift+e",
        ] {
            assert!(check_chord(chord).is_ok(), "{chord}");
        }
        assert_eq!(check_input(&obj(json!({"input": "wait", "ms": 600}))), Ok(600));
        assert_eq!(
            check_input(&obj(
                json!({"input": "drag", "x": 0, "y": 40, "to_x": 1535, "to_y": 993, "mods": "shift"})
            )),
            Ok(0)
        );
    }

    /// BV7: a batch is 1 to 12 inputs, waits at most 10 s, and holds no action line; the `key=value` form types the
    /// integer members.
    #[test]
    fn bv7_the_batch_and_the_key_value_form() {
        let line = r#"{"input":"click","x":10,"y":10}"#;
        let batch = |n: usize| vec![line; n].join("\n");
        assert_eq!(parse_inputs(&batch(12)).unwrap().len(), 12);
        assert_eq!(
            parse_inputs(&batch(13)).unwrap_err(),
            "harness: a batch has 1 to 12 inputs"
        );
        assert_eq!(
            parse_inputs("").unwrap_err(),
            "harness: a batch has 1 to 12 inputs"
        );
        assert_eq!(
            parse_inputs(r#"{"action":"file.save","params":{}}"#).unwrap_err(),
            NO_ACTIONS
        );
        assert_eq!(
            parse_inputs(
                "{\"input\":\"wait\",\"ms\":5000}\n{\"input\":\"wait\",\"ms\":5000}\n{\"input\":\"wait\",\"ms\":100}"
            )
            .unwrap_err(),
            "harness: a batch waits at most 10000 ms"
        );
        assert_eq!(
            parse_inputs("{\"input\":\"click\",\"x\":2000,\"y\":1}").unwrap_err(),
            "harness: line 1: `x` is outside the frame (0 to 1535)"
        );
        let o = parse_input_args(
            "click",
            &["x=412".to_owned(), "y=96".to_owned(), "button=right".to_owned()],
        )
        .unwrap();
        assert_eq!(
            Value::Object(o),
            json!({"input": "click", "x": 412, "y": 96, "button": "right"})
        );
        let o = parse_input_args("type", &["text=12".to_owned()]).unwrap();
        assert_eq!(o["text"], "12", "a text stays text");
        assert_eq!(parse_input_args("file.save", &[]).unwrap_err(), NO_ACTIONS);
        let zoom = |a: &[&str]| parse_zoom(&a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        assert_eq!(zoom(&["0", "0", "768", "497"]), Ok([0, 0, 768, 497]));
        assert!(zoom(&["0", "0", "769", "10"]).is_err());
        assert!(zoom(&["1000", "0", "768", "10"]).is_err());
        assert!(zoom(&["1", "2", "3"]).is_err());
    }

    /// BV7: every input of a batch is sent (a refused one too, no broken rule), the frame follows, `do` prints only
    /// the frame's path; a note is needed before the next `do`; looks and zooms are frames, three per turn.
    #[test]
    fn bv7_a_visual_do_sends_every_input_and_prints_only_the_frame() {
        let mut f = visual_fixture();
        let batch = inputs(&[
            json!({"input": "click", "x": 5, "y": 50}),
            json!({"input": "click", "x": 100, "y": 50}),
            json!({"input": "key", "chord": "ctrl+z"}),
        ]);
        let out = do_inputs(&f.run, &mut f.backend, &batch).unwrap();
        assert_eq!(out, "frame frames/t0001.png\n");
        let steps = f.run.steps().unwrap();
        assert_eq!(steps.len(), 4, "three inputs and the frame");
        assert_eq!(steps[3], (FRAME_LINE.to_owned(), json!({"frame": "t0001.png"})));
        let text = std::fs::read_to_string(f.run.path("steps.uiscript")).unwrap();
        assert!(text.contains("{\"input\":\"click\",\"x\":5,\"y\":50}"), "{text}");
        let turns = f.run.turns().unwrap();
        assert_eq!(turns[0].frame.as_deref(), Some("t0001.png"));
        assert_eq!(turns[0].steps, Some([1, 4]));
        assert!(turns[0].first_not_done.is_none() && turns[0].broken.is_none());
        let rj = f.run.load().unwrap();
        assert_eq!((rj.counts.steps, rj.counts.refused, rj.counts.turns), (3, 1, 1));
        // The same refused input again in the next turn is sent again: no broken rule in a visual run.
        assert!(
            do_inputs(&f.run, &mut f.backend, &batch)
                .unwrap()
                .starts_with("harness: note required")
        );
        note(&f.run, "patience 5 flow 5 fine").unwrap();
        assert_eq!(
            do_inputs(&f.run, &mut f.backend, &batch).unwrap(),
            "frame frames/t0002.png\n"
        );
        assert_eq!(f.run.load().unwrap().counts.broken, 0);
        // Looks: the frame again and zooms, three per turn.
        assert_eq!(
            look(&f.run, &mut f.backend, None).unwrap(),
            "frames/t0002-l1.png\n"
        );
        assert_eq!(
            look(&f.run, &mut f.backend, Some([0, 0, 100, 100])).unwrap(),
            "frames/t0002-z2.png\n"
        );
        let last: Value = serde_json::from_str(f.run.transcript().unwrap().last().unwrap()).unwrap();
        assert_eq!(last["frame"], "t0002-z2.png");
        assert!(
            std::fs::read_to_string(f.run.path("steps.uiscript"))
                .unwrap()
                .contains("{\"frame\":\"t0002-z2.png\",\"settle\":false,\"zoom\":[0,0,100,100]}")
        );
        look(&f.run, &mut f.backend, None).unwrap();
        assert_eq!(
            look(&f.run, &mut f.backend, None).unwrap(),
            "harness: look cap per turn\n"
        );
        assert_eq!(f.run.load().unwrap().counts.refusals, 1);
        // A text view is refused and counted; it opens no turn that needs a note.
        assert_eq!(refuse(&f.run, NO_TEXT_VIEW).unwrap(), format!("{NO_TEXT_VIEW}\n"));
        assert_eq!(f.run.load().unwrap().counts.refusals, 2);
        assert_eq!(
            crate::blind::protocol::derive_state(&f.run.turns().unwrap()).pending_note,
            Some(2)
        );
    }

    /// BV7: the input cap ends the run; the application closing ends it `app-closed`, after which a hand-back with
    /// the play-tester's eight headings is filed.
    #[test]
    fn bv7_caps_and_the_closed_application_end_the_run() {
        let mut f = visual_fixture();
        f.run.update(|j| j.caps.steps = 2).unwrap();
        let three = inputs(&[
            json!({"input": "click", "x": 100, "y": 50}),
            json!({"input": "click", "x": 101, "y": 50}),
            json!({"input": "click", "x": 102, "y": 50}),
        ]);
        do_inputs(&f.run, &mut f.backend, &three).unwrap();
        let rj = f.run.load().unwrap();
        assert_eq!((rj.counts.steps, rj.end.as_deref()), (2, Some("cap")));

        let mut f = visual_fixture();
        let out = do_inputs(
            &f.run,
            &mut f.backend,
            &inputs(&[
                json!({"input": "key", "chord": "alt+f4"}),
                json!({"input": "click", "x": 100, "y": 50}),
            ]),
        )
        .unwrap();
        assert_eq!(out, format!("{APP_CLOSED}\n"));
        let rj = f.run.load().unwrap();
        assert_eq!((rj.counts.steps, rj.end.as_deref()), (1, Some("app-closed")));
        assert!(f.run.turns().unwrap()[0].frame.is_none());
        assert_eq!(
            do_inputs(
                &f.run,
                &mut f.backend,
                &inputs(&[json!({"input": "wait", "ms": 100})])
            )
            .unwrap(),
            "harness: the run has ended (app-closed)\n"
        );
        let seven: String = PLAYTEST_HEADINGS[..7]
            .iter()
            .map(|h| format!("## {h}\nx\n"))
            .collect();
        assert!(
            handback(&f.run, &seven)
                .unwrap()
                .contains("8. What I could not read")
        );
        let eight: String = PLAYTEST_HEADINGS.iter().map(|h| format!("## {h}\nx\n")).collect();
        assert_eq!(
            handback(&f.run, &eight).unwrap(),
            "hand-back filed; the run is over\n"
        );
    }
}
