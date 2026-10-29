// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The visual play-test driver (WP-S6.13 part 4, BV2 to BV6): the input vocabulary's bounds, inputs that reach the
//! window's widgets, equal dispatch logs and frames for equal scripts, the frame clock, the jobs and the file dialogs
//! answered for the system, and the application closing.

mod support;

use std::path::{Path, PathBuf};

use egui::{Key, Modifiers, PointerButton};
use ew_ui::ew_viewmodel::script::state_json;
use ew_ui::ew_viewmodel::shell::ShellConfig;
use ew_ui::headless::{HeadlessArgs, ScriptSource};
use ew_ui::visual::{self, Driver, FRAME_HEIGHT, FRAME_WIDTH, Input, parse_chord, parse_frame, parse_input};
use serde_json::{Map, Value, json};
use support::Tmp;

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn args(script: PathBuf, frames: Option<PathBuf>) -> HeadlessArgs {
    HeadlessArgs {
        script: ScriptSource::File(script),
        record: None,
        config_dir: None,
        keep_going: true,
        visual: true,
        frames,
        no_frames: false,
    }
}

/// Runs `lines` (after the header) in the fresh folder `<tmp>/<name>`: the transcript's lines (header first), the
/// exit code and the error output.
fn run(tmp: &Tmp, name: &str, lines: &[Value], frames: bool) -> (Vec<Value>, i32, String) {
    let dir = tmp.0.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let mut text = String::from("{\"$schema\":\"ew.uiscript/1\"}\n");
    for l in lines {
        text.push_str(&l.to_string());
        text.push('\n');
    }
    let script = dir.join("s.uiscript");
    std::fs::write(&script, text).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let a = args(script, frames.then(|| dir.join("frames")));
    let config = ShellConfig::in_dir(&dir.join("config"), "0.0.0-test");
    // The dialogs answer inside the run's folder, never in the crate's (the test's working directory).
    let code = visual::run_in_folder(config, &a, dir.clone(), &mut std::io::empty(), &mut out, &mut err);
    let transcript = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (transcript, code, String::from_utf8(err).unwrap())
}

fn state(d: &mut Driver) -> Value {
    serde_json::from_str(&state_json(d.shell_mut())).unwrap()
}

fn actions(d: &mut Driver) -> Vec<String> {
    d.shell_mut()
        .take_dispatch_log()
        .into_iter()
        .map(|e| e.action)
        .collect()
}

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

fn key(chord: &str) -> Input {
    let (key, mods) = parse_chord(chord).unwrap();
    Input::Key { key, mods, repeat: 1 }
}

/// A driver over a copy of Golden A on folio 101, settled, with its accessibility tree.
fn golden_on_101(tmp: &Tmp) -> Driver {
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, ew_ui::ew_viewmodel::shell::Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    let _ = actions(&mut d);
    d
}

/// BV2: the vocabulary's members and bounds (spec §4.1); the frame line's.
#[test]
fn bv2_the_inputs_check_their_members_and_bounds() {
    let ok = |v: Value| parse_input(&obj(v)).unwrap();
    let err = |v: Value| parse_input(&obj(v)).unwrap_err();
    // The frame's edges are inside, one more is not.
    assert!(matches!(
        ok(json!({"input": "click", "x": 0, "y": 0})),
        Input::Click { .. }
    ));
    assert!(matches!(
        ok(json!({"input": "click", "x": FRAME_WIDTH - 1, "y": FRAME_HEIGHT - 1})),
        Input::Click { .. }
    ));
    assert!(err(json!({"input": "click", "x": FRAME_WIDTH, "y": 0})).contains("outside the frame"));
    assert!(err(json!({"input": "hover", "x": 0, "y": FRAME_HEIGHT})).contains("outside the frame"));
    assert!(err(json!({"input": "click", "x": -1, "y": 0})).contains("outside the frame"));
    assert!(err(json!({"input": "click", "x": 1.5, "y": 0})).contains("integer frame pixel"));
    // The right button, the modifiers.
    assert_eq!(
        ok(json!({"input": "right_click", "x": 3, "y": 4, "mods": "ctrl+shift"})),
        Input::Click {
            at: (3, 4),
            button: PointerButton::Secondary,
            mods: Modifiers::CTRL | Modifiers::SHIFT | Modifiers::COMMAND,
        }
    );
    assert!(err(json!({"input": "click", "x": 3, "y": 4, "mods": "hyper"})).contains("unknown modifier"));
    assert!(
        err(json!({"input": "click", "x": 3, "y": 4, "button": "fourth"})).contains("left, right or middle")
    );
    // Scroll lines, typed text, key repeats and waits.
    assert!(matches!(
        ok(json!({"input": "scroll", "x": 1, "y": 1, "dy": -20})),
        Input::Scroll { .. }
    ));
    assert!(err(json!({"input": "scroll", "x": 1, "y": 1, "dy": 21})).contains("-20 to 20"));
    assert!(err(json!({"input": "scroll", "x": 1, "y": 1})).contains("needs `dx` or `dy`"));
    let long: String = "a".repeat(200);
    assert!(matches!(
        ok(json!({"input": "type", "text": long})),
        Input::Type { .. }
    ));
    assert!(err(json!({"input": "type", "text": "a".repeat(201)})).contains("1 to 200"));
    assert!(err(json!({"input": "type", "text": "a\nb"})).contains("control characters"));
    assert!(err(json!({"input": "type", "text": ""})).contains("1 to 200"));
    assert!(matches!(
        ok(json!({"input": "key", "chord": "ctrl+s", "repeat": 20})),
        Input::Key { repeat: 20, .. }
    ));
    assert!(err(json!({"input": "key", "chord": "ctrl+s", "repeat": 21})).contains("1 to 20"));
    assert!(err(json!({"input": "key", "chord": "ctrl+nokey"})).contains("unknown key"));
    assert_eq!(ok(json!({"input": "wait", "ms": 100})), Input::Wait { ms: 100 });
    assert_eq!(ok(json!({"input": "wait", "ms": 5000})), Input::Wait { ms: 5000 });
    assert!(err(json!({"input": "wait", "ms": 99})).contains("100 to 5000"));
    assert!(err(json!({"input": "wait", "ms": 5001})).contains("100 to 5000"));
    // Unknown inputs and members are refused.
    assert!(err(json!({"input": "teleport"})).contains("unknown input"));
    assert!(err(json!({"input": "hover", "x": 1, "y": 1, "button": "left"})).contains("unknown member"));
    // Chords: egui's key names in any case, the modifiers, `ctrl++`, a modifier alone.
    assert_eq!(
        parse_chord("ctrl+s").unwrap(),
        (Some(Key::S), Modifiers::COMMAND | Modifiers::CTRL)
    );
    assert_eq!(
        parse_chord("shift+tab").unwrap(),
        (Some(Key::Tab), Modifiers::SHIFT)
    );
    assert_eq!(
        parse_chord("arrowdown").unwrap(),
        (Some(Key::ArrowDown), Modifiers::NONE)
    );
    assert_eq!(parse_chord("F2").unwrap(), (Some(Key::F2), Modifiers::NONE));
    assert_eq!(parse_chord("escape").unwrap().0, Some(Key::Escape));
    assert_eq!(parse_chord("ctrl++").unwrap().0, Some(Key::Plus));
    assert_eq!(parse_chord("alt").unwrap(), (None, Modifiers::ALT));
    // The frame line: a plain file name; a zoom of at most 768 × 497 inside the frame.
    let f = parse_frame(&obj(
        json!({"frame": "t0001-z1.png", "settle": false, "zoom": [10, 20, 768, 497]}),
    ))
    .unwrap();
    assert_eq!(
        (f.settle, f.zoom, f.paint),
        (false, Some([10, 20, 768, 497]), true)
    );
    assert!(
        !parse_frame(&obj(json!({"frame": "t1.png", "paint": false})))
            .unwrap()
            .paint
    );
    assert!(parse_frame(&obj(json!({"frame": "t1.png", "paint": 0}))).is_err());
    assert!(parse_frame(&obj(json!({"frame": "../x.png"}))).is_err());
    assert!(parse_frame(&obj(json!({"frame": "t1.png", "zoom": [0, 0, 769, 10]}))).is_err());
    assert!(parse_frame(&obj(json!({"frame": "t1.png", "zoom": [1000, 0, 768, 10]}))).is_err());
}

/// BV3: a click on a known button dispatches its action; a right click on a property row opens its menu; typing
/// into a field sets it; a rubber band selects and a drag from a selected item moves the selection.
#[test]
fn bv3_clicks_right_clicks_typing_and_drags_reach_the_widgets() {
    let tmp = Tmp::new();
    let mut d = golden_on_101(&tmp);
    // A click on the command bar's Check project.
    let at = d.find("Check project").expect("the Check project button");
    d.input(&click(at));
    let done = actions(&mut d);
    assert!(
        done == ["messages.check"],
        "the click dispatched the check: {at:?} {done:?}"
    );
    // A right click on a property row with actions opens its menu (doc 16a §6.8).
    d.input(&key("escape"));
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "1"}));
    d.settle();
    let _ = actions(&mut d);
    let at = d.find("Actions of Title").expect("the Title row's actions");
    d.input(&Input::Click {
        at: (at.0 - 150, at.1),
        button: PointerButton::Secondary,
        mods: Modifiers::NONE,
    });
    let done = actions(&mut d);
    assert!(done.contains(&"props.row_menu".to_owned()), "{done:?}");
    d.input(&key("escape"));
    // Typing into the properties' filter filters on every character and puts the cursor on the first match.
    let at = d.find("Filter properties").expect("the properties' filter");
    d.input(&click(at));
    let _ = actions(&mut d);
    d.input(&Input::Type {
        text: "Title".to_owned(),
    });
    assert_eq!(actions(&mut d), ["props.filter"; 5]);
    assert_eq!(state(&mut d)["properties"]["row"], "page/title");
    d.input(&key("escape"));
    // A rubber band over the page selects; a drag from a selected item moves the selection (one undo step).
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    let _ = d.shell_mut().dispatch("select.clear", &json!({}));
    d.settle();
    let _ = actions(&mut d);
    d.input(&Input::Drag {
        from: (600, 300),
        to: (1300, 800),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    let done = actions(&mut d);
    assert!(done.contains(&"canvas.press".to_owned()) && done.contains(&"canvas.release".to_owned()));
    let selected = state(&mut d)["canvas"]["selection"].as_array().unwrap().len();
    assert!(selected > 0, "the rubber band selected");
    assert_eq!(state(&mut d)["can_undo"], false);
    // A point over a selected item: the first of a coarse grid whose hover names something.
    let mut over = None;
    'scan: for y in (340..780).step_by(40) {
        for x in (640..1260).step_by(40) {
            d.input(&Input::Hover { at: (x, y) });
            if !state(&mut d)["canvas"]["hover"].is_null() {
                over = Some((x, y));
                break 'scan;
            }
        }
    }
    let (x, y) = over.expect("an item under the pointer");
    let _ = actions(&mut d);
    d.input(&Input::Drag {
        from: (x, y),
        to: (x + 48, y + 48),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    let s = state(&mut d);
    assert_eq!(s["can_undo"], true, "the drag moved the selection: {s}");
    assert_eq!(s["dirty"], true);
    d.shell_mut().shutdown();
}

/// BV4: the same script twice in fresh folders gives the same transcript (dispatch logs and states) and, on Windows,
/// the same frames; the visual frame path reads no wall clock.
#[test]
fn bv4_the_same_script_twice_gives_the_same_logs_states_and_frames() {
    let tmp = Tmp::new();
    let script = |dir: &Path| {
        vec![
            json!({"action": "file.new", "params": {"dir": dir.join("p").to_string_lossy()}}),
            json!({"frame": "t0000.png"}),
            json!({"input": "click", "x": 477, "y": 53}),
            json!({"input": "hover", "x": 900, "y": 500}),
            json!({"input": "wait", "ms": 400}),
            json!({"input": "key", "chord": "ctrl+shift+s"}),
            json!({"input": "drag", "x": 600, "y": 300, "to_x": 900, "to_y": 600}),
            json!({"input": "scroll", "x": 900, "y": 500, "dy": 3, "mods": "ctrl"}),
            json!({"frame": "t0001.png"}),
        ]
    };
    let frames = cfg!(windows);
    let (a, code_a, err_a) = run(&tmp, "a", &script(&tmp.0.join("a")), frames);
    let (b, code_b, _) = run(&tmp, "b", &script(&tmp.0.join("b")), frames);
    assert_eq!((code_a, code_b), (0, 0), "{err_a}");
    assert_eq!(a.len(), 10);
    // Every line is equal, the frames' SHA-256 included (no line names the run's folder).
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(x, y, "line {i}");
    }
    assert_eq!(a.len(), b.len());
    let dispatched: Vec<&Value> = a.iter().filter_map(|l| l.get("dispatched")).collect();
    assert!(
        dispatched
            .iter()
            .flat_map(|d| d.as_array().unwrap())
            .any(|e| e["action"] == "canvas.dwell"),
        "the hover's rest dispatched a dwell on the frame clock"
    );
    if frames {
        assert!(a[2]["sha256"].is_string() && a[9]["sha256"].is_string());
        let png = std::fs::read(tmp.0.join("a/frames/t0001.png")).unwrap();
        // 1536 × 994 pixels, from the PNG's header.
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), FRAME_WIDTH);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), FRAME_HEIGHT);
    }
    assert_eq!(
        ew_ui::clock::wall_reads(),
        0,
        "no wall-clock read on the frame clock"
    );
}

/// BV4: the canvas's dwells follow the frame clock: none while the pointer moves, one at 80 ms and one at 300 ms of
/// frame time, however long the machine takes.
#[test]
fn bv4_the_dwells_follow_the_frame_clock() {
    let tmp = Tmp::new();
    let mut d = golden_on_101(&tmp);
    let dwells = |d: &mut Driver| actions(d).iter().filter(|a| *a == "canvas.dwell").count();
    d.input(&Input::Hover { at: (900, 500) });
    let start = d.frame_count();
    assert_eq!(dwells(&mut d), 0, "no rest yet");
    d.input(&Input::Wait { ms: 100 });
    assert_eq!(dwells(&mut d), 1, "the 80 ms dwell");
    d.input(&Input::Wait { ms: 100 });
    assert_eq!(dwells(&mut d), 0, "nothing between 80 and 300 ms");
    d.input(&Input::Wait { ms: 200 });
    assert_eq!(dwells(&mut d), 1, "the 300 ms dwell");
    assert!(d.frame_count() - start >= 24, "the waits ran frames, not time");
    d.shell_mut().shutdown();
}

/// BV5 and BV6: the file dialogs are answered with `project` and `project.pdf` in the run's folder; an export runs to
/// its end within the input that started it; the caret is drawn in every frame.
#[test]
fn bv5_bv6_dialogs_are_answered_and_an_export_ends_within_its_input() {
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    d.settle();
    // New project: the system's dialog is answered with `project`.
    d.input(&key("ctrl+n"));
    let s = state(&mut d);
    assert_eq!(s["project"], "project", "{s}");
    assert!(tmp.0.join("project").is_dir());
    // Export: `project.pdf`, written by the time the input has settled, its toast on show.
    d.input(&key("ctrl+shift+e"));
    assert!(
        tmp.0.join("project.pdf").is_file(),
        "the export ended within its input"
    );
    assert!(!state(&mut d)["toast"].is_null(), "its end is on show");
    assert!(
        actions(&mut d).contains(&"file.export_pdf".to_owned()),
        "the export went through the window's dialog answer"
    );
    d.shell_mut().shutdown();
}

/// BV5 (Windows, where frames are painted): the caret does not blink, so a field with the focus paints the same
/// frame before and after half a second.
#[cfg(windows)]
#[test]
fn bv5_the_caret_is_always_drawn() {
    let tmp = Tmp::new();
    let lines = vec![
        json!({"action": "file.new", "params": {"dir": tmp.0.join("p").to_string_lossy()}}),
        json!({"input": "click", "x": 222, "y": 253}),
        json!({"input": "type", "text": "x"}),
        json!({"frame": "t0001.png"}),
        json!({"input": "wait", "ms": 500}),
        json!({"frame": "t0002.png"}),
        json!({"frame": "t0002-z1.png", "settle": false, "zoom": [100, 200, 300, 100]}),
    ];
    let (t, code, err) = run(&tmp, "c", &lines, true);
    assert_eq!(code, 0, "{err}");
    let shas: Vec<&Value> = t.iter().filter_map(|l| l.get("sha256")).collect();
    assert_eq!(shas[0], shas[1], "the same frame after 500 ms: {t:?}");
    let zoom = std::fs::read(tmp.0.join("c/frames/t0002-z1.png")).unwrap();
    assert_eq!(
        u32::from_be_bytes(zoom[16..20].try_into().unwrap()),
        600,
        "a zoom doubles the region"
    );
    // The sample frame of the report, kept in the target folder.
    let keep = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("visual-sample");
    std::fs::create_dir_all(&keep).unwrap();
    std::fs::copy(tmp.0.join("c/frames/t0002.png"), keep.join("t0002.png")).unwrap();
}

/// The application closes through the window (Ctrl+Q on a clean project): the input says so, later inputs and frames
/// do nothing, and action lines go to a fresh shell (the harness's epilogue).
#[test]
fn the_application_closes_and_later_lines_say_so() {
    let tmp = Tmp::new();
    let p = tmp.0.join("p");
    let lines = vec![
        json!({"action": "file.new", "params": {"dir": p.to_string_lossy()}}),
        json!({"action": "file.save", "params": {}}),
        json!({"input": "key", "chord": "ctrl+q"}),
        json!({"input": "click", "x": 10, "y": 10}),
        json!({"frame": "t0002.png"}),
        json!({"action": "file.open", "params": {"dir": p.to_string_lossy()}}),
        json!({"action": "app.quit", "params": {"discard": true}}),
    ];
    let (t, code, err) = run(&tmp, "q", &lines, false);
    assert_eq!(code, 0, "{err}");
    assert_eq!(t[3]["closed"], true, "{:?}", t[3]);
    assert!(
        t[3]["dispatched"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "app.quit"),
        "{:?}",
        t[3]
    );
    assert_eq!(t[4]["closed"], true);
    assert_eq!(t[4]["dispatched"], json!([]));
    assert_eq!(t[5]["closed"], true);
    assert_eq!(t[6]["outcome"], "done", "the epilogue's shell: {:?}", t[6]);
}

/// The harness (xtask, which has no edge to this crate) checks a chord against its own copy of the key names; every
/// name of that copy must be a key here, alone and after modifiers.
#[test]
fn the_harness_key_names_are_keys() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../xtask/src/blind/visual.rs"),
    )
    .unwrap();
    let start = source.find("pub const KEY_NAMES").unwrap();
    let body = &source[start..start + source[start..].find("];").unwrap()];
    let body = &body[body.find("= &[").unwrap() + 4..];
    let names: Vec<&str> = body
        .split("\",")
        .map(|n| n.trim().trim_matches('"'))
        .filter(|n| !n.is_empty())
        .collect();
    assert!(names.len() > 80, "{names:?}");
    for name in &names {
        let (k, _) = parse_chord(name).unwrap_or_else(|e| panic!("`{name}`: {e}"));
        assert!(k.is_some(), "`{name}` is a key");
        let (k, m) = parse_chord(&format!("ctrl+shift+{name}")).unwrap_or_else(|e| panic!("`{name}`: {e}"));
        assert!(k.is_some() && m.ctrl && m.shift, "`ctrl+shift+{name}`");
    }
}
