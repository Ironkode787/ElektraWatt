// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The visual play-test driver: the input vocabulary's bounds, inputs that reach the
//! window's widgets, equal dispatch logs and frames for equal scripts, the frame clock, the jobs and the file dialogs
//! answered for the system, and the application closing.

use crate::support;

use std::path::{Path, PathBuf};

use egui::{Key, Modifiers, PointerButton};
use ew_ui::headless::{HeadlessArgs, ScriptSource};
use ew_ui::visual::{
    self, Driver, FRAME_HEIGHT, FRAME_WIDTH, Geometry, Input, parse_chord, parse_frame, parse_input,
    parse_input_in,
};
use ew_viewmodel::script::state_json;
use ew_viewmodel::shell::ShellConfig;
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

/// A driver over a copy of the sample on folio 101, settled, with its accessibility tree.
fn sample_on_101(tmp: &Tmp) -> Driver {
    let mut d = crate::support::driver_on_sample(tmp, "101");
    let _ = actions(&mut d);
    d
}

/// A script's `ui.dismiss` action line with the app menu open closes the menu and nothing else, as the
/// window's own `ui.dismiss` does: the selection stays; with the menu closed it dismisses as before.
#[test]
fn a_scripts_ui_dismiss_with_the_app_menu_open_closes_only_the_menu() {
    let tmp = Tmp::new();
    let sample = tmp.sample("s.ew");
    let lines = [
        json!({"action": "file.open", "params": {"dir": sample.to_string_lossy()}}),
        json!({"action": "page.goto", "params": {"folio": "101"}}),
        json!({"action": "select.set", "params": {"refs": ["tag:-101F1"]}}),
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

/// BV2: the vocabulary's members and bounds; the frame line's.
#[test]
fn the_inputs_check_their_members_and_bounds() {
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
    assert!(parse_frame(&obj(json!({"frame": "t1.png", "zoom": [1200, 0, 768, 10]}))).is_err());
    // A corner near u32::MAX is refused, not added with overflow (a debug panic, a wrapped slice in release).
    for zoom in [
        json!([4_294_967_295_u32, 0, 10, 10]),
        json!([0, 4_294_967_295_u32, 10, 10]),
    ] {
        assert!(parse_frame(&obj(json!({"frame": "t1.png", "zoom": zoom}))).is_err());
    }
}

/// The driver's `double_click` on the Pages navigator's row of 100 shows 100, and the next one, on
/// 101, shows 101: the driver pauses before a double click, so egui does not count it as a triple click with the one
/// before (both ran a few frames apart).
#[test]
fn the_drivers_double_clicks_on_two_rows_open_both() {
    let tmp = Tmp::new();
    let mut d = sample_on_101(&tmp);
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

/// In the visual harness as in the window, a Shift+click on a row of the Terminal strips table marks the
/// range from the cursor row and a Ctrl+click toggles one row (R1's clicks lost their modifiers: the harness let go of
/// them in the release's frame, where the grid reads the click); Ctrl+A marks every row of the filtered table.
#[test]
fn shift_and_ctrl_clicks_mark_table_rows_and_ctrl_a_marks_them_all() {
    use ew_viewmodel::navigators::NavKind;
    use ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = sample_on_101(&tmp);
    for (action, params) in [
        // The strip's page: a terminal of another page, selected, would add the chip line under the page.
        ("page.goto", json!({"folio": "102"})),
        ("props.show", json!({"on": false})),
        ("navigator.show", json!({"kind": "strips"})),
        ("navigator.table", json!({"kind": "strips"})),
        (
            "navigator.filter",
            json!({"kind": "strips", "text": "strip:-X1021"}),
        ),
        ("ui.focus", json!({"area": "navigator", "kind": "strips"})),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done, "{action}");
    }
    // A few frames lay the table out.
    d.input(&Input::Wait { ms: 100 });
    let _ = actions(&mut d);
    let row = |d: &Driver, i: usize| {
        let needle = format!(", row {i} of ");
        d.labels()
            .into_iter()
            .find(|(l, _)| l.starts_with("Strip ") && l.contains(&needle))
            .unwrap_or_else(|| {
                let cells: Vec<String> = d.labels().into_iter().map(|(l, _)| l).collect();
                panic!("the Strip cell of row {i} among {cells:?}")
            })
            .1
    };
    let marked = |d: &Driver| {
        d.app()
            .unwrap()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .marked()
            .len()
    };
    let with = |at, mods| Input::Click {
        at,
        button: PointerButton::Primary,
        mods,
    };
    d.input(&click(row(&d, 1)));
    assert_eq!(actions(&mut d), ["ui.focus", "navigator.select", "table.cell"]);
    d.input(&with(row(&d, 3), Modifiers::SHIFT));
    assert_eq!(actions(&mut d), ["ui.focus", "navigator.row", "table.cell"]);
    assert_eq!(marked(&d), 3, "rows 1 to 3");
    d.input(&with(row(&d, 5), Modifiers::COMMAND));
    assert_eq!(actions(&mut d), ["ui.focus", "navigator.mark", "table.cell"]);
    assert_eq!(marked(&d), 4, "and row 5");
    d.input(&key("ctrl+a"));
    assert_eq!(actions(&mut d), ["navigator.select"]);
    let rows = d
        .app()
        .unwrap()
        .shell()
        .nav(NavKind::Strips)
        .unwrap()
        .visible()
        .len();
    assert_eq!(marked(&d), rows, "every row");
    d.shell_mut().shutdown();
}

/// BV3: a click on a known button dispatches its action; a right click on a property row opens its menu; typing
/// into a field sets it; a rubber band selects and a drag from a selected item moves the selection.
#[test]
fn clicks_right_clicks_typing_and_drags_reach_the_widgets() {
    let tmp = Tmp::new();
    let mut d = sample_on_101(&tmp);
    // The drags below aim at Fit page's view (the first view is fit-width).
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
    // Over the drawing, inside the frame's border.
    d.input(&Input::Drag {
        from: (790, 290),
        to: (1650, 850),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    let done = actions(&mut d);
    assert!(done.contains(&"canvas.press".to_owned()) && done.contains(&"canvas.release".to_owned()));
    let selected = state(&mut d)["canvas"]["selection"].as_array().unwrap().len();
    assert!(selected > 0, "the rubber band selected");
    assert_eq!(state(&mut d)["can_undo"], false);
    // A point over a selected item: the first of a coarse grid whose hover names one (the frame, which the band did
    // not hold, is hovered too).
    let mut over = None;
    'scan: for y in (340..840).step_by(40) {
        for x in (840..1600).step_by(40) {
            d.input(&Input::Hover { at: (x, y) });
            let canvas = state(&mut d)["canvas"].clone();
            if canvas["selection"]
                .as_array()
                .is_some_and(|s| s.contains(&canvas["hover"]))
            {
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
fn the_same_script_twice_gives_the_same_logs_states_and_frames() {
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
        // 1920 × 1108 pixels, from the PNG's header.
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
fn the_dwells_follow_the_frame_clock() {
    let tmp = Tmp::new();
    let mut d = sample_on_101(&tmp);
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
fn dialogs_are_answered_and_an_export_ends_within_its_input() {
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    d.settle();
    // New project: its dialog, named `project` in the run's folder, and Enter.
    d.input(&key("ctrl+n"));
    d.shell_mut().dispatch(
        "new_project.set",
        &json!({"name": "project", "folder": tmp.0.to_string_lossy()}),
    );
    d.settle();
    d.input(&key("enter"));
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

/// `answer_open` names the file the next open dialog answers with, resolved against the script's folder, and
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
fn the_caret_is_always_drawn() {
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

/// 2560 × 1440 points at 1 pixel per point, the extension of the main resolution (doc 16a §2).
fn extension() -> Geometry {
    Geometry::parse(Some("2560x1440"), Some("1")).unwrap()
}

/// A run may choose its window size and scale. At 2560 × 1440 points and ppp 1 the frame is 2560 × 1468
/// pixels (a 28-pixel strip), at 1920 × 1080 and ppp 2 3840 × 2216; the default is 1920 × 1080 at 1, a frame of
/// 1920 × 1108.
#[test]
fn the_geometry_sizes_the_frame() {
    let g = extension();
    assert_eq!(
        (g.frame_width(), g.frame_height(), g.strip_px(), g.dpi()),
        (2560, 1468, 28, 96)
    );
    let g2 = Geometry::parse(Some("1920x1080"), Some("2")).unwrap();
    assert_eq!(
        (g2.frame_width(), g2.frame_height(), g2.strip_px(), g2.dpi()),
        (3840, 2216, 56, 192)
    );
    let d = Geometry::DEFAULT;
    assert_eq!(
        (d.frame_width(), d.frame_height(), d.strip_px(), d.dpi(), d.ppp()),
        (FRAME_WIDTH, FRAME_HEIGHT, visual::STRIP_PX, 96, visual::PPP)
    );
    assert_eq!(Geometry::parse(None, None), Ok(Geometry::DEFAULT));
    assert_eq!(
        Geometry::parse(Some("1920x1080"), Some("1")),
        Ok(Geometry::DEFAULT)
    );
}

/// An input at x = 2500 is inside a 2560-pixel frame and outside the default one, with the existing
/// out-of-frame text.
#[test]
fn coordinates_are_checked_against_the_runs_frame() {
    let o = obj(json!({"input": "click", "x": 2500, "y": 1400}));
    assert!(matches!(
        parse_input_in(&o, extension()),
        Ok(Input::Click { at: (2500, 1400), .. })
    ));
    let refused = parse_input(&o).unwrap_err();
    assert_eq!(refused, "`x` is outside the frame (0 to 1919)");
    assert!(parse_input_in(&obj(json!({"input": "click", "x": 2560, "y": 0})), extension()).is_err());
    let zoom = obj(json!({"frame": "z.png", "zoom": [1600, 900, 768, 497]}));
    assert!(visual::parse_frame_in(&zoom, extension()).is_ok());
    assert!(parse_frame(&zoom).is_err());
}

/// At 2560 × 1440 points and ppp 1 a click at a widget's centre in that frame's pixels reaches the widget.
#[test]
fn a_click_at_the_extension_reaches_the_widget() {
    let tmp = Tmp::new();
    let sample = tmp.sample("s.ew");
    let mut d = Driver::with_geometry(tmp.config(), None, tmp.0.clone(), extension()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": sample.to_string_lossy()}));
    assert_eq!(opened, ew_viewmodel::shell::Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    let _ = actions(&mut d);
    let at = d.find("Check project").expect("the Check project button");
    assert!(at.0 < 2560 && at.1 < 1468, "{at:?}");
    d.input(&click(at));
    assert_eq!(actions(&mut d), ["messages.check"], "the click at {at:?}");
    d.shell_mut().shutdown();
}

/// A run at 2560 × 1440 points and ppp 1 names its geometry in the transcript's header, accepts an input at
/// x = 2500 and (where frames are painted) paints 2560 × 1468 pixels; at the default the header names none.
#[test]
fn a_run_at_the_extension_paints_its_frame_and_names_it() {
    let tmp = Tmp::new();
    let lines = |dir: &Path| {
        vec![
            json!({"action": "file.new", "params": {"dir": dir.join("p").to_string_lossy()}}),
            json!({"input": "hover", "x": 2500, "y": 600}),
            json!({"frame": "t0001.png"}),
        ]
    };
    let frames = cfg!(windows);
    let (t, code, err) = run_at(&tmp, "w", &lines(&tmp.0.join("w")), frames, extension());
    assert_eq!(code, 0, "{err}");
    assert_eq!(t[0]["window"], json!([2560, 1440]), "{:?}", t[0]);
    assert_eq!(t[0]["ppp"], json!(1));
    assert!(t[2].get("input").is_some(), "the hover ran: {:?}", t[2]);
    if frames {
        let png = std::fs::read(tmp.0.join("w/frames/t0001.png")).unwrap();
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 2560);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 1468);
    }
    assert_eq!(
        visual::transcript_header("1.0", Geometry::parse(Some("1366x768"), Some("1.25")).unwrap()),
        r#"{"$schema":"ew.uitranscript/1","version":"1.0","window":[1366,768],"ppp":1.25}"#
    );
    // The default run's header has no geometry, and its input at x = 2500 is refused.
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

/// The driver's fixed rendering inputs hold whatever the settings folder holds: no minimum text size and
/// reduced motion on, after a session that set 12 pt and motion.
#[test]
fn the_driver_pins_the_minimum_text_size_and_reduced_motion() {
    use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
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

/// A click on Wire marker in the Insert symbol list must do something. By the pointer:
/// a double click on the row arms the Wire marker tool, and on the Interruption point row the Interruption tool.
#[test]
fn a_click_on_a_tool_s_symbol_in_the_insert_symbol_list_arms_the_tool() {
    use ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    for (typed, row, tool) in [
        ("wire marker", "Wire marker", "marker"),
        ("interruption point", "Interruption point", "ip"),
    ] {
        d.input(&click(d.find("Symbol").unwrap()));
        d.input(&Input::Type {
            text: typed.to_owned(),
        });
        d.settle();
        // The list's row, not the left rail's button of a like name, nor the command bar's above the list.
        let (rail, bar) = (d.find("Symbol").unwrap().0 * 2, d.find("Symbol").unwrap().1 + 40);
        let at = d
            .labels()
            .into_iter()
            .find(|(l, at)| l.starts_with(row) && at.0 > rail && at.1 > bar)
            .unwrap_or_else(|| panic!("no row {row}: {:?}", d.labels()))
            .1;
        // A click makes the row current; a double click chooses it.
        d.input(&Input::DoubleClick {
            at,
            mods: Modifiers::NONE,
        });
        d.settle();
        assert_eq!(state(&mut d)["canvas"]["tool"]["name"], json!(tool), "{row}");
        assert!(d.shell_mut().picker().is_none(), "{row}: the list stays open");
        d.input(&key("escape"));
        d.settle();
    }
    d.shell_mut().shutdown();
}

/// A right click cancels whatever is active, as Escape does. With a
/// symbol floating on the pointer and with a wire being drawn, a right click on the page dismisses one step; back at
/// the Select tool it opens the page's menu and dismisses nothing.
#[test]
fn a_right_click_on_the_page_cancels_the_active_tool() {
    use ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    let tool = |d: &mut Driver| {
        state(d)["canvas"]["tool"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    let right = |at: (i64, i64)| Input::Click {
        at,
        button: PointerButton::Secondary,
        mods: Modifiers::NONE,
    };
    let page = (900, 500);
    // A symbol floating on the pointer.
    d.input(&click(d.find("Symbol").unwrap()));
    d.input(&Input::Type {
        text: "fuse".to_owned(),
    });
    d.input(&key("enter"));
    d.input(&Input::Hover { at: page });
    d.settle();
    assert_eq!(tool(&mut d), "place");
    d.input(&right(page));
    d.settle();
    assert_eq!(tool(&mut d), "select");
    assert_eq!(
        state(&mut d)["canvas"]["selection"],
        json!([]),
        "nothing was placed"
    );
    // The Wire tool, armed.
    d.input(&click(d.find("Wire").unwrap()));
    d.input(&Input::Hover { at: page });
    d.settle();
    assert_eq!(tool(&mut d), "wire");
    d.input(&right(page));
    d.settle();
    assert_eq!(tool(&mut d), "select");
    // At the Select tool a right click is the page's menu, and no Escape.
    let _ = actions(&mut d);
    d.input(&right(page));
    d.settle();
    assert!(!actions(&mut d).contains(&"ui.dismiss".to_owned()));
    d.shell_mut().shutdown();
}

/// A route is edited with the mouse, by dots on the sections of a wire that are dragged. A selected wire shows a
/// handle on each segment and bend, and a drag on a straight wire's line moves
/// it sideways: the ends stay on their connection points, a stub leaves each, the route stays right-angled, and one
/// undo takes it back.
#[test]
fn a_drag_on_a_selected_wire_edits_its_route_with_the_mouse() {
    let tmp = Tmp::new();
    let mut d = sample_on_101(&tmp);
    let _ = d.shell_mut().dispatch("view.fit_page", &json!({}));
    d.settle();
    // The window's pixels of a sheet point, from two pointer positions the canvas reports.
    let probe = |d: &mut Driver, at: (i64, i64)| {
        d.input(&Input::Hover { at });
        d.settle();
        let p = state(d)["canvas"]["pointer_nm"].clone();
        (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())
    };
    let (f0, f1) = ((700, 400), (1000, 700));
    let (s0, s1) = (probe(&mut d, f0), probe(&mut d, f1));
    let pixel = |sheet: (i64, i64)| {
        (
            f0.0 + (sheet.0 - s0.0) * (f1.0 - f0.0) / (s1.0 - s0.0),
            f0.1 + (sheet.1 - s0.1) * (f1.1 - f0.1) / (s1.1 - s0.1),
        )
    };
    let step = pixel((s0.0 + 4_000_000, s0.1)).0 - f0.0;
    let route = |d: &mut Driver, id: &str| -> Vec<(i64, i64)> {
        let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
        tables
            .connections
            .iter()
            .find(|c| c.id.to_string() == id)
            .map(|c| c.polyline.clone())
            .unwrap_or_default()
    };
    // The straight routed wires of the folio, the longest first; one of them lies clear of every symbol.
    let mut straight: Vec<(String, Vec<(i64, i64)>)> = {
        let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
        tables
            .connections
            .iter()
            .filter(|c| c.stored_route && c.polyline.len() == 2)
            .map(|c| (c.id.to_string(), c.polyline.clone()))
            .collect()
    };
    let length = |l: &[(i64, i64)]| (l[1].0 - l[0].0).abs() + (l[1].1 - l[0].1).abs();
    straight.sort_by_key(|(_, l)| std::cmp::Reverse(length(l)));
    let mut edited = None;
    for (id, line) in straight.into_iter().take(8) {
        let _ = d.shell_mut().dispatch("select.set", &json!({ "ids": [id] }));
        d.settle();
        assert!(
            !d.shell_mut().tool_overlay().items.is_empty(),
            "a selected wire shows its handles"
        );
        let sheet =
            |d: &mut Driver, p: (i64, i64)| d.shell_mut().canvas().unwrap().page().snapper.page_to_sheet(p);
        let mid = ((line[0].0 + line[1].0) / 2, (line[0].1 + line[1].1) / 2);
        let from = pixel(sheet(&mut d, mid));
        // Sideways: across a vertical wire, down from a horizontal one, three grid steps.
        let to = if line[0].0 == line[1].0 {
            (from.0 + 3 * step, from.1)
        } else {
            (from.0, from.1 + 3 * step)
        };
        d.input(&Input::Drag {
            from,
            to,
            button: PointerButton::Primary,
            mods: Modifiers::NONE,
        });
        d.settle();
        let after = route(&mut d, &id);
        if after != line {
            edited = Some((id, line, after));
            break;
        }
    }
    let (id, before, after) = edited.expect("a straight wire clear of the symbols, dragged");
    assert_eq!(
        (after.first(), after.last()),
        (before.first(), before.last()),
        "the ends stay"
    );
    assert!(after.len() > 2, "a jog: {after:?}");
    assert!(
        after.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1),
        "a diagonal: {after:?}"
    );
    d.input(&key("ctrl+z"));
    d.settle();
    assert_eq!(route(&mut d, &id), before, "one undo");
    d.shell_mut().shutdown();
}

/// The company-standard wizard gone through by clicks and keys, its steps numbered 1 to 10. Every
/// step shows its Result, a colour profile changes its rows and turns the example to the cover page, the steps passed
/// are done, and Create writes the basic project.
#[test]
fn the_wizard_shows_each_steps_result_by_clicks_and_keys_and_creates_the_standard() {
    use ew_viewmodel::strings;
    use ew_viewmodel::wizard::Step;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config_with_resources(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    d.settle();
    let press = |d: &mut Driver, label: &str| {
        let at = d
            .find(label)
            .unwrap_or_else(|| panic!("{label}: {:?}", d.labels()));
        d.input(&click(at));
        d.settle();
    };
    // The window's widgets by their names; the step's texts (its Result) as `ui.describe` lists them.
    let shows = |d: &Driver, text: &str| d.labels().iter().any(|(l, _)| l.contains(text));
    let result = |d: &mut Driver, text: &str| d.shell_mut().describe().contains(&format!("example \"{text}"));
    press(&mut d, "Templates…");
    press(&mut d, "New template…");
    assert!(d.shell_mut().wizard().is_some());
    assert!(shows(&d, "1. Company") && shows(&d, "10. Review and create"));
    assert!(result(&mut d, "Company: -"));
    // The company name by keys: Shift+Tab from no focus walks back from the last field (logo, notice, name).
    for _ in 0..3 {
        d.input(&key("shift+tab"));
    }
    d.input(&Input::Type {
        text: "Example Panels".to_owned(),
    });
    d.input(&key("enter"));
    d.settle();
    assert!(result(&mut d, "Company: Example Panels"));
    let steps: [(&str, &str); 8] = [
        (
            "2. Frame and title block",
            "Schematic pages: QET-compatible A4 landscape, with grid",
        ),
        (
            "3. Colour profile",
            "24 VDC: Positive dark blue, Signal dark blue, 0V white",
        ),
        (
            "4. Wire cross-sections",
            "A control wire is 0.75 mm² and a mains wire 1.5 mm²",
        ),
        (
            "5. Identifiers and numbering",
            "The third contactor on page 102 is named -102K3.",
        ),
        (
            "6. Structure and sections",
            "Section 1, Wiring Definition: pages 100 to 199",
        ),
        ("7. Revisions", "The title block's revision cell reads V1.0."),
        ("8. Languages and formats", "A date prints as 14-03-2026."),
        (
            "9. Reports and checks",
            "Page 901, parts list: every part with its quantity",
        ),
    ];
    for (title, text) in steps {
        press(&mut d, "Next");
        let step = d.shell_mut().wizard().unwrap().step();
        assert_eq!(
            format!("{}. {}", step.number(), strings::text(step.title_key())),
            title
        );
        assert!(result(&mut d, text), "{title}: {}", d.shell_mut().describe());
        if step == Step::Colour {
            // The colour step shows the cover page, which prints the table, and its rows follow the profile.
            assert!(d.shell_mut().wizard().unwrap().preview_is_cover());
            press(&mut d, "EN 60204-1 machine (not yet checked)");
            assert!(result(&mut d, "AC control: Control red"));
            assert!(!result(&mut d, "24 VDC: Positive dark blue"));
        }
    }
    // The steps passed are done; the review creates the standard in a new folder.
    press(&mut d, "Next");
    assert!(d.shell_mut().wizard().unwrap().is_done(Step::Reports));
    press(&mut d, "Create the template");
    // In a folder of its own, named after the template, in the templates folder.
    let made = tmp.0.join("config/projects/Templates/Example Panels");
    let project = std::fs::read_to_string(made.join("project.jsonl")).unwrap();
    assert!(
        project.contains("Example Panels") && project.contains("ew.en60204-1-machine"),
        "{project}"
    );
    // The check level is in the template: Create in the window writes it.
    assert!(project.contains("\"release_scheme\""), "{project}");
    d.shell_mut().shutdown();
}

/// "a project has a button to add a page but no way to add a
/// section". The Pages panel's New section button opens the form with a number and a hundred of folios proposed;
/// Create makes the section, which the tree then lists; a right click on a row of the tree offers New page… and
/// New section…, and the entry opens the form again with the next proposal.
#[test]
fn a_section_is_made_from_the_pages_panel_and_from_the_trees_right_click_menu() {
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    let _ = d.shell_mut().dispatch("file.new", &project);
    d.settle();
    let form = |d: &mut Driver| -> Vec<(String, String)> {
        match d.shell_mut().dialog() {
            Some(ew_viewmodel::shell::Dialog::Form(f)) => {
                assert_eq!(f.action, "section.new");
                f.fields
                    .iter()
                    .map(|x| (x.name.to_owned(), x.value.clone()))
                    .collect()
            }
            other => panic!("the New section form: {other:?}"),
        }
    };
    let listed = |d: &Driver| d.labels().iter().any(|(l, _)| l.contains("Field wiring"));
    assert!(!listed(&d));
    d.input(&click(
        d.find("New section…")
            .expect("the Pages panel's New section button"),
    ));
    d.settle();
    let fields = form(&mut d);
    let value = |name: &str| fields.iter().find(|(n, _)| n == name).unwrap().1.clone();
    let (first, last, number): (u32, u32, u16) = (
        value("first").parse().unwrap(),
        value("last").parse().unwrap(),
        value("number").parse().unwrap(),
    );
    assert!(
        first % 100 == 0 && last == first + 99,
        "a hundred of folios: {first} to {last}"
    );
    assert!(number >= 1);
    // The title is the form's first field and has the focus.
    d.input(&Input::Type {
        text: "Field wiring".to_owned(),
    });
    d.input(&key("enter"));
    d.settle();
    assert!(d.shell_mut().dialog().is_none(), "Create closes the form");
    let toast = state(&mut d)["toast"].to_string();
    assert!(
        toast.contains(&format!(
            "Section {number} Field wiring made, folios {first} to {last}."
        )),
        "{toast}"
    );
    assert!(listed(&d), "the tree lists the section: {:?}", d.labels());
    // The tree's right-click menu.
    let row = d
        .labels()
        .into_iter()
        .find(|(l, _)| l.contains("Field wiring"))
        .unwrap_or_else(|| panic!("the section's row: {:?}", d.labels()))
        .1;
    d.input(&Input::Click {
        at: row,
        button: PointerButton::Secondary,
        mods: Modifiers::NONE,
    });
    d.settle();
    assert!(d.find("New page…").is_some(), "{:?}", d.labels());
    d.input(&click(d.find("New section…").expect("the menu's New section…")));
    d.settle();
    let fields = form(&mut d);
    let next = fields
        .iter()
        .find(|(n, _)| n == "first")
        .unwrap()
        .1
        .parse::<u32>()
        .unwrap();
    assert!(
        next > last,
        "the next proposal lies after the section just made: {next}"
    );
    d.input(&key("escape"));
    d.settle();
    // The section chosen in the tree: New page proposes its first folio and its number.
    d.input(&click(row));
    d.settle();
    d.input(&click(
        d.find("New page…").expect("the Pages panel's New page button"),
    ));
    d.settle();
    match d.shell_mut().dialog() {
        Some(ew_viewmodel::shell::Dialog::Form(f)) => {
            let value = |name: &str| f.fields.iter().find(|x| x.name == name).unwrap().value.clone();
            assert_eq!(
                (value("folio"), value("section")),
                (first.to_string(), number.to_string())
            );
        }
        other => panic!("the New page form: {other:?}"),
    }
    d.input(&key("enter"));
    d.settle();
    assert!(d.shell_mut().dialog().is_none(), "the page is made");
    assert_eq!(state(&mut d)["page"], first.to_string());
    d.shell_mut().shutdown();
}

/// "important parts such as the terminal strip have no
/// button of their own", the Wire marker was in the menu only, and the symbol editor hid behind a small button
/// named "Symbols". On the Draw tab, at 1920 × 1080: Terminal opens its own form (strip, kind, how many, from
/// which number), Marker arms the Wire marker, and Symbol editor opens the editor.
#[test]
fn the_draw_tab_has_buttons_for_terminal_wire_marker_and_the_symbol_editor() {
    use ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    let _ = d.shell_mut().dispatch("file.new", &project);
    d.settle();
    let press = |d: &mut Driver, label: &str| {
        let at = d
            .find(label)
            .unwrap_or_else(|| panic!("the button {label}: {:?}", d.labels()));
        d.input(&click(at));
        d.settle();
    };
    press(&mut d, "Terminal");
    let form = match d.shell_mut().dialog() {
        Some(Dialog::Form(f)) => f.action,
        _ => panic!("the Terminal form"),
    };
    assert_eq!(form, "tool.place_terminal");
    d.input(&key("escape"));
    d.settle();
    assert!(d.shell_mut().dialog().is_none());
    press(&mut d, "Marker");
    assert_eq!(d.shell_mut().tool().name(), "marker");
    d.input(&key("escape"));
    d.settle();
    press(&mut d, "Symbol editor");
    assert!(d.shell_mut().symbol_editor().is_some(), "the editor opens");
    d.shell_mut().shutdown();
}

/// Making one page arrow the source was refused while its partner on another page was the source ("2 source points
/// and no star source"), until the partner was changed by hand. Choosing Source in the arrow's Properties now makes
/// the partner the target in the same step, and one undo takes both back.
#[test]
fn choosing_source_for_a_page_arrow_makes_its_partner_the_target_in_one_step() {
    let tmp = Tmp::new();
    let mut d = sample_on_101(&tmp);
    let direction = |d: &mut Driver, folio: &str| -> String {
        let _ = d.shell_mut().dispatch("page.goto", &json!({ "folio": folio }));
        d.settle();
        let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
        tables
            .reps
            .iter()
            .find_map(|r| r.ip.as_ref().filter(|i| i.name == "24V"))
            .expect("the page arrow 24V")
            .direction
            .clone()
    };
    assert_eq!(direction(&mut d, "101"), "source");
    assert_eq!(direction(&mut d, "102"), "target");
    let arrow = {
        let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
        tables
            .reps
            .iter()
            .find(|r| r.ip.as_ref().is_some_and(|i| i.name == "24V"))
            .unwrap()
            .id
            .to_string()
    };
    let _ = d.shell_mut().dispatch("select.set", &json!({ "ids": [arrow] }));
    d.settle();
    // Properties: the Direction row, its choices, Source.
    d.input(&click(d.find("Filter properties").unwrap()));
    d.input(&Input::Type {
        text: "Direction".to_owned(),
    });
    d.settle();
    // A click on the row's value opens its menu; Esc closes the menu and Enter opens the choices.
    let row = d.find("Direction").expect("the Direction row");
    d.input(&click((row.0 + 150, row.1)));
    d.input(&key("escape"));
    d.input(&key("enter"));
    d.settle();
    d.input(&click(d.find("Source").expect("the choice Source")));
    d.settle();
    assert_eq!(
        direction(&mut d, "102"),
        "source",
        "{:?}",
        state(&mut d)["properties"]
    );
    assert_eq!(
        direction(&mut d, "101"),
        "target",
        "the partner turned in the same step"
    );
    d.input(&key("ctrl+z"));
    d.settle();
    assert_eq!(direction(&mut d, "101"), "source");
    assert_eq!(direction(&mut d, "102"), "target", "one undo takes both back");
    d.shell_mut().shutdown();
}
