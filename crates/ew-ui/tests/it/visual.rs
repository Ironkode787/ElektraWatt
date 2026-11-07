// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The visual play-test driver (WP-S6.13 part 4, BV2 to BV6): the input vocabulary's bounds, inputs that reach the
//! window's widgets, equal dispatch logs and frames for equal scripts, the frame clock, the jobs and the file dialogs
//! answered for the system, and the application closing.

use crate::support;

use std::path::{Path, PathBuf};

use egui::{Key, Modifiers, PointerButton};
use ew_ui::ew_viewmodel::script::state_json;
use ew_ui::ew_viewmodel::shell::ShellConfig;
use ew_ui::headless::{HeadlessArgs, ScriptSource};
use ew_ui::visual::{
    self, Driver, FRAME_HEIGHT, FRAME_WIDTH, Geometry, Input, parse_chord, parse_frame, parse_input,
    parse_input_in,
};
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
        geometry: Geometry::DEFAULT,
    }
}

/// Runs `lines` (after the header) in the fresh folder `<tmp>/<name>`: the transcript's lines (header first), the
/// exit code and the error output.
fn run(tmp: &Tmp, name: &str, lines: &[Value], frames: bool) -> (Vec<Value>, i32, String) {
    run_at(tmp, name, lines, frames, Geometry::DEFAULT)
}

/// [`run`] at the window size and scale of `geometry`.
fn run_at(
    tmp: &Tmp,
    name: &str,
    lines: &[Value],
    frames: bool,
    geometry: Geometry,
) -> (Vec<Value>, i32, String) {
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
    let mut a = args(script, frames.then(|| dir.join("frames")));
    a.geometry = geometry;
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

/// FT-1188: a script's `ui.dismiss` action line with the app menu open closes the menu and nothing else, as the
/// window's own `ui.dismiss` does (FT-1130): the selection stays; with the menu closed it dismisses as before.
#[test]
fn a_scripts_ui_dismiss_with_the_app_menu_open_closes_only_the_menu() {
    let tmp = Tmp::new();
    let golden = tmp.golden("g.ew");
    let lines = [
        json!({"action": "file.open", "params": {"dir": golden.to_string_lossy()}}),
        json!({"action": "page.goto", "params": {"folio": "101"}}),
        json!({"action": "select.set", "params": {"refs": ["tag:F55"]}}),
        json!({"input": "key", "chord": "f10"}),
        json!({"action": "ui.dismiss"}),
        json!({"action": "ui.dismiss"}),
    ];
    let (t, code, err) = run(&tmp, "dismiss", &lines, false);
    assert_eq!((code, err.as_str()), (0, ""), "{t:?}");
    let selected = |v: &Value| v["state"]["canvas"]["selection"].as_array().map(Vec::len);
    assert_eq!(t[5]["outcome"], "done", "{}", t[5]);
    assert_eq!(selected(&t[5]), selected(&t[3]), "the menu took it: {}", t[5]);
    assert_eq!(selected(&t[3]), Some(1), "{}", t[3]);
    assert_eq!(selected(&t[6]), Some(0), "then it dismisses: {}", t[6]);
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
    // A corner near u32::MAX is refused, not added with overflow (a debug panic, a wrapped slice in release).
    for zoom in [
        json!([4_294_967_295_u32, 0, 10, 10]),
        json!([0, 4_294_967_295_u32, 10, 10]),
    ] {
        assert!(parse_frame(&obj(json!({"frame": "t1.png", "zoom": zoom}))).is_err());
    }
}

/// FT-891, proof (2): the driver's `double_click` on the Pages navigator's row of 100 shows 100, and the next one, on
/// 101, shows 101: the driver pauses before a double click, so egui does not count it as a triple click with the one
/// before (both ran a few frames apart).
#[test]
fn ft891_the_drivers_double_clicks_on_two_rows_open_both() {
    let tmp = Tmp::new();
    let mut d = golden_on_101(&tmp);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "102"}));
    d.settle();
    let row = |d: &Driver, folio: &str| {
        let prefix = format!("{folio} ");
        d.labels()
            .into_iter()
            .find(|(l, _)| l.starts_with(&prefix) && l.contains(", page"))
            .unwrap_or_else(|| panic!("no page row {folio}"))
            .1
    };
    for folio in ["100", "101"] {
        let at = row(&d, folio);
        d.input(&Input::DoubleClick {
            at,
            mods: Modifiers::NONE,
        });
        assert_eq!(
            d.app().unwrap().shell().current_folio(),
            Some(folio),
            "a double click on {folio}"
        );
    }
}

/// BV3: a click on a known button dispatches its action; a right click on a property row opens its menu; typing
/// into a field sets it; a rubber band selects and a drag from a selected item moves the selection.
#[test]
fn bv3_clicks_right_clicks_typing_and_drags_reach_the_widgets() {
    let tmp = Tmp::new();
    let mut d = golden_on_101(&tmp);
    // The drags below aim at Fit page's view (FT-1443: the first view is fit-width).
    let _ = d.shell_mut().dispatch("view.fit_page", &json!({}));
    d.settle();
    let _ = actions(&mut d);
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

/// FT-1201: `answer_open` names the file the next open dialog answers with, resolved against the script's folder, and
/// runs no frame; the QElectroTech import then reads that file and its card shows. Without it the same import is
/// cancelled.
#[test]
fn answer_open_names_the_file_of_the_next_open_dialog() {
    let tmp = Tmp::new();
    let scripts = tmp.0.join("scripts");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(
        scripts.join("plant.qet"),
        r#"<project version="0.100" title="Plant"><diagram title="Supply" folio="1" order="1" cols="17" colsize="60" rows="8" rowsize="80" height="640"/></project>"#,
    )
    .unwrap();
    let import = |answer: bool| {
        let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
        d.set_script_dir(scripts.clone());
        d.settle();
        let _ = actions(&mut d);
        if answer {
            let start = d.frame_count();
            d.input(&parse_input(&obj(json!({"input": "answer_open", "path": "plant.qet"}))).unwrap());
            assert_eq!(d.frame_count(), start, "no frame runs");
        }
        let _ = d.shell_mut().dispatch("file.import_qet", &json!({}));
        d.input(&Input::Wait { ms: 100 });
        assert!(d.shell_mut().wait_jobs().is_empty());
        d.input(&Input::Wait { ms: 100 });
        d
    };
    let mut d = import(true);
    let card = format!("{:?}", d.shell_mut().dialog());
    assert!(card.starts_with("Some(ImportQet"), "the import's card: {card}");
    assert!(card.contains("plant.qet"), "of the answered file: {card}");
    d.shell_mut().shutdown();
    let mut d = import(false);
    assert!(d.shell_mut().dialog().is_none(), "cancelled");
    assert_eq!(
        actions(&mut d),
        ["file.import_qet"],
        "only the step's own dispatch"
    );
    d.shell_mut().shutdown();
    assert_eq!(
        parse_input(&obj(json!({"input": "answer_open", "path": ""}))),
        Err("`path` names the file, 1 to 260 characters without control characters".to_owned())
    );
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

/// 1920 × 1080 points at 1 pixel per point (cycle 39).
fn typical() -> Geometry {
    Geometry::parse(Some("1920x1080"), Some("1")).unwrap()
}

/// Cycle 39: a run may choose its window size and scale. At 1920 × 1080 points and ppp 1 the frame is 1920 × 1108
/// pixels (a 28-pixel strip), at ppp 2 3840 × 2216; the default stays 1536 × 994.
#[test]
fn the_geometry_sizes_the_frame() {
    let g = typical();
    assert_eq!(
        (g.frame_width(), g.frame_height(), g.strip_px(), g.dpi()),
        (1920, 1108, 28, 96)
    );
    let g2 = Geometry::parse(Some("1920x1080"), Some("2")).unwrap();
    assert_eq!(
        (g2.frame_width(), g2.frame_height(), g2.strip_px(), g2.dpi()),
        (3840, 2216, 56, 192)
    );
    let d = Geometry::DEFAULT;
    assert_eq!(
        (d.frame_width(), d.frame_height(), d.strip_px(), d.dpi(), d.ppp()),
        (FRAME_WIDTH, FRAME_HEIGHT, visual::STRIP_PX, 115, visual::PPP)
    );
    assert_eq!(Geometry::parse(None, None), Ok(Geometry::DEFAULT));
    assert_eq!(
        Geometry::parse(Some("1280x800"), Some("1.2")),
        Ok(Geometry::DEFAULT)
    );
}

/// Cycle 39: an input at x = 1900 is inside a 1920-pixel frame and outside the default one, with the existing
/// out-of-frame text.
#[test]
fn coordinates_are_checked_against_the_runs_frame() {
    let o = obj(json!({"input": "click", "x": 1900, "y": 1100}));
    assert!(matches!(
        parse_input_in(&o, typical()),
        Ok(Input::Click { at: (1900, 1100), .. })
    ));
    let refused = parse_input(&o).unwrap_err();
    assert_eq!(refused, "`x` is outside the frame (0 to 1535)");
    assert!(parse_input_in(&obj(json!({"input": "click", "x": 1920, "y": 0})), typical()).is_err());
    let zoom = obj(json!({"frame": "z.png", "zoom": [1100, 600, 768, 497]}));
    assert!(visual::parse_frame_in(&zoom, typical()).is_ok());
    assert!(parse_frame(&zoom).is_err());
}

/// Cycle 39: at 1920 × 1080 points and ppp 1 a click at a widget's centre in that frame's pixels reaches the widget.
#[test]
fn a_click_at_the_typical_size_reaches_the_widget() {
    let tmp = Tmp::new();
    let golden = tmp.golden("g.ew");
    let mut d = Driver::with_geometry(tmp.config(), None, tmp.0.clone(), typical()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, ew_ui::ew_viewmodel::shell::Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    let _ = actions(&mut d);
    let at = d.find("Check project").expect("the Check project button");
    assert!(at.0 < 1920 && at.1 < 1108, "{at:?}");
    d.input(&click(at));
    assert_eq!(actions(&mut d), ["messages.check"], "the click at {at:?}");
    d.shell_mut().shutdown();
}

/// Cycle 39: a run at 1920 × 1080 points and ppp 1 names its geometry in the transcript's header, accepts an input at
/// x = 1900 and (where frames are painted) paints 1920 × 1108 pixels; at the default the header is unchanged.
#[test]
fn a_run_at_the_typical_size_paints_its_frame_and_names_it() {
    let tmp = Tmp::new();
    let lines = |dir: &Path| {
        vec![
            json!({"action": "file.new", "params": {"dir": dir.join("p").to_string_lossy()}}),
            json!({"input": "hover", "x": 1900, "y": 600}),
            json!({"frame": "t0001.png"}),
        ]
    };
    let frames = cfg!(windows);
    let (t, code, err) = run_at(&tmp, "w", &lines(&tmp.0.join("w")), frames, typical());
    assert_eq!(code, 0, "{err}");
    assert_eq!(t[0]["window"], json!([1920, 1080]), "{:?}", t[0]);
    assert_eq!(t[0]["ppp"], json!(1));
    assert!(t[2].get("input").is_some(), "the hover ran: {:?}", t[2]);
    if frames {
        let png = std::fs::read(tmp.0.join("w/frames/t0001.png")).unwrap();
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1920);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 1108);
    }
    assert_eq!(
        visual::transcript_header("1.0", Geometry::parse(Some("1366x768"), Some("1.25")).unwrap()),
        r#"{"$schema":"ew.uitranscript/1","version":"1.0","window":[1366,768],"ppp":1.25}"#
    );
    // The default run's header has no geometry, and its input at x = 1900 is refused.
    let (t, code, _) = run(&tmp, "d", &lines(&tmp.0.join("d")), false);
    assert_eq!(code, 2);
    assert!(
        t[0].get("window").is_none() && t[0].get("ppp").is_none(),
        "{:?}",
        t[0]
    );
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

/// FT-1253: the driver's fixed rendering inputs hold whatever the settings folder holds: no minimum text size and
/// reduced motion on, after a session that set 12 pt and motion.
#[test]
fn the_driver_pins_the_minimum_text_size_and_reduced_motion() {
    use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    assert_eq!(shell.dispatch("view.min_font", &json!({"pt": 12})), Outcome::Done);
    assert_eq!(
        shell.dispatch("view.reduce_motion", &json!({"on": false})),
        Outcome::Done
    );
    shell.shutdown();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    assert_eq!(d.shell_mut().settings().min_font_pt, 0);
    assert!(d.shell_mut().reduce_motion());
}

/// WP-S2.10 review: named variants/pin controls and keyboard editing/creation use the real window.
#[test]
fn symbol_editor_accessible_controls_and_numeric_keyboard_creation() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    assert_eq!(
        d.shell_mut().dispatch(
            "symbol.new",
            &json!({"key":"example.keyboard", "name":"Keyboard"})
        ),
        Outcome::Done
    );
    assert_eq!(
        d.shell_mut().dispatch(
            "symbol.cp",
            &json!({"x":0,"y":0,"designation":"FE","direction":"left"})
        ),
        Outcome::Done
    );
    d.settle();
    for variant in 'A'..='H' {
        let name = format!("Keyboard, variant {variant}, connection points FE");
        assert!(d.find(&name).is_some(), "{name}: {:?}", d.labels());
    }
    assert!(d.find("Drawing of Keyboard").is_some());
    d.input(&click(d.find("Electrical data").unwrap()));
    for name in [
        "Designation 1",
        "Potential role FE",
        "Source output FE",
        "May remain unconnected FE",
        "Function type",
    ] {
        assert!(d.find(name).is_some(), "missing accessible control {name}");
    }
    d.input(&click(d.find("Connection points (1)").unwrap()));
    for name in [
        "x (mm) of Connection point FE",
        "y (mm) of Connection point FE",
        "Direction of Connection point FE",
    ] {
        assert!(
            d.find(name).is_some(),
            "missing accessible control {name}: {:?}",
            d.labels()
        );
    }
    d.input(&click(d.find("Drawing").unwrap()));
    d.input(&click(d.find("Line").unwrap()));
    // Clicking enters numeric editing; type the endpoint and reach Add with the keyboard.
    let at = d.find("End x (mm)").unwrap_or_else(|| panic!("{:?}", d.labels()));
    d.input(&click(at));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type { text: "8".into() });
    d.input(&key("tab"));
    d.input(&key("tab"));
    d.input(&key("space"));
    assert!(actions(&mut d).contains(&"symbol.draw".to_owned()));
    let draft = &d.shell_mut().symbol_editor().unwrap().draft;
    let graphics = serde_json::to_value(&draft.symbol.graphics).unwrap();
    assert_eq!(graphics[0]["line"]["to"], json!([8_000_000, 0]));
    d.shell_mut().shutdown();
}

#[test]
fn symbol_editor_drag_and_keyboard_mapping_use_the_same_undoable_action() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    for (action, params) in [
        ("symbol.new", json!({"key":"example.mapping","name":"Contact"})),
        ("symbol.function", json!({"key":"ew.fn.contact-no"})),
        (
            "symbol.cp",
            json!({"idx":1,"fn_cp":0,"x":0,"y":0,"direction":"up"}),
        ),
        (
            "symbol.cp",
            json!({"idx":2,"fn_cp":1,"x":0,"y":8,"direction":"down"}),
        ),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done);
    }
    d.settle();
    d.input(&click(d.find("Electrical data").unwrap()));
    let names: Vec<_> = d
        .shell_mut()
        .symbol_editor()
        .unwrap()
        .draft
        .function
        .cps
        .iter()
        .map(|p| p.designation.to_string())
        .collect();
    let from = d.find(&format!("Function pin {}", names[1])).unwrap();
    let to = d.find("Connection point 1").unwrap();
    d.input(&Input::Drag {
        from,
        to,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    assert!(actions(&mut d).contains(&"symbol.map_cp".to_owned()));
    assert_eq!(
        d.shell_mut()
            .symbol_editor()
            .unwrap()
            .draft
            .symbol
            .cps
            .get(&1)
            .unwrap()
            .fn_cp,
        1
    );
    d.input(&key("ctrl+z"));
    assert_eq!(
        d.shell_mut()
            .symbol_editor()
            .unwrap()
            .draft
            .symbol
            .cps
            .get(&1)
            .unwrap()
            .fn_cp,
        0
    );
    d.input(&click(d.find("Map connection point 1").unwrap()));
    d.input(&click(d.find(&names[1]).unwrap()));
    assert_eq!(
        d.shell_mut()
            .symbol_editor()
            .unwrap()
            .draft
            .symbol
            .cps
            .get(&1)
            .unwrap()
            .fn_cp,
        1
    );
    d.shell_mut().shutdown();
}

#[test]
fn symbol_editor_forks_a_named_library_symbol_through_the_window() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    assert_eq!(
        d.shell_mut()
            .dispatch("symbol.new", &json!({"key":"example.empty","name":"Empty"})),
        Outcome::Done
    );
    let source = d
        .shell_mut()
        .symbol_editor()
        .unwrap()
        .sources
        .iter()
        .find(|s| s.key.as_deref() == Some("ew.iec.contact-no"))
        .unwrap();
    let name = match source.name.view() {
        ew_ui::ew_viewmodel::ew_api::symbols::MlTextView::Neutral(s) => s.to_owned(),
        ew_ui::ew_viewmodel::ew_api::symbols::MlTextView::Langs(m) => m.values().next().unwrap().to_owned(),
    };
    let library_name = source.library.clone().unwrap();
    d.settle();
    d.input(&click(d.find("Library").unwrap()));
    d.input(&click(d.find("Find symbol by name").unwrap()));
    d.input(&Input::Type { text: name.clone() });
    d.input(&click(d.find("Choose a symbol").unwrap()));
    d.input(&click(
        d.find(&format!("{name} · 4 mm · {library_name}"))
            .unwrap_or_else(|| panic!("source menu: {:?}", d.labels())),
    ));
    d.input(&click(d.find("New symbol key").unwrap()));
    d.input(&Input::Type {
        text: "example.copy".into(),
    });
    d.input(&click(d.find("Create editable copy").unwrap()));
    // M25-p3#7: the fresh draft has no unsaved changes, so the fork asks nothing.
    assert!(d.find("Discard changes").is_none());
    assert!(actions(&mut d).contains(&"symbol.fork".to_owned()));
    let draft = &d.shell_mut().symbol_editor().unwrap().draft;
    assert_eq!(draft.key, "example.copy");
    assert!(!draft.symbol.graphics.is_empty());
    assert!(draft.function_ref.is_none());
    d.shell_mut().shutdown();
}

#[test]
fn symbol_editor_duplicate_designations_remain_distinguishable_when_mapping() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    for (action, params) in [
        ("symbol.new", json!({"key":"example.mcb","name":"Breaker"})),
        ("symbol.function", json!({"key":"ew.fn.mcb-1pn"})),
        ("symbol.cp", json!({"idx":1,"fn_cp":2,"x":0,"y":0})),
        ("symbol.cp", json!({"idx":2,"fn_cp":3,"x":0,"y":4})),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done);
    }
    d.settle();
    d.input(&click(d.find("Electrical data").unwrap()));
    assert!(d.find("Function pin N (3)").is_some());
    assert!(d.find("Function pin N (4)").is_some());
    d.input(&click(d.find("Map connection point 1").unwrap()));
    assert!(d.find("N (3)").is_some());
    d.input(&click(d.find("N (4)").unwrap()));
    let draft = &d.shell_mut().symbol_editor().unwrap().draft;
    assert_eq!(draft.symbol.cps.get(&1).unwrap().fn_cp, 3);
    assert_eq!(draft.symbol.cps.get(&2).unwrap().fn_cp, 2);
    d.shell_mut().shutdown();
}

#[test]
fn symbol_editor_save_refuses_off_grid_pin_in_words_and_saves_fine_graphics() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let library = tmp.0.join("fine.ewlib");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    for (action, params) in [
        ("symbol.new", json!({"key":"example.grid","name":"Grid"})),
        (
            "symbol.draw",
            json!({"tool":"line","x":0.5,"y":0.5,"x2":8.5,"y2":0.5}),
        ),
        (
            "symbol.cp",
            json!({"idx":1,"designation":"1","x":4.5,"y":0,"direction":"up"}),
        ),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done);
    }
    d.settle();
    d.input(&click(d.find("Library").unwrap()));
    d.input(&click(d.find("Library folder").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type {
        text: library.to_string_lossy().into_owned(),
    });
    d.input(&click(d.find("Save to library").unwrap()));
    assert!(actions(&mut d).contains(&"symbol.save".to_owned()));
    assert_eq!(
        d.shell_mut().symbol_editor().unwrap().error.as_deref(),
        Some("Connection point 1 is off the connection grid. Move it onto the grid before saving.")
    );
    assert!(!library.exists(), "invalid save created a library");
    assert_eq!(
        d.shell_mut().dispatch("symbol.cp", &json!({"idx":1,"x":4,"y":0})),
        Outcome::Done
    );
    d.settle();
    d.input(&click(d.find("Save to library").unwrap()));
    let editor = d.shell_mut().symbol_editor().unwrap();
    assert_eq!(editor.error, None);
    assert!(!editor.dirty());
    assert!(library.is_dir());
    let graphics = serde_json::to_value(&editor.draft.symbol.graphics).unwrap();
    assert_eq!(graphics[0]["line"]["from"], json!([500_000, 500_000]));
    d.shell_mut().shutdown();
}

#[test]
fn symbol_editor_pointer_tools_dispatch_the_drawing_tool_name() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    assert_eq!(
        d.shell_mut()
            .dispatch("symbol.new", &json!({"key":"example.pointer","name":"Pointer"})),
        Outcome::Done
    );
    d.settle();
    d.input(&click(d.find("Line").unwrap()));
    d.input(&click((700, 400)));
    d.input(&click((800, 450)));
    assert!(actions(&mut d).contains(&"symbol.draw".to_owned()));
    assert_eq!(
        d.shell_mut().symbol_editor().unwrap().draft.symbol.graphics.len(),
        1
    );
    assert_eq!(d.shell_mut().symbol_editor().unwrap().error, None);
    d.input(&click(d.find("Select").unwrap()));
    d.input(&click(d.find("Line 1").unwrap()));
    // ID-1477 (9): the canvas fits only when asked.
    d.input(&click(d.find("Fit drawing").unwrap()));
    let before = d.shell_mut().symbol_editor().unwrap().draft.clone();
    // M25-p3#3: the drag starts on the line (the fitted drawing's centre).
    let on_line = d.find("Drawing of Pointer").unwrap();
    d.input(&Input::Drag {
        from: on_line,
        to: (on_line.0 + 80, on_line.1 + 40),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    let after = d.shell_mut().symbol_editor().unwrap().draft.clone();
    assert_ne!(after.symbol.graphics, before.symbol.graphics);
    d.input(&key("ctrl+z"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, before);
    d.input(&key("delete"));
    assert!(
        d.shell_mut()
            .symbol_editor()
            .unwrap()
            .draft
            .symbol
            .graphics
            .is_empty()
    );
    d.input(&key("ctrl+z"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, before);
    d.input(&click(d.find("Anchor").unwrap()));
    d.input(&click((700, 400)));
    let anchor = serde_json::to_value(&d.shell_mut().symbol_editor().unwrap().draft.symbol.anchor).unwrap();
    assert!(anchor.get("named").is_some());
    d.shell_mut().shutdown();
}

#[test]
fn symbol_editor_numeric_properties_commit_complete_text_as_one_undo() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    assert_eq!(
        d.shell_mut()
            .dispatch("symbol.new", &json!({"key":"example.numeric","name":"Numeric"})),
        Outcome::Done
    );
    assert_eq!(
        d.shell_mut().dispatch(
            "symbol.draw",
            &json!({"tool":"rectangle","x":-12,"y":-12,"x2":12,"y2":12})
        ),
        Outcome::Done
    );
    d.settle();
    d.input(&click(d.find("Rectangle 1").unwrap()));
    let before = d.shell_mut().symbol_editor().unwrap().draft.clone();
    let at = d
        .find("Corner 1 x (mm) of Rectangle 1")
        .unwrap_or_else(|| panic!("{:?}", d.labels()));
    d.input(&Input::DoubleClick {
        at,
        mods: Modifiers::NONE,
    });
    d.input(&key("ctrl+a"));
    d.input(&Input::Type { text: "-13.5".into() });
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, before);
    d.input(&key("tab"));
    let graphic =
        serde_json::to_value(&d.shell_mut().symbol_editor().unwrap().draft.symbol.graphics[0]).unwrap();
    assert_eq!(graphic["rect"]["rect"][0][0], json!(-13_500_000));
    d.input(&key("escape"));
    d.input(&key("ctrl+z"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, before);
    d.input(&click(d.find("Fill shape of Rectangle 1").unwrap()));
    let filled =
        serde_json::to_value(&d.shell_mut().symbol_editor().unwrap().draft.symbol.graphics[0]).unwrap();
    assert_eq!(filled["rect"]["fill"], json!("by_layer"));
    d.input(&key("ctrl+z"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, before);
    d.shell_mut().shutdown();
}

#[test]
fn symbol_editor_name_commits_on_mouse_save_and_new_draft_has_no_stale_text() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    for (action, params) in [
        ("symbol.new", json!({"key":"example.name"})),
        ("symbol.draw", json!({"tool":"line","x":0,"y":0,"x2":4,"y2":0})),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done);
    }
    d.settle();
    d.input(&click(d.find("Library").unwrap()));
    d.input(&click(d.find("Library folder").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type {
        text: tmp.0.join("names.ewlib").to_string_lossy().into_owned(),
    });
    d.input(&click(d.find("Name").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type {
        text: "Committed name".into(),
    });
    // No Enter or Tab: the toolbar's mouse click must commit and save the visible value.
    d.input(&click(d.find("Save to library").unwrap()));
    let editor = d.shell_mut().symbol_editor().unwrap();
    assert_eq!(
        serde_json::to_value(&editor.draft.symbol.name).unwrap(),
        json!({"en":"Committed name"})
    );
    assert_eq!(editor.error, None);
    assert!(!editor.dirty());
    d.input(&click(d.find("Close editor").unwrap()));
    d.settle();
    assert_eq!(
        d.shell_mut()
            .dispatch("symbol.new", &json!({"key":"example.name"})),
        Outcome::Done
    );
    d.input(&Input::Hover { at: (600, 300) });
    d.settle();
    d.input(&click(
        d.find("Name").unwrap_or_else(|| panic!("{:?}", d.labels())),
    ));
    // Focusing and leaving the fresh field must not resurrect the previous buffer.
    d.input(&key("tab"));
    assert_eq!(
        serde_json::to_value(&d.shell_mut().symbol_editor().unwrap().draft.symbol.name).unwrap(),
        json!({"en":"New symbol"})
    );
    d.shell_mut().shutdown();
}

/// The supply-block acceptance is drawn through the window's coordinate controls, then saved and placed.
/// The source picture stays private; the reusable fixture contains only the generic electrical drawing.
/// G-M2.5-01 (M25-p3#12): A–H are checked pin by pin; a scratch copy in variant B and the published library symbol are
/// placed, each as its preview shows it, and both are wired and reported.
#[test]
fn symbol_editor_supply_block_is_drawn_saved_and_placed_through_the_window() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    fn enter(d: &mut Driver, name: &str, value: &str) {
        let at = d
            .labels()
            .into_iter()
            .filter(|(n, _)| n == name)
            .map(|(_, p)| p)
            .next_back()
            .unwrap_or_else(|| panic!("missing {name}: {:?}", d.labels()));
        d.input(&click(at));
        d.input(&key("ctrl+a"));
        d.input(&Input::Type {
            text: value.to_owned(),
        });
        d.input(&key("tab"));
    }
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    assert_eq!(
        d.shell_mut().dispatch(
            "file.new",
            &json!({"dir":tmp.0.join("supply.ew"),"template":"builtin"})
        ),
        Outcome::Done
    );
    assert_eq!(
        d.shell_mut().dispatch(
            "symbol.new",
            &json!({"key":"example.supply-block","name":"Supply block"})
        ),
        Outcome::Done
    );
    d.settle();
    d.input(&click(d.find("Graphics (0)").unwrap()));
    let solution = include_str!("../../../../tools/blind/tasks/brief-symbol-editor/solution.uiscript");
    for row in solution
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
    {
        let action = row["action"].as_str().unwrap();
        let p = &row["params"];
        if !matches!(action, "symbol.draw" | "symbol.cp" | "symbol.slot") {
            continue;
        }
        let tool = match action {
            "symbol.cp" => "Connection point",
            "symbol.slot" => "Text slot",
            _ => match p["tool"].as_str().unwrap() {
                "line" => "Line",
                "arc" => "Arc",
                "circle" => "Circle",
                "text" => "Text",
                "anchor" => "Anchor",
                _ => panic!("fixture tool"),
            },
        };
        if tool == "Arc" {
            d.input(&click(d.find("More shapes").unwrap()));
        }
        d.input(&click(d.find(tool).unwrap()));
        let two_points = matches!(tool, "Line" | "Arc" | "Circle");
        for (member, label) in [
            ("x", if two_points { "Start x (mm)" } else { "x (mm)" }),
            ("y", if two_points { "Start y (mm)" } else { "y (mm)" }),
            ("x2", "End x (mm)"),
            ("y2", "End y (mm)"),
        ] {
            if let Some(value) = p.get(member) {
                enter(&mut d, label, &value.to_string());
            }
        }
        if tool == "Text" {
            enter(&mut d, "Text", p["text"].as_str().unwrap());
        }
        if tool == "Arc" {
            enter(&mut d, "Arc sweep (degrees)", "-180");
        }
        if tool == "Connection point" {
            enter(&mut d, "Designation", p["designation"].as_str().unwrap());
            d.input(&click(d.find("Direction").unwrap()));
            d.input(&click(
                d.find(if p["direction"] == "up" {
                    "Facing up"
                } else {
                    "Facing right"
                })
                .unwrap_or_else(|| panic!("direction menu: {:?}", d.labels())),
            ));
        }
        if tool == "Text slot" {
            let kind = if p["kind"].is_object() {
                "Pin designation"
            } else if p["kind"] == "tag" {
                "Device tag"
            } else {
                "Cross-reference"
            };
            d.input(&click(d.find("Text field").unwrap()));
            d.input(&click(d.find(kind).unwrap()));
            if let Some(pin) = p["kind"].get("cp_designation") {
                enter(&mut d, "Pin number", &pin.to_string());
            }
        }
        d.input(&click(d.find("Add").unwrap()));
        assert_eq!(d.shell_mut().symbol_editor().unwrap().error, None, "{row}");
    }
    d.input(&click(d.find("Electrical data").unwrap()));
    d.input(&click(d.find("Pin mapping (6)").unwrap()));
    for (pin, role) in [
        ("1", "M"),
        ("2", "M"),
        ("3", "PE"),
        ("4", "L+"),
        ("5", "L+"),
        ("FE", "FE"),
    ] {
        d.input(&click(d.find(&format!("Potential role {pin}")).unwrap()));
        if d.find(role).is_none() {
            let at = d
                .find("L1")
                .or_else(|| d.find("Unassigned"))
                .unwrap_or_else(|| panic!("role menu for {pin}: {:?}", d.labels()));
            d.input(&Input::Scroll {
                at,
                dx: 0,
                dy: 6,
                mods: Modifiers::NONE,
            });
        }
        d.input(&click(
            d.find(role)
                .unwrap_or_else(|| panic!("role {role}: {:?}", d.labels())),
        ));
    }
    let authored = d.shell_mut().symbol_editor().unwrap().draft.clone();
    assert!(authored.validate().is_empty());
    assert_eq!(authored.symbol.cps.len(), 6);
    for (i, x) in [(1, 12), (2, 20), (3, 28), (4, 36), (5, 44)] {
        let pin = authored.symbol.cps.get(&i).unwrap();
        assert_eq!((pin.pos.x.0, pin.pos.y.0), (x * 1_000_000, -4_000_000));
    }
    let fe = authored.symbol.cps.get(&6).unwrap();
    assert_eq!((fe.pos.x.0, fe.pos.y.0), (60_000_000, 12_000_000));
    assert_eq!(authored.function.cps.get(&6).unwrap().designation.as_str(), "FE");
    assert_eq!(
        serde_json::to_value(&authored.symbol.anchor).unwrap(),
        json!({"named":[0,10_000_000]})
    );
    // A–H (D4, M25-p3#12): the variant the window shows previews every pin with its index, its function pin, A's
    // position under the variant's transform and its direction turned with it.
    use ew_ui::ew_viewmodel::ew_api::symbols::schema::Dir4;
    use ew_ui::ew_viewmodel::ew_api::symbols::units::{Nm, Orient2, Pt2};
    let orients = [
        Orient2::R0,
        Orient2::R90,
        Orient2::R180,
        Orient2::R270,
        Orient2::M0,
        Orient2::M90,
        Orient2::M180,
        Orient2::M270,
    ];
    let unit = |d: &Dir4| {
        let (x, y) = match d {
            Dir4::Up => (0, -1),
            Dir4::Right => (1, 0),
            Dir4::Down => (0, 1),
            Dir4::Left => (-1, 0),
            other => panic!("direction {other:?}"),
        };
        Pt2 { x: Nm(x), y: Nm(y) }
    };
    let a = authored.preview(0).unwrap();
    // M25-p3#8 (doc 07 §5): a preview is named by the symbol, its variant and its pins.
    let variant_name = |v: char| format!("Supply block, variant {v}, connection points 1, 2, 3, 4, 5, FE");
    for variant in 'A'..='H' {
        d.input(&click(d.find(&variant_name(variant)).unwrap()));
        let editor = d.shell_mut().symbol_editor().unwrap();
        let v = variant as u8 - b'A';
        assert_eq!(editor.variant, v);
        let shown = editor.draft.preview(editor.variant).unwrap();
        let o = orients[usize::from(v)];
        let expected: Vec<_> = a
            .cps
            .iter()
            .map(|c| (c.idx, c.fn_cp, o.apply(c.pos), o.apply(unit(&c.dir))))
            .collect();
        let got: Vec<_> = shown
            .cps
            .iter()
            .map(|c| (c.idx, c.fn_cp, c.pos, unit(&c.dir)))
            .collect();
        assert_eq!(got, expected, "variant {variant}");
    }
    // B, worked by hand: pin 1 at (12, -4) facing up stands at (4, 12) facing right; FE at (60, 12) facing right
    // stands at (-12, 60) facing down.
    let b = authored.preview(1).unwrap();
    let pin = |g: &ew_ui::ew_viewmodel::ew_api::symbols::SymbolGeometry, idx: u16| {
        let c = g.cps.iter().find(|c| c.idx == idx).unwrap();
        ((c.pos.x.0, c.pos.y.0), c.dir.clone())
    };
    assert_eq!(pin(&b, 1), ((4_000_000, 12_000_000), Dir4::Right));
    assert_eq!(pin(&b, 6), ((-12_000_000, 60_000_000), Dir4::Down));
    // The scratch copy is placed as the selected variant B.
    d.input(&click(d.find(&variant_name('B')).unwrap()));
    d.input(&click(d.find("Library").unwrap()));
    enter(
        &mut d,
        "Library folder",
        &tmp.0.join("symbols.ewlib").to_string_lossy(),
    );
    d.input(&click(d.find("Save to library").unwrap()));
    assert!(!d.shell_mut().symbol_editor().unwrap().dirty());
    d.input(&click(d.find("Test on a new page").unwrap()));
    // ID-1463: the editor's view closes itself; the page shows with the test copy selected, the draft kept behind
    // Back to the editor.
    assert!(d.shell_mut().symbol_editor().is_none());
    assert_eq!(d.shell_mut().kept_symbol_editor().unwrap().error, None);
    assert_eq!(d.shell_mut().selection().items.len(), 1);
    d.settle();
    assert!(d.find("Back to the editor").is_some(), "{:?}", d.labels());
    // The published library symbol, from the Insert symbol picker as Draw ▸ Place ▸ Symbol… opens it, on the same
    // scratch page (its grid is the symbol's) below the test copy.
    {
        let s = d.shell_mut();
        assert_eq!(s.dispatch("tool.place", &json!({})), Outcome::Done);
        let row = s
            .picker()
            .unwrap()
            .catalogue
            .symbols
            .iter()
            .find(|r| r.key.starts_with("library:") && r.name.starts_with("Supply block"))
            .unwrap_or_else(|| panic!("{:?}", s.picker().unwrap().catalogue))
            .clone();
        assert_eq!(s.dispatch("ui.dismiss", &json!({})), Outcome::Done);
        assert_eq!(
            s.dispatch("place.pick", &json!({"symbol":row.key,"letter":"U"})),
            Outcome::Done
        );
        let at = s
            .canvas()
            .unwrap()
            .page()
            .snapper
            .page_to_sheet((60_000_000, 140_000_000));
        assert_eq!(
            s.dispatch(
                "cursor.set",
                &json!({"x_mm":at.0 as f64/1_000_000.0,"y_mm":at.1 as f64/1_000_000.0})
            ),
            Outcome::Done
        );
        assert_eq!(s.dispatch("canvas.activate", &json!({})), Outcome::Done);
        assert_eq!(s.dispatch("ui.dismiss", &json!({})), Outcome::Done);
    }
    // Each placed copy stands as the live preview of its variant shows it: every pin at the insertion point plus its
    // previewed position (G-M2.5-01).
    {
        let tables = &d.shell_mut().canvas().unwrap().page().tables;
        let placed = |key: &dyn Fn(&str) -> bool| {
            let rep = tables
                .reps
                .iter()
                .find(|r| key(&r.symbol_key))
                .unwrap_or_else(|| panic!("{:?}", tables.reps));
            let mut cps: Vec<_> = tables
                .cps
                .iter()
                .filter(|c| c.rep == rep.id)
                .map(|c| (c.idx, (c.at.0 - rep.pos.0, c.at.1 - rep.pos.1)))
                .collect();
            cps.sort_unstable();
            (rep.orient, cps)
        };
        let previewed = |g: &ew_ui::ew_viewmodel::ew_api::symbols::SymbolGeometry| {
            let mut cps: Vec<_> = g.cps.iter().map(|c| (c.fn_cp, (c.pos.x.0, c.pos.y.0))).collect();
            cps.sort_unstable();
            cps
        };
        assert_eq!(
            placed(&|k| k.starts_with("example.supply-block.test.")),
            (1, previewed(&b))
        );
        assert_eq!(placed(&|k| k == "example.supply-block"), (0, previewed(&a)));
    }
    // Route from every real pin of both copies through the same wire tool used in the main window.
    // Page points must be converted to sheet coordinates before setting the cursor.
    use ew_ui::ew_viewmodel::ew_api::geometry::Dir;
    let mut pins = d.shell_mut().canvas().unwrap().page().tables.cps.clone();
    assert_eq!(pins.len(), 12);
    pins.sort_by_key(|p| std::cmp::Reverse(p.at.0));
    for pin in pins {
        let s = d.shell_mut();
        assert_eq!(s.dispatch("tool.wire", &json!({})), Outcome::Done);
        let start = s.canvas().unwrap().page().snapper.page_to_sheet(pin.at);
        assert_eq!(
            s.dispatch(
                "cursor.set",
                &json!({"x_mm":start.0 as f64/1_000_000.0,"y_mm":start.1 as f64/1_000_000.0})
            ),
            Outcome::Done
        );
        assert_eq!(s.dispatch("canvas.activate", &json!({})), Outcome::Done);
        assert_eq!(s.tool().state(), "routing", "{}", pin.label);
        let delta = match pin.dir {
            Some(Dir::Up) => (0, -12_000_000),
            Some(Dir::Down) => (0, 12_000_000),
            Some(Dir::Left) => (-12_000_000, 0),
            _ => (12_000_000, 0),
        };
        let end = s
            .canvas()
            .unwrap()
            .page()
            .snapper
            .page_to_sheet((pin.at.0 + delta.0, pin.at.1 + delta.1));
        assert_eq!(
            s.dispatch(
                "cursor.set",
                &json!({"x_mm":end.0 as f64/1_000_000.0,"y_mm":end.1 as f64/1_000_000.0})
            ),
            Outcome::Done
        );
        assert_eq!(s.dispatch("tool.ip", &json!({})), Outcome::Done);
        let tables = &s.canvas().unwrap().page().tables;
        let connected = tables.cps.iter().find(|p| p.id == pin.id).unwrap();
        assert_eq!(connected.connections.len(), 1, "{}", pin.label);
        let wire = tables
            .connections
            .iter()
            .find(|w| w.id == connected.connections[0])
            .unwrap();
        assert_eq!(wire.polyline.first(), Some(&pin.at));
        assert_eq!(s.dispatch("ui.dismiss", &json!({})), Outcome::Done);
    }
    d.input(&key("ctrl+s"));
    assert!(!d.shell_mut().is_dirty());
    assert!(d.shell_mut().symbol_editor().is_none());
    d.shell_mut().shutdown();
    let (session, _) =
        ew_ui::ew_viewmodel::ew_api::Session::open_read_only(&tmp.0.join("supply.ew"), "ElektraWatt test")
            .unwrap();
    use ew_ui::ew_viewmodel::ew_api::output::{RowExport, row_export};
    let report = row_export(&session.query(), &RowExport::Kind("wire_list".into()), None).unwrap();
    let projection = session.sql_projection(None).unwrap();
    let reader = projection
        .reader(ew_ui::ew_viewmodel::ew_api::sql::Profile::EW_SQL)
        .unwrap();
    let endpoints = reader.query("SELECT cp AS pin FROM (SELECT from_cp AS cp FROM api_v1_wire UNION SELECT to_cp FROM api_v1_wire) WHERE cp <> '' ORDER BY pin").unwrap().to_csv();
    assert_eq!(endpoints, "pin\n1\n2\n3\n4\n5\nFE\n");
    // The library-linked copy uses the published function definition, and all six of its pins are wired, as are the
    // test copy's.
    let functions = reader
        .query("SELECT fndef FROM api_v1_function WHERE fndef LIKE 'example.supply-block%' AND page_id IS NOT NULL ORDER BY fndef")
        .unwrap()
        .to_csv();
    let functions: Vec<&str> = functions.lines().collect();
    assert_eq!(functions.len(), 3, "{functions:?}");
    assert_eq!(functions[..2], ["fndef", "example.supply-block.function"]);
    assert!(
        functions[2].starts_with("example.supply-block.test.") && functions[2].ends_with(".function"),
        "{functions:?}"
    );
    let wired = reader
        .query("SELECT f.fndef LIKE 'example.supply-block.test.%' AS test, e.cp FROM (SELECT from_device_id AS d, from_cp AS cp FROM api_v1_wire UNION SELECT to_device_id, to_cp FROM api_v1_wire) e JOIN api_v1_function f ON f.device_id = e.d WHERE e.cp <> '' ORDER BY test, e.cp")
        .unwrap()
        .to_csv();
    assert_eq!(
        wired,
        "test,cp\n0,1\n0,2\n0,3\n0,4\n0,5\n0,FE\n1,1\n1,2\n1,3\n1,4\n1,5\n1,FE\n"
    );
    // The blind brief's hard check A2 finds both copies; its scratch-page branch names the scratch page's title.
    let acceptance = include_str!("../../../../tools/blind/tasks/brief-symbol-editor/acceptance.toml");
    let a2 = acceptance
        .split("[[item]]")
        .find(|item| item.contains("id = \"A2\""))
        .and_then(|item| item.lines().find_map(|l| l.strip_prefix("query = \"")))
        .and_then(|q| q.strip_suffix('"'))
        .unwrap();
    assert_eq!(reader.query(a2).unwrap().to_csv().lines().count(), 3);
    let scratch = reader
        .query("SELECT count(*) AS n FROM api_v1_function f JOIN api_v1_page p ON p.id = f.page_id WHERE p.title = 'Symbol test: Supply block' AND f.ip_name IS NULL")
        .unwrap()
        .to_csv();
    assert_eq!(scratch, "n\n2\n");
    // Interruption-point pairs can join two drawn legs into one physical wire.
    assert!(report.rows > 0);
    for pin in ["1", "2", "3", "4", "5", "FE"] {
        assert!(
            report.csv.contains(&format!(",{pin},")),
            "missing {pin}: {}",
            report.csv
        );
    }
}

/// M25-p3#3: a drag moves the rectangle under its start, and a drag that starts on no shape moves nothing, whatever
/// is selected.
#[test]
fn symbol_editor_drags_a_rectangle_and_hit_tests_the_drag_start() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    for (action, params) in [
        ("symbol.new", json!({"key":"example.drag","name":"Drag"})),
        (
            "symbol.draw",
            json!({"tool":"rectangle","x":-12,"y":-8,"x2":12,"y2":8}),
        ),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done);
    }
    d.settle();
    let centre = d.find("Drawing of Drag").unwrap();
    let first = d.find("Drag, variant A").unwrap();
    // The canvas's left edge from its centre and the first of the eight variant previews below it.
    let eighth = ((centre.0 - first.0) as f64 / 3.5) as i64;
    let left = first.0 - eighth / 2;
    let rect = |d: &mut Driver| {
        serde_json::to_value(&d.shell_mut().symbol_editor().unwrap().draft.symbol.graphics[0]).unwrap()["rect"]
            ["rect"]
            .clone()
    };
    let before = rect(&mut d);
    d.input(&Input::Drag {
        from: centre,
        to: (centre.0 + 80, centre.1 + 40),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    let moved = rect(&mut d);
    assert_ne!(moved, before);
    assert_eq!(
        moved[1][0].as_i64().unwrap() - moved[0][0].as_i64().unwrap(),
        24_000_000,
        "a move keeps the size"
    );
    // The rectangle stays selected; a drag from the empty margin leaves it where it is.
    d.input(&Input::Drag {
        from: (left + eighth / 4, centre.1),
        to: (left + eighth / 4 + 80, centre.1 + 40),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    assert_eq!(rect(&mut d), moved);
    d.shell_mut().shutdown();
}

/// ID-1477 (4), (9): with the Rectangle tool a press, drag and release draws a rectangle; the canvas keeps its place
/// when the connection point tool shows its fields, and its zoom as the drawing grows: the same drag again draws the
/// same rectangle.
#[test]
fn symbol_editor_draws_a_rectangle_by_drag_and_keeps_its_view() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = symbol_driver(&tmp, "example.drag-draw");
    let canvas = d.find("Drawing of Drawn").unwrap();
    assert_eq!(
        d.shell_mut().dispatch("symbol.tool", &json!({"tool": "cp"})),
        Outcome::Done
    );
    d.settle();
    assert_eq!(d.find("Drawing of Drawn").unwrap(), canvas, "the canvas stays put");
    assert_eq!(
        d.shell_mut()
            .dispatch("symbol.tool", &json!({"tool": "rectangle"})),
        Outcome::Done
    );
    d.settle();
    let from = (canvas.0 - 160, canvas.1 - 90);
    for _ in 0..2 {
        d.input(&Input::Drag {
            from,
            to: (from.0 + 120, from.1 + 60),
            button: PointerButton::Primary,
            mods: Modifiers::NONE,
        });
        d.settle();
    }
    let rects: Vec<Value> = d
        .shell_mut()
        .symbol_editor()
        .unwrap()
        .draft
        .symbol
        .graphics
        .iter()
        .filter_map(|g| serde_json::to_value(g).unwrap().get("rect").map(|r| r["rect"].clone()))
        .collect();
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert_eq!(rects[0], rects[1], "the second drag draws at the first one's zoom");
    assert!(rects[0][1][0].as_i64().unwrap() > rects[0][0][0].as_i64().unwrap());
    d.shell_mut().shutdown();
}

/// M25-p3#4: a field of the document commits once, on focus loss, and a pin designation can be cleared and retyped.
#[test]
fn symbol_editor_fields_commit_on_focus_loss_and_a_designation_is_retyped() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    for (action, params) in [
        ("symbol.new", json!({"key":"example.fields","name":"Fields"})),
        (
            "symbol.cp",
            json!({"x":0,"y":0,"designation":"13","direction":"left"}),
        ),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done);
    }
    d.settle();
    d.input(&click(d.find("Library").unwrap()));
    let before = d.shell_mut().symbol_editor().unwrap().draft.clone();
    d.input(&click(d.find("Key").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type {
        text: "example.typed".into(),
    });
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, before);
    d.input(&key("tab"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft.key, "example.typed");
    // One undo step for the whole entry (Ctrl+Z in the next field is the field's own).
    assert_eq!(d.shell_mut().dispatch("edit.undo", &json!({})), Outcome::Done);
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, before);
    d.input(&click(d.find("Electrical data").unwrap()));
    d.input(&click(d.find("Designation 1").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&key("backspace"));
    d.input(&Input::Type { text: "A1".into() });
    d.input(&key("enter"));
    let draft = d.shell_mut().symbol_editor().unwrap().draft.clone();
    assert_eq!(
        draft.function.cps.iter().next().unwrap().designation.as_str(),
        "A1"
    );
    // A designation left empty keeps the one it had.
    d.input(&click(d.find("Designation 1").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&key("backspace"));
    d.input(&key("tab"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, draft);
    d.shell_mut().shutdown();
}

/// The brief (FIX25a's no shared key): an import through the window takes the file's name as its key, never the open
/// draft's.
#[test]
fn symbol_editor_import_through_the_window_takes_the_file_name_as_its_key() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    assert_eq!(
        d.shell_mut()
            .dispatch("symbol.new", &json!({"key":"example.open","name":"Open"})),
        Outcome::Done
    );
    d.settle();
    d.input(&click(d.find("Library").unwrap()));
    let qet = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ew-import-qet/tests/fixtures/contact_no.elmt"
    );
    // The second "Open or import" is the section's header; the first, the bar's command, reads the same field.
    let section = d
        .labels()
        .into_iter()
        .filter(|(n, _)| n == "Open or import")
        .nth(1)
        .unwrap()
        .1;
    d.input(&click(section));
    d.input(&click(d.find("Symbol file").unwrap()));
    d.input(&Input::Type { text: qet.into() });
    d.input(&click(d.find("Open or import").unwrap()));
    assert!(actions(&mut d).contains(&"symbol.open".to_owned()));
    let editor = d.shell_mut().symbol_editor().unwrap();
    assert_eq!(editor.error, None);
    assert_eq!(editor.draft.key, "contact_no");
    d.shell_mut().shutdown();
}

/// ID-1463 (doc 16a P8, P10): a key of the keymap chooses a tool, its tooltip names it, and a tool leaves the panel.
#[test]
fn symbol_editor_tools_take_keys_of_the_keymap_and_leave_the_panel() {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    assert_eq!(
        d.shell_mut()
            .dispatch("symbol.new", &json!({"key":"example.keys","name":"Keys"})),
        Outcome::Done
    );
    d.settle();
    d.input(&click(d.find("Library").unwrap()));
    d.input(&Input::Hover { at: (600, 400) });
    d.input(&key("l"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().tool, "line");
    d.input(&key("p"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().tool, "cp");
    // The Library panel stays, and Escape returns to Select.
    assert!(d.find("Library folder").is_some());
    d.input(&key("escape"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().tool, "select");
    d.shell_mut().shutdown();
}

/// A driver with the symbol editor open on a drawn one-pin draft (a line and pin 1 at the origin).
fn symbol_driver(tmp: &Tmp, key: &str) -> Driver {
    use ew_ui::ew_viewmodel::shell::Outcome;
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    for (action, params) in [
        ("symbol.new", json!({"key":key,"name":"Drawn"})),
        ("symbol.draw", json!({"tool":"line","x":0,"y":0,"x2":0,"y2":4})),
        (
            "symbol.cp",
            json!({"x":0,"y":0,"designation":"1","direction":"left"}),
        ),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done);
    }
    d.settle();
    d
}

/// M25-p5#1: the Library folder typed in its field is the editor's once the field is left, so Ctrl+S saves there.
#[test]
fn symbol_editor_saves_with_ctrl_s_to_the_library_folder_typed() {
    let tmp = Tmp::new();
    let mut d = symbol_driver(&tmp, "example.folder");
    d.input(&click(d.find("Library").unwrap()));
    let typed = tmp.0.join("typed.ewlib");
    d.input(&click(d.find("Library folder").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type {
        text: typed.to_string_lossy().into_owned(),
    });
    d.input(&key("tab"));
    assert_eq!(
        PathBuf::from(&d.shell_mut().symbol_editor().unwrap().directory),
        typed
    );
    d.input(&Input::Hover { at: (600, 400) });
    d.input(&key("ctrl+s"));
    let editor = d.shell_mut().symbol_editor().unwrap();
    assert_eq!(editor.error, None);
    assert!(editor.stored());
    assert!(typed.is_dir());
    d.shell_mut().shutdown();
}

/// M25-p5#2 (doc 16a §6.8): Escape cancels a field's entry, and the tool stays; Enter commits.
#[test]
fn symbol_editor_escape_cancels_a_field_and_keeps_the_tool() {
    let tmp = Tmp::new();
    let mut d = symbol_driver(&tmp, "example.escape");
    d.input(&Input::Hover { at: (600, 400) });
    d.input(&key("l"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().tool, "line");
    let before = d.shell_mut().symbol_editor().unwrap().draft.clone();
    for (field, text) in [("Name", "Typed name"), ("Designation 1", "A1")] {
        if field == "Designation 1" {
            d.input(&click(d.find("Electrical data").unwrap()));
        }
        d.input(&click(d.find(field).unwrap()));
        d.input(&key("ctrl+a"));
        d.input(&Input::Type { text: text.into() });
        d.input(&key("escape"));
        assert_eq!(d.shell_mut().symbol_editor().unwrap().draft, before, "{field}");
        assert_eq!(d.shell_mut().symbol_editor().unwrap().tool, "line", "{field}");
    }
    d.input(&click(d.find("Designation 1").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type { text: "A1".into() });
    d.input(&key("enter"));
    let draft = d.shell_mut().symbol_editor().unwrap().draft.clone();
    assert_eq!(
        draft.function.cps.iter().next().unwrap().designation.as_str(),
        "A1"
    );
    d.shell_mut().shutdown();
}

/// M25-p3#4, M25-p5#3: a drag of a coordinate through the window moves it and commits once, at its end (the slow drag
/// of one point per frame is `symbol_editor`'s unit test: the driver's drag moves eight times).
#[test]
fn symbol_editor_a_drag_of_a_coordinate_commits_once() {
    let tmp = Tmp::new();
    let mut d = symbol_driver(&tmp, "example.slow");
    d.input(&click(d.find("Electrical data").unwrap()));
    d.input(&click(d.find("Connection points (1)").unwrap()));
    let at = d.find("x (mm) of Connection point 1").unwrap();
    d.input(&Input::Drag {
        from: at,
        to: (at.0 + 40, at.1),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    let editor = d.shell_mut().symbol_editor().unwrap();
    let x = editor.draft.symbol.cps.iter().next().unwrap().pos.x.0;
    assert!(x > 0, "the pin did not move: {x}");
    // One undo step for the whole drag.
    assert_eq!(
        d.shell_mut().dispatch("edit.undo", &json!({})),
        ew_ui::ew_viewmodel::shell::Outcome::Done
    );
    let editor = d.shell_mut().symbol_editor().unwrap();
    assert_eq!(editor.draft.symbol.cps.iter().next().unwrap().pos.x.0, 0);
    d.shell_mut().shutdown();
}

/// ID-1472 (M25-p5#6): the editor's bar has New symbol; over unsaved changes it asks in the house dialog, whose
/// Discard changes starts the fresh draft.
#[test]
fn symbol_editor_new_symbol_asks_before_it_drops_unsaved_changes() {
    let tmp = Tmp::new();
    let mut d = symbol_driver(&tmp, "example.fresh");
    d.input(&click(d.find("New symbol").unwrap()));
    d.settle();
    assert!(d.shell_mut().dialog().is_some());
    assert!(d.find("Save symbol").is_some(), "{:?}", d.labels());
    d.input(&click(d.find("Discard changes").unwrap()));
    d.settle();
    assert!(d.shell_mut().dialog().is_none());
    let editor = d.shell_mut().symbol_editor().unwrap();
    assert!(editor.draft.key.is_empty() && editor.draft.symbol.graphics.is_empty());
    d.shell_mut().shutdown();
}
